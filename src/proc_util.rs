//! Führt externe Befehle (amixer/wpctl) mit Timeout aus statt mit dem
//! blockierenden Command::output()/status() direkt. Live beobachtet: bei
//! einem PipeWire-USB-Stall (wireplumber blockiert im Kernel in
//! usb_control_msg, siehe HANDOFF.md) hängt `wpctl` auf unbestimmte Zeit.
//! Ohne Timeout würde das unsere eigenen Hintergrund-Threads (Mixer-Poll,
//! virtueller Surround) für immer mitreißen, statt den Fehler nur zu
//! melden – kein Software-Fix für den Stall selbst (der braucht ein
//! physisches Aus-/Einstecken des Geräts), aber verhindert, dass wir ihn
//! verschlimmern oder die App komplett einfriert.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn drain<R: Read + Send + 'static>(mut r: R) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        buf
    })
}

pub fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Option<Output> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    // Ausgabe parallel auslesen: sonst blockiert das Kind beim Schreiben in
    // die volle Pipe (pactl list cards) und läuft bis zum Timeout.
    let stdout = drain(child.stdout.take()?);
    let stderr = drain(child.stderr.take()?);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Some(Output {
                    status,
                    stdout: stdout.join().unwrap_or_default(),
                    stderr: stderr.join().unwrap_or_default(),
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    eprintln!("proc_util: '{cmd} {args:?}' hängt fest, breche ab (evtl. PipeWire-USB-Stall – siehe HANDOFF.md)");
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}
