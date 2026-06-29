//! Slice-4b driver INTEGRATION test — the headline of the slice: a greenball
//! fired through `SimState::process_frame` flies a gravity parabola, hits the
//! dirt floor, explodes, and **writes terrain** — making the `level` component
//! hash a *time series* for the FIRST time in the port's history.
//!
//! Slices 1-4a never moved the `level` hash: it folds `material_id`, and nothing
//! before 4b dug. The fan (4a) had `dirt_effect = -1`, so its `blow_up` was inert
//! and the 4a golden's `level` column is constant on every line. Greenball's
//! `dirt_effect = 6` is the first explosion that calls `draw_dirt_effect` and
//! mutates `material_id` — so the tick where the wobject pool goes 1->0 is also
//! the tick the `level` hash jumps. This test drives the whole
//! fire -> fly -> explode -> terrain chain end-to-end and pins exactly that jump.
//!
//! It exercises the Task-2 wiring (the Explode arm of the wobjects loop threads
//! `large_sprites`/`textures` from `SimState` into `blow_up`): without those real
//! assets reaching `draw_dirt_effect`, the crater would not be stamped and the
//! `level` hash would not move.

use assets::object::Weapon;
use assets::tc::TcConfig;
use sim::control::ControlConsts;
use sim::hash::hash_components;
use sim::physics::PhysicsConsts;
use sim::state::{
    ControlState, SimState, WeaponInit, WormInit, MAT_BACKGROUND, MAT_DIRT, MAT_DIRT_ROCK,
    NUM_WEAPONS,
};
use sim_core::fixed::itof;
use sim_core::rng::Rand;
use sim_core::tables::precompute_cossin;
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const SEED: u32 = 42;

/// The real greenball weapon, loaded from the shipped TC config. The cross-ref
/// lists are empty: greenball omits `createOnExp`/`splinterType`/`objTrailType`/
/// `partTrailObj`, so every ObjRefFromStr resolves to the `-1` sentinel exactly
/// as an empty-list load yields — which is what the deferred-branch `debug_assert`s
/// in `wobject_process`/`blow_up` require. `id` is set by the caller (== its index
/// in the one-element weapon table the state holds).
fn greenball_weapon(id: i32) -> Weapon {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/weapons/greenball.cfg"
    ));
    let mut w = Weapon::load(bytes, &[], &[], &[]).unwrap();
    w.id = id;
    w
}

/// The shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`)
/// from `sprites/large.tga` — the real crater mask/fill `draw_dirt_effect` reads.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Build a `SimState` with a Background-sky-over-Dirt-floor level and a grounded,
/// visible worm holding greenball in slot 0 (aiming flat-right so the shot arcs
/// over open sky and descends into the floor). The second worm is invisible/idle.
///
/// The Background/Dirt material *ids* are discovered from the real `tc.materials`
/// flag table (no hard-coded magic numbers): the floor uses the first id flagged
/// Dirt (so `dirt_rock` is true -> a ground hit explodes greenball, whose
/// `explGround = true`), the sky the first id flagged Background (so the additive
/// `draw_dirt_effect` path has Background cells to fill).
fn fire_state() -> (SimState, Weapon, Vec2, u8, u8) {
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    assert!(
        tc.textures.len() > 6,
        "greenball dirt_effect=6 indexes the texture table"
    );

    // Pick PURE materials: some TC materials carry combined flags (e.g. id 1 is
    // Background|Dirt), which would make the "sky" solid. The sky must be
    // Background and NOT DirtRock (so the shot flies); the floor Dirt and NOT
    // Background (so it ground-explodes and is not itself dug as "background").
    let bg_id = (0u8..=255)
        .find(|&i| {
            let f = tc.materials[i as usize];
            f & MAT_BACKGROUND != 0 && f & MAT_DIRT_ROCK == 0
        })
        .expect("a pure Background material exists in the TC");
    let dirt_id = (0u8..=255)
        .find(|&i| {
            let f = tc.materials[i as usize];
            f & MAT_DIRT != 0 && f & MAT_BACKGROUND == 0
        })
        .expect("a pure Dirt material exists in the TC");

    // Sky above, solid dirt floor at y >= FLOOR_Y. Wide enough that the flat shot
    // never leaves the level before its parabola reaches the floor.
    let width = 400i32;
    let height = 300i32;
    const FLOOR_Y: i32 = 200;
    let mut material_id = vec![bg_id; (width * height) as usize];
    for y in FLOOR_Y..height {
        for x in 0..width {
            material_id[(y * width + x) as usize] = dirt_id;
        }
    }
    let level = assets::level::LevelData {
        width,
        height,
        material_id,
        palette: None,
        display: None,
    };

    let greenball = greenball_weapon(0);

    // worm0: grounded just above the floor, greenball in slot 0 with real ammo.
    let start = Vec2::new(itof(100), itof(FLOOR_Y - 4));
    let mut weapons0 = [WeaponInit::default(); NUM_WEAPONS];
    weapons0[0] = WeaponInit {
        ty: Some(0),
        ammo: greenball.ammo,
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
    // worm1: invisible & inert (its `if (visible)` arm never runs), far away.
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
        vec![greenball.clone()],
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        Vec::new(),
        Vec::new(),
        100,
        true,
    );

    // Aim flat-right at cossin index 32 (== (+max, 0); see precompute_cossin). It
    // sits inside the default left-facing aim band [12,64], so process_aiming
    // leaves it untouched with no Up/Down input -> the firing index is exactly 32.
    // The shot leaves the muzzle horizontal and gravity (700) bends it down into
    // the floor over many ticks: a genuine parabola, unlike the fan's straight line.
    s.worms[0].aiming_angle = itof(32);
    s.worms[0].direction = 0;

    // Sanity: the level we built really is Background-over-Dirt under the real flags.
    assert!(
        s.level.material_flags[bg_id as usize] & MAT_BACKGROUND != 0,
        "sky id is Background-flagged"
    );
    assert!(
        s.level.dirt_rock(0, FLOOR_Y),
        "floor pixel is solid (DirtRock) so greenball ground-explodes"
    );
    assert!(
        !s.level.dirt_rock(0, FLOOR_Y - 10),
        "sky pixel is NOT solid so the shot flies before it hits"
    );

    (s, greenball, start, bg_id, dirt_id)
}

fn fire_input() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::FIRE);
    cs
}

#[test]
fn greenball_fire_fly_explode_writes_terrain_and_moves_level_hash() {
    let cossin = precompute_cossin();
    let (mut s, greenball, start, _bg, _dirt) = fire_state();

    // The level hash is CONSTANT in slices 1-4a; capture its tick-0 value — every
    // pre-explode tick must still read this, and only the explosion may move it.
    let level_hash_const = hash_components(&s).level;

    assert_eq!(s.cycles, 0, "cycles starts at 0");
    assert!(s.wobjects.is_empty(), "no wobjects before firing");
    assert_eq!(s.rand.last(), 0, "no rand drawn before firing");

    // An oracle RNG mirroring greenball's draws, in C++ order. Comparing
    // `last()` (the raw next_u32, independent of bound()'s max) proves the *number*
    // of draws the sim made matches the oracle at each checkpoint.
    let mut oracle = Rand::new();
    oracle.seed(SEED);

    // --- Fire tick: the Fire gate trips, worm_fire spawns one greenball. -------
    s.process_frame(&[fire_input(), ControlState::new()]);

    assert_eq!(s.cycles, 1, "process_frame increments cycles once (game.cpp:357)");
    assert_eq!(
        s.worms[0].weapons[0].ammo,
        greenball.ammo - 1,
        "ammo decremented by Fire"
    );
    assert_eq!(
        s.worms[0].weapons[0].delay_left, greenball.delay,
        "delay_left = greenball.delay (4)"
    );

    // greenball Fire draws exactly THREE rands: spread-x rand(distribution*2),
    // spread-y rand(distribution*2), colour rand(2) (start_frame=-1 path). No
    // leading leave-shell draw (leave_shells=0) and no time-var draw (time_to_explo_v=0).
    oracle.bound((greenball.distribution * 2) as u32);
    oracle.bound((greenball.distribution * 2) as u32);
    oracle.bound(2);
    let rng_after_fire = oracle.last();
    assert_ne!(s.rand.last(), 0, "Fire drew RNG");
    assert_eq!(
        s.rand.last(),
        rng_after_fire,
        "Fire drew exactly the 3 greenball rands (spread x, spread y, colour)"
    );

    assert_eq!(
        s.wobjects.len(),
        1,
        "exactly one greenball projectile spawned"
    );
    let birth = *s.wobjects.iter().next().expect("one wobject");

    // THE off-by-one: the wobjects loop ran BEFORE the worm loop this tick, so the
    // freshly-spawned shot was NOT walked — its pos is the un-advanced muzzle.
    // firing_pos = cossin[32]*(detect_distance+5) + worm.pos - (0, Itof(1));
    // worm.pos at fire time == start (fire is step 8, BEFORE the physics of step 9).
    let expected_birth = cossin[32]
        .mul(greenball.detect_distance + 5)
        .add(start)
        .sub(Vec2::new(0, itof(1)));
    assert_eq!(
        birth.pos, expected_birth,
        "birth tick: shot sits at the firing position (NOT advanced by vel)"
    );

    // --- Fly ticks: the shot arcs down under gravity until it hits the floor. --
    // Each free-air tick: pos += vel, then vel.y += gravity (the parabola). The
    // level hash and RNG both stay frozen — nothing digs or draws while it flies.
    let mut prev = birth;
    let mut flight_ticks = 0usize;
    let mut explode_tick = None;
    let mut level_before_explode = None;
    for tick in 1..2000 {
        let level_before = hash_components(&s).level;
        s.process_frame(&[ControlState::new(), ControlState::new()]);
        // cycles advances once per process_frame: the fire tick (call 1) plus this
        // many fly ticks => cycles == tick + 1.
        assert_eq!(s.cycles, tick + 1, "cycles advances once per tick (tick {tick})");

        if s.wobjects.is_empty() {
            // The pool went 1 -> 0: this is the explode tick (greenball has no
            // timeout, so the only way out is a ground hit -> Explode -> blow_up).
            explode_tick = Some(tick);
            level_before_explode = Some(level_before);
            break;
        }

        // A genuine free-flight tick.
        assert_eq!(
            hash_components(&s).level,
            level_hash_const,
            "level hash CONSTANT across pre-explode tick {tick} (nothing dug yet)"
        );
        assert_eq!(
            s.rand.last(),
            rng_after_fire,
            "no rng drawn while the shot flies (tick {tick})"
        );

        let cur = *s.wobjects.iter().next().expect("alive wobject");
        assert_eq!(
            cur.pos,
            prev.pos.add(prev.vel),
            "fly tick {tick}: pos advances by exactly vel"
        );
        assert_eq!(
            cur.vel.x, prev.vel.x,
            "fly tick {tick}: horizontal velocity unchanged"
        );
        assert_eq!(
            cur.vel.y,
            prev.vel.y + greenball.gravity,
            "fly tick {tick}: vel.y grows by gravity (700) -> a downward parabola"
        );
        assert!(
            cur.vel.y > prev.vel.y,
            "fly tick {tick}: the arc is bending DOWNWARD"
        );
        prev = cur;
        flight_ticks += 1;
    }

    let explode_tick = explode_tick.expect("greenball must hit the floor and explode");
    let level_before = level_before_explode.expect("captured the pre-explode level hash");

    // Multi-tick arc, not an instant ground-stab: prove the parabola really flew.
    assert!(
        flight_ticks >= 3,
        "the shot must fly a multi-tick arc before impact (saw {flight_ticks})"
    );

    // THE HEADLINE: the level hash was CONSTANT every pre-explode tick, then MOVED
    // on the explode tick — the first time in the port the `level` fold is a time
    // series. The terrain write is `blow_up`'s `draw_dirt_effect` crater, fed the
    // real large_sprites/textures threaded through process_frame (Task 2 wiring).
    assert_eq!(
        level_before, level_hash_const,
        "level hash was constant right up to the explode tick"
    );
    let level_after = hash_components(&s).level;
    assert_ne!(
        level_after, level_hash_const,
        "explode tick {explode_tick}: terrain written -> level hash MOVED"
    );

    // The explode tick drew EXACTLY one more rand — draw_dirt_effect's rand(rframe);
    // wobject_process draws none, and the idle worm draws none. (greenball
    // create_on_exp/-1 + splinter 0 means no other explosion draws.)
    oracle.bound(2);
    assert_eq!(
        s.rand.last(),
        oracle.last(),
        "explode tick drew exactly one more rand (the dirt-effect draw, no other)"
    );

    // cycles advanced exactly once per process_frame: 1 fire tick + explode_tick
    // fly/explode ticks.
    assert_eq!(
        s.cycles,
        explode_tick + 1,
        "cycles advanced once per process_frame call across the whole sequence"
    );
    assert!(s.wobjects.is_empty(), "pool empty after the explosion");
}

#[test]
fn no_fire_keeps_level_pristine() {
    // Step 2 guard: with greenball loaded but Fire NEVER pressed, nothing digs, so
    // the `level` hash stays at its tick-0 value for the whole run — proving the
    // 4b wiring did not introduce a spurious terrain write.
    let (mut s, _greenball, _start, _bg, _dirt) = fire_state();
    let level_hash_const = hash_components(&s).level;

    for tick in 0..60 {
        s.process_frame(&[ControlState::new(), ControlState::new()]);
        assert!(s.wobjects.is_empty(), "wobjects empty (tick {tick})");
        assert_eq!(s.cycles, tick + 1, "cycles advances once per tick (tick {tick})");
        assert_eq!(s.rand.last(), 0, "no Fire -> no rand drawn (tick {tick})");
        assert_eq!(
            hash_components(&s).level,
            level_hash_const,
            "no explosion -> level hash stays pristine (tick {tick})"
        );
    }
}
