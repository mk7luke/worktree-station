/**
 * Title-bar behaviour.
 *
 * Dragging is driven explicitly rather than through `data-tauri-drag-region`,
 * so there is one code path that we can reason about, and so double-click to
 * zoom behaves the way it does in every other Mac window.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";

const INTERACTIVE = "button, a, input, select, textarea, [role='button']";

export function useWindowChrome() {
  function onPointerDown(event: PointerEvent) {
    // Left button only, and never when the press landed on a control.
    if (event.button !== 0) return;
    if ((event.target as HTMLElement | null)?.closest(INTERACTIVE)) return;

    if (event.detail === 2) {
      void getCurrentWindow().toggleMaximize();
      return;
    }
    void getCurrentWindow().startDragging();
  }

  return { onPointerDown };
}
