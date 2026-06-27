# Step 1a+1b — IO + level material map: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new Rust `assets` crate that loads a `.lev` file into `LevelData { width, height, material_id }`, idiomatic Rust, proven byte-identical to the C++ `Level::load` via a golden differential test.

**Architecture:** Idiomatic Rust parser over a `&[u8]` slice (no port of C++'s `Reader`/`MemReader`). A C++ dumper links the `game` library, runs the *real* `Level::load` on three inputs, and writes an FNV-1a hash of each material map to a tiny committed golden file. The Rust test reproduces all three and compares.

**Tech Stack:** Rust (stable, edition 2021), cargo workspace; C++20 + CMake (links the existing `game` target) for the dumper.

## Global Constraints

- **Modernization charter:** behaviour that feeds the simulation is locked and differential-tested; the implementation is idiomatic Rust. Do **not** port `io::Reader`/`MemReader`/`coding.hpp`.
- The `assets` crate has **no dependencies** for this slice.
- `load` returns `Result<LevelData, LevelError>` (not C++'s `bool`); parse with `u16::from_le_bytes` and slicing.
- The C++ oracle, under `src/game/`, is **not modified** — only read and linked.
- Golden is committed; the lightweight `rust.yml` CI runs `cargo test` against it. Regenerating the level golden needs the full C++ build and is a local/manual step (not in `rust.yml`).
- Format facts (from `src/game/level.cpp:229-273`): OLLEVEL2 header = magic `"OLLEVEL2"` (8) + version (1) + width (`u16` LE) + height (`u16` LE) = 13 bytes, then `width*height` material bytes. No magic → legacy `504×350`, material bytes from offset 0. Dimension cap `1..=4096`.
- **Shared synthetic inputs** (defined identically in the C++ dumper and the Rust test):
  - *legacy*: `504*350 = 176400` bytes, `byte[i] = (i % 251) as u8`.
  - *ollevel2*: `width=13, height=11`; header `"OLLEVEL2"` + `0x00` + `[13,0]` + `[11,0]`, then `13*11 = 143` bytes, `byte[i] = ((i*5 + 2) % 256) as u8`.
- **FNV-1a 64-bit** (defined identically both sides): `h = 0xcbf29ce484222325`; for each byte `b`: `h ^= b; h = h.wrapping_mul(0x100000001b3)`.

---

### Task 1: Add the `assets` crate to the workspace

**Files:**
- Modify: `rust/Cargo.toml`
- Create: `rust/assets/Cargo.toml`
- Create: `rust/assets/src/lib.rs`
- Modify: `rust/oracle-tests/Cargo.toml`

**Interfaces:**
- Consumes: the existing workspace from step 0.
- Produces: a buildable `assets` lib crate; `oracle-tests` gains `assets` as a dev-dependency.

- [ ] **Step 1: Add `assets` to the workspace members**

`rust/Cargo.toml` — change the members line to:
```toml
[workspace]
resolver = "2"
members = ["sim-core", "assets", "oracle-tests"]
```

- [ ] **Step 2: Create the crate manifest and lib**

`rust/assets/Cargo.toml`:
```toml
[package]
name = "assets"
version = "0.1.0"
edition = "2021"

[dependencies]
```

`rust/assets/src/lib.rs`:
```rust
//! On-disk data-format loaders for Liero-rs (no Bevy). Behaviour that feeds the
//! simulation is differential-tested against the C++ engine; the implementation
//! is idiomatic Rust, not a port of the C++ `io` layer.
pub mod level;
```

- [ ] **Step 3: Create a placeholder `level` module so the crate compiles**

`rust/assets/src/level.rs`:
```rust
//! `.lev` level loader — material map only (palette/display/sprites are later slices).
```

- [ ] **Step 4: Wire `assets` into oracle-tests**

`rust/oracle-tests/Cargo.toml` — add to `[dev-dependencies]`:
```toml
[dev-dependencies]
sim-core = { path = "../sim-core" }
assets = { path = "../assets" }
```

- [ ] **Step 5: Verify the workspace builds**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: PASS (`Compiling assets`, `Finished`).

- [ ] **Step 6: Commit**

```bash
git -C . add rust/Cargo.toml rust/assets rust/oracle-tests/Cargo.toml
git -C . commit -m "feat(assets): scaffold assets crate in the workspace"
```

---

### Task 2: `load` — OLLEVEL2 sized format

**Files:**
- Modify: `rust/assets/src/level.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `assets::level::{LevelData, LevelError, load}`.
  - `LevelData { width: i32, height: i32, material_id: Vec<u8> }`
  - `LevelError` (enum: `Truncated`, `BadDimensions(i32, i32)`)
  - `load(bytes: &[u8]) -> Result<LevelData, LevelError>`

- [ ] **Step 1: Write the failing test**

Append to `rust/assets/src/level.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // OLLEVEL2: magic(8) + version(1) + w(2 LE) + h(2 LE) + w*h material bytes.
    fn make_ollevel2(w: i32, h: i32, fill: impl Fn(usize) -> u8) -> Vec<u8> {
        let mut b = b"OLLEVEL2".to_vec();
        b.push(0); // version
        b.extend_from_slice(&(w as u16).to_le_bytes());
        b.extend_from_slice(&(h as u16).to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            b.push(fill(i));
        }
        b
    }

    #[test]
    fn loads_ollevel2_dimensions_and_materials() {
        let buf = make_ollevel2(13, 11, |i| ((i * 5 + 2) % 256) as u8);
        let lvl = load(&buf).unwrap();
        assert_eq!(lvl.width, 13);
        assert_eq!(lvl.height, 11);
        assert_eq!(lvl.material_id.len(), 13 * 11);
        assert_eq!(lvl.material_id[0], 2);
        assert_eq!(lvl.material_id[3], ((3 * 5 + 2) % 256) as u8);
    }
}
```

- [ ] **Step 2: Run the test, verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: FAIL (`cannot find function load` / `LevelData`).

- [ ] **Step 3: Implement the types and OLLEVEL2 parsing**

Replace the body of `rust/assets/src/level.rs` (above the `#[cfg(test)]` module) with:
```rust
//! `.lev` level loader — material map only (palette/display/sprites are later slices).

/// The parsed level data the simulation needs: dimensions + the per-pixel
/// palette-index material map (row-major, `width*height` bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelData {
    pub width: i32,
    pub height: i32,
    pub material_id: Vec<u8>,
}

/// Why a `.lev` failed to load. C++ returns `bool`; we use a typed error.
#[derive(Debug, PartialEq, Eq)]
pub enum LevelError {
    /// The buffer ended before the header or material bytes were complete.
    Truncated,
    /// OLLEVEL2 width/height outside the valid `1..=4096` range.
    BadDimensions(i32, i32),
}

const SIZED_MAGIC: &[u8; 8] = b"OLLEVEL2";
const LEGACY_WIDTH: i32 = 504;
const LEGACY_HEIGHT: i32 = 350;
const MAX_DIM: i32 = 4096;
const SIZED_HEADER_LEN: usize = 13; // magic(8) + version(1) + w(2) + h(2)

/// Load a `.lev` byte buffer into its material map. Mirrors the C++
/// `Level::load` format detection (`level.cpp:229`): an `OLLEVEL2` magic selects
/// the sized format; otherwise the bytes are a legacy 504×350 material map.
/// Trailing POWERLEVEL/MODERNLV blocks are ignored — only the material map matters.
pub fn load(bytes: &[u8]) -> Result<LevelData, LevelError> {
    if bytes.len() >= 8 && &bytes[0..8] == SIZED_MAGIC {
        load_sized(bytes)
    } else {
        load_legacy(bytes)
    }
}

fn load_sized(bytes: &[u8]) -> Result<LevelData, LevelError> {
    if bytes.len() < SIZED_HEADER_LEN {
        return Err(LevelError::Truncated);
    }
    let width = u16::from_le_bytes([bytes[9], bytes[10]]) as i32;
    let height = u16::from_le_bytes([bytes[11], bytes[12]]) as i32;
    if width < 1 || width > MAX_DIM || height < 1 || height > MAX_DIM {
        return Err(LevelError::BadDimensions(width, height));
    }
    let cells = width as usize * height as usize;
    let end = SIZED_HEADER_LEN + cells;
    if bytes.len() < end {
        return Err(LevelError::Truncated);
    }
    Ok(LevelData {
        width,
        height,
        material_id: bytes[SIZED_HEADER_LEN..end].to_vec(),
    })
}

fn load_legacy(bytes: &[u8]) -> Result<LevelData, LevelError> {
    let cells = LEGACY_WIDTH as usize * LEGACY_HEIGHT as usize; // 176400
    if bytes.len() < cells {
        return Err(LevelError::Truncated);
    }
    Ok(LevelData {
        width: LEGACY_WIDTH,
        height: LEGACY_HEIGHT,
        material_id: bytes[0..cells].to_vec(),
    })
}
```

- [ ] **Step 4: Run the test, verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: PASS (`loads_ollevel2_dimensions_and_materials ... ok`).

- [ ] **Step 5: Commit**

```bash
git -C . add rust/assets/src/level.rs
git -C . commit -m "feat(assets): load OLLEVEL2 sized level material map"
```

---

### Task 3: `load` — legacy 504×350 format

**Files:**
- Modify: `rust/assets/src/level.rs`

**Interfaces:**
- Consumes: `load`, `LevelData` from Task 2.
- Produces: legacy-format behaviour in the same `load`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `rust/assets/src/level.rs`:
```rust
    #[test]
    fn loads_legacy_504x350_when_no_magic() {
        // 176400 bytes, first 8 != "OLLEVEL2" so it is legacy.
        let buf: Vec<u8> = (0..504 * 350).map(|i| (i % 251) as u8).collect();
        let lvl = load(&buf).unwrap();
        assert_eq!(lvl.width, 504);
        assert_eq!(lvl.height, 350);
        assert_eq!(lvl.material_id.len(), 504 * 350);
        assert_eq!(lvl.material_id[0], 0);
        assert_eq!(lvl.material_id[7], 7);
        assert_eq!(lvl.material_id[176399], ((176399 % 251) as u8));
    }
```

- [ ] **Step 2: Run the test, verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: PASS. (Legacy parsing already exists from Task 2's implementation; this test pins the behaviour and the byte values. If it fails, fix `load_legacy` before proceeding.)

- [ ] **Step 3: Commit**

```bash
git -C . add rust/assets/src/level.rs
git -C . commit -m "test(assets): pin legacy 504x350 level loading"
```

---

### Task 4: `load` — validation and error cases

**Files:**
- Modify: `rust/assets/src/level.rs`

**Interfaces:**
- Consumes: `load`, `LevelError` from Task 2.
- Produces: verified error behaviour (no new public API).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `rust/assets/src/level.rs`:
```rust
    #[test]
    fn rejects_truncated_sized_header() {
        let buf = b"OLLEVEL2\x00\x0d".to_vec(); // magic + version + 1 byte (header incomplete)
        assert_eq!(load(&buf), Err(LevelError::Truncated));
    }

    #[test]
    fn rejects_truncated_sized_body() {
        let mut buf = make_ollevel2(4, 4, |_| 0);
        buf.truncate(SIZED_HEADER_LEN + 5); // fewer than 16 material bytes
        assert_eq!(load(&buf), Err(LevelError::Truncated));
    }

    #[test]
    fn rejects_zero_dimensions() {
        let buf = make_ollevel2(0, 10, |_| 0); // width 0 is invalid
        assert_eq!(load(&buf), Err(LevelError::BadDimensions(0, 10)));
    }

    #[test]
    fn rejects_truncated_legacy() {
        let buf = vec![1u8; 1000]; // far fewer than 176400, no magic
        assert_eq!(load(&buf), Err(LevelError::Truncated));
    }
```

- [ ] **Step 2: Run the tests, verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: PASS (4 new tests). The Task 2 implementation already returns these errors; this task pins them. If any fails, fix `load_sized`/`load_legacy` before proceeding.

- [ ] **Step 3: Commit**

```bash
git -C . add rust/assets/src/level.rs
git -C . commit -m "test(assets): pin level loader validation and error cases"
```

---

### Task 5: C++ oracle dumper + golden file

**Files:**
- Create: `src/tools/oracle_dump/level_dump.cpp`
- Modify: `CMakeLists.txt`
- Create: `rust/oracle-tests/gen_level_golden.sh`
- Create (generated, committed): `rust/oracle-tests/golden/level.txt`

**Interfaces:**
- Consumes: the `game` CMake target; `src/game/{level,common,settings}.hpp`, `io/stream.hpp`.
- Produces: `golden/level.txt` — three lines, each `<width> <height> <fnv1a_hex16>` for: the real `modern_test.lev`, the synthetic legacy buffer, the synthetic OLLEVEL2 buffer (in that order).

- [ ] **Step 1: Write the dumper**

`src/tools/oracle_dump/level_dump.cpp`:
```cpp
// Generates the golden material-map digest for the Rust level differential test.
// Runs the REAL C++ Level::load on three inputs and writes one FNV-1a hash per
// material map. Links the `game` library; built via the OPENLIERO_BUILD_ORACLE_DUMP
// CMake option (see gen_level_golden.sh). Not part of the default build.
#include <cstdint>
#include <cstdio>
#include <fstream>
#include <iterator>
#include <vector>

#include "common.hpp"
#include "io/stream.hpp"
#include "level.hpp"
#include "settings.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

std::vector<uint8_t> SlurpFile(char const* path) {
  std::ifstream f(path, std::ios::binary);
  return std::vector<uint8_t>(std::istreambuf_iterator<char>(f),
                              std::istreambuf_iterator<char>());
}

// MUST match the Rust test's synthetic inputs exactly.
std::vector<uint8_t> MakeLegacy() {
  std::vector<uint8_t> b(504 * 350);
  for (std::size_t i = 0; i < b.size(); ++i) {
    b[i] = static_cast<uint8_t>(i % 251);
  }
  return b;
}

std::vector<uint8_t> MakeOllevel2() {
  std::vector<uint8_t> b = {'O', 'L', 'L', 'E', 'V', 'E', 'L', '2'};
  b.push_back(0);   // version
  b.push_back(13);  // width LE
  b.push_back(0);
  b.push_back(11);  // height LE
  b.push_back(0);
  for (int i = 0; i < 13 * 11; ++i) {
    b.push_back(static_cast<uint8_t>((i * 5 + 2) % 256));
  }
  return b;
}

void DumpOne(std::FILE* out, Common& common, Settings const& settings,
             std::vector<uint8_t> const& buf) {
  io::MemReader r(buf);
  Level level(common);
  if (!level.load(common, settings, r)) {
    std::fprintf(stderr, "Level::load failed\n");
    std::exit(1);
  }
  std::fprintf(out, "%d %d %016llx\n", level.width, level.height,
               static_cast<unsigned long long>(Fnv1a(level.material_id)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_level <modern.lev> <out.txt>\n");
    return 1;
  }
  Common common;
  for (auto& m : common.materials) {
    m.flags = 0;  // FillMaterials, as in test_sized_level.cpp
  }
  Settings settings;
  settings.load_powerlevel_palette = false;

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  DumpOne(out, common, settings, SlurpFile(argv[1]));  // real modern_test.lev
  DumpOne(out, common, settings, MakeLegacy());
  DumpOne(out, common, settings, MakeOllevel2());
  std::fclose(out);
  return 0;
}
```

- [ ] **Step 2: Add the CMake target behind an option**

Append to `CMakeLists.txt` (near the other optional tools, after the existing options):
```cmake
option(OPENLIERO_BUILD_ORACLE_DUMP "Build the Rust differential-test oracle dumpers" OFF)
if(OPENLIERO_BUILD_ORACLE_DUMP)
  add_executable(oracle_dump_level src/tools/oracle_dump/level_dump.cpp)
  target_link_libraries(oracle_dump_level PRIVATE game)
endif()
```

- [ ] **Step 3: Write the generator script**

`rust/oracle-tests/gen_level_golden.sh`:
```bash
#!/usr/bin/env bash
# Regenerates golden/level.txt by running the REAL C++ Level::load.
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL
# step — it is NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_level
"build/$PRESET/Release/oracle_dump_level" \
  "$ROOT/data/TC/openliero/Levels/modern_test.lev" \
  "$ROOT/rust/oracle-tests/golden/level.txt"
echo "wrote rust/oracle-tests/golden/level.txt"
```

- [ ] **Step 4: Generate the golden file**

Run: `bash rust/oracle-tests/gen_level_golden.sh`
Expected: `wrote rust/oracle-tests/golden/level.txt`. The file has exactly 3 lines; line 1 starts with `504 350 ` (modern_test.lev loads as legacy 504×350), line 2 starts with `504 350 `, line 3 starts with `13 11 `.

- [ ] **Step 5: Verify the file shape**

Run: `cat rust/oracle-tests/golden/level.txt`
Expected: 3 lines of the form `<w> <h> <16 hex digits>`. Confirm widths/heights are `504 350`, `504 350`, `13 11`.

- [ ] **Step 6: Commit**

```bash
git -C . add src/tools/oracle_dump/level_dump.cpp CMakeLists.txt rust/oracle-tests/gen_level_golden.sh rust/oracle-tests/golden/level.txt
git -C . commit -m "feat(oracle): C++ level dumper + golden material-map digest"
```

---

### Task 6: Rust golden differential test

**Files:**
- Create: `rust/oracle-tests/tests/level_golden.rs`

**Interfaces:**
- Consumes: `assets::level::load`; `golden/level.txt`; `data/TC/openliero/Levels/modern_test.lev`.
- Produces: the differential test proving Rust matches C++.

- [ ] **Step 1: Write the failing test**

`rust/oracle-tests/tests/level_golden.rs`:
```rust
//! Differential test for the level material-map loader against the C++ oracle.
//! See `fixed_golden.rs` for the golden pattern. The golden stores an FNV-1a hash
//! of each material map (computed by the real C++ `Level::load`); the Rust loader
//! must reproduce the same dimensions and hash for all three inputs.

use assets::level::load;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// MUST match the C++ dumper's synthetic inputs exactly.
fn make_legacy() -> Vec<u8> {
    (0..504 * 350).map(|i| (i % 251) as u8).collect()
}

fn make_ollevel2() -> Vec<u8> {
    let mut b = b"OLLEVEL2".to_vec();
    b.push(0);
    b.extend_from_slice(&13u16.to_le_bytes());
    b.extend_from_slice(&11u16.to_le_bytes());
    for i in 0..13 * 11 {
        b.push(((i * 5 + 2) % 256) as u8);
    }
    b
}

#[test]
fn level_material_map_matches_cpp_oracle() {
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

    let inputs: [Vec<u8>; 3] = [modern, make_legacy(), make_ollevel2()];
    let mut lines = golden.lines();

    for (idx, buf) in inputs.iter().enumerate() {
        let line = lines.next().unwrap_or_else(|| panic!("missing golden line {idx}"));
        let mut it = line.split_whitespace();
        let want_w: i32 = it.next().unwrap().parse().unwrap();
        let want_h: i32 = it.next().unwrap().parse().unwrap();
        let want_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();

        let lvl = load(buf).unwrap_or_else(|e| panic!("input {idx} failed: {e:?}"));
        assert_eq!(lvl.width, want_w, "width mismatch input {idx}");
        assert_eq!(lvl.height, want_h, "height mismatch input {idx}");
        assert_eq!(lvl.material_id.len(), (want_w * want_h) as usize);
        assert_eq!(fnv1a(&lvl.material_id), want_hash, "material hash mismatch input {idx}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 2: Run the test, verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p oracle-tests --test level_golden`
Expected: PASS (`level_material_map_matches_cpp_oracle ... ok`). If the hash mismatches for input 0, the Rust loader and C++ disagree on `modern_test.lev`'s material map — debug the loader (do NOT edit the golden).

- [ ] **Step 3: Commit**

```bash
git -C . add rust/oracle-tests/tests/level_golden.rs
git -C . commit -m "test(oracle): level material map differential test vs C++"
```

---

### Task 7: README + full verification

**Files:**
- Modify: `rust/README.md`

**Interfaces:**
- Consumes: all earlier tasks.
- Produces: documentation + a green whole-workspace run.

- [ ] **Step 1: Document the assets crate and the level oracle**

Add to `rust/README.md`, after the existing crate list, a new section:
```markdown
## assets crate

`assets` loads OpenLiero's on-disk formats (no Bevy). Behaviour that feeds the
simulation is differential-tested against the C++ engine; the implementation is
idiomatic Rust (`std::io`/`from_le_bytes`, `Result`), not a port of the C++ `io`
layer.

- `level` — `.lev` material-map loader (legacy 504×350 + OLLEVEL2 sized).

### Level golden

Unlike the math golden (cheap standalone clang build), the level golden runs the
real `Level::load` and so needs the full C++ build. Regenerate it locally:

\`\`\`bash
bash rust/oracle-tests/gen_level_golden.sh      # PRESET=linux-x64 on Linux
\`\`\`

The lightweight `rust.yml` CI does not regenerate it; it runs `cargo test` against
the committed `golden/level.txt`.
```

- [ ] **Step 2: Run the whole workspace test suite**

Run: `cargo test --manifest-path rust/Cargo.toml --workspace`
Expected: PASS — step-0 golden suites + the `assets` unit tests + `level_golden`.

- [ ] **Step 3: Commit**

```bash
git -C . add rust/README.md
git -C . commit -m "docs(assets): document the assets crate and level golden flow"
```

---

## Self-Review

**Spec coverage:** Every item in the 1a+1b spec's "Included" scope maps to a task — `assets` crate (T1), idiomatic `load(&[u8])` for OLLEVEL2 (T2) and legacy (T3) with `Result`/`from_le_bytes` and no Reader port, validation (T4), C++ dumper linking `game` via a CMake option with the `test_sized_level.cpp` scaffolding (T5), Rust golden differential test over the real file + two synthetics (T6), README/CI note (T7). "Definition of done" is covered: crate builds dependency-free (T1), `cargo test` green incl. `level_golden` (T6/T7), real `modern_test.lev` round-trips (T6), both synthetic formats match (T6), committed CMake-built dumper + reproducible script (T5), CI runs the test (T7, via existing `cargo test --workspace`).

**Placeholder scan:** No TBD/TODO; all code and commands are concrete.

**Type consistency:** `LevelData { width: i32, height: i32, material_id: Vec<u8> }`, `LevelError::{Truncated, BadDimensions}`, and `load(&[u8]) -> Result<LevelData, LevelError>` are defined in T2 and used unchanged in T3/T4/T6. The synthetic input patterns and the FNV-1a constants are byte-for-byte identical between the C++ dumper (T5) and the Rust test (T6). Golden line order (modern, legacy, ollevel2) matches the dumper's write order and the test's read order.
```
