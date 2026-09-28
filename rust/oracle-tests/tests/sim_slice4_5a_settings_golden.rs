//! Step 4½a-1 T9 — MILESTONE (design §7): four SETTINGS-DRIVEN matches bit-exact vs C++.
//!
//! Each committed scenario names a C++-schema setup file (`settings <file>`, relative to
//! the scenario file's directory). The C++ dumper read it with the real
//! `Settings::FromToml` and started the worms in the C++ LocalController state; here the
//! SAME committed files go through the real Rust readers — `Scenario::parse` (the
//! `settings` directive), `settings_toml::settings_from_toml` on the sidecar the
//! directive names, and `scenario::build::build_match`. Nothing is regenerated in memory.
//! Every golden row is asserted on the 11 hash columns (components first, master last)
//! and on column 12, `Game::IsGameOver()`, against `sim::game_over::is_game_over`.
//!
//! Variants (design §7.3): `defaults` (the shipped legacy `data/Setups/liero.cfg`, no
//! input, never over), `killemall` (lives 1, health 150, loading 37, blood 250,
//! !loadChange, bonuses 6, shadow, pool 300), `scales` (mode 3, lives 2, health 40 —
//! T8 fix round lowered it from the design's 120 so the match ends — loading 150, blood
//! 60, bonuses 8, pool 500), `gametag` (mode 1, timeToLose 12, blood 0, loading 0,
//! shadow off, bonuses 3). Row counts come from the committed files (`ticks + 1`).

use std::path::Path;

use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::{
    MatchConfig, Settings, WormSettings, GM_GAME_OF_TAG, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE,
};
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// The directory the scenario files live in — a `settings <file>` path is relative to it.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// C++ `LocalController`: 180 simulated frames after the game-over frame.
const POST_MORTEM: usize = 180;

/// Read `rel` relative to the scenario files' directory.
fn read(rel: &str) -> String {
    let path = Path::new(GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

struct Row {
    tick: u32,
    hashes: [u32; 10], // master, rng, level, worm0, worm1, bob, bon, sob, nob, wob
    game_over: u32,
}

/// Parse a 12-column settings-path golden; row `k` must carry tick `k`.
fn parse_golden(text: &str) -> Vec<Row> {
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
            let game_over: u32 = cols[11].parse().expect("game-over column");
            assert!(game_over <= 1, "IsGameOver column is 0/1, got {game_over}");
            Row {
                tick: cols[0].parse().expect("tick"),
                hashes,
                game_over,
            }
        })
        .collect();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(r.tick, k as u32, "golden row {k} carries tick {}", r.tick);
    }
    rows
}

fn check(state: &SimState, row: &Row) {
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
    // Components first, master last, so a divergence localises.
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

struct Case {
    scenario: Scenario,
    cfg: MatchConfig,
}

/// Parse the COMMITTED `sim_slice4_5a_<name>_scenario.txt`, then read the setup sidecar
/// its `settings` directive names (`want_sidecar`, relative to the scenario directory)
/// through the real `settings_from_toml`.
fn case(name: &str, want_sidecar: &str) -> Case {
    let scenario = Scenario::parse(&read(&format!("sim_slice4_5a_{name}_scenario.txt")))
        .unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"));
    let rel = scenario
        .settings
        .clone()
        .expect("a settings-driven scenario");
    assert_eq!(rel, want_sidecar, "{name}: the directive names the sidecar");
    assert!(scenario.worms.is_empty(), "worms come from the setup");
    let settings = settings_from_toml(&read(&rel))
        .unwrap_or_else(|e| panic!("{name}: setup {rel} parses: {e:?}"));
    let cfg = MatchConfig {
        settings,
        seed: scenario.seed,
    };
    Case { scenario, cfg }
}

fn matrix_case(name: &str) -> Case {
    case(name, &format!("sim_slice4_5a_{name}_setup.cfg"))
}

fn defaults_case() -> Case {
    case("defaults", "../../../data/Setups/liero.cfg")
}

fn golden(name: &str, c: &Case) -> Vec<Row> {
    let g = parse_golden(&read(&format!("sim_slice4_5a_{name}.txt")));
    assert_eq!(
        g.len() as u32,
        c.scenario.ticks + 1,
        "{name}: golden rows 0..=ticks"
    );
    g
}

/// What the coverage witnesses read, from the genuinely driven Rust state.
#[derive(Default)]
struct Witness {
    game_over_tick: Option<u32>,
    deaths: u32,
    respawns: u32,
    peak_bobjects: usize,
    reload_started: bool,
    bonus_dropped: bool,
    scales_death_kept_health: bool,
    life_gained: bool,
    timer_bumped: bool,
    level_series: Vec<u32>,
    masters: Vec<u32>,
}

fn drive(c: &Case, cfg: &MatchConfig, golden: Option<&[Row]>) -> Witness {
    let bytes = std::fs::read(format!("{TC_ROOT}/{}", c.scenario.level)).expect("read level");
    let level = assets::level::load(&bytes).expect("level loads");
    let mut st = build_match(Path::new(TC_ROOT), cfg, &level)
        .expect("builds")
        .state;
    let mut w = Witness::default();
    if let Some(g) = golden {
        assert_eq!(
            g.len() as u32,
            c.scenario.ticks + 1,
            "golden rows 0..=ticks"
        );
        check(&st, &g[0]);
    }
    w.level_series.push(hash_components(&st).level);
    w.masters.push(hash_game_state(&st));
    for k in 1..=c.scenario.ticks {
        let prev: Vec<(bool, i32, i32)> = st
            .worms
            .iter()
            .map(|x| (x.visible, x.lives, x.timer))
            .collect();
        st.process_frame(&[
            ControlState::unpack(c.scenario.input(k - 1, 0)),
            ControlState::unpack(c.scenario.input(k - 1, 1)),
        ]);
        if let Some(g) = golden {
            check(&st, &g[k as usize]);
        }
        w.level_series.push(hash_components(&st).level);
        w.masters.push(hash_game_state(&st));
        for (i, x) in st.worms.iter().enumerate() {
            let (was_visible, lives, timer) = prev[i];
            if was_visible && !x.visible {
                w.deaths += 1;
                w.scales_death_kept_health |= x.health > 0;
            }
            if !was_visible && x.visible {
                w.respawns += 1;
            }
            w.life_gained |= x.lives > lives;
            w.timer_bumped |= x.timer > timer;
            w.reload_started |= x.weapons.iter().any(|ww| ww.loading_left > 0);
        }
        w.peak_bobjects = w.peak_bobjects.max(st.bobjects.len());
        w.bonus_dropped |= !st.bonuses.is_empty();
        if w.game_over_tick.is_none() && is_game_over(&st) {
            w.game_over_tick = Some(k);
        }
    }
    w
}

/// `CorrectShadow` fired: the same match with `shadow = false` carves a different level.
fn shadow_fired(c: &Case, on: &Witness) -> bool {
    let mut off = c.cfg.clone();
    off.settings.shadow = false;
    drive(c, &off, None).level_series != on.level_series
}

/// Ruling R3: C++ `IsGameOver` flips 0 -> 1 exactly at `want_flip` (the Rust
/// `is_game_over` first-true tick too), is 1 on that row AND on every later row, and at
/// least the 180-frame post-mortem is recorded after it. (`check` has already asserted
/// Rust == C++ on column 12 for every row.)
fn assert_post_mortem(name: &str, golden: &[Row], w: &Witness, want_flip: u32) {
    let g = w
        .game_over_tick
        .unwrap_or_else(|| panic!("{name}: the match must end"));
    let first = golden
        .iter()
        .position(|r| r.game_over == 1)
        .expect("golden game over");
    assert_eq!(first as u32, want_flip, "{name}: C++ flip tick");
    assert_eq!(
        first as u32, g,
        "{name}: Rust and C++ agree on the game-over tick"
    );
    assert_eq!(golden[first].game_over, 1, "{name}: flip row is 1");
    assert!(
        golden[first..].iter().all(|r| r.game_over == 1),
        "{name}: IsGameOver stays true on every row after the flip"
    );
    assert!(
        golden.len() - first > POST_MORTEM,
        "{name}: >= 180 post-mortem ticks recorded"
    );
}

/// `weap_table` of every generated sidecar: RIFLE/WINCHESTER/LASER/GAUSS GUN/MISSILE
/// (indices 2/7/28/32/39) banned = 2; DOOMSDAY/HELLRAIDER/CHIQUITA BOMB (8/25/33)
/// bonus-only = 1; everything else 0.
fn want_weap_table() -> [u32; 40] {
    let mut t = [0u32; 40];
    for i in [2, 7, 28, 32, 39] {
        t[i] = 2;
    }
    for i in [8, 25, 33] {
        t[i] = 1;
    }
    t
}

/// One worm's non-sim fields as the generator (`gen_slice4_5a.rs` `WORMS`) wrote them.
type WormNonSim<'a> = (
    &'a str,  // name
    [i32; 3], // rgb
    [u32; 7], // controls
    [u32; 8], // controls_ex
    [u32; 8], // gamepad_controls
    &'a str,  // gamepad_name
    &'a str,  // gamepad_serial
    bool,     // random_name
    i32,      // color
    u32,      // controller
    u32,      // input_device
);

fn non_sim(ws: &WormSettings) -> WormNonSim<'_> {
    (
        &ws.name,
        ws.rgb,
        ws.controls,
        ws.controls_ex,
        ws.gamepad_controls,
        &ws.gamepad_name,
        &ws.gamepad_serial,
        ws.random_name,
        ws.color,
        ws.controller,
        ws.input_device,
    )
}

/// Intent guard (ruling R12, spec wins): the NON-sim fields carry the generator's
/// non-default values after parsing, so a misspelled reader key cannot hide behind a
/// default. Identical across the three matrix sidecars.
fn assert_non_sim_fields(name: &str, s: &Settings) {
    assert_eq!(
        (
            s.ai_frames,
            s.ai_mutations,
            s.ai_parallels,
            s.zone_timeout,
            s.select_bot_weapons,
            s.flags_to_win,
            s.bonus_timeout,
            s.input_delay,
            s.random_map_width,
            s.random_map_height,
            s.max_spectator_render_height
        ),
        (99, 5, 7, 90, 2, 7, 45, 3, 640, 400, 720),
        "{name}: non-sim scalars"
    );
    assert!(
        s.ai_traces
            && s.allow_viewing_spawn_point
            && s.fullscreen
            && s.modern_colors
            && s.names_on_bonuses
            && s.regenerate_level
            && s.single_screen_replay
            && s.spectator_window
            && !s.map
            && !s.random_level
            && !s.record_replays
            && !s.screen_sync
            && !s.load_powerlevel_palette,
        "{name}: non-sim bools"
    );
    assert_eq!(s.level_file, "Levels/modern_test.lev", "{name}: levelFile");
    assert_eq!(
        s.tc, "openliero",
        "{name}: tc (the documented default exception)"
    );
    // Worm order: 0 = player1, 1 = player2, 2 = network_player.
    let want: [WormNonSim<'static>; 3] = [
        (
            "Lefty",
            [250, 10, 128],
            [76, 80, 79, 81, 163, 168, 165],
            [76, 80, 79, 81, 163, 168, 165, 54],
            [1, 2, 3, 4, 5, 6, 7, 8],
            "Pad One",
            "SN-1",
            false,
            33,
            1,
            3,
        ),
        (
            "Righty",
            [12, 240, 99],
            [17, 31, 30, 32, 20, 21, 22],
            [17, 31, 30, 32, 20, 21, 22, 57],
            [21, 22, 23, 24, 125, 20, 1, 19],
            "Pad Two",
            "SN-2",
            false,
            42,
            2,
            1,
        ),
        (
            "Netty",
            [200, 100, 50],
            [19, 33, 32, 34, 29, 42, 56],
            [19, 33, 32, 34, 29, 42, 56, 58],
            [15, 16, 17, 18, 111, 8, 2, 7],
            "Pad Net",
            "SN-3",
            false,
            44,
            1,
            2,
        ),
    ];
    for (i, want) in want.iter().enumerate() {
        assert_eq!(&non_sim(&s.worm_settings[i]), want, "{name}: worm {i}");
    }
    // The R12 fields, spelled out.
    let ws = &s.worm_settings;
    assert_eq!(
        (ws[0].controller, ws[1].controller, ws[2].controller),
        (1, 2, 1),
        "{name}: controller players 1/2/network"
    );
    assert_eq!(ws[0].input_device, 3, "{name}: player1 inputDevice");
    assert_eq!(
        ws[2].gamepad_controls,
        [15, 16, 17, 18, 111, 8, 2, 7],
        "{name}: network gamepadControls"
    );
    // The network player never plays; its sidecar values are the generator's fixed ones.
    assert_eq!((ws[2].health, ws[2].weapons), (100, [2, 3, 4, 5, 6]));
}

/// The sim-reaching per-worm fields shared by every matrix variant.
fn assert_loadouts(s: &Settings, health: i32, p1: [u32; 5], p2: [u32; 5]) {
    let ws = &s.worm_settings;
    assert_eq!((ws[0].health, ws[1].health), (health, health));
    assert_eq!(
        (ws[0].weapons, ws[1].weapons),
        (p1, p2),
        "per-worm loadouts"
    );
    assert_eq!(s.weap_table, want_weap_table(), "weap_table");
}

#[test]
fn defaults_the_shipped_setup_matches_cpp() {
    let c = defaults_case();
    assert_eq!(
        c.cfg.settings,
        Settings::default(),
        "shipped liero.cfg == C++ Settings()"
    );
    let golden = golden("defaults", &c);
    assert!(golden.iter().all(|r| r.game_over == 0), "C++: never over");
    let w = drive(&c, &c.cfg, Some(&golden));
    assert!(
        w.respawns >= 2,
        "both worms spawn in-sim from the LocalController start"
    );
    assert_eq!(w.game_over_tick, None, "lives 15, no input: never over");
}

#[test]
fn killemall_matches_cpp() {
    let c = matrix_case("killemall");
    let s = &c.cfg.settings;
    assert_non_sim_fields("killemall", s);
    assert_eq!(
        (s.game_mode, s.lives, s.loading_time, s.blood),
        (GM_KILL_EM_ALL, 1, 37, 250)
    );
    assert!(!s.load_change && s.shadow);
    assert_eq!(
        (s.max_bonuses, s.blood_particle_max, s.time_to_lose),
        (6, 300, 600)
    );
    assert_loadouts(s, 150, [1, 12, 7, 22, 23], [15, 21, 16, 1, 12]);
    let golden = golden("killemall", &c);
    let w = drive(&c, &c.cfg, Some(&golden));
    assert_post_mortem("killemall", &golden, &w, 934);
    assert_eq!(
        w.peak_bobjects, 300,
        "the blood pool reaches its settings cap"
    );
    assert!(w.reload_started, "loading_time 37 is exercised");
    assert!(w.bonus_dropped, "max_bonuses 6 opens the drop roll");
    assert!(w.deaths >= 1);
    assert!(shadow_fired(&c, &w), "CorrectShadow changed material_id");
}

#[test]
fn scales_matches_cpp() {
    let c = matrix_case("scales");
    let s = &c.cfg.settings;
    assert_non_sim_fields("scales", s);
    assert_eq!(
        (s.game_mode, s.lives, s.loading_time, s.blood),
        (GM_SCALES_OF_JUSTICE, 2, 150, 60)
    );
    assert!(s.load_change && s.shadow);
    assert_eq!(
        (s.max_bonuses, s.blood_particle_max, s.time_to_lose),
        (8, 500, 600)
    );
    assert_loadouts(s, 40, [22, 16, 15, 23, 7], [12, 1, 21, 22, 16]);
    let golden = golden("scales", &c);
    let w = drive(&c, &c.cfg, Some(&golden));
    assert_post_mortem("scales", &golden, &w, 2282);
    assert!(
        w.scales_death_kept_health,
        "the Scales death branch (worm.cpp:384-388)"
    );
    assert!(w.life_gained, "the Scales overflow-to-lives rule");
    assert!(w.reload_started, "loading_time 150 is exercised");
    assert!(w.bonus_dropped, "max_bonuses 8 opens the drop roll");
    assert!(shadow_fired(&c, &w), "CorrectShadow changed material_id");
}

#[test]
fn gametag_matches_cpp() {
    let c = matrix_case("gametag");
    let s = &c.cfg.settings;
    assert_non_sim_fields("gametag", s);
    assert_eq!(
        (s.game_mode, s.time_to_lose, s.blood, s.loading_time),
        (GM_GAME_OF_TAG, 12, 0, 0)
    );
    assert!(s.load_change && !s.shadow);
    assert_eq!((s.lives, s.max_bonuses, s.blood_particle_max), (3, 3, 700));
    assert_loadouts(s, 80, [7, 1, 12, 16, 21], [23, 15, 22, 7, 1]);
    let golden = golden("gametag", &c);
    let w = drive(&c, &c.cfg, Some(&golden));
    assert_post_mortem("gametag", &golden, &w, 1610);
    assert!(w.timer_bumped, "the it-timer runs to time_to_lose");
    assert!(w.bonus_dropped, "max_bonuses 3 opens the drop roll");
    assert!(w.deaths >= 1);
}

#[test]
fn every_variant_is_internally_deterministic() {
    let cases = [
        ("defaults", defaults_case()),
        ("killemall", matrix_case("killemall")),
        ("scales", matrix_case("scales")),
        ("gametag", matrix_case("gametag")),
    ];
    for (name, c) in &cases {
        assert_eq!(
            drive(c, &c.cfg, None).masters,
            drive(c, &c.cfg, None).masters,
            "{name}"
        );
    }
}

/// Design §7.1: the tick-0 `scenario::load` path REFUSES a scenario carrying
/// `settings` — here the committed killemall scenario itself.
#[test]
#[should_panic(expected = "scenario::load refuses a `settings` scenario")]
fn scenario_load_refuses_a_committed_settings_scenario() {
    let s = Scenario::parse(&read("sim_slice4_5a_killemall_scenario.txt")).expect("parses");
    assert!(s.settings.is_some());
    let _ = scenario::load(Path::new(TC_ROOT), &s);
}
