//! Driver-level tests for [`SimState::process_worms`] — the per-worm `Process`
//! pass wired in exact C++ order (`worm.cpp:210-353`).
//!
//! These exercise the *cross-method* ordering the unit tests in `control.rs` /
//! `physics.rs` can't see in isolation:
//!
//! * `reacts` is computed **once** and shared by `process_tasks` (jump) and
//!   `worm_process_physics` — a grounded jump fires (tasks saw `reacts[kRfUp] >
//!   0`) *and* gravity is skipped the same tick (physics saw the same grounding).
//! * jump (step 6) writes `vel.y` **before** physics (step 9) reads it.
//! * walk (step 11) writes `vel.x` **after** physics, so the walked velocity is
//!   un-frictioned this tick and only friction-decayed next tick.
//! * under empty input the full pass is inert on `pos`/`vel` (the Slice-2
//!   equivalence guard).

use assets::level::LevelData;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::state::{
    ControlState, SimState, WeaponInit, WormInit, MAT_BACKGROUND, NUM_WEAPONS,
};
use sim_core::fixed::{ftoi, itof};
use sim_core::vec::Vec2;

// An all-background level: material 1 is background and flag-table entry 0 (the
// OOB fallback) is background too, so `calculate_reaction_force` always returns 0
// and the only `reacts` come from the level-edge additions in `worm_reactions`.
fn all_background_level(width: i32, height: i32) -> (LevelData, [u8; 256]) {
    let mut flags = [0u8; 256];
    flags[0] = MAT_BACKGROUND;
    flags[1] = MAT_BACKGROUND;
    let level = LevelData {
        width,
        height,
        material_id: vec![1u8; (width * height) as usize],
        palette: None,
        display: None,
    };
    (level, flags)
}

// One worm placed at `pos` with zero velocity. Mid-x (no left/right edge add);
// the caller picks `pos.y` to be grounded (near the floor) or mid-air.
fn worm_init(pos: Vec2) -> WormInit {
    WormInit {
        index: 0,
        health: 100,
        lives: 5,
        stats_x: 0,
        weapons: [WeaponInit { ty: Some(0), ammo: 10 }; NUM_WEAPONS],
        start_pos: pos,
        visible: true,
    }
}

fn build(level: &LevelData, flags: &[u8; 256], pos: Vec2) -> SimState {
    SimState::new(
        level,
        &[worm_init(pos)],
        42,
        flags,
        PhysicsConsts::default(),
        ControlConsts::default(),
    )
}

// A grounded worm: placed so `i_next_y > height - 6`, which makes the UP edge add
// fire in `worm_reactions` (reacts[kRfUp] = 10 after the per-iteration resets).
// width/height 200; pos at x=100 (mid, no x edge), y just under the floor band.
fn grounded_state() -> SimState {
    let (level, flags) = all_background_level(200, 200);
    // i_next_y = Ftoi(pos.y + vel.y) = 196 > height-6 (194) -> grounded.
    build(&level, &flags, Vec2::new(itof(100), itof(196)))
}

#[test]
fn grounded_worm_is_actually_grounded() {
    // Sanity: confirm the fixture grounds the worm. An empty tick on a grounded
    // worm leaves vel == 0 (no gravity because reacts[kRfUp] > 0; nothing else
    // moves it), which only holds if the UP edge add fired.
    let mut s = grounded_state();
    s.process_worms(&[ControlState::new()]);
    assert_eq!(s.worms[0].vel.y, 0, "grounded -> gravity skipped -> vel.y stays 0");
    assert_eq!(s.worms[0].vel.x, 0);
}

#[test]
fn mid_air_worm_falls_under_gravity() {
    // Contrast fixture: mid-air worm (no edge add) gains gravity each tick — proves
    // the grounded fixture's stillness is the grounding, not a dead driver.
    let (level, flags) = all_background_level(200, 200);
    let mut s = build(&level, &flags, Vec2::new(itof(100), itof(100)));
    s.process_worms(&[ControlState::new()]);
    assert_eq!(s.worms[0].vel.y, 1500, "mid-air -> +WormGravity");
}

#[test]
fn jump_writes_vel_y_before_physics_using_shared_grounded_reacts() {
    // Arm the jump on an empty tick (Jump released -> able_to_jump = true), then
    // jump. The jump fires ONLY if `process_tasks` saw reacts[kRfUp] > 0, and
    // gravity is skipped the SAME tick ONLY if `worm_process_physics` saw the same
    // grounding — so vel.y lands EXACTLY at -JumpForce (no gravity added on top).
    // If physics ran before the jump it would be the same value here, but if the
    // two reads used different `reacts`, either the jump wouldn't fire or gravity
    // would perturb vel.y. The exact -JumpForce proves the shared grounded reacts.
    let c = ControlConsts::default();
    let mut s = grounded_state();

    // Tick 1 (empty): arms able_to_jump.
    s.process_worms(&[ControlState::new()]);
    assert!(s.worms[0].able_to_jump, "Jump released -> able_to_jump armed");
    assert_eq!(s.worms[0].vel.y, 0);

    // Tick 2 (Jump): impulse applied in step 6, physics (step 9) skips gravity.
    let mut jump = ControlState::new();
    jump.press(ControlState::JUMP);
    s.process_worms(&[jump]);
    assert_eq!(
        s.worms[0].vel.y,
        -c.jump_force,
        "vel.y == -JumpForce: jump fired (tasks saw grounded) and gravity skipped \
         (physics saw the SAME grounded reacts)"
    );
    assert!(!s.worms[0].able_to_jump, "impulse consumed able_to_jump");
}

#[test]
fn walk_writes_vel_x_after_physics() {
    // On a grounded worm, a Right tick: physics runs friction on the OLD vel.x (0)
    // first, THEN process_movement adds WalkVelRight. So vel.x == WalkVelRight
    // exactly (un-frictioned this tick). If movement ran BEFORE physics, the
    // grounded friction (×89/100) would have decayed it to 2670, not 3000.
    let c = ControlConsts::default();
    let mut s = grounded_state();

    let mut right = ControlState::new();
    right.press(ControlState::RIGHT);
    s.process_worms(&[right]);
    assert_eq!(
        s.worms[0].vel.x, c.walk_vel_right,
        "vel.x == WalkVelRight (walk wrote AFTER physics friction this tick)"
    );
    assert_eq!(s.worms[0].direction, 1, "Right faces the worm right");

    // Next tick (empty, grounded): NOW friction hits the walked velocity:
    // 3000 * 89 / 100 = 2670 (truncating toward zero).
    s.process_worms(&[ControlState::new()]);
    assert_eq!(
        s.worms[0].vel.x, 2670,
        "next tick friction decays the previously-walked vel.x"
    );
}

#[test]
fn aiming_up_raises_angle_monotonically_until_clamp() {
    // Several Up ticks (direction 0, left-facing): aiming_angle rises each tick
    // until it pins at AimMinLeft (Ftoi == 64). Monotone non-decreasing throughout.
    let mut s = grounded_state();
    let up = {
        let mut cs = ControlState::new();
        cs.press(ControlState::UP);
        cs
    };
    let mut prev = s.worms[0].aiming_angle;
    let mut hit_clamp = false;
    // Monotone non-decreasing while the angle climbs toward the AimMinLeft clamp.
    // Once `Ftoi(angle)` reaches 64 (the limit band) the angle is about to be
    // pinned *down* to exactly Itof(64) and then oscillates, so we stop the
    // monotone check on entry to the band (which is itself a rise).
    for _ in 0..200 {
        s.process_worms(&[up]);
        let a = s.worms[0].aiming_angle;
        assert!(a >= prev, "aiming_angle is monotonically non-decreasing under Up");
        if ftoi(a) >= 64 {
            hit_clamp = true;
            break;
        }
        prev = a;
    }
    assert!(hit_clamp, "aiming_angle reaches the AimMinLeft clamp (Ftoi == 64)");
}

#[test]
fn weapon_change_cycles_current_weapon_and_clears_direction_bit() {
    // A Change|Right hold cycles current_weapon. Because the first Change tick
    // latches key_change_pressed and consumes that tick's Right, cycling starts on
    // the second tick; from then a held Change+Right re-set each tick (Unpack) and
    // PressedOnce fires every tick. After the cycling tick the Right bit is cleared
    // in the packed control state.
    let mut s = grounded_state();
    let cr = {
        let mut cs = ControlState::new();
        cs.press(ControlState::CHANGE);
        cs.press(ControlState::RIGHT);
        cs
    };

    // Tick 1: first Change tick — latches, Right consumed, no cycle yet.
    s.process_worms(&[cr]);
    assert_eq!(s.worms[0].current_weapon, 0, "first Change tick consumes Right, no cycle");
    assert!(s.worms[0].key_change_pressed, "key_change_pressed latched");

    // Tick 2: Right re-set by Unpack, PressedOnce(Right) -> cycle up to slot 1.
    s.process_worms(&[cr]);
    assert_eq!(s.worms[0].current_weapon, 1, "second Change tick cycles to weapon 1");
    assert!(
        !s.worms[0].control_states.get(ControlState::RIGHT),
        "Right bit cleared in control_states after the cycle"
    );
    assert!(
        s.worms[0].control_states.pack() & (1 << ControlState::RIGHT) == 0,
        "pack() shows the Right bit cleared"
    );
}

#[test]
fn empty_input_matches_slice2_reactions_then_physics() {
    // Equivalence guard: under empty input the full per-worm pass must leave
    // pos/vel/health identical to the Slice-2 path (worm_reactions then
    // worm_process_physics) over several ticks. We replay the same scenario via
    // the public driver and via the bare physics fns and compare.
    use sim::physics::{worm_process_physics, worm_reactions};

    let (level, flags) = all_background_level(200, 200);
    // A falling worm (mid-air) so the path actually exercises gravity + motion.
    let start = Vec2::new(itof(100), itof(100));
    let mut driven = build(&level, &flags, start);

    // Mirror state for the manual Slice-2 path.
    let phys = PhysicsConsts::default();
    let mut ref_worm = driven.worms[0].clone();

    for _ in 0..30 {
        driven.process_worms(&[ControlState::new()]);

        let reacts = worm_reactions(&driven.level, &mut ref_worm, &phys);
        worm_process_physics(&mut ref_worm, &reacts, &phys);

        assert_eq!(driven.worms[0].pos, ref_worm.pos, "pos matches Slice-2 path");
        assert_eq!(driven.worms[0].vel, ref_worm.vel, "vel matches Slice-2 path");
        assert_eq!(driven.worms[0].health, ref_worm.health, "health unchanged");
    }
}
