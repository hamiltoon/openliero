# Step 1, sub-slice 1e-3 — WAV sounds: design

Status: **draft for review** · 2026-06-26
Part of: `2026-06-26-liero-rs-step1e-tc-bundle-overview.md`
Follows: `2026-06-26-liero-rs-step1e1-tc-config-design.md` (and, when written,
`…-step1e2-object-configs-design.md`)

Sub-slice 1e-3 — the **third and last** row of slice 1e — reads OpenLiero's
**WAV sounds** (`data/TC/openliero/sounds/<name>.wav`) in Rust, reproducing the
decoded 8-bit PCM the C++ engine stores in each `SfxSample::original_data`,
byte-for-byte. This adds a `wav` module to the `assets` crate (a small,
hand-rolled RIFF/WAVE reader, in the spirit of 1d's hand-rolled TGA parser — no
`hound` dependency). As always, the C++ engine is the oracle: a C++ dumper runs
the real `Common::load` and a golden differential test pins bit-exactness.

## Charter posture (read this first)

**WAV sample data is audio, NOT sim-affecting.** No `processFrame` logic reads
sample bytes; sounds are played, never simulated. So 1e-3 gates *no* determinism.
But — exactly as 1c (MODERNLV display data) and 1d (sprite pixels / `exepal`)
did for rendering data — we still **"read the bytes the same"**: the decoded
`original_data` (8-bit PCM) for every sound is the **locked, golden-tested
artifact**, reproduced byte-exact vs C++. This keeps the loader complete and
honest while labeling it non-sim so step 2 knows it may not depend on it.

What we deliberately drop (C++ in-memory artifacts, per the charter): the
`sfx_sound*` handle (`SfxNewSound`), the SDL/mixer playback wiring, and any
audio device state. Those are step-3 (audio backend) concerns.

## Scope

In scope (read identically, golden-verified):

- **A hand-rolled RIFF/WAVE reader** for OpenLiero's *single* accepted WAV shape
  (`common.cpp:340-349`): a fixed 44-byte canonical header (mono, 22050 Hz,
  8-bit PCM, `fmt `=16, `data` chunk), validated field-by-field.
- **`original_data` decode** (`common.cpp:352-356`): `dataSize` bytes, each
  stored as `raw_byte - 128` (`u8` wrapping; ≡ `raw ^ 0x80`). **This is the
  locked read.**
- **The `CreateSound` upsample** (`common.cpp:583-602`): the `int16` playback
  samples (2× linear-interpolated, `× 30`). **Reproduced here too**, with its own
  golden — see "The `original_data`-vs-`CreateSound` decision" for why this
  qualifies for the overview's "trivial & deterministic enough to golden cheaply"
  exception rather than being deferred to step 3.
- Sound **names and order** come from 1e-1's `tc.cfg` `types.sounds` list
  (`TcConfig::types.sounds`), exactly as C++ sets `sounds[i].name =
  types.sounds[i]` (`common_model.hpp:580-585`). 1e-3 depends on 1e-1 for this.

Explicitly out of scope (deferred):

- The `sfx_sound*` handle / `SfxNewSound` / `SfxSoundData` / mixer / SDL audio
  playback — step 3 (audio backend). We reproduce the *samples* `CreateSound`
  computes, not the playback machinery that owns them.
- The bundle-level *assembly* of a `Vec<Sound>` (name + tolerance for
  missing/invalid files) — that is the step-1e *integration* concern that wires
  1e-1/1e-2/1e-3 together; 1e-3 ships the pure per-file decoder. (The golden test
  does walk all 30 files via the `types.sounds` list, so the end-to-end path is
  still exercised.)
- Writing WAVs back out — we only read.

## Background: the C++ format (oracle truth)

Verified against `src/game/common.cpp:325-363` (`Common::load` WAV loop),
`src/game/common.cpp:583-602` (`SfxSample::CreateSound`),
`src/game/common.hpp:78-120` (`struct SfxSample`), `src/game/io/coding.hpp:78-93`
(`ReadUint16Le`/`ReadUint32Le`), and `src/game/io/stream.hpp:35` (`Reader::Get`
returns `uint8_t`). Confirmed by hexdump of the shipped files (below).

### The `SfxSample` struct (`common.hpp:78-120`)

```cpp
struct SfxSample {
  // ...
  std::string name;                    // = types.sounds[i]
  sfx_sound* sound;                    // playback handle (DROPPED in Rust)
  std::vector<uint8_t> original_data;  // decoded 8-bit PCM  <-- LOCKED ARTIFACT
};
```

`original_data` is `std::vector<uint8_t>`. The `sound` handle and the upsampled
`int16` samples it owns are *playback* state.

### The WAV load loop (`common.cpp:325-363`)

For each `sounds[i]` (slot count + name from `tc.cfg`):

1. **Missing file** (`329-336`): if `sounds/<name>.wav` does not exist, emit a
   warning and `continue` — the slot survives (stable indices) with
   `original_data` empty and `sound == nullptr` (a silent no-op). *No error.*
2. **`RIFF` gate** (`340`): read a LE `u32`; if it is not `'RIFF'`
   (`Quad('R','I','F','F')`, `common.cpp:303-306`), do nothing for this slot.
3. **Riff size** (`341-343`): read a LE `u32` (`+ 8`), then **ignore** it.
4. **Header `&&` chain** (`345-349`): the file is accepted only if *every* one of
   these reads matches (short-circuit `&&`):

   | C++ read | Required value | Offset | Meaning |
   |---|---|---|---|
   | `ReadUint32Le` | `'WAVE'` | 8 | format |
   | `ReadUint32Le` | `'fmt '` | 12 | fmt chunk id |
   | `ReadUint32Le` | `16` | 16 | fmt chunk size |
   | `ReadUint16Le` | `1` | 20 | audio format = PCM |
   | `ReadUint16Le` | `1` | 22 | channels = mono |
   | `ReadUint32Le` | `22050` | 24 | sample rate |
   | `ReadUint32Le` | `22050*1*1` | 28 | byte rate |
   | `ReadUint16Le` | `1*1` | 32 | block align |
   | `ReadUint16Le` | `8` | 34 | bits per sample |
   | `ReadUint32Le` | `'data'` | 36 | data chunk id |

5. **Data** (`350-356`): `dataSize = ReadUint32Le` (offset 40);
   `original_data.resize(dataSize)`; then for each byte `z = r.Get() - 128`.
   `Get()` returns `uint8_t`, so the subtraction wraps in `uint8_t`:
   `original_data[i] = (raw - 128) mod 256 ≡ raw ^ 0x80`. This converts the
   file's unsigned-8-bit PCM (centered at 128) to an offset/signed-centered byte
   (centered at 0, stored back in `u8`).
6. **Upsample** (`358-360`): `s.sound = SfxNewSound(dataSize * 2); s.CreateSound();`.

So the header is a **fixed 44-byte canonical WAV** (no chunk-walking: `data`
*must* sit at offset 36). If `RIFF` matches but a later field does not, the slot
is left empty (silent) and load continues; the shipped files all match.

### `CreateSound` — the upsample (`common.cpp:583-602`)

```cpp
void SfxSample::CreateSound() {
  if (original_data.empty()) return;
  std::vector<int16_t>& samples = SfxSoundData(sound);   // mixer/mixer.hpp:25
  samples.clear();
  int prev = (int8_t)original_data[0] * 30;
  samples.push_back(prev);
  for (size_t j = 1; j < original_data.size(); ++j) {
    int cur = (int8_t)original_data[j] * 30;
    samples.push_back((prev + cur) / 2);                 // interpolated tween
    samples.push_back(cur);
    prev = cur;
  }
  samples.push_back(prev);
}
```

A 2× linear-interpolation upsample: each stored byte is reinterpreted as `int8_t`
(undoing the `^ 0x80`), scaled `× 30`, and emitted with an averaged tween sample
between neighbours. Output length is exactly `2 * original_data.len()` (matching
`SfxNewSound(dataSize * 2)`). It is **pure deterministic integer arithmetic** —
no float, no platform/endian dependence (`(int8_t) × 30` ranges
`-3840..+3810`, fits `int16`).

### The shipped files

`data/TC/openliero/sounds/` holds **30 `.wav` files** (+ a `LICENSE` text),
one per `tc.cfg` `types.sounds` entry. Sizes 1.2 KB–55 KB. Every one is the
single accepted shape; sample header hexdumps (offsets 0x00–0x2B = the 44-byte
header, payload from 0x2C):

```
bump.wav   5249 4646 240d 0000 5741 5645 666d 7420   RIFF$...WAVEfmt
           1000 0000 0100 0100 2256 0000 2256 0000   ......PCM mono 22050 22050
           0100 0800 6461 7461 000d 0000 7c7c 7c7b   blkA=1 8bit data sz=0x0d00
moveup.wav 5249 4646 cc0b 0000 5741 5645 666d 7420   (identical header shape)
           1000 0000 0100 0100 2256 0000 2256 0000
           0100 0800 6461 7461 a80b 0000 8088 9199
shot.wav   5249 4646 d704 0000 5741 5645 666d 7420   (identical header shape)
           1000 0000 0100 0100 2256 0000 2256 0000
           0100 0800 6461 7461 b304 0000 3e48 5f72
```

`0x5622` = 22050; `0x10` = fmt size 16; `0x0001` = PCM/mono; `0x0008` = 8-bit.
For `bump.wav`: file = 3372 B = 44 (header) + 0x0d00 (3328, data size) — i.e. the
`data` payload runs to EOF, no trailing chunks. **Confirms a single fixed shape.**

## The `original_data`-vs-`CreateSound` decision

**Decision: 1e-3 reproduces BOTH `original_data` (locked, required) AND the
`CreateSound` upsample (its own golden). Only the `sfx_sound` handle / SDL
playback is deferred to step 3.**

Rationale (the overview, `…-step1e-…overview.md:181-183`, defaults to "lock
`original_data`; defer the upsample *unless it is trivial and deterministic
enough to golden cheaply*" — and `CreateSound` squarely meets that exception):

- **`original_data` is the locked read** regardless: it is the raw decoded PCM,
  the byte-level contract with the file format. Required.
- **`CreateSound` qualifies for the exception.** It is ~15 lines of pure integer
  arithmetic — no float, no platform dependence, fully deterministic — so its
  output is golden-able with *one extra digest line per sound* and ~15 extra
  lines of Rust. Reproducing it now makes 1e-3 a *complete* WAV decode and means
  step 3 (audio backend) never has to reopen `wav.rs`; it just consumes
  `WavSound::upsampled()`.
- **What stays deferred** is the genuinely backend-coupled part: the
  `sfx_sound*` handle, `SfxNewSound`/`SfxSoundData` ownership, and the SDL mixer.
  Those are the C++ in-memory artifacts the charter says to drop.

This is the "don't leave it half-done, don't gold-plate" balance: the upsample is
in-scope because it is cheap and completes the format; the playback engine is
out-of-scope because it is a separate subsystem. (If the controller prefers the
strict-minimum reading of the overview, dropping the upsample is a one-task, one-
golden-line removal — flagged in Open Questions.)

## Rust design

### Crate layout

```
rust/assets/src/
├── io.rs        (1a)
├── level.rs     (1b, 1c)
├── palette.rs   (1c)
├── sprite.rs    (1d)
├── tc.rs        (1e-1)
├── object.rs    (1e-2)
├── wav.rs       ← NEW (1e-3): RIFF/WAVE decode + CreateSound upsample
└── lib.rs       (re-export wav)
```

**No new dependencies.** `wav.rs` is pure `std` (slicing + `from_le_bytes`),
mirroring 1d's hand-rolled TGA reader. `sim-core` is untouched. (`serde`/`toml`
already added by 1e-1 are not used here.)

### `wav.rs`

```rust
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
    /// (≡ `raw ^ 0x80`), exactly as C++ stores it (`common.cpp:354-356`).
    /// This is the LOCKED, golden-verified artifact.
    pub original_data: Vec<u8>,
}

impl WavSound {
    /// Decode an OpenLiero `.wav`. Mirrors the load loop in `Common::load`
    /// (`common.cpp:340-356`): validate the fixed 44-byte RIFF/WAVE header
    /// (PCM, mono, 22050 Hz, 8-bit), then read `dataSize` bytes as `raw - 128`.
    pub fn load(bytes: &[u8]) -> Result<WavSound, WavError>;

    /// The `int16` playback samples C++ `SfxSample::CreateSound`
    /// (`common.cpp:583-602`) produces: a 2× linear-interpolated upsample of
    /// `original_data * 30`. Length = `2 * original_data.len()` (empty when the
    /// sound is empty). Audio-only; SDL playback lives in step 3.
    pub fn upsampled(&self) -> Vec<i16>;
}
```

Idiomatic Rust: fixed-offset slicing + `u16/u32::from_le_bytes`, typed
`Result`/error enum, `wrapping_sub` / `as i8 as i32` for the documented C++
wraps — not a port of the streaming `io::Reader`. The observable values
(`original_data` bytes, upsampled `int16`s) are what the golden pins. Full code
is in the plan.

#### Missing / invalid-file tolerance

C++ tolerates a *missing* file (`continue`, silent slot) and an *existing-but-
non-matching* header (left empty). In Rust this tolerance is the **bundle
assembly's** job (the step-1e integration that maps a `types.sounds` name to
either `WavSound::load(bytes)` on success or `WavSound::default()` on
missing/`Err`). `wav.rs` itself is the strict decoder: present-but-malformed →
typed `Err`. The golden test reproduces the tolerance (missing file →
`WavSound::default()`), but all 30 shipped files decode cleanly, so the silent-
slot path is exercised only by `wav.rs` unit tests.

## The oracle: real `Common::load`

The WAV decode is buried inside `Common::load` (not a callable free function), so
— like 1d — a new dumper runs the genuine public path
`Common::load(FsNode(argv[1]))` (argv[1] = `data/TC/openliero`), then reads the
populated `common.sounds[]`. For each sound it emits:

- `original_data` length + FNV-1a digest of the raw `original_data` bytes (the
  locked read).
- the upsample length + FNV-1a digest of the `int16` samples, read back from the
  engine's own `SfxSoundData(s.sound)` (populated by the `CreateSound()` that
  `Common::load` already called at `common.cpp:360`) — i.e. the **true** engine
  output, not a re-derivation. Silent slots (`s.sound == nullptr`) hash the empty
  buffer, matching `WavSound::default().upsampled()`.

New dumper `src/tools/oracle_dump/wav_dump.cpp` links `game` and
`mixer/mixer.hpp` (for `SfxSoundData`, `mixer.hpp:25`). New CMake target
`oracle_dump_wav` under the existing `OPENLIERO_BUILD_ORACLE_DUMP` option, beside
`oracle_dump_sprite`/`oracle_dump_tc`.

### Golden format (`golden/wav.txt`)

One line per sound, **labeled by name** (so the Rust side can look up by name and
walk `tc.cfg`'s order independently — same HashMap pattern as `tc_golden.rs`):

```
<name> <orig_len> <orig_hash> <up_len> <up_hash>
```

e.g.

```
bump 3328 <hash> 6656 <hash>
moveup 2984 <hash> 5968 <hash>
...
```

30 lines. `<orig_hash>` = FNV-1a (64-bit, seed `0xcbf29ce484222325`, prime
`0x100000001b3`) over the raw `original_data` bytes (a byte buffer, hashed
directly per the charter). `<up_hash>` = FNV-1a over the `int16` samples as
explicit little-endian byte pairs. Hex digests are 16 chars, identical helper to
1c/1d/1e-1.

### Oracle input

The 30 real shipped files, exercised through the full loader:
`data/TC/openliero/sounds/*.wav`, ordered/named by `tc.cfg` `types.sounds`. No
synthetic inputs — the format is fixed and the real files cover the decode +
upsample paths end to end. Error paths (bad header field, truncation) and the
silent-slot default are covered by `wav.rs` unit tests on small synthetic buffers.

## Testing

1. **Smoke test first** (TDD risk-buster): `WavSound::load` on the real
   `bump.wav` must succeed and yield `original_data.len() == file_len - 44`
   (proves the fixed-header decode against a real file before any error-path
   work).
2. **Unit tests** in `wav.rs`: a hand-built minimal valid WAV round-trips
   (`raw - 128` / `^ 0x80` decode at the right offset); each rejected header
   field → `BadHeader`; short header / short payload → `Truncated`; zero-length
   `data` → empty `original_data`, `Ok`; the `CreateSound` upsample math on a
   tiny known input (length `2n`, tween = `(prev+cur)/2`, `int8 × 30`); empty
   sound → empty upsample.
3. **Golden differential test** `oracle-tests/tests/wav_golden.rs`: walk
   `tc.cfg`'s `types.sounds`, decode each real `.wav`, and reproduce every
   `orig_len`/`orig_hash`/`up_len`/`up_hash`. Regenerated by `gen_wav_golden.sh`
   against the real C++ build (local/manual, like 1b–1e-1).
4. CI (`rust.yml`) runs `cargo test --workspace` against the committed golden; it
   does not rebuild the C++ oracle.

**Done when:** the full Rust workspace suite is green and every 1e-3 golden digest
matches C++ bit-for-bit.

## Modernization-charter check

- **Locked / bit-exact (read the bytes the same):** every sound's
  `original_data` (the decoded 8-bit PCM) reproduces C++ exactly, golden-proven
  against the real `Common::load`. The `CreateSound` upsample is also reproduced
  and golden-pinned (the trivial/deterministic exception).
- **Free to modernize:** a `WavSound` value type + typed `Result`/error enum +
  fixed-offset slicing replace C++'s streaming `io::Reader` and the
  `resize`+`Get()` loop. `upsampled()` is a pure function instead of mutating a
  borrowed `SfxSoundData` vector.
- **Dropped C++ artifacts:** `sfx_sound*` / `SfxNewSound` / `SfxFreeSound` /
  `SfxSoundData` ownership and the SDL mixer — step-3 audio backend.
- **Sim impact:** none. Sounds are audio; no `processFrame` logic reads sample
  data. Labeled non-sim so step 2 may not depend on it.

## Open questions for the controller

1. **`CreateSound` scope (RESOLVED → reproduce here).** This design reproduces
   the upsample now (cheap, trivial, deterministic; completes the format and
   spares step 3 from reopening `wav.rs`). If you prefer the overview's strict-
   minimum ("lock `original_data` only, defer the upsample"), it is a one-task,
   one-golden-line removal. Recommendation: keep it (as specced).
2. **`WavSound` granularity.** 1e-3 ships a per-file decoder (`WavSound`); the
   name + missing/invalid tolerance that assembles a `Vec<Sound>` is left to the
   step-1e integration (consistent with 1d, which shipped `Tga`/`SpriteSet`
   value types, not a bundle). Confirm you want the bundle assembly handled at
   integration time rather than inside 1e-3.
3. **Reading the upsample from `SfxSoundData` vs re-deriving it in the dumper.**
   The dumper reads the *actual* `CreateSound` output via `SfxSoundData(s.sound)`
   (true engine output, no logic duplication) — this assumes the headless oracle
   build links a working `SfxNewSound`/`SfxSoundData` (it must, since
   `Common::load` already calls them and the 1d sprite dumper runs the full load
   fine). If that proves not to hold in the oracle build, the fallback is to
   re-derive the upsample in the dumper with code identical to `CreateSound`
   (mild duplication). Recommendation: read `SfxSoundData`.

## Next concrete artifact

Implementation plan:
`docs/superpowers/plans/2026-06-26-liero-rs-step1e3-wav-sounds.md`.
