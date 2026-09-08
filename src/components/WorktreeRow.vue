<script setup lang="ts">
import { computed } from "vue";
import IconGlyph from "./ui/IconGlyph.vue";
import type { WorktreeView } from "@/composables/useWorkspace";
import type { Editor } from "@/lib/types";
import { displayPath, shortSha } from "@/lib/format";

const props = defineProps<{
  worktree: WorktreeView;
  home: string | null;
  editors: Editor[];
  preferredEditor: string | null;
  hasTerminal: boolean;
  selected: boolean;
}>();

const emit = defineEmits<{
  select: [];
  claude: [];
  terminal: [];
  editor: [editorId: string];
  reveal: [];
  remove: [];
}>();

/** The left rule answers one question: is anything waiting on me here? */
const rule = computed(() => {
  switch (props.worktree.agent) {
    case "waiting":
      return { class: "bg-waiting breathe", label: "Waiting for you" };
    case "working":
      return { class: "bg-working", label: "Claude is working" };
    case "idle":
      return { class: "bg-resting", label: "Claude is idle" };
    default:
      return { class: "bg-transparent", label: "" };
  }
});

const st = computed(() => props.worktree.status);

const dirty = computed(() => {
  const s = st.value;
  if (!s) return null;
  const total = s.staged + s.unstaged + s.untracked + s.conflicted;
  if (total === 0) return null;
  return { total, conflicted: s.conflicted };
});

const editor = computed(
  () =>
    props.editors.find((e) => e.id === props.preferredEditor) ?? props.editors[0] ?? null,
);
</script>

<template>
  <div
    class="group relative flex cursor-default items-stretch gap-0 transition-colors"
    :class="selected ? 'bg-sunken' : 'hover:bg-sunken/60'"
    @click="emit('select')"
  >
    <!-- Agent state, encoded as the row's leading edge. -->
    <div class="w-[3px] shrink-0" :class="rule.class" :title="rule.label" />

    <div class="min-w-0 flex-1 px-3 py-2.5">
      <div class="flex items-baseline gap-2">
        <span class="truncate font-mono text-base font-medium text-ink" :title="worktree.branch ?? worktree.head">
          {{ worktree.branch ?? shortSha(worktree.head) }}
        </span>
        <span v-if="worktree.isDetached" class="text-xs text-faint">detached</span>
        <span
          v-if="worktree.isMain"
          class="rounded-sm border border-edge px-1 text-2xs leading-[15px] text-muted"
        >main checkout</span>
        <span v-if="worktree.locked" class="text-xs text-waiting">locked</span>

        <span class="flex-1" />

        <!-- Ledger column: divergence, then working-tree state. -->
        <span
          v-if="st && (st.ahead || st.behind)"
          class="flex items-center gap-1.5 font-mono text-xs text-muted"
        >
          <span v-if="st.ahead" class="flex items-center" :title="`${st.ahead} to push`">
            <IconGlyph name="arrowUp" :size="11" />{{ st.ahead }}
          </span>
          <span v-if="st.behind" class="flex items-center" :title="`${st.behind} to pull`">
            <IconGlyph name="arrowDown" :size="11" />{{ st.behind }}
          </span>
        </span>
        <span
          v-if="dirty"
          class="font-mono text-xs"
          :class="dirty.conflicted ? 'text-danger' : 'text-waiting'"
          :title="
            st
              ? `${st.staged} staged, ${st.unstaged} unstaged, ${st.untracked} untracked`
              : undefined
          "
        >
          {{ dirty.conflicted ? `${dirty.conflicted} conflicted` : `${dirty.total} changed` }}
        </span>
        <span v-else-if="worktree.prunable" class="text-xs text-danger">folder missing</span>
        <span v-else-if="st" class="text-xs text-faint">clean</span>
      </div>

      <div class="mt-0.5 truncate font-mono text-xs text-faint" :title="worktree.path">
        {{ displayPath(worktree.path, home) }}
      </div>

      <div class="mt-1 flex items-center gap-2">
        <p v-if="st?.subject" class="min-w-0 flex-1 truncate text-xs text-muted">
          {{ st.subject }}<span class="text-faint"> — {{ st.relativeDate }}, {{ st.author }}</span>
        </p>
        <p v-else class="min-w-0 flex-1 truncate text-xs text-faint">No commits yet</p>

        <!-- Actions stay out of the way until the row is in play. -->
        <div
          class="flex shrink-0 items-center gap-0.5 transition-opacity"
          :class="selected ? 'opacity-100' : 'opacity-0 group-hover:opacity-100 focus-within:opacity-100'"
          @click.stop
        >
          <button
            class="rounded p-1 hover:bg-raised"
            :class="hasTerminal ? 'text-accent' : 'text-muted hover:text-ink'"
            :title="hasTerminal ? 'Show this terminal' : 'Run Claude here'"
            @click="emit('claude')"
          >
            <IconGlyph name="sparkle" :size="14" />
          </button>
          <button
            class="rounded p-1 text-muted hover:bg-raised hover:text-ink"
            title="Open a shell here"
            @click="emit('terminal')"
          >
            <IconGlyph name="terminal" :size="14" />
          </button>
          <button
            v-if="editor"
            class="rounded p-1 text-muted hover:bg-raised hover:text-ink"
            :title="`Open in ${editor.name}`"
            @click="emit('editor', editor.id)"
          >
            <IconGlyph name="code" :size="14" />
          </button>
          <button
            class="rounded p-1 text-muted hover:bg-raised hover:text-ink"
            title="Show in Finder"
            @click="emit('reveal')"
          >
            <IconGlyph name="folder" :size="14" />
          </button>
          <button
            v-if="!worktree.isMain"
            class="rounded p-1 text-muted hover:bg-raised hover:text-danger"
            title="Remove this worktree"
            @click="emit('remove')"
          >
            <IconGlyph name="trash" :size="14" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
