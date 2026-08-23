//! Mixer-Pegel der G8 sind Standard-UAC2-Feature-Units. Der Kernel-Treiber
//! (snd-usb-audio) exponiert sie bereits als ALSA-Mixer-Controls auf Karte
//! "USB1" – kein 5a-Report, kein eigener USB-Zugriff nötig (siehe
//! HANDOFF.md, "Standard-USB-Audio-Class"). Namen per `amixer -c USB1
//! controls` verifiziert.

use crate::proc_util::run_with_timeout;
use std::time::Duration;

const CARD: &str = "USB1";
// amixer/wpctl laufen auf dem 120ms-Poll-Thread (siehe main.rs) – ein
// Timeout deutlich unter dem Poll-Intervall verhindert, dass ein
// hängender Aufruf (z.B. bei einem PipeWire-USB-Stall, siehe HANDOFF.md)
// den Thread und damit die komplette Lautstärke-Anzeige einfriert.
const CMD_TIMEOUT: Duration = Duration::from_millis(500);

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

// Der Lautsprecher-Drehregler der G8 (entity-id 0) ist ein analoges
// Potentiometer mit "Audio-Taper"-Kennlinie: gleich große Drehschritte
// verändern den rohen ALSA-Wert (0-128) sehr ungleichmäßig – unten viel,
// oben wenig. Aus einem Windows-USB-Capture einer langsamen, kompletten
// Minimum->Maximum-Drehung extrahiert (`vol min-max-min_mit
// sync.pcapng`, siehe HANDOFF.md): 24 tatsächlich durchlaufene Rohwerte
// in der Reihenfolge, wie das Gerät sie beim Drehen meldet. Angenommen,
// die Drehung war einigermaßen gleichmäßig, entspricht der Index in
// dieser Liste (0..23) der gefühlten/linearen Position (0-100%) –
// dieselbe Kalibrierung, die auch live am Gerät nachgemessen wurde
// (optische Mitte = Rohwert 107 ≈ Tabellenindex 12 ≈ 52%, sehr nah an
// den erwarteten 50%). Die anderen Mixer-Regler (Line In, External Mic,
// SPDIF, ...) sind rein digitale ALSA-Gains ohne physisches Poti
// dahinter, deshalb gilt die Korrektur NUR für entity-id 0.
const TAPER_TABLE: [(u8, f32); 24] = [
    (8, 0.0000),
    (56, 4.3478),
    (62, 8.6957),
    (71, 13.0435),
    (78, 17.3913),
    (84, 21.7391),
    (88, 26.0870),
    (92, 30.4348),
    (96, 34.7826),
    (99, 39.1304),
    (102, 43.4783),
    (105, 47.8261),
    (107, 52.1739),
    (110, 56.5217),
    (112, 60.8696),
    (114, 65.2174),
    (116, 69.5652),
    (118, 73.9130),
    (120, 78.2609),
    (121, 82.6087),
    (123, 86.9565),
    (124, 91.3043),
    (126, 95.6522),
    (128, 100.0000),
];

/// Roher ALSA-Prozentwert -> gefühlter Prozentwert (stückweise linear
/// über TAPER_TABLE, in Rohwert-Einheiten interpoliert).
fn taper_correct(alsa_pct: f32) -> f32 {
    let raw = (alsa_pct / 100.0 * 128.0).clamp(0.0, 128.0);
    if raw <= TAPER_TABLE[0].0 as f32 {
        return 0.0;
    }
    if raw >= TAPER_TABLE[TAPER_TABLE.len() - 1].0 as f32 {
        return 100.0;
    }
    for w in TAPER_TABLE.windows(2) {
        let (r0, p0) = (w[0].0 as f32, w[0].1);
        let (r1, p1) = (w[1].0 as f32, w[1].1);
        if raw >= r0 && raw <= r1 {
            return p0 + (raw - r0) / (r1 - r0) * (p1 - p0);
        }
    }
    100.0
}

/// Umkehrfunktion zu taper_correct(): gefühlter Prozentwert -> roher
/// ALSA-Prozentwert, zum Schreiben (set_level()).
fn taper_uncorrect(display_pct: f32) -> f32 {
    let display_pct = display_pct.clamp(0.0, 100.0);
    if display_pct <= TAPER_TABLE[0].1 {
        return TAPER_TABLE[0].0 as f32 / 128.0 * 100.0;
    }
    let last = TAPER_TABLE[TAPER_TABLE.len() - 1];
    if display_pct >= last.1 {
        return last.0 as f32 / 128.0 * 100.0;
    }
    for w in TAPER_TABLE.windows(2) {
        let (r0, p0) = (w[0].0 as f32, w[0].1);
        let (r1, p1) = (w[1].0 as f32, w[1].1);
        if display_pct >= p0 && display_pct <= p1 {
            let raw = r0 + (display_pct - p0) / (p1 - p0) * (r1 - r0);
            return raw / 128.0 * 100.0;
        }
    }
    100.0
}

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
    // Taper-Korrektur nur für den Lautsprecher-Regler (entity-id 0, siehe
    // Kommentar oben bei TAPER_MIN_ALSA_PCT) – percent kommt hier als
    // "gefühlter" Wert vom Slider, amixer braucht aber den rohen Wert.
    let alsa_percent = if entity_id == 0 { taper_uncorrect(percent) } else { percent };
    let alsa_percent = alsa_percent.clamp(0.0, 100.0) as i32;
    run_amixer(&["sset", simple, direction, "--", &format!("{alsa_percent}%")]);
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

    let out = run_with_timeout("amixer", &["-c", CARD, "sget", simple], CMD_TIMEOUT)?;
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
    let alsa_pct = extract_percent(&text, &capitalized)?;
    Some(if entity_id == 0 { taper_correct(alsa_pct) } else { alsa_pct })
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

/// Setzt die PipeWire-Sink-Lautstärke direkt auf einen linearen
/// Prozentwert (kein dB/kubische Umrechnung) – für den Lautsprecher-
/// Regler (entity-id 0) gedacht, dessen get_level() bereits taper-
/// korrigiert ist (siehe TAPER_TABLE oben). Der Zweck von Volume Sync
/// ist, dass die PC-Lautstärke *gefühlt* zum Reglerstand passt – dafür
/// ist der bereits korrigierte "gefühlte" Prozentwert die richtige
/// Zielgröße, nicht der physikalisch exakte dB-Wert (der würde wieder
/// bei ~84% statt ~50% in der Mitte landen, siehe HANDOFF.md).
pub fn set_pipewire_sink_volume_percent(percent: f32) {
    let percent = percent.clamp(0.0, 100.0);
    run_with_timeout(
        "wpctl",
        &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{percent}%")],
        CMD_TIMEOUT,
    );
}

fn run_amixer(args: &[&str]) {
    let mut full_args = vec!["-c", CARD];
    full_args.extend_from_slice(args);
    match run_with_timeout("amixer", &full_args, CMD_TIMEOUT) {
        Some(out) if !out.status.success() => {
            eprintln!("amixer fehlgeschlagen ({args:?}): {}", String::from_utf8_lossy(&out.stderr));
        }
        None => eprintln!("amixer nicht aufrufbar oder hängt fest ({args:?})"),
        _ => {}
    }
}
