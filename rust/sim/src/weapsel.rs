//! Step 4½c — the weapon selection phase (`weapsel.cpp`), Bevy-free (overview LD 7; design
//! `specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md`, cited **design §N**).
//!
//! Every C++ match starts with a pick screen whose constructor and RANDOMIZE item draw from
//! `game.rand`, the SIMULATION RNG (`weapsel.cpp:61`, `:68`, `:323`), so the number and order of
//! draws before match frame 0 decides every later tick. The phase therefore lives next to
//! `SimState`: it draws `state.rand` and writes the worms' control words and weapons (both hashed,
//! `hash.rs:49`, `:65-73`), and Step 5 snapshots it as `SimState` plus a `WeaponSelection` clone
//! (design §8). It is driven only from `tick_and_render` (LD 3); `game` owns the presentation.

use std::fmt;

use assets::object::Weapon;
use sim_core::rng::Rand;

use crate::state::{SimState, NUM_WEAPONS};

/// `rand(1, 41)` hard-codes forty weapons (`weapsel.cpp:61`, `:68`, `:323`, finding 8).
pub const WEAPON_COUNT: usize = 40;
/// RANDOMIZE, five weapon slots, DONE (`weapsel.cpp:49`, `:87`, `:90`).
pub const MENU_ITEMS: u8 = 7;
/// The RANDOMIZE item (`weapsel.cpp:316`).
pub const RANDOMIZE_ITEM: u8 = 0;
/// The DONE item (`weapsel.cpp:338`, "TODO: Unhardcode").
pub const DONE_ITEM: u8 = 6;
/// `LocalController::kKeyRepeatInitial` (`localController.hpp:44`).
pub const KEY_REPEAT_INITIAL: u16 = 12;
/// `LocalController::kKeyRepeatInterval` (`localController.hpp:45`).
pub const KEY_REPEAT_INTERVAL: u16 = 3;
/// The seven packed controls the repeat covers (`localController.cpp:130`).
const REPEAT_BITS: u32 = 7;

/// One player's saved setup the phase starts from (`WormSettings::weapons`, `::controller`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeapselPlayer {
    /// 1-based `weap_order` picks; `0` = unset, rolled by the constructor (`weapsel.cpp:60`).
    pub weapons: [u32; NUM_WEAPONS],
    /// 0 human, 1 DumbLieroAI, 2 FollowAI (`localController.cpp:19-27`).
    pub controller: u32,
}

/// What the phase reads from `Settings` (design §4.2). `scenario::build::weapsel_config` maps a
/// `Settings` onto it; `sim` does not depend on `scenario`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeapselConfig {
    /// `Settings::weap_table` by weapon index: 0 menu, 1 bonus only, 2 banned — enabled iff 0
    /// (`settings.hpp:68` is `uint32_t`, so `> 0`, `!= 0` and `<= 0` all mean the same).
    pub weap_table: [u32; WEAPON_COUNT],
    /// Raw `Settings::select_bot_weapons`: a bot is RANDOM iff `== 0` (`weapsel.cpp:57`) and
    /// readies at once iff `!= 1` (`:95`), so a file value >= 3 behaves as KEEP (finding 2).
    pub select_bot_weapons: u32,
    /// Index = worm index = viewport index (`localController.cpp:47-48`, `weapsel.cpp:44-47`).
    pub players: [WeapselPlayer; 2],
}

/// One player's menu (design §4.2). The menu is a seven-state cursor (finding 3: the items are
/// `emplace_back`ed, so `visible_item_count` stays 0 — no scrolling, no scrollbar), so picks,
/// cursor and the ready flag are the whole menu state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerSel {
    pub picks: [u32; NUM_WEAPONS],
    pub cursor: u8,
    pub ready: bool,
}

/// `LocalController`'s weapsel key repeat for one worm, on SAMPLED words (design §4.5): the
/// previous sampled word (replays `OnKey` from its change) and the per-bit held counters
/// (`worm_held_frames`, `localController.hpp:46`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyRepeat {
    prev: u32,
    held: [u16; REPEAT_BITS as usize],
}

/// A configuration C++ would hang or crash on (finding 8), refused instead (design §4.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeapselError {
    /// The TC does not have exactly 40 weapons (`rand(1, 41)` hard-codes 40).
    WeaponCount(usize),
    /// The phase needs exactly two worms (`localController.cpp:33-51`).
    WormCount(usize),
    /// Every weapon is disabled: the constructor loop, RANDOMIZE and cycling would never end.
    /// Mirrors `LS(NoWeaps)` (`weaponMenuState.cpp:91-109`).
    NoWeaponsEnabled,
    /// A saved pick above 40 indexes `weap_order` out of bounds (`weapsel.cpp:66`).
    InvalidPick {
        worm: usize,
        slot: usize,
        value: u32,
    },
}

impl fmt::Display for WeapselError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WeapselError::WeaponCount(n) => {
                write!(
                    f,
                    "weapon selection needs exactly {WEAPON_COUNT} weapons, the TC has {n}"
                )
            }
            WeapselError::WormCount(n) => write!(f, "weapon selection needs 2 worms, got {n}"),
            WeapselError::NoWeaponsEnabled => write!(f, "no weapon is enabled"),
            WeapselError::InvalidPick { worm, slot, value } => {
                write!(
                    f,
                    "player {worm} weapon slot {slot}: {value} is not a weapon"
                )
            }
        }
    }
}

impl std::error::Error for WeapselError {}

/// The phase (design §4.2): plain data, `Clone` — Step 5 snapshots it next to `SimState`
/// (design §8). `weap_order`, `weap_table` and `enabled_weaps` are derived once and immutable;
/// `menu_sounds` is per-frame output, not state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponSelection {
    weap_order: Vec<usize>,
    weap_table: [u32; WEAPON_COUNT],
    enabled_weaps: i32,
    players: [PlayerSel; 2],
    repeat: [KeyRepeat; 2],
    menu_sounds: Vec<i32>,
}

/// `game.rand(1, 41)` (`rand.hpp:36-38`: `rand(max - min) + min`, Lemire's bound).
fn roll(rand: &mut Rand) -> u32 {
    rand.bound_range(1, WEAPON_COUNT as u32 + 1)
}

impl WeaponSelection {
    /// The constructor (`weapsel.cpp:28-97`, design §3.2/§4.3), in C++ draw order: player 0's
    /// five slots, then player 1's. Draws `state.rand`, writes each worm's weapons as
    /// `{type, ammo 0}` (`:82-85`, the frozen HUD reads them) and `current_weapon = 0` (`:92`).
    /// Every refusal happens before the first draw.
    pub fn new(state: &mut SimState, cfg: &WeapselConfig) -> Result<Self, WeapselError> {
        if state.weapons.len() != WEAPON_COUNT {
            return Err(WeapselError::WeaponCount(state.weapons.len()));
        }
        if state.worms.len() != 2 {
            return Err(WeapselError::WormCount(state.worms.len()));
        }
        for (worm, p) in cfg.players.iter().enumerate() {
            for (slot, &value) in p.weapons.iter().enumerate() {
                if value as usize > WEAPON_COUNT {
                    return Err(WeapselError::InvalidPick { worm, slot, value });
                }
            }
        }
        // weapsel.cpp:35-39 — over all forty entries.
        let enabled_weaps = cfg.weap_table.iter().filter(|&&v| v == 0).count() as i32;
        if enabled_weaps == 0 {
            return Err(WeapselError::NoWeaponsEnabled);
        }
        let mut ws = WeaponSelection {
            weap_order: weap_order(&state.weapons),
            weap_table: cfg.weap_table,
            enabled_weaps,
            players: [PlayerSel::default(); 2],
            repeat: [KeyRepeat::default(); 2],
            menu_sounds: Vec::new(),
        };
        let enough = ws.enough();
        for i in 0..2 {
            let p = cfg.players[i];
            let random = p.controller != 0 && cfg.select_bot_weapons == 0; // :57
            let mut picks = p.weapons;
            let mut used = [false; WEAPON_COUNT]; // :42, per player
            for j in 0..NUM_WEAPONS {
                if picks[j] == 0 || random {
                    picks[j] = roll(&mut state.rand); // :60-62 — 0 or 1 draw
                }
                // :66 — the loop runs ONLY for a disabled pick, and checks uniqueness only
                // inside (finding 1): an enabled duplicate is kept.
                if ws.weap_table[ws.weapon_of(picks[j])] > 0 {
                    loop {
                        picks[j] = roll(&mut state.rand); // :68
                        let w = ws.weapon_of(picks[j]);
                        if (!enough || !used[w]) && ws.weap_table[w] == 0 {
                            break; // :72
                        }
                    }
                }
                let w = ws.weapon_of(picks[j]);
                used[w] = true; // :80
                let id = state.weapons[w].id;
                let slot = &mut state.worms[i].weapons[j];
                slot.ty = Some(id); // :85
                slot.ammo = 0; // :84
            }
            state.worms[i].current_weapon = 0; // :92
            ws.players[i] = PlayerSel {
                picks,
                cursor: RANDOMIZE_ITEM, // :94 MoveToFirstVisible
                ready: p.controller != 0 && cfg.select_bot_weapons != 1, // :95
            };
        }
        Ok(ws)
    }

    /// `enabled_weaps >= Settings::kSelectableWeapons` (`weapsel.cpp:64`, `:319`).
    fn enough(&self) -> bool {
        self.enabled_weaps >= NUM_WEAPONS as i32
    }

    /// The weapon index a 1-based pick names (`common.weap_order[pick - 1]`).
    fn weapon_of(&self, pick: u32) -> usize {
        self.weap_order[pick as usize - 1]
    }

    /// Player `i`'s menu (for drawing, the goldens and the write-back).
    pub fn player(&self, i: usize) -> &PlayerSel {
        &self.players[i]
    }

    /// The weapon index player `i`'s `slot` names — its menu label (`weapsel.cpp:87`, `:259`).
    pub fn weapon_index(&self, i: usize, slot: usize) -> usize {
        self.weapon_of(self.players[i].picks[slot])
    }

    /// `WeaponSelection::enabled_weaps` (`weapsel.hpp:23`).
    pub fn enabled_weaps(&self) -> i32 {
        self.enabled_weaps
    }

    /// Worm `i`'s seven key-repeat counters (the golden's `held` column).
    pub fn held(&self, i: usize) -> [u16; 7] {
        self.repeat[i].held
    }

    /// The menu sample ids the last `process_frame` played, in C++ call order (design §4.8):
    /// hash-inert, not snapshotted, drained by `game` into its `AudioSink`.
    pub fn menu_sounds(&self) -> &[i32] {
        &self.menu_sounds
    }
}

/// `Common::weap_order` (`common.cpp:491-499`): weapon indices sorted by name. A saved pick `p`
/// (1-based, `WormSettings::weapons`) names weapon `weap_order[p - 1]`. The one shared copy
/// (design §4.7). C++ sorts unstably and Rust stably; the two agree while the TC's weapon names
/// are unique, which `the_tc_weapon_names_are_unique_so_both_sorts_agree` pins (finding 12).
pub fn weap_order(weapons: &[Weapon]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..weapons.len()).collect();
    order.sort_by(|&a, &b| weapons[a].name.cmp(&weapons[b].name));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc_weapons() -> Vec<Weapon> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc =
            assets::tc::TcConfig::load(&std::fs::read(format!("{root}/tc.cfg")).unwrap()).unwrap();
        assets::object::Objects::load(&tc.types, |sub, id| {
            std::fs::read(format!("{root}/{sub}/{id}.cfg"))
        })
        .unwrap()
        .weapons
    }

    #[test]
    fn the_tc_weapon_names_are_unique_so_both_sorts_agree() {
        let weapons = tc_weapons();
        let mut names: Vec<&str> = weapons.iter().map(|w| w.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            weapons.len(),
            "finding 12: C++ sorts weap_order unstably (common.cpp:498-499), Rust stably — \
             they agree only while weapon names are unique"
        );
    }

    #[test]
    fn weap_order_is_the_byte_order_of_the_names() {
        let weapons = tc_weapons();
        let order = weap_order(&weapons);
        assert_eq!(order.len(), 40);
        let name = |pick: usize| weapons[order[pick - 1]].name.as_str();
        // The picks the 4½c corpus and the live default loadout rely on (T6, T9).
        assert_eq!(
            (name(1), name(12), name(26), name(31), name(40)),
            ("BAZOOKA", "DART", "LASER", "MISSILE", "ZIMM")
        );
        // std::string's `<` is a byte compare: the space (0x20) sorts before letters.
        assert_eq!(
            (name(28), name(29), name(30)),
            ("MINI NUKE", "MINI ROCKETS", "MINIGUN")
        );
    }

    // ---- 4½c T1: the constructor (weapsel.cpp:28-97) --------------------------------

    use crate::control::ControlConsts;
    use crate::physics::PhysicsConsts;
    use crate::state::{WeaponInit, WormInit};
    use assets::level::LevelData;
    use assets::sprite::SpriteSet;
    use assets::tc::SoundHooks;
    use sim_core::rng::Rand;
    use sim_core::vec::Vec2;

    pub(super) const MOVE_UP: i32 = 11;
    pub(super) const MOVE_DOWN: i32 = 12;
    pub(super) const SELECT: i32 = 13;

    /// `n` weapons named W00, W01, … so `weap_order` is the IDENTITY: pick `p` is weapon `p - 1`.
    fn weapons(n: usize) -> Vec<Weapon> {
        (0..n)
            .map(|i| Weapon {
                name: format!("W{i:02}"),
                id: i as i32,
                ammo: 100 + i as i32,
                ..Default::default()
            })
            .collect()
    }

    /// A 4x4 air level, `n_worms` fresh worms (no weapons), `n_weapons` identity-ordered weapons,
    /// menu sound hooks 11/12/13.
    fn state_n(seed: u32, n_weapons: usize, n_worms: usize) -> SimState {
        let level = LevelData {
            width: 4,
            height: 4,
            material_id: vec![0; 16],
            palette: None,
            display: None,
        };
        let worms: Vec<WormInit> = (0..n_worms)
            .map(|i| WormInit {
                index: i as i32,
                health: 100,
                lives: 0,
                stats_x: 0,
                weapons: [WeaponInit::default(); NUM_WEAPONS],
                start_pos: Vec2::zero(),
                visible: false,
            })
            .collect();
        let mut s = SimState::new(
            &level,
            &worms,
            seed,
            &[0u8; 256],
            weapons(n_weapons),
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            100,
            true,
            100,
        );
        s.sound_hooks = SoundHooks {
            MenuMoveUp: MOVE_UP,
            MenuMoveDown: MOVE_DOWN,
            MenuSelect: SELECT,
            ..SoundHooks::default()
        };
        s
    }

    fn state(seed: u32) -> SimState {
        state_n(seed, WEAPON_COUNT, 2)
    }

    fn cfg(
        weap_table: [u32; WEAPON_COUNT],
        select_bot_weapons: u32,
        p0: ([u32; NUM_WEAPONS], u32),
        p1: ([u32; NUM_WEAPONS], u32),
    ) -> WeapselConfig {
        WeapselConfig {
            weap_table,
            select_bot_weapons,
            players: [
                WeapselPlayer {
                    weapons: p0.0,
                    controller: p0.1,
                },
                WeapselPlayer {
                    weapons: p1.0,
                    controller: p1.1,
                },
            ],
        }
    }

    /// Two humans, every weapon enabled, the `Settings()` default picks `[1; 5]`, PICK.
    fn humans() -> WeapselConfig {
        cfg([0; WEAPON_COUNT], 1, ([1; 5], 0), ([1; 5], 0))
    }

    /// Only the 1-based `enabled` picks are enabled (identity order); the rest alternate 1/2.
    fn only(enabled: &[u32]) -> [u32; WEAPON_COUNT] {
        let mut t = [0u32; WEAPON_COUNT];
        for (i, v) in t.iter_mut().enumerate() {
            *v = 1 + (i as u32 % 2);
        }
        for &p in enabled {
            t[p as usize - 1] = 0;
        }
        t
    }

    /// The raw stream a `SimState` seeded with `seed` draws from.
    fn stream(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    #[test]
    fn saved_enabled_picks_draw_nothing_and_load_type_with_ammo_zero() {
        let mut st = state(1);
        st.worms[0].weapons[2].delay_left = 7; // :82-85 set type + ammo only
        let mut c = humans();
        c.players[0].weapons = [1, 2, 3, 4, 5];
        c.players[1].weapons = [40, 39, 38, 37, 36];
        let ws = WeaponSelection::new(&mut st, &c).expect("legal config");
        assert_eq!(
            (st.rand.draws(), st.rand.last()),
            (0, 0),
            "no roll, no loop"
        );
        assert_eq!(ws.player(0).picks, [1, 2, 3, 4, 5]);
        for j in 0..NUM_WEAPONS {
            assert_eq!(st.worms[0].weapons[j].ty, Some(j as i32));
            assert_eq!(st.worms[1].weapons[j].ty, Some(39 - j as i32));
            assert_eq!(st.worms[0].weapons[j].ammo, 0, "weapsel.cpp:84 ammo = 0");
        }
        assert_eq!(st.worms[0].weapons[2].delay_left, 7, "delay_left untouched");
        assert_eq!(st.worms[0].current_weapon, 0);
        assert_eq!(
            *ws.player(1),
            PlayerSel {
                picks: [40, 39, 38, 37, 36],
                cursor: 0,
                ready: false
            }
        );
        assert_eq!(ws.enabled_weaps(), 40);
    }

    #[test]
    fn a_zero_pick_draws_once_and_an_enabled_duplicate_is_kept() {
        let mut st = state(99);
        let mut c = humans();
        c.players[0].weapons = [0, 7, 0, 7, 0];
        c.players[1].weapons = [3; 5];
        let mut r = stream(99);
        let want = [
            r.bound_range(1, 41),
            7,
            r.bound_range(1, 41),
            7,
            r.bound_range(1, 41),
        ];
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(
            st.rand.draws(),
            3,
            "one draw per zero pick; all enabled => no loop"
        );
        assert_eq!(ws.player(0).picks, want);
        assert_eq!(
            ws.player(1).picks,
            [3; 5],
            "finding 1: a duplicate outside the loop is kept"
        );
    }

    #[test]
    fn a_random_bot_draws_all_five_even_over_saved_picks_and_is_ready() {
        let mut st = state(5);
        let c = cfg([0; WEAPON_COUNT], 0, ([2; 5], 0), ([9; 5], 1));
        let mut r = stream(5);
        let want: [u32; 5] = std::array::from_fn(|_| r.bound_range(1, 41));
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(st.rand.draws(), 5);
        assert_eq!(ws.player(0).picks, [2; 5], "the human keeps its picks");
        assert_eq!(ws.player(1).picks, want);
        assert!(!ws.player(0).ready && ws.player(1).ready, "weapsel.cpp:95");
    }

    #[test]
    fn a_disabled_pick_loops_until_an_enabled_weapon_duplicates_allowed() {
        // Only pick 8 enabled: `enough` is false, so uniqueness is waived (:72).
        let mut st = state(3);
        let c = cfg(only(&[8]), 1, ([1; 5], 0), ([8; 5], 0));
        let mut r = stream(3);
        let mut want_draws = 0;
        for _ in 0..NUM_WEAPONS {
            loop {
                want_draws += 1;
                if r.bound_range(1, 41) == 8 {
                    break;
                }
            }
        }
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(
            st.rand.draws(),
            want_draws,
            "player 1's enabled picks draw nothing"
        );
        assert_eq!(ws.player(0).picks, [8; 5]);
        assert!(want_draws > 5, "non-vacuous: the loop iterated");
    }

    #[test]
    fn with_enough_weapons_the_loop_also_rejects_used_weapons() {
        // Exactly five enabled (picks 1..=5): the loop yields a permutation (:64, :72).
        let mut st = state(11);
        let c = cfg(
            only(&[1, 2, 3, 4, 5]),
            1,
            ([40; 5], 0),
            ([1, 2, 3, 4, 5], 0),
        );
        let mut r = stream(11);
        let mut used = [false; 41];
        let mut want = [0u32; 5];
        for slot in want.iter_mut() {
            *slot = loop {
                let p = r.bound_range(1, 41);
                if p <= 5 && !used[p as usize] {
                    break p;
                }
            };
            used[*slot as usize] = true;
        }
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(ws.player(0).picks, want);
        let mut sorted = want;
        sorted.sort_unstable();
        assert_eq!(sorted, [1, 2, 3, 4, 5], "a permutation");
    }

    #[test]
    fn readiness_follows_the_raw_select_bot_weapons() {
        // Finding 2: RANDOM iff == 0, auto-ready iff != 1 — a value >= 3 behaves as KEEP.
        for (sbw, bot_ready) in [(0, true), (1, false), (2, true), (7, true)] {
            let mut st = state(2);
            let c = cfg([0; WEAPON_COUNT], sbw, ([1; 5], 0), ([1; 5], 1));
            let ws = WeaponSelection::new(&mut st, &c).unwrap();
            assert!(!ws.player(0).ready, "a human is never auto-ready");
            assert_eq!(ws.player(1).ready, bot_ready, "select_bot_weapons {sbw}");
            assert_eq!(st.rand.draws(), if sbw == 0 { 5 } else { 0 }, "sbw {sbw}");
        }
    }

    #[test]
    fn every_hazard_is_refused_before_any_draw() {
        let mut st = state_n(1, 39, 2);
        assert_eq!(
            WeaponSelection::new(&mut st, &humans()).unwrap_err(),
            WeapselError::WeaponCount(39)
        );
        let mut st = state_n(1, WEAPON_COUNT, 3);
        assert_eq!(
            WeaponSelection::new(&mut st, &humans()).unwrap_err(),
            WeapselError::WormCount(3)
        );
        let mut st = state(1);
        let none = cfg([1; WEAPON_COUNT], 1, ([0; 5], 0), ([0; 5], 0));
        assert_eq!(
            WeaponSelection::new(&mut st, &none).unwrap_err(),
            WeapselError::NoWeaponsEnabled
        );
        let mut c = humans();
        c.players[1].weapons[4] = 41;
        assert_eq!(
            WeaponSelection::new(&mut st, &c).unwrap_err(),
            WeapselError::InvalidPick {
                worm: 1,
                slot: 4,
                value: 41
            }
        );
        assert_eq!(st.rand.draws(), 0, "a refused config draws nothing");
        assert!(
            st.worms[0].weapons.iter().all(|w| w.ty.is_none()),
            "and writes nothing"
        );
    }
}
