//! Port of `Game::IsGameOver` (`game.cpp:521-544`), Step 4½a-1 (design §5.3).
//!
//! A pure read of sim state, checked by the match flow after every tick
//! (`localController.cpp:177-179`). Total: Holdazone (mode 2) checks `time_to_lose`
//! exactly like C++ (not a "time to win" — cpp-map §7.1), and an unknown mode is never
//! over. The builder refuses Holdazone and unknown modes, so neither reaches a match.

use crate::state::SimState;

/// `true` once the match has ended: KillEmAll (0) / ScalesOfJustice (3) when any worm
/// has `lives <= 0`; GameOfTag (1) / Holdazone (2) when any worm's `timer >=
/// time_to_lose`; any other mode never.
pub fn is_game_over(state: &SimState) -> bool {
    match state.game_mode {
        0 | 3 => state.worms.iter().any(|w| w.lives <= 0),
        1 | 2 => state.worms.iter().any(|w| w.timer >= state.time_to_lose),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::ControlConsts;
    use crate::physics::PhysicsConsts;
    use crate::state::{WeaponInit, WormInit, NUM_WEAPONS};
    use assets::level::LevelData;
    use assets::sprite::SpriteSet;
    use sim_core::vec::Vec2;

    fn state(game_mode: u32) -> SimState {
        let level = LevelData {
            width: 4,
            height: 4,
            material_id: vec![0; 16],
            palette: None,
            display: None,
        };
        let worm = |index: i32, stats_x: i32| WormInit {
            index,
            health: 100,
            lives: 5,
            stats_x,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::zero(),
            visible: true,
        };
        let mut s = SimState::new(
            &level,
            &[worm(0, 0), worm(1, 218)],
            1,
            &[0u8; 256],
            Vec::new(),
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            100,
            true,
            100,
        );
        s.game_mode = game_mode;
        s
    }

    #[test]
    fn kill_em_all_and_scales_end_when_any_worm_has_no_lives() {
        for mode in [0, 3] {
            let mut s = state(mode);
            assert!(!is_game_over(&s));
            s.worms[1].timer = 10_000; // timers are irrelevant in these modes
            assert!(!is_game_over(&s));
            s.worms[1].lives = 0;
            assert!(
                is_game_over(&s),
                "mode {mode}: lives <= 0 (game.cpp:522-528)"
            );
            s.worms[1].lives = -2;
            assert!(is_game_over(&s));
        }
    }

    #[test]
    fn game_of_tag_and_holdazone_end_on_time_to_lose() {
        for mode in [1, 2] {
            let mut s = state(mode);
            s.time_to_lose = 12;
            s.worms[0].lives = 0; // lives are irrelevant in these modes
            s.worms[1].timer = 11;
            assert!(!is_game_over(&s));
            s.worms[1].timer = 12;
            assert!(
                is_game_over(&s),
                "mode {mode}: timer >= time_to_lose (game.cpp:529-540)"
            );
        }
    }

    #[test]
    fn an_unknown_mode_never_ends() {
        let mut s = state(7);
        s.worms[0].lives = 0;
        s.worms[0].timer = 10_000;
        assert!(!is_game_over(&s));
    }
}
