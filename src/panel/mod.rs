//! luxafor-panel: a small window to pick a status, optionally for a while.
//!
//! Status tab: choose a duration (∞ = no timer), then click a status: it is
//! applied and the panel closes. Keyboard: 1–9 pick a status, Esc closes.
//!
//! Settings tab: statuses (color, name, blinking, order), the status after
//! a timer, during calls and while the screen is locked, brightness, do
//! not disturb, the timers of the tray menu, and the theme.
//!
//! Statuses are set through `luxafor-status toggle`, like the tray and the
//! command line, so they never disagree.

mod model;

use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::color::{self, Rgb};
use crate::config::{self, Config, Effect};
use crate::i18n::{tr, trf};
use crate::state::{self, human_minutes};
use crate::toggle::spawn_detached;
use model::{Model, MAX_SHORTCUTS};

/// (minutes, label) of the duration buttons; None = no timer.
const DURATIONS: [Option<u32>; 5] = [None, Some(15), Some(30), Some(60), Some(120)];
const CUSTOM_DEFAULT: u32 = 45;
const WIDTH: f32 = 440.0;
/// D-Bus name of the panel (Linux): a second panel replaces the first.
#[cfg(target_os = "linux")]
const BUS_NAME: &str = "io.github.nevilledubuis.luxafor_status.panel";

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Status,
    Settings,
}

#[derive(PartialEq, Clone, Copy)]
enum Pick {
    Preset(Option<u32>),
    Custom,
}

fn effect_label(e: Effect) -> String {
    match e {
        Effect::Steady => tr("Steady"),
        Effect::BlinkSlow => tr("Blink slowly"),
        Effect::Blink => tr("Blink"),
        Effect::BlinkFast => tr("Blink fast"),
    }
}

fn theme_choices() -> [(&'static str, String); 3] {
    [
        ("auto", tr("Follow the desktop")),
        ("light", tr("Light")),
        ("dark", tr("Dark")),
    ]
}

/// Is the desktop dark? (GNOME / Budgie "Style: Dark", or a dark GTK theme.)
fn desktop_is_dark() -> bool {
    #[cfg(target_os = "linux")]
    {
        use gtk::gio;
        use gtk::prelude::*;
        if let Some(source) = gio::SettingsSchemaSource::default() {
            if let Some(schema) = source.lookup("org.gnome.desktop.interface", true) {
                let s = gio::Settings::new("org.gnome.desktop.interface");
                if schema.has_key("color-scheme") && s.string("color-scheme") == "prefer-dark" {
                    return true;
                }
                if schema.has_key("gtk-theme")
                    && s.string("gtk-theme").to_lowercase().contains("dark")
                {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        winreg::RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme"))
            .map(|v| v == 0)
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    false
}

fn apply_theme(ctx: &egui::Context, theme: &str) {
    let dark = match theme {
        "dark" => true,
        "light" => false,
        _ => desktop_is_dark(),
    };
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

/// A dot of a status color; it blinks like the light for a blinking status.
fn dot(ui: &mut egui::Ui, rgb: Rgb, effect: Effect, radius: f32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(radius * 2.0 + 2.0, radius * 2.0 + 2.0),
        egui::Sense::hover(),
    );
    let period = match effect {
        Effect::Steady => 0.0,
        Effect::BlinkSlow => 1.6,
        Effect::Blink => 0.9,
        Effect::BlinkFast => 0.4,
    };
    let mut alpha = 255u8;
    if period > 0.0 {
        let t = ui.input(|i| i.time);
        if (t % period) / period >= 0.5 {
            alpha = 40;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    let [r, g, b] = rgb;
    ui.painter().circle_filled(
        rect.center(),
        radius,
        Color32::from_rgba_unmultiplied(r, g, b, alpha),
    );
}

#[derive(Clone, Copy)]
enum Glyph {
    Up,
    Down,
    Delete,
}

/// A small flat button with a drawn symbol (no font has them all).
fn glyph_button(ui: &mut egui::Ui, glyph: Glyph, enabled: bool, tip: &str) -> bool {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), sense);
    let visuals = ui.style().interact(&response);
    let color = if enabled {
        visuals.fg_stroke.color
    } else {
        ui.visuals().weak_text_color().gamma_multiply(0.5)
    };
    if response.hovered() && enabled {
        ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
    }
    let c = rect.center();
    let stroke = egui::Stroke::new(1.6, color);
    let p = |x: f32, y: f32| c + egui::vec2(x, y);
    match glyph {
        Glyph::Up => {
            ui.painter()
                .line(vec![p(-5.0, 3.0), p(0.0, -3.0), p(5.0, 3.0)], stroke);
        }
        Glyph::Down => {
            ui.painter()
                .line(vec![p(-5.0, -3.0), p(0.0, 3.0), p(5.0, -3.0)], stroke);
        }
        Glyph::Delete => {
            ui.painter()
                .line_segment([p(-4.5, -4.5), p(4.5, 4.5)], stroke);
            ui.painter()
                .line_segment([p(-4.5, 4.5), p(4.5, -4.5)], stroke);
        }
    }
    let clicked = response.clicked();
    response.on_hover_text(tip);
    enabled && clicked
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).size(11.0).strong().weak());
}

struct Panel {
    tab: Tab,
    config: Config,
    current: String,
    pick: Pick,
    custom: u32,
    model: Model,
    dirty: bool,
    message: String,
    /// The theme in use (previewed from the Settings tab before saving).
    theme: String,
    close: bool,
    /// Window height, fitted to the content.
    height: f32,
}

impl Panel {
    fn new(ctx: &egui::Context, config: Config) -> Panel {
        let theme = config.theme.clone();
        apply_theme(ctx, &theme);
        Panel {
            tab: Tab::Status,
            model: Model::from_config(&config),
            current: state::current(),
            config,
            pick: Pick::Preset(None),
            custom: CUSTOM_DEFAULT,
            dirty: false,
            message: String::new(),
            theme,
            close: false,
            height: 0.0,
        }
    }

    fn reload(&mut self) {
        if let Ok(c) = Config::load(false) {
            self.config = c;
        }
        self.model = Model::from_config(&self.config);
        self.current = state::current();
        self.dirty = false;
    }

    fn minutes(&self) -> Option<u32> {
        match self.pick {
            Pick::Preset(m) => m,
            Pick::Custom => Some(self.custom),
        }
    }

    fn set_status(&mut self, id: &str) {
        let mut args = vec!["toggle".to_string(), id.to_string()];
        if let Some(m) = self.minutes() {
            args.extend(["--for".to_string(), m.to_string()]);
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        spawn_detached(&args);
        self.close = true;
    }

    // --- Status tab -----------------------------------------------------------------

    fn status_tab(&mut self, ui: &mut egui::Ui) {
        // Current status and timer
        ui.horizontal(|ui| {
            let effect = self
                .config
                .get(&self.current)
                .map(|s| s.effect)
                .unwrap_or_default();
            dot(
                ui,
                color::display(&self.config.color_of(&self.current)),
                effect,
                11.0,
            );
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.config.label_of(&self.current))
                        .strong()
                        .size(17.0),
                );
                match state::timer() {
                    Some(t) => {
                        let who = if t.status == self.current {
                            String::new()
                        } else {
                            format!("{} ", self.config.label_of(&t.status))
                        };
                        let text = trf(
                            "until {time} · {left} left, then {status}",
                            &[
                                ("time", &state::clock(t.end)),
                                ("left", &human_minutes(t.minutes_left())),
                                ("status", &self.config.label_of(&self.config.default_status)),
                            ],
                        );
                        ui.label(RichText::new(format!("{who}{text}")).weak());
                    }
                    None => {
                        ui.label(RichText::new(tr("No timer")).weak());
                    }
                }
            });
            if state::timer().is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Cancel timer")).clicked() {
                        spawn_detached(&["toggle", "--cancel-timer"]);
                        self.close = true;
                    }
                });
            }
        });
        ui.separator();

        // Duration
        heading(ui, &tr("1 · DURATION"));
        ui.horizontal(|ui| {
            for m in DURATIONS {
                let text = m
                    .map(|m| human_minutes(m as i64))
                    .unwrap_or_else(|| "∞".into());
                if ui
                    .selectable_label(self.pick == Pick::Preset(m), text)
                    .clicked()
                {
                    self.pick = Pick::Preset(m);
                }
            }
            let custom = ui.add(
                egui::DragValue::new(&mut self.custom)
                    .range(1..=1440)
                    .speed(1.0),
            );
            let label = ui.selectable_label(self.pick == Pick::Custom, tr("min"));
            if custom.changed() || custom.gained_focus() || label.clicked() {
                self.pick = Pick::Custom;
            }
            custom.on_hover_text(tr("Custom duration"));
        });
        ui.add_space(6.0);

        // Statuses
        heading(ui, &tr("2 · STATUS"));
        let mut entries: Vec<(String, Rgb, Effect, String)> = self
            .config
            .statuses
            .iter()
            .map(|s| {
                (
                    s.id.clone(),
                    color::display(&s.color),
                    s.effect,
                    s.label.clone(),
                )
            })
            .collect();
        entries.push((
            "off".into(),
            color::display(color::OFF_COLOR),
            Effect::Steady,
            tr("Off"),
        ));
        let mut clicked = None;
        let button_width = (ui.available_width() - 8.0) / 2.0;
        egui::Grid::new("statuses")
            .num_columns(2)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for (n, (id, rgb, effect, label)) in entries.iter().enumerate() {
                    let (rect, response) = ui
                        .allocate_exact_size(egui::vec2(button_width, 36.0), egui::Sense::click());
                    let visuals = ui
                        .style()
                        .interact_selectable(&response, *id == self.current);
                    ui.painter().rect(
                        rect,
                        6.0,
                        visuals.weak_bg_fill,
                        visuals.bg_stroke,
                        egui::StrokeKind::Inside,
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(egui::vec2(10.0, 0.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    dot(&mut child, *rgb, *effect, 7.0);
                    let name = RichText::new(label);
                    child.add(
                        egui::Label::new(if *id == self.current {
                            name.strong()
                        } else {
                            name
                        })
                        .truncate()
                        .selectable(false),
                    );
                    if n < 9 {
                        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new((n + 1).to_string()).small().weak());
                        });
                    }
                    if response
                        .on_hover_text(format!("luxafor-toggle {id}"))
                        .clicked()
                    {
                        clicked = Some(id.clone());
                    }
                    if n % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        // Keys 1–9 (not while typing a duration)
        if !ui.ctx().egui_wants_keyboard_input() {
            const KEYS: [egui::Key; 9] = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ];
            for (n, key) in KEYS.iter().enumerate() {
                if n < self.config.statuses.len() && ui.input(|i| i.key_pressed(*key)) {
                    clicked = Some(self.config.statuses[n].id.clone());
                }
            }
        }
        if let Some(id) = clicked {
            self.set_status(&id);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr("Pick a duration, then a status · Esc to close"))
                .small()
                .weak(),
        );
    }

    // --- Settings tab ---------------------------------------------------------------

    fn combo(
        ui: &mut egui::Ui,
        id: &str,
        value: &mut String,
        choices: &[(String, String)],
    ) -> bool {
        let selected = choices
            .iter()
            .find(|(k, _)| k == value)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let mut changed = false;
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (key, label) in choices {
                    changed |= ui.selectable_value(value, key.clone(), label).changed();
                }
            });
        changed
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let m = &mut self.model;
        heading(ui, &tr("STATUSES"));
        let mut action: Option<(usize, isize)> = None; // (row, -1 up / +1 down / 0 delete)
        let count = m.rows.len();
        for (i, row) in m.rows.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                changed |= ui.color_edit_button_srgb(&mut row.color).changed();
                let mut edit = egui::TextEdit::singleline(&mut row.label)
                    .desired_width((ui.available_width() - 222.0).max(90.0));
                if row.error {
                    edit = edit.background_color(Color32::from_rgb(0x80, 0x20, 0x20));
                }
                changed |= ui.add(edit).changed();
                egui::ComboBox::from_id_salt(("effect", &row.key))
                    .selected_text(effect_label(row.effect))
                    .width(124.0)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for e in Effect::ALL {
                            changed |= ui
                                .selectable_value(&mut row.effect, e, effect_label(e))
                                .changed();
                        }
                    })
                    .response
                    .on_hover_text(tr("How the light shows this status"));
                if glyph_button(ui, Glyph::Up, i > 0, &tr("Move up")) {
                    action = Some((i, -1));
                }
                if glyph_button(ui, Glyph::Down, i + 1 < count, &tr("Move down")) {
                    action = Some((i, 1));
                }
                if glyph_button(ui, Glyph::Delete, true, &tr("Delete")) {
                    action = Some((i, 0));
                }
            });
        }
        match action {
            Some((i, 0)) => match m.remove_row(i) {
                Ok(()) => changed = true,
                Err(e) => self.message = e,
            },
            Some((i, step)) => {
                m.move_row(i, step);
                changed = true;
            }
            None => {}
        }
        if ui.button(tr("Add status")).clicked() {
            m.add_status();
            changed = true;
        }
        m.fix_choices();

        heading(ui, &tr("BEHAVIOR"));
        egui::Grid::new("behavior").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, text: String, tip: String| {
                ui.label(text).on_hover_text(tip);
            };
            row(ui, tr("After a timer"), tr("Status to go back to when a timer ends"));
            let choices = m.default_choices();
            changed |= Self::combo(ui, "default", &mut m.default_key, &choices);
            ui.end_row();
            row(ui, tr("During calls"), tr("Status used automatically while you are in a call"));
            let choices = m.call_choices();
            changed |= Self::combo(ui, "call", &mut m.call_key, &choices);
            ui.end_row();
            row(ui, tr("Screen locked"), tr("Status while the screen is locked; the previous one comes back when you unlock"));
            let choices = m.lock_choices();
            changed |= Self::combo(ui, "lock", &mut m.lock_key, &choices);
            ui.end_row();
            row(ui, tr("Brightness"), tr("Brightness of the light"));
            changed |= ui.add(egui::Slider::new(&mut m.brightness, 5..=100).step_by(5.0).suffix(" %")).changed();
            ui.end_row();
            row(ui, tr("Theme"), tr("Look of this panel"));
            let themes: Vec<(String, String)> = theme_choices().iter().map(|(k, l)| (k.to_string(), l.clone())).collect();
            if Self::combo(ui, "theme", &mut m.theme, &themes) {
                changed = true;
                self.theme = m.theme.clone();
                apply_theme(ui.ctx(), &self.theme);
            }
            ui.end_row();
        });

        heading(ui, &tr("DO NOT DISTURB"));
        ui.label(tr("Silence desktop notifications while:"))
            .on_hover_text(tr(
                "Optional: you may still want to see incoming call notifications",
            ));
        let choices = m.choices();
        egui::Grid::new("dnd").num_columns(2).show(ui, |ui| {
            for (n, (key, label)) in choices.iter().enumerate() {
                let mut on = m.dnd.contains(key);
                if ui.checkbox(&mut on, label).changed() {
                    changed = true;
                    if on {
                        m.dnd.push(key.clone());
                    } else {
                        m.dnd.retain(|k| k != key);
                    }
                }
                if n % 2 == 1 {
                    ui.end_row();
                }
            }
        });

        heading(ui, &tr("TIMERS IN THE TRAY MENU"));
        let mut remove = None;
        for (i, (key, minutes)) in m.shortcuts.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.scope(|ui| {
                    ui.set_width(170.0);
                    changed |= Self::combo(ui, &format!("shortcut{i}"), key, &choices);
                });
                ui.label(tr("for"));
                changed |= ui
                    .add(egui::DragValue::new(minutes).range(1..=1440).speed(1.0))
                    .changed();
                ui.label(tr("min"));
                if glyph_button(ui, Glyph::Delete, true, &tr("Delete")) {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            m.shortcuts.remove(i);
            changed = true;
        }
        if ui
            .add_enabled(
                m.shortcuts.len() < MAX_SHORTCUTS,
                egui::Button::new(tr("Add timer")),
            )
            .clicked()
        {
            m.add_shortcut();
            changed = true;
        }
        if changed {
            self.dirty = true;
            if !matches!(action, Some((_, 0))) || self.message != tr("Keep at least one status.") {
                self.message.clear();
            }
        }
    }

    fn settings_footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .link(tr("Edit the file…"))
                .on_hover_text(crate::paths::conf().display().to_string())
                .clicked()
            {
                spawn_detached(&["toggle", "--edit", "--gui"]);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(self.dirty, egui::Button::new(tr("Save")))
                    .clicked()
                {
                    self.save(ui.ctx());
                }
                if ui
                    .add_enabled(self.dirty, egui::Button::new(tr("Revert")))
                    .clicked()
                {
                    self.reload();
                    self.theme = self.config.theme.clone();
                    apply_theme(ui.ctx(), &self.theme);
                    self.message.clear();
                }
                ui.label(RichText::new(&self.message).small().weak());
            });
        });
    }

    fn save(&mut self, ctx: &egui::Context) -> bool {
        let settings = match self.model.collect() {
            Ok(s) => s,
            Err(e) => {
                self.message = e;
                return false;
            }
        };
        if let Err(e) = config::write(&settings, &crate::paths::conf()) {
            self.message = trf("Could not save: {error}", &[("error", &e)]);
            return false;
        }
        // Icons and launcher actions; the new color or brightness on the
        // light. (The tray picks the file up by itself.)
        if let Ok(exe) = std::env::current_exe() {
            for args in [["toggle", "--reload"], ["toggle", "--apply"]] {
                let _ = std::process::Command::new(&exe)
                    .args(args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
        self.reload();
        self.theme = self.config.theme.clone();
        apply_theme(ctx, &self.theme);
        self.message = tr("Saved.");
        true
    }
}

impl eframe::App for Panel {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "linux")]
        {
            // D-Bus: another panel took over.
            gtk::glib::MainContext::default().iteration(false);
            if REPLACED.load(std::sync::atomic::Ordering::SeqCst) {
                self.close = true;
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close = true;
        }
        if self.tab == Tab::Status {
            self.current = state::current();
        }
        let top = egui::Panel::top("tabs")
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let w = 200.0;
                    ui.add_space((ui.available_width() - w) / 2.0);
                    ui.selectable_value(&mut self.tab, Tab::Status, tr("Status"));
                    ui.selectable_value(&mut self.tab, Tab::Settings, tr("Settings"));
                });
                ui.add_space(4.0);
            })
            .response
            .rect
            .height();
        if self.tab == Tab::Settings {
            egui::Panel::bottom("footer").show(ui, |ui| {
                ui.add_space(4.0);
                self.settings_footer(ui);
                ui.add_space(4.0);
            });
        }
        let content = egui::CentralPanel::default()
            .show(ui, |ui| match self.tab {
                Tab::Status => ui.scope(|ui| self.status_tab(ui)).response.rect.height() + 16.0,
                Tab::Settings => {
                    let out = egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.settings_tab(ui));
                    out.content_size.y + 56.0 // + footer
                }
            })
            .inner;
        // The window fits its content (up to a limit: the settings scroll).
        let wanted = (top + content + 24.0).clamp(240.0, 640.0).round();
        if (wanted - self.height).abs() > 2.0 {
            self.height = wanted;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(WIDTH, wanted)));
        }
        if self.close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

#[cfg(target_os = "linux")]
static REPLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Starts the tray icon if it isn't running (after installing the package,
/// it only starts at the next login otherwise), unless turned off by the
/// user in their startup applications.
fn start_tray() {
    #[cfg(windows)]
    if crate::tray::take_lock().is_some() {
        // (released right away: the tray takes it)
        spawn_detached(&["tray"]);
    }
    #[cfg(target_os = "linux")]
    {
        use gtk::{gio, glib};
        let override_file = crate::paths::config_home().join("autostart/luxafor-tray.desktop");
        if let Ok(text) = std::fs::read_to_string(override_file) {
            if text
                .lines()
                .any(|l| l.trim() == "Hidden=true" || l.trim() == "X-GNOME-Autostart-enabled=false")
            {
                return;
            }
        }
        let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
            return;
        };
        let running = bus
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                Some(&glib::Variant::tuple_from_iter([
                    glib::ToVariant::to_variant(crate::tray::BUS_NAME),
                ])),
                Some(glib::VariantTy::new("(b)").unwrap()),
                gio::DBusCallFlags::NONE,
                2000,
                gio::Cancellable::NONE,
            )
            .ok()
            .and_then(|r| r.child_value(0).get::<bool>())
            .unwrap_or(true); // can't tell: don't risk starting a second one
        if !running {
            spawn_detached(&["tray"]);
        }
    }
}

pub fn main(args: Vec<String>) -> i32 {
    let settings_tab = args.iter().any(|a| a == "--settings");
    let config = match Config::load(false) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("luxafor-panel: {e}");
            return 1;
        }
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Luxafor")
        .with_app_id("luxafor-status")
        .with_inner_size([WIDTH, 470.0])
        .with_min_inner_size([WIDTH, 300.0])
        .with_always_on_top();
    #[cfg(target_os = "linux")]
    {
        use gtk::gdk::prelude::*;
        use gtk::{gdk, gio};
        use std::sync::atomic::Ordering;
        if gtk::init().is_ok() {
            // Open where the mouse is (next to the tray icon, usually).
            if let Some(display) = gdk::Display::default() {
                if let Some(pointer) = display.default_seat().and_then(|s| s.pointer()) {
                    let (_screen, x, y) = pointer.position();
                    let area = display.monitor_at_point(x, y).map(|m| m.workarea());
                    let (left, top, right, bottom) = area
                        .map(|a| {
                            (
                                a.x() as f32,
                                a.y() as f32,
                                (a.x() + a.width()) as f32,
                                (a.y() + a.height()) as f32,
                            )
                        })
                        .unwrap_or((0.0, 0.0, f32::MAX, f32::MAX));
                    let px = (x as f32 - WIDTH / 2.0).clamp(left, (right - WIDTH).max(left));
                    let py = (y as f32 - 40.0).clamp(top, (bottom - 520.0).max(top));
                    viewport = viewport.with_position([px, py]);
                }
            }
            // Owned as long as the panel runs (never unowned).
            let _ = gio::bus_own_name(
                gio::BusType::Session,
                BUS_NAME,
                gio::BusNameOwnerFlags::ALLOW_REPLACEMENT | gio::BusNameOwnerFlags::REPLACE,
                |_, _| {},
                |_, _| {},
                |_, _| REPLACED.store(true, Ordering::SeqCst),
            );
        }
    }
    start_tray();
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    match eframe::run_native(
        "luxafor-panel",
        options,
        Box::new(move |cc| {
            let mut panel = Panel::new(&cc.egui_ctx, config);
            if settings_tab {
                panel.tab = Tab::Settings;
            }
            Ok(Box::new(panel))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("luxafor-panel: {e}");
            1
        }
    }
}
