//! The world-block object draw: sprite-index selectors, the `shot_type` 2/3
//! `cur_frame` remap, and **pass 1** — `shadow_pass` — the six shadow families of
//! `viewport.cpp:274-398`, in exact C++ family order. Pass 2 (the sprite pass) is
//! T5; the selectors/remap here are shared by both passes.
//!
//! **Two-pass ordering is load-bearing** (Global Constraint). Every shadow of
//! every family composites BEFORE any sprite, and within the shadow pass the
//! families run bonuses -> sobjects -> wobjects -> nobjects -> worms(+ninjarope)
//! -> bobjects. A reordered family or a shadow drawn after a sprite moves the
//! frame hash. Each formula carries its `viewport.cpp` line.
//!
//! Shadows never resolve through `pal32`: `blit_shadow_image`/`draw_shadow_line`
//! and the single-pixel arms all paint the *darkened terrain* ARGB from the
//! `ShadowQuery` (which reads the LEVEL, so the writes are idempotent). The pass
//! is gated on `draw_shadow` at the caller (T5); `shadow_pass` itself assumes it
//! should draw.

use crate::bitmap::{Bitmap, ColorMode, Pal32};
use crate::blit::{
    blit_fire_cone, blit_image, blit_image_r, blit_shadow_image, draw_laser_sight, draw_line,
    draw_ninjarope, draw_shadow_line,
};
use crate::fire_cone::FIRE_CONE_OFFSET;
use crate::shadow_query::ShadowQuery;
use crate::viewport::Viewport;
use assets::sprite::SpriteSet;
use sim::state::{angle_frame, ControlState, SimState};
use sim_core::fixed::ftoi;

/// `common.hpp:150` `WormSprite(f, dir, w)`: the worm-body sprite index into the
/// `worm_sprites` bank. `f` is the animation frame, `dir` the facing (0/1), `w`
/// the worm index (0/1, doubling as the colour sub-bank). Shared by the shadow
/// pass (`viewport.cpp:382`) and the sprite pass (`:545`, T5).
pub fn worm_sprite_index(f: i32, dir: i32, index: i32) -> usize {
    (f + dir * 7 * 3 + index * 2 * 7 * 3) as usize
}

/// `common.hpp:153` `FireConeSprite(f, dir)`: index into the `2*7` fire-cone bank
/// ([`crate::fire_cone::build_fire_cone_sprites`]). Used by the sprite pass
/// (`viewport.cpp:540`, T5).
pub fn fire_cone_sprite_index(f: i32, dir: i32) -> usize {
    (f + dir * 7) as usize
}

/// `viewport.cpp:307-326` (== `:435-454`): the `shot_type` 2/3 `cur_frame` remap
/// applied to a wobject before selecting its `start_frame + cur_frame` sprite.
/// `shot_type` 0/1 (and any other) leave `cur_frame` untouched. Runs identically
/// in both passes, so it is a shared helper.
// Ported verbatim from C++ (`viewport.cpp:316-325`): the `shot_type == 3` tail is
// a lo/hi clamp, but keeping the `if/else if` mirrors the source line-for-line, so
// the `manual_clamp` suggestion is intentionally declined.
#[allow(clippy::manual_clamp)]
pub fn wobj_remap(mut cur_frame: i32, shot_type: i32) -> i32 {
    if shot_type == 2 {
        // viewport.cpp:308-314
        cur_frame += 4;
        cur_frame >>= 3;
        if cur_frame < 0 {
            cur_frame = 16;
        } else if cur_frame > 15 {
            cur_frame -= 16;
        }
    } else if shot_type == 3 {
        // viewport.cpp:316-325
        if cur_frame > 64 {
            cur_frame -= 1;
        }
        cur_frame -= 12;
        cur_frame >>= 3;
        if cur_frame < 0 {
            cur_frame = 0;
        } else if cur_frame > 12 {
            cur_frame = 12;
        }
    }
    cur_frame
}

/// `LC(BonusFlickerTime)` (TC `common.C[CBonusFlickerTime]`, `viewport.cpp:278`).
/// **Not threaded** into `shadow_pass`: no chosen 3b scenario spawns bonuses, so
/// the bonus loop body is dead (empty `bonuses` pool). Carried as the real TC
/// value (`data/TC/openliero/tc.cfg:51 = 220`) so the gate is faithful in shape;
/// promote to a parameter when a bonus scenario lands (deferral ledger, T9).
const BONUS_FLICKER_TIME: i32 = 220;

/// **Pass 1** — port of `viewport.cpp:274-398`, the six shadow families in C++
/// order. `off_x`/`off_y` are `kOffs` (`screen = world + kOffs`); the `shadow`
/// query carries the matching `world_offset = -kOffs` so its screen-keyed reads
/// hit the right level cell. `bonus_frames` maps a bonus's `frame` to its small
/// sprite (empty `&[]` for 3b — the bonus loop never runs).
///
/// The caller (T5) gates this on `draw_shadow`; here we always draw. Every write
/// is a darkened-terrain ARGB (or nothing) — no `pal32` resolve.
pub fn shadow_pass(
    scr: &mut Bitmap,
    state: &SimState,
    shadow: &ShadowQuery,
    off_x: i32,
    off_y: i32,
    bonus_frames: &[i32],
) {
    // (1) bonuses — viewport.cpp:276-284.
    for i in state.bonuses.iter() {
        // :278 flicker gate.
        if i.timer > BONUS_FLICKER_TIME || (state.cycles & 3) == 0 {
            let f = bonus_frames[i.frame as usize]; // :279
            // :280-282 — 7x7 small sprite at (Ftoi(x)-5, Ftoi(y)-1) + offs.
            blit_shadow_image(
                scr,
                shadow,
                &state.small_sprites,
                f as usize,
                ftoi(i.x) - 5 + off_x,
                ftoi(i.y) - 1 + off_y,
            );
        }
    }

    // (2) sobjects — viewport.cpp:287-298.
    for i in state.sobjects.iter() {
        let t = &state.sobject_types[i.id as usize]; // :290
        let frame = i.cur_frame + t.start_frame; // :291
        // :292-296 — 16x16 large sprite at (x-3, y+3) + offs. NB x/y are int pixels.
        blit_shadow_image(
            scr,
            shadow,
            &state.large_sprites,
            frame as usize,
            i.x + off_x - 3,
            i.y + off_y + 3,
        );
    }

    // (3) wobjects — viewport.cpp:300-343.
    for i in state.wobjects.iter() {
        let w = &state.weapons[i.ty.expect("live wobject has a weapon type") as usize]; // :303 *i->type
        if w.start_frame > -1 {
            // :304
            let cur_frame = wobj_remap(i.cur_frame, w.shot_type); // :305-326
            let pos_x = ftoi(i.pos.x) - 3; // :327
            let pos_y = ftoi(i.pos.y) - 3; // :328
            if w.shadow {
                // :329 — 7x7 small sprite at (kPosX-3, kPosY+3) + offs.
                blit_shadow_image(
                    scr,
                    shadow,
                    &state.small_sprites,
                    (w.start_frame + cur_frame) as usize,
                    pos_x - 3 + off_x,
                    pos_y + 3 + off_y,
                );
            }
        } else if i.cur_frame > 0 {
            // :334 — single shadowed pixel at (Ftoi(x)+offs-3, Ftoi(y)+offs+3).
            let pos_x = ftoi(i.pos.x) + off_x - 3; // :335
            let pos_y = ftoi(i.pos.y) + off_y + 3; // :336
            let sh = shadow.shadowed_argb(pos_x, pos_y); // :337
            // :338-339 — !=0 && Inside(kPosX, kPosY). put_argb re-applies the
            // Inside gate (Rect::inside), so the combined predicate is exact.
            if sh != 0 {
                scr.put_argb(pos_x, pos_y, sh);
            }
        }
    }

    // (4) nobjects — viewport.cpp:345-366.
    for i in state.nobjects.iter() {
        let t = &state.nobject_types[i.ty.expect("live nobject has a type") as usize]; // :348 *i->type
        if t.start_frame > 0 {
            // :349 — pos = Ftoi(pos) - (3,3); blit small[start_frame+cur_frame] at
            // (pos.x-3, pos.y+3) + offs (7x7).
            let px = ftoi(i.pos.x) - 3; // :350
            let py = ftoi(i.pos.y) - 3;
            blit_shadow_image(
                scr,
                shadow,
                &state.small_sprites,
                (t.start_frame + i.cur_frame) as usize,
                px - 3 + off_x, // :353
                py + 3 + off_y,
            );
        } else if i.cur_frame > 1 {
            // :354 — pos = Ftoi(pos) + offs; pos.x-=3; pos.y+=3.
            let px = ftoi(i.pos.x) + off_x - 3; // :355-357
            let py = ftoi(i.pos.y) + off_y + 3;
            // :358 Encloses gate BEFORE the query; :359-362 write if shadowed.
            if scr.clip.encloses(px, py) {
                let sh = shadow.shadowed_argb(px, py);
                if sh != 0 {
                    scr.put_argb(px, py, sh);
                }
            }
        }
    }

    // (5) worms + ninjarope — viewport.cpp:368-385. Iterate game.worms order;
    // only VISIBLE worms cast a shadow.
    for w in state.worms.iter() {
        if w.visible {
            // :370
            let temp_x = ftoi(w.pos.x) - 7 + off_x; // :371
            let temp_y = ftoi(w.pos.y) - 5 + off_y; // :372
            if w.ninjarope.out {
                // :373
                let nx = ftoi(w.ninjarope.pos.x) + off_x; // :374
                let ny = ftoi(w.ninjarope.pos.y) + off_y; // :375
                // :376-377 — shadow line rope tip -> worm handle.
                draw_shadow_line(scr, shadow, nx - 3, ny + 3, temp_x + 7 - 3, temp_y + 4 + 3);
                // :378-379 — the rope-hook sprite large[84] at (nx-4, ny+2) (16x16).
                blit_shadow_image(scr, shadow, &state.large_sprites, 84, nx - 4, ny + 2);
            }
            // :381-383 — the worm body shadow at (tempX-3, tempY+3) (16x16).
            let frame = worm_sprite_index(w.current_frame, w.direction, w.index);
            blit_shadow_image(scr, shadow, &state.worm_sprites, frame, temp_x - 3, temp_y + 3);
        }
    }

    // (6) bobjects (blood) — viewport.cpp:387-397.
    for i in state.bobjects.iter() {
        // :388-390 — ipos = Ftoi(pos) + offs; ipos.x-=3; ipos.y+=3.
        let px = ftoi(i.pos.x) + off_x - 3;
        let py = ftoi(i.pos.y) + off_y + 3;
        // :391 Encloses gate; :392-395 write if shadowed.
        if scr.clip.encloses(px, py) {
            let sh = shadow.shadowed_argb(px, py);
            if sh != 0 {
                scr.put_argb(px, py, sh);
            }
        }
    }
}

/// **Pass 2** — port of `viewport.cpp:400-590`, all sprites drawn on top of the
/// fully-composited shadow layer. The families run in the same fixed C++ order as
/// the shadow pass: bonuses -> sobjects -> wobjects -> nobjects -> worms(+laser
/// sight/beam/ninjarope/fire cone/body) -> aim crosshair -> bobjects. This is the
/// second half of the load-bearing two-pass ordering (Global Constraint): every
/// shadow composited before ANY sprite, so `frame::draw` runs `shadow_pass`
/// COMPLETELY before this per viewport.
///
/// `off_x`/`off_y` are `kOffs` (`screen = world + kOffs`). `vp` is `&mut` because
/// the worm sub-loop is where the **viewport-local RNG goes live**: `draw_laser_sight`
/// advances `vp.rand` per stepped Bresenham pixel (`viewport.cpp:516`). The aim
/// crosshair is gated on the **viewport's own** worm being visible (`vp.worm_idx`),
/// NOT on the loop worm.
///
/// The **name-label `DrawTextSmall` calls** (`:411`, `:476`, `:580`) and the
/// **AI debug** draw (`:550`) are deliberately omitted here — they belong to 3e
/// (font/HUD). The sobject blit is the sole sprite-pass user of the `ShadowQuery`
/// (`BlitImageR`'s water-range test, `viewport.cpp:423`); the query is built
/// internally (a pure read-only view of the level + palette), so `frame::draw`'s
/// own shadow query can be dropped before this takes `&mut vp`.
#[allow(clippy::too_many_arguments)]
pub fn sprite_pass(
    scr: &mut Bitmap,
    state: &SimState,
    pal: &Pal32,
    vp: &mut Viewport,
    off_x: i32,
    off_y: i32,
    fire_cone_sprites: &SpriteSet,
    nr_begin: i32,
    nr_end: i32,
    laser_weapon: i32,
    bonus_frames: &[i32],
) {
    // The sobject BlitImageR (:423) reads the LEVEL (water range) via this query.
    // Same `world_offset = -kOffs` convention as `frame::draw`/`shadow_pass`.
    let shadow = ShadowQuery {
        level: &state.level,
        pal32: pal,
        world_offset_x: -off_x,
        world_offset_y: -off_y,
        mode: ColorMode::Classic,
        cycles: state.cycles,
    };

    // (7) bonuses — viewport.cpp:402-415 (name label :411 skipped).
    for i in state.bonuses.iter() {
        if i.timer > BONUS_FLICKER_TIME || (state.cycles & 3) == 0 {
            let f = bonus_frames[i.frame as usize]; // :405
            // :406-407 — small sprite at (Ftoi(x)-3, Ftoi(y)-3) + offs.
            blit_image(
                scr,
                pal,
                &state.small_sprites,
                f as usize,
                ftoi(i.x) - 3 + off_x,
                ftoi(i.y) - 3 + off_y,
            );
        }
    }

    // (8) sobjects — viewport.cpp:418-425. BlitImageR gates on the water range.
    for i in state.sobjects.iter() {
        let t = &state.sobject_types[i.id as usize]; // :421
        let frame = i.cur_frame + t.start_frame; // :422
        // :423-424 — 16x16 large sprite at (x, y) + offs (x/y are int pixels).
        blit_image_r(
            scr,
            pal,
            &shadow,
            &state.large_sprites,
            frame as usize,
            i.x + off_x,
            i.y + off_y,
            16,
            16,
        );
    }

    // (9) wobjects — viewport.cpp:428-481 (name label :465-479 skipped). The
    // sprite blit is UNCONDITIONAL on `w.shadow` (unlike the shadow pass).
    for i in state.wobjects.iter() {
        let w = &state.weapons[i.ty.expect("live wobject has a weapon type") as usize]; // :431
        if w.start_frame > -1 {
            // :432
            let cur_frame = wobj_remap(i.cur_frame, w.shot_type); // :433-454
            let pos_x = ftoi(i.pos.x) - 3; // :455
            let pos_y = ftoi(i.pos.y) - 3; // :456
            // :457-458 — small[start_frame+cur_frame] at (kPosX, kPosY) + offs.
            blit_image(
                scr,
                pal,
                &state.small_sprites,
                (w.start_frame + cur_frame) as usize,
                pos_x + off_x,
                pos_y + off_y,
            );
        } else if i.cur_frame > 0 {
            // :459-462 — single palette pixel at (Ftoi(x)+offs, Ftoi(y)+offs).
            // SetPixel clips internally (Inside), matching C++ (no extra gate).
            let pos_x = ftoi(i.pos.x) + off_x;
            let pos_y = ftoi(i.pos.y) + off_y;
            scr.set_pixel(pos_x, pos_y, i.cur_frame as u8, pal);
        }
    }

    // (10) nobjects — viewport.cpp:483-498.
    for i in state.nobjects.iter() {
        let t = &state.nobject_types[i.ty.expect("live nobject has a type") as usize]; // :486
        if t.start_frame > 0 {
            // :487 — pos = Ftoi(pos)-(3,3); blit small[start_frame+cur_frame] at
            // (pos.x, pos.y) + offs.
            let px = ftoi(i.pos.x) - 3; // :488
            let py = ftoi(i.pos.y) - 3;
            blit_image(
                scr,
                pal,
                &state.small_sprites,
                (t.start_frame + i.cur_frame) as usize,
                px + off_x, // :489-490
                py + off_y,
            );
        } else if i.cur_frame > 1 {
            // :491 — pos = Ftoi(pos) + offs; Encloses gate then SetPixel.
            let px = ftoi(i.pos.x) + off_x; // :492
            let py = ftoi(i.pos.y) + off_y;
            if scr.clip.encloses(px, py) {
                // :493
                scr.set_pixel(px, py, i.cur_frame as u8, pal); // :494
            }
        }
    }

    // (11) worms — viewport.cpp:500-552. game.worms order; only VISIBLE worms.
    // The worm sub-loop order is: laser sight -> laser beam -> ninjarope ->
    // fire cone -> worm body (AI debug :550 skipped).
    for w in state.worms.iter() {
        if w.visible {
            // :503
            let temp_x = ftoi(w.pos.x) - 7 + off_x; // :504
            let temp_y = ftoi(w.pos.y) - 5 + off_y; // :505
            let af = angle_frame(w.aiming_angle, w.direction); // :506
            let cw = w.current_weapon as usize;
            let ww = w.weapons[cw];

            if ww.available() {
                // :508
                let hotspot_x = w.hotspot_x + off_x; // :509
                let hotspot_y = w.hotspot_y + off_y; // :510
                let weapon = &state.weapons[ww.ty.expect("current weapon has a type") as usize]; // :513
                if weapon.laser_sight {
                    // :515 — the viewport-RNG trap: advances vp.rand.
                    draw_laser_sight(scr, pal, &mut vp.rand, hotspot_x, hotspot_y, temp_x + 7, temp_y + 4);
                }
                // :519 — the designated laser weapon, firing, draws a beam.
                if ww.ty == Some(laser_weapon - 1) && w.control_states.get(ControlState::FIRE) {
                    draw_line(scr, pal, hotspot_x, hotspot_y, temp_x + 7, temp_y + 4, weapon.color_bullets);
                }
            }

            if w.ninjarope.out {
                // :525
                let nx = ftoi(w.ninjarope.pos.x) + off_x; // :526
                let ny = ftoi(w.ninjarope.pos.y) + off_y; // :527
                // :529 — the rope line, colour cycling [nr_begin, nr_end).
                draw_ninjarope(scr, pal, nx, ny, temp_x + 7, temp_y + 4, nr_begin, nr_end);
                // :531 — the rope-hook sprite large[84] at (nx-1, ny-1).
                blit_image(scr, pal, &state.large_sprites, 84, nx - 1, ny - 1);
            }

            // :534 — the fire cone, gated on the weapon's fire_cone AND the worm's
            // fire_cone countdown. The weapon type is *i->type of the slot.
            let weapon_fire_cone = ww
                .ty
                .map(|ty| state.weapons[ty as usize].fire_cone)
                .unwrap_or(0);
            if weapon_fire_cone > 0 && w.fire_cone > 0 {
                // :539-542.
                blit_fire_cone(
                    scr,
                    pal,
                    w.fire_cone / 2,
                    fire_cone_sprites,
                    fire_cone_sprite_index(af, w.direction),
                    FIRE_CONE_OFFSET[w.direction as usize][af as usize][0] + temp_x,
                    FIRE_CONE_OFFSET[w.direction as usize][af as usize][1] + temp_y,
                );
            }

            // :545 — the worm body sprite at (tempX, tempY).
            let frame = worm_sprite_index(w.current_frame, w.direction, w.index);
            blit_image(scr, pal, &state.worm_sprites, frame, temp_x, temp_y);
        }
    }

    // (12) aim crosshair — viewport.cpp:566-583 (change-name label :575-582
    // skipped). Gated on the VIEWPORT'S OWN worm being visible.
    let worm = &state.worms[vp.worm_idx];
    if worm.visible {
        // :566
        // :568 — temp = Ftoi(pos) - (1,2) + Ftoi(cossin[Ftoi(aiming_angle)] * 16) + offs.
        // The index can be exactly 128 (same benign OOB as `process_sight`,
        // worm.cpp:1197); the sim's 128-entry table is periodic, so mask `& 0x7f`
        // (a no-op for 0..=127, mapping the degenerate 128 -> 0). Render-only.
        let cd = state.cossin[(ftoi(worm.aiming_angle) & 0x7f) as usize].mul(16);
        let temp_x = ftoi(worm.pos.x) - 1 + ftoi(cd.x) + off_x;
        let temp_y = ftoi(worm.pos.y) - 2 + ftoi(cd.y) + off_y;
        // :572 — small[44] when the sight is green, else small[43].
        let f = if worm.make_sight_green { 44 } else { 43 };
        blit_image(scr, pal, &state.small_sprites, f, temp_x, temp_y);
    }

    // (13) bobjects (blood) — viewport.cpp:585-590. Encloses gate then SetPixel.
    for i in state.bobjects.iter() {
        let px = ftoi(i.pos.x) + off_x; // :586
        let py = ftoi(i.pos.y) + off_y;
        if scr.clip.encloses(px, py) {
            // :587
            scr.set_pixel(px, py, i.color as u8, pal); // :588
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::{Bitmap, ColorMode, Pal32, Rect};
    use crate::shadow_query::{ShadowQuery, MAT_SEE_SHADOW};
    use crate::viewport::Viewport;
    use assets::level::LevelData;
    use assets::object::{NObjectType, SObjectType, Weapon};
    use assets::sprite::SpriteSet;
    use sim::control::ControlConsts;
    use sim::physics::PhysicsConsts;
    use sim::state::{
        NObject, SObject, SimState, WObject, WeaponInit, WormWeapon, WormInit, NUM_WEAPONS,
    };
    use sim_core::fixed::itof;
    use sim_core::vec::Vec2;

    const SENTINEL: u32 = 0xDEAD_BEEF;

    // pal[i] = 0xFF000000 | i so a shadowed pixel (pal[m+4]) reveals its material.
    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    // The darkened ARGB every SeeShadow-20 cell paints: pal[20+4].
    const SHADOW_ARGB: u32 = 0xFF00_0000 | 24;

    // ---- selectors + remap (hand tables) ----

    #[test]
    fn worm_sprite_index_formula() {
        // f + dir*21 + index*42 (7*3=21, 2*7*3=42).
        assert_eq!(worm_sprite_index(0, 0, 0), 0);
        assert_eq!(worm_sprite_index(5, 0, 0), 5);
        assert_eq!(worm_sprite_index(0, 1, 0), 21);
        assert_eq!(worm_sprite_index(0, 0, 1), 42);
        assert_eq!(worm_sprite_index(3, 1, 1), 3 + 21 + 42);
    }

    #[test]
    fn fire_cone_sprite_index_formula() {
        // f + dir*7.
        assert_eq!(fire_cone_sprite_index(0, 0), 0);
        assert_eq!(fire_cone_sprite_index(6, 0), 6);
        assert_eq!(fire_cone_sprite_index(0, 1), 7);
        assert_eq!(fire_cone_sprite_index(6, 1), 13);
    }

    #[test]
    fn wobj_remap_shot_type_0_1_identity() {
        for st in [0, 1, 4, 99] {
            for cf in [-5, 0, 7, 64, 100] {
                assert_eq!(wobj_remap(cf, st), cf, "shot_type {st} leaves cur_frame {cf}");
            }
        }
    }

    #[test]
    fn wobj_remap_shot_type_2() {
        // (cf+4)>>3, then clamp: <0 -> 16, >15 -> -=16.
        assert_eq!(wobj_remap(0, 2), 0, "(0+4)>>3 = 0");
        assert_eq!(wobj_remap(4, 2), (4 + 4) >> 3); // 1
        assert_eq!(wobj_remap(60, 2), (60 + 4) >> 3); // 8
        // >15 branch: cur_frame 130 -> (134)>>3 = 16 -> >15 -> 16-16 = 0.
        assert_eq!(wobj_remap(130, 2), 0, "134>>3=16 -> -16 -> 0");
        // <0 branch: cur_frame -40 -> (-36)>>3 = -5 (arith shift) -> <0 -> 16.
        assert_eq!(wobj_remap(-40, 2), 16, "negative -> clamps to 16");
    }

    #[test]
    fn wobj_remap_shot_type_3() {
        // cf>64 -> --; then (cf-12)>>3; clamp <0 -> 0, >12 -> 12.
        // cf=12: (0)>>3 = 0.
        assert_eq!(wobj_remap(12, 3), 0);
        // cf=0: (-12)>>3 = -2 (arith) -> <0 -> 0.
        assert_eq!(wobj_remap(0, 3), 0, "below floor clamps to 0");
        // cf=60: (48)>>3 = 6.
        assert_eq!(wobj_remap(60, 3), 6);
        // cf=200: >64 -> 199; (199-12)>>3 = 187>>3 = 23 -> >12 -> 12.
        assert_eq!(wobj_remap(200, 3), 12, "high clamps to 12, with the >64 -1");
        // cf=65: >64 -> 64; (64-12)>>3 = 52>>3 = 6.
        assert_eq!(wobj_remap(65, 3), 6, "the >64 decrement applies");
    }

    // ---- shadow_pass scaffolding ----

    // A 64x64 level entirely SeeShadow material 20 -> every shadow query returns
    // SHADOW_ARGB; a sprite stencil thus reveals exactly WHERE the shadow lands.
    fn all_shadow_level(w: i32, h: i32) -> LevelData {
        let material_id = vec![20u8; (w * h) as usize];
        LevelData { width: w, height: h, material_id, palette: None, display: None }
    }

    fn see_shadow_flags() -> [u8; 256] {
        let mut f = [0u8; 256];
        f[20] = MAT_SEE_SHADOW;
        f
    }

    fn solid_bank(sz: i32, count: i32) -> SpriteSet {
        SpriteSet { width: sz, height: sz, count, data: vec![1u8; (count * sz * sz) as usize] }
    }

    fn worm_init(idx: i32, px: i32, py: i32, visible: bool) -> WormInit {
        WormInit {
            index: idx,
            health: 100,
            lives: 3,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::new(px << 16, py << 16),
            visible,
        }
    }

    // Build a SimState over the all-shadow level with the given worms, then attach
    // solid sprite banks big enough for every frame index the pass uses.
    fn base_state(worms: &[WormInit]) -> SimState {
        let level = all_shadow_level(64, 64);
        let flags = see_shadow_flags();
        let mut state = SimState::new(
            &level,
            worms,
            0,
            &flags,
            vec![],
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            vec![],
            vec![],
            vec![],
            0,
            false,
            0,
        );
        state.small_sprites = solid_bank(7, 200);
        state.large_sprites = solid_bank(16, 128); // covers frame 84 (rope hook)
        state.worm_sprites = solid_bank(16, 84);
        state
    }

    fn shadow_query<'a>(state: &'a SimState, pal: &'a Pal32) -> ShadowQuery<'a> {
        ShadowQuery {
            level: &state.level,
            pal32: pal,
            world_offset_x: 0, // off_x = 0 in tests, so world_offset = -0
            world_offset_y: 0,
            mode: ColorMode::Classic,
            cycles: 0,
        }
    }

    fn filled(w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for p in b.pixels.iter_mut() {
            *p = SENTINEL;
        }
        b
    }

    fn px(b: &Bitmap, x: i32, y: i32) -> u32 {
        b.pixels[(y * b.pitch + x) as usize]
    }

    // ---- the empty-pool / invisibility guard (3a ripple) ----

    #[test]
    fn shadow_pass_empty_scene_writes_nothing() {
        // Two INVISIBLE worms, all pools empty -> not a single pixel written.
        let pal = ramp_pal();
        let state = base_state(&[worm_init(0, 30, 30, false), worm_init(1, 40, 40, false)]);
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        assert!(b.pixels.iter().all(|&p| p == SENTINEL), "empty scene draws nothing");
    }

    // ---- (2) sobjects: shadow at (x-3, y+3), 16x16 ----

    #[test]
    fn shadow_pass_sobject_offset() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.sobject_types = vec![SObjectType { start_frame: 0, ..Default::default() }];
        state.sobjects.spawn(SObject { id: 0, x: 20, y: 20, cur_frame: 0, anim_delay: 0 });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // Shadow block screen [17,33) x [23,39): top-left corner at (17,23).
        assert_eq!(px(&b, 17, 23), SHADOW_ARGB, "sobject shadow top-left = (x-3, y+3)");
        assert_eq!(px(&b, 32, 38), SHADOW_ARGB, "sobject shadow bottom-right corner");
        assert_eq!(px(&b, 16, 23), SENTINEL, "one left of block -> unshadowed");
        assert_eq!(px(&b, 17, 22), SENTINEL, "one above block -> unshadowed (proves +3 y)");
        assert_eq!(px(&b, 20, 20), SENTINEL, "sprite origin is NOT shadowed (offset applied)");
    }

    // ---- (3) wobjects: shadow-flag branch + else-pixel branch ----

    #[test]
    fn shadow_pass_wobject_shadow_flag_on() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        // Weapon 0: shadow=true, start_frame=0, shot_type=0 (identity remap).
        state.weapons = vec![Weapon { shadow: true, start_frame: 0, shot_type: 0, ..Default::default() }];
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // kPosX = 30-3 = 27; shadow at kPosX-3 = 24, kPosY+3 = 30. 7x7 block [24,31)x[30,37).
        assert_eq!(px(&b, 24, 30), SHADOW_ARGB, "wobject shadow top-left = (Ftoi-6, Ftoi)");
        assert_eq!(px(&b, 30, 36), SHADOW_ARGB, "wobject shadow bottom-right (7x7)");
        assert_eq!(px(&b, 23, 30), SENTINEL, "one left of block");
        assert_eq!(px(&b, 24, 29), SENTINEL, "one above block");
    }

    #[test]
    fn shadow_pass_wobject_shadow_flag_off_writes_nothing() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        // shadow=false -> the start_frame>-1 branch computes but never blits.
        state.weapons = vec![Weapon { shadow: false, start_frame: 0, shot_type: 0, ..Default::default() }];
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        assert!(b.pixels.iter().all(|&p| p == SENTINEL), "shadow=false weapon casts no shadow");
    }

    #[test]
    fn shadow_pass_wobject_else_pixel() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        // start_frame = -1 -> else branch: single pixel iff cur_frame > 0.
        state.weapons = vec![Weapon { start_frame: -1, ..Default::default() }];
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 5, // > 0
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // pixel at (Ftoi(x)-3, Ftoi(y)+3) = (27, 33).
        assert_eq!(px(&b, 27, 33), SHADOW_ARGB, "wobject else-branch single shadowed pixel");
        // Exactly one pixel written.
        let n = b.pixels.iter().filter(|&&p| p != SENTINEL).count();
        assert_eq!(n, 1, "else-branch draws exactly one pixel");
    }

    #[test]
    fn shadow_pass_wobject_else_pixel_gated_on_cur_frame() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.weapons = vec![Weapon { start_frame: -1, ..Default::default() }];
        // cur_frame 0 -> the `> 0` gate fails -> nothing.
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        assert!(b.pixels.iter().all(|&p| p == SENTINEL), "cur_frame 0 -> no else pixel");
    }

    // ---- (4) nobjects: blit branch (start_frame>0) + else-pixel (cur_frame>1) ----

    #[test]
    fn shadow_pass_nobject_blit_branch() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.nobject_types = vec![NObjectType { start_frame: 1, ..Default::default() }];
        state.nobjects.spawn(NObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // pos = (30-3) = 27; blit at (27-3, 27+3) = (24, 30), 7x7 block [24,31)x[30,37).
        assert_eq!(px(&b, 24, 30), SHADOW_ARGB, "nobject blit shadow top-left");
        assert_eq!(px(&b, 30, 36), SHADOW_ARGB, "nobject blit shadow bottom-right (7x7)");
        assert_eq!(px(&b, 23, 30), SENTINEL, "one left of block");
    }

    #[test]
    fn shadow_pass_nobject_else_pixel() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        // start_frame = 0 (NOT > 0) -> else branch, needs cur_frame > 1.
        state.nobject_types = vec![NObjectType { start_frame: 0, ..Default::default() }];
        state.nobjects.spawn(NObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 2, // > 1
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // pixel at (30-3, 30+3) = (27, 33).
        assert_eq!(px(&b, 27, 33), SHADOW_ARGB, "nobject else-branch single pixel");
        let n = b.pixels.iter().filter(|&&p| p != SENTINEL).count();
        assert_eq!(n, 1, "else-branch draws exactly one pixel");
        // cur_frame == 1 (not > 1) draws nothing.
        let mut state2 = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state2.nobject_types = vec![NObjectType { start_frame: 0, ..Default::default() }];
        state2.nobjects.spawn(NObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 1,
            ..Default::default()
        });
        let q2 = shadow_query(&state2, &pal);
        let mut b2 = filled(64, 64);
        shadow_pass(&mut b2, &state2, &q2, 0, 0, &[]);
        assert!(b2.pixels.iter().all(|&p| p == SENTINEL), "cur_frame 1 -> no else pixel");
    }

    // ---- (5) worms + ninjarope ----

    #[test]
    fn shadow_pass_visible_worm_body_shadow() {
        let pal = ramp_pal();
        // One visible worm at (30,30); the other invisible.
        let mut state = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        // current_frame/direction/index all 0 -> worm_sprites[0]; body 16x16.
        state.worms[0].current_frame = 0;
        state.worms[0].direction = 0;
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // tempX = 30-7 = 23, tempY = 30-5 = 25; body shadow at (tempX-3, tempY+3) =
        // (20, 28), 16x16 block [20,36)x[28,44).
        assert_eq!(px(&b, 20, 28), SHADOW_ARGB, "worm body shadow top-left = (Ftoi-10, Ftoi-2)");
        assert_eq!(px(&b, 35, 43), SHADOW_ARGB, "worm body shadow bottom-right (16x16)");
        assert_eq!(px(&b, 19, 28), SENTINEL, "one left of block");
    }

    #[test]
    fn shadow_pass_worm_ninjarope_line_and_hook() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 40, 40, true), worm_init(1, 0, 0, false)]);
        state.worms[0].ninjarope.out = true;
        state.worms[0].ninjarope.pos = Vec2::new(itof(10), itof(10));
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // Rope hook large[84] at (nx-4, ny+2) = (10-4, 10+2) = (6, 12), 16x16
        // block [6,22)x[12,28).
        assert_eq!(px(&b, 6, 12), SHADOW_ARGB, "rope-hook shadow top-left = (nx-4, ny+2)");
        assert_eq!(px(&b, 21, 27), SHADOW_ARGB, "rope-hook shadow bottom-right (16x16)");
        // The shadow line runs from (nx-3, ny+3)=(7,13) to (tempX+4, tempY+7) with
        // tempX=40-7=33, tempY=40-5=35 -> (37, 42). A mid-line pixel is shadowed.
        // Just assert the worm body shadow is also present (both worm draws ran).
        // Body at (tempX-3, tempY+3) = (30, 38).
        assert_eq!(px(&b, 30, 38), SHADOW_ARGB, "worm body shadow present alongside rope");
    }

    // ---- (6) bobjects (blood) ----

    #[test]
    fn shadow_pass_bobject_pixel() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.bobjects.spawn(sim::state::BObject {
            pos: Vec2::new(itof(30), itof(30)),
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        // pixel at (Ftoi(x)-3, Ftoi(y)+3) = (27, 33).
        assert_eq!(px(&b, 27, 33), SHADOW_ARGB, "bobject shadowed pixel at (x-3, y+3)");
        let n = b.pixels.iter().filter(|&&p| p != SENTINEL).count();
        assert_eq!(n, 1, "one blood particle -> one shadowed pixel");
    }

    // ---- ordering: a family drawn later overlays the same cell identically ----

    #[test]
    fn shadow_pass_overlap_is_order_independent_within_pass() {
        // A bobject (family 6) and an nobject-else (family 4) both target the SAME
        // cell (27,33). Because every shadow reads the LEVEL (idempotent), the cell
        // holds SHADOW_ARGB regardless of which family painted last — this is the
        // load-bearing invariant that lets the sprite pass (T5) run entirely after.
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.nobject_types = vec![NObjectType { start_frame: 0, ..Default::default() }];
        state.nobjects.spawn(NObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 2,
            ..Default::default()
        });
        state.bobjects.spawn(sim::state::BObject {
            pos: Vec2::new(itof(30), itof(30)),
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut b = filled(64, 64);
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        assert_eq!(px(&b, 27, 33), SHADOW_ARGB, "overlapping shadows resolve to the same value");
    }

    // ===================================================================
    // sprite_pass (pass 2) — viewport.cpp:400-590
    // ===================================================================

    const SPRITE: u32 = 0xFF00_0001; // solid_bank writes index 1 -> pal[1].

    fn vp_at(worm_idx: usize) -> Viewport {
        Viewport::new(Rect::new(0, 0, 64, 64), worm_idx)
    }

    // Give worm `wi`'s current slot a real weapon type so the `Available()` block
    // can dereference `*i->type` without hitting the None-slot invariant.
    fn arm_worm(state: &mut SimState, wi: usize, w: Weapon) {
        state.weapons = vec![w];
        state.worms[wi].weapons[0] =
            WormWeapon { ty: Some(0), ammo: 0, delay_left: 0, loading_left: 0 };
        state.worms[wi].current_weapon = 0;
    }

    // ---- (11) worm body: sprite at (tempX, tempY), worm_sprite_index ----

    #[test]
    fn sprite_pass_worm_body_at_temp() {
        // Two worms: worm[0] visible (draws its body); worm[1] invisible AND the
        // viewport's own worm, so the crosshair (visible-gated on vp.worm_idx=1)
        // is skipped — isolating the body blit.
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        arm_worm(&mut state, 0, Weapon::default()); // no laser/firecone -> body only
        state.worms[0].current_frame = 0;
        state.worms[0].direction = 0;
        let mut vp = vp_at(1);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        // tempX = 30-7 = 23, tempY = 30-5 = 25; body 16x16 block [23,39)x[25,41).
        assert_eq!(px(&b, 23, 25), SPRITE, "worm body top-left = (Ftoi-7, Ftoi-5)");
        assert_eq!(px(&b, 38, 40), SPRITE, "worm body bottom-right (16x16)");
        assert_eq!(px(&b, 22, 25), SENTINEL, "one left of body");
        assert_eq!(px(&b, 23, 24), SENTINEL, "one above body");
    }

    // ---- (12) crosshair: gated on the VIEWPORT'S OWN worm visibility ----

    #[test]
    fn sprite_pass_crosshair_visible_gated() {
        let pal = ramp_pal();
        // Viewport worm is index 0. When visible -> crosshair small[43] draws;
        // arm it so the Available() block does not panic, but with a plain weapon.
        let mut state = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        arm_worm(&mut state, 0, Weapon::default());
        state.worms[0].aiming_angle = 0; // cossin[0]
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        // temp = Ftoi(pos) - (1,2) + Ftoi(cossin[0] * 16). Compute the exact cell.
        let cd = state.cossin[0].mul(16);
        let tx = 30 - 1 + ftoi(cd.x);
        let ty = 30 - 2 + ftoi(cd.y);
        assert_eq!(px(&b, tx, ty), SPRITE, "crosshair small[43] top-left at temp");

        // Same scene but the viewport worm is INVISIBLE -> no crosshair at temp.
        let mut state2 = base_state(&[worm_init(0, 30, 30, false), worm_init(1, 0, 0, false)]);
        arm_worm(&mut state2, 0, Weapon::default());
        let mut vp2 = vp_at(0);
        let mut b2 = filled(64, 64);
        sprite_pass(&mut b2, &state2, &pal, &mut vp2, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert!(b2.pixels.iter().all(|&p| p == SENTINEL), "invisible viewport worm -> no crosshair");
    }

    #[test]
    fn sprite_pass_crosshair_make_sight_green_picks_44() {
        // make_sight_green flips the crosshair sprite index 43 -> 44. With a
        // per-frame-distinct bank we can prove the frame selection.
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        arm_worm(&mut state, 0, Weapon::default());
        state.worms[0].make_sight_green = true;
        // Bank whose frame 44 top-left byte is a UNIQUE index (7), frame 43 is 5.
        let mut data = vec![0u8; 200 * 7 * 7];
        data[43 * 49] = 5;
        data[44 * 49] = 7;
        state.small_sprites = SpriteSet { width: 7, height: 7, count: 200, data };
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        let cd = state.cossin[0].mul(16);
        let tx = 30 - 1 + ftoi(cd.x);
        let ty = 30 - 2 + ftoi(cd.y);
        assert_eq!(px(&b, tx, ty), 0xFF00_0000 | 7, "green sight -> small[44] (byte 7)");
    }

    // ---- viewport-RNG: laser sight advances vp.rand, non-laser leaves it ----

    #[test]
    fn sprite_pass_laser_sight_advances_vp_rand() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        let laser = Weapon { laser_sight: true, ..Default::default() };
        arm_worm(&mut state, 0, laser);
        state.worms[0].hotspot_x = 10;
        state.worms[0].hotspot_y = 10;
        // Viewport worm 1 (invisible) so no crosshair; worm 0 draws the sight.
        let mut vp = vp_at(1);
        let before = vp.rand.draws();
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert!(vp.rand.draws() > before, "laser_sight walked the viewport RNG live");

        // Non-laser weapon: the sight is never walked -> vp.rand untouched.
        let mut state2 = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        arm_worm(&mut state2, 0, Weapon::default());
        state2.worms[0].hotspot_x = 10;
        state2.worms[0].hotspot_y = 10;
        let mut vp2 = vp_at(1);
        let before2 = vp2.rand.draws();
        let mut b2 = filled(64, 64);
        sprite_pass(&mut b2, &state2, &pal, &mut vp2, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert_eq!(vp2.rand.draws(), before2, "no laser_sight -> no RNG draw");
    }

    // ---- laser beam: designated LaserWeapon slot + Fire pressed -> DrawLine ----

    #[test]
    fn sprite_pass_laser_beam_gated_on_weapon_index_and_fire() {
        use sim::state::ControlState;
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        // color_bullets = 9 so the beam paints pal[9]; ty slot 0 == laser_weapon-1
        // requires laser_weapon = 1.
        arm_worm(&mut state, 0, Weapon { color_bullets: 9, ..Default::default() });
        state.worms[0].hotspot_x = 10;
        state.worms[0].hotspot_y = 10;
        state.worms[0].control_states.set(ControlState::FIRE, true);
        let mut vp = vp_at(1);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 1, &[]);
        // The beam line from (10,10) to (tempX+7, tempY+4) = (30, 29) paints pal[9]
        // on its stepped pixels; a mid pixel just past the start must be pal[9].
        let painted = b.pixels.iter().any(|&p| p == 0xFF00_0009);
        assert!(painted, "laser beam DrawLine painted color_bullets when Fire held");

        // Fire NOT held -> no beam.
        let mut state2 = base_state(&[worm_init(0, 30, 30, true), worm_init(1, 0, 0, false)]);
        arm_worm(&mut state2, 0, Weapon { color_bullets: 9, ..Default::default() });
        state2.worms[0].hotspot_x = 10;
        state2.worms[0].hotspot_y = 10;
        let mut vp2 = vp_at(1);
        let mut b2 = filled(64, 64);
        sprite_pass(&mut b2, &state2, &pal, &mut vp2, 0, 0, &SpriteSet::default(), 0, 0, 1, &[]);
        assert!(!b2.pixels.iter().any(|&p| p == 0xFF00_0009), "no Fire -> no beam");
    }

    // ---- (13) bobjects (blood): SetPixel(color) at Encloses cells ----

    #[test]
    fn sprite_pass_bobject_setpixel_color() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.bobjects.spawn(sim::state::BObject {
            pos: Vec2::new(itof(30), itof(30)),
            color: 17,
            ..Default::default()
        });
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        // Blood pixel at Ftoi(pos)+offs = (30,30), palette index = color 17.
        assert_eq!(px(&b, 30, 30), 0xFF00_0000 | 17, "blood SetPixel(color) at Ftoi(pos)");
        let n = b.pixels.iter().filter(|&&p| p != SENTINEL).count();
        assert_eq!(n, 1, "one blood particle -> exactly one pixel");
    }

    // ---- (9) wobject sprite branch (unconditional on w.shadow) + else pixel ----

    #[test]
    fn sprite_pass_wobject_sprite_and_else_pixel() {
        let pal = ramp_pal();
        // start_frame >= 0 -> blit small[start_frame+cur_frame] (shadow flag IGNORED
        // in the sprite pass). shot_type 0 -> identity remap.
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.weapons = vec![Weapon { start_frame: 0, shot_type: 0, shadow: false, ..Default::default() }];
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        // kPosX = 30-3 = 27; small 7x7 block [27,34)x[27,34).
        assert_eq!(px(&b, 27, 27), SPRITE, "wobject sprite top-left = (Ftoi-3, Ftoi-3)");
        assert_eq!(px(&b, 33, 33), SPRITE, "wobject sprite bottom-right (7x7)");

        // else branch: start_frame = -1, cur_frame > 0 -> SetPixel(cur_frame) at
        // (Ftoi(pos)+offs).
        let mut state2 = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state2.weapons = vec![Weapon { start_frame: -1, ..Default::default() }];
        state2.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 6,
            ..Default::default()
        });
        let mut vp2 = vp_at(0);
        let mut b2 = filled(64, 64);
        sprite_pass(&mut b2, &state2, &pal, &mut vp2, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert_eq!(px(&b2, 30, 30), 0xFF00_0000 | 6, "wobject else SetPixel(cur_frame) at Ftoi(pos)");
    }

    // ---- (10) nobject sprite branch + else pixel (Encloses-gated) ----

    #[test]
    fn sprite_pass_nobject_sprite_and_else_pixel() {
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.nobject_types = vec![NObjectType { start_frame: 1, ..Default::default() }];
        state.nobjects.spawn(NObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        // pos = Ftoi-(3,3) = (27,27); small 7x7 block [27,34)x[27,34).
        assert_eq!(px(&b, 27, 27), SPRITE, "nobject sprite top-left = (Ftoi-3, Ftoi-3)");
        assert_eq!(px(&b, 33, 33), SPRITE, "nobject sprite bottom-right (7x7)");

        // else branch (start_frame = 0): SetPixel iff cur_frame > 1 at Ftoi(pos).
        let mut state2 = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state2.nobject_types = vec![NObjectType { start_frame: 0, ..Default::default() }];
        state2.nobjects.spawn(NObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 3,
            ..Default::default()
        });
        let mut vp2 = vp_at(0);
        let mut b2 = filled(64, 64);
        sprite_pass(&mut b2, &state2, &pal, &mut vp2, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert_eq!(px(&b2, 30, 30), 0xFF00_0000 | 3, "nobject else SetPixel(cur_frame) at Ftoi(pos)");
    }

    // ---- (8) sobjects: BlitImageR gates on the water range [160,168) ----

    #[test]
    fn sprite_pass_sobject_blit_image_r_water_gated() {
        let pal = ramp_pal();
        // A 64x64 level with a water cell (material 160) at world (20,20) and rock
        // (material 0) elsewhere. BlitImageR only paints over the water cell.
        let mut material_id = vec![0u8; 64 * 64];
        material_id[(20 + 20 * 64) as usize] = 160; // in [160,168)
        let level = LevelData { width: 64, height: 64, material_id, palette: None, display: None };
        let flags = [0u8; 256];
        let mut state = SimState::new(
            &level, &[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)], 0, &flags, vec![],
            PhysicsConsts::default(), ControlConsts::default(), false, SpriteSet::default(),
            vec![], vec![], vec![], 0, false, 0,
        );
        state.large_sprites = solid_bank(16, 16); // frame 0 solid
        state.sobject_types = vec![SObjectType { start_frame: 0, ..Default::default() }];
        // sobject at pixel (13,13): 16x16 sprite covers [13,29)x[13,29), so the water
        // cell (20,20) is inside the sprite footprint.
        state.sobjects.spawn(SObject { id: 0, x: 13, y: 13, cur_frame: 0, anim_delay: 0 });
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        // Only the water cell (20,20) is painted (pal[1]); a dry cell is untouched.
        assert_eq!(px(&b, 20, 20), SPRITE, "sobject BlitImageR paints over water cell");
        assert_eq!(px(&b, 14, 14), SENTINEL, "dry cell inside sprite -> not painted");
    }

    // ---- empty scene guard: no visible worms, empty pools -> nothing ----

    #[test]
    fn sprite_pass_empty_scene_writes_nothing() {
        let pal = ramp_pal();
        let state = base_state(&[worm_init(0, 30, 30, false), worm_init(1, 40, 40, false)]);
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert!(b.pixels.iter().all(|&p| p == SENTINEL), "empty sprite scene draws nothing");
    }

    // ---- the LOAD-BEARING two-pass ordering: shadow pass COMPLETE, then sprites ----

    #[test]
    fn two_pass_shadow_then_sprite_ordering() {
        // A single shadow-casting wobject: its shadow (pass 1) lands at (Ftoi-6,
        // Ftoi) and its sprite (pass 2) at (Ftoi-3, Ftoi-3); the two footprints
        // overlap. Running shadow_pass COMPLETELY before sprite_pass (exactly what
        // frame::draw does per viewport) means the sprite survives at the overlap,
        // while a shadow-only cell keeps its pass-1 darkening. A single reordered
        // draw would flip either assertion.
        let pal = ramp_pal();
        let mut state = base_state(&[worm_init(0, 0, 0, false), worm_init(1, 0, 0, false)]);
        state.weapons =
            vec![Weapon { shadow: true, start_frame: 0, shot_type: 0, ..Default::default() }];
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(30), itof(30)),
            ty: Some(0),
            cur_frame: 0,
            ..Default::default()
        });
        let q = shadow_query(&state, &pal);
        let mut vp = vp_at(0);
        let mut b = filled(64, 64);
        // shadow 7x7 at (24,30) -> [24,31)x[30,37); sprite 7x7 at (27,27) ->
        // [27,34)x[27,34). Overlap region [27,31)x[30,34).
        shadow_pass(&mut b, &state, &q, 0, 0, &[]);
        sprite_pass(&mut b, &state, &pal, &mut vp, 0, 0, &SpriteSet::default(), 0, 0, 0, &[]);
        assert_eq!(px(&b, 27, 30), SPRITE, "overlap: sprite (pass 2) survives over shadow (pass 1)");
        assert_eq!(px(&b, 24, 30), SHADOW_ARGB, "shadow-only cell keeps pass-1 darkening");
        assert_eq!(px(&b, 33, 27), SPRITE, "sprite-only cell shows the sprite");
    }
}
