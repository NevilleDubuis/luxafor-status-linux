//! luxafor-probe — checks that the v2 building blocks work on this computer.
//!
//! v2 is one cross-platform app (Linux, Windows) written in Rust. Before the
//! rewrite, this probe checks on real machines that:
//!
//! 1. the light can be driven through hidapi;
//! 2. the tray icon shows up, and changes color with the status;
//! 3. the tray menu reaches the app, and can bring the window back;
//! 4. (Windows) a call is detected: which apps are using the microphone.
//!
//! Click "Copy report" at the end and paste the report in issue #11.

// No console window on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

const VENDOR: u16 = 0x04D8;
const PRODUCT: u16 = 0xF372;

#[derive(Clone, Copy)]
struct Status {
    label: &'static str,
    rgb: Option<[u8; 3]>, // None = off
    blink: bool,
}

const STATUSES: [Status; 5] = [
    Status { label: "Available", rgb: Some([0, 255, 0]), blink: false },
    Status { label: "Busy", rgb: Some([255, 0, 0]), blink: false },
    Status { label: "On a call", rgb: Some([255, 255, 0]), blink: false },
    Status { label: "Busy, blinking", rgb: Some([255, 0, 0]), blink: true },
    Status { label: "Off", rgb: None, blink: false },
];

// --- log ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn add(&self, key: &str, value: impl std::fmt::Display) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!(
            "{:02}:{:02}:{:02}  {key}: {value}",
            secs / 3600 % 24, secs / 60 % 60, secs % 60
        );
        println!("{line}");
        self.0.lock().unwrap().push(line);
    }

    fn text(&self) -> String {
        self.0.lock().unwrap().join("\n")
    }
}

// --- light -------------------------------------------------------------------------

fn find_lights() -> Result<Vec<String>, String> {
    let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
    Ok(api
        .device_list()
        .filter(|d| d.vendor_id() == VENDOR && d.product_id() == PRODUCT)
        .map(|d| d.product_string().unwrap_or("Luxafor").to_string())
        .collect())
}

/// The 8-byte Luxafor command, preceded by the report ID (0).
fn report(status: &Status) -> [u8; 9] {
    let [r, g, b] = status.rgb.unwrap_or([0, 0, 0]);
    if status.blink {
        [0, 3, 0xFF, r, g, b, 90, 0, 0] // strobe, repeat forever
    } else {
        [0, 1, 0xFF, r, g, b, 0, 0, 0] // static color
    }
}

fn send(status: &Status) -> String {
    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => return format!("hidapi error: {e}"),
    };
    let lights: Vec<_> = api
        .device_list()
        .filter(|d| d.vendor_id() == VENDOR && d.product_id() == PRODUCT)
        .collect();
    if lights.is_empty() {
        return "no Luxafor light found — is it plugged in?".into();
    }
    let results: Vec<String> = lights
        .iter()
        .map(|info| match info.open_device(&api) {
            Ok(dev) => match dev.write(&report(status)) {
                Ok(n) => format!("{n} bytes written"),
                Err(e) => format!("write failed: {e}"),
            },
            Err(e) => {
                let hint = if cfg!(target_os = "linux") {
                    " (is luxafor-status v1 installed? its udev rule gives access)"
                } else {
                    ""
                };
                format!("cannot open the light: {e}{hint}")
            }
        })
        .collect();
    results.join("; ")
}

// --- microphone (Windows) ----------------------------------------------------------

/// Apps using the microphone right now: Windows records it under
/// CapabilityAccessManager — an app whose LastUsedTimeStop is 0 uses it.
#[cfg(windows)]
fn microphone_users() -> Vec<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    fn scan(key: &RegKey, users: &mut Vec<String>) {
        for name in key.enum_keys().flatten() {
            let Ok(app) = key.open_subkey(&name) else { continue };
            if name == "NonPackaged" {
                scan(&app, users);
                continue;
            }
            let stop: u64 = app.get_value("LastUsedTimeStop").unwrap_or(1);
            let start: u64 = app.get_value("LastUsedTimeStart").unwrap_or(0);
            if stop == 0 && start != 0 {
                users.push(name.replace('#', "\\"));
            }
        }
    }

    let mut users = Vec::new();
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(
        r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone",
    ) {
        scan(&key, &mut users);
    }
    users
}

#[cfg(windows)]
fn watch_microphone(log: Log) {
    std::thread::spawn(move || {
        let mut last: Option<Vec<String>> = None;
        loop {
            let users = microphone_users();
            if last.as_ref() != Some(&users) {
                let text = if users.is_empty() { "nobody".to_string() } else { users.join(", ") };
                log.add("microphone in use by", text);
                last = Some(users);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

// --- tray --------------------------------------------------------------------------

fn circle(rgb: Option<[u8; 3]>, size: u32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    let r = c - 1.5;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            let i = ((y * size + x) * 4) as usize;
            let (fill, alpha) = match rgb {
                Some(rgb) => (rgb, (r - d + 0.5).clamp(0.0, 1.0)),
                // off: a gray ring
                None => ([136, 136, 136], (1.0 - ((d - (r - 3.0)).abs() - 2.0)).clamp(0.0, 1.0)),
            };
            px[i..i + 3].copy_from_slice(&fill);
            px[i + 3] = (alpha * 255.0) as u8;
        }
    }
    px
}

fn icon(rgb: Option<[u8; 3]>) -> Icon {
    Icon::from_rgba(circle(rgb, 64), 64, 64).expect("valid icon")
}

/// Menu item ids → what they do.
#[derive(Clone)]
struct MenuIds {
    statuses: Vec<(tray_icon::menu::MenuId, usize)>,
    show: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

/// Builds the tray icons. On Linux, a second one keeps its pictures in
/// ~/.local/share/icons: some tray hosts (Budgie) ignore the folder the
/// first one uses — the report tells which one works.
fn build_trays(log: &Log) -> (Vec<TrayIcon>, MenuIds) {
    let menu = Menu::new();
    let mut statuses = Vec::new();
    for (n, s) in STATUSES.iter().enumerate() {
        let item = MenuItem::new(s.label, true, None);
        statuses.push((item.id().clone(), n));
        menu.append(&item).unwrap();
    }
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    let show = MenuItem::new("Show the probe window", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append(&show).unwrap();
    menu.append(&quit).unwrap();
    let ids = MenuIds { statuses, show: show.id().clone(), quit: quit.id().clone() };

    let mut trays = Vec::new();
    let mut builders = vec![("A", None)];
    if cfg!(target_os = "linux") {
        let home = std::env::var("HOME").unwrap_or_default();
        builders.push(("B", Some(format!("{home}/.local/share/icons"))));
    }
    for (name, dir) in builders {
        let mut builder = TrayIconBuilder::new()
            .with_id(format!("luxafor-probe-{}", name.to_lowercase()))
            .with_menu(Box::new(menu.clone()))
            .with_tooltip(format!("Luxafor probe — icon {name}"))
            .with_icon(icon(None));
        if let Some(dir) = &dir {
            builder = builder.with_temp_dir_path(dir);
        }
        match builder.build() {
            Ok(tray) => {
                log.add(&format!("tray icon {name}"), match &dir {
                    Some(d) => format!("created (pictures in {d})"),
                    None => "created".to_string(),
                });
                trays.push(tray);
            }
            Err(e) => log.add(&format!("tray icon {name}"), format!("FAILED: {e}")),
        }
    }
    (trays, ids)
}

fn set_tray_icons(trays: &[TrayIcon], status: &Status) {
    for tray in trays {
        let _ = tray.set_icon(Some(icon(status.rgb)));
        let _ = tray.set_tooltip(Some(format!("Luxafor probe — {}", status.label)));
    }
}

/// On Linux the tray lives in a GTK thread; the app talks to it with this.
#[cfg(target_os = "linux")]
type TrayHandle = std::sync::mpsc::Sender<usize>;
#[cfg(not(target_os = "linux"))]
type TrayHandle = Vec<TrayIcon>;

#[cfg(target_os = "linux")]
fn start_tray(log: &Log) -> (TrayHandle, MenuIds) {
    let (ids_tx, ids_rx) = std::sync::mpsc::channel();
    let (tx, rx) = std::sync::mpsc::channel::<usize>();
    let log = log.clone();
    std::thread::spawn(move || {
        if let Err(e) = gtk::init() {
            log.add("gtk", format!("FAILED: {e}"));
            return;
        }
        let (trays, ids) = build_trays(&log);
        ids_tx.send(ids).unwrap();
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            while let Ok(n) = rx.try_recv() {
                set_tray_icons(&trays, &STATUSES[n]);
            }
            gtk::glib::ControlFlow::Continue
        });
        gtk::main();
    });
    let ids = ids_rx.recv().expect("tray thread");
    (tx, ids)
}

#[cfg(not(target_os = "linux"))]
fn start_tray(log: &Log) -> (TrayHandle, MenuIds) {
    build_trays(log)
}

// --- app ---------------------------------------------------------------------------

/// Quits, removing the tray pictures left in ~/.local/share/icons (Linux).
fn quit() -> ! {
    if let Ok(home) = std::env::var("HOME") {
        if let Ok(entries) = std::fs::read_dir(format!("{home}/.local/share/icons")) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("tray-icon-luxafor-probe-") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    std::process::exit(0)
}

struct Probe {
    log: Log,
    tray: TrayHandle,
    pending: Arc<Mutex<Vec<usize>>>,
}

impl Probe {
    fn set_status(&mut self, n: usize) {
        apply(&self.log, n);
        #[cfg(target_os = "linux")]
        let _ = self.tray.send(n);
        #[cfg(not(target_os = "linux"))]
        set_tray_icons(&self.tray, &STATUSES[n]);
    }
}

fn apply(log: &Log, n: usize) {
    let status = &STATUSES[n];
    log.add(&format!("status '{}'", status.label), send(status));
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Statuses picked in the tray menu (the tray icon follows here).
        let picked: Vec<usize> = std::mem::take(&mut *self.pending.lock().unwrap());
        for n in picked {
            #[cfg(target_os = "linux")]
            let _ = self.tray.send(n);
            #[cfg(not(target_os = "linux"))]
            set_tray_icons(&self.tray, &STATUSES[n]);
        }
        // Closing the window keeps the app in the tray.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.log.add("window", "closed → hidden, the app stays in the tray");
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Luxafor probe (v2)");
            ui.label("1. Click the statuses: the light AND the tray icon should change.");
            ui.label("2. Try the tray menu (also \"Show the probe window\" after closing this one).");
            ui.label("3. Windows: join a Teams call — is it detected?");
            ui.label("4. Copy the report and paste it in issue #11, with what you saw.");
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (n, s) in STATUSES.iter().enumerate() {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    match s.rgb {
                        Some([r, g, b]) => {
                            ui.painter().circle_filled(rect.center(), 6.0, egui::Color32::from_rgb(r, g, b));
                        }
                        None => {
                            ui.painter().circle_stroke(rect.center(), 5.0, (2.0, egui::Color32::GRAY));
                        }
                    }
                    if ui.button(s.label).clicked() {
                        self.set_status(n);
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Copy report").clicked() {
                    ctx.copy_text(self.log.text());
                    let path = std::env::temp_dir().join("luxafor-probe-report.txt");
                    let saved = std::fs::write(&path, self.log.text()).is_ok();
                    self.log.add("report", if saved {
                        format!("copied to the clipboard, and saved to {}", path.display())
                    } else {
                        "copied to the clipboard".to_string()
                    });
                }
                if ui.button("Quit").clicked() {
                    quit();
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(self.log.text()).monospace()).wrap());
            });
        });
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn main() -> eframe::Result {
    let log = Log::default();
    log.add("system", format!("{} {} (times in UTC)", std::env::consts::OS, std::env::consts::ARCH));
    log.add("probe", env!("CARGO_PKG_VERSION"));
    if cfg!(target_os = "linux") {
        for var in ["XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE"] {
            log.add(var, std::env::var(var).unwrap_or_else(|_| "(not set)".into()));
        }
    }
    match find_lights() {
        Ok(l) if l.is_empty() => log.add("lights", "none found"),
        Ok(l) => log.add("lights", l.join(", ")),
        Err(e) => log.add("lights", format!("hidapi error: {e}")),
    }
    #[cfg(windows)]
    watch_microphone(log.clone());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Luxafor probe (v2)")
            .with_inner_size([620.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "luxafor-probe",
        options,
        Box::new(move |cc| {
            // The tray is created once the event loop exists (Windows needs it
            // on this thread).
            let (tray, ids) = start_tray(&log);
            let pending = Arc::new(Mutex::new(Vec::new()));
            // Menu clicks are handled right away, even while the window is
            // hidden (it isn't redrawn then).
            let ctx = cc.egui_ctx.clone();
            let menu_log = log.clone();
            let menu_pending = pending.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if let Some((_, n)) = ids.statuses.iter().find(|(id, _)| *id == event.id) {
                    apply(&menu_log, *n);
                    menu_pending.lock().unwrap().push(*n);
                } else if event.id == ids.show {
                    menu_log.add("tray menu", "show the window");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                } else if event.id == ids.quit {
                    quit();
                }
                ctx.request_repaint();
            }));
            let click_log = log.clone();
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                // Not sent on Linux (AppIndicator only has a menu).
                if let TrayIconEvent::Click { button, button_state, .. } = event {
                    if button_state == tray_icon::MouseButtonState::Up {
                        click_log.add("tray click", format!("{button:?}"));
                    }
                }
            }));
            Ok(Box::new(Probe { log, tray, pending }))
        }),
    )
}
