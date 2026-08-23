//! Optionaler Hintergrunddienst für Volume Sync (Lautstärkeknopf↔PipeWire-
//! Kopplung), unabhängig vom GUI-Prozess: läuft als systemd-Nutzerdienst
//! weiter, auch wenn die App komplett beendet wird (nicht nur in den
//! Tray minimiert) – dafür reicht "In den Tray minimieren" nicht, wenn
//! man wirklich gar keinen GUI-Prozess/kein Tray-Icon offen haben will.
//!
//! Derselbe Programmpfad (`g8-control --daemon`) läuft dafür OHNE Fenster
//! und OHNE Tray-Icon, nur die Poll-Schleife aus main.rs – siehe
//! `run()`. Start/Stop läuft über systemd (`systemctl --user`), nicht
//! über einen selbst verwalteten Kindprozess: systemd übernimmt
//! Neustart bei Absturz, sauberes Stoppen und Status-Abfragen, das
//! bräuchten wir sonst alles selbst nachbauen.

use crate::mixer;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

const SERVICE_NAME: &str = "g8-control-volume-daemon.service";

fn systemd_user_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").expect("HOME nicht gesetzt")).join(".config")
        });
    base.join("systemd").join("user")
}

fn service_path() -> PathBuf {
    systemd_user_dir().join(SERVICE_NAME)
}

fn exec_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "g8-control".to_string())
}

pub fn is_enabled() -> bool {
    service_path().is_file()
}

pub fn set_enabled(enabled: bool) {
    let path = service_path();
    if !enabled {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", SERVICE_NAME])
            .status();
        if let Err(e) = fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Hintergrunddienst-Datei konnte nicht entfernt werden ({}): {e}", path.display());
            }
        }
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        return;
    }

    let content = format!(
        "[Unit]\nDescription=Sound Blaster G8 Control - Lautstaerke-Hintergrunddienst\n\n[Service]\nExecStart={} --daemon\nRestart=on-failure\nRestartSec=2\n\n[Install]\nWantedBy=default.target\n",
        exec_path()
    );
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Systemd-Nutzerverzeichnis konnte nicht angelegt werden: {e}");
            return;
        }
    }
    if let Err(e) = fs::write(&path, content) {
        eprintln!("Hintergrunddienst-Datei konnte nicht angelegt werden ({}): {e}", path.display());
        return;
    }
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
    if let Err(e) = Command::new("systemctl")
        .args(["--user", "enable", "--now", SERVICE_NAME])
        .status()
    {
        eprintln!("Hintergrunddienst konnte nicht gestartet werden: {e}");
    }
}

/// Die eigentliche Dauerschleife für `g8-control --daemon`: identisch zur
/// Volume-Sync-Kopplung im GUI-Prozess (siehe main.rs), aber ohne
/// Fenster/Tray-Icon und ohne HID-Gerätezugriff (Volume Sync ist reine
/// ALSA/PipeWire-Kopplung, kein 5a-Protokoll nötig).
pub fn run() -> ! {
    let mut last: Option<f32> = None;
    loop {
        thread::sleep(Duration::from_millis(120));
        if let Some(pct) = mixer::get_level(0) {
            let changed = last.map(|prev| (prev - pct).abs() > 0.5).unwrap_or(true);
            if changed {
                last = Some(pct);
                mixer::set_pipewire_sink_volume_percent(pct);
            }
        }
    }
}
