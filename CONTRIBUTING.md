# Contributing to luxafor-status-linux

Thanks for your interest! Every contribution is welcome, from a one-line
"works with my Flag" report to a new feature.

## Reporting hardware compatibility

So far only the **wired Luxafor ORB** has been tested. If you try another
model, please open an issue with:

- the model (e.g. Luxafor Flag, Bluetooth, Mute);
- the output of `lsusb | grep -i 04d8` and `busylight list`;
- your distribution and desktop (e.g. Ubuntu 24.04, Budgie / GNOME / KDE);
- whether it worked, and what didn't.

## Reporting bugs

Open an issue describing what you did, what you expected and what happened.
Running `luxafor-toggle` from a terminal usually prints a useful error message.

## Submitting changes

1. Fork the repository and create a branch.
2. Keep changes small and focused — one topic per pull request.
3. Check your shell scripts with [ShellCheck](https://www.shellcheck.net/):

   ```bash
   shellcheck install.sh uninstall.sh bin/luxafor-toggle
   python3 -m py_compile bin/luxafor-tray
   ```

4. Test `./install.sh` and `./uninstall.sh` on your machine if you touched them.
5. Open a pull request explaining what and why.

## Project layout

```
bin/luxafor-toggle     sets or cycles the status (calls `busylight`), and generates
                       the status icons and the dock launcher (--reload)
bin/luxafor-tray       tray icon showing the current status (Python + AppIndicator)
share/statuses.conf    default status configuration
share/icons/           application icon
share/applications/    tray autostart template (@BINDIR@ is replaced at install time)
udev/                  fallback udev rule
install.sh             per-user installer
uninstall.sh           uninstaller
```

Runtime files:

- `~/.config/luxafor-status/statuses.conf` — the user's statuses
  (`id | color | label`). `luxafor-toggle` and `luxafor-tray` both parse it and
  must stay in sync if the format changes.
- `~/.cache/luxafor-state` — the current status id. The tray icon polls it
  (and the config file's modification time) every second, so any tool that
  calls `luxafor-toggle` keeps the tray in sync.
- `~/.local/share/luxafor-status/icons/` — one generated SVG per status.

## Code of conduct

Be kind and patient. Everyone was a beginner once.
