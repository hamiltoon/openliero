//! Texts the C++ hardcodes in `Texts::Texts()` (`common.cpp:205-225`, design finding 3 — not
//! read from `tc.cfg`), the `text.cpp` time formatters, and `GetBasename(GetLeaf(..))`.

use std::path::Path;

use assets::object::{Objects, Weapon};
use assets::palette::Palette;
use assets::sprite::Tga;
use assets::tc::TcConfig;
use scenario::build::BuildError;
use sim::weapsel::WeapselError;

use crate::keys::MAX_DOS_KEY;
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

/// `Texts::controllers` (`common.cpp:214-216`), indexed by `WormSettings::controller`: the player
/// menu's CONTROLLER values (Step 4½f-2).
pub const CONTROLLERS: [&str; 3] = ["Human", "CPU", "AI"];

/// `Texts::key_names` (`common.cpp:25-203`) verbatim, indexed by DOS key code: the player menu's
/// key rows. The source's spellings stay (`"Left Crtl"` at 29), UTF-8 as in the source (`"Å"` at
/// 26, drawn through the font's CP437 map), and `""` at 89 (`APPLICATION`) and every gap.
pub const KEY_NAMES: [&str; MAX_DOS_KEY as usize] = [
    "",
    "Esc",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "0",
    "+",
    "`",
    "Backspace",
    "Tab",
    "Q",
    "W",
    "E",
    "R",
    "T",
    "Y",
    "U",
    "I",
    "O",
    "P",
    "Å",
    "^",
    "Enter",
    "Left Crtl",
    "A",
    "S",
    "D",
    "F",
    "G",
    "H",
    "J",
    "K",
    "L",
    "Ö",
    "Ä",
    "½",
    "Left Shift",
    "'",
    "Z",
    "X",
    "C",
    "V",
    "B",
    "N",
    "M",
    ",",
    ".",
    "-",
    "Right Shift",
    "* (Pad)",
    "Left Alt",
    "",
    "Caps Lock",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "Num Lock",
    "Scroll Lock",
    "7 (Pad)",
    "8 (Pad)",
    "9 (Pad)",
    "- (Pad)",
    "4 (Pad)",
    "5 (Pad)",
    "6 (Pad)",
    "+ (Pad)",
    "1 (Pad)",
    "2 (Pad)",
    "3 (Pad)",
    "0 (Pad)",
    ", (Pad)",
    "",
    "",
    "<",
    "F11",
    "F12",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "Enter (Pad)",
    "Right Ctrl",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "Print Screen",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "/ (Pad)",
    "",
    "Print Screen",
    "Right Alt",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "Home",
    "Up",
    "Page Up",
    "",
    "Left",
    "",
    "Right",
    "",
    "End",
    "Down",
    "Page Down",
    "Insert",
    "Delete",
    "",
    "",
    "",
    "",
    "",
];

/// `kJoyKeysStart`, `kMaxJoyButtons` (`keys.hpp:15-18`).
const JOY_KEYS_START: u32 = 512;
const MAX_JOY_BUTTONS: u32 = 32;
/// `WormSettingsExtensions::kGamepadAxisBase` (`worm.hpp:74-77`).
const GAMEPAD_AXIS_BASE: u32 = 100;

/// `Gfx::GetKeyName(key)` (`gfx.cpp:828-840`): the table below `kMaxDosKey`, `J<pad>_<button>`
/// from `kJoyKeysStart` on, `""` between.
pub fn get_key_name(key: u32) -> String {
    if key < MAX_DOS_KEY {
        KEY_NAMES[key as usize].to_string()
    } else if key >= JOY_KEYS_START {
        let k = key - JOY_KEYS_START;
        format!("J{}_{}", k / MAX_JOY_BUTTONS, k % MAX_JOY_BUTTONS)
    } else {
        String::new()
    }
}

/// `Gfx::GetGamepadKeyName(gamepad_key)` (`gfx.cpp:842-859`): from `kGamepadAxisBase` an axis
/// (`LX LY RX RY LT RT`, then `A<n>`) with `+`, or `-` for an odd offset; below 15 a button
/// name; else `Btn<n>`.
pub fn get_gamepad_key_name(gamepad_key: u32) -> String {
    if gamepad_key >= GAMEPAD_AXIS_BASE {
        const AXIS_NAMES: [&str; 6] = ["LX", "LY", "RX", "RY", "LT", "RT"];
        let axis = (gamepad_key - GAMEPAD_AXIS_BASE) / 2;
        let negative = (gamepad_key - GAMEPAD_AXIS_BASE) % 2 != 0;
        let name = match AXIS_NAMES.get(axis as usize) {
            Some(n) => n.to_string(),
            None => format!("A{axis}"),
        };
        return name + if negative { "-" } else { "+" };
    }
    const BUTTON_NAMES: [&str; 15] = [
        "A", "B", "X", "Y", "Back", "Guide", "Start", "LS", "RS", "LB", "RB", "Up", "Down", "Left",
        "Right",
    ];
    match BUTTON_NAMES.get(gamepad_key as usize) {
        Some(n) => n.to_string(),
        None => format!("Btn{gamepad_key}"),
    }
}

/// The Rust-only refusal boxes' texts (plan D5; Q2; 4½f Q2 for FollowAI), drawn at (160, 100)
/// without clearing. A NUL breaks the line, as in the TC's own texts. Zero enabled weapons reuses
/// the TC's `NoWeaps`.
pub fn refusal_text(r: &Refusal, tc: &UiTc) -> String {
    match r {
        Refusal::Build(BuildError::HoldazoneUnsupported) => {
            "HOLDAZONE IS NOT\0SUPPORTED YET".into()
        }
        Refusal::Build(BuildError::FollowAiUnsupported { .. }) => {
            "AI PLAYERS ARE NOT\0SUPPORTED YET".into()
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

/// A DOS byte string as `Font::DrawString` decodes it (e-1 plan fact 8): a byte below 0x80 is
/// itself; every byte `Utf8ToDos` makes above it is a lone UTF-8 continuation byte, which C++
/// decodes to U+FFFD (drawn as nothing).
pub fn dos_display(b: &[u8]) -> String {
    b.iter()
        .map(|&c| if c < 0x80 { c as char } else { '\u{FFFD}' })
        .collect()
}

/// `cp437.cpp:11-44` `kHighHalf`: the Unicode codepoint of each CP437 byte 0x80..=0xFF (the
/// table `render::font` draws with).
#[rustfmt::skip]
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å',
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ',
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»',
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐',
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧',
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀',
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩',
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{A0}',
];

/// An entry buffer as a file name (Step 4½e-2, SAVE SETUP AS…; plan D7, design §4.8): a buffer
/// that is valid UTF-8 as it stands — ASCII, or the untouched initial name — is kept; any other
/// is decoded byte by byte, ASCII as-is and bytes ≥ 0x80 through CP437 (what `Utf8ToDos` typed).
/// Rust only: C++ uses the bytes as the path.
pub fn dos_to_text(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b
            .iter()
            .map(|&c| {
                if c < 0x80 {
                    c as char
                } else {
                    CP437_HIGH[usize::from(c - 0x80)]
                }
            })
            .collect(),
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

/// `SafeToUpper` (`text.cpp:51`): `std::toupper` in the "C" locale over the unsigned byte —
/// ASCII only.
fn ci_upper(b: u8) -> u8 {
    b.to_ascii_uppercase()
}

/// `CiCompare(a, b)` (`text.cpp:53-65`): equal lengths and equal bytes under `ci_upper`.
pub fn ci_compare(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| ci_upper(x) == ci_upper(y))
}

/// `CiStartsWith(text, starts_with)` (`text.cpp:67-79`).
pub fn ci_starts_with(text: &str, starts_with: &str) -> bool {
    starts_with.len() <= text.len()
        && text
            .bytes()
            .zip(starts_with.bytes())
            .all(|(x, y)| ci_upper(x) == ci_upper(y))
}

/// `CiLess(a, b)` (`text.cpp:81-96`): byte by byte under `ci_upper` (unsigned), a proper
/// prefix first (plan fact 4).
pub fn ci_less(a: &str, b: &str) -> bool {
    let b = b.as_bytes();
    for (i, &x) in a.as_bytes().iter().enumerate() {
        let Some(&y) = b.get(i) else {
            return false;
        };
        let (x, y) = (ci_upper(x), ci_upper(y));
        if x != y {
            return x < y;
        }
    }
    b.len() > a.len()
}

/// `JoinPath(root, leaf)` (`filesystem.cpp:283-288`): a `/` between them unless `root` is empty
/// or already ends in `/` or `\`.
pub fn join_path(root: &str, leaf: &str) -> String {
    if !root.is_empty() && !root.ends_with(['/', '\\']) {
        format!("{root}/{leaf}")
    } else {
        format!("{root}{leaf}")
    }
}

/// `GetBasename(path)` (`filesystem.cpp:47-54`): up to the last `.` (the whole path without one).
pub fn get_basename(path: &str) -> &str {
    path.rsplit_once('.').map_or(path, |(b, _)| b)
}

/// `GetExtension(path)` (`filesystem.cpp:56-63`): after the last `.`, or `""` without one.
pub fn get_extension(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(_, e)| e)
}

/// `Levenshtein(s1, s2)` (`mainMenuState.cpp:26-54`), loop for loop: the
/// `(s2len + 1) × (s1len + 1)` matrix, rows over `s2`, `MIN3` picking the first of equal
/// minima, each byte compared through `std::tolower` (ASCII only; a byte ≥ 0x80 is compared as
/// itself — C++ passes a negative `char`, undefined, and no gate types one). The slices are what
/// `strlen` sees.
pub fn levenshtein(s1: &[u8], s2: &[u8]) -> u32 {
    fn min3(a: u32, b: u32, c: u32) -> u32 {
        if a < b {
            if a < c { a } else { c }
        } else if b < c {
            b
        } else {
            c
        }
    }
    let (s1len, s2len) = (s1.len(), s2.len());
    let w = s1len + 1;
    let mut matrix = vec![0u32; w * (s2len + 1)];
    for x in 1..=s2len {
        matrix[x * w] = matrix[(x - 1) * w] + 1;
    }
    for y in 1..=s1len {
        matrix[y] = matrix[y - 1] + 1;
    }
    for x in 1..=s2len {
        for y in 1..=s1len {
            let c = u32::from(s1[y - 1].to_ascii_lowercase() != s2[x - 1].to_ascii_lowercase());
            matrix[x * w + y] = min3(
                matrix[(x - 1) * w + y] + 1,
                matrix[x * w + y - 1] + 1,
                matrix[(x - 1) * w + y - 1] + c,
            );
        }
    }
    matrix[s2len * w + s1len]
}

/// WEAPON n's typed name (`mainMenuState.cpp:398-417`): the 1-based index into `names` (the
/// TC's weapon names in `weap_order`) with the smallest `Levenshtein(name, typed) / len(name)`,
/// the first of equal ratios; `current` when nothing is taken. The caller never passes an empty
/// `typed` (C++ tests `!result.empty()` first).
///
/// C++ compares `double` quotients with a strict `<` from `DBL_MAX`; this compares
/// `d * l_best < d_best * l` in integers (finding 14), which is the same order: IEEE division is
/// correctly rounded and monotonic, so equal ratios give equal doubles, and two different ratios
/// whose lengths are below 2^16 (the box takes 10 bytes) differ by at least `1 / (l * l_best)`,
/// more than 2^-32, while doubles below 2^16 are spaced at most 2^-37 apart, so their quotients
/// keep their order. A name of length 0 gives C++ `inf` or `NaN`,
/// which is never `<` anything, so it is never taken. The distance runs over the name up to its
/// first NUL (`c_str()`), the length is the whole name's (`length()`).
pub fn weapon_fuzzy_match(names: &[String], typed: &[u8], current: u32) -> u32 {
    fn c_str(b: &[u8]) -> &[u8] {
        b.iter().position(|&c| c == 0).map_or(b, |n| &b[..n])
    }
    let typed = c_str(typed);
    let mut best = current;
    let mut best_ratio: Option<(u64, u64)> = None;
    for (i, name) in (1u32..).zip(names) {
        let l = name.len() as u64;
        if l == 0 {
            continue;
        }
        let d = u64::from(levenshtein(c_str(name.as_bytes()), typed));
        if best_ratio.is_none_or(|(bd, bl)| d * bl < bd * l) {
            best = i;
            best_ratio = Some((d, l));
        }
    }
    best
}

/// What the menus read from the TC: `common.s[..]` strings (`tc.cfg [texts]`), `common.c[..]`
/// constants, and `common.sound_hook[..]` as sample ids (`tc.cfg [sounds]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiTc {
    pub copyright2: String,
    /// Step 4½e-2: `LS(Random)` (`"[RANDOM]"`, `tc.cfg:239`), the level selector's first row,
    /// and `LS(SelLevel)` (`"Select level:"`, `tc.cfg:255`), its title.
    pub random: String,
    pub sel_level: String,
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
            random: tc.texts.Random.clone(),
            sel_level: tc.texts.SelLevel.clone(),
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
    fn dos_bytes_decode_for_display_and_for_file_names() {
        assert_eq!(dos_display(b"ab"), "ab");
        assert_eq!(dos_display(&[b'a', 0x86]), "a\u{FFFD}");
        for t in ["å", "ä", "ö", "Å", "Ä", "Ö"] {
            assert_eq!(dos_to_text(&[utf8_to_dos(t)]), t, "Utf8ToDos round trip");
        }
        assert_eq!(dos_to_text(&[b'm', 0x94, b'r', b'k']), "mörk");
        assert_eq!(
            dos_to_text("mörk".as_bytes()),
            "mörk",
            "valid UTF-8 is kept"
        );
        assert_eq!(dos_to_text(&[0x80, 0xE1, 0xFF]), "Çß\u{A0}");
        assert_eq!(dos_to_text(b"mine"), "mine");
    }

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
    fn the_cpp_string_helpers() {
        // text.cpp:51-96 (plan fact 4): ASCII toupper per byte, a proper prefix first.
        assert!(ci_less("a", "B") && !ci_less("B", "a"));
        assert!(!ci_less("ab", "a") && ci_less("a", "ab"));
        assert!(ci_less("", "a") && !ci_less("", "") && !ci_less("a", "A"));
        assert!(
            ci_less("Z", "_") && ci_less("z", "_"),
            "0x5F sorts after 'Z'"
        );
        assert!(ci_less("Zeta", "\u{e4}"), "bytes >= 0x80 compare unsigned");
        assert!(ci_compare("alpha.LEV", "ALPHA.lev") && !ci_compare("a", "ab"));
        assert!(ci_compare("", ""));
        assert!(ci_starts_with("./user/TC", "./USER") && ci_starts_with("x", ""));
        assert!(!ci_starts_with("./us", "./user"));
        // filesystem.cpp:47-63, :283-288.
        assert_eq!(join_path("./user", "TC"), "./user/TC");
        assert_eq!(join_path("/", "x"), "/x");
        assert_eq!(join_path("C:\\", "x"), "C:\\x");
        assert_eq!(join_path("", "x"), "x");
        assert_eq!(get_basename("a.b.lev"), "a.b");
        assert_eq!(get_basename("noext"), "noext");
        assert_eq!(get_basename(".hidden.lev"), ".hidden");
        assert_eq!(get_extension("noext"), "");
        assert_eq!(get_extension("alpha.LEV"), "LEV");
        assert_eq!(get_extension("a.b.lev"), "lev");
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
            (tc.random.as_str(), tc.sel_level.as_str()),
            ("[RANDOM]", "Select level:"),
            "tc.cfg:239, :255"
        );
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
        for worm in 0..2 {
            assert_eq!(
                t(Refusal::Build(BuildError::FollowAiUnsupported { worm })),
                "AI PLAYERS ARE NOT\0SUPPORTED YET"
            );
        }
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

    /// Addendum T0 P0: `Texts::key_names[i]` as the real C++ printed it (`PROBE_VECTORS`,
    /// `$S/t0f2/vectors_rel.txt`), `<i>:<hex bytes>`, `-` for the empty string.
    const KEY_NAMES_HEX: &str = "
        0:- 1:457363 2:31 3:32 4:33 5:34 6:35 7:36
        8:37 9:38 10:39 11:30 12:2b 13:60 14:4261636b7370616365 15:546162
        16:51 17:57 18:45 19:52 20:54 21:59 22:55 23:49
        24:4f 25:50 26:c385 27:5e 28:456e746572 29:4c656674204372746c 30:41 31:53
        32:44 33:46 34:47 35:48 36:4a 37:4b 38:4c 39:c396
        40:c384 41:c2bd 42:4c656674205368696674 43:27 44:5a 45:58 46:43 47:56
        48:42 49:4e 50:4d 51:2c 52:2e 53:2d 54:5269676874205368696674 55:2a202850616429
        56:4c65667420416c74 57:- 58:43617073204c6f636b 59:4631 60:4632 61:4633 62:4634 63:4635
        64:4636 65:4637 66:4638 67:4639 68:463130 69:4e756d204c6f636b 70:5363726f6c6c204c6f636b 71:37202850616429
        72:38202850616429 73:39202850616429 74:2d202850616429 75:34202850616429 76:35202850616429 77:36202850616429 78:2b202850616429 79:31202850616429
        80:32202850616429 81:33202850616429 82:30202850616429 83:2c202850616429 84:- 85:- 86:3c 87:463131
        88:463132 89:- 90:- 91:- 92:- 93:- 94:- 95:-
        96:- 97:- 98:- 99:- 100:- 101:- 102:- 103:-
        104:- 105:- 106:- 107:- 108:- 109:- 110:- 111:-
        112:- 113:- 114:- 115:- 116:456e746572202850616429 117:5269676874204374726c 118:- 119:-
        120:- 121:- 122:- 123:- 124:- 125:- 126:- 127:-
        128:- 129:- 130:5072696e742053637265656e 131:- 132:- 133:- 134:- 135:-
        136:- 137:- 138:- 139:- 140:- 141:2f202850616429 142:- 143:5072696e742053637265656e
        144:526967687420416c74 145:- 146:- 147:- 148:- 149:- 150:- 151:-
        152:- 153:- 154:- 155:- 156:- 157:- 158:- 159:486f6d65
        160:5570 161:50616765205570 162:- 163:4c656674 164:- 165:5269676874 166:- 167:456e64
        168:446f776e 169:5061676520446f776e 170:496e73657274 171:44656c657465 172:- 173:- 174:- 175:-
        176:-
    ";

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return Vec::new();
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn key_names_are_the_cpp_table_byte_for_byte() {
        // common.cpp:25-203, checked against Addendum T0 P0's hex table, all 177.
        let entries: Vec<&str> = KEY_NAMES_HEX.split_whitespace().collect();
        assert_eq!(entries.len(), 177);
        assert_eq!(KEY_NAMES.len(), 177);
        for (i, e) in entries.iter().enumerate() {
            let (idx, hex) = e.split_once(':').unwrap();
            assert_eq!(idx.parse::<usize>().unwrap(), i);
            assert_eq!(KEY_NAMES[i].as_bytes(), unhex(hex), "key_names[{i}]");
        }
        // The spellings pitfall 16 pins: never "fixed".
        assert_eq!(KEY_NAMES[29], "Left Crtl");
        assert_eq!(KEY_NAMES[117], "Right Ctrl");
        assert_eq!(KEY_NAMES[26].as_bytes(), [0xc3, 0x85], "Å");
        assert_eq!((KEY_NAMES[12], KEY_NAMES[13]), ("+", "`"));
        assert_eq!(KEY_NAMES[89], "", "APPLICATION's DOS code: a blank row");
    }

    #[test]
    fn get_key_name_is_the_table_then_the_joystick_names() {
        // gfx.cpp:828-840; Addendum T0 P0's GetKeyName vectors.
        let cases: [(u32, &str); 18] = [
            (0, ""),
            (1, "Esc"),
            (12, "+"),
            (13, "`"),
            (26, "Å"),
            (27, "^"),
            (29, "Left Crtl"),
            (41, "½"),
            (89, ""),
            (176, ""),
            (177, ""),
            (300, ""),
            (511, ""),
            (512, "J0_0"),
            (513, "J0_1"),
            (543, "J0_31"),
            (544, "J1_0"),
            (1000, "J15_8"),
        ];
        for (k, name) in cases {
            assert_eq!(get_key_name(k), name, "GetKeyName({k})");
        }
    }

    #[test]
    fn get_gamepad_key_name_is_the_buttons_then_the_axes() {
        // gfx.cpp:842-859; Addendum T0 P0's GetGamepadKeyName vectors.
        let cases: [(u32, &str); 33] = [
            (0, "A"),
            (1, "B"),
            (2, "X"),
            (3, "Y"),
            (4, "Back"),
            (5, "Guide"),
            (6, "Start"),
            (7, "LS"),
            (8, "RS"),
            (9, "LB"),
            (10, "RB"),
            (11, "Up"),
            (12, "Down"),
            (13, "Left"),
            (14, "Right"),
            (15, "Btn15"),
            (99, "Btn99"),
            (100, "LX+"),
            (101, "LX-"),
            (102, "LY+"),
            (103, "LY-"),
            (104, "RX+"),
            (105, "RX-"),
            (106, "RY+"),
            (107, "RY-"),
            (108, "LT+"),
            (109, "LT-"),
            (110, "RT+"),
            (111, "RT-"),
            (112, "A6+"),
            (113, "A6-"),
            (114, "A7+"),
            (130, "A15+"),
        ];
        for (k, name) in cases {
            assert_eq!(get_gamepad_key_name(k), name, "GetGamepadKeyName({k})");
        }
    }

    #[test]
    fn the_controller_names() {
        // common.cpp:214-216; Addendum T0 P0.
        assert_eq!(CONTROLLERS, ["Human", "CPU", "AI"]);
    }

    /// Addendum T0 P0: `Levenshtein(name, typed)` from the verbatim copy of
    /// `mainMenuState.cpp:26-54`, for the 40 names in `weap_order` and the probe strings of
    /// `LEV_PROBES`.
    const LEV_PROBES: [&str; 8] = [
        "bazoka",
        "LSR",
        "a",
        "zzzzzzzzzz",
        "BIG NUKE",
        "big nuke",
        "",
        "la",
    ];

    fn lev_table() -> [(&'static str, [u32; 8]); 40] {
        [
            ("BAZOOKA", [1, 7, 6, 9, 6, 6, 7, 6]),
            ("BIG NUKE", [6, 8, 8, 10, 0, 0, 8, 8]),
            ("BLASTER", [5, 4, 6, 10, 7, 7, 7, 5]),
            ("BOOBY TRAP", [8, 9, 9, 10, 8, 8, 10, 9]),
            ("BOUNCY LARPA", [10, 10, 11, 12, 10, 10, 12, 10]),
            ("BOUNCY MINE", [10, 11, 11, 11, 8, 8, 11, 11]),
            ("CANNON", [5, 6, 5, 10, 7, 7, 6, 5]),
            ("CHAINGUN", [7, 8, 7, 10, 7, 7, 8, 7]),
            ("CHIQUITA BOMB", [11, 13, 12, 13, 11, 11, 13, 12]),
            ("CLUSTER BOMB", [11, 9, 12, 12, 11, 11, 12, 11]),
            ("CRACKLER", [7, 6, 7, 10, 8, 8, 8, 7]),
            ("DART", [5, 3, 3, 10, 8, 8, 4, 3]),
            ("DIRTBALL", [7, 7, 7, 10, 7, 7, 8, 7]),
            ("DOOMSDAY", [7, 7, 7, 10, 8, 8, 8, 7]),
            ("EXPLOSIVES", [9, 8, 10, 10, 9, 9, 10, 9]),
            ("FAN", [5, 3, 2, 10, 7, 7, 3, 2]),
            ("FLAMER", [6, 4, 5, 10, 8, 8, 6, 4]),
            ("FLOAT MINE", [9, 9, 9, 10, 8, 8, 10, 8]),
            ("GAUSS GUN", [8, 8, 8, 10, 8, 8, 9, 8]),
            ("GRASSHOPPER", [9, 9, 10, 11, 10, 10, 11, 10]),
            ("GREENBALL", [8, 9, 8, 10, 8, 8, 9, 8]),
            ("GRENADE", [7, 7, 6, 10, 6, 6, 7, 6]),
            ("HANDGUN", [6, 7, 6, 10, 7, 7, 7, 6]),
            ("HELLRAIDER", [9, 8, 9, 10, 9, 9, 10, 8]),
            ("LARPA", [4, 3, 4, 10, 8, 8, 5, 3]),
            ("LASER", [5, 2, 4, 10, 8, 8, 5, 3]),
            ("MINE", [6, 4, 4, 10, 5, 5, 4, 4]),
            ("MINI NUKE", [8, 9, 9, 10, 3, 3, 9, 9]),
            ("MINI ROCKETS", [10, 11, 12, 12, 8, 8, 12, 12]),
            ("MINIGUN", [7, 7, 7, 10, 6, 6, 7, 7]),
            ("MISSILE", [7, 6, 7, 10, 6, 6, 7, 6]),
            ("NAPALM", [5, 6, 5, 10, 8, 8, 6, 5]),
            ("RB RAMPAGE", [8, 9, 9, 10, 8, 8, 10, 9]),
            ("RIFLE", [6, 5, 5, 10, 6, 6, 5, 4]),
            ("SHOTGUN", [7, 7, 7, 10, 7, 7, 7, 7]),
            ("SPIKEBALLS", [9, 9, 9, 10, 9, 9, 10, 9]),
            ("SUPER SHOTGUN", [12, 12, 13, 13, 12, 12, 13, 13]),
            ("UZI", [5, 3, 3, 9, 7, 7, 3, 3]),
            ("WINCHESTER", [10, 8, 10, 10, 8, 8, 10, 10]),
            ("ZIMM", [5, 4, 4, 9, 7, 7, 4, 4]),
        ]
    }

    #[test]
    fn levenshtein_is_the_cpp_copy() {
        let tc = UiTc::load(std::path::Path::new(scenario::paths::TC_ROOT));
        let table = lev_table();
        let names: Vec<&str> = table.iter().map(|(n, _)| *n).collect();
        assert_eq!(tc.weapon_names, names, "the TC's weap_order names");
        for (name, row) in table {
            for (typed, d) in LEV_PROBES.iter().zip(row) {
                assert_eq!(
                    levenshtein(name.as_bytes(), typed.as_bytes()),
                    d,
                    "Levenshtein({name:?}, {typed:?})"
                );
            }
        }
        assert_eq!(levenshtein(b"kitten", b"sitting"), 3);
        assert_eq!(levenshtein(b"ABC", b"abc"), 0, "std::tolower per byte");
        assert_eq!(levenshtein(b"a", b""), 1);
    }

    #[test]
    fn weapon_fuzzy_match_is_the_real_menus_pick() {
        // Addendum T0 P0, `v_fuzzy`: WEAPON 2's value before and after each box, in order.
        let tc = UiTc::load(std::path::Path::new(scenario::paths::TC_ROOT));
        let cases: [(&str, u32, u32); 7] = [
            ("bazoka", 1, 1),
            ("LSR", 1, 26),
            ("a", 26, 16),
            // A 14-way tie at ratio 1: the lowest index (BOOBY TRAP) wins.
            ("zzzzzzzzzz", 16, 4),
            ("BIG NUKE", 4, 2),
            // LARPA 3/5 ties LASER 3/5: the lower index wins.
            ("la", 2, 25),
            ("bazoka", 25, 1),
        ];
        for (typed, current, want) in cases {
            assert_eq!(
                weapon_fuzzy_match(&tc.weapon_names, typed.as_bytes(), current),
                want,
                "{typed:?} from {current}"
            );
        }
        assert_eq!(
            weapon_fuzzy_match(&[], b"x", 7),
            7,
            "no names: the current value"
        );
    }
}
