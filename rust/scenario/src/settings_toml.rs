//! Step 4½a-1 — the C++ setup/profile TOML **reader** (`Settings::FromToml`,
//! `settings.cpp:133-156`; `WormSettings::LoadProfile`, `worm.cpp:73-95`), design §3.2.
//!
//! Parsing is the `toml` 0.8 crate; the SEMANTICS are hand-written to mirror cereal's
//! `TomlInputArchive` (`toml_archive.hpp:158-317`) through a [`Frame`] that is a direct
//! transcription of its `Frame`/`Lookup`/`Advance`: a missing or wrongly-typed key
//! leaves the prior value, integers narrow like `static_cast`, arrays are positional,
//! a missing/scalar worm table behaves like an empty one (an *array*-valued one is read
//! positionally, as the C++ does), and the `rgbDepth` marker drives the legacy 6-bit
//! expansion (`cereal_types.hpp:288-303`).
//!
//! Step 4½a-2 adds the **writer** (`Settings::ToToml`, `WormSettings::ToToml`, the
//! `SerializeGameplay` subset — design §3.3): the keys `TomlOutputArchive` inserts, laid out by
//! the toml++ formatter port in `crate::toml_fmt`, plus the archive's closing `"\n"`.

use std::collections::BTreeMap;

use toml::{Table, Value};

use crate::settings::{Settings, WormSettings, CONFIG_VERSION};
use crate::toml_fmt::{format_doc, KeyVals, Val};

/// A TOML parse failure (C++ `TomlParseError`, `toml_archive.hpp:154-156`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TomlError(pub String);

impl std::fmt::Display for TomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TOML parse error: {}", self.0)
    }
}

impl std::error::Error for TomlError {}

/// The worm tables in `Settings::FromToml` order (`settings.cpp:146`).
pub const WORM_TABLE_NAMES: [&str; 3] = ["player1", "player2", "network_player"];

/// The `TomlInputArchive` constructor (`toml_archive.hpp:161-173`): parse, or fail
/// before anything is assigned.
fn parse_root(text: &str) -> Result<Value, TomlError> {
    toml::from_str::<Table>(text)
        .map(Value::Table)
        .map_err(|e| TomlError(e.to_string()))
}

/// A `TomlInputArchive::Frame` (`toml_archive.hpp:271-275`): the current node — `None`
/// when `startNode` found no child — and the next array slot to consume.
struct Frame<'a> {
    node: Option<&'a Value>,
    index: usize,
}

impl<'a> Frame<'a> {
    fn new(node: Option<&'a Value>) -> Self {
        Frame { node, index: 0 }
    }

    /// `Lookup` (`:281-309`): a table frame finds `name` (an unnamed read finds nothing);
    /// an array frame IGNORES the name and yields slot `index` (if in range); a null or
    /// scalar frame yields nothing.
    fn lookup(&self, name: Option<&str>) -> Option<&'a Value> {
        match self.node? {
            Value::Table(t) => t.get(name?),
            Value::Array(a) => a.get(self.index),
            _ => None,
        }
    }

    /// `Advance` (`:311-316`): only an array frame moves on.
    fn advance(&mut self) {
        if let Some(Value::Array(_)) = self.node {
            self.index += 1;
        }
    }

    /// The lookup + advance of one `loadValue` (`:217-268`) — the index advances whether
    /// or not the slot held the right type.
    fn load(&mut self, name: Option<&str>) -> Option<&'a Value> {
        let v = self.lookup(name);
        self.advance();
        v
    }

    /// `setNextName(name); startNode()` (`:175-186`) plus the parent half of
    /// `finishNode` (`:188-202`), which advances an array parent (its `next_name_` is
    /// always null by then: every `Lookup` clears it). Nothing reads the parent while
    /// the child is open, so advancing it here rather than at `finishNode` is equivalent.
    fn child(&mut self, name: &str) -> Frame<'a> {
        let c = Frame::new(self.lookup(Some(name)));
        self.advance();
        c
    }
}

/// `loadValue(bool&)` (`:217-222`): only a boolean node assigns.
fn rd_bool(f: &mut Frame<'_>, key: &str, dst: &mut bool) {
    if let Some(Value::Boolean(v)) = f.load(Some(key)) {
        *dst = *v;
    }
}

/// `loadValue(int32_t&)` (`:224-229`): only an integer node assigns, `static_cast`.
fn rd_i32(f: &mut Frame<'_>, key: &str, dst: &mut i32) {
    if let Some(Value::Integer(v)) = f.load(Some(key)) {
        *dst = *v as i32;
    }
}

/// `loadValue(uint32_t&)` (`:230-235`).
fn rd_u32(f: &mut Frame<'_>, key: &str, dst: &mut u32) {
    if let Some(Value::Integer(v)) = f.load(Some(key)) {
        *dst = *v as u32;
    }
}

/// `loadValue(std::string&)` (`:263-268`).
fn rd_string(f: &mut Frame<'_>, key: &str, dst: &mut String) {
    if let Some(Value::String(v)) = f.load(Some(key)) {
        dst.clone_from(v);
    }
}

/// `SerializeArray` on load (`cereal_types.hpp:51-60`): a child frame, then all `N`
/// unnamed reads; slot `i` assigns only when the child is an array with an integer at
/// `i` (the index advances regardless). The size tag (`LoadSize`) is read and unused.
fn rd_array<T, const N: usize>(f: &mut Frame<'_>, key: &str, dst: &mut [T; N], cast: fn(i64) -> T) {
    let mut arr = f.child(key);
    for slot in dst.iter_mut() {
        if let Some(Value::Integer(v)) = arr.load(None) {
            *slot = cast(*v);
        }
    }
}

/// `SerializeWormSettingsToml` on load (`cereal_types.hpp:282-308`), in its exact field
/// order (which matters for an array frame).
fn read_worm(f: &mut Frame<'_>, ws: &mut WormSettings) {
    rd_string(f, "name", &mut ws.name);
    rd_i32(f, "health", &mut ws.health);
    rd_u32(f, "controller", &mut ws.controller);
    rd_bool(f, "randomName", &mut ws.random_name);
    rd_i32(f, "color", &mut ws.color);
    rd_u32(f, "inputDevice", &mut ws.input_device);
    rd_string(f, "gamepadName", &mut ws.gamepad_name);
    rd_string(f, "gamepadSerial", &mut ws.gamepad_serial);
    // Files without the marker predate 8-bit colours (default 6 on load, :290-294).
    let mut rgb_depth: i32 = 6;
    rd_i32(f, "rgbDepth", &mut rgb_depth);
    rd_array(f, "rgb", &mut ws.rgb, |v| v as i32);
    for v in ws.rgb.iter_mut() {
        if rgb_depth < 8 {
            *v = (*v & 63) << 2;
        }
        *v = (*v).clamp(0, 255);
    }
    rd_array(f, "weapons", &mut ws.weapons, |v| v as u32);
    rd_array(f, "controls", &mut ws.controls, |v| v as u32);
    rd_array(f, "controlsEx", &mut ws.controls_ex, |v| v as u32);
    rd_array(f, "gamepadControls", &mut ws.gamepad_controls, |v| v as u32);
}

/// `Settings::FromToml` over an existing value (`settings.cpp:133-156`). On a parse
/// error `s` is untouched (C++ throws before any assignment).
pub fn read_settings_into(text: &str, s: &mut Settings) -> Result<(), TomlError> {
    let root = parse_root(text)?;
    let mut top = Frame::new(Some(&root));
    let mut st = top.child("settings");
    // `version` is read into a local and discarded (:139-140) — but it still consumes a
    // slot when `settings` is an array.
    let mut version: i32 = 0;
    rd_i32(&mut st, "version", &mut version);
    rd_bool(&mut st, "modernColors", &mut s.modern_colors);
    // SerializeSettingsScalars (cereal_types.hpp:161-194).
    rd_bool(&mut st, "recordReplays", &mut s.record_replays);
    rd_bool(
        &mut st,
        "loadPowerlevelPalette",
        &mut s.load_powerlevel_palette,
    );
    rd_i32(&mut st, "aiFrames", &mut s.ai_frames);
    rd_i32(&mut st, "aiMutations", &mut s.ai_mutations);
    rd_bool(&mut st, "aiTraces", &mut s.ai_traces);
    rd_i32(&mut st, "aiParallels", &mut s.ai_parallels);
    rd_i32(&mut st, "zoneTimeout", &mut s.zone_timeout);
    rd_u32(&mut st, "selectBotWeapons", &mut s.select_bot_weapons);
    rd_bool(
        &mut st,
        "allowViewingSpawnPoint",
        &mut s.allow_viewing_spawn_point,
    );
    rd_string(&mut st, "tc", &mut s.tc);
    rd_bool(&mut st, "fullscreen", &mut s.fullscreen);
    rd_bool(&mut st, "singleScreenReplay", &mut s.single_screen_replay);
    rd_bool(&mut st, "spectatorWindow", &mut s.spectator_window);
    rd_i32(&mut st, "bloodParticleMax", &mut s.blood_particle_max);
    rd_i32(&mut st, "maxBonuses", &mut s.max_bonuses);
    rd_i32(&mut st, "blood", &mut s.blood);
    rd_i32(&mut st, "timeToLose", &mut s.time_to_lose);
    rd_i32(&mut st, "flagsToWin", &mut s.flags_to_win);
    rd_u32(&mut st, "gameMode", &mut s.game_mode);
    rd_bool(&mut st, "shadow", &mut s.shadow);
    rd_bool(&mut st, "loadChange", &mut s.load_change);
    rd_bool(&mut st, "namesOnBonuses", &mut s.names_on_bonuses);
    rd_bool(&mut st, "regenerateLevel", &mut s.regenerate_level);
    rd_i32(&mut st, "lives", &mut s.lives);
    rd_i32(&mut st, "loadingTime", &mut s.loading_time);
    rd_bool(&mut st, "randomLevel", &mut s.random_level);
    rd_string(&mut st, "levelFile", &mut s.level_file);
    rd_bool(&mut st, "map", &mut s.map);
    rd_bool(&mut st, "screenSync", &mut s.screen_sync);
    rd_i32(&mut st, "bonusTimeout", &mut s.bonus_timeout);
    rd_i32(&mut st, "inputDelay", &mut s.input_delay);
    rd_i32(&mut st, "randomMapWidth", &mut s.random_map_width);
    rd_i32(&mut st, "randomMapHeight", &mut s.random_map_height);
    rd_i32(
        &mut st,
        "maxSpectatorRenderHeight",
        &mut s.max_spectator_render_height,
    );
    rd_array(&mut st, "weapTable", &mut s.weap_table, |v| v as u32);
    for (i, name) in WORM_TABLE_NAMES.iter().enumerate() {
        read_worm(&mut top.child(name), &mut s.worm_settings[i]);
    }
    Ok(())
}

/// `Settings::FromToml` over `Settings::default()` — what `Settings::load` does on a
/// fresh `Settings` (`settings.cpp:62-90`).
pub fn settings_from_toml(text: &str) -> Result<Settings, TomlError> {
    let mut s = Settings::default();
    read_settings_into(text, &mut s)?;
    Ok(s)
}

/// `WormSettings::LoadProfile` (`worm.cpp:73-95`): the profile keys sit at the root;
/// the pre-load `color` is restored; on a parse error `ws` is untouched.
pub fn load_profile(text: &str, ws: &mut WormSettings) -> Result<(), TomlError> {
    let root = parse_root(text)?;
    let old_color = ws.color;
    read_worm(&mut Frame::new(Some(&root)), ws);
    ws.color = old_color;
    Ok(())
}

// --- Writer (Step 4½a-2, design §3.3) ----------------------------------------------------
// The key ORDER below follows cereal_types.hpp for review only: `toml::table` sorts keys, so
// the BTreeMap decides the byte order, exactly as in C++.

/// An integer array as `TomlOutputArchive` stores it: every element widened to `int64`
/// (`toml_archive.hpp:71-74`; `uint32_t` values are therefore never negative).
fn ints<'a, T: Copy + Into<i64>>(a: &[T]) -> Val<'a> {
    Val::Ints(a.iter().map(|&v| v.into()).collect())
}

/// `SerializeWormSettingsToml` on save (`cereal_types.hpp:282-308`): `rgbDepth` is 8.
fn worm_keys(ws: &WormSettings) -> KeyVals<'_> {
    let mut kv = KeyVals::new();
    kv.insert("name", Val::Str(&ws.name));
    kv.insert("health", Val::Int(ws.health.into()));
    kv.insert("controller", Val::Int(ws.controller.into()));
    kv.insert("randomName", Val::Bool(ws.random_name));
    kv.insert("color", Val::Int(ws.color.into()));
    kv.insert("inputDevice", Val::Int(ws.input_device.into()));
    kv.insert("gamepadName", Val::Str(&ws.gamepad_name));
    kv.insert("gamepadSerial", Val::Str(&ws.gamepad_serial));
    kv.insert("rgbDepth", Val::Int(8));
    kv.insert("rgb", ints(&ws.rgb));
    kv.insert("weapons", ints(&ws.weapons));
    kv.insert("controls", ints(&ws.controls));
    kv.insert("controlsEx", ints(&ws.controls_ex));
    kv.insert("gamepadControls", ints(&ws.gamepad_controls));
    kv
}

/// `SerializeGameplay` (`cereal_types.hpp:218-239`) — the `UpdateHash` subset: the
/// `GameplayExtensions`, the gameplay scalars, `weapTable`, `bonusTimeout`, `inputDelay`.
/// `ToToml`'s `[settings]` is this plus nine more keys (`settings_to_toml`).
fn gameplay_keys(s: &Settings) -> KeyVals<'_> {
    let mut kv = KeyVals::new();
    kv.insert("recordReplays", Val::Bool(s.record_replays));
    kv.insert(
        "loadPowerlevelPalette",
        Val::Bool(s.load_powerlevel_palette),
    );
    kv.insert("aiFrames", Val::Int(s.ai_frames.into()));
    kv.insert("aiMutations", Val::Int(s.ai_mutations.into()));
    kv.insert("aiTraces", Val::Bool(s.ai_traces));
    kv.insert("aiParallels", Val::Int(s.ai_parallels.into()));
    kv.insert("zoneTimeout", Val::Int(s.zone_timeout.into()));
    kv.insert("selectBotWeapons", Val::Int(s.select_bot_weapons.into()));
    kv.insert(
        "allowViewingSpawnPoint",
        Val::Bool(s.allow_viewing_spawn_point),
    );
    kv.insert("tc", Val::Str(&s.tc));
    kv.insert("maxBonuses", Val::Int(s.max_bonuses.into()));
    kv.insert("blood", Val::Int(s.blood.into()));
    kv.insert("timeToLose", Val::Int(s.time_to_lose.into()));
    kv.insert("flagsToWin", Val::Int(s.flags_to_win.into()));
    kv.insert("gameMode", Val::Int(s.game_mode.into()));
    kv.insert("shadow", Val::Bool(s.shadow));
    kv.insert("loadChange", Val::Bool(s.load_change));
    kv.insert("namesOnBonuses", Val::Bool(s.names_on_bonuses));
    kv.insert("regenerateLevel", Val::Bool(s.regenerate_level));
    kv.insert("lives", Val::Int(s.lives.into()));
    kv.insert("loadingTime", Val::Int(s.loading_time.into()));
    kv.insert("randomLevel", Val::Bool(s.random_level));
    kv.insert("levelFile", Val::Str(&s.level_file));
    kv.insert("map", Val::Bool(s.map));
    kv.insert("screenSync", Val::Bool(s.screen_sync));
    kv.insert("weapTable", ints(&s.weap_table));
    kv.insert("bonusTimeout", Val::Int(s.bonus_timeout.into()));
    kv.insert("inputDelay", Val::Int(s.input_delay.into()));
    kv
}

/// `~TomlOutputArchive` (`toml_archive.hpp:47`): `out_ << root_ << "\n"`.
fn archive_bytes(values: &KeyVals<'_>, tables: &BTreeMap<&'static str, KeyVals<'_>>) -> String {
    let mut out = format_doc(values, tables);
    out.push('\n');
    out
}

/// `Settings::ToToml` (`settings.cpp:103-131`) — the bytes `Settings::save` writes: a
/// `[settings]` table (`version` = `kConfigVersion`, `modernColors`,
/// `SerializeSettingsScalars`, `weapTable`) and the three worm tables.
pub fn settings_to_toml(s: &Settings) -> String {
    let mut st = gameplay_keys(s);
    st.insert("version", Val::Int(CONFIG_VERSION.into()));
    st.insert("modernColors", Val::Bool(s.modern_colors));
    st.insert("fullscreen", Val::Bool(s.fullscreen));
    st.insert("singleScreenReplay", Val::Bool(s.single_screen_replay));
    st.insert("spectatorWindow", Val::Bool(s.spectator_window));
    st.insert("bloodParticleMax", Val::Int(s.blood_particle_max.into()));
    st.insert("randomMapWidth", Val::Int(s.random_map_width.into()));
    st.insert("randomMapHeight", Val::Int(s.random_map_height.into()));
    st.insert(
        "maxSpectatorRenderHeight",
        Val::Int(s.max_spectator_render_height.into()),
    );
    let mut tables = BTreeMap::new();
    tables.insert("settings", st);
    for (name, ws) in WORM_TABLE_NAMES.iter().zip(&s.worm_settings) {
        tables.insert(*name, worm_keys(ws));
    }
    archive_bytes(&KeyVals::new(), &tables)
}

/// `WormSettings::ToToml` (`worm.cpp:45-52`) — the bytes `SaveProfile` writes (`:60-71`):
/// the profile keys at the root.
pub fn worm_settings_to_toml(ws: &WormSettings) -> String {
    archive_bytes(&worm_keys(ws), &BTreeMap::new())
}

/// The `SerializeGameplay` TOML `Settings::UpdateHash` hashes (`settings.cpp:92-98`): the
/// gameplay keys at the root.
pub fn gameplay_toml(s: &Settings) -> String {
    archive_bytes(&gameplay_keys(s), &BTreeMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, WormSettings};

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

    fn shipped(rel: &str) -> String {
        std::fs::read_to_string(format!("{DATA}/{rel}"))
            .unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

    #[test]
    fn shipped_liero_cfg_reads_as_exactly_the_cpp_defaults() {
        // A legacy v5 file: no rgbDepth (6-bit 26,26,63 / 15,43,15 expand to the default
        // 104,104,252 / 60,172,60), no maxSpectatorRenderHeight (keeps 1080).
        let s = settings_from_toml(&shipped("Setups/liero.cfg")).expect("liero.cfg parses");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn shipped_orbmit_cfg_differs_from_the_defaults_in_four_fields() {
        let s = settings_from_toml(&shipped("Setups/orbmit.cfg")).expect("orbmit.cfg parses");
        let mut want = Settings::default();
        want.blood = 25;
        want.lives = 9;
        want.loading_time = 20;
        want.max_bonuses = 0;
        assert_eq!(s, want);
    }

    #[test]
    fn shipped_ai_profile_loads_expands_rgb_and_keeps_the_colour() {
        let mut ws = Settings::default().worm_settings[0].clone(); // colour 32
        load_profile(&shipped("Profiles/AI (L).toml"), &mut ws).expect("profile parses");
        assert_eq!(ws.name, "AI Joe");
        assert_eq!(ws.health, 100);
        assert_eq!(ws.controller, 2);
        assert!(!ws.random_name);
        assert_eq!(
            ws.color, 32,
            "LoadProfile restores the pre-load colour (worm.cpp:94)"
        );
        assert_eq!(ws.input_device, 0);
        assert_eq!(ws.gamepad_name, "");
        assert_eq!(ws.rgb, [80, 80, 160], "no rgbDepth => 6-bit: (v & 63) << 2");
        assert_eq!(ws.weapons, [19, 31, 40, 29, 36]);
        assert_eq!(ws.controls, [17, 31, 30, 32, 20, 21, 22]);
        assert_eq!(ws.controls_ex, [17, 31, 30, 32, 20, 21, 22, 0]);
        assert_eq!(ws.gamepad_controls, [11, 12, 13, 14, 110, 10, 0, 9]);
    }

    #[test]
    fn every_shipped_profile_parses() {
        for name in [
            "AI (L)",
            "AI (R)",
            "Joystick0",
            "Joystick1",
            "Lefty (L)",
            "Lefty (R)",
            "Righty (L)",
            "Righty (R)",
        ] {
            let mut ws = WormSettings::default();
            load_profile(&shipped(&format!("Profiles/{name}.toml")), &mut ws)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                !ws.name.is_empty(),
                "{name}: every shipped profile is named"
            );
        }
        let mut joy = WormSettings::default();
        load_profile(&shipped("Profiles/Joystick1.toml"), &mut joy).unwrap();
        assert_eq!(joy.input_device, 1);
        assert_eq!(joy.rgb, [104, 104, 252]);
        assert_eq!(joy.controls, [160, 33, 32, 34, 29, 42, 56]);
    }

    #[test]
    fn missing_and_wrongly_typed_keys_keep_their_prior_values() {
        let text = shipped("Setups/liero.cfg")
            .replace("lives = 15", "lives = \"nine\"")
            .replace("shadow = true", "shadow = 1")
            .replace("blood = 100", "blood = 42")
            .replace("zoneTimeout = 30\n", "");
        let s = settings_from_toml(&text).unwrap();
        let mut want = Settings::default();
        want.blood = 42;
        assert_eq!(s, want);
        // R14: a wrongly-typed key keeps a NON-default prior value too.
        let mut p = Settings::default();
        p.lives = 77;
        read_settings_into("[settings]\nlives = \"x\"\n", &mut p).unwrap();
        assert_eq!(p.lives, 77);
    }

    #[test]
    fn integers_truncate_like_static_cast() {
        // 4294967311 = 2^32 + 15 -> int32 15; -1 -> uint32 0xFFFF_FFFF.
        let text = shipped("Setups/liero.cfg")
            .replace("lives = 15", "lives = 4294967311")
            .replace("gameMode = 0", "gameMode = -1");
        let s = settings_from_toml(&text).unwrap();
        assert_eq!(s.lives, 15);
        assert_eq!(s.game_mode, u32::MAX);
    }

    #[test]
    fn arrays_are_positional_short_long_and_typed_per_element() {
        let text = "[settings]\nweapTable = [2, \"x\", 1]\n\
                    [player1]\nrgbDepth = 8\nweapons = [5, 6, 7, 8, 9, 10, 11]\n\
                    [player2]\nrgbDepth = 8\nweapons = [3]\n";
        let s = settings_from_toml(text).unwrap();
        assert_eq!(
            &s.weap_table[..4],
            &[2, 0, 1, 0],
            "\"x\" keeps 0 and the index still advances"
        );
        assert_eq!(
            s.worm_settings[0].weapons,
            [5, 6, 7, 8, 9],
            "extra elements ignored"
        );
        assert_eq!(
            s.worm_settings[1].weapons,
            [3, 1, 1, 1, 1],
            "missing tail kept"
        );
        let t = settings_from_toml("[settings]\nweapTable = 7\n").unwrap();
        assert_eq!(t.weap_table, [0; 40], "a non-array keeps the whole array");
    }

    #[test]
    fn the_rgb_depth_marker_controls_the_expansion_and_values_clamp() {
        let text = "[player1]\nrgbDepth = 8\nrgb = [300, -5, 70]\n\
                    [player2]\nrgbDepth = 7\nrgb = [63, 64, 1]\n\
                    [network_player]\nrgb = [10, 20, 30]\n";
        let s = settings_from_toml(text).unwrap();
        assert_eq!(s.worm_settings[0].rgb, [255, 0, 70], "8-bit: clamp only");
        assert_eq!(
            s.worm_settings[1].rgb,
            [252, 0, 4],
            "rgbDepth 7 < 8 => (v & 63) << 2"
        );
        assert_eq!(
            s.worm_settings[2].rgb,
            [40, 80, 120],
            "absent marker => 6-bit"
        );
    }

    #[test]
    fn a_missing_worm_table_still_expands_its_default_rgb() {
        // C++ quirk: startNode on a missing table gives a null frame, the rgbDepth read is a
        // no-op so rgb_depth stays 6, and the DEFAULT colour is then 6-bit expanded.
        let s = settings_from_toml("[settings]\nlives = 3\n").unwrap();
        assert_eq!(s.lives, 3);
        assert_eq!(s.worm_settings[0].rgb, [160, 160, 240]); // 104&63=40<<2, 252&63=60<<2
        assert_eq!(s.worm_settings[1].rgb, [240, 176, 240]); // 60->240, 172&63=44<<2=176
        assert_eq!(s.worm_settings[2].rgb, [160, 160, 240]);
        let t = settings_from_toml("player1 = 5\n").unwrap();
        assert_eq!(
            t.worm_settings[0].rgb,
            [160, 160, 240],
            "a non-table child = a missing one"
        );
        // ...except an ARRAY child: its frame is the array, and `Lookup` on an array frame
        // ignores the key and reads slot `index` (toml_archive.hpp:299-306), so the fields
        // are consumed positionally in serialization order — a nested array (rgb, weapons)
        // takes one slot too (finishNode advances the parent, :188-202).
        let u = settings_from_toml(
            "settings = [99, true, false]\n\
             player1 = [\"Pos\", 55, 7, false, 3, 1, 'g', 's', 8, [1, 2, 3], [9]]\n",
        )
        .unwrap();
        assert!(u.modern_colors, "slot 0 is version, slot 1 modernColors");
        assert!(!u.record_replays, "slot 2 recordReplays");
        assert_eq!(u.lives, 15, "no slot left for lives");
        let p1 = &u.worm_settings[0];
        assert_eq!((p1.name.as_str(), p1.health, p1.controller), ("Pos", 55, 7));
        assert!(!p1.random_name);
        assert_eq!((p1.color, p1.input_device), (3, 1));
        assert_eq!(
            (p1.gamepad_name.as_str(), p1.gamepad_serial.as_str()),
            ("g", "s")
        );
        assert_eq!(
            p1.rgb,
            [1, 2, 3],
            "slot 8 is rgbDepth = 8, slot 9 the rgb array"
        );
        assert_eq!(p1.weapons, [9, 1, 1, 1, 1], "slot 10 the weapons array");
        assert_eq!(p1.controls, Settings::default().worm_settings[0].controls);
    }

    #[test]
    fn version_is_read_and_discarded() {
        let text = shipped("Setups/liero.cfg").replace("version = 5", "version = 99");
        assert_eq!(settings_from_toml(&text).unwrap(), Settings::default());
    }

    #[test]
    fn a_parse_error_is_an_error_and_leaves_values_untouched() {
        assert!(settings_from_toml("[settings\nlives = 3").is_err());
        let mut s = Settings::default();
        s.lives = 77;
        assert!(read_settings_into("lives = ", &mut s).is_err());
        assert_eq!(s.lives, 77);
        let mut ws = WormSettings::default();
        ws.name = "keep".to_string();
        assert!(load_profile("name = \"x\"\nhealth = ", &mut ws).is_err());
        assert_eq!(ws.name, "keep");
    }

    #[test]
    fn profile_keys_are_read_at_the_root_and_the_colour_is_preserved() {
        let mut ws = WormSettings::default();
        ws.color = 41;
        load_profile(
            "color = 7\nname = 'x'\nrgbDepth = 8\nrgb = [1, 2, 3]\n",
            &mut ws,
        )
        .unwrap();
        assert_eq!(ws.color, 41);
        assert_eq!(ws.name, "x");
        assert_eq!(ws.rgb, [1, 2, 3]);
    }

    const GOLDEN_SETTINGS: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../oracle-tests/golden/settings"
    );

    /// A C++-generated golden (T0, `gen_settings_golden.sh`).
    fn golden(name: &str) -> String {
        std::fs::read_to_string(format!("{GOLDEN_SETTINGS}/{name}"))
            .unwrap_or_else(|e| panic!("read golden {name}: {e}"))
    }

    #[test]
    fn the_default_profile_is_the_cpp_worm_settings_to_toml() {
        // Sorted keys, inner-spaced inline arrays, literal '' strings, rgbDepth = 8 on save,
        // exactly one trailing newline (the archive's "\n").
        let want = "color = 0\ncontroller = 0\ncontrols = [ 0, 0, 0, 0, 0, 0, 0 ]\n\
                    controlsEx = [ 0, 0, 0, 0, 0, 0, 0, 0 ]\n\
                    gamepadControls = [ 11, 12, 13, 14, 110, 10, 0, 9 ]\n\
                    gamepadName = ''\ngamepadSerial = ''\nhealth = 100\ninputDevice = 0\n\
                    name = ''\nrandomName = true\nrgb = [ 104, 104, 248 ]\nrgbDepth = 8\n\
                    weapons = [ 1, 1, 1, 1, 1 ]\n";
        assert_eq!(worm_settings_to_toml(&WormSettings::default()), want);
    }

    #[test]
    fn the_writer_reproduces_the_cpp_defaults_goldens() {
        assert_eq!(
            settings_to_toml(&Settings::default()),
            golden("defaults.cfg")
        );
        assert_eq!(
            gameplay_toml(&Settings::default()),
            golden("defaults.gameplay.toml")
        );
        assert_eq!(
            worm_settings_to_toml(&WormSettings::default()),
            golden("default_profile.toml")
        );
    }

    #[test]
    fn the_settings_table_has_37_keys_and_the_gameplay_subset_28() {
        let full = settings_to_toml(&Settings::default());
        assert!(
            full.starts_with("[network_player]\n"),
            "tables are byte-sorted"
        );
        assert!(full.ends_with("zoneTimeout = 30\n") && !full.ends_with("\n\n"));
        let settings_table = full
            .split("[settings]\n")
            .nth(1)
            .expect("a [settings] table");
        assert_eq!(
            settings_table.lines().filter(|l| l.contains(" = ")).count(),
            37
        );
        let gameplay = gameplay_toml(&Settings::default());
        assert_eq!(gameplay.lines().filter(|l| l.contains(" = ")).count(), 28);
        for absent in [
            "version",
            "modernColors",
            "fullscreen",
            "singleScreenReplay",
            "spectatorWindow",
            "bloodParticleMax",
            "randomMapWidth",
            "randomMapHeight",
            "maxSpectatorRenderHeight",
            "rgbDepth",
        ] {
            assert!(
                !gameplay.contains(&format!("{absent} =")),
                "{absent} is not in SerializeGameplay"
            );
        }
        assert!(
            !gameplay.starts_with('['),
            "the gameplay subset sits at the root"
        );
        assert!(gameplay.starts_with("aiFrames = 140\n"));
    }

    #[test]
    fn writing_then_reading_is_the_identity() {
        let mut s = Settings::default();
        s.tc = "it's\ta \"tc\"".to_string();
        s.level_file = "two\nlines".to_string();
        s.lives = -4;
        s.game_mode = u32::MAX;
        s.weap_table[7] = 2;
        s.map = false;
        s.worm_settings[0].name = "Zo\u{e9}\u{2028}".to_string();
        s.worm_settings[1].rgb = [0, 255, 7];
        s.worm_settings[2].controls_ex = [u32::MAX; 8];
        assert_eq!(settings_from_toml(&settings_to_toml(&s)).unwrap(), s);
        let ws = s.worm_settings[0].clone();
        let mut back = WormSettings::default();
        back.color = ws.color; // LoadProfile restores the pre-load colour
        load_profile(&worm_settings_to_toml(&ws), &mut back).unwrap();
        assert_eq!(back, ws);
    }
}
