//! Egocentric per-agent observation extraction (design §3.1): a flat,
//! **fixed→float, one-directional** `Vec<f32>` — nothing computed here ever
//! writes back into the sim (the sim stays pure fixed-point/integer,
//! determinism-load-bearing, per `sim_core`'s own docs). `observe()` reads a
//! live [`SimState`] and returns `agent_idx`'s observation vector.
//!
//! ## Layout (`OBS_DIM` = 43 scalars)
//!
//! All values are "≈unit scale" per design §3.1 — normalized against a level
//! dimension, a documented approximate max (velocity/aim-speed/rope-length),
//! or a fixed denominator (health/ammo/loading) — NOT hard-clamped to `[-1,1]`
//! everywhere (a worm past 0 health, or moving unusually fast, legitimately
//! produces values outside the nominal range; that is signal, not a bug, and
//! it is still always a finite `f32` — see [`NaN-freedom`](#nan-freedom)
//! below).
//!
//! | Index | Field | Normalization |
//! |---|---|---|
//! | 0 | self `pos.x` | `/ level.width` |
//! | 1 | self `pos.y` | `/ level.height` |
//! | 2 | self `vel.x` | fixed→px, `/ VEL_SCALE` |
//! | 3 | self `vel.y` | fixed→px, `/ VEL_SCALE` |
//! | 4 | `sin(self.aiming_angle)` | `cossin[idx].x / 65536` |
//! | 5 | `cos(self.aiming_angle)` | `cossin[idx].y / 65536` |
//! | 6 | self `aiming_speed` | fixed→px, `/ AIM_SPEED_SCALE` |
//! | 7 | self `direction` | `0 -> -1.0` (left), `1 -> +1.0` (right) |
//! | 8 | self `health` | `/ settings_health` |
//! | 9 | self `lives` | `/ LIVES_SCALE` |
//! | 10 | self `visible` | `0.0` / `1.0` |
//! | 11 | self `able_to_jump` | `0.0` / `1.0` |
//! | 12 | self `able_to_dig` | `0.0` / `1.0` |
//! | 13 | self `ninjarope.out` | `0.0` / `1.0` |
//! | 14 | rope tip rel `x` (0 if stowed) | fixed→px, `/ ROPE_SCALE` |
//! | 15 | rope tip rel `y` (0 if stowed) | fixed→px, `/ ROPE_SCALE` |
//! | 16..21 | self `current_weapon` one-hot | `1.0` at the active slot |
//! | 21..26 | self per-slot `ammo` (slot order 0..5) | see [`ammo_norm`] (`-1` = infinite → `1.0`) |
//! | 26..31 | self per-slot `loading_left` | `/ LOADING_SCALE`, clamped `[0,1]` |
//! | 31 | opponent `pos.x` | `/ level.width` |
//! | 32 | opponent `pos.y` | `/ level.height` |
//! | 33 | opponent `vel.x` | fixed→px, `/ VEL_SCALE` |
//! | 34 | opponent `vel.y` | fixed→px, `/ VEL_SCALE` |
//! | 35 | `sin(opponent.aiming_angle)` | `cossin[idx].x / 65536` |
//! | 36 | `cos(opponent.aiming_angle)` | `cossin[idx].y / 65536` |
//! | 37 | opponent `health` | `/ settings_health` |
//! | 38 | opponent `lives` | `/ LIVES_SCALE` |
//! | 39 | opponent `visible` | `0.0` / `1.0` |
//! | 40 | relative `x` (`opp.pos.x - self.pos.x`) | `/ level.width` |
//! | 41 | relative `y` (`opp.pos.y - self.pos.y`) | `/ level.height` |
//! | 42 | relative distance | pixel Euclidean distance `/ level diagonal` |
//!
//! ## NaN-freedom
//! Every division here is by a compile-time-nonzero constant or a level
//! dimension (`scenario::load` never produces a zero-sized level) — never by a
//! runtime value that could be zero (e.g. ammo/loading are read, not divided
//! by). A dead/invisible worm (`health <= 0`, `lives == 0`, stale `pos`/`vel`
//! from its last living tick) still produces ordinary finite floats: `visible
//! == false` just reads `0.0` at its index, nothing branches into an undefined
//! value. [`observe_produces_no_nan_or_inf_for_a_dead_worm`] pins this.

use sim::state::{SimState, WormState, NUM_WEAPONS};
use sim_core::vec::Vec2;

/// Total observation vector length (see the module-doc layout table).
pub const OBS_DIM: usize = 16 + 3 * NUM_WEAPONS + 9 + 3;

/// Approximate max worm speed in pixels/tick, used only to bring `vel` to
/// "≈unit scale" (design §3.1) — NOT a hard clamp bound (see module docs).
pub const VEL_SCALE: f32 = 15.0;
/// Approximate max `aiming_speed` in pixels/tick-equivalent units.
pub const AIM_SPEED_SCALE: f32 = 5.0;
/// Assumed max `lives` setting, for scaling the `lives` scalar toward unit
/// range (the RL fixture uses `lives 1`; higher settings just read `> 1/LIVES_SCALE`).
pub const LIVES_SCALE: f32 = 5.0;
/// Approximate max ninjarope length in pixels (classic `NRMaxLength`-ish).
pub const ROPE_SCALE: f32 = 300.0;
/// Denominator for the per-slot ammo scalar; ammo pools observed in TC configs
/// top out around this order of magnitude. `-1` (infinite ammo) is a special
/// case mapped to `1.0`, not divided by this.
pub const AMMO_SCALE: f32 = 100.0;
/// Denominator for the per-slot `loading_left` scalar — matches
/// `SimState::settings_loading_time`'s documented in-game default (100).
pub const LOADING_SCALE: f32 = 100.0;

const FIXED_TO_F32: f32 = 65536.0;

/// Fixed-point (16.16) → float, unscaled (just `/65536`).
#[inline]
fn fixed_to_f32(v: i32) -> f32 {
    v as f32 / FIXED_TO_F32
}

/// Fixed-point → "≈unit scale" float: convert then divide by `scale`. Never
/// clamped (module docs: out-of-nominal-range is signal, not a NaN risk —
/// `scale` is always a nonzero compile-time constant).
#[inline]
fn fixed_scaled(v: i32, scale: f32) -> f32 {
    fixed_to_f32(v) / scale
}

/// One weapon slot's ammo, normalized: infinite ammo (`ammo < 0`, the sim's
/// sentinel) maps to `1.0` (read as "fully stocked"); otherwise `ammo /
/// AMMO_SCALE`, clamped to `[0, 1]` so an unusually large pool still reads as
/// "full" rather than overshooting.
fn ammo_norm(ammo: i32) -> f32 {
    if ammo < 0 {
        1.0
    } else {
        (ammo as f32 / AMMO_SCALE).clamp(0.0, 1.0)
    }
}

/// `loading_left` normalized to `[0, 1]` (`0` = ready to fire, `1` = just
/// depleted).
fn loading_norm(loading_left: i32) -> f32 {
    (loading_left as f32 / LOADING_SCALE).clamp(0.0, 1.0)
}

/// This crate's fixed 1v1: the opponent of worm `agent_idx` (design §5,
/// `N_WORMS == 2`).
fn opponent_idx(agent_idx: usize) -> usize {
    debug_assert!(
        agent_idx < 2,
        "observe() is scoped to the symmetric 1v1 (N_WORMS == 2)"
    );
    1 - agent_idx
}

/// `sin(w.aiming_angle)`, `cos(w.aiming_angle)` — reads the precomputed
/// `cossin` table `SimState` already carries (see `sim_core::tables`'s doc:
/// `table[i].x = sin`, `table[(i+32)&0x7f].y = cos`, both 16.16 fixed),
/// converted to float. Same table + index convention `sim::control` uses
/// (`ftoi(aiming_angle) & 0x7f`), so this always indexes in range — no NaN risk.
fn aim_sin_cos(w: &WormState, cossin: &[Vec2; 128]) -> (f32, f32) {
    let idx = (sim_core::fixed::ftoi(w.aiming_angle) & 0x7f) as usize;
    let c = cossin[idx];
    (fixed_to_f32(c.x), fixed_to_f32(c.y))
}

/// Append one worm's self-kinematics block (design §3.1 "Self:" — everything
/// except the weapon slots), `16` scalars.
fn push_self_kinematics(
    out: &mut Vec<f32>,
    w: &WormState,
    cossin: &[Vec2; 128],
    level: (i32, i32),
    settings_health: i32,
) {
    let (lw, lh) = (level.0 as f32, level.1 as f32);
    out.push(fixed_to_f32(w.pos.x) / lw);
    out.push(fixed_to_f32(w.pos.y) / lh);
    out.push(fixed_scaled(w.vel.x, VEL_SCALE));
    out.push(fixed_scaled(w.vel.y, VEL_SCALE));
    let (sin_a, cos_a) = aim_sin_cos(w, cossin);
    out.push(sin_a);
    out.push(cos_a);
    out.push(fixed_scaled(w.aiming_speed, AIM_SPEED_SCALE));
    out.push(if w.direction == 0 { -1.0 } else { 1.0 });
    out.push(w.health as f32 / settings_health as f32);
    out.push(w.lives as f32 / LIVES_SCALE);
    out.push(if w.visible { 1.0 } else { 0.0 });
    out.push(if w.able_to_jump { 1.0 } else { 0.0 });
    out.push(if w.able_to_dig { 1.0 } else { 0.0 });
    out.push(if w.ninjarope.out { 1.0 } else { 0.0 });
    if w.ninjarope.out {
        out.push(fixed_scaled(w.ninjarope.pos.x - w.pos.x, ROPE_SCALE));
        out.push(fixed_scaled(w.ninjarope.pos.y - w.pos.y, ROPE_SCALE));
    } else {
        out.push(0.0);
        out.push(0.0);
    }
}

/// Append one worm's weapon block (design §3.1 "Weapons:"), `3 * NUM_WEAPONS`
/// scalars: a `current_weapon` one-hot, then per-slot `ammo`, then per-slot
/// `loading_left`.
///
/// **Deviation from design §3.1:** the design's "Weapons:" list names `ammo`,
/// `delay_left`, AND `loading_left` per slot; `delay_left` is intentionally
/// OMITTED here. `delay_left` is the short between-shots cooldown, largely
/// redundant with `loading_left` (the dominant reload/ammo-discipline signal
/// design §3.1 calls out) at this obs fidelity, and adding it would widen every
/// slot by one scalar (`OBS_DIM` would grow by `NUM_WEAPONS`). It can be added
/// later behind an `OBS_DIM` bump and a matching layout-table row if reload
/// timing proves under-observed — tracked as a v1.1 obs tweak, not a silent gap.
fn push_weapons(out: &mut Vec<f32>, w: &WormState) {
    for slot in 0..NUM_WEAPONS {
        out.push(if w.current_weapon == slot as i32 {
            1.0
        } else {
            0.0
        });
    }
    for slot in &w.weapons {
        out.push(ammo_norm(slot.ammo));
    }
    for slot in &w.weapons {
        out.push(loading_norm(slot.loading_left));
    }
}

/// Append the opponent's kinematics block (design §3.1 "Opponent:" kinematics
/// half — no weapons), `9` scalars.
fn push_opponent_kinematics(
    out: &mut Vec<f32>,
    w: &WormState,
    cossin: &[Vec2; 128],
    level: (i32, i32),
    settings_health: i32,
) {
    let (lw, lh) = (level.0 as f32, level.1 as f32);
    out.push(fixed_to_f32(w.pos.x) / lw);
    out.push(fixed_to_f32(w.pos.y) / lh);
    out.push(fixed_scaled(w.vel.x, VEL_SCALE));
    out.push(fixed_scaled(w.vel.y, VEL_SCALE));
    let (sin_a, cos_a) = aim_sin_cos(w, cossin);
    out.push(sin_a);
    out.push(cos_a);
    out.push(w.health as f32 / settings_health as f32);
    out.push(w.lives as f32 / LIVES_SCALE);
    out.push(if w.visible { 1.0 } else { 0.0 });
}

/// Append the relative-geometry block (design §3.1 "relative geometry"), `3`
/// scalars: `(opp.pos - self.pos)` normalized per axis, plus Euclidean
/// distance normalized by the level diagonal.
fn push_relative_geometry(out: &mut Vec<f32>, me: &WormState, opp: &WormState, level: (i32, i32)) {
    let (lw, lh) = (level.0 as f32, level.1 as f32);
    let dx_px = sim_core::fixed::ftoi(opp.pos.x.wrapping_sub(me.pos.x)) as f32;
    let dy_px = sim_core::fixed::ftoi(opp.pos.y.wrapping_sub(me.pos.y)) as f32;
    out.push(dx_px / lw);
    out.push(dy_px / lh);
    let diag = (lw * lw + lh * lh).sqrt();
    let dist = (dx_px * dx_px + dy_px * dy_px).sqrt();
    out.push(dist / diag);
}

/// Extract `agent_idx`'s egocentric observation vector from the live `state`
/// (design §3.1). See the module docs for the exact index layout.
pub fn observe(state: &SimState, agent_idx: usize) -> Vec<f32> {
    let opp_idx = opponent_idx(agent_idx);
    let me = &state.worms[agent_idx];
    let opp = &state.worms[opp_idx];
    let level = (state.level.width, state.level.height);

    let mut out = Vec::with_capacity(OBS_DIM);
    push_self_kinematics(&mut out, me, &state.cossin, level, state.settings_health);
    push_weapons(&mut out, me);
    push_opponent_kinematics(&mut out, opp, &state.cossin, level, state.settings_health);
    push_relative_geometry(&mut out, me, opp, level);

    debug_assert_eq!(
        out.len(),
        OBS_DIM,
        "push_* helpers must total exactly OBS_DIM"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{set_worm, two_worm_state as build_state, worm_init};
    use sim_core::fixed::itof;
    use sim_core::vec::Vec2;

    const LEVEL_W: i32 = 100;
    const LEVEL_H: i32 = 200;

    fn base_state() -> SimState {
        let w0 = worm_init(0, Vec2::new(itof(25), itof(50)), 80, 3, true);
        let w1 = worm_init(1, Vec2::new(itof(75), itof(150)), 40, 1, false);
        build_state(LEVEL_W, LEVEL_H, w0, w1)
    }

    /// RED (plan T2): `observe`'s length is exactly `OBS_DIM`, for both agents.
    #[test]
    fn observation_length_is_obs_dim() {
        let state = base_state();
        assert_eq!(observe(&state, 0).len(), OBS_DIM);
        assert_eq!(observe(&state, 1).len(), OBS_DIM);
    }

    /// RED (plan T2, index-pinning): self position/health/lives/visible read
    /// from known `WormState` fields land at their documented indices with the
    /// documented normalization.
    #[test]
    fn self_position_health_lives_visible_are_pinned() {
        let state = base_state();
        let obs = observe(&state, 0);
        assert!((obs[0] - 0.25).abs() < 1e-6, "pos.x/W: {}", obs[0]);
        assert!((obs[1] - 0.25).abs() < 1e-6, "pos.y/H: {}", obs[1]); // 50/200
        assert!(
            (obs[8] - 0.8).abs() < 1e-6,
            "health/settings_health: {}",
            obs[8]
        ); // 80/100
        assert!(
            (obs[9] - 3.0 / LIVES_SCALE).abs() < 1e-6,
            "lives: {}",
            obs[9]
        );
        assert_eq!(obs[10], 1.0, "worm 0 is visible");
    }

    /// RED (plan T2, index-pinning): the opponent block mirrors worm 1's known
    /// fields when observing from agent 0's perspective.
    #[test]
    fn opponent_position_health_visible_are_pinned() {
        let state = base_state();
        let obs = observe(&state, 0);
        assert!((obs[31] - 0.75).abs() < 1e-6, "opp pos.x/W: {}", obs[31]);
        assert!((obs[32] - 0.75).abs() < 1e-6, "opp pos.y/H: {}", obs[32]); // 150/200
        assert!(
            (obs[37] - 0.4).abs() < 1e-6,
            "opp health/settings_health: {}",
            obs[37]
        ); // 40/100
        assert_eq!(obs[39], 0.0, "worm 1 is not visible");
    }

    /// RED (plan T2, index-pinning): velocity, aim sin/cos (read straight off
    /// the state's own `cossin` table, self-consistently), aiming_speed, and
    /// direction all land where documented.
    #[test]
    fn self_velocity_aim_and_direction_are_pinned() {
        let mut state = base_state();
        set_worm(&mut state, 0, |w| {
            w.vel = Vec2::new(itof(3), itof(-6));
            w.aiming_angle = itof(10);
            w.aiming_speed = itof(1);
            w.direction = 1;
        });
        let obs = observe(&state, 0);
        assert!((obs[2] - 3.0 / VEL_SCALE).abs() < 1e-6, "vel.x: {}", obs[2]);
        assert!(
            (obs[3] - (-6.0 / VEL_SCALE)).abs() < 1e-6,
            "vel.y: {}",
            obs[3]
        );
        let idx = (sim_core::fixed::ftoi(itof(10)) & 0x7f) as usize;
        let expected = state.cossin[idx];
        assert!(
            (obs[4] - expected.x as f32 / 65536.0).abs() < 1e-6,
            "sin(aim) must read state.cossin[idx].x: {}",
            obs[4]
        );
        assert!(
            (obs[5] - expected.y as f32 / 65536.0).abs() < 1e-6,
            "cos(aim) must read state.cossin[idx].y: {}",
            obs[5]
        );
        assert!(
            (obs[6] - 1.0 / AIM_SPEED_SCALE).abs() < 1e-6,
            "aiming_speed: {}",
            obs[6]
        );
        assert_eq!(obs[7], 1.0, "direction=1 (right) -> +1.0");

        set_worm(&mut state, 0, |w| w.direction = 0);
        let obs2 = observe(&state, 0);
        assert_eq!(obs2[7], -1.0, "direction=0 (left) -> -1.0");
    }

    /// RED (plan T2, index-pinning): the current-weapon one-hot, per-slot
    /// ammo (including the `-1` infinite-ammo sentinel), and per-slot
    /// `loading_left` all land in the weapon block at documented offsets.
    #[test]
    fn weapon_block_onehot_ammo_and_loading_are_pinned() {
        let mut state = base_state();
        set_worm(&mut state, 0, |w| {
            w.current_weapon = 2;
            w.weapons[0].ammo = -1; // infinite
            w.weapons[1].ammo = 50;
            w.weapons[0].loading_left = 25;
            w.weapons[1].loading_left = 0;
        });
        let obs = observe(&state, 0);
        // one-hot over slots 0..5 at indices 16..21
        for slot in 0..NUM_WEAPONS {
            let expected = if slot == 2 { 1.0 } else { 0.0 };
            assert_eq!(
                obs[16 + slot],
                expected,
                "current_weapon one-hot slot {slot}"
            );
        }
        // ammo at indices 21..26
        assert_eq!(obs[21], 1.0, "infinite ammo (-1) -> 1.0");
        assert!(
            (obs[22] - 0.5).abs() < 1e-6,
            "50/AMMO_SCALE=100 -> 0.5: {}",
            obs[22]
        );
        // loading_left at indices 26..31
        assert!(
            (obs[26] - 0.25).abs() < 1e-6,
            "25/LOADING_SCALE=100 -> 0.25: {}",
            obs[26]
        );
        assert_eq!(obs[27], 0.0, "loading_left=0 -> 0.0 (ready)");
    }

    /// RED (plan T2, index-pinning): relative geometry is `opp - self`,
    /// normalized per axis, plus a normalized Euclidean distance.
    #[test]
    fn relative_geometry_is_pinned() {
        let state = base_state();
        let obs = observe(&state, 0);
        // self px=(25,50), opp px=(75,150) -> dx=50, dy=100
        assert!(
            (obs[40] - 50.0 / LEVEL_W as f32).abs() < 1e-6,
            "rel x: {}",
            obs[40]
        );
        assert!(
            (obs[41] - 100.0 / LEVEL_H as f32).abs() < 1e-6,
            "rel y: {}",
            obs[41]
        );
        let diag = ((LEVEL_W * LEVEL_W + LEVEL_H * LEVEL_H) as f32).sqrt();
        let dist = (50.0f32 * 50.0 + 100.0 * 100.0).sqrt();
        assert!(
            (obs[42] - dist / diag).abs() < 1e-6,
            "distance: {}",
            obs[42]
        );
    }

    /// RED (plan T2): observing from agent 1's perspective swaps self/opponent
    /// — a basic egocentricity check (not literally symmetric values, since
    /// worm 0 and worm 1 differ, but the SELF block must read worm 1's fields).
    #[test]
    fn observe_is_egocentric_per_agent() {
        let state = base_state();
        let obs1 = observe(&state, 1);
        assert!(
            (obs1[0] - 0.75).abs() < 1e-6,
            "agent 1's self pos.x/W: {}",
            obs1[0]
        );
        assert!(
            (obs1[8] - 0.4).abs() < 1e-6,
            "agent 1's self health: {}",
            obs1[8]
        );
        assert_eq!(
            obs1[10], 0.0,
            "agent 1 is not visible, read from its OWN slot now"
        );
        assert!(
            (obs1[31] - 0.25).abs() < 1e-6,
            "agent 1's opponent (worm 0) pos.x/W: {}",
            obs1[31]
        );
    }

    /// RED (plan T2, "NaN-free... döda/osynliga worms -> definierade värden"):
    /// a dead, invisible, out-of-lives, zero-ammo worm still produces an
    /// entirely finite observation vector — no NaN, no +/-inf, for either
    /// perspective.
    #[test]
    fn observe_produces_no_nan_or_inf_for_a_dead_worm() {
        let mut state = base_state();
        set_worm(&mut state, 0, |w| {
            w.health = 0;
            w.lives = 0;
            w.visible = false;
            w.vel = Vec2::new(itof(-2), itof(2));
            for slot in w.weapons.iter_mut() {
                slot.ammo = 0;
                slot.loading_left = 0;
            }
        });
        for agent in [0usize, 1] {
            let obs = observe(&state, agent);
            for (i, &v) in obs.iter().enumerate() {
                assert!(
                    v.is_finite(),
                    "obs[{i}] must be finite for agent {agent}, got {v}"
                );
            }
        }
    }
}
