<script setup lang="ts">
import AppButton from "./ui/AppButton.vue";
import IconGlyph from "./ui/IconGlyph.vue";
import WorktreeRow from "./WorktreeRow.vue";
import { computed } from "vue";
import { useWindowChrome } from "@/composables/useWindowChrome";
import type { WorktreeView } from "@/composables/useWorkspace";
import type { Editor, RepoInfo } from "@/lib/types";

const props = defineProps<{
  repo: RepoInfo | null;
  worktrees: WorktreeView[];
  loading: boolean;
  home: string | null;
  editors: Editor[];
  preferredEditor: string | null;
  selectedPath: string | null;
  liveTerminals: Set<string>;
}>();

const { onPointerDown } = useWindowChrome();

const stale = computed(() => props.worktrees.filter((w) => w.prunable).length);

const emit = defineEmits<{
  prune: [];
  addRepo: [];
  create: [];
  refresh: [];
  select: [worktree: WorktreeView];
  claude: [worktree: WorktreeView];
  terminal: [worktree: WorktreeView];
  editor: [worktree: WorktreeView, editorId: string];
  reveal: [worktree: WorktreeView];
  remove: [worktree: WorktreeView];
}>();
</script>

<template>
  <main class="flex min-h-0 min-w-0 flex-1 flex-col bg-window">
    <!-- Title bar row. The handler ignores presses that land on a control, so
         the buttons on the right stay clickable. -->
    <header class="flex h-titlebar shrink-0 items-center gap-2 px-4" @pointerdown="onPointerDown">
      <template v-if="repo">
        <h1 class="truncate text-md font-semibold">{{ repo.name }}</h1>
        <span class="font-mono text-xs text-faint">{{ repo.headBranch ?? "detached" }}</span>
      </template>
      <span class="flex-1" />
      <AppButton v-if="repo" variant="quiet" size="sm" title="Refresh" @click="emit('refresh')">
        <IconGlyph name="refresh" :size="14" />
      </AppButton>
      <AppButton v-if="repo" variant="primary" size="sm" @click="emit('create')">
        <IconGlyph name="plus" :size="14" />
        New worktree
      </AppButton>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
      <!-- No repositories at all -->
      <div v-if="!repo" class="flex h-full flex-col items-center justify-center px-8 text-center">
        <div class="mb-4 text-faint"><IconGlyph name="stack" :size="34" /></div>
        <h2 class="text-lg font-semibold">Add a repository to begin</h2>
        <p class="mt-1.5 max-w-[380px] text-base leading-relaxed text-muted">
          Every worktree is a separate checkout of the same repository, so several agents can
          work at once without touching each other's files.
        </p>
        <AppButton variant="primary" class="mt-5" @click="emit('addRepo')">
          <IconGlyph name="plus" :size="14" />
          Add a repository
        </AppButton>
      </div>

      <template v-else>
          <div v-if="loading && !worktrees.length" class="pt-10 text-center text-base text-muted">
            Reading worktrees…
          </div>

        <template v-else>
          <div
            v-if="stale"
            class="mb-3 flex items-center gap-2.5 rounded border border-edge bg-surface px-3 py-2"
          >
            <IconGlyph name="alert" :size="14" class="text-waiting" />
            <p class="min-w-0 flex-1 text-base">
              {{ stale }} {{ stale === 1 ? "entry points" : "entries point" }} at
              {{ stale === 1 ? "a folder" : "folders" }} that no longer
              {{ stale === 1 ? "exists" : "exist" }}.
            </p>
            <AppButton size="sm" @click="emit('prune')">Clean up</AppButton>
          </div>

          <div class="list-shell row-divide">
            <WorktreeRow
              v-for="wt in worktrees"
              :key="wt.path"
              :worktree="wt"
              :home="home"
              :editors="editors"
              :preferred-editor="preferredEditor"
              :has-terminal="liveTerminals.has(wt.canonical)"
              :selected="wt.path === selectedPath"
              @select="emit('select', wt)"
              @claude="emit('claude', wt)"
              @terminal="emit('terminal', wt)"
              @editor="(id) => emit('editor', wt, id)"
              @reveal="emit('reveal', wt)"
              @remove="emit('remove', wt)"
            />
          </div>
        </template>

        <p v-if="worktrees.length === 1" class="mt-3 px-1 text-sm leading-relaxed text-faint">
          Only the main checkout so far. Create a worktree to give a branch — and the agent
          working on it — a directory of its own.
        </p>
      </template>
    </div>
  </main>
</template>
