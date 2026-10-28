#!/usr/bin/env bash
# Désinstalle luxafor-status.
# Usage : ./uninstall.sh [--all]   (--all retire aussi busylight et la règle udev)
set -euo pipefail

BINDIR="$HOME/.local/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"

ALL=0
[ "${1:-}" = "--all" ] && ALL=1

pkill -f "$BINDIR/luxafor-tray" >/dev/null 2>&1 || true
command -v busylight >/dev/null 2>&1 && busylight off >/dev/null 2>&1 || true

rm -f  "$BINDIR/luxafor-toggle" "$BINDIR/luxafor-tray"
rm -f  "$DATA_HOME/applications/luxafor.desktop"
rm -f  "$DATA_HOME/icons/hicolor/scalable/apps/luxafor.svg"
rm -rf "$DATA_HOME/luxafor-status"
rm -f  "$CONFIG_HOME/autostart/luxafor-tray.desktop"
rm -f  "$CACHE_HOME/luxafor-state"
update-desktop-database "$DATA_HOME/applications" >/dev/null 2>&1 || true
echo "✓ luxafor-status désinstallé"

if [ "$ALL" -eq 1 ]; then
    pipx uninstall busylight-for-humans >/dev/null 2>&1 && echo "✓ busylight-for-humans désinstallé" || true
    if [ -f /etc/udev/rules.d/99-busylights.rules ]; then
        sudo rm -f /etc/udev/rules.d/99-busylights.rules
        sudo udevadm control --reload-rules
        echo "✓ règle udev retirée"
    fi
fi
