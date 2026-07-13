//! The sim-emitted **explosion-shake event stream** (Slice 4d T1).
//!
//! Screen shake is per-**viewport** and position-dependent: C++
//! `SObjectType::Create` walks `game.viewports` and, for every viewport whose
//! rect contains the RAW blast `(x, y)`, does `v.shake = max(Itof(shake),
//! v.shake)` (`sobject.cpp:27-33`). The viewport concept does not exist inside
//! the sim firewall (viewports are a render/game datum), so the sim cannot set
//! `shake` directly. Instead it **emits one `(x, y, amount)` event per explosion
//! sobject with `shake > 0`** into a per-tick [`Vec`] that the `game` layer drains
//! ([`SimState::drain_shake_events`](crate::state::SimState::drain_shake_events)) and
//! applies against the two live viewports in Slice 4d T2 — doing the per-viewport
//! rect test + `itof(amount)` + `max` there (design §3b).
//!
//! # Isolation firewall — hash-neutral, zero `rand`
//!
//! The stream is a **side-channel output only**: never read back by the sim, never
//! hashed ([`crate::hash`] does not walk `SimState.shake_events`), and it draws
//! **zero** `rand` (the viewport shake loop in C++ is rand-free — the shake RNG
//! lives in `Viewport::Process` at render time, which the sim never runs). So every
//! committed `sim_slice*` golden stays byte-identical whether the vec is populated
//! or empty (design §0, §9 risk 4) — proven by the `rand.draws()` + hash-inert
//! tests, not a runtime flag.
//!
//! # Transport: a per-tick thread-local collector, drained into `SimState`
//!
//! Like the `screen_flash` raise (`flash.rs`) and the sound stream (`sound.rs`),
//! the shake write fires deep in the object/worm call tree — `SObjectType::Create`
//! is reached through `nobject`/`weapon`/`bonus` across many functions and ~120
//! call sites (mostly tests). Threading a `&mut Vec<ShakeEvent>` out-param through
//! all of them purely to carry a **determinism-inert, unhashed** side channel would
//! be the same enormous churn the sound/flash side channels avoid. Instead the
//! create path calls [`emit`], which pushes onto a **thread-local** per-tick buffer;
//! [`SimState::process_frame`](crate::state::SimState::process_frame) [`begin_frame`]s
//! it (clear) at the top of the tick and [`take_frame`]s it into
//! `SimState.shake_events` at the bottom.
//!
//! The buffer is thread-local, so parallel tests never cross-contaminate; the
//! top-of-frame clear means a direct (non-`process_frame`) unit-test call can never
//! leak stale events into a later `process_frame`. Because it carries only a plain
//! [`ShakeEvent`] (three `i32`s — never a hashed value or a `rand` handle), it
//! structurally cannot become a determinism back-channel.

use std::cell::RefCell;

/// A single explosion-shake event emitted at sobject creation
/// (`sobject.cpp:27-33`). `x`/`y` are the **raw** blast pixel coordinates (the
/// `SObjectType::Create` args, PRE the `-8` sprite offset applied to
/// `obj.x`/`obj.y`), matching the C++ viewport-containment test which reads the raw
/// `(x, y)`. `amount` is the raw `type.shake`; the game layer applies
/// `itof(amount)` and `max`es it into each containing viewport's `shake` (T2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShakeEvent {
    pub x: i32,
    pub y: i32,
    pub amount: i32,
}

thread_local! {
    /// This thread's in-progress per-tick shake-event buffer. Cleared at the top
    /// of each `process_frame`, drained into `SimState.shake_events` at the bottom.
    static FRAME: RefCell<Vec<ShakeEvent>> = const { RefCell::new(Vec::new()) };
}

/// Open a tick: clear the per-tick buffer so it holds exactly this tick's events
/// (a prior panic or a direct unit-test call can never leak stale events in).
/// Called at the TOP of every `process_frame`.
#[inline]
pub fn begin_frame() {
    FRAME.with(|f| f.borrow_mut().clear());
}

/// Clear the per-tick buffer without any other side effect — for direct unit tests
/// that drive an emitting function outside `process_frame`.
#[inline]
pub fn reset_frame() {
    FRAME.with(|f| f.borrow_mut().clear());
}

/// Push an explosion-shake event from the sobject-create path onto this thread's
/// per-tick buffer. Determinism-inert (no `rand`, never hashed). Called only when
/// `type.shake > 0` (`sobject.cpp:31` is a no-op `max(0, v.shake)` otherwise).
#[inline]
pub fn emit(x: i32, y: i32, amount: i32) {
    FRAME.with(|f| f.borrow_mut().push(ShakeEvent { x, y, amount }));
}

/// Take this tick's collected events, leaving the buffer empty — called at the
/// BOTTOM of every `process_frame` to move them into `SimState.shake_events`.
#[inline]
pub fn take_frame() -> Vec<ShakeEvent> {
    FRAME.with(|f| std::mem::take(&mut *f.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_collects_then_take_drains() {
        reset_frame();
        emit(10, 20, 4);
        emit(1, 2, 9);
        let drained = take_frame();
        assert_eq!(
            drained,
            vec![
                ShakeEvent {
                    x: 10,
                    y: 20,
                    amount: 4
                },
                ShakeEvent {
                    x: 1,
                    y: 2,
                    amount: 9
                },
            ]
        );
        // A second take is empty — take_frame left the buffer cleared.
        assert!(take_frame().is_empty());
    }

    #[test]
    fn begin_frame_discards_stale_events() {
        emit(7, 7, 7);
        begin_frame();
        assert!(take_frame().is_empty(), "begin_frame clears stale events");
    }

    #[test]
    fn reset_frame_discards_stale_events() {
        emit(3, 4, 5);
        reset_frame();
        assert!(take_frame().is_empty());
    }
}
