<script setup lang="ts">
import IconGlyph from "./ui/IconGlyph.vue";
import { useWindowChrome } from "@/composables/useWindowChrome";
import type { RepoInfo } from "@/lib/types";

defineProps<{
  repos: RepoInfo[];
  activePath: string | null;
  waitingCount: number;
}>();

const { onPointerDown } = useWindowChrome();

const emit = defineEmits<{
  select: [path: string];
  add: [];
  forget: [path: string];
  settings: [];
}>();
</script>

<template>
  <aside class="flex w-[214px] shrink-0 flex-col border-r border-hairline bg-window">
    <!-- Space for the traffic lights; the whole strip drags the window. -->
    <div class="h-titlebar shrink-0" @pointerdown="onPointerDown" />

    <div class="flex items-center justify-between px-3 pb-1.5">
      <h2 class="text-sm font-medium text-muted">Repositories</h2>
      <button
        class="rounded p-1 text-faint hover:bg-sunken hover:text-ink"
        title="Add a repository"
        @click="emit('add')"
      >
        <IconGlyph name="plus" :size="14" />
      </button>
    </div>

    <nav class="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
      <p v-if="!repos.length" class="px-1 py-2 text-xs leading-relaxed text-faint">
        No repositories yet. Add one to see its worktrees.
      </p>

      <button
        v-for="repo in repos"
        :key="repo.path"
        class="group mb-0.5 flex w-full items-center gap-2 rounded px-2 py-1.5 text-left"
        :class="
          repo.path === activePath
            ? 'bg-accent text-accent-ink'
            : 'text-ink hover:bg-sunken'
        "
        @click="emit('select', repo.path)"
      >
        <IconGlyph name="stack" :size="14" :class="repo.path === activePath ? '' : 'text-faint'" />
        <span class="min-w-0 flex-1">
          <span class="block truncate text-base leading-tight">{{ repo.name }}</span>
          <span
            class="block truncate font-mono text-2xs leading-tight"
            :class="repo.path === activePath ? 'opacity-75' : 'text-faint'"
          >{{ repo.headBranch ?? "detached" }}</span>
        </span>
        <span
          class="rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-70 hover:!opacity-100"
          role="button"
          title="Remove from this list"
          @click.stop="emit('forget', repo.path)"
        >
          <IconGlyph name="close" :size="12" />
        </span>
      </button>
    </nav>

    <footer class="border-t border-hairline p-2">
      <p
        v-if="waitingCount"
        class="mb-1.5 flex items-center gap-1.5 rounded bg-waiting/10 px-2 py-1.5 text-xs text-waiting"
      >
        <IconGlyph name="alert" :size="13" />
        {{ waitingCount }} {{ waitingCount === 1 ? "agent needs" : "agents need" }} you
      </p>
      <button
        class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-base text-muted hover:bg-sunken hover:text-ink"
        @click="emit('settings')"
      >
        <IconGlyph name="settings" :size="14" />
        Settings
      </button>
    </footer>
  </aside>
</template>
