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
   sudo apt install libgtk-3-dev libayatana-appindicator3-dev   # to build
   cargo fmt --check && cargo clippy -- -D warnings
   cargo test                   # unit tests
   cargo build && tests/run.sh  # command line, with a fake light
   xvfb-run -a dbus-run-session -- tests/ui.sh   # tray and panel
   shellcheck install.sh uninstall.sh packaging/*.sh tests/*.sh po/*.sh
   ```

4. Test `./install.sh` and `./uninstall.sh` on your machine if you touched them.
5. Open a pull request explaining what and why.

## Building the package

```bash
cargo build --release
packaging/build-deb.sh 2.0.0       # → dist/luxafor-status_2.0.0_amd64.deb
lintian --pedantic dist/*.deb      # should print nothing
```

It only needs `dpkg-deb` and `dpkg-shlibdeps` (no debhelper). The package
installs to `/usr` (`luxafor-status`, and the v1 command names as links), puts
the udev rule in `/usr/lib/udev/rules.d` and starts the tray icon through
`/etc/xdg/autostart`. Releases are built on Ubuntu 22.04 so the packages work
there and later.

## Releasing

Push a version tag; the *Release* workflow tests, builds and publishes a GitHub
release with the packages (amd64, arm64) and the Windows `.exe` attached:

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
src/main.rs            one program, several commands (also reached through the
                       v1 command names: luxafor-toggle → toggle…)
src/toggle.rs          luxafor-toggle: sets or cycles the status, timers, settings
src/lightcli.rs        luxafor-light: the light alone
src/light.rs           driver: Linux writes HID reports to /dev/hidraw* (found
                       through sysfs), Windows uses hidapi
src/config.rs          statuses.conf: reading, settings, writing (panel)
src/state.rs           current status and timer files, durations
src/calls.rs           call detection: pactl (Linux), Windows' microphone list
src/tray/              tray icon (tray-icon crate): menu, light unplugged,
                       screen lock, do not disturb; runs the call detection
src/panel/             panel (egui): Status and Settings tabs; model.rs holds
                       the Settings logic, unit-tested
src/i18n.rs            translations, built in from po/*.po
src/desktop.rs         Linux dock launcher and notification icons (--reload)
share/statuses.conf    default status configuration
share/icons/           application icon (luxafor-status)
share/applications/    launcher templates (@BINDIR@ is replaced at install time)
udev/                  udev rule granting the logged-in user access to the light
install.sh             per-user install from source
packaging/             .deb build script, apt repository script, man pages, AppStream metadata
tests/run.sh           command-line tests with a fake light (from v1)
tests/ui.sh            tray and panel, under a virtual display
po/                    translations: template, *.po, extract and update scripts
docs/                  README images
.github/workflows/     CI and releases
```

Runtime files:

- `~/.config/luxafor-status/statuses.conf` — the user's statuses
  (`id | color | label [| effect]`) and settings (`key = value`: `default-status`,
  `call-status`, `call-apps`, `lock-status`, `dnd-statuses`, `timer-shortcuts`,
  `brightness`, `theme`), read by src/config.rs. On Windows:
  `%APPDATA%\luxafor-status\statuses.conf`, and the files below are in
  `%LOCALAPPDATA%\luxafor-status`.
- `~/.cache/luxafor-dnd` — while the tray has silenced notifications: the
  user's own do-not-disturb setting, put back afterwards (also after a crash).
- `~/.cache/luxafor-state` — the current status id. The tray icon polls it
  (and the config file's modification time) every second, so any tool that
  calls `luxafor-toggle` keeps the tray in sync.
- `~/.cache/luxafor-timer` — the running timer, `<end epoch> <status id>`.
  A background `luxafor-status toggle --timer-expire` process ends it; it
  checks the wall clock every 20 s so suspend doesn't delay it, and exits as
  soon as the file changes (status set by hand, new timer, cancel).
  `--keep-timer` (used by the tray for the screen lock) sets a status without
  cancelling the timer.
- `~/.cache/luxafor-tray.log` — errors met by the tray.
- `$XDG_RUNTIME_DIR/luxafor-*.lock` — one call detection per user (and one
  timer ending at a time; on Windows, one tray).
- `~/.local/share/luxafor-status/icons/` — one generated SVG per status (notifications).

The tray checks every second whether the light is plugged in (same lookup as
`luxafor-light`, honouring `LUXAFOR_IDS` / `LUXAFOR_SYSFS`), and shows the
current status on it again when it comes back. Its icon is drawn by the
program (a disc, or a dashed ring while the light is unplugged).

## Testing without a light

`luxafor-light` reads `LUXAFOR_SYSFS` and `LUXAFOR_DEV`, so you can point it at
a fake sysfs tree and regular files, then check the bytes written:

```bash
mkdir -p /tmp/lx/sys/class/hidraw/hidraw0/device /tmp/lx/dev
printf 'HID_ID=0003:000004D8:0000F372\nHID_NAME=Fake Luxafor\n' \
  > /tmp/lx/sys/class/hidraw/hidraw0/device/uevent
touch /tmp/lx/dev/hidraw0
LUXAFOR_SYSFS=/tmp/lx/sys LUXAFOR_DEV=/tmp/lx/dev target/debug/luxafor-status light orange
od -An -tx1 /tmp/lx/dev/hidraw0     # 00 01 ff ff a5 00 00 00 00
```

To support another model, add its `vendor:product` ID to `ids()` in
`src/light.rs` and to `udev/70-luxafor-status.rules`.

## Translations

Messages are marked with `tr("…")`, or `trf("… {name} …", &[("name", &value)])`
for placeholders. The `.po` files (gettext format) are built into the program:
nothing to compile or install.

- Add a language: `msginit -i po/luxafor-status.pot -l de -o po/de.po`,
  translate, add the file to `CATALOGS` in `src/i18n.rs`, open a pull request.
- After changing messages: `po/update-pot.sh` (CI checks the template is up to date).
- Try it: `LUXAFOR_LANG=de cargo run -- panel`.

The names of the shipped statuses (`share/statuses.conf`) are translated
until the user has statuses of their own.

## Code of conduct

Be kind and patient. Everyone was a beginner once.
