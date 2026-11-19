#!/usr/bin/env bash
# Tray and panel, under a virtual display, with a fake light:
#
#   xvfb-run -a dbus-run-session -- tests/ui.sh [binary] [--screenshots DIR]
#
# The panel's logic is tested by `cargo test` (src/panel/model.rs); this
# checks that the tray and the panel run, and what the tray does with the
# light (plugged in, unplugged, plugged back in).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$ROOT/target/debug/luxafor-status"
SHOTS=""
while [ $# -gt 0 ]; do
    case "$1" in
        --screenshots) SHOTS="$2"; shift 2 ;;
        *) BINARY="$1"; shift ;;
    esac
done
BINARY="$(realpath "$BINARY")"
TMP="$(mktemp -d)"
PIDS=()
trap 'kill "${PIDS[@]}" 2>/dev/null; [ -n "${KEEP:-}" ] || rm -rf "$TMP"' EXIT

PASS=0; FAIL=0
ok()   { PASS=$((PASS + 1)); printf '  \033[32m✓\033[0m %s\n' "$1"; }
fail() {
    FAIL=$((FAIL + 1)); printf '  \033[31m✗\033[0m %s\n      %s\n' "$1" "${2:-}"
    if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::error title=Test failed::$1: ${2:-}"; fi
}
eq() { if [ "$2" = "$3" ]; then ok "$1"; else fail "$1" "expected [$3], got [$2]"; return 1; fi; }
show_errors() {  # file: in the CI summary, where job logs may not be reachable
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        grep -v AT-SPI "$1" | tail -5 | while IFS= read -r l; do echo "::error title=stderr::$l"; done
    fi
}


mkdir -p "$TMP/sys/class/hidraw/hidraw1/device" "$TMP/dev" "$TMP/config/luxafor-status" "$TMP/home"
printf 'HID_ID=0003:000004D8:0000F372\nHID_NAME=Fake Luxafor\n' > "$TMP/sys/class/hidraw/hidraw1/device/uevent"
: > "$TMP/dev/hidraw1"
cat > "$TMP/config/luxafor-status/statuses.conf" <<'EOF'
available | green   | Available
busy      | red     | Busy
call      | yellow  | On a call
focus     | #0000ff | Focus — do not disturb | blink-slow
default-status = available
timer-shortcuts = focus 1h30, busy 30
EOF
export LUXAFOR_SYSFS="$TMP/sys" LUXAFOR_DEV="$TMP/dev" HOME="$TMP/home" \
       XDG_CONFIG_HOME="$TMP/config" XDG_CACHE_HOME="$TMP/cache" XDG_DATA_HOME="$TMP/data" \
       LC_ALL=C.UTF-8 LANGUAGE=en LIBGL_ALWAYS_SOFTWARE=1
bytes() { od -An -tx1 "$TMP/dev/hidraw1" | tr -s ' ' | sed 's/^ //; s/ $//'; }
shot() {  # name
    [ -n "$SHOTS" ] || return 0
    mkdir -p "$SHOTS"
    xdotool mousemove 0 0 2>/dev/null; sleep 0.3
    import -window root -trim "$SHOTS/$1.png" 2>/dev/null && echo "  (screenshot: $SHOTS/$1.png)"
}

echo "luxafor-tray"
"$BINARY" toggle focus >/dev/null 2>&1
: > "$TMP/dev/hidraw1"
"$BINARY" tray 2>"$TMP/tray.err" & PIDS+=($!); TRAY=$!
sleep 2
eq "starts" "$(kill -0 $TRAY 2>/dev/null && echo running)" running
eq "shows the current status on the light at startup" "$(bytes)" "00 03 ff 00 00 ff b4 00 00"
eq "writes the dock launcher" "$([ -f "$TMP/data/applications/luxafor.desktop" ] && echo yes)" yes
mv "$TMP/sys/class/hidraw/hidraw1" "$TMP/away"; sleep 3
"$BINARY" toggle busy >/dev/null 2>&1
mv "$TMP/away" "$TMP/sys/class/hidraw/hidraw1"; : > "$TMP/dev/hidraw1"; sleep 2.5
eq "light plugged back in → the status set meanwhile is shown" "$(bytes)" "00 01 ff ff 00 00 00 00 00"
"$BINARY" tray 2>/dev/null & PIDS+=($!); sleep 2
eq "a second tray replaces the first" "$(kill -0 $TRAY 2>/dev/null && echo running || echo gone)" gone
eq "…and keeps running" "$(kill -0 "${PIDS[-1]}" 2>/dev/null && echo running)" running
eq "no errors" "$(grep -v AT-SPI "$TMP/tray.err" | grep -ci 'error\|panic')" 0 || show_errors "$TMP/tray.err"

echo "luxafor-panel"
"$BINARY" toggle busy --for 30 >/dev/null 2>&1
"$BINARY" panel 2>"$TMP/panel.err" & PIDS+=($!); PANEL=$!
sleep 3
eq "opens" "$(kill -0 $PANEL 2>/dev/null && echo running)" running
kill -0 $PANEL 2>/dev/null || show_errors "$TMP/panel.err"
shot panel
"$BINARY" panel --settings 2>/dev/null & PIDS+=($!); sleep 3
eq "a second panel replaces the first" "$(kill -0 $PANEL 2>/dev/null && echo running || echo gone)" gone
shot settings
LUXAFOR_LANG=fr "$BINARY" panel --settings 2>/dev/null & PIDS+=($!); sleep 3
shot settings-fr
eq "no errors" "$(grep -v AT-SPI "$TMP/panel.err" | grep -ci 'error\|panic')" 0 || show_errors "$TMP/panel.err"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
