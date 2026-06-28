//! Deterministic state hashing, mirroring the C++ `stateHash.hpp`.
//!
//! These two functions reproduce `HashGameState` / `HashGameComponents`
//! *bit-for-bit*. They are the determinism oracle for Step 2: the Rust tick-0
//! state must hash to the same `u32` the C++ engine produces for the same state.
//!
//! Every arithmetic step uses `wrapping_*` because the C++ relies on `uint32_t`
//! overflow, and every signed field is reinterpreted with `as u32`
//! (two's-complement reinterpret == C++ `static_cast<uint32_t>(int)`). Field
//! order, the `*31`/`*33^` mixers, the empty-pool seed of `1`, and the
//! `if (type)` conditional pushes are all load-bearing — see `stateHash.hpp`.

use crate::state::SimState;

/// Number of players whose per-worm component hash is reported. Mirrors C++
/// `kNumPlayers` (`stateHash.hpp:116`).
pub const NUM_PLAYERS: usize = 2;

/// Per-subsystem hashes, for diagnostic output on a desync. Mirrors the C++
/// `ComponentHashes` struct (`stateHash.hpp:118`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ComponentHashes {
    pub rng: u32,
    pub level: u32,
    pub worms: [u32; NUM_PLAYERS],
    pub bobjects: u32,
    pub bonuses: u32,
    pub sobjects: u32,
    pub nobjects: u32,
    pub wobjects: u32,
}

/// Folds the level material map into `h` with the `h = h*33 ^ byte` mixer
/// (C++ `stateHash.hpp:21-23` / `136-138`). Shared by master + component hash.
#[inline]
fn fold_level(mut h: u32, state: &SimState) -> u32 {
    let count = (state.level.width as i64 * state.level.height as i64) as usize;
    for &byte in &state.level.material_id[..count] {
        h = h.wrapping_mul(33) ^ (byte as u32);
    }
    h
}

/// Comprehensive hash of all simulation-relevant state. Mirrors C++
/// `HashGameState` (`stateHash.hpp:15-113`) line for line.
pub fn hash_game_state(state: &SimState) -> u32 {
    let mut h: u32 = 1;

    h = h.wrapping_mul(31).wrapping_add(state.rand.last());
    h = h.wrapping_mul(31).wrapping_add(state.cycles as u32);

    h = fold_level(h, state);

    for w in &state.worms {
        h = h.wrapping_mul(31).wrapping_add(w.pos.x as u32);
        h = h.wrapping_mul(31).wrapping_add(w.pos.y as u32);
        h = h.wrapping_mul(31).wrapping_add(w.vel.x as u32);
        h = h.wrapping_mul(31).wrapping_add(w.vel.y as u32);
        h = h.wrapping_mul(31).wrapping_add(w.aiming_angle as u32);
        h = h.wrapping_mul(31).wrapping_add(w.health as u32);
        h = h.wrapping_mul(31).wrapping_add(w.lives as u32);
        h = h.wrapping_mul(31).wrapping_add(w.kills as u32);
        h = h.wrapping_mul(31).wrapping_add(w.timer as u32);
        h = h.wrapping_mul(31).wrapping_add(w.visible as u32);
        h = h.wrapping_mul(31).wrapping_add(w.control_states.pack());

        for weapon in &w.weapons {
            h = h.wrapping_mul(31).wrapping_add(weapon.ammo as u32);
            h = h.wrapping_mul(31).wrapping_add(weapon.delay_left as u32);
            h = h.wrapping_mul(31).wrapping_add(weapon.loading_left as u32);
            if let Some(id) = weapon.ty {
                h = h.wrapping_mul(31).wrapping_add(id as u32);
            }
        }

        h = h.wrapping_mul(31).wrapping_add(w.ninjarope.out as u32);
        h = h.wrapping_mul(31).wrapping_add(w.ninjarope.pos.x as u32);
        h = h.wrapping_mul(31).wrapping_add(w.ninjarope.pos.y as u32);
    }

    for b in state.bobjects.iter() {
        h = h.wrapping_mul(31).wrapping_add(b.pos.x as u32);
        h = h.wrapping_mul(31).wrapping_add(b.pos.y as u32);
    }

    for b in state.bonuses.iter() {
        h = h.wrapping_mul(31).wrapping_add(b.x as u32);
        h = h.wrapping_mul(31).wrapping_add(b.y as u32);
        h = h.wrapping_mul(31).wrapping_add(b.timer as u32);
        h = h.wrapping_mul(31).wrapping_add(b.weapon as u32);
        h = h.wrapping_mul(31).wrapping_add(b.frame as u32);
    }

    for s in state.sobjects.iter() {
        h = h.wrapping_mul(31).wrapping_add(s.id as u32);
        h = h.wrapping_mul(31).wrapping_add(s.cur_frame as u32);
    }

    for n in state.nobjects.iter() {
        h = h.wrapping_mul(31).wrapping_add(n.pos.x as u32);
        h = h.wrapping_mul(31).wrapping_add(n.pos.y as u32);
        h = h.wrapping_mul(31).wrapping_add(n.vel.x as u32);
        h = h.wrapping_mul(31).wrapping_add(n.vel.y as u32);
        h = h.wrapping_mul(31).wrapping_add(n.cur_frame as u32);
        if let Some(id) = n.ty {
            h = h.wrapping_mul(31).wrapping_add(id as u32);
        }
    }

    for wo in state.wobjects.iter() {
        h = h.wrapping_mul(31).wrapping_add(wo.pos.x as u32);
        h = h.wrapping_mul(31).wrapping_add(wo.pos.y as u32);
        h = h.wrapping_mul(31).wrapping_add(wo.vel.x as u32);
        h = h.wrapping_mul(31).wrapping_add(wo.vel.y as u32);
        h = h.wrapping_mul(31).wrapping_add(wo.cur_frame as u32);
        h = h.wrapping_mul(31).wrapping_add(wo.time_left as u32);
        if let Some(id) = wo.ty {
            h = h.wrapping_mul(31).wrapping_add(id as u32);
        }
    }

    h
}

/// Per-component hashes for diagnostic output on desync. Mirrors C++
/// `HashGameComponents` (`stateHash.hpp:129-213`). Each pool's hash seeds at
/// `1`; the per-worm hash uses the *subset* of fields in `HashGameComponents`.
pub fn hash_components(state: &SimState) -> ComponentHashes {
    let mut c = ComponentHashes {
        rng: state.rand.last(),
        ..ComponentHashes::default()
    };

    c.level = fold_level(1, state);

    for (wi, w) in state.worms.iter().take(NUM_PLAYERS).enumerate() {
        let mut h: u32 = 1;
        h = h.wrapping_mul(31).wrapping_add(w.pos.x as u32);
        h = h.wrapping_mul(31).wrapping_add(w.pos.y as u32);
        h = h.wrapping_mul(31).wrapping_add(w.vel.x as u32);
        h = h.wrapping_mul(31).wrapping_add(w.vel.y as u32);
        h = h.wrapping_mul(31).wrapping_add(w.health as u32);
        h = h.wrapping_mul(31).wrapping_add(w.lives as u32);
        h = h.wrapping_mul(31).wrapping_add(w.visible as u32);
        h = h.wrapping_mul(31).wrapping_add(w.timer as u32);
        c.worms[wi] = h;
    }

    {
        let mut h: u32 = 1;
        for b in state.bobjects.iter() {
            h = h.wrapping_mul(31).wrapping_add(b.pos.x as u32);
            h = h.wrapping_mul(31).wrapping_add(b.pos.y as u32);
        }
        c.bobjects = h;
    }

    {
        let mut h: u32 = 1;
        for b in state.bonuses.iter() {
            h = h.wrapping_mul(31).wrapping_add(b.x as u32);
            h = h.wrapping_mul(31).wrapping_add(b.y as u32);
            h = h.wrapping_mul(31).wrapping_add(b.timer as u32);
            h = h.wrapping_mul(31).wrapping_add(b.weapon as u32);
        }
        c.bonuses = h;
    }

    {
        let mut h: u32 = 1;
        for s in state.sobjects.iter() {
            h = h.wrapping_mul(31).wrapping_add(s.id as u32);
            h = h.wrapping_mul(31).wrapping_add(s.cur_frame as u32);
        }
        c.sobjects = h;
    }

    {
        let mut h: u32 = 1;
        for n in state.nobjects.iter() {
            h = h.wrapping_mul(31).wrapping_add(n.pos.x as u32);
            h = h.wrapping_mul(31).wrapping_add(n.pos.y as u32);
        }
        c.nobjects = h;
    }

    {
        let mut h: u32 = 1;
        for wo in state.wobjects.iter() {
            h = h.wrapping_mul(31).wrapping_add(wo.pos.x as u32);
            h = h.wrapping_mul(31).wrapping_add(wo.pos.y as u32);
        }
        c.wobjects = h;
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{BloodPool, Pool};
    use crate::state::{
        BObject, Bonus, ControlState, LevelSim, NObject, Ninjarope, SObject, WObject, WormState,
        WormWeapon, NUM_WEAPONS,
    };
    use sim_core::rng::Rand;
    use sim_core::vec::Vec2;

    // An empty state with a single-byte level and no worms / no objects, built
    // by hand (not via `SimState::new`) so the test owns every hashed field.
    fn empty_state(last_seed: u32, cycles: i32, level_byte: u8) -> SimState {
        let mut rand = Rand::new();
        rand.seed(last_seed);
        SimState {
            rand,
            cycles,
            level: LevelSim {
                width: 1,
                height: 1,
                material_id: vec![level_byte],
                material_flags: [0u8; 256],
            },
            worms: vec![],
            bonuses: Pool::new(4),
            wobjects: Pool::new(4),
            sobjects: Pool::new(4),
            nobjects: Pool::new(4),
            bobjects: BloodPool::new(4),
            physics: crate::physics::PhysicsConsts::default(),
        }
    }

    // (a) Master hash for a trivial state, folded by hand from the documented
    // accumulation: h=1, +rand.last, +cycles, then h*33 ^ byte for the level.
    #[test]
    fn master_hash_trivial_state_matches_hand_fold() {
        // rand.last() is 0 right after seeding (no draw consumed).
        let state = empty_state(0x1234, 7, 200);
        assert_eq!(state.rand.last(), 0, "no RNG drawn -> last == 0");

        // Hand fold, independent of the implementation.
        let mut h: u32 = 1;
        h = h.wrapping_mul(31).wrapping_add(0); // + rand.last
        h = h.wrapping_mul(31).wrapping_add(7u32); // + cycles
        h = h.wrapping_mul(33) ^ 200u32; // level byte
        // h = ((1*31+0)*31+7) = 968; 968*33 = 31944; 31944 ^ 200 = 31744.
        assert_eq!(h, 31744, "by-hand intermediate sanity");

        assert_eq!(hash_game_state(&state), 31744);
    }

    // Same, but with a non-zero rand.last (draw once) and a different cycles to
    // make sure both the rng and cycles terms are wired the right way round.
    #[test]
    fn master_hash_uses_rand_last_and_cycles() {
        let mut rand = Rand::new();
        rand.seed(0xABCD);
        let _ = rand.next_u32();
        let last = rand.last();
        assert_ne!(last, 0, "drew once -> last is the drawn value");

        let mut state = empty_state(0, 0, 50);
        state.rand = rand;
        state.cycles = -3; // exercise the signed->unsigned reinterpret on cycles

        let mut h: u32 = 1;
        h = h.wrapping_mul(31).wrapping_add(last);
        h = h.wrapping_mul(31).wrapping_add((-3i32) as u32);
        h = h.wrapping_mul(33) ^ 50u32;

        assert_eq!(hash_game_state(&state), h);
    }

    // (b) Component hashes for an empty state: every pool == seed 1, rng ==
    // rand.last, level == by-hand level fold, worms all default (1, no worms).
    #[test]
    fn component_hashes_empty_state() {
        let state = empty_state(0, 0, 123);
        let c = hash_components(&state);

        // Level fold by hand: h=1, then 1*33 ^ 123 = 33 ^ 123 = 90.
        let mut lv: u32 = 1;
        lv = lv.wrapping_mul(33) ^ 123u32;
        assert_eq!(lv, 90);
        assert_eq!(c.level, 90);

        assert_eq!(c.rng, state.rand.last());
        assert_eq!(c.rng, 0);

        // Empty pools all reduce to the seed value 1.
        assert_eq!(c.bobjects, 1);
        assert_eq!(c.bonuses, 1);
        assert_eq!(c.sobjects, 1);
        assert_eq!(c.nobjects, 1);
        assert_eq!(c.wobjects, 1);

        // No worms -> the worm slots stay at their default 0 (never written).
        assert_eq!(c.worms, [0, 0]);
    }

    // A worm with chosen field values, used by both the master- and
    // component-hash worm tests below.
    fn worm_fixture() -> WormState {
        let mut weapons = [WormWeapon::default(); NUM_WEAPONS];
        // Slot 0: a real weapon (ty set -> its id IS pushed).
        weapons[0] = WormWeapon {
            ty: Some(4),
            ammo: 10,
            delay_left: 1,
            loading_left: 2,
        };
        // Slot 1: empty slot (ty None -> id is NOT pushed), negative ammo to
        // exercise the signed reinterpret.
        weapons[1] = WormWeapon {
            ty: None,
            ammo: -1,
            delay_left: 0,
            loading_left: 0,
        };
        WormState {
            pos: Vec2::new(100, -200),
            vel: Vec2::new(-5, 6),
            aiming_angle: 32768,
            health: 100,
            lives: 5,
            kills: 3,
            timer: -7,
            visible: true,
            killed_timer: 150,
            control_states: ControlState::unpack(0x5a),
            weapons,
            ninjarope: Ninjarope {
                out: true,
                pos: Vec2::new(11, -22),
            },
            index: 0,
            stats_x: 0,
        }
    }

    // (c) The worm contribution to the master hash matches a by-hand fold that
    // covers field order, the `as u32` casts, the per-weapon `if type` push,
    // and the ninjarope tail.
    #[test]
    fn master_hash_worm_contribution_matches_hand_fold() {
        let mut state = empty_state(0, 0, 200);
        let w = worm_fixture();
        state.worms = vec![w.clone()];

        // Reproduce the full accumulation by hand.
        let mut h: u32 = 1;
        h = h.wrapping_mul(31).wrapping_add(0); // rand.last
        h = h.wrapping_mul(31).wrapping_add(0); // cycles
        h = h.wrapping_mul(33) ^ 200u32; // level byte

        h = h.wrapping_mul(31).wrapping_add((100i32) as u32); // pos.x
        h = h.wrapping_mul(31).wrapping_add((-200i32) as u32); // pos.y
        h = h.wrapping_mul(31).wrapping_add((-5i32) as u32); // vel.x
        h = h.wrapping_mul(31).wrapping_add((6i32) as u32); // vel.y
        h = h.wrapping_mul(31).wrapping_add((32768i32) as u32); // aiming_angle
        h = h.wrapping_mul(31).wrapping_add((100i32) as u32); // health
        h = h.wrapping_mul(31).wrapping_add((5i32) as u32); // lives
        h = h.wrapping_mul(31).wrapping_add((3i32) as u32); // kills
        h = h.wrapping_mul(31).wrapping_add((-7i32) as u32); // timer
        h = h.wrapping_mul(31).wrapping_add(1u32); // visible (true)
        h = h.wrapping_mul(31).wrapping_add(0x5au32); // control_states.pack()

        // weapon slot 0 (ty Some(4)): ammo, delay, loading, THEN id.
        h = h.wrapping_mul(31).wrapping_add((10i32) as u32);
        h = h.wrapping_mul(31).wrapping_add((1i32) as u32);
        h = h.wrapping_mul(31).wrapping_add((2i32) as u32);
        h = h.wrapping_mul(31).wrapping_add((4i32) as u32); // id pushed
                                                            // weapon slot 1 (ty None): ammo, delay, loading, NO id.
        h = h.wrapping_mul(31).wrapping_add((-1i32) as u32);
        h = h.wrapping_mul(31).wrapping_add((0i32) as u32);
        h = h.wrapping_mul(31).wrapping_add((0i32) as u32);
        // weapon slots 2..5 are default (ty None, all zero): ammo, delay, loading.
        for _ in 2..NUM_WEAPONS {
            h = h.wrapping_mul(31).wrapping_add(0u32);
            h = h.wrapping_mul(31).wrapping_add(0u32);
            h = h.wrapping_mul(31).wrapping_add(0u32);
        }

        // ninjarope tail.
        h = h.wrapping_mul(31).wrapping_add(1u32); // out (true)
        h = h.wrapping_mul(31).wrapping_add((11i32) as u32); // pos.x
        h = h.wrapping_mul(31).wrapping_add((-22i32) as u32); // pos.y

        assert_eq!(hash_game_state(&state), h);
    }

    // (c') The per-worm component hash uses the documented SUBSET, in order:
    // pos.x, pos.y, vel.x, vel.y, health, lives, visible, timer.
    #[test]
    fn component_worm_hash_uses_subset() {
        let mut state = empty_state(0, 0, 1);
        state.worms = vec![worm_fixture()];
        let c = hash_components(&state);

        let mut h: u32 = 1;
        h = h.wrapping_mul(31).wrapping_add((100i32) as u32); // pos.x
        h = h.wrapping_mul(31).wrapping_add((-200i32) as u32); // pos.y
        h = h.wrapping_mul(31).wrapping_add((-5i32) as u32); // vel.x
        h = h.wrapping_mul(31).wrapping_add((6i32) as u32); // vel.y
        h = h.wrapping_mul(31).wrapping_add((100i32) as u32); // health
        h = h.wrapping_mul(31).wrapping_add((5i32) as u32); // lives
        h = h.wrapping_mul(31).wrapping_add(1u32); // visible
        h = h.wrapping_mul(31).wrapping_add((-7i32) as u32); // timer

        assert_eq!(c.worms[0], h);
        assert_eq!(c.worms[1], 0, "second worm slot untouched");
    }

    // The level fold must visit exactly width*height bytes in order (and only
    // those), with the h*33 ^ byte mixer.
    #[test]
    fn level_fold_visits_all_cells_in_order() {
        let mut state = empty_state(0, 0, 0);
        state.level = LevelSim {
            width: 2,
            height: 3,
            material_id: vec![1, 2, 3, 4, 5, 6], // 6 cells
            material_flags: [0u8; 256],
        };
        let c = hash_components(&state);

        let mut h: u32 = 1;
        for byte in [1u8, 2, 3, 4, 5, 6] {
            h = h.wrapping_mul(33) ^ (byte as u32);
        }
        assert_eq!(c.level, h);
    }

    // Populated pools fold their fields in slot order; spot-check bonuses and
    // bobjects feed the master hash (and the component subset) correctly.
    #[test]
    fn populated_pools_fold_in_order() {
        let mut state = empty_state(0, 0, 9);
        state.bonuses.spawn(Bonus {
            x: 10,
            y: 20,
            timer: 30,
            weapon: 2,
            frame: 1,
        });
        state.bobjects.spawn(BObject {
            pos: Vec2::new(-1, -2),
        });
        state.sobjects.spawn(SObject { id: 7, cur_frame: 8 });
        state.nobjects.spawn(NObject {
            pos: Vec2::new(3, 4),
            vel: Vec2::new(5, 6),
            cur_frame: 9,
            ty: Some(2),
        });
        state.wobjects.spawn(WObject {
            pos: Vec2::new(7, 8),
            vel: Vec2::new(9, 10),
            cur_frame: 11,
            time_left: 12,
            ty: None,
        });

        // Master hash by hand (no worms).
        let mut h: u32 = 1;
        h = h.wrapping_mul(31).wrapping_add(0); // rand
        h = h.wrapping_mul(31).wrapping_add(0); // cycles
        h = h.wrapping_mul(33) ^ 9u32; // level
                                       // bobjects: pos.x, pos.y
        h = h.wrapping_mul(31).wrapping_add((-1i32) as u32);
        h = h.wrapping_mul(31).wrapping_add((-2i32) as u32);
        // bonuses: x, y, timer, weapon, frame
        h = h.wrapping_mul(31).wrapping_add(10u32);
        h = h.wrapping_mul(31).wrapping_add(20u32);
        h = h.wrapping_mul(31).wrapping_add(30u32);
        h = h.wrapping_mul(31).wrapping_add(2u32);
        h = h.wrapping_mul(31).wrapping_add(1u32);
        // sobjects: id, cur_frame
        h = h.wrapping_mul(31).wrapping_add(7u32);
        h = h.wrapping_mul(31).wrapping_add(8u32);
        // nobjects: pos.x, pos.y, vel.x, vel.y, cur_frame, ty.id (Some(2))
        h = h.wrapping_mul(31).wrapping_add(3u32);
        h = h.wrapping_mul(31).wrapping_add(4u32);
        h = h.wrapping_mul(31).wrapping_add(5u32);
        h = h.wrapping_mul(31).wrapping_add(6u32);
        h = h.wrapping_mul(31).wrapping_add(9u32);
        h = h.wrapping_mul(31).wrapping_add(2u32);
        // wobjects: pos.x, pos.y, vel.x, vel.y, cur_frame, time_left (ty None -> no id)
        h = h.wrapping_mul(31).wrapping_add(7u32);
        h = h.wrapping_mul(31).wrapping_add(8u32);
        h = h.wrapping_mul(31).wrapping_add(9u32);
        h = h.wrapping_mul(31).wrapping_add(10u32);
        h = h.wrapping_mul(31).wrapping_add(11u32);
        h = h.wrapping_mul(31).wrapping_add(12u32);

        assert_eq!(hash_game_state(&state), h);

        // Component subset spot-checks.
        let c = hash_components(&state);
        let mut hb: u32 = 1;
        hb = hb.wrapping_mul(31).wrapping_add(10u32);
        hb = hb.wrapping_mul(31).wrapping_add(20u32);
        hb = hb.wrapping_mul(31).wrapping_add(30u32);
        hb = hb.wrapping_mul(31).wrapping_add(2u32);
        assert_eq!(c.bonuses, hb, "component bonuses omits frame");
        let mut hn: u32 = 1;
        hn = hn.wrapping_mul(31).wrapping_add(3u32);
        hn = hn.wrapping_mul(31).wrapping_add(4u32);
        assert_eq!(c.nobjects, hn, "component nobjects is pos only");
    }
}
