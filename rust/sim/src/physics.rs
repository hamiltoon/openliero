//! Worm physics ports.
//!
//! Slice 2 begins with the collision-reaction probe, a direct port of
//! `Worm::CalculateReactionForce` (`src/game/worm.cpp:97-147`). It samples a
//! small fixed pattern of pixels around a candidate `(x, y)` for one of four
//! directions and counts how many land on a *non-background* (solid) material —
//! the per-direction "reaction" the integrator later uses to bounce/stop the
//! worm. The probe pattern and per-direction point counts are copied verbatim
//! from the C++ table so the counts match bit-for-bit.

use crate::state::{LevelSim, WormState};
use sim_core::fixed::{ftoi, itof};

/// Reaction-direction indices, mirroring the anonymous enum in
/// `Worm` (`worm.hpp:137`): `{ kRfDown, kRfLeft, kRfUp, kRfRight }`.
pub const RF_DOWN: usize = 0;
pub const RF_LEFT: usize = 1;
pub const RF_UP: usize = 2;
pub const RF_RIGHT: usize = 3;

/// A single probe offset, mirroring the C++ `Point { int x, y; }` used in
/// `kColPoints`.
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

const fn p(x: i32, y: i32) -> Point {
    Point { x, y }
}

/// The collision-probe offset table, copied verbatim from `worm.cpp:98-131`.
///
/// One row per direction (`kRfDown`, `kRfLeft`, `kRfUp`, `kRfRight`); the DOWN
/// and UP rows only use their first three points (see [`COL_POINT_COUNT`]) and
/// pad the rest with `(0, 0)` exactly as the C++ does.
const COL_POINTS: [[Point; 7]; 4] = [
    // DOWN reaction points
    [
        p(-1, -4),
        p(0, -4),
        p(1, -4),
        p(0, 0),
        p(0, 0),
        p(0, 0),
        p(0, 0),
    ],
    // LEFT reaction points
    [
        p(1, -3),
        p(1, -2),
        p(1, -1),
        p(1, 0),
        p(1, 1),
        p(1, 2),
        p(1, 3),
    ],
    // UP reaction points
    [
        p(-1, 4),
        p(0, 4),
        p(1, 4),
        p(0, 0),
        p(0, 0),
        p(0, 0),
        p(0, 0),
    ],
    // RIGHT reaction points
    [
        p(-1, -3),
        p(-1, -2),
        p(-1, -1),
        p(-1, 0),
        p(-1, 1),
        p(-1, 2),
        p(-1, 3),
    ],
];

/// How many of each direction's [`COL_POINTS`] are live, from
/// `kColPointCount[4] = {3, 7, 3, 7}` (`worm.cpp:133`): DOWN/UP probe 3 points,
/// LEFT/RIGHT probe 7.
pub const COL_POINT_COUNT: [i32; 4] = [3, 7, 3, 7];

/// Port of `Worm::CalculateReactionForce` (`worm.cpp:97-147`).
///
/// For direction `dir`, resets `reacts[dir] = 0`, then probes the
/// `COL_POINT_COUNT[dir]` offsets in `COL_POINTS[dir]` around `(x, y)` and adds
/// one for every offset whose pixel is **not** background
/// (`!level.checked_mat_background(x + dx, y + dy)`). `(x, y)` is the candidate
/// position (`new_x`/`new_y` in C++, i.e. `x + vel_x` at the first call).
pub fn calculate_reaction_force(
    level: &LevelSim,
    x: i32,
    y: i32,
    dir: usize,
    reacts: &mut [i32; 4],
) {
    reacts[dir] = 0;

    for pt in COL_POINTS[dir].iter().take(COL_POINT_COUNT[dir] as usize) {
        let col_x = x + pt.x;
        let col_y = y + pt.y;

        if !level.checked_mat_background(col_x, col_y) {
            reacts[dir] += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// PhysicsConsts — the TC constants/hacks the physics reads
// ---------------------------------------------------------------------------

/// The small set of `TcConfig` constants + hacks `ProcessPhysics` and the
/// reaction orchestration read (design doc, *`PhysicsConsts`*). Built once from a
/// loaded `TcConfig` and carried on [`SimState`](crate::state::SimState) so the
/// driver signature stays `(state, inputs)`. Not hashed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PhysicsConsts {
    /// `WormGravity` — added to `vel.y` each tick the worm is airborne.
    pub worm_gravity: i32,
    /// `WormFricMult` / `WormFricDiv` — horizontal friction numerator/denominator.
    pub worm_fric_mult: i32,
    pub worm_fric_div: i32,
    /// `MinBounce{Up,Down,Left,Right}` — the bounce-vs-stop thresholds.
    pub min_bounce_up: i32,
    pub min_bounce_down: i32,
    pub min_bounce_left: i32,
    pub min_bounce_right: i32,
    /// `FallDamageRight` / `FallDamageDown` — health loss on a hard bounce, only
    /// applied when the `FallDamage` hack is on.
    pub fall_damage_right: i32,
    pub fall_damage_down: i32,
    /// `WormFloatLevel` / `WormFloatPower` — the `WormFloat`-hack anti-gravity.
    pub worm_float_level: i32,
    pub worm_float_power: i32,
    /// Hack `FallDamage` — when set, hard bounces subtract `FallDamage*` health.
    pub h_fall_damage: bool,
    /// Hack `WormFloat` — when set, the low-`y` edge applies float instead of
    /// `reacts[kRfDown] += 5` for high `y` (and replaces the ceiling reaction).
    pub h_worm_float: bool,
}

impl Default for PhysicsConsts {
    /// The `data/TC/openliero` values (design doc table). Used by unit tests and
    /// as a sane default; the differential pipeline builds from the real TC via
    /// [`PhysicsConsts::from_tc`].
    fn default() -> Self {
        PhysicsConsts {
            worm_gravity: 1500,
            worm_fric_mult: 89,
            worm_fric_div: 100,
            min_bounce_up: -53248,
            min_bounce_down: 53248,
            min_bounce_left: -53248,
            min_bounce_right: 53248,
            fall_damage_right: 0,
            fall_damage_down: 0,
            worm_float_level: 163,
            worm_float_power: -8386178,
            h_fall_damage: false,
            h_worm_float: false,
        }
    }
}

impl PhysicsConsts {
    /// Build from a loaded `TcConfig` (`assets::tc`), pulling `[constants]` and
    /// `[hacks]` — the exact `LC(...)` / `common.h[...]` values C++ reads.
    pub fn from_tc(tc: &assets::tc::TcConfig) -> Self {
        let c = &tc.constants;
        let h = &tc.hacks;
        PhysicsConsts {
            worm_gravity: c.WormGravity,
            worm_fric_mult: c.WormFricMult,
            worm_fric_div: c.WormFricDiv,
            min_bounce_up: c.MinBounceUp,
            min_bounce_down: c.MinBounceDown,
            min_bounce_left: c.MinBounceLeft,
            min_bounce_right: c.MinBounceRight,
            fall_damage_right: c.FallDamageRight,
            fall_damage_down: c.FallDamageDown,
            worm_float_level: c.WormFloatLevel,
            worm_float_power: c.WormFloatPower,
            h_fall_damage: h.FallDamage,
            h_worm_float: h.WormFloat,
        }
    }
}

// ---------------------------------------------------------------------------
// Reaction orchestration (Worm::Process, worm.cpp:221-283)
// ---------------------------------------------------------------------------

/// Port of the reaction-force orchestration block inside `Worm::Process`
/// (`worm.cpp:221-283`).
///
/// Computes `next = pos + vel`, `i_next = Ftoi(next)`, then runs
/// [`calculate_reaction_force`] for each of the four directions, applying the
/// per-iteration level-edge additions **every iteration** (matching the C++
/// "Yes, Liero does this in every iteration. Keep it this way." comment — note
/// each `CalculateReactionForce(i)` resets `reacts[i]=0`, so an edge add to a
/// direction whose probe runs later in the loop is partially wiped). Handles the
/// `WormFloat` branch, then the two `pos.y ± Itof(1)` nudge corrections that
/// re-probe left/right.
///
/// Mutates the worm's `pos.y` (nudge corrections) and `vel.y` (`WormFloat`),
/// exactly as the C++ does, and returns the tick-local `reacts` for
/// [`worm_process_physics`].
pub fn worm_reactions(level: &LevelSim, worm: &mut WormState, c: &PhysicsConsts) -> [i32; 4] {
    let mut reacts = [0i32; 4];

    // next = pos + vel; i_next = Ftoi(next)  (integer pixel space)
    let mut next = worm.pos.add(worm.vel);
    let i_next_x = ftoi(next.x);
    let mut i_next_y = ftoi(next.y);

    for i in 0..4usize {
        calculate_reaction_force(level, i_next_x, i_next_y, i, &mut reacts);

        // Edge additions — applied every iteration (worm.cpp:231-247).
        if i_next_x < 4 {
            reacts[RF_RIGHT] += 5;
        } else if i_next_x > level.width - 5 {
            reacts[RF_LEFT] += 5;
        }

        if i_next_y < 5 {
            reacts[RF_DOWN] += 5;
        } else if c.h_worm_float {
            if i_next_y > c.worm_float_level {
                worm.vel.y = worm.vel.y.wrapping_sub(c.worm_float_power);
            }
        } else if i_next_y > level.height - 6 {
            reacts[RF_UP] += 5;
        }
    }

    // Nudge correction A (worm.cpp:250-265): pushed up, low/no push down,
    // and pushed left or right -> nudge pos.y up one pixel and re-probe L/R.
    if reacts[RF_DOWN] < 2 && reacts[RF_UP] > 0 && (reacts[RF_LEFT] > 0 || reacts[RF_RIGHT] > 0) {
        worm.pos.y = worm.pos.y.wrapping_sub(itof(1));
        next.y = worm.pos.y.wrapping_add(worm.vel.y);
        i_next_y = ftoi(next.y);
        calculate_reaction_force(level, i_next_x, i_next_y, RF_LEFT, &mut reacts);
        calculate_reaction_force(level, i_next_x, i_next_y, RF_RIGHT, &mut reacts);
    }

    // Nudge correction B (worm.cpp:267-282): pushed down, low/no push up,
    // and pushed left or right -> nudge pos.y down one pixel and re-probe L/R.
    if reacts[RF_UP] < 2 && reacts[RF_DOWN] > 0 && (reacts[RF_LEFT] > 0 || reacts[RF_RIGHT] > 0) {
        worm.pos.y = worm.pos.y.wrapping_add(itof(1));
        next.y = worm.pos.y.wrapping_add(worm.vel.y);
        i_next_y = ftoi(next.y);
        calculate_reaction_force(level, i_next_x, i_next_y, RF_LEFT, &mut reacts);
        calculate_reaction_force(level, i_next_x, i_next_y, RF_RIGHT, &mut reacts);
    }

    reacts
}

// ---------------------------------------------------------------------------
// ProcessPhysics (worm.cpp:149-208)
// ---------------------------------------------------------------------------

/// Port of `Worm::ProcessPhysics` (`worm.cpp:149-208`): horizontal friction when
/// grounded, the horizontal then vertical bounce-or-stop, gravity, and the
/// guarded position integration. `reacts` is the value from [`worm_reactions`].
///
/// All arithmetic is `wrapping_*` / truncating `/` to match C++ `int` semantics
/// bit-for-bit (friction `*mult/div` truncates toward zero; bounce `-vel/3`
/// truncates toward zero; `abs`/`neg` guard `i32::MIN`).
pub fn worm_process_physics(worm: &mut WormState, reacts: &[i32; 4], c: &PhysicsConsts) {
    // Horizontal friction when grounded (reacts[kRfUp] > 0).
    if reacts[RF_UP] > 0 {
        worm.vel.x = worm
            .vel
            .x
            .wrapping_mul(c.worm_fric_mult)
            .wrapping_div(c.worm_fric_div);
    }

    // kAbsvel(std::abs(vel.x), std::abs(vel.y))
    let abs_x = worm.vel.x.wrapping_abs();
    let abs_y = worm.vel.y.wrapping_abs();

    let rh = reacts[if worm.vel.x >= 0 { RF_LEFT } else { RF_RIGHT }];
    let rv = reacts[if worm.vel.y >= 0 { RF_UP } else { RF_DOWN }];
    let mbh = if worm.vel.x > 0 {
        c.min_bounce_right
    } else {
        c.min_bounce_left.wrapping_neg()
    };
    let mbv = if worm.vel.y > 0 {
        c.min_bounce_down
    } else {
        c.min_bounce_up.wrapping_neg()
    };

    // Horizontal bounce-or-stop.
    if worm.vel.x != 0 && rh != 0 {
        if abs_x > mbh {
            if c.h_fall_damage {
                worm.health = worm.health.wrapping_sub(c.fall_damage_right);
            }
            worm.vel.x = worm.vel.x.wrapping_neg().wrapping_div(3);
        } else {
            worm.vel.x = 0;
        }
    }

    // Vertical bounce-or-stop.
    if worm.vel.y != 0 && rv != 0 {
        if abs_y > mbv {
            if c.h_fall_damage {
                worm.health = worm.health.wrapping_sub(c.fall_damage_down);
            }
            worm.vel.y = worm.vel.y.wrapping_neg().wrapping_div(3);
        } else {
            worm.vel.y = 0;
        }
    }

    // Gravity (only while there is no upward reaction).
    if reacts[RF_UP] == 0 {
        worm.vel.y = worm.vel.y.wrapping_add(c.worm_gravity);
    }

    // Guarded integration — re-reads reacts (out of date rh/rv not reused),
    // with the >= 0 sign test selecting the reaction index.
    if reacts[if worm.vel.x >= 0 { RF_LEFT } else { RF_RIGHT }] < 2 {
        worm.pos.x = worm.pos.x.wrapping_add(worm.vel.x);
    }
    if reacts[if worm.vel.y >= 0 { RF_UP } else { RF_DOWN }] < 2 {
        worm.pos.y = worm.pos.y.wrapping_add(worm.vel.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MAT_BACKGROUND;

    const ALL_DIRS: [usize; 4] = [RF_DOWN, RF_LEFT, RF_UP, RF_RIGHT];

    /// Build a `width x height` level whose every pixel carries `mat`, with a
    /// flag table where material `1` is background-flagged and material `2` is
    /// solid (no background bit). Material `0` is left flagless too, so any
    /// out-of-range probe (which reads flag-table entry 0) counts as *solid*.
    fn uniform_level(width: i32, height: i32, mat: u8) -> LevelSim {
        let mut material_flags = [0u8; 256];
        // 0 -> no background bit (solid); used by the OOB fallback.
        material_flags[1] = MAT_BACKGROUND;
        material_flags[2] = 1 << 2; // solid (e.g. rock), no background bit
        LevelSim {
            width,
            height,
            material_id: vec![mat; (width * height) as usize],
            material_flags,
        }
    }

    // ---- all background -> zero reaction for every direction ----------------

    #[test]
    fn all_background_neighbourhood_reacts_zero() {
        // Large enough that every probe around the centre stays in-range and
        // reads material 1 (background).
        let level = uniform_level(64, 64, 1);
        let (x, y) = (32, 32);
        for &dir in &ALL_DIRS {
            let mut reacts = [9; 4];
            calculate_reaction_force(&level, x, y, dir, &mut reacts);
            assert_eq!(reacts[dir], 0, "dir {dir}: all-background -> 0");
        }
    }

    // ---- fully solid -> full count for every direction ----------------------

    #[test]
    fn fully_solid_neighbourhood_reacts_full_count() {
        // Every pixel is material 2 (solid). Each live probe point is a hit.
        let level = uniform_level(64, 64, 2);
        let (x, y) = (32, 32);
        for &dir in &ALL_DIRS {
            let mut reacts = [0; 4];
            calculate_reaction_force(&level, x, y, dir, &mut reacts);
            assert_eq!(
                reacts[dir], COL_POINT_COUNT[dir],
                "dir {dir}: all-solid -> kColPointCount[dir]"
            );
        }
        // Spell out the expected counts so the table sizes are pinned.
        let mut r = [0; 4];
        calculate_reaction_force(&level, x, y, RF_DOWN, &mut r);
        assert_eq!(r[RF_DOWN], 3);
        calculate_reaction_force(&level, x, y, RF_UP, &mut r);
        assert_eq!(r[RF_UP], 3);
        calculate_reaction_force(&level, x, y, RF_LEFT, &mut r);
        assert_eq!(r[RF_LEFT], 7);
        calculate_reaction_force(&level, x, y, RF_RIGHT, &mut r);
        assert_eq!(r[RF_RIGHT], 7);
    }

    /// A mostly-background level on which we can place solid pixels at exact
    /// `(x, y)` cells to drive a known probe count.
    fn background_level(width: i32, height: i32) -> LevelSim {
        // All background (mat 1); the caller pokes solid (mat 2) cells in.
        uniform_level(width, height, 1)
    }

    fn set_solid(level: &mut LevelSim, x: i32, y: i32) {
        let idx = (x + y * level.width) as usize;
        level.material_id[idx] = 2; // solid
    }

    // ---- partial pattern -> exact count, verifying the offsets & dir index --

    #[test]
    fn partial_pattern_down_counts_exact_probe_offsets() {
        // DOWN probes (relative to centre): (-1,-4), (0,-4), (1,-4).
        let (cx, cy) = (32, 32);
        let mut level = background_level(64, 64);
        // Make two of the three DOWN probe cells solid; leave the third (0,-4)
        // background. Also drop a solid pixel at a NON-probe cell to prove only
        // the listed offsets are sampled.
        set_solid(&mut level, cx - 1, cy - 4); // probe 0 -> hit
        set_solid(&mut level, cx + 1, cy - 4); // probe 2 -> hit
        set_solid(&mut level, cx, cy); // not a DOWN probe cell -> ignored
        let mut reacts = [0; 4];
        calculate_reaction_force(&level, cx, cy, RF_DOWN, &mut reacts);
        assert_eq!(reacts[RF_DOWN], 2, "two of three DOWN probes are solid");
    }

    #[test]
    fn partial_pattern_left_counts_exact_probe_offsets() {
        // LEFT probes: x+1, y in -3..=3 (seven cells along the right edge).
        let (cx, cy) = (32, 32);
        let mut level = background_level(64, 64);
        // Solidify y-offsets -3, 0, +3 -> 3 hits; the column is x+1.
        set_solid(&mut level, cx + 1, cy - 3);
        set_solid(&mut level, cx + 1, cy);
        set_solid(&mut level, cx + 1, cy + 3);
        // Decoys on the wrong column (x-1) must be ignored by LEFT.
        set_solid(&mut level, cx - 1, cy - 3);
        set_solid(&mut level, cx - 1, cy + 1);
        let mut reacts = [0; 4];
        calculate_reaction_force(&level, cx, cy, RF_LEFT, &mut reacts);
        assert_eq!(reacts[RF_LEFT], 3, "three of seven LEFT probes are solid");
    }

    #[test]
    fn partial_pattern_right_uses_opposite_column_from_left() {
        // RIGHT probes the x-1 column; this pins the dir indexing against LEFT.
        let (cx, cy) = (32, 32);
        let mut level = background_level(64, 64);
        // Solid along x-1 for y offsets -2, -1, 0, 1 -> 4 hits for RIGHT.
        set_solid(&mut level, cx - 1, cy - 2);
        set_solid(&mut level, cx - 1, cy - 1);
        set_solid(&mut level, cx - 1, cy);
        set_solid(&mut level, cx - 1, cy + 1);
        // Solid along x+1 (LEFT's column) must NOT count for RIGHT.
        set_solid(&mut level, cx + 1, cy - 3);
        set_solid(&mut level, cx + 1, cy + 3);
        let mut reacts = [0; 4];
        calculate_reaction_force(&level, cx, cy, RF_RIGHT, &mut reacts);
        assert_eq!(reacts[RF_RIGHT], 4, "four of seven RIGHT probes are solid");
        // And LEFT on the same level sees its own column: x+1 at -3 and +3 -> 2.
        calculate_reaction_force(&level, cx, cy, RF_LEFT, &mut reacts);
        assert_eq!(reacts[RF_LEFT], 2, "LEFT reads x+1 column independently");
    }

    #[test]
    fn partial_pattern_up_probes_positive_y_offsets() {
        // UP probes (-1,4), (0,4), (1,4): y is +4 (below centre).
        let (cx, cy) = (32, 32);
        let mut level = background_level(64, 64);
        set_solid(&mut level, cx, cy + 4); // probe 1 -> hit
                                           // A solid at y-4 (the DOWN row) must not count for UP.
        set_solid(&mut level, cx, cy - 4);
        let mut reacts = [0; 4];
        calculate_reaction_force(&level, cx, cy, RF_UP, &mut reacts);
        assert_eq!(reacts[RF_UP], 1, "only the (0,+4) UP probe is solid");
    }

    #[test]
    fn resets_reacts_dir_each_call_and_leaves_others_untouched() {
        let level = uniform_level(64, 64, 1); // all background
        let mut reacts = [5, 6, 7, 8];
        calculate_reaction_force(&level, 32, 32, RF_LEFT, &mut reacts);
        assert_eq!(reacts[RF_LEFT], 0, "dir reset to 0 then accumulated");
        assert_eq!(
            [reacts[RF_DOWN], reacts[RF_UP], reacts[RF_RIGHT]],
            [5, 7, 8],
            "other directions are untouched"
        );
    }

    // ===================================================================
    // PhysicsConsts / process_physics / reaction orchestration
    // ===================================================================

    use sim_core::vec::Vec2;

    /// A worm at `pos` with `vel`, visible, health 100 — the only fields the
    /// physics reads. Other fields are tick-0 defaults.
    fn worm_at(pos: Vec2, vel: Vec2) -> WormState {
        use crate::state::{Ninjarope, WormWeapon, NUM_WEAPONS};
        WormState {
            pos,
            vel,
            aiming_angle: 0,
            health: 100,
            lives: 10,
            kills: 0,
            timer: 0,
            visible: true,
            killed_timer: 150,
            control_states: crate::state::ControlState::new(),
            weapons: [WormWeapon::default(); NUM_WEAPONS],
            ninjarope: Ninjarope::default(),
            index: 0,
            stats_x: 0,
            // Slice-3 control fields are not read by the physics pass; tick-0
            // defaults.
            aiming_speed: 0,
            direction: 0,
            movable: true,
            able_to_jump: false,
            able_to_dig: false,
            key_change_pressed: false,
            current_weapon: 0,
            fire_cone: 0,
            leave_shell_timer: 0,
        }
    }

    /// A wide level where EVERY probe (in range or OOB) reads background, so
    /// `calculate_reaction_force` always returns 0 and the only `reacts`
    /// contributions come from the edge additions. Material 1 = background, and
    /// flag-table entry 0 (the OOB fallback) is also background.
    fn all_background_level(width: i32, height: i32) -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[0] = MAT_BACKGROUND; // OOB fallback -> background
        material_flags[1] = MAT_BACKGROUND;
        LevelSim {
            width,
            height,
            material_id: vec![1u8; (width * height) as usize],
            material_flags,
        }
    }

    // ---- Free-fall: gravity each tick, guarded integration -------------------

    #[test]
    fn free_fall_gravity_and_integration_hand_folded() {
        // Mid-air, away from every edge so reacts stay [0,0,0,0] each tick.
        let level = all_background_level(200, 200);
        let c = PhysicsConsts::default();
        let start = itof(100); // 6553600; i_next = 100 (safe: 4..=194 / 5..=194)
        let mut w = worm_at(Vec2::new(start, start), Vec2::zero());

        // Tick 1: vel.y += 1500 -> 1500; pos.y += 1500.
        let r = worm_reactions(&level, &mut w, &c);
        assert_eq!(r, [0, 0, 0, 0], "mid-air -> no reactions");
        worm_process_physics(&mut w, &r, &c);
        assert_eq!(w.vel.y, 1500);
        assert_eq!(w.pos.y, start + 1500);
        assert_eq!(w.vel.x, 0);
        assert_eq!(w.pos.x, start, "no horizontal motion");

        // Tick 2: vel.y -> 3000; pos.y += 3000.
        let r = worm_reactions(&level, &mut w, &c);
        assert_eq!(r, [0, 0, 0, 0]);
        worm_process_physics(&mut w, &r, &c);
        assert_eq!(w.vel.y, 3000);
        assert_eq!(w.pos.y, start + 1500 + 3000);

        // Tick 3: vel.y -> 4500; pos.y += 4500. Closed form:
        // pos.y = start + 1500 * k(k+1)/2 = start + 1500*6 = start + 9000.
        let r = worm_reactions(&level, &mut w, &c);
        worm_process_physics(&mut w, &r, &c);
        assert_eq!(w.vel.y, 4500);
        assert_eq!(w.pos.y, start + 9000, "sum of 1500,3000,4500");
    }

    // ---- Bounce: vertical sign flip + /3 truncation, integration suppressed --

    #[test]
    fn vertical_bounce_flips_sign_truncates_and_suppresses_integration() {
        // reacts = [down=2, left=0, up=3, right=0]: an upward reaction (rv for a
        // downward vel reads kRfUp=3), and down=2 so the post-flip integration
        // (vel.y<0 -> reads kRfDown=2) is suppressed (2 < 2 is false).
        let c = PhysicsConsts::default();
        let reacts = [2, 0, 3, 0];
        let pos0 = itof(50);
        let mut w = worm_at(Vec2::new(pos0, pos0), Vec2::new(0, 200000));

        worm_process_physics(&mut w, &reacts, &c);
        // abs(200000) > MinBounceDown(53248) -> vel.y = -200000/3 (toward zero).
        assert_eq!(w.vel.y, -66666, "sign flip + /3 truncation toward zero");
        // gravity skipped (reacts[up]=3 != 0); integration suppressed (down=2).
        assert_eq!(w.pos.y, pos0, "integration suppressed by reacts[down] >= 2");
    }

    // ---- Stop: slow downward velocity zeroed --------------------------------

    #[test]
    fn vertical_slow_velocity_stops() {
        let c = PhysicsConsts::default();
        let reacts = [2, 0, 3, 0]; // up reaction present, down=2 suppresses move
        let pos0 = itof(50);
        // abs(40000) <= MinBounceDown(53248) -> stop.
        let mut w = worm_at(Vec2::new(pos0, pos0), Vec2::new(0, 40000));
        worm_process_physics(&mut w, &reacts, &c);
        assert_eq!(w.vel.y, 0, "slow velocity stops dead");
        assert_eq!(w.pos.y, pos0, "no integration after stop (vel.y==0)");
    }

    // ---- Friction: negative vel.x truncates toward zero ---------------------

    #[test]
    fn horizontal_friction_truncates_toward_zero_on_negative() {
        // Grounded (reacts[kRfUp] > 0). vel.x = -1234 -> -1234*89/100.
        // -109826 / 100 = -1098 (toward zero); a shift/floor would give -1099.
        let c = PhysicsConsts::default();
        let reacts = [0, 0, 1, 0]; // up=1 grounds the worm; right=0
        let pos0 = itof(50);
        let mut w = worm_at(Vec2::new(pos0, pos0), Vec2::new(-1234, 0));
        worm_process_physics(&mut w, &reacts, &c);
        assert_eq!(w.vel.x, -1098, "friction truncates toward zero, not floor");
        // No horizontal bounce (rh = reacts[right] = 0), so pos.x integrates.
        assert_eq!(w.pos.x, pos0 - 1098);
    }

    // ---- Edge additions: accumulate every iteration (with the reset quirk) ---

    #[test]
    fn edge_addition_low_y_accumulates_four_times() {
        // i_next.y < 5 adds to kRfDown (index 0), whose probe runs FIRST (i=0),
        // so all four iterations' +5 survive -> 20.
        let level = all_background_level(200, 200);
        let c = PhysicsConsts::default();
        let mut w = worm_at(Vec2::new(itof(100), itof(2)), Vec2::zero());
        let r = worm_reactions(&level, &mut w, &c);
        assert_eq!(
            r,
            [20, 0, 0, 0],
            "low-y edge add accumulates 4x into kRfDown"
        );
    }

    #[test]
    fn edge_addition_low_x_partly_wiped_by_later_reset() {
        // i_next.x < 4 adds to kRfRight (index 3), whose probe runs LAST (i=3) and
        // resets reacts[3]=0 just before that iteration's +5 -> only 5 survives.
        let level = all_background_level(200, 200);
        let c = PhysicsConsts::default();
        let mut w = worm_at(Vec2::new(itof(2), itof(100)), Vec2::zero());
        let r = worm_reactions(&level, &mut w, &c);
        assert_eq!(
            r,
            [0, 0, 0, 5],
            "low-x edge add to kRfRight is reset at i=3, only last +5 survives"
        );
    }

    #[test]
    fn edge_addition_high_x_accumulates_into_left() {
        // i_next.x > width-5 adds to kRfLeft (index 1), probe runs at i=1: the
        // i=0 add is wiped, i=1..3 survive -> 15.
        let level = all_background_level(200, 200);
        let c = PhysicsConsts::default();
        let mut w = worm_at(Vec2::new(itof(198), itof(100)), Vec2::zero());
        let r = worm_reactions(&level, &mut w, &c);
        assert_eq!(r, [0, 15, 0, 0], "high-x edge add: i=1..3 survive -> 15");
    }
}
