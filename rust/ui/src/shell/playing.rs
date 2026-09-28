//! `Match`, the `LocalController` analog behind the `Playing` screen (design §4.12): the 4½c
//! live loop (`game/src/main.rs` `tick_and_render`, moved), with the Esc fade, `Focus`/`Unfocus`
//! and one shared fade tail. `process` is `LocalController::Process` (`localController.cpp:
//! 122-200`); `draw` is `LocalController::Draw` for the main window (`:203-212`), which also sets
//! the play renderer's fade. Plus the boot: the never-focused `LocalController` whose game the
//! first menu is drawn over (`gfx.cpp:1439-1465`). Step 4½f-1: the match's CPU players
//! (`DumbLieroAI`, `sim::ai`) run in `process`, after the key edges and before the tick, as
//! `LocalController::Process` runs them (`localController.cpp:156-175`; plan D8).

use std::path::Path;

use assets::level::LevelData;
use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::object_draw::SmallLabels;
use render::palette::{build_palette, set_worm_colour};
use render::viewport::Viewport;
use scenario::build::{BuildError, build_match, enter_game, new_match, validate_for_selection};
use scenario::settings::{GM_HOLDAZONE, GM_KILL_EM_ALL, MatchConfig, Settings, WEAP_TABLE_LEN};
use scenario::{Loaded, SceneData};
use sim::ai::{AiTrace, DumbLieroAi, run_ais_traced};
use sim::state::{ControlState, NUM_WEAPONS, SimState};
use sim::weapsel::WeaponSelection;

use super::HudFlags;
use super::loadout::apply_weapons;
use super::match_flow::{FlowStep, MatchFlow};
use super::selection::{CONTROLLER_BOT, Selection, new_game_config};
use super::viewport_step::tick_viewports;
use crate::keys::{CleanEdges, ReleaseLatch, fold_dig};

/// How a match starts (design §7.5): `skip_selection` (`?weapons=`), the preview loadout,
/// and the touch-only rule (4½c Q8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartOptions {
    pub skip_selection: bool,
    pub loadout: Vec<String>,
    pub touch_only: bool,
}

/// Players 1 and 2's names (`WormSettings::name`), as the selection's name boxes show them.
fn names_of(s: &Settings) -> [String; 2] {
    [
        s.worm_settings[0].name.clone(),
        s.worm_settings[1].name.clone(),
    ]
}

/// `Game::Focus` → `UpdateSettings` (`game.cpp:475-488`): both worms' ramps into `origpal`.
fn focus_palette(scene: &mut SceneData, settings: &Settings) {
    for i in 0..2 {
        set_worm_colour(&mut scene.origpal, i, settings.worm_settings[i].rgb);
    }
}

/// The boot controller (`gfx.cpp:1441-1450`): `LocalController(common, settings)` on the boot
/// level, focused for its palette only. Its sim RNG is never used (C++ seeds it from the clock).
///
/// Any setup boots (C++ constructs any mode's game from any `liero.cfg`; plan fact 21): this game
/// is never processed, so it is built from a sanitised copy and never panics.
/// - Holdazone is built as Kill'em All — the sim's Holdazone arm is unported — and only
///   `state.game_mode` (read by the HUD's timer arm) carries the 2 (Step 4½d G2
///   `shell_holdazone_boot`).
/// - Any other `validate_for_selection` refusal takes that field from `Settings::default()`.
///
/// The refusal box comes at NEW GAME (plan T4 Step 5), not here.
pub fn boot_state(tc_root: &Path, settings: &Settings, level: &LevelData, seed: u32) -> Loaded {
    let cfg = MatchConfig {
        settings: bootable(settings),
        seed,
    };
    let mut loaded = new_match(tc_root, &cfg, level).expect("a sanitised setup builds");
    loaded.state.game_mode = settings.game_mode;
    focus_palette(&mut loaded.scene, settings);
    loaded
}

/// [`boot_state`]'s sanitised copy of `settings`: every `validate_for_selection` refusal fixed
/// (one field per round; `TooManyWeapons` is the TC's, which the boot cannot fix).
fn bootable(settings: &Settings) -> Settings {
    let defaults = Settings::default();
    let mut s = settings.clone();
    if s.game_mode == GM_HOLDAZONE {
        s.game_mode = GM_KILL_EM_ALL;
    }
    for _ in 0..64 {
        let cfg = MatchConfig {
            settings: s.clone(),
            seed: 0,
        };
        match validate_for_selection(&cfg, WEAP_TABLE_LEN) {
            // `FollowAiUnsupported` is `refuse_follow_ai`'s only (the NEW GAME gate), never
            // `validate_for_selection`'s: a FollowAI player boots (4½f-1 D1).
            Ok(())
            | Err(BuildError::TooManyWeapons(_) | BuildError::FollowAiUnsupported { .. }) => {
                break;
            }
            Err(BuildError::HoldazoneUnsupported | BuildError::InvalidGameMode(_)) => {
                s.game_mode = defaults.game_mode;
            }
            Err(BuildError::InvalidHealth(_)) => {
                for i in 0..2 {
                    if s.worm_settings[i].health < 1 {
                        s.worm_settings[i].health = defaults.worm_settings[i].health;
                    }
                }
            }
            Err(BuildError::InvalidWeapon { worm, slot, .. }) => {
                s.worm_settings[worm].weapons[slot] = defaults.worm_settings[worm].weapons[slot];
            }
            Err(BuildError::InvalidBloodParticleMax(_)) => {
                s.blood_particle_max = defaults.blood_particle_max;
            }
        }
    }
    s
}

/// `controller->Draw(play_renderer)` of the unstarted boot game (`gfx.cpp:1452-1454`): a
/// `frame::draw` with fresh viewports (C++ never processed them), the HUD per `settings.map`,
/// the small labels per `settings.names_on_bonuses` (the same `Game::Draw`; Step 4½e-1).
/// Returns the palette it leaves behind (the copyright bar's, finding 12).
pub fn draw_boot(
    surface: &mut Bitmap,
    state: &SimState,
    scene: &SceneData,
    settings: &Settings,
) -> Pal32 {
    let mut s = scene.as_scene(state.screen_flash, settings.shadow);
    HudFlags {
        draw_hud: true,
        map: settings.map,
    }
    .apply(&mut s);
    s.small_labels = Some(SmallLabels {
        text: &scene.text_sprites,
        names_on_bonuses: settings.names_on_bonuses,
    });
    render::frame::draw(surface, state, &mut Viewport::player_layout(), &s);
    build_palette(s.origpal, s.color_anim, state.cycles, s.screen_flash)
}

pub struct Match {
    flow: MatchFlow,
    /// `None` when selection was skipped; kept (inactive) after DONE for the picks.
    selection: Option<Selection>,
    viewports: [Viewport; 2],
    scene: SceneData,
    /// Over 8-bit clean words (Step 4½f-2, plan D1): a DIG key held at a boundary stays latched.
    latch: ReleaseLatch,
    /// C++ `OnKey`'s edges over the clean words (`keys::apply_clean_edges`, both arms of the DIG
    /// rule): a bit the sim consumed stays clear while its key is held.
    edges: CleanEdges,
    /// The latched clean words of the last [`Match::process`] call, and the sim input of its last
    /// match tick (after the edges and the AIs): behaviour-free, for tests (4½f-2 T2).
    words: [u8; 2],
    inputs: [ControlState; 2],
    /// `Worm::ai` of each worm (`CreateAi`, `localController.cpp:19-28`): a `DumbLieroAI` for a
    /// controller-1 player, made at [`Match::start`] and kept for the whole match. RESUME and
    /// LOAD SETUP never touch it: CONTROLLER does not reach a running match (design finding 7).
    ais: [Option<DumbLieroAi>; 2],
    /// The AI step of the last `process` call (`ran == false` for a human, and on a frame with no
    /// match tick): behaviour-free, for tests and the G2 ledgers (plan D12).
    traces: [AiTrace; 2],
    hud: HudFlags,
    /// The match's copy of the settings: the `kStateGame` lives and blood pool, the selection's
    /// level label, the draw's shadow gate and `names_on_bonuses`. RESUME refreshes it while
    /// `attached` ([`Match::resync`]).
    cfg: MatchConfig,
    /// `sound_hook[SoundBegin]` (`game.cpp:500-503`).
    begin: i32,
    /// Whether the menu's settings are this match's settings (C++: the game holds the same
    /// `gfx.settings` pointer; plan D7). Set at NEW GAME; LOAD SETUP clears it ([`Match::detach`]).
    attached: bool,
}

impl Match {
    /// NEW GAME's controller on `level` (`gfx.cpp:1507-1523`) and `GamePlayState::Enter` →
    /// `LocalController::Focus` (`gamePlayState.cpp:14`, `localController.cpp:100-120`):
    /// `kStateInitial` → weapon selection (the `WeaponSelection` constructor draws `state.rand`),
    /// `Game::Focus`'s worm ramps, fade 0. `held` (the clean words over these settings, 4½f-2
    /// D1) arms the release latch: key-downs made in the menu never reached the controller
    /// (design §4.11). The skip route (`?weapons=`, Rust only)
    /// starts at match tick 0 like 4½c. The constructor's `CreateAi` (`localController.cpp:
    /// 19-45`): a fresh `DumbLieroAI` (`mt19937(0x1337)`) for every controller-1 player, once
    /// per NEW GAME (4½f-1 D8; T0 P1).
    pub fn start(
        tc_root: &Path,
        settings: &Settings,
        level: &LevelData,
        seed: u32,
        opts: &StartOptions,
        begin: i32,
        held: &[u8; 2],
    ) -> (Match, SimState) {
        // 4½f-1 D1: `CreateAi`'s FollowAI arm (controller 2) is unported. The menu's NEW GAME
        // gate refuses it (`build::refuse_follow_ai`), and the skip route cannot meet it (the
        // browser store holds only the shipped setups and `?cpu=` sets 0 or 1; plan fact 17). Were
        // it reached, that player would play as an input-less human.
        debug_assert!(
            settings.worm_settings[..2]
                .iter()
                .all(|w| w.controller != 2),
            "a FollowAI player reached Match::start (4½f Q2: refused at NEW GAME)"
        );
        let cfg = MatchConfig {
            settings: settings.clone(),
            seed,
        };
        let ais = [0, 1].map(|i| {
            (settings.worm_settings[i].controller == CONTROLLER_BOT).then(DumbLieroAi::new)
        });
        let built = if opts.skip_selection {
            build_match(tc_root, &cfg, level)
        } else {
            new_match(tc_root, &cfg, level)
        };
        let Loaded {
            mut state,
            viewports,
            mut scene,
        } = built.expect("the settings build a match");
        apply_weapons(&mut state, &opts.loadout);
        focus_palette(&mut scene, settings);
        let (flow, selection) = if opts.skip_selection {
            (MatchFlow::new(), None)
        } else {
            let mut sel = Selection::new(new_game_config(settings, opts.touch_only));
            sel.set_names(names_of(settings));
            sel.begin(&mut state)
                .expect("the settings select over the TC");
            (MatchFlow::with_weapon_selection(), Some(sel))
        };
        let mut latch = ReleaseLatch::default();
        latch.arm_clean(held);
        let hud = HudFlags {
            draw_hud: true,
            map: settings.map,
        };
        let m = Match {
            flow,
            selection,
            viewports,
            scene,
            latch,
            edges: CleanEdges::default(),
            words: [0; 2],
            inputs: [ControlState::new(); 2],
            ais,
            traces: [AiTrace::default(); 2],
            hud,
            cfg,
            begin,
            attached: true,
        };
        (m, state)
    }

    pub fn in_selection(&self) -> bool {
        self.selection.as_ref().is_some_and(Selection::is_active)
    }

    /// The running weapon selection, if any.
    pub fn weapon_selection(&self) -> Option<&WeaponSelection> {
        self.selection.as_ref().and_then(Selection::active)
    }

    /// Whether worm `i` is a CPU player (`worm->ai` is a `DumbLieroAI`).
    pub fn is_cpu(&self, i: usize) -> bool {
        self.ais[i].is_some()
    }

    /// Worm `i`'s AI (its RNG), if it is a CPU player.
    pub fn ai(&self, i: usize) -> Option<&DumbLieroAi> {
        self.ais[i].as_ref()
    }

    /// The latched clean words of the last [`Match::process`] call (see the field).
    pub fn words(&self) -> [u8; 2] {
        self.words
    }

    /// The sim input of the last match tick (see the field).
    pub fn inputs(&self) -> [ControlState; 2] {
        self.inputs
    }

    /// What each AI did in the last [`Match::process`] call (see the field).
    pub fn ai_traces(&self) -> &[AiTrace; 2] {
        &self.traces
    }

    pub fn running(&self) -> bool {
        self.flow.running()
    }

    pub fn fade(&self) -> i32 {
        self.flow.fade_value()
    }

    pub fn flow(&self) -> &MatchFlow {
        &self.flow
    }

    /// `renderer.Origpal()` after this match's `Game::Focus`.
    pub fn origpal(&self) -> &Palette {
        &self.scene.origpal
    }

    /// Whether RESUME hands this match the menu's settings (plan D7).
    pub fn attached(&self) -> bool {
        self.attached
    }

    /// LOAD SETUP (Step 4½e-2; finding 1's other half, T0 P8): C++ `LoadSettings` swaps a fresh
    /// `Settings` into `gfx.settings`, and this match's game keeps the old object. From here on
    /// RESUME hands it nothing, its selection's picks stay its own, and NEW GAME writes none back.
    pub fn detach(&mut self) {
        self.attached = false;
    }

    /// RESUME's refresh of the match's own copies of the settings (plan T4 Step 6; finding 1,
    /// facts 14-16), after `apply_live_settings` wrote the sim's live-read set: the whole
    /// `MatchConfig::settings` (the `kStateGame` lives and blood pool, the selection's
    /// `level_file`, the shadow gate, `names_on_bonuses`), the HUD's `map`, and a running
    /// selection's `weap_table` (`weapsel.cpp:255`, `:278`, `:327` read it live) and — Step
    /// 4½f-2 (R-10) — its name boxes' names (`weapsel.cpp:199-203` reads them live too). The kill
    /// banners read the refreshed copy in [`Match::draw`].
    pub fn resync(&mut self, s: &Settings) {
        self.cfg.settings = s.clone();
        self.hud.map = s.map;
        if let Some(sel) = self.selection.as_mut() {
            sel.set_weap_table(s.weap_table);
            sel.set_names(names_of(s));
        }
    }

    /// The names the selection's name boxes show, while one runs (Step 4½f-2; tests).
    pub fn selection_names(&self) -> Option<[&str; 2]> {
        self.selection
            .as_ref()
            .filter(|s| s.is_active())
            .map(Selection::names)
    }

    /// The selection's picks as the shared C++ `WormSettings::weapons` hold them now
    /// ([`Selection::picks`]); `None` when selection was skipped.
    pub fn picks(&self) -> Option<[[u32; NUM_WEAPONS]; 2]> {
        self.selection.as_ref().map(Selection::picks)
    }

    /// The HUD switches the match draws with.
    pub fn hud(&self) -> HudFlags {
        self.hud
    }

    /// The match's copy of the settings.
    pub fn settings(&self) -> &Settings {
        &self.cfg.settings
    }

    /// `OnKey(kDkEscape, _)`.
    pub fn esc(&mut self) {
        self.flow.esc();
    }

    /// RESUME (`gfx.cpp:1525-1530` → `LocalController::Focus`, `localController.cpp:100-120`):
    /// the flow, a running selection's `Focus`, the latch over the keys held at the boundary (the
    /// clean words over the match's settings after `resync`, 4½f-2 D1),
    /// and `Game::Focus`'s worm ramps from the match's settings, on every RESUME as C++ does
    /// (4½f-1 D9, T0 P6). Attached, [`Match::resync`] has just refreshed them, so a colour edited
    /// in the menu shows from the first resumed frame; detached, they are the old ones.
    pub fn focus(&mut self, held: &[u8; 2]) {
        self.flow.focus();
        if let Some(sel) = self.selection.as_mut().filter(|s| s.is_active()) {
            sel.focus();
        }
        self.latch.arm_clean(held);
        focus_palette(&mut self.scene, &self.cfg.settings);
    }

    /// `LocalController::Unfocus` (`localController.cpp:90-97`).
    pub fn unfocus(&mut self) {
        if let Some(sel) = self.selection.as_mut().filter(|s| s.is_active()) {
            sel.unfocus();
        }
    }

    /// `LocalController::Process` (`localController.cpp:122-200`) on this frame's clean words
    /// (`keys::clean_words` over this match's settings, 4½f-2 D1): the latch, then the selection
    /// step on the words with DIG folded into Left + Right (`keys::fold_dig`) (the frame the last player readies runs
    /// `ChangeState(kStateGame)`: Finalize, lives, `StartGame`'s blood pool and `SoundBegin`,
    /// fade 33) or one match tick on `OnKey`'s edges ([`CleanEdges`]; the selection keeps its own
    /// 4½c key repeat): the AIs (`:156-164`, [`run_ais_traced`]: a CPU worm's word is its
    /// post-tick `control_states` plus the edges of any key bound to it, fact 6), then
    /// `tick_viewports` + game over (`:175`). No AI runs in selection, nor on the frame that
    /// finalises it (`if … else if`, `:123-153`). Then the shared tail. `ais` false skips the AI
    /// step (a test-only switch, `ShellDebug::ais`). Returns (keep running, the sim ticked).
    pub fn process(
        &mut self,
        sim: &mut SimState,
        words: [u8; 2],
        ais: bool,
        sounds: &mut Vec<i32>,
    ) -> (bool, bool) {
        let mut w = words;
        self.latch.apply_clean(&mut w);
        self.words = w;
        self.traces = [AiTrace::default(); 2];
        let mut ticked = false;
        if self.in_selection() {
            let sel = self.selection.as_mut().expect("in selection");
            if sel.step(sim, &w.map(fold_dig), sounds) {
                enter_game(sim, &self.cfg);
                if self.begin >= 0 {
                    sounds.push(self.begin);
                }
                self.flow.enter_game();
                self.latch.arm_clean(&words);
            }
        } else {
            let mut inputs = self.edges.apply(&w, &sim.worms);
            if ais {
                run_ais_traced(&mut self.ais, sim, &mut inputs, &mut self.traces);
            }
            self.inputs = inputs;
            tick_viewports(&mut self.viewports, sim, &inputs);
            self.flow.check_game_over(sim);
            ticked = true;
        }
        (self.flow.tail() == FlowStep::Continue, ticked)
    }

    /// `GamePlayState::Draw` for the main window (`gamePlayState.cpp:98-101` →
    /// `LocalController::Draw`, `localController.cpp:203-212`): the selection screen or the game.
    /// Returns the palette the draw leaves behind; the caller takes `fade()` as the play
    /// renderer's fade (`:211`). `small_labels` draws the three `DrawTextSmall` labels (Step
    /// 4½e-1; off only for T8's counterfactual witness).
    pub fn draw(
        &mut self,
        surface: &mut Bitmap,
        frozen: &mut Bitmap,
        sim: &SimState,
        menu_cycles: u32,
        small_labels: bool,
    ) -> Pal32 {
        let mut scene = self
            .scene
            .as_scene(sim.screen_flash, self.cfg.settings.shadow);
        self.hud.apply(&mut scene);
        // 4½f-2 D11: the kill banners' names, from the match's copy of the settings.
        let ws = &self.cfg.settings.worm_settings;
        scene.names = [&ws[0].name, &ws[1].name];
        if small_labels {
            scene.small_labels = Some(SmallLabels {
                text: &self.scene.text_sprites,
                names_on_bonuses: self.cfg.settings.names_on_bonuses,
            });
        }
        match self.selection.as_mut().filter(|s| s.is_active()) {
            Some(sel) => sel.render(
                surface,
                frozen,
                sim,
                &scene,
                &self.scene.weapsel_texts,
                &self.cfg.settings.level_file,
                menu_cycles,
            ),
            None => {
                render::frame::draw(surface, sim, &mut self.viewports, &scene);
                build_palette(
                    scene.origpal,
                    scene.color_anim,
                    sim.cycles,
                    scene.screen_flash,
                )
            }
        }
    }

    /// C++ `WeaponSelection` edits the shared `WormSettings::weapons` in place (4½c finding 4): a
    /// running selection's picks — or the finalized ones — become the settings' picks for the
    /// next NEW GAME.
    pub fn write_back_picks(mut self, settings: &mut Settings) {
        if let Some(sel) = self.selection.as_mut() {
            sel.abandon();
            for i in 0..2 {
                settings.worm_settings[i].weapons = sel.config().players[i].weapons;
            }
        }
    }
}
