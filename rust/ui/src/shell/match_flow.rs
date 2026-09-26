//! The match lifecycle after weapon selection (Step 4½a-1, design §6): the tail of C++
//! `LocalController::Process` (`localController.cpp:153-199`) and `ChangeState`
//! (`:214-290`), Bevy-free (moved to `ui::shell` in Step 4½d).
//!
//! Once `Game::IsGameOver` holds (`:177-179`) the state becomes `GameEnded` with
//! `fade_value = 180` (`:277-282`); the sim KEEPS TICKING (the frame loop runs for
//! `kStateGame || kStateGameEnded`, `:153-155`); each call decrements the fade and the
//! call that finds it at 0 ends the match (`:185-194`) — 180 more simulated frames.
//! `fade_value` is also the C++ renderer fade (`:211`); 4½d draws it. Since 4½c the flow
//! starts in the weapon-selection phase (`with_weapon_selection`; `LocalController::Focus`,
//! `:112-119`) unless selection is skipped. Since 4½d: the Esc fade (`esc`, `OnKey` `:82-85`),
//! `focus` (RESUME) and one shared `tail` for every phase; 4½g routes `Finished` to the stats
//! screen.

use sim::state::SimState;

/// `ChangeState(kStateGameEnded)`'s fade (`localController.cpp:279`).
pub const POST_MORTEM_FRAMES: i32 = 180;
/// The fade-in ceiling (`localController.cpp:196`) and the value entering the game from
/// weapon selection (`:285`).
pub const FADE_IN_MAX: i32 = 33;

/// `kStateWeaponSelection` / `kStateGame` / `kStateGameEnded`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchPhase {
    /// `kStateWeaponSelection` (Step 4½c): `game::selection` runs the phase; the sim does not tick.
    WeaponSelection,
    Game,
    GameEnded,
}

/// `LocalController::Process`'s return value: `Continue` = true, `Finished` = false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowStep {
    Continue,
    Finished,
}

#[derive(Clone, Debug)]
pub struct MatchFlow {
    phase: MatchPhase,
    fade_value: i32,
    going_to_menu: bool,
}

impl Default for MatchFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchFlow {
    /// Entering `kStateGame` from weapon selection: `fade_value = 33` (`:284-287`).
    pub fn new() -> Self {
        MatchFlow {
            phase: MatchPhase::Game,
            fade_value: FADE_IN_MAX,
            going_to_menu: false,
        }
    }

    /// A fresh controller's `Focus` (`localController.cpp:112-119`): weapon selection first,
    /// `fade_value = 0`.
    pub fn with_weapon_selection() -> Self {
        MatchFlow {
            phase: MatchPhase::WeaponSelection,
            fade_value: 0,
            going_to_menu: false,
        }
    }

    /// `ChangeState(kStateGame)` from weapon selection (`:284-287`): fade 33; the next tick is
    /// match tick 0.
    pub fn enter_game(&mut self) {
        debug_assert_eq!(self.phase, MatchPhase::WeaponSelection);
        self.phase = MatchPhase::Game;
        self.fade_value = FADE_IN_MAX;
    }

    pub fn phase(&self) -> MatchPhase {
        self.phase
    }

    pub fn fade_value(&self) -> i32 {
        self.fade_value
    }

    /// `LocalController::OnKey(kDkEscape, _)` (`localController.cpp:82-85`): a key-down OR a
    /// key-up of Esc (finding 9) starts the 32-frame return to the menu, unless already leaving.
    pub fn esc(&mut self) {
        if !self.going_to_menu {
            self.fade_value = 31;
            self.going_to_menu = true;
        }
    }

    /// The flow half of `LocalController::Focus` (`localController.cpp:100-120`): after game over
    /// straight back to the menu; otherwise fade in from 0.
    pub fn focus(&mut self) {
        self.going_to_menu = self.phase == MatchPhase::GameEnded;
        self.fade_value = 0;
    }

    /// After a match tick: `IsGameOver` → `ChangeState(kStateGameEnded)` (`:177-179`, `:277-282`).
    pub fn check_game_over(&mut self, state: &SimState) {
        debug_assert_ne!(
            self.phase,
            MatchPhase::WeaponSelection,
            "after_frame runs after a match tick, not during weapon selection"
        );
        if self.phase == MatchPhase::Game && sim::game_over::is_game_over(state) {
            self.phase = MatchPhase::GameEnded;
            if !self.going_to_menu {
                self.fade_value = POST_MORTEM_FRAMES;
                self.going_to_menu = true;
            }
        }
    }

    /// The tail of every `LocalController::Process` (`localController.cpp:185-199`), in every
    /// phase: leaving counts down and finishes at 0; otherwise the fade counts up to 33.
    pub fn tail(&mut self) -> FlowStep {
        if self.going_to_menu {
            if self.fade_value > 0 {
                self.fade_value -= 1;
                FlowStep::Continue
            } else {
                FlowStep::Finished
            }
        } else {
            if self.fade_value < FADE_IN_MAX {
                self.fade_value += 1;
            }
            FlowStep::Continue
        }
    }

    /// One match tick's bookkeeping: [`check_game_over`](Self::check_game_over) then
    /// [`tail`](Self::tail) (the C++ order).
    pub fn after_frame(&mut self, state: &SimState) -> FlowStep {
        self.check_game_over(state);
        self.tail()
    }

    pub fn going_to_menu(&self) -> bool {
        self.going_to_menu
    }

    /// `LocalController::Running` (`:304`) for a started controller: false only after game over
    /// (the shell has no `kStateInitial` controller: the boot has no `Match`).
    pub fn running(&self) -> bool {
        self.phase != MatchPhase::GameEnded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use scenario::Scenario;

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
    const SAMPLE: &str = "seed 42\nlevel Levels/render_stage.lev\nticks 1\n\
                          worm 0 6553600 7602176 100 10 0   1\n\
                          worm 1 3276800 7602176 100 10 218 1\nweapon 0 DART\n";

    fn state() -> SimState {
        scenario::load(Path::new(TC_ROOT), &Scenario::parse(SAMPLE).unwrap()).state
    }

    #[test]
    fn a_running_match_continues_at_the_post_weapon_selection_fade() {
        let s = state();
        let mut f = MatchFlow::new();
        assert_eq!(
            f.fade_value(),
            FADE_IN_MAX,
            "kStateWeaponSelection -> kStateGame: 33"
        );
        for _ in 0..50 {
            assert_eq!(f.after_frame(&s), FlowStep::Continue);
        }
        assert_eq!(f.phase(), MatchPhase::Game);
        assert_eq!(f.fade_value(), 33);
    }

    #[test]
    fn game_over_runs_exactly_180_more_frames_then_finishes() {
        let mut s = state();
        let mut f = MatchFlow::new();
        assert_eq!(f.after_frame(&s), FlowStep::Continue);
        s.worms[1].lives = 0;
        // The detection call (the game-over frame): fade 180, then decremented once.
        assert_eq!(f.after_frame(&s), FlowStep::Continue);
        assert_eq!(f.phase(), MatchPhase::GameEnded);
        assert_eq!(f.fade_value(), 179);
        for n in 1..180 {
            assert_eq!(
                f.after_frame(&s),
                FlowStep::Continue,
                "post-mortem call {n}"
            );
        }
        assert_eq!(f.fade_value(), 0);
        assert_eq!(
            f.after_frame(&s),
            FlowStep::Finished,
            "the 180th frame after game over"
        );
    }

    #[test]
    fn a_revived_worm_does_not_cancel_the_post_mortem() {
        let mut s = state();
        let mut f = MatchFlow::new();
        s.worms[0].lives = 0;
        assert_eq!(f.after_frame(&s), FlowStep::Continue);
        s.worms[0].lives = 3; // C++ never leaves kStateGameEnded
        let mut steps = 1;
        while f.after_frame(&s) == FlowStep::Continue {
            steps += 1;
            assert!(steps <= 180, "must finish on schedule");
        }
        assert_eq!(steps, 180);
        assert_eq!(f.phase(), MatchPhase::GameEnded);
    }

    #[test]
    fn weapon_selection_fades_in_from_zero_then_the_game_starts_at_33() {
        // localController.cpp:119 (Focus: fade 0), :195-199 (+1 per Process, up to 33),
        // :284-287 (ChangeState from weapsel: 33).
        let mut f = MatchFlow::with_weapon_selection();
        assert_eq!(
            (f.phase(), f.fade_value()),
            (MatchPhase::WeaponSelection, 0)
        );
        for n in 1..=40 {
            assert_eq!(f.tail(), FlowStep::Continue);
            assert_eq!(f.fade_value(), n.min(FADE_IN_MAX));
        }
        f.enter_game();
        assert_eq!((f.phase(), f.fade_value()), (MatchPhase::Game, FADE_IN_MAX));
    }

    #[test]
    fn entering_the_game_mid_fade_jumps_to_33() {
        let mut f = MatchFlow::with_weapon_selection();
        for _ in 0..3 {
            assert_eq!(f.tail(), FlowStep::Continue);
        }
        f.enter_game();
        assert_eq!(f.fade_value(), FADE_IN_MAX);
        assert_eq!(f.after_frame(&state()), FlowStep::Continue);
    }

    #[test]
    #[should_panic(expected = "weapon selection")]
    fn after_frame_is_for_the_match_only() {
        MatchFlow::with_weapon_selection().after_frame(&state());
    }

    #[test]
    fn game_of_tag_ends_on_the_timer() {
        let mut s = state();
        s.game_mode = 1;
        s.time_to_lose = 5;
        let mut f = MatchFlow::new();
        s.worms[0].timer = 4;
        f.after_frame(&s);
        assert_eq!(f.phase(), MatchPhase::Game);
        s.worms[0].timer = 5;
        f.after_frame(&s);
        assert_eq!(f.phase(), MatchPhase::GameEnded);
    }

    #[test]
    fn esc_fades_out_over_31_presented_frames_then_finishes() {
        // localController.cpp:82-85 (fade 31, going_to_menu) + :185-194 (the tail).
        let mut f = MatchFlow::new();
        f.esc();
        assert!(f.going_to_menu());
        for want in (0..=30).rev() {
            assert_eq!(f.tail(), FlowStep::Continue);
            assert_eq!(f.fade_value(), want);
        }
        assert_eq!(f.tail(), FlowStep::Finished, "the 32nd call: the pop frame");
    }

    #[test]
    fn esc_is_ignored_while_already_leaving() {
        let mut f = MatchFlow::new();
        let mut dead = state(); // `SimState` is not `Clone`
        dead.worms[1].lives = 0;
        f.after_frame(&dead);
        let fade = f.fade_value();
        f.esc();
        assert_eq!(
            f.fade_value(),
            fade,
            "`!going_to_menu` guard (localController.cpp:82)"
        );
    }

    #[test]
    fn done_during_an_esc_fade_restarts_it_at_33() {
        // ChangeState(kStateGame) sets fade 33 even while going_to_menu (:284-287, design §3.7).
        let mut f = MatchFlow::with_weapon_selection();
        f.esc();
        f.tail();
        f.enter_game();
        assert_eq!(f.tail(), FlowStep::Continue);
        assert_eq!((f.fade_value(), f.going_to_menu()), (32, true));
    }

    #[test]
    fn focus_fades_back_in_from_zero_unless_the_game_ended() {
        let mut f = MatchFlow::new();
        f.esc();
        for _ in 0..5 {
            f.tail();
        }
        f.focus();
        assert_eq!(
            (f.going_to_menu(), f.fade_value()),
            (false, 0),
            "localController.cpp:118-119"
        );
        assert_eq!(f.tail(), FlowStep::Continue);
        assert_eq!(f.fade_value(), 1);
        let mut dead = state();
        dead.worms[0].lives = 0;
        let mut g = MatchFlow::new();
        g.after_frame(&dead);
        assert!(!g.running(), "Running(): not after game over (:304)");
        g.focus();
        assert_eq!((g.going_to_menu(), g.fade_value()), (true, 0), ":101-105");
        assert_eq!(g.tail(), FlowStep::Finished);
    }
}
