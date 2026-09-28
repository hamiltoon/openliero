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
use crate::font::Font;
use crate::hud::{self, HudLabels};
use crate::level_draw::draw_level;
use crate::object_draw::{shadow_pass, sprite_pass_with, SmallLabels};
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
    /// The 250-glyph HUD font used by `draw_hud`'s text pass (kills/lives/
    /// reloading). Threaded even when `draw_hud` is false (a non-optional ref
    /// keeps the field cheap); it is only read when `draw_hud` is true.
    pub font: &'a Font,
    /// The three HUD label prefixes (`Kills: `/`Lives: `/`Reloading...`).
    pub labels: &'a HudLabels,
    /// The decoupled HUD directive (mirrors 3a's `render` / 3b's `render_shadow`).
    /// When false the world-only draw path is byte-identical to 3a/3b — the HUD
    /// pre-block and minimap are skipped, so the existing frame goldens hold.
    pub draw_hud: bool,
    /// `Settings::map` — the minimap toggle. The minimap is drawn only when
    /// `draw_hud && map` (`viewport.cpp:593`, gated inside the HUD path here).
    pub map: bool,
    /// Step 4½e-1: the three `DrawTextSmall` name labels (bonus, booby trap, Change held).
    /// `None` on every golden path — the sprite pass is then byte-identical to its pre-4½e-1
    /// self; `Some` draws them at their C++ points (`object_draw::sprite_pass_with`).
    pub small_labels: Option<SmallLabels<'a>>,
}

pub fn draw(bmp: &mut Bitmap, state: &SimState, viewports: &mut [Viewport], scene: &Scene) {
    // 1. Per-frame palette, before any blit (game.cpp:171-183). `screen_flash > 0`
    //    makes `LightUp` live via `build_palette` (game.cpp:170-198).
    let pal = build_palette(scene.origpal, scene.color_anim, state.cycles, scene.screen_flash);
    // 2. Repaint the whole surface through the fresh LUT (game.cpp:189).
    bmp.fill(0, &pal);
    // 3. Per viewport: center (shake goes live automatically via vp.shake), clip,
    //    then terrain -> ALL shadows -> ALL sprites (viewport.cpp:196-591).
    // HUD render-map (§7): `render_res_y` is the surface height, `multiplier =
    // render_res_x / 320` (viewport.cpp:81), `center_x = render_res_x / 2`
    // (viewport.cpp:82). Derived from the surface, constant across viewports.
    let render_res_y = bmp.h;
    let multiplier = bmp.w / 320;
    let center_x = bmp.w / 2;
    let full_clip = bmp.clip;
    // Phase 5 (`ProcessViewports`, game.cpp:463): center + shake-RNG + banner
    // reset for ALL viewports BEFORE any is drawn. C++ runs `ProcessViewports`
    // fully (over every viewport) before `Game::Draw` renders them, so a
    // cross-viewport death banner reads the OTHER viewport's POST-process
    // `banner_y` — critically its `killed_timer==150` reset to -8 on the death
    // tick. Splitting this out of the draw loop is hash-neutral: each viewport's
    // OWN local RNG still draws shake (here) then laser sights (in its own
    // `sprite_pass`) in the same relative order, and `process` never touches the
    // shared surface. (An interleaved snapshot would read the pre-reset `banner_y`
    // and mis-draw the banner on the death tick.)
    for vp in viewports.iter_mut() {
        let worm = &state.worms[vp.worm_idx];
        vp.process(worm, state.level.width, state.level.height);
    }
    // Cross-viewport banner inputs (viewport.cpp:256-270), captured POST-process
    // (matches C++, above). `(worm_idx, banner_y)` per viewport; the draw loop
    // reads the OTHERs from here without a second mutable borrow.
    let banner_state: Vec<(usize, i32)> =
        viewports.iter().map(|v| (v.worm_idx, v.banner_y)).collect();
    for (self_idx, vp) in viewports.iter_mut().enumerate() {
        // C++ `Viewport::Draw` (viewport.cpp:78-635) does, PER VIEWPORT, in one
        // call: HUD pre-block (full clip, :84-189) -> world block (rect clip,
        // :196-591) -> minimap (full clip, :593-635). The Rust loop mirrors that
        // ordering around the existing world block. The HUD pre-block draws into
        // the full-surface clip (still set here, BEFORE `bmp.clip = vp.rect`).
        if scene.draw_hud {
            hud::draw_hud(
                bmp,
                &pal,
                state,
                vp.worm_idx,
                scene.font,
                scene.labels,
                render_res_y,
                multiplier,
            );
        }
        bmp.clip = vp.rect;
        bmp.cycles = state.cycles;
        // kOffs = rect.Ul() - (vp.x, vp.y): screen = world + kOffs.
        let (ulx, uly) = vp.rect.ul();
        let off_x = ulx - vp.x;
        let off_y = uly - vp.y;
        draw_level(bmp, &state.level, &pal, off_x, off_y, ColorMode::Classic);
        // Cross-viewport death banners (viewport.cpp:256-270): for every OTHER
        // viewport whose worm is dead and whose banner has walked into view
        // (banner_y > -8), draw the kill/suicide message in THIS viewport's
        // column at the OTHER's banner_y. Shadow (colour 0) at (+3,+1) then text
        // (colour 50) at (+2,+0), size 1 — the C++ DrawString default. Keyed by
        // `last_killed_by_idx`. The own-worm YoureIt/GameOfTag arm
        // (viewport.cpp:249-254) is DEFERRED (needs got_changed + game-mode,
        // spec §7). The worm-name suffix/prefix (`other_worm.settings->name`) is
        // empty in every render scenario (the dumper leaves WormSettings::name
        // default ""), so only the label draws; the C++ concatenation ORDER is
        // preserved (prefix for a kill, suffix for a suicide) for when worm names
        // are threaded later.
        let this_index = state.worms[vp.worm_idx].index;
        for (other_idx, &(other_worm_idx, other_banner_y)) in banner_state.iter().enumerate() {
            if other_idx == self_idx {
                continue; // v != this
            }
            let other_worm = &state.worms[other_worm_idx];
            if other_worm.health <= 0 && other_banner_y > -8 {
                let msg: &str = if other_worm.last_killed_by_idx == this_index {
                    &scene.labels.killed_msg
                } else {
                    &scene.labels.committed_suicide_msg
                };
                scene
                    .font
                    .draw_string(bmp, &pal, msg, vp.rect.x1 + 3, other_banner_y + 1, 0, 1);
                scene
                    .font
                    .draw_string(bmp, &pal, msg, vp.rect.x1 + 2, other_banner_y, 50, 1);
            }
        }
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
        // Pass 2: all sprites (advances vp.rand for laser sights), with the optional
        // name labels.
        sprite_pass_with(
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
            scene.small_labels.as_ref(),
        );
        bmp.clip = full_clip;
        // Minimap (viewport.cpp:593-635) — AFTER the world block, into the
        // restored full-surface clip, gated on `settings->map`. It lives INSIDE
        // the viewport loop because C++ draws it per viewport at the SAME centred
        // position (`kCenterX`), so viewport 1's pass overwrites viewport 0's
        // (spec §7 Q6). Reproduce that double-draw exactly.
        if scene.draw_hud && scene.map {
            hud::draw_minimap(bmp, &pal, state, center_x, render_res_y);
        }
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
        let font = Font::default();
        let labels = HudLabels::default();
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
            font: &font,
            labels: &labels,
            draw_hud: false,
            map: false,
            small_labels: None,
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

    // A 250-glyph synthetic font: each glyph paints a single "on" pixel (index 8)
    // at its cell (0,0) origin and advances 5px — so a drawn string leaves a
    // pal[color] pixel at its first glyph origin (mirrors hud.rs `origin_font`).
    fn origin_font() -> Font {
        use crate::font::Char;
        let mut chars = vec![Char::default(); Font::NUM_CHARS];
        for ch in chars.iter_mut() {
            ch.data[0] = 8;
            ch.width = 5;
        }
        Font { chars }
    }

    fn hud_labels() -> HudLabels {
        HudLabels {
            kills: "Kills: ".to_string(),
            lives: "Lives: ".to_string(),
            reloading: "Reloading...".to_string(),
            ..Default::default()
        }
    }

    // Build a 2-worm KillEmAll state whose worms are VISIBLE (so the HUD life bar
    // + kills/lives + minimap dots all paint). worm 0 at world (79,79) stats_x 0,
    // worm 1 at (100,100) stats_x 218; both health 100, current weapon slot 0.
    fn hud_state() -> SimState {
        let width = 320;
        let height = 200;
        let level = LevelData {
            width,
            height,
            material_id: vec![0u8; (width * height) as usize],
            palette: None,
            display: None,
        };
        let mut w0 = worm_init(0, 79, 79);
        let mut w1 = worm_init(1, 100, 100);
        w0.stats_x = 0;
        w1.stats_x = 218;
        let worms = [w0, w1];
        let flags = [0u8; 256];
        let mut state = SimState::new(
            &level,
            &worms,
            0,
            &flags,
            vec![assets::object::Weapon::default()],
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
        for w in state.worms.iter_mut() {
            w.killed_timer = 0;
            w.current_weapon = 0;
            w.weapons[0] =
                sim::state::WormWeapon { ty: Some(0), ammo: 0, delay_left: 0, loading_left: 0 };
        }
        state
    }

    // draw_hud=false must leave the frame byte-identical to the world-only path
    // (anti-bleed guard); draw_hud=true + map=true must paint the HUD region (the
    // life bar) AND the minimap region (a worm dot). Both regions are below the
    // viewports (y >= 158), so world-only leaves them at fill(pal32[0]).
    #[test]
    fn draw_hud_flag_gates_hud_and_minimap() {
        let origpal = ramp_origpal();
        let empty_bank = SpriteSet::default();
        let font = origin_font();
        let labels = hud_labels();

        let scene_off = Scene {
            origpal: &origpal,
            color_anim: &[],
            fire_cone_sprites: &empty_bank,
            bonus_frames: &[],
            nr_begin: 0,
            nr_end: 0,
            laser_weapon: 0,
            screen_flash: 0,
            draw_shadow: false,
            font: &font,
            labels: &labels,
            draw_hud: false,
            map: false,
            small_labels: None,
        };
        let scene_on = Scene {
            draw_hud: true,
            map: true,
            ..scene_off
        };

        // World-only: the HUD + minimap regions stay fill(pal32[0]).
        let state = hud_state();
        let mut vps_off = Viewport::player_layout();
        let mut bmp_off = Bitmap::new(320, 200);
        draw(&mut bmp_off, &state, &mut vps_off, &scene_off);
        let px_off = |x: i32, y: i32| bmp_off.pixels[(x + y * 320) as usize];
        // life bar row (worm 0, stats_x 0, y = 200-39 = 161) is untouched.
        assert_eq!(px_off(0, 161), 0xFF00_0000, "world-only: no life bar");
        // worm 0 minimap dot cell (145,175) is untouched.
        assert_eq!(px_off(145, 175), 0xFF00_0000, "world-only: no minimap dot");

        // HUD on: the life bar paints (health 100 -> width 100, colour
        // 100/10+234 = 244 -> pal32[244] = 0xFFF4_0000) and worm 0's minimap dot
        // paints (index 0 -> colour 129 -> pal32[129] = 0xFF81_0000). step_x =
        // ceil(320/52) = 7, step_y = ceil(200/36) = 6; map_x = 160-26 = 134,
        // map_y = 200-38 = 162; dot at (79/7+134, 79/6+162) = (145,175).
        let mut vps_on = Viewport::player_layout();
        let mut bmp_on = Bitmap::new(320, 200);
        draw(&mut bmp_on, &state, &mut vps_on, &scene_on);
        let px_on = |x: i32, y: i32| bmp_on.pixels[(x + y * 320) as usize];
        assert_eq!(px_on(0, 161), 0xFFF4_0000, "HUD on: life bar pal32[244]");
        assert_eq!(
            px_on(145, 175),
            0xFF81_0000,
            "HUD on: minimap dot pal32[129]"
        );

        // The world region above the HUD is IDENTICAL with draw_hud on/off — the
        // HUD/minimap never bleed into the world-only rows (y < 158).
        for y in 0..158 {
            for x in 0..320 {
                let i = (x + y * 320) as usize;
                assert_eq!(
                    bmp_off.pixels[i], bmp_on.pixels[i],
                    "world row unchanged by HUD at ({x},{y})"
                );
            }
        }
    }
}
