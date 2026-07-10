//! The 3a draw path: the terrain-only subset of `Game::Draw`
//! (`game.cpp:170-198`) + the viewport world block (`viewport.cpp:196-210`).
//! Build the per-frame palette, Fill(0) the whole surface, then for each
//! viewport center it, clip to its rect, and DrawLevel at kOffs. Shadows,
//! sprites, HUD, minimap are 3b/3e.

use crate::bitmap::{Bitmap, ColorMode};
use crate::level_draw::draw_level;
use crate::palette::build_palette;
use crate::viewport::Viewport;
use assets::palette::Palette;
use assets::tc::ColorAnim;
use sim::state::SimState;

pub fn draw(
    bmp: &mut Bitmap,
    state: &SimState,
    origpal: &Palette,
    color_anim: &[ColorAnim],
    viewports: &mut [Viewport],
    screen_flash: i32,
) {
    // 1. Per-frame palette, before any blit (game.cpp:171-183).
    let pal = build_palette(origpal, color_anim, state.cycles, screen_flash);
    // 2. Repaint the whole surface through the fresh LUT (game.cpp:189).
    bmp.fill(0, &pal);
    // 3. Per viewport: center, clip, DrawLevel at kOffs (viewport.cpp:196-210).
    let full_clip = bmp.clip;
    for vp in viewports.iter_mut() {
        let worm = &state.worms[vp.worm_idx];
        vp.process(worm, state.level.width, state.level.height);
        bmp.clip = vp.rect;
        bmp.cycles = state.cycles;
        // kOffs = rect.Ul() - (vp.x, vp.y): screen = world + kOffs.
        let (ulx, uly) = vp.rect.ul();
        draw_level(bmp, &state.level, &pal, ulx - vp.x, uly - vp.y, ColorMode::Classic);
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
        for w in state.worms.iter_mut() {
            w.killed_timer = 0;
        }

        let origpal = ramp_origpal();
        let mut vps = Viewport::player_layout();
        let mut bmp = Bitmap::new(320, 200);
        draw(&mut bmp, &state, &origpal, &[], &mut vps, 0);

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
