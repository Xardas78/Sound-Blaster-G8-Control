//! Eigene Sound-Modi ("Neuen Modus hinzufügen" im Original): rein lokal in der
//! App gespeichert (der Windows-Mitschnitt zeigt dafür keinen Gerätebefehl, siehe
//! HANDOFF.md Nachtrag 2026-10-26 (3)). Ein Modus ist der komplette Satz aus
//! Acoustic-Engine-Werten und EQ-Kurve, angewendet wird er wie ein eingebautes Preset.

use crate::persist;
use crate::protocol::sound_mode::SoundModePreset;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct UserMode {
    pub name: String,
    pub preset: SoundModePreset,
}

fn path() -> PathBuf {
    persist::config_dir().join("modes.txt")
}

/// Eine Zeile je Modus: `Name<TAB>23 kommagetrennte Werte`.
fn to_line(m: &UserMode) -> String {
    let p = &m.preset;
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let mut v = vec![
        flag(p.surround_on), p.surround_level, flag(p.crystalizer_on), p.crystalizer_level,
        flag(p.bass_on), p.bass_level, p.bass_xover, flag(p.svm_on), p.svm_level, p.svm_mode,
        flag(p.dialog_on), p.dialog_level, p.eq_preamp,
    ];
    v.extend_from_slice(&p.eq_bands);
    let values: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("{}\t{}", m.name, values.join(","))
}

fn from_line(line: &str) -> Option<UserMode> {
    let (name, rest) = line.split_once('\t')?;
    let v: Vec<f32> = rest.split(',').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
    if v.len() != 23 || name.is_empty() {
        return None;
    }
    let mut eq_bands = [0.0f32; 10];
    eq_bands.copy_from_slice(&v[13..23]);
    Some(UserMode {
        name: name.to_string(),
        preset: SoundModePreset {
            surround_on: v[0] > 0.5, surround_level: v[1],
            crystalizer_on: v[2] > 0.5, crystalizer_level: v[3],
            bass_on: v[4] > 0.5, bass_level: v[5], bass_xover: v[6],
            svm_on: v[7] > 0.5, svm_level: v[8], svm_mode: v[9],
            dialog_on: v[10] > 0.5, dialog_level: v[11],
            eq_preamp: v[12], eq_bands, eq_preset_index: -1,
        },
    })
}

pub fn load() -> Vec<UserMode> {
    fs::read_to_string(path()).map(|t| t.lines().filter_map(from_line).collect()).unwrap_or_default()
}

pub fn save(modes: &[UserMode]) {
    let path = path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text: String = modes.iter().map(|m| to_line(m) + "\n").collect();
    if let Err(e) = fs::write(&path, text) {
        eprintln!("Eigene Modi konnten nicht gespeichert werden: {e}");
    }
}

/// Name für die Anzeige/Datei bereinigen (keine Tabs/Zeilenumbrüche, begrenzte Länge).
pub fn clean_name(raw: &str) -> String {
    let s: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    s.trim().chars().take(40).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let (_, base) = crate::protocol::sound_mode::PRESETS[8];
        let m = UserMode { name: "Mein Modus".into(), preset: SoundModePreset { eq_preset_index: -1, ..base } };
        let back = from_line(&to_line(&m)).expect("parsen");
        assert_eq!(back.name, "Mein Modus");
        assert_eq!(back.preset.eq_bands, m.preset.eq_bands);
        assert_eq!(back.preset.surround_on, m.preset.surround_on);
        assert!((back.preset.surround_level - m.preset.surround_level).abs() < 1e-6);
    }

    #[test]
    fn clean_name_strips_control_chars() {
        assert_eq!(clean_name("  a\tb\n "), "a b");
        assert_eq!(clean_name(&"x".repeat(80)).len(), 40);
    }
}
