//! The main menu (`MainMenu`, `mainMenu.hpp:9-25`; items `gfx.cpp:505-521`; design §3.3). T7 adds
//! `MainMenuState`. Step 4½f-2 (T3): the player-menu focus (`CurMenu::Player`), its Enter arms,
//! F5 / F6 / F9 in C++ order, and its draw.

use render::bitmap::Rect;
use render::font::Font;
use scenario::storage::ConfigStore;

use super::files::{LevelSelectorState, SetupSelectorState};
use super::overlay::{
    EntryTarget, InfoBoxState, InfoPurpose, InputPurpose, InputStringState, KeyTarget, RefusalGate,
    SaveAsKind, WaitForKeyState, filter_digits,
};
use super::player_menu::{
    PL_DIG, PL_LOAD_PROFILE, PL_NAME, PL_SAVE_PROFILE, PL_SAVE_PROFILE_AS, PL_UP, PL_WEAP0,
    PlayerMenuModel,
};
use super::settings_menu::{
    LOAD_OPTIONS, SAVE_OPTIONS, SI_LEVEL, SI_WEAPON_OPTIONS, SettingsModel,
};
use super::stack::Screen;
use super::weapon_options::WeaponMenuState;
use super::{CurMenu, MenuWorld, save_as_box};
use crate::keys::{
    DK_DOWN, DK_ESCAPE, DK_F1, DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9, DK_KP_ENTER,
    DK_LEFT, DK_PGDN, DK_PGUP, DK_RETURN, DK_RIGHT, DK_UP, K_DOWN, K_FIRE, K_JUMP, K_LEFT, K_RIGHT,
    K_UP, reset_left_right,
};
use crate::menu::{Enter, Menu, MenuCx, MenuItem, PlainModel};
use crate::text::refusal_text;

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
/// Step 4½e-1: `pushes` are the screens an update pushes (C++ `state_stack.Push` inside
/// `Update`, plan fact 4), in order; the shell runs each one's `enter` and pushes it after the
/// update. `now_ms` is the type-to-search clock (WEAPON OPTIONS, the selectors), and `gate` the
/// Rust-only refusal check (plan T4 Step 5).
pub struct MenuCtx<'a> {
    pub w: &'a mut MenuWorld,
    /// Step 4½e-2: the config store the selectors list and read (C++ `GetConfigNode()`).
    pub store: &'a dyn ConfigStore,
    pub font: &'a Font,
    pub running: bool,
    pub sounds: &'a mut Vec<i32>,
    pub pushes: Vec<Screen>,
    pub now_ms: u64,
    pub gate: RefusalGate,
}

impl MenuCtx<'_> {
    /// Request `screen`'s push after this update (plan fact 4).
    pub fn push(&mut self, screen: Screen) {
        self.pushes.push(screen);
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

/// `g_sound_player->Play(hook)`: a negative id is dropped (`mixer/player.hpp:15-22`).
pub(super) fn play(sounds: &mut Vec<i32>, hook: i32) {
    if hook >= 0 {
        sounds.push(hook);
    }
}

/// `gfx->cur_menu` (plan fact 1).
fn cur_menu_mut(w: &mut MenuWorld) -> &mut Menu {
    match w.cur_menu {
        CurMenu::Main => &mut w.main_menu,
        CurMenu::Settings => &mut w.settings_menu,
        CurMenu::Player(_) => &mut w.player_menu,
    }
}

/// `gfx->cur_menu->OnLeftRight(common, dir)` with the menu's own model: the base behavior for
/// the main menu, the settings for the settings menu, player `p`'s settings for the player menu.
fn cur_menu_left_right(w: &mut MenuWorld, dir: i32, cx: &mut MenuCx) -> bool {
    match w.cur_menu {
        CurMenu::Main => w.main_menu.on_left_right(&mut PlainModel, dir, cx),
        CurMenu::Settings => w.settings_menu.on_left_right(
            &mut SettingsModel {
                settings: &mut w.settings,
                tc: &w.tc,
                setup_name: &w.setup_name,
            },
            dir,
            cx,
        ),
        CurMenu::Player(p) => {
            let (menu, mut model) = w.player_parts(p);
            menu.on_left_right(&mut model, dir, cx)
        }
    }
}

/// `ItemPosition` of the player menu's item `id` plus the value column (`x += value_offset_x +
/// 2`, `mainMenuState.cpp:329-331`, `:353-354`, `:395-396`), or `None` when it is not in view.
fn player_value_pos(m: &Menu, id: i32) -> Option<(i32, i32)> {
    let idx = usize::try_from(m.index_from_id(id)).ok()?;
    m.item_position(idx)
        .map(|(x, y)| (x + m.value_offset_x + 2, y))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuPhase {
    Active,
    FadingOut,
}

/// C++ `MainMenuState` (`mainMenuState.hpp`, `mainMenuState.cpp:95-626`). Step 4½e-1: the
/// settings focus (`cur_menu`), its Enter dispatch and the Rust-only refusals.
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

    /// `Update` (`mainMenuState.cpp:152-612`): the fade-out, else the keys in C++ source order,
    /// acting on `cur_menu` (the main or the settings menu, plan fact 1; design §3.2). The 4½d
    /// `||` short-circuits are kept (finding 15).
    pub fn update(&mut self, cx: &mut MenuCtx) -> bool {
        if self.phase == MenuPhase::FadingOut {
            let w = &mut *cx.w;
            if w.fade > 0 {
                w.fade -= 1;
                return true;
            }
            return false;
        }
        let MenuCtx { w, sounds, .. } = cx;
        let hooks = w.tc.hooks;
        // :171-179 (Esc, or any keyboard player's jump): main focus — the cursor to QUIT TO OS;
        // settings focus — back to the main menu, no sound.
        if w.keys.test_once(DK_ESCAPE)
            || w.keys.test_control_once(&w.settings.worm_settings, K_JUMP)
        {
            if w.cur_menu == CurMenu::Main {
                w.main_menu.move_to_id(MA_QUIT);
            } else {
                w.cur_menu = CurMenu::Main;
            }
        }
        // :181-192: Up plays MenuMoveDown, Down plays MenuMoveUp, on `cur_menu`.
        if w.keys.test_once(DK_UP) || w.keys.test_control_once(&w.settings.worm_settings, K_UP) {
            play(sounds, hooks.move_down);
            cur_menu_mut(w).movement(-1);
        }
        if w.keys.test_once(DK_DOWN) || w.keys.test_control_once(&w.settings.worm_settings, K_DOWN)
        {
            play(sounds, hooks.move_up);
            cur_menu_mut(w).movement(1);
        }
        // :194-420.
        let mut push = None;
        if w.keys.test_once(DK_RETURN)
            || w.keys.test_once(DK_KP_ENTER)
            || w.keys.test_control_once(&w.settings.worm_settings, K_FIRE)
        {
            if w.cur_menu == CurMenu::Main {
                play(sounds, hooks.select);
                match w.main_menu.selected_id() {
                    MA_SETTINGS => w.cur_menu = CurMenu::Settings,
                    // :207-216 (Step 4½f-2): `PlayerSettings(0 | 1 | kNetworkPlayerIdx)`.
                    id @ (MA_PLAYER1_SETTINGS | MA_PLAYER2_SETTINGS) => {
                        w.player_settings((id - MA_PLAYER1_SETTINGS) as usize);
                    }
                    MA_NET_PLAYER_SETTINGS => w.player_settings(2),
                    // `default:` (:263-266).
                    id @ (MA_RESUME_GAME | MA_NEW_GAME | MA_QUIT) => {
                        w.cur_menu = CurMenu::Main;
                        self.selected = id;
                    }
                    // Their second MenuSelect (:234, :248, :254); inert until Step 5 (plan fact 2).
                    MA_JOIN_GAME | MA_HOST_ONLINE | MA_JOIN_ONLINE => play(sounds, hooks.select),
                    // Inert placeholders (§5, Q2): OPTIONS (4½g), HOST LAN (Step 5), REPLAYS / TC
                    // (deferred).
                    _ => {}
                }
            } else if let CurMenu::Player(p) = w.cur_menu {
                push = self.player_enter(w, p, sounds);
            } else {
                match w.settings_menu.selected_id() {
                    // :275-278: MenuSelect + push WeaponMenuState.
                    SI_WEAPON_OPTIONS => {
                        play(sounds, hooks.select);
                        push = Some(Screen::WeaponOptions(WeaponMenuState::new()));
                    }
                    // :272-275, :282-285 (Step 4½e-2): MenuSelect + push the level selector or
                    // the options selector (LOAD SETUP).
                    SI_LEVEL => {
                        play(sounds, hooks.select);
                        push = Some(Screen::LevelSelect(LevelSelectorState::new()));
                    }
                    LOAD_OPTIONS => {
                        play(sounds, hooks.select);
                        push = Some(Screen::SetupSelect(SetupSelectorState::new()));
                    }
                    // :287-311: MenuSelect, then — only while `ItemPosition` finds the item in
                    // view — the name box on the setup's name at the item's value column
                    // (`MakeSaveAsState`, :69-91).
                    SAVE_OPTIONS => {
                        play(sounds, hooks.select);
                        let m = &w.settings_menu;
                        let idx = m.index_from_id(SAVE_OPTIONS);
                        if let Some((x, y)) =
                            usize::try_from(idx).ok().and_then(|i| m.item_position(i))
                        {
                            let x = x + m.value_offset_x + 2;
                            push = Some(save_as_box(
                                SaveAsKind::Setup,
                                w.setup_name.as_bytes(),
                                x,
                                y,
                            ));
                        }
                    }
                    // :313-315: the behavior plays its own MenuSelect (plan fact 2); an
                    // IntegerBehavior pushes its number entry (integerBehavior.cpp:56-78).
                    _ => {
                        let mut mcx = MenuCx {
                            menu_cycles: w.menu_cycles,
                            hooks,
                            sounds,
                        };
                        let enter = w.settings_menu.on_enter(
                            &mut SettingsModel {
                                settings: &mut w.settings,
                                tc: &w.tc,
                                setup_name: &w.setup_name,
                            },
                            &mut mcx,
                        );
                        if let Enter::EditValue(e) = enter {
                            let initial = e.initial.clone().into_bytes();
                            push = Some(Screen::InputString(InputStringState::new(
                                &initial,
                                e.digits as usize,
                                e.x,
                                e.y,
                                Some(filter_digits),
                                "",
                                false,
                                InputPurpose::IntegerEntry {
                                    entry: e,
                                    target: EntryTarget::Settings,
                                },
                            )));
                        }
                    }
                }
            }
        }
        // :432-436.
        if w.keys.test_once(DK_F1) {
            w.cur_menu = CurMenu::Main;
            w.main_menu.move_to_id(self.start_item_id);
            self.selected = self.start_item_id;
        }
        // :437-446: consumed; OPTIONS (4½g) and REPLAYS (deferred) stay inert.
        for k in [DK_F2, DK_F3] {
            w.keys.test_once(k);
        }
        // :448-455 (Step 4½f-2): no sound, the main cursor to the item, `PlayerSettings`.
        if w.keys.test_once(DK_F5) {
            w.main_menu.move_to_id(MA_PLAYER1_SETTINGS);
            w.player_settings(0);
        }
        if w.keys.test_once(DK_F6) {
            w.main_menu.move_to_id(MA_PLAYER2_SETTINGS);
            w.player_settings(1);
        }
        // :456-459.
        if w.keys.test_once(DK_F7) {
            w.main_menu.move_to_id(MA_SETTINGS);
            w.cur_menu = CurMenu::Settings;
        }
        // :461-464 (Step 4½f-2): the network player's menu.
        if w.keys.test_once(DK_F9) {
            w.main_menu.move_to_id(MA_NET_PLAYER_SETTINGS);
            w.player_settings(2);
        }
        // :466 — the F8 easter egg: consumed, inert.
        w.keys.test_once(DK_F8);
        let mut mcx = MenuCx {
            menu_cycles: w.menu_cycles,
            hooks,
            sounds,
        };
        // :581-592: held; a behavior returning false releases Left/Right (Bool and Enum do).
        for (key, control, dir) in [(DK_LEFT, K_LEFT, -1), (DK_RIGHT, K_RIGHT, 1)] {
            if (w.keys.test(key) || w.keys.test_control(&w.settings.worm_settings, control))
                && !cur_menu_left_right(w, dir, &mut mcx)
            {
                reset_left_right(&mut w.keys, &w.settings.worm_settings);
            }
        }
        // :594-602.
        if w.keys.test_once(DK_PGUP) {
            mcx.play(hooks.move_down);
            cur_menu_mut(w).movement_page(-1);
        }
        if w.keys.test_once(DK_PGDN) {
            mcx.play(hooks.move_up);
            cur_menu_mut(w).movement_page(1);
        }
        if let Some(screen) = push {
            cx.push(screen);
        }
        // :604-609: start the fade-out — unless the Rust-only refusal (Q2, D5) stops it: the
        // selection is dropped and the box goes up, with no sound of its own.
        if self.selected >= 0 {
            let w = &mut *cx.w;
            if let Some(r) = cx.gate.refusal(&w.settings, self.selected) {
                eprintln!("refused: {r}");
                self.selected = -1;
                let text = refusal_text(&r, &w.tc);
                cx.push(Screen::InfoBox(InfoBoxState::new(
                    &text,
                    160,
                    100,
                    false,
                    InfoPurpose::Refused(r),
                )));
            } else {
                self.phase = MenuPhase::FadingOut;
                w.fade = 32;
            }
        }
        true
    }

    /// The player menu's Enter (`mainMenuState.cpp:316-427`; plan facts 1-2, D3, D4), in source
    /// order: LOAD PROFILE, NAME, SAVE PROFILE AS…, the eight key rows, WEAPON n, else the row's
    /// own `OnEnter` (`selected_ = …`). Every intercepted arm plays `MenuSelect` first; NAME,
    /// SAVE PROFILE AS… and WEAPON n push only while the item is in view, the key rows always.
    /// Returns the screen to push.
    fn player_enter(
        &mut self,
        w: &mut MenuWorld,
        p: usize,
        sounds: &mut Vec<i32>,
    ) -> Option<Screen> {
        let hooks = w.tc.hooks;
        let id = w.player_menu.selected_id();
        match id {
            // :318-322, :348-366; SAVE PROFILE is the `else` arm's `ProfileSaveBehavior`
            // (plan fact 2, D6). Task 4 makes them live; here: the MenuSelect alone.
            PL_LOAD_PROFILE | PL_SAVE_PROFILE_AS | PL_SAVE_PROFILE => {
                play(sounds, hooks.select);
                None
            }
            // :323-347: `InputStringState(ws.name, 20, x + 95 + 2, y, no filter)`.
            PL_NAME => {
                play(sounds, hooks.select);
                let (x, y) = player_value_pos(&w.player_menu, id)?;
                let name = w.settings.worm_settings[p].name.as_bytes();
                Some(Screen::InputString(InputStringState::new(
                    name,
                    20,
                    x,
                    y,
                    None,
                    "",
                    false,
                    InputPurpose::WormName { player: p },
                )))
            }
            // :367-389: `WaitForKeyState(extended = true)`, pushed whether or not in view.
            PL_UP..=PL_DIG => {
                play(sounds, hooks.select);
                Some(Screen::WaitForKey(WaitForKeyState::new(KeyTarget {
                    player: p,
                    control: (id - PL_UP) as usize,
                })))
            }
            // :390-423: `InputStringState("", 10, x + 97, y, no filter)`.
            _ if (PL_WEAP0..PL_WEAP0 + 5).contains(&id) => {
                play(sounds, hooks.select);
                let (x, y) = player_value_pos(&w.player_menu, id)?;
                Some(Screen::InputString(InputStringState::new(
                    b"",
                    10,
                    x,
                    y,
                    None,
                    "",
                    false,
                    InputPurpose::WeaponFuzzy {
                        player: p,
                        slot: (id - PL_WEAP0) as usize,
                    },
                )))
            }
            // :424-426: the behavior plays its own sound (INPUT, CONTROLLER); HEALTH and R/G/B
            // push their number entry (integerBehavior.cpp:56-78) into this player's fields.
            _ => {
                let mut mcx = MenuCx {
                    menu_cycles: w.menu_cycles,
                    hooks,
                    sounds,
                };
                let (menu, mut model) = w.player_parts(p);
                match menu.on_enter(&mut model, &mut mcx) {
                    Enter::Result(r) => {
                        self.selected = r;
                        None
                    }
                    // `IntegerBehavior::OnEnter` returns -1 after its push.
                    Enter::EditValue(e) => {
                        self.selected = -1;
                        let initial = e.initial.clone().into_bytes();
                        Some(Screen::InputString(InputStringState::new(
                            &initial,
                            e.digits as usize,
                            e.x,
                            e.y,
                            Some(filter_digits),
                            "",
                            false,
                            InputPurpose::IntegerEntry {
                                entry: e,
                                target: EntryTarget::Player(p),
                            },
                        )))
                    }
                }
            }
        }
    }

    /// `Draw` (`mainMenuState.cpp:614-626`; plan fact 4): `DrawBasicMenu`, then — with the main
    /// menu's focus — the settings menu disabled, else `cur_menu` enabled (the settings menu or
    /// the player menu with its colour bars). `DrawSpectatorInfo` draws into the spectator
    /// renderer only (finding 2).
    pub fn draw(&self, cx: &mut MenuCtx) {
        let w = &mut *cx.w;
        draw_basic_menu(w, cx.font);
        match w.cur_menu {
            CurMenu::Main | CurMenu::Settings => w.settings_menu.draw(
                &PlainModel,
                &mut w.surface,
                &w.pal32,
                cx.font,
                w.cur_menu == CurMenu::Main,
                -1,
                false,
            ),
            CurMenu::Player(p) => {
                let model = PlayerMenuModel {
                    ws: &mut w.settings.worm_settings[p],
                    profile: w.profiles[p].as_ref(),
                    tc: &w.tc,
                };
                w.player_menu
                    .draw(&model, &mut w.surface, &w.pal32, cx.font, false, -1, false);
            }
        }
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
