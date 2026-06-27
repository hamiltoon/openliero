# Step 1e-2 — object configs (weapon / nobject / sobject) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read OpenLiero's per-object `.cfg` files (`weapons/*.cfg`,
`nobjects/*.cfg`, `sobjects/*.cfg`, TOML) in Rust, reproducing the C++ engine's
parsed values **and every resolved cross-reference index**, by adding an `object`
module to the `assets` crate. Reuses the `serde`/`toml` dependency and the
type-name lists from 1e-1.

**Architecture:** `object.rs` deserializes each flat `.cfg` with `serde`/`toml`
into a private `Raw*` mirror (camelCase TOML keys), then resolves the name→index
cross-refs into typed `Weapon` / `NObjectType` / `SObjectType` structs. Resolution
reuses 1e-1's semantics (`ObjRefFromStr`: empty→-1, unknown→0, hit→index;
`SoundRefFromStr`: empty→-1, unknown→-1, hit→index) against `TcConfig.types`. An
`Objects::load` aggregate mirrors the `Common::load` loop (`common.cpp:437–507`),
reading each file by `id_str` and assigning `id = array index`. Correctness is
proven by a C++ oracle dumper that runs the real `Common::load` and a golden
differential test that reproduces its FNV-1a digests.

**Tech Stack:** Rust (`assets`, `oracle-tests`), `serde` + `toml` (already deps,
from 1e-1), C++ oracle dumper (`oracle_dump_object`, links `game`), CMake option
`OPENLIERO_BUILD_ORACLE_DUMP`, FNV-1a digests.

## Global Constraints

- **Bit-exact vs C++ for parsed values AND resolved indices.** Source of truth:
  `src/game/common_model.hpp:256–336` (`LoadWeaponConfig`), `96–138`
  (`LoadNObjectConfig`), `160–178` (`LoadSObjectConfig`), `16–52` (cross-ref
  helpers); struct members `src/game/weapon.hpp:12–332`, `src/game/nobject.hpp:18–199`,
  `src/game/sobject.hpp:18–87`; load loop + `id=index` `src/game/common.cpp:437–507`.
  Every field of every weapon/nobject/sobject and every resolved cross-ref index
  must reproduce C++ exactly. Rendering/audio fields (`shadow`, `laserSight`,
  `fireCone`, `loopAnim`, `name`, the sound refs, sobject `animDelay`/`startFrame`/
  `numFrames`/`shake`/`flash`) are parsed and golden-verified too ("read the bytes
  the same"). Note `startFrame`/`numFrames`/`numSounds`/`colorBullets` feed RNG
  draws (`weapon.cpp:43,68`, `nobject.cpp:25`, `sobject.cpp:24`) and are
  sim-critical.
- **`fixed` is `int`** (`math.hpp:7`): every numeric field (incl. `gravity`,
  `addSpeed`) is `i32`.
- **Defaults (match C++):** the `weapons[i]`/`nobject_types[i]`/`sobject_types[i]`
  slot is value-initialized (`vector::resize`) to `0`/`false`/`""` before reading;
  absent keys keep that default (`toml_archive.hpp:217–268` only assigns present,
  correctly-typed keys). Reproduced with `#[serde(default)]`; ref strings default
  to `""` → resolve as empty.
- **Cross-ref semantics (sim state):**
  - `obj_ref_from_str` = `ObjRefFromStr` (`common_model.hpp:24`): empty→`-1`,
    unknown→`0`, else index. Used by object refs (`createOnExp`, `splinterType`,
    `objTrailType`, `partTrailObj`, `leaveObj`).
  - `sound_ref_from_str` = `SoundRefFromStr` (`common_model.hpp:47`): empty→`-1`,
    else `SoundIndex` (→`-1` if absent). Used by `launchSound`/`exploSound`/
    `startSound`.
  - **`Weapon::loop_sound` is `bool`** (`weapon.hpp:67`) assigned the int result
    of `SoundRefFromStr` (`common_model.hpp:277–281`): reproduce the C++ int→bool
    as `sound_ref_from_str(...) != 0` (empty/unknown → -1 → `true`; index 0 →
    `false`; index ≥1 → `true`). This is an engine quirk we lock.
- **Resolution lists come from 1e-1:** object refs resolve against
  `TcConfig.types.nobjects` / `.sobjects`; sound refs against `.sounds`. File read
  order + `id = index` follow `TcConfig.types.{weapons,nobjects,sobjects}`.
- **Re-implement the two helpers locally** (do NOT modify `tc.rs`; its copies are
  private and the parallel-agent rule forbids touching `rust/`'s existing files).
  Transcribed from the same C++ source; golden catches any drift.
- **Idiomatic Rust, not a port:** `serde`/`toml` raw→public two-layer split, typed
  `Result`/structs — NOT a port of cereal's per-field `make_nvp`. Public structs use
  snake_case (match C++ members, what step 2 reads); `Raw*` mirrors use camelCase
  TOML keys verbatim (`#![allow(non_snake_case)]`).
- **FNV-1a (64-bit)** seed `0xcbf29ce484222325`, prime `0x100000001b3` — identical
  helper on both sides (see `rust/oracle-tests/tests/tc_golden.rs`). Ints hashed as
  explicit little-endian (`PushI32`); bools as 1 byte; strings as `u32` LE length +
  bytes (`PushStr`). Resolved cross-refs hashed as their `i32` index.
- **Field order in digests = `Load*Config` read order**, hand-listed on both sides
  (no field macro for object structs). C++ dumper inlines the pushes; Rust golden
  test has `encode_weapon`/`encode_nobject`/`encode_sobject` (test-only). Drift
  caught by the golden.
- **No Bevy** in `assets`. **Golden regeneration is LOCAL/MANUAL** (full C++ build
  links `game`); CI (`rust.yml`) runs `cargo test --workspace` against the
  committed golden. PRESET defaults to `macos-arm64`.
- **No AI/"Generated with" taglines** in commits. C++ matches the existing
  `tc_config_dump.cpp` / `level_dump.cpp` Google/100-col style.

## File Structure

- `rust/assets/src/object.rs` — NEW: `Weapon`/`NObjectType`/`SObjectType` +
  `Objects` + loaders + helpers + unit tests.
- `rust/assets/src/lib.rs` — MODIFY: add `pub mod object;`.
- `src/tools/oracle_dump/object_dump.cpp` — NEW: runs real `Common::load`, dumps
  weapon/nobject/sobject digests.
- `CMakeLists.txt` — MODIFY: add `oracle_dump_object` target inside the existing
  `OPENLIERO_BUILD_ORACLE_DUMP` block (after `oracle_dump_tc`).
- `rust/oracle-tests/gen_object_golden.sh` — NEW: regenerate the object golden.
- `rust/oracle-tests/golden/object.txt` — NEW: committed golden.
- `rust/oracle-tests/tests/object_golden.rs` — NEW: object differential test.

No `Cargo.toml` change: `serde` + `toml` were added by 1e-1.

---

### Task 0: Smoke-test the flat `.cfg` dialect

De-risk: prove the `toml` crate parses a real flat object `.cfg` before any struct
work. (Cheap — `toml` is already proven on `tc.cfg`; this guards the flat-table
shape.)

**Files:**
- Create (stub): `rust/assets/src/object.rs`
- Modify: `rust/assets/src/lib.rs`

- [ ] **Step 1: Stub `object.rs` with only a smoke test**

Create `rust/assets/src/object.rs`:

```rust
//! Per-object `.cfg` (TOML) loading: weapon / nobject / sobject parameter
//! tables. Reproduces the values C++ `LoadWeaponConfig` / `LoadNObjectConfig` /
//! `LoadSObjectConfig` (`src/game/common_model.hpp`) parse, including every
//! resolved name→index cross-reference. Idiomatic Rust via `serde`/`toml`, not a
//! port of the cereal `TomlInputArchive`. Consumes 1e-1's type-name lists
//! (`crate::tc::TcTypes`) for cross-ref resolution.
#![allow(non_snake_case)]

#[cfg(test)]
mod smoke {
    /// A real flat object .cfg must parse with the `toml` crate.
    #[test]
    fn real_bazooka_cfg_parses() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/weapons/bazooka.cfg"
        ));
        let text = std::str::from_utf8(bytes).expect("bazooka.cfg is UTF-8");
        let doc: toml::Table = toml::from_str(text).expect("toml crate parses bazooka.cfg");
        assert_eq!(doc.get("name").and_then(|v| v.as_str()), Some("BAZOOKA"));
        assert!(doc.contains_key("splinterType"));
    }
}
```

Add to `rust/assets/src/lib.rs` after the `pub mod tc;` line:

```rust
pub mod object;
```

- [ ] **Step 2: Run the smoke test**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets object::smoke`
Expected: PASS. If it FAILS to parse, STOP and report BLOCKED.

- [ ] **Step 3: Commit**

```bash
git add rust/assets/src/object.rs rust/assets/src/lib.rs
git commit -m "build(assets): stub object module; smoke-test object .cfg parse"
```

---

### Task 1: `NObjectType` — struct, loader, helpers, unit tests

Start with the nobject (mid-size, two object-ref kinds, no sound refs). This task
also introduces the shared `ObjectError`, the two cross-ref helpers, and the
`parse_cfg` helper used by all three structs.

**Files:**
- Modify: `rust/assets/src/object.rs` (replace the smoke stub's top with real code;
  keep a `#[cfg(test)]` module).

**Interfaces (all `pub` unless noted):**
- `enum ObjectError { Parse(String) }`
- `struct NObjectType { /* 28 fields + id + id_str */ }`
- `NObjectType::load(bytes, nobjects: &[String], sobjects: &[String]) -> Result<NObjectType, ObjectError>`
- private `obj_ref_from_str`, `sound_ref_from_str`, `parse_cfg`.

- [ ] **Step 1: Replace `object.rs` with the nobject implementation**

Replace the file contents (the smoke `mod smoke` is folded into the new test
module's real-file test):

```rust
//! Per-object `.cfg` (TOML) loading: weapon / nobject / sobject parameter
//! tables. Reproduces the values C++ `LoadWeaponConfig` / `LoadNObjectConfig` /
//! `LoadSObjectConfig` (`src/game/common_model.hpp`) parse, including every
//! resolved name→index cross-reference. Idiomatic Rust via `serde`/`toml`, not a
//! port of the cereal `TomlInputArchive`. Consumes 1e-1's type-name lists
//! (`crate::tc::TcTypes`) for cross-ref resolution.
#![allow(non_snake_case)]

use serde::Deserialize;

/// Why an object `.cfg` failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum ObjectError {
    /// TOML parse / shape / UTF-8 error (message carried).
    Parse(String),
}

// ObjRefFromStr (common_model.hpp:24): empty -> -1, unknown -> 0, else index.
// Re-implemented here (tc.rs's copy is private; we must not modify tc.rs).
fn obj_ref_from_str(s: &str, list: &[String]) -> i32 {
    if s.is_empty() {
        return -1;
    }
    match list.iter().position(|n| n == s) {
        Some(i) => i as i32,
        None => 0,
    }
}

// SoundRefFromStr (common_model.hpp:47): empty -> -1, else SoundIndex
// (common.cpp:574, -1 if absent). Distinct from obj_ref_from_str: an unknown
// non-empty sound resolves to -1, not 0.
fn sound_ref_from_str(s: &str, sounds: &[String]) -> i32 {
    if s.is_empty() {
        return -1;
    }
    sounds.iter().position(|n| n == s).map_or(-1, |i| i as i32)
}

// Parse a flat `.cfg` byte buffer into a `Raw*` serde mirror.
fn parse_cfg<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ObjectError> {
    let text = std::str::from_utf8(bytes).map_err(|e| ObjectError::Parse(e.to_string()))?;
    toml::from_str(text).map_err(|e| ObjectError::Parse(e.to_string()))
}

// ===== NObjectType =====

/// Parsed `nobjects/<id_str>.cfg`. Cross-refs are resolved to indices.
/// Mirrors `NObjectType` (`src/game/nobject.hpp:18`) + `LoadNObjectConfig`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NObjectType {
    pub worm_explode: bool,
    pub expl_ground: bool,
    pub worm_destroy: bool,
    pub draw_on_map: bool,
    pub affect_by_explosions: bool,
    pub blood_trail: bool,
    pub detect_distance: i32,
    pub gravity: i32,
    pub speed: i32,
    pub speed_v: i32,
    pub distribution: i32,
    pub blow_away: i32,
    pub bounce: i32,
    pub hit_damage: i32,
    pub blood_on_hit: i32,
    pub start_frame: i32,
    pub num_frames: i32,
    pub color_bullets: i32,
    pub create_on_exp: i32, // ObjRefFromStr against sobjects
    pub dirt_effect: i32,
    pub splinter_amount: i32,
    pub splinter_colour: i32,
    pub splinter_type: i32, // ObjRefFromStr against nobjects
    pub blood_trail_delay: i32,
    pub leave_obj: i32, // ObjRefFromStr against sobjects
    pub leave_obj_delay: i32,
    pub time_to_explo: i32,
    pub time_to_explo_v: i32,
    pub id: i32,         // = array index (Precompute)
    pub id_str: String,  // from 1e-1 types.nobjects
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawNObject {
    wormExplode: bool,
    explGround: bool,
    wormDestroy: bool,
    drawOnMap: bool,
    affectByExplosions: bool,
    bloodTrail: bool,
    detectDistance: i32,
    gravity: i32,
    speed: i32,
    speedV: i32,
    distribution: i32,
    blowAway: i32,
    bounce: i32,
    hitDamage: i32,
    bloodOnHit: i32,
    startFrame: i32,
    numFrames: i32,
    colorBullets: i32,
    createOnExp: String,
    dirtEffect: i32,
    splinterAmount: i32,
    splinterColour: i32,
    splinterType: String,
    bloodTrailDelay: i32,
    leaveObj: String,
    leaveObjDelay: i32,
    timeToExplo: i32,
    timeToExploV: i32,
}

impl NObjectType {
    /// Mirrors `LoadNObjectConfig` (`common_model.hpp:96`). `id`/`id_str` are set
    /// by the caller (`Objects::load`); this leaves them at `0`/`""`.
    pub fn load(
        bytes: &[u8],
        nobjects: &[String],
        sobjects: &[String],
    ) -> Result<NObjectType, ObjectError> {
        let r: RawNObject = parse_cfg(bytes)?;
        Ok(NObjectType {
            worm_explode: r.wormExplode,
            expl_ground: r.explGround,
            worm_destroy: r.wormDestroy,
            draw_on_map: r.drawOnMap,
            affect_by_explosions: r.affectByExplosions,
            blood_trail: r.bloodTrail,
            detect_distance: r.detectDistance,
            gravity: r.gravity,
            speed: r.speed,
            speed_v: r.speedV,
            distribution: r.distribution,
            blow_away: r.blowAway,
            bounce: r.bounce,
            hit_damage: r.hitDamage,
            blood_on_hit: r.bloodOnHit,
            start_frame: r.startFrame,
            num_frames: r.numFrames,
            color_bullets: r.colorBullets,
            create_on_exp: obj_ref_from_str(&r.createOnExp, sobjects),
            dirt_effect: r.dirtEffect,
            splinter_amount: r.splinterAmount,
            splinter_colour: r.splinterColour,
            splinter_type: obj_ref_from_str(&r.splinterType, nobjects),
            blood_trail_delay: r.bloodTrailDelay,
            leave_obj: obj_ref_from_str(&r.leaveObj, sobjects),
            leave_obj_delay: r.leaveObjDelay,
            time_to_explo: r.timeToExplo,
            time_to_explo_v: r.timeToExploV,
            id: 0,
            id_str: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lists() -> (Vec<String>, Vec<String>, Vec<String>) {
        // (nobjects, sobjects, sounds)
        (
            vec!["blood".into(), "dirt".into(), "shells".into()],
            vec!["small_explosion".into(), "large_explosion".into()],
            vec!["bump".into(), "begin".into(), "exp2".into()],
        )
    }

    const NOBJ_SAMPLE: &str = r#"
wormExplode = true
explGround = false
gravity = 700
speed = 75
speedV = 40
distribution = 20000
colorBullets = 82
createOnExp = "large_explosion"
splinterType = "does_not_exist"
bloodTrailDelay = 10
timeToExplo = 0
"#;

    #[test]
    fn nobject_scalars_bools_and_defaults() {
        let (n, s, _snd) = lists();
        let o = NObjectType::load(NOBJ_SAMPLE.as_bytes(), &n, &s).unwrap();
        assert!(o.worm_explode);
        assert!(!o.expl_ground);
        assert_eq!(o.gravity, 700);
        assert_eq!(o.distribution, 20000);
        assert_eq!(o.color_bullets, 82);
        assert_eq!(o.blood_trail_delay, 10);
        // Missing key -> default 0/false.
        assert_eq!(o.detect_distance, 0);
        assert!(!o.draw_on_map);
    }

    #[test]
    fn nobject_cross_refs() {
        let (n, s, _snd) = lists();
        let o = NObjectType::load(NOBJ_SAMPLE.as_bytes(), &n, &s).unwrap();
        // createOnExp "large_explosion" -> sobjects index 1.
        assert_eq!(o.create_on_exp, 1);
        // splinterType unknown -> ObjRefFromStr 0.
        assert_eq!(o.splinter_type, 0);
        // leaveObj absent -> "" -> -1.
        assert_eq!(o.leave_obj, -1);
    }

    #[test]
    fn obj_ref_semantics() {
        let list = vec!["a".to_string(), "b".to_string()];
        assert_eq!(obj_ref_from_str("", &list), -1);
        assert_eq!(obj_ref_from_str("nope", &list), 0);
        assert_eq!(obj_ref_from_str("b", &list), 1);
    }

    #[test]
    fn sound_ref_semantics() {
        let snd = vec!["x".to_string(), "y".to_string()];
        assert_eq!(sound_ref_from_str("", &snd), -1);
        assert_eq!(sound_ref_from_str("nope", &snd), -1); // unknown -> -1 (not 0)
        assert_eq!(sound_ref_from_str("y", &snd), 1);
    }

    #[test]
    fn nobject_malformed_is_error() {
        let (n, s, _snd) = lists();
        assert!(matches!(
            NObjectType::load(b"this is = = not toml", &n, &s),
            Err(ObjectError::Parse(_))
        ));
    }

    #[test]
    fn real_blood_cfg_loads() {
        let (n, s, _snd) = lists();
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/nobjects/blood.cfg"
        ));
        let o = NObjectType::load(bytes, &n, &s).unwrap();
        assert!(o.blood_trail);
        assert_eq!(o.gravity, 700);
        // blood.cfg omits createOnExp/splinterType/leaveObj -> all -1.
        assert_eq!(o.create_on_exp, -1);
        assert_eq!(o.splinter_type, -1);
        assert_eq!(o.leave_obj, -1);
    }
}
```

- [ ] **Step 2: Run the nobject tests**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets object`
Expected: all `object` tests PASS.

- [ ] **Step 3: Commit**

```bash
git add rust/assets/src/object.rs
git commit -m "feat(assets): nobject .cfg loader with cross-ref resolution"
```

---

### Task 2: `SObjectType` — struct, loader, unit tests

Append the sobject (smallest; one sound ref). Reuses the helpers from Task 1.

**Files:**
- Modify: `rust/assets/src/object.rs` (append the struct/loader; add tests).

**Interfaces:**
- `struct SObjectType { /* 12 fields + id + id_str */ }`
- `SObjectType::load(bytes, sounds: &[String]) -> Result<SObjectType, ObjectError>`

- [ ] **Step 1: Append the sobject implementation** (after the `NObjectType` impl,
  before the `#[cfg(test)] mod tests`):

```rust
// ===== SObjectType =====

/// Parsed `sobjects/<id_str>.cfg`. Mirrors `SObjectType`
/// (`src/game/sobject.hpp:18`) + `LoadSObjectConfig`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SObjectType {
    pub shadow: bool,
    pub start_sound: i32, // SoundRefFromStr
    pub num_sounds: i32,
    pub anim_delay: i32,
    pub start_frame: i32,
    pub num_frames: i32,
    pub detect_range: i32,
    pub damage: i32,
    pub blow_away: i32,
    pub shake: i32,
    pub flash: i32,
    pub dirt_effect: i32,
    pub id: i32,
    pub id_str: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawSObject {
    shadow: bool,
    startSound: String,
    numSounds: i32,
    animDelay: i32,
    startFrame: i32,
    numFrames: i32,
    detectRange: i32,
    damage: i32,
    blowAway: i32,
    shake: i32,
    flash: i32,
    dirtEffect: i32,
}

impl SObjectType {
    /// Mirrors `LoadSObjectConfig` (`common_model.hpp:160`). `id`/`id_str` set by
    /// the caller.
    pub fn load(bytes: &[u8], sounds: &[String]) -> Result<SObjectType, ObjectError> {
        let r: RawSObject = parse_cfg(bytes)?;
        Ok(SObjectType {
            shadow: r.shadow,
            start_sound: sound_ref_from_str(&r.startSound, sounds),
            num_sounds: r.numSounds,
            anim_delay: r.animDelay,
            start_frame: r.startFrame,
            num_frames: r.numFrames,
            detect_range: r.detectRange,
            damage: r.damage,
            blow_away: r.blowAway,
            shake: r.shake,
            flash: r.flash,
            dirt_effect: r.dirtEffect,
            id: 0,
            id_str: String::new(),
        })
    }
}
```

- [ ] **Step 2: Add sobject tests** (inside the existing `mod tests`):

```rust
    const SOBJ_SAMPLE: &str = r#"
shadow = true
startSound = "exp2"
numSounds = 4
animDelay = 2
startFrame = 40
numFrames = 15
detectRange = 20
damage = 15
blowAway = 3000
shake = 4
flash = 8
dirtEffect = 0
"#;

    #[test]
    fn sobject_fields_and_sound_ref() {
        let (_n, _s, snd) = lists();
        let o = SObjectType::load(SOBJ_SAMPLE.as_bytes(), &snd).unwrap();
        assert!(o.shadow);
        assert_eq!(o.num_sounds, 4);
        assert_eq!(o.detect_range, 20);
        assert_eq!(o.blow_away, 3000);
        assert_eq!(o.dirt_effect, 0);
        // startSound "exp2" -> sounds index 2.
        assert_eq!(o.start_sound, 2);
    }

    #[test]
    fn sobject_missing_sound_is_minus_one() {
        let (_n, _s, snd) = lists();
        // No startSound key -> "" -> -1.
        let o = SObjectType::load(b"shadow = false\ndamage = 3\n", &snd).unwrap();
        assert_eq!(o.start_sound, -1);
        assert_eq!(o.damage, 3);
        assert_eq!(o.num_sounds, 0);
    }

    #[test]
    fn real_large_explosion_cfg_loads() {
        let (_n, _s, snd) = lists();
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sobjects/large_explosion.cfg"
        ));
        let o = SObjectType::load(bytes, &snd).unwrap();
        assert_eq!(o.num_frames, 15);
        assert_eq!(o.damage, 15);
        // "exp2" is in our synthetic list at index 2.
        assert_eq!(o.start_sound, 2);
    }
```

- [ ] **Step 3: Run & commit**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets object`
Expected: PASS.

```bash
git add rust/assets/src/object.rs
git commit -m "feat(assets): sobject .cfg loader with sound-ref resolution"
```

---

### Task 3: `Weapon` + `Objects` aggregate — struct, loader, unit tests

The largest struct (50 fields, all three ref kinds, the `loopSound` int→bool
quirk) plus the `Objects::load` aggregate that mirrors the `common.cpp:437–507`
loop and assigns `id = index`.

**Files:**
- Modify: `rust/assets/src/object.rs` (append `Weapon`, `Objects`; add tests).

**Interfaces:**
- `struct Weapon { /* 50 fields + id + id_str */ }`
- `Weapon::load(bytes, nobjects, sobjects, sounds: &[String]) -> Result<Weapon, ObjectError>`
- `struct Objects { weapons, nobject_types, sobject_types }`
- `Objects::load(types: &crate::tc::TcTypes, read: impl Fn(&str, &str) -> std::io::Result<Vec<u8>>) -> Result<Objects, ObjectError>`

- [ ] **Step 1: Append the weapon implementation** (after `SObjectType`):

```rust
// ===== Weapon =====

/// Parsed `weapons/<id_str>.cfg`. Mirrors `Weapon` (`src/game/weapon.hpp:12`) +
/// `LoadWeaponConfig`. Note `loop_sound` is a `bool` reproducing C++'s int→bool
/// quirk (`common_model.hpp:277`, `weapon.hpp:67`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Weapon {
    pub name: String,
    pub affect_by_worm: bool,
    pub shadow: bool,
    pub laser_sight: bool,
    pub play_reload_sound: bool,
    pub worm_explode: bool,
    pub expl_ground: bool,
    pub worm_collide: bool,
    pub collide_with_objects: bool,
    pub affect_by_explosions: bool,
    pub loop_anim: bool,
    pub detect_distance: i32,
    pub blow_away: i32,
    pub gravity: i32,
    pub launch_sound: i32, // SoundRefFromStr
    pub loop_sound: bool,  // SoundRefFromStr(...) != 0  (int->bool quirk)
    pub explo_sound: i32,  // SoundRefFromStr
    pub speed: i32,
    pub add_speed: i32,
    pub distribution: i32,
    pub parts: i32,
    pub recoil: i32,
    pub mult_speed: i32,
    pub delay: i32,
    pub loading_time: i32,
    pub ammo: i32,
    pub dirt_effect: i32,
    pub leave_shells: i32,
    pub leave_shell_delay: i32,
    pub fire_cone: i32,
    pub bounce: i32,
    pub time_to_explo: i32,
    pub time_to_explo_v: i32,
    pub hit_damage: i32,
    pub blood_on_hit: i32,
    pub start_frame: i32,
    pub num_frames: i32,
    pub shot_type: i32,
    pub color_bullets: i32,
    pub splinter_amount: i32,
    pub splinter_colour: i32,
    pub splinter_type: i32, // ObjRefFromStr against nobjects
    pub splinter_scatter: i32,
    pub obj_trail_type: i32, // ObjRefFromStr against sobjects
    pub obj_trail_delay: i32,
    pub part_trail_type: i32,
    pub part_trail_obj: i32, // ObjRefFromStr against nobjects
    pub part_trail_delay: i32,
    pub create_on_exp: i32, // ObjRefFromStr against sobjects
    pub chain_explosion: bool,
    pub id: i32,
    pub id_str: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawWeapon {
    name: String,
    affectByWorm: bool,
    shadow: bool,
    laserSight: bool,
    playReloadSound: bool,
    wormExplode: bool,
    explGround: bool,
    wormCollide: bool,
    collideWithObjects: bool,
    affectByExplosions: bool,
    loopAnim: bool,
    detectDistance: i32,
    blowAway: i32,
    gravity: i32,
    launchSound: String,
    loopSound: String,
    exploSound: String,
    speed: i32,
    addSpeed: i32,
    distribution: i32,
    parts: i32,
    recoil: i32,
    multSpeed: i32,
    delay: i32,
    loadingTime: i32,
    ammo: i32,
    dirtEffect: i32,
    leaveShells: i32,
    leaveShellDelay: i32,
    fireCone: i32,
    bounce: i32,
    timeToExplo: i32,
    timeToExploV: i32,
    hitDamage: i32,
    bloodOnHit: i32,
    startFrame: i32,
    numFrames: i32,
    shotType: i32,
    colorBullets: i32,
    splinterAmount: i32,
    splinterColour: i32,
    splinterType: String,
    splinterScatter: i32,
    objTrailType: String,
    objTrailDelay: i32,
    partTrailType: i32,
    partTrailObj: String,
    partTrailDelay: i32,
    createOnExp: String,
    chainExplosion: bool,
}

impl Weapon {
    /// Mirrors `LoadWeaponConfig` (`common_model.hpp:256`). `id`/`id_str` set by
    /// the caller.
    pub fn load(
        bytes: &[u8],
        nobjects: &[String],
        sobjects: &[String],
        sounds: &[String],
    ) -> Result<Weapon, ObjectError> {
        let r: RawWeapon = parse_cfg(bytes)?;
        Ok(Weapon {
            name: r.name,
            affect_by_worm: r.affectByWorm,
            shadow: r.shadow,
            laser_sight: r.laserSight,
            play_reload_sound: r.playReloadSound,
            worm_explode: r.wormExplode,
            expl_ground: r.explGround,
            worm_collide: r.wormCollide,
            collide_with_objects: r.collideWithObjects,
            affect_by_explosions: r.affectByExplosions,
            loop_anim: r.loopAnim,
            detect_distance: r.detectDistance,
            blow_away: r.blowAway,
            gravity: r.gravity,
            launch_sound: sound_ref_from_str(&r.launchSound, sounds),
            // C++ stores SoundRefFromStr's int into a bool: value != 0.
            loop_sound: sound_ref_from_str(&r.loopSound, sounds) != 0,
            explo_sound: sound_ref_from_str(&r.exploSound, sounds),
            speed: r.speed,
            add_speed: r.addSpeed,
            distribution: r.distribution,
            parts: r.parts,
            recoil: r.recoil,
            mult_speed: r.multSpeed,
            delay: r.delay,
            loading_time: r.loadingTime,
            ammo: r.ammo,
            dirt_effect: r.dirtEffect,
            leave_shells: r.leaveShells,
            leave_shell_delay: r.leaveShellDelay,
            fire_cone: r.fireCone,
            bounce: r.bounce,
            time_to_explo: r.timeToExplo,
            time_to_explo_v: r.timeToExploV,
            hit_damage: r.hitDamage,
            blood_on_hit: r.bloodOnHit,
            start_frame: r.startFrame,
            num_frames: r.numFrames,
            shot_type: r.shotType,
            color_bullets: r.colorBullets,
            splinter_amount: r.splinterAmount,
            splinter_colour: r.splinterColour,
            splinter_type: obj_ref_from_str(&r.splinterType, nobjects),
            splinter_scatter: r.splinterScatter,
            obj_trail_type: obj_ref_from_str(&r.objTrailType, sobjects),
            obj_trail_delay: r.objTrailDelay,
            part_trail_type: r.partTrailType,
            part_trail_obj: obj_ref_from_str(&r.partTrailObj, nobjects),
            part_trail_delay: r.partTrailDelay,
            create_on_exp: obj_ref_from_str(&r.createOnExp, sobjects),
            chain_explosion: r.chainExplosion,
            id: 0,
            id_str: String::new(),
        })
    }
}

// ===== Aggregate (mirrors Common::load loop + id=index, common.cpp:437-507) =====

/// All three object-parameter tables, indexed exactly as the engine's
/// `weapons[]` / `nobject_types[]` / `sobject_types[]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Objects {
    pub weapons: Vec<Weapon>,
    pub nobject_types: Vec<NObjectType>,
    pub sobject_types: Vec<SObjectType>,
}

impl Objects {
    /// Read every object `.cfg` named in `types`, resolve cross-refs, and assign
    /// `id = array index` (the part of `Precompute` this slice reproduces).
    /// `read(subdir, id_str)` returns the bytes of `<subdir>/<id_str>.cfg`
    /// (subdir ∈ {"weapons","nobjects","sobjects"}) — no coupling to an IO
    /// backend. An IO failure is surfaced as `ObjectError::Parse`.
    pub fn load(
        types: &crate::tc::TcTypes,
        read: impl Fn(&str, &str) -> std::io::Result<Vec<u8>>,
    ) -> Result<Objects, ObjectError> {
        let fetch = |sub: &str, id: &str| -> Result<Vec<u8>, ObjectError> {
            read(sub, id).map_err(|e| ObjectError::Parse(format!("{sub}/{id}.cfg: {e}")))
        };

        let mut weapons = Vec::with_capacity(types.weapons.len());
        for (i, id_str) in types.weapons.iter().enumerate() {
            let bytes = fetch("weapons", id_str)?;
            let mut w = Weapon::load(&bytes, &types.nobjects, &types.sobjects, &types.sounds)?;
            w.id = i as i32;
            w.id_str = id_str.clone();
            weapons.push(w);
        }

        let mut nobject_types = Vec::with_capacity(types.nobjects.len());
        for (i, id_str) in types.nobjects.iter().enumerate() {
            let bytes = fetch("nobjects", id_str)?;
            let mut n = NObjectType::load(&bytes, &types.nobjects, &types.sobjects)?;
            n.id = i as i32;
            n.id_str = id_str.clone();
            nobject_types.push(n);
        }

        let mut sobject_types = Vec::with_capacity(types.sobjects.len());
        for (i, id_str) in types.sobjects.iter().enumerate() {
            let bytes = fetch("sobjects", id_str)?;
            let mut s = SObjectType::load(&bytes, &types.sounds)?;
            s.id = i as i32;
            s.id_str = id_str.clone();
            sobject_types.push(s);
        }

        Ok(Objects {
            weapons,
            nobject_types,
            sobject_types,
        })
    }
}
```

- [ ] **Step 2: Add weapon + aggregate tests** (inside `mod tests`):

```rust
    const WEAP_SAMPLE: &str = r#"
name = "TESTGUN"
affectByWorm = true
wormExplode = true
detectDistance = 1
gravity = 0
launchSound = "exp2"
loopSound = "bump"
exploSound = ""
speed = 200
addSpeed = 3
splinterType = "blood"
objTrailType = "large_explosion"
partTrailObj = "missing_nobj"
createOnExp = "small_explosion"
chainExplosion = false
"#;

    #[test]
    fn weapon_scalars_and_object_refs() {
        let (n, s, snd) = lists();
        let w = Weapon::load(WEAP_SAMPLE.as_bytes(), &n, &s, &snd).unwrap();
        assert_eq!(w.name, "TESTGUN");
        assert!(w.affect_by_worm);
        assert_eq!(w.speed, 200);
        assert_eq!(w.add_speed, 3);
        assert_eq!(w.detect_distance, 1);
        // splinterType "blood" -> nobjects 0; objTrailType "large_explosion" -> sobjects 1.
        assert_eq!(w.splinter_type, 0);
        assert_eq!(w.obj_trail_type, 1);
        // partTrailObj unknown -> ObjRefFromStr 0; createOnExp "small_explosion" -> 0.
        assert_eq!(w.part_trail_obj, 0);
        assert_eq!(w.create_on_exp, 0);
        // Missing keys -> defaults.
        assert_eq!(w.ammo, 0);
        assert!(!w.shadow);
    }

    #[test]
    fn weapon_sound_refs_and_loop_sound_quirk() {
        let (n, s, snd) = lists(); // sounds = [bump(0), begin(1), exp2(2)]
        let w = Weapon::load(WEAP_SAMPLE.as_bytes(), &n, &s, &snd).unwrap();
        // launchSound "exp2" -> 2; exploSound "" -> -1.
        assert_eq!(w.launch_sound, 2);
        assert_eq!(w.explo_sound, -1);
        // loopSound "bump" -> index 0 -> bool false (int->bool quirk).
        assert!(!w.loop_sound);
    }

    #[test]
    fn weapon_loop_sound_empty_is_true() {
        let (n, s, snd) = lists();
        // loopSound "" -> SoundRefFromStr -1 -> -1 != 0 -> true.
        let w = Weapon::load(b"name = \"X\"\n", &n, &s, &snd).unwrap();
        assert!(w.loop_sound);
        // A sound at index >= 1 -> true.
        let w2 = Weapon::load(b"loopSound = \"begin\"\n", &n, &s, &snd).unwrap();
        assert!(w2.loop_sound);
    }

    #[test]
    fn real_bazooka_cfg_loads() {
        let (n, s, snd) = lists();
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/weapons/bazooka.cfg"
        ));
        let w = Weapon::load(bytes, &n, &s, &snd).unwrap();
        assert_eq!(w.name, "BAZOOKA");
        assert_eq!(w.loading_time, 410);
        // bazooka.cfg omits partTrailObj -> "" -> -1.
        assert_eq!(w.part_trail_obj, -1);
    }

    #[test]
    fn objects_aggregate_assigns_index() {
        use crate::tc::TcTypes;
        let types = TcTypes {
            sounds: vec!["bump".into(), "exp2".into()],
            weapons: vec!["bazooka".into()],
            nobjects: vec!["blood".into()],
            sobjects: vec!["small_explosion".into(), "large_explosion".into()],
        };
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let objs = Objects::load(&types, |sub, id| {
            std::fs::read(format!("{root}/{sub}/{id}.cfg"))
        })
        .unwrap();
        assert_eq!(objs.weapons.len(), 1);
        assert_eq!(objs.weapons[0].id, 0);
        assert_eq!(objs.weapons[0].id_str, "bazooka");
        assert_eq!(objs.sobject_types[1].id, 1);
        assert_eq!(objs.sobject_types[1].id_str, "large_explosion");
    }
```

- [ ] **Step 3: Run the assets suite (no regressions)**

Run: `cargo test --manifest-path rust/Cargo.toml -p assets`
Expected: prior assets tests (level/palette/sprite/tc) still PASS plus all new
`object` tests.

- [ ] **Step 4: Commit**

```bash
git add rust/assets/src/object.rs
git commit -m "feat(assets): weapon .cfg loader + Objects aggregate (id=index)"
```

---

### Task 4: object golden — differential test vs real `Common::load`

Prove the parsed object tables match the C++ engine bit-for-bit, including every
resolved cross-ref index and `id = index`.

**Files:**
- Create: `src/tools/oracle_dump/object_dump.cpp`
- Modify: `CMakeLists.txt` (inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block)
- Create: `rust/oracle-tests/gen_object_golden.sh`
- Create: `rust/oracle-tests/golden/object.txt` (generated)
- Create: `rust/oracle-tests/tests/object_golden.rs`

**Interfaces:**
- Consumes: `assets::tc::TcConfig` (1e-1) for the type lists; `assets::object::{Objects, Weapon, NObjectType, SObjectType}` (Tasks 1–3); the real C++ `common.weapons[]` / `common.nobject_types[]` / `common.sobject_types[]`.
- Produces: golden — three `label hash` lines (`weapons`, `nobjects`, `sobjects`).

- [ ] **Step 1: Write the C++ object dumper**

Create `src/tools/oracle_dump/object_dump.cpp`:

```cpp
// Generates golden digests for the Rust object-config differential test by
// running the REAL C++ Common::load (which reads weapons/nobjects/sobjects .cfg
// and resolves cross-references). Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the default build.
// Usage: oracle_dump_object <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <string>
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

void PushU32(std::vector<unsigned char>& b, uint32_t v) {
  b.push_back(static_cast<unsigned char>(v & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 8) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 16) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 24) & 0xff));
}

void PushI32(std::vector<unsigned char>& b, int32_t v) {
  PushU32(b, static_cast<uint32_t>(v));
}

void PushBool(std::vector<unsigned char>& b, bool v) {
  b.push_back(v ? 1 : 0);
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
    std::fprintf(stderr, "usage: oracle_dump_object <tc-dir> <out.txt>\n");
    return 1;
  }
  Common common;
  common.load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  // weapons[] in LoadWeaponConfig read order (common_model.hpp:258-335),
  // prefixed per entry by id (i32) + id_str (string).
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.weapons.size()));
    for (Weapon const& w : common.weapons) {
      PushI32(b, w.id);
      PushStr(b, w.id_str);
      PushStr(b, w.name);
      PushBool(b, w.affect_by_worm);
      PushBool(b, w.shadow);
      PushBool(b, w.laser_sight);
      PushBool(b, w.play_reload_sound);
      PushBool(b, w.worm_explode);
      PushBool(b, w.expl_ground);
      PushBool(b, w.worm_collide);
      PushBool(b, w.collide_with_objects);
      PushBool(b, w.affect_by_explosions);
      PushBool(b, w.loop_anim);
      PushI32(b, w.detect_distance);
      PushI32(b, w.blow_away);
      PushI32(b, w.gravity);
      PushI32(b, w.launch_sound);
      PushBool(b, w.loop_sound);  // bool (int->bool quirk)
      PushI32(b, w.explo_sound);
      PushI32(b, w.speed);
      PushI32(b, w.add_speed);
      PushI32(b, w.distribution);
      PushI32(b, w.parts);
      PushI32(b, w.recoil);
      PushI32(b, w.mult_speed);
      PushI32(b, w.delay);
      PushI32(b, w.loading_time);
      PushI32(b, w.ammo);
      PushI32(b, w.dirt_effect);
      PushI32(b, w.leave_shells);
      PushI32(b, w.leave_shell_delay);
      PushI32(b, w.fire_cone);
      PushI32(b, w.bounce);
      PushI32(b, w.time_to_explo);
      PushI32(b, w.time_to_explo_v);
      PushI32(b, w.hit_damage);
      PushI32(b, w.blood_on_hit);
      PushI32(b, w.start_frame);
      PushI32(b, w.num_frames);
      PushI32(b, w.shot_type);
      PushI32(b, w.color_bullets);
      PushI32(b, w.splinter_amount);
      PushI32(b, w.splinter_colour);
      PushI32(b, w.splinter_type);
      PushI32(b, w.splinter_scatter);
      PushI32(b, w.obj_trail_type);
      PushI32(b, w.obj_trail_delay);
      PushI32(b, w.part_trail_type);
      PushI32(b, w.part_trail_obj);
      PushI32(b, w.part_trail_delay);
      PushI32(b, w.create_on_exp);
      PushBool(b, w.chain_explosion);
    }
    Emit(out, "weapons", b);
  }

  // nobject_types[] in LoadNObjectConfig read order (common_model.hpp:98-137).
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.nobject_types.size()));
    for (NObjectType const& n : common.nobject_types) {
      PushI32(b, n.id);
      PushStr(b, n.id_str);
      PushBool(b, n.worm_explode);
      PushBool(b, n.expl_ground);
      PushBool(b, n.worm_destroy);
      PushBool(b, n.draw_on_map);
      PushBool(b, n.affect_by_explosions);
      PushBool(b, n.blood_trail);
      PushI32(b, n.detect_distance);
      PushI32(b, n.gravity);
      PushI32(b, n.speed);
      PushI32(b, n.speed_v);
      PushI32(b, n.distribution);
      PushI32(b, n.blow_away);
      PushI32(b, n.bounce);
      PushI32(b, n.hit_damage);
      PushI32(b, n.blood_on_hit);
      PushI32(b, n.start_frame);
      PushI32(b, n.num_frames);
      PushI32(b, n.color_bullets);
      PushI32(b, n.create_on_exp);
      PushI32(b, n.dirt_effect);
      PushI32(b, n.splinter_amount);
      PushI32(b, n.splinter_colour);
      PushI32(b, n.splinter_type);
      PushI32(b, n.blood_trail_delay);
      PushI32(b, n.leave_obj);
      PushI32(b, n.leave_obj_delay);
      PushI32(b, n.time_to_explo);
      PushI32(b, n.time_to_explo_v);
    }
    Emit(out, "nobjects", b);
  }

  // sobject_types[] in LoadSObjectConfig read order (common_model.hpp:162-177).
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.sobject_types.size()));
    for (SObjectType const& s : common.sobject_types) {
      PushI32(b, s.id);
      PushStr(b, s.id_str);
      PushBool(b, s.shadow);
      PushI32(b, s.start_sound);
      PushI32(b, s.num_sounds);
      PushI32(b, s.anim_delay);
      PushI32(b, s.start_frame);
      PushI32(b, s.num_frames);
      PushI32(b, s.detect_range);
      PushI32(b, s.damage);
      PushI32(b, s.blow_away);
      PushI32(b, s.shake);
      PushI32(b, s.flash);
      PushI32(b, s.dirt_effect);
    }
    Emit(out, "sobjects", b);
  }

  std::fclose(out);
  return 0;
}
```

- [ ] **Step 2: Register the CMake target**

In `CMakeLists.txt`, inside the existing `if(OPENLIERO_BUILD_ORACLE_DUMP)` block
(after the `oracle_dump_tc` lines, before `endif()`), add:

```cmake
  add_executable(oracle_dump_object src/tools/oracle_dump/object_dump.cpp)
  target_link_libraries(oracle_dump_object PRIVATE game)
```

- [ ] **Step 3: Write the regeneration script**

Create `rust/oracle-tests/gen_object_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/object.txt by running the REAL C++ Common::load (which reads
# weapons/nobjects/sobjects .cfg and resolves cross-references). Needs the full
# C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run in
# the lightweight rust.yml CI. Override PRESET for other platforms (e.g.
# linux-x64). Run from the repo root so the TC dir resolves like the in-tree
# tests do.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "build/$PRESET" --config Release --target oracle_dump_object
(
  cd "$ROOT"
  "build/$PRESET/Release/oracle_dump_object" \
    "data/TC/openliero" \
    "rust/oracle-tests/golden/object.txt"
)
echo "wrote rust/oracle-tests/golden/object.txt"
```

Make it executable:

```bash
chmod +x rust/oracle-tests/gen_object_golden.sh
```

- [ ] **Step 4: Generate the golden**

Run: `bash rust/oracle-tests/gen_object_golden.sh`
Expected: prints `wrote rust/oracle-tests/golden/object.txt`; the file has 3 lines
(`weapons`, `nobjects`, `sobjects`), each with a 16-hex digest.

- [ ] **Step 5: Write the Rust golden test**

Create `rust/oracle-tests/tests/object_golden.rs`:

```rust
//! Differential test for the object-config loaders against the C++ oracle. The
//! golden (one `label hash` line per group) is produced by the real C++
//! `Common::load` (which reads the .cfg files + resolves cross-refs); the Rust
//! loaders must reproduce every FNV-1a digest from the same shipped
//! `data/TC/openliero`. Digest byte layout matches object_dump.cpp exactly (LE
//! ints; bools = 1 byte; strings = u32 LE length + bytes; resolved cross-refs as
//! i32). The encode_* field order mirrors LoadWeaponConfig / LoadNObjectConfig /
//! LoadSObjectConfig.

use std::collections::HashMap;

use assets::object::{NObjectType, Objects, SObjectType, Weapon};
use assets::tc::TcConfig;

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
fn push_bool(b: &mut Vec<u8>, v: bool) {
    b.push(if v { 1 } else { 0 });
}
fn push_str(b: &mut Vec<u8>, s: &str) {
    push_u32(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
}

fn encode_weapon(b: &mut Vec<u8>, w: &Weapon) {
    push_i32(b, w.id);
    push_str(b, &w.id_str);
    push_str(b, &w.name);
    push_bool(b, w.affect_by_worm);
    push_bool(b, w.shadow);
    push_bool(b, w.laser_sight);
    push_bool(b, w.play_reload_sound);
    push_bool(b, w.worm_explode);
    push_bool(b, w.expl_ground);
    push_bool(b, w.worm_collide);
    push_bool(b, w.collide_with_objects);
    push_bool(b, w.affect_by_explosions);
    push_bool(b, w.loop_anim);
    push_i32(b, w.detect_distance);
    push_i32(b, w.blow_away);
    push_i32(b, w.gravity);
    push_i32(b, w.launch_sound);
    push_bool(b, w.loop_sound);
    push_i32(b, w.explo_sound);
    push_i32(b, w.speed);
    push_i32(b, w.add_speed);
    push_i32(b, w.distribution);
    push_i32(b, w.parts);
    push_i32(b, w.recoil);
    push_i32(b, w.mult_speed);
    push_i32(b, w.delay);
    push_i32(b, w.loading_time);
    push_i32(b, w.ammo);
    push_i32(b, w.dirt_effect);
    push_i32(b, w.leave_shells);
    push_i32(b, w.leave_shell_delay);
    push_i32(b, w.fire_cone);
    push_i32(b, w.bounce);
    push_i32(b, w.time_to_explo);
    push_i32(b, w.time_to_explo_v);
    push_i32(b, w.hit_damage);
    push_i32(b, w.blood_on_hit);
    push_i32(b, w.start_frame);
    push_i32(b, w.num_frames);
    push_i32(b, w.shot_type);
    push_i32(b, w.color_bullets);
    push_i32(b, w.splinter_amount);
    push_i32(b, w.splinter_colour);
    push_i32(b, w.splinter_type);
    push_i32(b, w.splinter_scatter);
    push_i32(b, w.obj_trail_type);
    push_i32(b, w.obj_trail_delay);
    push_i32(b, w.part_trail_type);
    push_i32(b, w.part_trail_obj);
    push_i32(b, w.part_trail_delay);
    push_i32(b, w.create_on_exp);
    push_bool(b, w.chain_explosion);
}

fn encode_nobject(b: &mut Vec<u8>, n: &NObjectType) {
    push_i32(b, n.id);
    push_str(b, &n.id_str);
    push_bool(b, n.worm_explode);
    push_bool(b, n.expl_ground);
    push_bool(b, n.worm_destroy);
    push_bool(b, n.draw_on_map);
    push_bool(b, n.affect_by_explosions);
    push_bool(b, n.blood_trail);
    push_i32(b, n.detect_distance);
    push_i32(b, n.gravity);
    push_i32(b, n.speed);
    push_i32(b, n.speed_v);
    push_i32(b, n.distribution);
    push_i32(b, n.blow_away);
    push_i32(b, n.bounce);
    push_i32(b, n.hit_damage);
    push_i32(b, n.blood_on_hit);
    push_i32(b, n.start_frame);
    push_i32(b, n.num_frames);
    push_i32(b, n.color_bullets);
    push_i32(b, n.create_on_exp);
    push_i32(b, n.dirt_effect);
    push_i32(b, n.splinter_amount);
    push_i32(b, n.splinter_colour);
    push_i32(b, n.splinter_type);
    push_i32(b, n.blood_trail_delay);
    push_i32(b, n.leave_obj);
    push_i32(b, n.leave_obj_delay);
    push_i32(b, n.time_to_explo);
    push_i32(b, n.time_to_explo_v);
}

fn encode_sobject(b: &mut Vec<u8>, s: &SObjectType) {
    push_i32(b, s.id);
    push_str(b, &s.id_str);
    push_bool(b, s.shadow);
    push_i32(b, s.start_sound);
    push_i32(b, s.num_sounds);
    push_i32(b, s.anim_delay);
    push_i32(b, s.start_frame);
    push_i32(b, s.num_frames);
    push_i32(b, s.detect_range);
    push_i32(b, s.damage);
    push_i32(b, s.blow_away);
    push_i32(b, s.shake);
    push_i32(b, s.flash);
    push_i32(b, s.dirt_effect);
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
fn object_configs_match_cpp_oracle() {
    let golden = parse_golden(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/object.txt"
    )));

    // 1e-1 gives us the ordered type-name lists (the resolution lists + read order).
    let tc_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/tc.cfg"
    ));
    let tc = TcConfig::load(tc_bytes).expect("tc.cfg parses");

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
    let objs = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{root}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    let want = |k: &str| *golden.get(k).unwrap_or_else(|| panic!("missing golden {k}"));

    {
        let mut b = Vec::new();
        push_u32(&mut b, objs.weapons.len() as u32);
        for w in &objs.weapons {
            encode_weapon(&mut b, w);
        }
        assert_eq!(fnv1a(&b), want("weapons"));
    }
    {
        let mut b = Vec::new();
        push_u32(&mut b, objs.nobject_types.len() as u32);
        for n in &objs.nobject_types {
            encode_nobject(&mut b, n);
        }
        assert_eq!(fnv1a(&b), want("nobjects"));
    }
    {
        let mut b = Vec::new();
        push_u32(&mut b, objs.sobject_types.len() as u32);
        for s in &objs.sobject_types {
            encode_sobject(&mut b, s);
        }
        assert_eq!(fnv1a(&b), want("sobjects"));
    }
}
```

- [ ] **Step 6: Run the full workspace suite**

Run: `cargo test --manifest-path rust/Cargo.toml --workspace`
Expected: ALL tests PASS (sim-core goldens, assets unit incl. object,
level/palette/sprite/tc goldens, object_golden).

- [ ] **Step 7: Commit**

```bash
git add src/tools/oracle_dump/object_dump.cpp CMakeLists.txt \
  rust/oracle-tests/gen_object_golden.sh rust/oracle-tests/golden/object.txt \
  rust/oracle-tests/tests/object_golden.rs
git commit -m "test(oracle): object-config differential test vs C++ Common::load"
```

---

## Self-Review

**Spec coverage:**
- `Weapon` (50 fields + id + id_str, cross-refs resolved) → Task 3 + golden `weapons`. ✓
- `NObjectType` (28 fields, cross-refs) → Task 1 + golden `nobjects`. ✓
- `SObjectType` (12 fields, sound ref) → Task 2 + golden `sobjects`. ✓
- Cross-ref resolution (`obj_ref_from_str` empty→-1/unknown→0/hit; `sound_ref_from_str` empty→-1/unknown→-1/hit) → Task 1 helpers + unit tests + pinned by golden. ✓
- `loopSound` int→bool quirk → Task 3 (`!= 0`) + dedicated unit tests + bool byte in golden. ✓
- `id = array index` (Precompute part) → Task 3 `Objects::load` + golden id prefix. ✓
- Dependency on 1e-1 type lists → `Objects::load(&TcConfig.types, …)`; golden test builds `TcConfig` from real `tc.cfg`. ✓
- Flat-`.cfg` toml-crate risk → Task 0 smoke. ✓
- tc.cfg / WAV / weap_order sort / derived sprites → out of scope (1e-1 / 1e-3 / deferred). ✓

**Placeholder scan:** No TBD/TODO; all Rust + C++ is complete. The Task 0 stub is
fully replaced in Task 1. ✓

**Type/order consistency:** `encode_weapon` (Rust) ↔ weapon push block (C++),
both in `LoadWeaponConfig` order (`common_model.hpp:258–335`), 50 fields + id +
id_str + name; `encode_nobject` ↔ nobject block (`98–137`), 28 fields; `encode_sobject`
↔ sobject block (`162–177`), 12 fields. Field types transcribed from
`weapon.hpp`/`nobject.hpp`/`sobject.hpp` (`fixed = int`). Digest byte layout
(`push_i32`/`push_u32`/`push_bool`/`push_str` = LE ints, 1-byte bools,
u32-len-prefixed strings) identical to the C++ `PushI32`/`PushU32`/`PushBool`/
`PushStr`. Resolved cross-refs hashed as i32 on both sides → pins index
resolution. Count prefix pins array length/order/index space. ✓

**Cross-ref semantics:** `obj_ref_from_str` == `ObjRefFromStr` (`common_model.hpp:24`);
`sound_ref_from_str` == `SoundRefFromStr` (`common_model.hpp:47`, via `SoundIndex`
`common.cpp:574`). Helpers re-implemented locally (cannot touch `tc.rs`); proven by
unit tests; drift vs C++ caught by the golden. ✓

**Accepted divergence:** serde errors on wrong-typed keys where C++ silently keeps
the default; locked contract is the well-formed shipped assets used by the golden.
Same posture as 1e-1. ✓

**Note on RNG-feeding "rendering" fields:** `startFrame`/`numFrames`/`numSounds`/
`colorBullets` feed `game.rand(...)` draws (`weapon.cpp:43,68`, `nobject.cpp:25`,
`sobject.cpp:24`), so they are sim-critical for determinism despite being
sprite/sound-shaped. All are locked by the golden regardless. ✓
