//! Mixer-Pegel der G8 sind Standard-UAC2-Feature-Units. Der Kernel-Treiber
//! (snd-usb-audio) exponiert sie bereits als ALSA-Mixer-Controls auf Karte
//! "USB1" – kein 5a-Report, kein eigener USB-Zugriff nötig (siehe
//! HANDOFF.md, "Standard-USB-Audio-Class"). Namen per `amixer -c USB1
//! controls` verifiziert.

use crate::proc_util::run_with_timeout;
use alsa::mixer::{Mixer, Selem, SelemChannelId, SelemId};
use std::cell::RefCell;
use std::time::Duration;

const CARD: &str = "USB1";
// amixer/wpctl laufen auf dem 120ms-Poll-Thread (siehe main.rs) – ein
// Timeout deutlich unter dem Poll-Intervall verhindert, dass ein
// hängender Aufruf (z.B. bei einem PipeWire-USB-Stall, siehe HANDOFF.md)
// den Thread und damit die komplette Lautstärke-Anzeige einfriert.
const CMD_TIMEOUT: Duration = Duration::from_millis(500);

const ENOENT: i32 = 2;

thread_local! {
    // Pro Thread eine dauerhafte ALSA-Verbindung (Mixer ist nicht Send): der
    // 120ms-Poll-Thread liest, der UI-Thread schreibt. Statt pro Zugriff einen
    // amixer-Prozess zu starten (Issue #8). Bei jedem ALSA-Fehler (z.B. Karte
    // nach einer Neuanmeldung des G8 weg) wird sie verworfen und beim nächsten
    // Zugriff neu geöffnet.
    static MIXER: RefCell<Option<Mixer>> = const { RefCell::new(None) };
}

fn with_selem<T>(entity_id: i32, f: impl FnOnce(&Selem, bool, (i64, i64)) -> Result<T, alsa::Error>) -> Option<T> {
    let name = usize::try_from(entity_id).ok().and_then(|i| CONTROLS.get(i))?;
    let (simple, direction) = simple_name_and_direction(name)?;
    let capture = direction == "capture";
    MIXER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Mixer::new(&format!("hw:{CARD}"), false).ok();
        }
        let mixer = slot.as_ref()?;
        let _ = mixer.handle_events();
        let result = (|| {
            let selem = mixer.find_selem(&SelemId::new(simple, 0)).ok_or_else(|| alsa::Error::new("find_selem", ENOENT))?;
            let range = if capture { selem.get_capture_volume_range() } else { selem.get_playback_volume_range() };
            f(&selem, capture, range)
        })();
        match result {
            Ok(v) => Some(v),
            Err(_) => {
                *slot = None;
                None
            }
        }
    })
}

fn percent_to_raw(percent: f32, (min, max): (i64, i64)) -> i64 {
    min + (percent.clamp(0.0, 100.0) / 100.0 * (max - min) as f32).round() as i64
}

fn raw_to_percent(raw: i64, (min, max): (i64, i64)) -> f32 {
    if max <= min {
        return 0.0;
    }
    (raw - min) as f32 / (max - min) as f32 * 100.0
}

fn write_levels(entity_id: i32, left_pct: f32, right_pct: f32) {
    let ok = with_selem(entity_id, |selem, capture, range| {
        let (l, r) = (percent_to_raw(left_pct, range), percent_to_raw(right_pct, range));
        if capture {
            selem.set_capture_volume(SelemChannelId::FrontLeft, l)?;
            selem.set_capture_volume(SelemChannelId::FrontRight, r)
        } else {
            selem.set_playback_volume(SelemChannelId::FrontLeft, l)?;
            selem.set_playback_volume(SelemChannelId::FrontRight, r)
        }
    });
    if ok.is_none() {
        eprintln!("Mixer: Schreiben von Entity {entity_id} fehlgeschlagen (Karte {CARD} nicht erreichbar?)");
    }
}

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
    // Taper-Korrektur nur für den Lautsprecher-Regler (entity-id 0, siehe
    // Kommentar oben bei TAPER_TABLE) – percent kommt hier als "gefühlter"
    // Wert vom Slider, ALSA braucht aber den rohen Wert.
    let alsa_percent = if entity_id == 0 { taper_uncorrect(percent) } else { percent };
    write_levels(entity_id, alsa_percent, alsa_percent);
}

/// Balance in Prozent, -100 (nur links) .. 0 (Mitte) .. 100 (nur rechts).
/// Senkt nur die leisere Seite proportional zum eingestellten Pegel (wie bei
/// PulseAudio/Windows üblich) – hebt die lautere Seite nie über den Pegel
/// hinaus. `level_percent` ist der "gefühlte" Wert wie bei set_level(). Per
/// `amixer sget` verifiziert: alle acht CONTROLS-Einträge haben unabhängige
/// Front-Left/Front-Right-Kanäle (auch die Mikrofoneingänge), Balance ist
/// also für jede Mixer-Zeile technisch möglich.
pub fn set_balance(entity_id: i32, level_percent: f32, balance: f32) {
    let alsa_percent = if entity_id == 0 { taper_uncorrect(level_percent) } else { level_percent };
    let alsa_percent = alsa_percent.clamp(0.0, 100.0);
    let balance = balance.clamp(-100.0, 100.0);
    let left = alsa_percent * if balance > 0.0 { 1.0 - balance / 100.0 } else { 1.0 };
    let right = alsa_percent * if balance < 0.0 { 1.0 + balance / 100.0 } else { 1.0 };
    write_levels(entity_id, left, right);
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
    // MAXIMUM über beide Kanäle statt nur Front Left: set_balance() dämpft
    // gezielt nur die leisere Seite, der unangetastete Kanal zeigt immer den
    // wahren, vor der Balance gemeinten Pegel – ein reiner Left-Read würde bei
    // aktiver Balance eine viel zu niedrige Anzeige liefern.
    let alsa_pct = with_selem(entity_id, |selem, capture, range| {
        let read = |ch| if capture { selem.get_capture_volume(ch) } else { selem.get_playback_volume(ch) };
        let l = read(SelemChannelId::FrontLeft)?;
        let r = read(SelemChannelId::FrontRight).unwrap_or(l);
        Ok(raw_to_percent(l.max(r), range))
    })?;
    Some(if entity_id == 0 { taper_correct(alsa_pct) } else { alsa_pct })
}

/// Fallback für den Pro-Audio-Profil-Fall (siehe audio_profile.rs): dort
/// spiegelt PipeWire die ALSA-Hardware-Lautstärke nicht mehr automatisch,
/// hier wird sie stattdessen aktiv nachgezogen. Nur sicher, wenn der
/// Aufrufer vorher audio_profile::is_pro_audio_profile_active() geprüft
/// hat – sonst droht wieder der Zwei-Schreiber-Bug von früher (siehe
/// HANDOFF.md Abschnitt 7 Nachtrag 2).
///
/// `felt_percent` ist der taper-korrigierte "gefühlte" Wert wie er auch
/// im Fußleisten-/Sidebar-Regler angezeigt wird (siehe get_level()) –
/// PipeWires eigene Lautstärke-Skala ist aber linear zum rohen ALSA-Pegel,
/// nicht zu unserer Poti-Korrekturkurve. Erst zurückrechnen (taper_uncorrect),
/// sonst zeigt `wpctl get-volume` einen ganz anderen Wert als `amixer`
/// tatsächlich hat (z.B. 44% statt echter 80%).
pub fn set_pipewire_sink_volume_percent(felt_percent: f32) {
    let alsa_percent = taper_uncorrect(felt_percent);
    let vol = (alsa_percent / 100.0).clamp(0.0, 1.0);
    run_with_timeout(
        "wpctl",
        &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{vol:.2}")],
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Braucht die echte G8 (Karte USB1): `cargo test -- --ignored mixer_roundtrip`.
    #[test]
    #[ignore]
    fn mixer_roundtrip() {
        let before = get_level(1).expect("Line In Playback lesen");
        set_level(1, 40.0);
        let after = get_level(1).expect("Line In Playback nach Schreiben lesen");
        set_level(1, before);
        assert!((after - 40.0).abs() < 2.0, "erwartet ~40, gelesen {after}");
        let restored = get_level(1).unwrap();
        assert!((restored - before).abs() < 2.0, "Wiederherstellung: {before} -> {restored}");
    }
}
