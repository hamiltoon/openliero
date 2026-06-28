//! Slice-4c Task-5 driver INTEGRATION test — the slice's full-circle headline:
//! a projectile fired through `SimState::process_frame` flies, hits the dirt
//! floor, explodes, and the explosion's `small_explosion` SObject **and** its
//! thrown dirt-debris NObjects are now driven LIVE by the (newly un-no-op'd)
//! `sobjects` / `nobjects` Process loops.
//!
//! What Task 5 actually brings live is the cross-pool spawn ORDERING
//! (`game.cpp:334-355`, `sobjects → wobjects → nobjects → bobjects`):
//!
//! * the `small_explosion` SObject is appended by `blow_up` DURING the wobjects
//!   loop, but the sobjects loop already ran THIS tick → the sobject is NOT
//!   processed on its birth tick (`anim_delay`/`cur_frame` stay at their spawn
//!   values; first animation is next tick);
//! * the dirt-debris NObjects are appended by the same `blow_up` (via
//!   `sobject_create`'s dirt-throw) and the nobjects loop runs AFTER the wobjects
//!   loop → they ARE processed on their birth tick (the load-bearing
//!   "double-step": `Create2` moved them once, the birth-tick `Process` moves
//!   them again).
//!
//! The CLUSTER constants are the REAL TC data: `sobjects[2] = small_explosion`
//! (id 2, damage 5, dirtEffect 2) and `nobjects[2] = particle__disappearing`
//! (the dirt particle). The projectile is the real **dart** weapon with one
//! field normalized: `shot_type` is forced to `0` (ST_NORMAL). The shipped dart
//! is `shotType = 1` (ST_TYPE1), a branch `wobject_process` still defers in 4c;
//! every *other* real weapon that creates `small_explosion` trips a different
//! deferred guard (the guns set `leave_shell_timer`, whose expiry path is
//! unported; spikeballs has `bounce`/`numFrames`). Normalizing only `shot_type`
//! keeps every load-bearing dart constant — crucially `distribution = 0`,
//! `leaveShells = 0`, `timeToExploV = 0` (so Fire draws ZERO rand) and
//! `createOnExp = small_explosion` — while letting the 4c-incomplete projectile
//! Process path drive it ballistically. The subject under test (the object
//! loops + the real explosion cluster) is exercised end to end.

use assets::object::{Objects, Weapon};
use assets::tc::TcConfig;
use sim::control::ControlConsts;
use sim::hash::hash_components;
use sim::physics::PhysicsConsts;
use sim::pool::Pool;
use sim::state::{
    ControlState, NObject, SObject, SimState, WObject, WeaponInit, WormInit,
    MAT_BACKGROUND, MAT_DIRT, MAT_DIRT_ROCK, NUM_WEAPONS,
};
use sim::weapon::{blow_up, wobject_process, WObjectOutcome};
use sim_core::fixed::itof;
use sim_core::rng::Rand;
use sim_core::tables::precompute_cossin;
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const SEED: u32 = 42;
const FLOOR_Y: i32 = 200;
const WIDTH: i32 = 400;
const HEIGHT: i32 = 300;

/// The shipped 16x16 large-sprite bank, the crater mask/fill `draw_dirt_effect`
/// reads (same loader as the 4b greenball test).
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Load the full real object tables (cross-refs resolved, `id = index`) so
/// `sobjects[2] = small_explosion` and `nobjects[2] = particle__disappearing`
/// land at the indices `blow_up` / `sobject_create` hard-code.
fn load_objects(tc: &TcConfig) -> Objects {
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object tables load")
}

/// The real DART weapon with `shot_type` normalized to ST_NORMAL (see file
/// docs). `id` is set to 0 == its index in the one-element weapon table the
/// state holds; `createOnExp` is resolved against the real sobjects name list,
/// so it points at `small_explosion` (sobjects index 2).
fn dart_weapon(tc: &TcConfig) -> Weapon {
    let bytes = std::fs::read(format!("{TC_ROOT}/weapons/dart.cfg")).expect("read dart.cfg");
    let mut w = Weapon::load(
        &bytes,
        &tc.types.nobjects,
        &tc.types.sobjects,
        &tc.types.sounds,
    )
    .expect("dart loads");
    w.id = 0;
    w.shot_type = 0; // ST_NORMAL: drive the ballistic Process path (see file docs)
    w
}

/// A Background-sky-over-Dirt-floor level. Materials are discovered from the
/// real `tc.materials` flag table: sky is a pure Background id (shot flies),
/// floor a pure Dirt id (ground-explodes + has dirt for the debris throw).
fn build_level(tc: &TcConfig) -> (assets::level::LevelData, u8, u8) {
    let bg_id = (0u8..=255)
        .find(|&i| {
            let f = tc.materials[i as usize];
            f & MAT_BACKGROUND != 0 && f & MAT_DIRT_ROCK == 0
        })
        .expect("a pure Background material exists");
    let dirt_id = (0u8..=255)
        .find(|&i| {
            let f = tc.materials[i as usize];
            f & MAT_DIRT != 0 && f & MAT_BACKGROUND == 0
        })
        .expect("a pure Dirt material exists");

    // Sky everywhere, a solid dirt floor at the bottom, and a full-height dirt
    // wall on the LEFT. cossin[32] (the default left-facing aim) fires the dart
    // flat-LEFT; the weak dart gravity (200) barely bends it, so it would sail
    // off the left edge in open sky. The dirt wall gives it a dirt cell to
    // explode INTO — far (~85px) from the worm at x=100, so both worms stay out
    // of the 9x9 damage box while the dirt-throw still has dirt to scatter.
    const WALL_X: i32 = 15;
    let mut material_id = vec![bg_id; (WIDTH * HEIGHT) as usize];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if y >= FLOOR_Y || x < WALL_X {
                material_id[(y * WIDTH + x) as usize] = dirt_id;
            }
        }
    }
    (
        assets::level::LevelData {
            width: WIDTH,
            height: HEIGHT,
            material_id,
            palette: None,
            display: None,
        },
        bg_id,
        dirt_id,
    )
}

/// Build the `SimState`: one grounded visible worm holding the (normalized)
/// dart in slot 0 aimed flat-right so the shot arcs into the floor far from
/// either worm (both OUT of the 9x9 explosion box), plus an invisible idle
/// worm. Returns the state, the dart weapon (for the differential replay), and
/// the firing worm's start position.
fn fire_state() -> (SimState, Weapon, Vec2) {
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = load_objects(&tc);

    // Pin the cluster indices the explosion hard-codes.
    assert_eq!(
        objects.sobject_types[2].id_str, "small_explosion",
        "sobjects[2] is small_explosion"
    );
    assert_eq!(
        objects.nobject_types[2].id_str, "particle__disappearing",
        "nobjects[2] is the dirt particle"
    );

    let dart = dart_weapon(&tc);
    assert_eq!(
        dart.create_on_exp, 2,
        "dart createOnExp resolves to small_explosion (index 2)"
    );
    assert_eq!(dart.distribution, 0, "dart spread is 0 -> Fire draws no spread");
    assert_eq!(dart.leave_shells, 0, "dart leaves no shell -> no shell rand/timer");
    assert!(dart.expl_ground, "dart explodes on ground contact");
    assert_eq!(dart.splinter_amount, 0, "dart has no splinter scatter");

    let (level, bg_id, _dirt_id) = build_level(&tc);

    let start = Vec2::new(itof(100), itof(FLOOR_Y - 4));
    let mut weapons0 = [WeaponInit::default(); NUM_WEAPONS];
    weapons0[0] = WeaponInit {
        ty: Some(0),
        ammo: dart.ammo,
    };
    let worm0 = WormInit {
        index: 0,
        health: 100,
        lives: 5,
        stats_x: 0,
        weapons: weapons0,
        start_pos: start,
        visible: true,
    };
    let worm1 = WormInit {
        index: 1,
        health: 100,
        lives: 5,
        stats_x: 218,
        weapons: [WeaponInit::default(); NUM_WEAPONS],
        start_pos: Vec2::new(itof(350), itof(FLOOR_Y - 4)),
        visible: false,
    };

    let mut s = SimState::new(
        &level,
        &[worm0, worm1],
        SEED,
        &tc.materials,
        vec![dart.clone()],
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
    );

    // Aim flat-right at cossin index 32; in the default left-facing band so
    // process_aiming leaves it alone with no Up/Down input.
    s.worms[0].aiming_angle = itof(32);
    s.worms[0].direction = 0;

    assert!(
        s.level.material_flags[bg_id as usize] & MAT_BACKGROUND != 0,
        "sky id is Background"
    );
    assert!(
        s.level.dirt_rock(0, FLOOR_Y),
        "floor is solid so the dart ground-explodes"
    );

    (s, dart, start)
}

fn fire_input() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::FIRE);
    cs
}

fn no_fire() -> [ControlState; 2] {
    [ControlState::new(), ControlState::new()]
}

/// Snapshot the live objects of a pool in slot order (NObject/SObject are Copy).
fn snapshot<T: Copy>(pool: &Pool<T>) -> Vec<T> {
    pool.iter().copied().collect()
}

#[test]
fn dart_explosion_drives_sobject_and_dirt_debris_with_crosspool_ordering() {
    let cossin = precompute_cossin();
    let (mut s, dart, start) = fire_state();

    let level_hash_const = hash_components(&s).level;
    assert_eq!(s.cycles, 0, "cycles starts at 0");
    assert!(s.wobjects.is_empty() && s.sobjects.is_empty() && s.nobjects.is_empty());
    assert!(s.bobjects.is_empty(), "no blood objects");
    assert_eq!(s.rand.last(), 0, "no rand before firing");

    // ---- Fire tick: dart spawns, draws ZERO rand. -------------------------
    s.process_frame(&[fire_input(), ControlState::new()]);
    assert_eq!(s.cycles, 0, "process_frame never bumps cycles");
    assert_eq!(s.rand.last(), 0, "dart Fire draws no rand");
    assert_eq!(s.worms[0].weapons[0].ammo, dart.ammo - 1, "ammo decremented");
    assert_eq!(s.worms[0].weapons[0].delay_left, dart.delay, "delay_left = 30");
    assert_eq!(s.wobjects.len(), 1, "one dart projectile spawned");
    assert!(s.sobjects.is_empty() && s.nobjects.is_empty(), "no objects yet");

    // Birth-tick off-by-one: the wobjects loop ran before the worm loop, so the
    // shot sits at the un-advanced muzzle position.
    let birth = *s.wobjects.iter().next().expect("one wobject");
    let expected_birth = cossin[32]
        .mul(dart.detect_distance + 5)
        .add(start)
        .sub(Vec2::new(0, itof(1)));
    assert_eq!(birth.pos, expected_birth, "birth tick: shot at firing position");

    // ---- Fly ticks: arc under gravity until the floor; nothing draws/digs. --
    let mut prev = birth;
    let mut level_before = s.level.clone();
    let mut pre_explode_wobj = birth;
    let mut explode_tick = None;
    for tick in 1..2000 {
        let lvl_snapshot = s.level.clone();
        let wobj_snapshot = *s.wobjects.iter().next().expect("alive wobject");
        s.process_frame(&no_fire());
        assert_eq!(s.cycles, 0, "cycles stays 0 (tick {tick})");
        assert!(s.bobjects.is_empty(), "no blood (tick {tick})");

        if s.wobjects.is_empty() {
            explode_tick = Some(tick);
            level_before = lvl_snapshot;
            pre_explode_wobj = wobj_snapshot;
            break;
        }

        assert_eq!(s.rand.last(), 0, "no rand while flying (tick {tick})");
        assert_eq!(s.nobjects.len(), 0, "no debris while flying (tick {tick})");
        assert_eq!(s.sobjects.len(), 0, "no sobject while flying (tick {tick})");
        assert_eq!(
            hash_components(&s).level,
            level_hash_const,
            "level pristine while flying (tick {tick})"
        );
        let cur = *s.wobjects.iter().next().expect("alive wobject");
        assert_eq!(cur.pos, prev.pos.add(prev.vel), "fly tick {tick}: pos += vel");
        assert_eq!(
            cur.vel.y,
            prev.vel.y + dart.gravity,
            "fly tick {tick}: vel.y grows by gravity (200)"
        );
        prev = cur;
    }
    let explode_tick = explode_tick.expect("dart must hit the floor and explode");
    assert!(explode_tick >= 3, "a multi-tick arc, not an instant stab");

    // ===== The explode tick: object loops go live. =========================
    assert!(s.wobjects.is_empty(), "the dart is gone");

    // (a) The SObject: present and NOT processed its birth tick. The sobjects
    //     loop ran BEFORE blow_up spawned it, so anim_delay/cur_frame are still
    //     at their spawn values (a processed sobject would have anim_delay 1).
    assert_eq!(s.sobjects.len(), 1, "one small_explosion sobject spawned");
    let sob = *s.sobjects.iter().next().expect("the sobject");
    let small_expl_anim_delay = s.sobject_types[2].anim_delay;
    let small_expl_num_frames = s.sobject_types[2].num_frames;
    assert_eq!(sob.id, 2, "sobject id = small_explosion (2)");
    assert_eq!(sob.cur_frame, 0, "sobject NOT animated on its birth tick");
    assert_eq!(
        sob.anim_delay, small_expl_anim_delay,
        "sobject anim_delay untouched (sobjects loop ran before the spawn)"
    );

    // (b) The dirt-debris: spawned and PROCESSED their birth tick. Carved.
    assert!(!s.nobjects.is_empty(), "dirt debris spawned");
    assert_ne!(s.rand.last(), 0, "explosion cluster advanced the rng");
    assert_ne!(
        hash_components(&s).level,
        level_hash_const,
        "explosion carved terrain"
    );
    assert_eq!(s.cycles, 0, "cycles still 0");
    assert!(s.bobjects.is_empty(), "still no blood objects");

    // ---- Differential proof of the cross-pool ordering. -------------------
    // Fire + flight drew ZERO rand, so at the explode tick the RNG was still
    // pristine (== fresh seed). Replay JUST the wobjects-loop explosion
    // (wobject_process -> blow_up) on a fresh seed and the pre-explode level:
    // that yields the nobjects pool EXACTLY as it stood immediately after
    // blow_up, i.e. BEFORE the nobjects loop ran. The real pool (post nobjects
    // loop) must differ -> the loop processed the freshly born debris.
    let mut r2 = Rand::new();
    r2.seed(SEED);
    let mut level2 = level_before.clone();
    let mut worms2: Vec<sim::state::WormState> = Vec::new();
    let mut wobjects2: Pool<WObject> = Pool::new(600);
    let mut nobjects2: Pool<NObject> = Pool::new(600);
    let mut sobjects2: Pool<SObject> = Pool::new(700);

    let mut obj = pre_explode_wobj;
    assert_eq!(
        wobject_process(&mut obj, &level_before, &dart, &mut r2),
        WObjectOutcome::Explode,
        "the captured dart explodes on the floor"
    );
    blow_up(
        &dart,
        &mut level2,
        &s.large_sprites,
        &s.textures,
        obj.pos,
        0, // fired by worm 0
        &s.sobject_types,
        &s.nobject_types,
        &cossin,
        &mut worms2,
        &mut wobjects2,
        std::slice::from_ref(&dart),
        &mut nobjects2,
        &mut sobjects2,
        &mut r2,
    );

    let debris_preloop = snapshot(&nobjects2);
    assert!(
        !debris_preloop.is_empty(),
        "the explosion threw dirt debris (the differential needs >=1)"
    );
    // Same rand draws reached the same place: corroborates the replay fidelity
    // (the nobjects loop + worm loop draw zero rand for this cluster).
    assert_eq!(
        r2.last(),
        s.rand.last(),
        "replayed explosion drew the same rand stream as the live tick"
    );
    // The sobject is identical in both (sobjects loop did not touch the new one).
    assert_eq!(snapshot(&sobjects2), snapshot(&s.sobjects), "sobject matches replay");

    // THE PIN: the live debris differ from the just-spawned (pre-loop) debris,
    // because the nobjects loop ran AFTER the wobjects loop and processed them.
    let debris_live = snapshot(&s.nobjects);
    assert_ne!(
        debris_live, debris_preloop,
        "nobjects loop processed the birth-tick debris (cross-pool ordering)"
    );

    // ---- Later ticks: sobject animates 0->5 then frees (~12 ticks); debris
    //      fall and free. No further rand is drawn (the cluster was a one-shot).
    let rng_after_explode = s.rand.last();
    let mut max_cur_frame = sob.cur_frame;
    let mut sobject_gone_at = None;
    let mut debris_gone_at = None;
    for extra in 1..400 {
        s.process_frame(&no_fire());
        assert_eq!(s.cycles, 0, "cycles stays 0 (post-explode {extra})");
        assert!(s.bobjects.is_empty(), "still no blood (post-explode {extra})");
        assert_eq!(
            s.rand.last(),
            rng_after_explode,
            "no rand after the explosion (post-explode {extra})"
        );
        if let Some(o) = s.sobjects.iter().next() {
            max_cur_frame = max_cur_frame.max(o.cur_frame);
        } else if sobject_gone_at.is_none() {
            sobject_gone_at = Some(extra);
        }
        if s.nobjects.is_empty() && debris_gone_at.is_none() {
            debris_gone_at = Some(extra);
        }
        if sobject_gone_at.is_some() && debris_gone_at.is_some() {
            break;
        }
    }

    assert_eq!(
        max_cur_frame, small_expl_num_frames,
        "sobject animated up to its last frame (5) before freeing"
    );
    let sobject_gone_at = sobject_gone_at.expect("sobject must free after animating");
    assert!(
        (10..=14).contains(&sobject_gone_at),
        "sobject frees ~12 ticks after birth (saw {sobject_gone_at})"
    );
    assert!(
        debris_gone_at.is_some(),
        "all dirt debris eventually fall and free"
    );
}

#[test]
fn no_fire_keeps_object_pools_and_level_pristine() {
    // Step 2 guard: with the dart loaded but Fire NEVER pressed, the new
    // sobjects/nobjects loops add no spurious spawn or write — the pools stay
    // empty and the level hash stays at its tick-0 value.
    let (mut s, _dart, _start) = fire_state();
    let level_hash_const = hash_components(&s).level;

    for tick in 0..60 {
        s.process_frame(&no_fire());
        assert!(s.wobjects.is_empty(), "wobjects empty (tick {tick})");
        assert!(s.sobjects.is_empty(), "sobjects empty (tick {tick})");
        assert!(s.nobjects.is_empty(), "nobjects empty (tick {tick})");
        assert!(s.bobjects.is_empty(), "bobjects empty (tick {tick})");
        assert_eq!(s.cycles, 0, "cycles stays 0 (tick {tick})");
        assert_eq!(s.rand.last(), 0, "no rand drawn (tick {tick})");
        assert_eq!(
            hash_components(&s).level,
            level_hash_const,
            "level pristine (tick {tick})"
        );
    }
}
