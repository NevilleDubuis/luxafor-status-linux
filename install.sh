#!/usr/bin/env bash
# Installs luxafor-status for the current user (Ubuntu / Debian).
# Usage: ./install.sh [--no-deps] [--no-udev] [--no-autostart]
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINDIR="$HOME/.local/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
SHAREDIR="$DATA_HOME/luxafor-status"
APPICONDIR="$DATA_HOME/icons/hicolor/scalable/apps"
USER_CONF="$CONFIG_HOME/luxafor-status/statuses.conf"
AUTOSTART="$CONFIG_HOME/autostart"
UDEV_RULE="/etc/udev/rules.d/70-luxafor-status.rules"

DO_DEPS=1; DO_UDEV=1; DO_AUTOSTART=1
for arg in "$@"; do
    case "$arg" in
        --no-deps)      DO_DEPS=0 ;;
        --no-udev)      DO_UDEV=0 ;;
        --no-autostart) DO_AUTOSTART=0 ;;
        -h|--help)
            sed -n '2,3p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $arg" >&2; exit 2 ;;
    esac
done

step() { printf '\n\033[1;34m==>\033[0m %s\n' "$1"; }
ok()   { printf '    \033[32m✓\033[0m %s\n' "$1"; }
warn() { printf '    \033[33m!\033[0m %s\n' "$1"; }

if [ "$(id -u)" -eq 0 ]; then
    echo "Run this script as your normal user (not root); it will ask for sudo when needed." >&2
    exit 1
fi

# 1. System dependencies ------------------------------------------------------
if [ "$DO_DEPS" -eq 1 ]; then
    step "System dependencies"
    PKGS=(python3-gi gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1 libnotify-bin pulseaudio-utils)
    MISSING=()
    for p in "${PKGS[@]}"; do
        dpkg -s "$p" >/dev/null 2>&1 || MISSING+=("$p")
    done
    if [ "${#MISSING[@]}" -gt 0 ]; then
        echo "    Installing: ${MISSING[*]}"
        sudo apt-get update -qq
        sudo apt-get install -y "${MISSING[@]}"
    fi
    ok "apt packages present"
fi

# 2. udev rule (USB access without sudo) --------------------------------------
if [ "$DO_UDEV" -eq 1 ]; then
    step "udev rule (USB access without sudo)"
    sudo install -m 0644 "$SRC/udev/70-luxafor-status.rules" "$UDEV_RULE"
    sudo udevadm control --reload-rules
    sudo udevadm trigger --subsystem-match=hidraw
    ok "rule installed in $UDEV_RULE"
fi

# 3. Scripts, icons, configuration, launcher ----------------------------------
step "Installing files"
mkdir -p "$BINDIR" "$SHAREDIR" "$APPICONDIR"

install -m 0755 "$SRC/bin/luxafor-light"  "$BINDIR/luxafor-light"
install -m 0755 "$SRC/bin/luxafor-toggle" "$BINDIR/luxafor-toggle"
install -m 0755 "$SRC/bin/luxafor-tray"   "$BINDIR/luxafor-tray"
install -m 0755 "$SRC/bin/luxafor-callwatch" "$BINDIR/luxafor-callwatch"
install -m 0644 "$SRC/share/icons/luxafor-status.svg" "$APPICONDIR/luxafor-status.svg"
# Clean up icons from previous versions. An app icon named "luxafor" made
# Budgie's tray show it instead of the status color (see bin/luxafor-tray).
rm -f "$APPICONDIR/luxafor.svg" "$DATA_HOME"/icons/luxafor-tray-*.svg
install -m 0644 "$SRC/share/statuses.conf" "$SHAREDIR/statuses.conf"
gtk-update-icon-cache -q "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
ok "scripts in $BINDIR"

if [ -f "$USER_CONF" ]; then
    ok "keeping your statuses in $USER_CONF"
else
    mkdir -p "$(dirname "$USER_CONF")"
    install -m 0644 "$SRC/share/statuses.conf" "$USER_CONF"
    ok "default statuses written to $USER_CONF"
fi

"$BINDIR/luxafor-toggle" --reload >/dev/null
ok "\"Luxafor\" launcher added to the app menu"

if [ "$DO_AUTOSTART" -eq 1 ]; then
    mkdir -p "$AUTOSTART"
    sed "s|@BINDIR@|$BINDIR|g" "$SRC/share/applications/luxafor-tray.desktop.in" \
        > "$AUTOSTART/luxafor-tray.desktop"
    sed "s|@BINDIR@|$BINDIR|g" "$SRC/share/applications/luxafor-callwatch.desktop.in" \
        > "$AUTOSTART/luxafor-callwatch.desktop"
    ok "tray icon and call detection start at login"
fi

# 4. Start the tray icon and call detection now -------------------------------
step "Starting the tray icon and call detection"
pkill -f "$BINDIR/luxafor-tray" >/dev/null 2>&1 || true
pkill -f "$BINDIR/luxafor-callwatch" >/dev/null 2>&1 || true
if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    nohup "$BINDIR/luxafor-tray" >/dev/null 2>&1 &
    disown || true
    ok "tray icon started"
    if [ "$DO_AUTOSTART" -eq 1 ]; then
        nohup "$BINDIR/luxafor-callwatch" >/dev/null 2>&1 &
        disown || true
        ok "call detection started"
    fi
else
    warn "no graphical session detected: the tray icon will start at next login"
fi

# 5. Check --------------------------------------------------------------------
step "Checking"
if LIGHTS="$("$BINDIR/luxafor-light" list 2>/dev/null)"; then
    while IFS= read -r l; do ok "light detected: $l"; done <<< "$LIGHTS"
else
    warn "no Luxafor detected — unplug and replug the light, then run: luxafor-light list"
fi

cat <<EOF

Done ✔

  • App menu → "Luxafor": drag it to your dock (click = next status)
  • System tray: the dot shows the current color (click = menu)
  • Terminal: luxafor-toggle --list
  • Customize statuses: luxafor-toggle --edit
  • Call detection: luxafor-callwatch --check (during a call)

Uninstall: ./uninstall.sh
EOF
