/**
 * Embedded terminal sessions.
 *
 * Each session's xterm instance lives in a detached element that is moved into
 * the dock when its tab is shown, so switching tabs never loses scrollback.
 */
import { computed, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { api } from "@/lib/api";
import { useToasts } from "./useToasts";
import { useTheme } from "./useTheme";
import { terminalFontStack } from "@/lib/fonts";

export interface TerminalSession {
  id: string;
  label: string;
  cwd: string;
  alive: boolean;
  /** Set when output arrived while the tab was in the background. */
  unread: boolean;
}

interface Instance {
  term: Terminal;
  fit: FitAddon;
  host: HTMLDivElement;
}

const sessions = ref<TerminalSession[]>([]);
const fontFamily = ref<string | null>(null);
const fontSize = ref(12);
const activeId = ref<string | null>(null);
const dockOpen = ref(false);
const instances = new Map<string, Instance>();
let listening = false;

const { fail } = useToasts();
const { theme } = useTheme();

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** xterm needs literal colours, so read them back out of the design tokens. */
function terminalTheme() {
  const dark = document.documentElement.dataset.theme === "dark";
  return {
    background: cssVar("--surface"),
    foreground: cssVar("--ink"),
    cursor: cssVar("--accent"),
    cursorAccent: cssVar("--surface"),
    selectionBackground: dark ? "rgba(10,132,255,0.35)" : "rgba(0,113,227,0.22)",
    black: dark ? "#3d3d43" : "#33333a",
    red: "#e05a52",
    green: "#3aab63",
    yellow: "#c98a1b",
    blue: "#3d8ce0",
    magenta: "#a86bd1",
    cyan: "#2d9fa8",
    white: dark ? "#d8d8dd" : "#4a4a52",
    brightBlack: dark ? "#6e6e76" : "#8a8a92",
    brightRed: "#ff7b70",
    brightGreen: "#4cc97c",
    brightYellow: "#e8ad3c",
    brightBlue: "#5aa5ff",
    brightMagenta: "#c489ea",
    brightCyan: "#3fc0ca",
    brightWhite: dark ? "#ffffff" : "#1d1d1f",
  };
}

watch(theme, () => {
  const next = terminalTheme();
  for (const { term } of instances.values()) term.options.theme = next;
});

// Font changes apply to sessions that are already open, then re-measure: the
// cell grid depends on glyph size, so the PTY needs the new rows and columns.
watch([fontFamily, fontSize], () => {
  const family = terminalFontStack(fontFamily.value);
  for (const [id, inst] of instances) {
    inst.term.options.fontFamily = family;
    inst.term.options.fontSize = fontSize.value;
    resize(id);
  }
});

async function ensureListeners() {
  if (listening) return;
  listening = true;
  await listen<{ id: string; chunk: string }>("pty://data", ({ payload }) => {
    instances.get(payload.id)?.term.write(payload.chunk);
    if (payload.id !== activeId.value) {
      const session = sessions.value.find((s) => s.id === payload.id);
      if (session) session.unread = true;
    }
  });
  await listen<{ id: string }>("pty://exit", ({ payload }) => {
    const session = sessions.value.find((s) => s.id === payload.id);
    if (session) session.alive = false;
    instances.get(payload.id)?.term.write("\r\n\x1b[2m— session ended —\x1b[0m\r\n");
  });
}

function createInstance(id: string): Instance {
  const term = new Terminal({
    fontFamily: terminalFontStack(fontFamily.value),
    fontSize: fontSize.value,
    lineHeight: 1.35,
    cursorBlink: true,
    allowProposedApi: true,
    scrollback: 10000,
    macOptionIsMeta: true,
    theme: terminalTheme(),
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new WebLinksAddon());

  const host = document.createElement("div");
  host.style.height = "100%";
  // xterm measures on open, so it must be in the document with a real size.
  host.style.width = "100%";
  term.open(host);

  term.onData((data) => {
    void api.ptyWrite(id, data).catch(() => {
      /* the session ended; the exit banner already says so */
    });
  });

  return { term, fit, host };
}

/** Open (or focus) a session for a worktree, optionally running a command. */
async function open(opts: { id: string; label: string; cwd: string; command?: string }) {
  await ensureListeners();
  dockOpen.value = true;

  const existing = sessions.value.find((s) => s.id === opts.id);
  if (existing?.alive) {
    activeId.value = opts.id;
    existing.unread = false;
    return;
  }

  if (!instances.has(opts.id)) {
    instances.set(opts.id, createInstance(opts.id));
  }
  const inst = instances.get(opts.id)!;

  if (!existing) {
    sessions.value = [...sessions.value, { ...opts, alive: true, unread: false }];
  } else {
    existing.alive = true;
    existing.unread = false;
  }
  activeId.value = opts.id;

  // Give the dock a frame to lay out before measuring rows and columns.
  await new Promise((r) => requestAnimationFrame(() => r(null)));
  try {
    inst.fit.fit();
  } catch {
    /* not attached yet; the dock fits again on mount */
  }

  try {
    await api.ptySpawn({
      id: opts.id,
      cwd: opts.cwd,
      rows: inst.term.rows || 24,
      cols: inst.term.cols || 80,
      command: opts.command ?? null,
    });
  } catch (e) {
    sessions.value = sessions.value.filter((s) => s.id !== opts.id);
    fail("Could not open a terminal", e);
  }
}

async function close(id: string) {
  await api.ptyKill(id).catch(() => undefined);
  instances.get(id)?.term.dispose();
  instances.delete(id);
  sessions.value = sessions.value.filter((s) => s.id !== id);
  if (activeId.value === id) {
    activeId.value = sessions.value.at(-1)?.id ?? null;
    if (!activeId.value) dockOpen.value = false;
  }
}

/** Attach the active session's element to the dock and size it to fit. */
function mount(container: HTMLElement | null) {
  if (!container) return;
  const id = activeId.value;
  container.replaceChildren();
  if (!id) return;
  const inst = instances.get(id);
  if (!inst) return;
  container.appendChild(inst.host);
  requestAnimationFrame(() => resize(id));
}

function resize(id: string | null = activeId.value) {
  if (!id) return;
  const inst = instances.get(id);
  if (!inst) return;
  try {
    inst.fit.fit();
    void api.ptyResize(id, inst.term.rows, inst.term.cols).catch(() => undefined);
  } catch {
    /* the dock is collapsed; nothing to measure */
  }
}

function focus(id: string | null = activeId.value) {
  if (id) instances.get(id)?.term.focus();
}

export function useTerminals() {
  return {
    /** Apply the user's terminal typeface to new and existing sessions. */
    configure(font: string | null, size: number) {
      fontFamily.value = font;
      fontSize.value = size;
    },
    sessions,
    activeId,
    dockOpen,
    activeSession: computed(() => sessions.value.find((s) => s.id === activeId.value) ?? null),
    statusOf: (id: string) => sessions.value.find((s) => s.id === id) ?? null,
    open,
    close,
    mount,
    resize,
    focus,
    select(id: string) {
      activeId.value = id;
      const session = sessions.value.find((s) => s.id === id);
      if (session) session.unread = false;
    },
  };
}
