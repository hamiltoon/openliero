# Step 4, Slice 4d — Live shake / flash / banners (`ProcessViewports` port): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** Wire the render-only viewport side effects Step 3 deferred so they run **live** in a real
match — top-of-frame `screen_flash`/`shake`/`banner_y` stepping, `ProcessViewports` centering, the
viewport-local-RNG shake, and death banners — driven by real worm/explosion events rather than the 3b
`render_shake`/`render_flash` injection directives. Companion spec:
`specs/2026-07-13-liero-rs-step4-slice4d-live-viewport-design.md` (cited **spec §N**).

**Milestone:** a **live flash/shake render golden bit-exact** vs a new C++ dumper sidecar (explosion-
and spawn-driven, not injected), **triple-isolation** green (Rust `state_hash` == sidecar ==
`_sim.txt`), and the effect **visible in `cargo run -p game -- --live`**. Everything after the
milestone (death-banner text draw, broad review) rides on top.

**Architecture (spec §3):** additive and hash-neutral. `screen_flash` becomes a **`SimState` field**
(top-of-frame decrement + sobject-create write) but is **NOT** added to `hash_game_state` — because
C++ `HashGameState` omits it too, so the sim goldens re-diff **byte-identical** (no re-fuzz, spec §0).
`shake` is fed by a **sim-emitted explosion-shake event** `(x,y,amount)` drained by the game layer
(the 4c-analogous seam, but independent). The game loop runs the top-of-frame decrement + banner walk
**before** `process_frame` (previous-tick values, pre-`++cycles`) and the event-max **after**, with
`ProcessViewports` (centering + shake-RNG) in the render — reproducing C++ phase order because the
only `shake` reader is the render RNG (spec §3). The golden needs a **new opt-in C++ dumper directive
`render_live`** that wires the two viewports + runs real `Game::ProcessFrame`; because it is opt-in,
priors stay byte-identical behind a **mandatory re-diff gate** (spec §5). **Steerable centering is
DEFERRED** (porting `ProcessSteerables` mutates hashed `wobject.cur_frame` → real re-fuzz for a camera
nicety; no scenario reaches it — spec §6); the `steerable_count==0` assert stays as a live guard.

## Global constraints

*(inherit every Step 2/3/4a/4b constraint; the 4d-specific ones follow)*

- **Bevy stays confined to `game`.** `sim`, `render`, `scenario`, `assets`, `sim-core` remain
  Bevy-free. The `screen_flash` field + explosion-shake event vec are plain data on `SimState`
  (`cargo tree -p sim` shows no `bevy*`); no `f32`/`Vec2`/`Transform` leaks into the sim.
- **`screen_flash` is hash-neutral — never fold it into `hash_game_state`** (spec §0, §9 risk 4).
  The entire "no re-fuzz" property depends on this. The shake-event vec is likewise unhashed and
  draws **zero** `rand`.
- **The 3b injection path is untouched.** `render_shake`/`render_flash` dumper directives and the
  `render_slice3b_common` set-before/restore-after test path stay exactly as-is. 4d adds a **new**
  live path; it does not replace or regenerate the 3b corpus.
- **Non-live paths byte-identical (re-diff discipline).** Every committed `sim_slice*`,
  `render_slice3a/3b/3e` golden MUST stay byte-identical. `git diff --stat` on `golden/` shows only
  **new** `render_slice4d_*` files. The dumper `render_live` change is behind a **mandatory re-diff
  gate task** (T3): rerun every `gen_*` script, assert all priors byte-identical.
- **Steerable centering deferred; keep the `steerable_count==0` live guard** (spec §6). The 4d
  scenario must be **steerable-weapon-free** so the guard never trips.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-4`** (the accumulating Step-4 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new/edited files** (watch
  the `blit.rs`/`viewport.rs` rustfmt footgun — do not blanket-fmt; new lines fmt-clean by hand).
  **No sub-subagents.** **Bash discipline:** one command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`;
  no `cd`+`git`; create files with the editor.

## Model tiers

- **[Opus]:** every task touching the sim, the bit-exactness ordering, the C++ dumper, the re-diff
  gate, the golden/milestone, and the final review (the hard ports + all gates).
- **[Sonnet]:** mechanical sub-steps only (adding `LS` strings, wiring an already-designed struct
  field) — none of the ordering-sensitive or gate tasks. When in doubt, Opus.

## Tasks

### T0 [Opus] — `screen_flash` into the sim (hash-neutral) + re-diff

- [ ] **RED:** unit test in `sim` — an explosion sobject-create with `ty.flash>0` sets
      `state.screen_flash = max(flash, prev)`, and a following `process_frame` decrements it by 1
      (floored at 0). Assert it is **absent** from `hash_game_state` (mutate `screen_flash`, hash
      unchanged).
- [ ] Add `pub screen_flash: i32` to `SimState` (init 0, `state.rs:907-`/`:1255`); decrement at the
      **top** of `process_frame` (`state.rs:1431`, before the object loops); write it in
      `sobject.rs:146` (replace the "omitted" comment). **Do not** touch `hash.rs`.
- [ ] **Re-diff gate:** rerun the `gen_sim_*` scripts (or assert via the existing oracle-tests) that
      every `sim_slice*` golden is byte-identical. GREEN = hash-neutral proven.

### T1 [Opus] — explosion-shake event seam (sim emits, hash-neutral)

- [ ] **RED:** unit test — an explosion with `ty.shake>0` pushes one `(x, y, amount)` onto the sim's
      shake-event vec; `ty.shake==0` pushes none; the vec draws **zero** `rand` and does not change
      `hash_game_state`. A `drain` accessor empties it and returns the events.
- [ ] Add the shake-event `Vec` to `SimState` + a `drain_shake_events()` accessor; push in the
      sobject-create path (`sobject.rs:146`, alongside T0's `screen_flash` write) using the raw blast
      `(x,y)` (pre `-8` offset, matching `sobject.cpp:29-31`) and `ty.shake`. Clear/drain semantics
      per tick. **Not** hashed.
- [ ] Re-diff: sim goldens still byte-identical.

### T2 [Opus] — game-layer live stepping: decrement + banner walk + event apply + live flash

- [ ] **RED (ordering):** a `game`/headless test driving a 2-tick explosion+spawn scenario asserts
      the per-tick `(vp.shake, vp.banner_y, vp.x, vp.y)` and `sim.screen_flash` match hand-computed
      C++ values — proving decrement-before-`process_frame`, event-max-after, banner walk on
      pre-`++cycles` `(cycles&1)` reading pre-worm-loop `killed_timer`, and centering reading
      post-worm-loop state (spec §3, §9 risk 1). SEE it fail first.
- [ ] In `game/src/main.rs tick_and_render`, **before** `process_frame`: `for vp { if vp.shake>0
      { vp.shake -= 4000 } }`; `if (sim.cycles & 1)==0 { for vp: step banner_y toward
      (worm[vp.worm_idx].killed_timer>16 ? 2 : -8) }`.
- [ ] **After** `process_frame`: drain shake events; for each, `for vp where rect contains (x,y):
      vp.shake = max(itof(amount), vp.shake)`.
- [ ] Thread live flash: `render_and_upload` calls `as_scene(sim.screen_flash, draw_shadow)` —
      **drop the 3c hardcoded `0`** (`main.rs:497`). `Viewport::process` (centering + shake-RNG +
      clamp + `banner_y=-8` reset) already runs inside `frame::draw`; confirm the steerable live
      guard (`viewport.rs:86`) stays.
- [ ] Debug determinism self-check stays Scripted-only (4a/4b posture); Live/replay unaffected.

### T3 [Opus] — C++ dumper `render_live` + MANDATORY re-diff gate

- [ ] Add an opt-in `render_live` directive to `src/tools/oracle_dump/sim_physics_dump.cpp` that, for
      the scenario that sets it: registers the two player viewports (`AddViewport`), runs the **real
      `game.ProcessFrame()`** per tick (so sobject-create sets viewport `shake` + `screen_flash`, the
      top-of-frame decrements run, and `ProcessViewports` centers/shakes/walks `banner_y`), then draws
      the world block. Leave the reduced-tail + `render_shake`/`render_flash` injection path
      **unchanged** for all other scenarios (spec §5).
- [ ] **Mandatory re-diff gate:** rerun **every** `gen_*` script; assert every committed
      `sim_slice*`, `render_slice3a/3b/3e` golden is **byte-identical** (the dumper binary changed;
      opt-in directive ⇒ priors must not move). If any prior moves, fall back to reduced-tail +
      manual viewport-registration + top-decrements + `ProcessViewports` (spec §5) and re-diff again.

### T4 [Opus] — Live 4d golden + oracle-test (THE MILESTONE)

- [ ] Author `render_slice4d_*` scenario(s): a worm **Fire-to-spawn** (reaches `visible=true,
      killed_timer<=0` so `SetCenter` runs — the `killed_timer` trap, spec §4), a **real explosion**
      that sets `screen_flash` + viewport `shake` (flash blip + camera jitter), and a **death**
      (drives `health<=0`, exercising the dead-arm `SetCenter` + `banner_y=-8` reset + the every-
      other-cycle banner walk). **No steerable weapon** (keep the live guard quiet). Set `render_live`.
- [ ] `gen_render_slice4d_*.sh` produces the `_sim.txt` (11-col) + sidecar `render_slice4d_*.txt`
      (`<tick> <frame_hash16> <state_hash8>` + `total`) via the T3 dumper.
- [ ] **RED→GREEN:** `rust/oracle-tests/tests/render_slice4d*.rs` drives the scenario through the
      **live game-layer path** (T2's decrement + event-drain + `frame::draw`), asserting per tick:
      `frame_hash` == sidecar, `state_hash` == sidecar == `_sim.txt` (**triple isolation**), the
      folded `total`, and row count. **Non-vacuity:** the flash tick's frame differs from a no-flash
      tick; a shaking tick's viewport `(x,y)` differs from centering-only; `banner_y` reaches `-8`
      then walks. This is the **milestone** — flash/shake live + bit-exact.
- [ ] **Visible check:** `cargo run -p game -- --live <4d-scenario>` (or the `liero-shot` skill at a
      flash tick) shows the flash blip + shake + camera follow. Advisory (eyeball), not a CI gate —
      4a/3c posture.

### T5 [Opus] — Death-banner text draw (font exists since 3e)

- [ ] **RED:** a golden/oracle assertion on a tick where a dead worm's banner is visible
      (`health<=0`, `banner_y>-8`) — the cross-viewport `KilledMsg`/`CommittedSuicideMsg` text lands
      (frame hash changes vs banner-suppressed).
- [ ] Add `LS` strings `KilledMsg` / `CommittedSuicideMsg`; port the cross-viewport banner block
      (`viewport.cpp:256-270`) using `render::font::Font::DrawString` (3e). Keyed by
      `last_killed_by_idx`. **Defer** the `YoureIt`/GameOfTag arm (`viewport.cpp:249-254`) — needs
      `got_changed`+game-mode (spec §7). The dumper `render_live` path draws banners so the golden
      gates it.
- [ ] Re-diff: no prior golden moves.
- [ ] *(If the banner draw balloons — string plumbing / cross-viewport edge cases — trim to
      `banner_y`-state-only and defer the draw with a note; T2's `banner_y` stepping already ships
      the state. Recommendation is to land the draw.)*

### T6 [Opus] — Broad final review + PROGRESS + deferral bookkeeping

- [ ] Full-suite green: `cargo test --workspace --exclude game` + `cargo test -p game` (the CI
      commands); every `render_slice4d_*` + `render_slice3b_*` + `sim_slice*` golden accounted for;
      `git diff --stat golden/` shows only **new** 4d files.
- [ ] Broad review pass (0 Critical / 0 Important bar): the decrement/event ordering (spec §3), the
      `screen_flash` hash-neutrality (never in `hash.rs`), the re-diff gate results, the steerable
      live guard intact, the 3b injection path untouched.
- [ ] Update `docs/superpowers/liero-rs-PROGRESS.md`: 4d MILESTONE GREEN entry + the carried
      deferrals — **steerable centering** (`ProcessSteerables` port + re-fuzz + steerable scenario),
      **GameOfTag `YoureIt` banner**, **follow-cam** (unchanged from 3c). Use the real currentDate.
- [ ] Commit on `liero-rs-step-4`. No push, no PR (controller owns that).

## Task dependency / milestone map

```
T0 (screen_flash) ─┐
T1 (shake event) ──┼─> T2 (game-layer stepping + live flash) ─┐
T3 (dumper render_live + re-diff) ────────────────────────────┼─> T4 (LIVE GOLDEN = MILESTONE)
                                                               │
                                                        T5 (banner draw) ──> T6 (final review)
```

T0/T1/T3 are independent; T2 needs T0+T1; **T4 (milestone) needs T2+T3**; T5 builds on T4; T6 last.
