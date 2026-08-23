# Sound Blaster G8 Control

Natives Linux-Kontrollpanel für den Creative Sound Blaster G8 (USB-DAC/Kopfhörerverstärker), geschrieben in Rust mit [Slint](https://slint.dev/). Bildet die Original-Windows-Software nach – Acoustic Engine, 10-Band-Equalizer, CrystalVoice, Scout Mode, Decoder, Mixer und mehr –, reverse-engineert aus USB-Captures, da Creative keine offizielle Linux-Unterstützung anbietet.

![Screenshot der Acoustic-Engine-Seite](docs/screenshot-acoustic-engine.png)

Inoffizielles Projekt, keine Verbindung zu oder Unterstützung von Creative Technology Ltd.

## Funktionen

- **Acoustic Engine** – Surround, Crystalizer, Bass, Smart Volume (inkl. Nacht/Normal/Laut), Dialog+
- **Sound-Modus** – 60 komplette Klangbild-Presets (Surround/Crystalizer/Bass/Smart Volume/Dialog+ + passende Equalizer-Kurve in einem Klick), plus dieselben Live-Regler wie Acoustic Engine direkt eingebettet, im selben Zustand wie der Acoustic-Engine-Tab (Regler-Dreh an einer Stelle zeigt sich sofort an der anderen)
- **10-Band-Equalizer** mit Vorverstärkung und 71 echten Original-Presets
- **CrystalVoice** – Noise Reduction, Echo Cancellation, Voice Morph, Mikrofon-Equalizer
- **Modi & Decoder** – Direct Mode, Scout Mode (inkl. Zugriffstaste), Dolby-Decoder
- **Mixer** – alle Ein-/Ausgangspegel inkl. Stummschaltung
- **Wiedergabe** – 7.1-Positionsdiagramm mit Testtönen, Kopfhörer/Lautsprecher-Umschaltung
- **Virtueller HRTF-Kopfhörer-Surround**: PipeWire-Filter-Chain mit HeSuVi-Faltung, da die G8-Hardware selbst nur Stereo ausgibt
- **Lautstärkeknopf-Livesync**: der physische Regler am Gerät steuert live den Regler in der App (und optional die PipeWire-Systemlautstärke, "Volume Sync")
- Dark Mode

## Status

Funktionsfähig und in aktiver Entwicklung. Große Teile des proprietären USB-Protokolls sind aus echten Captures rekonstruiert (siehe [HANDOFF.md](HANDOFF.md) für die vollständigen Hex-Funde und Analyse).

**Funktioniert:** Sound-Modus (60 echte kombinierte Presets, 1:1 aus der Windows-Preset-Bibliothek), Acoustic Engine, 10-Band-Equalizer (31 Hz–16 kHz) mit allen 71 echten Original-Presets, CrystalVoice inkl. korrekter Voice-Morph- (18 echte Charaktere) und Mikrofon-Equalizer-Presets (9 echte Presets) sowie Smart-Volume-Parameter, Scout Mode, Direct Mode/SBX, Decoder, Mixer inkl. Stummschaltung, **Kopfhörer/Lautsprecher-Umschaltung inkl. Fernsteuerung** (Relais-Klick + LED, nicht nur Anzeige), SPDIF-Out Direct Mode, Energiesparen, Logo-LED, virtueller HRTF-Surround, Lautstärkeknopf-Livesync (Fußleiste und Mixer-Tab teilen sich denselben Wert für Lautsprecher/Mikrofon).

**Baustelle – Lautstärke-Synchronisierung:** Es gibt mehrere Stellen, an denen Lautstärke geändert werden kann (ALSA-Hardwareregister, PipeWire-Sink-Lautstärke, unsere eigenen UI-Regler), die nicht vollständig miteinander verknüpft sind. Bekannte Probleme:
- Volume Sync (Einstellungen) spiegelt aktuell nur in eine Richtung (ALSA → PipeWire); Änderungen an der PC-Lautstärke über andere Wege (System-Lautstärkeregler, Medientasten) fließen nicht zurück
- Smart Volume (Acoustic Engine) hebt/senkt den Pegel nicht immer sauber nachvollziehbar
- Die übrigen Mixer-Regler (Line In, SPDIF, What U Hear) sind noch nicht live synchronisiert, nur Lautsprecher und externes Mikrofon

USB-Captures des Originals zeigen: beim Drehen des Knopfs entsteht auch mit aktivem Volume Sync kein zusätzlicher USB-Traffic – Windows koppelt Knopf und Systemlautstärke über einen eigenen Kernel-Treiber, unsichtbar auf dem USB-Kabel. Es fehlt also kein Protokoll-Befehl; das ist eine PipeWire/ALSA-Architekturfrage (mehrere echte Lautstärke-Ebenen), siehe [HANDOFF.md](HANDOFF.md) Abschnitt 7.

**Funktioniert noch nicht** (UI vorhanden, aber ohne Wirkung – Protokoll unbekannt):
- Sound-Modus-Presets im Gerät speichern/laden (Anwenden funktioniert, Auslesen/Persistieren eines aktiven Presets nicht)
- Bass-Crossover-Frequenz aus den Sound-Modus-Presets (kein bekannter Geräteparameter)
- Firmware-Versionsanzeige/-Update-Check, Gerät zurücksetzen
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
