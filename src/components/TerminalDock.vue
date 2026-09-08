<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import IconGlyph from "./ui/IconGlyph.vue";
import { useTerminals } from "@/composables/useTerminals";

const { sessions, activeId, dockOpen, select, close, mount, resize, focus } = useTerminals();

const host = ref<HTMLElement | null>(null);
const height = ref(280);
const dragging = ref(false);
const MIN = 132;

function clamp(value: number) {
  return Math.max(MIN, Math.min(value, window.innerHeight - 220));
}

function startDrag(event: PointerEvent) {
  dragging.value = true;
  const startY = event.clientY;
  const startHeight = height.value;
  const el = event.currentTarget as HTMLElement;
  el.setPointerCapture(event.pointerId);

  const move = (e: PointerEvent) => {
    height.value = clamp(startHeight + (startY - e.clientY));
  };
  const up = (e: PointerEvent) => {
    dragging.value = false;
    el.releasePointerCapture(e.pointerId);
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    resize();
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

// The active session's element is moved into the dock rather than recreated,
// so its scrollback survives every tab switch.
watch([activeId, dockOpen], async () => {
  await nextTick();
  mount(host.value);
  focus();
});

watch(height, () => resize());

let observer: ResizeObserver | null = null;
onMounted(() => {
  mount(host.value);
  observer = new ResizeObserver(() => resize());
  if (host.value) observer.observe(host.value);
});
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <section
    v-if="dockOpen && sessions.length"
    class="flex shrink-0 flex-col border-t border-edge bg-surface"
    :style="{ height: `${height}px` }"
  >
    <!-- Drag to resize -->
    <div
      class="group -mt-1 h-2 shrink-0 cursor-ns-resize"
      :class="dragging ? 'bg-accent/25' : 'hover:bg-accent/15'"
      role="separator"
      aria-label="Resize terminal"
      @pointerdown="startDrag"
    />

    <header class="flex h-[30px] shrink-0 items-center gap-0.5 border-b border-hairline px-1.5">
      <button
        v-for="session in sessions"
        :key="session.id"
        class="group flex max-w-[190px] items-center gap-1.5 rounded px-2 py-1 text-sm transition-colors"
        :class="
          session.id === activeId
            ? 'bg-sunken text-ink'
            : 'text-muted hover:bg-sunken/60 hover:text-ink'
        "
        @click="select(session.id)"
      >
        <span
          class="h-1.5 w-1.5 shrink-0 rounded-full"
          :class="
            !session.alive
              ? 'bg-faint'
              : session.unread
                ? 'bg-working'
                : 'bg-resting'
          "
        />
        <span class="truncate font-mono text-xs">{{ session.label }}</span>
        <span
          class="-mr-1 rounded p-0.5 opacity-0 hover:bg-raised group-hover:opacity-70"
          role="button"
          aria-label="Close session"
          @click.stop="close(session.id)"
        >
          <IconGlyph name="close" :size="11" />
        </span>
      </button>

      <span class="flex-1" />

      <button
        class="rounded p-1 text-faint hover:bg-sunken hover:text-ink"
        title="Hide the terminal"
        @click="dockOpen = false"
      >
        <IconGlyph name="chevronDown" :size="14" />
      </button>
    </header>

    <div ref="host" class="min-h-0 flex-1" @click="focus()" />
  </section>
</template>
