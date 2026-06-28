//! Worm physics ports.
//!
//! Slice 2 begins with the collision-reaction probe, a direct port of
//! `Worm::CalculateReactionForce` (`src/game/worm.cpp:97-147`). It samples a
//! small fixed pattern of pixels around a candidate `(x, y)` for one of four
//! directions and counts how many land on a *non-background* (solid) material —
//! the per-direction "reaction" the integrator later uses to bounce/stop the
//! worm. The probe pattern and per-direction point counts are copied verbatim
//! from the C++ table so the counts match bit-for-bit.

use crate::state::LevelSim;

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
}
