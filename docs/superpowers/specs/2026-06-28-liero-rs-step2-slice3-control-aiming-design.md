# Step 2, Slice 3 — Worm control + aiming (the rest of `Worm::Process` minus combat)

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice2-worm-physics-design.md`
(the proven `sim` crate, the `oracle_dump_sim_physics` per-tick oracle, the
scenario-file pipeline, the `process_worm_physics` driver this slice extends).

## Purpose

Slice 2 made the worm *fall* (terrain collision + gravity) and matched the worm
**component** hash tick-for-tick under **empty** input. Slice 3 makes the worm
*play*: it ports the rest of `Worm::Process` minus combat — **movement, aiming,
jump, weapon-change, weapon timers, ninjarope throw, direction** — driven by
**scripted input**, and flips on the **master `HashGameState`** assertion.

This is the slice where the master hash turns on. That is not optional polish:
the per-worm **component** hash reads only `{pos, vel, health, lives, visible,
timer}` (`stateHash.hpp:145-153`), so it cannot see `aiming_angle`,
`control_states.Pack()`, the per-weapon `delay_left`, or `ninjarope.out/pos`.
Everything Slice 3 ports is invisible to the component hash and visible **only**
to the master hash (`stateHash.hpp:25-50`). So matching the master per tick under
scripted input *is* the proof of this slice.

### What the C++ oracle already gives us (no C++ work this slice)

The Slice-2 dumper `oracle_dump_sim_physics`
(`src/tools/oracle_dump/sim_physics_dump.cpp`) already:

- drives the **unmodified** `worm->Process(game)` — i.e. the full control/aim/
  jump/weapon-change/weapon-timer logic, not a physics-only subset;
- already **parses `input <tick> <w0> <w1>` lines** and `Unpack`s them per worm
  each tick (`sim_physics_dump.cpp:116-124, 227-238`);
- already **dumps the master `HashGameState`** as the second column of every
  golden line (it was *carried un-asserted* in Slice 2).

So the master hashes for scripted input are **already produced** by the existing
dumper. Slice 3 is **all Rust** plus a new scenario + golden + test: there is no
new dumper, no CMake change, no change to determinism-critical C++ sim code.
(This is the single biggest scoping fact of the slice — see *Oracle / golden*.)

## Scope

### IN — ported to Rust this slice (C++ line references)

`Worm::Process` (`worm.cpp:210-452`) runs these in order under `visible == true`;
Slice 3 ports every row not already done and not deferred:

| `worm.cpp` | What | Slice 3 |
|---|---|---|
| 213 | `health = min(health, settings->health)` | IN (inert; health stays 100) |
| 221-283 | reaction orchestration → `reacts[4]` | **DONE (Slice 2)** — now also feeds `ProcessTasks` |
| 324 (def 1214-1241) | `ProcessSteerables` | IN as **no-op** (empty `wobjects` ⇒ loop body never runs; sets `steerable_count=0`, not hashed) |
| 326-330 | `movable` reset | IN |
| 332 (def 1003-1062) | `ProcessAiming` | **IN** |
| 333 (def 959-1001) | `ProcessTasks` (jump, ninjarope throw/retract, rope pull/release) | **IN** |
| 334 (def 811-848) | `ProcessWeapons` (weapon-timer countdown) | **IN** |
| 345 | `ProcessPhysics` | **DONE (Slice 2)** |
| 348-353 (def 1064-1098 / 850-957) | `ProcessWeaponChange` / `ProcessMovement` | **IN** (except the dig terrain body — see below) |

Plus the **control-state bit mutations** these paths perform
(`Press`/`Release`/`PressedOnce`), which are part of the master hash via
`control_states.Pack()` (`stateHash.hpp:36`). See *Control-state mutation*.

### OUT — deferred (C++ lines + target slice)

| `worm.cpp` | What | Why inert / deferred to |
|---|---|---|
| 285-322 | bonus pickup (reads `game.bonuses`, may `rand()`) | empty pool ⇒ skipped → **Slice 5** |
| 336-343 (def 1100-1148) | `Fire` gate + `Worm::Fire` (recoil, ammo, `delay_left`, `leave_shell` `rand()`, spawns wobject) | no `kFire` in scenario → **Slice 4** |
| 346 (def 1190-1212) | `ProcessSight` (laser raycast) | writes only `hotspot_x/y`, `make_sight_green` — **none hashed**, no `rand()` ⇒ **skip entirely** (→ Slice 4 with the laser weapon) |
| 893-948 | dig terrain body `DrawDirtEffect` (inside `ProcessMovement`) | draws `rand()` **and** mutates `material_id` ⇒ **deferred → Slice 4**; scenario never holds Left+Right (see *RNG decision*) |
| 355-367 | low-health smoke nobject (`rand()`) | `health == 100` ⇒ skipped → **Slice 6** |
| 369-426 | death: blood/splinters, `--lives`, respawn (`rand()`) | `health > 0` ⇒ skipped → **Slice 6** |
| 428-430 | animation `current_frame` update | not hashed ⇒ skip |
| 431-450 (def 711-809) | dead-worm branch (`BeginRespawn`/`DoRespawning`, `rand()`) | `visible == true` ⇒ skipped → **Slice 6** |
| (ProcessFrame-level) | `ninjarope.Process`, `cycles++`, bonus-drop RNG roll, object `Process` loops | not called by the worms-only driver → **Slice 6** |

> **`ProcessSight` decision (explicit, per the planning brief).** `ProcessSight`
> has **no hashed effect**: it writes `hotspot_x`, `hotspot_y`, and
> `make_sight_green`, none of which appear in `HashGameState` or
> `HashGameComponents`, and it draws no RNG. It runs in the dumper (it is part of
> the unmodified `Process`) but is invisible to both hashes, so the Rust port
> **omits it**. The full raycast lands in Slice 4 alongside the laser weapon.

> **Ninjarope `Process` is NOT run.** The driver calls `worm->Process` only, not
> `Game::ProcessFrame`, so the ProcessFrame-level `worm->ninjarope.Process` loop
> never runs (mirrored exactly by the dumper, `sim_physics_dump.cpp:233-238`).
> A thrown rope's `out`/`pos` are set at throw time and then **frozen** — which is
> all the master hash reads (`ninjarope.out`, `ninjarope.pos.x/y`,
> `stateHash.hpp:47-49`). Rope movement/attachment is Slice 6.

## RNG decision: still pristine, **and** level pristine — recommended

**Decision: keep Slice 3 RNG-pristine (`rand.last == 0` every tick) *and*
level-pristine (the level component hash stays constant), by scripting input that
never triggers a `rand()`-drawing or terrain-writing path.** This is the simplest
*honest* option (the planning brief asks for exactly this judgement call).

Every `rand()` reachable from `Worm::Process` lives in an **OUT** row above, and
every one is excludable by scenario construction:

| `rand()` site | Reached when | Excluded by |
|---|---|---|
| bonus pickup (`worm.cpp:295,299`) | a bonus overlaps the worm | pools empty (Slice 5) |
| dig `DrawDirtEffect` (`worm.cpp:931,941`) | `movable && kLeft && kRight && able_to_dig` | **scenario never holds Left+Right together** |
| `ProcessWeapons` shell drop (`worm.cpp:843-845`) | `leave_shell_timer > 0` | `leave_shell_timer == 0` always (only `Worm::Fire` sets it; no Fire) |
| low-health smoke (`worm.cpp:356-359`) | `health < health/4` (i.e. `< 25`) | `health == 100` |
| death/respawn (`worm.cpp:378,...,799`) | `health <= 0` / dead branch | `health == 100`, `visible == true` |

So with a scenario that (a) **never sets Left and Right on the same worm/tick**,
(b) **never sets Fire**, and (c) keeps `health == 100`, **no `rand()` is
consumed** and **no pixel is written**. Consequence:

- The `rng` component is a constant `00000000`; the master's `rand.last` term is a
  fixed `0` (re-asserted every tick) — same invariant Slice 2 proved.
- The `level` component hash is constant (== the Slice-1/2 value), re-asserted
  every tick. Digging is the *only* worm-path that writes the level, and it is
  excluded.

**`rand()` audit of the IN methods (all clean):** `ProcessAiming` — none;
`ProcessTasks` — none (jump, throw, pull/release draw no RNG); `ProcessWeapons` —
only the `leave_shell_timer>0` branch, never entered; `ProcessWeaponChange` —
none; `ProcessMovement` — only the dig body, excluded; `ProcessSteerables` —
none. So the IN set is RNG-free under the scenario.

**Deferred alternative (and why).** The dig path (`DrawDirtEffect`) is the one
worm-control path that *both* draws RNG *and* destroys terrain. Pulling it into
Slice 3 would force the RNG sequence **and** the level-hash time series to be
matched here, coupling control/aiming to the terrain-destruction machinery that
Slice 4 builds anyway (weapon craters use the same `DrawDirtEffect`). We therefore
**port the dig *control flow*** (the `able_to_dig` toggle, the `kLeft && kRight`
detection) but **defer the `DrawDirtEffect` body to Slice 4**, guarding the Rust
dig branch with a `debug_assert!` so an accidental Left+Right scenario panics
loudly rather than diverging silently (see *The hard 10%*).

## Master-hash flip-on + inertness verification

Slice 3 asserts the master `HashGameState` column per tick. For that to hold, the
OUT/deferred paths must be **inert under the scenario** so the master's hashed
fields evolve **only** via the ported methods. Audit of every master-hash field
(`stateHash.hpp:25-50`):

- `rand.last` → `0` (RNG pristine, above).
- `cycles` → `0` (driver never `++cycles`; worms-only pass).
- level `material_id[*]` → constant (level pristine, above).
- per worm: `pos/vel` (physics + walk + jump — ported), `aiming_angle`
  (ProcessAiming/Movement — ported), `health` (= 100; fall-damage is `0`/off in
  `data/TC/openliero`, verified by Slice 2's worm-component hash — which *includes
  health* — passing across the bounce), `lives`/`kills`/`timer` (unchanged; no
  death, no GOT mode), `visible` (= true; no death/respawn),
  `control_states.Pack()` (scripted input minus the bits the ported paths clear —
  see *Control-state mutation*).
- per weapon `ammo`/`delay_left`/`loading_left`/`type->id`: `delay_left` counts
  down (ProcessWeapons — ported); `ammo`/`loading_left` unchanged (`ammo > 0` for
  every slot ⇒ the reload branch `if (ammo <= 0)` never runs, so `loading_left`
  stays `0`); `type->id` constant.
- `ninjarope.out`, `ninjarope.pos.x/y`: set by the throw/retract (ProcessTasks —
  ported), frozen otherwise (rope `Process` not run).

**Two master proofs.** Because the *existing* Slice-2 golden (empty input) was
also produced by the full `worm->Process`, its master column already reflects the
ported `delay_left` countdown. After the port, the Rust master matches it too. So
Slice 3 **also flips the master assertion on for the Slice-2 empty-input golden**
(`sim_slice2_golden.rs`) — a free second proof that the un-ported gap Slice 2
documented (the `ProcessWeapons` `delay_left` countdown) is now closed — *and*
adds the new scripted-input golden where aiming/control/weapons/ninjarope actually
move.

## Datamodel additions (`sim` crate)

`WormState` already carries **every master-hashed field** (Slice 1/2 built the
hash; `hash.rs:54-79` already folds `aiming_angle`, `control_states`, the five
`weapons` slots, and the ninjarope). **No new *hashed* field is added.** What
Slice 3 adds is the **non-hashed worm state the ported methods read/write across
ticks** to make the hashed fields evolve correctly. Each is justified by the
C++ field it mirrors and the hashed field it drives:

| New `WormState` field | C++ (`worm.hpp`) | Default (post-`ResetWorms`/ctor) | Drives (hashed) |
|---|---|---|---|
| `aiming_speed: Fixed` | `aiming_speed{0}` (226) | `0` | `aiming_angle` |
| `direction: i32` | `direction{0}` (262) | `0` | `aiming_angle` (clamps + flips) |
| `movable: bool` | `movable{...}`, ctor sets `true` (180,230) | `true` | gates Aiming/Movement |
| `able_to_jump: bool` | `able_to_jump{false}` (228) | `false` | `vel.y` (jump) |
| `able_to_dig: bool` | `able_to_dig{false}` (228) | `false` | (dig deferred; flag toggles) |
| `key_change_pressed: bool` | `key_change_pressed{false}` (229) | `false` | `control_states` (Release L/R) |
| `current_weapon: i32` | `current_weapon{0}`, `ResetWorms` sets 0 (250; game.cpp:164) | `0` | which weapon slot reloads / cycles |
| `fire_cone: i32` | `fire_cone{0}` (252) | `0` | (inert; ProcessWeapons decrement, not hashed) |
| `leave_shell_timer: i32` | `leave_shell_timer{0}` (253) | `0` | (inert; gates the shell `rand()` branch — stays `0`) |

**Defaults verified against the oracle.** The dumper sets only `pos/vel/health/
lives/visible` after `ResetWorms` (`sim_physics_dump.cpp:189-197`); everything
else is the C++ ctor/`ResetWorms` value above. `WormState::from_init` must
initialise the new fields to exactly these constants so tick-0 parity (Slice 1/2)
is preserved — none of them are hashed at tick 0, but `current_weapon` and
`direction` are load-bearing for the *evolution* under input.

**Not added** (not read/written by any IN+hashed path): `hotspot_x/y`,
`make_sight_green` (ProcessSight omitted), `steerable_sum_x/y`/`steerable_count`
(no-op steerables), `ninjarope.attached/length/vel` (rope frozen, not hashed),
`animate` (only affects `current_frame`, not hashed), `prev_control_states`
(controller-level; the dumper `Unpack`s `control_states` each tick so it is never
read by `Process`). Resist widening.

### `ControlConsts` — the TC constants the control paths read

A new struct built from `TcConfig.constants` + `TcConfig.hacks` (sibling to the
existing `PhysicsConsts`), carried on `SimState`. All fields already exist in
`assets::tc` (verified). Honest naming keeps it separate from `PhysicsConsts`.

| Group | Fields (`TcConfig.constants` unless noted) |
|---|---|
| Aiming | `AimFricMult`, `AimFricDiv`, `AimMaxRight`, `AimMinRight`, `AimMaxLeft`, `AimMinLeft`, `MaxAimVelLeft`, `MaxAimVelRight`, `AimAccLeft`, `AimAccRight` |
| Movement | `WalkVelLeft`, `MaxVelLeft`, `WalkVelRight`, `MaxVelRight` |
| Jump | `JumpForce`; hacks `AirJump`, `MultiJump` |
| Ninjarope | `NRInitialLength`, `NRMinLength`, `NRMaxLength`, `NRPullVel`, `NRReleaseVel`, `NRThrowVelX`, `NRThrowVelY` |

(`PhysicsConsts` already carries `h_fall_damage`/`h_worm_float`.) Build via
`ControlConsts::from_tc(&tc)`, store on `SimState` next to `physics`.

## Control-state mutation (load-bearing for the master hash)

The master hash reads `control_states.Pack()` **after** `Process` has run, and the
ported paths *mutate the control bits*. The dumper `Unpack`s the raw scripted
input into `control_states` at the **start** of each worm's pass
(`sim_physics_dump.cpp:235`), then `Process` clears some bits; the hash sees the
**post-`Process`** value. The Rust driver must reproduce this exactly: overwrite
`control_states` from the tick's input, then apply the same bit clears.

The bit-clearing calls in the IN paths:

- `ProcessTasks` (change branch): `PressedOnce(kJump)` clears the **Jump** bit on
  a ninjarope throw (`worm.cpp:975`).
- `ProcessWeaponChange`: `Release(kLeft)`/`Release(kRight)` on the first
  change-tick (`key_change_pressed` false→true), then `PressedOnce(kLeft)` /
  `PressedOnce(kRight)` clear the **Left/Right** bits (`worm.cpp:1066-1096`).
- `ProcessMovement`, the jump else-branch, the Fire gate: **read only** (`Pressed`),
  no clears.

> **The `Unpack`-each-tick subtlety (must be documented in code).** Because the
> dumper overwrites `control_states` from the scripted input every tick,
> `PressedOnce`'s edge detection **degenerates to a per-tick bit read + clear**:
> `prev_control_states` is never consulted (and is not hashed). So holding
> Change+Right for *k* ticks cycles the weapon *k* times (the Right bit is
> re-set by `Unpack` each tick, so `PressedOnce(kRight)` returns true each tick).
> This is "unrealistic" vs the live controller, but the **oracle and the port
> agree** because both re-`Unpack` each tick — which is all that matters. The
> Rust `ControlState` needs mutating `press`/`release`/`pressed_once(&mut self)`
> helpers mirroring C++ `Press`/`Release`/`PressedOnce`.

## Per-worm pass: exact ordering

Slice 2's driver computed `reacts` and ran physics. Slice 3's per-worm pass runs
the full `Process` body in C++ order. **`reacts` is computed once** (in the
reaction orchestration) and consumed by **both** `ProcessTasks` (jump reads
`reacts[kRfUp]`) **and** `ProcessPhysics` — it is *not* recomputed between:

1. `health = min(health, settings_health)` (inert).
2. `worm_reactions(level, worm, physics)` → `reacts` (Slice 2; may nudge
   `pos.y`/`vel.y`).
3. `process_steerables` — no-op (empty `wobjects`).
4. movable reset: `if !movable && !Pressed(Left) && !Pressed(Right) { movable = true }`.
5. `process_aiming(worm, control_consts)`.
6. `process_tasks(worm, reacts, control_consts)` — jump uses `reacts[kRfUp]`.
7. `process_weapons(worm)` — `delay_left` countdown (+ inert branches).
8. *(Fire gate — OUT, Slice 4.)*
9. `process_physics(worm, reacts, physics)` (Slice 2).
10. *(ProcessSight — OUT, omitted.)*
11. `if Pressed(Change) { process_weapon_change(worm) } else { worm.key_change_pressed = false; process_movement(worm, control_consts) }`.

Note the cross-method ordering that is load-bearing: jump (step 6) changes
`vel.y` **before** `ProcessPhysics` (step 9) reads it for the bounce/gravity
checks; walk (step 11, `ProcessMovement`) changes `vel.x` **after**
`ProcessPhysics`, so a walk this tick affects *next* tick's integration. Port in
order; do not "optimise".

**Driver rename.** `process_worm_physics` is no longer physics-only — rename to
`process_worms` (honest: it runs each worm's full `Process`, still worms-only:
no `cycles++`, no bonus-drop RNG, no object/ninjarope `Process` loops — those are
Slice 6's `process_frame`). Update the one Slice-2 test call site. **Interleave
input-apply + process per worm** (`Unpack` worm i, then `Process` worm i, matching
`sim_physics_dump.cpp:233-238`); not load-bearing for Slice 3 (no IN+hashed path
reads the other worm), chosen for forward safety.

## Input scenario design

A **new** scenario `golden/sim_slice3_scenario.txt`, same grammar as Slice 2
(`oracle_tests::scenario` already parses `input` lines and exposes
`Scenario::input(tick, worm)`), reusing the existing fixture level
`Levels/physics_fall_test.lev` (an open sky band over a solid floor — lets the
worm fall, **land (grounded ⇒ `reacts[kRfUp] > 0`)**, then jump; no new fixture
needed). `seed 42`, `ticks ≈ 150`, two visible worms mid-air (different `x`/`y`
so the two master/component hashes diverge).

A scripted phase sequence that exercises every IN path (worm 0; worm 1 gets a
different L+R-free / Fire-free pattern so the hashes diverge), bits Up=1 Down=2
Left=4 Right=8 Change=32 Jump=64:

1. **Fall + land** (empty): establishes grounding for the jump phases.
2. **Walk right** (`Right`=8): `vel.x` rises, `direction` 0→1, `aiming_angle`
   flips, `animate`.
3. **Aim up/down** (`Up`=1 / `Down`=2): `aiming_speed`/`aiming_angle` accelerate
   then clamp at `AimMax/Min`.
4. **Walk left** (`Left`=4): `vel.x` falls, `direction` 1→0 flip.
5. **Jump** (one empty tick to set `able_to_jump`, then `Jump`=64): `vel.y -=
   JumpForce` while grounded.
6. **Weapon change** (`Change|Right`=40 / `Change|Left`=36, single taps and a
   multi-tick hold): `current_weapon` cycles, Left/Right bits cleared in the hash.
7. **Ninjarope throw** (`Change|Jump`=96): `ninjarope.out`→true, `ninjarope.pos`=
   worm pos, Jump bit cleared; then `Jump`=64 (no change) retracts (`out`→false).

**Load-bearing scenario constraints** (enforce + comment in the file):

- **Never set Left(4) and Right(8) together** on the same worm/tick → no dig → no
  `rand()`, no level write.
- **Never set Fire(16)** → no `Worm::Fire`.
- Keep both worms `health 100`, `visible 1`, in-bounds.

The implementer tunes exact ticks so each phase is actually exercised and the
worm stays alive/in-bounds; correctness does **not** depend on the values (Rust
matches whatever the dumper produced from the same file), but **coverage** does —
inspect the golden so `aiming_angle`, `delay_left`, `control_states`, and
`ninjarope.out` are seen to change.

## Oracle / golden decision

**Reuse the dumper and format; add a new scenario + golden + test; flip Slice 2's
master on.** Concretely:

- **C++ dumper: unchanged.** `oracle_dump_sim_physics` already drives the full
  `Process`, parses `input` lines, and dumps the master + 9 component columns.
  **No new dumper, no CMake change, no sim-code change.** `test_determinism` /
  `test_rollback_*` are untouched (we add no C++).
- **New** `golden/sim_slice3_scenario.txt` (with `input` lines) +
  `gen_sim_slice3_golden.sh` (copy of `gen_sim_physics_golden.sh` pointed at the
  slice-3 files) + committed `golden/sim_slice3.txt` (regenerated via the existing
  dumper, LOCAL/MANUAL).
- **New** `tests/sim_slice3_golden.rs`: asserts the **master `state_hash`** *and*
  all component columns per tick, with input keyed `k-1` (see *Input timing*).
- **Edit** `tests/sim_slice2_golden.rs`: flip the master assertion **on** against
  the existing empty-input golden (now matches). Keep its component + bounce
  assertions.

Keeping the Slice-2 scenario/golden frozen (empty input, the pure-physics proof)
and layering a *separate* scripted-input golden is cleaner than rewriting Slice
2's golden in place — two independent proofs (empty-input master, scripted-input
master+components).

### Input timing (the off-by-one, restated for this slice)

The dumper applies `input[t]` on the pass that advances tick `t → t+1`, then dumps
that result as line **`t+1`** (`sim_physics_dump.cpp:227-238`); line `0` is the
pre-motion state. So the Rust test producing golden line **`k`** (`k ≥ 1`) must
call `process_worms` with the input keyed **`k-1`**:
`process_worms([unpack(scn.input(k-1, 0)), unpack(scn.input(k-1, 1))])`. Assert
line 0 against the freshly-built tick-0 state with no `process_worms` call. Get
this wrong and every post-input tick is shifted by one.

## Definition of done

- [ ] `WormState` gains `aiming_speed, direction, movable, able_to_jump,
      able_to_dig, key_change_pressed, current_weapon, fire_cone,
      leave_shell_timer`; `from_init` sets the verified post-`ResetWorms`/ctor
      defaults; Slice 1/2 tick-0 goldens still pass.
- [ ] `ControlConsts` + `ControlConsts::from_tc`; carried on `SimState`.
- [ ] `ControlState` gains mutating `press`/`release`/`pressed_once`; unit-tested
      for the bit-clear semantics.
- [ ] `ProcessAiming`, `ProcessTasks`, `ProcessWeapons`, `ProcessWeaponChange`,
      `ProcessMovement` ported (dig body deferred w/ `debug_assert!`), each
      unit-tested against hand-folded C++ behaviour.
- [ ] Driver renamed `process_worms`, runs the full per-worm pass in C++ order
      with `reacts` shared by tasks+physics; interleaves input+process per worm;
      Slice-2 test call updated.
- [ ] New `sim_slice3_scenario.txt` (no Left+Right, no Fire), `gen_sim_slice3_
      golden.sh`, committed `sim_slice3.txt`.
- [ ] `tests/sim_slice3_golden.rs`: master **and** all 9 component columns match
      every tick (input keyed `k-1`); golden inspected to confirm
      `aiming_angle`/`delay_left`/`control_states`/`ninjarope.out` actually move.
- [ ] `tests/sim_slice2_golden.rs`: master assertion flipped **on** (now matches).
- [ ] `cargo test --workspace` green; `sim` still Bevy-free / float-free; deps
      unchanged (`sim-core`, `assets`).
- [ ] C++ side untouched ⇒ `test_determinism` / `test_rollback_*` unaffected.

## Open questions (decide in the plan / with the controller)

1. **Flip Slice-2 master on?** Recommended **yes** (free proof the un-ported gap
   closed). Confirm vs keeping Slice 2 frozen.
2. **Driver rename** `process_worm_physics` → `process_worms`? Recommended yes;
   touches the one Slice-2 call site.
3. **Interleave input+process per worm** vs apply-all-then-process-all? Recommended
   interleave (C++-exact, forward-safe); not load-bearing this slice.
4. **Exercise ninjarope throw** in the scenario? Recommended yes (deterministic,
   no RNG, adds `out`/`pos` master coverage cheaply).
5. **Compute `ninjarope.vel`/`length` on throw** (needs `cossin_table`)?
   Recommended **skip** — not hashed, rope frozen; avoids pulling the sin/cos
   table into Slice 3. (Add `attached`/`length`/`vel` fields only if a later slice
   needs them.)
6. **Confirm dig deferral to Slice 4** (vs porting `DrawDirtEffect` + matching the
   RNG sequence and level-hash time series now). Recommended defer.
7. **`ticks` count and exact input vector** — implementer tunes for coverage; the
   design fixes only the constraints (no L+R, no Fire, health 100).

## The hard 10% (carried into this slice)

- **Master-hash field coverage via non-hashed state.** The component hash can't
  see aiming/control/weapons/ninjarope, so a bug there is invisible until the
  master assertion. The new non-hashed fields (`aiming_speed`, `direction`,
  `current_weapon`, `key_change_pressed`, …) are load-bearing precisely because
  they steer the *hashed* fields across ticks. Cross-read each ported method
  against `worm.cpp` line by line.
- **Control-bit clears in the master hash.** `PressedOnce`/`Release` mutate
  `control_states`, and the hash reads the post-`Process` value. Miss a clear and
  `control_states.Pack()` diverges even when motion is correct. The `Unpack`-each-
  tick degeneration of `PressedOnce` must be reproduced (overwrite then clear).
- **`reacts` shared, computed once.** `ProcessTasks` (jump) and `ProcessPhysics`
  read the *same* `reacts` from the orchestration; recomputing it between (or
  letting the nudge corrections run twice) desyncs the jump/bounce interaction.
- **Cross-method velocity ordering.** Jump writes `vel.y` before physics; walk
  writes `vel.x` after physics. The two orderings are not symmetric — copy them.
- **Truncating division in aiming/friction.** `aiming_speed * AimFricMult /
  AimFricDiv` and `vel.x * WalkVel…` clamps truncate toward zero (Rust `/` /
  `wrapping_*`, never `>>`), same discipline as Slice 2's bounce/friction.
- **Dig is the one RNG+terrain path** — excluded by the no-Left+Right scenario
  constraint and guarded by a `debug_assert!` in the Rust dig branch so a future
  scenario that accidentally digs fails loudly instead of diverging silently.
  Deferring `DrawDirtEffect` to Slice 4 keeps Slice 3 RNG- and level-pristine.

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice3-plan.md`.
