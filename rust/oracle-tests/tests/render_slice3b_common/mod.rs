//! Shared harness for the Slice-3b per-scenario FRAME-HASH golden tests (T8).
//!
//! Each `render_slice3b_<name>_golden.rs` integration test declares
//! `mod render_slice3b_common;` and calls [`run`] to drive the Step-2 `SimState`
//! tick-by-tick, render the FULL world view per tick with the `render` crate
//! (terrain + shadows + every object family + worms + ninjarope + laser +
//! crosshair + blood), and match the committed C++ sidecar golden
//! (`golden/render_slice3b_<name>.txt`) **line-for-line INCLUDING the `total`
//! line**. Column 3 (`state_hash`) is asserted against the Rust
//! `hash_game_state` (rendering is a pure consumer) AND the sim golden's master
//! column (`render_slice3b_<name>_sim.txt`) — the isolation triple.
//!
//! [`modified_frame_hash`] re-drives a scenario to a target tick and renders it
//! with a controlled modification (shadow flag flipped, or one object pool
//! emptied) — the non-vacuity controls that prove the trap families are LIVE.
//!
//! ## The fixed-camera invariant (load-bearing — T7 report)
//!
//! Every 3b worm keeps `killed_timer == 150` (the `SimState::new` default) and
//! `health 100`, so `viewport::process` takes NEITHER centering arm and the
//! camera stays pinned at the constructed origin (0,0). The harness therefore
//! MUST NOT reset `killed_timer` — leaving the `WormInit` default is what keeps
//! the visible world window fixed at `[0,158)²` so the frame matches C++.

#![allow(dead_code)] // each test binary uses a subset of the shared surface.

use assets::object::Objects;
use assets::palette::Palette;
use assets::sprite::SpriteSet;
use assets::tc::{ColorAnim, TcConfig};
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::hash_game_state;
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::fixed::itof;
use sim_core::vec::Vec2;

use render::bitmap::Bitmap;
use render::fire_cone::build_fire_cone_sprites;
use render::frame::Scene;
use render::viewport::Viewport;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

fn load_sprites(file: &str, w: i32, h: i32, count: i32) -> SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/{file}")).unwrap_or_else(|e| {
        panic!("read sprites/{file}: {e}");
    });
    let tga = assets::sprite::Tga::load(&bytes).unwrap_or_else(|_| panic!("{file} parses"));
    SpriteSet::from_tga(&tga, w, h, count).unwrap_or_else(|_| panic!("{file} sprite bank"))
}

/// One parsed frame-sidecar line: `<tick> <frame_hash_hex16> <state_hash_hex8>`.
pub struct FrameLine {
    pub tick: u32,
    pub frame_hash: u64,
    pub state_hash: u32,
}

/// Parse a frame sidecar into (per-tick lines, total_count, total_accumulator).
/// Identical grammar to the 3a golden (`render_slice3a_golden.rs::parse_frames`).
pub fn parse_frames(text: &str) -> (Vec<FrameLine>, u32, u64) {
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

fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/golden/render_slice3b_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// Which object pool to empty for a control render (non-vacuity proof that the
/// pool's sprites actually paint pixels).
#[derive(Clone, Copy)]
pub enum Drop {
    None,
    Wobjects,
    Nobjects,
    Sobjects,
    Bobjects,
}

/// Everything built once per scenario: the driven sim, the render surface, the
/// two viewports (fresh, default-seeded RNG), and the owned scene ingredients.
struct Built {
    scenario: Scenario,
    state: SimState,
    viewports: [Viewport; 2],
    bmp: Bitmap,
    origpal: Palette,
    color_anim: Vec<ColorAnim>,
    fire_cone: SpriteSet,
    nr_begin: i32,
    nr_end: i32,
    laser_weapon: i32,
}

/// Build tick-0 state + render harness for a 3b scenario — the full Step-2 setup
/// copied from `render_slice3a_golden.rs`, plus the 5b-style `weapon 0 <name>`
/// override applied to BOTH worms (so their 5-slot weapon hash matches the
/// dumper), plus the fire-cone bank the sprite pass needs.
fn build(name: &str) -> Built {
    let scenario_text = read_golden(name, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "3b scenarios all use seed 42");

    // Origpal = small.tga's embedded palette (C++ common.exepal), as in 3a.
    let small_bytes =
        std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let small_tga = assets::sprite::Tga::load(&small_bytes).expect("small.tga parses");
    let origpal = small_tga.palette.clone();

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

    // weap_order: indices sorted by weapon name; id == index (Common::Precompute).
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // Override slot 0 with the scenario's `weapon 0 <name>` (FAN/DART/RIFLE),
    // applied to BOTH worms — mirrors the C++ dumper's ResolveWeapon (5b-style).
    let weapon_name = scenario
        .weapon(0)
        .expect("3b scenario has a `weapon 0 <name>` directive");
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
        load_sprites("large.tga", 16, 16, 110),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        0,
        true,
        100,
    );
    // TC scalars defaulted to 0 by `new` (as in 3a/5b) — the blood/spawn consts.
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_sprites("small.tga", 7, 7, 130);
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state.game_mode = scenario.game_mode as u32;

    // NOTE: killed_timer is left at its `WormInit` default (150) — the camera
    // stays pinned at (0,0). Resetting it would centre the viewport and diverge.

    let fire_cone = build_fire_cone_sprites(&state.large_sprites);

    Built {
        scenario,
        state,
        viewports: Viewport::player_layout(),
        bmp: Bitmap::new(320, 200),
        origpal,
        color_anim,
        fire_cone,
        nr_begin: tc.constants.NRColourBegin,
        nr_end: tc.constants.NRColourEnd,
        laser_weapon: tc.constants.LaserWeapon,
    }
}

/// Render one tick's frame off the current `Built` state, mirroring the dumper's
/// per-tick semantics: inject `render_shake` (`shake = itof(amount)`) for the
/// draw and restore it after, feed `render_flash` into `Scene.screen_flash`, and
/// gate the shadow pass on `render_shadow` (unless `force_shadow` overrides it).
fn render_tick(b: &mut Built, tick: u32, force_shadow: Option<bool>) -> u64 {
    let draw_shadow = force_shadow.unwrap_or(b.scenario.shadow());
    let screen_flash = b.scenario.flash_at(tick).unwrap_or(0);

    // Inject shake for THIS draw (set before, restore after — dumper semantics).
    let shakes = b.scenario.shake_at(tick);
    for &(vp, amount) in &shakes {
        b.viewports[vp].shake = itof(amount);
    }

    let scene = Scene {
        origpal: &b.origpal,
        color_anim: &b.color_anim,
        fire_cone_sprites: &b.fire_cone,
        bonus_frames: &[],
        nr_begin: b.nr_begin,
        nr_end: b.nr_end,
        laser_weapon: b.laser_weapon,
        screen_flash,
        draw_shadow,
    };
    render::frame::draw(&mut b.bmp, &b.state, &mut b.viewports, &scene);

    // Restore shake so the next tick's Process is clean (mirror the dumper).
    for &(vp, _) in &shakes {
        b.viewports[vp].shake = 0;
    }

    let fade = if tick == 0 { 0 } else { 33 };
    render::hash::hash_frame(&b.bmp, fade)
}

/// The outcome of a full scenario run: the per-tick frame hashes (for the
/// non-vacuity asserts), each viewport's accumulated RNG draw count, and the
/// per-tick object-pool occupancy `[wob, sob, nob, bob]`.
pub struct RunResult {
    pub ticks: u32,
    pub frame_hashes: Vec<u64>,
    pub vp_rand_draws: [u64; 2],
    /// `[wobjects, sobjects, nobjects, bobjects]` live counts per tick.
    pub pool_lens: Vec<[usize; 4]>,
}

impl RunResult {
    /// The tick index (in `0..=ticks`) with the most live objects in the given
    /// pool. Panics if the pool is empty on every tick (the scenario never
    /// exercised that family — a non-vacuity control would be meaningless).
    pub fn peak_tick(&self, drop: Drop) -> u32 {
        let idx = match drop {
            Drop::Wobjects => 0,
            Drop::Sobjects => 1,
            Drop::Nobjects => 2,
            Drop::Bobjects => 3,
            Drop::None => panic!("peak_tick needs a real pool"),
        };
        let (t, max) = self
            .pool_lens
            .iter()
            .enumerate()
            .map(|(t, lens)| (t as u32, lens[idx]))
            .max_by_key(|&(_, n)| n)
            .expect("at least one tick");
        assert!(max > 0, "pool was empty on every tick (nothing to drop)");
        t
    }
}

/// Drive the scenario tick-by-tick, render the full world per tick, and match
/// the committed sidecar golden line-for-line + `total` + the isolation triple
/// (`state_hash` column == `hash_game_state` == sim golden master). Returns the
/// [`RunResult`] for the caller's scenario-specific non-vacuity asserts.
pub fn run(name: &str) -> RunResult {
    let mut b = build(name);
    let ticks = b.scenario.ticks;

    let (frames, total_n, total_acc) = parse_frames(&read_golden(name, ".txt"));
    assert_eq!(
        frames.len(),
        (ticks + 1) as usize,
        "{name}: one frame line per tick 0..=ticks"
    );

    // Isolation source: the sim golden's master column (2nd column, hex).
    let sim_text = read_golden(name, "_sim.txt");
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
        (ticks + 1) as usize,
        "{name}: sim golden has one line per tick 0..=ticks"
    );

    let mut acc = render::hash::FNV_OFFSET;
    let mut frame_hashes = Vec::with_capacity((ticks + 1) as usize);
    let mut pool_lens = Vec::with_capacity((ticks + 1) as usize);

    let record_pools = |st: &SimState| {
        [
            st.wobjects.len(),
            st.sobjects.len(),
            st.nobjects.len(),
            st.bobjects.len(),
        ]
    };

    // tick 0: rendered/hashed BEFORE the first ProcessFrame (as the sim goldens).
    assert_eq!(frames[0].tick, 0, "{name}: first golden line is tick 0");
    let fh0 = render_tick(&mut b, 0, None);
    assert_eq!(fh0, frames[0].frame_hash, "{name} tick 0: frame hash");
    assert_eq!(
        hash_game_state(&b.state),
        frames[0].state_hash,
        "{name} tick 0: state hash == hash_game_state"
    );
    assert_eq!(
        frames[0].state_hash, sim_master[0],
        "{name} tick 0: isolation vs sim golden"
    );
    acc = (acc ^ fh0).wrapping_mul(render::hash::FNV_PRIME);
    frame_hashes.push(fh0);
    pool_lens.push(record_pools(&b.state));

    for k in 1..=ticks {
        let inputs = [
            ControlState::unpack(b.scenario.input(k - 1, 0)),
            ControlState::unpack(b.scenario.input(k - 1, 1)),
        ];
        b.state.process_frame(&inputs);
        let fh = render_tick(&mut b, k, None);
        let g = &frames[k as usize];
        assert_eq!(g.tick, k, "{name}: golden tick column");
        assert_eq!(fh, g.frame_hash, "{name} tick {k}: frame hash");
        assert_eq!(
            hash_game_state(&b.state),
            g.state_hash,
            "{name} tick {k}: state hash == hash_game_state"
        );
        assert_eq!(
            g.state_hash, sim_master[k as usize],
            "{name} tick {k}: isolation vs sim golden master"
        );
        acc = (acc ^ fh).wrapping_mul(render::hash::FNV_PRIME);
        frame_hashes.push(fh);
        pool_lens.push(record_pools(&b.state));
    }

    assert_eq!(total_n, ticks + 1, "{name}: total frame count");
    assert_eq!(acc, total_acc, "{name}: total accumulator matches C++");

    RunResult {
        ticks,
        frame_hashes,
        vp_rand_draws: [b.viewports[0].rand.draws(), b.viewports[1].rand.draws()],
        pool_lens,
    }
}

/// Re-drive the scenario to `target_tick` and render THAT tick with a controlled
/// modification — the shadow flag forced (`force_shadow`) and/or one object pool
/// emptied (`drop`) just before the draw. Used by the non-vacuity controls:
/// dropping a pool that genuinely paints changes the frame hash; forcing shadow
/// off changes a shadow-window frame. The sim is driven identically to [`run`]
/// (draw-only shake/flash never perturb the sim), so `target_tick`'s state
/// matches the main run's; the returned hash is the MODIFIED render.
pub fn modified_frame_hash(
    name: &str,
    target_tick: u32,
    force_shadow: Option<bool>,
    drop: Drop,
) -> u64 {
    let mut b = build(name);
    assert!(target_tick <= b.scenario.ticks, "target tick in range");
    for k in 1..=target_tick {
        let inputs = [
            ControlState::unpack(b.scenario.input(k - 1, 0)),
            ControlState::unpack(b.scenario.input(k - 1, 1)),
        ];
        b.state.process_frame(&inputs);
    }

    // Empty the requested pool IN PLACE (no Clone on SimState): swap in a fresh
    // empty pool of the same capacity. Rendering is a pure consumer, so the only
    // effect is that the pool's sprites are not drawn this frame.
    match drop {
        Drop::None => {}
        Drop::Wobjects => {
            let cap = b.state.wobjects.capacity();
            let _ = std::mem::replace(&mut b.state.wobjects, sim::pool::Pool::new(cap));
        }
        Drop::Nobjects => {
            let cap = b.state.nobjects.capacity();
            let _ = std::mem::replace(&mut b.state.nobjects, sim::pool::Pool::new(cap));
        }
        Drop::Sobjects => {
            let cap = b.state.sobjects.capacity();
            let _ = std::mem::replace(&mut b.state.sobjects, sim::pool::Pool::new(cap));
        }
        Drop::Bobjects => {
            let cap = b.state.bobjects.capacity();
            let _ = std::mem::replace(&mut b.state.bobjects, sim::pool::BloodPool::new(cap));
        }
    }

    render_tick(&mut b, target_tick, force_shadow)
}

/// Count of distinct values in a hash slice (for the laser RNG non-vacuity).
pub fn distinct(hashes: &[u64]) -> usize {
    let mut v: Vec<u64> = hashes.to_vec();
    v.sort_unstable();
    v.dedup();
    v.len()
}
