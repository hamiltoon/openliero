//! Step 4½c — the live weapon-selection phase (design §7), Bevy-free (moved to `ui::shell` in
//! Step 4½d).
//!
//! C++ runs the phase inside `LocalController` (`localController.cpp:112-152`, `:224-229`): a
//! fresh controller constructs `WeaponSelection`, every `Process` runs the 12/3 key repeat and
//! `ProcessFrame`, and the frame the last player readies calls `Finalize` and enters the game.
//! [`Selection`] owns the running `sim::weapsel::WeaponSelection` and the in-memory picks it
//! starts from and writes back to. Since 4½d the frozen screen and `menu_cycles` come from the
//! shell (design §4.7, §4.11): the frozen background is drawn into the shell's shared frozen
//! screen, and the palette rotates by the shell's `menu_cycles`. The picks stand in for the C++
//! `WormSettings` a `shared_ptr` shares with the worms (finding 4): every NEW GAME — here F5 and
//! the post-match restart — starts from them. Loading a setup is 4½d's.
//!
//! Two configs feed it: [`new_game_config`] (the NEW GAME start, `game::new_game`: the
//! `Settings` picks, as C++ does) and [`live_config`] (a native `--live <scenario>` match: the
//! scenario's launched loadout as the saved picks, design §7.3 / Q7).

use render::bitmap::{Bitmap, Pal32, Rect};
use render::frame::Scene;
use render::weapsel::{self as screen, WeapselTexts};
use scenario::build::weapsel_config;
use scenario::settings::Settings;
use sim::state::{ControlState, NUM_WEAPONS, SimState};
use sim::weapsel::{WEAPON_COUNT, WeaponSelection, WeapselConfig, WeapselError, weap_order};

/// `WormSettings::controller` DumbLieroAI (`localController.cpp:20`).
pub const CONTROLLER_BOT: u32 = 1;
/// `Settings::select_bot_weapons` KEEP (`hiddenMenu.cpp:10`): a bot keeps its saved picks and
/// readies at once (`weapsel.cpp:95`).
pub const BOT_WEAPONS_KEEP: u32 = 2;

/// The 1-based `weap_order` picks that reproduce worm `worm`'s launched loadout (design §7.3,
/// Q7): the inverse of `Worm::InitWeapons`. The default-match fixture launches DART + four
/// copies of the first weapon by name, i.e. `[12, 1, 1, 1, 1]`.
pub fn loadout_picks(state: &SimState, worm: usize) -> [u32; NUM_WEAPONS] {
    let order = weap_order(&state.weapons);
    state.worms[worm].weapons.map(|w| {
        let id = w.ty.expect("a launched loadout fills every slot") as usize;
        order
            .iter()
            .position(|&i| i == id)
            .expect("every weapon is in weap_order") as u32
            + 1
    })
}

/// What a scenario-started live match (native `--live <scenario>`) selects from (design §7.3):
/// `Settings::default()` (every weapon enabled, bots PICK, both human) with each worm's launched
/// loadout as its saved picks, plus the touch rule ([`apply_touch_rule`]).
pub fn live_config(state: &SimState, touch_only: bool) -> WeapselConfig {
    let mut cfg = weapsel_config(&Settings::default());
    for (i, p) in cfg.players.iter_mut().enumerate() {
        p.weapons = loadout_picks(state, i);
    }
    apply_touch_rule(&mut cfg, touch_only);
    cfg
}

/// What the NEW GAME start selects from: the `Settings` as C++ passes them to
/// `WeaponSelection` (`weapsel.cpp:28-97`), plus the touch rule ([`apply_touch_rule`]).
pub fn new_game_config(settings: &Settings, touch_only: bool) -> WeapselConfig {
    let mut cfg = weapsel_config(settings);
    apply_touch_rule(&mut cfg, touch_only);
    cfg
}

/// John's Q8 ruling: on a touch-only page player 2 has no input, so it becomes a bot with
/// `select_bot_weapons = KEEP`, ready at once. Until 4½f it idles in the match as before.
fn apply_touch_rule(cfg: &mut WeapselConfig, touch_only: bool) {
    if touch_only {
        cfg.players[1].controller = CONTROLLER_BOT;
        cfg.select_bot_weapons = BOT_WEAPONS_KEEP;
    }
}

/// The live phase and the in-memory picks (see the module doc).
pub struct Selection {
    cfg: WeapselConfig,
    /// `WormSettings::name` of players 0/1: `Settings::default()`'s (empty) until 4½f.
    names: [String; 2],
    active: Option<WeaponSelection>,
    /// `WeaponSelection::cached_background` (`weapsel.hpp:27`): the frozen background has been
    /// drawn into the shell's shared frozen screen.
    cached_background: bool,
    /// `WeaponSelection::focused` (`weapsel.hpp:28`).
    focused: bool,
}

impl Selection {
    pub fn new(cfg: WeapselConfig) -> Selection {
        Selection {
            cfg,
            names: Default::default(),
            active: None,
            cached_background: false,
            focused: true,
        }
    }

    /// The saved picks and rules every new selection starts from.
    pub fn config(&self) -> &WeapselConfig {
        &self.cfg
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active(&self) -> Option<&WeaponSelection> {
        self.active.as_ref()
    }

    /// `ChangeState(kStateWeaponSelection)` on a freshly built tick-0 `state`: a new
    /// `WeaponSelection` from the saved picks (it draws `state.rand`), focused, with its frozen
    /// background still to draw (`weapsel.hpp:27-28`).
    pub fn begin(&mut self, state: &mut SimState) -> Result<(), WeapselError> {
        self.active = Some(WeaponSelection::new(state, &self.cfg)?);
        self.cached_background = false;
        self.focused = true;
        Ok(())
    }

    /// One phase tick (design §7.1). Appends the menu sample ids to `sounds`. Returns true on the
    /// frame the last player readies; the selection is then finalized (`InitWeapons` +
    /// `ReleaseControls`), its picks written back, and it is no longer active.
    pub fn step(
        &mut self,
        state: &mut SimState,
        inputs: &[ControlState; 2],
        sounds: &mut Vec<i32>,
    ) -> bool {
        let ws = self
            .active
            .as_mut()
            .expect("step needs an active selection");
        let done = ws.process_frame(state, inputs);
        sounds.extend_from_slice(ws.menu_sounds());
        if done {
            let ws = self.active.take().expect("active");
            let picks = ws.finalize(state);
            self.write_back(picks);
        }
        done
    }

    /// F5 or a restart mid-selection (design §7.4): the running picks — constructor rolls and
    /// every Left/Right included — become the saved picks, as the C++ aliasing makes them
    /// (finding 4). No-op when no selection runs: `step` already wrote the finalized picks back.
    pub fn abandon(&mut self) {
        if let Some(ws) = self.active.take() {
            self.write_back([ws.player(0).picks, ws.player(1).picks]);
        }
    }

    fn write_back(&mut self, picks: [[u32; NUM_WEAPONS]; 2]) {
        for (p, picks) in self.cfg.players.iter_mut().zip(picks) {
            p.weapons = picks;
        }
    }

    /// `WeaponSelection::DrawNormalViewports` (`weapsel.cpp:160-209`) into `surface`: the weapsel
    /// palette at the shell's `menu_cycles`, then, on the first draw of this selection, the frozen
    /// background INTO THE SHELL'S shared frozen screen (`gfx.frozen_screen`, which the main
    /// menu's `Enter` also writes — finding 11), then the frozen copy and, while focused, the
    /// header, names and menus. Returns the palette the draw leaves behind.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        surface: &mut Bitmap,
        frozen: &mut Bitmap,
        state: &SimState,
        scene: &Scene,
        texts: &WeapselTexts,
        level_file: &str,
        menu_cycles: u32,
    ) -> Pal32 {
        let ws = self
            .active
            .as_ref()
            .expect("render needs an active selection");
        let pal = screen::weapsel_palette(scene.origpal, menu_cycles);
        if !self.cached_background {
            let label = screen::level_label(texts, level_file);
            *frozen = screen::build_frozen(state, scene, &label, menu_cycles);
            self.cached_background = true;
        }
        if self.focused {
            let names = [self.names[0].as_str(), self.names[1].as_str()];
            screen::draw_screen(
                surface,
                frozen,
                &pal,
                scene.font,
                texts,
                ws,
                &state.weapons,
                names,
            );
        } else {
            surface.pixels.copy_from_slice(&frozen.pixels);
            surface.clip = Rect::new(0, 0, surface.w, surface.h);
        }
        pal
    }

    /// A running selection reads `settings->weap_table` live (`weapsel.cpp:255`, `:278`, `:327`;
    /// plan fact 15): RESUME hands it the menu's table. The enabled count stays the constructor's.
    pub fn set_weap_table(&mut self, t: [u32; WEAPON_COUNT]) {
        if let Some(ws) = self.active.as_mut() {
            ws.set_weap_table(t);
        }
    }

    /// `WeaponSelection::Focus` / `Unfocus` (`weapsel.cpp:363-365`).
    pub fn focus(&mut self) {
        self.focused = true;
    }

    pub fn unfocus(&mut self) {
        self.focused = false;
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use render::bitmap::Bitmap;
    use scenario::Scenario;
    use scenario::paths::TC_ROOT;

    use super::*;

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../game/scenarios/default_match.txt"
    ));

    fn default_match() -> scenario::Loaded {
        scenario::load(Path::new(TC_ROOT), &Scenario::parse(FIXTURE).unwrap())
    }

    fn cs(bits: u32) -> ControlState {
        ControlState::unpack(bits)
    }

    #[test]
    fn the_launched_loadout_inverts_to_dart_plus_the_first_weapon_by_name() {
        let st = default_match().state;
        for worm in 0..2 {
            assert_eq!(
                loadout_picks(&st, worm),
                [12, 1, 1, 1, 1],
                "DART + BAZOOKA x4 (Q7)"
            );
        }
    }

    #[test]
    fn the_live_config_is_the_cpp_default_plus_the_touch_rule() {
        let st = default_match().state;
        let keys = live_config(&st, false);
        assert_eq!(keys.weap_table, [0; 40]);
        assert_eq!(keys.select_bot_weapons, 1, "Settings(): PICK");
        assert_eq!(
            (keys.players[0].controller, keys.players[1].controller),
            (0, 0)
        );
        let touch = live_config(&st, true);
        assert_eq!(
            (
                touch.players[0].controller,
                touch.players[1].controller,
                touch.select_bot_weapons
            ),
            (0, CONTROLLER_BOT, BOT_WEAPONS_KEEP)
        );
        assert_eq!(touch.players[1].weapons, keys.players[1].weapons);
    }

    #[test]
    fn the_new_game_config_is_the_settings_plus_the_touch_rule() {
        let s = Settings::default();
        assert_eq!(new_game_config(&s, false), weapsel_config(&s));
        let touch = new_game_config(&s, true);
        assert_eq!(touch.players[0], weapsel_config(&s).players[0]);
        assert_eq!(
            (
                touch.players[1].controller,
                touch.select_bot_weapons,
                touch.players[1].weapons
            ),
            (CONTROLLER_BOT, BOT_WEAPONS_KEEP, [1; 5]),
            "Settings(): the saved picks are [1; 5] (settings.cpp, data/Setups/liero.cfg)"
        );
    }

    #[test]
    fn on_a_touch_only_page_player_one_alone_ends_the_phase() {
        let mut st = default_match().state;
        let mut sel = Selection::new(live_config(&st, true));
        sel.begin(&mut st).unwrap();
        let ws = sel.active().unwrap();
        assert!(
            !ws.player(0).ready && ws.player(1).ready,
            "Q8: player 2 is ready at once"
        );
        assert_eq!(st.rand.draws(), 0, "KEEP with saved picks draws nothing");
        let mut sounds = Vec::new();
        for (bits, done) in [(1, false), (0, false), (16, true)] {
            assert_eq!(sel.step(&mut st, &[cs(bits), cs(0)], &mut sounds), done);
        }
        assert!(!sel.is_active());
    }

    #[test]
    fn finishing_writes_the_picks_back_and_loads_them() {
        let mut st = default_match().state;
        let mut sel = Selection::new(live_config(&st, false));
        sel.begin(&mut st).unwrap();
        let mut sounds = Vec::new();
        // P0: Down to slot 0, Right (DART -> DIRTBALL), Up, Up to DONE, Fire. P1: Up, Fire.
        for w in [
            [2, 1],
            [0, 0],
            [8, 16],
            [0, 0],
            [1, 0],
            [0, 0],
            [1, 0],
            [0, 0],
        ] {
            assert!(!sel.step(&mut st, &[cs(w[0]), cs(w[1])], &mut sounds));
        }
        assert!(sel.step(&mut st, &[cs(16), cs(0)], &mut sounds));
        assert_eq!(
            sel.config().players[0].weapons,
            [13, 1, 1, 1, 1],
            "written back"
        );
        let ty = st.worms[0].weapons[0].ty.expect("loaded") as usize;
        assert_eq!(st.weapons[ty].name, "DIRTBALL");
        assert_eq!(
            st.worms[0].weapons[0].ammo, st.weapons[ty].ammo,
            "InitWeapons: full ammo"
        );
        assert_eq!(st.worms[0].control_states.pack(), 0, "ReleaseControls");
        assert!(!sounds.is_empty());
    }

    #[test]
    fn abandoning_keeps_the_running_picks_for_the_next_selection() {
        let mut st = default_match().state;
        let mut sel = Selection::new(live_config(&st, false));
        sel.begin(&mut st).unwrap();
        let mut sounds = Vec::new();
        for w in [2, 0, 8] {
            sel.step(&mut st, &[cs(w), cs(0)], &mut sounds);
        }
        sel.abandon(); // F5 mid-selection
        assert!(!sel.is_active());
        assert_eq!(
            sel.config().players[0].weapons,
            [13, 1, 1, 1, 1],
            "finding 4"
        );
        let mut fresh = default_match().state;
        sel.begin(&mut fresh).unwrap();
        assert_eq!(
            sel.active().unwrap().player(0).picks,
            [13, 1, 1, 1, 1],
            "NEW GAME from them"
        );
    }

    #[test]
    fn render_builds_the_shared_frozen_screen_once_and_reads_the_callers_menu_cycles() {
        // weapsel.cpp:160-186 + finding 11: the pixels live in the SHELL's frozen screen.
        let scenario::Loaded {
            mut state, scene, ..
        } = default_match();
        let mut sel = Selection::new(live_config(&state, false));
        sel.begin(&mut state).unwrap();
        let s = scene.as_scene(0, false);
        let (mut surface, mut frozen) = (Bitmap::new(320, 200), Bitmap::new(320, 200));
        let lf = "Levels/render_stage.lev";
        let pal = sel.render(
            &mut surface,
            &mut frozen,
            &state,
            &s,
            &scene.weapsel_texts,
            lf,
            5,
        );
        assert_eq!(
            pal,
            render::weapsel::weapsel_palette(&scene.origpal, 5),
            "UpdateWeapselPalette"
        );
        let (first_surface, first_frozen) = (surface.clone(), frozen.clone());
        sel.render(
            &mut surface,
            &mut frozen,
            &state,
            &s,
            &scene.weapsel_texts,
            lf,
            6,
        );
        assert_eq!(frozen, first_frozen, "cached_background: built once");
        assert_ne!(
            surface, first_surface,
            "the selected item's colour 168 rotates"
        );
        sel.unfocus();
        sel.render(
            &mut surface,
            &mut frozen,
            &state,
            &s,
            &scene.weapsel_texts,
            lf,
            7,
        );
        assert_eq!(
            surface.pixels, frozen.pixels,
            "unfocused: the frozen copy only (:184-186)"
        );
        frozen.pixels.fill(0xFF12_3456); // the main menu's Enter overwrote the shared screen
        sel.focus();
        sel.render(
            &mut surface,
            &mut frozen,
            &state,
            &s,
            &scene.weapsel_texts,
            lf,
            8,
        );
        assert_eq!(
            surface.get_pixel(0, 199),
            0xFF12_3456,
            "RESUME redraws over the menu's frozen screen"
        );
    }
}
