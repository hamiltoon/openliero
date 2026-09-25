//! Step 4½c T7 — handoff equality (design §1 done-when 3, §6.6). Path one: `new_match` → weapon
//! selection → `finalize` → `enter_game`. Path two: `build_match` with the saved picks replaced
//! by the final picks, same seed. They must agree on every hash component but `rng`, on the whole
//! `WormState` (weapons, ammo, lives, `current_weapon`, control words, …) and the pool capacity,
//! and — once path two's RNG is advanced by the phase's draws — on the master hash. On the two
//! continuation cases and on a randomized run of configurations.

mod weapsel_common;

use std::path::Path;

use scenario::build::{build_match, enter_game, new_match, weapsel_config};
use scenario::settings::{MatchConfig, Settings};
use sim::hash::{hash_components, hash_game_state, ComponentHashes};
use sim::state::{ControlState, SimState};
use sim::weapsel::{WeaponSelection, DONE_ITEM, WEAPON_COUNT};
use sim_core::rng::Rand;
use weapsel_common as wc;

fn assert_handoff(label: &str, cfg: &MatchConfig, picks: [[u32; 5]; 2], one: &SimState) {
    let level = wc::load_level(wc::LEVEL);
    let mut cfg2 = cfg.clone();
    cfg2.settings.worm_settings[0].weapons = picks[0];
    cfg2.settings.worm_settings[1].weapons = picks[1];
    let mut two = build_match(Path::new(wc::TC_ROOT), &cfg2, &level)
        .unwrap_or_else(|e| panic!("{label}: {e}"))
        .state;
    let (a, b) = (hash_components(one), hash_components(&two));
    assert_eq!(
        ComponentHashes { rng: 0, ..a },
        ComponentHashes { rng: 0, ..b },
        "{label}: every component but rng"
    );
    assert_eq!(one.worms, two.worms, "{label}: the worms");
    assert_eq!(
        one.bobjects.capacity(),
        two.bobjects.capacity(),
        "{label}: pool"
    );
    for _ in 0..one.rand.draws() {
        two.rand.next_u32();
    }
    assert_eq!(
        hash_game_state(one),
        hash_game_state(&two),
        "{label}: master hash after advancing path two by {} draws",
        one.rand.draws()
    );
}

#[test]
fn the_two_continuation_cases_hand_off_like_build_match() {
    for name in ["match_humans", "match_bot"] {
        let scenario = wc::read_scenario(name);
        let settings = wc::read_settings(&scenario);
        let (run, mut one) = wc::drive(name, &scenario, &settings);
        let cfg = MatchConfig {
            settings,
            seed: scenario.seed,
        };
        enter_game(&mut one, &cfg);
        let last = run.steps.last().expect("at least one frame");
        assert_handoff(name, &cfg, [last.after[0].picks, last.after[1].picks], &one);
        assert!(one.rand.draws() > 0, "{name}: non-vacuous: the phase drew");
    }
}

/// `prefix` frames of random words, then every player not yet ready walks Down to DONE with
/// clean taps and fires. Returns when the phase ends.
fn drive_to_done(ws: &mut WeaponSelection, st: &mut SimState, rng: &mut Rand, prefix: u32) {
    let step = |ws: &mut WeaponSelection, st: &mut SimState, w: [u32; 2]| {
        ws.process_frame(
            st,
            &[ControlState::unpack(w[0]), ControlState::unpack(w[1])],
        )
    };
    for _ in 0..prefix {
        let w = [rng.next_u32() & 0x7f, rng.next_u32() & 0x7f];
        if step(ws, st, w) {
            return;
        }
    }
    for _ in 0..64 {
        if step(ws, st, [0, 0]) {
            return;
        }
        let w = [0, 1].map(|i| {
            let p = ws.player(i);
            if p.ready {
                0
            } else if p.cursor == DONE_ITEM {
                wc::FIRE
            } else {
                wc::DOWN
            }
        });
        if step(ws, st, w) {
            return;
        }
    }
    panic!("the finishing walk did not end the phase");
}

#[test]
fn a_randomized_run_of_configurations_hands_off_like_build_match() {
    let level = wc::load_level(wc::LEVEL);
    let mut rng = Rand::new();
    rng.seed(0x4c5c);
    let mut drew = 0;
    for case in 0..48 {
        let mut s = Settings::default();
        for v in s.weap_table.iter_mut() {
            *v = if rng.bound(2) == 0 {
                0
            } else {
                1 + rng.bound(2)
            };
        }
        let keep = rng.bound(WEAPON_COUNT as u32) as usize;
        s.weap_table[keep] = 0; // at least one enabled
        for i in 0..2 {
            s.worm_settings[i].weapons = std::array::from_fn(|_| {
                if rng.bound(5) == 0 {
                    0
                } else {
                    rng.bound_range(1, 41)
                }
            });
            s.worm_settings[i].controller = rng.bound(2);
        }
        s.select_bot_weapons = rng.bound(4);
        let cfg = MatchConfig {
            settings: s,
            seed: rng.next_u32(),
        };
        let mut one = new_match(Path::new(wc::TC_ROOT), &cfg, &level)
            .unwrap()
            .state;
        let mut ws = WeaponSelection::new(&mut one, &weapsel_config(&cfg.settings)).unwrap();
        let prefix = rng.bound(40);
        drive_to_done(&mut ws, &mut one, &mut rng, prefix);
        let picks = ws.finalize(&mut one);
        enter_game(&mut one, &cfg);
        drew += (one.rand.draws() > 0) as u32;
        assert_handoff(&format!("random config {case}"), &cfg, picks, &one);
    }
    assert!(
        drew >= 24,
        "non-vacuous: most random configurations drew ({drew}/48)"
    );
}
