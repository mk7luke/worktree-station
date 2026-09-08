//! Handing a directory off to the rest of the desktop: file manager, terminal,
//! editor. Each platform gets its own implementation behind one API.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn detached(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

fn spawn(program: &str, args: &[&str]) -> Result<(), String> {
    detached(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not launch {program}: {e}"))
}

/// First directory on PATH containing `exe`.
fn which(exe: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if cfg!(windows) {
        vec![format!("{exe}.exe"), format!("{exe}.cmd"), exe.to_owned()]
    } else {
        vec![exe.to_owned()]
    };
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        for name in &candidates {
            let full = dir.join(name);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Show the directory selected inside its parent, rather than opening it.
pub fn reveal(path: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Err(format!("{path} no longer exists"));
    }
    #[cfg(target_os = "macos")]
    return spawn("open", &["-R", path]);
    #[cfg(target_os = "windows")]
    return spawn(
        "explorer",
        &[&format!("/select,{}", path.replace('/', "\\"))],
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let parent = Path::new(path).parent().unwrap_or(Path::new(path));
        return spawn("xdg-open", &[&parent.to_string_lossy()]);
    }
}

/// Open the system terminal application at `path`.
pub fn open_terminal(path: &str) -> Result<(), String> {
    if !Path::new(path).is_dir() {
        return Err(format!("{path} is not a directory"));
    }

    #[cfg(target_os = "macos")]
    {
        // Respect the terminal the user actually uses, if it is installed.
        for app in ["Ghostty", "iTerm", "WezTerm", "Alacritty", "kitty"] {
            if Path::new(&format!("/Applications/{app}.app")).exists()
                && spawn("open", &["-a", app, path]).is_ok()
            {
                return Ok(());
            }
        }
        spawn("open", &["-a", "Terminal", path])
    }

    #[cfg(target_os = "windows")]
    {
        let win_path = path.replace('/', "\\");
        // Windows Terminal if available, otherwise a bare PowerShell window.
        if which("wt").is_some() {
            return spawn("wt", &["-d", &win_path]);
        }
        return detached("cmd")
            .args([
                "/C",
                "start",
                "powershell",
                "-NoExit",
                "-Command",
                "Set-Location",
                "-LiteralPath",
                &win_path,
            ])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open a terminal: {e}"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for (exe, args) in [
            ("gnome-terminal", vec!["--working-directory"]),
            ("konsole", vec!["--workdir"]),
            ("xfce4-terminal", vec!["--working-directory"]),
            ("alacritty", vec!["--working-directory"]),
            ("kitty", vec!["--directory"]),
        ] {
            if which(exe).is_some() {
                let mut full: Vec<&str> = args;
                full.push(path);
                if spawn(exe, &full).is_ok() {
                    return Ok(());
                }
            }
        }
        if which("x-terminal-emulator").is_some() {
            return spawn("x-terminal-emulator", &[]);
        }
        return Err("No terminal emulator found".into());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Editor {
    pub id: String,
    pub name: String,
}

/// Editors we know how to launch, in the order we prefer them.
const EDITORS: &[(&str, &str, &str)] = &[
    // (id, display name, macOS .app bundle name)
    ("code", "VS Code", "Visual Studio Code"),
    ("cursor", "Cursor", "Cursor"),
    ("windsurf", "Windsurf", "Windsurf"),
    ("zed", "Zed", "Zed"),
    ("subl", "Sublime Text", "Sublime Text"),
    ("idea", "IntelliJ IDEA", "IntelliJ IDEA"),
    ("webstorm", "WebStorm", "WebStorm"),
];

/// Which of the known editors are actually installed on this machine.
pub fn detect_editors() -> Vec<Editor> {
    EDITORS
        .iter()
        .filter(|(cli, _, app)| which(cli).is_some() || mac_app_installed(app))
        .map(|(id, name, _)| Editor {
            id: (*id).to_owned(),
            name: (*name).to_owned(),
        })
        .collect()
}

fn mac_app_installed(_app: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        Path::new(&format!("/Applications/{_app}.app")).exists()
            || dirs::home_dir()
                .map(|h| h.join(format!("Applications/{_app}.app")).exists())
                .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    false
}

pub fn open_in_editor(editor_id: &str, path: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Err(format!("{path} no longer exists"));
    }
    let entry = EDITORS
        .iter()
        .find(|(id, _, _)| *id == editor_id)
        .ok_or_else(|| format!("Unknown editor '{editor_id}'"))?;

    // The CLI shim is the reliable route: it reuses an existing window.
    if which(entry.0).is_some() {
        return spawn(entry.0, &[path]);
    }
    // Otherwise fall back to launching the bundle by name (macOS only).
    #[cfg(target_os = "macos")]
    if mac_app_installed(entry.2) {
        return spawn("open", &["-a", entry.2, path]);
    }
    Err(format!("{} is not installed", entry.1))
}
