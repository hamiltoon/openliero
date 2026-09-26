//! Step 4½e-2 G2e-2 — the shell corpus for the level selector, SAVE SETUP AS… and LOAD SETUP
//! (plan Task 5; design §6): the six e-2 cases, their inputs (manifests, two tiny levels, two
//! user setups), and the per-case witnesses the generator (`gen_slice4_5e2_shell`) and the gate
//! (`shell_golden.rs`) check. The script model, the driver and the validators are
//! `shell_common`'s; the C++ side is `oracle_dump_shell` (`gen_shell_golden.sh`).
//!
//! Every case has `detail` and `fs`, scripts its match seeds, boots from a distinct seed and ends
//! by QUIT (the exit save writes the `file` lines). A case that plays a picked level has it in
//! both layers (plan D1.1); the Q4 twin (`shell_golden.rs`) drops the user copy.
//!
//! The settings cursor is back on GAME MODE after every return from a match (the menu's
//! `Enter`), and stays where it was when a selector or a box pops; the rows the scripts count
//! are the visible ones (a level file hides MAP WIDTH and MAP HEIGHT).
#![allow(dead_code)]

use scenario::settings::{Settings, GM_GAME_OF_TAG};
use scenario::settings_toml::settings_to_toml;

use crate::shell_common::{both_done, drive_with, play, Fs, Kind, Opts, Run, ShellScript, B};

/// The e-2 cases, in corpus order; `setups_and_levels` is the 🎯 milestone.
pub const NAMES: [&str; 6] = [
    "level_tree",
    "level_pick",
    "level_missing",
    "setup_save",
    "setup_load",
    "setups_and_levels",
];

/// One e-2 case: its script and the golden-dir files it reads (the `fs` manifest and the
/// generator-written inputs the manifest copies), as (file name, bytes).
pub struct Case {
    pub name: &'static str,
    pub script: ShellScript,
    pub files: Vec<(String, Vec<u8>)>,
}

/// The fixture's root label: every `level_file` a pick writes starts with it.
pub const ROOT: &str = "./user";
/// The Levels folder's `full_path` in the fixture.
pub const LEVELS: &str = "./user/TC/openliero/Levels";

const GOLDEN_REL: &str = "rust/oracle-tests/golden";

/// T0's `tiny.lev` (`$S/t0e2b/mk.py`): `OLLEVEL2` 60×40, material `(x + y) % 64 + 160` —
/// preview-only (40 rows: never played, e-1 Addendum G3).
pub fn tiny_lev() -> Vec<u8> {
    let mut v = b"OLLEVEL2\x01".to_vec();
    v.extend(60u16.to_le_bytes());
    v.extend(40u16.to_le_bytes());
    for y in 0..40u32 {
        for x in 0..60u32 {
            v.push(((x + y) % 64 + 160) as u8);
        }
    }
    v
}

/// T0's `trunc.lev`: `OLLEVEL2` 8×8 + `MODERNLV` + 10 zero bytes — rejected by C++ under
/// fact 11's clause (e), so preview-only (its NEW GAME is C++ UB, Addendum T0 P10).
pub fn trunc_lev() -> Vec<u8> {
    let mut v = b"OLLEVEL2\x01\x08\x00\x08\x00".to_vec();
    v.extend([0xA0; 64]);
    v.extend(b"MODERNLV");
    v.extend([0; 10]);
    v
}

/// `level_missing`'s user `liero.cfg`: a level file that is in neither layer.
pub fn missing_liero() -> Settings {
    Settings {
        random_level: false,
        level_file: format!("{LEVELS}/gone.lev"),
        ..Settings::default()
    }
}

/// `setup_load`'s user `Setups/mine.cfg`: Game of Tag, `timeToLose = 120`, lives 3, a random
/// level, everything else default.
pub fn load_mine() -> Settings {
    Settings {
        game_mode: GM_GAME_OF_TAG,
        time_to_lose: 120,
        lives: 3,
        random_level: true,
        ..Settings::default()
    }
}

fn manifest(case: &str) -> String {
    format!("shell_{case}_fs.txt")
}

fn golden_src(file: &str) -> String {
    format!("{GOLDEN_REL}/{file}")
}

/// `n` taps of `name`, `gap` frames apart (down at t, up at t + 2).
fn taps_gap(b: B, name: &str, n: u32, gap: u32) -> B {
    (0..n).fold(b, |b, _| {
        b.at(0, Kind::Down, name).at(2, Kind::Up, name).idle(gap)
    })
}

/// The level selector at the root (on `[RANDOM]`): Down `downs` rows to `TC`, then Right ×3,
/// 4 frames apart (T0 P1: `TC` → `openliero` → `Levels`, the cursor on Levels' first row).
fn root_to_levels(b: B, downs: u32) -> B {
    taps_gap(b.taps_n("DOWN", downs), "RIGHT", 3, 4)
}

/// Esc in the main focus (the cursor to QUIT TO OS), then Return: the quit.
fn quit(b: B) -> ShellScript {
    b.tap("ESC").idle(5).tap("RETURN").end(40, true)
}

/// From the settings focus: Esc to the main focus, then [`quit`].
fn quit_from_settings(b: B) -> ShellScript {
    quit(b.tap("ESC").idle(3))
}

/// NEW GAME from the main focus by F1 at boot (nothing to resume), selection, DONE, play `n`,
/// Esc back to the menu.
fn new_game_f1(b: B, seed: u32, n: u32) -> B {
    let b = b.seed(seed).tap("F1").after_menu_select();
    play(both_done(b), n).tap("ESC").after_esc()
}

/// NEW GAME from the settings focus while a match is paused: Esc (the main focus, the cursor
/// on MATCH SETUP), Down ×2 (wraps to RESUME, then NEW GAME), Return, selection, DONE, play
/// `n`, Esc.
fn new_game_paused(b: B, seed: u32, n: u32) -> B {
    let b = b
        .tap("ESC")
        .idle(3)
        .taps_n("DOWN", 2)
        .seed(seed)
        .tap("RETURN")
        .after_menu_select();
    play(both_done(b), n).tap("ESC").after_esc()
}

/// SAVE SETUP AS… on the current name (the settings cursor on it): Return (the name is
/// reserved: the box), any key (the box reopens the input on it), Backspace ×`bs`, `name`,
/// Return (saved).
fn save_as_refused_then(b: B, bs: u32, name: &str) -> B {
    b.tap("RETURN")
        .idle(3)
        .tap("RETURN")
        .idle(3)
        .tap("SPACE")
        .idle(3)
        .taps_n("BACKSPACE", bs)
        .type_chars(name)
        .tap("RETURN")
        .idle(4)
}

fn level_tree() -> Case {
    let m = manifest("level_tree");
    let tiny = "shell_level_tree_tiny.lev";
    let fs = Fs::install()
        .user_dir("Replays")
        .user_file(
            "TC/openliero/Levels/Zeta.lev",
            "data/TC/openliero/Levels/water_stage.lev",
        )
        .user_file(
            "TC/openliero/Levels/alpha.LEV",
            "data/TC/openliero/Levels/water_stage.lev",
        )
        .user_file(
            "TC/openliero/Levels/.hidden.lev",
            "data/TC/openliero/Levels/render_stage.lev",
        )
        .user_file("TC/openliero/Levels/notes.txt", "data/README.md")
        .user_file("TC/openliero/Levels/tiny.lev", &golden_src(tiny));
    // LEVEL (F7, Down ×2, Return): the root on [RANDOM] — [RANDOM], Profiles, Replays,
    // Resources, Setups, TC. Profiles (empty; the parent pane) and back; TC; Right ×3 into
    // Levels on `.hidden`; Down through every row 3 frames apart (the late preview, `tiny`'s
    // footprint); PgDn, PgUp; Left (the parent pane), Right; the `contains` search `h`, `i`
    // (Addendum T0 change 3); Esc (the menu keeps the last preview); P1's Jump to the main
    // focus; QUIT.
    let b = B::new(None, 61)
        .fs(&m)
        .detail()
        .idle(40)
        .tap("F7")
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(6)
        .tap("DOWN")
        .tap("RIGHT")
        .idle(3)
        .tap("LEFT")
        .idle(3);
    let b = root_to_levels(b, 4)
        .idle(3)
        .taps_n("DOWN", 8)
        .idle(3)
        .tap("PAGEDOWN")
        .idle(3)
        .tap("PAGEUP")
        .idle(3)
        .tap("LEFT")
        .idle(3)
        .tap("RIGHT")
        .idle(3)
        .tap("H")
        .tap("I")
        .idle(3)
        .tap("ESC")
        .idle(10)
        .tap("LALT")
        .idle(3);
    Case {
        name: "level_tree",
        script: quit(b),
        files: vec![
            (tiny.to_string(), tiny_lev()),
            (
                m,
                fs.text(&[
                    "Step 4½e-2 G2e-2 level_tree (T0 P1, P3): the install's system layer; the user's",
                    "Replays folder and five Levels entries (two water copies under other names, a",
                    "dotfile, a .txt the filter drops, the 60x40 tiny.lev).",
                ])
                .into_bytes(),
            ),
        ],
    }
}

fn level_pick() -> Case {
    let m = manifest("level_pick");
    let fs = Fs::install().user_level_copy("water_stage");
    // LEVEL → TC/openliero/Levels (rows modern_test … water_stage) → water_stage (Down ×4) →
    // pick; Esc; NEW GAME (F1) plays the file; 300 ticks; Esc. F7, Down ×2 → LEVEL reopens on
    // water_stage (the restore) → Left ×3 to the root (on TC) → Up ×4 to [RANDOM] → pick; NEW
    // GAME generates; 150 ticks; Esc; QUIT.
    let b = B::new(None, 62)
        .fs(&m)
        .detail()
        .idle(40)
        .tap("F7")
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(3);
    let b = root_to_levels(b, 4)
        .taps_n("DOWN", 4)
        .tap("RETURN")
        .idle(4)
        .tap("ESC")
        .idle(3);
    let b = new_game_f1(b, 6201, 300)
        .idle(10)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(6);
    let b = taps_gap(b, "LEFT", 3, 4)
        .taps_n("UP", 4)
        .tap("RETURN")
        .idle(4);
    let b = new_game_paused(b, 6202, 150);
    Case {
        name: "level_pick",
        script: quit(b),
        files: vec![(
            m,
            fs.text(&[
                "Step 4½e-2 G2e-2 level_pick (plan D1.1): the install's system layer and a user copy",
                "of water_stage.lev, which C++ opens from ./user/ (finding 2's workaround).",
            ])
            .into_bytes(),
        )],
    }
}

fn level_missing() -> Case {
    let m = manifest("level_missing");
    let liero = "shell_level_missing_user_liero.cfg";
    let trunc = "shell_level_missing_trunc.lev";
    let fs = Fs::install()
        .user_file("Setups/liero.cfg", &golden_src(liero))
        .user_file("TC/openliero/Levels/broken.lev", "data/README.md")
        .user_file("TC/openliero/Levels/trunc.lev", &golden_src(trunc));
    // Boot on the missing `gone.lev`: random. NEW GAME (F1) reuses the boot level; 150 ticks;
    // Esc. F7 → LEVEL (the root on [RANDOM]: `gone` is not found) → Levels on `broken` (no
    // preview) → Down ×5 to `trunc` (no preview) → Up ×5 back to `broken` → pick (Addendum T0
    // change 1). NEW GAME: C++ rejects `broken` (clause (b)), random; 150 ticks; Esc; QUIT.
    let b = B::new(None, 63).fs(&m).detail().idle(40);
    let b = new_game_f1(b, 6301, 150)
        .idle(10)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(3);
    let b = root_to_levels(b, 4).idle(4);
    let b = taps_gap(b, "DOWN", 5, 4).idle(4);
    let b = taps_gap(b, "UP", 5, 4).idle(4).tap("RETURN").idle(4);
    let b = new_game_paused(b, 6302, 150);
    Case {
        name: "level_missing",
        script: quit(b),
        files: vec![
            (liero.to_string(), settings_to_toml(&missing_liero()).into_bytes()),
            (trunc.to_string(), trunc_lev()),
            (
                m,
                fs.text(&[
                    "Step 4½e-2 G2e-2 level_missing (T0 P10): the user's liero.cfg names",
                    "./user/TC/openliero/Levels/gone.lev, in neither layer; broken.lev (a README) and",
                    "trunc.lev (a truncated MODERNLV block) are rows C++ neither previews nor plays.",
                ])
                .into_bytes(),
            ),
        ],
    }
}

fn setup_save() -> Case {
    let m = manifest("setup_save");
    let fs = Fs::install();
    // T0 P5: SAVE SETUP AS… (F7, Down ×13) on `liero` → Return (reserved: the box) → Space (the
    // input again) → Backspace ×5, `mine`, Return (saved) → SAVE SETUP AS… on `mine` →
    // Backspace ×4, `orbmit`, Return (shipped: the box) → Space → Esc (cancelled) → QUIT.
    let b = B::new(None, 64)
        .fs(&m)
        .detail()
        .idle(40)
        .tap("F7")
        .taps_n("DOWN", 13);
    let b = save_as_refused_then(b, 5, "mine")
        .tap("RETURN")
        .idle(3)
        .taps_n("BACKSPACE", 4)
        .type_chars("orbmit")
        .tap("RETURN")
        .idle(3)
        .tap("SPACE")
        .idle(3)
        .tap("ESC")
        .idle(4);
    Case {
        name: "setup_save",
        script: quit_from_settings(b),
        files: vec![(
            m,
            fs.text(&[
                "Step 4½e-2 G2e-2 setup_save (T0 P5): the install's system layer, an empty user",
                "folder (ShadowsSystem refuses liero.cfg and the shipped orbmit.cfg).",
            ])
            .into_bytes(),
        )],
    }
}

fn setup_load() -> Case {
    let m = manifest("setup_load");
    let mine = "shell_setup_load_user_mine.cfg";
    let fs = Fs::install().user_file("Setups/mine.cfg", &golden_src(mine));
    // NEW GAME (F1), 300 ticks, Esc. F7 → LOAD SETUP (Down ×14; opens in Setups on `liero`) →
    // Down ×2 → Left (the root, on Setups) → Right (Setups, the cursor kept on `orbmit`) →
    // pick `orbmit` → Esc → RESUME (F1): the paused match is detached and keeps its settings;
    // 300 ticks; Esc. F7 → LOAD SETUP (Down ×14 again) → `mine` → NEW GAME (Game of Tag from `mine`); 200 ticks;
    // Esc; QUIT (`liero.cfg` holds `mine`'s values).
    let b = B::new(None, 65).fs(&m).detail().idle(40);
    let b = new_game_f1(b, 6501, 300)
        .idle(10)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 14)
        .tap("RETURN")
        .idle(4)
        .taps_n("DOWN", 2)
        .tap("LEFT")
        .idle(3)
        .tap("RIGHT")
        .idle(3)
        .tap("RETURN")
        .idle(4)
        .tap("ESC")
        .idle(3)
        .tap("F1")
        .after_menu_select();
    let b = play(b, 300).tap("ESC").after_esc();
    let b = b
        .idle(10)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 14)
        .tap("RETURN")
        .idle(4)
        .tap("DOWN")
        .tap("RETURN")
        .idle(4);
    let b = new_game_paused(b, 6502, 200);
    Case {
        name: "setup_load",
        script: quit(b),
        files: vec![
            (
                mine.to_string(),
                settings_to_toml(&load_mine()).into_bytes(),
            ),
            (
                m,
                fs.text(&[
                    "Step 4½e-2 G2e-2 setup_load (T0 P4, P8): the install's system layer and the",
                    "user's Setups/mine.cfg (Game of Tag, timeToLose 120, lives 3).",
                ])
                .into_bytes(),
            ),
        ],
    }
}

fn setups_and_levels() -> Case {
    let m = manifest("setups_and_levels");
    let fs = Fs::install().user_level_copy("water_stage");
    // 🎯 LEVEL → Levels → water_stage → NEW GAME (the file is played); 200 ticks; Esc. F7,
    // Down ×2 → LEVEL reopens on water_stage → Esc. SAVE SETUP AS… (Down ×9: MAP WIDTH / HEIGHT are
    // hidden) → Return (`liero` refused) → Space → Backspace ×5, `mine`, Return. LOAD SETUP
    // (Down) → `orbmit` (Down ×2: liero, mine, orbmit). NEW GAME (a random level from orbmit);
    // 120 ticks; Esc; QUIT.
    let b = B::new(None, 66)
        .fs(&m)
        .detail()
        .idle(40)
        .tap("F7")
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(3);
    let b = root_to_levels(b, 4)
        .taps_n("DOWN", 4)
        .tap("RETURN")
        .idle(4)
        .tap("ESC")
        .idle(3);
    let b = new_game_f1(b, 6601, 200)
        .idle(10)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(6)
        .tap("ESC")
        .idle(3)
        .taps_n("DOWN", 9);
    let b = save_as_refused_then(b, 5, "mine")
        .tap("DOWN")
        .tap("RETURN")
        .idle(4)
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(4);
    let b = new_game_paused(b, 6602, 120);
    Case {
        name: "setups_and_levels",
        script: quit(b),
        files: vec![(
            m,
            fs.text(&[
                "Step 4½e-2 G2e-2 setups_and_levels (the milestone): the install's system layer and",
                "a user copy of water_stage.lev (plan D1.1).",
            ])
            .into_bytes(),
        )],
    }
}

/// The e-2 cases, in [`NAMES`] order.
pub fn cases() -> Vec<Case> {
    vec![
        level_tree(),
        level_pick(),
        level_missing(),
        setup_save(),
        setup_load(),
        setups_and_levels(),
    ]
}

fn need(ok: bool, what: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("witness missing: {what}"))
    }
}

/// The `f` line's `upd` and `top` (0-based fields 2 and 8).
fn upd_top(run: &Run, f: usize) -> (char, char) {
    let l: Vec<&str> = run.lines[f].split_whitespace().collect();
    (l[2].chars().next().unwrap(), l[8].chars().next().unwrap())
}

/// The frames whose first event is a key-down of `name`.
fn downs_of(script: &ShellScript, name: &str) -> Vec<u32> {
    script
        .events
        .iter()
        .filter_map(|e| match e {
            crate::shell_common::Event::Key(k) if k.name == name && k.kind == Kind::Down => {
                Some(k.frame)
            }
            _ => None,
        })
        .collect()
}

/// The first frame at or after `from` whose `state8` differs between two runs' `d` lines, as
/// (frame, ticks after `from`).
fn first_state_divergence(a: &Run, b: &Run, from: u32) -> Option<(u32, u32)> {
    a.details
        .iter()
        .zip(&b.details)
        .skip(from as usize)
        .find(|(x, y)| x.split_whitespace().nth(5) != y.split_whitespace().nth(5))
        .map(|(x, _)| {
            let f: u32 = x.split_whitespace().nth(1).unwrap().parse().unwrap();
            (f, f - from)
        })
}

/// A restore frame: the first `L` frame of a visit (`upd M`) inside Levels with the cursor on
/// `water_stage`.
fn restore_frame(run: &Run) -> Option<usize> {
    (0..run.lines.len()).find(|&f| {
        upd_top(run, f) == ('M', 'L')
            && run.ledger.selector[f]
                .as_ref()
                .is_some_and(|(v, row)| v.folder == LEVELS && row == "water_stage")
    })
}

/// The late-preview witnesses (T0 P3): each frozen change at frame `f` shows in the surface
/// at `f + 1` and not at `f`. Returns the preview rows in order.
fn late_previews(run: &Run) -> Result<Vec<String>, String> {
    let r = &run.ledger.preview_rect;
    for (f, row) in &run.ledger.previews {
        let f = *f as usize;
        need(
            r[f].1 != r[f].0 && r.get(f + 1).is_some_and(|n| n.1 == r[f].0),
            &format!("the preview of {row} lands one frame late (frame {f})"),
        )?;
    }
    Ok(run.ledger.previews.iter().map(|p| p.1.clone()).collect())
}

/// The LEVEL values after the boot's.
fn level_values(run: &Run) -> Vec<&str> {
    run.ledger
        .level_values
        .iter()
        .map(|(_, v)| v.as_str())
        .collect()
}

/// The case's witnesses (plan T5 Step 4, as amended by Addendum T0), as summary lines; `Err`
/// names the first missing one.
pub fn witnesses(case: &Case, run: &Run) -> Result<Vec<String>, String> {
    let l = &run.ledger;
    let mut out = Vec::new();
    need(l.violations.is_empty(), "no violations")?;
    need(l.quit && !run.files.is_empty(), "QUIT and file lines")?;
    match case.name {
        "level_tree" => {
            need(l.tops.contains(&'L'), "an L top")?;
            let rows = late_previews(run)?;
            need(rows.len() >= 6, "six previews")?;
            let tiny = rows
                .iter()
                .position(|r| r == "tiny")
                .ok_or("no tiny preview")?;
            need(
                tiny > 0 && rows[tiny - 1] == "see_shadow_test",
                "tiny previewed after a 504x350 level",
            )?;
            // The Esc frame (upd L, top M): the next frame's surface shows the last preview.
            let esc = (0..run.lines.len())
                .find(|&f| upd_top(run, f) == ('L', 'M'))
                .ok_or("no Esc from the selector")?;
            let r = &l.preview_rect;
            need(
                r[esc + 1].1 == r[esc].0 && r[esc].0 != r[0].0,
                "the main menu shows the last preview",
            )?;
            need(
                l.selector
                    .iter()
                    .flatten()
                    .any(|(v, _)| v.folder == format!("{ROOT}/Profiles")),
                "an empty-folder visit",
            )?;
            let i = *downs_of(&case.script, "I").first().ok_or("no I")? as usize;
            let before = l.selector[i - 1].as_ref().map(|x| x.1.clone());
            let after = l.selector[i].as_ref().map(|x| x.1.clone());
            need(
                before.as_deref() == Some("alpha") && after.as_deref() == Some(".hidden"),
                "the search `hi` moved the cursor from alpha to .hidden",
            )?;
            out.push(format!("previews {:?}", l.previews));
            out.push(format!("search: {before:?} -> {after:?} on frame {i}"));
        }
        "level_pick" => {
            need(l.new_games == 2, "two NEW GAMEs")?;
            need(
                l.level_from_file == [true, false],
                "the file played, then a generated level",
            )?;
            let r = restore_frame(run).ok_or("no restore frame")?;
            need(
                l.level_picks.len() == 2
                    && l.level_picks[0].1 == format!("{LEVELS}/water_stage.lev")
                    && l.level_picks[1].1 == "[RANDOM]",
                "the picks water_stage, then [RANDOM]",
            )?;
            out.push(format!("restore frame {r}; picks {:?}", l.level_picks));
            out.push(format!(
                "level_from_file per NEW GAME {:?}",
                l.level_from_file
            ));
        }
        "level_missing" => {
            need(l.new_games == 2, "two NEW GAMEs")?;
            need(
                !l.boot_from_file && l.level_from_file == [false, false],
                "the boot and both NEW GAMEs random",
            )?;
            need(
                level_values(run) == ["\"gone\"", "\"broken\""],
                "the LEVEL value gone, then broken",
            )?;
            // No preview on `broken` or `trunc`: the frozen screen never changed on their rows.
            need(
                !l.previews
                    .iter()
                    .any(|(_, r)| r == "broken" || r == "trunc"),
                "no preview on broken or trunc",
            )?;
            let visited = |name: &str| {
                l.selector
                    .iter()
                    .flatten()
                    .any(|(v, r)| v.folder == LEVELS && r == name)
            };
            need(
                visited("broken") && visited("trunc"),
                "the cursor on broken and trunc",
            )?;
            out.push(format!(
                "LEVEL values {:?}; previews {:?}",
                l.level_values, l.previews
            ));
        }
        "setup_save" => {
            need(l.reserved_boxes == 2, "two reserved boxes")?;
            need(l.saves == ["mine"], "one save (mine)")?;
            need(l.save_cancels == 1, "one cancel")?;
            need(run.setup_name == "mine", "the name mine")?;
            need(
                run.files.len() == 2
                    && run.files[0].starts_with("file Setups/liero.cfg ")
                    && run.files[1].starts_with("file Setups/mine.cfg "),
                "the file lines liero.cfg and mine.cfg",
            )?;
            out.extend(run.files.iter().cloned());
        }
        "setup_load" => {
            need(l.new_games == 2 && l.resumes == 1, "2 NEW GAMEs, 1 RESUME")?;
            need(
                l.loads.iter().map(|x| x.1.as_str()).collect::<Vec<_>>() == ["orbmit", "mine"],
                "two loads (orbmit, mine)",
            )?;
            need(
                l.selector.iter().flatten().any(|(v, r)| {
                    v.top == 'P' && v.folder == format!("{ROOT}/Setups") && r == "liero"
                }),
                "a P frame inside Setups on liero",
            )?;
            need(
                run.settings.game_mode == GM_GAME_OF_TAG && run.settings.time_to_lose == 120,
                "Game of Tag from mine",
            )?;
            let r = *l.resume_frames.first().ok_or("no RESUME")?;
            let cf = drive_with(
                &case.script,
                None,
                Opts {
                    load_detach: false,
                    ..Opts::default()
                },
            );
            let (f, tick) = first_state_divergence(run, &cf, r)
                .ok_or("load_detach = false leaves the resumed state8 sequence unchanged")?;
            out.push(format!(
                "counterfactual load_detach=false: state8 diverges at frame {f}, {tick} frames \
                 after the RESUME route at {r}"
            ));
            out.push(format!("loads {:?}", l.loads));
            out.extend(run.files.iter().cloned());
        }
        "setups_and_levels" => {
            need(
                l.new_games == 2 && l.level_from_file == [true, false],
                "2 NEW GAMEs: the file, then random",
            )?;
            let r = restore_frame(run).ok_or("no restore frame")?;
            need(l.reserved_boxes == 1, "one reserved box")?;
            need(l.saves == ["mine"], "one save (mine)")?;
            need(
                l.loads.iter().map(|x| x.1.as_str()).collect::<Vec<_>>() == ["orbmit"],
                "one load (orbmit)",
            )?;
            need(
                run.files.len() == 3
                    && run.files[0].starts_with("file Setups/liero.cfg ")
                    && run.files[1].starts_with("file Setups/mine.cfg ")
                    && run.files[2].starts_with("file TC/openliero/Levels/water_stage.lev "),
                "the file lines liero.cfg, mine.cfg and the water_stage copy",
            )?;
            out.push(format!(
                "restore frame {r}; picks {:?}; loads {:?}",
                l.level_picks, l.loads
            ));
            out.extend(run.files.iter().cloned());
        }
        other => panic!("unknown e-2 case {other}"),
    }
    Ok(out)
}
