//! Mixer-Pegel der G8 sind Standard-UAC2-Feature-Units. Der Kernel-Treiber
//! (snd-usb-audio) exponiert sie bereits als ALSA-Mixer-Controls auf Karte
//! "USB1" – kein 5a-Report, kein eigener USB-Zugriff nötig (siehe
//! HANDOFF.md, "Standard-USB-Audio-Class"). Namen per `amixer -c USB1
//! controls` verifiziert.

use std::process::Command;

const CARD: &str = "USB1";

/// Index = entity-id aus ui/tabs/mixer-tab.slint.
const CONTROLS: [&str; 8] = [
    "Speaker Playback Volume",
    "Line In Playback Volume",
    "External Mic Playback Volume",
    "S/PDIF In Playback Volume",
    "S/PDIF In Capture Volume",
    "Line In Capture Volume",
    "External Mic Capture Volume",
    "What U Hear Capture Volume",
];

pub fn set_level(entity_id: i32, percent: f32) {
    let Some(&name) = usize::try_from(entity_id).ok().and_then(|i| CONTROLS.get(i)) else {
        eprintln!("Unbekannte Mixer-Entity-ID {entity_id}");
        return;
    };
    let percent = percent.clamp(0.0, 100.0) as i32;
    run_amixer(&["sset", name, "--", &format!("{percent}%")]);
}

/// Index = Zeilenindex im "Aufnahme"-Tab (ui/tabs/recording-settings-tab.slint).
/// Entspricht der ALSA-Enum-Control "PCM Capture Source" (per `amixer -c USB1
/// sget 'PCM Capture Source'` verifiziert), die exakt den "Standard
/// festlegen"-Button im Original abbildet.
const CAPTURE_SOURCES: [&str; 4] = ["S/PDIF In", "Line In", "External Mic", "What U Hear"];

pub fn set_capture_source(index: i32) {
    let Some(&name) = usize::try_from(index).ok().and_then(|i| CAPTURE_SOURCES.get(i)) else {
        eprintln!("Unbekannter Aufnahmequellen-Index {index}");
        return;
    };
    run_amixer(&["sset", "PCM Capture Source", name]);
}

fn run_amixer(args: &[&str]) {
    let mut full_args = vec!["-c", CARD];
    full_args.extend_from_slice(args);
    let result = Command::new("amixer").args(&full_args).output();
    match result {
        Ok(out) if !out.status.success() => {
            eprintln!("amixer fehlgeschlagen ({args:?}): {}", String::from_utf8_lossy(&out.stderr));
        }
        Err(e) => eprintln!("amixer nicht aufrufbar ({args:?}): {e}"),
        _ => {}
    }
}
