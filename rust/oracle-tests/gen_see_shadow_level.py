#!/usr/bin/env python3
# Generates data/TC/openliero/Levels/see_shadow_test.lev — a purpose-built fixture
# for the Slice-3b SHADOW-PASS render golden (render_slice3b_shadow).
#
# Why a new level instead of physics_fall_test.lev: the draw-time shadow pass
# (viewport.cpp:274-398) only paints a shadow pixel where the UNDERLYING level
# material carries kSeeShadow (material.hpp:11, 1<<4). physics_fall_test.lev is built
# from exactly two materials — sky = palette 130 (material 8 = kBackground only) and
# floor = palette 12 (material 1 = kDirt only) — NEITHER of which is kSeeShadow, so a
# shadow cast onto it paints NOTHING (ShadowedArgb returns 0 everywhere) and the
# shadow pass is unprovable there. This level swaps the sky band for the openliero
# TC's only kSeeShadow material (palette 160..163 → material value 24 = kSeeShadow |
# kBackground): a background the worm still falls through, but onto which its shadow
# silhouette DOES paint. A worm dropped into the band therefore casts a real,
# hash-moving shadow every tick — the empirical proof the brief (spec O3) requires.
#
# Layout (504x350, matching physics_fall_test geometry so the framehash viewports
# centre identically): rows [0, FLOOR_Y) are the kSeeShadow background band; rows
# [FLOOR_Y, HEIGHT) are a solid kDirt floor so the worms stay in-level. The band is
# tall enough (FLOOR_Y=340) that both a FALLING worm (mid-band) and a settled worm
# (feet near the floor, its shadow silhouette reaching up into the band) cast shadow
# pixels — so the shadow is painted on every dumped tick, not just one.
#
# Format = OLLEVEL2 sized header the C++ Level::load accepts (level.cpp):
#   "OLLEVEL2" + version(1=0) + width(2 LE) + height(2 LE) + width*height material_id
# No POWERLEVEL/MODERNLV extension blocks (collision map only). Material indices are
# taken from the LIVE data/TC/openliero/tc.cfg so the fixture stays TC-consistent.
import re
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
TC = ROOT / "data" / "TC" / "openliero"

WIDTH = 504
HEIGHT = 350
FLOOR_Y = 340  # rows [0, FLOOR_Y) are the kSeeShadow band; [FLOOR_Y, HEIGHT) floor.


def load_materials():
    txt = (TC / "tc.cfg").read_text()
    m = re.search(r"materials\s*=\s*\[([^\]]*)\]", txt)
    return [int(x) for x in m.group(1).split(",")]


def main():
    flags = load_materials()
    K_DIRT = 1 << 0
    K_BACKGROUND = 1 << 3
    K_SEE_SHADOW = 1 << 4
    # SeeShadow band: the first palette index whose material sets kSeeShadow (and, in
    # this TC, kBackground too — value 24 — so the worm falls through it as sky).
    band = next(i for i, f in enumerate(flags) if (f & K_SEE_SHADOW))
    # Solid floor: a Dirt-only (non-Background) material so the worm lands on it.
    ground = next(i for i, f in enumerate(flags) if f == K_DIRT)
    assert flags[band] & K_SEE_SHADOW and flags[band] & K_BACKGROUND
    assert not (flags[ground] & K_BACKGROUND)

    out = bytearray()
    out += b"OLLEVEL2"
    out += bytes([0])  # version
    out += bytes([WIDTH & 0xFF, (WIDTH >> 8) & 0xFF])
    out += bytes([HEIGHT & 0xFF, (HEIGHT >> 8) & 0xFF])
    for y in range(HEIGHT):
        mat = band if y < FLOOR_Y else ground
        out += bytes([mat]) * WIDTH

    dst = TC / "Levels" / "see_shadow_test.lev"
    dst.write_bytes(out)
    print(f"see_shadow_band={band} (mat {flags[band]}) ground={ground} floor_y={FLOOR_Y}")
    print(f"wrote {dst} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
