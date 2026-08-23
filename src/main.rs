mod device;
mod mixer;
mod protocol;
mod speaker_test;
mod virtual_surround;

use device::G8Device;
use protocol::{acoustic_engine, build_level_report, build_report, build_toggle_report, decoder, equalizer, special, Namespace};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;

    let device = match G8Device::open() {
        Ok(d) => {
            ui.set_device_connected(true);
            Some(Rc::new(d))
        }
        Err(e) => {
            eprintln!("G8 nicht verbunden, UI läuft ohne Hardware-Anbindung: {e}");
            ui.set_device_status(format!("✗ {e}").into());
            None
        }
    };

    {
        let device = device.clone();
        ui.on_playback_param_changed(move |id, value| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = build_report(Namespace::Playback, id as u8, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    {
        let device = device.clone();
        ui.on_recording_param_changed(move |id, value| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = build_report(Namespace::Recording, id as u8, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    {
        // Equalizer liegt ebenfalls im Playback-Namespace (0x96), siehe
        // protocol.rs::equalizer.
        let device = device.clone();
        ui.on_eq_param_changed(move |id, value| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = build_report(Namespace::Playback, id as u8, value);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    {
        let device = device.clone();
        ui.on_scout_toggled(move |on| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
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
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            // Direct Mode und SBX sind derselbe zugrundeliegende Zustand,
            // invers zueinander (Direct Mode an = SBX-Effekte aus). Per
            // USB-Capture "g8 neu.pcapng" UND Live-Test bestätigt: die
            // Original-Software schaltet dabei 6 Acoustic-Engine-Toggles
            // gemeinsam um (Surround, Dialog+, Smart Volume, Crystalizer,
            // Equalizer, Bass) statt eines einzelnen Masterschalter-Bytes.
            let sbx_on = !on;
            let toggles = [
                acoustic_engine::SURROUND_TOGGLE,
                acoustic_engine::DIALOG_PLUS_TOGGLE,
                acoustic_engine::SMART_VOLUME_TOGGLE,
                acoustic_engine::CRYSTALIZER_TOGGLE,
                acoustic_engine::BASS_TOGGLE,
                equalizer::TOGGLE,
            ];
            for toggle in toggles {
                if let Err(e) = dev.send_report(&build_toggle_report(toggle, sbx_on)) {
                    eprintln!("Senden fehlgeschlagen: {e}");
                }
            }
        });
    }

    {
        let device = device.clone();
        ui.on_decoder_mode_changed(move |mode| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
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

    ui.on_mixer_level_changed(move |entity_id, value| {
        mixer::set_level(entity_id, value);
    });

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
    {
        let device = device.clone();
        ui.on_output_target_changed(move |target| {
            if target == 0 {
                virtual_surround::enable();
            } else {
                virtual_surround::disable();
            }
            let Some(dev) = &device else {
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
    // geteiltes Handle würde die UI beim Schreiben blockieren.
    if device.is_some() {
        let weak = ui.as_weak();
        thread::spawn(move || {
            let dev = match G8Device::open() {
                Ok(d) => d,
                Err(e) => return eprintln!("Schalter-Abfrage: Gerät nicht geöffnet: {e}"),
            };
            let mut last_target: Option<i32> = None;
            loop {
                // Erst liegen gebliebene Reports verwerfen (z.B. vom
                // periodischen Status-Heartbeat, der zufällig dieselbe
                // "5a 2c 05 01 xx"-Form hat) – sonst wird eine alte,
                // ungültige Antwort mit der frischen Anfrage verwechselt.
                while dev.read_report(20).ok().flatten().is_some() {}

                if dev.send_report(&special::query_output_target()).is_ok() {
                    if let Ok(Some(buf)) = dev.read_report(300) {
                        if let Some(target) = special::output_target_from_reply(&buf) {
                            if last_target != Some(target) {
                                last_target = Some(target);
                                let weak = weak.clone();
                                let _ = weak.upgrade_in_event_loop(move |ui| ui.set_output_target(target));
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
    }

    // Volume Sync (Einstellungen-Reiter): spiegelt den Lautstärkeknopf
    // zusätzlich auf die PipeWire-Sink-Lautstärke (die separate
    // Software-Pegelstufe oberhalb der ALSA-Hardwarelautstärke, die
    // set_level/get_level ansprechen – vergleichbar mit "PC-Lautstärke"
    // unter Windows). Original-Software verlangt vor dem Einschalten den
    // Regler auf 50%, sonst springt die PC-Lautstärke beim Aktivieren
    // sofort auf den aktuellen Reglerstand – das bilden wir hier 1:1 nach.
    let volume_sync_enabled = Arc::new(AtomicBool::new(false));
    {
        let flag = volume_sync_enabled.clone();
        let device = device.clone();
        ui.on_volume_sync_toggled(move |on| {
            flag.store(on, Ordering::SeqCst);
            if on {
                if let Some(v) = mixer::get_level(0) {
                    mixer::set_pipewire_sink_volume(v);
                }
            }
            // Per Windows-Capture bestätigt: eigener Geräte-Befehl
            // (setting_id::VOLUME_SYNC) zusätzlich zu unserem PipeWire-
            // Mirroring – was er intern genau bewirkt, ist unklar, aber
            // die Original-Software sendet ihn beim Umschalten.
            if let Some(dev) = &device {
                let report = special::device_setting(special::setting_id::VOLUME_SYNC, on);
                if let Err(e) = dev.send_report(&report) {
                    eprintln!("Senden fehlgeschlagen: {e}");
                }
            }
        });
    }

    // Der Lautstärkeknopf am Gerät schreibt direkt in dasselbe ALSA-
    // Feature-Unit-Register wie unser Slider (per USB-Capture bestätigt,
    // siehe HANDOFF.md) – es gibt aber keine Push-Benachrichtigung dafür,
    // nur Polling macht den Regler live mitlaufen.
    {
        let weak = ui.as_weak();
        let volume_sync_enabled = volume_sync_enabled.clone();
        thread::spawn(move || {
            let mut last_speaker: Option<f32> = None;
            let mut last_mic: Option<f32> = None;
            loop {
                thread::sleep(Duration::from_millis(120));

                if let Some(v) = mixer::get_level(0) {
                    let changed = last_speaker.map(|prev| (prev - v).abs() > 0.5).unwrap_or(true);
                    if changed {
                        last_speaker = Some(v);
                        if volume_sync_enabled.load(Ordering::SeqCst) {
                            mixer::set_pipewire_sink_volume(v);
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
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
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

    // Reset ist weiterhin unbekannt (kein Capture vorhanden, bewusst nicht
    // spekulativ implementiert – Risiko eines Bricking). Energiesparen und
    // Logo-LED per Windows-Capture bestätigt (special::setting_id).
    ui.on_reset_clicked(move || {
        eprintln!("Gerät zurücksetzen: Report-Format noch nicht reverse-engineert");
    });
    {
        let device = device.clone();
        ui.on_power_saving_toggled(move |on| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = special::device_setting(special::setting_id::POWER_SAVING, on);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }
    {
        let device = device.clone();
        ui.on_logo_led_toggled(move |on| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            let report = special::device_setting(special::setting_id::LOGO_LED, on);
            if let Err(e) = dev.send_report(&report) {
                eprintln!("Senden fehlgeschlagen: {e}");
            }
        });
    }

    ui.run()?;
    Ok(())
}
