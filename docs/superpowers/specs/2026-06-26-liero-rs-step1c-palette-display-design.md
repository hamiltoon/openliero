# Step 1, slice 1c — Palette + display layers: design

Status: **draft for review** · 2026-06-26
Part of: `2026-06-26-liero-rs-step1-data-formats-overview.md`
Follows: `2026-06-26-liero-rs-step1a-1b-io-and-material-map-design.md`

Slice 1c of the Liero-rs rewrite: read OpenLiero's **palette** and **MODERNLV
display layers** in Rust, **byte-identical to the C++ engine**. This extends the
`assets` crate's `.lev` loader (1a+1b loaded only the material map) and adds a
standalone `Palette` module. As always: the C++ engine is the oracle; a C++
dumper writes golden digests and the Rust loader must reproduce them bit-for-bit.

## Scope

In scope (locked, differential-tested against C++):

- **`Palette` module** — VGA 6→8-bit read (`(v & 63) << 2`), full 8-bit read,
  and the VGA-grid → full-range expansion (`e |= e >> 6`).
- **POWERLEVEL block** in `.lev` — a custom VGA 6-bit palette (256×3 bytes)
  following the material map.
- **MODERNLV block** in `.lev` — true-color display layers (`display_data`
  ARGB + `display_valid` flags) plus the optional animation extension (ramp
  table + per-pixel `display_anim` indices), **read byte-exact only**.

Explicitly out of scope (deferred):

- **Animation resolve** — the per-tick colour function
  `phase = offset + (cycles >> shift); colors[phase % len]` is *rendering*, not a
  data format. It does not affect the simulation. Deferred to step 3 (rendering).
- The display layers' integration into a renderer, `modern.pal` *file discovery*
  in the data tree (we read the format; wiring it into asset resolution is later),
  sprite palettes (1d), and TC config (1e).

## Background: the C++ formats (oracle truth)

Verified against `src/game/gfx/palette.cpp`, `src/game/gfx/color.hpp`, and
`src/game/level.cpp:229–395`.

### Palette (`src/game/gfx/palette.{hpp,cpp}`)

`Palette` holds `Color entries[256]`; `Color = {u8 r, g, b, unused}`. On disk a
palette is **256×3 bytes (RGB, no alpha)**. Three operations matter:

| C++ op | Bytes | Per-channel transform |
|---|---|---|
| `Read` (VGA 6-bit) | 768 | `entry = (raw & 63) << 2` |
| `ReadFull` (8-bit) | 768 | `entry = raw` |
| `ExpandToFullRange` | — | `e \|= e >> 6` (maps VGA grid 252→255) |

`exepal` is the stock palette (VGA-grid). `modernpal` is loaded from a
`modern.pal` file via `ReadFull`, or, if absent, `modernpal = exepal` then
`ExpandToFullRange()`.

### `.lev` extension blocks (`src/game/level.cpp`)

After the material map (1b), `Level::load` probes for two optional blocks, in
this order:

1. **POWERLEVEL** — 10-byte magic `"POWERLEVEL"`, then a **VGA 6-bit palette**
   read via `Palette::Read` (768 bytes). Sets the level's custom palette.
   In C++ this probe is gated by `settings.load_powerlevel_palette`; when it is
   `false` the block is left unread and the bytes stay in the stream. **Rust
   always parses POWERLEVEL** (the loader's meaningful behaviour, equivalent to
   the flag being `true`); `load()` takes no settings argument. The 1c golden
   therefore runs the C++ dumper with `load_powerlevel_palette = true` so both
   sides parse the block (the 1b dumper left it `false` because 1b ignored the
   block entirely).
2. **MODERNLV** — 8-byte magic `"MODERNLV"`, then, with `cells = width*height`:
   - `display_data`: `cells × u32` ARGB, **little-endian**
   - `display_valid`: `cells × u8` (1 = authored colour, 0 = fall back to palette)
   - **Animation extension (optional, read with `TryGet`; a file may end before it):**
     - `ramp_count`: `u8` (0 ⇒ no animation)
     - for each ramp: `shift` (`u8`), `color_count` (`u16` LE, must be `1..=4096`),
       then `color_count × u32` ARGB LE
     - `display_anim`: `cells × u8` ramp indices (each must be `<= ramp_count`)
   - **Validity rule (must match C++):** the animation is accepted *only if* every
     ramp parses fully **and** `display_anim` reads fully **and** every index is
     `<= ramp_count`. If any check fails, the ramps and anim map are **discarded**
     (left empty) while `display_data`/`display_valid` are kept. Partial reads at
     EOF before `ramp_count` simply mean "no animation".

The probe-buffer carry-over between the POWERLEVEL and MODERNLV probes (C++
reuses unconsumed bytes) is a streaming-reader artifact; the Rust slice-based
loader tracks a single cursor and does not need to mirror it. The observable
result — which bytes belong to which block — is what the golden pins.

## Rust design

### Crate layout

A new module beside `io.rs`/`level.rs`:

```
rust/assets/src/
├── io.rs        (1a, unchanged)
├── level.rs     (1b; extended here with POWERLEVEL + MODERNLV parsing)
├── palette.rs   ← NEW (1c): Palette + Color, read/full/expand
└── lib.rs       (re-export palette)
```

### `palette.rs`

```rust
/// One palette entry. Mirrors the on-disk RGB triple; C++'s 4th `unused`
/// byte is an in-memory padding detail we do not keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette { pub entries: [Color; 256] }

#[derive(Debug, PartialEq, Eq)]
pub enum PaletteError { Truncated }   // < 768 bytes

impl Palette {
    /// VGA 6-bit read: `(v & 63) << 2` per channel. (C++ `Palette::Read`.)
    pub fn load_vga(bytes: &[u8]) -> Result<Palette, PaletteError>;
    /// Full 8-bit read: channels verbatim. (C++ `Palette::ReadFull`.)
    pub fn load_full(bytes: &[u8]) -> Result<Palette, PaletteError>;
    /// In place: `e |= e >> 6` per channel. (C++ `ExpandToFullRange`.)
    pub fn expand_to_full_range(&mut self);
}
```

`load_*` consume exactly 768 bytes (the caller slices); shorter ⇒ `Truncated`.

### `level.rs` — extended `LevelData`

```rust
pub struct LevelData {
    pub width: i32,
    pub height: i32,
    pub material_id: Vec<u8>,
    pub palette: Option<Palette>,        // Some(_) iff a POWERLEVEL block was present
    pub display: Option<DisplayLayers>,  // Some(_) iff a MODERNLV block was present
}

pub struct DisplayLayers {
    pub data: Vec<u32>,        // cells, ARGB
    pub valid: Vec<u8>,        // cells, 0/1
    pub ramps: Vec<ArgbRamp>,  // empty unless a valid animation block followed
    pub anim: Vec<u8>,         // empty unless ramps are present; else cells
}

pub struct ArgbRamp { pub shift: u8, pub colors: Vec<u32> }
```

`load()` keeps its current return type and material-map behaviour; after reading
the material map it advances a cursor and parses the optional blocks. The new
fields are additive, so existing 1b callers that ignore them still compile. C++
mostly *degrades gracefully* (drops the animation, keeps display data) rather
than erroring, so 1c mirrors that: a malformed/short animation is **not** an
error — it yields empty `ramps`/`anim`. Only a buffer too short for the declared
material map remains `LevelError::Truncated`, exactly as in 1b.

## The oracle, extended

`src/tools/oracle_dump/level_dump.cpp` gains palette + display digests, and a new
POWERLEVEL synthetic input. It already links `game` and runs the real
`Level::load`, so it sees the true POWERLEVEL/MODERNLV parsing.

Per input, the golden line is extended (kept FNV-1a over the raw little-endian
field bytes, matching 1b's style) with hashes of, when present:

- the palette entries (POWERLEVEL input),
- `display_data`, `display_valid`,
- the ramp table (shift + colors, serialized LE) and `display_anim`.

A small, explicit text format (e.g. `w h mat_hash pal_hash dd_hash dv_hash
ramp_hash anim_hash`, with a sentinel like `-` for absent fields) keeps the Rust
side a straightforward line parse, as in `level_golden.rs`.

A separate **palette golden** (`oracle_dump_palette`, or a section of the level
dumper) covers the standalone `Palette` ops on synthetic 768-byte buffers:
`load_vga`, `load_full`, and `load_vga` + `expand_to_full_range`. The C++ side
calls the real `Palette::Read`/`ReadFull`/`ExpandToFullRange`.

### Oracle inputs

| Input | Exercises |
|---|---|
| `data/.../modern_test.lev` (existing) | legacy material map + MODERNLV display + 1 animation ramp |
| synthetic OLLEVEL2 + **POWERLEVEL** | custom VGA palette parse (6→8), `palette: Some` |
| synthetic OLLEVEL2 + **MODERNLV**, no anim | display data/valid only, `ramps` empty |
| synthetic OLLEVEL2 + MODERNLV + **bad anim** | graceful degrade (display kept, ramps dropped) |
| synthetic 768-byte palette buffers | `load_vga` / `load_full` / `expand_to_full_range` |

All synthetic inputs are generated identically on both sides (the C++ dumper and
the Rust test build the same bytes), exactly as 1b does with `MakeLegacy`/
`MakeOllevel2`.

## Testing

Per the TDD + oracle discipline (charter):

1. **Unit tests** in `palette.rs` and `level.rs` pin the byte math and the
   block-parsing edge cases (no POWERLEVEL, POWERLEVEL only, MODERNLV only,
   MODERNLV + good anim, MODERNLV + truncated anim, bad ramp index).
2. **Golden differential tests** in `oracle-tests` extend `level_golden.rs` and
   add a palette golden, regenerated by `gen_level_golden.sh` (and a palette
   counterpart) against the real C++ build.
3. CI (`rust.yml`) runs the lightweight Rust side (unit + golden compare); golden
   regeneration stays a local/manual step needing the full C++ build, as in 1b.

**Done when:** the full Rust suite is green and every 1c golden digest matches
C++ bit-for-bit across all inputs above.

## Modernization-charter check

- **Locked / bit-exact:** all parsed bytes — palette channels after 6→8, ARGB
  display data, ramp tables, anim indices — reproduce C++ exactly (golden-proven).
- **Free to modernize:** `Color` drops the `unused` padding byte; a single cursor
  replaces C++'s probe-buffer carry-over; typed `Option`/`Result`; `from_le_bytes`
  and slicing instead of a streaming `Reader`. None of this is observable in the
  parsed values the golden pins.
- **Sim impact:** none of 1c feeds the deterministic simulation. The palette and
  display layers are rendering data; the animation is rendering-only and its
  resolve is deferred. 1c is purely "read the bytes the same".

## Next concrete artifact

Implementation plan: `docs/superpowers/plans/2026-06-26-liero-rs-step1c-palette-display.md`
(via the writing-plans skill).
