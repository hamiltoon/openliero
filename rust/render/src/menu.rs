//! C++ `MenuItem::Draw` (`menuItem.cpp:6-42`), the item recipe every Liero menu draws with, the
//! `Menu::Draw` scrollbar (`menu.cpp:107-124`) and the menu palette (`gfx.cpp:986-990`). Step 4½c
//! pulled the text arm forward; Step 4½d adds the value arm, the scrollbar and `menu_palette`. The
//! `Menu` framework itself lives in `ui::menu`.

use assets::palette::Palette;

use crate::bitmap::{Bitmap, Pal32};
use crate::blit::draw_rounded_box;
use crate::font::Font;
use crate::palette::{pack_pal32, rotate_from, set_worm_colour};

/// The menu water rotation (`gfx.cpp:987`, `weapsel.cpp:22`).
pub const ROTATE_FROM: i32 = 168;
pub const ROTATE_TO: i32 = 174;

/// The selected item's text colour (`menuItem.cpp:32`), the head of the rotated range 168..174.
pub const SELECTED_COLOUR: i32 = 168;

/// `MenuItem::color` / `dis_colour` (`menuItem.hpp:11-16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemColours {
    pub color: u8,
    pub dis_colour: u8,
}

/// `MenuItem::Draw` with no value (`has_value == false`); see [`draw_item_value`].
#[allow(clippy::too_many_arguments)]
pub fn draw_item(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    selected: bool,
    disabled: bool,
    centered: bool,
    colours: ItemColours,
) {
    draw_item_value(
        bmp, pal, font, text, None, x, y, selected, disabled, centered, 0, colours,
    );
}

/// `MenuItem::Draw` (`menuItem.cpp:6-42`). `value` is `Some` iff `has_value`. A selected item
/// gets a rounded box of its text width, plus one of the value's width centred on
/// `x + value_offset_x`; an unselected one gets colour-0 shadows at (+3, +2). Then the text at
/// (+2, +1) and the value at `x + value_offset_x - vw/2 + 2`, both in `dis_colour` if disabled,
/// else 168 if selected, else `color` — C++'s `c = disabled ? 7 : 168` sits inside the
/// `else if (selected)` arm, which only runs when not disabled, so it is always 168 (finding 15).
/// `centered` shifts `x` left by half the text width first (`:10-12`).
#[allow(clippy::too_many_arguments)]
pub fn draw_item_value(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    text: &str,
    value: Option<&str>,
    x: i32,
    y: i32,
    selected: bool,
    disabled: bool,
    centered: bool,
    value_offset_x: i32,
    colours: ItemColours,
) {
    let wid = font.get_dims(text);
    let vwid = value.map_or(0, |v| font.get_dims(v));
    let x = if centered { x - (wid >> 1) } else { x };
    let vx = x + value_offset_x - (vwid >> 1);
    if selected {
        draw_rounded_box(bmp, pal, x, y, 0, 7, wid);
        if value.is_some() {
            draw_rounded_box(bmp, pal, vx, y, 0, 7, vwid);
        }
    } else {
        font.draw_string(bmp, pal, text, x + 3, y + 2, 0, 1);
        if let Some(v) = value {
            font.draw_string(bmp, pal, v, vx + 3, y + 2, 0, 1);
        }
    }
    let c = if disabled {
        colours.dis_colour as i32
    } else if selected {
        SELECTED_COLOUR
    } else {
        colours.color as i32
    };
    font.draw_string(bmp, pal, text, x + 2, y + 1, c, 1);
    if let Some(v) = value {
        font.draw_string(bmp, pal, v, vx + 2, y + 1, c, 1);
    }
}

/// `Menu::Draw`'s scrollbar (`menu.cpp:107-124`), drawn by the caller iff
/// `visible_item_count > height`: glyphs 22 (up) and 23 (down) — passed straight to `DrawChar`,
/// i.e. CP437 bytes 24/25 — with a colour-0 shadow at (x-6, ..) under colour 50 at (x-7, ..),
/// then the tab as two clip-clamped `FillRect`s, colour 0 at (x-7, tab_y+9) under colour 7 at
/// (x-8, tab_y+8). All C++ `int` arithmetic.
#[allow(clippy::too_many_arguments)]
pub fn draw_scrollbar(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    x: i32,
    y: i32,
    height: i32,
    item_height: i32,
    top_item: i32,
    visible_item_count: i32,
) {
    let menu_height = height * item_height + 1;
    font.draw_char(bmp, pal, 22, x - 6, y + 2, 0, 1);
    font.draw_char(bmp, pal, 22, x - 7, y + 1, 50, 1);
    font.draw_char(bmp, pal, 23, x - 6, y + menu_height - 7, 0, 1);
    font.draw_char(bmp, pal, 23, x - 7, y + menu_height - 8, 50, 1);
    let bar = menu_height - 17;
    let tab = (height * bar / visible_item_count).min(bar).max(0);
    let tab_y = y + top_item * bar / visible_item_count;
    bmp.fill_rect(x - 7, tab_y + 9, 7, tab, 0, pal);
    bmp.fill_rect(x - 8, tab_y + 8, 7, tab, 7, pal);
}

/// `Gfx::UpdateMenuPalettes`'s palette (`gfx.cpp:987-993`): `Origpal`, `RotateFrom(Origpal, 168,
/// 174, menu_cycles)`, then `SetWormColours(settings)` for worms 0 and 1 (`palette.cpp:114-118`).
/// `menu_cycles` is `Gfx::menu_cycles` (`unsigned`). Not the weapsel palette: that one has no
/// worm step (plan-time fact 5).
pub fn menu_palette(origpal: &Palette, menu_cycles: u32, worm_rgb: [[i32; 3]; 2]) -> Pal32 {
    let mut pal = origpal.clone();
    rotate_from(&mut pal, origpal, ROTATE_FROM, ROTATE_TO, menu_cycles);
    for (i, rgb) in worm_rgb.iter().enumerate() {
        set_worm_colour(&mut pal, i, *rgb);
    }
    pack_pal32(&pal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::Pal32;

    fn real_font() -> Font {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sprites/font.tga"
        );
        Font::load(&assets::sprite::Tga::load(&std::fs::read(path).unwrap()).unwrap())
    }

    fn ramp_pal() -> Pal32 {
        std::array::from_fn(|i| 0xFF00_0000 | i as u32)
    }

    const DONE: ItemColours = ItemColours {
        color: 10,
        dis_colour: 9,
    };

    fn item(selected: bool, disabled: bool, centered: bool, x: i32) -> Bitmap {
        let mut bmp = Bitmap::new(80, 12);
        draw_item(
            &mut bmp,
            &ramp_pal(),
            &real_font(),
            "DONE!",
            x,
            2,
            selected,
            disabled,
            centered,
            DONE,
        );
        bmp
    }

    #[test]
    fn a_selected_item_is_a_box_then_colour_168() {
        let pal = ramp_pal();
        let bmp = item(true, false, false, 4);
        assert_eq!(
            bmp.get_pixel(4, 3),
            pal[0],
            "the box band starts at x (menuItem.cpp:15)"
        );
        assert!(bmp.pixels.contains(&pal[168]) && !bmp.pixels.contains(&pal[10]));
    }

    #[test]
    fn an_unselected_item_is_a_colour_0_shadow_then_its_colour() {
        let pal = ramp_pal();
        let bmp = item(false, false, false, 4);
        assert!(bmp.pixels.contains(&pal[10]) && bmp.pixels.contains(&pal[0]));
        assert!(!bmp.pixels.contains(&pal[168]));
        assert_eq!(bmp.get_pixel(4, 3), 0, "no box: (x, y+1) is untouched");
    }

    #[test]
    fn disabled_wins_over_selected() {
        let pal = ramp_pal();
        let bmp = item(true, true, false, 4);
        assert!(bmp.pixels.contains(&pal[9]) && !bmp.pixels.contains(&pal[168]));
    }

    #[test]
    fn centered_shifts_left_by_half_the_width() {
        let half = real_font().get_dims("DONE!") >> 1;
        assert_eq!(
            item(false, false, true, 40),
            item(false, false, false, 40 - half)
        );
    }

    #[test]
    fn draw_item_is_the_value_arm_without_a_value() {
        for (sel, dis, cen) in [
            (false, false, false),
            (true, false, true),
            (true, true, false),
        ] {
            let mut a = Bitmap::new(80, 12);
            let mut b = Bitmap::new(80, 12);
            draw_item(
                &mut a,
                &ramp_pal(),
                &real_font(),
                "DONE!",
                40,
                2,
                sel,
                dis,
                cen,
                DONE,
            );
            draw_item_value(
                &mut b,
                &ramp_pal(),
                &real_font(),
                "DONE!",
                None,
                40,
                2,
                sel,
                dis,
                cen,
                77,
                DONE,
            );
            assert_eq!(a, b);
        }
    }

    #[test]
    fn a_selected_value_gets_its_own_box_centred_on_value_offset_x() {
        // menuItem.cpp:17-19: DrawRoundedBox(x + voff - vw/2, y, 0, 7, vw).
        let pal = ramp_pal();
        let font = real_font();
        let mut bmp = Bitmap::new(200, 12);
        draw_item_value(
            &mut bmp,
            &pal,
            &font,
            "LIVES",
            Some("15"),
            4,
            2,
            true,
            false,
            false,
            100,
            DONE,
        );
        let vx = 4 + 100 - (font.get_dims("15") >> 1);
        assert_eq!(
            bmp.get_pixel(vx, 3),
            pal[0],
            "the value box band starts at vx"
        );
        assert_eq!(bmp.get_pixel(vx - 1, 3), 0, "and not before");
    }

    #[test]
    fn an_unselected_value_gets_a_shadow_then_the_item_colour() {
        let pal = ramp_pal();
        let mut bmp = Bitmap::new(200, 12);
        draw_item_value(
            &mut bmp,
            &pal,
            &real_font(),
            "MAP",
            Some("ON"),
            4,
            2,
            false,
            true,
            false,
            100,
            ItemColours {
                color: 48,
                dis_colour: 7,
            },
        );
        assert!(
            bmp.pixels.contains(&pal[7]) && !bmp.pixels.contains(&pal[48]),
            "disabled: dis_colour"
        );
        assert!(bmp.pixels[100..].contains(&pal[0]), "the colour-0 shadows");
    }

    #[test]
    fn the_scrollbar_is_two_arrows_and_a_two_tone_tab() {
        // menu.cpp:107-124: height 15, 20 visible, top 5 -> bar 104, tab 78, tab_y = y + 26.
        let pal = ramp_pal();
        let font = real_font();
        let mut bmp = Bitmap::new(320, 200);
        draw_scrollbar(&mut bmp, &pal, &font, 178, 20, 15, 8, 5, 20);
        let (x, tab_y) = (178, 20 + 5 * 104 / 20);
        assert_eq!(
            bmp.get_pixel(x - 8, tab_y + 8),
            pal[7],
            "the light face at (x-8, tab_y+8)"
        );
        assert_eq!(
            bmp.get_pixel(x - 2, tab_y + 8 + 77),
            pal[7],
            "78 rows tall, 7 wide"
        );
        assert_eq!(
            bmp.get_pixel(x - 1, tab_y + 9 + 77),
            pal[0],
            "the colour-0 shadow at +1,+1"
        );
        assert_eq!(
            bmp.get_pixel(x - 8, tab_y + 8 + 78),
            0,
            "nothing below the tab"
        );
        assert!(bmp.pixels.contains(&pal[50]), "the arrows are colour 50");
        let mut none = Bitmap::new(320, 200);
        draw_scrollbar(&mut none, &pal, &font, 178, 20, 15, 8, 0, 15);
        assert!(
            none.pixels.contains(&pal[7]),
            "the caller decides visibility; the tab still draws"
        );
    }

    #[test]
    fn the_menu_palette_rotates_168_to_174_then_sets_the_worm_ramps() {
        use assets::palette::{Color, Palette};
        let mut p = Palette {
            entries: [Color::default(); 256],
        };
        for (i, e) in p.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        let rgb = [[104, 104, 252], [60, 172, 60]];
        let got = menu_palette(&p, 2, rgb);
        let mut want = p.clone();
        crate::palette::rotate_from(&mut want, &p, 168, 174, 2);
        crate::palette::set_worm_colour(&mut want, 0, rgb[0]);
        crate::palette::set_worm_colour(&mut want, 1, rgb[1]);
        assert_eq!(got, crate::palette::pack_pal32(&want));
        // With the ramps already in origpal (after Game::Focus) it equals the weapsel palette.
        let mut focused = p.clone();
        crate::palette::set_worm_colour(&mut focused, 0, rgb[0]);
        crate::palette::set_worm_colour(&mut focused, 1, rgb[1]);
        assert_eq!(
            menu_palette(&focused, 9, rgb),
            crate::weapsel::weapsel_palette(&focused, 9)
        );
    }
}
