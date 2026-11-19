#!/usr/bin/env bash
# Builds and installs luxafor-status for the current user, from source
# (Ubuntu / Debian). The .deb package or the apt repository are simpler:
# see README.md. Needs Rust (https://rustup.rs).
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
COMMANDS=(luxafor-light luxafor-toggle luxafor-tray luxafor-callwatch luxafor-panel)

DO_DEPS=1; DO_UDEV=1; DO_AUTOSTART=1
for arg in "$@"; do
    case "$arg" in
        --no-deps)      DO_DEPS=0 ;;
        --no-udev)      DO_UDEV=0 ;;
        --no-autostart) DO_AUTOSTART=0 ;;
        -h|--help)
            sed -n '2,5p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
command -v cargo >/dev/null 2>&1 || { echo "Rust is needed to build from source: https://rustup.rs" >&2; exit 1; }

# 1. System dependencies ------------------------------------------------------
if [ "$DO_DEPS" -eq 1 ]; then
    step "System dependencies"
    PKGS=(build-essential pkg-config libgtk-3-dev libayatana-appindicator3-dev pulseaudio-utils)
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

# 2. Build --------------------------------------------------------------------
step "Building (cargo build --release — a few minutes the first time)"
(cd "$SRC" && cargo build --release --quiet)
ok "built"

# 3. udev rule (USB access without sudo) --------------------------------------
if [ "$DO_UDEV" -eq 1 ]; then
    step "udev rule (USB access without sudo)"
    sudo install -m 0644 "$SRC/udev/70-luxafor-status.rules" "$UDEV_RULE"
    sudo udevadm control --reload-rules
    sudo udevadm trigger --subsystem-match=hidraw
    ok "rule installed in $UDEV_RULE"
fi

# 4. Program, icons, configuration, launcher ----------------------------------
step "Installing files"
mkdir -p "$BINDIR" "$SHAREDIR" "$APPICONDIR"
# Stop the running ones first (the file is replaced).
pkill -u "$(id -u)" -f "luxafor-(tray|callwatch)" >/dev/null 2>&1 || true
install -m 0755 "$SRC/target/release/luxafor-status" "$BINDIR/luxafor-status"
for cmd in "${COMMANDS[@]}"; do
    ln -sfn luxafor-status "$BINDIR/$cmd"   # the v1 command names
done
install -m 0644 "$SRC/share/icons/luxafor-status.svg" "$APPICONDIR/luxafor-status.svg"
install -m 0644 "$SRC/share/statuses.conf" "$SHAREDIR/statuses.conf"
# v1 leftovers: compiled translations (now built into the program).
rm -f "$DATA_HOME"/locale/*/LC_MESSAGES/luxafor-status.mo
gtk-update-icon-cache -q "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
ok "program in $BINDIR"

if [ -f "$USER_CONF" ]; then
    ok "keeping your statuses in $USER_CONF"
else
    mkdir -p "$(dirname "$USER_CONF")"
    install -m 0644 "$SRC/share/statuses.conf" "$USER_CONF"
    ok "default statuses written to $USER_CONF"
fi

"$BINDIR/luxafor-toggle" --reload >/dev/null
mkdir -p "$DATA_HOME/applications"
sed "s|@BINDIR@|$BINDIR|g" "$SRC/share/applications/luxafor-panel.desktop.in" \
    > "$DATA_HOME/applications/luxafor-panel.desktop"
ok "\"Luxafor\" launcher added to the app menu"

# v1 started the call detection on its own; the tray does it now.
rm -f "$AUTOSTART/luxafor-callwatch.desktop"
if [ "$DO_AUTOSTART" -eq 1 ]; then
    mkdir -p "$AUTOSTART"
    sed "s|@BINDIR@|$BINDIR|g" "$SRC/share/applications/luxafor-tray.desktop.in" \
        > "$AUTOSTART/luxafor-tray.desktop"
    ok "tray icon (and call detection) start at login"
fi

# 5. Start the tray icon now --------------------------------------------------
step "Starting the tray icon"
if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    nohup "$BINDIR/luxafor-tray" >/dev/null 2>&1 &
    disown || true
    ok "tray icon started"
else
    warn "no graphical session detected: the tray icon will start at next login"
fi

# 6. Check --------------------------------------------------------------------
step "Checking"
if LIGHTS="$("$BINDIR/luxafor-light" list 2>/dev/null)"; then
    while IFS= read -r l; do ok "light detected: $l"; done <<< "$LIGHTS"
else
    warn "no Luxafor detected — unplug and replug the light, then run: luxafor-light list"
fi

cat <<EOF

Done ✔

  • App menu → "Luxafor Panel": pick a status, optionally for a while
  • App menu → "Luxafor": one click = next status (drag either to your dock)
  • System tray: the dot shows the current color (click = menu)
  • Terminal: luxafor-toggle --list
  • Settings: Luxafor Panel → Settings, or luxafor-toggle --edit
  • Timer: luxafor-toggle busy --for 30
  • Call detection: luxafor-callwatch --check (during a call)

Uninstall: ./uninstall.sh
EOF
