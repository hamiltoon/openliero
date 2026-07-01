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

use assets::object::{NObjectType, SObjectType, Weapon};
use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::fixed::{ftoi, itof};
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::blit::{blit_image_on_map, draw_dirt_effect};
use crate::bobject::create_bobject;
use crate::pool::{BloodPool, Pool};
use crate::sobject::sobject_create;
use crate::state::{BObject, LevelSim, NObject, SObject, WObject, WormState};

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

    // O3 — full-pool overwrite. C++ `NObjectType::Create` allocates via
    // `NewObjectReuse` (`exactObjectList.hpp:57-67`), which at the 600-slot cap
    // returns `&arr[limit-1]`: overwrite the last slot in place (no free/swap,
    // count unchanged) rather than bailing. This is what keeps the death/damage
    // blood storms bit-exact at the pool cap instead of panicking. Below cap it
    // is identical to `spawn`, so slices 1-5c (which never reach the cap) are
    // byte-identical.
    nobjects.spawn_reuse(obj)
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

/// The verdict a single [`nobject_process`] pass returns to the driver (Task 5),
/// mirroring the `do_explode` / `worm_destroy` tail of C++ `NObject::Process`
/// (`nobject.cpp:205-233`):
///
/// * [`Keep`](NObjectOutcome::Keep) — the object lives on (no free).
/// * [`Explode`](NObjectOutcome::Explode) — `do_explode` was set (ground-explode,
///   timeout, or — when ported — worm-explode). **The explode side-effects
///   (`create_on_exp` / `dirt_effect` / splinter scatter) have ALREADY run inside
///   `nobject_process`** (unlike [`crate::weapon::WObjectOutcome::Explode`], which
///   asks the driver to call `blow_up`); the driver only performs the final
///   `Pool::free` (`nobject.cpp:230-232`, `if (used) game.nobjects.Free(this)`).
/// * [`Remove`](NObjectOutcome::Remove) — the `worm_destroy && used` path
///   (`nobject.cpp:197-199`): free **without** exploding. Never produced while the
///   worm-hit loop is deferred (`hit_damage <= 0` for the 4c types), but part of
///   the contract Task 5 consumes.
///
/// As with [`crate::weapon::wobject_process`], the verdict is split out instead of
/// freeing inside `Process` because the C++ frees `this` mid-iteration, which the
/// borrow checker forbids while the driver still holds the pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NObjectOutcome {
    Keep,
    Explode,
    Remove,
}

/// Port of `CheckForSpecWormHit` (`worm.cpp:1162-1188`) reduced to its **rand-free
/// geometry**: the worm must be `visible`, and the `±dist` box around the impact
/// point `(x, y)` must overlap the worm's 16x16 sprite box (offset `(+7, +5)` from
/// `Ftoi(w.pos)`, exactly as C++). The per-pixel
/// `materials[worm_sprite[...]].Worm()` test — which decides a *solid* hit and
/// needs the worm sprite bank + the `Worm` material flag (neither lives in the
/// sim yet) — is the part of the predicate that belongs to the DEFERRED
/// DoDamage/blood body (O10/5b), so it is approximated here by treating the whole
/// 16x16 sprite box as solid.
///
/// This reduction **over-approximates** (it can only return `true` where C++
/// returns `true`-or-`false`, never `false` where C++ returns `true` within the
/// box), which is safe for 5a: every worm is far out of range, so both the full
/// C++ test and this reduction return `false`, the loop draws nothing, and the
/// deferred body guard is never reached. The `Rect::Intersect` is the same
/// `max(x1)/max(y1)/min(x2)/min(y2)` clamp [`crate::sobject`] uses; a non-empty
/// intersection is the hit.
fn check_for_spec_worm_hit(worm: &WormState, x: i32, y: i32, dist: i32) -> bool {
    // :1165-1167 invisible worms are never hit.
    if !worm.visible {
        return false;
    }
    // :1171-1172 deltas relative to the worm sprite's top-left (offset +7,+5).
    let delta_x = x - ftoi(worm.pos.x) + 7;
    let delta_y = y - ftoi(worm.pos.y) + 5;
    // :1174-1176 Rect(delta-dist, delta-dist, delta+dist+1, delta+dist+1)
    // intersected with the 16x16 sprite rect; a non-empty result is a hit.
    let x1 = (delta_x - dist).max(0);
    let y1 = (delta_y - dist).max(0);
    let x2 = (delta_x + dist + 1).min(16);
    let y2 = (delta_y + dist + 1).min(16);
    x1 < x2 && y1 < y2
}

/// Port of `NObject::Process` (`nobject.cpp:68-234`) — advance one nobject by one
/// tick.
///
/// The 4c dirt-debris path (`particle__disappearing`: `bounce=0`,
/// `blood_trail=false`, `num_frames=0`, `hit_damage=0`, `time_to_explo=0`,
/// `create_on_exp=-1`, `dirt_effect=-1`, `splinter_amount=0`) draws **zero rand**:
/// it integrates `pos += vel`, clamps at the level edges, and on a ground hit
/// zeroes `vel` and (because `expl_ground=true`) returns
/// [`Explode`](NObjectOutcome::Explode) — and every explode arm is skipped, so it
/// just frees. The whole function is ported; branches that need state this slice
/// does not own are **guarded** so a config that would take an un-ported path
/// trips loudly in debug builds:
///
/// * **bounce** (`:81-93`) — `if ty.bounce > 0`, the natural C++ guard; fully
///   ported (reflects `vel`, no rand). Skipped for the dirt particle (`bounce=0`).
/// * **blood_trail** (`:95-97`) — LIVE (T3). Gate `blood_trail && blood_trail_delay
///   > 0 && cycles % blood_trail_delay == 0` spawns a `BObject` via
///   [`create_bobject`] (one `rand(NumBloodColours)` draw) at the nobject's current
///   `pos` with `vel / 4`. `cycles` is the pre-`++cycles` snapshot (the same value
///   the `cycles & 7` animation gate reads), so now that `cycles` advances (T0) the
///   trail fires at the faithful 1/10 cadence — no warm-up storm.
/// * **boundary clamp** (`:100-113`) — fully ported. **Clamps to `Itof(width)` /
///   `Itof(height)`, NOT `width-1`** — the load-bearing difference from
///   [`crate::weapon::wobject_process`], which clamps to `width-1`.
/// * **ground vs air** (`:115-141`) — fully ported via [`LevelSim::inside`] /
///   [`LevelSim::dirt_rock`] (NOT `checked_mat_background`), exactly as
///   `wobject_process`. The `BlitImageOnMap`-on-ground arm (`:119-128`, gated
///   `start_frame > 0 && draw_on_map`) and the `leave_obj` sobject trail
///   (`:133-138`) are `debug_assert!`ed off (need sprite-blit / sobject Create).
/// * **animation** (`:143-158`) — `if ty.num_frames > 0` natural guard; fully
///   ported (no rand). Inert for the dirt particle (`num_frames=0`).
/// * **timeout** (`:160-164`) — `if ty.time_to_explo > 0` natural guard; fully
///   ported (no rand). Inert for the dirt particle (`time_to_explo=0`).
/// * **worm-hit** (`:166-203`) — the per-worm loop SKELETON + the rand-free
///   in-range test ([`check_for_spec_worm_hit`]) are ported; the vel-kick /
///   `DoDamage` / hit-sound `rand(3)` / `rand(128)` blood fan / worm_explode /
///   worm_destroy BODY stays DEFERRED (O10/5b) behind a guard INSIDE the in-range
///   branch (per-worm, mirroring `sobject.rs`). The loop draws NOTHING on a no-hit,
///   so a `hit_damage > 0` type with no worm in range is a no-op (no panic) —
///   exactly what the 5a splinters need every flight tick.
/// * **explode arms** (`:205-233`, `if do_explode`) — `create_on_exp` fully ported
///   via [`sobject_create`] (BEFORE `dirt_effect`, the C++ order; this is the
///   splinter's secondary `small_explosion`); `dirt_effect` fully ported via
///   [`draw_dirt_effect`] (`CorrectShadow` omitted, O4); splinter scatter fully
///   ported via [`nobject_create2`] (`rand(128)` + `rand(2)` per splinter, then
///   Create2's draws). All three are skipped for the dirt particle.
///
/// `inew` is computed once before the clamp and reused by the ground test (the
/// clamp mutates `pos`, `inew` stays frozen) — the same ordering quirk as
/// `wobject_process`. `used` (the C++ live-flag) is always true for an object the
/// driver is processing, so `if (used) Free` becomes an unconditional free on the
/// [`Explode`](NObjectOutcome::Explode) / [`Remove`](NObjectOutcome::Remove) verdict.
#[allow(clippy::too_many_arguments)]
pub fn nobject_process(
    obj: &mut NObject,
    ty: &NObjectType,
    nobject_types: &[NObjectType],
    sobject_types: &[SObjectType],
    level: &mut LevelSim,
    cossin: &[Vec2; 128],
    large_sprites: &SpriteSet,
    small_sprites: &SpriteSet,
    textures: &[Texture],
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    weapons: &[Weapon],
    nobjects: &mut Pool<NObject>,
    sobjects: &mut Pool<SObject>,
    bobjects: &mut BloodPool<BObject>,
    cycles: i32,
    blood: i32,
    num_blood_colours: i32,
    first_blood_colour: i32,
    rand: &mut Rand,
) -> NObjectOutcome {
    let mut bounced = false;
    let mut do_explode = false;

    // :74 pos += vel.
    obj.pos = obj.pos.add(obj.vel);

    // :76-77 inew = Ftoi(pos + vel), ipos = Ftoi(pos). Arithmetic-shift floor.
    let mut inew_x = ftoi(obj.pos.x.wrapping_add(obj.vel.x));
    let mut inew_y = ftoi(obj.pos.y.wrapping_add(obj.vel.y));
    let ipos_x = ftoi(obj.pos.x);
    let ipos_y = ftoi(obj.pos.y);

    // :81-93 bounce. Natural C++ guard `ty.bounce > 0` (skipped for the dirt
    // particle, bounce=0). The two probes mutate `vel` sequentially: the second
    // reads the value the first wrote (corner case), so mirror in-place.
    if ty.bounce > 0 {
        // :82 x probe: (inew.x, ipos.y).
        if !level.inside(inew_x, ipos_y) || level.dirt_rock(inew_x, ipos_y) {
            // :83 vel.x = -vel.x * bounce / 100; :84 vel.y = (vel.y * 4) / 5.
            obj.vel.x = obj
                .vel
                .x
                .wrapping_neg()
                .wrapping_mul(ty.bounce)
                .wrapping_div(100);
            obj.vel.y = obj.vel.y.wrapping_mul(4).wrapping_div(5);
            bounced = true;
        }
        // :88 y probe: (ipos.x, inew.y).
        if !level.inside(ipos_x, inew_y) || level.dirt_rock(ipos_x, inew_y) {
            // :89 vel.y = -vel.y * bounce / 100; :90 vel.x = (vel.x * 4) / 5.
            obj.vel.y = obj
                .vel
                .y
                .wrapping_neg()
                .wrapping_mul(ty.bounce)
                .wrapping_div(100);
            obj.vel.x = obj.vel.x.wrapping_mul(4).wrapping_div(5);
            bounced = true;
        }
    }

    // :95-97 blood_trail BObject spawn (LIVE). Gate: blood_trail set, delay > 0, and
    // `cycles % delay == 0` — `cycles` is the SAME pre-`++cycles` snapshot the
    // animation `cycles & 7` gate reads (game.cpp object loops precede :357), so the
    // faithful 1/10 cadence holds now that cycles advances. Spawns at the nobject's
    // CURRENT pos with `vel / 4` (truncating fixed-vector divide), AFTER the bounce
    // block mutated `vel` — exactly the C++ position. `create_bobject` draws one
    // `rand(NumBloodColours)`.
    if ty.blood_trail && ty.blood_trail_delay > 0 && cycles.wrapping_rem(ty.blood_trail_delay) == 0
    {
        create_bobject(
            bobjects,
            obj.pos,
            obj.vel.div(4),
            num_blood_colours,
            first_blood_colour,
            rand,
        );
    }

    // :100 recompute inew = Ftoi(pos + vel) (uses the post-bounce vel).
    inew_x = ftoi(obj.pos.x.wrapping_add(obj.vel.x));
    inew_y = ftoi(obj.pos.y.wrapping_add(obj.vel.y));

    // :102-113 boundary clamp. NOTE: clamps to Itof(width)/Itof(height), NOT
    // width-1 (the difference from wobject_process). `inew` is NOT recomputed
    // after the clamp — it stays frozen for the ground test below.
    if inew_x < 0 {
        obj.pos.x = 0;
    }
    if inew_y < 0 {
        obj.pos.y = 0;
    }
    if inew_x >= level.width {
        obj.pos.x = itof(level.width);
    }
    if inew_y >= level.height {
        obj.pos.y = itof(level.height);
    }

    // :115-141 ground collision vs free air (frozen `inew`).
    if !level.inside(inew_x, inew_y) || level.dirt_rock(inew_x, inew_y) {
        // :116 vel.Zero().
        obj.vel = Vec2::zero();

        if ty.expl_ground {
            // :119-128 BlitImageOnMap-on-ground arm (Slice-4d): a `draw_on_map`
            // object with `start_frame > 0` (the spent SHELL) paints its 7x7 image
            // into `material_id` at `(ipos - 3)` before exploding. `CorrectShadow`
            // (:123-127, behind settings->shadow) is OMITTED (shadow off, render-
            // only). Inert for the dirt particle (draw_on_map=false).
            if ty.start_frame > 0 && ty.draw_on_map {
                blit_image_on_map(
                    level,
                    small_sprites,
                    (ty.start_frame + obj.cur_frame) as usize,
                    ipos_x - 3,
                    ipos_y - 3,
                );
            }
            // :130
            do_explode = true;
        }
    } else {
        // :133-138 leave_obj sobject trail — deferred (needs SObject Create).
        // C++ gate is `!bounced && leave_obj_delay != 0 && leave_obj >= 0 && ...`;
        // the assert reproduces that gate so `bounced` is load-bearing (the trail
        // is suppressed right after a bounce). Inert for the dirt particle
        // (leave_obj=-1).
        debug_assert!(
            bounced || ty.leave_obj < 0 || ty.leave_obj_delay == 0,
            "leave_obj sobject trail deferred (needs SObject Create)"
        );
        // :140 vel.y += gravity.
        obj.vel.y = obj.vel.y.wrapping_add(ty.gravity);
    }

    // :143-158 animation. Natural C++ guard `num_frames > 0`; no rand. Inert for
    // the dirt particle (num_frames=0).
    if ty.num_frames > 0 && (cycles & 7) == 0 {
        if obj.vel.x > 0 {
            obj.cur_frame += 1;
            if obj.cur_frame > ty.num_frames {
                obj.cur_frame = 0;
            }
        } else if obj.vel.x < 0 {
            obj.cur_frame -= 1;
            if obj.cur_frame < 0 {
                obj.cur_frame = ty.num_frames;
            }
        }
    }

    // :160-164 timeout. Natural C++ guard `time_to_explo > 0`; no rand. `--time_left
    // <= 0` decrements first. Inert for the dirt particle (time_to_explo=0).
    if ty.time_to_explo > 0 {
        obj.time_left -= 1;
        if obj.time_left <= 0 {
            do_explode = true;
        }
    }

    // :166-203 worm-hit loop SKELETON. The C++ `if (t.hit_damage > 0)` per-worm
    // loop runs the rand-free in-range test [`check_for_spec_worm_hit`]; the
    // vel-kick / DoDamage / hit-sound `rand(3)` / `rand(128)` blood fan /
    // worm_explode / worm_destroy BODY (`:172-199`) stays DEFERRED (O10/5b) behind
    // a guard INSIDE the in-range branch — mirroring how `sobject.rs` defers its
    // in-box DoDamage per-worm rather than type-level. For 5a every worm is out of
    // range, so the loop iterates, finds NO hit, draws NOTHING, and never trips the
    // guard — bit-exact with C++ (which also draws nothing on a no-hit). The
    // `particle__small_damage` splinter has `hit_damage = 2`, so the OLD type-level
    // `debug_assert!(hit_damage <= 0)` panicked every flight tick; the per-worm
    // structure is the fix.
    if !do_explode && ty.hit_damage > 0 {
        for w in worms.iter() {
            if check_for_spec_worm_hit(w, ftoi(obj.pos.x), ftoi(obj.pos.y), ty.detect_distance) {
                // :172-199 DEFERRED body (O10/5b): w.vel += vel*blow_away/100,
                // DoDamage, the hit-sound rand(3), the rand(128) blood fan, and
                // worm_explode/worm_destroy. They DRAW RAND and need DoDamage / the
                // blood nobject, so a worm actually in range trips loudly here.
                debug_assert!(
                    false,
                    "nobject worm-hit DoDamage/blood/vel-kick body deferred (O10/5b)"
                );
            }
        }
    }

    // :205-233 explode arms.
    if do_explode {
        // :206-209 create_on_exp sobject — BEFORE dirt_effect and the splinter arm
        // (C++ order: create_on_exp -> dirt_effect -> splinter; load-bearing because
        // the sobject's own sound/dirt-throw/crater draw RNG between this spawn and
        // the dart crater). Pass `Ftoi(pos.x), Ftoi(pos.y)`; the `-8` centre→top-left
        // offset is applied inside sobject_create. owner_idx == cause_idx == fired_by
        // (the exploding nobject's owner). This is the splinter's secondary explosion
        // (`particle__small_damage` -> small_explosion). Inert for the dirt particle
        // (create_on_exp=-1).
        if ty.create_on_exp >= 0 {
            sobject_create(
                &sobject_types[ty.create_on_exp as usize],
                ftoi(obj.pos.x),
                ftoi(obj.pos.y),
                obj.owner_idx,
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
                blood,
                rand,
            );
        }

        // :211-219 dirt_effect crater. Fully ported; CorrectShadow omitted (O4).
        // Inert for the dirt particle (dirt_effect=-1).
        if ty.dirt_effect >= 0 {
            draw_dirt_effect(
                level,
                large_sprites,
                textures,
                ty.dirt_effect,
                ftoi(obj.pos.x) - 7,
                ftoi(obj.pos.y) - 7,
                rand,
            );
        }

        // :221-228 splinter scatter. Per splinter: rand(128) [kAngle] + rand(2)
        // [kColorSub], then nobject_types[splinter_type].Create2 (its own draws).
        // Inert for the dirt particle (splinter_amount=0).
        if ty.splinter_amount > 0 {
            for _ in 0..ty.splinter_amount {
                let angle = rand.bound(128) as i32;
                let color_sub = rand.bound(2) as i32;
                nobject_create2(
                    &nobject_types[ty.splinter_type as usize],
                    angle,
                    Vec2::zero(),
                    obj.pos,
                    ty.splinter_colour - color_sub,
                    obj.owner_idx,
                    cossin,
                    rand,
                    nobjects,
                );
            }
        }

        // :230-232 if (used) Free(this) — `used` always true for a processed
        // object; the driver performs the free on the Explode verdict.
        return NObjectOutcome::Explode;
    }

    NObjectOutcome::Keep
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

    // ====================== nobject_process (Task 2) ==========================

    use crate::state::{LevelSim, MAT_BACKGROUND, MAT_ROCK};

    // An all-background level (every pixel material 0, which carries the
    // Background flag and is NOT DirtRock) — open air everywhere in [0,w)x[0,h).
    fn bg_level(width: i32, height: i32) -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[0] = MAT_BACKGROUND;
        material_flags[1] = MAT_ROCK;
        LevelSim {
            width,
            height,
            material_id: vec![0u8; (width * height) as usize],
            material_flags,
        }
    }

    // Background level with a solid rock floor at rows y >= floor_y.
    fn level_with_floor(width: i32, height: i32, floor_y: i32) -> LevelSim {
        let mut level = bg_level(width, height);
        for y in floor_y..height {
            for x in 0..width {
                level.material_id[(y * width + x) as usize] = 1; // rock
            }
        }
        level
    }

    // Background level with a solid rock wall at cols x >= wall_x.
    fn level_with_wall(width: i32, height: i32, wall_x: i32) -> LevelSim {
        let mut level = bg_level(width, height);
        for y in 0..height {
            for x in wall_x..width {
                level.material_id[(y * width + x) as usize] = 1; // rock
            }
        }
        level
    }

    // The 4c dirt-debris type `particle__disappearing`: expl_ground=true, all the
    // other Process branches inert. Draws ZERO rand in Process.
    fn particle_disappearing() -> NObjectType {
        NObjectType {
            id: 4,
            expl_ground: true,
            draw_on_map: false,
            start_frame: 0,
            bounce: 0,
            blood_trail: false,
            num_frames: 0,
            hit_damage: 0,
            time_to_explo: 0,
            create_on_exp: -1,
            dirt_effect: -1,
            splinter_amount: 0,
            leave_obj: -1,
            gravity: 700,
            ..Default::default()
        }
    }

    // Empty sprite bank + texture table for paths that never call draw_dirt_effect
    // (dirt_effect < 0). The signature still needs them.
    fn no_sprites() -> SpriteSet {
        SpriteSet {
            width: 16,
            height: 16,
            count: 0,
            data: Vec::new(),
        }
    }

    // Run nobject_process with throwaway sprite/texture args (dirt_effect<0 cases)
    // and empty worm list / cross pools / weapon + sobject tables (the new explode
    // / worm-hit arms are inert for these callers: create_on_exp<0, no worms).
    #[allow(clippy::too_many_arguments)]
    fn run_process(
        obj: &mut NObject,
        ty: &NObjectType,
        nobject_types: &[NObjectType],
        level: &mut LevelSim,
        cossin: &[Vec2; 128],
        nobjects: &mut Pool<NObject>,
        cycles: i32,
        rand: &mut Rand,
    ) -> NObjectOutcome {
        let sprites = no_sprites();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut sobjects: Pool<SObject> = Pool::new(1);
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        nobject_process(
            obj,
            ty,
            nobject_types,
            &[],
            level,
            cossin,
            &sprites,
            &sprites,
            &[],
            &mut worms,
            &mut wobjects,
            &[],
            nobjects,
            &mut sobjects,
            &mut bobjects,
            cycles,
            100,
            0,
            0,
            rand,
        )
    }

    // ---- Step 1: move + gravity + boundary clamp -----------------------------

    #[test]
    fn process_moves_then_applies_gravity_in_open_air() {
        let cossin = precompute_cossin();
        // Air-only type: gravity 700, expl_ground false so an in-air step never
        // explodes.
        let ty = NObjectType {
            id: 5,
            gravity: 700,
            expl_ground: false,
            leave_obj: -1,
            ..Default::default()
        };
        let mut level = bg_level(100, 100);
        let mut pool: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();

        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(50)),
            vel: Vec2::new(itof(1), itof(2)),
            ty: Some(5),
            ..Default::default()
        };

        let out = run_process(
            &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 0, &mut rand,
        );

        assert_eq!(out, NObjectOutcome::Keep, "open-air, no explode");
        // pos += vel (no clamp; well inside).
        assert_eq!(obj.pos, Vec2::new(itof(51), itof(52)), "pos += vel");
        // Air branch: vel.y += gravity; vel.x unchanged.
        assert_eq!(
            obj.vel,
            Vec2::new(itof(1), itof(2) + 700),
            "vel.y += gravity (air branch)"
        );
        assert_eq!(rand.last(), 0, "Process drew no rand in open air");
        assert_eq!(pool.len(), 0, "no spawns");
    }

    #[test]
    fn process_clamps_pos_past_each_edge_to_itof_dim() {
        let cossin = precompute_cossin();
        // gravity 0 so vel changes don't distract; expl_ground false.
        let ty = NObjectType {
            id: 5,
            gravity: 0,
            expl_ground: false,
            leave_obj: -1,
            ..Default::default()
        };
        let (w, h) = (100, 100);

        // (start_pos, vel, expected clamped component, axis, edge name).
        let cases = [
            (Vec2::new(itof(1), itof(50)), Vec2::new(itof(-5), 0), 0, 'x', "left"),
            (
                Vec2::new(itof(98), itof(50)),
                Vec2::new(itof(5), 0),
                itof(w),
                'x',
                "right -> Itof(width), NOT width-1",
            ),
            (Vec2::new(itof(50), itof(1)), Vec2::new(0, itof(-5)), 0, 'y', "top"),
            (
                Vec2::new(itof(50), itof(98)),
                Vec2::new(0, itof(5)),
                itof(h),
                'y',
                "bottom -> Itof(height), NOT height-1",
            ),
        ];

        for (pos, vel, expected, axis, name) in cases {
            let mut level = bg_level(w, h);
            let mut pool: Pool<NObject> = Pool::new(4);
            let mut rand = seeded();
            let mut obj = NObject {
                pos,
                vel,
                ty: Some(5),
                ..Default::default()
            };
            run_process(
                &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 0, &mut rand,
            );
            let got = if axis == 'x' { obj.pos.x } else { obj.pos.y };
            assert_eq!(got, expected, "clamp at {name} edge");
        }
    }

    // ---- Step 2: ground explode (expl_ground), no rand, just-free ------------

    #[test]
    fn process_ground_hit_zeroes_vel_and_explodes_without_rand() {
        let cossin = precompute_cossin();
        let ty = particle_disappearing();
        let mut level = level_with_floor(100, 100, 60);
        let before_level = level.material_id.clone();
        let mut pool: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        // Pre-advance the rng so "unchanged" is a real assertion, not == 0.
        rand.bound(99);
        let rng_before = rand.last();

        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(59)),
            vel: Vec2::new(0, itof(2)),
            ty: Some(4),
            ..Default::default()
        };

        let out = run_process(
            &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 0, &mut rand,
        );

        assert_eq!(out, NObjectOutcome::Explode, "expl_ground on a floor -> Explode");
        assert_eq!(obj.vel, Vec2::zero(), "vel.Zero() on ground contact");
        assert_eq!(
            rand.last(),
            rng_before,
            "dirt-debris Process draws NO rand (all explode arms inert)"
        );
        assert_eq!(pool.len(), 0, "create_on_exp/splinter all -1/0: no spawns");
        assert_eq!(
            level.material_id, before_level,
            "draw_on_map=false & dirt_effect=-1: level untouched"
        );
    }

    // ---- Step 3: bounce (guarded behind bounce>0), x and y reflect -----------

    #[test]
    fn process_bounce_reflects_x_and_y_against_walls() {
        let cossin = precompute_cossin();
        let ty = NObjectType {
            id: 6,
            bounce: 50,
            gravity: 0,
            expl_ground: false,
            leave_obj: -1,
            ..Default::default()
        };

        // --- x reflect only: vertical wall at x>=60; ipos.x<60 so y-probe skips.
        {
            let mut level = level_with_wall(100, 100, 60);
            let mut pool: Pool<NObject> = Pool::new(4);
            let mut rand = seeded();
            let mut obj = NObject {
                pos: Vec2::new(itof(57), itof(50)),
                vel: Vec2::new(itof(2), itof(3)),
                ty: Some(6),
                ..Default::default()
            };
            let out = run_process(
                &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 0, &mut rand,
            );
            assert_eq!(out, NObjectOutcome::Keep);
            // vel.x = -itof(2)*50/100 = -itof(1); vel.y = itof(3)*4/5 (trunc).
            assert_eq!(obj.vel.x, -itof(1), "x reflect: -vel.x*bounce/100");
            assert_eq!(obj.vel.y, itof(3) * 4 / 5, "y damped by 4/5 on x-bounce");
            assert_ne!(obj.vel.x, itof(2), "vel.x actually reflected");
            assert_eq!(rand.last(), 0, "bounce draws no rand");
        }

        // --- y reflect only: horizontal floor at y>=60; ipos.y<60 so x-probe skips.
        {
            let mut level = level_with_floor(100, 100, 60);
            let mut pool: Pool<NObject> = Pool::new(4);
            let mut rand = seeded();
            let mut obj = NObject {
                pos: Vec2::new(itof(50), itof(57)),
                vel: Vec2::new(itof(3), itof(2)),
                ty: Some(6),
                ..Default::default()
            };
            let out = run_process(
                &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 0, &mut rand,
            );
            assert_eq!(out, NObjectOutcome::Keep);
            // vel.y = -itof(2)*50/100 = -itof(1); vel.x = itof(3)*4/5 (trunc).
            assert_eq!(obj.vel.y, -itof(1), "y reflect: -vel.y*bounce/100");
            assert_eq!(obj.vel.x, itof(3) * 4 / 5, "x damped by 4/5 on y-bounce");
            assert_ne!(obj.vel.y, itof(2), "vel.y actually reflected");
        }
    }

    // ---- Step 4: inert guarded branches draw no rand -------------------------

    #[test]
    fn process_dirt_debris_in_air_leaves_rng_untouched() {
        let cossin = precompute_cossin();
        let ty = particle_disappearing();
        let mut level = bg_level(100, 100);
        let mut pool: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        // Advance the engine a few times so rng_before is a non-trivial value.
        rand.bound(7);
        rand.bound(123);
        rand.bound(9999);
        let rng_before = rand.last();

        let mut obj = NObject {
            pos: Vec2::new(itof(40), itof(40)),
            vel: Vec2::new(itof(1), itof(1)),
            ty: Some(4),
            time_left: 0,
            ..Default::default()
        };

        let out = run_process(
            &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 5, &mut rand,
        );

        assert_eq!(out, NObjectOutcome::Keep, "open air, no explode");
        assert_eq!(
            rand.last(),
            rng_before,
            "blood_trail/anim/timeout/worm-hit all inert: rand.last unchanged"
        );
        assert_eq!(pool.len(), 0, "no spawns");
        // time_to_explo=0 so the timeout decrement never runs.
        assert_eq!(obj.time_left, 0, "time_left untouched (no timeout)");
    }

    // ---- Step 5: explode side-effects guarded (splinter arm covered) ---------

    #[test]
    fn process_explode_runs_splinter_arm_with_exact_rng_order() {
        let cossin = precompute_cossin();
        // Splinter type shaped like the dirt particle (speed_v=40, dist=10000) so
        // its Create2 draws rand(40), rand(20000), rand(20000) and Create draws
        // nothing. Index 0 in the nobject_types table.
        let splinter_ty = dirt_like_nobject(0);
        let nobject_types = vec![splinter_ty];

        // Exploding type: expl_ground, splinter_amount=2 into type 0, no
        // create_on_exp / dirt_effect (isolate the splinter arm).
        let ty = NObjectType {
            id: 7,
            expl_ground: true,
            draw_on_map: false,
            start_frame: 0,
            create_on_exp: -1,
            dirt_effect: -1,
            splinter_amount: 2,
            splinter_type: 0,
            splinter_colour: 80,
            leave_obj: -1,
            gravity: 0,
            ..Default::default()
        };

        let mut level = level_with_floor(100, 100, 60);
        let mut pool: Pool<NObject> = Pool::new(64);
        let mut rand = seeded();

        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(59)),
            vel: Vec2::new(0, itof(2)),
            ty: Some(7),
            owner_idx: 3,
            ..Default::default()
        };

        let out = run_process_with(
            &mut obj,
            &ty,
            &nobject_types,
            &mut level,
            &cossin,
            &mut pool,
            0,
            &mut rand,
        );

        // Reference rng stream: per splinter rand(128) + rand(2), then Create2's
        // rand(40), rand(20000), rand(20000) — for 2 splinters, in order.
        let mut refr = seeded();
        let mut first_color_sub = 0;
        for i in 0..2 {
            let _angle = refr.bound(128);
            let color_sub = refr.bound(2);
            if i == 0 {
                first_color_sub = color_sub as i32;
            }
            refr.bound(40); // Create2 speed_v
            refr.bound(20000); // dist x
            refr.bound(20000); // dist y
        }

        assert_eq!(out, NObjectOutcome::Explode, "ground explode -> Explode");
        assert_eq!(pool.len(), 2, "splinter_amount=2 spawned two nobjects");
        assert_eq!(
            rand.last(),
            refr.last(),
            "exact rng order: 2x [rand(128), rand(2), rand(40), rand(20000), rand(20000)]"
        );
        // First splinter's cur_frame = splinter_colour - kColorSub (color path,
        // start_frame<=0 & color!=0 -> no draw inside Create).
        let first = *pool.get(0).expect("first splinter spawned");
        assert_eq!(
            first.cur_frame,
            80 - first_color_sub,
            "splinter cur_frame = splinter_colour - rand(2)"
        );
        assert_eq!(first.owner_idx, 3, "splinter inherits owner_idx");
        assert_eq!(first.ty, Some(0), "splinter is nobject_types[splinter_type]");
    }

    #[test]
    fn dirt_debris_explode_hits_no_side_effect_arm() {
        // The companion to Step 5: the real dirt particle explodes on the floor
        // but every side-effect arm (create_on_exp/dirt_effect/splinter) is
        // skipped, so nothing spawns and no rand is drawn.
        let cossin = precompute_cossin();
        let ty = particle_disappearing();
        let mut level = level_with_floor(100, 100, 60);
        let mut pool: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        rand.bound(55);
        let rng_before = rand.last();

        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(59)),
            vel: Vec2::new(0, itof(2)),
            ty: Some(4),
            ..Default::default()
        };

        let out = run_process(
            &mut obj, &ty, &[], &mut level, &cossin, &mut pool, 0, &mut rand,
        );

        assert_eq!(out, NObjectOutcome::Explode);
        assert_eq!(pool.len(), 0, "dirt-debris explode spawns nothing");
        assert_eq!(rand.last(), rng_before, "dirt-debris explode draws no rand");
    }

    // run_process variant that takes a real nobject_types table (splinter arm).
    // Empty worms / cross pools / weapon + sobject tables (create_on_exp<0, no
    // worms in these callers).
    #[allow(clippy::too_many_arguments)]
    fn run_process_with(
        obj: &mut NObject,
        ty: &NObjectType,
        nobject_types: &[NObjectType],
        level: &mut LevelSim,
        cossin: &[Vec2; 128],
        nobjects: &mut Pool<NObject>,
        cycles: i32,
        rand: &mut Rand,
    ) -> NObjectOutcome {
        let sprites = no_sprites();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut sobjects: Pool<SObject> = Pool::new(1);
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        nobject_process(
            obj,
            ty,
            nobject_types,
            &[],
            level,
            cossin,
            &sprites,
            &sprites,
            &[],
            &mut worms,
            &mut wobjects,
            &[],
            nobjects,
            &mut sobjects,
            &mut bobjects,
            cycles,
            100,
            0,
            0,
            rand,
        )
    }

    // ---- Step 6: the new explode/worm-hit arms (T2a) -------------------------

    use crate::state::{WeaponInit, WormInit};

    // A worm at a pixel position, visible or not, for the worm-hit in-range test.
    fn worm_at(px: i32, py: i32, visible: bool) -> WormState {
        let mut w = WormState::from_init(&WormInit {
            index: 0,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit::default(); crate::state::NUM_WEAPONS],
            start_pos: Vec2::new(itof(px), itof(py)),
            visible,
        });
        w.vel = Vec2::zero();
        w
    }

    // A synthetic exploding type whose create_on_exp spawns sobject_types[idx] on a
    // ground explode. No dirt_effect / splinters, so the ONLY explode side effect is
    // the create_on_exp sobject (isolates the new arm).
    fn create_on_exp_nobject(create_on_exp: i32) -> NObjectType {
        NObjectType {
            id: 8,
            expl_ground: true,
            draw_on_map: false,
            start_frame: 0,
            bounce: 0,
            blood_trail: false,
            num_frames: 0,
            hit_damage: 0,
            time_to_explo: 0,
            create_on_exp,
            dirt_effect: -1,
            splinter_amount: 0,
            leave_obj: -1,
            gravity: 0,
            ..Default::default()
        }
    }

    // An sobject type with NO side effects that draw rand: start_sound < 0 (no sound
    // rand), damage 0 (no worm/dirt-throw block), dirt_effect < 0 (no carve). So
    // sobject_create spawns exactly one sobject and draws ZERO rand — making the
    // spawn itself the assertion, not an rng count.
    fn inert_sobject(id: i32) -> SObjectType {
        SObjectType {
            id,
            start_sound: -1,
            num_sounds: 0,
            anim_delay: 3,
            start_frame: 0,
            num_frames: 4,
            detect_range: 0,
            damage: 0,
            blow_away: 0,
            dirt_effect: -1,
            ..Default::default()
        }
    }

    #[test]
    fn explode_create_on_exp_spawns_one_sobject() {
        // A create_on_exp >= 0 type that ground-explodes must spawn the secondary
        // sobject via sobject_create — the splinter's small_explosion path.
        let cossin = precompute_cossin();
        let ty = create_on_exp_nobject(1); // -> sobject_types[1]
        let sobject_types = vec![inert_sobject(0), inert_sobject(1)];
        let mut level = level_with_floor(100, 100, 60);
        let sprites = no_sprites();
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut sobjects: Pool<SObject> = Pool::new(8);
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut worms: Vec<WormState> = Vec::new();
        let mut rand = seeded();
        // Pre-advance so "no rand drawn" is a real assertion, not == 0.
        rand.bound(77);
        let rng_before = rand.last();

        // Zero vel so pos is unchanged by the `pos += vel` step and the impact
        // coordinates are exactly (50, 60): inew = Ftoi(pos+vel) = (50,60) is the
        // rock floor (rows >= 60), so it ground-explodes this tick.
        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(60)),
            vel: Vec2::zero(),
            ty: Some(8),
            owner_idx: 2,
            ..Default::default()
        };

        let out = nobject_process(
            &mut obj,
            &ty,
            &[],
            &sobject_types,
            &mut level,
            &cossin,
            &sprites,
            &sprites,
            &[],
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut bobjects,
            0,
            100,
            0,
            0,
            &mut rand,
        );

        assert_eq!(out, NObjectOutcome::Explode, "ground explode -> Explode");
        assert_eq!(sobjects.len(), 1, "create_on_exp spawned exactly one sobject");
        let s = *sobjects.get(0).expect("sobject in slot 0");
        assert_eq!(s.id, 1, "spawned sobject is sobject_types[create_on_exp]");
        // x/y = Ftoi(pos) - 8 (the centre->top-left offset inside sobject_create).
        assert_eq!(s.x, 50 - 8, "sobject x = Ftoi(pos.x) - 8");
        assert_eq!(s.y, 60 - 8, "sobject y = Ftoi(pos.y) - 8");
        assert_eq!(
            rand.last(),
            rng_before,
            "inert sobject draws no rand (start_sound<0, damage 0, dirt_effect<0)"
        );
        assert_eq!(nobjects.len(), 0, "no splinters: create_on_exp only");
    }

    #[test]
    fn worm_hit_loop_no_worm_in_range_draws_nothing_and_no_panic() {
        // A hit_damage > 0 type that does NOT explode (in free air) runs the worm-hit
        // loop. With the only worm far out of detect_distance range, the loop finds no
        // hit: it draws NOTHING and never trips the deferred-body guard (no panic in a
        // debug build). This is the exact 5a splinter-in-flight case the old
        // type-level assert wrongly panicked on.
        let cossin = precompute_cossin();
        let ty = NObjectType {
            id: 9,
            hit_damage: 2, // > 0 -> the worm-hit loop runs
            detect_distance: 2,
            expl_ground: false,
            gravity: 0,
            time_to_explo: 0,
            create_on_exp: -1,
            dirt_effect: -1,
            splinter_amount: 0,
            leave_obj: -1,
            ..Default::default()
        };
        let mut level = bg_level(200, 200);
        let sprites = no_sprites();
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut sobjects: Pool<SObject> = Pool::new(1);
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        let mut wobjects: Pool<WObject> = Pool::new(1);
        // A VISIBLE worm 100px away (well outside detect_distance=2) + an invisible
        // worm right on top (invisible -> never a hit).
        let mut worms = vec![worm_at(150, 50, true), worm_at(50, 50, false)];
        let mut rand = seeded();
        rand.bound(33);
        let rng_before = rand.last();

        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(50)),
            vel: Vec2::new(itof(1), 0),
            ty: Some(9),
            owner_idx: 0,
            ..Default::default()
        };

        let out = nobject_process(
            &mut obj,
            &ty,
            &[],
            &[],
            &mut level,
            &cossin,
            &sprites,
            &sprites,
            &[],
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut bobjects,
            0,
            100,
            0,
            0,
            &mut rand,
        );

        assert_eq!(out, NObjectOutcome::Keep, "free air, no explode");
        assert_eq!(
            rand.last(),
            rng_before,
            "no worm in range -> worm-hit loop draws nothing"
        );
        assert_eq!(sobjects.len(), 0, "no explode -> no create_on_exp");
        assert_eq!(nobjects.len(), 0, "no spawns");
    }

    #[test]
    fn worm_hit_in_range_check_is_visibility_and_box_gated() {
        // Directly pin check_for_spec_worm_hit's reduced geometry: invisible -> false
        // regardless of distance; visible + far -> false; visible + on-point -> true.
        let invisible = worm_at(50, 50, false);
        assert!(
            !check_for_spec_worm_hit(&invisible, 50, 50, 4),
            "invisible worm is never hit even on-point"
        );
        let visible_far = worm_at(50, 50, true);
        assert!(
            !check_for_spec_worm_hit(&visible_far, 200, 200, 2),
            "visible worm far outside the box is not hit"
        );
        assert!(
            check_for_spec_worm_hit(&visible_far, 57, 55, 2),
            "visible worm whose sprite box overlaps the +/-dist box is hit"
        );
    }

    // ---- Step 7: blood_trail arm (T3) ----------------------------------------

    // A blood-nobject shaped like TC type 6: blood_trail with delay 10, flies in air
    // (expl_ground=false, gravity), no bounce/anim/timeout/hit. Its ONLY rand-drawing
    // behaviour is the blood-trail's CreateBObject when the cycles gate opens.
    fn blood_nobject(id: i32, delay: i32) -> NObjectType {
        NObjectType {
            id,
            blood_trail: true,
            blood_trail_delay: delay,
            expl_ground: false,
            bounce: 0,
            num_frames: 0,
            hit_damage: 0,
            time_to_explo: 0,
            create_on_exp: -1,
            dirt_effect: -1,
            splinter_amount: 0,
            leave_obj: -1,
            gravity: 700,
            ..Default::default()
        }
    }

    // Run nobject_process with a real bobjects pool + blood-colour constants, so the
    // blood-trail arm is exercised. Returns the verdict; the caller inspects bobjects.
    #[allow(clippy::too_many_arguments)]
    fn run_process_blood(
        obj: &mut NObject,
        ty: &NObjectType,
        level: &mut LevelSim,
        cossin: &[Vec2; 128],
        bobjects: &mut BloodPool<BObject>,
        cycles: i32,
        num_blood_colours: i32,
        first_blood_colour: i32,
        rand: &mut Rand,
    ) -> NObjectOutcome {
        let sprites = no_sprites();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut sobjects: Pool<SObject> = Pool::new(1);
        let mut nobjects: Pool<NObject> = Pool::new(8);
        nobject_process(
            obj,
            ty,
            &[],
            &[],
            level,
            cossin,
            &sprites,
            &sprites,
            &[],
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            bobjects,
            cycles,
            100,
            num_blood_colours,
            first_blood_colour,
            rand,
        )
    }

    #[test]
    fn blood_trail_fires_only_when_cycles_mod_delay_is_zero() {
        let cossin = precompute_cossin();
        let ty = blood_nobject(6, 10);

        // cycles=10 -> 10 % 10 == 0 -> fires exactly one CreateBObject.
        {
            let mut level = bg_level(200, 200);
            let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
            let mut rand = seeded();
            // Reference: one rand(NumBloodColours) draw (the only draw this tick).
            let mut refr = seeded();
            let _ = refr.bound(9);

            let mut obj = NObject {
                pos: Vec2::new(itof(50), itof(50)),
                vel: Vec2::new(itof(4), itof(8)),
                ty: Some(6),
                ..Default::default()
            };
            let out = run_process_blood(
                &mut obj, &ty, &mut level, &cossin, &mut bobjects, 10, 9, 64, &mut rand,
            );

            assert_eq!(out, NObjectOutcome::Keep, "blood nobject flies on (no explode)");
            assert_eq!(bobjects.len(), 1, "cycles=10, delay=10 -> one bobject spawned");
            let b = *bobjects.iter().next().unwrap();
            // pos is the nobject's pos AFTER `pos += vel` (no bounce): 50+4, 50+8.
            assert_eq!(
                b.pos,
                Vec2::new(itof(54), itof(58)),
                "bobject.pos = nobject pos (post pos+=vel)"
            );
            // vel = nobject vel / 4 (truncating). Gravity is added to the NOBJECT's
            // vel later, after the trail arm, so the captured vel is the pre-gravity
            // post-bounce value / 4.
            assert_eq!(
                b.vel,
                Vec2::new(itof(4), itof(8)).div(4),
                "bobject.vel = nobject vel / 4"
            );
            assert_eq!(
                rand.last(),
                refr.last(),
                "CreateBObject drew exactly one rand(NumBloodColours)"
            );
        }

        // cycles=5 and cycles=3 -> non-zero remainder -> NO spawn, NO rand.
        for c in [5, 3] {
            let mut level = bg_level(200, 200);
            let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
            let mut rand = seeded();
            rand.bound(55); // pre-advance: "no rand" is a real assertion
            let rng_before = rand.last();

            let mut obj = NObject {
                pos: Vec2::new(itof(50), itof(50)),
                vel: Vec2::new(itof(4), itof(8)),
                ty: Some(6),
                ..Default::default()
            };
            run_process_blood(
                &mut obj, &ty, &mut level, &cossin, &mut bobjects, c, 9, 64, &mut rand,
            );

            assert_eq!(bobjects.len(), 0, "cycles={c}, delay=10 -> no bobject");
            assert_eq!(rand.last(), rng_before, "cycles={c}: blood-trail drew no rand");
        }
    }

    #[test]
    fn blood_trail_dormant_when_delay_zero() {
        // blood_trail_delay == 0 -> the `delay > 0` guard short-circuits, so no spawn
        // and (crucially) no `% 0` divide. Pins the guard's load-bearing order.
        let cossin = precompute_cossin();
        let ty = blood_nobject(6, 0);
        let mut level = bg_level(200, 200);
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        let mut rand = seeded();
        rand.bound(11);
        let rng_before = rand.last();

        let mut obj = NObject {
            pos: Vec2::new(itof(50), itof(50)),
            vel: Vec2::new(itof(4), itof(8)),
            ty: Some(6),
            ..Default::default()
        };
        // cycles=0 would satisfy `% delay == 0` if delay were non-zero; delay=0 must
        // still suppress it without panicking.
        run_process_blood(
            &mut obj, &ty, &mut level, &cossin, &mut bobjects, 0, 9, 64, &mut rand,
        );

        assert_eq!(bobjects.len(), 0, "delay==0 -> no spawn (and no %0 panic)");
        assert_eq!(rand.last(), rng_before, "delay==0 drew no rand");
    }
}
