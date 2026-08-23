// Katalog aller bekannten Parameter, auch die noch nicht ans UI angebundenen
// (Profilblöcke, Modus-Parameter etc.) – bewusst als vollständige Referenz
// erhalten, siehe HANDOFF.md.
#![allow(dead_code)]

//! Sound Blaster G8 – reverse-engineertes proprietäres Protokoll.
//!
//! Befehlskanal: HID SET_REPORT (Output Report, ReportID 0) per Control-Transfer
//! an Interface 4 (wIndex 4) des Composite-Geräts mit USB-ID 041e:329b.
//! 64-Byte-Payload, KEIN separater Interrupt-OUT-Endpoint.
//!
//! Byte-Layout für die meisten Parameter (Toggle/Level):
//!   [0]     0x5a  fester Report-Header
//!   [1]     0x12  Kommando (Set Parameter)
//!   [2]     0x07  fest
//!   [3]     0x01  fest
//!   [4]     Namespace (0x95 Aufnahme / 0x96 Wiedergabe / 0x97 Decoder)
//!   [5]     Parameter-ID
//!   [6..10] Float32 little-endian Wert
//!   [10..64] Padding (0x00)
//!
//! Toggle-Parameter verwenden denselben Float-Slot: 0.0 = aus, 1.0 = ein.
//!
//! Standard-USB-Audio-Class (kein Reverse Engineering nötig, hier nicht abgebildet):
//!   - Lautstärke (Feature Unit SET_CUR/GET_CUR)
//!   - Mixer-Pegel (7 Feature-Unit-Entities: 0x03,0x04,0x05,0x06,0x09,0x0a,0x0c)
//!   - Bit-Tiefe/Sample-Format (SET_INTERFACE, Alt-Setting 0=Idle/1=24bit/2=32bit
//!     auf Interface 1)
//!   - Mic-Jack-Sensing (UAC2 Interrupt Data Message auf EP 0x84)

/// Adressierbare Parameter-Namespaces (Byte 4 im Report).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    /// Aufnahme-Seite (CrystalVoice).
    Recording = 0x95,
    /// Wiedergabe-Seite (Acoustic Engine, Equalizer).
    Playback = 0x96,
    /// Decoder (Dolby Digital).
    Decoder = 0x97,
}

/// Ein Ein/Aus-Schalter-Parameter. Wert 0.0 = aus, 1.0 = ein.
#[derive(Clone, Copy, Debug)]
pub struct ToggleParam {
    pub namespace: Namespace,
    pub param_id: u8,
}

/// Ein kontinuierlicher Pegel-/Wert-Parameter (Float32).
#[derive(Clone, Copy, Debug)]
pub struct LevelParam {
    pub namespace: Namespace,
    pub param_id: u8,
}

/// Baut den vollständigen 64-Byte-Output-Report für einen Toggle- oder Level-Parameter.
pub fn build_report(namespace: Namespace, param_id: u8, value: f32) -> [u8; 64] {
    let mut buf = [0u8; 64];
    buf[0] = 0x5a;
    buf[1] = 0x12;
    buf[2] = 0x07;
    buf[3] = 0x01;
    buf[4] = namespace as u8;
    buf[5] = param_id;
    buf[6..10].copy_from_slice(&value.to_le_bytes());
    buf
}

pub fn build_toggle_report(p: ToggleParam, on: bool) -> [u8; 64] {
    build_report(p.namespace, p.param_id, if on { 1.0 } else { 0.0 })
}

pub fn build_level_report(p: LevelParam, value: f32) -> [u8; 64] {
    build_report(p.namespace, p.param_id, value)
}

// ---------------------------------------------------------------------------
// Acoustic Engine (Wiedergabe, Namespace::Playback)
//
// Per USB-Capture ("acoustic engine.pcapng") bestätigt: alle LevelParam-
// Werte hier sind Floats zwischen 0.0 und 1.0, NICHT 0..100! Die UI zeigt
// 0..100 an (wie im Original), muss den Wert also vor dem Senden durch
// 100 teilen (siehe ui/tabs/acoustic-engine-tab.slint). Toggle-Werte
// (0.0/1.0) und SMART_VOLUME_MODE (0.0=Normal/1.0=Laut/2.0=Nacht) sind
// laut Capture unverändert korrekt.
// ---------------------------------------------------------------------------
pub mod acoustic_engine {
    use super::{LevelParam, Namespace, ToggleParam};

    pub const SURROUND_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Playback, param_id: 0x00 };
    pub const SURROUND_LEVEL: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x01 };

    pub const DIALOG_PLUS_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Playback, param_id: 0x02 };
    pub const DIALOG_PLUS_LEVEL: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x03 };

    pub const SMART_VOLUME_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Playback, param_id: 0x04 };
    pub const SMART_VOLUME_LEVEL: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x05 };
    /// Modus-Parameter (kein einfacher Toggle): Nacht=2.0, Laut=1.0, Normal=0.0
    pub const SMART_VOLUME_MODE: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x06 };

    pub const CRYSTALIZER_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Playback, param_id: 0x07 };
    pub const CRYSTALIZER_LEVEL: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x08 };

    pub const BASS_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Playback, param_id: 0x18 };
    pub const BASS_LEVEL: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x19 };

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum SmartVolumeMode {
        Normal,
        Laut,
        Nacht,
    }
    impl SmartVolumeMode {
        pub fn as_value(self) -> f32 {
            match self {
                SmartVolumeMode::Normal => 0.0,
                SmartVolumeMode::Laut => 1.0,
                SmartVolumeMode::Nacht => 2.0,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wiedergabe-Equalizer (10-Band, Namespace::Playback)
// ---------------------------------------------------------------------------
pub mod equalizer {
    use super::{LevelParam, Namespace, ToggleParam};

    pub const TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Playback, param_id: 0x09 };
    /// Separater Parameter, größerer Wertebereich (auch negativ).
    pub const PREAMP: LevelParam = LevelParam { namespace: Namespace::Playback, param_id: 0x0a };

    /// 10 Frequenzbänder, IDs 0x0b..=0x14 (62Hz .. 16kHz).
    /// "Bass"- und "Höhen"-Kurzregler im Original steuern direkt Bänder aus
    /// diesem Block, haben keine eigene ID.
    pub const BAND_IDS: [u8; 10] = [0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14];

    pub fn band(index: usize) -> LevelParam {
        LevelParam { namespace: Namespace::Playback, param_id: BAND_IDS[index] }
    }

    /// ACHTUNG – vermutlich Fehlinterpretation, nicht verwenden: ursprünglich
    /// aus dem USB-Capture "hp-spk ein-aus.pcapng" als "Kopfhörer- bzw.
    /// Lautsprecher-EQ-Profil" abgeleitet. Live-Tests am echten Gerät haben
    /// aber gezeigt, dass die dortigen "5a 2c 05 01 <02|04>"-Blöcke Teil
    /// eines periodischen Status-Heartbeats sind (alle ca. 1,5-2,5s, sobald
    /// das Gerät aktiv ist), NICHT an den physischen Schalter gekoppelt –
    /// die beiden Werte hier sind vermutlich nur zwei Momentaufnahmen
    /// desselben Heartbeats, keine echten Kopfhörer/Lautsprecher-Profile.
    /// Siehe HANDOFF.md.
    pub const HEADPHONE_PREAMP: f32 = 0.0;
    pub const HEADPHONE_BANDS: [f32; 10] = [3.0, 4.0, -1.0, -2.0, 0.0, 2.0, 2.0, -5.0, 2.0, 0.0];
    pub const SPEAKER_PREAMP: f32 = 0.0;
    pub const SPEAKER_BANDS: [f32; 10] = [0.0; 10];
}

// ---------------------------------------------------------------------------
// Sound-Modus-Presets (kombiniert Acoustic Engine + Equalizer). Echte
// Original-Presets, 1:1 aus den Windows-JSON-Dateien (SoundMode/*.json,
// Kopfhörer-Variante, verknüpft mit dem passenden EQ-Preset über
// GraphicEQ.PresetId) übernommen – siehe HANDOFF.md Abschnitt 6.
// ---------------------------------------------------------------------------
pub mod sound_mode {
    #[derive(Clone, Copy)]
    pub struct SoundModePreset {
        pub surround_on: bool,
        pub surround_level: f32,
        pub crystalizer_on: bool,
        pub crystalizer_level: f32,
        pub bass_on: bool,
        pub bass_level: f32,
        pub bass_xover: f32,
        pub svm_on: bool,
        pub svm_level: f32,
        pub svm_mode: f32,
        pub dialog_on: bool,
        pub dialog_level: f32,
        pub eq_preamp: f32,
        pub eq_bands: [f32; 10],
    }

    pub const PRESETS: [(&str, SoundModePreset); 60] = [
        ("64 Audio Aspire Series", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 2.0, -1.5, -1.5, 0.0, 2.0, -2.0, 2.0, 2.0] }),
        ("Adventure and Action", SoundModePreset { surround_on: true, surround_level: 0.3, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [4.0, 4.0, -1.0, -2.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0] }),
        ("Apex Legends", SoundModePreset { surround_on: true, surround_level: 0.2, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 3.0, -2.0, 3.0, -2.0, 0.0, 2.0, -3.0, 2.0, 0.0] }),
        ("Arena Of Valor", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.3, bass_on: true, bass_level: 0.3, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, 2.0, -2.0, -2.0, 2.0, 2.0, -3.0, 2.0, 0.0] }),
        ("Assetto Corsa", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 0.0, 2.0, 2.0, 0.0, 2.0, -2.0, 2.0, -1.0] }),
        ("Baldur's Gate 3", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, 0.0, 0.0, 2.0, 3.0, 3.0, 0.0, 3.0, 2.0] }),
        ("Bass Boost", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 5.0, 3.0, -1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0] }),
        ("Bass Cut", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [-3.0, -5.0, -5.0, -1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0] }),
        ("Battle Royale", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: true, crystalizer_level: 0.35, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: true, dialog_level: 0.15, eq_preamp: 0.0, eq_bands: [2.0, 2.0, 0.0, 0.0, 2.0, 2.0, 2.0, -3.0, 0.0, 0.0] }),
        ("Battlefield Series", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 3.0, 2.0, 0.0, 0.0, 0.0, 0.0, -3.0, 2.0, 0.0] }),
        ("Black Myth: Wukong", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 3.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 2.0] }),
        ("Call of Duty Series", SoundModePreset { surround_on: true, surround_level: 0.3, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [4.0, 3.0, -1.0, -2.0, 2.0, 2.0, 2.0, -4.0, 0.0, 2.0] }),
        ("Counter-Strike 2", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [-6.0, -3.0, 0.0, -6.0, -1.0, -1.0, -1.0, -3.0, 3.0, 0.0] }),
        ("Counter-Strike: Global Offensive", SoundModePreset { surround_on: true, surround_level: 0.2, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [-5.0, 3.0, -3.0, -3.0, -2.0, -2.0, 2.0, 0.0, 0.0, 3.0] }),
        ("Cyberpunk 2077", SoundModePreset { surround_on: true, surround_level: 0.67, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.2, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 3.0, 0.0, -2.0, 0.0, 2.0, 2.0, -3.0, 2.0, 0.0] }),
        ("Death Stranding", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.2, eq_preamp: 0.0, eq_bands: [2.0, 2.0, 2.0, 0.0, 0.0, 2.0, 2.0, 0.0, 2.0, 0.0] }),
        ("Delta Force", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 4.0, 2.0, 2.0, 0.0, 0.0, 0.0, -2.0, 3.0, 0.0] }),
        ("Diablo Series", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, -2.0, 0.0, 0.0, 1.0, 2.0, -4.0, 2.0, 0.0] }),
        ("Doom Eternal", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, -2.0, -2.0, 0.0, 0.0, -2.0, -3.0, -1.0, 0.0] }),
        ("DOTA 2", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [-5.0, 0.0, -3.0, -4.0, -5.0, -1.0, -3.0, -2.0, -3.0, -1.0] }),
        ("Dynamic Music", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 4.0, 2.0, -1.0, -1.0, 0.0, 0.0, 0.0, 4.0, 4.0] }),
        ("Effects Off", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }),
        ("Escape from Tarkov", SoundModePreset { surround_on: true, surround_level: 0.2, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, -1.0, 0.0, 0.0, 2.0, 2.0, -2.0, 2.0, 2.0] }),
        ("Esports", SoundModePreset { surround_on: true, surround_level: 0.3, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [-4.0, -4.0, 4.0, 3.0, 0.0, 2.0, 0.0, 2.0, 3.0, 0.0] }),
        ("Fortnite", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: true, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, -2.0, -2.0, 0.0, 0.0, 2.0, -2.0, 0.0, 0.0] }),
        ("Forza Horizon Series", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 0.0, 3.0, 2.0, 0.0, 0.0, 2.0, 2.0, 0.0] }),
        ("FPS", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 3.0, -2.0, -2.0, 0.0, 2.0, 2.0, -5.0, 2.0, 1.0] }),
        ("Gaming", SoundModePreset { surround_on: true, surround_level: 0.12, crystalizer_on: true, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 4.0, -1.0, -2.0, 0.0, 2.0, 2.0, -5.0, 2.0, 0.0] }),
        ("Genshin Impact", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.0, 0.0, 0.0, 0.0, 2.0, 5.0, 0.0, 0.0, 0.0] }),
        ("Halo Infinite", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 0.0, 3.0, 2.0, 0.0, 0.0, 0.0, 2.0, 2.0, 0.0] }),
        ("Helldivers 2", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 2.0, 3.0, 0.0, 0.0, 0.0, 3.0, 2.0, 0.0, 0.0] }),
        ("Hogswart Legacy", SoundModePreset { surround_on: true, surround_level: 0.6, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [0.0, 2.0, 0.0, 2.0, 0.0, 2.0, 2.0, 2.0, 3.0, 0.0] }),
        ("League of Legends", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [-3.0, 0.0, -3.0, -4.0, -5.0, -1.0, -3.0, -2.0, 2.0, 0.0] }),
        ("Marvel Rivals", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 3.0, 0.0, 0.0, 0.0, 2.0, 2.0, -3.0, 3.0, 3.0] }),
        ("Marvel's Spider-Man Series", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [2.0, 2.0, 2.0, 0.0, 0.0, 2.0, 0.0, 3.0, 3.0, 0.0] }),
        ("Minecraft Dungeons", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 0.0, 3.0, 0.0, 0.0, 0.0, 2.0, 2.0, 0.0] }),
        ("MMORPG", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 3.0, -2.0, -2.0, 0.0, 1.5, 0.0, -3.5, 2.0, 0.0] }),
        ("MOBA", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.4, bass_on: true, bass_level: 0.25, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 0.0, 0.0, 2.0, 0.0, 2.0, -3.0, 0.0, 0.0] }),
        ("Movies", SoundModePreset { surround_on: true, surround_level: 0.67, crystalizer_on: true, crystalizer_level: 0.65, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.4, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [4.0, 3.0, -2.0, -2.0, 2.0, 2.0, 1.0, 0.0, 1.0, 2.0] }),
        ("Music", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 2.0, 0.0, -2.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.5] }),
        ("Overwatch 2", SoundModePreset { surround_on: true, surround_level: 0.67, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [2.0, 2.5, 1.5, -1.5, 1.5, 2.0, 2.0, -3.5, 1.5, 2.0] }),
        ("Path of Exile 2", SoundModePreset { surround_on: true, surround_level: 0.25, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 5.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0] }),
        ("PUBG: BATTLEGROUNDS", SoundModePreset { surround_on: true, surround_level: 0.45, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 3.0, -4.0, -4.0, 0.0, 2.0, 2.0, 0.0, 2.0, 0.0] }),
        ("Racing Simulation", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.4, eq_preamp: 0.0, eq_bands: [4.0, 4.0, 2.0, 2.0, -2.0, -2.0, 2.0, 0.0, 2.0, 0.0] }),
        ("Red Dead Redemption 2", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: true, crystalizer_level: 0.2, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.2, eq_preamp: 0.0, eq_bands: [0.0, 2.0, 2.0, 0.0, -2.0, 3.0, 2.0, -2.0, 0.0, 0.0] }),
        ("Resident Evil Series", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.2, eq_preamp: 0.0, eq_bands: [0.0, 2.0, 2.0, 2.0, 0.0, 2.0, 0.0, -2.0, 2.0, 0.0] }),
        ("Rocket League", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 3.0, -2.0, 2.0, 2.0, 0.0, 2.0, 0.0, -2.0, 0.0] }),
        ("RPG", SoundModePreset { surround_on: true, surround_level: 0.35, crystalizer_on: false, crystalizer_level: 0.5, bass_on: true, bass_level: 0.2, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: true, dialog_level: 0.15, eq_preamp: 0.0, eq_bands: [0.0, 2.0, 0.0, -1.0, -1.0, 3.0, 2.0, -2.0, 2.0, 2.0] }),
        ("RTS", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.5, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, -2.0, -3.0, 0.0, -2.0, 2.0, 0.0, -3.0, -1.0, 0.0] }),
        ("S.T.A.L.K.E.R. 2: Heart of Chornobyl", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 2.0, -2.0, 2.0, 2.0] }),
        ("Stadium", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [0.0, 0.0, -3.0, 2.0, 3.0, 3.0, 2.0, -2.0, 3.0, 0.0] }),
        ("Star War Jedi Series", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.3, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 0.0, 2.0, 2.0, 0.0, 0.0, -3.0, 2.0, 0.0] }),
        ("The Witcher Series", SoundModePreset { surround_on: true, surround_level: 0.65, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.3, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 4.0, -2.0, 0.0, 0.0, 2.0, 2.0, 0.0, -2.0, 0.0] }),
        ("Tom Clancy's Rainbow Six Siege", SoundModePreset { surround_on: true, surround_level: 0.35, crystalizer_on: true, crystalizer_level: 0.3, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: true, svm_level: 0.2, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [3.0, 3.0, 0.0, 0.0, -2.0, 2.0, 3.0, -3.0, 0.0, 0.0] }),
        ("Treble Boost", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 4.0, 4.0] }),
        ("Treble Cut", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -5.0, -4.0, -2.0] }),
        ("Valorant", SoundModePreset { surround_on: false, surround_level: 0.67, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, -3.0, 4.0, 3.0, 0.0, 0.0, 0.0, -2.0, 3.0, 0.0] }),
        ("Warframe", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: true, dialog_level: 0.2, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0, 2.0, 0.0] }),
        ("Warhammer 40000 Space Marine 2", SoundModePreset { surround_on: true, surround_level: 0.5, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 3.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0] }),
        ("World War 3", SoundModePreset { surround_on: true, surround_level: 0.4, crystalizer_on: false, crystalizer_level: 0.5, bass_on: false, bass_level: 0.5, bass_xover: 80.0, svm_on: false, svm_level: 0.74, svm_mode: 0.0, dialog_on: false, dialog_level: 0.5, eq_preamp: 0.0, eq_bands: [0.0, 0.0, 3.0, 2.0, 0.0, 0.0, 0.0, 2.0, 2.0, 0.0] }),
    ];
}

// ---------------------------------------------------------------------------
// CrystalVoice (Aufnahme, Namespace::Recording)
// ---------------------------------------------------------------------------
pub mod crystal_voice {
    use super::{LevelParam, Namespace, ToggleParam};

    pub const AEC_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Recording, param_id: 0x00 };

    pub const NOISE_REDUCTION_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Recording, param_id: 0x04 };
    pub const NOISE_REDUCTION_LEVEL: LevelParam = LevelParam { namespace: Namespace::Recording, param_id: 0x05 };

    /// Per Windows-Capture ("mic voicemorph ein-aus.pcapng") korrigiert:
    /// 0x0a, nicht 0x2c wie zuvor angenommen (0x2c ist Smart Volume Mic).
    pub const VOICE_MORPH_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Recording, param_id: 0x0a };
    /// Profilabhängiger Parameterblock (Pitch/Formant), 8 Werte pro
    /// Charakter (nicht 9 – 0x0a ist der Toggle selbst, kein Profilwert).
    /// Exakte Werte je Charakter liegen als JSON in der Windows-Install
    /// (`VoiceFxProfile\*.json`, 18 Presets) – siehe HANDOFF.md Abschnitt 6.
    pub const VOICE_MORPH_PROFILE_IDS: [u8; 8] = [0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12];

    pub const MIC_EQ_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Recording, param_id: 0x13 };
    /// Profilabhängiger Parameterblock, 10 Werte pro Preset (z.B. Pitch-
    /// Reduzierung). Exakte Werte je Preset liegen als JSON in der
    /// Windows-Install (`MicEqProfile\*.json`, 9 Presets) – siehe
    /// HANDOFF.md Abschnitt 6.
    pub const MIC_EQ_PROFILE_IDS: [u8; 10] =
        [0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d];

    /// Per Windows-Capture bestätigt (vorher "Smart Volume (Mic)" fälschlich
    /// für Voice Morph gehalten).
    pub const MIC_SMART_VOLUME_TOGGLE: ToggleParam = ToggleParam { namespace: Namespace::Recording, param_id: 0x2c };

    /// Echte Voice-Morph-Profile, 1:1 aus den Windows-JSON-Presets
    /// (`VoiceFxProfile\*.json`) übernommen – nicht mehr geraten. Werte in
    /// Preset-Reihenfolge = VOICE_MORPH_PROFILE_IDS-Reihenfolge.
    pub const VOICE_MORPH_PRESETS: [(&str, [f32; 8]); 18] = [
        ("Brute", [383.0, 1428.0, 2550.0, 0.65, 1.31138, 1.0, 0.03, 0.0]),
        ("Chipmunk", [400.0, 1400.0, 2000.0, 1.0, 0.601627, 1.0, 0.0, 0.0]),
        ("DeepVoice", [339.5443, 1362.496, 2046.0, 0.4291186, 1.201318, 1.0, 0.0, 0.0]),
        ("Demon", [453.0, 1544.0, 2600.0, 0.69, 2.0, 1.0, 0.0, 0.0]),
        ("Dwarf", [294.0, 1527.0, 2055.0, 0.77, 0.97, 1.0, 0.0, 0.0]),
        ("Elderly", [713.0, 1500.0, 1800.0, 1.4, 1.45, 1.0, 0.1818, 0.0]),
        ("Elf", [398.0, 1395.0, 2451.0, 1.11, 0.96, 1.0, 0.0, 0.0]),
        ("Emo", [400.0, 1400.0, 2000.0, 1.0, 1.0, 0.2, 0.0, 0.0]),
        ("Female", [400.0, 1400.0, 2220.0, 2.37, 0.92, 1.0, 0.0, 0.0]),
        ("FromUpNorth", [400.0, 1400.0, 2000.0, 1.0, 1.0, -0.2066, 0.0, 0.264461]),
        ("Infiltrator", [200.0, 1370.0, 2000.0, 1.24, 1.06, 1.0, 0.13, 0.0]),
        ("Kid", [400.0, 1400.0, 2000.0, 3.0, 0.66, 1.0, 0.0, 0.0]),
        ("Male", [400.0, 1400.0, 2000.0, 0.6, 1.05, 1.0, 0.0, 0.0]),
        ("Marine", [388.0, 1155.0, 1848.0, 0.8, 1.08, 0.54, 0.0, 0.0]),
        ("Neutral", [400.0, 1400.0, 2000.0, 1.0, 1.0, 1.0, 0.0, 0.0]),
        ("Orc", [468.0, 1321.0, 2200.0, 0.66, 1.5, 1.0, 0.0, 0.0]),
        ("Robot", [400.0, 1400.0, 2000.0, 1.394, 1.0, -0.00824, 0.0, 0.0]),
        ("Unstable", [400.0, 1400.0, 2000.0, 2.37, 0.92, 2.5, 0.0, 0.0]),
    ];

    /// Echte Mikrofon-Equalizer-Presets aus `MicEqProfile\*.json`. Alle
    /// PreAmp-Werte im Original sind 0.0, deshalb hier nicht separat
    /// geführt. Bandreihenfolge 31Hz..16kHz = MIC_EQ_PROFILE_IDS-Reihenfolge.
    pub const MIC_EQ_PRESETS: [(&str, [f32; 10]); 9] = [
        ("Balanced", [0.0, 0.0, 2.0, 2.0, 3.0, -4.0, 2.0, 4.0, 0.0, 0.0]),
        ("Broadcast", [0.0, 0.0, 2.0, 2.0, -2.0, 0.0, -4.0, 4.0, 0.0, 0.0]),
        ("Communication", [-2.0, -3.0, 2.0, 4.0, 4.0, 0.0, -3.0, 3.0, 0.0, 0.0]),
        ("PitchBoost", [0.0, 3.0, -2.0, -4.0, -4.0, -2.0, 5.0, 7.0, 0.0, 0.0]),
        ("PitchReduction", [0.0, 0.0, 2.0, 2.0, -2.0, 0.0, -4.0, 4.0, 0.0, 0.0]),
        ("PublicAnnouncement", [-3.0, -5.0, 0.0, 4.0, 4.0, -3.0, 0.0, 0.0, 0.0, 0.0]),
        ("Recording", [-2.0, -3.0, 3.0, 4.0, 4.0, 0.0, 3.0, 2.0, 0.0, 0.0]),
        ("VoiceClarity", [-3.0, -4.0, 0.0, 2.0, 4.0, -2.0, 2.0, 4.0, 0.0, 0.0]),
        ("VoiceWarmth", [-2.0, -3.0, 3.0, 4.0, 4.0, -4.0, 3.0, 2.0, 0.0, 0.0]),
    ];
}

// ---------------------------------------------------------------------------
// Decoder (Dolby Digital, Namespace::Decoder)
// ---------------------------------------------------------------------------
pub mod decoder {
    use super::{LevelParam, Namespace};

    pub const MODE: LevelParam = LevelParam { namespace: Namespace::Decoder, param_id: 0x02 };

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum DecoderMode {
        Normal,
        Voll,
        Nacht,
    }
    impl DecoderMode {
        pub fn as_value(self) -> f32 {
            match self {
                DecoderMode::Normal => 2.0,
                DecoderMode::Voll => 1.0,
                DecoderMode::Nacht => 3.0,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sonderfälle außerhalb des Toggle/Level-Schemas
// ---------------------------------------------------------------------------
pub mod special {
    /// Scout Mode ein/aus. Löst bei "ein" zusätzlich sbx_sync() aus
    /// (das vom Nutzer beobachtete "SBX leuchtet blau auf").
    pub fn scout_mode(on: bool) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0] = 0x5a;
        buf[1] = 0x26;
        buf[2] = 0x05;
        buf[3] = 0x07;
        buf[4] = 0x02;
        buf[5] = 0x00;
        buf[6] = if on { 0x01 } else { 0x00 };
        buf
    }

    /// Generischer SBX-Sync/Toggle-Befehl. Mode (ein/aus, Sound-Modus-Kachel)
    /// und SBX sind praktisch derselbe zugrundeliegende Zustand (invers/exklusiv) –
    /// dieser eine Befehl schaltet beide gemeinsam.
    pub fn sbx_sync() -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0] = 0x5a;
        buf[1] = 0x26;
        buf[2] = 0x03;
        buf[3] = 0x08;
        buf[4] = 0xff;
        buf[5] = 0xff;
        buf
    }

    /// Fragt den aktuellen Kopfhörer/Lautsprecher-Schalterstand ab (GET,
    /// kein SET – der Schalter selbst ist rein hardwareseitig und nicht
    /// per USB steuerbar). Per Windows-Capture "g8 neu.pcapng" UND
    /// eigenem Live-Test am echten Gerät exakt bestätigt: das Senden
    /// dieses Reports löst umgehend eine Antwort auf dem Interrupt-IN-
    /// Kanal aus, Format "5a 2c 05 01 <02|04>" – 0x04 vermutlich
    /// Kopfhörer, 0x02 Lautsprecher (siehe output_target_from_reply).
    /// Muss aktiv gepollt werden, es gibt keine Push-Benachrichtigung bei
    /// Schalterwechsel.
    pub fn query_output_target() -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0] = 0x5a;
        buf[1] = 0x2c;
        buf[2] = 0x01;
        buf[3] = 0x01;
        buf
    }

    /// Liest aus der Antwort auf query_output_target() den Zielwert
    /// (analog zum output-target-Property: 0=Kopfhörer, 1=Lautsprecher).
    /// None, falls der Report kein Treffer auf "5a 2c 05 01 xx" ist.
    pub fn output_target_from_reply(buf: &[u8; 64]) -> Option<i32> {
        if buf[0] == 0x5a && buf[1] == 0x2c && buf[2] == 0x05 && buf[3] == 0x01 {
            Some(if buf[4] == 0x04 { 0 } else { 1 })
        } else {
            None
        }
    }

    /// Schaltet den physischen Kopfhörer/Lautsprecher-Schalter per Software
    /// um – per Windows-Capture ("umschalten in software_hp-spk-und
    /// zurück.pcapng") UND Live-Test am echten Gerät bestätigt (hörbares
    /// Relais-Klicken, LED wechselt). Byte[3]=0x00 markiert den Befehl
    /// (Host→Gerät), das Gerät bestätigt mit demselben Report aber
    /// Byte[3]=0x01 (siehe output_target_from_reply/query_output_target)
    /// – frühere Versuche mit Byte[3]=0x01 als Befehl waren der Fehler,
    /// deshalb schien der Schalter lange nicht fernsteuerbar.
    pub fn set_output_target(headphones: bool) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0] = 0x5a;
        buf[1] = 0x2c;
        buf[2] = 0x05;
        buf[3] = 0x00;
        buf[4] = if headphones { 0x04 } else { 0x02 };
        buf
    }

    /// Geräte-/Einstellungen-Befehlsfamilie (eigenes Schema, nicht das
    /// Toggle/Level-Schema von build_toggle_report). Per Windows-Capture
    /// bestätigt für mehrere zuvor unbekannte Einstellungen.
    pub fn device_setting(setting_id: u8, on: bool) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0] = 0x5a;
        buf[1] = 0x39;
        buf[2] = 0x03;
        buf[3] = 0x00;
        buf[4] = setting_id;
        buf[5] = if on { 0x01 } else { 0x00 };
        buf
    }

    /// IDs für device_setting(), per Windows-Capture bestätigt.
    pub mod setting_id {
        pub const DIRECT_MODE: u8 = 0x05;
        pub const SPDIF_OUT_DIRECT_MODE: u8 = 0x0d;
        pub const LOGO_LED: u8 = 0x0e;
        pub const POWER_SAVING: u8 = 0x11;
        pub const VOLUME_SYNC: u8 = 0x1c;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surround_on_report() {
        let r = build_toggle_report(acoustic_engine::SURROUND_TOGGLE, true);
        assert_eq!(&r[0..6], &[0x5a, 0x12, 0x07, 0x01, 0x96, 0x00]);
        assert_eq!(&r[6..10], &1.0f32.to_le_bytes());
    }

    #[test]
    fn scout_on_report() {
        let r = special::scout_mode(true);
        assert_eq!(&r[0..7], &[0x5a, 0x26, 0x05, 0x07, 0x02, 0x00, 0x01]);
    }

    #[test]
    fn decoder_mode_report() {
        let r = build_level_report(decoder::MODE, decoder::DecoderMode::Nacht.as_value());
        assert_eq!(&r[0..6], &[0x5a, 0x12, 0x07, 0x01, 0x97, 0x02]);
        assert_eq!(&r[6..10], &3.0f32.to_le_bytes());
    }
}
