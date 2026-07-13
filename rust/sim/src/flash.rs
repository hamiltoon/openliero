//! The per-tick **`screen_flash` accumulator** (Slice 4d T0).
//!
//! `screen_flash` is a single `i32` scalar on [`SimState`](crate::state::SimState)
//! that persists across ticks: C++ `Game::ProcessFrame` decrements it at the top
//! (`game.cpp:271-273`) and `SObjectType::Create` raises it to `max(type.flash,
//! screen_flash)` when an explosion sobject is spawned (`sobject.cpp:41`). It drives
//! the render palette `LightUp` (`game.cpp:179-181`) and is **never hashed** — C++
//! `HashGameState` omits it (`stateHash.hpp:15-113`), so folding it into
//! [`hash_game_state`](crate::hash::hash_game_state) is forbidden; the whole
//! "no re-fuzz" property of Slice 4d rests on that (design §0, §9 risk 4).
//!
//! # Transport: a per-tick thread-local, seeded and folded back by `process_frame`
//!
//! The raise (`sobject.cpp:41`) fires deep in the object/worm call tree —
//! `SObjectType::Create` is reached through `nobject`/`weapon`/`bonus` across many
//! functions and ~120 call sites (mostly tests). Threading a `&mut i32` out-param
//! through all of them purely to carry a **determinism-inert, unhashed** scalar
//! would be the same enormous churn the sound side channel avoids (`sound.rs` §
//! Transport). Instead the create path calls [`raise`], which folds `type.flash`
//! into a **thread-local** accumulator; [`SimState::process_frame`](crate::state::SimState::process_frame)
//! [`begin_frame`]s it with the (already-decremented) persisted `screen_flash` at
//! the top of the tick and [`take_frame`]s it back into the field at the bottom.
//!
//! Seeding with the decremented value makes [`raise`] compute exactly
//! `max(type.flash, screen_flash)` — the C++ write — while keeping the top-of-frame
//! decrement strictly before every raise (there is no in-tick reader, so the raise
//! order among themselves is a commutative `max`).
//!
//! The accumulator is thread-local, so parallel tests never cross-contaminate; it
//! carries only a plain `i32` (never a hashed value or a `rand` handle), so it
//! structurally cannot become a determinism back-channel.

use std::cell::Cell;

thread_local! {
    /// This thread's in-progress per-tick flash accumulator. Seeded by
    /// [`begin_frame`] with the decremented persisted `screen_flash`, raised by
    /// deep [`raise`] calls, read back by [`take_frame`].
    static FLASH: Cell<i32> = const { Cell::new(0) };
}

/// Open a tick: seed the accumulator with the (already top-of-frame-decremented)
/// persisted `screen_flash`, so a subsequent [`raise`] computes `max(type.flash,
/// screen_flash)` exactly as C++ `game.screen_flash = std::max(flash,
/// game.screen_flash)` (`sobject.cpp:41`). Called at the TOP of every
/// `process_frame`.
#[inline]
pub fn begin_frame(seed: i32) {
    FLASH.with(|f| f.set(seed));
}

/// Raise the accumulator to `max(current, flash)` — the sobject-create
/// `screen_flash` write (`sobject.cpp:41`). Determinism-inert: draws no `rand`,
/// touches no hashed state.
#[inline]
pub fn raise(flash: i32) {
    FLASH.with(|f| f.set(f.get().max(flash)));
}

/// Read the accumulated value at the BOTTOM of the tick, to fold back into
/// `SimState.screen_flash`. Non-draining: [`begin_frame`] reseeds it next tick.
#[inline]
pub fn take_frame() -> i32 {
    FLASH.with(|f| f.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_seeds_and_take_reads_it_back() {
        begin_frame(5);
        assert_eq!(take_frame(), 5, "take reads the seed when nothing raised");
    }

    #[test]
    fn raise_takes_the_max_of_seed_and_flash() {
        // C++ max(flash, screen_flash): a larger seed wins, a larger flash wins.
        begin_frame(10);
        raise(4);
        assert_eq!(take_frame(), 10, "seed larger -> seed wins");

        begin_frame(3);
        raise(8);
        assert_eq!(take_frame(), 8, "flash larger -> flash wins");
    }

    #[test]
    fn multiple_raises_accumulate_the_running_max() {
        begin_frame(0);
        raise(2);
        raise(9);
        raise(5);
        assert_eq!(
            take_frame(),
            9,
            "the running max across every raise this tick"
        );
    }
}
