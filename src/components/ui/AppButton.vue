<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: "primary" | "standard" | "quiet" | "danger";
    size?: "sm" | "md";
    disabled?: boolean;
    title?: string;
  }>(),
  { variant: "standard", size: "md", disabled: false },
);
</script>

<template>
  <button
    type="button"
    :disabled="disabled"
    :title="title"
    class="inline-flex items-center justify-center gap-1.5 rounded font-medium
           whitespace-nowrap transition-colors disabled:opacity-40 disabled:pointer-events-none
           active:translate-y-px"
    :class="[
      size === 'sm' ? 'h-[24px] px-2 text-sm' : 'h-[28px] px-2.5 text-base',
      variant === 'primary' && 'bg-accent text-accent-ink hover:brightness-110',
      variant === 'standard' &&
        'bg-raised text-ink border border-edge hover:bg-sunken',
      variant === 'quiet' && 'text-muted hover:text-ink hover:bg-sunken',
      variant === 'danger' && 'bg-danger text-accent-ink hover:brightness-110',
    ]"
  >
    <slot />
  </button>
</template>
