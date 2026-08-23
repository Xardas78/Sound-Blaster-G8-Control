# Sound Blaster G8 – Reverse-Engineering: Vollständiger Wissensstand

Kontext für die Fortsetzung mit Claude Code. Fasst alles zusammen, was per
USB-Traffic-Analyse (USBPcap unter Windows + Live-Tests am echten Gerät
unter Linux) über die Sound Blaster G8 (USB-ID `041e:329b`) herausgefunden
wurde.

## 1. USB-Topologie

Die G8 zeigt sich unter Windows als **zwei getrennte USB-Geräte** an
unterschiedlichen Root-Hubs (physisch bedingt durch die Dual-USB-Bauweise):

- `0c70:f011` – nur Status-Telemetrie (Interrupt-IN), kein Schreibkanal
- `041e:329b` – der eigentliche Befehlskanal, matched die Linux-Composite-ID

**Wichtig für eigene Captures:** Ein einzelner USBPcap-Adapter fängt nur
eine der beiden USB-Seiten ein. Für vollständige Captures müssen mehrere/
alle USBPcap-Adapter gleichzeitig mitgeschnitten werden.

`041e:329b` hat u. a.:

- **Interface 0**: UAC2 Audio Control (Feature Units, Clock Source,
  Extension Units) – Standard-Lautstärke, Mixer, SBX-Extension-Unit. Unter
  Linux generischer UAC2-Support über ALSA, funktioniert ohne eigenen Code.
- **Interface 1**: UAC2 Audio Streaming (Wiedergabe) – Alt-Settings 0=Idle,
  1=24-Bit (Paketgröße 294), 2=32-Bit (Paketgröße 392)
- **Interface 3**: HID, Consumer-Control (EP 0x83, 16 Byte) – ungenutzt
- **Interface 4**: HID, der proprietäre Vendor-Kanal (EP 0x85, 64 Byte)

## 2. Proprietärer Befehlskanal

HID **SET_REPORT** (Output Report, ReportID 0) per **Control-Transfer** an
Interface 4 (wIndex 4), 64-Byte-Payload, über den Control-Endpoint 0x00.
Antworten/Statusmeldungen kommen über den **Interrupt-IN-Endpoint 0x85**
auf demselben Interface zurück (siehe `device.rs::read_report`).

### Haupt-Byte-Layout (Toggle/Level-Parameter)

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

**Acoustic Engine (Wiedergabe, Namespace 0x96):** Level-Werte sind Floats
**0.0–1.0** (per Capture bestätigt), nicht 0–100 – die UI zeigt 0–100 an
und teilt vor dem Senden durch 100.

| Feature | Toggle | Level | Sonstiges |
|---|---|---|---|
| Surround | 0x00 | 0x01 | |
| Dialog+ | 0x02 | 0x03 | |
| Smart Volume | 0x04 | 0x05 | Modus 0x06: Nacht=2.0/Laut=1.0/Normal=0.0 |
| Crystalizer | 0x07 | 0x08 | |
| Bass | 0x18 | 0x19 | |
| Equalizer | 0x09 | – | Vorverstärkung=0x0a, 10 Bänder=0x0b–0x14 |
| Crossover-Frequenz | – | 0x17 | Wert direkt in Hz als Float |

**Wiedergabe-Equalizer, Details:** 10 Bänder von **31 Hz bis 16 kHz** (31,
62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000 Hz), Parameter `0x0b`–
`0x14`. **Bug gefunden und gefixt:** eine frühere Version hatte nur 9
Bänder ab 62 Hz implementiert (`0x0b`–`0x13`) – das unterste 31-Hz-Band
(`0x14`) fehlte komplett, "Bass"/"Höhen"-Kurzregler zeigten dadurch auf
die falschen Bänder. Jetzt korrekt: "Bass" dockt an Band 0 (31 Hz),
"Höhen" an Band 9 (16 kHz).

**CrystalVoice (Aufnahme, Namespace 0x95):**

| Feature | Toggle | Sonstiges |
|---|---|---|
| Acoustic Echo Cancellation | 0x00 | |
| Noise Reduction | 0x04 | Level=0x05 |
| Voice Morph | 0x0a | Profilblock 0x0b–0x12 (8 Werte je Charakter) |
| Mikrofon-Equalizer | 0x13 | Profilblock 0x14–0x1d (10 Werte je Preset) |
| Smart Volume (Mic) | 0x2c | |

**Bug gefunden und gefixt:** Voice Morph und Smart Volume (Mic) waren
vertauscht – Voice Morph benutzte fälschlich `0x2c`, Smart Volume war
"nicht bestätigt". Per Capture (`mic voicemorph ein-aus.pcapng`) korrigiert.

**Decoder (Dolby Digital, Namespace 0x97):**

- Parameter `0x02`: Normal=2.0, Nacht=3.0, Voll=1.0

### Geräte-/Einstellungen-Befehlsfamilie (eigenes Schema)

```
5a 39 03 00 [Einstellungs-ID] [01=ein|00=aus]
```

| Einstellung | ID | Status |
|---|---|---|
| Direct Mode (Haupt) | 0x05 | implementiert (als 6er-Toggle-Batch, siehe unten) |
| SPDIF-Out Direct Mode | 0x0d | implementiert (Wiedergabe-Tab, nur bei Lautsprecher) |
| Logo LED | 0x0e | implementiert |
| Energiesparen | 0x11 | implementiert |
| Volume Sync | 0x1c | implementiert (zusätzlich zum PipeWire-Mirroring, siehe unten) |

Alle fünf per Windows-Capture bestätigt (`Logo ein und aus.pcapng`,
`energy save ein und aus.pcapng`, `spdif_out direct ein-aus.pcapng`,
`volsync aus und ein.pcapng`, `Directmode ein-aus.pcapng`).

Direct Mode löst beim Einschalten zusätzlich den generischen SBX-Sync-Burst
aus (Direct Mode umgeht alle Effekte, deaktiviert daher SBX). Was genau
"Volume Sync" geräteseitig bewirkt, ist unklar (Windows-spezifisches
Konzept "PC-Lautstärke" hat unter Linux keine 1:1-Entsprechung) – wir
senden den Befehl trotzdem mit, zusätzlich zu unserem eigenen
PipeWire-Mirroring (siehe main.rs).

### Kopfhörer/Lautsprecher-Umschaltung – jetzt vollständig gelöst

```
5a 2c 05 00 <02|04>    Befehl (Host→Gerät): 02=Lautsprecher, 04=Kopfhörer
5a 2c 05 01 <02|04>    Bestätigung (Gerät→Host), identisches Format, Byte[3]=0x01
```

**Wichtige Geschichte:** Über mehrere Sessions wurde dieser Schalter erst
für "nicht fernsteuerbar, nur auslesbar" gehalten – Grund war ein einzelnes
falsches Byte. Frühere Versuche sendeten `5a 2c 05 01 <wert>` (Byte[3]=0x01,
das ist aber das Antwort-Format!) statt `5a 2c 05 00 <wert>`
(Byte[3]=0x00, das echte Befehls-Format). Mit dem richtigen Byte
funktioniert die Fernsteuerung einwandfrei – bestätigt per Windows-Capture
(`umschalten in software_hp-spk-und zurück.pcapng`) UND Live-Test am
echten Gerät (hörbares Relais-Klicken, LED-Wechsel). `special::
set_output_target()` sendet den Befehl, `special::query_output_target()`
+ `output_target_from_reply()` liest den Status aktiv aus (kein
Push-Event bei Wechsel, siehe unten).

Löst danach im Original einen vollen Parameter-Scan aus (Kopfhörer/
Lautsprecher haben unterschiedliche EQ-Profile) – bei uns nicht
nachgebildet (würde eigene Nutzereinstellungen überschreiben).

### Periodischer Status-Heartbeat

`5a 2c 05 01 <02|04>` taucht auch unaufgefordert auf dem Interrupt-IN-
Kanal auf, unabhängig vom Schalter – das Gerät sendet einen kompletten
Status-Heartbeat (Acoustic Engine/CrystalVoice/EQ-Werte) ca. alle 1,5–2,5s,
sobald es durch irgendeinen Befehl "aktiv" wurde. Für den echten
Schalterstand deshalb aktiv pollen (`query_output_target()`, main.rs
Sekundentakt), nicht auf den Heartbeat verlassen.

### SBX-Masterschalter (Direct Mode)

Windows schaltet beim Direct-Mode/SBX-Toggle 6 Acoustic-Engine-Parameter
gemeinsam um (alle auf `0.0` bzw. `1.0`):

```
Surround (0x00), Dialog+ (0x02), Smart Volume (0x04),
Crystalizer (0x07), Equalizer (0x09), Bass (0x18)
```

Bestätigt per Capture UND Live-Test (Batch-Reports `5a 11 2c 07 00 ...`
bzw. `5a 11 26 06 00 ...`). In `main.rs` als 6 einzelne
`build_toggle_report()`-Aufrufe nachgebildet (funktional identisch, nicht
Byte-für-Byte das Original-Batch-Format). Zusätzlich existiert der
generische SBX-Sync-Befehl `5a 26 03 08 ff ff` (Scout Mode löst ihn z.B.
mit aus).

**Scout Mode** (eigenes Format, nicht Toggle/Level-Schema):

```
5a 26 05 07 02 00 01   Scout ein
5a 26 05 07 02 00 00   Scout aus
```

### Anfrage/Antwort-Mechanismus (GET), weitere unentschlüsselte Familien

Host schickt einen kurzen Report, Gerät antwortet umgehend über den
Interrupt-IN-Kanal. Bestätigte Paare (Anfrage → Antwort):

```
5a 07 01 02   → 5a 07 10 "1.2.250819.0903"   (Firmware-Version, ASCII)
5a 2c 01 01   → 5a 2c 05 01 <02|04>          (Kopfhörer/Lautsprecher-Status, siehe oben)
5a 2c 01 02   → 5a 2c 0a 02 82 02 00 00 00 04
5a 26 01 05   → 5a 26 0a 05 00 03 00 01 00 02 00 11
5a 39 01 02   → 5a 39 09 02 20 64 02 14      (vermutlich Direct-Mode-Status)
5a 6c 01 02   → 5a 6c 0e 02 05 85 00 01 00 02 00 03 00 04 00 05
5a 10 00      → 5a 10 08 ef 67 74
```

Nur das erste Paar (0x2c) ist bei uns angebunden. Die anderen (0x26 Rest,
0x39, 0x6c, 0x10) sind unentschlüsselt – wirken wie weitere
Geräte-Statusabfragen.

## 3. Standard-USB-Audio-Class (kein Reverse Engineering nötig)

- **Lautstärke**: normale UAC Feature-Unit SET_CUR/GET_CUR. Encoding:
  Int16 little-endian, Wert ÷ 256 = dB.
- **Mixer**: 7 Feature-Unit-Entities für 8 Regler (Wiedergabe/Überwachung/
  Aufnahme), siehe `mixer.rs::CONTROLS`.
- **Bit-Tiefe/Abtastrate**: SET_INTERFACE mit Alt-Setting. ALSA/PipeWire
  handelt das automatisch aus – kein expliziter Befehl nötig, deshalb kein
  Dropdown dafür im UI (wäre ohne Wirkung irreführend).
- **Mic-Jack-Sensing**: UAC2 Interrupt Data Message auf EP 0x84, komplett
  automatisch von ALSA gehandhabt.

## 4. Rein Software-seitige Features (kein USB-Traffic zur Hardware)

- **Kopfhörer-Konfiguration** (Stereo/5.1/7.1 virtuelles Downmixing, siehe
  `HP_Konfig.png`) und **Lautsprecherkonfiguration** (siehe
  `SPK_Konfig*.png`, inkl. "Kopfhörer-Virtualisierung auf Line-Out"-Option
  und Crossover-Frequenz bei benutzerdefiniertem Lautsprechertyp) – erzeugen
  gar keinen Traffic im 5a-Kanal. Unsere virtuelle HRTF-Surround-Funktion
  (virtual_surround.rs) deckt das konzeptionell ähnlich ab, aber eigenständig
  implementiert (PipeWire-Filter-Chain statt Windows-Downmix).
- **GameVoice Mix Knob** – rein analoger Hardware-Mixer in der G8 selbst,
  keine Software-Ansteuerung nötig/möglich.

## 5. Bewusst nicht implementiert

- **Firmware-Update**: Risiko eines Bricking bei falscher Byte-Sequenz,
  Nutzer sollen dafür die offizielle Windows-App nutzen.
- **Gerät zurücksetzen**: kein Capture vorhanden, nicht spekulativ
  implementiert.
- **Decoder-Feintuning (DRC)**: G8 zeigt laut Original-UI nur Normal/Voll/
  Nacht, kein verstecktes Protokoll zu vermuten.

## 6. Preset-Bibliothek (direkt nutzbar, kein Reverse Engineering nötig)

Die Windows-App legt ihre komplette Preset-Bibliothek als lesbare
JSON-Dateien ab – direkt für die eigene Software übernehmbar, kein
Neuaufbau nötig. Speicherorte (Windows):

- `%LOCALAPPDATA%\Creative\{GUID}\Product\SB1900\` – Nutzer-eigene/aktuell
  aktive Profile
- `C:\ProgramData\Creative\CreativeApp\Product\SB1900\` – werkseitig
  mitgelieferte Standard-Presets

**EQ-Presets** (`Presets\EQ\*.json`): Dutzende benannte Presets (Spiele,
Genres). Format: `PreAmp` + `Bands[]` mit `Frequency`+`Value` (dB).

**SoundMode-Presets** (`SoundMode\*.json`): Kombinierte Acoustic-Engine-
Profile (Surround/Crystalizer/Bass/SVM/DialogPlus + Verweis auf ein
EQ-Preset).

**Voice-Morph-Presets** (`VoiceFxProfile\*.json`, 18 Stück: Brute,
Chipmunk, DeepVoice, Demon, Dwarf, Elderly, Elf, Emo, Female, FromUpNorth,
Infiltrator, Kid, Male, Marine, Neutral, Orc, Robot, Unstable): exaktes
Mapping `Bands[Index].Value`, Index 0-7 = Parameter `0x0b`-`0x12` (z. B.
"Male" = `[400, 1400, 2000, 0.6, 1.05, 1, 0, 0]`). **Noch nicht
implementiert** – unsere Voice-Morph-Dropdown-Liste in
crystal-voice-tab.slint ist aktuell nur teilweise geraten und sendet beim
Charakterwechsel keine echten Werte. Diese JSON-Presets wären der direkte
Weg, das sauber nachzuholen.

**Mic-EQ-Presets** (`MicEqProfile\*.json`, 9 Stück: Communication,
PitchBoost, PitchReduction, PublicAnnouncement, Recording, VoiceClarity,
VoiceWarmth, Balanced, Broadcast): 10-Band, direkt nutzbar für
Parameterblock `0x14`-`0x1d`. Ebenfalls noch nicht implementiert.

## 7. Baustelle: Lautstärke-Synchronisierung

Mehrere Stellen können Lautstärke ändern (ALSA-Hardwareregister,
PipeWire-Sink-Lautstärke, unsere UI-Regler), die nicht vollständig
miteinander verknüpft sind:

- Volume Sync spiegelt aktuell nur einseitig (ALSA → PipeWire); Änderungen
  an der PC-Lautstärke über andere Wege (System-Regler, Medientasten)
  fließen nicht zurück
- Smart Volume (Acoustic Engine) hebt/senkt den Pegel nicht immer sauber
  nachvollziehbar
- Gelegentlich große, unerklärte Sprünge im Lautstärke-Poll (z.B. 44%→6%),
  größer als die normale Rasterung des physischen Knopfs erklären würde
- Footer-Regler und Mixer-Tab teilten sich lange keinen Zustand (gefixt,
  siehe Commit-Historie) – weitere Mixer-Regler (Line In, SPDIF, What U
  Hear) sind noch nicht live synchronisiert, nur Lautsprecher/externes Mic

Weitere Untersuchung mit gezielten Windows-Captures pro Einzelfunktion
geplant.

## 8. Offene Punkte

- Smart Volume Modus-Bestätigung nur für Wiedergabe-Seite, nicht Mic-Seite
- Abtastrate/Sample-Rate-Wechsel (Clock Source) noch nicht direkt beobachtet
- Sound-Modus-Presets speichern/laden im Gerät weiterhin unbekannt (Tab
  deshalb entfernt)
- Scout-Mode-Zugriffstaste (Hotkey) ist nur UI-vorbereitet, kein echter
  globaler Tastatur-Hook angebunden
- GET-Anfrage-Familien 0x26 (teilweise), 0x39, 0x6c, 0x10 unentschlüsselt
- Voice-Morph-/Mikrofon-Equalizer: echte Profilwerte aus den Windows-JSON-
  Presets (siehe Abschnitt 6) noch nicht übernommen
- Balance-Regler (L/R) im Mixer nicht implementiert (nice-to-have, gleicher
  UAC-Mechanismus wie Hauptlautstärke, nur anderer Channel)

## 9. Projektstruktur

```
src/protocol.rs            – Namespace/Param-Konstanten + Report-Builder (mit Tests)
src/device.rs               – HID-Vendor-Kanal (send_report/read_report)
src/mixer.rs                 – ALSA-Mixer-Wrapper (amixer)
src/virtual_surround.rs     – PipeWire-Filter-Chain für virtuellen HRTF-Surround
src/speaker_test.rs          – 7.1-Testtöne
ui/app-window.slint          – Hauptfenster, TabWidget-Struktur, geteilter Zustand
ui/tabs/                     – ein .slint pro Reiter
```
