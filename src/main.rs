//! luxafor-status — control a Luxafor status light from your desktop.
//!
//! One program, several commands (also reachable through links named after
//! the v1 commands, so scripts and shortcuts keep working):
//!
//!   luxafor-status                 the panel (and the tray icon, if it isn't running)
//!   luxafor-status toggle …        = luxafor-toggle: set or cycle the status
//!   luxafor-status light …         = luxafor-light: drive the light directly
//!   luxafor-status tray            = luxafor-tray: the tray icon (runs at login)
//!   luxafor-status panel           = luxafor-panel: the panel
//!   luxafor-status callwatch …     = luxafor-callwatch: call detection on its own

// No console window for the tray and the panel on Windows. (The command
// line still prints when started from a terminal: see attach_console.)
#![cfg_attr(windows, windows_subsystem = "windows")]

mod calls;
mod color;
mod config;
mod desktop;
mod i18n;
mod light;
mod lightcli;
mod notify;
mod panel;
mod paths;
mod state;
mod toggle;
mod tray;

const COMMANDS: &[&str] = &["toggle", "light", "tray", "panel", "callwatch"];

fn main() {
    #[cfg(windows)]
    attach_console();
    let mut args: Vec<String> = std::env::args().collect();
    let name = std::path::Path::new(&args[0])
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    args.remove(0);
    let command = match name.strip_prefix("luxafor-") {
        Some(cmd) if COMMANDS.contains(&cmd) => cmd.to_string(),
        _ => match args.first().map(String::as_str) {
            Some(cmd) if COMMANDS.contains(&cmd) => args.remove(0),
            Some("-h" | "--help") => {
                println!(
                    "{}",
                    include_str!("main.rs")
                        .lines()
                        .skip(2)
                        .take(8)
                        .map(|l| l.trim_start_matches("//!").strip_prefix(' ').unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                return;
            }
            Some("--version") => {
                println!("luxafor-status {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            // `luxafor-status busy --for 30`: a toggle command.
            Some(_) => "toggle".into(),
            None => "panel".into(),
        },
    };
    let code = match command.as_str() {
        "toggle" => toggle::main(args),
        "light" => lightcli::main(args),
        "tray" => tray::main(args),
        "panel" => panel::main(args),
        "callwatch" => calls::main(args),
        _ => unreachable!(),
    };
    std::process::exit(code);
}

/// Windows: a GUI program has no console; when started from a terminal,
/// print to that terminal anyway.
#[cfg(windows)]
fn attach_console() {
    extern "system" {
        fn AttachConsole(pid: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}
