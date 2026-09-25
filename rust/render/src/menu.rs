//! The text arm of C++ `MenuItem::Draw` (`menuItem.cpp:6-42`), the item recipe every Liero menu
//! draws with. Step 4½c pulls it forward from 4½d (design §5 option A); the `has_value` arm and
//! the `Menu` framework (visibility, scrolling, the scrollbar, type-to-search) stay 4½d's.

use crate::bitmap::{Bitmap, Pal32};
use crate::blit::draw_rounded_box;
use crate::font::Font;

/// The selected item's text colour (`menuItem.cpp:32`), the head of the rotated range 168..174.
pub const SELECTED_COLOUR: i32 = 168;

/// `MenuItem::color` / `dis_colour` (`menuItem.hpp:11-16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemColours {
    pub color: u8,
    pub dis_colour: u8,
}

/// `MenuItem::Draw` with no value (`has_value == false`): a selected item gets a rounded box of
/// its text width, an unselected one a colour-0 shadow at (+3, +2); then the text at (+2, +1)
/// in `dis_colour` if disabled, else 168 if selected, else `color`. `centered` shifts left by
/// half the width (`:10-12`).
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
    let wid = font.get_dims(text);
    let x = if centered { x - (wid >> 1) } else { x };
    if selected {
        draw_rounded_box(bmp, pal, x, y, 0, 7, wid);
    } else {
        font.draw_string(bmp, pal, text, x + 3, y + 2, 0, 1);
    }
    let c = if disabled {
        colours.dis_colour as i32
    } else if selected {
        SELECTED_COLOUR
    } else {
        colours.color as i32
    };
    font.draw_string(bmp, pal, text, x + 2, y + 1, c, 1);
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
}
