//! Per-tick differential test for the Slice-2 worm physics against the C++
//! oracle. The golden (`golden/sim_slice2.txt`, 101 lines for ticks 0..=100) is
//! produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice2_scenario.txt`): seed 42, a LOADED `physics_fall_test.lev`,
//! two visible worms spawned mid-air, no input — they free-fall under gravity and
//! bounce off the floor. Each golden line dumps component hashes so a divergence
//! localises to a tick + subsystem.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What is asserted
//!
//! This test asserts ALL columns every tick: the COMPONENT columns (`rng`,
//! `level`, `worm0`, `worm1`, and the five object-pool columns) AND the master
//! `state_hash`. The master column was read-but-not-asserted in the original
//! Slice-2 work because the C++ `Game::ProcessFrame` also ran the then-un-ported
//! `ProcessWeapons` `delay_left` countdown; Slice 3 ported that countdown
//! (`process_weapons`), closing the gap. The master now matches the C++ golden on
//! all 101 ticks under empty input.
//!
//! `rng` is 00000000 (the seed never advances in a worms-only pass), `level` is
//! constant 95f63601 (physics never writes the level this slice), and the five
//! pool columns are all 00000001 (empty pools) — exactly the Slice-2 contract.
//!
//! The scenario is the single source of truth (parsed via `oracle_tests::scenario`)
//! and the expected values are PARSED from the golden file, never hard-coded.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`)
/// from `sprites/large.tga`. Threaded into `SimState` for Slice-4b's DrawDirtEffect;
/// not hashed, so it does not affect this golden.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// One parsed golden line — all 11 columns, master included (asserted since Slice 3).
struct GoldenTick {
    tick: u32,
    master: u32,
    rng: u32,
    level: u32,
    worm0: u32,
    worm1: u32,
    pools: [u32; 5], // bob, bon, sob, nob, wob
}

fn parse_golden(text: &str) -> Vec<GoldenTick> {
    let hex = |s: &str| u32::from_str_radix(s, 16).expect("hex column");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let mut it = line.split_whitespace();
            let mut next = || it.next().expect("golden column present");
            let tick: u32 = next().parse().expect("tick");
            let master = hex(next()); // state_hash: ASSERTED (Slice 3 closed the gap).
            let rng = hex(next());
            let level = hex(next());
            let worm0 = hex(next());
            let worm1 = hex(next());
            let pools = [hex(next()), hex(next()), hex(next()), hex(next()), hex(next())];
            assert!(it.next().is_none(), "golden line has exactly 11 columns");
            GoldenTick { tick, master, rng, level, worm0, worm1, pools }
        })
        .collect()
}

#[test]
fn sim_slice2_physics_matches_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice2_scenario.txt"
    ))
    .expect("read golden/sim_slice2_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");

    // --- Parse the golden component vectors (ticks 0..=100). -----------------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice2.txt"
    ))
    .expect("read golden/sim_slice2.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- Load the SAME level the C++ dumper loaded (the fall fixture, NOT
    // modern_test — modern_test has no Background-flagged pixels, so worms can't
    // fall in it). Path is relative to the TC root, mirroring sim_slice1_golden.
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics constants. ------------------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as sim_slice1_golden.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Build the two worm inits from the scenario (start_pos, visible=true). -
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

    // --- Load the real large-sprite bank + TC textures (Slice-4b assets). Not
    // indexed this slice and not hashed, so this golden is unchanged.
    let large_sprites = load_large_sprites();

    // --- Build tick-0 state. -------------------------------------------------
    let mut state = SimState::new(
        &level,
        &worms_init,
        scenario.seed,
        &tc.materials,
        Vec::new(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        large_sprites,
        tc.textures.clone(),
    );

    // Assert tick-0 components against the freshly-built state FIRST. Then drive
    // each subsequent tick and assert in component order (rng, level, worm0,
    // worm1, pools) so a failure localises to a tick + subsystem.
    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };

    let assert_components = |state: &SimState, g: &GoldenTick| {
        let c = hash_components(state);
        // Assert components first so a divergence localises to tick + subsystem
        // before the master flags it.
        check(g.tick, "rng", c.rng, g.rng);
        check(g.tick, "level", c.level, g.level);
        check(g.tick, "worm0", c.worms[0], g.worm0);
        check(g.tick, "worm1", c.worms[1], g.worm1);
        check(g.tick, "bobjects", c.bobjects, g.pools[0]);
        check(g.tick, "bonuses", c.bonuses, g.pools[1]);
        check(g.tick, "sobjects", c.sobjects, g.pools[2]);
        check(g.tick, "nobjects", c.nobjects, g.pools[3]);
        check(g.tick, "wobjects", c.wobjects, g.pools[4]);
        // The MASTER: now asserted. Slice 3 ported the ProcessWeapons delay_left
        // countdown, closing the gap that had kept this un-asserted in Slice 2.
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_components(&state, &golden[0]);

    // Slice 2 has no input every tick (the scenario defines none); drive both
    // worms with an empty control state. process_frame advances one worms-only
    // tick (the full per-worm Process pass).
    let empty = [ControlState::new(), ControlState::new()];
    // A genuine bounce: worm0 falls (vel.y > 0) then the floor flips it
    // (vel.y <= 0) on some tick whose worm0 component we have asserted matches
    // the C++ golden. Recorded so the test fails loudly if the scenario ever
    // stops exercising a bounce within `ticks`.
    let mut bounce_tick: Option<u32> = None;
    let mut prev_vy = state.worms[0].vel.y;
    for g in &golden[1..] {
        state.process_frame(&empty);
        assert_components(&state, g);
        let vy = state.worms[0].vel.y;
        if bounce_tick.is_none() && prev_vy > 0 && vy < prev_vy {
            // Downward velocity reversed/decelerated by the floor on this matched
            // tick — the bounce.
            bounce_tick = Some(g.tick);
        }
        prev_vy = vy;
    }
    assert!(
        bounce_tick.is_some(),
        "expected worm0 to fall then bounce off the floor within {} ticks; \
         worm0's hashed state was matched against the C++ golden every tick",
        scenario.ticks
    );
}
