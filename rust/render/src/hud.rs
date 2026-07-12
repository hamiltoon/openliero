//! The HUD bar/text pre-block, per viewport-worm. Port of `viewport.cpp:84-189`
//! (the block drawn into the full-surface clip BEFORE the per-viewport world
//! clip is set), math verbatim, idiomatic in API.
//!
//! Reachable elements (KillEmAll / 2 worms, spec §1): the life bar (visible +
//! dying arms), the ammo bar (available-ammo + reloading loading-bar) with the
//! blinking "Reloading" text, the always-drawn kills text, and the KillEmAll/
//! Scales lives text. Every colour constant (`w/10+234`, `w/10+245`, 50, 10, 6)
//! and every y offset is load-bearing (a stray pixel is a frame-hash miss);
//! each carries its `viewport.cpp` line. The Holdazone/GameOfTag/replay arms are
//! **tripwired** (`debug_assert!`) — unreachable in our scenarios, never silently
//! dropped (spec §6).
//!
//! The HUD is a **pure consumer** of `SimState`: it reads `worm.health/kills/
//! lives/killed_timer/visible/stats_x/weapons/current_weapon`, `state.cycles`,
//! `state.settings_health`, `state.game_mode`, `state.settings_loading_time`, and
//! the current weapon's `Weapon::{ammo, loading_time}` — all existing, all
//! hash-silent for HUD purposes (no new sim field).

use crate::bitmap::{Bitmap, Pal32};
use crate::blit::draw_bar;
use crate::font::Font;
use sim::state::SimState;

/// The three HUD text labels, carried verbatim from the TC's `[texts]`
/// (`Kills`/`Lives`/`Reloading`). Defined HERE (in `render`) rather than reused
/// from `scenario::loader` because **`render` must not depend on `scenario`**
/// (the crate-dependency direction is `scenario -> render`). `scenario` keeps its
/// own identical `HudLabels`; T5 threads a `&HudLabels` of THIS type into
/// `frame::Scene`, and the loader converts its labels into this struct there.
/// (The alternative — three `&str` params — was rejected as noisier at the call
/// site; a single struct matches the plan's `labels: &HudLabels` signature.)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HudLabels {
    /// `Texts::Kills` — the "Kills: " prefix (`viewport.cpp:131-132`).
    pub kills: String,
    /// `Texts::Lives` — the "Lives: " prefix (`viewport.cpp:148-153`).
    pub lives: String,
    /// `Texts::Reloading` — the blinking "Reloading..." label (`viewport.cpp:125-128`).
    pub reloading: String,
}

/// Port of `Weapon::ComputedLoadingTime` (`weapon.cpp:8-14`):
/// `ret = (settings.loading_time * loading_time) / 100`, floored to `1`. `i32`
/// truncating division, matching C++. Threaded as the two scalars the HUD reads
/// (`state.settings_loading_time`, `weapon.loading_time`) instead of a `Settings`
/// ref, so `render` stays free of the sim's settings type.
fn computed_loading_time(settings_loading_time: i32, weapon_loading_time: i32) -> i32 {
    // weapon.cpp:9
    let ret = (settings_loading_time * weapon_loading_time) / 100;
    // weapon.cpp:10-12
    if ret == 0 {
        1
    } else {
        ret
    }
}

/// Draw the HUD pre-block for `worm_idx`'s worm (`viewport.cpp:84-189`, the
/// KillEmAll/Scales arms). Drawn into the full-surface clip (the caller sets it
/// before calling; the bars are unclipped, the text clips to `scr.clip`), at
/// `worm.stats_x * multiplier`. `render_res_y` is the surface height (200 for the
/// framehash layout); `multiplier = render_res_x / 320` (1 there).
#[allow(clippy::too_many_arguments)]
pub fn draw_hud(
    scr: &mut Bitmap,
    pal: &Pal32,
    state: &SimState,
    worm_idx: usize,
    font: &Font,
    labels: &HudLabels,
    render_res_y: i32,
    multiplier: i32,
) {
    let worm = &state.worms[worm_idx];
    // viewport.cpp:86 — every element positions at `worm.stats_x * kMultiplier`.
    let stats_x = worm.stats_x * multiplier;

    // --- Life bar (viewport.cpp:84-95) ---
    if worm.visible {
        // viewport.cpp:85 — kLifebarWidth = worm.health * 100 / worm.settings->health.
        // `worm.settings->health` is the per-worm max health; the framehash layout
        // carries it as the single `state.settings_health` scalar (WormSettings
        // default 100, never overridden by the dumper).
        let lifebar_width = worm.health * 100 / state.settings_health;
        // viewport.cpp:86-87 — the 4-arg DrawBar overload (blit.cpp:101-102) => height 2;
        // colour kLifebarWidth/10 + 234.
        draw_bar(
            scr,
            pal,
            stats_x,
            render_res_y - 39,
            lifebar_width,
            2,
            lifebar_width / 10 + 234,
        );
    } else {
        // viewport.cpp:89 — dying countdown; parens bind `(killed_timer*25)/37` before
        // the subtraction (i32 truncating division, matching C++).
        let mut lifebar_width = 100 - (worm.killed_timer * 25) / 37;
        if lifebar_width > 0 {
            // viewport.cpp:91 — std::min(lifebar_width, 100).
            lifebar_width = lifebar_width.min(100);
            // viewport.cpp:92-93 — same 4-arg DrawBar overload (height 2), colour +234.
            draw_bar(
                scr,
                pal,
                stats_x,
                render_res_y - 39,
                lifebar_width,
                2,
                lifebar_width / 10 + 234,
            );
        }
    }

    // --- Ammo / loading bar + reloading text (viewport.cpp:99-129) ---
    // viewport.cpp:99 — the current weapon slot.
    let ww = &worm.weapons[worm.current_weapon as usize];
    if ww.available() {
        // viewport.cpp:102
        if ww.ammo > 0 {
            // viewport.cpp:103 — kAmmoBarWidth = ww.ammo * 100 / ww.type->ammo.
            let type_ammo = state.weapons[ww.ty.expect("current weapon has a type") as usize].ammo;
            let ammo_bar_width = ww.ammo * 100 / type_ammo;
            if ammo_bar_width > 0 {
                // viewport.cpp:106-107 — 4-arg DrawBar (height 2), colour +245.
                draw_bar(
                    scr,
                    pal,
                    stats_x,
                    render_res_y - 34,
                    ammo_bar_width,
                    2,
                    ammo_bar_width / 10 + 245,
                );
            }
        }
    } else {
        // viewport.cpp:113 — ww.type->loading_time.
        let loading_time =
            state.weapons[ww.ty.expect("current weapon has a type") as usize].loading_time;
        let ammo_bar_width = if loading_time != 0 {
            // viewport.cpp:114-115 — 100 - ww.loading_left * 100 / ComputedLoadingTime.
            let computed = computed_loading_time(state.settings_loading_time, loading_time);
            100 - ww.loading_left * 100 / computed
        } else {
            // viewport.cpp:117 — 100 - ww.loading_left * 100.
            100 - ww.loading_left * 100
        };
        if ammo_bar_width > 0 {
            // viewport.cpp:121-122 — 4-arg DrawBar (height 2), colour +245.
            draw_bar(
                scr,
                pal,
                stats_x,
                render_res_y - 34,
                ammo_bar_width,
                2,
                ammo_bar_width / 10 + 245,
            );
        }
        // viewport.cpp:125-128 — blinking "Reloading" text, gated `(cycles%20)>10 &&
        // visible`; drawn at y = 164*kMultiplier (NOT render_res_y-relative — a C++
        // quirk, ported verbatim), colour 50, size 1 (DrawString default, font.hpp:46-47).
        if (state.cycles % 20) > 10 && worm.visible {
            font.draw_string(
                scr,
                pal,
                &labels.reloading,
                stats_x,
                164 * multiplier,
                50,
                1,
            );
        }
    }

    // --- Kills text — ALWAYS (viewport.cpp:131-132) ---
    // LS(Kills) + ToString(worm.kills); i32::to_string == C++ ToString(int) (decimal),
    // colour 10, at render_res_y-29.
    let kills_text = format!("{}{}", labels.kills, worm.kills);
    font.draw_string(scr, pal, &kills_text, stats_x, render_res_y - 29, 10, 1);

    // --- Replay HUD (viewport.cpp:134-144) — DEFERRED (spec §6, is_replay=false) ---
    // The in-game player view is is_replay==false, so the worm-name / colour-box /
    // match-time block is never reached; a future `render_replay_hud` directive would
    // lift it. Tripwired here so it is not silently forgotten.
    debug_assert!(
        true,
        "replay HUD (viewport.cpp:134-144) deferred: in-game view is is_replay==false (spec §6)"
    );

    // --- Lives text / game-mode switch (viewport.cpp:146-189) ---
    // game_mode enum (settings.hpp:51): 0 kGmKillEmAll, 1 kGmGameOfTag,
    // 2 kGmHoldazone, 3 kGmScalesOfJustice.
    match state.game_mode {
        // viewport.cpp:149-153 — kGmKillEmAll | kGmScalesOfJustice: lives text,
        // LS(Lives) + ToString(worm.lives), colour 6, at render_res_y-22.
        0 | 3 => {
            let lives_text = format!("{}{}", labels.lives, worm.lives);
            font.draw_string(scr, pal, &lives_text, stats_x, render_res_y - 22, 6, 1);
        }
        // viewport.cpp:155-169 — kGmHoldazone timer text: TRIPWIRE (Holdazone deferred
        // past Step 2; no scenario sets game_mode 2). Never silently dropped.
        2 => debug_assert!(
            false,
            "Holdazone HUD timer (viewport.cpp:155-169) deferred past Step 2 (spec §6)"
        ),
        // viewport.cpp:171-185 — kGmGameOfTag timer text: TRIPWIRE (no render scenario
        // sets game_mode 1).
        1 => debug_assert!(
            false,
            "GameOfTag HUD timer (viewport.cpp:171-185) deferred (spec §6)"
        ),
        // viewport.cpp:187 — default: break.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::Bitmap;
    use crate::font::{Char, Font};
    use assets::level::LevelData;
    use assets::object::Weapon;
    use sim::control::ControlConsts;
    use sim::physics::PhysicsConsts;
    use sim::state::{SimState, WeaponInit, WormInit, WormWeapon, NUM_WEAPONS};
    use sim_core::vec::Vec2;

    const SENTINEL: u32 = 0xDEAD_BEEF;

    // pal[i] = 0xFF000000 | i (distinct per index so a pixel reveals its index).
    fn ramp_pal() -> Pal32 {
        let mut p = [0u32; 256];
        for (i, e) in p.iter_mut().enumerate() {
            *e = 0xFF00_0000 | i as u32;
        }
        p
    }

    fn filled(w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for p in b.pixels.iter_mut() {
            *p = SENTINEL;
        }
        b
    }

    // A 250-glyph font where every glyph paints a single "on" pixel at its cell
    // (0,0) origin and advances 5px — so a drawn string leaves a pal[color] pixel
    // at each glyph origin (the running x), which the text asserts probe.
    fn origin_font() -> Font {
        let mut chars = vec![Char::default(); Font::NUM_CHARS];
        for ch in chars.iter_mut() {
            ch.data[0] = 8; // on-pixel at cell (0,0)
            ch.width = 5;
        }
        Font { chars }
    }

    fn labels() -> HudLabels {
        HudLabels {
            kills: "Kills: ".to_string(),
            lives: "Lives: ".to_string(),
            reloading: "Reloading...".to_string(),
        }
    }

    fn worm_init(index: i32, stats_x: i32, health: i32, visible: bool) -> WormInit {
        WormInit {
            index,
            health,
            lives: 3,
            stats_x,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::new(0, 0),
            visible,
        }
    }

    fn base_state(worms: &[WormInit], weapons: Vec<Weapon>) -> SimState {
        let width = 320;
        let height = 200;
        let level = LevelData {
            width,
            height,
            material_id: vec![0u8; (width * height) as usize],
            palette: None,
            display: None,
        };
        let flags = [0u8; 256];
        SimState::new(
            &level,
            worms,
            0,
            &flags,
            weapons,
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
        )
    }

    #[test]
    fn killemall_visible_life_kills_lives_and_dying_countdown() {
        // Worm 0: visible, health 60/100, stats_x 0 -> life bar width 60 at
        // (0, 200-39=161), color 60/10+234=240; kills text at (0,171) color 10;
        // lives text at (0,178) color 6. Worm 1: dying (visible=false,
        // killed_timer=37), stats_x 218 -> countdown bar width 100-(37*25)/37=75,
        // color 75/10+234=241, at (218,161).
        let pal = ramp_pal();
        let font = origin_font();
        let lbl = labels();
        let worms = [worm_init(0, 0, 60, true), worm_init(1, 218, 100, false)];
        let mut state = base_state(&worms, vec![]);
        // KillEmAll (default), settings_health 100 (default).
        state.worms[0].kills = 5;
        state.worms[0].lives = 3;
        state.worms[0].current_weapon = 0;
        state.worms[0].weapons[0] = WormWeapon {
            ty: None,
            ammo: 0,
            delay_left: 0,
            loading_left: 0,
        };
        state.worms[1].killed_timer = 37;
        state.worms[1].current_weapon = 0;
        state.worms[1].weapons[0] = WormWeapon {
            ty: None,
            ammo: 0,
            delay_left: 0,
            loading_left: 0,
        };

        let mut b = filled(320, 200);
        draw_hud(&mut b, &pal, &state, 0, &font, &lbl, 200, 1);
        draw_hud(&mut b, &pal, &state, 1, &font, &lbl, 200, 1);

        let px = |b: &Bitmap, x: i32, y: i32| b.pixels[(y * b.pitch + x) as usize];

        // Worm 0 life bar: width 60, height 2, at (0,161)-(59,162), pal[240].
        assert_eq!(
            px(&b, 0, 161),
            0xFF00_0000 | 240,
            "life bar left px pal[240]"
        );
        assert_eq!(
            px(&b, 59, 161),
            0xFF00_0000 | 240,
            "life bar right px pal[240]"
        );
        assert_eq!(
            px(&b, 59, 162),
            0xFF00_0000 | 240,
            "life bar row 2 pal[240]"
        );
        assert_eq!(px(&b, 60, 161), SENTINEL, "one past the bar is untouched");
        assert_eq!(
            px(&b, 0, 163),
            SENTINEL,
            "below the 2px-high bar is untouched"
        );
        // Worm 0 kills text: first glyph origin at (0,171), color 10.
        assert_eq!(px(&b, 0, 171), 0xFF00_0000 | 10, "kills text glyph pal[10]");
        // Worm 0 lives text: first glyph origin at (0,178), color 6.
        assert_eq!(px(&b, 0, 178), 0xFF00_0000 | 6, "lives text glyph pal[6]");

        // Worm 1 dying countdown bar: width 75, color 241, at (218,161)-(292,162).
        assert_eq!(
            px(&b, 218, 161),
            0xFF00_0000 | 241,
            "dying bar left px pal[241]"
        );
        assert_eq!(
            px(&b, 292, 161),
            0xFF00_0000 | 241,
            "dying bar right px pal[241]"
        );
        assert_eq!(
            px(&b, 293, 161),
            SENTINEL,
            "one past the dying bar untouched"
        );
        // Worm 1 kills/lives text still draw at stats_x 218.
        assert_eq!(
            px(&b, 218, 171),
            0xFF00_0000 | 10,
            "worm1 kills glyph pal[10]"
        );
        assert_eq!(
            px(&b, 218, 178),
            0xFF00_0000 | 6,
            "worm1 lives glyph pal[6]"
        );
    }

    #[test]
    fn reload_arm_loading_bar_and_blinking_text() {
        // Worm reloading: available()==false (loading_left 50). Weapon
        // loading_time 100, settings_loading_time 100 -> ComputedLoadingTime =
        // (100*100)/100 = 100; ammo_bar_width = 100 - 50*100/100 = 50, color
        // 50/10+245 = 250, at (0, 200-34=166). Reloading text gate
        // (cycles%20)>10 && visible: cycles 15 -> 15>10 true, visible true ->
        // draws at (0, 164*1=164) color 50.
        let pal = ramp_pal();
        let font = origin_font();
        let lbl = labels();
        let weapon = Weapon {
            loading_time: 100,
            ammo: 100,
            ..Default::default()
        };
        let worms = [worm_init(0, 0, 100, true), worm_init(1, 218, 100, true)];
        let mut state = base_state(&worms, vec![weapon]);
        state.settings_loading_time = 100;
        state.cycles = 15;
        state.worms[0].current_weapon = 0;
        state.worms[0].weapons[0] = WormWeapon {
            ty: Some(0),
            ammo: 0,
            delay_left: 0,
            loading_left: 50,
        };
        // Keep worm 1 available with no ammo so its HUD adds no interfering pixels.
        state.worms[1].weapons[0] = WormWeapon {
            ty: None,
            ammo: 0,
            delay_left: 0,
            loading_left: 0,
        };

        let mut b = filled(320, 200);
        draw_hud(&mut b, &pal, &state, 0, &font, &lbl, 200, 1);

        let px = |b: &Bitmap, x: i32, y: i32| b.pixels[(y * b.pitch + x) as usize];

        // Loading bar: width 50, color 250, at (0,166)-(49,167).
        assert_eq!(
            px(&b, 0, 166),
            0xFF00_0000 | 250,
            "loading bar left px pal[250]"
        );
        assert_eq!(
            px(&b, 49, 166),
            0xFF00_0000 | 250,
            "loading bar right px pal[250]"
        );
        assert_eq!(
            px(&b, 50, 166),
            SENTINEL,
            "one past the loading bar untouched"
        );
        // Reloading text: first glyph origin at (0,164), color 50.
        assert_eq!(
            px(&b, 0, 164),
            0xFF00_0000 | 50,
            "reloading text glyph pal[50]"
        );
    }

    #[test]
    fn reloading_text_gate_off_when_low_cycles() {
        // (cycles%20)>10 is false for cycles 5 -> no reloading text. Same reload
        // state as above but cycles 5: the (0,164) reloading pixel must NOT paint.
        let pal = ramp_pal();
        let font = origin_font();
        let lbl = labels();
        let weapon = Weapon {
            loading_time: 100,
            ammo: 100,
            ..Default::default()
        };
        let worms = [worm_init(0, 0, 100, true), worm_init(1, 218, 100, true)];
        let mut state = base_state(&worms, vec![weapon]);
        state.settings_loading_time = 100;
        state.cycles = 5;
        state.worms[0].current_weapon = 0;
        state.worms[0].weapons[0] = WormWeapon {
            ty: Some(0),
            ammo: 0,
            delay_left: 0,
            loading_left: 50,
        };
        state.worms[1].weapons[0] = WormWeapon {
            ty: None,
            ammo: 0,
            delay_left: 0,
            loading_left: 0,
        };

        let mut b = filled(320, 200);
        draw_hud(&mut b, &pal, &state, 0, &font, &lbl, 200, 1);
        let px = |b: &Bitmap, x: i32, y: i32| b.pixels[(y * b.pitch + x) as usize];
        // The loading bar still paints (proves the else-arm ran)...
        assert_eq!(px(&b, 0, 166), 0xFF00_0000 | 250, "loading bar still drawn");
        // ...but the blinking reloading text is gated off at cycles 5.
        assert_eq!(
            px(&b, 0, 164),
            SENTINEL,
            "reloading text gated off (cycles%20<=10)"
        );
    }
}
