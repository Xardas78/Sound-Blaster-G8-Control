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

/// `amixer sset` (simple mixer) kennt nur den "einfachen" Basisnamen ohne
/// " Playback"/" Capture Volume"-Suffix (z.B. "Speaker" statt "Speaker
/// Playback Volume") plus ein Richtungswort, wenn ein Control (wie
/// "External Mic") sowohl Playback- als auch Capture-Pegel hat. Ohne diese
/// Aufteilung meldet amixer "Unable to find simple control" – lange nicht
/// bemerkt, weil set_level() das nur nach stderr geloggt hat.
fn simple_name_and_direction(full: &str) -> Option<(&str, &str)> {
    if let Some(s) = full.strip_suffix(" Capture Volume") {
        Some((s, "capture"))
    } else if let Some(s) = full.strip_suffix(" Playback Volume") {
        Some((s, "playback"))
    } else {
        None
    }
}

pub fn set_level(entity_id: i32, percent: f32) {
    let Some(&name) = usize::try_from(entity_id).ok().and_then(|i| CONTROLS.get(i)) else {
        eprintln!("Unbekannte Mixer-Entity-ID {entity_id}");
        return;
    };
    let Some((simple, direction)) = simple_name_and_direction(name) else {
        eprintln!("Mixer-Control-Name nicht erkannt: {name}");
        return;
    };
    let percent = percent.clamp(0.0, 100.0) as i32;
    run_amixer(&["sset", simple, direction, "--", &format!("{percent}%")]);
}

/// Jeder Pegel-Control aus CONTROLS hat eine gleichnamige "...Switch"-Control
/// für Stummschaltung (z.B. "Speaker Playback Volume" -> "Speaker Playback
/// Switch"), per `amixer -c USB1 controls` verifiziert. Läuft über `cset
/// name=...` (Rohcontrol-Zugriff über den exakten Namen) statt `sset`: die
/// "simple mixer"-Ansicht fasst Playback- und Capture-Switch bei manchen
/// Controls (z.B. "External Mic") zu einem gemeinsamen Eintrag zusammen,
/// bei dem sset keine zuverlässige Syntax anbietet, um gezielt nur eine
/// Richtung stummzuschalten.
pub fn set_mute(entity_id: i32, muted: bool) {
    let Some(&name) = usize::try_from(entity_id).ok().and_then(|i| CONTROLS.get(i)) else {
        eprintln!("Unbekannte Mixer-Entity-ID {entity_id}");
        return;
    };
    let switch_name = name.replace("Volume", "Switch");
    run_amixer(&["cset", &format!("name={switch_name}"), if muted { "off" } else { "on" }]);
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

/// Liest den aktuellen Prozentwert einer Mixer-Entity direkt aus ALSA.
/// Für den Lautsprecher-Regler auf dem Gerät gedacht: der physische Knopf
/// schreibt (per USB-Capture bestätigt, siehe HANDOFF.md) direkt in
/// dasselbe Feature-Unit-Register wie unser Slider – ein Poll auf diese
/// Funktion sieht also auch Änderungen, die am Gerät selbst gemacht wurden.
pub fn get_level(entity_id: i32) -> Option<f32> {
    let name = usize::try_from(entity_id).ok().and_then(|i| CONTROLS.get(i))?;
    let (simple, direction) = simple_name_and_direction(name)?;

    let out = Command::new("amixer")
        .args(["-c", CARD, "sget", simple])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // sget schreibt das Richtungswort großgeschrieben in die Wertzeile
    // ("Front Left: Playback 128 [88%] ... Capture 48 [84%] ..."),
    // simple_name_and_direction liefert die für sset nötige Kleinschreibung.
    let mut capitalized = direction.to_string();
    if let Some(first) = capitalized.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    extract_percent(&text, &capitalized)
}

/// Sucht die Wertzeile (z.B. "Front Left: Playback 128 [88%] [0.00dB] [off]
/// Capture 48 [84%] [0.00dB] [on]") und liest den Prozentwert hinter dem
/// gewünschten Richtungswort. Auf "%]" eingeschränkt, damit die vorherige
/// "Capture channels: ..."-Zeile nicht fälschlich matcht.
fn extract_percent(text: &str, direction: &str) -> Option<f32> {
    for line in text.lines() {
        if !line.contains("%]") {
            continue;
        }
        if let Some(idx) = line.find(direction) {
            let rest = &line[idx + direction.len()..];
            let start = rest.find('[')?;
            let rest = &rest[start + 1..];
            let end = rest.find('%')?;
            return rest[..end].trim().parse().ok();
        }
    }
    None
}

/// Setzt die PipeWire-Sink-Lautstärke – das ist NICHT dieselbe Stellgröße
/// wie set_level()/get_level() (ALSA-Hardwareregister der G8 selbst),
/// sondern die zusätzliche Software-Pegelstufe, die PipeWire on top of
/// jedem Sink führt (vergleichbar mit der "PC-Lautstärke" unter Windows).
/// Für "Volume Sync", siehe main.rs.
pub fn set_pipewire_sink_volume(percent: f32) {
    let percent = percent.clamp(0.0, 100.0);
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{percent}%")])
        .output();
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
