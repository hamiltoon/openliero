//! Player viewport: centering/scroll/clamp. Port of `viewport.hpp:11-58` +
//! `viewport.cpp:22-57`. The world-block draw (clip + DrawLevel at kOffs) lives
//! in `frame::draw` (viewport.cpp:196-210). The shake RNG branch is present but
//! gated on `shake>0` (inert in 3a); the laser-sight RNG arrives in 3b.

use crate::bitmap::Rect;
use sim::state::{WormState, KILLED_TIMER_INITIAL};
use sim_core::fixed::ftoi;
use sim_core::rng::Rand;

pub struct Viewport {
    pub rect: Rect,
    pub worm_idx: usize,
    pub x: i32,
    pub y: i32,
    pub shake: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub center_x: i32,
    pub center_y: i32,
    pub banner_y: i32,
    /// Default-seeded, display-only RNG (`viewport.hpp:33`, `rand.hpp`). Never
    /// advanced in 3a (no shake, no laser).
    pub rand: Rand,
}

impl Viewport {
    /// `viewport.hpp:12-22`.
    pub fn new(rect: Rect, worm_idx: usize) -> Viewport {
        Viewport {
            center_x: rect.width() >> 1,
            center_y: rect.height() >> 1,
            rect,
            worm_idx,
            x: 0,
            y: 0,
            shake: 0,
            max_x: 0,
            max_y: 0,
            banner_y: -8,
            rand: Rand::new(),
        }
    }

    /// The two-viewport 320x200 player layout (framehash_main.cpp:92-95).
    pub fn player_layout() -> [Viewport; 2] {
        [
            Viewport::new(Rect::new(0, 0, 158, 158), 0),
            Viewport::new(Rect::new(160, 0, 318, 158), 1),
        ]
    }

    /// `viewport.hpp:35-38`.
    pub fn set_center(&mut self, x: i32, y: i32) {
        self.x = x - self.center_x;
        self.y = y - self.center_y;
    }

    /// `viewport.hpp:40-54`.
    pub fn scroll_to(&mut self, dest_x: i32, dest_y: i32, iter: i32) {
        for _ in 0..iter {
            if self.x < dest_x - self.center_x {
                self.x += 1;
            } else if self.x > dest_x - self.center_x {
                self.x -= 1;
            }
            if self.y < dest_y - self.center_y {
                self.y += 1;
            } else if self.y > dest_y - self.center_y {
                self.y -= 1;
            }
        }
    }

    /// `viewport.cpp:22-57`. An alive, visible worm that steered missiles this tick
    /// (`steerable_count > 0`, set by the sim's `process_steerables`) is followed at the
    /// missiles' pixel centroid (`viewport.cpp:30-32`, 4½c-0 T9); otherwise at its own
    /// position.
    pub fn process(&mut self, worm: &WormState, level_w: i32, level_h: i32) {
        self.max_x = level_w - self.rect.width();
        self.max_y = level_h - self.rect.height();

        if worm.killed_timer <= 0 {
            if worm.visible {
                if worm.steerable_count > 0 {
                    self.set_center(
                        worm.steerable_sum_x / worm.steerable_count,
                        worm.steerable_sum_y / worm.steerable_count,
                    );
                } else {
                    self.set_center(ftoi(worm.pos.x), ftoi(worm.pos.y));
                }
            } else {
                self.scroll_to(ftoi(worm.pos.x), ftoi(worm.pos.y), 4);
            }
        } else if worm.health <= 0 {
            self.set_center(ftoi(worm.pos.x), ftoi(worm.pos.y));
            if worm.killed_timer == KILLED_TIMER_INITIAL {
                self.banner_y = -8;
            }
        }

        let real_shake = ftoi(self.shake);
        if real_shake > 0 {
            self.x += self.rand.bound((real_shake * 2) as u32) as i32 - real_shake;
            self.y += self.rand.bound((real_shake * 2) as u32) as i32 - real_shake;
        }

        self.x = self.x.max(0);
        self.y = self.y.max(0);
        self.x = self.x.min(self.max_x);
        self.y = self.y.min(self.max_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap::Rect;
    use sim::state::{ControlState, Ninjarope, WormState, WormWeapon, NUM_WEAPONS};
    use sim_core::vec::Vec2;

    // A worm helper: alive, visible, static, no steerables. Built as a full
    // struct literal (WormState has no Default); the test depends only on pos,
    // visible, health, killed_timer, steerable_count.
    fn worm_at(px: i32, py: i32) -> WormState {
        WormState {
            pos: Vec2::new(px << 16, py << 16),
            vel: Vec2::new(0, 0),
            aiming_angle: 0,
            health: 100,
            lives: 0,
            kills: 0,
            timer: 0,
            visible: true,
            killed_timer: 0,
            control_states: ControlState::default(),
            weapons: [WormWeapon::default(); NUM_WEAPONS],
            ninjarope: Ninjarope::default(),
            index: 0,
            stats_x: 0,
            last_killed_by_idx: -1,
            aiming_speed: 0,
            direction: 0,
            movable: true,
            able_to_jump: false,
            able_to_dig: false,
            key_change_pressed: false,
            current_weapon: 0,
            fire_cone: 0,
            leave_shell_timer: 0,
            logic_respawn: Vec2::new(0, 0),
            ready: true,
            make_sight_green: false,
            steerable_count: 0,
            steerable_sum_x: 0,
            steerable_sum_y: 0,
            current_frame: 0,
            animate: false,
            hotspot_x: 0,
            hotspot_y: 0,
        }
    }

    #[test]
    fn process_centers_on_worm_and_clamps() {
        // rect 158x158, level 200x200 -> max_x=max_y=42, center=79.
        let mut vp = Viewport::new(Rect::new(0, 0, 158, 158), 0);
        vp.process(&worm_at(100, 100), 200, 200);
        // SetCenter: x = 100 - 79 = 21, y = 21; within [0,42] -> unclamped.
        assert_eq!((vp.x, vp.y), (21, 21));
        // A worm near the far edge clamps to max.
        vp.process(&worm_at(199, 199), 200, 200);
        assert_eq!((vp.x, vp.y), (42, 42), "clamped to max_x/max_y");
        // A worm near origin clamps to 0.
        vp.process(&worm_at(1, 1), 200, 200);
        assert_eq!((vp.x, vp.y), (0, 0), "clamped to 0");
    }

    #[test]
    fn process_shake_branch_inert_leaves_rand_untouched() {
        let mut vp = Viewport::new(Rect::new(0, 0, 158, 158), 0);
        assert_eq!(vp.shake, 0, "3a viewport has no shake");
        let before = vp.rand.draws();
        vp.process(&worm_at(100, 100), 200, 200);
        assert_eq!(vp.rand.draws(), before, "shake==0 -> no rand drawn");
    }

    #[test]
    fn process_centres_on_the_steerable_centroid() {
        // viewport.cpp:30-32: an alive, visible worm steering missiles is followed at the
        // missiles' pixel centroid, not at the worm. C++ `int /` truncates like Rust's.
        let mut vp = Viewport::new(Rect::new(0, 0, 158, 158), 0);
        let mut w = worm_at(20, 20);
        w.steerable_count = 2;
        w.steerable_sum_x = 150 + 170; // x 150 and 170 -> 160
        w.steerable_sum_y = 90 + 101; // y 90 and 101 -> 95 (truncating)
        vp.process(&w, 400, 300);
        assert_eq!((vp.x, vp.y), (160 - 79, 95 - 79), "SetCenter(sum / count)");
        // Without steerables the same worm is centred on itself (clamped at 0).
        w.steerable_count = 0;
        vp.process(&w, 400, 300);
        assert_eq!((vp.x, vp.y), (0, 0));
    }

    #[test]
    fn player_layout_matches_framehash_rects() {
        let vps = Viewport::player_layout();
        assert_eq!(
            (vps[0].rect.x1, vps[0].rect.y1, vps[0].rect.x2, vps[0].rect.y2),
            (0, 0, 158, 158)
        );
        assert_eq!(vps[0].worm_idx, 0);
        assert_eq!(
            (vps[1].rect.x1, vps[1].rect.y1, vps[1].rect.x2, vps[1].rect.y2),
            (160, 0, 318, 158)
        );
        assert_eq!(vps[1].worm_idx, 1);
    }
}
