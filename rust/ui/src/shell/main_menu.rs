//! The main menu (`MainMenu`, `mainMenu.hpp:9-25`; items `gfx.cpp:505-521`; design §3.3). T7 adds
//! `MainMenuState`.

use crate::menu::{Menu, MenuItem, PlainModel};

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
