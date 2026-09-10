//! Random level generation — port of C++ `Level::GenerateRandom` and friends
//! (`src/game/level.cpp:11-193`, `:397-429`) plus `BlitStone` (`gfx/blit.cpp:462-532`).
//! Step 4½, slice 4½b.
//!
//! Bit-exact vs C++ for the same `Rand` state, TC assets and dimensions — gated by
//! `oracle-tests/tests/levelgen_golden.rs` against `oracle_dump_levelgen`, which hashes
//! `material_id` and `rand.last` after every stage. Integer-only; no I/O; no `SimState`.
//!
//! * **The generator never constructs a `Rand`.** Every entry point takes `&mut Rand`; the
//!   game shell seeds a dedicated one from the match seed (overview locked decision 6,
//!   design §2), the oracle drives it from any seeded state.
//! * **Flags are read live** from `material_flags[material_id[i]]` — equivalent to the C++
//!   `materials[]` cache because every generation writer keeps that cache in sync and the
//!   field writes every cell before any flag is read (design F6).
//! * **C++ `rand()` returns `u32`**: `rand(n) - 8`, `height - 1 - rand(20)` wrap and convert
//!   back to `int`, which is exactly `rand.bound(n) as i32 - 8` here (`bound < n <= 4096`,
//!   design F7). No C++ expression here holds two `rand()` calls, so there is no
//!   unspecified evaluation order to reproduce.

use sim_core::rng::Rand;

use crate::state::LevelSim;

/// Largest level side C++ accepts (`level.cpp:232`, `assets/src/level.rs:51`).
const MAX_DIM: i32 = 4096;

/// `Level::Resize` (`level.cpp:218-227`) on a fresh level: a zero-filled `width × height`
/// material map carrying the TC flag table. Precondition `1 <= width, height <= 4096`
/// (the menu range is 64..=4096 step 8, `gfx.cpp:1295-1297`, but a hand-edited setup file
/// can hold any value).
pub fn new_level(width: i32, height: i32, material_flags: &[u8; 256]) -> LevelSim {
    debug_assert!(
        (1..=MAX_DIM).contains(&width) && (1..=MAX_DIM).contains(&height),
        "level size {width}x{height} outside 1..=4096"
    );
    LevelSim {
        width,
        height,
        material_id: vec![0u8; (width * height) as usize],
        material_flags: *material_flags,
    }
}

/// The diffusion noise field — first block of `GenerateDirtPattern` (`level.cpp:12-26`).
/// Draws exactly `width * height` values: the corner `rand(7)+12`; the first COLUMN, each
/// `(rand(7)+12 + above) >> 1`; the first ROW, each `(rand(7)+12 + left) >> 1`; the
/// interior row-major, each `(left + up + rand(8)+12) / 3`. Values stay in 12..=18 (the
/// tc.cfg dirt shades).
pub fn generate_dirt_field(level: &mut LevelSim, rand: &mut Rand) {
    let w = level.width;
    let h = level.height;
    let at = |x: i32, y: i32| (x + y * w) as usize;
    let corner = rand.bound(7) + 12;
    level.set_material(0, corner as u8);
    for y in 1..h {
        let up = level.material_id[at(0, y - 1)] as u32;
        let v = (rand.bound(7) + 12 + up) >> 1;
        level.set_material(at(0, y), v as u8);
    }
    for x in 1..w {
        let left = level.material_id[at(x - 1, 0)] as u32;
        let v = (rand.bound(7) + 12 + left) >> 1;
        level.set_material(at(x, 0), v as u8);
    }
    for y in 1..h {
        for x in 1..w {
            let left = level.material_id[at(x - 1, y)] as u32;
            let up = level.material_id[at(x, y - 1)] as u32;
            let v = (left + up + rand.bound(8) + 12) / 3;
            level.set_material(at(x, y), v as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MAT_ROCK;

    fn seeded(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    #[test]
    fn new_level_is_zero_filled_with_dims_and_flags() {
        let mut flags = [0u8; 256];
        flags[7] = MAT_ROCK;
        let level = new_level(5, 3, &flags);
        assert_eq!((level.width, level.height), (5, 3));
        assert_eq!(level.material_id, vec![0u8; 15]);
        assert_eq!(level.material_flags, flags);
    }

    /// level.cpp:12-26 restated statement by statement with an identically seeded
    /// reference `Rand`: corner, then COLUMN x=0 (y=1..h), then ROW y=0 (x=1..w), then the
    /// interior row-major. Swapping the column and row loops shifts every later draw.
    #[test]
    fn dirt_field_follows_level_cpp_12_26_draw_order() {
        let (w, h) = (9i32, 7i32);
        let mut r = seeded(7);
        let mut want = vec![0u8; (w * h) as usize];
        let at = |x: i32, y: i32| (x + y * w) as usize;
        want[0] = (r.bound(7) + 12) as u8;
        for y in 1..h {
            want[at(0, y)] = ((r.bound(7) + 12 + want[at(0, y - 1)] as u32) >> 1) as u8;
        }
        for x in 1..w {
            want[at(x, 0)] = ((r.bound(7) + 12 + want[at(x - 1, 0)] as u32) >> 1) as u8;
        }
        for y in 1..h {
            for x in 1..w {
                let left = want[at(x - 1, y)] as u32;
                let up = want[at(x, y - 1)] as u32;
                want[at(x, y)] = ((left + up + r.bound(8) + 12) / 3) as u8;
            }
        }

        let mut level = new_level(w, h, &[0u8; 256]);
        let mut rand = seeded(7);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(level.material_id, want);
        assert_eq!(rand.last(), r.last());
        assert_eq!(rand.draws(), r.draws());
    }

    #[test]
    fn dirt_field_draws_w_times_h_and_stays_in_12_to_18() {
        let mut level = new_level(17, 9, &[0u8; 256]);
        let mut rand = seeded(42);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(rand.draws(), 17 * 9);
        assert!(level.material_id.iter().all(|&p| (12..=18).contains(&p)));
    }
}
