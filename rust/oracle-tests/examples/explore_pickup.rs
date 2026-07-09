//! Scenario-engineering aid for Slice 5'b (T9) — finds a seed+placement where a
//! dropped bonus can be WALKED onto by worm0, and traces a candidate scenario to
//! confirm the pickup genuinely fires. Purely a dev tool: it drives the same
//! `process_frame` sim used by the goldens and never touches sim logic or golden
//! files. NOT a test; not run in CI.
//!
//! Usage:
//!   cargo run -p oracle-tests --example explore_pickup -- scan <seed_lo> <seed_hi> <max_drop_tick> <ticks>
//!   cargo run -p oracle-tests --example explore_pickup -- trace <scenario_path>
//!
//! `scan` reports, for each seed whose first bonus drops at/before <max_drop_tick>:
//!   seed, drop_tick, frame(0=weapon,1=health), and the bonus RESTING (ix,iy) at
//!   the end of the window (both worms idle at the level edges so no pickup).
//! `trace` drives an authored scenario and prints per-tick diagnostics around the
//!   pickup (worm0 ipos/health/weapon, bonus ipos/frame, rng draw count).

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::fixed::{ftoi, itof};
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

struct Loaded {
    tc: TcConfig,
    objects: Objects,
    level: assets::level::LevelData,
}

fn load(level_path: &str) -> Loaded {
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{level_path}"))
        .unwrap_or_else(|e| panic!("read {level_path}: {e}"));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");
    Loaded { tc, objects, level }
}

/// Build a SimState with ALL bonus + pickup consts wired (same as the T9 difftest).
#[allow(clippy::too_many_arguments)]
fn build_state(
    l: &Loaded,
    worms_init: &[WormInit],
    seed: u32,
    max_bonuses: i32,
) -> SimState {
    let tc = &l.tc;
    let objects = &l.objects;
    let mut state = SimState::new(
        &l.level,
        worms_init,
        seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(tc),
        ControlConsts::from_tc(tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        0,
        true,
        100,
    );
    state.settings_max_bonuses = max_bonuses;
    state.bonus_drop_chance = tc.constants.BonusDropChance;
    state.bonus_spawn_rect_w = tc.constants.BonusSpawnRectW;
    state.bonus_spawn_rect_h = tc.constants.BonusSpawnRectH;
    state.bonus_spawn_rect_x = tc.constants.BonusSpawnRectX;
    state.bonus_spawn_rect_y = tc.constants.BonusSpawnRectY;
    state.h_bonus_spawn_rect = tc.hacks.BonusSpawnRect;
    state.h_bonus_only_health = tc.hacks.BonusOnlyHealth;
    state.h_bonus_only_weapon = tc.hacks.BonusOnlyWeapon;
    state.h_bonus_disable = tc.hacks.BonusDisable;
    state.bonus_rand_timer = [
        [tc.bonuses[0].timer, tc.bonuses[0].timer_v],
        [tc.bonuses[1].timer, tc.bonuses[1].timer_v],
    ];
    state.bonus_s_objects = [tc.bonuses[0].sobj, tc.bonuses[1].sobj];
    state.bonus_gravity = tc.constants.BonusGravity;
    state.bonus_bounce_mul = tc.constants.BonusBounceMul;
    state.bonus_bounce_div = tc.constants.BonusBounceDiv;
    state.weap_table = vec![0i32; objects.weapons.len()];
    // Pickup consts (T8).
    state.bonus_health_var = tc.constants.BonusHealthVar;
    state.bonus_min_health = tc.constants.BonusMinHealth;
    state.bonus_explode_risk = tc.constants.BonusExplodeRisk;
    state.h_bonus_reload_only = tc.hacks.BonusReloadOnly;
    // Blood/small-sprite consts (a booby/heal path might carve/spray).
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_small_sprites();
    state
}

fn default_worms_init(l: &Loaded, scenario: &Scenario) -> Vec<WormInit> {
    let objects = &l.objects;
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(objects, &weap_order, &settings_weapons);
    if let Some(name) = scenario.weapon(0) {
        let idx = objects
            .weapons
            .iter()
            .position(|w| w.name == name)
            .unwrap_or_else(|| panic!("weapon {name:?} in TC"));
        let ammo = scenario.weapon_ammo(0).unwrap_or(objects.weapons[idx].ammo);
        resolved[0] = WeaponInit { ty: Some(idx as WeaponId), ammo };
    }
    scenario
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
        .collect()
}

fn scan(seed_lo: u32, seed_hi: u32, max_drop: u32, ticks: u32) {
    let level_path = "Levels/physics_fall_test.lev";
    let l = load(level_path);
    // Default weapon loadout (as 5c) so Worm::Process has a resolved current weapon.
    let mut weap_order: Vec<usize> = (0..l.objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| l.objects.weapons[a].name.cmp(&l.objects.weapons[b].name));
    let resolved = WormInit::resolve_weapons(&l.objects, &weap_order, &[1u32; NUM_WEAPONS]);
    // Both worms idle at the level edges (as 5c) so they never pick up: the drop is
    // then a pure function of seed. Worm floor rest y from 5c.
    let idle_worms = vec![
        WormInit {
            index: 0,
            health: 100,
            lives: 10,
            stats_x: 0,
            weapons: resolved,
            start_pos: Vec2::new(3_932_160, 12_845_520),
            visible: true,
        },
        WormInit {
            index: 1,
            health: 100,
            lives: 10,
            stats_x: 218,
            weapons: resolved,
            start_pos: Vec2::new(30_277_632, 12_845_520),
            visible: true,
        },
    ];
    println!("# seed drop_tick frame rest_ix rest_iy");
    for seed in seed_lo..=seed_hi {
        let mut state = build_state(&l, &idle_worms, seed, 4);
        let empty = [ControlState::unpack(0), ControlState::unpack(0)];
        let mut drop_tick: Option<u32> = None;
        let mut frame = -1;
        for k in 1..=ticks {
            state.process_frame(&empty);
            if drop_tick.is_none() {
                if let Some(b) = state.bonuses.iter().next() {
                    drop_tick = Some(k);
                    frame = b.frame;
                }
            }
        }
        if let Some(d) = drop_tick {
            if d <= max_drop {
                let (ix, iy) = match state.bonuses.iter().next() {
                    Some(b) => (ftoi(b.x), ftoi(b.y)),
                    None => (-1, -1), // dropped then expired (unlikely in-window)
                };
                println!("{seed} {d} {frame} {ix} {iy}");
            }
        }
    }
}

fn trace(scenario_path: &str) {
    let scenario_text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    let l = load(&scenario.level);
    let worms_init = default_worms_init(&l, &scenario);
    let mut state = build_state(&l, &worms_init, scenario.seed, scenario.max_bonuses);

    let w0 = |s: &SimState| {
        let w = &s.worms[0];
        let cw = w.current_weapon as usize;
        (ftoi(w.pos.x), ftoi(w.pos.y), w.health, w.weapons[cw].ty, w.weapons[cw].ammo)
    };
    let bonus = |s: &SimState| {
        s.bonuses
            .iter()
            .next()
            .map(|b| (ftoi(b.x), ftoi(b.y), b.frame, b.timer))
    };

    println!("# tick w0(ix,iy,hp,ty,ammo) bonus(ix,iy,frame,timer) nbon rng_draws");
    let mut prev_draws = state.rand.draws();
    let report = |tick: u32, s: &SimState, dd: u64| {
        let (ix, iy, hp, ty, ammo) = w0(s);
        let b = bonus(s);
        println!(
            "{tick} w0({ix},{iy},hp={hp},ty={ty:?},ammo={ammo}) bonus={b:?} nbon={} d+{dd}",
            s.bonuses.len()
        );
    };
    report(0, &state, 0);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        let d = state.rand.draws();
        report(k, &state, d - prev_draws);
        prev_draws = d;
    }
}

/// For a frame-0 (weapon) bonus, auto-walk worm0 onto it and classify the pickup
/// branch (reload vs booby). Returns (pickup_tick, draw_delta, health_delta,
/// ww_changed). Walking draws no RNG, so the drop is identical to the idle run.
fn weapon_walkon(
    l: &Loaded,
    seed: u32,
    rest_ix: i32,
    ticks: u32,
) -> Option<(u32, u64, i32, bool)> {
    let mut weap_order: Vec<usize> = (0..l.objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| l.objects.weapons[a].name.cmp(&l.objects.weapons[b].name));
    let resolved = WormInit::resolve_weapons(&l.objects, &weap_order, &[1u32; NUM_WEAPONS]);
    // worm0 25px LEFT of the bonus, walking right; worm1 idle on the far side.
    let w1x = if rest_ix < 252 { 30 } else { 474 };
    let worms = vec![
        WormInit {
            index: 0,
            health: 100,
            lives: 10,
            stats_x: 0,
            weapons: resolved,
            start_pos: Vec2::new(itof(rest_ix - 25), 12_845_520),
            visible: true,
        },
        WormInit {
            index: 1,
            health: 100,
            lives: 10,
            stats_x: 218,
            weapons: resolved,
            start_pos: Vec2::new(itof(w1x), 12_845_520),
            visible: true,
        },
    ];
    let mut state = build_state(l, &worms, seed, 4);
    let right = [ControlState::unpack(8), ControlState::unpack(0)];
    let mut prev_draws = state.rand.draws();
    for k in 1..=ticks {
        let cw = state.worms[0].current_weapon as usize;
        let ww_before = (state.worms[0].weapons[cw].ty, state.worms[0].weapons[cw].ammo);
        let hp_before = state.worms[0].health;
        let n_before = state.bonuses.len();
        state.process_frame(&right);
        let n_after = state.bonuses.len();
        let draws = state.rand.draws();
        if n_before == 1 && n_after == 0 {
            let cw2 = state.worms[0].current_weapon as usize;
            let ww_after = (state.worms[0].weapons[cw2].ty, state.worms[0].weapons[cw2].ammo);
            let hp_after = state.worms[0].health;
            return Some((k, draws - prev_draws, hp_after - hp_before, ww_before != ww_after));
        }
        prev_draws = draws;
    }
    None
}

fn wscan(seed_lo: u32, seed_hi: u32, max_drop: u32) {
    let l = load("Levels/physics_fall_test.lev");
    let mut weap_order: Vec<usize> = (0..l.objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| l.objects.weapons[a].name.cmp(&l.objects.weapons[b].name));
    let resolved = WormInit::resolve_weapons(&l.objects, &weap_order, &[1u32; NUM_WEAPONS]);
    let idle = |x: i32, idx: i32, sx: i32| WormInit {
        index: idx,
        health: 100,
        lives: 10,
        stats_x: sx,
        weapons: resolved,
        start_pos: Vec2::new(itof(x), 12_845_520),
        visible: true,
    };
    println!("# seed drop frame rest_ix rest_iy | pickup_tick draw_delta hp_delta ww_changed BRANCH");
    for seed in seed_lo..=seed_hi {
        // Idle run to learn the drop.
        let idle_worms = vec![idle(60, 0, 0), idle(462, 1, 218)];
        let mut state = build_state(&l, &idle_worms, seed, 4);
        let empty = [ControlState::unpack(0), ControlState::unpack(0)];
        let mut drop_tick = None;
        let mut frame = -1;
        for k in 1..=200u32 {
            state.process_frame(&empty);
            if drop_tick.is_none() {
                if let Some(b) = state.bonuses.iter().next() {
                    drop_tick = Some(k);
                    frame = b.frame;
                }
            }
        }
        let d = match drop_tick {
            Some(d) if d <= max_drop && frame == 0 => d,
            _ => continue,
        };
        let (ix, iy) = match state.bonuses.iter().next() {
            Some(b) => (ftoi(b.x), ftoi(b.y)),
            None => continue,
        };
        if !(60..=450).contains(&ix) || iy < 194 {
            continue;
        }
        if let Some((pt, dd, hpd, wc)) = weapon_walkon(&l, seed, ix, 170) {
            let branch = if dd > 50 || hpd < 0 { "BOOBY" } else { "RELOAD" };
            println!("{seed} {d} {frame} {ix} {iy} | {pt} {dd} {hpd} {wc} {branch}");
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("wscan") => {
            let lo: u32 = args[2].parse().expect("seed_lo");
            let hi: u32 = args[3].parse().expect("seed_hi");
            let max_drop: u32 = args[4].parse().expect("max_drop_tick");
            wscan(lo, hi, max_drop);
        }
        Some("scan") => {
            let lo: u32 = args[2].parse().expect("seed_lo");
            let hi: u32 = args[3].parse().expect("seed_hi");
            let max_drop: u32 = args[4].parse().expect("max_drop_tick");
            let ticks: u32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(200);
            scan(lo, hi, max_drop, ticks);
        }
        Some("trace") => trace(&args[2]),
        _ => {
            eprintln!("usage: explore_pickup scan <lo> <hi> <max_drop> [ticks] | trace <scenario>");
            std::process::exit(2);
        }
    }
}
