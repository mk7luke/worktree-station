<script setup lang="ts">
import IconGlyph from "./ui/IconGlyph.vue";
import { useToasts } from "@/composables/useToasts";

const { toasts, dismiss } = useToasts();
</script>

<template>
  <div class="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-[340px] flex-col gap-2">
    <div
      v-for="toast in toasts"
      :key="toast.id"
      class="pointer-events-auto flex items-start gap-2 rounded-md border border-edge bg-raised px-3 py-2.5"
      :style="{ boxShadow: 'var(--shadow-pop)' }"
    >
      <IconGlyph
        :name="toast.tone === 'error' ? 'alert' : 'check'"
        :size="14"
        :class="toast.tone === 'error' ? 'mt-0.5 text-danger' : 'mt-0.5 text-resting'"
      />
      <div class="min-w-0 flex-1">
        <p class="text-base font-medium leading-snug">{{ toast.title }}</p>
        <p v-if="toast.detail" data-selectable class="mt-0.5 break-words text-xs text-muted">
          {{ toast.detail }}
        </p>
      </div>
      <button class="rounded p-0.5 text-faint hover:text-ink" aria-label="Dismiss" @click="dismiss(toast.id)">
        <IconGlyph name="close" :size="12" />
      </button>
    </div>
  </div>
</template>
