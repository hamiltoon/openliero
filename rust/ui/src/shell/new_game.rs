//! The C++ NEW GAME start's level generation (Step 4½c; moved to `ui::shell` in Step 4½d): the
//! settings a live match starts from and `Level::GenerateFromSettings` over the match seed. The
//! NEW GAME loop — the seed policy, level reuse, the controller — is `ui::shell` (`LevelSlot`,
//! `SeedSource`, `Match`) since 4½d.
//!
//! Seeds. C++ single-player seeds the sim RNG from the wall clock (`Game::Game`,
//! `game.cpp:42`) and generates from the wall-clock `gfx.rand` (`gameEntry.cpp:23`). Rust takes
//! the overview's LD 6 / 4½b design §2 shape: one match seed per NEW GAME (fresh from the clock
//! in `game`'s `main.rs`, or fixed by `?seed=`); `SimState::new` seeds the sim RNG with it and a
//! dedicated `Rand` seeded with it generates the level. The seed is only the initial value:
//! within a match everything stays deterministic.

use std::path::Path;

use assets::level::LevelData;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use scenario::assets::read_asset;
use scenario::settings::Settings;
use sim::levelgen::{LevelGenAssets, LevelGenParams, generate_from_settings};
use sim_core::rng::Rand;

/// The settings a live match starts from until 4½d loads a setup: `Settings::default()`, with a
/// stock level file (`?level=`, TC-relative) replacing the generated level when given.
pub fn start_settings(level_file: Option<String>) -> Settings {
    let mut s = Settings::default();
    if let Some(file) = level_file {
        s.random_level = false;
        s.level_file = file;
    }
    s
}

/// `Level::GenerateFromSettings(common, settings, rand)` (`level.cpp:397-429`) with the level
/// `Rand` seeded from the match seed (LD 6; 4½b design §2): a random level of
/// `random_map_width x random_map_height`, or `file` when `random_level` is off (`None` — the
/// file was missing or did not parse — falls back to a random level, as C++ does), then
/// `MakeShadow` when `shadow` is on. Step 4½e-1: the caller resolves the file
/// (`level_path::read_level`); this never reads one.
pub fn generate_level(
    tc_root: &Path,
    settings: &Settings,
    file: Option<LevelData>,
    seed: u32,
) -> LevelData {
    let tc = TcConfig::load(&read_asset(tc_root, "tc.cfg")).expect("tc.cfg parses");
    let tga = Tga::load(&read_asset(tc_root, "sprites/large.tga")).expect("large.tga parses");
    let large = SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank");
    let assets = LevelGenAssets {
        large_sprites: &large,
        textures: &tc.textures,
        material_flags: &tc.materials,
    };
    let params = LevelGenParams {
        random_level: settings.random_level,
        random_map_width: settings.random_map_width,
        random_map_height: settings.random_map_height,
        shadow: settings.shadow,
    };
    let mut rand = Rand::new();
    rand.seed(seed);
    generate_from_settings(&assets, &params, file, &mut rand)
}

#[cfg(test)]
mod tests {
    use scenario::build::{build_match, enter_game, new_match};
    use scenario::paths::TC_ROOT;
    use scenario::settings::MatchConfig;
    use sim::state::{ControlState, LevelSim};

    use super::*;
    use crate::shell::selection::{Selection, new_game_config};

    fn tc() -> &'static Path {
        Path::new(TC_ROOT)
    }

    fn seeded(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    /// The default settings with `seed`, and the level generated from it.
    fn start(seed: u32) -> (MatchConfig, LevelData) {
        let settings = Settings::default();
        let level = generate_level(tc(), &settings, None, seed);
        (MatchConfig { settings, seed }, level)
    }

    #[test]
    fn the_default_level_is_generated_from_a_rand_seeded_with_the_match_seed() {
        let lv = generate_level(tc(), &Settings::default(), None, 7);
        assert_eq!((lv.width, lv.height), (504, 350), "Settings(): 504x350");
        assert!(lv.palette.is_none(), "a generated level has the TC palette");
        // Same seed, same level; another seed, another level (LD 6: level seed = match seed).
        assert_eq!(generate_level(tc(), &Settings::default(), None, 7), lv);
        assert_ne!(
            generate_level(tc(), &Settings::default(), None, 8).material_id,
            lv.material_id
        );
        // generate_level is GenerateFromSettings over a Rand seeded with the seed (4½b §2).
        let tc_cfg = TcConfig::load(&read_asset(tc(), "tc.cfg")).unwrap();
        let tga = Tga::load(&read_asset(tc(), "sprites/large.tga")).unwrap();
        let large = SpriteSet::from_tga(&tga, 16, 16, 110).unwrap();
        let assets = LevelGenAssets {
            large_sprites: &large,
            textures: &tc_cfg.textures,
            material_flags: &tc_cfg.materials,
        };
        let direct =
            generate_from_settings(&assets, &LevelGenParams::default(), None, &mut seeded(7));
        assert_eq!(direct, lv);
    }

    #[test]
    fn a_stock_level_file_is_loaded_not_generated() {
        let s = start_settings(Some("Levels/water_stage.lev".into()));
        assert!(!s.random_level);
        let store = scenario::storage::MemoryStore::new();
        let file = crate::shell::level_path::read_level(&store, tc(), &s.level_file);
        let level = generate_level(tc(), &s, file, 5);
        let mut want = assets::level::load(&read_asset(tc(), "Levels/water_stage.lev")).unwrap();
        let mut lv = LevelSim {
            width: want.width,
            height: want.height,
            material_id: want.material_id.clone(),
            material_flags: TcConfig::load(&read_asset(tc(), "tc.cfg"))
                .unwrap()
                .materials,
        };
        sim::levelgen::make_shadow(&mut lv); // settings.shadow (level.cpp:426-428)
        want.material_id = lv.material_id;
        assert_eq!(level, want);
    }

    #[test]
    fn the_start_is_the_cpp_local_controller() {
        let (cfg, level) = start(11);
        let st = new_match(tc(), &cfg, &level).unwrap().state;
        for w in &st.worms {
            assert!(!w.visible, "worm.hpp: not visible until the first spawn");
            assert_eq!(w.lives, 0, "lives are set at kStateGame");
            assert!(
                w.weapons.iter().all(|s| s.ty.is_none()),
                "no InitWeapons yet"
            );
        }
        assert_eq!(st.rand.draws(), 0);
        assert_eq!(st.level.material_id, level.material_id);
    }

    #[test]
    fn new_match_then_selection_then_enter_game() {
        let (cfg, level) = start(11);
        let mut st = new_match(tc(), &cfg, &level).unwrap().state;
        let mut sel = Selection::new(new_game_config(&cfg.settings, false));
        sel.begin(&mut st).unwrap();
        assert_eq!(
            sel.active().unwrap().player(0).picks,
            [1; 5],
            "Settings() picks"
        );
        let cs = ControlState::unpack;
        let mut sounds = Vec::new();
        // Both: Up (RANDOMIZE -> DONE!), release, Fire.
        for w in [1, 0] {
            assert!(!sel.step(&mut st, &[cs(w), cs(w)], &mut sounds));
        }
        assert!(sel.step(&mut st, &[cs(16), cs(16)], &mut sounds));
        enter_game(&mut st, &cfg);
        let lives = cfg.settings.lives;
        assert!(st.worms.iter().all(|w| w.lives == lives));
        assert!(st.worms.iter().all(|w| w.weapons[0].ty.is_some()));
    }

    #[test]
    fn skipping_selection_loads_the_saved_picks_and_enters_the_game() {
        let (cfg, level) = start(11);
        let st = build_match(tc(), &cfg, &level).unwrap().state;
        let lives = cfg.settings.lives;
        assert!(st.worms.iter().all(|w| w.lives == lives));
        assert!(
            st.worms
                .iter()
                .all(|w| w.weapons.iter().all(|s| s.ty.is_some()))
        );
        assert_eq!(st.rand.draws(), 0, "the skip path draws nothing");
    }
}

#[cfg(test)]
mod play_tests {
    use render::bitmap::Bitmap;
    use scenario::build::{enter_game, new_match};
    use scenario::paths::TC_ROOT;
    use scenario::settings::MatchConfig;
    use sim::state::ControlState;

    use super::*;
    use crate::shell::selection::{Selection, new_game_config};

    /// The browser check found it: a NEW GAME has `max_bonuses = 4`, so bonuses spawn, and the
    /// draw indexes `Common::bonus_frames` (`SceneData::bonus_frames`, empty before 4½c).
    #[test]
    fn a_new_game_plays_and_draws_its_bonuses() {
        let tc = Path::new(TC_ROOT);
        let settings = Settings::default();
        let seed = 3488140121;
        let level = generate_level(tc, &settings, None, seed);
        let cfg = MatchConfig { settings, seed };
        let loaded = new_match(tc, &cfg, &level).unwrap();
        let (mut st, mut vps, scene) = (loaded.state, loaded.viewports, loaded.scene);
        let mut sel = Selection::new(new_game_config(&cfg.settings, false));
        sel.begin(&mut st).unwrap();
        let cs = ControlState::unpack;
        let mut sounds = Vec::new();
        for w in [1, 0] {
            sel.step(&mut st, &[cs(w), cs(w)], &mut sounds);
        }
        assert!(sel.step(&mut st, &[cs(16), cs(16)], &mut sounds));
        enter_game(&mut st, &cfg);
        let mut bmp = Bitmap::new(320, 200);
        let mut saw_bonus = false;
        for t in 0..1500u32 {
            let i = if t > 300 {
                [cs(8), cs(4)]
            } else {
                [cs(0), cs(0)]
            };
            crate::shell::viewport_step::tick_viewports(&mut vps, &mut st, &i);
            let mut s = scene.as_scene(st.screen_flash, true);
            s.draw_hud = true;
            s.map = true;
            render::frame::draw(&mut bmp, &st, &mut vps, &s);
            saw_bonus |= !st.bonuses.is_empty();
        }
        assert!(saw_bonus, "the run must draw a bonus to pin the fix");
    }
}
