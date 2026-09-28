//! The weapon-selection screen, normal viewports (`weapsel.cpp:160-209`). Step 4½c pulls it
//! forward from 4½d (design §5 option A; John's Q1 ruling: pixel-exact now). Bevy-free, so
//! `shot` and the C++ frame gate (`oracle-tests/tests/render_weapsel_golden.rs`, plan Addendum
//! A1) draw it headlessly.
//!
//! C++ first rebuilds the weapsel palette (`UpdateWeapselPalette`, `:20-24`: `Origpal`, then
//! `RotateFrom(Origpal, 168, 174, menu_cycles)`). On the first draw it caches a FROZEN ARGB
//! copy of `game.Draw` (level, HUD, minimap — through `Game::Draw`'s own palette, the colour
//! animation at `cycles`) plus the level label (`:165-180`), so later rotation only reaches what
//! is drawn on top of it: colour 168 of the selected item. Every frame then copies the frozen
//! screen back (`:182`), draws the header box and "Select your weapons:" (`:188-190`), and per
//! player the name box and name (`:200-203`) and, unless the player is ready, the seven-item
//! menu (`:205-207`).
//!
//! `Origpal` is the FOCUSED palette: `Game::Focus` (`game.cpp:473-488`) writes both worms'
//! colour ramps into it right after the phase starts. The caller applies
//! [`set_worm_colour`](crate::palette::set_worm_colour) to the palette it passes in (the
//! `Scene`'s `origpal` and `weapsel_palette`'s argument).
//!
//! Not here (4½d): the render fade, the spectator variant, `Focus`/`Unfocus` (`focused` is
//! always true in the phase).

use assets::object::Weapon;
use assets::palette::Palette;
use assets::tc::Texts;
use sim::state::SimState;
use sim::weapsel::{WeaponSelection, DONE_ITEM, MENU_ITEMS, RANDOMIZE_ITEM};

use crate::bitmap::{Bitmap, Pal32, Rect};
use crate::blit::draw_rounded_box;
use crate::font::Font;
use crate::frame::{self, Scene};
use crate::menu::{draw_item, ItemColours};
use crate::palette::{pack_pal32, rotate_from};
use crate::viewport::Viewport;

/// The menu water rotation (`weapsel.cpp:22`).
pub const ROTATE_FROM: i32 = 168;
pub const ROTATE_TO: i32 = 174;
/// The header and the level label (`weapsel.cpp:172`, `:175`, `:190`).
pub const LABEL_COLOUR: i32 = 50;
/// `Palette::kWormColorBlocks[i].base + 1` (`palette.cpp:77-79`, `weapsel.cpp:203`).
pub const NAME_COLOURS: [i32; 2] = [33, 42];
/// RANDOMIZE, the weapon slots, DONE (`weapsel.cpp:49`, `:87`, `:90`).
pub const RANDOMIZE_COLOURS: ItemColours = ItemColours {
    color: 57,
    dis_colour: 57,
};
pub const WEAPON_COLOURS: ItemColours = ItemColours {
    color: 48,
    dis_colour: 48,
};
pub const DONE_COLOURS: ItemColours = ItemColours {
    color: 10,
    dis_colour: 10,
};
/// `Menu::item_height` (`menu.hpp:37`, `menu.cpp:104`).
pub const ITEM_HEIGHT: i32 = 8;
/// The level label's position (`weapsel.cpp:172`, `:175`).
pub const LABEL_POS: (i32, i32) = (0, 162);

/// The TC strings the screen draws (`tc.cfg:245-250`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeapselTexts {
    pub sel_weap: String,
    pub level_random: String,
    pub level_is1: String,
    pub level_is2: String,
    pub randomize: String,
    pub done: String,
}

impl WeapselTexts {
    pub fn from_tc(t: &Texts) -> WeapselTexts {
        WeapselTexts {
            sel_weap: t.SelWeap.clone(),
            level_random: t.LevelRandom.clone(),
            level_is1: t.LevelIs1.clone(),
            level_is2: t.LevelIs2.clone(),
            randomize: t.Randomize.clone(),
            done: t.Done.clone(),
        }
    }
}

/// The level label (`weapsel.cpp:171-176`): `LevelRandom` when `level_file` is EMPTY (not when
/// `random_level` is set), else `LevelIs1 + GetBasename(GetLeaf(level_file)) + LevelIs2`
/// (`filesystem.cpp:38-54`: the leaf after the last `/` or `\`, then up to its last `.`).
pub fn level_label(texts: &WeapselTexts, level_file: &str) -> String {
    if level_file.is_empty() {
        return texts.level_random.clone();
    }
    let leaf = level_file.rsplit(['/', '\\']).next().unwrap_or(level_file);
    let base = leaf.rsplit_once('.').map_or(leaf, |(b, _)| b);
    format!("{}{}{}", texts.level_is1, base, texts.level_is2)
}

/// `UpdateWeapselPalette` (`weapsel.cpp:20-24`): `Origpal`, then `RotateFrom(Origpal, 168, 174,
/// menu_cycles)`. No `color_anim`, no flash. `menu_cycles` is `Gfx::menu_cycles` (`unsigned`).
pub fn weapsel_palette(origpal: &Palette, menu_cycles: u32) -> Pal32 {
    let mut pal = origpal.clone();
    rotate_from(&mut pal, origpal, ROTATE_FROM, ROTATE_TO, menu_cycles);
    pack_pal32(&pal)
}

/// `Menu::Place(vp.rect.CenterX() - 31, vp.rect.CenterY() - 51)` (`weapsel.cpp:51-55`), with
/// the integer `(x1 + x2) / 2` of `math/rect.hpp:72-74`.
pub fn menu_origin(rect: &Rect) -> (i32, i32) {
    ((rect.x1 + rect.x2) / 2 - 31, (rect.y1 + rect.y2) / 2 - 51)
}

/// The frozen background (`weapsel.cpp:165-180`), built once per phase on the first draw.
/// `game.Draw` becomes `frame::draw` with FRESH viewports, which equal C++'s unprocessed ones
/// (plan-time fact 3), then the level label is drawn at (0, 162) in colour 50 through the
/// weapsel palette of that first draw's `menu_cycles`.
pub fn build_frozen(state: &SimState, scene: &Scene, label: &str, menu_cycles: u32) -> Bitmap {
    let mut bmp = Bitmap::new(320, 200);
    let mut viewports = Viewport::player_layout();
    frame::draw(&mut bmp, state, &mut viewports, scene);
    bmp.clip = Rect::new(0, 0, bmp.w, bmp.h);
    let pal = weapsel_palette(scene.origpal, menu_cycles);
    let (x, y) = LABEL_POS;
    scene
        .font
        .draw_string(&mut bmp, &pal, label, x, y, LABEL_COLOUR, 1);
    bmp
}

/// One frame of the screen over `frozen` (`weapsel.cpp:182-208`) with the weapsel palette `pal`.
/// `names` are the players' names (`WormSettings::name`).
#[allow(clippy::too_many_arguments)]
pub fn draw_screen(
    bmp: &mut Bitmap,
    frozen: &Bitmap,
    pal: &Pal32,
    font: &Font,
    texts: &WeapselTexts,
    ws: &WeaponSelection,
    weapons: &[Weapon],
    names: [&str; 2],
) {
    bmp.pixels.copy_from_slice(&frozen.pixels); // :182 renderer.bmp.Copy(frozen_screen)
    bmp.clip = Rect::new(0, 0, bmp.w, bmp.h);
    draw_rounded_box(bmp, pal, 114, 2, 0, 7, font.get_dims(&texts.sel_weap)); // :188
    font.draw_string(bmp, pal, &texts.sel_weap, 116, 3, LABEL_COLOUR, 1); // :190
    for (i, vp) in Viewport::player_layout().iter().enumerate() {
        let (mx, my) = menu_origin(&vp.rect);
        let width = font.get_dims(names[i]); // :200-203
        draw_rounded_box(bmp, pal, mx + 29 - width / 2, my - 11, 0, 7, width);
        font.draw_string(
            bmp,
            pal,
            names[i],
            mx + 31 - width / 2,
            my - 10,
            NAME_COLOURS[i],
            1,
        );
        let p = ws.player(i);
        if p.ready {
            continue; // :205
        }
        // Menu::Draw (menu.cpp:81-105): all seven items, no scrollbar (height 15 > 7).
        for k in 0..MENU_ITEMS {
            let (text, colours) = match k {
                RANDOMIZE_ITEM => (texts.randomize.as_str(), RANDOMIZE_COLOURS),
                DONE_ITEM => (texts.done.as_str(), DONE_COLOURS),
                slot => (
                    weapons[ws.weapon_index(i, slot as usize - 1)].name.as_str(),
                    WEAPON_COLOURS,
                ),
            };
            let y = my + ITEM_HEIGHT * k as i32;
            draw_item(
                bmp,
                pal,
                font,
                text,
                mx,
                y,
                p.cursor == k,
                false,
                false,
                colours,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assets::palette::Color;

    fn texts() -> WeapselTexts {
        WeapselTexts {
            level_random: "Level: Random".into(),
            level_is1: "Level: \"".into(),
            level_is2: "\"".into(),
            ..WeapselTexts::default()
        }
    }

    #[test]
    fn the_level_label_follows_level_file_not_random_level() {
        // weapsel.cpp:171-176 + filesystem.cpp:38-54.
        let t = texts();
        assert_eq!(level_label(&t, ""), "Level: Random");
        assert_eq!(
            level_label(&t, "Levels/render_stage.lev"),
            "Level: \"render_stage\""
        );
        assert_eq!(
            level_label(&t, "C:\\lev\\a.b.lev"),
            "Level: \"a.b\"",
            "the last '.'"
        );
        assert_eq!(level_label(&t, "noext"), "Level: \"noext\"");
    }

    #[test]
    fn the_tc_strings_are_the_cpp_ones() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc =
            assets::tc::TcConfig::load(&std::fs::read(format!("{root}/tc.cfg")).unwrap()).unwrap();
        let t = WeapselTexts::from_tc(&tc.texts);
        assert_eq!(
            (t.sel_weap.as_str(), t.randomize.as_str(), t.done.as_str()),
            ("Select your weapons:", "Randomize", "DONE!")
        );
        assert_eq!(level_label(&t, "x/y.lev"), "Level: \"y\"");
    }

    #[test]
    fn the_palette_rotates_only_168_to_174() {
        let mut p = Palette {
            entries: [Color::default(); 256],
        };
        for (i, e) in p.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        let pal = weapsel_palette(&p, 2);
        let packed = |i: usize| pack_pal32(&p)[i];
        assert_eq!(
            pal[168],
            packed(173),
            "dst[from+i] = src[from + (i + 7 - 2) % 7]"
        );
        assert_eq!(pal[174], packed(172));
        assert_eq!(
            (pal[167], pal[175], pal[50]),
            (packed(167), packed(175), packed(50))
        );
        assert_eq!(weapsel_palette(&p, 7), pack_pal32(&p), "a full turn");
    }

    #[test]
    fn the_menus_sit_at_the_cpp_origins() {
        let vps = Viewport::player_layout();
        assert_eq!(menu_origin(&vps[0].rect), (48, 28));
        assert_eq!(menu_origin(&vps[1].rect), (208, 28));
    }
}
