//! Step 4½f-1 T6 — the G-AI + G-HP helpers, shared by `examples/gen_slice4_5f1_sim.rs` (via
//! `#[path]`) and `sim_slice4_5f_golden.rs`: the 8-case table, the setup sidecar, the generated
//! level, the start state (`build_match`, or 4½c's weapon-selection route), the driver that runs
//! the CPU players in `LocalController`'s order right before each tick (plan T6 Step 4), the
//! ledger (plan §Formats, from `run_ais_traced` and the driven state, D12) and the witnesses.
//! One copy, so the generator's seed choice and the gate's witness guard cannot drift apart.

#![allow(dead_code)] // each includer uses a subset.

use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use scenario::build::{build_match, enter_game, new_match, weapsel_config};
use scenario::settings::{
    MatchConfig, Settings, GM_GAME_OF_TAG, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE,
};
use scenario::settings_toml::settings_to_toml;
use sim::ai::{run_ais_traced, AiTrace, DumbLieroAi, Fallback, MaxDistArm};
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams};
use sim::state::{ControlState, SimState};
use sim::weapsel::WeaponSelection;
use sim_core::fixed::ftoi;
use sim_core::rng::Rand;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// Rows recorded after a game-over tick (>= the 180-frame C++ post-mortem).
pub const MARGIN: u32 = 200;

/// How many rows a case records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ticks {
    /// A fixed horizon; a game over inside it shortens the rows to the game-over tick +
    /// [`MARGIN`], which must still fit.
    Fixed(u32),
    /// Until the game-over tick + [`MARGIN`]; the match must end within the horizon.
    UntilOver(u32),
}

impl Ticks {
    pub fn horizon(self) -> u32 {
        match self {
            Ticks::Fixed(n) | Ticks::UntilOver(n) => n,
        }
    }
}

/// One G-AI / G-HP case (plan T6 Step 1's table).
pub struct Case {
    pub name: &'static str,
    pub width: i32,
    pub height: i32,
    /// The `generate <level_seed>` of the scenario (fixed per case).
    pub level_seed: u32,
    pub game_mode: u32,
    pub lives: i32,
    /// Players 0 and 1's `health` (their own max).
    pub health: [i32; 2],
    /// Players 0 and 1's `controller` (1 = CPU).
    pub controller: [u32; 2],
    pub max_bonuses: i32,
    pub time_to_lose: i32,
    pub select_bot_weapons: u32,
    /// The case runs 4½c's weapon-selection phase (`weapsel` lines) instead of InitWeapons.
    pub weapsel: bool,
    /// Player 0 is a human that presses nothing (no `input` lines).
    pub p0_idle: bool,
    /// Per player: `Some(names)` sets the five picks, `None` keeps the default picks.
    pub picks: [Option<[&'static str; 5]>; 2],
    pub ticks: Ticks,
    /// One line on what the case is for (the scenario header).
    pub about: &'static str,
}

impl Case {
    /// The case runs the `ai` directive (it has a CPU player).
    pub fn ai(&self) -> bool {
        self.controller.contains(&1)
    }
}

/// `ai_weapons`' CPU picks (plan T6: `time_to_explo` in `(0, 500)`, `0`, `>= 500`, a weapon
/// floored to 90, and a laser), chosen from the TC's weapon table: GRENADE (tte 115: the
/// Explo arm, 143), BAZOOKA (tte 0: the Speed arm, 200), ZIMM (tte 1000: the Speed arm, 300),
/// FAN (tte 45: the Explo arm, 55, floored to 90) and LASER (no laser sight: no
/// `cossin_table[128]` read at any angle).
pub const AI_WEAPONS_PICKS: [&str; 5] = ["GRENADE", "BAZOOKA", "ZIMM", "FAN", "LASER"];

pub const CASES: [Case; 8] = [
    Case {
        name: "ai_idle",
        width: 504,
        height: 350,
        level_seed: 45101,
        game_mode: GM_KILL_EM_ALL,
        lives: 3,
        health: [100, 100],
        controller: [0, 1],
        max_bonuses: 4,
        time_to_lose: 600,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: true,
        picks: [None, None],
        ticks: Ticks::Fixed(3000),
        about: "P2 the CPU against an idle human P1 (no input), default picks, Kill'em All lives 3",
    },
    Case {
        name: "ai_vs_human",
        width: 504,
        height: 350,
        level_seed: 45102,
        game_mode: GM_KILL_EM_ALL,
        lives: 3,
        health: [100, 100],
        controller: [0, 1],
        max_bonuses: 4,
        time_to_lose: 600,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: false,
        picks: [
            Some(["SHOTGUN", "BAZOOKA", "GRENADE", "CHAINGUN", "MINIGUN"]),
            Some(["UZI", "BAZOOKA", "GRENADE", "SHOTGUN", "CHAINGUN"]),
        ],
        ticks: Ticks::UntilOver(20000),
        about: "a fuzzed human P1 against the CPU P2, Kill'em All lives 3, to game over \
                (Hard gate 4's human-vs-CPU golden)",
    },
    Case {
        name: "ai_vs_ai",
        width: 504,
        height: 350,
        level_seed: 45103,
        game_mode: GM_KILL_EM_ALL,
        lives: 2,
        health: [100, 100],
        controller: [1, 1],
        max_bonuses: 4,
        time_to_lose: 600,
        select_bot_weapons: 0,
        weapsel: true,
        p0_idle: false,
        picks: [None, None],
        ticks: Ticks::UntilOver(30000),
        about: "two CPUs, BOT WEAPONS RANDOM through one weapsel frame, Kill'em All lives 2, \
                to game over",
    },
    Case {
        name: "ai_weapons",
        width: 504,
        height: 350,
        level_seed: 45104,
        game_mode: GM_KILL_EM_ALL,
        lives: 15,
        health: [100, 100],
        controller: [0, 1],
        max_bonuses: 4,
        time_to_lose: 600,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: false,
        picks: [
            Some(["SHOTGUN", "BAZOOKA", "GRENADE", "CHAINGUN", "MINIGUN"]),
            Some(AI_WEAPONS_PICKS),
        ],
        ticks: Ticks::Fixed(3000),
        about: "the CPU P2 with picks covering every max_dist arm and the floor, a fuzzed P1",
    },
    Case {
        name: "ai_close",
        width: 333,
        height: 360,
        level_seed: 333360,
        game_mode: GM_KILL_EM_ALL,
        lives: 15,
        health: [100, 100],
        controller: [0, 1],
        max_bonuses: 4,
        time_to_lose: 600,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: false,
        picks: [
            Some(["SHOTGUN", "BAZOOKA", "GRENADE", "CHAINGUN", "MINIGUN"]),
            Some(["UZI", "SHOTGUN", "CHAINGUN", "DART", "MINIGUN"]),
        ],
        ticks: Ticks::Fixed(3000),
        about: "a small 333x360 level: the CPU P2 close to a fuzzed P1 (fallback arms, the \
                rope, the reacts arms)",
    },
    Case {
        name: "hp_killemall",
        width: 504,
        height: 350,
        level_seed: 45201,
        game_mode: GM_KILL_EM_ALL,
        lives: 15,
        health: [50, 300],
        controller: [0, 0],
        max_bonuses: 10,
        time_to_lose: 600,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: false,
        picks: [None, None],
        ticks: Ticks::Fixed(3000),
        about: "unequal healths P1 50 / P2 300, MAX BONUSES 10, both fuzzed, Kill'em All",
    },
    Case {
        name: "hp_scales",
        width: 504,
        height: 350,
        level_seed: 45202,
        game_mode: GM_SCALES_OF_JUSTICE,
        lives: 5,
        health: [30, 200],
        controller: [0, 0],
        max_bonuses: 4,
        time_to_lose: 600,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: false,
        picks: [None, None],
        ticks: Ticks::Fixed(3000),
        about: "unequal healths P1 30 / P2 200, Scales of Justice lives 5, both fuzzed",
    },
    Case {
        name: "hp_tag",
        width: 504,
        height: 350,
        level_seed: 45203,
        game_mode: GM_GAME_OF_TAG,
        lives: 15,
        health: [1000, 10],
        controller: [0, 0],
        max_bonuses: 4,
        time_to_lose: 60,
        select_bot_weapons: 1,
        weapsel: false,
        p0_idle: false,
        picks: [None, None],
        ticks: Ticks::UntilOver(20000),
        about: "unequal healths P1 1000 / P2 10, Game of Tag, TIME TO LOSE 60, both fuzzed, to \
                game over",
    },
];

pub fn case(name: &str) -> &'static Case {
    CASES
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("unknown case {name}"))
}

pub fn load_tc() -> TcConfig {
    TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap()
}

pub fn load_objects(tc: &TcConfig) -> Objects {
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap()
}

/// The 1-based `weap_order` index a `WormSettings.weapons` entry stores.
pub fn menu_index(o: &Objects, name: &str) -> u32 {
    let order = sim::weapsel::weap_order(&o.weapons);
    order
        .iter()
        .position(|&i| o.weapons[i].name == name)
        .unwrap_or_else(|| panic!("no weapon {name:?} in weap_order")) as u32
        + 1
}

/// The case's settings: `Settings::default()` (a random level) with the case's fields.
pub fn settings(c: &Case, o: &Objects) -> Settings {
    let mut s = Settings {
        random_level: true,
        level_file: String::new(),
        random_map_width: c.width,
        random_map_height: c.height,
        game_mode: c.game_mode,
        lives: c.lives,
        max_bonuses: c.max_bonuses,
        time_to_lose: c.time_to_lose,
        select_bot_weapons: c.select_bot_weapons,
        ..Settings::default()
    };
    for i in 0..2 {
        s.worm_settings[i].health = c.health[i];
        s.worm_settings[i].controller = c.controller[i];
        if let Some(p) = c.picks[i] {
            s.worm_settings[i].weapons = p.map(|n| menu_index(o, n));
        }
    }
    s
}

/// The setup sidecar: C++ `Settings::ToToml` bytes (`settings_to_toml`, byte-identical to it).
pub fn setup_cfg(c: &Case, o: &Objects) -> String {
    settings_to_toml(&settings(c, o))
}

/// `GenerateFromSettings` with a `Rand` seeded `level_seed` (the dumper's `generate`).
pub fn generate(s: &Settings, level_seed: u32) -> LevelData {
    let tc = load_tc();
    let tga = Tga::load(&std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).unwrap()).unwrap();
    let large = SpriteSet::from_tga(&tga, 16, 16, 110).unwrap();
    let assets = LevelGenAssets {
        large_sprites: &large,
        textures: &tc.textures,
        material_flags: &tc.materials,
    };
    let params = LevelGenParams {
        random_level: s.random_level,
        random_map_width: s.random_map_width,
        random_map_height: s.random_map_height,
        shadow: s.shadow,
    };
    let mut rand = Rand::new();
    rand.seed(level_seed);
    generate_from_settings(&assets, &params, None, &mut rand)
}

/// 4½a's fuzz: per tick `Rand(input_seed).next_u32() & 0x7f` for worm 0 then worm 1; a CPU
/// worm's and an idle human's word is 0 (the draw is still made, so the stream is 4½a's).
pub fn inputs(c: &Case, input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    let quiet = [c.controller[0] == 1 || c.p0_idle, c.controller[1] == 1];
    (0..ticks)
        .map(|_| {
            let w = [r.next_u32() & 0x7f, r.next_u32() & 0x7f];
            [0, 1].map(|i| if quiet[i] { 0 } else { w[i] })
        })
        .collect()
}

/// The match's start state and its CPU players, exactly as the C++ dumper builds them: the
/// settings path's LocalController start state (`build_match`), or — with `weapsel` frames —
/// 4½c's route (`new_match`, the REAL `WeaponSelection` constructor, one `process_frame` per
/// frame, `finalize`, then `enter_game` = the dumper's `ResetWorms`). With `ai`, a fresh
/// `DumbLieroAi` (`mt19937(0x1337)`) per controller-1 player.
pub fn start(
    settings: &Settings,
    seed: u32,
    level: &LevelData,
    ai: bool,
    weapsel: &[[u32; 2]],
) -> (SimState, [Option<DumbLieroAi>; 2]) {
    let cfg = MatchConfig {
        settings: settings.clone(),
        seed,
    };
    let st = if weapsel.is_empty() {
        build_match(Path::new(TC_ROOT), &cfg, level)
            .expect("the case config builds")
            .state
    } else {
        let mut st = new_match(Path::new(TC_ROOT), &cfg, level)
            .expect("the case config builds")
            .state;
        let mut ws =
            WeaponSelection::new(&mut st, &weapsel_config(settings)).expect("the selection starts");
        let end = weapsel.len() - 1;
        for (f, w) in weapsel.iter().enumerate() {
            let done = ws.process_frame(&mut st, &w.map(ControlState::unpack));
            assert_eq!(done, f == end, "the phase ends on the last weapsel frame");
        }
        ws.finalize(&mut st);
        enter_game(&mut st, &cfg);
        st
    };
    let ais =
        [0, 1].map(|i| (ai && settings.worm_settings[i].controller == 1).then(DumbLieroAi::new));
    (st, ais)
}

/// One golden row: 12 columns, or 16 with `ai`.
pub struct Row {
    pub tick: u32,
    pub hashes: [u32; 10], // master, rng, level, worm0, worm1, bob, bon, sob, nob, wob
    pub game_over: u32,
    /// Columns 13-16 per worm: `None` for `- -`, else `(Pack, rand.last)`.
    pub ai: Option<[Option<(u32, u32)>; 2]>,
}

pub fn parse_golden(text: &str) -> Vec<Row> {
    let rows: Vec<Row> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert!(
                cols.len() == 12 || cols.len() == 16,
                "a G-AI/G-HP row has 12 or 16 columns: {line}"
            );
            let mut hashes = [0u32; 10];
            for (i, c) in cols[1..11].iter().enumerate() {
                hashes[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            let game_over: u32 = cols[11].parse().expect("game-over column");
            assert!(game_over <= 1, "IsGameOver column is 0/1, got {game_over}");
            let ai = (cols.len() == 16).then(|| {
                [0, 1].map(|w| {
                    let (p, l) = (cols[12 + 2 * w], cols[13 + 2 * w]);
                    if p == "-" {
                        assert_eq!(l, "-", "a human's two AI columns are both `-`");
                        None
                    } else {
                        Some((
                            u32::from_str_radix(p, 16).expect("hex word"),
                            u32::from_str_radix(l, 16).expect("hex rand.last"),
                        ))
                    }
                })
            });
            Row {
                tick: cols[0].parse().expect("tick"),
                hashes,
                game_over,
                ai,
            }
        })
        .collect();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(r.tick, k as u32, "golden row {k} carries tick {}", r.tick);
    }
    rows
}

const HASH_NAMES: [&str; 10] = [
    "master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob",
];

/// Row `row.tick`'s columns against the Rust state and this pass's AI columns; the first
/// difference panics with the tick and the column. The AI columns first (they are made
/// before the tick), then the hash components, the master and IsGameOver.
pub fn check(state: &SimState, ai: [Option<(u32, u32)>; 2], row: &Row) {
    if let Some(want) = row.ai {
        let got = if row.tick == 0 { [None, None] } else { ai };
        for w in 0..2 {
            let show = |c: Option<(u32, u32)>| match c {
                None => "- -".to_string(),
                Some((p, l)) => format!("{p:02x} {l:08x}"),
            };
            assert_eq!(
                got[w],
                want[w],
                "tick {}: AI columns of worm {w} (cols {}-{}): got `{}` want `{}`",
                row.tick,
                13 + 2 * w,
                14 + 2 * w,
                show(got[w]),
                show(want[w])
            );
        }
    }
    let c = hash_components(state);
    let got = [
        hash_game_state(state),
        c.rng,
        c.level,
        c.worms[0],
        c.worms[1],
        c.bobjects,
        c.bonuses,
        c.sobjects,
        c.nobjects,
        c.wobjects,
    ];
    for i in (1..10).chain(0..1) {
        assert_eq!(
            got[i],
            row.hashes[i],
            "tick {}: {} (col {}): got {:08x} want {:08x}",
            row.tick,
            HASH_NAMES[i],
            i + 2,
            got[i],
            row.hashes[i]
        );
    }
    assert_eq!(
        is_game_over(state) as u32,
        row.game_over,
        "tick {}: IsGameOver (col 12)",
        row.tick
    );
}

/// The ten fallback arms (`worm.cpp:557-593`) in the ledger's order, with their labels.
pub const FALLBACK_ARMS: [(i32, bool, &str); 10] = [
    (64, true, "64+r"),
    (80, true, "80+r"),
    (80, false, "80"),
    (96, true, "96+r"),
    (116, false, "116"),
    (48, true, "48+r"),
    (32, true, "32+r"),
    (48, false, "48"),
    (12, true, "12+r"),
    (12, false, "12"),
];

fn fallback_index(f: Fallback) -> usize {
    FALLBACK_ARMS
        .iter()
        .position(|&(b, d, _)| b == f.base && d == f.drew)
        .expect("one of the ten fallback arms")
}

/// Everything the witnesses read, from `run_ais_traced` and the genuinely driven state.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    /// Rows the golden records (ticks); row 0 is the start state.
    pub rows: u32,
    pub game_over_tick: Option<u32>,
    /// `is_game_over` stayed true on every tick after it first fired.
    pub game_over_held: bool,
    /// Every worm's whole-unit `aiming_angle` stayed in `0..128` (no `cossin_table[128]`).
    pub angle_ok: bool,
    /// The tick-0 `rng` column (`rand.last` after the start: a selection draw shows here).
    pub tick0_rng: u32,
    pub cpu: [bool; 2],
    // ---- the AI (per call of a CPU worm; summed over the CPU worms) ----
    pub runs: [u32; 2],
    pub fire_draws: u32,
    /// Visible, `real_dist < max_dist`: the in-range Fire arm.
    pub fire_in_range: u32,
    /// Change not pressed and `real_dist > max_dist`: the walk arm (`worm.cpp:652-655`).
    pub walk: u32,
    pub fallback: [u32; 10],
    pub change: u32,
    pub rope: u32,
    /// The Left / Right `reacts` arms.
    pub reacts: [u32; 2],
    pub explo: u32,
    pub speed: u32,
    pub floored: u32,
    /// The least `real_dist` of a call made while both worms were visible.
    pub min_real_dist: Option<i32>,
    /// The weapon types a CPU worm held when its AI ran, in first-seen order (both CPUs).
    pub cpu_weapons: Vec<i32>,
    /// Their names.
    pub cpu_weapon_names: Vec<String>,
    /// Per worm, its five loaded weapons with their `max_dist` arm (the ledger's picks line).
    pub pick_notes: [Vec<String>; 2],
    /// Tick 1's AI `rand.last` per worm (`None` for a human).
    pub tick1_last: [Option<u32>; 2],
    /// With two CPUs: the first tick whose two `rand.last` columns differ.
    pub first_differ: Option<u32>,
    /// `cycles % 2` of the AI steps seen (both orders of `localController.cpp:156-164`).
    pub phases: [bool; 2],
    // ---- deaths and respawns ----
    /// Invisible -> visible transitions, the first spawns included.
    pub spawned: [bool; 2],
    /// `(tick, worm, last_killed_by_idx)`.
    pub deaths: Vec<(u32, usize, i32)>,
    /// `(tick, worm, health)` of a spawn after a death.
    pub respawns: Vec<(u32, usize, i32)>,
    // ---- health ----
    pub max: [i32; 2],
    /// `(tick, worm, health before, health after)`: a health bonus the worm picked.
    pub bonus_picks: Vec<(u32, usize, i32, i32)>,
    /// Visible ticks with `health < max_health / 4` (the low-health blood gate), per worm.
    pub drips: [u32; 2],
    /// `(tick, worm)`: a Scales heal wrapped the worm's own max into an extra life.
    pub wraps: Vec<(u32, usize)>,
    /// Ticks on which worm i's `lives * max + health` rose while the other's fell (Scales).
    pub transfers_onto: [u32; 2],
}

impl Ledger {
    pub fn deaths_of(&self, w: usize) -> u32 {
        self.deaths.iter().filter(|d| d.1 == w).count() as u32
    }
    pub fn respawns_of(&self, w: usize) -> u32 {
        self.respawns.iter().filter(|d| d.1 == w).count() as u32
    }
    pub fn kills(&self, victim: usize, by: i32) -> u32 {
        self.deaths
            .iter()
            .filter(|d| d.1 == victim && d.2 == by)
            .count() as u32
    }
    pub fn picks_of(&self, w: usize) -> u32 {
        self.bonus_picks.iter().filter(|p| p.1 == w).count() as u32
    }
    /// Health-bonus heals that ended at the picker's own max (`DoHealing`'s `min`).
    pub fn clamps(&self) -> u32 {
        self.bonus_picks
            .iter()
            .filter(|p| p.3 == self.max[p.1])
            .count() as u32
    }
    pub fn wraps_of(&self, w: usize) -> u32 {
        self.wraps.iter().filter(|p| p.1 == w).count() as u32
    }
    pub fn fallback_drew(&self) -> u32 {
        (0..10)
            .filter(|&i| FALLBACK_ARMS[i].1)
            .map(|i| self.fallback[i])
            .sum()
    }
    pub fn fallback_quiet(&self) -> u32 {
        (0..10)
            .filter(|&i| !FALLBACK_ARMS[i].1)
            .map(|i| self.fallback[i])
            .sum()
    }
}

/// Worm `w`'s five loaded weapons, each with its `max_dist` arm (`worm.cpp:497-506`).
fn pick_notes(st: &SimState, w: usize) -> Vec<String> {
    st.worms[w]
        .weapons
        .iter()
        .map(|slot| {
            let wp = &st.weapons[slot.ty.expect("loaded") as usize];
            let (arm, d) = if wp.time_to_explo > 0 && wp.time_to_explo < 500 {
                (
                    "explo",
                    (wp.time_to_explo - wp.time_to_explo_v / 2) * wp.speed / 130,
                )
            } else {
                ("speed", wp.speed - wp.gravity / 10)
            };
            let fl = if d < 90 { " floored 90" } else { "" };
            let laser = if wp.laser_sight { ", laser sight" } else { "" };
            format!(
                "{} (tte {}, {arm} {d}{fl}{laser})",
                wp.name, wp.time_to_explo
            )
        })
        .collect()
}

fn total(s: &SimState, i: usize) -> i64 {
    i64::from(s.worms[i].lives) * i64::from(s.worms[i].max_health) + i64::from(s.worms[i].health)
}

fn angles_ok(s: &SimState) -> bool {
    s.worms
        .iter()
        .all(|w| (0..128).contains(&ftoi(w.aiming_angle)))
}

fn live_health_bonuses(s: &SimState) -> usize {
    (0..s.bonuses.capacity())
        .filter(|&i| s.bonuses.get(i).is_some_and(|b| b.frame == 1))
        .count()
}

/// Drive `ticks` ticks from `(st, ais)` the way the C++ `ai` pass runs them: a human's word is
/// its `input` (unpacked wholesale), a CPU's is the word its worm's last `Worm::Process` left
/// (`control_states`, fact 14); then `run_ais` in `(i + cycles % 2) % 2` order; then the tick.
/// With `golden`, row k is checked after the pass producing it (its AI columns come from this
/// pass's AI step). With `stop_after_over`, it stops `MARGIN` ticks after the first game over.
pub fn drive(
    mut st: SimState,
    mut ais: [Option<DumbLieroAi>; 2],
    ins: &[[u32; 2]],
    ticks: u32,
    stop_after_over: bool,
    golden: Option<&[Row]>,
    mut on_tick: impl FnMut(u32),
) -> Ledger {
    let cpu = [ais[0].is_some(), ais[1].is_some()];
    let mut l = Ledger {
        game_over_held: true,
        angle_ok: angles_ok(&st),
        tick0_rng: hash_components(&st).rng,
        cpu,
        max: [st.worms[0].max_health, st.worms[1].max_health],
        ..Ledger::default()
    };
    l.pick_notes = [0, 1].map(|w| pick_notes(&st, w));
    if let Some(g) = golden {
        check(&st, [None, None], &g[0]);
    }
    let mut died = [false; 2];
    let mut prev_vis = [st.worms[0].visible, st.worms[1].visible];
    let mut prev_tot = [total(&st, 0), total(&st, 1)];
    let mut k = 0u32;
    while k < ticks {
        k += 1;
        on_tick(k);
        let t = (k - 1) as usize;
        let mut inputs = [0, 1].map(|i| {
            if cpu[i] {
                st.worms[i].control_states
            } else {
                ControlState::unpack(ins[t][i])
            }
        });
        let mut traces = [AiTrace::default(); 2];
        l.phases[st.cycles.rem_euclid(2) as usize] = true;
        run_ais_traced(&mut ais, &st, &mut inputs, &mut traces);
        let both_visible = st.worms[0].visible && st.worms[1].visible;
        for (w, tr) in traces.iter().enumerate() {
            if !tr.ran {
                continue;
            }
            l.runs[w] += 1;
            l.fire_draws += u32::from(tr.fire_drew);
            l.fire_in_range += u32::from(st.worms[w].visible && tr.real_dist < tr.max_dist);
            l.walk += u32::from(!tr.change && tr.real_dist > tr.max_dist);
            if let Some(f) = tr.fallback {
                l.fallback[fallback_index(f)] += 1;
            }
            l.change += u32::from(tr.change);
            l.rope += u32::from(tr.rope_attached);
            l.reacts[0] += u32::from(tr.reacts_press[0]);
            l.reacts[1] += u32::from(tr.reacts_press[1]);
            match tr.max_dist_arm {
                MaxDistArm::Explo => l.explo += 1,
                MaxDistArm::Speed => l.speed += 1,
            }
            l.floored += u32::from(tr.max_dist_floored);
            if both_visible {
                l.min_real_dist = Some(
                    l.min_real_dist
                        .map_or(tr.real_dist, |m| m.min(tr.real_dist)),
                );
            }
            let me = &st.worms[w];
            let ty = me.weapons[me.current_weapon as usize].ty.expect("a weapon");
            if !l.cpu_weapons.contains(&ty) {
                l.cpu_weapons.push(ty);
                l.cpu_weapon_names
                    .push(st.weapons[ty as usize].name.clone());
            }
        }
        let ai_cols = [0, 1].map(|w| ais[w].as_ref().map(|a| (inputs[w].pack(), a.rand.last())));
        if k == 1 {
            l.tick1_last = ai_cols.map(|c| c.map(|c| c.1));
        }
        if let [Some(a), Some(b)] = ai_cols {
            if a.1 != b.1 && l.first_differ.is_none() {
                l.first_differ = Some(k);
            }
        }
        let prev_health = [st.worms[0].health, st.worms[1].health];
        let prev_lives = [st.worms[0].lives, st.worms[1].lives];
        let prev_bonuses = live_health_bonuses(&st);

        st.process_frame(&inputs);

        if let Some(g) = golden {
            check(&st, ai_cols, &g[k as usize]);
        }
        l.angle_ok &= angles_ok(&st);
        let bonus_gone = live_health_bonuses(&st) < prev_bonuses;
        for i in 0..2 {
            let w = &st.worms[i];
            if prev_vis[i] && !w.visible {
                l.deaths.push((k, i, w.last_killed_by_idx));
                died[i] = true;
            }
            if !prev_vis[i] && w.visible {
                l.spawned[i] = true;
                if died[i] {
                    l.respawns.push((k, i, w.health));
                }
            }
            if prev_vis[i]
                && w.visible
                && w.lives == prev_lives[i]
                && w.health > prev_health[i]
                && bonus_gone
            {
                l.bonus_picks.push((k, i, prev_health[i], w.health));
            }
            if w.visible && w.health < w.max_health / 4 {
                l.drips[i] += 1;
            }
            for _ in prev_lives[i]..w.lives.max(prev_lives[i]) {
                l.wraps.push((k, i));
            }
            prev_vis[i] = w.visible;
        }
        let tot = [total(&st, 0), total(&st, 1)];
        for i in 0..2 {
            if tot[i] > prev_tot[i] && tot[1 - i] < prev_tot[1 - i] {
                l.transfers_onto[i] += 1;
            }
        }
        prev_tot = tot;
        if l.game_over_tick.is_none() {
            if is_game_over(&st) {
                l.game_over_tick = Some(k);
            }
        } else {
            l.game_over_held &= is_game_over(&st);
        }
        if stop_after_over && l.game_over_tick.is_some_and(|g| k >= g + MARGIN) {
            break;
        }
    }
    l.rows = k;
    l
}

/// The rows the case records for a ledger, or why the run cannot be a golden.
pub fn rows_ok(c: &Case, l: &Ledger) -> Result<(), String> {
    if !l.angle_ok {
        return Err("an aiming_angle left 0..128".into());
    }
    if !l.game_over_held {
        return Err("IsGameOver flipped back".into());
    }
    match (c.ticks, l.game_over_tick) {
        (Ticks::UntilOver(_), None) => Err("no game over within the horizon".into()),
        (_, Some(g)) if g + MARGIN > c.ticks.horizon() => {
            Err(format!("game over at {g}: the margin does not fit"))
        }
        _ => Ok(()),
    }
}

/// The case's witnesses (plan T6 Step 1's table); `Err` names the first one missing.
pub fn witnesses(c: &Case, l: &Ledger) -> Result<(), String> {
    rows_ok(c, l)?;
    let need = |ok: bool, what: &str| if ok { Ok(()) } else { Err(what.to_string()) };
    match c.name {
        "ai_idle" => {
            need(l.spawned[1], "the CPU is visible")?;
            need(l.walk > 0, "the walk arm")?;
            need(l.fire_in_range > 0, "the in-range Fire arm")?;
            need(
                l.kills(0, 1) >= 1 || l.deaths_of(1) >= 1,
                "a P1 death by P2, or a P2 death",
            )?;
            need(
                l.deaths_of(1) == 0 || l.respawns_of(1) >= 1,
                "a dead CPU readies itself (its own Fire toggles) and respawns",
            )
        }
        "ai_vs_human" => {
            let over = l.game_over_tick.unwrap_or(u32::MAX);
            let before = |victim: usize, by: i32| {
                l.deaths
                    .iter()
                    .any(|d| d.1 == victim && d.2 == by && d.0 < over)
            };
            need(before(0, 1), "P1 killed by P2 before the game over")?;
            need(before(1, 0), "P2 killed by P1 before the game over")?;
            need(l.respawns_of(1) >= 1, "a CPU respawn")?;
            need(l.game_over_tick.is_some(), "game over")
        }
        "ai_vs_ai" => {
            need(
                l.tick0_rng != 0,
                "the RANDOM constructor drew (tick-0 rng != 0)",
            )?;
            need(l.first_differ.is_some(), "the two AI streams diverge")?;
            need(l.phases == [true, true], "both orders of cycles % 2")?;
            need(l.respawns_of(0) + l.respawns_of(1) >= 1, "a CPU respawn")?;
            need(l.game_over_tick.is_some(), "game over")
        }
        "ai_weapons" => {
            need(l.explo > 0 && l.speed > 0, "both max_dist arms")?;
            need(l.floored > 0, "the max_dist floor")?;
            need(
                l.cpu_weapons.len() >= 3,
                ">= 3 distinct CPU weapons (the Change arm)",
            )
        }
        "ai_close" => {
            need(l.fallback_drew() > 0, "a rand(16) fallback")?;
            need(l.fallback_quiet() > 0, "a non-drawing fallback arm")?;
            need(l.rope > 0, "a rope-attached Change tick")?;
            need(l.reacts[0] + l.reacts[1] > 0, "a reacts press")?;
            need(
                l.min_real_dist.is_some(),
                "both worms visible at an AI step",
            )
        }
        "hp_killemall" => {
            need(
                l.clamps() >= 1,
                "a health bonus clamped at the picker's own max",
            )?;
            need(l.picks_of(0) + l.picks_of(1) >= 1, "a health bonus picked")?;
            need(l.drips[0] >= 1, "P1 under its own quarter (12)")?;
            // P2 at 300 practically never dies in 3,000 fuzz ticks (no seed of 2,000 scanned
            // did), so its own max shows at its first spawn and in the pickups instead.
            need(l.respawns_of(0) >= 1, "a P1 respawn")?;
            need(
                l.respawns.iter().all(|r| r.2 == l.max[r.1]),
                "every respawn restores the worm's own max",
            )
        }
        "hp_scales" => {
            need(
                l.transfers_onto[0] >= 1 && l.transfers_onto[1] >= 1,
                "Scales transfers onto each worm",
            )?;
            need(l.wraps_of(0) >= 1, "a wrap into an extra life for P1")
        }
        "hp_tag" => {
            let (d0, d1) = (l.deaths_of(0), l.deaths_of(1));
            need(d1 >= 3 && d1 >= 3 * d0.max(1), "P2 deaths >> P1 deaths")?;
            need(l.game_over_tick.is_some(), "game over")
        }
        other => panic!("unknown case {other}"),
    }
}

fn list(items: Vec<String>, max: usize) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    let n = items.len();
    let head: Vec<String> = items.into_iter().take(max).collect();
    let tail = if n > max {
        format!(", ... ({n} in all)")
    } else {
        String::new()
    };
    format!("{}{tail}", head.join(", "))
}

fn who(by: i32) -> String {
    if by < 0 {
        "none".to_string()
    } else {
        format!("w{by}")
    }
}

/// The scenario header's ledger (plan §Formats), every line starting `# LEDGER`. The gate
/// re-derives it from its own drive and compares it line for line.
pub fn ledger_lines(c: &Case, l: &Ledger) -> Vec<String> {
    let mut out = Vec::new();
    out.push(match l.game_over_tick {
        None => "# LEDGER (Rust, driven state): never over".to_string(),
        Some(g) => format!(
            "# LEDGER (Rust, driven state): game over at tick {g}, over for {} rows",
            l.rows - g + 1
        ),
    });
    let deaths: Vec<String> = l
        .deaths
        .iter()
        .map(|(t, w, by)| format!("t{t}/w{w} by {}", who(*by)))
        .collect();
    let respawns: Vec<String> = l
        .respawns
        .iter()
        .map(|(t, w, h)| format!("t{t}/w{w} at {h}"))
        .collect();
    if c.ai() {
        let cpus: Vec<usize> = (0..2).filter(|&w| l.cpu[w]).collect();
        assert!(
            cpus.iter().all(|&w| l.runs[w] == l.runs[cpus[0]]),
            "every CPU runs once per tick"
        );
        let arms: Vec<String> = (0..10)
            .filter(|&i| l.fallback[i] > 0)
            .map(|i| format!("{}:{}", FALLBACK_ARMS[i].2, l.fallback[i]))
            .collect();
        let arms = if arms.is_empty() {
            "none".to_string()
        } else {
            arms.join(" ")
        };
        out.push(format!(
            "# LEDGER ai: runs {} per worm; fire draws {}; fallback {arms}; change {}; \
             rope-attached {}; reacts presses L {} R {}; max_dist arms explo {} speed {} \
             floored {}; deaths {} {}; respawns {} {}",
            l.runs[cpus[0]],
            l.fire_draws,
            l.change,
            l.rope,
            l.reacts[0],
            l.reacts[1],
            l.explo,
            l.speed,
            l.floored,
            l.deaths_of(0),
            l.deaths_of(1),
            l.respawns_of(0),
            l.respawns_of(1),
        ));
        let names = &l.cpu_weapon_names;
        let show = |x: Option<u32>| x.map_or("-".to_string(), |v| format!("{v:08x}"));
        out.push(format!(
            "# LEDGER detail: CPU worms {}; walk {}; fire in range {}; min real_dist (both \
             visible) {}; CPU weapons held {}; tick-1 rand.last {} {}; the two AI streams first \
             differ at {}; tick-0 rng {:08x}",
            cpus.iter()
                .map(|w| format!("w{w}"))
                .collect::<Vec<_>>()
                .join(" "),
            l.walk,
            l.fire_in_range,
            l.min_real_dist
                .map_or("none".to_string(), |d| d.to_string()),
            names.join(", "),
            show(l.tick1_last[0]),
            show(l.tick1_last[1]),
            l.first_differ
                .map_or("-".to_string(), |t| format!("tick {t}")),
            l.tick0_rng,
        ));
        for &w in &cpus {
            out.push(format!(
                "# LEDGER detail: w{w} picks {}",
                l.pick_notes[w].join(", ")
            ));
        }
    }
    out.push(format!("# LEDGER detail: deaths at {}", list(deaths, 16)));
    out.push(format!(
        "# LEDGER detail: respawns at {}",
        list(respawns, 16)
    ));
    if c.name.starts_with("hp_") {
        out.push(format!(
            "# LEDGER hp: max {} {}; clamps {}; health bonuses {} {}; drips {}; scales wraps {} {}",
            l.max[0],
            l.max[1],
            l.clamps(),
            l.picks_of(0),
            l.picks_of(1),
            l.drips[0] + l.drips[1],
            l.wraps_of(0),
            l.wraps_of(1),
        ));
        let picks: Vec<String> = l
            .bonus_picks
            .iter()
            .map(|(t, w, a, b)| format!("t{t}/w{w} {a}->{b}"))
            .collect();
        let wraps: Vec<String> = l.wraps.iter().map(|(t, w)| format!("t{t}/w{w}")).collect();
        out.push(format!(
            "# LEDGER detail: drips w0 {} w1 {}; scales transfers onto w0 {} w1 {}",
            l.drips[0], l.drips[1], l.transfers_onto[0], l.transfers_onto[1]
        ));
        out.push(format!(
            "# LEDGER detail: health bonuses at {}",
            list(picks, 16)
        ));
        out.push(format!("# LEDGER detail: wraps at {}", list(wraps, 16)));
    }
    out
}
