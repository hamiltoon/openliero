//! ASCII visualization of the Slice-5c bonus-drop scenario, for human
//! eyeballing (bonus dropping ~tick 252 and falling) instead of reading hash
//! columns. Reuses the exact scenario/TC/const setup from
//! `oracle-tests/tests/sim_slice5c_golden.rs` (same input k-1 keying), then
//! renders a handful of "interesting" ticks with [`sim::debug::render_ascii`]
//! to both stdout and `target/frames/slice5c/tick_NNNNN.txt`.
//!
//! Run: `cargo run -p oracle-tests --example render_slice5c`
//!
//! Purely a visualization aid: it drives the SAME `process_frame` sim
//! forward and never touches sim logic or the golden files.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::debug::{render_ascii, RenderOpts};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WormInit, NUM_WEAPONS};
use sim_core::fixed::ftoi;
use sim_core::vec::Vec2;
use std::path::PathBuf;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`).
/// Threaded into `SimState` for `draw_dirt_effect`; inert in 5c (no carving) but the
/// `SimState::new` signature requires it. Copied verbatim from the slice5c golden test.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// The output ticks to render: a hand-picked window straddling the seed-42
/// bonus drop (~tick 252), so a human can see it appear and start falling.
const INTERESTING_TICKS: [u32; 6] = [251, 252, 255, 260, 270, 300];

fn main() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    // Verbatim setup from sim_slice5c_golden.rs's scenario/TC/const block.
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5c_scenario.txt"
    ))
    .expect("read golden/sim_slice5c_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). --------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants + object tables.
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // weap_order: indices sorted by weapon name; id == index. Mirrors Common::Precompute
    // (common.cpp:492-499), exactly as slices 1-5b.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0. No
    // `weapon` override in 5c, so the worms keep their default InitWeapons loadout.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

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

    // --- Build tick-0 state (same `new` signature as 5b). --------------------
    let large_sprites = load_large_sprites();
    let mut state = SimState::new(
        &level,
        &worms_init,
        scenario.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        large_sprites,
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        0,
        true,
        100,
    );

    // --- THE CRITICAL 5c STEP: set ALL the bonus consts post-`new` (defaulted to
    // 0/false/empty by `SimState::new`). Verbatim from the golden test.
    state.settings_max_bonuses = scenario.max_bonuses;
    state.bonus_drop_chance = tc.constants.BonusDropChance;
    state.bonus_spawn_rect_w = tc.constants.BonusSpawnRectW;
    state.bonus_spawn_rect_h = tc.constants.BonusSpawnRectH;
    state.bonus_spawn_rect_x = tc.constants.BonusSpawnRectX;
    state.bonus_spawn_rect_y = tc.constants.BonusSpawnRectY;
    state.h_bonus_spawn_rect = tc.hacks.BonusSpawnRect;
    state.h_bonus_only_health = tc.hacks.BonusOnlyHealth;
    state.h_bonus_only_weapon = tc.hacks.BonusOnlyWeapon;
    state.h_bonus_disable = tc.hacks.BonusDisable;
    assert!(
        tc.bonuses.len() >= 2,
        "TC must define 2 bonus types (weapon, health)"
    );
    state.bonus_rand_timer = [
        [tc.bonuses[0].timer, tc.bonuses[0].timer_v],
        [tc.bonuses[1].timer, tc.bonuses[1].timer_v],
    ];
    state.bonus_s_objects = [tc.bonuses[0].sobj, tc.bonuses[1].sobj];
    state.bonus_gravity = tc.constants.BonusGravity;
    state.bonus_bounce_mul = tc.constants.BonusBounceMul;
    state.bonus_bounce_div = tc.constants.BonusBounceDiv;
    state.weap_table = vec![0i32; objects.weapons.len()];

    // --- Output dir: <workspace>/target/frames/slice5c (git-ignored via /target). ---
    let out_dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "target", "frames", "slice5c"]
        .iter()
        .collect();
    std::fs::create_dir_all(&out_dir).expect("create target/frames/slice5c");

    let opts = RenderOpts { scale: 6 };
    let render_and_emit = |state: &SimState, tick: u32| {
        // Bonus::x/y are 16.16 fixed-point (see sim::debug's note); convert to
        // pixel coords for the human-facing header.
        let bonus_str = match state.bonuses.iter().next() {
            Some(b) => format!("({}, {})", ftoi(b.x), ftoi(b.y)),
            None => "none".to_string(),
        };
        let header = format!(
            "=== tick {tick}  cycles={}  bonus={bonus_str} ===\n",
            state.cycles
        );
        let frame = render_ascii(state, &opts);
        let mut out = header;
        out.push_str(&frame);

        print!("{out}");

        let path = out_dir.join(format!("tick_{tick:05}.txt"));
        std::fs::write(&path, &out).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    };

    // --- Drive each tick under EMPTY scripted input; render the interesting ones.
    // Same input k-1 keying as the golden test: golden line k (k>=1) is the result
    // of applying input[k-1] on the pass advancing tick k-1 -> k.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        if INTERESTING_TICKS.contains(&k) {
            render_and_emit(&state, k);
        }
    }

    eprintln!(
        "\nWrote {} frames to {}",
        INTERESTING_TICKS.len(),
        out_dir.display()
    );
}
