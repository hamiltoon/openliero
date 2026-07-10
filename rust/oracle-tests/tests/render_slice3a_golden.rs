//! Per-tick FRAME-HASH differential test for Slice-3a: the first pixel-exact
//! terrain frame. Drives `SimState` N ticks (Step-2 scenario runner), renders
//! each tick with the `render` crate (terrain-only, two-viewport 320x200), and
//! matches the C++ sidecar golden (`golden/render_slice3a.txt`) line-for-line
//! INCLUDING the `total` line. Column 3 (`state_hash`) is asserted against the
//! Rust `hash_game_state` (isolation: rendering did not perturb the sim) AND
//! against the sim golden's master column (`render_slice3a_sim.txt`).
//!
//! Proves: (1) DrawLevel Classic + palette build are pixel-exact vs C++; (2) the
//! frame hash VISIBLY CHANGES on a cycles>>3 boundary (RotateFrom observable);
//! (3) the render path is a pure consumer (state hashes unchanged).

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::hash_game_state;
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WormInit, NUM_WEAPONS};
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

struct FrameLine {
    tick: u32,
    frame_hash: u64,
    state_hash: u32,
}

fn parse_frames(text: &str) -> (Vec<FrameLine>, u32, u64) {
    let mut lines = Vec::new();
    let mut total_n = 0u32;
    let mut total_acc = 0u64;
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        let head = it.next().unwrap();
        if head == "total" {
            total_n = it.next().unwrap().parse().unwrap();
            total_acc = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        } else {
            let tick: u32 = head.parse().unwrap();
            let frame_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            let state_hash = u32::from_str_radix(it.next().unwrap(), 16).unwrap();
            lines.push(FrameLine {
                tick,
                frame_hash,
                state_hash,
            });
        }
    }
    (lines, total_n, total_acc)
}

#[test]
fn render_slice3a_frame_hash_matches_cpp_oracle() {
    // --- load scenario + goldens ---
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/render_slice3a_scenario.txt"
    ))
    .expect("read golden/render_slice3a_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.ticks, 26, "scenario ticks");
    assert_eq!(scenario.game_mode, 0, "game_mode 0 (KillEmAll)");

    let frames_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/render_slice3a.txt"
    ))
    .expect("read golden/render_slice3a.txt");
    let (frames, total_n, total_acc) = parse_frames(&frames_text);
    assert_eq!(
        frames.len(),
        (scenario.ticks + 1) as usize,
        "one line per tick 0..=ticks"
    );

    // --- isolation source: the sim golden's master column ---
    let sim_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/render_slice3a_sim.txt"
    ))
    .expect("read golden/render_slice3a_sim.txt");
    let sim_master: Vec<u32> = sim_text
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|l| u32::from_str_radix(l.split_whitespace().nth(1).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(
        sim_master.len(),
        (scenario.ticks + 1) as usize,
        "sim golden has one line per tick 0..=ticks"
    );

    // --- Origpal = small.tga's embedded palette (C++ common.exepal) ---
    let small_bytes =
        std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let small_tga = assets::sprite::Tga::load(&small_bytes).expect("small.tga parses");
    let origpal = small_tga.palette.clone();

    // --- TC + level + objects (Step-2 setup) ---
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let color_anim = tc.color_anim.clone();
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
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
    state.game_mode = scenario.game_mode as u32;

    // --- render harness ---
    let mut bmp = render::bitmap::Bitmap::new(320, 200);
    let mut viewports = render::viewport::Viewport::player_layout();

    let render_tick = |bmp: &mut render::bitmap::Bitmap,
                       vps: &mut [render::viewport::Viewport],
                       state: &SimState,
                       tick: u32|
     -> u64 {
        render::frame::draw(bmp, state, &origpal, &color_anim, vps, 0);
        let fade = if tick == 0 { 0 } else { 33 };
        render::hash::hash_frame(bmp, fade)
    };

    let mut acc = render::hash::FNV_OFFSET;
    let mut frame_hashes: Vec<u64> = Vec::new();

    // tick 0: dumped BEFORE the first ProcessFrame (as the sim goldens).
    assert_eq!(frames[0].tick, 0);
    let fh0 = render_tick(&mut bmp, &mut viewports, &state, 0);
    assert_eq!(fh0, frames[0].frame_hash, "tick 0 frame hash");
    assert_eq!(
        hash_game_state(&state),
        frames[0].state_hash,
        "tick 0 state hash"
    );
    assert_eq!(
        frames[0].state_hash, sim_master[0],
        "tick 0 isolation vs sim golden"
    );
    acc = (acc ^ fh0).wrapping_mul(render::hash::FNV_PRIME);
    frame_hashes.push(fh0);

    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        let fh = render_tick(&mut bmp, &mut viewports, &state, k);
        let g = &frames[k as usize];
        assert_eq!(g.tick, k);
        assert_eq!(fh, g.frame_hash, "tick {k}: frame hash");
        assert_eq!(
            hash_game_state(&state),
            g.state_hash,
            "tick {k}: state hash"
        );
        assert_eq!(
            g.state_hash, sim_master[k as usize],
            "tick {k}: isolation vs sim golden"
        );
        acc = (acc ^ fh).wrapping_mul(render::hash::FNV_PRIME);
        frame_hashes.push(fh);
    }

    // total line
    assert_eq!(total_n, scenario.ticks + 1, "total frame count");
    assert_eq!(acc, total_acc, "total accumulator matches C++");

    // --- RotateFrom observability (non-vacuous, from the DRIVEN render) ---
    // Constant within an 8-tick window, changes on the cycles>>3 boundary.
    assert_eq!(
        frame_hashes[1], frame_hashes[7],
        "constant within [1,7] (dist 0)"
    );
    assert_ne!(
        frame_hashes[7], frame_hashes[8],
        "hash CHANGES at cycles>>3 boundary (tick 8)"
    );
    assert_eq!(frame_hashes[8], frame_hashes[9], "constant within [8,15]");
    assert_ne!(
        frame_hashes[8], frame_hashes[16],
        "hash changes again at tick 16"
    );
}
