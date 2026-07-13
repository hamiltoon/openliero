# Step 4, Slice 4c — Audio: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** Make a live match **audible** by having the sim emit a per-tick sound-event stream at its
existing `Play`/`Stop` callsites (**zero new `rand`, zero hashed-state change**), drained by a thin
`game`-side audio backend — loops keyed by object handle with play/stop. The sim stays Bevy-free and
`hash_game_state` byte-identical. Companion spec:
`specs/2026-07-13-liero-rs-step4-slice4c-audio-design.md` (cited **spec §N**).

**Architecture:** Additive. **`sim` crate:** new Bevy-free `sound.rs` (`SoundEvent`/`LoopKey`/
`SoundAction`); a `sound_events: Vec<SoundEvent>` field on `SimState`, cleared at the top of
`process_frame`, pushed at each ported callsite (spec §3). **Not hashed** (spec §6). `process_frame`'s
signature is unchanged. **`game` crate:** an `AudioSink` trait (`RodioSink` native + `NullSink`), a
`sounds[]` sample table loaded via `assets::WavSound`, a `loops: HashMap<LoopKey, _>` drainer wired
into `tick_and_render` after `process_frame`. **wasm:** embed `sounds/` in `scenario/src/assets.rs`;
Web-Audio sink. `render`/`scenario`(logic)/`assets` stay behavior-unchanged.

**Tech stack:** Rust. New dep `rodio` (native; behind the `AudioSink` seam — spec §5). Samples from
`assets::WavSound::upsampled()` (already golden, `i16` mono, present at 44100 Hz). CI: the sim event +
isolation tests ride `cargo test --workspace --exclude game`; the drainer/keying tests ride
`cargo test -p game`. No new CI job; audio output is never CI-gated (spec §1).

## Global constraints

*(inherit every Step 2/3/4a/4b constraint; the 4c-specific ones follow)*

- **Bevy stays confined to `game`.** `sim`, `render`, `scenario`, `assets`, `sim-core` remain
  Bevy-free. `sim/src/sound.rs` is plain Rust (no Bevy, no `f32`) — `cargo tree -p sim` shows no
  `bevy*`. The new `SimState.sound_events` field is the only sim-state addition and is **never hashed**
  (spec §6.1) — no `f32`/`Vec2`/`Transform` enters the sim (the 3c isolation firewall).
- **Zero new `rand`, zero hashed-state change.** Every sound-variant draw already exists in the sim
  (the "sound not hashed / omitted" markers); 4c only converts each omit into a `Vec::push`. A test
  asserts `rand.draws()` on a representative tick is unchanged, and a test asserts `hash_game_state` is
  identical with `sound_events` populated vs cleared (spec §6.1/§6.3).
- **No golden moves.** Every committed `sim_slice*` / `render_slice3b_*` / `render_slice3e_*` /
  `record_slice4b_*` golden stays **byte-identical** — 4c is additive. `git diff --stat golden/` shows
  only *new* 4c fixtures (if any), never a change to an existing golden.
- **Headless is silent; the 4b hard gate holds.** `replay_state_series`, the 4b round-trip, and the
  passthrough gate construct **no sink** (or `NullSink`) and read only `hash_game_state`. They stay
  green unchanged (spec §6.4).
- **Loops must not leak channels** (spec §4.2, input-map §6). Emit the explicit C++ `Stop` sites **and**
  run the game-side liveness reaper; a test proves the `loops` map returns to empty after
  fire→cease→death. `Stop` events are unconditional (never speculative-gated).
- **No speculative code in 4c.** Step 4 has no resim; the Play-emit guard is a Step-5 forward hook only
  (spec §2.3) — do **not** add a `speculative` flag to the Rust sim now.
- **Do NOT touch slice 4d.** Another planner owns 4d. Do not edit any `slice4d*` file, the overview's
  4d bullet, or `PROGRESS.md`'s 4d line. Update only the 4c overview bullet + the 4c `PROGRESS.md` line.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no `Claude-Session`).
  **Commit on branch `liero-rs-step-4`** (the accumulating Step-4 PR). **Do NOT push and do NOT open a
  PR** — the controller owns push + PR. **fmt only new/edited files by hand — write new lines already
  fmt-clean; do NOT run `rustfmt` on files that are not already fmt-clean** (the `blit.rs` footgun; do
  not blanket-fmt). **No sub-subagents.** **Bash discipline:** one command per call; no
  `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## File structure

- `rust/sim/src/sound.rs` — **NEW.** `SoundEvent`, `LoopKey`, `SoundAction`. Unit tests.
- `rust/sim/src/lib.rs` — add `pub mod sound;`.
- `rust/sim/src/state.rs` — add `sound_events: Vec<SoundEvent>`; clear at top of `process_frame`;
  emit at the one-shot callsites (spec §3.1–§3.3) and the death/respawn sites. Unit tests per family.
- `rust/sim/src/{physics,control,weapon,sobject,nobject,bonus}.rs` — convert the existing omit-markers
  (spec §3) into `self.sound_events.push(...)` at the exact callsite.
- `rust/sim/src/hash.rs` — **no change** (the field is deliberately not walked); add the hash-inert
  assertion in a test module.
- `rust/game/src/audio.rs` — **NEW.** `AudioSink` trait; `NullSink`; `RodioSink` (native); the
  `loops: HashMap<LoopKey, _>` drainer + liveness reaper; sample table from `assets::WavSound`.
- `rust/game/src/main.rs` — drain `sim.0.sound_events` into the sink in `tick_and_render` after
  `process_frame`; construct the sink in `setup` (native Live/Scripted); `NullSink` headless.
- `rust/game/src/lib.rs` — `pub mod audio;`.
- `rust/game/Cargo.toml` — add `rodio` (native target table; wasm handled in T5).
- `rust/scenario/src/assets.rs` — T5: embed `sounds/` for the wasm target.
- `docs/superpowers/liero-rs-PROGRESS.md` + the overview's **4c** line — updated in T6 (4c only).

## Tasks

### T1 — Sim one-shot sound events + isolation firewall  [Opus]

The core parity task: the event type, the drained-per-tick vec, and every **one-shot** callsite.

- [ ] **RED:** in `sim/src/sound.rs` add `SoundEvent`/`LoopKey`/`SoundAction`; in `state.rs` add the
  `sound_events` field + clear-at-top. Write a failing test: after a `process_frame` that bumps a worm,
  `sim.sound_events` contains a `Play{ sound: sound_hooks.Bump, key: None }`. See it fail.
- [ ] **GREEN:** convert the one-shot omit-markers to `push` — bump (`physics.rs`, worm.cpp:175/188),
  reload (`control.rs:471`, worm.cpp:309), bonus reload (`bonus.rs:533`, worm.cpp:833), respawn
  (`state.rs:2583`, worm.cpp:789), ninjarope (`control.rs:342`, worm.cpp:979), sobject variant
  (`sobject.rs:142`), wobject explo (`weapon.rs:744`), launch non-loop (`weapon.rs:204`), death-spray
  `15+rand(3)` (`state.rs:2706`), hit/blood `18+rand(3)` (`state.rs:2642`). Each carries the
  **already-computed** index (spec §3).
- [ ] **Isolation RED→GREEN:** test that `hash_game_state` is identical with `sound_events` populated vs
  cleared (hash-inert, spec §6.1); test that `rand.draws()` on a representative tick is unchanged vs the
  pre-4c count (spec §6.3). Per-family assertion tests for the emitted index/variant.
- [ ] Run `cargo test -p sim`; run the full determinism/oracle suite; `git diff --stat golden/` empty.
- [ ] Commit on `liero-rs-step-4`.

### T2 — Sim loop-sound events (play/stop) + leak model  [Opus]

- [ ] **RED:** write a failing test: firing a looping weapon emits `Play{ key: Some(WormWeapon(w,slot))
  }` on the fire tick, and ceasing fire / switching weapon / dying emits the matching
  `Stop{ key: Some(WormWeapon(..)) }`. See it fail.
- [ ] **GREEN:** emit the loop `Play` sites (worm.cpp:360/379/1120, weapon.cpp:311, nobject.cpp:183,
  sobject.cpp:108 → keys `Worm(idx)` / `WormWeapon(idx,slot)`) and the `Stop` sites (worm.cpp:341/375/
  1076, and the death loop-stop at `state.rs:2703-2707`). The sim emits `Play(loop)` every tick the
  loop should sound (no `IsPlaying` state in the sim — idempotency is the game's job, spec §3.4).
- [ ] Test the emitted keys are stable and that the death path emits the loop `Stop` (leak-parity).
- [ ] `cargo test -p sim`; determinism suite; `git diff --stat golden/` still empty. Commit.

### T3 — `AudioSink` trait + native rodio backend + sample table  [Sonnet]

- [ ] Add `rodio` to `game/Cargo.toml` under the `cfg(not(target_arch = "wasm32"))` target table.
- [ ] **RED:** in `game/src/audio.rs` define `trait AudioSink { fn play_one_shot(..); fn play_loop(key,
  sound); fn stop_loop(key); fn reap(live_keys); }`, a `NullSink`, and a `MockSink` (records calls).
  Write a failing test: draining a `[Play(None), Play(Some(k)), Stop(Some(k))]` event slice through the
  drainer drives the expected `MockSink` calls and leaves `loops` empty. See it fail.
- [ ] **GREEN:** implement the drainer (`HashMap<LoopKey, _>`: idempotent loop-start, stop-on-Stop) +
  the liveness reaper (spec §4.2); `RodioSink` loading `sounds[]` from `assets::WavSound::upsampled()`
  (mono, 44100 Hz) into `rodio::buffer::SamplesBuffer`; one `Sink` per `LoopKey`, `.repeat_infinite()`
  for loops, fire-and-forget for one-shots.
- [ ] Unit-test the leak model with `MockSink`: fire→cease→death leaves `loops` empty (spec §8 risk 1).
- [ ] `cargo test -p game`. Commit.

### T4 — Game wiring + MILESTONE (audible `--live`, isolation green)  [Sonnet]

- [ ] **GREEN:** in `main.rs::tick_and_render`, after `sim.0.process_frame(&inputs)` (`main.rs:420`),
  drain `&sim.0.sound_events` into the sink and call `reap` with the live worm/weapon keys read from
  `sim.0`. Construct a `RodioSink` in `setup` for native Live/Scripted; a `NullSink` for any headless
  entry. Ensure `replay_state_series` / round-trip / passthrough use **no** real sink.
- [ ] Confirm the 4b round-trip (`cargo test -p game`), the passthrough gate, and the debug
  determinism self-check are all still green; `git diff --stat golden/` empty.
- [ ] **MILESTONE — manual audible check (advisory, not a gate):** `cargo run -p game -- --live`
  produces the right sounds (fire loop starts/stops, bump, reload, explosion, death); the isolation
  gate is green. Record the check in the commit message. Commit.

### T5 — Wasm Web-Audio path + `sounds/` embed  [Sonnet]

- [ ] **GREEN:** in `scenario/src/assets.rs` add `include_dir!(".../sounds")` to the wasm branch (spec
  §8 wasm-embed; +~505 KB, no runtime fetch). Provide a wasm `AudioSink` impl (rodio/cpal Web-Audio;
  resume the `AudioContext` on the first user gesture; single-threaded worklet — spec §8 risk 3). If
  rodio's wasm path stalls, swap **this file only** to kira (the seam contains it, spec §5).
- [ ] Verify the debug wasm build still passes its render-parity + determinism self-checks (audio is
  additive; the scripted `blood` demo's sounds play — no live wasm input per 3f).
- [ ] Keep T5 independently revertable so a wasm-audio stall never blocks the native milestone. Commit.

### T6 — Broad final review + docs  [Opus]

- [ ] Full sweep: `cargo test --workspace`; the determinism/rollback/oracle suites; `git diff --stat
  golden/` empty; `cargo tree -p sim` / `-p render` / `-p scenario` show no `bevy*` and no `rodio` (it
  is `game`-only); `rand.draws()` + hash-inert tests green; the loop-leak test green.
- [ ] Confirm the isolation invariant holds native **and** wasm, and the 4b hard gate is unchanged.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md` (**4c line only**) and the overview's **4c**
  bullet to LANDED with commit range + what it proved. **Do not** touch any 4d content.
- [ ] Request a broad code review (superpowers:requesting-code-review) covering isolation, leak model,
  and backend seam. Commit.

## Oracle / done-when

- **Isolation gate (hard, standing):** `hash_game_state` byte-identical on every committed golden,
  native + wasm; `sound_events` hash-inert; `rand.draws()` unchanged; `git diff --stat golden/` empty.
- **4b round-trip (hard, inherited):** stays green — headless replay is silent.
- **Leak model (hard):** the `loops` map returns to empty after fire→cease→death (unit + integration).
- **Audible output (advisory):** the right sounds in `--live` (native) and the scripted demo (wasm) —
  eyeball/ear, never CI (audio nondeterminism).
- **Done when:** T1–T6 complete, the milestone (audible `--live` + isolation green) met, the broad
  review clean, docs updated (4c only).
