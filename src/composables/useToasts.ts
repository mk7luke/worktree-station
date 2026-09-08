import { ref } from "vue";

export interface Toast {
  id: number;
  tone: "info" | "error";
  title: string;
  detail?: string;
}

const toasts = ref<Toast[]>([]);
let nextId = 1;

function push(tone: Toast["tone"], title: string, detail?: string) {
  const id = nextId++;
  toasts.value.push({ id, tone, title, detail });
  // Errors stay until dismissed; confirmations get out of the way on their own.
  if (tone !== "error") {
    setTimeout(() => dismiss(id), 4000);
  }
  return id;
}

function dismiss(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id);
}

/** Turn whatever a rejected command threw into a readable sentence. */
export function messageOf(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function useToasts() {
  return {
    toasts,
    dismiss,
    notify: (title: string, detail?: string) => push("info", title, detail),
    fail: (title: string, error?: unknown) =>
      push("error", title, error === undefined ? undefined : messageOf(error)),
  };
}
