//! Step 4½e-1 T8 — the G2e-1 shell corpus writer (plan Task 8). A dev tool, not a test; not run
//! in CI. Run in a DEBUG build.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5e1_shell -- check
//!   cargo run -p oracle-tests --example gen_slice4_5e1_shell -- write <golden dir>
//!
//! `write` first writes each case's sidecar files (setups, `fs` manifests, user `liero.cfg`s),
//! which the driver reads from the golden dir. Both then drive every case through the REAL Rust
//! shell and refuse a case with any violation or a missing witness (plan T8 Steps 2 and 4);
//! `write` finally writes the `shell_<case>_script.txt`s with a two-line header (case, ledger).
//! `check` also asserts that the committed sidecars equal the generated ones. The goldens are
//! C++'s: gen_shell_golden.sh.

#[path = "../tests/shell_common/mod.rs"]
mod shell_common;
#[path = "../tests/shell_e1_cases/mod.rs"]
mod shell_e1_cases;

use shell_common as sc;
use shell_e1_cases as e1;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first().map(String::as_str) {
        Some("check") => None,
        Some("write") => Some(std::path::PathBuf::from(
            args.get(1).expect("write <golden dir>"),
        )),
        _ => panic!("usage: gen_slice4_5e1_shell check | write <golden dir>"),
    };
    // Sidecars first (they do not depend on the labels seed): `drive` reads them from the
    // golden dir.
    let mut bad = Vec::new();
    for c in &e1::cases(0) {
        for (file, text) in &c.files {
            match &dir {
                Some(d) => std::fs::write(d.join(file), text).unwrap(),
                None => {
                    let on_disk =
                        std::fs::read_to_string(std::path::Path::new(sc::GOLDEN).join(file));
                    if on_disk.as_deref().ok() != Some(text.as_str()) {
                        bad.push(format!("{}: {file} differs from the generator", c.name));
                    }
                }
            }
        }
    }
    let seed = e1::find_labels_seed();
    println!("labels match seed: {seed}");
    let cases = e1::cases(seed);
    assert_eq!(
        cases.iter().map(|c| c.name).collect::<Vec<_>>(),
        e1::NAMES,
        "the case order"
    );
    let mut written = 0;
    for c in &cases {
        let run = sc::drive(&c.script, None);
        let l = &run.ledger;
        let summary = format!(
            "{} frames, end `{}`, NEW GAME {}, RESUME {}, back-to-menu {}, tops {}, entries {}, \
             weapon boxes {}, file lines {}",
            run.lines.len(),
            run.end,
            l.new_games,
            l.resumes,
            l.menus,
            l.tops.iter().collect::<String>(),
            l.entries.len(),
            l.weapon_boxes,
            run.files.len()
        );
        println!("{:<15} {summary}", c.name);
        for v in &l.violations {
            println!("  VIOLATION {v}");
            bad.push(format!("{}: {v}", c.name));
        }
        match e1::witnesses(c, &run) {
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
                    "Step 4½e-1 G2e-1 case {} (plan Task 8) — written by gen_slice4_5e1_shell.",
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
