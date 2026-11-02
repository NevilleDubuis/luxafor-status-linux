//! Where things are kept. Same files as v1 on Linux (XDG directories), so an
//! upgrade keeps the statuses, the current status and a running timer.

use std::path::PathBuf;

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn home() -> PathBuf {
    env_dir("HOME")
        .or_else(|| env_dir("USERPROFILE"))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(not(windows))]
pub fn config_home() -> PathBuf {
    env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home().join(".config"))
}
#[cfg(not(windows))]
pub fn cache_home() -> PathBuf {
    env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home().join(".cache"))
}
#[cfg(not(windows))]
pub fn data_home() -> PathBuf {
    env_dir("XDG_DATA_HOME").unwrap_or_else(|| home().join(".local/share"))
}

#[cfg(windows)]
pub fn config_home() -> PathBuf {
    env_dir("APPDATA").unwrap_or_else(|| home().join("AppData/Roaming"))
}
#[cfg(windows)]
pub fn cache_home() -> PathBuf {
    env_dir("LOCALAPPDATA")
        .unwrap_or_else(|| home().join("AppData/Local"))
        .join("luxafor-status")
}
#[cfg(windows)]
pub fn data_home() -> PathBuf {
    cache_home()
}

/// The user's statuses and settings.
pub fn conf() -> PathBuf {
    config_home().join("luxafor-status").join("statuses.conf")
}

/// The shipped statuses (package: /usr/share; per-user install:
/// ~/.local/share), used while the user has none of their own. None:
/// use the copy built into the program.
pub fn default_conf() -> Option<PathBuf> {
    #[cfg(not(windows))]
    {
        let dirs = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        std::iter::once(data_home())
            .chain(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from))
            .map(|d| d.join("luxafor-status").join("statuses.conf"))
            .find(|p| p.is_file())
    }
    #[cfg(windows)]
    {
        None
    }
}

/// The current status id.
pub fn state() -> PathBuf {
    cache_home().join("luxafor-state")
}

/// The running timer: "<end epoch> <status id>".
pub fn timer() -> PathBuf {
    cache_home().join("luxafor-timer")
}

/// While the tray has silenced notifications: the user's own setting.
pub fn dnd_mark() -> PathBuf {
    cache_home().join("luxafor-dnd")
}

pub fn tray_log() -> PathBuf {
    cache_home().join("luxafor-tray.log")
}

/// Lock files (one process at a time): runtime dir if there is one.
pub fn lock(name: &str) -> PathBuf {
    env_dir("XDG_RUNTIME_DIR")
        .unwrap_or_else(cache_home)
        .join(format!("luxafor-{name}.lock"))
}

/// Status icons for notifications (and the launcher), one SVG per status.
pub fn icon_dir() -> PathBuf {
    data_home().join("luxafor-status").join("icons")
}

/// The dock launcher, with one right-click action per status.
pub fn desktop_file() -> PathBuf {
    data_home().join("applications").join("luxafor.desktop")
}
