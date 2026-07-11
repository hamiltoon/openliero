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

use crate::bitmap::Bitmap;
use crate::blit::{blit_shadow_image, draw_shadow_line};
use crate::shadow_query::ShadowQuery;
use sim::state::SimState;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::{Bitmap, ColorMode, Pal32};
    use crate::shadow_query::{ShadowQuery, MAT_SEE_SHADOW};
    use assets::level::LevelData;
    use assets::object::{NObjectType, SObjectType, Weapon};
    use assets::sprite::SpriteSet;
    use sim::control::ControlConsts;
    use sim::physics::PhysicsConsts;
    use sim::state::{NObject, SObject, SimState, WObject, WeaponInit, WormInit, NUM_WEAPONS};
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
}
