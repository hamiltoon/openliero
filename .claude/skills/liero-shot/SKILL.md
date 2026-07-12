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
shot --scenario <name> --tick <n> [--tick <n> ...] (--out <path> | --hashes) [--scale <n>] [--tc-root <path>]
```

`--scale` default 3 (0 rejected); `--tc-root` defaults to the bundled TC assets.

## 5. Fixed-seed note

Every scenario is seed 42, input-scripted, and GPU-free ⇒ the same frame on every
run and every machine (no adapter dependence). The renderer drives **all** ticks up
to the target — the viewport-local RNG steps per draw, so a skipped tick would
desync every later frame. That means `--tick 500` is not momentary: it renders
0..=500 in order.
