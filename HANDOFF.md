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
  Int16 little-endian, Wert ÷ 256 = dB. Per Capture bestätigt (`vol
  min-max-min_mit sync.pcapng`): `GET_CUR` ist `bmRequestType=0xa1,
  bRequest=0x01, wValue=(CS=0x02 VOLUME_CONTROL)<<8|Kanal(1=L/2=R),
  wIndex=(Entity=0x01)<<8|Interface=0`. Windows fragt das während aktiver
  Wiedergabe alle ~32ms ab (beide Kanäle abwechselnd) – also deutlich
  öfter als unser eigener 120ms-Mixer-Poll (main.rs). Das bestätigt: die
  reine Abfragehäufigkeit ist nicht das Problem (siehe USB-Stall-Vorfall
  unten) – Windows nutzt dafür aber eine schlanke, dauerhaft offene
  Verbindung, während wir bei jedem Tick einen neuen `amixer`-Prozess
  starten (neues Öffnen/Schließen des ALSA-Mixer-Handles). Es gibt keinen
  separaten "Positions-Abfrage"-Befehl – GET_CUR auf genau dieser Feature
  Unit IST die Positionsabfrage, und ALSA nutzt intern denselben
  Mechanismus (deshalb liefert unser `get_level()` über `amixer`
  dieselbe Information ohne einen eigenen 5a- oder Rohzugriff).
- **Mixer**: 7 Feature-Unit-Entities für 8 Regler (Wiedergabe/Überwachung/
  Aufnahme), siehe `mixer.rs::CONTROLS`. **Bestätigt:** "Speaker Playback
  Volume" (entity-id 0) dämpft nicht nur den analogen Line-Out, sondern
  auch den digitalen S/PDIF-/optischen Ausgang – kein bit-perfekter
  Passthrough, wie man bei USB-Audio-Interfaces sonst oft erwarten würde.
  Live bestätigt: Lautsprecher-Modus über Optical-Out an eine Sonos
  Playbase, unser Regler ändert hörbar deren Lautstärke. Es gibt keine
  separate ALSA-Control für den S/PDIF-Ausgang (nur "S/PDIF In
  Playback/Capture Volume" für den Eingang/das Monitoring) – die
  Dämpfung läuft also intern über dieselbe Speaker-Volume-Stufe wie der
  analoge Ausgang.
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
- **Autostart** (`src/autostart.rs`): Standard-XDG-Mechanismus, legt/löscht
  `~/.config/autostart/g8-control.desktop` – kein systemd-Unit, keine
  Root-Rechte.
- **Tray-Icon + Schließen-Verhalten** (`ui/tray.slint`,
  `Window::on_close_requested` in main.rs): Slint bringt `SystemTrayIcon`
  nativ mit (Linux: StatusNotifierItem/ksni, bereits eine transitive
  Abhängigkeit von `i-slint-core`) – kein eigenes Ksni-Setup nötig.
  "Schließen" beendet das Programm richtig (`slint::quit_event_loop()`),
  "In den Tray minimieren" versteckt nur das Fenster
  (`CloseRequestResponse::HideWindow`), das Tray-Icon hält die
  Ereignisschleife am Laufen.
- **Einstellungen persistieren** (`src/persist.rs`): App-seitige Werte,
  die das Gerät nicht zurücklesen lässt (Acoustic Engine/Equalizer,
  Energiesparen, Logo-LED, Dark Mode, Schließen-Verhalten, Volume Sync),
  werden bei jeder Änderung in `~/.config/g8-control/settings.txt`
  gespeichert (einfaches Text-Format, bewusst kein serde/JSON für eine
  Handvoll Werte) und beim Start wieder angezeigt UND ans Gerät gesendet
  – sonst würde die App nach jedem Neustart wieder ihre eingebauten
  Standardwerte zeigen, obwohl das Gerät längst andere Werte hat.
  **Volume Sync** wird beim Start automatisch wiederhergestellt (anders
  als die übrigen Werte ursprünglich bewusst ausgenommen, dann auf
  Nutzerwunsch ergänzt) – ohne den Bestätigungsdialog, der nur für
  bewusstes manuelles Umschalten gedacht ist. Sicherheitsmaßnahme gegen
  einen Lautstärkeschock: main.rs setzt PipeWire beim Wiederherstellen
  zuerst auf 0%, dann erst auf den tatsächlich ausgelesenen
  Reglerstand, statt direkt auf einen möglicherweise lauten alten
  PipeWire-Stand zu wechseln.
  Zusammen mit Autostart + "In den Tray minimieren" macht das den
  Lautstärkeknopf durchgehend wirksam, auch ohne offenes Fenster – ohne
  laufende App gibt es unter Linux (anders als unter Windows mit dem
  `CTUSBa64`-Kerneltreiber) keine Kopplung zwischen Knopf und tatsächlich
  hörbarer Lautstärke, da PipeWire eine eigene, vom ALSA-Register
  unabhängige Software-Lautstärkestufe führt (siehe Abschnitt 7).
- **Lautstärke-Hintergrunddienst** (`src/daemon.rs`, Schalter im
  Einstellungen-Tab): für den Fall, dass die App wirklich komplett
  beendet sein soll (kein Tray-Icon, kein Fenster), aber der
  Lautstärkeknopf trotzdem mit PipeWire gekoppelt bleiben soll. Läuft
  als `systemctl --user`-Dienst (`~/.config/systemd/user/
  g8-control-volume-daemon.service`) – **kein Root/sudo nötig**, da
  `--user` die vom systemweiten Root-systemd komplett getrennte
  Nutzer-Instanz anspricht (verwaltet `~/.config/systemd/user/`, nicht
  `/etc/systemd/system/`). Derselbe Programmpfad, mit `--daemon`
  aufgerufen (`daemon::run()` in main.rs, vor jeder Slint-
  Initialisierung abgezweigt), läuft ohne Fenster/Tray-Icon nur die
  Poll-Schleife (identisch zur Volume-Sync-Kopplung im GUI-Prozess,
  aber ohne HID-Gerätezugriff – reine ALSA/PipeWire-Kopplung). Start/
  Stop direkt über den Schalter (kein Bestätigungsdialog mit
  kopierbaren Befehlen wie z.B. bei CoolerControl – dort nötig, weil es
  Root braucht, hier bewusst nicht, da risikolos userspace-only).

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
Neuaufbau nötig. **Wichtig: diese JSON-Dateien werden zur Laufzeit NICHT
gebraucht und nicht gelesen** – alle Werte wurden einmalig extrahiert und
liegen fest im Programm (`protocol.rs::sound_mode::PRESETS`,
`crystal_voice::VOICE_MORPH_PRESETS`/`MIC_EQ_PRESETS`,
`equalizer-tab.slint::builtin-presets`). Ein Ordner mit den rohen JSONs
im Projektverzeichnis (z.B. `SB1900/`) ist reine Ablage/Referenz für
künftige Ergänzungen, gehört nicht zum Build und ist per `.gitignore`
vom Repo ausgeschlossen (persönliche Geräte-Profile). Speicherorte
(Windows):

- `%LOCALAPPDATA%\Creative\{GUID}\Product\SB1900\` – Nutzer-eigene/aktuell
  aktive Profile
- `C:\ProgramData\Creative\CreativeApp\Product\SB1900\` – werkseitig
  mitgelieferte Standard-Presets

**EQ-Presets** (`Presets\EQ\*.json`, 71 Stück: Spiele, Genres). Format:
`PreAmp` + `Bands[]` mit `Frequency`+`Value` (dB), **zwei getrennte
`Settings`-Einträge pro Datei** (`Type: "Speaker"` und `Type:
"Headphone"`, unterschiedliche Kurven!). **Implementiert:** alle 71,
alphabetisch im Equalizer-Dropdown (`protocol.rs`, `builtin-presets` in
equalizer-tab.slint) – jeweils die Headphone-Kurve übernommen (unser
Equalizer kennt bislang keine getrennte Lautsprecher-Kurve). Drei freie
Custom-Slots zum eigenen Speichern kommen dahinter.

**SoundMode-Presets** (`SoundMode\*.json`, 60 Stück): Kombinierte
Acoustic-Engine-Profile (Surround/Crystalizer/Bass/SVM/DialogPlus +
Verweis auf ein EQ-Preset, per GUID über `GraphicEQ.PresetId` mit dem
passenden EQ-Preset verknüpft). **Implementiert:** alle 60 in
`protocol.rs::sound_mode::PRESETS`, eigener "Sound-Modus"-Tab (wieder als
erster Tab im Geräte-Hub, wie im Original) mit Dropdown ("Im Gerät" +
alle 60 echten Presets), sendet beim Auswählen alle zugehörigen
Acoustic-Engine-Toggles/Level sowie EQ-Vorverstärkung + 10 Bänder.
`bass_xover` aus den JSONs wird nicht gesendet – kein bekannter
Geräteparameter dafür (siehe Abschnitt 8). Der Sound-Modus-Tab zeigt
zusätzlich dieselben Live-Regler wie der Acoustic-Engine-Tab
(Surround/Crystalizer/Bass/Smart Volume/Dialog+) sowie eine
Equalizer-Zeile (Toggle + dasselbe 71er-Preset-Dropdown wie im
Equalizer-Tab) – beide Tabs teilen sich denselben Zustand
(Zwei-Wege-Bindung über app-window.slint/device-hub-tab.slint, analog
zum Fußleisten-/Mixer-Sync), ein Regler-Dreh an einer Stelle zeigt sich
sofort an der anderen. Rust aktualisiert diese geteilten Werte auch
selbst, wenn ein komplettes Sound-Modus-Preset angewendet wird (kein
Rücklese-Mechanismus vom Gerät nötig).

**Voice-Morph-Presets** (`VoiceFxProfile\*.json`, 18 Stück) –
**implementiert**: `protocol.rs::crystal_voice::VOICE_MORPH_PRESETS`,
echte Werte je Charakter (z. B. "Male" = `[400, 1400, 2000, 0.6, 1.05, 1,
0, 0]`), Dropdown in crystal-voice-tab.slint zeigt die echten 18 Namen,
main.rs sendet beim Wechsel die vollen 8 Bandwerte.

**Mic-EQ-Presets** (`MicEqProfile\*.json`, 9 Stück) – **implementiert**:
`protocol.rs::crystal_voice::MIC_EQ_PRESETS`, analog zu Voice Morph.

## 7. Baustelle: Lautstärke-Synchronisierung

Mehrere Stellen können Lautstärke ändern (ALSA-Hardwareregister,
PipeWire-Sink-Lautstärke, unsere UI-Regler), die nicht vollständig
miteinander verknüpft sind:

- **Gefunden und behoben (Zwischenstand, siehe Taper-Korrektur unten für
  die finale Lösung):** ALSA-Prozent (`amixer`/`get_level()`) und
  PipeWire-Sink-Prozent (`wpctl get-volume`) sind für die G8 **zwei
  unterschiedliche Skalen**, keine austauschbaren Zahlen – am laufenden
  Gerät empirisch bestätigt: ALSA 27 % entspricht PipeWire 17 %, ALSA
  65 % entspricht PipeWire 42 %. PipeWire/PulseAudio verwenden die
  "kubische" Lautstärke-Konvention (Prozent = dritte Wurzel des linearen
  Gain-Faktors), ALSA zeigt einen dB-basierten Prozentwert. Volume Sync
  kopierte ursprünglich den rohen ALSA-Prozentwert 1:1 in die PipeWire-API
  (`wpctl set-volume`) – das erzeugte bei *jeder* Synchronisierung
  garantiert eine falsche PC-Lautstärke (genau das "System 100 % vs.
  Lautsprecher 27 %"-Verhalten).
- **Gefunden und behoben: der Lautsprecher-Drehregler hat eine
  "Audio-Taper"-Kennlinie (analog-Poti), keine lineare Entsprechung
  zwischen Drehposition und Rohwert.** Live kalibriert (Anschläge +
  optische Mitte angefahren) UND aus einem Windows-Capture einer
  kompletten langsamen Minimum→Maximum-Drehung verifiziert
  (`vol min-max-min_mit sync.pcapng`, 24 tatsächlich durchlaufene
  Rohwerte): der rohe ALSA-Wert (raw/128, linear zu dB:
  `dB = (raw-128)*0.5`, min=-64dB/max=0dB per `amixer cget` bestätigt)
  steigt am unteren Ende sehr schnell (große Rohwert-Sprünge pro
  Dreh-Klick) und flacht zum oberen Ende stark ab (nur noch 1-2 Rohwert-
  Einheiten pro Klick) – bei optischer Mitte steht der rohe ALSA-Wert
  bereits bei ~84 % statt bei 50 %. Das ist eine reine Hardware-
  Charakteristik (bestätigt: die komplette Poll-/Änderungserkennungs-
  Kette funktioniert lückenlos korrekt, siehe unten), kein Software-Bug.
  Fix in zwei Teilen:
  1. `mixer.rs::TAPER_TABLE` – 24-Punkte-Stützstellentabelle aus dem
     Capture, `taper_correct()`/`taper_uncorrect()` interpolieren
     stückweise linear zwischen den Rohwerten. `get_level(0)` gibt jetzt
     den "gefühlten" (korrigierten) statt den rohen linearen Prozentwert
     zurück, `set_level(0, ...)` rechnet beim Schreiben zurück. Gilt NUR
     für entity-id 0 (Lautsprecher-Poti) – die übrigen Mixer-Regler sind
     rein digitale ALSA-Gains ohne physisches Poti dahinter.
  2. Volume Sync sendet jetzt den bereits korrigierten linearen Prozent-
     wert direkt an PipeWire (`mixer.rs::set_pipewire_sink_volume_percent()`,
     ersetzt die frühere dB/kubische Umrechnung `get_level_db()`/
     `set_pipewire_sink_volume_db()`, die deshalb entfernt wurden) – Ziel
     ist, dass die PC-Lautstärke *gefühlt* zum Reglerstand passt, nicht
     dass sie physikalisch exakt denselben Gain repräsentiert. Bewusste
     Design-Entscheidung, mit dem Nutzer abgestimmt: eine systemweite
     ALSA-/PipeWire-Konfigurationsänderung (würde alle Programme
     betreffen) wurde als zu riskant verworfen – die Korrektur bleibt auf
     die App selbst und den optionalen, expliziten Volume-Sync-Schalter
     beschränkt (der schon eine Bestätigung verlangt, bevor er aktiviert
     wird).
- **Schwerwiegender Vorfall gefunden – vermutlich NICHT durch die G8
  verursacht.** Beim Testen (Umschalten Kopfhörer/Lautsprecher + aktives
  Volume Sync) blieb `wireplumber` im Kernel hängen: `cat
  /proc/<pid>/wchan` zeigte **`usb_control_msg`** – ein
  USB-Control-Transfer kam nie zurück, der Prozess landete im `D`-State
  (uninterruptible sleep) und war nicht mal per `SIGKILL` zu beenden.
  Jeder `wpctl`-Aufruf (auch von außerhalb der App) hängte sich daran
  auf; `systemctl restart` half nicht, da der Dienst zwar "aktiv" ist,
  aber der Kernel-Aufruf blockiert bleibt.
  **Auflösung:** Der Nutzer hat daraufhin das CORSAIR HS80 MAX
  WIRELESS-Headset-Dongle abgezogen (nicht die G8!) – Sound funktionierte
  danach sofort wieder. Das deutet stark darauf hin, dass **der Corsair-
  USB-Dongle** der eigentlich im `usb_control_msg` feststeckende
  Client war, nicht die G8 – der ursprüngliche Verdacht (viele
  gleichzeitige G8-USB-Zugriffe durch g8-control) ist damit NICHT
  bestätigt und eher unwahrscheinlich. Wireless-Dongles wie der Corsair
  sind für gelegentliche Firmware-/USB-Hänger bekannt; da `wireplumber`
  alle Audiogeräte über denselben Prozess verwaltet, reicht ein
  hängender Control-Transfer zu IRGENDEINEM Gerät, um `wpctl` global
  einzufrieren (nicht nur für das betroffene Gerät).
  - Trotzdem als generelle Härtung sinnvoll und beibehalten (schadet
    nicht, hilft bei jedem künftigen Stall – egal welches Gerät ihn
    auslöst):
    `src/proc_util.rs::run_with_timeout()` – neuer gemeinsamer Wrapper um
    `amixer`/`wpctl`-Aufrufe (mixer.rs, virtual_surround.rs) mit
    Timeout (500ms bzw. 3s) statt des blockierenden
    `Command::output()`. Verhindert, dass ein zukünftiger Stall auch
    unsere eigenen Threads (Mixer-Poll, virtueller Surround) für immer
    mitreißt – der ursprüngliche Prozess bleibt zwar trotzdem hängen
    (Kernel-Problem), aber die App friert nicht mit ein.
    `on_output_target_changed` (main.rs) ignoriert jetzt außerdem
    Klicks auf das bereits aktive Ziel (`last_output_target`-Guard) –
    vorher löste JEDER Klick auf Kopfhörer/Lautsprecher einen vollen
    PipeWire-Neustart aus, auch wenn schon ausgewählt (mehrere Tabs
    zeigen denselben Umschalter), was unnötige Neustarts und damit
    zusätzliches Stall-Risiko bedeutete.
  - **Noch offen:** die eigentliche Ursache im Kernel/WirePlumber ist
    nicht abschließend geklärt – falls der Stall trotz obiger
    Eindämmung wieder auftritt, wäre der nächste Schritt, `amixer`/HID-
    Zugriffe zeitlich zu entkoppeln (z.B. Mixer-Poll und
    Ausgabe-Umschaltung nie überlappend), oder amixer/wpctl komplett
    durch eine dauerhaft offene ALSA-Verbindung zu ersetzen (siehe
    unten) statt bei jedem Tick neue Prozesse zu starten.
- **Gefunden und behoben: WirePlumber überschreibt die Lautstärke bei
  jedem PipeWire-Neustart mit einem alten gespeicherten Wert.** Live
  bestätigt: nach Umschalten Kopfhörer/Lautsprecher sprang die
  Lautstärke plötzlich auf einen alten, durch nichts erklärbaren Wert
  zurück – weder der Knopf noch die App hatten sie geändert.
  `~/.local/state/wireplumber/default-routes` enthält pro Gerät/Route
  einen dauerhaft gespeicherten `channelVolumes`-Wert (z.B.
  `alsa_card.usb-Creative_..._Sound_Blaster_G8_USB-1_F1006147691-00:
  output:analog-output-speaker={"channelVolumes":[0.266082, ...]}`),
  den WirePlumber bei jedem Neu-Einlesen des Geräts wiederherstellt –
  und genau das passiert bei jedem `restart_pipewire()`-Aufruf (siehe
  `virtual_surround.rs`, ausgelöst durch den Kopfhörer/Lautsprecher-
  Umschalter für den virtuellen HRTF-Surround). Fix:
  `restart_pipewire_preserving_volume()` merkt sich den aktuellen
  ALSA-Pegel vor dem Neustart und schreibt ihn nach einer kurzen
  Wartezeit (WirePlumbers eigener Restore muss zuerst fertig sein)
  aktiv zurück – überschreibt damit WirePlumbers veralteten
  gespeicherten Wert. Betrifft nur die ALSA-Hardware-Route; ob
  WirePlumbers PipeWire-Sink-Lautstärke (die Volume-Sync-Zielgröße)
  bei einem Neustart ebenfalls zurückgesetzt wird, ist nicht gesondert
  geprüft – falls doch, müsste Volume Sync nach einem Neustart
  zusätzlich einmal neu synchronisieren.
- Smart Volume (Acoustic Engine) hebt/senkt den Pegel nicht immer sauber
  nachvollziehbar
- **Geklärt und korrigiert (siehe Taper-Korrektur oben):** große,
  unerklärte Sprünge im Lautstärke-Poll (z.B. 44%→6%) sind die normale
  Rasterung des physischen Audio-Taper-Potis, kein Software-Bug. Live
  per Diagnose-Logging nachgewiesen: die komplette Poll-/Änderungs-
  erkennungs-/UI-Push-Kette (`main.rs`, Mixer-Poll-Thread) funktioniert
  lückenlos und korrekt – jede Knopfdrehung wird exakt und ohne
  Aussetzer weitergegeben (Log zeigte u.a. `...48→44→6→44→52...` bei
  durchgehendem Drehen, genau die großen Rohwert-Sprünge am unteren
  Ende der Taper-Kurve). Der ursprünglich gewünschte Windows-Mitschnitt
  mit langsamem Drehen war bereits vorhanden (`vol min-max-min_mit
  sync.pcapng`) und lieferte die 24-Punkte-Kalibriertabelle für
  `TAPER_TABLE`.
- Footer-Regler und Mixer-Tab teilten sich lange keinen Zustand (gefixt,
  siehe Commit-Historie) – weitere Mixer-Regler (Line In, SPDIF, What U
  Hear) sind noch nicht live synchronisiert, nur Lautsprecher/externes Mic

**Volume-Sync-Captures ausgewertet** (`vol min-max-min_mit sync.pcapng`,
`volsync aus min max-volsync-ein min max.pcapng`): Der Knopf am Gerät
erzeugt beim Drehen – auch bei aktivem Volume Sync – **keinerlei
zusätzlichen USB-Traffic** (weder auf dem proprietären 0x5a-Kanal noch
als zusätzliches Standard-UAC SET_CUR). Volume Sync selbst ist nur der
eine bestätigte `5a 39 03 00 1c <an|aus>`-Befehl (Gerät bestätigt mit
`5a 02 0a 39 00 1c <wert>` – neu entdecktes generisches ACK-Format für
die 0x39-Familie). Windows nutzt für die G8 einen eigenen Kernel-Treiber
(`CTUSBa64`, siehe Abschnitt 1) statt des generischen UAC-Treibers – die
eigentliche Kopplung Knopf↔Windows-Systemlautstärke passiert vermutlich
treiberintern, unsichtbar auf dem USB-Kabel. **Schlussfolgerung:** Es
gibt keinen fehlenden USB-Befehl zu finden, den wir noch senden müssten
– das "kreuz und quer synchronisieren" ist eine PipeWire/ALSA-
Architekturfrage (mehrere echte Lautstärke-Ebenen: Hardware-Register,
PipeWire-Sink, ggf. App-Stream-Lautstärke), kein Protokoll-Rätsel. Weitere
USB-Captures zu diesem Thema bringen vermutlich keine neuen Erkenntnisse
mehr.

## 8. Offene Punkte

- **Sound-Modus-Preset auf dem Gerät auslesen** (geplanter nächster
  USB-Capture, noch nicht gemacht): Ziel ist herauszufinden, ob das Gerät
  oder die Windows-Software beim (Wieder-)Verbinden ein zuletzt aktives
  Sound-Modus-Preset zurückmeldet – bisher unbekannt, nur Anwenden ist
  implementiert (software-seitiges Merken über den App-Neustart hinweg IST
  seit 2026-08-23 umgesetzt, siehe persist.rs/main.rs
  `sound_mode_preset_index` – das hier ist nur für ein *echtes*, geräte-
  seitiges Auslesen, z.B. auch von einem anderen Rechner/unter Windows
  aus sichtbar). Capture-Rezept (USBPcap + Wireshark, wie die bisherigen
  Captures in diesem Projekt):
  1. Capture starten, Creative-Software öffnen.
  2. Im Sound-Modus ein bestimmtes Preset wählen (Namen notieren).
  3. Software komplett schließen (oder Gerät ab-/anstecken), dann neu
     öffnen bzw. neu verbinden lassen – genau in diesem Moment würde ein
     "welches Preset ist aktiv"-Abruf passieren, falls es ihn gibt.
  4. Prüfen, ob die Software das zuvor gewählte Preset beim Neustart
     wieder anzeigt (Screenshot zum Abgleich mit dem Capture).
  5. .pcapng speichern, wie gehabt mit scapy/rdpcap auswerten (siehe
     Abschnitt 3 für das Paket-Parsing-Vorgehen).
  Ohne diesen Capture wird hier nichts spekulativ implementiert.
- Mixer-Zugriff (`mixer.rs`) startet für jedes Poll-Tick einen neuen
  `amixer`-Prozess statt eine dauerhafte ALSA-Verbindung offen zu halten
  (wie Windows/PipeWire es intern tun, siehe Abschnitt 3/7). Funktioniert,
  ist aber unnötig schwergewichtig und ein Faktor beim beobachteten
  USB-Stall (Abschnitt 7). Sauberer wäre ein persistentes Mixer-Handle
  über die `alsa`-Crate statt Prozess-Spawning pro Tick – bisher nicht
  umgesetzt (neue Abhängigkeit, größerer Umbau).
- Smart Volume Modus-Bestätigung nur für Wiedergabe-Seite, nicht Mic-Seite
- Abtastrate/Sample-Rate-Wechsel (Clock Source) noch nicht direkt beobachtet
- Sound-Modus-Presets speichern/laden im Gerät weiterhin unbekannt (nur
  Anwenden implementiert, kein Auslesen/Persistieren eines aktiven Presets)
- Bass-Crossover-Frequenz (`bass_xover` in den SoundMode-JSONs) hat keinen
  bekannten Geräteparameter, wird beim Anwenden eines Sound-Modus nicht
  gesendet
- Scout-Mode-Zugriffstaste (Hotkey) ist nur UI-vorbereitet, kein echter
  globaler Tastatur-Hook angebunden
- GET-Anfrage-Familien 0x26 (teilweise), 0x39, 0x6c, 0x10 unentschlüsselt
- Equalizer kennt nur eine Kurve, keine getrennte Kopfhörer-/Lautsprecher-
  Variante wie das Original (siehe Abschnitt 6, EQ-Presets) – Original-
  Preset-JSONs haben tatsächlich getrennte Kurven für 43 von 71 Presets
  (per Skript verglichen, 2026-08-23), technisch also sinnvoll umsetzbar

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
