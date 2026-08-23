//! Firmware-Versions-Check über Creatives Update-Feed – reiner Netzwerk-
//! Request, kein USB-Bezug (siehe HANDOFF.md Abschnitt 5). Nutzt `curl`
//! als Subprozess statt einer neuen HTTP-Crate-Abhängigkeit, gleiches
//! Muster wie amixer/wpctl in mixer.rs.

use crate::proc_util::run_with_timeout;
use std::time::Duration;

const RELEASE_URL: &str =
    "https://files.creative.com/creative/bin/firmware/fwureleases/SB1900/SB1900_release.json";
const TIMEOUT: Duration = Duration::from_secs(8);

/// Fragt die neueste verfügbare Firmware-Version ab. None bei jedem
/// Fehler (kein Internet, Timeout, unerwartetes Format) – wird vom
/// Aufrufer als "Prüfung fehlgeschlagen" behandelt.
///
/// Bewusst kein JSON-Parser für das eine Feld, das uns interessiert
/// (Antwort ist z.B. `{"latestVersion": "1.2.250819.0903", "minReqVer":
/// "00000000", "minReqList": []}`) – spart eine Abhängigkeit für eine
/// einzelne Zeichenkette, siehe persist.rs für dieselbe Überlegung.
pub fn latest_version() -> Option<String> {
    let out = run_with_timeout("curl", &["-s", "--max-time", "5", RELEASE_URL], TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let key = "\"latestVersion\"";
    let after_key = &text[text.find(key)? + key.len()..];
    let after_first_quote = &after_key[after_key.find('"')? + 1..];
    let version = after_first_quote[..after_first_quote.find('"')?].trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}
