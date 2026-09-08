<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import AppModal from "./ui/AppModal.vue";
import AppButton from "./ui/AppButton.vue";
import IconGlyph from "./ui/IconGlyph.vue";
import { api } from "@/lib/api";
import type { BranchRef, LinkTarget, RepoInfo } from "@/lib/types";
import { messageOf } from "@/composables/useToasts";
import { displayPath } from "@/lib/format";

const props = defineProps<{
  repo: RepoInfo;
  worktreeRoot: string | null;
  home: string | null;
}>();

const emit = defineEmits<{
  close: [];
  created: [path: string, branch: string];
  rootChanged: [root: string];
}>();

type Mode = "new" | "existing";

const mode = ref<Mode>("new");
const branch = ref("");
const base = ref(props.repo.defaultBranch);
const branches = ref<BranchRef[]>([]);
const candidates = ref<LinkTarget[]>([]);
const selectedLinks = ref<Set<string>>(new Set());
const shareIgnored = ref(true);
const showAllCandidates = ref(false);
const busy = ref(false);
const error = ref("");
const path = ref("");

const localBranches = computed(() => branches.value.filter((b) => !b.isRemote));
const availableBranches = computed(() =>
  branches.value.filter((b) => !b.isCheckedOut),
);

onMounted(async () => {
  try {
    branches.value = await api.branchesList(props.repo.path);
  } catch {
    /* the picker degrades to free text */
  }
  try {
    candidates.value = await api.linkCandidates(props.repo.path);
    selectedLinks.value = new Set(
      candidates.value.filter((c) => c.recommended).map((c) => c.relPath),
    );
  } catch {
    /* nothing shareable, or git could not list ignores */
  }
});

// Keep the destination in step with the branch name as it is typed.
watch([branch, () => props.worktreeRoot], async () => {
  const root = props.worktreeRoot;
  if (!root || !branch.value.trim()) {
    path.value = "";
    return;
  }
  path.value = await api.suggestWorktreePath(root, props.repo.path, branch.value.trim());
});

watch(mode, () => {
  branch.value = "";
  error.value = "";
});

const recommended = computed(() => candidates.value.filter((c) => c.recommended));
const visibleCandidates = computed(() =>
  showAllCandidates.value ? candidates.value : recommended.value,
);

const canSubmit = computed(
  () => !busy.value && branch.value.trim().length > 0 && path.value.length > 0,
);

function toggleLink(rel: string) {
  const next = new Set(selectedLinks.value);
  if (next.has(rel)) next.delete(rel);
  else next.add(rel);
  selectedLinks.value = next;
}

async function pickRoot() {
  const chosen = await openDialog({
    directory: true,
    multiple: false,
    title: "Choose where new worktrees are created",
  });
  if (typeof chosen === "string") emit("rootChanged", chosen);
}

async function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  error.value = "";
  try {
    const existing = mode.value === "existing";
    const picked = existing ? branches.value.find((b) => b.name === branch.value.trim()) : null;
    // A remote branch becomes a local branch of the same short name.
    const localName = picked?.isRemote
      ? picked.name.split("/").slice(1).join("/")
      : branch.value.trim();
    const destination = picked?.isRemote
      ? await api.suggestWorktreePath(props.worktreeRoot!, props.repo.path, localName)
      : path.value;

    const result = await api.worktreeCreate({
      repoRoot: props.repo.path,
      path: destination,
      branch: localName,
      base: existing ? (picked?.isRemote ? picked.name : null) : base.value || null,
      useExistingBranch: existing,
      trackRemote: false,
      linkTargets: shareIgnored.value ? [...selectedLinks.value] : [],
    });
    emit("created", result.path, result.branch);
  } catch (e) {
    error.value = messageOf(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <AppModal title="New worktree" :subtitle="repo.name" wide @close="emit('close')">
    <div class="space-y-4">
      <!-- Create a branch, or give an existing one its own directory. -->
      <div class="inline-flex rounded border border-edge p-0.5">
        <button
          v-for="option in [
            { id: 'new' as Mode, label: 'Create a branch' },
            { id: 'existing' as Mode, label: 'Check out an existing branch' },
          ]"
          :key="option.id"
          class="rounded-sm px-2.5 py-1 text-sm transition-colors"
          :class="mode === option.id ? 'bg-accent text-accent-ink' : 'text-muted hover:text-ink'"
          @click="mode = option.id"
        >
          {{ option.label }}
        </button>
      </div>

      <div v-if="mode === 'new'" class="grid grid-cols-2 gap-3">
        <label class="block">
          <span class="mb-1 block text-sm text-muted">Branch name</span>
          <input
            v-model="branch"
            class="field font-mono"
            placeholder="feature/login"
            autofocus
            @keydown.enter="submit"
          />
        </label>
        <label class="block">
          <span class="mb-1 block text-sm text-muted">Start from</span>
          <input v-model="base" class="field font-mono" list="gwm-branches" />
          <datalist id="gwm-branches">
            <option v-for="b in branches" :key="b.name" :value="b.name" />
          </datalist>
        </label>
      </div>

      <label v-else class="block">
        <span class="mb-1 block text-sm text-muted">Branch</span>
        <input
          v-model="branch"
          class="field font-mono"
          list="gwm-available"
          placeholder="origin/feature/login"
          autofocus
          @keydown.enter="submit"
        />
        <datalist id="gwm-available">
          <option v-for="b in availableBranches" :key="b.name" :value="b.name">
            {{ b.subject }}
          </option>
        </datalist>
        <span class="mt-1 block text-xs text-faint">
          {{ localBranches.length }} local,
          {{ branches.length - localBranches.length }} remote. Branches already checked out
          elsewhere are not offered.
        </span>
      </label>

      <!-- Destination -->
      <div>
        <div class="mb-1 flex items-baseline justify-between">
          <span class="text-sm text-muted">Location</span>
          <button class="text-sm text-accent hover:underline" @click="pickRoot">
            {{ worktreeRoot ? "Change folder" : "Choose folder" }}
          </button>
        </div>
        <p
          v-if="!worktreeRoot"
          class="rounded border border-dashed border-edge px-2.5 py-2 text-sm text-muted"
        >
          Pick the folder where new worktrees should live. They are created next to each other,
          not inside the repository.
        </p>
        <p
          v-else
          data-selectable
          class="truncate rounded border border-edge bg-sunken px-2.5 py-2 font-mono text-sm"
          :class="path ? 'text-ink' : 'text-faint'"
          :title="path"
        >
          {{ path ? displayPath(path, home, 62) : `${displayPath(worktreeRoot, home, 58)}/…` }}
        </p>
      </div>

      <!-- Shared build artefacts -->
      <div v-if="candidates.length" class="rounded border border-edge">
        <label class="flex cursor-pointer items-start gap-2.5 px-3 py-2.5">
          <input v-model="shareIgnored" type="checkbox" class="mt-0.5 accent-accent" />
          <span class="min-w-0 flex-1">
            <span class="block text-base">Share ignored files with the new worktree</span>
            <span class="block text-xs text-muted">
              Symlinks things git ignores — <span class="font-mono">node_modules</span>,
              <span class="font-mono">.env</span> and the like — so the worktree is usable
              without reinstalling.
            </span>
          </span>
        </label>

        <div v-if="shareIgnored" class="border-t border-hairline">
          <ul class="max-h-[132px] overflow-y-auto px-3 py-2">
            <li v-for="c in visibleCandidates" :key="c.relPath">
              <label class="flex cursor-pointer items-center gap-2 py-0.5">
                <input
                  type="checkbox"
                  class="accent-accent"
                  :checked="selectedLinks.has(c.relPath)"
                  @change="toggleLink(c.relPath)"
                />
                <span class="truncate font-mono text-xs" :title="c.relPath">{{ c.relPath }}</span>
                <span v-if="c.isDir" class="text-2xs text-faint">folder</span>
              </label>
            </li>
          </ul>
          <button
            v-if="candidates.length > recommended.length"
            class="w-full border-t border-hairline px-3 py-1.5 text-left text-xs text-accent hover:bg-sunken"
            @click="showAllCandidates = !showAllCandidates"
          >
            {{
              showAllCandidates
                ? `Show only the ${recommended.length} suggested`
                : `Show all ${candidates.length} ignored paths`
            }}
          </button>
        </div>
      </div>

      <p v-if="error" data-selectable class="flex items-start gap-2 text-sm text-danger">
        <IconGlyph name="alert" :size="14" class="mt-0.5" />
        <span>{{ error }}</span>
      </p>
    </div>

    <template #actions>
      <AppButton variant="quiet" @click="emit('close')">Cancel</AppButton>
      <AppButton variant="primary" :disabled="!canSubmit" @click="submit">
        {{ busy ? "Creating…" : "Create worktree" }}
      </AppButton>
    </template>
  </AppModal>
</template>
