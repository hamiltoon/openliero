//! Step 4½e-1 G2e-1 — the shell corpus for the settings menu (plan Task 8; design §6): the
//! eleven e-1 cases (the plan's ten plus `key_edges`, Batch 5's key-edge gate), their sidecar
//! files, and the per-case witnesses the generator (`gen_slice4_5e1_shell`) and the gate
//! (`shell_golden.rs`) check. The script model, the driver and the validators are
//! `shell_common`'s; the C++ side is `oracle_dump_shell` (`gen_shell_golden.sh`).
#![allow(dead_code)]

use std::path::Path;

use scenario::settings::{Settings, GM_GAME_OF_TAG};
use scenario::settings_toml::settings_to_toml;
use ui::text::UiTc;

use crate::shell_common::{
    both_done, drive_with, play, EntryOutcome, Kind, Opts, Run, ShellScript, B, TC_ROOT,
};

/// The e-1 cases, in corpus order.
pub const NAMES: [&str; 11] = [
    "settings_nav",
    "settings_edit",
    "int_entry",
    "weapon_options",
    "map_size",
    "live_settings",
    "labels",
    "cfg_boot",
    "cfg_default",
    "match_setup",
    "key_edges",
];

/// One e-1 case: its script and the golden-dir files it reads (setup sidecars, `fs` manifests,
/// the user `liero.cfg`s a manifest copies), as (file name, text).
pub struct Case {
    pub name: &'static str,
    pub script: ShellScript,
    pub files: Vec<(String, String)>,
}

fn tc() -> UiTc {
    UiTc::load(Path::new(TC_ROOT))
}

/// WEAPON OPTIONS' row of weapon `name` (`weap_order` order).
fn row(tc: &UiTc, name: &str) -> usize {
    tc.weapon_names
        .iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("no weapon {name}"))
}

/// A player pick naming `name`: `WormSettings::weapons` holds `weap_order` rows + 1
/// (`weapsel.cpp:66`).
fn pick(tc: &UiTc, name: &str) -> u32 {
    row(tc, name) as u32 + 1
}

/// `weap_table[weap_order[row(name)]]`'s index.
fn index(tc: &UiTc, name: &str) -> usize {
    tc.weap_order[row(tc, name)]
}

/// A `weapTable` where only `names` are in the menu (0) and every other weapon is banned (2).
fn only(tc: &UiTc, names: &[&str]) -> [u32; 40] {
    let mut t = [2; 40];
    for n in names {
        t[index(tc, n)] = 0;
    }
    t
}

fn toml_array(v: &[u32]) -> String {
    let items: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("[ {} ]", items.join(", "))
}

fn picks(p: u32) -> String {
    toml_array(&[p; 5])
}

/// Enter on the current settings row, then (on the entry) BACKSPACE `bs` times and type `digits`,
/// then Return. The entry is on top from the Enter frame + 1.
fn entry(b: B, bs: u32, digits: &str) -> B {
    b.tap("RETURN")
        .idle(2)
        .taps_n("BACKSPACE", bs)
        .type_digits(digits)
        .tap("RETURN")
        .idle(3)
}

/// Esc in the menu (the cursor to QUIT TO OS), then Return: the quit (4½d's ending).
fn quit(b: B) -> ShellScript {
    b.tap("ESC").idle(5).tap("RETURN").end(40, true)
}

fn settings_nav() -> Case {
    // Enter on MATCH SETUP (Up wraps NEW GAME -> MATCH SETUP), Up/Down across the wrap over the
    // hidden rows, P1's R/F, held Down with OS repeats, Esc (the main cursor stays on MATCH SETUP),
    // F7 (the settings cursor kept), PgDn/PgUp, then F1 from the settings focus: NEW GAME ->
    // selection -> Esc -> menu -> QUIT.
    let b = B::new(None, 21)
        .detail()
        .idle(40)
        .tap("UP")
        .tap("RETURN")
        .idle(5)
        .taps_n("UP", 3)
        .taps_n("DOWN", 5)
        .tap("R")
        .tap("F")
        .tap("F")
        .repeats("DOWN", 15, 3, 4)
        .idle(3)
        .tap("ESC")
        .idle(5)
        .tap("F7")
        .idle(3)
        .tap("PAGEDOWN")
        .tap("PAGEDOWN")
        .tap("PAGEUP")
        .idle(5)
        .seed(2101)
        .tap("F1")
        .after_menu_select();
    let b = b.idle(15).tap("ESC").after_esc().idle(10);
    Case {
        name: "settings_nav",
        script: quit(b),
        files: Vec::new(),
    }
}

fn settings_edit() -> Case {
    // Rows (Kill'em All): GAME MODE 0, LIVES 1, LEVEL 2, MAP WIDTH 3, MAP HEIGHT 4, LOADING TIMES 5,
    // WEAPON OPTIONS 6, MAX BONUSES 7, NAMES ON BONUSES 8, MAP 9, AMOUNT OF BLOOD 10,
    // LOAD+CHANGE 11, REGENERATE LEVEL 12, SAVE SETUP AS... 13, LOAD SETUP 14.
    let b = B::new(None, 22)
        .detail()
        .idle(40)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 5)
        .hold("RIGHT", 40)
        .idle(3)
        .hold("LEFT", 25)
        .idle(3)
        .taps_n("UP", 5)
        // GAME MODE Enter: Game of Tag; TIME TO LOSE held Right.
        .tap("RETURN")
        .tap("DOWN")
        .hold("RIGHT", 30)
        .idle(3)
        .tap("UP")
        // Holdazone: 16 rows (the scrollbar), TIME TO WIN and ZONE TIMEOUT; Up wraps to the last
        // row (the view scrolls) and Down wraps back.
        .tap("RETURN")
        .idle(5)
        .taps_n("DOWN", 4)
        .taps_n("UP", 4)
        .tap("UP")
        .idle(5)
        .tap("DOWN")
        // Scales of Justice, then Kill'em All again.
        .tap("RETURN")
        .idle(3)
        .tap("RETURN")
        .idle(3)
        // NAMES ON BONUSES: held Left toggles once (the behavior releases Left), a re-press
        // toggles again.
        .taps_n("DOWN", 8)
        .hold("LEFT", 20)
        .idle(3)
        .hold("LEFT", 10)
        .idle(3)
        // MAP Enter, AMOUNT OF BLOOD Enter (the sound only), REGENERATE LEVEL Enter.
        .tap("DOWN")
        .tap("RETURN")
        .tap("DOWN")
        .tap("RETURN")
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .idle(20);
    Case {
        name: "settings_edit",
        script: b.end(0, false),
        files: Vec::new(),
    }
}

fn int_entry() -> Case {
    let b = B::new(None, 23).detail().idle(40).tap("F7").idle(3);
    // LIVES: `7`, BACKSPACE, `42` (the third digit is dropped), Return.
    let b = b
        .tap("DOWN")
        .tap("RETURN")
        .idle(2)
        .type_digits("7")
        .tap("BACKSPACE")
        .type_digits("42")
        .tap("RETURN")
        .idle(3);
    // MAP WIDTH: 9999 -> 4096, 0 -> 64, then 333.
    let b = b.taps_n("DOWN", 2);
    let b = entry(b, 3, "9999");
    let b = entry(b, 4, "0");
    let b = entry(b, 2, "333");
    // MAX BONUSES: `5`, then Esc (cancelled).
    let b = b
        .taps_n("DOWN", 4)
        .tap("RETURN")
        .idle(2)
        .type_digits("5")
        .tap("ESC")
        .idle(3);
    // LOADING TIMES: BACKSPACE x3, Return (the empty entry keeps the value).
    let b = b
        .taps_n("UP", 2)
        .tap("RETURN")
        .idle(2)
        .taps_n("BACKSPACE", 3)
        .tap("RETURN")
        .idle(3);
    // LOADING TIMES again: P1's R and an `r` typed into the entry (filtered, never leaks), then
    // `5` and Return.
    let b = b
        .tap("RETURN")
        .idle(2)
        .at(0, Kind::Down, "R")
        .text(b"r")
        .at(2, Kind::Up, "R")
        .idle(3)
        .type_digits("5")
        .tap("RETURN")
        .idle(3)
        .taps_n("DOWN", 2)
        .idle(10);
    Case {
        name: "int_entry",
        script: b.end(0, false),
        files: Vec::new(),
    }
}

fn weapon_options(tc: &UiTc) -> Case {
    // Only LARPA is in the menu; both players' picks are LARPA.
    let setup = format!(
        "[player1]\nweapons = {p}\n\n[player2]\nweapons = {p}\n\n[settings]\nversion = 6\n\
         weapTable = {}\n",
        toml_array(&only(tc, &["LARPA"])),
        p = picks(pick(tc, "LARPA")),
    );
    let name = "shell_weapon_options_setup.cfg";
    let b = B::new(Some(name), 24)
        .detail()
        .idle(40)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 6)
        .tap("RETURN")
        .idle(3)
        // 40 rows: Down x20, PgDn, PgUp; held Left then held Right act once each.
        .taps_n("DOWN", 20)
        .tap("PAGEDOWN")
        .tap("PAGEUP")
        .hold("LEFT", 8)
        .idle(2)
        .hold("RIGHT", 8)
        .idle(2)
        // The search, one visit: `L`, `A` -> LARPA. Ban it: every weapon is out of the menu, so
        // Esc raises the NoWeaps box; any key dismisses it; Right re-enables LARPA; Esc closes.
        .tap("L")
        .tap("A")
        .tap("LEFT")
        .tap("ESC")
        .idle(3)
        .tap("SPACE")
        .idle(3)
        .tap("RIGHT")
        .tap("ESC")
        .idle(3)
        .tap("ESC")
        .idle(3)
        // NEW GAME (F1 from the main focus): selection over LARPA only; P1's cursor to slot 1;
        // Esc during the selection.
        .seed(2401)
        .tap("F1")
        .after_menu_select()
        .idle(5)
        .tap("F")
        .idle(5)
        .tap("ESC")
        .after_esc()
        .idle(10)
        // F7 -> WEAPON OPTIONS -> enable LASER (the row under LARPA) -> Esc -> Esc -> F1 (RESUME).
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 6)
        .tap("RETURN")
        .idle(3)
        .tap("L")
        .tap("A")
        .tap("DOWN")
        .tap("RIGHT")
        .tap("ESC")
        .idle(3)
        .tap("ESC")
        .idle(3)
        .tap("F1")
        .after_menu_select()
        // P1 Right: slot 1 cycles LARPA -> LASER through the live weap_table (fact 15); then
        // DONE (P1 slot 1 -> RANDOMIZE -> DONE, P2 RANDOMIZE -> DONE), both Fire.
        .idle(5)
        .tap("G")
        .idle(5)
        .taps(&["R", "UP"])
        .tap("R")
        .taps(&["LCTRL", "RCTRL"])
        .idle(60)
        .tap("ESC")
        .after_esc();
    Case {
        name: "weapon_options",
        script: quit(b),
        files: vec![(name.to_string(), setup)],
    }
}

fn map_size() -> Case {
    // Heights >= 342 keep C++ spawning inside the level (Addendum G3).
    let b = B::new(None, 25)
        .detail()
        .idle(40)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 3);
    let b = entry(b, 3, "333").tap("DOWN");
    let b = entry(b, 3, "360")
        .tap("ESC")
        .idle(3)
        .seed(2501)
        .tap("F1")
        .after_menu_select();
    let b = play(both_done(b), 300).tap("ESC").after_esc().idle(10);
    let b = b.tap("F7").idle(3).taps_n("DOWN", 3);
    let b = entry(b, 3, "184").tap("DOWN");
    let b = entry(b, 3, "420")
        .tap("ESC")
        .idle(3)
        // Main focus on MATCH SETUP: Down wraps to RESUME, Down to NEW GAME.
        .taps_n("DOWN", 2)
        .seed(2502)
        .tap("RETURN")
        .after_menu_select();
    let b = play(both_done(b), 300).tap("ESC").after_esc();
    Case {
        name: "map_size",
        script: quit(b),
        files: Vec::new(),
    }
}

/// The edits of `live_settings` and `match_setup` reach a paused match only through RESUME's
/// resync (finding 1).
fn live_settings() -> Case {
    let b = B::new(None, 26)
        .detail()
        .idle(40)
        .seed(2601)
        .tap("RETURN")
        .after_menu_select();
    let b = play(both_done(b), 200).tap("ESC").after_esc().idle(10);
    let b = b.tap("F7").idle(3).taps_n("DOWN", 5);
    // LOADING TIMES `0`; AMOUNT OF BLOOD held Right to 300; MAX BONUSES `20`; MAP Enter; NAMES ON
    // BONUSES Enter; GAME MODE -> Game of Tag.
    let b = entry(b, 3, "0")
        .taps_n("DOWN", 5)
        .hold("RIGHT", BLOOD_HOLD)
        .idle(3)
        .taps_n("UP", 3);
    let b = entry(b, 1, "20")
        .taps_n("DOWN", 2)
        .tap("RETURN")
        .tap("UP")
        .tap("RETURN")
        .taps_n("UP", 8)
        .tap("RETURN")
        .tap("ESC")
        .idle(3)
        .tap("F1")
        .after_menu_select();
    let b = play(b, 300).tap("ESC").after_esc();
    Case {
        name: "live_settings",
        script: quit(b),
        files: Vec::new(),
    }
}

/// Frames of held Right on AMOUNT OF BLOOD that take it from 100 to 300 (the behavior's
/// cadence; `live_settings`' witness checks the value).
const BLOOD_HOLD: u32 = 41;

/// `labels`: its script for match seed `seed` (a weapon bonus must drop while it plays; the drop
/// chance is 1/1700 a tick, so the generator searches the seed, [`find_labels_seed`]).
fn labels(tc: &UiTc, seed: u32) -> Case {
    let setup = format!(
        "[player1]\nweapons = {}\n\n[settings]\nversion = 6\nmaxBonuses = 8\nnamesOnBonuses = true\n",
        picks(pick(tc, "BOOBY TRAP")),
    );
    let name = "shell_labels_setup.cfg";
    let mut b = both_done(
        B::new(Some(name), 27)
            .detail()
            .idle(40)
            .seed(seed)
            .tap("RETURN")
            .after_menu_select(),
    );
    // P1 fires booby traps, P1 holds Change (LSHIFT) and P2 holds Change (RALT) for stretches;
    // everything is released before the Esc.
    for _ in 0..8 {
        b = b.tap("LCTRL").idle(12);
    }
    b = b.hold("LSHIFT", 40).idle(5).hold("RALT", 40).idle(5);
    b = b
        .at(0, Kind::Down, "LSHIFT")
        .at(0, Kind::Down, "RALT")
        .at(30, Kind::Up, "LSHIFT")
        .at(30, Kind::Up, "RALT")
        .idle(35);
    for _ in 0..6 {
        b = b.tap("LCTRL").idle(12);
    }
    let b = b.idle(250).tap("ESC").after_esc();
    Case {
        name: "labels",
        script: quit(b),
        files: vec![(name.to_string(), setup)],
    }
}

fn cfg_boot() -> Case {
    let mut s = Settings::default();
    s.game_mode = GM_GAME_OF_TAG;
    s.time_to_lose = 120;
    s.random_map_width = 400;
    s.random_map_height = 300;
    s.names_on_bonuses = true;
    s.random_level = true;
    let user = "shell_cfg_boot_user_liero.cfg";
    let manifest = "shell_cfg_boot_fs.txt";
    let fs = format!(
        "# Step 4½e-1 G2e-1 cfg_boot: the shipped setup in the system layer, the user's own\n\
         # liero.cfg (Game of Tag, timeToLose 120, 400x300, namesOnBonuses) over it.\n\
         dir sys Profiles\ndir sys Resources\nfile sys Setups/liero.cfg data/Setups/liero.cfg\n\
         file user Setups/liero.cfg rust/oracle-tests/golden/{user}\n"
    );
    // Rows (Game of Tag): GAME MODE 0, TIME TO LOSE 1, LEVEL 2, MAP WIDTH 3, MAP HEIGHT 4,
    // LOADING TIMES 5.
    let b = B::new(None, 28)
        .fs(manifest)
        .detail()
        .idle(40)
        .tap("F7")
        .idle(3)
        .taps_n("DOWN", 5)
        .hold("RIGHT", 20)
        .idle(3)
        .tap("ESC")
        .idle(3);
    Case {
        name: "cfg_boot",
        script: quit(b),
        files: vec![
            (user.to_string(), settings_to_toml(&s)),
            (manifest.to_string(), fs),
        ],
    }
}

fn cfg_default() -> Case {
    let manifest = "shell_cfg_default_fs.txt";
    let fs =
        "# Step 4½e-1 G2e-1 cfg_default: no liero.cfg in either layer, so the boot saves the\n\
              # defaults to the user layer (gameEntry.cpp:55-58).\n\
              dir sys Profiles\ndir sys Resources\n"
            .to_string();
    let b = B::new(None, 29).fs(manifest).detail().idle(40);
    Case {
        name: "cfg_default",
        script: quit(b),
        files: vec![(manifest.to_string(), fs)],
    }
}

fn match_setup(tc: &UiTc) -> Case {
    let mut s = Settings::default();
    s.weap_table = only(tc, &["SHOTGUN"]);
    for w in &mut s.worm_settings[..2] {
        w.weapons = [pick(tc, "SHOTGUN"); 5];
    }
    s.random_level = true;
    let user = "shell_match_setup_user_liero.cfg";
    let manifest = "shell_match_setup_fs.txt";
    let fs = format!(
        "# Step 4½e-1 G2e-1 match_setup (the milestone): the shipped setup in the system layer, the\n\
         # user's liero.cfg (SHOTGUN the only weapon in the menu, both players' picks on it) over it.\n\
         dir sys Profiles\ndir sys Resources\nfile sys Setups/liero.cfg data/Setups/liero.cfg\n\
         file user Setups/liero.cfg rust/oracle-tests/golden/{user}\n"
    );
    // GAME MODE Enter x3: Kill'em All -> Game of Tag -> Holdazone -> Scales of Justice. Rows
    // (Scales): GAME MODE 0, LIVES 1, LEVEL 2, MAP WIDTH 3, MAP HEIGHT 4, LOADING TIMES 5,
    // WEAPON OPTIONS 6, MAX BONUSES 7, ..., AMOUNT OF BLOOD 10, ..., LOAD SETUP 14.
    let b = B::new(None, 30)
        .fs(manifest)
        .detail()
        .idle(40)
        .tap("F7")
        .idle(3)
        .taps_n("RETURN", 3)
        .tap("DOWN");
    let b = entry(b, 2, "3")
        .taps_n("DOWN", 4)
        .hold("RIGHT", 20)
        .idle(3)
        .taps_n("UP", 2);
    let b = entry(b, 3, "333").tap("DOWN");
    // Heights >= 342 keep C++ spawning inside the level (Addendum G3).
    let b = entry(b, 3, "352")
        .taps_n("DOWN", 2)
        // WEAPON OPTIONS: the search to SHOTGUN, ban it, Esc -> the NoWeaps box -> any key ->
        // re-enable -> Esc -> Esc.
        .tap("RETURN")
        .idle(3)
        .tap("S")
        .tap("H")
        .tap("LEFT")
        .tap("ESC")
        .idle(3)
        .tap("SPACE")
        .idle(3)
        .tap("RIGHT")
        .tap("ESC")
        .idle(3)
        .tap("ESC")
        .idle(3)
        .seed(3001)
        .tap("F1")
        .after_menu_select();
    let b = play(both_done(b), 300).tap("ESC").after_esc().idle(10);
    // AMOUNT OF BLOOD (held) and MAX BONUSES (typed `0`), Esc, F1 (RESUME).
    let b = b
        .tap("F7")
        .idle(3)
        .taps_n("UP", 5)
        .hold("RIGHT", 20)
        .idle(3)
        .taps_n("UP", 3);
    let b = entry(b, 1, "0")
        .tap("ESC")
        .idle(3)
        .tap("F1")
        .after_menu_select();
    let b = play(b, 200).tap("ESC").after_esc();
    Case {
        name: "match_setup",
        script: quit(b),
        files: vec![
            (user.to_string(), settings_to_toml(&s)),
            (manifest.to_string(), fs),
        ],
    }
}

/// Batch 5's key-edge gate against the real C++ (true key-event edges): Change held with
/// Right/Left taps steps one weapon per tap; Change + Jump held throws the rope once; Fire held
/// from a dead worm's ready press through the respawn, then released and pressed again.
fn key_edges(tc: &UiTc) -> Case {
    let p1 = toml_array(&[
        pick(tc, "BAZOOKA"),
        pick(tc, "SHOTGUN"),
        pick(tc, "LARPA"),
        pick(tc, "GRENADE"),
        pick(tc, "UZI"),
    ]);
    let setup = format!(
        "[player1]\nhealth = 1\nweapons = {p1}\n\n[player2]\nhealth = 1\n\n[settings]\nversion = 6\n"
    );
    let name = "shell_key_edges_setup.cfg";
    let b = both_done(
        B::new(Some(name), 31)
            .detail()
            .idle(40)
            .seed(3101)
            .tap("RETURN")
            .after_menu_select(),
    )
    // Both worms drop in first (visible from about 225 match frames).
    .idle(240);
    // P1: Change held, Right, Right, Left; P2: Change held, Right, Left.
    let b = b
        .at(0, Kind::Down, "LSHIFT")
        .at(4, Kind::Down, "G")
        .at(6, Kind::Up, "G")
        .at(10, Kind::Down, "G")
        .at(12, Kind::Up, "G")
        .at(16, Kind::Down, "D")
        .at(18, Kind::Up, "D")
        .at(24, Kind::Up, "LSHIFT")
        .idle(28)
        .at(0, Kind::Down, "RALT")
        .at(4, Kind::Down, "RIGHT")
        .at(6, Kind::Up, "RIGHT")
        .at(10, Kind::Down, "LEFT")
        .at(12, Kind::Up, "LEFT")
        .at(18, Kind::Up, "RALT")
        .idle(22);
    // P1: Change + Jump held (the rope is thrown once), then Jump alone (the rope released).
    let b = b
        .at(0, Kind::Down, "LSHIFT")
        .at(1, Kind::Down, "LALT")
        .at(30, Kind::Up, "LALT")
        .at(31, Kind::Up, "LSHIFT")
        .idle(40)
        .tap("LALT")
        .idle(40);
    // P1 (health 1) aims down and fires the bazooka at its feet, dies; Fire held from the ready
    // press through the respawn; released; pressed again.
    let b = b
        .hold("F", 25)
        .tap("LCTRL")
        .idle(KEY_EDGES_DEAD_WAIT)
        .hold("LCTRL", KEY_EDGES_FIRE_HOLD)
        .idle(20)
        .tap("LCTRL")
        .idle(40)
        .tap("ESC")
        .after_esc();
    Case {
        name: "key_edges",
        script: quit(b),
        files: vec![(name.to_string(), setup)],
    }
}

/// `key_edges`: frames from the fatal shot to the ready press, and how long Fire is held.
const KEY_EDGES_DEAD_WAIT: u32 = 60;
const KEY_EDGES_FIRE_HOLD: u32 = 150;

/// The first match seed from 2701 for which `labels` shows every label witness (a frame-0 weapon
/// bonus, a booby trap at `cur_frame` 0, both players' Change) with no violation.
pub fn find_labels_seed() -> u32 {
    let tc = tc();
    for seed in 2701..=2900 {
        let run = crate::shell_common::drive(&labels(&tc, seed).script, None);
        let l = &run.ledger;
        if l.violations.is_empty()
            && l.bonus_labels > 0
            && l.booby_labels > 0
            && l.change_labels.iter().all(|&n| n > 0)
        {
            return seed;
        }
    }
    panic!("no labels seed in 2701..=2900: play longer (plan T8)");
}

/// The e-1 cases, in [`NAMES`] order; `labels_seed` from [`find_labels_seed`] (the gate reads it
/// back from the committed script).
pub fn cases(labels_seed: u32) -> Vec<Case> {
    let tc = tc();
    vec![
        settings_nav(),
        settings_edit(),
        int_entry(),
        weapon_options(&tc),
        map_size(),
        live_settings(),
        labels(&tc, labels_seed),
        cfg_boot(),
        cfg_default(),
        match_setup(&tc),
        key_edges(&tc),
    ]
}

/// Whether `name` is held on `frame`'s sampled word (its last event at or before `frame`).
fn held_at(script: &ShellScript, name: &str, frame: u32) -> bool {
    let mut on = false;
    for e in &script.events {
        if let crate::shell_common::Event::Key(k) = e {
            if k.frame > frame {
                break;
            }
            if k.name == name {
                on = k.kind != Kind::Up;
            }
        }
    }
    on
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

/// The resume counterfactual (known pitfall 19): with `resume_sync = false` the `state8`
/// sequence diverges after the RESUME.
fn resume_witness(case: &Case, run: &Run, out: &mut Vec<String>) -> Result<(), String> {
    let r = *run.ledger.resume_frames.first().ok_or("no RESUME")?;
    let cf = drive_with(
        &case.script,
        None,
        Opts {
            resume_sync: false,
            ..Opts::default()
        },
    );
    let (f, tick) = first_state_divergence(run, &cf, r)
        .ok_or("resume_sync = false leaves the resumed state8 sequence unchanged")?;
    out.push(format!(
        "counterfactual resume_sync=false: state8 diverges at frame {f}, {tick} frames after the RESUME route at {r}"
    ));
    Ok(())
}

fn need(ok: bool, what: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("witness missing: {what}"))
    }
}

/// The case's witnesses (plan T8 Step 4), as summary lines; `Err` names the first missing one.
pub fn witnesses(case: &Case, run: &Run) -> Result<Vec<String>, String> {
    let l = &run.ledger;
    let mut out = Vec::new();
    need(l.violations.is_empty(), "no violations")?;
    match case.name {
        "settings_nav" => {
            need(
                l.new_games == 1 && l.menus == 1 && l.quit,
                "NEW GAME, Esc, QUIT",
            )?;
        }
        "settings_edit" => {
            need(l.new_games == 0, "no NEW GAME")?;
        }
        "int_entry" => {
            let has = |o: EntryOutcome| l.entries.iter().any(|e| e.outcome == o);
            need(has(EntryOutcome::Accepted), "an accepted entry")?;
            need(has(EntryOutcome::Cancelled), "a cancelled entry")?;
            need(has(EntryOutcome::Empty), "an empty entry")?;
            let clamp = |v: &str| {
                l.entries
                    .iter()
                    .any(|e| e.item == "MAP WIDTH" && e.value == v)
            };
            need(clamp("4096"), "the clamp to 4096")?;
            need(clamp("64"), "the clamp to 64")?;
            for e in &l.entries {
                out.push(format!(
                    "entry frame {} {} {:?} -> {}",
                    e.frame, e.item, e.outcome, e.value
                ));
            }
        }
        "weapon_options" => {
            let tc = tc();
            need(l.tops.contains(&'O'), "a WEAPON OPTIONS visit")?;
            need(l.weapon_boxes >= 1, "the NoWeaps box")?;
            let larpa = index(&tc, "LARPA");
            // The held Left / Right act once each on the row the cursor reached, then the
            // search moves the cursor to LARPA, whose ban is the next change.
            let c = &l.weap_changes;
            need(c.len() >= 3, "three weap_table changes")?;
            need(
                c[0].1 == c[1].1 && c[0].1 != larpa && (c[0].2, c[0].3, c[1].3) == (2, 1, 2),
                "one change per held Left / Right on a non-LARPA row",
            )?;
            need(
                c[2].1 == larpa && (c[2].2, c[2].3) == (0, 2),
                "the search moved the cursor to LARPA (its ban)",
            )?;
            out.push(format!("weap_table changes {c:?}"));
            let cf = drive_with(
                &case.script,
                None,
                Opts {
                    resume_sync: false,
                    ..Opts::default()
                },
            );
            need(
                cf.p1_weapons != run.p1_weapons,
                "resume_sync=false changes P1's weapons after the RESUME cycle",
            )?;
            out.push(format!(
                "counterfactual resume_sync=false: P1 weapons {:?} vs {:?}",
                cf.p1_weapons, run.p1_weapons
            ));
        }
        "map_size" => {
            need(
                l.level_sizes == [(333, 360), (184, 420)],
                "both level sizes played",
            )?;
            out.push(format!("level sizes {:?}", l.level_sizes));
        }
        "live_settings" => {
            need(l.resumes == 1, "one RESUME")?;
            let s = &run.settings;
            need(
                (s.loading_time, s.blood, s.max_bonuses, s.game_mode)
                    == (0, 300, 20, GM_GAME_OF_TAG),
                "LOADING TIMES 0, AMOUNT OF BLOOD 300, MAX BONUSES 20, Game of Tag",
            )?;
            out.push(format!(
                "edited: loading {} blood {} max bonuses {} map {} names {} mode {}",
                s.loading_time, s.blood, s.max_bonuses, s.map, s.names_on_bonuses, s.game_mode
            ));
            resume_witness(case, run, &mut out)?;
        }
        "labels" => {
            let cf = drive_with(
                &case.script,
                None,
                Opts {
                    small_labels: false,
                    ..Opts::default()
                },
            );
            let diff = run
                .lines
                .iter()
                .zip(&cf.lines)
                .filter(|(a, b)| a != b)
                .count();
            need(diff > 0, "small_labels=false changes a presented frame")?;
            need(l.bonus_labels > 0, "a frame-0 bonus under namesOnBonuses")?;
            need(l.booby_labels > 0, "a booby trap at cur_frame 0")?;
            need(
                l.change_labels[0] > 0 && l.change_labels[1] > 0,
                "each player visible with Change held",
            )?;
            out.push(format!(
                "counterfactual small_labels=false: {diff} f lines differ; bonus-label frames {}, \
                 booby-label frames {}, change-label frames {:?}",
                l.bonus_labels, l.booby_labels, l.change_labels
            ));
        }
        "cfg_boot" | "cfg_default" => {
            need(l.quit && !run.files.is_empty(), "file lines")?;
            out.extend(run.files.iter().cloned());
        }
        "match_setup" => {
            need(
                (l.new_games, l.resumes, l.quit) == (1, 1, true),
                "1 NEW GAME, 1 RESUME, quit",
            )?;
            need(!run.files.is_empty(), "file lines")?;
            need(l.weapon_boxes >= 1, "the NoWeaps box")?;
            resume_witness(case, run, &mut out)?;
            out.extend(run.files.iter().cloned());
        }
        "key_edges" => key_edges_witnesses(case, run, &mut out)?,
        other => panic!("unknown e-1 case {other}"),
    }
    Ok(out)
}

fn key_edges_witnesses(case: &Case, run: &Run, out: &mut Vec<String>) -> Result<(), String> {
    let s = &case.script;
    let w = |f: usize, i: usize| run.worms[f].map(|x| x[i]);
    // Weapon steps per worm while its Change key is held, before the rope part.
    let steps = |i: usize, key: &str| {
        (1..run.worms.len())
            .filter(|&f| held_at(s, key, f as u32))
            .filter(|&f| {
                matches!((w(f - 1, i), w(f, i)), (Some(a), Some(b)) if a.current_weapon != b.current_weapon)
            })
            .count()
    };
    let (p1, p2) = (steps(0, "LSHIFT"), steps(1, "RALT"));
    need(p1 == 3 && p2 == 2, "one weapon step per tap under Change")?;
    let throws = (1..run.worms.len())
        .filter(
            |&f| matches!((w(f - 1, 0), w(f, 0)), (Some(a), Some(b)) if !a.rope_out && b.rope_out),
        )
        .count();
    need(throws == 1, "the rope thrown once")?;
    let died = (1..run.worms.len())
        .find(|&f| matches!((w(f - 1, 0), w(f, 0)), (Some(a), Some(b)) if a.visible && !b.visible))
        .ok_or("P1 never died")?;
    let ready = (died..run.worms.len())
        .find(|&f| w(f, 0).is_some_and(|x| x.ready))
        .ok_or("P1 never readied")?;
    need(
        held_at(s, "LCTRL", ready as u32),
        "the ready press is the Fire hold",
    )?;
    let back = (ready..run.worms.len())
        .find(|&f| w(f, 0).is_some_and(|x| x.visible))
        .ok_or("P1 never respawned")?;
    need(
        held_at(s, "LCTRL", back as u32),
        "the respawn comes while Fire is held",
    )?;
    out.push(format!(
        "weapon steps P1 {p1} P2 {p2}; rope throws {throws}; P1 died at frame {died}, ready at \
         {ready}, respawned at {back} with Fire held"
    ));
    Ok(())
}
