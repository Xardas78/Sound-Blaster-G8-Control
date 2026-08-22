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
