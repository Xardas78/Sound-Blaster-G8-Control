# Sound Blaster G8 Control

Natives Linux-Kontrollpanel für den Creative Sound Blaster G8 (USB-DAC/Kopfhörerverstärker), geschrieben in Rust mit [Slint](https://slint.dev/). Bildet die Original-Windows-Software nach – Acoustic Engine, 10-Band-Equalizer, CrystalVoice, Scout Mode, Decoder, Mixer und mehr –, reverse-engineert aus USB-Captures, da Creative keine offizielle Linux-Unterstützung anbietet.

![Screenshot der Acoustic-Engine-Seite](docs/screenshot-acoustic-engine.png)

Inoffizielles Projekt, keine Verbindung zu oder Unterstützung von Creative Technology Ltd.

## Funktionen

- **Acoustic Engine** – Surround, Crystalizer, Bass, Smart Volume (inkl. Nacht/Normal/Laut), Dialog+
- **Sound-Modus** – 60 komplette Klangbild-Presets (Surround/Crystalizer/Bass/Smart Volume/Dialog+ + passende Equalizer-Kurve in einem Klick), plus dieselben Live-Regler wie Acoustic Engine direkt eingebettet, im selben Zustand wie der Acoustic-Engine-Tab (Regler-Dreh an einer Stelle zeigt sich sofort an der anderen) – wählt ein Preset auch im Equalizer-Tab denselben Namen aus, nicht nur die Kurve
- **10-Band-Equalizer** mit Vorverstärkung und 71 echten Original-Presets
- **CrystalVoice** – Noise Reduction, Echo Cancellation, Voice Morph, Mikrofon-Equalizer
- **Modi & Decoder** – Direct Mode, Scout Mode (inkl. Zugriffstaste), Dolby-Decoder
- **Mixer** – alle Ein-/Ausgangspegel inkl. Stummschaltung und Balance (L/R)
- **Wiedergabe** – 7.1-Positionsdiagramm mit Testtönen, Kopfhörer/Lautsprecher-Umschaltung
- **Virtueller HRTF-Kopfhörer-Surround**: PipeWire-Filter-Chain mit HeSuVi-Faltung, da die G8-Hardware selbst nur Stereo ausgibt
- **Lautstärkeknopf-Livesync**: der physische Regler am Gerät steuert live den Regler in der App; die PC-Systemlautstärke folgt automatisch über PipeWire, ganz ohne eigenes Zutun der App (siehe HANDOFF.md Abschnitt 7)
- **Autostart, Tray-Icon, Einstellungen merken sich selbst**: Autostart mit dem System, Schließen-Verhalten wählbar (beenden oder in den Tray minimieren), App-seitige Einstellungen (Acoustic Engine, Equalizer, Energiesparen, Logo-LED, Dark Mode) werden gespeichert und beim Start automatisch wieder angezeigt und ans Gerät gesendet
- **Automatischer Reconnect**: verliert die App die Geräteverbindung (z.B. nach "Gerät zurücksetzen"), verbindet sie sich von selbst wieder, sobald das Gerät zurück ist
- **Firmware-Versions-Check**: zeigt die aktuelle Firmware an und prüft gegen Creatives Update-Feed, ob eine neuere verfügbar ist (reiner Versions-Check, kein Update-Flash – siehe Status unten)
- Dark Mode

## Status

Funktionsfähig und in aktiver Entwicklung. Große Teile des proprietären USB-Protokolls sind aus echten Captures rekonstruiert (siehe [HANDOFF.md](HANDOFF.md) für die vollständigen Hex-Funde und Analyse).

**Funktioniert:** Sound-Modus (60 echte kombinierte Presets, 1:1 aus der Windows-Preset-Bibliothek), Acoustic Engine, 10-Band-Equalizer (31 Hz–16 kHz) mit allen 71 echten Original-Presets, CrystalVoice inkl. korrekter Voice-Morph- (18 echte Charaktere) und Mikrofon-Equalizer-Presets (9 echte Presets) sowie Smart-Volume-Parameter, Scout Mode, Direct Mode/SBX, Decoder, Mixer inkl. Stummschaltung und Balance (L/R), **Kopfhörer/Lautsprecher-Umschaltung inkl. Fernsteuerung** (Relais-Klick + LED, nicht nur Anzeige), SPDIF-Out Direct Mode, Energiesparen, Logo-LED, virtueller HRTF-Surround, Lautstärkeknopf-Livesync (Fußleiste und Mixer-Tab teilen sich denselben Wert für Lautsprecher/Mikrofon, PC-Lautstärke folgt automatisch über PipeWire), Autostart, Tray-Icon mit wählbarem Schließen-Verhalten, automatisches Speichern/Wiederherstellen der Einstellungen, **Gerät zurücksetzen** (echter Werksdefault-Reset, live bestätigt, automatischer Reconnect danach), Sound-Modus-Preset wird software-seitig gemerkt, Seriennummer-Anzeige, Firmware-Versions-Check gegen Creatives Update-Feed.

**Gelöst – Lautstärkeregler-Kennlinie:** Der Lautsprecher-Drehregler ist ein analoges Poti mit "Audio-Taper"-Kennlinie – gleich große Drehschritte verändern den elektrischen Wert sehr ungleichmäßig (unten viel, oben wenig; bei optischer Mitte lag der rohe Wert bei ~84 % statt 50 %). Aus einem Windows-USB-Capture einer kompletten langsamen Drehung wurde eine 24-Punkte-Korrekturkurve extrahiert, live am Gerät verifiziert – die App-Anzeige zeigt jetzt den gefühlt-richtigen Wert statt des rohen linearen ALSA-Werts. Details siehe [HANDOFF.md](HANDOFF.md) Abschnitt 7.

**Gelöst – PC-Lautstärke-Kopplung:** Ein früherer eigener Push zur PipeWire-Sink-Lautstärke ("Volume Sync"/Hintergrunddienst) wurde entfernt – PipeWire spiegelt die G8-Hardware-Lautstärke bereits von selbst, ganz ohne dass die App etwas tun muss. Der eigene Push führte sogar zu Konflikten (Lautstärke sprang zeitweise auf 0% zurück); ohne ihn läuft es sauber. Einzige Einschränkung: die App zeigt die lineare (taper-korrigierte) Reglerposition, während KDE/PipeWire die "kubische" Lautstärke-Skala zeigt – zwei unterschiedliche, beide korrekte Zahlen für denselben Wert, rein kosmetisch. Details siehe HANDOFF.md Abschnitt 7, Nachtrag 2.

Weiterhin bekannt:
- Smart Volume (Acoustic Engine) hebt/senkt den Pegel nicht immer sauber nachvollziehbar
- Die übrigen Mixer-Regler (Line In, SPDIF, What U Hear) sind noch nicht live synchronisiert, nur Lautsprecher und externes Mikrofon
- Der Lautstärkeknopf kann in einen Zustand geraten, in dem er gar nicht mehr reagiert (weder Software- noch Hardware-Bug in g8-control) – meist durch echten Stromverlust am USB-Kabel behebbar, manchmal erst durch einen vollständigen PC-Neustart. Siehe HANDOFF.md Abschnitt 7 für die Diagnose und Details.

**Funktioniert noch nicht** (UI vorhanden, aber ohne Wirkung – Protokoll unbekannt):
- Sound-Modus-Preset **auf dem Gerät selbst** speichern/auslesen (bräuchte einen neuen USB-Capture, siehe HANDOFF.md Abschnitt 8) – **software-seitig** merkt sich die App das zuletzt gewählte Preset aber bereits und wendet es beim nächsten Start automatisch wieder an
- Bass-Crossover-Frequenz aus den Sound-Modus-Presets (kein bekannter Geräteparameter)
- Firmware-**Flash** – **bewusst dauerhaft nicht implementiert** (Bricking-Risiko bei falscher Byte-Sequenz), nicht nur vorläufig offen; bei verfügbarem Update verweist die App auf die offizielle Windows-Software; siehe [HANDOFF.md](HANDOFF.md) Abschnitt 5
- Scout-Mode-Zugriffstaste (Hotkey) ist nur UI-seitig vorbereitet, kein echter globaler Tastatur-Hook

**Bekannte Einschränkung:** Windows führt für Equalizer-Presets separate Kopfhörer-/Lautsprecher-Kurven; unser Equalizer hat nur eine gemeinsame Kurve unabhängig vom Kopfhörer/Lautsprecher-Umschalter (wir übernehmen nur die Kopfhörer-Variante der Original-Presets).

## Voraussetzungen

- Linux mit PipeWire (für den virtuellen Surround) und ALSA
- Rust-Toolchain (`cargo`)
- Ein angeschlossener Sound Blaster G8 (USB-ID `041e:329b`)

## Bauen & Starten

```bash
cargo run --release
```

Ohne angeschlossenes Gerät startet die App trotzdem – alle proprietären Befehle werden dann nur geloggt statt gesendet.

## Installieren (Desktop-Integration)

```bash
./packaging/install.sh
```

Baut die Release-Version und installiert Binary, Icon und `.desktop`-Eintrag nach `~/.local/{bin,share}` (kein root nötig). Zum Deinstallieren einfach die dort angelegten Dateien wieder löschen.

## Technische Details

Das proprietäre USB-Protokoll (HID Vendor-Kanal auf Interface 4) sowie die genutzten Standard-USB-Audio-Class-Mechanismen sind in [HANDOFF.md](HANDOFF.md) dokumentiert.
