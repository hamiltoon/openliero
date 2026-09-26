//! Step 4½a-2 — which HUD the `game` binary draws (design §8, overview LD 9b: the live HUD
//! bug). `scenario::SceneData::as_scene` returns a world-only `Scene` (`draw_hud = map =
//! false`, kept for the 3a/3b goldens) and the binary never flipped it, so `cargo run -p game`
//! showed no stats panel and no minimap. C++ always draws the HUD during play and gates the
//! minimap on `settings->map` (`viewport.cpp:593`). A played match (`Live`) and a replay of
//! one (`Replay`) therefore draw `(true, settings.map)`. The `Scripted` demo keeps its
//! scenario's `render_hud` directive for both flags, so the `blood` demo — and the wasm
//! frame-parity witness built on it — stays world-only. Until 4½e loads the setup (design
//! §9.3.6) the binary passes `Settings::default().map`.

use crate::input::Mode;
pub use ui::shell::HudFlags;

/// The HUD for `mode`: `Live`/`Replay` ⇒ drawn, minimap per `settings_map`; `Scripted` ⇒ the
/// scenario's `render_hud` directive (`scenario_hud`) for both.
pub fn hud_flags(mode: Mode, scenario_hud: bool, settings_map: bool) -> HudFlags {
    match mode {
        Mode::Live | Mode::Replay => HudFlags {
            draw_hud: true,
            map: settings_map,
        },
        Mode::Scripted => HudFlags {
            draw_hud: scenario_hud,
            map: scenario_hud,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use render::bitmap::Bitmap;
    use render::viewport::Viewport;
    use scenario::build::build_match;
    use scenario::paths::TC_ROOT;
    use scenario::settings::{MatchConfig, Settings};

    use super::*;
    use crate::new_game::generate_level;

    #[test]
    fn a_played_match_draws_the_hud_and_follows_the_map_setting() {
        for mode in [Mode::Live, Mode::Replay] {
            assert_eq!(
                hud_flags(mode, false, true),
                HudFlags {
                    draw_hud: true,
                    map: true
                }
            );
            assert_eq!(
                hud_flags(mode, false, false),
                HudFlags {
                    draw_hud: true,
                    map: false
                }
            );
            assert_eq!(
                hud_flags(mode, true, false),
                HudFlags {
                    draw_hud: true,
                    map: false
                },
                "a scenario directive does not override the setting"
            );
        }
        assert!(
            Settings::default().map,
            "the C++ default the binary passes until 4½d"
        );
    }

    #[test]
    fn the_scripted_demo_keeps_its_scenario_directive() {
        assert_eq!(
            hud_flags(Mode::Scripted, false, true),
            HudFlags {
                draw_hud: false,
                map: false
            }
        );
        assert_eq!(
            hud_flags(Mode::Scripted, true, false),
            HudFlags {
                draw_hud: true,
                map: true
            }
        );
    }

    #[test]
    fn the_flags_reach_the_frame_of_the_live_default_match() {
        // Step 4½c: the live default match is the NEW GAME start (`ui::shell`), here in play
        // (selection skipped: the saved picks, lives, the pool).
        let tc = Path::new(TC_ROOT);
        let settings = Settings::default();
        let shadow = settings.shadow;
        let level = generate_level(tc, &settings, None, 42);
        let loaded = build_match(tc, &MatchConfig { settings, seed: 42 }, &level)
            .expect("the default settings build");
        let frame = |flags: HudFlags| -> Vec<u32> {
            let mut scene = loaded.scene.as_scene(0, shadow);
            flags.apply(&mut scene);
            let mut viewports = Viewport::player_layout();
            let mut bmp = Bitmap::new(320, 200);
            render::frame::draw(&mut bmp, &loaded.state, &mut viewports, &scene);
            bmp.pixels
        };
        let world_only = frame(hud_flags(Mode::Scripted, false, true));
        let hud = frame(hud_flags(Mode::Live, false, false));
        let hud_map = frame(hud_flags(Mode::Live, false, true));
        // Rows 158.. hold the stats panel and the minimap (render `frame.rs` HUD test).
        const HUD_START: usize = 158 * 320;
        assert_ne!(
            world_only[HUD_START..],
            hud[HUD_START..],
            "Live paints the stats panel"
        );
        assert_ne!(
            hud[HUD_START..],
            hud_map[HUD_START..],
            "settings.map adds the minimap"
        );
        assert_eq!(
            world_only[..HUD_START],
            hud_map[..HUD_START],
            "the world rows are untouched"
        );
    }
}
