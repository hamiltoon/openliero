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

use assets::object::{NObjectType, SObjectType, Weapon};
use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::fixed::{ftoi, itof};
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::blit::draw_dirt_effect;
use crate::pool::Pool;
use crate::sobject::sobject_create;
use crate::state::{LevelSim, NObject, SObject, WObject, WormState};

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

/// The verdict a single [`wobject_process`] pass returns to the driver
/// (Task 3), mirroring the `do_explode` / `do_remove` flags at the tail of C++
/// `WObject::Process` (`weapon.cpp:328-335`):
///
/// * [`Keep`](WObjectOutcome::Keep) — the projectile lives on (no flag set).
/// * [`Explode`](WObjectOutcome::Explode) — `do_explode`: the driver calls
///   [`blow_up`] then frees the slot.
/// * [`Remove`](WObjectOutcome::Remove) — `do_remove`: the driver frees the
///   slot **without** exploding (the `worm_collide` path). Never produced for
///   fan in 4a — the worm-hit loop is deferred — but part of the contract Task
///   3 consumes.
///
/// Splitting the verdict out (instead of freeing inside `Process`) keeps
/// `wobject_process` free of the pool: the C++ frees `this` mid-iteration, which
/// Rust's borrow checker forbids while the driver still holds the pool, so the
/// free-during-iteration is the driver's job.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WObjectOutcome {
    Keep,
    Explode,
    Remove,
}

/// Port of the single non-laser pass of `WObject::Process` (`weapon.cpp:127-338`)
/// for the **fan** (ST_NORMAL) and **dart** (ST_TYPE1) projectile shapes — the two
/// share an identical per-tick flight (see the `shot_type` guard below).
///
/// Advances one wobject by one tick: integrate `pos += vel`, clamp `pos` at the
/// level edges, test the next-step cell for a ground collision, apply gravity in
/// free air, and run the explosion-timer countdown. Returns the
/// [`WObjectOutcome`]; the driver (Task 3) performs the [`blow_up`] + `Pool::free`
/// when it is [`Explode`](WObjectOutcome::Explode)/[`Remove`](WObjectOutcome::Remove).
///
/// **Collision uses `inside`/`dirt_rock`, NOT `checked_mat_background`.** The
/// worm-physics probe wraps a negative `x` into a wrong-row in-range pixel; the
/// projectile collision instead tests `Inside` *first* (a true range check) and
/// only then reads `DirtRock`, so a projectile leaving the level never reads a
/// wrapped cell (`weapon.cpp:249`).
///
/// **`inew` is computed once, before the clamp, and reused.** C++ computes
/// `inew_pos = Ftoi(pos + vel)` at line 234, clamps `pos` against it (lines
/// 236-247) **without recomputing**, then feeds the *same* `inew` into the
/// collision test (line 249). The clamp mutates `pos`; `inew` is frozen — that
/// ordering is load-bearing, so we mirror it exactly.
///
/// Deferred / inert branches (guarded by `debug_assert!` so a non-fan config
/// trips loudly, or omitted because they need state the driver owns):
/// steering (`shot_type` 2/3) and the laser do-loop, `bounce`, `mult_speed`,
/// object/particle trails, and projectile animation are all `debug_assert`ed to
/// their fan-shaped no-op values. The `collide_with_objects` impulse loop and
/// the worm-hit loop need the object pools / worm list and draw no RNG under the
/// 4a single-shot scenario (self-skip, worms out of range), so they are omitted
/// here and land in 4b/4c with the driver. The `RemExp` early-explode block
/// (`weapon.cpp:138-142`, gated on the `HRemExp` hack AND the weapon being the
/// configurable `RemExpObject` LC slot) is likewise omitted: fan is not the
/// `RemExpObject` weapon, so it is inert here (differential-proven over 93 ticks);
/// port it when a slice exercises `RemExpObject`.
pub fn wobject_process(
    obj: &mut WObject,
    level: &LevelSim,
    weapon: &Weapon,
    _rand: &mut Rand,
) -> WObjectOutcome {
    // Deferred-branch guards (4b/4c). Fan and dart satisfy every one (the dart
    // trips only the shot_type guard, now relaxed to admit ST_TYPE1); a config
    // that would take an un-ported branch fails loudly in debug builds.
    // ST_NORMAL and ST_TYPE1 (the dart) share an identical per-tick flight: the
    // C++ Process do-while body runs once for any non-laser, and the only
    // shot_type-specific branches are the steering ones (2/3) and the laser loop
    // guard (4) — ST_TYPE1 takes none of them and draws no rng (weapon.cpp:144-167,
    // 336). ST_TYPE2/STEERABLE/laser STAY deferred.
    debug_assert!(
        weapon.shot_type == ST_NORMAL || weapon.shot_type == ST_TYPE1,
        "steerable/type2/laser Process branches deferred (4b/4c)"
    );
    debug_assert!(weapon.bounce == 0, "bounce Process branch deferred (4b/4c)");
    debug_assert!(
        weapon.mult_speed == 100,
        "mult_speed Process branch deferred (4b/4c)"
    );
    debug_assert!(
        weapon.obj_trail_type < 0,
        "object-trail spawn deferred (4b/4c)"
    );
    debug_assert!(
        weapon.part_trail_obj < 0,
        "particle-trail spawn deferred (4b/4c)"
    );
    debug_assert!(
        weapon.num_frames == 0,
        "projectile animation deferred (4b/4c)"
    );

    let mut do_explode = false;

    // do { ... } while (shot_type == kStLaser && ...): fan is not a laser, so the
    // body runs exactly once.

    // pos += vel.
    obj.pos = obj.pos.add(obj.vel);

    // The collide_with_objects impulse loop (weapon.cpp:212-232) and the worm-hit
    // loop (287-326) go here in C++; omitted (driver-owned + inert for one shot;
    // no RNG drawn under the scenario). See the doc-comment.

    // Boundary clamp (weapon.cpp:234-247). inew = Ftoi(pos + vel), computed ONCE
    // and reused by the collision test; the clamp below mutates pos, not inew.
    // wrapping_add + arithmetic-shift Ftoi match C++'s two's-complement `pos+vel`
    // and signed `>>`.
    let inew_x = ftoi(obj.pos.x.wrapping_add(obj.vel.x));
    let inew_y = ftoi(obj.pos.y.wrapping_add(obj.vel.y));
    if inew_x < 0 {
        obj.pos.x = 0;
    }
    if inew_y < 0 {
        obj.pos.y = 0;
    }
    if inew_x >= level.width {
        obj.pos.x = itof(level.width - 1);
    }
    if inew_y >= level.height {
        obj.pos.y = itof(level.height - 1);
    }

    // Ground collision vs free air (weapon.cpp:249-279).
    if !level.inside(inew_x, inew_y) || level.dirt_rock(inew_x, inew_y) {
        if weapon.bounce == 0 {
            if weapon.expl_ground {
                do_explode = true;
            } else {
                obj.vel = Vec2::zero();
            }
        }
    } else {
        // Free air: apply gravity (fan gravity 0 -> no-op). The num_frames
        // animation that follows in C++ is deferred (guarded above).
        obj.vel.y = obj.vel.y.wrapping_add(weapon.gravity);
    }

    // Explosion timer (weapon.cpp:281-285): the decrement only happens when
    // time_to_explo > 0, and an underflow past 0 explodes.
    if weapon.time_to_explo > 0 {
        obj.time_left -= 1;
        if obj.time_left < 0 {
            do_explode = true;
        }
    }

    if do_explode {
        WObjectOutcome::Explode
    } else {
        WObjectOutcome::Keep
    }
}

/// Port of `WObject::BlowUpObject` (`weapon.cpp:78-125`) — the `create_on_exp`
/// sobject spawn (`weapon.cpp:89-92`) followed by the `dirt_effect` crater branch
/// (`weapon.cpp:117-124`).
///
/// In C++ this frees the wobject, then (conditionally) spawns a `create_on_exp`
/// sobject, plays the explosion sound, scatters `splinter_amount` nobjects, and
/// applies a `dirt_effect` crater. The actual `Pool::free` is the driver's job
/// (it frees the slot after this returns), and the sound is a render-only side
/// effect with no sim/RNG impact, so it is omitted.
///
/// **`create_on_exp` is now live** (Slice-4c Task 4): when `create_on_exp >= 0`
/// it calls [`sobject_create`] for `sobject_types[create_on_exp]` at
/// `Ftoi(pos.x), Ftoi(pos.y)` (`weapon.cpp:90-91` `Create(game, Ftoi(kX),
/// Ftoi(kY), cause_idx, fired_by, this)`; the `-8` centre→top-left offset is
/// applied INSIDE `sobject_create`, NOT here). This runs **after** `Free(this)`
/// (the driver's job) and **before** the dart's own `dirt_effect` branch — the
/// C++ order is load-bearing because the sobject's own dirt-throw / crater draw
/// their RNG between the two. `owner_idx` is the exploding wobject's `owner_idx`
/// (C++ `cause_idx == fired_by`).
///
/// The **`dirt_effect` branch is live** (Slice-4b): when the DART's own
/// `dirt_effect >= 0` it calls [`draw_dirt_effect`] to carve a 16x16 crater
/// centred on the wobject, with the C++ `Ftoi(x) - 7, Ftoi(y) - 7` top-left
/// offset ([`ftoi`] is the arithmetic `>> 16`). This is where greenball-style
/// explosions (dirt_effect=6) destroy terrain and draw their `rand(rframe)`.
/// **`CorrectShadow` is omitted (O4)** — the dumper sets `settings->shadow =
/// false`, so it never runs.
///
/// Branch behaviour by weapon:
/// * **fan** (`create_on_exp = -1`, `dirt_effect = -1`) — both branches skipped:
///   inert, draws no RNG, writes no `material_id`. The 4a path is preserved.
/// * **greenball** (`create_on_exp = -1`, `dirt_effect = 6`) — only the crater
///   fires: a crater is stamped and exactly one `rand(rframe)` is drawn (4b).
/// * **dart** (`create_on_exp = 2`) — the sobject spawn fires: `small_explosion`
///   runs its full sound / dirt-throw / crater cluster via [`sobject_create`].
///
/// The `splinter_amount` scatter (+ its RNG) is still `debug_assert`ed off
/// (deferred to O9) so a config that would take that un-ported branch trips
/// loudly in debug builds.
#[allow(clippy::too_many_arguments)]
pub fn blow_up(
    weapon: &Weapon,
    level: &mut LevelSim,
    large_sprites: &SpriteSet,
    textures: &[Texture],
    pos: Vec2,
    owner_idx: i32,
    sobject_types: &[SObjectType],
    nobject_types: &[NObjectType],
    cossin: &[Vec2; 128],
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    weapons: &[Weapon],
    nobjects: &mut Pool<NObject>,
    sobjects: &mut Pool<SObject>,
    rand: &mut Rand,
) {
    debug_assert!(
        weapon.splinter_amount <= 0,
        "splinter scatter (+ its rng) deferred (O9)"
    );

    // :89-92 create-on-explosion — BEFORE the dart's own dirt_effect (the order
    // is load-bearing: the sobject's sound/dirt-throw/crater draw RNG between the
    // sobject spawn and the dart crater). Pass `Ftoi(pos.x), Ftoi(pos.y)`; the
    // `-8` centre→top-left offset is applied inside sobject_create.
    if weapon.create_on_exp >= 0 {
        sobject_create(
            &sobject_types[weapon.create_on_exp as usize],
            ftoi(pos.x),
            ftoi(pos.y),
            owner_idx,
            worms,
            wobjects,
            weapons,
            nobjects,
            nobject_types,
            level,
            cossin,
            large_sprites,
            textures,
            sobjects,
            rand,
        );
    }

    if weapon.dirt_effect >= 0 {
        draw_dirt_effect(
            level,
            large_sprites,
            textures,
            weapon.dirt_effect,
            ftoi(pos.x) - 7,
            ftoi(pos.y) - 7,
            rand,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        LevelSim, WeaponInit, WormInit, MAT_BACKGROUND, MAT_DIRT, MAT_ROCK, NUM_WEAPONS,
    };
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

    // ====================================================================
    // wobject_process + blow_up (Task 2)
    // ====================================================================

    // A large, all-background level: every cell is material 0 with no flags, so
    // `dirt_rock` is false everywhere in range and `inside` is true for the test
    // positions. Lets a projectile fly free so only timeout/explicit collision
    // pins an outcome.
    fn air_level() -> LevelSim {
        LevelSim {
            width: 1000,
            height: 1000,
            material_id: vec![0u8; 1000 * 1000],
            material_flags: [0u8; 256],
        }
    }

    // A 20x20 level with a single rock cell at (10,10) (idx 10 + 10*20 = 210).
    // Material 1 carries the kRock flag -> DirtRock; everything else is empty.
    fn floor_level() -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[1] = MAT_ROCK;
        let mut material_id = vec![0u8; 20 * 20];
        material_id[10 + 10 * 20] = 1; // (10,10) is solid
        LevelSim {
            width: 20,
            height: 20,
            material_id,
            material_flags,
        }
    }

    // A synthetic projectile weapon shaped like fan's *Process* path (shot_type
    // normal, no bounce, mult_speed 100, no trails, no animation) but with the
    // collision knobs under test set explicitly. obj_trail_type / part_trail_obj
    // are -1 (the "no trail" sentinel) so the deferred-branch debug_asserts in
    // wobject_process are satisfied.
    fn proc_weapon(expl_ground: bool, gravity: i32, time_to_explo: i32) -> Weapon {
        Weapon {
            id: 1,
            shot_type: ST_NORMAL,
            bounce: 0,
            mult_speed: 100,
            gravity,
            expl_ground,
            time_to_explo,
            num_frames: 0,
            obj_trail_type: -1,
            part_trail_obj: -1,
            ..Default::default()
        }
    }

    // ---- Step 1: movement + gravity -----------------------------------------

    #[test]
    fn fan_free_flight_is_a_straight_line_with_constant_velocity() {
        // Fan gravity = 0, so on a free-flight tick vel is unchanged and pos
        // advances by exactly vel each tick: a straight line.
        let fan = fan_weapon(7);
        assert_eq!(fan.gravity, 0, "fan gravity is 0");
        let level = air_level();
        let mut rand = seeded();

        let vel = Vec2::new(itof(3), itof(-1));
        let mut obj = WObject {
            pos: Vec2::new(itof(100), itof(200)),
            vel,
            time_left: 100, // well above the tick count -> no timeout
            ty: Some(fan.id),
            ..WObject::default()
        };

        let mut expected = obj.pos;
        for tick in 0..3 {
            let out = wobject_process(&mut obj, &level, &fan, &mut rand);
            assert_eq!(out, WObjectOutcome::Keep, "tick {tick} keeps the object");
            expected = expected.add(vel);
            assert_eq!(obj.pos, expected, "pos advanced by vel on tick {tick}");
            assert_eq!(obj.vel, vel, "gravity 0 -> vel unchanged on tick {tick}");
        }
    }

    // ---- Step 2: boundary clamp (weapon.cpp:234-247) ------------------------

    #[test]
    fn boundary_clamp_pins_pos_to_each_edge() {
        // expl_ground false + bounce 0: an out-of-level inew zeroes vel (no
        // explode), so we can read back the clamped pos. inew is computed from
        // the already-moved pos PLUS vel again, so it overshoots the edge.
        let w = proc_weapon(false, 0, 0); // 10x10 air level below
        let level = LevelSim {
            width: 10,
            height: 10,
            material_id: vec![0u8; 100],
            material_flags: [0u8; 256],
        };

        let run = |pos: Vec2, vel: Vec2| -> WObject {
            let mut obj = WObject {
                pos,
                vel,
                time_left: 100,
                ty: Some(w.id),
                ..WObject::default()
            };
            let mut rand = seeded();
            wobject_process(&mut obj, &level, &w, &mut rand);
            obj
        };

        // Right edge: inew.x >= width -> pos.x = Itof(width-1).
        let r = run(Vec2::new(itof(9), itof(5)), Vec2::new(itof(5), 0));
        assert_eq!(r.pos.x, itof(9), "right edge clamps pos.x to Itof(width-1)");

        // Left edge: inew.x < 0 -> pos.x = 0.
        let l = run(Vec2::new(itof(1), itof(5)), Vec2::new(itof(-5), 0));
        assert_eq!(l.pos.x, 0, "left edge clamps pos.x to 0");

        // Top edge: inew.y < 0 -> pos.y = 0.
        let t = run(Vec2::new(itof(5), itof(1)), Vec2::new(0, itof(-5)));
        assert_eq!(t.pos.y, 0, "top edge clamps pos.y to 0");

        // Bottom edge: inew.y >= height -> pos.y = Itof(height-1).
        let b = run(Vec2::new(itof(5), itof(9)), Vec2::new(0, itof(5)));
        assert_eq!(
            b.pos.y,
            itof(9),
            "bottom edge clamps pos.y to Itof(height-1)"
        );
    }

    // ---- Step 3: ground collision explode vs air (weapon.cpp:249-258) -------

    #[test]
    fn dirt_rock_collision_with_expl_ground_returns_explode() {
        // inew lands on the rock cell (10,10). bounce 0 + expl_ground -> Explode.
        let w = proc_weapon(true, 0, 0);
        let level = floor_level();
        let mut rand = seeded();

        // pos += vel -> (9,10); inew = Ftoi(pos+vel) = (10,10) = the rock cell.
        let mut obj = WObject {
            pos: Vec2::new(itof(8), itof(10)),
            vel: Vec2::new(itof(1), 0),
            time_left: 100,
            ty: Some(w.id),
            ..WObject::default()
        };
        let out = wobject_process(&mut obj, &level, &w, &mut rand);
        assert_eq!(
            out,
            WObjectOutcome::Explode,
            "DirtRock + expl_ground -> Explode"
        );
    }

    #[test]
    fn air_tick_adds_gravity_and_keeps() {
        // inew lands on empty space -> air branch: vel.y += gravity, no explode.
        let w = proc_weapon(true, 1000, 0);
        let level = floor_level();
        let mut rand = seeded();

        let mut obj = WObject {
            pos: Vec2::new(itof(2), itof(2)),
            vel: Vec2::new(itof(1), 0),
            time_left: 100,
            ty: Some(w.id),
            ..WObject::default()
        };
        let out = wobject_process(&mut obj, &level, &w, &mut rand);
        assert_eq!(out, WObjectOutcome::Keep, "free air -> Keep");
        assert_eq!(obj.vel.y, 1000, "air branch adds gravity to vel.y");
    }

    #[test]
    fn dirt_rock_collision_without_expl_ground_zeroes_velocity() {
        // bounce 0, expl_ground false: a ground hit zeroes vel instead of exploding.
        let w = proc_weapon(false, 0, 0);
        let level = floor_level();
        let mut rand = seeded();

        let mut obj = WObject {
            pos: Vec2::new(itof(8), itof(10)),
            vel: Vec2::new(itof(1), 0),
            time_left: 100,
            ty: Some(w.id),
            ..WObject::default()
        };
        let out = wobject_process(&mut obj, &level, &w, &mut rand);
        assert_eq!(out, WObjectOutcome::Keep, "no expl_ground -> Keep");
        assert_eq!(
            obj.vel,
            Vec2::zero(),
            "ground hit without expl_ground zeroes vel"
        );
    }

    // ---- Step 4: timeout explode (weapon.cpp:281-285) -----------------------

    #[test]
    fn timeout_explodes_when_time_left_goes_negative() {
        let fan = fan_weapon(7);
        assert!(
            fan.time_to_explo > 0,
            "fan time_to_explo gates the countdown"
        );
        let level = air_level();

        // time_left 0 -> --time_left = -1 < 0 -> Explode this tick.
        let mut at_zero = WObject {
            pos: Vec2::new(itof(100), itof(100)),
            vel: Vec2::new(itof(1), 0),
            time_left: 0,
            ty: Some(fan.id),
            ..WObject::default()
        };
        let mut rand = seeded();
        assert_eq!(
            wobject_process(&mut at_zero, &level, &fan, &mut rand),
            WObjectOutcome::Explode,
            "time_left 0 -> explodes on this tick"
        );

        // time_left 1 -> --time_left = 0, not < 0 -> Keep, counter now 0.
        let mut at_one = WObject {
            pos: Vec2::new(itof(100), itof(100)),
            vel: Vec2::new(itof(1), 0),
            time_left: 1,
            ty: Some(fan.id),
            ..WObject::default()
        };
        let mut rand2 = seeded();
        assert_eq!(
            wobject_process(&mut at_one, &level, &fan, &mut rand2),
            WObjectOutcome::Keep,
            "time_left 1 -> survives this tick"
        );
        assert_eq!(at_one.time_left, 0, "time_left decremented to 0");
    }

    // ---- Step 5: inert guarded branches draw NO rng -------------------------

    #[test]
    fn fan_process_draws_no_rng() {
        // A free-flight fan tick (no collision, no timeout) must not touch the
        // RNG: the bounce branch (bounce 0), the collide-with-objects loop (no
        // pool walk here) and the worm-hit loop (no worms) are all inert. We
        // pre-advance the RNG, snapshot last(), and assert it is unchanged.
        let fan = fan_weapon(7);
        let level = air_level();
        let mut rand = seeded();
        rand.bound(1000);
        rand.bound(1000);
        let last_before = rand.last();

        let mut obj = WObject {
            pos: Vec2::new(itof(100), itof(100)),
            vel: Vec2::new(itof(2), 0),
            time_left: 100,
            ty: Some(fan.id),
            ..WObject::default()
        };
        let out = wobject_process(&mut obj, &level, &fan, &mut rand);

        assert_eq!(out, WObjectOutcome::Keep, "free flight keeps");
        assert_eq!(
            rand.last(),
            last_before,
            "fan Process draws no rng (rand.last unchanged)"
        );
        assert_eq!(obj.vel, Vec2::new(itof(2), 0), "gravity 0 -> vel unchanged");
    }

    // ---- Step 6: real dart (shot_type = 1) flows through Process -------------

    // The REAL shipped dart, loaded from the TC config. shot_type = 1 (ST_TYPE1).
    // Cross-ref lists are empty: none of the *Process* fields (shot_type, bounce,
    // mult_speed, gravity, expl_ground, time_to_explo, num_frames, the trail
    // delays) depend on a cross-ref, and the two trail object refs are ABSENT from
    // dart.cfg so they resolve via the empty-string sentinel to -1 regardless of
    // the lists. `createOnExp = "small_explosion"` resolves to 0 here, but
    // create_on_exp is unused by `wobject_process`, so the flight is unaffected.
    fn dart_weapon_real(id: i32) -> Weapon {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/weapons/dart.cfg"
        ));
        let mut w = Weapon::load(bytes, &[], &[], &[]).unwrap();
        w.id = id;
        w
    }

    #[test]
    fn dart_resolved_process_fields_trip_only_the_shot_type_guard() {
        // Empirical resolution of the REAL dart: it is shot_type = 1 (ST_TYPE1),
        // so it trips ONLY the relaxed shot_type guard. Every OTHER deferred-branch
        // guard is satisfied by the resolved config — in particular both trail
        // object refs resolve to the -1 "no trail" sentinel (dart.cfg omits
        // objTrailType/partTrailObj -> "" -> obj_ref_from_str -> -1), even though
        // the trail DELAYS are 0. So no trail guard needs relaxing.
        let dart = dart_weapon_real(1);
        assert_eq!(dart.shot_type, ST_TYPE1, "dart is shot_type = 1 (ST_TYPE1)");
        assert_eq!(dart.bounce, 0, "dart bounce = 0 (guard satisfied)");
        assert_eq!(dart.mult_speed, 100, "dart mult_speed = 100 (guard satisfied)");
        assert_eq!(dart.num_frames, 0, "dart num_frames = 0 (guard satisfied)");
        assert_eq!(
            dart.obj_trail_type, -1,
            "dart obj_trail_type = -1 (no obj trail; guard satisfied)"
        );
        assert_eq!(
            dart.part_trail_obj, -1,
            "dart part_trail_obj = -1 (no part trail; guard satisfied)"
        );
        // Flight knobs that differ from fan but are already handled by Process.
        assert_eq!(dart.gravity, 200, "dart gravity = 200");
        assert_eq!(
            dart.time_to_explo, 0,
            "dart time_to_explo = 0 -> no countdown (guard gates it)"
        );
        assert!(dart.expl_ground, "dart expl_ground = true");
    }

    #[test]
    fn dart_flows_through_process_like_a_normal_projectile() {
        // The REAL dart (shot_type = 1) must flow through wobject_process WITHOUT
        // panicking on the (now relaxed) shot_type guard and arc under gravity
        // exactly as a NORMAL projectile would — the C++ Process do-while body runs
        // once for any non-laser, and ST_TYPE1 takes none of the shot_type-specific
        // branches (the steering branches are 2/3, the laser is the loop guard).
        // cur_frame is set deterministically at Fire by the ST_TYPE1 clamp; with
        // num_frames = 0 the animation never runs, so Process leaves it FROZEN.
        let dart = dart_weapon_real(1);
        let level = air_level();

        // A Fire-set cur_frame in the ST_TYPE1 clamp range [0, 12] — a DETERMINISTIC
        // value, NOT a colour-rand value (dart color_bullets = 0, so the colour-rand
        // path would have produced 0 - rand(2); shot_type = 1 never takes it).
        const FIRE_CUR_FRAME: i32 = 7;
        let vel0 = Vec2::new(itof(3), itof(-2));
        let mut obj = WObject {
            pos: Vec2::new(itof(100), itof(200)),
            vel: vel0,
            cur_frame: FIRE_CUR_FRAME,
            time_left: 0, // dart time_to_explo = 0 -> never decremented, never times out
            ty: Some(dart.id),
            ..WObject::default()
        };

        // Process must draw NO rng for this config (no steering rand, no
        // particle-trail rand, no worm-hit rand). Pre-advance, snapshot, compare.
        let mut rand = seeded();
        rand.bound(1000);
        let last_before = rand.last();

        // Independent expected arc, identical to the fan/ST_NORMAL flight path
        // (see fan_free_flight_*): pos += vel using the CURRENT vel, THEN free-air
        // gravity is added to vel.y for the next tick.
        let mut exp_pos = obj.pos;
        let mut exp_vel = vel0;
        for tick in 0..5 {
            let out = wobject_process(&mut obj, &level, &dart, &mut rand);
            assert_eq!(
                out,
                WObjectOutcome::Keep,
                "dart free flight keeps on tick {tick}"
            );
            exp_pos = exp_pos.add(exp_vel);
            exp_vel.y = exp_vel.y.wrapping_add(dart.gravity);
            assert_eq!(obj.pos, exp_pos, "dart pos arcs under gravity on tick {tick}");
            assert_eq!(obj.vel, exp_vel, "dart vel gains gravity on tick {tick}");
            assert_eq!(
                obj.cur_frame, FIRE_CUR_FRAME,
                "num_frames = 0 -> cur_frame frozen on tick {tick}"
            );
        }

        // No rng drawn: the dart's Process path is rand-free, identical to fan.
        assert_eq!(
            rand.last(),
            last_before,
            "dart Process draws no rng (the _rand param is unused for this config)"
        );
        assert_eq!(
            obj.cur_frame, FIRE_CUR_FRAME,
            "cur_frame stayed the deterministic Fire value across all ticks"
        );
    }

    // ---- blow_up: dirt_effect branch (greenball) + fan regression ----------

    const SPRITE_SIZE: usize = 256; // 16 x 16

    // A SpriteSet of `count` 16x16 sprites with each (index, bytes) override laid
    // over an all-zero bank (mirrors blit.rs's test helper).
    fn make_sprites(count: i32, overrides: &[(usize, Vec<u8>)]) -> SpriteSet {
        let mut data = vec![0u8; count as usize * SPRITE_SIZE];
        for (idx, bytes) in overrides {
            assert_eq!(bytes.len(), SPRITE_SIZE);
            data[idx * SPRITE_SIZE..idx * SPRITE_SIZE + SPRITE_SIZE].copy_from_slice(bytes);
        }
        SpriteSet {
            width: 16,
            height: 16,
            count,
            data,
        }
    }

    // The shipped greenball weapon shape relevant to blow_up: dirt_effect = 6
    // (indexes the texture table), create_on_exp = -1, splinter_amount = 0 so the
    // ONLY rng blow_up draws is draw_dirt_effect's rand(rframe).
    fn greenball_weapon() -> Weapon {
        Weapon {
            id: 6,
            dirt_effect: 6,
            create_on_exp: -1,
            splinter_amount: 0,
            ..Default::default()
        }
    }

    #[test]
    fn greenball_blow_up_writes_terrain_and_draws_one_rand() {
        // Greenball: dirt_effect = 6 -> blow_up stamps a 16x16 crater at
        // (Ftoi(pos.x)-7, Ftoi(pos.y)-7) via draw_dirt_effect.
        let weapon = greenball_weapon();

        // mask 38 = all case-6 (fill every Background cell); fill frames 82=const
        // 200, 83=const 201, so the written value reveals which frame was picked.
        let sprites = make_sprites(
            84,
            &[
                (38, vec![6u8; SPRITE_SIZE]),
                (82, vec![200u8; SPRITE_SIZE]),
                (83, vec![201u8; SPRITE_SIZE]),
            ],
        );
        // dirt_effect (=6) indexes the texture table; textures[6] is greenball.
        let mut textures = vec![Texture::default(); 7];
        textures[6] = Texture {
            sframe: 82,
            rframe: 2,
            mframe: 38,
            ndrawback: false,
        };

        // Background-above-Dirt boundary: rows < 20 are Background (material 0),
        // rows >= 20 are Dirt (material 5).
        let mut material_flags = [0u8; 256];
        material_flags[0] = MAT_BACKGROUND;
        material_flags[5] = MAT_DIRT;
        let width = 40;
        let height = 40;
        let mut material_id = vec![0u8; (width * height) as usize];
        for y in 20..height {
            for x in 0..width {
                material_id[(y * width + x) as usize] = 5;
            }
        }
        let mut level = LevelSim {
            width,
            height,
            material_id,
            material_flags,
        };

        // pos = (20.5, 20.5) in 16.16. Ftoi TRUNCATES to 20 (not rounds to 21),
        // so the window top-left = (20-7, 20-7) = (13, 13).
        let pos = Vec2::new(itof(20) + 0x8000, itof(20) + 0x8000);
        assert_eq!(ftoi(pos.x), 20, "Ftoi truncates 20.5 -> 20");

        // Oracle: exactly one rand(2) selects the fill frame (82 + draw).
        let mut oracle = seeded();
        let draw = oracle.bound(2);
        let expected_last = oracle.last();
        let fill_val = 200u8.wrapping_add(draw as u8);

        let mut rand = seeded();
        let cossin = precompute_cossin();
        let (mut worms, mut wobjects, mut nobjects, mut sobjects) = blow_up_pools();
        blow_up(
            &weapon,
            &mut level,
            &sprites,
            &textures,
            pos,
            0,
            &[],
            &[],
            &cossin,
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut rand,
        );

        // (a) exactly one rand(2): no create_on_exp / splinter draws.
        assert_eq!(
            rand.last(),
            expected_last,
            "only draw_dirt_effect's rand(2)"
        );
        assert_eq!(
            sobjects.len(),
            0,
            "greenball create_on_exp = -1 -> no sobject spawned"
        );

        // (b) -7,-7 offset + Ftoi truncation: top-left written cell is (13,13);
        // the cells just left/above the window are untouched.
        let at = |x: i32, y: i32| level.material_id[(y * width + x) as usize];
        assert_eq!(
            at(13, 13),
            fill_val,
            "window top-left = (Ftoi-7, Ftoi-7) = (13,13)"
        );
        assert_eq!(at(12, 13), 0, "x=12 is left of the window -> untouched");
        assert_eq!(at(13, 12), 0, "y=12 is above the window -> untouched");

        // (c) Background cells in the window changed; Dirt cells did NOT (the
        // additive-over-Background path only writes Background cells).
        assert_eq!(
            at(28, 19),
            fill_val,
            "last Background cell in-window written"
        );
        assert_eq!(at(13, 20), 5, "first Dirt cell in-window untouched");
        assert_eq!(at(28, 28), 5, "last Dirt cell in-window untouched");
    }

    #[test]
    fn fan_blow_up_is_inert_and_draws_no_rng() {
        // Fan has create_on_exp/dirt_effect = -1 and splinter_amount = 0, so
        // blow_up does nothing for it: no dirt write, no rng. The 4a path is
        // preserved despite the new signature — the dirt_effect branch is skipped
        // for dirt_effect < 0, so the assets are never read.
        let fan = fan_weapon(7);
        assert_eq!(fan.dirt_effect, -1, "fan dirt_effect is the -1 sentinel");

        let sprites = make_sprites(1, &[]);
        let textures: Vec<Texture> = Vec::new();
        let mut level = air_level();
        let before = level.material_id.clone();

        let mut rand = seeded();
        rand.bound(1000);
        let last_before = rand.last();

        let cossin = precompute_cossin();
        let (mut worms, mut wobjects, mut nobjects, mut sobjects) = blow_up_pools();
        blow_up(
            &fan,
            &mut level,
            &sprites,
            &textures,
            Vec2::new(itof(50), itof(50)),
            0,
            &[],
            &[],
            &cossin,
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut rand,
        );

        assert_eq!(
            rand.last(),
            last_before,
            "fan blow_up draws no rng (rand.last unchanged)"
        );
        assert_eq!(
            level.material_id, before,
            "fan blow_up writes no material (4a path preserved)"
        );
        assert_eq!(sobjects.len(), 0, "fan create_on_exp = -1 -> no sobject");
    }

    // ====================================================================
    // blow_up: create_on_exp branch (dart -> small_explosion) — Task 4
    // ====================================================================

    use assets::object::{NObjectType, SObjectType};

    // Empty object pools + worm list for blow_up calls whose `create_on_exp < 0`
    // never touch them (fan / greenball regression guards).
    fn blow_up_pools() -> (Vec<WormState>, Pool<WObject>, Pool<NObject>, Pool<SObject>) {
        (Vec::new(), Pool::new(1), Pool::new(1), Pool::new(1))
    }

    // The shipped small_explosion sobject (the dart's create_on_exp target):
    // start_sound >= 0 (num_sounds 2), anim_delay 2, num_frames 5, detect_range 8
    // (kWidth 4, a 9x9 dirt-throw box), damage > 0, blow_away set. `dirt_effect` is
    // passed in: 2 to exercise the crater, -1 to isolate the dirt-throw.
    fn small_explosion(dirt_effect: i32) -> SObjectType {
        SObjectType {
            id: 2,
            start_sound: 0,
            num_sounds: 2,
            anim_delay: 2,
            start_frame: 0,
            num_frames: 5,
            detect_range: 8,
            damage: 5,
            blow_away: 3000,
            dirt_effect,
            ..Default::default()
        }
    }

    // The dirt particle nobject_types[2]: speed_v 40, distribution 10000, so its
    // Create2 draws exactly rand(40), rand(20000), rand(20000) and Create resolves
    // cur_frame = kPix with no draw.
    fn dirt_nobject() -> NObjectType {
        NObjectType {
            id: 2,
            speed: 100,
            speed_v: 40,
            distribution: 10000,
            start_frame: 0,
            num_frames: 0,
            color_bullets: 0,
            time_to_explo: 0,
            time_to_explo_v: 0,
            ..Default::default()
        }
    }

    // The dart weapon shape relevant to blow_up: create_on_exp = 2 (-> the
    // small_explosion sobject), splinter_amount = 0 (no wobject splinters), and
    // its OWN dirt_effect = -1 (the explosion is wholly delegated to the sobject).
    fn dart_weapon() -> Weapon {
        Weapon {
            id: 1,
            create_on_exp: 2,
            splinter_amount: 0,
            dirt_effect: -1,
            ..Default::default()
        }
    }

    // A level whose entire 16x16 carve window (and the inner 9x9 dirt-throw box)
    // around (cx, cy) is dirt material `dirt_mat`. Carved cells become `fill_mat`.
    fn all_dirt_level(cx: i32, cy: i32, dirt_mat: u8, fill_mat: u8) -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[dirt_mat as usize] = MAT_DIRT;
        material_flags[fill_mat as usize] = MAT_BACKGROUND;
        let width = 80;
        let height = 80;
        let mut material_id = vec![0u8; (width * height) as usize];
        for yy in (cy - 7)..(cy + 9) {
            for xx in (cx - 7)..(cx + 9) {
                material_id[(yy * width + xx) as usize] = dirt_mat;
            }
        }
        LevelSim {
            width,
            height,
            material_id,
            material_flags,
        }
    }

    #[test]
    fn dart_blow_up_spawns_small_explosion_with_full_cluster() {
        // A dart explodes over dirt: blow_up must (a) spawn sobject id=2 at
        // (Ftoi(x)-8, Ftoi(y)-8) — the -8 applied INSIDE sobject_create, proving
        // blow_up passed Ftoi(x),Ftoi(y) and NOT -7/-8; (b) spawn dirt debris from
        // the dirt-throw; (c) carve the level; (d) advance rand by exactly the
        // cluster (sound + per-fired-cell + crater rand(2)); the dart's
        // splinter_amount=0 / dirt_effect=-1 add no draws of their own.
        let cossin = precompute_cossin();
        let weapon = dart_weapon();
        let (cx, cy) = (40, 40);
        let dirt_mat: u8 = 10;
        let fill_mat: u8 = 7;
        let mut level = all_dirt_level(cx, cy, dirt_mat, fill_mat);

        // pos = (40.5, 40.5) in 16.16; Ftoi truncates to 40, so the sobject lands
        // at (40-8, 40-8) and the carve window top-left at (40-7-... )—the -7/-8
        // offsets are inside the called functions.
        let pos = Vec2::new(itof(cx) + 0x8000, itof(cy) + 0x8000);
        assert_eq!(ftoi(pos.x), cx, "Ftoi truncates 40.5 -> 40");

        // Carve texture textures[2]: ndrawback, mframe all case-6, sframe const
        // fill_mat (a Background material) so case-6 clears dirt to fill_mat.
        let sprites = make_sprites(
            84,
            &[
                (38, vec![6u8; SPRITE_SIZE]),
                (82, vec![fill_mat; SPRITE_SIZE]),
                (83, vec![fill_mat; SPRITE_SIZE]),
            ],
        );
        let mut textures = vec![Texture::default(); 3];
        textures[2] = Texture {
            sframe: 82,
            rframe: 2,
            mframe: 38,
            ndrawback: true,
        };

        let mut sobject_types = vec![SObjectType::default(); 3];
        sobject_types[2] = small_explosion(2); // dirt_effect = 2 -> carve LIVE
        let mut nobject_types = vec![NObjectType::default(); 3];
        nobject_types[2] = dirt_nobject();

        // Reference RNG stream: sound rand(2), then row-major over the all-dirt 9x9
        // box (every cell fires rand(8); on 0 -> rand(128), rand(40), rand(20000)x2),
        // then the crater rand(rframe) = rand(2) LAST.
        let mut refr = seeded();
        refr.bound(2); // sound
        let mut fired = 0;
        for _yy in (cy - 4)..(cy + 5) {
            for _xx in (cx - 4)..(cx + 5) {
                if refr.bound(8) == 0 {
                    refr.bound(128);
                    refr.bound(40);
                    refr.bound(20000);
                    refr.bound(20000);
                    fired += 1;
                }
            }
        }
        refr.bound(2); // crater rand(rframe), LAST
        let expected_last = refr.last();

        let mut rand = seeded();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(8);
        let mut nobjects: Pool<NObject> = Pool::new(600);
        let mut sobjects: Pool<SObject> = Pool::new(700);

        blow_up(
            &weapon,
            &mut level,
            &sprites,
            &textures,
            pos,
            3, // owner_idx (= cause_idx / fired_by)
            &sobject_types,
            &nobject_types,
            &cossin,
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut rand,
        );

        // (a) sobject spawned at (Ftoi(x)-8, Ftoi(y)-8): the -8 is inside Create,
        // so x == 32 proves blow_up passed Ftoi(x)=40 (NOT 40-7 or 40-8).
        assert_eq!(sobjects.len(), 1, "dart spawns exactly one sobject");
        let so = *sobjects.get(0).expect("sobject in slot 0");
        assert_eq!(so.id, 2, "sobject id = create_on_exp target id");
        assert_eq!(so.x, cx - 8, "sobject.x = Ftoi(x) - 8 (offset inside Create)");
        assert_eq!(so.y, cy - 8, "sobject.y = Ftoi(y) - 8 (offset inside Create)");

        // (d) exact rand cluster: sound + dirt-throw + crater rand(2). A wrong
        // Ftoi offset / a missing or extra draw shifts rand.last.
        assert_eq!(
            rand.last(),
            expected_last,
            "rand advanced by exactly the small_explosion cluster"
        );

        // (b) dirt debris: one nobject per fired cell, owner_idx carried through.
        assert!(fired > 0, "seed must fire at least one dirt cell");
        assert_eq!(nobjects.len(), fired, "one dirt nobject per fired cell");
        assert_eq!(
            nobjects.get(0).unwrap().owner_idx,
            3,
            "debris carries the dart's owner_idx"
        );
        // PRE-CARVE read: debris colour is the original dirt_mat.
        assert_eq!(
            nobjects.get(0).unwrap().cur_frame,
            dirt_mat as i32,
            "debris cur_frame = PRE-CARVE kPix (dirt_mat)"
        );

        // (c) the crater cleared the box dirt to the fill (Background) material.
        assert_eq!(
            level.material_id[(cy * level.width + cx) as usize],
            fill_mat,
            "crater cleared the box centre dirt to the fill material"
        );
    }

    #[test]
    fn dart_blow_up_with_no_dirt_draws_only_the_sound() {
        // Over a background-only level the small_explosion's dirt-throw fires no
        // rand(8) (short-circuit) and its dirt_effect = -1 draws no crater, so the
        // ONLY draw is the sound rand(2). Proves the create_on_exp branch is wired
        // straight through to sobject_create's sound, with nothing extra.
        let cossin = precompute_cossin();
        let weapon = dart_weapon();

        let mut material_flags = [0u8; 256];
        material_flags[0] = MAT_BACKGROUND;
        let mut level = LevelSim {
            width: 100,
            height: 100,
            material_id: vec![0u8; 100 * 100],
            material_flags,
        };

        let mut sobject_types = vec![SObjectType::default(); 3];
        sobject_types[2] = small_explosion(-1); // no crater -> sound is the only draw
        let mut nobject_types = vec![NObjectType::default(); 3];
        nobject_types[2] = dirt_nobject();

        let mut refr = seeded();
        refr.bound(2); // sound only
        let expected_last = refr.last();

        let mut rand = seeded();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(8);
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut sobjects: Pool<SObject> = Pool::new(8);

        blow_up(
            &weapon,
            &mut level,
            &SpriteSet::default(),
            &[],
            Vec2::new(itof(50), itof(50)),
            2,
            &sobject_types,
            &nobject_types,
            &cossin,
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut rand,
        );

        assert_eq!(sobjects.len(), 1, "sobject spawned");
        assert_eq!(rand.last(), expected_last, "only the sound rand(2) drawn");
        assert_eq!(nobjects.len(), 0, "background level -> no dirt debris");
    }
}
