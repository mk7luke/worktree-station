//! "Smart sync": share git-ignored artifacts (node_modules, .env, target, …)
//! between the main checkout and a fresh worktree instead of reinstalling them.
//!
//! Candidates come from `git ls-files --others --ignored --exclude-standard
//! --directory`, so the full gitignore grammar — negations, nested .gitignore
//! files, globs — is honoured by git itself rather than re-implemented here.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use crate::git;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTarget {
    /// Path relative to the repository root, e.g. `apps/web/node_modules`.
    pub rel_path: String,
    pub is_dir: bool,
    /// Whether we pre-select this one in the UI.
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkReport {
    pub linked: Vec<String>,
    pub copied: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<String>,
    /// Windows only: links that need one elevation prompt to finish.
    pub needs_elevation: usize,
}

/// Names worth sharing by default: expensive to rebuild, safe to share.
const RECOMMENDED: &[&str] = &[
    "node_modules",
    "vendor",
    "target",
    ".venv",
    "venv",
    ".env",
    ".env.local",
    ".direnv",
    "Pods",
    ".gradle",
    ".tox",
];

/// Things that must never be shared: sharing them corrupts the new worktree.
const NEVER: &[&str] = &[".git", ".DS_Store", "Thumbs.db"];

fn is_recommended(rel: &str) -> bool {
    let base = rel.trim_end_matches('/').rsplit('/').next().unwrap_or(rel);
    RECOMMENDED.contains(&base)
}

/// Everything git considers ignored in `repo_root`, collapsed at directory level.
pub fn candidates(repo_root: &str) -> Result<Vec<LinkTarget>, String> {
    let out = git::run(
        Path::new(repo_root),
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
    )?;

    let mut targets = Vec::new();
    for line in out.lines() {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }
        // `--directory` marks collapsed directories with a trailing slash.
        let is_dir = raw.ends_with('/');
        let rel = raw.trim_end_matches('/');
        if rel.is_empty() {
            continue;
        }
        let base = rel.rsplit('/').next().unwrap_or(rel);
        if NEVER.contains(&base) {
            continue;
        }
        targets.push(LinkTarget {
            rel_path: rel.to_owned(),
            is_dir,
            recommended: is_recommended(rel),
        });
    }
    // Recommended first, then alphabetical — the useful ones surface at the top.
    targets.sort_by(|a, b| {
        b.recommended
            .cmp(&a.recommended)
            .then_with(|| a.rel_path.cmp(&b.rel_path))
    });
    Ok(targets)
}

/// Reject anything that escapes the repository root. `rel` arrives from the
/// frontend, so it is untrusted even though we produced the original list.
fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let candidate = Path::new(rel);
    // `is_absolute` alone is not enough on Windows, where a path only counts
    // as absolute with a drive or UNC prefix — "/windows/system32" is merely
    // rooted. Reject rooted and prefixed paths too, so the guard means the
    // same thing on every platform.
    let has_prefix = matches!(candidate.components().next(), Some(Component::Prefix(_)));
    if rel.is_empty() || candidate.is_absolute() || candidate.has_root() || has_prefix {
        return None;
    }
    let mut out = root.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => return None,
            p => out.push(p),
        }
    }
    Some(out)
}

/// Link each selected target from the source checkout into the new worktree.
pub fn apply(repo_root: &str, worktree_path: &str, rels: &[String]) -> LinkReport {
    let root = Path::new(repo_root);
    let dest_root = Path::new(worktree_path);
    let mut report = LinkReport {
        linked: Vec::new(),
        copied: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
        needs_elevation: 0,
    };
    #[allow(unused_mut)]
    let mut deferred: Vec<(PathBuf, PathBuf)> = Vec::new();

    for rel in rels {
        let (src, dest) = match (safe_join(root, rel), safe_join(dest_root, rel)) {
            (Some(s), Some(d)) => (s, d),
            _ => {
                report.failed.push(format!("{rel}: unsafe path"));
                continue;
            }
        };
        if !src.exists() {
            report.skipped.push(rel.clone());
            continue;
        }
        // symlink_metadata: an existing symlink is a real conflict even if it dangles.
        if dest.symlink_metadata().is_ok() {
            report.skipped.push(rel.clone());
            continue;
        }
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                report.failed.push(format!("{rel}: {e}"));
                continue;
            }
        }
        match link_one(&src, &dest) {
            Outcome::Linked => report.linked.push(rel.clone()),
            Outcome::Copied => report.copied.push(rel.clone()),
            Outcome::NeedsElevation => {
                deferred.push((src, dest));
                report.needs_elevation += 1;
            }
            Outcome::Failed(e) => report.failed.push(format!("{rel}: {e}")),
        }
    }

    #[cfg(target_os = "windows")]
    if !deferred.is_empty() {
        match windows::elevate_batch(&deferred) {
            Ok(()) => {
                for (_, dest) in &deferred {
                    report.linked.push(dest.to_string_lossy().into_owned());
                }
            }
            Err(e) => report.failed.push(format!("elevated link batch: {e}")),
        }
    }

    let placed: Vec<String> = report
        .linked
        .iter()
        .chain(report.copied.iter())
        .cloned()
        .collect();
    if let Err(e) = record_excludes(root, &placed) {
        report.failed.push(format!("recording local excludes: {e}"));
    }

    report
}

const BLOCK_START: &str = "# --- worktree-station: shared worktree links (safe to delete) ---";
const BLOCK_END: &str = "# --- end worktree-station ---";

/// Keep shared links out of `git status`.
///
/// A gitignore pattern ending in `/` matches directories only, and git sees a
/// symlink as a file — so a shared `node_modules/` reappears as untracked in
/// every new worktree, ready to be committed by accident. Anchored entries in
/// the repository's local exclude file fix that. That file is never committed,
/// and these paths are already ignored wherever they are real directories, so
/// nothing else changes.
fn record_excludes(repo_root: &Path, rels: &[String]) -> Result<(), String> {
    if rels.is_empty() {
        return Ok(());
    }
    let exclude_path = git::run(
        repo_root,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "info/exclude",
        ],
    )?;
    let exclude_path = PathBuf::from(exclude_path.trim());
    if let Some(parent) = exclude_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let existing = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    let (preserved, previous) = split_managed_block(&existing);

    // Union with whatever earlier worktrees recorded, so removing one worktree
    // never un-ignores a link another one still relies on.
    let mut entries: Vec<String> = previous;
    for rel in rels {
        let anchored = format!("/{}", rel.trim_start_matches('/'));
        if !entries.contains(&anchored) {
            entries.push(anchored);
        }
    }
    entries.sort();

    let mut out = preserved.trim_end().to_owned();
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(BLOCK_START);
    out.push('\n');
    for entry in &entries {
        out.push_str(entry);
        out.push('\n');
    }
    out.push_str(BLOCK_END);
    out.push('\n');

    std::fs::write(&exclude_path, out).map_err(|e| e.to_string())
}

/// Split an exclude file into everything we do not manage, and the entries
/// inside our marked block.
fn split_managed_block(content: &str) -> (String, Vec<String>) {
    let mut preserved = String::new();
    let mut entries = Vec::new();
    let mut inside = false;
    for line in content.lines() {
        if line.trim() == BLOCK_START {
            inside = true;
        } else if line.trim() == BLOCK_END {
            inside = false;
        } else if inside {
            let entry = line.trim();
            if !entry.is_empty() && !entry.starts_with('#') {
                entries.push(entry.to_owned());
            }
        } else {
            preserved.push_str(line);
            preserved.push('\n');
        }
    }
    (preserved, entries)
}

/// Windows exercises every arm; Unix only ever links.
#[allow(dead_code)]
enum Outcome {
    Linked,
    Copied,
    NeedsElevation,
    Failed(String),
}

#[cfg(unix)]
fn link_one(src: &Path, dest: &Path) -> Outcome {
    // On Unix a symlink covers both files and directories and needs no privileges.
    match std::os::unix::fs::symlink(src, dest) {
        Ok(()) => Outcome::Linked,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(windows)]
fn link_one(src: &Path, dest: &Path) -> Outcome {
    if src.is_dir() {
        // A junction works for unprivileged users; a directory symlink usually
        // does not unless Developer Mode is on.
        if windows::try_junction(src, dest) {
            Outcome::Linked
        } else {
            Outcome::NeedsElevation
        }
    } else {
        // Files: symlink, then hard link, then give up and copy.
        if std::os::windows::fs::symlink_file(src, dest).is_ok()
            || std::fs::hard_link(src, dest).is_ok()
        {
            Outcome::Linked
        } else {
            match std::fs::copy(src, dest) {
                Ok(_) => Outcome::Copied,
                Err(e) => Outcome::Failed(e.to_string()),
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn mklink_args(src: &Path, dest: &Path) -> String {
        format!(
            "mklink /J \"{}\" \"{}\"",
            dest.display().to_string().replace('/', "\\"),
            src.display().to_string().replace('/', "\\")
        )
    }

    pub fn try_junction(src: &Path, dest: &Path) -> bool {
        Command::new("cmd")
            .args(["/C", &mklink_args(src, dest)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Collect every junction that needs privileges into one script so the user
    /// sees a single UAC prompt rather than one per directory.
    pub fn elevate_batch(pairs: &[(std::path::PathBuf, std::path::PathBuf)]) -> Result<(), String> {
        // The BOM tells PowerShell the file is UTF-8, which keeps non-ASCII
        // paths intact.
        let mut script = String::from("\u{FEFF}$ErrorActionPreference = 'Continue'\r\n");
        for (src, dest) in pairs {
            script.push_str(&format!("cmd /c '{}'\r\n", mklink_args(src, dest)));
        }
        let script_path = std::env::temp_dir().join("gwm-links.ps1");
        std::fs::write(&script_path, script).map_err(|e| e.to_string())?;

        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &format!(
                    "Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait \
                     -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-WindowStyle','Hidden','-File','\"{}\"'",
                    script_path.display()
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;

        let _ = std::fs::remove_file(&script_path);
        if status.success() {
            Ok(())
        } else {
            Err("elevation was declined or failed".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_ordinary_relative_paths() {
        let root = Path::new("/repo");
        assert_eq!(
            safe_join(root, "apps/web/node_modules"),
            Some(PathBuf::from("/repo/apps/web/node_modules"))
        );
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let root = Path::new("/repo");
        assert_eq!(safe_join(root, "../outside"), None);
        assert_eq!(safe_join(root, "apps/../../outside"), None);
        assert_eq!(safe_join(root, "/etc/passwd"), None);
        assert_eq!(safe_join(root, ""), None);
        // On Unix these are legal, if odd, relative file names — only Windows
        // treats them as rooted or drive-prefixed.
        #[cfg(windows)]
        {
            assert_eq!(safe_join(root, "\\windows\\system32"), None);
            assert_eq!(safe_join(root, "C:\\Windows"), None);
        }
    }

    #[test]
    fn recommends_known_expensive_directories() {
        assert!(is_recommended("node_modules"));
        assert!(is_recommended("apps/web/node_modules"));
        assert!(is_recommended(".env"));
        assert!(!is_recommended("dist"));
        assert!(!is_recommended("coverage"));
    }
}
