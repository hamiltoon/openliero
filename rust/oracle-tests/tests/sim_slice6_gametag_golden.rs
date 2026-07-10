//! Per-tick differential test for Slice-6 T5 — **GAME OF TAG** (`game_mode 1`)
//! frame-tail gate going LIVE against the C++ oracle. The golden
//! (`golden/sim_slice6_gametag.txt`, 431 lines for ticks 0..=430) is produced by the
//! real C++ `Game` running the *same scenario* (`golden/sim_slice6_gametag_scenario.txt`)
//! with `settings->game_mode = kGmGameOfTag`, so the game-mode switch
//! (`game.cpp:373-388`, already ported in the dumper's ProcessFrame tail) bumps the
//! "it"-worm's `timer`.
//!
//! Setup = the slice-5d death+respawn kill chain: worm0 air-bursts EXPLOSIVES that
//! KILL worm1 (which starts at health 12); worm1 goes invisible for its 150-tick
//! killed_timer + respawn, then flips visible again. The death sets
//! `game.last_killed_idx = 1` (worm1 is "it"). From the first 70-cycle boundary where
//! BOTH worms are visible, the gate bumps `WormByIdx(1)->timer` (hashed): with 431
//! ticks the post-respawn boundaries at cycles 350 and 420 each add 1, so worm1's
//! `timer` climbs 0 -> 1 -> 2.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves
//!
//! The gate is RNG-free, so `timer` is the only mutation and it is INVISIBLE except
//! through the hashed `worm1` column. A bit-exact match over all 431 ticks proves (a)
//! the whole 5d death/respawn chain still holds under GameOfTag, (b) the gate stays
//! CLOSED while worm1 is dead/invisible (`some_invisible`), and (c) it bumps `timer`
//! at exactly the post-respawn 70-cycle boundaries — end-to-end vs the C++ oracle.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const EMPTY_POOL: u32 = 0x0000_0001;

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

struct GoldenTick {
    tick: u32,
    master: u32,
    rng: u32,
    level: u32,
    worm0: u32,
    worm1: u32,
    pools: [u32; 5],
}

fn parse_golden(text: &str) -> Vec<GoldenTick> {
    let hex = |s: &str| u32::from_str_radix(s, 16).expect("hex column");
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let mut it = line.split_whitespace();
            let mut next = || it.next().expect("golden column present");
            let tick: u32 = next().parse().expect("tick");
            let master = hex(next());
            let rng = hex(next());
            let level = hex(next());
            let worm0 = hex(next());
            let worm1 = hex(next());
            let pools = [
                hex(next()),
                hex(next()),
                hex(next()),
                hex(next()),
                hex(next()),
            ];
            assert!(it.next().is_none(), "golden line has exactly 11 columns");
            GoldenTick {
                tick,
                master,
                rng,
                level,
                worm0,
                worm1,
                pools,
            }
        })
        .collect()
}

#[test]
fn sim_slice6_gametag_timer_gate_match_cpp_oracle() {
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice6_gametag_scenario.txt"
    ))
    .expect("read golden/sim_slice6_gametag_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.ticks, 430, "scenario ticks");
    assert_eq!(scenario.game_mode, 1, "game_mode 1 == kGmGameOfTag");
    assert_eq!(
        scenario.worms[1].health, 12,
        "worm1 (victim) starts at low health 12"
    );

    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice6_gametag.txt"
    ))
    .expect("read golden/sim_slice6_gametag.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(
        golden.len(),
        (scenario.ticks + 1) as usize,
        "golden has tick 0..=ticks"
    );

    // The death+respawn chain must be present in the golden (the gate's precondition:
    // a kill sets last_killed_idx, then both worms must be visible again).
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(
        worm1_cols.len() >= 3,
        "worm1 must show alive/dead/reborn phases"
    );
    for g in &golden {
        assert_eq!(
            g.pools[1], EMPTY_POOL,
            "tick {}: bonuses empty (max_bonuses 0)",
            g.tick
        );
    }

    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(
            w.id, i as i32,
            "weapon id must equal its index (weapon[{i}], id {})",
            w.id
        );
    }
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(
            s.id, i as i32,
            "sobject_type id must equal its index (got id {})",
            s.id
        );
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(
            n.id, i as i32,
            "nobject_type id must equal its index (got id {})",
            n.id
        );
    }

    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    let weapon_name = scenario
        .weapon(0)
        .expect("scenario `weapon 0 <name>` directive present");
    let weapon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == weapon_name)
        .unwrap_or_else(|| panic!("weapon {weapon_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(weapon_idx as WeaponId),
        ammo: objects.weapons[weapon_idx].ammo,
    };

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

    // Blood + respawn consts (as 5d) + the game mode (THE T5 STEP). Left at the `new`
    // default (game_mode 0) the switch hits `default: break` and worm1's timer never
    // bumps — the run then diverges at the first 70-cycle boundary post-respawn.
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
    state.game_mode = scenario.game_mode as u32;
    assert_eq!(
        state.time_to_lose, 600,
        "time_to_lose defaults to the Settings value 600"
    );

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };
    let assert_tick = |state: &SimState, g: &GoldenTick| {
        let c = hash_components(state);
        check(g.tick, "rng", c.rng, g.rng);
        check(g.tick, "level", c.level, g.level);
        check(g.tick, "worm0", c.worms[0], g.worm0);
        check(g.tick, "worm1", c.worms[1], g.worm1);
        check(g.tick, "bobjects", c.bobjects, g.pools[0]);
        check(g.tick, "bonuses", c.bonuses, g.pools[1]);
        check(g.tick, "sobjects", c.sobjects, g.pools[2]);
        check(g.tick, "nobjects", c.nobjects, g.pools[3]);
        check(g.tick, "wobjects", c.wobjects, g.pools[4]);
        check(
            g.tick,
            "MASTER state_hash",
            hash_game_state(state),
            g.master,
        );
    };

    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // Coverage witnesses from the DRIVEN state (never re-parsed from the golden).
    let mut w1_timer: Vec<i32> = Vec::with_capacity(golden.len());
    let mut w0_timer: Vec<i32> = Vec::with_capacity(golden.len());
    let mut w1_visible: Vec<bool> = Vec::with_capacity(golden.len());
    let mut w0_visible: Vec<bool> = Vec::with_capacity(golden.len());
    let mut w1_health: Vec<i32> = Vec::with_capacity(golden.len());
    let mut last_killed: Vec<i32> = Vec::with_capacity(golden.len());
    let mut cycles: Vec<i32> = Vec::with_capacity(golden.len());
    let mut record = |state: &SimState| {
        w1_timer.push(state.worms[1].timer);
        w0_timer.push(state.worms[0].timer);
        w1_visible.push(state.worms[1].visible);
        w0_visible.push(state.worms[0].visible);
        w1_health.push(state.worms[1].health);
        last_killed.push(state.last_killed_idx);
        cycles.push(state.cycles);
    };
    record(&state);

    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // ================= THE GAMEOFTAG COVERAGE GUARDS (all from the DRIVEN state) =====

    // The kill happened: worm1 died (health crossed <= 0) and `last_killed_idx`
    // latched to 1 (worm1 is "it") — the gate's precondition.
    assert!(
        w1_health.iter().any(|&h| h <= 0),
        "worm1 must DIE (sets last_killed_idx)"
    );
    assert_eq!(
        *last_killed.last().unwrap(),
        1,
        "last_killed_idx latches to worm1 (idx 1)"
    );
    assert_eq!(w1_timer[0], 0, "worm1 timer starts at 0");

    // worm1 respawns: it goes invisible (dead) then visible again.
    assert!(w1_visible[0], "worm1 starts visible");
    let first_dead = w1_visible
        .iter()
        .position(|&v| !v)
        .expect("worm1 goes invisible (dies)");
    let reborn = (first_dead..w1_visible.len())
        .find(|&k| w1_visible[k])
        .expect("worm1 goes visible again (respawn)");

    // THE WITNESS: worm1's timer bumps EXACTLY on the ticks where it increments; each
    // bump must be a 70-cycle boundary with BOTH worms visible, and must be strictly
    // after the respawn (while dead the `some_invisible` guard keeps the gate closed).
    let mut bump_ticks: Vec<usize> = Vec::new();
    for k in 1..w1_timer.len() {
        if w1_timer[k] != w1_timer[k - 1] {
            assert_eq!(
                w1_timer[k],
                w1_timer[k - 1] + 1,
                "tick {k}: timer bumps by exactly 1"
            );
            bump_ticks.push(k);
        }
    }
    assert_eq!(
        bump_ticks.len(),
        2,
        "exactly two timer bumps in the window (cycles 350, 420)"
    );
    for &k in &bump_ticks {
        assert_eq!(
            cycles[k] % 70,
            0,
            "tick {k}: bump only on a 70-cycle boundary"
        );
        assert!(
            w0_visible[k] && w1_visible[k],
            "tick {k}: bump only while BOTH worms visible"
        );
        assert!(
            k > reborn,
            "tick {k}: bump only after respawn (gate closed while dead)"
        );
    }

    // While worm1 was dead/invisible, NO 70-cycle boundary bumped the timer.
    for k in first_dead..reborn {
        assert_eq!(
            w1_timer[k], 0,
            "tick {k}: timer stays 0 while worm1 is dead/invisible"
        );
    }

    // Final state: worm1's timer reached 2; worm0 was NEVER "it", so its timer is flat 0.
    assert_eq!(
        *w1_timer.last().unwrap(),
        2,
        "worm1 timer climbs to 2 (bumps at 350 + 420)"
    );
    assert!(
        w0_timer.iter().all(|&t| t == 0),
        "worm0 is never it -> its timer stays 0"
    );
}
