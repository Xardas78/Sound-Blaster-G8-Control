#!/usr/bin/env bash
# Zeigt, wann sich die Sound Blaster G8 am USB neu angemeldet hat (jede Neuanmeldung steht im Kernel-Log).
# Normal sind Neuanmeldungen nur, wenn du das Kabel ziehst, den PC startest oder Direct Mode umschaltest
# (auch per MODE-Taste). Alles andere sind "Aussetzer" -- dann bitte melden (GitHub-Issue #3).
# Aufruf: ./g8-aussetzer-pruefen.sh [Tage]   (Standard: 7)
TAGE="${1:-7}"
echo "G8-Neuanmeldungen der letzten $TAGE Tage:"
# _TRANSPORT=kernel statt -k: -k würde nur den aktuellen Systemstart durchsuchen.
journalctl _TRANSPORT=kernel --since "$TAGE days ago" -o short-iso --no-pager 2>/dev/null | awk '
    /new high-speed USB device number/ { t=$1; next }
    /Product: Sound Blaster G8/ && t != "" { sub(/T/, " ", t); sub(/[+-][0-9][0-9]:?[0-9][0-9]$/, "", t); print "  " t; n++; t="" }
    END { print "Summe: " n+0 }'
echo
echo "Zum Vergleich: am 25. und 26.10.2026 gab es viele Neuanmeldungen, fast alle durch Direct-Mode-Tests."
echo "Steigt die Zahl, ohne dass du umgeschaltet oder umgesteckt hast, ist das ein Aussetzer."
