//! The CPU player: C++ `DumbLieroAI` (`worm.cpp:477-696`), line for line, and the
//! `LocalController` loop that runs it (`localController.cpp:156-164`).
//!
//! # What it is
//!
//! A [`DumbLieroAi`] owns only its own [`Rand`] — C++ `DumbLieroAI` holds a
//! default-constructed `Rand rand` (`worm.hpp:130-134`), i.e. `mt19937(0x1337)` with
//! `last = 0` (`rand.hpp:15-16`), made once per NEW GAME per CPU player (`CreateAi`,
//! `localController.cpp:19-28`) and never reseeded (plan fact 1, T0 P1). It is **not**
//! part of [`SimState`] and is never hashed (C++ keeps `Worm::ai` out of snapshots,
//! `cereal_types.hpp:329`).
//!
//! Each call reads the state (the other worm's position, the worm's current weapon,
//! `pos`, `visible`, `direction`, `aiming_angle`, `ninjarope`, the stale `reacts`, the
//! TC's `ai_params` and the `cossin` table) and turns the worm's control word `cs` into
//! the word the tick applies. **It reads `cs`, never `state.worms[w].control_states`**:
//! the state's word is last tick's, before this tick's key edges, while C++'s AI starts
//! from the post-key word (plan pitfall 9).
//!
//! # The draws of one call, in order (plan fact 3)
//!
//! 1. Fire: `rand(k[fire][kFire])` only when `real_dist < max_dist || !visible`
//!    (`:513-518`); otherwise (visible and out of range) `Release(kFire)`, no draw.
//! 2. Jump: one draw, always (`:524-527`).
//! 3. Change: one draw, always (`:529-532`).
//! 4. Fallback: `rand(16)` only when the scan found no direction and the quadrant arm
//!    is one of the six `+ rand(16)` arms (`:557-593`).
//! 5. When Change is pressed (read again, `:622`): Left, then Right, one draw each; then,
//!    if the rope is out and attached, Up then Down, one draw each (`:625-648`).
//!
//! Every `rand(k)` draws, even for `k` 0 or 1 (both return 0), so `== 0` holds and the
//! toggle happens (plan pitfall 10).
//!
//! # Arithmetic
//!
//! `i32` throughout, with `wrapping_*` where C++ `int` arithmetic could overflow; `Ftoi`
//! is [`ftoi`] (`>> 16`); `VectorLength` is [`vector_length`] (integer square root);
//! `delta /= real_dist` is [`Vec2::div`] (per component, truncating toward zero,
//! `math/rect.hpp:41-45`). No floats, no clock, no hash containers.

use sim_core::fixed::ftoi;
use sim_core::math::vector_length;
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::physics::{RF_DOWN, RF_LEFT, RF_RIGHT};
use crate::state::{ControlState, SimState};

/// Which formula set `max_dist` (`worm.cpp:497-501`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MaxDistArm {
    /// `0 < time_to_explo < 500`: `(time_to_explo - time_to_explo_v / 2) * speed / 130`.
    #[default]
    Explo,
    /// Otherwise: `speed - gravity / 10`.
    Speed,
}

/// The fallback direction arm taken when the `cossin` scan found no match
/// (`worm.cpp:557-593`): `base` is the arm's constant and `drew` whether it added
/// `rand(16)`. The ten arms are the ten distinct `(base, drew)` pairs:
/// `(64, true)`, `(80, true)`, `(80, false)`, `(96, true)`, `(116, false)` for
/// `delta.x > 0`, and `(48, true)`, `(32, true)`, `(48, false)`, `(12, true)`,
/// `(12, false)` for `delta.x <= 0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fallback {
    pub base: i32,
    pub drew: bool,
}

/// Which arm each part of one [`DumbLieroAi::process_traced`] call took. Behaviour-free:
/// for unit tests and the gate ledgers (plan D12), never read by the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiTrace {
    /// The AI ran this call (`false` in a trace [`run_ais_traced`] left untouched).
    pub ran: bool,
    /// The target worm's index (`worm.cpp:480-493`).
    pub target: usize,
    /// `max_dist` after the floor (`:497-506`).
    pub max_dist: i32,
    /// The formula that set `max_dist` before the floor.
    pub max_dist_arm: MaxDistArm,
    /// The `max(…, 90)` floor raised it.
    pub max_dist_floored: bool,
    /// `VectorLength(Ftoi(delta))` (`:510`).
    pub real_dist: i32,
    /// The Fire arm drew (`:513-518`).
    pub fire_drew: bool,
    /// The final `dir` (the scan's match, or the fallback's).
    pub dir: i32,
    /// The fallback arm, when the scan found nothing.
    pub fallback: Option<Fallback>,
    /// Change was pressed after its toggle (`:622`): the Left/Right draw arm ran.
    pub change: bool,
    /// Within the Change arm, the rope was out and attached: the Up/Down draws ran.
    pub rope_attached: bool,
    /// The Left (`[0]`) and Right (`[1]`) `reacts` arms fired (`:680-694`).
    pub reacts_press: [bool; 2],
    /// Raw RNG draws this call made.
    pub draws: u32,
}

/// C++ `DumbLieroAI` (`worm.hpp:130-134`): the CPU player of one worm.
#[derive(Clone)]
pub struct DumbLieroAi {
    /// The AI's own RNG: `Rand::new()`, i.e. `mt19937(0x1337)` with `last = 0`.
    pub rand: Rand,
}

impl Default for DumbLieroAi {
    fn default() -> Self {
        Self::new()
    }
}

/// `SqrVectorLength` (`worm.cpp:475`).
fn sqr_vector_length(x: i32, y: i32) -> i32 {
    x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y))
}

/// `ToggleControlState` (`worm.hpp`): flip one bit of the local word.
fn toggle(cs: &mut ControlState, n: u32) {
    let v = cs.get(n);
    cs.set(n, !v);
}

impl DumbLieroAi {
    /// A fresh AI, as `CreateAi` makes it (`new DumbLieroAI()`).
    pub fn new() -> Self {
        DumbLieroAi { rand: Rand::new() }
    }

    /// `rand(k)` (`rand.hpp:30-32`): always one draw, whatever `k` is.
    fn draw(&mut self, k: i32) -> i32 {
        self.rand.bound(k as u32) as i32
    }

    /// `DumbLieroAI::Process` (`worm.cpp:477-696`). `cs` is the worm's word as the tick
    /// will start it (last tick's post-tick word plus this tick's key edges); returns the
    /// word the tick applies. Reads `state` only.
    pub fn process(&mut self, state: &SimState, worm: usize, cs: ControlState) -> ControlState {
        let mut trace = AiTrace::default();
        self.process_traced(state, worm, cs, &mut trace)
    }

    /// [`process`](Self::process), filling `trace` (behaviour-identical).
    pub fn process_traced(
        &mut self,
        state: &SimState,
        worm: usize,
        cs: ControlState,
        trace: &mut AiTrace,
    ) -> ControlState {
        let draws_before = self.rand.draws();
        let mut cs = cs;
        let me = &state.worms[worm];
        let k = &state.ai_params;
        *trace = AiTrace {
            ran: true,
            ..AiTrace::default()
        };

        // :480-493 the target: the other worm with the smallest squared pixel distance
        // (the first one, or a strictly closer one).
        let mut target: Option<usize> = None;
        let mut min_len = 0i32;
        for (j, w) in state.worms.iter().enumerate() {
            if j != worm {
                let len = sqr_vector_length(
                    ftoi(me.pos.x).wrapping_sub(ftoi(w.pos.x)),
                    ftoi(me.pos.y).wrapping_sub(ftoi(w.pos.y)),
                );
                if target.is_none() || len < min_len {
                    target = Some(j);
                    min_len = len;
                }
            }
        }
        let target = target.expect("DumbLieroAI needs another worm (C++ dereferences it)");
        trace.target = target;

        // :495-506 max_dist from the current weapon.
        let ty = me.weapons[me.current_weapon as usize]
            .ty
            .expect("the current weapon slot holds a weapon");
        let w = &state.weapons[ty as usize];
        let mut max_dist = if w.time_to_explo > 0 && w.time_to_explo < 500 {
            trace.max_dist_arm = MaxDistArm::Explo;
            w.time_to_explo
                .wrapping_sub(w.time_to_explo_v / 2)
                .wrapping_mul(w.speed)
                / 130
        } else {
            trace.max_dist_arm = MaxDistArm::Speed;
            w.speed.wrapping_sub(w.gravity / 10)
        }; // 4D43
        if max_dist < 90 {
            trace.max_dist_floored = true;
            max_dist = 90;
        }
        trace.max_dist = max_dist;

        // :508-511 delta (fixed) and the pixel distance.
        let mut delta: Vec2 = state.worms[target].pos.sub(me.pos);
        let real_dist = vector_length(ftoi(delta.x), ftoi(delta.y));
        trace.real_dist = real_dist;

        // :513-521 Fire: a draw only in range or while invisible.
        if real_dist < max_dist || !me.visible {
            let fire = cs.get(ControlState::FIRE);
            trace.fire_drew = true;
            if self.draw(k[fire as usize][ControlState::FIRE as usize]) == 0 {
                cs.set(ControlState::FIRE, !fire);
            } // 4DE7
        } else if me.visible {
            cs.release(ControlState::FIRE);
        } // 4DFA

        // :523-527 Jump: always one draw.
        let jump = cs.get(ControlState::JUMP);
        if self.draw(k[jump as usize][ControlState::JUMP as usize]) == 0 {
            toggle(&mut cs, ControlState::JUMP);
        }

        // :529-532 Change: always one draw.
        let change = cs.get(ControlState::CHANGE);
        if self.draw(k[change as usize][ControlState::CHANGE as usize]) == 0 {
            toggle(&mut cs, ControlState::CHANGE);
        }

        // :537-541 delta /= real_dist (per component, truncating), or zero.
        if real_dist > 0 {
            delta = delta.div(real_dist);
        } else {
            delta = Vec2::zero();
        } // 4F2F

        // :543-552 the scan over cossin[1..128) (index 128 is never read).
        let mut dir = 1i32;
        while dir < 128 {
            let c = state.cossin[dir as usize];
            if c.x.wrapping_sub(delta.x).wrapping_abs() < 0xC00
                && c.y.wrapping_sub(delta.y).wrapping_abs() < 0xC00
            {
                break;
            }
            dir += 1;
        } // 4F93

        let adelta_x = delta.x.wrapping_abs();
        let adelta_y = delta.y.wrapping_abs();

        // :557-593 the fallback: ten arms, six of which draw rand(16).
        if dir >= 128 {
            let (base, drew) = if delta.x > 0 {
                if delta.y < 0 {
                    if adelta_y > adelta_x {
                        (64, true)
                    } else if adelta_x > adelta_y {
                        (80, true)
                    } else {
                        (80, false)
                    }
                } else if adelta_x > adelta_y {
                    // deltaY >= 0
                    (96, true)
                } else {
                    (116, false)
                }
            } else if delta.y < 0 {
                if adelta_y > adelta_x {
                    (48, true)
                } else if adelta_x > adelta_y {
                    (32, true)
                } else {
                    (48, false) // "This was 56, but that seems wrong"
                }
            } else if adelta_x > adelta_y {
                // deltaX <= 0 && deltaY >= 0
                (12, true)
            } else {
                (12, false)
            };
            dir = if drew { base + self.draw(16) } else { base };
            trace.fallback = Some(Fallback { base, drew });
        } // 50FD
        trace.dir = dir;

        // :622 Change is read again, after its toggle.
        if cs.get(ControlState::CHANGE) {
            trace.change = true;
            // :625-631 Left, then Right: one draw each.
            let left = cs.get(ControlState::LEFT);
            if self.draw(k[left as usize][ControlState::LEFT as usize]) == 0 {
                toggle(&mut cs, ControlState::LEFT);
            }
            let right = cs.get(ControlState::RIGHT);
            if self.draw(k[right as usize][ControlState::RIGHT as usize]) == 0 {
                toggle(&mut cs, ControlState::RIGHT);
            }

            if me.ninjarope.out && me.ninjarope.attached {
                // :633-644 l_525F: Up, then Down.
                trace.rope_attached = true;
                let up = cs.get(ControlState::UP);
                if self.draw(k[up as usize][ControlState::UP as usize]) == 0 {
                    toggle(&mut cs, ControlState::UP);
                }
                let down = cs.get(ControlState::DOWN);
                if self.draw(k[down as usize][ControlState::DOWN as usize]) == 0 {
                    toggle(&mut cs, ControlState::DOWN);
                }
            } else {
                // :645-648 l_52D2.
                cs.release(ControlState::UP);
                cs.release(ControlState::DOWN);
            } // 52F8
        } else {
            // :652-659 walk toward the target when out of range.
            if real_dist > max_dist {
                cs.set(ControlState::RIGHT, delta.x > 0);
                cs.set(ControlState::LEFT, delta.x <= 0);
            } else {
                // 5347
                cs.release(ControlState::RIGHT);
                cs.release(ControlState::LEFT);
            }

            // :661-677 aim toward `dir`.
            let angle = ftoi(me.aiming_angle);
            if me.direction != 0 {
                if dir < 64 {
                    cs.press(ControlState::LEFT);
                }
                cs.set(ControlState::UP, dir + 1 < angle); // 5369
                cs.set(ControlState::DOWN, dir - 1 > angle); // 5379
            } else {
                if dir > 64 {
                    cs.press(ControlState::RIGHT);
                }
                cs.set(ControlState::UP, dir - 1 > angle); // 53C6
                cs.set(ControlState::DOWN, dir + 1 < angle); // 53E8
            }

            // :680-694 the `reacts` arms (the stale values, plan D5).
            if cs.get(ControlState::LEFT) && me.reacts[RF_RIGHT] != 0 {
                trace.reacts_press[0] = true;
                if me.reacts[RF_DOWN] > 0 {
                    cs.press(ControlState::RIGHT);
                } else {
                    cs.press(ControlState::JUMP);
                }
            } // 5454
            if cs.get(ControlState::RIGHT) && me.reacts[RF_LEFT] != 0 {
                trace.reacts_press[1] = true;
                if me.reacts[RF_DOWN] > 0 {
                    cs.press(ControlState::LEFT);
                } else {
                    cs.press(ControlState::JUMP);
                }
            } // 549E
        }

        trace.draws = (self.rand.draws() - draws_before) as u32;
        cs
    }
}

/// The order `LocalController::Process` runs the AIs in (`localController.cpp:156-164`):
/// `kPhase = cycles % 2`, then worm `(i + kPhase) % 2` for `i` in `0..2`. Worm 0 first
/// on even `cycles`, worm 1 first on odd.
pub fn ai_order(cycles: i32) -> [usize; 2] {
    let phase = cycles.rem_euclid(2) as usize;
    [phase, 1 - phase]
}

/// `LocalController::Process`'s AI loop (`localController.cpp:156-164`): in
/// [`ai_order`], each worm with an AI turns `inputs[worm]` into the word the tick applies.
/// A `None` leaves that worm's word unchanged.
pub fn run_ais(
    ais: &mut [Option<DumbLieroAi>; 2],
    state: &SimState,
    inputs: &mut [ControlState; 2],
) {
    let mut traces = [AiTrace::default(); 2];
    run_ais_traced(ais, state, inputs, &mut traces);
}

/// [`run_ais`], filling `traces[worm]` for each worm whose AI ran (the others keep their
/// value; pass defaults to see `ran == false` for them).
pub fn run_ais_traced(
    ais: &mut [Option<DumbLieroAi>; 2],
    state: &SimState,
    inputs: &mut [ControlState; 2],
    traces: &mut [AiTrace; 2],
) {
    for w in ai_order(state.cycles) {
        if let Some(ai) = ais[w].as_mut() {
            inputs[w] = ai.process_traced(state, w, inputs[w], &mut traces[w]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::ControlConsts;
    use crate::physics::{PhysicsConsts, RF_UP};
    use crate::state::{WeaponInit, WormInit, NUM_WEAPONS};
    use assets::level::LevelData;
    use assets::object::Weapon;
    use assets::sprite::SpriteSet;
    use sim_core::fixed::itof;

    /// `rand(k) == 0` on (practically) no draw of the fixed AI stream: `bound(2^31 - 1)`
    /// is 0 only for a raw value below 2.
    const NEVER: i32 = i32::MAX;
    /// `rand(1) == 0` on every draw (the TC's `jump on = 1`, plan fact 3).
    const ALWAYS: i32 = 1;

    // The weapons the tests pick through `weapons[0].ty` (plan fact 4's arms).
    const W_EXPLO: i32 = 0; // (100 - 20 / 2) * 200 / 130 = 138
    const W_SPEED: i32 = 1; // time_to_explo 0: 150 - 300 / 10 = 120
    const W_SLOW_EXPLO: i32 = 2; // time_to_explo 500: 1000 - 0 / 10 = 1000
    const W_WEAK: i32 = 3; // 50 - 100 / 10 = 40, floored to 90

    fn weapon(time_to_explo: i32, time_to_explo_v: i32, speed: i32, gravity: i32) -> Weapon {
        Weapon {
            time_to_explo,
            time_to_explo_v,
            speed,
            gravity,
            ..Default::default()
        }
    }

    /// `n` visible worms on an empty level, all at pixel (200, 200), holding `W_EXPLO`,
    /// with every `ai_params` entry `NEVER`. The AI reads no level pixel.
    fn state_n(n: i32) -> SimState {
        let level = LevelData {
            width: 64,
            height: 64,
            material_id: vec![0u8; 64 * 64],
            palette: None,
            display: None,
        };
        let inits: Vec<WormInit> = (0..n)
            .map(|index| WormInit {
                index,
                health: 100,
                lives: 5,
                stats_x: 0,
                weapons: [WeaponInit {
                    ty: Some(W_EXPLO),
                    ammo: 1,
                }; NUM_WEAPONS],
                start_pos: Vec2::new(itof(200), itof(200)),
                visible: true,
            })
            .collect();
        let mut state = SimState::new(
            &level,
            &inits,
            1,
            &[0u8; 256],
            vec![
                weapon(100, 20, 200, 0),
                weapon(0, 0, 150, 300),
                weapon(500, 0, 1000, 0),
                weapon(0, 0, 50, 100),
            ],
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
        state.ai_params = [[NEVER; 7]; 2];
        state
    }

    fn state() -> SimState {
        state_n(2)
    }

    /// Worm 1 at worm 0's position plus `d` (16.16 fixed), so `delta == d` for worm 0.
    fn place_target(state: &mut SimState, d: Vec2) {
        state.worms[1].pos = state.worms[0].pos.add(d);
    }

    /// The openliero TC's odds (tc.cfg:85-118), `[off; 7]` then `[on; 7]`.
    const TC_PARAMS: [[i32; 7]; 2] = [
        [120, 120, 50, 50, 80, 300, 400],
        [20, 20, 20, 20, 80, 60, 1],
    ];

    fn word(bits: &[u32]) -> ControlState {
        let mut cs = ControlState::new();
        for &b in bits {
            cs.press(b);
        }
        cs
    }

    fn run(state: &SimState, cs: ControlState) -> (ControlState, AiTrace, DumbLieroAi) {
        let mut ai = DumbLieroAi::new();
        let mut trace = AiTrace::default();
        let out = ai.process_traced(state, 0, cs, &mut trace);
        assert_eq!(
            u64::from(trace.draws),
            ai.rand.draws(),
            "trace.draws == the RNG's count"
        );
        (out, trace, ai)
    }

    /// Half a pixel, in 16.16.
    const HALF: i32 = 0x8000;
    /// A tenth of a pixel (rounded), in 16.16.
    const TENTH: i32 = 6554;

    // ---- the fresh AI (plan fact 1, T0 P1) -----------------------------------------

    #[test]
    fn a_fresh_ai_is_mt19937_0x1337_and_its_first_tick_ends_at_2af09813() {
        // DumbLieroAI's default-constructed `Rand` (worm.hpp:130-134, rand.hpp:15-16):
        // last 0, then the mt19937(0x1337) stream (golden/rng.txt: 1699724873,
        // 2804711513, 720410643).
        let mut ai = DumbLieroAi::new();
        assert_eq!((ai.rand.last(), ai.rand.draws()), (0, 0));
        let mut fresh = Rand::new();
        for want in [0x654f_be49u32, 0xa72c_8059, 0x2af0_9813] {
            assert_eq!(ai.rand.next_u32(), want);
            assert_eq!(fresh.next_u32(), want);
        }

        // T0 P1: the CPU's first match tick (invisible, both worms at (0, 0), the
        // released word Finalize leaves) draws Fire, Jump and Change — three draws —
        // and C++ then shows `last = 2af09813`.
        let mut s = state();
        s.ai_params = TC_PARAMS;
        for w in s.worms.iter_mut() {
            w.visible = false;
            w.pos = Vec2::zero();
        }
        let (_, trace, ai) = run(&s, ControlState::new());
        assert_eq!(trace.draws, 3);
        assert_eq!(ai.rand.last(), 0x2af0_9813);
        assert_eq!(
            (trace.real_dist, trace.dir, trace.fallback),
            (
                0,
                12,
                Some(Fallback {
                    base: 12,
                    drew: false
                })
            )
        );
    }

    // ---- the target (worm.cpp:480-493) -------------------------------------------

    #[test]
    fn the_target_is_the_first_nearest_other_worm() {
        let mut s = state_n(3);
        s.worms[1].pos = Vec2::new(itof(300), itof(200)); // 100 px
        s.worms[2].pos = Vec2::new(itof(100), itof(200)); // 100 px: a tie, the first wins
        assert_eq!(run(&s, ControlState::new()).1.target, 1);
        s.worms[2].pos = Vec2::new(itof(150), itof(200)); // 50 px: strictly closer
        assert_eq!(run(&s, ControlState::new()).1.target, 2);
        s.worms[1].pos = Vec2::new(itof(200), itof(200)); // 0 px
        assert_eq!(run(&s, ControlState::new()).1.target, 1);
        // With two worms the target is always the other one, at any distance.
        let s = state();
        assert_eq!(run(&s, ControlState::new()).1.target, 1);
    }

    // ---- max_dist (worm.cpp:495-506) ---------------------------------------------

    #[test]
    fn max_dist_takes_each_arm_and_the_floor() {
        for (ty, want, arm, floored) in [
            (W_EXPLO, 138, MaxDistArm::Explo, false),
            (W_SPEED, 120, MaxDistArm::Speed, false),
            (W_SLOW_EXPLO, 1000, MaxDistArm::Speed, false),
            (W_WEAK, 90, MaxDistArm::Speed, true),
        ] {
            let mut s = state();
            s.worms[0].weapons[0].ty = Some(ty);
            let t = run(&s, ControlState::new()).1;
            assert_eq!(
                (t.max_dist, t.max_dist_arm, t.max_dist_floored),
                (want, arm, floored),
                "weapon {ty}"
            );
        }
    }

    #[test]
    fn max_dist_reads_the_current_weapon_slot() {
        let mut s = state();
        s.worms[0].weapons[3].ty = Some(W_SLOW_EXPLO);
        s.worms[0].current_weapon = 3;
        assert_eq!(run(&s, ControlState::new()).1.max_dist, 1000);
    }

    // ---- Fire (worm.cpp:513-521) -------------------------------------------------

    #[test]
    fn fire_draws_in_range_and_toggles_only_on_a_zero() {
        // In range (0 < 138): one draw with k[pressed][kFire]; ALWAYS toggles, NEVER not.
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(50), 0));
        s.ai_params[0][ControlState::FIRE as usize] = ALWAYS;
        let (out, t, _) = run(&s, ControlState::new());
        assert!(
            t.fire_drew && out.get(ControlState::FIRE),
            "off -> k[0] = 1 -> pressed"
        );
        let (out, t, _) = run(&s, word(&[ControlState::FIRE]));
        assert!(
            t.fire_drew && out.get(ControlState::FIRE),
            "on -> k[1] = NEVER -> kept"
        );
        s.ai_params[1][ControlState::FIRE as usize] = ALWAYS;
        let (out, _, _) = run(&s, word(&[ControlState::FIRE]));
        assert!(!out.get(ControlState::FIRE), "on -> k[1] = 1 -> released");
        // Fire, Jump, Change: three draws (the scan matches at 50 px: no fallback draw).
        assert_eq!((t.draws, t.fallback), (3, None));
    }

    #[test]
    fn fire_out_of_range_and_visible_releases_without_a_draw() {
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(500), 0)); // 500 > 138
        s.ai_params = [[ALWAYS; 7]; 2]; // would toggle Fire if it drew
        s.ai_params[1][ControlState::CHANGE as usize] = NEVER;
        s.ai_params[0][ControlState::CHANGE as usize] = NEVER;
        let (out, t, _) = run(&s, word(&[ControlState::FIRE]));
        assert!(
            !t.fire_drew && !out.get(ControlState::FIRE),
            "Release(kFire), no draw"
        );
        assert_eq!(t.draws, 2, "Jump and Change only");
    }

    #[test]
    fn fire_draws_out_of_range_while_invisible() {
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(500), 0));
        s.worms[0].visible = false;
        s.ai_params[1][ControlState::FIRE as usize] = NEVER;
        let (out, t, _) = run(&s, word(&[ControlState::FIRE]));
        assert!(
            t.fire_drew && out.get(ControlState::FIRE),
            "drew, kept (NEVER)"
        );
        assert_eq!(t.draws, 3);
    }

    // ---- Jump and Change (worm.cpp:523-532) --------------------------------------

    #[test]
    fn a_pressed_jump_is_always_released_by_the_tcs_rand_1() {
        // jump on = 1: rand(1) is always 0 (it still draws), so a pressed Jump toggles off.
        let mut s = state();
        s.ai_params = TC_PARAMS;
        place_target(&mut s, Vec2::new(itof(500), 0)); // out of range: no Fire draw
        for _ in 0..5 {
            let (out, t, ai) = run(&s, word(&[ControlState::JUMP]));
            assert!(!out.get(ControlState::JUMP));
            // Jump then Change drew, whatever Change did.
            let mut r = Rand::new();
            r.bound(1);
            r.bound(300);
            if t.change {
                r.bound(20);
                r.bound(20);
            }
            assert_eq!(ai.rand.last(), r.last(), "rand(1) then rand(300), in order");
        }
    }

    #[test]
    fn jump_and_change_draw_once_each_whatever_they_do() {
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(500), 0));
        for (jump, change) in [(NEVER, NEVER), (ALWAYS, NEVER)] {
            s.ai_params[0][ControlState::JUMP as usize] = jump;
            s.ai_params[0][ControlState::CHANGE as usize] = change;
            let (out, t, _) = run(&s, ControlState::new());
            assert_eq!(out.get(ControlState::JUMP), jump == ALWAYS);
            assert_eq!(t.draws, 2);
        }
    }

    // ---- the scan (worm.cpp:537-552) ---------------------------------------------

    /// Worm 1 `r` pixels from worm 0 along `cossin[i]`.
    fn place_along(state: &mut SimState, i: usize, r: i32) {
        let c = state.cossin[i];
        place_target(state, Vec2::new(c.x.wrapping_mul(r), c.y.wrapping_mul(r)));
    }

    #[test]
    fn the_scan_finds_the_first_cossin_entry_within_0xc00() {
        // cossin[0] is (0, 65536) and the scan starts at 1; adjacent entries are ~0xC91
        // apart, so two can both be within 0xC00 on both axes, and the FIRST wins.
        let mut s = state();
        place_along(&mut s, 37, 1000); // delta == cossin[37] exactly; only 37 matches
        let t = run(&s, ControlState::new()).1;
        assert_eq!((t.real_dist, t.dir, t.fallback), (1000, 37, None));
        place_along(&mut s, 37, 100); // real_dist 101 shrinks delta: 36 and 37 match
        let t = run(&s, ControlState::new()).1;
        assert_eq!((t.real_dist, t.dir, t.fallback), (101, 36, None));
        place_along(&mut s, 90, 1000); // real_dist 999: 89 and 90 match
        let t = run(&s, ControlState::new()).1;
        assert_eq!((t.real_dist, t.dir, t.fallback), (999, 89, None));
        // Straight right is cossin[96]; straight left cossin[32].
        place_target(&mut s, Vec2::new(itof(500), 0));
        assert_eq!(run(&s, ControlState::new()).1.dir, 96);
        place_target(&mut s, Vec2::new(itof(-500), 0));
        assert_eq!(run(&s, ControlState::new()).1.dir, 32);
    }

    #[test]
    fn a_zero_distance_lands_on_dir_12_without_a_draw() {
        let s = state(); // both at (200, 200)
        let t = run(&s, ControlState::new()).1;
        assert_eq!((t.real_dist, t.dir), (0, 12));
        assert_eq!(
            t.fallback,
            Some(Fallback {
                base: 12,
                drew: false
            })
        );
        assert_eq!(
            t.draws, 3,
            "Fire (in range), Jump, Change; the fallback draws nothing"
        );
    }

    // ---- the ten fallback arms (worm.cpp:557-593) --------------------------------

    #[test]
    fn each_fallback_arm_draws_rand_16_exactly_where_cpp_does() {
        // Sub-pixel deltas with a pixel distance of 1 keep `delta` far off the unit
        // circle (|delta| < 61191 or > 69881), so no cossin entry is within 0xC00 on both
        // axes and the fallback runs.
        let p = |x: i32, y: i32| Vec2::new(x, y);
        let cases = [
            (p(TENTH, -HALF), 64, true),              // x > 0, y < 0, |y| > |x|
            (p(HALF, -TENTH), 80, true),              // x > 0, y < 0, |x| > |y|
            (p(HALF, -HALF), 80, false),              // x > 0, y < 0, equal
            (p(itof(1) + HALF, 2 * TENTH), 96, true), // x > 0, y >= 0, |x| > |y|
            (p(itof(1) + TENTH, itof(1) + HALF), 116, false), // x > 0, y >= 0, |x| <= |y|
            (p(-TENTH, -HALF), 48, true),             // x <= 0, y < 0, |y| > |x|
            (p(-HALF, -TENTH), 32, true),             // x <= 0, y < 0, |x| > |y|
            (p(-HALF, -HALF), 48, false),             // x <= 0, y < 0, equal
            (p(-HALF, TENTH), 12, true),              // x <= 0, y >= 0, |x| > |y|
            (p(-TENTH, HALF), 12, false),             // x <= 0, y >= 0, |x| <= |y|
        ];
        for (d, base, drew) in cases {
            let mut s = state();
            place_target(&mut s, d);
            let (_, t, ai) = run(&s, ControlState::new());
            assert!(t.real_dist >= 1, "{d:?}: a non-zero distance");
            assert_eq!(t.fallback, Some(Fallback { base, drew }), "{d:?}");
            // Fire (in range), Jump, Change, then rand(16) iff the arm draws.
            let mut r = Rand::new();
            r.bound(NEVER as u32);
            r.bound(NEVER as u32);
            r.bound(NEVER as u32);
            let want_dir = if drew {
                base + r.bound(16) as i32
            } else {
                base
            };
            assert_eq!(t.dir, want_dir, "{d:?}");
            assert_eq!(t.draws, 3 + u32::from(drew), "{d:?}");
            assert_eq!(ai.rand.last(), r.last(), "{d:?}");
        }
    }

    // ---- the Change arm (worm.cpp:622-649) ----------------------------------------

    #[test]
    fn change_pressed_draws_left_then_right_and_releases_up_down_without_a_rope() {
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(50), 0));
        s.ai_params[0][ControlState::LEFT as usize] = 7;
        s.ai_params[0][ControlState::RIGHT as usize] = 11;
        let cs = word(&[ControlState::CHANGE, ControlState::UP, ControlState::DOWN]);
        let (out, t, ai) = run(&s, cs);
        assert!(t.change && !t.rope_attached);
        assert!(
            !out.get(ControlState::UP) && !out.get(ControlState::DOWN),
            "released"
        );
        let mut r = Rand::new();
        for k in [NEVER, NEVER, NEVER] {
            r.bound(k as u32); // Fire, Jump, Change (kept pressed)
        }
        let l = r.bound(7);
        let rr = r.bound(11);
        assert_eq!(out.get(ControlState::LEFT), l == 0);
        assert_eq!(out.get(ControlState::RIGHT), rr == 0);
        assert_eq!(
            (t.draws, ai.rand.last()),
            (5, r.last()),
            "Left then Right, one draw each"
        );
    }

    #[test]
    fn change_pressed_with_an_attached_rope_draws_up_then_down() {
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(50), 0));
        s.worms[0].ninjarope.out = true;
        s.worms[0].ninjarope.attached = true;
        s.ai_params = [[ALWAYS; 7]; 2];
        s.ai_params[1][ControlState::CHANGE as usize] = NEVER; // Change stays pressed
        let cs = word(&[ControlState::CHANGE, ControlState::UP]);
        let (out, t, _) = run(&s, cs);
        assert!(t.change && t.rope_attached);
        assert_eq!(t.draws, 7, "Fire, Jump, Change, Left, Right, Up, Down");
        assert!(
            out.get(ControlState::LEFT) && out.get(ControlState::RIGHT),
            "toggled on"
        );
        assert!(
            !out.get(ControlState::UP) && out.get(ControlState::DOWN),
            "Up off, Down on"
        );

        // Out but not attached: Up and Down released, no draws.
        s.worms[0].ninjarope.attached = false;
        let (out, t, _) = run(&s, cs);
        assert!(t.change && !t.rope_attached);
        assert_eq!(t.draws, 5);
        assert!(!out.get(ControlState::UP) && !out.get(ControlState::DOWN));
    }

    #[test]
    fn change_is_read_after_its_toggle() {
        // Change pressed before, toggled off by the draw: the walk arm runs instead.
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(50), 0));
        s.ai_params[1][ControlState::CHANGE as usize] = ALWAYS;
        let (out, t, _) = run(&s, word(&[ControlState::CHANGE]));
        assert!(!t.change && !out.get(ControlState::CHANGE));
        assert_eq!(t.draws, 3);
        // Not pressed, toggled on: the Change arm runs.
        s.ai_params[0][ControlState::CHANGE as usize] = ALWAYS;
        let (out, t, _) = run(&s, ControlState::new());
        assert!(t.change && out.get(ControlState::CHANGE));
        assert_eq!(t.draws, 5);
    }

    // ---- walk and aim (worm.cpp:652-677) -----------------------------------------

    #[test]
    fn out_of_range_walks_toward_the_target_and_in_range_stops() {
        // Facing 0 with a target on the left (dir 32 < 64): the aim presses nothing
        // sideways, so the walk's word shows.
        let mut s = state();
        s.worms[0].direction = 0;
        s.worms[0].aiming_angle = itof(64);
        place_target(&mut s, Vec2::new(itof(-500), 0)); // 500 > 138
        let (out, t, _) = run(&s, word(&[ControlState::RIGHT]));
        assert_eq!((t.real_dist, t.dir), (500, 32));
        assert!(
            out.get(ControlState::LEFT) && !out.get(ControlState::RIGHT),
            "delta.x <= 0"
        );
        // Facing 1 with a target on the right (dir 96 >= 64): likewise.
        s.worms[0].direction = 1;
        place_target(&mut s, Vec2::new(itof(500), 0));
        let (out, t, _) = run(&s, word(&[ControlState::LEFT]));
        assert_eq!(t.dir, 96);
        assert!(
            out.get(ControlState::RIGHT) && !out.get(ControlState::LEFT),
            "delta.x > 0"
        );
        // In range: both released.
        place_target(&mut s, Vec2::new(itof(50), 0));
        let (out, t, _) = run(&s, word(&[ControlState::LEFT, ControlState::RIGHT]));
        assert!(t.real_dist < t.max_dist);
        assert!(!out.get(ControlState::LEFT) && !out.get(ControlState::RIGHT));
    }

    #[test]
    fn the_aim_follows_direction_dir_and_the_aiming_angle() {
        // dir 37 (the scan); facing 1: dir < 64 presses Left; Up iff dir + 1 < angle,
        // Down iff dir - 1 > angle. Facing 0: Right only above 64; Up iff dir - 1 >
        // angle, Down iff dir + 1 < angle. In range (500 < 1000), so the walk releases
        // Left and Right first and only the aim presses them.
        let mut s = state();
        s.worms[0].weapons[0].ty = Some(W_SLOW_EXPLO);
        place_along(&mut s, 37, 500);
        for (direction, angle, left, up, down) in [
            (1, 50, true, true, false),
            (1, 20, true, false, true),
            (1, 37, true, false, false),
            (0, 50, false, false, true),
            (0, 20, false, true, false),
            (0, 37, false, false, false),
        ] {
            s.worms[0].direction = direction;
            s.worms[0].aiming_angle = itof(angle);
            let (out, t, _) = run(&s, ControlState::new());
            assert_eq!(t.dir, 37);
            assert_eq!(
                out.get(ControlState::LEFT),
                left,
                "dir 1 presses Left below 64"
            );
            assert!(!out.get(ControlState::RIGHT));
            assert_eq!(
                (out.get(ControlState::UP), out.get(ControlState::DOWN)),
                (up, down),
                "{direction} {angle}"
            );
        }
        // dir 89: facing 0 presses Right; facing 1 presses nothing sideways.
        place_along(&mut s, 90, 500);
        s.worms[0].aiming_angle = itof(89);
        s.worms[0].direction = 0;
        let (out, t, _) = run(&s, ControlState::new());
        assert_eq!(t.dir, 89);
        assert!(out.get(ControlState::RIGHT) && !out.get(ControlState::LEFT));
        s.worms[0].direction = 1;
        let (out, _, _) = run(&s, ControlState::new());
        assert!(!out.get(ControlState::RIGHT) && !out.get(ControlState::LEFT));
    }

    // ---- the reacts arms (worm.cpp:680-694) --------------------------------------

    #[test]
    fn the_reacts_arms_press_right_or_jump_and_left_or_jump() {
        let mut s = state();
        s.worms[0].weapons[0].ty = Some(W_SLOW_EXPLO); // in range: only the aim presses
                                                       // Left pressed (facing 1, dir 37) with reacts[Right] != 0: Right if
                                                       // reacts[Down] > 0, else Jump (a negative Down, like P2's 0xBEBEBEBE, is Jump).
        place_along(&mut s, 37, 500);
        s.worms[0].direction = 1;
        s.worms[0].aiming_angle = itof(37);
        for (down, right, jump) in [(1, true, false), (0, false, true), (-5, false, true)] {
            s.worms[0].reacts = [down, 0, 0, 3];
            let (out, t, _) = run(&s, ControlState::new());
            assert_eq!(t.reacts_press, [true, false], "down {down}");
            assert!(out.get(ControlState::LEFT));
            assert_eq!(
                (out.get(ControlState::RIGHT), out.get(ControlState::JUMP)),
                (right, jump)
            );
        }
        // A negative reacts[Right] is truthy too (C++ `if (reacts[kRfRight])`).
        s.worms[0].reacts = [0, 0, 0, -1];
        assert_eq!(run(&s, ControlState::new()).1.reacts_press, [true, false]);
        // Only Up set: no arm.
        s.worms[0].reacts = [0, 0, 4, 0];
        assert_eq!(s.worms[0].reacts[RF_UP], 4);
        let (out, t, _) = run(&s, ControlState::new());
        assert_eq!(t.reacts_press, [false, false]);
        assert!(!out.get(ControlState::JUMP));

        // The mirror: Right pressed (facing 0, dir 89) with reacts[Left] != 0.
        place_along(&mut s, 90, 500);
        s.worms[0].direction = 0;
        s.worms[0].aiming_angle = itof(89);
        for (down, left, jump) in [(2, true, false), (0, false, true)] {
            s.worms[0].reacts = [down, 6, 0, 0];
            let (out, t, _) = run(&s, ControlState::new());
            assert_eq!(t.reacts_press, [false, true], "down {down}");
            assert!(out.get(ControlState::RIGHT));
            assert_eq!(
                (out.get(ControlState::LEFT), out.get(ControlState::JUMP)),
                (left, jump)
            );
        }
    }

    #[test]
    fn the_ai_reads_cs_not_the_states_word() {
        // Plan pitfall 9: `state.worms[w].control_states` is last tick's word; the AI
        // starts from `cs`. Everything held in the state, nothing in `cs`: the result
        // follows `cs` (Change off, so no Change arm; Fire off -> k[0] = ALWAYS -> on).
        let mut s = state();
        place_target(&mut s, Vec2::new(itof(50), 0));
        s.ai_params[0][ControlState::FIRE as usize] = ALWAYS;
        s.worms[0].control_states = ControlState::unpack(0x7f);
        let (out, t, _) = run(&s, ControlState::new());
        assert!(!t.change && out.get(ControlState::FIRE) && !out.get(ControlState::JUMP));
    }

    // ---- run_ais (localController.cpp:156-164) ------------------------------------

    #[test]
    fn ai_order_alternates_with_cycles() {
        assert_eq!(ai_order(0), [0, 1]);
        assert_eq!(ai_order(1), [1, 0]);
        assert_eq!(ai_order(2), [0, 1]);
        assert_eq!(ai_order(1001), [1, 0]);
    }

    #[test]
    fn run_ais_runs_each_ai_on_its_own_word_and_skips_none() {
        let mut s = state();
        s.ai_params = TC_PARAMS;
        for w in s.worms.iter_mut() {
            w.visible = false;
        }
        let human = word(&[ControlState::UP, ControlState::FIRE]);
        let mut ais = [None, Some(DumbLieroAi::new())];
        let mut inputs = [human, ControlState::new()];
        let mut traces = [AiTrace::default(); 2];
        run_ais_traced(&mut ais, &s, &mut inputs, &mut traces);
        assert_eq!(inputs[0], human, "no AI: the word is untouched");
        assert!(!traces[0].ran && traces[1].ran);
        assert_eq!(traces[1].target, 0);

        // Two AIs carry identical, separate streams (T0 P1 `s_both`): same state shape
        // mirrored, same draws, same `last` — one shared RNG would put one a draw ahead.
        let mut ais = [Some(DumbLieroAi::new()), Some(DumbLieroAi::new())];
        let mut inputs = [ControlState::new(); 2];
        for cycles in 0..4 {
            s.cycles = cycles;
            run_ais(&mut ais, &s, &mut inputs);
            let [a, b] = &ais;
            let (a, b) = (a.as_ref().unwrap(), b.as_ref().unwrap());
            assert_eq!(
                (a.rand.draws(), a.rand.last()),
                (b.rand.draws(), b.rand.last())
            );
            assert_eq!(inputs[0], inputs[1]);
        }
        let first = ais[0].as_ref().unwrap();
        assert!(first.rand.draws() >= 12, "three draws per tick at least");
    }

    #[test]
    fn run_ais_equals_process_in_ai_order() {
        let mut s = state();
        s.ai_params = TC_PARAMS;
        place_target(&mut s, Vec2::new(itof(40), itof(-30)));
        s.cycles = 7;
        let mut ais = [Some(DumbLieroAi::new()), Some(DumbLieroAi::new())];
        let mut inputs = [word(&[ControlState::FIRE]), word(&[ControlState::CHANGE])];
        let mut want = inputs;
        let (mut a0, mut a1) = (DumbLieroAi::new(), DumbLieroAi::new());
        want[1] = a1.process(&s, 1, want[1]);
        want[0] = a0.process(&s, 0, want[0]);
        run_ais(&mut ais, &s, &mut inputs);
        assert_eq!(inputs, want);
        assert_eq!(ais[0].as_ref().unwrap().rand.last(), a0.rand.last());
        assert_eq!(ais[1].as_ref().unwrap().rand.last(), a1.rand.last());
    }
}
