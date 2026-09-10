//! Step 4½a-1 T8 — setup-sidecar writer, seed scanner and scenario writer for the
//! SETTINGS-DRIVEN goldens (design §7.3). A dev tool, not a test; not run in CI.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5a -- cfg  <variant> <out_setup.cfg>
//!   cargo run -p oracle-tests --example gen_slice4_5a -- scan <variant> <input_seed> <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5a -- gen  <variant> <game_seed> <input_seed> <out_scenario.txt>
//!   cargo run -p oracle-tests --example gen_slice4_5a -- dump <variant> <game_seed> <input_seed> <ticks> <out.txt>
//!
//! Variants: killemall, scales, gametag. Run it in a DEBUG build: every deferred sim
//! branch is a `debug_assert!`, and `scan` skips (and reports) a seed that trips one.
//! Inputs: per tick `Rand(input_seed).next_u32() & 0x7f` for worm 0 then worm 1,
//! applied on the pass advancing t -> t+1 (the dumper seam, as the slice-6 fuzz).
//!
//! Two rulings amend the plan's sketch:
//!
//! * **R3** — the ledger also witnesses that `is_game_over` STAYS true once it fires
//!   (`game_over_held`). Scales lives can rise again after the game is over, so a seed
//!   whose flag flickered would fail T9's "stays true" assertion on the C++ column.
//! * **R12** — the `WORMS` table varies the non-sim fields too (design §7.3 "every
//!   non-sim field non-default"): `controller`, `inputDevice` and the network player's
//!   `gamepadControls`. `tc` stays `'openliero'` — the only TC — a documented exception.

use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};
use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use scenario::build::build_match;
use scenario::settings::MatchConfig;
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState, WeaponId, NUM_WEAPONS};
use sim_core::fixed::ftoi;
use sim_core::rng::Rand;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const LEVEL: &str = "Levels/modern_test.lev";
/// Scan horizon; a kept seed ends its match by `MAX_TICKS - POST_MORTEM_MARGIN`.
const MAX_TICKS: u32 = 3000;
/// Ticks recorded after the game-over tick (>= the 180-frame C++ post-mortem).
const POST_MORTEM_MARGIN: u32 = 200;
/// Deferred Step-2 sim branches (design §1.3 finding 8): shotType 4 (the laser
/// do-loop) + MISSILE (ProcessSteerables). `weap_table = 2`, never in a loadout.
const BANNED: [&str; 5] = ["RIFLE", "WINCHESTER", "LASER", "GAUSS GUN", "MISSILE"];
/// `weap_table = 1` (bonus only): sim-inert until weapon selection, varied for coverage.
const BONUS_ONLY: [&str; 3] = ["DOOMSDAY", "HELLRAIDER", "CHIQUITA BOMB"];

struct Variant {
    name: &'static str,
    game_mode: u32,
    lives: i32,
    health: i32,
    loading_time: i32,
    blood: i32,
    load_change: bool,
    max_bonuses: i32,
    shadow: bool,
    time_to_lose: i32,
    blood_particle_max: i32,
    p1: [&'static str; 5],
    p2: [&'static str; 5],
}

/// Design §7.3's coverage table. Loadouts: golden-proven weapons only.
const VARIANTS: [Variant; 3] = [
    Variant {
        name: "killemall",
        game_mode: 0,
        lives: 1,
        health: 150,
        loading_time: 37,
        blood: 250,
        load_change: false,
        max_bonuses: 6,
        shadow: true,
        time_to_lose: 600,
        blood_particle_max: 300,
        p1: ["BAZOOKA", "DART", "CANNON", "GRENADE", "HANDGUN"],
        p2: ["EXPLOSIVES", "GREENBALL", "FAN", "BAZOOKA", "DART"],
    },
    Variant {
        name: "scales",
        game_mode: 3,
        lives: 2,
        health: 120,
        loading_time: 150,
        blood: 60,
        load_change: true,
        max_bonuses: 8,
        shadow: true,
        time_to_lose: 600,
        blood_particle_max: 500,
        p1: ["GRENADE", "FAN", "EXPLOSIVES", "HANDGUN", "CANNON"],
        p2: ["DART", "BAZOOKA", "GREENBALL", "GRENADE", "FAN"],
    },
    Variant {
        name: "gametag",
        game_mode: 1,
        lives: 3,
        health: 80,
        loading_time: 0,
        blood: 0,
        load_change: true,
        max_bonuses: 3,
        shadow: false,
        time_to_lose: 12,
        blood_particle_max: 700,
        p1: ["CANNON", "BAZOOKA", "DART", "FAN", "GREENBALL"],
        p2: ["HANDGUN", "EXPLOSIVES", "GRENADE", "CANNON", "BAZOOKA"],
    },
];

fn variant(name: &str) -> &'static Variant {
    VARIANTS
        .iter()
        .find(|v| v.name == name)
        .unwrap_or_else(|| panic!("unknown variant {name}"))
}

fn load_objects() -> Objects {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap()
}

fn load_level() -> LevelData {
    assets::level::load(&std::fs::read(format!("{TC_ROOT}/{LEVEL}")).unwrap()).unwrap()
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
    let mut order: Vec<usize> = (0..o.weapons.len()).collect();
    order.sort_by(|&a, &b| o.weapons[a].name.cmp(&o.weapons[b].name));
    order
        .iter()
        .position(|&i| o.weapons[i].name == name)
        .unwrap() as u32
        + 1
}

fn arr(v: &[u32]) -> String {
    let items: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("[ {} ]", items.join(", "))
}

struct WormCfg {
    header: &'static str,
    color: i32,
    /// R12: 0 human / 1 DumbLieroAI / 2 FollowAI — non-default for every worm. Read by
    /// `LocalController` only; neither the dumper's `settings` start nor `build_match`
    /// looks at it, so it is pure reader coverage (T9 guards the parsed value).
    controller: u32,
    controls: [u32; 7],
    dig: u32,
    gamepad: [u32; 8],
    pad_name: &'static str,
    pad_serial: &'static str,
    input_device: u32,
    name: &'static str,
    rgb: [u32; 3],
}

/// Non-default values for every non-sim worm field (fixed across variants). Ordered as
/// toml++ sorts the tables: `network_player`, `player1`, `player2`.
const WORMS: [WormCfg; 3] = [
    WormCfg {
        header: "network_player",
        color: 44,
        controller: 1,
        controls: [19, 33, 32, 34, 29, 42, 56],
        dig: 58,
        // R12: the C++ default is [11, 12, 13, 14, 110, 10, 0, 9] — shifted off it.
        gamepad: [15, 16, 17, 18, 111, 8, 2, 7],
        pad_name: "Pad Net",
        pad_serial: "SN-3",
        input_device: 2,
        name: "Netty",
        rgb: [200, 100, 50],
    },
    WormCfg {
        header: "player1",
        color: 33,
        controller: 1,
        controls: [76, 80, 79, 81, 163, 168, 165],
        dig: 54,
        gamepad: [1, 2, 3, 4, 5, 6, 7, 8],
        pad_name: "Pad One",
        pad_serial: "SN-1",
        // R12: 0 (keyboard) is the default — gamepad 2.
        input_device: 3,
        name: "Lefty",
        rgb: [250, 10, 128],
    },
    WormCfg {
        header: "player2",
        color: 42,
        controller: 2,
        controls: [17, 31, 30, 32, 20, 21, 22],
        dig: 57,
        gamepad: [21, 22, 23, 24, 125, 20, 1, 19],
        pad_name: "Pad Two",
        pad_serial: "SN-2",
        input_device: 1,
        name: "Righty",
        rgb: [12, 240, 99],
    },
];

/// The setup sidecar, in toml++'s canonical layout (sorted tables and keys, literal
/// strings, `weapTable` multiline) so 4½a-2's byte gate can reuse it as an input.
fn setup_cfg(v: &Variant, o: &Objects) -> String {
    for n in v.p1.iter().chain(v.p2.iter()) {
        assert!(!BANNED.contains(n), "{n} is a deferred-branch weapon");
    }
    let mut table = [0u32; 40];
    for n in BANNED {
        table[weapon_index(o, n)] = 2;
    }
    for n in BONUS_ONLY {
        table[weapon_index(o, n)] = 1;
    }
    let pick = |names: &[&str; 5]| -> [u32; 5] { names.map(|n| menu_index(o, n)) };
    let loadouts = [[2, 3, 4, 5, 6], pick(&v.p1), pick(&v.p2)];
    let healths = [100, v.health, v.health];

    let mut out = String::new();
    for (i, w) in WORMS.iter().enumerate() {
        let mut ex = [0u32; 8];
        ex[..7].copy_from_slice(&w.controls);
        ex[7] = w.dig;
        out.push_str(&format!("[{}]\n", w.header));
        out.push_str(&format!("color = {}\n", w.color));
        out.push_str(&format!("controller = {}\n", w.controller));
        out.push_str(&format!("controls = {}\n", arr(&w.controls)));
        out.push_str(&format!("controlsEx = {}\n", arr(&ex)));
        out.push_str(&format!("gamepadControls = {}\n", arr(&w.gamepad)));
        out.push_str(&format!("gamepadName = '{}'\n", w.pad_name));
        out.push_str(&format!("gamepadSerial = '{}'\n", w.pad_serial));
        out.push_str(&format!("health = {}\n", healths[i]));
        out.push_str(&format!("inputDevice = {}\n", w.input_device));
        out.push_str(&format!("name = '{}'\n", w.name));
        out.push_str("randomName = false\n");
        out.push_str(&format!("rgb = {}\n", arr(&w.rgb)));
        out.push_str("rgbDepth = 8\n");
        out.push_str(&format!("weapons = {}\n\n", arr(&loadouts[i])));
    }
    let s = |b: bool| if b { "true" } else { "false" };
    out.push_str("[settings]\n");
    out.push_str("aiFrames = 99\naiMutations = 5\naiParallels = 7\naiTraces = true\n");
    out.push_str("allowViewingSpawnPoint = true\n");
    out.push_str(&format!("blood = {}\n", v.blood));
    out.push_str(&format!("bloodParticleMax = {}\n", v.blood_particle_max));
    out.push_str("bonusTimeout = 45\nflagsToWin = 7\nfullscreen = true\n");
    out.push_str(&format!("gameMode = {}\n", v.game_mode));
    out.push_str("inputDelay = 3\nlevelFile = 'Levels/modern_test.lev'\n");
    out.push_str(&format!("lives = {}\n", v.lives));
    out.push_str(&format!("loadChange = {}\n", s(v.load_change)));
    out.push_str("loadPowerlevelPalette = false\n");
    out.push_str(&format!("loadingTime = {}\n", v.loading_time));
    out.push_str("map = false\n");
    out.push_str(&format!("maxBonuses = {}\n", v.max_bonuses));
    out.push_str("maxSpectatorRenderHeight = 720\nmodernColors = true\nnamesOnBonuses = true\n");
    out.push_str("randomLevel = false\nrandomMapHeight = 400\nrandomMapWidth = 640\n");
    out.push_str("recordReplays = false\nregenerateLevel = true\nscreenSync = false\n");
    out.push_str("selectBotWeapons = 2\n");
    out.push_str(&format!("shadow = {}\n", s(v.shadow)));
    out.push_str("singleScreenReplay = true\nspectatorWindow = true\ntc = 'openliero'\n");
    out.push_str(&format!("timeToLose = {}\n", v.time_to_lose));
    out.push_str("version = 6\nweapTable = [\n");
    let rows: Vec<String> = table.iter().map(|t| format!("    {t}")).collect();
    out.push_str(&rows.join(",\n"));
    out.push_str("\n]\nzoneTimeout = 90\n");
    out
}

fn inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    (0..ticks)
        .map(|_| [r.next_u32() & 0x7f, r.next_u32() & 0x7f])
        .collect()
}

/// The per-tick facts the witnesses compare across a tick boundary.
struct Snap {
    visible: [bool; 2],
    lives: [i32; 2],
    timer: [i32; 2],
    ipos: [(i32, i32); 2],
    weapon_ty: [[Option<WeaponId>; NUM_WEAPONS]; 2],
    /// Live bonus positions in whole pixels (the pickup AABB's units).
    bonuses: Vec<(i32, i32)>,
}

fn snap(s: &SimState) -> Snap {
    let g = |i: usize| {
        let w = &s.worms[i];
        (
            w.visible,
            w.lives,
            w.timer,
            (ftoi(w.pos.x), ftoi(w.pos.y)),
            w.weapons.map(|x| x.ty),
        )
    };
    let (v0, l0, t0, p0, y0) = g(0);
    let (v1, l1, t1, p1, y1) = g(1);
    Snap {
        visible: [v0, v1],
        lives: [l0, l1],
        timer: [t0, t1],
        ipos: [p0, p1],
        weapon_ty: [y0, y1],
        bonuses: s.bonuses.iter().map(|b| (ftoi(b.x), ftoi(b.y))).collect(),
    }
}

/// The strict 11x11 pickup AABB (`worm.cpp:289-290`): a bonus that vanished under a
/// living worm was COLLECTED, not expired.
fn on_bonus(w: (i32, i32), b: (i32, i32)) -> bool {
    (w.0 - b.0).abs() < 5 && (w.1 - b.1).abs() < 5
}

/// Everything the witnesses read, from the genuinely driven Rust state.
#[derive(Default, Debug)]
struct Ledger {
    game_over_tick: Option<u32>,
    /// R3: `is_game_over` was still true on EVERY tick after it first fired.
    game_over_held: bool,
    deaths: u32,
    respawns: u32,
    peak_bobjects: usize,
    reload_started: bool,
    bonus_dropped: bool,
    /// Ticks on which the bonus pool grew (the settings-driven `max_bonuses` roll).
    bonus_drop_ticks: Vec<u32>,
    /// `(tick, worm)` of every bonus collected off the ground.
    pickup_ticks: Vec<(u32, usize)>,
    /// A pickup that swapped a weapon slot's type — the `weap_table` path.
    weapon_pickup_ticks: Vec<(u32, usize)>,
    scales_death_kept_health: bool,
    life_gained: bool,
    timer_bumped: bool,
    /// Per worm: the lowest `lives` seen, and the value at the last recorded tick.
    /// Diagnostic only — Scales hands lives back, so the *floor* is what decides
    /// whether a seed can ever reach `is_game_over`.
    min_lives: [i32; 2],
    end_lives: [i32; 2],
    /// `(tick, worm, lives)` for every tick boundary at which `lives` changed.
    life_events: Vec<(u32, usize, i32)>,
    /// `(tick, worm)` of every death, for lining deaths up against `life_events`.
    death_events: Vec<(u32, usize)>,
    level_series: Vec<u32>,
}

impl Ledger {
    /// A compact one-liner (the full `Debug` carries the 3000-entry level series).
    fn summary(&self) -> String {
        format!(
            "go={:?} held={} deaths={} respawns={} peak_bob={} reload={} bonus={} \
             drops={} pickups={} weap_pickups={} scales_death={} life_gained={} timer={} \
             min_lives={:?} end_lives={:?} deaths_at={:?} lives_at={:?}",
            self.game_over_tick,
            self.game_over_held,
            self.deaths,
            self.respawns,
            self.peak_bobjects,
            self.reload_started,
            self.bonus_dropped,
            self.bonus_drop_ticks.len(),
            self.pickup_ticks.len(),
            self.weapon_pickup_ticks.len(),
            self.scales_death_kept_health,
            self.life_gained,
            self.timer_bumped,
            self.min_lives,
            self.end_lives,
            self.death_events,
            self.life_events,
        )
    }
}

fn drive(cfg: &MatchConfig, level: &LevelData, ins: &[[u32; 2]]) -> Ledger {
    let mut st = build_match(Path::new(TC_ROOT), cfg, level)
        .expect("variant config builds")
        .state;
    let mut prev = snap(&st);
    let mut l = Ledger {
        game_over_held: true,
        min_lives: prev.lives,
        end_lives: prev.lives,
        level_series: vec![hash_components(&st).level],
        ..Ledger::default()
    };
    for (t, w) in ins.iter().enumerate() {
        st.process_frame(&[ControlState::unpack(w[0]), ControlState::unpack(w[1])]);
        let k = t as u32 + 1;
        let cur = snap(&st);
        l.level_series.push(hash_components(&st).level);
        for i in 0..2 {
            if prev.visible[i] && !cur.visible[i] {
                l.deaths += 1;
                l.death_events.push((k, i));
                if st.worms[i].health > 0 {
                    l.scales_death_kept_health = true;
                }
            }
            if !prev.visible[i] && cur.visible[i] {
                l.respawns += 1;
            }
            l.life_gained |= cur.lives[i] > prev.lives[i];
            if cur.lives[i] != prev.lives[i] {
                l.life_events.push((k, i, cur.lives[i]));
            }
            l.min_lives[i] = l.min_lives[i].min(cur.lives[i]);
            l.end_lives[i] = cur.lives[i];
            l.timer_bumped |= cur.timer[i] > prev.timer[i];
            l.reload_started |= st.worms[i].weapons.iter().any(|ww| ww.loading_left > 0);
            if prev.visible[i] && cur.visible[i] && cur.bonuses.len() < prev.bonuses.len() {
                let taken = prev
                    .bonuses
                    .iter()
                    .filter(|b| on_bonus(prev.ipos[i], **b))
                    .any(|b| !cur.bonuses.contains(b));
                if taken {
                    l.pickup_ticks.push((k, i));
                    if cur.weapon_ty[i] != prev.weapon_ty[i] {
                        l.weapon_pickup_ticks.push((k, i));
                    }
                }
            }
        }
        if cur.bonuses.len() > prev.bonuses.len() {
            l.bonus_drop_ticks.push(k);
        }
        l.peak_bobjects = l.peak_bobjects.max(st.bobjects.len());
        l.bonus_dropped |= !cur.bonuses.is_empty();
        // R3: the flag must LATCH — Scales can hand a life back after the game is over.
        if l.game_over_tick.is_none() {
            if is_game_over(&st) {
                l.game_over_tick = Some(k);
            }
        } else {
            l.game_over_held &= is_game_over(&st);
        }
        prev = cur;
        if let Some(g) = l.game_over_tick {
            if k >= g + POST_MORTEM_MARGIN {
                break;
            }
        }
    }
    l
}

/// The match ended early enough that the whole post-mortem window fits the horizon.
fn ends(l: &Ledger) -> bool {
    matches!(l.game_over_tick, Some(g) if g + POST_MORTEM_MARGIN <= MAX_TICKS)
}

/// The variant's witnesses (design §7.3). `shadow_fired`: the level column differs
/// from a `shadow = false` re-run of the same inputs.
///
/// The settings-driven BONUS path (`max_bonuses` + `weap_table`) is required in two
/// strengths, because a pickup needs the random input stream to walk a worm onto an
/// 11x11 box and only ~1 seed in 200 manages it inside a variant's window: every
/// variant with a short window asks for the DROP (`bonus_dropped`), and `gametag` —
/// the long one — asks for a real PICKUP as well, so at least one golden covers the
/// whole path (T7 review's mandatory coverage).
fn ok(v: &Variant, l: &Ledger, shadow_fired: bool) -> bool {
    let base = ends(l) && l.game_over_held && l.deaths >= 1 && l.respawns >= 2;
    base && match v.name {
        "killemall" => {
            shadow_fired
                && l.peak_bobjects == v.blood_particle_max as usize
                && l.reload_started
                && l.bonus_dropped
        }
        "scales" => shadow_fired && l.scales_death_kept_health && l.life_gained,
        "gametag" => l.timer_bumped && !l.pickup_ticks.is_empty(),
        _ => unreachable!(),
    }
}

struct Run {
    ledger: Ledger,
    shadow_fired: bool,
}

fn run(v: &Variant, game_seed: u32, input_seed: u32) -> Option<Run> {
    let objects = load_objects();
    let level = load_level();
    let settings = settings_from_toml(&setup_cfg(v, &objects)).expect("sidecar parses");
    let cfg = MatchConfig {
        settings,
        seed: game_seed,
    };
    let ins = inputs(input_seed, MAX_TICKS);
    catch_unwind(AssertUnwindSafe(|| {
        let ledger = drive(&cfg, &level, &ins);
        // The shadow witness costs a second full drive, and `ok` short-circuits on
        // `ends` before it ever reads the flag — so only a seed that ends pays for it.
        let shadow_fired = ends(&ledger) && {
            let mut off = cfg.clone();
            off.settings.shadow = false;
            let off_ledger = drive(&off, &level, &ins);
            let n = ledger.level_series.len().min(off_ledger.level_series.len());
            ledger.level_series[..n] != off_ledger.level_series[..n]
        };
        Run {
            ledger,
            shadow_fired,
        }
    }))
    .ok()
}

/// `(tick, worm)` pairs rendered for the scenario header / the scan log, capped.
fn ticks_of(pairs: &[(u32, usize)], max: usize) -> String {
    if pairs.is_empty() {
        return "none".to_string();
    }
    let head: Vec<String> = pairs
        .iter()
        .take(max)
        .map(|(t, w)| format!("t{t}/w{w}"))
        .collect();
    let tail = if pairs.len() > max { ", ..." } else { "" };
    format!("{}{tail}", head.join(", "))
}

fn first_ticks(ticks: &[u32], max: usize) -> String {
    if ticks.is_empty() {
        return "none".to_string();
    }
    let head: Vec<String> = ticks.iter().take(max).map(|t| format!("t{t}")).collect();
    let tail = if ticks.len() > max { ", ..." } else { "" };
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
            let text = setup_cfg(variant(arg(1)), &load_objects());
            std::fs::write(arg(2), text).expect("write setup");
            println!("wrote {}", arg(2));
        }
        "dump" => {
            // The Rust side of a golden, in the dumper's exact 12-column layout, so a
            // divergence localises with a plain `diff` (T9 owns the actual test).
            let v = variant(arg(1));
            let (game_seed, input_seed, ticks) = (num(2), num(3), num(4));
            let objects = load_objects();
            let settings = settings_from_toml(&setup_cfg(v, &objects)).expect("sidecar parses");
            let cfg = MatchConfig {
                settings,
                seed: game_seed,
            };
            let mut st = build_match(Path::new(TC_ROOT), &cfg, &load_level())
                .expect("variant config builds")
                .state;
            let mut out = String::new();
            let mut row = |t: u32, st: &SimState| {
                let c = hash_components(st);
                out.push_str(&format!(
                    "{t} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {}\n",
                    hash_game_state(st),
                    c.rng,
                    c.level,
                    c.worms[0],
                    c.worms[1],
                    c.bobjects,
                    c.bonuses,
                    c.sobjects,
                    c.nobjects,
                    c.wobjects,
                    u32::from(is_game_over(st)),
                ));
            };
            row(0, &st);
            for (t, w) in inputs(input_seed, ticks).iter().enumerate() {
                st.process_frame(&[ControlState::unpack(w[0]), ControlState::unpack(w[1])]);
                row(t as u32 + 1, &st);
            }
            std::fs::write(arg(5), out).expect("write dump");
            println!("wrote {}", arg(5));
        }
        "scan" => {
            let v = variant(arg(1));
            let input_seed = num(2);
            set_hook(Box::new(|_| {})); // a deferred-branch panic is reported below
            for game_seed in num(3)..=num(4) {
                match run(v, game_seed, input_seed) {
                    None => println!(
                        "{} game_seed={game_seed} PANIC (deferred sim branch)",
                        v.name
                    ),
                    Some(r) => println!(
                        "{} game_seed={game_seed} ok={} {} shadow={}",
                        v.name,
                        ok(v, &r.ledger, r.shadow_fired),
                        r.ledger.summary(),
                        r.shadow_fired,
                    ),
                }
            }
        }
        "gen" => {
            let v = variant(arg(1));
            let (game_seed, input_seed) = (num(2), num(3));
            let r =
                run(v, game_seed, input_seed).expect("the seed must not trip a deferred branch");
            assert!(
                ok(v, &r.ledger, r.shadow_fired),
                "seed fails the witnesses: {} shadow={}",
                r.ledger.summary(),
                r.shadow_fired,
            );
            let g = r.ledger.game_over_tick.unwrap();
            let ticks = g + POST_MORTEM_MARGIN;
            let l = &r.ledger;
            let mut out = format!(
                "# Step 4½ slice 4½a-1 T8 — SETTINGS-DRIVEN match `{name}` (design §7.3). Read by BOTH\n\
                 # the C++ dumper (oracle_dump_sim_physics `settings` path: the real Settings::FromToml\n\
                 # + the LocalController start state) and the Rust golden test (settings_toml reader +\n\
                 # scenario::build::build_match). Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5a -- gen {name} {game_seed} {input_seed} <this file>\n\
                 # LEDGER (Rust, driven state): game over at tick {g}, still over on every later tick\n\
                 #   (game_over_held={held}); deaths={deaths} respawns={respawns} peak_bobjects={peak}\n\
                 #   reload={reload} bonus_dropped={bonus} scales_death_kept_health={scales}\n\
                 #   life_gained={life} timer_bumped={timer} shadow_fired={shadow}\n\
                 #   bonus drops at {drops}\n\
                 #   bonus pickups at {pickups}   (weapon swaps at {weap})\n\
                 # Inputs: per tick Rand(input_seed).next_u32() & 0x7f, worm 0 then worm 1 (zero pairs omitted).\n\
                 seed {game_seed}\nlevel {LEVEL}\nticks {ticks}\nsettings sim_slice4_5a_{name}_setup.cfg\n",
                name = v.name,
                held = l.game_over_held,
                deaths = l.deaths,
                respawns = l.respawns,
                peak = l.peak_bobjects,
                reload = l.reload_started,
                bonus = l.bonus_dropped,
                scales = l.scales_death_kept_health,
                life = l.life_gained,
                timer = l.timer_bumped,
                shadow = r.shadow_fired,
                drops = first_ticks(&l.bonus_drop_ticks, 12),
                pickups = ticks_of(&l.pickup_ticks, 12),
                weap = ticks_of(&l.weapon_pickup_ticks, 12),
            );
            for (t, w) in inputs(input_seed, ticks).iter().enumerate() {
                if w[0] != 0 || w[1] != 0 {
                    out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
                }
            }
            std::fs::write(arg(4), out).expect("write scenario");
            println!("wrote {} (game over at tick {g}, ticks {ticks})", arg(4));
            println!("{} shadow={}", l.summary(), r.shadow_fired);
        }
        other => panic!("unknown command {other:?} (cfg | scan | gen)"),
    }
}
