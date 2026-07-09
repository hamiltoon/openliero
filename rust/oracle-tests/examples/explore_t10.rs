//! Scenario-engineering aid for Slice 5' (T10) — the MOVING-WORMS fuzz. Drives an
//! authored scenario with the exact `process_frame` sim the goldens use and prints,
//! per tick, BOTH worms' ipos / health / current_frame / direction / animate plus the
//! rng draw delta, flagging every CONTACT tick (a worm's health drops) with the VICTIM's
//! (current_frame, direction) at that tick — the coverage the fuzz must land at varied
//! combos. Purely a dev tool: never touches sim logic or golden files; NOT a test.
//!
//! Usage: cargo run -p oracle-tests --example explore_t10 -- trace <scenario_path>

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::fixed::ftoi;
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
}

fn build_state(scenario: &Scenario) -> SimState {
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);
    if let Some(name) = scenario.weapon(0) {
        let idx = objects
            .weapons
            .iter()
            .position(|w| w.name == name)
            .unwrap_or_else(|| panic!("weapon {name:?} in TC"));
        let ammo = scenario.weapon_ammo(0).unwrap_or(objects.weapons[idx].ammo);
        resolved[0] = WeaponInit { ty: Some(idx as WeaponId), ammo };
    }

    let worms_init: Vec<WormInit> = scenario
        .worms
        .iter()
        .map(|w| WormInit {
            index: w.index,
            health: w.health,
            lives: w.lives,
            stats_x: w.stats_x,
            weapons: resolved,
            start_pos: Vec2::new(w.pos_x, w.pos_y),
            visible: w.visible,
        })
        .collect();

    let mut state = SimState::new(
        &level,
        &worms_init,
        scenario.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        0,
        true,
        100,
    );
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_small_sprites();
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state
}

fn row(tick: u32, s: &SimState, dd: u64, mark: &str) {
    let f = |w: &sim::state::WormState| {
        (ftoi(w.pos.x), ftoi(w.pos.y), w.health, w.current_frame, w.direction, w.animate as u8)
    };
    let (x0, y0, h0, cf0, d0, a0) = f(&s.worms[0]);
    let (x1, y1, h1, cf1, d1, a1) = f(&s.worms[1]);
    let darts: Vec<(i32, i32)> = s.wobjects.iter().map(|w| (ftoi(w.pos.x), ftoi(w.pos.y))).collect();
    println!(
        "{tick:3} | w0 ({x0:3},{y0:3}) h{h0:3} cf{cf0:2} dir{d0} an{a0} | \
         w1 ({x1:3},{y1:3}) h{h1:3} cf{cf1:2} dir{d1} an{a1} | \
         nob{} wob{darts:?} d+{dd:2} {mark}",
        s.nobjects.len(),
    );
}

fn trace(scenario_path: &str) {
    let scenario_text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    let mut state = build_state(&scenario);

    let mut prev_h = [state.worms[0].health, state.worms[1].health];
    let mut prev_draws = state.rand.draws();
    println!("# tick | worm0 (ix,iy) h cf dir an | worm1 ... | pools d+draws  CONTACT?");
    row(0, &state, 0, "");
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        let d = state.rand.draws();
        let mut mark = String::new();
        for wi in 0..2 {
            let h = state.worms[wi].health;
            if h < prev_h[wi] {
                let w = &state.worms[wi];
                mark.push_str(&format!(
                    "*** worm{wi} HIT {}->{} (victim cf{} dir{} an{} ix{})",
                    prev_h[wi], h, w.current_frame, w.direction, w.animate as u8, ftoi(w.pos.x)
                ));
            }
            prev_h[wi] = h;
        }
        row(k, &state, d - prev_draws, &mark);
        prev_draws = d;
    }
}

/// Emit the exact 11-column golden format (tick master rng level worm0 worm1 bob bon
/// sob nob wob) the C++ dumper produces, so a `diff` against the C++ output proves the
/// scenario is bit-exact before it is committed.
fn dump(scenario_path: &str) {
    let scenario_text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    let mut state = build_state(&scenario);
    let emit = |tick: u32, s: &SimState| {
        let c = hash_components(s);
        println!(
            "{tick} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}",
            hash_game_state(s), c.rng, c.level, c.worms[0], c.worms[1],
            c.bobjects, c.bonuses, c.sobjects, c.nobjects, c.wobjects
        );
    };
    emit(0, &state);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        emit(k, &state);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("trace") => trace(&args[2]),
        Some("dump") => dump(&args[2]),
        _ => {
            eprintln!("usage: explore_t10 trace|dump <scenario>");
            std::process::exit(2);
        }
    }
}
