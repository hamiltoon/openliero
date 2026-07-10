//! Slice-4d Task-5 driver INTEGRATION tests — the per-worm pass now threads the
//! 4d args through its three calls, end to end via `SimState::process_frame`:
//!
//! * `process_weapons` gets `rand` + `&mut nobjects` + `&nobject_types` +
//!   `worm_index` + `&weapons` + `settings_loading_time` — so the shell-drop
//!   (4d-T2) and the reload + loading countdown (4d-T3) are LIVE in the driver.
//! * `process_movement` gets `&mut level` + `&large_sprites` + `&textures` +
//!   `cossin` + `rand` — so the dig carve (4d-T1) is LIVE in the driver.
//! * `process_weapon_change` gets `load_change` (4d-T4) — so a held Change cycles
//!   the weapon mid-reload.
//!
//! All four tests drive the **real HANDGUN TC** (`handgun.cfg`: `shotType=0`,
//! `leaveShells=1`, `leaveShellDelay=1`, `loadingTime=220`, `ammo=15`,
//! `createOnExp=small_explosion`). The handgun's explosion spawns the 4c
//! sobject/DoDamage cluster, so the fire-path tests keep the projectile in OPEN
//! BACKGROUND (no dirt to ground-explode into, a level far larger than the shot
//! can cross in the observed window) — the shot never explodes, so `blow_up` /
//! DoDamage never run and both worms stay untouched. The shell-drop / reload /
//! weapon-change behaviour all happen in `process_weapons` /
//! `process_weapon_change` regardless of the projectile's fate.

use assets::object::{Objects, Weapon};
use assets::tc::TcConfig;
use sim::control::{computed_loading_time, ControlConsts};
use sim::hash::hash_components;
use sim::physics::PhysicsConsts;
use sim::state::{
    ControlState, SimState, WeaponInit, WormInit, MAT_BACKGROUND, MAT_DIRT, MAT_DIRT_ROCK,
    NUM_WEAPONS,
};
use sim_core::fixed::itof;
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const SEED: u32 = 42;
const SETTINGS_LOADING_TIME: i32 = 100;

/// The shipped 16x16 large-sprite bank (the crater mask/fill `draw_dirt_effect`
/// reads for the dig).
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Full real object tables (cross-refs resolved, `id = index`), so the shell
/// drop's hard-coded `nobjects[7]` resolves to `shells`.
fn load_objects(tc: &TcConfig) -> Objects {
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object tables load")
}

/// The real HANDGUN weapon, `id` set to 0 == its index in the one-element weapon
/// table the state holds. No field is normalized (`shotType` is already 0).
fn handgun_weapon(tc: &TcConfig) -> Weapon {
    let bytes = std::fs::read(format!("{TC_ROOT}/weapons/handgun.cfg")).expect("read handgun.cfg");
    let mut w = Weapon::load(&bytes, &tc.types.nobjects, &tc.types.sobjects, &tc.types.sounds)
        .expect("handgun loads");
    w.id = 0;
    w
}

/// Pick a pure-Background and a pure-Dirt material id from the real flag table.
fn pick_materials(tc: &TcConfig) -> (u8, u8) {
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
    (bg_id, dirt_id)
}

fn fire() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::FIRE);
    cs
}

fn idle() -> ControlState {
    ControlState::new()
}

fn left() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::LEFT);
    cs
}

fn left_right() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::LEFT);
    cs.press(ControlState::RIGHT);
    cs
}

fn change_right() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::CHANGE);
    cs.press(ControlState::RIGHT);
    cs
}

/// Find the single live shell (`nobjects[7]`) in the pool, if any.
fn find_shell(s: &SimState) -> Option<sim::state::NObject> {
    s.nobjects.iter().copied().find(|o| o.ty == Some(7))
}

/// Build a `SimState` over a large OPEN-BACKGROUND level: one visible worm
/// holding the handgun in `weapon_slots`, aimed straight UP so the shot climbs
/// into open sky (and, with no dirt anywhere, never explodes), plus a distant
/// invisible idle worm. The level is far larger than the ~3.6px/tick shot can
/// cross in the observed window, so the projectile stays in-bounds and inert.
fn open_state(weapon_slots: [WeaponInit; NUM_WEAPONS]) -> (SimState, Weapon) {
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = load_objects(&tc);
    assert_eq!(
        objects.nobject_types[7].id_str, "shells",
        "nobjects[7] is the spent shell the drop hard-codes"
    );
    let handgun = handgun_weapon(&tc);
    assert_eq!(handgun.shot_type, 0, "handgun is ST_NORMAL (no normalization)");
    assert_eq!(handgun.leave_shells, 1, "handgun leaves a shell");
    assert_eq!(handgun.leave_shell_delay, 1, "shell drops the tick after firing");
    assert_eq!(handgun.loading_time, 220, "handgun loadingTime = 220");

    let (bg_id, _dirt_id) = pick_materials(&tc);

    const W: i32 = 1500;
    const H: i32 = 1500;
    let level = assets::level::LevelData {
        width: W,
        height: H,
        material_id: vec![bg_id; (W * H) as usize],
        palette: None,
        display: None,
    };

    // Worm 0 up in the open middle: lots of sky above for the rising shot and
    // lots of room below for the slowly-falling shell — neither reaches an edge.
    let worm0 = WormInit {
        index: 0,
        health: 100,
        lives: 5,
        stats_x: 0,
        weapons: weapon_slots,
        start_pos: Vec2::new(itof(500), itof(400)),
        visible: true,
    };
    let worm1 = WormInit {
        index: 1,
        health: 100,
        lives: 5,
        stats_x: 218,
        weapons: [WeaponInit::default(); NUM_WEAPONS],
        start_pos: Vec2::new(itof(1200), itof(400)),
        visible: false,
    };

    let mut s = SimState::new(
        &level,
        &[worm0, worm1],
        SEED,
        &tc.materials,
        vec![handgun.clone()],
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        SETTINGS_LOADING_TIME,
        true, // load_change default
        100,
    );
    // Aim straight up (cossin[64] = (0,-1)); index 64 sits at the left band's
    // aim_min_left limit, so process_aiming leaves it alone with no Up/Down input.
    s.worms[0].aiming_angle = itof(64);
    s.worms[0].direction = 0;
    (s, handgun)
}

/// A single full-handgun slot, all five slots loaded so the weapon-change test
/// can cycle into a valid slot without tripping the reload's `ty.expect`.
fn full_slots() -> [WeaponInit; NUM_WEAPONS] {
    [WeaponInit {
        ty: Some(0),
        ammo: 15,
    }; NUM_WEAPONS]
}

// ---------------------------------------------------------------------------
// Step 1 — fire -> shell drop -> the nobjects loop advances the shell.
// ---------------------------------------------------------------------------
#[test]
fn fire_drops_a_shell_next_tick_then_the_nobjects_loop_advances_it() {
    let mut slots = [WeaponInit::default(); NUM_WEAPONS];
    slots[0] = WeaponInit {
        ty: Some(0),
        ammo: 15,
    };
    let (mut s, handgun) = open_state(slots);

    assert!(s.nobjects.is_empty() && s.wobjects.is_empty(), "pools start empty");
    assert_eq!(s.cycles, 0, "cycles starts at 0");

    // --- Fire tick: worm_fire arms leave_shell_timer; process_weapons (which
    //     ran BEFORE the Fire gate) saw the timer at 0, so NO shell yet. --------
    s.process_frame(&[fire(), idle()]);
    assert_eq!(s.cycles, 1, "process_frame increments cycles once (game.cpp:357)");
    assert_eq!(s.worms[0].weapons[0].ammo, handgun.ammo - 1, "ammo decremented");
    assert_eq!(s.worms[0].weapons[0].delay_left, handgun.delay, "delay_left = 20");
    assert_eq!(s.wobjects.len(), 1, "one handgun projectile spawned");
    assert_eq!(s.worms[0].leave_shell_timer, 1, "leave_shell_timer armed to 1");
    assert!(find_shell(&s).is_none(), "no shell on the fire tick itself");

    // --- Next tick: process_weapons counts the timer 1 -> 0 and drops the shell
    //     (nobjects[7]). The nobjects loop ran BEFORE process_weapons, so the
    //     brand-new shell is NOT walked on its birth tick. ---------------------
    s.process_frame(&[idle(), idle()]);
    assert_eq!(s.cycles, 2, "second process_frame -> cycles == 2");
    assert_eq!(s.worms[0].leave_shell_timer, 0, "shell timer expired");
    let shell = find_shell(&s).expect("a shell dropped the tick after firing");
    assert_eq!(shell.ty, Some(7), "the dropped nobject is the shell type (7)");
    let birth_pos = shell.pos;

    // --- Later ticks: the (4c) nobjects loop now drives the shell — its position
    //     advances. This is the cross-loop wiring under test (process_weapons
    //     spawned it; the driver's nobjects loop walks it next tick onward). ----
    let mut advanced = false;
    for tick in 0..12 {
        s.process_frame(&[idle(), idle()]);
        // 2 prior process_frame calls + (tick + 1) here => cycles == tick + 3.
        assert_eq!(s.cycles, tick + 3, "cycles advances once per tick (advance tick {tick})");
        let cur = find_shell(&s).expect("shell persists (no ground in open sky)");
        if cur.pos != birth_pos {
            advanced = true;
            break;
        }
    }
    assert!(advanced, "the nobjects loop advanced the shell on a later tick");
}

// ---------------------------------------------------------------------------
// Step 2 — fire to empty -> process_weapons reloads (loading_left + ammo reset).
// ---------------------------------------------------------------------------
#[test]
fn firing_to_empty_arms_the_reload_and_refills_ammo() {
    let mut slots = [WeaponInit::default(); NUM_WEAPONS];
    slots[0] = WeaponInit {
        ty: Some(0),
        ammo: 2, // two fires to empty
    };
    let (mut s, handgun) = open_state(slots);

    // The armed value the reload computes (220 * 100 / 100), pinned before it is
    // immediately decremented once by the same-tick loading countdown.
    assert_eq!(
        computed_loading_time(&handgun, SETTINGS_LOADING_TIME),
        220,
        "ComputedLoadingTime(handgun, 100) = 220"
    );

    // Hold Fire; the gate fires on tick 0 then again after delay (20). The tick
    // the second shot empties the slot, ammo == 0 with loading_left STILL 0
    // (process_weapons ran before that fire). The FOLLOWING tick reloads.
    let mut empty_seen = false;
    let mut reloaded = false;
    for tick in 0..80 {
        s.process_frame(&[fire(), idle()]);
        assert_eq!(s.cycles, tick + 1, "cycles advances once per tick (tick {tick})");
        let w = s.worms[0].weapons[0];
        if !empty_seen {
            if w.ammo == 0 {
                empty_seen = true;
                assert_eq!(w.loading_left, 0, "not reloaded yet on the empty tick");
            }
            continue;
        }
        // First tick after the slot went empty: the reload fired.
        assert_eq!(
            w.loading_left, 219,
            "reload armed 220, same-tick countdown -> 219"
        );
        assert_eq!(w.ammo, handgun.ammo, "ammo reset to the weapon's ammo (15)");
        reloaded = true;

        // And it keeps counting down on the next tick (still empty-firing, but
        // Available() is false now so no refire).
        s.process_frame(&[fire(), idle()]);
        assert_eq!(s.worms[0].weapons[0].loading_left, 218, "loading_left keeps ticking down");
        assert_eq!(s.worms[0].weapons[0].ammo, handgun.ammo, "ammo stays refilled while loading");
        break;
    }
    assert!(empty_seen, "the slot emptied after two fires");
    assert!(reloaded, "process_weapons reloaded the empty slot");
}

// ---------------------------------------------------------------------------
// Step 3 — dig: Change-NOT-held + L+R over dirt carves the level (rng += 2),
//          and a single-direction tick re-arms able_to_dig.
// ---------------------------------------------------------------------------
#[test]
fn dig_carves_the_level_advances_rng_and_rearms_on_single_direction() {
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = load_objects(&tc);
    let (bg_id, dirt_id) = pick_materials(&tc);

    // Sky over a dirt floor; the worm stands on the surface and aims down-left
    // (cossin[12] has a downward component) so both craters carve the floor.
    const W: i32 = 400;
    const H: i32 = 300;
    const FLOOR_Y: i32 = 200;
    let mut material_id = vec![bg_id; (W * H) as usize];
    for y in FLOOR_Y..H {
        for x in 0..W {
            material_id[(y * W + x) as usize] = dirt_id;
        }
    }
    let level = assets::level::LevelData {
        width: W,
        height: H,
        material_id,
        palette: None,
        display: None,
    };

    let mut slots = [WeaponInit::default(); NUM_WEAPONS];
    slots[0] = WeaponInit {
        ty: Some(0),
        ammo: 15,
    };
    let worm0 = WormInit {
        index: 0,
        health: 100,
        lives: 5,
        stats_x: 0,
        weapons: slots,
        start_pos: Vec2::new(itof(100), itof(FLOOR_Y - 4)),
        visible: true,
    };

    let mut s = SimState::new(
        &level,
        &[worm0],
        SEED,
        &tc.materials,
        vec![handgun_weapon(&tc)],
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        SETTINGS_LOADING_TIME,
        true,
        100,
    );
    // Down-left aim, inside the left band [12, 64] so process_aiming keeps it.
    s.worms[0].aiming_angle = itof(12);
    s.worms[0].direction = 0;
    s.worms[0].able_to_dig = true; // edge armed (Worm ctor leaves it false)

    // The dig is the ONLY rng consumer in this test (no firing), so before the
    // first dig the rng is still at its freshly-seeded state. Replay the exact
    // draw sequence on a side Rand: each draw_dirt_effect draws ONE rand(rframe)
    // and texture 7 has rframe=2, so each dig is two rand(2) draws. `last()` after
    // N digs pins the cumulative draw count to EXACTLY 2*N.
    let mut oracle = Rand::new();
    oracle.seed(SEED);
    oracle.bound(2);
    oracle.bound(2);
    let last_after_one_dig = oracle.last();
    oracle.bound(2);
    oracle.bound(2);
    let last_after_two_digs = oracle.last();

    let level_before = hash_components(&s).level;
    assert_eq!(s.rand.last(), 0, "no rng drawn before the first dig");

    // --- Dig tick: Change NOT held, L+R both held, able_to_dig set -> two
    //     draw_dirt_effect carves (texture 7, rframe 2 -> one rand each). -------
    s.process_frame(&[left_right()]);
    assert_eq!(s.cycles, 1, "process_frame increments cycles once (game.cpp:357)");
    assert!(!s.worms[0].able_to_dig, "the dig spent the able_to_dig edge");
    assert_ne!(hash_components(&s).level, level_before, "the dig carved the level");
    assert_eq!(
        s.rand.last(),
        last_after_one_dig,
        "the dig advanced the rng by exactly two rand(2) draws (one per crater)"
    );

    // --- Edge-trigger: L+R again with able_to_dig false does NOT dig. ----------
    let level_after_dig = hash_components(&s).level;
    s.process_frame(&[left_right()]);
    assert!(!s.worms[0].able_to_dig, "still spent -> no re-dig");
    assert_eq!(hash_components(&s).level, level_after_dig, "no second carve while spent");
    assert_eq!(s.rand.last(), last_after_one_dig, "no rng draw while the edge is spent");

    // --- Single-direction tick re-arms able_to_dig (the brief's pin). ----------
    s.process_frame(&[left()]);
    assert!(s.worms[0].able_to_dig, "a not-both-held tick re-arms able_to_dig");
    assert_eq!(s.rand.last(), last_after_one_dig, "the re-arm tick draws no rng");

    // --- Re-armed: the next L+R digs again. The worm has not moved, so the two
    //     craters fall on the SAME (already-cleared) cells -> no further level
    //     change; the proof the re-armed edge fired is that the dig drew its two
    //     rand(2) again (cumulative four) and spent able_to_dig once more. ------
    s.process_frame(&[left_right()]);
    assert!(!s.worms[0].able_to_dig, "second dig spends the re-armed edge");
    assert_eq!(
        s.rand.last(),
        last_after_two_digs,
        "second dig drew two more rand(2) (cumulative four) -> the re-arm let it fire"
    );
}

// ---------------------------------------------------------------------------
// Step 4 — change-during-reload: a held Change cycles current_weapon even while
//          the current slot is still loading (load_change = true).
// ---------------------------------------------------------------------------
#[test]
fn change_during_reload_cycles_current_weapon() {
    // Slot 0 has one round so a single fire empties it and the next tick reloads;
    // every slot is a full handgun so cycling lands on a valid (non-panicking)
    // slot.
    let mut slots = full_slots();
    slots[0] = WeaponInit {
        ty: Some(0),
        ammo: 1,
    };
    let (mut s, _handgun) = open_state(slots);

    assert_eq!(s.worms[0].current_weapon, 0, "starts on slot 0");

    // --- Fire tick: empties slot 0 (ammo 1 -> 0). -----------------------------
    s.process_frame(&[fire(), idle()]);
    assert_eq!(s.worms[0].weapons[0].ammo, 0, "slot 0 emptied");
    assert_eq!(s.worms[0].weapons[0].loading_left, 0, "not reloaded yet");

    // --- Change+Right tick #1: process_weapons reloads slot 0 (loading_left>0);
    //     the Change gate's FIRST tick latches + consumes Right, so NO cycle. ---
    s.process_frame(&[change_right(), idle()]);
    assert!(
        s.worms[0].weapons[0].loading_left > 0,
        "slot 0 is now reloading (loading_left > 0)"
    );
    assert!(
        !s.worms[0].weapons[0].available(),
        "the current slot is NOT Available while loading"
    );
    assert_eq!(s.worms[0].current_weapon, 0, "first Change tick only latches, no cycle");

    // --- Change+Right tick #2: Right is re-set by the per-tick Unpack and
    //     PressedOnce cycles. The current slot is NOT Available, so the cycle
    //     happens ONLY because load_change = true. -----------------------------
    s.process_frame(&[change_right(), idle()]);
    assert!(
        s.worms[0].weapons[0].loading_left > 0,
        "slot 0 still loading when the change cycles"
    );
    assert_eq!(
        s.worms[0].current_weapon, 1,
        "Change held mid-reload cycled current_weapon (load_change)"
    );
    assert_eq!(s.cycles, 3, "cycles advanced once per process_frame call (3 ticks)");
}
