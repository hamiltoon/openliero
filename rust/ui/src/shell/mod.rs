//! Step 4½d — the C++ shell: `StateStack` (`state.hpp`), `MainMenuState`
//! (`mainMenuState.cpp`), the router and `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`). T3 adds the
//! two concrete menus; T6 moves the 4½c live modules here; T7 adds the stack, the screens and
//! `Shell`. T6 moved the 4½c live modules here from `game` (`new_game`, `selection`,
//! `match_flow`, `viewport_step`, `loadout`), unchanged; `game` re-exports them.
pub mod loadout;
pub mod main_menu;
pub mod match_flow;
pub mod new_game;
pub mod selection;
pub mod settings_menu;
pub mod viewport_step;

use render::frame::Scene;

/// The two HUD switches of a `render::frame::Scene` (moved from `game::hud_mode`, Step 4½a-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudFlags {
    pub draw_hud: bool,
    pub map: bool,
}

impl HudFlags {
    /// Set `scene.draw_hud` and `scene.map`.
    pub fn apply(self, scene: &mut Scene<'_>) {
        scene.draw_hud = self.draw_hud;
        scene.map = self.map;
    }
}
