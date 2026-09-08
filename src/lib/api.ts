/**
 * Typed wrappers over the Rust command surface.
 *
 * Tauri converts between Rust snake_case and JS camelCase for command
 * arguments, but struct fields cross the bridge verbatim — so the Rust structs
 * are declared with camelCase serde renaming and the types here match 1:1.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  BranchRef,
  CreateResult,
  Editor,
  LinkTarget,
  RepoInfo,
  Settings,
  Worktree,
  WorktreeStatus,
} from "./types";

export const api = {
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<void>("settings_set", { settings }),

  repoResolve: (path: string) => invoke<RepoInfo>("repo_resolve", { path }),
  worktreesList: (repoRoot: string) => invoke<Worktree[]>("worktrees_list", { repoRoot }),
  worktreeStatus: (worktreePath: string) =>
    invoke<WorktreeStatus>("worktree_status", { worktreePath }),
  branchesList: (repoRoot: string) => invoke<BranchRef[]>("branches_list", { repoRoot }),

  linkCandidates: (repoRoot: string) => invoke<LinkTarget[]>("link_candidates", { repoRoot }),
  suggestWorktreePath: (worktreeRoot: string, repoRoot: string, branch: string) =>
    invoke<string>("suggest_worktree_path", { worktreeRoot, repoRoot, branch }),
  worktreeCreate: (request: {
    repoRoot: string;
    path: string;
    branch: string;
    base: string | null;
    useExistingBranch: boolean;
    trackRemote: boolean;
    linkTargets: string[];
  }) => invoke<CreateResult>("worktree_create", { request }),
  worktreePrune: (repoRoot: string) => invoke<string>("worktree_prune", { repoRoot }),
  worktreeRemove: (request: {
    repoRoot: string;
    worktreePath: string;
    force: boolean;
    deleteBranch: boolean;
    deleteRemoteBranch: boolean;
  }) => invoke<void>("worktree_remove", { request }),

  revealInFileManager: (path: string) => invoke<void>("reveal_in_file_manager", { path }),
  openTerminal: (path: string) => invoke<void>("open_terminal", { path }),
  editorsList: () => invoke<Editor[]>("editors_list"),
  monospaceFonts: () => invoke<string[]>("monospace_fonts"),
  openInEditor: (editorId: string, path: string) =>
    invoke<void>("open_in_editor", { editorId, path }),

  ptySpawn: (request: {
    id: string;
    cwd: string;
    rows: number;
    cols: number;
    command: string | null;
  }) => invoke<void>("pty_spawn", { request }),
  ptyWrite: (id: string, data: string) => invoke<void>("pty_write", { id, data }),
  ptyResize: (id: string, rows: number, cols: number) =>
    invoke<void>("pty_resize", { id, rows, cols }),
  ptyKill: (id: string) => invoke<void>("pty_kill", { id }),

  claudeHooksInstalled: () => invoke<boolean>("claude_hooks_installed"),
  claudeInstallHooks: () => invoke<string>("claude_install_hooks"),
  claudeUninstallHooks: () => invoke<void>("claude_uninstall_hooks"),
  claudeStatuses: () => invoke<Record<string, string>>("claude_statuses"),
  canonicalize: (path: string) => invoke<string>("canonicalize", { path }),
};
