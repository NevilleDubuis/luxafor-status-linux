#!/usr/bin/env bash
# Uninstalls luxafor-status.
# Usage: ./uninstall.sh [--all]
#   --all  also removes your status configuration, busylight-for-humans and the udev rule
set -euo pipefail

BINDIR="$HOME/.local/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"

ALL=0
[ "${1:-}" = "--all" ] && ALL=1

pkill -f "$BINDIR/luxafor-tray" >/dev/null 2>&1 || true
if command -v busylight >/dev/null 2>&1; then busylight off >/dev/null 2>&1 || true; fi

rm -f  "$BINDIR/luxafor-toggle" "$BINDIR/luxafor-tray"
rm -f  "$DATA_HOME/applications/luxafor.desktop"
rm -f  "$DATA_HOME/icons/hicolor/scalable/apps/luxafor.svg"
rm -f  "$DATA_HOME"/icons/luxafor-tray-*.svg
rm -rf "$DATA_HOME/luxafor-status"
rm -f  "$CONFIG_HOME/autostart/luxafor-tray.desktop"
rm -f  "$CACHE_HOME/luxafor-state"
update-desktop-database "$DATA_HOME/applications" >/dev/null 2>&1 || true
echo "✓ luxafor-status uninstalled"

if [ "$ALL" -eq 1 ]; then
    rm -rf "$CONFIG_HOME/luxafor-status"
    echo "✓ status configuration removed"
    if pipx uninstall busylight-for-humans >/dev/null 2>&1; then
        echo "✓ busylight-for-humans uninstalled"
    fi
    if [ -f /etc/udev/rules.d/99-busylights.rules ]; then
        sudo rm -f /etc/udev/rules.d/99-busylights.rules
        sudo udevadm control --reload-rules
        echo "✓ udev rule removed"
    fi
else
    echo "  (your statuses are kept in $CONFIG_HOME/luxafor-status — use --all to remove them)"
fi
