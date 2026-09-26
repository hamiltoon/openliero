//! Step 4½d — 🎯 THE MILESTONE (design §6.6, done-when 2-3). Every G2 case — boot → main menu →
//! NEW GAME → weapon selection → play → Esc → menu (RESUME / NEW GAME) → … → QUIT, and the
//! corners around it — replayed through `ui::shell::Shell` matches the REAL C++
//! `Gfx::RunOneFrame` (`oracle_dump_shell`, `gen_shell_golden.sh`) on EVERY frame: presents, the
//! presented (faded) frame, the back buffer, fade, menu_cycles, the top screen, the main-menu
//! cursor, and the sounds of every frame the menu or weapon selection updated (`upd` M/W; a match
//! frame's sim sounds are 4c's parity, design §6.5). `SHELL_RUST_PPM_DIR=<dir>` writes the Rust
//! presented frames around a mismatch as `<dir>/shell_<case>/f_NNNN.ppm` (the dumper's
//! --ppm-dir names).

//!
//! Step 4½e-1 (plan Task 9): the G2e-1 cases (`shell_e1_cases`) add a `d` line after every `f`
//! line (`cur`, `ssel`, `cfg16`, `state8`) and, for the `fs` cases, the `file` lines after the
//! `end` line (the exit save and the boot-time defaults save); both are compared field by field.

mod shell_common;
mod shell_e1_cases;

use render::present::fade_argb;
use shell_common as sc;
use shell_e1_cases as e1;

const FIELDS: [&str; 11] = [
    "f",
    "frame",
    "upd",
    "presents",
    "presented16",
    "bmp16",
    "fade",
    "menu_cycles",
    "top",
    "sel",
    "sounds",
];

/// The `d` line's fields (plan D1).
const D_FIELDS: [&str; 6] = ["d", "frame", "cur", "ssel", "cfg16", "state8"];
/// The `file` line's fields (plan §Formats).
const FILE_FIELDS: [&str; 3] = ["file", "rel", "fnv16"];

struct Golden {
    boot: String,
    frames: Vec<String>,
    details: Vec<String>,
    end: String,
    files: Vec<String>,
}

fn golden(name: &str) -> Golden {
    let text = std::fs::read_to_string(format!("{}/shell_{name}.txt", sc::GOLDEN)).unwrap();
    let mut g = Golden {
        boot: String::new(),
        frames: Vec::new(),
        details: Vec::new(),
        end: String::new(),
        files: Vec::new(),
    };
    for l in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        match l.split_whitespace().next() {
            Some("boot") => g.boot = l.to_string(),
            Some("f") => g.frames.push(l.to_string()),
            Some("d") => g.details.push(l.to_string()),
            Some("end") => g.end = l.to_string(),
            Some("file") => g.files.push(l.to_string()),
            _ => panic!("{name}: unexpected golden line {l}"),
        }
    }
    g
}

/// The first field of `fields` where two lines differ.
fn first_diff<'a>(fields: &[&'a str], a: &str, b: &str) -> Option<&'a str> {
    let (x, y): (Vec<&str>, Vec<&str>) = (
        a.split_whitespace().collect(),
        b.split_whitespace().collect(),
    );
    fields
        .iter()
        .enumerate()
        .find(|(k, _)| x.get(*k) != y.get(*k))
        .map(|(_, f)| *f)
        .or((x.len() != y.len()).then_some("(extra fields)"))
}

/// `Err((frame, message))` at the first difference.
fn compare(name: &str, want: &Golden, run: &sc::Run) -> Result<(), (u32, String)> {
    if run.boot != want.boot {
        return Err((
            0,
            format!(
                "{name}: G2 boot line differs\n  C++:  {}\n  Rust: {}",
                want.boot, run.boot
            ),
        ));
    }
    for (i, (g, w)) in run.lines.iter().zip(&want.frames).enumerate() {
        let (gf, wf): (Vec<&str>, Vec<&str>) = (
            g.split_whitespace().collect(),
            w.split_whitespace().collect(),
        );
        for (k, field) in FIELDS.iter().enumerate() {
            if k == 10 && wf[2] == "G" {
                continue; // a match frame's sim sounds (design §6.5)
            }
            if gf.get(k) != wf.get(k) {
                return Err((
                    i as u32,
                    format!("{name}: G2 line {i} (frame {}), column `{field}` differs\n  C++:  {w}\n  Rust: {g}", wf[1]),
                ));
            }
        }
        // Step 4½e-1: the frame's `d` line, field by field (plan D1).
        match (run.details.get(i), want.details.get(i)) {
            (None, None) => {}
            (Some(g), Some(w)) => {
                if let Some(field) = first_diff(&D_FIELDS, g, w) {
                    return Err((
                        i as u32,
                        format!("{name}: G2 d line of frame {}, column `{field}` differs\n  C++:  {w}\n  Rust: {g}", wf[1]),
                    ));
                }
            }
            (g, w) => {
                return Err((
                    i as u32,
                    format!("{name}: G2 d line of frame {} present on one side only\n  C++:  {w:?}\n  Rust: {g:?}", wf[1]),
                ))
            }
        }
    }
    if run.lines.len() != want.frames.len() || run.end != want.end {
        let n = run.lines.len().min(want.frames.len()) as u32;
        return Err((
            n,
            format!(
                "{name}: G2 line count / end: C++ {} `{}`, Rust {} `{}`",
                want.frames.len(),
                want.end,
                run.lines.len(),
                run.end
            ),
        ));
    }
    if run.details.len() != want.details.len() {
        return Err((
            run.lines.len() as u32,
            format!(
                "{name}: G2 d line count: C++ {}, Rust {}",
                want.details.len(),
                run.details.len()
            ),
        ));
    }
    if run.files.len() != want.files.len() {
        return Err((
            run.lines.len() as u32,
            format!(
                "{name}: G2 file line count: C++ {} {:?}, Rust {} {:?}",
                want.files.len(),
                want.files,
                run.files.len(),
                run.files
            ),
        ));
    }
    for (g, w) in run.files.iter().zip(&want.files) {
        if let Some(field) = first_diff(&FILE_FIELDS, g, w) {
            return Err((
                run.lines.len() as u32,
                format!("{name}: G2 file line, column `{field}` differs\n  C++:  {w}\n  Rust: {g}"),
            ));
        }
    }
    Ok(())
}

fn check(name: &str) {
    let script = sc::read_script(name);
    let want = golden(name);
    let run = sc::drive(&script, None);
    assert!(
        run.ledger.violations.is_empty(),
        "{name}: {:?}",
        run.ledger.violations
    );
    if let Err((frame, msg)) = compare(name, &want, &run) {
        if let Ok(dir) = std::env::var("SHELL_RUST_PPM_DIR") {
            let dir = std::path::Path::new(&dir).join(format!("shell_{name}"));
            std::fs::create_dir_all(&dir).unwrap();
            let keep = sc::drive(&script, Some((frame.saturating_sub(3), frame + 1)));
            for (f, bmp, fade) in &keep.shots {
                let mut data = format!("P6\n{} {}\n255\n", bmp.w, bmp.h).into_bytes();
                for p in &bmp.pixels {
                    let c = fade_argb(*p, *fade);
                    data.extend([(c >> 16) as u8, (c >> 8) as u8, c as u8]);
                }
                std::fs::write(dir.join(format!("f_{f:04}.ppm")), data).unwrap();
            }
        }
        panic!("{msg}");
    }
}

#[test]
fn the_milestone_is_bit_exact_on_every_frame() {
    // design §6.6: boot → menu → NEW GAME → selection → play → Esc → menu (RESUME GAME (F1) /
    // NEW GAME) → RESUME → play → Esc → NEW GAME (level reused) → selection → Esc → menu → QUIT.
    let l = sc::drive(&sc::read_script("milestone"), None).ledger;
    assert_eq!(
        (l.new_games, l.resumes, l.menus, l.quit),
        (2, 1, 3, true),
        "the milestone path"
    );
    check("milestone");
}

#[test]
fn every_g2_case_is_bit_exact() {
    for name in sc::CASES_NAMES.iter().filter(|n| **n != "milestone") {
        check(name);
    }
}

#[test]
fn the_committed_scripts_are_the_generators() {
    let seed = sc::read_script("game_over").match_seeds[0];
    for c in sc::cases(seed) {
        assert_eq!(
            sc::read_script(c.name),
            c.script,
            "{}: regenerate with gen_slice4_5d",
            c.name
        );
    }
    let mut on_disk: Vec<String> = std::fs::read_dir(sc::GOLDEN)
        .unwrap()
        .filter_map(|e| {
            let f = e.unwrap().file_name().to_string_lossy().into_owned();
            Some(
                f.strip_prefix("shell_")?
                    .strip_suffix("_script.txt")?
                    .to_string(),
            )
        })
        .collect();
    on_disk.sort();
    // The union: the 4½d 11 and the e-1 11, each against its own generator.
    let labels_seed = sc::read_script("labels").match_seeds[0];
    for c in e1::cases(labels_seed) {
        assert_eq!(
            sc::read_script(c.name),
            c.script,
            "{}: regenerate with gen_slice4_5e1_shell",
            c.name
        );
        for (file, text) in &c.files {
            assert_eq!(
                std::fs::read_to_string(format!("{}/{file}", sc::GOLDEN)).unwrap(),
                *text,
                "{}: {file} — regenerate with gen_slice4_5e1_shell",
                c.name
            );
        }
    }
    let mut want: Vec<String> = sc::CASES_NAMES
        .iter()
        .chain(e1::NAMES.iter())
        .map(|s| s.to_string())
        .collect();
    want.sort();
    assert_eq!(on_disk, want);
}

#[test]
fn the_e1_milestone_is_bit_exact() {
    // 🎯 e-1 (done-when 2): F7 → GAME MODE ×3 → LIVES typed → LOADING TIMES held → MAP WIDTH /
    // HEIGHT typed → WEAPON OPTIONS (ban, the box, re-enable) → NEW GAME → selection → play →
    // Esc → AMOUNT OF BLOOD held, MAX BONUSES typed → RESUME → play → Esc → QUIT → the exit save.
    let run = sc::drive(&sc::read_script("match_setup"), None);
    let l = &run.ledger;
    assert_eq!(
        (
            l.new_games,
            l.resumes,
            l.quit,
            l.weapon_boxes > 0,
            run.files.len()
        ),
        (1, 1, true, true, 1),
        "the milestone path"
    );
    check("match_setup");
}

#[test]
fn every_g2e1_case_is_bit_exact() {
    for name in e1::NAMES.iter().filter(|n| **n != "match_setup") {
        check(name);
    }
}

/// `golden(name)` with one bit of the 64-bit hex field `k` of `line` flipped.
fn flip(line: &str, k: usize) -> String {
    let mut f: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    f[k] = format!("{:016x}", u64::from_str_radix(&f[k], 16).unwrap() ^ 1);
    f.join(" ")
}

#[test]
#[should_panic(expected = "G2 d line")]
fn the_gate_sees_a_d_line_change() {
    let mut want = golden("cfg_default");
    want.details[10] = flip(&want.details[10], 4);
    let run = sc::drive(&sc::read_script("cfg_default"), None);
    if let Err((_, msg)) = compare("cfg_default", &want, &run) {
        panic!("{msg}");
    }
}

#[test]
#[should_panic(expected = "G2 file line")]
fn the_gate_sees_a_saved_byte_change() {
    let mut want = golden("cfg_default");
    want.files[0] = flip(&want.files[0], 2);
    let run = sc::drive(&sc::read_script("cfg_default"), None);
    if let Err((_, msg)) = compare("cfg_default", &want, &run) {
        panic!("{msg}");
    }
}

#[test]
#[should_panic(expected = "G2 line")]
fn the_gate_sees_a_one_pixel_change() {
    let mut want = golden("boot_idle");
    let mut f: Vec<String> = want.frames[5]
        .split_whitespace()
        .map(str::to_string)
        .collect();
    f[5] = format!("{:016x}", u64::from_str_radix(&f[5], 16).unwrap() ^ 1);
    want.frames[5] = f.join(" ");
    let run = sc::drive(&sc::read_script("boot_idle"), None);
    if let Err((_, msg)) = compare("boot_idle", &want, &run) {
        panic!("{msg}");
    }
}
