//! Step 4½f-1 G2f-1 — the CPU through the real `Gfx::RunOneFrame` (plan Task 8): the four f-1
//! shell cases, their inputs (two user `liero.cfg`s, two setups, two manifests), their pinned
//! seeds and the per-case witnesses the generator (`gen_slice4_5f1_shell`) and the gate
//! (`shell_golden.rs`) check. The script model, the driver, the CPU ledger and the validators are
//! `shell_common`'s; the C++ side is `oracle_dump_shell` (`gen_shell_golden.sh`), whose
//! `LocalController` runs the real `DumbLieroAI` (intervention 3′ zeroes the new game's
//! `reacts`, check 3″ proves each new AI's RNG fresh, plan D5 / D14).
//!
//! Every case has `detail`, scripts its match seeds, boots from a distinct seed and ends by QUIT.
//! The CPUs are KEEP or PICK bots, never RANDOM (plan D2: intervention 3 refuses a selection
//! constructor that draws). No key bound to player 2 is pressed in a match phase except by
//! design (known pitfall 16): `cpu_pick` presses them in selection only.
#![allow(dead_code)]

use scenario::settings::Settings;
use scenario::settings_toml::settings_to_toml;
use sim::state::ControlState;
use ui::shell::selection::{BOT_WEAPONS_KEEP, CONTROLLER_BOT};

use crate::shell_common::{drive_with, fnv64, Fs, Kind, Opts, Run, ShellScript, B};

/// The f-1 cases, in corpus order; `cpu_match` is the 🎯 milestone.
pub const NAMES: [&str; 4] = ["cpu_match", "cpu_vs_cpu", "cpu_pick", "hp_boot"];

/// One f-1 case: its script and the golden-dir files it reads (a setup sidecar, or an `fs`
/// manifest and the user `liero.cfg` it copies), as (file name, bytes).
pub struct Case {
    pub name: &'static str,
    pub script: ShellScript,
    pub files: Vec<(String, Vec<u8>)>,
}

const GOLDEN_REL: &str = "rust/oracle-tests/golden";

/// BOT WEAPONS PICK (`select_bot_weapons == 1`, the C++ default; `weapsel.cpp:95`).
pub const BOT_WEAPONS_PICK: u32 = 1;

/// Heavy picks (1-based `weap_order`: BIG NUKE, MINI NUKE, DOOMSDAY, CRACKLER, NAPALM; Batch 6's
/// pinned CPU): with them the CPU kills and dies within a short match.
pub const HEAVY_PICKS: [u32; 5] = [2, 28, 14, 11, 32];

/// The pinned seeds, found once by `gen_slice4_5f1_shell -- search <case> <lo> <hi>` (the first
/// seed of the range whose drive has no violation and every witness).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pins {
    /// `cpu_match`'s match seed.
    pub cpu_match: u32,
    /// `cpu_vs_cpu`'s match seed and the frame of its scripted Esc (20 frames after the
    /// post-mortem's pop to the menu).
    pub cpu_vs_cpu: (u32, u32),
    /// `cpu_pick`'s match seed (any seed: its witnesses are the selection's).
    pub cpu_pick: u32,
    /// `hp_boot`'s match seed.
    pub hp_boot: u32,
}

pub const PINS: Pins = Pins {
    cpu_match: 7105,
    cpu_vs_cpu: (7204, 4017),
    cpu_pick: 7301,
    hp_boot: 7401,
};

/// `cpu_match`'s user `liero.cfg`: C++ `Settings()` with player 2 the CPU on KEEP (ready at once
/// with its saved picks, [`HEAVY_PICKS`]).
pub fn cpu_match_liero() -> Settings {
    let mut s = Settings::default();
    s.worm_settings[1].controller = CONTROLLER_BOT;
    s.worm_settings[1].weapons = HEAVY_PICKS;
    s.select_bot_weapons = BOT_WEAPONS_KEEP;
    s
}

/// `cpu_vs_cpu`'s setup: both players CPUs on KEEP with the default picks, lives 2.
pub fn cpu_vs_cpu_setup() -> Settings {
    let mut s = Settings::default();
    s.worm_settings[0].controller = CONTROLLER_BOT;
    s.worm_settings[1].controller = CONTROLLER_BOT;
    s.select_bot_weapons = BOT_WEAPONS_KEEP;
    s.lives = 2;
    s
}

/// `cpu_pick`'s setup: player 2 the CPU on PICK (its keys drive its selection menu).
pub fn cpu_pick_setup() -> Settings {
    let mut s = Settings::default();
    s.worm_settings[1].controller = CONTROLLER_BOT;
    s.select_bot_weapons = BOT_WEAPONS_PICK;
    s
}

/// `hp_boot`'s user `liero.cfg`: healths 40 / 250, two humans, `recordReplays` off (so the boot's
/// `cfg16`, after intervention 6, is the file's own hash).
pub fn hp_boot_liero() -> Settings {
    let mut s = Settings::default();
    s.worm_settings[0].health = 40;
    s.worm_settings[1].health = 250;
    s.record_replays = false;
    s
}

fn setup_file(case: &str) -> String {
    format!("shell_{case}_setup.cfg")
}

fn manifest(case: &str) -> String {
    format!("shell_{case}_fs.txt")
}

fn user_liero(case: &str) -> String {
    format!("shell_{case}_user_liero.cfg")
}

/// Player 1's and player 2's keys, in control order (`Up, Down, Left, Right, Fire, Change,
/// Jump`; `Settings()`'s defaults).
pub const P1_KEYS: [&str; 7] = ["R", "F", "D", "G", "LCTRL", "LSHIFT", "LALT"];
pub const P2_KEYS: [&str; 7] = ["UP", "DOWN", "LEFT", "RIGHT", "RCTRL", "RALT", "RSHIFT"];

/// One player's fight for `cycles` 64-frame cycles from the cursor (the cursor does not move):
/// walk (right on even cycles, left on odd), fire, aim up, fire, then per cycle mod 4 a jump, the
/// ninja rope (Change held, Jump), a jump or a weapon change (Change held, Right); aim down, fire
/// twice. Every key is up by the cycle's frame 60.
fn fight_keys(mut b: B, k: [&str; 7], cycles: u32) -> B {
    let [up, down, left, right, fire, change, jump] = k;
    for c in 0..cycles {
        let t = c * 64;
        let walk = if c % 2 == 0 { right } else { left };
        b = b
            .at(t, Kind::Down, walk)
            .at(t + 18, Kind::Up, walk)
            .at(t + 20, Kind::Down, fire)
            .at(t + 22, Kind::Up, fire)
            .at(t + 24, Kind::Down, up)
            .at(t + 30, Kind::Up, up)
            .at(t + 32, Kind::Down, fire)
            .at(t + 34, Kind::Up, fire);
        b = match c % 4 {
            1 => b
                .at(t + 36, Kind::Down, change)
                .at(t + 38, Kind::Down, jump)
                .at(t + 40, Kind::Up, jump)
                .at(t + 42, Kind::Up, change),
            3 => b
                .at(t + 36, Kind::Down, change)
                .at(t + 38, Kind::Down, right)
                .at(t + 40, Kind::Up, right)
                .at(t + 42, Kind::Up, change),
            _ => b.at(t + 36, Kind::Down, jump).at(t + 40, Kind::Up, jump),
        };
        b = b
            .at(t + 44, Kind::Down, down)
            .at(t + 48, Kind::Up, down)
            .at(t + 50, Kind::Down, fire)
            .at(t + 52, Kind::Up, fire)
            .at(t + 56, Kind::Down, fire)
            .at(t + 58, Kind::Up, fire);
    }
    b
}

/// [`fight_keys`] for the players in `who`, then the cursor moves `cycles * 64`.
fn fight(b: B, who: &[[&str; 7]], cycles: u32) -> B {
    let b = who.iter().fold(b, |b, k| fight_keys(b, *k, cycles));
    b.idle(cycles * 64)
}

/// Esc in the main focus (the cursor to QUIT TO OS), then Return: the quit.
fn quit(b: B) -> ShellScript {
    b.tap("ESC").idle(5).tap("RETURN").end(40, true)
}

/// NEW GAME (Return on the boot cursor) with `seed`, to the selection's first frame.
fn new_game(b: B, seed: u32) -> B {
    b.seed(seed).tap("RETURN").after_menu_select()
}

pub fn cpu_match_script(seed: u32) -> ShellScript {
    // 🎯 NEW GAME → selection (the CPU ready at once; P1 Up to DONE!, Fire) → 23 cycles of P1's
    // fight (1,472 frames: walk, fire, rope, change) → Esc (every key up) → the fade → F1
    // RESUME → 7 more cycles → Esc → QUIT → the exit save. Player 2's keys are never pressed.
    let b = B::new(None, 71)
        .fs(&manifest("cpu_match"))
        .detail()
        .idle(40);
    let b = new_game(b, seed).tap("R").tap("LCTRL");
    let b = fight(b, &[P1_KEYS], 23).tap("ESC").after_esc();
    let b = b.idle(20).tap("F1").after_menu_select();
    let b = fight(b, &[P1_KEYS], 7).tap("ESC").after_esc();
    quit(b.idle(10))
}

/// `quit_at` = the frame of the scripted Esc in the menu after the post-mortem, or `None` for the
/// search (idle 9,000 frames with no quit).
pub fn cpu_vs_cpu_script(seed: u32, quit_at: Option<u32>) -> ShellScript {
    // NEW GAME → selection ends on its first frame with no key (two KEEP bots, T0 P1) → the two
    // CPUs play to game over → the 180-frame post-mortem → the menu (RESUME hidden) → QUIT.
    let b = B::new(Some(&setup_file("cpu_vs_cpu")), 72)
        .detail()
        .idle(40);
    let b = new_game(b, seed);
    match quit_at {
        Some(q) => {
            let t = b.now();
            assert!(q > t, "cpu_vs_cpu: the quit before the match");
            quit(b.idle(q - t))
        }
        None => b.idle(9000).end(0, false),
    }
}

pub fn cpu_pick_script(seed: u32) -> ShellScript {
    // T0 P3's path: NEW GAME → P2's Down (WEAPON 1), Right (its weapon changes), Up ×2 (DONE!),
    // Right Ctrl (P2 ready: its keys drive the bot's menu) → P1's Up, Fire (selection ends) →
    // 300 match frames (the AI starts only now) → Esc → QUIT.
    let b = B::new(Some(&setup_file("cpu_pick")), 73).detail().idle(40);
    let b = new_game(b, seed)
        .idle(1)
        .tap("DOWN")
        .tap("RIGHT")
        .tap("UP")
        .tap("UP")
        .tap("RCTRL")
        .tap("R")
        .tap("LCTRL");
    let b = b.idle(300).tap("ESC").after_esc();
    quit(b.idle(10))
}

pub fn hp_boot_script(seed: u32) -> ShellScript {
    // Boot on healths 40 / 250 (no sanitising: the first d line's cfg16 is the file's) → NEW GAME
    // → both DONE → 16 cycles of both players' fight (1,024 frames) → Esc → QUIT.
    let b = B::new(None, 74).fs(&manifest("hp_boot")).detail().idle(40);
    let b = new_game(b, seed)
        .taps(&["R", "UP"])
        .taps(&["LCTRL", "RCTRL"]);
    let b = fight(b, &[P1_KEYS, P2_KEYS], 16).tap("ESC").after_esc();
    quit(b.idle(10))
}

fn fs_files(case: &str, liero: &Settings, what: &[&str]) -> Vec<(String, Vec<u8>)> {
    let u = user_liero(case);
    let fs = Fs::install().user_file("Setups/liero.cfg", &format!("{GOLDEN_REL}/{u}"));
    vec![
        (u, settings_to_toml(liero).into_bytes()),
        (manifest(case), fs.text(what).into_bytes()),
    ]
}

/// The f-1 cases on `pins`, in [`NAMES`] order.
pub fn cases_with(pins: &Pins) -> Vec<Case> {
    vec![
        Case {
            name: "cpu_match",
            script: cpu_match_script(pins.cpu_match),
            files: fs_files(
                "cpu_match",
                &cpu_match_liero(),
                &[
                    "Step 4½f-1 G2f-1 cpu_match (the milestone): the install's system layer and the",
                    "user's liero.cfg (defaults; player 2 the CPU on KEEP with heavy picks).",
                ],
            ),
        },
        Case {
            name: "cpu_vs_cpu",
            script: cpu_vs_cpu_script(pins.cpu_vs_cpu.0, Some(pins.cpu_vs_cpu.1)),
            files: vec![(
                setup_file("cpu_vs_cpu"),
                settings_to_toml(&cpu_vs_cpu_setup()).into_bytes(),
            )],
        },
        Case {
            name: "cpu_pick",
            script: cpu_pick_script(pins.cpu_pick),
            files: vec![(
                setup_file("cpu_pick"),
                settings_to_toml(&cpu_pick_setup()).into_bytes(),
            )],
        },
        Case {
            name: "hp_boot",
            script: hp_boot_script(pins.hp_boot),
            files: fs_files(
                "hp_boot",
                &hp_boot_liero(),
                &[
                    "Step 4½f-1 G2f-1 hp_boot: the install's system layer and the user's liero.cfg",
                    "(healths 40 / 250, recordReplays off).",
                ],
            ),
        },
    ]
}

/// The f-1 cases on [`PINS`].
pub fn cases() -> Vec<Case> {
    cases_with(&PINS)
}

fn need(ok: bool, what: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("witness missing: {what}"))
    }
}

/// The frames whose controller ran a match tick: the worms were recorded after them.
fn match_frames(run: &Run) -> Vec<usize> {
    (0..run.lines.len())
        .filter(|&f| run.worms[f].is_some())
        .collect()
}

/// The match frames that ticked: a match frame after a match frame (the first match frame after
/// the selection or the menu is the finalise or the RESUME pop, which runs no tick).
fn tick_frames(run: &Run) -> Vec<usize> {
    (1..run.lines.len())
        .filter(|&f| run.worms[f].is_some() && run.worms[f - 1].is_some())
        .collect()
}

/// The `f` line's `upd` (0-based field 2).
fn upd(run: &Run, f: usize) -> char {
    run.lines[f]
        .split_whitespace()
        .nth(2)
        .unwrap()
        .chars()
        .next()
        .unwrap()
}

/// The frames of `name`'s key-downs.
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

/// The first key-down of `name` on a frame whose controller ran a match tick (`upd` G, not in
/// selection), and the first frame after it that is not a match frame (the menu).
fn esc_fade(run: &Run, script: &ShellScript) -> Option<(usize, usize)> {
    let esc = downs_of(script, "ESC")
        .into_iter()
        .map(|f| f as usize)
        .find(|&f| f > 0 && run.worms[f - 1].is_some() && upd(run, f) == 'G')?;
    let back = (esc..run.lines.len()).find(|&f| run.worms[f].is_none())?;
    Some((esc, back))
}

/// The distinct player-2 words on `frames`.
fn p2_words(run: &Run, frames: impl Iterator<Item = usize>) -> Vec<u32> {
    let mut w: Vec<u32> = frames
        .filter_map(|f| run.ledger.words[f].map(|x| x[1]))
        .collect();
    w.sort_unstable();
    w.dedup();
    w
}

/// The case's witnesses (plan T8 Step 3), as summary lines; `Err` names the first missing one.
pub fn witnesses(case: &Case, run: &Run) -> Result<Vec<String>, String> {
    let l = &run.ledger;
    let mut out = Vec::new();
    need(l.violations.is_empty(), "no violations")?;
    need(l.quit, "QUIT")?;
    let mf = match_frames(run);
    let ticks = tick_frames(run);
    need(!ticks.is_empty(), "match ticks")?;
    let fire = 1 << ControlState::FIRE;
    let lr = (1 << ControlState::LEFT) | (1 << ControlState::RIGHT);
    match case.name {
        "cpu_match" => {
            need(l.new_games == 1 && l.resumes == 1, "1 NEW GAME, 1 RESUME")?;
            need(
                mf.iter().all(|&f| l.cpu[f] == [false, true]),
                "player 2 is a CPU (and player 1 not) on every match frame",
            )?;
            need(!l.p2_keys, "no key bound to player 2 in a match phase")?;
            need(l.deaths[0] >= 1 && l.deaths[1] >= 1, "a death of each worm")?;
            need(l.respawns[1] >= 1, "a CPU respawn")?;
            let traced: Vec<usize> = mf
                .iter()
                .copied()
                .filter(|&f| l.ai_traces[f][1].ran)
                .collect();
            need(
                traced == ticks && mf.iter().all(|&f| !l.ai_traces[f][0].ran),
                "the AI ran for player 2 (only), on every match tick and on no other frame",
            )?;
            let toggles = traced
                .iter()
                .filter(|&&f| {
                    l.ai_traces[f][1].fire_drew
                        && l.words[f - 1]
                            .zip(l.words[f])
                            .is_some_and(|(a, b)| (a[1] ^ b[1]) & fire != 0)
                })
                .count();
            let walks = traced
                .iter()
                .filter(|&&f| {
                    let t = &l.ai_traces[f][1];
                    !t.change
                        && run.worms[f].is_some_and(|w| w[1].visible)
                        && l.words[f].is_some_and(|w| w[1] & lr != 0)
                })
                .count();
            let changes = traced.iter().filter(|&&f| l.ai_traces[f][1].change).count();
            need(toggles > 0, "a Fire toggle by the AI")?;
            need(walks > 0, "the AI's walk arm")?;
            need(changes > 0, "the AI's Change arm")?;
            let (esc, back) = esc_fade(run, &case.script).ok_or("no Esc in a match")?;
            let fade = p2_words(run, esc..back);
            need(
                fade.len() >= 2,
                "the Esc fade frames carry changing P2 words",
            )?;
            out.push(format!(
                "deaths {:?} at {:?}, respawns {:?}; AI ticks {}, fire toggles {toggles}, walks \
                 {walks}, changes {changes}; Esc fade {esc}..{back}: P2 words {fade:02x?}",
                l.deaths,
                l.death_frames,
                l.respawns,
                traced.len()
            ));
            out.extend(run.files.iter().cloned());
        }
        "cpu_vs_cpu" => {
            need(
                l.new_games == 1 && l.menus == 1,
                "1 NEW GAME, back to the menu once",
            )?;
            need(
                mf.iter().all(|&f| l.cpu[f] == [true, true]),
                "both players CPUs on every match frame",
            )?;
            need(
                case.script
                    .events
                    .iter()
                    .all(|e| run.worms[e.frame() as usize].is_none()),
                "no key in the match",
            )?;
            // Two KEEP bots: the selection ends on the frame after NEW GAME's, with no key (T0
            // P1), and the next frame is the first tick.
            let ng = (0..run.lines.len())
                .find(|&f| l.cpu[f] != [false, false])
                .ok_or("no match")?;
            need(
                run.worms[ng].is_none() && mf[0] == ng + 1 && ticks[0] == ng + 2,
                "the selection finalises on the first frame after NEW GAME's",
            )?;
            need(
                l.game_over_frames.len() == 1 && l.game_over_menu,
                "game over, and the RESUME-less menu",
            )?;
            let over = l.game_over_frames[0] as usize;
            let post = p2_words(
                run,
                (over + 1..run.lines.len()).filter(|f| run.worms[*f].is_some()),
            );
            need(post.len() >= 2, "the post-mortem's P2 words change")?;
            let last = *mf.last().unwrap();
            out.push(format!(
                "match frames {}..={last}; game over at {over}, post-mortem {} frames (P2 words \
                 {post:02x?}); deaths {:?} at {:?}",
                mf[0],
                last - over,
                l.deaths,
                l.death_frames
            ));
        }
        "cpu_pick" => {
            need(l.new_games == 1, "1 NEW GAME")?;
            let w: Vec<usize> = (0..run.lines.len())
                .filter(|&f| l.sel_cursor[f].is_some())
                .collect();
            need(!w.is_empty(), "selection frames")?;
            let moved: Vec<u32> = w
                .iter()
                .copied()
                .filter(|&f| {
                    l.sel_cursor[f - 1].is_some_and(|c| c[1] != l.sel_cursor[f].unwrap()[1])
                })
                .map(|f| f as u32)
                .collect();
            let p2_moves: Vec<u32> = ["DOWN", "UP"]
                .iter()
                .flat_map(|k| downs_of(&case.script, k))
                .collect();
            need(
                !moved.is_empty() && moved.iter().all(|f| p2_moves.contains(f)),
                "P2's cursor moved by P2's keys (and only then) on W frames",
            )?;
            need(
                w.iter()
                    .all(|&f| !l.ai_traces[f][1].ran && l.ai_last[f][1] == Some(0)),
                "no AI step in selection",
            )?;
            let first = ticks[0];
            need(
                first == mf[0] + 1 && w.iter().all(|&f| f < mf[0]),
                "the first tick right after the finalise frame",
            )?;
            need(
                l.ai_traces[first][1].ran && l.ai_last[first][1] == Some(0x2af0_9813),
                "the AI's first draw on the first match tick",
            )?;
            // The twin: player 2 Human on the same keys. Every frame through the one that
            // finalises the selection is the same (the `f` line, both words), so the bot's
            // selection is its keys'.
            let twin = drive_with(
                &case.script,
                None,
                Opts {
                    p2_human: true,
                    ..Opts::default()
                },
            );
            let fin = first - 1;
            let same = (0..=fin)
                .all(|f| run.lines[f] == twin.lines[f] && l.words[f] == twin.ledger.words[f]);
            need(
                same,
                "the selection frames equal a human player 2's on the same keys",
            )?;
            out.push(format!(
                "W frames {}..={fin}: P2's cursor moved on {moved:?}; twin (P2 human) equal \
                 through the finalise frame {fin}; first AI tick {first}",
                w[0]
            ));
        }
        "hp_boot" => {
            need(l.new_games == 1, "1 NEW GAME")?;
            let file = &case.files[0].1;
            let cfg16 = run.details[0].split_whitespace().nth(4).unwrap();
            need(
                cfg16 == format!("{:016x}", fnv64(file)),
                "the boot cfg16 is the fixture file's",
            )?;
            need(
                mf.iter()
                    .all(|&f| run.worms[f].unwrap().map(|w| w.max_health) == [40, 250]),
                "maxes 40 and 250",
            )?;
            let both = mf.iter().find(|&&f| {
                run.worms[f]
                    .unwrap()
                    .iter()
                    .all(|w| w.visible && w.health < w.max_health)
            });
            need(both.is_some(), "a lifebar below its max drawn for each")?;
            need(
                l.deaths.iter().sum::<u32>() >= 1 && l.respawns.iter().sum::<u32>() >= 1,
                "a death and a respawn",
            )?;
            let f = *both.unwrap();
            let ws = run.worms[f].unwrap();
            out.push(format!(
                "cfg16 {cfg16}; frame {f}: health {}/{} and {}/{}; deaths {:?} at {:?}, respawns \
                 {:?}",
                ws[0].health,
                ws[0].max_health,
                ws[1].health,
                ws[1].max_health,
                l.deaths,
                l.death_frames,
                l.respawns
            ));
            out.extend(run.files.iter().cloned());
        }
        other => panic!("unknown f-1 case {other}"),
    }
    Ok(out)
}
