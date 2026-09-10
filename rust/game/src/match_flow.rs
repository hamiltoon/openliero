//! The match lifecycle after weapon selection (Step 4½a-1, design §6): the tail of C++
//! `LocalController::Process` (`localController.cpp:153-199`) and `ChangeState`
//! (`:214-290`), Bevy-free so it is headlessly testable (the `lib.rs` rule).
//!
//! Once `Game::IsGameOver` holds (`:177-179`) the state becomes `GameEnded` with
//! `fade_value = 180` (`:277-282`); the sim KEEPS TICKING (the frame loop runs for
//! `kStateGame || kStateGameEnded`, `:153-155`); each call decrements the fade and the
//! call that finds it at 0 ends the match (`:185-194`) — 180 more simulated frames.
//! `fade_value` is also the C++ renderer fade (`:211`); 4½d draws it. Seams: 4½c adds a
//! weapon-selection phase in front, 4½d the Esc fade (`OnKey`, `:82-85`), 4½g routes
//! `Finished` to the stats screen.

use sim::state::SimState;

/// `ChangeState(kStateGameEnded)`'s fade (`localController.cpp:279`).
pub const POST_MORTEM_FRAMES: i32 = 180;
/// The fade-in ceiling (`localController.cpp:196`) and the value entering the game from
/// weapon selection (`:285`).
pub const FADE_IN_MAX: i32 = 33;

/// `kStateGame` / `kStateGameEnded`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchPhase {
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

    pub fn phase(&self) -> MatchPhase {
        self.phase
    }

    pub fn fade_value(&self) -> i32 {
        self.fade_value
    }

    /// Call once per tick AFTER `process_frame` (the C++ order: `ProcessFrame`, then
    /// `IsGameOver`, then the fade bookkeeping).
    pub fn after_frame(&mut self, state: &SimState) -> FlowStep {
        if self.phase == MatchPhase::Game && sim::game_over::is_game_over(state) {
            self.phase = MatchPhase::GameEnded;
            if !self.going_to_menu {
                self.fade_value = POST_MORTEM_FRAMES;
                self.going_to_menu = true;
            }
        }
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
}
