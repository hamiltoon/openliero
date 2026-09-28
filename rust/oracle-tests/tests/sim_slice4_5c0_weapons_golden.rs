//! Step 4½c-0 T12 — MILESTONE (design §5.2-§5.3): the four weapon-branch matches
//! bit-exact vs C++.
//!
//! Each committed scenario names a lean C++-schema setup (`settings <file>`). The C++
//! dumper read it with the real `Settings::FromToml`; here the SAME files go through
//! `Scenario::parse`, `settings_toml::settings_from_toml` and `build_match`. Every row is
//! asserted on the 11 hash columns (components first, master last) and on column 12
//! (`IsGameOver`, constant 0 — lives 99). The shared ledger then re-derives every reach
//! witness from the driven state, so a golden that passed without reaching its branches
//! fails here.

mod sim_slice4_5c0_common;

use std::path::Path;

use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::MatchConfig;
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};
use sim_slice4_5c0_common as c0;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

fn read(rel: &str) -> String {
    let path = Path::new(GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

struct Row {
    tick: u32,
    hashes: [u32; 10], // master, rng, level, worm0, worm1, bob, bon, sob, nob, wob
    game_over: u32,
}

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

fn case(name: &str) -> Case {
    let scenario = Scenario::parse(&read(&format!("sim_slice4_5c0_{name}_scenario.txt")))
        .unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"));
    let rel = scenario
        .settings
        .clone()
        .expect("a settings-driven scenario");
    assert_eq!(rel, format!("sim_slice4_5c0_{name}_setup.cfg"));
    assert!(scenario.worms.is_empty(), "worms come from the setup");
    let settings = settings_from_toml(&read(&rel))
        .unwrap_or_else(|e| panic!("{name}: setup {rel} parses: {e:?}"));
    Case {
        cfg: MatchConfig {
            settings,
            seed: scenario.seed,
        },
        scenario,
    }
}

/// Intent guard: the committed sidecar carries the variant's settings (design §5.2, §6).
fn assert_sidecar(name: &str, c: &Case, o: &assets::object::Objects) {
    let v = c0::variant(name);
    let s = &c.cfg.settings;
    assert_eq!(
        s.weap_table, [0u32; 40],
        "{name}: every weapon may drop (the ban lift)"
    );
    assert_eq!(
        (s.game_mode, s.lives, s.loading_time, s.max_bonuses, s.blood),
        (0, c0::LIVES, c0::LOADING_TIME, c0::MAX_BONUSES, c0::BLOOD),
        "{name}: sim settings"
    );
    assert_eq!(s.blood_particle_max, c0::BLOOD_PARTICLE_MAX);
    assert!(s.shadow && s.load_change, "{name}: shadow + loadChange");
    let ws = &s.worm_settings;
    assert_eq!((ws[0].health, ws[1].health), (c0::HEALTH, c0::HEALTH));
    assert_eq!(
        ws[0].weapons,
        v.p1.map(|n| c0::menu_index(o, n)),
        "{name}: player 1"
    );
    assert_eq!(
        ws[1].weapons,
        v.p2.map(|n| c0::menu_index(o, n)),
        "{name}: player 2"
    );
    assert_eq!(c.scenario.ticks, v.ticks, "{name}: ticks");
    assert_eq!(c.scenario.level, c0::LEVEL, "{name}: level");
}

/// Drive the committed case; with `golden`, assert every row. Returns the ledger and
/// the master series.
fn drive(c: &Case, golden: Option<&[Row]>) -> (c0::Ledger, Vec<u32>) {
    let o = c0::load_objects();
    let ids = c0::Ids::new(&o);
    let bytes = std::fs::read(format!("{}/{}", c0::TC_ROOT, c.scenario.level)).expect("level");
    let level = assets::level::load(&bytes).expect("level loads");
    let mut st = build_match(Path::new(c0::TC_ROOT), &c.cfg, &level)
        .expect("builds")
        .state;
    if let Some(g) = golden {
        assert_eq!(
            g.len() as u32,
            c.scenario.ticks + 1,
            "golden rows 0..=ticks"
        );
        check(&st, &g[0]);
    }
    let mut l = c0::Ledger::default();
    let mut masters = vec![hash_game_state(&st)];
    for k in 1..=c.scenario.ticks {
        let input = [c.scenario.input(k - 1, 0), c.scenario.input(k - 1, 1)];
        let pre = c0::Pre::capture(&st);
        st.process_frame(&[
            ControlState::unpack(input[0]),
            ControlState::unpack(input[1]),
        ]);
        if let Some(g) = golden {
            check(&st, &g[k as usize]);
        }
        l.observe(&ids, &o, &pre, &st, input);
        masters.push(hash_game_state(&st));
    }
    (l, masters)
}

fn matches_cpp(name: &str) -> c0::Ledger {
    let c = case(name);
    assert_sidecar(name, &c, &c0::load_objects());
    let g = parse_golden(&read(&format!("sim_slice4_5c0_{name}.txt")));
    assert!(
        g.iter().all(|r| r.game_over == 0),
        "{name}: C++ never over (lives 99)"
    );
    let (l, _) = drive(&c, Some(&g));
    assert!(
        c0::ok(c0::variant(name), &l),
        "{name}: reach witnesses — {}",
        l.summary()
    );
    l
}

#[test]
fn laser_matches_cpp() {
    // RIFLE / WINCHESTER / GAUSS GUN survive ticks with 8 steps of gravity; a LASER blast
    // lands >= 10 px from every LASER that entered its tick; a beam is removed by a hit.
    matches_cpp("laser");
}

#[test]
fn missile_matches_cpp() {
    // ProcessSteerables turns missiles both ways on both cycle parities; the Up boost is
    // read in the object loop from this tick's input.
    matches_cpp("missile");
}

#[test]
fn trails_matches_cpp() {
    // Three particle trails, the napalm / nuke / hellraider leave_obj trails, MINI NUKE's
    // Create1 splinters, and a formerly-deferred weapon offered by a bonus.
    matches_cpp("trails");
}

#[test]
fn booby_matches_cpp() {
    // A BOOBY TRAP goes off away from every worm with its timer running: a chain.
    matches_cpp("booby");
}

#[test]
fn every_variant_is_internally_deterministic() {
    for name in ["laser", "missile", "trails", "booby"] {
        let c = case(name);
        assert_eq!(drive(&c, None).1, drive(&c, None).1, "{name}");
    }
}

/// Non-vacuity of the comparison itself: one flipped master bit in row 1 must fail.
#[test]
#[should_panic(expected = "tick 1: master")]
fn a_perturbed_golden_row_fails() {
    let c = case("laser");
    let mut g = parse_golden(&read("sim_slice4_5c0_laser.txt"));
    g[1].hashes[0] ^= 1;
    drive(&c, Some(&g));
}
