---
name: liero-shot
description: Run and observe the Liero-rs renderer headless via the shot CLI — screenshot a scenario at a fixed tick to a PNG, and diff frame/state hashes against the committed goldens for regression comparison. Use in the change → screenshot → judge loop and as the run/verify harness's project run-skill for the renderer.
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
