//! Port of `Worm::Fire` (`worm.cpp:1099-1148`) + `Weapon::Fire`
//! (`weapon.cpp:16-76`) — **the slice where RNG goes live**.
//!
//! [`worm_fire`] is the per-worm fire entry point: it decrements ammo, arms the
//! firing delay, sets the fire-cone, computes the muzzle position/velocity, fires
//! `parts` projectiles (each via [`weapon_fire`]), and finally applies recoil to
//! the worm's velocity. [`weapon_fire`] spawns one [`WObject`] into the wobjects
//! pool and draws the spread / colour / time-variance RNG.
//!
//! **The `rand()` call order is the contract.** For the fan weapon the sequence
//! is exactly four draws, in this order (`weapon.cpp:33-75`):
//!
//! 1. spread `vel.x` = `rand(distribution * 2)`   (fan: `rand(24000)`)
//! 2. spread `vel.y` = `rand(distribution * 2)`   (fan: `rand(24000)`)
//! 3. colour       = `rand(2)`                    (`start_frame < 0` path)
//! 4. time-var     = `rand(time_to_explo_v)`      (fan: `rand(10)`)
//!
//! The leave-shell draw (`rand(leave_shells)`, `worm.cpp:1114`) precedes all of
//! these but is **guarded** by `leave_shells > 0`; fan has `leave_shells = 0`, so
//! it is not drawn. A reordered / extra / missing draw shifts every downstream
//! `rand.last` and desyncs the simulation, so the order here is load-bearing.
//!
//! Fixed-point: `cossin * speed / 100`, `vel * 100 / speed`, `* recoil / 100` all
//! use **truncating** integer division ([`Vec2::div`], `wrapping_div`), never an
//! arithmetic shift. `Ftoi(aiming_angle)` is the arithmetic `>> 16` ([`ftoi`]).

use assets::object::Weapon;
use sim_core::fixed::{ftoi, itof};
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::pool::Pool;
use crate::state::{WObject, WormState};

// `Weapon::shot_type` enum values (`weapon.hpp:21`):
// `enum { kStNormal, kStdType1, kStSteerable, kStdType2, kStLaser };`
const ST_NORMAL: i32 = 0;
const ST_TYPE1: i32 = 1;
const ST_STEERABLE: i32 = 2;
const ST_TYPE2: i32 = 3;

/// Port of `Weapon::Fire` (`weapon.cpp:16-76`): spawn one projectile.
///
/// `angle` is `Ftoi(aiming_angle)` (the 0..127 cossin index), `vel` is the
/// `firing_vel` carried from [`worm_fire`], `speed` the (possibly worm-adjusted)
/// weapon speed, `pos` the muzzle position, `owner_idx` the firing worm's index.
///
/// Draws RNG in C++ order: spread `vel.x`, spread `vel.y` (only when
/// `distribution != 0`), then the colour `rand(2)` on the `start_frame < 0` path,
/// then `rand(time_to_explo_v)` (only when non-zero). Returns the spawned slot
/// index (`Some` while the pool has room — always the case in 4a; the
/// `NewObjectReuse` full-pool overwrite is deferred).
///
/// The C++ stats calls (`DamagePotential`, `Shot`) and the stats-only
/// `fired_by` / `has_hit` fields are no-ops and are omitted.
#[allow(clippy::too_many_arguments)]
pub fn weapon_fire(
    weapon: &Weapon,
    angle: i32,
    vel: Vec2,
    speed: i32,
    pos: Vec2,
    owner_idx: i32,
    cossin: &[Vec2; 128],
    rand: &mut Rand,
    wobjects: &mut Pool<WObject>,
) -> Option<usize> {
    let mut obj = WObject {
        pos,
        owner_idx,
        ty: Some(weapon.id),
        ..WObject::default()
    };

    // obj.vel = cossin[angle] * speed / 100 + vel   (truncating div, then add).
    obj.vel = cossin[angle as usize].mul(speed).div(100).add(vel);

    // Spread RNG — x THEN y, only when distribution != 0 (fan: 12000).
    if weapon.distribution != 0 {
        let dist = weapon.distribution;
        let max = (dist * 2) as u32;
        obj.vel.x = obj
            .vel
            .x
            .wrapping_add((rand.bound(max) as i32).wrapping_sub(dist));
        obj.vel.y = obj
            .vel
            .y
            .wrapping_add((rand.bound(max) as i32).wrapping_sub(dist));
    }

    // cur_frame (weapon.cpp:39-69). Fan takes the `start_frame < 0` colour-rand
    // path; the other branches are ported faithfully for non-fan weapons.
    if weapon.start_frame >= 0 {
        if weapon.shot_type == ST_NORMAL {
            obj.cur_frame = if weapon.loop_anim {
                if weapon.num_frames != 0 {
                    rand.bound((weapon.num_frames + 1) as u32) as i32
                } else {
                    rand.bound(2) as i32
                }
            } else {
                0
            };
        } else if weapon.shot_type == ST_TYPE1 {
            let mut a = angle;
            if a > 64 {
                a -= 1;
            }
            // C++ clamps cur_frame into [0, 12] via two ifs; clamp is identical.
            obj.cur_frame = ((a - 12) >> 3).clamp(0, 12);
        } else if weapon.shot_type == ST_TYPE2 || weapon.shot_type == ST_STEERABLE {
            obj.cur_frame = angle;
        } else {
            obj.cur_frame = 0;
        }
    } else {
        obj.cur_frame = weapon.color_bullets - rand.bound(2) as i32;
    }

    // time_left = time_to_explo (- rand(time_to_explo_v) when non-zero).
    obj.time_left = weapon.time_to_explo;
    if weapon.time_to_explo_v != 0 {
        obj.time_left -= rand.bound(weapon.time_to_explo_v as u32) as i32;
    }

    wobjects.spawn(obj)
}

/// Port of `Worm::Fire` (`worm.cpp:1099-1148`).
///
/// Mutates `worm` (ammo--, `delay_left`, `fire_cone`, the recoil on `vel`, and —
/// when its guard fires — `leave_shell_timer`) and spawns `parts` projectiles
/// into `wobjects`. `h_signed_recoil` is `common.h[HSignedRecoil]` (the TC
/// `[hacks].SignedRecoil` flag); fan's `recoil = 2` never triggers it.
///
/// Statement order matches C++ exactly: ammo/delay/fire_cone, muzzle position,
/// **leave-shell guard** (the first potential `rand`), sound (skipped — no sim or
/// RNG effect), `affect_by_worm` speed/firing_vel, the `parts × weapon_fire`
/// loop, then recoil **after** the loop.
pub fn worm_fire(
    worm: &mut WormState,
    weapons: &[Weapon],
    cossin: &[Vec2; 128],
    h_signed_recoil: bool,
    rand: &mut Rand,
    wobjects: &mut Pool<WObject>,
) {
    let cw = worm.current_weapon as usize;
    let weapon_id = worm.weapons[cw]
        .ty
        .expect("worm_fire: current weapon slot must have a resolved type");
    let w = &weapons[weapon_id as usize];

    // --ww.ammo;  ww.delay_left = w.delay;
    worm.weapons[cw].ammo -= 1;
    worm.weapons[cw].delay_left = w.delay;

    worm.fire_cone = w.fire_cone;

    // kFiring = cossin[angle] * (detect_distance + 5) + pos - (0, Itof(1)).
    let angle = ftoi(worm.aiming_angle);
    let firing_pos = cossin[angle as usize]
        .mul(w.detect_distance + 5)
        .add(worm.pos)
        .sub(Vec2::new(0, itof(1)));

    // Leave-shell guard (the first potential rand). Fan: leave_shells = 0 -> skip.
    if w.leave_shells > 0 && rand.bound(w.leave_shells as u32) == 0 {
        worm.leave_shell_timer = w.leave_shell_delay;
    }

    // Launch sound: skipped (no sim / RNG effect).

    let mut speed = w.speed;
    let mut firing_vel = Vec2::zero();
    let parts = w.parts;

    if w.affect_by_worm {
        speed = speed.max(100);
        firing_vel = worm.vel.mul(100).div(speed);
    }

    for _ in 0..parts {
        weapon_fire(
            w, angle, firing_vel, speed, firing_pos, worm.index, cossin, rand, wobjects,
        );
    }

    // Recoil, AFTER the parts loop. HSignedRecoil hack: recoil >= 128 -> -256.
    let mut recoil = w.recoil;
    if h_signed_recoil && recoil >= 128 {
        recoil -= 256;
    }
    worm.vel = worm.vel.sub(cossin[angle as usize].mul(recoil).div(100));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{WeaponInit, WormInit, WormWeapon, NUM_WEAPONS};
    use sim_core::tables::precompute_cossin;

    // The real fan weapon, loaded from the shipped TC config. Cross-ref lists are
    // empty: none of the fired fields (speed, distribution, parts, recoil,
    // time_to_explo*, start_frame, color_bullets, leave_shells, affect_by_worm,
    // fire_cone, delay, detect_distance) depend on a cross-ref, and `id` is set by
    // the caller (== array index in the weapon table). So an empty-list load
    // yields fan's exact fire parameters.
    fn fan_weapon(id: i32) -> Weapon {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/weapons/fan.cfg"
        ));
        let mut w = Weapon::load(bytes, &[], &[], &[]).unwrap();
        w.id = id;
        w
    }

    // A worm wired to fire weapon-slot 0 at a known kinematic state: aiming_angle
    // = Itof(32) (cossin index 32), a non-zero pos and vel so the muzzle position
    // and firing_vel are both exercised. The slot's `ty` is the index INTO the
    // weapons slice (0), which is decoupled from the weapon's `id` (the value
    // stored on the spawned wobject); callers pass a one-element `&[weapon]`.
    fn firing_worm(ammo: i32) -> WormState {
        let mut weapons = [WeaponInit::default(); NUM_WEAPONS];
        weapons[0] = WeaponInit { ty: Some(0), ammo };
        let mut w = WormState::from_init(&WormInit {
            index: 1,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons,
            start_pos: Vec2::new(6_553_600, 3_276_800), // (100.0, 50.0) in 16.16
            visible: true,
        });
        w.aiming_angle = itof(32);
        w.vel = Vec2::new(200_000, -100_000);
        w.current_weapon = 0;
        w
    }

    // A seed whose first two rand(24000) draws differ, so an x<->y swap is
    // detectable; asserted in the order test.
    const SEED: u32 = 0x4242;

    fn seeded() -> Rand {
        let mut r = Rand::new();
        r.seed(SEED);
        r
    }

    // ---- Step 1: Fire RNG order + spawn (fan constants) ----------------------

    #[test]
    fn fan_fire_spawns_one_wobject_with_spread_vel_x_then_y() {
        let cossin = precompute_cossin();
        let fan = fan_weapon(7);
        let mut worm = firing_worm(150);
        let pre_vel = worm.vel;
        let mut pool: Pool<WObject> = Pool::new(600);
        let mut rand = seeded();

        // Reference RNG stream: the four fan draws in order.
        let mut refr = seeded();
        let d_spread_x = refr.bound(24000) as i32;
        let d_spread_y = refr.bound(24000) as i32;
        let d_color = refr.bound(2) as i32;
        let d_time = refr.bound(10) as i32;
        assert_ne!(
            d_spread_x, d_spread_y,
            "seed must give distinct x/y draws so an order swap is detectable"
        );

        worm_fire(&mut worm, &[fan], &cossin, false, &mut rand, &mut pool);

        // Exactly `parts` (= 1) wobjects spawned, in slot 0.
        assert_eq!(pool.len(), 1, "fan parts = 1 -> exactly one wobject");
        let obj = *pool.get(0).expect("wobject spawned in slot 0");

        // vel = cossin[32]*180/100 + firing_vel, then += (dx-12000, dy-12000).
        let firing_vel = pre_vel.mul(100).div(180); // affect_by_worm, speed 180
        let base = cossin[32].mul(180).div(100).add(firing_vel);
        assert_eq!(
            obj.vel.x,
            base.x + (d_spread_x - 12000),
            "vel.x uses the FIRST rand(24000)"
        );
        assert_eq!(
            obj.vel.y,
            base.y + (d_spread_y - 12000),
            "vel.y uses the SECOND rand(24000)"
        );

        // cur_frame = color_bullets - rand(2) (start_frame < 0 path).
        assert_eq!(obj.cur_frame, 25 - d_color, "cur_frame = 25 - rand(2)");
        // time_left = 45 - rand(10).
        assert_eq!(obj.time_left, 45 - d_time, "time_left = 45 - rand(10)");

        // owner + type carried through.
        assert_eq!(obj.owner_idx, 1, "owner_idx = worm.index");
        assert_eq!(obj.ty, Some(7), "ty = weapon id");
    }

    #[test]
    fn fan_fire_draws_exactly_four_rands_in_order() {
        let cossin = precompute_cossin();
        let fan = fan_weapon(3);
        let mut worm = firing_worm(150);
        let mut pool: Pool<WObject> = Pool::new(600);
        let mut rand = seeded();

        worm_fire(&mut worm, &[fan], &cossin, false, &mut rand, &mut pool);

        // A reference Rand advanced by EXACTLY the four fan draws (spread x,
        // spread y, colour, time-var). leave_shells = 0 -> no fifth/leading draw.
        // bound() consumes one next_u32 each, so last() matches iff worm_fire drew
        // exactly four times, in this order.
        let mut refr = seeded();
        refr.bound(24000);
        refr.bound(24000);
        refr.bound(2);
        refr.bound(10);
        assert_eq!(
            rand.last(),
            refr.last(),
            "fan must draw exactly 4 rands (no leave-shell draw)"
        );
        // leave_shells = 0 -> the shell branch never ran.
        assert_eq!(
            worm.leave_shell_timer, 0,
            "no leave-shell draw/timer for fan"
        );
    }

    #[test]
    fn fan_fire_updates_worm_ammo_delay_firecone_and_recoil() {
        let cossin = precompute_cossin();
        let fan = fan_weapon(7);
        let mut worm = firing_worm(150);
        let pre_vel = worm.vel;
        let mut pool: Pool<WObject> = Pool::new(600);
        let mut rand = seeded();

        worm_fire(&mut worm, &[fan], &cossin, false, &mut rand, &mut pool);

        assert_eq!(worm.weapons[0].ammo, 149, "ammo decremented");
        assert_eq!(worm.weapons[0].delay_left, 0, "delay_left = w.delay (0)");
        assert_eq!(worm.fire_cone, 0, "fire_cone = w.fire_cone (0)");

        // vel -= cossin[32] * recoil(2) / 100, AFTER the parts loop. The recoil
        // subtracts from the PRE-fire vel (the loop does not touch worm.vel).
        let expected = pre_vel.sub(cossin[32].mul(2).div(100));
        assert_eq!(worm.vel, expected, "recoil applied after parts loop");
    }

    // ---- Step 2: affect_by_worm + HSignedRecoil ------------------------------

    // A synthetic weapon with NO RNG draws (distribution 0, start_frame >= 0 with
    // shot_type 0 + loop_anim false, time_to_explo_v 0) so obj.vel is exactly the
    // deterministic base — isolating the affect_by_worm / recoil arithmetic.
    fn synth_weapon(id: i32, speed: i32, recoil: i32, affect_by_worm: bool) -> Weapon {
        Weapon {
            id,
            speed,
            recoil,
            affect_by_worm,
            distribution: 0,
            parts: 1,
            delay: 0,
            fire_cone: 0,
            detect_distance: 1,
            time_to_explo: 45,
            time_to_explo_v: 0,
            start_frame: 0,
            shot_type: ST_NORMAL,
            loop_anim: false,
            num_frames: 0,
            color_bullets: 25,
            leave_shells: 0,
            ..Default::default()
        }
    }

    #[test]
    fn affect_by_worm_clamps_speed_and_carries_firing_vel() {
        let cossin = precompute_cossin();
        // speed 50 < 100 -> clamps to 100; firing_vel = vel * 100 / 100 = vel.
        let w = synth_weapon(2, 50, 0, true);
        let mut worm = firing_worm(10);
        let pre_vel = worm.vel;
        let mut pool: Pool<WObject> = Pool::new(8);
        let mut rand = seeded();

        worm_fire(&mut worm, &[w], &cossin, false, &mut rand, &mut pool);

        let obj = *pool.get(0).unwrap();
        // speed clamped to 100: base = cossin[32]*100/100 + vel = cossin[32] + vel.
        let firing_vel = pre_vel.mul(100).div(100);
        let expected = cossin[32].mul(100).div(100).add(firing_vel);
        assert_eq!(obj.vel, expected, "speed clamped to 100, firing_vel = vel");
        // No RNG consumed by this synthetic weapon.
        assert_eq!(
            rand.last(),
            0,
            "no rand drawn (distribution/v/frame all skip)"
        );
    }

    #[test]
    fn no_affect_by_worm_leaves_firing_vel_zero_and_speed_untouched() {
        let cossin = precompute_cossin();
        let w = synth_weapon(2, 200, 0, false);
        let mut worm = firing_worm(10);
        let mut pool: Pool<WObject> = Pool::new(8);
        let mut rand = seeded();

        worm_fire(&mut worm, &[w], &cossin, false, &mut rand, &mut pool);

        let obj = *pool.get(0).unwrap();
        // firing_vel = 0, speed unchanged (200): base = cossin[32]*200/100.
        let expected = cossin[32].mul(200).div(100);
        assert_eq!(
            obj.vel, expected,
            "no affect_by_worm: firing_vel 0, speed kept"
        );
    }

    #[test]
    fn signed_recoil_hack_subtracts_256_when_recoil_ge_128() {
        let cossin = precompute_cossin();
        // recoil 200 >= 128. Hack ON -> recoil 200-256 = -56; hack OFF -> 200.
        let w = synth_weapon(2, 100, 200, false);

        let mut worm_on = firing_worm(10);
        let pre = worm_on.vel;
        let mut pool: Pool<WObject> = Pool::new(8);
        let mut r = seeded();
        worm_fire(&mut worm_on, &[w.clone()], &cossin, true, &mut r, &mut pool);
        let expected_on = pre.sub(cossin[32].mul(-56).div(100));
        assert_eq!(worm_on.vel, expected_on, "hack on: recoil -= 256 -> -56");

        let mut worm_off = firing_worm(10);
        let mut pool2: Pool<WObject> = Pool::new(8);
        let mut r2 = seeded();
        worm_fire(&mut worm_off, &[w], &cossin, false, &mut r2, &mut pool2);
        let expected_off = pre.sub(cossin[32].mul(200).div(100));
        assert_eq!(worm_off.vel, expected_off, "hack off: recoil stays 200");

        assert_ne!(
            worm_on.vel, worm_off.vel,
            "the HSignedRecoil branch must change the recoil sign"
        );
    }

    #[test]
    fn fan_recoil_two_is_below_signed_recoil_threshold() {
        // Fan recoil = 2 < 128, so the hack is a no-op even when enabled: the
        // worm vel is identical with the hack on or off.
        let cossin = precompute_cossin();
        let fan = fan_weapon(7);

        let mut worm_on = firing_worm(150);
        let mut p1: Pool<WObject> = Pool::new(8);
        let mut r1 = seeded();
        worm_fire(
            &mut worm_on,
            &[fan.clone()],
            &cossin,
            true,
            &mut r1,
            &mut p1,
        );

        let mut worm_off = firing_worm(150);
        let mut p2: Pool<WObject> = Pool::new(8);
        let mut r2 = seeded();
        worm_fire(&mut worm_off, &[fan], &cossin, false, &mut r2, &mut p2);

        assert_eq!(
            worm_on.vel, worm_off.vel,
            "fan recoil 2 < 128: HSignedRecoil is a no-op"
        );
    }

    #[test]
    fn weapon_fire_returns_spawned_slot() {
        let cossin = precompute_cossin();
        let fan = fan_weapon(7);
        let mut pool: Pool<WObject> = Pool::new(8);
        let mut rand = seeded();
        let slot = weapon_fire(
            &fan,
            32,
            Vec2::zero(),
            fan.speed,
            Vec2::new(1, 2),
            1,
            &cossin,
            &mut rand,
            &mut pool,
        );
        assert_eq!(slot, Some(0), "spawn returns the slot index (Some in 4a)");
    }
}
