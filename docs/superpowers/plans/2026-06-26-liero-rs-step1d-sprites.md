# Step 1d — Sprites (TGA banks) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read OpenLiero's sprite banks (`small`/`large`/`text`) and the `exepal` palette from their TGA files in Rust, byte-identical to the C++ engine.

**Architecture:** A new `sprite.rs` in the `assets` crate provides a `Tga` parser (OpenLiero's exact sprite-TGA dialect) and a `SpriteSet` bank view. Correctness is proven by a new C++ oracle dumper that runs the real `Common::load` and a golden differential test that reproduces its FNV-1a digests.

**Tech Stack:** Rust (`assets`, `oracle-tests`), C++ oracle dumper (`oracle_dump_sprite`, links `game`), CMake option `OPENLIERO_BUILD_ORACLE_DUMP`, FNV-1a digests.

## Global Constraints

- **Bit-exact vs C++.** Source of truth: `src/game/common.cpp:242-299` (`ReadSpriteTga`) and `:365-404` (bank loading). Sprite pixels (after the bottom-to-top de-flip) and `exepal` (after BGR→RGB `& 0xfc`) must reproduce C++ exactly.
- **TGA header (18 bytes), all validated:** byte1 colour-map-type `==1`; byte2 image-type `==1`; bytes3-4 LE colour-map-first `==0`; bytes5-6 LE colour-map-length `==256`; byte7 colour-map-entry-size `==24`; bytes8-9 LE x-origin `==0`; bytes10-11 LE y-origin `==0`; bytes12-13 LE width; bytes14-15 LE height; byte16 bpp `==8`; byte17 descriptor `==0`. byte0 = id_len (skipped). Then skip `id_len`, then **require** `width==dest_width && height==dest_height`.
- **Colour map:** 256 × BGR triples (768 bytes). Stored as RGB with low 2 bits dropped: `b=get()&0xfc; g=get()&0xfc; r=get()&0xfc`. Always consumed (skipped when unused).
- **Pixels:** bottom-to-top — for `y` from `height-1` down to `0`, the next `width` bytes go to row `y`. In-memory buffer is top-to-bottom row-major, `width × (count*height)`; sprite `N` at `data[N*width*height ..]`.
- **Banks:** `small` 7×7 ×130 (palette kept → `exepal`); `large` 16×16 ×110 (palette skipped); `text` 4×4 ×26 (palette skipped).
- **`exepal`** = `small.tga`'s colour map after BGR→RGB `& 0xfc`; never modified after load (`common.cpp:376` writes, `:388` only reads).
- **FNV-1a (64-bit):** seed `0xcbf29ce484222325`, prime `0x100000001b3` — identical helper on both sides. Palette hash = 256 × (r,g,b) = 768 bytes, no 4th byte (same as slice 1c).
- **Idiomatic Rust, not a port:** `from_le_bytes`/slicing, typed `Result`/error enum; do NOT port C++'s streaming `Reader`.
- **No Bevy** in `assets`. **Golden regeneration is LOCAL/MANUAL** (full C++ build); CI runs `cargo test --workspace` against the committed golden. PRESET defaults to `macos-arm64`.
- **No AI/"Generated with" taglines** in commits. C++ matches the existing `level_dump.cpp` Google/100-col style.

## File Structure

- `rust/assets/src/sprite.rs` — NEW: `Tga`, `SpriteSet`, `SpriteError`, `Tga::load`, `SpriteSet::from_tga`/`sprite`.
- `rust/assets/src/lib.rs` — MODIFY: `pub mod sprite;`.
- `src/tools/oracle_dump/sprite_dump.cpp` — NEW: runs real `Common::load`, dumps bank + exepal digests.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_sprite` target inside the existing `OPENLIERO_BUILD_ORACLE_DUMP` block.
- `rust/oracle-tests/gen_sprite_golden.sh` — NEW: regenerate sprite golden.
- `rust/oracle-tests/golden/sprite.txt` — NEW: committed golden.
- `rust/oracle-tests/tests/sprite_golden.rs` — NEW: sprite differential test.

---

### Task 1: `sprite.rs` — TGA parser + sprite-bank view

Pure Rust; unit-test the header validation, palette transform, and de-flip directly.

**Files:**
- Create: `rust/assets/src/sprite.rs`
- Modify: `rust/assets/src/lib.rs`
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::palette::{Palette, Color}` (slice 1c).
- Produces:
  - `pub struct Tga { pub width: i32, pub height: i32, pub palette: Palette, pub pixels: Vec<u8> }` (derives `Debug, Clone, PartialEq, Eq`)
  - `pub struct SpriteSet { pub width: i32, pub height: i32, pub count: i32, pub data: Vec<u8> }` (derives `Debug, Clone, PartialEq, Eq`)
  - `pub enum SpriteError { BadHeader, Truncated, DimensionMismatch { want: (i32, i32), got: (i32, i32) } }` (derives `Debug, PartialEq, Eq`)
  - `Tga::load(bytes: &[u8]) -> Result<Tga, SpriteError>`
  - `SpriteSet::from_tga(tga: &Tga, sprite_width: i32, sprite_height: i32, count: i32) -> Result<SpriteSet, SpriteError>`
  - `SpriteSet::sprite(&self, frame: usize) -> &[u8]`

- [ ] **Step 1: Write `sprite.rs` with the implementation and unit tests**

```rust
//! Sprite-bank loading from OpenLiero's TGA files. The TGA dialect and the
//! bottom-to-top pixel de-flip mirror C++ `ReadSpriteTga` (`common.cpp:242`);
//! the implementation is idiomatic Rust, not a port of the streaming reader.

use crate::palette::{Color, Palette};

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

/// Why a sprite TGA failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum SpriteError {
    /// A header field had a value OpenLiero's loader rejects.
    BadHeader,
    /// The buffer ended before the header, colour map, or pixels were complete.
    Truncated,
    /// The TGA dimensions did not match the requested bank layout.
    DimensionMismatch { want: (i32, i32), got: (i32, i32) },
}

const HEADER_LEN: usize = 18;
const PALETTE_BYTES: usize = 256 * 3;

fn le_u16(bytes: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([bytes[o], bytes[o + 1]])
}

impl Tga {
    /// Parse an OpenLiero sprite TGA (`ReadSpriteTga`, `common.cpp:242`).
    pub fn load(bytes: &[u8]) -> Result<Tga, SpriteError> {
        if bytes.len() < HEADER_LEN {
            return Err(SpriteError::Truncated);
        }
        let id_len = bytes[0] as usize;
        // Validate every fixed header field, exactly as the C++ CHECK(...) chain.
        if bytes[1] != 1            // colour-map type
            || bytes[2] != 1        // image type (uncompressed indexed)
            || le_u16(bytes, 3) != 0    // colour-map first entry
            || le_u16(bytes, 5) != 256  // colour-map length
            || bytes[7] != 24       // colour-map entry size (bits)
            || le_u16(bytes, 8) != 0    // x origin
            || le_u16(bytes, 10) != 0   // y origin
            || bytes[16] != 8       // bits per pixel
            || bytes[17] != 0       // image descriptor
        {
            return Err(SpriteError::BadHeader);
        }
        let width = le_u16(bytes, 12) as i32;
        let height = le_u16(bytes, 14) as i32;

        let mut pos = HEADER_LEN + id_len;

        // Colour map: 256 × BGR, stored RGB with the low 2 bits dropped.
        if bytes.len() < pos + PALETTE_BYTES {
            return Err(SpriteError::Truncated);
        }
        let mut entries = [Color::default(); 256];
        for e in entries.iter_mut() {
            e.b = bytes[pos] & 0xfc;
            e.g = bytes[pos + 1] & 0xfc;
            e.r = bytes[pos + 2] & 0xfc;
            pos += 3;
        }
        let palette = Palette { entries };

        // Pixels: bottom-to-top. The first `width` bytes are the bottom row.
        let cells = width as usize * height as usize;
        if bytes.len() < pos + cells {
            return Err(SpriteError::Truncated);
        }
        let mut pixels = vec![0u8; cells];
        let w = width as usize;
        for y in (0..height as usize).rev() {
            pixels[y * w..y * w + w].copy_from_slice(&bytes[pos..pos + w]);
            pos += w;
        }

        Ok(Tga { width, height, palette, pixels })
    }
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
    ) -> Result<SpriteSet, SpriteError> {
        let want = (sprite_width, count * sprite_height);
        if tga.width != want.0 || tga.height != want.1 {
            return Err(SpriteError::DimensionMismatch {
                want,
                got: (tga.width, tga.height),
            });
        }
        Ok(SpriteSet {
            width: sprite_width,
            height: sprite_height,
            count,
            data: tga.pixels.clone(),
        })
    }

    /// Palette indices for sprite `frame` (`width*height` bytes).
    pub fn sprite(&self, frame: usize) -> &[u8] {
        let size = self.width as usize * self.height as usize;
        &self.data[frame * size..frame * size + size]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a minimal valid sprite TGA: `width × height`, id_len bytes of ID,
    // a 256×BGR colour map (channel = index%modulo), and bottom-to-top pixels
    // where the byte value encodes its file order (so the de-flip is testable).
    fn make_tga(width: i32, height: i32, id_len: u8) -> Vec<u8> {
        let mut b = vec![
            id_len, 1, 1, // id_len, cmap type, image type
            0, 0, // cmap first
            0, 1, // cmap length = 256 (LE)
            24, // cmap entry size
            0, 0, 0, 0, // x/y origin
        ];
        b.extend_from_slice(&(width as u16).to_le_bytes());
        b.extend_from_slice(&(height as u16).to_le_bytes());
        b.push(8); // bpp
        b.push(0); // descriptor
        for _ in 0..id_len {
            b.push(0xEE); // ID field (skipped)
        }
        for i in 0..256 * 3 {
            b.push((i % 64) as u8); // colour map BGR
        }
        // Pixels in FILE order (bottom row first). Encode file index in value.
        for i in 0..(width as usize * height as usize) {
            b.push(i as u8);
        }
        b
    }

    #[test]
    fn parses_and_deflips_pixels() {
        // 2 wide × 4 tall. File order rows: bottom..top. After de-flip, the
        // first file row (values 0,1) lands at the LAST in-memory row.
        let buf = make_tga(2, 4, 0);
        let tga = Tga::load(&buf).unwrap();
        assert_eq!((tga.width, tga.height), (2, 4));
        assert_eq!(tga.pixels.len(), 8);
        // file bytes: [0,1, 2,3, 4,5, 6,7] read bottom-to-top.
        // row y=3 gets file bytes 0,1; y=2 -> 2,3; y=1 -> 4,5; y=0 -> 6,7.
        assert_eq!(tga.pixels, vec![6, 7, 4, 5, 2, 3, 0, 1]);
    }

    #[test]
    fn palette_is_bgr_to_rgb_masked() {
        let tga = Tga::load(&make_tga(2, 2, 0)).unwrap();
        // entry 0: file bytes 0,1,2 = b,g,r -> r=(2&0xfc),g=(1&0xfc),b=(0&0xfc)
        assert_eq!(tga.palette.entries[0], Color { r: 0, g: 0, b: 0 });
        // entry 1: file bytes 3,4,5 -> b=3&0xfc=0, g=4&0xfc=4, r=5&0xfc=4
        assert_eq!(tga.palette.entries[1], Color { r: 4, g: 4, b: 0 });
    }

    #[test]
    fn id_field_is_skipped() {
        // Same pixels regardless of id_len.
        let a = Tga::load(&make_tga(2, 2, 0)).unwrap();
        let b = Tga::load(&make_tga(2, 2, 5)).unwrap();
        assert_eq!(a.pixels, b.pixels);
        assert_eq!(a.palette, b.palette);
    }

    #[test]
    fn from_tga_splits_into_banks() {
        // 2×4 image = 2 sprites of 2×2.
        let tga = Tga::load(&make_tga(2, 4, 0)).unwrap();
        let set = SpriteSet::from_tga(&tga, 2, 2, 2).unwrap();
        assert_eq!(set.count, 2);
        assert_eq!(set.data.len(), 8);
        assert_eq!(set.sprite(0), &[6, 7, 4, 5]); // top sprite
        assert_eq!(set.sprite(1), &[2, 3, 0, 1]); // bottom sprite
    }

    #[test]
    fn rejects_bad_header_field() {
        let mut buf = make_tga(2, 2, 0);
        buf[2] = 9; // image type 9 (RLE) not supported
        assert_eq!(Tga::load(&buf), Err(SpriteError::BadHeader));
    }

    #[test]
    fn rejects_truncated() {
        let buf = make_tga(2, 2, 0);
        assert_eq!(Tga::load(&buf[..10]), Err(SpriteError::Truncated));
        // header+palette OK but pixels cut short:
        assert_eq!(
            Tga::load(&buf[..HEADER_LEN + PALETTE_BYTES + 1]),
            Err(SpriteError::Truncated)
        );
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let tga = Tga::load(&make_tga(2, 4, 0)).unwrap();
        assert_eq!(
            SpriteSet::from_tga(&tga, 7, 7, 130),
            Err(SpriteError::DimensionMismatch { want: (7, 910), got: (2, 4) })
        );
    }
}
```

Add to `rust/assets/src/lib.rs` after the `pub mod palette;` line:

```rust
pub mod sprite;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets sprite`
Expected: all `sprite` tests PASS.

- [ ] **Step 3: Run the full assets suite (no regressions)**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: all prior assets tests (level/palette) still PASS plus the new sprite tests.

- [ ] **Step 4: Commit**

```bash
git add rust/assets/src/sprite.rs rust/assets/src/lib.rs
git commit -m "feat(assets): TGA sprite-bank loader (small/large/text + exepal)"
```

---

### Task 2: Sprite golden — differential test vs real `Common::load`

Prove the three real banks + `exepal` match the C++ engine bit-for-bit.

**Files:**
- Create: `src/tools/oracle_dump/sprite_dump.cpp`
- Modify: `CMakeLists.txt` (inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block)
- Create: `rust/oracle-tests/gen_sprite_golden.sh`
- Create: `rust/oracle-tests/golden/sprite.txt` (generated)
- Create: `rust/oracle-tests/tests/sprite_golden.rs`

**Interfaces:**
- Consumes: `Tga::load`, `SpriteSet::from_tga` (Task 1); the real C++ `Common` fields `small_sprites`/`large_sprites`/`text_sprites` (`.data`, `.count`, `.width`, `.height`) and `exepal`.
- Produces: golden file — 3 bank lines `<label> <count> <w> <h> <data_hash>` + one `exepal <palette_hash>` line.

- [ ] **Step 1: Write the C++ sprite dumper**

Create `src/tools/oracle_dump/sprite_dump.cpp`:

```cpp
// Generates golden digests for the Rust sprite differential test by running the
// REAL C++ Common::load (which calls ReadSpriteTga). Links the `game` library;
// built via the OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the
// default build. Usage: oracle_dump_sprite <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// Hash a palette's r,g,b channels (256 entries * 3 bytes), matching Rust
// Color { r, g, b } (C++'s 4th `unused` byte is not hashed).
uint64_t HashPalette(Palette const& p) {
  std::vector<unsigned char> bytes;
  bytes.reserve(256 * 3);
  for (auto const& e : p.entries) {
    bytes.push_back(e.r);
    bytes.push_back(e.g);
    bytes.push_back(e.b);
  }
  return Fnv1a(bytes);
}

void DumpBank(std::FILE* out, char const* label, SpriteSet const& ss) {
  std::vector<unsigned char> bytes(ss.data.begin(), ss.data.end());
  std::fprintf(out, "%s %d %d %d %016llx\n", label, ss.count, ss.width, ss.height,
               static_cast<unsigned long long>(Fnv1a(bytes)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_sprite <tc-dir> <out.txt>\n");
    return 1;
  }
  auto common = std::make_shared<Common>();
  common->load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  DumpBank(out, "small", common->small_sprites);
  DumpBank(out, "large", common->large_sprites);
  DumpBank(out, "text", common->text_sprites);
  std::fprintf(out, "exepal %016llx\n",
               static_cast<unsigned long long>(HashPalette(common->exepal)));
  std::fclose(out);
  return 0;
}
```

- [ ] **Step 2: Register the CMake target**

In `CMakeLists.txt`, inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block (after the `oracle_dump_palette` lines), add:

```cmake
  add_executable(oracle_dump_sprite src/tools/oracle_dump/sprite_dump.cpp)
  target_link_libraries(oracle_dump_sprite PRIVATE game)
```

- [ ] **Step 3: Write the regeneration script**

Create `rust/oracle-tests/gen_sprite_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/sprite.txt by running the REAL C++ Common::load (which calls
# ReadSpriteTga). Needs the full C++ build (links the `game` target), so this is
# a LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET
# for other platforms (e.g. linux-x64). Run from the repo root so the TC dir
# resolves the same way the in-tree tests do.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_sprite
(
  cd "$ROOT"
  "build/$PRESET/Release/oracle_dump_sprite" \
    "data/TC/openliero" \
    "rust/oracle-tests/golden/sprite.txt"
)
echo "wrote rust/oracle-tests/golden/sprite.txt"
```

Make it executable:

```bash
chmod +x rust/oracle-tests/gen_sprite_golden.sh
```

- [ ] **Step 4: Generate the golden**

Run: `bash rust/oracle-tests/gen_sprite_golden.sh`
Expected: prints `wrote rust/oracle-tests/golden/sprite.txt`; the file has 4 lines:
`small 130 7 7 <hash>`, `large 110 16 16 <hash>`, `text 26 4 4 <hash>`,
`exepal <hash>`.

- [ ] **Step 5: Write the Rust golden test**

Create `rust/oracle-tests/tests/sprite_golden.rs`:

```rust
//! Differential test for the sprite loader against the C++ oracle. The golden
//! (3 bank lines `<label> <count> <w> <h> <data_hash>` + `exepal <hash>`) is
//! produced by the real C++ `Common::load`; the Rust loader must reproduce every
//! digest and bank shape from the same shipped TGA files.

use assets::palette::Palette;
use assets::sprite::{SpriteSet, Tga};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hash_palette(p: &Palette) -> u64 {
    let mut b = Vec::with_capacity(256 * 3);
    for e in &p.entries {
        b.push(e.r);
        b.push(e.g);
        b.push(e.b);
    }
    fnv1a(&b)
}

fn read_tga(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../data/TC/openliero/sprites/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn sprites_match_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sprite.txt"
    ))
    .unwrap();

    // (label, file, sprite_width, sprite_height, count)
    let banks = [
        ("small", "small.tga", 7, 7, 130),
        ("large", "large.tga", 16, 16, 110),
        ("text", "text.tga", 4, 4, 26),
    ];

    let mut lines = golden.lines();

    // small.tga carries exepal; keep its parsed Tga to check the palette line.
    let mut small_palette: Option<Palette> = None;

    for (label, file, sw, sh, count) in banks {
        let line = lines.next().unwrap_or_else(|| panic!("missing line for {label}"));
        let mut it = line.split_whitespace();
        assert_eq!(it.next().unwrap(), label, "label order");
        let want_count: i32 = it.next().unwrap().parse().unwrap();
        let want_w: i32 = it.next().unwrap().parse().unwrap();
        let want_h: i32 = it.next().unwrap().parse().unwrap();
        let want_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();

        let tga = Tga::load(&read_tga(file)).unwrap();
        let set = SpriteSet::from_tga(&tga, sw, sh, count).unwrap();
        if label == "small" {
            small_palette = Some(tga.palette.clone());
        }

        assert_eq!(set.count, want_count, "{label} count");
        assert_eq!(set.width, want_w, "{label} width");
        assert_eq!(set.height, want_h, "{label} height");
        assert_eq!(fnv1a(&set.data), want_hash, "{label} data hash");
    }

    // exepal line: small.tga's colour map.
    let line = lines.next().expect("missing exepal line");
    let mut it = line.split_whitespace();
    assert_eq!(it.next().unwrap(), "exepal");
    let want_pal = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
    assert_eq!(hash_palette(&small_palette.unwrap()), want_pal, "exepal hash");

    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 6: Run the full workspace suite**

Run: `cargo test --manifest-path rust/Cargo.toml --workspace`
Expected: ALL tests PASS (sim-core goldens, assets unit incl. sprite, palette_golden, level_golden, sprite_golden).

- [ ] **Step 7: Commit**

```bash
git add src/tools/oracle_dump/sprite_dump.cpp CMakeLists.txt \
  rust/oracle-tests/gen_sprite_golden.sh rust/oracle-tests/golden/sprite.txt \
  rust/oracle-tests/tests/sprite_golden.rs
git commit -m "test(oracle): sprite-bank differential test vs C++ Common::load"
```

---

## Self-Review

**Spec coverage:**
- TGA parser (header validation, BGR→RGB `& 0xfc`, bottom-to-top de-flip) → Task 1 + golden Task 2. ✓
- Three banks small/large/text → Task 1 (`from_tga`) + Task 2 golden. ✓
- `exepal` from small.tga → Task 1 (`tga.palette`) + Task 2 golden `exepal` line. ✓
- Oracle via real `Common::load(FsNode)` → Task 2. ✓
- font.tga, rendering, RLE deferred → not implemented (out of scope). ✓

**Placeholder scan:** No TBD/TODO; all code complete. ✓

**Type consistency:** `Tga`/`SpriteSet`/`SpriteError` names and fields consistent across Tasks 1–2. Golden line shape (`label count w h hash` + `exepal hash`) identical in C++ `DumpBank`/`main` and the Rust parser. FNV helper + palette-hash byte order identical to slice 1c's convention. ✓

**Note on `Common::load` robustness:** it also loads sounds, configs, and `font.tga` from the TC dir. The shipped `data/TC/openliero` is complete and the in-tree tests (`test_determinism.cpp`) call `common->load` on it routinely, so the dumper is robust. If `Common::load` fails for an environment reason, that surfaces as the dumper exiting non-zero during golden regeneration (a BLOCKED signal), not a silent wrong golden.
