//! What the Settings tab edits, apart from the widgets: the rows, the
//! choices of the combo boxes, and what Save writes.

use crate::color::{self, Rgb};
use crate::config::{self, Config, Effect, Settings, Status};
use crate::i18n::tr;

pub const MAX_SHORTCUTS: usize = 6;

/// One editable status. `id` is None for a status added in the panel (it
/// gets an id from its name when saved).
#[derive(Clone, Debug)]
pub struct Row {
    pub key: String,
    pub id: Option<String>,
    pub color: Rgb,
    /// The color as written in the file, kept if the color isn't changed.
    pub original_color: String,
    pub label: String,
    pub effect: Effect,
    pub error: bool,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub rows: Vec<Row>,
    /// Combo boxes hold row keys (the id, or "new:N"), or "none" / "off".
    pub default_key: String,
    pub call_key: String,
    pub lock_key: String,
    pub dnd: Vec<String>,
    pub shortcuts: Vec<(String, u32)>,
    pub brightness: u32,
    pub theme: String,
    next_new: u32,
}

pub fn dont_change() -> (String, String) {
    ("none".into(), tr("Don't change"))
}

pub fn light_off() -> (String, String) {
    ("off".into(), tr("Light off"))
}

impl Model {
    pub fn from_config(c: &Config) -> Model {
        let rows = c
            .statuses
            .iter()
            .map(|s| Row {
                key: s.id.clone(),
                id: Some(s.id.clone()),
                color: color::display(&s.color),
                original_color: s.color.clone(),
                label: s.label.clone(),
                effect: s.effect,
                error: false,
            })
            .collect();
        let mut m = Model {
            rows,
            default_key: c.default_status.clone(),
            call_key: c.call_status.clone(),
            lock_key: c.lock_status.clone(),
            dnd: c.dnd_statuses.clone(),
            shortcuts: c.shortcuts.clone(),
            brightness: c.brightness,
            theme: c.theme.clone(),
            next_new: 1,
        };
        m.fix_choices();
        m
    }

    /// (key, label) of the statuses, for the combo boxes.
    pub fn choices(&self) -> Vec<(String, String)> {
        self.rows
            .iter()
            .map(|r| {
                let label = r.label.split_whitespace().collect::<Vec<_>>().join(" ");
                (
                    r.key.clone(),
                    if label.is_empty() {
                        tr("(no name)")
                    } else {
                        label
                    },
                )
            })
            .collect()
    }

    pub fn default_choices(&self) -> Vec<(String, String)> {
        self.choices()
    }

    pub fn call_choices(&self) -> Vec<(String, String)> {
        let mut c = self.choices();
        c.push(dont_change());
        c
    }

    pub fn lock_choices(&self) -> Vec<(String, String)> {
        let mut c = vec![light_off()];
        c.extend(self.choices());
        c.push(dont_change());
        c
    }

    /// After a status is deleted: choices pointing to it fall back to the
    /// first one available.
    pub fn fix_choices(&mut self) {
        let fix = |key: &mut String, choices: Vec<(String, String)>| {
            if !choices.iter().any(|(k, _)| k == key) {
                *key = choices.first().map(|(k, _)| k.clone()).unwrap_or_default();
            }
        };
        let (d, c, l) = (
            self.default_choices(),
            self.call_choices(),
            self.lock_choices(),
        );
        fix(&mut self.default_key, d);
        fix(&mut self.call_key, c);
        fix(&mut self.lock_key, l);
        let keys: Vec<String> = self.rows.iter().map(|r| r.key.clone()).collect();
        self.dnd.retain(|k| keys.contains(k));
        for (key, _) in self.shortcuts.iter_mut() {
            if !keys.contains(key) {
                *key = keys.first().cloned().unwrap_or_default();
            }
        }
    }

    pub fn add_status(&mut self) {
        let key = format!("new:{}", self.next_new);
        self.next_new += 1;
        self.rows.push(Row {
            key,
            id: None,
            color: [255, 255, 255],
            original_color: "#ffffff".into(),
            label: String::new(),
            effect: Effect::Steady,
            error: false,
        });
    }

    pub fn move_row(&mut self, index: usize, step: isize) {
        let to = index as isize + step;
        if to >= 0 && (to as usize) < self.rows.len() {
            self.rows.swap(index, to as usize);
        }
    }

    /// Err: the message to show (the last status can't be deleted).
    pub fn remove_row(&mut self, index: usize) -> Result<(), String> {
        if self.rows.len() <= 1 {
            return Err(tr("Keep at least one status."));
        }
        self.rows.remove(index);
        self.fix_choices();
        Ok(())
    }

    pub fn add_shortcut(&mut self) {
        if self.shortcuts.len() < MAX_SHORTCUTS {
            let key = self.rows.first().map(|r| r.key.clone()).unwrap_or_default();
            self.shortcuts.push((key, 30));
        }
    }

    /// What Save writes, or the message to show.
    pub fn collect(&mut self) -> Result<Settings, String> {
        let mut empty = false;
        for r in &mut self.rows {
            r.error = r.label.trim().is_empty();
            empty |= r.error;
        }
        if empty {
            return Err(tr("Give every status a name."));
        }
        let mut taken: Vec<String> = self.rows.iter().filter_map(|r| r.id.clone()).collect();
        let mut ids = Vec::new();
        let mut statuses = Vec::new();
        for r in &self.rows {
            let label = r.label.split_whitespace().collect::<Vec<_>>().join(" ");
            let id = match &r.id {
                Some(id) => id.clone(),
                None => {
                    let id = config::make_id(&label, &taken);
                    taken.push(id.clone());
                    id
                }
            };
            let color = if color::parse(&r.original_color) == Some(r.color) {
                r.original_color.clone()
            } else {
                color::to_hex(r.color)
            };
            ids.push((r.key.clone(), id.clone()));
            statuses.push(Status {
                id,
                color,
                label,
                effect: r.effect,
            });
        }
        let id_of = |key: &str| ids.iter().find(|(k, _)| k == key).map(|(_, id)| id.clone());
        Ok(Settings {
            default_status: id_of(&self.default_key).unwrap_or_else(|| statuses[0].id.clone()),
            call_status: id_of(&self.call_key).unwrap_or_else(|| "none".into()),
            lock_status: id_of(&self.lock_key).unwrap_or_else(|| {
                if self.lock_key == "none" {
                    "none".into()
                } else {
                    "off".into()
                }
            }),
            dnd_statuses: self
                .rows
                .iter()
                .filter(|r| self.dnd.contains(&r.key))
                .filter_map(|r| id_of(&r.key))
                .collect(),
            shortcuts: self
                .shortcuts
                .iter()
                .filter_map(|(k, m)| id_of(k).map(|id| (id, *m)))
                .collect(),
            brightness: self.brightness,
            theme: self.theme.clone(),
            statuses,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::parse(
            "available | green | Available\nbusy | red | Busy\ncall | yellow | On a call\n\
             focus | #0000ff | Focus — do not disturb | blink-slow\ndefault-status = available\n\
             timer-shortcuts = focus 1h30, busy 30, nope 10\n",
            None,
            true,
            false,
        )
        .unwrap()
    }

    #[test]
    fn loads() {
        let m = Model::from_config(&config());
        assert_eq!(m.rows.len(), 4);
        assert_eq!(m.rows[3].effect, Effect::BlinkSlow);
        assert_eq!(m.call_key, "call");
        assert_eq!(m.lock_key, "off");
        assert_eq!(
            m.shortcuts,
            [("focus".to_string(), 90), ("busy".to_string(), 30)]
        );
    }

    #[test]
    fn edits_and_saves() {
        let mut m = Model::from_config(&config());
        m.rows[1].label = "Occupé".into();
        m.rows[1].effect = Effect::BlinkFast;
        m.rows[0].color = [0x12, 0x34, 0x56];
        m.move_row(3, -1);
        m.add_status();
        m.rows[4].label = "Pause café".into();
        let new_key = m.rows[4].key.clone();
        m.call_key = "call".into();
        m.remove_row(3).unwrap(); // "On a call"
        assert_eq!(m.call_key, "available", "deleted status → falls back");
        m.shortcuts.push((new_key.clone(), 15));
        m.dnd = vec!["busy".into(), new_key];
        m.lock_key = "none".into();
        let s = m.collect().unwrap();
        let ids: Vec<_> = s.statuses.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["available", "busy", "focus", "pause-cafe"]);
        assert_eq!(s.statuses[0].color, "#123456");
        assert_eq!(
            s.statuses[1].color, "red",
            "unchanged color kept as written"
        );
        assert_eq!(s.statuses[1].effect, Effect::BlinkFast);
        assert_eq!(s.call_status, "available");
        assert_eq!(s.lock_status, "none");
        assert_eq!(s.dnd_statuses, ["busy", "pause-cafe"]);
        assert_eq!(s.shortcuts.last().unwrap(), &("pause-cafe".to_string(), 15));
    }

    #[test]
    fn checks() {
        let mut m = Model::from_config(&config());
        m.rows[0].label = "   ".into();
        assert!(m.collect().is_err());
        assert!(m.rows[0].error);
        while m.rows.len() > 1 {
            m.remove_row(0).unwrap();
        }
        assert!(m.remove_row(0).is_err());
        for _ in 0..10 {
            m.add_shortcut();
        }
        assert_eq!(m.shortcuts.len(), MAX_SHORTCUTS);
    }
}
