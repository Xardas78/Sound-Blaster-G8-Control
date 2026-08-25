//! Umgeht eine Nebenwirkung des virtuellen Kopfhörer-Surrounds
//! (virtual_surround.rs): dessen Filter-Chain braucht Zugriff auf die
//! einzelnen Rohkanäle der G8, die nur im PipeWire-"Pro Audio"-Profil
//! existieren. WirePlumber schaltet die Karte dafür automatisch dorthin
//! um – und wechselt beim Verlassen des Kopfhörer-Modus NICHT von selbst
//! zurück (live beobachtet, 2026-08-24: Karte blieb auf "pro-audio",
//! obwohl längst auf Lautsprecher zurückgeschaltet war). Im Pro-Audio-
//! Profil spiegelt PipeWire die ALSA-Hardware-Lautstärke NICHT mehr
//! automatisch (siehe HANDOFF.md Abschnitt 7) – die Synchronisierung mit
//! dem physischen Regler bricht dadurch dauerhaft ab.
//!
//! Zwei Gegenmaßnahmen, mit Erkennung statt fester Annahme (auf
//! Nutzerwunsch): 1. reset_to_stereo_profile() versucht aktiv, zurück auf
//! das normale Stereo-Profil zu schalten (siehe virtual_surround::
//! disable()). 2. Hat das nicht geklappt (is_pro_audio_profile_active()
//! danach erneut abgefragt liefert immer noch true), aktiviert main.rs
//! stattdessen einen aktiven Lautstärke-Push im 120ms-Poll-Thread als
//! Fallback (dort unproblematisch, weil PipeWire im Pro-Profil ohnehin
//! nicht selbst mitspiegelt – kein Zwei-Schreiber-Konflikt wie beim
//! früher entfernten Hintergrunddienst, siehe HANDOFF.md Abschnitt 7
//! Nachtrag 2).

use crate::proc_util::run_with_timeout;
use std::time::Duration;

const CARD_NAME_FRAGMENT: &str = "Sound_Blaster_G8";
const STEREO_PROFILE: &str = "output:analog-stereo+input:analog-stereo";
const CMD_TIMEOUT: Duration = Duration::from_secs(3);

fn card_name() -> Option<String> {
    let out = run_with_timeout("pactl", &["list", "cards", "short"], CMD_TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.contains(CARD_NAME_FRAGMENT))
        .and_then(|l| l.split_whitespace().nth(1))
        .map(str::to_string)
}

/// Sucht den Textblock der G8-Karte in `pactl list cards` (endet an der
/// nächsten "Card #"-Zeile) und prüft darin auf "Active Profile: pro-audio".
pub fn is_pro_audio_profile_active() -> bool {
    let Some(out) = run_with_timeout("pactl", &["list", "cards"], CMD_TIMEOUT) else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let Some(start) = text.find(CARD_NAME_FRAGMENT) else {
        return false;
    };
    let block = &text[start..];
    let end = block[1..].find("Card #").map(|p| p + 1).unwrap_or(block.len());
    block[..end].contains("Active Profile: pro-audio")
}

/// Best-effort: kann fehlschlagen (z.B. wenn noch etwas anderes das
/// Pro-Audio-Profil braucht). Der Aufrufer prüft danach über
/// is_pro_audio_profile_active(), ob es tatsächlich geklappt hat, statt
/// den Erfolg einfach anzunehmen.
pub fn reset_to_stereo_profile() {
    let Some(name) = card_name() else {
        eprintln!("audio_profile: G8-Kartenname nicht gefunden, kann Profil nicht zurücksetzen");
        return;
    };
    run_with_timeout("pactl", &["set-card-profile", &name, STEREO_PROFILE], CMD_TIMEOUT);
}
