//! The `.lrp` replay rollback checksum (`WideRollbackChecksum`), ported
//! bit-for-bit from `replay.cpp:168-258`.
//!
//! This is a **second, distinct** state hash — do NOT conflate it with
//! [`crate::hash::hash_game_state`]. Where `hash_game_state` mirrors
//! `stateHash.hpp` (a `*31`/`*33^` fold over a smaller field subset), this
//! reproduces the replay recorder's `Mix32` (boost-style hash-combine) over a
//! **wider** inventory in a **different order**: RNG + cycles, every worm's full
//! sim state (21 fields + per-weapon triples), each of the four projectile/bonus
//! pools (count-prefixed, per-object fields including the ones `hash_game_state`
//! drops — `wobject.owner_idx`, `sobject.x/y`, `bonus.frame/timer`), and the
//! whole `material_id` buffer. The C++ recorder embeds this word every 1050
//! frames (`replay.cpp:369`); the Phase-1 `.lrp` gate replays a corpus and
//! checks each embedded word against this function.
//!
//! Every step uses `wrapping_*` (the C++ relies on `uint32_t` overflow) and each
//! signed field is reinterpreted with `as u32` (two's-complement reinterpret ==
//! C++ `static_cast<uint32_t>`). Two fields have no home on the Rust `SimState`:
//!
//! - **`worm.flags`** (`worm.hpp:246`, the GameOfTag flag *count*) — Rust
//!   `WormState` has no such field; it is always `0` in the KillEmAll corpus
//!   (`game_mode 0`, no flag pickups), so a constant `0` is folded in its slot.
//! - **`worm.prev_control_states.istate`** — Rust `WormState` carries no XOR
//!   baseline (the sim replays absolute per-tick words). The **caller** supplies
//!   it per worm (`prev_istates[i]`); the `.lrp` reader maintains that baseline
//!   as it decodes the delta stream, and the gate harness passes it here.

use crate::state::SimState;

/// The `Mix32` boost-style hash-combine (`replay.cpp:168`):
/// `h ^= v + 0x9e3779b9 + (h << 6) + (h >> 2)`, all `u32`-wrapping. The shifts
/// read the *pre-XOR* `h`.
#[inline]
fn mix32(h: u32, v: u32) -> u32 {
    h ^ v
        .wrapping_add(0x9e37_79b9)
        .wrapping_add(h << 6)
        .wrapping_add(h >> 2)
}

/// The replay rollback checksum over `state`, mirroring `WideRollbackChecksum`
/// (`replay.cpp:177-258`) field-for-field. `prev_istates[i]` is worm `i`'s
/// reader-tracked `prev_control_states.istate` (the XOR baseline); missing
/// entries fold as `0`.
pub fn wide_rollback_checksum(state: &SimState, prev_istates: &[u32]) -> u32 {
    // Release semantics stay as-is (`prev_istates.get(i)…unwrap_or(0)` below),
    // but a length mismatch means some worm's baseline is silently masked to 0.
    // Catch that caller bug in test/debug without changing release behaviour.
    debug_assert_eq!(
        prev_istates.len(),
        state.worms.len(),
        "wide_rollback_checksum expects one XOR baseline per worm \
         (prev_istates.len() == worms.len()); a shorter slice masks per-worm \
         istate via unwrap_or(0)"
    );
    let mut h: u32 = state.rand.last(); // seed (replay.cpp:181)
    h = mix32(h, state.cycles as u32);

    for (i, w) in state.worms.iter().enumerate() {
        h = mix32(h, w.pos.x as u32);
        h = mix32(h, w.pos.y as u32);
        h = mix32(h, w.vel.x as u32);
        h = mix32(h, w.vel.y as u32);
        h = mix32(h, w.aiming_angle as u32);
        h = mix32(h, w.aiming_speed as u32);
        h = mix32(h, w.health as u32);
        h = mix32(h, w.lives as u32);
        h = mix32(h, w.kills as u32);
        h = mix32(h, w.timer as u32);
        h = mix32(h, w.killed_timer as u32);
        h = mix32(h, w.current_frame as u32);
        h = mix32(h, w.current_weapon as u32);
        h = mix32(h, w.direction as u32);
        // `worm.flags` (GameOfTag flag count) — no Rust field; always 0 in the
        // KillEmAll corpus (see module doc).
        h = mix32(h, 0);
        h = mix32(h, w.visible as u32);
        h = mix32(h, w.ready as u32);
        h = mix32(h, w.able_to_jump as u32);
        h = mix32(h, w.able_to_dig as u32);
        h = mix32(h, w.control_states.pack());
        // `prev_control_states.istate` — reader-tracked baseline (see module doc).
        h = mix32(h, prev_istates.get(i).copied().unwrap_or(0));
        for ww in &w.weapons {
            h = mix32(h, ww.ammo as u32);
            h = mix32(h, ww.delay_left as u32);
            h = mix32(h, ww.loading_left as u32);
        }
    }

    h = mix32(h, state.wobjects.len() as u32);
    for o in state.wobjects.iter() {
        h = mix32(h, o.pos.x as u32);
        h = mix32(h, o.pos.y as u32);
        h = mix32(h, o.vel.x as u32);
        h = mix32(h, o.vel.y as u32);
        h = mix32(h, o.cur_frame as u32);
        h = mix32(h, o.time_left as u32);
        h = mix32(h, o.owner_idx as u32);
    }

    h = mix32(h, state.sobjects.len() as u32);
    for o in state.sobjects.iter() {
        h = mix32(h, o.x as u32);
        h = mix32(h, o.y as u32);
        h = mix32(h, o.cur_frame as u32);
        h = mix32(h, o.id as u32);
    }

    h = mix32(h, state.nobjects.len() as u32);
    for o in state.nobjects.iter() {
        h = mix32(h, o.pos.x as u32);
        h = mix32(h, o.pos.y as u32);
        h = mix32(h, o.vel.x as u32);
        h = mix32(h, o.vel.y as u32);
        h = mix32(h, o.cur_frame as u32);
    }

    h = mix32(h, state.bonuses.len() as u32);
    for b in state.bonuses.iter() {
        h = mix32(h, b.x as u32);
        h = mix32(h, b.y as u32);
        h = mix32(h, b.frame as u32);
        h = mix32(h, b.timer as u32);
    }

    // The whole material buffer, byte by byte (`replay.cpp:251-255`, MixBytes).
    let cells = (state.level.width as i64 * state.level.height as i64) as usize;
    for &byte in &state.level.material_id[..cells] {
        h = mix32(h, byte as u32);
    }

    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{BloodPool, Pool};
    use crate::state::{
        Bonus, ControlState, LevelSim, NObject, SObject, WObject, WormState, WormWeapon,
        NUM_WEAPONS,
    };
    use sim_core::rng::Rand;
    use sim_core::vec::Vec2;

    /// A hand-built empty state (single-byte level, no worms/objects) — mirrors
    /// the `hash.rs` test helper so the test owns every folded field.
    fn empty_state(last_seed: u32, cycles: i32, level_byte: u8) -> SimState {
        let mut rand = Rand::new();
        rand.seed(last_seed);
        SimState {
            rand,
            cycles,
            screen_flash: 0,
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
            control: crate::control::ControlConsts::default(),
            weapons: Vec::new(),
            cossin: sim_core::tables::precompute_cossin(),
            h_signed_recoil: false,
            large_sprites: assets::sprite::SpriteSet::default(),
            worm_sprites: assets::sprite::SpriteSet::default(),
            small_sprites: assets::sprite::SpriteSet::default(),
            textures: Vec::new(),
            sobject_types: Vec::new(),
            nobject_types: Vec::new(),
            settings_loading_time: 100,
            load_change: true,
            blood: 100,
            num_blood_colours: 0,
            first_blood_colour: 0,
            bobj_gravity: 0,
            laser_weapon: 0,
            wobject_consts: crate::weapon::WObjectConsts::default(),
            settings_health: 100,
            game_mode: 0,
            time_to_lose: 600,
            shadow: false,
            last_killed_idx: -1,
            got_changed: false,
            settings_max_bonuses: 0,
            bonus_drop_chance: 0,
            bonus_spawn_rect_w: 0,
            bonus_spawn_rect_h: 0,
            bonus_spawn_rect_x: 0,
            bonus_spawn_rect_y: 0,
            h_bonus_spawn_rect: false,
            h_bonus_only_health: false,
            h_bonus_only_weapon: false,
            h_bonus_disable: false,
            bonus_rand_timer: [[0, 0], [0, 0]],
            weap_table: Vec::new(),
            bonus_gravity: 0,
            bonus_bounce_mul: 0,
            bonus_bounce_div: 0,
            bonus_s_objects: [0, 0],
            bonus_health_var: 0,
            bonus_min_health: 0,
            bonus_explode_risk: 0,
            h_bonus_reload_only: false,
            worm_spawn_rect_x: 0,
            worm_spawn_rect_y: 0,
            worm_spawn_rect_w: 0,
            worm_spawn_rect_h: 0,
            worm_min_spawn_dist_last: 0,
            worm_min_spawn_dist_enemy: 0,
            sound_hooks: assets::tc::SoundHooks::default(),
            sound_events: Vec::new(),
            shake_events: Vec::new(),
        }
    }

    /// A worm whose every WIDE-checksum field has a distinct, chosen value —
    /// including the ones `hash_game_state` drops (`aiming_speed`, `killed_timer`,
    /// `current_frame`, `current_weapon`, `direction`, `ready`, `able_to_jump`,
    /// `able_to_dig`) — so the hand-fold pins the full field order.
    fn worm_fixture() -> WormState {
        let mut weapons = [WormWeapon::default(); NUM_WEAPONS];
        weapons[0] = WormWeapon {
            ty: Some(4),
            ammo: 10,
            delay_left: 1,
            loading_left: 2,
        };
        weapons[1] = WormWeapon {
            ty: None,
            ammo: -1, // exercise the signed reinterpret
            delay_left: 3,
            loading_left: 4,
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
            ninjarope: Default::default(),
            index: 0,
            stats_x: 0,
            last_killed_by_idx: -1,
            aiming_speed: -9,
            direction: 1,
            movable: true,
            able_to_jump: true,
            able_to_dig: false,
            key_change_pressed: false,
            current_weapon: 2,
            fire_cone: 0,
            leave_shell_timer: 0,
            logic_respawn: Vec2::zero(),
            ready: true,
            make_sight_green: false,
            steerable_count: 0,
            steerable_sum_x: 0,
            steerable_sum_y: 0,
            current_frame: 13,
            animate: false,
            hotspot_x: 0,
            hotspot_y: 0,
        }
    }

    /// Independent hand-fold of the full `Mix32` field order over a populated
    /// state: seed + cycles, one worm (all 21 fields + 5 weapon triples), one of
    /// each pool object (count-prefixed, folding the fields `hash_game_state`
    /// drops), then the level byte. The `flags` slot folds a constant `0`; the
    /// worm's `prev_control_states.istate` is the caller-supplied baseline.
    #[test]
    fn wide_checksum_matches_hand_fold() {
        let mut state = empty_state(0, 0, 200);
        state.worms = vec![worm_fixture()];
        state.wobjects.spawn(WObject {
            pos: Vec2::new(7, 8),
            vel: Vec2::new(9, 10),
            cur_frame: 11,
            time_left: 12,
            ty: None,
            owner_idx: 99, // folded by WIDE (hash_game_state drops it)
        });
        state.sobjects.spawn(SObject {
            id: 7,
            x: 111, // folded by WIDE (hash_game_state drops it)
            y: 222, // folded by WIDE (hash_game_state drops it)
            cur_frame: 8,
            anim_delay: 33,
        });
        state.nobjects.spawn(NObject {
            pos: Vec2::new(3, 4),
            vel: Vec2::new(5, 6),
            cur_frame: 9,
            ty: Some(2),
            owner_idx: 55,
            time_left: 66,
        });
        state.bonuses.spawn(Bonus {
            x: 10,
            y: 20,
            timer: 30, // folded by WIDE
            weapon: 2,
            frame: 1, // folded by WIDE
            vel_y: 99,
        });

        let prev_istate = 0x33u32;

        // ---- Independent hand-fold (does not call the impl) -------------------
        let mix = |h: u32, v: u32| -> u32 {
            h ^ v
                .wrapping_add(0x9e37_79b9)
                .wrapping_add(h << 6)
                .wrapping_add(h >> 2)
        };
        let mut h: u32 = 0; // rand.last() after seed(0), no draw
        h = mix(h, 0u32); // cycles
                          // worm
        h = mix(h, 100u32); // pos.x
        h = mix(h, (-200i32) as u32); // pos.y
        h = mix(h, (-5i32) as u32); // vel.x
        h = mix(h, 6u32); // vel.y
        h = mix(h, 32768u32); // aiming_angle
        h = mix(h, (-9i32) as u32); // aiming_speed
        h = mix(h, 100u32); // health
        h = mix(h, 5u32); // lives
        h = mix(h, 3u32); // kills
        h = mix(h, (-7i32) as u32); // timer
        h = mix(h, 150u32); // killed_timer
        h = mix(h, 13u32); // current_frame
        h = mix(h, 2u32); // current_weapon
        h = mix(h, 1u32); // direction
        h = mix(h, 0u32); // flags (constant 0)
        h = mix(h, 1u32); // visible
        h = mix(h, 1u32); // ready
        h = mix(h, 1u32); // able_to_jump
        h = mix(h, 0u32); // able_to_dig
        h = mix(h, 0x5au32); // control_states.istate
        h = mix(h, prev_istate); // prev_control_states.istate (caller-supplied)
                                 // weapon slot 0
        h = mix(h, 10u32);
        h = mix(h, 1u32);
        h = mix(h, 2u32);
        // weapon slot 1 (negative ammo)
        h = mix(h, (-1i32) as u32);
        h = mix(h, 3u32);
        h = mix(h, 4u32);
        // weapon slots 2..NUM_WEAPONS (default zero)
        for _ in 2..NUM_WEAPONS {
            h = mix(h, 0u32);
            h = mix(h, 0u32);
            h = mix(h, 0u32);
        }
        // wobjects: count, then pos.x, pos.y, vel.x, vel.y, cur_frame, time_left, owner_idx
        h = mix(h, 1u32);
        h = mix(h, 7u32);
        h = mix(h, 8u32);
        h = mix(h, 9u32);
        h = mix(h, 10u32);
        h = mix(h, 11u32);
        h = mix(h, 12u32);
        h = mix(h, 99u32);
        // sobjects: count, then x, y, cur_frame, id
        h = mix(h, 1u32);
        h = mix(h, 111u32);
        h = mix(h, 222u32);
        h = mix(h, 8u32);
        h = mix(h, 7u32);
        // nobjects: count, then pos.x, pos.y, vel.x, vel.y, cur_frame
        h = mix(h, 1u32);
        h = mix(h, 3u32);
        h = mix(h, 4u32);
        h = mix(h, 5u32);
        h = mix(h, 6u32);
        h = mix(h, 9u32);
        // bonuses: count, then x, y, frame, timer
        h = mix(h, 1u32);
        h = mix(h, 10u32);
        h = mix(h, 20u32);
        h = mix(h, 1u32); // frame
        h = mix(h, 30u32); // timer
                           // level material buffer (1 byte)
        h = mix(h, 200u32);

        assert_eq!(wide_rollback_checksum(&state, &[prev_istate]), h);
    }

    /// Seed + cycles wiring: a non-zero `rand.last()` and a negative `cycles`
    /// (exercising the signed reinterpret) over an empty, worm-less state.
    #[test]
    fn wide_checksum_seed_and_cycles() {
        let mut rand = Rand::new();
        rand.seed(0xABCD);
        let _ = rand.next_u32();
        let last = rand.last();
        assert_ne!(last, 0, "drew once -> last is the drawn value");

        let mut state = empty_state(0, 0, 50);
        state.rand = rand;
        state.cycles = -3;

        let mix = |h: u32, v: u32| -> u32 {
            h ^ v
                .wrapping_add(0x9e37_79b9)
                .wrapping_add(h << 6)
                .wrapping_add(h >> 2)
        };
        let mut h = last;
        h = mix(h, (-3i32) as u32); // cycles
        h = mix(h, 0u32); // wobjects count
        h = mix(h, 0u32); // sobjects count
        h = mix(h, 0u32); // nobjects count
        h = mix(h, 0u32); // bonuses count
        h = mix(h, 50u32); // level byte

        assert_eq!(wide_rollback_checksum(&state, &[]), h);
    }

    /// Non-vacuity: a field the WIDE checksum folds but `hash_game_state`
    /// **drops** (`wobject.owner_idx`) must MOVE the wide checksum, while a field
    /// neither folds (`sobject.anim_delay`) must leave it unchanged. This proves
    /// the port folds the strictly wider inventory.
    #[test]
    fn wide_checksum_folds_wider_set_than_hash_game_state() {
        let base = {
            let mut s = empty_state(0, 0, 1);
            s.wobjects.spawn(WObject {
                pos: Vec2::new(1, 2),
                vel: Vec2::new(3, 4),
                cur_frame: 5,
                time_left: 6,
                ty: None,
                owner_idx: 0,
            });
            s.sobjects.spawn(SObject {
                id: 1,
                x: 0,
                y: 0,
                cur_frame: 0,
                anim_delay: 0,
            });
            s
        };
        let base_h = wide_rollback_checksum(&base, &[]);

        // Perturb wobject.owner_idx (WIDE folds it; hash_game_state ignores it).
        let mut owner = empty_state(0, 0, 1);
        owner.wobjects.spawn(WObject {
            pos: Vec2::new(1, 2),
            vel: Vec2::new(3, 4),
            cur_frame: 5,
            time_left: 6,
            ty: None,
            owner_idx: 123, // the only change
        });
        owner.sobjects.spawn(SObject {
            id: 1,
            x: 0,
            y: 0,
            cur_frame: 0,
            anim_delay: 0,
        });
        assert_ne!(
            wide_rollback_checksum(&owner, &[]),
            base_h,
            "wobject.owner_idx MUST move the wide checksum (it folds the wider set)"
        );

        // Perturb sobject.anim_delay — folded by NEITHER hash — checksum unchanged.
        let mut anim = empty_state(0, 0, 1);
        anim.wobjects.spawn(WObject {
            pos: Vec2::new(1, 2),
            vel: Vec2::new(3, 4),
            cur_frame: 5,
            time_left: 6,
            ty: None,
            owner_idx: 0,
        });
        anim.sobjects.spawn(SObject {
            id: 1,
            x: 0,
            y: 0,
            cur_frame: 0,
            anim_delay: 42, // the only change; folded by neither
        });
        assert_eq!(
            wide_rollback_checksum(&anim, &[]),
            base_h,
            "sobject.anim_delay is folded by NEITHER hash -> checksum unchanged"
        );
    }

    /// The caller-supplied `prev_control_states` baseline is folded per worm: two
    /// states identical except the `prev_istates` slice must hash differently.
    #[test]
    fn wide_checksum_folds_prev_control_states_baseline() {
        let mut state = empty_state(0, 0, 1);
        state.worms = vec![worm_fixture()];
        let a = wide_rollback_checksum(&state, &[0x00]);
        let b = wide_rollback_checksum(&state, &[0x7f]);
        assert_ne!(a, b, "prev_control_states.istate is folded per worm");
    }

    /// A caller that passes a `prev_istates` slice shorter than the worm count
    /// would have its per-worm baseline silently masked to `0` by the
    /// `unwrap_or(0)` default. Release semantics are unchanged (T1 keeps the
    /// tick-0 `unwrap_or(0)` sound), but a `debug_assert` turns that latent
    /// caller bug into a hard failure under test/debug.
    #[test]
    #[should_panic(expected = "one XOR baseline per worm")]
    fn wide_checksum_debug_asserts_prev_len_matches_worms() {
        let mut state = empty_state(0, 0, 1);
        state.worms = vec![worm_fixture()];
        // One worm, zero baselines -> length mismatch.
        let _ = wide_rollback_checksum(&state, &[]);
    }
}
