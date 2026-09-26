//! Step 4½e-1 T7 — G3: setup-sidecar writer, seed scanner and scenario writer for the four
//! GENERATED-LEVEL sim goldens (plan T7; design §6.5). A dev tool, not a test; not run in CI.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5e1_sim -- cfg  <case> <out_setup.cfg>
//!   cargo run -p oracle-tests --example gen_slice4_5e1_sim -- scan <case> <input_seed> <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5e1_sim -- gen  <case> <game_seed> <input_seed> <out_scenario.txt>
//!   cargo run -p oracle-tests --example gen_slice4_5e1_sim -- dump <case> <game_seed> <input_seed> <ticks> <out.txt>
//!
//! Cases: small, odd, tall, banned — the first sim gate on levels that are not 504x350. The
//! level is `sim::levelgen::generate_from_settings(&assets, &params, None, &mut rand)` with
//! `rand = Rand::new(); rand.seed(level_seed)` (the C++ dumper's `generate <level_seed>`: the
//! REAL `Level::GenerateFromSettings` over its own `Rand`), then `scenario::build::build_match`.
//! It does not go through `ui`. Inputs: per tick `Rand(input_seed).next_u32() & 0x7f` for worm
//! 0 then worm 1, applied on the pass advancing t -> t+1 (4½a's shape).
//!
//! Run it in a DEBUG build: `scan` skips (and reports, with the tick and the panicking line) a
//! seed that panics. On these levels a panic marks C++ undefined behaviour, so no golden may
//! contain one:
//!
//! * **Spawn reads past the level.** `Worm::BeginRespawn` draws candidates from the TC's fixed
//!   `WormSpawnRect` (5,5)+(494x340) whatever the level size (`worm.cpp:724-739`). Its drop-down
//!   reads `Mat(x, y + 4)` and `CheckRespawnPosition` (`game.cpp:611-649`) clamps its box's MAX
//!   corner to the level but walks it with `!=` bounds. A candidate below the level
//!   (`y - 4 > height - 1`) starts the row loop past the last row and never meets `max_y`; one
//!   right of it (`x - 3 > width - 1`) reads on through the next rows (`materials[x + y * width]`
//!   is a flat index) until it meets a Rock pixel. C++ `Level::Mat` indexes `std::vector`
//!   unchecked; Rust indexes the same flat `material_id` and panics exactly where C++ leaves the
//!   vector. (C++ at 96x64 segfaults at tick 150; see `CASES`.)
//! * **`cossin_table[128]`** (Batch 3's caution): a laser-sight weapon aimed at `Itof(128)` reads
//!   one past the table in C++. `scan` refuses any seed whose driven state ever holds an
//!   `aiming_angle` outside `0..128` in whole units (`angle_ok`), whatever the weapon.
//!
//! The C++ side of every committed golden is also re-run under `_GLIBCXX_ASSERTIONS` + ASan
//! (`gen_sim_slice4_5e_golden.sh`, `CHECKED_DUMPER`), which aborts on any such read.

use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};
use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use scenario::build::build_match;
use scenario::settings::{
    MatchConfig, Settings, GM_GAME_OF_TAG, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE,
};
use scenario::settings_toml::{settings_from_toml, settings_to_toml};
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams};
use sim::state::{ControlState, SimState};
use sim_core::fixed::ftoi;
use sim_core::rng::Rand;

thread_local! {
    /// Where the last caught panic happened (`scan`'s report).
    static LAST_PANIC: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// The tick `drive` is processing (`scan`'s panic report).
    static TICK: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// Rows recorded after a game-over tick (>= the 180-frame C++ post-mortem).
const POST_MORTEM_MARGIN: u32 = 200;

/// One G3 case: the level, the settings that differ from `Settings::default()`, the horizon.
struct Case {
    name: &'static str,
    width: i32,
    height: i32,
    /// The `generate <level_seed>` of the scenario (fixed per case).
    level_seed: u32,
    game_mode: u32,
    lives: i32,
    loading_time: i32,
    blood: i32,
    shadow: bool,
    time_to_lose: i32,
    max_bonuses: i32,
    /// Weapons (by name) whose `weap_table` entry is 2 (banned); every other entry stays 0.
    /// `None` = none banned; `Some(keep)` = all but `keep` banned.
    keep_only: Option<[&'static str; 4]>,
    p1: [&'static str; 5],
    p2: [&'static str; 5],
    /// Ticks recorded when the match never ends (a game over shortens it to over + margin,
    /// `tall` REQUIRES one and scans up to this horizon).
    ticks: u32,
}

const CASES: [Case; 4] = [
    // The plan's 96x64 cannot be a golden: C++ segfaults at the first spawn (tick 150) on it.
    // The first spawn of a worm rejects every candidate with `y <= 160` (`kDeltaX = old_x = 0`,
    // `game.cpp:614-624`), and every other candidate row lies below a 64-row level, so the spawn
    // box reads past `materials[]` (the module doc). A level admits spawns without such reads
    // only when every candidate row fits, `344 - 4 <= height - 1`; and a level narrower than
    // 161 px separates the two worms only vertically (the enemy test), which needs that height
    // too. 96x344 keeps the width (smaller than a 158-px viewport: the negative `max_x` clamp
    // of `DoRespawning`/the viewport, design §10) at the least such height, a multiple of 8.
    Case {
        name: "small",
        width: 96,
        height: 344,
        level_seed: 9601,
        game_mode: GM_KILL_EM_ALL,
        lives: 3,
        loading_time: 100,
        blood: 100,
        shadow: true,
        time_to_lose: 600,
        max_bonuses: 4,
        keep_only: None,
        p1: ["BAZOOKA", "SHOTGUN", "GRENADE", "CHAINGUN", "MINIGUN"],
        p2: ["DART", "UZI", "FAN", "CLUSTER BOMB", "BLASTER"],
        ticks: 1500,
    },
    Case {
        name: "odd",
        width: 333,
        height: 211,
        level_seed: 33321,
        game_mode: GM_SCALES_OF_JUSTICE,
        lives: 5,
        loading_time: 37,
        blood: 300,
        shadow: true,
        time_to_lose: 600,
        max_bonuses: 4,
        keep_only: None,
        p1: ["SHOTGUN", "CHAINGUN", "BAZOOKA", "MINIGUN", "GRENADE"],
        p2: ["UZI", "SUPER SHOTGUN", "LARPA", "CANNON", "FLAMER"],
        ticks: 1500,
    },
    Case {
        name: "tall",
        width: 160,
        height: 1000,
        level_seed: 1601000,
        game_mode: GM_GAME_OF_TAG,
        lives: 15,
        loading_time: 100,
        blood: 100,
        shadow: true,
        time_to_lose: 60,
        max_bonuses: 4,
        keep_only: None,
        p1: ["BAZOOKA", "CHAINGUN", "GRENADE", "SHOTGUN", "MINIGUN"],
        p2: ["UZI", "DART", "CANNON", "FAN", "SUPER SHOTGUN"],
        ticks: 12000,
    },
    Case {
        name: "banned",
        width: 1024,
        height: 256,
        level_seed: 1024256,
        game_mode: GM_KILL_EM_ALL,
        lives: 15,
        loading_time: 100,
        blood: 100,
        shadow: true,
        time_to_lose: 600,
        max_bonuses: 20,
        keep_only: Some(["SHOTGUN", "BAZOOKA", "GRENADE", "UZI"]),
        p1: ["CHAINGUN", "DART", "MINIGUN", "FAN", "CANNON"],
        p2: [
            "LARPA",
            "BLASTER",
            "FLAMER",
            "CLUSTER BOMB",
            "SUPER SHOTGUN",
        ],
        ticks: 1500,
    },
];

fn case(name: &str) -> &'static Case {
    CASES
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("unknown case {name}"))
}

fn load_tc() -> TcConfig {
    TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap()
}

fn load_objects(tc: &TcConfig) -> Objects {
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap()
}

/// Index into `objects.weapons` (== the `weap_table` index, tc.cfg `[types] weapons`).
fn weapon_index(o: &Objects, name: &str) -> usize {
    o.weapons
        .iter()
        .position(|w| w.name == name)
        .unwrap_or_else(|| panic!("no weapon {name:?}"))
}

/// The 1-based `weap_order` index a `WormSettings.weapons` entry stores.
fn menu_index(o: &Objects, name: &str) -> u32 {
    let order = sim::weapsel::weap_order(&o.weapons);
    order
        .iter()
        .position(|&i| o.weapons[i as usize].name == name)
        .unwrap_or_else(|| panic!("no weapon {name:?} in weap_order")) as u32
        + 1
}

/// The case's settings: `Settings::default()` (a random level) with the case's fields.
fn settings(c: &Case, o: &Objects) -> Settings {
    let mut s = Settings::default();
    s.random_level = true;
    s.level_file = String::new();
    s.random_map_width = c.width;
    s.random_map_height = c.height;
    s.game_mode = c.game_mode;
    s.lives = c.lives;
    s.loading_time = c.loading_time;
    s.blood = c.blood;
    s.shadow = c.shadow;
    s.time_to_lose = c.time_to_lose;
    s.max_bonuses = c.max_bonuses;
    if let Some(keep) = c.keep_only {
        s.weap_table = [2; 40];
        for n in keep {
            s.weap_table[weapon_index(o, n)] = 0;
        }
    }
    s.worm_settings[0].weapons = c.p1.map(|n| menu_index(o, n));
    s.worm_settings[1].weapons = c.p2.map(|n| menu_index(o, n));
    s
}

/// The setup sidecar: C++ `Settings::ToToml` bytes (`settings_to_toml`, byte-identical to it).
fn setup_cfg(c: &Case, o: &Objects) -> String {
    settings_to_toml(&settings(c, o))
}

/// `GenerateFromSettings` with a `Rand` seeded `level_seed` (the dumper's `generate`).
fn generate(s: &Settings, level_seed: u32) -> LevelData {
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

fn inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    (0..ticks)
        .map(|_| [r.next_u32() & 0x7f, r.next_u32() & 0x7f])
        .collect()
}

/// A pool's live slots with their whole-pixel positions.
fn live<T>(pool: &sim::pool::Pool<T>, pos: impl Fn(&T) -> (i32, i32)) -> Vec<Option<(i32, i32)>> {
    (0..pool.capacity())
        .map(|i| pool.get(i).map(&pos))
        .collect()
}

/// Everything the witnesses read, from the genuinely driven Rust state.
#[derive(Default, Debug)]
struct Ledger {
    game_over_tick: Option<u32>,
    /// `is_game_over` stayed true on every tick after it first fired.
    game_over_held: bool,
    deaths: u32,
    /// Invisible -> visible transitions (the two first spawns included).
    spawns: u32,
    /// Spawns of a worm that had died before (a real respawn).
    respawns: u32,
    death_ticks: Vec<(u32, usize)>,
    respawn_ticks: Vec<(u32, usize)>,
    /// `(tick, pool, x, y)`: an object whose slot was live at t-1 and free at t, while its
    /// t-1 position lay outside `[0, width) x [0, height)`.
    freed_outside: Vec<(u32, &'static str, i32, i32)>,
    reload_started: bool,
    peak_bobjects: usize,
    /// `(tick, from, to)`: on one tick one worm's `lives * health_setting + health` fell and
    /// the other's rose (the Scales transfer, `game.cpp:568-592`).
    transfers: Vec<(u32, usize, usize)>,
    /// `(tick, weapon name)`: a slot of the bonus pool newly held a WEAPON bonus (`frame == 0`).
    weapon_bonus_ticks: Vec<(u32, String)>,
    health_bonus_ticks: Vec<u32>,
    /// Every worm's whole-unit `aiming_angle` stayed in `0..128` (see the module doc).
    angle_ok: bool,
    /// Rows the golden records: `ticks`, or the game-over tick + the margin.
    rows: u32,
}

fn total(s: &SimState, i: usize) -> i64 {
    i64::from(s.worms[i].lives) * i64::from(s.settings_health) + i64::from(s.worms[i].health)
}

fn angles_ok(s: &SimState) -> bool {
    s.worms
        .iter()
        .all(|w| (0..128).contains(&ftoi(w.aiming_angle)))
}

fn drive(
    c: &Case,
    cfg: &MatchConfig,
    level: &LevelData,
    ins: &[[u32; 2]],
    require_over: bool,
) -> Ledger {
    let mut st = build_match(Path::new(TC_ROOT), cfg, level)
        .expect("case config builds")
        .state;
    let (w, h) = (level.width, level.height);
    let outside = |p: (i32, i32)| p.0 < 0 || p.1 < 0 || p.0 >= w || p.1 >= h;
    let mut l = Ledger {
        game_over_held: true,
        angle_ok: angles_ok(&st),
        rows: c.ticks,
        ..Ledger::default()
    };
    let mut died = [false; 2];
    let mut prev_vis: Vec<bool> = st.worms.iter().map(|x| x.visible).collect();
    let mut prev_tot = [total(&st, 0), total(&st, 1)];
    let mut prev_wob = live(&st.wobjects, |o| (ftoi(o.pos.x), ftoi(o.pos.y)));
    let mut prev_nob = live(&st.nobjects, |o| (ftoi(o.pos.x), ftoi(o.pos.y)));
    let bonus_slots = |st: &SimState| -> Vec<Option<(i32, i32)>> {
        (0..st.bonuses.capacity())
            .map(|i| st.bonuses.get(i).map(|b| (b.frame, b.weapon)))
            .collect()
    };
    let mut prev_bon = bonus_slots(&st);
    for (t, inp) in ins.iter().enumerate() {
        TICK.with(|c| c.set(t as u32 + 1));
        st.process_frame(&[ControlState::unpack(inp[0]), ControlState::unpack(inp[1])]);
        let k = t as u32 + 1;
        l.angle_ok &= angles_ok(&st);
        for i in 0..2 {
            let vis = st.worms[i].visible;
            if prev_vis[i] && !vis {
                l.deaths += 1;
                l.death_ticks.push((k, i));
                died[i] = true;
            }
            if !prev_vis[i] && vis {
                l.spawns += 1;
                if died[i] {
                    l.respawns += 1;
                    l.respawn_ticks.push((k, i));
                }
            }
            prev_vis[i] = vis;
            l.reload_started |= st.worms[i].weapons.iter().any(|ww| ww.loading_left > 0);
        }
        let tot = [total(&st, 0), total(&st, 1)];
        for i in 0..2 {
            if tot[i] < prev_tot[i] && tot[1 - i] > prev_tot[1 - i] {
                l.transfers.push((k, i, 1 - i));
            }
        }
        prev_tot = tot;
        let wob = live(&st.wobjects, |o| (ftoi(o.pos.x), ftoi(o.pos.y)));
        let nob = live(&st.nobjects, |o| (ftoi(o.pos.x), ftoi(o.pos.y)));
        for (pool, before, after) in [("wobject", &prev_wob, &wob), ("nobject", &prev_nob, &nob)] {
            for (a, b) in before.iter().zip(after.iter()) {
                if let (Some(p), None) = (a, b) {
                    if outside(*p) {
                        l.freed_outside.push((k, pool, p.0, p.1));
                    }
                }
            }
        }
        prev_wob = wob;
        prev_nob = nob;
        let bon = bonus_slots(&st);
        for (a, b) in prev_bon.iter().zip(bon.iter()) {
            if let (None, Some((f, weapon))) = (a, b) {
                if *f == 0 {
                    let name = st.weapons[*weapon as usize].name.clone();
                    l.weapon_bonus_ticks.push((k, name));
                } else {
                    l.health_bonus_ticks.push(k);
                }
            }
        }
        prev_bon = bon;
        l.peak_bobjects = l.peak_bobjects.max(st.bobjects.len());
        if l.game_over_tick.is_none() {
            if is_game_over(&st) {
                l.game_over_tick = Some(k);
                l.rows = k + POST_MORTEM_MARGIN;
            }
        } else {
            l.game_over_held &= is_game_over(&st);
        }
        if k >= l.rows {
            break;
        }
        if !require_over && k >= c.ticks {
            break;
        }
    }
    l
}

/// The case's witnesses (plan T7 Step 1's table), plus the no-UB guard and a held game over.
fn ok(c: &Case, l: &Ledger) -> bool {
    let over_ok = l.game_over_held
        && match l.game_over_tick {
            None => c.name != "tall",
            Some(g) => g + POST_MORTEM_MARGIN <= c.ticks,
        };
    let base = l.angle_ok && over_ok && l.spawns >= 2;
    base && match c.name {
        "small" => l.deaths >= 1 && l.respawns >= 1 && !l.freed_outside.is_empty(),
        "odd" => l.reload_started && l.peak_bobjects >= 50 && !l.transfers.is_empty(),
        "tall" => l.game_over_tick.is_some(),
        "banned" => l.weapon_bonus_ticks.len() >= 3,
        _ => unreachable!(),
    }
}

fn summary(l: &Ledger) -> String {
    format!(
        "rows={} over={:?} held={} spawns={} deaths={} respawns={} freed_outside={} reload={} \
         peak_bob={} transfers={} weapon_bonuses={} health_bonuses={} angle_ok={}",
        l.rows,
        l.game_over_tick,
        l.game_over_held,
        l.spawns,
        l.deaths,
        l.respawns,
        l.freed_outside.len(),
        l.reload_started,
        l.peak_bobjects,
        l.transfers.len(),
        l.weapon_bonus_ticks.len(),
        l.health_bonus_ticks.len(),
        l.angle_ok,
    )
}

struct World {
    cfg: MatchConfig,
    level: LevelData,
}

fn world(c: &Case, game_seed: u32) -> World {
    let tc = load_tc();
    let objects = load_objects(&tc);
    let settings = settings_from_toml(&setup_cfg(c, &objects)).expect("sidecar parses");
    let level = generate(&settings, c.level_seed);
    assert_eq!((level.width, level.height), (c.width, c.height));
    World {
        cfg: MatchConfig {
            settings,
            seed: game_seed,
        },
        level,
    }
}

fn run(c: &Case, w: &World, input_seed: u32) -> Option<Ledger> {
    let ins = inputs(input_seed, c.ticks);
    catch_unwind(AssertUnwindSafe(|| {
        drive(c, &w.cfg, &w.level, &ins, c.name == "tall")
    }))
    .ok()
}

fn list<T: std::fmt::Display>(items: &[T], max: usize) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    let head: Vec<String> = items.iter().take(max).map(|t| t.to_string()).collect();
    let tail = if items.len() > max { ", ..." } else { "" };
    format!("{}{tail}", head.join(", "))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| {
        args.get(i)
            .unwrap_or_else(|| panic!("missing argument {i}"))
            .as_str()
    };
    let num = |i: usize| {
        arg(i)
            .parse::<u32>()
            .unwrap_or_else(|e| panic!("argument {i}: {e}"))
    };
    match arg(0) {
        "cfg" => {
            let tc = load_tc();
            let text = setup_cfg(case(arg(1)), &load_objects(&tc));
            std::fs::write(arg(2), text).expect("write setup");
            println!("wrote {}", arg(2));
        }
        "dump" => {
            // The Rust side of a golden in the dumper's 12-column layout (a localisation aid:
            // it regenerates the setup in memory; the gate test reads the committed files).
            let c = case(arg(1));
            let (game_seed, input_seed, ticks) = (num(2), num(3), num(4));
            let w = world(c, game_seed);
            let mut st = build_match(Path::new(TC_ROOT), &w.cfg, &w.level)
                .expect("case config builds")
                .state;
            let mut out = String::new();
            let mut row = |t: u32, st: &SimState| {
                let h = hash_components(st);
                out.push_str(&format!(
                    "{t} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {}\n",
                    hash_game_state(st),
                    h.rng,
                    h.level,
                    h.worms[0],
                    h.worms[1],
                    h.bobjects,
                    h.bonuses,
                    h.sobjects,
                    h.nobjects,
                    h.wobjects,
                    u32::from(is_game_over(st)),
                ));
            };
            row(0, &st);
            for (t, i) in inputs(input_seed, ticks).iter().enumerate() {
                st.process_frame(&[ControlState::unpack(i[0]), ControlState::unpack(i[1])]);
                row(t as u32 + 1, &st);
            }
            std::fs::write(arg(5), out).expect("write dump");
            println!("wrote {}", arg(5));
        }
        "scan" => {
            let c = case(arg(1));
            let input_seed = num(2);
            // A panic is reported below, with where it happened (the first sim frame).
            set_hook(Box::new(|info| {
                let at = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_default();
                LAST_PANIC.with(|p| *p.borrow_mut() = at);
            }));
            let mut w = world(c, 0);
            for game_seed in num(3)..=num(4) {
                w.cfg.seed = game_seed;
                match run(c, &w, input_seed) {
                    None => println!(
                        "{} game_seed={game_seed} PANIC tick {} at {}",
                        c.name,
                        TICK.with(|t| t.get()),
                        LAST_PANIC.with(|p| p.borrow().clone())
                    ),
                    Some(l) => println!(
                        "{} game_seed={game_seed} ok={} {}",
                        c.name,
                        ok(c, &l),
                        summary(&l)
                    ),
                }
            }
        }
        "gen" => {
            let c = case(arg(1));
            let (game_seed, input_seed) = (num(2), num(3));
            let w = world(c, game_seed);
            let l = run(c, &w, input_seed).expect("the seed must not panic");
            assert!(ok(c, &l), "seed fails the witnesses: {}", summary(&l));
            let ticks = l.rows;
            let pairs = |v: &[(u32, usize)]| -> Vec<String> {
                v.iter().map(|(t, w)| format!("t{t}/w{w}")).collect()
            };
            let freed: Vec<String> = l
                .freed_outside
                .iter()
                .map(|(t, p, x, y)| format!("t{t} {p}@({x},{y})"))
                .collect();
            let transfers: Vec<String> = l
                .transfers
                .iter()
                .map(|(t, a, b)| format!("t{t} w{a}->w{b}"))
                .collect();
            let wb: Vec<String> = l
                .weapon_bonus_ticks
                .iter()
                .map(|(t, n)| format!("t{t} {n}"))
                .collect();
            let over = match l.game_over_tick {
                Some(g) => format!(
                    "game over at tick {g}, still over on every later tick (held={}); \
                     {POST_MORTEM_MARGIN} rows follow it",
                    l.game_over_held
                ),
                None => "never over".to_string(),
            };
            let banned_note = if c.keep_only.is_some() {
                "# STATISTICAL WITNESS (banned): 36 of 40 weapons are banned (weapTable 2), so each\n\
                 #   weapon bonus redraws `rand(40)` until it hits one of the 4 kept (game.cpp:256-258);\n\
                 #   a single draw lands on a kept weapon with p = 0.1, so P(no redraw in 3 spawns)\n\
                 #   = 0.1^3 = 10^-3.\n"
            } else {
                ""
            };
            let mut out = format!(
                "# Step 4½e-1 T7 — G3 GENERATED-LEVEL match `{name}` ({w}x{h}; plan T7). Read by BOTH\n\
                 # the C++ dumper (oracle_dump_sim_physics: `generate` = the REAL\n\
                 # Level::GenerateFromSettings over its own Rand seeded {ls}; `settings` = the real\n\
                 # Settings::FromToml + the LocalController start state) and the Rust golden test\n\
                 # (sim::levelgen::generate_from_settings + scenario::build::build_match). Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5e1_sim -- gen {name} {game_seed} {input_seed} <this file>\n\
                 # level_seed {ls} (fixed per case); seed = the game seed.\n\
                 # LEDGER (Rust, driven state): {over}\n\
                 #   spawns={spawns} deaths={deaths} respawns={respawns} reload={reload} peak_bobjects={peak}\n\
                 #   deaths at {dt}\n\
                 #   respawns at {rt}\n\
                 #   objects freed outside the level at {fo}\n\
                 #   scales transfers at {tr}\n\
                 #   weapon bonuses spawned at {wb}\n\
                 #   every spawn candidate's reads stayed inside the level; aiming angles in 0..128\n\
                 {banned_note}\
                 # Inputs: per tick Rand(input_seed).next_u32() & 0x7f, worm 0 then worm 1 (zero pairs omitted).\n\
                 seed {game_seed}\ngenerate {ls}\nticks {ticks}\nsettings sim_slice4_5e_{name}_setup.cfg\n",
                name = c.name,
                w = c.width,
                h = c.height,
                ls = c.level_seed,
                spawns = l.spawns,
                deaths = l.deaths,
                respawns = l.respawns,
                reload = l.reload_started,
                peak = l.peak_bobjects,
                dt = list(&pairs(&l.death_ticks), 12),
                rt = list(&pairs(&l.respawn_ticks), 12),
                fo = list(&freed, 6),
                tr = list(&transfers, 8),
                wb = list(&wb, 12),
            );
            for (t, i) in inputs(input_seed, ticks).iter().enumerate() {
                if i[0] != 0 || i[1] != 0 {
                    out.push_str(&format!("input {t} {} {}\n", i[0], i[1]));
                }
            }
            std::fs::write(arg(4), out).expect("write scenario");
            println!("wrote {} (ticks {ticks})", arg(4));
            println!("{}", summary(&l));
        }
        other => panic!("unknown command {other:?} (cfg | scan | gen | dump)"),
    }
}
