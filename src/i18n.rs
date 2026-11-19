//! Translations: the po/*.po files are built into the program.
//!
//! `tr("text")` gives the text in the user's language; `trf("… {name} …",
//! &[("name", value)])` also fills in `{name}` placeholders. Messages are
//! extracted with po/update-pot.sh (calls to `tr(` and `trf(`).

use std::collections::HashMap;
use std::sync::OnceLock;

/// The translations shipped with the program: (language, .po file).
const CATALOGS: &[(&str, &str)] = &[("fr", include_str!("../po/fr.po"))];

static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();

fn catalog() -> &'static HashMap<String, String> {
    CATALOG.get_or_init(|| {
        let Some(lang) = language() else {
            return HashMap::new();
        };
        CATALOGS
            .iter()
            .find(|(l, _)| *l == lang)
            .map(|(_, po)| parse_po(po))
            .unwrap_or_default()
    })
}

/// The user's language among the shipped ones, like gettext: LANGUAGE
/// (a list), unless the locale is C/POSIX, then LC_ALL, LC_MESSAGES, LANG.
pub fn language() -> Option<&'static str> {
    let shipped = |code: &str| {
        let code = code.split(['_', '.', '@']).next().unwrap_or("");
        CATALOGS.iter().map(|(l, _)| *l).find(|l| *l == code)
    };
    if let Some(lang) = std::env::var("LUXAFOR_LANG").ok().filter(|v| !v.is_empty()) {
        return shipped(&lang);
    }
    #[cfg(windows)]
    {
        return windows_language().as_deref().and_then(shipped);
    }
    #[allow(unreachable_code)]
    {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        let base = locale.split(['.', '@']).next().unwrap_or("");
        if locale.is_empty() || base == "C" || base == "POSIX" {
            return None;
        }
        if let Ok(list) = std::env::var("LANGUAGE") {
            for code in list.split(':').filter(|c| !c.is_empty()) {
                if let Some(l) = shipped(code) {
                    return Some(l);
                }
                if code.starts_with("en") {
                    return None;
                }
            }
        }
        shipped(&locale)
    }
}

#[cfg(windows)]
fn windows_language() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Control Panel\International")
        .ok()?;
    key.get_value::<String, _>("LocaleName").ok()
}

/// The text in the user's language.
pub fn tr(msgid: &str) -> String {
    match catalog().get(msgid) {
        Some(s) if !s.is_empty() => s.clone(),
        _ => msgid.to_string(),
    }
}

/// The text in the user's language, with `{name}` placeholders filled in.
pub fn trf(msgid: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut text = tr(msgid);
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// Reads the translated messages of a .po file (fuzzy ones left out).
fn parse_po(po: &str) -> HashMap<String, String> {
    #[derive(PartialEq)]
    enum In {
        Nothing,
        Id,
        Str,
    }
    let mut map = HashMap::new();
    let (mut id, mut text, mut fuzzy, mut next_fuzzy) =
        (String::new(), String::new(), false, false);
    let mut state = In::Nothing;
    let mut save = |id: &mut String, text: &mut String, fuzzy: bool| {
        if !id.is_empty() && !text.is_empty() && !fuzzy {
            map.insert(std::mem::take(id), std::mem::take(text));
        }
        id.clear();
        text.clear();
    };
    for line in po.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("msgid ") {
            if state == In::Str {
                save(&mut id, &mut text, fuzzy);
            }
            fuzzy = std::mem::take(&mut next_fuzzy);
            id = unquote(rest);
            state = In::Id;
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            text = unquote(rest);
            state = In::Str;
        } else if line.starts_with('"') {
            match state {
                In::Id => id.push_str(&unquote(line)),
                In::Str => text.push_str(&unquote(line)),
                In::Nothing => {}
            }
        } else if line.starts_with("#,") && line.contains("fuzzy") {
            next_fuzzy = true;
        } else if !line.is_empty() && !line.starts_with('#') {
            state = In::Nothing; // msgctxt, plurals: not used
        }
    }
    if state == In::Str {
        save(&mut id, &mut text, fuzzy);
    }
    map
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s);
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn po_parsing() {
        let po = r#"
msgid ""
msgstr "Content-Type: text/plain; charset=UTF-8\n"

msgid "Busy"
msgstr "Occupé"

#, fuzzy
msgid "Maybe"
msgstr "Peut-être"

msgid "Long "
"text"
msgstr "Texte "
"long \"cité\""

msgid "Untranslated"
msgstr ""
"#;
        let map = parse_po(po);
        assert_eq!(map.get("Busy").map(String::as_str), Some("Occupé"));
        assert_eq!(map.get("Maybe"), None);
        assert_eq!(
            map.get("Long text").map(String::as_str),
            Some("Texte long \"cité\"")
        );
        assert_eq!(map.get("Untranslated"), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn french_catalog_loads() {
        let map = parse_po(CATALOGS[0].1);
        assert!(map.len() > 50, "fr.po has {} messages", map.len());
    }
}
