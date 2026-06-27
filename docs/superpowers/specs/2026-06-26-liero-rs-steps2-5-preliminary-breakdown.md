# Liero-rs — steps 2–5: PRELIMINARY / BASE-LEVEL breakdown

Status: **PRELIMINARY / BASE-LEVEL — to be detailed just-in-time per the charter** · 2026-06-26
Part of: `2026-06-26-liero-rs-roadmap.md`

> **Read this first.** This is *not* an implementation plan and *not* a spec. It
> is an altitude document that sits **between the roadmap and the just-in-time
> per-slice specs**. The roadmap is explicit: *"Steps 2–5 are detailed-specced
> just-in-time; we understand them better after steps 0–1."* So every breakdown
> below is **provisional** and **will be revised** when its step is actually
> reached. It deliberately contains **no TDD task lists and no implementation
> code** — its job is to map the terrain, surface risks / dependencies /
> unknowns, and propose a *plausible* slice ordering. One level more concrete
> than the roadmap, no more. When a step is reached, write its real spec then.

## What exists today (the foundation steps 2–5 build on)

- **`sim-core`** (Bevy-free, no floating point): `fixed` (16.16), `vec` (`IVec2`),
  `math` (integer sqrt / vector length), `tables` (precomputed sin/cos), `rng`
  (ported MT19937 with restorable state). This is the locked, bit-exact base.
- **`assets`** (Bevy-free): `LevelData` + material map (1a/1b, what step 2 needs
  first), palette/display (1c), sprites (1d), TC bundle — constants `c[]`, flags
  `h[]`, weapon / nobject / sobject defs, sounds (1e).
- **`oracle-tests`**: the golden-vector pattern — a C++ dumper emits a golden
  file; a Rust test reproduces it bit-for-bit. Per-slice `gen_*_golden.sh` +
  `golden/*.txt` + a `tests/*.rs`.
- **C++ oracle internals worth knowing:**
  - `Game::ProcessFrame()` (`src/game/game.cpp:267`) is the canonical tick. Fixed
    order: screen-flash / viewport shake → bonuses → banner → **sobjects →
    wobjects → nobjects → bobjects** → `++cycles` → bonus-drop RNG roll → **worms
    → ninjaropes** → game-mode logic → viewports → store prev controls.
  - Per-entity `Process()` methods: `worm.cpp` (incl. `ProcessPhysics`, `Fire`),
    `weapon.cpp` (`WObject::Process`), `nobject.cpp`, `sobject.cpp`, `bonus.cpp`.
  - `HashGameState` / `HashGameComponents` (`src/game/stateHash.hpp`) — the
    per-frame checksum and per-component diagnostic hashes, already the
    determinism oracle for `test_determinism.cpp`.
  - `GameSnapshot` / `fast_snapshot.hpp` — exactly which fields are sim state
    (the rollback-relevant set): worm `WormSimState`, object pools, `rand`,
    `cycles`, level `material_id` (+ dirty `display_valid`), holdazone. This is a
    ready-made **inventory of the state step 2 must reproduce and step 5 must
    roll back.**

---

## Step 2 — Sim core in ECS

### Goal / done-when
Port `ProcessFrame` and the per-entity `process()` logic into the Rust/Bevy ECS
so that, given identical seed + level + per-frame inputs, the Rust sim's
`HashGameState`-equivalent **matches C++ tick by tick** over a long run (the
roadmap's "bullet fired → moves → explodes → destroys terrain, checksum matches
C++"). This is the crown-jewel step: it is where determinism is won or lost.

### Provisional sub-slice breakdown
A plausible *thin-vertical-then-widen* ordering, each piece independently
differential-testable against a C++ component hash:

1. **Level → ECS + checksum harness.** Load `LevelData` into the world; reproduce
   the level material-map hash. Stand up the Rust-side state-hash that mirrors
   `HashGameState`. No dynamics yet — proves the harness and the level half of
   the checksum before any motion exists.
2. **One worm, physics only.** Port `Worm::ProcessPhysics` (gravity, terrain
   collision against the material map, position/velocity in fixed-point) for a
   single worm with scripted inputs; match worm `pos/vel` component hash.
3. **Worm control + aiming.** The rest of `Worm::Process` minus combat: movement
   keys, aim angle/speed, jump/dig, direction. Match the full worm hash.
4. **One weapon, full lifecycle.** `Worm::Fire` → spawn a `WObject` →
   `WObject::Process` (move, collide, explode) → terrain destruction → resulting
   `SObject`/`NObject`. Pick the simplest projectile first. This is the roadmap's
   headline bullet-lifecycle milestone.
5. **Remaining object families.** `nobject` (incl. splinters), `sobject`
   (explosions / blast → terrain + worm damage), `bobject` (blood), `bonus`
   spawn/pickup. Each added against its component hash.
6. **Full `ProcessFrame` integration + game-mode logic.** Wire the entities in
   the **exact C++ order**, add `cycles`, the bonus-drop RNG roll, ninjarope,
   damage/death/respawn, and at least Kill-em-all mode. **Milestone:** full
   `HashGameState` matches C++ for N>1000 ticks under fuzzed inputs (mirror of
   `test_determinism.cpp`'s 2-worm 1000-frame loop).

### Oracle / verification strategy
- **Reuse the per-frame checksum as the differential oracle.** Extend the golden
  pattern: a CMake-gated C++ harness (the `OPENLIERO_BUILD_ORACLE_DUMP` lineage
  from step 1, linking the `game` lib) runs a fixed scenario — seed, level,
  scripted/fuzzed per-frame input stream — and dumps **per-tick** `HashGameState`
  *and* `HashGameComponents` (rng, level, worms, b/n/s/wobjects, bonuses). Rust
  runs the same scenario and must reproduce every line.
- **Per-component hashes are the debugging superpower:** when a tick diverges,
  the component hash says *which subsystem* and the existing C++ deep-compare
  diagnostics (see `test_determinism.cpp`) show the field. Build the Rust harness
  to emit the same component breakdown.
- **This extends the existing golden pattern but needs a new, richer harness:**
  step 1 dumped static parsed bytes; step 2 must dump a *time series* of hashes
  driven by a shared input vector. The input-stream format and the scenario set
  (which seeds/levels/inputs) are new artifacts to design when the step starts.
- Snapshot field inventory in `fast_snapshot.hpp` defines the "what counts as
  state" set the Rust hash must cover — use it as the checklist.

### Key risks & the hard 10%
- **The Bevy trap (the central risk).** Bevy schedules systems in parallel and in
  unspecified order; `ProcessFrame` is a strict sequence with read-after-write
  dependencies (sobjects before wobjects before nobjects before bobjects, then
  worms, then ninjaropes). The sim must run in an explicitly **ordered, single
  schedule** (the future `GgrsSchedule` shape, even before ggrs lands) with no
  reliance on parallelism. Decide early whether the sim is "ECS-native systems
  with hard ordering" or "sim-core functions called from one driver system." The
  latter de-risks determinism at the cost of less idiomatic ECS — an open design
  call (see open questions).
- **RNG as ordered, shared state.** `rand` is consumed mid-frame (bonus-drop
  roll, fire spread, splinters, respawn search). Call order must match C++
  exactly; a single extra/missing/reordered `rand()` call desyncs everything
  downstream. `rng` lives in `sim-core` and must be threaded through in C++ order
  — not pulled ad hoc from systems.
- **Fixed-point / no-float discipline everywhere.** Any `f32` leaking into
  physics is a latent cross-machine desync. The sim half must stay in `sim-core`
  types; Bevy's `Vec2`/`Transform` (float) are rendering-only and must not feed
  back into the sim.
- **Iteration order over object pools.** C++ uses pool iterators with stable
  order and free-list semantics; ECS query iteration order is not guaranteed.
  Entity spawn/despawn order and iteration order must be made deterministic
  (explicit ordering key, or keep the pool model inside sim-core).
- **Death/respawn + level-dependent RNG search** (`beginRespawn`) is a known C++
  desync-sensitive path — port it carefully and fuzz it (the existing death fuzz
  test is the template).

### Dependencies
- Needs `sim-core` (done) and `assets` 1a/1b material map (done). Palette /
  sprites / sounds are **not** needed (rendering/audio is step 3).
- Needs the TC object/constant data from 1e (weapon/nobject/sobject params, `c[]`
  / `h[]`). Confirm 1e coverage is sufficient before slice 4.
- No Bevy-rollback dependency yet, but **system-ordering discipline established
  here is the prerequisite for step 5** — design step 2 as if ggrs is coming.

### Open questions to resolve before detailed planning
- ECS-native systems vs. sim-core-driver-system architecture for the tick?
- How are pooled objects modeled in ECS while keeping deterministic
  iteration/spawn order (entities + ordering component, vs. keep pools in
  sim-core, vs. a hybrid)?
- Exact scenario/input-vector format for the time-series oracle, and how many
  seeds/levels/modes constitute "enough" coverage for the milestone.
- Where does the Rust state-hash live so both the headless oracle test and the
  future game binary share it?
- Single-worm shortcut: is a 1-worm scenario meaningfully testable, or must the
  oracle always run the 2-worm setup the C++ fixtures assume?

---

## Step 3 — Rendering

### Goal / done-when
Bevy draws the simulated world to a window **and** in the browser (Wasm): worms,
terrain, objects, viewport(s) rendered from the (already-correct) sim state. Done
when there's a playable image natively and in Wasm. Note: rendering is **not**
bit-exact-gated — it reads sim state, it does not produce it.

### Provisional sub-slice breakdown
1. **Static terrain blit.** Render the level material/display buffer as a texture
   (palette from 1c). Visual parity check against C++ screenshots.
2. **Sprites for entities.** Draw worms/objects from the 1d sprite banks at their
   sim positions, with palette mapping.
3. **Viewport / camera model.** Reproduce the split-screen viewport framing
   (offsets, shake) as a render concern only.
4. **HUD / text** (4×4 font bank), minimap — lower priority, can trail.
5. **Wasm target bring-up.** Build/run in the browser; confirm the same frame
   renders. May surface asset-loading and threading constraints.

### Oracle / verification strategy
- **See `2026-06-26-liero-rs-interactive-iteration-exploration.md`** for the full
  run/observe/iterate + regression design (native window vs headless screenshot vs
  web/wasm; deterministic replay + state-checksum as the authoritative regression
  gate; the headless-screenshot mode + fixed-seed demo launch to add here at step 3).
- **No tick-by-tick checksum here** — rendering is derived, not authoritative.
  Verification is mostly visual / screenshot comparison against the C++ renderer
  for a fixed sim state, plus "does it run in the browser."
- Risk: rendering work must **not** feed back into the sim (no float positions,
  no render-driven RNG). The only hard invariant is *isolation* — confirm the
  step-2 checksum still matches after rendering is wired in.

### Key risks & the hard 10%
- **API churn (research-before-build).** Bevy's rendering API (sprites, cameras,
  `bevy_render`, asset pipeline) moves fast. **Do fresh API research right before
  this step** — deep-research + context7 on the then-current Bevy version. Treat
  any Bevy version/feature decision as provisional until then.
- **Float boundary discipline.** Bevy `Transform` is `f32`; the conversion
  sim-fixed → render-float must be one-directional (sim → render only).
- **Wasm gotchas:** asset loading, no threads by default, palette/indexed-color
  blitting in a modern GPU pipeline. May force choices (e.g. shader-side palette
  lookup) that don't matter natively.

### Dependencies
- Needs step 2 sim state to draw, and `assets` 1c (palette/display) + 1d
  (sprites). Sounds (1e) belong with step 4 (loop/input) or here if convenient.

### Open questions to resolve before detailed planning
- Indexed-palette rendering approach (CPU blit to texture vs. GPU palette shader)
  — affects Wasm.
- Which Bevy version / rendering features to commit to (defer to step-start
  research).
- How faithful must the image be (pixel-exact vs. "looks like Liero")? Rendering
  isn't oracle-gated, so a tolerance must be chosen.

---

## Step 4 — Loop + input

### Goal / done-when
A fixed-rate game loop with keyboard input, producing **playable single-player
that feels like Liero**. The sim already ticks correctly; this step drives it at
the right cadence from real input and closes the play loop (menus enough to
start, sound triggers).

### Provisional sub-slice breakdown
1. **Fixed-timestep driver.** Run the step-2 tick at Liero's rate, decoupled from
   render framerate, with deterministic input sampling per tick.
2. **Input → control-state mapping.** Keyboard → the 7-bit `ControlState`
   (`Pack`/`Unpack`) the sim consumes; one-set-of-keys single player first.
3. **Sound triggering.** Hook sim events (fire, explosion, etc.) to the 1e WAV
   sounds. Audio is non-deterministic-output but must be triggered from sim
   events without affecting sim state.
4. **Minimal menu/start flow** to launch a match and respawn — only what
   single-player needs.

### Oracle / verification strategy
- **Determinism must survive real input.** Key invariant: a recorded input
  stream replayed through the loop reproduces the same checksums as the step-2
  oracle. This is the bridge test toward step 5 — **replay determinism is a
  precondition for rollback.**
- Reuse the step-2 time-series checksum harness, now fed by recorded real input
  rather than scripted vectors.
- **See `2026-06-26-liero-rs-interactive-iteration-exploration.md`** — the replay
  player + state-checksum regression harness (and the in-repo `run`/`verify` launch
  skill + stable CLI) to add at this step belong to that design.

### Key risks & the hard 10%
- **Input sampling timing.** Exactly one input snapshot per sim tick, sampled
  deterministically; sub-tick input or render-rate coupling breaks replay.
- **Loop/tick separation.** Render interpolation (if any) must not perturb sim
  timing or state.
- **Audio side effects.** Sound must be a pure consumer of sim events; no RNG or
  state in the audio path.

### Dependencies
- Needs step 2 (sim) and step 3 (something to look at). Sound needs 1e WAVs.
- The deterministic input-sampling model here is a **direct prerequisite for
  step 5** (ggrs feeds inputs per tick).

### Open questions to resolve before detailed planning
- Fixed-timestep strategy in Bevy (Bevy's `FixedUpdate` vs. ggrs's own schedule —
  may want to adopt the ggrs cadence now to avoid rework).
- Input recording/replay format (shared with the oracle and with step 5?).
- How much menu/UI is in scope vs. deferred.

---

## Step 5 — bevy_ggrs (rollback netplay)

### Goal / done-when
Two clients play the same match **desync-free** over the network via rollback
(GGRS). Done when the per-frame checksum agrees across peers under jitter / loss /
reorder — i.e. the Rust equivalent of the C++ `test_rollback_*` suite passes.

### Provisional sub-slice breakdown
1. **Sim under `GgrsSchedule`.** Move the step-2 ordered tick into ggrs's fixed
   schedule; register **all** sim state (incl. `sim-core` RNG via bevy_rand) as
   rollback state. This is mostly a *re-homing* of step 2, which is why step 2
   must be ordering-clean from the start.
2. **Snapshot / restore + checksum.** Implement save/load of the full sim state
   (the `fast_snapshot.hpp` field inventory) and a ggrs frame checksum equal to
   the step-2 hash.
3. **Local two-session rollback.** Two in-process sessions with synthetic delay
   (mirror `test_rollback_correctness.cpp`'s jitter transport): predict, advance,
   reconcile; both must match a zero-jitter reference.
4. **Real transport + signaling.** Wire to the network (the existing Go
   `server/` signaling/relay can likely be reused). Then jitter, packet loss,
   reorder, generation-drop scenarios.

### Oracle / verification strategy
- **The per-frame checksum becomes the rollback desync detector** — exactly what
  GGRS uses, and what the C++ rollback tests assert (post-convergence, all peers
  agree on every checksum still in the ring; state matches a zero-jitter
  reference run on the same inputs).
- The C++ `test_rollback_*` suite (`correctness`, `desync`, `packet_loss`,
  `reorder`, `replay`, `generation_*`, `skew_repro`, `weapsel`) is the **menu of
  scenarios to port**. This is a *new* harness family (transport simulation), not
  an extension of the static golden dumper.
- Snapshot round-trip correctness (save → restore → identical checksum) is its
  own test, mirroring `test_snapshot_fast.cpp` / `test_snapshot_roundtrip.cpp`.

### Key risks & the hard 10%
- **API churn (research-before-build, highest here).** bevy_ggrs **and**
  bevy_rand APIs move fast and must be co-version-matched with the chosen Bevy
  version. **Do fresh deep-research + context7 right before this step**; treat the
  roadmap's tech names as intent, not locked versions.
- **RNG as rollback state.** The ported MT19937 must be registered as ggrs
  rollback state (via bevy_rand or a custom resource) and saved/restored exactly.
  Any RNG state outside the rollback set = desync after a rollback.
- **Completeness of the snapshot set.** Every field that affects a future tick
  must be in the rollback state. The `fast_snapshot.hpp` inventory is the
  authority — anything omitted there (and only there for good reason, e.g. static
  display data) must be re-derivable, not silently dropped.
- **Determinism under re-execution.** Rollback re-runs ticks; the tick must be a
  pure function of (state, input). Any frame-counter, wall-clock, or
  iteration-order nondeterminism that survived step 2 will only now manifest.
- **The Bevy trap, again, with teeth:** ggrs enforces the ordered schedule, but
  any system reading non-rollback resources or relying on Bevy change-detection
  across rollbacks can desync.

### Dependencies
- Needs steps 2–4. Critically depends on **step 2 having been built
  ordering-clean and float-free** and **step 4's deterministic per-tick input
  model**. Possibly reuses the Go `server/` for signaling.

### Open questions to resolve before detailed planning
- Adopt the `GgrsSchedule` cadence as early as step 4 to avoid a re-home?
- bevy_rand vs. a hand-rolled rollback-registered RNG resource for the ported
  MT19937 (depends on then-current bevy_rand ergonomics).
- Snapshot strategy: full-copy vs. the C++ dirty-cell sparse level snapshot —
  performance vs. simplicity, decide with real numbers.
- Reuse existing Go signaling/relay vs. new transport.
- Number of players: C++ snapshot assumes 2 worms; does the rewrite generalize
  now or stay at 2 for parity?

---

## What we should NOT decide yet

These genuinely depend on learning from earlier steps; locking them now would be
false precision.

- **ECS architecture of the tick** (systems-native vs. sim-core-driver, how pools
  map to entities, deterministic iteration strategy) — decide *during step 2*
  with the checksum oracle in hand.
- **Exact Bevy / bevy_ggrs / bevy_rand versions and feature sets** — these move
  fast; research them right before steps 3 and 5, not now.
- **Rendering fidelity bar and palette-rendering technique** — decide in step 3
  once Wasm constraints are concrete.
- **Fixed-timestep mechanism** (Bevy `FixedUpdate` vs. ggrs schedule from the
  start) — decide in step 4 informed by whether we pre-adopt ggrs cadence.
- **Snapshot/rollback-state representation and transport** — decide in step 5
  with measured performance, guided by `fast_snapshot.hpp`.
- **Player count / larger-level / moddability generalizations** — explicitly
  deferred by the roadmap; do not let them complicate steps 2–4.
- **The full scenario/coverage matrix** for each oracle harness — sketch per
  step; finalize when writing that step's real spec.

> Reaffirming the posture: each section above is a *map*, not a commitment. When a
> step is reached, write its just-in-time spec, re-validate these assumptions
> against what steps 0–N actually taught us, and revise freely.
