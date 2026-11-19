//! The light itself: Luxafor's USB HID protocol.
//!
//! Static color: 01 FF R G B 00 00 00 (01 = static color, FF = all LEDs).
//! Blinking: 03 FF R G B <speed> 00 00 (03 = strobe, repeat 0 = forever:
//! the light blinks by itself until the next command).
//!
//! Linux: written straight to the light's /dev/hidraw* (the udev rule gives
//! the logged-in user access), found through sysfs — no library needed.
//! Windows: hidapi.

use crate::color::Rgb;
use crate::config::Effect;
use crate::i18n::{tr, trf};

/// Strobe periods for blink-slow / blink / blink-fast (lower is faster).
pub const BLINK_SLOW: u8 = 180;
pub const BLINK_NORMAL: u8 = 90;
pub const BLINK_FAST: u8 = 30;

#[derive(Debug)]
pub enum Error {
    /// No light plugged in.
    NoLight,
    /// Found, but couldn't be written to: the messages.
    Write(Vec<String>),
}

impl Error {
    pub fn message(&self) -> String {
        match self {
            Error::NoLight => tr("no Luxafor light found — is it plugged in?"),
            Error::Write(msgs) => msgs.join("\n"),
        }
    }

    /// v1's exit codes: 3 no light, 1 cannot write.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::NoLight => 3,
            Error::Write(_) => 1,
        }
    }
}

/// The 8-byte command for a color and an effect.
pub fn command(rgb: Rgb, effect: Effect) -> [u8; 8] {
    let [r, g, b] = rgb;
    let speed = match effect {
        Effect::Steady => return [1, 0xFF, r, g, b, 0, 0, 0],
        Effect::BlinkSlow => BLINK_SLOW,
        Effect::Blink => BLINK_NORMAL,
        Effect::BlinkFast => BLINK_FAST,
    };
    // LUXAFOR_BLINK_SPEED (1–255): to try other speeds.
    let speed = std::env::var("LUXAFOR_BLINK_SPEED")
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(speed);
    [3, 0xFF, r, g, b, speed, 0, 0]
}

pub const OFF: [u8; 8] = [1, 0xFF, 0, 0, 0, 0, 0, 0];

/// A connected light: its name, and where to write.
#[derive(Clone, Debug)]
pub struct Light {
    pub product: String,
    pub path: String,
}

/// USB vendor:product IDs to look for (LUXAFOR_IDS overrides).
fn ids() -> Vec<(u16, u16)> {
    std::env::var("LUXAFOR_IDS")
        .unwrap_or_else(|_| "04d8:f372".into())
        .split_whitespace()
        .filter_map(|id| {
            let (v, p) = id.split_once(':')?;
            Some((
                u16::from_str_radix(v, 16).ok()?,
                u16::from_str_radix(p, 16).ok()?,
            ))
        })
        .collect()
}

#[cfg(not(windows))]
pub fn find() -> Vec<Light> {
    let sysfs = std::env::var("LUXAFOR_SYSFS").unwrap_or_else(|_| "/sys".into());
    let dev = std::env::var("LUXAFOR_DEV").unwrap_or_else(|_| "/dev".into());
    let wanted = ids();
    let Ok(entries) = std::fs::read_dir(format!("{sysfs}/class/hidraw")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort_by_key(|n| (n.len(), n.clone()));
    let mut lights = Vec::new();
    for name in names {
        let Ok(uevent) =
            std::fs::read_to_string(format!("{sysfs}/class/hidraw/{name}/device/uevent"))
        else {
            continue;
        };
        let field = |key: &str| {
            uevent
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .map(|v| v.trim().to_string())
        };
        // HID_ID=0003:000004D8:0000F372
        let Some(hid_id) = field("HID_ID=") else {
            continue;
        };
        let parts: Vec<&str> = hid_id.split(':').collect();
        if parts.len() != 3 {
            continue;
        }
        let (Ok(vid), Ok(pid)) = (
            u32::from_str_radix(parts[1], 16),
            u32::from_str_radix(parts[2], 16),
        ) else {
            continue;
        };
        if wanted
            .iter()
            .any(|(v, p)| *v as u32 == vid && *p as u32 == pid)
        {
            lights.push(Light {
                product: field("HID_NAME=")
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "Luxafor".into()),
                path: format!("{dev}/{name}"),
            });
        }
    }
    lights
}

#[cfg(windows)]
pub fn find() -> Vec<Light> {
    let Ok(api) = hidapi::HidApi::new() else {
        return Vec::new();
    };
    let wanted = ids();
    api.device_list()
        .filter(|d| wanted.contains(&(d.vendor_id(), d.product_id())))
        .map(|d| Light {
            product: d.product_string().unwrap_or("Luxafor").to_string(),
            path: d.path().to_string_lossy().into_owned(),
        })
        .collect()
}

/// True if a light is plugged in.
pub fn connected() -> bool {
    !find().is_empty()
}

#[cfg(not(windows))]
fn write(light: &Light, report: &[u8; 9]) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&light.path)
        .and_then(|mut f| f.write_all(report))
        .map_err(|_| {
            trf(
                "cannot write to {path} (permission denied? replug the light)",
                &[("path", &light.path)],
            )
        })
}

#[cfg(windows)]
fn write(light: &Light, report: &[u8; 9]) -> Result<(), String> {
    let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
    let path = std::ffi::CString::new(light.path.clone()).map_err(|e| e.to_string())?;
    let dev = api
        .open_path(&path)
        .map_err(|e| trf("cannot open the light: {error}", &[("error", &e)]))?;
    dev.write(report)
        .map(|_| ())
        .map_err(|e| trf("cannot open the light: {error}", &[("error", &e)]))
}

/// Sends a command to every light.
pub fn send(command: [u8; 8]) -> Result<(), Error> {
    let lights = find();
    if lights.is_empty() {
        return Err(Error::NoLight);
    }
    // hidraw expects the report ID first; Luxafor lights don't use numbered
    // reports, so it is 0.
    let mut report = [0u8; 9];
    report[1..].copy_from_slice(&command);
    let errors: Vec<String> = lights
        .iter()
        .filter_map(|l| write(l, &report).err())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::Write(errors))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands() {
        assert_eq!(
            command([255, 0, 0], Effect::Steady),
            [1, 0xFF, 255, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            command([255, 0, 0], Effect::Blink),
            [3, 0xFF, 255, 0, 0, 90, 0, 0]
        );
        assert_eq!(
            command([0, 0, 255], Effect::BlinkSlow),
            [3, 0xFF, 0, 0, 255, 180, 0, 0]
        );
        assert_eq!(command([0, 0, 255], Effect::BlinkFast)[5], 30);
    }
}
