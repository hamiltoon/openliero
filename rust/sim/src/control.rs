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

use crate::state::{ControlState, WormState};

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
        assert!(w.control_states.get(ControlState::CHANGE), "Change untouched");
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
}
