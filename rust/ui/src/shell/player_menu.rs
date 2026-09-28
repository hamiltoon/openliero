//! Step 4½f-2 — the player menu (`PlayerMenu`, `gfx.hpp:38-65`; items `gfx.cpp:459-483`;
//! behaviours `gfx.cpp:46-262`, `:1362-1428`; the colour bars `:1343-1360`; design §3.1, §4.7,
//! refresh R2-1..R2-9). One `Menu`, re-pointed per player like C++'s `player_menu.ws`
//! (`CurMenu::Player(p)`, plan D2); [`PlayerMenuModel`] is its virtuals over the live
//! `WormSettings` of that player.

use render::bitmap::{Bitmap, Pal32};
use render::blit::draw_rounded_box;
use scenario::settings::WormSettings;

use crate::keys::INPUT_KEYBOARD;
use crate::menu::behavior::Integer;
use crate::menu::{Behavior, CustomBehavior, Enter, LeftRight, Menu, MenuCx, MenuItem, MenuModel};
use crate::text::{CONTROLLERS, UiTc, get_gamepad_key_name, get_key_name, leaf_basename};

/// `PlayerMenu` item ids (`gfx.hpp:41-62`).
pub const PL_NAME: i32 = 0;
pub const PL_HEALTH: i32 = 1;
pub const PL_RED: i32 = 2;
pub const PL_GREEN: i32 = 3;
pub const PL_BLUE: i32 = 4;
pub const PL_INPUT: i32 = 5;
pub const PL_UP: i32 = 6;
pub const PL_DOWN: i32 = 7;
pub const PL_LEFT: i32 = 8;
pub const PL_RIGHT: i32 = 9;
pub const PL_FIRE: i32 = 10;
pub const PL_CHANGE: i32 = 11;
pub const PL_JUMP: i32 = 12;
pub const PL_DIG: i32 = 13;
pub const PL_WEAP0: i32 = 14;
pub const PL_CONTROLLER: i32 = PL_WEAP0 + 5;
pub const PL_SAVE_PROFILE: i32 = 20;
pub const PL_SAVE_PROFILE_AS: i32 = 21;
pub const PL_LOAD_PROFILE: i32 = 22;
pub const PL_LOADED_PROFILE: i32 = 23;

/// A player's loaded profile (C++ `WormSettings::profile_node`, runtime state, never in a file;
/// plan D5): the config-relative path the store reads, such as `Profiles/Lefty (L).toml`.
/// PROFILE LOADED shows its `leaf_basename`. Task 4 sets and clears it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileRef {
    pub rel: String,
}

/// `Gfx::LoadMenus`'s player menu (`gfx.cpp:459-483`, `:524`) at (178, 20) (`gfx.cpp:268`):
/// the four profile rows in colour 3, the rest 48, all disabled-colour 7; `value_offset_x` 95.
pub fn player_menu() -> Menu {
    let mut m = Menu::new(178, 20, false);
    for (c, s, id) in [
        (3, "PROFILE LOADED", PL_LOADED_PROFILE),
        (3, "SAVE PROFILE", PL_SAVE_PROFILE),
        (3, "SAVE PROFILE AS...", PL_SAVE_PROFILE_AS),
        (3, "LOAD PROFILE", PL_LOAD_PROFILE),
        (48, "NAME", PL_NAME),
        (48, "HEALTH", PL_HEALTH),
        (48, "Red", PL_RED),
        (48, "Green", PL_GREEN),
        (48, "Blue", PL_BLUE),
        (48, "INPUT", PL_INPUT),
        (48, "AIM UP", PL_UP),
        (48, "AIM DOWN", PL_DOWN),
        (48, "MOVE LEFT", PL_LEFT),
        (48, "MOVE RIGHT", PL_RIGHT),
        (48, "FIRE", PL_FIRE),
        (48, "CHANGE", PL_CHANGE),
        (48, "JUMP", PL_JUMP),
        (48, "DIG", PL_DIG),
    ] {
        m.add_item(MenuItem::new(c, 7, s, id));
    }
    for i in 0..5 {
        m.add_item(MenuItem::new(
            48,
            7,
            &format!("WEAPON {}", i + 1),
            PL_WEAP0 + i,
        ));
    }
    m.add_item(MenuItem::new(48, 7, "CONTROLLER", PL_CONTROLLER));
    m.value_offset_x = 95;
    m
}

/// `PlayerMenu`'s virtuals (`GetItemBehavior`, `DrawItemOverlay`) over `ws` — C++
/// `*player_menu.ws` — with the player's loaded profile and the TC's weapon names.
pub struct PlayerMenuModel<'a> {
    pub ws: &'a mut WormSettings,
    pub profile: Option<&'a ProfileRef>,
    pub tc: &'a UiTc,
}

impl PlayerMenuModel<'_> {
    /// The `int&` an `IntegerBehavior` of item `item_id` edits (`gfx.cpp:1370-1389`): where a
    /// number entry's continuation writes (design R-8).
    pub fn int_field(&mut self, item_id: i32) -> Option<&mut i32> {
        match item_id {
            PL_HEALTH => Some(&mut self.ws.health),
            PL_RED | PL_GREEN | PL_BLUE => Some(&mut self.ws.rgb[(item_id - PL_RED) as usize]),
            _ => None,
        }
    }
}

/// `WormNameBehavior::OnUpdate` (`gfx.cpp:73-83`).
struct WormName<'a> {
    name: &'a str,
}

impl CustomBehavior for WormName<'_> {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].value = self.name.to_string();
        menu.items[idx].has_value = true;
    }
}

/// `KeyBehavior::OnUpdate` (`gfx.cpp:46-71`) with `extended = Settings::kExtensions = true`:
/// `controls_ex[i]`'s name for a keyboard player, else `gamepad_controls[i]`'s (R2-6). DIG
/// reads `controls_ex[7]` for both refs (`:1405-1408`).
struct KeyRow {
    key_ex: u32,
    gamepad_key: u32,
    input_device: u32,
}

impl CustomBehavior for KeyRow {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].value = if self.input_device != INPUT_KEYBOARD {
            get_gamepad_key_name(self.gamepad_key)
        } else {
            get_key_name(self.key_ex)
        };
        menu.items[idx].has_value = true;
    }
}

/// `InputDeviceBehavior` (`gfx.cpp:85-201`) with no gamepad (Rust has none; the dumper opens
/// none, R2-21): `BuildOptions()` is empty, so the row reads `Keyboard` or the stored pad name
/// (its first 20 bytes) or `Gamepad (none)`, and `Cycle` always lands on Keyboard, clearing
/// the pad's name and serial, then `UpdateItems` (plan fact 7).
struct InputDevice<'a> {
    ws: &'a mut WormSettings,
}

impl InputDevice<'_> {
    /// `Cycle(menu, dir)` with zero options: `kCount == 1`, so `cur` is 0 (the keyboard).
    fn cycle(&mut self) {
        self.ws.input_device = INPUT_KEYBOARD;
        self.ws.gamepad_name.clear();
        self.ws.gamepad_serial.clear();
    }
}

impl CustomBehavior for InputDevice<'_> {
    fn on_left_right(
        &mut self,
        _menu: &mut Menu,
        _idx: usize,
        dir: i32,
        cx: &mut MenuCx,
    ) -> LeftRight {
        let hook = if dir > 0 {
            cx.hooks.move_up
        } else {
            cx.hooks.move_down
        };
        cx.play(hook);
        self.cycle();
        LeftRight {
            keep: false,
            update_all: true,
        }
    }

    fn on_enter(&mut self, _menu: &mut Menu, _idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        cx.play(cx.hooks.select);
        self.cycle();
        (Enter::Result(-1), true)
    }

    fn on_update(&self, menu: &mut Menu, idx: usize) {
        let ws = &*self.ws;
        menu.items[idx].value = if ws.input_device == INPUT_KEYBOARD {
            "Keyboard".to_string()
        } else if ws.gamepad_name.is_empty() {
            "Gamepad (none)".to_string()
        } else {
            // `substr(0, 20)`: bytes (a split UTF-8 character is C++'s mojibake; outside the gate).
            let b = ws.gamepad_name.as_bytes();
            String::from_utf8_lossy(&b[..b.len().min(20)]).into_owned()
        };
        menu.items[idx].has_value = true;
    }
}

/// `WeaponEnumBehavior` (`gfx.cpp:253-262`): `EnumBehavior(v, 1, weapons.size(), broken =
/// false)` whose value is the weapon's name in `weap_order`. A `v` of 0 or above the count is an
/// out-of-range read in C++ (UB, R2-7); Rust shows an empty value.
struct WeaponEnum<'a> {
    inner: Behavior<'a>,
    names: &'a [String],
}

impl CustomBehavior for WeaponEnum<'_> {
    fn on_left_right(
        &mut self,
        menu: &mut Menu,
        idx: usize,
        dir: i32,
        cx: &mut MenuCx,
    ) -> LeftRight {
        self.inner.on_left_right(menu, idx, dir, cx)
    }

    fn on_enter(&mut self, menu: &mut Menu, idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        self.inner.on_enter(menu, idx, cx)
    }

    fn on_update(&self, menu: &mut Menu, idx: usize) {
        let Behavior::Enum { v, .. } = &self.inner else {
            unreachable!("a weapon row is an Enum");
        };
        let name = v
            .checked_sub(1)
            .and_then(|i| self.names.get(i as usize))
            .map_or("", String::as_str);
        menu.items[idx].value = name.to_string();
        menu.items[idx].has_value = true;
    }
}

/// CONTROLLER's `ArrayEnumBehavior` over `Texts::controllers` (`gfx.cpp:1410-1411`). A value of
/// 3 or more is an out-of-range read in C++ (UB, R2-8); Rust shows an empty value.
struct Controller<'a> {
    inner: Behavior<'a>,
}

impl CustomBehavior for Controller<'_> {
    fn on_left_right(
        &mut self,
        menu: &mut Menu,
        idx: usize,
        dir: i32,
        cx: &mut MenuCx,
    ) -> LeftRight {
        self.inner.on_left_right(menu, idx, dir, cx)
    }

    fn on_enter(&mut self, menu: &mut Menu, idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        self.inner.on_enter(menu, idx, cx)
    }

    fn on_update(&self, menu: &mut Menu, idx: usize) {
        let Behavior::ArrayEnum { v, arr, .. } = &self.inner else {
            unreachable!("CONTROLLER is an ArrayEnum");
        };
        menu.items[idx].value = arr.get(**v as usize).copied().unwrap_or("").to_string();
        menu.items[idx].has_value = true;
    }
}

/// `ProfileSaveBehavior(save_as = false)::OnUpdate` (`gfx.cpp:223-228`): the row is visible iff
/// a profile is loaded. C++ writes `item.visible` directly — not `SetVisibility` — so
/// `visible_item_count` keeps the 24 of `AddItem` (the source; plan T3 Step 1's "22" is not what
/// C++ does). Its Enter (MenuSelect, the user-copy save) is intercepted (plan D6, Task 4).
struct ProfileSave {
    loaded: bool,
}

impl CustomBehavior for ProfileSave {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].visible = self.loaded;
    }
}

/// `ProfileLoadedBehavior::OnUpdate` (`gfx.cpp:234-251`): the loaded profile's
/// `GetBasename(GetLeaf(..))` and visible, else empty and hidden (the field, as above).
struct ProfileLoaded<'a> {
    profile: Option<&'a ProfileRef>,
}

impl CustomBehavior for ProfileLoaded<'_> {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        let item = &mut menu.items[idx];
        match self.profile {
            Some(p) => {
                item.value = leaf_basename(&p.rel).to_string();
                item.visible = true;
            }
            None => {
                item.value.clear();
                item.visible = false;
            }
        }
        item.has_value = true;
    }
}

impl MenuModel for PlayerMenuModel<'_> {
    /// `PlayerMenu::GetItemBehavior` (`gfx.cpp:1362-1428`), item for item. RGB is the classic
    /// picker only (0..252 in steps of 4, shown ÷4; design decision 10: no F10 modern mode).
    fn behavior(&mut self, item_id: i32) -> Behavior<'_> {
        let PlayerMenuModel { ws, profile, tc } = self;
        if (PL_WEAP0..PL_WEAP0 + 5).contains(&item_id) {
            return Behavior::Custom(Box::new(WeaponEnum {
                inner: Behavior::Enum {
                    v: &mut ws.weapons[(item_id - PL_WEAP0) as usize],
                    min: 1,
                    max: tc.weapon_names.len() as u32,
                    broken: false,
                },
                names: &tc.weapon_names,
            }));
        }
        match item_id {
            PL_NAME => Behavior::Custom(Box::new(WormName { name: &ws.name })),
            PL_HEALTH => {
                let mut b = Integer::new(&mut ws.health, 1, 10000, 1, true);
                b.scroll_interval = 4;
                Behavior::Integer(b)
            }
            PL_RED | PL_GREEN | PL_BLUE => {
                let v = &mut ws.rgb[(item_id - PL_RED) as usize];
                let mut b = Integer::new(v, 0, 252, 4, false);
                b.display_div = 4;
                b.scroll_interval = 4;
                Behavior::Integer(b)
            }
            PL_INPUT => Behavior::Custom(Box::new(InputDevice { ws })),
            PL_UP..=PL_DIG => {
                let i = (item_id - PL_UP) as usize;
                Behavior::Custom(Box::new(KeyRow {
                    key_ex: ws.controls_ex[i],
                    gamepad_key: ws.gamepad_controls[i],
                    input_device: ws.input_device,
                }))
            }
            PL_CONTROLLER => Behavior::Custom(Box::new(Controller {
                inner: Behavior::ArrayEnum {
                    v: &mut ws.controller,
                    arr: &CONTROLLERS,
                    broken: false,
                },
            })),
            PL_SAVE_PROFILE => Behavior::Custom(Box::new(ProfileSave {
                loaded: profile.is_some(),
            })),
            PL_LOADED_PROFILE => Behavior::Custom(Box::new(ProfileLoaded { profile: *profile })),
            // SAVE PROFILE AS… (`ProfileSaveBehavior(true)`: no display) and LOAD PROFILE
            // (`ProfileLoadBehavior`, no overrides): both Enters are intercepted.
            _ => Behavior::Plain,
        }
    }

    /// `PlayerMenu::DrawItemOverlay` (`gfx.cpp:1343-1360`, R2-9): the Red/Green/Blue bars —
    /// `DrawRoundedBox(x + 24, y, selected ? 168 : 0, 7, (rgb >> 2) - 1)`, then
    /// `FillRect(x + 25, y + 1, rgb >> 2, 5, ws.color)`.
    fn draw_item_overlay(
        &self,
        item: &MenuItem,
        x: i32,
        y: i32,
        selected: bool,
        _disabled: bool,
        bmp: &mut Bitmap,
        pal: &Pal32,
    ) {
        if (PL_RED..=PL_BLUE).contains(&item.id) {
            let width = self.ws.rgb[(item.id - PL_RED) as usize] >> 2;
            let frame = if selected { 168 } else { 0 };
            draw_rounded_box(bmp, pal, x + 24, y, frame, 7, width - 1);
            bmp.fill_rect(x + 25, y + 1, width, 5, self.ws.color as u8, pal);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use scenario::paths::TC_ROOT;
    use scenario::settings::Settings;

    use super::*;
    use crate::menu::MenuHooks;

    fn tc() -> UiTc {
        UiTc::load(Path::new(TC_ROOT))
    }

    const HOOKS: MenuHooks = MenuHooks {
        move_up: 25,
        move_down: 26,
        select: 27,
    };

    fn updated(ws: &mut WormSettings, profile: Option<&ProfileRef>, tc: &UiTc) -> Menu {
        let mut m = player_menu();
        m.update_items(&mut PlayerMenuModel { ws, profile, tc });
        m.move_to_first_visible();
        m
    }

    fn value(m: &Menu, id: i32) -> &str {
        &m.item_from_id(id).unwrap().value
    }

    fn visible_rows(m: &Menu) -> usize {
        m.items.iter().filter(|i| i.visible).count()
    }

    #[test]
    fn the_player_menu_is_24_rows_in_loadmenus_order() {
        let m = player_menu();
        assert_eq!(
            (m.x, m.y, m.value_offset_x, m.height),
            (178, 20, 95, 15),
            "gfx.cpp:268, :524"
        );
        let rows: Vec<(i32, u8, u8, &str)> = m
            .items
            .iter()
            .map(|i| (i.id, i.color, i.dis_colour, i.string.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (23, 3, 7, "PROFILE LOADED"),
                (20, 3, 7, "SAVE PROFILE"),
                (21, 3, 7, "SAVE PROFILE AS..."),
                (22, 3, 7, "LOAD PROFILE"),
                (0, 48, 7, "NAME"),
                (1, 48, 7, "HEALTH"),
                (2, 48, 7, "Red"),
                (3, 48, 7, "Green"),
                (4, 48, 7, "Blue"),
                (5, 48, 7, "INPUT"),
                (6, 48, 7, "AIM UP"),
                (7, 48, 7, "AIM DOWN"),
                (8, 48, 7, "MOVE LEFT"),
                (9, 48, 7, "MOVE RIGHT"),
                (10, 48, 7, "FIRE"),
                (11, 48, 7, "CHANGE"),
                (12, 48, 7, "JUMP"),
                (13, 48, 7, "DIG"),
                (14, 48, 7, "WEAPON 1"),
                (15, 48, 7, "WEAPON 2"),
                (16, 48, 7, "WEAPON 3"),
                (17, 48, 7, "WEAPON 4"),
                (18, 48, 7, "WEAPON 5"),
                (19, 48, 7, "CONTROLLER"),
            ]
        );
        assert_eq!(
            (PL_CONTROLLER, PL_LOADED_PROFILE),
            (19, 23),
            "gfx.hpp:41-62"
        );
        assert_eq!(m.visible_item_count, 24);
    }

    #[test]
    fn the_profile_rows_show_only_with_a_profile_and_never_change_the_count() {
        let tc = tc();
        let mut s = Settings::default();
        let m = updated(&mut s.worm_settings[0], None, &tc);
        assert_eq!(
            visible_rows(&m),
            22,
            "PROFILE LOADED and SAVE PROFILE hidden"
        );
        assert_eq!(
            m.visible_item_count, 24,
            "C++ writes item.visible directly (gfx.cpp:225, :240-243): AddItem's count stays"
        );
        assert_eq!(
            m.selected_id(),
            PL_SAVE_PROFILE_AS,
            "the first visible row (R2-3)"
        );
        assert!(!m.item_from_id(PL_SAVE_PROFILE).unwrap().has_value);
        let p = ProfileRef {
            rel: "Profiles/Lefty (L).toml".into(),
        };
        let m = updated(&mut s.worm_settings[0], Some(&p), &tc);
        assert_eq!((visible_rows(&m), m.visible_item_count), (24, 24));
        assert_eq!(value(&m, PL_LOADED_PROFILE), "Lefty (L)");
        assert_eq!(m.selected_id(), PL_LOADED_PROFILE);
        assert!(
            !m.item_from_id(PL_SAVE_PROFILE).unwrap().has_value,
            "SAVE PROFILE has no value"
        );
    }

    #[test]
    fn the_default_players_values() {
        let tc = tc();
        let mut s = Settings::default();
        let m = updated(&mut s.worm_settings[0], None, &tc);
        let ids = [
            PL_NAME,
            PL_HEALTH,
            PL_RED,
            PL_GREEN,
            PL_BLUE,
            PL_INPUT,
            PL_UP,
            PL_DOWN,
            PL_LEFT,
            PL_RIGHT,
            PL_FIRE,
            PL_CHANGE,
            PL_JUMP,
            PL_DIG,
            PL_CONTROLLER,
        ];
        let got: Vec<&str> = ids.iter().map(|&id| value(&m, id)).collect();
        assert_eq!(
            got,
            [
                "",
                "100%",
                "26",
                "26",
                "63",
                "Keyboard",
                "R",
                "F",
                "D",
                "G",
                "Left Crtl",
                "Left Shift",
                "Left Alt",
                "",
                "Human"
            ]
        );
        for i in 0..5 {
            assert_eq!(
                value(&m, PL_WEAP0 + i),
                tc.weapon_names[s.worm_settings[0].weapons[i as usize] as usize - 1]
            );
        }
        assert!(
            m.items
                .iter()
                .filter(|i| i.id != PL_SAVE_PROFILE_AS
                    && i.id != PL_LOAD_PROFILE
                    && i.id != PL_SAVE_PROFILE)
                .all(|i| i.has_value)
        );
    }

    fn joystick(ws: &mut WormSettings) {
        ws.input_device = 1;
        ws.gamepad_name.clear();
        ws.gamepad_controls = [11, 12, 13, 14, 110, 10, 0, 9];
    }

    #[test]
    fn a_pad_player_shows_the_pad_and_its_buttons() {
        let tc = tc();
        let mut s = Settings::default();
        joystick(&mut s.worm_settings[0]);
        let m = updated(&mut s.worm_settings[0], None, &tc);
        let rows: Vec<&str> = (PL_INPUT..=PL_DIG).map(|id| value(&m, id)).collect();
        assert_eq!(
            rows,
            [
                "Gamepad (none)",
                "Up",
                "Down",
                "Left",
                "Right",
                "RT+",
                "RB",
                "A",
                "LB"
            ],
            "R2-5, R2-6 (T0 P6)"
        );
        s.worm_settings[0].gamepad_name = "A very long pad name that is long".into();
        let m = updated(&mut s.worm_settings[0], None, &tc);
        assert_eq!(value(&m, PL_INPUT), "A very long pad name", "substr(0, 20)");
        assert_eq!(value(&m, PL_INPUT).len(), 20);
    }

    fn cx(sounds: &mut Vec<i32>, menu_cycles: u32) -> MenuCx<'_> {
        MenuCx {
            menu_cycles,
            hooks: HOOKS,
            sounds,
        }
    }

    #[test]
    fn input_cycles_back_to_the_keyboard_with_the_sound_of_the_direction() {
        let tc = tc();
        let mut s = Settings::default();
        for (dir, sound) in [(-1, 26), (1, 25), (0, 27)] {
            let ws = &mut s.worm_settings[0];
            joystick(ws);
            ws.gamepad_name = "Pad".into();
            ws.gamepad_serial = "123".into();
            let mut m = updated(ws, None, &tc);
            m.move_to_id(PL_INPUT);
            let mut sounds = Vec::new();
            let mut model = PlayerMenuModel {
                ws,
                profile: None,
                tc: &tc,
            };
            if dir == 0 {
                assert_eq!(
                    m.on_enter(&mut model, &mut cx(&mut sounds, 0)),
                    Enter::Result(-1)
                );
            } else {
                assert!(
                    !m.on_left_right(&mut model, dir, &mut cx(&mut sounds, 1)),
                    "returns false"
                );
            }
            assert_eq!(sounds, [sound], "dir {dir}: gfx.cpp:170-175");
            assert_eq!(value(&m, PL_INPUT), "Keyboard");
            assert_eq!(value(&m, PL_UP), "R", "UpdateItems ran");
            let ws = &s.worm_settings[0];
            assert_eq!(
                (
                    ws.input_device,
                    ws.gamepad_name.as_str(),
                    ws.gamepad_serial.as_str()
                ),
                (0, "", "")
            );
        }
    }

    #[test]
    fn integers_repeat_on_their_interval_and_clamp() {
        let tc = tc();
        let mut s = Settings::default();
        let ws = &mut s.worm_settings[0];
        let mut m = updated(ws, None, &tc);
        m.move_to_id(PL_HEALTH);
        let mut sounds = Vec::new();
        let mut model = PlayerMenuModel {
            ws,
            profile: None,
            tc: &tc,
        };
        for cycles in 1..=12 {
            assert!(m.on_left_right(&mut model, 1, &mut cx(&mut sounds, cycles)));
        }
        assert_eq!(model.ws.health, 103, "+1 every 4th menu cycle (4, 8, 12)");
        assert_eq!(value(&m, PL_HEALTH), "103%");
        m.move_to_id(PL_RED);
        model.ws.rgb[0] = 8;
        for _ in 0..3 {
            m.on_left_right(&mut model, -1, &mut cx(&mut sounds, 0));
        }
        assert_eq!(model.ws.rgb[0], 0, "step 4, clamped at 0");
        model.ws.rgb[0] = 248;
        for _ in 0..3 {
            m.on_left_right(&mut model, 1, &mut cx(&mut sounds, 8));
        }
        assert_eq!(
            (model.ws.rgb[0], value(&m, PL_RED)),
            (252, "63"),
            "clamped at 252, shown ÷4"
        );
        assert!(sounds.is_empty());
    }

    #[test]
    fn weapons_and_the_controller_wrap_and_odd_values_show_empty() {
        let tc = tc();
        let mut s = Settings::default();
        let ws = &mut s.worm_settings[0];
        let mut m = updated(ws, None, &tc);
        let mut sounds = Vec::new();
        let mut model = PlayerMenuModel {
            ws,
            profile: None,
            tc: &tc,
        };
        m.move_to_id(PL_WEAP0);
        assert!(!m.on_left_right(&mut model, -1, &mut cx(&mut sounds, 1)));
        assert_eq!(model.ws.weapons[0], 40, "1 wraps to 40");
        assert_eq!(value(&m, PL_WEAP0), tc.weapon_names[39]);
        m.on_left_right(&mut model, 1, &mut cx(&mut sounds, 1));
        assert_eq!(model.ws.weapons[0], 1);
        m.move_to_id(PL_CONTROLLER);
        let mut seen = Vec::new();
        for dir in [1, 1, 1, -1] {
            m.on_left_right(&mut model, dir, &mut cx(&mut sounds, 1));
            seen.push((model.ws.controller, value(&m, PL_CONTROLLER).to_string()));
        }
        assert_eq!(
            seen,
            [
                (1, "CPU".into()),
                (2, "AI".into()),
                (0, "Human".into()),
                (2, "AI".into())
            ]
        );
        assert_eq!(sounds, [26, 25, 25, 25, 25, 26]);
        model.ws.weapons[1] = 0;
        model.ws.weapons[2] = 41;
        model.ws.controller = 3;
        m.update_items(&mut model);
        assert_eq!(
            (
                value(&m, PL_WEAP0 + 1),
                value(&m, PL_WEAP0 + 2),
                value(&m, PL_CONTROLLER)
            ),
            ("", "", ""),
            "C++ UB (R2-7, R2-8): empty, no panic"
        );
    }

    fn ramp() -> Pal32 {
        std::array::from_fn(|i| 0xFF00_0000 | i as u32)
    }

    #[test]
    fn the_colour_bars_are_a_box_and_a_fill_in_the_players_colour() {
        let tc = tc();
        let pal = ramp();
        let mut s = Settings::default();
        s.worm_settings[0].rgb = [252, 40, 0];
        let item = |id: i32| MenuItem::new(48, 7, "", id);
        let (x, y) = (178, 60);
        let model = PlayerMenuModel {
            ws: &mut s.worm_settings[0],
            profile: None,
            tc: &tc,
        };
        let sentinel = 0xDEAD_BEEF;
        for (id, selected, width) in [(PL_RED, true, 63), (PL_GREEN, false, 10)] {
            let mut got = Bitmap::new(320, 200);
            got.pixels.fill(sentinel);
            model.draw_item_overlay(&item(id), x, y, selected, false, &mut got, &pal);
            let mut want = Bitmap::new(320, 200);
            want.pixels.fill(sentinel);
            let frame = if selected { 168 } else { 0 };
            // DrawRoundedBox(x + 24, y, frame, 7, width - 1): the band, the top and bottom rows.
            want.fill_rect(x + 24, y + 1, width + 2, 5, frame, &pal);
            want.fill_rect(x + 25, y, width, 1, frame, &pal);
            want.fill_rect(x + 25, y + 6, width, 1, frame, &pal);
            want.fill_rect(x + 25, y + 1, width, 5, 32, &pal);
            assert_eq!(got, want, "item {id}");
            assert_eq!(got.get_pixel(x + 24, y + 3), pal[frame as usize]);
            assert_eq!(got.get_pixel(x + 25, y + 3), pal[32], "ws.color 32");
            assert_eq!(got.get_pixel(x + 25 + width, y + 3), pal[frame as usize]);
            assert_eq!(got.get_pixel(x + 26 + width, y + 3), sentinel);
            assert_eq!(got.get_pixel(x + 24, y), sentinel, "an open corner");
        }
        // Blue = 0: the width -1 box (a 2-pixel band), no fill, no panic.
        let mut got = Bitmap::new(320, 200);
        got.pixels.fill(sentinel);
        model.draw_item_overlay(&item(PL_BLUE), x, y, false, false, &mut got, &pal);
        let drawn: Vec<(i32, i32)> = (0..320)
            .flat_map(|c| (0..200).map(move |r| (c, r)))
            .filter(|&(c, r)| got.get_pixel(c, r) != sentinel)
            .collect();
        let want: Vec<(i32, i32)> = (x + 24..x + 26)
            .flat_map(|c| (y + 1..y + 6).map(move |r| (c, r)))
            .collect();
        assert_eq!(drawn, want);
        // Every other row draws nothing.
        let mut got = Bitmap::new(320, 200);
        got.pixels.fill(sentinel);
        model.draw_item_overlay(&item(PL_HEALTH), x, y, true, false, &mut got, &pal);
        assert!(got.pixels.iter().all(|&p| p == sentinel));
    }

    #[test]
    fn int_field_names_health_and_the_channels() {
        let tc = tc();
        let mut s = Settings::default();
        let mut model = PlayerMenuModel {
            ws: &mut s.worm_settings[1],
            profile: None,
            tc: &tc,
        };
        *model.int_field(PL_HEALTH).unwrap() = 7;
        *model.int_field(PL_BLUE).unwrap() = 12;
        assert!(model.int_field(PL_INPUT).is_none() && model.int_field(PL_NAME).is_none());
        assert_eq!(
            (s.worm_settings[1].health, s.worm_settings[1].rgb[2]),
            (7, 12)
        );
    }
}
