# Liero-rs (Rust rewrite)

Cargo workspace for the agent-driven Rust/Bevy rewrite of OpenLiero.
See `../docs/superpowers/specs/2026-06-26-liero-rs-roadmap.md`.

## Crates

- `sim-core` — the deterministic core (fixed-point, vector, sqrt, cossin, RNG).
  No dependencies, no Bevy. Everything is integer arithmetic that matches the
  C++ engine bit-exactly.
- `oracle-tests` — differential tests against the C++ oracle via golden vectors.

## assets crate

`assets` loads OpenLiero's on-disk formats (no Bevy). Behaviour that feeds the
simulation is differential-tested against the C++ engine; the implementation is
idiomatic Rust (`from_le_bytes` + slicing, `Result`), not a port of the C++ `io`
layer.

- `level` — `.lev` material-map loader (legacy 504×350 + OLLEVEL2 sized).

### Level golden

Unlike the math golden (cheap standalone clang build), the level golden runs the
real `Level::load` and so needs the full C++ build. Regenerate it locally:

```bash
bash rust/oracle-tests/gen_level_golden.sh      # PRESET=linux-x64 on Linux
```

The lightweight `rust.yml` CI does not regenerate it; it runs `cargo test` against
the committed `golden/level.txt`.

## Oracle workflow

1. The C++ dumper (`src/tools/oracle_dump`) runs the *existing* C++ functions and
   writes deterministic results to `oracle-tests/golden/*.txt`.
2. The Rust tests read the same files and require identical output line by line.

Regenerate the golden vectors after changing the C++ `math`/`rand` code:

```bash
./oracle-tests/gen_golden.sh
```

## Running the tests

```bash
cd rust && cargo test
```
