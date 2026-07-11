#!/usr/bin/env python3
# Generates data/TC/openliero/Levels/render_stage.lev — the Slice-3b SPRITE-family
# render stage (fan / dart / blood / laser / shake goldens).
#
# Why a new level instead of physics_fall_test.lev: the reduced framehash dumper wires
# two Viewports whose worm has killed_timer == kKilledTimerInitial (150) and health 100
# for the whole run — ResetWorms sets it and the ALIVE worm branch never decrements it
# (worm.cpp:439 lives in the DEAD branch). So Viewport::Process (viewport.cpp:28) takes
# NEITHER the `killed_timer <= 0` centering arm NOR the `health <= 0` arm — the camera
# stays at its constructed origin (x=0, y=0) for every tick (the Rust viewport::process,
# viewport.rs:84, mirrors this exactly, so both sides agree). The VISIBLE WORLD WINDOW is
# therefore FIXED at world [0,158) x [0,158): viewport 0 maps it to screen x[0,158),
# viewport 1 (kOffs.x = 160) maps the SAME world box to screen x[160,318). physics_fall_
# test.lev's floor is at y=200 — BELOW that window — so a grounded worm (y≈196) and its
# shots never appear on screen (the frame is uniform sky, byte-identical to 3a). This
# level puts the floor HIGH (FLOOR_Y=120) so a grounded, stationary worm at y≈116 and its
# projectiles/explosions/blood all fall INSIDE the visible world box and actually draw.
#
# Layout (504x350, standard geometry): rows [0, FLOOR_Y) sky (Background mat 130); rows
# [FLOOR_Y, HEIGHT) solid Dirt floor (mat 12) — same two TC materials as physics_fall_
# test, just a higher horizon so the action is on-screen. Material indices are read from
# the LIVE tc.cfg so the fixture stays TC-consistent.
#
# Format = OLLEVEL2 (level.cpp): "OLLEVEL2" + version(0) + width(2 LE) + height(2 LE) +
# width*height material_id. Collision map only (no POWERLEVEL/MODERNLV blocks).
import re
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
TC = ROOT / "data" / "TC" / "openliero"

WIDTH = 504
HEIGHT = 350
FLOOR_Y = 120  # rows [0,120) sky (in-window); [120,350) floor. Grounded worm y≈116.


def load_materials():
    txt = (TC / "tc.cfg").read_text()
    m = re.search(r"materials\s*=\s*\[([^\]]*)\]", txt)
    return [int(x) for x in m.group(1).split(",")]


def main():
    flags = load_materials()
    K_BACKGROUND = 1 << 3
    K_DIRT = 1 << 0
    sky = next(i for i, f in enumerate(flags) if f == K_BACKGROUND)
    ground = next(i for i, f in enumerate(flags) if f == K_DIRT)
    assert flags[sky] & K_BACKGROUND and not (flags[ground] & K_BACKGROUND)

    out = bytearray()
    out += b"OLLEVEL2"
    out += bytes([0])  # version
    out += bytes([WIDTH & 0xFF, (WIDTH >> 8) & 0xFF])
    out += bytes([HEIGHT & 0xFF, (HEIGHT >> 8) & 0xFF])
    for y in range(HEIGHT):
        mat = sky if y < FLOOR_Y else ground
        out += bytes([mat]) * WIDTH

    dst = TC / "Levels" / "render_stage.lev"
    dst.write_bytes(out)
    print(f"sky={sky} ground={ground} floor_y={FLOOR_Y}")
    print(f"wrote {dst} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
