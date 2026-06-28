//! Worm-control TC constants.
//!
//! [`ControlConsts`] is the set of `TcConfig` `[constants]` + `[hacks]` values
//! the worm control/aiming paths (`ProcessAiming`, `ProcessTasks`,
//! `ProcessWeaponChange`, `ProcessMovement`) read each tick. It is a sibling to
//! [`PhysicsConsts`](crate::physics::PhysicsConsts): built once from a loaded
//! `TcConfig` via [`ControlConsts::from_tc`] and carried on
//! [`SimState`](crate::state::SimState) so the driver signature stays
//! `(state, inputs)`. Not hashed.
//!
//! Slice 3, Task 0 adds only the data; the control *logic* that reads these
//! lands in later tasks (design doc, *`ControlConsts`*).

use sim_core::fixed::{ftoi, itof};

use crate::state::{ControlState, WormState, NUM_WEAPONS};

/// The TC constants/hacks the worm control + aiming paths read. Field groups
/// mirror the design-doc table (Aiming / Movement / Jump / Ninjarope); each
/// field names the `TcConfig.constants` (or `.hacks`) key it carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlConsts {
    // --- Aiming ---
    /// `AimFricMult` / `AimFricDiv` — aiming-speed friction numerator/denominator.
    pub aim_fric_mult: i32,
    pub aim_fric_div: i32,
    /// `AimMaxRight` / `AimMinRight` — aim-angle clamp bounds while facing right.
    pub aim_max_right: i32,
    pub aim_min_right: i32,
    /// `AimMaxLeft` / `AimMinLeft` — aim-angle clamp bounds while facing left.
    pub aim_max_left: i32,
    pub aim_min_left: i32,
    /// `MaxAimVelLeft` / `MaxAimVelRight` — aiming-speed clamps (note the TC
    /// values are signed: left is positive, right is negative).
    pub max_aim_vel_left: i32,
    pub max_aim_vel_right: i32,
    /// `AimAccLeft` / `AimAccRight` — aiming-speed acceleration per tick.
    pub aim_acc_left: i32,
    pub aim_acc_right: i32,

    // --- Movement ---
    /// `WalkVelLeft` / `MaxVelLeft` — leftward walk acceleration and velocity cap.
    pub walk_vel_left: i32,
    pub max_vel_left: i32,
    /// `WalkVelRight` / `MaxVelRight` — rightward walk acceleration and velocity cap.
    pub walk_vel_right: i32,
    pub max_vel_right: i32,

    // --- Jump ---
    /// `JumpForce` — upward velocity impulse applied on a grounded jump.
    pub jump_force: i32,
    /// Hack `AirJump` — when set, the worm may jump without being grounded.
    pub h_air_jump: bool,
    /// Hack `MultiJump` — when set, the worm may jump repeatedly without
    /// releasing the key.
    pub h_multi_jump: bool,

    // --- Ninjarope ---
    /// `NRInitialLength` — rope length set at throw time.
    pub nr_initial_length: i32,
    /// `NRMinLength` / `NRMaxLength` — rope length clamps.
    pub nr_min_length: i32,
    pub nr_max_length: i32,
    /// `NRPullVel` / `NRReleaseVel` — length change per tick while pulling/releasing.
    pub nr_pull_vel: i32,
    pub nr_release_vel: i32,
    /// `NRThrowVelX` / `NRThrowVelY` — throw-velocity multipliers.
    pub nr_throw_vel_x: i32,
    pub nr_throw_vel_y: i32,
}

impl Default for ControlConsts {
    /// The `data/TC/openliero` values (`tc.cfg`). Used by unit tests and as a
    /// sane default; the differential pipeline builds from the real TC via
    /// [`ControlConsts::from_tc`].
    fn default() -> Self {
        ControlConsts {
            aim_fric_mult: 83,
            aim_fric_div: 100,
            aim_max_right: 116,
            aim_min_right: 64,
            aim_max_left: 12,
            aim_min_left: 64,
            max_aim_vel_left: 70000,
            max_aim_vel_right: -70000,
            aim_acc_left: 4000,
            aim_acc_right: 4000,
            walk_vel_left: 3000,
            max_vel_left: -29184,
            walk_vel_right: 3000,
            max_vel_right: 29184,
            jump_force: 56064,
            h_air_jump: false,
            h_multi_jump: false,
            nr_initial_length: 4000,
            nr_min_length: 170,
            nr_max_length: 4000,
            nr_pull_vel: 24,
            nr_release_vel: 24,
            nr_throw_vel_x: 2,
            nr_throw_vel_y: 2,
        }
    }
}

impl ControlConsts {
    /// Build from a loaded `TcConfig` (`assets::tc`), pulling `[constants]` and
    /// `[hacks]` — the exact values C++ reads in the control/aiming paths.
    pub fn from_tc(tc: &assets::tc::TcConfig) -> Self {
        let c = &tc.constants;
        let h = &tc.hacks;
        ControlConsts {
            aim_fric_mult: c.AimFricMult,
            aim_fric_div: c.AimFricDiv,
            aim_max_right: c.AimMaxRight,
            aim_min_right: c.AimMinRight,
            aim_max_left: c.AimMaxLeft,
            aim_min_left: c.AimMinLeft,
            max_aim_vel_left: c.MaxAimVelLeft,
            max_aim_vel_right: c.MaxAimVelRight,
            aim_acc_left: c.AimAccLeft,
            aim_acc_right: c.AimAccRight,
            walk_vel_left: c.WalkVelLeft,
            max_vel_left: c.MaxVelLeft,
            walk_vel_right: c.WalkVelRight,
            max_vel_right: c.MaxVelRight,
            jump_force: c.JumpForce,
            h_air_jump: h.AirJump,
            h_multi_jump: h.MultiJump,
            nr_initial_length: c.NRInitialLength,
            nr_min_length: c.NRMinLength,
            nr_max_length: c.NRMaxLength,
            nr_pull_vel: c.NRPullVel,
            nr_release_vel: c.NRReleaseVel,
            nr_throw_vel_x: c.NRThrowVelX,
            nr_throw_vel_y: c.NRThrowVelY,
        }
    }
}

// ---------------------------------------------------------------------------
// ProcessAiming (worm.cpp:1003-1062)
// ---------------------------------------------------------------------------

/// Port of `Worm::ProcessAiming` (`src/game/worm.cpp:1003-1062`).
///
/// Advances the worm's aim each tick. Reads the Up/Down (and, for the accel
/// gate, Change) control bits, `aiming_speed`, `aiming_angle`, `direction`,
/// `movable`, and `ninjarope.out`; writes `aiming_speed` and `aiming_angle`
/// (the latter is master-hashed, so this must be bit-exact).
///
/// In C++ order:
///
/// 1. **Integrate + clamp** (only when `aiming_speed != 0`):
///    - `aiming_angle += aiming_speed` (integrate, using the *pre-friction*
///      speed);
///    - if neither Up nor Down is held, apply friction
///      `aiming_speed = aiming_speed * AimFricMult / AimFricDiv` — a **truncating
///      `int` division** (toward zero), *not* an arithmetic shift, so a negative
///      speed truncates up toward zero;
///    - clamp `Ftoi(aiming_angle)` into the per-`direction` limits, zeroing
///      `aiming_speed` and pinning `aiming_angle` to the limit on overflow.
///      `direction == 1` (right) uses `[AimMinRight..AimMaxRight]`; the `else`
///      (left) branch uses `AimMaxLeft` as the lower pin and `AimMinLeft` as the
///      upper pin (note the C++ naming: for the left limits `AimMaxLeft` is the
///      *small* angle and `AimMinLeft` the *large* one).
/// 2. **Accelerate** (only when `movable && (!ninjarope.out || !Change)`): Up
///    and Down each push `aiming_speed` toward the direction-appropriate cap.
///    With `direction == 0`, Up adds `AimAccLeft` while `aiming_speed <
///    MaxAimVelLeft`; otherwise Up subtracts `AimAccRight` while `aiming_speed >
///    MaxAimVelRight` (the caps are signed: left positive, right negative). Down
///    mirrors Up with the `direction == 1` test selecting the additive branch.
///
/// All arithmetic is `wrapping_*` / truncating `/` to match C++ `int` semantics
/// bit-for-bit (same discipline as the Slice-2 physics port).
pub fn process_aiming(worm: &mut WormState, c: &ControlConsts) {
    let k_up = worm.control_states.get(ControlState::UP);
    let k_down = worm.control_states.get(ControlState::DOWN);

    if worm.aiming_speed != 0 {
        worm.aiming_angle = worm.aiming_angle.wrapping_add(worm.aiming_speed);

        if !k_up && !k_down {
            // Truncating int division toward zero (NOT `>>`).
            worm.aiming_speed = worm.aiming_speed.wrapping_mul(c.aim_fric_mult) / c.aim_fric_div;
        }

        if worm.direction == 1 {
            if ftoi(worm.aiming_angle) > c.aim_max_right {
                worm.aiming_speed = 0;
                worm.aiming_angle = itof(c.aim_max_right);
            }
            if ftoi(worm.aiming_angle) < c.aim_min_right {
                worm.aiming_speed = 0;
                worm.aiming_angle = itof(c.aim_min_right);
            }
        } else {
            if ftoi(worm.aiming_angle) < c.aim_max_left {
                worm.aiming_speed = 0;
                worm.aiming_angle = itof(c.aim_max_left);
            }
            if ftoi(worm.aiming_angle) > c.aim_min_left {
                worm.aiming_speed = 0;
                worm.aiming_angle = itof(c.aim_min_left);
            }
        }
    }

    if worm.movable && (!worm.ninjarope.out || !worm.control_states.get(ControlState::CHANGE)) {
        if k_up {
            if worm.direction == 0 {
                if worm.aiming_speed < c.max_aim_vel_left {
                    worm.aiming_speed = worm.aiming_speed.wrapping_add(c.aim_acc_left);
                }
            } else if worm.aiming_speed > c.max_aim_vel_right {
                worm.aiming_speed = worm.aiming_speed.wrapping_sub(c.aim_acc_right);
            }
        }

        if k_down {
            if worm.direction == 1 {
                if worm.aiming_speed < c.max_aim_vel_left {
                    worm.aiming_speed = worm.aiming_speed.wrapping_add(c.aim_acc_left);
                }
            } else if worm.aiming_speed > c.max_aim_vel_right {
                worm.aiming_speed = worm.aiming_speed.wrapping_sub(c.aim_acc_right);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ProcessTasks (worm.cpp:959-1001)
// ---------------------------------------------------------------------------

/// Port of `Worm::ProcessTasks` (`src/game/worm.cpp:959-1001`) — jump and
/// ninjarope throw/retract.
///
/// `reacts` is the per-tick reaction array from the reaction orchestration
/// ([`worm_reactions`](crate::physics::worm_reactions)); the grounded check
/// reads `reacts[kRfUp]` (index [`RF_UP`](crate::physics::RF_UP)), matching the
/// physics-fn style. Runs **before** `process_physics` in the per-worm pass, so
/// the jump impulse on `vel.y` is what the bounce/gravity checks then read.
///
/// In C++ order, split on whether the Change bit is held:
///
/// * **Change held** (`Pressed(kChange)`):
///   - If the rope is already out, C++ adjusts `ninjarope.length` by
///     `NRPullVel`/`NRReleaseVel` on Up/Down and clamps it to
///     `[NRMinLength, NRMaxLength]`. **Skipped here (design doc OQ5):**
///     `ninjarope.length` is *not* hashed and the rope's `Process` is not run
///     this slice (rope frozen), so pull/release is a no-op on hashed state.
///   - `PressedOnce(kJump)` throws the rope: sets `ninjarope.out = true`,
///     `ninjarope.pos = pos`, and **clears the Jump bit** in `control_states`
///     (the master hash reads `control_states.Pack()` post-`Process`). C++ also
///     sets `attached = false`, plays a sound, and computes `ninjarope.vel`
///     (via the `cossin_table`) and `ninjarope.length` — **all skipped**:
///     `attached`/`vel`/`length` are not hashed and the cossin table is not
///     pulled into this slice; only the hashed `out`/`pos` are written.
///
/// * **Change not held** (`else`):
///   - **Jump held** (`Pressed(kJump)`): retract the rope (`ninjarope.out =
///     false`), then jump *iff* `(reacts[kRfUp] > 0 || AirJump) && (able_to_jump
///     || MultiJump)` — apply `vel.y -= JumpForce` and clear `able_to_jump`.
///   - **Jump not held**: re-arm the jump (`able_to_jump = true`). This is the
///     set/clear edge: `able_to_jump` is true only after a tick with Jump
///     released, and the impulse clears it so holding Jump (sans `MultiJump`)
///     fires once.
///
/// `vel.y` arithmetic is `wrapping_sub` to match C++ `int` semantics.
pub fn process_tasks(worm: &mut WormState, reacts: &[i32; 4], c: &ControlConsts) {
    use crate::physics::RF_UP;

    if worm.control_states.get(ControlState::CHANGE) {
        // Rope pull/release on Up/Down adjusts `ninjarope.length` here in C++.
        // SKIPPED (design doc OQ5): `length` is non-hashed and the rope is frozen
        // this slice, so pull/release touches no hashed state.

        if worm.control_states.pressed_once(ControlState::JUMP) {
            // Throw the rope. Only the hashed fields are written:
            worm.ninjarope.out = true;
            worm.ninjarope.pos = worm.pos;
            // SKIPPED (OQ5): attached=false, sound, ninjarope.vel (cossin_table),
            // ninjarope.length = NRInitialLength — none are hashed.
        }
    } else {
        // Jump = remove ninjarope, jump.
        if worm.control_states.get(ControlState::JUMP) {
            worm.ninjarope.out = false;
            // SKIPPED: attached = false (non-hashed).

            if (reacts[RF_UP] > 0 || c.h_air_jump) && (worm.able_to_jump || c.h_multi_jump) {
                worm.vel.y = worm.vel.y.wrapping_sub(c.jump_force);
                worm.able_to_jump = false;
            }
        } else {
            worm.able_to_jump = true;
        }
    }
}

// ---------------------------------------------------------------------------
// ProcessWeapons (worm.cpp:811-848)
// ---------------------------------------------------------------------------

/// Port of `Worm::ProcessWeapons` (`src/game/worm.cpp:811-848`) — the per-tick
/// weapon-timer countdown.
///
/// This is the Slice-3 MASTER-hash linchpin: in Slice 2 the master hash diverged
/// precisely because the per-slot `delay_left` countdown was not ported. The
/// state hash (`stateHash.hpp:38-44`) reads each slot's `ammo`, `delay_left`,
/// `loading_left`, and `type->id`, so the arithmetic here must be bit-exact.
///
/// In C++ order:
///
/// 1. **Per-slot `delay_left` countdown** (`worm.cpp:814-818`): every slot with
///    `delay_left >= 0` decrements by 1. The `>= 0` guard means `0 -> -1` and then
///    holds at `-1` (off-by-one here is exactly what desynced the master in
///    Slice 2).
/// 2. **Current-weapon reload** (`worm.cpp:823-827`) — **DEFERRED to Slice 4**:
///    when `ammo <= 0`, C++ sets `loading_left = w.ComputedLoadingTime(...)` and
///    `ammo = w.ammo`. That needs the [`Weapon`](assets::object::Weapon)
///    definition (`ComputedLoadingTime`/`ammo`), which `WormState` does not carry,
///    and is only reached once `Worm::Fire` depletes ammo. This slice never sets
///    Fire, so `ammo > 0` on every slot and the branch never runs — guarded with a
///    `debug_assert!`.
/// 3. **Current-weapon loading countdown** (`worm.cpp:829-835`): while
///    `loading_left > 0`, decrement it. (The reload SOUND at `<= 0` is not
///    simulated — sound is not hashed.) `loading_left` only becomes `> 0` via the
///    deferred reload, so it stays `0` this slice; the decrement is a faithful
///    no-op here.
/// 4. **`fire_cone` countdown** (`worm.cpp:837-839`): decrement while `> 0`
///    (non-hashed).
/// 5. **Shell drop** (`worm.cpp:841-847`) — **DEFERRED to Slice 4**: when
///    `leave_shell_timer > 0`, C++ decrements it and on reaching `0` draws RNG
///    twice (`game.rand`) and spawns `nobject_types[7]`. Only `Worm::Fire` arms
///    `leave_shell_timer`, so it is `0` every tick this slice (keeping the RNG
///    pristine — design doc, *RNG decision*). Guarded with a `debug_assert!`; the
///    body is `unreachable!` since it is never armed without Fire.
///
/// Takes no `Rand`: the only RNG-drawing path (the shell drop) is unreachable
/// this slice, which is what keeps `rand.last == 0` every tick.
pub fn process_weapons(worm: &mut WormState) {
    // worm.cpp:814-818 — per-slot delay_left countdown (the MASTER-hash linchpin).
    // The `>= 0` guard is bounded, so a plain `- 1` cannot overflow.
    for weapon in worm.weapons.iter_mut() {
        if weapon.delay_left >= 0 {
            weapon.delay_left -= 1;
        }
    }

    let ww = &mut worm.weapons[worm.current_weapon as usize];

    // worm.cpp:823-827 — reload when depleted. DEFERRED to Slice 4 (Fire): needs
    // the Weapon definition (ComputedLoadingTime/ammo) not carried in WormState and
    // is only reached once Fire depletes ammo. ammo > 0 on every slot this slice.
    debug_assert!(
        ww.ammo > 0,
        "ProcessWeapons reload branch (ammo<=0) is Slice-4 (Fire) territory; \
         needs Weapon data and is unreached while ammo>0"
    );

    // worm.cpp:829-835 — loading countdown. loading_left only becomes > 0 via the
    // deferred reload, so this is a faithful no-op this slice; the SoundReloaded
    // play at <= 0 is not simulated (not hashed).
    if ww.loading_left > 0 {
        ww.loading_left -= 1;
    }

    // worm.cpp:837-839 — firecone countdown (non-hashed).
    if worm.fire_cone > 0 {
        worm.fire_cone -= 1;
    }

    // worm.cpp:841-847 — shell drop. DEFERRED to Slice 4 (Fire): only Worm::Fire
    // arms leave_shell_timer, and the timer-expiry body draws RNG twice and spawns
    // nobject_types[7]. It is 0 every tick this slice (RNG stays pristine).
    debug_assert!(
        worm.leave_shell_timer == 0,
        "ProcessWeapons shell-drop (leave_shell_timer>0) is Slice-4 (Fire) \
         territory; only Worm::Fire arms it and it draws RNG"
    );
    if worm.leave_shell_timer > 0 {
        // worm.cpp:842-846: --leave_shell_timer; on reaching 0, game.rand() twice +
        // nobject_types[7].Create1(...). Unimplemented: never armed without Fire.
        unreachable!("leave_shell_timer is always 0 this slice (no Fire path arms it)");
    }
}

// ---------------------------------------------------------------------------
// ProcessWeaponChange (worm.cpp:1064-1098)
// ---------------------------------------------------------------------------

/// Port of `Worm::ProcessWeaponChange` (`src/game/worm.cpp:1064-1098`) — cycle
/// the selected weapon while the Change key is held.
///
/// Runs in step 11 of the per-worm pass *only* when `Pressed(kChange)` (the
/// driver gates this against [`process_movement`]; Task 5 wires it). Hashed
/// output: `control_states.Pack()` — the Left/Right bit clears below. Non-hashed
/// `current_weapon`/`key_change_pressed` steer it across ticks.
///
/// In C++ order:
///
/// 1. **First change-tick edge** (`worm.cpp:1065-1070`): while
///    `!key_change_pressed`, `Release(kLeft)` and `Release(kRight)` clear those
///    bits and `key_change_pressed` latches `true`. Because the Release runs
///    *before* the `PressedOnce` reads below, the very first tick of a Change
///    hold consumes that tick's Left/Right press — so a fresh hold cycles on
///    tick 2 onward, not tick 1.
/// 2. `fire_cone = 0`, `animate = false` (`worm.cpp:1072-1073`) — non-hashed;
///    only `fire_cone` is tracked, `animate` is render-only (skipped).
/// 3. Loop-sound stop (`worm.cpp:1075-1077`) — SKIPPED (sound not hashed).
/// 4. **Cycle gate** (`worm.cpp:1079`): `weapons[current_weapon].Available() ||
///    settings->load_change`. `Available()` is `loading_left == 0`
///    (`worm.hpp:35`), which holds every tick this slice (no reload arms
///    `loading_left` — see [`process_weapons`]); `load_change` defaults `true`
///    (`settings.hpp:75`). So the gate is unconditionally true here and cycling
///    always runs; the `loading_left == 0` invariant is pinned with a
///    `debug_assert!`.
/// 5. **`PressedOnce(kLeft)`** (`worm.cpp:1080-1087`): decrement `current_weapon`,
///    wrapping below 0 to `kSelectableWeapons - 1` (== `NUM_WEAPONS - 1`), and
///    clear the Left bit. `hotspot_x/y` are render-only (skipped).
/// 6. **`PressedOnce(kRight)`** (`worm.cpp:1089-1096`): increment `current_weapon`,
///    wrapping at `kSelectableWeapons` back to 0, and clear the Right bit.
///
/// The `Unpack`-each-tick degeneration (design doc, *Control-state mutation*):
/// the driver re-`Unpack`s `control_states` from the scripted input every tick,
/// so once `key_change_pressed` is latched a held Change+Right re-sets the Right
/// bit each tick and `PressedOnce` fires every tick — holding for *k* steady
/// ticks cycles *k* times.
pub fn process_weapon_change(worm: &mut WormState) {
    // worm.cpp:1065-1070 — first change-tick: clear Left/Right, latch.
    if !worm.key_change_pressed {
        worm.control_states.release(ControlState::LEFT);
        worm.control_states.release(ControlState::RIGHT);
        worm.key_change_pressed = true;
    }

    // worm.cpp:1072 — fire_cone = 0. (animate = false at :1073 is render-only.)
    worm.fire_cone = 0;

    // worm.cpp:1075-1077 — loop-sound stop: SKIPPED (sound not hashed).

    // worm.cpp:1079 gate — Available() || load_change. Available() == loading_left
    // == 0 holds every tick this slice (no reload), and load_change defaults true,
    // so the gate is always entered. Pin the Available() invariant.
    debug_assert!(
        worm.weapons[worm.current_weapon as usize].loading_left == 0,
        "ProcessWeaponChange gate: Available() (loading_left==0) holds this slice; \
         reload is Slice-4 (Fire) territory"
    );

    // worm.cpp:1080-1087 — PressedOnce(Left): cycle down, wrap to NUM_WEAPONS-1.
    if worm.control_states.pressed_once(ControlState::LEFT) {
        worm.current_weapon -= 1;
        if worm.current_weapon < 0 {
            worm.current_weapon = NUM_WEAPONS as i32 - 1;
        }
        // hotspot_x/y = Ftoi(pos) — render-only, not hashed; skipped.
    }

    // worm.cpp:1089-1096 — PressedOnce(Right): cycle up, wrap to 0.
    if worm.control_states.pressed_once(ControlState::RIGHT) {
        worm.current_weapon += 1;
        if worm.current_weapon >= NUM_WEAPONS as i32 {
            worm.current_weapon = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// ProcessMovement (worm.cpp:850-957)
// ---------------------------------------------------------------------------

/// Port of `Worm::ProcessMovement` (`src/game/worm.cpp:850-957`) — walk left/
/// right, face the worm, and re-arm the dig (terrain dig body DEFERRED).
///
/// Runs in step 11 of the per-worm pass when Change is *not* held (the driver's
/// `else` branch; Task 5 wires it, and that branch also clears
/// `key_change_pressed`). Hashed outputs: `vel.x` (the walk accel) and
/// `aiming_angle` (the direction-flip `Itof(128) - aiming_angle`). Non-hashed
/// `direction`/`aiming_speed`/`able_to_dig` steer hashed state across ticks.
///
/// The whole body is gated on `movable` (`worm.cpp:853`). In C++ order:
///
/// * **Walk left** (`worm.cpp:857-871`, `kLeft && !kRight`): while `vel.x >
///   MaxVelLeft`, `vel.x -= WalkVelLeft` (a single guarded add — it may overshoot
///   the cap, there is no clamp to the exact value). On a facing change
///   (`direction != 0`): `aiming_speed = 0`, and if `aiming_angle >= Itof(64)`
///   flip it to `Itof(128) - aiming_angle`, then `direction = 0`.
/// * **Walk right** (`worm.cpp:873-887`, `!kLeft && kRight`): the mirror with
///   `WalkVelRight`/`MaxVelRight`, the `aiming_angle <= Itof(64)` flip, and
///   `direction = 1`.
/// * **Dig** (`worm.cpp:889-951`, `kLeft && kRight`): if `able_to_dig`, clear it
///   and run `DrawDirtEffect` — which draws `rand()` **and** writes the level.
///   That body is **DEFERRED to Slice 4**; only the `able_to_dig` toggle and the
///   Left+Right condition are ported, with the deferred call site guarded by a
///   `debug_assert!`. The slice-3 scenario never holds Left+Right, so this is
///   unreachable in the real run (design doc, *RNG decision* / *The hard 10%*).
///   The `else` (not both held) re-arms `able_to_dig = true`.
/// * Idle `animate = false` (`worm.cpp:953-955`) — render-only, skipped.
///
/// `vel.x` arithmetic is `wrapping_*` to match C++ `int` semantics; the angle
/// flip uses `itof`/`wrapping_sub` (same discipline as the aiming port).
pub fn process_movement(worm: &mut WormState, c: &ControlConsts) {
    if !worm.movable {
        return;
    }

    let k_left = worm.control_states.get(ControlState::LEFT);
    let k_right = worm.control_states.get(ControlState::RIGHT);

    // worm.cpp:857-871 — walk left.
    if k_left && !k_right {
        if worm.vel.x > c.max_vel_left {
            worm.vel.x = worm.vel.x.wrapping_sub(c.walk_vel_left);
        }
        if worm.direction != 0 {
            worm.aiming_speed = 0;
            if worm.aiming_angle >= itof(64) {
                worm.aiming_angle = itof(128).wrapping_sub(worm.aiming_angle);
            }
            worm.direction = 0;
        }
        // animate = true — render-only (skipped).
    }

    // worm.cpp:873-887 — walk right.
    if !k_left && k_right {
        if worm.vel.x < c.max_vel_right {
            worm.vel.x = worm.vel.x.wrapping_add(c.walk_vel_right);
        }
        if worm.direction != 1 {
            worm.aiming_speed = 0;
            if worm.aiming_angle <= itof(64) {
                worm.aiming_angle = itof(128).wrapping_sub(worm.aiming_angle);
            }
            worm.direction = 1;
        }
    }

    // worm.cpp:889-951 — dig detection. The DrawDirtEffect body is DEFERRED.
    if k_left && k_right {
        if worm.able_to_dig {
            worm.able_to_dig = false;
            // worm.cpp:893-948: DrawDirtEffect draws rand() AND writes material_id.
            // DEFERRED to Slice 4; the scenario never holds Left+Right, so this is
            // unreachable. A debug build trips this guard if it ever does.
            debug_assert!(
                false,
                "dig DrawDirtEffect deferred to Slice 4; scenario must not hold Left+Right"
            );
        }
    } else {
        worm.able_to_dig = true;
    }

    // worm.cpp:953-955 — idle animate = false (render-only, skipped).
}

#[cfg(test)]
mod tests {
    use super::*;
    use assets::tc::TcConfig;

    // tc.cfg fragment carrying every ControlConsts source key, with the real
    // data/TC/openliero values for a few spot-checks plus the two jump hacks.
    const SAMPLE: &str = r#"
[constants]
AimFricMult = 83
AimFricDiv = 100
AimMaxRight = 116
AimMinRight = 64
AimMaxLeft = 12
AimMinLeft = 64
MaxAimVelLeft = 70000
MaxAimVelRight = -70000
AimAccLeft = 4000
AimAccRight = 4000
WalkVelLeft = 3000
MaxVelLeft = -29184
WalkVelRight = 3000
MaxVelRight = 29184
JumpForce = 56064
NRInitialLength = 4000
NRMinLength = 170
NRMaxLength = 4000
NRPullVel = 24
NRReleaseVel = 24
NRThrowVelX = 2
NRThrowVelY = 2

[hacks]
AirJump = true
MultiJump = true
"#;

    #[test]
    fn from_tc_pulls_documented_constants_and_hacks() {
        let tc = TcConfig::load(SAMPLE.as_bytes()).unwrap();
        let cc = ControlConsts::from_tc(&tc);
        // Spot-check one field per group (design doc: JumpForce, AimFricMult,
        // MultiJump) plus the signed aim-vel and a ninjarope value.
        assert_eq!(cc.aim_fric_mult, 83);
        assert_eq!(cc.aim_fric_div, 100);
        assert_eq!(cc.jump_force, 56064);
        assert_eq!(cc.max_aim_vel_right, -70000, "signed: right is negative");
        assert_eq!(cc.max_vel_left, -29184);
        assert_eq!(cc.nr_min_length, 170);
        assert_eq!(cc.nr_throw_vel_y, 2);
        // Hacks come from [hacks], not [constants].
        assert!(cc.h_air_jump);
        assert!(cc.h_multi_jump);
    }

    #[test]
    fn from_tc_matches_openliero_default() {
        // The SAMPLE carries the real openliero constants (hacks differ: the
        // shipped TC has both jump hacks off). Every non-hack field must equal
        // the hardcoded Default.
        let tc = TcConfig::load(SAMPLE.as_bytes()).unwrap();
        let from_tc = ControlConsts::from_tc(&tc);
        let dflt = ControlConsts::default();
        assert_eq!(
            ControlConsts {
                h_air_jump: false,
                h_multi_jump: false,
                ..from_tc
            },
            dflt,
            "from_tc (sans hacks) must equal the openliero Default"
        );
    }

    #[test]
    fn missing_keys_default_to_zero() {
        // serde(default) on Constants: an empty TC yields all-zero constants.
        let tc = TcConfig::load(b"[constants]\n").unwrap();
        let cc = ControlConsts::from_tc(&tc);
        assert_eq!(cc.jump_force, 0);
        assert_eq!(cc.aim_fric_mult, 0);
        assert!(!cc.h_multi_jump);
    }

    // ---- process_aiming (ProcessAiming port) ---------------------------------
    //
    // All tests use the real `data/TC/openliero` aim constants
    // (`ControlConsts::default()`): AimFricMult/Div = 83/100, AimMaxRight 116,
    // AimMinRight 64, AimMaxLeft 12, AimMinLeft 64, MaxAimVelLeft 70000,
    // MaxAimVelRight -70000, AimAccLeft/Right 4000.

    use crate::state::{WeaponInit, WormInit, NUM_WEAPONS};

    // A bare, alive, in-bounds worm with the given direction/aim, no keys held,
    // movable, rope stowed. Tests set `control_states`/`movable`/`ninjarope.out`
    // as needed.
    fn aim_worm(direction: i32, aiming_speed: i32, aiming_angle: i32) -> WormState {
        let mut w = WormState::from_init(&WormInit {
            index: 0,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: sim_core::vec::Vec2::zero(),
            visible: true,
        });
        w.direction = direction;
        w.aiming_speed = aiming_speed;
        w.aiming_angle = aiming_angle;
        w
    }

    #[test]
    fn aim_up_accelerates_speed_then_angle_rises() {
        // direction 0 (left-facing): Up adds AimAccLeft while speed < MaxAimVelLeft.
        // With aiming_speed == 0 the integrate block is skipped, so the first tick
        // only accelerates; the second tick then integrates the new speed into the
        // angle (which rises) and accelerates again.
        let c = ControlConsts::default();
        let mut w = aim_worm(0, 0, itof(40)); // mid-range angle, no clamp
        w.control_states.press(ControlState::UP);

        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 4000, "Up accel: 0 + AimAccLeft");
        assert_eq!(
            w.aiming_angle,
            itof(40),
            "speed was 0 -> angle un-integrated"
        );

        process_aiming(&mut w, &c);
        // integrate uses the pre-accel speed (4000); Up held -> no friction.
        assert_eq!(w.aiming_angle, itof(40) + 4000, "angle += aiming_speed");
        assert_eq!(w.aiming_speed, 8000, "Up accel again: 4000 + AimAccLeft");
    }

    #[test]
    fn down_accelerates_when_facing_right() {
        // direction 1 + Down selects the additive (AimAccLeft) branch.
        let c = ControlConsts::default();
        let mut w = aim_worm(1, 0, itof(90));
        w.control_states.press(ControlState::DOWN);
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 4000, "Down/dir1: 0 + AimAccLeft");
    }

    #[test]
    fn down_decelerates_when_facing_left() {
        // direction 0 + Down selects the subtractive (AimAccRight) branch toward
        // the negative MaxAimVelRight cap.
        let c = ControlConsts::default();
        let mut w = aim_worm(0, 0, itof(40));
        w.control_states.press(ControlState::DOWN);
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, -4000, "Down/dir0: 0 - AimAccRight");
    }

    #[test]
    fn accel_caps_at_max_aim_vel_left() {
        // At the cap, the `aiming_speed < MaxAimVelLeft` guard is false -> no add.
        let c = ControlConsts::default();
        let mut w = aim_worm(0, 70000, itof(40)); // already at MaxAimVelLeft
        w.control_states.press(ControlState::UP);
        process_aiming(&mut w, &c);
        // integrate ran (speed != 0); Up held -> no friction; mid angle -> no clamp.
        assert_eq!(w.aiming_angle, itof(40) + 70000, "angle integrated");
        assert_eq!(
            w.aiming_speed, 70000,
            "speed pinned at MaxAimVelLeft (no add)"
        );
    }

    #[test]
    fn friction_truncates_negative_speed_toward_zero() {
        // No Up/Down -> friction = speed * 83 / 100, a TRUNCATING int division.
        // -150 * 83 / 100 = -12450 / 100 = -124 (toward zero), NOT -125 (floor/`>>`).
        let c = ControlConsts::default();
        let mut w = aim_worm(1, -150, itof(80)); // dir 1, mid range [64..116]
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, -124, "friction truncates toward zero");
        // angle integrated with the pre-friction speed; still in range -> no clamp.
        assert_eq!(w.aiming_angle, itof(80) - 150);
    }

    #[test]
    fn no_input_decays_via_friction_no_clamp() {
        // movable + no keys: accel gate passes but neither Up/Down fires, so only
        // friction acts. 1000 * 83 / 100 = 830.
        let c = ControlConsts::default();
        let mut w = aim_worm(1, 1000, itof(90));
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 830, "friction: 1000 * 83 / 100");
        assert_eq!(
            w.aiming_angle,
            itof(90) + 1000,
            "angle integrated, no clamp"
        );
    }

    #[test]
    fn clamp_right_max_pins_angle_and_zeroes_speed() {
        // direction 1: Ftoi(angle) > AimMaxRight (116) -> speed 0, angle = Itof(116).
        // movable = false so the accel block can't re-add to aiming_speed, isolating
        // the clamp's zeroing.
        let c = ControlConsts::default();
        let mut w = aim_worm(1, itof(1), itof(116)); // +1.0 push past the max
        w.movable = false;
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 0, "clamp zeroes aiming_speed");
        assert_eq!(w.aiming_angle, itof(116), "angle pinned to AimMaxRight");
    }

    #[test]
    fn clamp_right_min_pins_angle_and_zeroes_speed() {
        // direction 1: Ftoi(angle) < AimMinRight (64) -> speed 0, angle = Itof(64).
        let c = ControlConsts::default();
        let mut w = aim_worm(1, itof(-1), itof(64)); // -1.0 push below the min
        w.movable = false;
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 0);
        assert_eq!(w.aiming_angle, itof(64), "angle pinned to AimMinRight");
    }

    #[test]
    fn clamp_left_uses_aim_max_left_as_lower_pin() {
        // direction 0: Ftoi(angle) < AimMaxLeft (12) -> speed 0, angle = Itof(12).
        let c = ControlConsts::default();
        let mut w = aim_worm(0, itof(-1), itof(12)); // -1.0 push below AimMaxLeft
        w.movable = false;
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 0);
        assert_eq!(
            w.aiming_angle,
            itof(12),
            "angle pinned to AimMaxLeft (left lower)"
        );
    }

    #[test]
    fn clamp_left_uses_aim_min_left_as_upper_pin() {
        // direction 0: Ftoi(angle) > AimMinLeft (64) -> speed 0, angle = Itof(64).
        let c = ControlConsts::default();
        let mut w = aim_worm(0, itof(1), itof(64)); // +1.0 push above AimMinLeft
        w.movable = false;
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 0);
        assert_eq!(
            w.aiming_angle,
            itof(64),
            "angle pinned to AimMinLeft (left upper)"
        );
    }

    #[test]
    fn accel_gated_off_when_not_movable() {
        // !movable -> the whole accel block is skipped; speed stays put.
        let c = ControlConsts::default();
        let mut w = aim_worm(0, 0, itof(40));
        w.movable = false;
        w.control_states.press(ControlState::UP);
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 0, "no accel when !movable");
    }

    #[test]
    fn accel_gated_off_when_rope_out_and_change_held() {
        // movable but ninjarope.out && Change held -> accel gate is false.
        let c = ControlConsts::default();
        let mut w = aim_worm(0, 0, itof(40));
        w.ninjarope.out = true;
        w.control_states.press(ControlState::UP);
        w.control_states.press(ControlState::CHANGE);
        process_aiming(&mut w, &c);
        assert_eq!(w.aiming_speed, 0, "no accel when rope out and Change held");

        // Same rope-out state but Change NOT held -> accel resumes (!out||!change).
        let mut w2 = aim_worm(0, 0, itof(40));
        w2.ninjarope.out = true;
        w2.control_states.press(ControlState::UP);
        process_aiming(&mut w2, &c);
        assert_eq!(
            w2.aiming_speed, 4000,
            "rope out but Change clear -> accel runs"
        );
    }

    // ---- process_tasks (ProcessTasks port) -----------------------------------
    //
    // Hand-folded against worm.cpp:959-1001. Hashed outputs under test: `vel.y`
    // (jump), `ninjarope.out`/`pos`, and `control_states.Pack()` (Jump-bit clear
    // on throw). Non-hashed `able_to_jump` drives the jump edge across ticks.
    // Uses the real `data/TC/openliero` JumpForce (56064) via Default (hacks off).

    use sim_core::vec::Vec2;

    // A bare, alive worm with no keys held, rope stowed, at a known position.
    fn task_worm() -> WormState {
        WormState::from_init(&WormInit {
            index: 0,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::new(123, 456),
            visible: true,
        })
    }

    #[test]
    fn jump_grounded_and_able_applies_impulse_and_retracts_rope() {
        // !Change && Jump, reacts[kRfUp] > 0, able_to_jump (hacks off) ->
        // vel.y -= JumpForce, able_to_jump = false, ninjarope.out = false.
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.vel = Vec2::new(0, 1000);
        w.able_to_jump = true;
        w.ninjarope.out = true; // jump branch must retract it
        w.control_states.press(ControlState::JUMP);
        let reacts = [0, 0, 1, 0]; // kRfUp = index 2 -> grounded

        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, 1000 - 56064, "vel.y -= JumpForce");
        assert!(!w.able_to_jump, "impulse clears able_to_jump");
        assert!(!w.ninjarope.out, "jump branch retracts the rope");
    }

    #[test]
    fn jump_airborne_is_gated_no_impulse() {
        // reacts[kRfUp] == 0 and AirJump off -> the jump condition fails: vel.y
        // unchanged, able_to_jump unchanged. Rope still retracts (out := false).
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.vel = Vec2::new(0, 1000);
        w.able_to_jump = true;
        w.control_states.press(ControlState::JUMP);
        let reacts = [0, 0, 0, 0]; // not grounded

        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, 1000, "airborne -> no impulse");
        assert!(w.able_to_jump, "able_to_jump untouched when gated");
        assert!(!w.ninjarope.out);
    }

    #[test]
    fn jump_grounded_but_not_able_no_impulse() {
        // Grounded but able_to_jump false (and MultiJump off) -> no impulse. This
        // is the "held Jump fires once" gate.
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.vel = Vec2::new(0, 1000);
        w.able_to_jump = false;
        w.control_states.press(ControlState::JUMP);
        let reacts = [0, 0, 1, 0]; // grounded

        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, 1000, "not able_to_jump -> no impulse");
        assert!(!w.able_to_jump);
    }

    #[test]
    fn able_to_jump_rearms_when_jump_released() {
        // !Change && !Jump -> able_to_jump = true (re-arm).
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.able_to_jump = false;
        // No keys held.
        let reacts = [0, 0, 1, 0];
        process_tasks(&mut w, &reacts, &c);
        assert!(w.able_to_jump, "Jump released -> able_to_jump re-armed");
    }

    #[test]
    fn able_to_jump_edge_across_ticks_fires_once() {
        // Tick 1: release Jump -> able_to_jump becomes true.
        // Tick 2: hold Jump, grounded -> impulse, able_to_jump becomes false.
        // Tick 3: hold Jump again, still grounded -> gated (no second impulse)
        //         until Jump is released again.
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.vel = Vec2::new(0, 0);
        w.able_to_jump = false;
        let reacts = [0, 0, 1, 0]; // grounded throughout

        // Tick 1: Jump released.
        process_tasks(&mut w, &reacts, &c);
        assert!(w.able_to_jump, "tick1 re-arms");
        assert_eq!(w.vel.y, 0);

        // Tick 2: Jump held -> fires.
        w.control_states.press(ControlState::JUMP);
        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, -56064, "tick2 fires the impulse");
        assert!(!w.able_to_jump, "tick2 clears able_to_jump");

        // Tick 3: Jump still held -> gated, no second impulse.
        w.control_states.press(ControlState::JUMP);
        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, -56064, "tick3 gated: vel.y unchanged");
    }

    #[test]
    fn air_jump_hack_allows_jump_off_ground() {
        // AirJump on: the grounded test passes via the hack even with reacts[kRfUp]
        // == 0, as long as able_to_jump (MultiJump off).
        let mut c = ControlConsts::default();
        c.h_air_jump = true;
        let mut w = task_worm();
        w.able_to_jump = true;
        w.control_states.press(ControlState::JUMP);
        let reacts = [0, 0, 0, 0]; // airborne

        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, -56064, "AirJump: jump off the ground");
        assert!(!w.able_to_jump);
    }

    #[test]
    fn multi_jump_hack_ignores_able_to_jump() {
        // MultiJump on: jump fires even when able_to_jump is false (grounded).
        let mut c = ControlConsts::default();
        c.h_multi_jump = true;
        let mut w = task_worm();
        w.able_to_jump = false;
        w.control_states.press(ControlState::JUMP);
        let reacts = [0, 0, 1, 0]; // grounded

        process_tasks(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, -56064, "MultiJump: fires despite !able_to_jump");
    }

    #[test]
    fn ninjarope_throw_sets_out_pos_and_clears_jump_bit() {
        // Change + PressedOnce(Jump) -> out=true, pos=worm.pos, Jump bit cleared.
        let c = ControlConsts::default();
        let mut w = task_worm(); // pos = (123, 456)
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::JUMP);

        process_tasks(&mut w, &[0, 0, 0, 0], &c);
        assert!(w.ninjarope.out, "throw sets out");
        assert_eq!(w.ninjarope.pos, Vec2::new(123, 456), "pos = worm.pos");
        // PressedOnce(kJump) consumed the Jump bit; Change stays set.
        assert!(
            !w.control_states.get(ControlState::JUMP),
            "throw clears the Jump bit"
        );
        assert!(
            w.control_states.get(ControlState::CHANGE),
            "Change untouched"
        );
        assert_eq!(
            w.control_states.pack(),
            1 << ControlState::CHANGE,
            "pack() reflects: only Change set"
        );
    }

    #[test]
    fn ninjarope_throw_does_not_jump_or_touch_vel() {
        // The throw branch (Change held) is entirely separate from the jump
        // branch: vel and able_to_jump are untouched even when grounded.
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.vel = Vec2::new(7, 9);
        w.able_to_jump = true;
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::JUMP);

        process_tasks(&mut w, &[0, 0, 1, 0], &c); // grounded
        assert_eq!(w.vel, Vec2::new(7, 9), "throw doesn't apply jump impulse");
        assert!(w.able_to_jump, "throw doesn't touch able_to_jump");
    }

    #[test]
    fn change_without_jump_does_not_throw() {
        // Change held but Jump not pressed -> no throw, rope state unchanged.
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.control_states.press(ControlState::CHANGE);
        process_tasks(&mut w, &[0, 0, 0, 0], &c);
        assert!(!w.ninjarope.out, "no Jump -> no throw");
        assert_eq!(w.ninjarope.pos, Vec2::zero(), "pos untouched");
    }

    #[test]
    fn jump_branch_retracts_out_rope_even_when_jump_gated() {
        // Even if the jump is gated (airborne, no AirJump), the !Change && Jump
        // branch always sets ninjarope.out = false (rope retract).
        let c = ControlConsts::default();
        let mut w = task_worm();
        w.ninjarope.out = true;
        w.ninjarope.pos = Vec2::new(5, 5);
        w.able_to_jump = false;
        w.control_states.press(ControlState::JUMP);
        process_tasks(&mut w, &[0, 0, 0, 0], &c); // airborne, gated
        assert!(!w.ninjarope.out, "retract even when jump impulse is gated");
    }

    // ---- process_weapons (ProcessWeapons port) -------------------------------
    //
    // Hand-folded against worm.cpp:811-848. Hashed outputs under test: every
    // slot's `delay_left` (the per-tick countdown — the Slice-2 MASTER-hash
    // linchpin), plus the current weapon's `loading_left`/`ammo`. Non-hashed
    // `fire_cone`/`leave_shell_timer` countdowns are also checked. The `ammo<=0`
    // reload and `leave_shell_timer>0` shell-drop branches are Slice-4 (Fire)
    // territory and never run this slice (`ammo>0`, `leave_shell_timer==0`); see
    // `process_weapons` for the deferral.

    // A bare worm whose every weapon slot carries ammo (so the current-weapon
    // `ammo<=0` reload branch — which needs Weapon data we don't carry — is never
    // entered) and the given per-slot `delay_left`. current_weapon stays 0.
    fn weapon_worm(delays: [i32; NUM_WEAPONS]) -> WormState {
        let mut w = WormState::from_init(&WormInit {
            index: 0,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit {
                ty: Some(0),
                ammo: 10,
            }; NUM_WEAPONS],
            start_pos: Vec2::zero(),
            visible: true,
        });
        for (slot, d) in w.weapons.iter_mut().zip(delays.iter()) {
            slot.delay_left = *d;
        }
        w
    }

    #[test]
    fn delay_left_decrements_all_slots_and_stops_at_floor() {
        // worm.cpp:814-818: every slot with delay_left >= 0 decrements by 1.
        // Boundary: 1 -> 0 -> stays -1; 0 -> -1 -> stays -1; -1 stays -1 (>= 0 guard).
        let mut w = weapon_worm([3, 1, 0, -1, 5]);

        process_weapons(&mut w);
        let after1: Vec<i32> = w.weapons.iter().map(|s| s.delay_left).collect();
        assert_eq!(after1, vec![2, 0, -1, -1, 4], "tick 1: each >= 0 slot -= 1");

        process_weapons(&mut w);
        let after2: Vec<i32> = w.weapons.iter().map(|s| s.delay_left).collect();
        // slot1 hit the floor (0 -> -1), slot2 already at -1 stays, slots 0/4 keep ticking.
        assert_eq!(
            after2,
            vec![1, -1, -1, -1, 3],
            "tick 2: stops at the -1 floor"
        );
    }

    #[test]
    fn delay_left_zero_goes_to_minus_one_then_holds() {
        // The exact >= 0 boundary on a single slot across three ticks: 0 -> -1 -> -1.
        let mut w = weapon_worm([0, 0, 0, 0, 0]);
        process_weapons(&mut w);
        assert_eq!(
            w.weapons[0].delay_left, -1,
            "0 -> -1 (decrement ran at the boundary)"
        );
        process_weapons(&mut w);
        assert_eq!(
            w.weapons[0].delay_left, -1,
            "-1 stays -1 (guard blocks further decrement)"
        );
    }

    #[test]
    fn ammo_positive_leaves_loading_left_and_ammo_untouched() {
        // worm.cpp:823-827: reload branch is gated on ammo <= 0. With ammo > 0 it
        // is skipped, so loading_left stays 0 and ammo is unchanged (the scenario's
        // hashed invariant: no Fire -> ammo never depletes -> no reload).
        let mut w = weapon_worm([5, 5, 5, 5, 5]);
        w.weapons[0].loading_left = 0;
        process_weapons(&mut w);
        assert_eq!(
            w.weapons[0].loading_left, 0,
            "ammo>0 -> reload skipped -> loading_left stays 0"
        );
        assert_eq!(w.weapons[0].ammo, 10, "ammo untouched without a reload");
    }

    #[test]
    fn loading_left_counts_down_when_positive() {
        // worm.cpp:829-835: once a reload has armed loading_left (> 0) it decrements
        // each tick (ammo stays > 0 post-reload, so the reload branch is skipped).
        // The reload SOUND at <= 0 (832-834) is not simulated (not hashed).
        let mut w = weapon_worm([5, 5, 5, 5, 5]);
        w.weapons[0].loading_left = 3;
        process_weapons(&mut w);
        assert_eq!(w.weapons[0].loading_left, 2, "loading_left -= 1");
        assert_eq!(w.weapons[0].ammo, 10, "ammo unchanged while reloading");
        process_weapons(&mut w);
        assert_eq!(w.weapons[0].loading_left, 1);
    }

    #[test]
    fn loading_left_only_touches_current_weapon() {
        // The reload/loading countdown reads weapons[current_weapon] only. A
        // non-current slot's loading_left is left alone.
        let mut w = weapon_worm([5, 5, 5, 5, 5]);
        w.current_weapon = 2;
        w.weapons[2].loading_left = 4;
        w.weapons[0].loading_left = 9; // non-current; must not move
        process_weapons(&mut w);
        assert_eq!(w.weapons[2].loading_left, 3, "current weapon counts down");
        assert_eq!(w.weapons[0].loading_left, 9, "non-current weapon untouched");
    }

    #[test]
    fn fire_cone_decrements_while_positive_and_stops_at_zero() {
        // worm.cpp:837-839: fire_cone-- while > 0, holds at 0 (not hashed, but a
        // faithful per-tick countdown).
        let mut w = weapon_worm([5, 5, 5, 5, 5]);
        w.fire_cone = 2;
        process_weapons(&mut w);
        assert_eq!(w.fire_cone, 1);
        process_weapons(&mut w);
        assert_eq!(w.fire_cone, 0);
        process_weapons(&mut w);
        assert_eq!(w.fire_cone, 0, "fire_cone holds at 0 (> 0 guard)");
    }

    #[test]
    fn leave_shell_timer_zero_skips_shell_branch_without_panic() {
        // worm.cpp:841-847: the shell-drop branch is gated on leave_shell_timer > 0
        // and draws RNG. It is Slice-4 (Fire) territory; leave_shell_timer == 0
        // every tick this slice, so process_weapons must skip it cleanly (no panic,
        // timer stays 0, RNG untouched — process_weapons takes no Rand).
        let mut w = weapon_worm([5, 5, 5, 5, 5]);
        assert_eq!(w.leave_shell_timer, 0);
        process_weapons(&mut w);
        assert_eq!(
            w.leave_shell_timer, 0,
            "shell branch skipped, timer untouched"
        );
    }

    // ---- process_weapon_change (ProcessWeaponChange port) --------------------
    //
    // Hand-folded against worm.cpp:1064-1098. Hashed output under test:
    // `control_states.Pack()` (the Left/Right bit clears via Release/PressedOnce).
    // Non-hashed `current_weapon`/`key_change_pressed` steer it across ticks.
    // Cycle direction: PressedOnce(Left) decrements (wraps to NUM_WEAPONS-1),
    // PressedOnce(Right) increments (wraps to 0); kSelectableWeapons == NUM_WEAPONS.

    // A bare, movable worm with a carried weapon in every slot (loading_left == 0
    // so the Available() gate holds), current_weapon 0, key_change_pressed false.
    fn change_worm() -> WormState {
        WormState::from_init(&WormInit {
            index: 0,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit {
                ty: Some(0),
                ammo: 10,
            }; NUM_WEAPONS],
            start_pos: Vec2::zero(),
            visible: true,
        })
    }

    #[test]
    fn weapon_change_first_tick_releases_left_right_and_latches() {
        // worm.cpp:1065-1070: !key_change_pressed -> Release(Left), Release(Right),
        // key_change_pressed = true. The Release pre-clears the bits, so the
        // PressedOnce reads below see false -> NO cycle on the first change-tick.
        let mut w = change_worm();
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::LEFT);
        w.control_states.press(ControlState::RIGHT);

        process_weapon_change(&mut w);

        assert!(
            w.key_change_pressed,
            "first tick latches key_change_pressed"
        );
        assert!(!w.control_states.get(ControlState::LEFT), "Left released");
        assert!(!w.control_states.get(ControlState::RIGHT), "Right released");
        assert_eq!(w.current_weapon, 0, "first tick eats the press -> no cycle");
        assert_eq!(
            w.control_states.pack(),
            1 << ControlState::CHANGE,
            "pack(): only Change remains set"
        );
    }

    #[test]
    fn weapon_change_right_cycles_up_and_clears_bit() {
        // Steady state (key_change_pressed already latched): PressedOnce(Right)
        // increments current_weapon and clears the Right bit (worm.cpp:1089-1096).
        let mut w = change_worm();
        w.key_change_pressed = true;
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::RIGHT);

        process_weapon_change(&mut w);

        assert_eq!(w.current_weapon, 1, "Right: current_weapon 0 -> 1");
        assert!(
            !w.control_states.get(ControlState::RIGHT),
            "Right bit cleared"
        );
        assert_eq!(
            w.control_states.pack(),
            1 << ControlState::CHANGE,
            "pack(): only Change remains"
        );
    }

    #[test]
    fn weapon_change_right_wraps_to_zero() {
        // worm.cpp:1090-1092: ++current_weapon >= kSelectableWeapons -> 0.
        let mut w = change_worm();
        w.key_change_pressed = true;
        w.current_weapon = NUM_WEAPONS as i32 - 1; // 4
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::RIGHT);

        process_weapon_change(&mut w);

        assert_eq!(w.current_weapon, 0, "Right at slot 4 wraps to 0");
    }

    #[test]
    fn weapon_change_left_cycles_down_and_clears_bit() {
        // PressedOnce(Left) decrements current_weapon and clears the Left bit
        // (worm.cpp:1080-1087).
        let mut w = change_worm();
        w.key_change_pressed = true;
        w.current_weapon = 2;
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::LEFT);

        process_weapon_change(&mut w);

        assert_eq!(w.current_weapon, 1, "Left: current_weapon 2 -> 1");
        assert!(
            !w.control_states.get(ControlState::LEFT),
            "Left bit cleared"
        );
        assert_eq!(w.control_states.pack(), 1 << ControlState::CHANGE);
    }

    #[test]
    fn weapon_change_left_wraps_to_max() {
        // worm.cpp:1081-1083: --current_weapon < 0 -> kSelectableWeapons - 1.
        let mut w = change_worm();
        w.key_change_pressed = true;
        w.current_weapon = 0;
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::LEFT);

        process_weapon_change(&mut w);

        assert_eq!(
            w.current_weapon,
            NUM_WEAPONS as i32 - 1,
            "Left at slot 0 wraps to NUM_WEAPONS-1 (4)"
        );
    }

    #[test]
    fn weapon_change_left_and_right_same_tick_net_zero() {
        // Steady state, both Left and Right held (re-set by Unpack): PressedOnce
        // fires both -> -1 then +1 -> net unchanged, both bits cleared. Order is
        // Left (decrement) then Right (increment) per worm.cpp:1080-1096.
        let mut w = change_worm();
        w.key_change_pressed = true;
        w.current_weapon = 3;
        w.control_states.press(ControlState::CHANGE);
        w.control_states.press(ControlState::LEFT);
        w.control_states.press(ControlState::RIGHT);

        process_weapon_change(&mut w);

        assert_eq!(w.current_weapon, 3, "Left then Right: -1 then +1 -> net 0");
        assert_eq!(
            w.control_states.pack(),
            1 << ControlState::CHANGE,
            "both Left and Right bits cleared"
        );
    }

    #[test]
    fn weapon_change_unpack_each_tick_cycles_k_times() {
        // The Unpack-each-tick degeneration (design doc, *Control-state mutation*):
        // once key_change_pressed is latched, re-Unpacking Change+Right every tick
        // makes PressedOnce(Right) fire each tick -> holding for k ticks cycles k
        // times. k = 7 from slot 0 wraps: 7 % 5 == 2.
        let mut w = change_worm();
        w.key_change_pressed = true; // already latched (steady hold)
        for _ in 0..7 {
            // Mimic the driver's per-tick Unpack of the scripted input.
            w.control_states =
                ControlState::unpack((1 << ControlState::CHANGE) | (1 << ControlState::RIGHT));
            process_weapon_change(&mut w);
        }
        assert_eq!(
            w.current_weapon,
            7 % NUM_WEAPONS as i32,
            "7 cycles from 0 -> 2"
        );
    }

    #[test]
    fn weapon_change_first_tick_then_steady_cycles_k_minus_one() {
        // Starting unlatched (key_change_pressed false): the first change-tick's
        // Release eats that tick's Right press, so holding Change+Right for k=3
        // ticks yields only 2 cycles (tick1 = latch+Release, ticks 2-3 cycle).
        let mut w = change_worm(); // key_change_pressed = false
        for _ in 0..3 {
            w.control_states =
                ControlState::unpack((1 << ControlState::CHANGE) | (1 << ControlState::RIGHT));
            process_weapon_change(&mut w);
        }
        assert_eq!(
            w.current_weapon, 2,
            "first tick eats one: 3 ticks -> 2 cycles"
        );
    }

    // ---- process_movement (ProcessMovement port) ----------------------------
    //
    // Hand-folded against worm.cpp:850-957. Hashed outputs under test: `vel.x`
    // (walk accel, truncating) and `aiming_angle` (the direction-flip
    // `Itof(128) - aiming_angle`). Non-hashed `direction`/`aiming_speed`/
    // `able_to_dig` steer hashed state across ticks. The dig DrawDirtEffect body
    // (worm.cpp:893-948) is DEFERRED to Slice 4 — only the able_to_dig toggle +
    // the Left+Right condition are ported, guarded with a `debug_assert!`. Uses
    // the real `data/TC/openliero` walk constants via Default: WalkVelLeft/Right
    // 3000, MaxVelLeft -29184, MaxVelRight 29184.

    // A bare, movable worm at rest with the given facing direction.
    fn move_worm(direction: i32) -> WormState {
        let mut w = change_worm();
        w.direction = direction;
        w
    }

    #[test]
    fn walk_right_accelerates_vel_x() {
        // direction already 1 (no flip): vel.x += WalkVelRight while < MaxVelRight.
        let c = ControlConsts::default();
        let mut w = move_worm(1);
        w.vel = Vec2::new(0, 0);
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
        assert_eq!(w.vel.x, 3000, "vel.x += WalkVelRight");
        assert_eq!(w.direction, 1, "already facing right -> no flip");
    }

    #[test]
    fn walk_right_caps_at_max_vel_right() {
        // At/above MaxVelRight the `vel.x < MaxVelRight` guard is false -> no add.
        let c = ControlConsts::default();
        let mut w = move_worm(1);
        w.vel = Vec2::new(29184, 0); // == MaxVelRight
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
        assert_eq!(w.vel.x, 29184, "at the cap -> no further accel");

        // Just below the cap: a single guarded add overshoots it (C++ adds once,
        // no clamp to the exact cap).
        let mut w2 = move_worm(1);
        w2.vel = Vec2::new(29000, 0); // < MaxVelRight
        w2.control_states.press(ControlState::RIGHT);
        process_movement(&mut w2, &c);
        assert_eq!(
            w2.vel.x, 32000,
            "below cap -> one add overshoots (no clamp)"
        );
    }

    #[test]
    fn walk_left_accelerates_vel_x() {
        // direction already 0 (no flip): vel.x -= WalkVelLeft while > MaxVelLeft.
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.vel = Vec2::new(0, 0);
        w.control_states.press(ControlState::LEFT);
        process_movement(&mut w, &c);
        assert_eq!(w.vel.x, -3000, "vel.x -= WalkVelLeft");
        assert_eq!(w.direction, 0, "already facing left -> no flip");
    }

    #[test]
    fn walk_left_caps_at_max_vel_left() {
        // At MaxVelLeft the `vel.x > MaxVelLeft` guard is false -> no subtract.
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.vel = Vec2::new(-29184, 0); // == MaxVelLeft
        w.control_states.press(ControlState::LEFT);
        process_movement(&mut w, &c);
        assert_eq!(w.vel.x, -29184, "at the cap -> no further accel");
    }

    #[test]
    fn walk_right_flips_direction_and_aiming_angle() {
        // direction 0 -> 1: aiming_speed = 0, and (aiming_angle <= Itof(64)) ->
        // aiming_angle = Itof(128) - aiming_angle. 30 <= 64 -> 128 - 30 = 98.
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.aiming_speed = 5000;
        w.aiming_angle = itof(30);
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
        assert_eq!(w.direction, 1, "direction flips to right");
        assert_eq!(w.aiming_speed, 0, "aiming_speed zeroed on flip");
        assert_eq!(w.aiming_angle, itof(98), "Itof(128) - Itof(30) = Itof(98)");
    }

    #[test]
    fn walk_left_flips_direction_and_aiming_angle() {
        // direction 1 -> 0: aiming_speed = 0, and (aiming_angle >= Itof(64)) ->
        // aiming_angle = Itof(128) - aiming_angle. 100 >= 64 -> 128 - 100 = 28.
        let c = ControlConsts::default();
        let mut w = move_worm(1);
        w.aiming_speed = -5000;
        w.aiming_angle = itof(100);
        w.control_states.press(ControlState::LEFT);
        process_movement(&mut w, &c);
        assert_eq!(w.direction, 0, "direction flips to left");
        assert_eq!(w.aiming_speed, 0, "aiming_speed zeroed on flip");
        assert_eq!(w.aiming_angle, itof(28), "Itof(128) - Itof(100) = Itof(28)");
    }

    #[test]
    fn walk_right_flip_skips_angle_when_above_64() {
        // direction 0 -> 1 but aiming_angle > Itof(64): the `<= Itof(64)` guard is
        // false, so the angle is NOT flipped (aiming_speed still zeroes).
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.aiming_speed = 7;
        w.aiming_angle = itof(100); // > 64 -> no flip
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
        assert_eq!(w.direction, 1);
        assert_eq!(
            w.aiming_speed, 0,
            "speed still zeroed on a direction change"
        );
        assert_eq!(w.aiming_angle, itof(100), "angle untouched (100 > 64)");
    }

    #[test]
    fn movement_gated_off_when_not_movable() {
        // !movable -> the whole body is skipped; nothing moves or toggles.
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.movable = false;
        w.able_to_dig = false;
        w.vel = Vec2::new(0, 0);
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
        assert_eq!(w.vel.x, 0, "no walk when !movable");
        assert_eq!(w.direction, 0, "no flip when !movable");
        assert!(!w.able_to_dig, "able_to_dig untouched when !movable");
    }

    #[test]
    fn able_to_dig_rearms_on_single_direction() {
        // worm.cpp:949-951: !(Left && Right) -> able_to_dig = true. A single
        // direction (or none) re-arms the dig.
        let c = ControlConsts::default();
        let mut w = move_worm(1);
        w.able_to_dig = false;
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
        assert!(w.able_to_dig, "single-direction input re-arms able_to_dig");
    }

    #[test]
    fn able_to_dig_rearms_when_idle() {
        // No keys -> !(Left && Right) -> able_to_dig = true.
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.able_to_dig = false;
        process_movement(&mut w, &c);
        assert!(w.able_to_dig, "idle re-arms able_to_dig");
    }

    #[test]
    fn single_direction_does_not_trigger_dig_assert() {
        // The slice-3 scenario only ever holds a single direction; with able_to_dig
        // true, a single-direction walk must NOT reach the deferred dig assert.
        let c = ControlConsts::default();
        let mut w = move_worm(1);
        w.able_to_dig = true;
        w.vel = Vec2::new(0, 0);
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c); // must not panic
        assert_eq!(w.vel.x, 3000, "single-direction walk still accelerates");
        assert!(w.able_to_dig, "single direction leaves able_to_dig set");
    }

    #[test]
    #[should_panic(expected = "dig DrawDirtEffect deferred to Slice 4")]
    fn dig_branch_with_able_to_dig_trips_deferred_assert() {
        // worm.cpp:889-948: Left && Right && able_to_dig reaches the DrawDirtEffect
        // body, which is DEFERRED. In debug it trips the guard. The scenario never
        // holds Left+Right, so this is unreachable in the real run.
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.able_to_dig = true;
        w.control_states.press(ControlState::LEFT);
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c);
    }

    #[test]
    fn dig_branch_without_able_to_dig_does_not_panic() {
        // Left && Right but able_to_dig false: the inner `if able_to_dig` is
        // skipped, so the deferred body is never reached (no panic). able_to_dig
        // stays false (the else re-arm is not taken when both are held).
        let c = ControlConsts::default();
        let mut w = move_worm(0);
        w.able_to_dig = false;
        w.control_states.press(ControlState::LEFT);
        w.control_states.press(ControlState::RIGHT);
        process_movement(&mut w, &c); // must not panic
        assert!(
            !w.able_to_dig,
            "L+R with able_to_dig false stays false (no re-arm)"
        );
    }
}
