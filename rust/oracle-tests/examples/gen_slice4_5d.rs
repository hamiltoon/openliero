//! Step 4½d T9 — the G2 shell corpus writer (design §6.4). A dev tool, not a test; not run in CI.
//! Run in a DEBUG build.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5d -- check
//!   cargo run -p oracle-tests --example gen_slice4_5d -- write <golden dir>
//!
//! Both drive every case through the REAL Rust shell and refuse a case with any violation
//! (plan Task 9); `write` then writes the 11 `shell_<case>_script.txt` + the 5 `_setup.cfg`.
//! The goldens are C++'s: gen_shell_golden.sh.

#[path = "../tests/shell_common/mod.rs"]
mod sc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first().map(String::as_str) {
        Some("check") => None,
        Some("write") => Some(std::path::PathBuf::from(
            args.get(1).expect("write <golden dir>"),
        )),
        _ => panic!("usage: gen_slice4_5d check | write <golden dir>"),
    };
    // Setups first: `drive` reads them from the golden dir.
    let seed = {
        if let Some(d) = &dir {
            for c in sc::cases(1) {
                if let Some(text) = c.setup {
                    std::fs::write(d.join(format!("shell_{}_setup.cfg", c.name)), text).unwrap();
                }
            }
        }
        sc::find_game_over_seed()
    };
    println!("game_over match seed: {seed}");
    let mut bad = Vec::new();
    for c in sc::cases(seed) {
        let run = sc::drive(&c.script, None);
        let l = &run.ledger;
        let summary = format!(
            "{} frames, end `{}`, NEW GAME {}, RESUME {}, back-to-menu {} (game over: {}), black presents {}",
            run.lines.len(), run.end, l.new_games, l.resumes, l.menus, l.game_over_menu, l.black
        );
        println!("{:<15} {summary}", c.name);
        for v in &l.violations {
            println!("  VIOLATION {v}");
            bad.push(format!("{}: {v}", c.name));
        }
        if let Some(d) = &dir {
            let header = vec![
                format!(
                    "Step 4½d G2 case {} (design §6.4; plan Task 9) — written by gen_slice4_5d.",
                    c.name
                ),
                format!("Rust ledger: {summary}"),
            ];
            std::fs::write(
                d.join(format!("shell_{}_script.txt", c.name)),
                c.script.to_text(&header),
            )
            .unwrap();
        }
    }
    assert!(bad.is_empty(), "the corpus has violations: {bad:?}");
    if let Some(d) = dir {
        println!("wrote 11 cases to {}", d.display());
    }
}
