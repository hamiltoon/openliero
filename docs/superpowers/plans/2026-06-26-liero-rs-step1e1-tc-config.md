# Step 1e-1 — tc.cfg (TOML config) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read OpenLiero's `tc.cfg` (TOML) in Rust, reproducing the C++ engine's parsed values, by adding a `tc` module to the `assets` crate and introducing the project's first TOML dependency.

**Architecture:** `tc.rs` deserializes `tc.cfg` with `serde`/`toml` into a typed `TcConfig` (types lists, 72 constants, material flags, textures, bonuses, colour-anim, AI params, texts, hacks, resolved sound hooks). Cross-references that live inside tc.cfg (bonus `sobj`, sound hooks) are resolved to indices with the same semantics as C++. Correctness is proven by a C++ oracle dumper that runs the real `Common::load` and a golden differential test that reproduces its FNV-1a digests.

**Tech Stack:** Rust (`assets`, `oracle-tests`), `serde` + `toml` (new deps in `assets`), C++ oracle dumper (`oracle_dump_tc`, links `game`), CMake option `OPENLIERO_BUILD_ORACLE_DUMP`, FNV-1a digests.

## Global Constraints

- **Bit-exact vs C++ for sim-affecting values.** Source of truth: `src/game/common_model.hpp:565–681` (`LoadTcConfig`), `src/game/constants.hpp:15–155` (`LIERO_CDEFS/SDEFS/HDEFS/SOUNDDEFS`), `src/game/common.hpp` (field storage & caps). The 72 constants, 256 material flags, 9 textures, 2 bonuses (incl. resolved sobj index), 7×2 AI params, and 11 hacks must reproduce C++ exactly. Texts (40), colour-anim (4) and the 8 sound-hook indices are parsed and golden-verified too (rendering/audio, but "read the bytes the same").
- **Caps & defaults (match C++):** materials → first `MAX_MATERIALS=256`, each `flags = value & 0xff`; textures → first `NUM_TEXTURES=9`; bonuses → first `NUM_BONUS_SOBJECTS=2`; colorAnim → first `NUM_COLOR_ANIM=4`. Missing keys keep the C++ struct default (int 0, bool false, string "") — reproduced with `#[serde(default)]`.
- **Cross-ref semantics (sim state):** `obj_ref_from_str` = `ObjRefFromStr` (`common_model.hpp:24–35`): empty→`-1`, unknown→`0`, else index. Sound hooks (`common_model.hpp:654–680`): configured non-empty → `SoundIndex` (`-1` if unknown); empty → `SoundIndex(default_name)`. `SoundIndex` (`common.cpp:574–581`) → `-1` if absent.
- **Idiomatic Rust, not a port:** `serde`/`toml` deserialization, typed `Result`/structs — NOT a port of `serialization/toml_archive.hpp` (cereal `TomlInputArchive` + prologue/epilogue). Field names of the macro blocks mirror the TOML keys verbatim (`#![allow(non_snake_case)]`); the TC schema is the contract.
- **FNV-1a (64-bit)** seed `0xcbf29ce484222325`, prime `0x100000001b3` — identical helper on both sides (see `rust/oracle-tests/tests/level_golden.rs`). Multi-byte fields are hashed as explicit little-endian bytes (`PushI32`/`PushU32` with shifts); strings as a `u32` LE length prefix + raw bytes (handles embedded NUL + UTF-8, host-endian independent).
- **Field order in digests = engine macro order.** The C++ dumper iterates via `LIERO_CDEFS/SDEFS/HDEFS/SOUNDDEFS`; the Rust `ordered()` helpers list fields in the same order. Drift is caught by the golden.
- **No Bevy** in `assets`. **Golden regeneration is LOCAL/MANUAL** (full C++ build links `game`); CI (`rust.yml`) runs `cargo test --workspace` against the committed golden. PRESET defaults to `macos-arm64`.
- **No AI/"Generated with" taglines** in commits. C++ matches the existing `level_dump.cpp` Google/100-col style.

## File Structure

- `rust/assets/Cargo.toml` — MODIFY: add `serde` (derive) + `toml` deps.
- `rust/assets/src/tc.rs` — NEW: `TcConfig` + all sub-structs + `load` + `ordered()` helpers + unit tests.
- `rust/assets/src/lib.rs` — MODIFY: `pub mod tc;`.
- `src/tools/oracle_dump/tc_config_dump.cpp` — NEW: runs real `Common::load`, dumps tc.cfg-derived digests via the engine macros.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_tc` target inside the existing `OPENLIERO_BUILD_ORACLE_DUMP` block (after `oracle_dump_palette`).
- `rust/oracle-tests/gen_tc_golden.sh` — NEW: regenerate the tc golden.
- `rust/oracle-tests/golden/tc.txt` — NEW: committed golden.
- `rust/oracle-tests/tests/tc_golden.rs` — NEW: tc differential test.

---

### Task 0: Add TOML deps and smoke-test the real file

De-risk the `toml` crate against `tc.cfg`'s `\u0000` / UTF-8 escapes **before** building any structs.

**Files:**
- Modify: `rust/assets/Cargo.toml`
- Create (stub): `rust/assets/src/tc.rs`
- Modify: `rust/assets/src/lib.rs`

- [ ] **Step 1: Add dependencies**

Replace the `[dependencies]` section of `rust/assets/Cargo.toml` with:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

- [ ] **Step 2: Stub `tc.rs` with only a smoke test**

Create `rust/assets/src/tc.rs`:

```rust
//! tc.cfg (TOML) loading. Reproduces the values C++ `LoadTcConfig`
//! (`src/game/common_model.hpp:565`) parses; idiomatic Rust via `serde`/`toml`,
//! not a port of the cereal `TomlInputArchive`.

#[cfg(test)]
mod smoke {
    /// The real shipped tc.cfg must parse with the `toml` crate. This guards
    /// the `\u0000`/UTF-8 escapes in Copyright/NoWeaps; failure here is a
    /// BLOCKED signal (see the design's open questions), not a code bug.
    #[test]
    fn real_tc_cfg_parses() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/tc.cfg"
        ));
        let text = std::str::from_utf8(bytes).expect("tc.cfg is UTF-8");
        let doc: toml::Table = toml::from_str(text).expect("toml crate parses tc.cfg");
        assert!(doc.contains_key("types"));
        assert!(doc.contains_key("constants"));
        assert!(doc.contains_key("texts"));
    }
}
```

Add to `rust/assets/src/lib.rs` after the `pub mod sprite;` line:

```rust
pub mod tc;
```

- [ ] **Step 3: Run the smoke test**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets tc::smoke`
Expected: PASS. If it FAILS to parse `\u0000`, STOP and report BLOCKED (the design lists fallback options for the controller).

- [ ] **Step 4: Commit**

```bash
git add rust/assets/Cargo.toml rust/assets/src/tc.rs rust/assets/src/lib.rs
git commit -m "build(assets): add serde/toml deps; smoke-test tc.cfg parse"
```

---

### Task 1: `tc.rs` — TcConfig types, loader, and unit tests

**Files:**
- Modify: `rust/assets/src/tc.rs` (replace the stub)
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing new (uses `serde`/`toml`).
- Produces (all `pub`):
  - `struct TcConfig { types, constants, materials: [u8; 256], textures: Vec<Texture>, bonuses: Vec<Bonus>, color_anim: Vec<ColorAnim>, aiparams: AiParams, texts, hacks, sound_hooks: SoundHooks }`
  - `struct TcTypes { sounds, weapons, nobjects, sobjects: Vec<String> }`
  - `struct Constants { /* 72 i32 fields */ }` + `Constants::ordered(&self) -> [i32; 72]`
  - `struct Texture { mframe, rframe, sframe: i32, ndrawback: bool }`
  - `struct Bonus { timer, timer_v, frame, sobj: i32 }`
  - `struct ColorAnim { from, to: i32 }`
  - `struct AiKey { on, off: i32 }`, `struct AiParams { up, down, left, right, fire, change, jump: AiKey }`
  - `struct Texts { /* 40 String fields */ }` + `Texts::ordered(&self) -> [&str; 40]`
  - `struct Hacks { /* 11 bool fields */ }` + `Hacks::ordered(&self) -> [bool; 11]`
  - `struct SoundHooks { /* 8 i32 fields */ }` + `SoundHooks::ordered(&self) -> [i32; 8]`
  - `enum TcError { Parse(String) }`
  - `TcConfig::load(bytes: &[u8]) -> Result<TcConfig, TcError>`

- [ ] **Step 1: Write `tc.rs` (replace the stub)**

```rust
//! tc.cfg (TOML) loading. Reproduces the values C++ `LoadTcConfig`
//! (`src/game/common_model.hpp:565`) parses; idiomatic Rust via `serde`/`toml`,
//! not a port of the cereal `TomlInputArchive`. Field names of the macro-block
//! structs mirror the TOML keys verbatim (the TC schema is the contract).
#![allow(non_snake_case)]

use serde::Deserialize;

// Engine caps (src/game/common.hpp).
const MAX_MATERIALS: usize = 256;
const NUM_TEXTURES: usize = 9;
const NUM_BONUS_SOBJECTS: usize = 2;
const NUM_COLOR_ANIM: usize = 4;

/// Why a tc.cfg failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum TcError {
    /// TOML parse / shape error (message from the `toml` crate or UTF-8 check).
    Parse(String),
}

// ----- [types] -----
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct TcTypes {
    pub sounds: Vec<String>,
    pub weapons: Vec<String>,
    pub nobjects: Vec<String>,
    pub sobjects: Vec<String>,
}

// ----- [constants] scalars (LIERO_CDEFS order) -----
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Constants {
    pub NRInitialLength: i32,
    pub NRAttachLength: i32,
    pub MinBounceUp: i32,
    pub MinBounceDown: i32,
    pub MinBounceLeft: i32,
    pub MinBounceRight: i32,
    pub WormGravity: i32,
    pub WalkVelLeft: i32,
    pub MaxVelLeft: i32,
    pub WalkVelRight: i32,
    pub MaxVelRight: i32,
    pub JumpForce: i32,
    pub MaxAimVelLeft: i32,
    pub AimAccLeft: i32,
    pub MaxAimVelRight: i32,
    pub AimAccRight: i32,
    pub NinjaropeGravity: i32,
    pub NRMinLength: i32,
    pub NRMaxLength: i32,
    pub BonusGravity: i32,
    pub WormFricMult: i32,
    pub WormFricDiv: i32,
    pub WormMinSpawnDistLast: i32,
    pub WormMinSpawnDistEnemy: i32,
    pub WormSpawnRectX: i32,
    pub WormSpawnRectY: i32,
    pub WormSpawnRectW: i32,
    pub WormSpawnRectH: i32,
    pub AimFricMult: i32,
    pub AimFricDiv: i32,
    pub NRThrowVelX: i32,
    pub NRThrowVelY: i32,
    pub NRForceShlX: i32,
    pub NRForceDivX: i32,
    pub NRForceShlY: i32,
    pub NRForceDivY: i32,
    pub NRForceLenShl: i32,
    pub BonusBounceMul: i32,
    pub BonusBounceDiv: i32,
    pub BonusFlickerTime: i32,
    pub AimMaxRight: i32,
    pub AimMinRight: i32,
    pub AimMaxLeft: i32,
    pub AimMinLeft: i32,
    pub NRPullVel: i32,
    pub NRReleaseVel: i32,
    pub NRColourBegin: i32,
    pub NRColourEnd: i32,
    pub BonusExplodeRisk: i32,
    pub BonusHealthVar: i32,
    pub BonusMinHealth: i32,
    pub LaserWeapon: i32,
    pub FirstBloodColour: i32,
    pub NumBloodColours: i32,
    pub BObjGravity: i32,
    pub BonusDropChance: i32,
    pub SplinterLarpaVelDiv: i32,
    pub SplinterCracklerVelDiv: i32,
    pub BloodStepUp: i32,
    pub BloodStepDown: i32,
    pub BloodLimit: i32,
    pub FallDamageRight: i32,
    pub FallDamageLeft: i32,
    pub FallDamageDown: i32,
    pub FallDamageUp: i32,
    pub WormFloatLevel: i32,
    pub WormFloatPower: i32,
    pub BonusSpawnRectX: i32,
    pub BonusSpawnRectY: i32,
    pub BonusSpawnRectW: i32,
    pub BonusSpawnRectH: i32,
    pub RemExpObject: i32,
}

impl Constants {
    /// The 72 values in `LIERO_CDEFS` order (single source of digest order).
    pub fn ordered(&self) -> [i32; 72] {
        [
            self.NRInitialLength, self.NRAttachLength, self.MinBounceUp,
            self.MinBounceDown, self.MinBounceLeft, self.MinBounceRight,
            self.WormGravity, self.WalkVelLeft, self.MaxVelLeft,
            self.WalkVelRight, self.MaxVelRight, self.JumpForce,
            self.MaxAimVelLeft, self.AimAccLeft, self.MaxAimVelRight,
            self.AimAccRight, self.NinjaropeGravity, self.NRMinLength,
            self.NRMaxLength, self.BonusGravity, self.WormFricMult,
            self.WormFricDiv, self.WormMinSpawnDistLast, self.WormMinSpawnDistEnemy,
            self.WormSpawnRectX, self.WormSpawnRectY, self.WormSpawnRectW,
            self.WormSpawnRectH, self.AimFricMult, self.AimFricDiv,
            self.NRThrowVelX, self.NRThrowVelY, self.NRForceShlX,
            self.NRForceDivX, self.NRForceShlY, self.NRForceDivY,
            self.NRForceLenShl, self.BonusBounceMul, self.BonusBounceDiv,
            self.BonusFlickerTime, self.AimMaxRight, self.AimMinRight,
            self.AimMaxLeft, self.AimMinLeft, self.NRPullVel,
            self.NRReleaseVel, self.NRColourBegin, self.NRColourEnd,
            self.BonusExplodeRisk, self.BonusHealthVar, self.BonusMinHealth,
            self.LaserWeapon, self.FirstBloodColour, self.NumBloodColours,
            self.BObjGravity, self.BonusDropChance, self.SplinterLarpaVelDiv,
            self.SplinterCracklerVelDiv, self.BloodStepUp, self.BloodStepDown,
            self.BloodLimit, self.FallDamageRight, self.FallDamageLeft,
            self.FallDamageDown, self.FallDamageUp, self.WormFloatLevel,
            self.WormFloatPower, self.BonusSpawnRectX, self.BonusSpawnRectY,
            self.BonusSpawnRectW, self.BonusSpawnRectH, self.RemExpObject,
        ]
    }
}

// ----- [[constants.textures]] / [[constants.bonuses]] / [[constants.colorAnim]] -----
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Texture {
    pub mframe: i32,
    pub rframe: i32,
    pub sframe: i32,
    pub ndrawback: bool,
}

/// A bonus entry with `sobj` already resolved to a sobject index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bonus {
    pub timer: i32,
    pub timer_v: i32,
    pub frame: i32,
    pub sobj: i32, // ObjRefFromStr against types.sobjects
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ColorAnim {
    pub from: i32,
    pub to: i32,
}

// ----- [constants.aiparams.*] -----
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AiKey {
    pub on: i32,
    pub off: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AiParams {
    pub up: AiKey,
    pub down: AiKey,
    pub left: AiKey,
    pub right: AiKey,
    pub fire: AiKey,
    pub change: AiKey,
    pub jump: AiKey,
}

impl AiParams {
    /// 7 keys × (on, off) in engine order up..jump.
    pub fn ordered(&self) -> [(i32, i32); 7] {
        [
            (self.up.on, self.up.off),
            (self.down.on, self.down.off),
            (self.left.on, self.left.off),
            (self.right.on, self.right.off),
            (self.fire.on, self.fire.off),
            (self.change.on, self.change.off),
            (self.jump.on, self.jump.off),
        ]
    }
}

// ----- [texts] (LIERO_SDEFS order) -----
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Texts {
    pub InitSound: String,
    pub LoadingSounds: String,
    pub LoadingAndThinking: String,
    pub OK: String,
    pub OK2: String,
    pub PressAnyKey: String,
    pub CommittedSuicideMsg: String,
    pub KilledMsg: String,
    pub YoureIt: String,
    pub Init_BaseIO: String,
    pub Init_IRQ: String,
    pub Init_DMA8: String,
    pub Init_DMA16: String,
    pub Init_DSPVersion: String,
    pub Init_Colon: String,
    pub Init_16bit: String,
    pub Init_Autoinit: String,
    pub Init_XMSSucc: String,
    pub Init_FreeXMS: String,
    pub Init_k: String,
    pub Random: String,
    pub Random2: String,
    pub RegenLevel: String,
    pub ReloadLevel: String,
    pub Copyright: String,
    pub Copyright2: String,
    pub SelWeap: String,
    pub LevelRandom: String,
    pub LevelIs1: String,
    pub LevelIs2: String,
    pub Randomize: String,
    pub Done: String,
    pub Reloading: String,
    pub PressFire: String,
    pub Kills: String,
    pub Lives: String,
    pub SelLevel: String,
    pub Weapon: String,
    pub Availability: String,
    pub NoWeaps: String,
}

impl Texts {
    pub fn ordered(&self) -> [&str; 40] {
        [
            &self.InitSound, &self.LoadingSounds, &self.LoadingAndThinking,
            &self.OK, &self.OK2, &self.PressAnyKey, &self.CommittedSuicideMsg,
            &self.KilledMsg, &self.YoureIt, &self.Init_BaseIO, &self.Init_IRQ,
            &self.Init_DMA8, &self.Init_DMA16, &self.Init_DSPVersion,
            &self.Init_Colon, &self.Init_16bit, &self.Init_Autoinit,
            &self.Init_XMSSucc, &self.Init_FreeXMS, &self.Init_k, &self.Random,
            &self.Random2, &self.RegenLevel, &self.ReloadLevel, &self.Copyright,
            &self.Copyright2, &self.SelWeap, &self.LevelRandom, &self.LevelIs1,
            &self.LevelIs2, &self.Randomize, &self.Done, &self.Reloading,
            &self.PressFire, &self.Kills, &self.Lives, &self.SelLevel,
            &self.Weapon, &self.Availability, &self.NoWeaps,
        ]
    }
}

// ----- [hacks] (LIERO_HDEFS order) -----
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Hacks {
    pub FallDamage: bool,
    pub BonusReloadOnly: bool,
    pub BonusSpawnRect: bool,
    pub BonusOnlyHealth: bool,
    pub BonusOnlyWeapon: bool,
    pub BonusDisable: bool,
    pub WormFloat: bool,
    pub RemExp: bool,
    pub SignedRecoil: bool,
    pub AirJump: bool,
    pub MultiJump: bool,
}

impl Hacks {
    pub fn ordered(&self) -> [bool; 11] {
        [
            self.FallDamage, self.BonusReloadOnly, self.BonusSpawnRect,
            self.BonusOnlyHealth, self.BonusOnlyWeapon, self.BonusDisable,
            self.WormFloat, self.RemExp, self.SignedRecoil, self.AirJump,
            self.MultiJump,
        ]
    }
}

// ----- [sounds] hook indices (LIERO_SOUNDDEFS order), resolved -----
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundHooks {
    pub MenuMoveUp: i32,
    pub MenuMoveDown: i32,
    pub MenuSelect: i32,
    pub Bump: i32,
    pub Begin: i32,
    pub Reloaded: i32,
    pub Alive: i32,
    pub NinjaropeThrow: i32,
}

impl SoundHooks {
    pub fn ordered(&self) -> [i32; 8] {
        [
            self.MenuMoveUp, self.MenuMoveDown, self.MenuSelect, self.Bump,
            self.Begin, self.Reloaded, self.Alive, self.NinjaropeThrow,
        ]
    }
}

// ----- public aggregate -----
// NB: `Default` is hand-written (not derived) because `[u8; 256]` does not
// implement `Default` (std only covers arrays up to length 32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcConfig {
    pub types: TcTypes,
    pub constants: Constants,
    pub materials: [u8; 256], // fixed; mirrors C++ Material materials[256]
    pub textures: Vec<Texture>,
    pub bonuses: Vec<Bonus>,
    pub color_anim: Vec<ColorAnim>,
    pub aiparams: AiParams,
    pub texts: Texts,
    pub hacks: Hacks,
    pub sound_hooks: SoundHooks,
}

impl Default for TcConfig {
    fn default() -> Self {
        TcConfig {
            types: TcTypes::default(),
            constants: Constants::default(),
            materials: [0u8; 256],
            textures: Vec::new(),
            bonuses: Vec::new(),
            color_anim: Vec::new(),
            aiparams: AiParams::default(),
            texts: Texts::default(),
            hacks: Hacks::default(),
            sound_hooks: SoundHooks::default(),
        }
    }
}

// ----- private deserialize mirrors -----
#[derive(Default, Deserialize)]
#[serde(default)]
struct RawCollections {
    materials: Vec<i32>,
    textures: Vec<Texture>,
    bonuses: Vec<RawBonus>,
    #[serde(rename = "colorAnim")]
    color_anim: Vec<ColorAnim>,
    aiparams: AiParams,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawBonus {
    timer: i32,
    #[serde(rename = "timerV")]
    timer_v: i32,
    frame: i32,
    sobj: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawSounds {
    MenuMoveUp: String,
    MenuMoveDown: String,
    MenuSelect: String,
    Bump: String,
    Begin: String,
    Reloaded: String,
    Alive: String,
    NinjaropeThrow: String,
}

// ObjRefFromStr (common_model.hpp:24): empty -> -1, unknown -> 0, else index.
fn obj_ref_from_str(s: &str, list: &[String]) -> i32 {
    if s.is_empty() {
        return -1;
    }
    match list.iter().position(|n| n == s) {
        Some(i) => i as i32,
        None => 0,
    }
}

// SoundIndex (common.cpp:574): -1 if absent, else index.
fn sound_index(name: &str, sounds: &[String]) -> i32 {
    sounds.iter().position(|n| n == name).map_or(-1, |i| i as i32)
}

// resolveHook (common_model.hpp:654): configured non-empty -> SoundIndex;
// empty -> SoundIndex(default_name).
fn resolve_hook(configured: &str, default_name: &str, sounds: &[String]) -> i32 {
    if configured.is_empty() {
        sound_index(default_name, sounds)
    } else {
        sound_index(configured, sounds)
    }
}

impl TcConfig {
    /// Parse a `tc.cfg` byte buffer. Mirrors `LoadTcConfig`
    /// (`common_model.hpp:565`): deserialize the five sections, cap the
    /// fixed-size arrays, store materials as `& 0xff` flags, and resolve
    /// bonus-sobj and sound-hook names to indices.
    pub fn load(bytes: &[u8]) -> Result<TcConfig, TcError> {
        let text = std::str::from_utf8(bytes).map_err(|e| TcError::Parse(e.to_string()))?;
        let doc: toml::Table = toml::from_str(text).map_err(|e| TcError::Parse(e.to_string()))?;

        let sub = |key: &str| -> toml::Value {
            doc.get(key)
                .cloned()
                .unwrap_or_else(|| toml::Value::Table(toml::Table::new()))
        };
        let de = |e: toml::de::Error| TcError::Parse(e.to_string());

        let types: TcTypes = sub("types").try_into().map_err(de)?;
        let cval = sub("constants");
        // Read the [constants] table twice from two angles: the scalar struct
        // ignores collection keys, the collection struct ignores scalar keys
        // (serde ignores unknown fields). Avoids `#[serde(flatten)]`, which is
        // unreliable in the toml crate alongside arrays-of-tables.
        let constants: Constants = cval.clone().try_into().map_err(de)?;
        let coll: RawCollections = cval.try_into().map_err(de)?;
        let texts: Texts = sub("texts").try_into().map_err(de)?;
        let hacks: Hacks = sub("hacks").try_into().map_err(de)?;
        let raw_sounds: RawSounds = sub("sounds").try_into().map_err(de)?;

        // Fold the raw Vec<i32> into the fixed [u8; 256] (C++ LoadTcConfig writes
        // materials[i].flags = value & 0xff for i < 256; common_model.hpp:621-623).
        // The shipped file has exactly 256; a shorter list leaves the tail at 0.
        let mut materials = [0u8; MAX_MATERIALS];
        for (i, &v) in coll.materials.iter().take(MAX_MATERIALS).enumerate() {
            materials[i] = (v & 0xff) as u8;
        }

        let mut textures = coll.textures;
        textures.truncate(NUM_TEXTURES);

        let mut color_anim = coll.color_anim;
        color_anim.truncate(NUM_COLOR_ANIM);

        let bonuses: Vec<Bonus> = coll
            .bonuses
            .iter()
            .take(NUM_BONUS_SOBJECTS)
            .map(|b| Bonus {
                timer: b.timer,
                timer_v: b.timer_v,
                frame: b.frame,
                sobj: obj_ref_from_str(&b.sobj, &types.sobjects),
            })
            .collect();

        let s = &types.sounds;
        let sound_hooks = SoundHooks {
            MenuMoveUp: resolve_hook(&raw_sounds.MenuMoveUp, "moveup", s),
            MenuMoveDown: resolve_hook(&raw_sounds.MenuMoveDown, "movedown", s),
            MenuSelect: resolve_hook(&raw_sounds.MenuSelect, "select", s),
            Bump: resolve_hook(&raw_sounds.Bump, "bump", s),
            Begin: resolve_hook(&raw_sounds.Begin, "begin", s),
            Reloaded: resolve_hook(&raw_sounds.Reloaded, "reloaded", s),
            Alive: resolve_hook(&raw_sounds.Alive, "alive", s),
            NinjaropeThrow: resolve_hook(&raw_sounds.NinjaropeThrow, "throw", s),
        };

        Ok(TcConfig {
            types,
            constants,
            materials,
            textures,
            bonuses,
            color_anim,
            aiparams: coll.aiparams,
            texts,
            hacks,
            sound_hooks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[types]
sounds = ["bump", "begin"]
weapons = ["bazooka"]
nobjects = ["blood"]
sobjects = ["small_explosion", "zimm_flash"]

[constants]
materials = [0, 9, 300, 257]
WormGravity = 1500
JumpForce = 56064
RemExpObject = 35

[[constants.bonuses]]
timer = 3000
timerV = 2000
frame = 100
sobj = "zimm_flash"

[[constants.bonuses]]
timer = 1
timerV = 2
frame = 3
sobj = "does_not_exist"

[[constants.textures]]
mframe = 0
rframe = 2
sframe = 73
ndrawback = true

[[constants.colorAnim]]
from = 129
to = 131

[constants.aiparams.fire]
on = 80
off = 81

[texts]
OK = "OK"
Copyright = "a\u0000b"
Copyright2 = "\u00e4"

[hacks]
FallDamage = true
MultiJump = true

[sounds]
Bump = "bump"
Begin = ""
"#;

    fn load() -> TcConfig {
        TcConfig::load(SAMPLE.as_bytes()).unwrap()
    }

    #[test]
    fn types_lists() {
        let c = load();
        assert_eq!(c.types.sounds, vec!["bump", "begin"]);
        assert_eq!(c.types.sobjects, vec!["small_explosion", "zimm_flash"]);
    }

    #[test]
    fn scalars_and_defaults() {
        let c = load();
        assert_eq!(c.constants.WormGravity, 1500);
        assert_eq!(c.constants.JumpForce, 56064);
        assert_eq!(c.constants.RemExpObject, 35);
        assert_eq!(c.constants.NRInitialLength, 0); // missing -> default
    }

    #[test]
    fn materials_masked_to_flags() {
        let c = load();
        // [0, 9, 300, 257] -> & 0xff = [0, 9, 44, 1]; rest of the fixed
        // [u8; 256] is zero-filled (only 4 entries in SAMPLE).
        assert_eq!(&c.materials[..4], &[0u8, 9, 44, 1]);
        assert!(c.materials[4..].iter().all(|&x| x == 0));
    }

    #[test]
    fn bonus_sobj_resolution() {
        let c = load();
        assert_eq!(c.bonuses.len(), 2);
        // "zimm_flash" is index 1 in sobjects.
        assert_eq!(c.bonuses[0].sobj, 1);
        assert_eq!(c.bonuses[0].timer_v, 2000);
        // unknown name -> 0 (ObjRefFromStr).
        assert_eq!(c.bonuses[1].sobj, 0);
    }

    #[test]
    fn sound_hooks_resolution() {
        let c = load();
        // configured "bump" -> index 0 in sounds.
        assert_eq!(c.sound_hooks.Bump, 0);
        // empty Begin -> default "begin" -> index 1.
        assert_eq!(c.sound_hooks.Begin, 1);
        // unconfigured Alive -> default "alive", absent -> -1.
        assert_eq!(c.sound_hooks.Alive, -1);
    }

    #[test]
    fn textures_and_coloranim_and_ai() {
        let c = load();
        assert_eq!(c.textures.len(), 1);
        assert_eq!(c.textures[0].sframe, 73);
        assert!(c.textures[0].ndrawback);
        assert_eq!(c.color_anim, vec![ColorAnim { from: 129, to: 131 }]);
        assert_eq!(c.aiparams.fire, AiKey { on: 80, off: 81 });
        assert_eq!(c.aiparams.up, AiKey::default());
    }

    #[test]
    fn texts_with_escapes_and_hacks() {
        let c = load();
        assert_eq!(c.texts.OK, "OK");
        // Embedded-NUL escape decodes to the raw NUL byte.
        assert_eq!(c.texts.Copyright.as_bytes(), b"a\x00b");
        // Multi-byte UTF-8 escape (ä = U+00E4) decodes to its 2 UTF-8 bytes.
        assert_eq!(c.texts.Copyright2.as_bytes(), &[0xc3, 0xa4]);
        assert!(c.hacks.FallDamage);
        assert!(c.hacks.MultiJump);
        assert!(!c.hacks.RemExp);
    }

    #[test]
    fn malformed_toml_is_error() {
        assert!(matches!(
            TcConfig::load(b"this is = = not toml"),
            Err(TcError::Parse(_))
        ));
    }
}
```

- [ ] **Step 2: Run the unit tests**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets tc`
Expected: all `tc` tests PASS (smoke + 8 unit tests).

- [ ] **Step 3: Run the full assets suite (no regressions)**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: prior assets tests (level/palette/sprite) still PASS plus the new tc tests.

- [ ] **Step 4: Commit**

```bash
git add rust/assets/src/tc.rs
git commit -m "feat(assets): tc.cfg loader (types/constants/materials/bonuses/hacks/sound hooks)"
```

---

### Task 2: tc golden — differential test vs real `Common::load`

Prove the parsed `tc.cfg` values match the C++ engine bit-for-bit.

**Files:**
- Create: `src/tools/oracle_dump/tc_config_dump.cpp`
- Modify: `CMakeLists.txt` (inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block)
- Create: `rust/oracle-tests/gen_tc_golden.sh`
- Create: `rust/oracle-tests/golden/tc.txt` (generated)
- Create: `rust/oracle-tests/tests/tc_golden.rs`

**Interfaces:**
- Consumes: `TcConfig` (Task 1); the real C++ `Common` fields populated by `LoadTcConfig` (`c[]`, `materials[].flags`, `textures[]`, `bonus_*[]`, `ai_params.k`, `color_anim[]`, `s[]`, `h[]`, `sound_hook[]`, and `sounds[].name`/`weapons[].id_str`/… for the types lists).
- Produces: golden file — one `label hash` line per group (see Global Constraints / design golden format).

- [ ] **Step 1: Write the C++ tc dumper**

Create `src/tools/oracle_dump/tc_config_dump.cpp`:

```cpp
// Generates golden digests for the Rust tc.cfg differential test by running the
// REAL C++ Common::load (which calls LoadTcConfig). Links the `game` library;
// built via the OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the
// default build. Usage: oracle_dump_tc <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

#include "common.hpp"
#include "constants.hpp"
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

void PushU32(std::vector<unsigned char>& b, uint32_t v) {
  b.push_back(static_cast<unsigned char>(v & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 8) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 16) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 24) & 0xff));
}

void PushI32(std::vector<unsigned char>& b, int32_t v) {
  PushU32(b, static_cast<uint32_t>(v));
}

void PushStr(std::vector<unsigned char>& b, std::string const& s) {
  PushU32(b, static_cast<uint32_t>(s.size()));
  for (char c : s) {
    b.push_back(static_cast<unsigned char>(c));
  }
}

void Emit(std::FILE* out, char const* label, std::vector<unsigned char> const& b) {
  std::fprintf(out, "%s %016llx\n", label, static_cast<unsigned long long>(Fnv1a(b)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_tc <tc-dir> <out.txt>\n");
    return 1;
  }
  Common common;
  common.load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.sounds.size()));
    for (auto const& s : common.sounds) PushStr(b, s.name);
    Emit(out, "types_sounds", b);
  }
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.weapons.size()));
    for (auto const& w : common.weapons) PushStr(b, w.id_str);
    Emit(out, "types_weapons", b);
  }
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.nobject_types.size()));
    for (auto const& w : common.nobject_types) PushStr(b, w.id_str);
    Emit(out, "types_nobjects", b);
  }
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.sobject_types.size()));
    for (auto const& w : common.sobject_types) PushStr(b, w.id_str);
    Emit(out, "types_sobjects", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_C(n) PushI32(b, common.c[C##n]);
    LIERO_CDEFS(HASH_C)
#undef HASH_C
    Emit(out, "constants", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < MAX_MATERIALS; ++i) b.push_back(common.materials[i].flags);
    Emit(out, "materials", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < NUM_TEXTURES; ++i) {
      Texture const& t = common.textures[i];
      PushI32(b, t.m_frame);
      PushI32(b, t.r_frame);
      PushI32(b, t.s_frame);
      b.push_back(t.n_draw_back ? 1 : 0);
    }
    Emit(out, "textures", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < NUM_BONUS_SOBJECTS; ++i) {
      PushI32(b, common.bonus_rand_timer[i][0]);
      PushI32(b, common.bonus_rand_timer[i][1]);
      PushI32(b, common.bonus_frames[i]);
      PushI32(b, common.bonus_s_objects[i]);
    }
    Emit(out, "bonuses", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < NUM_COLOR_ANIM; ++i) {
      PushI32(b, common.color_anim[i].from);
      PushI32(b, common.color_anim[i].to);
    }
    Emit(out, "coloranim", b);
  }

  {
    // Engine stores k[1][idx] = on, k[0][idx] = off; idx 0..6 = up..jump.
    std::vector<unsigned char> b;
    for (int idx = 0; idx < NUM_AIPARAMS_KEYS; ++idx) {
      PushI32(b, common.ai_params.k[1][idx]);
      PushI32(b, common.ai_params.k[0][idx]);
    }
    Emit(out, "aiparams", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_S(n) PushStr(b, common.s[S##n]);
    LIERO_SDEFS(HASH_S)
#undef HASH_S
    Emit(out, "texts", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_H(n) b.push_back(common.h[H##n] ? 1 : 0);
    LIERO_HDEFS(HASH_H)
#undef HASH_H
    Emit(out, "hacks", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_SO(n) PushI32(b, common.sound_hook[Sound##n]);
    LIERO_SOUNDDEFS(HASH_SO)
#undef HASH_SO
    Emit(out, "soundhooks", b);
  }

  std::fclose(out);
  return 0;
}
```

- [ ] **Step 2: Register the CMake target**

In `CMakeLists.txt`, inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block (after the `oracle_dump_palette` lines, before `endif()`), add:

```cmake
  add_executable(oracle_dump_tc src/tools/oracle_dump/tc_config_dump.cpp)
  target_link_libraries(oracle_dump_tc PRIVATE game)
```

- [ ] **Step 3: Write the regeneration script**

Create `rust/oracle-tests/gen_tc_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/tc.txt by running the REAL C++ Common::load (which calls
# LoadTcConfig). Needs the full C++ build (links the `game` target), so this is
# a LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET
# for other platforms (e.g. linux-x64). Run from the repo root so the TC dir
# resolves the same way the in-tree tests do.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_tc
(
  cd "$ROOT"
  "build/$PRESET/Release/oracle_dump_tc" \
    "data/TC/openliero" \
    "rust/oracle-tests/golden/tc.txt"
)
echo "wrote rust/oracle-tests/golden/tc.txt"
```

Make it executable:

```bash
chmod +x rust/oracle-tests/gen_tc_golden.sh
```

- [ ] **Step 4: Generate the golden**

Run: `bash rust/oracle-tests/gen_tc_golden.sh`
Expected: prints `wrote rust/oracle-tests/golden/tc.txt`; the file has 13 lines: `types_sounds`, `types_weapons`, `types_nobjects`, `types_sobjects`, `constants`, `materials`, `textures`, `bonuses`, `coloranim`, `aiparams`, `texts`, `hacks`, `soundhooks`, each with a 16-hex digest.

- [ ] **Step 5: Write the Rust golden test**

Create `rust/oracle-tests/tests/tc_golden.rs`:

```rust
//! Differential test for the tc.cfg loader against the C++ oracle. The golden
//! (one `label hash` line per group) is produced by the real C++ `Common::load`
//! (`LoadTcConfig`); the Rust loader must reproduce every FNV-1a digest from the
//! same shipped `data/TC/openliero/tc.cfg`. Digest byte layout matches the C++
//! dumper exactly (LE ints; strings = u32 LE length + bytes).

use std::collections::HashMap;

use assets::tc::{Bonus, ColorAnim, TcConfig, Texture};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn push_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push_i32(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push_str(b: &mut Vec<u8>, s: &str) {
    push_u32(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
}

fn hash_names(names: &[String]) -> u64 {
    let mut b = Vec::new();
    push_u32(&mut b, names.len() as u32);
    for n in names {
        push_str(&mut b, n);
    }
    fnv1a(&b)
}

fn parse_golden(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split_whitespace();
            let label = it.next().unwrap().to_string();
            let hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            (label, hash)
        })
        .collect()
}

#[test]
fn tc_config_matches_cpp_oracle() {
    let golden = parse_golden(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/tc.txt"
    )));
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/tc.cfg"
    ));
    let cfg = TcConfig::load(bytes).expect("tc.cfg parses");

    let want = |k: &str| *golden.get(k).unwrap_or_else(|| panic!("missing golden {k}"));

    // types
    assert_eq!(hash_names(&cfg.types.sounds), want("types_sounds"));
    assert_eq!(hash_names(&cfg.types.weapons), want("types_weapons"));
    assert_eq!(hash_names(&cfg.types.nobjects), want("types_nobjects"));
    assert_eq!(hash_names(&cfg.types.sobjects), want("types_sobjects"));

    // constants (72 i32 in CDEFS order)
    {
        let mut b = Vec::new();
        for v in cfg.constants.ordered() {
            push_i32(&mut b, v);
        }
        assert_eq!(fnv1a(&b), want("constants"));
    }

    // materials (fixed [u8; 256] flags; mirrors the engine's materials[256])
    {
        assert_eq!(fnv1a(&cfg.materials), want("materials"));
    }

    // textures (9 × mframe,rframe,sframe i32 + ndrawback byte; pad to 9)
    {
        let mut t = cfg.textures.clone();
        t.resize(9, Texture::default());
        let mut b = Vec::new();
        for x in &t {
            push_i32(&mut b, x.mframe);
            push_i32(&mut b, x.rframe);
            push_i32(&mut b, x.sframe);
            b.push(if x.ndrawback { 1 } else { 0 });
        }
        assert_eq!(fnv1a(&b), want("textures"));
    }

    // bonuses (2 × timer,timerV,frame,sobj i32; pad to 2)
    {
        let mut bo = cfg.bonuses.clone();
        bo.resize(2, Bonus::default());
        let mut b = Vec::new();
        for x in &bo {
            push_i32(&mut b, x.timer);
            push_i32(&mut b, x.timer_v);
            push_i32(&mut b, x.frame);
            push_i32(&mut b, x.sobj);
        }
        assert_eq!(fnv1a(&b), want("bonuses"));
    }

    // coloranim (4 × from,to; pad to 4)
    {
        let mut ca = cfg.color_anim.clone();
        ca.resize(4, ColorAnim::default());
        let mut b = Vec::new();
        for x in &ca {
            push_i32(&mut b, x.from);
            push_i32(&mut b, x.to);
        }
        assert_eq!(fnv1a(&b), want("coloranim"));
    }

    // aiparams (7 × on,off; up..jump)
    {
        let mut b = Vec::new();
        for (on, off) in cfg.aiparams.ordered() {
            push_i32(&mut b, on);
            push_i32(&mut b, off);
        }
        assert_eq!(fnv1a(&b), want("aiparams"));
    }

    // texts (40 strings, SDEFS order)
    {
        let mut b = Vec::new();
        for s in cfg.texts.ordered() {
            push_str(&mut b, s);
        }
        assert_eq!(fnv1a(&b), want("texts"));
    }

    // hacks (11 bools, HDEFS order)
    {
        let mut b = Vec::new();
        for h in cfg.hacks.ordered() {
            b.push(if h { 1 } else { 0 });
        }
        assert_eq!(fnv1a(&b), want("hacks"));
    }

    // soundhooks (8 i32, SOUNDDEFS order)
    {
        let mut b = Vec::new();
        for i in cfg.sound_hooks.ordered() {
            push_i32(&mut b, i);
        }
        assert_eq!(fnv1a(&b), want("soundhooks"));
    }
}
```

- [ ] **Step 6: Run the full workspace suite**

Run: `cargo test --manifest-path rust/Cargo.toml --workspace`
Expected: ALL tests PASS (sim-core goldens, assets unit incl. tc, level/palette/sprite goldens, tc_golden).

- [ ] **Step 7: Commit**

```bash
git add src/tools/oracle_dump/tc_config_dump.cpp CMakeLists.txt \
  rust/oracle-tests/gen_tc_golden.sh rust/oracle-tests/golden/tc.txt \
  rust/oracle-tests/tests/tc_golden.rs
git commit -m "test(oracle): tc.cfg differential test vs C++ Common::load"
```

---

## Self-Review

**Spec coverage:**
- `[types]` lists → Task 1 (`TcTypes`) + golden `types_*`. ✓
- `[constants]` 72 scalars → Task 1 (`Constants` + `ordered`) + golden `constants`. ✓
- `materials` (cap 256, `& 0xff`) → Task 1 + golden `materials`. ✓
- `textures` (cap 9) → Task 1 + golden `textures`. ✓
- `bonuses` (cap 2, sobj resolved) → Task 1 + golden `bonuses`. ✓
- `colorAnim` (cap 4) → Task 1 + golden `coloranim`. ✓
- `aiparams` (7 keys up..jump, on/off) → Task 1 (`AiParams::ordered`) + golden `aiparams`. ✓
- `[texts]` (40) → Task 1 + golden `texts`. ✓
- `[hacks]` (11) → Task 1 + golden `hacks`. ✓
- `[sounds]` hooks (8, resolved + fallback) → Task 1 (`resolve_hook`) + golden `soundhooks`. ✓
- TOML-crate / `\u0000` risk → Task 0 smoke test (early BLOCKED signal). ✓
- Object configs / WAV / Precompute → out of scope (1e-2 / 1e-3 / deferred). ✓

**Placeholder scan:** No TBD/TODO; all code is complete. The Task 0 stub `tc.rs` is fully replaced in Task 1 Step 1. ✓

**Type/order consistency:** `Constants::ordered` (72) ↔ C++ `LIERO_CDEFS(HASH_C)`; `Texts::ordered` (40) ↔ `LIERO_SDEFS(HASH_S)`; `Hacks::ordered` (11) ↔ `LIERO_HDEFS(HASH_H)`; `SoundHooks::ordered` (8) ↔ `LIERO_SOUNDDEFS(HASH_SO)`; `AiParams::ordered` up..jump ↔ dumper idx 0..6 with `k[1]=on,k[0]=off`. All field lists transcribed from `src/game/constants.hpp:15–155` (verified: 72/40/11/8). Digest byte layout (`push_i32`/`push_u32`/`push_str` = LE ints, u32-len-prefixed strings) identical to the C++ `PushI32`/`PushU32`/`PushStr`. Array caps (256/9/2/4) identical on both sides; `materials` is a fixed `[u8; 256]` (no padding needed), and the Rust golden pads the `Vec` arrays (textures/bonuses/coloranim) to the engine's fixed sizes (no-op for the shipped file, which has exactly those counts). ✓

**Cross-ref semantics:** `obj_ref_from_str` (empty→-1, unknown→0, hit→idx) == `ObjRefFromStr` (`common_model.hpp:24`); `resolve_hook` empty→default-name lookup, configured→`SoundIndex` (−1 if unknown) == `resolveHook` (`common_model.hpp:654`); both proven by unit tests and pinned by the golden (`bonuses`/`soundhooks`). ✓

**Accepted divergence:** serde errors on wrong-typed keys where C++ silently keeps the default; the locked contract is the real (well-formed) shipped asset used by the golden. Noted in the design's charter check. ✓

**Note on `materials` representation:** stored as a fixed `[u8; 256]` (mirrors C++ `Material materials[256]`) so step 2 can index `materials[material_id[i]]` with an arbitrary `u8` in-bounds. The raw TOML deserializes as `Vec<i32>`; `load` folds up to 256 entries in with `& 0xff` and zero-fills any tail. The hashed byte sequence is still 256 bytes, so the golden digest is identical to a `Vec`-then-`resize(256)` framing. `TcConfig` therefore has a hand-written `Default` (arrays > 32 don't derive `Default`). Resolves design open question #3. ✓
