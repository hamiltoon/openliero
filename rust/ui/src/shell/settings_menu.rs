//! The settings menu, display only in 4½d (`SettingsMenu`, `gfx.hpp:72-100`; items
//! `gfx.cpp:485-503`; behaviors `:1262-1312`; `OnUpdate` `:1314-1341`; design finding 1, §4.10).
//! The main screen draws it disabled at (178, 20) with `value_offset_x = 100`. 4½e gives it
//! focus, the LEVEL / WEAPON OPTIONS / SAVE / LOAD pushes and config I/O.

use scenario::settings::{
    GM_GAME_OF_TAG, GM_HOLDAZONE, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE, Settings,
};

use crate::menu::behavior::Integer;
use crate::menu::{Behavior, CustomBehavior, Menu, MenuItem, MenuModel};
use crate::text::{GAME_MODES, UiTc, leaf_basename};

/// `SettingsMenu` item ids (`gfx.hpp:73-93`).
pub const SI_GAME_MODE: i32 = 0;
pub const SI_LIVES: i32 = 1;
pub const SI_TIME_TO_LOSE: i32 = 2;
pub const SI_TIME_TO_WIN: i32 = 3;
pub const SI_ZONE_TIMEOUT: i32 = 4;
pub const SI_FLAGS_TO_WIN: i32 = 5;
pub const SI_LOADING_TIMES: i32 = 6;
pub const SI_MAX_BONUSES: i32 = 7;
pub const SI_NAMES_ON_BONUSES: i32 = 8;
pub const SI_MAP: i32 = 9;
pub const SI_AMOUNT_OF_BLOOD: i32 = 10;
pub const SI_LEVEL: i32 = 11;
pub const SI_RANDOM_MAP_WIDTH: i32 = 12;
pub const SI_RANDOM_MAP_HEIGHT: i32 = 13;
pub const SI_REGENERATE_LEVEL: i32 = 14;
pub const SI_WEAPON_OPTIONS: i32 = 15;
pub const LOAD_OPTIONS: i32 = 16;
pub const SAVE_OPTIONS: i32 = 17;
pub const LOAD_CHANGE: i32 = 18;

/// `Gfx::LoadMenus`'s settings menu (`gfx.cpp:485-503`, `:523`) at (178, 20) (`gfx.cpp:267`).
pub fn settings_menu() -> Menu {
    let mut m = Menu::new(178, 20, false);
    for (s, id) in [
        ("GAME MODE", SI_GAME_MODE),
        ("TIME TO LOSE", SI_TIME_TO_LOSE),
        ("TIME TO WIN", SI_TIME_TO_WIN),
        ("ZONE TIMEOUT", SI_ZONE_TIMEOUT),
        ("FLAGS TO WIN", SI_FLAGS_TO_WIN),
        ("LIVES", SI_LIVES),
        ("LEVEL", SI_LEVEL),
        ("MAP WIDTH", SI_RANDOM_MAP_WIDTH),
        ("MAP HEIGHT", SI_RANDOM_MAP_HEIGHT),
        ("LOADING TIMES", SI_LOADING_TIMES),
        ("WEAPON OPTIONS", SI_WEAPON_OPTIONS),
        ("MAX BONUSES", SI_MAX_BONUSES),
        ("NAMES ON BONUSES", SI_NAMES_ON_BONUSES),
        ("MAP", SI_MAP),
        ("AMOUNT OF BLOOD", SI_AMOUNT_OF_BLOOD),
        ("LOAD+CHANGE", LOAD_CHANGE),
        ("REGENERATE LEVEL", SI_REGENERATE_LEVEL),
        ("SAVE SETUP AS...", SAVE_OPTIONS),
        ("LOAD SETUP", LOAD_OPTIONS),
    ] {
        m.add_item(MenuItem::new(48, 7, s, id));
    }
    m.value_offset_x = 100;
    m
}

/// `SettingsMenu`'s virtuals over the live `Settings` (C++ `gfx.settings`). `setup_name` is
/// `GetBasename(GetLeaf(gfx.settings_node.FullPath()))`: `"liero"` until 4½e loads setups.
pub struct SettingsModel<'a> {
    pub settings: &'a mut Settings,
    pub tc: &'a UiTc,
    pub setup_name: &'a str,
}

/// `LevelSelectBehavior::OnUpdate` (`gfx.cpp:1222-1238`), which relabels REGENERATE LEVEL.
struct LevelSelect<'a> {
    random_level: bool,
    level_file: &'a str,
    tc: &'a UiTc,
}

impl CustomBehavior for LevelSelect<'_> {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].has_value = true;
        let label = if self.random_level {
            menu.items[idx].value = self.tc.random2.clone();
            &self.tc.regen_level
        } else {
            menu.items[idx].value = format!("\"{}\"", leaf_basename(self.level_file));
            &self.tc.reload_level
        };
        menu.item_from_id_mut(SI_REGENERATE_LEVEL)
            .expect("the settings menu has REGENERATE LEVEL")
            .string = label.clone();
    }
}

/// `OptionsSaveBehavior::OnUpdate` (`gfx.cpp:1245-1253`).
struct OptionsSave<'a> {
    name: &'a str,
}

impl CustomBehavior for OptionsSave<'_> {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].value = self.name.to_string();
        menu.items[idx].has_value = true;
    }
}

impl MenuModel for SettingsModel<'_> {
    /// `SettingsMenu::GetItemBehavior` (`gfx.cpp:1262-1312`), item for item.
    fn behavior(&mut self, item_id: i32) -> Behavior<'_> {
        let SettingsModel {
            settings: s,
            tc,
            setup_name,
        } = self;
        match item_id {
            SI_NAMES_ON_BONUSES => Behavior::Bool(&mut s.names_on_bonuses),
            SI_MAP => Behavior::Bool(&mut s.map),
            SI_REGENERATE_LEVEL => Behavior::Bool(&mut s.regenerate_level),
            SI_LOADING_TIMES => {
                Behavior::Integer(Integer::new(&mut s.loading_time, 0, 9999, 1, true))
            }
            SI_MAX_BONUSES => Behavior::Integer(Integer::new(&mut s.max_bonuses, 0, 99, 1, false)),
            SI_AMOUNT_OF_BLOOD => {
                let mut b = Integer::new(&mut s.blood, 0, tc.blood_limit, tc.blood_step_up, true);
                b.allow_entry = false;
                Behavior::Integer(b)
            }
            SI_LIVES => Behavior::Integer(Integer::new(&mut s.lives, 1, 999, 1, false)),
            SI_TIME_TO_LOSE | SI_TIME_TO_WIN => {
                Behavior::time(&mut s.time_to_lose, 60, 3600, 10, false)
            }
            SI_ZONE_TIMEOUT => Behavior::time(&mut s.zone_timeout, 10, 3600, 10, false),
            SI_FLAGS_TO_WIN => {
                Behavior::Integer(Integer::new(&mut s.flags_to_win, 1, 999, 1, false))
            }
            SI_LEVEL => Behavior::Custom(Box::new(LevelSelect {
                random_level: s.random_level,
                level_file: &s.level_file,
                tc,
            })),
            SI_RANDOM_MAP_WIDTH => {
                Behavior::Integer(Integer::new(&mut s.random_map_width, 64, 4096, 8, false))
            }
            SI_RANDOM_MAP_HEIGHT => {
                Behavior::Integer(Integer::new(&mut s.random_map_height, 64, 4096, 8, false))
            }
            SI_GAME_MODE => Behavior::ArrayEnum {
                v: &mut s.game_mode,
                arr: &GAME_MODES,
                broken: false,
            },
            SAVE_OPTIONS => Behavior::Custom(Box::new(OptionsSave { name: setup_name })),
            LOAD_CHANGE => Behavior::Bool(&mut s.load_change),
            // WEAPON OPTIONS, LOAD SETUP: behaviors with no display (their pushes are 4½e's).
            _ => Behavior::Plain,
        }
    }

    /// `SettingsMenu::OnUpdate` (`gfx.cpp:1314-1341`).
    fn on_update(&mut self, menu: &mut Menu) {
        for id in [
            SI_LIVES,
            SI_TIME_TO_LOSE,
            SI_TIME_TO_WIN,
            SI_ZONE_TIMEOUT,
            SI_FLAGS_TO_WIN,
        ] {
            menu.set_visibility(id, false);
        }
        menu.set_visibility(SI_RANDOM_MAP_WIDTH, self.settings.random_level);
        menu.set_visibility(SI_RANDOM_MAP_HEIGHT, self.settings.random_level);
        match self.settings.game_mode {
            GM_KILL_EM_ALL | GM_SCALES_OF_JUSTICE => menu.set_visibility(SI_LIVES, true),
            GM_GAME_OF_TAG => menu.set_visibility(SI_TIME_TO_LOSE, true),
            GM_HOLDAZONE => {
                menu.set_visibility(SI_TIME_TO_WIN, true);
                menu.set_visibility(SI_ZONE_TIMEOUT, true);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Enter, MenuCx};
    use crate::text::UiTc;
    use scenario::paths::TC_ROOT;
    use scenario::settings::{
        GM_GAME_OF_TAG, GM_HOLDAZONE, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE, Settings,
    };
    use std::path::Path;

    fn tc() -> UiTc {
        UiTc::load(Path::new(TC_ROOT))
    }

    fn shown(m: &Menu) -> Vec<&str> {
        m.items
            .iter()
            .filter(|i| i.visible)
            .map(|i| i.string.as_str())
            .collect()
    }

    fn updated(s: &mut Settings) -> Menu {
        let tc = tc();
        let mut m = settings_menu();
        m.move_to_first_visible();
        m.update_items(&mut SettingsModel {
            settings: s,
            tc: &tc,
            setup_name: "liero",
        });
        m
    }

    #[test]
    fn the_settings_menu_is_19_items_in_loadmenus_order() {
        let m = settings_menu();
        assert_eq!(
            (m.x, m.y, m.value_offset_x),
            (178, 20, 100),
            "gfx.cpp:267, :523"
        );
        let ids: Vec<i32> = m.items.iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            [
                0, 2, 3, 4, 5, 1, 11, 12, 13, 6, 15, 7, 8, 9, 10, 18, 14, 17, 16
            ]
        );
        assert!(m.items.iter().all(|i| (i.color, i.dis_colour) == (48, 7)));
        assert_eq!(m.items[16].string, "REGENERATE LEVEL");
        assert_eq!(m.visible_item_count, 19);
    }

    #[test]
    fn the_defaults_show_15_items_with_their_values() {
        let mut s = Settings::default();
        let m = updated(&mut s);
        let rows: Vec<(&str, &str)> = m
            .items
            .iter()
            .filter(|i| i.visible)
            .map(|i| (i.string.as_str(), i.value.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("GAME MODE", "Kill'em All"),
                ("LIVES", "15"),
                ("LEVEL", "Random"),
                ("MAP WIDTH", "504"),
                ("MAP HEIGHT", "350"),
                ("LOADING TIMES", "100%"),
                ("WEAPON OPTIONS", ""),
                ("MAX BONUSES", "4"),
                ("NAMES ON BONUSES", "OFF"),
                ("MAP", "ON"),
                ("AMOUNT OF BLOOD", "100%"),
                ("LOAD+CHANGE", "ON"),
                ("REGENERATE LEVEL", "OFF"),
                ("SAVE SETUP AS...", "liero"),
                ("LOAD SETUP", ""),
            ]
        );
        assert!(
            !m.items[10].has_value && !m.items[18].has_value,
            "WEAPON OPTIONS, LOAD SETUP: no value"
        );
        assert_eq!(m.visible_item_count, 15);
    }

    #[test]
    fn visibility_follows_the_game_mode_and_the_level_kind() {
        for (mode, random, count, extra) in [
            (GM_KILL_EM_ALL, true, 15, "LIVES"),
            (GM_SCALES_OF_JUSTICE, true, 15, "LIVES"),
            (GM_GAME_OF_TAG, true, 15, "TIME TO LOSE"),
            (GM_HOLDAZONE, true, 16, "ZONE TIMEOUT"),
            (GM_KILL_EM_ALL, false, 13, "LIVES"),
            (GM_HOLDAZONE, false, 14, "TIME TO WIN"),
        ] {
            let mut s = Settings {
                game_mode: mode,
                random_level: random,
                ..Settings::default()
            };
            let m = updated(&mut s);
            assert_eq!(m.visible_item_count, count, "mode {mode} random {random}");
            assert!(shown(&m).contains(&extra));
            assert!(
                !shown(&m).contains(&"FLAGS TO WIN"),
                "never shown (gfx.cpp:1319)"
            );
            assert_eq!(shown(&m).contains(&"MAP WIDTH"), random);
        }
    }

    #[test]
    fn a_file_level_shows_its_quoted_basename_and_relabels_regenerate() {
        let mut s = Settings {
            random_level: false,
            level_file: "Levels/water_stage.lev".into(),
            ..Settings::default()
        };
        let m = updated(&mut s);
        let level = m.item_from_id(SI_LEVEL).unwrap();
        assert_eq!(level.value, "\"water_stage\"", "gfx.cpp:1227");
        assert_eq!(
            m.item_from_id(SI_REGENERATE_LEVEL).unwrap().string,
            "RELOAD LEVEL"
        );
        let mut s = Settings::default();
        assert_eq!(
            updated(&mut s)
                .item_from_id(SI_REGENERATE_LEVEL)
                .unwrap()
                .string,
            "REGENERATE LEVEL"
        );
    }

    #[test]
    fn odd_values_render_as_cpp_does() {
        let mut s = Settings {
            game_mode: GM_GAME_OF_TAG,
            time_to_lose: 3599,
            loading_time: 9999,
            blood: 0,
            ..Settings::default()
        };
        let m = updated(&mut s);
        let v = |id| m.item_from_id(id).unwrap().value.clone();
        assert_eq!(
            (
                v(SI_TIME_TO_LOSE),
                v(SI_LOADING_TIMES),
                v(SI_AMOUNT_OF_BLOOD)
            ),
            ("59:59".into(), "9999%".into(), "0%".into())
        );
    }

    #[test]
    fn the_behaviors_edit_the_settings() {
        let tc = tc();
        let mut s = Settings::default();
        let mut m = settings_menu();
        m.move_to_first_visible();
        let mut sounds = Vec::new();
        let mut cx = MenuCx {
            menu_cycles: 0,
            hooks: tc.hooks,
            sounds: &mut sounds,
        };
        let mut model = SettingsModel {
            settings: &mut s,
            tc: &tc,
            setup_name: "liero",
        };
        m.update_items(&mut model);
        m.on_left_right(&mut model, 1, &mut cx); // GAME MODE: Kill'em All -> Game of Tag
        assert!(
            m.item_from_id(SI_TIME_TO_LOSE).unwrap().visible,
            "Change -> UpdateItems -> OnUpdate"
        );
        m.move_to_id(SI_AMOUNT_OF_BLOOD);
        m.on_left_right(&mut model, 1, &mut cx);
        assert_eq!(
            m.on_enter(&mut model, &mut cx),
            Enter::Result(-1),
            "blood: allow_entry = false"
        );
        m.move_to_id(SI_LIVES);
        assert!(
            m.selected_id() != SI_LIVES,
            "LIVES is hidden in Game of Tag"
        );
        drop(model);
        assert_eq!(
            (s.game_mode, s.blood),
            (GM_GAME_OF_TAG, 125),
            "BloodStepUp = 25 (tc.cfg:70)"
        );
    }
}
