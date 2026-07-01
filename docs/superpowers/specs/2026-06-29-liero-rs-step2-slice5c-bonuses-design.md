# Step 2, Slice 5c — Bonuses (the bonus pool goes live)

Status: **draft for review** · 2026-06-29
Part of: `2026-06-29-liero-rs-step2-slice5-object-families-overview.md`
Follows: `2026-06-29-liero-rs-step2-slice5b-damage-blood-design.md`
(the proven `sim` crate, `oracle_dump_sim_physics` with its `weapon <slot> <name>
[ammo]` directive, the `process_frame` driver with the **object loops + `++cycles`
both live since 5b**, `sobject_create`/`NObject::Create2`, the blood/bobjects pool —
all live since 4a–5b.)

## Purpose

5c makes the **`bonuses` pool go live**: the per-tick **bonus-drop roll**
(`rand(c[CBonusDropChance])`, `game.cpp:359-362`) starts (conditionally) consuming RNG
each tick; `CreateBonus` (`game.cpp:216-265`) spawns a bonus via an RNG position search;
`Bonus::Process` (`bonus.cpp:6-35`) makes the bonus fall, bounce, and expire; and the
**bonuses driver loop** (`game.cpp:287-290`) drives them. It is the first slice where the
`bonuses` master/component column goes non-empty.

5c follows immediately from 5b: 5b advanced `cycles` and put the tick tail in place, and
the bonus-drop roll lives **between `++cycles` (`:357`) and the worm loop (`:364`)** — so
its insertion point is ready. 5c wires that roll, the bonus Process loop, `CreateBonus`,
and `Bonus::Process`. **Pickup** (`worm.cpp:287-322`) and the recursive **bonus
chain-loop** (`sobject.cpp:217-227`) are scoped by whether 5c's scenario reaches them
(see Scope / Open questions) — the thin recommendation **defers both with tripwires**.

### What changes vs Slice 5b

| Invariant | 5b | **5c** |
|---|---|---|
| bonus-drop roll (`game.cpp:359-362`) | excluded from the dumper | **wired** into dumper + Rust `process_frame`, **gated on `max_bonuses>0`** |
| `bonuses` pool | empty every tick | **non-empty** once a bonus drops (first time) |
| `Bonus::Process` + bonuses driver loop (`:287-290`) | dormant | **live** — fall / gravity / bounce / expire |
| `CreateBonus` (`game.cpp:216-265`) | not ported | **ported** — position search + frame/timer/weapon RNG |
| bonus pickup (`worm.cpp:287-322`) | deferred | **deferred (tripwire)** — thin path; see Scope |
| bonus chain-loop (`sobject.cpp:217-227`) | omitted, un-tripwired | **tripwire added** (or ported) — see Scope |
| C++ dumper | base `StatsRecorder` + `++cycles` | **+ bonus Process loop + bonus-drop roll + a `max_bonuses` directive (default 0)** |
| `SimState` | gained `blood: i32` | gains **`settings_max_bonuses: i32`** (= 0, like 5b's `blood`) |
| prior goldens (1–5b) | master regenerated (the `cycles` ripple) | **byte-identical (git diff empty)** — see below |

## ✅ The bonus-drop-roll question — TRANSPARENT, not a ripple (a `max_bonuses` gate)

**Finding: adding the bonus-drop roll does NOT change any prior golden (1–5b), provided
the dumper's `max_bonuses` defaults to 0. This is fully transparent — unlike 5b's
`cycles` fold, it needs NO golden regeneration and NO redefined regression gate.**

This is the one residual risk the brief flagged (like 5b's `cycles`). The evidence:

- **The roll draws RNG every tick *only when enabled*.** `game.cpp:359-362`:
  ```cpp
  ++cycles;                                                       // :357 (live since 5b)
  if (!common->h[HBonusDisable] && settings->max_bonuses > 0 &&
      rand(common->c[CBonusDropChance]) == 0) {                   // :359-360
    CreateBonus();                                                // :361
  }
  ```
  C++ `&&` **short-circuits left-to-right**: when `settings->max_bonuses == 0`, the
  `rand(c[CBonusDropChance])` term is **never evaluated** ⇒ **no RNG draw** ⇒ `rand.last`
  and every downstream column are untouched.
- **The dumper does not run the roll today** (`sim_physics_dump.cpp:13-27` header + the
  driver loop `:311-359` deliberately omits it; comment: *"we still EXCLUDE the
  bonus-drop roll, the bonuses loop, ninjarope, and the game-mode switch"*).
- **The dumper's `settings->max_bonuses` is the Settings ctor default `4`**
  (`settings.hpp:69` `int32_t max_bonuses{4}`); the dumper overrides `game_mode`,
  `lives`, `loading_time`, and `shadow` (`sim_physics_dump.cpp:195-204`) but **does NOT
  override `max_bonuses`**. And `h[HBonusDisable]` is **false** in the openliero TC
  (bonuses are enabled in normal play; `HBonusDisable` is referenced only in
  `game.cpp`). So a **naive unconditional port** of the roll into the dumper **WOULD
  draw every tick for prior scenarios 1–5b** — a real ripple.

**Resolution (TRANSPARENT — recommended): gate via a `max_bonuses` scenario directive
that defaults to 0.** Add a `max_bonuses <n>` directive to the dumper's scenario parser
(default `0`), and have the dumper set `settings->max_bonuses = scn.max_bonuses`
(overriding the ctor's `4`). Mirror a `settings_max_bonuses: i32` field (= 0) on the
Rust `SimState` and gate the Rust roll on it identically. Then:

- **Prior scenarios 1–5b omit the directive ⇒ `max_bonuses == 0` ⇒ the roll
  short-circuits before `rand` ⇒ no draw ⇒ their goldens are BYTE-IDENTICAL** (all 11
  columns, master included — `cycles` already advances from 5b, and no new RNG enters).
  The 5a/5b literal Definition-of-Done gate *"slices 1–N goldens byte-identical (git
  diff empty)"* **holds again**.
- **5c's scenario sets `max_bonuses <n>` (n>0) ⇒ the roll fires** and the bonus machinery
  engages.

This is strictly better than 5b: the shared single `process_frame`/dumper driver gains
the roll code, but it is a provable no-op for priors. **No controller golden-regen gate
is required.** (Rejected alternative — forking the driver to omit the roll for priors —
loses the single-shared-driver property and is unnecessary given the short-circuit.)

The one pre-T0 controller item is **scope** (pickup + chain-loop), not goldens — see
Open questions.

## Scope

### IN — ported this slice

- **The bonus-drop roll** (`game.cpp:359-362`) in both the dumper (after `++cycles`,
  before the worm loop) and Rust `process_frame` (after `*cycles += 1` at `state.rs:1051`,
  before the worm loop at `:1053`), gated `!HBonusDisable && settings_max_bonuses > 0 &&
  rand(c[CBonusDropChance]) == 0`. The gate's `max_bonuses` term comes from the new
  `settings_max_bonuses` field/directive (default 0); `HBonusDisable` is a TC hidden flag
  (false for openliero) — model it as a const read (already false), or fold it into the
  gate as `!h_bonus_disable`. **One `rand(CBonusDropChance)` per tick when enabled.**
- **`CreateBonus`** (`game.cpp:216-265`), verified RNG order:
  - `:219` early-out `if (bonuses.Size() >= max_bonuses) return;` — **no rand before
    this**.
  - search loop ≤ 50000 trials (`:223`): per trial `rand(BonusSpawnRectW)` then
    `rand(BonusSpawnRectH)` (**2 draws/trial**); `+= BonusSpawnRectX/Y` iff
    `h[HBonusSpawnRect]`; `CheckBonusSpawnPosition` (`:200-214`, a 5×5 box
    `[x-2,x+3)×[y-2,y+3)` rejecting any `DirtRock` pixel, **no rand**).
  - on a valid position: `frame = rand(2)` (unless `h[HBonusOnlyHealth]`→1 /
    `HBonusOnlyWeapon`→0); `bonuses.NewObject()`; `x/y = Itof(ix/iy)`, `vel_y = 0`;
    `timer = rand(bonus_rand_timer[frame][1]) + bonus_rand_timer[frame][0]` (**1 draw**);
    `weapon = 0`; **for weapon bonuses (`frame==0`)** a reject loop
    `do { weapon = rand(weapons.size()) } while (weap_table[weapon] == 2)` (**variable
    draws** — a desync-sensitive reject loop like the respawn search); then
    `sobject_types[7].Create(ix, iy, 0, nullptr)` (the spawn flash — an sobject create,
    whose own RNG, if any, folds into the stream).
  - **The trial count depends on the live level** (DirtRock rejection): a desync trap.
    5c's scenario MUST place the spawn rect over clear ground so the first trial succeeds
    deterministically (2 draws), bounding the search.
- **`Bonus::Process`** (`bonus.cpp:6-35`), **no direct RNG**: `y += vel_y`;
  `vel_y += BonusGravity` iff `Mat(ix, iy+1).Background()`; bounce on
  floor/`DirtRock(ix, newY)`: `vel_y = -(vel_y * BonusBounceMul) / BonusBounceDiv`, zeroed
  if `|vel_y| < 100`; `if (--timer <= 0) { sobject_types[bonus_s_objects[frame]].Create(
  ix, iy, 0, nullptr); if (used) bonuses.Free(this); }`. The **expiry sobject** Create
  may itself draw RNG depending on the configured type — fold it into the stream; the
  scenario keeps it over clear ground.
- **The bonuses driver loop** (`game.cpp:287-290`): `for each bonus in bonuses.All():
  i->Process(*this)`. **This runs at the TOP of `ProcessFrame`, BEFORE the
  sobject/wobject/nobject/bobject loops** — not in the tick tail. In Rust, insert it at
  the very start of `process_frame`'s object section (before the sobjects loop at
  `state.rs:924`). The `bonuses` pool is `ExactObjectList<Bonus,99>` — **lowest-free-index
  allocate, free-by-slot** (NOT swap-remove); the Rust `Pool<Bonus>` already has these
  semantics (`pool.rs:7-9,24-27`).

### OUT — deferred this slice (thin recommendation; controller may pull pickup IN)

- **Bonus pickup** (`worm.cpp:287-322`, inside the `if (visible)` worm body) → **deferred
  with a tripwire.** Pickup runs *inside the worm loop* and adds worm-loop RNG: per bonus
  in an 11×11 box (`±5`), a **health bonus** (`frame==1`, only if `health < settings_health`)
  draws `rand(BonusHealthVar)` then `DoHealing(... +BonusMinHealth)*health/100)`
  (1 rand); a **weapon bonus** (`frame==0`) always draws `rand(BonusExplodeRisk)` — on
  `>1` reload (set `ww.type`/`ammo`, no further rand), else **booby**
  `sobject_types[0].Create` (explosion). If 5c's scenario keeps both worms away from the
  dropped bonus's 11×11 box, this path never fires; guard the worm-loop bonus branch with
  a `debug_assert!`-style tripwire so the first scenario that walks a worm onto a bonus
  trips it. (If the controller wants pickup IN, it becomes T5 — see plan.)
- **The recursive bonus chain-loop** (`sobject.cpp:217-227`) → **add a real tripwire**
  (it is currently omitted *without* a tripwire — the one such gap 5a/5b carried). After
  an explosion (`detect_range>0`), `for each bonus in the blast box: bonuses.Free(br);
  sobject_types[0].Create(... recurse ...)`. 5c threads the bonus pool into
  `sobject_create` and, **if 5c's scenario puts no bonus near an explosion**, adds a
  tripwire (`debug_assert!` on a non-empty bonus pool reached by a damaging sobject) and
  defers the port to slice 6; **if** the scenario does reach it (e.g. a chosen bonus
  expiry sobject has `detect_range>0`), port the recursion. The thin path assumes the
  bonus expiry sobject and spawn flash have `detect_range==0`, so the tripwire suffices.
- **Death / respawn** → 5d. **`HBonusOnlyHealth`/`HBonusOnlyWeapon`/`HBonusSpawnRect`
  branches** → keep them in the port for fidelity but the openliero TC leaves them false,
  so the scenario exercises the `rand(2)` frame + no-offset paths.

## Datamodel

- **`Bonus` already exists** (`state.rs:374-382`): fields `x, y, timer, weapon, frame`
  (the hashed set). It **lacks the runtime `vel_y`** (C++ `bonus.hpp:13` `fixed vel_y{0}`)
  and the **`used`** flag read at `bonus.cpp:31` — both **not hashed**, so adding them
  leaves the bonus hash and prior goldens unchanged. T1 adds `vel_y: i32` (Q16.16, like
  worm `pos`/`vel`) and models `used` (verify whether it lives on the C++
  `ExactObjectListBase`; default it to match real bonuses).
- **`bonuses: Pool<Bonus>` already exists** (`state.rs:636`), capacity `99`
  (`BONUS_CAPACITY`, `state.rs:37`) == C++ `ExactObjectList<Bonus,99>`. `Pool` is the
  **lowest-free-index / ExactObjectList** flavour (`pool.rs:7-9,24-27`) — the correct free
  semantics. **No `SimState` signature change for the pool.**
- **Hash fold already wired AND tested**: master folds `x, y, timer, weapon, frame`
  (`hash.rs:86`; test `state.rs:466-510`); component folds `x, y, timer, weapon`
  (**drops frame**, the O11-style asymmetry; `hash.rs:160-166`; test `state.rs:543`).
  Nothing to add.
- **`SimState.settings_max_bonuses: i32`** — the one new field (default `0`, the dumper
  override; the real `Settings` default is `4`). Threaded through every `SimState::new`
  caller (like 5b's `blood`); slices 1–5b stay byte-identical because the gate
  short-circuits at 0. Also need the bonus constants (`CBonusDropChance`,
  `BonusSpawnRectW/H/X/Y`, `BonusGravity`, `BonusBounceMul/Div`, `bonus_rand_timer`,
  `bonus_s_objects`) loaded from the TC, plus `weap_table` (for the weapon reject loop)
  and `HBonusDisable`/`HBonusSpawnRect`/`HBonusOnlyHealth`/`HBonusOnlyWeapon` hidden flags.

## Scenario + golden

- **`sim_slice5c_scenario.txt`** — seed chosen so the first bonus-drop roll hits `0`
  within the dumped ticks (`rand(CBonusDropChance) == 0`); `physics_fall_test.lev` (or a
  fixture with a wide clear band); a new **`max_bonuses 4`** directive (enables the roll);
  worms placed **away from any dropped bonus's 11×11 pickup box and away from any
  explosion** (thin path: no pickup, no chain-loop). Run enough ticks that (a) the roll
  fires and `CreateBonus` spawns a bonus over clear ground (first trial succeeds), (b) the
  bonus **falls/bounces** under gravity for several ticks (`bonuses` column moves), and
  (c) optionally the bonus **expires** (`timer` reaches 0 → expiry sobject + `Free`) if
  the run is long enough — else assert it survives. Keep the spawn rect over clear ground
  so the search is a single trial (deterministic 2 draws). **Tune the seed** so the drop
  lands mid-run (inspect the trace).
- **`golden/sim_slice5c.txt`** — N+1 rows × 11 columns. Expected signature: `rng` flat
  until the first drop tick, then **moves every tick** (the per-tick `rand(CBonusDropChance)`
  once `max_bonuses>0`); at the drop tick a **burst** (`rand(W)`+`rand(H)` search +
  `rand(2)` frame + `rand(timer)` [+ weapon reject loop if `frame==0`] + spawn-flash
  sobject draws); `bonuses` column **non-empty** from the drop tick, **moving** as the
  bonus falls (`y`/`vel_y`/`timer` change — note `vel_y` is not hashed but `y`/`timer`
  are); `level` may carve if the expiry/flash sobject digs (keep it clean); `worm` columns
  flat (no pickup); `sobjects` gains the spawn flash (+ expiry); master moves with `cycles`
  every tick. Inspect the numbers directly (4b/4c/5b discipline). **Plus** slices 1–5b
  re-run **byte-identical** (the transparency check — git diff empty).

## Difftest (the 5c milestone)

`sim_slice5c_golden.rs` — mirror `sim_slice5b_golden.rs`: expected parsed from the golden
(all 11 columns); actual from a genuinely driven `SimState` (real
`.lev`/`tc.cfg`/`Objects::load`, **weapon by name** if any, `id==index` for the object
tables, `SimState::new` full args incl. the new `settings_max_bonuses` set to the
scenario's value); components asserted **before** master; input keyed `k-1`; all ticks
incl. tick 0. Non-vacuous coverage guards from driven state:

- `bonuses` count **> 0** on at least one tick (the pool goes live — first time);
- the dropped bonus's `y` (hashed) **changes** across ticks (it actually falls — a real
  witness, not a frozen spawn);
- `rng` **moves on every tick ≥ the drop tick** (the per-tick roll is firing — proves
  `max_bonuses>0` took effect, not vacuous);
- both worms' `health` **unchanged** and `worm` columns flat (no pickup leaked into the
  thin scenario);
- `nobjects`/`bobjects` empty (or unchanged); `bonuses < 99` (the pool cap).

**Milestone:** master + all 9 component hashes bit-exact for every tick vs the C++
golden, **and** slices 1–5b re-run **byte-identical** (git diff empty — the transparency
proof).

## Definition of Done

1. `cargo test --workspace` green (incl. `sim_slice5c_golden` + slices 1–5b **unchanged**).
2. Dumper: bonus Process loop at the top of the driver (`game.cpp:287-290` point) + the
   bonus-drop roll after `++cycles` (`game.cpp:359-362` point) + a `max_bonuses <n>`
   scenario directive **defaulting to 0** that sets `settings->max_bonuses`. Rust
   `process_frame` mirrors both at the same points, gated on `settings_max_bonuses`.
3. `CreateBonus` ported (search + `frame`/`timer`/`weapon` RNG, incl. the weapon reject
   loop); `Bonus::Process` ported (fall/gravity/bounce/expire, no direct RNG);
   `CheckBonusSpawnPosition` ported (5×5 box, no rand).
4. **Prior-slice gate (restored to literal):** slices 1–5b goldens **byte-identical**
   (git diff empty) — the `max_bonuses==0` short-circuit makes the roll a no-op for them.
   NOT a regen (contrast 5b's `cycles` ripple).
5. `SimState.settings_max_bonuses` threaded; bonus pickup + the chain-loop remain deferred
   with tripwires (unless the controller pulls pickup IN — then it ships + a pickup
   scenario).
6. `Bonus` gains the runtime `vel_y` (+`used`); pool/hash unchanged (already present).
7. `sim` stays float-free + deps = `sim-core` + `assets` only.
8. Overview row 5c + the bonus open-questions (chain-loop tripwire, pickup scope) recorded
   resolved in the SDD ledger; the bonus-drop-roll TRANSPARENT finding noted.

## Open questions

- **Scope: pickup + chain-loop (the one item needing the controller before T0).** The
  thin recommendation **defers both with tripwires** (a clean 5c = drop + fall + expire,
  no worm-loop RNG, no explosion-adjacent bonus). The controller may instead pull **pickup
  IN** (a second scenario walks a worm onto a bonus → health/weapon/booby RNG in the worm
  loop) — a larger but self-contained slice. The chain-loop (`sobject.cpp:217-227`) at
  minimum gets a **real tripwire** this slice (it is currently the only omitted-without-
  tripwire path). The planner cannot adjudicate scope alone.
- **Expiry / spawn-flash sobject RNG.** `CreateBonus` spawns `sobject_types[7]` and
  `Bonus::Process` expiry spawns `sobject_types[bonus_s_objects[frame]]`. Confirm at T2/T3
  whether these configured sobject types draw RNG (dirt effect) or have `detect_range>0`
  (which would reach the chain-loop). The thin scenario assumes clean (no dig, no chain);
  verify against the openliero TC's bonus sobject configs and pick the scenario position
  accordingly.
- **`used` flag.** Confirm whether C++ `Bonus::used` lives on `ExactObjectListBase` and
  its default (`bonus.cpp:31` gates the `Free`). Model it to match; it is not hashed.

## Tasks

See the companion plan (`plans/2026-06-29-liero-rs-step2-slice5c-plan.md`).
Sketch: **T0** dumper (bonus Process loop + bonus-drop roll + `max_bonuses` directive
default 0) + **re-diff 1–5b byte-identical** (no controller golden-gate) → **T1** Rust
`settings_max_bonuses` field + `Bonus.vel_y`/`used` + the gated roll + bonuses driver loop
(empty pool no-op; 1–5b green) → **T2** `CreateBonus` + `CheckBonusSpawnPosition` (search
+ frame/timer/weapon RNG) → **T3** `Bonus::Process` (fall/bounce/expire) + expiry sobject
→ **T4** chain-loop tripwire (port iff scenario reaches it) → **T5** *(optional,
controller-gated)* pickup → **T6** scenario + gen + golden → **T7**
`sim_slice5c_golden` difftest (MILESTONE) → **T8** done-check + ledger.
