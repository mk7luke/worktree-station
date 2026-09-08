//! Git plumbing. Every call shells out to `git` with an explicit working
//! directory; nothing here assumes a particular platform.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// Build a `git` command that never flashes a console window on Windows.
pub fn git(cwd: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        // Keep output machine-readable regardless of the user's config.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Run git and return stdout, or the trimmed stderr as an error.
pub fn run(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = git(cwd)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        Err(if err.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            err
        })
    }
}

/// Run git and report only whether it succeeded, discarding all output.
fn ok(cwd: &Path, args: &[&str]) -> bool {
    git(cwd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    pub name: String,
    pub head: String,
    pub branch: Option<String>,
    pub is_main: bool,
    pub is_bare: bool,
    pub is_detached: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStatus {
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub subject: String,
    pub author: String,
    pub relative_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub head_branch: Option<String>,
    pub default_branch: String,
}

/// Resolve any path inside a repository to its main worktree root.
pub fn resolve_repo_root(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Err(format!("{path} is not a directory"));
    }
    // `--path-format=absolute` keeps this correct when git is invoked from a
    // subdirectory, and `--git-common-dir` points at the *main* .git even when
    // `path` is itself a linked worktree.
    let common = run(
        p,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = common.trim();
    if common.is_empty() {
        return Err(format!("{path} is not inside a git repository"));
    }
    let git_dir = Path::new(common);
    // A bare repo's common dir *is* the repo root; otherwise strip the `.git`.
    let root = if git_dir.file_name().map(|n| n == ".git").unwrap_or(false) {
        git_dir.parent().unwrap_or(git_dir)
    } else {
        git_dir
    };
    Ok(root.to_string_lossy().into_owned())
}

pub fn repo_info(root: &str) -> Result<RepoInfo, String> {
    let p = Path::new(root);
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_owned());
    let head_branch = run(p, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    Ok(RepoInfo {
        path: root.to_owned(),
        name,
        head_branch,
        default_branch: default_branch(p),
    })
}

/// Best-effort guess at the repository's integration branch.
pub fn default_branch(cwd: &Path) -> String {
    // Prefer what the remote actually says.
    if let Ok(s) = run(
        cwd,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) {
        if let Some(b) = s.trim().strip_prefix("origin/") {
            if !b.is_empty() {
                return b.to_owned();
            }
        }
    }
    for candidate in ["main", "master", "develop", "trunk"] {
        if ok(
            cwd,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{candidate}"),
            ],
        ) {
            return candidate.to_owned();
        }
    }
    run(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "main".to_owned())
}

pub fn list_worktrees(root: &str) -> Result<Vec<Worktree>, String> {
    let out = run(Path::new(root), &["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_list(&out))
}

/// Parse `git worktree list --porcelain`. Records are blank-line separated and
/// the first record is always the main worktree.
pub fn parse_worktree_list(out: &str) -> Vec<Worktree> {
    let mut result = Vec::new();
    let mut cur: Option<Worktree> = None;

    let flush = |cur: &mut Option<Worktree>, result: &mut Vec<Worktree>| {
        if let Some(mut wt) = cur.take() {
            wt.is_main = result.is_empty();
            result.push(wt);
        }
    };

    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            flush(&mut cur, &mut result);
            continue;
        }
        let (key, value) = match line.split_once(' ') {
            Some((k, v)) => (k, v),
            None => (line, ""),
        };
        match key {
            "worktree" => {
                flush(&mut cur, &mut result);
                let name = Path::new(value)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| value.to_owned());
                cur = Some(Worktree {
                    path: value.to_owned(),
                    name,
                    head: String::new(),
                    branch: None,
                    is_main: false,
                    is_bare: false,
                    is_detached: false,
                    locked: None,
                    prunable: None,
                });
            }
            "HEAD" => {
                if let Some(w) = cur.as_mut() {
                    w.head = value.to_owned();
                }
            }
            "branch" => {
                if let Some(w) = cur.as_mut() {
                    w.branch = Some(
                        value
                            .strip_prefix("refs/heads/")
                            .unwrap_or(value)
                            .to_owned(),
                    );
                }
            }
            // The flag-style keys carry an optional reason after the keyword.
            "bare" => {
                if let Some(w) = cur.as_mut() {
                    w.is_bare = true;
                }
            }
            "detached" => {
                if let Some(w) = cur.as_mut() {
                    w.is_detached = true;
                }
            }
            "locked" => {
                if let Some(w) = cur.as_mut() {
                    w.locked = Some(value.to_owned());
                }
            }
            "prunable" => {
                if let Some(w) = cur.as_mut() {
                    w.prunable = Some(value.to_owned());
                }
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut result);
    result
}

/// Working-tree state for one worktree: divergence, dirty counts, last commit.
pub fn status(worktree_path: &str) -> Result<WorktreeStatus, String> {
    let p = Path::new(worktree_path);
    if !p.is_dir() {
        return Err(format!("{worktree_path} no longer exists"));
    }
    let mut st = WorktreeStatus::default();

    let porcelain = run(
        p,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
        ],
    )?;
    parse_status_v2(&porcelain, &mut st);

    // `%s` subject, `%an` author, `%cr` committer date relative — one line each
    // so a subject containing our separator can't corrupt the later fields.
    if let Ok(log) = run(p, &["log", "-1", "--format=%s%n%an%n%cr"]) {
        let mut lines = log.lines();
        st.subject = lines.next().unwrap_or_default().to_owned();
        st.author = lines.next().unwrap_or_default().to_owned();
        st.relative_date = lines.next().unwrap_or_default().to_owned();
    }
    Ok(st)
}

/// Parse `git status --porcelain=v2 --branch` into counts.
pub fn parse_status_v2(out: &str, st: &mut WorktreeStatus) {
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            st.upstream = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            // Format: "+<ahead> -<behind>"
            for tok in rest.split_whitespace() {
                if let Some(n) = tok.strip_prefix('+') {
                    st.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = tok.strip_prefix('-') {
                    st.behind = n.parse().unwrap_or(0);
                }
            }
        } else if line.starts_with("? ") {
            st.untracked += 1;
        } else if line.starts_with("u ") {
            st.conflicted += 1;
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            // Field 2 is the two-character XY code: X staged, Y unstaged,
            // '.' meaning unchanged in that half.
            if let Some(xy) = line.split_whitespace().nth(1) {
                let mut chars = xy.chars();
                if chars.next().map(|c| c != '.').unwrap_or(false) {
                    st.staged += 1;
                }
                if chars.next().map(|c| c != '.').unwrap_or(false) {
                    st.unstaged += 1;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRef {
    pub name: String,
    pub is_remote: bool,
    pub is_checked_out: bool,
    pub subject: String,
    pub relative_date: String,
}

/// Local and remote branches, most recently committed first.
pub fn list_branches(root: &str) -> Result<Vec<BranchRef>, String> {
    let p = Path::new(root);
    let out = run(
        p,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)%09%(refname)%09%(worktreepath)%09%(contents:subject)%09%(committerdate:relative)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    let mut branches = Vec::new();
    for line in out.lines() {
        let mut f = line.split('\t');
        let short = f.next().unwrap_or_default();
        let full = f.next().unwrap_or_default();
        let worktreepath = f.next().unwrap_or_default();
        let subject = f.next().unwrap_or_default();
        let date = f.next().unwrap_or_default();
        if short.is_empty() || short.ends_with("/HEAD") {
            continue;
        }
        branches.push(BranchRef {
            name: short.to_owned(),
            is_remote: full.starts_with("refs/remotes/"),
            is_checked_out: !worktreepath.is_empty(),
            subject: subject.to_owned(),
            relative_date: date.to_owned(),
        });
    }
    Ok(branches)
}

/// True when a local branch of this name already exists.
pub fn branch_exists(root: &str, branch: &str) -> bool {
    ok(
        Path::new(root),
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
}

/// Reject names git itself would reject, before we build a path from them.
pub fn validate_branch_name(root: &str, branch: &str) -> Result<(), String> {
    if branch.trim().is_empty() {
        return Err("Branch name is required".into());
    }
    if !ok(Path::new(root), &["check-ref-format", "--branch", branch]) {
        return Err(format!("'{branch}' is not a valid branch name"));
    }
    Ok(())
}

/// The remote and branch name a local branch tracks, e.g. ("origin", "feat/login").
/// `None` when the branch has no upstream configured.
pub fn upstream_of(root: &str, branch: &str) -> Option<(String, String)> {
    let out = run(
        Path::new(root),
        &[
            "for-each-ref",
            "--format=%(upstream:remotename)%09%(upstream:lstrip=3)",
            &format!("refs/heads/{branch}"),
        ],
    )
    .ok()?;
    let line = out.lines().next()?;
    let (remote, name) = line.split_once('\t')?;
    if remote.trim().is_empty() || name.trim().is_empty() {
        return None;
    }
    Some((remote.trim().to_owned(), name.trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_main_and_linked_worktrees() {
        let out = "\
worktree /Users/dev/proj
HEAD abc123def456
branch refs/heads/main

worktree /Users/dev/wt/proj-feature
HEAD 0f0f0f0f0f0f
branch refs/heads/feature/login

worktree /Users/dev/wt/proj-detached
HEAD 1a1a1a1a1a1a
detached
";
        let wts = parse_worktree_list(out);
        assert_eq!(wts.len(), 3);

        assert!(wts[0].is_main);
        assert_eq!(wts[0].path, "/Users/dev/proj");
        assert_eq!(wts[0].branch.as_deref(), Some("main"));

        assert!(!wts[1].is_main);
        assert_eq!(wts[1].name, "proj-feature");
        // refs/heads/ is stripped even when the branch name contains a slash.
        assert_eq!(wts[1].branch.as_deref(), Some("feature/login"));

        assert!(wts[2].is_detached);
        assert_eq!(wts[2].branch, None);
    }

    #[test]
    fn parses_bare_locked_and_prunable_flags() {
        let out = "\
worktree /srv/repo.git
bare

worktree /srv/wt/one
HEAD aaa
branch refs/heads/one
locked reason for the lock
prunable gitdir file points to non-existent location
";
        let wts = parse_worktree_list(out);
        assert!(wts[0].is_bare);
        assert_eq!(wts[1].locked.as_deref(), Some("reason for the lock"));
        assert!(wts[1].prunable.is_some());
    }

    #[test]
    fn trailing_record_without_blank_line_is_kept() {
        let out = "worktree /a\nHEAD abc\nbranch refs/heads/main";
        assert_eq!(parse_worktree_list(out).len(), 1);
    }

    #[test]
    fn counts_divergence_and_dirty_files() {
        let out = "\
# branch.oid abc123
# branch.head feature
# branch.upstream origin/feature
# branch.ab +3 -2
1 M. N... 100644 100644 100644 aaa bbb staged-only.txt
1 .M N... 100644 100644 100644 aaa bbb unstaged-only.txt
1 MM N... 100644 100644 100644 aaa bbb both.txt
2 R. N... 100644 100644 100644 aaa bbb R100 new.txt\told.txt
u UU N... 100644 100644 100644 100644 aaa bbb ccc conflict.txt
? untracked.txt
? another-untracked.txt
";
        let mut st = WorktreeStatus::default();
        parse_status_v2(out, &mut st);

        assert_eq!(st.upstream.as_deref(), Some("origin/feature"));
        assert_eq!(st.ahead, 3);
        assert_eq!(st.behind, 2);
        // staged-only, both, and the rename all have a staged half
        assert_eq!(st.staged, 3);
        // unstaged-only and both have an unstaged half
        assert_eq!(st.unstaged, 2);
        assert_eq!(st.untracked, 2);
        assert_eq!(st.conflicted, 1);
    }

    #[test]
    fn clean_tree_reports_nothing() {
        let out = "# branch.oid abc\n# branch.head main\n";
        let mut st = WorktreeStatus::default();
        parse_status_v2(out, &mut st);
        assert_eq!(
            (st.ahead, st.behind, st.staged, st.unstaged, st.untracked),
            (0, 0, 0, 0, 0)
        );
        assert!(st.upstream.is_none());
    }
}
