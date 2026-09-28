//! Step 4½f-1 T6 — G-AI + G-HP: the CPU player's per-tick control words and its RNG, and
//! unequal-health matches, bit-exact vs the real C++ (plan T6; design §8).
//!
//! Each committed scenario carries `generate <level_seed>` and a `settings <file>` sidecar; the
//! `ai_*` cases also carry `ai` (and `ai_vs_ai` a `weapsel` frame). The C++ dumper built the level
//! with the REAL `Level::GenerateFromSettings`, read the sidecar with the real
//! `Settings::FromToml`, started the worms in the C++ LocalController state, gave every
//! controller-1 player a REAL `DumbLieroAI` (with its `reacts` zeroed, plan D5) and ran the AIs in
//! `LocalController::Process`'s order right before each tick. Here the SAME committed files go
//! through the real Rust readers (`Scenario::parse`, `settings_from_toml`,
//! `generate_from_settings`, `build_match` or 4½c's selection route) and, per tick:
//!
//! 1. a human's word is its `input`, a CPU's the word its worm's last tick left
//!    (`sim.worms[c].control_states`, plan fact 14);
//! 2. `sim::ai::run_ais` runs the CPUs in `(i + cycles % 2) % 2` order — columns 13-16 of the
//!    next row are each CPU's word right after it (`%02x`) and its AI's `rand.last` (`%08x`);
//! 3. `process_frame`; the 11 hash columns and `IsGameOver` (column 12).
//!
//! The first differing tick and column panics. The witness guard re-derives every `# LEDGER`
//! line of the committed headers from the driven Rust state and asserts each case's witnesses
//! (plan T6 Step 1's table). `ai_vs_human` is Hard gate 4's human-vs-CPU golden.

mod sim_slice4_5f_common;

use std::path::Path;

use scenario::settings::{Settings, GM_GAME_OF_TAG, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE};
use scenario::settings_toml::settings_from_toml;
use scenario::Scenario;
use sim_slice4_5f_common as common;

fn read(rel: &str) -> String {
    let path = Path::new(common::GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A committed case: its scenario, sidecar settings and the gate's inputs.
struct Committed {
    case: &'static common::Case,
    text: String,
    scenario: Scenario,
    settings: Settings,
}

fn committed(name: &str) -> Committed {
    let case = common::case(name);
    let text = read(&format!("sim_slice4_5f_{name}_scenario.txt"));
    let scenario =
        Scenario::parse(&text).unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"));
    let rel = format!("sim_slice4_5f_{name}_setup.cfg");
    assert_eq!(
        scenario.settings.as_deref(),
        Some(rel.as_str()),
        "{name}: the sidecar"
    );
    assert_eq!(
        scenario.generate(),
        Some(case.level_seed),
        "{name}: generate"
    );
    assert_eq!(scenario.level, "", "{name}: no level path");
    assert!(
        scenario.worms.is_empty(),
        "{name}: worms come from the setup"
    );
    assert_eq!(scenario.ai(), case.ai(), "{name}: the ai directive");
    let settings =
        settings_from_toml(&read(&rel)).unwrap_or_else(|e| panic!("{name}: {rel}: {e:?}"));
    scenario
        .check_ai_players(&settings)
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    Committed {
        case,
        text,
        scenario,
        settings,
    }
}

/// Drive the committed case; with `golden`, every row is checked.
fn run(c: &Committed, golden: bool) -> common::Ledger {
    run_edited(c, golden, |_| {})
}

/// [`run`], with `edit` applied to the parsed golden rows first (the negative controls).
fn run_edited(
    c: &Committed,
    golden: bool,
    edit: impl FnOnce(&mut Vec<common::Row>),
) -> common::Ledger {
    let s = &c.settings;
    assert_eq!(
        (s.random_map_width, s.random_map_height),
        (c.case.width, c.case.height),
        "{}: map size",
        c.case.name
    );
    let level = common::generate(s, c.case.level_seed);
    let weapsel: Vec<[u32; 2]> = match c.scenario.weapsel_end() {
        None => Vec::new(),
        Some(end) => (0..=end)
            .map(|f| {
                [
                    c.scenario.weapsel_input(f, 0),
                    c.scenario.weapsel_input(f, 1),
                ]
            })
            .collect(),
    };
    let (st, ais) = common::start(s, c.scenario.seed, &level, c.scenario.ai(), &weapsel);
    let ticks = c.scenario.ticks;
    let ins: Vec<[u32; 2]> = (0..ticks)
        .map(|t| [c.scenario.input(t, 0), c.scenario.input(t, 1)])
        .collect();
    let rows = golden.then(|| {
        let mut g = common::parse_golden(&read(&format!("sim_slice4_5f_{}.txt", c.case.name)));
        assert_eq!(g.len() as u32, ticks + 1, "{}: rows 0..=ticks", c.case.name);
        assert!(
            g.iter().all(|r| r.ai.is_some() == c.case.ai()),
            "{}: 16 columns exactly with ai",
            c.case.name
        );
        edit(&mut g);
        g
    });
    let l = common::drive(st, ais, &ins, ticks, false, rows.as_deref(), |_| {});
    assert_eq!(l.rows, ticks, "{}: every row driven", c.case.name);
    l
}

/// The witness guard: the committed `# LEDGER` lines are exactly what the driven Rust state
/// gives, and the case's witnesses hold.
fn assert_ledger(c: &Committed, l: &common::Ledger) {
    let committed: Vec<&str> = c
        .text
        .lines()
        .filter(|x| x.starts_with("# LEDGER"))
        .collect();
    let derived = common::ledger_lines(c.case, l);
    assert_eq!(committed, derived, "{}: the ledger re-derives", c.case.name);
    common::witnesses(c.case, l).unwrap_or_else(|e| panic!("{}: witness: {e}", c.case.name));
}

fn gate(name: &str) -> (Committed, common::Ledger) {
    let c = committed(name);
    let l = run(&c, true);
    assert_ledger(&c, &l);
    (c, l)
}

#[test]
fn ai_idle_matches_cpp() {
    let (c, l) = gate("ai_idle");
    let s = &c.settings;
    assert_eq!((s.game_mode, s.lives), (GM_KILL_EM_ALL, 3));
    assert_eq!(l.cpu, [false, true], "P2 the CPU");
    assert!(
        (0..c.scenario.ticks).all(|t| c.scenario.input(t, 0) == 0),
        "P1 idle"
    );
    assert_eq!(l.runs[1], c.scenario.ticks, "the AI runs on every tick");
}

/// Hard gate 4: a human against the CPU, to game over.
#[test]
fn ai_vs_human_matches_cpp() {
    let (c, l) = gate("ai_vs_human");
    assert_eq!(
        (c.settings.game_mode, c.settings.lives),
        (GM_KILL_EM_ALL, 3)
    );
    assert_eq!(l.cpu, [false, true]);
    assert!(l.game_over_tick.is_some());
}

#[test]
fn ai_vs_ai_matches_cpp() {
    let (c, l) = gate("ai_vs_ai");
    let s = &c.settings;
    assert_eq!((s.select_bot_weapons, s.lives), (0, 2), "RANDOM, lives 2");
    assert_eq!(c.scenario.weapsel_end(), Some(0), "one weapsel frame");
    assert_eq!(l.cpu, [true, true]);
    assert_ne!(l.tick0_rng, 0, "the RANDOM constructor drew");
    // Two fresh AIs make the same three draws on the first (invisible) tick.
    assert_eq!(l.tick1_last, [Some(0x2af0_9813), Some(0x2af0_9813)]);
    assert!(l.first_differ.is_some());
}

#[test]
fn ai_weapons_matches_cpp() {
    let (c, l) = gate("ai_weapons");
    let o = common::load_objects(&common::load_tc());
    let want = common::AI_WEAPONS_PICKS.map(|n| common::menu_index(&o, n));
    assert_eq!(c.settings.worm_settings[1].weapons, want, "the CPU's picks");
    assert!(l.explo > 0 && l.speed > 0 && l.floored > 0);
}

#[test]
fn ai_close_matches_cpp() {
    let (c, l) = gate("ai_close");
    assert_eq!((c.case.width, c.case.height), (333, 360));
    assert!(l.fallback_drew() > 0 && l.fallback_quiet() > 0 && l.rope > 0);
}

#[test]
fn hp_killemall_matches_cpp() {
    let (c, l) = gate("hp_killemall");
    let s = &c.settings;
    assert_eq!(
        (
            s.game_mode,
            s.max_bonuses,
            s.worm_settings[0].health,
            s.worm_settings[1].health
        ),
        (GM_KILL_EM_ALL, 10, 50, 300)
    );
    assert_eq!(l.max, [50, 300], "per-worm max_health");
}

#[test]
fn hp_scales_matches_cpp() {
    let (c, l) = gate("hp_scales");
    let s = &c.settings;
    assert_eq!(
        (
            s.game_mode,
            s.lives,
            s.worm_settings[0].health,
            s.worm_settings[1].health
        ),
        (GM_SCALES_OF_JUSTICE, 5, 30, 200)
    );
    assert_eq!(l.max, [30, 200]);
}

#[test]
fn hp_tag_matches_cpp() {
    let (c, l) = gate("hp_tag");
    let s = &c.settings;
    assert_eq!(
        (
            s.game_mode,
            s.time_to_lose,
            s.worm_settings[0].health,
            s.worm_settings[1].health
        ),
        (GM_GAME_OF_TAG, 60, 1000, 10)
    );
    assert_eq!(l.max, [1000, 10]);
}

/// The committed sidecars are what the generator's case table writes.
#[test]
fn the_sidecars_are_the_case_table() {
    let o = common::load_objects(&common::load_tc());
    for case in &common::CASES {
        assert_eq!(
            read(&format!("sim_slice4_5f_{}_setup.cfg", case.name)),
            common::setup_cfg(case, &o),
            "{}",
            case.name
        );
    }
}

/// The gate sees the AI: the same `ai_vs_human` drive with the CPU's AI switched off (its
/// word stays 0) mismatches the committed golden.
#[test]
#[should_panic(expected = "AI columns of worm 1")]
fn ai_vs_human_without_its_ai_diverges() {
    let c = committed("ai_vs_human");
    let level = common::generate(&c.settings, c.case.level_seed);
    let (st, _) = common::start(&c.settings, c.scenario.seed, &level, true, &[]);
    let g = common::parse_golden(&read("sim_slice4_5f_ai_vs_human.txt"));
    let ins: Vec<[u32; 2]> = (0..c.scenario.ticks)
        .map(|t| [c.scenario.input(t, 0), c.scenario.input(t, 1)])
        .collect();
    common::drive(
        st,
        [None, None],
        &ins,
        c.scenario.ticks,
        false,
        Some(&g),
        |_| {},
    );
}

/// The gate reads every hash column on every row: one worm-1 bit flipped in the parsed
/// `hp_killemall` golden at a mid tick is reported at that tick and column.
#[test]
#[should_panic(expected = "tick 1500: worm1 (col 6)")]
fn a_flipped_hash_cell_is_caught() {
    let c = committed("hp_killemall");
    run_edited(&c, true, |g| g[1500].hashes[4] ^= 1);
}

/// The gate reads the AI's RNG column on every row: one bit of the CPU's `rand.last`
/// flipped in the parsed `ai_close` golden at a mid tick is reported at that tick.
#[test]
#[should_panic(expected = "tick 1234: AI columns of worm 1")]
fn a_flipped_ai_rng_cell_is_caught() {
    let c = committed("ai_close");
    run_edited(&c, true, |g| {
        let ai = g[1234].ai.as_mut().expect("16 columns");
        ai[1].as_mut().expect("the CPU's columns").1 ^= 1;
    });
}
