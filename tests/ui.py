#!/usr/bin/python3
"""UI tests for luxafor-panel and luxafor-tray, run under a virtual display:

    xvfb-run -a dbus-run-session -- tests/ui.py [--screenshot out.png]

They use a fake light and isolated config dirs, like tests/run.sh.
"""
import os
import shutil
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
        "timer-shortcuts = focus 1h30, busy 30, nope 10\n"
    )
os.environ.update(
    HOME=f"{TMP}/home",
    XDG_CONFIG_HOME=f"{TMP}/config",
    XDG_CACHE_HOME=f"{TMP}/cache",
    XDG_DATA_HOME=f"{TMP}/data",
    LUXAFOR_SYSFS=f"{TMP}/sys",
    LUXAFOR_DEV=f"{TMP}/dev",
    GSETTINGS_BACKEND="memory",  # do-not-disturb tests never touch real settings
    LC_ALL="C.UTF-8", LANGUAGE="en",  # English messages; translations tested at the end
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
        if os.environ.get("GITHUB_ACTIONS"):  # readable in the CI summary
            print(f"::error title=UI test failed::{name}: expected [{want}], got [{got}]")


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
    window.focus_status()
    pump()
    return window, calls


def key(window, keyval):
    event = Gdk.Event.new(Gdk.EventType.KEY_PRESS)
    event.keyval = keyval
    event.window = window.get_window()
    return window.on_key(window, event)


# The virtual pointer sits mid-screen, where it would open tooltips over
# the screenshots: move it to a corner.
_display = Gdk.Display.get_default()
_display.get_default_seat().get_pointer().warp(_display.get_default_screen(), 0, 0)

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
w.emit("focus-out-event", Gdk.Event.new(Gdk.EventType.FOCUS_CHANGE))
pump(0.4)
eq("losing focus (e.g. while moving it) keeps it open", w.get_visible(), True)
key(w, Gdk.KEY_Escape)
pump(0.2)
eq("Esc closes it", w.get_visible(), False)

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
_conf = open(f"{TMP}/config/luxafor-status/statuses.conf").read()
with open(f"{TMP}/config/luxafor-status/statuses.conf", "a") as f:
    f.write("theme = dark\n")
eq("theme setting read from statuses.conf", panel_ns["apply_theme"](), "dark")
with open(f"{TMP}/config/luxafor-status/statuses.conf", "w") as f:
    f.write(_conf)

print("settings")
CONF_PATH = f"{TMP}/config/luxafor-status/statuses.conf"
saved_conf = open(CONF_PATH).read()
with open(CONF_PATH, "a") as f:
    f.write("my-own-key = kept\n")
apply_theme = panel_ns["apply_theme"]
apply_theme("light")
w, calls = new_panel()
w.stack.set_visible_child_name("settings")
pump()
page = w.settings


def labels_of(rows):
    return [r.label.get_text() for r in rows]


def combo_texts(combo):
    model = combo.get_model()
    return [row[0] for row in model]


eq("statuses listed in order", labels_of(page.rows()),
   ["Available", "Busy", "On a call", "Focus — do not disturb"])
eq("after a timer: available", page.default_combo.get_active_id(), "available")
eq("during calls: call", page.call_combo.get_active_id(), "call")
eq("timer shortcuts loaded (invalid one dropped)",
   [(r.status.get_active_id(), int(r.minutes.get_value())) for r in page.shortcut_rows()],
   [("focus", 90), ("busy", 30)])
eq("theme loaded", page.theme_combo.get_active_id(), "auto")
eq("nothing to save yet", page.save_button.get_sensitive(), False)
eq("keys 1–9 do nothing on the settings tab", key(w, Gdk.KEY_2), False)

if "--screenshot" in sys.argv:
    out = sys.argv[sys.argv.index("--screenshot") + 1].replace(".png", "-settings.png")
    pump(0.5)
    win = w.get_window()
    Gdk.pixbuf_get_from_window(win, 0, 0, win.get_width(), win.get_height()).savev(out, "png", [], [])
    print(f"  (screenshot: {out})")

rows = page.rows()
rows[1].label.set_text("Occupé")
eq("renaming marks the page as changed", page.save_button.get_sensitive(), True)
eq("…and updates the status choices", "Occupé" in combo_texts(page.call_combo), True)
blue = Gdk.RGBA()
blue.parse("#123456")
rows[0].color.set_rgba(blue)
rows[0].color.emit("color-set")
page.move(rows[3], -1)
eq("move up", labels_of(page.rows())[2], "Focus — do not disturb")
page.add_status()
page.rows()[-1].label.set_text("Pause café")
page.call_combo.set_active_id("call")
page.remove(page.rows()[3])  # "On a call"
eq("deleted status → its choices fall back", page.call_combo.get_active_id(), "available")
page.add_shortcut()
new = page.shortcut_rows()[-1]
new_key = next(k for k, label in page.choices() if label == "Pause café")
new.status.set_active_id(new_key)
new.minutes.set_value(15)
apply_theme("dark")
page.theme_combo.set_active_id("light")
eq("theme preview applies right away",
   Gtk.Settings.get_default().get_property("gtk-application-prefer-dark-theme"), False)
eq("save", page.save(), True)
eq("saved message", page.message.get_text(), "Saved.")
eq("statuses saved, new one gets an id from its name",
   [l.split()[0] for l in toggle("--list").splitlines()],
   ["available", "busy", "focus", "pause-cafe", "off"])
listed = {l.split()[0]: l.split(None, 2) for l in toggle("--list").splitlines()}
eq("changed color written as hex", listed["available"][1], "#123456")
eq("unchanged color kept as written", listed["busy"][1], "red")
eq("label saved", listed["busy"][2], "Occupé")
eq("call status saved", toggle("--setting", "call-status"), "available")
eq("timer shortcuts saved", toggle("--setting", "timer-shortcuts").splitlines(),
   ["focus 90", "busy 30", "pause-cafe 15"])
eq("theme saved", toggle("--setting", "theme"), "light")
eq("other settings kept", "my-own-key = kept" in open(CONF_PATH).read(), True)
eq("previous file kept as .bak", os.path.exists(CONF_PATH + ".bak"), True)
eq("status tab rebuilt with the new statuses", list(w.status_buttons),
   ["available", "busy", "focus", "pause-cafe", "off"])
eq("still on the settings tab after saving", w.stack.get_visible_child_name(), "settings")

if "--screenshot" in sys.argv:
    out = sys.argv[sys.argv.index("--screenshot") + 1].replace(".png", "-settings-dark.png")
    page.load()
    apply_theme("dark")
    pump(0.5)
    win = w.get_window()
    Gdk.pixbuf_get_from_window(win, 0, 0, win.get_width(), win.get_height()).savev(out, "png", [], [])
    print(f"  (screenshot: {out})")

eq("screen locked: light off by default", page.lock_combo.get_active_id(), "off")
eq("…choices: light off, the statuses, don't change",
   [page.lock_combo.get_model()[0][0], page.lock_combo.get_model()[len(page.lock_combo.get_model()) - 1][0]],
   ["Light off", "Don't change"])
eq("brightness loaded", int(page.brightness.get_value()), 100)
eq("no dnd status by default", [c.key for c in page.dnd_checks() if c.get_active()], [])
page.lock_combo.set_active_id("focus")
page.call_combo.set_active_id("none")
page.brightness.set_value(60)
next(c for c in page.dnd_checks() if c.key == "busy").set_active(True)
eq("save behavior settings", page.save(), True)
eq("lock-status saved", toggle("--setting", "lock-status"), "focus")
eq("call-status none saved (call detection off)", toggle("--setting", "call-status"), "none")
eq("brightness saved", toggle("--setting", "brightness"), "60")
eq("dnd-statuses saved", toggle("--setting", "dnd-statuses"), "busy")
eq("reloaded: dnd check box kept", [c.key for c in page.dnd_checks() if c.get_active()], ["busy"])
page.lock_combo.set_active_id("none")
page.load()
eq("revert puts the combo boxes back too", page.lock_combo.get_active_id(), "focus")

page.rows()[0].label.set_text("   ")
eq("empty name → not saved", page.save(), False)
eq("…with a message", page.message.get_text(), "Give every status a name.")
page.load()
eq("revert → back to the saved file", labels_of(page.rows())[0], "Available")
for row in page.rows()[1:]:
    page.remove(row)
page.remove(page.rows()[0])
eq("the last status can't be deleted", len(page.rows()), 1)
eq("…and it says why", page.message.get_text(), "Keep at least one status.")
w.destroy()

with open(CONF_PATH, "w") as f:
    f.write(saved_conf)
apply_theme("light")

print("starting the background parts from the panel")
started = []
os.makedirs(f"{TMP}/config/autostart", exist_ok=True)
eq("nothing running → tray and call detection started",
   panel_ns["start_background"](lambda path: started.append(os.path.basename(path))),
   ["luxafor-tray", "luxafor-callwatch"])
eq("…from the scripts next to the panel", started, ["luxafor-tray", "luxafor-callwatch"])
owned = []
Gio = panel_ns["Gio"]
owner = Gio.bus_own_name(Gio.BusType.SESSION, panel_ns["TRAY_BUS_NAME"], 0,
                         None, lambda *_a: owned.append(True), None)
pump(1.0)
eq("tray running (bus name owned) → only call detection started",
   panel_ns["start_background"](lambda path: None), ["luxafor-callwatch"])
Gio.bus_unown_name(owner)
pump(0.5)
with open(f"{TMP}/config/autostart/luxafor-tray.desktop", "w") as f:
    f.write("[Desktop Entry]\nHidden=true\n")
with open(f"{TMP}/config/autostart/luxafor-callwatch.desktop", "w") as f:
    f.write("[Desktop Entry]\nX-GNOME-Autostart-enabled=false\n")
eq("autostart turned off by the user → nothing started",
   panel_ns["start_background"](lambda path: None), [])
subprocess.run(["rm", "-rf", f"{TMP}/config/autostart"])

print("luxafor-tray")
try:
    tray_ns = load("luxafor-tray")
    tray = tray_ns["Tray"]()
    pump(1.5)

    def items():
        return [i for i in tray.ind.get_menu().get_children()]

    def labels():
        return [i.get_label() for i in items() if not isinstance(i, Gtk.SeparatorMenuItem)]

    def sections():
        out, cur = [], []
        for i in items():
            if isinstance(i, Gtk.SeparatorMenuItem):
                out.append(cur)
                cur = []
            else:
                cur.append(i.get_label())
        return out + [cur]

    eq("menu sections: statuses / timers / off / settings", sections(), [
        ["Available", "Busy", "On a call", "Focus — do not disturb"],
        ["Focus — do not disturb for 1 h 30", "Busy for 30 min", "More…"],
        ["Off"],
        ["Quit"],
    ])
    eq("middle-click opens the panel", tray.ind.get_secondary_activate_target().get_label(), "More…")

    def click(label):
        item = next(i for i in items() if i.get_label() == label)
        item.activate()
        pump(1.0)

    click("Busy for 30 min")
    eq("timer shortcut → status set", toggle("--current"), "busy")
    eq("timer shortcut → 30 min timer", "30 min left" in toggle("--timer"), True)
    tray.tick()
    pump(0.3)
    eq("timer shown at the top", labels()[:2], [labels()[0], "Cancel timer"])
    eq("timer line", labels()[0].startswith("⏱ Busy until") and "30 min left" in labels()[0], True)
    click("Cancel timer")
    eq("cancel timer from the menu", toggle("--timer"), "No timer.")
    click("Off")
    eq("off from the menu", toggle("--current"), "off")

    # An error during an update must not stop the updates (GLib drops a
    # timeout whose callback raises).
    real = tray.update_timer
    tray.update_timer = lambda: (_ for _ in ()).throw(RuntimeError("boom"))
    eq("tick survives an error", tray.tick(), True)
    tray.update_timer = real
    log = open(f"{TMP}/cache/luxafor-tray.log").read()
    eq("error logged to ~/.cache/luxafor-tray.log", "RuntimeError: boom" in log, True)
    toggle("busy")
    pump(1.5)
    eq("updates go on after the error", tray.last_state, ("busy", True))

    # Laptop leaves its dock: the light disappears, then comes back.
    uevent = f"{TMP}/sys/class/hidraw/hidraw0/device/uevent"
    os.rename(uevent, uevent + ".away")
    tray.update_light()  # first check right after: no pump in between
    eq("one missed check isn't enough (dock flicker)", tray.connected, True)
    pump(2.2)
    eq("light unplugged → noticed", tray.connected, False)
    eq("…dashed icon", tray.ind.get_icon().startswith("lxftrayx-busy-"), True)
    eq("…warning at the top of the menu", labels()[0], "⚠ Light not connected")
    toggle("call")
    pump(1.2)
    eq("status changed while away → kept", toggle("--current"), "call")
    eq("…and shown on the dashed icon", tray.ind.get_icon().startswith("lxftrayx-call-"), True)
    open(f"{TMP}/dev/hidraw0", "w").close()
    os.rename(uevent + ".away", uevent)
    pump(1.5)
    eq("light plugged back → noticed", tray.connected, True)
    eq("…status shown on the light again", open(f"{TMP}/dev/hidraw0", "rb").read().hex(), "0001ffffff00000000")  # call = yellow
    eq("…normal icon", tray.ind.get_icon().startswith("lxftray-call-"), True)
    eq("…warning gone", labels()[0] != "⚠ Light not connected", True)

    # Screen lock (Budgie and GNOME: org.gnome.ScreenSaver ActiveChanged)
    Gio = panel_ns["Gio"]
    conn = Gio.bus_get_sync(Gio.BusType.SESSION)

    def screensaver(active):
        conn.emit_signal(None, "/org/gnome/ScreenSaver", "org.gnome.ScreenSaver",
                         "ActiveChanged", GLib.Variant("(b)", (active,)))
        pump(1.2)

    CONF_PATH = f"{TMP}/config/luxafor-status/statuses.conf"

    def add_setting(line):
        with open(CONF_PATH, "a") as f:
            f.write(line + "\n")
        pump(1.5)  # the tray reloads the file when it changes

    toggle("busy")
    pump(1.2)
    screensaver(True)
    eq("screen locked → light off (default lock-status)", toggle("--current"), "off")
    screensaver(False)
    eq("unlocked → previous status back", toggle("--current"), "busy")
    toggle("busy", "--for", "30")
    screensaver(True)
    eq("locking keeps a running timer", "30 min left" in toggle("--timer"), True)
    screensaver(False)
    eq("…and the timed status comes back", toggle("--current"), "busy")
    toggle("--cancel-timer")
    screensaver(True)
    toggle("call")
    screensaver(False)
    eq("status changed while locked is kept", toggle("--current"), "call")
    add_setting("lock-status = focus")
    screensaver(True)
    eq("lock-status = focus", toggle("--current"), "focus")
    screensaver(False)
    eq("…back after unlocking", toggle("--current"), "call")
    add_setting("lock-status = none")
    screensaver(True)
    eq("lock-status = none → status left alone", toggle("--current"), "call")
    screensaver(False)

    # Do not disturb
    dnd = tray.dnd
    eq("do-not-disturb switch found", dnd.available, True)
    if dnd.available:
        toggle("busy")
        pump(1.2)
        eq("no dnd-statuses → notifications left alone", dnd.get(), False)
        add_setting("dnd-statuses = busy, focus")
        eq("busy is a dnd status → notifications silenced", dnd.get(), True)
        toggle("focus")
        pump(1.2)
        eq("…still while switching to another dnd status", dnd.get(), True)
        toggle("available")
        pump(1.2)
        eq("other status → notifications back", dnd.get(), False)
        dnd.set(True)  # the user had silenced notifications themselves
        toggle("busy")
        pump(1.2)
        toggle("available")
        pump(1.2)
        eq("…and the user's own setting is put back", dnd.get(), True)
        dnd.set(False)
        toggle("busy")
        pump(1.2)
        tray.restore_dnd()
        eq("quitting the tray never leaves notifications silenced", dnd.get(), False)
    toggle("--cancel-timer")
except Exception as e:  # AppIndicator needs a D-Bus session
    FAIL += 1
    print(f"  \033[31m✗\033[0m tray could not start: {e}")

print("translations")
if shutil.which("msgfmt"):
    subprocess.run([f"{ROOT}/po/build.sh", f"{TMP}/locale"], check=True, capture_output=True)
    probe = (
        "import sys\n"
        "for name in ('luxafor-panel', 'luxafor-tray'):\n"
        "    src = open(sys.argv[1] + '/bin/' + name).read().replace('if __name__ == \"__main__\":', 'if False:')\n"
        "    ns = {'__file__': sys.argv[1] + '/bin/' + name, '__name__': 'x'}\n"
        "    exec(compile(src, name, 'exec'), ns)\n"
        "    print(ns['_']('Settings'), '|', ns['_']('More…'), '|', ns['human_minutes'](90))\n"
    )
    out = subprocess.run(
        [sys.executable, "-c", probe, ROOT], capture_output=True, text=True,
        env=dict(os.environ, LUXAFOR_LOCALEDIR=f"{TMP}/locale", LANGUAGE="fr", LC_ALL="", LANG="fr_CH.UTF-8"),
    ).stdout.splitlines()
    eq("panel and tray in French", out, ["Réglages | Plus… | 1 h 30", "Réglages | Plus… | 1 h 30"])
else:
    print("  (skipped: needs msgfmt)")

print(f"\n{PASS} passed, {FAIL} failed")
subprocess.run(["rm", "-rf", TMP])
sys.exit(1 if FAIL else 0)
