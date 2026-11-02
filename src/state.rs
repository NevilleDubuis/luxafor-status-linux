//! The current status and the running timer, kept in small files so every
//! part (command line, tray, panel) agrees — same files as v1.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::i18n::trf;
use crate::paths;

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The current status id ("off" if none).
pub fn current() -> String {
    std::fs::read_to_string(paths::state())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "off".into())
}

pub fn set_current(id: &str) -> std::io::Result<()> {
    let path = paths::state();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{id}\n"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    pub end: i64,
    pub status: String,
}

impl Timer {
    /// The file's content, which also identifies this timer.
    pub fn token(&self) -> String {
        format!("{} {}", self.end, self.status)
    }

    pub fn parse(text: &str) -> Option<Timer> {
        let mut words = text.split_whitespace();
        let end = words.next()?.parse().ok()?;
        let status = words.next()?.to_string();
        Some(Timer { end, status })
    }

    /// Minutes left, rounded up.
    pub fn minutes_left(&self) -> i64 {
        ((self.end - now()).max(0) + 59) / 60
    }
}

pub fn timer() -> Option<Timer> {
    Timer::parse(&std::fs::read_to_string(paths::timer()).ok()?)
}

pub fn timer_token() -> Option<String> {
    std::fs::read_to_string(paths::timer())
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn set_timer(timer: &Timer) -> std::io::Result<()> {
    let path = paths::timer();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{}\n", timer.token()))
}

pub fn clear_timer() {
    let _ = std::fs::remove_file(paths::timer());
}

/// A duration in seconds: 30 (minutes), 30m, 45min, 1h, 1h30, 1h30m —
/// from 1 minute to 24 hours.
pub fn parse_duration(text: &str) -> Option<u64> {
    let d: String = text
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let number = |s: &str| -> Option<u64> {
        (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let strip_minutes = |s: &str| -> String {
        s.strip_suffix("min")
            .or_else(|| s.strip_suffix('m'))
            .unwrap_or(s)
            .to_string()
    };
    let minutes = if let Some((h, m)) = d.split_once('h') {
        let h = number(h)?;
        if m.is_empty() {
            h * 60
        } else {
            h * 60 + number(&strip_minutes(m))?
        }
    } else {
        number(&strip_minutes(&d))?
    };
    (1..=1440).contains(&minutes).then_some(minutes * 60)
}

/// "25 min", "1 h", "1 h 30".
pub fn human_minutes(minutes: i64) -> String {
    if minutes < 60 {
        trf("{minutes} min", &[("minutes", &minutes)])
    } else if minutes % 60 == 0 {
        trf("{hours} h", &[("hours", &(minutes / 60))])
    } else {
        trf(
            "{hours} h {minutes}",
            &[
                ("hours", &(minutes / 60)),
                ("minutes", &format!("{:02}", minutes % 60)),
            ],
        )
    }
}

/// Local time "HH:MM" of an epoch.
pub fn clock(epoch: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|t| t.format("%H:%M").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30"), Some(1800));
        assert_eq!(parse_duration("45m"), Some(2700));
        assert_eq!(parse_duration("45min"), Some(2700));
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("1H30"), Some(5400));
        assert_eq!(parse_duration("1h30m"), Some(5400));
        assert_eq!(parse_duration("24h"), Some(86400));
        assert_eq!(parse_duration("24h1"), None);
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration("soon"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("h"), None);
    }

    #[test]
    fn timer_tokens() {
        let t = Timer::parse("1700000000 busy\n").unwrap();
        assert_eq!(
            t,
            Timer {
                end: 1700000000,
                status: "busy".into()
            }
        );
        assert_eq!(t.token(), "1700000000 busy");
        assert_eq!(Timer::parse("x busy"), None);
    }
}
