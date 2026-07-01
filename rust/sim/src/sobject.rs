//! Port of `SObjectType::Create` (`sobject.cpp:16-228`) + `SObject::Process`
//! (`sobject.cpp:230-241`) — **the core of Slice 4c**.
//!
//! [`sobject_create`] is the explosion entry point reached from
//! `WObject::BlowUpObject` (`weapon.cpp:90`): it spawns the [`SObject`], runs the
//! worm-damage block, scatters dirt debris, and carves the crater. [`SObject::Process`]
//! ([`sobject_process`]) drives the hashed `cur_frame` animation and frees the
//! object when the animation runs out.
//!
//! ## The RNG cluster contract (the whole game)
//!
//! At the explode tick the `rand()` draws fire in EXACTLY this source order
//! (`sobject.cpp`, consolidated in the slice-4c dossier §8):
//!
//! 1. **Sound** (`:24`): `rand(num_sounds)` iff `start_sound >= 0`. The sound
//!    `Play` is a hashing no-op, but **the `rand` is consumed** — skipping it
//!    shifts every later draw.
//! 2. **Worm-damage block** (`:47-114`): the box test + blow-away nudges (no rand)
//!    plus the LIVE `w.health > 0` damage arm (Slice 5b): `DoDamage` (RNG-free
//!    wound) then the `kBloodAmount × [rand(128) + Create2(rand(speed_v) +
//!    rand(distribution*2)×2)]` blood fan, then the `rand(3)` hit-sound gate
//!    (always drawn; a 2nd `rand(3)` iff the gate hits 0). The spawned blood
//!    nobjects (type 6, `blood_trail=true`) are NOT Process'd here — their
//!    blood-trail arm is still deferred (T3).
//! 3. **wobjects / nobjects blow-away loops** (`:118-186`): nudge `vel` of pooled
//!    objects with `affect_by_explosions`; **draw NO rand**. The
//!    `chain_explosion -> BlowUpObject` recursion is **deferred (O9)**.
//! 4. **Dirt-throw** (`:188-205`): `kWidth = detect_range/2`; the
//!    `Rect(x-kWidth, y-kWidth, x+kWidth+1, y+kWidth+1)` intersected with the
//!    level bounds is scanned **row-major (`y` outer, `x` inner)**. Per cell the
//!    short-circuit `any_dirt(x,y) && rand(8) == 0` draws `rand(8)` **only for
//!    AnyDirt cells, reading PRE-CARVE terrain**; on a `0` it reads
//!    `kPix = material_id[y*width + x]`, draws `rand(128)` (the angle), then spawns
//!    a dirt nobject via [`nobject_create2`] (`nobject_types[2]`: `rand(40)`,
//!    `rand(20000)`, `rand(20000)`).
//! 5. **Crater** (`:209-210`): iff `dirt_effect >= 0`, [`draw_dirt_effect`] carves
//!    the level and draws `rand(r_frame)` — the LAST cluster draw. `CorrectShadow`
//!    is omitted (`settings->shadow = false`, O4).
//! 6. **Bonus loop** (`:217-227`): empty pool in 4c ⇒ 0 draws; deferred (the
//!    recursive `sobject_types[0].Create` would need the bonus pool + full
//!    recursion).
//!
//! ## The three load-bearing traps
//!
//! * **Carve LAST.** The dirt-throw scans `any_dirt` and reads `kPix` on the
//!   ORIGINAL material; [`draw_dirt_effect`] writes `material_id` AFTER. Carving
//!   first would change which cells are AnyDirt and miscount the `rand(8)`s.
//! * **`Create2` draws `rand(speed_v)` FIRST**, then the distribution scatter —
//!   encoded in [`nobject_create2`]; the dirt-throw just calls it.
//! * **Sound `rand(2)` is consumed** even though the hash ignores sound; skipping
//!   it shifts every later draw.
//!
//! `sim` stays Bevy- and float-free: every `vel` nudge uses `wrapping_*`, the
//! `cossin * speed / 100` scaling inside [`nobject_create2`] truncates, and
//! `Ftoi` is the arithmetic `>> 16` ([`ftoi`]). The stats calls
//! (`DamagePotential`/`Hit`/`DamageDealt`), the viewport `shake` loop, and the
//! `screen_flash` write draw no rand and touch no hashed state, so they are
//! omitted exactly as the other ports omit their stats/render side effects.

use assets::object::{NObjectType, SObjectType, Weapon};
use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::fixed::{ftoi, itof};
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::blit::draw_dirt_effect;
use crate::nobject::nobject_create2;
use crate::pool::Pool;
use crate::state::{LevelSim, NObject, SObject, WObject, WormState};

/// The verdict a single [`sobject_process`] pass returns to the driver
/// (Task 4/5), mirroring the `Free(this)` tail of C++ `SObject::Process`
/// (`sobject.cpp:237-238`):
///
/// * [`Keep`](SObjectOutcome::Keep) — the object lives on.
/// * [`Free`](SObjectOutcome::Free) — `cur_frame > num_frames`: the driver frees
///   the slot. Split out (instead of freeing inside `Process`) because the C++
///   frees `this` mid-iteration, which the borrow checker forbids while the
///   driver still holds the pool — the same pattern as
///   [`crate::weapon::WObjectOutcome`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SObjectOutcome {
    Keep,
    Free,
}

/// Port of `SObjectType::Create` (`sobject.cpp:16-228`) — spawn the explosion
/// sobject, run the worm-damage block, scatter dirt debris, and carve the crater.
///
/// `x`/`y` are the impact pixel coordinates (`Ftoi` of the dart's fixed-point
/// position, from `BlowUpObject`); `owner_idx` is the firing worm. The whole
/// function is ported in C++ statement order; see the module docs for the exact
/// `rand()` cluster. Branches the 4c fixture cannot exercise are guarded:
///
/// * worm-in-range **damage arm** (`w.health > 0`) — LIVE (Slice 5b): DoDamage +
///   the `rand(128)` blood fan (`nobject_types[6].Create2`) + the `rand(3)`
///   hit-sound gate. ScalesOfJustice redistribution stays mode-gated/deferred;
/// * **chain_explosion** recursion in the wobjects loop — `debug_assert!`ed off (O9);
/// * the **bonus loop** (`:217-227`) — omitted (needs the bonus pool + recursive
///   `Create`); rand-neutral when no bonus sits in range, which the 4c fixture
///   guarantees.
#[allow(clippy::too_many_arguments)]
pub fn sobject_create(
    ty: &SObjectType,
    x: i32,
    y: i32,
    owner_idx: i32,
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    weapons: &[Weapon],
    nobjects: &mut Pool<NObject>,
    nobject_types: &[NObjectType],
    level: &mut LevelSim,
    cossin: &[Vec2; 128],
    large_sprites: &SpriteSet,
    textures: &[Texture],
    sobjects: &mut Pool<SObject>,
    blood: i32,
    rand: &mut Rand,
) {
    // :19 NewObjectReuse + :35-39 field init. Allocated first; the field writes
    // carry no rand and the slot index is deterministic, so spawning here (with
    // the final constant fields) is equivalent to the C++ alloc-then-write split.
    // id + cur_frame are hashed; x (= x-8), y (= y-8), anim_delay are not.
    sobjects
        .spawn(SObject {
            id: ty.id,
            x: x - 8,
            y: y - 8,
            cur_frame: 0,
            anim_delay: ty.anim_delay,
        })
        .expect("sobjects pool not full in 4c (NewObjectReuse overwrite deferred)");

    // :23-25 sound — the FIRST observable rand. Consumed even though `Play` is a
    // hashing no-op; the rand is the argument, evaluated before `Play`.
    if ty.start_sound >= 0 {
        rand.bound(ty.num_sounds as u32);
    }

    // :27-33 viewport shake + :41 screen_flash: render-only, no rand — omitted.

    let dr = ty.detect_range;

    // :47-207 `if (damage > 0)` — the damage block (entered for small_explosion).
    if ty.damage > 0 {
        // --- 7a. Per-worm loop (:48-114). In 4c every worm is out of range, so
        // the box test is false and nothing runs. Box test + blow-away nudges
        // (no rand) ported; the `w.health > 0` damage arm deferred (O10).
        for w in worms.iter_mut() {
            let kwix = ftoi(w.pos.x);
            let kwiy = ftoi(w.pos.y);

            // :54-55 range gate (strict on all four sides).
            if kwix < x + dr && kwix > x - dr && kwiy < y + dr && kwiy > y - dr {
                // :56-67 x blow-away nudge (gated `abs(vel.x) < Itof(2)`, no rand).
                let delta_x = kwix - x;
                let power_x = dr - delta_x.abs();
                if w.vel.x.abs() < itof(2) {
                    if delta_x > 0 {
                        w.vel.x = w.vel.x.wrapping_add(ty.blow_away.wrapping_mul(power_x));
                    } else {
                        w.vel.x = w.vel.x.wrapping_sub(ty.blow_away.wrapping_mul(power_x));
                    }
                }

                // :69-80 y blow-away nudge (symmetric).
                let delta_y = kwiy - y;
                let power_y = dr - delta_y.abs();
                if w.vel.y.abs() < itof(2) {
                    if delta_y > 0 {
                        w.vel.y = w.vel.y.wrapping_add(ty.blow_away.wrapping_mul(power_y));
                    } else {
                        w.vel.y = w.vel.y.wrapping_sub(ty.blow_away.wrapping_mul(power_y));
                    }
                }

                // :58/:71 power_sum: starts as the x `power`, then folds in the y
                // `power` as `(power_sum + power) / 2` (truncating). Reuses the
                // already-computed power_x/power_y (do NOT recompute differently).
                let power_sum = (power_x + power_y) / 2;

                // :82-85 z = damage * power_sum, then `if (detect_range) z /=
                // detect_range` (truncating).
                let mut z = ty.damage * power_sum;
                if dr != 0 {
                    z /= dr;
                }

                // :87-90 `if (from && !from->has_hit) Hit(...)` — stats only, no
                // sim/RNG; omitted (the `from`/`has_hit` plumbing is unported).

                // :92-112 the LIVE worm-damage arm (5b/O10 turned ON). Gated on
                // `w.health > 0`; draws RNG, so the order is the contract.
                if w.health > 0 {
                    // :93 DoDamage(w, z, owner_idx) — RNG-free wound (normal mode).
                    w.do_damage(z, owner_idx);
                    // :94 DamageDealt stat — omitted (no sim/RNG).

                    // :96 kBloodAmount = settings.blood * power_sum / 100 (trunc).
                    let k_blood_amount = blood * power_sum / 100;

                    // :98-103 the blood fan. Per particle: rand(128) [kAngle] THEN
                    // nobject_types[6].Create2(kAngle, w.vel/3, w.pos, 0, w.index,
                    // fired_by) — Create2 draws rand(speed_v) then rand(dist*2)x2.
                    // owner is `w.index` (the wounded worm), NOT the explosion
                    // owner_idx. `w.vel` is the POST blow-away nudge velocity; the
                    // `/3` is the truncating per-component divide. (fired_by is
                    // stats-only — unported in nobject_create2.) The blood nobject
                    // (type 6) has blood_trail=true; it is NOT Process'd here.
                    if k_blood_amount > 0 {
                        for _ in 0..k_blood_amount {
                            let k_angle = rand.bound(128) as i32;
                            nobject_create2(
                                &nobject_types[6],
                                k_angle,
                                w.vel.div(3),
                                w.pos,
                                0,
                                w.index,
                                cossin,
                                rand,
                                nobjects,
                            );
                        }
                    }

                    // :105-111 hit-sound gate: rand(3) is ALWAYS drawn; on `== 0`
                    // a SECOND rand(3) picks `18 + rand(3)`. The `Play` is a hashing
                    // no-op (skipped) but the rand draws are the contract.
                    if rand.bound(3) == 0 {
                        let _k_snd = 18 + rand.bound(3) as i32;
                        // sound_player->Play(kSnd) — omitted (no sim/RNG).
                    }
                }
            }
        }

        // --- 7b. wobjects blow-away loop (:118-153). Nudges `vel` of wobjects
        // with `affect_by_explosions`; draws NO rand. chain_explosion deferred (O9).
        let obj_blow_away = ty.blow_away / 3;
        for i in wobjects.iter_mut() {
            let weapon = &weapons[i
                .ty
                .expect("live wobject must carry a resolved weapon type")
                as usize];
            if weapon.affect_by_explosions {
                let ipx = ftoi(i.pos.x);
                let ipy = ftoi(i.pos.y);
                if ipx < x + dr && ipx > x - dr && ipy < y + dr && ipy > y - dr {
                    // x nudge: note the `else if delta < 0` — delta == 0 does
                    // nothing (distinct from the worm loop's `if/else`).
                    let delta = ipx - x;
                    let power = dr - delta.abs();
                    if power > 0 {
                        if delta > 0 {
                            i.vel.x = i.vel.x.wrapping_add(obj_blow_away.wrapping_mul(power));
                        } else if delta < 0 {
                            i.vel.x = i.vel.x.wrapping_sub(obj_blow_away.wrapping_mul(power));
                        }
                    }
                    let delta = ipy - y;
                    let power = dr - delta.abs();
                    if power > 0 {
                        if delta > 0 {
                            i.vel.y = i.vel.y.wrapping_add(obj_blow_away.wrapping_mul(power));
                        } else if delta < 0 {
                            i.vel.y = i.vel.y.wrapping_sub(obj_blow_away.wrapping_mul(power));
                        }
                    }
                    debug_assert!(
                        !weapon.chain_explosion,
                        "chain_explosion -> BlowUpObject recursion deferred (O9)"
                    );
                }
            }
        }

        // --- 7c. nobjects blow-away loop (:155-186). Identical structure; NO rand.
        for i in nobjects.iter_mut() {
            let t = &nobject_types[i
                .ty
                .expect("live nobject must carry a resolved type") as usize];
            if t.affect_by_explosions {
                let ipx = ftoi(i.pos.x);
                let ipy = ftoi(i.pos.y);
                if ipx < x + dr && ipx > x - dr && ipy < y + dr && ipy > y - dr {
                    let delta = ipx - x;
                    let power = dr - delta.abs();
                    if power > 0 {
                        if delta > 0 {
                            i.vel.x = i.vel.x.wrapping_add(obj_blow_away.wrapping_mul(power));
                        } else if delta < 0 {
                            i.vel.x = i.vel.x.wrapping_sub(obj_blow_away.wrapping_mul(power));
                        }
                    }
                    let delta = ipy - y;
                    let power = dr - delta.abs();
                    if power > 0 {
                        if delta > 0 {
                            i.vel.y = i.vel.y.wrapping_add(obj_blow_away.wrapping_mul(power));
                        } else if delta < 0 {
                            i.vel.y = i.vel.y.wrapping_sub(obj_blow_away.wrapping_mul(power));
                        }
                    }
                }
            }
        }

        // --- 7d. DIRT-THROW (:188-205) — the heart of the slice.
        // kWidth = detect_range/2 (truncating). The 9x9 box is intersected with
        // the level bounds (Rect::Intersect = max(x1)/max(y1)/min(x2)/min(y2)).
        let kwidth = dr / 2;
        let rx1 = (x - kwidth).max(0);
        let ry1 = (y - kwidth).max(0);
        let rx2 = (x + kwidth + 1).min(level.width);
        let ry2 = (y + kwidth + 1).min(level.height);

        // Row-major: y outer, x inner (:195-196). Per cell the short-circuit
        // `any_dirt(x,y) && rand(8) == 0` draws rand(8) ONLY for AnyDirt cells,
        // reading the PRE-CARVE material. The whole dirt-throw runs BEFORE the
        // carve below, so kPix is the original colour.
        for yy in ry1..ry2 {
            for xx in rx1..rx2 {
                if level.any_dirt(xx, yy) && rand.bound(8) == 0 {
                    // :198 kPix = Pixel(x,y) == material_id[y*width + x].
                    let kpix = level.material_id[(yy * level.width + xx) as usize] as i32;
                    // :199 kAngle = rand(128).
                    let kangle = rand.bound(128) as i32;
                    // :200-201 nobject_types[2].Create2(kAngle, fixedvec(),
                    // Itof(IVec2(x,y)), kPix, owner_idx, fired_by). Create2 draws
                    // rand(speed_v) then the two rand(distribution*2).
                    nobject_create2(
                        &nobject_types[2],
                        kangle,
                        Vec2::zero(),
                        Vec2::new(itof(xx), itof(yy)),
                        kpix,
                        owner_idx,
                        cossin,
                        rand,
                        nobjects,
                    );
                }
            }
        }
    }

    // :209-215 crater. Carve AFTER the dirt-throw (the trap): draw_dirt_effect
    // writes material_id, and its rand(r_frame) is the LAST cluster draw.
    // CorrectShadow omitted (settings->shadow = false, O4).
    if ty.dirt_effect >= 0 {
        draw_dirt_effect(
            level,
            large_sprites,
            textures,
            ty.dirt_effect,
            x - 7,
            y - 7,
            rand,
        );
    }

    // :217-227 bonus loop — omitted (needs the bonus pool + the recursive
    // sobject_types[0].Create). Rand-neutral when no bonus sits in range, which
    // the 4c fixture guarantees.
}

/// Port of `SObject::Process` (`sobject.cpp:230-241`) — advance one sobject by
/// one tick. Draws **NO rand**.
///
/// `--anim_delay <= 0` reloads `anim_delay = t.anim_delay`, increments the hashed
/// `cur_frame`, and frees the object once `cur_frame > t.num_frames` (STRICT, so
/// it lives for `num_frames + 1` displayed frames). The pre-decrement matches C++
/// (`:234`). Returns the [`SObjectOutcome`]; the driver performs the free.
pub fn sobject_process(obj: &mut SObject, ty: &SObjectType) -> SObjectOutcome {
    obj.anim_delay -= 1;
    if obj.anim_delay <= 0 {
        obj.anim_delay = ty.anim_delay;
        obj.cur_frame += 1;
        if obj.cur_frame > ty.num_frames {
            return SObjectOutcome::Free;
        }
    }
    SObjectOutcome::Keep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MAT_BACKGROUND, MAT_DIRT, MAT_ROCK};
    use sim_core::tables::precompute_cossin;

    // A seed whose successive draws are distinct, so an order swap / miscount is
    // detectable.
    const SEED: u32 = 0x4242;

    fn seeded() -> Rand {
        let mut r = Rand::new();
        r.seed(SEED);
        r
    }

    // small_explosion (the 4c sobject): start_sound >= 0 (num_sounds 2),
    // anim_delay 2, num_frames 5, detect_range 8 (=> kWidth 4, a 9x9 box),
    // damage > 0, blow_away set. `dirt_effect` is overridden per test (-1 to
    // isolate the dirt-throw, 2 to exercise the carve).
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

    // The dirt particle nobject_types[2]: speed_v 40, distribution 10000,
    // start_frame <= 0, num_frames 0, time_to_explo(_v) 0 — so Create2 draws
    // exactly rand(40), rand(20000), rand(20000), and Create resolves
    // cur_frame = color (= kPix) with NO draw. Index 2 in the table.
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

    // nobject_types table padded so index 2 is the dirt particle (the dirt-throw
    // indexes [2]; indices 0/1 are unused placeholders here).
    fn nobject_types() -> Vec<NObjectType> {
        vec![NObjectType::default(), NObjectType::default(), dirt_nobject()]
    }

    // An all-background level (material 0 = Background, NOT dirt) — the dirt-throw
    // draws nothing on it.
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

    fn empty_pools() -> (Pool<WObject>, Pool<NObject>, Pool<SObject>) {
        (Pool::new(600), Pool::new(600), Pool::new(700))
    }

    // ---- Step 1: sound + obj init -------------------------------------------

    #[test]
    fn create_inits_sobject_and_draws_one_sound_rand_first() {
        let cossin = precompute_cossin();
        let ty = small_explosion(-1); // no carve: the sound is the only draw
        let nts = nobject_types();
        let mut level = bg_level(100, 100); // no dirt -> dirt-throw draws nothing
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let mut rand = seeded();

        // Reference: exactly one rand(num_sounds) = rand(2).
        let mut refr = seeded();
        refr.bound(2);
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        // Obj init (:35-39): id = 2, x = 50-8, y = 50-8, cur_frame = 0,
        // anim_delay = 2.
        assert_eq!(sobjects.len(), 1, "exactly one sobject spawned");
        let obj = *sobjects.get(0).expect("sobject in slot 0");
        assert_eq!(obj.id, 2, "obj.id = type id");
        assert_eq!(obj.x, 42, "obj.x = x - 8");
        assert_eq!(obj.y, 42, "obj.y = y - 8");
        assert_eq!(obj.cur_frame, 0, "obj.cur_frame = 0");
        assert_eq!(obj.anim_delay, 2, "obj.anim_delay = type anim_delay");

        // Exactly one rand drawn (the sound), and it is the first.
        assert_eq!(
            rand.last(),
            expected_last,
            "one rand(num_sounds) consumed first; nothing else drew"
        );
        assert_eq!(nobjects.len(), 0, "no dirt (bg level) -> no debris");
    }

    #[test]
    fn create_start_sound_negative_draws_no_sound_rand() {
        // start_sound < 0 -> the sound rand is NOT drawn. With a bg level + no
        // carve, the whole explosion draws zero rand.
        let cossin = precompute_cossin();
        let mut ty = small_explosion(-1);
        ty.start_sound = -1;
        let nts = nobject_types();
        let mut level = bg_level(100, 100);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let mut rand = seeded();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        assert_eq!(rand.last(), 0, "start_sound < 0 -> no rand drawn at all");
        assert_eq!(sobjects.len(), 1, "sobject still spawned");
    }

    // ---- Step 2: worm loop inert (O10) --------------------------------------

    // A minimal worm at a given pixel position with a chosen health. vel starts
    // at zero (so the `abs(vel) < Itof(2)` blow-away gate is open).
    fn worm_at(px: i32, py: i32, health: i32) -> WormState {
        let mut w = WormState::from_init(&crate::state::WormInit {
            index: 0,
            health,
            lives: 5,
            stats_x: 0,
            weapons: [crate::state::WeaponInit::default(); crate::state::NUM_WEAPONS],
            start_pos: Vec2::new(itof(px), itof(py)),
            visible: true,
        });
        w.vel = Vec2::zero();
        w
    }

    #[test]
    fn worm_outside_box_is_untouched_and_draws_nothing() {
        // detect_range 8 -> in-box is kwix in [x-7, x+7]. A worm at x+9 is OUT:
        // the box test is false, vel untouched, no rand beyond the sound.
        let cossin = precompute_cossin();
        let ty = small_explosion(-1);
        let nts = nobject_types();
        let mut level = bg_level(100, 100);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms = vec![worm_at(59, 50, 100)]; // x=50 -> 59 is x+9, OUT
        let mut rand = seeded();

        let mut refr = seeded();
        refr.bound(2); // sound only
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        assert_eq!(worms[0].vel, Vec2::zero(), "out-of-range worm not nudged");
        assert_eq!(
            rand.last(),
            expected_last,
            "out-of-range worm draws no rand (box test false)"
        );
    }

    #[test]
    fn worm_inside_box_is_nudged_but_dead_worm_draws_no_rand() {
        // A worm at x+7 (inside the box) with health <= 0: the blow-away nudge
        // runs (proving the box test branch is taken) but the `health > 0` damage
        // arm (the only rand source in the worm loop) is skipped. Covers the box
        // test without tripping the O10 debug_assert.
        let cossin = precompute_cossin();
        let ty = small_explosion(-1);
        let nts = nobject_types();
        let mut level = bg_level(100, 100);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms = vec![worm_at(57, 50, 0)]; // x=50 -> 57 is x+7, IN; health 0
        let mut rand = seeded();

        let mut refr = seeded();
        refr.bound(2); // sound only (dead worm draws nothing more)
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        // delta_x = 57-50 = 7 > 0 -> vel.x += blow_away * (8 - 7) = 3000.
        assert_eq!(worms[0].vel.x, 3000, "in-box worm gets x blow-away nudge");
        // delta_y = 0 -> power_y = 8; delta not > 0 -> vel.y -= 3000 * 8.
        assert_eq!(
            worms[0].vel.y,
            -(3000 * 8),
            "in-box worm gets y blow-away nudge (delta 0 -> minus branch)"
        );
        assert_eq!(
            rand.last(),
            expected_last,
            "dead in-box worm: damage arm skipped, no extra rand"
        );
    }

    // ---- Step 2b: the LIVE worm-damage arm (5b/O10) -------------------------

    // The blood nobject (nobject_types[6], blood_trail = true). Tuned so the arm's
    // per-particle draw shape is exactly [rand(speed_v), rand(distribution*2),
    // rand(distribution*2)] inside Create2 and NOTHING inside Create:
    //   speed_v = 40         -> rand(40)
    //   distribution = 20000 -> rand(40000) x2 (Create2 draws rand(distribution*2))
    //   start_frame <= 0     -> cur_frame = color/color_bullets (NO rand)
    //   time_to_explo_v = 0  -> no time jitter (NO rand)
    // Matches the brief's blood-spray draw shape [rand(128), rand(40), rand(40000),
    // rand(40000)] (the rand(128) kAngle is drawn by the arm BEFORE Create2).
    fn blood_nobject() -> NObjectType {
        NObjectType {
            id: 6,
            speed: 100,
            speed_v: 40,
            distribution: 20000,
            start_frame: 0,
            num_frames: 0,
            color_bullets: 0,
            time_to_explo: 0,
            time_to_explo_v: 0,
            blood_trail: true,
            ..Default::default()
        }
    }

    // nobject_types table padded so index 6 is the blood particle (index 2 is the
    // dirt particle; the rest are unused placeholders).
    fn nts_with_blood() -> Vec<NObjectType> {
        let mut v = vec![NObjectType::default(); 7];
        v[2] = dirt_nobject();
        v[6] = blood_nobject();
        v
    }

    #[test]
    fn worm_inside_box_wounded_sprays_blood_with_exact_draw_order() {
        // A worm at x+7 (inside the box) with health 100: the LIVE damage arm
        // wounds it (health stays > 0), spawns kBloodAmount type-6 blood nobjects,
        // and draws the exact RNG cluster. We do NOT Process the spawned blood
        // nobjects (their blood_trail arm is deferred).
        let cossin = precompute_cossin();
        let ty = small_explosion(-1); // no carve: isolate the worm arm + (inert) dirt-throw
        let nts = nts_with_blood();
        let mut level = bg_level(100, 100); // no dirt -> dirt-throw draws nothing
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms = vec![worm_at(57, 50, 100)]; // x=50 -> 57 is x+7, IN; healthy
        let mut rand = seeded();
        let blood = 100;

        // Hand-computed arithmetic (no RNG):
        //  power_x = dr - |57-50| = 8 - 7 = 1; power_y = dr - |50-50| = 8.
        //  power_sum = (1 + 8) / 2 = 4 (truncating).
        //  z = damage(5) * power_sum(4) = 20; dr != 0 -> z /= 8 -> 2.
        //  health 100 - 2 = 98 (> 0 -> wound, not kill).
        //  kBloodAmount = blood(100) * power_sum(4) / 100 = 4.
        // Post blow-away nudge: vel.x += 3000*1 = 3000; vel.y -= 3000*8 = -24000.
        // Create2 base vel = w.vel / 3 = (1000, -8000) (truncating per component).
        let expected_blood = 4;

        // Reference stream (separately seeded): sound, then the worm arm's draws.
        let mut refr = seeded();
        refr.bound(2); // :24 sound
        for _ in 0..expected_blood {
            refr.bound(128); // :100 kAngle
            refr.bound(40); // Create2 :53 rand(speed_v)
            refr.bound(40000); // Create2 :59 rand(distribution*2) x
            refr.bound(40000); // Create2 :60 rand(distribution*2) y
        }
        // :105 hit-sound gate: rand(3) ALWAYS; on 0 a SECOND rand(3) (kSnd pick).
        if refr.bound(3) == 0 {
            refr.bound(3);
        }
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, blood, &mut rand,
        );

        // Wound: health dropped by the clamped z = 2, stays > 0; not a kill, so
        // last_killed_by_idx is untouched (-1).
        assert_eq!(worms[0].health, 98, "health reduced by z = 2 (wound)");
        assert!(worms[0].health > 0, "worm wounded, not killed");
        assert_eq!(
            worms[0].last_killed_by_idx, -1,
            "wound (health > 0) does NOT set last_killed_by_idx"
        );

        // The blow-away nudge is the ONLY thing that changed w.vel; the blood
        // spray reads w.vel / 3 by value and does not mutate it.
        assert_eq!(worms[0].vel, Vec2::new(3000, -24000), "vel = post blow-away nudge");

        // Exactly kBloodAmount type-6 nobjects spawned, each owned by w.index (0).
        assert_eq!(
            nobjects.len(),
            expected_blood as usize,
            "kBloodAmount = blood * power_sum / 100 = 4 blood nobjects"
        );
        for k in 0..nobjects.len() {
            let b = *nobjects.get(k).expect("blood nobject in slot");
            assert_eq!(b.ty, Some(6), "blood nobject {k} is nobject_types[6]");
            assert_eq!(b.owner_idx, 0, "blood nobject {k} owned by w.index");
        }

        // Exact RNG order/count: sound, then per blood particle [rand(128),
        // rand(40), rand(40000), rand(40000)], then the rand(3) gate (+ a 2nd
        // rand(3) iff the gate hit 0).
        assert_eq!(
            rand.last(),
            expected_last,
            "blood-spray + hit-sound RNG order/count matches the reference stream"
        );
    }

    #[test]
    fn worm_outside_box_takes_no_damage_and_sprays_no_blood() {
        // A healthy worm OUTSIDE the box: the arm is skipped entirely — no damage,
        // no blood, and the only draw is the sound.
        let cossin = precompute_cossin();
        let ty = small_explosion(-1);
        let nts = nts_with_blood();
        let mut level = bg_level(100, 100);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms = vec![worm_at(59, 50, 100)]; // x+9 -> OUT
        let mut rand = seeded();

        let mut refr = seeded();
        refr.bound(2); // sound only
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        assert_eq!(worms[0].health, 100, "out-of-range worm takes no damage");
        assert_eq!(worms[0].vel, Vec2::zero(), "out-of-range worm not nudged");
        assert_eq!(nobjects.len(), 0, "out-of-range worm sprays no blood");
        assert_eq!(rand.last(), expected_last, "only the sound rand drawn");
    }

    #[test]
    fn worm_inside_box_with_zero_blood_setting_draws_no_blood_but_still_gates_sound() {
        // blood = 0 -> kBloodAmount = 0: the blood loop runs zero times (no
        // rand(128)/Create2 draws, no spawns), but the hit-sound rand(3) gate is
        // STILL drawn (it is outside the `kBloodAmount > 0` guard). Discriminates
        // the gate from the blood fan.
        let cossin = precompute_cossin();
        let ty = small_explosion(-1);
        let nts = nts_with_blood();
        let mut level = bg_level(100, 100);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms = vec![worm_at(57, 50, 100)];
        let mut rand = seeded();

        let mut refr = seeded();
        refr.bound(2); // sound
        // No blood draws (kBloodAmount == 0); straight to the gate.
        if refr.bound(3) == 0 {
            refr.bound(3);
        }
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 0, &mut rand,
        );

        // Still wounded (DoDamage runs regardless of blood), but no blood nobjects.
        assert_eq!(worms[0].health, 98, "DoDamage still applied with blood = 0");
        assert_eq!(nobjects.len(), 0, "blood = 0 -> no blood nobjects");
        assert_eq!(
            rand.last(),
            expected_last,
            "hit-sound gate drawn even when kBloodAmount == 0"
        );
    }

    // ---- Step 3: dirt-throw RNG + spawn order -------------------------------

    // A level whose 9x9 box around (cx, cy) carries a KNOWN dirt/background
    // pattern: cells where `(xx + yy) % 2 == 0` are dirt (material id = the dirt
    // material `dirt_mat`, which doubles as kPix), the rest are background. This
    // checkerboard exercises the short-circuit (rand(8) only for dirt cells).
    fn checker_dirt_level(cx: i32, cy: i32, kwidth: i32, dirt_mat: u8) -> LevelSim {
        let mut level = bg_level(100, 100);
        level.material_flags[dirt_mat as usize] = MAT_DIRT;
        for yy in (cy - kwidth)..(cy + kwidth + 1) {
            for xx in (cx - kwidth)..(cx + kwidth + 1) {
                if (xx + yy) % 2 == 0 {
                    level.material_id[(yy * level.width + xx) as usize] = dirt_mat;
                }
            }
        }
        level
    }

    #[test]
    fn dirt_throw_row_major_short_circuit_and_exact_draw_count() {
        let cossin = precompute_cossin();
        let ty = small_explosion(-1); // no carve: isolate the dirt-throw
        let nts = nobject_types();
        let (cx, cy) = (50, 50);
        let dirt_mat: u8 = 10;
        let mut level = checker_dirt_level(cx, cy, 4, dirt_mat);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let mut rand = seeded();

        // Reference: replay the cluster in EXACT order. sound, then row-major over
        // the 9x9 box; rand(8) ONLY for dirt cells; on a 0, kPix + rand(128) +
        // Create2's rand(40), rand(20000), rand(20000).
        let mut refr = seeded();
        refr.bound(2); // sound
        let mut expected_kpix = Vec::new();
        for yy in (cy - 4)..(cy + 5) {
            for xx in (cx - 4)..(cx + 5) {
                if (xx + yy) % 2 == 0 {
                    // dirt cell
                    if refr.bound(8) == 0 {
                        let kpix = dirt_mat as i32;
                        refr.bound(128); // kAngle
                        refr.bound(40); // Create2 speed_v
                        refr.bound(20000); // dist x
                        refr.bound(20000); // dist y
                        expected_kpix.push(kpix);
                    }
                }
            }
        }
        let expected_last = refr.last();

        sobject_create(
            &ty, cx, cy, 3, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        // Exact total draw count + order: rand.last matches the reference iff the
        // dirt-throw drew rand(8) only for dirt cells, in row-major order, with
        // Create2's draws nested correctly.
        assert_eq!(
            rand.last(),
            expected_last,
            "dirt-throw rand order/count: sound, then per dirt cell rand(8) [+ if 0: rand(128), rand(40), rand(20000)x2]"
        );

        // Some cells must have fired (otherwise the test is vacuous).
        assert!(!expected_kpix.is_empty(), "seed must fire at least one cell");
        assert_eq!(
            nobjects.len(),
            expected_kpix.len(),
            "one dirt nobject spawned per fired cell"
        );

        // Each spawned debris (in pool slot == spawn order == row-major fire
        // order) carries cur_frame = kPix and ty = 2.
        for (k, &kpix) in expected_kpix.iter().enumerate() {
            let deb = *nobjects.get(k).expect("debris in slot");
            assert_eq!(deb.cur_frame, kpix, "debris {k} cur_frame = kPix");
            assert_eq!(deb.ty, Some(2), "debris {k} is nobject_types[2]");
            assert_eq!(deb.owner_idx, 3, "debris {k} carries owner_idx");
        }
    }

    #[test]
    fn dirt_throw_skips_when_box_has_no_dirt() {
        // A background-only box draws zero rand(8) (short-circuit), so the only
        // draw is the sound. Discriminates the short-circuit from "always draw".
        let cossin = precompute_cossin();
        let ty = small_explosion(-1);
        let nts = nobject_types();
        let mut level = bg_level(100, 100);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let mut rand = seeded();

        let mut refr = seeded();
        refr.bound(2);
        let expected_last = refr.last();

        sobject_create(
            &ty, 50, 50, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &SpriteSet::default(), &[], &mut sobjects, 100, &mut rand,
        );

        assert_eq!(rand.last(), expected_last, "no dirt -> no rand(8) drawn");
        assert_eq!(nobjects.len(), 0, "no debris");
    }

    // ---- Step 4: carving DrawDirtEffect reused (live) -----------------------

    const SPRITE_SIZE: usize = 256; // 16 x 16

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

    // A level whose ENTIRE 9x9 dirt-throw box (and the wider 16x16 carve window)
    // is dirt material `dirt_mat`. Carved cells become `fill_mat` (background).
    fn all_dirt_level(cx: i32, cy: i32, dirt_mat: u8, fill_mat: u8) -> LevelSim {
        let mut level = bg_level(80, 80);
        level.material_flags[dirt_mat as usize] = MAT_DIRT;
        level.material_flags[fill_mat as usize] = MAT_BACKGROUND;
        // Fill the whole carve window [cx-7, cx+9) x [cy-7, cy+9) with dirt.
        for yy in (cy - 7)..(cy + 9) {
            for xx in (cx - 7)..(cx + 9) {
                level.material_id[(yy * level.width + xx) as usize] = dirt_mat;
            }
        }
        level
    }

    #[test]
    fn carve_runs_after_dirt_throw_and_is_the_last_cluster_draw() {
        let cossin = precompute_cossin();
        let ty = small_explosion(2); // dirt_effect = 2 -> textures[2], carve LIVE
        let nts = nobject_types();
        let (cx, cy) = (40, 40);
        let dirt_mat: u8 = 10;
        let fill_mat: u8 = 7;
        let mut level = all_dirt_level(cx, cy, dirt_mat, fill_mat);
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();

        // Carving texture: ndrawback = true, mask all case-6, fill const = fill_mat
        // (a Background material). Over AnyDirt cells, case 6 writes fill[wrap].
        let sprites = make_sprites(
            84,
            &[
                (38, vec![6u8; SPRITE_SIZE]),  // mframe: all case-6
                (82, vec![fill_mat; SPRITE_SIZE]), // sframe: const fill_mat
                (83, vec![fill_mat; SPRITE_SIZE]), // sframe+1 (rframe 2)
            ],
        );
        let mut textures = vec![Texture::default(); 3];
        textures[2] = Texture {
            sframe: 82,
            rframe: 2,
            mframe: 38,
            ndrawback: true,
        };

        // Reference: sound, then dirt-throw over the all-dirt 9x9 box, then the
        // carve's rand(rframe) = rand(2) LAST.
        let mut refr = seeded();
        refr.bound(2); // sound
        let mut fired = 0;
        for _yy in (cy - 4)..(cy + 5) {
            for _xx in (cx - 4)..(cx + 5) {
                // every cell is dirt
                if refr.bound(8) == 0 {
                    refr.bound(128);
                    refr.bound(40);
                    refr.bound(20000);
                    refr.bound(20000);
                    fired += 1;
                }
            }
        }
        refr.bound(2); // carve rand(rframe), LAST
        let expected_last = refr.last();

        let mut rand = seeded();
        sobject_create(
            &ty, cx, cy, 1, &mut worms, &mut wobjects, &[], &mut nobjects, &nts,
            &mut level, &cossin, &sprites, &textures, &mut sobjects, 100, &mut rand,
        );

        // (a) the carve's rand(2) is the LAST draw of the cluster.
        assert_eq!(
            rand.last(),
            expected_last,
            "carve rand(rframe) is the last cluster draw (carve AFTER dirt-throw)"
        );

        // (b) carve actually mutated the level: the box dirt is gone.
        assert!(fired > 0, "at least one cell fired");
        assert_eq!(nobjects.len(), fired, "one debris per fired cell");
        let still_dirt = level.material_id[((cy) * level.width + cx) as usize];
        assert_eq!(
            still_dirt, fill_mat,
            "carve cleared the box dirt to the fill (Background) material"
        );

        // (c) PRE-CARVE read proof: every debris carries cur_frame = dirt_mat (the
        // colour BEFORE the carve), yet no box cell still holds dirt_mat. So the
        // dirt-throw read the original terrain, then the carve cleared it.
        let no_dirt_left = !level
            .material_id
            .iter()
            .skip(((cy - 7) * level.width) as usize)
            .take((15 * level.width) as usize)
            .any(|&m| m == dirt_mat);
        assert!(
            no_dirt_left,
            "carve cleared every dirt cell in the window"
        );
        for k in 0..nobjects.len() {
            assert_eq!(
                nobjects.get(k).unwrap().cur_frame,
                dirt_mat as i32,
                "debris {k} carries the PRE-CARVE kPix (now absent from the level)"
            );
        }
    }

    // ---- Step 5: sobject_process anim/free ----------------------------------

    #[test]
    fn process_advances_frame_every_anim_delay_and_frees_past_num_frames() {
        let ty = small_explosion(-1); // anim_delay 2, num_frames 5
        let mut obj = SObject {
            id: 2,
            x: 0,
            y: 0,
            cur_frame: 0,
            anim_delay: ty.anim_delay,
        };

        // cur_frame increments once every 2 ticks; the object frees when
        // cur_frame > 5. Hand-stepped: 6 increments x 2 ticks = 12 ticks.
        let mut frames_seen = Vec::new();
        let mut freed_at = None;
        for tick in 1..=20 {
            match sobject_process(&mut obj, &ty) {
                SObjectOutcome::Keep => frames_seen.push((tick, obj.cur_frame)),
                SObjectOutcome::Free => {
                    freed_at = Some(tick);
                    break;
                }
            }
        }

        assert_eq!(freed_at, Some(12), "frees on tick 12 (cur_frame 6 > 5)");
        // The reload sets anim_delay back to 2 each time it hits 0; the increment
        // ticks are the even ones.
        assert_eq!(
            frames_seen,
            vec![
                (1, 0), // --2 -> 1, no increment yet
                (2, 1), // --1 -> 0, reload, cur_frame 1
                (3, 1),
                (4, 2),
                (5, 2),
                (6, 3),
                (7, 3),
                (8, 4),
                (9, 4),
                (10, 5),
                (11, 5),
                // tick 12: cur_frame -> 6 > 5 -> Free (not pushed)
            ],
            "cur_frame steps every anim_delay=2 ticks"
        );
    }

    #[test]
    fn process_draws_no_rand_and_lives_num_frames_plus_one() {
        // The object is displayed for num_frames+1 = 6 frames (cur_frame 0..=5)
        // before freeing. sobject_process never touches a Rand (it takes none).
        let mut ty = small_explosion(-1);
        ty.anim_delay = 1; // one tick per frame, to count displayed frames simply
        ty.num_frames = 3;
        let mut obj = SObject {
            id: 2,
            x: 0,
            y: 0,
            cur_frame: 0,
            anim_delay: 1,
        };
        let mut distinct_frames = vec![obj.cur_frame];
        let mut freed_at = None;
        for tick in 1..=10 {
            match sobject_process(&mut obj, &ty) {
                SObjectOutcome::Keep => distinct_frames.push(obj.cur_frame),
                SObjectOutcome::Free => {
                    freed_at = Some(tick);
                    break;
                }
            }
        }
        assert_eq!(freed_at, Some(4), "anim_delay 1: frees on tick num_frames+1");
        // Displayed cur_frames before free: 0,1,2,3 (num_frames+1 = 4 frames).
        assert_eq!(distinct_frames, vec![0, 1, 2, 3]);
    }
}
