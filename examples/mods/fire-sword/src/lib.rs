//! Fire Sword (visual only).
//!
//! While you hold the FIRE_SWORD item, hitting an animal makes it look like it is on fire
//! for a few seconds. Nothing is sent to the server: other players see nothing and the
//! animal takes no extra damage. The flames are drawn locally as camera-facing billboards.
//!
//! Grants needed: player_state, entities, render.

use std::cell::RefCell;

use mod_api::bindings::{
    cinnabar::extension::{
        gameplay::{self, Mob},
        hud, player_state,
        player_state::CombatKind,
        render::{self, Billboard, BillboardPattern, Primitives, Rgba},
    },
    Guest,
};

/// The item that counts as the fire sword. Change this to any item identifier.
const FIRE_SWORD: &str = "minecraft:golden_sword";
/// How long a hit animal keeps burning, in seconds.
const BURN_SECONDS: f64 = 4.0;
/// Ignore combat evidence for this long after load, so old retained hits do not ignite things.
const WARMUP_SECONDS: f64 = 1.0;
/// If the hit event's actor id is not in the mob list, use the nearest mob within this range.
const FALLBACK_RANGE_BLOCKS: f32 = 6.0;
/// Most animals burning at once.
const MAX_BURNING: usize = 16;

/// Animals that can be set alight. Add or remove identifiers to taste.
const ANIMALS: &[&str] = &[
    "minecraft:cow", "minecraft:pig", "minecraft:sheep", "minecraft:chicken",
    "minecraft:horse", "minecraft:donkey", "minecraft:mule", "minecraft:llama",
    "minecraft:rabbit", "minecraft:cat", "minecraft:wolf", "minecraft:fox",
    "minecraft:goat", "minecraft:mooshroom", "minecraft:ocelot", "minecraft:parrot",
    "minecraft:panda", "minecraft:polar_bear", "minecraft:turtle", "minecraft:camel",
];

struct Burn {
    id: u64,
    until: f64,
}

#[derive(Default)]
struct State {
    last_seq: u64,
    now: f64,
    burning: Vec<Burn>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

struct FireSword;

impl Guest for FireSword {
    fn init() {
        let _ = hud::set_label("Fire Sword: hold a golden sword and hit an animal");
    }

    fn frame() {
        STATE.with(|cell| step(&mut cell.borrow_mut()));
    }
}

fn is_animal(type_id: &str) -> bool {
    ANIMALS.contains(&type_id)
}

/// Rough flame height by animal size.
fn body_height(type_id: &str) -> f32 {
    match type_id {
        "minecraft:chicken" | "minecraft:rabbit" | "minecraft:cat" | "minecraft:parrot"
        | "minecraft:ocelot" | "minecraft:turtle" => 0.5,
        "minecraft:cow" | "minecraft:horse" | "minecraft:donkey" | "minecraft:mule"
        | "minecraft:llama" | "minecraft:mooshroom" | "minecraft:camel"
        | "minecraft:polar_bear" => 1.5,
        _ => 0.9,
    }
}

fn holding_fire_sword() -> bool {
    let Ok(Some(eq)) = player_state::read_equipment() else {
        return false;
    };
    let Some(slot) = eq.selected_slot else {
        return false;
    };
    eq.inventory
        .get(slot as usize)
        .and_then(|s| s.item.as_ref())
        .and_then(|i| i.identifier.as_deref())
        == Some(FIRE_SWORD)
}

fn dist2(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
    let (dx, dy, dz) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
    dx * dx + dy * dy + dz * dz
}

fn step(s: &mut State) {
    // Real elapsed time from measured fps (no clock grant needed).
    let fps = player_state::read_fps()
        .ok()
        .flatten()
        .filter(|f| *f > 1.0)
        .unwrap_or(60.0);
    s.now += 1.0 / fps as f64;

    let mobs: Vec<Mob> = gameplay::read_mobs().unwrap_or_default();
        let events = player_state::read_combat(s.last_seq, 4).unwrap_or_default();
    let warm = s.now < WARMUP_SECONDS;
    let holding = !events.is_empty() && holding_fire_sword();

    for ev in events {
        s.last_seq = s.last_seq.max(ev.sequence);
        if warm || !holding || !ev.local || !matches!(ev.kind, CombatKind::AttackAdmitted) {
            continue;
        }
        // Prefer the mob the event names; otherwise the nearest mob to where it happened.
        let target = mobs
            .iter()
            .find(|m| m.runtime_id == ev.runtime_id)
            .or_else(|| {
                mobs.iter()
                    .map(|m| (dist2((m.position.x, m.position.y, m.position.z), ev.position), m))
                    .filter(|(d, _)| *d <= FALLBACK_RANGE_BLOCKS * FALLBACK_RANGE_BLOCKS)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, m)| m)
            });
        let Some(mob) = target else { continue };
        if !is_animal(&mob.type_id) {
            continue;
        }
        let until = s.now + BURN_SECONDS;
        let id = mob.runtime_id;
        if let Some(i) = s.burning.iter().position(|b| b.id == id) {
            s.burning[i].until = until;
        } else if s.burning.len() < MAX_BURNING {
            s.burning.push(Burn { id, until });
        }
    }

    let now = s.now;
    s.burning.retain(|b| b.until > now);
    if s.burning.is_empty() {
        return;
    }

    let mut billboards = Vec::new();
    for burn in &s.burning {
        let Some(mob) = mobs.iter().find(|m| m.runtime_id == burn.id) else {
            continue; // out of range this frame; keep the timer running
        };
        let fade = ((burn.until - now) / 0.6).clamp(0.0, 1.0) as f32;
        let height = body_height(&mob.type_id);
        for k in 0..6u32 {
            let t = k as f32 / 5.0; // 0 at the feet, 1 at the top
            let phase = (now as f32) * 9.0 + (burn.id % 97) as f32 * 1.7 + k as f32 * 1.3;
            let mut pos = mob.position;
            pos.x += phase.sin() * 0.22 * height;
            pos.z += (phase * 1.1).cos() * 0.22 * height;
            pos.y += 0.1 + t * height + (phase * 1.3).sin() * 0.05;
            let size = (0.75 - 0.35 * t) * (0.6 + 0.4 * height.min(1.2));
            // Orange at the base fading to yellow at the tips.
            let color = Rgba { r: 1.0, g: 0.3 + 0.5 * t, b: 0.05 + 0.1 * t, a: (0.9 - 0.3 * t) * fade };
            billboards.push(Billboard {
                position: pos,
                width: size,
                height: size * 1.25,
                color,
                pattern: if k % 2 == 0 { BillboardPattern::SoftDisc } else { BillboardPattern::Spark },
                upright: false,
            });
        }
    }

    if !billboards.is_empty() {
        let _ = render::draw(&Primitives {
            decals: Vec::new(),
            ribbons: Vec::new(),
            beams: Vec::new(),
            billboards,
        });
    }
}

mod_api::bindings::export!(FireSword with_types_in mod_api::bindings);
