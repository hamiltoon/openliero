# Step 1c — Palette + display layers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read OpenLiero's palette and MODERNLV display layers in Rust, byte-identical to the C++ engine, extending the `assets` crate's `.lev` loader and adding a standalone `Palette` module.

**Architecture:** A new `palette.rs` holds `Palette`/`Color` with the three byte transforms (VGA 6→8, full 8-bit, expand). `level.rs` gains optional `palette`/`display` fields parsed after the material map. Correctness is proven by extending the C++ oracle dumper (real `Palette::*` and `Level::load`) and matching its FNV-1a digests in Rust golden tests.

**Tech Stack:** Rust (`assets`, `oracle-tests` crates), C++ oracle dumper (`oracle_dump_level`, new `oracle_dump_palette`), CMake option `OPENLIERO_BUILD_ORACLE_DUMP`, FNV-1a digests.

## Global Constraints

- **Bit-exact vs C++:** every parsed value (palette channels after 6→8, ARGB display data, ramp tables, anim indices) must reproduce the C++ engine exactly, proven by golden digests. Source of truth: `src/game/gfx/palette.cpp`, `src/game/level.cpp:229–395`.
- **Idiomatic Rust, not a port:** use `from_le_bytes`/slicing and typed `Result`/`Option`; do NOT port C++'s streaming `io::Reader`. `Color` drops C++'s 4th `unused` padding byte.
- **No Bevy** in the `assets` crate.
- **Graceful degrade, not error:** a malformed/short MODERNLV animation yields empty `ramps`/`anim` with `display_data`/`display_valid` kept — exactly as C++ does. Only a buffer too short for the declared material map is `LevelError::Truncated`.
- **FNV-1a (64-bit)** seed `0xcbf29ce484222325`, prime `0x100000001b3` — identical helper on both sides (see `rust/oracle-tests/tests/level_golden.rs`). Multi-byte fields are hashed as **explicit little-endian bytes** (build the byte sequence with shifts, never `memcpy` of a `u32`), so C++ and Rust hash identical sequences regardless of host endianness.
- **Golden regeneration is a LOCAL/MANUAL step** (needs the full C++ build). CI (`rust.yml`) only runs `cargo test --workspace` against the committed golden plus the sim-core regen; it does NOT rebuild the level/palette oracle. PRESET defaults to `macos-arm64`.
- **No AI taglines** in commits.

## File Structure

- `rust/assets/src/palette.rs` — NEW: `Color`, `Palette`, `PaletteError`, `load_vga`, `load_full`, `expand_to_full_range`.
- `rust/assets/src/lib.rs` — MODIFY: `pub mod palette;`.
- `rust/assets/src/level.rs` — MODIFY: extend `LevelData` with `palette`/`display`; add `DisplayLayers`, `ArgbRamp`; parse POWERLEVEL + MODERNLV.
- `src/tools/oracle_dump/palette_dump.cpp` — NEW: dump palette-op digests (real `Palette::*`).
- `src/tools/oracle_dump/level_dump.cpp` — MODIFY: dump palette/display/ramp/anim digests; add POWERLEVEL + MODERNLV synthetic inputs.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_palette` target under the existing `OPENLIERO_BUILD_ORACLE_DUMP` option block (lines 372–375).
- `rust/oracle-tests/tests/palette_golden.rs` — NEW: palette differential test.
- `rust/oracle-tests/tests/level_golden.rs` — MODIFY: parse extended golden columns and the new inputs.
- `rust/oracle-tests/golden/palette.txt` — NEW: committed golden.
- `rust/oracle-tests/golden/level.txt` — REGENERATE.
- `rust/oracle-tests/gen_palette_golden.sh` — NEW: regenerate palette golden.

---

### Task 1: Palette module (`palette.rs`)

Pure Rust, no oracle yet — unit-test the byte math directly.

**Files:**
- Create: `rust/assets/src/palette.rs`
- Modify: `rust/assets/src/lib.rs:4`
- Test: in-file `#[cfg(test)] mod tests` in `palette.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct Color { pub r: u8, pub g: u8, pub b: u8 }` (derives `Debug, Clone, Copy, PartialEq, Eq, Default`)
  - `pub struct Palette { pub entries: [Color; 256] }` (derives `Debug, Clone, PartialEq, Eq`)
  - `pub enum PaletteError { Truncated }` (derives `Debug, PartialEq, Eq`)
  - `Palette::load_vga(bytes: &[u8]) -> Result<Palette, PaletteError>`
  - `Palette::load_full(bytes: &[u8]) -> Result<Palette, PaletteError>`
  - `Palette::expand_to_full_range(&mut self)`

- [ ] **Step 1: Write `palette.rs` with failing tests**

```rust
//! Palette loading for `.lev` POWERLEVEL blocks and `modern.pal` files.
//! Byte transforms mirror C++ `Palette::Read`/`ReadFull`/`ExpandToFullRange`
//! (`src/game/gfx/palette.cpp`); the implementation is idiomatic Rust.

/// One palette entry. Mirrors the on-disk RGB triple; C++'s 4th `unused`
/// byte is an in-memory padding detail we do not keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A 256-entry palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub entries: [Color; 256],
}

/// Why a palette failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum PaletteError {
    /// Fewer than 768 bytes (256 RGB triples) available.
    Truncated,
}

const PALETTE_BYTES: usize = 256 * 3;

impl Palette {
    /// VGA 6-bit read: `(v & 63) << 2` per channel. (C++ `Palette::Read`,
    /// `palette.cpp:61`.) Consumes the first 768 bytes of `bytes`.
    pub fn load_vga(bytes: &[u8]) -> Result<Palette, PaletteError> {
        if bytes.len() < PALETTE_BYTES {
            return Err(PaletteError::Truncated);
        }
        let mut entries = [Color::default(); 256];
        for (i, e) in entries.iter_mut().enumerate() {
            let o = i * 3;
            e.r = (bytes[o] & 63) << 2;
            e.g = (bytes[o + 1] & 63) << 2;
            e.b = (bytes[o + 2] & 63) << 2;
        }
        Ok(Palette { entries })
    }

    /// Full 8-bit read: channels verbatim. (C++ `Palette::ReadFull`,
    /// `palette.cpp:81`.) Consumes the first 768 bytes of `bytes`.
    pub fn load_full(bytes: &[u8]) -> Result<Palette, PaletteError> {
        if bytes.len() < PALETTE_BYTES {
            return Err(PaletteError::Truncated);
        }
        let mut entries = [Color::default(); 256];
        for (i, e) in entries.iter_mut().enumerate() {
            let o = i * 3;
            e.r = bytes[o];
            e.g = bytes[o + 1];
            e.b = bytes[o + 2];
        }
        Ok(Palette { entries })
    }

    /// Expand a VGA-grid palette to the full 8-bit range in place:
    /// `e |= e >> 6` per channel. (C++ `Palette::ExpandToFullRange`,
    /// `palette.cpp:106`.)
    pub fn expand_to_full_range(&mut self) {
        for e in self.entries.iter_mut() {
            e.r |= e.r >> 6;
            e.g |= e.g >> 6;
            e.b |= e.b >> 6;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 768 bytes where channel value = (index) % `modulo`.
    fn buf(modulo: usize) -> Vec<u8> {
        (0..PALETTE_BYTES).map(|i| (i % modulo) as u8).collect()
    }

    #[test]
    fn load_vga_masks_and_shifts() {
        let p = Palette::load_vga(&buf(256)).unwrap();
        // entry 0: bytes 0,1,2 -> (0&63)<<2, (1&63)<<2, (2&63)<<2
        assert_eq!(p.entries[0], Color { r: 0, g: 4, b: 8 });
        // a byte > 63 is masked: byte 200 -> (200 & 63) << 2 = (8) << 2 = 32
        // find an offset whose raw value is 200: 200 (offset 200) is channel
        // r of entry 66 (200/3 = 66, 66*3 = 198 -> r=198,g=199,b=200).
        assert_eq!(p.entries[66].b, (200u8 & 63) << 2);
    }

    #[test]
    fn load_full_keeps_channels() {
        let p = Palette::load_full(&buf(256)).unwrap();
        assert_eq!(p.entries[0], Color { r: 0, g: 1, b: 2 });
        assert_eq!(p.entries[85], Color { r: 255, g: 0, b: 1 }); // 85*3 = 255
    }

    #[test]
    fn expand_maps_vga_white_to_255() {
        // load_vga of 0x3f (63) -> (63 & 63) << 2 = 252; expand -> 252|3 = 255.
        let raw = vec![63u8; PALETTE_BYTES];
        let mut p = Palette::load_vga(&raw).unwrap();
        assert_eq!(p.entries[0], Color { r: 252, g: 252, b: 252 });
        p.expand_to_full_range();
        assert_eq!(p.entries[0], Color { r: 255, g: 255, b: 255 });
    }

    #[test]
    fn rejects_short_buffer() {
        assert_eq!(Palette::load_vga(&[0u8; 767]), Err(PaletteError::Truncated));
        assert_eq!(Palette::load_full(&[0u8; 0]), Err(PaletteError::Truncated));
    }
}
```

Add to `rust/assets/src/lib.rs` after the `pub mod level;` line:

```rust
pub mod palette;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets palette`
Expected: 4 palette tests PASS. (They are written to match the implementation, which is small enough to land together; the golden test in Task 2 is the real adversarial check.)

- [ ] **Step 3: Commit**

```bash
git add rust/assets/src/palette.rs rust/assets/src/lib.rs
git commit -m "feat(assets): palette module (VGA 6->8, full 8-bit, expand)"
```

---

### Task 2: Palette golden (differential test vs C++)

Prove `palette.rs` matches the real `Palette::Read`/`ReadFull`/`ExpandToFullRange`.

**Files:**
- Create: `src/tools/oracle_dump/palette_dump.cpp`
- Modify: `CMakeLists.txt:372-375` (add target inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block)
- Create: `rust/oracle-tests/gen_palette_golden.sh`
- Create: `rust/oracle-tests/golden/palette.txt` (generated)
- Create: `rust/oracle-tests/tests/palette_golden.rs`

**Interfaces:**
- Consumes: `Palette::load_vga`, `load_full`, `expand_to_full_range` (Task 1).
- Produces: golden file format — one line per synthetic buffer: `vga_hash full_hash expand_hash` (16-hex FNV-1a each). Two buffers: `modulo=64`, `modulo=256`.

- [ ] **Step 1: Write the C++ palette dumper**

Create `src/tools/oracle_dump/palette_dump.cpp`:

```cpp
// Generates golden digests for the Rust palette differential test by running
// the REAL C++ Palette ops. Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the default build.
#include <cstdint>
#include <cstdio>
#include <vector>

#include "gfx/palette.hpp"
#include "io/stream.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// Hash a palette's r,g,b channels (256 entries * 3 bytes), matching the Rust
// Color { r, g, b } layout (C++'s 4th `unused` byte is not hashed).
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

// MUST match the Rust test's synthetic buffers exactly.
std::vector<uint8_t> Buf(int modulo) {
  std::vector<uint8_t> b(256 * 3);
  for (std::size_t i = 0; i < b.size(); ++i) {
    b[i] = static_cast<uint8_t>(i % modulo);
  }
  return b;
}

void DumpOne(std::FILE* out, std::vector<uint8_t> const& buf) {
  Palette vga;
  io::MemReader r1(buf);
  vga.Read(r1);

  Palette full;
  io::MemReader r2(buf);
  full.ReadFull(r2);

  Palette expand = vga;
  expand.ExpandToFullRange();

  std::fprintf(out, "%016llx %016llx %016llx\n",
               static_cast<unsigned long long>(HashPalette(vga)),
               static_cast<unsigned long long>(HashPalette(full)),
               static_cast<unsigned long long>(HashPalette(expand)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: oracle_dump_palette <out.txt>\n");
    return 1;
  }
  std::FILE* out = std::fopen(argv[1], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[1]);
    return 1;
  }
  DumpOne(out, Buf(64));
  DumpOne(out, Buf(256));
  std::fclose(out);
  return 0;
}
```

- [ ] **Step 2: Register the target in CMake**

In `CMakeLists.txt`, inside the existing block (after line 375 `target_link_libraries(oracle_dump_level PRIVATE game)`), add:

```cmake
  add_executable(oracle_dump_palette src/tools/oracle_dump/palette_dump.cpp)
  target_link_libraries(oracle_dump_palette PRIVATE game)
```

- [ ] **Step 3: Write the regeneration script**

Create `rust/oracle-tests/gen_palette_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/palette.txt by running the REAL C++ Palette ops.
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL
# step — it is NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_palette
"build/$PRESET/Release/oracle_dump_palette" \
  "$ROOT/rust/oracle-tests/golden/palette.txt"
echo "wrote rust/oracle-tests/golden/palette.txt"
```

Make it executable:

```bash
chmod +x rust/oracle-tests/gen_palette_golden.sh
```

- [ ] **Step 4: Generate the golden file**

Run: `bash rust/oracle-tests/gen_palette_golden.sh`
Expected: prints `wrote rust/oracle-tests/golden/palette.txt`; the file has 2 lines, each three 16-hex values.

- [ ] **Step 5: Write the Rust golden test**

Create `rust/oracle-tests/tests/palette_golden.rs`:

```rust
//! Differential test for the palette loader against the C++ oracle.
//! The golden (one line per synthetic buffer: `vga full expand`) is produced by
//! the real C++ `Palette::Read`/`ReadFull`/`ExpandToFullRange`; the Rust ops
//! must reproduce each FNV-1a digest.

use assets::palette::Palette;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hash_palette(p: &Palette) -> u64 {
    let mut bytes = Vec::with_capacity(256 * 3);
    for e in &p.entries {
        bytes.push(e.r);
        bytes.push(e.g);
        bytes.push(e.b);
    }
    fnv1a(&bytes)
}

// MUST match the C++ dumper's synthetic buffers exactly.
fn buf(modulo: usize) -> Vec<u8> {
    (0..256 * 3).map(|i| (i % modulo) as u8).collect()
}

#[test]
fn palette_ops_match_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/palette.txt"
    ))
    .unwrap();

    let bufs = [buf(64), buf(256)];
    let mut lines = golden.lines();

    for (idx, b) in bufs.iter().enumerate() {
        let line = lines.next().unwrap_or_else(|| panic!("missing golden line {idx}"));
        let mut it = line.split_whitespace();
        let want_vga = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        let want_full = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        let want_expand = u64::from_str_radix(it.next().unwrap(), 16).unwrap();

        let vga = Palette::load_vga(b).unwrap();
        let full = Palette::load_full(b).unwrap();
        let mut expand = Palette::load_vga(b).unwrap();
        expand.expand_to_full_range();

        assert_eq!(hash_palette(&vga), want_vga, "vga mismatch buf {idx}");
        assert_eq!(hash_palette(&full), want_full, "full mismatch buf {idx}");
        assert_eq!(hash_palette(&expand), want_expand, "expand mismatch buf {idx}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 6: Run the golden test**

Run: `cargo test --manifest-path rust/Cargo.toml -p oracle-tests palette_golden`
Expected: PASS (Rust digests equal the C++ golden).

- [ ] **Step 7: Commit**

```bash
git add src/tools/oracle_dump/palette_dump.cpp CMakeLists.txt \
  rust/oracle-tests/gen_palette_golden.sh rust/oracle-tests/golden/palette.txt \
  rust/oracle-tests/tests/palette_golden.rs
git commit -m "test(oracle): palette ops differential test vs C++"
```

---

### Task 3: POWERLEVEL palette in `level.rs`

Extend `LevelData` with `palette: Option<Palette>` and parse the POWERLEVEL block.

**Files:**
- Modify: `rust/assets/src/level.rs`
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `Palette`, `Palette::load_vga` (Task 1).
- Produces: `LevelData.palette: Option<Palette>` (Some iff a `POWERLEVEL` block followed the material map).

- [ ] **Step 1: Write the failing tests**

Add to `level.rs` `mod tests`:

```rust
    use crate::palette::Color;

    fn powerlevel_block() -> Vec<u8> {
        let mut b = b"POWERLEVEL".to_vec();
        // 768 VGA bytes: channel value = offset % 64.
        for i in 0..256 * 3 {
            b.push((i % 64) as u8);
        }
        b
    }

    #[test]
    fn parses_powerlevel_palette() {
        let mut buf = make_ollevel2(4, 4, |_| 1);
        buf.extend_from_slice(&powerlevel_block());
        let lvl = load(&buf).unwrap();
        let pal = lvl.palette.expect("expected custom palette");
        // VGA: (0&63)<<2, (1&63)<<2, (2&63)<<2
        assert_eq!(pal.entries[0], Color { r: 0, g: 4, b: 8 });
    }

    #[test]
    fn no_palette_when_no_powerlevel() {
        let buf = make_ollevel2(4, 4, |_| 1);
        assert!(load(&buf).unwrap().palette.is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets level`
Expected: FAIL — `LevelData` has no field `palette`.

- [ ] **Step 3: Implement POWERLEVEL parsing**

In `level.rs`, add the import at the top:

```rust
use crate::palette::Palette;
```

Add the field to `LevelData` (after `material_id`):

```rust
    pub palette: Option<Palette>,
```

Replace the bodies of `load_sized`/`load_legacy` so they return the material map *and the offset where trailing blocks begin*, then parse blocks in `load`. Concretely, change `load` to:

```rust
pub fn load(bytes: &[u8]) -> Result<LevelData, LevelError> {
    let (width, height, mat_start) = if bytes.len() >= 8 && &bytes[0..8] == SIZED_MAGIC {
        let (w, h) = parse_sized_header(bytes)?;
        (w, h, SIZED_HEADER_LEN)
    } else {
        (LEGACY_WIDTH, LEGACY_HEIGHT, 0usize)
    };
    let cells = width as usize * height as usize;
    let mat_end = mat_start + cells;
    if bytes.len() < mat_end {
        return Err(LevelError::Truncated);
    }
    let material_id = bytes[mat_start..mat_end].to_vec();

    // Cursor now points just past the material map; optional blocks follow.
    let mut cursor = mat_end;
    let palette = parse_powerlevel(bytes, &mut cursor);

    Ok(LevelData {
        width,
        height,
        material_id,
        palette,
        display: None, // filled in Task 4
    })
}

// OLLEVEL2 header: magic(8) + version(1) + w(2 LE) + h(2 LE). Returns (w, h).
fn parse_sized_header(bytes: &[u8]) -> Result<(i32, i32), LevelError> {
    if bytes.len() < SIZED_HEADER_LEN {
        return Err(LevelError::Truncated);
    }
    let width = u16::from_le_bytes([bytes[9], bytes[10]]) as i32;
    let height = u16::from_le_bytes([bytes[11], bytes[12]]) as i32;
    if width < 1 || width > MAX_DIM || height < 1 || height > MAX_DIM {
        return Err(LevelError::BadDimensions(width, height));
    }
    Ok((width, height))
}

// If `bytes[cursor..]` starts with "POWERLEVEL", consume the 10-byte magic and
// a 768-byte VGA palette, advancing `cursor`. C++ always parses this block
// (equivalent to load_powerlevel_palette = true; see the spec).
fn parse_powerlevel(bytes: &[u8], cursor: &mut usize) -> Option<Palette> {
    const MAGIC: &[u8; 10] = b"POWERLEVEL";
    let start = *cursor;
    if bytes.len() < start + MAGIC.len() || &bytes[start..start + MAGIC.len()] != MAGIC {
        return None;
    }
    let pal_start = start + MAGIC.len();
    match Palette::load_vga(&bytes[pal_start..]) {
        Ok(pal) => {
            *cursor = pal_start + 256 * 3;
            Some(pal)
        }
        Err(_) => None, // truncated palette: leave cursor, no custom palette
    }
}
```

Delete the now-unused `load_sized`/`load_legacy` functions (their logic moved into `load`/`parse_sized_header`). Update every existing test that constructs `LevelData` directly — none do; tests use `load`, so only the assertions about new fields change.

Fix the three existing pass-through tests that compare `LevelData` via derived `PartialEq` only if any do so by value — they assert individual fields, so no change needed.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: all `assets` tests PASS (1b tests + the two new POWERLEVEL tests).

- [ ] **Step 5: Commit**

```bash
git add rust/assets/src/level.rs
git commit -m "feat(assets): parse POWERLEVEL custom palette in .lev loader"
```

---

### Task 4: MODERNLV display layers + animation in `level.rs`

Add `display: Option<DisplayLayers>` and parse the MODERNLV block (byte-exact read; resolve deferred).

**Files:**
- Modify: `rust/assets/src/level.rs`
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: cursor from Task 3's `load`.
- Produces:
  - `pub struct DisplayLayers { pub data: Vec<u32>, pub valid: Vec<u8>, pub ramps: Vec<ArgbRamp>, pub anim: Vec<u8> }` (derives `Debug, Clone, PartialEq, Eq`)
  - `pub struct ArgbRamp { pub shift: u8, pub colors: Vec<u32> }` (derives `Debug, Clone, PartialEq, Eq`)
  - `LevelData.display: Option<DisplayLayers>`

- [ ] **Step 1: Write the failing tests**

Add to `level.rs` `mod tests`:

```rust
    // Build a MODERNLV block for `cells` pixels. `anim` = None -> no animation;
    // Some((ramps, anim_indices)) appends the animation extension.
    fn modernlv_block(
        cells: usize,
        anim: Option<(Vec<(u8, Vec<u32>)>, Vec<u8>)>,
    ) -> Vec<u8> {
        let mut b = b"MODERNLV".to_vec();
        for i in 0..cells {
            b.extend_from_slice(&((0x11223300u32).wrapping_add(i as u32)).to_le_bytes());
        }
        for i in 0..cells {
            b.push((i % 2) as u8); // display_valid
        }
        if let Some((ramps, anim_idx)) = anim {
            b.push(ramps.len() as u8);
            for (shift, colors) in &ramps {
                b.push(*shift);
                b.extend_from_slice(&(colors.len() as u16).to_le_bytes());
                for c in colors {
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            for idx in &anim_idx {
                b.push(*idx);
            }
        }
        b
    }

    #[test]
    fn parses_modernlv_without_animation() {
        let mut buf = make_ollevel2(2, 2, |_| 7);
        buf.extend_from_slice(&modernlv_block(4, None));
        let d = load(&buf).unwrap().display.expect("display");
        assert_eq!(d.data.len(), 4);
        assert_eq!(d.valid, vec![0, 1, 0, 1]);
        assert_eq!(d.data[0], 0x11223300);
        assert!(d.ramps.is_empty());
        assert!(d.anim.is_empty());
    }

    #[test]
    fn parses_modernlv_with_good_animation() {
        let ramps = vec![(3u8, vec![0xAABBCCDDu32, 0x01020304])];
        let anim = vec![0u8, 1, 1, 0]; // all <= ramp_count (1)
        let mut buf = make_ollevel2(2, 2, |_| 7);
        buf.extend_from_slice(&modernlv_block(4, Some((ramps, anim.clone()))));
        let d = load(&buf).unwrap().display.unwrap();
        assert_eq!(d.ramps.len(), 1);
        assert_eq!(d.ramps[0].shift, 3);
        assert_eq!(d.ramps[0].colors, vec![0xAABBCCDD, 0x01020304]);
        assert_eq!(d.anim, anim);
    }

    #[test]
    fn modernlv_bad_ramp_index_degrades_gracefully() {
        // anim index 2 > ramp_count 1 -> C++ drops ramps+anim, keeps display.
        let ramps = vec![(3u8, vec![0xAABBCCDDu32])];
        let anim = vec![0u8, 2, 0, 0];
        let mut buf = make_ollevel2(2, 2, |_| 7);
        buf.extend_from_slice(&modernlv_block(4, Some((ramps, anim))));
        let d = load(&buf).unwrap().display.unwrap();
        assert_eq!(d.data.len(), 4); // display kept
        assert!(d.ramps.is_empty()); // animation dropped
        assert!(d.anim.is_empty());
    }

    #[test]
    fn modernlv_truncated_animation_degrades_gracefully() {
        // ramp_count=1 but stream ends before the ramp body -> drop animation.
        let mut buf = make_ollevel2(2, 2, |_| 7);
        let mut block = modernlv_block(4, None);
        block.push(1); // ramp_count = 1, then EOF
        buf.extend_from_slice(&block);
        let d = load(&buf).unwrap().display.unwrap();
        assert!(d.ramps.is_empty());
        assert!(d.anim.is_empty());
    }

    #[test]
    fn no_display_when_no_modernlv() {
        let buf = make_ollevel2(2, 2, |_| 7);
        assert!(load(&buf).unwrap().display.is_none());
    }

    #[test]
    fn powerlevel_then_modernlv_both_parsed() {
        let mut buf = make_ollevel2(2, 2, |_| 1);
        buf.extend_from_slice(&powerlevel_block());
        buf.extend_from_slice(&modernlv_block(4, None));
        let lvl = load(&buf).unwrap();
        assert!(lvl.palette.is_some());
        assert!(lvl.display.is_some());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets level`
Expected: FAIL — no `display` field / no `DisplayLayers`.

- [ ] **Step 3: Implement MODERNLV parsing**

Add the types near `LevelData`:

```rust
/// True-color display layers from a `.lev` MODERNLV block. Read byte-exact;
/// the per-tick colour resolve is rendering (step 3), out of scope here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLayers {
    /// `cells` ARGB values (per-pixel phase offset when animated).
    pub data: Vec<u32>,
    /// `cells` flags: 1 = authored colour, 0 = fall back to palette.
    pub valid: Vec<u8>,
    /// Animation ramps; empty unless a valid animation block followed.
    pub ramps: Vec<ArgbRamp>,
    /// `cells` ramp indices (0 = static, N = ramp N-1); empty unless `ramps`.
    pub anim: Vec<u8>,
}

/// One animation ramp: a colour cycle advanced by `shift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgbRamp {
    pub shift: u8,
    pub colors: Vec<u32>,
}
```

Replace `display: None` in `load` with:

```rust
    let display = parse_modernlv(bytes, &mut cursor, cells);
```

Add the parser (mirrors `level.cpp:296–382`):

```rust
const MAX_RAMP_COLORS: usize = 4096;

// If `bytes[cursor..]` starts with "MODERNLV", parse display layers + optional
// animation, advancing `cursor`. Animation degrades gracefully: any malformed
// or short part drops ramps+anim while keeping display data (matches C++).
fn parse_modernlv(bytes: &[u8], cursor: &mut usize, cells: usize) -> Option<DisplayLayers> {
    const MAGIC: &[u8; 8] = b"MODERNLV";
    let start = *cursor;
    if bytes.len() < start + MAGIC.len() || &bytes[start..start + MAGIC.len()] != MAGIC {
        return None;
    }
    let mut pos = start + MAGIC.len();

    // display_data: cells * u32 LE
    let dd_end = pos + cells * 4;
    if bytes.len() < dd_end {
        return None; // C++ Get() would fail; treat as no MODERNLV block
    }
    let data: Vec<u32> = bytes[pos..dd_end]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    pos = dd_end;

    // display_valid: cells * u8
    let dv_end = pos + cells;
    if bytes.len() < dv_end {
        return None;
    }
    let valid = bytes[pos..dv_end].to_vec();
    pos = dv_end;
    *cursor = pos; // display layers committed; cursor past them

    // Optional animation extension (graceful degrade on any shortfall).
    let (ramps, anim) = parse_animation(bytes, &mut pos, cells).unwrap_or_default();
    if !ramps.is_empty() {
        *cursor = pos; // animation consumed too
    }

    Some(DisplayLayers { data, valid, ramps, anim })
}

// Returns Some((ramps, anim)) only when the full, valid animation parses;
// None on any shortfall/violation (caller keeps display, drops animation).
fn parse_animation(
    bytes: &[u8],
    pos: &mut usize,
    cells: usize,
) -> Option<(Vec<ArgbRamp>, Vec<u8>)> {
    let mut p = *pos;
    let ramp_count = *bytes.get(p)?; // EOF here -> no animation
    p += 1;
    if ramp_count == 0 {
        return None;
    }

    let mut ramps = Vec::with_capacity(ramp_count as usize);
    for _ in 0..ramp_count {
        let shift = *bytes.get(p)?;
        p += 1;
        let cc_lo = *bytes.get(p)?;
        let cc_hi = *bytes.get(p + 1)?;
        p += 2;
        let color_count = u16::from_le_bytes([cc_lo, cc_hi]) as usize;
        if color_count == 0 || color_count > MAX_RAMP_COLORS {
            return None;
        }
        let end = p + color_count * 4;
        if bytes.len() < end {
            return None;
        }
        let colors: Vec<u32> = bytes[p..end]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        p = end;
        ramps.push(ArgbRamp { shift, colors });
    }

    let anim_end = p + cells;
    if bytes.len() < anim_end {
        return None;
    }
    let anim = bytes[p..anim_end].to_vec();
    // Every index must be <= ramp_count (C++ rejects `> ramp_count`).
    if anim.iter().any(|&idx| idx > ramp_count) {
        return None;
    }
    p = anim_end;

    *pos = p;
    Some((ramps, anim))
}
```

Add the `display` field to the `LevelData` struct definition (after `palette`):

```rust
    pub display: Option<DisplayLayers>,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: all `assets` tests PASS (1b + POWERLEVEL + 6 MODERNLV tests).

- [ ] **Step 5: Commit**

```bash
git add rust/assets/src/level.rs
git commit -m "feat(assets): parse MODERNLV display layers + animation in .lev loader"
```

---

### Task 5: Level golden — POWERLEVEL + MODERNLV differential test

Extend the C++ level dumper and Rust level golden to prove the new fields match C++ bit-for-bit on real and synthetic inputs.

**Files:**
- Modify: `src/tools/oracle_dump/level_dump.cpp`
- Modify: `rust/oracle-tests/golden/level.txt` (regenerate)
- Modify: `rust/oracle-tests/tests/level_golden.rs`

**Interfaces:**
- Consumes: `LevelData.palette`, `LevelData.display` (Tasks 3–4); the real C++ `Level` fields `origpal`, `has_custom_palette`, `display_data`, `display_valid`, `argb_ramps`, `display_anim`.
- Produces: golden line format `w h mat_hash pal_hash dd_hash dv_hash ramp_hash anim_hash`, `-` for absent fields. Inputs, in order: `modern_test.lev`, `MakeLegacy`, `MakeOllevel2`, `MakePowerlevel`, `MakeModernNoAnim`, `MakeModernBadAnim`.

- [ ] **Step 1: Extend the C++ level dumper**

In `src/tools/oracle_dump/level_dump.cpp`:

Add LE-byte hash helpers after `Fnv1a`:

```cpp
// Hash a palette's r,g,b channels (matches Rust Color { r, g, b }).
uint64_t HashPalette(Palette const& p) {
  std::vector<unsigned char> b;
  b.reserve(256 * 3);
  for (auto const& e : p.entries) {
    b.push_back(e.r);
    b.push_back(e.g);
    b.push_back(e.b);
  }
  return Fnv1a(b);
}

// Hash u32 values as explicit little-endian bytes (host-endian independent).
uint64_t HashU32LE(std::vector<uint32_t> const& v) {
  std::vector<unsigned char> b;
  b.reserve(v.size() * 4);
  for (uint32_t x : v) {
    b.push_back(static_cast<unsigned char>(x & 0xff));
    b.push_back(static_cast<unsigned char>((x >> 8) & 0xff));
    b.push_back(static_cast<unsigned char>((x >> 16) & 0xff));
    b.push_back(static_cast<unsigned char>((x >> 24) & 0xff));
  }
  return Fnv1a(b);
}

uint64_t HashBytes(std::vector<uint8_t> const& v) {
  return Fnv1a(std::vector<unsigned char>(v.begin(), v.end()));
}

// Ramp table serialized as: shift byte then colors as LE u32, per ramp.
uint64_t HashRamps(std::vector<Level::ArgbRamp> const& ramps) {
  std::vector<unsigned char> b;
  for (auto const& r : ramps) {
    b.push_back(r.shift);
    for (uint32_t c : r.colors) {
      b.push_back(static_cast<unsigned char>(c & 0xff));
      b.push_back(static_cast<unsigned char>((c >> 8) & 0xff));
      b.push_back(static_cast<unsigned char>((c >> 16) & 0xff));
      b.push_back(static_cast<unsigned char>((c >> 24) & 0xff));
    }
  }
  return Fnv1a(b);
}
```

Replace `DumpOne` with the extended version:

```cpp
void DumpOne(std::FILE* out, Common& common, Settings const& settings,
             std::vector<uint8_t> const& buf) {
  io::MemReader r(buf);
  Level level(common);
  if (!level.load(common, settings, r)) {
    std::fprintf(stderr, "Level::load failed\n");
    std::exit(1);
  }
  std::fprintf(out, "%d %d %016llx", level.width, level.height,
               static_cast<unsigned long long>(Fnv1a(level.material_id)));

  if (level.has_custom_palette) {
    std::fprintf(out, " %016llx",
                 static_cast<unsigned long long>(HashPalette(level.origpal)));
  } else {
    std::fprintf(out, " -");
  }

  if (!level.display_data.empty()) {
    std::fprintf(out, " %016llx %016llx",
                 static_cast<unsigned long long>(HashU32LE(level.display_data)),
                 static_cast<unsigned long long>(HashBytes(level.display_valid)));
  } else {
    std::fprintf(out, " - -");
  }

  if (!level.argb_ramps.empty()) {
    std::fprintf(out, " %016llx %016llx",
                 static_cast<unsigned long long>(HashRamps(level.argb_ramps)),
                 static_cast<unsigned long long>(HashBytes(level.display_anim)));
  } else {
    std::fprintf(out, " - -");
  }
  std::fprintf(out, "\n");
}
```

Add the new synthetic inputs (after `MakeOllevel2`):

```cpp
// A small OLLEVEL2 level (w*h cells) with the given trailing block appended.
std::vector<uint8_t> MakeSizedWith(int w, int h, std::vector<uint8_t> const& tail) {
  std::vector<uint8_t> b = {'O', 'L', 'L', 'E', 'V', 'E', 'L', '2'};
  b.push_back(0);
  b.push_back(static_cast<uint8_t>(w & 0xff));
  b.push_back(static_cast<uint8_t>((w >> 8) & 0xff));
  b.push_back(static_cast<uint8_t>(h & 0xff));
  b.push_back(static_cast<uint8_t>((h >> 8) & 0xff));
  for (int i = 0; i < w * h; ++i) {
    b.push_back(static_cast<uint8_t>((i * 5 + 2) % 256));
  }
  b.insert(b.end(), tail.begin(), tail.end());
  return b;
}

std::vector<uint8_t> Powerlevel() {
  std::vector<uint8_t> b = {'P', 'O', 'W', 'E', 'R', 'L', 'E', 'V', 'E', 'L'};
  for (int i = 0; i < 256 * 3; ++i) b.push_back(static_cast<uint8_t>(i % 64));
  return b;
}

// MODERNLV block for `cells` pixels. anim_kind: 0=none, 1=good, 2=bad index.
std::vector<uint8_t> Modernlv(int cells, int anim_kind) {
  std::vector<uint8_t> b = {'M', 'O', 'D', 'E', 'R', 'N', 'L', 'V'};
  for (int i = 0; i < cells; ++i) {
    uint32_t v = 0x11223300u + static_cast<uint32_t>(i);
    b.push_back(static_cast<uint8_t>(v & 0xff));
    b.push_back(static_cast<uint8_t>((v >> 8) & 0xff));
    b.push_back(static_cast<uint8_t>((v >> 16) & 0xff));
    b.push_back(static_cast<uint8_t>((v >> 24) & 0xff));
  }
  for (int i = 0; i < cells; ++i) b.push_back(static_cast<uint8_t>(i % 2));
  if (anim_kind != 0) {
    b.push_back(1);  // ramp_count
    b.push_back(3);  // shift
    b.push_back(2);  // color_count LE
    b.push_back(0);
    uint32_t cols[2] = {0xAABBCCDDu, 0x01020304u};
    for (uint32_t c : cols) {
      b.push_back(static_cast<uint8_t>(c & 0xff));
      b.push_back(static_cast<uint8_t>((c >> 8) & 0xff));
      b.push_back(static_cast<uint8_t>((c >> 16) & 0xff));
      b.push_back(static_cast<uint8_t>((c >> 24) & 0xff));
    }
    for (int i = 0; i < cells; ++i) {
      uint8_t idx = (anim_kind == 2 && i == 1) ? 2 : static_cast<uint8_t>(i % 2);
      b.push_back(idx);
    }
  }
  return b;
}
```

In `main`, set the POWERLEVEL gate true and dump the new inputs (the existing three stay first, in order):

```cpp
  settings.load_powerlevel_palette = true;
```

and after the existing three `DumpOne(...)` calls:

```cpp
  DumpOne(out, common, settings, MakeSizedWith(4, 4, Powerlevel()));
  DumpOne(out, common, settings, MakeSizedWith(2, 2, Modernlv(4, 0)));
  DumpOne(out, common, settings, MakeSizedWith(2, 2, Modernlv(4, 2)));
```

(Note: `modern_test.lev`'s line now also carries display/anim hashes, since it contains a MODERNLV block with one ramp. `MakeLegacy`/`MakeOllevel2` carry `-` for all optional fields.)

- [ ] **Step 2: Regenerate the golden**

Run: `bash rust/oracle-tests/gen_level_golden.sh`
Expected: prints `wrote rust/oracle-tests/golden/level.txt`; the file has 6 lines. Line 1 (`modern_test.lev`) has non-`-` `dd`/`dv`/`ramp`/`anim`; lines 2–3 have `-` for all optional; line 4 has a `pal` hash; line 5 has `dd`/`dv` only; line 6 has `dd`/`dv` only (bad-anim degraded).

- [ ] **Step 3: Rewrite the Rust level golden test**

Replace `rust/oracle-tests/tests/level_golden.rs` with the extended version:

```rust
//! Differential test for the level loader (material map + POWERLEVEL palette +
//! MODERNLV display layers/animation) against the C++ oracle. The golden line is
//! `w h mat pal dd dv ramp anim` with `-` for absent optional fields, produced
//! by the real C++ `Level::load`; the Rust loader must reproduce every digest.

use assets::level::{load, ArgbRamp, DisplayLayers, LevelData};
use assets::palette::Palette;

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

fn hash_u32_le(v: &[u32]) -> u64 {
    let mut b = Vec::with_capacity(v.len() * 4);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    fnv1a(&b)
}

fn hash_ramps(ramps: &[ArgbRamp]) -> u64 {
    let mut b = Vec::new();
    for r in ramps {
        b.push(r.shift);
        for c in &r.colors {
            b.extend_from_slice(&c.to_le_bytes());
        }
    }
    fnv1a(&b)
}

// MUST match the C++ dumper's synthetic inputs exactly.
fn make_legacy() -> Vec<u8> {
    (0..504 * 350).map(|i| (i % 251) as u8).collect()
}

fn make_ollevel2_base(w: i32, h: i32) -> Vec<u8> {
    let mut b = b"OLLEVEL2".to_vec();
    b.push(0);
    b.extend_from_slice(&(w as u16).to_le_bytes());
    b.extend_from_slice(&(h as u16).to_le_bytes());
    for i in 0..(w * h) {
        b.push(((i * 5 + 2) % 256) as u8);
    }
    b
}

fn powerlevel() -> Vec<u8> {
    let mut b = b"POWERLEVEL".to_vec();
    for i in 0..256 * 3 {
        b.push((i % 64) as u8);
    }
    b
}

// anim_kind: 0=none, 2=bad index (matches the C++ dumper's Modernlv).
fn modernlv(cells: usize, anim_kind: u8) -> Vec<u8> {
    let mut b = b"MODERNLV".to_vec();
    for i in 0..cells {
        b.extend_from_slice(&(0x11223300u32.wrapping_add(i as u32)).to_le_bytes());
    }
    for i in 0..cells {
        b.push((i % 2) as u8);
    }
    if anim_kind != 0 {
        b.push(1); // ramp_count
        b.push(3); // shift
        b.extend_from_slice(&2u16.to_le_bytes()); // color_count
        b.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        b.extend_from_slice(&0x01020304u32.to_le_bytes());
        for i in 0..cells {
            let idx = if anim_kind == 2 && i == 1 { 2 } else { (i % 2) as u8 };
            b.push(idx);
        }
    }
    b
}

fn with_tail(mut base: Vec<u8>, tail: Vec<u8>) -> Vec<u8> {
    base.extend_from_slice(&tail);
    base
}

// Parse a golden hash column: `-` => None, else u64 hex.
fn col(s: &str) -> Option<u64> {
    if s == "-" {
        None
    } else {
        Some(u64::from_str_radix(s, 16).unwrap())
    }
}

#[test]
fn level_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/level.txt"
    ))
    .unwrap();
    let modern = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/Levels/modern_test.lev"
    ))
    .unwrap();

    let inputs: [Vec<u8>; 6] = [
        modern,
        make_legacy(),
        make_ollevel2_base(13, 11),
        with_tail(make_ollevel2_base(4, 4), powerlevel()),
        with_tail(make_ollevel2_base(2, 2), modernlv(4, 0)),
        with_tail(make_ollevel2_base(2, 2), modernlv(4, 2)),
    ];
    let mut lines = golden.lines();

    for (idx, buf) in inputs.iter().enumerate() {
        let line = lines.next().unwrap_or_else(|| panic!("missing golden line {idx}"));
        let mut it = line.split_whitespace();
        let want_w: i32 = it.next().unwrap().parse().unwrap();
        let want_h: i32 = it.next().unwrap().parse().unwrap();
        let want_mat = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        let want_pal = col(it.next().unwrap());
        let want_dd = col(it.next().unwrap());
        let want_dv = col(it.next().unwrap());
        let want_ramp = col(it.next().unwrap());
        let want_anim = col(it.next().unwrap());

        let lvl: LevelData = load(buf).unwrap_or_else(|e| panic!("input {idx} failed: {e:?}"));
        assert_eq!(lvl.width, want_w, "width input {idx}");
        assert_eq!(lvl.height, want_h, "height input {idx}");
        assert_eq!(fnv1a(&lvl.material_id), want_mat, "material input {idx}");

        assert_eq!(lvl.palette.as_ref().map(hash_palette), want_pal, "palette input {idx}");

        let dd = lvl.display.as_ref().map(|d: &DisplayLayers| hash_u32_le(&d.data));
        let dv = lvl.display.as_ref().map(|d| fnv1a(&d.valid));
        assert_eq!(dd, want_dd, "display_data input {idx}");
        assert_eq!(dv, want_dv, "display_valid input {idx}");

        let ramp = lvl
            .display
            .as_ref()
            .filter(|d| !d.ramps.is_empty())
            .map(|d| hash_ramps(&d.ramps));
        let anim = lvl
            .display
            .as_ref()
            .filter(|d| !d.ramps.is_empty())
            .map(|d| fnv1a(&d.anim));
        assert_eq!(ramp, want_ramp, "ramp input {idx}");
        assert_eq!(anim, want_anim, "anim input {idx}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 4: Run the full workspace suite**

Run: `cargo test --manifest-path rust/Cargo.toml --workspace`
Expected: ALL tests PASS (sim-core golden, assets unit, palette_golden, level_golden).

- [ ] **Step 5: Commit**

```bash
git add src/tools/oracle_dump/level_dump.cpp rust/oracle-tests/golden/level.txt \
  rust/oracle-tests/tests/level_golden.rs
git commit -m "test(oracle): POWERLEVEL + MODERNLV level differential test vs C++"
```

---

## Self-Review

**Spec coverage:**
- Palette module (VGA/full/expand) → Task 1 + golden Task 2. ✓
- POWERLEVEL block → Task 3 + golden Task 5. ✓
- MODERNLV display + animation (read byte-exact) → Task 4 + golden Task 5. ✓
- Graceful degrade on bad/short animation → Task 4 tests + Task 5 bad-anim input. ✓
- `Option<Palette>`/`Option<DisplayLayers>` additive API → Tasks 3–4. ✓
- Oracle inputs (modern_test.lev, synthetic POWERLEVEL/MODERNLV/bad-anim, palette buffers) → Tasks 2 & 5. ✓
- `load_powerlevel_palette = true` in dumper; Rust always parses POWERLEVEL → Task 3 (`parse_powerlevel`) + Task 5 (`main`). ✓
- Animation resolve deferred → not implemented (out of scope). ✓

**Placeholder scan:** No TBD/TODO; all code is complete. The `display: None` placeholder in Task 3 is explicitly replaced in Task 4 Step 3. ✓

**Type consistency:** `Palette`/`Color`/`PaletteError` (Task 1) used verbatim in Tasks 2–3, 5. `DisplayLayers`/`ArgbRamp` field names (`data`, `valid`, `ramps`, `anim`; `shift`, `colors`) consistent across Tasks 4–5. Golden column order (`w h mat pal dd dv ramp anim`) identical in C++ `DumpOne` (Task 5 Step 1) and Rust parser (Task 5 Step 3). Synthetic-input builders byte-identical between C++ and Rust (Tasks 2 & 5). ✓

**Note on C++ `Level::ArgbRamp`:** Verified at `src/game/level.hpp:49–52` — the nested struct is `Level::ArgbRamp { std::vector<uint32_t> colors; uint8_t shift{0}; }`, and `Level::argb_ramps` is `std::vector<ArgbRamp>` (`level.hpp:192`). Task 5's `HashRamps(std::vector<Level::ArgbRamp> const&)` with `r.shift`/`r.colors` matches exactly; no adjustment needed.
