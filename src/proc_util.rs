//! Führt externe Befehle (amixer/wpctl) mit Timeout aus statt mit dem
//! blockierenden Command::output()/status() direkt. Live beobachtet: bei
//! einem PipeWire-USB-Stall (wireplumber blockiert im Kernel in
//! usb_control_msg, siehe HANDOFF.md) hängt `wpctl` auf unbestimmte Zeit.
//! Ohne Timeout würde das unsere eigenen Hintergrund-Threads (Mixer-Poll,
//! virtueller Surround) für immer mitreißen, statt den Fehler nur zu
//! melden – kein Software-Fix für den Stall selbst (der braucht ein
//! physisches Aus-/Einstecken des Geräts), aber verhindert, dass wir ihn
//! verschlimmern oder die App komplett einfriert.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Option<Output> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if start.elapsed() > timeout {
                    eprintln!("proc_util: '{cmd} {args:?}' hängt fest, breche ab (evtl. PipeWire-USB-Stall – siehe HANDOFF.md)");
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}
