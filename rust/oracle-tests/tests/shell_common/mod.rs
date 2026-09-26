//! Step 4½d G2 — the shell cases (design §6.4; plan Tasks 8-10): the script model and builder,
//! the case table, the Rust driver that produces the golden lines through `ui::shell::Shell`,
//! and the validators the generator enforces. The C++ side is `oracle_dump_shell`.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;

use render::bitmap::Bitmap;
use render::hash::hash_frame;
use scenario::settings::{Settings, GM_HOLDAZONE};
use scenario::settings_toml::settings_from_toml;
use sim::state::ControlState;
use ui::keys::TypedKey;
use ui::shell::level_slot::SeedSource;
use ui::shell::playing::StartOptions;
use ui::shell::{InputEvent, KeyEvent, Phase, Present, Route, Shell, ShellInput};
use ui::text::UiTc;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

pub const CASES_NAMES: [&str; 11] = [
    "boot_idle",
    "nav",
    "level_file",
    "gametag",
    "holdazone_boot",
    "milestone",
    "esc_in_weapsel",
    "reuse",
    "regenerate",
    "game_over",
    "f1_quit",
];

/// The keys a case may press: nothing the C++ menu would act on where the Rust 4½d menu is inert
/// (plan-time fact 2 — no F-key but F1). R/F/D/G are P1's up/down/left/right.
pub const ALLOWED: [&str; 20] = [
    "ESC", "RETURN", "KP_ENTER", "UP", "DOWN", "LEFT", "RIGHT", "PAGEUP", "PAGEDOWN", "LCTRL",
    "RCTRL", "LALT", "RALT", "LSHIFT", "RSHIFT", "R", "F", "D", "G", "F1",
];

/// DOS index of each letter (`keys.cpp:9-35`).
const LETTERS: [(u8, u32); 26] = [
    (b'Q', 16),
    (b'W', 17),
    (b'E', 18),
    (b'R', 19),
    (b'T', 20),
    (b'Y', 21),
    (b'U', 22),
    (b'I', 23),
    (b'O', 24),
    (b'P', 25),
    (b'A', 30),
    (b'S', 31),
    (b'D', 32),
    (b'F', 33),
    (b'G', 34),
    (b'H', 35),
    (b'J', 36),
    (b'K', 37),
    (b'L', 38),
    (b'Z', 44),
    (b'X', 45),
    (b'C', 46),
    (b'V', 47),
    (b'B', 48),
    (b'N', 49),
    (b'M', 50),
];

/// `SDLToDOSKey` (`keys.cpp:9-75`) of a script key name, and its `key_buf` symbol (only the
/// printable ones matter: nothing in 4½d reads key_buf).
pub fn key_of(name: &str) -> (u32, TypedKey) {
    let none = TypedKey::Sym(0);
    match name {
        "ESC" => (1, none),
        "RETURN" => (28, none),
        "KP_ENTER" => (116, none),
        "UP" => (160, none),
        "DOWN" => (168, none),
        "LEFT" => (163, none),
        "RIGHT" => (165, none),
        "PAGEUP" => (161, none),
        "PAGEDOWN" => (169, none),
        "LCTRL" => (29, none),
        "RCTRL" => (117, none),
        "LALT" => (56, none),
        "RALT" => (144, none),
        "LSHIFT" => (42, none),
        "RSHIFT" => (54, none),
        "SPACE" => (57, TypedKey::Sym(32)),
        "TAB" => (15, TypedKey::Tab),
        "F11" => (87, none),
        "F12" => (88, none),
        f if f.len() > 1 && f.starts_with('F') => {
            (58 + f[1..].parse::<u32>().expect("F1..F10"), none)
        }
        c if c.len() == 1 => {
            let b = c.as_bytes()[0];
            let dos = match b {
                b'1'..=b'9' => (b - b'1') as u32 + 2,
                b'0' => 11,
                _ => LETTERS.iter().find(|(l, _)| *l == b).expect("a letter").1,
            };
            (dos, TypedKey::Sym(b.to_ascii_lowercase() as u32))
        }
        other => panic!("unknown key name {other}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Down,
    Up,
    Repeat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyLine {
    pub frame: u32,
    pub kind: Kind,
    pub name: String,
}

/// A `shell_<case>_script.txt` (format: plan Task 8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellScript {
    /// The setup sidecar's file name, `None` = `default`.
    pub setup: Option<String>,
    pub boot_seed: u32,
    pub match_seeds: Vec<u32>,
    pub frames: u32,
    pub expect_quit: bool,
    pub keys: Vec<KeyLine>,
}

impl ShellScript {
    pub fn parse(text: &str) -> ShellScript {
        let mut s = ShellScript::default();
        for line in text.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            match t.as_slice() {
                [] => {}
                [c, ..] if c.starts_with('#') => {}
                ["setup", f] => s.setup = (*f != "default").then(|| f.to_string()),
                ["boot_seed", v] => s.boot_seed = v.parse().unwrap(),
                ["match_seed", v] => s.match_seeds.push(v.parse().unwrap()),
                ["frames", v] => s.frames = v.parse().unwrap(),
                ["expect", v] => s.expect_quit = *v == "quit",
                ["key", f, k, n] => s.keys.push(KeyLine {
                    frame: f.parse().unwrap(),
                    kind: match *k {
                        "down" => Kind::Down,
                        "up" => Kind::Up,
                        "repeat" => Kind::Repeat,
                        other => panic!("bad key kind {other}"),
                    },
                    name: n.to_string(),
                }),
                other => panic!("bad script line {other:?}"),
            }
        }
        s.keys.sort_by_key(|k| k.frame); // stable: the file order within a frame, as the dumper
        s
    }

    pub fn to_text(&self, header: &[String]) -> String {
        let mut out: String = header.iter().map(|h| format!("# {h}\n")).collect();
        out += &format!("setup {}\n", self.setup.as_deref().unwrap_or("default"));
        out += &format!("boot_seed {}\n", self.boot_seed);
        for m in &self.match_seeds {
            out += &format!("match_seed {m}\n");
        }
        out += &format!(
            "frames {}\nexpect {}\n",
            self.frames,
            if self.expect_quit { "quit" } else { "frames" }
        );
        for k in &self.keys {
            let kind = match k.kind {
                Kind::Down => "down",
                Kind::Up => "up",
                Kind::Repeat => "repeat",
            };
            out += &format!("key {} {kind} {}\n", k.frame, k.name);
        }
        out
    }
}

/// The script builder: a frame cursor `t`; `at` schedules relative to it without moving it.
pub struct B {
    s: ShellScript,
    t: u32,
}

impl B {
    pub fn new(setup: Option<&str>, boot_seed: u32) -> B {
        B {
            s: ShellScript {
                setup: setup.map(str::to_string),
                boot_seed,
                ..ShellScript::default()
            },
            t: 0,
        }
    }

    pub fn at(mut self, dt: u32, kind: Kind, name: &str) -> B {
        self.s.keys.push(KeyLine {
            frame: self.t + dt,
            kind,
            name: name.to_string(),
        });
        self
    }

    pub fn idle(mut self, n: u32) -> B {
        self.t += n;
        self
    }

    /// Down now, up two frames later; the cursor moves 3.
    pub fn tap(self, name: &str) -> B {
        self.at(0, Kind::Down, name).at(2, Kind::Up, name).idle(3)
    }

    /// Several keys down on the same frame (Up + R; both players' DONE).
    pub fn taps(self, names: &[&str]) -> B {
        let mut b = self;
        for n in names {
            b = b.at(0, Kind::Down, n).at(2, Kind::Up, n);
        }
        b.idle(3)
    }

    pub fn hold(self, name: &str, n: u32) -> B {
        self.at(0, Kind::Down, name)
            .at(n, Kind::Up, name)
            .idle(n + 1)
    }

    /// Down, then `count` OS repeats every `every` frames from `first`, then up.
    pub fn repeats(self, name: &str, first: u32, every: u32, count: u32) -> B {
        let mut b = self.at(0, Kind::Down, name);
        for i in 0..count {
            b = b.at(first + i * every, Kind::Repeat, name);
        }
        let up = first + count * every;
        b.at(up, Kind::Up, name).idle(up + 1)
    }

    pub fn seed(mut self, s: u32) -> B {
        self.s.match_seeds.push(s);
        self
    }

    /// After a `tap` that SELECTS a menu item: the select frame shows fade 32, 32 more fade
    /// frames follow, then the pop; the next Playing frame is the tap frame + 34 (finding 14).
    pub fn after_menu_select(self) -> B {
        self.idle(31)
    }

    /// After a `tap` of Esc in play or selection: fades 30..0 from the key-down, the pop at +31;
    /// the first menu frame is the key-down frame + 32 (finding 9).
    pub fn after_esc(self) -> B {
        self.idle(29)
    }

    /// The script, its keys stably sorted by frame (the order `parse` and the dumper see).
    pub fn end(mut self, extra: u32, quit: bool) -> ShellScript {
        self.s.keys.sort_by_key(|k| k.frame);
        self.s.frames = self.t + extra;
        self.s.expect_quit = quit;
        self.s
    }
}

/// One case: its name, its setup sidecar text (C++-schema TOML), its script.
pub struct Case {
    pub name: &'static str,
    pub setup: Option<&'static str>,
    pub script: ShellScript,
}

const LEVEL_FILE_SETUP: &str =
    "[settings]\nversion = 6\nrandomLevel = false\nlevelFile = 'Levels/water_stage.lev'\n";
const GAMETAG_SETUP: &str = "[settings]\nversion = 6\ngameMode = 1\n";
const HOLDAZONE_SETUP: &str = "[settings]\nversion = 6\ngameMode = 2\n";
const REGENERATE_SETUP: &str = "[settings]\nversion = 6\nregenerateLevel = true\n";
// Both players' health 1: the Rust builder refuses asymmetric health (`BuildError::AsymmetricHealth`).
const GAME_OVER_SETUP: &str =
    "[player1]\nhealth = 1\n\n[player2]\nhealth = 1\n\n[settings]\nversion = 6\nlives = 1\n";

fn setup_name(case: &str) -> String {
    format!("shell_{case}_setup.cfg")
}

/// Both players Up (RANDOMIZE -> DONE!), then both Fire: the selection ends on the Fire frame.
fn both_done(b: B) -> B {
    b.taps(&["R", "UP"]).taps(&["LCTRL", "RCTRL"])
}

/// Movement and fire for `n` match frames (P1 right + fire, P2 left + fire), all keys released
/// by the end.
fn play(b: B, n: u32) -> B {
    assert!(n >= 120);
    b.at(0, Kind::Down, "G")
        .at(10, Kind::Down, "LCTRL")
        .at(12, Kind::Up, "LCTRL")
        .at(25, Kind::Down, "LCTRL")
        .at(27, Kind::Up, "LCTRL")
        .at(40, Kind::Up, "G")
        .at(45, Kind::Down, "LEFT")
        .at(50, Kind::Down, "RCTRL")
        .at(52, Kind::Up, "RCTRL")
        .at(80, Kind::Up, "LEFT")
        .at(90, Kind::Down, "F")
        .at(110, Kind::Up, "F")
        .at(113, Kind::Down, "LCTRL")
        .at(115, Kind::Up, "LCTRL")
        .idle(n)
}

/// The game_over script for a match seed (lives 1, P1 health 1): P1 aims down and keeps firing.
pub fn game_over_script(seed: u32) -> ShellScript {
    let mut b = B::new(Some(&setup_name("game_over")), 41)
        .idle(40)
        .seed(seed)
        .tap("RETURN")
        .after_menu_select();
    b = both_done(b).hold("F", 25);
    for _ in 0..12 {
        b = b.tap("LCTRL").idle(12);
    }
    // The pop to the menu must come before frame 700 (checked by the generator).
    let t = b.t;
    b.idle(700 - t)
        .tap("ESC")
        .idle(5)
        .tap("RETURN")
        .end(40, true)
}

pub fn cases(game_over_seed: u32) -> Vec<Case> {
    let d = None;
    vec![
        Case {
            name: "boot_idle",
            setup: None,
            script: B::new(d, 11).idle(60).end(0, false),
        },
        Case {
            name: "nav",
            setup: None,
            script: B::new(d, 12)
                .idle(35)
                .tap("DOWN")
                .tap("DOWN")
                .tap("UP")
                .tap("UP")
                .tap("UP")
                .tap("DOWN")
                .tap("R")
                .tap("F")
                .repeats("DOWN", 15, 3, 4)
                .tap("PAGEDOWN")
                .tap("PAGEUP")
                .tap("ESC")
                .tap("UP")
                .tap("LALT")
                .tap("UP")
                .tap("RSHIFT")
                .taps(&["UP", "R"])
                .idle(3)
                .hold("LEFT", 20)
                .hold("RIGHT", 20)
                .hold("D", 10)
                .idle(20)
                .end(0, false),
        },
        Case {
            name: "level_file",
            setup: Some(LEVEL_FILE_SETUP),
            script: B::new(Some(&setup_name("level_file")), 13)
                .idle(40)
                .end(0, false),
        },
        Case {
            name: "gametag",
            setup: Some(GAMETAG_SETUP),
            script: B::new(Some(&setup_name("gametag")), 14)
                .idle(40)
                .end(0, false),
        },
        Case {
            name: "holdazone_boot",
            setup: Some(HOLDAZONE_SETUP),
            script: B::new(Some(&setup_name("holdazone_boot")), 15)
                .idle(40)
                .tap("DOWN")
                .idle(10)
                .end(0, false),
        },
        Case {
            name: "milestone",
            setup: None,
            script: {
                // design §6.6: boot, idle 40; Down, Up; NEW GAME; selection; play 150; Esc; RESUME;
                // play 60; Esc; Down; NEW GAME (level reused); selection; Esc; Esc; Enter (QUIT).
                let b = B::new(d, 1)
                    .idle(40)
                    .tap("DOWN")
                    .tap("UP")
                    .seed(101)
                    .tap("RETURN")
                    .after_menu_select();
                let b = play(both_done(b), 150).tap("ESC").after_esc();
                let b = b.idle(40).tap("RETURN").after_menu_select();
                let b = b
                    .at(10, Kind::Down, "RCTRL")
                    .at(12, Kind::Up, "RCTRL")
                    .idle(60)
                    .tap("ESC")
                    .after_esc();
                let b = b
                    .idle(20)
                    .tap("DOWN")
                    .seed(102)
                    .tap("RETURN")
                    .after_menu_select();
                let b = b.idle(15).tap("ESC").after_esc();
                b.idle(20).tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "esc_in_weapsel",
            setup: None,
            script: {
                // Both cursors to slot 1 (P1 F, P2 Down), P2 holds Right (the 12/3 repeat cycles
                // its weapon), Esc mid-selection, RESUME (the copyright bar stays, finding 11),
                // Up twice (1 -> RANDOMIZE -> DONE!), both Fire, play, Esc, QUIT.
                let b = B::new(d, 2)
                    .idle(40)
                    .seed(111)
                    .tap("RETURN")
                    .after_menu_select();
                let b = b
                    .taps(&["F", "DOWN"])
                    .hold("RIGHT", 20)
                    .idle(5)
                    .tap("ESC")
                    .after_esc();
                let b = b.idle(20).tap("RETURN").after_menu_select();
                let b = b
                    .idle(10)
                    .taps(&["R", "UP"])
                    .taps(&["R", "UP"])
                    .taps(&["LCTRL", "RCTRL"]);
                let b = play(b, 120).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "reuse",
            setup: None,
            script: {
                let b = B::new(d, 3)
                    .idle(40)
                    .seed(201)
                    .tap("RETURN")
                    .after_menu_select();
                let b = play(both_done(b), 120).tap("ESC").after_esc();
                let b = b
                    .idle(20)
                    .tap("DOWN")
                    .seed(202)
                    .tap("RETURN")
                    .after_menu_select();
                let b = b.idle(20).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "regenerate",
            setup: Some(REGENERATE_SETUP),
            script: {
                let b = B::new(Some(&setup_name("regenerate")), 4)
                    .idle(40)
                    .seed(301)
                    .tap("RETURN")
                    .after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                let b = b
                    .idle(10)
                    .tap("DOWN")
                    .seed(302)
                    .tap("RETURN")
                    .after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "game_over",
            setup: Some(GAME_OVER_SETUP),
            script: game_over_script(game_over_seed),
        },
        Case {
            name: "f1_quit",
            setup: None,
            script: {
                let b = B::new(d, 5)
                    .idle(30)
                    .seed(401)
                    .tap("F1")
                    .after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                let b = b.idle(10).tap("F1").after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
    ]
}

/// The case's `Settings`: `Settings::default()` or the committed sidecar via `settings_from_toml`.
pub fn settings_for(script: &ShellScript) -> Settings {
    match &script.setup {
        None => Settings::default(),
        Some(f) => settings_from_toml(&std::fs::read_to_string(Path::new(GOLDEN).join(f)).unwrap())
            .unwrap(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pub new_games: u32,
    pub resumes: u32,
    pub menus: u32,
    pub game_over_menu: bool,
    pub quit: bool,
    pub black: u32,
    pub pops: u32,
    pub violations: Vec<String>,
}

pub struct Run {
    pub boot: String,
    pub lines: Vec<String>,
    pub end: String,
    pub ledger: Ledger,
    /// (frame, the surface, its presented fade) for frames in `keep`.
    pub shots: Vec<(u32, Bitmap, i32)>,
}

fn upd_char(p: Phase) -> char {
    match p {
        Phase::Menu => 'M',
        // Step 4½e-1: only an `InputStringState` is `Phase::Text`; T8 derives O/B from the top.
        Phase::Text => 'I',
        Phase::Weapsel => 'W',
        Phase::Game => 'G',
        Phase::Quit => '-',
    }
}

fn present_fields(sh: &Shell, p: Option<Present>) -> (String, Option<i32>) {
    match p {
        None => ("0 -".into(), None),
        Some(Present::Frame { fade }) => (
            format!("1 {:016x}", hash_frame(sh.surface(), fade)),
            Some(fade),
        ),
        Some(Present::Black) => (format!("1 {:016x}", hash_frame(sh.surface(), 0)), Some(0)),
    }
}

fn tail(sh: &Shell) -> String {
    format!(
        "{:016x} {} {} {} {}",
        hash_frame(sh.surface(), 33),
        sh.fade(),
        sh.menu_cycles(),
        sh.top_char(),
        sh.main_selection()
    )
}

/// The sampled words from the held DOS keys through the settings' bindings (design §6.5): bit
/// `c` iff `controls_ex[c]` is held; DIG (`controls_ex[7]`) presses Left + Right.
fn words(held: &BTreeSet<u32>, s: &Settings) -> [ControlState; 2] {
    [0, 1].map(|i| {
        let ex = &s.worm_settings[i].controls_ex;
        let on = |k: u32| k != 0 && held.contains(&k);
        let mut cs = ControlState::new();
        for (c, &k) in ex.iter().enumerate().take(7) {
            cs.set(c as u32, on(k));
        }
        if on(ex[7]) {
            cs.set(ControlState::LEFT, true);
            cs.set(ControlState::RIGHT, true);
        }
        cs
    })
}

/// Drive `script` through `ui::shell::Shell`: the golden-format lines, the ledger and the
/// validators' findings (plan Task 9). `keep` = an inclusive frame range whose surfaces to keep.
pub fn drive(script: &ShellScript, keep: Option<(u32, u32)>) -> Run {
    let settings = settings_for(script);
    let select = UiTc::load(Path::new(TC_ROOT)).hooks.select;
    let seeds = SeedSource::Scripted {
        boot: script.boot_seed,
        matches: script.match_seeds.iter().copied().collect(),
    };
    let (mut sh, mut sim, out) = Shell::boot(
        Path::new(TC_ROOT),
        settings.clone(),
        Box::new(scenario::storage::MemoryStore::new()),
        seeds,
        0,
        StartOptions::default(),
    );
    let (p, _) = present_fields(&sh, out.present);
    let mut run = Run {
        boot: format!("boot {p} {}", tail(&sh)),
        lines: Vec::new(),
        end: String::new(),
        ledger: Ledger::default(),
        shots: Vec::new(),
    };
    let mut held: BTreeSet<u32> = BTreeSet::new();
    let v = |run: &mut Run, frame: u32, what: String| {
        run.ledger.violations.push(format!("frame {frame}: {what}"))
    };
    for frame in 0..script.frames {
        let mut seen = BTreeSet::new();
        let mut events = Vec::new();
        for k in script.keys.iter().filter(|k| k.frame == frame) {
            if !ALLOWED.contains(&k.name.as_str()) {
                v(
                    &mut run,
                    frame,
                    format!("key {} is not menu-safe (plan-time fact 2)", k.name),
                );
            }
            let (dos, typed) = key_of(&k.name);
            if !seen.insert(dos) {
                v(
                    &mut run,
                    frame,
                    format!("two events for {} in one frame", k.name),
                );
            }
            match k.kind {
                Kind::Down if !held.insert(dos) => {
                    v(&mut run, frame, format!("{} down while held", k.name))
                }
                Kind::Repeat if !held.contains(&dos) => {
                    v(&mut run, frame, format!("{} repeat while up", k.name))
                }
                Kind::Up if !held.remove(&dos) => {
                    v(&mut run, frame, format!("{} up while up", k.name))
                }
                _ => {}
            }
            events.push(InputEvent::Key(KeyEvent {
                dos,
                down: k.kind != Kind::Up,
                repeat: k.kind == Kind::Repeat,
                typed,
            }));
        }
        let sampled = words(&held, &settings);
        let input = ShellInput {
            events: &events,
            sampled,
            fresh_seed: 0,
            now_ms: u64::from(frame) * 14,
            restart: false,
        };
        let out = sh.frame(&mut sim, &input);
        if out.upd == Phase::Menu
            && out.menu_sounds.contains(&select)
            && !sh.menu_fading()
            && out.routed.is_none()
        {
            v(
                &mut run,
                frame,
                "a placeholder was selected (the C++ menu acts on it)".into(),
            );
        }
        match out.routed {
            Some(Route::NewGame { .. }) => {
                run.ledger.new_games += 1;
                run.ledger.pops += 1;
                if sim.rand.draws() != 0 {
                    v(
                        &mut run,
                        frame,
                        "the selection constructor drew the RNG (intervention 3)".into(),
                    );
                }
                if settings.game_mode == GM_HOLDAZONE {
                    v(&mut run, frame, "a Holdazone match (unported)".into());
                }
            }
            Some(Route::Resume) => {
                run.ledger.resumes += 1;
                run.ledger.pops += 1;
            }
            Some(Route::Menu) => {
                run.ledger.menus += 1;
                run.ledger.pops += 1;
                if sampled.iter().any(|c| c.pack() != 0) {
                    v(
                        &mut run,
                        frame,
                        "a worm key is held at the back-to-menu pop (plan-time fact 12)".into(),
                    );
                }
                if !sh.main_menu().items[0].visible {
                    run.ledger.game_over_menu = true;
                }
            }
            Some(Route::Quit) => run.ledger.quit = true,
            None => {}
        }
        if out.present == Some(Present::Black) {
            run.ledger.black += 1;
        }
        let (p, fade) = present_fields(&sh, out.present);
        let sounds: Vec<String> = out.menu_sounds.iter().map(i32::to_string).collect();
        let sounds = if sounds.is_empty() {
            "-".to_string()
        } else {
            sounds.join(",")
        };
        run.lines.push(format!(
            "f {frame} {} {p} {} {sounds}",
            upd_char(out.upd),
            tail(&sh)
        ));
        if let (Some((a, b)), Some(f)) = (keep, fade) {
            if (a..=b).contains(&frame) {
                run.shots.push((frame, sh.surface().clone(), f));
            }
        }
        if out.quit {
            if script.keys.iter().any(|k| k.frame > frame) {
                v(&mut run, frame, "key events after the quit".into());
            }
            run.end = format!("end {frame} quit");
            break;
        }
    }
    if run.end.is_empty() {
        run.end = format!("end {} frames", script.frames);
    }
    if run.ledger.new_games as usize != script.match_seeds.len() {
        let what = format!(
            "{} NEW GAMEs for {} match seeds",
            run.ledger.new_games,
            script.match_seeds.len()
        );
        v(&mut run, script.frames, what);
    }
    if run.ledger.quit != script.expect_quit {
        v(
            &mut run,
            script.frames,
            "the expect line disagrees with the run".into(),
        );
    }
    run
}

/// The first match seed for which `game_over_script` ends the match (lives 1, health 1) and pops
/// back to a RESUME-less menu before the scripted Esc at frame 700, with no violation.
pub fn find_game_over_seed() -> u32 {
    for seed in 1..=64 {
        let run = drive(&game_over_script(seed), None);
        // `f <frame> <upd> <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> ...`: 0-based 8.
        let back_by_700 = run
            .lines
            .get(700)
            .is_some_and(|l| l.split_whitespace().nth(8) == Some("M"));
        if run.ledger.violations.is_empty() && run.ledger.game_over_menu && back_by_700 {
            return seed;
        }
    }
    panic!("no game_over seed in 1..=64: make P1 fire more (plan Task 9)");
}

/// The committed script of `name`.
pub fn read_script(name: &str) -> ShellScript {
    ShellScript::parse(
        &std::fs::read_to_string(Path::new(GOLDEN).join(format!("shell_{name}_script.txt")))
            .unwrap(),
    )
}
