//! Step 4½d — the C++ shell: `StateStack` (`state.hpp`), `MainMenuState`
//! (`mainMenuState.cpp`), the router and `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`). T3 adds the
//! two concrete menus; T6 moves the 4½c live modules here; T7 adds the stack, the screens and
//! `Shell`. T6 moved the 4½c live modules here from `game` (`new_game`, `selection`,
//! `match_flow`, `viewport_step`, `loadout`), unchanged; `game` re-exports them.
//!
//! Step 4½e-1 (T3): the ordered `InputEvent` stream (keys and text), the settings menu's
//! sub-screens (`overlay`, `weapon_options`) on the stack, `cur_menu` in `MenuWorld`,
//! `level_path`, and the `ConfigStore` the shell owns.
//!
//! Step 4½e-2 (T3): the file tree and the two selectors (`files`; `Screen::LevelSelect`,
//! `Screen::SetupSelect`, tops `L`/`P`), their `Picked` continuation, and the store in
//! `MenuCtx`. T4: LEVEL, LOAD SETUP and SAVE SETUP AS… live — the Save-As chain (the reserved
//! box and its reopen), LOAD SETUP (fresh settings, the name, the detach), picks written back
//! only while the match is attached.
pub mod files;
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
use scenario::settings_toml::{settings_from_toml, settings_to_toml};
use scenario::storage::{self, ConfigStore};
use sim::state::{ControlState, SimState};

use crate::keys::{DK_ESCAPE, KeyLatch, TypedKey};
use crate::menu::Menu;
use crate::text::{UiTc, dos_display, dos_to_text};
use files::{Picked, SelectorView};
use level_slot::{LevelSlot, SeedSource};
use main_menu::{MA_NEW_GAME, MA_QUIT, MA_RESUME_GAME, MainMenuState, MenuCtx, main_menu};
use overlay::{InfoBoxState, InfoPurpose, InputPurpose, InputStringState, RefusalGate};
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
    /// Rust-only messages for the glue to log (Step 4½e-2): a SAVE SETUP AS… write error, a
    /// LOAD SETUP file that does not parse (plan D7, D8).
    pub notes: Vec<String>,
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
            notes: Vec::new(),
        }
    }
}

/// The keyboard a phone raises for the `InputStringState` on top (Step 4½e-2, plan D11): number
/// entry has a digit filter, SAVE SETUP AS…'s name box none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMode {
    Numeric,
    Text,
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
/// step it names. All are `true` by default, which is the C++ behaviour.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellDebug {
    /// RESUME's live-settings resync (`apply_live_settings` + `Match::resync`).
    pub resume_sync: bool,
    /// The three `DrawTextSmall` labels in a match's draw.
    pub small_labels: bool,
    /// LOAD SETUP's detach of the current match (Step 4½e-2, plan D17): `false` keeps it
    /// attached, so RESUME resyncs it to the loaded setup.
    pub load_detach: bool,
    /// The match's AI step (4½f-1 D8): `false` leaves a CPU worm's word as its keys made it
    /// (the G2f-1 negative control).
    pub ais: bool,
}

impl Default for ShellDebug {
    fn default() -> Self {
        ShellDebug {
            resume_sync: true,
            small_labels: true,
            load_detach: true,
            ais: true,
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

/// `MakeSaveAsState("Setups", ".cfg", initial, x, y, …)` (`mainMenuState.cpp:69-91`): the name
/// box, 30 bytes, no filter, no prefix, not centred.
pub(crate) fn save_as_box(initial: &[u8], x: i32, y: i32) -> Screen {
    Screen::InputString(InputStringState::new(
        initial,
        30,
        x,
        y,
        None,
        "",
        false,
        InputPurpose::SaveSetupAs { x, y },
    ))
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
        let mut picked = None;
        let keep = match self.stack.top_mut().expect("non-empty") {
            Screen::MainMenu(s) => {
                let mut cx = MenuCtx {
                    w: &mut self.world,
                    store: &*self.store,
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
                let (keep, ticked) =
                    m.process(sim, input.sampled, self.debug.ais, &mut out.menu_sounds);
                out.sim_ticked = ticked;
                keep
            }
            Screen::WeaponOptions(s) => {
                let mut cx = MenuCtx {
                    w: &mut self.world,
                    store: &*self.store,
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
            Screen::LevelSelect(s) => {
                let o = s.update(&mut MenuCtx {
                    w: &mut self.world,
                    store: &*self.store,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    pushes: Vec::new(),
                    now_ms: input.now_ms,
                    gate,
                });
                picked = o.picked;
                o.keep
            }
            Screen::SetupSelect(s) => {
                let o = s.update(&mut MenuCtx {
                    w: &mut self.world,
                    store: &*self.store,
                    font: &self.boot_scene.font,
                    running,
                    sounds: &mut out.menu_sounds,
                    pushes: Vec::new(),
                    now_ms: input.now_ms,
                    gate,
                });
                picked = o.picked;
                o.keep
            }
        };
        // An overlay's close runs inside its `Update`, before the stack's pop test
        // (`inputState.cpp:75-84`, `:186-198`): a continuation may schedule a replacement.
        if let Some(c) = closing {
            self.close(c, &mut out);
        }
        // A selector's `OnSelected` is the last thing its `Update` does (plan D4, fact 9).
        if let Some(p) = picked {
            self.apply_picked(p, &mut out);
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
    /// `ClearKeys`, the optional `Fill(bmp, 0)`, `on_dismiss` (only the reserved-name box has
    /// one).
    fn close(&mut self, c: Closing, out: &mut FrameOut) {
        match c {
            Closing::Input(purpose, accepted, buffer) => {
                let select = self.world.tc.hooks.select;
                if select >= 0 {
                    out.menu_sounds.push(select);
                }
                self.world.keys.clear();
                self.input_done(purpose, accepted, &buffer, out);
            }
            Closing::Info(purpose, clear_screen) => {
                self.world.keys.clear();
                if clear_screen {
                    self.world.surface.fill(0, &self.world.pal32);
                }
                match purpose {
                    InfoPurpose::NoWeapons | InfoPurpose::Refused(_) => {}
                    // `mainMenuState.cpp:81-84`: the name box again, on what was typed; the
                    // replacement wins over the box's pop (`state.hpp:101-105`, fact 15).
                    InfoPurpose::Reserved { typed, x, y } => {
                        self.stack.schedule_replace_top(save_as_box(&typed, x, y));
                    }
                }
            }
        }
    }

    /// An `InputStringState`'s callback (`callback_(accepted_, buffer_)`).
    fn input_done(
        &mut self,
        purpose: InputPurpose,
        accepted: bool,
        buffer: &[u8],
        out: &mut FrameOut,
    ) {
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
            InputPurpose::SaveSetupAs { x, y } => self.save_setup_as(accepted, buffer, x, y, out),
        }
    }

    /// `MakeSaveAsState`'s callback and SAVE SETUP AS…'s `on_complete` (`mainMenuState.cpp:
    /// 69-91`, `:295-306`; plan facts 12, 15, D7). An accepted non-empty name whose leaf the
    /// store refuses — a shipped or reserved name, or (Rust only) one it cannot place — schedules
    /// the black `NAME '<leaf>' IS RESERVED` box, with no sound and no completion. Otherwise
    /// the completion: a non-empty accepted name saves the settings to `Setups/<name>.cfg` and
    /// becomes the setup's name (a write error is a note and keeps the name); then, always,
    /// `MenuSelect` + `UpdateItems`.
    fn save_setup_as(&mut self, accepted: bool, buffer: &[u8], x: i32, y: i32, out: &mut FrameOut) {
        if accepted && !buffer.is_empty() {
            let name = dos_to_text(buffer);
            let leaf = format!("{name}.cfg");
            if self.store.shadows_system("Setups", &leaf) || !storage::placeable_leaf(&leaf) {
                let text = format!("NAME '{}.cfg' IS RESERVED", dos_display(buffer));
                self.stack
                    .schedule_replace_top(Screen::InfoBox(InfoBoxState::new(
                        &text,
                        160,
                        100,
                        true,
                        InfoPurpose::Reserved {
                            typed: buffer.to_vec(),
                            x,
                            y,
                        },
                    )));
                return;
            }
            let toml = settings_to_toml(&self.world.settings);
            match self.store.write(&format!("Setups/{leaf}"), toml.as_bytes()) {
                Ok(()) => self.world.setup_name = name,
                Err(e) => out.notes.push(format!("SAVE SETUP AS: Setups/{leaf}: {e}")),
            }
        }
        let w = &mut self.world;
        main_menu::play(&mut out.menu_sounds, w.tc.hooks.select);
        w.settings_menu.update_items(&mut SettingsModel {
            settings: &mut w.settings,
            tc: &w.tc,
            setup_name: &w.setup_name,
        });
    }

    /// A selector's `OnSelected` (plan D4), then `settings_menu.UpdateItems`.
    /// - `LevelSelectorState::OnSelected` (`fileSelectorState.cpp:95-103`): `[RANDOM]` sets
    ///   `random_level` and clears `level_file`; a file sets `level_file` to its `full_path`.
    /// - `OptionsSelectorState::OnSelected` (`:222-226` → `Gfx::LoadSettings`, `gfx.cpp:
    ///   1693-1697`; plan fact 13, D8, D17): a fresh `Settings` from the file and the setup's
    ///   name. The current match keeps the old settings object — it is detached (T0 P8). A file
    ///   that does not parse is a note and changes nothing (C++ swaps in a half-read object).
    fn apply_picked(&mut self, p: Picked, out: &mut FrameOut) {
        match p {
            Picked::Random => {
                self.world.settings.random_level = true;
                self.world.settings.level_file.clear();
            }
            Picked::Level(path) => {
                self.world.settings.random_level = false;
                self.world.settings.level_file = path;
            }
            Picked::Setup { rel, name } => {
                let parsed = self
                    .store
                    .read(&rel)
                    .ok_or_else(|| "cannot be read".to_string())
                    .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                    .and_then(|t| settings_from_toml(&t).map_err(|e| e.to_string()));
                match parsed {
                    Ok(s) => {
                        self.world.settings = s;
                        // 4½f-1 D3: a phone cannot drive a setup's human player 2.
                        if self.options.touch_only {
                            selection::touch_settings(&mut self.world.settings);
                        }
                        self.world.setup_name = name;
                        if self.debug.load_detach
                            && let Some(m) = self.current.as_mut()
                        {
                            m.detach();
                        }
                    }
                    Err(e) => out.notes.push(format!("LOAD SETUP: {rel}: {e}")),
                }
            }
        }
        let w = &mut self.world;
        w.settings_menu.update_items(&mut SettingsModel {
            settings: &mut w.settings,
            tc: &w.tc,
            setup_name: &w.setup_name,
        });
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
                    store: &*self.store,
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
            // `LevelSelectorState::Enter`, `OptionsSelectorState::Enter`.
            Screen::LevelSelect(s) => s.enter(&mut self.world, &*self.store),
            Screen::SetupSelect(s) => s.enter(&mut self.world, &*self.store),
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
                    // `Game::Focus` rewrites the renderer's worm ramps (4½f-1 D9), which the
                    // menus draw with too (T0 P6: the menu after the resumed play shows them).
                    m.focus(&input.sampled);
                    self.world.origpal = m.origpal().clone();
                    out.routed = Some(Route::Resume);
                }
                other => unreachable!("main-menu item {other} never selects (4½d §5, 4½e-1)"),
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
        // The old selection's picks are the menu's only while the match shares its settings: after
        // LOAD SETUP they belong to the old settings object (plan fact 17).
        if let Some(old) = self.current.take().filter(Match::attached) {
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
            match &mut self.stack.screens_mut()[i] {
                Screen::MainMenu(s) => {
                    let mut sounds = Vec::new();
                    s.draw(&mut MenuCtx {
                        w: &mut self.world,
                        store: &*self.store,
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
                Screen::LevelSelect(s) => {
                    s.draw(&mut self.world, &*self.store, &self.boot_scene.font);
                }
                Screen::SetupSelect(s) => {
                    s.draw(&mut self.world, &*self.store, &self.boot_scene.font);
                }
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
            Some(
                Screen::MainMenu(_)
                | Screen::WeaponOptions(_)
                | Screen::InfoBox(_)
                | Screen::LevelSelect(_)
                | Screen::SetupSelect(_),
            ) => Phase::Menu,
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

    /// `M` / `G` / `O` / `I` / `B` / `L` / `P` / `-` (the G2 golden's `<top>`; `O`, `I`, `B` are
    /// WEAPON OPTIONS, an `InputStringState` and an `InfoBoxState`, Step 4½e-1; `L`, `P` the
    /// level and options selectors, Step 4½e-2).
    pub fn top_char(&self) -> char {
        match self.stack.top() {
            None => '-',
            Some(Screen::MainMenu(_)) => 'M',
            Some(Screen::Playing) => 'G',
            Some(Screen::WeaponOptions(_)) => 'O',
            Some(Screen::InputString(_)) => 'I',
            Some(Screen::InfoBox(_)) => 'B',
            Some(Screen::LevelSelect(_)) => 'L',
            Some(Screen::SetupSelect(_)) => 'P',
        }
    }

    /// The selector on top, if one is (Step 4½e-2; the harness's ledger and the glue's hooks):
    /// its top, current folder and cursor.
    pub fn selector_view(&self) -> Option<SelectorView> {
        match self.stack.top() {
            Some(Screen::LevelSelect(s)) => Some(s.view()),
            Some(Screen::SetupSelect(s)) => Some(s.view()),
            _ => None,
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
                InfoPurpose::NoWeapons | InfoPurpose::Reserved { .. } => None,
            },
            _ => None,
        }
    }

    /// The keyboard the top `InputStringState` wants on a phone, if one is on top (plan D11).
    pub fn text_mode(&self) -> Option<TextMode> {
        match self.stack.top() {
            Some(Screen::InputString(s)) if s.filter.is_some() => Some(TextMode::Numeric),
            Some(Screen::InputString(_)) => Some(TextMode::Text),
            _ => None,
        }
    }

    /// `GetBasename(GetLeaf(settings_node.FullPath()))`: the setup SAVE SETUP AS… shows (Step
    /// 4½e-2).
    pub fn setup_name(&self) -> &str {
        &self.world.setup_name
    }

    /// Whether the level the router holds — the boot level, or the last NEW GAME's — was read
    /// from the settings' level file rather than generated (Step 4½e-2; `LevelSlot::from_file`).
    pub fn level_from_file(&self) -> bool {
        self.level.from_file
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

    /// For tests (to place the main menu's cursor).
    pub fn main_menu_mut(&mut self) -> &mut Menu {
        &mut self.world.main_menu
    }

    pub fn settings(&self) -> &Settings {
        &self.world.settings
    }

    /// For tests (MATCH SETUP edits the settings through the settings focus).
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
                // The e-2 pushes (Step 4½e-2 T4): the level selector, the options selector and
                // SAVE SETUP AS…'s name box.
                SI_LEVEL => ('L', s == before),
                LOAD_OPTIONS => ('P', s == before),
                SAVE_OPTIONS => ('I', s == before),
                // Sound only: blood (allow_entry = false) and a time.
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
    fn new_game_and_f1_refuse_a_follow_ai_player_with_a_box_and_the_menu_stays() {
        // 4½f Q2 (John): an "AI" (FollowAI, controller 2) player is refused at NEW GAME, the
        // Holdazone way (4½f-1 D1).
        let select = hooks().hooks.select;
        for worm in 0..2 {
            for (key, sounds) in [(DK_RETURN, vec![select]), (DK_F1, vec![])] {
                let (mut sh, mut sim, o) =
                    refused_new_game(|s| s.worm_settings[worm].controller = 2, key);
                let refusal =
                    overlay::Refusal::Build(scenario::build::BuildError::FollowAiUnsupported {
                        worm,
                    });
                assert_eq!(o.menu_sounds, sounds);
                assert_eq!((sh.top_char(), sh.menu_fading()), ('B', false));
                assert_eq!(sh.top_refusal(), Some(&refusal));
                let b = top_box(&sh);
                assert_eq!(
                    (b.text.as_str(), b.x, b.y, b.clear_screen),
                    ("AI PLAYERS ARE NOT\0SUPPORTED YET", 160, 100, false)
                );
                assert_eq!(b.purpose, InfoPurpose::Refused(refusal));
                assert!(
                    idle(&mut sh, &mut sim, 40)
                        .iter()
                        .all(|o| o.routed.is_none()),
                    "never routed"
                );
                assert!(sh.current().is_none(), "no match started");
                tap(&mut sh, &mut sim, 57);
                assert_eq!((sh.top_char(), sh.main_selection()), ('M', MA_NEW_GAME));
                // A CPU (or a human) starts.
                sh.settings_mut().worm_settings[worm].controller = 1;
                let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
                assert_eq!(
                    outs.last().unwrap().routed,
                    Some(Route::NewGame { seed: 21 })
                );
            }
        }
    }

    #[test]
    fn resume_never_refuses_a_follow_ai_player() {
        // CONTROLLER never reaches a running match (design finding 7): a paused, attached match
        // whose settings now say controller 2 resumes (4½f-1 D1).
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        to_menu(&mut sh, &mut sim);
        assert!(sh.current().unwrap().attached());
        sh.settings_mut().worm_settings[1].controller = 2;
        let outs = until_routed(&mut sh, &mut sim, DK_F1);
        assert_eq!(outs.last().unwrap().routed, Some(Route::Resume));
        assert_eq!(sh.top_refusal(), None);
        let cycles = sim.cycles;
        idle(&mut sh, &mut sim, 3);
        assert!(sim.cycles > cycles, "the match ticks again");
    }

    #[test]
    fn unequal_healths_boot_start_and_play_at_each_worms_own_max() {
        // 4½f-1 T3: no refusal, and the boot no longer copies player 1's health.
        let mut settings = Settings::default();
        settings.worm_settings[0].health = 70;
        settings.worm_settings[1].health = 250;
        let seeds = SeedSource::Scripted {
            boot: 11,
            matches: VecDeque::from([21]),
        };
        let (mut sh, mut sim, _) = Shell::boot(
            tc(),
            settings,
            Box::new(MemoryStore::new()),
            seeds,
            0,
            StartOptions::default(),
        );
        assert_eq!(
            (sim.worms[0].max_health, sim.worms[1].max_health),
            (70, 250),
            "the boot game"
        );
        start_match(&mut sh, &mut sim);
        assert_eq!(
            (sim.worms[0].max_health, sim.worms[1].max_health),
            (70, 250)
        );
        assert_eq!((sim.worms[0].health, sim.worms[1].health), (70, 250));
    }

    #[test]
    fn new_game_refuses_zero_weapons_and_blood_max_but_not_unequal_healths() {
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
        let (sh, _, _) = refused_new_game(|s| s.blood_particle_max = 0, DK_RETURN);
        assert_eq!(top_box(&sh).text, "THIS SETUP CANNOT\0BE PLAYED YET");
        let (mut sh, mut sim, o) = refused_new_game(|s| s.worm_settings[1].health = 50, DK_RETURN);
        assert_eq!(sh.top_refusal(), None, "4½f-1 T3: unequal healths play");
        let mut routed = o.routed;
        for _ in 0..300 {
            if routed.is_some() {
                break;
            }
            routed = step(&mut sh, &mut sim, &[], [0, 0]).routed;
        }
        assert_eq!(routed, Some(Route::NewGame { seed: 21 }));
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
        sh.current.as_mut().unwrap().detach();
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
        s.worm_settings[0].health = 30;
        s.worm_settings[1].health = 250;
    }

    /// Both worms' `max_health` (4½f-1: HEALTH is live, `apply_live_settings`).
    fn maxes(sim: &SimState) -> [i32; 2] {
        [sim.worms[0].max_health, sim.worms[1].max_health]
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
        assert_eq!(maxes(&sim), [100, 100]);
        edited(sh.settings_mut());
        assert_eq!(live_fields(&sim), before, "the menu never touches the sim");
        assert_eq!(maxes(&sim), [100, 100]);
        until_routed(&mut sh, &mut sim, DK_F1);
        assert_eq!(maxes(&sim), [30, 250], "RESUME brings both maxes (T0 P5)");
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
                sh.current.as_mut().unwrap().detach();
            } else {
                sh.debug_mut().resume_sync = false;
            }
            let before = live_fields(&sim);
            let settings = sh.current().unwrap().settings().clone();
            edited(sh.settings_mut());
            until_routed(&mut sh, &mut sim, DK_F1);
            assert_eq!(live_fields(&sim), before, "detach {detach}");
            assert_eq!(maxes(&sim), [100, 100], "detach {detach}");
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
                |s| s.worm_settings[1].controller = 2,
                "AI PLAYERS ARE NOT\0SUPPORTED YET",
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

    // Step 4½e-2 (T3 Step 7): the selectors through `Shell::frame`, pushed directly (T4 wires
    // the Enter arms), on T0 P1's fixture (root label `./user`).

    const WATER_PATH: &str = "./user/TC/openliero/Levels/water_stage.lev";

    fn boot_store(store: MemoryStore) -> (Shell, SimState) {
        let seeds = SeedSource::Scripted {
            boot: 11,
            matches: VecDeque::from([21, 22, 23]),
        };
        let (mut sh, mut sim, _) = Shell::boot(
            tc(),
            Settings::default(),
            Box::new(store),
            seeds,
            0,
            StartOptions::default(),
        );
        idle(&mut sh, &mut sim, 40);
        sh.world.cur_menu = CurMenu::Settings;
        sh.world.settings_menu.move_to_id(settings_menu::SI_LEVEL);
        step(&mut sh, &mut sim, &[], [0, 0]);
        (sh, sim)
    }

    /// A push from the settings menu's Enter (`Push` runs `Enter`, then the frame draws it).
    fn push_selector(sh: &mut Shell, sim: &mut SimState, mut screen: Screen) -> FrameOut {
        let mut out = FrameOut::new(Phase::Menu);
        sh.enter(&mut screen, &mut out);
        sh.stack.push(screen);
        step(sh, sim, &[], [0, 0])
    }

    fn view(sh: &Shell) -> (char, String, i32) {
        let v = sh.selector_view().expect("a selector on top");
        (v.top, v.folder, v.selection)
    }

    fn level_value(sh: &Shell) -> (String, bool, String, bool, bool) {
        let m = sh.settings_menu();
        let item = |id| m.item_from_id(id).unwrap();
        let lv = item(settings_menu::SI_LEVEL);
        (
            lv.value.clone(),
            lv.has_value,
            item(settings_menu::SI_REGENERATE_LEVEL).string.clone(),
            item(settings_menu::SI_RANDOM_MAP_WIDTH).visible,
            item(settings_menu::SI_RANDOM_MAP_HEIGHT).visible,
        )
    }

    #[test]
    fn the_level_selector_walks_t0s_p1_path_frame_by_frame() {
        use crate::keys::DK_LEFT;
        use crate::keys::DK_RIGHT;
        let (mut sh, mut sim) = boot_store(files::tests::p1_store());
        let h = hooks().hooks;
        let o = push_selector(
            &mut sh,
            &mut sim,
            Screen::LevelSelect(files::LevelSelectorState::new()),
        );
        assert_eq!((sh.top_char(), o.phase), ('L', Phase::Menu));
        assert_eq!(view(&sh), ('L', "./user".into(), 0));
        assert_eq!(sh.selector_view().unwrap().top, 'L');
        assert!(
            sh.main_menu_state().is_some(),
            "(sel, fading) still come from the buried main menu"
        );
        let expect = |sh: &mut Shell, sim: &mut SimState, k, sounds: &[i32], want: (&str, i32)| {
            let o = tap(sh, sim, k);
            assert_eq!(o.menu_sounds, sounds, "key {k}");
            assert_eq!(
                (o.upd, o.phase, sh.top_char()),
                (Phase::Menu, Phase::Menu, 'L')
            );
            assert_eq!(view(sh), ('L', want.0.to_string(), want.1), "key {k}");
        };
        expect(&mut sh, &mut sim, DK_DOWN, &[h.move_up], ("./user", 1));
        expect(&mut sh, &mut sim, DK_RIGHT, &[], ("./user/Profiles", 0));
        expect(&mut sh, &mut sim, DK_LEFT, &[], ("./user", 1));
        for row in 2..=5 {
            expect(&mut sh, &mut sim, DK_DOWN, &[h.move_up], ("./user", row));
        }
        expect(&mut sh, &mut sim, DK_RIGHT, &[], ("./user/TC", 0));
        expect(&mut sh, &mut sim, DK_RIGHT, &[], ("./user/TC/openliero", 0));
        let levels = "./user/TC/openliero/Levels";
        expect(&mut sh, &mut sim, DK_RIGHT, &[], (levels, 0));
        for row in 1..=8 {
            expect(&mut sh, &mut sim, DK_DOWN, &[h.move_up], (levels, row));
        }
        expect(&mut sh, &mut sim, DK_PGUP, &[h.move_down], (levels, 1));
        expect(&mut sh, &mut sim, DK_LEFT, &[], ("./user/TC/openliero", 0));
        expect(&mut sh, &mut sim, DK_RIGHT, &[], (levels, 1));
        let preview = |b: &Bitmap| -> Vec<u32> {
            (162..198)
                .flat_map(|y| (134..186).map(move |x| (x, y)))
                .map(|(x, y)| b.get_pixel(x, y))
                .collect()
        };
        let last = preview(sh.frozen());
        assert_eq!(preview(sh.surface()), last, "alpha's preview, shown late");
        let o = tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(
            (o.upd, sh.top_char(), o.menu_sounds.len()),
            (Phase::Menu, 'M', 0),
            "Esc leaves with no sound"
        );
        assert_eq!(sh.selector_view(), None);
        assert_eq!(
            preview(sh.surface()),
            last,
            "the preview persists into the main menu"
        );
        assert!(sh.settings().random_level, "Esc picks nothing");
    }

    #[test]
    fn picking_a_level_or_random_updates_the_settings_menu() {
        let (mut sh, mut sim) = boot_store(files::tests::p1_store());
        let select = hooks().hooks.select;
        assert_eq!(
            level_value(&sh),
            ("Random".into(), true, "REGENERATE LEVEL".into(), true, true)
        );
        sh.world.settings.level_file = WATER_PATH.into();
        push_selector(
            &mut sh,
            &mut sim,
            Screen::LevelSelect(files::LevelSelectorState::new()),
        );
        assert_eq!(
            view(&sh),
            ('L', "./user/TC/openliero/Levels".into(), 7),
            "restore (select runs even with random_level set)"
        );
        let o = tap(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(
            (o.upd, o.phase, sh.top_char(), o.menu_sounds),
            (Phase::Menu, Phase::Menu, 'M', vec![select]),
            "one MenuSelect; the state pops that frame (T0 P2)"
        );
        assert_eq!(
            (
                sh.settings().random_level,
                sh.settings().level_file.as_str()
            ),
            (false, WATER_PATH)
        );
        assert_eq!(
            level_value(&sh),
            (
                "\"water_stage\"".into(),
                true,
                "RELOAD LEVEL".into(),
                false,
                false
            )
        );
        push_selector(
            &mut sh,
            &mut sim,
            Screen::LevelSelect(files::LevelSelectorState::new()),
        );
        assert_eq!(view(&sh).1, "./user/TC/openliero/Levels");
        for _ in 0..3 {
            tap(&mut sh, &mut sim, crate::keys::DK_LEFT);
        }
        assert_eq!(view(&sh), ('L', "./user".into(), 5), "the root, on TC");
        tap(&mut sh, &mut sim, DK_PGUP);
        assert_eq!(view(&sh).2, 0, "[RANDOM]");
        let o = tap(&mut sh, &mut sim, DK_RETURN);
        assert_eq!((sh.top_char(), o.menu_sounds), ('M', vec![select]));
        assert_eq!(
            (
                sh.settings().random_level,
                sh.settings().level_file.as_str()
            ),
            (true, "")
        );
        assert_eq!(
            level_value(&sh),
            ("Random".into(), true, "REGENERATE LEVEL".into(), true, true)
        );
    }

    #[test]
    fn the_setup_selector_is_a_menu_phase_screen_on_top_p() {
        let store = files::tests::install();
        let (mut sh, mut sim) = boot_store(store);
        let o = push_selector(
            &mut sh,
            &mut sim,
            Screen::SetupSelect(files::SetupSelectorState::new()),
        );
        assert_eq!(
            (sh.top_char(), sh.phase(), o.phase),
            ('P', Phase::Menu, Phase::Menu)
        );
        assert_eq!(view(&sh), ('P', "./user/Setups".into(), 0));
        tap(&mut sh, &mut sim, crate::keys::DK_LEFT);
        assert_eq!(view(&sh), ('P', "./user".into(), 2));
        let o = tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!((sh.top_char(), o.menu_sounds.len()), ('M', 0));
    }

    // Step 4½e-2 (T4 Step 6): LEVEL, SAVE SETUP AS… and LOAD SETUP live, through their Enter
    // arms, on the `Fs::install()` system layer (root label `./user`).

    fn data_file(rel: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../data")
                .join(rel),
        )
        .unwrap()
    }

    const WATER_REL: &str = "TC/openliero/Levels/water_stage.lev";

    /// Boot on `store` with the menu faded in (main focus).
    fn boot_on(store: MemoryStore) -> (Shell, SimState) {
        let seeds = SeedSource::Scripted {
            boot: 11,
            matches: VecDeque::from([21, 22, 23]),
        };
        let (mut sh, mut sim, _) = Shell::boot(
            tc(),
            Settings::default(),
            Box::new(store),
            seeds,
            0,
            StartOptions::default(),
        );
        idle(&mut sh, &mut sim, 40);
        (sh, sim)
    }

    fn taps(sh: &mut Shell, sim: &mut SimState, dos: u32, n: usize) {
        for _ in 0..n {
            tap(sh, sim, dos);
        }
    }

    /// From the root of a fresh level selector on the install tree to `water_stage` (row 4 of
    /// Levels) and Return.
    fn pick_water(sh: &mut Shell, sim: &mut SimState) -> FrameOut {
        assert_eq!(view(sh), ('L', "./user".into(), 0), "the root, on [RANDOM]");
        taps(sh, sim, DK_DOWN, 4);
        taps(sh, sim, DK_RIGHT, 3);
        taps(sh, sim, DK_DOWN, 4);
        assert_eq!(view(sh), ('L', "./user/TC/openliero/Levels".into(), 4));
        tap(sh, sim, DK_RETURN)
    }

    #[test]
    fn level_picks_a_file_that_new_game_plays_from_either_layer() {
        // `both`: the gated both-layers layout (plan D1.1); otherwise the level is in the system
        // layer only, where C++ plays random and Rust plays the file (Q4, D1.2).
        let select = hooks().hooks.select;
        for both in [true, false] {
            let store = files::tests::install();
            if both {
                store.write(WATER_REL, &data_file(WATER_REL)).unwrap();
            }
            let (mut sh, mut sim) = boot_store(store);
            let o = tap(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(
                (o.menu_sounds, o.phase, sh.top_char()),
                (vec![select], Phase::Menu, 'L'),
                "LEVEL: MenuSelect + push"
            );
            let o = pick_water(&mut sh, &mut sim);
            assert_eq!((o.menu_sounds, sh.top_char()), (vec![select], 'M'));
            assert_eq!(sh.settings().level_file, WATER_PATH);
            assert_eq!(level_value(&sh).0, "\"water_stage\"");
            // Reopen: the cursor is restored.
            tap(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(view(&sh), ('L', "./user/TC/openliero/Levels".into(), 4));
            tap(&mut sh, &mut sim, DK_ESCAPE);
            tap(&mut sh, &mut sim, DK_ESCAPE);
            assert_eq!((sh.top_char(), sh.cur_menu()), ('M', CurMenu::Main));
            let outs = until_routed(&mut sh, &mut sim, DK_F1);
            assert_eq!(
                outs.last().unwrap().routed,
                Some(Route::NewGame { seed: 21 })
            );
            assert!(sh.level_from_file(), "both {both}: the file is played");
            let file = level_path::read_level(sh.store(), tc(), WATER_PATH, true)
                .expect("water_stage is accepted");
            let want = generate_level(tc(), sh.settings(), Some(file), 21);
            assert_eq!(sim.level.material_id, want.material_id, "both {both}");
            let random = generate_level(tc(), &Settings::default(), None, 21);
            assert_ne!(want.material_id, random.material_id);
        }
    }

    #[test]
    fn a_random_pick_after_a_file_level_generates_at_new_game() {
        let (mut sh, mut sim) = boot_store(files::tests::install());
        tap(&mut sh, &mut sim, DK_RETURN);
        pick_water(&mut sh, &mut sim);
        tap(&mut sh, &mut sim, DK_ESCAPE);
        until_routed(&mut sh, &mut sim, DK_F1);
        assert!(sh.level_from_file());
        to_menu(&mut sh, &mut sim); // from the selection: the pause menu
        tap(&mut sh, &mut sim, DK_F7);
        enter_on(&mut sh, &mut sim, SI_LEVEL);
        assert_eq!(view(&sh), ('L', "./user/TC/openliero/Levels".into(), 4));
        taps(&mut sh, &mut sim, DK_LEFT, 3);
        assert_eq!(view(&sh), ('L', "./user".into(), 4), "the root, on TC");
        tap(&mut sh, &mut sim, DK_PGUP);
        tap(&mut sh, &mut sim, DK_RETURN);
        assert!(sh.settings().random_level);
        tap(&mut sh, &mut sim, DK_ESCAPE);
        sh.main_menu_mut().move_to_id(MA_NEW_GAME);
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(
            outs.last().unwrap().routed,
            Some(Route::NewGame { seed: 22 })
        );
        assert!(!sh.level_from_file());
        let want = generate_level(tc(), sh.settings(), None, 22);
        assert_eq!(sim.level.material_id, want.material_id, "generated");
    }

    fn input_state(sh: &Shell) -> &overlay::InputStringState {
        match sh.stack.top() {
            Some(Screen::InputString(s)) => s,
            _ => panic!("an entry is on top, not {}", sh.top_char()),
        }
    }

    /// Backspace ×`n` (OS repeats), then `t` one char per event, then Return; the Return frame.
    fn retype(sh: &mut Shell, sim: &mut SimState, n: usize, t: &str) -> FrameOut {
        for _ in 0..n {
            let rep = KeyEvent {
                repeat: true,
                ..ev(DK_BACKSPACE, true)
            };
            step(sh, sim, &[rep], [0, 0]);
        }
        type_str(sh, sim, t);
        tap(sh, sim, DK_RETURN)
    }

    #[test]
    fn save_setup_as_refuses_reserved_names_reopens_on_them_and_saves_the_rest() {
        let select = hooks().hooks.select;
        let (mut sh, mut sim) = boot_store(files::tests::install());
        let o = enter_on(&mut sh, &mut sim, SAVE_OPTIONS);
        let m = sh.settings_menu();
        let (x, y) = m
            .item_position(m.index_from_id(SAVE_OPTIONS) as usize)
            .unwrap();
        assert_eq!(
            (o.menu_sounds, o.phase, sh.text_mode()),
            (vec![select], Phase::Text, Some(TextMode::Text))
        );
        let e = input_state(&sh);
        assert_eq!(
            (e.buffer.as_slice(), e.max_len, e.x, e.y, e.filter.is_none()),
            (&b"liero"[..], 30, 280, y, true),
            "the name box at 178 + 100 + 2"
        );
        assert_eq!(
            (x, e.purpose.clone()),
            (178, InputPurpose::SaveSetupAs { x: 280, y })
        );
        // A bare Return: the reserved box replaces the entry and is presented on the Return
        // frame (T0 P5): black, through the exepal.
        let o = step(&mut sh, &mut sim, &[ev(DK_RETURN, true)], [0, 0]);
        let exe = render::palette::pack_pal32(&hooks().exepal);
        assert_eq!((sh.top_char(), *sh.pal32()), ('B', exe));
        assert_eq!(o.present, Some(Present::Frame { fade: 32 }));
        assert_eq!(sh.surface().get_pixel(0, 0), exe[0]);
        step(&mut sh, &mut sim, &[ev(DK_RETURN, false)], [0, 0]);
        let b = top_box(&sh);
        assert_eq!(
            (
                o.menu_sounds,
                o.upd,
                o.phase,
                b.text.as_str(),
                b.clear_screen
            ),
            (
                vec![select],
                Phase::Text,
                Phase::Menu,
                "NAME 'liero.cfg' IS RESERVED",
                true
            ),
            "one MenuSelect: the entry's own; no completion"
        );
        assert_eq!((b.x, b.y, sh.text_mode()), (160, 100, None));
        assert_eq!(sh.top_refusal(), None);
        // Any key: the entry again, on what was typed, on the dismissing frame.
        let o = step(&mut sh, &mut sim, &[ev(57, true)], [0, 0]);
        assert_eq!(
            (o.menu_sounds.len(), o.upd, o.phase, sh.top_char()),
            (0, Phase::Menu, Phase::Text, 'I')
        );
        assert_eq!(sh.text_mode(), Some(TextMode::Text));
        step(&mut sh, &mut sim, &[ev(57, false)], [0, 0]);
        assert_eq!(input_state(&sh).buffer, b"liero");
        assert_eq!(
            input_state(&sh).purpose,
            InputPurpose::SaveSetupAs { x: 280, y }
        );
        // `mine`: saved; two MenuSelects; the value reads `mine`.
        let o = retype(&mut sh, &mut sim, 5, "mine");
        assert_eq!((o.menu_sounds, sh.top_char()), (vec![select, select], 'M'));
        let store = sh.store();
        assert_eq!(
            store.read("Setups/mine.cfg"),
            Some(scenario::settings_toml::settings_to_toml(sh.settings()).into_bytes())
        );
        assert_eq!(
            (sh.setup_name(), item_value(&sh, SAVE_OPTIONS).as_str()),
            ("mine", "mine")
        );
        // `orbmit` is shipped: the box, then the entry on `orbmit`; Esc cancels with two
        // MenuSelects and saves nothing.
        enter_on(&mut sh, &mut sim, SAVE_OPTIONS);
        assert_eq!(input_state(&sh).buffer, b"mine");
        let o = retype(&mut sh, &mut sim, 4, "orbmit");
        assert_eq!(
            (o.menu_sounds, top_box(&sh).text.as_str()),
            (vec![select], "NAME 'orbmit.cfg' IS RESERVED")
        );
        tap(&mut sh, &mut sim, 57);
        assert_eq!(input_state(&sh).buffer, b"orbmit");
        let o = tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(
            (o.menu_sounds, sh.top_char(), sh.setup_name()),
            (vec![select, select], 'M', "mine")
        );
        assert_eq!(
            sh.store().read("Setups/orbmit.cfg"),
            Some(data_file("Setups/orbmit.cfg"))
        );
        // Rust only (D7): a name the store cannot place gets the same box.
        enter_on(&mut sh, &mut sim, SAVE_OPTIONS);
        let o = retype(&mut sh, &mut sim, 4, "a/b");
        assert_eq!(
            (o.menu_sounds, top_box(&sh).text.as_str()),
            (vec![select], "NAME 'a/b.cfg' IS RESERVED")
        );
        tap(&mut sh, &mut sim, 57);
        assert_eq!(input_state(&sh).buffer, b"a/b");
        // An empty Return completes like a cancel.
        let o = retype(&mut sh, &mut sim, 3, "");
        assert_eq!(
            (o.menu_sounds, sh.top_char(), sh.setup_name()),
            (vec![select, select], 'M', "mine")
        );
        let saved: Vec<String> = sh
            .store()
            .list("Setups")
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(saved, ["liero.cfg", "mine.cfg", "orbmit.cfg"]);
    }

    #[test]
    fn number_entry_raises_the_numeric_keyboard() {
        let (mut sh, mut sim) = settings_focus();
        assert_eq!(sh.text_mode(), None);
        enter_on(&mut sh, &mut sim, SI_LIVES);
        assert_eq!(sh.text_mode(), Some(TextMode::Numeric));
    }

    #[test]
    fn the_single_layer_store_refuses_only_the_reserved_name() {
        // The C++ web build (plan D9): `orbmit` is no separate layer to shadow.
        let store = MemoryStore::single_layer([
            ("Setups/liero.cfg", data_file("Setups/liero.cfg")),
            ("Setups/orbmit.cfg", data_file("Setups/orbmit.cfg")),
        ])
        .with_root_label("./user");
        let (mut sh, mut sim) = boot_store(store);
        enter_on(&mut sh, &mut sim, SAVE_OPTIONS);
        tap(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(top_box(&sh).text, "NAME 'liero.cfg' IS RESERVED");
        tap(&mut sh, &mut sim, 57);
        retype(&mut sh, &mut sim, 5, "orbmit");
        assert_eq!((sh.top_char(), sh.setup_name()), ('M', "orbmit"));
        assert_eq!(
            sh.store().read("Setups/orbmit.cfg"),
            Some(scenario::settings_toml::settings_to_toml(sh.settings()).into_bytes())
        );
    }

    fn orbmit() -> Settings {
        let text = String::from_utf8(data_file("Setups/orbmit.cfg")).unwrap();
        scenario::settings_toml::settings_from_toml(&text).unwrap()
    }

    /// F7 (or the settings focus already), LOAD SETUP, Down ×`downs` inside Setups, Return; the
    /// Return frame.
    fn load_setup(sh: &mut Shell, sim: &mut SimState, downs: usize) -> FrameOut {
        tap(sh, sim, DK_F7);
        let o = enter_on(sh, sim, LOAD_OPTIONS);
        assert_eq!(
            (o.menu_sounds, sh.top_char()),
            (vec![hooks().hooks.select], 'P')
        );
        assert_eq!(view(sh), ('P', "./user/Setups".into(), 0), "inside Setups");
        taps(sh, sim, DK_DOWN, downs);
        tap(sh, sim, DK_RETURN)
    }

    #[test]
    fn load_setup_replaces_the_settings_and_the_name() {
        let (mut sh, mut sim) = boot_on(files::tests::install());
        let o = load_setup(&mut sh, &mut sim, 1);
        assert_eq!(
            (o.menu_sounds, o.upd, sh.top_char()),
            (vec![hooks().hooks.select], Phase::Menu, 'M'),
            "one MenuSelect; the state pops that frame (T0 P4)"
        );
        assert_eq!(*sh.settings(), orbmit());
        assert_eq!(
            (sh.settings().lives, sh.settings().loading_time),
            (9, 20),
            "T0 P4"
        );
        assert_eq!(
            (sh.setup_name(), item_value(&sh, SAVE_OPTIONS).as_str()),
            ("orbmit", "orbmit")
        );
        assert_eq!(item_value(&sh, SI_LIVES), "9", "UpdateItems");
        assert!(o.notes.is_empty());
    }

    #[test]
    fn load_setup_of_a_file_that_does_not_parse_is_a_note_and_changes_nothing() {
        let store = files::tests::install();
        store.write("Setups/bad.cfg", b"settings = [[[").unwrap();
        let (mut sh, mut sim) = boot_on(store);
        edited(sh.settings_mut());
        let before = sh.settings().clone();
        let o = load_setup(&mut sh, &mut sim, 0);
        assert_eq!(sh.top_char(), 'M');
        assert_eq!(*sh.settings(), before);
        assert_eq!(sh.setup_name(), "liero");
        assert_eq!(o.notes.len(), 1, "{:?}", o.notes);
        assert!(o.notes[0].starts_with("LOAD SETUP: Setups/bad.cfg: "));
    }

    #[test]
    fn load_setup_detaches_a_paused_match_unless_the_switch_is_off() {
        for detach in [true, false] {
            let (mut sh, mut sim) = boot_on(files::tests::install());
            sh.debug_mut().load_detach = detach;
            start_match(&mut sh, &mut sim);
            to_menu(&mut sh, &mut sim);
            let before = live_fields(&sim);
            let kept = sh.current().unwrap().settings().clone();
            load_setup(&mut sh, &mut sim, 1);
            assert_eq!(sh.current().unwrap().attached(), !detach);
            assert_eq!(live_fields(&sim), before, "the menu never touches the sim");
            let outs = until_routed(&mut sh, &mut sim, DK_F1);
            assert_eq!(outs.last().unwrap().routed, Some(Route::Resume));
            let o = orbmit();
            let loaded = (
                o.max_bonuses,
                o.weap_table.iter().map(|&v| v as i32).collect(),
                o.game_mode,
                o.time_to_lose,
                o.blood,
                o.loading_time,
                o.load_change,
                o.shadow,
            );
            assert_ne!(loaded, before);
            if detach {
                assert_eq!(
                    live_fields(&sim),
                    before,
                    "the paused game kept its settings (P8)"
                );
                assert_eq!(*sh.current().unwrap().settings(), kept);
            } else {
                assert_eq!(live_fields(&sim), loaded, "the counterfactual resyncs");
            }
        }
    }

    #[test]
    fn new_game_after_load_setup_keeps_the_loaded_picks() {
        // Plan fact 17: the old selection's picks belong to the old settings once detached.
        let (mut sh, mut sim) = boot_on(files::tests::install());
        until_routed(&mut sh, &mut sim, DK_RETURN);
        step(&mut sh, &mut sim, &[], [2, 0]); // P1 Down: the cursor onto weapon slot 1
        step(&mut sh, &mut sim, &[], [0, 0]);
        step(&mut sh, &mut sim, &[], [8, 0]); // P1 Right: slot 1's pick cycles
        let moved = sh.settings().worm_settings[0].weapons;
        let loaded = orbmit().worm_settings.each_ref().map(|w| w.weapons);
        assert_ne!(moved, loaded[0]);
        to_menu(&mut sh, &mut sim);
        load_setup(&mut sh, &mut sim, 1);
        assert_eq!(
            sh.settings().worm_settings.each_ref().map(|w| w.weapons),
            loaded
        );
        tap(&mut sh, &mut sim, DK_ESCAPE);
        sh.main_menu_mut().move_to_id(MA_NEW_GAME);
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(
            outs.last().unwrap().routed,
            Some(Route::NewGame { seed: 22 })
        );
        assert_eq!(
            sh.settings().worm_settings.each_ref().map(|w| w.weapons),
            loaded,
            "the menu's picks are the loaded setup's, not the old selection's"
        );
        assert!(
            sh.current().unwrap().attached(),
            "the new match shares them"
        );
    }

    /// A store whose writes fail (a read-only user folder).
    struct ReadOnly(MemoryStore);

    impl ConfigStore for ReadOnly {
        fn read(&self, rel: &str) -> Option<Vec<u8>> {
            self.0.read(rel)
        }
        fn write(&self, _rel: &str, _bytes: &[u8]) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "read-only"))
        }
        fn shadows_system(&self, subdir: &str, leaf: &str) -> bool {
            self.0.shadows_system(subdir, leaf)
        }
        fn root_label(&self) -> &str {
            self.0.root_label()
        }
        fn list(&self, rel: &str) -> Vec<storage::DirEntry> {
            self.0.list(rel)
        }
    }

    #[test]
    fn a_save_setup_as_write_error_is_a_note_and_keeps_the_name() {
        let select = hooks().hooks.select;
        let seeds = SeedSource::Fixed(3);
        let (mut sh, mut sim, _) = Shell::boot(
            tc(),
            Settings::default(),
            Box::new(ReadOnly(files::tests::install())),
            seeds,
            0,
            StartOptions::default(),
        );
        idle(&mut sh, &mut sim, 40);
        tap(&mut sh, &mut sim, DK_F7);
        enter_on(&mut sh, &mut sim, SAVE_OPTIONS);
        let o = retype(&mut sh, &mut sim, 5, "mine");
        assert_eq!(
            (o.menu_sounds, sh.top_char(), sh.setup_name()),
            (vec![select, select], 'M', "liero"),
            "the completion's MenuSelect + UpdateItems still run"
        );
        assert_eq!(o.notes.len(), 1, "{:?}", o.notes);
        assert!(o.notes[0].starts_with("SAVE SETUP AS: Setups/mine.cfg: "));
    }

    // Step 4½f-1 (T7 Step 4): the CPU player (`DumbLieroAI`) through `Shell::frame`.
    mod cpu {
        use sim::ai::DumbLieroAi;
        use sim::hash::hash_game_state;
        use sim_core::fixed::ftoi;

        use super::*;
        use crate::keys::KeyEdges;
        use crate::shell::selection::{BOT_WEAPONS_KEEP, CONTROLLER_BOT};

        /// C++ `Settings()` with player 2 the CPU and BOT WEAPONS KEEP (ready at once with its
        /// saved picks: BIG NUKE, MINI NUKE, DOOMSDAY, CRACKLER, NAPALM, so it dies too), and
        /// REGENERATE LEVEL on, so every NEW GAME generates its level from its own seed.
        fn cpu_settings() -> Settings {
            let mut s = Settings::default();
            s.worm_settings[1].controller = CONTROLLER_BOT;
            s.worm_settings[1].weapons = [2, 28, 14, 11, 32];
            s.select_bot_weapons = BOT_WEAPONS_KEEP;
            s.regenerate_level = true;
            s
        }

        fn boot_with(
            settings: Settings,
            matches: &[u32],
            touch_only: bool,
        ) -> (Shell, SimState) {
            let seeds = SeedSource::Scripted {
                boot: 11,
                matches: VecDeque::from(matches.to_vec()),
            };
            let options = StartOptions {
                touch_only,
                ..StartOptions::default()
            };
            let (sh, sim, _) = Shell::boot(
                tc(),
                settings,
                Box::new(MemoryStore::new()),
                seeds,
                0,
                options,
            );
            (sh, sim)
        }

        fn m(sh: &Shell) -> &Match {
            sh.current().expect("a match")
        }

        /// Menu → NEW GAME → the selection (the CPU ready at once) → P1 Up, Fire: the frame
        /// that finalises it (no tick, no AI). P2's keys are never pressed.
        fn start_cpu_match(sh: &mut Shell, sim: &mut SimState) {
            idle(sh, sim, 40);
            until_routed(sh, sim, DK_RETURN);
            let ws = m(sh).weapon_selection().expect("selection");
            assert!(
                !ws.player(0).ready && ws.player(1).ready,
                "KEEP: the CPU is ready on the selection's first frame (weapsel.cpp:95)"
            );
            for w in [1, 0] {
                step(sh, sim, &[], [w, 0]);
            }
            let o = step(sh, sim, &[], [16, 0]);
            assert!(!o.sim_ticked && sh.phase() == Phase::Game, "DONE finalises");
            assert_eq!(m(sh).ai(1).unwrap().rand.draws(), 0, "no AI in selection");
        }

        /// Player 1 presses FIRE on every other tick while it waits to respawn and is idle
        /// otherwise, so the CPU has a target; player 2 has no key at all.
        fn p1_word(sim: &SimState) -> u32 {
            u32::from(!sim.worms[0].visible && sim.cycles % 2 == 0) * 16
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct Tick {
            pub word: u32,
            pub visible: bool,
            pub pos: (i32, i32),
            pub lives: [i32; 2],
            pub last: u32,
            pub hash: u32,
        }

        /// `n` frames of [`p1_word`]; one [`Tick`] per frame (every one ticks the match).
        fn drive(sh: &mut Shell, sim: &mut SimState, n: usize) -> Vec<Tick> {
            (0..n)
                .map(|_| {
                    let o = step(sh, sim, &[], [p1_word(sim), 0]);
                    assert!(o.sim_ticked);
                    let w = &sim.worms[1];
                    Tick {
                        word: w.control_states.pack(),
                        visible: w.visible,
                        pos: (ftoi(w.pos.x), ftoi(w.pos.y)),
                        lives: [sim.worms[0].lives, w.lives],
                        last: m(sh).ai(1).unwrap().rand.last(),
                        hash: hash_game_state(sim),
                    }
                })
                .collect()
        }

        /// Whether player 2 was visible, then not, then visible again.
        fn died_and_respawned(ticks: &[Tick]) -> bool {
            let mut phase = 0;
            for t in ticks {
                phase = match (phase, t.visible) {
                    (0, true) | (1, true) => 1,
                    (1, false) | (2, false) => 2,
                    (2, true) | (3, _) => 3,
                    (p, _) => p,
                };
            }
            phase == 3
        }

        /// The pinned CPU match, found by [`search_cpu_seeds`] over seeds 1..=40 (22 of them
        /// qualify; 8 dies first): with [`p1_word`] and no P2 key, the CPU is visible, dies (by
        /// its own explosives, on match tick 597) and is visible again within 1,500 ticks, and
        /// player 1 never dies. With `lives = 1` that death is the game over.
        const CPU_SEED: u32 = 8;

        #[test]
        fn a_cpu_player_fights_dies_and_respawns_without_a_key() {
            let (mut sh, mut sim) = boot_with(cpu_settings(), &[CPU_SEED], false);
            start_cpu_match(&mut sh, &mut sim);
            assert!(!m(&sh).is_cpu(0) && m(&sh).is_cpu(1));
            let ticks = drive(&mut sh, &mut sim, 1500);
            assert_eq!(
                ticks[0].last, 0x2af0_9813,
                "the first tick of a fresh mt19937(0x1337) AI (T0 P1)"
            );
            assert!(ticks.iter().any(|t| t.word != 0), "the CPU presses keys");
            let seen: Vec<(i32, i32)> = ticks.iter().filter(|t| t.visible).map(|t| t.pos).collect();
            assert!(
                seen.windows(2).any(|w| w[0] != w[1]),
                "the CPU moves while visible"
            );
            assert!(
                died_and_respawned(&ticks),
                "the CPU dies and respawns by itself"
            );
            assert!(m(&sh).ai_traces()[1].ran && !m(&sh).ai_traces()[0].ran);
        }

        #[test]
        fn the_match_runs_the_ai_on_the_edge_word_before_the_tick_in_cpp_order() {
            // A hand driver on its own state, built as the NEW GAME builds it: `edges.apply`,
            // then each CPU's `DumbLieroAi::process` in `(k + cycles % 2) % 2` order
            // (localController.cpp:156-164), then the sim tick. Same states and AI draws on
            // every tick.
            let settings = cpu_settings();
            let (mut sh, mut sim) = boot_with(settings.clone(), &[CPU_SEED], false);
            start_cpu_match(&mut sh, &mut sim);
            let cfg = scenario::settings::MatchConfig {
                settings: settings.clone(),
                seed: CPU_SEED,
            };
            let level = generate_level(tc(), &settings, None, CPU_SEED);
            let mut h = scenario::build::new_match(tc(), &cfg, &level)
                .unwrap()
                .state;
            let wcfg = scenario::build::weapsel_config(&settings);
            let mut ws = sim::weapsel::WeaponSelection::new(&mut h, &wcfg).unwrap();
            for (w, done) in [(1, false), (0, false), (16, true)] {
                let inputs = [ControlState::unpack(w), ControlState::unpack(0)];
                assert_eq!(ws.process_frame(&mut h, &inputs), done);
            }
            ws.finalize(&mut h);
            scenario::build::enter_game(&mut h, &cfg);
            assert_eq!(hash_game_state(&h), hash_game_state(&sim), "the same start");
            let mut edges = KeyEdges::default();
            let mut ais = [None, Some(DumbLieroAi::new())];
            let mut words = Vec::new();
            for k in 0..1500 {
                // Two empty ticks first: the DONE latch lets go (keys::ReleaseLatch).
                let p1 = if k < 2 { 0 } else { p1_word(&h) };
                let mut inputs = edges.apply(
                    &[ControlState::unpack(p1), ControlState::unpack(0)],
                    &h.worms,
                );
                for i in 0..2 {
                    let w = (i + h.cycles.rem_euclid(2) as usize) % 2;
                    if let Some(ai) = ais[w].as_mut() {
                        inputs[w] = ai.process(&h, w, inputs[w]);
                    }
                }
                words.push(inputs[1].pack());
                h.process_frame(&inputs);
                h.drain_shake_events();
                assert!(step(&mut sh, &mut sim, &[], [p1, 0]).sim_ticked);
                assert_eq!(hash_game_state(&h), hash_game_state(&sim), "tick {k}");
                assert_eq!(h.worms[1].control_states, sim.worms[1].control_states);
                assert_eq!(
                    ais[1].as_ref().unwrap().rand.last(),
                    m(&sh).ai(1).unwrap().rand.last()
                );
            }
            assert!(words.iter().any(|&w| w != 0));
        }

        #[test]
        fn no_ai_runs_in_selection_and_a_pick_bot_takes_its_players_keys() {
            // Plan fact 5, T0 P3: BOT WEAPONS PICK. P2's keys drive the CPU's menu exactly as
            // they drive a human player 2's (the same words and states on every selection
            // frame, no AI draw); the AI starts on the first match tick.
            let run = |controller: u32| {
                let mut s = Settings::default();
                s.worm_settings[1].controller = controller;
                assert_eq!(s.select_bot_weapons, 1, "PICK");
                let (mut sh, mut sim) = boot_with(s, &[CPU_SEED], false);
                idle(&mut sh, &mut sim, 40);
                until_routed(&mut sh, &mut sim, DK_RETURN);
                let p2 = *m(&sh).weapon_selection().unwrap().player(1);
                assert!(!p2.ready);
                let mut frames =
                    vec![(sim.worms[1].control_states, hash_game_state(&sim), Some(p2))];
                // P2: Down, Right (weapon 1 changes), Up, Up (DONE!), Fire; then P1 Up, Fire.
                for w in [
                    [0, 2],
                    [0, 0],
                    [0, 8],
                    [0, 0],
                    [0, 1],
                    [0, 0],
                    [0, 1],
                    [0, 0],
                    [0, 16],
                    [0, 0],
                    [1, 0],
                    [0, 0],
                    [16, 0],
                ] {
                    let o = step(&mut sh, &mut sim, &[], w);
                    assert!(!o.sim_ticked);
                    let p2 = m(&sh).weapon_selection().map(|ws| *ws.player(1));
                    if controller == 1 {
                        assert_eq!(m(&sh).ai(1).unwrap().rand.draws(), 0, "no AI draw");
                        assert!(!m(&sh).ai_traces()[1].ran);
                    }
                    frames.push((sim.worms[1].control_states, hash_game_state(&sim), p2));
                }
                assert_eq!(sh.phase(), Phase::Game, "both DONE");
                (sh, sim, frames)
            };
            let (mut sh, mut sim, cpu) = run(CONTROLLER_BOT);
            let (_, _, human) = run(0);
            assert_eq!(cpu, human, "the AI is invisible in selection");
            let p2: Vec<_> = cpu.iter().filter_map(|f| f.2).collect();
            assert_ne!(
                p2[0].cursor, p2[1].cursor,
                "P2's Down moves the CPU's cursor"
            );
            assert_ne!(p2[0].picks, p2[3].picks, "P2's Right changes its weapon 1");
            assert!(
                p2.last().unwrap().ready,
                "P2's Fire on DONE! readies the CPU"
            );
            assert!(m(&sh).is_cpu(1));
            step(&mut sh, &mut sim, &[], [0, 0]);
            assert!(m(&sh).ai_traces()[1].ran);
            assert_eq!(
                m(&sh).ai(1).unwrap().rand.last(),
                0x2af0_9813,
                "the first draws on the first match tick"
            );
        }

        /// The AI's per-frame (draws, P2 word) over frames that each tick the match.
        fn ai_frames(sh: &Shell, sim: &SimState) -> (u64, u32) {
            (
                m(sh).ai(1).unwrap().rand.draws(),
                sim.worms[1].control_states.pack(),
            )
        }

        fn assert_ai_ran_on_each(frames: &[(u64, u32)], what: &str) {
            assert!(
                frames.windows(2).all(|w| w[1].0 > w[0].0),
                "{what}: the AI draws on every tick"
            );
            assert!(
                frames.windows(2).any(|w| w[1].1 != w[0].1),
                "{what}: P2's word changes"
            );
        }

        #[test]
        fn the_esc_fade_and_the_post_mortem_still_run_the_ai() {
            // kStateGame / kStateGameEnded: every tick the match processes runs the AIs
            // (localController.cpp:156-175, plan fact 5).
            // Esc where the undisturbed run's P2 word changes within the next 32 ticks (the fade
            // ticks are those same ticks: Esc touches no worm).
            let (mut sh, mut sim) = boot_with(cpu_settings(), &[CPU_SEED], false);
            start_cpu_match(&mut sh, &mut sim);
            let reference = drive(&mut sh, &mut sim, 1500);
            let esc = (100..1400)
                .find(|&k| {
                    reference[k - 1..k + 32]
                        .windows(2)
                        .any(|w| w[0].word != w[1].word)
                })
                .expect("the CPU acts");
            let (mut sh, mut sim) = boot_with(cpu_settings(), &[CPU_SEED], false);
            start_cpu_match(&mut sh, &mut sim);
            drive(&mut sh, &mut sim, esc);
            let mut fade = vec![ai_frames(&sh, &sim)];
            let p1 = p1_word(&sim);
            let mut o = step(&mut sh, &mut sim, &[ev(DK_ESCAPE, true)], [p1, 0]);
            fade.push(ai_frames(&sh, &sim));
            let mut up = vec![ev(DK_ESCAPE, false)];
            while o.routed.is_none() {
                let p1 = p1_word(&sim);
                o = step(&mut sh, &mut sim, &up, [p1, 0]);
                up.clear();
                assert!(o.sim_ticked);
                fade.push(ai_frames(&sh, &sim));
            }
            assert_eq!(fade.len(), 33, "32 fade ticks");
            assert_ai_ran_on_each(&fade, "the Esc fade");

            let mut s = cpu_settings();
            s.lives = 1;
            let (mut sh, mut sim) = boot_with(s, &[CPU_SEED], false);
            start_cpu_match(&mut sh, &mut sim);
            let mut k = 0;
            while !sim::game_over::is_game_over(&sim) {
                let p1 = p1_word(&sim);
                step(&mut sh, &mut sim, &[], [p1, 0]);
                k += 1;
                assert!(k < 4000, "the pinned match ends");
            }
            let mut post = vec![ai_frames(&sh, &sim)];
            while sh.phase() == Phase::Game {
                let o = step(&mut sh, &mut sim, &[], [0, 0]);
                if !o.sim_ticked {
                    break;
                }
                post.push(ai_frames(&sh, &sim));
            }
            assert!(post.len() > 150, "the post-mortem ({} ticks)", post.len());
            assert_ai_ran_on_each(&post, "the post-mortem");
            assert_eq!(sh.top_char(), 'M', "back to the menu");
        }

        #[test]
        fn every_new_game_makes_a_fresh_ai() {
            // CreateAi runs in every NEW GAME's LocalController (T0 P1): the second match
            // replays the first one's words exactly (the level regenerates from the same seed).
            let (mut sh, mut sim) = boot_with(cpu_settings(), &[CPU_SEED, CPU_SEED], false);
            start_cpu_match(&mut sh, &mut sim);
            let first = drive(&mut sh, &mut sim, 200);
            to_menu(&mut sh, &mut sim);
            sh.main_menu_mut().move_to_id(MA_NEW_GAME);
            let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(
                outs.last().unwrap().routed,
                Some(Route::NewGame { seed: CPU_SEED })
            );
            for w in [1, 0, 16] {
                step(&mut sh, &mut sim, &[], [w, 0]);
            }
            assert_eq!(m(&sh).ai(1).unwrap().rand.draws(), 0);
            let second = drive(&mut sh, &mut sim, 200);
            assert_eq!(first, second);
        }

        #[test]
        fn two_shell_runs_with_the_same_seeds_hash_alike() {
            // Plan D13 (no recorded-CPU replay yet): the CPU match is deterministic.
            let run = || {
                let (mut sh, mut sim) = boot_with(cpu_settings(), &[CPU_SEED], false);
                start_cpu_match(&mut sh, &mut sim);
                drive(&mut sh, &mut sim, 1000)
            };
            let (a, b) = (run(), run());
            assert_eq!(
                a.iter().map(|t| t.hash).collect::<Vec<_>>(),
                b.iter().map(|t| t.hash).collect::<Vec<_>>()
            );
            assert_eq!(a, b);
        }

        #[test]
        fn the_skip_route_runs_the_ai_from_tick_zero() {
            let opts = StartOptions {
                skip_selection: true,
                loadout: vec!["BAZOOKA".into()],
                touch_only: false,
            };
            let (mut sh, mut sim, out) = Shell::boot_playing(
                tc(),
                cpu_settings(),
                Box::new(MemoryStore::new()),
                SeedSource::Fixed(7),
                0,
                opts,
            );
            assert_eq!(out.phase, Phase::Game);
            assert!(m(&sh).is_cpu(1) && !m(&sh).is_cpu(0));
            assert_eq!(m(&sh).ai(1).unwrap().rand.draws(), 0);
            assert!(step(&mut sh, &mut sim, &[], [0, 0]).sim_ticked);
            assert!(m(&sh).ai_traces()[1].ran);
            assert_eq!(m(&sh).ai(1).unwrap().rand.last(), 0x2af0_9813);
        }

        #[test]
        fn resume_reapplies_the_worm_colours_while_attached() {
            // D9, T0 P6: Game::Focus rewrites the ramps on every RESUME, from the game's
            // settings (the menu's while attached, the old object after LOAD SETUP), and the
            // menus draw with them afterwards.
            let red = [255, 20, 20];
            for (edit, detach) in [(false, false), (true, false), (true, true)] {
                let (mut sh, mut sim, _) = boot();
                start_match(&mut sh, &mut sim);
                to_menu(&mut sh, &mut sim);
                let old = m(&sh).origpal().clone();
                if detach {
                    sh.current.as_mut().unwrap().detach();
                }
                if edit {
                    sh.settings_mut().worm_settings[0].rgb = red;
                }
                until_routed(&mut sh, &mut sim, DK_F1);
                let got = m(&sh).origpal().clone();
                let mut want = old.clone();
                if edit && !detach {
                    render::palette::set_worm_colour(&mut want, 0, red);
                    assert_ne!(got, old, "the new ramp");
                }
                assert_eq!(got, want, "edit {edit} detach {detach}");
                assert_eq!(sh.world.origpal, got, "the menus' palette follows");
            }
        }

        #[test]
        fn unequal_healths_start_and_resume_at_each_worms_own_max() {
            let mut settings = Settings::default();
            settings.worm_settings[0].health = 40;
            settings.worm_settings[1].health = 250;
            let (mut sh, mut sim) = boot_with(settings, &[21], false);
            start_match(&mut sh, &mut sim);
            assert_eq!(maxes(&sim), [40, 250]);
            assert_eq!((sim.worms[0].health, sim.worms[1].health), (40, 250));
            to_menu(&mut sh, &mut sim);
            sh.settings_mut().worm_settings[0].health = 30;
            until_routed(&mut sh, &mut sim, DK_F1);
            assert_eq!(maxes(&sim), [30, 250], "RESUME brings the max (T0 P5)");
            assert_eq!(sim.worms[0].health, 40, "no tick yet");
            assert!(step(&mut sh, &mut sim, &[], [0, 0]).sim_ticked);
            assert_eq!(
                (sim.worms[0].health, sim.worms[1].health),
                (30, 250),
                "the clamp (worm.cpp:213) on the first resumed tick"
            );
        }

        #[test]
        fn a_touch_only_load_setup_makes_player_two_the_cpu_again() {
            assert_eq!(super::orbmit().worm_settings[1].controller, 0);
            for touch_only in [false, true] {
                let seeds = SeedSource::Scripted {
                    boot: 11,
                    matches: VecDeque::from([21]),
                };
                let (mut sh, mut sim, _) = Shell::boot(
                    tc(),
                    Settings::default(),
                    Box::new(files::tests::install()),
                    seeds,
                    0,
                    StartOptions {
                        touch_only,
                        ..StartOptions::default()
                    },
                );
                idle(&mut sh, &mut sim, 40);
                load_setup(&mut sh, &mut sim, 1);
                let mut want = super::orbmit();
                if touch_only {
                    want.worm_settings[1].controller = CONTROLLER_BOT;
                }
                assert_eq!(*sh.settings(), want, "touch_only {touch_only}");
            }
        }

        #[test]
        fn a_touch_only_new_game_gives_the_cpu_random_weapons_ready_at_once() {
            // D3's selection half: BOT WEAPONS RANDOM draws the picks in the constructor
            // (weapsel.cpp:57-61), which a live match allows (only a G2 case refuses it).
            let mut s = cpu_settings();
            s.select_bot_weapons = 1; // PICK in the file: the touch rule overrides it
            let picks = |touch_only: bool| {
                let (mut sh, mut sim) = boot_with(s.clone(), &[CPU_SEED], touch_only);
                idle(&mut sh, &mut sim, 40);
                until_routed(&mut sh, &mut sim, DK_RETURN);
                let ws = m(&sh).weapon_selection().unwrap();
                assert!(!ws.player(0).ready, "P1 is human");
                (ws.player(1).ready, ws.player(1).picks, sim.rand.draws())
            };
            let (desk_ready, desk_picks, desk_draws) = picks(false);
            let (ready, random, draws) = picks(true);
            assert!(!desk_ready && ready, "PICK on a desktop; RANDOM on a phone");
            assert_eq!(desk_picks, s.worm_settings[1].weapons, "the saved picks");
            assert_eq!(draws, desk_draws + 5, "five rand(1, 41) draws");
            assert_ne!(random, desk_picks);
            assert!(random.iter().all(|&p| (1..=40).contains(&p)));
        }

        #[test]
        fn the_ais_switch_leaves_a_cpu_to_its_keys() {
            // `ShellDebug::ais` (the G2f-1 negative control): no AI step, so the CPU worm plays
            // on its keys' edge words, as a human would: with no key, an empty word.
            let (mut sh, mut sim) = boot_with(cpu_settings(), &[CPU_SEED], false);
            sh.debug_mut().ais = false;
            start_cpu_match(&mut sh, &mut sim);
            let ticks = drive(&mut sh, &mut sim, 300);
            assert!(m(&sh).is_cpu(1));
            assert_eq!(m(&sh).ai(1).unwrap().rand.draws(), 0);
            assert!(!m(&sh).ai_traces()[1].ran);
            assert!(ticks.iter().all(|t| t.word == 0));
        }

        #[test]
        #[ignore = "the seed search behind the pinned CPU seeds (run by hand)"]
        fn search_cpu_seeds() {
            let lo: u32 = std::env::var("CPU_SEARCH_LO").map_or(1, |v| v.parse().unwrap());
            let hs: Vec<_> = (0..4u32)
                .map(|t| {
                    std::thread::spawn(move || {
                        for seed in (lo..lo + 40).filter(|s| s % 4 == t) {
                            let (mut sh, mut sim) = boot_with(cpu_settings(), &[seed], false);
                            start_cpu_match(&mut sh, &mut sim);
                            let ticks = drive(&mut sh, &mut sim, 1500);
                            let first_death = ticks.iter().position(|t| t.lives != [15, 15]);
                            println!(
                                "seed {seed}: respawn {} first death {first_death:?} lives {:?}",
                                died_and_respawned(&ticks),
                                ticks.last().unwrap().lives
                            );
                        }
                    })
                })
                .collect();
            for h in hs {
                h.join().unwrap();
            }
        }
    }
}
