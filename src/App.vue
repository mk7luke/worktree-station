<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { homeDir } from "@tauri-apps/api/path";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import RepoSidebar from "./components/RepoSidebar.vue";
import WorktreeBoard from "./components/WorktreeBoard.vue";
import TerminalDock from "./components/TerminalDock.vue";
import CreateWorktreeDialog from "./components/CreateWorktreeDialog.vue";
import RemoveWorktreeDialog from "./components/RemoveWorktreeDialog.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import ToastStack from "./components/ToastStack.vue";

import { api } from "./lib/api";
import { useWorkspace, type WorktreeView } from "./composables/useWorkspace";
import { useTerminals } from "./composables/useTerminals";
import { useToasts } from "./composables/useToasts";
import { displayPath } from "./lib/format";
import type { Settings } from "./lib/types";

const {
  ready,
  settings,
  persist,
  setTheme,
  repos,
  activeRepo,
  editors,
  worktrees,
  activeWorktreeRoot,
  setRepoRoot,
  waitingCount,
  loadingRepo,
  init,
  addRepo,
  removeRepo,
  selectRepo,
  refreshWorktrees,
} = useWorkspace();

const terminals = useTerminals();
const { notify, fail } = useToasts();

const home = ref<string | null>(null);
const selectedPath = ref<string | null>(null);
const showCreate = ref(false);
const showSettings = ref(false);
const removing = ref<WorktreeView | null>(null);
const hooksInstalled = ref(false);

const liveTerminals = computed(
  () => new Set(terminals.sessions.value.filter((s) => s.alive).map((s) => s.id)),
);

// Keep a selection so the keyboard always has something to act on, and drop it
// when the selected worktree disappears.
watch(worktrees, (list) => {
  if (!list.length) {
    selectedPath.value = null;
  } else if (!list.some((w) => w.path === selectedPath.value)) {
    selectedPath.value = list[0]!.path;
  }
});

// Terminal typeface follows the setting, for sessions open now and later.
watch(
  () => [settings.value.terminalFont, settings.value.terminalFontSize] as const,
  ([font, size]) => terminals.configure(font, size),
  { immediate: true },
);

onMounted(async () => {
  home.value = await homeDir().catch(() => null);
  await init();
  hooksInstalled.value = await api.claudeHooksInstalled().catch(() => false);
  window.addEventListener("keydown", onKey);
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

const selected = computed(
  () => worktrees.value.find((w) => w.path === selectedPath.value) ?? null,
);

const dialogIsOpen = computed(() => showCreate.value || showSettings.value || !!removing.value);

/** Text fields — including the terminal's hidden input — keep the plain keys. */
function typingInto(target: EventTarget | null) {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

function moveSelection(delta: number) {
  const list = worktrees.value;
  if (!list.length) return;
  const current = list.findIndex((w) => w.path === selectedPath.value);
  const next = current === -1 ? 0 : Math.min(list.length - 1, Math.max(0, current + delta));
  selectedPath.value = list[next]!.path;
}

function onKey(e: KeyboardEvent) {
  // A dialog owns the keyboard entirely while it is up.
  if (dialogIsOpen.value) return;

  // Arrow keys move the selection, unless they are being typed somewhere —
  // the terminal needs them for history and cursor movement.
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    if (typingInto(e.target)) return;
    e.preventDefault();
    moveSelection(e.key === "ArrowDown" ? 1 : -1);
    return;
  }

  // Command shortcuts stay available even with the terminal focused.
  const mod = e.metaKey || e.ctrlKey;
  if (!mod) return;
  const target = selected.value;

  switch (e.key) {
    case "n":
      if (!activeRepo.value) return;
      e.preventDefault();
      showCreate.value = true;
      break;
    case "r":
      e.preventDefault();
      void refreshWorktrees();
      break;
    case ",":
      e.preventDefault();
      showSettings.value = true;
      break;
    case "t":
      if (!target) return;
      e.preventDefault();
      void openShell(target);
      break;
    case "Enter":
      if (!target) return;
      e.preventDefault();
      void openClaude(target);
      break;
    case "Backspace":
      if (!target || target.isMain) return;
      e.preventDefault();
      removing.value = target;
      break;
    case "`":
      e.preventDefault();
      if (terminals.sessions.value.length) terminals.dockOpen.value = !terminals.dockOpen.value;
      break;
  }
}

async function pickRepo() {
  const chosen = await openDialog({
    directory: true,
    multiple: false,
    title: "Choose a git repository",
  });
  if (typeof chosen === "string") await addRepo(chosen);
}

function patch(next: Partial<Settings>) {
  if (next.theme) setTheme(next.theme);
  void persist(next);
}

async function openClaude(wt: WorktreeView) {
  await terminals.open({
    id: wt.canonical,
    label: wt.branch ?? wt.name,
    cwd: wt.path,
    // Only type the command for a fresh session; an existing one just comes forward.
    command: liveTerminals.value.has(wt.canonical) ? undefined : "claude",
  });
}

async function openShell(wt: WorktreeView) {
  await terminals.open({ id: wt.canonical, label: wt.branch ?? wt.name, cwd: wt.path });
}

async function openEditor(wt: WorktreeView, editorId: string) {
  try {
    await api.openInEditor(editorId, wt.path);
  } catch (e) {
    fail("Could not open your editor", e);
  }
}

async function pruneStale() {
  if (!activeRepo.value) return;
  try {
    await api.worktreePrune(activeRepo.value.path);
    await refreshWorktrees();
    notify("Stale entries removed");
  } catch (e) {
    fail("Could not clean up stale entries", e);
  }
}

async function reveal(wt: WorktreeView) {
  try {
    await api.revealInFileManager(wt.path);
  } catch (e) {
    fail("Could not open the folder", e);
  }
}

async function onCreated(path: string, branch: string) {
  showCreate.value = false;
  await refreshWorktrees();
  selectedPath.value = path;
  notify("Worktree created", `${branch} is ready at ${displayPath(path, home.value, 60)}`);
}

async function onRemoved() {
  const name = removing.value?.branch ?? removing.value?.name ?? "Worktree";
  const id = removing.value?.canonical;
  removing.value = null;
  if (id && liveTerminals.value.has(id)) await terminals.close(id);
  await refreshWorktrees();
  notify("Worktree removed", `${name} is gone from disk.`);
}
</script>

<template>
  <div v-if="ready" class="flex h-full flex-col overflow-hidden">
    <div class="flex min-h-0 flex-1">
      <RepoSidebar
        :repos="repos"
        :active-path="activeRepo?.path ?? null"
        :waiting-count="waitingCount"
        @add="pickRepo"
        @select="(p) => selectRepo(p)"
        @forget="(p) => removeRepo(p)"
        @settings="showSettings = true"
      />

      <WorktreeBoard
        :repo="activeRepo"
        :worktrees="worktrees"
        :loading="loadingRepo"
        :home="home"
        :editors="editors"
        :preferred-editor="settings.preferredEditor"
        :selected-path="selectedPath"
        :live-terminals="liveTerminals"
        @add-repo="pickRepo"
        @create="showCreate = true"
        @refresh="refreshWorktrees()"
        @prune="pruneStale"
        @select="(wt) => (selectedPath = wt.path)"
        @claude="openClaude"
        @terminal="openShell"
        @editor="openEditor"
        @reveal="reveal"
        @remove="(wt) => (removing = wt)"
      />
    </div>

    <TerminalDock />

    <CreateWorktreeDialog
      v-if="showCreate && activeRepo"
      :repo="activeRepo"
      :worktree-root="activeWorktreeRoot"
      :home="home"
      @close="showCreate = false"
      @created="onCreated"
      @root-changed="(root) => activeRepo && setRepoRoot(activeRepo.path, root)"
    />

    <RemoveWorktreeDialog
      v-if="removing && activeRepo"
      :repo-root="activeRepo.path"
      :worktree="removing"
      :home="home"
      @close="removing = null"
      @removed="onRemoved"
    />

    <SettingsDialog
      v-if="showSettings"
      :settings="settings"
      :active-repo="activeRepo"
      :editors="editors"
      :home="home"
      :hooks-installed="hooksInstalled"
      @close="showSettings = false"
      @patch="patch"
      @repo-root="(root) => activeRepo && setRepoRoot(activeRepo.path, root)"
      @hooks-changed="(v) => (hooksInstalled = v)"
    />

    <ToastStack />
  </div>
</template>
