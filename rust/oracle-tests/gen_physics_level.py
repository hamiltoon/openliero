#!/usr/bin/env python3
# Generates data/TC/openliero/Levels/physics_fall_test.lev — a purpose-built
# fixture for the Slice-2 worm-physics golden.
#
# Why a new level instead of modern_test.lev: the worm collision predicate is
# `!CheckedMatWrap(x,y).Background()` (worm.cpp), so a worm only FALLS through
# pixels whose material has the Background flag (0x08). modern_test.lev's collision
# map uses only materials {0, 12, 168} — NONE flagged Background — so a worm placed
# anywhere in it is embedded in "solid" and never moves. To exercise real falling +
# a bounce (vel.y sign flip), this builds a trivial level: an open sky band (a
# Background-flagged material) above a solid floor (a Dirt material), both indices
# taken from the live data/TC/openliero/tc.cfg so the fixture stays TC-consistent.
#
# Format = OLLEVEL2 sized header the C++ Level::load accepts (level.cpp):
#   "OLLEVEL2" + version(1=0) + width(2 LE) + height(2 LE) + width*height material_id
# No POWERLEVEL/MODERNLV extension blocks (collision map only).
import re
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
TC = ROOT / "data" / "TC" / "openliero"

WIDTH = 504
HEIGHT = 350
FLOOR_Y = 200  # rows [0, FLOOR_Y) are sky; [FLOOR_Y, HEIGHT) are solid floor.


def load_materials():
    txt = (TC / "tc.cfg").read_text()
    m = re.search(r"materials\s*=\s*\[([^\]]*)\]", txt)
    return [int(x) for x in m.group(1).split(",")]


def main():
    flags = load_materials()
    K_BACKGROUND = 1 << 3
    K_DIRT = 1 << 0
    sky = next(i for i, f in enumerate(flags) if f == K_BACKGROUND)  # Background only
    ground = next(i for i, f in enumerate(flags) if f == K_DIRT)  # Dirt only
    assert flags[sky] & K_BACKGROUND and not (flags[ground] & K_BACKGROUND)

    out = bytearray()
    out += b"OLLEVEL2"
    out += bytes([0])  # version
    out += bytes([WIDTH & 0xFF, (WIDTH >> 8) & 0xFF])
    out += bytes([HEIGHT & 0xFF, (HEIGHT >> 8) & 0xFF])
    for y in range(HEIGHT):
        mat = sky if y < FLOOR_Y else ground
        out += bytes([mat]) * WIDTH

    dst = TC / "Levels" / "physics_fall_test.lev"
    dst.write_bytes(out)
    print(f"sky_material={sky} ground_material={ground} floor_y={FLOOR_Y}")
    print(f"wrote {dst} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
