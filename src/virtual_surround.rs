// Optionaler virtueller Kopfhörer-Surround auf Basis von PipeWires
// eingebautem filter-chain-Modul (libpipewire-module-filter-chain) mit
// einer HRIR-Faltung im HeSuVi-14-Kanal-Format. Die HRIR-Datei ist das
// SBX-100%-Profil aus https://github.com/eadwu/HeSuVi-HRIRs (aus der
// originalen Creative-HeSuVi-Software extrahiert), die Filter-Graph-
// Struktur ist PipeWires eigenes Beispiel
// (/usr/share/pipewire/filter-chain/sink-virtual-surround-7.1-hesuvi.conf).
//
// Wird über den Kopfhörer/Lautsprecher-Schalter auf der Acoustic-Engine-
// Seite gesteuert: Kopfhörer erzeugt den virtuellen Surround-Sink und
// setzt ihn als Standardausgabe, Lautsprecher entfernt ihn wieder und
// schaltet zurück auf die G8-Hardware direkt. PipeWire braucht für eine
// neue/entfernte context.modules-Konfiguration einen Neustart der
// Session – das passiert hier im Hintergrund-Thread, damit die UI nicht
// für die ein bis zwei Sekunden einfriert.

use crate::proc_util::run_with_timeout;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

// Verhindert, dass ein Doppelklick oder schnelles Hin-und-Herschalten
// mehrere Neustarts gleichzeitig anstößt: systemd zählt Neustarts pro
// Zeitfenster und sperrt den Dienst nach zu vielen ("start-limit-hit") –
// das ist uns beim Testen selbst passiert (mehrere Enable/Disable-Zyklen
// kurz hintereinander) und hätte im echten Betrieb bei hektischem
// Klicken die komplette Audioausgabe lahmgelegt.
static OPERATION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct OperationGuard;
impl Drop for OperationGuard {
    fn drop(&mut self) {
        OPERATION_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

const HRIR_WAV: &[u8] = include_bytes!("../assets/hrir/sbx100.wav");
const CONF_TEMPLATE: &str = include_str!("../assets/hrir/filter-chain-template.conf");
// wpctl status listet den Filter-Chain-Sink unter "Filters:" mit seinem
// internen node.name (nicht der node.description "Virtual Surround Sink"),
// die G8-Hardware dagegen ganz normal unter "Sinks:".
const VIRTUAL_SINK_SECTION: &str = "Filters:";
const VIRTUAL_SINK_MATCH: &str = "effect_input.virtual-surround";
const HARDWARE_SINK_SECTION: &str = "Sinks:";
const HARDWARE_SINK_MATCH: &str = "Sound Blaster G8";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME nicht gesetzt"))
}

fn hrir_path() -> PathBuf {
    home().join(".config/pipewire/filter-chain/hrir_hesuvi/hrir.wav")
}

fn conf_path() -> PathBuf {
    home().join(".config/pipewire/pipewire.conf.d/99-g8-virtual-surround.conf")
}

fn restart_pipewire() {
    // reset-failed zuerst: falls ein vorheriger schneller Wechsel den
    // systemd-Start-Limit-Zähler schon ausgereizt hat, würde der
    // eigentliche restart sonst mit "start-limit-hit" scheitern, ohne
    // dass der Dienst je neu startet.
    for unit in ["wireplumber", "pipewire-pulse", "pipewire"] {
        let _ = Command::new("systemctl")
            .args(["--user", "reset-failed", unit])
            .status();
    }
    // Reihenfolge wichtig: wireplumber und pipewire-pulse hängen an
    // pipewire.service, zuerst die abhängigen Dienste beenden vermeidet
    // kurzzeitige Fehlermeldungen der Klienten beim Neustart.
    for unit in ["wireplumber", "pipewire-pulse", "pipewire"] {
        let _ = Command::new("systemctl")
            .args(["--user", "restart", unit])
            .status();
    }
}

// `wpctl status` gliedert den Audio-Block in Unterabschnitte
// ("Devices:", "Sinks:", "Sources:", "Filters:", "Streams:"). Wir suchen
// nur innerhalb des angegebenen Abschnitts nach `name_fragment`, sonst
// könnte z.B. "Sound Blaster G8" auch im Devices- oder Sources-Abschnitt
// matchen.
fn find_node_id(section: &str, name_fragment: &str) -> Option<String> {
    const SECTIONS: [&str; 5] = ["Devices:", "Sinks:", "Sources:", "Filters:", "Streams:"];
    let out = run_with_timeout("wpctl", &["status"], Duration::from_secs(3))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut in_section = false;
    for line in text.lines() {
        if let Some(hit) = SECTIONS.iter().find(|s| line.contains(**s)) {
            in_section = *hit == section;
            continue;
        }
        if in_section && line.contains(name_fragment) {
            let cleaned = line.trim_start_matches(|c: char| !c.is_ascii_digit());
            if let Some(dot) = cleaned.find('.') {
                let id = &cleaned[..dot];
                if !id.is_empty() {
                    return Some(id.to_string());
                }
            }
        }
    }
    None
}

fn set_default_sink(section: &str, name_fragment: &str) {
    // Nach einem PipeWire-Neustart braucht der Sink etwas, bis er wieder
    // im Graph auftaucht.
    for _ in 0..20 {
        if let Some(id) = find_node_id(section, name_fragment) {
            run_with_timeout("wpctl", &["set-default", &id], Duration::from_secs(3));
            return;
        }
        thread::sleep(Duration::from_millis(300));
    }
    eprintln!("virtual_surround: '{name_fragment}' nach Neustart nicht gefunden");
}

pub fn enable() {
    if OPERATION_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        eprintln!("virtual_surround: Umschaltung läuft bereits, ignoriere weiteren Klick");
        return;
    }
    thread::spawn(|| {
        let _guard = OperationGuard;
        let hrir = hrir_path();
        if let Some(parent) = hrir.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&hrir, HRIR_WAV) {
            eprintln!("virtual_surround: HRIR-Datei konnte nicht geschrieben werden: {e}");
            return;
        }

        let conf = CONF_TEMPLATE.replace("hrir_hesuvi/hrir.wav", &hrir.to_string_lossy());
        let conf_file = conf_path();
        if let Some(parent) = conf_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&conf_file, conf) {
            eprintln!("virtual_surround: Filter-Chain-Konfiguration konnte nicht geschrieben werden: {e}");
            return;
        }

        restart_pipewire();
        set_default_sink(VIRTUAL_SINK_SECTION, VIRTUAL_SINK_MATCH);
    });
}

pub fn disable() {
    if OPERATION_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        eprintln!("virtual_surround: Umschaltung läuft bereits, ignoriere weiteren Klick");
        return;
    }
    thread::spawn(|| {
        let _guard = OperationGuard;
        let _ = fs::remove_file(conf_path());
        restart_pipewire();
        set_default_sink(HARDWARE_SINK_SECTION, HARDWARE_SINK_MATCH);
    });
}
