//! Step 4½a-1 T6 — the `MatchConfig` builder reproduces an EXISTING C++ golden
//! (design §7.4). `sim_slice6_fuzz5` was produced by the classic dumper path with both
//! worms seeded dead at (0,0), health 100, lives 50, max_bonuses 4, loading_time 0,
//! shadow false and every other C++ default — which is exactly a `MatchConfig`. So
//! `build_match` with those settings must drive the committed 1501-row golden
//! bit-for-bit on all 11 columns, before any dumper change.

use std::path::Path;

use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::{MatchConfig, Settings};
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{GOLDEN}/{name}"))
        .unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// `<tick> <master> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`.
fn parse_golden(text: &str) -> Vec<(u32, [u32; 10])> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(cols.len(), 11, "11-column golden line");
            let mut h = [0u32; 10];
            for (i, c) in cols[1..].iter().enumerate() {
                h[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            (cols[0].parse().expect("tick"), h)
        })
        .collect()
}

fn check(state: &SimState, tick: u32, want: &[u32; 10]) {
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
            got[i], want[i],
            "tick {tick}: {}: got {:08x} want {:08x}",
            names[i], got[i], want[i]
        );
    }
}

#[test]
fn build_match_reproduces_sim_slice6_fuzz5() {
    let scenario = Scenario::parse(&read("sim_slice6_fuzz5_scenario.txt")).expect("parses");
    assert_eq!(
        (scenario.seed, scenario.max_bonuses, scenario.game_mode),
        (43, 4, 0)
    );
    for w in &scenario.worms {
        assert_eq!(
            (w.pos_x, w.pos_y, w.health, w.lives, w.visible),
            (0, 0, 100, 50, false)
        );
    }
    let mut settings = Settings::default();
    settings.lives = 50;
    settings.max_bonuses = 4;
    settings.loading_time = 0; // the classic dumper path forces 0
    settings.shadow = false; // the classic dumper path forces false
    let cfg = MatchConfig {
        settings,
        seed: scenario.seed,
    };
    let bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level)).expect("read level");
    let level = assets::level::load(&bytes).expect("level loads");
    let mut state = build_match(Path::new(TC_ROOT), &cfg, &level)
        .expect("builds")
        .state;

    let golden = parse_golden(&read("sim_slice6_fuzz5.txt"));
    assert_eq!(golden.len(), 1501);
    check(&state, 0, &golden[0].1);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        check(&state, k, &golden[k as usize].1);
    }
}
