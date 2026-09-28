//! Step 4½a-1 — the C++ `Settings` / `WormSettings` model (`settings.hpp:10-102`,
//! `worm.hpp:44-118`) with the C++ member names and defaults (`settings.cpp:17-60`,
//! `worm.hpp:61-95`, `worm.cpp:21-32`), plus the [`MatchConfig`] the builder consumes
//! (design §2). Integer widths are the C++ widths (`int`/`int32_t` -> `i32`,
//! `uint32_t` -> `u32`) so the reader's `static_cast` truncation
//! (`toml_archive.hpp:224-235`) is a plain `as`. Pure data — no I/O here.

/// `Settings::kSelectableWeapons` (`settings.hpp:53`) == C++ `NUM_WEAPONS` (`worm.hpp:13`).
pub const SELECTABLE_WEAPONS: usize = 5;
/// `Settings::weap_table[40]` (`settings.hpp:68`), indexed by weapon index (not `weap_order`).
pub const WEAP_TABLE_LEN: usize = 40;
/// `Settings::kNumWormSettings` (`settings.hpp:92`): 0 = left, 1 = right, 2 = network.
pub const NUM_WORM_SETTINGS: usize = 3;
/// `Settings::kNetworkPlayerIdx` (`settings.hpp:93`).
pub const NETWORK_PLAYER_IDX: usize = 2;
/// `Settings::kConfigVersion` (`settings.hpp:98`) — what `[settings].version` is saved as.
pub const CONFIG_VERSION: i32 = 6;
/// `WormSettingsExtensions::kMaxControl` (`worm.hpp:54`): the seven classic controls.
pub const MAX_CONTROL: usize = 7;
/// `WormSettingsExtensions::kMaxControlEx` (`worm.hpp:55`): the seven + DIG.
pub const MAX_CONTROL_EX: usize = 8;

/// `Settings::GameModes` (`settings.hpp:51`).
pub const GM_KILL_EM_ALL: u32 = 0;
pub const GM_GAME_OF_TAG: u32 = 1;
pub const GM_HOLDAZONE: u32 = 2;
pub const GM_SCALES_OF_JUSTICE: u32 = 3;

/// `InitDefaultGamepadControls` (`worm.cpp:21-32`) as SDL3 enum values: DPAD_UP 11,
/// DPAD_DOWN 12, DPAD_LEFT 13, DPAD_RIGHT 14, fire = `GamepadAxisPositive(RIGHT_TRIGGER
/// = 5)` = 100 + 5*2 = 110, change = RIGHT_SHOULDER 10, jump = SOUTH 0, dig =
/// LEFT_SHOULDER 9 — the `gamepadControls` array every shipped file carries.
pub const DEFAULT_GAMEPAD_CONTROLS: [u32; MAX_CONTROL_EX] = [11, 12, 13, 14, 110, 10, 0, 9];

/// Default DOS scancodes (`settings.cpp:36-37`): left player, right player.
const DEF_CONTROLS: [[u32; MAX_CONTROL]; 2] = [
    [0x13, 0x21, 0x20, 0x22, 0x1D, 0x2A, 0x38],
    [0xA0, 0xA8, 0xA3, 0xA5, 0x75, 0x90, 0x36],
];
/// Default 8-bit RGB (`settings.cpp:39`): left player, right player.
const DEF_RGB: [[i32; 3]; 2] = [[104, 104, 252], [60, 172, 60]];

/// C++ `WormSettings` (`worm.hpp:85-118`) + its `WormSettingsExtensions` base
/// (`:44-83`). `profile_node` (a filesystem handle) and `hash` (a cache) are runtime
/// state, not data, and are not modelled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WormSettings {
    pub health: i32,
    /// 0 human, 1 DumbLieroAI, 2 FollowAI (`localController.cpp:19-27`).
    pub controller: u32,
    /// DOS scancodes for Up/Down/Left/Right/Fire/Change/Jump.
    pub controls: [u32; MAX_CONTROL],
    /// `controls` + DIG.
    pub controls_ex: [u32; MAX_CONTROL_EX],
    /// 0..99 SDL button, `100 + axis*2 (+1)` axis (`worm.hpp:72-77`).
    pub gamepad_controls: [u32; MAX_CONTROL_EX],
    /// 0 keyboard, 1 gamepad 0, 2 gamepad 1, … (`worm.hpp:58-59`).
    pub input_device: u32,
    pub gamepad_name: String,
    pub gamepad_serial: String,
    /// 1-based `weap_order` indices (`worm.cpp:704`).
    pub weapons: [u32; SELECTABLE_WEAPONS],
    pub name: String,
    /// 0..255 per channel (`worm.hpp:109`).
    pub rgb: [i32; 3],
    pub random_name: bool,
    pub color: i32,
}

impl Default for WormSettings {
    /// `WormSettings()` over `WormSettingsExtensions()` (`worm.hpp:61-66`, `:86-95`).
    fn default() -> Self {
        WormSettings {
            health: 100,
            controller: 0,
            controls: [0; MAX_CONTROL],
            controls_ex: [0; MAX_CONTROL_EX],
            gamepad_controls: DEFAULT_GAMEPAD_CONTROLS,
            input_device: 0,
            gamepad_name: String::new(),
            gamepad_serial: String::new(),
            weapons: [1; SELECTABLE_WEAPONS],
            name: String::new(),
            rgb: [104, 104, 248],
            random_name: true,
            color: 0,
        }
    }
}

/// C++ `Settings` (`settings.hpp:50-102`) = `GameplayExtensions` (`:11-28`) +
/// `AppSettings` (`:31-46`) + its own fields, in C++ declaration order. `hash` is a
/// cache and not modelled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    // --- GameplayExtensions (hashed + replayed) ---
    pub record_replays: bool,
    pub load_powerlevel_palette: bool,
    pub ai_frames: i32,
    pub ai_mutations: i32,
    pub ai_traces: bool,
    pub ai_parallels: i32,
    pub zone_timeout: i32,
    pub select_bot_weapons: u32,
    pub allow_viewing_spawn_point: bool,
    pub tc: String,
    // --- AppSettings (not hashed) ---
    pub fullscreen: bool,
    pub single_screen_replay: bool,
    pub spectator_window: bool,
    pub blood_particle_max: i32,
    pub modern_colors: bool,
    pub max_spectator_render_height: i32,
    // --- Settings ---
    pub weap_table: [u32; WEAP_TABLE_LEN],
    pub max_bonuses: i32,
    pub blood: i32,
    pub time_to_lose: i32,
    pub flags_to_win: i32,
    pub game_mode: u32,
    pub shadow: bool,
    pub load_change: bool,
    pub names_on_bonuses: bool,
    pub regenerate_level: bool,
    pub lives: i32,
    pub loading_time: i32,
    pub random_level: bool,
    pub level_file: String,
    pub map: bool,
    pub screen_sync: bool,
    pub bonus_timeout: i32,
    pub input_delay: i32,
    pub random_map_width: i32,
    pub random_map_height: i32,
    /// 0 = left, 1 = right, 2 = network (`settings.hpp:92-99`).
    pub worm_settings: [WormSettings; NUM_WORM_SETTINGS],
}

impl Default for Settings {
    /// `Settings::Settings()` (`settings.cpp:23-60`) over the in-class initialisers.
    fn default() -> Self {
        let mut worm_settings = [
            WormSettings::default(),
            WormSettings::default(),
            WormSettings::default(),
        ];
        worm_settings[0].color = 32;
        worm_settings[1].color = 41;
        worm_settings[2].color = 32;
        for (i, ws) in worm_settings.iter_mut().take(2).enumerate() {
            ws.controls = DEF_CONTROLS[i];
            ws.controls_ex[..MAX_CONTROL].copy_from_slice(&DEF_CONTROLS[i]);
            ws.rgb = DEF_RGB[i];
        }
        // The network player defaults to the left player's controls and colour (:52-59).
        worm_settings[2].controls = DEF_CONTROLS[0];
        worm_settings[2].controls_ex[..MAX_CONTROL].copy_from_slice(&DEF_CONTROLS[0]);
        worm_settings[2].rgb = DEF_RGB[0];

        Settings {
            record_replays: true,
            load_powerlevel_palette: true,
            ai_frames: 70 * 2,
            ai_mutations: 2,
            ai_traces: false,
            ai_parallels: 3,
            zone_timeout: 30,
            select_bot_weapons: 1,
            allow_viewing_spawn_point: false,
            tc: "openliero".to_string(),
            fullscreen: false,
            single_screen_replay: false,
            spectator_window: false,
            blood_particle_max: 700,
            modern_colors: false,
            max_spectator_render_height: 1080,
            weap_table: [0; WEAP_TABLE_LEN],
            max_bonuses: 4,
            blood: 100,
            time_to_lose: 600,
            flags_to_win: 20,
            game_mode: GM_KILL_EM_ALL,
            shadow: true,
            load_change: true,
            names_on_bonuses: false,
            regenerate_level: false,
            lives: 15,
            loading_time: 100,
            random_level: true,
            level_file: String::new(),
            map: true,
            screen_sync: true,
            bonus_timeout: 0,
            input_delay: 1,
            random_map_width: 504,
            random_map_height: 350,
            worm_settings,
        }
    }
}

/// A configured match: the settings plus the match seed. The seed seeds the sim
/// `Rand` (overview LD 6); C++ single-player seeds it implicitly, so it is not a
/// `Settings` field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchConfig {
    pub settings: Settings,
    pub seed: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worm_settings_default_is_the_cpp_constructor() {
        // worm.hpp:61-66 (extensions) + :86-95, :104-112; worm.cpp:21-32 (gamepad).
        let ws = WormSettings::default();
        assert_eq!(ws.health, 100);
        assert_eq!(ws.controller, 0);
        assert_eq!(ws.controls, [0; 7]);
        assert_eq!(ws.controls_ex, [0; 8]);
        assert_eq!(ws.gamepad_controls, [11, 12, 13, 14, 110, 10, 0, 9]);
        assert_eq!(ws.input_device, 0);
        assert!(ws.gamepad_name.is_empty() && ws.gamepad_serial.is_empty() && ws.name.is_empty());
        assert_eq!(ws.weapons, [1; 5]);
        assert_eq!(
            ws.rgb,
            [104, 104, 248],
            "bare WormSettings() blue is 248, not 252"
        );
        assert!(ws.random_name);
        assert_eq!(ws.color, 0);
    }

    #[test]
    fn settings_default_is_the_cpp_constructor() {
        // settings.hpp:16-28, :34-45, :68-90 in-class initialisers; settings.cpp:23-60.
        let s = Settings::default();
        assert!(s.record_replays && s.load_powerlevel_palette && !s.ai_traces);
        assert_eq!((s.ai_frames, s.ai_mutations, s.ai_parallels), (140, 2, 3));
        assert_eq!(s.zone_timeout, 30);
        assert_eq!(
            s.select_bot_weapons, 1,
            "uint32_t select_bot_weapons{{true}}"
        );
        assert!(!s.allow_viewing_spawn_point);
        assert_eq!(s.tc, "openliero");
        assert!(
            !s.fullscreen && !s.single_screen_replay && !s.spectator_window && !s.modern_colors
        );
        assert_eq!(s.blood_particle_max, 700);
        assert_eq!(s.max_spectator_render_height, 1080);
        assert_eq!(s.weap_table, [0; WEAP_TABLE_LEN]);
        assert_eq!(
            (s.max_bonuses, s.blood, s.time_to_lose, s.flags_to_win),
            (4, 100, 600, 20)
        );
        assert_eq!(s.game_mode, GM_KILL_EM_ALL);
        assert!(s.shadow && s.load_change && !s.names_on_bonuses && !s.regenerate_level);
        assert_eq!((s.lives, s.loading_time), (15, 100));
        assert!(s.random_level && s.level_file.is_empty() && s.map && s.screen_sync);
        assert_eq!((s.bonus_timeout, s.input_delay), (0, 1));
        assert_eq!((s.random_map_width, s.random_map_height), (504, 350));

        let [p1, p2, net] = &s.worm_settings;
        assert_eq!((p1.color, p2.color, net.color), (32, 41, 32));
        assert_eq!(p1.controls, [0x13, 0x21, 0x20, 0x22, 0x1D, 0x2A, 0x38]);
        assert_eq!(
            p1.controls_ex,
            [0x13, 0x21, 0x20, 0x22, 0x1D, 0x2A, 0x38, 0]
        );
        assert_eq!(p2.controls, [0xA0, 0xA8, 0xA3, 0xA5, 0x75, 0x90, 0x36]);
        assert_eq!(
            p2.controls_ex,
            [0xA0, 0xA8, 0xA3, 0xA5, 0x75, 0x90, 0x36, 0]
        );
        assert_eq!(
            net.controls, p1.controls,
            "network player clones the left controls"
        );
        assert_eq!(net.controls_ex, p1.controls_ex);
        assert_eq!(p1.rgb, [104, 104, 252]);
        assert_eq!(p2.rgb, [60, 172, 60]);
        assert_eq!(net.rgb, [104, 104, 252]);
        for ws in &s.worm_settings {
            assert_eq!(ws.health, 100);
            assert_eq!(ws.weapons, [1; 5]);
            assert_eq!(ws.gamepad_controls, DEFAULT_GAMEPAD_CONTROLS);
        }
    }

    #[test]
    fn game_mode_and_layout_constants_match_cpp() {
        assert_eq!(
            (
                GM_KILL_EM_ALL,
                GM_GAME_OF_TAG,
                GM_HOLDAZONE,
                GM_SCALES_OF_JUSTICE
            ),
            (0, 1, 2, 3)
        );
        assert_eq!(
            (SELECTABLE_WEAPONS, WEAP_TABLE_LEN, NUM_WORM_SETTINGS),
            (5, 40, 3)
        );
        assert_eq!(
            (
                NETWORK_PLAYER_IDX,
                CONFIG_VERSION,
                MAX_CONTROL,
                MAX_CONTROL_EX
            ),
            (2, 6, 7, 8)
        );
    }
}
