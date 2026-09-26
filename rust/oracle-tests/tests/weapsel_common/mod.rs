//! Step 4½c — the weapon-selection golden helpers, shared by `examples/gen_slice4_5c.rs` (via
//! `#[path]`), `weapsel_golden.rs`, `sim_slice4_5c_continuation_golden.rs` and
//! `weapsel_handoff.rs`: the 16-case corpus (design §6.4), the lean setup sidecar and the
//! scenario writer, the menu scripts, the Rust driver that yields a golden's lines (§6.3), the
//! witness guard (§6.7) and the 12-column settings-path harness (§6.5). One copy, so the
//! generator's corpus and the milestone's non-vacuity checks cannot drift apart.

#![allow(dead_code)] // each includer uses a subset.

use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use scenario::build::{new_match, weapsel_config};
use scenario::settings::{MatchConfig, Settings};
use scenario::settings_toml::settings_from_toml;
use scenario::Scenario;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};
use sim::weapsel::{
    weap_order, PlayerSel, WeaponSelection, WeapselConfig, WeapselPlayer, WEAPON_COUNT,
};
use sim_core::rng::Rand;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// Every case's level (design §6.4).
pub const LEVEL: &str = "Levels/render_stage.lev";
/// The lean sidecar's sim settings (4½c-0's set). Lives 99: a continuation never ends.
pub const LIVES: i32 = 99;
pub const HEALTH: i32 = 100;
pub const LOADING_TIME: i32 = 20;
pub const BLOOD: i32 = 100;
pub const MAX_BONUSES: i32 = 4;
pub const BLOOD_PARTICLE_MAX: i32 = 700;

pub const UP: u32 = 1;
pub const DOWN: u32 = 2;
pub const LEFT: u32 = 4;
pub const RIGHT: u32 = 8;
pub const FIRE: u32 = 16;

/// A menu script: one `[worm0, worm1]` word pair per phase frame, frame 0 first.
#[derive(Clone, Debug, Default)]
pub struct Script(pub Vec<[u32; 2]>);

impl Script {
    pub fn new() -> Script {
        Script(Vec::new())
    }
    /// `n` frames holding `a` on worm 0 and `b` on worm 1.
    pub fn both(mut self, a: u32, b: u32, n: u32) -> Script {
        for _ in 0..n {
            self.0.push([a, b]);
        }
        self
    }
    pub fn p0(self, bits: u32, n: u32) -> Script {
        self.both(bits, 0, n)
    }
    pub fn p1(self, bits: u32, n: u32) -> Script {
        self.both(0, bits, n)
    }
    pub fn idle(self, n: u32) -> Script {
        self.both(0, 0, n)
    }
    /// A clean tap: held one frame, released the next.
    pub fn tap0(self, bits: u32) -> Script {
        self.p0(bits, 1).idle(1)
    }
    pub fn tap1(self, bits: u32) -> Script {
        self.p1(bits, 1).idle(1)
    }
    pub fn tap_both(self, a: u32, b: u32) -> Script {
        self.both(a, b, 1).idle(1)
    }
}

pub struct Case {
    pub name: &'static str,
    pub pins: &'static str,
    pub seed: u32,
    /// Match ticks after the phase: 0, except the two continuation cases.
    pub ticks: u32,
    /// Seed of a continuation's per-tick input stream.
    pub input_seed: u32,
    pub select_bot_weapons: u32,
    pub table: fn(&Objects) -> [u32; WEAPON_COUNT],
    pub players: [WeapselPlayer; 2],
    pub script: fn() -> Script,
}

const fn human(weapons: [u32; 5]) -> WeapselPlayer {
    WeapselPlayer {
        weapons,
        controller: 0,
    }
}
const fn bot(weapons: [u32; 5]) -> WeapselPlayer {
    WeapselPlayer {
        weapons,
        controller: 1,
    }
}
/// `Settings()`'s saved picks (`worm.hpp:92`): five copies of the first weapon by name.
const DEFAULT: [u32; 5] = [1; 5];
const MATCH_TICKS: u32 = 600;

pub fn load_objects() -> Objects {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap()
}

pub fn load_level(rel: &str) -> LevelData {
    assets::level::load(&std::fs::read(format!("{TC_ROOT}/{rel}")).unwrap()).unwrap()
}

/// Index into `objects.weapons` (== the `weap_table` index).
pub fn weapon_index(o: &Objects, name: &str) -> usize {
    o.weapons
        .iter()
        .position(|w| w.name == name)
        .unwrap_or_else(|| panic!("no weapon {name:?}"))
}

/// The weapon a 1-based pick names.
pub fn weapon_name_of_pick(o: &Objects, pick: u32) -> String {
    o.weapons[weap_order(&o.weapons)[pick as usize - 1]]
        .name
        .clone()
}

fn all_enabled(_: &Objects) -> [u32; WEAPON_COUNT] {
    [0; WEAPON_COUNT]
}

/// `names` disabled with their value (1 bonus only, 2 banned); the rest enabled.
fn disabled(o: &Objects, names: &[(&str, u32)]) -> [u32; WEAPON_COUNT] {
    let mut t = [0u32; WEAPON_COUNT];
    for &(n, v) in names {
        t[weapon_index(o, n)] = v;
    }
    t
}

/// Only `names` enabled; the rest alternate bonus-only (1) and banned (2) by weapon index.
fn only(o: &Objects, names: &[&str]) -> [u32; WEAPON_COUNT] {
    let mut t = [0u32; WEAPON_COUNT];
    for (i, v) in t.iter_mut().enumerate() {
        *v = 1 + (i as u32 % 2);
    }
    for n in names {
        t[weapon_index(o, n)] = 0;
    }
    t
}

fn t_twelve(o: &Objects) -> [u32; WEAPON_COUNT] {
    disabled(
        o,
        &[
            ("BAZOOKA", 2),
            ("BIG NUKE", 1),
            ("BLASTER", 2),
            ("CHAINGUN", 1),
            ("DART", 2),
            ("FAN", 1),
            ("LASER", 2),
            ("MINE", 1),
            ("MISSILE", 2),
            ("RIFLE", 1),
            ("SHOTGUN", 2),
            ("ZIMM", 1),
        ],
    )
}
fn t_three(o: &Objects) -> [u32; WEAPON_COUNT] {
    only(o, &["DART", "HANDGUN", "SHOTGUN"])
}
fn t_five(o: &Objects) -> [u32; WEAPON_COUNT] {
    only(o, &["CANNON", "DART", "GRENADE", "HANDGUN", "UZI"])
}
fn t_one(o: &Objects) -> [u32; WEAPON_COUNT] {
    only(o, &["DART"])
}
fn t_keep(o: &Objects) -> [u32; WEAPON_COUNT] {
    disabled(o, &[("BAZOOKA", 2), ("LASER", 2), ("MISSILE", 1)])
}

fn s_humans_default() -> Script {
    Script::new()
        .tap0(UP) // f0: cursor 0 -> 6, wrapping up
        .tap0(DOWN) // f2: 6 -> 0, wrapping down
        .tap0(DOWN) // f4: 0 -> 1 (slot 0)
        .tap0(LEFT) // f6: pick 1 -> 40, wrapping left
        .tap0(RIGHT) // f8: 40 -> 1, wrapping right
        .tap0(RIGHT) // f10: a tap cycles once: 1 -> 2
        .p0(RIGHT, 19) // f12-30: cycles on 12 (the press), 24, 27, 30 (held 12, 15, 18)
        .idle(1) // f31
        .tap0(UP) // f32: 1 -> 0
        .tap0(UP) // f34: 0 -> 6
        .tap0(FIRE) // f36: P0 ready
        .tap1(DOWN) // f38: P1 moves while P0 is ready
        .tap1(UP) // f40: 1 -> 0
        .tap1(UP) // f42: 0 -> 6
        .p1(FIRE, 1) // f44: P1 ready: the phase ends
}
fn s_both_done() -> Script {
    Script::new().tap_both(UP, UP).both(FIRE, FIRE, 1) // f0: both 0 -> 6; f2: done
}
fn s_full_laps() -> Script {
    Script::new()
        .tap_both(DOWN, DOWN) // f0: both to slot 0
        .both(RIGHT, LEFT, 91) // f2-92: 28 cycles each: a full lap of the 28 enabled
        .idle(1) // f93
        .tap_both(UP, UP) // f94: 1 -> 0
        .tap_both(UP, UP) // f96: 0 -> 6
        .both(FIRE, FIRE, 1) // f98: done
}
fn s_few() -> Script {
    Script::new()
        .tap0(FIRE) // f0: P0 RANDOMIZE: five picks from three weapons, duplicates kept
        .tap0(DOWN) // f2: slot 0
        .tap0(RIGHT) // f4: across the disabled run
        .tap0(LEFT) // f6: and back
        .tap_both(UP, UP) // f8: P0 1 -> 0; P1 0 -> 6
        .tap_both(UP, FIRE) // f10: P0 0 -> 6; P1 ready
        .p0(FIRE, 1) // f12: done
}
fn s_five() -> Script {
    Script::new()
        .tap_both(FIRE, FIRE) // f0: both RANDOMIZE: permutations of the five
        .tap_both(UP, UP) // f2: 0 -> 6
        .both(FIRE, FIRE, 1) // f4: done
}
fn s_one() -> Script {
    Script::new()
        .tap0(DOWN) // f0: slot 0
        .tap0(LEFT) // f2: a full lap back to DART
        .tap0(RIGHT) // f4: and the other way
        .tap_both(UP, UP) // f6: P0 1 -> 0; P1 0 -> 6
        .tap_both(UP, FIRE) // f8: P0 0 -> 6; P1 ready
        .p0(FIRE, 1) // f10: done
}
fn s_held() -> Script {
    Script::new()
        .p0(FIRE, 10) // f0-9: a silent re-roll every frame (finding 7)
        .idle(1) // f10
        .tap_both(UP, UP) // f11: 0 -> 6
        .both(FIRE, FIRE, 1) // f13: done
}
fn s_same() -> Script {
    Script::new()
        .tap_both(UP, DOWN) // f0: P0 0 -> 6; P1 0 -> 1
        .tap_both(UP, LEFT | RIGHT) // f2: P0 6 -> 5; P1 Left then Right: net zero, two sounds
        .tap_both(0, UP | DOWN) // f4: P1 Up then Down: net zero, two sounds
        .tap_both(DOWN | FIRE, UP | FIRE) // f6: P0 5 -> 6 -> DONE; P1 1 -> 0 -> RANDOMIZE
        .tap1(UP) // f8: P1 0 -> 6 while P0 is ready
        .p1(FIRE, 1) // f10: done
}
fn s_edge() -> Script {
    Script::new()
        .p0(LEFT, 20) // f0-19: Left held on RANDOMIZE: never read, its counter stays 0
        .p0(LEFT | DOWN, 1) // f20: Down runs after the Left check: 0 -> 1
        .p0(LEFT, 16) // f21-36: cycles on 21, 33, 36 (Rollback would give 21, 24, 27)
        .idle(1) // f37
        .tap0(UP) // f38: 1 -> 0
        .tap_both(UP, UP) // f40: both 0 -> 6
        .both(FIRE, FIRE, 1) // f42: done
}
fn s_p0_done() -> Script {
    Script::new().tap0(UP).p0(FIRE, 1) // player 2 is ready at once; f2: done
}
fn s_bot_pick() -> Script {
    Script::new()
        .tap1(DOWN) // f0: the PICK bot's menu, driven by worm 1's keys (finding 2)
        .tap1(RIGHT) // f2
        .tap_both(UP, UP) // f4: P0 0 -> 6; P1 1 -> 0
        .tap_both(FIRE, UP) // f6: P0 ready; P1 0 -> 6
        .p1(FIRE, 1) // f8: done
}
fn s_none() -> Script {
    Script::new().idle(1) // both ready at construction: done on frame 0 (`weapsel 0 0 0`)
}
fn s_match_humans() -> Script {
    Script::new()
        .tap_both(FIRE, DOWN) // f0: P0 RANDOMIZE; P1 0 -> 1
        .tap_both(DOWN, DOWN) // f2: P0 0 -> 1; P1 1 -> 2
        .tap_both(RIGHT, LEFT) // f4: P0 slot 0 + 1; P1 slot 1: 1 -> 40
        .tap_both(UP, UP) // f6: P0 1 -> 0; P1 2 -> 1
        .tap_both(UP, UP) // f8: P0 0 -> 6; P1 1 -> 0
        .tap_both(FIRE, UP) // f10: P0 ready; P1 0 -> 6
        .p1(FIRE, 1) // f12: done
}
fn s_match_bot() -> Script {
    Script::new().tap0(FIRE).tap0(UP).p0(FIRE, 1) // f0 RANDOMIZE, f2 0 -> 6, f4 done
}

pub const CASES: [Case; 16] = [
    Case { name: "humans_default", pins: "zero constructor draws; cursor wrap both ways; cycle wrap 1<->40; a tap vs Right held (cycles at held 12, 15, 18); P0 DONE first, then P1", seed: 1, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_humans_default },
    Case { name: "unset_picks", pins: "constructor draws only for zero picks; enabled duplicates are kept (finding 1)", seed: 2, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human([0, 5, 0, 40, 0]), human([3; 5])], script: s_both_done },
    Case { name: "disabled_saved_s1", pins: "the loop runs only for disabled saved picks, uniqueness inside it; cycling skips bonus-only and banned weapons over a full lap", seed: 3, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_twelve, players: [human([1, 12, 5, 40, 26]), human([2, 3, 7, 34, 35])], script: s_full_laps },
    Case { name: "disabled_saved_s2", pins: "as disabled_saved_s1, second seed", seed: 4, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_twelve, players: [human([1, 12, 5, 40, 26]), human([2, 3, 7, 34, 35])], script: s_full_laps },
    Case { name: "few_enabled", pins: "3 enabled, enough = false: duplicates in the constructor loop and in RANDOMIZE", seed: 5, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_three, players: [human(DEFAULT), human(DEFAULT)], script: s_few },
    Case { name: "five_enabled", pins: "the >= 5 boundary: loops and RANDOMIZE yield permutations (long tails)", seed: 6, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_five, players: [human(DEFAULT), human(DEFAULT)], script: s_five },
    Case { name: "one_enabled", pins: "1 enabled: ~40 draws per slot; a cycle is a full lap back to the same weapon", seed: 7, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_one, players: [human(DEFAULT), human(DEFAULT)], script: s_one },
    Case { name: "randomize_held", pins: "Fire held 10 frames on RANDOMIZE: a silent re-roll every frame (finding 7)", seed: 8, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_held },
    Case { name: "same_frame", pins: "in-frame order (finding 6): Down+Fire on slot 5 readies; Left+Right and Up+Down cancel with two sounds; Up+Fire randomizes", seed: 9, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_same },
    Case { name: "repeat_edge", pins: "LocalController repeat: Left held unread on RANDOMIZE, then cycles on 21, 33, 36 (finding 5)", seed: 10, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_edge },
    Case { name: "bot_random", pins: "a RANDOM bot (select_bot_weapons 0) draws all five over its saved picks and is ready at once", seed: 11, ticks: 0, input_seed: 0, select_bot_weapons: 0, table: all_enabled, players: [human(DEFAULT), bot([5, 6, 7, 8, 9])], script: s_p0_done },
    Case { name: "bot_pick", pins: "a PICK bot (select_bot_weapons 1) is not ready and is driven by worm 1's keys (finding 2)", seed: 12, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), bot(DEFAULT)], script: s_bot_pick },
    Case { name: "bot_keep", pins: "a KEEP bot (select_bot_weapons 2) readies at once with its saved picks; its zero and its disabled pick still roll", seed: 13, ticks: 0, input_seed: 0, select_bot_weapons: 2, table: t_keep, players: [human([2, 3, 4, 5, 6]), bot([0, 26, 5, 6, 7])], script: s_p0_done },
    Case { name: "bots_only_7", pins: "select_bot_weapons 7 behaves as KEEP; both bots: done on frame 0", seed: 14, ticks: 0, input_seed: 0, select_bot_weapons: 7, table: all_enabled, players: [bot([0, 2, 3, 4, 5]), bot([9; 5])], script: s_none },
    Case { name: "match_humans", pins: "continuation: RANDOMIZE + cycling + DONE, then 600 fuzz ticks", seed: 15, ticks: MATCH_TICKS, input_seed: 1015, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_match_humans },
    Case { name: "match_bot", pins: "continuation: a RANDOM bot + a human RANDOMIZE (different picks per worm), then 600 fuzz ticks", seed: 16, ticks: MATCH_TICKS, input_seed: 1016, select_bot_weapons: 0, table: all_enabled, players: [human(DEFAULT), bot(DEFAULT)], script: s_match_bot },
];

pub fn case(name: &str) -> &'static Case {
    CASES
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

fn arr(v: &[u32]) -> String {
    let items: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("[ {} ]", items.join(", "))
}

/// The LEAN setup sidecar: the 4½c-0 keys + `controller` and `selectBotWeapons`. Missing keys
/// keep their defaults in both readers (`toml_archive.hpp:177-186`).
pub fn setup_cfg(c: &Case, o: &Objects) -> String {
    let mut out = String::new();
    for (header, p) in [("player1", &c.players[0]), ("player2", &c.players[1])] {
        out.push_str(&format!(
            "[{header}]\ncontroller = {}\nhealth = {HEALTH}\nweapons = {}\n\n",
            p.controller,
            arr(&p.weapons)
        ));
    }
    out.push_str("[settings]\n");
    out.push_str(&format!(
        "blood = {BLOOD}\nbloodParticleMax = {BLOOD_PARTICLE_MAX}\n"
    ));
    out.push_str(&format!(
        "gameMode = 0\nlevelFile = '{LEVEL}'\nlives = {LIVES}\n"
    ));
    out.push_str(&format!(
        "loadChange = true\nloadingTime = {LOADING_TIME}\n"
    ));
    out.push_str(&format!(
        "maxBonuses = {MAX_BONUSES}\nrandomLevel = false\n"
    ));
    out.push_str(&format!("selectBotWeapons = {}\n", c.select_bot_weapons));
    out.push_str("shadow = true\ntimeToLose = 600\nversion = 6\n");
    out.push_str(&format!("weapTable = {}\n", arr(&(c.table)(o))));
    out
}

/// Per tick `Rand(input_seed).next_u32() & 0x7f`, worm 0 then worm 1 (the 4½a-1 fuzz stream).
pub fn inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    (0..ticks)
        .map(|_| [r.next_u32() & 0x7f, r.next_u32() & 0x7f])
        .collect()
}

/// The committed scenario text. `weapsel` lines are sparse, but the last frame is always
/// written (`weapsel 0 0 0` for `bots_only_7`).
pub fn scenario_text(c: &Case, ledger: &str) -> String {
    let frames = (c.script)().0;
    let end = frames.len() - 1;
    let also = if c.ticks > 0 {
        format!(
            " and oracle_dump_sim_physics (golden/sim_slice4_5c_{}.txt)",
            c.name
        )
    } else {
        String::new()
    };
    let mut out = format!(
        "# Step 4½ slice 4½c — weapon-selection case `{name}` (design §6.4): {pins}.\n\
         # Read by oracle_dump_weapsel (golden/weapsel_{name}.txt){also} and by the\n\
         # Rust milestone (weapsel_golden.rs). Written by\n\
         #   cargo run -p oracle-tests --example gen_slice4_5c -- write <golden dir>\n\
         # LEDGER (Rust, driven): {ledger}\n\
         # weapsel <frame> <worm0_7bit> <worm1_7bit>: Up=1 Down=2 Left=4 Right=8 Fire=16; an\n\
         # absent frame is 0; the last line is the frame the phase ends on (design §6.1).\n\
         seed {seed}\nlevel {LEVEL}\nticks {ticks}\nsettings weapsel_{name}_setup.cfg\n",
        name = c.name,
        pins = c.pins,
        seed = c.seed,
        ticks = c.ticks,
    );
    for (f, w) in frames.iter().enumerate() {
        if w[0] != 0 || w[1] != 0 || f == end {
            out.push_str(&format!("weapsel {f} {} {}\n", w[0], w[1]));
        }
    }
    if c.ticks > 0 {
        out.push_str(&format!(
            "# input <tick> <w0> <w1>: per tick Rand({}).next_u32() & 0x7f, worm 0 then worm 1.\n",
            c.input_seed
        ));
        for (t, w) in inputs(c.input_seed, c.ticks).iter().enumerate() {
            if w[0] != 0 || w[1] != 0 {
                out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
            }
        }
    }
    out
}

pub fn read(rel: &str) -> String {
    let path = Path::new(GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

pub fn read_scenario(name: &str) -> Scenario {
    Scenario::parse(&read(&format!("weapsel_{name}_scenario.txt")))
        .unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"))
}

pub fn read_settings(s: &Scenario) -> Settings {
    let rel = s.settings.as_ref().expect("a settings scenario");
    settings_from_toml(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"))
}

/// The non-comment lines of the committed C++ golden `weapsel_<name>.txt`.
pub fn golden_file_lines(name: &str) -> Vec<String> {
    read(&format!("weapsel_{name}.txt"))
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// One phase frame, as a golden `f` line records it.
#[derive(Clone, Debug)]
pub struct Step {
    pub frame: u32,
    pub input: [u32; 2],
    pub before: [PlayerSel; 2],
    pub after: [PlayerSel; 2],
    pub ctl: [u32; 2],
    pub held: [[u16; 7]; 2],
    pub sounds: Vec<i32>,
    pub draws: u64,
    pub last: u32,
    pub next: u32,
    pub done: bool,
}

/// A whole case driven through the REAL Rust path.
#[derive(Clone, Debug)]
pub struct Run {
    pub name: String,
    pub cfg: WeapselConfig,
    pub order: Vec<usize>,
    pub enabled: i32,
    pub init: [PlayerSel; 2],
    pub init_draws: u64,
    pub init_last: u32,
    pub init_next: u32,
    pub steps: Vec<Step>,
    /// After `finalize`: per worm the five `(weapon id, ammo)`.
    pub loadout: [[(i32, i32); 5]; 2],
    pub current_weapon: [i32; 2],
    pub final_ctl: [u32; 2],
    pub final_last: u32,
    pub final_next: u32,
}

impl Run {
    pub fn summary(&self) -> String {
        let draws: u64 = self.steps.iter().map(|s| s.draws).sum();
        format!(
            "init draws {}, {} frames, {} draws in frames, final rng {:08x}",
            self.init_draws,
            self.steps.len(),
            draws,
            self.final_last
        )
    }
}

fn next_of(st: &SimState) -> u32 {
    st.rand.clone().next_u32()
}

fn players(ws: &WeaponSelection) -> [PlayerSel; 2] {
    [*ws.player(0), *ws.player(1)]
}

/// Drive `scenario`'s phase through the REAL Rust path — `new_match`, `weapsel_config`,
/// `WeaponSelection::new` / `process_frame` / `finalize` — recording everything a golden line
/// holds. Enforces the end-frame invariant (design §6.1). Returns the run and the state after
/// `finalize` (before `enter_game`).
pub fn drive(name: &str, scenario: &Scenario, settings: &Settings) -> (Run, SimState) {
    let level = load_level(&scenario.level);
    let cfg = MatchConfig {
        settings: settings.clone(),
        seed: scenario.seed,
    };
    let mut st = new_match(Path::new(TC_ROOT), &cfg, &level)
        .unwrap_or_else(|e| panic!("{name}: builds: {e}"))
        .state;
    let wcfg = weapsel_config(settings);
    let d0 = st.rand.draws();
    let mut ws = WeaponSelection::new(&mut st, &wcfg).unwrap_or_else(|e| panic!("{name}: {e}"));
    let enabled = ws.enabled_weaps();
    let init = players(&ws);
    let (init_draws, init_last, init_next) = (st.rand.draws() - d0, st.rand.last(), next_of(&st));
    let end = scenario
        .weapsel_end()
        .unwrap_or_else(|| panic!("{name}: no weapsel line"));
    let mut steps = Vec::new();
    for f in 0..=end {
        let input = [0, 1].map(|i| ControlState::unpack(scenario.weapsel_input(f, i)).pack());
        let before = players(&ws);
        let d = st.rand.draws();
        let done = ws.process_frame(
            &mut st,
            &[
                ControlState::unpack(input[0]),
                ControlState::unpack(input[1]),
            ],
        );
        steps.push(Step {
            frame: f,
            input,
            before,
            after: players(&ws),
            ctl: [
                st.worms[0].control_states.pack(),
                st.worms[1].control_states.pack(),
            ],
            held: [ws.held(0), ws.held(1)],
            sounds: ws.menu_sounds().to_vec(),
            draws: st.rand.draws() - d,
            last: st.rand.last(),
            next: next_of(&st),
            done,
        });
        assert_eq!(
            done,
            f == end,
            "{name}: the phase must end exactly on the last weapsel frame {end} (frame {f})"
        );
    }
    let order = weap_order(&st.weapons);
    let d = st.rand.draws();
    ws.finalize(&mut st);
    assert_eq!(st.rand.draws(), d, "{name}: finalize draws nothing");
    let loadout = [0, 1].map(|i| {
        st.worms[i]
            .weapons
            .map(|w| (w.ty.expect("finalize loads every slot"), w.ammo))
    });
    let run = Run {
        name: name.to_string(),
        cfg: wcfg,
        order,
        enabled,
        init,
        init_draws,
        init_last,
        init_next,
        steps,
        loadout,
        current_weapon: [st.worms[0].current_weapon, st.worms[1].current_weapon],
        final_ctl: [
            st.worms[0].control_states.pack(),
            st.worms[1].control_states.pack(),
        ],
        final_last: st.rand.last(),
        final_next: next_of(&st),
    };
    (run, st)
}

fn pfield(p: &PlayerSel) -> String {
    let picks: Vec<String> = p.picks.iter().map(u32::to_string).collect();
    format!("{}:{}:{}", picks.join(","), p.cursor, p.ready as u8)
}

fn join<T: ToString>(v: &[T]) -> String {
    v.iter().map(T::to_string).collect::<Vec<_>>().join(",")
}

fn lfield(l: &[(i32, i32); 5], current_weapon: i32) -> String {
    let slots: Vec<String> = l.iter().map(|(w, a)| format!("{w}:{a}")).collect();
    format!("{}:{current_weapon}", slots.join(","))
}

/// The golden lines (design §6.3 + the T5 `final` refinement) a run implies — the exact text
/// `oracle_dump_weapsel` prints.
pub fn golden_lines(r: &Run) -> Vec<String> {
    let mut out = vec![format!(
        "init {} {} {} {} {:08x} {:08x}",
        r.enabled,
        pfield(&r.init[0]),
        pfield(&r.init[1]),
        r.init_draws,
        r.init_last,
        r.init_next
    )];
    for s in &r.steps {
        let sounds = if s.sounds.is_empty() {
            "-".to_string()
        } else {
            join(&s.sounds)
        };
        out.push(format!(
            "f {} {} {} {} {} {:02x} {:02x} {} {} {} {} {:08x} {:08x} {}",
            s.frame,
            s.input[0],
            s.input[1],
            pfield(&s.after[0]),
            pfield(&s.after[1]),
            s.ctl[0],
            s.ctl[1],
            join(&s.held[0]),
            join(&s.held[1]),
            sounds,
            s.draws,
            s.last,
            s.next,
            s.done as u8
        ));
    }
    out.push(format!(
        "final {} {} {:02x} {:02x} {:08x} {:08x}",
        lfield(&r.loadout[0], r.current_weapon[0]),
        lfield(&r.loadout[1], r.current_weapon[1]),
        r.final_ctl[0],
        r.final_ctl[1],
        r.final_last,
        r.final_next
    ));
    out
}

fn weapon(r: &Run, pick: u32) -> usize {
    r.order[pick as usize - 1]
}

fn has_dup(r: &Run, picks: &[u32; 5]) -> bool {
    let mut seen = [false; WEAPON_COUNT];
    picks
        .iter()
        .any(|&p| std::mem::replace(&mut seen[weapon(r, p)], true))
}

/// Player `i` ran RANDOMIZE this frame: not ready, the (moved) cursor on RANDOMIZE, Fire set.
fn randomized(s: &Step, i: usize) -> bool {
    !s.before[i].ready && s.after[i].cursor == 0 && s.ctl[i] & FIRE != 0
}

/// The single cycling direction player `i` pressed (not both, no Fire), if any.
fn dir(s: &Step, i: usize) -> Option<u32> {
    let w = s.input[i];
    match (w & LEFT != 0, w & RIGHT != 0, w & FIRE != 0) {
        (true, false, false) => Some(LEFT),
        (false, true, false) => Some(RIGHT),
        _ => None,
    }
}

/// `(before, after)` of the slot player `i`'s frame-start cursor names, if its pick changed.
fn slot_change(s: &Step, i: usize) -> Option<(u32, u32)> {
    let c = s.before[i].cursor;
    if s.before[i].ready || !(1..=5).contains(&c) {
        return None;
    }
    let k = c as usize - 1;
    let (a, b) = (s.before[i].picks[k], s.after[i].picks[k]);
    (a != b).then_some((a, b))
}

/// The reach witnesses (design §6.7), derived from the driven Rust state over the whole corpus.
pub fn witnesses(runs: &[Run]) -> Vec<(&'static str, bool)> {
    let steps = || {
        runs.iter()
            .flat_map(|r| r.steps.iter().map(move |s| (r, s)))
    };
    let ctor_loop = runs.iter().any(|r| {
        // Every forced slot (a saved disabled pick) draws >= 1 in the loop; every optional slot
        // (zero or RANDOM) draws 1, plus >= 1 if disabled. More than forced + 2 * optional
        // draws means some loop iterated at least twice.
        let (mut forced, mut optional) = (0u64, 0u64);
        for p in &r.cfg.players {
            let random = p.controller != 0 && r.cfg.select_bot_weapons == 0;
            for &pick in &p.weapons {
                if pick == 0 || random {
                    optional += 1;
                } else if r.cfg.weap_table[weapon(r, pick)] != 0 {
                    forced += 1;
                }
            }
        }
        r.init_draws > forced + 2 * optional
    });
    let cycled = |want: fn(u32, u32, u32) -> bool| {
        steps().any(|(_, s)| {
            (0..2).any(|i| match (dir(s, i), slot_change(s, i)) {
                (Some(d), Some((a, b))) => want(d, a, b),
                _ => false,
            })
        })
    };
    let naive = |d: u32, a: u32| -> u32 {
        if d == LEFT {
            if a == 1 {
                WEAPON_COUNT as u32
            } else {
                a - 1
            }
        } else if a == WEAPON_COUNT as u32 {
            1
        } else {
            a + 1
        }
    };
    let across_disabled = steps().any(|(_, s)| {
        (0..2).any(|i| match (dir(s, i), slot_change(s, i)) {
            (Some(d), Some((a, b))) => b != naive(d, a),
            _ => false,
        })
    });
    let wrap_left = cycled(|d, a, b| d == LEFT && b > a);
    let wrap_right = cycled(|d, a, b| d == RIGHT && b < a);
    let cursor_wrap = |pressed: u32, other: u32, from: u8, to: u8| {
        steps().any(|(_, s)| {
            (0..2).any(|i| {
                !s.before[i].ready
                    && s.input[i] & pressed != 0
                    && s.input[i] & other == 0
                    && s.before[i].cursor == from
                    && s.after[i].cursor == to
            })
        })
    };
    let repeat_at = |held: u16| {
        steps().any(|(_, s)| {
            (0..2).any(|i| {
                dir(s, i).is_some()
                    && slot_change(s, i).is_some()
                    && (s.held[i][2] == held || s.held[i][3] == held)
            })
        })
    };
    let edge = runs.iter().filter(|r| r.name == "repeat_edge").any(|r| {
        let frames: Vec<u32> = r
            .steps
            .iter()
            .filter(|s| slot_change(s, 0).is_some())
            .map(|s| s.frame)
            .collect();
        frames == [21, 33, 36]
    });
    let sbw = |f: fn(u32) -> bool| runs.iter().any(|r| f(r.cfg.select_bot_weapons));
    vec![
        ("a constructor loop with >= 2 iterations", ctor_loop),
        (
            "a constructor-kept duplicate (>= 5 enabled)",
            runs.iter()
                .any(|r| r.enabled >= 5 && r.init.iter().any(|p| has_dup(r, &p.picks))),
        ),
        (
            "a RANDOMIZE that rejected a duplicate (40 enabled, one player, > 5 draws)",
            steps().any(|(r, s)| {
                r.enabled == 40 && (0..2).filter(|&i| randomized(s, i)).count() == 1 && s.draws > 5
            }),
        ),
        (
            "a RANDOMIZE that kept a duplicate",
            steps().any(|(r, s)| (0..2).any(|i| randomized(s, i) && has_dup(r, &s.after[i].picks))),
        ),
        ("a cycle across a disabled weapon", across_disabled),
        ("a cycle wrapping left (up through 1)", wrap_left),
        ("a cycle wrapping right (down through 40)", wrap_right),
        ("a cursor wrap up (0 -> 6)", cursor_wrap(UP, DOWN, 0, 6)),
        ("a cursor wrap down (6 -> 0)", cursor_wrap(DOWN, UP, 6, 0)),
        ("a repeat at held-frame 12", repeat_at(12)),
        ("a repeat at held-frame 15", repeat_at(15)),
        ("the repeat_edge timing 21, 33, 36", edge),
        ("select_bot_weapons 0 (RANDOM)", sbw(|v| v == 0)),
        ("select_bot_weapons 1 (PICK)", sbw(|v| v == 1)),
        ("select_bot_weapons 2 (KEEP)", sbw(|v| v == 2)),
        ("select_bot_weapons >= 3 (as KEEP)", sbw(|v| v >= 3)),
        (
            "one player ready while the other still moves",
            steps().any(|(_, s)| {
                (0..2).any(|a| {
                    let b = 1 - a;
                    s.before[a].ready && !s.before[b].ready && s.after[b] != s.before[b]
                })
            }),
        ),
        ("done on frame 0", runs.iter().any(|r| r.steps.len() == 1)),
        (
            "a RANDOMIZE held over consecutive frames",
            runs.iter().any(|r| {
                r.steps
                    .windows(2)
                    .any(|w| (0..2).any(|i| randomized(&w[0], i) && randomized(&w[1], i)))
            }),
        ),
    ]
}

/// One row of a 12-column settings-path golden: the 10 hashes (master, rng, level, worm0,
/// worm1, bob, bon, sob, nob, wob) + `IsGameOver`.
pub struct Row {
    pub tick: u32,
    pub hashes: [u32; 10],
    pub game_over: u32,
}

/// Parse a 12-column settings-path golden; row `k` must carry tick `k`
/// (`sim_slice4_5a_settings_golden.rs`'s reader).
pub fn parse_golden12(text: &str) -> Vec<Row> {
    let rows: Vec<Row> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(cols.len(), 12, "settings-path golden lines have 12 columns");
            let mut hashes = [0u32; 10];
            for (i, c) in cols[1..11].iter().enumerate() {
                hashes[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            Row {
                tick: cols[0].parse().expect("tick"),
                hashes,
                game_over: cols[11].parse().expect("game-over column"),
            }
        })
        .collect();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(r.tick, k as u32, "golden row {k} carries tick {}", r.tick);
    }
    rows
}

/// Components first, master last, so a divergence localises; then column 12.
pub fn check12(state: &SimState, row: &Row) {
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
    let names = [
        "master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob",
    ];
    for i in (1..10).chain(0..1) {
        assert_eq!(
            got[i], row.hashes[i],
            "tick {}: {}: got {:08x} want {:08x}",
            row.tick, names[i], got[i], row.hashes[i]
        );
    }
    assert_eq!(
        is_game_over(state) as u32,
        row.game_over,
        "tick {}: IsGameOver",
        row.tick
    );
}
