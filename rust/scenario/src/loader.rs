//! The tick-0 scenario loader — factored **verbatim** out of the T8 harness
//! `render_slice3b_common::build()` (Step 3, Slice 3c, T0).
//!
//! [`load`] is a byte-for-byte re-home of that `build()` body: the same reads in
//! the same order, the same `resolve_weapons`, the same `weapon 0 <name>`
//! override applied to BOTH worms, the same `SimState::new` call, the same
//! post-`new` TC-scalar assignments, and the same fixed-camera invariant
//! (`killed_timer` left at the `WormInit` default of 150). The only intended
//! change is that the TC root is now the `tc_root: &Path` argument instead of a
//! module-level `TC_ROOT` const. The committed goldens are the regression proof:
//! any drift here flips a `render_slice3b_*` frame hash.

use std::path::Path;

use assets::object::Objects;
use assets::palette::Palette;
use assets::sprite::SpriteSet;
use assets::tc::{ColorAnim, TcConfig};
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::state::{SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

use render::fire_cone::build_fire_cone_sprites;
use render::font::Font;
use render::frame::Scene;
use render::viewport::Viewport;

// The HUD labels type is owned by `render` (`render::hud::HudLabels`). Slice 3e T5
// dropped the identical `scenario`-local duplicate and re-exports the render type
// here, so `frame::Scene.labels: &HudLabels` is filled without a conversion and
// `scenario::HudLabels` keeps naming the same struct.
pub use render::hud::HudLabels;

use crate::parser::Scenario;

/// Owned Scene ingredients — everything the per-tick `render::frame::draw` needs
/// besides the surface, the SimState, and the viewports. Mirrors the `Built`
/// fields the T8 harness carried inline.
pub struct SceneData {
    pub origpal: Palette,
    pub color_anim: Vec<ColorAnim>,
    pub fire_cone: SpriteSet,
    pub nr_begin: i32,
    pub nr_end: i32,
    pub laser_weapon: i32,
    /// The 250-glyph HUD font (`sprites/font.tga` post-processed by `Font::load`).
    /// Owned here (Slice 3e T5) so `as_scene` can thread `&Font` into the widened
    /// `frame::Scene` without a second load.
    pub font: Font,
    /// The three HUD text labels from the TC's `[texts]` (`Kills`/`Lives`/`Reloading`).
    pub labels: HudLabels,
}

impl SceneData {
    /// Borrow the owned ingredients into a `render::frame::Scene` for one draw.
    /// `screen_flash`/`draw_shadow` are per-draw (demo passes 0 / scenario.shadow()).
    /// `draw_hud`/`map` default to `false` — the world-only path every existing
    /// caller (shot, game, the 3b harness) drives, so 3a/3b frame hashes stay
    /// byte-identical. A HUD-enabling caller (3e T8) sets them on the returned
    /// `Scene`.
    pub fn as_scene(&self, screen_flash: i32, draw_shadow: bool) -> Scene<'_> {
        Scene {
            origpal: &self.origpal,
            color_anim: &self.color_anim,
            fire_cone_sprites: &self.fire_cone,
            bonus_frames: &[],
            nr_begin: self.nr_begin,
            nr_end: self.nr_end,
            laser_weapon: self.laser_weapon,
            screen_flash,
            draw_shadow,
            font: &self.font,
            labels: &self.labels,
            draw_hud: false,
            map: false,
        }
    }
}

/// The full tick-0 load: driven `SimState`, the two fixed-camera viewports
/// (fresh default-seeded RNG), and the owned Scene ingredients (which now carry
/// the HUD font + labels, Slice 3e T5). `as_scene` threads those into the widened
/// `frame::Scene`.
pub struct Loaded {
    pub state: SimState,
    pub viewports: [Viewport; 2],
    pub scene: SceneData,
}

fn load_sprites(tc_root: &Path, file: &str, w: i32, h: i32, count: i32) -> SpriteSet {
    let bytes = crate::assets::read_asset(tc_root, &format!("sprites/{file}"));
    let tga = assets::sprite::Tga::load(&bytes).unwrap_or_else(|_| panic!("{file} parses"));
    SpriteSet::from_tga(&tga, w, h, count).unwrap_or_else(|_| panic!("{file} sprite bank"))
}

/// Verbatim factor-out of `render_slice3b_common::build()`. `tc_root` is the TC
/// directory (`data/TC/openliero`); `scenario` is the already-parsed scenario.
pub fn load(tc_root: &Path, scenario: &Scenario) -> Loaded {
    // Origpal = small.tga's embedded palette (C++ common.exepal), as in 3a.
    let small_bytes = crate::assets::read_asset(tc_root, "sprites/small.tga");
    let small_tga = assets::sprite::Tga::load(&small_bytes).expect("small.tga parses");
    let origpal = small_tga.palette.clone();

    let lev_bytes = crate::assets::read_asset(tc_root, &scenario.level);
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = crate::assets::read_asset(tc_root, "tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let color_anim = tc.color_anim.clone();
    let objects = Objects::load(&tc.types, |sub, id| {
        Ok(crate::assets::read_asset(
            tc_root,
            &format!("{sub}/{id}.cfg"),
        ))
    })
    .expect("object configs load");

    // weap_order: indices sorted by weapon name; id == index (Common::Precompute).
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // Override slot 0 with the scenario's `weapon 0 <name>` (FAN/DART/RIFLE),
    // applied to BOTH worms — mirrors the C++ dumper's ResolveWeapon (5b-style).
    let weapon_name = scenario
        .weapon(0)
        .expect("3b scenario has a `weapon 0 <name>` directive");
    let weapon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == weapon_name)
        .unwrap_or_else(|| panic!("weapon {weapon_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(weapon_idx as WeaponId),
        ammo: objects.weapons[weapon_idx].ammo,
    };

    let worms_init: Vec<WormInit> = scenario
        .worms
        .iter()
        .map(|w| WormInit {
            index: w.index,
            health: w.health,
            lives: w.lives,
            stats_x: w.stats_x,
            weapons: resolved,
            start_pos: Vec2::new(w.pos_x, w.pos_y),
            visible: w.visible,
        })
        .collect();

    let mut state = SimState::new(
        &level,
        &worms_init,
        scenario.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_sprites(tc_root, "large.tga", 16, 16, 110),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        0,
        true,
        100,
    );
    // TC scalars defaulted to 0 by `new` (as in 3a/5b) — the blood/spawn consts.
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_sprites(tc_root, "small.tga", 7, 7, 130);
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state.game_mode = scenario.game_mode as u32;

    // NOTE: killed_timer is left at its `WormInit` default (150) — the camera
    // stays pinned at (0,0). Resetting it would centre the viewport and diverge.

    let fire_cone = build_fire_cone_sprites(&state.large_sprites);

    // HUD font: `sprites/font.tga` is a plain uncompressed indexed TGA, so the
    // generic `Tga::load` parses it (7 × 250*8, de-flipped); `Font::load` runs the
    // `common.cpp:414-433` per-glyph post-process. No new TGA parser (T0).
    let font_bytes = crate::assets::read_asset(tc_root, "sprites/font.tga");
    let font_tga = assets::sprite::Tga::load(&font_bytes).expect("font.tga parses");
    let font = Font::load(&font_tga);
    // HUD labels carried verbatim from the TC's `[texts]` (already parsed by `assets::tc`).
    let labels = HudLabels {
        kills: tc.texts.Kills.clone(),
        lives: tc.texts.Lives.clone(),
        reloading: tc.texts.Reloading.clone(),
    };

    Loaded {
        state,
        viewports: Viewport::player_layout(),
        scene: SceneData {
            origpal,
            color_anim,
            fire_cone,
            nr_begin: tc.constants.NRColourBegin,
            nr_end: tc.constants.NRColourEnd,
            laser_weapon: tc.constants.LaserWeapon,
            font,
            labels,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Scenario;

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

    // A minimal real-TC scenario (real level + real weapon) so `load` succeeds;
    // the font + labels it yields do not depend on the scenario specifics.
    const SAMPLE: &str = "\
seed 42
level Levels/render_stage.lev
ticks 1
worm 0 6553600 7602176 100 10 0   1
worm 1 3276800 7602176 100 10 218 1
weapon 0 DART
";

    #[test]
    fn load_yields_font_and_labels() {
        let scenario = Scenario::parse(SAMPLE).expect("scenario parses");
        let loaded = load(Path::new(TC_ROOT), &scenario);
        // Non-empty 250-glyph font (now owned by `scene`, Slice 3e T5).
        assert_eq!(loaded.scene.font.chars.len(), 250);
        assert!(
            loaded.scene.font.chars.iter().any(|c| c.width > 0),
            "the real font has glyphs with nonzero advance"
        );
        // Labels carried verbatim from the TC's `[texts]` (data/TC/openliero/tc.cfg).
        assert_eq!(loaded.scene.labels.kills, "Kills: ");
        assert_eq!(loaded.scene.labels.lives, "Lives: ");
        assert_eq!(loaded.scene.labels.reloading, "Reloading...");
    }
}
