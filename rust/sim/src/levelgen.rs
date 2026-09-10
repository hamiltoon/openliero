//! Random level generation — port of C++ `Level::GenerateRandom` and friends
//! (`src/game/level.cpp:11-193`, `:397-429`) plus `BlitStone` (`gfx/blit.cpp:462-532`).
//! Step 4½, slice 4½b.
//!
//! Bit-exact vs C++ for the same `Rand` state, TC assets and dimensions — gated by
//! `oracle-tests/tests/levelgen_golden.rs` against `oracle_dump_levelgen`, which hashes
//! `material_id` and `rand.last` after every stage. Integer-only; no I/O; no `SimState`.
//!
//! * **The generator never constructs a `Rand`.** Every entry point takes `&mut Rand`; the
//!   game shell seeds a dedicated one from the match seed (overview locked decision 6,
//!   design §2), the oracle drives it from any seeded state.
//! * **Flags are read live** from `material_flags[material_id[i]]` — equivalent to the C++
//!   `materials[]` cache because every generation writer keeps that cache in sync and the
//!   field writes every cell before any flag is read (design F6).
//! * **C++ `rand()` returns `u32`**: `rand(n) - 8`, `height - 1 - rand(20)` wrap and convert
//!   back to `int`, which is exactly `rand.bound(n) as i32 - 8` here (`bound < n <= 4096`,
//!   design F7). No C++ expression here holds two `rand()` calls, so there is no
//!   unspecified evaluation order to reproduce.

use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::rng::Rand;

use crate::blit::draw_dirt_effect;
use crate::state::{LevelSim, MAT_BACKGROUND, MAT_DIRT_ROCK, MAT_ROCK};

/// Largest level side C++ accepts (`level.cpp:232`, `assets/src/level.rs:51`).
const MAX_DIM: i32 = 4096;

/// `Material::kSeeShadow` (`material.hpp:11`, `1 << 4`): a background shade that shows a
/// cast shadow. Same value as `render::shadow_query::MAT_SEE_SHADOW` and as the
/// `pub const MAT_SEE_SHADOW` the parallel slice 4½a adds to `sim::state`; private here so
/// the two slices stay merge-free (design §1). Deduplicate onto `sim::state` in slice 4½d.
const MAT_SEE_SHADOW: u8 = 1 << 4;

/// `stone_tab` (`common.cpp:23`): the four large-sprite frames of each 32x32 rock
/// formation, in quadrant order top-left, top-right, bottom-left, bottom-right.
pub const STONE_TAB: [[usize; 4]; 3] = [[98, 60, 61, 62], [63, 75, 85, 86], [89, 90, 97, 96]];

/// Diagnostic statistics of one rock-placement loop (no C++ counterpart — C++ returns
/// nothing). `count` = the drawn number of items, `placed` = how many found a rock-free
/// window before the `kMaxTries` cap, `tries` = candidate positions drawn (loop
/// iterations) summed over all items. The golden compares all three, which proves the cap
/// path and the retry path ran and localises a divergence to the predicate vs the draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RockStats {
    pub count: i32,
    pub placed: i32,
    pub tries: u64,
}

/// The TC assets generation reads (C++ `Common`): the 16x16 large-sprite bank (110
/// frames, `common.cpp:368`), the texture table (tc.cfg `[[constants.textures]]`) and the
/// 256-entry material flag table (tc.cfg `materials`).
pub struct LevelGenAssets<'a> {
    pub large_sprites: &'a SpriteSet,
    pub textures: &'a [Texture],
    pub material_flags: &'a [u8; 256],
}

/// `Level::Resize` (`level.cpp:218-227`) on a fresh level: a zero-filled `width × height`
/// material map carrying the TC flag table. Precondition `1 <= width, height <= 4096`
/// (the menu range is 64..=4096 step 8, `gfx.cpp:1295-1297`, but a hand-edited setup file
/// can hold any value).
pub fn new_level(width: i32, height: i32, material_flags: &[u8; 256]) -> LevelSim {
    debug_assert!(
        (1..=MAX_DIM).contains(&width) && (1..=MAX_DIM).contains(&height),
        "level size {width}x{height} outside 1..=4096"
    );
    LevelSim {
        width,
        height,
        material_id: vec![0u8; (width * height) as usize],
        material_flags: *material_flags,
    }
}

/// The diffusion noise field — first block of `GenerateDirtPattern` (`level.cpp:12-26`).
/// Draws exactly `width * height` values: the corner `rand(7)+12`; the first COLUMN, each
/// `(rand(7)+12 + above) >> 1`; the first ROW, each `(rand(7)+12 + left) >> 1`; the
/// interior row-major, each `(left + up + rand(8)+12) / 3`. Values stay in 12..=18 (the
/// tc.cfg dirt shades).
pub fn generate_dirt_field(level: &mut LevelSim, rand: &mut Rand) {
    let w = level.width;
    let h = level.height;
    let at = |x: i32, y: i32| (x + y * w) as usize;
    let corner = rand.bound(7) + 12;
    level.set_material(0, corner as u8);
    for y in 1..h {
        let up = level.material_id[at(0, y - 1)] as u32;
        let v = (rand.bound(7) + 12 + up) >> 1;
        level.set_material(at(0, y), v as u8);
    }
    for x in 1..w {
        let left = level.material_id[at(x - 1, 0)] as u32;
        let v = (rand.bound(7) + 12 + left) >> 1;
        level.set_material(at(x, 0), v as u8);
    }
    for y in 1..h {
        for x in 1..w {
            let left = level.material_id[at(x - 1, y)] as u32;
            let up = level.material_id[at(x, y - 1)] as u32;
            let v = (left + up + rand.bound(8) + 12) / 3;
            level.set_material(at(x, y), v as u8);
        }
    }
}

/// Port of `BlitStone(common, level, p1 = false, mem, x, y)` (`gfx/blit.cpp:462-532`).
/// `CLIP_IMAGE` (`gfx/macros.hpp:3-22`) against **`Rect(0, 0, width, height)` — the full
/// height** (unlike `DrawDirtEffect`'s `height - 1`), then every NON-ZERO texel overwrites
/// the destination (the `p1 = false` branch has no material test, `:505-531`). Every
/// C++ caller is the generator and passes `p1 = false`; the `p1 = true` branch has no caller
/// and is not ported.
///
/// The clipping block duplicates its twin in `draw_dirt_effect` (`sim/src/blit.rs:58-90`,
/// which clips to `height - 1`); the two are to be merged into one `CLIP_IMAGE` helper
/// after slice 4½a lands (controller ruling R10 — `blit.rs` is not touched here).
pub fn blit_stone(level: &mut LevelSim, large_sprites: &SpriteSet, frame: usize, x: i32, y: i32) {
    let sprite = large_sprites.sprite(frame);
    let pitch: i32 = 16;
    let mut w: i32 = 16;
    let mut h: i32 = 16;
    let mut mem: i32 = 0;
    let mut x = x;
    let mut y = y;
    let (cx1, cy1, cx2, cy2) = (0i32, 0i32, level.width, level.height);
    let top = y - cy1;
    if top < 0 {
        mem += -top * pitch;
        h += top;
        y = cy1;
    }
    let bottom = y + h - cy2;
    if bottom > 0 {
        h -= bottom;
    }
    let left = x - cx1;
    if left < 0 {
        mem -= left;
        w += left;
        x = cx1;
    }
    let right = x + w - cx2;
    if right > 0 {
        w -= right;
    }
    if w <= 0 || h <= 0 {
        return;
    }
    let level_width = level.width;
    for y_ in 0..h {
        for x_ in 0..w {
            let c = sprite[(mem + y_ * pitch + x_) as usize];
            if c != 0 {
                level.set_material(((y + y_) * level_width + x + x_) as usize, c);
            }
        }
    }
}

/// One large-sprite splat (`level.cpp:38-70`): rows `my >= height` BREAK, `my < 0`
/// CONTINUE (same for columns) — a literal port, not a clamp. A non-zero texel over a
/// destination in 177..=179 (`> 176 && < 180`) blends to `(src + dest) / 2`, otherwise it
/// overwrites.
fn splat_sprite(level: &mut LevelSim, image: &[u8], kx: i32, ky: i32) {
    for cy in 0..16 {
        let my = cy + ky;
        if my >= level.height {
            break;
        }
        if my < 0 {
            continue;
        }
        for cx in 0..16 {
            let mx = cx + kx;
            if mx >= level.width {
                break;
            }
            if mx < 0 {
                continue;
            }
            let src = image[((cy << 4) + cx) as usize];
            if src > 0 {
                let idx = (mx + my * level.width) as usize;
                let pix = level.material_id[idx];
                let v = if pix > 176 && pix < 180 {
                    ((src as u32 + pix as u32) / 2) as u8
                } else {
                    src
                };
                level.set_material(idx, v);
            }
        }
    }
}

/// `GenerateDirtPattern`'s splat block (`level.cpp:30-71`): `count = rand(100)`, then per
/// splat `x = rand(w)-8`, `y = rand(h)-8`, frame `rand(4)+69`. Draws `1 + 3*count`.
pub fn splat_large_sprites(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    let count = rand.bound(100) as i32;
    for _ in 0..count {
        let kx = rand.bound(level.width as u32) as i32 - 8;
        let ky = rand.bound(level.height as u32) as i32 - 8;
        let frame = rand.bound(4) as usize + 69;
        splat_sprite(level, large_sprites.sprite(frame), kx, ky);
    }
}

/// `GenerateDirtPattern`'s stone block (`level.cpp:73-82`): `count = rand(15)`, then per
/// stone `x = rand(w)-8`, `y = rand(h)-8`, frame `rand(4)+56`, [`blit_stone`]. Draws
/// `1 + 3*count`.
pub fn scatter_stones(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    let count = rand.bound(15) as i32;
    for _ in 0..count {
        let kx = rand.bound(level.width as u32) as i32 - 8;
        let ky = rand.bound(level.height as u32) as i32 - 8;
        let frame = rand.bound(4) as usize + 56;
        blit_stone(level, large_sprites, frame, kx, ky);
    }
}

/// `Level::GenerateDirtPattern` (`level.cpp:11-83`) = field, splats, stones.
pub fn generate_dirt_pattern(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    generate_dirt_field(level, rand);
    splat_large_sprites(level, large_sprites, rand);
    scatter_stones(level, large_sprites, rand);
}

/// The tunnel walk of `GenerateRandom` (`level.cpp:108-135`), with the per-stamp action
/// injected so tests can record positions. `count = rand(50)+5` tunnels; each starts at
/// `(rand(w)-8, rand(h)-8)` with step `(rand(11)-5, rand(5)-2)` and `rand(12)` segments; a
/// segment draws `count3 = rand(5)`, takes `count3` steps (stamping after each), backtracks
/// by `(count3 + 1) * step`, then jitters by `(rand(7)-3, rand(15)-7)`.
fn tunnel_walk(
    width: i32,
    height: i32,
    rand: &mut Rand,
    mut stamp: impl FnMut(&mut Rand, i32, i32),
) {
    let count = rand.bound(50) as i32 + 5;
    for _ in 0..count {
        let mut cx = rand.bound(width as u32) as i32 - 8;
        let mut cy = rand.bound(height as u32) as i32 - 8;
        let dx = rand.bound(11) as i32 - 5;
        let dy = rand.bound(5) as i32 - 2;
        let count2 = rand.bound(12) as i32;
        for _ in 0..count2 {
            let count3 = rand.bound(5) as i32;
            for _ in 0..count3 {
                cx += dx;
                cy += dy;
                stamp(&mut *rand, cx, cy);
            }
            cx -= (count3 + 1) * dx;
            cy -= (count3 + 1) * dy;
            cx += rand.bound(7) as i32 - 3;
            cy += rand.bound(15) as i32 - 7;
        }
    }
}

/// `GenerateRandom`'s dirt-effect worm tunnels (`level.cpp:108-135`): every stamp is
/// `DrawDirtEffect(texture 1, cx, cy)` with `(cx, cy)` the window's TOP-LEFT (no `-7`,
/// unlike `BlowUpObject`). Texture 1 is the carving texture (tc.cfg `mframe=1, rframe=2,
/// sframe=73, ndrawback=true`). Each stamp draws its own `rand(r_frame)` first, even when
/// it is clipped away entirely (`blit.cpp:537`) — reused unchanged from Step 2
/// ([`draw_dirt_effect`]).
pub fn dig_tunnels(
    level: &mut LevelSim,
    large_sprites: &SpriteSet,
    textures: &[Texture],
    rand: &mut Rand,
) {
    let (w, h) = (level.width, level.height);
    tunnel_walk(w, h, rand, |rand, x, y| {
        draw_dirt_effect(level, large_sprites, textures, 1, x, y, rand)
    });
}

/// Port of the file-static `IsNoRock` (`level.cpp:85-99`): the `(size+1) x (size+1)`
/// window at `(x, y)`, intersected with the level, contains no `Rock()` cell. A window
/// clipped to nothing is rock-free.
fn is_no_rock(level: &LevelSim, size: i32, x: i32, y: i32) -> bool {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + size + 1).min(level.width);
    let y2 = (y + size + 1).min(level.height);
    for yy in y1..y2 {
        for xx in x1..x2 {
            if level.rock(xx, yy) {
                return false;
            }
        }
    }
    true
}

/// The 2x2 [`blit_stone`] of one 32x32 formation (`level.cpp:162-169`).
pub fn blit_formation(
    level: &mut LevelSim,
    large_sprites: &SpriteSet,
    kind: usize,
    cx: i32,
    cy: i32,
) {
    let t = STONE_TAB[kind];
    blit_stone(level, large_sprites, t[0], cx, cy);
    blit_stone(level, large_sprites, t[1], cx + 16, cy);
    blit_stone(level, large_sprites, t[2], cx, cy + 16);
    blit_stone(level, large_sprites, t[3], cx + 16, cy + 16);
}

/// The shared `do { … } while (!IsNoRock(size, cx, cy) && ++tries < kMaxTries)` retry loop
/// of both rock stages (`level.cpp:147-155` formations, `:178-186` rocks), with
/// `kMaxTries = width * height` (`:140`). Per candidate, in C++ statement order:
/// `cx = rand(w) - off`; then `rand(bottom_die) == 0 ? h - 1 - rand(bottom_range)
/// : rand(h) - off`. The rejection counter increments ONLY on rejection (the `&&`
/// short-circuits on acceptance), so `None` ⇔ C++ `tries >= kMaxTries` after the loop.
/// `stats.tries` counts every candidate drawn.
fn find_rock_free(
    level: &LevelSim,
    rand: &mut Rand,
    stats: &mut RockStats,
    off: i32,
    bottom_die: u32,
    bottom_range: u32,
    size: i32,
) -> Option<(i32, i32)> {
    let max_tries = level.width * level.height;
    let mut tries = 0i32;
    loop {
        stats.tries += 1;
        let cx = rand.bound(level.width as u32) as i32 - off;
        let cy = if rand.bound(bottom_die) == 0 {
            level.height - 1 - rand.bound(bottom_range) as i32
        } else {
            rand.bound(level.height as u32) as i32 - off
        };
        if is_no_rock(level, size, cx, cy) {
            return Some((cx, cy));
        }
        tries += 1;
        if tries >= max_tries {
            return None;
        }
    }
}

/// `GenerateRandom`'s rock formations (`level.cpp:137-170`). `kMaxTries = w*h`;
/// `count = rand(15)+5`; per formation draw candidates `cx = rand(w)-16`,
/// `cy = rand(4)==0 ? h-1-rand(20) : rand(h)-16` until `is_no_rock(32)` accepts or the
/// rejection counter (incremented ONLY on rejection) reaches the cap; on the cap skip the
/// formation WITHOUT drawing its kind; else `kind = rand(3)` and [`blit_formation`].
pub fn place_rock_formations(
    level: &mut LevelSim,
    large_sprites: &SpriteSet,
    rand: &mut Rand,
) -> RockStats {
    let mut s = RockStats {
        count: rand.bound(15) as i32 + 5,
        placed: 0,
        tries: 0,
    };
    for _ in 0..s.count {
        let Some((cx, cy)) = find_rock_free(level, rand, &mut s, 16, 4, 20, 32) else {
            continue;
        };
        s.placed += 1;
        let kind = rand.bound(3) as usize;
        blit_formation(level, large_sprites, kind, cx, cy);
    }
    s
}

/// `GenerateRandom`'s 16x16 rocks (`level.cpp:172-192`): `count = rand(25)+5`; candidates
/// `cx = rand(w)-8`, `cy = rand(5)==0 ? h-1-rand(13) : rand(h)-8`; `is_no_rock(15)`;
/// same cap rule; placed ⇒ frame `rand(6)+3` via [`blit_stone`].
pub fn place_rocks(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) -> RockStats {
    let mut s = RockStats {
        count: rand.bound(25) as i32 + 5,
        placed: 0,
        tries: 0,
    };
    for _ in 0..s.count {
        let Some((cx, cy)) = find_rock_free(level, rand, &mut s, 8, 5, 13, 15) else {
            continue;
        };
        s.placed += 1;
        let frame = rand.bound(6) as usize + 3;
        blit_stone(level, large_sprites, frame, cx, cy);
    }
    s
}

/// `Level::GenerateRandom` (`level.cpp:101-193`) minus the palette reset (a generated level
/// has no custom palette — [`generate_from_settings`] returns `palette: None`, design F8).
/// Stages in C++ order: resize, dirt pattern, tunnels, formations, rocks.
pub fn generate_random(
    assets: &LevelGenAssets,
    width: i32,
    height: i32,
    rand: &mut Rand,
) -> LevelSim {
    let mut level = new_level(width, height, assets.material_flags);
    generate_dirt_pattern(&mut level, assets.large_sprites, rand);
    dig_tunnels(&mut level, assets.large_sprites, assets.textures, rand);
    place_rock_formations(&mut level, assets.large_sprites, rand);
    place_rocks(&mut level, assets.large_sprites, rand);
    level
}

/// Flag byte of the in-bounds pixel `(x, y)` — the C++ `Mat(x, y)` read, derived live
/// from `material_flags[material_id]`.
fn flags_at(level: &LevelSim, x: i32, y: i32) -> u8 {
    level.material_flags[level.material_id[(x + y * level.width) as usize] as usize]
}

/// Port of `Level::MakeShadow` (`level.cpp:195-216`). For `x in 0..w-3` (outer), `y in 3..h`
/// (inner): (1) `SeeShadow(x,y) && DirtRock(x+3,y-3)` ⇒ pixel `+4` (wrapping `PalIdx`);
/// (2) then, re-reading the possibly updated pixel, `12..=18 && Rock(x+3,y-3)` ⇒ `-2`,
/// floored at 12. Finally every `Background` pixel of the bottom row (all x) becomes 13.
/// In place, like C++ `SetPixel`: the neighbour `(x+3, y-3)` lies in a column not yet
/// visited, so it is always read pre-pass (design §5). Precondition `height >= 1`.
pub fn make_shadow(level: &mut LevelSim) {
    let w = level.width;
    let h = level.height;
    for x in 0..w - 3 {
        for y in 3..h {
            let idx = (x + y * w) as usize;
            if flags_at(level, x, y) & MAT_SEE_SHADOW != 0
                && flags_at(level, x + 3, y - 3) & MAT_DIRT_ROCK != 0
            {
                let p = level.material_id[idx];
                level.set_material(idx, p.wrapping_add(4));
            }
            let p = level.material_id[idx];
            if (12..=18).contains(&p) && flags_at(level, x + 3, y - 3) & MAT_ROCK != 0 {
                let dimmed = p - 2;
                level.set_material(idx, if dimmed < 12 { 12 } else { dimmed });
            }
        }
    }
    for x in 0..w {
        if flags_at(level, x, h - 1) & MAT_BACKGROUND != 0 {
            level.set_material((x + (h - 1) * w) as usize, 13);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MAT_ROCK;
    use crate::state::{MAT_BACKGROUND, MAT_DIRT};

    fn seeded(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    #[test]
    fn new_level_is_zero_filled_with_dims_and_flags() {
        let mut flags = [0u8; 256];
        flags[7] = MAT_ROCK;
        let level = new_level(5, 3, &flags);
        assert_eq!((level.width, level.height), (5, 3));
        assert_eq!(level.material_id, vec![0u8; 15]);
        assert_eq!(level.material_flags, flags);
    }

    /// level.cpp:12-26 restated statement by statement with an identically seeded
    /// reference `Rand`: corner, then COLUMN x=0 (y=1..h), then ROW y=0 (x=1..w), then the
    /// interior row-major. Swapping the column and row loops shifts every later draw.
    #[test]
    fn dirt_field_follows_level_cpp_12_26_draw_order() {
        let (w, h) = (9i32, 7i32);
        let mut r = seeded(7);
        let mut want = vec![0u8; (w * h) as usize];
        let at = |x: i32, y: i32| (x + y * w) as usize;
        want[0] = (r.bound(7) + 12) as u8;
        for y in 1..h {
            want[at(0, y)] = ((r.bound(7) + 12 + want[at(0, y - 1)] as u32) >> 1) as u8;
        }
        for x in 1..w {
            want[at(x, 0)] = ((r.bound(7) + 12 + want[at(x - 1, 0)] as u32) >> 1) as u8;
        }
        for y in 1..h {
            for x in 1..w {
                let left = want[at(x - 1, y)] as u32;
                let up = want[at(x, y - 1)] as u32;
                want[at(x, y)] = ((left + up + r.bound(8) + 12) / 3) as u8;
            }
        }

        let mut level = new_level(w, h, &[0u8; 256]);
        let mut rand = seeded(7);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(level.material_id, want);
        assert_eq!(rand.last(), r.last());
        assert_eq!(rand.draws(), r.draws());
    }

    #[test]
    fn dirt_field_draws_w_times_h_and_stays_in_12_to_18() {
        let mut level = new_level(17, 9, &[0u8; 256]);
        let mut rand = seeded(42);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(rand.draws(), 17 * 9);
        assert!(level.material_id.iter().all(|&p| (12..=18).contains(&p)));
    }

    /// A bank of `count` 16x16 sprites, all zero except the `(frame, 256 bytes)` overrides.
    fn bank(count: i32, overrides: &[(usize, Vec<u8>)]) -> SpriteSet {
        let mut data = vec![0u8; count as usize * 256];
        for (frame, bytes) in overrides {
            assert_eq!(bytes.len(), 256);
            data[frame * 256..frame * 256 + 256].copy_from_slice(bytes);
        }
        SpriteSet {
            width: 16,
            height: 16,
            count,
            data,
        }
    }

    #[test]
    fn blit_stone_overwrites_nonzero_texels_and_clips_to_full_height() {
        // texel (x_, y_) = 100 + y_*16 + x_, except texel (1, 0) = 0 (transparent).
        let mut spr: Vec<u8> = (0..256).map(|i| 100u8.wrapping_add(i as u8)).collect();
        spr[1] = 0;
        let sprites = bank(4, &[(3, spr)]);
        let mut level = new_level(20, 10, &[0u8; 256]);
        level.material_id.iter_mut().for_each(|p| *p = 7);
        blit_stone(&mut level, &sprites, 3, 15, 5); // clipped to x 15..20, y 5..10
        let at = |x: i32, y: i32| level.material_id[(x + y * 20) as usize];
        assert_eq!(at(15, 5), 100, "texel (0,0)");
        assert_eq!(at(16, 5), 7, "zero texel is transparent");
        assert_eq!(
            at(19, 9),
            100 + 4 * 16 + 4,
            "bottom row IS written: clip is Rect(0,0,w,h)"
        );
        assert_eq!(at(14, 5), 7, "left of the window untouched");
    }

    #[test]
    fn blit_stone_negative_origin_offsets_the_source_like_clip_image() {
        let spr: Vec<u8> = (0..256).map(|i| 1u8.wrapping_add(i as u8)).collect();
        let sprites = bank(1, &[(0, spr)]);
        let mut level = new_level(20, 10, &[0u8; 256]);
        blit_stone(&mut level, &sprites, 0, -3, -2); // visible window: x 0..13, y 0..14∩0..10
        assert_eq!(level.material_id[0], 1 + 2 * 16 + 3, "(0,0) <- texel (3,2)");
        assert_eq!(
            level.material_id[12],
            1 + 2 * 16 + 15,
            "(12,0) <- texel (15,2)"
        );
        assert_eq!(
            level.material_id[13], 0,
            "(13,0) is outside the clipped window"
        );
    }

    #[test]
    fn splat_blends_177_to_179_and_overwrites_everything_else() {
        // All-150 image except texel (5,0) = 0; row 0 destination = 176,177,178,179,180,55.
        let mut img = vec![150u8; 256];
        img[5] = 0;
        let mut level = new_level(16, 16, &[0u8; 256]);
        for (x, v) in [176u8, 177, 178, 179, 180, 55].iter().enumerate() {
            level.material_id[x] = *v;
        }
        splat_sprite(&mut level, &img, 0, 0);
        // (150+177)/2 = 163, (150+178)/2 = 164, (150+179)/2 = 164 (level.cpp:63-64).
        assert_eq!(&level.material_id[0..6], &[150, 163, 164, 164, 150, 55]);
        assert_eq!(level.material_id[16], 150, "(0,1) overwritten");
    }

    #[test]
    fn splat_clips_with_break_and_continue_at_every_edge() {
        let img = vec![9u8; 256];
        let mut a = new_level(10, 6, &[0u8; 256]);
        splat_sprite(&mut a, &img, -8, -8); // rand(w)-8 can be -8: cells 0..8 x 0..8, h=6
        let at = |l: &LevelSim, x: i32, y: i32| l.material_id[(x + y * 10) as usize];
        assert_eq!(at(&a, 0, 0), 9);
        assert_eq!(at(&a, 7, 5), 9);
        assert_eq!(at(&a, 8, 5), 0, "x = kX + 16 is past the sprite");
        let mut b = new_level(10, 6, &[0u8; 256]);
        splat_sprite(&mut b, &img, 5, 3); // breaks at x = 10 and y = 6
        assert_eq!(at(&b, 9, 5), 9);
        assert_eq!(at(&b, 4, 3), 0);
        assert_eq!(at(&b, 5, 2), 0);
    }

    #[test]
    fn splats_draw_one_count_plus_three_per_splat() {
        let sprites = bank(73, &[]); // frames 69..=72 must exist
        let mut level = new_level(40, 30, &[0u8; 256]);
        let mut rand = seeded(5);
        splat_large_sprites(&mut level, &sprites, &mut rand);
        let mut r = seeded(5);
        let count = r.bound(100) as u64;
        assert_eq!(rand.draws(), 1 + 3 * count);
    }

    #[test]
    fn stones_draw_one_count_plus_three_per_stone_from_frames_56_to_59() {
        let solid = |v: u8| vec![v; 256];
        let sprites = bank(
            60,
            &[
                (56, solid(56)),
                (57, solid(57)),
                (58, solid(58)),
                (59, solid(59)),
            ],
        );
        let mut level = new_level(64, 48, &[0u8; 256]);
        let mut rand = seeded(11);
        scatter_stones(&mut level, &sprites, &mut rand);
        let mut r = seeded(11);
        let count = r.bound(15) as u64;
        assert_eq!(rand.draws(), 1 + 3 * count);
        assert!(level
            .material_id
            .iter()
            .all(|&p| p == 0 || (56..=59).contains(&p)));
    }

    #[test]
    fn dirt_pattern_is_field_then_splats_then_stones() {
        let sprites = bank(73, &[(70, vec![200; 256]), (57, vec![30; 256])]);
        let mut a = new_level(50, 40, &[0u8; 256]);
        let mut ra = seeded(3);
        generate_dirt_pattern(&mut a, &sprites, &mut ra);
        let mut b = new_level(50, 40, &[0u8; 256]);
        let mut rb = seeded(3);
        generate_dirt_field(&mut b, &mut rb);
        splat_large_sprites(&mut b, &sprites, &mut rb);
        scatter_stones(&mut b, &sprites, &mut rb);
        assert_eq!(a, b);
        assert_eq!(ra.last(), rb.last());
    }

    /// level.cpp:108-135 restated statement by statement (the stamp recorder draws one
    /// value per stamp, as DrawDirtEffect's rand(r_frame) does). Pins the draw order, the
    /// `(count3 + 1) * d` backtrack and the jitter ranges in readable form; the golden's
    /// `tunnels` stage token is the correctness gate.
    #[test]
    fn tunnel_walk_follows_level_cpp_108_135() {
        let (w, h) = (300, 200);
        let mut got = Vec::new();
        let mut rand = seeded(99);
        tunnel_walk(w, h, &mut rand, |r, x, y| {
            r.bound(2);
            got.push((x, y));
        });

        let mut r = seeded(99);
        let mut want = Vec::new();
        let count = r.bound(50) as i32 + 5;
        for _ in 0..count {
            let mut cx = r.bound(w as u32) as i32 - 8;
            let mut cy = r.bound(h as u32) as i32 - 8;
            let dx = r.bound(11) as i32 - 5;
            let dy = r.bound(5) as i32 - 2;
            let count2 = r.bound(12) as i32;
            for _ in 0..count2 {
                let count3 = r.bound(5) as i32;
                for _ in 0..count3 {
                    cx += dx;
                    cy += dy;
                    r.bound(2);
                    want.push((cx, cy));
                }
                cx -= (count3 + 1) * dx;
                cy -= (count3 + 1) * dy;
                cx += r.bound(7) as i32 - 3;
                cy += r.bound(15) as i32 - 7;
            }
        }
        assert_eq!(got, want);
        assert_eq!(rand.draws(), r.draws());
        assert!(!got.is_empty());
    }

    /// Every stamp draws rand(r_frame) even when it is fully clipped (blit.cpp:537 runs
    /// before the clip at :545-547). On a 4x4 level almost every 16x16 stamp is clipped, so
    /// a port that skipped the draw for off-level stamps would diverge here.
    #[test]
    fn dig_tunnels_draws_once_per_stamp_even_when_clipped() {
        let tex = Texture {
            mframe: 0,
            rframe: 2,
            sframe: 1,
            ndrawback: true,
        };
        let textures = vec![tex.clone(), tex];
        let sprites = bank(3, &[]);
        let mut level = new_level(4, 4, &[0u8; 256]);
        let mut rand = seeded(21);
        dig_tunnels(&mut level, &sprites, &textures, &mut rand);

        let mut r = seeded(21);
        let mut stamps = 0u64;
        tunnel_walk(4, 4, &mut r, |r, _, _| {
            r.bound(2);
            stamps += 1;
        });
        assert!(stamps > 0);
        assert_eq!(rand.draws(), r.draws());
        assert_eq!(rand.last(), r.last());
    }

    /// dirt effect 1 = tc.cfg texture 1 (mframe 1, carving: ndrawback = true). With an
    /// all-1 mask over all-Dirt terrain every in-clip window cell becomes material 1
    /// (blit.cpp:566-579), the window's top-left is the walked (cx, cy) — no -7 offset —
    /// and the clip is Rect(0, 0, w, h - 1).
    #[test]
    fn dig_tunnels_carves_texture_1_at_the_walked_top_left_positions() {
        let mut flags = [0u8; 256];
        flags[12] = MAT_DIRT;
        flags[1] = MAT_DIRT | MAT_BACKGROUND; // tc.cfg materials[1] = 9
        let textures = vec![
            Texture {
                mframe: 0,
                rframe: 2,
                sframe: 2,
                ndrawback: true,
            },
            Texture {
                mframe: 1,
                rframe: 2,
                sframe: 2,
                ndrawback: true,
            },
        ];
        let sprites = bank(4, &[(1, vec![1u8; 256])]);
        let (w, h) = (64, 48);
        let mut level = new_level(w, h, &flags);
        level.material_id.iter_mut().for_each(|p| *p = 12);
        let mut rand = seeded(8);
        dig_tunnels(&mut level, &sprites, &textures, &mut rand);

        let mut r = seeded(8);
        let mut want = vec![12u8; (w * h) as usize];
        tunnel_walk(w, h, &mut r, |r, x, y| {
            r.bound(2);
            for my in y.max(0)..(y + 16).min(h - 1) {
                for mx in x.max(0)..(x + 16).min(w) {
                    want[(mx + my * w) as usize] = 1;
                }
            }
        });
        assert_eq!(level.material_id, want);
    }

    /// The shipped TC's large-sprite bank, texture table and material flags.
    fn real_tc() -> (SpriteSet, Vec<Texture>, [u8; 256]) {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc_bytes = std::fs::read(format!("{root}/tc.cfg")).expect("read tc.cfg");
        let tc = assets::tc::TcConfig::load(&tc_bytes).expect("tc.cfg parses");
        let tga_bytes = std::fs::read(format!("{root}/sprites/large.tga")).expect("read large.tga");
        let tga = assets::sprite::Tga::load(&tga_bytes).expect("large.tga parses");
        let large = SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank");
        (large, tc.textures.clone(), tc.materials)
    }

    #[test]
    fn is_no_rock_checks_a_size_plus_one_window_clipped_to_the_level() {
        let mut flags = [0u8; 256];
        flags[5] = MAT_ROCK;
        let mut level = new_level(40, 40, &flags);
        level.material_id[(10 + 10 * 40) as usize] = 5; // rock at (10,10)
        assert!(
            !is_no_rock(&level, 4, 6, 6),
            "(10,10) is the far corner of the 5x5 window"
        );
        assert!(is_no_rock(&level, 4, 5, 5), "window 5..=9 excludes (10,10)");
        assert!(
            !is_no_rock(&level, 15, -5, -5),
            "negative origin clips to 0..11"
        );
        assert!(
            is_no_rock(&level, 32, 39, 39),
            "window clipped to the single cell (39,39)"
        );
    }

    #[test]
    fn blit_formation_lays_stone_tab_out_as_2x2() {
        let overrides: Vec<(usize, Vec<u8>)> = STONE_TAB[1]
            .iter()
            .map(|&f| (f, vec![f as u8; 256]))
            .collect();
        let sprites = bank(99, &overrides);
        let mut level = new_level(40, 40, &[0u8; 256]);
        blit_formation(&mut level, &sprites, 1, 4, 2);
        let at = |x: i32, y: i32| level.material_id[(x + y * 40) as usize];
        assert_eq!(at(4, 2), 63, "top-left = stone_tab[1][0]");
        assert_eq!(at(20, 2), 75, "top-right = stone_tab[1][1] at cx+16");
        assert_eq!(at(4, 18), 85, "bottom-left = stone_tab[1][2] at cy+16");
        assert_eq!(at(20, 18), 86, "bottom-right = stone_tab[1][3]");
    }

    /// 8x8 all-rock: every candidate is rejected, kMaxTries = 64 per formation, and on the
    /// cap the loop `continue`s BEFORE rand(3) (level.cpp:155-160).
    #[test]
    fn formations_on_solid_rock_hit_the_cap_and_skip_the_kind_draw() {
        let mut flags = [0u8; 256];
        flags[5] = MAT_ROCK;
        let mut level = new_level(8, 8, &flags);
        level.material_id.iter_mut().for_each(|p| *p = 5);
        let sprites = bank(99, &[]);
        let mut rand = seeded(13);
        let stats = place_rock_formations(&mut level, &sprites, &mut rand);

        let mut r = seeded(13);
        let count = r.bound(15) as i32 + 5;
        for _ in 0..count {
            for _ in 0..64 {
                r.bound(8);
                if r.bound(4) == 0 {
                    r.bound(20);
                } else {
                    r.bound(8);
                }
            }
        }
        assert_eq!(
            stats,
            RockStats {
                count,
                placed: 0,
                tries: 64 * count as u64
            }
        );
        assert_eq!(
            rand.draws(),
            r.draws(),
            "no rand(3) after a capped formation"
        );
        assert!(level.material_id.iter().all(|&p| p == 5), "nothing blitted");
    }

    /// Same for the 16x16 rocks (level.cpp:172-192): candidates rand(w)-8 and
    /// rand(5)==0 ? h-1-rand(13) : rand(h)-8; on the cap no rand(6).
    #[test]
    fn rocks_on_solid_rock_hit_the_cap_and_skip_the_sprite_draw() {
        let mut flags = [0u8; 256];
        flags[5] = MAT_ROCK;
        let mut level = new_level(8, 8, &flags);
        level.material_id.iter_mut().for_each(|p| *p = 5);
        let sprites = bank(99, &[]);
        let mut rand = seeded(14);
        let stats = place_rocks(&mut level, &sprites, &mut rand);

        let mut r = seeded(14);
        let count = r.bound(25) as i32 + 5;
        for _ in 0..count {
            for _ in 0..64 {
                r.bound(8);
                if r.bound(5) == 0 {
                    r.bound(13);
                } else {
                    r.bound(8);
                }
            }
        }
        assert_eq!(
            stats,
            RockStats {
                count,
                placed: 0,
                tries: 64 * count as u64
            }
        );
        assert_eq!(rand.draws(), r.draws(), "no rand(6) after a capped rock");
    }

    /// No rock anywhere and all-zero sprites (so blits write nothing): every formation and
    /// every rock is accepted on its first candidate.
    #[test]
    fn open_terrain_accepts_every_first_candidate() {
        let mut level = new_level(200, 150, &[0u8; 256]);
        let sprites = bank(99, &[]);
        let mut rand = seeded(17);
        let f = place_rock_formations(&mut level, &sprites, &mut rand);
        assert!((5..=19).contains(&f.count));
        assert_eq!((f.placed, f.tries), (f.count, f.count as u64));
        let k = place_rocks(&mut level, &sprites, &mut rand);
        assert!((5..=29).contains(&k.count));
        assert_eq!((k.placed, k.tries), (k.count, k.count as u64));
    }

    #[test]
    fn generate_random_composes_the_stages_in_level_cpp_order() {
        let (large, textures, flags) = real_tc();
        let assets = LevelGenAssets {
            large_sprites: &large,
            textures: &textures,
            material_flags: &flags,
        };
        let mut ra = seeded(42);
        let a = generate_random(&assets, 504, 350, &mut ra);

        let mut rb = seeded(42);
        let mut b = new_level(504, 350, &flags);
        generate_dirt_pattern(&mut b, &large, &mut rb);
        dig_tunnels(&mut b, &large, &textures, &mut rb);
        place_rock_formations(&mut b, &large, &mut rb);
        place_rocks(&mut b, &large, &mut rb);
        assert_eq!(a, b);
        assert_eq!(ra.last(), rb.last());
        assert!(
            a.material_id
                .iter()
                .any(|&m| flags[m as usize] & MAT_ROCK != 0),
            "rocks placed"
        );
        assert!(
            a.material_id
                .iter()
                .any(|&m| flags[m as usize] & MAT_BACKGROUND != 0),
            "tunnels carved background (tc.cfg materials 1/2 carry the background bit)"
        );
    }

    /// Flags shaped like the shipped tc.cfg `materials`: dirt 12..=18 (1), rock 19 (4),
    /// see-shadow background 160..=163 (24), their shadowed twins 164..=167 (8), and
    /// dirt+background 1 (9). Material 40 has no flags (the default fill).
    fn shadow_flags() -> [u8; 256] {
        let mut f = [0u8; 256];
        f[12..=18].fill(MAT_DIRT);
        f[19] = MAT_ROCK;
        f[160..=163].fill(MAT_SEE_SHADOW | MAT_BACKGROUND);
        f[164..=167].fill(MAT_BACKGROUND);
        f[1] = MAT_DIRT | MAT_BACKGROUND;
        f
    }

    fn shadow_level(w: i32, h: i32) -> LevelSim {
        let mut l = new_level(w, h, &shadow_flags());
        l.material_id.iter_mut().for_each(|p| *p = 40);
        l
    }

    fn set(l: &mut LevelSim, x: i32, y: i32, v: u8) {
        let w = l.width;
        l.material_id[(x + y * w) as usize] = v;
    }

    fn get(l: &LevelSim, x: i32, y: i32) -> u8 {
        l.material_id[(x + y * l.width) as usize]
    }

    #[test]
    fn make_shadow_adds_4_to_see_shadow_pixels_under_dirt_rock() {
        let mut l = shadow_level(10, 10);
        set(&mut l, 2, 5, 160);
        set(&mut l, 5, 2, 12); // DirtRock neighbour of (2,5)
        set(&mut l, 2, 7, 161); // neighbour (5,4) = 40: no flags
        make_shadow(&mut l);
        assert_eq!(get(&l, 2, 5), 164);
        assert_eq!(get(&l, 2, 7), 161);
    }

    #[test]
    fn make_shadow_dims_dirt_beside_rock_floored_at_12_and_skips_the_last_3_columns() {
        let mut l = shadow_level(10, 10);
        set(&mut l, 1, 6, 16);
        set(&mut l, 4, 3, 19); // rock neighbour of (1,6): 16 - 2 = 14
        set(&mut l, 1, 8, 13);
        set(&mut l, 4, 5, 19); // rock neighbour of (1,8): 13 - 2 = 11 -> floor 12
        set(&mut l, 6, 6, 14);
        set(&mut l, 9, 3, 19); // x = 6 < w - 3: processed -> 12
        set(&mut l, 7, 6, 14); // x = 7 = w - 3: never processed
        make_shadow(&mut l);
        assert_eq!(get(&l, 1, 6), 14);
        assert_eq!(get(&l, 1, 8), 12);
        assert_eq!(get(&l, 6, 6), 12);
        assert_eq!(get(&l, 7, 6), 14);
    }

    /// Rule 2 re-reads the pixel AFTER rule 1 wrote it (level.cpp:198-206): a see-shadow
    /// 10 becomes 14, which is in 12..=18 with a rock neighbour, so it drops to 12.
    #[test]
    fn make_shadow_rule_two_rereads_the_rule_one_result() {
        let mut l = shadow_level(10, 10);
        l.material_flags[10] = MAT_SEE_SHADOW;
        set(&mut l, 3, 6, 10);
        set(&mut l, 6, 3, 19); // rock: DirtRock for rule 1 AND Rock for rule 2
        make_shadow(&mut l);
        assert_eq!(get(&l, 3, 6), 12);
    }

    /// A = (0,6) looks at B = (3,3); B looks at C = (6,0). B turns 161 -> 165 and 165 is
    /// made DirtRock here, so if B were processed before A (a y-outer loop), A would darken.
    #[test]
    fn make_shadow_reads_neighbours_before_their_column_is_visited() {
        let mut l = shadow_level(10, 10);
        l.material_flags[165] = MAT_DIRT | MAT_BACKGROUND;
        set(&mut l, 0, 6, 160);
        set(&mut l, 3, 3, 161);
        set(&mut l, 6, 0, 12);
        make_shadow(&mut l);
        assert_eq!(get(&l, 3, 3), 165, "B shadowed by C");
        assert_eq!(get(&l, 0, 6), 160, "A read B before B changed");
    }

    #[test]
    fn make_shadow_sets_background_bottom_row_pixels_to_13() {
        let mut l = shadow_level(10, 10);
        set(&mut l, 0, 9, 1); // dirt+background -> 13
        set(&mut l, 9, 9, 164); // background, in the last 3 columns -> 13
        set(&mut l, 5, 9, 12); // dirt, not background -> unchanged
        make_shadow(&mut l);
        assert_eq!(get(&l, 0, 9), 13);
        assert_eq!(get(&l, 9, 9), 13);
        assert_eq!(get(&l, 5, 9), 12);
        assert_eq!(get(&l, 4, 9), 40);
    }
}
