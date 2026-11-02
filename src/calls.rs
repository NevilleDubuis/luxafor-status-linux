//! Call detection: while a watched application (web browsers by default —
//! Teams, Odoo, Meet, Jitsi… in a browser tab) records from the microphone,
//! the light switches to the call status, and goes back to the previous
//! status when the call ends — unless the status was changed by hand
//! meanwhile. A timer running when the call starts keeps running; if it
//! ended during the call, the light goes to the default status instead.
//!
//! Linux: microphone streams from `pactl` (PulseAudio / PipeWire).
//! Windows: the apps Windows lists as using the microphone.

use std::time::Duration;

use crate::config::Config;
use crate::i18n::tr;
use crate::state;
use crate::toggle::try_set_status;

fn log(message: &str) {
    eprintln!("luxafor-callwatch: {message}");
}

/// For problems the user must fix: the watcher runs at login, so stderr is
/// usually not visible.
fn alert(message: &str) {
    log(message);
    crate::notify::notify(message, "luxafor-status", &tr("Luxafor call detection"));
}

/// One entry per microphone stream: (application name, binary).
#[cfg(not(windows))]
pub fn mic_streams() -> Vec<(String, String)> {
    let Ok(out) = std::process::Command::new("pactl")
        .args(["list", "source-outputs"])
        .env("LC_ALL", "C")
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut streams = Vec::new();
    let mut current: Option<(String, String)> = None;
    let value = |line: &str| {
        line.split_once('"')
            .map(|(_, v)| v.strip_suffix('"').unwrap_or(v).to_string())
            .unwrap_or_default()
    };
    for line in text.lines() {
        if line.starts_with("Source Output #") {
            streams.extend(current.take());
            current = Some(Default::default());
        } else if let Some(c) = current.as_mut() {
            let l = line.trim();
            if l.starts_with("application.process.binary = ") {
                c.1 = value(l);
            } else if l.starts_with("application.name = ") {
                c.0 = value(l);
            }
        }
    }
    streams.extend(current);
    streams
}

/// Apps using the microphone right now: Windows records it under
/// CapabilityAccessManager — an app whose LastUsedTimeStop is 0 uses it.
#[cfg(windows)]
pub fn mic_streams() -> Vec<(String, String)> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    fn scan(key: &RegKey, users: &mut Vec<(String, String)>) {
        for name in key.enum_keys().flatten() {
            let Ok(app) = key.open_subkey(&name) else {
                continue;
            };
            if name == "NonPackaged" {
                scan(&app, users);
                continue;
            }
            let stop: u64 = app.get_value("LastUsedTimeStop").unwrap_or(1);
            let start: u64 = app.get_value("LastUsedTimeStart").unwrap_or(0);
            if stop == 0 && start != 0 {
                let path = name.replace('#', "\\");
                let short = path.rsplit('\\').next().unwrap_or(&path).to_string();
                users.push((short, path));
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

/// True if one of `apps` (space-separated, matched in the name or binary)
/// uses the microphone.
pub fn in_call(apps: &str, streams: &[(String, String)]) -> bool {
    streams.iter().any(|(name, bin)| {
        let text = format!("{bin}|{name}").to_lowercase();
        apps.split_whitespace()
            .any(|app| text.contains(&app.to_lowercase()))
    })
}

/// `luxafor-callwatch --check`
pub fn check() -> i32 {
    #[cfg(not(windows))]
    if !has_pactl() {
        log("pactl not found — install pulseaudio-utils");
        return 1;
    }
    let config = Config::load(false).unwrap_or_else(|e| {
        log(&e);
        std::process::exit(1)
    });
    println!(
        "Call status: {}    Watched apps: {}",
        config.call_status, config.call_apps
    );
    println!();
    let streams = mic_streams();
    if streams.is_empty() {
        println!("No application is using the microphone.");
    } else {
        println!("Applications using the microphone:");
        for (name, bin) in &streams {
            let or = |s: &str| {
                if s.is_empty() {
                    "?".to_string()
                } else {
                    s.to_string()
                }
            };
            println!("  - {} ({})", or(name), or(bin));
        }
    }
    println!();
    println!(
        "{}",
        if in_call(&config.call_apps, &streams) {
            "→ This counts as a call."
        } else {
            "→ Not a call."
        }
    );
    0
}

#[cfg(not(windows))]
fn has_pactl() -> bool {
    std::process::Command::new("pactl")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Follows calls and changes the status.
#[derive(Default)]
pub struct Watcher {
    /// While we have switched the light to the call status: the call
    /// status used, the status to restore, and the end of the timer that
    /// was running when the call started.
    active: Option<(String, String, Option<i64>)>,
}

impl Watcher {
    pub fn evaluate(&mut self) {
        let Ok(config) = Config::load(false) else {
            return;
        };
        let calling = in_call(&config.call_apps, &mic_streams());
        if calling {
            if self.active.is_none() {
                let call = config.call_status.clone();
                if call == "none" {
                    return; // call detection turned off
                }
                let previous = state::current();
                let timer_end = state::timer().map(|t| t.end);
                if previous != call {
                    // A call doesn't cancel a running timer.
                    if let Err(e) = try_set_status(&config, &call, None) {
                        alert(&e);
                    }
                }
                log(&format!("call started (previous status: {previous})"));
                self.active = Some((call, previous, timer_end));
            }
        } else if let Some((call, previous, timer_end)) = self.active.clone() {
            // Browsers sometimes close and reopen the microphone stream
            // (device change, reconnection) during a call: wait a little.
            let delay = std::env::var("LUXAFOR_CALL_END_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0);
            std::thread::sleep(Duration::from_secs_f64(delay));
            let Ok(config) = Config::load(false) else {
                return;
            };
            if in_call(&config.call_apps, &mic_streams()) {
                return;
            }
            self.active = None;
            // Only restore if nobody changed the status during the call.
            if state::current() == call && previous != call {
                let mut back = previous;
                // The timer that was running ended during the call: go where
                // it would have gone, the default status.
                if let Some(end) = timer_end {
                    if state::timer().is_none() && state::now() >= end {
                        back = config.default_status.clone();
                    }
                }
                if let Err(e) = try_set_status(&config, &back, None) {
                    alert(&e);
                }
            }
            log("call ended");
        }
    }

    /// Watches forever (in its own thread or process).
    pub fn run(mut self) {
        let old = crate::paths::config_home()
            .join("luxafor-status")
            .join("callwatch.conf");
        if old.is_file() {
            alert(&tr("callwatch.conf is no longer read: set the call status and the watched apps in Luxafor Panel → Settings (call-status, call-apps in statuses.conf)"));
        }
        #[cfg(not(windows))]
        {
            use std::io::BufRead;
            if !has_pactl() {
                alert(&tr(
                    "pactl not found — install it with: sudo apt install pulseaudio-utils",
                ));
                return;
            }
            // React to microphone streams appearing or disappearing. If the
            // audio server restarts, "pactl subscribe" exits: subscribe again.
            loop {
                self.evaluate();
                if let Ok(mut child) = std::process::Command::new("pactl")
                    .arg("subscribe")
                    .env("LC_ALL", "C")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    if let Some(out) = child.stdout.take() {
                        for line in std::io::BufReader::new(out).lines() {
                            let Ok(line) = line else { break };
                            if line.contains("source-output") {
                                self.evaluate();
                            }
                        }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        }
        #[cfg(windows)]
        loop {
            self.evaluate();
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}

/// Holds the call-detection lock: one watcher per user, whether it runs in
/// the tray or on its own. None: another one is running.
pub fn take_lock() -> Option<std::fs::File> {
    let path = crate::paths::lock("callwatch");
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

/// `luxafor-callwatch`: watches on its own (the tray does it too).
pub fn main(args: Vec<String>) -> i32 {
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => {
            println!(
                "luxafor-callwatch — switch the light to a status while you are in a call.\n\n  \
                 luxafor-callwatch           watch the microphone (the tray icon does it too)\n  \
                 luxafor-callwatch --check   show which apps use the microphone right now"
            );
            0
        }
        Some("--check") => check(),
        None => {
            let Some(_lock) = take_lock() else {
                log("already running");
                return 0;
            };
            Watcher::default().run();
            1
        }
        Some(other) => {
            log(&format!("unknown option '{other}' (see --help)"));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_matching() {
        let streams = vec![("Google Chrome".to_string(), "chrome".to_string())];
        assert!(in_call("firefox chrome", &streams));
        assert!(!in_call("firefox", &streams));
        assert!(in_call("CHROME", &streams));
        assert!(!in_call("", &streams));
    }
}
