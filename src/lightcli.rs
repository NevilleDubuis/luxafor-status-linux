//! `luxafor-light`: the light alone, without statuses — same as v1.

use crate::config::Effect;
use crate::i18n::trf;
use crate::light;

const HELP: &str = "\
luxafor-light — drive Luxafor USB lights directly.

  luxafor-light <color>     set a color: name (red, orange…) or hex (#ff8800)
  luxafor-light <color> blink [slow|fast]   make the light blink
  luxafor-light off         turn the light off
  luxafor-light list        list connected Luxafor lights

Exit status: 0 ok, 1 cannot write to the light, 2 unknown color or effect,
3 no light.";

fn fail(message: &str, code: i32) -> i32 {
    eprintln!("luxafor-light: {message}");
    code
}

fn sent(result: Result<(), light::Error>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => fail(&e.message(), e.exit_code()),
    }
}

pub fn main(args: Vec<String>) -> i32 {
    let first = args.first().map(String::as_str).unwrap_or("");
    match first {
        "" | "-h" | "--help" => {
            println!("{HELP}");
            if first.is_empty() {
                1
            } else {
                0
            }
        }
        "list" => {
            let lights = light::find();
            if lights.is_empty() {
                eprintln!("No Luxafor light found.");
                return 3;
            }
            for (i, l) in lights.iter().enumerate() {
                println!("{i}  {}  ({})", l.product, l.path);
            }
            0
        }
        "off" => sent(light::send(light::OFF)),
        color => {
            let Some(rgb) = crate::color::parse(color) else {
                return fail(
                    &trf(
                        "unknown color '{color}' — use a name (red, orange…) or a hex code (#ff8800)",
                        &[("color", &color)],
                    ),
                    2,
                );
            };
            let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
            let effect = match rest.as_slice() {
                [] | ["steady"] | ["none"] => Effect::Steady,
                ["blink"] | ["blink", "normal"] | ["blink-normal"] => Effect::Blink,
                ["blink", "slow"] | ["blink-slow"] => Effect::BlinkSlow,
                ["blink", "fast"] | ["blink-fast"] => Effect::BlinkFast,
                _ => {
                    return fail(
                        &trf(
                            "unknown effect '{effect}' — use: blink, blink slow, blink fast",
                            &[("effect", &rest.join(" "))],
                        ),
                        2,
                    )
                }
            };
            let brightness = std::env::var("LUXAFOR_BRIGHTNESS")
                .ok()
                .and_then(|b| b.parse().ok())
                .unwrap_or(100);
            sent(light::send(light::command(
                crate::color::scale(rgb, brightness),
                effect,
            )))
        }
    }
}
