//! Persisted app settings: which repos are tracked, where new worktrees go,
//! and the user's UI preferences. Stored as JSON in the OS app-config dir.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Repository roots the user has added, most recently used first.
    pub repos: Vec<String>,
    pub active_repo: Option<String>,
    /// Default parent directory for new worktrees, used by any repository that
    /// has no entry in `repo_roots`.
    pub worktree_root: Option<String>,
    /// Per-repository override, keyed by repository root path.
    pub repo_roots: std::collections::HashMap<String, String>,
    pub preferred_editor: Option<String>,
    /// Terminal typeface. `None` uses the system monospace stack.
    pub terminal_font: Option<String>,
    pub terminal_font_size: u8,
    /// Whether we have written our hook into the user's Claude settings.
    pub claude_hooks_installed: bool,
    /// "system" | "light" | "dark"
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            active_repo: None,
            worktree_root: None,
            repo_roots: std::collections::HashMap::new(),
            preferred_editor: None,
            terminal_font: None,
            terminal_font_size: 12,
            claude_hooks_installed: false,
            theme: "system".into(),
        }
    }
}

pub struct Store {
    path: PathBuf,
    cache: Mutex<Settings>,
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let path = dir.join("settings.json");
        let cache = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            cache: Mutex::new(cache),
        }
    }

    pub fn get(&self) -> Settings {
        self.cache.lock().clone()
    }

    pub fn set(&self, next: Settings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?;
        // Write to a sibling then rename, so a crash mid-write cannot leave a
        // truncated settings file behind.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())?;
        *self.cache.lock() = next;
        Ok(())
    }
}
