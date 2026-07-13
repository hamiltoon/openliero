//! Test-only helpers for building a minimal two-worm [`SimState`] without going
//! through `scenario::load` (which needs the real TC asset tree on disk). Mirrors
//! the pattern `sim::state`'s own unit tests use (`idle_state`/`synthetic_level`/
//! `two_worms`) — a bare `SimState::new` call with `Default`/empty values for
//! every field the obs/reward mappers under test don't read.
//!
//! `#[cfg(test)]`-only: never compiled into the `cdylib`/non-test `rlib`.

use assets::level::LevelData;
use assets::sprite::SpriteSet;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::state::{SimState, WeaponInit, WormInit, WormState, NUM_WEAPONS};
use sim_core::vec::Vec2;

/// A flat, uniform `width`x`height` level — obs/reward never read
/// `material_id` content, only `level.width`/`level.height` (obs's position
/// normalization), so a single filler byte per cell is enough.
pub fn flat_level(width: i32, height: i32) -> LevelData {
    LevelData {
        width,
        height,
        material_id: vec![0u8; (width * height).max(0) as usize],
        palette: None,
        display: None,
    }
}

/// A [`WormInit`] with every weapon slot empty (`ty: None, ammo: 0`) — obs/reward
/// tests that care about weapons overwrite `state.worms[i].weapons` afterward.
pub fn worm_init(index: i32, pos: Vec2, health: i32, lives: i32, visible: bool) -> WormInit {
    WormInit {
        index,
        health,
        lives,
        stats_x: 0,
        weapons: [WeaponInit::default(); NUM_WEAPONS],
        start_pos: pos,
        visible,
    }
}

/// Build a tick-0 two-worm [`SimState`] over a `level_w`x`level_h` flat level,
/// with worm 0 / worm 1 built from `w0`/`w1`. Every TC-sourced field (weapon
/// table, physics/control consts, sprite banks, blood/bonus constants, …) is
/// `Default`/empty — obs/reward never read them (they only read `WormState`
/// fields + `level.width/height/cossin/settings_health`, all present here).
pub fn two_worm_state(level_w: i32, level_h: i32, w0: WormInit, w1: WormInit) -> SimState {
    let level = flat_level(level_w, level_h);
    SimState::new(
        &level,
        &[w0, w1],
        0, // seed — obs/reward tests never consume RNG
        &[0u8; 256],
        Vec::new(),
        PhysicsConsts::default(),
        ControlConsts::default(),
        false,
        SpriteSet::default(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        100,  // settings_loading_time
        true, // load_change
        100,  // blood
    )
}

/// Convenience: mutate worm `idx`'s `WormState` in place via a closure, for
/// tests that need to set fields `WormInit` doesn't expose directly (e.g.
/// `aiming_angle`, `vel`, `current_weapon`, per-slot `ammo`/`loading_left`).
pub fn set_worm(state: &mut SimState, idx: usize, f: impl FnOnce(&mut WormState)) {
    f(&mut state.worms[idx]);
}
