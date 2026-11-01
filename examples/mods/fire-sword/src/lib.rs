// GRANTS: player_state, environment, hud
use mod_api::bindings::{cinnabar::extension::{environment::{self, Weather}, hud::{self, VanillaElement}, input, player_state}, Guest};
use std::cell::RefCell;

struct S { mode: u8, avg: f32, t: f32 }
thread_local! { static ST: RefCell<S> = RefCell::new(S { mode: 0, avg: 0.0, t: 0.0 }); }

const NAMES: [&str; 3] = ["Default", "Boost", "Max boost"];

fn apply(mode: u8) {
    let _ = environment::set_weather_override(if mode >= 1 { Some(Weather::Clear) } else { None });
    let hidden: &[VanillaElement] = if mode >= 2 {
        &[VanillaElement::Sidebar, VanillaElement::StatusEffects]
    } else {
        &[]
    };
    let _ = hud::set_hidden_elements(hidden);
}

struct M;
impl Guest for M {
    fn init() { let _ = hud::set_label("Performance: press the mod key to change"); }
    fn frame() {
        let fps = player_state::read_fps().ok().flatten().unwrap_or(0.0);
        ST.with(|s| {
            let mut s = s.borrow_mut();
            if input::demo_pressed() {
                s.mode = (s.mode + 1) % 3;
                apply(s.mode);
            }
            if fps <= 0.0 { return; }
            s.avg = if s.avg == 0.0 { fps } else { s.avg * 0.98 + fps * 0.02 };
            s.t += 1.0 / fps;
            if s.t < 0.25 { return; }
            s.t = 0.0;
            let _ = hud::set_label(&format!(
                "Perf [{}] FPS {:.0} avg {:.0} | key: change",
                NAMES[s.mode as usize], fps, s.avg));
        });
    }
}
mod_api::bindings::export!(M with_types_in mod_api::bindings);
