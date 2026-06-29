# Liero-rs — progress at a glance

> Two tracks:
> **🔁 Rewrite** — a faithful port of existing OpenLiero (C++/SDL3) to Rust (+ Bevy),
> proven **bit-for-bit** against the C++ engine as a truth oracle.
> **✨ New** — capabilities the original never had (enabled once the rewrite lands).
>
> The headline % tracks the **rewrite**; the **new** track is exploratory/future.
> The dense machine ledger lives in `.superpowers/sdd/progress.md` (gitignored).
>
> **Last updated:** 2026-06-29 · **Current focus:** Step 2, Slice 5 (remaining object
> families) — **5a splinters SHIPPED** (PR #3) and **5b damage+blood MILESTONE difftest
> GREEN** (`sim_slice5b_golden` matches the C++ master + all 9 components, all 121 ticks;
> a worm is **wounded by an explosion** (health 100→82, survives) and **bleeds** — the
> `bobjects` blood pool goes live for the first time, fed by the blood-trail at the
> faithful `cycles%10` cadence). 5b also turned `cycles` live (dumper + Rust `++cycles`,
> regenerating the prior goldens' master columns) and ported the wobject bounce+animation
> flight branches. **Deferred to a follow-up slice:** the per-pixel `CheckForSpecWormHit`
> + the wobject/nobject *in-flight* worm-hit arms (5b wounds only via the explosion's
> sobject AABB, using the closed-gate weapon `explosives`). Next: 5c bonuses, 5d
> death+respawn.

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
├─ ✅ Slice 4  one weapon, full lifecycle (4a–4d)              SHIPPED
│   ├─ ✅ 4a  projectile lifecycle — fan (RNG goes live)         SHIPPED
│   ├─ ✅ 4b  terrain destruction — greenball / DrawDirtEffect   SHIPPED (level hash live, 91 ticks bit-exact)
│   ├─ ✅ 4c  explosion sobjects + nobjects — dart               SHIPPED (sobjects/nobjects live + carving, 91 ticks bit-exact)
│   └─ ✅ 4d  slice-3/4 deferrals (dig, reload, shell-drop+land, load_change)  SHIPPED (handgun, master+9 components bit-exact 126 ticks vs C++)
├─ 🔄 Slice 5  remaining object families — decomposed 5a–5d
│   ├─ ✅ 5a  splinters (cannon → medium_explosion + 5 splinters)  SHIPPED (PR #3, 131 ticks bit-exact)
│   ├─ ✅ 5b  worm damage + blood (O10)  MILESTONE GREEN (explosives wound → blood → live bobjects, 121 ticks; cycles live; await broad review + push)
│   ├─ ⬜ 5c  bonuses (CreateBonus + bonus-drop roll + chain-loop)
│   └─ ⬜ 5d  death + respawn (BeginRespawn RNG-search; fuzzed)
├─ ⬜ Slice 5′ (deferred follow-up)  per-pixel CheckForSpecWormHit + wobject/nobject in-flight worm-hit arms
└─ ⬜ Slice 6  full ProcessFrame + game modes + >1000-tick fuzz match
```

| Level | Done |
|---|---|
| Rewrite track (steps 0–5) | **~43–50%** |
| Step 2 (current) | **~70–73%** |
| Slice 5b (worm damage + blood) | **✅ MILESTONE GREEN** (`sim_slice5b_golden` master+9 components 121 ticks; worm wounded 100→82 + bleeds, **`bobjects` pool live**; `cycles` now advances; wobject bounce+animation flight branches ported; per-pixel worm-hit deferred → follow-up; await broad review + push) |
| Slice 5a (splinters) | **✅ SHIPPED** (`sim_slice5a_golden` master+9 components 131 ticks, debug+release; `BlowUpObject` splinter arm + `NObject::Process` `create_on_exp`/explode arms live; on PR #3) |
| Slice 4 (weapon lifecycle) | **✅ SHIPPED** (4a + 4b + 4c + 4d all bit-exact vs C++) |
| Slice 4c | **✅ SHIPPED** (sobjects/nobjects pools live + carving DrawDirtEffect, master+9 components bit-exact 91 ticks vs C++, on PR #3) |
| Slice 4d | **✅ SHIPPED** (dig + shell-drop/landing-blit + reload + load_change; HANDGUN, master+9 components bit-exact 126 ticks vs C++; `BlitImageOnMap` + small-sprite bank added) |

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
