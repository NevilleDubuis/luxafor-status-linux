#!/usr/bin/env python3
"""luxafor-probe — checks that the v2 building blocks work on this computer.

v2 will be one cross-platform app (Linux, Windows) built on Qt and hidapi.
Before rewriting everything, this probe checks on real machines that:

  1. the light can be driven through hidapi;
  2. the Qt tray icon shows up, and changes color with the status;
  3. clicks on the tray icon reach the app;
  4. (Windows) a call is detected: which apps are using the microphone.

Run it, try the buttons and the tray icon, then click "Copy report" and
paste the report in the v2 issue (#11).

Linux (Ubuntu 24.04):  sudo apt install python3-pyqt6 python3-hid
                       python3 luxafor-probe.py
Windows:               py -m pip install PySide6 hidapi
                       py luxafor-probe.py
"""
import datetime
import platform
import sys

try:
    from PySide6 import QtCore, QtGui, QtWidgets
    BINDING = "PySide6"
except ImportError:
    from PyQt6 import QtCore, QtGui, QtWidgets
    BINDING = "PyQt6"

# hidapi: on Linux, the "hidraw" backend (the udev rule installed by
# luxafor-status v1 gives access to /dev/hidraw*); elsewhere "hid".
HID_ERROR = None
hid = None
for name in (["hidraw"] if sys.platform.startswith("linux") else []) + ["hid"]:
    try:
        hid = __import__(name)
        HID_MODULE = name
        break
    except ImportError as e:
        HID_ERROR = str(e)
else:
    HID_MODULE = None

VENDOR, PRODUCT = 0x04D8, 0xF372

STATUSES = [
    ("Available", "#00ff00", "steady"),
    ("Busy", "#ff0000", "steady"),
    ("On a call", "#ffff00", "steady"),
    ("Busy, blinking", "#ff0000", "blink"),
    ("Off", None, "steady"),
]


# --- light ---------------------------------------------------------------------

def find_lights():
    """[(path, product name)] of the connected Luxafor lights."""
    if hid is None:
        return []
    return [(d["path"], d.get("product_string") or "Luxafor")
            for d in hid.enumerate(VENDOR, PRODUCT)]


def report(color, effect):
    """The 8-byte Luxafor command, preceded by the report ID (0)."""
    if color is None:
        r = g = b = 0
    else:
        c = QtGui.QColor(color)
        r, g, b = c.red(), c.green(), c.blue()
    if effect == "blink":
        return [0, 3, 0xFF, r, g, b, 90, 0, 0]  # strobe, repeat forever
    return [0, 1, 0xFF, r, g, b, 0, 0, 0]       # static color


def send(color, effect):
    """Sends a status to every light; returns a line for the log."""
    if hid is None:
        return f"hidapi not available: {HID_ERROR}"
    lights = find_lights()
    if not lights:
        return "no Luxafor light found — is it plugged in?"
    results = []
    for path, _name in lights:
        dev = hid.device()
        try:
            dev.open_path(path)
            written = dev.write(report(color, effect))
            results.append(f"{written} bytes written" if written > 0 else f"write failed ({written})")
        except (OSError, IOError) as e:
            hint = " (Linux: is luxafor-status v1 installed? its udev rule gives access)" \
                if sys.platform.startswith("linux") else ""
            results.append(f"cannot open the light: {e}{hint}")
        finally:
            dev.close()
    return "; ".join(results)


# --- microphone (Windows) ------------------------------------------------------------

def microphone_users():
    """Apps using the microphone right now (Windows), or None elsewhere.

    Windows records it under CapabilityAccessManager: an app whose
    LastUsedTimeStop is 0 is using the microphone."""
    if sys.platform != "win32":
        return None
    import winreg

    base = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone"
    users = []

    def scan(path, prefix=""):
        try:
            key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, path)
        except OSError:
            return
        with key:
            n = 0
            while True:
                try:
                    sub = winreg.EnumKey(key, n)
                except OSError:
                    break
                n += 1
                if sub == "NonPackaged":
                    scan(f"{path}\\{sub}", "")
                    continue
                try:
                    with winreg.OpenKey(key, sub) as app:
                        stop, _t = winreg.QueryValueEx(app, "LastUsedTimeStop")
                        start, _t = winreg.QueryValueEx(app, "LastUsedTimeStart")
                except OSError:
                    continue
                if stop == 0 and start:
                    users.append(prefix + sub.replace("#", "\\"))

    scan(base)
    return users


# --- window ----------------------------------------------------------------------

def color_icon(color):
    pix = QtGui.QPixmap(64, 64)
    pix.fill(QtCore.Qt.GlobalColor.transparent)
    p = QtGui.QPainter(pix)
    p.setRenderHint(QtGui.QPainter.RenderHint.Antialiasing)
    if color is None:
        p.setPen(QtGui.QPen(QtGui.QColor("#888888"), 6))
        p.setBrush(QtCore.Qt.BrushStyle.NoBrush)
    else:
        p.setPen(QtCore.Qt.PenStyle.NoPen)
        p.setBrush(QtGui.QColor(color))
    p.drawEllipse(6, 6, 52, 52)
    p.end()
    return QtGui.QIcon(pix)


def sni_watcher():
    """Linux: is there a StatusNotifierItem host (what Qt uses for the tray)?"""
    if not sys.platform.startswith("linux"):
        return "n/a"
    try:
        QtDBus = __import__(f"{BINDING}.QtDBus", fromlist=["QtDBus"])
        bus = QtDBus.QDBusConnection.sessionBus()
        reply = bus.interface().isServiceRegistered("org.kde.StatusNotifierWatcher")
        return "yes" if reply.value() else "no (Qt falls back to the old XEmbed tray)"
    except Exception as e:  # noqa: BLE001 — diagnostics only
        return f"unknown ({e})"


class Probe(QtWidgets.QWidget):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Luxafor probe (v2)")
        self.resize(560, 520)
        layout = QtWidgets.QVBoxLayout(self)
        intro = QtWidgets.QLabel(
            "1. Click the statuses: the light <b>and</b> the tray icon should change.<br>"
            "2. Click, right-click and middle-click the tray icon.<br>"
            "3. Windows: join a Teams call to see if it is detected.<br>"
            "4. Copy the report and paste it in issue #11.")
        intro.setWordWrap(True)
        layout.addWidget(intro)

        buttons = QtWidgets.QHBoxLayout()
        for label, color, effect in STATUSES:
            b = QtWidgets.QPushButton(color_icon(color), label)
            b.clicked.connect(lambda _c=False, s=(label, color, effect): self.set_status(*s))
            buttons.addWidget(b)
        layout.addLayout(buttons)

        self.log_view = QtWidgets.QPlainTextEdit(readOnly=True)
        layout.addWidget(self.log_view)

        bottom = QtWidgets.QHBoxLayout()
        copy = QtWidgets.QPushButton("Copy report")
        copy.clicked.connect(self.copy_report)
        quit_button = QtWidgets.QPushButton("Quit")
        quit_button.clicked.connect(QtWidgets.QApplication.quit)
        bottom.addStretch()
        bottom.addWidget(copy)
        bottom.addWidget(quit_button)
        layout.addLayout(bottom)

        # Tray icon
        self.tray = QtWidgets.QSystemTrayIcon(color_icon(None))
        menu = QtWidgets.QMenu()
        for label, color, effect in STATUSES:
            action = menu.addAction(color_icon(color), label)
            action.triggered.connect(lambda _c=False, s=(label, color, effect): self.set_status(*s))
        menu.addSeparator()
        menu.addAction("Show the probe window").triggered.connect(self.show_window)
        menu.addAction("Quit").triggered.connect(QtWidgets.QApplication.quit)
        self.menu = menu  # keep a reference
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(self.on_tray)
        self.tray.setToolTip("Luxafor probe")
        self.tray.show()

        self.log("system", platform.platform())
        self.log("python", sys.version.split()[0])
        self.log("qt", f"{BINDING} {QtCore.qVersion()}, platform plugin "
                       f"'{QtGui.QGuiApplication.platformName()}'")
        self.log("tray available", QtWidgets.QSystemTrayIcon.isSystemTrayAvailable())
        self.log("tray host (SNI)", sni_watcher())
        self.log("hidapi", f"module '{HID_MODULE}'" if hid else f"missing: {HID_ERROR}")
        lights = find_lights()
        self.log("lights", ", ".join(name for _p, name in lights) or "none found")
        self.last_mic = None
        if sys.platform == "win32":
            self.check_microphone()
            self.mic_timer = QtCore.QTimer(self, interval=2000)
            self.mic_timer.timeout.connect(self.check_microphone)
            self.mic_timer.start()

    def log(self, key, value):
        stamp = datetime.datetime.now().strftime("%H:%M:%S")
        self.log_view.appendPlainText(f"{stamp}  {key}: {value}")

    def set_status(self, label, color, effect):
        self.tray.setIcon(color_icon(color))
        self.tray.setToolTip(f"Luxafor probe — {label}")
        self.log(f"status '{label}'", send(color, effect))

    def on_tray(self, reason):
        name = getattr(reason, "name", str(reason))
        self.log("tray click", name)
        if reason == QtWidgets.QSystemTrayIcon.ActivationReason.Trigger:
            self.show_window()

    def show_window(self):
        self.show()
        self.raise_()
        self.activateWindow()

    def check_microphone(self):
        users = microphone_users()
        if users != self.last_mic:
            self.last_mic = users
            self.log("microphone in use by", ", ".join(users) if users else "nobody")

    def copy_report(self):
        QtWidgets.QApplication.clipboard().setText(self.log_view.toPlainText())
        self.log("report", "copied to the clipboard")


def main():
    app = QtWidgets.QApplication(sys.argv)
    app.setQuitOnLastWindowClosed(False)  # closing the window keeps the tray icon
    probe = Probe()
    probe.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
