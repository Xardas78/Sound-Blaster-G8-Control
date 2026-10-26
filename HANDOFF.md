# Sound Blaster G8 – Reverse-Engineering: Vollständiger Wissensstand

Kontext für die Fortsetzung mit Claude Code. Fasst alles zusammen, was per
USB-Traffic-Analyse (USBPcap unter Windows + Live-Tests am echten Gerät
unter Linux) über die Sound Blaster G8 (USB-ID `041e:329b`) herausgefunden
wurde.

Zwei begleitende Dateien, beide nie löschen/überschreiben, nur ergänzen:
[HANDOFF-ARCHIVE.md](HANDOFF-ARCHIVE.md) (abgeschlossene, bestätigt
behobene Bugs – raus aus dem Weg, aber als Referenz erhalten) und
[HANDOFF-REVERTED.md](HANDOFF-REVERTED.md) (Ansätze, die implementiert
und wieder rückgängig gemacht wurden – wichtig, bevor man einen Ansatz
"das müsste doch helfen" erneut versucht).

## 1. USB-Topologie

**Korrektur 2026-08-24:** `0c70:f011` ist **nicht** Teil der G8 – live auf
zwei verschiedenen Rechnern nachgeprüft, das Gerät identifiziert sich
eindeutig als "MCT Elektronikladen OCTO", ein komplett unabhängiges
Produkt, das zufällig am selben Hub hing. Die vorherige Annahme einer
"Zwei-USB-Geräte"-Bauweise der G8 war ein Fehlschluss (vermutlich aus
einer frühen Windows-Session, wo dieses Fremdgerät zufällig mitgeloggt
wurde). Die G8 selbst zeigt sich nur als das eine Gerät `041e:329b`.

`041e:329b` ist das einzige Gerät, hat u. a.:

- **Interface 0**: UAC2 Audio Control (Feature Units, Clock Source,
  Extension Units) – Standard-Lautstärke, Mixer, SBX-Extension-Unit. Unter
  Linux generischer UAC2-Support über ALSA, funktioniert ohne eigenen Code.
- **Interface 1**: UAC2 Audio Streaming (Wiedergabe) – Alt-Settings 0=Idle,
  1=24-Bit (Paketgröße 294), 2=32-Bit (Paketgröße 392)
- **Interface 3**: HID, Consumer-Control, Interrupt-IN-Endpoint **0x84**
  (6 Byte) – lange als "ungenutzt" markiert, das war falsch: sendet
  `KEY_MUTE`/`KEY_VOLUMEUP`/`KEY_VOLUMEDOWN`-artige Reports beim Drehen
  des Reglers, aber NUR wenn `VOLUME_SYNC` aktiv ist – siehe die
  Korrektur 2026-08-24 in Abschnitt 7 für die volle Geschichte.
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
| Smart Volume (Mic) | 0x2c | reines An/Aus, **kein** Modus-Parameter (anders als Wiedergabe-SVM `0x06`) – per Capture bestätigt 2026-08-23 |

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
| SPDIF-Out Direct Mode | 0x0d | implementiert (Wiedergabe-Tab, nur bei Lautsprecher) – Bedeutung per Hörtest bestätigt (2026-08-24): DSP-Bypass, Effekte (Surround/Crystalizer/Bass/…) hörbar deaktiviert wenn an |
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

### Direct Mode (physischer MODE-Knopf)

**Korrektur 2026-08-24:** Direct Mode ist ein echtes einzelnes
Hardware-Register, keine Batch-Umschaltung mehrerer Acoustic-Engine-
Parameter:

```
5a 39 03 00 05 01   Direct Mode ein
5a 39 03 00 05 00   Direct Mode aus
```

Byte-genau bestätigt per frischem Capture `Directmode ein-aus.pcapng`
(2026-08-24) – derselbe Befehl, den auch der physische MODE-Knopf am
Gerät auslöst. In `main.rs` als `special::device_setting(setting_id::
DIRECT_MODE, on)` umgesetzt, siehe `protocol.rs`-Test `direct_mode_report`.
Die frühere Annahme, Direct Mode sei identisch mit dem SBX-Masterschalter
unten (6-Toggle-Batch) und wurde entsprechend nachgebildet, war ein
Fehlschluss – vermutlich wurde in der alten Capture-Session ein
separater SBX-Schalter in der Windows-Software getroffen, nicht der
MODE-Knopf. Details zur alten, verworfenen Implementierung und warum sie
zeitweise plausibel wirkte: siehe [HANDOFF-REVERTED.md](HANDOFF-REVERTED.md).

**Live bestätigt (2026-08-24):** App-Schalter und physischer MODE-Knopf
beide getestet – MODE-LED wechselt korrekt (grün bei an, aus bei aus,
SBX-LED entsprechend invers), sauberer Übergang in beide Richtungen,
Hörtest unauffällig (kein Knacken wie beim Lautstärke-Regler-Workaround).

Noch offen: der Status-Bitfeld-Reply, mit dem sich der aktuelle Direct-
Mode-Zustand zurücklesen ließe (nötig, damit der UI-Schalter dem
physischen MODE-Knopf folgt) – das vermutete Muster (`5a 39 05 01 ...`,
siehe Abschnitt 7, Nachtrag 2026-08-24) kam beim Live-Test über die
Diagnose-Logging-Schleife NICHT durch, bleibt also unbestätigt.

**Konkret beobachtetes Symptom (Nutzer, 2026-08-24):** Befehle kommen
nachweislich korrekt am Gerät an (bestätigt durch Klick-/LED-Feedback
am Gerät bei jeder Betätigung, egal ob per App oder physischem Knopf).
Das Problem ist reine Zustands-Divergenz: App und Gerät führen zwei
unabhängige, nicht synchronisierte Zustände. Drückt der Nutzer den
physischen Knopf, ändert sich der Geräte-Zustand, ohne dass die App es
mitbekommt – der nächste App-Klick sendet dann den Befehl passend zum
(veralteten) App-internen Zustand, nicht zur Realität. Kein Bug im
SET-Pfad, reines Fehlen eines GET/Status-Pfads.

### SBX-Masterschalter (separater Windows-Software-Schalter, nicht der MODE-Knopf)

Windows schaltet bei diesem – vom Direct-Mode-Knopf getrennten –
Schalter 6 Acoustic-Engine-Parameter gemeinsam um (alle auf `0.0` bzw.
`1.0`):

```
Surround (0x00), Dialog+ (0x02), Smart Volume (0x04),
Crystalizer (0x07), Equalizer (0x09), Bass (0x18)
```

Bestätigt per Capture UND Live-Test (Batch-Reports `5a 11 2c 07 00 ...`
bzw. `5a 11 26 06 00 ...`) – vermutlich Quelle der oben korrigierten
Verwechslung. Nicht in `main.rs` angebunden (kein bekanntes UI-Element
dafür identifiziert). Zusätzlich existiert der generische SBX-Sync-
Befehl `5a 26 03 08 ff ff` (Scout Mode und Direct-Mode-Ein lösen ihn
aus).

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
5a 39 01 02   → 5a 39 09 02 20 64 02 14      (NICHT der Direct-Mode-Status,
                                               siehe Korrektur unten – bleibt
                                               bei ein UND aus identisch)
5a 6c 01 02   → 5a 6c 0e 02 05 85 00 01 00 02 00 03 00 04 00 05
5a 10 00      → 5a 10 08 ef 67 74
```

Nur das erste Paar (0x2c) ist bei uns angebunden. Die anderen (0x26 Rest,
0x39, 0x6c, 0x10) sind unentschlüsselt – wirken wie weitere
Geräte-Statusabfragen.

### Namespace 0x09 – Geräteinformationen (neu entdeckt, 2026-08-23)

Per Capture (`schrit 1 bis 5 nach claudes vorgabe.pcapng`) beim
App-Start/Reconnect gefunden, taucht in exakt derselben Reihenfolge bei
jedem Verbindungsaufbau auf, direkt vor der 0x07/0x2c/0x26/0x39/0x10/0x6c-
Sequenz:

```
5a 09 01 02      → 5a 09 12 02 10 "1.2.250819.0903"   (Firmware, andere Rahmung als 0x07)
5a 09 01 03      → 5a 09 14 03 "<Seriennummer>"       (Seriennummer – neu! Per Screenshot Einstellungen-Tab
                                                        gegengeprüft, echter Wert in SB1900/device_info_private.md,
                                                        bewusst nicht hier im öffentlichen Repo-Verlauf)
5a 09 01         → 5a 09 11 00 9b 32 01 02 1e 04 00 00 00 00 00 00 00 00 00 39
5a 09 02 01      → 5a 09 08 01 00 23 4d 4c 21 22 04
5a 09 02 01 01   → 5a 09 08 01 01 20 04 18 20 08 02
```

Die letzten drei Antworten sind binäre Blöcke unbekannter Bedeutung
(vermutlich Hardware-Revision/Capability-Flags o.ä.) – bisher nicht
implementiert, da ohne bekannten Nutzen für die App. Byte nach dem
Namespace (0x12/0x14/0x11/0x08) wirkt wie eine Längenangabe der
folgenden Nutzdaten.

### Periodischer Status-Heartbeat, Wire-Format entschlüsselt (2026-08-23)

Der in Abschnitt "Kopfhörer/Lautsprecher-Umschaltung" erwähnte
unaufgeforderte Heartbeat sendet nicht nur den HP/SPK-Status, sondern in
schneller Folge mehrere 64-Byte-Reports mit Namespace/Param/Wert-Tripeln:

```
5a 11 32 08 00  [ns][param][float32] [ns][param][float32] ...   (volle Reports, ~9 Tripel)
5a 11 14 03 00  [ns][param][float32] ...                        (letzter, kürzerer Report)
```

Jedes Tripel ist 6 Byte: 1 Byte Namespace (0x95/0x96/0x97), 1 Byte
Parameter-ID, 4 Byte Float32 LE-Wert – identisches Format zu den
einzelnen GET-Antworten (`5a 11 08 01 00 <ns> <param> <float>`), nur
mehrere hintereinander ohne das `08 01 00`-Zwischenstück. Deckt bei
einem vollständigen Burst praktisch den kompletten Acoustic-
Engine/EQ/CrystalVoice-Parametersatz ab. Tritt u. a. unmittelbar nach
dem Verbindungsaufbau auf sowie periodisch danach (siehe oben,
ca. alle 1,5–2,5s).

Im selben Burst zwei weitere, bisher unbekannte unaufgeforderte
Pushes gefunden (nur je einmal beobachtet, keine zugehörige Anfrage
gesehen – vermutlich Statusänderungs-Events statt Abfrage-Antworten):

```
5a 6e 02 01   – Bedeutung unklar
5a 1a 02 02   – Bedeutung unklar, evtl. Jack-Sensing/Output-Status
```

Für beide braucht es einen gezielten Capture (z. B. Kopfhörer ein-/
ausstecken, LED/Ausgangs-Zustand gezielt ändern), um sie einer Aktion
zuzuordnen.

### Sound-Modus-Preset-Auslesen – Frage aus Abschnitt 8 beantwortet (2026-08-23)

Capture-Ergebnis (`schrit 1 bis 5 nach claudes vorgabe.pcapng`): Preset
"Baldur's Gate 3" gewählt (t≈47s im Capture), Software geschlossen und
neu verbunden (t≈107,6s). Beim Reconnect gibt es **keine** dedizierte
"welches Preset ist aktiv"-Abfrage – die Software liest stattdessen
jeden Einzelparameter per GET aus (`5a 11 03 01 96 0b` usw.), und die
zurückgelesenen Rohwerte stimmen exakt mit den beim Preset-Wechsel
gesetzten Werten überein. Das Gerät speichert also die rohen Acoustic-
Engine/EQ-**Werte** dauerhaft selbst (übersteht App-Neustart), aber es
gibt keinen Hinweis auf eine geräteseitige Speicherung der Preset-
**Kennung/des Namens** – aus Sicht des Protokolls sieht "Baldur's Gate
3" nur wie eine zufällig passende Kombination aus 71 EQ- und 60
Sound-Modus-Werten aus. Die Namensanzeige im UI nach dem Neustart ist
mit hoher Wahrscheinlichkeit reine lokale Software-Buchhaltung der
Windows-App (vergleichbar mit unserem eigenen `persist.rs`-Ansatz für
`sound_mode_preset_index`, siehe Abschnitt 4) – **bestätigt damit, dass
der bereits gewählte Software-seitige Persistenz-Ansatz ausreichend
ist, kein weiterer geräteseitiger Abgleich nötig.**

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
  Nutzer sollen dafür die offizielle Windows-App nutzen. Der reine
  Versions-*Check* (nicht der Flash-Vorgang) läuft rein über Netzwerk,
  kein USB-Bezug: unauthentifizierter HTTP-GET auf `files.creative.com`
  (`/creative/bin/firmware/fwureleases/SB1900/SB1900_release.json` für
  die neueste Version, `/creative/res/firmware/releasenotes/SB1900/
  {en,de}/releasenotes.json` für die Versionsgeschichte) – nur zur
  Referenz notiert (2026-08-23), weiterhin nicht implementiert.
- **Decoder-Feintuning (DRC)**: G8 zeigt laut Original-UI nur Normal/Voll/
  Nacht, kein verstecktes Protokoll zu vermuten.

## 5a. "Gerät zurücksetzen" – implementiert und bestätigt (2026-08-23)

Button in Einstellungen → Gerät. Auslöser-Report per drei unabhängigen
Windows-Captures byte-identisch bestätigt (`reset.pcapng`, `reset_spk.pcapng`,
`rest mit Jack-Sensing.pcapng`) und in `protocol.rs::special::reset_device()`
umgesetzt:

```
5a 9b 01 03 00...00   (64 Byte, Rest Nullen)
```

Löst eine **echte USB-Neuenumeration** aus (`GET_DESCRIPTOR`, sichtbar auch
am Gerät selbst: alle LEDs blinken kurz, MODE-LED kurz rot), danach (~1,1s
später unter Windows) schickt das Gerät einen vollen Status-Heartbeat mit
echten Werksdefaults: Equalizer-Toggle (`96 09`) und alle 10 Bänder +
Vorverstärkung (`96 0a`–`96 14`) auf 0.0/0dB, Crossover (`96 17`) auf 80Hz
(statt z.B. 120Hz bei "Baldur's Gate 3"), Bass/Smart Volume/Dialog+ aus,
Surround an, Crystalizer bleibt an. Dreifach reproduziert unter Windows –
alle drei Male **byte-identischer** Werte-Burst, Reset ist also
zielunabhängig und deterministisch.

**Live unter Linux getestet (2026-08-23) und vom Nutzer bestätigt: "war ein
Reset wie unter Windows"** – der Report funktioniert unverändert über
unseren eigenen HID-Kanal (`device.rs::send_report`), kein Windows-
spezifisches Verhalten.

**Behoben (2026-08-23):** Nach dem Reset verlor unser `G8Device`-Handle
zunächst die Verbindung (echte USB-Neuenumeration = neuer Gerätepfad)
und musste manuell durch einen App-Neustart ersetzt werden. Jetzt per
`slint::Timer` (2s-Intervall, main.rs) automatisch behoben – Gerät hinter
`Rc<RefCell<Option<Rc<G8Device>>>>`, jeder Timer-Tick versucht
`G8Device::open()` erneut und ersetzt das Handle bei Erfolg immer (auch
wenn technisch noch ein altes, totes Handle "vorhanden" ist). Live
verifiziert: Reset zweimal hintereinander ohne App-Neustart ausgelöst,
funktioniert beide Male.

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

**Volume-Sync-Captures ausgewertet, 2026-08-23** (`vol min-max-min_mit
sync.pcapng`, `volsync aus min max-volsync-ein min max.pcapng`): Der
Knopf am Gerät erzeugt beim Drehen – auch bei aktivem Volume Sync –
"keinerlei zusätzlichen USB-Traffic" (weder auf dem proprietären
0x5a-Kanal noch als zusätzliches Standard-UAC SET_CUR). Volume Sync
selbst ist nur der eine bestätigte `5a 39 03 00 1c <an|aus>`-Befehl
(Gerät bestätigt mit `5a 02 0a 39 00 1c <wert>` – generisches
ACK-Format für die 0x39-Familie). Windows nutzt für die G8 einen eigenen
Kernel-Treiber (`CTUSBa64`, siehe Abschnitt 1) statt des generischen
UAC-Treibers – die eigentliche Kopplung Knopf↔Windows-Systemlautstärke
passiert vermutlich treiberintern, unsichtbar auf dem USB-Kabel.
**Schlussfolgerung damals:** kein fehlender USB-Befehl, reine
PipeWire/ALSA-Architekturfrage.

**Diese Schlussfolgerung war unvollständig (siehe Korrektur
2026-08-24 unten) – archiviert, nicht gelöscht, weil die Suchmethode
selbst dokumentiert bleiben soll (nur nach SET_CUR auf dem Vendor-Kanal
gesucht, nicht nach dem HID-Consumer-Control-Kanal oder einer
host-initiierten GET_CUR-Abfrage).**

### Nebenbefund 2026-08-24: wiederholte USB-Disconnects, zwei unabhängige Ursachen gefunden

Während der Fehlersuche zum obigen Regler-Ausfall fiel zusätzlich auf,
dass die G8 über Stunden alle 15-90s disconnected und neu enumeriert
wurde (`dmesg`: `usb 3-3.4: USB disconnect` gefolgt von `new high-speed
USB device number N+1`). Zwei getrennte Ursachen identifiziert:

1. **Ein liegengebliebener `dumpcap`-Prozess** (Wiresharks Capture-
   Backend, von einer früheren USB-Mitschnitt-Session dieses Projekts)
   löste wiederholt `usbfs: USBDEVFS_CONTROL failed cmd dumpcap rqt 128
   rq 6 len 617 ret -71` aus, jedes Mal gefolgt von einem erzwungenen
   Disconnect. Betraf nur ein Zeitfenster (17:48–18:22 Uhr an diesem
   Tag), danach lief `dumpcap` nicht mehr. **Lehre:** nach jeder
   Wireshark/usbmon-Capture-Session mit fester Laufzeit arbeiten
   (`tshark -a duration:N`, nie interaktiv per Ctrl+C beenden und dann
   vergessen) – ein liegengebliebener `dumpcap` kann die G8 stundenlang
   unbrauchbar machen, ohne dass es offensichtlich mit `dumpcap`
   zusammenhängt.
2. **Direkte Root-Ports (kein Hub dazwischen) sind auf diesem Rechner
   für die G8 unzuverlässig**, unabhängig von `dumpcap`, App, Kabel oder
   PC-Neustart – reproduziert auf zwei verschiedenen Root-Controllern
   (Bus 1 und Bus 3). Der einzige über Stunden absolut stabile Port war
   ein **Hub-Port** (`3-3.4`, Punkt in der Nummer = hinter einem Hub).
   Praktische Konsequenz: G8 wenn möglich über einen Hub anschließen,
   nicht direkt an einen Board-Rückseiten-Port.

Diese beiden Punkte erklären vermutlich einen Teil der Verwirrung an
diesem Tag (Regler schien "mal ganz tot", je nachdem ob gerade ein
Disconnect-Zyklus lief), sind aber **nicht** die Ursache des
eigentlichen Regler-Problems – das bestand nachweislich auch bei
komplett stabiler Verbindung fort (siehe unten).

**Dritte Ursache gefunden und behoben (später am selben Tag):** Auch
mit Hub-Anschluss traten noch vereinzelte Disconnects auf (z.B. während
der Direct-Mode-Live-Tests). Live geprüft: **der Hub selbst
(`0bda:5411`, Realtek RTS5411) hatte USB-Autosuspend aktiv**
(`/sys/bus/usb/devices/3-3/power/control` stand auf `auto`, nicht `on`)
– das G8-Gerät selbst hatte Autosuspend zwar schon deaktiviert, aber
eine Ebene höher am Hub nicht geprüft. Passt zu einem bekannten,
dokumentierten Linux-Problem: AMD-xHCI-Controller (dieser Rechner hat
zwei: `1022:43ee` 500-Series-Chipsatz, `1022:149c` Matisse/Ryzen-3000-
CPU-integriert) reagieren empfindlich auf USB-Autosuspend, siehe
[RedHat Bugzilla #1196943](https://bugzilla.redhat.com/show_bug.cgi?id=1196943)
("USB autosuspend massively breaks xHCI on AMD APU chipsets"). **Fix:**
`echo on | sudo tee /sys/bus/usb/devices/3-3/power/control`, dauerhaft
per udev-Regel (`/etc/udev/rules.d/99-g8-hub-no-autosuspend.rules`):
```
SUBSYSTEM=="usb", ATTR{idVendor}=="0bda", ATTR{idProduct}=="5411", TEST=="power/control", ATTR{power/control}="on"
```
Greift unabhängig vom genauen Port, solange derselbe Hub verwendet
wird. Noch nicht über eine längere Zeitspanne verifiziert, ob das die
Disconnects vollständig beendet – nächstes Mal beobachten.

**In `packaging/install.sh` automatisiert (2026-08-24):** Erkennt beim
Installieren automatisch, ob die G8 über einen Hub mit aktivem
Autosuspend hängt, erklärt Vor-/Nachteile und fragt explizit nach
Zustimmung, bevor `sudo` für die udev-Regel verwendet wird – der Rest
der Installation bleibt root-frei wie bisher. Nur relevant, wenn die G8
beim Ausführen von `install.sh` bereits angeschlossen ist.

### Korrektur 2026-08-24: Volume Sync doch nötig, plus der tatsächliche Windows-Mechanismus gefunden

Am 2026-08-24 fiel der Regler erneut komplett aus (siehe den
"Lautstärkeknopf komplett unresponsiv"-Vorfall oben – gleiches Symptom,
neuer Tag). Diesmal per `usbmon` unter Linux **live mitgeschnitten,
während parallel `evtest` das HID-Device offenhielt** (wichtig: ohne
einen offenen Reader poll't der Linux-Kernel den Interrupt-Endpoint
mancher HID-Interfaces gar nicht erst – ein früherer usbmon-Test ohne
`evtest` daneben zeigte deshalb fälschlich "gar kein Traffic").
Ergebnis: **ohne aktives `VOLUME_SYNC` sendet die G8 beim Drehen
tatsächlich keinerlei Reports auf dem HID-Consumer-Control-Kanal**
(Endpoint 0x84, Interface 3 – vom `hid-generic`-Treiber als
`KEY_MUTE`/`KEY_VOLUMEUP`/`KEY_VOLUMEDOWN` erkannt, siehe Abschnitt 1).
Nach Senden von `5a 39 03 00 1c 01` (`special::device_setting(setting_id
::VOLUME_SYNC, true)`, byte-identisch aus einem frischen Windows-
Capture) beginnt das Gerät sofort, auf 0x84 zu senden – live per
`evtest` bestätigt (Flut von `KEY_VOLUMEUP`-Events beim Drehen).
**Deshalb am 2026-08-24 wieder in `main.rs` eingebaut** (Startblock,
zusammen mit Power-Saving/Logo-LED) – volle Vorgeschichte inkl. der
2026-08-23-Entscheidung, es zu entfernen, siehe
[HANDOFF-REVERTED.md](HANDOFF-REVERTED.md).

**Einschränkung, noch ungelöst:** Diese 6-Byte-HID-Reports codieren
selbst **keine Richtung und keinen Wert** – Auf- und Zudrehen erzeugen
byte-identische Reports (`000100020001`/`000101020001`/`000102020001`,
per direktem Vergleich Windows-Capture bestätigt). Linux' generische
Interpretation als `KEY_VOLUMEUP` (nie `KEY_VOLUMEDOWN`) ist deshalb
einseitig kaputt – ein Test trieb `wpctl` bis zum Anschlag auf 100 %,
ohne je wieder runterzukommen.

**Zusätzlich beobachtet (Live-Test 2026-08-24, nach Neustart mit dem
VOLUME_SYNC-Fix):** Der Regler tut jetzt tatsächlich etwas unter Linux
(Nutzer bestätigt: "der Regler tut unter Linux"), aber mit hörbarem
Knacken im Kopfhörer/Lautsprecher genau bei größeren Lautstärke-
Sprüngen (z.B. 50 %→55 %) – nicht bei jedem Schritt. Erklärung: die
generische `KEY_VOLUMEUP`-Behandlung (welcher Systembestandteil das
genau macht, ist nicht identifiziert) wendet vermutlich einen festen
~5%-Schritt pro Tastenereignis hart an, ohne sanfte Überblendung –
unabhängig davon, wie fein tatsächlich gedreht wurde. Spricht dafür,
dass die Lösung für die obigen drei offenen Punkte (Richtung/Wert
direkt lesen statt über die generischen Tastenereignisse) vermutlich
auch das Knacken mit behebt, da dann feingranular statt in groben
5%-Sprüngen synchronisiert werden könnte.

**Den echten Wert liefert Windows stattdessen über einen zweiten,
unabhängigen Mechanismus**, gefunden per genauer Analyse von
`vol min-max-min_mit sync.pcapng` (derselbe Capture wie für
`TAPER_TABLE`, siehe unten): direkt nach jedem 0x84-Report folgt ein
Burst aus 6 Control-Transfers auf Endpoint 0x80, **Standard-USB-Audio-
Class GET_CUR** (kein Vendor-Protokoll!):

```
bmRequestType: 0xa1 (Class, Interface, Device-to-Host)
bRequest:      0x01 (CUR)
wValue:        0x0201 / 0x0202  (Control Selector 0x02 = VOLUME_CONTROL,
                                  Channel 0x01 = Links / 0x02 = Rechts)
wIndex:        0x0100           (Entity ID 0x01 = Feature Unit, Interface 0)
wLength:       2
```

Antwort: 2 Bytes, letztes Byte trägt den eigentlichen Wert. Über den
kompletten min→max→min-Sweep extrahiert ergibt das eine glatte,
monotone Kurve (196 → … → 255 (Maximum, exakt in der Sweep-Mitte) → …
→ 196) mit **derselben Audio-Taper-Charakteristik** wie der ALSA-
Rohwert (steiler Anstieg am unteren Ende, z.B. 196→220 in einem
Schritt, dann feine ±1-Schritte nahe 255) – zusätzliche Bestätigung,
dass das wirklich der Lautstärkewert ist, nicht Zufall.

**Sackgasse beim Nachbauen:** `pyusb`/`libusb` kann diese Anfrage unter
Linux nicht selbst parallel schicken – `Resource busy`, weil
`snd-usb-audio` das Audio-Interface bereits exklusiv hält (Interface
kernseitig detachen würde die echte Audiowiedergabe unterbrechen, keine
Option). Das erklärt vermutlich auch, warum `amixer -c USB1 sget
Speaker` weiterhin einen alten, eingefrorenen Wert zeigt, obwohl der
Regler nachweislich funktioniert (KEY_VOLUMEUP-Flut lief): Linux'
eigener Kernel-Treiber scheint dieses Feature-Unit-Register nicht bei
jeder `amixer`-Abfrage frisch vom Gerät zu lesen, sondern einen
Software-Schattenwert zurückzugeben, der nur bei einer echten
Host-initiierten Neuabfrage (wie Windows sie aktiv macht) aktualisiert
wird – noch nicht abschließend verifiziert, ob das tatsächlich der
Mechanismus ist oder ob es einen anderen Weg gibt, den Kernel zu dieser
Neuabfrage zu bewegen (z.B. ein `alsactl`-Befehl oder ein Control-Event,
das wir noch nicht kennen).

**Offen für die nächste Sitzung:**
1. Richtungserkennung: entweder einen Weg finden, aus den 0x84-Reports
   (oder woanders) die tatsächliche Dreh-Richtung zu lesen, oder ganz auf
   sie verzichten und stattdessen periodisch den echten Wert lesen
   (siehe Punkt 2).
2. Einen Weg finden, den aktuellen Wert unter Linux zu lesen, OHNE mit
   `snd-usb-audio` in Konflikt zu geraten – z.B. prüfen, ob es einen
   ALSA-Mechanismus gibt, der den Kernel-Treiber zu einer frischen
   Hardware-Abfrage zwingt (statt den gecachten Wert zu liefern), oder ob
   `amixer`/die `alsa`-Crate das schon können und wir nur den falschen
   Aufruf benutzt haben.
3. Falls beides nicht geht: das könnte tatsächlich tiefer sitzen als
   App-Ebene (Kernel-Treiber-Verhalten von `snd-usb-audio` für dieses
   Gerät) – vom Nutzer selbst als Möglichkeit genannt, siehe
   Gesprächsverlauf 2026-08-24.
4. **Vielversprechender Ansatzpunkt, noch nicht ausprobiert:** `main.rs`
   verwirft an beiden Stellen, wo `read_report()` aufgerufen wird
   (Schalterstand-Poll, Firmware-Check), erstmal alle anstehenden Reports
   (`while d.read_report(20)... {}`), bevor die eigene Anfrage gesendet
   wird – der von `device.rs` dokumentierte periodische Status-Heartbeat
   (kompletter Parameter-Dump alle ~1,5-2,5s, sobald das Gerät "aktiv"
   ist) wird dadurch NIE ausgewertet, nur weggeworfen. Dieser Kanal
   (Interface 4, EP 0x85, unser eigener Vendor-Kanal) ist NICHT vom
   `snd-usb-audio`-Kernel-Treiber blockiert (anders als das Standard-UAC-
   Interface 0, siehe "Resource busy" oben) – falls der Heartbeat-Dump
   den aktuellen Lautstärkewert enthält (noch nicht geprüft), wäre das
   ein Weg, den Wert zu lesen, ohne mit dem Kernel-Treiber zu
   kollidieren. Nächster Schritt: einmal den Heartbeat mitschneiden
   (`read_report()` NICHT verwerfen, sondern loggen) und mit dem
   physischen Reglerstand vergleichen.

### Nachtrag 2026-08-24: Direct-Mode-Status-Bitfeld, teilweise gefunden

Bei der Suche nach dem Status-Antwortformat (`5a 39 01 02` → `5a 39 09
02 ...`, siehe Abschnitt 2) im frischen `Directmode ein-aus.pcapng`
zeigte sich: **dieser `09 02`-Reply ist NICHT der Direct-Mode-Status** –
er bleibt bei ein- und ausgeschaltetem Direct Mode identisch (`20 64 02
14` in beiden Fällen, je zweifach bestätigt). Die bisherige Vermutung
in Abschnitt 2 war falsch, jetzt korrigiert.

Stattdessen fiel ein anderes, bisher undokumentiertes Muster auf:
direkt nach jedem `5a 39 03 00 05 <01|00>` (Direct-Mode-Toggle) sendet
das Gerät dreimal `5a 39 05 01 ...`, wovon die zweite Nachricht
zuverlässig Byte[4] zwischen `0x20` (Direct Mode an) und `0x00` (Direct
Mode aus) ändert (Frames 537/544/549 vs. 1247/1251/1255, je identisch
innerhalb der Phase). **Einschränkung:** Dieses Muster kommt in KEINEM
der anderen Einstellungs-Captures vor (Logo, Energiesparen, SPDIF,
Volsync) – kein Quervergleich möglich, um die Bitfeld-Hypothese zu
erhärten oder andere Bit-Positionen anderen Einstellungen zuzuordnen.
Müsste live nachgetestet werden (`5a 39 05 01`-Traffic mitloggen,
verschiedene Einstellungen durchschalten, schauen ob/welches Bit
kippt) – ähnlich dem bereits vorbereiteten Diagnose-Logging für die
`09 02`-Antwort, nur auf `05 01` umgezielt.

### Nachtrag 2026-08-26: physischer MODE-Knopf löst echten USB-Reconnect aus (GitHub-Issue #1)

**Durchbruch, live bestätigt:** Der physische MODE-Knopf am Gerät löst
bei jedem einzelnen Druck eine **echte USB-Neuinitialisierung** aus
(Disconnect + Reconnect, `journalctl -k` zeigt `USB disconnect` gefolgt
von `New USB device found` für die G8), kein reines Register-Write wie
bei der Software-Umschaltung. Live 1:1 verifiziert: Nutzer drückte
~alle 2s wiederholt, Kernel-Log zeigte im selben Rhythmus einen
Disconnect/Reconnect-Zyklus pro Druck. Passt zur unabhängigen
Beobachtung des Nutzers unter Windows: die Original-Software zeigt
beim Knopfdruck sichtbar einen kurzen "Reset" der App-Verbindung, dann
Anzeige des neuen Direct-Mode-Zustands.

**Erklärt mehrere bisherige Beobachtungen auf einen Schlag:**
- Warum das Auslesen eines Status-Bitfelds per Diagnose-Logging so
  unzuverlässig war – das Gerät ist im fraglichen Moment oft gerade
  mitten im Reconnect, nicht stabil ansprechbar.
- Warum frühere Live-Tests (2026-08-24) beim Direct-Mode-Knopfdruck
  gemischte/keine Ergebnisse lieferten.

**Wichtige Nebenerkenntnis (Vorsicht für künftige Diagnose-Threads):**
Beim Versuch, dies robuster zu greifen, wurde testweise ein zweiter,
dedizierter Thread mit einem EIGENEN zweiten `G8Device`-Handle auf
Interface 4 eingebaut (parallel zum bereits bestehenden Schalterstand-
Poll-Thread, der ebenfalls ein Handle auf Interface 4 hält). Das löste
selbst einen Disconnect-Sturm aus (alle 1-5s, über mehrere Minuten,
sofort nach App-Start, sofort vorbei nach App-Stop) – vermutlich
Konflikt zweier gleichzeitiger blockierender `hidapi`-Lesevorgänge auf
demselben Interface. **Lehre: nur EIN Geräte-Handle pro Interface
gleichzeitig verwenden**, zusätzliche Diagnose-Logik in einen bereits
bestehenden Poll-Thread einbauen statt einen neuen mit eigenem Handle
zu starten. Der entsprechende Code wurde sofort wieder entfernt, aktuell
läuft die Diagnose sicher im bestehenden Schalterstand-Poll-Thread mit.

**Nächster Schritt für die Status-Rücklese (Issue #1):** Statt einen
Wert WÄHREND einer stabilen Verbindung abzufragen, wahrscheinlich
sinnvoller: auf das Reconnect-Ereignis selbst reagieren (die App hat
bereits Reconnect-Erkennung für den `special::reset_device()`-Fall,
siehe Abschnitt 5a) und direkt NACH einem erkannten Reconnect eine
gezielte Statusabfrage senden – der neue Direct-Mode-Zustand sollte zu
diesem Zeitpunkt bereits gültig sein, auch wenn die genaue Kodierung
noch nicht sicher entschlüsselt ist.

**Unabhängig im Windows-Capture bestätigt:** Im Mitschnitt
`2026-08-24_absturz-dm-reset-sequenz.pcapng` beginnt die G8
(Geräteadresse 5) ca. 5-6s nach der im Notizen-Zeitstempel vermerkten
Marke "via Tastendruck" (Frame 2453, ~197,0s) eine vollständige
USB-Neuenumeration – `GET_DESCRIPTOR`-Anfragen für Device-,
Configuration- und mehrere Class-spezifische Deskriptoren (Frames
2473-2478, ~202,8s). Kein spezieller "Status"-Report dabei, reine
Standard-Enumeration. Der eigentliche Direct-Mode-Zustand wird dem
Original-Client vermutlich über denselben periodischen Status-Heartbeat
mitgeteilt, der laut `device.rs`-Kommentar automatisch losläuft, sobald
das Gerät nach dem Reconnect wieder "aktiv" wird – passt zur oben
dokumentierten `5a 39 05 01`-Burst-Beobachtung direkt nach jedem
Direct-Mode-Wechsel.

### Nachtrag 2026-08-26 (2): Warnung – `5a 39 01 00 05` ist KEIN sicheres GET, hat Endpoint blockiert

Beim Versuch, live nach einem Status-GET für Issue #1 zu suchen, wurde
`5a 39 01 00 05` roh über `/dev/hidraw` (Interface 4, Vendor-Kanal)
gesendet, während die App lief bzw. kurz danach. Beobachtete Folgen,
in dieser Reihenfolge:

1. Hörbarer physischer "Klick" am Gerät selbst (Nutzer-Beobachtung) –
   bisher im Projekt immer ein Zeichen für eine ECHTE Zustandsänderung,
   nie für eine reine Abfrage.
2. Der physische MODE-Taster reagierte danach nicht mehr normal bzw.
   wurde sofort wieder zurückgesetzt ("der GET blockiert den
   Tastendruck bzw. macht ihn wieder direkt rückgängig" – Nutzer-O-Ton).
3. Kurz danach eigenständige Disconnect/Reconnect-Schleife am Gerät
   (Device-Nummer 30→31→32 innerhalb von ~20s, ohne dass in diesem
   Moment ein Kommando gesendet wurde).
4. Danach lehnte der Endpoint JEDEN weiteren Schreibversuch mit
   `Broken pipe` (USB-Stall) ab – auch den bekannten, sonst
   zuverlässigen SET-Befehl `5a 39 03 00 05 01` (Direct Mode an).
5. Erst nach einem physischen Stromverlust von ca. 15s (Kabel gezogen)
   kam das Gerät wieder in einen sauberen Zustand zurück – interessant:
   der zuvor gesendete SET-Befehl war offenbar im Stack hängen
   geblieben und wurde erst nach dem Reconnect zugestellt (Direct Mode
   war danach tatsächlich an, wie vom Nutzer beabsichtigt).

**Schlussfolgerung:** `5a 39 01 00 05` (und vermutlich die gesamte
`5a 39 01 ...`-Namensraum-Familie mit ungewöhnlicher Byte-Struktur) ist
NICHT als harmloses GET zu behandelt – es kann den Vendor-Endpoint in
einen blockierten Zustand bringen, der nur durch einen echten
Stromzyklus behebbar ist. **Für künftige Protokoll-Exploration: keine
weiteren rohen Kommando-Kandidaten per Trial-and-Error an dieses
Interface senden, solange die App parallel läuft oder das Gerät gerade
erst aus einem Reconnect kommt.** Passiv aus vorhandenen Captures
ableiten (siehe Windows-Mitschnitte) ist der sicherere Weg zur
Status-Rücklese für Issue #1.

### Nachtrag 2026-10-25: Direct-Mode-Statusabfrage in den eigenen Windows-Captures gefunden (Issue #1)

Anlass: Abgleich mit dem Upstream-Repo `RizeCrime/linuxblaster_control`
(G6, nicht G8, aber dieselbe `5a`-Protokollfamilie; `UsbProtocol.md` v5,
Stand 2026-10-12, gegen Hardware verifiziert). Daraus übernommenes
Schema: Byte 3 ist bei `0x2c`/`0x3c`/`0x6c` ein Richtungs-Flag
(`00`=schreiben, `01`=lesen, `02`=aufzählen). Mit dieser Brille ist
`5a 39 05 01 ...` in unseren G8-Captures eindeutig eine **Lese-Anfrage**
der Original-Software, kein Schreibbefehl.

**Befund (rein offline, aus allen 157 Captures, nichts ans Gerät gesendet):**
- Anfrage der Windows-Software: `5a 39 05 01 20 00 00 00` (danach
  zweimal `5a 39 05 01 00 04 00 00`). Die Antwort ist in allen Fällen
  `5a 39 05 01 [b4] [b5] [b6] [b7]`, unabhängig von den Anfrage-Bytes
  (Zustandsfeld, keine Maske).
- **`b4 & 0x20` = Direct Mode.** `Directmode ein-aus.pcapng`: ein →
  `20 00 00 10`, aus → `00 00 00 10`. `2026-08-24_absturz-dm-reset-sequenz.pcapng`:
  `00 40 00 00` (aus, 64,9s) → `20 40 00 00` (ein, 127,9s) →
  `00 40 00 00` (aus, 171,7s). In allen Reset-/Schritt-Captures
  durchgehend `00 40 ..` = Direct Mode aus.
- `b5 & 0x40` und `b7 & 0x10` sind weitere, noch unbenannte Zustandsbits
  (b5=0x40 meist gesetzt; b7=0x10 mal gesetzt, mal nicht).
- Die Windows-Software sendet diese Abfrage **ausschließlich direkt nach
  der Neu-Initialisierung** (`5a 09 01 02/03/00 ...`-Handshake) – nie
  während einer stabilen Verbindung. Das passt exakt zum Fund vom
  26.08., dass der MODE-Knopf eine echte Neu-Enumeration auslöst: Gerät
  verschwindet, kommt wieder, Software initialisiert neu und liest dann
  den Zustand.

**Umsetzung für Issue #1 (noch offen):** Nach Start der App und nach
jedem erkannten Reconnect genau einmal `5a 39 05 01 20 00 00 00` senden
(im bestehenden Poll-Thread, kein zweites Handle!), Antwort-Byte 4 mit
`0x20` maskieren, `direct-mode-on` in der UI setzen. Kein Dauer-Polling.

**Erklärung für den Klick vom 26.08.:** In *keinem* der 157 Captures
sendet die Original-Software jemals `5a 39 01 00` oder `5a 39 01 01`;
als Längen-1-Unterkommando kommt nur `5a 39 01 02` vor (50 Mal).
Upstream führt Sub `01` als "Commit" und Sub `04`/`05` als abgelehnt
(ACK-Status `0x81`, deckt sich mit unserer Beobachtung). Sub `00` ist
undokumentiert und hat offenbar eine echte Wirkung. **Regel: an dieses
Interface nur Byte-Folgen senden, die in einem Windows-Capture
nachweisbar vorkommen.**

**Weitere Erkenntnisse aus dem Abgleich:**
- ACK-Statusbyte (`5a 02 0a [cmd] [status]`): `00`=OK, `80`/`81`/`83`=
  abgelehnt. Unsere "akzeptierte" Antwort `5a 02 0a 39 00 ..` auf den
  Fuzz-Versuch hieß also "ausgeführt", nicht "gelesen".
- `sbx_sync()` (`5a 26 03 08 ff ff`) ist laut Upstream **kein Toggle**,
  sondern die Abfrage des Global-Profile-Bitmasks (Bit0=SBX aktiv,
  Bit1=Scout, Bit2=Aufnahme-DSP). Der Kommentar in `protocol.rs` ist
  irreführend. Auf dem G8 liefert er in `Directmode ein-aus.pcapng`
  konstant `0x05`, egal ob Direct Mode an oder aus – taugt also nicht
  als Direct-Mode-Anzeige.
- Issue #7 (undekodierte Namespaces): `0x26` = Global-Profile/SBX/Scout,
  `0x10` = Seriennummer, `0x6c` = DAC-Filter (die G8-Antwort auf
  `5a 6c 01 02` hat dasselbe Format wie beim G6, 5 Einträge – ein
  Filter-Auswahlfeld wäre möglicherweise auch beim G8 machbar, ungeprüft).
  `0x09` kommt im G6-Dokument nicht vor (G8-spezifischer Identify-
  Namespace). Genaue Tabellen: `UsbProtocol.md` im Upstream-Repo.
- Upstream bestätigt: hidraw-Backend vermeidet den USB-Reset, den
  libusb braucht. Wir nutzen bereits hidraw (`ldd` zeigt nur libudev).
- Upstream-Mixer öffnet den ALSA-Mixer pro Aufruf (`alsa`-Crate),
  keine persistente Verbindung – für Issue #8/#14 also keine fertige
  Vorlage, aber ein Beleg, dass die Mixer-Controls der Karte direkt
  ansprechbar sind.

### Nachtrag 2026-10-25 (2): Fehlerbehebungen, Direct-Mode-Abfrage eingebaut, Neustart-Schleife im Direct Mode (Issue #1)

**Behoben (Issues #11-#14):**
- **Slint-Bindungsfalle (wichtigste Lehre):** `prop: parent-prop;` ist nur
  eine EINSEITIGE Bindung. Sobald das Kind die Property selbst beschreibt
  (Slider ziehen, Dropdown wählen, `current-index`/`checked`/`value`),
  wird die Bindung ersetzt, spätere Änderungen von Rust oder aus anderen
  Tabs kommen nicht mehr an. Für Zustand, der von mehreren Stellen
  geschrieben wird, immer `<=>` verwenden. Betraf: AppWindow→DeviceHubTab
  (alle `ae-*`, Preset-Indizes, `footer-*-level`), Sound-Modus-/EQ-
  Dropdowns, Mixer-Zeilen, Seitenleisten-Slider, Direct-Mode-Schalter.
  Acoustic-Engine-Slider (`value: x`) sind laut Nutzertest nicht betroffen.
- Dark Mode beim Start (#13): `init =>` feuerte in `AppWindow::new()`,
  vor `set_dark_mode_on()`. Jetzt `public function apply-dark-mode()`,
  von main.rs einmal nach dem Setzen aufgerufen.
- EQ-Preset (#11/#12): neuer persistierter `eq_preset_index`
  (Altdateien: aus dem gespeicherten Sound-Modus abgeleitet). Beim
  Wiederherstellen setzt Rust `eq-skip-next-apply`, damit der
  `changed selected-preset-index`-Handler die gespeicherten Bänder nicht
  überschreibt. Manuelle EQ-Auswahl setzt den Sound-Modus auf "Im Gerät"
  (wie im Original, vom Nutzer bestätigt). `CompactComboBox`: Popup in
  ScrollView (max. 260px), Farben folgen dem Dark/Light-Modus.
- Mixer (#14): siehe Bindungsfalle. Zusätzlich war `proc_util::
  run_with_timeout` die Ursache für träges Reagieren auf den
  Lautstärkeknopf: `pactl list cards` (alle ~2s im Mixer-Poll-Thread)
  blockierte in `anon_pipe_write`, weil stdout erst nach `try_wait()`
  gelesen wurde; Folge: alle 3s Timeout, Poll stand still. Jetzt lesen
  zwei Threads stdout/stderr parallel, seitdem 0 Timeouts.
- **Installationsfalle:** Der Starter (`Exec=g8-control`) nutzt
  `~/.local/bin/g8-control`, NICHT `target/release/`. Nach jedem Build dort
  hinein kopieren, sonst testet man die alte Version (am 25.10. passiert;
  Sicherung der alten Datei: `g8-control.bak-2026-08-25`).

**Direct-Mode-Abfrage (Issue #1), eingebaut:** `special::query_device_state()`
(`5a 39 05 01 20 ...`) und `direct_mode_from_reply()` (Byte 4, Bit 0x20),
mit Unit-Test. Im bestehenden Schalterstand-Poll-Thread (kein zweites
Handle) einmal 1,5s nach jedem (Neu-)Öffnen des Handles, bis zu 3
Versuche, Ergebnis nach `direct-mode-on`. Live bestätigt: Register meldet
`20 40 00 10` (an) / `00 40 00 10` (aus), deckt sich mit dem Schalter.
Schalter ist nach einem Umschalten gesperrt (`direct-mode-busy`), bis die
Abfrage nach der Neuverbindung zurückkommt (Sicherheitsnetz: 8s), weil
ein Klick mitten in der Neu-Enumeration das Umschalten sofort umkehrte.
Jeder Schreibbefehl (auch einer ohne Wertänderung) löst eine
Neu-Enumeration des Geräts aus (~0,1s später Disconnect, ~1s später
wieder da).

**(GELÖST am 2026-10-26, siehe Nachtrag direkt darunter: Ursache war `alsactl restore`.)**
**Neustart-Schleife im Direct Mode.** Bei eingeschaltetem Direct
Mode startet das G8 unregelmäßig von selbst neu (Disconnect/Reconnect),
oft alle 2,4-3,7s, und kommt dabei mit Direct Mode = an zurück – das ist
das "kurz an, dann wieder aus" der LED. Messreihe 2026-10-25 (immer
App gestoppt oder irrelevant, Gerät am gespeisten 4-Port-Hub, Bus 5):
- Mit App, ohne App, ohne App und ohne WirePlumber: Schleife tritt auf
  (ohne WirePlumber 5 Neustarts in 45s). Ohne Direct Mode über Minuten
  stabil. Unregelmäßig: manche Läufe auch 40-80s stabil.
- Windows (laut Nutzer): Direct Mode bleibt dauerhaft an bzw. aus, per
  Software und per Taste. Unklar, ob am selben Hub/Port gemessen.
- Linux-usbmon-Mitschnitt (`captures/linux/verbinden-trennen/
  2026-10-25_dm-ein-stabil-vs-neustart_usbmon5.pcapng`): Eine Enumeration,
  die nach 3,8s starb, und eine, die >80s lebte, haben in den ersten 2s
  BYTE-IDENTISCHE Steuerverkehr (570 Transfers inkl. Antwortdaten:
  `SET_INTERFACE`-Schleife, Takt-/Lautstärke-Abfragen bei +0,9..1,15s,
  danach Funkstille). Der Tod kommt ohne Host-Aktivität in den letzten
  ~2,6s. Kernel-Log: keine Hub-/Port-/Over-Current-Meldungen. Gerät
  `power/control=on`.
- **`dumpcap` selbst stört:** Beim Start des Mitschnitts gab es zweimal
  hintereinander einen Disconnect (~1s nach Start), und das Gerät kam
  mit Direct Mode = an zurück. Mitschnitte unter Linux sind also
  verfälscht; nur zur Verlaufsanalyse nutzen.
- Offene Hypothesen: (a) Windows hält im Direct Mode ständig einen
  Audio-Stream offen, Linux hat danach Funkstille (Neustart ~2,5s nach
  letztem Verkehr) – ungeprüft; ein laufender Stream ließ sich wegen der
  `dumpcap`-Störung nicht sauber testen. (b) Host-/Controller-spezifisch
  (AMD Raphael USB2 xHCI `0f:00.0`, zwei Hubs hintereinander).
- **Nächster Schritt:** Windows-Mitschnitt am GLEICHEN Hub/Port, Direct
  Mode an, 60s idle, einmal ohne und einmal mit Musik; vergleichen, ob
  Windows im Leerlauf Datenverkehr erzeugt. Unter Linux Direct Mode mit
  laufendem Audio testen, ohne `dumpcap`.

### Nachtrag 2026-10-26: Ursache für "Direct Mode geht wieder aus" und die Neustart-Schleife gefunden und behoben (Issue #1)

**Ursache: `alsactl restore` (udev-Regel `90-alsa-restore.rules`).** Bei JEDER
Registrierung der G8-Soundkarte schreibt es die zuletzt gespeicherten
ALSA-Mixerwerte zurück, darunter `PCM Capture Source` (UAC-Selector `0x11`,
gespeichert: "Line In") und `Speaker Clock Source` (UAC-Clock-Selector `0x0e`,
gespeichert: "DSP Clock"). Im Direct Mode steht `Speaker Clock Source` auf
**"Stereo Direct"** (Selector-Wert 2) und `PCM Capture Source` auf Wert 2:
Das ist der echte Direct-Mode-Zustand. Weil jeder Direct-Mode-Wechsel das Gerät
neu am USB anmeldet, die Karte danach neu registriert wird und die Wiederherstellung
den Takt zurückschaltet, fällt der Modus nach ca. 1 s wieder aus. Das HID-Flag
(`5a 39 05 01`, Bit 0x20) bleibt dabei gesetzt: Flag = angeforderter Modus, nicht
tatsächlicher Zustand. Mit gesetztem Flag schaltet das Gerät beim nächsten Hochfahren
wieder in den Direct Mode, wird wieder zurückgesetzt, usw.: das war die
**Neustart-Schleife alle 2,4-3,7 s** (und die Störgeräusche: Audio lief gegen
einen zurückgeschalteten Takt). Windows schreibt diese Selektoren nie zurück.

Nachweis (alles bei laufendem Gerät, ohne `dumpcap`):
- Kernel-Treiber nur von Interface 0 abgehängt (Karte nicht registriert): Direct Mode
  bleibt an. Treiber wieder angehängt (Karte registriert, Restore läuft): LED aus,
  ohne Disconnect, Flag bleibt gesetzt.
- Einzelschritte des Kartenaufbaus nachgespielt (SET_INTERFACE-Folge, Lautstärke-
  Schreiben, nur Lesen, 384-kHz-Takt auf Einheit `0x10`): keiner schaltet aus.
  Capture-Selector `0x11` von 2 auf 1 geschrieben: LED aus, sofort.
- Udev-Ausnahme nur für das G8 (`041e:329b`, `controlC*`) vor dem
  `alsactl restore`: Direct Mode bleibt an, 0 Neustarts über Minuten, Audio sauber,
  `Speaker Clock Source = Stereo Direct`. Der Versuch mit `OPTIONS+="last_rule"` war
  wirkungslos (wird von neuem udev ignoriert: "Invalid value for OPTIONS key").
- Windows-Captures (`SB G8/von windows/`): pro Direct-Mode-Wechsel genau eine
  Neuanmeldung, danach Stabilität; Windows schreibt nach der Anmeldung selbst
  `SET_CUR 0x0e = 2` (Stereo Direct) und alle Mixer-Defaults, nie `SET_INTERFACE`.

**Fix:** Kopie der Systemregel nach `/etc/udev/rules.d/90-alsa-restore.rules` mit
einer Sprungzeile vor dem ersten Regelblock:
`ACTION=="add", SUBSYSTEM=="sound", KERNEL=="controlC*", ATTRS{idVendor}=="041e", ATTRS{idProduct}=="329b", GOTO="alsa_restore_end"`.
Im Installer (`packaging/install.sh`) als zusätzlicher Schritt mit Rückfrage,
Vor-/Nachteilen und Rückgängig-Hinweis (Nachteil: Kopie der Systemregel, spätere
alsa-utils-Updates wirken für diese Datei nicht; kein Mixer-Restore für das G8,
das Gerät behält seine Werte selbst).

**Objektive Anzeiger für den echten Direct-Mode-Zustand** (zusätzlich zum HID-Flag):
ALSA-Regler `Speaker Clock Source` (`numid` kann variieren, per Name suchen): "Stereo
Direct" = an. Per USB: Clock-Selector `0x0e` (`a1 01 wV=0100 wI=0e00`),
Capture-Selector `0x11`, Takt-Einheit `0x10` (384000 Hz im Direct Mode).
Der GET_RANGE der Takt-Einheit `0x0f` ist KEIN Anzeiger (immer bis 48 kHz).

**Weitere Befunde der Windows-Session (2026-10-26), aus ihrer HANDOFF-Kopie auf dem
Austauschlaufwerk (`SB G8/g8-scaffold/HANDOFF.md`, veraltete Basis, nur die
Nachträge zählen):** Weitere Statusbits in `5a 39 05 01`: `b5 & 0x40` Logo-LED,
`b6 & 0x02` Energiesparen, `b5 & 0x20` SPDIF-Direct (dabei zusätzlich `5a 6e 02 01`),
`b7 & 0x10` Volume Sync. `5a 39 01 01` liefert dieselben Statusflags. Nach jeder
USB-Enumeration schickt das Gerät ~1,1 s später ohne Host-Befehl das Trio
`6e`/`2c`/`1a` plus vollen Parameter-Dump (außer bei Direct Mode an). Fehlerquittung
`5a 02 0a <fam> <code>`: `0x81` unbekannte Sub-ID, `0x80`/`0x83` Befehl existiert,
aber nicht als GET lesbar. Neue Mitschnitte und Skripte liegen unter
`SB G8/von windows/2026-10-25-*/2026-10-26-*` (Skripte: `g8_query.py` u. a., NIE
senden: `5a 9b ...` Reset, Header `3e`/`3f` Firmware-Kanal).

**Lehren:**
- `modprobe -r snd_usb_audio` hält nur bis zur nächsten Neuanmeldung des G8 (Autoload).
- `dumpcap` löst beim Start einen Disconnect aus; für UAC-Verhalten besser Schritt für
  Schritt per `pyusb` nachspielen (Interface 0-2 abhängen, einzelne Requests senden,
  danach `attach_kernel_driver`).
- Zwei "unabhängige" Symptome (Schleife und Störgeräusche) hatten dieselbe Ursache.

### Nachtrag 2026-10-26 (2): Sprache (#4, #5, #6) und Mixer (#8) erledigt

- **Sprachumschalter** (Einstellungen -> "Sprache": Automatisch/Deutsch/English/Français), wirkt sofort per
  `slint::select_bundled_translation()`, gespeichert in `settings.txt` (`language=0..3`). "Automatisch"
  wertet `LANGUAGE`/`LC_ALL`/`LC_MESSAGES`/`LANG` aus (unbekannte Sprache -> Englisch). Slint wählt beim Start
  ohnehin schon per `sys_locale` (deshalb war #5 praktisch schon erfüllt). Neue Texte: `slint-tr-extractor -d
  g8-control --package-name g8-control --package-version 0.1.1 -o lang/g8-control.pot $(find ui -name '*.slint')`,
  dann Einträge in `lang/make_po.py` ergänzen und das Skript ausführen.
- **Build-Falle:** Cargo bemerkte geänderte `.po`-Dateien nicht (Build in 0,09 s, alte Texte). `build.rs` gibt jetzt
  `cargo:rerun-if-changed=lang` aus.
- **#6 Rust-Texte:** Es gibt nur einen nutzersichtbaren Rust-Text (Meldung "G8 nicht gefunden"); er liegt jetzt als
  `@tr()` in `app-window.slint` (`device-search-failed`). Alle `eprintln!`-Ausgaben sind Entwickler-Logs und bleiben
  bewusst deutsch.
- **#8 Mixer:** `get_level`/`set_level`/`set_balance` nutzen eine pro Thread dauerhaft geöffnete ALSA-Verbindung
  (`alsa`-Crate, einfacher Mixer, `handle_events()` vor jedem Lesen), die bei Fehlern (Karte nach einer
  Neuanmeldung weg) verworfen und neu geöffnet wird. Keine `amixer`-Prozesse mehr im Leerlauf. Stumm und
  Aufnahmequelle laufen weiter über `amixer` (seltene Klicks). Live-Test: `cargo test -- --ignored mixer_roundtrip`.
  Neue Build-Abhängigkeit: ALSA-Entwicklungsdateien (`libasound2-dev`/`alsa-lib`).

### Nachtrag 2026-10-26 (3): Windows-Mitschnitte zu 💾/↺, Ausgabewechsel, Verstärkung und Rückseiten-Schalter

Mitschnitte: `SB G8/von windows/2026-10-26-Wireshark-mitschnitte/` (`soundmodus speichern und so`, `eq tab speichern und so`,
`steckerwechsel mit eq preset`, `hp in mic`, `gain h-l`, `schalter an der rückseite`, Screenshot "Neuen Modus hinzufügen").
- **💾 = "Neuen Modus hinzufügen"** (Dialog "Neu speichern", Vorgabename "Neuer Modus", Abbrechen/OK). Dabei und beim
  Zurücksetzen (↺) gibt es **keinen neuen Befehl** an das Gerät: nur die üblichen `5a 12 07 01 96 <param> <f32>`-Schreibzugriffe.
  Einen Modus anwenden (Preset wählen, ↺) = Burst aus allen Acoustic-Engine-Parametern (`96:00..08, 18, 02`), den EQ-Bändern
  (`96:0b..14`) und dem Preamp (`96:0a`) in ca. 0,1 s. Beispiele im Mitschnitt: "Gaming" (Surround an 0,12, Bänder
  `3,4,-1,-2,0,2,2,-5,2,0`) und "Effects Off" (alles aus, Surround-Level 0,67). Eigene Modi liegen also **lokal in der Creative-App**,
  nicht im Gerät -> bei uns lokal in der App nachbaubar.
- **Ausgabewechsel (Stecker/Kopfhörer<->Lautsprecher):** Das Gerät schickt den vollen Parameter-Dump (`5a 11 32 08 ...`) plus
  `5a 6e`/`5a 1a`, etwa 2 s später schreibt die **Windows-App den gespeicherten EQ-Preset selbst wieder** ins Gerät (Burst `96:0b..14,0a`).
  Das Gerät hält nur EIN Kurvenset; die pro-Ausgang-Kurven (Issue #9, Kopfhörer-/Lautsprechersymbol unten im EQ-Tab) sind also
  eine reine App-Funktion, die bei jedem Ausgabewechsel neu eingespielt wird.
- **Verstärkung (`gain h-l`) und Rückseiten-Schalter:** In den Mitschnitten gibt es **weder `5a`-Befehle noch UAC-`SET_CUR`
  oder G8-Interrupts**. Beides ist reine Hardware, ohne für den Host sichtbare Protokolländerung (nichts nachzubauen).
- "Bearbeiten" am Geräte-Bild blendet nur Kacheln der Hauptseite ein/aus (laut Nutzer, nicht nachzubauen).

### Nachtrag 2026-10-26 (4): Version 0.2.0 - eigene Sound-Modi, Logos im Dark Mode

- **Eigene Modi** (Sound-Modus-Tab): 💾 öffnet eine Zeile "Neuen Modus hinzufügen" (Name, OK/Abbrechen), speichert die aktuellen
  Acoustic-Engine-Werte und die EQ-Kurve lokal in `~/.config/g8-control/modes.txt` (`Name<TAB>23 Werte`, siehe `src/modes.rs`);
  ↺ wendet den gewählten Modus erneut an; 🗑 (nur bei eigenen Modi, mit Rückfrage) löscht ihn, eingebaute Presets sind nie löschbar.
  Dropdown-Reihenfolge: "Im Gerät", 60 eingebaute Presets, dann eigene Modi; Rust füllt die Liste (`refresh_mode_names`),
  Index = Preset-Index + 1, eigene Modi ab `PRESETS.len()`. Eigene Modi haben `eq_preset_index = -1` (keine benannte EQ-Kurve,
  Kurve wird direkt gesetzt und gesendet). Im Original entspricht das "Neuen Modus hinzufügen" (kein Gerätebefehl, siehe Nachtrag 3).
- **Logos im Dark Mode:** die SVG-Logos (CrystalVoice, Direct/Scout Mode, Dolby, Lautsprecher-Grafik) werden per `colorize` mit
  `Palette.foreground` eingefärbt, im Hell-Modus unverändert.
- Version `0.1.1` -> `0.2.0` (neue Funktionsgruppen seit 0.1.1: Sprachwahl, ALSA-Mixer, Direct-Mode-Statusabfrage und -Fix, eigene Modi).
- **Ausgabewechsel (#9):** `changed output-target` in `app-window.slint` -> Rust spielt 3 s später EQ-Toggle, Preamp und 10 Bänder
  aus den gespeicherten Einstellungen neu ins Gerät (wie die Windows-App im Capture "steckerwechsel mit eq preset"). Der gewählte
  Preset wandert so mit; getrennte Kurven je Ausgang sind damit nicht nötig (Nutzer bestätigt: unter Windows wird der Preset
  beim Wechsel ebenfalls übernommen). Live bestätigt.
- **Scout-Mode-Hotkey (#10) über das XDG-GlobalShortcuts-Portal** (`src/hotkey.rs`, Crate `ashpd` 0.13 mit `async-io`,
  kein tokio): Thread mit `futures_lite::block_on`, Sitzung + `bind_shortcuts` ("scout-mode-toggle"), `receive_activated` ->
  Scout-Schalter im UI umlegen und `scout_toggled` auslösen. Checkbox "Zugriffstaste" (Modi & Decoder) schaltet die Reaktion
  (`scout_hotkey` in settings.txt). Die Taste vergibt der Nutzer in den KDE-Systemeinstellungen (Kurzbefehle -> Sound Blaster G8
  Control -> **Anwenden** nicht vergessen!); KDE warnt dort bei Kollisionen und merkt sie sich über Neustarts (kglobalshortcutsrc,
  `[g8-control] scout-mode-toggle=...`). `preferred_trigger` wird von KDE ignoriert. Das Portal erkennt die App an ihrer
  systemd-Scope/Desktop-Datei (`g8-control.desktop`): Start über das Anwendungsmenü passt; für Tests von Hand
  `systemd-run --user --unit='app-g8\x2dcontrol@test.service' g8-control` (ein reines `nohup` aus einer fremden Sitzung läuft
  unter deren App-ID). Live bestätigt (8 Aktivierungen, Taste überlebt Neustart). Nur KDE/GNOME mit Portal-Unterstützung;
  ohne Portal meldet die App "Globaler Hotkey nicht verfügbar" im Log und läuft normal weiter.
- **Version 0.2.1** (Scout-Hotkey, EQ-Übertrag beim Ausgabewechsel, Hilfsskript). `packaging/g8-aussetzer-pruefen.sh [Tage]` listet die
  USB-Neuanmeldungen der G8 über alle Systemstarts (`journalctl _TRANSPORT=kernel`; `journalctl -k` würde nur den aktuellen Start
  durchsuchen, deshalb war die Basislinie "25" aus Issue #3 pro Start gezählt). Für Issue #3 gilt: Neuanmeldungen ohne Umstecken,
  Neustart oder Direct-Mode-Wechsel sind Aussetzer. Beobachtung ab 2026-10-27 (vorher fast nur Test-Neuanmeldungen).

- Noch offen: (#9 erledigt, siehe oben; war: getrennte EQ-Kurven je Ausgang, laut Capture reine App-Funktion), #10 (Scout-Hotkey, geplant über das
  KDE-GlobalShortcuts-Portal), #2, #3 (Beobachtung), Hinweis: der Lautsprecher-Tab-Audioqualität/"Standard festlegen" ist host-seitig.

- **Vorfall: Lautstärkeknopf komplett unresponsiv, nur durch echten
  Stromverlust behoben (2026-08-23).** Nach einer intensiven Test-Session
  (mehrfache PipeWire-Profilwechsel `pro-audio`↔`analog-stereo`, mehrere
  USB-Neuenumerationen durch den Reset-Befehl, parallel laufende
  Wireshark/usbmon-Mitschnitte) reagierte der physische Lautstärkeknopf
  irgendwann gar nicht mehr – das rohe ALSA-Register (`amixer -c USB1
  sget Speaker`) blieb komplett statisch, selbst bei durchgehendem
  Drehen. Systematisch ausgeschlossen, in dieser Reihenfolge:
  - Eigene App/Balance-Feature: Anzeige-Bug gefunden und gefixt (`get_level()`
    las nur den linken statt beider Kanäle), war aber nicht die Ursache
    hier – beide Kanäle liefen immer synchron.
  - PipeWire-Hardware-Lautstärke-Rückschreiben: WirePlumber hatte in
    `~/.local/state/wireplumber/stream-properties` eine veraltete
    Lautstärke (~13%) für den G8-Sink gecacht, die tatsächlich direkt ins
    ALSA-Register durchschlug (bestätigt: `wpctl set-volume` bewegte den
    ALSA-Rohwert). Testweise per WirePlumber-Regel (`api.alsa.soft-mixer
    = true`) "behoben" – **später als Sackgasse erkannt und wieder
    entfernt** (siehe Nachtrag 2 unten: die Regel wirkte nachweislich gar
    nicht, `wpctl set-volume` bewegte das ALSA-Register weiterhin trotz
    `soft-mixer: true`). War ohnehin **nicht** die Ursache des kompletten
    Ausfalls: Test mit komplett gestopptem PipeWire+WirePlumber
    (`systemctl --user stop/mask`) zeigte das Problem unverändert.
  - Normaler PC-Neustart (Software-Reboot): keine Besserung.
  - **Tatsächlicher Fix: Das USB-Kabel direkt am Gerät abgezogen (nicht
    nur den PC aus-/eingeschaltet), danach funktionierte der Regler
    sofort wieder einwandfrei** (sauberer, glatter Verlauf über den
    ganzen Bereich, live verifiziert). Vermuteter Grund: das Mainboard
    hält USB-Ports auch im ausgeschalteten Zustand unter Strom (ErP-
    Energiesparmodus im BIOS war zum Testzeitpunkt aus/an in wechselnder
    Kombination, nicht abschließend zugeordnet) – ein normaler Reboot
    schneidet der G8 dadurch nie wirklich den Strom ab, ein interner
    Firmware-/Register-Zustand blieb über den Software-Neustart hinweg
    hängen. **Schlussfolgerung:** Kein Software-Bug in g8-control,
    PipeWire oder WirePlumber – ein reiner Geräte-/Firmware-Zustand, der
    nur durch echten Stromverlust am USB-Kabel selbst zurückgesetzt wird.
    Falls das nochmal auftritt: zuerst Kabel direkt am Gerät (nicht am
    PC/Hub) für 30+ Sekunden abziehen, bevor an der Software gesucht wird.
  - **Nachtrag, selber Tag:** Nach weiterem Reset-Testen (zwei Resets
    kurz hintereinander) trat exakt dasselbe Symptom nochmal auf –
    diesmal half das Kabel-Abziehen NICHT, erst ein vollständiger
    PC-Neustart brachte den Regler zurück. Das spricht dafür, dass der
    hängende Zustand nicht immer im selben Teil sitzt: mal in der
    G8-Firmware selbst (durch Kabel-Stromverlust behebbar), mal im
    USB-Host-Controller/Port auf PC-Seite (nur durch xHCI-Neuinitialisierung
    beim PC-Neustart behebbar, ein reines Geräte-Powercycle reicht dann
    nicht). Wiederholtes Reset-Auslösen (echte USB-Neuenumeration) scheint
    ein Auslöser für beide Varianten zu sein. **Praktische Reihenfolge bei
    komplett unresponsivem Regler:** 1) Kabel direkt am Gerät 30+s abziehen,
    2) falls das nicht hilft, kompletten PC-Neustart. Kein Software-Bug in
    g8-control in beiden beobachteten Fällen (live per rohem ALSA-Register-
    Polling verifiziert, unabhängig von PipeWire/WirePlumber/unserer App).
  - **Nachtrag 2, echter Software-Bug gefunden und behoben (selber Tag):**
    Nach dem PC-Neustart trat ein DRITTES, andersartiges Symptom auf: der
    Regler reagierte live korrekt, aber der Wert sprang nach jeder
    Änderung binnen ~200-400ms wieder auf 0% zurück ("blitzt kurz auf,
    fällt dann zurück"). Ursache diesmal wirklich in unserer eigenen
    Software: Sowohl der GUI-Prozess (Volume Sync, main.rs) als auch der
    separate Hintergrunddienst (daemon.rs) pollten unabhängig voneinander
    alle 120ms `mixer::get_level(0)` und schrieben beide unkoordiniert per
    `wpctl set-volume` auf dieselbe PipeWire-Sink-Lautstärke – zwei
    Schreiber mit leicht versetztem Timing auf denselben Zielwert. Erster
    Fix (GUI schreibt nicht mehr, wenn der Hintergrunddienst aktiv ist)
    behob es NICHT vollständig. Beim genaueren Hinschauen der eigentliche,
    tiefere Fund: **PipeWire spiegelt das ALSA-Hardware-Register der G8
    bereits von ganz allein** (live verifiziert: `wpctl get-volume`
    folgte dem Regler exakt und in Echtzeit, komplett ohne g8-control
    oder den Hintergrunddienst – weder App noch Daemon liefen während des
    Tests). Unser eigener `wpctl set-volume`-Push war also nicht nur
    unnötig, sondern die eigentliche Ursache des Zurückspringens: ein
    zweiter, leicht verzögerter Schreiber auf einen Wert, den PipeWire
    schon korrekt und aktuell hatte. **Fix:** `mixer::
    set_pipewire_sink_volume_percent()` und alle Aufrufstellen (GUI-
    Poll-Thread, Volume-Sync-Toggle-Handler, Start-Wiederherstellung,
    `daemon.rs::run()`) komplett entfernt – weder App noch Daemon
    schreiben noch aktiv zu PipeWire, beide verlassen sich auf PipeWires
    eigenes automatisches Hardware-Volume-Spiegeln. Die (zuvor unter
    Nachtrag 1 als "sinnvolle Absicherung" beschriebene) WirePlumber-
    Soft-Mixer-Regel wurde wieder entfernt: wir WOLLEN das automatische
    Hardware-Spiegeln jetzt aktiv, nicht verhindern. Live verifiziert
    (App+Daemon beide aktiv): Regler läuft stabil, kein Zurückspringen
    mehr, `wpctl get-volume`/ALSA bleiben synchron.

    **Entscheidung getroffen und umgesetzt (2026-08-23):** "Volume Sync"
    und "Hintergrunddienst" komplett aus der App entfernt (Settings-Tab-
    Schalter, `src/daemon.rs` gelöscht, `setting_id::VOLUME_SYNC`-Report
    nicht mehr gesendet, systemd-Dienst deinstalliert) – PipeWire koppelt
    PC-Lautstärke und Regler bereits von selbst, kein eigener Mechanismus
    mehr nötig. An der Stelle des früheren Hintergrunddienst-Schalters
    steht jetzt ein Hinweistext dazu.

    **Bekannter Unterschied (bewusst so belassen, Entscheidung 2026-08-24):**
    Die App zeigt weiterhin die lineare (taper-korrigierte) Reglerposition
    (z.B. 44%), während PipeWire/KDE die kubische Lautstärke-Skala zeigt
    (z.B. 60%) – zwei unterschiedliche, beide korrekte Zahlen für
    denselben Rohwert, da wir nicht mehr aktiv in PipeWire hineinschreiben.
    Bewusst NICHT auf die kubische Skala umgestellt: die physische
    Reglermitte würde dann wieder ~67% statt 50% zeigen – genau das
    Problem, das die Taper-Korrektur ursprünglich gelöst hat. Abgewogen
    und vom Nutzer bestätigt: unsere Anzeige bleibt bei der gefühlten
    Reglerposition.

## 7a. Dark Mode: Palette.color-scheme-Zuweisung reagiert nicht immer (behoben 2026-08-24)

Siehe [HANDOFF-ARCHIVE.md](HANDOFF-ARCHIVE.md) – zwei konkurrierende
Schreibpfade auf `Palette.color-scheme`, gefixt durch genau einen
Auslöser für den interaktiven Fall und `init =>` statt `changed` für die
Start-Wiederherstellung.

## 7b. Internationalisierung (i18n): DE/EN/FR vollständig (2026-08-24)

Technisches Grundgerüst für Mehrsprachigkeit aufgesetzt UND direkt auf
das **komplette Projekt** ausgeweitet (ursprünglich als Zwei-Tab-Pilot
geplant, nach erfolgreichem Test auf Nutzerwunsch sofort auf alle ~14
`.slint`-Dateien ausgedehnt). 107 übersetzbare Strings gefunden,
komplett in Deutsch (Identität), Englisch und Französisch übersetzt.
Live getestet, Nutzer bestätigt: "alles perfekt", kein deutscher Rest,
kein abgeschnittener/überlappender Text irgendwo, auch nicht bei den 5
Acoustic-Engine-Karten (siehe Layout-Fix unten).

**Mechanismus:** Slint 1.17 (in Cargo.toml als "1.8" mindestangefordert,
tatsächlich aufgelöst: 1.17.1) hat eingebaute Gettext-Unterstützung,
keine Eigenbau-Lösung nötig:
- `.slint`-Strings mit `@tr("Text")` markieren (Platzhalter mit
  `@tr("Text {}", value)`).
- `build.rs`: `slint_build::CompilerConfiguration::new()
  .with_bundled_translations("lang")` – **nicht** `with_translation_domain`
  (existiert in dieser Version nicht, erste Fehlannahme). Domäne ist
  automatisch `CARGO_PKG_NAME` ("g8-control").
- Erwartet **rohe `.po`-Dateien** (kein separates `msgfmt`/`.mo` für die
  Slint-Seite nötig – der Compiler bündelt sie selbst beim Bauen) unter
  `lang/<sprache>/LC_MESSAGES/g8-control.po`.
- Extraktion der Strings aus dem Quellcode: `slint-tr-extractor -d
  g8-control --package-name g8-control --package-version 0.1.1 -o
  lang/g8-control.pot <dateien>` (aus `cargo install
  slint-tr-extractor`, war schon installiert).
- Laufzeit-Sprachauswahl: **inzwischen umgesetzt** (Einstellungen -> Sprache, siehe
  Nachtrag 2026-10-26 (2)); die `LANGUAGE`-Variable wirkt nur noch bei "Automatisch".
  `LANGUAGE` braucht (anders als `LC_ALL`/`LANG`) keine installierte
  System-Locale, nur den Sprachcode.
- Live getestet (2026-08-24): `LANGUAGE=fr LC_ALL=C.UTF-8` zeigt
  korrekt französischen Text im Einstellungen-Tab, Screenshot vom
  Nutzer bestätigt.

**Bewusst NICHT übersetzt (bleiben als Original-Markenbegriffe stehen,
über alle Sprachen hinweg):** Feature-/Markennamen wie "Acoustic
Engine", "Surround", "Crystalizer", "Bass", "Dialog+", "Smart Volume",
"Equalizer" sowie sämtliche Spiel-/Preset-Namen in der Sound-Modus-
Dropdown-Liste (z.B. "Baldur's Gate 3", "Counter-Strike 2") – genau wie
Creative das in der echten Software über alle Sprachen hinweg auch
macht, passend zur Projekt-Philosophie "Original nachbilden".

**Layout-Lehre, live gefunden und behoben, auf alle betroffenen Stellen
angewendet:** Mehrere Segment-Umschalter (Kopfhörer/Lautsprecher in
`sound-mode-tab.slint` UND `acoustic-engine-tab.slint`, Schließen/Tray
in `settings-tab.slint`) hatten eine **feste** Container-Breite um die
Text-Segmente – bei längeren Übersetzungen (Französisch ist oft 15-20%
länger, siehe Nutzer-Frage) ein Risiko für abgeschnittenen/
überlappenden Text. Fix: `width` auf dem äußeren Rectangle entfernt,
jedes Segment bekommt stattdessen eine innere `HorizontalLayout` mit
`padding-left`/`padding-right` – Breite ergibt sich automatisch aus dem
längeren Segment-Inhalt. Die 5 Acoustic-Engine-Karten hatten zusätzlich
eine feste `height: 580px` – ebenfalls entfernt: `HorizontalLayout`
streckt Geschwister ohne eigene `height` automatisch auf die höchste
natürliche Inhaltshöhe, das liefert gleichzeitig Auto-Größe UND ein
bündiges Kartenraster in einem. Live mit dem längsten Testfall
bestätigt ("Réduire dans la barre d'état système"), Nutzer-Screenshot
zeigt sauberes Ergebnis ohne Abschneiden.

**Bewusst NICHT übersetzt (bleiben als Original-Markenbegriffe stehen,
über alle Sprachen hinweg):** Feature-/Markennamen wie "Acoustic
Engine", "Surround", "Crystalizer", "Smart Volume", "Dialog+", "Noise
Reduction", "Acoustic Echo Cancellation", "Voice Morph", "CrystalVoice",
"Direct Mode", "Scout Mode", "SPDIF-Out Direct Mode" sowie sämtliche
Spiel-/Genre-/Preset-Namen (Sound-Modus- und EQ-Presets, z.B. "Baldur's
Gate 3", "Jazz") und Hardware-/Signal-Quellennamen ("Line In",
"External Mic", "SPDIF In", "What U Hear") – genau wie Creative das in
der echten Software über alle Sprachen hinweg auch macht, passend zur
Projekt-Philosophie "Original nachbilden". "Bass"/"Equalizer" werden
JE NACH KONTEXT unterschiedlich behandelt: als Acoustic-Engine-Effekt-
Name unübersetzt, als generische EQ-Spaltenbeschriftung/Sektionstitel
übersetzt – bewusste, vertretbare Unterscheidung.

**Offen für später:**
- Rust-seitige Strings (Fehlermeldungen in `main.rs` etc.) sind über
  Slints Mechanismus NICHT abgedeckt, bräuchten eine eigene Lösung
  (z.B. `gettext-rs`, gleiches `.po`-Format, separate Einbindung).
- UI-Umschalter zum manuellen Sprachwechsel zur Laufzeit.
- Automatische Spracherkennung (System-Locale) – explizit auf "sehr
  viel später" verschoben.
- `lang/make_po.py` (Skript zum Regenerieren der drei `.po`-Dateien aus
  `lang/g8-control.pot`) hat die Übersetzungen fest im Skript hinterlegt
  – bei neuen Strings: `slint-tr-extractor` erneut laufen lassen (siehe
  Befehl oben), neue Einträge im Skript ergänzen, Skript erneut
  ausführen. Kein automatischer Übersetzungs-Merge-Workflow für neue
  Strings, nur Neugenerierung aus dem festen Dictionary.

## 8. Offene Punkte

- ~~Line-Out-Stecker trennt Kopfhörerausgang, Software bekommt nichts mit~~
  **geklärt, 2026-08-24**, siehe [HANDOFF-ARCHIVE.md](HANDOFF-ARCHIVE.md)
  – reiner analoger Relais-Schalter, kein USB-Traffic, kein Bug.
- ~~Sonnen-Icon (☀️) im Sound-Modus-Tab wird nicht dargestellt~~ **erledigt,
  2026-08-24**, siehe [HANDOFF-ARCHIVE.md](HANDOFF-ARCHIVE.md).
- ~~Kein Auto-Reconnect nach Geräteverlust~~ **erledigt, 2026-08-23** –
  siehe Abschnitt 5a und Commit "Automatischen Reconnect nach
  Geräteverlust hinzufügen".
- ~~Sound-Modus-Preset auf dem Gerät auslesen~~ **erledigt, 2026-08-23**,
  siehe [HANDOFF-ARCHIVE.md](HANDOFF-ARCHIVE.md).
- **Namespace 0x09 (Geräteinfo), Sub-IDs `01`/`02 01`/`02 01 01`**: drei
  binäre Antwortblöcke unbekannter Bedeutung (siehe Abschnitt 2) – noch
  zu entschlüsseln, evtl. durch Vergleich mit einer zweiten G8 oder
  Diff gegen andere Firmware-Stände.
- **Unaufgeforderte Pushes `5a 6e 02 01` und `5a 1a 02 02`** (gefunden
  2026-08-23): Auslöser identifiziert – treten immer gemeinsam mit
  `5a 2c 05 01 <target>` auf, sowohl bei physischem Kopfhörer-Aus-/
  Einstecken (Jack-Sensing) als auch nach einem Geräte-Reset/
  Reconnect. Wirken wie generische "Ausgangsziel/Konfiguration hat
  sich geändert"-Benachrichtigungen. Die konkrete Bedeutung der
  Werte selbst (`02`/`01`) ist noch nicht weiter aufgeschlüsselt
  (bisher nur je ein konstanter Wert beobachtet, keine Variation).
- Mixer-Zugriff (`mixer.rs`) startet für jedes Poll-Tick einen neuen
  `amixer`-Prozess statt eine dauerhafte ALSA-Verbindung offen zu halten
  (wie Windows/PipeWire es intern tun, siehe Abschnitt 3/7). Funktioniert,
  ist aber unnötig schwergewichtig und ein Faktor beim beobachteten
  USB-Stall (Abschnitt 7). Sauberer wäre ein persistentes Mixer-Handle
  über die `alsa`-Crate statt Prozess-Spawning pro Tick – bisher nicht
  umgesetzt (neue Abhängigkeit, größerer Umbau).
- Abtastrate/Sample-Rate-Wechsel (Clock Source) noch nicht direkt
  beobachtet – niedrige Priorität, da ALSA/PipeWire das ohnehin
  automatisch aushandelt (siehe Abschnitt 3)
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
