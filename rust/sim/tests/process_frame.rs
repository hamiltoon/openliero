//! Driver-level tests for [`SimState::process_frame`] — the per-worm `Process`
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
use sim::state::{ControlState, SimState, WeaponInit, WormInit, MAT_BACKGROUND, NUM_WEAPONS};
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
        weapons: [WeaponInit {
            ty: Some(0),
            ammo: 10,
        }; NUM_WEAPONS],
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
        Vec::new(),
        PhysicsConsts::default(),
        ControlConsts::default(),
        false,
        assets::sprite::SpriteSet::default(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        100,
        true,
        100,
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
    s.process_frame(&[ControlState::new()]);
    assert_eq!(
        s.worms[0].vel.y, 0,
        "grounded -> gravity skipped -> vel.y stays 0"
    );
    assert_eq!(s.worms[0].vel.x, 0);
}

#[test]
fn mid_air_worm_falls_under_gravity() {
    // Contrast fixture: mid-air worm (no edge add) gains gravity each tick — proves
    // the grounded fixture's stillness is the grounding, not a dead driver.
    let (level, flags) = all_background_level(200, 200);
    let mut s = build(&level, &flags, Vec2::new(itof(100), itof(100)));
    s.process_frame(&[ControlState::new()]);
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
    s.process_frame(&[ControlState::new()]);
    assert!(
        s.worms[0].able_to_jump,
        "Jump released -> able_to_jump armed"
    );
    assert_eq!(s.worms[0].vel.y, 0);

    // Tick 2 (Jump): impulse applied in step 6, physics (step 9) skips gravity.
    let mut jump = ControlState::new();
    jump.press(ControlState::JUMP);
    s.process_frame(&[jump]);
    assert_eq!(
        s.worms[0].vel.y, -c.jump_force,
        "vel.y == -JumpForce: jump fired (tasks saw grounded) and gravity skipped \
         (physics saw the SAME grounded reacts)"
    );
    assert!(!s.worms[0].able_to_jump, "impulse consumed able_to_jump");
}

#[test]
fn jump_impulse_precedes_physics_on_downward_velocity() {
    // DISCRIMINATING ordering test. The sibling `jump_writes_vel_y_before_physics_*`
    // test jumps from rest (vel.y == 0), where swapping steps 6 and 9 yields the
    // SAME result (a zero-velocity grounded bounce/stop is a no-op and gravity is
    // gated off when grounded). This test pins the order with a GROUNDED worm
    // carrying a DOWNWARD vel.y on the jump tick, so the two orderings diverge.
    //
    // The grounded fixture's reacts are [down=0, left=0, up=10, right=0] (the UP
    // edge add grounds the worm; RF_DOWN stays 0).
    //
    // jump-first (CORRECT, current order — process_tasks step 6 BEFORE physics step 9):
    //   tasks:   vel.y = 40000 - JumpForce(56064) = -16064  (now upward)
    //   physics: vel.y < 0 -> rv = reacts[kRfDown] = 0 -> vertical branch skipped;
    //            gravity skipped (reacts[kRfUp] != 0) -> vel.y stays -16064.
    //
    // physics-first (WRONG, steps 6 and 9 reordered):
    //   physics: vel.y > 0 -> rv = reacts[kRfUp] = 10; abs(40000) <= MinBounceDown
    //            (53248) -> STOP: vel.y = 0; gravity skipped -> vel.y = 0.
    //   tasks:   vel.y = 0 - JumpForce = -56064.
    //
    // -16064 != -56064, so this test FAILS if the jump/physics steps are swapped.
    let c = ControlConsts::default();
    let p = PhysicsConsts::default();
    let mut s = grounded_state();

    // Tick 1 (empty): arm able_to_jump; worm stays grounded with vel == 0.
    s.process_frame(&[ControlState::new()]);
    assert!(
        s.worms[0].able_to_jump,
        "Jump released -> able_to_jump armed"
    );
    assert_eq!(
        s.worms[0].vel.y, 0,
        "grounded -> still at rest before the injection"
    );

    // Inject a downward velocity that is at/below MinBounceDown, so the
    // physics-first vertical branch would STOP it (vel.y -> 0) rather than bounce.
    let down = 40000;
    assert!(
        down <= p.min_bounce_down,
        "downward vel must be <= MinBounceDown so the physics-first branch stops it"
    );
    s.worms[0].vel.y = down;

    // Tick 2 (Jump): jump-first applies the impulse to the downward vel.y FIRST,
    // then physics sees an UPWARD vel.y (rv = reacts[kRfDown] = 0) and leaves it.
    let mut jump = ControlState::new();
    jump.press(ControlState::JUMP);
    s.process_frame(&[jump]);

    assert_eq!(
        s.worms[0].vel.y,
        down - c.jump_force,
        "jump-first: impulse applied to the downward vel.y BEFORE physics"
    );
    // Pin the exact value and prove it differs from the physics-first ordering,
    // which would stop vel.y to 0 then jump to -JumpForce.
    assert_eq!(s.worms[0].vel.y, -16064, "exact jump-first vel.y");
    assert_ne!(
        s.worms[0].vel.y, -c.jump_force,
        "differs from the physics-first result (-JumpForce = -56064)"
    );
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
    s.process_frame(&[right]);
    assert_eq!(
        s.worms[0].vel.x, c.walk_vel_right,
        "vel.x == WalkVelRight (walk wrote AFTER physics friction this tick)"
    );
    assert_eq!(s.worms[0].direction, 1, "Right faces the worm right");

    // Next tick (empty, grounded): NOW friction hits the walked velocity:
    // 3000 * 89 / 100 = 2670 (truncating toward zero).
    s.process_frame(&[ControlState::new()]);
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
        s.process_frame(&[up]);
        let a = s.worms[0].aiming_angle;
        assert!(
            a >= prev,
            "aiming_angle is monotonically non-decreasing under Up"
        );
        if ftoi(a) >= 64 {
            hit_clamp = true;
            break;
        }
        prev = a;
    }
    assert!(
        hit_clamp,
        "aiming_angle reaches the AimMinLeft clamp (Ftoi == 64)"
    );
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
    s.process_frame(&[cr]);
    assert_eq!(
        s.worms[0].current_weapon, 0,
        "first Change tick consumes Right, no cycle"
    );
    assert!(s.worms[0].key_change_pressed, "key_change_pressed latched");

    // Tick 2: Right re-set by Unpack, PressedOnce(Right) -> cycle up to slot 1.
    s.process_frame(&[cr]);
    assert_eq!(
        s.worms[0].current_weapon, 1,
        "second Change tick cycles to weapon 1"
    );
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
fn process_frame_increments_cycles_once_per_tick() {
    // `cycles` starts at 0 and advances by exactly one per `process_frame` call
    // (mirroring the C++ `++game.cycles` at game.cpp:357). This was frozen at 0 in
    // the ProcessFrame subset before Slice 5b; the master hash folds `cycles`
    // (hash.rs:50), so an off-by-one here would diverge the regenerated goldens'
    // master column. Non-tautological: it would fail if the increment were dropped,
    // doubled, or moved to a branch that the empty-input path skips.
    let mut s = grounded_state();
    assert_eq!(s.cycles, 0, "cycles starts at 0 (tick-0 contract)");
    for expected in 1..=5 {
        s.process_frame(&[ControlState::new()]);
        assert_eq!(
            s.cycles, expected,
            "cycles advances exactly once per process_frame call"
        );
    }
}

#[test]
fn process_frame_emits_and_drains_bump_one_shot_end_to_end() {
    // End-to-end 4c wiring: a grounded worm carrying a hard DOWNWARD velocity
    // bounces in worm_process_physics (step 9), which plays SoundBump
    // (worm.cpp:188, HFallDamage off). Pins the whole path through the public
    // driver: `begin_frame` publishes the hook index, the deep physics callsite
    // emits, and the tail drain lands exactly this tick's event in `sound_events`.
    let mut s = grounded_state();
    s.sound_hooks.Bump = 3;
    // Downward vel.y above MinBounceDown (53248) so the vertical branch bounces.
    s.worms[0].vel.y = 200000;

    s.process_frame(&[ControlState::new()]);
    assert_eq!(
        s.sound_events,
        vec![sim::sound::SoundEvent::one_shot(3)],
        "process_frame drains the SoundBump one-shot into sound_events"
    );

    // Cleared at the TOP of the next tick (design §2.2): an idle tick that plays
    // nothing leaves the stream empty — events never accumulate across ticks.
    s.process_frame(&[ControlState::new()]);
    assert!(
        s.sound_events.is_empty(),
        "sound_events holds only the current tick's events"
    );
}

// ---- Slice 4c T2: loop-sound Play/Stop lifecycle (worm.cpp:336-343/1075-1077/
// 373-376) ------------------------------------------------------------------

// A SimState whose weapon table holds ONE looping weapon (loop_sound = true,
// launch_sound = 5). parts = 0 spawns nothing and distribution/leave_shells = 0
// draw nothing, so fire ticks are RNG-silent — isolating the loop events.
// delay = 0 lets the Fire gate re-fire every tick while held. The 7 default
// nobject types cover the death block's blood ([6]) and per-worm gib ([index]).
fn loop_weapon_state() -> SimState {
    let (level, flags) = all_background_level(200, 200);
    let w = assets::object::Weapon {
        loop_sound: true,
        launch_sound: 5,
        ammo: 10,
        loading_time: 100,
        ..Default::default()
    };
    SimState::new(
        &level,
        &[worm_init(Vec2::new(itof(100), itof(196)))],
        42,
        &flags,
        vec![w],
        PhysicsConsts::default(),
        ControlConsts::default(),
        false,
        assets::sprite::SpriteSet::default(),
        Vec::new(),
        Vec::new(),
        vec![assets::object::NObjectType::default(); 7],
        100,
        true,
        100,
    )
}

fn fire_input() -> ControlState {
    let mut cs = ControlState::new();
    cs.press(ControlState::FIRE);
    cs
}

// The loop key for this fixture's worm 0, weapon slot 0.
fn key00() -> sim::sound::LoopKey {
    sim::sound::LoopKey::WormWeapon(0, 0)
}

#[test]
fn firing_loop_weapon_emits_keyed_play_every_fire_tick_with_stable_key() {
    // worm.cpp:1119-1122: each fire tick emits Play(launch_sound,
    // WormWeapon(worm, slot), loop). No IsPlaying state in the sim — the Play
    // repeats every tick the trigger is held (idempotency is the game's job),
    // and the key is STABLE across ticks (the same map slot game-side).
    let mut s = loop_weapon_state();
    for tick in 0..3 {
        s.process_frame(&[fire_input()]);
        assert_eq!(
            s.sound_events,
            vec![sim::sound::SoundEvent::loop_play(5, key00())],
            "tick {tick}: exactly one keyed loop Play, stable key"
        );
    }
}

#[test]
fn releasing_fire_emits_the_cease_fire_stop() {
    // worm.cpp:339-343: the else-arm of the Fire gate (`!Pressed(kFire) || ...`)
    // stops the current weapon's loop. Fire one tick, release the next: the
    // release tick's stream is exactly one Stop on the same key.
    let mut s = loop_weapon_state();
    s.process_frame(&[fire_input()]);
    assert_eq!(
        s.sound_events,
        vec![sim::sound::SoundEvent::loop_play(5, key00())],
        "fire tick: loop Play"
    );

    s.process_frame(&[ControlState::new()]);
    assert_eq!(
        s.sound_events,
        vec![sim::sound::SoundEvent::loop_stop(key00())],
        "release tick: the cease-fire Stop (worm.cpp:341), same key"
    );
}

#[test]
fn holding_change_emits_both_cpp_stop_sites() {
    // Fire+Change held: C++ reaches TWO Stop sites the same tick — the Fire
    // gate's else-arm (`Pressed(kChange)` -> worm.cpp:341) and the top of
    // ProcessWeaponChange (worm.cpp:1075-1077). Both stop the SAME key (the
    // first Change tick only latches; current_weapon has not cycled yet), and
    // the game-side map treats the second as a no-op. Pinning both preserves
    // C++ Stop-site parity (never speculative-gate or dedup a Stop in the sim).
    let mut s = loop_weapon_state();
    s.process_frame(&[fire_input()]);

    let mut fc = fire_input();
    fc.press(ControlState::CHANGE);
    s.process_frame(&[fc]);
    assert_eq!(
        s.sound_events,
        vec![
            sim::sound::SoundEvent::loop_stop(key00()),
            sim::sound::SoundEvent::loop_stop(key00()),
        ],
        "change tick: the fire-gate Stop THEN the weapon-change Stop"
    );
}

#[test]
fn death_emits_the_death_stop_before_the_death_one_shot() {
    // worm.cpp:369-379: the death block stops the current weapon's loop
    // (:373-376) BEFORE playing the 15+rand(3) death one-shot (:378-379).
    // Fire is HELD on the death tick so the fire gate re-fires (a fresh Play)
    // and the ONLY Stop in the stream is the death-site one — isolating it
    // from the cease-fire site.
    let mut s = loop_weapon_state();
    s.process_frame(&[fire_input()]);

    s.worms[0].health = 0;
    s.process_frame(&[fire_input()]);

    let events = &s.sound_events;
    let stops: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.action == sim::sound::SoundAction::Stop)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        stops.len(),
        1,
        "exactly ONE Stop (the death site): {events:?}"
    );
    let stop_at = stops[0];
    assert_eq!(
        events[stop_at],
        sim::sound::SoundEvent::loop_stop(key00()),
        "the death Stop carries the WormWeapon key"
    );

    let play_at = events
        .iter()
        .position(|e| *e == sim::sound::SoundEvent::loop_play(5, key00()))
        .expect("the held-fire tick re-emits the loop Play");
    assert!(
        play_at < stop_at,
        "fire-gate Play precedes the death Stop (C++ statement order)"
    );

    let death_at = events
        .iter()
        .position(|e| e.key.is_none() && (15..=17).contains(&e.sound))
        .expect("the 15+rand(3) death one-shot is emitted");
    assert!(
        stop_at < death_at,
        "death Stop (:373-376) precedes the death one-shot (:378-379)"
    );
}

#[test]
fn ammo_depletion_reload_emits_the_not_available_stop() {
    // The third leg of worm.cpp:339 (`!weapons[cur].Available()`): ammo runs
    // out -> process_weapons arms loading_left (worm.cpp:820-827) -> the SAME
    // held-fire tick takes the else-arm and stops the loop, with Fire still
    // pressed. No release, no switch, no death — the reload itself stops it.
    let mut s = loop_weapon_state();
    // ammo 10, delay 0: 10 fire ticks deplete the slot to 0.
    for tick in 0..10 {
        s.process_frame(&[fire_input()]);
        assert_eq!(
            s.sound_events,
            vec![sim::sound::SoundEvent::loop_play(5, key00())],
            "tick {tick}: still firing"
        );
    }
    assert_eq!(s.worms[0].weapons[0].ammo, 0, "ammo depleted");

    // Tick 11 (fire still held): reload arms -> Available() false -> Stop.
    s.process_frame(&[fire_input()]);
    assert_eq!(
        s.sound_events,
        vec![sim::sound::SoundEvent::loop_stop(key00())],
        "reload tick: the !Available() Stop fires while the trigger is held"
    );
}

#[test]
fn loop_events_leave_no_dangling_keys_across_fire_cease_death() {
    // Sim-side leak parity (design §4.2): replaying the whole event stream into
    // a keyed set (insert on Play, remove on Stop — the game drainer's model)
    // ends EMPTY after fire->cease and after fire->death. Every Play has a
    // reachable Stop; a leaked key here would leak a mixer channel in T3.
    use std::collections::HashSet;

    let mut live: HashSet<sim::sound::LoopKey> = HashSet::new();
    let drain = |events: &[sim::sound::SoundEvent], live: &mut HashSet<sim::sound::LoopKey>| {
        for e in events {
            if let Some(k) = e.key {
                match e.action {
                    sim::sound::SoundAction::Play => {
                        live.insert(k);
                    }
                    sim::sound::SoundAction::Stop => {
                        live.remove(&k);
                    }
                }
            }
        }
    };

    // fire 3 ticks -> cease.
    let mut s = loop_weapon_state();
    for _ in 0..3 {
        s.process_frame(&[fire_input()]);
        drain(&s.sound_events, &mut live);
    }
    assert!(!live.is_empty(), "loop live while firing");
    s.process_frame(&[ControlState::new()]);
    drain(&s.sound_events, &mut live);
    assert!(live.is_empty(), "cease-fire: no dangling loop keys");

    // fire -> death (fire held through the death tick).
    let mut s2 = loop_weapon_state();
    s2.process_frame(&[fire_input()]);
    drain(&s2.sound_events, &mut live);
    s2.worms[0].health = 0;
    s2.process_frame(&[fire_input()]);
    drain(&s2.sound_events, &mut live);
    assert!(live.is_empty(), "death: no dangling loop keys");
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
        driven.process_frame(&[ControlState::new()]);

        let reacts = worm_reactions(&driven.level, &mut ref_worm, &phys);
        worm_process_physics(&mut ref_worm, &reacts, &phys);

        assert_eq!(
            driven.worms[0].pos, ref_worm.pos,
            "pos matches Slice-2 path"
        );
        assert_eq!(
            driven.worms[0].vel, ref_worm.vel,
            "vel matches Slice-2 path"
        );
        assert_eq!(driven.worms[0].health, ref_worm.health, "health unchanged");
    }
}
