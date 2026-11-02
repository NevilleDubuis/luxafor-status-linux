//! `luxafor-toggle`: sets or cycles the status — same options as v1.

use std::process::{Command, Stdio};

use crate::config::{self, Config};
use crate::i18n::{tr, trf};
use crate::light;
use crate::notify::{notify, status_icon};
use crate::paths;
use crate::state::{self, human_minutes, parse_duration, Timer};

const HELP: &str = "\
luxafor-toggle — set the status of a Luxafor light.

  luxafor-toggle                  cycle: first status → … → last status → off
  luxafor-toggle <id>             set a status by its id (see --list), or \"off\"
  luxafor-toggle <id> --for 30    set a status for 30 minutes (also 45m, 1h, 1h30),
                                  then go back to the default status
  luxafor-toggle --timer          show the running timer
  luxafor-toggle --cancel-timer   stop the timer, keep the current status
  luxafor-toggle --list           list configured statuses
  luxafor-toggle --list-effects   print \"<id> <effect>\" for each status
  luxafor-toggle --settings       show the settings
  luxafor-toggle --current        print the current status id
  luxafor-toggle --apply          show the current status on the light again
                                  (after plugging it back in)
  luxafor-toggle --edit           edit the configuration file (or use the
                                  Settings tab of luxafor-panel)
  luxafor-toggle --reload         regenerate icons and launcher after editing";

/// Prints an error, shows it as a notification too, and exits.
fn die(message: &str, code: i32) -> ! {
    eprintln!("luxafor-toggle: {message}");
    notify(message, "luxafor-status", "Luxafor");
    std::process::exit(code)
}

fn load(verbose: bool) -> Config {
    Config::load(verbose).unwrap_or_else(|e| die(&e, 1))
}

/// Sends a status to the light.
pub fn light(config: &Config, id: &str) -> Result<(), light::Error> {
    if id == "off" {
        return light::send(light::OFF);
    }
    let Some(status) = config.get(id) else {
        return light::send(light::OFF);
    };
    let Some(rgb) = crate::color::parse(&status.color) else {
        return Err(light::Error::Write(vec![trf(
            "unknown color '{color}' — use a name (red, orange…) or a hex code (#ff8800)",
            &[("color", &status.color)],
        )]));
    };
    light::send(light::command(
        crate::color::scale(rgb, config.brightness),
        status.effect,
    ))
}

/// Sets a status: light, state file, notification. Without a light (laptop
/// away from its dock), the status is still kept: the tray shows it on the
/// light when it's plugged back in.
pub fn set_status(config: &Config, id: &str, message: Option<String>) {
    if let Err(e) = try_set_status(config, id, message) {
        die(&e, 1);
    }
}

/// set_status for the tray and the call detection: errors are returned
/// (and shown as a notification), never fatal.
pub fn try_set_status(config: &Config, id: &str, message: Option<String>) -> Result<(), String> {
    let mut label = message.unwrap_or_else(|| config.label_of(id));
    match light(config, id) {
        Ok(()) => {}
        Err(light::Error::NoLight) => {
            label = trf("{status} — light not connected", &[("status", &label)]);
            eprintln!("luxafor-toggle: no light connected: '{id}' will be shown when it's plugged back in");
        }
        Err(e) => return Err(e.message()),
    }
    state::set_current(id).map_err(|e| format!("{}: {e}", paths::state().display()))?;
    notify(&label, &status_icon(id), "Luxafor");
    Ok(())
}

/// Starts this program again, detached (no terminal, own process group).
pub fn spawn_detached(args: &[&str]) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }
    let _ = cmd.spawn();
}

/// The background process that ends a timer. It sleeps in short steps
/// against the wall clock, so a suspended laptop doesn't delay it, and
/// leaves as soon as the timer is cancelled or replaced.
fn start_timer_watcher(timer: &Timer) {
    spawn_detached(&["toggle", "--timer-expire", &timer.token()]);
}

fn timer_expire(token: &str) {
    let Some(timer) = Timer::parse(token) else {
        return;
    };
    loop {
        if state::timer_token().as_deref() != Some(token) {
            return; // cancelled or replaced
        }
        let left = timer.end - state::now();
        if left <= 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(left.min(20) as u64));
    }
    finish_timer(token);
}

/// Ends a timer: back to the default status if the timer's status is still
/// on. If another status took over (a call, for instance), leave it alone:
/// the call detection goes back to the default status itself after the call.
pub fn finish_timer(token: &str) {
    // One at a time (a watcher, --resume-timer at login…).
    let lock_path = paths::lock("timer");
    if let Some(dir) = lock_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let lock = std::fs::File::create(&lock_path).ok();
    if let Some(f) = &lock {
        let _ = f.lock();
    }
    if state::timer_token().as_deref() != Some(token) {
        return;
    }
    state::clear_timer();
    let Some(timer) = Timer::parse(token) else {
        return;
    };
    if state::current() == timer.status {
        let config = load(false); // settings may have changed meanwhile
        let back = config.default_status.clone();
        let message = trf(
            "Timer ended — {status}",
            &[("status", &config.label_of(&back))],
        );
        set_status(&config, &back, Some(message));
    }
}

fn show_timer(config: &Config) {
    match state::timer() {
        None => println!("{}", tr("No timer.")),
        Some(t) => println!(
            "{}",
            trf(
                "{status} until {time} ({left} left), then {then}",
                &[
                    ("status", &config.label_of(&t.status)),
                    ("time", &state::clock(t.end)),
                    ("left", &human_minutes(t.minutes_left())),
                    ("then", &config.label_of(&config.default_status)),
                ],
            )
        ),
    }
}

/// --edit: an editor in the terminal, or the desktop's text editor with
/// --gui (what the tray and the panel use) or without a terminal.
fn edit_config(gui: bool) {
    let conf = paths::conf();
    if !conf.is_file() {
        if let Some(dir) = conf.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = paths::default_conf()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(|| config::BUILTIN.to_string());
        if let Err(e) = std::fs::write(&conf, text) {
            die(&format!("{}: {e}", conf.display()), 1);
        }
    }
    use std::io::IsTerminal;
    if !gui && std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "nano".into());
        let _ = Command::new(editor).arg(&conf).status();
        let config = load(true);
        crate::desktop::reload(&config);
        println!("Statuses reloaded.");
    } else {
        open_in_editor(&conf);
    }
}

/// Opens a file in the desktop's editor, fully detached so it outlives
/// this command. The tray reloads by itself once the file is saved.
pub fn open_in_editor(path: &std::path::Path) {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("notepad.exe");
        c.arg(path);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let _ = cmd.spawn();
}

pub fn main(args: Vec<String>) -> i32 {
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => {
            println!("{HELP}");
            return 0;
        }
        Some("--edit") => {
            edit_config(args.get(1).map(String::as_str) == Some("--gui"));
            return 0;
        }
        _ => {}
    }
    let verbose = matches!(
        args.first().map(String::as_str),
        Some("--list" | "--reload" | "--settings")
    );
    let config = load(verbose);

    // Options can come before or after the status id.
    let mut next: Option<String> = None;
    let mut duration: Option<String> = None;
    let mut keep_timer = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--list" => {
                for s in &config.statuses {
                    println!("{:<12} {:<10} {}", s.id, s.color, s.label);
                }
                println!("{:<12} {:<10} {}", "off", "-", tr("Off"));
                return 0;
            }
            "--list-effects" => {
                for s in &config.statuses {
                    println!("{} {}", s.id, s.effect.as_str());
                }
                return 0;
            }
            "--current" => {
                println!("{}", state::current());
                return 0;
            }
            "--apply" => {
                let mut cur = state::current();
                if cur != "off" && !config.has(&cur) {
                    cur = "off".into();
                }
                return match light(&config, &cur) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("luxafor-light: {}", e.message());
                        e.exit_code()
                    }
                };
            }
            "--reload" => {
                crate::desktop::reload(&config);
                println!("Statuses reloaded.");
                return 0;
            }
            "--settings" => {
                let sc: Vec<String> = config
                    .shortcuts
                    .iter()
                    .map(|(id, m)| format!("{id} {m}"))
                    .collect();
                let none = |v: String| {
                    if v.is_empty() {
                        "(none)".to_string()
                    } else {
                        v
                    }
                };
                println!("default-status  = {}", config.default_status);
                println!("call-status     = {}", config.call_status);
                println!("call-apps       = {}", config.call_apps);
                println!("lock-status     = {}", config.lock_status);
                println!("dnd-statuses    = {}", none(config.dnd_statuses.join(" ")));
                println!("timer-shortcuts = {}", none(sc.join(", ")));
                println!("brightness      = {}", config.brightness);
                println!("theme           = {}", config.theme);
                return 0;
            }
            "--setting" => {
                match args.get(i + 1).map(String::as_str).unwrap_or("") {
                    "default-status" => println!("{}", config.default_status),
                    "call-status" => println!("{}", config.call_status),
                    "call-apps" => println!("{}", config.call_apps),
                    "lock-status" => println!("{}", config.lock_status),
                    "dnd-statuses" => println!("{}", config.dnd_statuses.join(" ")),
                    "brightness" => println!("{}", config.brightness),
                    "theme" => println!("{}", config.theme),
                    "timer-shortcuts" => {
                        for (id, m) in &config.shortcuts {
                            println!("{id} {m}");
                        }
                    }
                    other => {
                        eprintln!("Unknown setting '{other}'");
                        return 2;
                    }
                }
                return 0;
            }
            "--timer" => {
                show_timer(&config);
                return 0;
            }
            "--cancel-timer" => {
                if paths::timer().exists() {
                    state::clear_timer();
                    notify(&tr("Timer cancelled"), "luxafor-status", "Luxafor");
                }
                return 0;
            }
            "--resume-timer" => {
                // At login: restart the watcher, or end an expired timer.
                if let Some(t) = state::timer() {
                    if t.end <= state::now() {
                        finish_timer(&t.token());
                    } else {
                        start_timer_watcher(&t);
                    }
                }
                return 0;
            }
            "--timer-expire" => {
                timer_expire(args.get(i + 1).map(String::as_str).unwrap_or(""));
                return 0;
            }
            "--keep-timer" => keep_timer = true,
            "--for" => {
                let Some(d) = args.get(i + 1) else {
                    eprintln!("--for needs a duration, e.g. --for 30");
                    return 2;
                };
                duration = Some(d.clone());
                i += 1;
            }
            _ if arg.starts_with("--for=") => duration = Some(arg["--for=".len()..].to_string()),
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {arg} (see --help)");
                return 2;
            }
            _ => {
                if let Some(prev) = &next {
                    eprintln!("Only one status at a time (got '{prev}' and '{arg}')");
                    return 2;
                }
                next = Some(arg.to_string());
            }
        }
        i += 1;
    }

    let next = match next {
        None => {
            if duration.is_some() {
                eprintln!("--for needs a status, e.g. luxafor-toggle busy --for 30");
                return 2;
            }
            // Cycle: first status → … → last status → off.
            let cur = state::current();
            match config.statuses.iter().position(|s| s.id == cur) {
                None => config.statuses[0].id.clone(),
                Some(i) if i + 1 < config.statuses.len() => config.statuses[i + 1].id.clone(),
                Some(_) => "off".into(),
            }
        }
        Some(n) if n != "off" && !config.has(&n) => {
            eprintln!("{}", config::status_unknown(&n, &config));
            return 2;
        }
        Some(n) => n,
    };

    if let Some(d) = duration {
        let Some(secs) = parse_duration(&d) else {
            eprintln!("Invalid duration '{d}' — use minutes (30), or 45m, 1h, 1h30 (max 24h)");
            return 2;
        };
        let end = state::now() + secs as i64;
        let message = trf(
            "{status} for {duration} — until {time}",
            &[
                ("status", &config.label_of(&next)),
                ("duration", &human_minutes(secs as i64 / 60)),
                ("time", &state::clock(end)),
            ],
        );
        set_status(&config, &next, Some(message));
        let timer = Timer { end, status: next };
        if let Err(e) = state::set_timer(&timer) {
            die(&format!("{}: {e}", paths::timer().display()), 1);
        }
        start_timer_watcher(&timer);
    } else {
        // A status chosen by hand replaces any running timer (its watcher
        // notices and leaves).
        if !keep_timer {
            state::clear_timer();
        }
        set_status(&config, &next, None);
    }
    0
}
