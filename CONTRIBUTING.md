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
bin/luxafor-toggle     sets or cycles the status (calls `busylight`)
bin/luxafor-tray       tray icon showing the current status (Python + AppIndicator)
share/icons/           status icons
share/applications/    .desktop templates (@BINDIR@ is replaced at install time)
udev/                  fallback udev rule
install.sh             per-user installer
uninstall.sh           uninstaller
```

The current status is stored in `~/.cache/luxafor-state`; the tray icon polls
it every second, so any tool that calls `luxafor-toggle` keeps the tray in sync.

## Code of conduct

Be kind and patient. Everyone was a beginner once.
