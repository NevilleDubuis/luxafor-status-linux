//! Colors: CSS names and hex codes, as in statuses.conf.

pub type Rgb = [u8; 3];

/// The gray used for "off" in icons and the panel.
pub const OFF_COLOR: &str = "#9e9e9e";

/// CSS color names (same values as the CSS standard).
const NAMES: &[(&str, u32)] = &[
    ("black", 0x000000),
    ("white", 0xffffff),
    ("gray", 0x808080),
    ("grey", 0x808080),
    ("silver", 0xc0c0c0),
    ("red", 0xff0000),
    ("maroon", 0x800000),
    ("orange", 0xffa500),
    ("darkorange", 0xff8c00),
    ("gold", 0xffd700),
    ("yellow", 0xffff00),
    ("olive", 0x808000),
    ("lime", 0x00ff00),
    ("green", 0x008000),
    ("teal", 0x008080),
    ("cyan", 0x00ffff),
    ("aqua", 0x00ffff),
    ("blue", 0x0000ff),
    ("navy", 0x000080),
    ("purple", 0x800080),
    ("magenta", 0xff00ff),
    ("fuchsia", 0xff00ff),
    ("violet", 0xee82ee),
    ("pink", 0xffc0cb),
    ("hotpink", 0xff69b4),
    ("deeppink", 0xff1493),
    ("turquoise", 0x40e0d0),
    ("indigo", 0x4b0082),
    ("crimson", 0xdc143c),
    ("coral", 0xff7f50),
    ("tomato", 0xff6347),
    ("salmon", 0xfa8072),
    ("skyblue", 0x87ceeb),
    ("royalblue", 0x4169e1),
    ("springgreen", 0x00ff7f),
    ("chartreuse", 0x7fff00),
];

/// A color name, #rgb, #rrggbb or rrggbb (any case).
pub fn parse(text: &str) -> Option<Rgb> {
    let c = text.trim().to_lowercase();
    if let Some((_, v)) = NAMES.iter().find(|(n, _)| *n == c) {
        return Some([(v >> 16) as u8, (v >> 8) as u8, *v as u8]);
    }
    let hex = c.strip_prefix('#').unwrap_or(&c);
    if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok();
    match hex.len() {
        6 => Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ]),
        3 => Some([digit(0)? * 17, digit(1)? * 17, digit(2)? * 17]),
        _ => None,
    }
}

/// A color for display (icons, panel): unknown colors show as gray.
pub fn display(text: &str) -> Rgb {
    parse(text).unwrap_or_else(|| parse(OFF_COLOR).unwrap())
}

pub fn to_hex(rgb: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Brightness in percent (5–100), rounded like v1.
pub fn scale(rgb: Rgb, percent: u32) -> Rgb {
    if !(1..100).contains(&percent) {
        return rgb;
    }
    rgb.map(|c| ((c as u32 * percent + 50) / 100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        assert_eq!(parse("red"), Some([255, 0, 0]));
        assert_eq!(parse("Orange"), Some([255, 165, 0]));
        assert_eq!(parse("#FF8800"), Some([255, 136, 0]));
        assert_eq!(parse("f80"), Some([255, 136, 0]));
        assert_eq!(parse("#12345"), None);
        assert_eq!(parse("notacolor"), None);
        assert_eq!(parse("+12345"), None);
    }

    #[test]
    fn brightness() {
        assert_eq!(scale([0, 0, 255], 40), [0, 0, 102]);
        assert_eq!(scale([255, 0, 0], 50), [128, 0, 0]);
        assert_eq!(scale([1, 2, 3], 100), [1, 2, 3]);
    }
}
