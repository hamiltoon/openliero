# Step 1 — Data formats: overview

Status: **draft for review** · 2026-06-26
Part of: `2026-06-26-liero-rs-roadmap.md`

Step 1 of the Liero-rs rewrite: read OpenLiero's on-disk data formats in Rust,
**byte-identical to the C++ engine**, so the new engine consumes the same
`data/` assets as the oracle. This document is the step-1 altitude ("everything");
each slice below gets its own detailed spec and is built one at a time.

## Principle: same oracle, sliced

We keep the step-0 strategy: the C++ engine is the **oracle of truth**. A C++
dumper loads an asset with the *real* engine loader and writes the parsed result
to golden files; the Rust loader must reproduce them bit-for-bit. Because "data
formats" is really five independent formats, we **decompose and build one slice
at a time** — each slice is self-contained, differential-tested, and CI-green
before the next begins.

## The slices

| Slice | Loads | Produces | Depends on | Oracle input |
|---|---|---|---|---|
| **1a** | IO primitives | `Reader`/`MemReader`, little/big-endian int reads | — | synthetic byte buffers |
| **1b** | Level material map | `LevelData { width, height, material_id }` | 1a | `modern_test.lev` + synthetic legacy/OLLEVEL2 |
| **1c** | Palette + display layers | `Palette` (VGA 6→8, modern 8-bit); MODERNLV true-color + animation | 1a, 1b | `.lev` MODERNLV/POWERLEVEL blocks, `modern.pal` |
| **1d** | Sprites (TGA) | `SpriteSet` banks (small 7×7, large 16×16, text 4×4) | 1a | `sprites/{small,large,text}.tga` |
| **1e** | TC bundle | `tc.cfg` (TOML), WAV sounds, weapon/nobject/sobject `.cfg` | 1a, +much | `data/TC/openliero/**` |

**Build order:** 1a+1b → 1c → 1d → 1e. Rationale: the simulation (step 2) needs
only the **material map** (`material_id`); palette, display, sprites and sounds
are rendering/audio data that step 3 needs. So the first slice (1a+1b) unblocks
step 2 directly; the rest follow when rendering is built.

## Crate layout

A new Bevy-free `assets` crate sits beside `sim-core`:

```
rust/
├── sim-core/         deterministic core (step 0, unchanged)
├── assets/           ← NEW: on-disk format loaders (no Bevy)
│   └── src/
│       ├── io.rs       Reader/MemReader + endianness helpers          (1a)
│       ├── level.rs    LevelData + .lev material-map loader           (1b)
│       ├── palette.rs  Palette + VGA/modern + display layers          (1c)
│       ├── sprite.rs   TGA sprite-bank loader                         (1d)
│       └── tc.rs       TC bundle (config, sounds, object defs)        (1e)
└── oracle-tests/     extended with one golden test per slice
```

The `assets` crate mirrors the C++ `io` layer and asset loaders. It stays free of
Bevy so it can be tested standalone and reused by both the future Bevy app and
the headless oracle tests.

## The oracle dumper, evolved

Step 0's dumper compiled standalone (`math.cpp` only). The asset loaders pull in
more of the engine (`Level::load` needs `Material`, `io`, `Settings`; sprites need
TGA + palette). So from slice 1b on, the C++ dumper becomes a **CMake-gated target
that links the `game` library** (the same pattern `tctool`/`videotool` use),
instead of a standalone `clang++` compile. The CMake integration that step 0
deferred is introduced here, behind an option (e.g. `OPENLIERO_BUILD_ORACLE_DUMP`).

## Dependencies we expect to need (decided per slice)

- **Deflate/miniz:** only if a format is compressed. The `.lev` material map is
  uncompressed, so 1a/1b need none. Introduce `flate2` (or a port) when a slice
  actually requires it.
- **TOML:** `tc.cfg` and profiles are TOML (1e). Introduce a TOML crate then.
- These land in the `assets` crate, never in `sim-core` (which stays dependency-free).

## Deliberately deferred

Writing assets back out (we only read), the original Liero `.exe` extraction
(`tctool`), replay/snapshot formats, and settings/profile TOML beyond what a slice
needs. Each is its own future concern.

## Next concrete artifact

Detailed spec for the first slice:
`2026-06-26-liero-rs-step1a-1b-io-and-material-map-design.md`.
