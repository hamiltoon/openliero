# Step 4, Slices 4f + 4g — minimal start-flow & run/verify harness: design

Status: **DESIGN — Step 4's last two slices, planned together (both small)** · 2026-07-13
Part of: `2026-07-12-liero-rs-step4-input-replay-overview.md` (cited **overview §N**)
Builds on: `2026-06-26-liero-rs-interactive-iteration-exploration.md` (cited **iter §N**)
Companion plan: `plans/2026-07-13-liero-rs-step4-slice4f4g-plan.md`

4a–4e have landed (playable live input, record→replay round-trip gate, audio, live
shake/flash/banners, phase-1 `.lrp` reader). This design closes Step 4 with its two
remaining slices, deliberately co-planned because each is tiny:

- **4f — minimal start flow.** Make bare `cargo run -p game` launch a *playable* match
  (not the fixed blood replay demo), with respawn (already in the sim), quit (already
  done), and restart. The smallest thing that turns single-player from a fixed demo into
  a **loop**.
- **4g — run/verify skill + CI regression sweep.** Extend the in-repo `liero-shot`
  run-skill to cover the input/replay flows, and confirm the CI regression wiring. As the
  inventory below shows, most of 4g's CI half has **already been delivered incrementally**
  across 4a–4e — so 4g reduces to a small run-skill documentation task plus one tiny
  `shot` enhancement.

---

## 1. 4f — minimal start flow

### 1.1 What "playable" already exists (the honest inventory)

The play loop is almost entirely built. Concretely, in `rust/game/src/main.rs`:

| Capability | Status today | Source |
|---|---|---|
| Keyboard-driven live tick | **Done (4a)** | `--live` → `InputSource::Live`, one snapshot/tick |
| Respawn (death → respawn) | **Done (in the sim)** | ported; the 4d milestone drove a real death→respawn at ~t240 |
| Quit | **Done** | `close_on_esc` (Esc) + winit window-X → `AppExit` |
| Indefinite run (no fixed tick bound) | **Done (4a)** | `Mode::Live` has no `ticks` boundary, runs forever |
| Audio + live shake/flash/banners in play | **Done (4c/4d)** | wired on the live path |

So `cargo run -p game -- --live` is *already a playable, respawning, quittable
single-player/2P-hotseat match*. What is missing for 4f is narrow:

1. **Bare `cargo run -p game` (no args) is still the fixed Scripted `blood` demo** — it
   replays committed recorded inputs (the DART-into-own-feet fire) for 40 ticks and loops
   bit-identically. It ignores the keyboard except Esc. To a first-time runner, "the game"
   is a non-interactive demo. 4f's core job: **bare invocation must start a playable
   match.**
2. **No restart** — you cannot reset the match without killing and re-launching the
   process. The slice done-when explicitly names "started and **restarted** without editing
   code" (overview 4f bullet).
3. **Cosmetic:** the window title is still `"Liero-rs — 3c demo ({name})"` — the "3c demo"
   string has been a flagged cosmetic minor since 4a T2.

### 1.2 The default-match question (the one real decision)

**Question:** to start a match with no committed scenario name, what level and worms does
it use? `scenario::load` (`rust/scenario/src/loader.rs:100`) is the *only* state-builder,
and it needs a parsed `Scenario` carrying a `level` path + `worm` lines + a `weapon 0`
override. C++'s real "new game" builds a **random** level via `GenerateFromSettings`, which
is **not ported** (a large deferred surface).

**Options considered:**

- **(A) Flip the bare-invocation default to `Mode::Live` over an existing committed
  scenario**, using it *only* for its level + worm-init (the recorded inputs are ignored
  in Live mode). Smallest possible change — no new fixture, no code-level `Scenario`
  synthesis, no level generation.
- **(B) Author a tiny dedicated `default_match` scenario fixture** (fixed `level` from
  `data/TC`, two visible worms at spawn positions, `weapon 0 <default>`) and default the
  bare invocation to `Mode::Live` over it. ~7 lines of committed text; no code generation,
  no `GenerateFromSettings`.
- **(C) Synthesize a default `Scenario` in Rust code** (no file). More code, and it must
  still name a `data/` level — buys nothing over (B).

**Recommendation: (B), with (A) as the fallback.** Author one small committed
`default_match` scenario (a fixed, shipped `data/TC` level; two visible worms; a sane
default `weapon 0`). Bare `cargo run -p game` loads it in `Mode::Live` — keyboard-driven,
respawning, indefinite. A purpose-built fixture reads clearly as "the default match,"
whereas shipping a 40-tick test golden (`blood`) as the default is confusing. If authoring
+ validating the fixture threatens to grow the slice, fall back to (A) (default to Live
over `blood`'s level+worm setup) — functionally identical, zero new files.

**Explicitly deferred (do NOT pull in):** random level generation /
`GenerateFromSettings`, seed randomization. The default match uses a **fixed level and a
fixed seed 42** — a documented, deliberate reduction (Live play diverges from input
immediately anyway; the seed only sets initial spawn positions + RNG). Random-level
generation is a Step-5+/post-step item, unrelated to the input/replay goal.

### 1.3 Restart

Reuse the machinery that already exists. The Scripted loop-reload path
(`main.rs` `tick_and_render`, `Mode::Scripted` arm) already rebuilds a bit-identical match
from tick 0 via `scenario::load` (resetting `sim.0`, `demo.viewports`, `demo.scene`). 4f
adds a **restart key** on the Live path that runs that same reload — ~10 lines.

**Key choice (constraint):** the restart key must **not collide** with the default
bindings (`rust/game/src/input.rs` `default_bindings`):

- P0: `KeyR` (fire), `KeyF`, `KeyD`, `KeyG`, `ControlLeft`, `ShiftLeft`, `AltLeft`
- P1: `ArrowUp/Down/Left/Right`, `ControlRight`, `AltRight`, `ShiftRight`

So **`R` is taken** (P0 fire) — the overview's parenthetical "R? — krockar med P0" is
correct; pick another. **Recommend `F5`** (universal reload/restart convention, unbound)
or `Backspace` as a second choice. Both are free. *(Implementation must verify the Bevy
0.19 `KeyCode` spelling — `KeyCode::F5` — same low-confidence flag 4a raised for KeyCode
names.)*

### 1.4 Quit / respawn — no work

Esc and the window-X already quit. Respawn is already in the sim. 4f writes **zero** code
for either; the design records them only so the slice done-when is provably met.

### 1.5 The regression-path implication (the one 4f risk)

Today, bare `cargo run` runs `Mode::Scripted` over `blood`, which drives the
`#[cfg(debug_assertions)]` per-tick determinism self-check against the committed golden —
a *free* regression signal on the default invocation. If bare invocation becomes
`Mode::Live`, that free self-check on the *bare* command is lost (Live has no golden).

**This is acceptable and does not weaken the regression gate**, because:

- The **authoritative** gate is `rust/game/tests/passthrough.rs` (4a), which constructs
  `InputSource::Scripted` **directly** over all 7 committed scenarios and runs in CI
  (`cargo test -p game`). It never depended on the bare run mode.
- The Scripted self-check demo stays reachable: **keep the positional-`<name>` path as
  `Mode::Scripted`** (unchanged). `cargo run -p game -- blood` still self-checks. Only the
  *no-arg* case flips to Live.

So the only change is: **"no positional name and no `--live`" → Live default match** instead
of Scripted-`blood`. `--live [name]`, `--replay`, `--record`, and `<name>` (Scripted) are
all unchanged.

**Wasm stays Scripted-`blood`.** Browser keyboard input is a deferred item (live-wasm,
4a). `resolve_scenario`'s `#[cfg(target_arch = "wasm32")]` arm is **untouched**, preserving
the wasm determinism + frame-hash parity witness (the debug `debug_assert` loop). The 4f
default-match flip is **native-only**.

### 1.6 4f scope — the minimal recommended set

**In scope (small):**
1. Bare `cargo run -p game` → `Mode::Live` over a committed default-match scenario
   (recommend a new tiny fixture; fall back to `blood`'s level+worm setup). Fixed level,
   fixed seed. Native-only; wasm unchanged.
2. Restart key (`F5`, verified-unbound) on the Live path, reusing the existing
   `scenario::load` reload.
3. Window-title cleanup ("3c demo" removed).

**Out of scope (resist — Locked decision 5 / overview §Open Q4/Q5):** main-menu tree,
interactive weapon selection, weapon-select key-repeat emulation, file/profile UI, random
level generation / `GenerateFromSettings`, the `*State.cpp` render/flow surface.

---

## 2. 4g — run/verify skill + CI regression sweep

### 2.1 CI regression wiring — the inventory (already de facto delivered)

overview 4g reads "wire the 4b round-trip and (if 4e lands) the `.lrp` framehash diff into
CI." Auditing `.github/workflows/rust.yml` against the shipped tests shows **every
input/replay gate already runs in CI** — they were each wired incrementally as their slice
landed:

| Gate | Test | CI step | Slice |
|---|---|---|---|
| Live-sampler pass-through state gate | `game/tests/passthrough.rs` | `cargo test -p game` | 4a |
| **Record → replay round-trip (the hard gate)** | `game/tests/round_trip.rs` | `cargo test -p game` | 4b |
| Recorded-corpus drift backstop | `game/tests/record_regression.rs` | `cargo test -p game` | 4b |
| Live viewport stepping | `game/tests/viewport_stepping.rs` | `cargo test -p game` | 4d |
| **Phase-1 `.lrp` `WideRollbackChecksum` gate** | `replay/tests/corpus.rs` | `cargo test --workspace --exclude game` | 4e |
| `WideRollbackChecksum` port units + all sim/render goldens | `sim` / `oracle-tests` | `cargo test --workspace --exclude game` | 2–4e |

The CI job runs exactly two cargo-test invocations — `--workspace --exclude game` (the fast
Bevy-free gate: sim, scenario, render, **replay**, oracle-tests) and `-p game` (the Bevy
crate's headless tests) — and between them they execute **all** of the above.

**Conclusion: 4g's "wire the round-trip + replay regression into CI" is already complete.**
The 4b round-trip gate and the 4e phase-1 `.lrp` checksum gate are both green in CI today.
The only unwired item is the **`.lrp` `framehash` diff**, which is **phase 2** — explicitly
out of Step-4 scope (a booked follow-on; overview done-when §5, §Open Q1). So there is *no*
CI work required for 4g beyond documenting that the wiring is in place.

### 2.2 Run-skill (`liero-shot`) — the genuine gaps

The run-skill (`.claude/skills/liero-shot/SKILL.md`, iter §5/§6) currently documents only
the `shot` renderer CLI over committed scenarios. Two real gaps remain:

1. **The skill says nothing about the input/replay flows** the `game` binary now offers:
   `--live [name]` (play), `--record <path>` (capture a live session to a scenario file),
   `--replay <path>` (play back any recorded scenario), and — after 4f — the bare
   playable default match + restart. iter §5 wants the run-skill to *drive the replay
   loop*; today it drives only the renderer. **Documentation gap.**
2. **`shot` cannot screenshot a replay/recording.** `shot` resolves `--scenario <name>`
   to a committed golden sidecar by name (`render_slice3e_*` then `render_slice3b_*`,
   `shot/src/lib.rs:324`). It does **not** know the `record_slice4b_` prefix and cannot
   take an arbitrary path — so a recorded match cannot be screenshotted at tick N. iter §5
   ("a replay player makes `verify` powerful — screenshot at tick N of a recorded match")
   is therefore not yet reachable headless. **One tiny code gap.**

### 2.3 4g scope — the minimal recommended set

**In scope (small):**
1. **`shot --scenario-path <file>`** — a new flag loading an *arbitrary* scenario/recording
   file (bypassing the name→golden-dir resolution), then screenshotting / hashing it via
   the existing `render_scenario`. Because a 4b recording **is** a scenario file, this is
   the whole "replay-driven screenshot" capability — `shot` already drives any parsed
   `Scenario` headlessly; only the *source* is new. ~15 lines + a unit test.
2. **`liero-shot` SKILL.md expansion** — document (a) `game --live` play, (b)
   `game --record`/`--replay` round-trip, (c) `shot --scenario-path <recording>` for a
   headless replay screenshot at a fixed tick, and (d) a one-line note that CI already
   gates the round-trip + `.lrp` checksum regressions (§2.1). Keep the existing renderer
   sections intact.

**Out of scope:** the `.lrp` `framehash` diff / phase-2 cereal work (booked follow-on);
any new CI job (the gates already run); a Playwright/browser-screenshot CI dependency (iter
open question — stays manual/native-headless).

### 2.4 Honest assessment

4g is **~70% delivered incrementally**: the entire CI-regression half is already green.
The residual is one small `shot` flag (to unlock replay screenshots) plus run-skill
documentation. It is genuinely a documentation-plus-a-flag slice, not a build-out.

---

## 3. Risks & the hard bits

- **Default-mode flip regresses the free self-check on the bare command (4f).** Mitigated
  by keeping the positional-`<name>` Scripted path and its self-check, and by the CI
  `passthrough.rs` gate being the authoritative regression — unaffected (§1.5). The design
  must not let bare-Live *silently disable* any regression path; it doesn't, because no
  gate ran off the bare invocation.
- **Wasm parity witness (4f).** The default-match flip is native-only; the wasm
  `resolve_scenario` arm and its debug determinism/frame-hash `debug_assert` loop stay
  byte-unchanged. Any accidental change to the wasm arm would break the 3f/4-era browser
  witness — a standing tripwire, re-checked in the final review.
- **Restart key collision (4f).** `R` is P0 fire; the restart key must be a verified-unbound
  key (`F5`/`Backspace`). Cross-check against `default_bindings` at implementation; verify
  the Bevy 0.19 `KeyCode` spelling.
- **Default-match fixture must be genuinely playable (4f).** If reusing an existing
  scenario (fallback A), confirm it spawns **two visible worms** so 2P hotseat works;
  otherwise author the fixture (option B) with two visible worms. A single-worm or
  all-invisible setup would launch an unplayable match.
- **`shot --scenario-path` must not perturb the golden path (4g).** The new source is
  purely additive — the name→golden resolution and every `render_slice*` frame/state hash
  stay byte-identical. A unit test drives an arbitrary file; the existing golden compares
  are untouched.
- **Scope creep into the deferred menu surface (4f).** The single largest 4f risk per
  Locked decision 5 — resist all of `*State.cpp`, weapon-select, and level generation.

---

## 4. Step-4 completion context (for the final review)

4f + 4g are the last two slices of Step 4. The plan's final task is therefore **both** the
4f/4g broad review **and** the **Step-4 final review** across the whole `liero-rs-step-4`
branch (PR #5), ahead of John's merge decision — with its own checklist covering every
slice's done-when, PROGRESS/overview consistency, a green CI, and a complete deferral
track. The one item to surface explicitly at merge: **`.lrp` phase 2 (cereal `Game` graph +
`framehash` diff) is a booked follow-on outside Step-4 scope** — John decides at step-close
whether it lands before merge (overview done-when §5, §Open Q1).
