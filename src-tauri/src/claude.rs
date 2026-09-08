//! Claude Code awareness.
//!
//! A small loopback server receives hook callbacks so worktree cards can show
//! whether Claude is working, waiting for approval, or idle. Installing the
//! hook edits the user's global Claude settings, so it is opt-in, merges into
//! whatever is already configured, and can be removed again cleanly.

use axum::{extract::State as AxumState, routing::post, Json, Router};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

pub const HOOK_PORT: u16 = 36911;
/// Present in every command we write, so we can find and remove exactly our
/// own entries without disturbing hooks the user configured themselves.
const HOOK_MARKER: &str = "worktree-station-hook";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEvent {
    pub path: String,
    /// "working" | "waiting" | "idle" | "ended"
    pub status: String,
}

#[derive(Default)]
pub struct ClaudeState {
    /// Canonical path -> latest status.
    statuses: Mutex<HashMap<String, String>>,
}

impl ClaudeState {
    pub fn snapshot(&self) -> HashMap<String, String> {
        self.statuses.lock().clone()
    }
}

/// Compare paths by their resolved form; symlinked worktree roots are common.
pub fn canonical(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.trim_end_matches(['/', '\\']).to_owned())
}

struct ServerState {
    app: AppHandle,
    claude: Arc<ClaudeState>,
}

async fn on_status(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(event): Json<StatusEvent>,
) -> &'static str {
    let path = canonical(&event.path);
    let normalized = StatusEvent {
        path: path.clone(),
        status: event.status.clone(),
    };

    if event.status == "ended" {
        state.claude.statuses.lock().remove(&path);
    } else {
        state
            .claude
            .statuses
            .lock()
            .insert(path.clone(), event.status.clone());
    }

    let _ = state.app.emit("claude://status", &normalized);

    if event.status == "waiting" {
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        let _ = state
            .app
            .notification()
            .builder()
            .title("Claude needs approval")
            .body(format!("{name} is waiting for your response."))
            .show();
    }
    "ok"
}

/// Bind the loopback listener. Failure is non-fatal: the app still works, it
/// just cannot show live Claude status.
pub async fn serve(app: AppHandle, claude: Arc<ClaudeState>) {
    let state = Arc::new(ServerState { app, claude });
    let router = Router::new()
        .route("/claude/status", post(on_status))
        .with_state(state);

    match tokio::net::TcpListener::bind(("127.0.0.1", HOOK_PORT)).await {
        Ok(listener) => {
            log::info!("Claude hook listener on 127.0.0.1:{HOOK_PORT}");
            let _ = axum::serve(listener, router).await;
        }
        Err(e) => log::warn!("Claude hook listener unavailable on {HOOK_PORT}: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Hook installation
// ---------------------------------------------------------------------------

/// Claude Code hook events we subscribe to, and the status each implies.
const EVENTS: &[(&str, &str)] = &[
    ("SessionStart", "idle"),
    ("UserPromptSubmit", "working"),
    ("PreToolUse", "working"),
    ("PostToolUse", "working"),
    ("Notification", "waiting"),
    ("Stop", "idle"),
    ("SessionEnd", "ended"),
];

fn claude_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .ok_or_else(|| "Could not locate your home directory".to_owned())
}

fn script_path() -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "worktree-station-hook.ps1"
    } else {
        "worktree-station-hook.sh"
    };
    Ok(claude_dir()?.join("hooks").join(name))
}

#[cfg(not(windows))]
fn write_script(path: &Path) -> Result<(), String> {
    // POSIX sh, no dependencies beyond curl. Always exits 0 so a stopped app
    // can never block a Claude session.
    let body = format!(
        r#"#!/bin/sh
# {HOOK_MARKER} — reports Claude Code activity to Worktree Station.
# Safe to delete; the app can reinstall it from Settings.
STATUS="$1"
DIR="${{CLAUDE_PROJECT_DIR:-$PWD}}"
curl -s -m 2 -o /dev/null \
  -X POST "http://127.0.0.1:{HOOK_PORT}/claude/status" \
  -H 'Content-Type: application/json' \
  --data-binary "$(printf '{{"path":"%s","status":"%s"}}' "$DIR" "$STATUS")" \
  >/dev/null 2>&1 || true
exit 0
"#
    );
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn write_script(path: &Path) -> Result<(), String> {
    let body = format!(
        r#"# {HOOK_MARKER} — reports Claude Code activity to Worktree Station.
param([string]$Status)
$dir = if ($env:CLAUDE_PROJECT_DIR) {{ $env:CLAUDE_PROJECT_DIR }} else {{ (Get-Location).Path }}
$payload = @{{ path = $dir; status = $Status }} | ConvertTo-Json -Compress
try {{
  Invoke-RestMethod -Uri "http://127.0.0.1:{HOOK_PORT}/claude/status" -Method Post `
    -Body ([System.Text.Encoding]::UTF8.GetBytes($payload)) `
    -ContentType 'application/json; charset=utf-8' -TimeoutSec 2 -ErrorAction SilentlyContinue | Out-Null
}} catch {{ }}
exit 0
"#
    );
    // The BOM keeps non-ASCII paths readable to Windows PowerShell.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(body.as_bytes());
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

fn hook_command(script: &Path, event: &str) -> String {
    #[cfg(windows)]
    {
        format!(
            "powershell -NoProfile -ExecutionPolicy Bypass -File \"{}\" {}",
            script.display(),
            event
        )
    }
    #[cfg(not(windows))]
    {
        format!("\"{}\" {}", script.display(), event)
    }
}

fn read_settings(path: &Path) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn write_settings(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.gwm-tmp");
    std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Drop every hook entry we previously wrote, leaving the user's own untouched.
fn strip_our_entries(hooks: &mut serde_json::Value) {
    let Some(map) = hooks.as_object_mut() else {
        return;
    };
    for (_event, entries) in map.iter_mut() {
        if let Some(list) = entries.as_array_mut() {
            list.retain(|entry| {
                !serde_json::to_string(entry)
                    .unwrap_or_default()
                    .contains(HOOK_MARKER)
            });
        }
    }
    map.retain(|_, entries| entries.as_array().map(|l| !l.is_empty()).unwrap_or(true));
}

pub fn install_hooks() -> Result<String, String> {
    let dir = claude_dir()?;
    std::fs::create_dir_all(dir.join("hooks")).map_err(|e| e.to_string())?;
    let script = script_path()?;
    write_script(&script)?;

    let settings_path = dir.join("settings.json");
    let mut settings = read_settings(&settings_path);
    if !settings.is_object() {
        return Err("~/.claude/settings.json is not a JSON object; not touching it".into());
    }
    if settings
        .get("hooks")
        .map(|h| !h.is_object())
        .unwrap_or(false)
    {
        return Err(
            "~/.claude/settings.json has an unexpected \"hooks\" value; not touching it".into(),
        );
    }
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    let hooks = &mut settings["hooks"];
    // Re-installing should replace our entries, not stack duplicates.
    strip_our_entries(hooks);

    for (event, _) in EVENTS {
        let entry = serde_json::json!({
            "hooks": [{ "type": "command", "command": hook_command(&script, event) }]
        });
        match hooks.get_mut(*event).and_then(|v| v.as_array_mut()) {
            Some(list) => list.push(entry),
            None => hooks[*event] = serde_json::json!([entry]),
        }
    }

    write_settings(&settings_path, &settings)?;
    Ok(settings_path.to_string_lossy().into_owned())
}

pub fn uninstall_hooks() -> Result<(), String> {
    let settings_path = claude_dir()?.join("settings.json");
    if settings_path.exists() {
        let mut settings = read_settings(&settings_path);
        if let Some(hooks) = settings.get_mut("hooks") {
            strip_our_entries(hooks);
        }
        write_settings(&settings_path, &settings)?;
    }
    if let Ok(script) = script_path() {
        let _ = std::fs::remove_file(script);
    }
    Ok(())
}

/// Whether our hook entries are currently present in the user's settings.
pub fn hooks_installed() -> bool {
    let Ok(dir) = claude_dir() else { return false };
    std::fs::read_to_string(dir.join("settings.json"))
        .map(|s| s.contains(HOOK_MARKER))
        .unwrap_or(false)
}
