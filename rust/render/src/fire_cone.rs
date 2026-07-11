//! Fire-cone offset table + fire-cone sprite bank. Ports `common.cpp:17-21`
//! (`fire_cone_offset`) and `common.cpp:539-555` (the `fire_cone_sprites` build,
//! part of `Common::Precompute`). The bank mirrors `large_sprites[9..9+7]` into a
//! `2*7` L/R bank exactly like [`sim::state::build_worm_sprites`] mirrors the worm
//! frames — same allocation/mirror discipline, in the `render` crate (no sim
//! change), reading `state.large_sprites`.

use assets::sprite::SpriteSet;

/// `common.cpp:17-21`. `[direction][angle_frame][x|y]`. Indexed at the fire-cone
/// draw (`viewport.cpp:541-542`) as `fire_cone_offset[w.direction][angle_frame]`.
pub const FIRE_CONE_OFFSET: [[[i32; 2]; 7]; 2] = [
    [[-3, 1], [-4, 0], [-4, -2], [-4, -4], [-3, -5], [-2, -6], [0, -6]],
    [[3, 1], [4, 0], [4, -2], [4, -4], [3, -5], [2, -6], [0, -6]],
];

/// Port of `common.cpp:539-555` (`Common::Precompute`, fire-cone section). Builds
/// the `16x16 x (2*7)` bank: for each of the 7 source frames `large_sprites[9+i]`,
/// `dir=1` (`FireConeSprite(i, 1)`) is a straight copy and `dir=0`
/// (`FireConeSprite(i, 0)`) is the horizontal mirror (`x -> 14-x`, col 15 -> 0).
/// Bank layout index `i + dir*7` == `FireConeSprite(i, dir)` (`common.hpp:153`).
///
/// Returns an EMPTY bank when `large_sprites` lacks the 7 fire-cone source frames
/// (`9..9+7`) — the same empty-bank guard `build_worm_sprites` (`state.rs:858`)
/// uses so slices that never load `large.tga` stay byte-identical.
pub fn build_fire_cone_sprites(large: &SpriteSet) -> SpriteSet {
    const FRAMES: i32 = 7;
    if large.width != 16 || large.height != 16 || large.count < 9 + FRAMES {
        return SpriteSet::default();
    }
    let count = 2 * FRAMES; // dir(2) * frames(7) == 14
    let mut data = vec![0u8; (count as usize) * 256];
    // Bank index for (i, dir): i + dir*7 (== C++ `FireConeSprite(i, dir)`).
    let base = |i: i32, dir: i32| ((i + dir * FRAMES) as usize) * 256;
    for i in 0..FRAMES {
        let src = large.sprite((9 + i) as usize); // common.cpp:544
        let d1 = base(i, 1);
        let d0 = base(i, 0);
        for y in 0..16usize {
            for x in 0..16usize {
                let pix = src[y * 16 + x];
                // dir=1: straight copy (common.cpp:546).
                data[d1 + y * 16 + x] = pix;
                // dir=0: horizontal mirror; col 15 -> 0 (common.cpp:548-552).
                if x == 15 {
                    data[d0 + y * 16 + 15] = 0;
                } else {
                    data[d0 + y * 16 + (14 - x)] = pix;
                }
            }
        }
    }
    SpriteSet { width: 16, height: 16, count, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_cone_offset_table_matches_cpp() {
        // Verbatim against common.cpp:17-21.
        assert_eq!(
            FIRE_CONE_OFFSET,
            [
                [[-3, 1], [-4, 0], [-4, -2], [-4, -4], [-3, -5], [-2, -6], [0, -6]],
                [[3, 1], [4, 0], [4, -2], [4, -4], [3, -5], [2, -6], [0, -6]],
            ]
        );
        // The two directions are x-mirrored in the x component, identical in y
        // (except the shared [0,-6] tail at angle_frame 6).
        for f in 0..7 {
            assert_eq!(
                FIRE_CONE_OFFSET[0][f][0], -FIRE_CONE_OFFSET[1][f][0],
                "x offset is L/R mirrored at angle_frame {f}"
            );
            assert_eq!(
                FIRE_CONE_OFFSET[0][f][1], FIRE_CONE_OFFSET[1][f][1],
                "y offset is shared at angle_frame {f}"
            );
        }
    }

    // A synthetic `large_sprites` bank: 16x16, `count` frames. Frame `9+i` carries
    // a per-frame distinctive column pattern so the mirror can be verified: the
    // pixel at (x,y) of source frame `9+i` is `(i*16 + x + 1)` clamped to u8 (never
    // 0, so index-0 semantics don't hide a miscopy), except we also plant a known
    // value at col 15 to check the col-15 -> 0 rule.
    fn large_bank(count: i32) -> SpriteSet {
        let mut data = vec![0u8; (count as usize) * 256];
        for frame in 0..count as usize {
            for y in 0..16usize {
                for x in 0..16usize {
                    // Distinct, non-zero, deterministic per (frame, x).
                    let v = ((frame * 16 + x + 1) % 255 + 1) as u8;
                    data[frame * 256 + y * 16 + x] = v;
                }
            }
        }
        SpriteSet { width: 16, height: 16, count, data }
    }

    #[test]
    fn build_fire_cone_bank_shape() {
        let large = large_bank(20);
        let fc = build_fire_cone_sprites(&large);
        assert_eq!((fc.width, fc.height), (16, 16), "16x16 sprites");
        assert_eq!(fc.count, 2 * 7, "2*7 = 14 sprites");
        assert_eq!(fc.data.len(), 14 * 256, "14 frames of 256 bytes");
    }

    #[test]
    fn build_fire_cone_dir1_is_straight_copy() {
        let large = large_bank(20);
        let fc = build_fire_cone_sprites(&large);
        // FireConeSprite(i, 1) == large_sprites[9+i], byte-for-byte.
        for i in 0..7usize {
            let dst = fc.sprite(i + 7); // i + dir*7, dir=1
            let src = large.sprite(9 + i);
            assert_eq!(dst, src, "dir=1 frame {i} is a straight copy of large[9+{i}]");
        }
    }

    #[test]
    fn build_fire_cone_dir0_is_horizontal_mirror() {
        let large = large_bank(20);
        let fc = build_fire_cone_sprites(&large);
        // FireConeSprite(i, 0): col 15 -> 0; col x (x<15) -> source col (14-x)?
        // C++: dst[14 - x] = src[x], i.e. dst[j] = src[14 - j] for j in 0..15, and
        // dst[15] = 0.
        for i in 0..7usize {
            let dst = fc.sprite(i); // i + dir*7, dir=0
            let src = large.sprite(9 + i);
            for y in 0..16usize {
                for j in 0..15usize {
                    assert_eq!(
                        dst[y * 16 + j],
                        src[y * 16 + (14 - j)],
                        "dir=0 frame {i} mirror at ({j},{y})"
                    );
                }
                assert_eq!(dst[y * 16 + 15], 0, "dir=0 frame {i} col 15 forced to 0 at row {y}");
            }
        }
    }

    #[test]
    fn build_fire_cone_empty_when_large_lacks_frames() {
        // count < 9+7 -> empty bank (the state.rs:858 empty-bank discipline).
        assert_eq!(build_fire_cone_sprites(&large_bank(15)).count, 0, "count 15 < 16 -> empty");
        assert_eq!(build_fire_cone_sprites(&SpriteSet::default()).count, 0, "default -> empty");
        // Wrong dimensions -> empty too.
        let wrong = SpriteSet { width: 7, height: 7, count: 100, data: vec![0u8; 100 * 49] };
        assert_eq!(build_fire_cone_sprites(&wrong).count, 0, "non-16x16 -> empty");
    }
}
