mod device;
mod mixer;
mod protocol;

use device::G8Device;
use protocol::{build_level_report, build_report, decoder, special, Namespace};
use std::rc::Rc;

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
        ui.on_direct_mode_toggled(move |_on| {
            let Some(dev) = &device else { return eprintln!("Ignoriert – kein Gerät verbunden") };
            // Direct Mode und SBX sind laut Zeichnung/Anleitung derselbe
            // zugrundeliegende Zustand (invers/exklusiv) – bislang einzig
            // bekannter Report dafür ist der gemeinsame SBX-Sync-Toggle.
            // Noch nicht per Capture bestätigt, siehe HANDOFF.md
            // "Offene Punkte".
            if let Err(e) = dev.send_report(&special::sbx_sync()) {
                eprintln!("Senden fehlgeschlagen: {e}");
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

    ui.on_capture_source_changed(move |index| {
        mixer::set_capture_source(index);
    });

    // Reset, Volume Sync, Energieeinsparung und Logo-LED sind über den
    // proprietären Kanal noch nicht reverse-engineert (kein Capture
    // vorhanden) – siehe HANDOFF.md "Offene Punkte". Controls existieren
    // im UI, lösen aber bewusst noch keinen (geratenen) Befehl aus.
    ui.on_reset_clicked(move || {
        eprintln!("Gerät zurücksetzen: Report-Format noch nicht reverse-engineert");
    });
    ui.on_volume_sync_toggled(move |_on| {
        eprintln!("Volume Sync: Report-Format noch nicht reverse-engineert");
    });
    ui.on_power_saving_toggled(move |_on| {
        eprintln!("Energieeinsparung: Report-Format noch nicht reverse-engineert");
    });
    ui.on_logo_led_toggled(move |_on| {
        eprintln!("Logo-LED: Report-Format noch nicht reverse-engineert");
    });

    ui.run()?;
    Ok(())
}
