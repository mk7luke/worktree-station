//! Creating and removing worktrees, including the destructive paths.
//! Every function that deletes verifies first that the target is genuinely a
//! worktree registered to the given repository.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::git;
use crate::linker::{self, LinkReport};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub repo_root: String,
    pub path: String,
    pub branch: String,
    /// Start point: a branch, tag or SHA. `None` means "current HEAD".
    pub base: Option<String>,
    /// Check out an existing branch instead of creating a new one.
    pub use_existing_branch: bool,
    /// Also set upstream tracking when `base` is a remote branch.
    pub track_remote: bool,
    /// Gitignored paths (repo-relative) to share with the new worktree.
    pub link_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResult {
    pub path: String,
    pub branch: String,
    pub link_report: Option<LinkReport>,
}

/// Turn a branch name into a filesystem-safe directory name.
/// `feature/user login` -> `feature-user-login`
pub fn slugify_branch(branch: &str) -> String {
    let mut out = String::with_capacity(branch.len());
    let mut last_dash = false;
    for c in branch.chars() {
        let mapped = match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' => Some(c),
            _ => None,
        };
        match mapped {
            Some(c) => {
                out.push(c);
                last_dash = false;
            }
            None => {
                if !last_dash && !out.is_empty() {
                    out.push('-');
                    last_dash = true;
                }
            }
        }
    }
    let trimmed = out.trim_matches(['-', '.']).to_owned();
    if trimmed.is_empty() {
        "worktree".to_owned()
    } else {
        trimmed
    }
}

/// The directory a new worktree will occupy: `<root>/<repo>-<branch-slug>`.
pub fn suggest_path(worktree_root: &str, repo_root: &str, branch: &str) -> String {
    let repo_name = Path::new(repo_root)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_owned());
    PathBuf::from(worktree_root)
        .join(format!("{repo_name}-{}", slugify_branch(branch)))
        .to_string_lossy()
        .into_owned()
}

pub fn create(req: CreateRequest) -> Result<CreateResult, String> {
    let root = Path::new(&req.repo_root);
    if !root.is_dir() {
        return Err(format!("Repository {} not found", req.repo_root));
    }
    git::validate_branch_name(&req.repo_root, &req.branch)?;

    let dest = Path::new(&req.path);
    if dest.exists() {
        return Err(format!("{} already exists", req.path));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }

    let mut args: Vec<String> = vec!["worktree".into(), "add".into()];
    let local_exists = git::branch_exists(&req.repo_root, &req.branch);

    if req.use_existing_branch {
        if local_exists {
            // Plain checkout of a branch that already exists locally.
            args.push(req.path.clone());
            args.push(req.branch.clone());
        } else {
            // The branch only exists on a remote: create the local branch here,
            // pointed at (and, by git's default, tracking) the remote ref.
            let base = req
                .base
                .as_ref()
                .filter(|b| !b.is_empty())
                .ok_or_else(|| format!("Branch '{}' does not exist locally", req.branch))?;
            args.push("-b".into());
            args.push(req.branch.clone());
            args.push(req.path.clone());
            args.push(base.clone());
        }
    } else {
        if local_exists {
            return Err(format!(
                "Branch '{}' already exists. Check it out instead of creating it.",
                req.branch
            ));
        }
        args.push("-b".into());
        args.push(req.branch.clone());
        if req.track_remote {
            args.push("--track".into());
        }
        args.push(req.path.clone());
        if let Some(base) = req.base.as_ref().filter(|b| !b.is_empty()) {
            args.push(base.clone());
        }
    }

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    git::run(root, &arg_refs)?;

    let link_report = if req.link_targets.is_empty() {
        None
    } else {
        Some(linker::apply(&req.repo_root, &req.path, &req.link_targets))
    };

    Ok(CreateResult {
        path: req.path,
        branch: req.branch,
        link_report,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRequest {
    pub repo_root: String,
    pub worktree_path: String,
    /// Discard uncommitted changes.
    pub force: bool,
    /// Delete the local branch the worktree had checked out.
    pub delete_branch: bool,
    /// Also delete that branch on its remote. Affects everyone, so it is always
    /// an explicit, separate choice.
    pub delete_remote_branch: bool,
}

/// Confirm `candidate` is a worktree git has registered for this repo, and is
/// not the main worktree. Returns the branch it had checked out, if any.
fn verify_removable(repo_root: &str, candidate: &str) -> Result<Option<String>, String> {
    let worktrees = git::list_worktrees(repo_root)?;
    let target = std::fs::canonicalize(candidate)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| candidate.to_owned());

    for wt in worktrees {
        let known = std::fs::canonicalize(&wt.path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| wt.path.clone());
        if known == target || wt.path == candidate {
            if wt.is_main {
                return Err("Refusing to remove the main worktree".into());
            }
            return Ok(wt.branch);
        }
    }
    Err(format!(
        "{candidate} is not a worktree of this repository — refusing to delete it"
    ))
}

pub fn remove(req: RemoveRequest) -> Result<(), String> {
    let root = Path::new(&req.repo_root);
    let branch = verify_removable(&req.repo_root, &req.worktree_path)?;

    let mut args = vec!["worktree", "remove"];
    if req.force {
        args.push("--force");
    }
    args.push(&req.worktree_path);

    // Without --force git refuses on a dirty tree; that error is surfaced
    // verbatim so the UI can offer to retry rather than silently destroy work.
    git::run(root, &args)?;

    // Junctions and symlinks we created can leave the directory behind. It is
    // safe to remove now: git has already accepted the removal, and
    // `remove_dir_all` does not follow links.
    let dir = Path::new(&req.worktree_path);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
    let _ = git::run(root, &["worktree", "prune"]);

    let Some(branch) = branch.filter(|b| !b.is_empty()) else {
        return Ok(());
    };

    // Resolve the upstream before deleting the local branch — the tracking
    // configuration goes away with it.
    let upstream = if req.delete_remote_branch {
        git::upstream_of(&req.repo_root, &branch)
    } else {
        None
    };

    if req.delete_branch {
        let force_flag = if req.force { "-D" } else { "-d" };
        git::run(root, &["branch", force_flag, &branch])
            .map_err(|e| format!("Worktree removed, but branch '{branch}' was kept: {e}"))?;
    }

    if req.delete_remote_branch {
        let (remote, remote_branch) = upstream.ok_or_else(|| {
            format!(
                "'{branch}' does not track a remote branch, so there was nothing to delete there"
            )
        })?;
        git::run(root, &["push", &remote, "--delete", &remote_branch]).map_err(|e| {
            format!("Removed locally, but {remote}/{remote_branch} is still on the remote: {e}")
        })?;
    }

    Ok(())
}

/// Clear stale administrative entries for worktrees whose directories are gone.
pub fn prune(repo_root: &str) -> Result<String, String> {
    git::run(Path::new(repo_root), &["worktree", "prune", "--verbose"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_makes_path_safe_names() {
        assert_eq!(slugify_branch("feature/login"), "feature-login");
        assert_eq!(slugify_branch("fix/JIRA-123_thing"), "fix-JIRA-123_thing");
        assert_eq!(slugify_branch("release/v1.2.0"), "release-v1.2.0");
        // Runs of separators collapse, and edges are trimmed.
        assert_eq!(slugify_branch("a//b  c"), "a-b-c");
        assert_eq!(slugify_branch("///"), "worktree");
        assert_eq!(slugify_branch(""), "worktree");
    }

    #[test]
    fn slugify_strips_traversal_characters() {
        // A branch name can legally contain dots; the slug must not become "..".
        assert_eq!(slugify_branch(".."), "worktree");
        assert_eq!(slugify_branch("../../etc"), "etc");
    }

    #[test]
    fn suggested_path_combines_root_repo_and_branch() {
        let p = suggest_path(
            "/Users/dev/worktrees",
            "/Users/dev/code/proj",
            "feature/login",
        );
        // Compare as paths, not strings: joining produces a backslash on
        // Windows, so a literal forward-slash string would not match there.
        assert_eq!(
            Path::new(&p),
            Path::new("/Users/dev/worktrees").join("proj-feature-login")
        );
    }
}
