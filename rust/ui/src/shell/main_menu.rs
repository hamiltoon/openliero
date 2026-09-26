//! The main menu (`MainMenu`, `mainMenu.hpp:9-25`; items `gfx.cpp:505-521`; design §3.3). T7 adds
//! `MainMenuState`.

use render::bitmap::Rect;
use render::font::Font;

use super::settings_menu::SettingsModel;
use super::stack::Screen;
use super::{CurMenu, MenuWorld};
use crate::keys::{
    DK_DOWN, DK_ESCAPE, DK_F1, DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9, DK_KP_ENTER,
    DK_LEFT, DK_PGDN, DK_PGUP, DK_RETURN, DK_RIGHT, DK_UP, K_DOWN, K_FIRE, K_JUMP, K_LEFT, K_RIGHT,
    K_UP, reset_left_right,
};
use crate::menu::{Menu, MenuCx, MenuItem, PlainModel};

/// `MainMenu` item ids (`mainMenu.hpp:9-25`).
pub const MA_RESUME_GAME: i32 = 0;
pub const MA_NEW_GAME: i32 = 1;
pub const MA_SETTINGS: i32 = 2;
pub const MA_PLAYER1_SETTINGS: i32 = 3;
pub const MA_PLAYER2_SETTINGS: i32 = 4;
pub const MA_ADVANCED: i32 = 5;
pub const MA_QUIT: i32 = 6;
pub const MA_REPLAYS: i32 = 7;
pub const MA_REPLAY: i32 = 8;
pub const MA_TC: i32 = 9;
pub const MA_HOST_GAME: i32 = 10;
pub const MA_JOIN_GAME: i32 = 11;
pub const MA_NET_PLAYER_SETTINGS: i32 = 12;
pub const MA_HOST_ONLINE: i32 = 13;
pub const MA_JOIN_ONLINE: i32 = 14;

/// `Gfx::LoadMenus`'s main menu (`gfx.cpp:505-521`) at (53, 20) (`gfx.cpp:266`). RESUME and NEW
/// GAME get their strings in `MainMenuState::enter`, TC its `"TC (<tc>)"`.
pub fn main_menu() -> Menu {
    let mut m = Menu::new(53, 20, false);
    for (c, s, id) in [
        (10, "", MA_RESUME_GAME),
        (10, "", MA_NEW_GAME),
        (48, "HOST LAN GAME", MA_HOST_GAME),
        (48, "JOIN LAN GAME", MA_JOIN_GAME),
        (48, "HOST ONLINE", MA_HOST_ONLINE),
        (48, "JOIN ONLINE", MA_JOIN_ONLINE),
        (48, "OPTIONS (F2)", MA_ADVANCED),
        (48, "REPLAYS (F3)", MA_REPLAYS),
        (48, "TC", MA_TC),
        (6, "QUIT TO OS", MA_QUIT),
    ] {
        m.add_item(MenuItem::new(c, c, s, id));
    }
    m.add_item(MenuItem::space());
    for (s, id) in [
        ("LEFT PLAYER (F5)", MA_PLAYER1_SETTINGS),
        ("RIGHT PLAYER (F6)", MA_PLAYER2_SETTINGS),
        ("NETWORK PLAYER (F9)", MA_NET_PLAYER_SETTINGS),
        ("MATCH SETUP (F7)", MA_SETTINGS),
    ] {
        m.add_item(MenuItem::new(48, 48, s, id));
    }
    m
}

/// `MainMenu::GetItemBehavior` (`mainMenu.cpp:6-10`): every item is the base behavior;
/// `MainMenuState::Update` intercepts them all.
pub type MainModel = PlainModel;

/// What `MainMenuState` may touch: the menu world (C++ `Gfx` members), the font, whether the
/// current controller `Running()`, and this frame's sound log. No path to the sim (LD 3, §4.9).
/// Step 4½e-1: `push` is the one screen an update may push (C++ `state_stack.Push` inside
/// `Update`, plan fact 4); the shell runs its `enter` and pushes it after the update.
pub struct MenuCtx<'a> {
    pub w: &'a mut MenuWorld,
    pub font: &'a Font,
    pub running: bool,
    pub sounds: &'a mut Vec<i32>,
    pub push: Option<Screen>,
}

impl MenuCtx<'_> {
    /// Request `screen`'s push after this update (plan fact 4). One per update.
    pub fn push(&mut self, screen: Screen) {
        debug_assert!(self.push.is_none(), "one push per update");
        self.push = Some(screen);
    }
}

/// `Gfx::DrawBasicMenu` (`gfx.cpp:1699-1704`): the frozen screen, then the main menu — disabled
/// whenever another menu has focus (`cur_menu != &main_menu`, plan fact 1) — its selection shown.
/// `MainMenuState::Draw` and (4½e-1 T4) `WeaponMenuState::Draw` call it.
pub fn draw_basic_menu(w: &mut MenuWorld, font: &Font) {
    w.surface.pixels.copy_from_slice(&w.frozen.pixels);
    w.surface.clip = Rect::new(0, 0, w.surface.w, w.surface.h);
    w.main_menu.draw(
        &PlainModel,
        &mut w.surface,
        &w.pal32,
        font,
        w.cur_menu != CurMenu::Main,
        -1,
        true,
    );
}

fn play(sounds: &mut Vec<i32>, hook: i32) {
    if hook >= 0 {
        sounds.push(hook);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuPhase {
    Active,
    FadingOut,
}

/// C++ `MainMenuState` (`mainMenuState.hpp`, `mainMenuState.cpp:95-626`). In 4½d the main menu
/// always has focus (`cur_menu == &main_menu`); 4½e adds the settings focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MainMenuState {
    phase: MenuPhase,
    selected: i32,
    start_item_id: i32,
}

impl Default for MainMenuState {
    fn default() -> Self {
        MainMenuState::new()
    }
}

impl MainMenuState {
    pub fn new() -> MainMenuState {
        MainMenuState {
            phase: MenuPhase::Active,
            selected: -1,
            start_item_id: 0,
        }
    }

    /// `Selection()`: the item chosen this menu, or -1.
    pub fn selection(&self) -> i32 {
        self.selected
    }

    pub fn is_fading_out(&self) -> bool {
        self.phase == MenuPhase::FadingOut
    }

    /// `Enter` (`mainMenuState.cpp:95-148`) after its `Flip` at fade 0 — the caller presents that
    /// black frame. The copyright bar goes through whatever palette the last draw left in
    /// `w.pal32` (finding 12); `Process()` (`:105`) has nothing left to poll.
    pub fn enter(&mut self, cx: &mut MenuCtx) {
        let w = &mut *cx.w;
        w.fade = 0;
        w.surface.clip = Rect::new(0, 0, w.surface.w, w.surface.h);
        w.surface.fill_rect(0, 151, 160, 7, 0, &w.pal32);
        cx.font
            .draw_string(&mut w.surface, &w.pal32, &w.tc.copyright2, 2, 152, 19, 1);
        let m = &mut w.main_menu;
        if cx.running {
            m.set_visibility(MA_RESUME_GAME, true);
            m.item_from_id_mut(MA_RESUME_GAME).expect("RESUME").string = "RESUME GAME (F1)".into();
            m.item_from_id_mut(MA_NEW_GAME).expect("NEW GAME").string = "NEW GAME".into();
            self.start_item_id = MA_RESUME_GAME;
        } else {
            m.set_visibility(MA_RESUME_GAME, false);
            m.item_from_id_mut(MA_NEW_GAME).expect("NEW GAME").string = "NEW GAME (F1)".into();
            self.start_item_id = MA_NEW_GAME;
        }
        m.item_from_id_mut(MA_TC).expect("TC").string = format!("TC ({})", w.settings.tc);
        m.move_to_first_visible();
        w.settings_menu.move_to_first_visible();
        w.settings_menu.update_items(&mut SettingsModel {
            settings: &mut w.settings,
            tc: &w.tc,
            setup_name: &w.setup_name,
        });
        w.fade = 0;
        w.cur_menu = CurMenu::Main; // :129
        w.frozen.pixels.copy_from_slice(&w.surface.pixels);
        w.menu_cycles = 0;
        self.selected = -1;
        self.phase = MenuPhase::Active;
    }

    /// `Update` (`mainMenuState.cpp:152-612`): the fade-out, else the keys in C++ source order.
    pub fn update(&mut self, cx: &mut MenuCtx) -> bool {
        let MenuCtx { w, sounds, .. } = cx;
        if self.phase == MenuPhase::FadingOut {
            if w.fade > 0 {
                w.fade -= 1;
                return true;
            }
            return false;
        }
        let hooks = w.tc.hooks;
        let ws = &w.settings.worm_settings;
        // :171-179 (Esc, or any keyboard player's jump): the cursor to QUIT TO OS.
        if w.keys.test_once(DK_ESCAPE) || w.keys.test_control_once(ws, K_JUMP) {
            w.main_menu.move_to_id(MA_QUIT);
        }
        // :181-192: Up plays MenuMoveDown, Down plays MenuMoveUp. `||` short-circuits (finding 15).
        if w.keys.test_once(DK_UP) || w.keys.test_control_once(ws, K_UP) {
            play(sounds, hooks.move_down);
            w.main_menu.movement(-1);
        }
        if w.keys.test_once(DK_DOWN) || w.keys.test_control_once(ws, K_DOWN) {
            play(sounds, hooks.move_up);
            w.main_menu.movement(1);
        }
        // :194-270.
        if w.keys.test_once(DK_RETURN)
            || w.keys.test_once(DK_KP_ENTER)
            || w.keys.test_control_once(ws, K_FIRE)
        {
            play(sounds, hooks.select);
            match w.main_menu.selected_id() {
                id @ (MA_RESUME_GAME | MA_NEW_GAME | MA_QUIT) => self.selected = id, // `default:`
                // Their second MenuSelect (:234, :248, :254); inert until Step 5 (plan fact 2).
                MA_JOIN_GAME | MA_HOST_ONLINE | MA_JOIN_ONLINE => play(sounds, hooks.select),
                // Inert placeholders (§5, Q2): MATCH SETUP (4½e), LEFT/RIGHT PLAYER (4½f), OPTIONS
                // (4½g), NETWORK PLAYER / HOST LAN (Step 5), REPLAYS / TC (deferred).
                _ => {}
            }
        }
        // :432-436.
        if w.keys.test_once(DK_F1) {
            w.main_menu.move_to_id(self.start_item_id);
            self.selected = self.start_item_id;
        }
        // :437-461 and the F8 easter egg (:463): consumed as C++ does, inert in 4½d (§5).
        for k in [DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F9, DK_F8] {
            w.keys.test_once(k);
        }
        let mut mcx = MenuCx {
            menu_cycles: w.menu_cycles,
            hooks,
            sounds,
        };
        // :581-592: held; a behavior returning false releases Left/Right (never, on MainModel).
        if (w.keys.test(DK_LEFT) || w.keys.test_control(ws, K_LEFT))
            && !w.main_menu.on_left_right(&mut PlainModel, -1, &mut mcx)
        {
            reset_left_right(&mut w.keys, ws);
        }
        if (w.keys.test(DK_RIGHT) || w.keys.test_control(ws, K_RIGHT))
            && !w.main_menu.on_left_right(&mut PlainModel, 1, &mut mcx)
        {
            reset_left_right(&mut w.keys, ws);
        }
        // :594-602.
        if w.keys.test_once(DK_PGUP) {
            mcx.play(hooks.move_down);
            w.main_menu.movement_page(-1);
        }
        if w.keys.test_once(DK_PGDN) {
            mcx.play(hooks.move_up);
            w.main_menu.movement_page(1);
        }
        // :604-609: start the fade-out.
        if self.selected >= 0 {
            self.phase = MenuPhase::FadingOut;
            w.fade = 32;
        }
        true
    }

    /// `Draw` (`mainMenuState.cpp:614-626`): `DrawBasicMenu`, then the settings menu — disabled
    /// while the main menu has focus, else enabled as `cur_menu` (plan fact 1; 4½f/4½g add the
    /// other menus). `DrawSpectatorInfo` draws into the spectator renderer only (finding 2).
    pub fn draw(&self, cx: &mut MenuCtx) {
        let w = &mut *cx.w;
        draw_basic_menu(w, cx.font);
        w.settings_menu.draw(
            &PlainModel,
            &mut w.surface,
            &w.pal32,
            cx.font,
            w.cur_menu == CurMenu::Main,
            -1,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_menu_is_the_cpp_table() {
        let m = main_menu();
        assert_eq!(
            (m.x, m.y, m.height, m.item_height),
            (53, 20, 15, 8),
            "gfx.cpp:266"
        );
        let rows: Vec<(i32, u8, u8, &str, bool)> = m
            .items
            .iter()
            .map(|i| (i.id, i.color, i.dis_colour, i.string.as_str(), i.selectable))
            .collect();
        assert_eq!(
            rows,
            [
                (MA_RESUME_GAME, 10, 10, "", true),
                (MA_NEW_GAME, 10, 10, "", true),
                (MA_HOST_GAME, 48, 48, "HOST LAN GAME", true),
                (MA_JOIN_GAME, 48, 48, "JOIN LAN GAME", true),
                (MA_HOST_ONLINE, 48, 48, "HOST ONLINE", true),
                (MA_JOIN_ONLINE, 48, 48, "JOIN ONLINE", true),
                (MA_ADVANCED, 48, 48, "OPTIONS (F2)", true),
                (MA_REPLAYS, 48, 48, "REPLAYS (F3)", true),
                (MA_TC, 48, 48, "TC", true),
                (MA_QUIT, 6, 6, "QUIT TO OS", true),
                (-1, 0, 0, "", false),
                (MA_PLAYER1_SETTINGS, 48, 48, "LEFT PLAYER (F5)", true),
                (MA_PLAYER2_SETTINGS, 48, 48, "RIGHT PLAYER (F6)", true),
                (MA_NET_PLAYER_SETTINGS, 48, 48, "NETWORK PLAYER (F9)", true),
                (MA_SETTINGS, 48, 48, "MATCH SETUP (F7)", true),
            ]
        );
        assert_eq!(
            m.visible_item_count, 15,
            "15 visible against height 15: never a scrollbar"
        );
        assert_eq!(
            [
                MA_RESUME_GAME,
                MA_NEW_GAME,
                MA_SETTINGS,
                MA_QUIT,
                MA_REPLAY,
                MA_TC,
                MA_JOIN_ONLINE
            ],
            [0, 1, 2, 6, 8, 9, 14],
            "mainMenu.hpp:9-25"
        );
    }
}
