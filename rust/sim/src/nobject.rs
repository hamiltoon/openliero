//! Port of `NObjectType::Create` / `Create1` / `Create2`
//! (`nobject.cpp:7-66`) — the nobject spawn family, and **the RNG-order core of
//! Slice 4c**.
//!
//! [`nobject_create`] is the shared spawn core (called by both variants);
//! [`nobject_create1`] and [`nobject_create2`] add the velocity scatter in front
//! of it. The dirt-throw (Task 3) spawns its debris via [`nobject_create2`]; the
//! splinter path (O9) spawns via [`nobject_create1`]. **The exact `rand()`
//! consumption order is the contract** — a reordered / extra / missing draw
//! shifts every downstream `rand.last` and desyncs the simulation.
//!
//! ## The two scatter forms are OPPOSITE (and value-bearing — do NOT normalise)
//!
//! Both draw `rand(distribution * 2)` twice, but the operand order differs:
//!
//! * [`nobject_create1`] (`nobject.cpp:44-45`): `vel += distribution - rand(distribution*2)`
//! * [`nobject_create2`] (`nobject.cpp:59-60`): `vel += rand(distribution*2) - distribution`
//!
//! Same draw count, different value — each is ported exactly as written.
//!
//! ## RNG order
//!
//! * [`nobject_create2`] (`:51-66`): `rand(speed_v)` **first** (`:53`, always —
//!   `rand(0)` still advances the engine), then — iff `distribution != 0` — two
//!   `rand(distribution*2)` (`:59-60`), then [`nobject_create`]'s draws, then
//!   `obj.pos += obj.vel` (`:65`, **a one-step advance at birth** that Create /
//!   Create1 do NOT perform).
//! * [`nobject_create1`] (`:41-49`): iff `distribution != 0`, two
//!   `rand(distribution*2)` (`:44-45`) — **no speed draw** (the explicit contrast
//!   with Create2) — then [`nobject_create`]'s draws. No birth step.
//! * [`nobject_create`] (`:7-39`): `rand(num_frames+1)` iff `start_frame > 0`
//!   (`:25`), then `rand(time_to_explo_v)` iff `time_to_explo_v != 0` (`:35`).
//!
//! ## Coverage notes (which paths the dirt-debris exercises vs. don't)
//!
//! For the 4c dirt particle (`nobject_types[2]`, spawned via [`nobject_create2`])
//! the live draws are `rand(speed_v)` + the two `rand(distribution*2)`; inside
//! [`nobject_create`] both branches are **inert** for that type
//! (`start_frame <= 0` so `cur_frame = color`, and `time_to_explo_v == 0`), so
//! Create draws nothing. The paths NOT exercised by the dart —
//! [`nobject_create1`] in full (the splinter path, O9), and the `start_frame > 0`
//! / `time_to_explo_v != 0` draws inside [`nobject_create`] — are ported now
//! (their draw count is part of the contract for other types) and pinned by the
//! synthetic-type unit tests below. They are flagged here as not-yet-live for the
//! dart, rather than `debug_assert`ed off, precisely because those synthetic tests
//! must drive them.
//!
//! Fixed-point: `cossin[angle] * real_speed / 100` uses **truncating** integer
//! division ([`Vec2::div`]); `obj.pos += obj.vel` is a componentwise wrapping add
//! ([`Vec2::add`]). The stats-only `fired_by` / owner-lookup `DamagePotential`
//! calls and the `has_hit` field (none hashed) are no-ops and are omitted, exactly
//! as [`crate::weapon`] omits them.

use assets::object::NObjectType;
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::pool::Pool;
use crate::state::NObject;

/// Port of `NObjectType::Create` (`nobject.cpp:7-39`) — the shared spawn core.
///
/// Allocates one [`NObject`] in `nobjects` (the C++ `NewObjectReuse`), writes
/// `pos`/`vel`/`ty`/`owner_idx`, resolves `cur_frame`, and sets the explode
/// countdown. Returns the spawned slot index.
///
/// Draws RNG in C++ order: `rand(num_frames+1)` **iff `start_frame > 0`**
/// (`:25`), else picks `color` (when non-zero) or `color_bullets` with **no
/// draw** (`:27`/`:29`); then `rand(time_to_explo_v)` **iff `time_to_explo_v != 0`**
/// (`:35`). For the dirt particle both gates are off (`start_frame <= 0`,
/// `time_to_explo_v == 0`) ⇒ no draw here; the `start_frame > 0` and
/// `time_to_explo_v` draws are exercised only by synthetic types in the tests.
pub fn nobject_create(
    ty: &NObjectType,
    vel: Vec2,
    pos: Vec2,
    color: i32,
    owner_idx: i32,
    rand: &mut Rand,
    nobjects: &mut Pool<NObject>,
) -> usize {
    let mut obj = NObject {
        pos,
        vel,
        ty: Some(ty.id),
        owner_idx,
        ..NObject::default()
    };

    // cur_frame branch (nobject.cpp:24-30). rand(num_frames+1) draws even when
    // num_frames == 0 (then rand(1), always 0, but still advances the engine).
    // NOT exercised by the dirt particle (start_frame <= 0); pinned by synthetic
    // types in the unit tests.
    if ty.start_frame > 0 {
        obj.cur_frame = rand.bound((ty.num_frames + 1) as u32) as i32;
    } else if color != 0 {
        obj.cur_frame = color;
    } else {
        obj.cur_frame = ty.color_bullets;
    }

    // time_left = time_to_explo (- rand(time_to_explo_v) when non-zero,
    // nobject.cpp:32-36). The jitter draw is NOT exercised by the dirt particle
    // (time_to_explo_v == 0); pinned by a synthetic type in the unit tests.
    obj.time_left = ty.time_to_explo;
    if ty.time_to_explo_v != 0 {
        obj.time_left -= rand.bound(ty.time_to_explo_v as u32) as i32;
    }

    nobjects
        .spawn(obj)
        .expect("nobject pool not full in 4c (NewObjectReuse overwrite deferred)")
}

/// Port of `NObjectType::Create1` (`nobject.cpp:41-49`) — distribution scatter
/// only, **no speed draw** (the contrast with [`nobject_create2`]).
///
/// Iff `distribution != 0`, draws two `rand(distribution*2)` (x then y) using the
/// **`distribution - rand(...)`** sign form (`:44-45`), then delegates to
/// [`nobject_create`]. Unlike [`nobject_create2`] it does **not** advance the new
/// object by a velocity step at birth.
///
/// This is the splinter spawn path (O9) — not exercised by the dart in 4c, ported
/// now because its draw count and sign form are part of the contract; pinned by
/// the unit tests.
pub fn nobject_create1(
    ty: &NObjectType,
    mut vel: Vec2,
    pos: Vec2,
    color: i32,
    owner_idx: i32,
    rand: &mut Rand,
    nobjects: &mut Pool<NObject>,
) -> usize {
    // distribution scatter (nobject.cpp:44-45): sign form `distribution - rand`.
    if ty.distribution != 0 {
        let dist = ty.distribution;
        let max = (dist * 2) as u32;
        vel.x = vel.x.wrapping_add(dist.wrapping_sub(rand.bound(max) as i32));
        vel.y = vel.y.wrapping_add(dist.wrapping_sub(rand.bound(max) as i32));
    }

    // nobject.cpp:48 — Create (no pos += vel afterwards).
    nobject_create(ty, vel, pos, color, owner_idx, rand, nobjects)
}

/// Port of `NObjectType::Create2` (`nobject.cpp:51-66`) — speed draw FIRST, then
/// distribution scatter, then [`nobject_create`], then a one-step birth advance.
///
/// This is the dirt-throw spawn path (Task 3 — the heart of Slice 4c). RNG order:
///
/// 1. `rand(speed_v)` (`:53`) — **always drawn** (even `speed_v == 0` ⇒ `rand(0)`
///    consumes the engine); `real_speed = speed - rand(speed_v)`.
/// 2. `vel += cossin[angle] * real_speed / 100` (`:55`, truncating `/100`). No rand.
/// 3. iff `distribution != 0`, two `rand(distribution*2)` (`:59-60`) using the
///    **`rand(...) - distribution`** sign form (opposite [`nobject_create1`]).
/// 4. [`nobject_create`]'s draws (`:63`).
/// 5. `obj.pos += obj.vel` (`:65`) — the spawned object steps once at creation.
///
/// Returns the spawned slot index.
#[allow(clippy::too_many_arguments)]
pub fn nobject_create2(
    ty: &NObjectType,
    angle: i32,
    mut vel: Vec2,
    pos: Vec2,
    color: i32,
    owner_idx: i32,
    cossin: &[Vec2; 128],
    rand: &mut Rand,
    nobjects: &mut Pool<NObject>,
) -> usize {
    // :53 FIRST draw: rand(speed_v). real_speed = speed - rand(speed_v).
    let real_speed = ty.speed - rand.bound(ty.speed_v as u32) as i32;

    // :55 vel += cossin[angle] * real_speed / 100 (componentwise, truncating /100).
    vel = cossin[angle as usize].mul(real_speed).div(100).add(vel);

    // distribution scatter (nobject.cpp:59-60): sign form `rand - distribution`.
    if ty.distribution != 0 {
        let dist = ty.distribution;
        let max = (dist * 2) as u32;
        vel.x = vel.x.wrapping_add((rand.bound(max) as i32).wrapping_sub(dist));
        vel.y = vel.y.wrapping_add((rand.bound(max) as i32).wrapping_sub(dist));
    }

    // :63 Create (its own draws), then :65 obj.pos += obj.vel (birth step).
    let slot = nobject_create(ty, vel, pos, color, owner_idx, rand, nobjects);
    let obj = nobjects
        .get_mut(slot)
        .expect("nobject just spawned in this slot");
    obj.pos = obj.pos.add(obj.vel);
    slot
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::fixed::itof;
    use sim_core::tables::precompute_cossin;

    // A seed whose successive draws differ, so an x<->y swap or a sign flip is
    // detectable (asserted explicitly where it matters).
    const SEED: u32 = 0x4242;

    fn seeded() -> Rand {
        let mut r = Rand::new();
        r.seed(SEED);
        r
    }

    // A synthetic type shaped like the real dirt particle (nobject_types[2]):
    // speed_v = 40, distribution = 10000, start_frame <= 0, num_frames = 0,
    // time_to_explo(_v) = 0 — so Create2 draws exactly rand(40), rand(20000),
    // rand(20000) and Create draws nothing. `speed` is a concrete synthetic value
    // (the dossier pins speed_v/distribution, not speed) so the vel is computable.
    fn dirt_like_nobject(id: i32) -> NObjectType {
        NObjectType {
            id,
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

    // ---- Step 1: Create2 RNG order (dirt-debris constants) -------------------

    #[test]
    fn create2_dirt_draws_speed_then_two_distribution_in_order() {
        let cossin = precompute_cossin();
        let ty = dirt_like_nobject(2);
        let angle = 30;
        let color = 7; // kPix, non-zero -> Create takes the color path (no draw)
        let pos = Vec2::new(itof(50), itof(60));
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded();

        // Reference stream: the THREE dirt draws in order — rand(40) [speed_v],
        // rand(20000) [dist x], rand(20000) [dist y]. Distinct so an order swap is
        // detectable.
        let mut refr = seeded();
        let d_speed = refr.bound(40) as i32;
        let d_dx = refr.bound(20000) as i32;
        let d_dy = refr.bound(20000) as i32;
        assert_ne!(
            d_dx, d_dy,
            "seed must give distinct dist x/y draws so an order swap is detectable"
        );

        let slot = nobject_create2(
            &ty,
            angle,
            Vec2::zero(),
            pos,
            color,
            1,
            &cossin,
            &mut rand,
            &mut pool,
        );

        assert_eq!(pool.len(), 1, "exactly one nobject spawned");
        let obj = *pool.get(slot).expect("nobject spawned in slot");

        // vel = cossin[angle] * (speed - rand(40)) / 100, then += (rand-dist).
        let real_speed = 100 - d_speed;
        let mut vel = cossin[angle as usize].mul(real_speed).div(100);
        vel.x += d_dx - 10000; // Create2 sign: rand - distribution
        vel.y += d_dy - 10000;
        assert_eq!(obj.vel, vel, "vel = cossin*realspeed/100 + (rand - dist)");

        // Create2 steps pos += vel at birth (nobject.cpp:65).
        assert_eq!(
            obj.pos,
            pos.add(vel),
            "Create2 advances pos += vel once at birth"
        );

        // start_frame <= 0 & color != 0 -> cur_frame = color (no draw).
        assert_eq!(obj.cur_frame, color, "cur_frame = color (kPix) on the no-draw path");
        assert_eq!(obj.owner_idx, 1, "owner_idx carried through");
        assert_eq!(obj.ty, Some(2), "ty = nobject type id");

        // Exactly 3 draws, in this order: matching rand.last pins both count AND
        // order (Create added no 4th draw).
        assert_eq!(
            rand.last(),
            refr.last(),
            "Create2(dirt) draws exactly 3: speed_v, dist x, dist y"
        );
    }

    // ---- Step 2: Create cur_frame / time-to-explo branches -------------------

    #[test]
    fn create_start_frame_positive_draws_num_frames_plus_one() {
        // start_frame > 0 -> cur_frame = rand(num_frames+1) (one draw).
        let ty = NObjectType {
            id: 1,
            start_frame: 1,
            num_frames: 4,
            color_bullets: 77,
            time_to_explo: 5,
            time_to_explo_v: 0,
            ..Default::default()
        };
        let mut pool: Pool<NObject> = Pool::new(4);
        let mut rand = seeded();

        let mut refr = seeded();
        let d = refr.bound(5) as i32; // num_frames + 1

        let slot = nobject_create(&ty, Vec2::zero(), Vec2::zero(), 42, 1, &mut rand, &mut pool);
        let obj = *pool.get(slot).unwrap();

        assert_eq!(obj.cur_frame, d, "start_frame>0 -> cur_frame = rand(num_frames+1)");
        assert_eq!(obj.time_left, 5, "time_left = time_to_explo (no jitter)");
        assert_eq!(
            rand.last(),
            refr.last(),
            "exactly one draw (cur_frame); time_to_explo_v == 0 -> no jitter draw"
        );
    }

    #[test]
    fn create_color_nonzero_uses_color_with_no_draw() {
        // start_frame <= 0 & color != 0 -> cur_frame = color, NO draw.
        let ty = NObjectType {
            id: 1,
            start_frame: 0,
            num_frames: 4,
            color_bullets: 77,
            ..Default::default()
        };
        let mut pool: Pool<NObject> = Pool::new(4);
        let mut rand = seeded();

        let slot = nobject_create(&ty, Vec2::zero(), Vec2::zero(), 42, 1, &mut rand, &mut pool);
        assert_eq!(pool.get(slot).unwrap().cur_frame, 42, "cur_frame = color");
        assert_eq!(rand.last(), 0, "color path draws no rng");
    }

    #[test]
    fn create_color_zero_uses_color_bullets_with_no_draw() {
        // start_frame <= 0 & color == 0 -> cur_frame = color_bullets, NO draw.
        let ty = NObjectType {
            id: 1,
            start_frame: -1,
            color_bullets: 77,
            ..Default::default()
        };
        let mut pool: Pool<NObject> = Pool::new(4);
        let mut rand = seeded();

        let slot = nobject_create(&ty, Vec2::zero(), Vec2::zero(), 0, 1, &mut rand, &mut pool);
        assert_eq!(
            pool.get(slot).unwrap().cur_frame,
            77,
            "color == 0 -> cur_frame = color_bullets"
        );
        assert_eq!(rand.last(), 0, "color_bullets path draws no rng");
    }

    #[test]
    fn create_time_to_explo_v_draws_jitter() {
        // time_to_explo_v > 0 -> time_left = time_to_explo - rand(time_to_explo_v).
        // color != 0 so the cur_frame branch draws nothing: the jitter is the only
        // draw, isolating it.
        let ty = NObjectType {
            id: 1,
            start_frame: 0,
            time_to_explo: 100,
            time_to_explo_v: 30,
            ..Default::default()
        };
        let mut pool: Pool<NObject> = Pool::new(4);
        let mut rand = seeded();

        let mut refr = seeded();
        let d = refr.bound(30) as i32;

        let slot = nobject_create(&ty, Vec2::zero(), Vec2::zero(), 9, 1, &mut rand, &mut pool);
        assert_eq!(
            pool.get(slot).unwrap().time_left,
            100 - d,
            "time_left = time_to_explo - rand(time_to_explo_v)"
        );
        assert_eq!(
            rand.last(),
            refr.last(),
            "exactly one draw (the time-to-explo jitter)"
        );
    }

    // ---- Step 3: Create1 RNG order + sign contrast with Create2 --------------

    #[test]
    fn create1_draws_two_distribution_no_speed_and_uses_dist_minus_rand_sign() {
        // distribution > 0, start_frame <= 0 & color != 0, time_to_explo_v == 0:
        // Create1 draws exactly the two distribution rands and nothing else (NO
        // speed draw — the contrast with Create2), in `distribution - rand` form.
        let ty = NObjectType {
            id: 3,
            speed: 100,
            speed_v: 40, // present but NOT drawn by Create1
            distribution: 10000,
            start_frame: 0,
            time_to_explo: 0,
            time_to_explo_v: 0,
            ..Default::default()
        };
        let pos = Vec2::new(itof(10), itof(20));
        let vel_in = Vec2::new(itof(1), itof(2));
        let color = 9;
        let mut pool: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();

        let mut refr = seeded();
        let d_dx = refr.bound(20000) as i32;
        let d_dy = refr.bound(20000) as i32;
        assert_ne!(
            d_dx, 10000,
            "seed guard: rand != distribution so the sign form is discriminating"
        );

        let slot = nobject_create1(&ty, vel_in, pos, color, 1, &mut rand, &mut pool);
        let obj = *pool.get(slot).unwrap();

        // Create1 sign: vel += distribution - rand.
        let exp_x = vel_in.x + (10000 - d_dx);
        let exp_y = vel_in.y + (10000 - d_dy);
        assert_eq!(obj.vel, Vec2::new(exp_x, exp_y), "Create1 sign is `distribution - rand`");
        assert_ne!(
            obj.vel.x,
            vel_in.x + (d_dx - 10000),
            "Create1 must NOT be normalised to Create2's `rand - distribution` sign"
        );

        // Create1 does NOT step pos += vel (only Create2 does).
        assert_eq!(obj.pos, pos, "Create1 leaves pos unchanged (no birth step)");

        // Exactly 2 draws (distribution x, y) — no speed_v draw, no Create draw.
        assert_eq!(
            rand.last(),
            refr.last(),
            "Create1 draws exactly 2 (distribution x,y); no speed draw"
        );
    }

    #[test]
    fn create2_draws_speed_first_create1_does_not() {
        // Same type & seed: Create1 advances the rng by 2 (distribution only);
        // Create2 advances it by 3 (speed_v FIRST, then the two distribution
        // draws). Pins the structural contrast.
        let cossin = precompute_cossin();
        let ty = dirt_like_nobject(2);

        let mut pool1: Pool<NObject> = Pool::new(8);
        let mut r1 = seeded();
        nobject_create1(&ty, Vec2::zero(), Vec2::zero(), 9, 1, &mut r1, &mut pool1);
        let mut ref1 = seeded();
        ref1.bound(20000);
        ref1.bound(20000);
        assert_eq!(r1.last(), ref1.last(), "Create1 = 2 draws (no speed)");

        let mut pool2: Pool<NObject> = Pool::new(8);
        let mut r2 = seeded();
        nobject_create2(&ty, 30, Vec2::zero(), Vec2::zero(), 9, 1, &cossin, &mut r2, &mut pool2);
        let mut ref2 = seeded();
        ref2.bound(40);
        ref2.bound(20000);
        ref2.bound(20000);
        assert_eq!(r2.last(), ref2.last(), "Create2 = 3 draws (speed_v first)");

        assert_ne!(
            r1.last(),
            r2.last(),
            "Create2 draws one more rng (speed_v) than Create1 for the same type"
        );
    }
}
