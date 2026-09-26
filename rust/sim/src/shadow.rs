//! Port of `CorrectShadow` (`gfx/blit.cpp:624-639`) and its per-tick enable flag
//! (Step 4½a-1, design §5.1).
//!
//! C++ runs `CorrectShadow` after every in-frame crater when `settings->shadow` is on
//! (default ON, `settings.hpp:74`): `nobject.cpp:123,215`, `sobject.cpp:212`,
//! `weapon.cpp:121`, `worm.cpp:784,932,942`. It rewrites `material_id`, which the
//! level hash reads. `SimState.shadow` carries the setting; `process_frame` publishes
//! it here at the top of every tick ([`begin_frame`]) and clears it at the bottom
//! ([`end_frame`]), so inside a tick the flag always equals `self.shadow` and never
//! leaks between states or threads. A thread-local instead of a parameter because
//! `sobject_create` (one of the sites) has a wide fan-in — the precedent is
//! `sound::begin_frame`'s hook indices. `MakeShadow` (level preparation) is 4½b's
//! `sim::levelgen::make_shadow`, not here.

use std::cell::Cell;

use crate::state::{LevelSim, MAT_SEE_SHADOW};

thread_local! {
    /// This thread's `settings->shadow` for the current tick. Off outside a tick.
    static ENABLED: Cell<bool> = const { Cell::new(false) };
}

/// Open a tick: publish `SimState.shadow`. Called at the top of `process_frame`.
pub fn begin_frame(enabled: bool) {
    ENABLED.with(|e| e.set(enabled));
}

/// Close a tick: the flag is off again. Called at the bottom of `process_frame`.
pub fn end_frame() {
    ENABLED.with(|e| e.set(false));
}

/// The current tick's flag (tests and diagnostics).
pub fn enabled() -> bool {
    ENABLED.with(|e| e.get())
}

/// `CorrectShadow(common, level, Rect(x1, y1, x2, y2))` — always applied. The rect is
/// intersected with `Rect(0, 3, width - 3, height)` (`blit.cpp:625`) so the
/// `(x + 3, y - 3)` probe is always in range; x-outer / y-inner like C++ (the probe's
/// column is never visited before it is read). `SeeShadow` under `DirtRock` ⇒ `+4`;
/// else `164..=167` not under `DirtRock` ⇒ `-4`. `PalIdx` arithmetic wraps.
pub fn correct_shadow(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32) {
    let x1 = x1.max(0);
    let y1 = y1.max(3);
    let x2 = x2.min(level.width - 3);
    let y2 = y2.min(level.height);
    for x in x1..x2 {
        for y in y1..y2 {
            let idx = (x + y * level.width) as usize;
            let pix = level.material_id[idx];
            let see_shadow = level.material_flags[pix as usize] & MAT_SEE_SHADOW != 0;
            let shaded = level.dirt_rock(x + 3, y - 3);
            if see_shadow && shaded {
                level.set_material(idx, pix.wrapping_add(4));
            } else if (164..=167).contains(&pix) && !shaded {
                level.set_material(idx, pix.wrapping_sub(4));
            }
        }
    }
}

/// [`correct_shadow`] iff the current tick's `settings->shadow` flag is on — the form
/// the seven sim call sites use.
pub fn correct_shadow_if_enabled(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32) {
    if enabled() {
        correct_shadow(level, x1, y1, x2, y2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{LevelSim, MAT_ROCK, MAT_SEE_SHADOW};

    const SEE: u8 = 10; // a SeeShadow material
    const ROCK: u8 = 20; // a Rock material (DirtRock)
    const WRAP: u8 = 254; // SeeShadow near the top of the palette: +4 wraps

    fn lvl(w: i32, h: i32) -> LevelSim {
        let mut flags = [0u8; 256];
        flags[SEE as usize] = MAT_SEE_SHADOW;
        flags[WRAP as usize] = MAT_SEE_SHADOW;
        flags[ROCK as usize] = MAT_ROCK;
        LevelSim {
            width: w,
            height: h,
            material_id: vec![0; (w * h) as usize],
            material_flags: flags,
        }
    }

    fn at(l: &LevelSim, x: i32, y: i32) -> u8 {
        l.material_id[(x + y * l.width) as usize]
    }

    fn put(l: &mut LevelSim, x: i32, y: i32, v: u8) {
        let w = l.width;
        l.material_id[(x + y * w) as usize] = v;
    }

    #[test]
    fn see_shadow_pixel_under_rock_darkens_by_four() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, SEE);
        put(&mut l, 5, 2, ROCK); // (x+3, y-3)
        correct_shadow(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), SEE + 4);
    }

    #[test]
    fn shadow_pixel_without_rock_lightens_by_four_and_with_rock_stays() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, 165);
        put(&mut l, 3, 6, 166);
        put(&mut l, 6, 3, ROCK); // shadows (3,6) only
        correct_shadow(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 161, "164..=167 not under DirtRock => -4");
        assert_eq!(at(&l, 3, 6), 166, "still under rock => unchanged");
    }

    #[test]
    fn the_rect_is_clipped_to_0_3_w_minus_3_h() {
        // Pixels whose (x+3, y-3) probe would leave the level are never visited.
        let mut l = lvl(10, 10);
        put(&mut l, 8, 5, 165); // x >= w-3
        put(&mut l, 2, 1, 165); // y < 3
        put(&mut l, 2, 5, 165); // inside
        correct_shadow(&mut l, -20, -20, 50, 50);
        assert_eq!(at(&l, 8, 5), 165);
        assert_eq!(at(&l, 2, 1), 165);
        assert_eq!(at(&l, 2, 5), 161);
    }

    #[test]
    fn the_caller_rect_limits_the_pass() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, 165);
        correct_shadow(&mut l, 3, 3, 10, 10); // x starts at 3
        assert_eq!(at(&l, 2, 5), 165);
    }

    #[test]
    fn palidx_arithmetic_wraps_like_cpp() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, WRAP);
        put(&mut l, 5, 2, ROCK);
        correct_shadow(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 2, "254 + 4 wraps to 2 (C++ PalIdx)");
    }

    #[test]
    fn the_if_enabled_variant_follows_the_frame_flag() {
        end_frame();
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, 165);
        correct_shadow_if_enabled(&mut l, 0, 0, 10, 10);
        assert_eq!(
            at(&l, 2, 5),
            165,
            "outside a frame the flag is off (today's behaviour)"
        );
        begin_frame(true);
        assert!(enabled());
        correct_shadow_if_enabled(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 161);
        end_frame();
        assert!(!enabled());
    }
}
