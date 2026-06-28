//! Driver-level tests for the Slice-4a `SimState::process_frame` additions: the
//! object-Process loops (the wobjects walk) that run BEFORE the worm loop, and
//! the per-worm Fire gate (`worm.cpp:336-339`) wired between `process_weapons`
//! and `worm_process_physics`.
//!
//! Step 1 (fire -> fly -> explode) exercises the load-bearing off-by-one: the
//! object loop runs before the worm loop, so a shot spawned by Fire (in the worm
//! loop) is NOT walked on its birth tick — its first `pos` advance is the *next*
//! tick. Step 2 (empty-input) guards that the new object loops are no-ops on
//! empty pools and the driver never spuriously fires or rolls RNG.

use assets::level::LevelData;
use assets::object::Weapon;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponInit, WormInit, MAT_BACKGROUND, NUM_WEAPONS};
use sim_core::fixed::itof;
use sim_core::tables::precompute_cossin;
use sim_core::vec::Vec2;

// The real fan weapon, loaded from the shipped TC config (empty cross-ref lists:
// none of the fired/processed fan fields depend on a cross-ref; `id` is set by
// the caller == its index in the weapons slice).
fn fan_weapon(id: i32) -> Weapon {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/weapons/fan.cfg"
    ));
    let mut w = Weapon::load(bytes, &[], &[], &[]).unwrap();
    w.id = id;
    w
}

// An all-background level (material 1 + flag-table entry 0 are background), so
// `dirt_rock` is false everywhere — the projectile never ground-collides and only
// the explosion timer can end it. Big enough that a fan shot stays in bounds for
// its whole ~45-tick lifetime.
fn all_background_level(width: i32, height: i32) -> LevelData {
    LevelData {
        width,
        height,
        material_id: vec![1u8; (width * height) as usize],
        palette: None,
        display: None,
    }
}

fn background_flags() -> [u8; 256] {
    let mut flags = [0u8; 256];
    flags[0] = MAT_BACKGROUND;
    flags[1] = MAT_BACKGROUND;
    flags
}

// A grounded, visible worm placed near the floor of a tall all-background level,
// fan in slot 0, aiming up-and-right (index 12, inside the left-facing aim band
// [12,64]) so the shot climbs into open space. `detect_distance` = 1.
const START: Vec2 = Vec2 {
    x: 65_536 * 1000,
    y: 65_536 * 1996,
};
const FAN_DETECT_DISTANCE: i32 = 1;
const AIM_INDEX: i32 = 12;

fn fire_state() -> SimState {
    let level = all_background_level(2000, 2000);
    let flags = background_flags();
    let mut weapons = [WeaponInit::default(); NUM_WEAPONS];
    weapons[0] = WeaponInit {
        ty: Some(0),
        ammo: 10,
    };
    let init = WormInit {
        index: 0,
        health: 100,
        lives: 5,
        stats_x: 0,
        weapons,
        start_pos: START,
        visible: true,
    };
    let mut s = SimState::new(
        &level,
        &[init],
        42,
        &flags,
        vec![fan_weapon(0)],
        PhysicsConsts::default(),
        ControlConsts::default(),
        false, // h_signed_recoil (fan recoil 2 < 128 -> inert anyway)
        assets::sprite::SpriteSet::default(),
        Vec::new(),
    );
    // Aim up-right at cossin index 12 (inside the default left-facing band
    // [aim_max_left=12 .. aim_min_left=64]); process_aiming leaves it untouched
    // with no Up/Down input, so the firing index is exactly 12.
    s.worms[0].aiming_angle = itof(AIM_INDEX);
    s.worms[0].direction = 0;
    s
}

fn fire_input() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::FIRE);
    cs
}

#[test]
fn fire_birth_tick_does_not_move_the_new_shot_then_it_flies_and_explodes() {
    let cossin = precompute_cossin();
    let mut s = fire_state();
    assert_eq!(s.cycles, 0, "cycles starts at 0");
    assert!(s.wobjects.is_empty(), "no wobjects before firing");
    assert_eq!(s.rand.last(), 0, "no rand drawn before firing");

    // --- Fire tick: Fire gate trips, worm_fire spawns one wobject. -----------
    s.process_frame(&[fire_input()]);

    assert_eq!(s.cycles, 0, "process_frame must NOT increment cycles");
    assert_eq!(s.worms[0].weapons[0].ammo, 9, "ammo decremented by Fire");
    assert_eq!(
        s.worms[0].weapons[0].delay_left, 0,
        "delay_left = fan delay (0)"
    );
    assert_ne!(s.rand.last(), 0, "Fire drew RNG (the 4 fan draws)");
    assert_eq!(s.wobjects.len(), 1, "exactly one fan projectile spawned");

    let birth = *s.wobjects.iter().next().expect("one wobject");

    // THE off-by-one: the wobjects loop ran BEFORE the worm loop this tick, so the
    // freshly-spawned shot was NOT walked — its pos is exactly the muzzle/firing
    // position, un-advanced. firing_pos = cossin[12]*(detect+5) + worm.pos -
    // (0, Itof(1)); worm.pos at fire time == START (grounded, vel 0, fire is
    // step 8 BEFORE the physics integration of step 9).
    let expected_birth_pos = cossin[AIM_INDEX as usize]
        .mul(FAN_DETECT_DISTANCE + 5)
        .add(START)
        .sub(Vec2::new(0, itof(1)));
    assert_eq!(
        birth.pos, expected_birth_pos,
        "birth tick: shot sits at the firing position (NOT advanced by vel)"
    );
    // cur_frame = color_bullets(25) - rand(2), so 24 or 25 (start_frame < 0 path).
    assert!(
        birth.cur_frame == 24 || birth.cur_frame == 25,
        "cur_frame = 25 - rand(2) (got {})",
        birth.cur_frame
    );

    // --- Fly ticks (no Fire): each tick the wobjects loop advances pos by vel
    // (fan gravity 0 -> vel constant) and counts the explosion timer down.
    let rng_after_fire = s.rand.last();
    let mut prev = birth;
    let mut exploded = false;
    for tick in 0..200 {
        s.process_frame(&[ControlState::new()]);
        assert_eq!(s.cycles, 0, "cycles stays 0 on fly tick {tick}");
        assert_eq!(
            s.rand.last(),
            rng_after_fire,
            "wobject Process + blow_up draw NO rng (tick {tick})"
        );
        assert_eq!(s.worms[0].weapons[0].ammo, 9, "no refire (ammo stays 9)");

        if s.wobjects.is_empty() {
            // The explosion tick: timer underflowed, driver freed the slot.
            exploded = true;
            break;
        }
        let cur = *s.wobjects.iter().next().expect("alive wobject");
        assert_eq!(
            cur.pos,
            prev.pos.add(prev.vel),
            "fly tick {tick}: pos advances by exactly vel"
        );
        assert_eq!(cur.vel, prev.vel, "fan gravity 0 -> vel unchanged");
        assert_eq!(
            cur.time_left,
            prev.time_left - 1,
            "fly tick {tick}: explosion timer counts down"
        );
        prev = cur;
    }
    assert!(exploded, "the fan shot eventually times out and explodes");
    assert!(s.wobjects.is_empty(), "pool empty after the explosion");
    assert_eq!(s.cycles, 0, "cycles still 0 after the whole sequence");
}

#[test]
fn empty_input_keeps_pools_empty_and_draws_no_rng() {
    // Step 2 guard: with a fan loaded but Fire NEVER pressed, the new object
    // loops are no-ops on the empty pools, the Fire gate never trips, cycles stays
    // 0 and no RNG is drawn — proving the object loops + Fire gate did not perturb
    // the worms-only behaviour. (The worms-only equivalence itself is pinned by
    // the still-green slice-2/3 oracle goldens, which now call process_frame.)
    let mut s = fire_state();
    for tick in 0..30 {
        s.process_frame(&[ControlState::new()]);
        assert!(s.wobjects.is_empty(), "wobjects empty (tick {tick})");
        assert!(s.sobjects.is_empty(), "sobjects empty (tick {tick})");
        assert!(s.nobjects.is_empty(), "nobjects empty (tick {tick})");
        assert!(s.bonuses.is_empty(), "bonuses empty (tick {tick})");
        assert!(s.bobjects.is_empty(), "bobjects empty (tick {tick})");
        assert_eq!(s.cycles, 0, "cycles stays 0 (tick {tick})");
        assert_eq!(s.rand.last(), 0, "no Fire -> no rand drawn (tick {tick})");
        assert_eq!(s.worms[0].weapons[0].ammo, 10, "no Fire -> ammo untouched");
    }
}
