# Liero-rs (Rust rewrite)

Cargo workspace for the agent-driven Rust/Bevy rewrite of OpenLiero.
See `../docs/superpowers/specs/2026-06-26-liero-rs-roadmap.md`.

## Crates

- `sim-core` — the deterministic core (fixed-point, vector, sqrt, cossin, RNG).
  No dependencies, no Bevy. Everything is integer arithmetic that matches the
  C++ engine bit-exactly.
- `oracle-tests` — differential tests against the C++ oracle via golden vectors.

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
