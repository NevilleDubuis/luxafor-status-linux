#!/usr/bin/python3
"""UI tests for luxafor-panel and luxafor-tray, run under a virtual display:

    xvfb-run -a dbus-run-session -- tests/ui.py [--screenshot out.png]

They use a fake light and isolated config dirs, like tests/run.sh.
"""
import os
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
TMP = tempfile.mkdtemp()

# Fake light and isolated user dirs — set before loading the scripts.
os.makedirs(f"{TMP}/sys/class/hidraw/hidraw0/device")
os.makedirs(f"{TMP}/dev")
with open(f"{TMP}/sys/class/hidraw/hidraw0/device/uevent", "w") as f:
    f.write("HID_ID=0003:000004D8:0000F372\nHID_NAME=Fake Luxafor\n")
open(f"{TMP}/dev/hidraw0", "w").close()
os.makedirs(f"{TMP}/config/luxafor-status")
with open(f"{TMP}/config/luxafor-status/statuses.conf", "w") as f:
    f.write(
        "available | green   | Available\n"
        "busy      | red     | Busy\n"
        "call      | yellow  | On a call\n"
        "focus     | #0000ff | Focus — do not disturb\n"
        "default-status = available\n"
    )
os.environ.update(
    HOME=f"{TMP}/home",
    XDG_CONFIG_HOME=f"{TMP}/config",
    XDG_CACHE_HOME=f"{TMP}/cache",
    XDG_DATA_HOME=f"{TMP}/data",
    LUXAFOR_SYSFS=f"{TMP}/sys",
    LUXAFOR_DEV=f"{TMP}/dev",
)
os.makedirs(os.environ["HOME"])
TOGGLE = f"{ROOT}/bin/luxafor-toggle"


def load(script):
    src = open(f"{ROOT}/bin/{script}").read().replace('if __name__ == "__main__":', "if False:")
    ns = {"__file__": f"{ROOT}/bin/{script}", "__name__": script}
    exec(compile(src, script, "exec"), ns)
    return ns


def toggle(*args):
    return subprocess.run([TOGGLE, *args], capture_output=True, text=True).stdout.strip()


panel_ns = load("luxafor-panel")
Gtk, Gdk, GLib = panel_ns["Gtk"], panel_ns["Gdk"], panel_ns["GLib"]

PASS = FAIL = 0


def eq(name, got, want):
    global PASS, FAIL
    if got == want:
        PASS += 1
        print(f"  \033[32m✓\033[0m {name}")
    else:
        FAIL += 1
        print(f"  \033[31m✗\033[0m {name}\n      expected [{want}], got [{got}]")


def pump(seconds=0.3):
    end = time.time() + seconds
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.02)


APP = Gtk.Application()  # no id: no D-Bus registration needed
APP.register(None)


def new_panel():
    calls = []
    window = panel_ns["Panel"](APP, run=lambda args: calls.append(args))
    window.show_all()
    window.refresh_header()
    pump()
    return window, calls


def key(window, keyval):
    event = Gdk.Event.new(Gdk.EventType.KEY_PRESS)
    event.keyval = keyval
    event.window = window.get_window()
    return window.on_key(window, event)


# CSS, as the app installs it at startup
provider = Gtk.CssProvider()
provider.load_from_data(panel_ns["CSS"])
Gtk.StyleContext.add_provider_for_screen(
    Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
)

print("luxafor-panel")
toggle("busy", "--for", "30")
w, calls = new_panel()
eq("one button per status, plus off", list(w.status_buttons), ["available", "busy", "call", "focus", "off"])
eq("current status highlighted", w.status_buttons["busy"].get_style_context().has_class("lx-current"), True)
eq("header shows the current status", w.head_name.get_text(), "Busy")
eq("header shows the timer", w.head_timer.get_text().endswith("30 min left, then Available"), True)
eq("header doesn't repeat the status", w.head_timer.get_text().startswith("until "), True)
eq("cancel button visible with a timer", w.cancel_button.get_visible(), True)
eq("∞ selected by default", w.duration_buttons[None].get_active(), True)

if "--screenshot" in sys.argv:
    out = sys.argv[sys.argv.index("--screenshot") + 1]
    pump(0.5)
    win = w.get_window()
    pix = Gdk.pixbuf_get_from_window(win, 0, 0, win.get_width(), win.get_height())
    pix.savev(out, "png", [], [])
    print(f"  (screenshot: {out})")

w.status_buttons["call"].clicked()
eq("click a status → luxafor-toggle <id>", calls, [["call"]])

w, calls = new_panel()
w.duration_buttons[60].set_active(True)
w.status_buttons["focus"].clicked()
eq("1 h then a status → --for 60", calls, [["focus", "--for", "60"]])

w, calls = new_panel()
w.spin.set_value(50)
eq("editing the custom duration selects it", w.duration_buttons["custom"].get_active(), True)
w.status_buttons["busy"].clicked()
eq("custom duration → --for 50", calls, [["busy", "--for", "50"]])

w, calls = new_panel()
key(w, Gdk.KEY_2)
eq("key 2 → second status", calls, [["busy"]])
w, calls = new_panel()
key(w, Gdk.KEY_KP_4)
eq("keypad 4 → fourth status", calls, [["focus"]])
w, calls = new_panel()
key(w, Gdk.KEY_9)
eq("key beyond the list does nothing", calls, [])

w, calls = new_panel()
w.cancel_button.clicked()
eq("cancel timer", calls, [["--cancel-timer"]])

w, calls = new_panel()
w.on_focus_out()
pump(0.4)
eq("focus lost before ever having it → stays open", w.get_visible(), True)
w.on_focus_in()
w.is_active = lambda: True   # focus moved to one of its own widgets
w.on_focus_out()
pump(0.4)
eq("focus moving inside the panel → stays open", w.get_visible(), True)
w.is_active = lambda: False  # click on another window
w.on_focus_out()
pump(0.4)
eq("click elsewhere → closes", w.get_visible(), False)

toggle("--cancel-timer")
w, calls = new_panel()
eq("no timer → cancel button hidden", w.cancel_button.get_visible(), False)
eq("no timer → header says so", w.head_timer.get_text(), "No timer")

# The real thing: run the panel's command, check the light and timer.
w, _ = new_panel()
w.run = lambda args: subprocess.run([TOGGLE, *args], capture_output=True)
w.duration_buttons[15].set_active(True)
w.status_buttons["busy"].clicked()
eq("end to end: status set", toggle("--current"), "busy")
eq("end to end: 15 min timer", toggle("--timer").endswith("then Available") and "15 min left" in toggle("--timer"), True)
toggle("--cancel-timer")

eq("theme setting defaults to auto", toggle("--setting", "theme"), "auto")
settings = Gtk.Settings.get_default()
panel_ns["apply_theme"]("dark")
eq("theme = dark → dark variant", settings.get_property("gtk-application-prefer-dark-theme"), True)
if "--screenshot" in sys.argv:
    out = sys.argv[sys.argv.index("--screenshot") + 1].replace(".png", "-dark.png")
    toggle("busy", "--for", "30")
    w, _ = new_panel()
    pump(0.5)
    win = w.get_window()
    Gdk.pixbuf_get_from_window(win, 0, 0, win.get_width(), win.get_height()).savev(out, "png", [], [])
    print(f"  (screenshot: {out})")
    toggle("--cancel-timer")
panel_ns["apply_theme"]("light")
eq("theme = light → light variant", settings.get_property("gtk-application-prefer-dark-theme"), False)
with open(f"{TMP}/config/luxafor-status/statuses.conf", "a") as f:
    f.write("theme = dark\n")
eq("theme setting read from statuses.conf", panel_ns["apply_theme"](), "dark")

print("luxafor-tray")
try:
    tray_ns = load("luxafor-tray")
    tray = tray_ns["Tray"]()
    pump(1.5)

    def labels():
        return [i.get_label() for i in tray.ind.get_menu().get_children()
                if not isinstance(i, Gtk.SeparatorMenuItem)]

    eq("menu: panel first, then statuses",
       labels(), ["Open panel…", "Available", "Busy", "On a call", "Focus — do not disturb", "Off",
                  "Edit statuses…", "Quit tray icon"])
    eq("middle-click opens the panel", tray.ind.get_secondary_activate_target().get_label(), "Open panel…")
    toggle("busy", "--for", "45")
    pump(1.5)
    tray.tick()
    eq("timer shown at the top", labels()[:2], [labels()[0], "Cancel timer"])
    eq("timer line", labels()[0].startswith("⏱ Busy until") and "45 min left" in labels()[0], True)
    toggle("--cancel-timer")
except Exception as e:  # AppIndicator needs a D-Bus session
    FAIL += 1
    print(f"  \033[31m✗\033[0m tray could not start: {e}")

print(f"\n{PASS} passed, {FAIL} failed")
subprocess.run(["rm", "-rf", TMP])
sys.exit(1 if FAIL else 0)
