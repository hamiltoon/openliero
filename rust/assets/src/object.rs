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
// Shared helper used by sobject/weapon loaders (added in later sub-slice tasks).
#[allow(dead_code)]
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
