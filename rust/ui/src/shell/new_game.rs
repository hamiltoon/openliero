//! Step 4½c (John's T9 ruling: the live game "works like the original openliero") — the C++
//! NEW GAME start, Bevy-free (moved to `ui::shell` in Step 4½d).
//!
//! C++ NEW GAME (`gfx.cpp:1507-1523`) makes a fresh `LocalController` over `settings`
//! (`localController.cpp:30-54` → [`scenario::build::new_match`]) and gives it a level:
//! `Level::GenerateFromSettings` (`level.cpp:397-429`) the first time, and afterwards the
//! PREVIOUS controller's level — as that match left it — unless `regenerate_level` is set or the
//! level settings changed (`random_level`, `level_file`, the map size). The controller then
//! runs weapon selection (`game::selection`) and `ChangeState(kStateGame)`
//! ([`scenario::build::enter_game`]).
//!
//! Seeds. C++ single-player seeds the sim RNG from the wall clock (`Game::Game`,
//! `game.cpp:42`) and generates from the wall-clock `gfx.rand` (`gameEntry.cpp:23`). Rust takes
//! the overview's LD 6 / 4½b design §2 shape: one match seed per NEW GAME (fresh from the clock
//! in `main.rs`, or fixed by `?seed=`); `SimState::new` seeds the sim RNG with it and a
//! dedicated `Rand` seeded with it generates the level. The seed is only the initial value:
//! within a match everything stays deterministic.
//!
//! 4½d replaces the fixed `Settings` with the loaded setup and the menu, and with them the full
//! reuse test (the `old_*` provenance): here the settings never change, so only
//! `regenerate_level` decides.

use std::path::Path;

use assets::level::LevelData;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use scenario::Loaded;
use scenario::assets::read_asset;
use scenario::build::{BuildError, build_match, new_match};
use scenario::settings::{MatchConfig, Settings};
use sim::levelgen::{LevelGenAssets, LevelGenParams, generate_from_settings, level_file_name};
use sim::state::{LevelSim, SimState};
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
/// `random_map_width x random_map_height`, or the `level_file` when `random_level` is off (a
/// file that does not parse falls back to a random level, as C++ does), then `MakeShadow` when
/// `shadow` is on. A missing file panics in `read_asset`: the preview only names embedded levels.
pub fn generate_level(tc_root: &Path, settings: &Settings, seed: u32) -> LevelData {
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
    let file = if settings.random_level {
        None
    } else {
        let bytes = read_asset(tc_root, &level_file_name(&settings.level_file));
        assets::level::load(&bytes).ok()
    };
    let mut rand = Rand::new();
    rand.seed(seed);
    generate_from_settings(&assets, &params, file, &mut rand)
}

/// The NEW GAME loop's state: the settings, this match's seed, the seed policy, and the level
/// the next match starts on (C++: the current controller's level).
pub struct NewGame {
    cfg: MatchConfig,
    fixed_seed: Option<u32>,
    level: LevelData,
}

impl NewGame {
    /// The first NEW GAME: the seed is `fixed_seed` (`?seed=`) or `fresh`, and the level is
    /// generated from it (`gfx.cpp:1519-1522`).
    pub fn new(tc_root: &Path, settings: Settings, fixed_seed: Option<u32>, fresh: u32) -> Self {
        let seed = fixed_seed.unwrap_or(fresh);
        let level = generate_level(tc_root, &settings, seed);
        NewGame {
            cfg: MatchConfig { settings, seed },
            fixed_seed,
            level,
        }
    }

    /// The settings and this match's seed.
    pub fn config(&self) -> &MatchConfig {
        &self.cfg
    }

    pub fn seed(&self) -> u32 {
        self.cfg.seed
    }

    /// The level the next start uses.
    pub fn level(&self) -> &LevelData {
        &self.level
    }

    /// The fresh `LocalController` (`localController.cpp:30-54`) on the level, before weapon
    /// selection: invisible worms, lives 0, empty weapon slots.
    pub fn start(&self, tc_root: &Path) -> Result<Loaded, BuildError> {
        new_match(tc_root, &self.cfg, &self.level)
    }

    /// The start with weapon selection skipped (`?weapons=`; the C++ skip path,
    /// `rollbackController.cpp:384-395`): `InitWeapons` from the saved picks, no draws, then
    /// `enter_game`.
    pub fn start_without_selection(&self, tc_root: &Path) -> Result<Loaded, BuildError> {
        build_match(tc_root, &self.cfg, &self.level)
    }

    /// `ChangeState(kStateGame)` after selection: lives + the blood pool (`enter_game`).
    pub fn enter_game(&self, state: &mut SimState) {
        scenario::build::enter_game(state, &self.cfg);
    }

    /// The next NEW GAME (F5, or the post-match restart): a new seed (`fixed_seed` or `fresh`;
    /// C++ seeds every new `Game` from the clock) and the level rule of `gfx.cpp:1507-1523` —
    /// `regenerate_level` generates a new level from the new seed; otherwise (the C++ default,
    /// `settings.hpp:77`) the next match plays on `played`, the level the last match left
    /// (`SwapLevel(*old_level)`: craters and all).
    pub fn next(&mut self, tc_root: &Path, played: &LevelSim, fresh: u32) {
        self.cfg.seed = self.fixed_seed.unwrap_or(fresh);
        if self.cfg.settings.regenerate_level {
            self.level = generate_level(tc_root, &self.cfg.settings, self.cfg.seed);
        } else {
            debug_assert_eq!(
                (played.width, played.height),
                (self.level.width, self.level.height)
            );
            self.level.material_id.clone_from(&played.material_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use scenario::paths::TC_ROOT;
    use sim::state::ControlState;

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

    #[test]
    fn the_default_level_is_generated_from_a_rand_seeded_with_the_match_seed() {
        let ng = NewGame::new(tc(), Settings::default(), None, 7);
        assert_eq!(ng.seed(), 7);
        let lv = ng.level();
        assert_eq!((lv.width, lv.height), (504, 350), "Settings(): 504x350");
        assert!(lv.palette.is_none(), "a generated level has the TC palette");
        // Same seed, same level; another seed, another level (LD 6: level seed = match seed).
        assert_eq!(generate_level(tc(), &Settings::default(), 7), *lv);
        assert_ne!(
            generate_level(tc(), &Settings::default(), 8).material_id,
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
        assert_eq!(direct, *lv);
    }

    #[test]
    fn a_fixed_seed_wins_over_the_fresh_one() {
        let a = NewGame::new(tc(), Settings::default(), Some(3), 100);
        let b = NewGame::new(tc(), Settings::default(), Some(3), 200);
        assert_eq!((a.seed(), b.seed()), (3, 3));
        assert_eq!(a.level(), b.level());
    }

    #[test]
    fn a_stock_level_file_is_loaded_not_generated() {
        let s = start_settings(Some("Levels/water_stage.lev".into()));
        assert!(!s.random_level);
        let ng = NewGame::new(tc(), s, None, 5);
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
        assert_eq!(*ng.level(), want);
    }

    #[test]
    fn the_start_is_the_cpp_local_controller() {
        let ng = NewGame::new(tc(), Settings::default(), None, 11);
        let st = ng.start(tc()).unwrap().state;
        for w in &st.worms {
            assert!(!w.visible, "worm.hpp: not visible until the first spawn");
            assert_eq!(w.lives, 0, "lives are set at kStateGame");
            assert!(
                w.weapons.iter().all(|s| s.ty.is_none()),
                "no InitWeapons yet"
            );
        }
        assert_eq!(st.rand.draws(), 0);
        assert_eq!(st.level.material_id, ng.level().material_id);
    }

    #[test]
    fn new_match_then_selection_then_enter_game() {
        let ng = NewGame::new(tc(), Settings::default(), None, 11);
        let mut st = ng.start(tc()).unwrap().state;
        let mut sel = Selection::new(new_game_config(&ng.config().settings, false));
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
        ng.enter_game(&mut st);
        let lives = ng.config().settings.lives;
        assert!(st.worms.iter().all(|w| w.lives == lives));
        assert!(st.worms.iter().all(|w| w.weapons[0].ty.is_some()));
    }

    #[test]
    fn skipping_selection_loads_the_saved_picks_and_enters_the_game() {
        let ng = NewGame::new(tc(), Settings::default(), None, 11);
        let st = ng.start_without_selection(tc()).unwrap().state;
        let lives = ng.config().settings.lives;
        assert!(st.worms.iter().all(|w| w.lives == lives));
        assert!(
            st.worms
                .iter()
                .all(|w| w.weapons.iter().all(|s| s.ty.is_some()))
        );
        assert_eq!(st.rand.draws(), 0, "the skip path draws nothing");
    }

    #[test]
    fn the_next_new_game_reuses_the_played_level_with_a_new_seed() {
        let mut ng = NewGame::new(tc(), Settings::default(), None, 11);
        let mut played = ng.start(tc()).unwrap().state.level;
        played.material_id[1234] ^= 0xff; // a crater
        ng.next(tc(), &played, 99);
        assert_eq!(ng.seed(), 99, "a fresh seed per NEW GAME");
        assert_eq!(
            ng.level().material_id,
            played.material_id,
            "gfx.cpp:1512-1518: SwapLevel(*old_level)"
        );
        assert_eq!(
            ng.start(tc()).unwrap().state.level.material_id,
            played.material_id
        );
    }

    #[test]
    fn regenerate_level_makes_a_new_level_from_the_new_seed() {
        let s = Settings {
            regenerate_level: true,
            ..Settings::default()
        };
        let mut ng = NewGame::new(tc(), s.clone(), None, 11);
        let played = ng.start(tc()).unwrap().state.level;
        ng.next(tc(), &played, 12);
        assert_eq!(*ng.level(), generate_level(tc(), &s, 12));
    }

    #[test]
    fn a_fixed_seed_is_kept_across_new_games() {
        let mut ng = NewGame::new(tc(), Settings::default(), Some(4), 11);
        let played = ng.start(tc()).unwrap().state.level;
        ng.next(tc(), &played, 12);
        assert_eq!(ng.seed(), 4);
    }
}

#[cfg(test)]
mod play_tests {
    use render::bitmap::Bitmap;
    use scenario::paths::TC_ROOT;
    use sim::state::ControlState;

    use super::*;
    use crate::shell::selection::{Selection, new_game_config};

    /// The browser check found it: a NEW GAME has `max_bonuses = 4`, so bonuses spawn, and the
    /// draw indexes `Common::bonus_frames` (`SceneData::bonus_frames`, empty before 4½c).
    #[test]
    fn a_new_game_plays_and_draws_its_bonuses() {
        let tc = Path::new(TC_ROOT);
        let ng = NewGame::new(tc, Settings::default(), Some(3488140121), 0);
        let loaded = ng.start(tc).unwrap();
        let (mut st, mut vps, scene) = (loaded.state, loaded.viewports, loaded.scene);
        let mut sel = Selection::new(new_game_config(&ng.config().settings, false));
        sel.begin(&mut st).unwrap();
        let cs = ControlState::unpack;
        let mut sounds = Vec::new();
        for w in [1, 0] {
            sel.step(&mut st, &[cs(w), cs(w)], &mut sounds);
        }
        assert!(sel.step(&mut st, &[cs(16), cs(16)], &mut sounds));
        ng.enter_game(&mut st);
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
