# v2 probe

A throwaway test for the v2 rewrite in Rust (issue #11) — not the app. It
checks on a real computer that the building blocks work: the light driven
through **hidapi**, the **tray icon** (shown, changes color, its menu), the
window hidden and shown again from the tray, and on Windows the detection of
apps using the **microphone** (calls).

## Run it

Download the file for your system from the
[probe pre-release](https://github.com/NevilleDubuis/luxafor-status-linux/releases/tag/probe):

- **Windows 11**: `luxafor-probe-windows.exe`, then double-click it. Windows
  may warn about an unknown publisher (the file isn't signed): *More info →
  Run anyway*.
- **Linux**: `luxafor-probe-linux-x86_64`, then
  `chmod +x luxafor-probe-linux-x86_64 && ./luxafor-probe-linux-x86_64`.
  Access to the light comes from the udev rule of luxafor-status v1.
  On Linux it shows **two** tray icons (A and B, see their tooltips): they
  keep their pictures in different folders — tell which one changes color.

Then follow the steps in its window, click **Copy report** and paste the
report in issue #11 with what you saw.

## Build

```bash
sudo apt install libgtk-3-dev libayatana-appindicator3-dev libudev-dev   # Linux
cargo build --release
```
