//! luxafor-tray: the tray icon showing the current status, with a menu to
//! change it. It also runs everything that must happen in the background:
//! call detection, the screen-lock status, do not disturb, and showing the
//! status again when the light is plugged back in.
//!
//! The status itself lives in files (see state.rs): the tray checks them
//! every second, so the command line, the panel and the tray always agree.

#[cfg(target_os = "linux")]
mod desktop_linux;
mod icons;

use std::process::{Child, Command, Stdio};
use std::time::SystemTime;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::Config;
use crate::i18n::{tr, trf};
use crate::notify::notify;
use crate::state::{self, human_minutes};
use crate::{color, light, paths, toggle};

/// D-Bus name of the tray (Linux), same as v1: a new tray takes over, the
/// previous one (v1 included) sees the name go and quits.
pub const BUS_NAME: &str = "io.github.nevilledubuis.luxafor_status.tray";
/// A dock being unplugged can flicker: wait this many checks (1 s apart)
/// before saying the light is gone.
const AWAY_AFTER: u32 = 2;
/// Freshly plugged in, the light may not be writable yet (udev): retry.
const APPLY_TRIES: u32 = 10;

/// Errors go to stderr and to the log file (kept short): started at login,
/// the tray has no terminal to show them.
pub fn log(message: &str) {
    let line = format!(
        "{} {message}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    eprintln!("{line}");
    let path = paths::tray_log();
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = old.lines().collect();
    let skip = lines.len().saturating_sub(400);
    lines.drain(..skip);
    lines.push(&line);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, lines.join("\n") + "\n");
}

#[derive(Clone)]
enum Action {
    Toggle(Vec<String>),
    Panel,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    config: Config,
    mtime: Option<Option<SystemTime>>,
    actions: Vec<(MenuId, Action)>,
    shown: Option<(String, Option<bool>)>,
    timer: Option<state::Timer>,
    timer_item: Option<MenuItem>,
    timer_text: Option<String>,
    // Light: None until the first check.
    connected: Option<bool>,
    missing_checks: u32,
    apply_tries: u32,
    /// Say "reconnected" once the status is back on the light.
    announce: bool,
    /// While the screen is locked: (status before, end of the timer then).
    lock: Option<(String, Option<i64>)>,
    #[cfg(target_os = "linux")]
    dnd: desktop_linux::Dnd,
    dnd_state: Option<bool>,
    children: Vec<Child>,
    pub quit: bool,
}

impl Tray {
    pub fn new() -> Result<Tray, String> {
        let config = Config::load(false)?;
        let icon = TrayIconBuilder::new()
            .with_id("luxafor-status")
            .with_tooltip("Luxafor")
            .with_icon(make_icon(crate::color::OFF_COLOR, false))
            .build()
            .map_err(|e| e.to_string())?;
        let mut tray = Tray {
            icon,
            config,
            mtime: None,
            actions: Vec::new(),
            shown: None,
            timer: None,
            timer_item: None,
            timer_text: None,
            connected: None,
            missing_checks: 0,
            apply_tries: 0,
            announce: false,
            lock: None,
            #[cfg(target_os = "linux")]
            dnd: desktop_linux::Dnd::find(),
            dnd_state: None,
            children: Vec::new(),
            quit: false,
        };
        // Restart a timer that was running before logout, or end it.
        tray.spawn(&["toggle", "--resume-timer"]);
        tray.tick();
        Ok(tray)
    }

    /// Starts this program with `args`, without waiting; collected later
    /// (otherwise every finished command would stay behind as a zombie).
    fn spawn(&mut self, args: &[&str]) {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let mut cmd = Command::new(exe);
        cmd.args(args).stdin(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Detached from any terminal the tray was started from: a child
            // touching it would be suspended by the shell's job control.
            cmd.process_group(0);
        }
        match cmd.spawn() {
            Ok(child) => self.children.push(child),
            Err(e) => log(&format!("cannot run {args:?}: {e}")),
        }
    }

    fn reload(&mut self) {
        match Config::load(false) {
            Ok(c) => self.config = c,
            Err(e) => log(&e),
        }
        // The dock launcher and the notification icons.
        crate::desktop::reload(&self.config);
        self.dnd_state = None; // apply the do-not-disturb setting again
        self.build_menu();
        self.shown = None; // set the icon again
    }

    fn timer_line(&self, timer: &state::Timer) -> (String, i64) {
        let left = timer.minutes_left();
        let text = trf(
            "{status} until {time} ({left} left)",
            &[
                ("status", &self.config.label_of(&timer.status)),
                ("time", &state::clock(timer.end)),
                ("left", &human_minutes(left)),
            ],
        );
        (format!("⏱ {text}"), left)
    }

    fn build_menu(&mut self) {
        let menu = Menu::new();
        self.actions.clear();
        self.timer_item = None;
        let add = |tray: &mut Tray, label: &str, action: Option<Action>| -> MenuItem {
            let item = MenuItem::new(label, action.is_some(), None);
            if let Some(a) = action {
                tray.actions.push((item.id().clone(), a));
            }
            let _ = menu.append(&item);
            item
        };
        let separator = |menu: &Menu| {
            let _ = menu.append(&PredefinedMenuItem::separator());
        };
        let toggle =
            |args: &[&str]| Some(Action::Toggle(args.iter().map(|s| s.to_string()).collect()));

        // Light unplugged
        if self.connected == Some(false) {
            add(self, &format!("⚠ {}", tr("Light not connected")), None);
            separator(&menu);
        }
        // Running timer
        if let Some(timer) = self.timer.clone() {
            let (text, _) = self.timer_line(&timer);
            let item = add(self, &text, None);
            self.timer_item = Some(item);
            add(self, &tr("Cancel timer"), toggle(&["--cancel-timer"]));
            separator(&menu);
        }
        // Statuses, in one click
        for s in self.config.statuses.clone() {
            add(self, &s.label, toggle(&[&s.id]));
        }
        separator(&menu);
        // Timers in one click; "More…": any duration, and the settings
        for (id, minutes) in self.config.shortcuts.clone() {
            let label = trf(
                "{status} for {duration}",
                &[
                    ("status", &self.config.label_of(&id)),
                    ("duration", &human_minutes(minutes as i64)),
                ],
            );
            add(self, &label, toggle(&[&id, "--for", &minutes.to_string()]));
        }
        add(self, &tr("More…"), Some(Action::Panel));
        separator(&menu);
        // Off isn't a status: the light is just off
        add(self, &tr("Off"), toggle(&["off"]));
        separator(&menu);
        add(self, &tr("Quit"), Some(Action::Quit));
        self.icon.set_menu(Some(Box::new(menu)));
    }

    /// Runs a menu click.
    pub fn on_menu(&mut self, event: &MenuEvent) {
        let Some(action) = self
            .actions
            .iter()
            .find(|(id, _)| *id == event.id)
            .map(|(_, a)| a.clone())
        else {
            return;
        };
        match action {
            Action::Toggle(args) => {
                let mut all = vec!["toggle"];
                all.extend(args.iter().map(String::as_str));
                self.spawn(&all);
            }
            Action::Panel => self.spawn(&["panel"]),
            Action::Quit => self.quit = true,
        }
    }

    /// Every second.
    pub fn tick(&mut self) {
        let mtime = Config::mtime();
        if self.mtime != Some(mtime) {
            self.mtime = Some(mtime);
            self.reload();
        }
        self.update_light();
        let mut current = state::current();
        if current != "off" && !self.config.has(&current) {
            current = "off".into();
        }
        self.update_dnd(&current);
        let shown = (current.clone(), self.connected);
        if self.shown.as_ref() != Some(&shown) {
            let away = self.connected == Some(false);
            let color = if current == "off" {
                crate::color::OFF_COLOR.to_string()
            } else {
                self.config.color_of(&current)
            };
            let _ = self.icon.set_icon(Some(make_icon(&color, away)));
            let label = self.config.label_of(&current);
            let tip = if away {
                trf("{status} — light not connected", &[("status", &label)])
            } else {
                label
            };
            let _ = self.icon.set_tooltip(Some(tip));
            self.shown = Some(shown);
        }
        self.update_timer();
        self.children
            .retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    }

    // --- screen lock -------------------------------------------------------------

    /// Locked: lock-status (off by default). Unlocked: back to the status
    /// from before, unless it was changed meanwhile (a call, by hand…).
    pub fn on_lock(&mut self, locked: bool) {
        let lock_status = self.config.lock_status.clone();
        if lock_status == "none" {
            return;
        }
        if locked {
            let before = state::current();
            self.lock = Some((before.clone(), state::timer().map(|t| t.end)));
            if before != lock_status {
                // A locked screen doesn't cancel a timer.
                self.spawn(&["toggle", &lock_status, "--keep-timer"]);
            }
            return;
        }
        let Some((before, timer_end)) = self.lock.take() else {
            return;
        };
        if state::current() != lock_status || before == lock_status {
            return;
        }
        let mut back = before;
        if let Some(end) = timer_end {
            if state::timer().is_none() && state::now() >= end {
                back = self.config.default_status.clone(); // the timer ended while locked
            }
        }
        self.spawn(&["toggle", &back, "--keep-timer"]);
    }

    // --- do not disturb ------------------------------------------------------------

    /// Silences desktop notifications while the status is one of
    /// dnd-statuses; puts the user's own setting back afterwards.
    fn update_dnd(&mut self, current: &str) {
        #[cfg(target_os = "linux")]
        {
            let want = self.config.dnd_statuses.iter().any(|d| d == current);
            if Some(want) == self.dnd_state || !self.dnd.available() {
                return;
            }
            self.dnd_state = Some(want);
            let mark = paths::dnd_mark();
            if want && !mark.exists() {
                // Remember the user's own setting (survives a tray restart).
                let _ = std::fs::write(&mark, if self.dnd.get() { "1\n" } else { "0\n" });
                self.dnd.set(true);
            } else if !want && mark.exists() {
                let previous = std::fs::read_to_string(&mark)
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false);
                let _ = std::fs::remove_file(&mark);
                self.dnd.set(previous);
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = current;
    }

    /// On quit: never leave notifications silenced behind.
    pub fn restore_dnd(&mut self) {
        self.dnd_state = None;
        self.update_dnd("");
    }

    // --- light unplugged / plugged back in -------------------------------------------

    fn update_light(&mut self) {
        if light::connected() {
            self.missing_checks = 0;
            if self.connected != Some(true) {
                self.announce = self.connected == Some(false); // not at startup
                self.connected = Some(true);
                self.apply_tries = APPLY_TRIES;
                self.build_menu();
            }
        } else {
            self.missing_checks += 1;
            if self.connected.is_none()
                || (self.connected == Some(true) && self.missing_checks >= AWAY_AFTER)
            {
                let was_connected = self.connected == Some(true);
                self.connected = Some(false);
                self.apply_tries = 0;
                self.build_menu();
                if was_connected {
                    let status = self.config.label_of(&state::current());
                    notify(
                        &trf(
                            "Light disconnected — {status} will be shown again when it's plugged back in",
                            &[("status", &status)],
                        ),
                        "luxafor-status",
                        "Luxafor",
                    );
                }
            }
        }
        if self.apply_tries > 0 {
            self.apply_tries -= 1;
            let mut current = state::current();
            if current != "off" && !self.config.has(&current) {
                current = "off".into();
            }
            if toggle::light(&self.config, &current).is_ok() {
                self.apply_tries = 0;
                if self.announce {
                    let status = self.config.label_of(&current);
                    notify(
                        &trf("Light reconnected — {status}", &[("status", &status)]),
                        "luxafor-status",
                        "Luxafor",
                    );
                    self.announce = false;
                }
            } else if self.apply_tries == 0 {
                log("could not show the status on the light after it was plugged in");
            }
        }
    }

    // --- timer ---------------------------------------------------------------------------

    fn update_timer(&mut self) {
        let timer = state::timer();
        if timer != self.timer {
            // A timer started or stopped: the menu gains or loses its lines.
            self.timer = timer;
            self.timer_text = None;
            self.build_menu();
        }
        let Some(timer) = self.timer.clone() else {
            if self.timer_text.as_deref() != Some("") {
                self.icon.set_title(None::<&str>);
                self.timer_text = Some(String::new());
            }
            return;
        };
        let (text, left) = self.timer_line(&timer);
        if self.timer_text.as_ref() != Some(&text) {
            // Updated in place: rebuilding would close an open menu.
            if let Some(item) = &self.timer_item {
                item.set_text(&text);
            }
            self.icon.set_title(Some(human_minutes(left)));
            self.timer_text = Some(text);
        }
    }
}

fn make_icon(color_text: &str, away: bool) -> Icon {
    let rgb = color::display(color_text);
    Icon::from_rgba(icons::status_icon(rgb, away), icons::SIZE, icons::SIZE).expect("valid icon")
}

/// Starts the call detection in this process, unless it runs elsewhere.
fn start_call_detection() {
    let Some(lock) = crate::calls::take_lock() else {
        log("call detection already running in another process");
        return;
    };
    std::thread::spawn(move || {
        let _lock = lock; // held as long as the watcher runs
        crate::calls::Watcher::default().run();
    });
}

#[cfg(target_os = "linux")]
pub fn main(_args: Vec<String>) -> i32 {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::time::Duration;

    use gtk::{gio, glib};

    if let Err(e) = gtk::init() {
        eprintln!("luxafor-tray: {e}");
        return 1;
    }
    // One tray per session: take the D-Bus name over (v1's tray too).
    // 1 = ours, 2 = lost.
    static NAME: AtomicU8 = AtomicU8::new(0);
    let _owner = gio::bus_own_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameOwnerFlags::ALLOW_REPLACEMENT | gio::BusNameOwnerFlags::REPLACE,
        |_, _| {},
        |_, _| NAME.store(1, Ordering::SeqCst),
        |_, _| NAME.store(2, Ordering::SeqCst),
    );
    let ctx = glib::MainContext::default();
    let start = std::time::Instant::now();
    while NAME.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(5) {
        ctx.iteration(true);
    }
    if NAME.load(Ordering::SeqCst) != 1 {
        log("cannot own the tray bus name — starting anyway");
        NAME.store(1, Ordering::SeqCst);
    }

    let tray = match Tray::new() {
        Ok(t) => Rc::new(RefCell::new(t)),
        Err(e) => {
            log(&format!("cannot start: {e}"));
            return 1;
        }
    };
    // After an upgrade from v1, its call detection may still be running in
    // this session: stop it, the tray does it now.
    let _ = Command::new("pkill")
        .args([
            "-u",
            &std::env::var("USER").unwrap_or_default(),
            "-f",
            "^[^ ]*bash [^ ]*luxafor-callwatch$",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    start_call_detection();
    {
        let tray = tray.clone();
        desktop_linux::watch_screen_lock(move |locked| tray.borrow_mut().on_lock(locked));
    }
    // Menu clicks.
    {
        let tray = tray.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                tray.borrow_mut().on_menu(&event);
            }
            if tray.borrow().quit {
                gtk::main_quit();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
    // Every second. A panic in an update must not stop the updates.
    {
        let tray = tray.clone();
        glib::timeout_add_local(Duration::from_secs(1), move || {
            if NAME.load(Ordering::SeqCst) == 2 {
                log("another luxafor-tray took over, quitting");
                gtk::main_quit();
                return glib::ControlFlow::Break;
            }
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tray.borrow_mut().tick()));
            if result.is_err() {
                log("error while updating (see above)");
            }
            glib::ControlFlow::Continue
        });
    }
    gtk::main();
    tray.borrow_mut().restore_dnd();
    0
}

/// The tray lock: one tray per user (Windows; Linux uses the D-Bus name).
/// None: a tray is running.
#[cfg(not(target_os = "linux"))]
pub fn take_lock() -> Option<std::fs::File> {
    let path = paths::lock("tray");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

#[cfg(windows)]
mod win {
    #[repr(C)]
    pub struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: [i32; 2],
        _private: [u8; 16],
    }

    #[link(name = "user32")]
    extern "system" {
        fn PeekMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> isize;
        fn SwitchDesktop(desktop: isize) -> i32;
        fn CloseDesktop(desktop: isize) -> i32;
    }

    /// Handles the window messages of this thread (tray icon, menu).
    pub fn pump() {
        const PM_REMOVE: u32 = 1;
        let mut msg = std::mem::MaybeUninit::<Msg>::zeroed();
        unsafe {
            while PeekMessageW(msg.as_mut_ptr(), 0, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(msg.as_ptr());
                DispatchMessageW(msg.as_ptr());
            }
        }
    }

    /// True while the screen is locked: the input desktop can't be
    /// switched to then.
    pub fn screen_locked() -> bool {
        const DESKTOP_SWITCHDESKTOP: u32 = 0x0100;
        unsafe {
            let desktop = OpenInputDesktop(0, 0, DESKTOP_SWITCHDESKTOP);
            if desktop == 0 {
                return true;
            }
            let switched = SwitchDesktop(desktop) != 0;
            CloseDesktop(desktop);
            !switched
        }
    }

    /// Starts the tray at login (once: the user can turn it off in the
    /// Task Manager's startup apps).
    pub fn ensure_autostart() {
        use winreg::enums::HKEY_CURRENT_USER;
        let mark = crate::paths::cache_home().join("autostart-set");
        if mark.exists() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        if let Ok((key, _)) = winreg::RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        {
            if key
                .set_value("Luxafor Status", &format!("\"{}\" tray", exe.display()))
                .is_ok()
            {
                let _ = std::fs::create_dir_all(crate::paths::cache_home());
                let _ = std::fs::write(mark, "");
            }
        }
    }
}

#[cfg(windows)]
pub fn main(_args: Vec<String>) -> i32 {
    use std::time::{Duration, Instant};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

    let Some(_lock) = take_lock() else {
        eprintln!("luxafor-tray: already running");
        return 0;
    };
    let mut tray = match Tray::new() {
        Ok(t) => t,
        Err(e) => {
            log(&format!("cannot start: {e}"));
            return 1;
        }
    };
    start_call_detection();
    win::ensure_autostart();
    let mut last_tick = Instant::now();
    let mut locked = false;
    loop {
        win::pump();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            tray.on_menu(&event);
        }
        // Left click: the panel (right click: the menu).
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                tray.spawn(&["panel"]);
            }
        }
        if tray.quit {
            break;
        }
        if last_tick.elapsed() >= Duration::from_secs(1) {
            last_tick = Instant::now();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tray.tick())).is_err() {
                log("error while updating (see above)");
            }
            let now_locked = win::screen_locked();
            if now_locked != locked {
                locked = now_locked;
                tray.on_lock(locked);
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    0
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn main(_args: Vec<String>) -> i32 {
    eprintln!("luxafor-tray: not available on this system yet");
    1
}
