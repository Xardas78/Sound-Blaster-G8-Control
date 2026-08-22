# Sound Blaster G8 – Reverse-Engineering-Ergebnisse & UI-Gerüst

Kontext für die Fortsetzung mit Claude Code. Dieses Verzeichnis enthält ein
erstes Gerüst (nicht kompiliert/getestet) für eine eigene G8-Steuersoftware,
die sich am Original-Layout orientiert.

## USB-Topologie

Die G8 zeigt sich unter Windows als **zwei getrennte USB-Geräte** an
unterschiedlichen Root-Hubs:

- `0c70:f011` – nur Status-Telemetrie (Interrupt-IN), kein Schreibkanal
- `041e:329b` – der eigentliche Befehlskanal (matched die Linux-Composite-ID)

Composite-Device `041e:329b` hat u. a.:

- **Interface 0**: UAC2 Audio Control (Feature Units, Clock Source, Extension
  Units) – Standard-Lautstärke, Mixer, SBX-Extension-Unit
- **Interface 1**: UAC2 Audio Streaming (Wiedergabe) – Alt-Settings 0=Idle,
  1=24-Bit (Paketgröße 294), 2=32-Bit (Paketgröße 392)
- **Interface 3/4**: HID – Interface 4 ist der proprietäre Vendor-Kanal

**Wichtig:** Ein einzelner USBPcap-Adapter fängt nur eine der beiden
USB-Seiten ein. Für vollständige Captures müssen mehrere/alle
USBPcap-Adapter gleichzeitig mitgeschnitten werden.

## Proprietärer Befehlskanal

HID **SET_REPORT** (Output Report, ReportID 0) per **Control-Transfer** an
Interface 4 (wIndex 4), 64-Byte-Payload. Kein separater Interrupt-OUT-Endpoint.

Byte-Layout (siehe `src/protocol.rs` für die vollständige, typisierte Fassung):

```
[0]     0x5a  Report-Header
[1]     0x12  Kommando (Set Parameter)
[2]     0x07  fest
[3]     0x01  fest
[4]     Namespace: 0x95=Aufnahme, 0x96=Wiedergabe, 0x97=Decoder
[5]     Parameter-ID
[6..10] Float32 little-endian Wert (Toggle: 0.0=aus, 1.0=ein)
```

### Vollständiger Parameter-Katalog

**Acoustic Engine (Wiedergabe, 0x96):**

Level-Werte sind Floats **0.0–1.0** (per Capture "acoustic engine.pcapng"
bestätigt), nicht 0–100 – die UI zeigt 0–100 an und teilt vor dem Senden
durch 100.

| Feature | Toggle | Level |
|---|---|---|
| Surround | 0x00 | 0x01 |
| Dialog+ | 0x02 | 0x03 |
| Smart Volume | 0x04 | 0x05 (Modus 0x06: Nacht=2.0/Laut=1.0/Normal=0.0) |
| Crystalizer | 0x07 | 0x08 |
| Bass | 0x18 | 0x19 |

**Equalizer (Wiedergabe, 0x96):**

- Toggle = 0x09
- Vorverstärkung = 0x0a (separat, größerer Wertebereich inkl. negativ)
- 10 Bänder = 0x0b–0x14
- "Bass"/"Höhen"-Kurzregler im Original haben keine eigenen IDs, docken an
  vorhandene Bänder an

**CrystalVoice (Aufnahme, 0x95):**

| Feature | Toggle | Level/Sonstiges |
|---|---|---|
| Noise Reduction | 0x04 | 0x05 |
| Acoustic Echo Cancellation | 0x00 | – |
| Voice Morph | 0x2c | Profilblock 0x0a–0x12 (9 Werte je Charakter) |
| Mikrofon-Equalizer | 0x13 | Profilblock 0x14–0x1d (10 Werte je Preset) |
| Smart Volume (Mic) | **nicht bestätigt** | – |

**Decoder (0x97):**

- Parameter 0x02: Normal=2.0, Nacht=3.0, Voll=1.0

**Scout Mode** (eigenes Format, nicht Toggle/Level-Schema):

```
5a 26 05 07 02 00 01   Scout ein
5a 26 05 07 02 00 00   Scout aus
```

Scout ein löst zusätzlich den generischen SBX-Sync-Befehl aus (siehe unten).

**Mode ↔ SBX** (praktisch derselbe Zustand, invers/exklusiv geschaltet):

```
5a 26 03 08 ff ff      Toggle (schaltet SBX/Mode gemeinsam)
```

## Standard-USB-Audio-Class (kein Reverse Engineering nötig)

- **Lautstärke**: normale UAC Feature-Unit SET_CUR/GET_CUR
- **Mixer**: 7 Feature-Unit-Entities (0x03, 0x04, 0x05, 0x06, 0x09, 0x0a, 0x0c),
  je einer pro Regler (Wiedergabe/Überwachung/Aufnahme)
- **Bit-Tiefe**: SET_INTERFACE mit Alt-Setting auf Interface 1 (0=Idle,
  1=24-Bit, 2=32-Bit)
- **Abtastrate**: vermutlich Clock-Source-SET_CUR, in Captures nicht erfasst
  (wahrscheinlich nur einmalig beim Stream-Start gesetzt) – noch zu klären
- **Mic-Jack-Sensing**: UAC2 Interrupt Data Message auf EP 0x84, komplett
  automatisch, keine eigene Implementierung nötig

## Offene Punkte

- Smart Volume (Mic-Seite) eigener Toggle-Parameter noch nicht isoliert
- Direct Mode: vermutlich reines Audio-Routing/Bypass, noch nicht per Capture
  bestätigt ob und welcher `5a`-Report dabei gesendet wird
- Abtastrate/Sample-Rate-Wechsel (Clock Source) noch nicht direkt beobachtet
- Gerät-Seite (Sound-Modus-Presets speichern/laden, Firmware-Update, Volume
  Sync, Energiesparen, Logo-LED) noch nicht untersucht

## Gerüst-Struktur

```
src/protocol.rs           – Namespace/Param-Konstanten + Report-Builder (mit Tests)
ui/app-window.slint        – Hauptfenster, TabWidget-Struktur
ui/effect-slider.slint     – wiederverwendbare Toggle+Slider-Komponente
ui/tabs/acoustic-engine-tab.slint
ui/tabs/crystal-voice-tab.slint
ui/tabs/equalizer-tab.slint
ui/tabs/misc-tabs.slint    – Scout Mode, Direct Mode, Decoder
ui/tabs/mixer-tab.slint
```

Noch zu tun für ein lauffähiges Projekt:

1. `cargo new` + `Cargo.toml` mit `slint` und einer HID-Bibliothek
   (z. B. `hidapi` oder `rusb`) als Dependencies, `build.rs` für
   `slint-build`
2. USB-Öffnen/Verbinden zur G8 (VID:PID 041e:329b, Interface 4)
3. Rust-Callbacks an die Slint-Signale binden (siehe
   `AppWindow::on_playback_param_changed` etc.), die Report-Builder aus
   `protocol.rs` aufrufen und per `send_report()` schreiben
4. "Gerät"- und "Einstellungen"-Tab füllen (noch nicht untersucht)
5. Standard-UAC-Funktionen für Lautstärke/Mixer/Bit-Tiefe ergänzen (kein
   `5a`-Report, siehe oben)
