//! Per-tick differential test for the Slice-3 worm control + aiming ports against
//! the C++ oracle. The golden (`golden/sim_slice3.txt`, 146 lines for ticks
//! 0..=145) is produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice3_scenario.txt`): seed 42, a LOADED `physics_fall_test.lev`,
//! two visible worms that fall, land, then are driven by SCRIPTED 7-bit input
//! through walk / aim / weapon-change / ninjarope / jump phases.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What is asserted — the MASTER turns on here
//!
//! Unlike Slice 2 (which read but did NOT assert the master `state_hash` because
//! the un-ported `ProcessWeapons` `delay_left` countdown diverged it), this test
//! asserts EVERY column every tick, including the master `state_hash` (column 2).
//! The master folds in the slice-3 control fields that the component columns do
//! NOT separately expose at the pool level — `aiming_angle`, `control_states`,
//! each weapon's `delay_left`, and the ninjarope — so a tick whose components all
//! match but whose master does not localises the divergence to a slice-3 control
//! port. This is the slice-3 master-hash gate: a real match over 146 ticks
//! including all control phases proves the five control ports are bit-exact.
//!
//! The component columns are still asserted FIRST (rng → level → worm0 → worm1 →
//! pools) so a divergence localises to a tick + subsystem before the master flags
//! it. rng stays 00000000 (the scripted input never digs/fires, so no `rand()`),
//! level stays 95f63601 (no terrain writes), and the five pools stay 00000001
//! (empty) — exactly the slice contract.
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

/// One parsed golden line — all 11 columns, master included (asserted this slice).
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
            let master = hex(next()); // state_hash: ASSERTED this slice (master gate).
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
fn sim_slice3_control_matches_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice3_scenario.txt"
    ))
    .expect("read golden/sim_slice3_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 145, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=145, master + 9 components). -----
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice3.txt"
    ))
    .expect("read golden/sim_slice3.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). Path is
    // relative to the TC root, mirroring sim_slice1/slice2_golden.
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants. ----------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slice 1/2.
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
        Vec::new(),
        Vec::new(),
        100,
        true,
    );

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, pools) THEN the master,
    // so a divergence localises to a tick + subsystem before the master flags it
    // (component match + master mismatch => a slice-3 control-only field diverged).
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
        // The MASTER, last: the slice-3 gate. If components matched but this does
        // not, the divergence is in a master-only field (aiming_angle /
        // control_states.pack() / weapons delay_left / ninjarope).
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage guard: record that the scripted run actually exercises the
    // slice-3 control paths (not just physics) across the matched ticks.
    let mut aiming_seen = std::collections::HashSet::new();
    let mut delay_seen = std::collections::HashSet::new();
    let mut pack_seen = std::collections::HashSet::new();
    let mut rope_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        for w in &state.worms {
            aiming_seen.insert(w.aiming_angle);
            pack_seen.insert(w.control_states.pack());
            rope_seen.insert(w.ninjarope.out);
            for wpn in &w.weapons {
                delay_seen.insert(wpn.delay_left);
            }
        }
    };
    record(&state);

    // --- Drive each subsequent tick under SCRIPTED input. THE OFF-BY-ONE:
    // golden line `k` (k>=1) is the result of applying input[k-1] on the pass that
    // advances tick k-1 -> k (design doc, *Input timing*). So produce line `k` by
    // calling process_frame with input keyed `k-1`.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // --- Coverage assertions: each ported control path took >= 2 distinct values,
    // so the master match above genuinely exercises aiming / weapon-change /
    // ninjarope, not merely the constant-component physics settle.
    assert!(
        aiming_seen.len() >= 2,
        "aiming_angle should vary across the run (aim phases); saw {:?}",
        aiming_seen
    );
    assert!(
        delay_seen.len() >= 2,
        "some weapon delay_left should vary (weapon-change/firecone); saw {:?}",
        delay_seen
    );
    assert!(
        pack_seen.len() >= 2,
        "control_states.pack() should vary across the input phases; saw {:?}",
        pack_seen
    );
    assert!(
        rope_seen.len() >= 2,
        "ninjarope.out should toggle (throw/retract phase); saw {:?}",
        rope_seen
    );
}
