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

    // Real-file coverage independent of the oracle golden: the shipped TC must
    // parse and yield the engine's fixed counts. (Restores the original Task 0
    // smoke test that was lost when this module was rewritten in Task 1.)
    #[test]
    fn real_shipped_tc_cfg_loads() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/tc.cfg"
        ));
        let c = TcConfig::load(bytes).expect("shipped tc.cfg parses");
        assert_eq!(c.materials.len(), MAX_MATERIALS);
        assert_eq!(c.textures.len(), NUM_TEXTURES);
        assert_eq!(c.bonuses.len(), NUM_BONUS_SOBJECTS);
        assert_eq!(c.color_anim.len(), NUM_COLOR_ANIM);
        assert!(!c.types.sounds.is_empty());
        assert!(!c.types.weapons.is_empty());
        // Sound hooks resolve to real indices for the shipped TC.
        assert!(c.sound_hooks.Bump >= 0);
        assert!(c.sound_hooks.Begin >= 0);
    }
}
