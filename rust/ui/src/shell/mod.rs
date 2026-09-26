//! Step 4½d — the C++ shell: `StateStack` (`state.hpp`), `MainMenuState`
//! (`mainMenuState.cpp`), the router and `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`). T3 adds the
//! two concrete menus; T6 moves the 4½c live modules here; T7 adds the stack, the screens and
//! `Shell`. T6 moved the 4½c live modules here from `game` (`new_game`, `selection`,
//! `match_flow`, `viewport_step`, `loadout`), unchanged; `game` re-exports them.
//!
//! Step 4½e-1 (T3): the ordered `InputEvent` stream (keys and text), the settings menu's
//! sub-screens (`overlay`, `weapon_options`) on the stack, `cur_menu` in `MenuWorld`,
//! `level_path`, and the `ConfigStore` the shell owns.
pub mod level_path;
pub mod level_slot;
pub mod loadout;
pub mod main_menu;
pub mod match_flow;
pub mod new_game;
pub mod overlay;
pub mod playing;
pub mod selection;
pub mod settings_menu;
pub mod stack;
pub mod viewport_step;
pub mod weapon_options;

use std::io;
use std::path::{Path, PathBuf};

use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::frame::Scene;
use render::menu::menu_palette;
use scenario::SceneData;
use scenario::build::apply_live_settings;
use scenario::settings::Settings;
use scenario::storage::{self, ConfigStore};
use sim::state::{ControlState, SimState};

use crate::keys::{DK_ESCAPE, KeyLatch, TypedKey};
use crate::menu::Menu;
use crate::text::UiTc;
use level_slot::{LevelSlot, SeedSource};
use main_menu::{MA_NEW_GAME, MA_QUIT, MA_RESUME_GAME, MainMenuState, MenuCtx, main_menu};
use overlay::{InfoPurpose, InputPurpose, RefusalGate};
use playing::{Match, StartOptions};
use settings_menu::{SettingsModel, settings_menu};
use stack::{AfterUpdate, Screen, ScreenStack};

/// The two HUD switches of a `render::frame::Scene` (moved from `game::hud_mode`, Step 4½a-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudFlags {
    pub draw_hud: bool,
    pub map: bool,
}

impl HudFlags {
    /// Set `scene.draw_hud` and `scene.map`.
    pub fn apply(self, scene: &mut Scene<'_>) {
        scene.draw_hud = self.draw_hud;
        scene.map = self.map;
    }
}

/// The play renderer's size (`gfx.cpp:277`).
pub const SURFACE_W: i32 = 320;
pub const SURFACE_H: i32 = 200;

/// One keyboard event of a frame, as `Gfx::ProcessEvent` sees it (`gfx.cpp:595-640`): the DOS
/// scancode (`SDLToDOSKey`), down/up, the OS-repeat flag, and the key's `key_buf` symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub dos: u32,
    pub down: bool,
    pub repeat: bool,
    pub typed: TypedKey,
}

/// One SDL event of a frame, in SDL's order (Step 4½e-1): a key event, or one
/// `SDL_EVENT_TEXT_INPUT` string (`Utf8ToDos` makes it one byte, plan fact 7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Text(String),
}

/// One frame's input: its events in order, the sim's sampled control words (the 4½c sampler plus
/// touch — C++ key events reach the worms at the frame boundary, the 4½c equivalence), a fresh
/// seed for `SeedSource::Fresh`, the type-to-search clock, and the Rust-only F5 restart (Q5).
#[derive(Clone, Copy, Debug)]
pub struct ShellInput<'a> {
    pub events: &'a [InputEvent],
    pub sampled: [ControlState; 2],
    pub fresh_seed: u32,
    pub now_ms: u64,
    pub restart: bool,
}

impl ShellInput<'static> {
    pub fn idle() -> ShellInput<'static> {
        ShellInput {
            events: &[],
            sampled: [ControlState::new(); 2],
            fresh_seed: 0,
            now_ms: 0,
            restart: false,
        }
    }
}

/// What a frame shows (design §4.7): the surface at `fade` (`Gfx::Draw`'s `ScaleDraw`), or the
/// black frame of `MainMenuState::Enter`'s `Flip` at fade 0 (finding 7). `None` in `FrameOut` is
/// a pop frame: nothing is presented and the window keeps its image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Present {
    Frame { fade: i32 },
    Black,
}

/// The screen as the page and the tests see it (`window.lieroPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Menu,
    /// Step 4½e-1: an `InputStringState` is on top (the phone page shows its text field, D9).
    Text,
    Weapsel,
    Game,
    Quit,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Menu => "menu",
            Phase::Text => "text",
            Phase::Weapsel => "weapsel",
            Phase::Game => "game",
            Phase::Quit => "quit",
        }
    }
}

/// What the router did on a pop frame (`gfx.cpp:1491-1625`), or the F5 restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    NewGame { seed: u32 },
    Resume,
    Menu,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameOut {
    pub present: Option<Present>,
    /// Menu, selection and `StartGame` sounds (sample ids, in call order). The sim's own sounds
    /// stay in `SimState::sound_events` (drain them when `sim_ticked`).
    pub menu_sounds: Vec<i32>,
    pub sim_ticked: bool,
    pub quit: bool,
    /// The screen that ran `Update` this frame (read before the frame).
    pub upd: Phase,
    /// The screen after the frame.
    pub phase: Phase,
    pub routed: Option<Route>,
}

impl FrameOut {
    fn new(upd: Phase) -> FrameOut {
        FrameOut {
            present: None,
            menu_sounds: Vec::new(),
            sim_ticked: false,
            quit: false,
            upd,
            phase: upd,
            routed: None,
        }
    }
}

/// `Gfx::cur_menu` (`gfx.hpp:321`; plan fact 1): which menu has focus. 4½f adds the player
/// menu, 4½g the hidden one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurMenu {
    Main,
    Settings,
}

/// An overlay whose `Update` found it done this frame (`Shell::close`).
enum Closing {
    Input(InputPurpose, bool, Vec<u8>),
    Info(InfoPurpose, bool),
}

/// Everything `MainMenuState` touches — C++ `Gfx` members: the menus, `cur_menu`, `settings`,
/// `settings_node`'s name, `dos_keys`, the play renderer's `fade_value`, `bmp`, `pal32` and
/// `Origpal()`, `frozen_screen`, `menu_cycles`. No `SimState`.
pub struct MenuWorld {
    pub main_menu: Menu,
    pub settings_menu: Menu,
    pub cur_menu: CurMenu,
    pub settings: Settings,
    pub tc: UiTc,
    pub setup_name: String,
    pub keys: KeyLatch,
    pub fade: i32,
    pub menu_cycles: u32,
    pub surface: Bitmap,
    pub frozen: Bitmap,
    pub pal32: Pal32,
    pub origpal: Palette,
}

/// Test-only switches (plan T4 Step 8; T8's counterfactual witnesses): each `false` skips the
/// step it names. Both are `true` by default, which is the C++ behaviour.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellDebug {
    /// RESUME's live-settings resync (`apply_live_settings` + `Match::resync`).
    pub resume_sync: bool,
    /// The three `DrawTextSmall` labels in a match's draw.
    pub small_labels: bool,
}

impl Default for ShellDebug {
    fn default() -> Self {
        ShellDebug {
            resume_sync: true,
            small_labels: true,
        }
    }
}

/// C `atoi` over the entry buffer (`integerBehavior.cpp:60`): leading whitespace, an optional
/// sign, then decimal digits up to the first other byte. The buffer holds at most a few bytes
/// (`kDigits`, or a longer initial value), so an `i64` never overflows in practice; it saturates
/// anyway.
fn atoi(b: &[u8]) -> i64 {
    let mut i = 0;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let neg = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    if neg { -v } else { v }
}

/// C++ `Gfx` driving `StateStack` (design §4.1, §4.7): one [`Shell::frame`] per
/// `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`).
pub struct Shell {
    tc_root: PathBuf,
    /// Where `liero.cfg` and the level files live (Step 4½e-1; C++ `gfx`'s config nodes).
    store: Box<dyn ConfigStore>,
    world: MenuWorld,
    /// The boot game's scene: the font every screen draws with, and the boot palette.
    boot_scene: SceneData,
    stack: ScreenStack,
    /// `gfx.controller` once a NEW GAME made one (the boot controller is never focused).
    current: Option<Match>,
    level: LevelSlot,
    seeds: SeedSource,
    options: StartOptions,
    debug: ShellDebug,
}

impl Shell {
    fn new(
        tc_root: &Path,
        settings: Settings,
        store: Box<dyn ConfigStore>,
        seeds: SeedSource,
        fresh: u32,
        options: StartOptions,
    ) -> (Shell, SimState) {
        let tc = UiTc::load(tc_root);
        let boot_seed = seeds.boot(fresh);
        let level = LevelSlot::generate(tc_root, &settings, &*store, boot_seed);
        let boot = playing::boot_state(tc_root, &settings, &level.level, boot_seed);
        let world = MenuWorld {
            main_menu: main_menu(),
            settings_menu: settings_menu(),
            cur_menu: CurMenu::Main,
            origpal: boot.scene.origpal.clone(),
            settings,
            tc,
            setup_name: "liero".into(),
            keys: KeyLatch::default(),
            fade: 0,
            menu_cycles: 0,
            surface: Bitmap::new(SURFACE_W, SURFACE_H),
            frozen: Bitmap::new(SURFACE_W, SURFACE_H),
            pal32: [0; 256],
        };
        let shell = Shell {
            tc_root: tc_root.to_path_buf(),
            store,
            world,
            boot_scene: boot.scene,
            stack: ScreenStack::default(),
            current: None,
            level,
            seeds,
            options,
            debug: ShellDebug::default(),
        };
        (shell, boot.state)
    }

    /// `InitFrameStepping` (`gfx.cpp:1439-1465`): the boot level (the boot seed), the unstarted
    /// boot game drawn once, then the main menu, whose `Enter` presents the black frame. Returns
    /// the shell, the boot state (the `Sim` until the first NEW GAME), and the boot frame.
    /// `store` holds the config root (`liero.cfg`, the level files; Step 4½e-1).
    pub fn boot(
        tc_root: &Path,
        settings: Settings,
        store: Box<dyn ConfigStore>,
        seeds: SeedSource,
        fresh: u32,
        options: StartOptions,
    ) -> (Shell, SimState, FrameOut) {
        let (mut shell, state) = Shell::new(tc_root, settings, store, seeds, fresh, options);
        let mut out = FrameOut::new(Phase::Menu);
        shell.world.pal32 = playing::draw_boot(
            &mut shell.world.surface,
            &state,
            &shell.boot_scene,
            &shell.world.settings,
        );
        shell.world.fade = 0;
        shell.push_main_menu(&mut out);
        out.phase = shell.phase();
        (shell, state, out)
    }

    /// The skip route (design §7.5, Q4; Rust only): `[Playing]` from the start, a NEW GAME on the
    /// boot level (so `?seed=N` plays 4½c's first match: level and sim from N). Esc still opens
    /// the pause menu.
    pub fn boot_playing(
        tc_root: &Path,
        settings: Settings,
        store: Box<dyn ConfigStore>,
        seeds: SeedSource,
        fresh: u32,
        options: StartOptions,
    ) -> (Shell, SimState, FrameOut) {
        let (mut shell, mut state) = Shell::new(tc_root, settings, store, seeds, fresh, options);
        let mut out = FrameOut::new(Phase::Game);
        let input = ShellInput {
            fresh_seed: fresh,
            ..ShellInput::idle()
        };
        let seed = shell.new_game(&mut state, &input);
        shell.stack.push(Screen::Playing);
        shell.sync_picks();
        out.routed = Some(Route::NewGame { seed });
        out.phase = shell.phase();
        (shell, state, out)
    }

    /// `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`; design §3.1).
    pub fn frame(&mut self, sim: &mut SimState, input: &ShellInput) -> FrameOut {
        let mut out = FrameOut::new(self.phase());
        if self.stack.is_empty() {
            out.quit = true;
            return out;
        }
        if input.restart && matches!(self.stack.top(), Some(Screen::Playing)) {
            let seed = self.new_game(sim, input);
            out.routed = Some(Route::NewGame { seed });
        }
        // :1473-1485 — every event, in order, reaches the top's `HandleEvent`: `ProcessEvent`
        // (dos_keys, key_buf; it ignores text) for every screen; while Playing a non-repeat
        // key-down or any key-up also reaches LocalController::OnKey, whose only non-worm effect
        // is Esc (finding 9) — worm keys reach the sim as `sampled`; then the overlays' own arms
        // (`inputState.cpp:27-72`, `:177-183`).
        self.world.keys.begin_frame();
        let playing = matches!(self.stack.top(), Some(Screen::Playing));
        for ev in input.events {
            match ev {
                InputEvent::Key(ev) => {
                    if ev.down {
                        self.world.keys.key_down(ev.dos, ev.typed);
                    } else {
                        self.world.keys.key_up(ev.dos);
                    }
                    if playing && ev.dos == DK_ESCAPE && (!ev.down || !ev.repeat) {
                        self.current.as_mut().expect("Playing has a match").esc();
                    }
                    match self.stack.top_mut() {
                        Some(Screen::InputString(s)) => s.handle_key(ev),
                        Some(Screen::InfoBox(b)) => b.handle_key(ev),
                        _ => {}
                    }
                }
                InputEvent::Text(t) => {
                    if let Some(Screen::InputString(s)) = self.stack.top_mut() {
                        s.handle_text(t);
                    }
                }
            }
        }
        // :1487-1489, captured BEFORE Update. `menuStatePtr_` points at the MainMenuState wherever
        // it sits — under WEAPON OPTIONS, an entry or a box too (plan fact 3) — and C++ clears it
        // at its dispatch.
        let (sel, fading) = self
            .main_menu_state()
            .map_or((-1, false), |s| (s.selection(), s.is_fading_out()));
        let running = self.current.as_ref().is_some_and(Match::running);
        let gate = self.gate();
        let mut pushes = Vec::new();
        let mut closing = None;
        let keep = match self.stack.top_mut().expect("non-empty") {
            Screen::MainMenu(s) => {
                let mut cx = MenuCtx {
                    w: &mut self.world,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    pushes: Vec::new(),
                    now_ms: input.now_ms,
                    gate,
                };
                let keep = s.update(&mut cx);
                pushes = cx.pushes;
                keep
            }
            Screen::Playing => {
                let m = self.current.as_mut().expect("Playing has a match");
                let (keep, ticked) = m.process(sim, input.sampled, &mut out.menu_sounds);
                out.sim_ticked = ticked;
                keep
            }
            Screen::WeaponOptions(s) => {
                let mut cx = MenuCtx {
                    w: &mut self.world,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    pushes: Vec::new(),
                    now_ms: input.now_ms,
                    gate,
                };
                let keep = s.update(&mut cx);
                pushes = cx.pushes;
                keep
            }
            Screen::InputString(s) => match s.is_done() {
                None => true,
                Some((accepted, buffer)) => {
                    closing = Some(Closing::Input(s.purpose.clone(), accepted, buffer));
                    false
                }
            },
            Screen::InfoBox(b) => {
                if b.done {
                    closing = Some(Closing::Info(b.purpose.clone(), b.clear_screen));
                }
                !b.done
            }
        };
        // An overlay's close runs inside its `Update`, before the stack's pop test
        // (`inputState.cpp:75-84`, `:186-198`): a continuation may schedule a replacement.
        if let Some(c) = closing {
            self.close(c, &mut out);
        }
        match self.stack.finish_update(keep) {
            AfterUpdate::Popped { empty: true } => {
                debug_assert!(pushes.is_empty(), "a screen that pushes keeps running");
                self.route(sel, sim, input, &mut out);
                self.sync_picks();
                out.phase = self.phase();
                return out;
            }
            // `state.hpp:101-105`: the replacement's `Push` runs its `Enter` (plan fact 5).
            AfterUpdate::Replaced => {
                let mut top = self.stack.pop().expect("the replacement");
                self.enter(&mut top, &mut out);
                self.stack.push(top);
            }
            // A pop that leaves a screen continues to the draw with the new top (plan fact 5).
            AfterUpdate::Popped { empty: false } | AfterUpdate::Running => {}
        }
        // Plan fact 4: C++ pushes inside `Update` (`Push` runs `Enter` at once); Rust pushes after.
        for mut screen in pushes {
            debug_assert!(keep, "a screen that pushes keeps running");
            self.enter(&mut screen, &mut out);
            self.stack.push(screen);
        }
        // :1631-1647.
        let menu_flip = self.stack.top().expect("non-empty").wants_menu_flip();
        if menu_flip {
            self.update_menu_palettes(fading);
        }
        self.draw_stack(sim);
        if !menu_flip {
            self.world.menu_cycles = self.world.menu_cycles.wrapping_add(1);
        }
        out.present = Some(Present::Frame {
            fade: self.world.fade,
        });
        self.sync_picks();
        out.phase = self.phase();
        out
    }

    /// C++ `WeaponSelection` edits the shared `gfx.settings` worm settings in place (4½c finding
    /// 4; `weapsel.cpp:66`, `:255-282`, `:327`): while the match holds the menu's settings
    /// (`attached`, D7), the menu's picks are the selection's, every frame — the settings menu's
    /// `cfg16` and the exit save see a cycled pick at once (found by G2e-1 `weapon_options`,
    /// frame 376).
    fn sync_picks(&mut self) {
        let Some(picks) = self
            .current
            .as_ref()
            .filter(|m| m.attached())
            .and_then(Match::picks)
        else {
            return;
        };
        for (ws, p) in self.world.settings.worm_settings.iter_mut().zip(picks) {
            ws.weapons = p;
        }
    }

    /// The close half of an overlay's `Update` (`inputState.cpp:75-84`, `:186-198`), before its
    /// pop. `InputStringState`: `MenuSelect`, `ClearKeys`, the continuation. `InfoBoxState`:
    /// `ClearKeys`, the optional `Fill(bmp, 0)`, `on_dismiss` (none in e-1).
    fn close(&mut self, c: Closing, out: &mut FrameOut) {
        match c {
            Closing::Input(purpose, accepted, buffer) => {
                let select = self.world.tc.hooks.select;
                if select >= 0 {
                    out.menu_sounds.push(select);
                }
                self.world.keys.clear();
                self.input_done(purpose, accepted, &buffer);
            }
            Closing::Info(purpose, clear_screen) => {
                self.world.keys.clear();
                if clear_screen {
                    self.world.surface.fill(0, &self.world.pal32);
                }
                match purpose {
                    InfoPurpose::NoWeapons | InfoPurpose::Refused(_) => {}
                }
            }
        }
    }

    /// An `InputStringState`'s callback (`callback_(accepted_, buffer_)`).
    fn input_done(&mut self, purpose: InputPurpose, accepted: bool, buffer: &[u8]) {
        match purpose {
            // `integerBehavior.cpp:56-76` (plan fact 13): on accept with a non-empty result,
            // `atoi`, clamp to the displayed range, store `val * div`; then ALWAYS rewrite the
            // item's value from the field — no `UpdateItems`.
            InputPurpose::IntegerEntry(e) => {
                let w = &mut self.world;
                let mut model = SettingsModel {
                    settings: &mut w.settings,
                    tc: &w.tc,
                    setup_name: &w.setup_name,
                };
                let field = model
                    .int_field(e.item_id)
                    .expect("an integer entry names an integer setting");
                if accepted && !buffer.is_empty() {
                    let val = atoi(buffer).clamp(i64::from(e.min), i64::from(e.max)) as i32;
                    *field = val * e.div;
                }
                let mut value = (*field / e.div).to_string();
                if e.percentage {
                    value.push('%');
                }
                let item = w
                    .settings_menu
                    .item_from_id_mut(e.item_id)
                    .expect("the entry's item");
                item.value = value;
                item.has_value = true;
            }
        }
    }

    /// `StateStack::Push`'s `Enter` (`state.hpp:50-54`) of a screen about to go on the stack.
    fn enter(&mut self, screen: &mut Screen, out: &mut FrameOut) {
        match screen {
            Screen::MainMenu(s) => {
                out.present = Some(Present::Black);
                let running = self.current.as_ref().is_some_and(Match::running);
                let gate = self.gate();
                s.enter(&mut MenuCtx {
                    w: &mut self.world,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    pushes: Vec::new(),
                    now_ms: 0,
                    gate,
                });
            }
            Screen::Playing => unreachable!("only the router pushes Playing"),
            Screen::WeaponOptions(s) => s.enter(&mut self.world),
            // `InputStringState::Enter` is `SDL_StartTextInput` (the page's text field keys on
            // `Phase::Text`, D9); `InfoBoxState::Enter` is empty.
            Screen::InputString(_) | Screen::InfoBox(_) => {}
        }
    }

    /// The router (`gfx.cpp:1491-1625`), on the frame the top popped and left the stack empty.
    fn route(&mut self, sel: i32, sim: &mut SimState, input: &ShellInput, out: &mut FrameOut) {
        if sel >= 0 {
            match sel {
                MA_QUIT => {
                    // :1496-1498 — no present: the last one was the fade-out's black frame.
                    out.quit = true;
                    out.routed = Some(Route::Quit);
                    return;
                }
                MA_NEW_GAME => {
                    let seed = self.new_game(sim, input);
                    out.routed = Some(Route::NewGame { seed });
                }
                MA_RESUME_GAME => {
                    // :1525-1530 + GamePlayState::Enter → Focus. Before it, the settings the
                    // menu edited reach the paused match, as C++'s shared `gfx.settings` does
                    // (finding 1, facts 14-16): the sim's live-read set, then the match's own
                    // copies (plan T4 Step 6).
                    let m = self
                        .current
                        .as_mut()
                        .expect("RESUME is shown only while a match runs");
                    if m.attached() && self.debug.resume_sync {
                        apply_live_settings(sim, &self.world.settings);
                        m.resync(&self.world.settings);
                    }
                    m.focus(&input.sampled);
                    out.routed = Some(Route::Resume);
                }
                other => unreachable!("main-menu item {other} never selects in 4½d (§5)"),
            }
            self.stack.push(Screen::Playing);
        } else {
            // :1594-1625 — back to the menu: Unfocus, ClearKeys, one draw of the pop frame's
            // tick (finding 12; the only draw it gets), then a new MainMenuState.
            let m = self
                .current
                .as_mut()
                .expect("only Playing pops without a selection");
            m.unfocus();
            self.world.keys.clear();
            self.world.pal32 = m.draw(
                &mut self.world.surface,
                &mut self.world.frozen,
                sim,
                self.world.menu_cycles,
                self.debug.small_labels,
            );
            self.world.fade = m.fade();
            self.push_main_menu(out);
            out.routed = Some(Route::Menu);
        }
    }

    /// NEW GAME (`gfx.cpp:1507-1523`; design §4.11): the old controller's picks, the next seed,
    /// the level (reused as played, or generated from the seed), a new `Match` — already focused,
    /// its `WeaponSelection` constructed — and the router's only write to the sim: replace it.
    fn new_game(&mut self, sim: &mut SimState, input: &ShellInput) -> u32 {
        if let Some(old) = self.current.take() {
            old.write_back_picks(&mut self.world.settings);
        }
        let seed = self.seeds.next_match(input.fresh_seed);
        if self.level.reusable(&self.world.settings) {
            self.level.take_played(&sim.level);
        } else {
            self.level =
                LevelSlot::generate(&self.tc_root, &self.world.settings, &*self.store, seed);
        }
        let (m, state) = Match::start(
            &self.tc_root,
            &self.world.settings,
            &self.level.level,
            seed,
            &self.options,
            self.world.tc.begin,
            &input.sampled,
        );
        self.world.origpal = m.origpal().clone();
        *sim = state;
        self.current = Some(m);
        seed
    }

    /// `Push(MainMenuState)`: its `Enter` (with the black present) then the push.
    fn push_main_menu(&mut self, out: &mut FrameOut) {
        let mut s = Screen::MainMenu(MainMenuState::new());
        self.enter(&mut s, out);
        self.stack.push(s);
    }

    /// `Gfx::UpdateMenuPalettes(quitting)` (`gfx.cpp:978-1005`) for the play renderer.
    fn update_menu_palettes(&mut self, fading: bool) {
        let w = &mut self.world;
        if w.fade < 32 && !fading {
            w.fade += 1;
        }
        w.menu_cycles = w.menu_cycles.wrapping_add(1);
        let rgb = [
            w.settings.worm_settings[0].rgb,
            w.settings.worm_settings[1].rgb,
        ];
        w.pal32 = menu_palette(&w.origpal, w.menu_cycles, rgb);
    }

    /// `StateStack::Draw` (`state.hpp:114-131`).
    fn draw_stack(&mut self, sim: &SimState) {
        let running = self.current.as_ref().is_some_and(Match::running);
        let gate = self.gate();
        for i in self.stack.draw_from()..self.stack.len() {
            match &self.stack.screens()[i] {
                Screen::MainMenu(s) => {
                    let mut sounds = Vec::new();
                    s.draw(&mut MenuCtx {
                        w: &mut self.world,
                        font: &self.boot_scene.font,
                        running,
                        sounds: &mut sounds,
                        pushes: Vec::new(),
                        now_ms: 0,
                        gate,
                    });
                }
                Screen::Playing => {
                    let m = self.current.as_mut().expect("Playing has a match");
                    self.world.pal32 = m.draw(
                        &mut self.world.surface,
                        &mut self.world.frozen,
                        sim,
                        self.world.menu_cycles,
                        self.debug.small_labels,
                    );
                    self.world.fade = m.fade();
                }
                Screen::WeaponOptions(s) => s.draw(&mut self.world, &self.boot_scene.font),
                Screen::InputString(s) => s.draw(
                    &mut self.world.surface,
                    &self.world.frozen,
                    &self.world.pal32,
                    &self.boot_scene.font,
                ),
                Screen::InfoBox(b) => b.draw(
                    &mut self.world.surface,
                    &mut self.world.pal32,
                    &self.world.tc.exepal,
                    &self.boot_scene.font,
                ),
            }
        }
    }

    /// What `MainMenuState` needs for the Rust-only refusals (plan T4 Step 5).
    fn gate(&self) -> RefusalGate {
        RefusalGate {
            attached: self.current.as_ref().is_some_and(Match::attached),
            skip_selection: self.options.skip_selection,
            touch_only: self.options.touch_only,
            n_weapons: self.world.tc.weap_order.len(),
        }
    }

    /// The stack's `MainMenuState`, wherever it sits (C++ `menuStatePtr_`, plan fact 3).
    fn main_menu_state(&self) -> Option<&MainMenuState> {
        self.stack.screens().iter().rev().find_map(|s| match s {
            Screen::MainMenu(m) => Some(m),
            _ => None,
        })
    }

    pub fn phase(&self) -> Phase {
        match self.stack.top() {
            None => Phase::Quit,
            Some(Screen::MainMenu(_) | Screen::WeaponOptions(_) | Screen::InfoBox(_)) => {
                Phase::Menu
            }
            Some(Screen::InputString(_)) => Phase::Text,
            Some(Screen::Playing) => {
                if self.current.as_ref().is_some_and(Match::in_selection) {
                    Phase::Weapsel
                } else {
                    Phase::Game
                }
            }
        }
    }

    /// `M` / `G` / `O` / `I` / `B` / `-` (the G2 golden's `<top>`; `O`, `I`, `B` are WEAPON
    /// OPTIONS, an `InputStringState` and an `InfoBoxState`, Step 4½e-1).
    pub fn top_char(&self) -> char {
        match self.stack.top() {
            None => '-',
            Some(Screen::MainMenu(_)) => 'M',
            Some(Screen::Playing) => 'G',
            Some(Screen::WeaponOptions(_)) => 'O',
            Some(Screen::InputString(_)) => 'I',
            Some(Screen::InfoBox(_)) => 'B',
        }
    }

    pub fn surface(&self) -> &Bitmap {
        &self.world.surface
    }

    pub fn frozen(&self) -> &Bitmap {
        &self.world.frozen
    }

    pub fn pal32(&self) -> &Pal32 {
        &self.world.pal32
    }

    /// `play_renderer.fade_value`.
    pub fn fade(&self) -> i32 {
        self.world.fade
    }

    pub fn menu_cycles(&self) -> u32 {
        self.world.menu_cycles
    }

    /// `main_menu.Selection()`.
    pub fn main_selection(&self) -> i32 {
        self.world.main_menu.selection()
    }

    /// Whether the main menu is fading out after a selection (the G2 generator's placeholder check).
    pub fn menu_fading(&self) -> bool {
        self.main_menu_state()
            .is_some_and(MainMenuState::is_fading_out)
    }

    /// `gfx.cur_menu` (Step 4½e-1): which menu has focus.
    pub fn cur_menu(&self) -> CurMenu {
        self.world.cur_menu
    }

    /// The Rust-only refusal the top info box shows, if one is up (plan D5): `game` logs it to
    /// the browser console, where the `eprintln!` goes nowhere (Step 4½e-1, T10).
    pub fn top_refusal(&self) -> Option<&overlay::Refusal> {
        match self.stack.top() {
            Some(Screen::InfoBox(b)) => match &b.purpose {
                InfoPurpose::Refused(r) => Some(r),
                InfoPurpose::NoWeapons => None,
            },
            _ => None,
        }
    }

    /// The config store (Step 4½e-1).
    pub fn store(&self) -> &dyn ConfigStore {
        &*self.store
    }

    /// `gfx.settings->save(user/Setups/liero.cfg)` at exit (`gameEntry.cpp:78`, finding 12):
    /// the current settings written to the store's `Setups/liero.cfg`.
    pub fn save_on_exit(&self) -> io::Result<()> {
        storage::save_setup(&*self.store, &self.world.settings)
    }

    /// `gfx.settings_menu`.
    pub fn settings_menu(&self) -> &Menu {
        &self.world.settings_menu
    }

    /// For tests: the settings menu, to place its cursor.
    pub fn settings_menu_mut(&mut self) -> &mut Menu {
        &mut self.world.settings_menu
    }

    /// Test-only switches (T8's counterfactual witnesses).
    #[doc(hidden)]
    pub fn debug_mut(&mut self) -> &mut ShellDebug {
        &mut self.debug
    }

    pub fn main_menu(&self) -> &Menu {
        &self.world.main_menu
    }

    /// For tests and 4½e's settings focus.
    pub fn main_menu_mut(&mut self) -> &mut Menu {
        &mut self.world.main_menu
    }

    pub fn settings(&self) -> &Settings {
        &self.world.settings
    }

    /// For tests and 4½e (MATCH SETUP edits the settings).
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.world.settings
    }

    pub fn current(&self) -> Option<&Match> {
        self.current.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use scenario::paths::TC_ROOT;
    use scenario::storage::MemoryStore;

    use super::*;
    use crate::keys::{
        DK_DOWN, DK_ESCAPE, DK_F1, DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9, DK_PGUP,
        DK_RETURN, DK_UP,
    };
    use crate::shell::main_menu::MA_TC;
    use crate::shell::new_game::generate_level;

    fn tc() -> &'static Path {
        Path::new(TC_ROOT)
    }

    fn hooks() -> UiTc {
        UiTc::load(tc())
    }

    fn boot() -> (Shell, SimState, FrameOut) {
        let seeds = SeedSource::Scripted {
            boot: 11,
            matches: VecDeque::from([21, 22, 23]),
        };
        Shell::boot(
            tc(),
            Settings::default(),
            Box::new(MemoryStore::new()),
            seeds,
            0,
            StartOptions::default(),
        )
    }

    fn ev(dos: u32, down: bool) -> KeyEvent {
        KeyEvent {
            dos,
            down,
            repeat: false,
            typed: TypedKey::Sym(0),
        }
    }

    fn step(sh: &mut Shell, sim: &mut SimState, events: &[KeyEvent], words: [u32; 2]) -> FrameOut {
        let events: Vec<InputEvent> = events.iter().copied().map(InputEvent::Key).collect();
        step_ev(sh, sim, &events, words)
    }

    fn step_ev(
        sh: &mut Shell,
        sim: &mut SimState,
        events: &[InputEvent],
        words: [u32; 2],
    ) -> FrameOut {
        let input = ShellInput {
            events,
            sampled: words.map(ControlState::unpack),
            ..ShellInput::idle()
        };
        sh.frame(sim, &input)
    }

    fn idle(sh: &mut Shell, sim: &mut SimState, n: usize) -> Vec<FrameOut> {
        (0..n).map(|_| step(sh, sim, &[], [0, 0])).collect()
    }

    /// A key-down this frame, its key-up the next; returns the key-down frame.
    fn tap(sh: &mut Shell, sim: &mut SimState, dos: u32) -> FrameOut {
        let out = step(sh, sim, &[ev(dos, true)], [0, 0]);
        step(sh, sim, &[ev(dos, false)], [0, 0]);
        out
    }

    /// Tap `dos`, then run until the router acts; every frame from the tap on.
    fn until_routed(sh: &mut Shell, sim: &mut SimState, dos: u32) -> Vec<FrameOut> {
        let mut outs = vec![
            step(sh, sim, &[ev(dos, true)], [0, 0]),
            step(sh, sim, &[ev(dos, false)], [0, 0]),
        ];
        while outs.last().unwrap().routed.is_none() {
            outs.push(step(sh, sim, &[], [0, 0]));
            assert!(outs.len() < 300, "never routed");
        }
        outs
    }

    /// Boot → NEW GAME → both players Up + Fire → the match's first tick.
    fn start_match(sh: &mut Shell, sim: &mut SimState) {
        idle(sh, sim, 40);
        until_routed(sh, sim, DK_RETURN);
        step(sh, sim, &[], [1, 1]);
        step(sh, sim, &[], [0, 0]);
        step(sh, sim, &[], [16, 16]);
        step(sh, sim, &[], [0, 0]);
        assert_eq!(sh.phase(), Phase::Game);
    }

    #[test]
    fn boot_presents_one_black_frame_then_the_menu_fades_in() {
        let (mut sh, mut sim, out) = boot();
        assert_eq!(
            (out.present, out.phase),
            (Some(Present::Black), Phase::Menu),
            "Enter's Flip (finding 7)"
        );
        assert_eq!(
            (
                sh.fade(),
                sh.menu_cycles(),
                sh.top_char(),
                sh.main_selection()
            ),
            (0, 0, 'M', 1)
        );
        let m = sh.main_menu();
        assert!(
            !m.items[0].visible,
            "RESUME hidden at boot (Running() is false)"
        );
        assert_eq!(m.items[1].string, "NEW GAME (F1)");
        assert_eq!(m.item_from_id(MA_TC).unwrap().string, "TC (openliero)");
        let outs = idle(&mut sh, &mut sim, 40);
        for (k, o) in outs.iter().enumerate() {
            assert_eq!(
                o.present,
                Some(Present::Frame {
                    fade: (k as i32 + 1).min(32)
                }),
                "frame {k}"
            );
        }
        assert_eq!(sh.menu_cycles(), 40, "finding 8");
    }

    #[test]
    fn the_boot_background_carries_the_copyright_bar() {
        let (sh, _sim, _) = boot();
        let pal = *sh.pal32();
        let mut text = false;
        for y in 151..158 {
            for x in 0..160 {
                let p = sh.frozen().get_pixel(x, y);
                assert!(
                    p == pal[0] || p == pal[19],
                    "({x},{y}) is the bar (mainMenuState.cpp:107-108)"
                );
                text |= p == pal[19];
            }
        }
        assert!(text);
    }

    #[test]
    fn the_keys_move_the_cursor_with_crossed_sounds() {
        let (mut sh, mut sim, _) = boot();
        let h = hooks().hooks;
        let o = tap(&mut sh, &mut sim, DK_DOWN);
        assert_eq!(
            (sh.main_selection(), o.menu_sounds),
            (2, vec![h.move_up]),
            "Down plays MenuMoveUp"
        );
        let o = tap(&mut sh, &mut sim, DK_UP);
        assert_eq!((sh.main_selection(), o.menu_sounds), (1, vec![h.move_down]));
        tap(&mut sh, &mut sim, DK_UP);
        assert_eq!(
            sh.main_selection(),
            14,
            "wraps past the hidden RESUME to MATCH SETUP"
        );
        tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(sh.main_selection(), 9, "Esc: QUIT TO OS");
        tap(&mut sh, &mut sim, DK_UP);
        tap(&mut sh, &mut sim, 56);
        assert_eq!(sh.main_selection(), 9, "LAlt is P1 jump");
        tap(&mut sh, &mut sim, DK_DOWN);
        assert_eq!(sh.main_selection(), 11, "over the spacer");
        let o = tap(&mut sh, &mut sim, DK_PGUP);
        assert_eq!(
            (sh.main_selection(), o.menu_sounds),
            (4, vec![h.move_down]),
            "visible 10 - 7 = 3 -> HOST ONLINE"
        );
    }

    #[test]
    fn placeholders_play_select_but_never_select_and_f_keys_are_inert() {
        // MATCH SETUP and F7 are live since 4½e-1 (`the_settings_focus_*` below).
        let (mut sh, mut sim, _) = boot();
        let s = hooks().hooks.select;
        for (idx, want) in [
            (2, 1),
            (3, 2),
            (4, 2),
            (5, 2),
            (6, 1),
            (7, 1),
            (8, 1),
            (11, 1),
            (12, 1),
            (13, 1),
        ] {
            sh.main_menu_mut().move_to(idx);
            let o = tap(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(
                o.menu_sounds,
                vec![s; want],
                "item {idx} (plan-time fact 2)"
            );
        }
        sh.main_menu_mut().move_to(1);
        for dos in [DK_F2, DK_F3, DK_F5, DK_F6, DK_F8, DK_F9] {
            let o = tap(&mut sh, &mut sim, dos);
            assert!(o.menu_sounds.is_empty());
        }
        assert_eq!(sh.main_selection(), 1);
        assert!(
            idle(&mut sh, &mut sim, 40)
                .iter()
                .all(|o| o.routed.is_none() && o.phase == Phase::Menu)
        );
    }

    #[test]
    fn new_game_fades_out_then_the_pop_frame_routes_into_selection() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        let boot_level = sim.level.material_id.clone();
        let mc = sh.menu_cycles();
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        let got: Vec<Option<Present>> = outs.iter().map(|o| o.present).collect();
        let mut want: Vec<Option<Present>> = (0..=32)
            .rev()
            .map(|f| Some(Present::Frame { fade: f }))
            .collect();
        want.push(None);
        assert_eq!(
            got, want,
            "32 on the select frame, 31..0, then a pop frame with no present (finding 14)"
        );
        let pop = outs.last().unwrap();
        assert_eq!(
            (pop.routed, pop.upd, pop.phase),
            (
                Some(Route::NewGame { seed: 21 }),
                Phase::Menu,
                Phase::Weapsel
            )
        );
        assert_eq!(sh.menu_cycles(), mc + 33, "the pop frame counts nothing");
        assert_eq!(
            sim.rand.draws(),
            0,
            "default picks: the constructor draws nothing (T8 intervention 3)"
        );
        assert_eq!(
            sim.level.material_id, boot_level,
            "the first NEW GAME reuses the boot level"
        );
    }

    #[test]
    fn selection_inherits_menu_cycles_and_done_plays_begin() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        until_routed(&mut sh, &mut sim, DK_RETURN);
        let mc = sh.menu_cycles();
        let origpal = sh.current().unwrap().origpal().clone();
        let o = step(&mut sh, &mut sim, &[], [1, 1]);
        assert_eq!(
            *sh.pal32(),
            render::weapsel::weapsel_palette(&origpal, mc),
            "finding 8: the menu's count"
        );
        assert_eq!(
            (sh.menu_cycles(), o.present),
            (mc + 1, Some(Present::Frame { fade: 1 }))
        );
        step(&mut sh, &mut sim, &[], [0, 0]);
        let o = step(&mut sh, &mut sim, &[], [16, 16]);
        assert_eq!(
            o.menu_sounds.last(),
            Some(&hooks().begin),
            "StartGame's SoundBegin (plan-time fact 9)"
        );
        assert_eq!(
            (o.upd, o.phase, sh.fade()),
            (Phase::Weapsel, Phase::Game, 33)
        );
        assert!(sim.worms.iter().all(|w| w.lives == 15));
    }

    fn to_menu(sh: &mut Shell, sim: &mut SimState) -> Vec<FrameOut> {
        until_routed(sh, sim, DK_ESCAPE)
    }

    #[test]
    fn esc_in_play_ticks_31_faded_frames_then_returns_to_a_pause_menu() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        idle(&mut sh, &mut sim, 5);
        let cycles = sim.cycles;
        let outs = to_menu(&mut sh, &mut sim);
        let got: Vec<Option<Present>> = outs.iter().map(|o| o.present).collect();
        let mut want: Vec<Option<Present>> = (0..=30)
            .rev()
            .map(|f| Some(Present::Frame { fade: f }))
            .collect();
        want.push(Some(Present::Black));
        assert_eq!(
            got, want,
            "OnKey: 31, the tail 30..0, then the pop frame's Enter flip"
        );
        assert!(
            outs.iter().all(|o| o.sim_ticked),
            "32 ticks, the pop frame's included (finding 9)"
        );
        assert_eq!(sim.cycles, cycles + 32);
        let m = sh.main_menu();
        assert_eq!(
            (
                m.items[0].visible,
                m.items[0].string.as_str(),
                m.items[1].string.as_str()
            ),
            (true, "RESUME GAME (F1)", "NEW GAME")
        );
        assert_eq!(
            (
                sh.main_selection(),
                sh.fade(),
                sh.menu_cycles(),
                sh.top_char()
            ),
            (0, 0, 0, 'M')
        );
    }

    #[test]
    fn an_esc_key_up_pauses_but_an_os_repeat_does_not() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        let rep = KeyEvent {
            dos: DK_ESCAPE,
            down: true,
            repeat: true,
            typed: TypedKey::Sym(0),
        };
        assert_eq!(
            step(&mut sh, &mut sim, &[rep], [0, 0]).present,
            Some(Present::Frame { fade: 33 }),
            "repeats never reach OnKey"
        );
        assert_eq!(
            step(&mut sh, &mut sim, &[ev(DK_ESCAPE, false)], [0, 0]).present,
            Some(Present::Frame { fade: 30 }),
            "finding 9"
        );
    }

    #[test]
    fn resume_refocuses_the_same_match_and_fades_in_from_zero() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        to_menu(&mut sh, &mut sim);
        let cycles = sim.cycles;
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(outs.last().unwrap().routed, Some(Route::Resume));
        assert_eq!(sim.cycles, cycles, "the menu never ticks the match");
        assert_eq!(
            step(&mut sh, &mut sim, &[], [0, 0]).present,
            Some(Present::Frame { fade: 1 })
        );
        assert_eq!(sim.cycles, cycles + 1);
    }

    #[test]
    fn new_game_after_play_reuses_the_played_level_with_the_next_seed() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        sim.level.material_id[4321] ^= 0x5a; // a crater
        to_menu(&mut sh, &mut sim); // 32 more ticks, then the pause menu
        let played = sim.level.material_id.clone();
        assert_eq!(played[4321], sim.level.material_id[4321]);
        tap(&mut sh, &mut sim, DK_DOWN);
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(
            outs.last().unwrap().routed,
            Some(Route::NewGame { seed: 22 })
        );
        assert_eq!(sim.level.material_id, played, "SwapLevel(*old_level)");
        assert_eq!(sh.phase(), Phase::Weapsel);
    }

    #[test]
    fn a_changed_level_setting_generates_from_the_match_seed() {
        let (mut sh, mut sim, _) = boot();
        sh.settings_mut().random_map_width = 512;
        until_routed(&mut sh, &mut sim, DK_RETURN);
        let want = generate_level(tc(), sh.settings(), None, 21);
        assert_eq!(
            (sim.level.width, &sim.level.material_id),
            (512, &want.material_id)
        );
    }

    #[test]
    fn resume_into_selection_draws_over_the_menus_frozen_screen() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        until_routed(&mut sh, &mut sim, DK_RETURN);
        step(&mut sh, &mut sim, &[], [1, 0]);
        step(&mut sh, &mut sim, &[], [0, 0]);
        to_menu(&mut sh, &mut sim);
        assert!(sh.main_menu().items[0].visible, "selection is Running()");
        let bar = |b: &Bitmap| -> Vec<u32> {
            (151..158)
                .flat_map(|y| (0..160).map(move |x| (x, y)))
                .map(|(x, y)| b.get_pixel(x, y))
                .collect()
        };
        let menu_bar = bar(sh.frozen());
        until_routed(&mut sh, &mut sim, DK_RETURN);
        step(&mut sh, &mut sim, &[], [0, 0]);
        assert_eq!(sh.phase(), Phase::Weapsel);
        assert_eq!(
            bar(sh.surface()),
            menu_bar,
            "finding 11: the copyright bar stays"
        );
    }

    #[test]
    fn quit_fades_out_and_ends_without_a_present() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        tap(&mut sh, &mut sim, DK_ESCAPE);
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        let last = outs.last().unwrap();
        assert_eq!(
            (last.quit, last.present, last.routed, last.phase),
            (true, None, Some(Route::Quit), Phase::Quit)
        );
        assert_eq!(
            outs[outs.len() - 2].present,
            Some(Present::Frame { fade: 0 }),
            "the last present is black"
        );
        assert!(step(&mut sh, &mut sim, &[], [0, 0]).quit);
    }

    #[test]
    fn game_over_runs_180_frames_then_returns_to_a_menu_without_resume() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        sim.worms[1].lives = 0;
        let mut n = 0;
        loop {
            let o = step(&mut sh, &mut sim, &[], [0, 0]);
            n += 1;
            if let Some(r) = o.routed {
                assert_eq!(r, Route::Menu);
                break;
            }
            assert!(n < 400);
        }
        assert_eq!(
            n, 181,
            "the detection frame (179) ... 0, then the pop frame"
        );
        let m = sh.main_menu();
        assert_eq!(
            (
                m.items[0].visible,
                m.items[1].string.as_str(),
                sh.main_selection()
            ),
            (false, "NEW GAME (F1)", 1)
        );
    }

    #[test]
    fn f1_is_new_game_at_boot_and_resume_when_paused() {
        let (mut sh, mut sim, _) = boot();
        assert_eq!(
            until_routed(&mut sh, &mut sim, DK_F1)
                .last()
                .unwrap()
                .routed,
            Some(Route::NewGame { seed: 21 })
        );
        to_menu(&mut sh, &mut sim);
        assert_eq!(
            until_routed(&mut sh, &mut sim, DK_F1)
                .last()
                .unwrap()
                .routed,
            Some(Route::Resume)
        );
    }

    #[test]
    fn f5_restart_is_the_next_new_game_without_the_menu() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        let o = sh.frame(
            &mut sim,
            &ShellInput {
                restart: true,
                ..ShellInput::idle()
            },
        );
        assert_eq!(
            (o.routed, o.phase, sh.top_char()),
            (Some(Route::NewGame { seed: 22 }), Phase::Weapsel, 'G')
        );
    }

    #[test]
    fn a_skip_route_boots_straight_into_play() {
        let opts = StartOptions {
            skip_selection: true,
            loadout: vec!["BAZOOKA".into()],
            touch_only: false,
        };
        let (sh, sim, out) = Shell::boot_playing(
            tc(),
            Settings::default(),
            Box::new(MemoryStore::new()),
            SeedSource::Fixed(7),
            0,
            opts,
        );
        assert_eq!(
            (out.phase, out.routed, sh.top_char()),
            (Phase::Game, Some(Route::NewGame { seed: 7 }), 'G')
        );
        let want = generate_level(tc(), &Settings::default(), None, 7);
        assert_eq!(
            sim.level.material_id, want.material_id,
            "?seed=7: 4½c's first match"
        );
        let (_, _, sel) = Shell::boot_playing(
            tc(),
            Settings::default(),
            Box::new(MemoryStore::new()),
            SeedSource::Fixed(7),
            0,
            StartOptions::default(),
        );
        assert_eq!(sel.phase, Phase::Weapsel, "?level= / ?seed= keep selection");
    }

    // Step 4½e-1 (T3): the sub-screen stack.

    fn entry_screen(initial: &str) -> Screen {
        Screen::InputString(overlay::InputStringState::new(
            initial.as_bytes(),
            3,
            120,
            60,
            Some(overlay::filter_digits),
            "",
            false,
            InputPurpose::IntegerEntry(crate::menu::ValueEntry {
                item_id: settings_menu::SI_LIVES,
                initial: initial.into(),
                digits: 3,
                x: 120,
                y: 60,
                min: 0,
                max: 999,
                div: 1,
                percentage: false,
            }),
        ))
    }

    const BOX_TEXT: &str = "AT LEAST\0ONE WEAPON";

    fn info_screen(clear_screen: bool) -> Screen {
        Screen::InfoBox(overlay::InfoBoxState::new(
            BOX_TEXT,
            160,
            100,
            clear_screen,
            InfoPurpose::NoWeapons,
        ))
    }

    fn key(dos: u32, down: bool) -> InputEvent {
        InputEvent::Key(ev(dos, down))
    }

    #[test]
    fn the_new_screens_name_their_phase_and_top() {
        let (mut sh, mut sim, _) = boot();
        assert_eq!(sh.cur_menu(), CurMenu::Main);
        for (screen, top, phase) in [
            (
                Screen::WeaponOptions(weapon_options::WeaponMenuState::default()),
                'O',
                Phase::Menu,
            ),
            (entry_screen(""), 'I', Phase::Text),
            (info_screen(false), 'B', Phase::Menu),
        ] {
            sh.stack.push(screen);
            let o = step(&mut sh, &mut sim, &[], [0, 0]);
            assert_eq!(
                (sh.top_char(), sh.phase(), o.upd, o.phase),
                (top, phase, phase, phase)
            );
            sh.stack.pop();
        }
        assert_eq!(Phase::Text.as_str(), "text");
    }

    #[test]
    fn an_entry_draws_over_the_main_menu_then_closes_with_select_and_clear_keys() {
        let (mut a, mut sim_a, _) = boot();
        let (mut b, mut sim_b, _) = boot();
        idle(&mut a, &mut sim_a, 40);
        idle(&mut b, &mut sim_b, 40);
        a.stack.push(entry_screen("15"));
        let (oa, ob) = (
            step(&mut a, &mut sim_a, &[], [0, 0]),
            step(&mut b, &mut sim_b, &[], [0, 0]),
        );
        assert_eq!(
            (oa.present, a.menu_cycles()),
            (ob.present, b.menu_cycles()),
            "WantsMenuFlip"
        );
        for y in (0..200).filter(|y| !(60..68).contains(y)) {
            for x in 0..320 {
                assert_eq!(
                    a.surface().get_pixel(x, y),
                    b.surface().get_pixel(x, y),
                    "({x},{y}): the menu below the overlay, drawn as without it"
                );
            }
        }
        assert_eq!(
            a.surface().get_pixel(118, 61),
            a.pal32()[0],
            "the field's box"
        );
        assert_ne!(a.surface(), b.surface());
        // A menu key typed during entry reaches dos_keys (ProcessEvent) but not the menu below.
        let select = hooks().hooks.select;
        let sel = a.main_selection();
        step_ev(&mut a, &mut sim_a, &[key(DK_DOWN, true)], [0, 0]);
        assert!(a.world.keys.test(DK_DOWN));
        let o = step_ev(
            &mut a,
            &mut sim_a,
            &[key(DK_RETURN, true), InputEvent::Text("4".into())],
            [0, 0],
        );
        assert_eq!(
            (o.menu_sounds, o.upd, o.phase, a.top_char()),
            (vec![select], Phase::Text, Phase::Menu, 'M'),
            "MenuSelect, pop, and the main menu drawn on the same frame"
        );
        assert_eq!(o.present, Some(Present::Frame { fade: 32 }));
        assert!(
            !a.world.keys.test(DK_DOWN) && !a.world.keys.test(DK_RETURN),
            "ClearKeys on close"
        );
        let o = step(
            &mut a,
            &mut sim_a,
            &[ev(DK_DOWN, false), ev(DK_RETURN, false)],
            [0, 0],
        );
        assert!(o.menu_sounds.is_empty());
        assert_eq!((a.main_selection(), a.menu_fading()), (sel, false));
    }

    #[test]
    fn an_info_box_draws_alone_on_the_stale_surface_until_a_key_down() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        let stale = sh.surface().clone();
        sh.stack.push(info_screen(false));
        step(&mut sh, &mut sim, &[], [0, 0]);
        let (w, h) = sh.boot_scene.font.get_dims_h(BOX_TEXT);
        let (cx, cy) = (160 - w / 2 - 2, 100 - h / 2 - 2);
        let in_box =
            |x: i32, y: i32| (cx..cx + w + 4).contains(&x) && (cy..cy + h + 1).contains(&y);
        for y in 0..200 {
            for x in (0..320).filter(|&x| !in_box(x, y)) {
                assert_eq!(
                    sh.surface().get_pixel(x, y),
                    stale.get_pixel(x, y),
                    "({x},{y})"
                );
            }
        }
        assert_eq!(sh.surface().get_pixel(cx + 1, cy), sh.pal32()[0]);
        let colour6 = (cy..cy + h + 1)
            .flat_map(|y| (cx..cx + w + 4).map(move |x| (x, y)))
            .filter(|&(x, y)| sh.surface().get_pixel(x, y) == sh.pal32()[6])
            .count();
        assert!(colour6 > 0, "the text in colour 6");
        step(&mut sh, &mut sim, &[ev(57, false)], [0, 0]);
        assert_eq!(sh.top_char(), 'B', "a key-up never dismisses");
        let rep = KeyEvent {
            dos: 57,
            down: true,
            repeat: true,
            typed: TypedKey::Sym(0),
        };
        let o = step(&mut sh, &mut sim, &[rep], [0, 0]);
        assert_eq!(
            (sh.top_char(), o.phase, o.menu_sounds.len()),
            ('M', Phase::Menu, 0),
            "an OS repeat dismisses; the box plays nothing"
        );
        assert!(!sh.world.keys.test(57), "ClearKeys");
    }

    #[test]
    fn a_clearing_box_swaps_the_palette_for_its_frames_only() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        sh.stack.push(info_screen(true));
        step(&mut sh, &mut sim, &[], [0, 0]);
        let exe = render::palette::pack_pal32(&hooks().exepal);
        assert_eq!(*sh.pal32(), exe, "pal = exepal, UpdatePal32");
        assert_eq!(sh.surface().get_pixel(0, 0), exe[0], "Fill(bmp, 0)");
        step(&mut sh, &mut sim, &[ev(57, true)], [0, 0]);
        assert_eq!(sh.top_char(), 'M');
        let w = &sh.world;
        let rgb = [
            w.settings.worm_settings[0].rgb,
            w.settings.worm_settings[1].rgb,
        ];
        assert_eq!(
            *sh.pal32(),
            menu_palette(&w.origpal, w.menu_cycles, rgb),
            "the next menu frame's UpdateMenuPalettes restores the rotation"
        );
    }

    #[test]
    fn a_scheduled_replacement_runs_its_enter() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        sh.world.cur_menu = CurMenu::Settings;
        let mut done = info_screen(false);
        if let Screen::InfoBox(b) = &mut done {
            b.done = true;
        }
        sh.stack.push(done);
        sh.stack
            .schedule_replace_top(Screen::MainMenu(MainMenuState::new()));
        let o = step(&mut sh, &mut sim, &[], [0, 0]);
        assert_eq!(
            (sh.stack.len(), sh.top_char(), o.phase),
            (2, 'M', Phase::Menu)
        );
        assert_eq!(
            (sh.cur_menu(), sh.menu_cycles()),
            (CurMenu::Main, 1),
            "MainMenuState::Enter ran (cur_menu, menu_cycles = 0), then the frame's draw"
        );
    }

    #[test]
    fn selection_and_fading_come_from_the_buried_main_menu() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        tap(&mut sh, &mut sim, DK_RETURN); // NEW GAME: fade 32, then 31
        let fade = sh.fade();
        sh.stack.push(info_screen(false));
        step(&mut sh, &mut sim, &[], [0, 0]);
        assert_eq!(
            sh.fade(),
            fade,
            "UpdateMenuPalettes(kMenuFadingOut) from menuStatePtr_ (plan fact 3): no fade-in"
        );
        assert!(sh.menu_fading());
        assert_eq!(
            sh.main_menu_state().map(MainMenuState::selection),
            Some(MA_NEW_GAME)
        );
        step(&mut sh, &mut sim, &[ev(57, true)], [0, 0]);
        let outs = until_routed(&mut sh, &mut sim, 57);
        assert_eq!(
            outs.last().unwrap().routed,
            Some(Route::NewGame { seed: 21 })
        );
    }

    #[test]
    fn cur_menu_moves_the_focus_in_the_draw() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        sh.world.cur_menu = CurMenu::Settings;
        step(&mut sh, &mut sim, &[], [0, 0]);
        assert_eq!(sh.cur_menu(), CurMenu::Settings);
        let w = &sh.world;
        let font = &sh.boot_scene.font;
        let mut want = w.frozen.clone();
        w.main_menu.draw(
            &crate::menu::PlainModel,
            &mut want,
            &w.pal32,
            font,
            true,
            -1,
            true,
        );
        w.settings_menu.draw(
            &crate::menu::PlainModel,
            &mut want,
            &w.pal32,
            font,
            false,
            -1,
            false,
        );
        assert_eq!(
            *sh.surface(),
            want,
            "DrawBasicMenu disables the main menu; the settings menu is drawn enabled"
        );
    }

    // Step 4½e-1 (T4): the settings focus, the Enter dispatch, number entry, WEAPON OPTIONS, the
    // refusals, the RESUME resync, the exit save.

    use crate::keys::{DK_BACKSPACE, DK_LEFT, DK_PGDN, DK_RIGHT, K_UP};
    use crate::shell::main_menu::MA_SETTINGS;
    use crate::shell::settings_menu::*;

    /// Boot, fade the menu in, F7: the settings menu has focus.
    fn settings_focus() -> (Shell, SimState) {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        tap(&mut sh, &mut sim, DK_F7);
        assert_eq!(sh.cur_menu(), CurMenu::Settings);
        (sh, sim)
    }

    /// Put the settings cursor on `id` and tap Return; the Return frame's output.
    fn enter_on(sh: &mut Shell, sim: &mut SimState, id: i32) -> FrameOut {
        sh.settings_menu_mut().move_to_id(id);
        assert_eq!(sh.settings_menu().selected_id(), id, "item {id} is visible");
        tap(sh, sim, DK_RETURN)
    }

    fn text(t: &str) -> InputEvent {
        InputEvent::Text(t.into())
    }

    /// One frame of text events (and an optional key), one char each (plan fact 7).
    fn type_str(sh: &mut Shell, sim: &mut SimState, t: &str) {
        let events: Vec<InputEvent> = t.chars().map(|c| text(&c.to_string())).collect();
        step_ev(sh, sim, &events, [0, 0]);
    }

    fn item_value(sh: &Shell, id: i32) -> String {
        sh.settings_menu().item_from_id(id).unwrap().value.clone()
    }

    fn top_box(sh: &Shell) -> &overlay::InfoBoxState {
        match sh.stack.top() {
            Some(Screen::InfoBox(b)) => b,
            _ => panic!("an info box is on top, not {}", sh.top_char()),
        }
    }

    fn weapon_state(sh: &Shell) -> &weapon_options::WeaponMenuState {
        sh.stack
            .screens()
            .iter()
            .rev()
            .find_map(|s| match s {
                Screen::WeaponOptions(w) => Some(w),
                _ => None,
            })
            .expect("WEAPON OPTIONS is on the stack")
    }

    #[test]
    fn the_settings_focus_comes_from_f7_and_match_setup_and_esc_returns() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        let o = tap(&mut sh, &mut sim, DK_F7);
        assert_eq!(
            (sh.cur_menu(), sh.main_menu().selected_id(), o.menu_sounds),
            (CurMenu::Settings, MA_SETTINGS, vec![]),
            "F7: MoveToId(MATCH SETUP), focus, no sound"
        );
        tap(&mut sh, &mut sim, DK_DOWN);
        tap(&mut sh, &mut sim, DK_DOWN);
        let ssel = sh.settings_menu().selection();
        let o = tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(
            (sh.cur_menu(), sh.main_menu().selected_id(), o.menu_sounds),
            (CurMenu::Main, MA_SETTINGS, vec![]),
            "Esc in settings focus: back to main, the cursor stays (not QUIT), no sound"
        );
        let o = tap(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(
            (sh.cur_menu(), o.menu_sounds),
            (CurMenu::Settings, vec![hooks().hooks.select]),
            "Enter on MATCH SETUP: MenuSelect, focus"
        );
        assert_eq!(sh.settings_menu().selection(), ssel, "the cursor survives");
        tap(&mut sh, &mut sim, DK_ESCAPE);
        tap(&mut sh, &mut sim, DK_F7);
        assert_eq!(sh.settings_menu().selection(), ssel, "…and F7");
        // Up / Down / PgDn act on the settings menu, not the main menu.
        let main = sh.main_selection();
        let o = tap(&mut sh, &mut sim, DK_UP);
        assert_eq!(o.menu_sounds, vec![hooks().hooks.move_down]);
        assert_eq!(sh.main_selection(), main);
        let mut want = sh.settings_menu().clone();
        want.movement_page(1);
        tap(&mut sh, &mut sim, DK_PGDN);
        assert_eq!(sh.settings_menu().selection(), want.selection());
        // F1 from settings focus: focus back to main, and NEW GAME starts.
        let outs = until_routed(&mut sh, &mut sim, DK_F1);
        assert_eq!(
            outs.last().unwrap().routed,
            Some(Route::NewGame { seed: 21 })
        );
        assert_eq!(outs[0].menu_sounds, vec![], "F1 plays nothing");
        // After a match, MainMenuState::Enter resets the focus and the settings cursor.
        step(&mut sh, &mut sim, &[], [1, 1]);
        step(&mut sh, &mut sim, &[], [0, 0]);
        step(&mut sh, &mut sim, &[], [16, 16]);
        step(&mut sh, &mut sim, &[], [0, 0]);
        to_menu(&mut sh, &mut sim);
        assert_eq!(sh.cur_menu(), CurMenu::Main);
        let mut first = sh.settings_menu().clone();
        first.move_to_first_visible();
        assert_eq!(sh.settings_menu().selection(), first.selection());
        assert_ne!(first.selection(), ssel);
    }

    #[test]
    fn every_settings_enter_arm_plays_exactly_one_select() {
        let select = hooks().hooks.select;
        for id in [
            SI_GAME_MODE,
            SI_LIVES,
            SI_LEVEL,
            SI_RANDOM_MAP_WIDTH,
            SI_RANDOM_MAP_HEIGHT,
            SI_LOADING_TIMES,
            SI_WEAPON_OPTIONS,
            SI_MAX_BONUSES,
            SI_NAMES_ON_BONUSES,
            SI_MAP,
            SI_AMOUNT_OF_BLOOD,
            LOAD_CHANGE,
            SI_REGENERATE_LEVEL,
            SAVE_OPTIONS,
            LOAD_OPTIONS,
            SI_TIME_TO_LOSE,
        ] {
            let (mut sh, mut sim) = settings_focus();
            if id == SI_TIME_TO_LOSE {
                // TIME TO LOSE shows in Game of Tag (OnUpdate's visibility).
                sh.settings_mut().game_mode = scenario::settings::GM_GAME_OF_TAG;
                sh.world.settings_menu.update_items(&mut SettingsModel {
                    settings: &mut sh.world.settings,
                    tc: &sh.world.tc,
                    setup_name: &sh.world.setup_name,
                });
            }
            let before = sh.settings().clone();
            let o = enter_on(&mut sh, &mut sim, id);
            assert_eq!(o.menu_sounds, vec![select], "item {id}: one MenuSelect");
            let s = sh.settings().clone();
            let (top, changed) = match id {
                SI_GAME_MODE => ('M', s.game_mode == before.game_mode + 1),
                SI_LIVES | SI_RANDOM_MAP_WIDTH | SI_RANDOM_MAP_HEIGHT | SI_LOADING_TIMES
                | SI_MAX_BONUSES => ('I', s == before),
                SI_WEAPON_OPTIONS => ('O', s == before),
                SI_NAMES_ON_BONUSES => ('M', s.names_on_bonuses != before.names_on_bonuses),
                SI_MAP => ('M', s.map != before.map),
                LOAD_CHANGE => ('M', s.load_change != before.load_change),
                SI_REGENERATE_LEVEL => ('M', s.regenerate_level != before.regenerate_level),
                // Sound only: blood (allow_entry = false), a time, and the e-2 pushes (D6).
                _ => ('M', s == before),
            };
            assert_eq!(sh.top_char(), top, "item {id}");
            assert!(changed, "item {id}");
            assert_eq!(sh.cur_menu(), CurMenu::Settings, "item {id}");
            assert!(!sh.menu_fading(), "item {id}: nothing selected");
        }
    }

    #[test]
    fn held_left_right_repeats_integers_and_releases_after_bools_and_enums() {
        let (mut sh, mut sim) = settings_focus();
        let h = hooks().hooks;
        sh.settings_menu_mut().move_to_id(SI_LIVES);
        // Held Right: IntegerBehavior acts when menu_cycles % 5 == 0 (integerBehavior.cpp:15).
        let mut want = sh.settings().lives;
        for k in 0..13 {
            if sh.menu_cycles() % 5 == 0 {
                want += 1;
            }
            let events = if k == 0 {
                vec![ev(DK_RIGHT, true)]
            } else {
                vec![]
            };
            let o = step(&mut sh, &mut sim, &events, [0, 0]);
            assert!(o.menu_sounds.is_empty(), "an integer plays nothing");
        }
        step(&mut sh, &mut sim, &[ev(DK_RIGHT, false)], [0, 0]);
        assert!(want >= 17, "the cadence ran");
        assert_eq!(sh.settings().lives, want);
        assert_eq!(
            item_value(&sh, SI_LIVES),
            sh.settings().lives.to_string(),
            "OnUpdate on each change"
        );
        // Bool: one toggle, MoveUp, then ResetLeftRight releases Right.
        sh.settings_menu_mut().move_to_id(SI_MAP);
        let map = sh.settings().map;
        let o = step(&mut sh, &mut sim, &[ev(DK_RIGHT, true)], [0, 0]);
        assert_eq!((sh.settings().map, o.menu_sounds), (!map, vec![h.move_up]));
        assert!(!sh.world.keys.test(DK_RIGHT), "ResetLeftRight");
        idle(&mut sh, &mut sim, 5);
        assert_eq!(sh.settings().map, !map, "held, but released: no repeat");
        step(&mut sh, &mut sim, &[ev(DK_RIGHT, false)], [0, 0]);
        // Enum: GAME MODE is cyclic; Left from Kill'em All is Scales of Justice, MoveDown.
        sh.settings_menu_mut().move_to_id(SI_GAME_MODE);
        let o = step(&mut sh, &mut sim, &[ev(DK_LEFT, true)], [0, 0]);
        assert_eq!(
            (sh.settings().game_mode, o.menu_sounds),
            (scenario::settings::GM_SCALES_OF_JUSTICE, vec![h.move_down])
        );
        assert!(!sh.world.keys.test(DK_LEFT));
        step(&mut sh, &mut sim, &[ev(DK_LEFT, false)], [0, 0]);
        assert_eq!(item_value(&sh, SI_GAME_MODE), "Scales of Justice");
    }

    #[test]
    fn number_entry_edits_backspaces_and_writes_back_on_return() {
        let (mut sh, mut sim) = settings_focus();
        let select = hooks().hooks.select;
        sh.settings_mut().lives = 7;
        // LIVES `7`: Enter starts the entry with the current value (ToString(v / div)).
        let o = enter_on(&mut sh, &mut sim, SI_LIVES);
        assert_eq!((sh.top_char(), sh.phase()), ('I', Phase::Text));
        let mut sounds = o.menu_sounds;
        match sh.stack.top() {
            Some(Screen::InputString(e)) => {
                assert_eq!((e.buffer.as_slice(), e.max_len), (&b"7"[..], 3));
            }
            _ => unreachable!(),
        }
        step(&mut sh, &mut sim, &[ev(DK_BACKSPACE, true)], [0, 0]);
        step(&mut sh, &mut sim, &[ev(DK_BACKSPACE, false)], [0, 0]);
        type_str(&mut sh, &mut sim, "42");
        let o = step(&mut sh, &mut sim, &[ev(DK_RETURN, true)], [0, 0]);
        sounds.extend(o.menu_sounds);
        step(&mut sh, &mut sim, &[ev(DK_RETURN, false)], [0, 0]);
        assert_eq!(
            (sh.settings().lives, item_value(&sh, SI_LIVES), sounds),
            (42, "42".to_string(), vec![select, select]),
            "the behavior's MenuSelect and the entry's"
        );
        assert_eq!((sh.top_char(), sh.cur_menu()), ('M', CurMenu::Settings));
    }

    /// Enter on `id`, Backspace `n` times, type `t`, Return.
    fn entry(sh: &mut Shell, sim: &mut SimState, id: i32, backspaces: usize, t: &str) {
        enter_on(sh, sim, id);
        assert_eq!(sh.top_char(), 'I');
        for _ in 0..backspaces {
            let rep = KeyEvent {
                repeat: true,
                ..ev(DK_BACKSPACE, true)
            };
            step(sh, sim, &[rep], [0, 0]);
        }
        type_str(sh, sim, t);
        tap(sh, sim, DK_RETURN);
        assert_eq!(sh.top_char(), 'M');
    }

    #[test]
    fn number_entry_clamps_and_keeps_the_value_on_esc_and_on_an_empty_return() {
        let (mut sh, mut sim) = settings_focus();
        entry(&mut sh, &mut sim, SI_RANDOM_MAP_WIDTH, 4, "9999");
        assert_eq!(sh.settings().random_map_width, 4096, "clamped to max");
        entry(&mut sh, &mut sim, SI_RANDOM_MAP_WIDTH, 4, "0");
        assert_eq!(sh.settings().random_map_width, 64, "clamped to min");
        entry(&mut sh, &mut sim, SI_RANDOM_MAP_WIDTH, 4, "333");
        assert_eq!(
            (
                sh.settings().random_map_width,
                item_value(&sh, SI_RANDOM_MAP_WIDTH)
            ),
            (333, "333".to_string()),
            "not rounded to the step"
        );
        // Four digits (1 + floor(log10 4096)): the fifth is dropped.
        entry(&mut sh, &mut sim, SI_RANDOM_MAP_HEIGHT, 3, "12345");
        assert_eq!(sh.settings().random_map_height, 1234);
        // A percentage: the `%` comes back with the value.
        entry(&mut sh, &mut sim, SI_LOADING_TIMES, 3, "50");
        assert_eq!(
            (
                sh.settings().loading_time,
                item_value(&sh, SI_LOADING_TIMES)
            ),
            (50, "50%".to_string())
        );
        // Esc keeps the value, even after typing; an empty Return keeps it too.
        enter_on(&mut sh, &mut sim, SI_MAX_BONUSES);
        type_str(&mut sh, &mut sim, "9");
        let o = tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(
            o.menu_sounds,
            vec![hooks().hooks.select],
            "a cancel plays it too"
        );
        assert_eq!(sh.settings().max_bonuses, 4);
        entry(&mut sh, &mut sim, SI_MAX_BONUSES, 1, "");
        assert_eq!(
            (sh.settings().max_bonuses, item_value(&sh, SI_MAX_BONUSES)),
            (4, "4".into())
        );
        // A non-digit is filtered; the value is untouched.
        entry(&mut sh, &mut sim, SI_MAX_BONUSES, 0, "x");
        assert_eq!(sh.settings().max_bonuses, 4);
        assert_eq!(
            sh.cur_menu(),
            CurMenu::Settings,
            "Esc closed the entry, not the focus"
        );
    }

    #[test]
    fn a_control_key_typed_during_entry_never_moves_the_cursor_after_the_close() {
        let (mut sh, mut sim) = settings_focus();
        let up = sh.settings().worm_settings[0].controls[K_UP];
        enter_on(&mut sh, &mut sim, SI_LIVES);
        let sel = sh.settings_menu().selection();
        let r = KeyEvent {
            typed: TypedKey::Sym(u32::from(b'r')),
            ..ev(up, true)
        };
        step_ev(&mut sh, &mut sim, &[InputEvent::Key(r), text("r")], [0, 0]);
        let o = step(&mut sh, &mut sim, &[ev(DK_RETURN, true)], [0, 0]);
        assert_eq!(o.menu_sounds, vec![hooks().hooks.select]);
        let o = step(&mut sh, &mut sim, &[ev(DK_RETURN, false)], [0, 0]);
        assert!(o.menu_sounds.is_empty(), "ClearKeys dropped the held R");
        step(&mut sh, &mut sim, &[ev(up, false)], [0, 0]);
        assert_eq!(sh.settings_menu().selection(), sel);
        assert_eq!(sh.settings().lives, 15, "'r' is filtered");
    }

    fn open_weapon_options(sh: &mut Shell, sim: &mut SimState) {
        enter_on(sh, sim, SI_WEAPON_OPTIONS);
        assert_eq!((sh.top_char(), sh.phase()), ('O', Phase::Menu));
    }

    #[test]
    fn weapon_options_lists_the_40_weapons_in_weap_order() {
        let (mut sh, mut sim) = settings_focus();
        let w3 = sh.world.tc.weap_order[3];
        sh.settings_mut().weap_table[w3] = 2;
        open_weapon_options(&mut sh, &mut sim);
        let m = weapon_state(&sh).menu();
        assert_eq!((m.x, m.y, m.height, m.value_offset_x), (179, 28, 14, 89));
        let rows: Vec<(&str, i32)> = m.items.iter().map(|i| (i.string.as_str(), i.id)).collect();
        let tc = hooks();
        assert_eq!(rows.len(), 40);
        for (i, (name, id)) in rows.iter().enumerate() {
            assert_eq!((*name, *id), (tc.weapon_names[i].as_str(), i as i32));
        }
        assert_eq!(rows[0].0, "BAZOOKA");
        assert_eq!(m.items[3].value, "Banned");
        assert!(
            m.items
                .iter()
                .enumerate()
                .all(|(i, it)| i == 3 || it.value == "Menu")
        );
        assert_eq!(m.selection(), 0);
        // The draw: DrawBasicMenu (main disabled, its selection shown), the headers, the menu.
        let w = &sh.world;
        let font = &sh.boot_scene.font;
        let mut want = w.frozen.clone();
        w.main_menu.draw(
            &crate::menu::PlainModel,
            &mut want,
            &w.pal32,
            font,
            true,
            -1,
            true,
        );
        font.draw_framed_text(&mut want, &w.pal32, "Weapon", 179, 20, 50);
        font.draw_framed_text(&mut want, &w.pal32, "Availability", 249, 20, 50);
        m.draw(
            &crate::menu::PlainModel,
            &mut want,
            &w.pal32,
            font,
            false,
            -1,
            false,
        );
        assert_eq!(*sh.surface(), want);
    }

    #[test]
    fn weapon_options_left_right_are_once_keys_with_pgdn_and_the_search() {
        let (mut sh, mut sim) = settings_focus();
        let h = hooks().hooks;
        let order = hooks().weap_order;
        open_weapon_options(&mut sh, &mut sim);
        let o = step(&mut sh, &mut sim, &[ev(DK_RIGHT, true)], [0, 0]);
        assert_eq!(o.menu_sounds, vec![h.move_up]);
        idle(&mut sh, &mut sim, 12);
        assert_eq!(sh.settings().weap_table[order[0]], 1, "held: once (Bonus)");
        step(&mut sh, &mut sim, &[ev(DK_RIGHT, false)], [0, 0]);
        tap(&mut sh, &mut sim, DK_RIGHT);
        tap(&mut sh, &mut sim, DK_RIGHT);
        assert_eq!(
            sh.settings().weap_table[order[0]],
            0,
            "Banned -> Menu: cyclic"
        );
        let o = tap(&mut sh, &mut sim, DK_LEFT);
        assert_eq!(
            (sh.settings().weap_table[order[0]], o.menu_sounds),
            (2, vec![h.move_down])
        );
        assert_eq!(weapon_state(&sh).menu().items[0].value, "Banned");
        // PgDn.
        let mut want = weapon_state(&sh).menu().clone();
        want.movement_page(1);
        let o = tap(&mut sh, &mut sim, DK_PGDN);
        assert_eq!(o.menu_sounds, vec![h.move_up]);
        assert_eq!(weapon_state(&sh).menu().selection(), want.selection());
        // Type-to-search `LA` (unbound letters): the first row starting with it.
        let key = |dos: u32, c: u8| KeyEvent {
            typed: TypedKey::Sym(u32::from(c)),
            ..ev(dos, true)
        };
        step(&mut sh, &mut sim, &[key(38, b'l'), key(30, b'a')], [0, 0]);
        let names = hooks().weapon_names;
        let want = names.iter().position(|n| n.starts_with("LA")).unwrap();
        assert_eq!(weapon_state(&sh).menu().selection(), want as i32);
        assert_eq!(names[want], "LARPA");
        step(&mut sh, &mut sim, &[ev(38, false), ev(30, false)], [0, 0]);
        // Esc closes (weapons are still in the menu): the main menu is drawn the same frame.
        let o = tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(
            (o.upd, o.phase, sh.top_char()),
            (Phase::Menu, Phase::Menu, 'M')
        );
        assert_eq!(sh.cur_menu(), CurMenu::Settings, "the focus stays");
        assert!(o.present.is_some());
    }

    #[test]
    fn weapon_options_refuses_to_close_with_every_weapon_out_of_the_menu() {
        let (mut sh, mut sim) = settings_focus();
        let order = hooks().weap_order;
        sh.settings_mut().weap_table = [1; 40];
        sh.settings_mut().weap_table[order[0]] = 0;
        open_weapon_options(&mut sh, &mut sim);
        tap(&mut sh, &mut sim, DK_RIGHT); // the last Menu weapon -> Bonus
        assert!(!sh.settings().weap_table.contains(&0));
        let stale = sh.surface().clone();
        let o = step(&mut sh, &mut sim, &[ev(DK_ESCAPE, true)], [0, 0]);
        assert_eq!((sh.top_char(), o.menu_sounds), ('B', vec![]));
        let b = top_box(&sh);
        assert_eq!(
            (b.text.as_str(), b.x, b.y, b.clear_screen, &b.purpose),
            (
                hooks().no_weaps.as_str(),
                223,
                68,
                false,
                &InfoPurpose::NoWeapons
            )
        );
        assert_eq!(sh.top_refusal(), None, "a C++ box is not a refusal");
        let (w, hh) = sh.boot_scene.font.get_dims_h(&b.text);
        let (cx, cy) = (223 - w / 2 - 2, 68 - hh / 2 - 2);
        for y in 0..200 {
            for x in 0..320 {
                if !((cx..cx + w + 4).contains(&x) && (cy..cy + hh + 1).contains(&y)) {
                    assert_eq!(
                        sh.surface().get_pixel(x, y),
                        stale.get_pixel(x, y),
                        "({x},{y})"
                    );
                }
            }
        }
        step(&mut sh, &mut sim, &[ev(DK_ESCAPE, false)], [0, 0]);
        assert_eq!(sh.top_char(), 'B', "a key-up never dismisses");
        // Any key: the box pops and WEAPON OPTIONS is back; ClearKeys dropped that key.
        tap(&mut sh, &mut sim, 57);
        assert_eq!(sh.top_char(), 'O');
        tap(&mut sh, &mut sim, DK_LEFT); // Bonus -> Menu
        tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(sh.top_char(), 'M');
    }

    /// Put `settings` on the shell and tap `key` on NEW GAME (or F1); returns that frame.
    fn refused_new_game(edit: impl FnOnce(&mut Settings), key: u32) -> (Shell, SimState, FrameOut) {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        edit(sh.settings_mut());
        let o = tap(&mut sh, &mut sim, key);
        (sh, sim, o)
    }

    #[test]
    fn new_game_and_f1_refuse_holdazone_with_a_box_and_the_menu_stays() {
        let select = hooks().hooks.select;
        for (key, sounds) in [(DK_RETURN, vec![select]), (DK_F1, vec![])] {
            let (mut sh, mut sim, o) =
                refused_new_game(|s| s.game_mode = scenario::settings::GM_HOLDAZONE, key);
            assert_eq!(
                o.menu_sounds, sounds,
                "the Enter's own MenuSelect only; F1 none"
            );
            assert_eq!((sh.top_char(), sh.menu_fading()), ('B', false));
            assert_eq!(
                sh.top_refusal(),
                Some(&overlay::Refusal::Build(
                    scenario::build::BuildError::HoldazoneUnsupported
                ))
            );
            let b = top_box(&sh);
            assert_eq!(
                (b.text.as_str(), b.x, b.y, b.clear_screen),
                ("HOLDAZONE IS NOT\0SUPPORTED YET", 160, 100, false)
            );
            assert_eq!(
                b.purpose,
                InfoPurpose::Refused(overlay::Refusal::Build(
                    scenario::build::BuildError::HoldazoneUnsupported
                ))
            );
            assert!(
                idle(&mut sh, &mut sim, 40)
                    .iter()
                    .all(|o| o.routed.is_none()),
                "never routed"
            );
            tap(&mut sh, &mut sim, 57);
            assert_eq!((sh.top_char(), sh.main_selection()), ('M', MA_NEW_GAME));
            // A playable mode starts.
            sh.settings_mut().game_mode = scenario::settings::GM_KILL_EM_ALL;
            let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(
                outs.last().unwrap().routed,
                Some(Route::NewGame { seed: 21 })
            );
        }
    }

    #[test]
    fn new_game_refuses_zero_weapons_and_unequal_healths() {
        let (sh, _, _) = refused_new_game(|s| s.weap_table = [2; 40], DK_RETURN);
        assert_eq!(
            (top_box(&sh).text.as_str(), &top_box(&sh).purpose),
            (
                hooks().no_weaps.as_str(),
                &InfoPurpose::Refused(overlay::Refusal::Weapsel(
                    sim::weapsel::WeapselError::NoWeaponsEnabled
                ))
            )
        );
        let (sh, _, _) = refused_new_game(|s| s.worm_settings[1].health = 50, DK_RETURN);
        assert_eq!(top_box(&sh).text, "BOTH PLAYERS NEED\0THE SAME HEALTH");
        let (sh, _, _) = refused_new_game(|s| s.blood_particle_max = 0, DK_RETURN);
        assert_eq!(top_box(&sh).text, "THIS SETUP CANNOT\0BE PLAYED YET");
    }

    #[test]
    fn resume_refuses_holdazone_only_while_the_match_is_attached() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        to_menu(&mut sh, &mut sim);
        // GAME MODE through the menu: Enter twice (Kill'em All -> Game of Tag -> Holdazone).
        tap(&mut sh, &mut sim, DK_F7);
        enter_on(&mut sh, &mut sim, SI_GAME_MODE);
        enter_on(&mut sh, &mut sim, SI_GAME_MODE);
        assert_eq!(sh.settings().game_mode, scenario::settings::GM_HOLDAZONE);
        let cycles = sim.cycles;
        tap(&mut sh, &mut sim, DK_F1);
        assert_eq!(sh.top_char(), 'B');
        assert_eq!(top_box(&sh).text, "HOLDAZONE IS NOT\0SUPPORTED YET");
        tap(&mut sh, &mut sim, 57);
        assert_eq!(sim.cycles, cycles, "the match never ticked");
        // Detached, the paused match keeps its own settings: RESUME goes through.
        sh.current.as_mut().unwrap().detach_for_test();
        let outs = until_routed(&mut sh, &mut sim, DK_F1);
        assert_eq!(outs.last().unwrap().routed, Some(Route::Resume));
        assert_eq!(sim.game_mode, scenario::settings::GM_KILL_EM_ALL);
    }

    /// Settings whose live-read fields (fact 16) all differ from the defaults'.
    fn edited(s: &mut Settings) {
        s.max_bonuses = 0;
        s.weap_table[7] = 1;
        s.game_mode = scenario::settings::GM_GAME_OF_TAG;
        s.time_to_lose = 120;
        s.blood = 300;
        s.loading_time = 50;
        s.load_change = false;
        s.shadow = false;
        s.map = false;
        s.names_on_bonuses = true;
        s.lives = 3;
    }

    fn live_fields(sim: &SimState) -> (i32, Vec<i32>, u32, i32, i32, i32, bool, bool) {
        (
            sim.settings_max_bonuses,
            sim.weap_table.clone(),
            sim.game_mode,
            sim.time_to_lose,
            sim.blood,
            sim.settings_loading_time,
            sim.load_change,
            sim.shadow,
        )
    }

    #[test]
    fn resume_hands_the_menus_settings_to_an_attached_match() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        to_menu(&mut sh, &mut sim);
        let before = live_fields(&sim);
        edited(sh.settings_mut());
        assert_eq!(live_fields(&sim), before, "the menu never touches the sim");
        until_routed(&mut sh, &mut sim, DK_F1);
        let st = sh.settings();
        let want = (
            st.max_bonuses,
            st.weap_table.iter().map(|&v| v as i32).collect(),
            st.game_mode,
            st.time_to_lose,
            st.blood,
            st.loading_time,
            st.load_change,
            st.shadow,
        );
        assert_eq!(live_fields(&sim), want, "apply_live_settings");
        assert_ne!(live_fields(&sim), before);
        let m = sh.current().unwrap();
        assert_eq!(m.settings(), sh.settings(), "the match's own copy");
        assert!(!m.hud().map, "the HUD's map");
    }

    #[test]
    fn resume_leaves_a_detached_match_and_the_counterfactual_switch_alone() {
        for detach in [true, false] {
            let (mut sh, mut sim, _) = boot();
            start_match(&mut sh, &mut sim);
            to_menu(&mut sh, &mut sim);
            if detach {
                sh.current.as_mut().unwrap().detach_for_test();
            } else {
                sh.debug_mut().resume_sync = false;
            }
            let before = live_fields(&sim);
            let settings = sh.current().unwrap().settings().clone();
            edited(sh.settings_mut());
            until_routed(&mut sh, &mut sim, DK_F1);
            assert_eq!(live_fields(&sim), before, "detach {detach}");
            assert_eq!(*sh.current().unwrap().settings(), settings);
        }
    }

    #[test]
    fn resume_during_selection_hands_it_the_weapon_table() {
        for sync in [true, false] {
            let (mut sh, mut sim, _) = boot();
            sh.debug_mut().resume_sync = sync;
            idle(&mut sh, &mut sim, 40);
            until_routed(&mut sh, &mut sim, DK_RETURN);
            step(&mut sh, &mut sim, &[], [2, 0]); // P1 Down: the cursor onto weapon slot 1
            step(&mut sh, &mut sim, &[], [0, 0]);
            to_menu(&mut sh, &mut sim);
            let order = hooks().weap_order;
            let pick = sh.settings().worm_settings[0].weapons[0] as usize; // 1-based
            let keep = (pick - 1 + 5) % 40; // not the next one
            let mut table = [2u32; 40];
            table[order[keep]] = 0;
            sh.settings_mut().weap_table = table;
            until_routed(&mut sh, &mut sim, DK_F1);
            assert_eq!(sh.phase(), Phase::Weapsel);
            step(&mut sh, &mut sim, &[], [8, 0]); // P1 Right
            let got = sim.worms[0].weapons[0].ty.unwrap() as usize;
            let kept = sim.weapons[order[keep]].id as usize;
            assert_eq!(
                got == kept,
                sync,
                "sync {sync}: the pick skips to the one Menu weapon"
            );
        }
    }

    #[test]
    fn a_selection_edits_the_menus_picks_in_place() {
        // weapsel.cpp:255-282 cycle `ws.weapons[j]` of the SHARED `gfx.settings` worm settings
        // (4½c finding 4): the menu (cfg16, the exit save) sees a pick the frame it changes, not
        // at the next NEW GAME (found by G2e-1 weapon_options, frame 376).
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        until_routed(&mut sh, &mut sim, DK_RETURN);
        let before = sh.settings().worm_settings[0].weapons;
        step(&mut sh, &mut sim, &[], [2, 0]); // P1 Down: the cursor onto weapon slot 1
        step(&mut sh, &mut sim, &[], [0, 0]);
        step(&mut sh, &mut sim, &[], [8, 0]); // P1 Right: slot 1's pick cycles
        let after = sh.settings().worm_settings[0].weapons;
        assert_ne!(after[0], before[0], "the pick cycled");
        assert_eq!(after[1..], before[1..]);
        let order = hooks().weap_order;
        assert_eq!(
            order[after[0] as usize - 1],
            sim.worms[0].weapons[0].ty.unwrap() as usize,
            "the menu's pick is the selection's"
        );
        sh.save_on_exit().unwrap();
        let saved = sh.store().read("Setups/liero.cfg").expect("saved");
        assert_eq!(
            String::from_utf8(saved).unwrap(),
            scenario::settings_toml::settings_to_toml(sh.settings()),
            "the exit save writes the running pick"
        );
    }

    #[test]
    fn the_exit_save_writes_the_settings_toml() {
        let (mut sh, _, _) = boot();
        edited(sh.settings_mut());
        sh.save_on_exit().unwrap();
        let bytes = sh.store().read("Setups/liero.cfg").expect("saved");
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            scenario::settings_toml::settings_to_toml(sh.settings())
        );
    }

    #[test]
    fn a_cpp_saved_oddity_boots_and_its_new_game_shows_the_refusal() {
        type Edit = fn(&mut Settings);
        let cases: [(Edit, &str); 4] = [
            (
                |s| s.game_mode = scenario::settings::GM_HOLDAZONE,
                "HOLDAZONE IS NOT\0SUPPORTED YET",
            ),
            (
                |s| s.worm_settings[0].health = 70,
                "BOTH PLAYERS NEED\0THE SAME HEALTH",
            ),
            (
                |s| s.blood_particle_max = 0,
                "THIS SETUP CANNOT\0BE PLAYED YET",
            ),
            (
                |s| s.worm_settings[1].weapons[2] = 41,
                "THIS SETUP CANNOT\0BE PLAYED YET",
            ),
        ];
        for (edit, want) in cases {
            let mut settings = Settings::default();
            edit(&mut settings);
            let (mut sh, mut sim, out) = Shell::boot(
                tc(),
                settings.clone(),
                Box::new(MemoryStore::new()),
                SeedSource::Fixed(5),
                0,
                StartOptions::default(),
            );
            assert_eq!((out.phase, sh.top_char()), (Phase::Menu, 'M'), "{want}");
            assert_eq!(*sh.settings(), settings, "the menu keeps the file's values");
            assert_eq!(
                sim.game_mode, settings.game_mode,
                "the HUD's mode is the real one"
            );
            idle(&mut sh, &mut sim, 40);
            tap(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(top_box(&sh).text, want);
        }
    }

    #[test]
    fn a_shell_match_takes_its_keys_as_edges() {
        // Held Change + one Right press steps one weapon, not one per tick (keys::KeyEdges).
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        for _ in 0..400 {
            if sim.worms[0].visible {
                break;
            }
            step(&mut sh, &mut sim, &[], [16, 0]);
            step(&mut sh, &mut sim, &[], [0, 0]);
        }
        assert!(sim.worms[0].visible);
        let before = sim.worms[0].current_weapon;
        for w in [32, 32, 32, 40, 40, 40, 40, 40, 40, 40, 32, 0] {
            step(&mut sh, &mut sim, &[], [w, 0]);
        }
        assert_eq!((sim.worms[0].current_weapon - before).rem_euclid(5), 1);
    }
}
