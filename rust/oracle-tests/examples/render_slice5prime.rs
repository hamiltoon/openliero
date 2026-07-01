//! ASCII visualization of the Slice-5'a per-pixel wobject (dart) worm-hit
//! scenario, for human eyeballing (the dart '+' skimming toward worm1, the
//! near-miss ticks where it grazes transparent pixels, and the contact tick
//! @43 where it lands on a solid pixel and blood sprays) instead of reading
//! hash columns. Reuses the exact scenario/TC/const setup from
//! `oracle-tests/tests/sim_slice5prime_golden.rs` (same input k-1 keying),
//! then renders a handful of "interesting" ticks with
//! [`sim::debug::render_ascii`] to both stdout and
//! `target/frames/slice5prime/tick_NNNNN.txt`.
//!
//! Run: `cargo run -p oracle-tests --example render_slice5prime`
//!
//! Purely a visualization aid: it drives the SAME `process_frame` sim
//! forward and never touches sim logic or the golden files.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::debug::{render_ascii, RenderOpts};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;
use std::path::PathBuf;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`).
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Load the shipped 7x7 small-sprite bank (C++ `small_sprites.Allocate(7,7,130)`).
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
}

/// Hand-picked ticks straddling the fire (40), the near-miss ticks (41, 42),
/// the contact tick (43), and the settle-out.
const INTERESTING_TICKS: [u32; 7] = [40, 41, 42, 43, 45, 50, 60];

fn main() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    // Verbatim setup from sim_slice5prime_golden.rs's scenario/TC/const block.
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5prime_scenario.txt"
    ))
    .expect("read golden/sim_slice5prime_scenario.txt");
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

    // weap_order: indices sorted by weapon name; id == index.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with DART (the scenario `weapon 0` directive).
    let weapon_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
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

    // --- Build tick-0 state (same `new` signature as 5d). ----------------------
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

    // --- THE CRITICAL 5'a STEP: set the blood consts (as 5b/5d). --------------
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

    // --- Output dir: <workspace>/target/frames/slice5prime (git-ignored via /target). --
    let out_dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "target", "frames", "slice5prime"]
        .iter()
        .collect();
    std::fs::create_dir_all(&out_dir).expect("create target/frames/slice5prime");

    let opts = RenderOpts { scale: 4 };
    let render_and_emit = |state: &SimState, tick: u32| {
        let w0 = &state.worms[0];
        let w1 = &state.worms[1];
        let header = format!(
            "=== tick {tick}  worm0(h{},vis{})  worm1(h{},vis{}) ===\n",
            w0.health, w0.visible as u8, w1.health, w1.visible as u8
        );
        let frame = render_ascii(state, &opts);
        let mut out = header;
        out.push_str(&frame);

        print!("{out}");

        let path = out_dir.join(format!("tick_{tick:05}.txt"));
        std::fs::write(&path, &out).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    };

    // --- Drive each tick under scripted input; render the interesting ones.
    // Same input k-1 keying as the golden test: golden line k (k>=1) is the
    // result of applying input[k-1] on the pass advancing tick k-1 -> k.
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
