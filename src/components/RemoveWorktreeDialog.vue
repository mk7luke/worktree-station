<script setup lang="ts">
import { computed, ref } from "vue";
import AppModal from "./ui/AppModal.vue";
import AppButton from "./ui/AppButton.vue";
import IconGlyph from "./ui/IconGlyph.vue";
import { api } from "@/lib/api";
import { messageOf } from "@/composables/useToasts";
import { displayPath } from "@/lib/format";
import type { WorktreeView } from "@/composables/useWorkspace";

const props = defineProps<{ repoRoot: string; worktree: WorktreeView; home: string | null }>();
const emit = defineEmits<{ close: []; removed: [] }>();

const deleteBranch = ref(false);
const deleteRemoteBranch = ref(false);
const force = ref(false);
const busy = ref(false);
const error = ref("");

const uncommitted = computed(() => {
  const s = props.worktree.status;
  if (!s) return 0;
  return s.staged + s.unstaged + s.untracked + s.conflicted;
});

const hasUnpushed = computed(() => (props.worktree.status?.ahead ?? 0) > 0);

/** e.g. "origin/feat/login" — absent when the branch was never pushed. */
const upstream = computed(() => props.worktree.status?.upstream ?? null);

async function submit() {
  busy.value = true;
  error.value = "";
  try {
    await api.worktreeRemove({
      repoRoot: props.repoRoot,
      worktreePath: props.worktree.path,
      force: force.value,
      deleteBranch: deleteBranch.value,
      deleteRemoteBranch: deleteRemoteBranch.value,
    });
    emit("removed");
  } catch (e) {
    error.value = messageOf(e);
    // git refuses a dirty worktree without --force; offer that as a decision
    // rather than forcing on the user's behalf.
    force.value = true;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <AppModal
    title="Remove this worktree?"
    :subtitle="worktree.branch ?? worktree.name"
    @close="emit('close')"
  >
    <p class="text-base leading-relaxed">
      The folder
      <span data-selectable class="font-mono text-sm">{{ displayPath(worktree.path, home, 64) }}</span>
      will be deleted from disk. Commits already pushed are unaffected.
    </p>

    <ul v-if="uncommitted || hasUnpushed" class="mt-3 space-y-1.5">
      <li v-if="uncommitted" class="flex items-start gap-2 text-base text-waiting">
        <IconGlyph name="alert" :size="14" class="mt-0.5" />
        <span>{{ uncommitted }} uncommitted {{ uncommitted === 1 ? "change" : "changes" }} will be lost.</span>
      </li>
      <li v-if="hasUnpushed" class="flex items-start gap-2 text-base text-waiting">
        <IconGlyph name="arrowUp" :size="14" class="mt-0.5" />
        <span>
          {{ worktree.status?.ahead }} commit{{ worktree.status?.ahead === 1 ? "" : "s" }} here
          {{ worktree.status?.ahead === 1 ? "has" : "have" }} not been pushed.
        </span>
      </li>
    </ul>

    <div class="mt-4 space-y-2.5">
      <label v-if="worktree.branch" class="flex cursor-pointer items-start gap-2.5">
        <input v-model="deleteBranch" type="checkbox" class="mt-0.5 accent-accent" />
        <span class="min-w-0">
          <span class="block text-base">
            Delete the local branch
            <span class="font-mono text-sm">{{ worktree.branch }}</span>
          </span>
          <span class="block text-xs text-muted">Removes it from this machine only.</span>
        </span>
      </label>

      <label
        v-if="worktree.branch && upstream"
        class="flex cursor-pointer items-start gap-2.5 rounded border border-edge px-2.5 py-2"
        :class="deleteRemoteBranch ? 'border-danger bg-danger/5' : ''"
      >
        <input v-model="deleteRemoteBranch" type="checkbox" class="mt-0.5 accent-accent" />
        <span class="min-w-0">
          <span class="block text-base">
            Delete <span class="font-mono text-sm">{{ upstream }}</span> on the remote
          </span>
          <span class="block text-xs" :class="deleteRemoteBranch ? 'text-danger' : 'text-muted'">
            This removes the branch for everyone on the project, not just you.
          </span>
        </span>
      </label>

      <p v-else-if="worktree.branch" class="text-xs text-faint">
        This branch was never pushed, so there is nothing on a remote to delete.
      </p>
      <label v-if="uncommitted || force" class="flex cursor-pointer items-center gap-2.5">
        <input v-model="force" type="checkbox" class="accent-accent" />
        <span class="text-base">Discard uncommitted changes</span>
      </label>
    </div>

    <p v-if="error" data-selectable class="mt-3 flex items-start gap-2 text-sm text-danger">
      <IconGlyph name="alert" :size="14" class="mt-0.5" />
      <span>{{ error }}</span>
    </p>

    <template #actions>
      <AppButton variant="quiet" @click="emit('close')">Keep it</AppButton>
      <AppButton variant="danger" :disabled="busy" @click="submit">
        {{ busy ? "Removing…" : "Remove worktree" }}
      </AppButton>
    </template>
  </AppModal>
</template>
