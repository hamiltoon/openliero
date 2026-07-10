# Step 2, Slice 4d — Slice-3 weapon deferrals (dig / reload / shell / sight / load_change): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing
> test (or its assertion) before the implementation it pins.

> **EXECUTION GATE — 4d is LAST in Slice 4.** Do **not** start until **4b**
> (`draw_dirt_effect`, incl. the `n_draw_back=true` carving half) **and 4c**
> (`NObject::Create1`/`Create` + `NObject::Process` + the `nobject_types` table on
> `SimState`) have landed. 4d **reuses** those APIs; if either's signature differs
> from what this plan assumes (noted at Task 1/Task 2), reconcile before coding.

**Goal:** Replace the five Slice-3/4a deferral tripwires in the `sim` crate with the
real ports — the dig `DrawDirtEffect` body, the `ProcessWeapons` reload branch, the
`leave_shell_timer` shell-drop, the `ProcessWeaponChange` `load_change` gate — and
**confirm `ProcessSight` stays omitted** (audited inert), so the Rust sim reproduces
the C++ master `HashGameState` **and** all 9 component hashes **tick-for-tick** under
a handgun scenario that fires (shell + reload), changes weapon during reload
(load_change), and digs (level carve).

**Architecture:** Extend `rust/sim/` (no new crate; deps unchanged: `sim-core`,
`assets`; Bevy-free, float-free). `SimState` gains two scalars
(`settings_loading_time`, `load_change`). `process_weapons` (`control.rs`) gains the
reload + shell-drop bodies (threading `Weapon` data, `Rand`, the `nobjects` pool +
`nobject_types`); `process_movement` gains the dig body (threading `LevelSim`,
`large_sprites`, `textures`, `cossin`, `Rand` → `draw_dirt_effect`);
`process_weapon_change` gains the `load_change` gate. The driver `process_frame`
(`state.rs`) threads the new args. The C++ dumper gains **one optional token**
(`weapon <slot> <name> [ammo]`); slices 1–4c goldens stay byte-identical.

**Tech stack:** Rust (`sim` extend, `oracle-tests`) + one oracle-gated C++ dumper
edit (non-sim). Golden regenerated locally via the dumper
(`OPENLIERO_BUILD_ORACLE_DUMP`); CI (`cargo test --workspace`) runs the committed
golden. `data/TC/openliero` real TC; weapon **handgun**.

## Global constraints

- **Bit-exact vs C++.** Sources of truth: `worm.cpp:811-848` (`ProcessWeapons`:
  reload `:823-827`, loading countdown `:829-835`, shell-drop `:841-847`),
  `worm.cpp:889-948` (`ProcessMovement` dig), `worm.cpp:1064-1098`
  (`ProcessWeaponChange`, gate `:1079`), `worm.cpp:1100-1148` (`Worm::Fire`,
  leave-shell `:1113-1117`), `worm.cpp:1190-1212` (`ProcessSight` — omit),
  `weapon.cpp:8-14` (`ComputedLoadingTime`), `nobject.cpp:7-49` (`Create`/`Create1`),
  `gfx/blit.cpp:534-622` (`DrawDirtEffect`, carving `:551-583`), `tc.cfg:183-188`
  (texture 7), `shells.cfg` + `tc.cfg:5` (`nobject_types[7]`), `settings.hpp:75,79`
  (`load_change`/`loading_time` defaults), `stateHash.hpp:38-44` (hashed weapon
  fields), `hash.rs:69-70,99-108` (Rust folds).
- **RNG order is the contract.** Per worm pass: `ProcessWeapons` shell-expiry burst
  `rand(20000), rand(16000)` then `Create1` (`distribution → Create`) **before** the
  Fire gate's leave-shell `rand(leave_shells)` + spread; dig `rand(2), rand(2)` in
  `ProcessMovement`. Thread the one `sim-core::Rand`; never pull ad hoc.
- **Reuses 4b + 4c.** dig ⇒ `draw_dirt_effect` (4b, carving half); shell ⇒
  `NObject::Create1`/`Create` + `NObject::Process` + `nobject_types` (4c). Do not
  re-port them.
- **`CorrectShadow` OMITTED** (O4) — dumper `settings->shadow=false` (already set by
  4b); dig does not port `CorrectShadow`.
- **Only `material_id`, `loading_left`/`ammo`, and the shell `NObject` are new hashed
  movers** — no `materials` cache, no `display_valid`, no `hotspot_*`/
  `make_sight_green` (ProcessSight omitted).
- **`cycles` stays 0.** Driver remains the 4a ProcessFrame subset; no `++cycles`, no
  bonus roll, no ninjarope.
- **Truncating division / shifts.** `Ftoi=>>16` (arithmetic), `Itof=<<16`;
  `ComputedLoadingTime` and `Create*` use Rust `/`, never `>>`. Same discipline as
  4a–4c.
- **Scenario is the single source of truth**, read by both the dumper and the Rust
  test. Golden regen LOCAL/MANUAL; `PRESET` defaults `macos-arm64`.
- **No AI / "Generated with" taglines.** **Bash discipline:** one command per call;
  no `>>`/heredoc/`&&`/`;`/`$VAR` chaining; create files with the editor.

## File structure

- `rust/sim/src/state.rs` — MODIFY: `SimState` gains `settings_loading_time: i32` +
  `load_change: bool`; `SimState::new` takes them; `process_frame` threads the new
  args into `process_weapons`/`process_movement`/`process_weapon_change`.
- `rust/sim/src/control.rs` — MODIFY: `process_weapons` (reload + shell-drop bodies,
  new signature), `process_movement` (dig body, new signature),
  `process_weapon_change` (`load_change` gate, new signature). Remove the four
  tripwires (`:357`, `:378-387`, `:447-451`, `:549-552`).
- `rust/sim/src/weapon.rs` — (no change required; the leave-shell arm `:168-170` is
  already live-able). Optionally relocate `ComputedLoadingTime` here or add it on
  `assets::object::Weapon`.
- `rust/oracle-tests/golden/sim_slice4d_scenario.txt` — NEW.
- `rust/oracle-tests/gen_sim_slice4d_golden.sh` — NEW (LOCAL/MANUAL).
- `rust/oracle-tests/golden/sim_slice4d.txt` — NEW (committed).
- `rust/oracle-tests/tests/sim_slice4d_golden.rs` — NEW.
- `src/tools/oracle_dump/sim_physics_dump.cpp` — MODIFY: parse the optional `[ammo]`
  token on the `weapon` directive (one line). (Oracle-gated, non-sim.)

---

### Task 0: datamodel — `SimState` carries `settings_loading_time` + `load_change`

De-risk the scalars before behaviour.

**Files:** `rust/sim/src/state.rs`.

- [ ] **Step 1 (test):** `SimState::new` accepts `settings_loading_time: i32` and
      `load_change: bool` and exposes them; a constructed state reports
      `settings_loading_time == 100` and `load_change == true` for the default args.
- [ ] **Step 2 (impl):** add the two fields + ctor params. Update **all**
      `SimState::new` call sites (slice-2/3/4a/4b/4c tests + builders) to pass
      `100`/`true` (the C++ defaults, `settings.hpp:75,79`).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests` still green
      (no behaviour change yet — the fields are unread).

---

### Task 1: dig body in `process_movement` (test-first) — reuses 4b `draw_dirt_effect`

**Files:** `rust/sim/src/control.rs` (+ `state.rs` driver wiring in Step 5).

Source: `worm.cpp:889-948`. **Assumes 4b's** `draw_dirt_effect(level, large_sprites,
textures, dirt_effect, x, y, rand)` (4b plan Task 1) — reconcile the signature first.
Replaces the `debug_assert!(false)` tripwire at `control.rs:549-552`. New signature:
`process_movement(worm, c, level, large_sprites, textures, cossin, rand)`.

- [ ] **Step 1 (test) — dig geometry + offset:** with a worm at a known `pos`/
      `aiming_angle` and `able_to_dig=true`, L+R held, assert the two
      `draw_dirt_effect` calls receive `x,y` at `Ftoi(kDir*2 + pos - Itof(7))` and
      `Ftoi(kDir*4 + pos - Itof(7))` respectively (`kDir = cossin[Ftoi(aiming_angle)]`),
      texture index **7**, and that `able_to_dig` flips to `false`
      (`worm.cpp:890-948`). **Pass `idig` directly — do not subtract 7 again.**
- [ ] **Step 2 (test) — level carve + RNG:** on a level whose dig window is Dirt,
      assert the dig writes `material_id` (the carving `n_draw_back=true` cases:
      `6 ⇒ AnyDirt→texel`, `1 ⇒ Dirt2→2 / Dirt→1`, `blit.cpp:551-583`) and advances
      `rand` by exactly **two** `rand(2)` draws (texture 7 `rframe=2`), nothing else.
- [ ] **Step 3 (test) — edge-trigger:** L+R with `able_to_dig=false` does **not**
      dig (no write, no RNG); a not-both-held tick re-arms `able_to_dig=true`
      (`worm.cpp:949-951`). (These re-confirm the Slice-3 toggle tests
      `control.rs:1523-1556`, now without the panic.)
- [ ] **Step 4 (impl):** replace the tripwire with the body: `kDir`, `dig_pos =
      kDir*2 + pos`, `dig_pos -= Itof(7)` per axis, `draw_dirt_effect(... 7,
      Ftoi(dig_pos.x), Ftoi(dig_pos.y), rand)`, `dig_pos += kDir*2`, second
      `draw_dirt_effect`. `CorrectShadow` omitted. Truncating fixed-point.
- [ ] **Verify:** `cargo test -p sim` green; the Slice-3 `should_panic` dig test
      (`control.rs:1559-1571`) is updated/removed (the path no longer panics).

---

### Task 2: shell-drop in `process_weapons` (test-first) — reuses 4c `NObject::Create1`

**Files:** `rust/sim/src/control.rs` (+ driver wiring Step 5).

Source: `worm.cpp:841-847` + `nobject.cpp:41-49`. **Assumes 4c's** `nobject_types`
on `SimState` and `Create1` drawing `distribution → Create(start_frame,
time_to_explo_v)` (4c). Replaces the `debug_assert!`+`unreachable!` at
`control.rs:378-387`. `process_weapons` gains `rand`, `nobjects`, `nobject_types`,
`worm_index`, `cossin` (and the `weapons`/`settings_loading_time` from Task 3).

- [ ] **Step 1 (test) — timer decrement, no spawn until expiry:** `leave_shell_timer
      = 2` ⇒ after one `process_weapons` it is `1`, no nobject, no RNG; after the
      next it is `0` and the shell spawns.
- [ ] **Step 2 (test) — expiry RNG order + spawn:** with `leave_shell_timer=1`, after
      `process_weapons` assert exactly **5** draws in order `rand(20000), rand(16000),
      rand(16000), rand(16000), rand(4)` (the two manual + `Create1` distribution×2 +
      `Create` `rand(num_frames+1)`), and **one** `NObject` spawned into `nobjects`
      with `ty == Some(7)`, `pos == worm.pos`, and `vel` per `vel_y=-rand(20000)`,
      `vel_x=rand(16000)-8000` then the `distribution` adjust. (Use the shells config:
      `distribution=8000`, `start_frame=45`, `num_frames=3`, `time_to_explo_v=0`.)
- [ ] **Step 3 (impl):** replace the tripwire: `if leave_shell_timer > 0 {
      leave_shell_timer -= 1; if leave_shell_timer <= 0 { let vel_y =
      -(rand.bound(20000) as i32); let vel_x = rand.bound(16000) as i32 - 8000;
      nobject_types[7].create1(fixedvec(vel_x, vel_y), pos, 0, worm_index, rand,
      nobjects, ...) } }`. Match the 4c `Create1` call shape exactly.
- [ ] **Verify:** `cargo test -p sim` green; the Slice-3
      `leave_shell_timer_zero_skips_shell_branch_without_panic` test
      (`control.rs:1184-1196`) still passes (timer 0 ⇒ no spawn, no panic).

---

### Task 3: reload + loading countdown in `process_weapons` (test-first)

**Files:** `rust/sim/src/control.rs` (+ `weapon.rs` or `assets` for
`ComputedLoadingTime`).

Source: `worm.cpp:823-835`, `weapon.cpp:8-14`. Replaces the
`debug_assert!(ww.ammo > 0)` at `control.rs:357-361`. `process_weapons` gains
`weapons: &[Weapon]` + `settings_loading_time: i32`.

- [ ] **Step 1 (test) — `computed_loading_time`:** `computed_loading_time(w, s) =
      max((s * w.loading_time) / 100, 1)` (`weapon.cpp:9-12`). Pin: handgun
      `loading_time=220`, `s=100` ⇒ `220`; a tiny product (e.g. `s*lt=50`, `/100=0`)
      ⇒ clamps to `1`.
- [ ] **Step 2 (test) — reload arms on depletion:** current slot `ammo=0` ⇒ after
      `process_weapons`, `loading_left = computed_loading_time(w, s)` and `ammo =
      w.ammo`; **non-current** slots' `loading_left`/`ammo` untouched
      (`worm.cpp:820-827` operates on `weapons[current_weapon]` only — reuses the
      Slice-3 `loading_left_only_touches_current_weapon` posture).
- [ ] **Step 3 (test) — countdown after arming:** with `loading_left>0` (armed), each
      `process_weapons` decrements it (`worm.cpp:829-831`); the reload sound at `<=0`
      is not simulated. (Re-confirms `control.rs:1143-1154` now that arming is live.)
- [ ] **Step 4 (test) — `ammo>0` no-ops:** `ammo>0` ⇒ no reload, `loading_left`
      unchanged (the old invariant, now without the `debug_assert!`).
- [ ] **Step 5 (impl):** add `if ww.ammo <= 0 { ww.loading_left =
      computed_loading_time(w, settings_loading_time); ww.ammo = w.ammo; }` before the
      existing countdown; `w = &weapons[ww.ty_id]` (resolve the current weapon's
      def). Keep the countdown (`:366-368`).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 4: `load_change` gate in `process_weapon_change` (test-first)

**Files:** `rust/sim/src/control.rs`.

Source: `worm.cpp:1079`. Replaces the `debug_assert!(loading_left == 0)` at
`control.rs:447-451`. `process_weapon_change` gains `load_change: bool`.

- [ ] **Step 1 (test) — cycles when loading & load_change:** `loading_left>0`
      (`available()==false`) + `load_change=true` ⇒ `PressedOnce(Right)` still
      increments `current_weapon` (gate entered). No panic.
- [ ] **Step 2 (test) — blocks when loading & !load_change:** `loading_left>0` +
      `load_change=false` ⇒ `current_weapon` **unchanged** (gate skipped) even with
      Left/Right held; the Left/Right bit clears at `:1065-1070` still happen
      (they are *outside* the gate).
- [ ] **Step 3 (test) — cycles when available:** `loading_left==0` ⇒ cycles
      regardless of `load_change` (the Slice-3 behaviour, preserved).
- [ ] **Step 4 (impl):** wrap the cycle block in `if ww.available() || load_change {
      ... }`; remove the `debug_assert!`. The first-tick `Release(L/R)` + `fire_cone=0`
      stay before the gate (`worm.cpp:1065-1072`).
- [ ] **Verify:** `cargo test -p sim` green.

---

### Task 5: driver wiring — thread the new args through `process_frame` (test-first)

**Files:** `rust/sim/src/state.rs`.

`process_frame` already runs the per-worm pass (4a). 4d threads: into
`process_weapons` — `&weapons`, `settings_loading_time`, `rand`, `&mut nobjects`,
`&nobject_types`, `worm_index`, `cossin`; into `process_movement` — `&mut level`,
`&large_sprites`, `&textures`, `cossin`, `rand`; into `process_weapon_change` —
`load_change`.

- [ ] **Step 1 (test) — fire→shell integration:** handgun in slot 0, fire once; the
      next tick's `process_weapons` drops a shell (`nobjects` non-empty, `ty==Some(7)`),
      and subsequent ticks advance it via the (4c) nobjects loop. Assert `cycles==0`.
- [ ] **Step 2 (test) — fire-to-empty→reload integration:** `ammo` starts at 2; after
      two fires, the following `process_weapons` arms `loading_left=220` and counts
      down; `ammo` resets to `w.ammo`.
- [ ] **Step 3 (test) — dig integration:** Change-not-held + L+R over Dirt ⇒ `level`
      material changes; `rng` +2; a single-direction tick re-arms `able_to_dig`.
- [ ] **Step 4 (test) — change-during-reload integration:** Change held while
      `loading_left>0` cycles `current_weapon` (`load_change=true`).
- [ ] **Step 5 (impl):** destructure the new fields from `SimState`; pass them into
      the three calls in the per-worm pass. Mind the borrow split (the worm is `&mut`
      while `level`/`nobjects`/`weapons`/`nobject_types`/`large_sprites`/`textures`/
      `rand` are borrowed disjointly — same pattern as 4a's `wobjects`/`rand` split).
- [ ] **Verify:** `cargo test -p sim` green; `cargo test -p oracle-tests sim_slice2
      sim_slice3 sim_slice4a sim_slice4b sim_slice4c` still green (no behaviour change
      for them — their scenarios never reload/dig/drop-shells; the new args are inert).

---

### Task 6: C++ dumper — optional `[ammo]` token on the `weapon` directive

**Files:** `src/tools/oracle_dump/sim_physics_dump.cpp`.

- [ ] **Step 1 (impl):** in the `weapon <slot> <name>` parse (added in 4a), read an
      optional 3rd token; if present, set `worm->weapons[slot].ammo = parsed` (for
      both worms, after `InitWeapons`, beside the existing `.type`/`.ammo` set). One
      comment: opt-in low-ammo override to reach the reload branch quickly. **No other
      change** — dig/reload/shell/sight/load_change are unmodified game code reached
      under the scenario; `settings->shadow=false` already present (4b).
- [ ] **Step 2 (verify no regression):** re-run the slice-1..4c gen scripts; `git
      diff` on `sim_slice1..4c.txt` must be **empty** (the new token is opt-in and
      absent from those scenarios). If any diff, the parse changed default behaviour —
      stop and fix.
- [ ] **Verify:** dumper builds under `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`; prior
      goldens unchanged.

---

### Task 7: scenario file + `gen_sim_slice4d_golden.sh` + committed golden

**Files:** `rust/oracle-tests/golden/sim_slice4d_scenario.txt`,
`rust/oracle-tests/gen_sim_slice4d_golden.sh`,
`rust/oracle-tests/golden/sim_slice4d.txt`.

- [ ] **Step 1:** Create `sim_slice4d_scenario.txt`: `seed 42`, `level
      Levels/physics_fall_test.lev`, `ticks ≈ 110`, `weapon 0 handgun 2`, two worms
      (worm 1 invisible/far, dig-free/fire-free). Worm 0 phases (design *Input
      scenario design*): grounded settle → **Fire ×2** (≥`delay=20` apart; second
      empties ammo → reload) → **Change-held during reload** (load_change cycle) →
      **dig window** (`Left|Right`, Change not held, toggled to re-arm). **Constraints
      (comment them):** health 100; non-firing worm invisible; no shot/shell within
      `detect_distance` of a *visible* worm; Fire and dig in **separate** ticks; dig
      over Dirt so `level` moves.
- [ ] **Step 2:** Create `gen_sim_slice4d_golden.sh` (copy of the 4c gen script):
      `set -euo pipefail`, `PRESET=${PRESET:-macos-arm64}`, configure
      `-DOPENLIERO_BUILD_ORACLE_DUMP=ON`, build `oracle_dump_sim_physics`, run from
      ROOT with the 4d scenario + output. LOCAL/MANUAL; `chmod +x`.
- [ ] **Step 3:** Run it; commit `sim_slice4d.txt`. Inspect: `rng` `00000000` until
      the first fire (then +4), +5 at each shell expiry, +2 at each dig; `nobjects`
      empty → non-empty after a fire and then evolving; `loading_left` 0 → 220 →
      counting down; `ammo` 2→1→0→reset; `level` constant then changing in the dig
      window; `current_weapon`-driven master change during reload.
- [ ] **Verify:** `sim_slice4d.txt` has `ticks+1` lines, 11 columns; the five
      coverage signals above are all visible.

---

### Task 8: Rust differential test `sim_slice4d_golden` (test-first against golden)

**Files:** `rust/oracle-tests/tests/sim_slice4d_golden.rs`.

- [ ] **Step 1:** Mirror `sim_slice4c_golden.rs` setup: parse the scenario; load the
      `.lev`, `TcConfig` (materials + `PhysicsConsts` + `ControlConsts` + textures +
      `loading_time`/`load_change` defaults), the `Objects` weapon table, the
      large-sprite bank, the `nobject_types`/`sobject_types` (4c); resolve
      `weap_order`; build worm inits with `weapon 0 handgun 2`
      (`WeaponInit { ty: Some(handgun_id), ammo: 2 }`); build `SimState::new(...
      settings_loading_time=100, load_change=true ...)`. `parse_golden` keeps all
      columns incl. `state_hash`.
- [ ] **Step 2:** Assert tick-0 (master + 9 components). For `k` in `1..=ticks`:
      `process_frame([unpack(scn.input(k-1,0)), unpack(scn.input(k-1,1))])` (**input
      keyed `k-1`**) and assert **components first** (rng → level → worm0 → worm1 →
      sobjects → nobjects → bobjects → wobjects → bonuses) then master, so a
      divergence localises before the master fires.
- [ ] **Step 3 (coverage guards):** assert across the run — `loading_left` takes a
      value `>0` then a smaller one (reload armed + countdown) and `ammo` resets from
      0; `nobjects` is empty then non-empty then `≥2` distinct values (shell spawned +
      `Process`d); `level` changes `≥1` time in the dig window; a master change occurs
      while `loading_left>0` (load_change cycle); `rng` advances by 4 at a fire tick.
- [ ] **Verify:** `cargo test -p oracle-tests sim_slice4d` green.

---

### Task 9: wire-up review + done-check

- [ ] **Step 1:** `cargo test --workspace` green; `sim` has no Bevy / no float / no
      deps beyond `sim-core` + `assets`.
- [ ] **Step 2:** Re-read each C++ source against the port: reload (`worm.cpp:823-827`
      + `weapon.cpp:8-14`), shell-drop (`worm.cpp:841-847` + `nobject.cpp:41-49`),
      dig (`worm.cpp:889-948` + `blit.cpp:551-583`), gate (`worm.cpp:1079`), and
      confirm `ProcessSight` (`worm.cpp:1190-1212`) is **not** ported and the golden
      (handgun `laser_sight=true`) still matches — proving omission correct.
- [ ] **Step 3:** Confirm the **four tripwires are gone** (`control.rs:357`,
      `:378-387`, `:447-451`, `:549-552`) and replaced by tested bodies/gates.
- [ ] **Step 4:** Confirm the **only** C++ change is the oracle-gated dumper `[ammo]`
      token and that slice-1..4c goldens are byte-identical ⇒
      `test_determinism`/`test_rollback_*` unaffected (note in PR; no need to run).
- [ ] **Step 5:** Update the Slice-4 overview (mark 4d done; record the deferrals
      closed, the dig as the first live carve, the ProcessSight omission confirmed,
      O9/O10/O11) and the Step-2 overview *Slice ordering* (docs only). Don't commit
      unrelated changes.
- [ ] **Definition of done:** every checkbox in the 4d design's *Definition of done*
      is satisfied.

## Notes for the implementer

- **Order the RNG by method, not by feature.** In one worm pass the draws are:
  `ProcessWeapons` (shell-expiry burst, if the timer hit 0) → Fire gate (leave-shell
  arm, then per-part spread) → … → `ProcessMovement` (dig, if Change not held). The
  scenario keeps Fire and dig on separate ticks so you never have to reason about
  both in one pass — but the *port* must still place each draw where C++ does.
- **Dig is the first live carve.** 4b only unit-tested `n_draw_back=true`; here it
  faces the oracle. If `level` does not move in the dig window, the worm dug into
  sky/rock, not Dirt — fix the aim/position, not the blit.
- **`computed_loading_time` clamps to 1.** `max((s*lt)/100, 1)` — integer `/`, then
  min 1 (`weapon.cpp:10-12`). A 0 would break the `loading_left>0` countdown guard.
- **Shell lifetime is 4c's correctness, surfaced here.** The shell is hashed at spawn
  and every later tick; if the 4d golden diverges a few ticks *after* the shell drops
  (not at the drop), suspect `NObject::Process` (4c), not the shell-drop code.
- **`load_change` default-true hides the gate.** The golden (default settings) proves
  the *entered* branch only; the *blocking* branch is unit-tested (Task 4 Step 2) and,
  if the controller wants golden coverage, behind the optional `load_change` directive
  (O9).
- **Don't touch sim-critical C++.** The only C++ edit is the oracle-gated dumper
  `[ammo]` token.
- **Truncating shifts.** `Ftoi(x)=x>>16` (arithmetic); the dig pre-subtracts `Itof(7)`
  before `Ftoi` — pass the result straight to `draw_dirt_effect` (no second `-7`).
