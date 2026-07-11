//! The full world draw: the world subset of `Game::Draw` (`game.cpp:170-198`) +
//! the two-pass `Viewport::Draw` world block (`viewport.cpp:196-591`). Build the
//! per-frame palette (with `screen_flash` -> `LightUp` live), Fill(0) the whole
//! surface, then per viewport: center, clip, `draw_level` at kOffs, the COMPLETE
//! `shadow_pass` (if `draw_shadow`), then the `sprite_pass`. HUD/banners/minimap/
//! font stay in 3e.
//!
//! **The two-pass ordering is load-bearing** (Global Constraint): all shadows of
//! a viewport composite before ANY of its sprites. `shadow_pass` runs COMPLETELY
//! before `sprite_pass`, per viewport, inside the viewport loop — mirroring the
//! C++ `Viewport::Draw` structure (everything happens per viewport in its `Draw`).

use crate::bitmap::{Bitmap, ColorMode};
use crate::level_draw::draw_level;
use crate::object_draw::{shadow_pass, sprite_pass};
use crate::palette::build_palette;
use crate::shadow_query::ShadowQuery;
use crate::viewport::Viewport;
use assets::palette::Palette;
use assets::sprite::SpriteSet;
use assets::tc::ColorAnim;
use sim::state::SimState;

/// The per-frame render inputs that are not the surface, the sim state, or the
/// viewports — bundled so the wide `draw` arg list stays readable. `origpal` +
/// `color_anim` + `screen_flash` feed the palette build (`game.cpp:170-198`, with
/// `screen_flash > 0` making `LightUp` live); `fire_cone_sprites`/`bonus_frames`/
/// `nr_begin`/`nr_end`/`laser_weapon` thread into the sprite pass; `draw_shadow`
/// gates the shadow pass (the decoupled draw-time shadow directive).
pub struct Scene<'a> {
    pub origpal: &'a Palette,
    pub color_anim: &'a [ColorAnim],
    pub fire_cone_sprites: &'a SpriteSet,
    pub bonus_frames: &'a [i32],
    pub nr_begin: i32,
    pub nr_end: i32,
    pub laser_weapon: i32,
    pub screen_flash: i32,
    pub draw_shadow: bool,
}

pub fn draw(bmp: &mut Bitmap, state: &SimState, viewports: &mut [Viewport], scene: &Scene) {
    // 1. Per-frame palette, before any blit (game.cpp:171-183). `screen_flash > 0`
    //    makes `LightUp` live via `build_palette` (game.cpp:170-198).
    let pal = build_palette(scene.origpal, scene.color_anim, state.cycles, scene.screen_flash);
    // 2. Repaint the whole surface through the fresh LUT (game.cpp:189).
    bmp.fill(0, &pal);
    // 3. Per viewport: center (shake goes live automatically via vp.shake), clip,
    //    then terrain -> ALL shadows -> ALL sprites (viewport.cpp:196-591).
    let full_clip = bmp.clip;
    for vp in viewports.iter_mut() {
        let worm = &state.worms[vp.worm_idx];
        vp.process(worm, state.level.width, state.level.height);
        bmp.clip = vp.rect;
        bmp.cycles = state.cycles;
        // kOffs = rect.Ul() - (vp.x, vp.y): screen = world + kOffs.
        let (ulx, uly) = vp.rect.ul();
        let off_x = ulx - vp.x;
        let off_y = uly - vp.y;
        draw_level(bmp, &state.level, &pal, off_x, off_y, ColorMode::Classic);
        // Pass 1: all shadows (world_offset = -kOffs). Scoped so its immutable
        // borrows of `pal`/`state.level` drop before `sprite_pass` takes `&mut vp`.
        if scene.draw_shadow {
            let shadow = ShadowQuery {
                level: &state.level,
                pal32: &pal,
                world_offset_x: -off_x,
                world_offset_y: -off_y,
                mode: ColorMode::Classic,
                cycles: state.cycles,
            };
            shadow_pass(bmp, state, &shadow, off_x, off_y, scene.bonus_frames);
        }
        // Pass 2: all sprites (advances vp.rand for laser sights).
        sprite_pass(
            bmp,
            state,
            &pal,
            vp,
            off_x,
            off_y,
            scene.fire_cone_sprites,
            scene.nr_begin,
            scene.nr_end,
            scene.laser_weapon,
            scene.bonus_frames,
        );
        bmp.clip = full_clip;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::Bitmap;
    use assets::level::LevelData;
    use assets::palette::{Color, Palette};
    use sim::control::ControlConsts;
    use sim::physics::PhysicsConsts;
    use sim::state::{SimState, WeaponInit, WormInit, NUM_WEAPONS};
    use sim_core::vec::Vec2;

    // origpal: entry i -> (r=i, g=0, b=0), so pal32[i] = 0xFF | (i<<16); distinct
    // per index and pal32[0] = 0xFF000000 (the HUD-gap background witness).
    fn ramp_origpal() -> Palette {
        let mut p = Palette { entries: [Color::default(); 256] };
        for (i, e) in p.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        p
    }

    fn worm_init(idx: i32, px: i32, py: i32) -> WormInit {
        WormInit {
            index: idx,
            health: 100,
            lives: 3,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::new(px << 16, py << 16),
            visible: true,
        }
    }

    #[test]
    fn draw_fills_background_then_terrain_per_viewport() {
        // Level 320x200 (>= a viewport so centering is non-degenerate). Set a
        // distinctive material 42 at world (5,5); everything else is material 0.
        let width = 320;
        let height = 200;
        let mut material_id = vec![0u8; (width * height) as usize];
        material_id[(5 + 5 * width) as usize] = 42;
        let level = LevelData {
            width,
            height,
            material_id,
            palette: None,
            display: None,
        };
        // Two worms centered at (79,79): SetCenter -> (0,0), clamp keeps (0,0).
        let worms = [worm_init(0, 79, 79), worm_init(1, 79, 79)];
        let flags = [0u8; 256];
        let mut state = SimState::new(
            &level,
            &worms,
            0,
            &flags,
            vec![],
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            Default::default(),
            vec![],
            vec![],
            vec![],
            0,
            false,
            0,
        );
        // Exercise the visible-centering arm (from_init leaves killed_timer=150).
        // Arm slot 0 with a real weapon type so the (now visible) sprite pass can
        // dereference `*i->type` without hitting the None-slot invariant; the
        // sprite banks stay empty (default), so every worm/crosshair blit no-ops
        // and this stays a pure terrain test.
        state.weapons = vec![assets::object::Weapon::default()];
        for w in state.worms.iter_mut() {
            w.killed_timer = 0;
            w.weapons[0] =
                sim::state::WormWeapon { ty: Some(0), ammo: 0, delay_left: 0, loading_left: 0 };
            w.current_weapon = 0;
        }

        let origpal = ramp_origpal();
        let mut vps = Viewport::player_layout();
        let mut bmp = Bitmap::new(320, 200);
        // 3a parity: no shadows, empty fire-cone bank, no flash. With invisible
        // worms + empty pools the sprite pass paints nothing, so the terrain-only
        // frame is reproduced byte-for-byte.
        let empty_bank = SpriteSet::default();
        let scene = Scene {
            origpal: &origpal,
            color_anim: &[],
            fire_cone_sprites: &empty_bank,
            bonus_frames: &[],
            nr_begin: 0,
            nr_end: 0,
            laser_weapon: 0,
            screen_flash: 0,
            draw_shadow: false,
        };
        draw(&mut bmp, &state, &mut vps, &scene);

        let px = |x: i32, y: i32| bmp.pixels[(x + y * 320) as usize];
        // Viewport 0 draws world (0,0) at screen (0,0); world (5,5) -> screen (5,5).
        assert_eq!(px(5, 5), 0xFF2A_0000, "viewport 0 terrain material 42");
        // Viewport 1 rect ul (160,0); world (5,5) -> screen (165,5).
        assert_eq!(px(165, 5), 0xFF2A_0000, "viewport 1 terrain material 42");
        // Background material 0 shows through elsewhere inside a viewport.
        assert_eq!(px(0, 0), 0xFF00_0000, "viewport 0 background material 0");
        // The HUD gap (x in [158,160)) is never drawn -> stays fill(pal32[0]).
        assert_eq!(px(159, 0), 0xFF00_0000, "HUD gap is pal32[0]");
        // Rows below the viewports (y>=158) stay fill(pal32[0]).
        assert_eq!(px(10, 199), 0xFF00_0000, "bottom rows are pal32[0]");
    }
}
