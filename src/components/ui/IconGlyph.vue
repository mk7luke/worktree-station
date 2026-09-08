<script setup lang="ts">
/** One inline SVG sprite, so icon geometry lives in a single place. */
const paths: Record<string, string[]> = {
  branch: ["M6 3v12", "M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M18 9a9 9 0 0 1-9 9"],
  folder: ["M4 20a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5l2 2h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1Z"],
  terminal: ["m5 8 3.5 3.5L5 15", "M12 16h6"],
  plus: ["M12 5v14", "M5 12h14"],
  refresh: ["M3 12a9 9 0 0 1 15.3-6.4L21 8", "M21 3v5h-5", "M21 12a9 9 0 0 1-15.3 6.4L3 16", "M3 21v-5h5"],
  settings: ["M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1V21a2 2 0 1 1-4 0v-.1A1.6 1.6 0 0 0 7.9 19.4l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.6 1.6 0 0 0 4 13.9H4a2 2 0 1 1 0-4h.1A1.6 1.6 0 0 0 5.1 7.2L5 7.1a2 2 0 1 1 2.8-2.8l.1.1A1.6 1.6 0 0 0 10 4.6V4a2 2 0 1 1 4 0v.1a1.6 1.6 0 0 0 2.7 1.1l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0 1.1 2.7H21a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1Z"],
  trash: ["M4 7h16", "M10 11v6", "M14 11v6", "M6 7l1 12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-12", "M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"],
  close: ["M6 6l12 12", "M18 6 6 18"],
  chevronDown: ["m6 9 6 6 6-6"],
  chevronRight: ["m9 6 6 6-6 6"],
  check: ["m5 13 4 4L19 7"],
  alert: ["M12 9v4", "M12 17h.01", "M10.3 4 2.6 17.3A2 2 0 0 0 4.3 20h15.4a2 2 0 0 0 1.7-2.7L13.7 4a2 2 0 0 0-3.4 0Z"],
  code: ["m8 6-6 6 6 6", "m16 6 6 6-6 6"],
  sparkle: ["M12 3 13.8 9 20 10.8 13.8 12.6 12 19 10.2 12.6 4 10.8 10.2 9Z"],
  reveal: ["M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z", "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"],
  arrowUp: ["M12 19V5", "m6 11 6-6 6 6"],
  arrowDown: ["M12 5v14", "m6 13 6 6 6-6"],
  pencil: ["M4 20h4L20 8l-4-4L4 16Z", "m14 6 4 4"],
  stack: ["M4 8h16v12H4Z", "M7 8V5h13v12h-3"],
};

const props = withDefaults(defineProps<{ name: keyof typeof paths | string; size?: number }>(), {
  size: 15,
});
</script>

<template>
  <svg
    :width="props.size"
    :height="props.size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    class="shrink-0"
  >
    <!-- The terminal glyph needs its frame; the rest are pure strokes. -->
    <rect v-if="props.name === 'terminal'" x="3" y="4" width="18" height="16" rx="2" />
    <path v-for="(d, i) in paths[props.name] ?? []" :key="i" :d="d" />
  </svg>
</template>
