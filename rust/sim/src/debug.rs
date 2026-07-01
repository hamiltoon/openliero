//! Read-only ASCII visualization of a [`SimState`] tick, for human eyeballing
//! during the per-tick differential tests. This module never draws from
//! `state.rand` and never mutates `SimState` — it only reads already-computed
//! fields, so it carries zero bit-exactness risk for the simulation.
//!
//! # Glyph legend
//!
//! Terrain (pass 1, block-downsampled):
//! - `' '` — air
//! - `'.'` — dirt (either dirt material)
//! - `'#'` — rock
//!
//! Overlay (pass 2, point-stamped over the terrain; later entries in this
//! list win when several land on the same output cell):
//! - `':'` — bobject (blood)
//! - `'*'` — nobject (debris/splinter)
//! - `'+'` — wobject (weapon projectile)
//! - `'x'` — sobject (sound/explosion flash)
//! - `'$'` — bonus, health (`frame == 1`)
//! - `'w'` — bonus, weapon (`frame != 1`)
//! - `'@'` — worm 0, visible
//! - `'&'` — worm 1 (or any worm index >= 1), visible
//! - `'o'` — worm, not visible
//!
//! Worms are stamped last, so a worm glyph is never hidden by another object
//! sharing its output cell.

use crate::state::SimState;
use sim_core::fixed::ftoi;

/// Rendering options for [`render_ascii`].
pub struct RenderOpts {
    /// Integer downsample factor: each output cell summarizes a `scale x
    /// scale` block of level pixels. Clamped to a minimum of 1.
    pub scale: i32,
}

/// Renders one tick of `state` to a multi-line ASCII string (rows separated by
/// `'\n'`, one trailing newline). Read-only: never draws from `state.rand`,
/// never mutates `state`.
pub fn render_ascii(state: &SimState, opts: &RenderOpts) -> String {
    let (w, h) = (state.level.width, state.level.height);
    let scale = opts.scale.max(1);
    let (out_w, out_h) = ((w + scale - 1) / scale, (h + scale - 1) / scale);

    // ----- Pass 1: terrain, block-downsampled. -----------------------------
    let mut grid = vec![b' '; (out_w * out_h) as usize];
    for out_y in 0..out_h {
        for out_x in 0..out_w {
            let (mut rock, mut dirt) = (false, false);
            for block_y in 0..scale {
                for block_x in 0..scale {
                    let (x, y) = (out_x * scale + block_x, out_y * scale + block_y);
                    if x >= w || y >= h {
                        continue;
                    }
                    if state.level.rock(x, y) {
                        rock = true;
                    } else if state.level.any_dirt(x, y) {
                        dirt = true;
                    }
                }
            }
            grid[(out_x + out_y * out_w) as usize] = if rock {
                b'#'
            } else if dirt {
                b'.'
            } else {
                b' '
            };
        }
    }

    // ----- Pass 2: overlay, point-stamped in low -> high priority order. ---
    let mut put = |px: i32, py: i32, glyph: u8| {
        let (out_x, out_y) = (px.div_euclid(scale), py.div_euclid(scale));
        if out_x >= 0 && out_x < out_w && out_y >= 0 && out_y < out_h {
            grid[(out_x + out_y * out_w) as usize] = glyph;
        }
    };

    for b in state.bobjects.iter() {
        put(ftoi(b.pos.x), ftoi(b.pos.y), b':');
    }
    for n in state.nobjects.iter() {
        put(ftoi(n.pos.x), ftoi(n.pos.y), b'*');
    }
    for p in state.wobjects.iter() {
        put(ftoi(p.pos.x), ftoi(p.pos.y), b'+');
    }
    for so in state.sobjects.iter() {
        put(so.x, so.y, b'x');
    }
    for bo in state.bonuses.iter() {
        put(bo.x, bo.y, if bo.frame == 1 { b'$' } else { b'w' });
    }
    for (i, wm) in state.worms.iter().enumerate() {
        let glyph = if !wm.visible {
            b'o'
        } else if i == 0 {
            b'@'
        } else {
            b'&'
        };
        put(ftoi(wm.pos.x), ftoi(wm.pos.y), glyph);
    }

    // ----- Assemble rows into the output string. ----------------------------
    let mut out = String::with_capacity(((out_w + 1) * out_h) as usize);
    for out_y in 0..out_h {
        let row = &grid[(out_y * out_w) as usize..((out_y + 1) * out_w) as usize];
        out.push_str(std::str::from_utf8(row).expect("grid is ASCII"));
        out.push('\n');
    }
    out
}
