//! Step 4½d G2 — the shell cases (design §6.4; plan Tasks 8-10): the script model and builder,
//! the case table, the Rust driver that produces the golden lines through `ui::shell::Shell`,
//! and the validators the generator enforces. The C++ side is `oracle_dump_shell`.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use render::bitmap::Bitmap;
use render::hash::{hash_frame, FNV_OFFSET, FNV_PRIME};
use scenario::settings::{Settings, GM_HOLDAZONE};
use scenario::settings_toml::{settings_from_toml, settings_to_toml};
use scenario::storage::{load_setup, ConfigStore, MemoryStore, NativeStore};
use sim::hash::hash_game_state;
use sim::state::{ControlState, SimState};
use ui::keys::TypedKey;
use ui::shell::level_slot::SeedSource;
use ui::shell::playing::StartOptions;
use ui::shell::settings_menu::{LOAD_OPTIONS, SAVE_OPTIONS, SI_LEVEL};
use ui::shell::{CurMenu, InputEvent, KeyEvent, Phase, Present, Route, Shell, ShellInput};
use ui::text::UiTc;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// The repo root: an `fs` manifest's `<source>` paths are relative to it (plan §Formats).
pub const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

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

/// The keys a case may press: nothing the C++ menu would act on where the Rust menu is inert
/// (4½d plan-time fact 2; 4½e-1 T8: F2, F3, F5, F6, F8, F9, F10 and F11 stay refused). R/F/D/G
/// are P1's up/down/left/right. Step 4½e-1 adds F7 (MATCH SETUP), BACKSPACE, SPACE and the
/// explicit letters and digits (number entry, WEAPON OPTIONS' search, any key for a box).
pub const ALLOWED: &[&str] = &[
    "ESC",
    "RETURN",
    "KP_ENTER",
    "UP",
    "DOWN",
    "LEFT",
    "RIGHT",
    "PAGEUP",
    "PAGEDOWN",
    "LCTRL",
    "RCTRL",
    "LALT",
    "RALT",
    "LSHIFT",
    "RSHIFT",
    "R",
    "F",
    "D",
    "G",
    "F1",
    "F7",
    "BACKSPACE",
    "SPACE",
    "A",
    "B",
    "C",
    "E",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
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
        // Step 4½e-1: SDL_SCANCODE_BACKSPACE, DOS 14, key_buf symbol 8 (outside the search's
        // 32..127, plan §Formats).
        "BACKSPACE" => (14, TypedKey::Sym(8)),
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

/// One script event (Step 4½e-1): a key line, or a `text <frame> <hex>` line — one
/// `SDL_EVENT_TEXT_INPUT` whose string is `bytes` (plan §Formats).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Key(KeyLine),
    Text { frame: u32, bytes: Vec<u8> },
}

impl Event {
    pub fn frame(&self) -> u32 {
        match self {
            Event::Key(k) => k.frame,
            Event::Text { frame, .. } => *frame,
        }
    }
}

/// A `shell_<case>_script.txt` (format: 4½d plan Task 8; 4½e-1 §Formats adds `detail`, `fs` and
/// `text`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellScript {
    /// The setup sidecar's file name, `None` = `default`.
    pub setup: Option<String>,
    pub boot_seed: u32,
    pub match_seeds: Vec<u32>,
    pub frames: u32,
    pub expect_quit: bool,
    /// A `d` line after every `f` line.
    pub detail: bool,
    /// The `fs` manifest's file name (golden-dir-relative).
    pub fs: Option<String>,
    /// Key and text events, stably ordered by frame: within a frame the file order is the SDL
    /// event order.
    pub events: Vec<Event>,
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert!(
        (2..=8).contains(&hex.len()) && hex.len() % 2 == 0,
        "bad text hex {hex}"
    );
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            let b = &hex[i..i + 2];
            assert!(
                b.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "bad text hex {hex}"
            );
            let v = u8::from_str_radix(b, 16).unwrap();
            assert!(v != 0, "a NUL byte in text {hex}");
            v
        })
        .collect()
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
                ["detail"] => {
                    assert!(!s.detail, "two detail lines");
                    s.detail = true;
                }
                ["fs", m] => {
                    assert!(s.fs.is_none(), "two fs lines");
                    s.fs = Some(m.to_string());
                }
                ["key", f, k, n] => s.events.push(Event::Key(KeyLine {
                    frame: f.parse().unwrap(),
                    kind: match *k {
                        "down" => Kind::Down,
                        "up" => Kind::Up,
                        "repeat" => Kind::Repeat,
                        other => panic!("bad key kind {other}"),
                    },
                    name: n.to_string(),
                })),
                ["text", f, hex] => s.events.push(Event::Text {
                    frame: f.parse().unwrap(),
                    bytes: decode_hex(hex),
                }),
                other => panic!("bad script line {other:?}"),
            }
        }
        assert!(
            s.fs.is_none() || s.setup.is_none(),
            "fs needs setup default"
        );
        s.events.sort_by_key(Event::frame); // stable: the file order within a frame, as the dumper
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
        if self.detail {
            out += "detail\n";
        }
        if let Some(m) = &self.fs {
            out += &format!("fs {m}\n");
        }
        for e in &self.events {
            match e {
                Event::Key(k) => {
                    let kind = match k.kind {
                        Kind::Down => "down",
                        Kind::Up => "up",
                        Kind::Repeat => "repeat",
                    };
                    out += &format!("key {} {kind} {}\n", k.frame, k.name);
                }
                Event::Text { frame, bytes } => {
                    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                    out += &format!("text {frame} {hex}\n");
                }
            }
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

    /// The frame cursor.
    pub fn now(&self) -> u32 {
        self.t
    }

    /// Step 4½e-1: a `d` line after every `f` line.
    pub fn detail(mut self) -> B {
        self.s.detail = true;
        self
    }

    /// Step 4½e-1: the `fs` fixture manifest (golden-dir-relative); needs `setup default`.
    pub fn fs(mut self, manifest: &str) -> B {
        assert!(self.s.setup.is_none(), "fs needs setup default");
        self.s.fs = Some(manifest.to_string());
        self
    }

    pub fn at(mut self, dt: u32, kind: Kind, name: &str) -> B {
        self.s.events.push(Event::Key(KeyLine {
            frame: self.t + dt,
            kind,
            name: name.to_string(),
        }));
        self
    }

    /// One `SDL_EVENT_TEXT_INPUT` of `bytes` on the cursor frame (the cursor does not move).
    pub fn text(mut self, bytes: &[u8]) -> B {
        self.s.events.push(Event::Text {
            frame: self.t,
            bytes: bytes.to_vec(),
        });
        self
    }

    /// Type `s` one char per event (plan fact 7): per char, `key down <c>` and `text <c>` on one
    /// frame, `key up` two frames later; the cursor moves 3 per char.
    pub fn type_digits(self, s: &str) -> B {
        let mut b = self;
        for c in s.chars() {
            let name = c.to_ascii_uppercase().to_string();
            b = b
                .at(0, Kind::Down, &name)
                .text(&[c as u8])
                .at(2, Kind::Up, &name)
                .idle(3);
        }
        b
    }

    pub fn idle(mut self, n: u32) -> B {
        self.t += n;
        self
    }

    /// Down now, up two frames later; the cursor moves 3.
    pub fn tap(self, name: &str) -> B {
        self.at(0, Kind::Down, name).at(2, Kind::Up, name).idle(3)
    }

    /// `n` taps of `name`.
    pub fn taps_n(self, name: &str, n: u32) -> B {
        (0..n).fold(self, |b, _| b.tap(name))
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

    /// The script, its events stably sorted by frame (the order `parse` and the dumper see).
    pub fn end(mut self, extra: u32, quit: bool) -> ShellScript {
        self.s.events.sort_by_key(Event::frame);
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
pub fn both_done(b: B) -> B {
    b.taps(&["R", "UP"]).taps(&["LCTRL", "RCTRL"])
}

/// Movement and fire for `n` match frames (P1 right + fire, P2 left + fire), all keys released
/// by the end.
pub fn play(b: B, n: u32) -> B {
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

/// FNV-1a-64 over `bytes` with the frames' constants (the `d` line's `cfg16`, the `file` line's
/// `fnv16`; plan §Formats).
pub fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    })
}

/// A manifest `<rel>`: non-empty, forward slashes, no absolute path, no `\`, no `.`/`..`/empty
/// part (plan §Formats; the dumper's `ValidRel`).
fn valid_rel(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains('\\')
        && rel
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

/// The `fs` fixture (plan §Formats): `root/{user,sys}`, filled from `manifest` with copies.
/// Any other line is refused, as the dumper refuses it.
pub fn make_fixture(manifest: &Path, root: &Path) {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root.join("user")).unwrap();
    std::fs::create_dir_all(root.join("sys")).unwrap();
    for line in std::fs::read_to_string(manifest).unwrap().lines() {
        let t: Vec<&str> = line
            .split_whitespace()
            .take_while(|t| !t.starts_with('#'))
            .collect();
        match t.as_slice() {
            [] => {}
            ["dir", layer @ ("user" | "sys"), rel] if valid_rel(rel) => {
                std::fs::create_dir_all(root.join(layer).join(rel)).unwrap();
            }
            ["file", layer @ ("user" | "sys"), rel, src] if valid_rel(rel) => {
                let dest = root.join(layer).join(rel);
                assert!(!dest.exists(), "fs manifest: {layer}/{rel} given twice");
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                std::fs::copy(Path::new(REPO).join(src), &dest)
                    .unwrap_or_else(|e| panic!("fs manifest source {src}: {e}"));
            }
            _ => panic!("bad fs manifest line: {line}"),
        }
    }
}

/// `file <rel> <fnv16>` for every regular file under `user`, `rel` sorted bytewise.
pub fn file_lines(user: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, u64)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let ft = std::fs::symlink_metadata(&p).unwrap().file_type();
            if ft.is_dir() {
                walk(&p, base, out);
            } else if ft.is_file() {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((rel, fnv64(&std::fs::read(&p).unwrap())));
            }
        }
    }
    let mut files = Vec::new();
    walk(user, user, &mut files);
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    files
        .into_iter()
        .map(|(rel, h)| format!("file {rel} {h:016x}"))
        .collect()
}

/// Harness switches: T8's counterfactual witnesses flip one of them (`Shell::debug_mut`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opts {
    pub resume_sync: bool,
    pub small_labels: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            resume_sync: true,
            small_labels: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryOutcome {
    Accepted,
    Cancelled,
    /// Return on an empty buffer: the value is kept (`integerBehavior.cpp:58`).
    Empty,
}

/// One number entry, as the harness saw it close: its item, how it closed, the item's value
/// after the close.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub frame: u32,
    pub item: String,
    pub outcome: EntryOutcome,
    pub value: String,
}

/// Worm facts after a match frame (the key-edge witnesses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WormSnap {
    pub visible: bool,
    pub ready: bool,
    pub health: i32,
    pub control_states: u32,
    pub current_weapon: i32,
    pub rope_out: bool,
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
    /// Step 4½e-1: every top seen after a frame.
    pub tops: BTreeSet<char>,
    /// `InfoBoxState`s WEAPON OPTIONS pushed (its close refusal).
    pub weapon_boxes: u32,
    pub entries: Vec<Entry>,
    /// (frame, weapon index, old, new) for every `weap_table` change.
    pub weap_changes: Vec<(u32, usize, u32, u32)>,
    /// The level size of every NEW GAME.
    pub level_sizes: Vec<(i32, i32)>,
    /// The frames of the RESUME routes.
    pub resume_frames: Vec<u32>,
    /// Match frames with a weapon bonus (`frame == 0`) while `names_on_bonuses`.
    pub bonus_labels: u32,
    /// Match frames with a booby-trap wobject at `cur_frame == 0` while `names_on_bonuses`.
    pub booby_labels: u32,
    /// Match frames per player with a visible worm holding Change.
    pub change_labels: [u32; 2],
}

pub struct Run {
    pub boot: String,
    pub lines: Vec<String>,
    /// Step 4½e-1: the `d` lines (with `detail`), one per `f` line.
    pub details: Vec<String>,
    pub end: String,
    /// Step 4½e-1: the `file` lines (with `fs`).
    pub files: Vec<String>,
    pub ledger: Ledger,
    /// (frame, the surface, its presented fade) for frames in `keep`.
    pub shots: Vec<(u32, Bitmap, i32)>,
    /// Per frame: both worms after it, on match frames (top `G`, not in selection).
    pub worms: Vec<Option<[WormSnap; 2]>>,
    /// P1's five weapon types after the run.
    pub p1_weapons: Vec<Option<i32>>,
    /// The menu's settings after the run.
    pub settings: Settings,
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
    drive_with(script, keep, Opts::default())
}

/// A fresh fixture directory per drive (tests drive the same case in parallel).
static FIXTURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The worms after a frame.
fn snaps(sim: &SimState) -> [WormSnap; 2] {
    [0, 1].map(|i| {
        let w = &sim.worms[i];
        WormSnap {
            visible: w.visible,
            ready: w.ready,
            health: w.health,
            control_states: w.control_states.pack(),
            current_weapon: w.current_weapon,
            rope_out: w.ninjarope.out,
        }
    })
}

/// [`drive`] with the harness switches (T8's counterfactual witnesses). Step 4½e-1: the events
/// in order (key and text), the `fs` store (plan §Formats), `record_replays = false` after the
/// boot load (intervention 6's mirror), `upd` from the top before the frame, the `d` and `file`
/// lines and the e-1 validators (plan T8 Step 2).
pub fn drive_with(script: &ShellScript, keep: Option<(u32, u32)>, opts: Opts) -> Run {
    let mut fixture: Option<PathBuf> = None;
    let (mut settings, store): (Settings, Box<dyn ConfigStore>) = match &script.fs {
        None => (settings_for(script), Box::new(MemoryStore::new())),
        Some(m) => {
            let stem = m.trim_start_matches("shell_").trim_end_matches("_fs.txt");
            let n = FIXTURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "liero_rs_shell_fs_{stem}_{}_{n}",
                std::process::id()
            ));
            make_fixture(&Path::new(GOLDEN).join(m), &root);
            let store = NativeStore::split(root.join("user"), Some(root.join("sys")))
                .with_root_label("./user");
            let s = load_setup(&store).expect("the boot load (gameEntry.cpp:55-58)");
            fixture = Some(root);
            (s, Box::new(store))
        }
    };
    settings.record_replays = false; // intervention 6's mirror
    let tc = UiTc::load(Path::new(TC_ROOT));
    let select = tc.hooks.select;
    let seeds = SeedSource::Scripted {
        boot: script.boot_seed,
        matches: script.match_seeds.iter().copied().collect(),
    };
    let (mut sh, mut sim, out) = Shell::boot(
        Path::new(TC_ROOT),
        settings.clone(),
        store,
        seeds,
        0,
        StartOptions::default(),
    );
    sh.debug_mut().resume_sync = opts.resume_sync;
    sh.debug_mut().small_labels = opts.small_labels;
    let (p, _) = present_fields(&sh, out.present);
    let mut run = Run {
        boot: format!("boot {p} {}", tail(&sh)),
        lines: Vec::new(),
        details: Vec::new(),
        end: String::new(),
        files: Vec::new(),
        ledger: Ledger::default(),
        shots: Vec::new(),
        worms: Vec::new(),
        p1_weapons: Vec::new(),
        settings: Settings::default(),
    };
    let mut held: BTreeSet<u32> = BTreeSet::new();
    let v = |run: &mut Run, frame: u32, what: String| {
        run.ledger.violations.push(format!("frame {frame}: {what}"))
    };
    // Every keyboard player's bound DOS keys (a letter typed in WEAPON OPTIONS moves a worm's
    // cursor there too, known pitfall 10).
    let controls: BTreeSet<u32> = settings.worm_settings[..2]
        .iter()
        .flat_map(|w| w.controls_ex.iter().copied())
        .filter(|&k| k != 0)
        .collect();
    // The open number entry: the harness's mirror of its buffer, and whether Esc closed it.
    let mut entry: Option<(Vec<u8>, Option<bool>)> = None;
    for frame in 0..script.frames {
        let top0 = sh.top_char();
        let upd = if top0 == 'G' && sh.phase() == Phase::Weapsel {
            'W'
        } else {
            top0
        };
        let cur0 = sh.cur_menu();
        let weap0 = sh.settings().weap_table;
        let mut seen = BTreeSet::new();
        let mut events = Vec::new();
        let mut downs = 0;
        for e in script.events.iter().filter(|e| e.frame() == frame) {
            let k = match e {
                Event::Text { bytes, .. } => {
                    if top0 != 'I' {
                        v(
                            &mut run,
                            frame,
                            format!("a text event while the top is {top0}"),
                        );
                    }
                    if let Some((buf, _)) = entry.as_mut() {
                        if bytes.len() == 1 && bytes[0].is_ascii_digit() {
                            buf.push(bytes[0]);
                        }
                    }
                    downs += 1;
                    events.push(InputEvent::Text(
                        String::from_utf8(bytes.clone()).expect("UTF-8 text"),
                    ));
                    continue;
                }
                Event::Key(k) => k,
            };
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
            if k.kind != Kind::Up {
                downs += 1;
                if top0 == 'O' && k.name.len() == 1 && controls.contains(&dos) {
                    v(
                        &mut run,
                        frame,
                        format!("{} typed in WEAPON OPTIONS is a player's control", k.name),
                    );
                }
                if let Some((buf, closed)) = entry.as_mut().filter(|_| top0 == 'I') {
                    match k.name.as_str() {
                        "BACKSPACE" => {
                            buf.pop();
                        }
                        "RETURN" | "KP_ENTER" => {
                            closed.get_or_insert(true);
                        }
                        "ESC" => {
                            closed.get_or_insert(false);
                        }
                        _ => {}
                    }
                }
            }
            events.push(InputEvent::Key(KeyEvent {
                dos,
                down: k.kind != Kind::Up,
                repeat: k.kind == Kind::Repeat,
                typed,
            }));
        }
        let sampled = words(&held, &settings);
        // Plan D4: `now_ms = 0`, so the search never times out (the dumper's gap check keeps C++
        // inside its 1500 ms too).
        let input = ShellInput {
            events: &events,
            sampled,
            fresh_seed: 0,
            now_ms: 0,
            restart: false,
        };
        let out = sh.frame(&mut sim, &input);
        let top1 = sh.top_char();
        run.ledger.tops.insert(top1);
        let selected = out.menu_sounds.contains(&select);
        if upd == 'M'
            && cur0 == CurMenu::Main
            && sh.cur_menu() == CurMenu::Main
            && top1 == 'M'
            && selected
            && !sh.menu_fading()
            && out.routed.is_none()
        {
            v(
                &mut run,
                frame,
                "a placeholder was selected (the C++ menu acts on it)".into(),
            );
        }
        if upd == 'M'
            && cur0 == CurMenu::Settings
            && sh.cur_menu() == CurMenu::Settings
            && top1 == 'M'
            && selected
            && [SI_LEVEL, LOAD_OPTIONS, SAVE_OPTIONS].contains(&sh.settings_menu().selected_id())
        {
            v(
                &mut run,
                frame,
                "Enter on LEVEL / LOAD SETUP / SAVE SETUP AS... (C++ pushes a selector, D6)".into(),
            );
        }
        if upd == 'M' && top1 == 'B' {
            v(
                &mut run,
                frame,
                "Rust refused a NEW GAME or RESUME that C++ plays (plan D5)".into(),
            );
        }
        if matches!(top0, 'M' | 'O') && matches!(top1, 'O' | 'I' | 'B') && top1 != top0 {
            if downs > 1 {
                v(
                    &mut run,
                    frame,
                    format!("another event in the frame that pushed {top1} (plan fact 4)"),
                );
            }
            if top0 == 'O' {
                run.ledger.weapon_boxes += 1;
            }
        }
        if top0 != 'I' && top1 == 'I' {
            let item = sh.settings_menu().selected().expect("the entry's item");
            entry = Some((item.value.trim_end_matches('%').as_bytes().to_vec(), None));
        } else if top0 == 'I' && top1 != 'I' {
            let (buf, closed) = entry.take().expect("an open entry");
            let item = sh.settings_menu().selected().expect("the entry's item");
            run.ledger.entries.push(Entry {
                frame,
                item: item.string.clone(),
                outcome: match closed {
                    Some(false) => EntryOutcome::Cancelled,
                    _ if buf.is_empty() => EntryOutcome::Empty,
                    _ => EntryOutcome::Accepted,
                },
                value: item.value.clone(),
            });
        }
        let weap1 = sh.settings().weap_table;
        for (i, (a, b)) in weap0.iter().zip(weap1.iter()).enumerate() {
            if a != b {
                run.ledger.weap_changes.push((frame, i, *a, *b));
            }
        }
        match out.routed {
            Some(Route::NewGame { .. }) => {
                run.ledger.new_games += 1;
                run.ledger.pops += 1;
                run.ledger
                    .level_sizes
                    .push((sim.level.width, sim.level.height));
                if sim.rand.draws() != 0 {
                    v(
                        &mut run,
                        frame,
                        "the selection constructor drew the RNG (intervention 3)".into(),
                    );
                }
                if sh.settings().game_mode == GM_HOLDAZONE {
                    v(&mut run, frame, "a Holdazone match (unported)".into());
                }
            }
            Some(Route::Resume) => {
                run.ledger.resumes += 1;
                run.ledger.pops += 1;
                run.ledger.resume_frames.push(frame);
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
        run.lines
            .push(format!("f {frame} {upd} {p} {} {sounds}", tail(&sh)));
        let in_match = top1 == 'G' && sh.phase() == Phase::Game;
        if script.detail {
            let cur = match sh.cur_menu() {
                CurMenu::Main => 'M',
                CurMenu::Settings => 'S',
            };
            let state = if in_match {
                format!("{:08x}", hash_game_state(&sim))
            } else {
                "-".to_string()
            };
            run.details.push(format!(
                "d {frame} {cur} {} {:016x} {state}",
                sh.settings_menu().selection(),
                fnv64(settings_to_toml(sh.settings()).as_bytes())
            ));
        }
        if in_match {
            let names = sh.current().expect("a match").settings().names_on_bonuses;
            if names && sim.bonuses.iter().any(|b| b.frame == 0) {
                run.ledger.bonus_labels += 1;
            }
            if names
                && !sim.wobject_consts.h_rem_exp
                && sim
                    .wobjects
                    .iter()
                    .any(|w| w.ty == Some(34) && w.cur_frame == 0)
            {
                run.ledger.booby_labels += 1;
            }
            for (i, w) in sim.worms.iter().enumerate().take(2) {
                if w.visible && w.control_states.get(ControlState::CHANGE) {
                    run.ledger.change_labels[i] += 1;
                }
            }
            run.worms.push(Some(snaps(&sim)));
        } else {
            run.worms.push(None);
        }
        if let (Some((a, b)), Some(f)) = (keep, fade) {
            if (a..=b).contains(&frame) {
                run.shots.push((frame, sh.surface().clone(), f));
            }
        }
        if out.quit {
            if script.events.iter().any(|e| e.frame() > frame) {
                v(&mut run, frame, "events after the quit".into());
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
    run.p1_weapons = sim.worms[0].weapons.iter().map(|w| w.ty).collect();
    run.settings = sh.settings().clone();
    if let Some(root) = fixture {
        if run.ledger.quit {
            sh.save_on_exit()
                .expect("the exit save (gameEntry.cpp:78, intervention 9)");
        }
        run.files = file_lines(&root.join("user"));
        let _ = std::fs::remove_dir_all(&root);
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
