//! The sim-emitted **sound-event stream** (Slice 4c).
//!
//! Audio is a pure *consumer* of a per-tick event record the sim produces at its
//! existing `Play`/`Stop` callsites (design §1-2). This module is plain Rust — no
//! Bevy, no `f32`: every event carries a *final* resolved index into the game's
//! `sounds[]` table (the variant is ALREADY chosen inside the sim at the callsite,
//! so the game never re-draws or re-resolves). The stream is drained by the
//! `game` crate; headless/library callers simply ignore it.
//!
//! # Isolation firewall (design §6, the standing invariant since Step 3)
//!
//! The event stream is a **side-channel output only**: it is never read back by
//! the sim, draws no `rand`, and is **never hashed** (`hash.rs` does not walk
//! `SimState.sound_events`). Emission is unconditional `push` at each ported
//! callsite; every variant `rand` already existed pre-4c (the "sound omitted /
//! not hashed" markers), so 4c adds *zero* new draws. `hash_game_state` stays
//! byte-identical on every committed golden — proven structurally by the
//! `rand.draws()` + hash-inert tests, not by a runtime flag.
//!
//! # Transport: a per-frame thread-local collector, drained into `SimState`
//!
//! The `Play`/`Stop` callsites live deep in the object/worm call tree
//! (`physics`, `control`, `weapon`, `sobject`, `bonus`, `state`), reached through
//! ~15 functions with ~120 call sites (mostly tests). Threading a `&mut
//! Vec<SoundEvent>` out-param through all of them — purely to carry a
//! determinism-inert side channel — would be enormous churn for zero behavioural
//! gain. Instead each callsite calls [`one_shot`] / [`emit`], which pushes onto a
//! **thread-local** per-frame buffer; [`SimState::process_frame`] resets that
//! buffer at the top of the tick and moves its contents into
//! `SimState.sound_events` at the bottom. The observable contract of design §2.2
//! is unchanged: `sound_events` is a per-tick `Vec` on `SimState`, cleared at the
//! top, holding exactly this tick's events, drained by `game`.
//!
//! The buffer is thread-local, so parallel tests and multiple sims on different
//! threads never cross-contaminate; the top-of-frame reset means a direct
//! (non-`process_frame`) call in a unit test can never leak stale events into a
//! later `process_frame`. Because the buffer carries only [`SoundEvent`] (never a
//! hashed value or a `rand` handle), it structurally cannot become a determinism
//! back-channel.
//!
//! [`SimState::process_frame`]: crate::state::SimState::process_frame

use std::cell::{Cell, RefCell};

/// Whether an event **starts** a sound or **stops** a looping channel.
///
/// One-shots (Slice-4c T1) only ever use [`SoundAction::Play`]; [`SoundAction::Stop`]
/// is emitted for looping channels (Slice-4c T2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundAction {
    Play,
    Stop,
}

/// A stable identity for a **looping** sound channel.
///
/// Mirrors the C++ `void* id` (the pointer key `IsPlaying`/`Stop` use — input-map
/// §6) but as a *value* key the game can hash into its `loops` map. One-shots
/// carry `None`; loop channels (Slice-4c T2) carry `Some(..)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoopKey {
    /// Keyed on the worm handle (`&worm` / `this`): worm/hit/death loops
    /// (`worm.cpp:360-361`, `weapon.cpp:311`, `nobject.cpp:183`, `sobject.cpp:108`).
    Worm(u8),
    /// Keyed on the worm's current weapon slot (`&weapons[current_weapon]`): the
    /// launch loop (`worm.cpp:1120-1121`, `Stop` at `:341`/`:375`/`:1076`).
    WormWeapon(u8, u8),
}

/// A single sim-emitted sound event (design §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundEvent {
    /// Final index into the game's `sounds[]` table (the variant is ALREADY
    /// chosen in-sim: `start_sound + rand(num_sounds)`, `15 + rand(3)`,
    /// `18 + rand(3)`, `explo_sound`/`launch_sound`, or a resolved
    /// `sound_hooks.*`). The game does a trivial `sounds[ev.sound]` lookup.
    pub sound: i32,
    /// `None` => one-shot (fire-and-forget). `Some(key)` => looping channel
    /// identity (Slice-4c T2).
    pub key: Option<LoopKey>,
    pub action: SoundAction,
}

impl SoundEvent {
    /// A fire-and-forget one-shot (`key = None`, `action = Play`) at the resolved
    /// index `sound`.
    #[inline]
    pub fn one_shot(sound: i32) -> Self {
        SoundEvent {
            sound,
            key: None,
            action: SoundAction::Play,
        }
    }

    /// A **keyed loop start** (`action = Play`, `key = Some(key)`) at the
    /// resolved index `sound` — the C++ `Play(sound, id, loops = -1)`
    /// (`worm.cpp:1121`, the only true-loop callsite in `ProcessFrame`). The sim
    /// emits this every tick the loop should sound; the game backend makes it
    /// idempotent per key (design §3.4/§4.1).
    #[inline]
    pub fn loop_play(sound: i32, key: LoopKey) -> Self {
        SoundEvent {
            sound,
            key: Some(key),
            action: SoundAction::Play,
        }
    }

    /// A **keyed loop stop** (`action = Stop`) — the C++ `Stop(id)`
    /// (`worm.cpp:341`/`:375`/`:1076`). `sound` is meaningless for a Stop (the
    /// game stops whatever plays on `key`), pinned to `-1`.
    #[inline]
    pub fn loop_stop(key: LoopKey) -> Self {
        SoundEvent {
            sound: -1,
            key: Some(key),
            action: SoundAction::Stop,
        }
    }
}

/// The four resolved **worm-hook** sound indices the hook-based one-shot
/// callsites (`bump`, `reloaded`, `alive`, `ninjarope_throw`) need. Mirrors the
/// subset of `assets::tc::SoundHooks` the sim's `ProcessFrame` path plays; set
/// once per tick (from `SimState.sound_hooks`) so deep callsites can play a hook
/// without threading its index through every intervening signature. All four are
/// resolved indices into `sounds[]` (or `< 0` when the TC leaves the hook unset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookIndices {
    pub bump: i32,
    pub reloaded: i32,
    pub alive: i32,
    pub ninjarope_throw: i32,
}

impl HookIndices {
    /// All hooks unset (`-1`). The default until [`begin_frame`] sets the real
    /// indices from the TC; a direct unit-test call that plays a hook without a
    /// preceding `begin_frame` sees `-1` (a harmless "no sound" index).
    pub const UNSET: HookIndices = HookIndices {
        bump: -1,
        reloaded: -1,
        alive: -1,
        ninjarope_throw: -1,
    };
}

thread_local! {
    /// This thread's in-progress per-frame event buffer. Reset at the top of each
    /// `process_frame`, drained into `SimState.sound_events` at the bottom.
    static FRAME: RefCell<Vec<SoundEvent>> = RefCell::new(Vec::new());
    /// This thread's resolved worm-hook indices for the current tick, set by
    /// [`begin_frame`]. Determinism-inert (never read by sim math, never hashed).
    static HOOKS: Cell<HookIndices> = const { Cell::new(HookIndices::UNSET) };
}

/// Open a tick: clear the per-frame buffer AND publish this tick's worm-hook
/// indices. Called at the TOP of every `process_frame` so the buffer holds
/// exactly this tick's events (a prior panic or a direct unit-test call can never
/// leak stale events in) and every hook callsite plays the current TC's index.
#[inline]
pub fn begin_frame(hooks: HookIndices) {
    FRAME.with(|f| f.borrow_mut().clear());
    HOOKS.with(|h| h.set(hooks));
}

/// Clear the per-frame buffer without touching the hook indices — for direct
/// unit tests that drive an emitting function outside `process_frame`.
#[inline]
pub fn reset_frame() {
    FRAME.with(|f| f.borrow_mut().clear());
}

/// Push a sound event from a `Play`/`Stop` callsite onto this thread's per-frame
/// buffer. Determinism-inert (no `rand`, never hashed).
#[inline]
pub fn emit(ev: SoundEvent) {
    FRAME.with(|f| f.borrow_mut().push(ev));
}

/// Play a one-shot at the already-resolved index `sound`. The faithful analog of
/// C++ `SoundPlayer::Play(int sound)` (`mixer/player.hpp:15-21`): a **negative
/// index is a no-op** (`if (sound >= 0)`), so an unset hook / `explo_sound` /
/// `launch_sound` (`-1`) emits nothing — matching C++ observable playback and the
/// design §2.1 contract that a `SoundEvent.sound` is a valid `>= 0` table index.
#[inline]
pub fn one_shot(sound: i32) {
    if sound >= 0 {
        emit(SoundEvent::one_shot(sound));
    }
}

/// Start (or keep sounding) the **looping channel** `key` at the
/// already-resolved index `sound`. The faithful analog of the C++ loop `Play`
/// (`worm.cpp:1120-1121`, `Play(launch_sound, &weapons[cur], -1)`): the same
/// negative-index no-op guard as [`one_shot`] (`player.hpp:15-21`), so a loop
/// weapon whose `launch_sound` is unset (`-1`) starts nothing. Emitted every
/// tick the loop should sound — the sim carries no `IsPlaying` channel state;
/// per-key idempotency is the game backend's job (design §3.4).
#[inline]
pub fn play_loop(sound: i32, key: LoopKey) {
    if sound >= 0 {
        emit(SoundEvent::loop_play(sound, key));
    }
}

/// Stop the looping channel `key` — the C++ `Stop(id)` (`worm.cpp:341`/`:375`/
/// `:1076`). **Unconditional**: C++ never speculative-gates `Stop` (input-map
/// §6 — a suppressed stop leaks a channel, a spurious stop self-heals), and a
/// Stop on an idle key is a game-side no-op (design §4.1).
#[inline]
pub fn stop_loop(key: LoopKey) {
    emit(SoundEvent::loop_stop(key));
}

/// Play the `SoundBump` worm hook (`worm.cpp:175`/`:188`) — the wall-bounce
/// one-shot, using this tick's resolved `bump` index.
#[inline]
pub fn play_bump() {
    one_shot(HOOKS.with(|h| h.get().bump));
}

/// Play the `SoundReloaded` worm hook (`worm.cpp:309`/`:833`) — the
/// reload-complete one-shot, using this tick's resolved `reloaded` index.
#[inline]
pub fn play_reloaded() {
    one_shot(HOOKS.with(|h| h.get().reloaded));
}

/// Play the `SoundAlive` worm hook (`worm.cpp:789`) — the respawn one-shot, using
/// this tick's resolved `alive` index.
#[inline]
pub fn play_alive() {
    one_shot(HOOKS.with(|h| h.get().alive));
}

/// Play the `SoundNinjaropeThrow` worm hook (`worm.cpp:979`) — the rope-throw
/// one-shot, using this tick's resolved `ninjarope_throw` index.
#[inline]
pub fn play_ninjarope_throw() {
    one_shot(HOOKS.with(|h| h.get().ninjarope_throw));
}

/// Take this frame's collected events, leaving the buffer empty — called at the
/// BOTTOM of every `process_frame` to move them into `SimState.sound_events`.
#[inline]
pub fn take_frame() -> Vec<SoundEvent> {
    FRAME.with(|f| std::mem::take(&mut *f.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_is_play_with_no_key() {
        let ev = SoundEvent::one_shot(7);
        assert_eq!(ev.sound, 7);
        assert_eq!(ev.key, None);
        assert_eq!(ev.action, SoundAction::Play);
    }

    #[test]
    fn emit_collects_then_take_drains() {
        reset_frame();
        one_shot(3);
        emit(SoundEvent::one_shot(9));
        let drained = take_frame();
        assert_eq!(
            drained,
            vec![SoundEvent::one_shot(3), SoundEvent::one_shot(9)]
        );
        // A second take is empty — take_frame left the buffer cleared.
        assert!(take_frame().is_empty());
    }

    #[test]
    fn reset_frame_discards_stale_events() {
        emit(SoundEvent::one_shot(1));
        reset_frame();
        assert!(take_frame().is_empty());
    }

    #[test]
    fn one_shot_negative_index_is_a_no_op() {
        // Mirrors C++ `SoundPlayer::Play(int)` `if (sound >= 0)`: a negative index
        // (an unset hook / explo_sound / launch_sound) plays nothing.
        reset_frame();
        one_shot(-1);
        one_shot(0);
        one_shot(-42);
        assert_eq!(
            take_frame(),
            vec![SoundEvent::one_shot(0)],
            "only the non-negative index emits"
        );
    }

    #[test]
    fn unset_hooks_emit_nothing() {
        // With the default UNSET hooks (all -1), every hook play is a no-op.
        begin_frame(HookIndices::UNSET);
        play_bump();
        play_reloaded();
        play_alive();
        play_ninjarope_throw();
        assert!(take_frame().is_empty(), "unset (-1) hooks play nothing");
    }

    #[test]
    fn loop_play_carries_key_and_play_action() {
        // The T2 loop constructor: a keyed Play at the resolved index.
        let key = LoopKey::WormWeapon(1, 3);
        let ev = SoundEvent::loop_play(9, key);
        assert_eq!(ev.sound, 9);
        assert_eq!(ev.key, Some(key));
        assert_eq!(ev.action, SoundAction::Play);
    }

    #[test]
    fn loop_stop_carries_key_and_stop_action() {
        // The T2 stop constructor: `sound` is meaningless for a Stop (the game
        // stops whatever plays on the key), pinned to -1.
        let key = LoopKey::Worm(2);
        let ev = SoundEvent::loop_stop(key);
        assert_eq!(ev.sound, -1);
        assert_eq!(ev.key, Some(key));
        assert_eq!(ev.action, SoundAction::Stop);
    }

    #[test]
    fn play_loop_negative_index_is_a_no_op_but_stop_loop_always_emits() {
        // `play_loop` mirrors the C++ `Play` guard (`player.hpp:15-21`,
        // `if (sound >= 0)`): a loop weapon with launch_sound unset (-1) starts
        // nothing. `stop_loop` is UNCONDITIONAL — C++ never speculative-gates
        // Stop (input-map §6, a suppressed stop leaks a channel); a Stop on an
        // idle key is a game-side no-op (design §4.1).
        reset_frame();
        let key = LoopKey::WormWeapon(0, 1);
        play_loop(-1, key);
        play_loop(4, key);
        stop_loop(key);
        assert_eq!(
            take_frame(),
            vec![SoundEvent::loop_play(4, key), SoundEvent::loop_stop(key)],
            "negative-index play skipped; valid play + unconditional stop emitted"
        );
    }
}
