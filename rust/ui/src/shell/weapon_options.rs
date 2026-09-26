//! Step 4½e-1 — C++ `WeaponMenuState` (`weaponMenuState.cpp`; design §3.4, §4.4, plan facts
//! 10-11): the WEAPON OPTIONS screen the settings focus pushes over the main menu. Forty rows in
//! `weap_order` order, each an `ArrayEnumBehavior` over `settings.weap_table[weap_order[row]]`
//! with the `weap_states` texts; Left/Right once, a type-to-search over `key_buf`, and a close
//! that refuses with `InfoBoxState(NoWeaps, 223, 68)` while every weapon is Bonus or Banned.

use render::font::Font;

use super::MenuWorld;
use super::main_menu::{MenuCtx, draw_basic_menu, play};
use super::overlay::{InfoBoxState, InfoPurpose};
use super::stack::Screen;
use crate::keys::{
    DK_DOWN, DK_ESCAPE, DK_LEFT, DK_PGDN, DK_PGUP, DK_RIGHT, DK_UP, K_DOWN, K_JUMP, K_LEFT,
    K_RIGHT, K_UP,
};
use crate::menu::{Behavior, Menu, MenuCx, MenuItem, MenuModel, PlainModel};
use crate::text::WEAP_STATES;

/// `WeaponMenu::GetItemBehavior` (`weaponMenuState.cpp:14-22`): row `id` edits
/// `weap_table[weap_order[id]]` through `ArrayEnumBehavior(weap_states)`.
pub struct WeaponModel<'a> {
    pub weap_table: &'a mut [u32; 40],
    pub weap_order: &'a [usize],
}

impl MenuModel for WeaponModel<'_> {
    fn behavior(&mut self, item_id: i32) -> Behavior<'_> {
        Behavior::ArrayEnum {
            v: &mut self.weap_table[self.weap_order[item_id as usize]],
            arr: &WEAP_STATES,
            broken: false,
        }
    }
}

/// C++ `WeaponMenuState` (`weaponMenuState.hpp`): its `weaponMenu_`, built by `enter`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponMenuState {
    menu: Menu,
}

impl Default for WeaponMenuState {
    fn default() -> Self {
        WeaponMenuState::new()
    }
}

impl WeaponMenuState {
    /// The state before its `Enter` (C++ constructs `weaponMenu_` in `Enter`).
    pub fn new() -> WeaponMenuState {
        WeaponMenuState {
            menu: Menu::new(179, 28, false),
        }
    }

    /// The weapon menu (`weaponMenu_`).
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    /// For tests: the weapon menu, to place the cursor.
    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    /// `Enter` (`weaponMenuState.cpp:26-42`): a `WeaponMenu` at (179, 28), height 14,
    /// `value_offset_x = 89`, one row per weapon in `weap_order` order (id = row), the cursor on
    /// the first visible row, then `UpdateItems`.
    pub fn enter(&mut self, w: &mut MenuWorld) {
        let mut menu = Menu::new(179, 28, false);
        menu.set_height(14);
        menu.value_offset_x = 89;
        for (i, name) in w.tc.weapon_names.iter().enumerate() {
            menu.add_item(MenuItem::new(48, 7, name, i as i32));
        }
        menu.move_to_first_visible();
        menu.update_items(&mut WeaponModel {
            weap_table: &mut w.settings.weap_table,
            weap_order: &w.tc.weap_order,
        });
        self.menu = menu;
    }

    /// `Update` (`weaponMenuState.cpp:46-112`) in C++ source order: Up, Down (crossed sounds),
    /// Left and Right *once* (`OnLeftRight`, its result ignored), PgUp, PgDn
    /// (`Settings::kExtensions`), `OnKeys` over this frame's `key_buf`, then Esc / any Jump: the
    /// close rule (fact 11). Returns false to pop this frame.
    pub fn update(&mut self, cx: &mut MenuCtx) -> bool {
        let now_ms = cx.now_ms;
        let MenuCtx { w, sounds, .. } = cx;
        let hooks = w.tc.hooks;
        let ws = &w.settings.worm_settings;
        if w.keys.test_once(DK_UP) || w.keys.test_control_once(ws, K_UP) {
            play(sounds, hooks.move_down);
            self.menu.movement(-1);
        }
        if w.keys.test_once(DK_DOWN) || w.keys.test_control_once(ws, K_DOWN) {
            play(sounds, hooks.move_up);
            self.menu.movement(1);
        }
        let left = w.keys.test_once(DK_LEFT) || w.keys.test_control_once(ws, K_LEFT);
        let right = w.keys.test_once(DK_RIGHT) || w.keys.test_control_once(ws, K_RIGHT);
        let mut mcx = MenuCx {
            menu_cycles: w.menu_cycles,
            hooks,
            sounds,
        };
        let mut model = WeaponModel {
            weap_table: &mut w.settings.weap_table,
            weap_order: &w.tc.weap_order,
        };
        if left {
            self.menu.on_left_right(&mut model, -1, &mut mcx);
        }
        if right {
            self.menu.on_left_right(&mut model, 1, &mut mcx);
        }
        if w.keys.test_once(DK_PGUP) {
            mcx.play(hooks.move_down);
            self.menu.movement_page(-1);
        }
        if w.keys.test_once(DK_PGDN) {
            mcx.play(hooks.move_up);
            self.menu.movement_page(1);
        }
        self.menu.on_keys(w.keys.typed(), now_ms, false);
        let w = &mut *cx.w;
        let ws = &w.settings.worm_settings;
        if w.keys.test_once(DK_ESCAPE) || w.keys.test_control_once(ws, K_JUMP) {
            // :94-104: close when any weapon is still in the menu (weap_table entry 0).
            if w.settings.weap_table.contains(&0) {
                return false;
            }
            let text = w.tc.no_weaps.clone();
            cx.push(Screen::InfoBox(InfoBoxState::new(
                &text,
                223,
                68,
                false,
                InfoPurpose::NoWeapons,
            )));
        }
        true
    }

    /// `Draw` (`weaponMenuState.cpp:114-127`): the frozen screen and `DrawBasicMenu` (the main
    /// menu disabled while the settings menu has focus, its selection shown), the two framed
    /// headers (fact 10), then the weapon menu enabled.
    pub fn draw(&self, w: &mut MenuWorld, font: &Font) {
        draw_basic_menu(w, font);
        font.draw_framed_text(&mut w.surface, &w.pal32, &w.tc.weapon, 179, 20, 50);
        font.draw_framed_text(&mut w.surface, &w.pal32, &w.tc.availability, 249, 20, 50);
        self.menu.draw(
            &PlainModel,
            &mut w.surface,
            &w.pal32,
            font,
            false,
            -1,
            false,
        );
    }
}
