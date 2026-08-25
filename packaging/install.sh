#!/usr/bin/env bash
# Installiert g8-control für den aktuellen Nutzer (kein root nötig):
# Release-Build, Binary nach ~/.local/bin, Icon und .desktop-Eintrag
# nach ~/.local/share/... . Zum Deinstallieren die dort angelegten
# Dateien einfach wieder löschen.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Baue Release-Binary..."
cargo build --release

BIN_DIR="$HOME/.local/bin"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
DESKTOP_DIR="$HOME/.local/share/applications"

mkdir -p "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR"

install -m 755 target/release/g8-control "$BIN_DIR/g8-control"
install -m 644 ui/assets/app-icon.svg "$ICON_DIR/g8-control.svg"
install -m 644 packaging/g8-control.desktop "$DESKTOP_DIR/g8-control.desktop"

command -v update-desktop-database >/dev/null && update-desktop-database "$DESKTOP_DIR" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "Fertig. '$BIN_DIR' muss in PATH sein (meist schon der Fall unter Arch/CachyOS)."
echo "App sollte jetzt im Anwendungsmenü als 'Sound Blaster G8 Control' auftauchen."

# --- Optionaler Schritt: USB-Hub-Autosuspend prüfen (braucht root) ---
#
# Nur relevant, wenn die G8 gerade über einen Hub angeschlossen ist (nicht
# direkt an einem Board-Port) UND der Hub Autosuspend aktiv hat. Live
# 2026-08-24 auf einem AMD-System als Ursache für sporadische USB-
# Disconnects der G8 bestätigt (siehe HANDOFF.md, RedHat-Bugzilla
# #1196943 "USB autosuspend massively breaks xHCI on AMD APU chipsets").
# Betrifft NICHT die G8 selbst (deren eigenes Autosuspend war schon aus),
# sondern die vorgeschaltete Hub-Hardware.
G8_DEV_PATH=""
for d in /sys/bus/usb/devices/*/idVendor; do
    if [ "$(cat "$d" 2>/dev/null)" = "041e" ]; then
        dir="$(dirname "$d")"
        if [ "$(cat "$dir/idProduct" 2>/dev/null)" = "329b" ]; then
            G8_DEV_PATH="$dir"
            break
        fi
    fi
done

if [ -n "$G8_DEV_PATH" ]; then
    # Elterlicher Hub = Pfad ohne das letzte ".N"-Suffix (z.B. 3-3.4 -> 3-3).
    # Direkt an einem Root-Port (kein Punkt in der Nummer) hat keinen
    # separaten Hub-Eintrag -- dann gibt es hier nichts zu tun.
    G8_BASENAME="$(basename "$G8_DEV_PATH")"
    if [[ "$G8_BASENAME" == *.* ]]; then
        HUB_PATH="/sys/bus/usb/devices/${G8_BASENAME%.*}"
        HUB_POWER_FILE="$HUB_PATH/power/control"
        if [ -f "$HUB_POWER_FILE" ] && [ "$(cat "$HUB_POWER_FILE")" = "auto" ]; then
            HUB_VENDOR="$(cat "$HUB_PATH/idVendor" 2>/dev/null || echo "?")"
            HUB_PRODUCT="$(cat "$HUB_PATH/idProduct" 2>/dev/null || echo "?")"
            echo ""
            echo "Hinweis: Der USB-Hub, an dem die G8 gerade hängt ($HUB_VENDOR:$HUB_PRODUCT),"
            echo "hat USB-Autosuspend aktiv. Auf manchen Systemen (v.a. mit AMD-xHCI-"
            echo "Controllern) kann das sporadische Disconnects der G8 verursachen."
            echo ""
            echo "Vorteil beim Deaktivieren: der Hub (und alles, was daran hängt) geht"
            echo "nie in einen USB-Energiesparzustand -- weniger Wahrscheinlichkeit für"
            echo "genau diese Aussetzer."
            echo "Nachteil: minimal höherer Leerlauf-Stromverbrauch für den Hub und ALLE"
            echo "daran angeschlossenen Geräte (nicht nur die G8), typischerweise"
            echo "vernachlässigbar bei einem Desktop-PC, bei Laptops im Akkubetrieb"
            echo "eher spürbar. Die Änderung gilt nur für DIESEN Hub-Chip"
            echo "(Hersteller/Produkt-ID) und übersteht Neustarts, per udev-Regel."
            echo ""
            read -r -p "USB-Autosuspend für diesen Hub dauerhaft deaktivieren? [y/N] " REPLY
            if [[ "$REPLY" =~ ^[Yy]$ ]]; then
                RULE_FILE="/etc/udev/rules.d/99-g8-hub-no-autosuspend.rules"
                RULE_LINE="SUBSYSTEM==\"usb\", ATTR{idVendor}==\"$HUB_VENDOR\", ATTR{idProduct}==\"$HUB_PRODUCT\", TEST==\"power/control\", ATTR{power/control}=\"on\""
                echo "$RULE_LINE" | sudo tee "$RULE_FILE" >/dev/null
                sudo udevadm control --reload-rules
                sudo udevadm trigger
                echo "on" | sudo tee "$HUB_POWER_FILE" >/dev/null
                echo "Erledigt: $RULE_FILE angelegt, sofort angewendet."
            else
                echo "Übersprungen -- keine Änderung vorgenommen."
            fi
        fi
    fi
fi
