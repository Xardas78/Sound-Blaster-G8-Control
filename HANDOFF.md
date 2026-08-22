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

## Weitere Capture-Funde (Session 2)

Aus einem zweiten, gezielteren Windows-Capture ("g8 neu.pcapng", G8
eingesteckt, danach Software gestartet) und Live-Tests direkt am Gerät.

### Anfrage/Antwort-Mechanismus (GET)

Zusätzlich zum bekannten SET_REPORT-Kanal (`5a 12 07 01 ...`) gibt es
GET-artige Anfragen: Host schickt einen kurzen Report, Gerät antwortet
umgehend über den Interrupt-IN-Kanal (Endpoint 0x85, Interface 4).
Beobachtete Anfrage/Antwort-Paare (jeweils Anfrage → Antwort):

```
5a 07 01 02                                   → 5a 07 10 "1.2.250819.0903"  (Firmware-Version, ASCII)
5a 2c 01 01                                   → 5a 2c 05 01 <02|04>         (Kopfhörer/Lautsprecher-Status)
5a 2c 01 02                                   → 5a 2c 0a 02 82 02 00 00 00 04
5a 26 01 05                                   → 5a 26 0a 05 00 03 00 01 00 02 00 11
5a 39 01 02                                   → 5a 39 09 02 20 64 02 14
5a 6c 01 02                                   → 5a 6c 0e 02 05 85 00 01 00 02 00 03 00 04 00 05
5a 10 00                                      → 5a 10 08 ef 67 74
```

`query_output_target()` in `protocol.rs` nutzt das erste Paar, um den
physischen Kopfhörer/Lautsprecher-Schalter live abzufragen (siehe
`main.rs`, Poll-Thread im Sekundentakt – der Schalter selbst sendet
KEINE Push-Benachrichtigung bei Wechsel). Die anderen Paare (0x26, 0x39,
0x6c, 0x10) sind noch nicht entschlüsselt – wirken wie weitere
Geräte-Statusabfragen (evtl. Firmware-Zustand, Profilslot, o.ä.).

**Wichtig:** Der Kopfhörer/Lautsprecher-Schalter selbst ist rein
hardwareseitig (mechanisches Relais) – per USB nur auslesbar, NICHT
fernsteuerbar. Mehrfach am echten Gerät getestet: Senden von
`5a 2c 05 01 <02|04>` als Befehl (in der Annahme, es sei ein SET) löst
keinerlei Reaktion aus (kein Klick, keine LED-Änderung, kein
Klangunterschied).

### Periodischer Status-Heartbeat (Fehlinterpretation korrigiert)

`5a 2c 05 01 <02|04>` taucht auch unaufgefordert auf dem Interrupt-IN-
Kanal auf – wurde zunächst fälschlich als Push-Benachrichtigung des
physischen Schalters interpretiert. Tatsächlich sendet das Gerät einen
kompletten Status-Heartbeat (Acoustic Engine/CrystalVoice/EQ-Werte,
inkl. dieses Blocks) ca. alle 1,5–2,5s, sobald es durch irgendeinen
Befehl "aktiv" wurde – unabhängig vom Schalter. Deshalb: für den
Schalterstand aktiv per `query_output_target()` pollen, nicht auf
diesen Heartbeat verlassen.

### SBX-Masterschalter (Direct Mode)

Windows schaltet beim Direct-Mode/SBX-Toggle 6 Acoustic-Engine-Parameter
gemeinsam um (alle auf `0.0` bzw. `1.0`), statt eines einzelnen
Masterschalter-Bytes:

```
Surround (0x00), Dialog+ (0x02), Smart Volume (0x04),
Crystalizer (0x07), Equalizer (0x09), Bass (0x18)
```

Bestätigt per Capture UND Live-Test (Batch-Reports `5a 11 2c 07 00 ...`
bzw. `5a 11 26 06 00 ...`). In `main.rs` als 6 einzelne
`build_toggle_report()`-Aufrufe nachgebildet (funktional identisch,
nicht Byte-für-Byte das Original-Batch-Format).

## Offene Punkte

- Smart Volume (Mic-Seite) eigener Toggle-Parameter noch nicht isoliert
- Abtastrate/Sample-Rate-Wechsel (Clock Source) noch nicht direkt beobachtet
- Gerät-Seite: Sound-Modus-Presets speichern/laden weiterhin unbekannt (Tab
  deshalb entfernt), ebenso Firmware-Update-Check, Energiesparen, Logo-LED,
  Gerät-zurücksetzen – Controls existieren im UI, lösen aber keinen
  (geratenen) Befehl aus
- Scout-Mode-Zugriffstaste (Hotkey) ist nur UI-vorbereitet, kein echter
  globaler Tastatur-Hook angebunden
- Die vier neuen GET-Anfrage-Familien (0x26, 0x39, 0x6c, 0x10 – siehe oben)
  sind unentschlüsselt; könnten weitere auslesbare Gerätezustände sein
- Voice-Morph-/Mikrofon-Equalizer-Profillisten in crystal-voice-tab.slint
  sind teilweise geraten (nur "Roboter"/"Pitch-Reduzierung" aus Screenshot
  bestätigt)

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
