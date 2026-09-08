export interface RepoInfo {
  path: string;
  name: string;
  headBranch: string | null;
  defaultBranch: string;
}

export interface Worktree {
  path: string;
  name: string;
  head: string;
  branch: string | null;
  isMain: boolean;
  isBare: boolean;
  isDetached: boolean;
  locked: string | null;
  prunable: string | null;
}

export interface WorktreeStatus {
  upstream: string | null;
  ahead: number;
  behind: number;
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  subject: string;
  author: string;
  relativeDate: string;
}

export interface BranchRef {
  name: string;
  isRemote: boolean;
  isCheckedOut: boolean;
  subject: string;
  relativeDate: string;
}

export interface LinkTarget {
  relPath: string;
  isDir: boolean;
  recommended: boolean;
}

export interface LinkReport {
  linked: string[];
  copied: string[];
  skipped: string[];
  failed: string[];
  needsElevation: number;
}

export interface CreateResult {
  path: string;
  branch: string;
  linkReport: LinkReport | null;
}

export interface Editor {
  id: string;
  name: string;
}

export interface Settings {
  repos: string[];
  activeRepo: string | null;
  /** Default for repositories with no override. */
  worktreeRoot: string | null;
  /** Per-repository override, keyed by repository root path. */
  repoRoots: Record<string, string>;
  preferredEditor: string | null;
  /** Terminal typeface; null means the system monospace stack. */
  terminalFont: string | null;
  terminalFontSize: number;
  claudeHooksInstalled: boolean;
  theme: "system" | "light" | "dark";
}

/** What an agent in a worktree is doing right now. */
export type AgentStatus = "working" | "waiting" | "idle";
