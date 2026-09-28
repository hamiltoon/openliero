//! Step 4½c T7 — the two CONTINUATION goldens (design §1 done-when 2, §6.5): a real weapon
//! selection (`weapsel` lines) in front of 600 fuzz ticks, 12 columns bit-exact vs
//! oracle_dump_sim_physics's settings path. The tick-0 rng is the post-selection `rand.last`.

mod weapsel_common;

use scenario::build::enter_game;
use scenario::settings::MatchConfig;
use sim::state::ControlState;
use weapsel_common as wc;

fn continuation(name: &str) {
    let case = wc::case(name);
    let scenario = wc::read_scenario(name);
    let settings = wc::read_settings(&scenario);
    let (run, mut st) = wc::drive(name, &scenario, &settings);
    enter_game(
        &mut st,
        &MatchConfig {
            settings,
            seed: scenario.seed,
        },
    );
    assert_ne!(
        st.worms[0].weapons.map(|w| w.ty),
        st.worms[1].weapons.map(|w| w.ty),
        "{name}: per-worm picks"
    );
    let rows = wc::parse_golden12(&wc::read(&format!("sim_slice4_5c_{name}.txt")));
    assert_eq!(rows.len() as u32, case.ticks + 1, "{name}: rows 0..=ticks");
    assert_ne!(
        rows[0].hashes[1], 0,
        "{name}: a selection draw reached tick 0"
    );
    assert_eq!(
        rows[0].hashes[1], run.final_last,
        "{name}: tick-0 rng = post-selection last"
    );
    wc::check12(&st, &rows[0]);
    for k in 1..=scenario.ticks {
        st.process_frame(&[
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ]);
        wc::check12(&st, &rows[k as usize]);
    }
}

#[test]
fn match_humans_continues_bit_exact() {
    continuation("match_humans");
}

#[test]
fn match_bot_continues_bit_exact() {
    continuation("match_bot");
}
