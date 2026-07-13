//! Native audio backend (Slice 4c, T3): the `AudioSink` seam, the sample table,
//! and the per-tick event drainer with its liveness reaper.
//!
//! `sim::sound::SoundEvent`s are a **pure output** of `process_frame` — plain
//! Rust, hash-inert (design §2.2). This module is the `game`-side *consumer*:
//! [`Drainer`] turns a tick's `&[SoundEvent]` into calls on an [`AudioSink`]
//! impl, keeping a small `loops: HashMap<LoopKey, ()>` so a loop's `Play` is
//! idempotent (the sim emits `Play(loop)` every tick it should sound — no
//! `IsPlaying` state in the sim, design §3.4/§4.1) and a `Stop` on an already-
//! stopped key is a no-op. [`Drainer::reap`] is the belt-and-braces liveness
//! reaper (design §4.2): a *second*, independent path back to the same
//! guarantee — given this tick's live keys (read from `SimState` by the
//! caller), it stops and drops any loop whose key is no longer live, self-
//! healing a dropped `Stop` event instead of leaking a channel.
//!
//! [`RodioSink`] plays real audio on native AND wasm (Slice 4c, T5: `rodio`'s
//! `wasm-bindgen` feature turns on `cpal`'s WebAudio host — `game/Cargo.toml`'s
//! wasm target table — so this is the SAME struct/impl on both targets, not a
//! separate wasm sink; only the sample-table *load* forks, since wasm has no
//! filesystem: [`load_sound_table_wasm`] reads through the `read_asset` embed
//! seam (`scenario::assets`) instead of `std::fs`). [`NullSink`] is the C++
//! `NullSoundPlayer` (`mixer/player.hpp:88-94`) analog for headless callers —
//! `replay_state_series`, the 4b round-trip, and the passthrough gate never
//! construct a real sink (design §6.4). [`MockSink`] records calls for tests.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sim::sound::{LoopKey, SoundAction, SoundEvent};

use rodio::Source as _;

/// The `game`-side audio backend seam. Mirrors the C++ `SoundPlayer`
/// (`mixer/player.hpp:12-44`): a one-shot `Play`, a keyed loop `Play`/`Stop`.
/// Unlike C++, there is no `IsPlaying`/`speculative` here — idempotency is
/// [`Drainer`]'s job (design §3.4), and Step 4 has no resim (design §2.3).
pub trait AudioSink {
    /// Fire-and-forget one-shot at the resolved sample-table index `sound`.
    fn play_one_shot(&mut self, sound: i32);
    /// Start the looping channel `key` at the resolved index `sound`. Called
    /// at most once per *new* key by [`Drainer`] (idempotency lives there) —
    /// an impl may assume `key` is not already sounding.
    fn play_loop(&mut self, key: LoopKey, sound: i32);
    /// Stop the looping channel `key`. Called only for a `key` the caller
    /// believes is active; a stop on an unknown key must be a harmless no-op.
    fn stop_loop(&mut self, key: LoopKey);
}

/// The C++ `NullSoundPlayer` (`mixer/player.hpp:88-94`) analog: every call is
/// a no-op. Used by every headless entry point (design §6.4) — construction
/// carries no side effect, so it never touches an audio device.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl AudioSink for NullSink {
    fn play_one_shot(&mut self, _sound: i32) {}
    fn play_loop(&mut self, _key: LoopKey, _sound: i32) {}
    fn stop_loop(&mut self, _key: LoopKey) {}
}

/// One recorded [`AudioSink`] call, in call order — [`MockSink`]'s log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkCall {
    OneShot(i32),
    Loop(LoopKey, i32),
    Stop(LoopKey),
}

/// Records every call in order, for asserting the `Drainer`'s output against
/// an event sequence without a real audio device.
#[derive(Debug, Default, Clone)]
pub struct MockSink {
    pub calls: Vec<SinkCall>,
}

impl AudioSink for MockSink {
    fn play_one_shot(&mut self, sound: i32) {
        self.calls.push(SinkCall::OneShot(sound));
    }
    fn play_loop(&mut self, key: LoopKey, sound: i32) {
        self.calls.push(SinkCall::Loop(key, sound));
    }
    fn stop_loop(&mut self, key: LoopKey) {
        self.calls.push(SinkCall::Stop(key));
    }
}

/// Drains a tick's `&[SoundEvent]` into an [`AudioSink`], keeping the small
/// `loops` bookkeeping map that makes loop `Play` idempotent and loop `Stop`
/// a no-op when the key isn't tracked (design §4.1). Generic over the sink so
/// tests use [`MockSink`]/[`NullSink`] and `game`'s wiring (T4) uses
/// [`RodioSink`].
pub struct Drainer<S: AudioSink> {
    sink: S,
    /// Keys believed to be currently looping. The value is `()` — this is a
    /// membership set, not a handle table (the sink owns real handles).
    loops: HashMap<LoopKey, ()>,
}

impl<S: AudioSink> Drainer<S> {
    pub fn new(sink: S) -> Self {
        Drainer {
            sink,
            loops: HashMap::new(),
        }
    }

    /// Drive one tick's events into the sink (design §4.1):
    /// - one-shot `Play` (`key = None`): always forwarded.
    /// - loop `Play` (`key = Some(k)`): forwarded and tracked **iff** `k` was
    ///   not already tracked; otherwise a no-op (idempotent — the sim emits
    ///   `Play(loop)` every tick with no `IsPlaying` gate, design §3.4).
    /// - loop `Stop` (`key = Some(k)`): forwarded and untracked **iff** `k`
    ///   was tracked; otherwise a no-op (a second/stray `Stop` self-heals,
    ///   design §2.3).
    pub fn drain(&mut self, events: &[SoundEvent]) {
        for ev in events {
            match (ev.key, ev.action) {
                (None, SoundAction::Play) => self.sink.play_one_shot(ev.sound),
                (Some(key), SoundAction::Play) => {
                    // `insert` returns the prior value: `None` => first time
                    // this key has been seen since the last stop/reap.
                    if self.loops.insert(key, ()).is_none() {
                        self.sink.play_loop(key, ev.sound);
                    }
                }
                (Some(key), SoundAction::Stop) => {
                    if self.loops.remove(&key).is_some() {
                        self.sink.stop_loop(key);
                    }
                }
                // The sim never emits a one-shot Stop (sound.rs's SoundEvent
                // constructors pin `key = Some(..)` for every Stop) — a
                // defensive no-op, not a reachable path.
                (None, SoundAction::Stop) => {}
            }
        }
    }

    /// Belt-and-braces liveness reaper (design §4.2, risk 1): stop and untrack
    /// every currently-tracked key that is **not** in `live_keys` (read by the
    /// caller from `SimState` each tick — dead worms, switched weapon slots).
    /// This is a second, independent path to the same "no leaked channel"
    /// guarantee as the explicit `Stop` events in [`Drainer::drain`] — it
    /// self-heals a dropped `Stop` instead of leaking a channel forever.
    pub fn reap(&mut self, live_keys: &HashSet<LoopKey>) {
        let dead: Vec<LoopKey> = self
            .loops
            .keys()
            .copied()
            .filter(|k| !live_keys.contains(k))
            .collect();
        for key in dead {
            self.loops.remove(&key);
            self.sink.stop_loop(key);
        }
    }

    /// Number of currently-tracked loop keys — the leak-model test's
    /// observable (design §8 risk 1: "the `loops` map returns to empty").
    pub fn loops_len(&self) -> usize {
        self.loops.len()
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }
}

/// Native playback sample rate: `assets::wav::WavSound::upsampled()` yields a
/// 2x-upsampled `Vec<i16>` presented at 44100 Hz mono (design §5, "Sample
/// rate").
pub const SAMPLE_RATE: u32 = 44100;

/// The `sounds[]` sample table: index -> playback samples (`upsampled()`
/// output; empty for a missing/silent slot). Index order must match the
/// resolved indices `sim::sound::SoundEvent.sound` carries, i.e. the TC's
/// `[types] sounds` order (`assets::tc::TcConfig::types.sounds`) — the same
/// order `assets::tc::SoundHooks`/`SoundIndex` resolve against.
pub type SoundTable = Vec<Vec<i16>>;

/// Load the sample table from `tc_root/sounds/<name>.wav` in `sound_names`
/// order by reading the filesystem directly (native path; the wasm embed
/// path is [`load_sound_table_wasm`], T5). Mirrors C++ `Common::load`'s
/// per-sound loop
/// (`common.cpp:325-362`): a missing WAV keeps the slot (preserving stable
/// indices for its siblings) but leaves it silent (empty samples) rather than
/// failing the whole table load.
pub fn load_sound_table(tc_root: &Path, sound_names: &[String]) -> SoundTable {
    sound_names
        .iter()
        .map(|name| {
            let path = tc_root.join("sounds").join(format!("{name}.wav"));
            std::fs::read(&path)
                .ok()
                .and_then(|bytes| assets::wav::WavSound::load(&bytes).ok())
                .map(|snd| snd.upsampled())
                .unwrap_or_default()
        })
        .collect()
}

/// Wasm sample-table load (Slice 4c, T5): mirrors [`load_sound_table`] but
/// reads each `sounds/<name>.wav` through [`scenario::assets::read_asset`]'s
/// wasm embed branch (`scenario/src/assets.rs`) instead of `std::fs` — the
/// browser has no filesystem. `tc_root` is threaded through unused (the wasm
/// `read_asset` ignores it, same as every other wasm asset read) purely to
/// keep the call shape identical to [`load_sound_table`]'s.
///
/// Unlike the native loader this does NOT degrade a missing file to a silent
/// slot: the wasm embed (`assets.rs`) is exhaustive for this TC (`sounds/`'s
/// 30 WAVs cover every name `tc.cfg`'s `[types] sounds` lists — verified
/// against the shipped TC) and a build-time-known key set makes a miss a bug,
/// not a runtime condition (mirroring `read_asset`'s own "a miss is a bug"
/// policy for every other wasm embed). A malformed WAV still degrades to a
/// silent (empty) slot, same as native — only the "file present" step differs.
#[cfg(target_arch = "wasm32")]
pub fn load_sound_table_wasm(tc_root: &Path, sound_names: &[String]) -> SoundTable {
    sound_names
        .iter()
        .map(|name| {
            let bytes = scenario::assets::read_asset(tc_root, &format!("sounds/{name}.wav"));
            assets::wav::WavSound::load(&bytes)
                .map(|snd| snd.upsampled())
                .unwrap_or_default()
        })
        .collect()
}

/// `rodio`-backed [`AudioSink`] (design §5) — native AND wasm (T5: `rodio`'s
/// `wasm-bindgen` feature selects `cpal`'s WebAudio host on `wasm32`, so this
/// struct needs no per-target variant; only [`RodioSink::try_new`]'s caller
/// differs by target, in `load_sound_table` vs [`load_sound_table_wasm`]).
/// Holds the output stream alive for the sink's lifetime (dropping
/// `OutputStream` ends playback — `rodio` 0.19 `stream.rs`), one transient
/// `Sink` per one-shot (`.detach()`d so it survives past this call, matching
/// fire-and-forget), and one `Sink` per active [`LoopKey`] (`.repeat_infinite()`,
/// dropped/stopped on `stop_loop` — `rodio::Sink`'s `Drop` stops playback
/// unless `detach`ed).
pub struct RodioSink {
    // Held only to keep the output stream alive; never read after
    // construction (dropping it would silence every sink).
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    sounds: SoundTable,
    loop_sinks: HashMap<LoopKey, rodio::Sink>,
}

impl RodioSink {
    /// Open the default output device and build a sink over `sounds` (design
    /// §5). `Err` iff no output device is available (e.g. a headless CI box,
    /// or a wasm build with no `AudioContext`); the caller (T4/T5's `setup`)
    /// falls back to [`NullSink`].
    pub fn try_new(sounds: SoundTable) -> Result<Self, rodio::StreamError> {
        let (stream, handle) = rodio::OutputStream::try_default()?;
        Ok(RodioSink {
            _stream: stream,
            handle,
            sounds,
            loop_sinks: HashMap::new(),
        })
    }

    /// The resolved index's samples as a fresh mono 44100 Hz `SamplesBuffer`
    /// (design §5's "one-line fit"), or `None` for an out-of-range/empty
    /// (silent-slot) index — the C++ `Play` negative-index guard
    /// (`player.hpp:19`) already keeps `sound` non-negative on entry, but an
    /// out-of-range or missing-file slot must still play nothing rather than
    /// panic.
    fn buffer(&self, sound: i32) -> Option<rodio::buffer::SamplesBuffer<i16>> {
        let idx = usize::try_from(sound).ok()?;
        let samples = self.sounds.get(idx)?;
        if samples.is_empty() {
            return None;
        }
        Some(rodio::buffer::SamplesBuffer::new(
            1,
            SAMPLE_RATE,
            samples.clone(),
        ))
    }
}

impl AudioSink for RodioSink {
    fn play_one_shot(&mut self, sound: i32) {
        let Some(buf) = self.buffer(sound) else {
            return;
        };
        let Ok(sink) = rodio::Sink::try_new(&self.handle) else {
            return;
        };
        sink.append(buf);
        sink.detach();
    }

    fn play_loop(&mut self, key: LoopKey, sound: i32) {
        let Some(buf) = self.buffer(sound) else {
            return;
        };
        let Ok(sink) = rodio::Sink::try_new(&self.handle) else {
            return;
        };
        sink.append(buf.repeat_infinite());
        self.loop_sinks.insert(key, sink);
    }

    fn stop_loop(&mut self, key: LoopKey) {
        // Removing (dropping) the Sink stops it: rodio 0.19's
        // `impl Drop for Sink` sets the stopped flag unless `detach`ed
        // (`sink.rs:335-344`), which this path never calls.
        self.loop_sinks.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worm(i: u8) -> LoopKey {
        LoopKey::Worm(i)
    }
    fn weap(w: u8, s: u8) -> LoopKey {
        LoopKey::WormWeapon(w, s)
    }

    #[test]
    fn drain_forwards_one_shot_and_loop_lifecycle() {
        // The T3 RED case (plan): Play(None), Play(Some(k)), Stop(Some(k)).
        let k = weap(0, 1);
        let events = vec![
            SoundEvent::one_shot(5),
            SoundEvent::loop_play(9, k),
            SoundEvent::loop_stop(k),
        ];
        let mut drainer = Drainer::new(MockSink::default());
        drainer.drain(&events);
        assert_eq!(
            drainer.sink().calls,
            vec![
                SinkCall::OneShot(5),
                SinkCall::Loop(k, 9),
                SinkCall::Stop(k),
            ]
        );
        assert_eq!(drainer.loops_len(), 0, "stop leaves the loops map empty");
    }

    #[test]
    fn repeated_loop_play_is_idempotent() {
        // The sim re-emits Play(loop) every tick it should sound (no
        // IsPlaying state in the sim, design §3.4) — the drainer must start
        // the channel once, not once per tick.
        let k = worm(1);
        let mut drainer = Drainer::new(MockSink::default());
        drainer.drain(&[SoundEvent::loop_play(3, k)]);
        drainer.drain(&[SoundEvent::loop_play(3, k)]);
        drainer.drain(&[SoundEvent::loop_play(3, k)]);
        assert_eq!(drainer.sink().calls, vec![SinkCall::Loop(k, 3)]);
        assert_eq!(drainer.loops_len(), 1);
    }

    #[test]
    fn second_stop_on_the_same_key_is_a_no_op() {
        // Fire+Change on the same tick emits two Stops for one key
        // (input-map §6 / the plan's T2 note) — the second must not re-call
        // the sink.
        let k = weap(0, 2);
        let mut drainer = Drainer::new(MockSink::default());
        drainer.drain(&[SoundEvent::loop_play(4, k)]);
        drainer.drain(&[SoundEvent::loop_stop(k), SoundEvent::loop_stop(k)]);
        assert_eq!(
            drainer.sink().calls,
            vec![SinkCall::Loop(k, 4), SinkCall::Stop(k)],
            "only the first Stop reaches the sink"
        );
        assert_eq!(drainer.loops_len(), 0);
    }

    #[test]
    fn stop_on_an_untracked_key_is_a_no_op() {
        let mut drainer = Drainer::new(MockSink::default());
        drainer.drain(&[SoundEvent::loop_stop(worm(9))]);
        assert!(drainer.sink().calls.is_empty());
        assert_eq!(drainer.loops_len(), 0);
    }

    #[test]
    fn fire_cease_death_leaves_loops_map_empty() {
        // The design §8 risk-1 leak-model test: fire -> cease -> death,
        // driven as three separate ticks (the shape a real drain sequence
        // takes), must return the loops map to empty.
        let fire = weap(0, 1);
        let worm_loop = worm(0);
        let mut drainer = Drainer::new(MockSink::default());

        // Fire tick: both the weapon loop and the worm-fire loop start.
        drainer.drain(&[
            SoundEvent::loop_play(10, fire),
            SoundEvent::loop_play(11, worm_loop),
        ]);
        assert_eq!(drainer.loops_len(), 2);

        // Cease-fire tick: explicit Stops at the C++ sites (design §3.4).
        drainer.drain(&[
            SoundEvent::loop_stop(fire),
            SoundEvent::loop_stop(worm_loop),
        ]);
        assert_eq!(drainer.loops_len(), 0);

        // Death tick: nothing left to leak even without a Stop event.
        drainer.drain(&[]);
        assert_eq!(drainer.loops_len(), 0);
        assert_eq!(
            drainer.sink().calls,
            vec![
                SinkCall::Loop(fire, 10),
                SinkCall::Loop(worm_loop, 11),
                SinkCall::Stop(fire),
                SinkCall::Stop(worm_loop),
            ]
        );
    }

    #[test]
    fn reap_stops_and_untracks_keys_missing_from_live_keys() {
        // The belt-and-braces path (design §4.2 risk 2): a Stop event was
        // dropped (simulated here by never sending one), but the key is no
        // longer live (e.g. the worm died) — reap must self-heal it.
        let dead = worm(2);
        let alive = worm(3);
        let mut drainer = Drainer::new(MockSink::default());
        drainer.drain(&[
            SoundEvent::loop_play(1, dead),
            SoundEvent::loop_play(2, alive),
        ]);
        assert_eq!(drainer.loops_len(), 2);

        let live_keys = HashSet::from([alive]);
        drainer.reap(&live_keys);

        assert_eq!(drainer.loops_len(), 1, "only the live key survives reap");
        assert_eq!(
            drainer.sink().calls,
            vec![
                SinkCall::Loop(dead, 1),
                SinkCall::Loop(alive, 2),
                SinkCall::Stop(dead),
            ]
        );
    }

    #[test]
    fn reap_with_all_keys_live_is_a_no_op() {
        let k = worm(0);
        let mut drainer = Drainer::new(MockSink::default());
        drainer.drain(&[SoundEvent::loop_play(1, k)]);
        drainer.reap(&HashSet::from([k]));
        assert_eq!(drainer.loops_len(), 1);
        assert_eq!(drainer.sink().calls, vec![SinkCall::Loop(k, 1)]);
    }

    #[test]
    fn null_sink_is_a_no_op_and_never_panics() {
        let mut sink = NullSink;
        sink.play_one_shot(0);
        sink.play_loop(worm(0), 1);
        sink.stop_loop(worm(0));
    }

    /// The real TC's `sounds/` directory, in `tc.cfg`'s `[types] sounds`
    /// order — exercises `load_sound_table` against the actual asset tree
    /// (mirrors `assets::wav`'s `real_bump_wav_decodes` golden-adjacent
    /// check), not just synthetic bytes.
    #[test]
    fn load_sound_table_decodes_the_real_tc_sounds_in_types_order() {
        let tc_root = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero"
        ));
        let tc_bytes = std::fs::read(tc_root.join("tc.cfg")).expect("tc.cfg reads");
        let tc = assets::tc::TcConfig::load(&tc_bytes).expect("tc.cfg parses");

        let table = load_sound_table(tc_root, &tc.types.sounds);

        assert_eq!(table.len(), tc.types.sounds.len());
        let bump_idx = tc
            .types
            .sounds
            .iter()
            .position(|n| n == "bump")
            .expect("bump is in the shipped TC's sound list");
        assert!(
            !table[bump_idx].is_empty(),
            "bump.wav is present on disk and must decode to non-empty samples"
        );
    }

    #[test]
    fn load_sound_table_missing_file_is_a_silent_slot_not_a_panic() {
        // Mirrors C++ Common::load: a name with no matching .wav on disk gets
        // an empty (silent) slot instead of failing the whole table load.
        let tc_root = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero"
        ));
        let names = vec!["bump".to_string(), "does_not_exist_xyz".to_string()];
        let table = load_sound_table(tc_root, &names);
        assert_eq!(table.len(), 2);
        assert!(!table[0].is_empty());
        assert!(table[1].is_empty());
    }
}
