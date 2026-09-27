//! Step 4½f-1 T8 — the G2f-1 shell corpus writer (plan Task 8). A dev tool, not a test; not run
//! in CI. Run in a DEBUG build.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5f1_shell -- check
//!   cargo run -p oracle-tests --example gen_slice4_5f1_shell -- write <golden dir>
//!   cargo run -p oracle-tests --example gen_slice4_5f1_shell -- search <case> <lo> <hi> <golden dir>
//!
//! `write` first writes each case's inputs (the setups, the user `liero.cfg`s and their `fs`
//! manifests), which the driver reads from the golden dir. Both then drive every case through
//! the REAL Rust shell and refuse a case with any violation or a missing witness (plan T8
//! Steps 3-4); `write` finally writes the `shell_<case>_script.txt`s with a two-line header
//! (case, ledger). `check` also asserts that the committed inputs equal the generated ones.
//! `search` writes the inputs, then prints every seed in `lo..hi` whose case (on that match
//! seed) has no violation and every witness — how `shell_f1_cases::PINS` were found. The goldens
//! are C++'s: gen_shell_golden.sh.

#[path = "../tests/shell_common/mod.rs"]
mod shell_common;
#[path = "../tests/shell_f1_cases/mod.rs"]
mod shell_f1_cases;

use shell_common as sc;
use shell_f1_cases as f1;

fn summary(run: &sc::Run) -> String {
    let l = &run.ledger;
    format!(
        "{} frames, end `{}`, NEW GAME {}, RESUME {}, back-to-menu {}, CPU match frames {:?}, \
         deaths {:?}, respawns {:?}, game over {:?}, P2 keys in a match {}, file lines {}",
        run.lines.len(),
        run.end,
        l.new_games,
        l.resumes,
        l.menus,
        [0, 1].map(|i| (0..run.lines.len())
            .filter(|&f| run.worms[f].is_some() && l.cpu[f][i])
            .count()),
        l.deaths,
        l.respawns,
        l.game_over_frames,
        l.p2_keys,
        run.files.len()
    )
}

fn write_inputs(dir: &std::path::Path) {
    for c in f1::cases() {
        for (file, bytes) in &c.files {
            std::fs::write(dir.join(file), bytes).unwrap();
        }
    }
}

/// `cpu_vs_cpu` on `seed`: the frame of the post-mortem's pop to the menu (the first frame after
/// the match whose top is `M`), from the quit-less search drive.
fn cpu_vs_cpu_pop(seed: u32) -> Option<u32> {
    let run = sc::drive(&f1::cpu_vs_cpu_script(seed, None), None);
    let first = (0..run.lines.len()).find(|&f| run.worms[f].is_some())?;
    (first..run.lines.len())
        .find(|&f| run.lines[f].split_whitespace().nth(8) == Some("M"))
        .map(|f| f as u32)
}

fn search(case: &str, lo: u32, hi: u32) {
    let threads = 4u32;
    let hs: Vec<_> = (0..threads)
        .map(|t| {
            let case = case.to_string();
            std::thread::spawn(move || {
                for seed in (lo..hi).filter(|s| s % threads == t) {
                    let mut pins = f1::PINS;
                    match case.as_str() {
                        "cpu_match" => pins.cpu_match = seed,
                        "cpu_pick" => pins.cpu_pick = seed,
                        "hp_boot" => pins.hp_boot = seed,
                        "cpu_vs_cpu" => match cpu_vs_cpu_pop(seed) {
                            Some(p) => pins.cpu_vs_cpu = (seed, p + 20),
                            None => {
                                println!("seed {seed}: no pop to the menu in 9,000 frames");
                                continue;
                            }
                        },
                        other => panic!("unknown case {other}"),
                    }
                    let c = f1::cases_with(&pins)
                        .into_iter()
                        .find(|c| c.name == case)
                        .unwrap();
                    let run = sc::drive(&c.script, None);
                    match f1::witnesses(&c, &run) {
                        Ok(w) => println!("seed {seed}: OK {:?} {}; {w:?}", pins, summary(&run)),
                        Err(e) => println!("seed {seed}: {e}; {}", summary(&run)),
                    }
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first().map(String::as_str) {
        Some("check") => None,
        Some("write") => Some(std::path::PathBuf::from(
            args.get(1).expect("write <golden dir>"),
        )),
        Some("search") => {
            let n = |i: usize| args.get(i).expect("search <case> <lo> <hi> <golden dir>");
            write_inputs(std::path::Path::new(n(4)));
            search(n(1), n(2).parse().unwrap(), n(3).parse().unwrap());
            return;
        }
        _ => panic!("usage: gen_slice4_5f1_shell check | write <golden dir> | search …"),
    };
    let mut bad = Vec::new();
    for f in sc::install_coverage() {
        bad.push(format!(
            "Fs::install leaves out data/{f}, which a filter lists"
        ));
    }
    let cases = f1::cases();
    assert_eq!(
        cases.iter().map(|c| c.name).collect::<Vec<_>>(),
        f1::NAMES,
        "the case order"
    );
    // Inputs first: `drive` reads them from the golden dir.
    match &dir {
        Some(d) => write_inputs(d),
        None => {
            for c in &cases {
                for (file, bytes) in &c.files {
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
        let summary = summary(&run);
        println!("{:<12} {summary}", c.name);
        for v in &run.ledger.violations {
            println!("  VIOLATION {v}");
            bad.push(format!("{}: {v}", c.name));
        }
        match f1::witnesses(c, &run) {
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
                    "Step 4½f-1 G2f-1 case {} (plan Task 8) — written by gen_slice4_5f1_shell.",
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
