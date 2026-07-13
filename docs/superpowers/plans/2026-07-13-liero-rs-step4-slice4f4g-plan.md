# Step 4, Slices 4f + 4g — start-flow & run/verify harness: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Test-first**: write the failing test (or its assertion) before the
> implementation it pins, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** close Step 4's last two slices, co-planned because each is tiny. **4f:** bare
`cargo run -p game` (no args) starts a *playable* keyboard-driven match (not the fixed Scripted
`blood` demo) — respawn (already in the sim), quit (already done), restart (new). **4g:** extend the
in-repo `liero-shot` run-skill to cover the input/replay flows + add `shot --scenario-path` so a
recording can be screenshotted headless; the CI-regression half of 4g is **already delivered** (every
input/replay gate runs in CI — design §2.1). Companion spec:
`specs/2026-07-13-liero-rs-step4-slice4f4g-start-flow-and-harness-design.md` (cited **design §N**).

**Milestone (T0 + T3):** `cargo run -p game` with no args launches a playable, respawning,
restartable default match, **and** the `liero-shot` run-skill documents the full live/record/replay
flow (overview done-when §6). The Scripted self-check demo (`cargo run -p game -- blood`) and the CI
`passthrough`/`round_trip`/`.lrp`-checksum gates stay green throughout.

**Final task (T4):** the broad review is **also the Step-4 final review** across the whole
`liero-rs-step-4` branch (PR #5), ahead of John's merge decision — own checklist below.

## Global constraints

*(inherit every Step 2/3/4a–4e constraint; the 4f/4g-specific ones follow)*

- **The default-mode flip is NATIVE-ONLY.** `resolve_scenario`'s `#[cfg(target_arch = "wasm32")]`
  arm stays **byte-unchanged** (`Mode::Scripted` over `blood`) — it preserves the wasm determinism +
  frame-hash `debug_assert` parity witness (design §1.5). Only the native "no positional name and no
  `--live`" case changes.
- **Do NOT weaken the regression path.** Keep the positional-`<name>` path as `Mode::Scripted` with
  its per-tick self-check (`cargo run -p game -- blood` still self-checks). The authoritative gate is
  `game/tests/passthrough.rs`, which constructs `InputSource::Scripted` directly and is unaffected —
  do not let bare-Live silently disable any gate (design §1.5, overview Risk "determinism guard
  retirement").
- **No new CI job / no `.lrp` framehash diff.** Every input/replay gate already runs in CI
  (design §2.1). The `.lrp` `framehash` diff is **phase 2** — a booked follow-on, out of Step-4 scope.
- **Resist the deferred menu surface (Locked decision 5 / overview §Open Q4/Q5).** No main menu, no
  weapon-select, no weapon-select key-repeat, no file/profile UI, **no random level generation /
  `GenerateFromSettings`** — the default match uses a **fixed** level + fixed seed 42.
- **Restart key must be verified-unbound.** `R` is P0 fire — do NOT use it. Use `F5` (or `Backspace`);
  cross-check `default_bindings` (`rust/game/src/input.rs`) and verify the Bevy 0.19 `KeyCode` spelling
  at implementation (low-confidence, same posture as 4a's KeyCode flag).
- **`shot --scenario-path` is purely additive.** The name→golden-dir resolution and every
  `render_slice*` frame/state hash stay byte-identical; a new flag only adds an alternative source.
- **No golden regressions.** `git diff --stat rust/oracle-tests/golden/` shows no moved existing
  golden. Any new default-match fixture is a **new** file.
- **No AI / "Generated with" taglines; no commit trailers** (no `Co-Authored-By`, no
  `Claude-Session`). **Commit on branch `liero-rs-step-4`** (the accumulating Step-4 PR). **Do NOT
  push and do NOT open a PR** — the controller owns push + PR. **fmt only new/edited files** (no
  blanket-fmt; `blit.rs` fmt-drift is a known footgun — write new lines fmt-clean by hand, do not
  rustfmt files that are not fmt-clean at HEAD). **No sub-subagents.** **Bash discipline:** one
  command per call; no `>>`/heredoc/`&&`/`;`/`$VAR`; no `cd`+`git`; create files with the editor.

## Model tiers

- **[Opus]:** the default-mode flip (a behavior change with a regression-path implication) and the
  Step-4 final review.
- **[Sonnet]:** the additive `shot` flag, the skill/PROGRESS/overview docs — mechanical, additive,
  low-risk. When in doubt, Opus.

## Tasks

### T0 [Opus] — 4f: playable default match + restart + title (the milestone)

- [ ] **RED:** add a native unit/smoke test (in `game`, e.g. a `resolve_scenario`-level or
      arg-parse test) asserting that **no args** resolves `Mode::Live` over the default match, while
      a positional `<name>` still resolves `Mode::Scripted`. See it fail against HEAD (bare = Scripted
      today).
- [ ] Pick the default-match source (design §1.2): **preferred** — author a tiny committed
      `default_match` scenario (a fixed shipped `data/TC` level; **two visible worms** at spawn
      positions; `weapon 0 <default>`; seed 42). **Fallback** — reuse an existing scenario's
      level+worm setup (verify it spawns two visible worms). No `GenerateFromSettings`, no random
      level, no code-level `Scenario` synthesis.
- [ ] Native `resolve_scenario`: when there is no positional name and no `--live`/`--replay`/
      `--record`, resolve `Mode::Live` over the default-match scenario. Leave `<name>` → Scripted,
      `--live [name]`, `--replay`, `--record` **unchanged**. Leave the wasm arm **byte-unchanged**.
- [ ] `setup`/`load_scenario_text`: source the default-match scenario text on the bare-Live path
      (native). Keep `Mode::Live`'s existing behavior (no golden column loaded, no self-check, runs
      indefinitely).
- [ ] **Restart key** on the Live path (`F5`, verified-unbound — cross-check `default_bindings`;
      verify `KeyCode::F5` spelling): on `just_pressed`, run the existing `scenario::load` reload used
      by the `Mode::Scripted` reload arm (reset `sim.0`, `demo.viewports`, `demo.scene`, `demo.tick`).
      Reuse that code; do not fork a new reset path.
- [ ] Window title: drop `"3c demo"` (e.g. `"Liero-rs — {name}"` or `"Liero-rs"`).
- [ ] **GREEN + gate:** the RED test passes; **launch-smoke both paths** (perl `alarm`-10s wrapper —
      `timeout`/`gtimeout` absent on this machine; exit 142 = ticked until alarm, no panic): bare
      `cargo run -p game` (Live default match) and `cargo run -p game -- blood` (Scripted, still
      self-checks). Confirm `cargo test -p game` (incl. `passthrough.rs`) and the wasm build stay
      green. `git diff` on existing goldens empty.
- [ ] Manual playability is John's advisory 30-sec check (same posture as 4a) — not a gate.

### T1 [Sonnet] — 4g: `shot --scenario-path <file>` (headless replay screenshot)

- [ ] **RED:** add a `shot` test (or arg-parse + render unit) that `--scenario-path <file>` loads an
      **arbitrary** scenario/recording file and renders/hashes it (e.g. point it at a committed 4b
      recording or any `render_slice*` scenario via full path). See it fail (flag unknown today).
- [ ] Add `--scenario-path <file>` to `shot`'s arg parser (`shot/src/lib.rs`), mutually exclusive
      with `--scenario <name>` (exactly one required). On the path branch, read the file directly and
      `Scenario::parse` it, **bypassing** `resolve_scenario_text`'s name→golden-dir lookup; feed the
      parsed `Scenario` into the existing `render_scenario`/`render_scenario_hud` unchanged.
- [ ] Update `USAGE`.
- [ ] **GREEN:** the RED test passes; confirm the name→golden path and every `render_slice*`
      frame/state hash are **byte-identical** (additive-only). `cargo test --workspace --exclude game`
      green.

### T2 [Sonnet] — 4g: `liero-shot` SKILL.md expansion (docs)

- [ ] Extend `.claude/skills/liero-shot/SKILL.md` (keep the existing renderer sections intact) with:
      (a) `cargo run -p game -- --live [name]` — play a match; (b) `--record <path>` capture +
      `--replay <path>` playback (the round-trip); (c) `shot --scenario-path <recording>
      --tick <n> --out ...` — headless replay screenshot at a fixed tick; (d) a one-line note that CI
      already gates the record→replay round-trip (`game/tests/round_trip.rs`) and the phase-1 `.lrp`
      checksum (`replay/tests/corpus.rs`) — design §2.1.
- [ ] Mention bare `cargo run -p game` now starts a playable default match + `F5` restart (from 4f).
- [ ] Keep it short and accurate; no invented flags.

### T3 [Sonnet] — 4f/4g docs: PROGRESS + overview LANDED (the doc half of the milestone)

- [ ] `docs/superpowers/liero-rs-PROGRESS.md`: header + step-4 tree → 4f ✅ / 4g ✅ (bump the %),
      with the whole-step map. Note 4g's CI half was delivered incrementally (design §2.1) and the
      run-skill now covers live/record/replay.
- [ ] `specs/2026-07-12-liero-rs-step4-input-replay-overview.md`: rewrite the **4f** and **4g**
      bullets to **LANDED** (what shipped vs deferred), and confirm **done-when §6** is met (run-skill
      drives the replay loop; the replay-checksum regression runs in CI). Use the real date
      (2026-07-13), not a frozen prefix.

### T4 [Opus] — STEP-4 FINAL REVIEW (broad; the whole PR #5, for John's merge decision)

This is the 4f/4g broad review **and** the Step-4-close review across the whole branch. Checklist:

- [ ] **4f/4g diff invariant:** changes confined to `rust/game/*`, `rust/shot/*`, any new
      default-match fixture, `.claude/skills/liero-shot/SKILL.md`, and `docs/`. No `sim`/`render`/
      `scenario`/`replay` behavior change; wasm `resolve_scenario` arm byte-unchanged.
- [ ] **Every slice's done-when met:** 4a (live input) · 4b (record→replay round-trip bit-exact) ·
      4c (audio, `HashGameState` isolation) · 4d (live shake/flash/banners) · 4e phase-1 (`.lrp`
      `WideRollbackChecksum` gate) · **4f** (bare-invoke playable + restart + quit + respawn) · **4g**
      (run-skill drives replay; CI regression wired). Spot-check each against the overview bullets.
- [ ] **CI green** on the full branch: `--workspace --exclude game` (sim/scenario/render/replay/
      oracle-tests, incl. `replay/tests/corpus.rs`) + `-p game` (incl. `passthrough`/`round_trip`/
      `record_regression`/`viewport_stepping`) + the wasm build job. No new red.
- [ ] **PROGRESS ↔ overview ↔ specs consistency:** every slice bullet LANDED and truthful; the
      %/tree matches reality.
- [ ] **Deferral track complete and explicit:** `GgrsSchedule`/rollback/netplay (Step 5) ·
      `.lrp` **phase 2** (cereal `Game` graph + `framehash` diff — the item to surface at merge) ·
      `.lrp` writing · full menu / weapon-select / `*State.cpp` · random level / `GenerateFromSettings`
      · gamepad · live-wasm (browser input) · steerable centering · `YoureIt`/GameOfTag banner ·
      modern color / spectator / interpolation / follow-cam. None silently dropped.
- [ ] **No commit trailers / no AI taglines** on any `%B` in the range; all commits on
      `liero-rs-step-4`.
- [ ] Produce the merge-readiness verdict + the one explicit question for John: **does `.lrp` phase 2
      land before merge, or as a post-step follow-on?** (overview done-when §5, §Open Q1).

## Risks (carried from design §3)

- Default-mode flip losing the free self-check on the bare command — mitigated by the retained
  Scripted `<name>` path + the CI `passthrough` gate (design §1.5).
- Wasm parity witness — the flip is native-only; the wasm arm is a standing tripwire, re-checked in T4.
- Restart-key collision (`R` = P0 fire) — verified-unbound `F5`/`Backspace`; check `KeyCode` spelling.
- Default-match fixture must spawn **two visible worms** (else unplayable) — verified in T0.
- `shot --scenario-path` must keep the golden path byte-identical — additive-only, unit-pinned (T1).
- Scope creep into the deferred `*State.cpp`/menu/level-gen surface — the top 4f risk; resist.
