//! Step 4½a-1 — the `MatchConfig -> SimState` builder (design §4).
//!
//! Produces the C++ `LocalController` start state: fresh worms (`visible = false`,
//! `killed_timer = 150`, `pos = (0,0)`, `worm.hpp:178-183`), `health = ws.health`
//! (`localController.cpp:35,42`), `stats_x` 0/218, `InitWeapons` from each worm's picks
//! (`worm.cpp:698-709`), `lives = settings->lives` (`localController.cpp:232-235`) and
//! the `StartGame` blood-pool size (`game.cpp:513`) — identical to `ResetWorms`
//! (`game.cpp:155-166`), which the oracle dumper's `settings` path runs.
//! `SimState::new`'s signature is unchanged; every other setting is a post-`new`
//! assignment (LD 5). The level arrives READY: level preparation (random vs file +
//! `MakeShadow`) is 4½b's `sim::levelgen::generate_from_settings`, composed in 4½d.
//!
//! Step 4½c (design §4.7) cuts the seam in three: [`new_match`] is the `LocalController`
//! constructor (`localController.cpp:30-54`: `health`, `stats_x`, invisible, `lives = 0`
//! (`worm.hpp:238`), no weapons); `sim::weapsel` runs the selection phase on it; [`enter_game`]
//! is `ChangeState(kStateGame)`'s lives (`:232-235`) + `StartGame`'s pool (`game.cpp:513`).
//! [`build_match`] = `new_match` → `sim::weapsel::init_weapons` (the saved picks) → `enter_game`,
//! with output identical to before.

use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use render::viewport::Viewport;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::pool::BloodPool;
use sim::state::{SimState, WeaponInit, WormInit, NUM_WEAPONS};
use sim::weapsel::{WeapselConfig, WeapselPlayer};
use sim_core::vec::Vec2;

use crate::loader::{load_sprites, scene_data, Loaded};
use crate::settings::{MatchConfig, Settings, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE, WEAP_TABLE_LEN};

/// Why a `MatchConfig` cannot become a match. Every variant is a configuration a menu
/// or a file can produce, so it is an error, never a panic (design §4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// `game_mode == 2`: the sim's Holdazone arm is unported (`state.rs:2250-2252`).
    HoldazoneUnsupported,
    /// `game_mode > 3`.
    InvalidGameMode(u32),
    /// A playing worm's `health < 1` (C++ divides by it and loops on it in Scales).
    InvalidHealth(i32),
    /// A playing worm's `controller == 2`, the unported "AI" (`FollowAI`, `localController.cpp:
    /// 23-25`; 4½f Q2). Only [`refuse_follow_ai`] returns it — the menu's NEW GAME gate — never
    /// [`validate`] (4½f-1 D1: committed sim setups play a controller-2 player as a human).
    FollowAiUnsupported { worm: usize },
    /// A weapon pick outside `1..=weap_order.len()` (`worm.cpp:704` indexes unchecked) — or, in
    /// front of a selection ([`new_match`]), outside `0..=weap_order.len()`.
    InvalidWeapon {
        worm: usize,
        slot: usize,
        value: u32,
    },
    /// `blood_particle_max < 1` (a zero-cap blood pool).
    InvalidBloodParticleMax(i32),
    /// The TC has more weapons than `weap_table[40]` can index.
    TooManyWeapons(usize),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::HoldazoneUnsupported => write!(f, "Holdazone is not supported yet"),
            BuildError::InvalidGameMode(m) => write!(f, "unknown game mode {m}"),
            BuildError::InvalidHealth(h) => write!(f, "worm health {h} must be at least 1"),
            BuildError::FollowAiUnsupported { worm } => {
                write!(
                    f,
                    "player {worm} is an AI (FollowAI) player; not supported yet"
                )
            }
            BuildError::InvalidWeapon { worm, slot, value } => {
                write!(
                    f,
                    "player {worm} weapon slot {slot}: {value} is not a weapon"
                )
            }
            BuildError::InvalidBloodParticleMax(n) => {
                write!(f, "blood particle max {n} must be at least 1")
            }
            BuildError::TooManyWeapons(n) => write!(f, "the TC has {n} weapons; at most 40"),
        }
    }
}

impl std::error::Error for BuildError {}

/// How [`validate`] treats a pick of `0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picks {
    /// `build_match`: every pick names a weapon (`InitWeapons` runs at once).
    Strict,
    /// `new_match`: `0` is unset and the selection's constructor rolls it (`weapsel.cpp:60`).
    AllowUnset,
}

/// Check `cfg` against a TC with `n_weapons` weapons. Only the two playing worms
/// (indices 0 and 1) are checked; the network player never plays a local match.
pub fn validate(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError> {
    validate_with(cfg, n_weapons, Picks::Strict)
}

/// [`validate`] for a match that runs weapon selection first: a `0` pick is legal (design §4.7).
pub fn validate_for_selection(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError> {
    validate_with(cfg, n_weapons, Picks::AllowUnset)
}

/// The menu's NEW GAME refusal of an "AI" (FollowAI, `controller == 2`) player (4½f Q2; 4½f-1
/// D1): the first of players 0 and 1 that is one. The network player never plays a local match.
/// Not part of [`validate`]: `build_match` and every sim harness take a controller-2 player as a
/// human (the C++ dumper never makes an AI without `ai`), and RESUME never refuses it (CONTROLLER
/// does not reach a running match, design finding 7).
pub fn refuse_follow_ai(s: &Settings) -> Result<(), BuildError> {
    match s.worm_settings[..2]
        .iter()
        .position(|ws| ws.controller == 2)
    {
        Some(worm) => Err(BuildError::FollowAiUnsupported { worm }),
        None => Ok(()),
    }
}

fn validate_with(cfg: &MatchConfig, n_weapons: usize, picks: Picks) -> Result<(), BuildError> {
    let s = &cfg.settings;
    if s.game_mode == GM_HOLDAZONE {
        return Err(BuildError::HoldazoneUnsupported);
    }
    if s.game_mode > GM_SCALES_OF_JUSTICE {
        return Err(BuildError::InvalidGameMode(s.game_mode));
    }
    // Each worm has its own max health (4½f-1): the two may differ.
    for ws in &s.worm_settings[..2] {
        if ws.health < 1 {
            return Err(BuildError::InvalidHealth(ws.health));
        }
    }
    if n_weapons > WEAP_TABLE_LEN {
        return Err(BuildError::TooManyWeapons(n_weapons));
    }
    for worm in 0..2 {
        for (slot, &value) in s.worm_settings[worm].weapons.iter().enumerate() {
            if (value == 0 && picks == Picks::Strict) || value as usize > n_weapons {
                return Err(BuildError::InvalidWeapon { worm, slot, value });
            }
        }
    }
    if s.blood_particle_max < 1 {
        return Err(BuildError::InvalidBloodParticleMax(s.blood_particle_max));
    }
    Ok(())
}

/// The `LocalController` constructor (`localController.cpp:30-54`): everything
/// [`build_match`] does EXCEPT the weapons, the lives and the blood pool, which weapon
/// selection and [`enter_game`] supply (design §4.7). A `0` pick is legal (the selection rolls
/// it). Worms: `health = ws.health`, `stats_x` 0/218, invisible, `lives = 0` (`worm.hpp:238`),
/// empty weapon slots; the blood pool keeps `SimState::new`'s default until `enter_game`.
pub fn new_match(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
) -> Result<Loaded, BuildError> {
    new_match_with(tc_root, cfg, level, Picks::AllowUnset)
}

fn new_match_with(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
    picks: Picks,
) -> Result<Loaded, BuildError> {
    let tc = TcConfig::load(&crate::assets::read_asset(tc_root, "tc.cfg")).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        Ok(crate::assets::read_asset(
            tc_root,
            &format!("{sub}/{id}.cfg"),
        ))
    })
    .expect("object configs load");
    validate_with(cfg, objects.weapons.len(), picks)?;
    let s = &cfg.settings;

    let worms_init: Vec<WormInit> = (0..2)
        .map(|i| WormInit {
            index: i as i32,
            health: s.worm_settings[i].health,
            lives: 0, // Worm::lives{0} (worm.hpp:238): set from settings only at kStateGame
            stats_x: if i == 0 { 0 } else { 218 },
            weapons: [WeaponInit::default(); NUM_WEAPONS], // no InitWeapons before selection
            start_pos: Vec2::zero(),
            visible: false,
        })
        .collect();

    let mut state = SimState::new(
        level,
        &worms_init,
        cfg.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_sprites(tc_root, "large.tga", 16, 16, 110),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        s.loading_time,
        s.load_change,
        s.blood,
    );

    // TC consts — the full post-`new` set (sim_slice6_fuzz.rs:199-235) + laser_weapon.
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_sprites(tc_root, "small.tga", 7, 7, 130);
    state.laser_weapon = tc.constants.LaserWeapon;
    state.wobject_consts = sim::weapon::WObjectConsts::from_tc(&tc); // 4½c-0 (design §4.9)
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state.bonus_drop_chance = tc.constants.BonusDropChance;
    state.bonus_spawn_rect_w = tc.constants.BonusSpawnRectW;
    state.bonus_spawn_rect_h = tc.constants.BonusSpawnRectH;
    state.bonus_spawn_rect_x = tc.constants.BonusSpawnRectX;
    state.bonus_spawn_rect_y = tc.constants.BonusSpawnRectY;
    state.h_bonus_spawn_rect = tc.hacks.BonusSpawnRect;
    state.h_bonus_only_health = tc.hacks.BonusOnlyHealth;
    state.h_bonus_only_weapon = tc.hacks.BonusOnlyWeapon;
    state.h_bonus_disable = tc.hacks.BonusDisable;
    state.bonus_rand_timer = [
        [tc.bonuses[0].timer, tc.bonuses[0].timer_v],
        [tc.bonuses[1].timer, tc.bonuses[1].timer_v],
    ];
    state.bonus_s_objects = [tc.bonuses[0].sobj, tc.bonuses[1].sobj];
    state.bonus_gravity = tc.constants.BonusGravity;
    state.bonus_bounce_mul = tc.constants.BonusBounceMul;
    state.bonus_bounce_div = tc.constants.BonusBounceDiv;
    state.bonus_health_var = tc.constants.BonusHealthVar;
    state.bonus_min_health = tc.constants.BonusMinHealth;
    state.bonus_explode_risk = tc.constants.BonusExplodeRisk;
    state.h_bonus_reload_only = tc.hacks.BonusReloadOnly;
    state.sound_hooks = tc.sound_hooks.clone();
    // Step 4½f-1 (D11): the CPU player's toggle odds. Unhashed, read only by `sim::ai`.
    state.ai_params = crate::loader::ai_params(&tc);

    // Settings (design §4.2): the live-read set through the one choke point RESUME shares
    // (Step 4½e-1), each worm's own max health included (4½f-1). The blood pool is
    // `enter_game`'s (StartGame, game.cpp:513).
    apply_live_settings(&mut state, s);

    // Palette (design §4.1): the level's POWERLEVEL palette only when
    // load_powerlevel_palette (level.cpp:281-294), else exepal == small.tga's palette
    // (level.cpp:385-392); Game::UpdateSettings makes it the renderer's (game.cpp:476).
    let small_tga =
        assets::sprite::Tga::load(&crate::assets::read_asset(tc_root, "sprites/small.tga"))
            .expect("small.tga parses");
    let origpal = match (&level.palette, s.load_powerlevel_palette) {
        (Some(pal), true) => pal.clone(),
        _ => small_tga.palette.clone(),
    };
    let scene = scene_data(tc_root, &tc, origpal, &state.large_sprites);
    Ok(Loaded {
        state,
        viewports: Viewport::player_layout(),
        scene,
    })
}

/// Write the settings a running C++ `Game` reads **live** from `gfx.settings` (design finding 1,
/// confirmed by the T0 probe) onto `state`: exactly the eight per-tick sim fields and each worm's
/// `max_health` (4½f-1 T3; its T0 P5), nothing else.
/// [`new_match`]'s one settings choke point (Step 4½e-1), and what the shell's RESUME runs so a
/// paused match sees the menu's edits, as the C++ one does. None of these fields is hashed, so
/// the call is hash-neutral by construction; it draws nothing. A worm's `health` itself is left to
/// the next tick's clamp (`worm.cpp:213`), as in C++.
///
/// Every C++ `settings->` read in the simulation and the viewport (`grep -n 'settings->'
/// game.cpp worm.cpp weapon.cpp nobject.cpp sobject.cpp bonus.cpp viewport.cpp`, re-derived
/// for 4½e-1), plus `Weapon::ComputedLoadingTime`'s `settings.loading_time` (`weapon.cpp:9`,
/// called from `worm.cpp:824`):
///
/// | C++ read site | Setting | Rust |
/// |---|---|---|
/// | `game.cpp:219`, `:359` | `max_bonuses` | `SimState::settings_max_bonuses` (here) |
/// | `game.cpp:258` | `weap_table` | `SimState::weap_table` (here, as `i32`) |
/// | `game.cpp:372`, `:516`, `:522-523`, `:529`, `:535`, `:557`, `:571`, `:594`; `worm.cpp:215-216`, `:384`, `:396`, `:794` | `game_mode` | `SimState::game_mode` (here) |
/// | `game.cpp:385`, `:531`, `:537` | `time_to_lose` | `SimState::time_to_lose` (here) |
/// | `worm.cpp:410`; `weapon.cpp:301`; `nobject.cpp:188`; `sobject.cpp:96` | `blood` | `SimState::blood` (here) |
/// | `weapon.cpp:9` (via `worm.cpp:824`) | `loading_time` | `SimState::settings_loading_time` (here) |
/// | `worm.cpp:1079` | `load_change` | `SimState::load_change` (here) |
/// | `worm.cpp:784`, `:932`, `:942`; `weapon.cpp:121`; `nobject.cpp:123`, `:215`; `sobject.cpp:212` | `shadow` | `SimState::shadow` (here) |
/// | `viewport.cpp:274` | `shadow` | draw (`Match`: the render shadow pass) |
/// | `viewport.cpp:114` (`ComputedLoadingTime`) | `loading_time` | draw (`Match`: the HUD ammo bar) |
/// | `viewport.cpp:148`, `:212`, `:250`, `:615` | `game_mode` | draw (`Match`: the HUD) |
/// | `viewport.cpp:408`, `:466` | `names_on_bonuses` | draw (`Match`: `Scene::small_labels`) |
/// | `viewport.cpp:593` | `map` | draw (`Match`: `Scene::map`) |
/// | `viewport.cpp:239` | `allow_viewing_spawn_point` | draw (hidden menu, 4½g) |
/// | `game.cpp:159` | `lives` | kStateGame only (`ResetWorms` is Rollback-only; `LocalController` reads it once, `localController.cpp:234`: [`enter_game`]) |
/// | `game.cpp:513` | `blood_particle_max` | kStateGame / `StartGame` only ([`enter_game`]) |
/// | `game.cpp:427-432`, `:499` | `zone_timeout` | Holdazone (unported) |
/// | `game.cpp:158`, `:558-563`, `:607`; `worm.cpp:213`, `:292-296`, `:355`, `:386`, `:795`; `viewport.cpp:85` | `worm.settings->health` | `WormState::max_health` of worms 0 and 1 (here; 4½f-1 T3) |
/// | `game.cpp:63-96`; `worm.cpp:704`; `viewport.cpp:135-139`, `:261`, `:265` | `worm.settings->{input_device, controls, weapons, name, color}` | per-worm (input / `InitWeapons` / names; not live sim settings) |
pub fn apply_live_settings(state: &mut SimState, s: &Settings) {
    state.settings_max_bonuses = s.max_bonuses;
    state.weap_table = s.weap_table.iter().map(|&v| v as i32).collect();
    state.game_mode = s.game_mode;
    state.time_to_lose = s.time_to_lose;
    state.blood = s.blood;
    state.settings_loading_time = s.loading_time;
    state.load_change = s.load_change;
    state.shadow = s.shadow;
    for (worm, ws) in state.worms.iter_mut().zip(&s.worm_settings[..2]) {
        worm.max_health = ws.health;
    }
}

/// `ChangeState(kStateGame)` after weapon selection (`localController.cpp:232-235`: `lives =
/// settings.lives`) and `Game::StartGame`'s blood pool (`game.cpp:513`). Draws nothing.
pub fn enter_game(state: &mut SimState, cfg: &MatchConfig) {
    for worm in state.worms.iter_mut() {
        worm.lives = cfg.settings.lives;
    }
    state.bobjects = BloodPool::new(cfg.settings.blood_particle_max as usize);
}

/// Build the tick-0 match for `cfg` on the ready `level` (design §4.2) — the C++ settings path
/// without a selection phase: [`new_match`] (strict picks) → `InitWeapons` from the saved
/// picks → [`enter_game`]. Output identical to the 4½a-1 builder (the unit tests + the eight
/// settings-path goldens).
pub fn build_match(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
) -> Result<Loaded, BuildError> {
    let mut loaded = new_match_with(tc_root, cfg, level, Picks::Strict)?;
    let order = sim::weapsel::weap_order(&loaded.state.weapons);
    let s = &cfg.settings;
    sim::weapsel::init_weapons(
        &mut loaded.state,
        &order,
        &[s.worm_settings[0].weapons, s.worm_settings[1].weapons],
    );
    enter_game(&mut loaded.state, cfg);
    Ok(loaded)
}

/// The `WeapselConfig` a `Settings` gives (design §4.7): `weap_table`, the raw
/// `select_bot_weapons`, and players 0/1's picks + controller (the network player never plays
/// a local match).
pub fn weapsel_config(s: &Settings) -> WeapselConfig {
    WeapselConfig {
        weap_table: s.weap_table,
        select_bot_weapons: s.select_bot_weapons,
        players: [0, 1].map(|i| WeapselPlayer {
            weapons: s.worm_settings[i].weapons,
            controller: s.worm_settings[i].controller,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE};
    use assets::palette::{Color, Palette};
    use sim_core::vec::Vec2;

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

    fn cfg() -> MatchConfig {
        MatchConfig {
            settings: Settings::default(),
            seed: 1234,
        }
    }

    fn level() -> LevelData {
        let bytes =
            std::fs::read(format!("{TC_ROOT}/Levels/render_stage.lev")).expect("read level");
        assets::level::load(&bytes).expect("level loads")
    }

    fn tc_and_objects() -> (TcConfig, Objects) {
        let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
        let objects = Objects::load(&tc.types, |sub, id| {
            std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
        })
        .unwrap();
        (tc, objects)
    }

    #[test]
    fn the_cpp_defaults_validate() {
        assert_eq!(validate(&cfg(), 40), Ok(()));
    }

    #[test]
    fn holdazone_and_unknown_modes_are_refused() {
        let mut c = cfg();
        c.settings.game_mode = GM_HOLDAZONE;
        assert_eq!(validate(&c, 40), Err(BuildError::HoldazoneUnsupported));
        c.settings.game_mode = 4;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidGameMode(4)));
    }

    #[test]
    fn health_must_be_positive_and_may_differ_between_the_players() {
        let mut c = cfg();
        c.settings.worm_settings[0].health = 0;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidHealth(0)));
        let mut c = cfg();
        c.settings.worm_settings[1].health = 150;
        assert_eq!(validate(&c, 40), Ok(()), "4½f-1 T3: unequal healths play");
        assert_eq!(validate_for_selection(&c, 40), Ok(()));
    }

    #[test]
    fn unequal_healths_build_each_worm_at_its_own_max() {
        let mut c = cfg();
        c.settings.worm_settings[0].health = 30;
        c.settings.worm_settings[1].health = 250;
        for st in [
            build_match(Path::new(TC_ROOT), &c, &level()).unwrap().state,
            new_match(Path::new(TC_ROOT), &c, &level()).unwrap().state,
        ] {
            assert_eq!((st.worms[0].health, st.worms[1].health), (30, 250));
            assert_eq!((st.worms[0].max_health, st.worms[1].max_health), (30, 250));
        }
    }

    // ---- 4½f-1 T3 Step 3: the FollowAI refusal (D1; John's Q2) ------------------------

    #[test]
    fn refuse_follow_ai_names_the_first_playing_follow_ai() {
        assert_eq!(refuse_follow_ai(&Settings::default()), Ok(()));
        for worm in 0..2 {
            for controller in [0, 1] {
                let mut s = Settings::default();
                s.worm_settings[worm].controller = controller;
                assert_eq!(refuse_follow_ai(&s), Ok(()), "worm {worm} {controller}");
            }
            let mut s = Settings::default();
            s.worm_settings[worm].controller = 2;
            assert_eq!(
                refuse_follow_ai(&s),
                Err(BuildError::FollowAiUnsupported { worm })
            );
        }
        let mut s = Settings::default();
        s.worm_settings[0].controller = 2;
        s.worm_settings[1].controller = 2;
        assert_eq!(
            refuse_follow_ai(&s),
            Err(BuildError::FollowAiUnsupported { worm: 0 }),
            "the first one"
        );
        let mut s = Settings::default();
        s.worm_settings[2].controller = 2;
        assert_eq!(
            refuse_follow_ai(&s),
            Ok(()),
            "the network player never plays"
        );
        assert_eq!(
            BuildError::FollowAiUnsupported { worm: 1 }.to_string(),
            "player 1 is an AI (FollowAI) player; not supported yet"
        );
    }

    #[test]
    fn the_builders_still_accept_a_follow_ai_setup() {
        // Fact 15: three committed sim setups have [player2] controller = 2 and go through
        // build_match; the refusal is the menu's NEW GAME gate only (D1).
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../oracle-tests/golden/sim_slice4_5a_killemall_setup.cfg"
        ))
        .unwrap();
        let settings = crate::settings_toml::settings_from_toml(&text).unwrap();
        assert_eq!(settings.worm_settings[1].controller, 2, "non-vacuous");
        let c = MatchConfig { settings, seed: 7 };
        assert_eq!(validate(&c, 40), Ok(()));
        assert_eq!(validate_for_selection(&c, 40), Ok(()));
        assert!(build_match(Path::new(TC_ROOT), &c, &level()).is_ok());
        assert!(
            refuse_follow_ai(&c.settings).is_err(),
            "the gate's own check"
        );
    }

    #[test]
    fn weapon_picks_must_index_weap_order() {
        let mut c = cfg();
        c.settings.worm_settings[1].weapons[3] = 41;
        assert_eq!(
            validate(&c, 40),
            Err(BuildError::InvalidWeapon {
                worm: 1,
                slot: 3,
                value: 41
            })
        );
        c.settings.worm_settings[1].weapons[3] = 0;
        assert_eq!(
            validate(&c, 40),
            Err(BuildError::InvalidWeapon {
                worm: 1,
                slot: 3,
                value: 0
            })
        );
    }

    #[test]
    fn pool_and_table_limits_are_refused() {
        let mut c = cfg();
        c.settings.blood_particle_max = 0;
        assert_eq!(
            validate(&c, 40),
            Err(BuildError::InvalidBloodParticleMax(0))
        );
        assert_eq!(validate(&cfg(), 41), Err(BuildError::TooManyWeapons(41)));
    }

    #[test]
    fn the_network_player_is_not_validated() {
        let mut c = cfg();
        c.settings.worm_settings[2].health = 0;
        c.settings.worm_settings[2].weapons = [0; 5];
        assert_eq!(validate(&c, 40), Ok(()));
    }

    #[test]
    fn build_match_maps_every_setting_onto_the_state() {
        let mut c = cfg();
        let s = &mut c.settings;
        s.lives = 7;
        s.loading_time = 37;
        s.blood = 250;
        s.load_change = false;
        s.max_bonuses = 6;
        s.shadow = false;
        s.time_to_lose = 99;
        s.game_mode = GM_SCALES_OF_JUSTICE;
        s.blood_particle_max = 300;
        s.weap_table[5] = 2;
        s.worm_settings[0].health = 150;
        s.worm_settings[1].health = 150;
        s.worm_settings[0].weapons = [2, 3, 4, 5, 6];
        s.worm_settings[1].weapons = [7, 8, 9, 10, 11];
        let loaded = build_match(Path::new(TC_ROOT), &c, &level()).expect("valid config builds");
        let st = &loaded.state;
        let (tc, objects) = tc_and_objects();
        let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
        weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
        for i in 0..2 {
            let w = &st.worms[i];
            assert_eq!(w.index, i as i32);
            assert_eq!((w.lives, w.health, w.killed_timer), (7, 150, 150));
            assert!(
                !w.visible,
                "LocalController start: invisible, respawns in-sim"
            );
            assert_eq!(w.pos, Vec2::zero());
            assert_eq!(w.stats_x, [0, 218][i]);
            let want = WormInit::resolve_weapons(
                &objects,
                &weap_order,
                &c.settings.worm_settings[i].weapons,
            );
            for j in 0..5 {
                assert_eq!(
                    (w.weapons[j].ty, w.weapons[j].ammo),
                    (want[j].ty, want[j].ammo)
                );
            }
        }
        assert_ne!(
            st.worms[0].weapons[0].ty, st.worms[1].weapons[0].ty,
            "per-worm loadouts"
        );
        assert_eq!(
            (st.settings_loading_time, st.blood, st.load_change),
            (37, 250, false)
        );
        assert_eq!(st.settings_max_bonuses, 6);
        assert_eq!(st.weap_table.len(), 40);
        assert_eq!(st.weap_table[5], 2);
        assert_eq!((st.worms[0].max_health, st.worms[1].max_health), (150, 150));
        assert_eq!((st.game_mode, st.time_to_lose), (3, 99));
        assert_eq!(
            st.ai_params,
            [
                [120, 120, 50, 50, 80, 300, 400],
                [20, 20, 20, 20, 80, 60, 1]
            ],
            "the openliero TC's aiparams (D11)"
        );
        assert!(!st.shadow);
        assert_eq!(st.bobjects.capacity(), 300);
        assert_eq!(st.sound_hooks, tc.sound_hooks);
        assert_eq!(st.bonus_drop_chance, tc.constants.BonusDropChance);
        assert_eq!(st.bonus_health_var, tc.constants.BonusHealthVar);
        assert_eq!(st.worm_spawn_rect_w, tc.constants.WormSpawnRectW);
        assert_eq!(st.laser_weapon, tc.constants.LaserWeapon);
        assert_eq!(
            st.wobject_consts,
            sim::weapon::WObjectConsts::from_tc(&tc),
            "4½c-0: the WObject::Process TC consts"
        );
        assert_eq!(st.rand.last(), 0, "building consumes no RNG");

        // Both poles of the shadow flag (T3 review): `SimState.shadow` is the builder's
        // only shadow duty — `correct_shadow_if_enabled` no-ops outside `process_frame`,
        // and the in-tick proof is T8/T9's goldens.
        c.settings.shadow = true;
        let on = build_match(Path::new(TC_ROOT), &c, &level()).expect("valid config builds");
        assert!(
            on.state.shadow,
            "settings.shadow = true reaches SimState.shadow"
        );
    }

    #[test]
    fn build_match_refuses_holdazone_without_panicking() {
        let mut c = cfg();
        c.settings.game_mode = GM_HOLDAZONE;
        assert_eq!(
            build_match(Path::new(TC_ROOT), &c, &level()).err(),
            Some(BuildError::HoldazoneUnsupported)
        );
    }

    #[test]
    fn a_powerlevel_palette_wins_only_when_the_setting_allows_it() {
        let mut custom = Palette {
            entries: [Color::default(); 256],
        };
        custom.entries[1] = Color { r: 1, g: 2, b: 3 };
        let mut lev = level();
        lev.palette = Some(custom.clone());
        let mut c = cfg();
        assert!(c.settings.load_powerlevel_palette, "C++ default: on");
        let on = build_match(Path::new(TC_ROOT), &c, &lev).unwrap();
        assert_eq!(on.scene.origpal, custom, "level.cpp:281-294 + game.cpp:476");
        c.settings.load_powerlevel_palette = false;
        let off = build_match(Path::new(TC_ROOT), &c, &lev).unwrap();
        let small = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).unwrap();
        let exepal = assets::sprite::Tga::load(&small).unwrap().palette;
        assert_eq!(
            off.scene.origpal, exepal,
            "level.cpp:385-392: reset to exepal (small.tga's)"
        );
        assert_ne!(exepal, custom, "non-vacuous: the two palettes differ");
    }

    // ---- 4½c T3: the seam split (design §4.7) ------------------------------------------

    use sim::hash::{hash_components, hash_game_state};

    #[test]
    fn a_zero_pick_is_legal_only_in_front_of_a_selection() {
        let mut c = cfg();
        c.settings.worm_settings[0].weapons = [0, 5, 0, 40, 0];
        assert_eq!(validate_for_selection(&c, 40), Ok(()));
        assert!(validate(&c, 40).is_err(), "build_match stays strict");
        c.settings.worm_settings[1].weapons[2] = 41;
        assert_eq!(
            validate_for_selection(&c, 40),
            Err(BuildError::InvalidWeapon {
                worm: 1,
                slot: 2,
                value: 41
            })
        );
    }

    #[test]
    fn new_match_is_the_localcontroller_start_before_selection() {
        let mut c = cfg();
        c.settings.worm_settings[0].weapons = [0, 5, 0, 40, 0];
        c.settings.lives = 7;
        let st = new_match(Path::new(TC_ROOT), &c, &level())
            .expect("a zero pick is legal here")
            .state;
        for (i, w) in st.worms.iter().enumerate() {
            assert_eq!(
                (w.lives, w.health, w.visible, w.stats_x, w.killed_timer),
                (0, 100, false, [0, 218][i], 150),
                "Worm::lives{{0}} (worm.hpp:238) until kStateGame"
            );
            assert!(
                w.weapons.iter().all(|ww| ww.ty.is_none() && ww.ammo == 0),
                "no InitWeapons yet"
            );
        }
        assert_eq!(st.rand.last(), 0, "building consumes no RNG");
        assert_eq!(st.weap_table.len(), 40);
    }

    #[test]
    fn build_match_is_new_match_then_init_weapons_then_enter_game() {
        let mut c = cfg();
        c.settings.lives = 7;
        c.settings.blood_particle_max = 300;
        c.settings.worm_settings[0].weapons = [2, 3, 4, 5, 6];
        c.settings.worm_settings[1].weapons = [40, 12, 1, 26, 31];
        let built = build_match(Path::new(TC_ROOT), &c, &level()).unwrap().state;
        let mut split = new_match(Path::new(TC_ROOT), &c, &level()).unwrap().state;
        let order = sim::weapsel::weap_order(&split.weapons);
        sim::weapsel::init_weapons(
            &mut split,
            &order,
            &[
                c.settings.worm_settings[0].weapons,
                c.settings.worm_settings[1].weapons,
            ],
        );
        enter_game(&mut split, &c);
        assert_eq!(built.worms, split.worms);
        assert_eq!(hash_components(&built), hash_components(&split));
        assert_eq!(hash_game_state(&built), hash_game_state(&split));
        assert_eq!(built.bobjects.capacity(), split.bobjects.capacity());
        assert_eq!(split.bobjects.capacity(), 300);
        assert_eq!(split.worms[1].lives, 7);
    }

    /// A `Settings` whose nine live fields (the eight sim fields and both worms' health) all
    /// differ from `Settings::default()`'s.
    fn live_edited() -> Settings {
        let mut s = Settings::default();
        s.max_bonuses += 3;
        s.weap_table[4] = 2;
        s.weap_table[9] = 1;
        s.game_mode = GM_SCALES_OF_JUSTICE;
        s.time_to_lose += 7;
        s.blood += 50;
        s.loading_time += 25;
        s.load_change = !s.load_change;
        s.shadow = !s.shadow;
        s.worm_settings[0].health += 5;
        s.worm_settings[1].health += 7;
        // Not live: none of these may reach the state.
        s.lives += 4;
        s.blood_particle_max += 11;
        s.map = !s.map;
        s.names_on_bonuses = !s.names_on_bonuses;
        s
    }

    #[test]
    fn apply_live_settings_writes_exactly_the_live_fields() {
        let mut state = new_match(Path::new(TC_ROOT), &cfg(), &level())
            .expect("new_match")
            .state;
        let before_hash = sim::hash::hash_game_state(&state);
        let before_parts = sim::hash::hash_components(&state);
        let before_rand = (state.rand.draws(), state.rand.last());
        let before_level = state.level.clone();
        let e = live_edited();
        let d = Settings::default();
        // Non-vacuous: every live field of `e` differs from the default.
        assert_ne!(e.max_bonuses, d.max_bonuses);
        assert_ne!(e.weap_table, d.weap_table);
        assert_ne!(e.game_mode, d.game_mode);
        assert_ne!(e.time_to_lose, d.time_to_lose);
        assert_ne!(e.blood, d.blood);
        assert_ne!(e.loading_time, d.loading_time);
        assert_ne!(e.load_change, d.load_change);
        assert_ne!(e.shadow, d.shadow);
        for i in 0..2 {
            assert_ne!(e.worm_settings[i].health, d.worm_settings[i].health);
        }

        let worms_before = state.worms.clone();
        apply_live_settings(&mut state, &e);

        assert_eq!(state.settings_max_bonuses, e.max_bonuses);
        assert_eq!(
            state.weap_table,
            e.weap_table.iter().map(|&v| v as i32).collect::<Vec<_>>()
        );
        assert_eq!(state.game_mode, e.game_mode);
        assert_eq!(state.time_to_lose, e.time_to_lose);
        assert_eq!(state.blood, e.blood);
        assert_eq!(state.settings_loading_time, e.loading_time);
        assert_eq!(state.load_change, e.load_change);
        assert_eq!(state.shadow, e.shadow);

        // Each worm's own max (4½f-1 T3; T0 P5: HEALTH reaches a paused match at RESUME).
        assert_eq!(
            (state.worms[0].max_health, state.worms[1].max_health),
            (e.worm_settings[0].health, e.worm_settings[1].health)
        );

        // Nothing else: the worms' other fields (lives, health: the next tick's clamp is the
        // sim's), the rand, the level, the cycles and the pools, and the state hash (none of the
        // live fields is hashed).
        let mut want_worms = worms_before;
        for (w, ws) in want_worms.iter_mut().zip(&e.worm_settings) {
            w.max_health = ws.health;
        }
        assert_eq!(
            state.worms, want_worms,
            "only max_health moves on the worms"
        );
        assert_eq!((state.rand.draws(), state.rand.last()), before_rand);
        assert!(state.level == before_level);
        assert_eq!(state.cycles, 0);
        assert_eq!(
            sim::hash::hash_components(&state),
            before_parts,
            "rng, level, worms and every pool"
        );
        assert_eq!(
            sim::hash::hash_game_state(&state),
            before_hash,
            "hash-neutral"
        );

        // Idempotent, and applying the defaults back restores new_match's fields.
        apply_live_settings(&mut state, &d);
        let fresh = new_match(Path::new(TC_ROOT), &cfg(), &level())
            .expect("new_match")
            .state;
        assert_eq!(state.settings_max_bonuses, fresh.settings_max_bonuses);
        assert_eq!(state.weap_table, fresh.weap_table);
        assert_eq!(
            (state.game_mode, state.time_to_lose, state.blood),
            (fresh.game_mode, fresh.time_to_lose, fresh.blood)
        );
        assert_eq!(
            (state.settings_loading_time, state.load_change, state.shadow),
            (fresh.settings_loading_time, fresh.load_change, fresh.shadow)
        );
        assert_eq!(state.worms, fresh.worms, "the maxes are back to 100");
    }

    #[test]
    fn a_health_edit_reaches_the_sim_live_and_the_next_tick_clamps() {
        // T0 P5: HEALTH 30 while paused reads 30 at once; the first resumed tick clamps the
        // worm's 100 to 30 (worm.cpp:213), and the other worm keeps its health under 250.
        let mut state = build_match(Path::new(TC_ROOT), &cfg(), &level())
            .expect("build_match")
            .state;
        let mut s = Settings::default();
        s.worm_settings[0].health = 30;
        s.worm_settings[1].health = 250;
        let draws = state.rand.draws();
        let hash = sim::hash::hash_game_state(&state);
        apply_live_settings(&mut state, &s);
        assert_eq!(
            (state.worms[0].max_health, state.worms[1].max_health),
            (30, 250)
        );
        assert_eq!(
            (state.worms[0].health, state.worms[1].health),
            (100, 100),
            "health itself waits for the tick"
        );
        assert_eq!(state.rand.draws(), draws, "it draws nothing");
        assert_eq!(sim::hash::hash_game_state(&state), hash, "hash-neutral");
        state.process_frame(&[sim::state::ControlState::default(); 2]);
        assert_eq!(
            (state.worms[0].health, state.worms[1].health),
            (30, 100),
            "the clamp is each worm's own max; nothing heals toward 250"
        );
    }

    #[test]
    fn new_match_takes_its_live_fields_through_apply_live_settings() {
        // The choke point: new_match of an edited setup equals new_match of the defaults with
        // the edit applied afterwards, on every live field.
        let mut edited = cfg();
        edited.settings = live_edited();
        edited.settings.worm_settings[0].health = 100;
        edited.settings.worm_settings[1].health = 100;
        edited.settings.game_mode = GM_SCALES_OF_JUSTICE;
        let direct = new_match(Path::new(TC_ROOT), &edited, &level())
            .expect("new_match")
            .state;
        let mut via = new_match(Path::new(TC_ROOT), &cfg(), &level())
            .expect("new_match")
            .state;
        apply_live_settings(&mut via, &edited.settings);
        assert_eq!(direct.settings_max_bonuses, via.settings_max_bonuses);
        assert_eq!(direct.weap_table, via.weap_table);
        assert_eq!(
            (direct.game_mode, direct.time_to_lose, direct.blood),
            (via.game_mode, via.time_to_lose, via.blood)
        );
        assert_eq!(
            (
                direct.settings_loading_time,
                direct.load_change,
                direct.shadow
            ),
            (via.settings_loading_time, via.load_change, via.shadow)
        );
        assert_eq!(
            (direct.worms[0].max_health, direct.worms[1].max_health),
            (via.worms[0].max_health, via.worms[1].max_health)
        );
        assert_eq!(
            sim::hash::hash_game_state(&direct),
            sim::hash::hash_game_state(&via)
        );
    }

    #[test]
    fn weapsel_config_maps_the_settings() {
        let mut s = Settings::default();
        s.weap_table[3] = 2;
        s.select_bot_weapons = 0;
        s.worm_settings[1].controller = 1;
        s.worm_settings[0].weapons = [0, 1, 2, 3, 4];
        s.worm_settings[2].weapons = [9; 5]; // the network player is not a player here
        let w = weapsel_config(&s);
        assert_eq!(w.weap_table, s.weap_table);
        assert_eq!(w.select_bot_weapons, 0);
        assert_eq!(w.players[0].weapons, [0, 1, 2, 3, 4]);
        assert_eq!((w.players[0].controller, w.players[1].controller), (0, 1));
        assert_eq!(w.players[1].weapons, [1; 5]);
    }

    // ---- 4½e-1 Addendum G3: safe edges (John's ruling) ----------------------------------

    /// `Level::GenerateFromSettings` over a `Rand` seeded `level_seed` (`file = None`): the
    /// NEW GAME / `generate` path at a small MAP WIDTH x MAP HEIGHT.
    fn generated(w: i32, h: i32, level_seed: u32) -> (Settings, LevelData) {
        use assets::sprite::{SpriteSet, Tga};
        use sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams};
        let (tc, _) = tc_and_objects();
        let tga =
            Tga::load(&std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).unwrap()).unwrap();
        let large = SpriteSet::from_tga(&tga, 16, 16, 110).unwrap();
        let s = Settings {
            random_level: true,
            random_map_width: w,
            random_map_height: h,
            ..Settings::default()
        };
        let params = LevelGenParams {
            random_level: true,
            random_map_width: w,
            random_map_height: h,
            shadow: s.shadow,
        };
        let assets = LevelGenAssets {
            large_sprites: &large,
            textures: &tc.textures,
            material_flags: &tc.materials,
        };
        let mut rand = sim_core::rng::Rand::new();
        rand.seed(level_seed);
        let level = generate_from_settings(&assets, &params, None, &mut rand);
        assert_eq!((level.width, level.height), (w, h));
        (s, level)
    }

    /// Run `ticks` ticks on a `w x h` generated level with Fire held (a dead worm respawns
    /// only once ready) and a deterministic walk/jump pattern, and return how many times each
    /// worm became visible. C++ reads past `materials[]` in `BeginRespawn` /
    /// `CheckRespawnPosition` on these sizes (UB); Rust reads rock there and must neither panic
    /// nor fail to spawn.
    fn spawns_on_small_level(w: i32, h: i32, seed: u32, ticks: u32) -> [u32; 2] {
        use sim::state::ControlState;
        let (s, level) = generated(w, h, seed);
        let c = MatchConfig {
            settings: s,
            seed: seed ^ 0x5eed,
        };
        let mut st = build_match(Path::new(TC_ROOT), &c, &level)
            .expect("a small level builds")
            .state;
        assert_eq!((st.level.width, st.level.height), (w, h));
        let mut spawns = [0u32; 2];
        let mut prev = [false; 2];
        let mut x = seed;
        for _ in 0..ticks {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let bits = [((x >> 16) & 0x7f) | 1 << 4, ((x >> 23) & 0x7f) | 1 << 4];
            st.process_frame(&[ControlState::unpack(bits[0]), ControlState::unpack(bits[1])]);
            for i in 0..2 {
                let vis = st.worms[i].visible;
                if vis && !prev[i] {
                    spawns[i] += 1;
                }
                prev[i] = vis;
            }
        }
        spawns
    }

    #[test]
    fn safe_edges_a_96x64_match_spawns_both_worms_without_panicking() {
        for seed in [1u32, 9664, 424242] {
            let spawns = spawns_on_small_level(96, 64, seed, 2000);
            assert!(
                spawns.iter().all(|&n| n >= 1),
                "96x64 seed {seed}: both worms spawn, got {spawns:?}"
            );
        }
    }

    #[test]
    fn safe_edges_a_160x120_match_spawns_both_worms_without_panicking() {
        for seed in [2u32, 160120, 777] {
            let spawns = spawns_on_small_level(160, 120, seed, 2000);
            assert!(
                spawns.iter().all(|&n| n >= 1),
                "160x120 seed {seed}: both worms spawn, got {spawns:?}"
            );
        }
    }

    /// A candidate exactly three rows below the level and far right of it: C++'s rock walk
    /// reads nothing and spins through a signed overflow; Rust accepts without the spin.
    #[test]
    fn safe_edges_a_100x200_match_survives_the_empty_walk() {
        let spawns = spawns_on_small_level(100, 200, 1, 8000);
        assert!(spawns.iter().all(|&n| n >= 1), "100x200: {spawns:?}");
    }

    /// The ruling at the accessor: an index inside the array (including the flat wrap for
    /// `x >= width`) reads the real pixel; one past the end, or before the start, reads rock.
    #[test]
    fn safe_edges_an_out_of_array_read_is_rock_and_in_array_reads_are_unchanged() {
        let (_, level) = generated(96, 64, 3);
        let st = build_match(Path::new(TC_ROOT), &cfg(), &level)
            .unwrap()
            .state;
        let l = &st.level;
        let flags = |idx: usize| l.material_flags[l.material_id[idx] as usize];
        // The defined flat wrap: (width + 5, 10) is (5, 11).
        let wrapped = (flags(5 + 11 * 96) & sim::state::MAT_BACKGROUND) != 0;
        assert_eq!(l.background(96 + 5, 10), wrapped);
        assert_eq!(l.background(96 + 5, 10), l.background(5, 11));
        assert_eq!(l.rock(96 + 5, 10), l.rock(5, 11));
        // Past the end (the last row, x beyond the width) and before the start.
        for (x, y) in [(96 + 40, 63), (0, 64), (400, 300), (-1, 0), (5, -3)] {
            assert!(l.rock(x, y), "({x}, {y}) is out of the array: rock");
            assert!(!l.background(x, y), "({x}, {y}): not background");
            assert!(!l.any_dirt(x, y) && !l.dirt(x, y) && !l.dirt2(x, y));
        }
        // `checked_mat_background`'s C++-defined out-of-range read is untouched.
        assert_eq!(
            l.checked_mat_background(0, 64),
            (l.material_flags[0] & sim::state::MAT_BACKGROUND) != 0
        );
    }

    // ---- 4½f-1 T2 Step 3: the CPU smoke (sim::ai over a real match) --------------------

    /// Two CPUs, fed only by `sim::ai::run_ais` (D8's flow: last tick's post-tick words, no
    /// key edges), on a generated 504 x 350 level for 2,000 ticks. Returns every tick's state
    /// hash, whether each worm was ever visible, and whether each word was ever non-zero.
    fn cpu_smoke() -> (Vec<u32>, [bool; 2], [bool; 2]) {
        use sim::ai::{run_ais, DumbLieroAi};
        let (s, level) = generated(504, 350, 4501);
        let c = MatchConfig {
            settings: s,
            seed: 4502,
        };
        let mut st = build_match(Path::new(TC_ROOT), &c, &level)
            .expect("the default setup builds")
            .state;
        let mut ais = [Some(DumbLieroAi::new()), Some(DumbLieroAi::new())];
        let (mut visible, mut pressed) = ([false; 2], [false; 2]);
        let mut hashes = Vec::with_capacity(2000);
        for _ in 0..2000 {
            let mut inputs = [st.worms[0].control_states, st.worms[1].control_states];
            run_ais(&mut ais, &st, &mut inputs);
            st.process_frame(&inputs);
            for i in 0..2 {
                pressed[i] |= inputs[i].pack() != 0;
                visible[i] |= st.worms[i].visible;
            }
            hashes.push(sim::hash::hash_game_state(&st));
        }
        (hashes, visible, pressed)
    }

    #[test]
    fn two_cpus_play_2000_ticks_deterministically() {
        let (a, visible, pressed) = cpu_smoke();
        assert_eq!(
            visible,
            [true, true],
            "both CPUs spawned (they press Fire while dead)"
        );
        assert_eq!(pressed, [true, true], "both CPUs pressed something");
        let (b, _, _) = cpu_smoke();
        assert_eq!(
            a, b,
            "the same seeds give the same state hash on every tick"
        );
    }
}
