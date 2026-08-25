mod audio_profile;
mod autostart;
mod device;
mod firmware;
mod mixer;
mod persist;
mod proc_util;
mod protocol;
mod speaker_test;
mod virtual_surround;

use device::G8Device;
use protocol::{acoustic_engine, build_level_report, build_report, build_toggle_report, crystal_voice, decoder, equalizer, sound_mode, special, Namespace};
use slint::{ModelRc, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());

    // Rc<RefCell<...>> statt eines einfachen Option<Rc<G8Device>>: das
    // Handle muss zur Laufzeit ausgetauscht werden können, wenn das Gerät
    // die Verbindung verliert und später wiederkommt (z.B. nach "Gerät
    // zurücksetzen" – das löst eine echte USB-Neuenumeration aus, siehe
    // HANDOFF.md Abschnitt 5a). Alle Callback-Handler lesen den aktuellen
    // Stand über `device.borrow().clone()` statt eine einmal gecachte
    // Kopie zu behalten, siehe reconnect_timer unten.
    let device: Rc<RefCell<Option<Rc<G8Device>>>> = Rc::new(RefCell::new(match G8Device::open() {
        Ok(d) => {
            ui.set_device_connected(true);
            ui.set_serial_number(d.serial_number().unwrap_or_default().into());
            Some(Rc::new(d))
        }
        Err(e) => {
            eprintln!("G8 nicht verbunden, UI läuft ohne Hardware-Anbindung: {e}");
            ui.set_device_status(format!("✗ {e}").into());
            None
        }
    }));

    // Reconnect-Timer: versucht alle 2s neu zu verbinden – bewusst nicht
    // nur, wenn `device` gerade `None` ist: nach einem Reset (echte USB-
    // Neuenumeration, siehe HANDOFF.md Abschnitt 5a) bleibt das ALTE
    // Handle technisch "vorhanden", ist aber tot (`hidapi error: No such
    // device` bei jedem Zugriff) – ohne aktives Neuöffnen würde das nie
    // bemerkt. G8Device::open() ist günstig genug (ein HidApi-Scan +
    // Öffnen), um alle 2s unbedingt versucht zu werden; ein Erfolg
    // ersetzt das Handle immer. `slint::Timer` statt `thread::spawn`,
    // weil `G8Device`/`hidapi::HidDevice` nicht `Send` sind (enthalten
    // einen `Box<dyn HidDeviceBackend>` ohne `Sync`) – der Timer feuert
    // im UI-Thread, kein Problem mit Rc/RefCell.
    let reconnect_timer = slint::Timer::default();
    {
        let device = device.clone();
        let weak = ui.as_weak();
        reconnect_timer.start(slint::TimerMode::Repeated, Duration::from_secs(2), move || {
            if let Ok(d) = G8Device::open() {
                let was_connected = device.borrow().is_some();
                let serial = d.serial_number().unwrap_or_default().to_string();
                *device.borrow_mut() = Some(Rc::new(d));
                if !was_connected {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_device_connected(true);
                        ui.set_serial_number(serial.into());
                    }
                }
            }
        });
    }

    // Zuletzt gespeicherte App-seitige Einstellungen laden: Anzeige UND
    // Gerät nachziehen, sonst würde die App nach jedem Neustart wieder
    // ihre eingebauten Standardwerte zeigen, obwohl das Gerät (das wir
    // für die meisten dieser Werte nicht zurücklesen können) längst
    // andere Werte hat. Volume Sync bewusst ausgenommen, siehe persist.rs.
    let settings = Rc::new(RefCell::new(persist::load()));
    {
        let s = settings.borrow();
        ui.set_ae_surround_on(s.ae_surround_on);
        ui.set_ae_surround_level(s.ae_surround_level * 100.0);
        ui.set_ae_crystalizer_on(s.ae_crystalizer_on);
        ui.set_ae_crystalizer_level(s.ae_crystalizer_level * 100.0);
        ui.set_ae_bass_on(s.ae_bass_on);
        ui.set_ae_bass_level(s.ae_bass_level * 100.0);
        ui.set_ae_svm_on(s.ae_svm_on);
        ui.set_ae_svm_level(s.ae_svm_level * 100.0);
        ui.set_ae_svm_mode(s.ae_svm_mode);
        ui.set_ae_dialog_on(s.ae_dialog_on);
        ui.set_ae_dialog_level(s.ae_dialog_level * 100.0);
        ui.set_ae_eq_on(s.ae_eq_on);
        ui.set_ae_eq_preamp(s.ae_eq_preamp);
        ui.set_ae_eq_bands(ModelRc::new(VecModel::from(s.ae_eq_bands.to_vec())));
        ui.set_power_saving_on(s.power_saving_on);
        ui.set_logo_led_on(s.logo_led_on);
        ui.set_dark_mode_on(s.dark_mode_on);
        ui.set_close_behavior(s.close_behavior);
        ui.set_power_saving_on(s.power_saving_on);
        ui.set_logo_led_on(s.logo_led_on);
        ui.set_dark_mode_on(s.dark_mode_on);
        // Zeigt nur den zuletzt gewählten Preset-Namen wieder im Sound-
        // Modus-Dropdown an – die dahinterliegenden Acoustic-Engine-/EQ-
        // Werte sind schon über s.ae_* oben abgedeckt (on_sound_mode_-
        // preset_selected() schreibt beim Anwenden eines Presets dieselben
        // Felder mit).
        if s.sound_mode_preset_index >= 0 {
            ui.set_sound_mode_selected_index(s.sound_mode_preset_index + 1);
        }

        if let Some(dev) = device.borrow().clone() {
            let send = |report: [u8; 64]| {
                if let Err(e) = dev.send_report(&report) {
                    eprintln!("Senden fehlgeschlagen (gespeicherte Einstellungen): {e}");
                }
            };
            send(build_toggle_report(acoustic_engine::SURROUND_TOGGLE, s.ae_surround_on));
            send(build_level_report(acoustic_engine::SURROUND_LEVEL, s.ae_surround_level));
            send(build_toggle_report(acoustic_engine::CRYSTALIZER_TOGGLE, s.ae_crystalizer_on));
            send(build_level_report(acoustic_engine::CRYSTALIZER_LEVEL, s.ae_crystalizer_level));
            send(build_toggle_report(acoustic_engine::BASS_TOGGLE, s.ae_bass_on));
            send(build_level_report(acoustic_engine::BASS_LEVEL, s.ae_bass_level));
            send(build_toggle_report(acoustic_engine::SMART_VOLUME_TOGGLE, s.ae_svm_on));
            send(build_level_report(acoustic_engine::SMART_VOLUME_LEVEL, s.ae_svm_level));
            send(build_level_report(acoustic_engine::SMART_VOLUME_MODE, s.ae_svm_mode as f32));
            send(build_toggle_report(acoustic_engine::DIALOG_PLUS_TOGGLE, s.ae_dialog_on));
            send(build_level_report(acoustic_engine::DIALOG_PLUS_LEVEL, s.ae_dialog_level));
            send(build_toggle_report(equalizer::TOGGLE, s.ae_eq_on));
            send(build_level_report(equalizer::PREAMP, s.ae_eq_preamp));
            for (i, &band) in s.ae_eq_bands.iter().enumerate() {
                send(build_level_report(equalizer::band(i), band));
            }
            send(special::device_setting(special::setting_id::POWER_SAVING, s.power_saving_on));
            send(special::device_setting(special::setting_id::LOGO_LED, s.logo_led_on));
            // Ohne dieses Kommando sendet die G8 beim Drehen des Lautstärke-
            // reglers überhaupt keine HID-Reports (weder auf dem Consumer-
            // Control-Interface noch sonstwo) – live per usbmon bestätigt
            // (2026-08-24): kein einziges Paket auf dem Regler-Endpoint, bis
            // dieses Kommando gesendet wurde. Byte-identisch aus einem
            // Windows-Capture extrahiert, siehe HANDOFF.md Abschnitt 7.
            send(special::device_setting(special::setting_id::VOLUME_SYNC, true));
        }
    }
    ui.set_autostart_on(autostart::is_enabled());

    {
        let device = device.clone();
        let settings = settings.clone();
        ui.on_playback_param_changed(move |id, value| {
            // Acoustic-Engine-Parameter merken (siehe persist.rs) – deckt
            // sowohl den Acoustic-Engine-Tab als auch den Sound-Modus-Tab
            // ab, da beide über denselben Callback laufen.
            {
                let mut s = settings.borrow_mut();
                match id {
                    0 => s.ae_surround_on = value != 0.0,
                    1 => s.ae_surround_level = value,
                    2 => s.ae_dialog_on = value != 0.0,
                    3 => s.ae_dialog_level = value,
                    4 => s.ae_svm_on = value != 0.0,
                    5 => s.ae_svm_level = value,
                    6 => s.ae_svm_mode = value as i32,
                    7 => s.ae_crystalizer_on = value != 0.0,
                    8 => s.ae_crystalizer_level = value,
                    24 => s.ae_bass_on = value != 0.0,
                    25 => s.ae_bass_level = value,
                    _ => {}
                }
                persist::save(&s);
            }

            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = build_report(Namespace::Playback, id as u8, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    {
        let device = device.clone();
        ui.on_recording_param_changed(move |id, value| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = build_report(Namespace::Recording, id as u8, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    // Voice Morph / Mikrofon-Equalizer: echte Profilwerte aus den
    // Windows-JSON-Presets (siehe protocol.rs::crystal_voice), nicht mehr
    // geraten. Reihenfolge der Presets entspricht der Dropdown-Reihenfolge
    // in crystal-voice-tab.slint.
    {
        let device = device.clone();
        ui.on_voice_morph_profile_changed(move |index| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let Some((name, values)) = crystal_voice::VOICE_MORPH_PRESETS.get(index as usize) else {
                return eprintln!("Unbekannter Voice-Morph-Index {index}");
            };
            for (i, &value) in values.iter().enumerate() {
                let param = crystal_voice::VOICE_MORPH_PROFILE_IDS[i];
                if let Err(e) = dev.send_report(&build_report(Namespace::Recording, param, value)) {
                    eprintln!("Senden fehlgeschlagen ({name}): {e}");
                }
            }
        });
    }
    // Sound-Modus: kombiniertes Preset aus Acoustic Engine + Equalizer,
    // echte Werte aus SoundMode/*.json + Presets/EQ/*.json (verknüpft über
    // GraphicEQ.PresetId), siehe protocol.rs::sound_mode. bass_xover wird
    // nicht gesendet – kein bekannter Geräteparameter dafür (HANDOFF.md §8).
    // Aktualisiert zusätzlich die geteilten UI-Werte (app-window.slint),
    // damit Acoustic-Engine-/Equalizer-Tab die neuen Werte sofort zeigen,
    // statt erst beim nächsten manuellen Regler-Dreh sichtbar zu werden –
    // das Gerät selbst hat keinen Rücklese-Mechanismus dafür (HANDOFF.md
    // Abschnitt 7/8), also muss die App ihre eigene Anzeige nachziehen.
    {
        let device = device.clone();
        let settings = settings.clone();
        let weak = ui.as_weak();
        ui.on_sound_mode_preset_selected(move |index| {
            let Some((name, p)) = sound_mode::PRESETS.get(index as usize) else {
                return eprintln!("Unbekannter Sound-Modus-Index {index}");
            };
            let ui_handle = weak.unwrap();

            // Sound-Modus-Preset software-seitig merken (siehe persist.rs):
            // dieselben Felder wie playback-param-changed, damit der
            // normale Start-Wiederherstellungscode oben (s.ae_*) beim
            // nächsten App-Start automatisch dieselben Werte ans Gerät
            // sendet – kein eigener Restore-Pfad nötig.
            {
                let mut s = settings.borrow_mut();
                s.ae_surround_on = p.surround_on;
                s.ae_surround_level = p.surround_level;
                s.ae_crystalizer_on = p.crystalizer_on;
                s.ae_crystalizer_level = p.crystalizer_level;
                s.ae_bass_on = p.bass_on;
                s.ae_bass_level = p.bass_level;
                s.ae_svm_on = p.svm_on;
                s.ae_svm_level = p.svm_level;
                s.ae_svm_mode = p.svm_mode as i32;
                s.ae_dialog_on = p.dialog_on;
                s.ae_dialog_level = p.dialog_level;
                s.ae_eq_on = true;
                s.sound_mode_preset_index = index;
                persist::save(&s);
            }
            // acoustic_engine-Level-Parameter sind 0.0-1.0-Floats fürs
            // Gerät, die UI-Slider laufen aber auf 0-100 (siehe
            // acoustic-engine-tab.slint) – deshalb hier *100 für die Anzeige.
            ui_handle.set_ae_surround_on(p.surround_on);
            ui_handle.set_ae_surround_level(p.surround_level * 100.0);
            ui_handle.set_ae_crystalizer_on(p.crystalizer_on);
            ui_handle.set_ae_crystalizer_level(p.crystalizer_level * 100.0);
            ui_handle.set_ae_bass_on(p.bass_on);
            ui_handle.set_ae_bass_level(p.bass_level * 100.0);
            ui_handle.set_ae_svm_on(p.svm_on);
            ui_handle.set_ae_svm_level(p.svm_level * 100.0);
            ui_handle.set_ae_svm_mode(p.svm_mode as i32);
            ui_handle.set_ae_dialog_on(p.dialog_on);
            ui_handle.set_ae_dialog_level(p.dialog_level * 100.0);
            ui_handle.set_ae_eq_on(true);
            // Setzt NUR den Index, nicht preamp/bands direkt: das löst in
            // equalizer-tab.slint den `changed selected-preset-index`-
            // Handler aus, der apply-preset() aufruft – dieselbe Kurve wie
            // p.eq_preamp/p.eq_bands (aus genau diesem EQ-Preset abgeleitet,
            // siehe protocol.rs::sound_mode), aber zusätzlich zeigen dann
            // BEIDE Equalizer-Dropdowns (Equalizer-Tab und die
            // Equalizer-Zeile im Sound-Modus-Tab) den richtigen Namen an,
            // statt nur die Kurve ohne erkennbaren Preset-Namen zu ändern.
            ui_handle.set_ae_eq_preset_index(p.eq_preset_index);

            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let send = |report: [u8; 64]| {
                if let Err(e) = dev.send_report(&report) {
                    eprintln!("Senden fehlgeschlagen ({name}): {e}");
                }
            };
            send(build_toggle_report(acoustic_engine::SURROUND_TOGGLE, p.surround_on));
            send(build_level_report(acoustic_engine::SURROUND_LEVEL, p.surround_level));
            send(build_toggle_report(acoustic_engine::CRYSTALIZER_TOGGLE, p.crystalizer_on));
            send(build_level_report(acoustic_engine::CRYSTALIZER_LEVEL, p.crystalizer_level));
            send(build_toggle_report(acoustic_engine::BASS_TOGGLE, p.bass_on));
            send(build_level_report(acoustic_engine::BASS_LEVEL, p.bass_level));
            send(build_toggle_report(acoustic_engine::SMART_VOLUME_TOGGLE, p.svm_on));
            send(build_level_report(acoustic_engine::SMART_VOLUME_LEVEL, p.svm_level));
            send(build_level_report(acoustic_engine::SMART_VOLUME_MODE, p.svm_mode));
            send(build_toggle_report(acoustic_engine::DIALOG_PLUS_TOGGLE, p.dialog_on));
            send(build_level_report(acoustic_engine::DIALOG_PLUS_LEVEL, p.dialog_level));
            send(build_toggle_report(equalizer::TOGGLE, true));
            // Kein direkter Preamp/Bänder-Report hier mehr nötig – kommt
            // bereits über den eq-param-changed-Callback oben an, ausgelöst
            // durch set_ae_eq_preset_index().
        });
    }

    {
        let device = device.clone();
        ui.on_mic_eq_profile_changed(move |index| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let Some((name, values)) = crystal_voice::MIC_EQ_PRESETS.get(index as usize) else {
                return eprintln!("Unbekannter Mic-EQ-Index {index}");
            };
            for (i, &value) in values.iter().enumerate() {
                let param = crystal_voice::MIC_EQ_PROFILE_IDS[i];
                if let Err(e) = dev.send_report(&build_report(Namespace::Recording, param, value)) {
                    eprintln!("Senden fehlgeschlagen ({name}): {e}");
                }
            }
        });
    }

    {
        // Equalizer liegt ebenfalls im Playback-Namespace (0x96), siehe
        // protocol.rs::equalizer.
        let device = device.clone();
        let settings = settings.clone();
        ui.on_eq_param_changed(move |id, value| {
            {
                let mut s = settings.borrow_mut();
                match id {
                    9 => s.ae_eq_on = value != 0.0,
                    10 => s.ae_eq_preamp = value,
                    11..=20 => s.ae_eq_bands[(id - 11) as usize] = value,
                    _ => {}
                }
                persist::save(&s);
            }

            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = build_report(Namespace::Playback, id as u8, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    {
        let device = device.clone();
        ui.on_scout_toggled(move |on| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            if let Err(e) = dev.send_report(&special::scout_mode(on)) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
            // Scout ein löst laut Beobachtung zusätzlich den SBX-Sync aus
            // (SBX-LED springt auf Blau), siehe HANDOFF.md.
            if on {
                if let Err(e) = dev.send_report(&special::sbx_sync()) {
                    eprintln!("Senden fehlgeschlagen: {e}");
                }
            }
        });
    }

    {
        let device = device.clone();
        ui.on_direct_mode_toggled(move |on| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            // Direct Mode ist ein echtes einzelnes Hardware-Register:
            // `5a 39 03 00 05 <01|00>` (Geräte-Einstellungs-Familie,
            // setting_id::DIRECT_MODE). Per frischem Windows-Capture
            // `Directmode ein-aus.pcapng` bestätigt (2026-08-24) – derselbe
            // Befehl, den auch der physische MODE-Knopf am Gerät auslöst.
            //
            // Die frühere Nachbildung über 6 einzelne Acoustic-Engine-
            // Toggles (Surround/Dialog+/Smart Volume/Crystalizer/Bass/EQ)
            // war nicht der echte Mechanismus UND ihr Aus-Pfad schaltete
            // pauschal alle 6 Effekte EIN, auch die, die der Nutzer vorher
            // bewusst aus hatte – siehe HANDOFF-REVERTED.md für die
            // Begründung, warum sie ursprünglich eingebaut wurde (Verwechs-
            // lung mit einem separaten "SBX-Masterschalter" in der
            // Windows-Software, anderes Byte-Muster `5a 11 2c 07 00...`).
            if let Err(e) = dev.send_report(&special::device_setting(special::setting_id::DIRECT_MODE, on)) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
            // Windows sendet beim Einschalten zusätzlich den generischen
            // SBX-Sync-Burst (Direct Mode umgeht alle Effekte, deaktiviert
            // daher SBX) – siehe HANDOFF.md Abschnitt 2, analog Scout Mode.
            if on {
                if let Err(e) = dev.send_report(&special::sbx_sync()) {
                    eprintln!("Senden fehlgeschlagen: {e}");
                }
            }
        });
    }

    {
        let device = device.clone();
        ui.on_decoder_mode_changed(move |mode| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let value = match mode {
                0 => decoder::DecoderMode::Normal,
                2 => decoder::DecoderMode::Nacht,
                _ => decoder::DecoderMode::Voll,
            }
            .as_value();
            let report = build_level_report(decoder::MODE, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    // Balance ist bewusst rein lokaler UI-Zustand (siehe mixer-tab.slint),
    // deshalb hier gemeinsam als [f32; 8] gehalten statt in persist.rs: ein
    // Pegel-Wechsel (Slider) soll die zuletzt gesetzte Balance nicht
    // stillschweigend auf Mitte zurücksetzen, und umgekehrt braucht ein
    // Balance-Wechsel den zuletzt bekannten Pegel, um beide ALSA-Kanäle
    // korrekt gemeinsam zu setzen (amixer kennt keine separate
    // Balance-Control).
    let mixer_balance: Rc<RefCell<[f32; 8]>> = Rc::new(RefCell::new([0.0; 8]));

    {
        let mixer_balance = mixer_balance.clone();
        ui.on_mixer_level_changed(move |entity_id, value| {
            let balance = usize::try_from(entity_id)
                .ok()
                .and_then(|i| mixer_balance.borrow().get(i).copied())
                .unwrap_or(0.0);
            mixer::set_balance(entity_id, value, balance);
        });
    }

    {
        let mixer_balance = mixer_balance.clone();
        ui.on_mixer_balance_changed(move |entity_id, balance| {
            if let Ok(i) = usize::try_from(entity_id) {
                if let Some(slot) = mixer_balance.borrow_mut().get_mut(i) {
                    *slot = balance;
                }
            }
            let level = mixer::get_level(entity_id).unwrap_or(50.0);
            mixer::set_balance(entity_id, level, balance);
        });
    }

    ui.on_mute_changed(move |entity_id, muted| {
        mixer::set_mute(entity_id, muted);
    });

    ui.on_capture_source_changed(move |index| {
        mixer::set_capture_source(index);
    });

    // Kopfhörer/Lautsprecher: per Windows-Capture UND Live-Test am echten
    // Gerät bestätigt doch fernsteuerbar (hörbares Relais-Klicken,
    // LED-Wechsel) – der ursprüngliche Fehlschlag lag an einem falschen
    // Byte (0x01 statt 0x00), siehe special::set_output_target(). Steuert
    // zusätzlich unseren eigenen virtuellen HRTF-Surround.
    // last_output_target: verhindert einen unnötigen PipeWire-Neustart
    // (virtual_surround::enable/disable, ~1-2s), wenn output-target-changed
    // mit dem bereits aktiven Ziel feuert (z.B. Klick auf die schon
    // ausgewählte Kopfhörer/Lautsprecher-Option) – kam vor, als mehrere
    // Tabs (Sound-Modus, Acoustic Engine, Wiedergabe) denselben Umschalter
    // zeigen und jeder Klick ungefiltert den Callback auslöst. Jeder
    // zusätzliche Neustart ist ein Risikofaktor für den USB-Stall, den wir
    // live beobachtet haben (WirePlumber blockiert in usb_control_msg).
    let last_output_target = Rc::new(std::cell::Cell::new(-1i32));
    {
        let device = device.clone();
        let last_output_target = last_output_target.clone();
        ui.on_output_target_changed(move |target| {
            if last_output_target.get() == target {
                return;
            }
            last_output_target.set(target);
            if target == 0 {
                virtual_surround::enable();
            } else {
                virtual_surround::disable();
            }
            let Some(dev) = device.borrow().clone() else {
                return eprintln!("Ignoriert – kein Gerät verbunden");
            };
            if let Err(e) = dev.send_report(&special::set_output_target(target == 0)) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    // Live-Anzeige des tatsächlichen physischen Schalterstands: es gibt
    // keine Push-Benachrichtigung dafür (siehe device.rs::read_report),
    // deshalb aktiv im Sekundentakt abfragen. Eigenes, drittes Geräte-
    // Handle aus demselben Grund wie beim Lautstärke-Poller: ein
    // geteiltes Handle würde die UI beim Schreiben blockieren. Läuft
    // unabhängig vom Start-Verbindungsstatus und öffnet sich bei Bedarf
    // selbst neu (z.B. nach "Gerät zurücksetzen", siehe HANDOFF.md
    // Abschnitt 5a) – bewusst NICHT über die geteilte `device`-Zelle,
    // damit ein eigener, stabiler Handle für die read_report-Timing-
    // Logik oben erhalten bleibt.
    {
        let weak = ui.as_weak();
        thread::spawn(move || {
            let mut dev: Option<G8Device> = G8Device::open().ok();
            let mut last_target: Option<i32> = None;
            // DIAGNOSE (temporär, siehe HANDOFF.md Abschnitt 7 "Nachtrag
            // 2026-08-24: Direct-Mode-Status-Bitfeld"): Im Directmode-
            // Capture sendet das Gerät nach dem Umschalten dreimal
            // `5a 39 05 01 ...`, wovon eine Nachricht ein Byte zwischen
            // Direct-Mode an/aus ändert (0x20/0x00) – aber nur in diesem
            // einen Capture beobachtet, kein Quervergleich mit anderen
            // Einstellungen möglich. Ziel hier: alles, was mit `5a 39`
            // anfängt, mitloggen (statt wie bisher blind zu verwerfen),
            // um live zu sehen, ob/wie sich das Muster bei Logo/Energie-
            // sparen/SPDIF/Volume-Sync/physischem MODE-Knopf verhält.
            // Danach entfernen.
            let mut last_5a39: Option<[u8; 16]> = None;
            loop {
                let Some(d) = &dev else {
                    thread::sleep(Duration::from_secs(2));
                    dev = G8Device::open().ok();
                    continue;
                };
                // Erst liegen gebliebene Reports verwerfen (z.B. vom
                // periodischen Status-Heartbeat, der zufällig dieselbe
                // "5a 2c 05 01 xx"-Form hat) – sonst wird eine alte,
                // ungültige Antwort mit der frischen Anfrage verwechselt.
                // [diag] TEMPORÄR breiter gefasst: alle 5a-Reports loggen
                // (nicht nur 0x39), um zu sehen, was der physische MODE-
                // Knopf tatsächlich sendet – laut HANDOFF.md vermutlich
                // eher die bekannten "unaufgeforderten Pushes" 5a6e/5a1a
                // + Heartbeat-Dump, nicht das 5a39-Muster.
                while let Ok(Some(buf)) = d.read_report(20) {
                    if buf[0] == 0x5a {
                        let mut snap = [0u8; 16];
                        snap.copy_from_slice(&buf[0..16]);
                        if last_5a39 != Some(snap) {
                            // Param 0x70 in Namespace 0x96 ist in keiner
                            // bekannten Tabelle -- moeglicher Direct-Mode-
                            // Status-Kandidat, deshalb den Float-Wert
                            // explizit mitloggen.
                            if buf[5] == 0x96 && buf[6] == 0x70 {
                                let f = f32::from_le_bytes([buf[7], buf[8], buf[9], buf[10]]);
                                eprintln!("[diag] 96/70 (Kandidat) = {f} roh={:02x?}", &buf[7..11]);
                            }
                            eprintln!("[diag] 5a-Report: {:02x?}", &buf[0..16]);
                            last_5a39 = Some(snap);
                        }
                    }
                }

                if d.send_report(&special::query_output_target()).is_err() {
                    dev = None; // Handle tot (z.B. nach Reset) – nächste Runde neu öffnen.
                    continue;
                }
                if let Ok(Some(buf)) = d.read_report(300) {
                    if let Some(target) = special::output_target_from_reply(&buf) {
                        if last_target != Some(target) {
                            last_target = Some(target);
                            let weak = weak.clone();
                            let _ = weak.upgrade_in_event_loop(move |ui| ui.set_output_target(target));
                        }
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
    }

    // Der Lautstärkeknopf am Gerät schreibt direkt in dasselbe ALSA-
    // Feature-Unit-Register wie unser Slider (per USB-Capture bestätigt,
    // siehe HANDOFF.md) – es gibt aber keine Push-Benachrichtigung dafür,
    // nur Polling macht den Regler live mitlaufen.
    {
        let weak = ui.as_weak();
        thread::spawn(move || {
            let mut last_speaker: Option<f32> = None;
            let mut last_mic: Option<f32> = None;
            // Normalerweise spiegelt PipeWire das ALSA-Hardware-Register von
            // selbst (live verifiziert, siehe HANDOFF.md) – AUSSER die
            // G8-Karte hängt im "Pro Audio"-Profil fest (Nebenwirkung des
            // virtuellen Kopfhörer-Surrounds, siehe audio_profile.rs). Nur
            // alle ~2s neu geprüft (audio_profile-Abfrage über `pactl list
            // cards` ist spürbar teurer als amixer sget), nicht bei jedem Tick.
            let mut pro_audio_active = audio_profile::is_pro_audio_profile_active();
            let mut pro_audio_check_tick: u32 = 0;
            loop {
                thread::sleep(Duration::from_millis(120));

                pro_audio_check_tick += 1;
                if pro_audio_check_tick >= 17 {
                    pro_audio_check_tick = 0;
                    pro_audio_active = audio_profile::is_pro_audio_profile_active();
                }

                if let Some(v) = mixer::get_level(0) {
                    let changed = last_speaker.map(|prev| (prev - v).abs() > 0.5).unwrap_or(true);
                    if changed {
                        last_speaker = Some(v);
                        // Bewusst kein UNBEDINGTER eigener PipeWire-Push:
                        // live verifiziert (2026-08-23, siehe HANDOFF.md),
                        // dass PipeWire das ALSA-Hardware-Register für die G8
                        // im Normalfall bereits von selbst spiegelt (wpctl
                        // get-volume folgt dem Regler exakt, auch komplett
                        // ohne g8-control). Ein unbedingter Push (früher hier
                        // plus im inzwischen entfernten Hintergrunddienst)
                        // schrieb denselben Wert dann unnötig ein zweites
                        // Mal, mit leicht verzögerter Ablesung – das zeigte
                        // sich als kurzes "auf 0% zurückspringen". Nur im
                        // erkannten Pro-Audio-Sonderfall (s.o.) ist ein
                        // eigener Push nötig UND sicher (dort spiegelt
                        // PipeWire ohnehin nicht selbst, kein Zwei-Schreiber-
                        // Konflikt).
                        if pro_audio_active {
                            mixer::set_pipewire_sink_volume_percent(v);
                        }
                        let weak = weak.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_footer_speaker_level(v));
                    }
                }

                if let Some(v) = mixer::get_level(6) {
                    let changed = last_mic.map(|prev| (prev - v).abs() > 0.5).unwrap_or(true);
                    if changed {
                        last_mic = Some(v);
                        let weak = weak.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_footer_mic_level(v));
                    }
                }
            }
        });
    }

    {
        let device = device.clone();
        ui.on_spdif_direct_toggled(move |on| {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = special::device_setting(special::setting_id::SPDIF_OUT_DIRECT_MODE, on);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    ui.on_speaker_test_clicked(move |channel| {
        speaker_test::test_position(channel as usize);
    });

    ui.on_speaker_test_cycle_clicked(move || {
        speaker_test::test_cycle();
    });

    // Per Windows-Capture bestätigt (special::reset_device(), HANDOFF.md
    // Abschnitt 5): löst eine echte USB-Neuenumeration mit Werksdefaults
    // aus, das Gerät ist danach für mehrere Sekunden nicht erreichbar.
    // Energiesparen und Logo-LED ebenfalls per Windows-Capture bestätigt
    // (special::setting_id).
    {
        let device = device.clone();
        ui.on_reset_clicked(move || {
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            if let Err(e) = dev.send_report(&special::reset_device()) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }
    // Firmware-Check: liest die aktuelle Version vom Gerät (5a-Kanal) und
    // die neueste verfügbare Version über Creatives Update-Feed (reiner
    // HTTP-GET, siehe firmware.rs) – beides in einem Hintergrund-Thread,
    // damit ein langsames/fehlendes Netzwerk die UI nicht einfriert.
    // Nur der Versions-Check ist implementiert, kein Firmware-Flash
    // (Bricking-Risiko) – bei verfügbarem Update wird auf die offizielle
    // Windows-Software verwiesen, siehe HANDOFF.md Abschnitt 5.
    {
        let device = device.clone();
        let weak = ui.as_weak();
        ui.on_firmware_check_clicked(move || {
            if device.borrow().is_none() {
                return eprintln!("Ignoriert – kein Gerät verbunden");
            }
            let weak = weak.clone();
            // Eigenes, kurzlebiges Handle statt des geteilten `device.
            // borrow().clone()`: Rc<G8Device> ist nicht Send, kann also
            // nicht in thread::spawn hinein – gleiches Muster wie beim
            // Schalterstand-Poller weiter oben.
            thread::spawn(move || {
                let current = (|| {
                    let dev = G8Device::open().ok()?;
                    while dev.read_report(20).ok().flatten().is_some() {}
                    dev.send_report(&special::get_firmware_version()).ok()?;
                    let buf = dev.read_report(500).ok().flatten()?;
                    special::firmware_version_from_reply(&buf)
                })();
                let Some(current) = current else {
                    return eprintln!("Firmware-Check: aktuelle Version nicht auslesbar");
                };
                let latest = firmware::latest_version();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_firmware_version(current.clone().into());
                    match latest {
                        Some(latest) if latest != current => {
                            ui.set_firmware_update_available(true);
                            ui.set_firmware_latest_version(latest.into());
                            ui.set_firmware_check_failed(false);
                        }
                        Some(_) => {
                            ui.set_firmware_update_available(false);
                            ui.set_firmware_check_failed(false);
                        }
                        None => {
                            eprintln!("Firmware-Check: Update-Feed nicht erreichbar");
                            ui.set_firmware_update_available(false);
                            ui.set_firmware_check_failed(true);
                        }
                    }
                    ui.set_show_firmware_result(true);
                });
            });
        });
    }
    {
        let device = device.clone();
        let settings = settings.clone();
        ui.on_power_saving_toggled(move |on| {
            {
                let mut s = settings.borrow_mut();
                s.power_saving_on = on;
                persist::save(&s);
            }
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = special::device_setting(special::setting_id::POWER_SAVING, on);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }
    {
        let device = device.clone();
        let settings = settings.clone();
        ui.on_logo_led_toggled(move |on| {
            {
                let mut s = settings.borrow_mut();
                s.logo_led_on = on;
                persist::save(&s);
            }
            let Some(dev) = device.borrow().clone() else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = special::device_setting(special::setting_id::LOGO_LED, on);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }
    {
        let settings = settings.clone();
        ui.on_dark_mode_toggled(move |on| {
            let mut s = settings.borrow_mut();
            s.dark_mode_on = on;
            persist::save(&s);
        });
    }
    ui.on_autostart_toggled(move |on| {
        autostart::set_enabled(on);
    });
    let close_behavior = Rc::new(Cell::new(settings.borrow().close_behavior));
    {
        // Schließen-Verhalten: "Schließen" beendet das Programm richtig,
        // "In den Tray minimieren" versteckt nur das Fenster – das
        // Tray-Icon hält die Ereignisschleife am Laufen (siehe unten),
        // die App bleibt im Hintergrund aktiv (z.B. für den
        // Lautstärke-Poll-Thread). close_behavior wird zusätzlich in
        // diesem gemeinsamen Cell gehalten, weil on_close_requested()
        // unten den aktuellen Wert synchron braucht, nicht nur bei
        // Änderungen.
        let settings = settings.clone();
        let close_behavior = close_behavior.clone();
        ui.on_close_behavior_changed(move |m| {
            close_behavior.set(m);
            let mut s = settings.borrow_mut();
            s.close_behavior = m;
            persist::save(&s);
        });
    }
    {
        let weak = ui.as_weak();
        let close_behavior = close_behavior.clone();
        ui.window().on_close_requested(move || {
            if close_behavior.get() == 1 {
                if let Some(ui) = weak.upgrade() {
                    let _ = ui.hide();
                }
            } else {
                let _ = slint::quit_event_loop();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    let tray = AppTray::new()?;
    {
        let weak = ui.as_weak();
        tray.on_open_clicked(move || {
            if let Some(ui) = weak.upgrade() {
                let _ = ui.show();
            }
        });
    }
    tray.on_quit_clicked(move || {
        let _ = slint::quit_event_loop();
    });

    ui.show()?;
    tray.show()?;
    slint::run_event_loop()?;
    Ok(())
}
