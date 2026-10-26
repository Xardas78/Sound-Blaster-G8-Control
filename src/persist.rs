//! Speichert die App-seitigen Einstellungen, die das Gerät selbst nicht
//! zurücklesen lässt (Acoustic Engine/Equalizer-Werte, Energiesparen,
//! Logo-LED, Dark Mode, Schließen-Verhalten), damit sie nach einem
//! Neustart der App wieder angezeigt UND ans Gerät gesendet werden – ohne
//! das würde die App nach jedem Start wieder ihre eingebauten Standard-
//! werte zeigen, obwohl das Gerät längst andere Werte hat.
//!
//! Bewusst kein serde/JSON: die paar Felder hier sind einfach genug für
//! ein simples "schlüssel=wert"-Textformat, spart eine schwere
//! Abhängigkeit für eine Handvoll Zahlen/Bools.

use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub struct Settings {
    pub ae_surround_on: bool,
    pub ae_surround_level: f32,
    pub ae_crystalizer_on: bool,
    pub ae_crystalizer_level: f32,
    pub ae_bass_on: bool,
    pub ae_bass_level: f32,
    pub ae_svm_on: bool,
    pub ae_svm_level: f32,
    pub ae_svm_mode: i32,
    pub ae_dialog_on: bool,
    pub ae_dialog_level: f32,
    pub ae_eq_on: bool,
    pub ae_eq_preamp: f32,
    pub ae_eq_bands: [f32; 10],
    pub power_saving_on: bool,
    pub logo_led_on: bool,
    pub dark_mode_on: bool,
    pub close_behavior: i32,
    // -1 = kein Sound-Modus-Preset gewählt ("Im Gerät"), sonst Index in
    // sound_mode::PRESETS. Rein software-seitig gemerkt (übersteht App-
    // Neustart) – das Gerät selbst hat keinen Rücklese-Mechanismus dafür
    // (HANDOFF.md Abschnitt 8), ein echtes Auslesen bräuchte einen neuen
    // USB-Capture unter Windows.
    pub sound_mode_preset_index: i32,
    pub eq_preset_index: i32,
    pub language: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ae_surround_on: true,
            ae_surround_level: 0.48,
            ae_crystalizer_on: true,
            ae_crystalizer_level: 0.52,
            ae_bass_on: true,
            ae_bass_level: 0.52,
            ae_svm_on: true,
            ae_svm_level: 0.52,
            ae_svm_mode: 0,
            ae_dialog_on: true,
            ae_dialog_level: 0.56,
            ae_eq_on: true,
            ae_eq_preamp: 0.0,
            ae_eq_bands: [0.0; 10],
            power_saving_on: false,
            logo_led_on: false,
            dark_mode_on: false,
            close_behavior: 0,
            sound_mode_preset_index: -1,
            eq_preset_index: -1,
            language: 0,
        }
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").expect("HOME nicht gesetzt")).join(".config")
        });
    base.join("g8-control").join("settings.txt")
}

pub fn config_dir() -> PathBuf {
    config_path().parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}

pub fn load() -> Settings {
    let mut s = Settings::default();
    let Ok(text) = fs::read_to_string(config_path()) else {
        return s;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        match key {
            "ae_surround_on" => s.ae_surround_on = value == "1",
            "ae_surround_level" => s.ae_surround_level = value.parse().unwrap_or(s.ae_surround_level),
            "ae_crystalizer_on" => s.ae_crystalizer_on = value == "1",
            "ae_crystalizer_level" => s.ae_crystalizer_level = value.parse().unwrap_or(s.ae_crystalizer_level),
            "ae_bass_on" => s.ae_bass_on = value == "1",
            "ae_bass_level" => s.ae_bass_level = value.parse().unwrap_or(s.ae_bass_level),
            "ae_svm_on" => s.ae_svm_on = value == "1",
            "ae_svm_level" => s.ae_svm_level = value.parse().unwrap_or(s.ae_svm_level),
            "ae_svm_mode" => s.ae_svm_mode = value.parse().unwrap_or(s.ae_svm_mode),
            "ae_dialog_on" => s.ae_dialog_on = value == "1",
            "ae_dialog_level" => s.ae_dialog_level = value.parse().unwrap_or(s.ae_dialog_level),
            "ae_eq_on" => s.ae_eq_on = value == "1",
            "ae_eq_preamp" => s.ae_eq_preamp = value.parse().unwrap_or(s.ae_eq_preamp),
            "ae_eq_bands" => {
                let parsed: Vec<f32> = value.split(',').filter_map(|v| v.parse().ok()).collect();
                if parsed.len() == 10 {
                    s.ae_eq_bands.copy_from_slice(&parsed);
                }
            }
            "power_saving_on" => s.power_saving_on = value == "1",
            "logo_led_on" => s.logo_led_on = value == "1",
            "dark_mode_on" => s.dark_mode_on = value == "1",
            "close_behavior" => s.close_behavior = value.parse().unwrap_or(s.close_behavior),
            "language" => s.language = value.parse().unwrap_or(s.language),
            "eq_preset_index" => {
                s.eq_preset_index = value.parse().unwrap_or(s.eq_preset_index)
            }
            "sound_mode_preset_index" => {
                s.sound_mode_preset_index = value.parse().unwrap_or(s.sound_mode_preset_index)
            }
            _ => {}
        }
    }
    s
}

pub fn save(s: &Settings) {
    let bands = s
        .ae_eq_bands
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let text = format!(
        "ae_surround_on={}\nae_surround_level={}\nae_crystalizer_on={}\nae_crystalizer_level={}\nae_bass_on={}\nae_bass_level={}\nae_svm_on={}\nae_svm_level={}\nae_svm_mode={}\nae_dialog_on={}\nae_dialog_level={}\nae_eq_on={}\nae_eq_preamp={}\nae_eq_bands={}\npower_saving_on={}\nlogo_led_on={}\ndark_mode_on={}\nclose_behavior={}\nsound_mode_preset_index={}\neq_preset_index={}\nlanguage={}\n",
        s.ae_surround_on as i32,
        s.ae_surround_level,
        s.ae_crystalizer_on as i32,
        s.ae_crystalizer_level,
        s.ae_bass_on as i32,
        s.ae_bass_level,
        s.ae_svm_on as i32,
        s.ae_svm_level,
        s.ae_svm_mode,
        s.ae_dialog_on as i32,
        s.ae_dialog_level,
        s.ae_eq_on as i32,
        s.ae_eq_preamp,
        bands,
        s.power_saving_on as i32,
        s.logo_led_on as i32,
        s.dark_mode_on as i32,
        s.close_behavior,
        s.sound_mode_preset_index,
        s.eq_preset_index,
        s.language,
    );
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&path, text) {
        eprintln!("Einstellungen konnten nicht gespeichert werden ({}): {e}", path.display());
    }
}
