# Step 1, slice 1d — Sprites (TGA banks): design

Status: **draft** · 2026-06-26
Part of: `2026-06-26-liero-rs-step1-data-formats-overview.md`
Follows: `2026-06-26-liero-rs-step1c-palette-display-design.md`

Slice 1d of the Liero-rs rewrite: read OpenLiero's **sprite banks** from their
TGA files in Rust, **byte-identical to the C++ engine**. This adds a `sprite`
module to the `assets` crate (a small TGA parser + a sprite-bank view) and a new
C++ oracle dumper that runs the real `Common::load`. As always: the C++ engine is
the oracle; a golden differential test pins bit-exactness.

## Scope

In scope (locked, differential-tested against C++):

- **TGA parser** for OpenLiero's exact sprite-TGA dialect: 18-byte header,
  256-entry 24-bit BGR colour map quantized to the VGA 6-bit grid (`& 0xfc`),
  8-bit indexed pixels stored bottom-to-top.
- **Sprite banks**: `small` (7×7, 130 sprites), `large` (16×16, 110 sprites),
  `text` (4×4, 26 sprites), parsed from `sprites/{small,large,text}.tga`.
- **`exepal`**: the palette carried by `small.tga` (the stock palette), which the
  C++ engine reads via the same TGA path.

Explicitly out of scope (deferred):

- `font.tga` and the font glyph layout (`Common::load` does extra post-processing
  on it; fonts are a rendering concern, deferred to step 3 / a later slice).
- Sprite rendering, blitting, and the precomputed `worm_sprites`/`fire_cone_sprites`
  derived tables (rendering, step 3).
- RLE-compressed TGA (image type 9/10) — OpenLiero's sprite TGAs are uncompressed
  indexed (type 1); the parser rejects anything else.
- The rest of the TC bundle (sounds, weapon/object configs) — that is slice 1e.

## Background: the C++ format (oracle truth)

Verified against `src/game/common.cpp:242-299` (`ReadSpriteTga`),
`src/game/common.cpp:365-404` (sprite loading inside `Common::load`), and
`src/game/gfx/sprite.hpp`.

### `ReadSpriteTga` (`common.cpp:242-295`)

The 18-byte TGA header is read and every field validated (the C++ `CHECK(...)`
macro returns failure on mismatch):

| Offset | Bytes | Field | Required value |
|---|---|---|---|
| 0 | 1 | ID length | (any; skipped later) |
| 1 | 1 | colour-map type | `1` |
| 2 | 1 | image type | `1` (uncompressed indexed) |
| 3–4 | 2 LE | colour-map first entry | `0` |
| 5–6 | 2 LE | colour-map length | `256` |
| 7 | 1 | colour-map entry size (bits) | `24` |
| 8–9 | 2 LE | X origin | `0` |
| 10–11 | 2 LE | Y origin | `0` |
| 12–13 | 2 LE | image width | (read) |
| 14–15 | 2 LE | image height | (read) |
| 16 | 1 | bits per pixel | `8` |
| 17 | 1 | image descriptor | `0` |

Then: skip `id_len` bytes; **require** `image_width == dest_width` and
`image_height == dest_height` (for a bank, `dest_width = sprite_width`,
`dest_height = count * sprite_height`).

**Colour map (768 bytes):** read as 256 × BGR triples. When a palette is wanted
(only `small.tga` passes one), each channel is stored into an RGB `Color` with the
low two bits dropped: `entry.b = get() & 0xfc; entry.g = get() & 0xfc; entry.r =
get() & 0xfc;`. When no palette is wanted (`large.tga`, `text.tga`) the 768 bytes
are skipped. Either way the 768 bytes are consumed.

**Pixels:** read **bottom-to-top** — for `y` from `image_height-1` down to `0`,
read `image_width` bytes into `data[y*image_width..]`. So the in-memory buffer is
top-to-bottom row-major, `width × (count*height)`, and sprite `N` occupies
`data[N*sprite_size .. (N+1)*sprite_size]` where `sprite_size = width*height`.

### Bank loading (`common.cpp:365-404`)

```
large_sprites.Allocate(16, 16, 110);  ReadSpriteTga(large.tga, large_sprites, nullptr);
small_sprites.Allocate(7,  7,  130);  ReadSpriteTga(small.tga, small_sprites, &exepal);
text_sprites .Allocate(4,  4,  26);   ReadSpriteTga(text.tga,  text_sprites,  nullptr);
```

`exepal` is written only here (`common.cpp:376`) and never modified afterwards in
`Common::load` (line 388 only *reads* it: `modernpal = exepal`). So `exepal`
equals `small.tga`'s colour map after BGR→RGB + `& 0xfc`.

### Sim impact

None. Sprite pixels and `exepal` are rendering data; no simulation logic depends
on them. 1d is purely "read the bytes the same".

## Rust design

### Crate layout

```
rust/assets/src/
├── io.rs        (1a)
├── level.rs     (1b, 1c)
├── palette.rs   (1c)
├── sprite.rs    ← NEW (1d): Tga parser + SpriteSet bank view
└── lib.rs       (re-export sprite)
```

### `sprite.rs`

```rust
use crate::palette::Palette;

/// A parsed sprite TGA: the colour map + the de-flipped pixel buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tga {
    pub width: i32,
    pub height: i32,
    /// 256-entry colour map, BGR→RGB with the low 2 bits dropped (`& 0xfc`).
    pub palette: Palette,
    /// `width * height` palette indices, top-to-bottom row-major.
    pub pixels: Vec<u8>,
}

/// A sprite bank: `count` sprites of `width × height` palette indices, stored
/// back-to-back (`sprite N` at `data[N*width*height ..]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSet {
    pub width: i32,
    pub height: i32,
    pub count: i32,
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpriteError {
    /// A header field had a value OpenLiero's loader rejects.
    BadHeader,
    /// The buffer ended before the header, colour map, or pixels were complete.
    Truncated,
    /// The TGA dimensions did not match the requested bank layout.
    DimensionMismatch { want: (i32, i32), got: (i32, i32) },
}

impl Tga {
    /// Parse an OpenLiero sprite TGA. Mirrors `ReadSpriteTga`
    /// (`common.cpp:242`): validates the 18-byte header, reads the 256×BGR
    /// colour map (→RGB `& 0xfc`), and de-flips the bottom-to-top pixels.
    pub fn load(bytes: &[u8]) -> Result<Tga, SpriteError>;
}

impl SpriteSet {
    /// View a parsed TGA as a bank of `count` sprites of `sprite_width ×
    /// sprite_height`. Requires `tga.width == sprite_width` and
    /// `tga.height == count * sprite_height`.
    pub fn from_tga(
        tga: &Tga,
        sprite_width: i32,
        sprite_height: i32,
        count: i32,
    ) -> Result<SpriteSet, SpriteError>;

    /// Palette indices for sprite `frame` (`width*height` bytes).
    pub fn sprite(&self, frame: usize) -> &[u8];
}
```

Callers load the three banks like the C++ does:

```rust
let small = Tga::load(small_bytes)?;
let small_set = SpriteSet::from_tga(&small, 7, 7, 130)?;
let exepal = small.palette;            // stock palette
let large_set = SpriteSet::from_tga(&Tga::load(large_bytes)?, 16, 16, 110)?;
let text_set  = SpriteSet::from_tga(&Tga::load(text_bytes)?,  4,  4,  26)?;
```

Idiomatic Rust: `from_le_bytes`/slicing, typed errors, a single forward scan —
not a port of C++'s streaming `Reader`. The palette is always parsed (the bytes
are present in every file); callers ignore it for `large`/`text`, exactly as C++
does. The observable values — pixel buffer and colour map — are what the golden
pins.

## The oracle: real `Common::load`

`ReadSpriteTga` is `static` (file-local), so it cannot be called directly.
Instead a new dumper runs the genuine public path `Common::load(FsNode)`, the same
way the existing tests do (`test_determinism.cpp:26`,
`framehash_main.cpp:66`):

```cpp
// src/tools/oracle_dump/sprite_dump.cpp  (links `game`)
Common common;
common.load(FsNode(argv[1]));          // argv[1] = data/TC/openliero
// emit FNV-1a of small/large/text sprite data + exepal channels
```

This exercises the real `ReadSpriteTga` end to end. A new CMake target
`oracle_dump_sprite` is added beside `oracle_dump_level`/`oracle_dump_palette`
under the `OPENLIERO_BUILD_ORACLE_DUMP` option. The dumper takes the TC directory
as `argv[1]` (so it does not depend on the working directory) and the output path
as `argv[2]`.

### Golden format

`golden/sprite.txt`, one line per bank plus the palette, each a label + metadata
+ FNV-1a digest of the raw little-endian field bytes (same FNV helper as the level
golden):

```
small 130 7 7 <data_hash>
large 110 16 16 <data_hash>
text 26 4 4 <data_hash>
exepal <palette_hash>
```

- `<data_hash>` = FNV-1a over the bank's `data` bytes (palette indices).
- `<palette_hash>` = FNV-1a over `exepal`'s 256 × (r,g,b) = 768 bytes (no 4th
  byte), identical to the palette-hash convention from slice 1c.

The Rust test reads the three real TGA files, parses them, and reproduces every
digest (and the metadata).

### Oracle input

The real shipped assets: `data/TC/openliero/sprites/{small,large,text}.tga`.
No synthetic inputs are needed — the formats are fixed and the real files
exercise every path (palette-kept for `small`, palette-skipped for `large`/`text`,
all three the bottom-to-top de-flip). Unit tests in `sprite.rs` cover the error
paths (bad header field, truncation, dimension mismatch) with small synthetic
buffers.

## Testing

1. **Unit tests** in `sprite.rs`: a hand-built minimal valid TGA round-trips
   (header → known pixels de-flipped correctly, palette BGR→RGB `& 0xfc`); each
   rejected header field → `BadHeader`; short buffer → `Truncated`; wrong
   dimensions → `DimensionMismatch`.
2. **Golden differential test** `oracle-tests/tests/sprite_golden.rs`: the three
   real TGA banks + `exepal` reproduce the C++ digests. Regenerated by
   `gen_sprite_golden.sh` against the real C++ build (local/manual, like the level
   and palette goldens).
3. CI (`rust.yml`) runs `cargo test --workspace` against the committed golden; it
   does not rebuild the C++ oracle.

**Done when:** the full Rust workspace suite is green and every 1d golden digest
matches C++ bit-for-bit.

## Modernization-charter check

- **Locked / bit-exact:** sprite pixel buffers (after the bottom-to-top de-flip)
  and the `exepal` colour map (after BGR→RGB `& 0xfc`) reproduce C++ exactly
  (golden-proven against the real `Common::load`).
- **Free to modernize:** a `Tga` value type + a `SpriteSet` view instead of
  `Allocate`+out-pointer; typed `Result`/error enum; slicing instead of a
  streaming `Reader`. None of it is observable in the parsed values.
- **Sim impact:** none — sprites and `exepal` are rendering data.

## Next concrete artifact

Implementation plan: `docs/superpowers/plans/2026-06-26-liero-rs-step1d-sprites.md`.
