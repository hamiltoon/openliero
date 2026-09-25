//! Step 4½c T6 — the weapon-selection corpus writer (design §6.4). A dev tool, not a test; not
//! run in CI. Run in a DEBUG build (overflow checks).
//!
//!   cargo run -p oracle-tests --example gen_slice4_5c -- check
//!   cargo run -p oracle-tests --example gen_slice4_5c -- write <golden dir>
//!
//! Both drive every case through the REAL Rust path (new_match + sim::weapsel) and refuse a
//! corpus that breaks the end-frame invariant (§6.1) or misses a witness (§6.7); `write` then
//! writes the 16 `weapsel_<case>_scenario.txt` + `_setup.cfg` pairs. The goldens are C++'s:
//! gen_weapsel_golden.sh + gen_sim_slice4_5c_golden.sh.

#[path = "../tests/weapsel_common/mod.rs"]
mod wc;

use scenario::settings_toml::settings_from_toml;
use scenario::Scenario;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let o = wc::load_objects();
    let mut runs = Vec::new();
    let mut files = Vec::new();
    for case in &wc::CASES {
        let setup = wc::setup_cfg(case, &o);
        let settings = settings_from_toml(&setup).expect("the generated setup parses");
        let scenario =
            Scenario::parse(&wc::scenario_text(case, "")).expect("the generated scenario parses");
        let (run, _) = wc::drive(case.name, &scenario, &settings);
        println!("{:<18} {}", case.name, run.summary());
        files.push((case.name, wc::scenario_text(case, &run.summary()), setup));
        runs.push(run);
    }
    let mut missing = Vec::new();
    for (name, ok) in wc::witnesses(&runs) {
        println!(
            "witness {name:<72} {}",
            if ok { "reached" } else { "MISSING" }
        );
        if !ok {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "the corpus misses {missing:?}: change the seed of the case meant to reach it (design §6.7)"
    );
    match args.first().map(String::as_str) {
        Some("check") => {}
        Some("write") => {
            let dir = std::path::Path::new(args.get(1).expect("write <golden dir>"));
            for (name, scenario, setup) in &files {
                std::fs::write(dir.join(format!("weapsel_{name}_scenario.txt")), scenario)
                    .expect("write scenario");
                std::fs::write(dir.join(format!("weapsel_{name}_setup.cfg")), setup)
                    .expect("write setup");
            }
            println!("wrote {} cases to {}", files.len(), dir.display());
        }
        _ => panic!("usage: gen_slice4_5c check | write <golden dir>"),
    }
}
