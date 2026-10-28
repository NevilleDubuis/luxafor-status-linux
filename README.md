# luxafor-status-linux

Control a **Luxafor** status light from Ubuntu, with:

- 🖱️ **a dock button** — click to cycle through statuses, right-click to pick one;
- 🟢 **a system tray icon** that shows the current color;
- ⌨️ **a command** — `luxafor-toggle` — for scripts and keyboard shortcuts;
- 🎨 **your own statuses** — as many as you want, with any name and color.

Default statuses:

| Status    | Color     |
|-----------|-----------|
| Available | 🟢 green  |
| Busy      | 🔴 red    |
| On a call | 🟡 yellow |
| Off       | ⚪ —      |

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
  luxafor-toggle              # next status
  luxafor-toggle busy         # set a status by its id
  luxafor-toggle off
  luxafor-toggle --list       # list your statuses
  luxafor-toggle --current    # print the current status
  ```

  Handy for a custom keyboard shortcut (Settings → Keyboard → Custom Shortcuts).

## Customizing statuses

Statuses live in `~/.config/luxafor-status/statuses.conf`. Open it with:

```bash
luxafor-toggle --edit
```

or from the tray menu → **Edit statuses…**

One status per line, as `id | color | label`:

```
available | green   | Available
busy      | red     | Busy
call      | yellow  | On a call
focus     | #0000ff | Focus — do not disturb
lunch     | orange  | Lunch break
```

- **id** — the short name for `luxafor-toggle <id>` (lowercase letters, digits, `-`, `_`; `off` is reserved);
- **color** — a color name (`red`, `orange`, `purple`…) or a hex code (`#ff8800`);
- **label** — what you see in menus and notifications; any language, spaces allowed.

Add as many statuses as you like. The dock button cycles through them in file
order, then off. The tray icon picks up changes automatically when you save;
the dock's right-click menu is updated too (run `luxafor-toggle --reload` if
you edited the file while the tray icon wasn't running).

Your configuration is kept when you re-run `./install.sh`.

## Troubleshooting

| Problem | Fix |
|---|---|
| "could not reach the light" notification | Unplug/replug the light, then run `busylight list`. |
| "could not set color" notification | Check the color in your `statuses.conf` — try a hex code like `#ff8800`. |
| A status is missing from the menu | Run `luxafor-toggle --list`: invalid lines in `statuses.conf` are reported there. |
| `busylight list` shows nothing | Run `lsusb \| grep -i 04d8`. If the light is listed, it's the udev rule — re-run `./install.sh` and replug. |
| No tray icon | Make sure the "System Tray" applet is on your panel. On GNOME, install the *AppIndicator Support* extension. |
| `luxafor-toggle: command not found` | Log out and back in so `~/.local/bin` is on your `PATH`. |

## Uninstall

```bash
./uninstall.sh         # removes scripts, launchers and icons
./uninstall.sh --all   # also removes your statuses, busylight-for-humans and the udev rule
```

## Contributing

Contributions are very welcome! This is a small hobby project, and help of any
size is appreciated:

- 🧪 **testing** with other Luxafor models or Linux distributions / desktops;
- 🌍 **translations** of the default statuses and messages;
- 🐛 **bug reports** and fixes;
- ✨ **ideas** — e.g. automatic status from calendar, screen lock or video calls.

See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

## Credits

- [busylight-for-humans](https://github.com/JnyJny/busylight) by Erik O'Shaughnessy — the driver doing the actual work.

## License

MIT — see [LICENSE](LICENSE).
