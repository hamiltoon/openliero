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

use std::path::{Path, PathBuf};

use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::frame::Scene;
use render::menu::menu_palette;
use scenario::SceneData;
use scenario::settings::Settings;
use scenario::storage::ConfigStore;
use sim::state::{ControlState, SimState};

use crate::keys::{DK_ESCAPE, KeyLatch, TypedKey};
use crate::menu::Menu;
use crate::text::UiTc;
use level_slot::{LevelSlot, SeedSource};
use main_menu::{MA_NEW_GAME, MA_QUIT, MA_RESUME_GAME, MainMenuState, MenuCtx, main_menu};
use overlay::{InfoPurpose, InputPurpose};
use playing::{Match, StartOptions};
use settings_menu::settings_menu;
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
        let mut push = None;
        let mut closing = None;
        let keep = match self.stack.top_mut().expect("non-empty") {
            Screen::MainMenu(s) => {
                let mut cx = MenuCtx {
                    w: &mut self.world,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    push: None,
                };
                let keep = s.update(&mut cx);
                push = cx.push.take();
                keep
            }
            Screen::Playing => {
                let m = self.current.as_mut().expect("Playing has a match");
                let (keep, ticked) = m.process(sim, input.sampled, &mut out.menu_sounds);
                out.sim_ticked = ticked;
                keep
            }
            // T4 ports `WeaponMenuState::Update`; nothing pushes the screen before it.
            Screen::WeaponOptions(_) => true,
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
                debug_assert!(push.is_none(), "a screen that pushes keeps running");
                self.route(sel, sim, input, &mut out);
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
        if let Some(mut screen) = push {
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
        out.phase = self.phase();
        out
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
    fn input_done(&mut self, purpose: InputPurpose, _accepted: bool, _buffer: &[u8]) {
        match purpose {
            // T4: `integerBehavior.cpp:56-76`, the value write-back (plan fact 13).
            InputPurpose::IntegerEntry(_) => {}
        }
    }

    /// `StateStack::Push`'s `Enter` (`state.hpp:50-54`) of a screen about to go on the stack.
    fn enter(&mut self, screen: &mut Screen, out: &mut FrameOut) {
        match screen {
            Screen::MainMenu(s) => {
                out.present = Some(Present::Black);
                let running = self.current.as_ref().is_some_and(Match::running);
                s.enter(&mut MenuCtx {
                    w: &mut self.world,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    push: None,
                });
            }
            Screen::Playing => unreachable!("only the router pushes Playing"),
            // T4 ports `WeaponMenuState::Enter`. `InputStringState::Enter` is
            // `SDL_StartTextInput` (the page's text field keys on `Phase::Text`, D9);
            // `InfoBoxState::Enter` is empty.
            Screen::WeaponOptions(_) | Screen::InputString(_) | Screen::InfoBox(_) => {}
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
                    // :1525-1530 + GamePlayState::Enter → Focus.
                    self.current
                        .as_mut()
                        .expect("RESUME is shown only while a match runs")
                        .focus(&input.sampled);
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
        for i in self.stack.draw_from()..self.stack.len() {
            match &self.stack.screens()[i] {
                Screen::MainMenu(s) => {
                    let mut sounds = Vec::new();
                    s.draw(&mut MenuCtx {
                        w: &mut self.world,
                        font: &self.boot_scene.font,
                        running,
                        sounds: &mut sounds,
                        push: None,
                    });
                }
                Screen::Playing => {
                    let m = self.current.as_mut().expect("Playing has a match");
                    self.world.pal32 = m.draw(
                        &mut self.world.surface,
                        &mut self.world.frozen,
                        sim,
                        self.world.menu_cycles,
                    );
                    self.world.fade = m.fade();
                }
                // T4 ports `WeaponMenuState::Draw`.
                Screen::WeaponOptions(_) => {}
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

    /// The config store (Step 4½e-1).
    pub fn store(&self) -> &dyn ConfigStore {
        &*self.store
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
            (14, 1),
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
        for dos in [DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9] {
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
                item_id: 0,
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
}
