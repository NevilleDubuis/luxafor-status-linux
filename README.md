# luxafor-status-linux

Control a **Luxafor** status light from Ubuntu, with:

- 🖱️ **a dock button** — click to cycle through statuses, right-click to pick one;
- 🟢 **a system tray icon** that shows the current color;
- ⌨️ **a command** — `luxafor-toggle` — for scripts and keyboard shortcuts.

| Status                 | Color      |
|------------------------|------------|
| Disponible (available) | 🟢 green   |
| Occupé (busy)          | 🔴 red     |
| En appel (on a call)   | 🟡 yellow  |
| Éteint (off)           | ⚪ —       |

> [!NOTE]
> The interface labels are currently in French. Translations are very welcome — see [Contributing](#contributing).

Luxafor's official software does not support Linux. This project builds on
[busylight-for-humans](https://pypi.org/project/busylight-for-humans/), an open
source driver that supports most Luxafor lights.

## Hardware support

> [!IMPORTANT]
> So far, this has **only been tested with the wired Luxafor ORB**, on Ubuntu Budgie.

Other Luxafor models (Flag, Bluetooth, Mute…) are supported by the underlying
driver and *should* work, but haven't been verified yet. If you try one, please
open an issue or a PR to report whether it works — even a quick "works for me" helps.

## Installation

```bash
git clone https://github.com/NevilleDubuis/luxafor-status-linux.git
cd luxafor-status-linux
./install.sh
```

The installer:

1. installs the required packages (`pipx`, GTK/AppIndicator bindings, `notify-send`);
2. installs `busylight-for-humans` with `pipx`;
3. adds a **udev** rule so the light can be used without `sudo`;
4. copies the scripts to `~/.local/bin`, adds the launcher to the app menu, and starts the tray icon at login.

It only asks for your `sudo` password for steps 1 and 3.
**Unplug and replug the light** after the first install.

Options:

| Option           | Effect                                         |
|------------------|------------------------------------------------|
| `--no-deps`      | don't touch apt packages or pipx               |
| `--no-udev`      | don't install the udev rule                    |
| `--no-autostart` | don't start the tray icon at login             |

## Usage

- **Dock**: open the app menu, search for "Luxafor" and drag it to your dock or panel.
- **System tray**: click the dot to pick a status.
- **Terminal**:

  ```bash
  luxafor-toggle          # next status
  luxafor-toggle red      # green | red | yellow | off
  ```

  Handy for a custom keyboard shortcut (Settings → Keyboard → Custom Shortcuts).

## Troubleshooting

| Problem | Fix |
|---|---|
| "Lampe non détectée" notification | Unplug/replug the light, then run `busylight list`. |
| `busylight list` shows nothing | Run `lsusb \| grep -i 04d8`. If the light is listed, it's the udev rule — re-run `./install.sh` and replug. |
| No tray icon | Make sure the "System Tray" applet is on your panel. On GNOME, install the *AppIndicator Support* extension. |
| `luxafor-toggle: command not found` | Log out and back in so `~/.local/bin` is on your `PATH`. |

## Uninstall

```bash
./uninstall.sh         # removes scripts, launchers and icons
./uninstall.sh --all   # also removes busylight-for-humans and the udev rule
```

## Contributing

Contributions are very welcome! This is a small hobby project, and help of any
size is appreciated:

- 🧪 **testing** with other Luxafor models or Linux distributions / desktops;
- 🌍 **translations** of the interface labels;
- 🐛 **bug reports** and fixes;
- ✨ **ideas** — e.g. automatic status from calendar, screen lock or video calls.

See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

## Credits

- [busylight-for-humans](https://github.com/JnyJny/busylight) by Erik O'Shaughnessy — the driver doing the actual work.

## License

MIT — see [LICENSE](LICENSE).
