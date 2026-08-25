# Archiv abgeschlossener Bugs/Vorfälle

Vollständig behobene, abgeschlossene Themen, die für die aktive Arbeit
am Projekt nicht mehr laufend gebraucht werden, aber als Referenz
erhalten bleiben sollen (z.B. um ein wiederkehrendes Symptom
wiederzuerkennen, siehe HANDOFF.md §7 für ein Beispiel, wo genau das
2026-08-24 passiert ist). Nichts wird von hier gelöscht.

Siehe auch [HANDOFF-REVERTED.md](HANDOFF-REVERTED.md) für Ansätze, die
implementiert und wieder rückgängig gemacht wurden (andere Kategorie:
dort ging der Fix NICHT auf oder wurde durch einen besseren ersetzt).

---

## Dark Mode: Palette.color-scheme-Zuweisung reagiert nicht immer (behoben 2026-08-24)

Der Dark-Mode-Schalter selbst funktionierte (sichtbares Umschalten,
korrekte Persistierung), aber die Farben reagierten teils gar nicht,
teils inkonsistent (manche Elemente hell, andere dunkel gleichzeitig).
Ursache: **zwei gleichzeitig aktive Mechanismen**, die beide
`Palette.color-scheme` setzten – einmal ein `changed dark-mode-on =>`-
Handler in app-window.slint, einmal eine direkte Zuweisung im Schalter
selbst (settings-tab.slint). Beide feuerten bei jedem Klick, was zu
inkonsistentem Neuzeichnen führte (welcher Handler "gewann", schien
nicht deterministisch). Fix: genau EIN Auslöser für den interaktiven
Fall (direkt im Schalter-Klick-Handler, settings-tab.slint – so wie in
der ursprünglichen Implementierung, commit `ed4a0c5`), und ein
separater `init =>`-Handler in app-window.slint (NICHT `changed`) nur
für die Start-Wiederherstellung aus main.rs. Zusätzlich: unsere eigenen
Hintergründe (Fenster, Karten) binden konsequent an dieselbe
`Palette.color-scheme == ColorScheme.dark ? ... : ...`-Bedingung wie die
Standard-Widgets, mit den echten Fluent-Palette-Farben (`#1C1C1C`/
`#FAFAFA` Hintergrund, `#2C2C2C`/`#f0f0f0` für Karten – aus
`FluentPalette` in std-widgets, nicht mehr selbst geraten). Live
verifiziert: beide Richtungen schalten jetzt zuverlässig und konsistent
um.

**Lehre für künftige Palette-Änderungen:** Eine `changed <property> =>`-
Reaktion auf eine ANDERE Property, die ihrerseits `Palette.color-scheme`
setzt, ist offenbar unzuverlässig, wenn gleichzeitig noch ein zweiter
Schreibpfad auf dieselbe Palette-Property existiert. Direkte Zuweisung
in einem echten Interaktions-Handler (Klick) funktioniert zuverlässig;
für rein programmatisches Setzen (Start, kein Klick) `init =>` statt
`changed` verwenden.

---

## Line-Out-Stecker trennt Kopfhörerausgang, Software bekommt nichts mit (geklärt 2026-08-24)

Steckt man etwas in Line-Out, wird der Kopfhörerausgang hörbar stumm.
Per Linux-`usbmon`-Capture (`lineou-test.pcapng`, mehrfaches Rein-/
Rausstecken Line-Out + einmal Line-In) bestätigt: **keine** USB-
Aktivität dabei – Gerätenummer bleibt über die komplette ~44s-Aufnahme
unverändert, keine Lücke im Datenfluss über 1ms. Reiner analoger
Relais-Schalter in der G8-Hardware, komplett am USB-Protokoll vorbei –
dafür gibt es grundsätzlich keine Push-Nachricht oder abfragbare
Kennung, die Software könnte das nie erkennen (kein Bug, keine offene
Aufgabe mehr).

(Ein früherer Capture-Versuch, `line out.pcapng`, zeigte scheinbar eine
Geräte-Neuanmeldung nach ~7s Stille – das war aber ein Artefakt vom
Start der Wireshark-Aufnahme selbst, nicht vom Line-Out-Stecken.)

---

## Sonnen-Icon (☀️) im Sound-Modus-Tab wird nicht dargestellt (erledigt 2026-08-24)

Zwei Emoji-Versuche scheiterten auf dem Testsystem (leeres Kästchen bzw.
ein falsches Ersatzzeichen 💥). Auf Nutzerwunsch am Ende bewusst durch 💥
ersetzt (passt semantisch: Nacht=Mond, Laut=Knall), kein Emoji-
Rendering-Problem mehr relevant.

---

## Kein Auto-Reconnect nach Geräteverlust (erledigt 2026-08-23)

Siehe HANDOFF.md Abschnitt 5a und Commit "Automatischen Reconnect nach
Geräteverlust hinzufügen".

---

## Sound-Modus-Preset auf dem Gerät auslesen (erledigt 2026-08-23)

`schrit 1 bis 5 nach claudes vorgabe.pcapng` – siehe HANDOFF.md
Abschnitt 2: keine geräteseitige Preset-Kennung, nur rohe Parameterwerte
werden vom Gerät persistiert, Preset-Name ist reine Software-
Buchhaltung.
