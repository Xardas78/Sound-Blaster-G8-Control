// Testtöne für die 7.1-Kreisgrafik auf der Wiedergabe-Seite. Erzeugt pro
// Position ein kurzes 8-Kanal-WAV (Ton nur im angeklickten Kanal, Stille
// sonst) und spielt es über `pw-play` auf der aktuellen Standardausgabe ab.
//
// Bewusst kein festes Ziel-Sink: ist der virtuelle Surround-Sink aktiv
// (Kopfhörer, siehe virtual_surround.rs), faltet PipeWire das Signal per
// HRTF und man hört die Position tatsächlich räumlich unterschiedlich. Ist
// die G8-Hardware direkt aktiv (Lautsprecher, nur stereo), mischt PipeWires
// eigener Kanal-Downmixer die 8 Kanäle automatisch sinnvoll auf L/R – ganz
// ohne Sonderfall-Code hier.

use std::process::Command;
use std::thread;
use std::time::Duration;

const SAMPLE_RATE: u32 = 48000;
const CHANNELS: u16 = 8;
const TONE_SECS: f32 = 0.5;
const FREQ: f32 = 880.0;
const STEP_DELAY: Duration = Duration::from_millis(300);

// Reihenfolge entspricht exakt audio.position in der Filter-Chain-Config
// (siehe assets/hrir/filter-chain-template.conf): FL FR FC LFE RL RR SL SR.
pub const CHANNEL_COUNT: usize = CHANNELS as usize;

fn build_wav(channel: usize) -> Vec<u8> {
    let n_samples = (SAMPLE_RATE as f32 * TONE_SECS) as u32;
    let fade = (n_samples as f32 * 0.05).max(1.0);
    let mut data = Vec::with_capacity((n_samples * CHANNELS as u32 * 2) as usize);
    for i in 0..n_samples {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = ((i as f32).min(fade) / fade).min(((n_samples - i) as f32).min(fade) / fade);
        let sample = (t * FREQ * std::f32::consts::TAU).sin() * env * 0.6;
        let value = (sample * i16::MAX as f32) as i16;
        for c in 0..CHANNELS as usize {
            let v = if c == channel { value } else { 0 };
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    wrap_wav(&data)
}

fn wrap_wav(data: &[u8]) -> Vec<u8> {
    let byte_rate = SAMPLE_RATE * CHANNELS as u32 * 2;
    let block_align = CHANNELS * 2;
    let mut buf = Vec::with_capacity(44 + data.len());
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&CHANNELS.to_le_bytes());
    buf.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

fn play_channel_blocking(channel: usize) {
    let wav = build_wav(channel);
    let path = std::env::temp_dir().join(format!("g8-speaker-test-{channel}.wav"));
    if std::fs::write(&path, &wav).is_err() {
        return;
    }
    let _ = Command::new("pw-play").arg(&path).status();
    let _ = std::fs::remove_file(&path);
}

pub fn test_position(channel: usize) {
    if channel >= CHANNEL_COUNT {
        return;
    }
    thread::spawn(move || play_channel_blocking(channel));
}

pub fn test_cycle() {
    thread::spawn(|| {
        for channel in 0..CHANNEL_COUNT {
            play_channel_blocking(channel);
            thread::sleep(STEP_DELAY);
        }
    });
}
