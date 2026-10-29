# luxafor-status-linux

[![CI](https://github.com/NevilleDubuis/luxafor-status-linux/actions/workflows/ci.yml/badge.svg)](https://github.com/NevilleDubuis/luxafor-status-linux/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/NevilleDubuis/luxafor-status-linux)](https://github.com/NevilleDubuis/luxafor-status-linux/releases/latest)

Control a **Luxafor** status light from Ubuntu, with:

- 🖱️ **a dock button** — click to cycle through statuses, right-click to pick one;
- 🟢 **a system tray icon** that shows the current color;
- ⌨️ **a command** — `luxafor-toggle` — for scripts and keyboard shortcuts;
- 🎨 **your own statuses** — as many as you want, with any name and color;
- ⏱ **timers** — "busy for 30 minutes", then back to your default status;
- 📞 **automatic call status** — the light switches to a status of your choice
  while you're in a call in your browser (Teams, Odoo, Google Meet, Jitsi…).

Default statuses:

| Status    | Color     |
|-----------|-----------|
| Available | 🟢 green  |
| Busy      | 🔴 red    |
| On a call | 🟡 yellow |
| Off       | ⚪ —      |

Luxafor's official software does not support Linux. This project talks to the
light directly through its documented USB HID protocol: **no driver to install,
no Python package, nothing but bash** for the light itself.

## Hardware support

> [!IMPORTANT]
> So far, this has **only been tested with the wired Luxafor ORB**, on Ubuntu Budgie.

Other Luxafor models that use the same USB protocol (such as the Flag) *should*
work, but haven't been verified yet. If you try one, please
open an issue or a PR to report whether it works — even a quick "works for me" helps.

## Installation

### Debian package (recommended)

Download the latest `.deb` from the
[releases page](https://github.com/NevilleDubuis/luxafor-status-linux/releases/latest),
then:

```bash
sudo apt install ./luxafor-status_*_all.deb
```

`apt` installs the dependencies, the udev rule and the launcher, and starts the
tray icon at your next login (or run `luxafor-tray &` right away). To remove it:
`sudo apt remove luxafor-status`.

### From source

For other distributions, or to try the latest changes:

```bash
git clone https://github.com/NevilleDubuis/luxafor-status-linux.git
cd luxafor-status-linux
./install.sh
```

The installer:

1. installs the required packages (GTK/AppIndicator bindings for the tray icon, `notify-send`);
2. adds a **udev** rule so the light can be used without `sudo`;
3. copies the scripts to `~/.local/bin`, adds the launcher to the app menu, and starts the tray icon at login.

It only asks for your `sudo` password for steps 1 and 2.
If the light doesn't respond right away, unplug and replug it.

> [!NOTE]
> Use one method or the other. If you switch from the source install to the
> package, run `./uninstall.sh` first.

Options:

| Option           | Effect                                         |
|------------------|------------------------------------------------|
| `--no-deps`      | don't install apt packages                     |
| `--no-udev`      | don't install the udev rule                    |
| `--no-autostart` | don't start the tray icon at login             |

## Usage

- **Dock**: open the app menu, search for "Luxafor" and drag it to your dock or panel.
- **System tray**: click the dot to pick a status, or **Set for…** to set one
  for a while (15 min to 2 h). While a timer runs, the time left shows next to
  the dot and at the top of the menu, with **Cancel timer**.
- **Terminal**:

  ```bash
  luxafor-toggle              # next status
  luxafor-toggle busy         # set a status by its id
  luxafor-toggle busy --for 30   # for 30 minutes (also 45m, 1h, 1h30), then default status
  luxafor-toggle --timer      # time left
  luxafor-toggle --cancel-timer
  luxafor-toggle off
  luxafor-toggle --list       # list your statuses
  luxafor-toggle --current    # print the current status
  ```

  Or drive the light directly, without statuses:

  ```bash
  luxafor-light orange        # any color, see below
  luxafor-light off
  luxafor-light list          # connected lights
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
- **color** — a hex code (`#ff8800` or short `#f80`) or a color name: `red`, `orange`,
  `darkorange`, `gold`, `yellow`, `lime`, `green`, `springgreen`, `cyan`, `teal`,
  `skyblue`, `blue`, `royalblue`, `navy`, `purple`, `violet`, `magenta`, `pink`,
  `hotpink`, `crimson`, `coral`, `white`… (CSS names — `green` is CSS green, a darker
  green; use `lime` for full green);
- **label** — what you see in menus and notifications; any language, spaces allowed.

Add as many statuses as you like. The dock button cycles through them in file
order, then off. The tray icon picks up changes automatically when you save;
the dock's right-click menu is updated too (run `luxafor-toggle --reload` if
you edited the file while the tray icon wasn't running).

Your configuration is kept when you re-run `./install.sh`.

### Settings

The same file holds two settings, as `key = value` lines:

```ini
# Status to go back to when a timer ends (default: available, or your first status)
default-status = available

# Status used automatically while you are in a call (default: call)
call-status = call
```

`luxafor-toggle --settings` shows the values in use.

### Timers

`luxafor-toggle busy --for 30` (or **Set for…** in the tray menu) sets a status
for a while, then goes back to `default-status` with a notification.

- Choosing another status by hand cancels the timer.
- A call doesn't: when the call ends, the light goes back to the timed status
  if the timer is still running, or to `default-status` if it ended during the call.
- Timers survive suspending the laptop. If you log out before a timer ends, it
  resumes at your next login (or ends right away if it's already over).

## Automatic call status

`luxafor-callwatch` starts at login and watches which applications use your
microphone. When your web browser starts recording — which is what happens when
you join a call in Teams, Odoo, Google Meet, Jitsi or any other web app — the
light switches to your `call-status` (**On a call** by default — see
[Settings](#settings)). When the call ends, it goes back to the status you had
before (unless you changed it yourself during the call).

It works with PipeWire and PulseAudio, needs no browser extension, and keeps
working when you mute yourself during the call.

To check what it sees, run this **during a call**:

```bash
luxafor-callwatch --check
```

To choose the status used during calls, set `call-status` in your
statuses (`luxafor-toggle --edit`). Other options go in
`~/.config/luxafor-status/callwatch.conf`:

```ini
# Apps that count as "in a call" when they use the microphone
# (matched against the app name; default: common web browsers)
apps = firefox chrome chromium brave msedge vivaldi opera

# Set to "no" to turn call detection off
enabled = yes
```

Changes to `call-status` apply from the next call. After editing
`callwatch.conf`, log out and back in (or restart it:
`pkill -f luxafor-callwatch; luxafor-callwatch &`).

> [!TIP]
> Versions up to 0.2 read the call status from `status = …` in `callwatch.conf`.
> That still works, but `call-status` in `statuses.conf` takes precedence.

> [!NOTE]
> Any recording by a watched app counts as a call — for example a voice message
> recorded in a browser tab, or the microphone test screen before joining a
> Teams meeting.

## Troubleshooting

| Problem | Fix |
|---|---|
| "no Luxafor light found" notification | Unplug/replug the light, then run `luxafor-light list`. |
| "cannot write to /dev/hidraw…" | The udev rule isn't active: re-run `./install.sh`, then unplug/replug the light. |
| "unknown color" notification | Check the color in your `statuses.conf` — try a hex code like `#ff8800`. |
| A status is missing from the menu | Run `luxafor-toggle --list`: invalid lines in `statuses.conf` are reported there. |
| `luxafor-light list` shows nothing | Run `lsusb \| grep -i 04d8`. If your light shows a different ID than `04d8:f372`, please open an issue — it's probably a model that isn't supported yet. |
| No tray icon | Make sure the "System Tray" applet is on your panel. On GNOME, install the *AppIndicator Support* extension. |
| Call status doesn't switch | Run `luxafor-callwatch --check` during the call: is your browser listed? If it has an unusual name, add it to `apps`. If the call detection reports that `pactl` is missing: `sudo apt install pulseaudio-utils`. |
| `luxafor-toggle: command not found` | Log out and back in so `~/.local/bin` is on your `PATH`. |

## Upgrading from an earlier version

Versions before the built-in driver used `busylight-for-humans`. Once the new
version works, you can remove it:

```bash
pipx uninstall busylight-for-humans
sudo rm -f /etc/udev/rules.d/99-busylights.rules
```

## Uninstall

```bash
./uninstall.sh         # removes scripts, launchers and icons
./uninstall.sh --all   # also removes your statuses and the udev rule
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

- Luxafor, for publishing the HID protocol of their lights.
- [busylight-for-humans](https://github.com/JnyJny/busylight) by Erik O'Shaughnessy, which powered the first versions of this project.

## License

MIT — see [LICENSE](LICENSE).
