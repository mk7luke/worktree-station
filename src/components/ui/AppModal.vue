<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import IconGlyph from "./IconGlyph.vue";

const props = defineProps<{ title: string; subtitle?: string; wide?: boolean }>();
const emit = defineEmits<{ close: [] }>();

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-start justify-center p-8 pt-titlebar">
    <div class="absolute inset-0 bg-ink/25" @click="emit('close')" />
    <div
      role="dialog"
      aria-modal="true"
      class="relative mt-6 w-full overflow-hidden rounded-lg border border-edge bg-surface"
      :class="props.wide ? 'max-w-[620px]' : 'max-w-[460px]'"
      :style="{ boxShadow: 'var(--shadow-pop)' }"
    >
      <header class="flex items-start gap-3 border-b border-hairline px-4 py-3">
        <div class="min-w-0 flex-1">
          <h2 class="text-md font-semibold leading-tight">{{ title }}</h2>
          <p v-if="subtitle" class="mt-0.5 truncate text-sm text-muted">{{ subtitle }}</p>
        </div>
        <button
          type="button"
          class="-mr-1 -mt-0.5 rounded p-1 text-faint hover:bg-sunken hover:text-ink"
          aria-label="Close"
          @click="emit('close')"
        >
          <IconGlyph name="close" :size="14" />
        </button>
      </header>

      <div class="max-h-[62vh] overflow-y-auto px-4 py-4">
        <slot />
      </div>

      <footer
        v-if="$slots.actions"
        class="flex items-center justify-end gap-2 border-t border-hairline bg-sunken px-4 py-3"
      >
        <slot name="actions" />
      </footer>
    </div>
  </div>
</template>
