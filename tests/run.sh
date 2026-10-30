#!/usr/bin/env bash
# Tests luxafor-light and luxafor-toggle against a fake sysfs and fake
# /dev/hidraw* files — no light needed. Usage: tests/run.sh
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

PASS=0; FAIL=0
ok()   { PASS=$((PASS + 1)); printf '  \033[32m✓\033[0m %s\n' "$1"; }
fail() {
    FAIL=$((FAIL + 1)); printf '  \033[31m✗\033[0m %s\n' "$1"; [ -n "${2:-}" ] && printf '      %s\n' "$2"
    # readable in the CI summary
    if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::error title=Test failed::$1: ${2:-}"; fi
}
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

# Messages in English, whatever the machine's language (translations are
# tested at the end).
export LC_ALL=C.UTF-8 LANGUAGE=en
export LUXAFOR_SYSFS="$TMP/sys" LUXAFOR_DEV="$TMP/dev"
export XDG_RUNTIME_DIR="$TMP/run"
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
"$LIGHT" red blink;        eq "blink → strobe report, repeat forever" "$(bytes hidraw1)" "00 03 ff ff 00 00 5a 00 00"
"$LIGHT" red blink slow;   eq "blink slow"  "$(bytes hidraw1)" "00 03 ff ff 00 00 b4 00 00"
"$LIGHT" red blink-fast;   eq "blink-fast"  "$(bytes hidraw1)" "00 03 ff ff 00 00 1e 00 00"
"$LIGHT" red steady;       eq "steady = static color" "$(bytes hidraw1)" "00 01 ff ff 00 00 00 00 00"
LUXAFOR_BLINK_SPEED=50 LUXAFOR_BRIGHTNESS=50 "$LIGHT" red blink
eq "LUXAFOR_BLINK_SPEED and brightness apply to blinking" "$(bytes hidraw1)" "00 03 ff 80 00 00 32 00 00"
"$LIGHT" red wobble 2>/dev/null; eq "unknown effect exits 2" "$?" 2
LUXAFOR_IDS=dead:beef "$LIGHT" red 2>/dev/null; eq "no light exits 3" "$?" 3
chmod 444 "$TMP/dev/hidraw1"
if ! [ -w "$TMP/dev/hidraw1" ]; then   # (root can write anyway)
    "$LIGHT" red 2>/dev/null; eq "light not writable exits 1" "$?" 1
fi
chmod 644 "$TMP/dev/hidraw1"

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
alert   | red     | Alerte | blink-fast
quiet   | blue    |  | blink-slow
EOF
eq "user config: invalid lines skipped" "$("$TOGGLE" --list 2>/dev/null | awk '{print $1}' | paste -sd' ')" "dispo focus pipe alert quiet off"
eq "labels keep spaces, accents and pipes" "$("$TOGGLE" --list 2>/dev/null | sed -n 3p | cut -c25-)" "Label | with pipe"
eq "effect (4th field) isn't part of the label" "$("$TOGGLE" --list 2>/dev/null | sed -n 4p | cut -c25-)" "Alerte"
eq "…no label: the id" "$("$TOGGLE" --list 2>/dev/null | sed -n 5p | cut -c25-)" "quiet"
eq "--list-effects" "$("$TOGGLE" --list-effects | paste -sd,)" "dispo steady,focus steady,pipe steady,alert blink-fast,quiet blink-slow"
"$TOGGLE" alert >/dev/null 2>&1
eq "blinking status → strobe" "$(bytes hidraw1)" "00 03 ff ff 00 00 1e 00 00"
"$TOGGLE" focus >/dev/null 2>&1
eq "set by id" "$(bytes hidraw1)" "00 01 ff 00 00 ff 00 00 00"
"$TOGGLE" nope >/dev/null 2>&1; eq "unknown id exits 2" "$?" 2
eq "unknown id keeps the current status" "$("$TOGGLE" --current)" focus
echo "legacy-green" > "$XDG_CACHE_HOME/luxafor-state"
"$TOGGLE" >/dev/null 2>&1; eq "unknown saved state restarts the cycle" "$("$TOGGLE" --current)" dispo

"$TOGGLE" --reload >/dev/null 2>&1
DESKTOP="$XDG_DATA_HOME/applications/luxafor.desktop"
eq "launcher: open panel, then one action per status" "$(sed -n 's/^Actions=//p' "$DESKTOP")" "panel;dispo;focus;pipe;alert;quiet;off;"
eq "one icon per status, plus off" "$(find "$XDG_DATA_HOME/luxafor-status/icons" -name "*.svg" | wc -l | tr -d ' ')" 6
if command -v desktop-file-validate >/dev/null 2>&1; then
    if desktop-file-validate "$DESKTOP"; then ok "launcher passes desktop-file-validate"
    else fail "launcher passes desktop-file-validate"; fi
fi

echo "light unplugged"
"$TOGGLE" focus >/dev/null 2>&1
LUXAFOR_IDS=dead:beef "$TOGGLE" pipe >/dev/null 2>"$TMP/away.err"
eq "status set while the light is away → succeeds" "$?" 0
eq "…and is kept" "$("$TOGGLE" --current)" pipe
eq "…saying it will be shown later" "$(grep -c 'no light connected' "$TMP/away.err")" 1
eq "…without touching the light" "$(bytes hidraw1)" "00 01 ff 00 00 ff 00 00 00"
: > "$TMP/dev/hidraw1"
"$TOGGLE" --apply; eq "--apply → exit 0" "$?" 0
eq "--apply shows the current status on the light" "$(bytes hidraw1)" "00 01 ff 80 00 80 00 00 00"
LUXAFOR_IDS=dead:beef "$TOGGLE" --apply 2>/dev/null; eq "--apply without a light → exit 3" "$?" 3
eq "--apply doesn't change the status" "$("$TOGGLE" --current)" pipe

echo "editing"
mkdir -p "$TMP/editbin"
printf '#!/bin/sh\necho "gui $*" > "%s/edited"\n' "$TMP" > "$TMP/editbin/xdg-open"
printf '#!/bin/sh\necho "terminal $*" > "%s/edited"\n' "$TMP" > "$TMP/editbin/fake-editor"
chmod +x "$TMP/editbin/"*
CONFFILE="$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
if command -v script >/dev/null 2>&1; then
    # In a terminal (script gives one): --edit uses the terminal editor…
    PATH="$TMP/editbin:$PATH" EDITOR=fake-editor script -qec "$TOGGLE --edit" /dev/null >/dev/null 2>&1
    eq "--edit in a terminal → terminal editor" "$(cat "$TMP/edited" 2>/dev/null)" "terminal $CONFFILE"
    # …but --gui (tray, panel) always opens the desktop editor, even when the
    # tray was started from a terminal.
    rm -f "$TMP/edited"
    # (the sleep keeps the test terminal open while the editor starts)
    PATH="$TMP/editbin:$PATH" EDITOR=fake-editor script -qec "$TOGGLE --edit --gui; sleep 1" /dev/null >/dev/null 2>&1
    eq "--edit --gui in a terminal → desktop editor" "$(cat "$TMP/edited" 2>/dev/null)" "gui $CONFFILE"
fi

echo "timer and settings"
TIMERF="$XDG_CACHE_HOME/luxafor-timer"
eq "default status falls back to the first status" "$("$TOGGLE" --setting default-status)" dispo
"$TOGGLE" focus --for 1h30 >/dev/null 2>&1
eq "--for 1h30" "$("$TOGGLE" --timer | grep -o '(.*left)')" "(1 h 30 left)"
"$TOGGLE" --for 45m focus >/dev/null 2>&1
eq "--for before the status, 45m" "$("$TOGGLE" --timer | grep -o '(.*left)')" "(45 min left)"
"$TOGGLE" focus --for 0 >/dev/null 2>&1; eq "--for 0 is rejected" "$?" 2
"$TOGGLE" focus --for soon >/dev/null 2>&1; eq "invalid duration is rejected" "$?" 2
"$TOGGLE" pipe >/dev/null 2>&1
eq "a status set by hand cancels the timer" "$("$TOGGLE" --timer)" "No timer."
short_timer() {  # status seconds
    "$TOGGLE" "$1" >/dev/null 2>&1
    echo "$(( $(date +%s) + $2 )) $1" > "$TIMERF"
    "$TOGGLE" --resume-timer
}
short_timer focus 1; sleep 2.5
eq "timer ends → default status" "$("$TOGGLE" --current)" dispo
eq "timer file removed" "$([ -f "$TIMERF" ] && echo yes || echo no)" no
short_timer focus 2; "$TOGGLE" pipe --keep-timer >/dev/null 2>&1; sleep 3.5
eq "timer ends while another status is on → left alone" "$("$TOGGLE" --current)" pipe
"$TOGGLE" focus >/dev/null 2>&1; echo "$(( $(date +%s) - 60 )) focus" > "$TIMERF"; "$TOGGLE" --resume-timer
eq "timer expired while logged out → default status at login" "$("$TOGGLE" --current)" dispo
"$TOGGLE" focus --for 10 >/dev/null 2>&1; "$TOGGLE" --cancel-timer
eq "--cancel-timer keeps the status" "$("$TOGGLE" --current) $("$TOGGLE" --timer)" "focus No timer."
printf 'default-status = pipe\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
eq "default-status setting" "$("$TOGGLE" --setting default-status)" pipe
eq "brightness defaults to 100" "$("$TOGGLE" --setting brightness)" 100
eq "lock-status defaults to off" "$("$TOGGLE" --setting lock-status)" off
eq "dnd-statuses default to none" "$("$TOGGLE" --setting dnd-statuses)" ""
eq "call-apps default to the usual browsers" "$("$TOGGLE" --setting call-apps | cut -d' ' -f1-2)" "firefox chrome"
printf 'brightness = 40\nlock-status = pipe\ndnd-statuses = focus, nope pipe\ncall-apps = firefox teams\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
eq "brightness setting" "$("$TOGGLE" --setting brightness)" 40
eq "lock-status setting" "$("$TOGGLE" --setting lock-status)" pipe
eq "dnd-statuses: valid ids kept" "$("$TOGGLE" --setting dnd-statuses)" "focus pipe"
eq "call-apps setting" "$("$TOGGLE" --setting call-apps)" "firefox teams"
"$TOGGLE" focus >/dev/null 2>&1
eq "brightness 40 % → colors scaled" "$(bytes hidraw1)" "00 01 ff 00 00 66 00 00 00"
printf 'brightness = 300\nlock-status = nope\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
eq "invalid brightness → 100" "$("$TOGGLE" --setting brightness)" 100
eq "invalid lock-status → off" "$("$TOGGLE" --setting lock-status)" off
printf 'brightness = 100\nlock-status =\ndnd-statuses =\ncall-apps =\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
eq "settings lines are not statuses" "$("$TOGGLE" --list 2>/dev/null | awk '{print $1}' | paste -sd' ')" "dispo focus pipe alert quiet off"
eq "timer shortcuts default: first non-default status, 30 min and 1 h" "$("$TOGGLE" --setting timer-shortcuts | paste -sd,)" "dispo 30,dispo 60"
printf 'timer-shortcuts = focus 1h30, nope 30, focus soon, pipe 15\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
eq "timer shortcuts: valid entries kept, in order" "$("$TOGGLE" --setting timer-shortcuts | paste -sd,)" "focus 90,pipe 15"
eq "timer shortcuts: invalid entries reported" "$("$TOGGLE" --settings 2>&1 >/dev/null | grep -c 'ignoring timer shortcut')" 2
printf 'timer-shortcuts =\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
eq "timer shortcuts: empty setting → none" "$("$TOGGLE" --setting timer-shortcuts)" ""

echo "luxafor-callwatch"
# Fake pactl: "list source-outputs" prints $TMP/streams, "subscribe" reads
# events from a FIFO that the test writes to.
mkdir -p "$TMP/fakebin"
mkfifo "$TMP/events"
cat > "$TMP/fakebin/pactl" <<EOF
#!/bin/sh
case "\$1" in
    list) cat "$TMP/streams" 2>/dev/null ;;
    subscribe) exec cat "$TMP/events" ;;
esac
EOF
chmod +x "$TMP/fakebin/pactl"
stream() {  # id app-name binary
    printf 'Source Output #%s\n\tDriver: PipeWire\n\tProperties:\n\t\tapplication.name = "%s"\n\t\tapplication.process.binary = "%s"\n' "$1" "$2" "$3"
}
event() { echo "Event '$1' on source-output #$2" >&3; sleep 0.5; }
WATCH="$ROOT/bin/luxafor-callwatch"
# Stops the watcher and its fake "pactl subscribe", which would otherwise
# keep reading (and stealing) events from the FIFO.
stop_watch() {
    kill "$WATCH_PID" 2>/dev/null
    pkill -f "cat $TMP/events" 2>/dev/null
    exec 3>&-
}
export PATH="$TMP/fakebin:$PATH" LUXAFOR_CALL_END_DELAY=1
printf 'call-status = focus   # blue during calls\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"

stream 41 "Audio Recorder" gnome-sound-recorder > "$TMP/streams"
eq "--check ignores non-browser apps" "$("$WATCH" --check | tail -1)" "→ Not a call."
stream 52 "Google Chrome" chrome > "$TMP/streams"
eq "--check detects a browser" "$("$WATCH" --check | tail -1)" "→ This counts as a call."

: > "$TMP/streams"
"$TOGGLE" dispo >/dev/null 2>&1
exec 3<>"$TMP/events"
"$WATCH" 2>/dev/null & WATCH_PID=$!
trap 'stop_watch; rm -rf "$TMP"' EXIT
sleep 0.5

"$WATCH" 2>"$TMP/second.err"
eq "a second watcher leaves at once" "$?" 0
eq "…saying the first one is running" "$(grep -c 'already running' "$TMP/second.err")" 1
eq "…and the first one keeps going" "$(kill -0 "$WATCH_PID" 2>/dev/null && echo yes || echo no)" yes

stream 52 "Google Chrome" chrome > "$TMP/streams"; event new 52
eq "call starts → configured status" "$("$TOGGLE" --current)" focus
eq "light shows the call color" "$(bytes hidraw1)" "00 01 ff 00 00 ff 00 00 00"
: > "$TMP/streams"; event remove 52
stream 53 "Google Chrome" chrome > "$TMP/streams"; event new 53; sleep 1
eq "brief microphone drop keeps the call" "$("$TOGGLE" --current)" focus
: > "$TMP/streams"; event remove 53; sleep 1.5
eq "call ends → previous status restored" "$("$TOGGLE" --current)" dispo

stream 60 Firefox firefox > "$TMP/streams"; event new 60
"$TOGGLE" pipe >/dev/null 2>&1
: > "$TMP/streams"; event remove 60; sleep 1.5
eq "status changed during the call is kept" "$("$TOGGLE" --current)" pipe

printf 'call-status = none\n' >> "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
"$TOGGLE" dispo >/dev/null 2>&1
stream 61 Firefox firefox > "$TMP/streams"; event new 61
eq "call-status = none → status left alone (read at each call)" "$("$TOGGLE" --current)" dispo
: > "$TMP/streams"; event remove 61; sleep 1.5
stop_watch
printf 'status = focus\n' > "$XDG_CONFIG_HOME/luxafor-status/callwatch.conf"
exec 3<>"$TMP/events"
"$WATCH" 2>"$TMP/old.err" & WATCH_PID=$!
sleep 0.5
eq "old callwatch.conf → says it's no longer read" "$(grep -c 'no longer read' "$TMP/old.err")" 1
stop_watch
rm -f "$XDG_CONFIG_HOME/luxafor-status/callwatch.conf"


echo "luxafor-callwatch with settings and timers"
cat > "$XDG_CONFIG_HOME/luxafor-status/statuses.conf" <<'CONF'
dispo | lime    | Disponible
focus | #0000ff | Focus
tel   | purple  | Au téléphone
default-status = dispo
call-status    = tel
CONF
: > "$TMP/streams"
exec 3<>"$TMP/events"
"$WATCH" 2>/dev/null & WATCH_PID=$!
sleep 0.5
"$TOGGLE" focus --for 30 >/dev/null 2>&1
stream 80 Firefox firefox > "$TMP/streams"; event new 80
eq "call-status from statuses.conf" "$("$TOGGLE" --current)" tel
eq "a call doesn't cancel the timer" "$([ -f "$TIMERF" ] && echo yes || echo no)" yes
: > "$TMP/streams"; event remove 80; sleep 1.5
eq "call ends, timer still running → back to the timed status" "$("$TOGGLE" --current)" focus
eq "…and the timer keeps running" "$([ -f "$TIMERF" ] && echo yes || echo no)" yes
short_timer focus 2
stream 81 Firefox firefox > "$TMP/streams"; event new 81
sleep 2.5
eq "timer ends during a call → call status kept" "$("$TOGGLE" --current)" tel
: > "$TMP/streams"; event remove 81; sleep 1.5
eq "call ends after the timer → default status" "$("$TOGGLE" --current)" dispo
stop_watch
"$TOGGLE" --cancel-timer

echo "translations"
FR_LOCALE="$(locale -a 2>/dev/null | grep -iE '^fr_[A-Z]{2}\.utf-?8$' | head -1)"
if command -v msgfmt >/dev/null 2>&1 && command -v gettext >/dev/null 2>&1 && [ -n "$FR_LOCALE" ]; then
    "$ROOT/po/build.sh" "$TMP/locale"
    fr() { LUXAFOR_LOCALEDIR="$TMP/locale" LC_ALL="$FR_LOCALE" LANGUAGE=fr "$@"; }
    rm -f "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"   # back to the shipped statuses
    eq "shipped status names translated" "$(fr "$TOGGLE" --list | awk '{print $3, $4}' | paste -sd,)" "Disponible ,Occupé ,En appel,Éteint "
    "$TOGGLE" busy --for 90 >/dev/null 2>&1
    eq "timer in French" "$(fr "$TOGGLE" --timer | sed 's/[0-9][0-9]:[0-9][0-9]/HH:MM/')" "Occupé jusqu’à HH:MM (encore 1 h 30), puis Disponible"
    "$TOGGLE" --cancel-timer
    eq "driver messages in French" "$(LUXAFOR_IDS=dead:beef fr "$LIGHT" red 2>&1)" "luxafor-light: aucune lampe Luxafor trouvée — est-elle branchée ?"
    mkdir -p "$XDG_CONFIG_HOME/luxafor-status"
    printf 'busy | red | Busy\n' > "$XDG_CONFIG_HOME/luxafor-status/statuses.conf"
    eq "the user's own status names are never translated" "$(fr "$TOGGLE" --list | head -1 | awk '{print $3}')" "Busy"
else
    echo "  (skipped: needs msgfmt, gettext and a French locale)"
fi

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
