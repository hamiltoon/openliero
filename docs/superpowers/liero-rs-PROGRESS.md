# Liero-rs rewrite — progress at a glance

> Human-facing status snapshot. Updated at milestones (a slice shipped + pushed).
> The dense machine ledger lives in `.superpowers/sdd/progress.md` (gitignored).
>
> **Last updated:** 2026-06-28 · **Current focus:** Step 2, Slice 4b — Task 1 (`draw_dirt_effect`, the core port)

## Overall: ~35–45%

The rewrite ports OpenLiero (C++/SDL3) to Rust (+ Bevy later), strangler-style:
the C++ engine is the truth oracle, and every piece is differential-tested
**bit-for-bit** against it before moving on. Steps 0–1 are merged; the
deterministic sim core (step 2, the hardest part) is roughly half done; the
Bevy-facing steps 3–5 are not started.

```
HELA REWRITEN (steg 0–5)                                    ~35–45%
├─ ✅ Step 0  sim-core primitives (RNG/fixed/vec/math/tables)   DONE — merged (PR #1)
├─ ✅ Step 1  asset IO, slices 1a–1e (level/palette/sprites/    DONE — merged (PR #2)
│             tc.cfg/objects/WAV)
├─ 🔄 Step 2  deterministic sim core (PR #3)                    ~50–55%   ◀── YOU ARE HERE
├─ ⬜ Step 3  Bevy rendering / window                           not started
├─ ⬜ Step 4  interactive iteration / input / replay+regression not started
└─ ⬜ Step 5  netplay / web (wasm + WebRTC)                     not started
```

## Step 2 — deterministic sim core (~50–55%)

Six slices, each differential-tested against a per-tick `HashGameState` /
`HashGameComponents` oracle.

```
├─ ✅ Slice 1  Level → sim-state + state-hash harness (tick 0)
├─ ✅ Slice 2  one worm, physics only
├─ ✅ Slice 3  worm control + aiming (master hash turns on)
├─ 🔄 Slice 4  one weapon, full lifecycle (4a–4d)              ~30%   ◀── HERE
│   ├─ ✅ 4a  projectile lifecycle — fan (RNG goes live)         SHIPPED
│   ├─ 🔄 4b  terrain destruction — greenball / DrawDirtEffect   ◀── HERE (~12%)
│   │         (the level hash becomes a time series)            T0✅ done, T1 of 8 next
│   ├─ ⬜ 4c  explosion sobjects + nobjects — dart
│   └─ ⬜ 4d  slice-3/4 deferrals (dig body, reload, shell, …)
├─ ⬜ Slice 5  remaining object families (nobjects/sobjects/blood/bonuses)
└─ ⬜ Slice 6  full ProcessFrame + game modes + >1000-tick fuzz match
```

## Roughly, by level

| Level | Done |
|---|---|
| Whole rewrite (steps 0–5) | **~35–45%** |
| Step 2 (current) | **~50–55%** |
| Slice 4 (weapon lifecycle) | **~30%** |
| Slice 4b (current) | **~12%** (T0 datamodel done + reviewed; 7 tasks left) |

## What's left

- **Step 2:** finish slice 4 (4b terrain, 4c explosion objects, 4d deferrals),
  then slice 5 (object families) and slice 6 (full-frame integration, game
  modes, the >1000-tick fuzz match that mirrors `test_determinism.cpp`).
- **Steps 3–5:** Bevy rendering, an interactive run/observe/iterate loop with
  replay + checksum regression, then netplay/web. Large in volume but lower
  risk — they sit on top of a sim already proven bit-exact.

## How it's built

Subagent-driven: per-task implement + two-stage review (spec + quality), a broad
whole-slice review before each push, all bit-exact vs the C++ oracle. PR #3
accumulates all of step 2 and is merged only when the whole step is complete.
