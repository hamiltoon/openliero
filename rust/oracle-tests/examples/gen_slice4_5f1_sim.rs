//! Step 4½f-1 T6 — G-AI + G-HP: setup-sidecar writer, seed scanner and scenario writer for the
//! eight generated-level sim goldens (plan T6). A dev tool, not a test; not run in CI.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5f1_sim -- cfg  <case> <out_setup.cfg>
//!   cargo run -p oracle-tests --example gen_slice4_5f1_sim -- scan <case> <input_seed> <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5f1_sim -- gen  <case> <game_seed> <input_seed> <out_scenario.txt>
//!
//! Cases (`tests/sim_slice4_5f_common/mod.rs`, `CASES`): `ai_idle`, `ai_vs_human` (Hard gate 4's
//! human-vs-CPU golden), `ai_vs_ai`, `ai_weapons`, `ai_close` (G-AI: the `ai` directive, the
//! CPU's per-tick word and its RNG in columns 13-16) and `hp_killemall`, `hp_scales`, `hp_tag`
//! (G-HP: unequal healths on all 12 columns). The level is `generate <level_seed>` (the REAL
//! `Level::GenerateFromSettings` over its own `Rand`); the start state is `build_match`, or 4½c's
//! weapon-selection route for `ai_vs_ai`. The Rust side is driven exactly as the gate drives it
//! (`drive`: a CPU keeps its persisted word, `run_ais` in `LocalController`'s order, then the
//! tick), and the ledger comes from `run_ais_traced` and the driven state (plan D12).
//!
//! Inputs: per tick `Rand(input_seed).next_u32() & 0x7f` for worm 0 then worm 1 (4½a's shape);
//! a CPU's and an idle human's word is 0, so the file has no `input` for it.
//!
//! Run it in a DEBUG build: `scan` skips (and reports, with the tick and the panicking line) a
//! seed that trips a `debug_assert!`, and refuses a seed whose driven state ever holds an
//! `aiming_angle` outside `0..128` (the `cossin_table[128]` UB, e-1 Addendum G3). Every level is at
//! least 342 rows tall (the spawn-box UB, e-1 Addendum G3). The C++ side of every committed
//! golden is also re-run under `_GLIBCXX_ASSERTIONS` + ASan (`gen_sim_slice4_5f_golden.sh`,
//! `CHECKED_DUMPER`).

#[path = "../tests/sim_slice4_5f_common/mod.rs"]
mod common;

use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};

use common::{Case, Ledger, Ticks};
use scenario::settings::Settings;
use scenario::settings_toml::settings_from_toml;

thread_local! {
    /// Where the last caught panic happened (`scan`'s report).
    static LAST_PANIC: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// The tick `drive` is processing (`scan`'s panic report).
    static TICK: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The one weapon-selection frame of a `weapsel` case: both CPUs are ready at once (RANDOM),
/// so the phase ends on frame 0 with no key.
const WEAPSEL: [[u32; 2]; 1] = [[0, 0]];

fn weapsel(c: &Case) -> &'static [[u32; 2]] {
    if c.weapsel {
        &WEAPSEL
    } else {
        &[]
    }
}

struct World {
    settings: Settings,
    level: assets::level::LevelData,
}

fn world(c: &Case) -> World {
    let tc = common::load_tc();
    let objects = common::load_objects(&tc);
    // Through the sidecar's bytes, as the gate reads it.
    let settings = settings_from_toml(&common::setup_cfg(c, &objects)).expect("sidecar parses");
    let level = common::generate(&settings, c.level_seed);
    assert_eq!((level.width, level.height), (c.width, c.height));
    assert!(c.height >= 342, "no level below 342 rows (e-1 Addendum G3)");
    World { settings, level }
}

fn run(c: &Case, w: &World, game_seed: u32, input_seed: u32) -> Option<Ledger> {
    let ins = common::inputs(c, input_seed, c.ticks.horizon());
    catch_unwind(AssertUnwindSafe(|| {
        let (st, ais) = common::start(&w.settings, game_seed, &w.level, c.ai(), weapsel(c));
        common::drive(st, ais, &ins, c.ticks.horizon(), true, None, |k| {
            TICK.with(|t| t.set(k))
        })
    }))
    .ok()
}

fn summary(l: &Ledger) -> String {
    format!(
        "rows={} over={:?} deaths={}/{} kills01={} kills10={} respawns={}/{} fb_drew={} \
         fb_quiet={} rope={} reacts={:?} min_dist={:?} explo={} speed={} floored={} weapons={} \
         picks={}/{} clamps={} drips={:?} wraps={}/{} transfers={:?} walk={} in_range={}",
        l.rows,
        l.game_over_tick,
        l.deaths_of(0),
        l.deaths_of(1),
        l.kills(0, 1),
        l.kills(1, 0),
        l.respawns_of(0),
        l.respawns_of(1),
        l.fallback_drew(),
        l.fallback_quiet(),
        l.rope,
        l.reacts,
        l.min_real_dist,
        l.explo,
        l.speed,
        l.floored,
        l.cpu_weapons.len(),
        l.picks_of(0),
        l.picks_of(1),
        l.clamps(),
        l.drips,
        l.wraps_of(0),
        l.wraps_of(1),
        l.transfers_onto,
        l.walk,
        l.fire_in_range,
    )
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
            let tc = common::load_tc();
            let text = common::setup_cfg(common::case(arg(1)), &common::load_objects(&tc));
            std::fs::write(arg(2), text).expect("write setup");
            println!("wrote {}", arg(2));
        }
        "scan" => {
            let c = common::case(arg(1));
            let input_seed = num(2);
            set_hook(Box::new(|info| {
                let at = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_default();
                LAST_PANIC.with(|p| *p.borrow_mut() = at);
            }));
            let w = world(c);
            for game_seed in num(3)..=num(4) {
                match run(c, &w, game_seed, input_seed) {
                    None => println!(
                        "{} game_seed={game_seed} PANIC tick {} at {}",
                        c.name,
                        TICK.with(|t| t.get()),
                        LAST_PANIC.with(|p| p.borrow().clone())
                    ),
                    Some(l) => {
                        let ok = common::witnesses(c, &l);
                        println!(
                            "{} game_seed={game_seed} ok={} {}",
                            c.name,
                            ok.as_ref()
                                .map_or_else(|e| format!("no({e})"), |()| "yes".into()),
                            summary(&l)
                        );
                    }
                }
            }
        }
        "gen" => {
            let c = common::case(arg(1));
            let (game_seed, input_seed) = (num(2), num(3));
            let w = world(c);
            let l = run(c, &w, game_seed, input_seed).expect("the seed must not panic");
            if let Err(e) = common::witnesses(c, &l) {
                panic!("seed fails the witness `{e}`: {}", summary(&l));
            }
            let ticks = l.rows;
            let gate = if c.ai() { "G-AI" } else { "G-HP" };
            let horizon = match c.ticks {
                Ticks::Fixed(n) => format!("{n} ticks (or a game over + {})", common::MARGIN),
                Ticks::UntilOver(_) => format!("until the game over + {}", common::MARGIN),
            };
            let mut out = format!(
                "# Step 4½f-1 T6 — {gate} `{name}` ({wd}x{ht}): {about}; {horizon}. Read by BOTH\n\
                 # the C++ dumper (oracle_dump_sim_physics: `generate` = the REAL\n\
                 # Level::GenerateFromSettings over its own Rand seeded {ls}; `settings` = the real\n\
                 # Settings::FromToml + the LocalController start state{ai_note}) and the Rust gate\n\
                 # (sim_slice4_5f_golden.rs). Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5f1_sim -- gen {name} {game_seed} {input_seed} <this file>\n\
                 # level_seed {ls} (fixed per case); seed = the game seed.\n",
                name = c.name,
                wd = c.width,
                ht = c.height,
                about = c.about,
                ls = c.level_seed,
                ai_note = if c.ai() {
                    "; `ai` = a REAL DumbLieroAI per\n\
                     # controller-1 player, run in LocalController's order before each tick"
                } else {
                    ""
                },
            );
            for line in common::ledger_lines(c, &l) {
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str(
                "# Inputs: per tick Rand(input_seed).next_u32() & 0x7f, worm 0 then worm 1 (zero pairs\n\
                 # omitted); a CPU's and an idle human's word is 0.\n",
            );
            out.push_str(&format!(
                "seed {game_seed}\ngenerate {ls}\nticks {ticks}\nsettings sim_slice4_5f_{name}_setup.cfg\n",
                ls = c.level_seed,
                name = c.name,
            ));
            if c.ai() {
                out.push_str("ai\n");
            }
            for (f, wd) in weapsel(c).iter().enumerate() {
                out.push_str(&format!("weapsel {f} {} {}\n", wd[0], wd[1]));
            }
            for (t, i) in common::inputs(c, input_seed, ticks).iter().enumerate() {
                if i[0] != 0 || i[1] != 0 {
                    out.push_str(&format!("input {t} {} {}\n", i[0], i[1]));
                }
            }
            std::fs::write(arg(4), out).expect("write scenario");
            println!("wrote {} (ticks {ticks})", arg(4));
            println!("{}", summary(&l));
        }
        other => panic!("unknown command {other:?} (cfg | scan | gen)"),
    }
}
