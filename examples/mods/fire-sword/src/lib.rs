// GRANTS: player_state, entities, render, hud
// NOTE: Client-side visual only. No real item or real damage is added. The "pistol" is any held
// golden_hoe (stand-in item). Bullets are visual tracers fired in the direction of the closest
// actor in front of you (or straight ahead if none), since the API gives no click or look input.
// A shot fires automatically every ~0.4s while holding the pistol and a mob/player is in range.
// A muzzle flash and a hit spark are drawn. Nothing is actually damaged.
use mod_api::bindings::{cinnabar::extension::{gameplay::Vector3, hud, player_state, render::{self, Billboard, BillboardPattern, Primitives, Rgba}}, Guest};
use std::cell::RefCell;

struct Bullet { pos: (f32, f32, f32), dir: (f32, f32, f32), life: f32 }
struct Flash { pos: (f32, f32, f32), life: f32 }

struct State {
    bullets: Vec<Bullet>,
    flashes: Vec<Flash>,
    cooldown: f32,
    clock: f32,
}

thread_local! {
    static S: RefCell<State> = RefCell::new(State { bullets: vec![], flashes: vec![], cooldown: 0.0, clock: 0.0 });
}

fn holding_pistol() -> bool {
    let Ok(Some(eq)) = player_state::read_equipment() else { return false };
    let Some(slot) = eq.selected_slot else { return false };
    eq.inventory
        .get(slot as usize)
        .and_then(|s| s.item.as_ref())
        .and_then(|i| i.identifier.as_deref())
        == Some("minecraft:golden_hoe")
}

fn bb(p: (f32, f32, f32), w: f32, h: f32, c: (f32, f32, f32, f32), pat: BillboardPattern) -> Billboard {
    Billboard {
        position: Vector3 { x: p.0, y: p.1, z: p.2 },
        width: w,
        height: h,
        color: Rgba { r: c.0, g: c.1, b: c.2, a: c.3 },
        pattern: pat,
        upright: false,
    }
}

struct M;
impl Guest for M {
    fn init() {
        let _ = hud::set_label("Pistol: hold a golden hoe");
    }
    fn frame() {
        let fps = player_state::read_fps().ok().flatten().unwrap_or(60.0).max(1.0);
        let dt = 1.0 / fps;
        let armed = holding_pistol();
        let me = player_state::read_world().ok().flatten();

        S.with(|s| {
            let mut s = s.borrow_mut();
            s.clock += dt;
            s.cooldown = (s.cooldown - dt).max(0.0);

            if armed {
                if let Some(w) = &me {
                    if w.alive && s.cooldown <= 0.0 {
                        let origin = (w.position.0, w.position.1 + 0.2, w.position.2);
                        let actors = player_state::read_actors().ok().unwrap_or_default();
                        let mut best: Option<((f32, f32, f32), f32)> = None;
                        for a in actors.iter() {
                            if a.runtime_id == w.runtime_id { continue; }
                            let tx = a.position.0 - origin.0;
                            let ty = a.position.1 + 0.9 - origin.1;
                            let tz = a.position.2 - origin.2;
                            let d = (tx * tx + ty * ty + tz * tz).sqrt();
                            if d < 0.5 || d > 30.0 { continue; }
                            if best.map_or(true, |b| d < b.1) {
                                best = Some(((tx / d, ty / d, tz / d), d));
                            }
                        }
                        if let Some((dir, _)) = best {
                            s.bullets.push(Bullet { pos: origin, dir, life: 1.0 });
                            s.flashes.push(Flash { pos: (origin.0 + dir.0 * 0.6, origin.1 + dir.1 * 0.6, origin.2 + dir.2 * 0.6), life: 0.08 });
                            s.cooldown = 0.4;
                        }
                    }
                }
            }

            let speed = 40.0;
            for b in s.bullets.iter_mut() {
                b.pos.0 += b.dir.0 * speed * dt;
                b.pos.1 += b.dir.1 * speed * dt;
                b.pos.2 += b.dir.2 * speed * dt;
                b.life -= dt;
            }
            for f in s.flashes.iter_mut() { f.life -= dt; }
            s.bullets.retain(|b| b.life > 0.0);
            s.flashes.retain(|f| f.life > 0.0);

            let mut billboards: Vec<Billboard> = Vec::new();
            for b in s.bullets.iter() {
                billboards.push(bb(b.pos, 0.12, 0.12, (1.0, 0.9, 0.4, 1.0), BillboardPattern::SoftDisc));
                for i in 1..4 {
                    let t = i as f32 * 0.35;
                    let p = (b.pos.0 - b.dir.0 * t, b.pos.1 - b.dir.1 * t, b.pos.2 - b.dir.2 * t);
                    let a = 0.6 - i as f32 * 0.15;
                    billboards.push(bb(p, 0.08, 0.08, (1.0, 0.7, 0.2, a), BillboardPattern::Spark));
                }
            }
            for f in s.flashes.iter() {
                let flick = 0.25 + 0.05 * (s.clock * 80.0).sin();
                billboards.push(bb(f.pos, flick, flick, (1.0, 0.85, 0.3, 0.9), BillboardPattern::Spark));
                billboards.push(bb(f.pos, flick * 1.6, flick * 1.6, (1.0, 0.5, 0.1, 0.5), BillboardPattern::SoftDisc));
            }

            let _ = render::draw(&Primitives { decals: vec![], ribbons: vec![], beams: vec![], billboards });
        });

        let _ = hud::set_label(if armed { "Pistol: ARMED" } else { "Pistol: hold a golden hoe" });
    }
}
mod_api::bindings::export!(M with_types_in mod_api::bindings);
