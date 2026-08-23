//! Autostart über den XDG-Standardmechanismus: eine .desktop-Datei in
//! ~/.config/autostart/ – wird von KDE Plasma (und den meisten anderen
//! Desktops) automatisch beim Anmelden gestartet. Kein systemd-Unit
//! nötig, keine Root-Rechte, rein im Nutzerverzeichnis.

use std::fs;
use std::path::PathBuf;

fn autostart_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").expect("HOME nicht gesetzt")).join(".config")
        });
    base.join("autostart")
}

fn desktop_file_path() -> PathBuf {
    autostart_dir().join("g8-control.desktop")
}

/// Pfad auf die eigene Programmdatei, für den Exec-Eintrag – funktioniert
/// sowohl für die per install.sh installierte Version (~/.local/bin) als
/// auch für einen laufenden Entwicklungs-Build (target/debug|release).
fn exec_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "g8-control".to_string())
}

pub fn is_enabled() -> bool {
    desktop_file_path().is_file()
}

pub fn set_enabled(enabled: bool) {
    let path = desktop_file_path();
    if !enabled {
        if let Err(e) = fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Autostart-Eintrag konnte nicht entfernt werden ({}): {e}", path.display());
            }
        }
        return;
    }
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Sound Blaster G8 Control\nExec={}\nIcon=g8-control\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec_path()
    );
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Autostart-Verzeichnis konnte nicht angelegt werden: {e}");
            return;
        }
    }
    if let Err(e) = fs::write(&path, content) {
        eprintln!("Autostart-Eintrag konnte nicht angelegt werden ({}): {e}", path.display());
    }
}
