//! Step 4½e-1 — the two C++ `inputState.cpp` sub-states the settings menu pushes (design §3.3,
//! §4.5; plan facts 6-9, 12): `InputStringState` (number entry now, SAVE SETUP AS… in 4½e-2), the
//! only overlay, and `InfoBoxState` (WEAPON OPTIONS' "no weapons" box, the Rust-only refusal
//! boxes). C++ hands each a lambda capturing `gfx`; Rust tags each with a purpose, and the shell
//! runs the continuation inside the overlay's update step (`ui::shell::Shell::frame`).

use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::blit::{blit_bitmap, draw_rounded_box};
use render::font::Font;
use render::palette::pack_pal32;
use scenario::build::BuildError;
use sim::weapsel::WeapselError;

use super::KeyEvent;
use crate::keys::{DK_BACKSPACE, DK_ESCAPE, DK_KP_ENTER, DK_RETURN};
use crate::menu::ValueEntry;
use crate::text::utf8_to_dos;

/// What an `InputStringState` edits: C++'s callback, as data. 4½e-2 adds `SaveSetupAs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputPurpose {
    /// `IntegerBehavior::OnEnter`'s entry (`integerBehavior.cpp:36-80`).
    IntegerEntry(ValueEntry),
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
        let mut s = self.prefix.clone();
        s.extend(
            self.buffer
                .iter()
                .map(|&b| if b < 0x80 { b as char } else { '\u{FFFD}' }),
        );
        s.push('_');
        s
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

/// Why the shell refused a NEW GAME or a RESUME (design Q2, plan D5): T4 adds the check and the
/// box texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Build(BuildError),
    Weapsel(WeapselError),
}

/// What an `InfoBoxState` reports. Neither e-1 purpose has an `on_dismiss` (4½e-2's
/// `Reserved` chains back into SAVE SETUP AS…).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InfoPurpose {
    /// `WeaponMenuState`'s close refusal (`weaponMenuState.cpp:108`).
    NoWeapons,
    /// A Rust-only refusal box (plan D5).
    Refused(Refusal),
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
            InputPurpose::IntegerEntry(ValueEntry {
                item_id: 0,
                initial: initial.into(),
                digits: max_len as i32,
                x: 100,
                y: 50,
                min: 0,
                max: 999,
                div: 1,
                percentage: false,
            }),
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
}
