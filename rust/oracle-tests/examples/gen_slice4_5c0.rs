//! Step 4½c-0 T10/T11 — setup-sidecar writer, seed scanner and scenario writer for the
//! weapon-branch goldens (design §5). A dev tool, not a test; not run in CI.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- cfg  <variant> <out_setup.cfg>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- scan <variant> <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- gen  <variant> <game_seed> <out_scenario.txt>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- steer-scan <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- steer-gen  <game_seed> <out_scenario.txt>
//!
//! Variants: laser, missile, trails, booby (input seeds fixed in the shared table). Run in
//! a DEBUG build (overflow checks). After 4½c-0 no weapon branch is a tripwire, so a PANIC
//! line from a scan is a real bug — investigate it, never skip it.

#[path = "../tests/sim_slice4_5c0_common/mod.rs"]
mod c0;

use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};
use std::path::Path;

use assets::level::LevelData;
use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::MatchConfig;
use scenario::settings_toml::settings_from_toml;
use sim::state::ControlState;

/// The steer render golden (design §5.4): no POWERLEVEL palette on this level, so the
/// `scenario::load` render path shows the same palette as C++.
const STEER_LEVEL: &str = "Levels/physics_fall_test.lev";
const STEER_TICKS: u32 = 500;
const STEER_INPUT_SEED: u32 = 5151;

fn load_level(rel: &str) -> LevelData {
    assets::level::load(&std::fs::read(format!("{}/{rel}", c0::TC_ROOT)).unwrap()).unwrap()
}

/// Drive a settings variant through the REAL Rust readers + builder; `None` = it panicked.
fn run(v: &c0::Variant, game_seed: u32) -> Option<c0::Ledger> {
    let o = c0::load_objects();
    let ids = c0::Ids::new(&o);
    let level = load_level(c0::LEVEL);
    let settings = settings_from_toml(&c0::setup_cfg(v, &o)).expect("sidecar parses");
    let cfg = MatchConfig {
        settings,
        seed: game_seed,
    };
    let ins = c0::inputs(v.input_seed, v.ticks);
    catch_unwind(AssertUnwindSafe(|| {
        let mut st = build_match(Path::new(c0::TC_ROOT), &cfg, &level)
            .expect("variant config builds")
            .state;
        let mut l = c0::Ledger::default();
        for w in &ins {
            let pre = c0::Pre::capture(&st);
            st.process_frame(&[ControlState::unpack(w[0]), ControlState::unpack(w[1])]);
            l.observe(&ids, &o, &pre, &st, *w);
        }
        l
    }))
    .ok()
}

fn steer_scenario(game_seed: u32, header: &str) -> String {
    let mut out = String::from(header);
    out.push_str(&format!(
        "seed {game_seed}\nlevel {STEER_LEVEL}\nticks {STEER_TICKS}\nmax_bonuses 0\n"
    ));
    out.push_str(
        "# worm <index> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>\n",
    );
    out.push_str("worm 0 0 0 100 10 0   0\nworm 1 0 0 100 10 218 0\n");
    out.push_str("weapon 0 MISSILE\nrender player\nrender_live\n");
    out.push_str("# input <tick> <worm0_7bit> <worm1_7bit>  (Up=1 Down=2 Left=4 Right=8 Fire=16 Change=32 Jump=64)\n");
    for (t, w) in c0::steer_inputs(STEER_INPUT_SEED, STEER_TICKS)
        .iter()
        .enumerate()
    {
        if w[0] != 0 || w[1] != 0 {
            out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
        }
    }
    out
}

/// Drive the steer scenario through `scenario::load` (the path the render test uses).
fn steer_run(game_seed: u32) -> Option<c0::SteerLedger> {
    let s = Scenario::parse(&steer_scenario(game_seed, "")).expect("steer scenario parses");
    let missile = c0::weapon_index(&c0::load_objects(), "MISSILE");
    catch_unwind(AssertUnwindSafe(|| {
        let mut st = scenario::load(Path::new(c0::TC_ROOT), &s).state;
        let mut l = c0::SteerLedger::default();
        for k in 1..=s.ticks {
            let input = [s.input(k - 1, 0), s.input(k - 1, 1)];
            let pre = c0::Pre::capture(&st);
            st.process_frame(&[
                ControlState::unpack(input[0]),
                ControlState::unpack(input[1]),
            ]);
            l.observe(k, missile, &pre, &st, input);
        }
        l
    }))
    .ok()
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
            let v = c0::variant(arg(1));
            std::fs::write(arg(2), c0::setup_cfg(v, &c0::load_objects())).expect("write setup");
            println!("wrote {}", arg(2));
        }
        "scan" => {
            let v = c0::variant(arg(1));
            set_hook(Box::new(|_| {})); // a panic is reported below
            for game_seed in num(2)..=num(3) {
                match run(v, game_seed) {
                    None => println!("{} game_seed={game_seed} PANIC — a real bug", v.name),
                    Some(l) => println!(
                        "{} game_seed={game_seed} ok={} {}",
                        v.name,
                        c0::ok(v, &l),
                        l.summary()
                    ),
                }
            }
        }
        "gen" => {
            let v = c0::variant(arg(1));
            let game_seed = num(2);
            let l = run(v, game_seed).expect("the seed must not panic");
            assert!(c0::ok(v, &l), "seed fails the witnesses: {}", l.summary());
            let mut out = format!(
                "# Step 4½ slice 4½c-0 T10 — weapon-branch match `{name}` (design §5.2). Read by\n\
                 # BOTH the C++ dumper (oracle_dump_sim_physics `settings` path: the real\n\
                 # Settings::FromToml + the LocalController start) and the Rust milestone test\n\
                 # (settings_toml + scenario::build::build_match). Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5c0 -- gen {name} {game_seed} <this file>\n\
                 # LEDGER (Rust, driven state): {summary}\n\
                 # Inputs: per tick Rand({input_seed}).next_u32() & 0x7f, worm 0 then worm 1 (zero pairs omitted).\n\
                 seed {game_seed}\nlevel {level}\nticks {ticks}\nsettings sim_slice4_5c0_{name}_setup.cfg\n",
                name = v.name,
                summary = l.summary(),
                input_seed = v.input_seed,
                level = c0::LEVEL,
                ticks = v.ticks,
            );
            for (t, w) in c0::inputs(v.input_seed, v.ticks).iter().enumerate() {
                if w[0] != 0 || w[1] != 0 {
                    out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
                }
            }
            std::fs::write(arg(3), out).expect("write scenario");
            println!("wrote {} — {}", arg(3), l.summary());
        }
        "steer-scan" => {
            set_hook(Box::new(|_| {}));
            for game_seed in num(1)..=num(2) {
                match steer_run(game_seed) {
                    None => println!("steer game_seed={game_seed} PANIC — a real bug"),
                    Some(l) => println!(
                        "steer game_seed={game_seed} ok={} camera_ticks={} off_worm={} left={} right={} boosted={}",
                        c0::steer_ok(&l),
                        l.camera_ticks.len(),
                        l.off_worm_ticks.len(),
                        l.left,
                        l.right,
                        l.boosted
                    ),
                }
            }
        }
        "steer-gen" => {
            let game_seed = num(1);
            let l = steer_run(game_seed).expect("the seed must not panic");
            assert!(c0::steer_ok(&l), "seed fails the steer witnesses: {l:?}");
            let header = format!(
                "# Step 4½ slice 4½c-0 T11 — the STEERABLE CAMERA, LIVE (design §5.4). Read by BOTH\n\
                 # the C++ dumper (oracle_dump_sim_physics `render_live`: real Game::ProcessFrame —\n\
                 # inputs first, so the MISSILE Up boost reads this tick's input — and the real\n\
                 # ProcessViewports) and the Rust render test (the 4d live harness). Both worms start\n\
                 # dead at (0,0) and respawn in-sim (killed_timer <= 0: the camera's alive arm), carry\n\
                 # MISSILE in slot 0 and never press Change. Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5c0 -- steer-gen {game_seed} <this file>\n\
                 # LEDGER (Rust): {} camera ticks, {} off the worm (first t{}), left={} right={} boosted={}\n\
                 # Inputs: per tick v = Rand({STEER_INPUT_SEED}).next_u32(); (v & 0x4f) | Fire iff (v >> 8) & 7 == 0.\n",
                l.camera_ticks.len(),
                l.off_worm_ticks.len(),
                l.off_worm_ticks[0].0,
                l.left,
                l.right,
                l.boosted,
            );
            std::fs::write(arg(2), steer_scenario(game_seed, &header)).expect("write scenario");
            println!("wrote {}", arg(2));
        }
        other => panic!("unknown command {other:?} (cfg | scan | gen | steer-scan | steer-gen)"),
    }
}
