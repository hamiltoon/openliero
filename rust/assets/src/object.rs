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
// Shared helper used by the sobject and weapon loaders.
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

// ===== Weapon =====

/// Parsed `weapons/<id_str>.cfg`. Mirrors `Weapon` (`src/game/weapon.hpp:12`) +
/// `LoadWeaponConfig`. Note `loop_sound` is a `bool` reproducing C++'s int->bool
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
    /// (subdir in {"weapons","nobjects","sobjects"}) — no coupling to an IO
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
}
