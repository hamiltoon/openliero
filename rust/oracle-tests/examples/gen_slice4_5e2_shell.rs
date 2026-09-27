//! Step 4½e-2 T5 — the G2e-2 shell corpus writer (plan Task 5). A dev tool, not a test; not run
//! in CI. Run in a DEBUG build.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5e2_shell -- check
//!   cargo run -p oracle-tests --example gen_slice4_5e2_shell -- write <golden dir>
//!
//! `write` first writes each case's inputs (the `fs` manifests, the two tiny levels, the two user
//! setups), which the driver reads from the golden dir. Both then drive every case through the
//! REAL Rust shell and refuse a case with any violation or a missing witness (plan T5 Steps 2
//! and 4, as amended by Addendum T0); `write` finally writes the `shell_<case>_script.txt`s with
//! a two-line header (case, ledger). `check` also asserts that the committed inputs equal the
//! generated ones. The goldens are C++'s: gen_shell_golden.sh.

#[path = "../tests/shell_common/mod.rs"]
mod shell_common;
#[path = "../tests/shell_e2_cases/mod.rs"]
mod shell_e2_cases;

use shell_common as sc;
use shell_e2_cases as e2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first().map(String::as_str) {
        Some("check") => None,
        Some("write") => Some(std::path::PathBuf::from(
            args.get(1).expect("write <golden dir>"),
        )),
        _ => panic!("usage: gen_slice4_5e2_shell check | write <golden dir>"),
    };
    let mut bad = Vec::new();
    // Plan §Formats: what Fs::install() leaves out of data/ is never listed by a gated case.
    for f in sc::install_coverage() {
        bad.push(format!(
            "Fs::install leaves out data/{f}, which a filter lists"
        ));
    }
    let cases = e2::cases();
    assert_eq!(
        cases.iter().map(|c| c.name).collect::<Vec<_>>(),
        e2::NAMES,
        "the case order"
    );
    // Inputs first: `drive` reads them from the golden dir.
    for c in &cases {
        for (file, bytes) in &c.files {
            match &dir {
                Some(d) => std::fs::write(d.join(file), bytes).unwrap(),
                None => {
                    let on_disk = std::fs::read(std::path::Path::new(sc::GOLDEN).join(file));
                    if on_disk.as_ref().ok() != Some(bytes) {
                        bad.push(format!("{}: {file} differs from the generator", c.name));
                    }
                }
            }
        }
    }
    let mut written = 0;
    for c in &cases {
        let run = sc::drive(&c.script, None);
        let l = &run.ledger;
        let summary = format!(
            "{} frames, end `{}`, NEW GAME {}, RESUME {}, back-to-menu {}, tops {}, previews {}, \
             level picks {}, reserved boxes {}, saves {}, loads {}, file lines {}",
            run.lines.len(),
            run.end,
            l.new_games,
            l.resumes,
            l.menus,
            l.tops.iter().collect::<String>(),
            l.previews.len(),
            l.level_picks.len(),
            l.reserved_boxes,
            l.saves.len(),
            l.loads.len(),
            run.files.len()
        );
        println!("{:<18} {summary}", c.name);
        for v in &l.violations {
            println!("  VIOLATION {v}");
            bad.push(format!("{}: {v}", c.name));
        }
        match e2::witnesses(c, &run) {
            Ok(lines) => {
                for w in lines {
                    println!("  witness: {w}");
                }
            }
            Err(e) => {
                println!("  WITNESS {e}");
                bad.push(format!("{}: {e}", c.name));
            }
        }
        if let Some(d) = &dir {
            let header = vec![
                format!(
                    "Step 4½e-2 G2e-2 case {} (plan Task 5) — written by gen_slice4_5e2_shell.",
                    c.name
                ),
                format!("Rust ledger: {summary}"),
            ];
            std::fs::write(
                d.join(format!("shell_{}_script.txt", c.name)),
                c.script.to_text(&header),
            )
            .unwrap();
            written += 1;
        }
    }
    assert!(bad.is_empty(), "the corpus is not clean: {bad:#?}");
    if let Some(d) = dir {
        println!("wrote {written} cases to {}", d.display());
    }
}
