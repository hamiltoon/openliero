//! Step 4½d — the C++ shell: `StateStack` (`state.hpp`), `MainMenuState`
//! (`mainMenuState.cpp`), the router and `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`). T3 adds the
//! two concrete menus; T6 moves the 4½c live modules here; T7 adds the stack, the screens and
//! `Shell`. T6 moved the 4½c live modules here from `game` (`new_game`, `selection`,
//! `match_flow`, `viewport_step`, `loadout`), unchanged; `game` re-exports them.
pub mod level_slot;
pub mod loadout;
pub mod main_menu;
pub mod match_flow;
pub mod new_game;
pub mod playing;
pub mod selection;
pub mod settings_menu;
pub mod stack;
pub mod viewport_step;

use std::path::{Path, PathBuf};

use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::frame::Scene;
use render::menu::menu_palette;
use scenario::SceneData;
use scenario::settings::Settings;
use sim::state::{ControlState, SimState};

use crate::keys::{DK_ESCAPE, KeyLatch, TypedKey};
use crate::menu::Menu;
use crate::text::UiTc;
use level_slot::{LevelSlot, SeedSource};
use main_menu::{MA_NEW_GAME, MA_QUIT, MA_RESUME_GAME, MainMenuState, MenuCtx, main_menu};
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

/// One frame's input: its events in order, the sim's sampled control words (the 4½c sampler plus
/// touch — C++ key events reach the worms at the frame boundary, the 4½c equivalence), a fresh
/// seed for `SeedSource::Fresh`, the type-to-search clock, and the Rust-only F5 restart (Q5).
#[derive(Clone, Copy, Debug)]
pub struct ShellInput<'a> {
    pub events: &'a [KeyEvent],
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
    Weapsel,
    Game,
    Quit,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Menu => "menu",
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

/// Everything `MainMenuState` touches — C++ `Gfx` members: the menus, `settings`,
/// `settings_node`'s name, `dos_keys`, the play renderer's `fade_value`, `bmp`, `pal32` and
/// `Origpal()`, `frozen_screen`, `menu_cycles`. No `SimState`.
pub struct MenuWorld {
    pub main_menu: Menu,
    pub settings_menu: Menu,
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
        seeds: SeedSource,
        fresh: u32,
        options: StartOptions,
    ) -> (Shell, SimState) {
        let tc = UiTc::load(tc_root);
        let boot_seed = seeds.boot(fresh);
        let level = LevelSlot::generate(tc_root, &settings, boot_seed);
        let boot = playing::boot_state(tc_root, &settings, &level.level, boot_seed);
        let world = MenuWorld {
            main_menu: main_menu(),
            settings_menu: settings_menu(),
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
    pub fn boot(
        tc_root: &Path,
        settings: Settings,
        seeds: SeedSource,
        fresh: u32,
        options: StartOptions,
    ) -> (Shell, SimState, FrameOut) {
        let (mut shell, state) = Shell::new(tc_root, settings, seeds, fresh, options);
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
        seeds: SeedSource,
        fresh: u32,
        options: StartOptions,
    ) -> (Shell, SimState, FrameOut) {
        let (mut shell, mut state) = Shell::new(tc_root, settings, seeds, fresh, options);
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
        // :1473-1485 — every event reaches ProcessEvent (dos_keys, key_buf); while Playing a
        // non-repeat key-down or any key-up also reaches LocalController::OnKey, whose only
        // non-worm effect is Esc (finding 9). Worm keys reach the sim as `sampled`.
        self.world.keys.begin_frame();
        let playing = matches!(self.stack.top(), Some(Screen::Playing));
        for ev in input.events {
            if ev.down {
                self.world.keys.key_down(ev.dos, ev.typed);
            } else {
                self.world.keys.key_up(ev.dos);
            }
            if playing && ev.dos == DK_ESCAPE && (!ev.down || !ev.repeat) {
                self.current.as_mut().expect("Playing has a match").esc();
            }
        }
        // :1487-1489, captured BEFORE Update: the menu state is on the stack only while it is the
        // top (finding 7), and C++ clears menuStatePtr_ at its dispatch.
        let (sel, fading) = match self.stack.top() {
            Some(Screen::MainMenu(s)) => (s.selection(), s.is_fading_out()),
            _ => (-1, false),
        };
        let running = self.current.as_ref().is_some_and(Match::running);
        let keep = match self.stack.top_mut().expect("non-empty") {
            Screen::MainMenu(s) => s.update(&mut MenuCtx {
                w: &mut self.world,
                font: &self.boot_scene.font,
                running,
                sounds: &mut out.menu_sounds,
            }),
            Screen::Playing => {
                let m = self.current.as_mut().expect("Playing has a match");
                let (keep, ticked) = m.process(sim, input.sampled, &mut out.menu_sounds);
                out.sim_ticked = ticked;
                keep
            }
        };
        match self.stack.finish_update(keep) {
            AfterUpdate::Popped { empty: true } => {
                self.route(sel, sim, input, &mut out);
                out.phase = self.phase();
                return out;
            }
            AfterUpdate::Replaced => unreachable!("no 4½d screen schedules a replacement"),
            AfterUpdate::Popped { empty: false } | AfterUpdate::Running => {}
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
            self.level = LevelSlot::generate(&self.tc_root, &self.world.settings, seed);
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
        let mut s = MainMenuState::new();
        out.present = Some(Present::Black);
        let running = self.current.as_ref().is_some_and(Match::running);
        s.enter(&mut MenuCtx {
            w: &mut self.world,
            font: &self.boot_scene.font,
            running,
            sounds: &mut out.menu_sounds,
        });
        self.stack.push(Screen::MainMenu(s));
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
            }
        }
    }

    pub fn phase(&self) -> Phase {
        match self.stack.top() {
            None => Phase::Quit,
            Some(Screen::MainMenu(_)) => Phase::Menu,
            Some(Screen::Playing) => {
                if self.current.as_ref().is_some_and(Match::in_selection) {
                    Phase::Weapsel
                } else {
                    Phase::Game
                }
            }
        }
    }

    /// `M` / `G` / `-` (the G2 golden's `<top>`).
    pub fn top_char(&self) -> char {
        match self.stack.top() {
            None => '-',
            Some(Screen::MainMenu(_)) => 'M',
            Some(Screen::Playing) => 'G',
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
        matches!(self.stack.top(), Some(Screen::MainMenu(s)) if s.is_fading_out())
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
        Shell::boot(tc(), Settings::default(), seeds, 0, StartOptions::default())
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
        let want = generate_level(tc(), sh.settings(), 21);
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
        let (sh, sim, out) =
            Shell::boot_playing(tc(), Settings::default(), SeedSource::Fixed(7), 0, opts);
        assert_eq!(
            (out.phase, out.routed, sh.top_char()),
            (Phase::Game, Some(Route::NewGame { seed: 7 }), 'G')
        );
        let want = generate_level(tc(), &Settings::default(), 7);
        assert_eq!(
            sim.level.material_id, want.material_id,
            "?seed=7: 4½c's first match"
        );
        let (_, _, sel) = Shell::boot_playing(
            tc(),
            Settings::default(),
            SeedSource::Fixed(7),
            0,
            StartOptions::default(),
        );
        assert_eq!(sel.phase, Phase::Weapsel, "?level= / ?seed= keep selection");
    }
}
