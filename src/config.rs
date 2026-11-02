//! statuses.conf: the statuses ("id | color | label [| effect]") and the
//! settings ("key = value"). Same format and rules as v1.

use std::path::{Path, PathBuf};

use crate::i18n::{tr, trf};
use crate::paths;
use crate::state::parse_duration;

/// The shipped statuses, used when no file is found (Windows, or a broken
/// install).
pub const BUILTIN: &str = include_str!("../share/statuses.conf");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Effect {
    #[default]
    Steady,
    BlinkSlow,
    Blink,
    BlinkFast,
}

impl Effect {
    pub const ALL: [Effect; 4] = [
        Effect::Steady,
        Effect::BlinkSlow,
        Effect::Blink,
        Effect::BlinkFast,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Steady => "steady",
            Effect::BlinkSlow => "blink-slow",
            Effect::Blink => "blink",
            Effect::BlinkFast => "blink-fast",
        }
    }

    pub fn parse(s: &str) -> Option<Effect> {
        Effect::ALL.into_iter().find(|e| e.as_str() == s)
    }

    pub fn blinks(self) -> bool {
        self != Effect::Steady
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub id: String,
    pub color: String,
    pub label: String,
    pub effect: Effect,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub statuses: Vec<Status>,
    /// Status a timer goes back to.
    pub default_status: String,
    /// Status during calls, or "none".
    pub call_status: String,
    /// Status while the screen is locked, "off", or "none".
    pub lock_status: String,
    pub dnd_statuses: Vec<String>,
    /// Timer shortcuts of the tray menu: (status id, minutes).
    pub shortcuts: Vec<(String, u32)>,
    pub brightness: u32,
    pub theme: String,
    pub call_apps: String,
}

pub const DEFAULT_CALL_APPS: &str =
    "firefox chrome chromium brave msedge edge vivaldi opera epiphany falkon";

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id != "off"
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[derive(Default)]
struct Raw {
    default: String,
    call: String,
    theme: String,
    shortcuts: Option<String>,
    lock: String,
    dnd: String,
    brightness: String,
    apps: String,
}

impl Config {
    /// Loads the user's file, else the shipped one. `verbose`: report
    /// invalid lines and settings on stderr (prefixed with `who`).
    pub fn load(verbose: bool) -> Result<Config, String> {
        let own = paths::conf();
        let (file, text, own_file) = if own.is_file() {
            let text =
                std::fs::read_to_string(&own).map_err(|e| format!("{}: {e}", own.display()))?;
            (Some(own), text, true)
        } else if let Some(def) = paths::default_conf() {
            let text =
                std::fs::read_to_string(&def).map_err(|e| format!("{}: {e}", def.display()))?;
            (Some(def), text, false)
        } else {
            (None, BUILTIN.to_string(), false)
        };
        Config::parse(&text, file, own_file, verbose)
    }

    pub fn parse(
        text: &str,
        file: Option<PathBuf>,
        own: bool,
        verbose: bool,
    ) -> Result<Config, String> {
        let name = file
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "statuses.conf".into());
        let warn = |msg: String| {
            if verbose {
                eprintln!("luxafor-toggle: {msg}");
            }
        };
        let mut statuses = Vec::new();
        let mut raw = Raw::default();
        for (n, line) in text.lines().enumerate() {
            let n = n + 1;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if !line.contains('|') && line.contains('=') {
                let (key, value) = line.split_once('=').unwrap();
                let key = key.trim();
                let value = value.split('#').next().unwrap_or("").trim().to_string();
                match key {
                    "default-status" => raw.default = value,
                    "call-status" => raw.call = value,
                    "theme" => raw.theme = value,
                    "timer-shortcuts" => raw.shortcuts = Some(value),
                    "lock-status" => raw.lock = value,
                    "dnd-statuses" => raw.dnd = value,
                    "brightness" => raw.brightness = value,
                    "call-apps" => raw.apps = value,
                    _ => warn(format!("unknown setting '{key}' on line {n} in {name}")),
                }
                continue;
            }
            let mut parts = line.splitn(3, '|');
            let id = parts.next().unwrap_or("").trim();
            let color = parts.next().unwrap_or("").trim();
            let mut label = parts.next().unwrap_or("").trim().to_string();
            if !valid_id(id) || color.is_empty() {
                warn(format!("ignoring invalid line {n} in {name}: {line}"));
                continue;
            }
            // "id | color | label | effect": the last field is the effect if
            // it is one (labels may contain "|" themselves).
            let mut effect = Effect::Steady;
            if let Some((head, last)) = label.rsplit_once('|') {
                if let Some(e) = Effect::parse(last.trim()) {
                    effect = e;
                    label = head.trim().to_string();
                }
            }
            if label.is_empty() {
                label = id.to_string();
            }
            // Shipped statuses (none of the user's own yet): translated.
            if !own {
                label = tr(&label);
            }
            statuses.push(Status {
                id: id.into(),
                color: color.into(),
                label,
                effect,
            });
        }
        if statuses.is_empty() {
            return Err(format!("no valid status in {name}"));
        }
        let mut config = Config {
            statuses,
            default_status: String::new(),
            call_status: String::new(),
            lock_status: String::new(),
            dnd_statuses: Vec::new(),
            shortcuts: Vec::new(),
            brightness: 100,
            theme: "auto".into(),
            call_apps: String::new(),
        };
        config.resolve(raw, &warn);
        Ok(config)
    }

    fn resolve(&mut self, raw: Raw, warn: &dyn Fn(String)) {
        // Status a timer goes back to: the setting, else "available", else
        // the first status.
        if !raw.default.is_empty() && (raw.default == "off" || self.has(&raw.default)) {
            self.default_status = raw.default;
        } else {
            if !raw.default.is_empty() {
                warn(format!(
                    "default-status '{}' is not one of your statuses",
                    raw.default
                ));
            }
            self.default_status = if self.has("available") {
                "available".into()
            } else {
                self.statuses[0].id.clone()
            };
        }
        self.theme = match raw.theme.as_str() {
            "" => "auto".into(),
            "auto" | "light" | "dark" => raw.theme,
            other => {
                warn(format!("theme '{other}' should be auto, light or dark"));
                "auto".into()
            }
        };
        // Timer shortcuts: without the setting, the first status that isn't
        // the default one, for 30 min and 1 h. An empty setting: none.
        let spec = match raw.shortcuts {
            Some(spec) => spec,
            None => self
                .statuses
                .iter()
                .find(|s| s.id != self.default_status)
                .map(|s| format!("{0} 30, {0} 60", s.id))
                .unwrap_or_default(),
        };
        for entry in spec.split(',') {
            let mut words = entry.split_whitespace();
            let Some(id) = words.next() else { continue };
            match (self.has(id), parse_duration(words.next().unwrap_or(""))) {
                (true, Some(secs)) => self.shortcuts.push((id.to_string(), (secs / 60) as u32)),
                _ => warn(format!(
                    "ignoring timer shortcut '{}' — use: <status> <duration>, e.g. busy 30",
                    entry.trim()
                )),
            }
        }
        // Status during calls, or "none" (no call detection).
        if raw.call == "none" || (!raw.call.is_empty() && self.has(&raw.call)) {
            self.call_status = raw.call;
        } else {
            if !raw.call.is_empty() {
                warn(format!(
                    "call-status '{}' is not one of your statuses",
                    raw.call
                ));
            }
            self.call_status = if self.has("call") {
                "call".into()
            } else {
                "none".into()
            };
        }
        // Status while the screen is locked, "off" (default) or "none".
        if raw.lock == "none" || raw.lock == "off" || (!raw.lock.is_empty() && self.has(&raw.lock))
        {
            self.lock_status = raw.lock;
        } else {
            if !raw.lock.is_empty() {
                warn(format!(
                    "lock-status '{}' is not one of your statuses",
                    raw.lock
                ));
            }
            self.lock_status = "off".into();
        }
        for d in raw.dnd.split([',', ' ']).filter(|d| !d.is_empty()) {
            if self.has(d) {
                self.dnd_statuses.push(d.to_string());
            } else {
                warn(format!("dnd-statuses: '{d}' is not one of your statuses"));
            }
        }
        self.brightness = match raw.brightness.parse::<u32>() {
            Ok(b) if (5..=100).contains(&b) => b,
            _ => {
                if !raw.brightness.is_empty() {
                    warn(format!(
                        "brightness '{}' should be a percentage from 5 to 100",
                        raw.brightness
                    ));
                }
                100
            }
        };
        self.call_apps = if raw.apps.is_empty() {
            DEFAULT_CALL_APPS.into()
        } else {
            raw.apps
        };
    }

    pub fn has(&self, id: &str) -> bool {
        self.statuses.iter().any(|s| s.id == id)
    }

    pub fn get(&self, id: &str) -> Option<&Status> {
        self.statuses.iter().find(|s| s.id == id)
    }

    pub fn label_of(&self, id: &str) -> String {
        if id == "off" {
            return tr("Off");
        }
        self.get(id)
            .map(|s| s.label.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn color_of(&self, id: &str) -> String {
        self.get(id)
            .map(|s| s.color.clone())
            .unwrap_or_else(|| crate::color::OFF_COLOR.into())
    }

    /// The file's modification time, to notice changes.
    pub fn mtime() -> Option<std::time::SystemTime> {
        let own = paths::conf();
        let path = if own.is_file() {
            Some(own)
        } else {
            paths::default_conf()
        };
        path.and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
    }
}

/// What the panel's Settings tab saves.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub statuses: Vec<Status>,
    pub default_status: String,
    pub call_status: String,
    pub lock_status: String,
    pub dnd_statuses: Vec<String>,
    pub shortcuts: Vec<(String, u32)>,
    pub brightness: u32,
    pub theme: String,
}

/// Settings the panel manages; other "key = value" lines are kept as they are.
const MANAGED: [&str; 7] = [
    "default-status",
    "call-status",
    "lock-status",
    "dnd-statuses",
    "timer-shortcuts",
    "brightness",
    "theme",
];

/// Writes statuses.conf (the previous file is kept as statuses.conf.bak).
pub fn write(settings: &Settings, path: &Path) -> std::io::Result<()> {
    let mut lines: Vec<String> = [
        "# luxafor-status — status configuration",
        "# Edited with Luxafor Panel → Settings. Hand edits work too:",
        "# one status per line, \"id | color | label\" (add \"| blink\",",
        "# \"| blink-slow\" or \"| blink-fast\" to make it blink), then settings as",
        "# \"key = value\" — see: man luxafor-toggle",
        "",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let width = settings
        .statuses
        .iter()
        .map(|s| s.id.len())
        .max()
        .unwrap_or(0)
        .max(4);
    for s in &settings.statuses {
        let mut line = format!("{:width$} | {:8} | {}", s.id, s.color, s.label);
        if s.effect.blinks() {
            line.push_str(&format!(" | {}", s.effect.as_str()));
        }
        lines.push(line);
    }
    let shortcuts: Vec<String> = settings
        .shortcuts
        .iter()
        .map(|(id, m)| format!("{id} {m}"))
        .collect();
    lines.extend([
        String::new(),
        format!("default-status = {}", settings.default_status),
        format!("call-status = {}", settings.call_status),
        format!("lock-status = {}", settings.lock_status),
        format!("dnd-statuses = {}", settings.dnd_statuses.join(", ")),
        format!("timer-shortcuts = {}", shortcuts.join(", ")),
        format!("brightness = {}", settings.brightness),
        format!("theme = {}", settings.theme),
    ]);
    lines.extend(other_setting_lines(path));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        std::fs::copy(path, path.with_extension("conf.bak"))?;
    }
    let tmp = path.with_extension("conf.tmp");
    std::fs::write(&tmp, lines.join("\n") + "\n")?;
    std::fs::rename(&tmp, path)
}

fn other_setting_lines(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.contains('|') && l.contains('='))
        .filter(|l| !MANAGED.contains(&l.split('=').next().unwrap_or("").trim()))
        .map(String::from)
        .collect()
}

/// A command-line id made from a label: "Focus — ne pas déranger" →
/// "focus-ne-pas-deranger"; unique among `taken`.
pub fn make_id(label: &str, taken: &[String]) -> String {
    let mut text = String::new();
    for c in label.chars() {
        let c = fold_accent(c).to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            text.push(c);
        } else if c.is_ascii() || !c.is_alphanumeric() {
            text.push('-');
        }
    }
    let mut base: String = text
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(24);
    let mut base = base.trim_matches('-').to_string();
    if base.is_empty() {
        base = "status".into();
    }
    if base == "off" {
        base = "off-status".into();
    }
    let (mut id, mut n) = (base.clone(), 2);
    while taken.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

fn fold_accent(c: char) -> char {
    const FROM: &str = "àáâãäåçèéêëìíîïñòóôõöùúûüýÿÀÁÂÃÄÅÇÈÉÊËÌÍÎÏÑÒÓÔÕÖÙÚÛÜÝ";
    const TO: &str = "aaaaaaceeeeiiiinooooouuuuyyAAAAAACEEEEIIIINOOOOOUUUUY";
    FROM.chars()
        .position(|f| f == c)
        .and_then(|i| TO.chars().nth(i))
        .unwrap_or(c)
}

/// Message for "no light" etc. are in light.rs; this one is shared.
pub fn status_unknown(id: &str, config: &Config) -> String {
    let ids: Vec<&str> = config.statuses.iter().map(|s| s.id.as_str()).collect();
    trf(
        "Unknown status '{id}'. Available: {ids} off",
        &[("id", &id), ("ids", &ids.join(" "))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Config {
        Config::parse(text, None, true, false).unwrap()
    }

    #[test]
    fn statuses_and_effects() {
        let c = parse(
            "# comment\ndispo | lime | Disponible\nBad Id | red | x\npipe | purple | Label | with pipe\n\
             alert | red | Alerte | blink-fast\nquiet | blue |  | blink-slow\nnocolor | | x\n",
        );
        let ids: Vec<_> = c.statuses.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["dispo", "pipe", "alert", "quiet"]);
        assert_eq!(c.statuses[1].label, "Label | with pipe");
        assert_eq!(c.statuses[2].label, "Alerte");
        assert_eq!(c.statuses[2].effect, Effect::BlinkFast);
        assert_eq!(c.statuses[3].label, "quiet");
    }

    #[test]
    fn settings_defaults() {
        let c = parse("dispo | lime | D\nfocus | blue | F\n");
        assert_eq!(c.default_status, "dispo");
        assert_eq!(c.call_status, "none");
        assert_eq!(c.lock_status, "off");
        assert_eq!(
            c.shortcuts,
            [("focus".to_string(), 30), ("focus".to_string(), 60)]
        );
        assert_eq!(c.brightness, 100);
        assert_eq!(c.theme, "auto");
        assert!(c.call_apps.starts_with("firefox chrome"));
    }

    #[test]
    fn settings_values() {
        let c = parse(
            "a | red | A\nb | blue | B\ndefault-status = b\ncall-status = nope\nlock-status = a\n\
             dnd-statuses = a, nope b\nbrightness = 40 # dim\ntimer-shortcuts = a 1h30, nope 30, a soon, b 15\n",
        );
        assert_eq!(c.default_status, "b");
        assert_eq!(c.call_status, "none");
        assert_eq!(c.lock_status, "a");
        assert_eq!(c.dnd_statuses, ["a", "b"]);
        assert_eq!(c.brightness, 40);
        assert_eq!(c.shortcuts, [("a".to_string(), 90), ("b".to_string(), 15)]);
        let empty = parse("a | red | A\ntimer-shortcuts =\n");
        assert!(empty.shortcuts.is_empty());
    }

    #[test]
    fn no_status() {
        assert!(Config::parse("theme = dark\n", None, true, false).is_err());
    }

    #[test]
    fn ids_from_labels() {
        assert_eq!(
            make_id("Focus — ne pas déranger", &[]),
            "focus-ne-pas-deranger"
        );
        assert_eq!(make_id("Pause café", &[]), "pause-cafe");
        assert_eq!(make_id("Off", &[]), "off-status");
        assert_eq!(make_id("!!!", &[]), "status");
        assert_eq!(make_id("Busy", &["busy".into(), "busy-2".into()]), "busy-3");
    }

    #[test]
    fn write_and_read_back() {
        let dir = std::env::temp_dir().join(format!("lx-conf-{}", std::process::id()));
        let path = dir.join("statuses.conf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            "a | red | A\nmy-own-key = kept\ncall-apps = teams\ntheme = dark\n",
        )
        .unwrap();
        let s = Settings {
            statuses: vec![
                Status {
                    id: "a".into(),
                    color: "red".into(),
                    label: "A".into(),
                    effect: Effect::Steady,
                },
                Status {
                    id: "bb".into(),
                    color: "#123456".into(),
                    label: "B b".into(),
                    effect: Effect::Blink,
                },
            ],
            default_status: "a".into(),
            call_status: "none".into(),
            lock_status: "off".into(),
            dnd_statuses: vec!["bb".into()],
            shortcuts: vec![("bb".into(), 15)],
            brightness: 60,
            theme: "light".into(),
        };
        write(&s, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("bb   | #123456  | B b | blink\n"), "{text}");
        assert!(text.contains("my-own-key = kept"));
        assert!(text.contains("call-apps = teams"));
        assert!(!text.contains("theme = dark"));
        assert!(path.with_extension("conf.bak").exists());
        let c = parse(&text);
        assert_eq!(c.statuses[1].effect, Effect::Blink);
        assert_eq!(c.brightness, 60);
        assert_eq!(c.dnd_statuses, ["bb"]);
        assert_eq!(c.call_apps, "teams");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
