//! Linux desktop integration: status icons for notifications, and the dock
//! launcher with one right-click action per status (`luxafor-toggle --reload`).

use crate::config::Config;

/// How the launcher runs this program: "/usr/bin/luxafor-status toggle".
#[cfg(not(windows))]
fn exec(sub: &str) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "luxafor-status".into());
    let exe = if exe.contains(' ') {
        format!("\"{exe}\"")
    } else {
        exe
    };
    format!("{exe} {sub}")
}

#[cfg(not(windows))]
pub fn reload(config: &Config) {
    use crate::i18n::tr;
    use crate::paths;

    let icons = paths::icon_dir();
    let desktop = paths::desktop_file();
    let _ = std::fs::create_dir_all(&icons);
    if let Ok(entries) = std::fs::read_dir(&icons) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("luxafor-") && name.ends_with(".svg") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let svg = |color: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><circle cx=\"32\" cy=\"32\" r=\"26\" fill=\"{}\"/></svg>\n",
            crate::color::to_hex(crate::color::display(color))
        )
    };
    for s in &config.statuses {
        let _ = std::fs::write(icons.join(format!("luxafor-{}.svg", s.id)), svg(&s.color));
    }
    let _ = std::fs::write(icons.join("luxafor-off.svg"), svg(crate::color::OFF_COLOR));

    let escape = |s: &str| s.replace('\\', "\\\\");
    let mut text = String::new();
    text += "[Desktop Entry]\nType=Application\nName=Luxafor\n";
    text += &format!("Comment={}\n", tr("Change the Luxafor light status"));
    text += &format!("Exec={}\n", exec("toggle"));
    text += "Icon=luxafor-status\nTerminal=false\nCategories=Utility;\nKeywords=luxafor;busylight;status;\n";
    let actions: String = config
        .statuses
        .iter()
        .map(|s| format!("{};", s.id))
        .collect();
    text += &format!("Actions=panel;{actions}off;\n");
    text += &format!(
        "\n[Desktop Action panel]\nName={}\nExec={}\n",
        tr("Open panel…"),
        exec("panel")
    );
    for s in &config.statuses {
        text += &format!(
            "\n[Desktop Action {}]\nName={}\nExec={}\n",
            s.id,
            escape(&s.label),
            exec(&format!("toggle {}", s.id))
        );
    }
    text += &format!(
        "\n[Desktop Action off]\nName={}\nExec={}\n",
        tr("Off"),
        exec("toggle off")
    );
    if let Some(dir) = desktop.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&desktop, text);
    if let Some(dir) = desktop.parent() {
        let _ = std::process::Command::new("update-desktop-database")
            .arg(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[cfg(windows)]
pub fn reload(_config: &Config) {}
