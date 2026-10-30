# Contributing to luxafor-status-linux

Thanks for your interest! Every contribution is welcome, from a one-line
"works with my Flag" report to a new feature.

## Reporting hardware compatibility

So far only the **wired Luxafor ORB** has been tested. If you try another
model, please open an issue with:

- the model (e.g. Luxafor Flag, Bluetooth, Mute);
- the output of `lsusb | grep -i 04d8` and `luxafor-light list`;
- your distribution and desktop (e.g. Ubuntu 24.04, Budgie / GNOME / KDE);
- whether it worked, and what didn't.

## Reporting bugs

Open an issue describing what you did, what you expected and what happened.
Running `luxafor-toggle` from a terminal usually prints a useful error message.

## Submitting changes

1. Fork the repository and create a branch.
2. Keep changes small and focused — one topic per pull request.
3. Run the checks — CI runs the same ones on every pull request:

   ```bash
   tests/run.sh                 # driver and statuses, no light needed
   xvfb-run -a dbus-run-session -- tests/ui.py   # panel and tray
   shellcheck install.sh uninstall.sh bin/luxafor-light bin/luxafor-toggle \
              bin/luxafor-callwatch packaging/build-deb.sh tests/run.sh
   python3 -m py_compile bin/luxafor-tray bin/luxafor-panel
   ```

4. Test `./install.sh` and `./uninstall.sh` on your machine if you touched them.
5. Open a pull request explaining what and why.

## Building the package

```bash
packaging/build-deb.sh 0.1.0       # → dist/luxafor-status_0.1.0_all.deb
lintian --pedantic dist/*.deb      # should print nothing
```

It only needs `dpkg-deb` (no debhelper). The package installs to `/usr`, puts
the udev rule in `/usr/lib/udev/rules.d` and starts the tray icon through
`/etc/xdg/autostart`.

## Releasing

Push a version tag; the *Release* workflow tests, builds and publishes a GitHub
release with the `.deb` attached:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release is also added to the apt repository, a `gh-pages` branch served by
GitHub Pages: `stable` gets releases, `testing` gets every version
(`packaging/update-apt-repo.sh`). One-time setup, by the maintainer:

1. Create a signing key without a passphrase (the workflow signs unattended;
   the secret is encrypted by GitHub), and keep the exported file somewhere
   safe — a password manager, an encrypted drive:
   ```bash
   gpg --batch --passphrase '' --quick-gen-key "luxafor-status apt repository" ed25519 sign never
   gpg --armor --export-secret-keys "luxafor-status apt repository" > luxafor-apt-key.asc
   ```
2. Add the armored secret key as the repository secret `APT_SIGNING_KEY`
   (Settings → Secrets and variables → Actions).
3. After the first release, turn on GitHub Pages: Settings → Pages → Deploy
   from a branch → `gh-pages`, `/ (root)`.

Without the secret, releases still work; only the apt repository is skipped.

For a pre-release, add a suffix: `v0.3.0-beta.1`, `v0.3.0-rc.1`. It is
published as a GitHub pre-release (so "Latest" keeps pointing to the last
stable version) and the package version becomes `0.3.0~beta.1`, which apt
sorts before `0.3.0`. Tags that aren't `vX.Y.Z[-suffix]` are rejected.

## Project layout

```
bin/luxafor-light      driver: writes HID reports to /dev/hidraw* (pure bash)
bin/luxafor-toggle     sets or cycles the status (calls luxafor-light), and generates
                       the status icons and the dock launcher (--reload)
bin/luxafor-tray       tray icon showing the current status (Python + AppIndicator)
bin/luxafor-callwatch  call detection: watches microphone streams with `pactl`
bin/luxafor-panel      panel (Python + GTK 3): Status tab runs luxafor-toggle for every
                       action; Settings tab reads settings through luxafor-toggle and
                       rewrites statuses.conf
share/statuses.conf    default status configuration
share/icons/           application icon (luxafor-status)
share/applications/    autostart templates (@BINDIR@ is replaced at install time)
udev/                  udev rule granting the logged-in user access to the light
install.sh             per-user installer (from source)
uninstall.sh           uninstaller for the per-user install
packaging/             .deb build script, apt repository script, man pages, AppStream metadata
tests/run.sh           tests with a fake light
tests/ui.py            panel and tray tests, under a virtual display
po/                    translations (gettext): template, *.po, build and update scripts
docs/                  README images
.github/workflows/     CI (checks + package build) and releases
```

Runtime files:

- `~/.config/luxafor-status/statuses.conf` — the user's statuses
  (`id | color | label [| effect]`) and settings (`key = value`: `default-status`,
  `call-status`, `call-apps`, `lock-status`, `dnd-statuses`, `timer-shortcuts`,
  `brightness`, `theme`). `luxafor-toggle` and `luxafor-tray` both parse the statuses
  and must stay in sync if the format changes; other tools read settings with
  `luxafor-toggle --setting <key>`.
- `~/.cache/luxafor-dnd` — while the tray has silenced notifications: the
  user's own do-not-disturb setting, put back afterwards (also after a crash).
- `~/.cache/luxafor-state` — the current status id. The tray icon polls it
  (and the config file's modification time) every second, so any tool that
  calls `luxafor-toggle` keeps the tray in sync.
- `~/.cache/luxafor-timer` — the running timer, `<end epoch> <status id>`.
  A background `luxafor-toggle --timer-expire` process ends it; it checks the
  wall clock every 20 s so suspend doesn't delay it, and exits as soon as the
  file changes (status set by hand, new timer, cancel). `--keep-timer` (used
  by `luxafor-callwatch`) sets a status without cancelling the timer.
- `~/.local/share/luxafor-status/icons/` — one generated SVG per status (notifications).
- `~/.local/share/icons/lxftrayx-<id>-<hash>.svg` — the same, as a dashed
  ring, shown while the light is unplugged. The tray checks sysfs every second
  (same lookup as `luxafor-light`, honouring `LUXAFOR_IDS` / `LUXAFOR_SYSFS`)
  and runs `luxafor-toggle --apply` when the light comes back.
- `~/.local/share/icons/lxftray-<id>-<hash>.svg` — tray icons, written by
  `luxafor-tray`. They live in the standard icon folder because some tray hosts
  (e.g. Budgie) ignore the indicator's custom icon path; the color hash in the
  name prevents hosts from showing a cached icon after a color change.
  The prefix deliberately shares nothing with other icon names — see the
  comment in `bin/luxafor-tray` (GTK generic fallback).

## Testing without a light

`luxafor-light` reads `LUXAFOR_SYSFS` and `LUXAFOR_DEV`, so you can point it at
a fake sysfs tree and regular files, then check the bytes written:

```bash
mkdir -p /tmp/lx/sys/class/hidraw/hidraw0/device /tmp/lx/dev
printf 'HID_ID=0003:000004D8:0000F372\nHID_NAME=Fake Luxafor\n' \
  > /tmp/lx/sys/class/hidraw/hidraw0/device/uevent
touch /tmp/lx/dev/hidraw0
LUXAFOR_SYSFS=/tmp/lx/sys LUXAFOR_DEV=/tmp/lx/dev bin/luxafor-light orange
od -An -tx1 /tmp/lx/dev/hidraw0     # 00 01 ff ff a5 00 00 00 00
```

To support another model, add its `vendor:product` ID to `IDS` in
`bin/luxafor-light` and to `udev/70-luxafor-status.rules`.

## Translations

Messages use gettext (domain `luxafor-status`): `_()` in the Python scripts,
`t` / `tf` (printf format) in the shell scripts — one call per line start,
so `xgettext` sees it.

- Add a language: `msginit -i po/luxafor-status.pot -l de -o po/de.po`, translate,
  open a pull request.
- After changing messages: `po/update-pot.sh` (CI checks the template is up to date).
- Try it: `po/build.sh /tmp/locale && LUXAFOR_LOCALEDIR=/tmp/locale LANGUAGE=de luxafor-panel`
  (shell scripts also need a matching locale, e.g. `LC_ALL=de_DE.UTF-8`).

The names of the shipped statuses (`share/statuses.conf`) are translated
until the user has statuses of their own.

## Code of conduct

Be kind and patient. Everyone was a beginner once.
