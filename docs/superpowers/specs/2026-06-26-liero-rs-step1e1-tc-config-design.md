# Step 1, sub-slice 1e-1 — tc.cfg (TOML config): design

Status: **draft for review** · 2026-06-26
Part of: `2026-06-26-liero-rs-step1e-tc-bundle-overview.md`
Follows: `2026-06-26-liero-rs-step1d-sprites-design.md`

Sub-slice 1e-1 of the Liero-rs rewrite: read OpenLiero's top-level **`tc.cfg`**
(TOML) in Rust, reproducing the C++ engine's parsed values. This adds a `tc`
module to the `assets` crate and introduces the project's first **TOML
dependency** (`toml` + `serde`). As always: the C++ engine is the oracle; a C++
dumper runs the real `Common::load` and a golden differential test pins the
parsed values.

## Scope

In scope:

- **Parse `tc.cfg` into a typed `TcConfig`** mirroring `LoadTcConfig`
  (`src/game/common_model.hpp:565–681`):
  - `[types]` — `sounds` / `weapons` / `nobjects` / `sobjects` name lists
    (**SIM**: they fix object identity and index).
  - `[constants]` scalars — the 72 `LIERO_CDEFS` integers (**SIM**).
  - `[constants].materials` — up to 256 ints, stored as `u8` flags
    (`flags = value & 0xff`, `common_model.hpp:621–623`) (**SIM**).
  - `[constants].textures` — up to 9 `{mframe, rframe, sframe, ndrawback}`
    (**SIM**).
  - `[constants].bonuses` — up to 2 `{timer, timerV, frame, sobj}`, with `sobj`
    resolved to a sobject index (**SIM**).
  - `[constants].colorAnim` — up to 4 `{from, to}` (rendering, still parsed).
  - `[constants].aiparams` — 7 keys (up/down/left/right/fire/change/jump), each
    `{on, off}` (**SIM**).
  - `[texts]` — the 40 `LIERO_SDEFS` strings (rendering/UI, still parsed).
  - `[hacks]` — the 11 `LIERO_HDEFS` booleans (**SIM**).
  - `[sounds]` — the 8 `LIERO_SOUNDDEFS` hook names, **resolved to indices** with
    the canonical-default fallback (`common_model.hpp:654–680`) (audio).
- **Cross-reference resolution that tc.cfg performs itself** (self-contained
  because it only needs lists that live *inside* tc.cfg):
  - `bonuses[i].sobj` → index via `ObjRefFromStr(sobj, sobjects)`
    (empty→`-1`, unknown→`0`, else position).
  - sound hooks → index via the `resolveHook` lambda: a configured non-empty
    name resolves via `SoundIndex` (`-1` if unknown); an empty name falls back to
    a per-hook canonical default name, also via `SoundIndex`.

Explicitly out of scope (deferred):

- Per-object `.cfg` parsing and *its* cross-refs (`splinterType`, `createOnExp`,
  …) — **1e-2** (it consumes 1e-1's `types` lists).
- WAV decode — **1e-3**.
- `Precompute()` (`weap_order` sort, derived sprites) — rendering / trivial.
- Writing config back out (`SaveTcConfig`).
- Reproducing toml++/cereal *type-mismatch leniency* on malformed input (a key
  present with the wrong type is silently ignored in C++; `serde` errors). The
  locked contract is "read the *real shipped* assets identically"; the shipped
  `tc.cfg` is well-formed, so the golden is unaffected. See "Charter check".

## Background: the C++ format (oracle truth)

`LoadTcConfig` (`common_model.hpp:565–681`) deserializes five top-level structs
via `cereal::TomlInputArchive` (`serialization/toml_archive.hpp`) and then
copies them into `Common`:

### `[types]` → vectors (`common_model.hpp:580–599`)

```
common.sounds.resize(types.sounds.size());        sounds[i].name      = types.sounds[i];
common.weapons.resize(types.weapons.size());       weapons[i].id_str   = types.weapons[i];
common.nobject_types.resize(types.nobjects.size()); nobject_types[i].id_str = types.nobjects[i];
common.sobject_types.resize(types.sobjects.size()); sobject_types[i].id_str = types.sobjects[i];
```

These are the canonical index spaces. In the shipped TC: 30 sounds, 40 weapons,
24 nobjects, 14 sobjects.

### `[constants]` scalars (`common_model.hpp:637–639`)

`common.c[Cn] = constants.n` for each `n` in `LIERO_CDEFS` (72 ints,
`constants.hpp:15–87`), stored in `c[ConstDefT::kMaxC]` (`common.hpp:189`). The
TOML keys are the PascalCase names verbatim (`WormGravity`, `JumpForce`, …).

### `[constants].materials` (`common_model.hpp:621–623`)

```
for i in 0 .. min(materials.size(), MAX_MATERIALS=256):
    common.materials[i].flags = uint8_t(materials[i] & 0xff);
```

The TOML is `materials = [0, 9, 10, 0, …]` (256 entries in the shipped file).
Entries beyond 256 are ignored. The shipped file has exactly 256, so every slot
of the public fixed `[u8; 256]` is written; for a shorter list the Rust side
leaves the remaining slots zero-filled (the shipped file never triggers this).

### `[constants].textures` (`common_model.hpp:609–614`, cap `NUM_TEXTURES=9`)

```
textures[i].m_frame     = te.mframe;
textures[i].r_frame     = te.rframe;
textures[i].s_frame     = te.sframe;
textures[i].n_draw_back = te.ndrawback;
```

TOML array-of-tables `[[constants.textures]]` with keys `mframe`/`rframe`/
`sframe`/`ndrawback` (`common_model.hpp:369–381`).

### `[constants].bonuses` (`common_model.hpp:602–607`, cap `NUM_BONUS_SOBJECTS=2`)

```
bonus_rand_timer[i][0] = be.timer;
bonus_rand_timer[i][1] = be.timer_v;     // TOML key "timerV"
bonus_frames[i]        = be.frame;
bonus_s_objects[i]     = ObjRefFromStr(be.sobj, sobject_types);
```

### `[constants].colorAnim` (`common_model.hpp:616–619`, cap `NUM_COLOR_ANIM=4`)

```
color_anim[i].from = ce.from;  color_anim[i].to = ce.to;
```

### `[constants].aiparams` (`common_model.hpp:625–635`)

Seven keys read in the fixed order **up, down, left, right, fire, change, jump**;
each `{on, off}` table maps to `ai_params.k[1][idx] = on; k[0][idx] = off`.

### `[texts]` (`common_model.hpp:641–643`)

`common.s[Sn] = texts.n` for each `n` in `LIERO_SDEFS` (40 strings). Note some
contain embedded `\u0000` and non-ASCII (`Copyright`, `Copyright2`,
`NoWeaps`) — these are TOML basic-string escapes decoding to UTF-8 bytes.

### `[hacks]` (`common_model.hpp:645–647`)

`common.h[Hn] = hacks.n` for each `n` in `LIERO_HDEFS` (11 bools).

### `[sounds]` hooks (`common_model.hpp:654–680`)

```
resolveHook(hook, configured, default_name):
    if configured non-empty:
        idx = SoundIndex(configured)         // -1 if unknown (warn)
        sound_hook[hook] = idx
    else:
        sound_hook[hook] = SoundIndex(default_name)
```

Per-hook canonical defaults (`common_model.hpp:671–678`):
`MenuMoveUp="moveup"`, `MenuMoveDown="movedown"`, `MenuSelect="select"`,
`Bump="bump"`, `Begin="begin"`, `Reloaded="reloaded"`, `Alive="alive"`,
`NinjaropeThrow="throw"`. `SoundIndex` (`common.cpp:574–581`) scans
`sounds[].name`, returns `-1` if absent.

### Defaults on missing keys

`TomlInputArchive::loadValue` only assigns when the key exists and has the right
type (`toml_archive.hpp:217–268`); otherwise the field keeps its C++ struct
default (`int 0`, `bool false`, `string ""`). 1e-1 reproduces this with
`#[serde(default)]` (missing key → Rust `Default`).

## Rust design

### Crate layout

```
rust/assets/src/
├── …            (1a–1d, unchanged)
├── tc.rs        ← NEW (1e-1): TcConfig + tc.cfg parser
└── lib.rs       (re-export tc)
```

`rust/assets/Cargo.toml` gains:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
toml  = "0.8"
```

### `tc.rs` types

A typed tree deserialized with `serde`. Field names of the macro-generated blocks
(`Constants`, `Texts`, `Hacks`) match the TOML keys **verbatim** (PascalCase,
under `#![allow(non_snake_case)]`) — the TC schema is the locked contract, so
mirroring its key names 1:1 is clearer and safer than 130 `#[serde(rename)]`
attributes. Hand-authored nested structs use idiomatic snake_case + `rename`
where the TOML key differs.

```rust
pub struct TcConfig {
    pub types: TcTypes,
    pub constants: Constants,   // scalars (72)
    pub materials: [u8; 256],   // 256 flags (value & 0xff); tail zero-filled
    pub textures: Vec<Texture>, // <= 9
    pub bonuses: Vec<Bonus>,    // <= 2, sobj resolved to index
    pub color_anim: Vec<ColorAnim>, // <= 4
    pub aiparams: AiParams,
    pub texts: Texts,           // 40 strings
    pub hacks: Hacks,           // 11 bools
    pub sound_hooks: SoundHooks, // 8 resolved indices (i32, -1 = none)
}

pub struct TcTypes { pub sounds: Vec<String>, pub weapons: Vec<String>,
                     pub nobjects: Vec<String>, pub sobjects: Vec<String> }

pub struct Texture { pub mframe: i32, pub rframe: i32, pub sframe: i32,
                     pub ndrawback: bool }
pub struct Bonus { pub timer: i32, pub timer_v: i32, pub frame: i32,
                   pub sobj: i32 }   // resolved index (ObjRefFromStr semantics)
pub struct ColorAnim { pub from: i32, pub to: i32 }
pub struct AiKey { pub on: i32, pub off: i32 }
pub struct AiParams { pub up: AiKey, pub down: AiKey, pub left: AiKey,
                      pub right: AiKey, pub fire: AiKey, pub change: AiKey,
                      pub jump: AiKey }

// `Constants` / `Texts` / `Hacks` / a `SoundHooks` index struct: see the plan
// for the full field lists (72 / 40 / 11 / 8 entries in CDEFS/SDEFS/HDEFS/
// SOUNDDEFS order).

pub enum TcError { Parse(String) }

impl TcConfig {
    /// Parse a `tc.cfg` byte buffer. Mirrors `LoadTcConfig`
    /// (`common_model.hpp:565`): deserialize the five sections, cap the
    /// fixed-size arrays, store material values as `& 0xff` flags, and resolve
    /// bonus-sobj and sound-hook names to indices.
    pub fn load(bytes: &[u8]) -> Result<TcConfig, TcError>;
}
```

The deserialization shape is split in two layers: a private
`#[derive(Deserialize)]` "raw" mirror of the TOML (with `materials: Vec<i32>`,
`bonuses[].sobj: String`, `sounds: SoundsRaw { …: String }`), then `load()`
post-processes it into the public `TcConfig` (cap the `Vec` arrays, fold the raw
`materials: Vec<i32>` into the fixed `[u8; 256]` with `& 0xff`, resolve indices).
This keeps `serde` doing the parsing and `load()` doing the C++
copy-and-resolve step (the analogue of `LoadTcConfig`'s second half).

### Cross-ref resolution helpers (mirror `common_model.hpp:16–52`)

```rust
fn obj_ref_from_str(s: &str, list: &[String]) -> i32 {  // ObjRefFromStr
    if s.is_empty() { return -1; }
    for (i, n) in list.iter().enumerate() { if n == s { return i as i32; } }
    0
}
fn sound_index(name: &str, sounds: &[String]) -> i32 {  // SoundIndex (-1 if absent)
    sounds.iter().position(|n| n == name).map_or(-1, |i| i as i32)
}
```

Idiomatic Rust: `serde`/`toml` instead of cereal; typed structs; `Result`. Not a
port of `TomlInputArchive`.

## The oracle: real `Common::load`

A new dumper (`src/tools/oracle_dump/tc_config_dump.cpp`, links `game`) runs
`Common::load(FsNode(argv[1]))` (argv[1] = `data/TC/openliero`) and emits FNV-1a
digests of the tc.cfg-derived `Common` fields, **using the engine's own
`LIERO_*DEFS` macros** to guarantee field order matches the engine. New CMake
target `oracle_dump_tc` under `OPENLIERO_BUILD_ORACLE_DUMP`.

### Golden format (`golden/tc.txt`)

One `label hash` line per group (16-hex FNV-1a over explicit little-endian bytes,
same helper as 1c/1d):

```
types_sounds   <hash>   # count(u32) then each name (len u32 + bytes)
types_weapons  <hash>
types_nobjects <hash>
types_sobjects <hash>
constants      <hash>   # 72 × i32 LE in LIERO_CDEFS order
materials      <hash>   # 256 × u8 flags (fixed [u8; 256], tail zero-filled)
textures       <hash>   # 9 × (mframe,rframe,sframe i32 LE; ndrawback 1 byte)
bonuses        <hash>   # 2 × (timer,timerV,frame i32 LE; resolved sobj i32 LE)
coloranim      <hash>   # 4 × (from,to i32 LE)
aiparams       <hash>   # 7 keys × (on,off i32 LE), up..jump
texts          <hash>   # 40 × (len u32 + UTF-8 bytes) in LIERO_SDEFS order
hacks          <hash>   # 11 × 1 byte (0/1) in LIERO_HDEFS order
soundhooks     <hash>   # 8 × i32 LE resolved index in LIERO_SOUNDDEFS order
```

String hashing uses a `u32` LE length prefix + raw bytes so embedded NUL and
multi-byte UTF-8 are unambiguous and host-endian independent.

### Oracle input

The single real shipped file, exercised through the full loader:
`data/TC/openliero/tc.cfg`. No synthetic inputs — the format is fixed and the
real file covers every section, the array caps (256 materials, 9 textures, 2
bonuses, 4 colorAnim), the embedded-NUL/UTF-8 text path, and both bonus-sobj and
sound-hook resolution. Error paths (malformed TOML) are covered by `tc.rs` unit
tests on small synthetic buffers.

## Testing

1. **Smoke test first** (TDD risk-buster): `toml::from_str` (or
   `toml::from_slice`) on the *real* `tc.cfg` must succeed — proves the Rust
   `toml` crate accepts the file's `\u0000`/UTF-8 escapes before any struct work.
   (If it rejects `\u0000`, that is a BLOCKED signal; see open questions.)
2. **Unit tests** in `tc.rs`: a small inline TOML round-trips (one of each
   section); `& 0xff` material truncation; array caps; bonus-sobj resolution
   (empty→-1, unknown→0, hit→index); sound-hook fallback (empty→default name,
   configured-unknown→-1); malformed TOML → `TcError::Parse`.
3. **Golden differential test** `oracle-tests/tests/tc_golden.rs`: parse the real
   `tc.cfg` and reproduce every digest above. Regenerated by `gen_tc_golden.sh`
   against the real C++ build (local/manual, like 1b–1d).
4. CI (`rust.yml`) runs `cargo test --workspace` against the committed golden; it
   does not rebuild the C++ oracle.

**Done when:** the full Rust workspace suite is green and every 1e-1 golden digest
matches C++ bit-for-bit.

## Modernization-charter check

- **Locked / bit-exact:** all sim-affecting values — the 72 constants, material
  flags, textures, bonuses (incl. resolved sobj index), AI params, hacks — plus
  the types index spaces, reproduce C++ exactly (golden-proven against real
  `Common::load`).
- **Free to modernize:** `serde`/`toml` deserialization replaces the cereal
  `TomlInputArchive`; typed structs + `Result` replace the archive's
  prologue/epilogue + macro field copy. The raw→public two-layer split is ours.
- **Rendering/audio parsed but labeled:** `[texts]`, `colorAnim`, and the
  `[sounds]` hooks are read and golden-verified but flagged non-sim.
- **Accepted divergence:** C++'s *silent* tolerance of wrong-typed keys on
  malformed input is not reproduced (`serde` errors). The locked contract is the
  real shipped asset, which is well-formed; the golden uses it. Documented, not a
  bit-exactness regression on valid data.

## Open questions for the controller

1. **`toml` crate vs `\u0000`:** the shipped `tc.cfg` has `\u0000` escapes in
   `Copyright`/`Copyright2`/`NoWeaps`. The plan's Task 0 smoke-test verifies the
   `toml` crate accepts them. If it does not, options are (a) pin/patch the
   parser, (b) pre-process those escapes, or (c) deserialize texts as raw and
   defer exact text bytes to a rendering slice (texts are non-sim). Prefer (a)/(b)
   to keep the golden whole.
2. **Constants representation:** named struct fields (this design) vs a
   `[i32; 72]` array indexed by a `Const` enum mirroring C++'s `c[]`. Named
   fields are more idiomatic and what step 2 will read by name; the array is a
   closer C++ analogue. Recommendation: named fields.
3. **`materials` length (RESOLVED → fixed `[u8; 256]`):** step 2 indexes
   `common.materials[material_id[i]]` with an arbitrary `u8` palette index, so the
   public field is a fixed `[u8; 256]` (mirrors C++ `Material materials[256]`,
   guarantees in-bounds, removes the golden's `resize`). The raw TOML still
   deserializes as a `Vec<i32>`; the raw→public step folds up to 256 entries in
   with `& 0xff` and zero-fills any unwritten tail. Note this is *not* a C++
   default: `Common::Common() = default;` (`common.cpp:558`) leaves `materials[]`
   indeterminate (only `sound_hook[]` is initialized, `common.hpp:193`). The
   golden is well-defined anyway because the shipped `tc.cfg` fills every fixed
   slot (256 materials, 9 textures, 2 bonuses, 4 colorAnim — verified); the Rust
   zero-fill only matters for inputs the shipped file never produces.

## Next concrete artifact

Implementation plan:
`docs/superpowers/plans/2026-06-26-liero-rs-step1e1-tc-config.md`.
