# Liero-rs — progress at a glance

> Two tracks:
> **🔁 Rewrite** — a faithful port of existing OpenLiero (C++/SDL3) to Rust (+ Bevy),
> proven **bit-for-bit** against the C++ engine as a truth oracle.
> **✨ New** — capabilities the original never had (enabled once the rewrite lands).
>
> The headline % tracks the **rewrite**; the **new** track is exploratory/future.
> The dense machine ledger lives in `.superpowers/sdd/progress.md` (gitignored).
>
> **Last updated:** 2026-06-29 · **Current focus:** Step 2, Slice 4d (Slice-3 deferrals), T7/9 done (golden generated); T8 milestone difftest next

---

## 🔁 Rewrite track — faithful port (~35–45%)

Strangler-style: the C++ engine is the oracle, every piece differential-tested
bit-for-bit before moving on. Steps 0–1 merged; the deterministic sim core
(step 2, the hardest part) is ~half done; the Bevy-facing steps 3–5 not started.

```
REWRITE (steg 0–5)                                          ~35–45%
├─ ✅ Step 0  sim-core primitives (RNG/fixed/vec/math/tables)   DONE — merged (PR #1)
├─ ✅ Step 1  asset IO, slices 1a–1e (level/palette/sprites/    DONE — merged (PR #2)
│             tc.cfg/objects/WAV)
├─ 🔄 Step 2  deterministic sim core (PR #3)                    ~50–55%   ◀── YOU ARE HERE
├─ ⬜ Step 3  Bevy rendering / window (reproduce the SDL3 view) not started
├─ ⬜ Step 4  input + replay (.lrp) playback                    not started
└─ ⬜ Step 5  native netplay (ENet + rollback + Go relay)       not started
```

> Everything above EXISTS in the original openliero — this track reproduces it in
> Rust (new engine/idioms = modernization, not new game functionality).

### Step 2 — deterministic sim core (~50–55%)

Six slices, each differential-tested against a per-tick `HashGameState` /
`HashGameComponents` oracle.

```
├─ ✅ Slice 1  Level → sim-state + state-hash harness (tick 0)
├─ ✅ Slice 2  one worm, physics only
├─ ✅ Slice 3  worm control + aiming (master hash turns on)
├─ 🔄 Slice 4  one weapon, full lifecycle (4a–4d)              ~75%   ◀── HERE
│   ├─ ✅ 4a  projectile lifecycle — fan (RNG goes live)         SHIPPED
│   ├─ ✅ 4b  terrain destruction — greenball / DrawDirtEffect   SHIPPED (level hash live, 91 ticks bit-exact)
│   ├─ ✅ 4c  explosion sobjects + nobjects — dart               SHIPPED (sobjects/nobjects live + carving, 91 ticks bit-exact)
│   └─ 🔄 4d  slice-3/4 deferrals (dig body, reload, shell, …)   ◀── HERE (T7/9 done — golden generated; T8 milestone)
├─ ⬜ Slice 5  remaining object families (nobjects/sobjects/blood/bonuses)
└─ ⬜ Slice 6  full ProcessFrame + game modes + >1000-tick fuzz match
```

| Level | Done |
|---|---|
| Rewrite track (steps 0–5) | **~38–45%** |
| Step 2 (current) | **~60–63%** |
| Slice 4 (weapon lifecycle) | **~75%** (4a + 4b + 4c shipped; 4d remains) |
| Slice 4c | **✅ SHIPPED** (sobjects/nobjects pools live + carving DrawDirtEffect, master+9 components bit-exact 91 ticks vs C++, on PR #3) |

---

## ✨ New capabilities — beyond the original (not started; future)

These do **not** exist in openliero today. They become possible once the rewrite
gives a clean, deterministic, embeddable Rust sim. Tracked separately — they do
not count toward the rewrite %.

```
✨ NEW
├─ ⬜ RL / self-play training (local, M2 Max)        needs the sim (step 2) first
├─ ⬜ Web / wasm build (browser reach)               new platform
├─ ⬜ WebRTC P2P netplay (serverless, lightweight)   ← NEW (original uses ENet + a Go relay)
└─ ⬜ Agent-driven dev harness                        new tooling
      (run/observe/iterate + headless screenshots
       + checksum-regression gate)
```

> Note: native ENet netplay, rollback, replay (.lrp), and video export already
> exist in openliero → those live in the **rewrite** track above, not here.
> "New" is specifically web/wasm + WebRTC, RL/self-play, and the agentic dev loop.

---

## How it's built

Subagent-driven: per-task implement + two-stage review (spec + quality), a broad
whole-slice review before each push, all bit-exact vs the C++ oracle. PR #3
accumulates all of step 2 and is merged only when the whole step is complete.
