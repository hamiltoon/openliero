//! Step 4½c-0 T12 — the STEERABLE CAMERA golden, LIVE (design §5.4).
//!
//! `render_slice4_5c0_steer_scenario.txt` is a generated MISSILE fuzz on
//! physics_fall_test.lev: both worms start dead, respawn in-sim, carry MISSILE in slot 0
//! and never press Change. The C++ `render_live` path drives the REAL Game::ProcessFrame
//! (inputs first, so the Up boost reads this tick's input) and the real ProcessViewports,
//! whose alive arm centres a steering worm's viewport on its missiles' pixel centroid
//! (viewport.cpp:30-32). [`run_stem`] gates frame + state + isolation bit-exact; this file
//! adds the non-vacuity: the witnesses hold on the driven state, and on an off-worm
//! steering tick the centering-only camera sits on the missile centroid.

mod render_slice4d_common;
mod sim_slice4_5c0_common;

use std::path::Path;

use oracle_tests::scenario::Scenario;
use render::viewport::Viewport;
use render_slice4d_common::{modified_stem, read_golden_stem, run_stem};
use sim::state::ControlState;
use sim_core::fixed::ftoi;
use sim_slice4_5c0_common as c0;

const STEM: &str = "render_slice4_5c0_steer";

#[derive(Clone, Copy, Default)]
struct Snap {
    count: i32,
    sum_x: i32,
    sum_y: i32,
    x: i32,
    y: i32,
}

/// Drive the committed scenario sim-only (`scenario::load` + `process_frame`), collecting
/// the steer ledger and per-tick `[worm0, worm1]` snapshots (index == tick) + level size.
fn drive() -> (c0::SteerLedger, Vec<[Snap; 2]>, (i32, i32)) {
    let s = Scenario::parse(&read_golden_stem(STEM, "_scenario.txt")).expect("parses");
    let missile = c0::weapon_index(&c0::load_objects(), "MISSILE");
    let mut st = scenario::load(Path::new(c0::TC_ROOT), &s).state;
    let dims = (st.level.width, st.level.height);
    let mut l = c0::SteerLedger::default();
    let mut snaps = vec![[Snap::default(); 2]];
    for k in 1..=s.ticks {
        let input = [s.input(k - 1, 0), s.input(k - 1, 1)];
        let pre = c0::Pre::capture(&st);
        st.process_frame(&[
            ControlState::unpack(input[0]),
            ControlState::unpack(input[1]),
        ]);
        l.observe(k, missile, &pre, &st, input);
        snaps.push([0usize, 1].map(|i| {
            let w = &st.worms[i];
            Snap {
                count: w.steerable_count,
                sum_x: w.steerable_sum_x,
                sum_y: w.steerable_sum_y,
                x: ftoi(w.pos.x),
                y: ftoi(w.pos.y),
            }
        }));
    }
    (l, snaps, dims)
}

#[test]
fn steerable_camera_matches_cpp_and_sits_on_the_missile_centroid() {
    // The bit-exact gate: every tick's frame hash == C++, state hash == hash_game_state ==
    // the sim golden master, plus the folded total and the row count.
    let r = run_stem(STEM);
    let (l, snaps, (level_w, level_h)) = drive();
    assert!(
        c0::steer_ok(&l),
        "the scenario reaches the steering witnesses: {l:?}"
    );
    assert_eq!(snaps.len() as u32, r.ticks + 1);

    // Non-vacuity: on an off-worm steering tick whose centroid camera differs from the
    // worm-centred one, the centering-only (shake-suppressed) re-render sits on the centroid.
    let layout = Viewport::player_layout();
    let mut proven = false;
    for &(k, i) in &l.off_worm_ticks {
        let s = snaps[k as usize][i];
        let vp = &layout[i];
        let cam = |cx: i32, cy: i32| {
            (
                (cx - vp.center_x).clamp(0, level_w - vp.rect.width()),
                (cy - vp.center_y).clamp(0, level_h - vp.rect.height()),
            )
        };
        let on_missiles = cam(s.sum_x / s.count, s.sum_y / s.count);
        if on_missiles == cam(s.x, s.y) {
            continue;
        }
        let (_, cams) = modified_stem(STEM, k, None, true, false);
        assert_eq!(
            cams[i], on_missiles,
            "tick {k} vp{i}: SetCenter(sum / count)"
        );
        proven = true;
        break;
    }
    assert!(
        proven,
        "some steering tick moves the camera off the worm onto its missiles"
    );
}
