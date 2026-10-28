#!/usr/bin/env bash
# Tests luxafor-light and luxafor-toggle against a fake sysfs and fake
# /dev/hidraw* files — no light needed. Usage: tests/run.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

PASS=0; FAIL=0
ok()   { PASS=$((PASS + 1)); printf '  \033[32m✓\033[0m %s\n' "$1"; }
fail() { FAIL=$((FAIL + 1)); printf '  \033[31m✗\033[0m %s\n' "$1"; [ -n "${2:-}" ] && printf '      %s\n' "$2"; }
eq()   { if [ "$2" = "$3" ]; then ok "$1"; else fail "$1" "expected [$3], got [$2]"; fi; }

# Fake devices: a mouse receiver (must be left alone) and two Luxafor lights.
fake_hidraw() {  # name hid_id hid_name
    mkdir -p "$TMP/sys/class/hidraw/$1/device" "$TMP/dev"
    printf 'HID_ID=%s\nHID_NAME=%s\n' "$2" "$3" > "$TMP/sys/class/hidraw/$1/device/uevent"
    : > "$TMP/dev/$1"
}
fake_hidraw hidraw0 0003:0000046D:0000C52B "Logitech USB Receiver"
fake_hidraw hidraw1 0003:000004D8:0000F372 "Microchip Technology Inc. LUXAFOR ORB"
fake_hidraw hidraw2 0003:000004D8:0000F372 "Microchip Technology Inc. LUXAFOR FLAG"

export LUXAFOR_SYSFS="$TMP/sys" LUXAFOR_DEV="$TMP/dev"
export HOME="$TMP/home" XDG_DATA_HOME="$TMP/data" XDG_CONFIG_HOME="$TMP/config" \
       XDG_CACHE_HOME="$TMP/cache" XDG_DATA_DIRS="$TMP/share"
mkdir -p "$HOME" "$TMP/share/luxafor-status"
cp "$ROOT/share/statuses.conf" "$TMP/share/luxafor-status/"

LIGHT="$ROOT/bin/luxafor-light"
TOGGLE="$ROOT/bin/luxafor-toggle"
bytes() { od -An -tx1 "$TMP/dev/$1" | tr -s ' ' | sed 's/^ //; s/ $//'; }

echo "luxafor-light"
eq "lists the two Luxafor lights only" "$("$LIGHT" list | wc -l | tr -d ' ')" 2
"$LIGHT" red
eq "red → report 00 01 ff ff 00 00 00 00 00" "$(bytes hidraw1)" "00 01 ff ff 00 00 00 00 00"
eq "every light gets the color" "$(bytes hidraw2)" "00 01 ff ff 00 00 00 00 00"
eq "other HID devices are left alone" "$(wc -c < "$TMP/dev/hidraw0" | tr -d ' ')" 0
"$LIGHT" '#FF8800';  eq "hex #rrggbb (any case)" "$(bytes hidraw1)" "00 01 ff ff 88 00 00 00 00"
"$LIGHT" f80;        eq "short hex without #"    "$(bytes hidraw1)" "00 01 ff ff 88 00 00 00 00"
"$LIGHT" Orange;     eq "CSS color name"         "$(bytes hidraw1)" "00 01 ff ff a5 00 00 00 00"
"$LIGHT" off;        eq "off"                    "$(bytes hidraw1)" "00 01 ff 00 00 00 00 00 00"
"$LIGHT" notacolor 2>/dev/null; eq "unknown color exits 2" "$?" 2
LUXAFOR_IDS=dead:beef "$LIGHT" red 2>/dev/null; eq "no light exits 1" "$?" 1

echo "luxafor-toggle"
eq "default statuses from XDG_DATA_DIRS" "$("$TOGGLE" --list | awk '{print $1}' | paste -sd' ')" "available busy call off"
"$TOGGLE" >/dev/null 2>&1; eq "cycle 1 → available" "$("$TOGGLE" --current)" available
eq "available is CSS green" "$(bytes hidraw1)" "00 01 ff 00 80 00 00 00 00"
"$TOGGLE" >/dev/null 2>&1; "$TOGGLE" >/dev/null 2>&1; "$TOGGLE" >/dev/null 2>&1
eq "cycles back to off after the last status" "$("$TOGGLE" --current)" off

mkdir -p "$XDG_CONFIG_HOME/luxafor-status"
cat > "$XDG_CONFIG_HOME/luxafor-status/statuses.conf" <<'EOF'
# custom
dispo   | lime    | Disponible
focus   | #0000ff | Focus — ne pas déranger
Bad Id  | red     | ignored
pipe    | purple  | Label | with pipe
EOF
eq "user config: invalid lines skipped" "$("$TOGGLE" --list 2>/dev/null | awk '{print $1}' | paste -sd' ')" "dispo focus pipe off"
eq "labels keep spaces, accents and pipes" "$("$TOGGLE" --list 2>/dev/null | sed -n 3p | cut -c25-)" "Label | with pipe"
"$TOGGLE" focus >/dev/null 2>&1
eq "set by id" "$(bytes hidraw1)" "00 01 ff 00 00 ff 00 00 00"
"$TOGGLE" nope >/dev/null 2>&1; eq "unknown id exits 2" "$?" 2
eq "unknown id keeps the current status" "$("$TOGGLE" --current)" focus
echo "legacy-green" > "$XDG_CACHE_HOME/luxafor-state"
"$TOGGLE" >/dev/null 2>&1; eq "unknown saved state restarts the cycle" "$("$TOGGLE" --current)" dispo

"$TOGGLE" --reload >/dev/null 2>&1
DESKTOP="$XDG_DATA_HOME/applications/luxafor.desktop"
eq "launcher has one action per status" "$(sed -n 's/^Actions=//p' "$DESKTOP")" "dispo;focus;pipe;off;"
eq "one icon per status, plus off" "$(find "$XDG_DATA_HOME/luxafor-status/icons" -name "*.svg" | wc -l | tr -d ' ')" 4
if command -v desktop-file-validate >/dev/null 2>&1; then
    if desktop-file-validate "$DESKTOP"; then ok "launcher passes desktop-file-validate"
    else fail "launcher passes desktop-file-validate"; fi
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
