//! Step 4½e-1 — the two C++ `inputState.cpp` sub-states the settings menu pushes (design §3.3,
//! §4.5; plan facts 6-9, 12): `InputStringState` (number entry; SAVE SETUP AS…'s name, 4½e-2),
//! the only overlay, and `InfoBoxState` (WEAPON OPTIONS' "no weapons" box, the Rust-only refusal
//! boxes, SAVE SETUP AS…'s reserved-name box). C++ hands each a lambda capturing `gfx`; Rust tags
//! each with a purpose, and the shell runs the continuation inside the overlay's update step
//! (`ui::shell::Shell::frame`). Step 4½f-2 adds the player menu's purposes (NAME, WEAPON n, its
//! number entry, SAVE PROFILE AS…) and the third sub-state, `WaitForKeyState` (PRESS A KEY).

use std::fmt;

use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::blit::{blit_bitmap, draw_rounded_box};
use render::font::Font;
use render::palette::pack_pal32;
use scenario::build::{self, BuildError};
use scenario::settings::{MatchConfig, Settings};
use sim::weapsel::{WeaponSelection, WeapselError};

use super::KeyEvent;
use super::main_menu::{MA_NEW_GAME, MA_RESUME_GAME};
use super::selection::new_game_config;
use crate::keys::{DK_BACKSPACE, DK_ESCAPE, DK_KP_ENTER, DK_RETURN};
use crate::menu::ValueEntry;
use crate::text::{dos_display, utf8_to_dos};

/// Which menu an `IntegerEntry` writes back into (design R-8; Step 4½f-2): the settings menu,
/// or the player menu of player `p` (whose HEALTH, Red, Green and Blue ids 1-4 are also settings
/// ids).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryTarget {
    Settings,
    Player(usize),
}

/// Which `MakeSaveAsState` a name box is (design RD-5; Step 4½f-2): SAVE SETUP AS… (`"Setups"`,
/// `".cfg"`) or SAVE PROFILE AS… of player `p` (`"Profiles"`, `".toml"`). The reserved box's
/// reopen keeps the kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveAsKind {
    Setup,
    Profile(usize),
}

/// What an `InputStringState` edits: C++'s callback, as data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputPurpose {
    /// `IntegerBehavior::OnEnter`'s entry (`integerBehavior.cpp:36-80`) and the menu it writes.
    IntegerEntry {
        entry: ValueEntry,
        target: EntryTarget,
    },
    /// A `MakeSaveAsState` name box (`mainMenuState.cpp:69-91`; SAVE SETUP AS… `:287-311`, Step
    /// 4½e-2; SAVE PROFILE AS… `:348-366`, 4½f-2): its field's `(x, y)`, which the reserved
    /// box's reopen reuses.
    SaveAs { kind: SaveAsKind, x: i32, y: i32 },
    /// The player menu's NAME box (`mainMenuState.cpp:323-347`; 4½f-2).
    WormName { player: usize },
    /// A WEAPON n box and its fuzzy match (`mainMenuState.cpp:390-423`; 4½f-2).
    WeaponFuzzy { player: usize, slot: usize },
}

/// `FilterDigits` (`integerBehavior.cpp:34`): `isdigit(k) ? k : 0`.
pub fn filter_digits(k: u8) -> u8 {
    if k.is_ascii_digit() { k } else { 0 }
}

/// C++ `InputStringState` (`inputState.cpp:13-97`). `buffer` holds DOS (CP437) bytes, as C++'s
/// `std::string` does (`Utf8ToDos`). Not `PartialEq`: `filter` is a function pointer.
#[derive(Clone, Debug)]
pub struct InputStringState {
    pub buffer: Vec<u8>,
    pub max_len: usize,
    pub x: i32,
    pub y: i32,
    pub filter: Option<fn(u8) -> u8>,
    pub prefix: String,
    pub centered: bool,
    pub purpose: InputPurpose,
    pub done: bool,
    pub accepted: bool,
}

impl InputStringState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        initial: &[u8],
        max_len: usize,
        x: i32,
        y: i32,
        filter: Option<fn(u8) -> u8>,
        prefix: &str,
        centered: bool,
        purpose: InputPurpose,
    ) -> InputStringState {
        InputStringState {
            buffer: initial.to_vec(),
            max_len,
            x,
            y,
            filter,
            prefix: prefix.to_string(),
            centered,
            purpose,
            done: false,
            accepted: false,
        }
    }

    /// `HandleEvent`'s key-down arm (`inputState.cpp:30-52`), after `ProcessEvent`: every
    /// key-down, OS repeats included, and even once `done` (fact 6). C++ tests the SDL scancodes
    /// BACKSPACE, RETURN, KP_ENTER and ESCAPE; their DOS codes map one to one.
    pub fn handle_key(&mut self, ev: &KeyEvent) {
        if !ev.down {
            return;
        }
        match ev.dos {
            DK_BACKSPACE => {
                self.buffer.pop();
            }
            DK_RETURN | DK_KP_ENTER => {
                self.accepted = true;
                self.done = true;
            }
            DK_ESCAPE => {
                self.accepted = false;
                self.done = true;
            }
            _ => {}
        }
    }

    /// `HandleEvent`'s `SDL_EVENT_TEXT_INPUT` arm (`inputState.cpp:55-63`): the whole string is
    /// one byte (`Utf8ToDos`, fact 7), appended when non-zero, below `max_len`, and passed (and
    /// replaced) by the filter — C++'s short-circuit order.
    pub fn handle_text(&mut self, s: &str) {
        let mut k = utf8_to_dos(s);
        if k != 0
            && self.buffer.len() < self.max_len
            && self.filter.is_none_or(|f| {
                k = f(k);
                k != 0
            })
        {
            self.buffer.push(k);
        }
    }

    /// `Update`'s test (`inputState.cpp:75-84`): `(accepted, buffer)` once Return, KP Enter or
    /// Esc was seen. The shell then plays `MenuSelect`, runs `ClearKeys` and the continuation,
    /// and pops.
    pub fn is_done(&self) -> Option<(bool, Vec<u8>)> {
        self.done.then(|| (self.accepted, self.buffer.clone()))
    }

    /// `prefix + buffer + '_'` as `Font::DrawString` decodes it (fact 8): a buffer byte below
    /// 0x80 is itself; every byte `Utf8ToDos` makes above it is a lone UTF-8 continuation byte,
    /// which C++ decodes to U+FFFD (drawn as nothing).
    pub fn display(&self) -> String {
        format!("{}{}_", self.prefix, dos_display(&self.buffer))
    }

    /// `Draw` (`inputState.cpp:86-97`): restore the strip under the field from `frozen` — the
    /// width argument is `kClrX + 10 + kWidth` (fact 9), clipped — then the rounded box and the
    /// string in colour 50.
    pub fn draw(&self, surface: &mut Bitmap, frozen: &Bitmap, pal: &Pal32, font: &Font) {
        let s = self.display();
        let w = font.get_dims(&s);
        let adj = if self.centered { w / 2 } else { 0 };
        let clr_x = self.x - 10 - adj;
        blit_bitmap(surface, frozen, clr_x, self.y, clr_x + 10 + w, 8);
        draw_rounded_box(surface, pal, self.x - 2 - adj, self.y, 0, 7, w);
        font.draw_string(surface, pal, &s, self.x - adj, self.y + 1, 50, 1);
    }
}

/// Why the shell refused a NEW GAME or a RESUME (design Q2, plan D5; Rust only). Its box text is
/// `crate::text::refusal_text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Build(BuildError),
    Weapsel(WeapselError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Build(e) => e.fmt(f),
            Refusal::Weapsel(e) => e.fmt(f),
        }
    }
}

/// What the shell tells `MainMenuState` so it can refuse a selection before its fade-out (plan
/// T4 Step 5): whether the current match takes the menu's settings at RESUME (`Match::attached`,
/// D7), the start options that shape NEW GAME, and the TC's weapon count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefusalGate {
    pub attached: bool,
    pub skip_selection: bool,
    pub touch_only: bool,
    pub n_weapons: usize,
}

impl RefusalGate {
    /// The Rust-only refusal of main-menu item `selected` under `settings` (Q2, D5), or `None`.
    ///
    /// - NEW GAME: `build::refuse_follow_ai` (4½f Q2; 4½f-1 D1), then
    ///   `build::validate_for_selection` (`build::validate` on the skip route), then
    ///   `WeaponSelection::validate` over the selection config NEW GAME would build.
    /// - RESUME of an attached match: `validate_for_selection`, whose first check is Holdazone. A
    ///   detached match keeps the settings it started with, so nothing is checked. FollowAI is
    ///   never refused here: CONTROLLER does not reach a running match (design finding 7).
    /// - Anything else (QUIT): never refused.
    pub fn refusal(&self, settings: &Settings, selected: i32) -> Option<Refusal> {
        let cfg = MatchConfig {
            settings: settings.clone(),
            seed: 0,
        };
        match selected {
            MA_NEW_GAME => {
                if let Err(e) = build::refuse_follow_ai(settings) {
                    return Some(Refusal::Build(e));
                }
                let built = if self.skip_selection {
                    build::validate(&cfg, self.n_weapons)
                } else {
                    build::validate_for_selection(&cfg, self.n_weapons)
                };
                if let Err(e) = built {
                    return Some(Refusal::Build(e));
                }
                WeaponSelection::validate(
                    self.n_weapons,
                    2,
                    &new_game_config(settings, self.touch_only),
                )
                .err()
                .map(Refusal::Weapsel)
            }
            MA_RESUME_GAME if self.attached => build::validate_for_selection(&cfg, self.n_weapons)
                .err()
                .map(Refusal::Build),
            _ => None,
        }
    }
}

/// What an `InfoBoxState` reports. Only `Reserved` has an `on_dismiss`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InfoPurpose {
    /// `WeaponMenuState`'s close refusal (`weaponMenuState.cpp:108`).
    NoWeapons,
    /// A Rust-only refusal box (plan D5).
    Refused(Refusal),
    /// A Save-As `NAME '<leaf>' IS RESERVED` box (`mainMenuState.cpp:79-85`; Step 4½e-2): its
    /// `on_dismiss` schedules the name box of the same `kind` again, on what was `typed`, at
    /// `(x, y)`.
    Reserved {
        kind: SaveAsKind,
        typed: Vec<u8>,
        x: i32,
        y: i32,
    },
}

/// C++ `InfoBoxState` (`inputState.cpp:166-217`; fact 12). Not an overlay: it draws alone, over
/// whatever the surface last held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoBoxState {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub clear_screen: bool,
    pub purpose: InfoPurpose,
    pub done: bool,
}

impl InfoBoxState {
    pub fn new(
        text: &str,
        x: i32,
        y: i32,
        clear_screen: bool,
        purpose: InfoPurpose,
    ) -> InfoBoxState {
        InfoBoxState {
            text: text.to_string(),
            x,
            y,
            clear_screen,
            purpose,
            done: false,
        }
    }

    /// `HandleEvent` (`inputState.cpp:177-183`): any key-down, repeats included; never a key-up.
    pub fn handle_key(&mut self, ev: &KeyEvent) {
        if ev.down {
            self.done = true;
        }
    }

    /// `Draw` (`inputState.cpp:201-217`). When clearing: `pal = exepal`, `UpdatePal32`, fill 0 —
    /// the shell's `pal32` for this frame only (the next menu frame's `UpdateMenuPalettes`
    /// rebuilds it). Then the box sized by `GetDims(text, &h)` and the text in colour 6.
    pub fn draw(&self, surface: &mut Bitmap, pal: &mut Pal32, exepal: &Palette, font: &Font) {
        if self.clear_screen {
            *pal = pack_pal32(exepal);
            surface.fill(0, pal);
        }
        let (w, h) = font.get_dims_h(&self.text);
        let cx = self.x - w / 2 - 2;
        let cy = self.y - h / 2 - 2;
        draw_rounded_box(surface, pal, cx, cy, 0, h + 1, w + 1);
        font.draw_string(surface, pal, &self.text, cx + 2, cy + 2, 6, 1);
    }
}

/// The player-menu key row a `WaitForKeyState` binds (Step 4½f-2): `worm_settings[player]`'s
/// control `control` (0..8, `kEyIdx = item id - kPlUp`, DIG = 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyTarget {
    pub player: usize,
    pub control: usize,
}

/// `WaitForKeyState::Draw`'s text (`inputState.cpp:152-162`).
pub const PRESS_A_KEY: &str = "PRESS A KEY";

/// C++ `WaitForKeyState` (`inputState.cpp:100-162`; plan fact 8, D3), pushed by a key row's
/// Enter. Not an overlay: it draws alone, over whatever the surface last held — on its push frame
/// too (design R-3, T0 P2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitForKeyState {
    pub target: KeyTarget,
    /// `result_` once `done_`: the DOS code of the frame's last key-down.
    pub result: Option<u32>,
}

impl WaitForKeyState {
    pub fn new(target: KeyTarget) -> WaitForKeyState {
        WaitForKeyState {
            target,
            result: None,
        }
    }

    /// `HandleEvent`'s key-down arm (`inputState.cpp:111-118`), after `ProcessEvent`: every
    /// key-down, OS repeats included, sets `result_ = SDLToDOSKey(sc)` (`extended_` is
    /// `Settings::kExtensions`, true); the last one of the frame wins. No gamepad arms (Rust has
    /// no pads; the dumper opens none, R2-21).
    pub fn handle_key(&mut self, ev: &KeyEvent) {
        if ev.down {
            self.result = Some(ev.dos);
        }
    }

    /// `Draw` (`inputState.cpp:152-162`): the box `DrawRoundedBox(cx, cy, 0, h + 1, w + 1)` around
    /// (160, 100) and `PRESS A KEY` in colour 50.
    pub fn draw(&self, surface: &mut Bitmap, pal: &Pal32, font: &Font) {
        let (w, h) = font.get_dims_h(PRESS_A_KEY);
        let cx = 160 - w / 2 - 2;
        let cy = 100 - h / 2 - 2;
        draw_rounded_box(surface, pal, cx, cy, 0, h + 1, w + 1);
        font.draw_string(surface, pal, PRESS_A_KEY, cx + 2, cy + 2, 50, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::TypedKey;

    fn font() -> Font {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sprites/font.tga"
        );
        Font::load(&assets::sprite::Tga::load(&std::fs::read(path).unwrap()).unwrap())
    }

    fn ramp() -> Pal32 {
        std::array::from_fn(|i| 0xFF00_0000 | i as u32)
    }

    fn key(dos: u32, down: bool, repeat: bool) -> KeyEvent {
        KeyEvent {
            dos,
            down,
            repeat,
            typed: TypedKey::Sym(0),
        }
    }

    fn entry(initial: &str, max_len: usize) -> InputStringState {
        InputStringState::new(
            initial.as_bytes(),
            max_len,
            100,
            50,
            Some(filter_digits),
            "",
            false,
            InputPurpose::IntegerEntry {
                entry: ValueEntry {
                    item_id: 0,
                    initial: initial.into(),
                    digits: max_len as i32,
                    x: 100,
                    y: 50,
                    min: 0,
                    max: 999,
                    div: 1,
                    percentage: false,
                },
                target: EntryTarget::Settings,
            },
        )
    }

    #[test]
    fn the_digit_filter_and_max_len_gate_typed_text() {
        let mut s = entry("15", 3);
        s.handle_text("a");
        assert_eq!(s.buffer, b"15", "FilterDigits refuses a letter");
        s.handle_text("4");
        assert_eq!(s.buffer, b"154");
        s.handle_text("2");
        assert_eq!(s.buffer, b"154", "max_len: extra digits are dropped");
        s.handle_text("");
        assert_eq!(s.buffer, b"154");
        let mut free = InputStringState {
            filter: None,
            ..entry("", 30)
        };
        for t in ["a", "ab", "å", "7"] {
            free.handle_text(t);
        }
        assert_eq!(
            free.buffer,
            [b'a', b'?', 0x86, b'7'],
            "no filter: every Utf8ToDos byte"
        );
        assert_eq!(filter_digits(b'0'), b'0');
        assert_eq!(
            (
                filter_digits(b'/'),
                filter_digits(b':'),
                filter_digits(0x86)
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn backspace_pops_on_every_key_down_repeats_included() {
        let mut s = entry("15", 3);
        s.handle_key(&key(DK_BACKSPACE, true, false));
        assert_eq!(s.buffer, b"1");
        s.handle_key(&key(DK_BACKSPACE, false, false));
        assert_eq!(s.buffer, b"1", "a key-up does nothing");
        s.handle_key(&key(DK_BACKSPACE, true, true));
        assert_eq!(s.buffer, b"", "an OS repeat is a key-down");
        s.handle_key(&key(DK_BACKSPACE, true, true));
        assert_eq!(s.buffer, b"", "Backspace on empty");
        assert_eq!(s.is_done(), None);
    }

    #[test]
    fn return_accepts_esc_cancels_and_later_events_still_apply() {
        let mut s = entry("7", 3);
        s.handle_key(&key(DK_RETURN, true, false));
        s.handle_text("4");
        assert_eq!(
            s.is_done(),
            Some((true, b"74".to_vec())),
            "text after Return in the same frame still lands (fact 6)"
        );
        let mut s = entry("7", 3);
        s.handle_key(&key(DK_KP_ENTER, true, false));
        assert_eq!(s.is_done(), Some((true, b"7".to_vec())));
        let mut s = entry("7", 3);
        s.handle_key(&key(DK_ESCAPE, true, false));
        assert_eq!(s.is_done(), Some((false, b"7".to_vec())));
        s.handle_key(&key(DK_RETURN, true, false));
        assert_eq!(
            s.is_done(),
            Some((true, b"7".to_vec())),
            "the last key wins"
        );
    }

    #[test]
    fn the_display_maps_high_bytes_to_the_replacement_character() {
        let s = InputStringState {
            buffer: vec![b'a', 0x86, b'b'],
            prefix: "P:".into(),
            ..entry("", 30)
        };
        assert_eq!(s.display(), "P:a\u{FFFD}b_");
        let f = font();
        assert_eq!(
            f.get_dims(&s.display()),
            f.get_dims("P:ab_"),
            "C++ draws no glyph for a lone continuation byte (fact 8)"
        );
    }

    #[test]
    fn the_strip_restore_is_kclrx_plus_10_plus_width_wide() {
        let f = font();
        let pal = ramp();
        let s = entry("42", 3);
        let w = f.get_dims(&s.display());
        let mut frozen = Bitmap::new(320, 200);
        for (i, p) in frozen.pixels.iter_mut().enumerate() {
            *p = 0x8000_0000 | i as u32;
        }
        let mut surface = Bitmap::new(320, 200);
        surface.pixels.fill(0xDEAD_BEEF);
        s.draw(&mut surface, &frozen, &pal, &f);
        let (clr_x, y) = (s.x - 10, s.y);
        let end = clr_x + clr_x + 10 + w; // fact 9: the width argument is kClrX + 10 + kWidth
        assert!(end < 320);
        for row in y..y + 8 {
            assert_eq!(
                surface.get_pixel(clr_x - 1, row),
                0xDEAD_BEEF,
                "left of the strip"
            );
            assert_eq!(
                surface.get_pixel(end - 1, row),
                frozen.get_pixel(end - 1, row),
                "the strip's last column"
            );
            assert_eq!(
                surface.get_pixel(end, row),
                0xDEAD_BEEF,
                "the first pixel outside"
            );
            // Right of the box (its band ends at x + w), past where design §3.3's `10 + w`
            // would stop (clr_x + 10 + w = x + w): still the frozen screen.
            for c in s.x + w + 1..end {
                assert_eq!(
                    surface.get_pixel(c, row),
                    frozen.get_pixel(c, row),
                    "column {c}"
                );
            }
        }
        assert_eq!(surface.get_pixel(clr_x, y + 8), 0xDEAD_BEEF, "8 rows");
        // The box (x - 2, y, 0, 7, w): colour 0 at its band.
        assert_eq!(surface.get_pixel(s.x - 2, y + 1), pal[0]);
        assert_eq!(surface.get_pixel(s.x - 2 + w + 2, y + 5), pal[0]);
        let text = (y + 1..y + 8)
            .flat_map(|r| (s.x..s.x + w).map(move |c| (c, r)))
            .filter(|&(c, r)| surface.get_pixel(c, r) == pal[50])
            .count();
        assert!(text > 0, "the string in colour 50");
    }

    #[test]
    fn a_centered_field_shifts_by_half_its_width_and_clips_at_the_left_edge() {
        let f = font();
        let pal = ramp();
        let s = InputStringState {
            centered: true,
            x: 4,
            ..entry("", 30)
        };
        let w = f.get_dims(&s.display());
        let frozen = Bitmap::new(320, 200);
        let mut surface = Bitmap::new(320, 200);
        surface.pixels.fill(0xDEAD_BEEF);
        s.draw(&mut surface, &frozen, &pal, &f); // kClrX < 0: clipped, no panic
        let clr_x = s.x - 10 - w / 2;
        let end = clr_x + clr_x + 10 + w;
        for c in 0..end.max(0) {
            assert_ne!(
                surface.get_pixel(c, s.y),
                0xDEAD_BEEF,
                "column {c} restored"
            );
        }
    }

    fn black() -> Palette {
        Palette {
            entries: [assets::palette::Color::default(); 256],
        }
    }

    fn info(text: &str, clear: bool) -> InfoBoxState {
        InfoBoxState::new(text, 160, 100, clear, InfoPurpose::NoWeapons)
    }

    #[test]
    fn an_info_box_is_dismissed_by_any_key_down_not_a_key_up() {
        let mut b = info("X", false);
        b.handle_key(&key(57, false, false));
        assert!(!b.done, "a key-up never dismisses");
        b.handle_key(&key(57, true, true));
        assert!(b.done, "an OS repeat does");
        let mut b = info("X", false);
        b.handle_key(&key(DK_RETURN, true, false));
        assert!(b.done);
    }

    #[test]
    fn a_nul_text_is_a_two_line_box_in_colour_6() {
        let f = font();
        let mut pal = ramp();
        let text = "At least one weapon must\0be available in the menu!";
        let b = InfoBoxState::new(text, 223, 68, false, InfoPurpose::NoWeapons);
        let (w, h) = f.get_dims_h(text);
        assert_eq!(h, 16, "GetDims: 8 + 8 per NUL");
        assert_eq!(
            w,
            f.get_dims("be available in the menu!")
                .max(f.get_dims("At least one weapon must"))
        );
        let mut surface = Bitmap::new(320, 200);
        surface.pixels.fill(0xDEAD_BEEF);
        b.draw(&mut surface, &mut pal, &black(), &f);
        assert_eq!(pal, ramp(), "no clear: the palette is untouched");
        let (cx, cy) = (223 - w / 2 - 2, 68 - h / 2 - 2);
        // DrawRoundedBox(cx, cy, 0, h + 1, w + 1): the band (cx, cy+1, w+4, h-1), open corners.
        assert_eq!(surface.get_pixel(cx, cy), 0xDEAD_BEEF, "an open corner");
        assert_eq!(surface.get_pixel(cx + 1, cy), pal[0]);
        assert_eq!(surface.get_pixel(cx, cy + 1), pal[0]);
        assert_eq!(
            surface.get_pixel(cx + w + 3, cy + h - 1),
            pal[0],
            "the band's far corner"
        );
        assert_eq!(surface.get_pixel(cx + w + 4, cy + 1), 0xDEAD_BEEF);
        assert_eq!(
            surface.get_pixel(cx + w + 1, cy + h),
            pal[0],
            "the bottom row"
        );
        assert_eq!(
            surface.get_pixel(cx, cy + h),
            0xDEAD_BEEF,
            "the other open corner"
        );
        assert_eq!(surface.get_pixel(cx + 1, cy + h + 1), 0xDEAD_BEEF);
        let colour_rows = |lo: i32, hi: i32| {
            (lo..hi)
                .flat_map(|r| (cx..cx + w + 4).map(move |c| (c, r)))
                .filter(|&(c, r)| surface.get_pixel(c, r) == pal[6])
                .count()
        };
        assert!(colour_rows(cy + 2, cy + 10) > 0, "line one in colour 6");
        assert!(colour_rows(cy + 10, cy + 18) > 0, "line two, 8 px lower");
    }

    #[test]
    fn clear_screen_swaps_in_the_exepal_and_fills_black() {
        let f = font();
        let mut pal = ramp();
        let mut exepal = black();
        for (i, e) in exepal.entries.iter_mut().enumerate() {
            e.r = (i % 64) as u8;
            e.g = 7;
        }
        let b = info("HI", true);
        let mut surface = Bitmap::new(320, 200);
        surface.pixels.fill(0xDEAD_BEEF);
        b.draw(&mut surface, &mut pal, &exepal, &f);
        assert_eq!(pal, pack_pal32(&exepal), "UpdatePal32 over exepal");
        assert_eq!(
            surface.get_pixel(0, 0),
            pal[0],
            "Fill(bmp, 0) through the new palette"
        );
    }

    #[test]
    fn wait_for_key_takes_the_last_key_down_of_the_frame_repeats_included() {
        let mut w = WaitForKeyState::new(KeyTarget {
            player: 0,
            control: 0,
        });
        w.handle_key(&key(46, false, false));
        assert_eq!(w.result, None, "a key-up never binds");
        w.handle_key(&key(46, true, false));
        w.handle_key(&key(47, true, false));
        assert_eq!(w.result, Some(47), "C + V down: V (T0 P2)");
        let mut w = WaitForKeyState::new(KeyTarget {
            player: 1,
            control: 3,
        });
        w.handle_key(&key(45, true, true));
        assert_eq!(w.result, Some(45), "an OS repeat binds");
    }

    #[test]
    fn the_press_a_key_box_is_centred_on_160_100() {
        let f = font();
        let pal = ramp();
        let w = WaitForKeyState::new(KeyTarget {
            player: 0,
            control: 0,
        });
        let mut got = Bitmap::new(320, 200);
        got.pixels.fill(0xDEAD_BEEF);
        w.draw(&mut got, &pal, &f);
        let (tw, th) = f.get_dims_h(PRESS_A_KEY);
        assert_eq!(th, 8);
        let (cx, cy) = (160 - tw / 2 - 2, 100 - th / 2 - 2);
        let mut want = Bitmap::new(320, 200);
        want.pixels.fill(0xDEAD_BEEF);
        // DrawRoundedBox(cx, cy, 0, h + 1, w + 1).
        want.fill_rect(cx, cy + 1, tw + 4, th - 1, 0, &pal);
        want.fill_rect(cx + 1, cy, tw + 2, 1, 0, &pal);
        want.fill_rect(cx + 1, cy + th, tw + 2, 1, 0, &pal);
        f.draw_string(&mut want, &pal, PRESS_A_KEY, cx + 2, cy + 2, 50, 1);
        assert_eq!(got, want);
        let changed: Vec<(i32, i32)> = (0..320)
            .flat_map(|c| (0..200).map(move |r| (c, r)))
            .filter(|&(c, r)| got.get_pixel(c, r) != 0xDEAD_BEEF)
            .collect();
        let (x0, x1) = (cx, cx + tw + 3);
        let (y0, y1) = (cy, cy + th);
        assert!(
            changed
                .iter()
                .all(|&(c, r)| (x0..=x1).contains(&c) && (y0..=y1).contains(&r)),
            "only inside ({x0}..={x1}, {y0}..={y1})"
        );
        assert!(
            changed.contains(&(cx + 1, cy)) && !changed.contains(&(cx, cy)),
            "open corners"
        );
        assert!(
            (x0..=x1)
                .flat_map(|c| (y0..=y1).map(move |r| (c, r)))
                .any(|(c, r)| got.get_pixel(c, r) == pal[50]),
            "the text in colour 50"
        );
    }
}
