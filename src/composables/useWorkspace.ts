/**
 * The app's shared state: which repositories are tracked, the worktrees inside
 * the active one, their git status, and what any agent running there is doing.
 */
import { computed, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { api } from "@/lib/api";
import type {
  AgentStatus,
  Editor,
  RepoInfo,
  Settings,
  Worktree,
  WorktreeStatus,
} from "@/lib/types";
import { useToasts } from "./useToasts";
import { useTheme } from "./useTheme";

export interface WorktreeView extends Worktree {
  /** Resolved path, used to match hook callbacks and terminal sessions. */
  canonical: string;
  status: WorktreeStatus | null;
  agent: AgentStatus | null;
}

const defaults: Settings = {
  repos: [],
  activeRepo: null,
  worktreeRoot: null,
  repoRoots: {},
  preferredEditor: null,
  terminalFont: null,
  terminalFontSize: 12,
  claudeHooksInstalled: false,
  theme: "system",
};

const settings = ref<Settings>({ ...defaults });
const repos = ref<RepoInfo[]>([]);
const activeRepoPath = ref<string | null>(null);
const worktrees = ref<Worktree[]>([]);
const canonicals = ref<Record<string, string>>({});
const statuses = ref<Record<string, WorktreeStatus>>({});
const agents = ref<Record<string, AgentStatus>>({});
const editors = ref<Editor[]>([]);
const homeDir = ref<string | null>(null);
const loadingRepo = ref(false);
const ready = ref(false);

const { fail } = useToasts();
const { theme, setTheme } = useTheme();

const activeRepo = computed(() => repos.value.find((r) => r.path === activeRepoPath.value) ?? null);

const views = computed<WorktreeView[]>(() =>
  worktrees.value.map((wt) => {
    const canonical = canonicals.value[wt.path] ?? wt.path;
    return {
      ...wt,
      canonical,
      status: statuses.value[wt.path] ?? null,
      agent: agents.value[canonical] ?? null,
    };
  }),
);

/** Worktrees needing attention sort first; the main checkout always sorts last. */
const sortedViews = computed(() => {
  const rank = (v: WorktreeView) =>
    v.agent === "waiting" ? 0 : v.agent === "working" ? 1 : v.isMain ? 3 : 2;
  return [...views.value].sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
});

const waitingCount = computed(() => views.value.filter((v) => v.agent === "waiting").length);

/** Where this repository's worktrees go: its own setting, else the default. */
const activeWorktreeRoot = computed(() => {
  const repo = activeRepoPath.value;
  if (repo && settings.value.repoRoots[repo]) return settings.value.repoRoots[repo]!;
  return settings.value.worktreeRoot;
});

/** Point one repository at its own folder; `null` restores the default. */
async function setRepoRoot(repo: string, root: string | null) {
  const next = { ...settings.value.repoRoots };
  if (root) next[repo] = root;
  else delete next[repo];
  await persist({ repoRoots: next });
}

async function persist(patch: Partial<Settings>) {
  const next = { ...settings.value, ...patch };
  settings.value = next;
  try {
    await api.settingsSet(next);
  } catch (e) {
    fail("Could not save your settings", e);
  }
}

async function loadRepoInfo(path: string): Promise<RepoInfo | null> {
  try {
    return await api.repoResolve(path);
  } catch {
    return null;
  }
}

/** Re-read the worktree list, then fill in per-worktree status in the background. */
async function refreshWorktrees(quiet = false) {
  const repo = activeRepoPath.value;
  if (!repo) {
    worktrees.value = [];
    return;
  }
  if (!quiet) loadingRepo.value = true;
  try {
    const list = await api.worktreesList(repo);
    worktrees.value = list;
    void hydrate(list);
  } catch (e) {
    fail("Could not read worktrees", e);
    worktrees.value = [];
  } finally {
    loadingRepo.value = false;
  }
}

/** Status and canonical path for each worktree, fetched in parallel. */
async function hydrate(list: Worktree[]) {
  await Promise.all(
    list.map(async (wt) => {
      const [canonical, status] = await Promise.all([
        api.canonicalize(wt.path).catch(() => wt.path),
        api.worktreeStatus(wt.path).catch(() => null),
      ]);
      canonicals.value = { ...canonicals.value, [wt.path]: canonical };
      if (status) statuses.value = { ...statuses.value, [wt.path]: status };
    }),
  );
}

async function selectRepo(path: string | null) {
  activeRepoPath.value = path;
  statuses.value = {};
  await persist({ activeRepo: path });
  await refreshWorktrees();
}

async function addRepo(path: string): Promise<RepoInfo | null> {
  const info = await loadRepoInfo(path);
  if (!info) {
    fail("Not a git repository", `${path} has no repository at or above it.`);
    return null;
  }
  if (!repos.value.some((r) => r.path === info.path)) {
    repos.value = [info, ...repos.value];
  }
  await persist({ repos: repos.value.map((r) => r.path) });
  await selectRepo(info.path);
  return info;
}

async function removeRepo(path: string) {
  repos.value = repos.value.filter((r) => r.path !== path);
  const nextActive = activeRepoPath.value === path ? (repos.value[0]?.path ?? null) : activeRepoPath.value;
  await persist({ repos: repos.value.map((r) => r.path) });
  await selectRepo(nextActive);
}

async function init() {
  settings.value = { ...defaults, ...(await api.settingsGet().catch(() => defaults)) };
  setTheme(settings.value.theme);

  const resolved = await Promise.all(settings.value.repos.map(loadRepoInfo));
  repos.value = resolved.filter((r): r is RepoInfo => r !== null);

  // Repositories that have since been deleted quietly drop off the list.
  if (repos.value.length !== settings.value.repos.length) {
    await persist({ repos: repos.value.map((r) => r.path) });
  }

  editors.value = await api.editorsList().catch(() => []);
  agents.value = normalizeAgents(await api.claudeStatuses().catch(() => ({})));

  const wanted = settings.value.activeRepo;
  activeRepoPath.value = repos.value.some((r) => r.path === wanted) ? wanted : (repos.value[0]?.path ?? null);
  if (activeRepoPath.value) await refreshWorktrees();

  await listen<{ path: string; status: string }>("claude://status", (event) => {
    const { path, status } = event.payload;
    const next = { ...agents.value };
    if (status === "ended") delete next[path];
    else if (isAgentStatus(status)) next[path] = status;
    agents.value = next;
  });

  // Someone may have created or removed a worktree from the command line.
  window.addEventListener("focus", () => void refreshWorktrees(true));

  ready.value = true;
}

function isAgentStatus(value: string): value is AgentStatus {
  return value === "working" || value === "waiting" || value === "idle";
}

function normalizeAgents(raw: Record<string, string>): Record<string, AgentStatus> {
  const out: Record<string, AgentStatus> = {};
  for (const [path, status] of Object.entries(raw)) {
    if (isAgentStatus(status)) out[path] = status;
  }
  return out;
}

export function useWorkspace() {
  return {
    ready,
    settings,
    persist,
    theme,
    setTheme: (next: Settings["theme"]) => {
      setTheme(next);
      void persist({ theme: next });
    },
    repos,
    activeRepo,
    activeRepoPath,
    editors,
    homeDir,
    worktrees: sortedViews,
    activeWorktreeRoot,
    setRepoRoot,
    waitingCount,
    loadingRepo,
    init,
    addRepo,
    removeRepo,
    selectRepo,
    refreshWorktrees,
  };
}
