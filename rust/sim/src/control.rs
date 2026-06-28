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
            ControlConsts { h_air_jump: false, h_multi_jump: false, ..from_tc },
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
}