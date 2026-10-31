// GRANTS: player_state, hud
// NOTE: Client-side HUD only. This is an OptiFine-style overlay that shows FPS (current, average, min, max),
// player coordinates, facing direction, nearby entity count, and a zoom-style indicator label.
// It cannot change real render settings, add real zoom, or alter the game. It only reads state and
// displays it in the HUD label.

use mod_api::bindings::{cinnabar::extension::{gameplay::Vector3, hud, player_state, render::{self, Billboard, BillboardPattern, Primitives, Rgba}}, Guest};
use std::cell::RefCell;

struct State {
    time: f32,
    frames: u32,
    fps_sum: f32,
    fps_min: f32,
    fps_max: f32,
    fps_now: f32,
    last_update: f32,
    last_pos: Option<(f32, f32, f32)>,
    speed: f32,
}

impl Default for State {
    fn default() -> Self {
        State {
            time: 0.0,
            frames: 0,
            fps_sum: 0.0,
            fps_min: f32::MAX,
            fps_max: 0.0,
            fps_now: 0.0,
            last_update: 0.0,
            last_pos: None,
            speed: 0.0,
        }
    }
}

thread_local! {
    static ST: RefCell<State> = RefCell::new(State::default());
}

fn compass(yaw: f32) -> &'static str {
    let mut y = yaw % 360.0;
    if y < 0.0 { y += 360.0; }
    // Bedrock yaw: 0 = south, 90 = west, 180 = north, 270 = east
    match ((y + 22.5) / 45.0) as u32 % 8 {
        0 => "S",
        1 => "SW",
        2 => "W",
        3 => "NW",
        4 => "N",
        5 => "NE",
        6 => "E",
        _ => "SE",
    }
}

fn bar(fps: f32) -> &'static str {
    if fps >= 120.0 { "||||||||||" }
    else if fps >= 90.0 { "||||||||| " }
    else if fps >= 60.0 { "||||||||  " }
    else if fps >= 40.0 { "|||||     " }
    else if fps >= 20.0 { "|||       " }
    else { "|         " }
}

struct M;
impl Guest for M {
    fn init() {
        let _ = hud::set_label("OptiFine HUD loading...");
    }

    fn frame() {
        let fps = player_state::read_fps().ok().flatten().unwrap_or(0.0);
        let dt = if fps > 1.0 { 1.0 / fps } else { 0.0 };

        let world = player_state::read_world().ok().flatten();
        let actors = player_state::read_actors().unwrap_or_default();

        ST.with(|s| {
            let mut s = s.borrow_mut();
            s.time += dt;
            s.fps_now = fps;
            if fps > 0.0 {
                s.frames += 1;
                s.fps_sum += fps;
                if fps < s.fps_min { s.fps_min = fps; }
                if fps > s.fps_max { s.fps_max = fps; }
            }

            // Update the label about 5 times per second to keep work small and text readable
            if s.time - s.last_update < 0.2 {
                return;
            }
            let elapsed = s.time - s.last_update;
            s.last_update = s.time;

            let mut pos_text = String::from("XYZ: n/a");
            let mut dir_text = String::from("Facing: n/a");
            if let Some(w) = world {
                let p = w.position;
                pos_text = format!("XYZ: {:.1} / {:.1} / {:.1}", p.0, p.1, p.2);

                if let Some(prev) = s.last_pos {
                    let dx = p.0 - prev.0;
                    let dz = p.2 - prev.2;
                    if elapsed > 0.0 {
                        s.speed = (dx * dx + dz * dz).sqrt() / elapsed;
                    }
                }
                s.last_pos = Some(p);

                // Use own yaw from the actor list if our runtime id appears there
                if let Some(me) = actors.iter().find(|a| a.runtime_id == w.runtime_id) {
                    dir_text = format!("Facing: {} ({:.0})", compass(me.yaw), me.yaw);
                }
            }

            let avg = if s.frames > 0 { s.fps_sum / s.frames as f32 } else { 0.0 };
            let min = if s.fps_min == f32::MAX { 0.0 } else { s.fps_min };

            let mobs = actors.iter().filter(|a| !a.player).count();
            let players = actors.iter().filter(|a| a.player).count();

            let text = format!(
                "[OptiFine] {:.0} fps [{}]\nAvg {:.0}  Min {:.0}  Max {:.0}\n{}\n{}\nSpeed: {:.1} b/s\nEntities: {} mobs, {} players",
                s.fps_now, bar(s.fps_now), avg, min, s.fps_max, pos_text, dir_text, s.speed, mobs, players
            );
            let _ = hud::set_label(&text);

            // Reset min/max window every ~10 seconds so stats stay relevant
            if s.time > 10.0 && s.frames > 600 {
                s.frames = 0;
                s.fps_sum = 0.0;
                s.fps_min = f32::MAX;
                s.fps_max = 0.0;
                s.time = 0.0;
                s.last_update = 0.0;
            }
        });

        // Draw nothing extra; clear any previous drawing
        let _ = render::draw(&Primitives { decals: vec![], ribbons: vec![], beams: vec![], billboards: Vec::<Billboard>::new() });
    }
}
mod_api::bindings::export!(M with_types_in mod_api::bindings);
