# Verworfene/rückgängig gemachte Ansätze

Diese Datei sammelt Lösungsansätze, die implementiert, getestet und dann
bewusst wieder entfernt/rückgängig gemacht wurden – mit der Begründung
von damals. Zweck: bevor ein Ansatz erneut versucht wird ("das müsste
doch eigentlich helfen"), hier nachschauen, ob er nicht schon mal
ausprobiert und aus gutem Grund verworfen wurde. Nichts wird aus dieser
Datei gelöscht, auch wenn ein Ansatz später doch wieder eingebaut wird
(siehe unten, Volume Sync) – dann wird ergänzt, nicht ersetzt.

Siehe auch [HANDOFF-ARCHIVE.md](HANDOFF-ARCHIVE.md) für abgeschlossene,
bestätigt behobene Bugs (nicht dasselbe wie hier: dort ging der Fix auf,
hier wurde der Ansatz selbst wieder verworfen).

---

## Direct Mode als 6-Toggle-Batch (Verwechslung mit dem SBX-Masterschalter)

**Ursprünglich implementiert:** Der physische MODE-Knopf/Direct-Mode-
Schalter wurde als Nachbildung eines "SBX-Masterschalters" umgesetzt –
6 einzelne Acoustic-Engine-Toggles (Surround, Dialog+, Smart Volume,
Crystalizer, Bass, Equalizer) gemeinsam auf `!on` gesetzt, basierend auf
einem älteren Capture (`g8 neu.pcapng`), das ein Batch-Muster
`5a 11 2c 07 00 ...` / `5a 11 26 06 00 ...` zeigte.

**Ersetzt am 2026-08-24**, nach einem frischen, gezielten Capture
(`Directmode ein-aus.pcapng`, mit klaren Zeitmarken für Software- UND
Tastendruck-Umschaltung): Direct Mode ist tatsächlich ein einzelnes
Hardware-Register (`5a 39 03 00 05 <01|00>`, Geräte-Einstellungs-
Familie, `setting_id::DIRECT_MODE`), byte-identisch bestätigt und
Unit-getestet (`protocol::tests::direct_mode_report`). Die alte 6-
Toggle-Nachbildung hatte zwei echte Probleme: (1) sie war nicht der
tatsächliche Mechanismus, (2) ihr Aus-Pfad schaltete pauschal alle 6
Effekte wieder EIN, auch die, die der Nutzer vorher bewusst ausgehabt
hatte – ein waschechter Bug, keine bloße Ungenauigkeit.

**Vermutlicher Grund für die Verwechslung:** Das alte Capture zeigte
wahrscheinlich einen SEPARATEN "SBX"-Schalter irgendwo in der Windows-
Software (nicht den physischen MODE-Knopf) – dieser SBX-Masterschalter
existiert weiterhin als eigenständig dokumentiertes, aber nicht
angebundenes Muster, siehe HANDOFF.md Abschnitt 2. Zwei echte,
unterschiedliche Mechanismen wurden über Monate als einer behandelt.

**Fund und Fix:** Erste Analyse und Patch von Fable (Claude), gegen die
Original-Captures verifiziert und mit angepasster Diagnose-Strategie
übernommen (2026-08-24) – siehe HANDOFF.md Abschnitt 2 und 7 für die
Details und den noch offenen Status-Bitfeld-Nachtrag.

---

## Volume Sync + eigener Hintergrunddienst

**Ursprünglich implementiert:** `setting_id::VOLUME_SYNC`-Kommando
(`5a 39 03 00 1c <an|aus>`) beim Start gesendet, plus ein separater
systemd-Hintergrunddienst (`src/daemon.rs`), der unabhängig von der
GUI-App alle 120ms den ALSA-Pegel pollte und aktiv per `wpctl
set-volume` in PipeWire schrieb.

**Entfernt am 2026-08-23**, Begründung (siehe HANDOFF.md §7, Abschnitt
"Nachtrag 2" für die volle Herleitung):

1. Ein drittes Symptom trat auf: Regler-Wert sprang nach jeder Änderung
   binnen ~200-400ms zurück auf 0%. Ursache: GUI-App UND
   Hintergrunddienst pollten beide unabhängig `mixer::get_level(0)` und
   schrieben beide leicht zeitversetzt per `wpctl set-volume` denselben
   Zielwert – zwei konkurrierende Schreiber.
2. Beim genaueren Hinsehen der eigentliche, tiefere Fund: **PipeWire
   spiegelte das ALSA-Hardware-Register der G8 zu diesem Zeitpunkt
   bereits von ganz allein** – live verifiziert mit komplett gestoppter
   App UND gestopptem Daemon: `wpctl get-volume` folgte dem Regler exakt
   und in Echtzeit, ganz ohne unser Zutun. Der eigene `wpctl
   set-volume`-Push war also nicht nur unnötig, sondern die Ursache des
   Zurückspringens.
3. Eine Auswertung zweier Windows-Captures (`vol min-max-min_mit
   sync.pcapng`, `volsync aus min max-volsync-ein min max.pcapng`) kam
   damals zum Schluss, dass Drehen am Regler – auch bei aktivem Volume
   Sync – "keinerlei zusätzlichen USB-Traffic" erzeugt, weder auf dem
   proprietären 0x5a-Kanal noch als zusätzliches Standard-UAC SET_CUR.
   Schlussfolgerung damals: kein fehlender USB-Befehl zu finden, rein
   eine PipeWire/ALSA-Architekturfrage.

**Fix:** `mixer::set_pipewire_sink_volume_percent()` und alle
Aufrufstellen entfernt, `setting_id::VOLUME_SYNC`-Report nicht mehr
gesendet, `src/daemon.rs` und der zugehörige systemd-Dienst komplett
gelöscht. App und PipeWire verlassen sich seither auf PipeWires eigenes
automatisches Hardware-Volume-Spiegeln.

---

### Nachtrag (2026-08-24): Volume Sync wieder eingebaut – die Analyse von 2026-08-23 war unvollständig

Am 2026-08-24 fiel der Regler erneut komplett aus (rohes ALSA-Register
blieb bei durchgehendem Drehen konstant – exakt das Symptom, das das
automatische PipeWire-Spiegeln voraussetzt und das laut obiger
Entscheidung "einfach funktionieren sollte"). Nach stundenlanger
Fehlersuche (USB-Ports, Kabel, PipeWire-Profile, komplette Neustarts,
Windows-Gegenprobe) wurde per `usbmon`-Mitschnitt unter Linux bestätigt:
**ohne `VOLUME_SYNC` sendet die G8 beim Drehen buchstäblich gar keine
USB-Pakete** – nicht auf dem HID-Consumer-Control-Kanal (Endpoint 0x84,
Interface 3), nicht sonstwo. Nach Senden von `5a 39 03 00 1c 01` (exakt
byte-identisch aus einem frischen Windows-Capture) begann das Gerät
sofort, auf Endpoint 0x84 zu senden (`KEY_VOLUMEUP`-artige HID-Reports),
und Windows zeigte im selben Capture zusätzlich eine GET_CUR-Abfrage-
schleife auf dem Standard-UAC-Kanal (Entity 1, siehe HANDOFF.md §7 für
Details) – beides hatte die Analyse vom Vortag offenbar übersehen (dort
wurde vermutlich nur nach zusätzlichem *SET_CUR* auf dem Vendor-Kanal
gesucht, nicht nach dem HID-Kanal oder einer GET_CUR-Abfrage vom Host).

**Also: `VOLUME_SYNC` wurde am 2026-08-24 wieder in `main.rs`
eingebaut** (Startblock, dort wo auch Power-Saving/Logo-LED gesendet
werden). Ob das die eigentliche Ursache des heutigen Ausfalls behebt,
ist NICHT abschließend geklärt – siehe HANDOFF.md §7 für den aktuellen,
offenen Stand (insbesondere: das rohe ALSA-Register bewegt sich
weiterhin nicht, vermutlich weil Linux das UAC-Feature-Unit-Register nur
bei eigener Abfrage aktualisiert statt bei Hardware-Änderung – ein
möglicher Kernel-/Treiber-Sachverhalt, keine reine App-Frage mehr).

**Lehre daraus:** Ein Ansatz, der einmal aus gutem Grund entfernt wurde,
kann später trotzdem wieder nötig werden, wenn sich die Rahmenbedingungen
ändern (hier: irgendetwas hat das automatische PipeWire-Spiegeln
zwischen 2026-08-23 und 2026-08-24 kaputt gemacht, unabhängig von Volume
Sync). Deshalb: diese Datei nie einfach löschen/überschreiben, sondern
ergänzen – die alte Begründung bleibt wertvoller Kontext, auch wenn die
Entscheidung inzwischen anders ausfällt.
