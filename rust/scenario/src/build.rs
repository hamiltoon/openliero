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

use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use render::viewport::Viewport;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::pool::BloodPool;
use sim::state::{SimState, WormInit};
use sim_core::vec::Vec2;

use crate::loader::{load_sprites, scene_data, Loaded};
use crate::settings::{MatchConfig, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE, WEAP_TABLE_LEN};

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
    /// The two players' healths differ: the sim carries one `settings_health` (design
    /// §1.3 finding 4); lifted with the player-menu HEALTH item in 4½f.
    AsymmetricHealth { p1: i32, p2: i32 },
    /// A weapon pick outside `1..=weap_order.len()` (`worm.cpp:704` indexes unchecked).
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
            BuildError::AsymmetricHealth { p1, p2 } => {
                write!(f, "player healths differ ({p1} vs {p2}); not supported yet")
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

/// Check `cfg` against a TC with `n_weapons` weapons. Only the two playing worms
/// (indices 0 and 1) are checked; the network player never plays a local match.
pub fn validate(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError> {
    let s = &cfg.settings;
    if s.game_mode == GM_HOLDAZONE {
        return Err(BuildError::HoldazoneUnsupported);
    }
    if s.game_mode > GM_SCALES_OF_JUSTICE {
        return Err(BuildError::InvalidGameMode(s.game_mode));
    }
    let (p1, p2) = (s.worm_settings[0].health, s.worm_settings[1].health);
    for h in [p1, p2] {
        if h < 1 {
            return Err(BuildError::InvalidHealth(h));
        }
    }
    if p1 != p2 {
        return Err(BuildError::AsymmetricHealth { p1, p2 });
    }
    if n_weapons > WEAP_TABLE_LEN {
        return Err(BuildError::TooManyWeapons(n_weapons));
    }
    for worm in 0..2 {
        for (slot, &value) in s.worm_settings[worm].weapons.iter().enumerate() {
            if value == 0 || value as usize > n_weapons {
                return Err(BuildError::InvalidWeapon { worm, slot, value });
            }
        }
    }
    if s.blood_particle_max < 1 {
        return Err(BuildError::InvalidBloodParticleMax(s.blood_particle_max));
    }
    Ok(())
}

/// Build the tick-0 match for `cfg` on the ready `level` (design §4.2).
pub fn build_match(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
) -> Result<Loaded, BuildError> {
    let tc = TcConfig::load(&crate::assets::read_asset(tc_root, "tc.cfg")).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        Ok(crate::assets::read_asset(
            tc_root,
            &format!("{sub}/{id}.cfg"),
        ))
    })
    .expect("object configs load");
    validate(cfg, objects.weapons.len())?;
    let s = &cfg.settings;

    // weap_order: indices sorted by weapon name (Common::Precompute, common.cpp:491-499) — the
    // one shared copy since Step 4½c (design finding 12).
    let weap_order = sim::weapsel::weap_order(&objects.weapons);
    let worms_init: Vec<WormInit> = (0..2)
        .map(|i| {
            let ws = &s.worm_settings[i];
            WormInit {
                index: i as i32,
                health: ws.health,
                lives: s.lives,
                stats_x: if i == 0 { 0 } else { 218 },
                weapons: WormInit::resolve_weapons(&objects, &weap_order, &ws.weapons),
                start_pos: Vec2::zero(),
                visible: false,
            }
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

    // Settings (design §4.2).
    state.settings_max_bonuses = s.max_bonuses;
    state.weap_table = s.weap_table.iter().map(|&v| v as i32).collect();
    state.settings_health = s.worm_settings[0].health; // == [1] (validated)
    state.game_mode = s.game_mode;
    state.time_to_lose = s.time_to_lose;
    state.shadow = s.shadow;
    state.bobjects = BloodPool::new(s.blood_particle_max as usize);

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
    fn health_must_be_positive_and_equal_for_both_players() {
        let mut c = cfg();
        c.settings.worm_settings[0].health = 0;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidHealth(0)));
        let mut c = cfg();
        c.settings.worm_settings[1].health = 150;
        assert_eq!(
            validate(&c, 40),
            Err(BuildError::AsymmetricHealth { p1: 100, p2: 150 })
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
        assert_eq!(
            (st.settings_health, st.game_mode, st.time_to_lose),
            (150, 3, 99)
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
}
