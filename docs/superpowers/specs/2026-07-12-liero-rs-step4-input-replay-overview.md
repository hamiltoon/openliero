# Step 4 — Input / `.lrp` replay / audio: overview / altitude decisions

Status: **OVERVIEW — Step 4 architecture/strategy** · 2026-07-13 · slices **4a–4g ALL LANDED — STEP 4 COMPLETE**
Part of: `2026-06-26-liero-rs-roadmap.md`
Detailing: the "Step 4 — Loop + input" section of `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md`
Built on: `2026-07-12-liero-rs-step4-cpp-input-replay-map.md` (C++ map, cited as **input-map §N**)
and `2026-06-26-liero-rs-interactive-iteration-exploration.md` (cited as **iter §N**)
Precedent template: `2026-07-10-liero-rs-step3-rendering-overview.md`

This is the Step 4 architecture/strategy decision document — one level more concrete than the
preliminary breakdown, **not** a per-slice spec and **not** a TDD task list. It locks the
cross-cutting decisions every slice inherits (the ggrs-ready input-snapshot model, the audio
isolation seam, where `.lrp` interop sits, the minimal-menu posture, the slice ordering and
oracle strategy) so each slice spec can be written against a stable foundation.

Steps 0–3 are complete and merged. The crown jewel is already in place: a Bevy-free `sim` crate
whose tick is **`SimState::process_frame(&[ControlState; N])`** — literally a per-tick
input-snapshot function — with `HashGameState` bit-exact vs C++ over long fuzzed runs; a pixel-exact
Bevy-free `render` crate (frame-hash gated); a native `game` binary that already ticks the sim on
`FixedUpdate` at the C++ cadence `1000/14 Hz` off a scenario's **recorded** per-tick inputs and
holds a debug per-tick determinism guard green over long loops (native + wasm); and a headless
`shot` CLI + in-repo `liero-shot` run-skill. Step 4 turns "plays a recorded scenario" into "plays
from real input, records/replays it, makes sound, shows shake/flash, and can read a real `.lrp`."

---

## Goal / done-when

Close the play loop: drive the (already-correct) sim from **real keyboard input** at the fixed
cadence, **record and replay** that input deterministically, **trigger audio** from sim events as a
pure side effect, wire the **live viewport** (shake/flash/banners), and read existing **`.lrp`**
replays byte-faithfully — all without perturbing the determinism firewall.

**Done when (ALL 6 MET — STEP 4 COMPLETE, 2026-07-13):**
1. ✅ **MET (4a, 4f).** A native `cargo run -p game` match is **playable from the keyboard**
   (1-player + 2-player hotseat), sim ticking at `1000/14 Hz` with exactly one input snapshot per
   tick — 4a delivered `--live`; 4f closed the loop by making the *bare* invocation (no flags) a
   playable default match.
2. ✅ **MET (4b).** A **record → replay round-trip is bit-exact**: a recorded live session, replayed
   headless, reproduces the identical `HashGameState` (and frame-hash) time series — *the* Step 4
   gate and the Step 5 precondition (breakdown §Step4 oracle; iter §3).
3. ✅ **MET (4c).** **Audio** plays from sim events (fire/explosion/bump/reload/…) with the Step 2/3
   `HashGameState` **provably unchanged** (isolation), native and wasm.
4. ✅ **MET (4d).** **Live shake/flash/banners** render in a real match (the `ProcessViewports` port
   Step 3 deferred).
5. ✅ **MET (4e phase 1) — phase-1 semantics.** A **real `.lrp` reads byte-faithfully** — proven by
   matching the C++ **`WideRollbackChecksum`** time series bit-exact over a committed `.lrp`
   corpus (the phase-1 C++-interop gate; a *harder* gate than `HashGameState` — it folds the whole
   rollback inventory, not just the tick's visible state). The `WideRollbackChecksum` diff
   **replaces** the framehash diff as the phase-1 proof, because phase 1 skips the cereal `Game`
   graph entirely (length-prefixed blobs let the reader skip them) and instead seeds the initial
   state from `scenario::load`. The original literal reading of this line — a C++ **`framehash`**
   diff driven by the full cereal-deserialized initial state — is **phase 2**, a bounded follow-on
   (possibly its own post-step item) — *scope-gated, see §Open Q1* — and does not block Step 4
   completion.
6. ✅ **MET (4g).** The in-repo **run-skill** drives the replay loop (`shot --scenario-path`) and CI
   runs the **replay-checksum regression** (iter §6) — delivered incrementally across 4a–4e and
   consolidated in 4g's SKILL.md §7 + CI-coverage inventory.

Presentation, live keyboard, and audio *output* are **not** bit-gated (they cannot be — see
Oracle strategy); the input→tick→state path **is**, via record→replay determinism and the `.lrp`
framehash diff.

---

## Locked decisions (inherited by every slice)

1. **The input model is already ggrs-shaped — do not re-home into `GgrsSchedule` yet.** The sim
   tick is `process_frame(&[ControlState; N])`: one immutable input snapshot per tick, a pure
   function of `(state, inputs)`. Step 4 only changes the **source** feeding that array (scripted →
   live/recorded/`.lrp`). Keep Bevy `FixedUpdate` + `Time::<Fixed>::from_hz(1000/14)`
   (already live, `rust/game/src/main.rs:107`). Moving the tick into `bevy_ggrs`'s schedule is
   **Step 5 slice 1's** re-homing job; pre-adopting it now buys nothing because the snapshot
   boundary — the real prerequisite — already exists. (Resolves breakdown §Step4 open-Q "FixedUpdate
   vs ggrs cadence".)
2. **One input snapshot per tick, sampled deterministically from accumulated key state.** Mirror
   C++'s level-triggered model (input-map §1a): live key edges update a per-worm `ControlState`;
   at each tick that `ControlState` is snapshotted into the `process_frame` array. No sub-tick
   sampling, no render-rate coupling. `Dig` expands to a Left+Right chord at snapshot time
   (input-map §1a); it is never a stored bit.
3. **Audio is a pure consumer of a sim-emitted event stream — zero new `rand`.** The RNG draws that
   *select* sound variants are already in the sim (input-map §6); Step 4 adds only an event
   *record* (id / variant / object handle / loop) drained by the game layer. `speculative`
   (predicted/resim ticks) suppresses draining, never a draw. Audio lives in `game` (or a tiny
   sink crate), never in `sim`/`render`. The Step 2/3 `HashGameState` isolation invariant is a
   standing gate, exactly as in Step 3.
4. **`.lrp` *reading* is byte-faithful; everything else may modernise.** Per charter: reading an
   existing `.lrp` must reproduce C++ playback bit-for-bit (container, delta stream, embedded
   `WideRollbackChecksum`, version legacy branches — input-map §3). The *recorded-input artifact we
   author ourselves* (the round-trip format) is free to be a clean Rust-native format — it need not
   be `.lrp`.
5. **Minimal start flow only.** Just enough to launch a match, respawn, and quit/restart — no full
   menu/weapon-select tree (that ~2–3k-LoC `*State.cpp` render surface stays a Step-3 deferral).
   See §Open Q5.
6. **One accumulating PR.** All slices 4a–4g land on branch `liero-rs-step-4`, merged when the step
   is done — same pattern as Steps 2 and 3.

---

## Oracle / verification strategy

The tension Step 4 must resolve: **most of what it adds is inherently un-gateable against a state
oracle** (live keyboard, audio output, window presentation). The strategy is to route every
sim-affecting path back onto the existing bit-exact gate, and prove the un-gateable paths by
*isolation* and *round-trip determinism*.

- **Hard gate 1 — record→replay round-trip (the Step 4 headline).** Sample live/scripted input,
  record the per-tick `ControlState` stream, replay it headless, and assert the identical
  `HashGameState` **and** frame-hash time series (reusing the Step 2/3 harness and the `shot`
  driver). This is "determinism survives real input" made objective, and it is the Step 5
  precondition (iter §3, breakdown §Step4). It needs **no C++** — it is self-checking.
- **Hard gate 2 — `.lrp` vs C++ `framehash` (the interop gate).** Feed a committed `.lrp` through
  the Rust reader and through C++ `framehash` (`CMakeLists.txt:552`, the existing `.lrp`→per-frame
  FNV player) and diff the per-tick frame-hash + `total`. This is the one place Step 4 touches C++
  again. **There are no `.lrp` files in the repo** (verified: `find -iname '*.lrp'` is empty), so a
  small corpus must be *generated* — see §Open Q2.
- **Isolation gate (standing).** Every slice re-asserts the Step 2/3 `HashGameState` is byte-
  identical after input/audio/viewport wiring — audio and viewport are render-only/side-effect-only
  and must not perturb the sim (input-map §6, §7). This is the Step 3 isolation discipline continued.
- **Advisory — screenshot/audio-output review.** Live frames and sound are eyeballed/heard via the
  native window and the `shot` PNG loop; never a CI pass/fail (GPU/audio nondeterminism), exactly
  as iter §3 prescribes.

Format note: the recorded-input artifact should be the *same* object that (a) drives the round-trip
regression, (b) drives the `shot` screenshot loop, and (c) later feeds Step 5 rollback tests — "one
artifact, four uses" (iter §3). The scenario `input <tick> <w0> <w1>` grammar
(`rust/scenario/src/parser.rs:18`) is already exactly this; 4b extends it (or adds a compact binary
sibling) rather than inventing a parallel format.

---

## Slice ordering (4a–4g)

Each slice accumulates on `liero-rs-step-4`, states its done-when and what it *proves*. Ordering is
**testability-first**: stand up the deterministic input path and its round-trip gate (4a→4b)
*before* the inherently un-gateable surfaces (audio 4c, live viewport 4d, menu 4f), so the hard gate
exists before the soft surfaces are layered on. `.lrp` interop (4e) is sequenced late because it is
the largest surface and depends on nothing the earlier slices produce.

- **4a — Live input core. LANDED (2026-07-12, commits `072817b`/`642adeb`/`e189de2`/`d9a54a3`,
  all reviewed READY 0 Critical/0 Important).** Bevy keyboard → per-worm `ControlState`,
  snapshotted once per `FixedUpdate` tick into `process_frame`; a new `InputSource{Scripted,Live}`
  replaced the inline `scenario.input(t, w)` feed (`rust/game/src/main.rs`, pre-4a `:257-261`) —
  Scripted is a verbatim pass-through, Live polls `ButtonInput`; `--live [name]` CLI (`Mode` enum,
  native-only); `Dig`→Left+Right chord (a pure OR, never a stored bit); default bindings decoded
  from `settings.cpp:36-37` via `keys.cpp:9-58` (P0 R/F/D/G+LCtrl/LShift/LAlt, P1 arrows+RCtrl/
  RAlt/RShift, Dig unbound both, index-for-index verified). The debug determinism self-check and
  the loop/reload are gated scripted-only (kept live for the regression path, off for `--live`).
  **Proved:** the game is playable from the keyboard, native, 1P+2P hotseat, one snapshot/tick, no
  render-rate coupling; Esc quits. **Oracle/gate — MET:** the headless pass-through determinism
  gate (`rust/game/tests/passthrough.rs`) drives the real `InputSource::Scripted` through all
  **7** committed `render_slice3b_*` scenarios and asserts `hash_game_state` bit-exact per tick vs
  the golden `state_hash` column, wired into CI via `cargo test -p game` (3c's `cargo build -p
  game` step upgraded). `sim`/`render`/`scenario` stayed byte-unchanged; every golden re-diffs
  empty. **Risk resolutions:** level-triggered sampling (once per tick, before `process_frame`) —
  confirmed the central invariant, RED-proven by an off-by-one repro (diverged at tick 2) before
  the fix; `Dig` chord and default-table mismatches — caught by a 14-index-for-index review pass.
  **Open-question resolutions carried forward:** sampler lives in a new Bevy-free-testable
  `game/src/input.rs` (not re-homed into a resource-accumulator or `Update`); the default run mode
  stays scripted (`--live` is opt-in, matching Open Q4's minimal-menu posture — no menu was needed
  to reach playability); live-mode camera is unchanged (fixed ×3, no follow-cam — that Step-3
  deferral stands). **Finding:** focus-loss needed **no** backstop — Bevy 0.19 already releases
  held keys on window-focus-loss (`bevy_winit/src/system.rs:133-173` `check_keyboard_focus_lost` →
  synthetic `Released` events → `bevy_input::keyboard.rs:199-203` `release_all`), verified by
  reading the Bevy source rather than added defensively. **Manual playability (John's 30-second
  keyboard check) remains an open advisory** — not yet run, same posture as 3c's Srgb eyeball
  check; not a gate. **Deferred:** live-wasm (browser keyboard input — wasm stays on the scripted
  witness per 3f, browser input revisited later); gamepad (unchanged, still later/optional);
  recording → **4b**.

- **4b — Record / replay round-trip + regression (the hard gate). — LANDED (2026-07-13, commits
  `8bf17cc`..`7936ce3` + hardening `4c73119`; all reviews 0 Critical / 0 Important).** Delivered as
  specified, with the format decision made literal: **the recorded artifact IS a scenario file** —
  `Scenario::to_text` (the crate's first serializer, every parsed field covered) +
  `with_recorded_inputs`; a `Recorder` taps the sampled `ControlState` array (Dig chord already
  resolved) and flushes on `AppExit` ordered `.after(bevy::window::ExitSystems)` (review-caught
  window-close race); `--replay <path>` drives any scenario through the **unchanged**
  `InputSource::Scripted` (play once, then hold); `replay_state_series` is the headless library
  entry. The round-trip gate (`round_trip.rs`) proves live-sampler-record → text → parse → scripted
  replay reproduces the live `HashGameState` series tick-for-tick, non-vacuously (3 asymmetries +
  hardcoded raw-text facit); a committed 12-tick corpus + sim-produced sidecar backstops
  symmetric drift. **Risk resolutions:** the `prev_control_states` concern was a non-issue (the
  Rust sim stores absolute per-tick words, no delta baseline — delta applies only to 4e's `.lrp`).
  **Hardening (H1):** the T3 review found the C++ `cossin[128]` UB **reachable in `--live`**
  (spawn→walk→fire; the aim clamp is gated on `aiming_speed != 0`) — three sim sites (`worm_fire`,
  ninjarope-throw, dig) now mask the table index `&0x7f` (3b precedent, same table; golden-neutral,
  full suite green).

- **4c — Audio. LANDED (2026-07-13, commits `5f10312`/`966c362`/`59b6fef`/`1ffc68e`/`2b8fc6c`,
  all reviews READY/MILESTONE-READY 0 Critical/0 Important).** Delivered as specified: a Bevy-free
  `sim::sound` module records `Play`/`Stop` at all 10 `ProcessFrame` callsites via a thread-local
  per-frame collector, drained into `SimState.sound_events` at the tick's tail —
  **adding zero `rand`**, `hash_game_state` byte-identical (isolation proven structurally, not by
  a runtime flag). **Classification finding (T2):** re-reading C++ `SoundPlayer::Play(int,
  void* id = nullptr, int loops = 0)` (`player.hpp:15`) against every callsite found `loops`
  defaults to `0` — only `worm.cpp:1120-1121` passes an explicit `loops = -1`. It is the **only
  true loop** `ProcessFrame` reaches (`WormWeapon`-keyed, 3 verbatim `Stop` sites); the slice
  design's other 5 "worm-keyed loops" were `loops=0` one-shots deduplicated only by the caller's
  `IsPlaying` guard — corrected in the design spec and in `sim/src/sound.rs`'s `LoopKey::Worm` doc.
  Per John's Väg-A call, the hit/blood trio is emitted as `key=None` one-shots (correct sound,
  C++'s restart-dedup lost — an audio-advisory-only difference). `game` drains the stream through
  a new `AudioSink` trait (`RodioSink` + `NullSink`, mirroring C++ `SoundPlayer`) backed by
  `rodio` 0.19 (`default-features=false`); an idempotent `Drainer` reproduces the C++ `IsPlaying`
  gate per loop key plus a belt-and-braces liveness reaper (closes the worm-death channel-leak
  edge). **MILESTONE (T4):** wired live in windowed play — `RodioSink` behind a Bevy `NonSend`
  resource (`cpal::Stream` is `!Send`), falls back to a silent `NullSink` instead of crashing on
  init failure; headless stays structurally silent (no sink constructed). **Wasm (T5):** the
  *same* `RodioSink` covers wasm too — the `wasm-bindgen` cargo feature routes `cpal` to its
  WebAudio backend, no `kira` fallback needed after all; `sounds/` (~505 KB) embedded via the
  existing `read_asset` seam; autoplay stays silent until a user gesture, no panic (one
  documented native/wasm asset-miss inconsistency: `load_sound_table_wasm` panics on a missing
  embedded file where native silently leaves the slot empty). **Proved:** a match makes the right
  sounds, native and browser; the sim is untouched. **Oracle/gate — MET:** `HashGameState` stayed
  byte-identical on every golden; output is advisory (John's ear-check, not yet run — same
  posture as prior eyeball checks). **Risk resolutions:** backend choice (§Open Q3) resolved to
  `rodio` for both targets, no `kira` needed; loop-channel leaks (input-map §6, `Stop` not
  speculative-gated) closed by the explicit C++ Stops **and** the liveness reaper; wasm
  audio-context init needs a user gesture as expected, handled without panic.

- **4d — Live shake / flash / banners (the `ProcessViewports` port). LANDED (2026-07-13, commits
  `370be59`..`bf36497`, all reviews READY/MILESTONE-READY 0 Critical/0 Important).** Delivered as
  specified, with one correction that reshaped the slice: the overview and input-map §7a/§8 called
  `screen_flash` **"already-hashed sim state"** — **wrong**. C++ `HashGameState` does not fold
  `screen_flash` (`stateHash.hpp` omits it; it lives only in the rollback `GameSnapshot`), so adding
  it to `SimState` (top-of-frame decrement `game.cpp:271-273`, sobject-create max-write
  `sobject.cpp:41`) is **hash-neutral** — gated by a re-diff of every prior golden, not a re-fuzz.
  `shake` follows the same shape as 4c's audio event stream: the sole writer (`sobject.cpp:31`)
  emits a **raw** blast-coordinate + amount event, and the game layer (not the sim) does the
  `itof`/rect-test/max — keeping the sim itself viewport-blind, as no viewport concept exists inside
  the firewall. The game-layer live-stepping core (`viewport_step.rs`) reproduces C++'s
  interleaving around one atomic `process_frame` call: decrement-without-floor on **raw** pre-tick
  values + banner-walk on **pre-frame** `killed_timer`/`cycles`, **then** `process_frame` (which
  applies the sim-side `screen_flash` decrement and emits the shake events), **then** apply the
  shake max, **then** `frame::draw`'s `Viewport::process` (centering + shake-RNG + clamp on
  **post**-frame state) — matching the real `ProcessViewports` call site (`game.cpp:463`, after the
  worm loop) with no reader running between the decrement and the max. A new C++ dumper directive,
  `render_live`, runs the **full** `ProcessFrame` with wired viewports (the reduced dumper never
  did either) to produce a golden whose flash/shake/banner/centering evolved from real explosions
  and a real spawn/death; a mandatory re-diff proved every prior golden stayed byte-identical.
  **MILESTONE (T4):** `render_slice4d_live` matched **first run** — 321 ticks bit-exact across a
  death→respawn window; the camera moves live as the respawn worm falls (the `SetCenter` arm is
  reached via a fire-triggered spawn, solving the design doc's `killed_timer`-vacuity trap); triple
  isolation held throughout, backed by four non-vacuity witnesses (flash decaying to zero, shake RNG
  jitter, the banner range walk, live centering itself) ruling out an accidental all-static pass.
  The harness is a documented hand-copy of `viewport_step.rs` (Bevy's `!Send`/ECS shape forces the
  duplication), double-anchored against drift. **T5** added death-banner **text** via the 3e font — a
  genuinely cross-viewport effect (the dying worm's banner draws in the *other* player's viewport,
  `viewport.cpp:256-270`). **Finding + fix:** `frame::draw` was interleaving process-then-draw **per
  viewport**, which diverges from C++'s process-**all**-viewports-first (`ProcessViewports`); the
  refactor to match is hash-neutral (each viewport's RNG stream is fully local — the 3a/3b/3e
  goldens are unchanged), and the 4d golden regen touched only the frame-hash column, only inside
  the death window (ticks 93–236) — the state-hash column stayed byte-identical. **Proved:**
  explosions flash and shake, death banners appear, the camera follows a live worm, in a real
  (dumper-driven) match. **Oracle/gate — MET:** the `render_slice4d_live` golden, triple-isolated
  per tick. **Risk resolutions:** `steerable_sum_x/y` — **resolved DEFER**, not the leaning-add
  premise this bullet originally recorded: `ProcessSteerables` mutates the **hashed**
  `wobject.cur_frame` and is unported, and no committed scenario reaches a steerable weapon, so the
  accumulators would be vacuous without also authoring a steerable-weapon scenario + golden +
  re-fuzz; the non-steerable `SetCenter(Ftoi(pos))` plus its `debug_assert_eq!(steerable_count, 0)`
  live guard stand instead. Banner stepping is confirmed every-other-cycle (`cycles & 1`), matching
  C++. **Deferred:** the `YoureIt`/GameOfTag banner arm (a comment, not a hard tripwire — routed to
  whichever later slice needs GameOfTag presentation); a spec-flagged minor — the harness hand-copy
  is a theoretical frame-level blind spot, mitigated (not eliminated) by double-anchoring.

- **4e — `.lrp` byte-faithful reader (the C++-interop gate). Phase 1 LANDED (2026-07-13, commits
  `fb9b9b7` plan/`d0cca91` T0/`0a08d54` T1/`b5fb884` T2/`cd711fc` T3/`b8bd8cb` T4, all reviews
  READY 0 Critical/0 Important); phase 2 (cereal `Game` graph) is a bounded follow-on.** Delivered
  per the §Open Q1 split recommendation, with the split boundary sharper than proposed: the
  **phase-boundary finding** is that the `.lrp` container's per-worm cereal blobs are
  **length-prefixed**, so phase 1 doesn't need to parse them at all — it **skips** the whole
  cereal `Game` graph via its length prefix and seeds the initial sim state from the existing
  `scenario::load` instead. That makes **`WideRollbackChecksum`** (folded every 1050 cycles over
  the *entire* rollback inventory — 21 worm fields + pools + material buffer +
  `prev_control_states` + terrain) a strictly **harder** gate than `HashGameState`, so it stands in
  as the phase-1 oracle in place of the originally-envisioned `framehash` diff (see done-when §5
  above). **T0** built `lrp_gen`, a C++ headless replay-writer (setup verbatim from the existing
  dumper), and a two-fixture corpus (320t + 1120t, checksum words at cycle 0 and 1050), proving
  tick-0 alignment. **T1** ported `WideRollbackChecksum` itself (Mix32 exact, all 21 worm fields +
  pools + material buffer, `prev_istates` caller-supplied) — the cycle-0 facit (`0x031057d7`)
  matched on the first run. **T2** built the Bevy-free `rust/replay` crate (`flate2`, no Bevy): the
  big-endian container, cereal-blob skip via its length prefix, tags `0x80`–`0x83`, the XOR-delta
  decode, checksum extraction — verified against all 1120×2 input words via the scenario grammar
  (an XOR-baseline caveat — mutating fixtures — documented and routed to phase 2). **T3
  MILESTONE:** the phase-1 gate went green on the first run — 1120 ticks replayed bit-exact
  including the cycle-1050 checksum word (`0xf6211087`), backed by a per-tick tripwire against a
  fresh sidecar and a negative (mutated-word) test. **T4** hardened the reader against hostile
  input (10 tests — no panic on garbage, `O(1)` `Err` on a huge length-prefix, etc.) plus
  `debug_assert`s, and formally booked phase 2 (spec §8: the cereal graph / a version-<7 lift
  [**adjudicated: rejected for phase 1** — the only version-<7 difference inside the cereal blob is
  palette-only] / `decode_with_baseline` / the `framehash` diff). **Proved:** the rewrite can
  consume authentic Liero replays byte-faithfully for the container/stream/checksum surface.
  **Oracle/gate — MET (phase 1):** hard diff vs the C++ `WideRollbackChecksum` time series over the
  generated corpus. **Deferred:** phase 2 in full — the cereal `Game` graph deserialization and the
  `framehash` diff it would enable, per §Open Q1's scope-gate (possibly its own post-step item).

- **4f — Minimal start flow. LANDED (2026-07-13, commits `80cb75c` plan/`7146490` T0/`41a1af2`
  T0-fix, T0-review NEEDS-FIX → fixed by 2 Important-severity implementer fixes → clean).**
  Delivered as specified, sharper than the "new match with defaults" framing: bare
  `cargo run -p game` (no flags) now starts a **playable** default match — Live mode, a new
  `default_match` fixture on the `render_stage` level (two visible worms on blood-bevised
  positions, DART, seed 42; the fixture lives outside the golden dir so it cannot perturb any
  golden enumeration). F5 restarts the match — adjudicated to also zero an in-flight recording
  (restart means restart, not "restart but keep recording state"). **Proved:** a match can be
  started and restarted without editing code or passing flags. **Finding + fix:** a positional
  `default_match` argument was initially accepted ambiguously alongside the named fixture path —
  the T0 review caught it NEEDS-FIX; the fix rejects the positional form cleanly. The window title
  was also tidied. The wasm arm stays byte-unchanged (the parity witness holds). **Oracle/gate —
  MET:** smoke (headless launch) + the 4b round-trip gate stays green. **Deferred:**
  `GenerateFromSettings` / random-level generation (§Open Q5's minimal-menu posture holds — no
  menu was needed to reach a playable default).

- **4g — run/verify skill + CI regression wiring. LANDED (2026-07-13, commits `f566404` T1/
  `e22c90a` T2, both self-verified and covered by the broad step-4 review).** Delivered as
  specified: `shot` gained `--scenario-path <file>`, rendering any recording or arbitrary
  scenario file (not just the golden corpus) — the golden path itself stays byte-identical, 19
  `shot` tests green. `SKILL.md` gained a new §7 covering live play + the full key-binding table,
  the record→replay loop (including the flush caveat: Esc/window-close flushes the recording, a
  killed process writes nothing, F5 zeroes an in-flight recording), headless verification, and a
  CI-coverage inventory showing every Step 4 gate is already wired into CI. **Finding:** the CI
  half of 4g was in fact delivered incrementally across 4a–4e (each slice wired its own gate into
  CI as it landed) — 4g's own diff is thin by design, consolidating documentation rather than
  adding new CI surface. **Proved:** the agent/CI loop covers input+replay end-to-end.
  **Oracle/gate — MET:** the CI jobs themselves (all pre-existing, now inventoried).

**STEP 4 = COMPLETE.** All slices 4a–4g landed; remaining for John: the PR #5 merge decision and
the `.lrp` phase-2 (cereal `Game` graph) follow-on decision (§Open Q1) — both bookkept, neither
blocking.

*4c and 4d are mutually independent and may be built in either order or in parallel; both depend on
4a (a live loop to attach to) but not on each other. 4g is woven in incrementally as each gate
lands, not saved for the end.*

---

## Deferrals (explicitly out of Step 4 scope)

- **`GgrsSchedule` / rollback / netplay** — Step 5 (the input snapshot boundary is the only Step-5
  prerequisite, and it is delivered here).
- **Full menu / weapon-selection / `*State.cpp` flow** — the large render+flow surface stays a
  Step-3 deferral; 4f delivers only a minimal start (§Open Q5).
- **`.lrp` *writing*** — Step 4 needs `.lrp` *reading* (charter) + a Rust-native record format
  (4b). Producing `.lrp` is only needed to generate the test corpus (§Open Q2) and can be a C++-side
  or minimal-writer task, not a full port.
- **Gamepad / joystick input** (`gfx.cpp:651-720` axis/button mapping) — keyboard first; gamepad is
  additive and un-gated, defer unless a target needs it.
- **Weapon-select key-repeat emulation** (`localController.cpp:124-152`) — only if 4f ports weapon
  selection, which it should not.
- **Modern color mode, spectator renderer, render interpolation, follow-cam** — carried from Step 3
  deferrals; unrelated to input/audio.
- **Steerable-centering camera** — may ship in 4d if the two `steerable_sum` accumulators are added;
  otherwise deferred with the non-steerable centering assert kept.

---

## Open questions for the controller to adjudicate (with recommendations)

1. **`.lrp` initial-state scope — full cereal `Game` parse, or split?** The `.lrp` container's
   initial state is the entire `Game` cereal graph (input-map §3a) — the single largest surface in
   Step 4. **Recommendation: split 4e.** Land 4a–4d (the playable, self-gated core) first. In 4e,
   deliver the **container + delta stream + `WideRollbackChecksum` reader** against a Rust-produced
   initial state first (gate the *stream* semantics), then port the cereal `Game` deserialization as
   a bounded follow-on gated by C++ `framehash`. Honor the charter's "byte-faithful reading" for the
   container/stream immediately; scope the cereal graph explicitly rather than letting it blow up the
   slice. If cereal `Game` proves larger than the rest of Step 4 combined, it becomes its own
   post-step item — but the container/stream reader ships in Step 4.

2. **Where does the `.lrp` test corpus come from (there are none in-repo)?** Verified: no `.lrp`
   exists in the worktree; C++ `framehash` is the reference player. **Recommendation:** generate a
   small, committed corpus from C++ — the cheapest path is a tiny C++ dumper mode that drives a
   `LocalController`/`ReplayWriter` over a scenario's recorded inputs (reusing the Step 2/3 scenario
   corpus so the same input vectors already have sim goldens), producing deterministic `.lrp`
   fixtures. Run both C++ `framehash` and the Rust reader on them. This also transitively validates
   4b's Rust-native format against the C++ `.lrp` semantics.

3. **Audio backend — bevy_audio, rodio, or kira? And wasm?** Step 3 deliberately dropped Bevy's
   audio feature (3c feature list). **Recommendation:** a thin, Bevy-free audio sink using **kira**
   (mixing + loop handles + web support) or **rodio** (simpler), chosen at 4c start with a fresh
   check, living in a small `audio` module in `game` (or a tiny `audio` crate) — *not* re-enabling
   `bevy_audio`, to keep `game`'s Bevy surface minimal and the audio path swappable. The sim's
   sound-event stream is backend-agnostic; the backend is a pure consumer. Confirm the wasm (Web
   Audio) path in the same slice, since 3f already runs in the browser.

4. **How much menu is in scope?** **Recommendation: the minimum that makes single-player a loop** —
   a "start match with default level+weapons" entry + respawn + quit/restart (4f). No main-menu
   tree, no interactive weapon selection, no file/profile UI. Full menus are a large deferred render
   surface; pulling them in would dwarf the actual Step 4 goal (playable input + replay).

5. **Does live shake/flash need a sim-state change?** Steerable-camera centering reads
   `steerable_sum_x/y`, which C++ `Worm` has (`worm.hpp:267`) but Rust `WormState` does not
   (input-map §7b). **Recommendation:** in 4d, add the two `steerable_sum` accumulators to the sim
   state (they are set in the steerables loop; small, hashed, re-fuzz against C++) so steerable
   centering is faithful — *or*, if it risks the isolation budget, keep the current non-steerable
   `SetCenter` on `Ftoi(pos)` and defer steerable centering with the existing assert. Everything
   else in `ProcessViewports` (shake RNG, banners, flash) is render-only and needs no sim change.

6. **One recorded-input format, or two (round-trip vs `.lrp`)?** **Recommendation: two, by design.**
   The self-authored round-trip artifact (4b) is a clean Rust-native format extending the scenario
   `input` grammar (or a compact binary sibling) — optimized for the regression + `shot` + Step-5
   reuse. `.lrp` (4e) is the *foreign* format we only *read*, byte-faithfully, for C++ interop.
   Conflating them would force the round-trip format to carry cereal-`Game` baggage it does not need.

---

## Risks & the hard 10%

- **Input sampling timing (the central risk).** Exactly one snapshot per tick, deterministic,
  decoupled from render rate; sub-tick or render-coupled sampling breaks replay and Step 5
  (input-map §1a, breakdown §Step4). The `process_frame(&inputs)` boundary already enforces this —
  the risk is the *sampler* that fills `inputs` leaking render-rate or OS-key-state coupling.
- **`.lrp` cereal surface.** The whole-`Game` deserialization is the largest single unknown; §Open
  Q1's split contains it.
- **Audio as a silent RNG leak.** The one non-negotiable: audio must add **zero** `rand` and touch
  no hashed state. The sim already draws the variant-selecting `rand` (input-map §6) — the trap is
  a "helpful" refactor that moves a draw into the audio path. The isolation gate catches it.
- **Loop-sound channel leaks.** C++ `Stop` is not speculative-gated for a reason (input-map §6);
  the Rust looping-sound model must key by object handle and stop on the same events, or leak
  channels — an output bug, not a determinism bug, but a real one.
- **Second checksum (`WideRollbackChecksum`).** `.lrp` playback needs this distinct hash on an exact
  1050-frame cadence (input-map §5); a Rust reader that only has `HashGameState` will desync at
  frame 1050.
- **Focus-loss / held keys.** Trust the recorded/edge stream, not re-read OS key state, across focus
  boundaries (input-map §1d).
- **Determinism guard retirement.** The 3c/3f debug per-tick guard assumes recorded input; live play
  has no golden. Keep the guard for scripted/replay runs, drop it for live — do not let live play
  silently disable the regression path.

---

## Next artifact

The first slice's detailed spec (companion document, to be written when 4a starts):
- `specs/2026-07-12-liero-rs-step4-slice4a-live-input-design.md`
</content>
</invoke>
