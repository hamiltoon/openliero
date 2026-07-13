# Step 4 · Slice 4a — Live input core: detailed design

Status: **draft for review** · 2026-07-12
Part of: `2026-07-12-liero-rs-step4-input-replay-overview.md` (cited as **overview**)
Sources: `2026-07-12-liero-rs-step4-cpp-input-replay-map.md` (**input-map §N**),
`2026-07-12-liero-rs-step3-slice3c-bevy-window-design.md` (the `game`-binary architecture, **3c**),
`2026-06-26-liero-rs-interactive-iteration-exploration.md` (**iter §N**).
Companion output feeds: `superpowers:writing-plans` (this is the spec; the plan is the sibling file).

This is the executable design for the **first real input** in the rewrite. 3c stood up the `game`
binary that ticks the (bit-exact) `sim` on `FixedUpdate` at the C++ cadence off a scenario's
**recorded** inputs (`rust/game/src/main.rs:244-307`, feed at `:257-261`). 4a replaces the *source*
of that per-tick `[ControlState; N]` array with **live keyboard**, while keeping the scripted path —
and its determinism self-check — intact as the objective gate. Nothing in `sim` changes; the snapshot
boundary `SimState::process_frame(&[ControlState; N])` (`rust/sim/src/state.rs:1431`) already is the
ggrs-shaped seam (overview locked-decision 1), so 4a only feeds it from a new source.

---

## Goal / done-when

`cargo run -p game -- --live` opens a window where a **1-player + 2-player hotseat** match is
**playable from the keyboard**, the sim ticking at `1000/14 Hz` with **exactly one input snapshot per
tick**, no render-rate coupling. The scripted path (`cargo run -p game -- <name>`) is unchanged and
still reproduces its golden.

**Done when:**
1. `cargo run -p game -- --live [name]` is playable: both default keysets drive their own worm
   (move / aim / fire / change / jump), **Dig = Left+Right chord**, Esc quits.
2. Exactly **one** `[ControlState; N]` snapshot is sampled per `FixedUpdate` tick, inside the single
   tick system, decoupled from render rate (overview risk §1 / input-map §1a).
3. **Pass-through gate (objective, headless, CI):** driving the sim through the new sampler in
   *scripted* mode reproduces every committed scenario's `hash_game_state` time series bit-exact —
   the sampler is a faithful pass-through for recorded input (overview 4a "Oracle/gate").
4. The debug per-tick determinism self-check is **retained for scripted mode, retired for live**
   (overview risk "determinism guard retirement").
5. Default bindings mirror the C++ defaults exactly (table below); bindings are a plain configurable
   struct (no settings UI — overview 4a scope).

**Not bit-gated** (cannot be): live keyboard, window presentation. Those are proven by the
pass-through determinism gate (3) + a manual playability check, exactly as the overline Oracle
strategy prescribes.

**Not in 4a:** recording/replay artifact (4b), audio (4c), live shake/flash (4d), `.lrp` (4e),
menu/start-flow (4f), gamepad (overview deferral). The design shows the **4b recorder seam** but
builds no recorder.

---

## 1. What C++ does — the input pipeline (with `file:line`)

The overview's input-map §1 is the ground truth; the load-bearing facts 4a must reproduce:

- **Level-triggered per bit, sampled at tick time.** C++ mutates a worm's live `control_states`
  immediately on each SDL key edge (`gfx.cpp:596-641` → `LocalController::OnKey`,
  `localController.cpp:58-86`), and the fixed-cadence loop consumes *whatever bits are set when
  `ProcessFrame` runs* (`localController.cpp:153-181`). There is **no sub-tick edge queue**: a
  key press+release that cancels before the tick is invisible; the last edge wins (input-map §1a).
- **Events pumped once per rendered frame** (`gfx.cpp:727-733`), then `kRealFrameSkip` ticks consume
  the same accumulated state (`localController.cpp:154-155`). So in a slow frame multiple ticks read
  the *same* input — determinism is by tick count, never wall clock (input-map §1c).
- **Hardware key-repeat is filtered** for the edge write (`gfx.cpp:608`, `!ev.key.repeat`); the level
  state (`dos_keys` / `control_states`) is unaffected. Only the weapon-select phase re-emulates repeat
  (`localController.cpp:124-152`), which 4a does not port (no weapon select).
- **`Dig` is a synthetic Left+Right chord, never a stored bit** (`localController.cpp:69-79`,
  input-map §1a/§8.2). `kDig` is control index **7**, outside the 7 packed bits
  (`worm.hpp:45-55`: `kUp=0…kJump=6, kDig=7, kMaxControl=7, kMaxControlEx=8`). Its net effect is a
  pure function of the physical key state — see §3.
- **Key → (worm, control)** via each worm's binding array (`Game::FindControlForKey`,
  `game.cpp:75-108`): with `Settings::kExtensions` it scans `controls_ex[0..kMaxControlEx=8]` (so the
  optional Dig binding is included), matching the DOS scancode to a `(worm, control)`.
- **Focus loss**: a controller that loses focus stops receiving key events; held keys must **not** be
  re-read from OS state on regain (input-map §1d) — the accumulated/recorded stream is the truth.

## 2. Exact default binding table (the deliverable)

C++ defaults live in `settings.cpp:36-37` as DOS scancodes and are applied in `settings.cpp:41-56`
(loop `j < 7`, so only `kUp..kJump` get defaults; `controls_ex[7]` = **Dig stays 0 / unbound**,
zeroed by `WormSettingsExtensions()` `worm.hpp:62`). Decoding each DOS scancode through the
`liero_to_sdl_keys[]` table (`keys.cpp:9-58`) to its SDL scancode, then to the Bevy `KeyCode`:

| Control | Player 0 (worm 0) DOS→SDL | Bevy `KeyCode` | Player 1 (worm 1) DOS→SDL | Bevy `KeyCode` |
|---------|---------------------------|----------------|---------------------------|----------------|
| Up      | `0x13` → `SDL_SCANCODE_R` | `KeyR`         | `0xA0` → `SDL_SCANCODE_UP`    | `ArrowUp`    |
| Down    | `0x21` → `SDL_SCANCODE_F` | `KeyF`         | `0xA8` → `SDL_SCANCODE_DOWN`  | `ArrowDown`  |
| Left    | `0x20` → `SDL_SCANCODE_D` | `KeyD`         | `0xA3` → `SDL_SCANCODE_LEFT`  | `ArrowLeft`  |
| Right   | `0x22` → `SDL_SCANCODE_G` | `KeyG`         | `0xA5` → `SDL_SCANCODE_RIGHT` | `ArrowRight` |
| Fire    | `0x1D` → `SDL_SCANCODE_LCTRL`  | `ControlLeft`  | `0x75` → `SDL_SCANCODE_RCTRL`  | `ControlRight` |
| Change  | `0x2A` → `SDL_SCANCODE_LSHIFT` | `ShiftLeft`    | `0x90` → `SDL_SCANCODE_RALT`   | `AltRight`     |
| Jump    | `0x38` → `SDL_SCANCODE_LALT`   | `AltLeft`      | `0x36` → `SDL_SCANCODE_RSHIFT` | `ShiftRight`   |
| Dig     | *(unbound in C++ defaults)* | `None`       | *(unbound)*                 | `None`         |

Player 0's WASD-style diamond is `R`(up)/`F`(down)/`D`(left)/`G`(right) around `F`; player 1 is the
arrow cluster. **Dig has no default key** — players dig by pressing Left+Right together (§3). The chord
expansion is still implemented so a *configured* Dig key works. (Bevy 0.19 `KeyCode` variant names
above are the stable winit names — confirm spelling against `bevy` 0.19 at implementation; the DOS→SDL
column is the authority.)

Worm colors are incidental to input but fix the P0/P1 identity: `worm_settings[0].color=32`,
`[1].color=41` (`settings.cpp:32-33`).

## 3. Dig-chord semantics — a pure function, never a stored bit

C++ `OnKey` runs, on **every** key edge (`localController.cpp:69-79`):
`if clean[kDig] { Press(kLeft); Press(kRight); } else { if !clean[kLeft] Release(kLeft); if
!clean[kRight] Release(kRight); }`. The steady-state invariant this produces is exactly:

```
Left_bit  = pressed(left_key)  || pressed(dig_key)
Right_bit = pressed(right_key) || pressed(dig_key)
```

So Dig is a **pure function of the physical key state** — it never lives in the 7-bit `ControlState`
(input-map §8.2). 4a computes it directly at snapshot time and never stores a dig bit. This also means
the two default keysets, which have no Dig key, dig by holding Left+Right — identical to C++.

## 4. Rust architecture — resources, the sampler, system order

All new code is in `game` (the only Bevy crate — 3c); `sim`/`render`/`scenario` stay Bevy-free and
**unchanged**. No new sim field, no `f32`/`Vec2` into the sim (the isolation firewall, 3c).

### 4.1 The pure sampler (the load-bearing, Bevy-free-testable piece)

Per-worm bindings and the sample function are **generic over the key type** so the correctness core
(bit mapping + Dig chord) is unit-testable without Bevy, and `game` instantiates it with `KeyCode`:

```rust
// game/src/input.rs
pub struct PlayerBindings<K> {
    pub up: K, pub down: K, pub left: K, pub right: K,
    pub fire: K, pub change: K, pub jump: K,
    pub dig: Option<K>,           // unbound by default (§2)
}

impl<K: Copy> PlayerBindings<K> {
    /// One tick's ControlState for this worm, level-triggered from the held-key
    /// query. Dig OR's into Left/Right (§3); never a stored bit. Mirrors the
    /// steady state of localController.cpp:58-79 + game.cpp:75-108.
    pub fn control_state(&self, pressed: impl Fn(K) -> bool) -> ControlState {
        let dig = self.dig.map_or(false, |k| pressed(k));
        let mut cs = ControlState::new();
        cs.set(ControlState::UP,     pressed(self.up));
        cs.set(ControlState::DOWN,   pressed(self.down));
        cs.set(ControlState::LEFT,   pressed(self.left)  || dig);
        cs.set(ControlState::RIGHT,  pressed(self.right) || dig);
        cs.set(ControlState::FIRE,   pressed(self.fire));
        cs.set(ControlState::CHANGE, pressed(self.change));
        cs.set(ControlState::JUMP,   pressed(self.jump));
        cs
    }
}
```

**Why generic + closure:** it makes the Dig-chord/bit-mapping logic testable with a mock key type and
a `HashSet` "pressed" closure — no window, no `ButtonInput` — so the correctness core runs in the fast
CI test set. The Bevy adapter is a one-liner: `bindings.control_state(|k| keys.pressed(k))`.

### 4.2 The input source (the 4b-extensible seam)

```rust
#[derive(Resource)]
enum InputSource {
    /// Recorded scenario inputs — the existing 3c path (main.rs:257-261),
    /// now behind the source. Bevy-free; drives the pass-through gate.
    Scripted(Scenario),
    /// Live keyboard: one PlayerBindings<KeyCode> per worm.
    Live(InputMap),
    // 4b adds: Replay(Recording) — reads back the recorded-input artifact,
    // symmetric with Scripted. (Not built in 4a.)
}
```

`Scripted::sample(t)` = `[ControlState::unpack(scn.input(t,0)), ControlState::unpack(scn.input(t,1))]`
(`parser.rs:261-268`) — a literal pass-through. `Live::sample(&keys)` = per-worm `control_state`. The
enum returns a `[ControlState; N]` (N = worm count = 2, positional by worm index — the same index
`process_frame` reads and `Viewport::worm_idx` maps, `viewport.rs:13`).

### 4.3 Where the snapshot happens — the single FixedUpdate system

Sampling replaces the inline `scenario.input` at the **top** of the existing single `FixedUpdate`
system (`tick_and_render`, `main.rs:244`), sampled **once**, immediately before `process_frame`:

```
FixedUpdate  (Time::<Fixed>::from_hz(1000.0/14.0), unchanged — main.rs:107)
  sample_tick_and_render:
    1. let inputs = source.sample(demo.tick, &keys);   // ONE snapshot / tick
    2. // [4b recorder seam: recorder.record(demo.tick, &inputs) goes HERE]
    3. sim.process_frame(&inputs);
    4. scripted-only: loop/reload at scenario.ticks (main.rs:263-272)
    5. render + upload (unchanged — render_and_upload)
    6. scripted-only + debug: self-check hash == golden (main.rs:281-287)
```

**Why sampling lives inside the FixedUpdate tick system, not an `Update` accumulator:** it guarantees
*exactly one snapshot per tick* with zero render-rate coupling — the central overview risk. `ButtonInput`
is already the accumulated, level-triggered held-key set (Bevy refreshes it in `PreUpdate`), so reading
`keys.pressed(k)` at tick time *is* the C++ "consume whatever bits are set at `ProcessFrame`". A
separate Update→resource accumulator would reintroduce an Update/FixedUpdate ordering hazard and a
second state to desync, for no gain (see §5). Esc/quit stays its own `Update` system (`close_on_esc`,
`main.rs:404-408`, retained unchanged).

### 4.4 Key-edge handling vs Bevy `ButtonInput` semantics

We sample **level state** (`keys.pressed`), not edges, so:
- **Key-repeat** is irrelevant (we never read `just_pressed`/events for the snapshot), matching C++'s
  repeat-filtered level state (input-map §8.3).
- **Sub-tick cancel** (press+release within one Update cycle before any tick): `pressed()` is false at
  tick time → invisible, matching input-map §1a exactly.
- **Catch-up ticks** (a slow frame runs `FixedUpdate` twice): both ticks read the same `ButtonInput`
  (only refreshed in `PreUpdate`), exactly as C++ feeds the same accumulated state to `kRealFrameSkip`
  ticks (input-map §1c). The sim's own edge detection (`prev_control_states` / `pressed_once`,
  `state.rs:117-128`) then behaves identically across the repeat.
- Per-tick edge semantics inside the sim (weapon change on the Change edge, etc.) are the sim's job,
  comparing successive snapshots — 4a feeds it faithful level snapshots and touches none of that.

### 4.5 Focus-loss handling (input-map §1d)

On window focus loss winit may not deliver key-up events, risking a "stuck" held key. Bevy releases
pressed keys on `WindowFocused(false)` by default (its keyboard/focus handling), so `keys.pressed`
goes false and the worm stops — the safe behavior and consistent with "don't re-read OS state across
focus." **Verify** this at implementation on the target `bevy` 0.19; if it does not auto-release, add
a tiny system that clears the held set on `WindowFocused(false)`. This is a live-only UX concern
(not gated): the scripted/replay path has no window and trusts the recorded stream, per input-map §1d.

## 5. The pass-through gate — construction

The objective gate (done-when 3): **the new sampler, fed a recorded scenario, reproduces the scenario
golden bit-exact.** Two composed proofs, both in the fast CI test set:

1. **Sampler pass-through (pure, no sim):** `InputSource::Scripted::sample(t)` equals
   `[unpack(scn.input(t,0)), unpack(scn.input(t,1))]` for every tick — the source is literally the
   recorded values. Plus the §4.1 sampler unit tests (each bit; Dig chord held→L&R, release reverts;
   unbound dig). These pin the sampler's correctness with no window.
2. **Headless determinism replay (the milestone gate):** for each committed scenario, `scenario::load`
   the initial state, drive `process_frame` through `InputSource::Scripted` for all `ticks`, and
   assert per-tick `hash_game_state` == the golden `state_hash` column. This runs **headless** (Scripted
   touches no Bevy/window), so it belongs in a `game` integration test wired into CI (see §7).

Together they prove determinism survives the new input plumbing — and (2) is exactly the object 4b
extends to the record→replay round-trip. The in-binary debug self-check (`main.rs:281-287`) is retained
for the *windowed* scripted run as John's manual integration smoke.

## 6. The 4b seam (built later, marked now)

4b records the per-tick `[ControlState; N]` snapshot and replays it headlessly for the record→replay
round-trip. The seam 4a leaves:
- **Record point:** the sampled `inputs` in the FixedUpdate system, between sample and `process_frame`
  (§4.3 step 2). 4b drops a recorder there that serializes `inputs` (+ seed/level/weapon) into the
  recorded-input artifact — the overview's "extend the scenario `input <tick> <w0> <w1>` grammar"
  (overview format note; `parser.rs:205-213`).
- **Replay source:** `InputSource::Replay(Recording)` — a new enum arm symmetric with `Scripted`,
  reading the artifact back (§4.2). The `InputSource` boundary is the whole extension point; §5's
  headless replay harness becomes 4b's round-trip CI gate.
- C++ order context for 4b (not 4a): record writes the delta from the *previous* tick then advances
  (`localController.cpp:166-176`), `prev_control_states` updated in `ProcessFrame`'s tail
  (`game.cpp:466-468`, input-map §1b/§3c).

## 7. CLI, verification, CI

- **CLI.** `cargo run -p game -- --live [name]` → live mode: load `name`'s (default `blood`) initial
  state via `scenario::load`, feed `InputSource::Live` with the default `InputMap`, **disable** the
  loop/reload and the golden self-check, run indefinitely. `cargo run -p game -- <name>` → scripted
  mode, unchanged (guard + loop kept). Arg parsing extends `resolve_scenario` (`main.rs:120-134`);
  `--live` is a flag consumed before the positional name. Default (no args) stays scripted `blood`
  so CI/self-check semantics are unchanged.
- **Milestone (manual, not gated):** John runs `--live`, drives both worms with the two keysets,
  fires/jumps/digs (L+R), Esc quits — the 4a playability proof (iter §1b/§4).
- **CI.** The pass-through gate (§5) runs under `cargo test -p game`. 3c's CI runs the fast determinism
  job as `cargo test --workspace --exclude game` **plus** a separate `cargo build -p game` step
  (3c Verification). 4a extends that build step to `cargo test -p game` so the sampler unit tests +
  the headless replay gate run in CI without dragging Bevy into the fast job. (This is the minimal
  slice of 4g wiring the gate needs; the rest of 4g is later.)
- **Non-live paths unchanged:** the existing `render_slice3b_*` / `sim_slice*` goldens must stay
  byte-identical (4a adds a source indirection, not a sim change) — a standing re-diff assertion.

## 8. Wasm notes

- `ButtonInput<KeyCode>` works under wasm (winit canvas), but 4a keeps wasm on the **scripted witness
  path** (3c/3f): the embedded `blood` scenario + the debug frame-hash/state self-check. **No live
  mode on wasm in 4a.**
- Live-wasm is deferred because the browser hijacks the P1 default keys (arrows scroll the page;
  `Ctrl`/`Alt` combos are browser shortcuts) — capturing them needs canvas focus + `preventDefault`
  wiring, a separate concern. The pass-through gate is native anyway (Scripted, headless).
- Nothing in 4a perturbs the wasm build: the `InputSource::Scripted` path is what wasm already runs;
  `--live`/CLI is `cfg(not(wasm))` (wasm has no args — `main.rs:141-144`).

## 9. Risks & the hard 10%

- **Sampling timing (the central risk).** Exactly one snapshot per tick, in the FixedUpdate system,
  from level state — never an Update accumulator, never `just_pressed` edges for the snapshot. Moving
  sampling to `Update` or reading edges would leak render-rate/OS coupling and break 4b + Step 5. §4.3
  is the mitigation; the pass-through gate catches a regression.
- **Dig chord direction.** Dig must OR into Left/Right at sample time (§3), never a stored 8th bit;
  getting it backwards (or storing dig) diverges the sim. Unit-tested in §4.1.
- **Focus-loss stuck keys.** Rely on Bevy's release-on-focus-lost; verify, else add a
  `WindowFocused(false)` clear (§4.5). Live-only, not gated, but a real UX bug.
- **Determinism-guard retirement done wrong.** The self-check must stay for scripted/replay and be
  skipped only for live — do not let `--live` silently disable the regression path for scripted runs
  (overview risk). Gate the check on `mode == Scripted` (§4.3 step 6).
- **Browser key hijack (wasm live).** Deferred (§8); do not ship a half-working live-wasm that eats
  page scroll.
- **CI coverage of the sampler.** The correctness core lives in `game`, which the fast job excludes;
  the gate only bites if `cargo build -p game` is upgraded to `cargo test -p game` (§7). If that step
  is skipped, the pass-through gate is dead — call it out explicitly in the plan.

## 10. Deferrals (explicitly out of 4a)

- Record/replay artifact + round-trip regression — **4b** (seam marked §6).
- Audio — **4c**; live shake/flash/banners — **4d**; `.lrp` — **4e**; menu/start-flow — **4f**;
  run/verify skill + full CI wiring — **4g** (4a wires only the pass-through gate step).
- Gamepad/joystick (`gfx.cpp:651-720`), weapon-select key-repeat (`localController.cpp:124-152`),
  settings UI for rebinding, follow-cam in live mode (the 3c Option-B toggle) — all deferred.
- Live mode on wasm (§8).

## 11. Open questions for the controller (max 3)

1. **Home of the pure sampler.** Recommendation: `game/src/input.rs` (generic core + `KeyCode`
   instantiation), tested via `cargo test -p game` (CI step upgraded from `cargo build -p game`). The
   alternative — hoisting the generic core into a Bevy-free crate (`scenario`) so it rides the fast
   `--workspace` job — is cleaner for CI but spreads an input concern into the file-format crate.
   Accept the `game`-local home + the test-step upgrade?
2. **Default run mode.** Recommendation: keep **scripted `blood`** as the no-arg default (preserves
   the self-check's meaning and CI smoke); `--live` is the opt-in playable mode. Or make `--live` the
   no-arg default for ergonomics, accepting the no-arg run no longer self-checks?
3. **Live-mode camera.** Recommendation: reuse the 3c fixed camera (worms are on-screen, playable);
   defer follow-cam to the 3c Option-B `--follow` toggle. Or pull follow-cam into 4a for feel?
