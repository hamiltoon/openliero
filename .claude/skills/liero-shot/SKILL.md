---
name: liero-shot
description: Run and observe the Liero-rs renderer headless via the shot CLI — screenshot a scenario at a fixed tick to a PNG, and diff frame/state hashes against the committed goldens for regression comparison. Also documents the `game` binary's live/record/replay loop (play, capture a session, play it back headless-verified). Use in the change → screenshot → judge loop and as the run/verify harness's project run-skill for the renderer.
---

# liero-shot — headless render/observe loop

`shot` is a Bevy-free, GPU-free, display-free CLI that drives a committed 3b scenario
and produces (a) a deterministic PNG to view and (b) machine-readable frame/state
hashes to diff against goldens. This is the renderer's `run`/`verify` entrypoint.

## 1. Build

```
cargo build --manifest-path rust/Cargo.toml -p shot
```

Fast — Bevy-free, no GPU/display. Contrast: the `game` binary is the minutes-long
Bevy window build; `shot` is the headless path and is what this loop uses.

## 2. Screenshot at a fixed seed/scenario/tick

```
cargo run --manifest-path rust/Cargo.toml -p shot -- --scenario blood --tick 35 --out /tmp/blood_t35.png
```

Deterministic PNG at a known path — then *view* it to judge the change
(change → screenshot → judge). `--scale` defaults to 3, so the 320×200 frame
becomes 960×600. One `--tick` + `--out` = exactly that file; multiple `--tick`
values make `--out` a directory of `<name>_tick<N>.png`.

## 3. Regression compare (the hard gate)

```
cargo run --manifest-path rust/Cargo.toml -p shot -- --scenario blood --tick 40 --hashes
```

`--hashes` prints to stdout, one line per tick `0..=max(--tick)`:
`<tick> <frame_hash_hex16> <state_hash_hex8>`, then `total <n> <acc_hex16>`.
Diff those lines against `rust/oracle-tests/golden/render_slice3b_blood.txt`:

- **frame_hash / total mismatch** at a named tick = a **render** regression.
- **state_hash mismatch** = a **sim** regression.

The checksum is authoritative; the screenshot is advisory. Note: the `total`
line is directly comparable only when `--tick` equals the scenario's full tick
count (blood = 40). Info/progress goes to stderr, so stdout stays parseable.

## 4. Scenarios (7) + flag contract

Each 3b golden exercises a subsystem surface — pick the one you changed:

- `laser` — laser sight + viewport-RNG
- `shadow` — the shadow pass
- `blood` — blood + terrain carve
- `dart_water` — water `BlitImageR`
- `shake` — screen shake + flash
- `dart` / `fan` — objects / projectiles

Full flag contract (requires `--out` OR `--hashes`):

```
shot --scenario <name> --tick <n> [--tick <n> ...] (--out <path> | --hashes) [--scale <n>] [--tc-root <path>] [--hud]
```

`--scale` default 3 (0 rejected); `--tc-root` defaults to the bundled TC assets.
`--hud` (opt-in) draws the FULL player view — HUD bars/text + minimap (slice 3e);
without it the frame is the world-only view the 3b goldens gate. The 3e corpus
(`hud`/`reload`/`death` scenarios) resolves too — 3e names are tried before 3b.

## 5. Fixed-seed note

Every scenario is seed 42, input-scripted, and GPU-free ⇒ the same frame on every
run and every machine (no adapter dependence). The renderer drives **all** ticks up
to the target — the viewport-local RNG steps per draw, so a skipped tick would
desync every later frame. That means `--tick 500` is not momentary: it renders
0..=500 in order.

## 6. Browser (wasm) dev loop — slice 3f

Run the same demo in a browser canvas (WebGL2, embedded assets — no fs, no server
assets). IMPORTANT: run from the `rust/` directory — cargo discovers the
target-scoped `wasm-server-runner` in `rust/.cargo/config.toml` from **cwd**, not
from `--manifest-path`:

```
cd rust
cargo run -p game --target wasm32-unknown-unknown
```

then open the printed `http://127.0.0.1:1334` URL. Prereqs (one-time):
`rustup target add wasm32-unknown-unknown`, `cargo install wasm-server-runner`.
A **debug** build keeps the per-tick determinism guard live (`state_hash` +
`frame_hash` vs the embedded golden) — a clean browser console over a loop IS the
wasm parity witness; a `debug_assert` panic in the console means a platform
divergence leaked into the sim (stop and reproduce natively). The eyeball check:
the blood demo's split-screen world view, animating at the C++ cadence. Static
bundle recipe: see `web/index.html`.

## 7. Play the live game — Slices 4a–4g

Everything above drives `shot`, the headless renderer. This section drives the
other binary: `game` — the real windowed Bevy app, native-only, keyboard-driven.

### 7.1 Live play

```
cargo run --manifest-path rust/Cargo.toml -p game
```

Bare invocation (no flags, no positional name, since 4f) opens a **playable
default match**: a fixed committed fixture (`game/scenarios/default_match.txt`
— shipped `render_stage` level, two visible worms, seed 42, `weapon 0 DART`),
keyboard-driven, indefinite (no tick bound, no self-check). Window title
`"Liero-rs — default match"`.

```
cargo run --manifest-path rust/Cargo.toml -p game -- --live [name]
```

`--live` alone plays the caller's default scenario (`blood`) live instead of
the fixed replay demo; `--live <name>` plays any committed
`render_slice3b_<name>_scenario.txt` live (its recorded inputs are ignored —
only its level + worm-init are used). A **positional `<name>` with no
`--live`** still resolves the old `Mode::Scripted` self-check demo
(`cargo run -p game -- blood` replays the committed inputs and asserts every
tick against the golden — unchanged, the CI regression path never runs off
the bare invocation).

Default key bindings (`game::input::default_bindings`), decoded from the C++
DOS scancode table:

| | Up | Down | Left | Right | Fire | Change | Jump |
|---|---|---|---|---|---|---|---|
| P0 | R | F | D | G | Ctrl(L) | Shift(L) | Alt(L) |
| P1 | ↑ | ↓ | ← | → | Ctrl(R) | Alt(R) | Shift(R) |

Dig is **unbound** for both worms by default — hold a worm's own Left+Right
together to dig (the C++ default). `F5` restarts the match from tick 0 (reruns
the same scenario load the Scripted loop-reload uses) — verified unbound
against the table above (`R` is P0 fire, so it was never a restart-key
candidate). `Esc` or the window-close button quits.

### 7.2 Record → replay round-trip

```
cargo run --manifest-path rust/Cargo.toml -p game -- --live --record /tmp/x.txt
```

Plays live exactly as 7.1, and additionally buffers every sampled per-tick
input snapshot in memory. **Quit gracefully** (`Esc`, or the window-close
button) to flush: the buffer is written to `/tmp/x.txt` only in the
`AppExit`-triggered `flush_recorder_on_exit` system (`main.rs`, ordered
`.after(bevy::window::ExitSystems)`). **A signal kill (`alarm`/`timeout`/Ctrl-C)
never reaches that system and writes no file** — window closure via Esc/X is
the only path that flushes, so a headless/scripted smoke test cannot exercise
this half of the loop (see 7.3 for the headless-safe verification instead).
`F5` during a recording also **restarts the recording**: it clears the
buffered snapshots so the eventual flush covers only ticks since the last
restart, not a spliced pre/post-restart stream (`Recorder::clear`).

Then play the recording back:

```
cargo run --manifest-path rust/Cargo.toml -p game -- --replay /tmp/x.txt
```

`--replay <path>` loads *any* scenario/recording file (bypassing the
golden-dir name lookup `<name>` uses) as `Mode::Replay`: the same
`InputSource::Scripted` feed as the self-check demo, but with the loop and the
debug self-check both retired (no golden exists for an arbitrary recording).
It plays once through the recording's `ticks` and then **holds** on the final
frame — it does not loop. `--replay` is mutually exclusive with
`--live`/`--record` (rejected before any window opens, e.g.
`--replay excludes --live/--record`).

### 7.3 Headless replay verification (no window)

Two ways to check a recording without a display:

- **State-hash time series** — `game::input::replay_state_series(tc_root,
  &scenario)` drives `InputSource::Scripted` over any parsed `Scenario`
  headlessly (no Bevy `App`) and returns the per-tick `hash_game_state`
  series. This is the library call both `tests/round_trip.rs` (the record →
  replay round-trip gate) and `tests/record_regression.rs` (the committed-corpus
  drift backstop) build on — see 7.4.
- **Screenshot a recording at a fixed tick** — `shot --scenario-path
  <recording> --tick <n> --out <path>` reads an arbitrary scenario/recording
  file directly (bypassing `--scenario <name>`'s golden-dir lookup) and renders
  it through the same `render_scenario` path as every other `shot` screenshot.
  Landed in the same slice (4g); integration-tested against the committed
  recorded corpus (`rust/shot/tests/scenario_path.rs`). It slots into the
  change → screenshot → judge loop (§2) for a recorded match exactly like
  any committed scenario.

### 7.4 CI coverage (already wired, no new job needed)

Every input/replay gate already runs in the two existing CI steps — nothing in
4g adds a new job:

- `cargo test -p game` — `tests/passthrough.rs` (4a, live-sampler pass-through
  state gate), `tests/round_trip.rs` (4b, **the hard gate**: record through
  `Live`, replay through `Scripted`, assert the two independently-derived
  series match tick-for-tick), `tests/record_regression.rs` (4b, committed-corpus
  drift backstop — catches a *symmetric* serializer drift the round-trip gate
  is blind to), `tests/viewport_stepping.rs` (4d, live shake/banner stepping).
- `cargo test --workspace --exclude game` — `replay/tests/corpus.rs` (4e,
  phase-1 `.lrp` `WideRollbackChecksum` gate over the committed `.lrp` corpus),
  plus every `sim`/`scenario`/`render`/`oracle-tests` golden.

The `.lrp` `framehash` diff (phase 2) is a booked follow-on, explicitly out of
scope here.

### 7.5 Audio note

Sound only plays in windowed mode: `setup_audio` opens the real output device
(`RodioSink`, falling back silently to a no-op `NullSink` if none is found —
never a crash). Every headless caller — `shot`, `replay_state_series`, the
round-trip/passthrough/corpus tests — never constructs the Bevy `App` at all,
so they never touch audio; a headless verification run is always silent by
construction, not by a special-cased flag.
