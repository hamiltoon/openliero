# Step 2, Slice 6 — Full `ProcessFrame` + game modes + chain-loop + >1000-tick fuzz: Design

Status: **draft for review** · 2026-07-09
Part of: `2026-06-28-liero-rs-step2-overview.md` (slice 6 = the final slice)
Detailing: the "Full `ProcessFrame` integration + game mode" step of the overview's slice ordering.

This is the LAST slice of Step 2. After it merges, the Rust `sim` crate reproduces the C++
`Game::ProcessFrame` **in full** — every hashed/RNG-bearing subsystem in the exact C++ order —
and a >1000-tick two-worm fuzz (mirroring `test_determinism.cpp`) matches the C++ oracle's
master `HashGameState` + all 9 `HashGameComponents` tick-for-tick. Then PR #3 merges and Step 2
is complete.

---

## Goal

1. Close the `ProcessFrame` gap: port the **two remaining hashed/RNG-bearing pieces** the Rust
   driver (`state.rs:process_frame`) and the C++ subset dumper (`sim_physics_dump.cpp`) both
   lack — the **ninjarope `Process` loop** (`game.cpp:368-370`) and the **game-mode switch**
   (`game.cpp:372-461`) — and formally skip the three render-only pieces with an inertness
   argument (the `ProcessSight`/4d precedent).
2. Port the deferred **chain-loop** (`sobject.cpp:217-227`): an explosion re-triggers bonuses
   in its `detect_range`, threading the bonus pool through `sobject_create`.
3. Turn on `new_object_reuse` full-pool overwrite for `sobjects`/`wobjects` (bobjects/nobjects
   got it in 5d) so the fuzz's long run cannot panic on a full pool.
4. Land the closing milestone: a **multi-seed, >1000-tick, 2-worm fuzz** on a loaded level,
   input model `input_rng() & 0x7f`, bit-exact vs the extended C++ oracle.
5. Triage the 10 accumulated deferrals: each goes IN slice 6 or is explicitly deferred past
   Step 2 with a rationale.

## Architecture

Extend `rust/sim/` (deps unchanged: `sim-core`, `assets`; Bevy-free, float-free) and
`src/tools/oracle_dump/sim_physics_dump.cpp` (the ONE dumper). The dumper's per-tick driver is
promoted from a ProcessFrame-**subset** to the ProcessFrame-**full-tail** (add the two hashed
pieces after the worm loop; keep the render/stats pieces omitted as hash-inert). Goldens are
generated locally/manually via the rebuilt dumper (`OPENLIERO_BUILD_ORACLE_DUMP`,
`CMakeLists.txt:388-389`); CI runs the committed goldens under `cargo test --workspace`.

**BASE for T0:** current `liero-rs-step-2` HEAD (`ed72de0`; slices 1–5′ + T10 shipped).

---

## 1. The `ProcessFrame` gap analysis

The verified C++ order is `Game::ProcessFrame` (`game.cpp:267-471`). The table below is every
step, its C++ lines, whether it touches **hashed** state (`stateHash.hpp` — master
`HashGameState` and/or the component fold) or draws **RNG**, and the port-or-skip verdict.
"PORTED" = already in both `state.rs:process_frame` (1298-1884) and the dumper
(`sim_physics_dump.cpp:328-400`).

| # | Step | C++ lines | Hashed? | RNG? | Status / verdict |
|---|------|-----------|---------|------|------------------|
| 1 | `stats_recorder->PreTick` | 269 | no | no | SKIP — stats, not sim (dumper installs a no-op `StatsRecorder`, `:230`). |
| 2 | `--screen_flash` | 271-273 | **no** | no | **MISSING → SKIP.** `screen_flash` is in `GameSnapshot`, NOT in `HashGameState` (grep `stateHash.hpp` — absent). Inert like `ProcessSight` (4d). |
| 3 | viewport / spectator `shake -= 4000` | 275-285 | no | no | **MISSING → SKIP.** Render-only; the dumper has no viewports. |
| 4 | bonuses `Process` loop | 287-290 | yes | yes | PORTED (5c). |
| 5 | banner gate `(cycles&1)==0` → `banner_y` | 292-332 | no | no | **MISSING → SKIP.** Render-only; iterates (empty) viewport lists. |
| 6 | sobjects `Process` | 334-337 | yes | yes | PORTED (4c). |
| 7 | wobjects `Process` | 339-342 | yes | yes | PORTED (4a). |
| 8 | nobjects `Process` | 344-347 | yes | yes | PORTED (4c/5a). |
| 9 | bobjects `Process` (`if !Process Free`) | 349-355 | yes | yes | PORTED (5a). |
| 10 | `++cycles` | 357 | yes (master only) | no | PORTED (4a). |
| 11 | bonus-drop RNG roll | 359-362 | yes | **yes** | PORTED (5c). |
| 12 | worms `Process` | 364-366 | yes | yes | PORTED (3/4/5*, incl. death/respawn/pickup). |
| 13 | **ninjaropes `Process` loop** | **368-370** | **yes** | **yes** | **MISSING → PORT.** See §2. |
| 14 | **game-mode switch** | **372-461** | **yes** (GameOfTag/Holdazone `timer`; Scales `health` via `DoDamage`) | **yes** (Holdazone `SpawnZone`) | **MISSING → PORT (scoped).** See §5. Inert for every existing golden (all KillEmAll → `default: break`, `:459`). |
| 15 | `ProcessViewports` + store `prev_control_states` | 463-468 | no | no | **SKIP.** `ProcessViewports` is render (viewport follow). `prev_control_states` is NOT hashed (only `control_states.Pack()` is, `stateHash.hpp:36`); the Rust `PressedOnce` read-and-clear model (state.rs:121) already subsumes the edge, so the store is inert. |
| — | `stats_recorder->Tick` | 470 | no | no | SKIP — stats. |

### Headline

**5 missing pieces; 2 are hashed/RNG-bearing** (the ninjarope loop and the game-mode switch),
**3 are render/snapshot-only** (screen_flash, viewport shake, banner) and are skipped with the
proof that they are absent from both `HashGameState` and `HashGameComponents` — the same
inertness argument that retired `ProcessSight` in 4d. Everything else in the tick is already
bit-exact.

---

## 2. Ninjarope port design (the big one)

The rope is a per-worm object whose `out` + `pos.{x,y}` **are hashed** (`stateHash.hpp:47-49`).
Its `Process` (`ninjarope.cpp:7-82`) mutates the rope `pos`/`vel` **and the owner/anchor worm
`vel`** (hashed) and, on a dirt attach, **draws `rand(128)` ×11 + spawns 11 nobjects**
(`ninjarope.cpp:41-45`). It runs as its own loop AFTER the worm loop (`game.cpp:368-370`),
BEFORE the game-mode switch.

Today the Rust throw (`control.rs:288-294`) writes only the hashed `out`/`pos` and **skips**
(the "OQ5" skip) the non-hashed `vel`/`attached`/`length`; no `Process` runs, so a thrown rope
stays frozen. Slice 6 completes it:

### 2a. Datamodel (state.rs `Ninjarope`, `worm.hpp:19`)

Add the non-hashed fields the port reads/writes across ticks (defaults = C++ ctor):
`vel: Vec2`, `attached: bool` (default false), `length: i32`, `cur_len: i32`,
`anchor: Option<usize>` (C++ `Worm* anchor`; a worm **index** or None). Only `out`/`pos` stay
hashed — adding these leaves the hash contract unchanged.

### 2b. Un-skip the throw (`control.rs:process_tasks`, C++ `worm.cpp:962-986`)

Under Change-held, restore:
- the Up/Down `length` adjust + clamp (`:964-972`: `-= NRPullVel` / `+= NRReleaseVel`, then
  `max(_, NRMinLength)`, `min(_, NRMaxLength)`);
- on `PressedOnce(kJump)` (`:975-985`): `attached = false`, `vel = (cossin[Ftoi(aiming_angle)].x
  << NRThrowVelX, cossin[Ftoi(aiming_angle)].y << NRThrowVelY)`, `length = NRInitialLength`. The
  sound (`:979`) stays omitted. `process_tasks` gains `&cossin` + the NR* constants.

### 2c. Port `Ninjarope::Process` (`ninjarope.cpp:7-82`) as `ninjarope_process`

A loop after the worm loop over `worms` by index (so the enemy/anchor worm `vel` is reachable —
same index-not-`iter_mut()` discipline as `begin_respawn`, state.rs:1635). Under `out`:
1. `pos += vel`; `ipos = Ftoi(pos)`; recompute `anchor` by scanning the OTHER worms with the
   already-ported `check_for_spec_worm_hit(game, ipos.x, ipos.y, 1, w)` (nobject.rs:256).
2. `kDiff = pos - owner.pos`; `kForce = (kDiff.x << NRForceShlX)/NRForceDivX,
   (kDiff.y << NRForceShlY)/NRForceDivY)`; `cur_len = (VectorLength(Ftoi(kDiff.x),
   Ftoi(kDiff.y)) + 1) << NRForceLenShl` (`sim_core::math::vector_length`).
3. **Terrain-attach branch** (`ipos` at level edge OR `Mat(ipos).DirtRock()`): if `!attached`,
   set `length = NRAttachLength`, `attached = true`, and if `Inside(ipos) && Mat(ipos).AnyDirt()`
   spray **11 × [`rand(128)` → `nobject_types[2].Create2(...)`]** (`:41-45`) — the RNG burst;
   then `vel.Zero()`.
4. **Worm-anchor branch** (`else if anchor`): attach; if `cur_len > length`,
   `anchor.vel -= kForce / cur_len`; then `vel = anchor.vel`, `pos = anchor.pos`.
5. else `attached = false`.
6. Tail: if attached and `cur_len > length`, `owner.vel += kForce / cur_len`; else `vel.y +=
   NinjaropeGravity` and if `cur_len > length`, `vel -= kForce / cur_len`.

All fixed-point shifts/divides are integer/`wrapping`. The `kForce / cur_len` divides are
truncating per-component (`fixedvec::operator/`). `Create2`'s own RNG order (5a) is reused.

### 2d. Borrow shape

The loop needs `worms` (owner + anchor, two slots), `nobjects` (spray), `nobject_types`,
`cossin`, `level`, `rand` — all disjoint from the already-consumed worm-loop borrows because it
is a **separate loop after** it. Index `worms` directly; hand `check_for_spec_worm_hit` the
anchor candidate's sprite via `worm_sprites` (as the in-flight arms do).

---

## 3. Chain-loop port (`sobject.cpp:217-227`, deferral #1)

The tail of `SObject::create` scans **all bonuses**; any bonus whose `Ftoi(pos)` is within
`detect_range` is **freed** and replaced with `sobject_types[0].Create(...)` at its position
(`:224-225`) — an explosion detonating nearby crates. Rust's `sobject_create`
(sobject.rs:100-102) omits it ("needs the bonus pool + recursive `Create`").

**Design:** thread `&mut Pool<Bonus>` into `sobject_create`, and after the damage block scan
`bonuses` in slot order; for each in range, `bonuses.free(slot)` then recurse
`sobject_create(&sobject_types[0], ix, iy, ...)`. Every caller of `sobject_create` (blow_up in
the wobjects loop; worm death; the pickup booby; nobject explode) must forward `&mut bonuses`.
Because the recursion also takes `&mut bonuses`, this is one `&mut` moving down a call chain
(no aliasing) — the "borrow conflict deferred since 5c" dissolves once the pool is a parameter
rather than a field captured elsewhere. Recursion terminates: each level frees ≥1 bonus (a
finite pool) before recursing, and `sobject_types[0]`'s own scan sees a strictly smaller pool.
**Remove the chain-loop tripwire.** Note: the sobject `chain_explosion → BlowUpObject` recursion
(the O9 `debug_assert!`, sobject.rs:273) is a **different**, weapon-object chain — keep it
deferred unless the fuzz's weapon set reaches it (re-confirm at T9).

## 4. `new_object_reuse` for sobjects/wobjects (deferral #2)

`sobject_create`/`worm_fire` today `.expect("pool not full")`. In a >1000-tick fuzz with many
explosions those pools (700/600) can fill. C++ `FixedObjectList::NewObjectReuse` overwrites the
oldest slot; bobjects/nobjects already do this (5d). Port the same overwrite-on-full for
`sobjects` + `wobjects` so `spawn` never fails. The overwrite order is part of the `All()`
iteration contract already modelled by `Pool`.

---

## 5. Game-mode scope decision (deferral #6) — JOHN-BESLUT #1

`game.cpp:372-461` switches on `settings->game_mode`. Touchpoints:

- **KillEmAll** (`kGmKillEmAll`): `default: break` — no per-tick work. **Fully exercised** by
  the fuzz. Already the only mode any golden uses.
- **GameOfTag** (`:373-388`): a rand-free per-tick `++last_killed_by->timer` gate; `timer` **is
  hashed**. Small.
- **Holdazone** (`:390-458`): per-tick holder `++timer` (hashed) AND `SpawnZone`
  (`game.cpp:490+`) which calls `level.SelectSpawn(rand, ...)` — **RNG-bearing** and a
  self-contained sub-port (zone rect geometry + a level-reading RNG search, cousin of the
  respawn search).
- **ScalesOfJustice** (`kGmScalesOfJustice`): NOT in this switch; its only hashed effect is the
  `DoDamage` redistribution (`game.cpp:570-587`, heals other worms — `health` hashed), already
  flagged DEFERRED in `state.rs:do_damage` (426-431).

**Recommendation:** ship **KillEmAll fully fuzzed + bit-exact** (the milestone). Add a
`game_mode` dumper directive and port the **cheap hashed hooks** for **GameOfTag** (timer gate)
and **Scales** (the `DoDamage` redistribution), each with **one small scripted golden**. **Defer
Holdazone's `SpawnZone`/`SelectSpawn` past Step 2** with the same rationale the overview already
applies to random level generation ("consumes RNG heavily; out of Step 2's critical path") — it
is a bounded, separately-testable RNG sub-project, and no shipped TC uses Holdazone.

**This is a genuine scope call → JOHN-BESLUT #1.** The alternative extremes: (a) KillEmAll-only,
all three others deferred (leanest, but "game modes" in the mandate goes unaddressed); (c) all
four fully ported + golden-per-mode incl. Holdazone SpawnZone (most complete, but pulls a
level-search sub-port into the final slice). The recommendation is the middle path.

---

## 6. Dumper-extension decision + prior-golden transparency — JOHN-BESLUT #2

**Recommendation: extend the single subset dumper to the full `ProcessFrame` tail**
(`sim_physics_dump.cpp`): after the worm loop, add (a) the ninjarope `Process` loop
(`game.cpp:368-370`) and (b) the game-mode switch (`game.cpp:372-461`), plus a `game_mode`
scenario directive (default `kGmKillEmAll`). Keep screen_flash / viewport shake / banner /
`ProcessViewports` OMITTED — they are provably hash-inert (§1) and pulling in viewport/stats
machinery would add noise. This keeps ONE dumper and ONE Rust `process_frame`, both now running
the full frame.

### The transparency cost — prior goldens are NOT all byte-identical

This is the first slice that **cannot** claim the literal "git diff empty" prior-slice gate,
because adding the ninjarope loop changes the hash on any tick where a rope is out and evolving:

- **Game-mode switch: inert for every existing golden.** All prior scenarios are KillEmAll →
  `default: break`. Adding the switch changes nothing. ✔ byte-identical.
- **Ninjarope loop: changes exactly the rope-throwing goldens.** `sim_slice3` throws the rope
  under scripted input (its scenario, ticks 93-97, input `96 = Change|Jump`), so its golden
  **will change** from the first rope-out tick onward. The random-input fuzzes must be
  re-diffed to see which press `Change|Jump` (the 5′ DART-duel fuzzes use scripted aim+fire with
  no Change/Jump, so likely unaffected; the 5d death fuzzes use random 7-bit input and may
  throw). Physics/weapon scenarios (2, 4a-d, 5a-c) never press Change+Jump → byte-identical.

### The re-diff gate (T0, the 5b "cycles-ripple" precedent)

Regenerate ALL sim goldens with the rebuilt dumper and classify each:
- **byte-identical** (no rope throw + KillEmAll) — the majority; asserted `git diff` empty.
- **diverges only from tick T onward**, where T is the scenario's **first rope-out tick**, with
  the pre-T prefix proven byte-identical. This is a *surgical, explained* diff — exactly the
  discipline 5b used when `++cycles` rippled the master column. Each such golden is regenerated
  and its diff-onset tick documented in the done-report.

### Why not an opt-in `[full_frame]` dumper flag (to preserve priors)?

Rejected: the Rust side is a **single** `process_frame` function. Once it runs the full
ninjarope, the rope-throwing goldens diverge *regardless* of any dumper flag — so a flag cannot
preserve `sim_slice3` unless the Rust ALSO gates the ninjarope per-scenario, which would leave
slices 1–5′ running a knowingly-incomplete frame forever. For the FINAL "full ProcessFrame"
slice that is the wrong trade. Hence the honest full-regen with the surgical-diff gate. **Flagged
as JOHN-BESLUT #2** because it breaks the byte-identical-priors streak, and John values that
transparency discipline.

---

## 7. The >1000-tick fuzz design (the closing milestone)

Mirror `test_determinism.cpp`'s `DualGameFixture` loop (`:14-123`) in **structure**, matching
our extended dumper as the oracle (not test_determinism's literal hashes — different level, no
`StartGame`).

- **Input model:** per tick, per worm, `input_rng() & 0x7f` then `Unpack` — verbatim from
  `test_determinism.cpp:97-99`. A dedicated `input_rng` seeded independently of the game `rand`.
- **Seeds:** the game `rand` seed × a few values (e.g. `42`, `1337`, `99999`) × an `input_rng`
  seed — 3–5 fuzz variants, each its own scenario + golden.
- **Tick count > 1000** (test_determinism uses 1000): e.g. **1500**, enough for multiple
  death→respawn cycles per worm and to fill/reuse pools.
- **Worms seeded DEAD** exactly as the fixture (`visible = 0`, `killed_timer = 150`,
  `pos = (0,0)`, health `settings->health`, `lives` high). They enter play via the ported
  `BeginRespawn`/`DoRespawning` on ~tick 150 — which **maximally exercises the canonical desync
  trap** (the level-reading RNG respawn search) AND, crucially, sets `aiming_angle ∈ {32, 96}`
  via `rand()&1` (`worm.cpp:799-805`, ported at `state.rs:2163-2167`) so `Ftoi(aiming_angle)` is
  never 0 → **cossin[128] is unreachable** (see §8).
- **Level:** a **loaded** fixed `.lev` (NOT `GenerateFromSettings` — the deliberate divergence
  from test_determinism, per the overview's "random generation is out of Step 2's critical
  path"). Use a real shipped Liero arena so respawns, bonuses and duels have room; the scenario
  `level` directive is the single source.
- **Bonuses live:** `max_bonuses = 4` (in-game default) so drop/fall/pickup/expire + the
  chain-loop all fire over the run.

**Expected coverage (assert non-vacuously, from driven `SimState`, never re-parsed from the
golden):** ≥1 death (some worm `health→0`, `lives` decremented), ≥1 respawn (`BeginRespawn` RNG
search ran — `rng` column moved on a respawn tick), ≥1 bonus drop + ≥1 pickup (`bonuses` pool
grows then shrinks), ≥1 in-flight worm hit, ≥1 ninjarope throw+attach (`rand(128)` burst → a
`nobjects` bump on a rope-attach tick), and pool pressure (blood storm drives `nobjects`/blood
near cap, exercising `new_object_reuse` for at least one pool). Master + all 9 components
bit-exact every tick.

---

## 8. cossin[128] OOB resolution (deferral #3)

The T10 report (`.superpowers/sdd/step2-slice5prime-task-T10-report.md:51-66`) found: an un-aimed
worm that walk-flips to `dir1` keeps `aiming_angle = Itof(128)`; firing then indexes
`cossin[128]` (`cossin_table[128]`, `math.cpp:5` — length 128) — C++ UB, Rust panic.

**Resolution: the fuzz design sidesteps it structurally, so no guard is needed for the
milestone.** Because the fuzz seeds worms DEAD and lets them respawn (§7), every worm enters play
with `aiming_angle ∈ {32, 96}` (never 0). The dir-flip maps `32↔96` (`128 − 32 = 96`,
`128 − 96 = 32`) and the aim clamp holds `[64,116]`, so `Ftoi(aiming_angle) ∈ [12,116]` always —
`cossin[128]` cannot be reached. The panic is reachable ONLY via the artificial dumper path that
hand-sets `aiming_angle = 0` (never real gameplay). **Recommend DEFER the residual hardening past
Step 2** as a unit-test-only robustness note (a targeted clamp/guard in `worm_fire` with its own
test), because it is unreachable by the faithful fuzz and by real play. If John prefers belt-and-
suspenders, a one-line clamp + unit test can land in slice 6 (cheap) — noted as a minor option,
not a blocker.

---

## 9. Deferral triage (all 10 items)

| # | Item | C++ ref | Verdict | Rationale |
|---|------|---------|---------|-----------|
| 1 | Chain-loop (explosion re-triggers bonuses) | `sobject.cpp:217-227` | **IN (T3)** | Mandate lists it; borrow dissolves once `bonuses` is a `sobject_create` parameter (§3). |
| 2 | `new_object_reuse` for sobjects/wobjects | `FixedObjectList` | **IN (T4)** | The long fuzz fills these pools; overwrite-on-full prevents a panic and matches C++ (§4). |
| 3 | cossin[128] OOB | T10 report | **DEFER (residual)** | Unreachable in the respawn-seeded fuzz (§8); optional cheap guard. |
| 4 | T8 truncating-division test gap (`settings_health` ∤ 100) | `worm.cpp:295` pickup | **IN (T7)** | Cheap unit test; the fuzz can also pick a non-100 `settings_health`. |
| 5 | Non-default `loading_time` golden | `settings.hpp:79` | **DEFER (unit-tested) / minor** | test_determinism uses `loading_time = 0`; the multi-tick reload countdown is unit-tested. Optionally one scripted golden variant with `loading_time > 0`. |
| 6 | Scales/GameOfTag/Holdazone modes | `game.cpp:372-461`, `570-587` | **PARTIAL (T5) — JOHN-BESLUT #1** | KillEmAll fuzzed; GameOfTag+Scales hooks + goldens; Holdazone SpawnZone deferred. |
| 7 | T4 pathological multi-worm destroy+explode ordering | (Rust defers `do_remove`) | **IN (re-confirm, T10)** | Empirically closed if the >1000-tick multi-worm fuzz with explosions passes bit-exact; else investigate. |
| 8 | `physics_fall_test.lev` relocation out of `data/` | — | **DEFER (pre-release chore)** | Not correctness; a packaging cleanup. |
| 9 | `Pool::spawn` O(n) → free-list | — | **DEFER (perf)** | Correctness-neutral; revisit only if the fuzz is too slow (note at T9). |
| 10 | weap_order unstable-sort vs Rust stable-sort | (weapon resolve) | **DEFER (documented)** | Unambiguous while weapon names are unique — true for the openliero TC; note for the fuzz's weapon set. |

---

## 10. JOHN-BESLUT summary

> **DECIDED (John, 2026-07-09): all three per the recommendations.** #1 KillEmAll fuzzed +
> GameOfTag/Scales hooks with one golden each, Holdazone `SpawnZone` deferred past Step 2.
> #2 full-regen with the surgical component-identity re-diff gate (the byte-identical-priors
> streak is consciously broken; the gate must PROVE each changed golden's pre-rope-onset
> prefix byte-identical). #3 cossin[128] guard deferred past Step 2 (unreachable in a
> faithful fuzz); T6's optional guard sub-item is OUT.

- **#1 — Game-mode scope.** Recommend: KillEmAll fully fuzzed + bit-exact; GameOfTag + Scales
  hashed hooks with one golden each; **Holdazone `SpawnZone` deferred past Step 2**. Alternatives:
  KillEmAll-only (leaner), or all four incl. Holdazone (most complete).
- **#2 — Dumper full-regen vs preserve-priors.** Recommend: extend the one dumper to the full
  frame and **full-regen** all sim goldens, with the surgical component-identity re-diff gate
  (5b precedent); accept that rope-throwing goldens (definitely `sim_slice3`, TBD fuzzes) diverge
  from their first rope-out tick. This breaks the byte-identical-priors streak — flagged for
  John's awareness. The opt-in-flag alternative is infeasible (single `process_frame`).
- **#3 (minor) — cossin[128] guard now or defer.** Recommend defer (unreachable in the fuzz);
  a cheap clamp+test can land if preferred.

## 11. Residual risks

- **RNG order in `ninjarope_process`** — the terrain-attach `rand(128)` ×11 must land at the
  exact `:41-45` point (after the `pos+=vel`/anchor scan, inside the `!attached` guard), and the
  loop must run AFTER worms / BEFORE game-mode. A misplaced draw desyncs everything downstream.
- **Full-regen honesty** — the re-diff gate must PROVE each changed golden's pre-onset prefix is
  byte-identical, not merely regenerate blindly.
- **Fuzz determinism of coverage** — the coverage asserts must derive from the driven state, and
  the seeds must be chosen so each asserted event (death/respawn/pickup/hit/rope-attach/pool-cap)
  actually occurs; tune with `OL_PHYS_TRACE` before committing goldens.
