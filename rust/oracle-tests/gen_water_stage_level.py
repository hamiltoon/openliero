#!/usr/bin/env python3
# Generates data/TC/openliero/Levels/water_stage.lev — the Slice-3b POSITIVE
# BlitImageR (sobject-over-water) render fixture (render_slice3b_dart_water golden).
#
# Why a new level rather than reusing render_stage.lev or see_shadow_test.lev:
#   * render_stage.lev's sky is palette 130 (a *non*-water background). BlitImageR
#     (blit.cpp:344-368) blits a sobject-explosion sprite pixel ONLY where the
#     UNDERLYING level pixel's palette index is in the classic WATER range [160,168)
#     (shadow.PixelAt gate). Over palette 130 the gate is always false, so the dart's
#     small_explosion sobject on render_stage paints ZERO pixels — that scenario proves
#     only BlitImageR's NEGATIVE path (its frame motion comes from nobject debris +
#     terrain carve, NOT from the sobject sprite).
#   * see_shadow_test.lev DOES carry water-range pixels (its kSeeShadow band is palette
#     160-163, value 24 = kSeeShadow | kBackground, inside [160,168)), BUT that band is
#     background all the way down to its floor at y=340 — far BELOW the fixed framehash
#     window [0,158). A DART only explodes on DirtRock terrain (weapon.cpp:249-252,
#     explGround; worm hits do NOT explode it — wormCollide=false), and see_shadow's only
#     DirtRock is that y=340 floor. So a dart on see_shadow can only explode OUT of the
#     window — its sobject is never drawn. A positive proof needs DirtRock the dart can
#     hit INSIDE the window, with water-range pixels DIRECTLY ABOVE it so the 16x16
#     explosion sprite (centred on the impact, sobject.cpp:36-37 obj.{x,y}=impact-8)
#     straddles the boundary and its upper rows fall on water.
#
# This level is render_stage.lev geometry EXACTLY (504x350, dirt floor at FLOOR_Y=120 so
# a grounded worm at y≈116 and the dart's floor-impact are inside the [0,158) window) with
# ONE change: the sky band [0,120) is a WATER-range background (palette 160, the first
# [160,168) index that is Background) instead of palette 130. The dart flies through the
# water sky exactly as it flew through the render_stage sky (both are Background, neither
# is DirtRock — identical collision/gravity => byte-identical sim), then explodes on the
# SAME dirt floor at y≈120. The small_explosion sobject sprite, centred on the impact,
# straddles the water/dirt boundary: its rows < 120 land on palette-160 water and are
# painted by BlitImageR; its rows >= 120 land on palette-12 dirt (out of range) and are
# skipped. That upper-half water paint is the POSITIVE BlitImageR proof.
#
# Because ONLY the sky palette differs from render_stage (160 vs 130) and the sky is never
# collided-with or carved, the dart's physics are identical to render_slice3b_dart: the
# sim sidecar's worm/sobject/nobject/bobject columns match render_slice3b_dart_sim.txt
# tick-for-tick (only the `level` column differs — different sky pixels hash differently).
# The over-water frame hashes therefore differ from the outside-band render_slice3b_dart
# frames in the explosion window: same debris + carve, PLUS the sobject sprite BlitImageR
# now paints — the differential the corpus was missing.
#
# Format = OLLEVEL2 (level.cpp): "OLLEVEL2" + version(0) + width(2 LE) + height(2 LE) +
# width*height material_id. Collision map only. Material indices come from the LIVE tc.cfg
# so the fixture stays TC-consistent.
import re
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
TC = ROOT / "data" / "TC" / "openliero"

WIDTH = 504
HEIGHT = 350
FLOOR_Y = 120  # rows [0,120) water sky (in-window); [120,350) dirt floor. Worm y≈116.


def load_materials():
    txt = (TC / "tc.cfg").read_text()
    m = re.search(r"materials\s*=\s*\[([^\]]*)\]", txt)
    return [int(x) for x in m.group(1).split(",")]


def main():
    flags = load_materials()
    K_BACKGROUND = 1 << 3
    K_DIRT = 1 << 0
    # Water sky: the first palette index in the classic BlitImageR water range [160,168)
    # that is a Background material (so the dart/worm pass through it exactly as through a
    # normal sky). In the openliero TC that is 160 (value 24 = kSeeShadow | kBackground).
    water = next(i for i in range(160, 168) if flags[i] & K_BACKGROUND)
    ground = next(i for i, f in enumerate(flags) if f == K_DIRT)
    assert 160 <= water < 168 and flags[water] & K_BACKGROUND
    assert not (flags[ground] & K_BACKGROUND)

    out = bytearray()
    out += b"OLLEVEL2"
    out += bytes([0])  # version
    out += bytes([WIDTH & 0xFF, (WIDTH >> 8) & 0xFF])
    out += bytes([HEIGHT & 0xFF, (HEIGHT >> 8) & 0xFF])
    for y in range(HEIGHT):
        mat = water if y < FLOOR_Y else ground
        out += bytes([mat]) * WIDTH

    dst = TC / "Levels" / "water_stage.lev"
    dst.write_bytes(out)
    print(f"water_sky={water} (mat {flags[water]}) ground={ground} floor_y={FLOOR_Y}")
    print(f"wrote {dst} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
