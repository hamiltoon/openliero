# Step 4, Slice 4d — Live shake / flash / banners (`ProcessViewports` port): design

Status: **DESIGN — slice 4d** · 2026-07-13 · branch `liero-rs-step-4` (accumulating Step-4 PR)
Part of: `2026-06-26-liero-rs-roadmap.md`
Built on: `2026-07-12-liero-rs-step4-input-replay-overview.md` (cited **overview §N**) and
`2026-07-12-liero-rs-step4-cpp-input-replay-map.md` (cited **input-map §N**)
Sibling precedent: the Step-3 render slices (3a/3b/3e) and the 4a/4b input slices.
Companion plan: `plans/2026-07-13-liero-rs-step4-slice4d-plan.md`.

This is the per-slice design for wiring the render-only viewport side effects Step 3 deferred:
top-of-frame `screen_flash` / `shake` / `banner_y` stepping, `ProcessViewports` centering, the
viewport-local-RNG shake, and death banners — made **live** (driven by real worm/explosion events)
rather than injected by the 3b `render_shake` / `render_flash` dumper directives.

---

## 0. The headline correction (read this first)

The overview and input-map §7a/§8 both call `screen_flash` **"hashed sim state."** That is
**wrong**, and the correction reshapes the whole slice:

- **C++ `HashGameState` does NOT fold `screen_flash`** (`src/game/stateHash.hpp:15-113` — the full
  field list; `screen_flash` is absent). It lives in `GameSnapshot` for **rollback** only
  (`game.cpp:693` save / `game.cpp:762` load), not in the determinism hash.
- **Rust does not carry `screen_flash` at all.** `SimState` (`rust/sim/src/state.rs:907-`) has no
  such field, and the sobject-explosion write is explicitly omitted
  (`rust/sim/src/sobject.rs:146`: "`:27-33 viewport shake + :41 screen_flash: render-only, no rand —
  omitted`").

**Consequence:** making flash live means **adding `screen_flash` to `SimState`** (a new field + a
top-of-frame decrement + the sobject-create write), but because it is **not** in `hash_game_state`,
this is a **hash-neutral** sim change — proven by a **re-diff** gate (every `sim_slice*` golden
byte-identical), **not** a re-fuzz. This is the elegant path the overview's "already-hashed" framing
obscured. `shake` and `banner_y` are viewport-only and never touch the sim hash (input-map §7,
correctly).

---

## 1. Exact C++ tick order (`Game::ProcessFrame`, `game.cpp:267-471`)

One `ProcessFrame` fuses six phases; flash/shake/banner touch three of them:

1. **Top-of-frame decrements** (`game.cpp:271-332`), using the **previous** tick's values and
   **pre-`++cycles`** `cycles`:
   - `if (screen_flash > 0) --screen_flash` (`:271-273`) — **sim scalar**, drives the palette
     `LightUp` at draw (`game.cpp:179-181`).
   - per player+spectator viewport `if (shake > 0) shake -= 4000` (`:275-285`) — **viewport field**.
   - **banner walk, only `(cycles & 1) == 0`** (`:292-332`): `banner_y` steps toward `2` when that
     viewport's worm `killed_timer > 16`, else toward `-8`. Reads `killed_timer` **before** the worm
     loop runs this tick (i.e. the previous tick's value). ⚠ every-**other**-cycle.
2. **Object loops** — bonuses, then `sobjects`/`wobjects`/`nobjects`/`bobjects` `Process`
   (`:287-355`). An explosion **sobject is created here** and, at creation
   (`sobject.cpp:27-33,41`), sets `v.shake = max(Itof(type.shake), v.shake)` for every viewport whose
   rect contains the blast `(x,y)`, and `game.screen_flash = max(type.flash, screen_flash)`.
3. `++cycles` (`:357`); bonus-drop roll; **worm loop** `Process` (`:364-370`) — mutates
   `killed_timer`, `pos`, `health`, and (for steerable weapons) `steerable_sum_*`/`steerable_count`.
4. game-mode timers (`:372-461`).
5. **`ProcessViewports()`** (`game.cpp:463` → `game.cpp:132-139` → `Viewport::Process`,
   `viewport.cpp:22-76`): centering + shake-RNG + clamp, reading **post-worm-loop** worm state.
6. `prev_control_states = control_states` (`:466-468`).

**The only `shake` reader is phase 5's RNG.** So the value that matters at render time is
`shake_prev −4000 (clamped >0)  then  max(explosion)` — decrement (phase 1) strictly before the
max (phase 2), with no reader in between. That single fact is what lets the Rust render/game layer
reproduce it without literally interleaving with the sim (§3).

`Viewport::Process` (`viewport.cpp:22-76`):
- **Centering** (`:28-45`): alive+visible → `SetCenter(steerable centroid)` if `steerable_count>0`
  else `SetCenter(Ftoi(pos))`; invisible → `ScrollTo(Ftoi(pos), 4)`; dead (`killed_timer>0 &&
  health<=0`) → `SetCenter(Ftoi(pos))` and, at `killed_timer==kKilledTimerInitial` (150,
  `worm.hpp:243`), `banner_y = -8`.
- **Shake RNG** (`:47-52`): `kRealShake = Ftoi(shake); if (kRealShake>0) { x += rand(kRealShake*2)−
  kRealShake; y += rand(kRealShake*2)−kRealShake; }` using the **viewport-local** `Rand`
  (`viewport.hpp:33`), never `game.rand` (input-map §7b).
- **Clamp** (`:54-57`) to `[0, max]`.

The banner **draw** (text) is in `Viewport::Draw` (`viewport.cpp:249-270`): own-worm `YoureIt`
(GameOfTag only, `:249-254`) and cross-viewport `KilledMsg`/`CommittedSuicideMsg` for every other
dead worm (`:256-270`), keyed by `last_killed_by_idx`.

---

## 2. What already exists on the Rust side

- `render::viewport::Viewport` (`rust/render/src/viewport.rs`) already has `shake`, `banner_y`, a
  default-seeded viewport-local `rand`, `set_center`/`scroll_to`, and a `process()` that ports
  `viewport.cpp:22-57` **including the shake-RNG branch** (`:98-102`) and the `killed_timer ==
  KILLED_TIMER_INITIAL → banner_y = -8` reset (`:93-95`). It carries a
  `debug_assert_eq!(worm.steerable_count, 0, ...)` guard on the visible-alive arm (`:86`).
- `render::frame::Scene` carries `screen_flash: i32` and `frame::draw` feeds it into
  `build_palette` (`rust/render/src/frame.rs:40,57-60`) — the `LightUp` path is **already live**;
  it is simply fed a hardcoded `0` today.
- `game/src/main.rs` renders via `demo.scene.as_scene(0, draw_shadow)` (`main.rs:497`) — the `0` is
  the **3c hardcoded `screen_flash`**, and `render_and_upload` calls `Viewport::process` per viewport
  inside `frame::draw`. There is **no** top-of-frame decrement, no banner walk, no explosion-shake
  application anywhere in the game loop yet.
- The 3b **injection** path (dumper `render_shake`/`render_flash`; Rust oracle-test
  `viewports[vp].shake = itof(amount)` set-before/restore-after,
  `rust/oracle-tests/tests/render_slice3b_common/mod.rs:151-185`) stays exactly as-is.

So 4d is: **(a)** add `screen_flash` to the sim; **(b)** emit an explosion-shake event; **(c)** add
the top-of-frame stepping + event application to the **game layer** and thread live `screen_flash`
into the render; **(d)** cut a live golden via a new dumper path.

---

## 3. Where the Rust stepping lives, and the bit-exactness argument

Three homes, matching what owns each datum:

### 3a `screen_flash` → the **sim** (`SimState`, hash-neutral)

Add `pub screen_flash: i32` to `SimState` (init 0). Decrement at the **top** of `process_frame`
(`state.rs:1431`, before the object loops), and write it in the sobject-create explosion path
(`sobject.rs:146`, replacing the "omitted" comment) as `screen_flash = max(ty.flash, screen_flash)`.
**Do not add it to `hash_game_state`.** Because C++ `HashGameState` omits it too, every `sim_slice*`
golden regenerates byte-identical → **re-diff, no re-fuzz**. The render reads `sim.screen_flash`
directly.

### 3b `shake` → the **game/render layer**, fed by a sim-emitted explosion event

`shake` is per-viewport and position-dependent (which viewport rects contain the blast), so it
**cannot** be a sim scalar and the viewport concept does not exist inside the firewall. The clean
seam — the same shape as 4c's audio event stream, but **independent** (4c and 4d are mutually
independent per the overview) — is: the sim **emits a per-tick explosion-shake event** `(x, y,
amount)` at sobject creation when `ty.shake > 0`, into a `Vec` on `SimState` drained by the game
layer. No rand, no hashed state → hash-neutral.

### 3c `banner_y` step + centering → the **game/render layer**

`banner_y` walk (phase 1) and the `ProcessViewports` centering/reset (phase 5) are already viewport
state; they move to the live game loop.

### The ordering the game loop must run (the bit-exactness core)

C++ interleaves decrement (phase 1) → explosion-max (phase 2) → RNG-read (phase 5) inside one
`ProcessFrame`. Rust runs `process_frame` as one atomic call. Because **the only `shake` reader is
phase 5**, and the banner walk + decrement read **pre-worm-loop** `killed_timer`/`cycles` while the
centering reads **post-worm-loop** state, the game tick reproduces C++ exactly by splitting around
`process_frame`:

```
// tick N, in game/src/main.rs tick_and_render, BEFORE process_frame
// (reads pre-tick sim state: previous shake, pre-++ cycles, pre-worm-loop killed_timer):
for vp in viewports: if vp.shake > 0 { vp.shake -= 4000 }
if (sim.cycles & 1) == 0:
    for vp: step vp.banner_y toward (worm[vp.worm_idx].killed_timer > 16 ? 2 : -8)

sim.process_frame(inputs)   // phase 1 screen_flash-- + phases 2-6; emits explosion-shake events

// apply explosion-shake events (C++ phase 2; no shake reader ran in between):
for (x, y, amount) in sim.drain_shake_events():
    for vp where vp rect contains (x, y): vp.shake = max(itof(amount), vp.shake)

// render: frame::draw runs Viewport::process (phase 5 centering + shake-RNG + clamp),
// reading sim.screen_flash for the palette LightUp.
render_and_upload(...)   // as_scene(sim.screen_flash, draw_shadow) — replaces the 3c hardcoded 0
```

- `screen_flash` decrement is **inside** `process_frame` (phase 1), so the game layer never touches
  it — it just reads the post-tick value for the palette. Correct because C++ decrements at phase 1
  and reads at draw.
- `shake` decrement is done **before** `process_frame` using the previous value, then max'd with the
  drained events **after** — reproducing "phase-1 decrement then phase-2 max" with no intervening
  reader. Bit-exact.
- `banner_y` walk uses `sim.cycles` (pre-`++`, matching C++ phase 1 reading pre-`++cycles`) and the
  **pre**-worm-loop `killed_timer`. The `ProcessViewports` `banner_y = -8` reset (in
  `Viewport::process`) uses the **post**-worm-loop `killed_timer` — automatically correct because it
  runs in the render after `process_frame`.

⚠ **RNG note:** because live `shake` is now nonzero on flash ticks, the viewport-local `rand` draws
two words per shaking viewport per tick (`viewport.cpp:50-51`). The same `rand` already serves laser
sparks (3b); the draw **order** (centering has no draws; shake draws x then y) must match C++, which
`Viewport::process` already encodes (`viewport.rs:100-101`). No new RNG, no `game.rand` touch.

---

## 4. Camera / `SetCenter` and the `killed_timer` trap

The 3b/3c deferral note (`liero-rs-PROGRESS.md:356-372`) records that centering is **structurally
unreachable** in the existing corpus: `ResetWorms` (`game.cpp:155-166`) starts every worm
`visible=false`, `health=full`, `killed_timer = kKilledTimerInitial (150)`. With `killed_timer=150`,
`Viewport::Process`'s first arm (`killed_timer <= 0`) is skipped and the second (`health <= 0`) is
skipped too — **no `SetCenter` ever runs**, the camera stays at world origin `(0,0)`. That is why the
reduced dumper ships a high-floor `render_stage.lev` (keeps grounded worms inside the fixed `[0,158)`
framehash window) and never re-centres.

**What 4d changes:** to make centering **live and gateable**, the 4d golden scenario must actually
**spawn a worm to `visible=true, killed_timer<=0`** — i.e. press Fire to trigger
`BeginRespawn`/`DoRespawning` (the 5d path: `visible→true`, `killed_timer` counts to 0). Only then
does the alive-visible arm fire and `SetCenter(Ftoi(pos))` move the camera. The design therefore
**requires a spawn in the 4d scenario**, not a static demo. The dead-worm arm
(`killed_timer>0 && health<=0`) is reached by the 3e-style death path and exercises the
`banner_y=-8` reset.

The `debug_assert_eq!(worm.steerable_count, 0)` in `viewport.rs:86` becomes a **live guard**: it now
protects the deferred steerable-centering arm at runtime, not just in 3a tests. Keep it (see §6).

---

## 5. Golden strategy (the crux — a C++ dumper change is required)

**The reduced dumper cannot produce a live flash/shake/banner/centering golden.** `oracle_dump_sim_physics`
(`src/tools/oracle_dump/sim_physics_dump.cpp`) deliberately runs only the **ProcessFrame TAIL** and
**never** calls `Game::ProcessFrame`, **never** wires viewports into `ProcessViewports`, and
**injects** shake/flash via `render_shake`/`render_flash` directives around a single draw
(`:183-266,462-505`). Its viewports are never registered, so sobject-create never sets their `shake`,
centering never runs, and `banner_y` never walks.

Two facts follow:

1. **The injection path stays for the 3b goldens.** They keep passing byte-identical (the directive
   handling is untouched). We do **not** delete or regenerate the 3b corpus.
2. **4d needs a new, opt-in dumper path** — call it `render_live` — that, for its scenario only:
   (a) registers the two player viewports via `AddViewport`, (b) runs the **real
   `game.ProcessFrame()`** for each tick (so sobject-create sets viewport `shake` + `screen_flash`,
   the top-of-frame decrements run, and `ProcessViewports` centers + shakes + walks `banner_y`
   naturally), (c) draws the world block + banners. This produces a sidecar whose flash/shake/
   banner/centering evolved from **real explosions and a real spawn/death**, exactly what the Rust
   live path must reproduce.

**Because `render_live` is opt-in (only the 4d scenario sets it), no prior scenario's dumper output
changes** — but the dumper **binary** changes, so a **mandatory re-diff gate** applies: rerun every
`gen_*` script and assert every committed `sim_slice*`, `render_slice3a/3b/3e` golden is
**byte-identical**. This is the 3b/3e re-diff discipline (`liero-rs-PROGRESS.md:92`).

**Investigation for the implementer (guarded by the re-diff gate):** the reduced dumper avoids
`ProcessFrame` to keep its master hash column matching the 11-column `_sim.txt` record. Calling real
`ProcessFrame` in the `render_live` path is expected to be sim-neutral (viewports do not affect
hashed state; `screen_flash` is unhashed), and the 4d scenario gets its **own** `_sim.txt` produced
by the same run, so it is self-consistent. If real `ProcessFrame` perturbs the master column for this
scenario (e.g. a `CreateBonus` roll or game-mode timer the reduced tail handled differently), fall
back to: reduced tail **plus** manual viewport registration + top-of-frame decrements +
`ProcessViewports`. Either way the Rust `_sim.txt` isolation assert (triple-isolation, §3b precedent)
catches any divergence.

**The Rust 4d oracle-test** (`rust/oracle-tests/tests/render_slice4d_common/` + a `render_slice4d.rs`)
drives the same scenario through the **live game-layer path** (top-decrement + event-drain +
`frame::draw`) — not the injection path — and asserts, per tick: `frame_hash` == sidecar, `state_hash`
== sidecar == `_sim.txt` (triple isolation), plus the folded `total` + row count. This is the 4d
milestone.

---

## 6. `steerable_sum` investigation + recommendation → **DEFER**

**Open Q5 (overview) leaned "add + re-fuzz." New evidence says defer.** The overview assumed adding
steerable centering = "two hashed accumulators, re-fuzz." The reality is worse:

- `steerable_sum_x/y`/`steerable_count` are set **only** inside `Worm::ProcessSteerables`
  (`worm.cpp:1214-1241`), which runs only when the current weapon's `shot_type == kStSteerable`
  (`= 2`, `weapon.hpp:21`). That function, when `Left`/`Right` is pressed, **mutates
  `wobject.cur_frame`** (`worm.cpp:1225,1229`) and sets `worm.movable = false` (`:1233`).
- **`wobject.cur_frame` IS hashed** — both C++ (`stateHash.hpp:104`) and Rust
  (`hash.rs`, wobject cur_frame). So porting `ProcessSteerables` is a **genuine simulation-behavior
  change** requiring a full re-fuzz, not the cheap "two unhashed fields" the overview imagined.
  (The accumulators themselves are unhashed — neither `HashGameState` nor `hash_game_state` folds
  `steerable_*` — but the **enabling port** touches hashed state.)
- **`ProcessSteerables` is not ported in Rust at all** (`state.rs:1867`: "process_steerables: no-op
  this slice"). Steerable-weapon *flight* is ported (`weapon.rs:119` shares `ST_TYPE2 || ST_STEERABLE`
  flight), but the worm-side steering/accumulation is not.
- **No committed scenario reaches a steerable weapon.** `steerable_count` is always 0 today, so an
  added centroid path would be **vacuous** (untestable without also authoring a steerable-weapon
  scenario + its own golden + the re-fuzz).

**Recommendation:** keep the non-steerable `SetCenter(Ftoi(pos))` and **keep the
`steerable_count == 0` `debug_assert` as a live guard** (§4). Defer steerable centering to a later,
dedicated slice that ports `ProcessSteerables` **with** a steerable-weapon scenario, a matching
golden, and a re-fuzz — where the cost is justified by an actual feature, not a camera nicety. This
keeps 4d's isolation budget clean (screen_flash + shake-event are both provably hash-neutral) and the
milestone crisp. Everything else in `ProcessViewports` (pos-centering, shake, banner) needs no sim
change.

---

## 7. Banners — in/out decision

**Decision: `banner_y` state stepping is IN (mandatory); the `KillEmAll` death-banner text draw is IN;
the `GameOfTag` `YoureIt` banner is DEFERRED.**

- **`banner_y` state stepping is inseparable from the port** — it is phase-1 + phase-5 viewport state
  and is fully bit-exact-gateable via the 4d death scenario's per-tick viewport state (rides the
  frame hash implicitly, and can be spot-asserted). It ships in the core game-layer stepping (T2).
- **Death-banner text draw is now unblocked:** 3e delivered `render::font::Font` +
  `Font::DrawString` (`liero-rs-PROGRESS.md:268`), so the text dependency the 3e deferral cited
  ("death banners… structurally unreachable → Step 4", `:386-387`) is solved, and we **are** in Step
  4. A death is reachable (the 3e death scenario drives `health<=0`), so the banner is gateable. Port
  the cross-viewport `KilledMsg`/`CommittedSuicideMsg` block (`viewport.cpp:256-270`) using the 3e
  font, gated by its own golden (dumper draws banners in the `render_live` path). This delivers the
  overview's "death banners appear" proof (overview §4d).
- **Defer the `YoureIt` / GameOfTag arm** (`viewport.cpp:249-254`): it needs `got_changed` + game-mode
  plumbing that is orthogonal to the flash/shake headline; adding it would import game-mode surface
  for no additional viewport-mechanics coverage. Route it to whichever later slice needs GameOfTag
  presentation.
- **New localized strings** `KilledMsg` / `CommittedSuicideMsg` must be added to the Rust `LS`
  corpus (3e added `Reloading`/`Kills`/`Lives`/`PressFire`); small and mechanical.

If the banner **draw** balloons during T5 (string plumbing, cross-viewport iteration edge cases), it
may be trimmed to `banner_y`-state-only with the draw deferred — but the recommendation, given the
font exists, is to ship the death-banner draw.

---

## 8. Follow-cam relation (deferral, unchanged)

3c deferred **follow-cam** (`--follow` / `killed_timer` zeroing to force centering; diverges from
goldens — `liero-rs-PROGRESS.md:370-371`). 4d delivers **faithful** centering: the camera follows the
worm exactly as C++ does — but only when the worm is `visible` + alive (§4). Follow-cam is the
*non-faithful convenience* that would centre even on a dead/invisible worm; it stays deferred and is
**not** the default. 4d's live `SetCenter` does not implement or block it; the two are independent.
The `as_scene(0,…)` → `as_scene(sim.screen_flash,…)` change (§3c) is the only main.rs render-call edit
4d makes; the fixed ×3, non-resizable window and 2-viewport layout are unchanged.

---

## 9. Risks & the hard 10%

1. **Shake/decrement ordering (the central risk).** The decrement must use the **previous** tick's
   `shake` and run **before** the explosion-max, with the RNG the only later reader (§3). Getting the
   split-around-`process_frame` order wrong (e.g. decrementing after the event max, or reading
   post-`++cycles` for the banner walk) desyncs the frame hash. RED-prove the order with a two-tick
   explosion scenario before wiring.
2. **The dumper `render_live` path perturbing priors.** Calling real `ProcessFrame` is the risk the
   **mandatory re-diff gate** exists to catch; every prior golden must come back byte-identical. If
   it does not, fall back to reduced-tail + manual `ProcessViewports` (§5).
3. **The `killed_timer` trap making centering vacuous.** If the 4d scenario does not actually spawn a
   worm to visible+alive, `SetCenter` never runs and the "live centering" claim is untested (§4). The
   scenario must include a Fire-to-spawn and reach `killed_timer<=0`.
4. **`screen_flash` accidentally entering the hash.** The whole hash-neutrality (and "no re-fuzz")
   rests on **not** folding `screen_flash` into `hash_game_state`. A "for completeness" addition to
   the hash would force a spurious re-fuzz and break isolation — do not.
5. **Steerable live-guard firing.** Any future/4d scenario that equips a `shot_type==2` weapon trips
   the `debug_assert` (§6). Keep the 4d scenario steerable-free; the guard is the tripwire, as
   intended.
6. **Banner string/rendering surface creep.** The `KilledMsg`/`CommittedSuicideMsg` draw is bounded;
   the `YoureIt`/GameOfTag arm is the creep vector — keep it deferred (§7).
7. **Harness hand-copy drift (minor, carried post-landing).** The Rust 4d oracle-test drives the
   live game-layer path through a small (~15-line) hand-copy of `viewport_step.rs`'s tick order
   (`game/src/main.rs`'s Bevy `!Send`/ECS resource shape cannot be driven headless without one) —
   a theoretical frame-level blind spot if the two drift apart silently. Mitigated, not eliminated,
   by double-anchoring: the golden's byte-for-byte comparison assertion plus a doc comment in the
   harness cross-referencing the source function it copies. A future consumer touching the ordering
   should re-verify both sides move together; factoring the copy away needs a third,
   headless-drivable consumer of the same stepping order to be worth it (same posture as 3d's
   CLI-local driver-copy deferral).

---

## 10. File-touch summary (for the plan)

- `rust/sim/src/state.rs` — `screen_flash` field + top-of-`process_frame` decrement; the
  explosion-shake event `Vec` + a drain accessor. **Not** added to `hash.rs`.
- `rust/sim/src/sobject.rs` — the create path writes `screen_flash` + pushes the shake event
  (`:146`).
- `rust/render/src/viewport.rs` — keep `process()` + the steerable live guard; no structural change.
- `rust/render/src/frame.rs` — no change (already threads `Scene.screen_flash`).
- `rust/game/src/main.rs` — top-of-frame stepping (decrement + banner walk) before `process_frame`;
  event-drain + apply after; `as_scene(sim.screen_flash, …)` (drop the hardcoded `0`).
- `rust/render/src/` (font/banner) — death-banner draw (T5) + `LS` strings.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — `render_live` directive (viewport wiring + real
  `ProcessFrame`); the dumper change behind the re-diff gate.
- `rust/oracle-tests/gen_render_slice4d_*.sh` + `golden/render_slice4d_*` — the new live golden.
- `rust/oracle-tests/tests/render_slice4d*.rs` — the live oracle-test (milestone).
- `docs/superpowers/liero-rs-PROGRESS.md` — 4d status + deferral bookkeeping (steerable centering,
  GameOfTag banner, follow-cam).
