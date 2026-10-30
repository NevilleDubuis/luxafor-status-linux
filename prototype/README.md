# v2 probe

`luxafor-probe.py` checks on a real computer that the building blocks of v2
(see issue #11) work: the light driven through **hidapi**, the **Qt** tray
icon (shown, changes color, receives clicks), and on Windows the detection
of apps using the **microphone** (calls).

It is a throwaway test, not part of the app.

## Linux (Ubuntu 24.04)

```bash
sudo apt install python3-pyqt6 python3-hid
python3 luxafor-probe.py
```

Access to the light comes from the udev rule installed by luxafor-status v1.

## Windows 11

Install [Python](https://www.python.org/downloads/) (tick *Add python.exe to
PATH*), then in a terminal:

```powershell
py -m pip install PySide6 hidapi
py luxafor-probe.py
```

## Then

1. Click the statuses: the light **and** the tray icon should change.
2. Click, right-click and middle-click the tray icon.
3. On Windows: join a Teams call — the log should say Teams uses the microphone.
4. Click **Copy report** and paste it in issue #11, with what you saw
   (did the light change? the tray icon? was the call detected?).
