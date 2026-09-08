//! Worktree Station — Tauri command surface and app wiring.

pub mod claude;
pub mod fonts;
pub mod git;
pub mod linker;
pub mod pty;
pub mod store;
pub mod system;
pub mod worktree;

use std::sync::Arc;
use tauri::{Manager, State};

struct AppState {
    store: store::Store,
    pty: Arc<pty::PtyManager>,
    claude: Arc<claude::ClaudeState>,
}

// --- settings ---------------------------------------------------------------

#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> store::Settings {
    state.store.get()
}

#[tauri::command]
fn settings_set(state: State<'_, AppState>, settings: store::Settings) -> Result<(), String> {
    state.store.set(settings)
}

// --- repositories -----------------------------------------------------------

#[tauri::command]
fn repo_resolve(path: String) -> Result<git::RepoInfo, String> {
    let root = git::resolve_repo_root(&path)?;
    git::repo_info(&root)
}

#[tauri::command]
fn worktrees_list(repo_root: String) -> Result<Vec<git::Worktree>, String> {
    git::list_worktrees(&repo_root)
}

#[tauri::command]
fn worktree_status(worktree_path: String) -> Result<git::WorktreeStatus, String> {
    git::status(&worktree_path)
}

#[tauri::command]
fn branches_list(repo_root: String) -> Result<Vec<git::BranchRef>, String> {
    git::list_branches(&repo_root)
}

// --- worktree lifecycle -----------------------------------------------------

#[tauri::command]
fn link_candidates(repo_root: String) -> Result<Vec<linker::LinkTarget>, String> {
    linker::candidates(&repo_root)
}

#[tauri::command]
fn suggest_worktree_path(worktree_root: String, repo_root: String, branch: String) -> String {
    worktree::suggest_path(&worktree_root, &repo_root, &branch)
}

#[tauri::command]
fn worktree_create(request: worktree::CreateRequest) -> Result<worktree::CreateResult, String> {
    worktree::create(request)
}

#[tauri::command]
fn worktree_remove(request: worktree::RemoveRequest) -> Result<(), String> {
    worktree::remove(request)
}

#[tauri::command]
fn worktree_prune(repo_root: String) -> Result<String, String> {
    worktree::prune(&repo_root)
}

// --- desktop integration ----------------------------------------------------

#[tauri::command]
fn reveal_in_file_manager(path: String) -> Result<(), String> {
    system::reveal(&path)
}

#[tauri::command]
fn open_terminal(path: String) -> Result<(), String> {
    system::open_terminal(&path)
}

/// Monospace families installed on this machine, for the terminal font picker.
#[tauri::command]
fn monospace_fonts() -> Vec<String> {
    fonts::monospace_families()
}

#[tauri::command]
fn editors_list() -> Vec<system::Editor> {
    system::detect_editors()
}

#[tauri::command]
fn open_in_editor(editor_id: String, path: String) -> Result<(), String> {
    system::open_in_editor(&editor_id, &path)
}

// --- embedded terminal ------------------------------------------------------

#[tauri::command]
fn pty_spawn(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: pty::SpawnRequest,
) -> Result<(), String> {
    pty::spawn(app, Arc::clone(&state.pty), request)
}

#[tauri::command]
fn pty_write(state: State<'_, AppState>, id: String, data: String) -> Result<(), String> {
    pty::write(&state.pty, &id, &data)
}

#[tauri::command]
fn pty_resize(state: State<'_, AppState>, id: String, rows: u16, cols: u16) -> Result<(), String> {
    pty::resize(&state.pty, &id, rows, cols)
}

#[tauri::command]
fn pty_kill(state: State<'_, AppState>, id: String) -> Result<(), String> {
    pty::kill(&state.pty, &id)
}

// --- Claude Code ------------------------------------------------------------

#[tauri::command]
fn claude_hooks_installed() -> bool {
    claude::hooks_installed()
}

#[tauri::command]
fn claude_install_hooks(state: State<'_, AppState>) -> Result<String, String> {
    let path = claude::install_hooks()?;
    let mut settings = state.store.get();
    settings.claude_hooks_installed = true;
    state.store.set(settings)?;
    Ok(path)
}

#[tauri::command]
fn claude_uninstall_hooks(state: State<'_, AppState>) -> Result<(), String> {
    claude::uninstall_hooks()?;
    let mut settings = state.store.get();
    settings.claude_hooks_installed = false;
    state.store.set(settings)
}

#[tauri::command]
fn claude_statuses(state: State<'_, AppState>) -> std::collections::HashMap<String, String> {
    state.claude.snapshot()
}

#[tauri::command]
fn canonicalize(path: String) -> String {
    claude::canonical(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let config_dir = app.path().app_config_dir()?;
            let claude_state = Arc::new(claude::ClaudeState::default());
            app.manage(AppState {
                store: store::Store::load(config_dir),
                pty: Arc::new(pty::PtyManager::new()),
                claude: Arc::clone(&claude_state),
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(claude::serve(handle, claude_state));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Shells outlive the webview unless we take them down explicitly.
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    pty::kill_all(&state.pty);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_set,
            repo_resolve,
            worktrees_list,
            worktree_status,
            branches_list,
            link_candidates,
            suggest_worktree_path,
            worktree_create,
            worktree_remove,
            worktree_prune,
            reveal_in_file_manager,
            open_terminal,
            editors_list,
            monospace_fonts,
            open_in_editor,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_kill,
            claude_hooks_installed,
            claude_install_hooks,
            claude_uninstall_hooks,
            claude_statuses,
            canonicalize,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Worktree Station");
}
