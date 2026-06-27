# Step 1e-3 — WAV sounds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read OpenLiero's WAV sounds (`sounds/<name>.wav`) in Rust, reproducing the C++ engine's decoded 8-bit PCM `original_data` byte-for-byte, plus the `CreateSound` `int16` upsample, by adding a `wav` module to the `assets` crate (a hand-rolled RIFF/WAVE reader, no new dependencies).

**Architecture:** `wav.rs` parses OpenLiero's single accepted WAV shape (fixed 44-byte canonical header: PCM, mono, 22050 Hz, 8-bit) with fixed-offset slicing into a typed `WavSound { original_data: Vec<u8> }`, each byte `= raw - 128` (≡ `raw ^ 0x80`). `WavSound::upsampled()` reproduces `CreateSound`'s 2× linear-interpolated `int16` samples. Correctness is proven by a C++ oracle dumper that runs the real `Common::load` and a golden differential test that walks `tc.cfg`'s `types.sounds` and reproduces its FNV-1a digests.

**Tech Stack:** Rust (`assets`, `oracle-tests`), `std` only (no new deps), C++ oracle dumper (`oracle_dump_wav`, links `game` + `mixer/mixer.hpp`), CMake option `OPENLIERO_BUILD_ORACLE_DUMP`, FNV-1a digests. Depends on 1e-1 (`assets::tc::TcConfig` for the sound name list/order).

## Global Constraints

- **Bit-exact vs C++ for the decoded bytes.** Source of truth: `src/game/common.cpp:325-363` (`Common::load` WAV loop), `src/game/common.cpp:583-602` (`SfxSample::CreateSound`), `src/game/common.hpp:78-120` (`struct SfxSample`; `original_data` is `std::vector<uint8_t>`), `src/game/io/coding.hpp:78-93` (`ReadUint16Le`/`ReadUint32Le`), `src/game/io/stream.hpp:35` (`Reader::Get` → `uint8_t`). Each sound's `original_data` (the LOCKED read) and the `CreateSound` upsample must reproduce C++ exactly.
- **Single accepted WAV shape** (`common.cpp:340-349`): fixed 44-byte canonical header — `'RIFF'`; riff-size `u32` (read, **ignored**); `'WAVE'`; `'fmt '`; fmt size `== 16`; audio format `== 1` (PCM); channels `== 1` (mono); sample rate `== 22050`; byte rate `== 22050`; block align `== 1`; bits `== 8`; `'data'`; then `dataSize` (`u32`) bytes. `data` MUST sit at offset 36 (no chunk-walking). Confirmed by hexdump of the shipped files.
- **Decode semantics:** `original_data[i] = raw_byte.wrapping_sub(128)` — C++ `z = r.Get() - 128` with `Get()` returning `uint8_t` wraps in `uint8_t` (≡ `raw ^ 0x80`). **Upsample semantics** (`CreateSound`): reinterpret each stored byte as `int8`, `× 30`; emit `prev`, then for each subsequent sample emit the tween `(prev+cur)/2` (integer division) and `cur`, then a trailing `prev`. Output length `== 2 * original_data.len()`; empty for an empty sound.
- **Audio, NOT sim-affecting.** No `processFrame` logic reads sample data; 1e-3 gates no determinism. We still golden-verify it ("read the bytes the same"), labeled non-sim. The `sfx_sound*` handle / `SfxNewSound` / SDL mixer are DROPPED (step-3 audio backend).
- **Missing/invalid tolerance is the caller's job.** C++ tolerates a missing file (`continue`, silent slot) or a non-matching header (left empty). `wav.rs` is the strict decoder (present-but-malformed → typed `Err`); the golden test maps missing-file → `WavSound::default()` (silent slot). All 30 shipped files decode cleanly.
- **Idiomatic Rust, not a port:** fixed-offset slicing + `from_le_bytes`, typed `Result`/error enum, `wrapping_sub`/`as i8 as i32` for the documented wraps — NOT a port of the streaming `io::Reader`. No `hound` dependency (like 1d's hand-rolled TGA). `wav.rs` is pure `std`.
- **FNV-1a (64-bit)** seed `0xcbf29ce484222325`, prime `0x100000001b3` — identical helper on both sides (see `rust/oracle-tests/tests/sprite_golden.rs` / `tc_golden.rs`). `original_data` is hashed as raw bytes; the `int16` upsample as explicit little-endian byte pairs.
- **Sound names/order from tc.cfg.** C++ sets `sounds[i].name = types.sounds[i]` (`common_model.hpp:580-585`). The golden test reads `assets::tc::TcConfig` to get the name list/order — 1e-3 depends on 1e-1.
- **No Bevy** in `assets`. **Golden regeneration is LOCAL/MANUAL** (full C++ build links `game`); CI (`rust.yml`) runs `cargo test --workspace` against the committed golden. PRESET defaults to `macos-arm64`.
- **No AI/"Generated with" taglines** in commits. C++ matches the existing `sprite_dump.cpp` Google/100-col style.

## File Structure

- `rust/assets/src/wav.rs` — NEW: `WavSound` + `WavError` + `load` + `upsampled` + unit tests.
- `rust/assets/src/lib.rs` — MODIFY: `pub mod wav;`.
- `src/tools/oracle_dump/wav_dump.cpp` — NEW: runs real `Common::load`, dumps per-sound `original_data` + `CreateSound` digests.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_wav` target inside the existing `OPENLIERO_BUILD_ORACLE_DUMP` block (after `oracle_dump_tc`).
- `rust/oracle-tests/gen_wav_golden.sh` — NEW: regenerate the wav golden.
- `rust/oracle-tests/golden/wav.txt` — NEW: committed golden (generated).
- `rust/oracle-tests/tests/wav_golden.rs` — NEW: wav differential test.

(`rust/assets/Cargo.toml` is **unchanged** — `wav.rs` uses only `std`. `rust/oracle-tests/Cargo.toml` already depends on `assets`, used by `tc_golden.rs`.)

---

### Task 0: Stub `wav.rs` and smoke-test a real file

De-risk the fixed-header decode against a real shipped `.wav` **before** building error paths.

**Files:**
- Create (stub): `rust/assets/src/wav.rs`
- Modify: `rust/assets/src/lib.rs`

- [ ] **Step 1: Stub `wav.rs` with the type, `load`, and a smoke test**

Create `rust/assets/src/wav.rs`:

```rust
//! WAV sound loading (`sounds/<name>.wav`). Reproduces the decoded 8-bit PCM
//! `original_data` that C++ `Common::load` (`src/game/common.cpp:325-363`)
//! stores for each sound, plus the `CreateSound` upsample
//! (`common.cpp:583-602`). Audio, NOT sim-affecting (no `processFrame` reads
//! sample data) — but "read the bytes the same": the decode is golden-pinned
//! vs C++. Idiomatic Rust (fixed-offset slicing + typed errors), not a port of
//! the streaming `io::Reader`.

/// Why a WAV failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum WavError {
    /// A header field did not match OpenLiero's single accepted WAV shape
    /// (`common.cpp:340-349`).
    BadHeader,
    /// The buffer ended before the 44-byte header or the declared PCM payload.
    Truncated,
}

/// A decoded OpenLiero sound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WavSound {
    /// 8-bit PCM, one byte per sample, each `= raw.wrapping_sub(128)`
    /// (equivalently `raw ^ 0x80`), exactly as C++ stores it
    /// (`common.cpp:354-356`). This is the LOCKED, golden-verified artifact.
    pub original_data: Vec<u8>,
}

/// Fixed canonical header length (`data` payload starts here).
const HEADER_LEN: usize = 44;

fn le_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn le_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

impl WavSound {
    /// Decode an OpenLiero `.wav`. Mirrors the load loop in `Common::load`
    /// (`common.cpp:340-356`): validate the fixed 44-byte RIFF/WAVE header
    /// (PCM, mono, 22050 Hz, 8-bit), then read `dataSize` bytes as `raw - 128`.
    pub fn load(bytes: &[u8]) -> Result<WavSound, WavError> {
        if bytes.len() < HEADER_LEN {
            return Err(WavError::Truncated);
        }
        // C++ reads 'RIFF', then the riff size (ignored), then validates the
        // remaining fields in one short-circuit `&&` chain. We check the same
        // fields at their fixed offsets.
        let ok = &bytes[0..4] == b"RIFF"
            // bytes[4..8] = riff size: read and ignored (common.cpp:341-343).
            && &bytes[8..12] == b"WAVE"
            && &bytes[12..16] == b"fmt "
            && le_u32(bytes, 16) == 16        // fmt chunk size
            && le_u16(bytes, 20) == 1         // audio format = PCM
            && le_u16(bytes, 22) == 1         // channels = mono
            && le_u32(bytes, 24) == 22050     // sample rate
            && le_u32(bytes, 28) == 22050     // byte rate (22050*1*1)
            && le_u16(bytes, 32) == 1         // block align (1*1)
            && le_u16(bytes, 34) == 8         // bits per sample
            && &bytes[36..40] == b"data";
        if !ok {
            return Err(WavError::BadHeader);
        }
        let data_size = le_u32(bytes, 40) as usize;
        let end = HEADER_LEN + data_size;
        if bytes.len() < end {
            return Err(WavError::Truncated);
        }
        // z = r.Get() - 128, wrapping in u8 (Get() returns uint8_t) == raw ^ 0x80.
        let original_data = bytes[HEADER_LEN..end]
            .iter()
            .map(|&b| b.wrapping_sub(128))
            .collect();
        Ok(WavSound { original_data })
    }
}

#[cfg(test)]
mod smoke {
    use super::*;

    /// The real shipped bump.wav must decode, with original_data length equal to
    /// (file length - 44-byte header). Proves the fixed-header decode against a
    /// real file; failure here is a format-assumption signal, not a code bug.
    #[test]
    fn real_bump_wav_decodes() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sounds/bump.wav"
        ));
        let snd = WavSound::load(bytes).expect("bump.wav decodes");
        assert_eq!(snd.original_data.len(), bytes.len() - 44);
    }
}
```

Add to `rust/assets/src/lib.rs` after the `pub mod tc;` line:

```rust
pub mod wav;
```

- [ ] **Step 2: Run the smoke test**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets wav::smoke`
Expected: PASS. If `bump.wav` does not decode, STOP and report (the format assumption is wrong — re-check the header against `common.cpp:340-349`).

- [ ] **Step 3: Commit**

```bash
git add rust/assets/src/wav.rs rust/assets/src/lib.rs
git commit -m "feat(assets): wav decoder stub + bump.wav smoke test"
```

---

### Task 1: `wav.rs` — add `upsampled()` and full unit tests

`load` already exists from Task 0; this task adds the `CreateSound` upsample and the error/edge-path unit tests.

**Files:**
- Modify: `rust/assets/src/wav.rs` (add `upsampled` + replace the `smoke` module with full `tests`)

**Interfaces:**
- Consumes: nothing new (`std` only).
- Produces (all `pub`): `WavSound { original_data: Vec<u8> }`, `WavError { BadHeader, Truncated }`, `WavSound::load(&[u8]) -> Result<WavSound, WavError>`, `WavSound::upsampled(&self) -> Vec<i16>`.

- [ ] **Step 1: Add `upsampled()` to the `impl WavSound` block**

Insert after `load` (before the closing `}` of `impl WavSound`):

```rust
    /// The `int16` playback samples C++ `SfxSample::CreateSound`
    /// (`common.cpp:583-602`) produces: a 2x linear-interpolated upsample of
    /// `original_data * 30`. Each stored byte is reinterpreted as `int8`
    /// (undoing the `^ 0x80`), scaled by 30; an averaged tween is inserted
    /// between neighbours. Length is `2 * original_data.len()` (empty for an
    /// empty sound). Audio-only; SDL playback lives in step 3.
    pub fn upsampled(&self) -> Vec<i16> {
        if self.original_data.is_empty() {
            return Vec::new();
        }
        let mut samples = Vec::with_capacity(self.original_data.len() * 2);
        let mut prev = (self.original_data[0] as i8 as i32) * 30;
        samples.push(prev as i16);
        for &b in &self.original_data[1..] {
            let cur = (b as i8 as i32) * 30;
            samples.push(((prev + cur) / 2) as i16); // interpolated tween
            samples.push(cur as i16);
            prev = cur;
        }
        samples.push(prev as i16);
        samples
    }
```

- [ ] **Step 2: Replace the `smoke` module with the full `tests` module**

Replace the entire `#[cfg(test)] mod smoke { ... }` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid OpenLiero WAV (44-byte canonical header + `data`).
    fn wav(data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes()); // riff size
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&22050u32.to_le_bytes()); // sample rate
        v.extend_from_slice(&22050u32.to_le_bytes()); // byte rate
        v.extend_from_slice(&1u16.to_le_bytes()); // block align
        v.extend_from_slice(&8u16.to_le_bytes()); // bits
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes()); // data size
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn real_bump_wav_decodes() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sounds/bump.wav"
        ));
        let snd = WavSound::load(bytes).expect("bump.wav decodes");
        assert_eq!(snd.original_data.len(), bytes.len() - 44);
    }

    #[test]
    fn decode_offsets_each_byte_minus_128() {
        // raw 0x80->0x00, 0x00->0x80, 0xff->0x7f, 0x7c->0xfc (== raw ^ 0x80).
        let snd = WavSound::load(&wav(&[0x80, 0x00, 0xff, 0x7c])).unwrap();
        assert_eq!(snd.original_data, vec![0x00, 0x80, 0x7f, 0xfc]);
    }

    #[test]
    fn zero_length_data_is_ok_and_empty() {
        let snd = WavSound::load(&wav(&[])).unwrap();
        assert!(snd.original_data.is_empty());
        assert!(snd.upsampled().is_empty());
    }

    #[test]
    fn upsample_matches_create_sound() {
        // raw [0x80, 0x82] -> original_data [0, 2]; (int8)0*30=0, (int8)2*30=60.
        // CreateSound: push 0; tween (0+60)/2=30; push 60; trailing prev=60.
        let snd = WavSound::load(&wav(&[0x80, 0x82])).unwrap();
        assert_eq!(snd.original_data, vec![0u8, 2u8]);
        let up = snd.upsampled();
        assert_eq!(up, vec![0i16, 30, 60, 60]);
        assert_eq!(up.len(), snd.original_data.len() * 2);
    }

    #[test]
    fn upsample_handles_negative_int8() {
        // raw 0x7c -> original_data 0xfc -> (int8)0xfc = -4 -> -4*30 = -120.
        let snd = WavSound::load(&wav(&[0x7c])).unwrap();
        assert_eq!(snd.original_data, vec![0xfcu8]);
        assert_eq!(snd.upsampled(), vec![-120i16, -120]); // n=1 -> [prev, prev]
    }

    #[test]
    fn bad_header_fields_rejected() {
        // Each corruption of a validated field -> BadHeader.
        let mut bad_riff = wav(&[1, 2, 3]);
        bad_riff[0] = b'X';
        assert_eq!(WavSound::load(&bad_riff), Err(WavError::BadHeader));

        let mut bad_rate = wav(&[1, 2, 3]);
        bad_rate[24] = 0x44; // sample rate 0x4422 != 22050
        assert_eq!(WavSound::load(&bad_rate), Err(WavError::BadHeader));

        let mut bad_bits = wav(&[1, 2, 3]);
        bad_bits[34] = 16; // 16-bit, not 8
        assert_eq!(WavSound::load(&bad_bits), Err(WavError::BadHeader));

        let mut bad_data = wav(&[1, 2, 3]);
        bad_data[36] = b'L'; // "Lata" != "data"
        assert_eq!(WavSound::load(&bad_data), Err(WavError::BadHeader));
    }

    #[test]
    fn truncated_header_and_payload() {
        // Shorter than the 44-byte header.
        assert_eq!(WavSound::load(b"RIFF"), Err(WavError::Truncated));
        // Header claims 10 data bytes but only 3 are present.
        let mut short = wav(&[1, 2, 3]);
        let len = short.len();
        short[40..44].copy_from_slice(&10u32.to_le_bytes());
        assert_eq!(len, 47); // 44 + 3
        assert_eq!(WavSound::load(&short), Err(WavError::Truncated));
    }
}
```

- [ ] **Step 3: Run the unit tests**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets wav`
Expected: all `wav` tests PASS (7 tests).

- [ ] **Step 4: Run the full assets suite (no regressions)**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: prior assets tests (level/palette/sprite/tc) still PASS plus the new wav tests.

- [ ] **Step 5: Commit**

```bash
git add rust/assets/src/wav.rs
git commit -m "feat(assets): wav original_data decode + CreateSound upsample"
```

---

### Task 2: wav golden — differential test vs real `Common::load`

Prove the decoded `original_data` + upsample match the C++ engine bit-for-bit.

**Files:**
- Create: `src/tools/oracle_dump/wav_dump.cpp`
- Modify: `CMakeLists.txt` (inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block)
- Create: `rust/oracle-tests/gen_wav_golden.sh`
- Create: `rust/oracle-tests/golden/wav.txt` (generated)
- Create: `rust/oracle-tests/tests/wav_golden.rs`

**Interfaces:**
- Consumes: `WavSound` (Task 1); `assets::tc::TcConfig` (1e-1) for the sound name list; the real C++ `common.sounds[]` (`name`, `original_data`, and `SfxSoundData(sound)`).
- Produces: golden file — one `name orig_len orig_hash up_len up_hash` line per sound.

- [ ] **Step 1: Write the C++ wav dumper**

Create `src/tools/oracle_dump/wav_dump.cpp`:

```cpp
// Generates golden digests for the Rust WAV differential test by running the
// REAL C++ Common::load (which decodes each sounds/<name>.wav into
// original_data and calls CreateSound). Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the default build.
// Usage: oracle_dump_wav <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"
#include "mixer/mixer.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// FNV-1a over original_data's raw bytes (a byte buffer, hashed directly).
uint64_t HashBytes(std::vector<uint8_t> const& data) {
  std::vector<unsigned char> b(data.begin(), data.end());
  return Fnv1a(b);
}

// FNV-1a over int16 samples as explicit little-endian byte pairs.
uint64_t HashSamples(std::vector<int16_t> const& s) {
  std::vector<unsigned char> b;
  b.reserve(s.size() * 2);
  for (int16_t v : s) {
    uint16_t u = static_cast<uint16_t>(v);
    b.push_back(static_cast<unsigned char>(u & 0xff));
    b.push_back(static_cast<unsigned char>((u >> 8) & 0xff));
  }
  return Fnv1a(b);
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_wav <tc-dir> <out.txt>\n");
    return 1;
  }
  Common common;
  common.load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  for (auto const& s : common.sounds) {
    uint64_t orig_hash = HashBytes(s.original_data);
    // Silent slot (missing/invalid file): sound == nullptr, no samples. Matches
    // the Rust WavSound::default().upsampled() == empty path.
    std::size_t up_len = 0;
    uint64_t up_hash = Fnv1a({});
    if (s.sound) {
      std::vector<int16_t>& samples = SfxSoundData(s.sound);
      up_len = samples.size();
      up_hash = HashSamples(samples);
    }
    std::fprintf(out, "%s %zu %016llx %zu %016llx\n", s.name.c_str(),
                 s.original_data.size(),
                 static_cast<unsigned long long>(orig_hash), up_len,
                 static_cast<unsigned long long>(up_hash));
  }

  std::fclose(out);
  return 0;
}
```

- [ ] **Step 2: Register the CMake target**

In `CMakeLists.txt`, inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block (after the `oracle_dump_tc` lines, before `endif()`), add:

```cmake
  add_executable(oracle_dump_wav src/tools/oracle_dump/wav_dump.cpp)
  target_link_libraries(oracle_dump_wav PRIVATE game)
```

(`game` already exposes `mixer/mixer.hpp`'s `SfxSoundData`, so no extra link is needed.)

- [ ] **Step 3: Write the regeneration script**

Create `rust/oracle-tests/gen_wav_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/wav.txt by running the REAL C++ Common::load (which decodes
# each sounds/<name>.wav into original_data and calls CreateSound). Needs the
# full C++ build (links the `game` target), so this is a LOCAL/MANUAL step —
# NOT run in the lightweight rust.yml CI. Override PRESET for other platforms
# (e.g. linux-x64). Run from the repo root so the TC dir resolves the same way
# the in-tree tests do.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_wav
(
  cd "$ROOT"
  "build/$PRESET/Release/oracle_dump_wav" \
    "data/TC/openliero" \
    "rust/oracle-tests/golden/wav.txt"
)
echo "wrote rust/oracle-tests/golden/wav.txt"
```

Make it executable:

```bash
chmod +x rust/oracle-tests/gen_wav_golden.sh
```

- [ ] **Step 4: Generate the golden**

Run: `bash rust/oracle-tests/gen_wav_golden.sh`
Expected: prints `wrote rust/oracle-tests/golden/wav.txt`; the file has 30 lines (one per shipped sound), each `<name> <orig_len> <16-hex> <up_len> <16-hex>`, with `up_len == 2 * orig_len` for every non-empty sound.

- [ ] **Step 5: Write the Rust golden test**

Create `rust/oracle-tests/tests/wav_golden.rs`:

```rust
//! Differential test for the WAV loader against the C++ oracle. The golden (one
//! `name orig_len orig_hash up_len up_hash` line per sound) is produced by the
//! real C++ `Common::load`, which decodes original_data and calls CreateSound.
//! The Rust loader must reproduce every FNV-1a digest from the same shipped
//! `data/TC/openliero/sounds/<name>.wav`, walked in tc.cfg's sound order.

use std::collections::HashMap;

use assets::tc::TcConfig;
use assets::wav::WavSound;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hash_i16(samples: &[i16]) -> u64 {
    let mut b = Vec::with_capacity(samples.len() * 2);
    for &v in samples {
        b.extend_from_slice(&v.to_le_bytes());
    }
    fnv1a(&b)
}

struct Row {
    orig_len: usize,
    orig_hash: u64,
    up_len: usize,
    up_hash: u64,
}

fn parse_golden(text: &str) -> HashMap<String, Row> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next().unwrap().to_string();
            let orig_len = it.next().unwrap().parse().unwrap();
            let orig_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            let up_len = it.next().unwrap().parse().unwrap();
            let up_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            (name, Row { orig_len, orig_hash, up_len, up_hash })
        })
        .collect()
}

const TC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

#[test]
fn wav_sounds_match_cpp_oracle() {
    let golden = parse_golden(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/wav.txt"
    )));

    // Sound names + order come from tc.cfg (1e-1), exactly like C++ Common::load
    // (sounds[i].name = types.sounds[i]).
    let tc_bytes = std::fs::read(format!("{TC_DIR}/tc.cfg")).expect("read tc.cfg");
    let cfg = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    assert_eq!(cfg.types.sounds.len(), golden.len(), "sound count vs golden");

    for name in &cfg.types.sounds {
        let want = golden
            .get(name)
            .unwrap_or_else(|| panic!("missing golden line for {name}"));

        // Mirror Common::load's tolerance: a missing file is a silent slot.
        let path = format!("{TC_DIR}/sounds/{name}.wav");
        let snd = match std::fs::read(&path) {
            Ok(bytes) => WavSound::load(&bytes)
                .unwrap_or_else(|e| panic!("decode {name}: {e:?}")),
            Err(_) => WavSound::default(),
        };

        assert_eq!(snd.original_data.len(), want.orig_len, "{name} orig_len");
        assert_eq!(fnv1a(&snd.original_data), want.orig_hash, "{name} orig_hash");

        let up = snd.upsampled();
        assert_eq!(up.len(), want.up_len, "{name} up_len");
        assert_eq!(hash_i16(&up), want.up_hash, "{name} up_hash");
    }
}
```

- [ ] **Step 6: Run the full workspace suite**

Run: `cargo test --manifest-path rust/Cargo.toml --workspace`
Expected: ALL tests PASS (sim-core goldens, assets unit incl. wav, level/palette/sprite/tc goldens, wav_golden).

- [ ] **Step 7: Commit**

```bash
git add src/tools/oracle_dump/wav_dump.cpp CMakeLists.txt \
  rust/oracle-tests/gen_wav_golden.sh rust/oracle-tests/golden/wav.txt \
  rust/oracle-tests/tests/wav_golden.rs
git commit -m "test(oracle): wav sounds differential test vs C++ Common::load"
```

---

## Self-Review

**Spec coverage:**
- Fixed 44-byte RIFF/WAVE header validation (`common.cpp:340-349`) → Task 0/1 (`load`) + unit `bad_header_fields_rejected` + golden. ✓
- `original_data` decode `raw - 128` (`common.cpp:354-356`) → Task 0 (`load`) + unit `decode_offsets_each_byte_minus_128` + golden `orig_hash`. ✓ (LOCKED read.)
- `CreateSound` upsample (`common.cpp:583-602`) → Task 1 (`upsampled`) + unit `upsample_matches_create_sound`/`upsample_handles_negative_int8` + golden `up_hash`. ✓
- Zero-length / truncation / missing-file tolerance → unit `zero_length_data_is_ok_and_empty` / `truncated_header_and_payload` + golden test's `WavSound::default()` branch. ✓
- Sound names/order from tc.cfg (1e-1 dependency) → golden test reads `assets::tc::TcConfig`. ✓
- Decode against real files → Task 0 smoke (`real_bump_wav_decodes`) + golden over all 30. ✓
- `sfx_sound`/SDL playback, bundle `Vec<Sound>` assembly → out of scope (step 3 / step-1e integration). ✓

**Placeholder scan:** No TBD/TODO; all Rust + C++ is complete. The Task 0 `smoke` module is replaced by the full `tests` module in Task 1 Step 2 (the `real_bump_wav_decodes` test is preserved inside it). `golden/wav.txt` is generated in Task 2 Step 4. ✓

**Type/byte-layout consistency:** `original_data` is `Vec<u8>` (≙ C++ `std::vector<uint8_t>`); hashed as raw bytes on both sides (`HashBytes` ↔ `fnv1a(&original_data)`). Upsample is `Vec<i16>` (≙ C++ `std::vector<int16_t>`); hashed as LE byte pairs on both sides (`HashSamples` ↔ `hash_i16`). `load`'s field offsets/required values transcribed verbatim from `common.cpp:340-350` and confirmed by hexdump. Decode wrap (`wrapping_sub(128)` ↔ `uint8_t(Get()-128)`) and upsample (`as i8 as i32 * 30`, `(prev+cur)/2` integer div, length `2n`) match `CreateSound` exactly. FNV seed/prime identical to the existing goldens. ✓

**Tolerance semantics:** C++ missing-file `continue` (silent slot) ↔ Rust golden test `WavSound::default()`; C++ silent skip of a non-matching header ↔ Rust strict `Err` at the decoder with tolerance pushed to the caller (documented). Shipped files all valid, so the locked golden is over clean decodes. ✓

**Charter posture:** audio, non-sim (no `processFrame` reads samples), but byte-verified ("read the bytes the same"). Dropped artifacts: `sfx_sound*`/`SfxNewSound`/SDL mixer. No new dependencies (`wav.rs` is pure `std`; hand-rolled reader like 1d's TGA). ✓

**Decision recorded:** `CreateSound` is reproduced in 1e-3 (not deferred) under the overview's "trivial & deterministic enough to golden cheaply" exception; deferral is a one-task/one-golden-line removal if the controller prefers strict-minimum. ✓
