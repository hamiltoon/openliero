//! Texts the C++ hardcodes in `Texts::Texts()` (`common.cpp:205-225`, design finding 3 — not
//! read from `tc.cfg`), the `text.cpp` time formatters, and `GetBasename(GetLeaf(..))`.

use std::path::Path;

use assets::object::{Objects, Weapon};
use assets::palette::Palette;
use assets::sprite::Tga;
use assets::tc::TcConfig;
use scenario::build::BuildError;
use sim::weapsel::WeapselError;

use crate::menu::MenuHooks;
use crate::shell::overlay::Refusal;

/// `Texts::game_modes` (`common.cpp:206-209`), indexed by `Settings::game_mode`.
pub const GAME_MODES: [&str; 4] = [
    "Kill'em All",
    "Game of Tag",
    "Holdazone",
    "Scales of Justice",
];
/// `Texts::onoff` (`common.cpp:211-212`).
pub const ONOFF: [&str; 2] = ["OFF", "ON"];
/// `Texts::weap_states` (`common.cpp:222-224`), indexed by a `Settings::weap_table` entry: the
/// WEAPON OPTIONS values (Step 4½e-1).
pub const WEAP_STATES: [&str; 3] = ["Menu", "Bonus", "Banned"];

/// The Rust-only refusal boxes' texts (plan D5; Q2), drawn at (160, 100) without clearing. A NUL
/// breaks the line, as in the TC's own texts. Zero enabled weapons reuses the TC's `NoWeaps`.
pub fn refusal_text(r: &Refusal, tc: &UiTc) -> String {
    match r {
        Refusal::Build(BuildError::HoldazoneUnsupported) => {
            "HOLDAZONE IS NOT\0SUPPORTED YET".into()
        }
        Refusal::Build(BuildError::AsymmetricHealth { .. }) => {
            "BOTH PLAYERS NEED\0THE SAME HEALTH".into()
        }
        Refusal::Weapsel(WeapselError::NoWeaponsEnabled) => tc.no_weaps.clone(),
        _ => "THIS SETUP CANNOT\0BE PLAYED YET".into(),
    }
}

/// `Utf8ToDos` (`text.cpp:99-118`; plan fact 7): ONE whole `SDL_EVENT_TEXT_INPUT` string to one
/// byte. A 1-byte string is that byte; the six two-byte sequences å ä ö Å Ä Ö are CP437
/// 0x86 0x84 0x94 0x8f 0x8e 0x99; anything else — a multi-character string included — is `'?'`
/// (C++ matches the table on the first two bytes only). SDL never sends an empty string; Rust
/// gives it 0, which `InputStringState` ignores.
pub fn utf8_to_dos(s: &str) -> u8 {
    let b = s.as_bytes();
    match b.len() {
        0 => 0,
        1 => b[0],
        _ => {
            const TABLE: [[u8; 3]; 6] = [
                [0xc3, 0xa5, 0x86], // å
                [0xc3, 0xa4, 0x84], // ä
                [0xc3, 0xb6, 0x94], // ö
                [0xc3, 0x85, 0x8f], // Å
                [0xc3, 0x84, 0x8e], // Ä
                [0xc3, 0x96, 0x99], // Ö
            ];
            TABLE
                .iter()
                .find(|t| t[0] == b[0] && t[1] == b[1])
                .map_or(b'?', |t| t[2])
        }
    }
}

/// C++ `'0' + n` stored into a `char` (`text.cpp`): the digit for 0..=9, and the same byte
/// arithmetic (wrapping) outside it.
fn digit(n: i32) -> char {
    (b'0' as i32 + n) as u8 as char
}

/// `TimeToString(sec)` (`text.cpp:5-16`): `M M : S S` with the first digit `sec / 600`.
pub fn time_to_string(sec: i32) -> String {
    [
        digit(sec / 600),
        digit((sec % 600) / 60),
        ':',
        digit((sec % 60) / 10),
        digit(sec % 10),
    ]
    .iter()
    .collect()
}

/// `TimeToStringEx(ms, force_hours, force_minutes)` (`text.cpp:18-45`).
pub fn time_to_string_ex(ms: i32, force_hours: bool, force_minutes: bool) -> String {
    let mut ms = ms;
    let mut s = String::new();
    if ms >= 6_000_000 || force_hours {
        s.push(digit(ms / 6_000_000));
        ms %= 6_000_000;
    }
    if ms >= 60_000 || force_minutes {
        s.push(digit(ms / 600_000));
        ms %= 600_000;
        s.push(digit(ms / 60_000));
        ms %= 60_000;
        s.push(':');
    }
    s.push(digit(ms / 10_000));
    ms %= 10_000;
    s.push(digit(ms / 1000));
    ms %= 1000;
    s.push('.');
    s.push(digit(ms / 100));
    ms %= 100;
    s.push(digit(ms / 10));
    s
}

/// `TimeToStringFrames(frames)` (`text.cpp:47-49`): 14 ms per frame.
pub fn time_to_string_frames(frames: i32) -> String {
    time_to_string_ex(frames * 14, false, false)
}

/// `GetBasename(GetLeaf(path))` (`filesystem.cpp:38-54`).
pub fn leaf_basename(path: &str) -> &str {
    let leaf = path.rsplit(['/', '\\']).next().unwrap_or(path);
    leaf.rsplit_once('.').map_or(leaf, |(b, _)| b)
}

/// What the menus read from the TC: `common.s[..]` strings (`tc.cfg [texts]`), `common.c[..]`
/// constants, and `common.sound_hook[..]` as sample ids (`tc.cfg [sounds]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiTc {
    pub copyright2: String,
    pub random2: String,
    pub regen_level: String,
    pub reload_level: String,
    pub blood_limit: i32,
    pub blood_step_up: i32,
    pub hooks: MenuHooks,
    /// `SoundBegin` (`game.cpp:500-503`, played by `StartGame`).
    pub begin: i32,
    /// Step 4½e-1: `LS(Weapon)`, `LS(Availability)` (WEAPON OPTIONS' headers) and `LS(NoWeaps)`
    /// (its close-refusal box; the TC text holds a NUL line break, `tc.cfg:258`).
    pub weapon: String,
    pub availability: String,
    pub no_weaps: String,
    /// `common.exepal`: `small.tga`'s palette (`InfoBoxState::Draw`'s `clear_screen` palette).
    pub exepal: Palette,
    /// `common.weapons[weap_order[i]].name` for `i` in `0..40`: WEAPON OPTIONS' rows.
    pub weapon_names: Vec<String>,
    /// `Common::weap_order` (`sim::weapsel::weap_order`): row `i` edits `weap_table[weap_order[i]]`.
    pub weap_order: Vec<usize>,
}

impl UiTc {
    /// The menu texts of `tc`, the TC's `weapons` (in `tc.types` order) and its `exepal`.
    pub fn from_tc(tc: &TcConfig, weapons: &[Weapon], exepal: Palette) -> UiTc {
        let weap_order = sim::weapsel::weap_order(weapons);
        UiTc {
            weapon: tc.texts.Weapon.clone(),
            availability: tc.texts.Availability.clone(),
            no_weaps: tc.texts.NoWeaps.clone(),
            exepal,
            weapon_names: weap_order
                .iter()
                .map(|&i| weapons[i].name.clone())
                .collect(),
            weap_order,
            copyright2: tc.texts.Copyright2.clone(),
            random2: tc.texts.Random2.clone(),
            regen_level: tc.texts.RegenLevel.clone(),
            reload_level: tc.texts.ReloadLevel.clone(),
            blood_limit: tc.constants.BloodLimit,
            blood_step_up: tc.constants.BloodStepUp,
            hooks: MenuHooks {
                move_up: tc.sound_hooks.MenuMoveUp,
                move_down: tc.sound_hooks.MenuMoveDown,
                select: tc.sound_hooks.MenuSelect,
            },
            begin: tc.sound_hooks.Begin,
        }
    }

    pub fn load(tc_root: &Path) -> UiTc {
        use scenario::assets::read_asset;
        let tc = TcConfig::load(&read_asset(tc_root, "tc.cfg")).expect("tc.cfg parses");
        let objects = Objects::load(&tc.types, |sub, id| {
            Ok(read_asset(tc_root, &format!("{sub}/{id}.cfg")))
        })
        .expect("object configs load");
        let small = Tga::load(&read_asset(tc_root, "sprites/small.tga")).expect("small.tga parses");
        UiTc::from_tc(&tc, &objects.weapons, small.palette)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_to_string_is_minutes_colon_seconds() {
        // text.cpp:5-16: '0' + sec/600, '0' + (sec%600)/60, ':', tens, units.
        assert_eq!(time_to_string(600), "10:00", "Settings(): time_to_lose");
        assert_eq!(time_to_string(3599), "59:59");
        assert_eq!(time_to_string(60), "01:00");
        assert_eq!(time_to_string(3600), "60:00");
        assert_eq!(time_to_string(45), "00:45");
    }

    #[test]
    fn time_to_string_frames_is_ms_at_14_per_frame() {
        // text.cpp:18-49: TimeToStringEx(frames * 14, false, false).
        assert_eq!(time_to_string_frames(70), "00.98");
        assert_eq!(
            time_to_string_frames(4286),
            "01:00.00",
            "60004 ms: minutes appear"
        );
        // An exact hour: the hours digit, then (ms == 0, no force) the minutes block is skipped.
        assert_eq!(time_to_string_ex(6_000_000, false, false), "100.00");
        assert_eq!(time_to_string_ex(1234, true, true), "000:01.23");
    }

    #[test]
    fn leaf_basename_is_getbasename_of_getleaf() {
        // filesystem.cpp:38-54: the leaf after the last '/' or '\', up to its last '.'.
        assert_eq!(leaf_basename("Levels/water_stage.lev"), "water_stage");
        assert_eq!(leaf_basename("C:\\a\\b.c.lev"), "b.c");
        assert_eq!(leaf_basename("noext"), "noext");
        assert_eq!(leaf_basename(""), "");
        assert_eq!(leaf_basename("data/Setups/liero.cfg"), "liero");
    }

    #[test]
    fn ui_tc_is_read_from_the_tc() {
        let tc = UiTc::load(std::path::Path::new(scenario::paths::TC_ROOT));
        assert_eq!(
            tc.copyright2, "Liero v1.33 (c) Mets\u{e4}nEl\u{e4}met 1998,1999",
            "tc.cfg:244"
        );
        assert_eq!(
            (
                tc.random2.as_str(),
                tc.regen_level.as_str(),
                tc.reload_level.as_str()
            ),
            ("Random", "REGENERATE LEVEL", "RELOAD LEVEL")
        );
        assert_eq!((tc.blood_limit, tc.blood_step_up), (500, 25));
        assert_eq!(
            (
                tc.hooks.move_up,
                tc.hooks.move_down,
                tc.hooks.select,
                tc.begin
            ),
            (25, 26, 27, 22),
            "tc.cfg [sounds]"
        );
    }

    #[test]
    fn utf8_to_dos_is_one_byte_per_text_event() {
        // text.cpp:99-118 (plan fact 7).
        assert_eq!(utf8_to_dos("a"), b'a');
        assert_eq!(utf8_to_dos("7"), b'7');
        let swedish: Vec<u8> = ["å", "ä", "ö", "Å", "Ä", "Ö"]
            .iter()
            .map(|s| utf8_to_dos(s))
            .collect();
        assert_eq!(swedish, [0x86, 0x84, 0x94, 0x8f, 0x8e, 0x99]);
        assert_eq!(utf8_to_dos("ab"), b'?', "a multi-character string");
        assert_eq!(
            utf8_to_dos("é"),
            b'?',
            "a two-byte sequence outside the table"
        );
        assert_eq!(utf8_to_dos("€"), b'?', "a three-byte sequence");
        assert_eq!(utf8_to_dos(""), 0);
    }

    #[test]
    fn ui_tc_carries_the_weapon_options_texts_and_rows() {
        let root = std::path::Path::new(scenario::paths::TC_ROOT);
        let tc = UiTc::load(root);
        assert_eq!(
            WEAP_STATES,
            ["Menu", "Bonus", "Banned"],
            "common.cpp:222-224"
        );
        assert_eq!(
            (tc.weapon.as_str(), tc.availability.as_str()),
            ("Weapon", "Availability")
        );
        assert_eq!(
            tc.no_weaps.matches('\0').count(),
            1,
            "tc.cfg:258: one NUL line break"
        );
        assert_eq!(
            tc.no_weaps,
            "At least one weapon must\u{0}be available in the menu!"
        );
        let cfg = TcConfig::load(&scenario::assets::read_asset(root, "tc.cfg")).unwrap();
        let objects = Objects::load(&cfg.types, |sub, id| {
            Ok(scenario::assets::read_asset(
                root,
                &format!("{sub}/{id}.cfg"),
            ))
        })
        .unwrap();
        assert_eq!(tc.weapon_names.len(), 40);
        assert_eq!(tc.weap_order, sim::weapsel::weap_order(&objects.weapons));
        for (i, name) in tc.weapon_names.iter().enumerate() {
            assert_eq!(*name, objects.weapons[tc.weap_order[i]].name);
        }
        assert!(
            tc.weapon_names.windows(2).all(|w| w[0] < w[1]),
            "weap_order sorts by name"
        );
        let small = Tga::load(&scenario::assets::read_asset(root, "sprites/small.tga")).unwrap();
        assert_eq!(tc.exepal, small.palette);
    }

    #[test]
    fn the_refusal_boxes_carry_the_d5_texts() {
        use crate::shell::overlay::Refusal;
        let tc = UiTc::load(std::path::Path::new(scenario::paths::TC_ROOT));
        let t = |r: Refusal| refusal_text(&r, &tc);
        assert_eq!(
            t(Refusal::Build(BuildError::HoldazoneUnsupported)),
            "HOLDAZONE IS NOT\0SUPPORTED YET"
        );
        assert_eq!(
            t(Refusal::Build(BuildError::AsymmetricHealth {
                p1: 100,
                p2: 50
            })),
            "BOTH PLAYERS NEED\0THE SAME HEALTH"
        );
        assert_eq!(
            t(Refusal::Weapsel(WeapselError::NoWeaponsEnabled)),
            tc.no_weaps
        );
        for r in [
            Refusal::Build(BuildError::InvalidBloodParticleMax(0)),
            Refusal::Build(BuildError::InvalidWeapon {
                worm: 0,
                slot: 1,
                value: 41,
            }),
            Refusal::Weapsel(WeapselError::InvalidPick {
                worm: 1,
                slot: 0,
                value: 41,
            }),
        ] {
            assert_eq!(t(r), "THIS SETUP CANNOT\0BE PLAYED YET");
        }
    }
}
