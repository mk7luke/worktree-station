//! Embedded terminal sessions.
//!
//! Each session owns a real PTY, so the app knows first-hand whether a shell
//! (or a Claude Code run inside it) is alive — no PID chasing, no polling the
//! process table, and it behaves the same on macOS, Linux and Windows.

use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct PtyOutput {
    pub id: String,
    pub chunk: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PtyExit {
    pub id: String,
    pub code: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnRequest {
    pub id: String,
    pub cwd: String,
    pub rows: u16,
    pub cols: u16,
    /// Typed into the shell once it starts, e.g. `claude`.
    pub command: Option<String>,
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, Session>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }


}

/// The interactive shell to run, as a login shell so the user's PATH (nvm,
/// volta, homebrew, …) is present — Claude Code is usually installed there.
fn shell_command(cwd: &str) -> CommandBuilder {
    #[cfg(windows)]
    let mut cmd = {
        // Prefer PowerShell 7 when present, else Windows PowerShell.
        let exe = if which("pwsh.exe").is_some() { "pwsh.exe" } else { "powershell.exe" };
        let mut c = CommandBuilder::new(exe);
        c.args(["-NoLogo", "-NoExit"]);
        c
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut c = CommandBuilder::new(shell);
        c.arg("-l");
        c
    };

    cmd.cwd(cwd);
    // xterm.js speaks xterm-256color; announcing anything else garbles colour.
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd
}

#[cfg(windows)]
fn which(exe: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(exe))
            .find(|p| p.is_file())
    })
}

pub fn spawn(app: AppHandle, mgr: Arc<PtyManager>, req: SpawnRequest) -> Result<(), String> {
    if mgr.sessions.lock().contains_key(&req.id) {
        return Err(format!("Session {} is already running", req.id));
    }
    if !std::path::Path::new(&req.cwd).is_dir() {
        return Err(format!("{} is not a directory", req.cwd));
    }

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: req.rows.max(1),
            cols: req.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Could not open a terminal: {e}"))?;

    let child = pair
        .slave
        .spawn_command(shell_command(&req.cwd))
        .map_err(|e| format!("Could not start the shell: {e}"))?;
    // Release our handle on the slave side, otherwise the reader never sees EOF
    // when the shell exits.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Could not read from the terminal: {e}"))?;
    let mut writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Could not write to the terminal: {e}"))?;

    if let Some(cmd) = req.command.as_ref().filter(|c| !c.trim().is_empty()) {
        // The tty buffers this until the shell is ready to read it.
        let _ = writer.write_all(format!("{cmd}\n").as_bytes());
        let _ = writer.flush();
    }

    mgr.sessions.lock().insert(
        req.id.clone(),
        Session {
            master: pair.master,
            writer,
            child,
        },
    );

    let id = req.id.clone();
    let mgr_for_reader = Arc::clone(&mgr);
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        // A read can land mid-codepoint; hold the incomplete tail until the
        // rest arrives rather than emitting replacement characters.
        let mut carry: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    carry.extend_from_slice(&buf[..n]);
                    let valid = match std::str::from_utf8(&carry) {
                        Ok(_) => carry.len(),
                        Err(e) => e.valid_up_to(),
                    };
                    if valid > 0 {
                        let chunk = String::from_utf8_lossy(&carry[..valid]).into_owned();
                        carry.drain(..valid);
                        let _ = app.emit(
                            "pty://data",
                            PtyOutput {
                                id: id.clone(),
                                chunk,
                            },
                        );
                    }
                    // Guard against a stream that is genuinely not UTF-8.
                    if carry.len() > 8 {
                        carry.clear();
                    }
                }
            }
        }

        let code = mgr_for_reader
            .sessions
            .lock()
            .get_mut(&id)
            .and_then(|s| s.child.wait().ok())
            .map(|st| st.exit_code());
        mgr_for_reader.sessions.lock().remove(&id);
        let _ = app.emit("pty://exit", PtyExit { id, code });
    });

    Ok(())
}

pub fn write(mgr: &PtyManager, id: &str, data: &str) -> Result<(), String> {
    let mut sessions = mgr.sessions.lock();
    let session = sessions
        .get_mut(id)
        .ok_or_else(|| format!("No terminal session {id}"))?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())
}

pub fn resize(mgr: &PtyManager, id: &str, rows: u16, cols: u16) -> Result<(), String> {
    let sessions = mgr.sessions.lock();
    let session = sessions
        .get(id)
        .ok_or_else(|| format!("No terminal session {id}"))?;
    session
        .master
        .resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

pub fn kill(mgr: &PtyManager, id: &str) -> Result<(), String> {
    let mut sessions = mgr.sessions.lock();
    if let Some(session) = sessions.get_mut(id) {
        let _ = session.child.kill();
    }
    // The reader thread removes the entry once the PTY closes; dropping it here
    // too would race, so leave removal to that thread.
    Ok(())
}

/// Kill every session — used on window close so no orphan shells survive.
pub fn kill_all(mgr: &PtyManager) {
    let mut sessions = mgr.sessions.lock();
    for session in sessions.values_mut() {
        let _ = session.child.kill();
    }
}
