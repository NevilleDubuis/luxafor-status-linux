//! Linux desktop services for the tray: screen lock and do not disturb.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

/// The desktop's "do not disturb" switch, wherever this desktop keeps it.
///
/// Budgie and other desktops keep a boolean "…do-not-disturb…" key in
/// their own GSettings schema; GNOME has org.gnome.desktop.notifications
/// show-banners (false = do not disturb). Found once, at startup.
pub struct Dnd {
    settings: Option<(gio::Settings, String, bool)>,
}

impl Dnd {
    const CANDIDATES: [&'static str; 7] = [
        "budgie",
        "raven",
        "cinnamon",
        "mate",
        "xfce",
        "pantheon",
        "notifications",
    ];

    pub fn find() -> Dnd {
        let Some(source) = gio::SettingsSchemaSource::default() else {
            return Dnd { settings: None };
        };
        let (mut schemas, _) = source.list_schemas(true);
        schemas.sort();
        for id in schemas.iter().map(|s| s.as_str()) {
            if !Self::CANDIDATES
                .iter()
                .any(|c| id.to_lowercase().contains(c))
            {
                continue;
            }
            let Some(schema) = source.lookup(id, true) else {
                continue;
            };
            for key in schema.list_keys() {
                let key = key.as_str();
                if (key.contains("disturb") || key == "dnd")
                    && schema.key(key).value_type().as_str() == "b"
                {
                    return Dnd {
                        settings: Some((gio::Settings::new(id), key.to_string(), false)),
                    };
                }
            }
        }
        if let Some(schema) = source.lookup("org.gnome.desktop.notifications", true) {
            if schema.has_key("show-banners") {
                let s = gio::Settings::new("org.gnome.desktop.notifications");
                return Dnd {
                    settings: Some((s, "show-banners".into(), true)),
                };
            }
        }
        Dnd { settings: None }
    }

    pub fn available(&self) -> bool {
        self.settings.is_some()
    }

    pub fn get(&self) -> bool {
        self.settings
            .as_ref()
            .is_some_and(|(s, key, inverted)| s.boolean(key) != *inverted)
    }

    pub fn set(&self, on: bool) {
        if let Some((s, key, inverted)) = &self.settings {
            let _ = s.set_boolean(key, on != *inverted);
            gio::Settings::sync();
        }
    }
}

/// Calls `on_change(locked)` when the screen is locked or unlocked.
///
/// Listens to the screensaver interfaces of the usual desktops (Budgie and
/// GNOME: org.gnome.ScreenSaver; KDE…: org.freedesktop.ScreenSaver;
/// Cinnamon, MATE) and to logind's LockedHint, whichever speaks first.
pub fn watch_screen_lock(on_change: impl Fn(bool) + 'static) {
    let locked = Rc::new(RefCell::new(false));
    let on_change = Rc::new(on_change);
    let set = {
        let locked = locked.clone();
        Rc::new(move |value: bool| {
            if *locked.borrow() != value {
                *locked.borrow_mut() = value;
                on_change(value);
            }
        })
    };
    if let Ok(session) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
        for iface in [
            "org.gnome.ScreenSaver",
            "org.freedesktop.ScreenSaver",
            "org.cinnamon.ScreenSaver",
            "org.mate.ScreenSaver",
        ] {
            let set = set.clone();
            session.signal_subscribe(
                None,
                Some(iface),
                Some("ActiveChanged"),
                None,
                None,
                gio::DBusSignalFlags::NONE,
                move |_c, _sender, _path, _iface, _signal, params| {
                    if let Some((active,)) = params.get::<(bool,)>() {
                        set(active);
                    }
                },
            );
        }
    }
    // logind: no session in tests and containers — screensaver signals only.
    let Ok(system) = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) else {
        return;
    };
    let Ok(reply) = system.call_sync(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        "GetSessionByPID",
        Some(&(std::process::id(),).to_variant()),
        Some(glib::VariantTy::new("(o)").unwrap()),
        gio::DBusCallFlags::NONE,
        2000,
        gio::Cancellable::NONE,
    ) else {
        return;
    };
    let Some(path) = reply.child_value(0).str().map(String::from) else {
        return;
    };
    system.signal_subscribe(
        Some("org.freedesktop.login1"),
        Some("org.freedesktop.DBus.Properties"),
        Some("PropertiesChanged"),
        Some(&path),
        None,
        gio::DBusSignalFlags::NONE,
        move |_c, _sender, _path, _iface, _signal, params| {
            if params.child_value(0).str() != Some("org.freedesktop.login1.Session") {
                return;
            }
            let changed = glib::VariantDict::new(Some(&params.child_value(1)));
            if let Some(hint) = changed
                .lookup_value("LockedHint", None)
                .and_then(|v| v.get::<bool>())
            {
                set(hint);
            }
        },
    );
}
