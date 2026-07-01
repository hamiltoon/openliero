# Step 2, Slice 4d — The Slice-3 weapon/combat deferrals (dig, reload, shell-drop, sight, load_change)

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-slice4-weapon-lifecycle-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice4c-...` (the SObject/NObject machinery) and
`2026-06-28-liero-rs-step2-slice4b-dirt-destruction-design.md` (`draw_dirt_effect`)
(the proven `sim` crate, `oracle_dump_sim_physics` per-tick oracle, the scenario
pipeline incl. the `weapon <slot> <name>` directive, the `process_frame`
ProcessFrame-subset driver, `worm_fire`/`wobject_process`/`blow_up` from 4a, the
`draw_dirt_effect` blit from 4b, and `NObject::Create*`/`NObject::Process` +
`nobject_types`/`sobject_types` from 4c — all of which 4d **reuses**).

## Purpose

4d is the **fourth and last** sub-slice of the weapon-lifecycle milestone. It closes
the five worm-side combat/weapon deferrals that Slices 3 and 4a explicitly punted to
"Slice 4" because each needed machinery that only 4a–4c build:

1. **dig body** — `Worm::ProcessMovement`'s Left+Right dig branch
   (`worm.cpp:889-948`) calls `DrawDirtEffect` **twice** with the carving texture 7.
   Reuses **4b's `draw_dirt_effect`** (the `n_draw_back=true` half).
2. **reload branch** — `Worm::ProcessWeapons` `ammo<=0 → loading_left =
   ComputedLoadingTime` (`worm.cpp:823-827`). Pure scalar; no dependency.
3. **leave_shell shell-drop** — `Worm::ProcessWeapons`'s `leave_shell_timer`
   expiry (`worm.cpp:841-847`) spawns a shell **NObject** (`nobject_types[7]`).
   Reuses **4c's `NObject::Create1`/`Create` + `NObject::Process`**.
4. **ProcessSight** (laser sight, `worm.cpp:1190-1212`) — audited here:
   **no hashed effect, no RNG, no terrain write ⇒ stays omitted** (confirms the
   Slice-3 decision; 4d adds a golden that proves it inert by running a
   `laser_sight=true` weapon live in the dumper).
5. **weapon-change `load_change` gate** — the `|| settings->load_change` term in
   `Worm::ProcessWeaponChange`'s cycle condition (`worm.cpp:1079`) that the Slice-3
   port dropped (it only checks `Available()`, valid only while `loading_left==0`).

4d ports **no new object type and no new blit** — it is the *thinnest* of the four
sub-slices, "almost free" precisely because 4a/4b/4c already built every heavy
piece (overview, *Decomposition* §"The Slice-3 deferrals (4d) reuse 4a/4b/4c
machinery"). Its risk is **wiring and RNG-order**, not new algorithms.

### What changes vs Slice 4c

| Invariant | 4c | **4d** |
|---|---|---|
| `level` component / master level-fold | live via sobject/wobject craters | **also live via dig** — `draw_dirt_effect` texture 7 (the **first live `n_draw_back=true` carving** path; 4b/4c only unit-tested it) |
| per-weapon `loading_left`/`ammo` (hashed, `stateHash.hpp:39,41`) | `loading_left==0`, `ammo>0` always | **`loading_left` goes live** — reload sets it `>0`, then it counts down |
| `nobjects` pool | spawned by sobject dirt-throw/splinters | **also spawned by the shell-drop** (`nobject_types[7]`), during the **worm pass** (ProcessWeapons), not only during the object loops |
| `rng` per tick | Fire + explosion draws | **+ leave-shell `rand(leave_shells)`** at fire (4a-guarded → live), **+ 5 shell-expiry draws**, **+ 2 dig `rand(2)`** |
| `ProcessWeaponChange` gate | `Available()` only (always true) | **`Available() \|\| load_change`** — load-bearing once reload makes `Available()` false |
| `ProcessSight` | omitted | still omitted (audited inert; proven by a live `laser_sight` weapon) |
| `cycles` | `0` | still `0` |

## Scope

### IN — ported this slice (C++ references)

- **Dig body** (`worm.cpp:889-948`, inside `ProcessMovement`). The Slice-3 port
  already has the `kLeft && kRight` detection + the `able_to_dig` toggle
  (`control.rs:543-556`, with a `debug_assert!(false)` tripwire at `:549-552`). 4d
  **replaces the tripwire with the body**:
  - `kDir = cossin_table[Ftoi(aiming_angle)]` (`worm.cpp:893`).
  - `dig_pos = kDir*2 + pos` (`:895`), then `dig_pos.x -= Itof(7); dig_pos.y -=
    Itof(7)` (`:927-928`); `idig_pos = Ftoi(dig_pos)` (`:930`).
  - **`DrawDirtEffect(common, rand, level, 7, idig_pos.x, idig_pos.y)`** (`:931`) —
    texture **7** (`tc.cfg:183-188`: `mframe=99, rframe=2, sframe=73,
    ndrawback=true` ⇒ the **carving** branch).
  - `dig_pos += kDir*2` (`:937`); a **second** `DrawDirtEffect` at the new
    `idig_pos` (`:941`).
  - `CorrectShadow` (`:932-935`/`:942-945`) is gated on `settings->shadow` ⇒
    **OMITTED** via the dumper's `settings->shadow=false` (the same O4 omission 4b
    already applies; see *CorrectShadow / O4*).
  - **Offset note (load-bearing, cite both):** dig subtracts `Itof(7)` *before*
    `Ftoi`, so it passes `idig_pos.x` directly; `BlowUpObject` passes `Ftoi(x)-7`
    (`weapon.cpp:118`). These are arithmetically equal (`Ftoi(x - Itof(7)) ==
    Ftoi(x) - 7`, since `7<<16` is exact), but the dig call site is **already
    pre-offset** — do not subtract 7 again in the Rust dig path.
- **Reload branch** (`worm.cpp:823-827`, inside `ProcessWeapons`). Replaces the
  `debug_assert!(ww.ammo > 0)` tripwire (`control.rs:357-361`): `if ww.ammo <= 0 {
  ww.loading_left = computed_loading_time(w, settings_loading_time); ww.ammo =
  w.ammo; }`, where `ComputedLoadingTime(s) = max((s.loading_time * w.loading_time)
  / 100, 1)` (`weapon.cpp:8-14`; `settings.loading_time` default `100`,
  `settings.hpp:79`). The loading countdown below it (`:829-835`) is **already
  ported** (`control.rs:366-368`) and becomes genuinely live once reload arms
  `loading_left`. The `SoundReloaded` play (`:832-834`) is sound-only ⇒ skipped.
- **Shell-drop** (`worm.cpp:841-847`, inside `ProcessWeapons`). Replaces the
  `debug_assert!(leave_shell_timer == 0)` + `unreachable!` (`control.rs:378-387`):
  `if leave_shell_timer > 0 { leave_shell_timer -= 1; if leave_shell_timer <= 0 {
  let vel_y = -(rand(20000) as i32); let vel_x = rand(16000) as i32 - 8000;
  nobject_types[7].create1(fixedvec(vel_x, vel_y), pos, 0, index, rand, nobjects,
  ...) } }`. The two scalar draws are `rand(20000)` (`:843`) **then** `rand(16000)`
  (`:844`), in that order, **before** `Create1`.
- **The leave-shell arm in `Worm::Fire`** (`worm.cpp:1113-1117`) is **already
  ported and guarded** (`weapon.rs:168-170`: `if w.leave_shells > 0 &&
  rand.bound(w.leave_shells) == 0 { worm.leave_shell_timer = w.leave_shell_delay }`).
  4d makes it **live** by choosing `leave_shells > 0` (it is the **first** `rand` in
  `Fire`, before the spread draws — overview *RNG audit* §1).
- **`load_change` gate** (`worm.cpp:1079`). Replaces the
  `debug_assert!(loading_left == 0)` (`control.rs:447-451`) with the real gate:
  wrap the existing cycle block in `if ww.available() || load_change { ... }`, where
  `load_change` is a new `SimState` flag (default `true`, `settings.hpp:75`).
- **`SimState` carries two scalars**: `settings_loading_time: i32` (for reload) and
  `load_change: bool` (for the gate). Both come from `Settings`; the dumper already
  builds a `Settings` (4b sets `shadow=false` on it).

### OUT — deferred / omitted (with reason)

| C++ | What | Disposition |
|---|---|---|
| `worm.cpp:1190-1212` `ProcessSight` | laser raycast (`hotspot_x/y`, `make_sight_green`) | **OMITTED** — audited: no hashed field, no RNG, no terrain write (see *ProcessSight audit*). Proven inert by a `laser_sight=true` weapon in the golden. |
| `worm.cpp:932-935`,`942-945` `CorrectShadow` | shadow-correction pixel pass | **OMITTED** via dumper `settings->shadow=false` (the 4b O4 omission; dig shares it) |
| `worm.cpp:355-367` low-health smoke nobject | `health<25` smoke | Slice 6 (health 100) |
| `worm.cpp:369-426` death/respawn | blood/splinters/`--lives` | Slice 6 (health 100, visible) |
| `++cycles`, bonus-drop roll, ninjarope `Process` | ProcessFrame integration | Slice 6 (driver stays the 4a subset; `cycles` stays `0`) |

> **Stats / audio are no-ops**, as in 4a–4c. The shell `NObject`'s `fired_by`/
> `has_hit` and the `DamagePotential` call (`nobject.cpp:18-22`) are stats-only
> (not hashed). The `SoundReloaded`/launch sounds are skipped.

## Dependency ordering (the central 4d risk — flag explicitly)

4d is **LAST in Slice 4** and reuses APIs that 4b and 4c deliver. **4d must not be
executed until 4b and 4c have landed.** The plan REFERENCES these as they *will*
exist and flags every hinge:

- **dig ⇒ 4b's `draw_dirt_effect`.** Dig is the **first live exercise of the
  `n_draw_back=true` carving branch** (`blit.cpp:551-583`), which 4b ports but only
  unit-tests (greenball uses the additive `n_draw_back=false` half; 4b design
  *Scope*/O7). **Hinge:** if 4b's `draw_dirt_effect` signature or its carving
  cases change, the dig wiring (Task 1) revises. 4d's design assumes the 4b plan's
  signature `draw_dirt_effect(level, large_sprites, textures, dirt_effect, x, y,
  rand)` (4b plan, Task 1).
- **shell-drop ⇒ 4c's NObject machinery.** The shell uses `nobject_types[7]`
  (`tc.cfg:5` — `"shells"`), `NObject::Create1` (`nobject.cpp:41-49`) → `Create`
  (`:7-39`), and is then advanced every later tick by `NObject::Process`
  (`nobject.cpp:68-234`) in the nobjects loop. **Hinge:** the shell's lifetime
  (gravity=1000, bounce=40, `tc.cfg`/`shells.cfg`) is matched **only** if 4c's
  `NObject::Process` is correct; and the **RNG order inside `Create1`/`Create`**
  (below) is 4c's contract — if 4c shifts the `Create*` signature or RNG order,
  the shell-drop wiring (Task 2) revises. 4d's design assumes 4c builds the
  `nobject_types` table on `SimState` and a `create1` that draws
  `distribution → Create(start_frame, time_to_explo_v)` exactly per `nobject.cpp`.
- **reload / load_change / sight ⇒ no 4b/4c dependency.** They are pure scalar /
  audit-only and could land independently; they are bundled into 4d because they
  are the same "Slice-3 weapon deferrals" cluster and share the scenario.

## RNG audit — every new `rand()` site, in C++ call order

Audited from source (the `last` each call writes is what the next reads). Under the
4d scenario the **only** new RNG vs 4c is:

### Fire tick (handgun) — `Worm::Fire` (`worm.cpp:1100-1148`)

In call order: **leave-shell first** (`:1113-1117`) `if (leave_shells>0)
rand(leave_shells)` — handgun `leave_shells=1` ⇒ `rand(1)` (always `==0` ⇒ arms
`leave_shell_timer = leave_shell_delay = 1`). Then the per-part `Weapon::Fire`
(`weapon.cpp:16-76`): spread `rand(distribution*2)=rand(4000)` x, `rand(4000)` y
(`distribution=2000`); colour `rand(2)` (`start_frame=-1`); **no** time-var
(`time_to_explo_v=0`). So **handgun Fire = 1 (leave-shell) + 3 (spread x, spread y,
colour) = 4 rands**, leave-shell **first**. (This is exactly the 4a fan order with
the leave-shell guard now *taken* instead of skipped.)

### Shell-expiry tick — `ProcessWeapons` (`worm.cpp:841-847`) + `NObject::Create1`

`leave_shell_delay=1` ⇒ the timer expires on the **next** `ProcessWeapons` after
the fire. On expiry, in order:

1. `rand(20000)` — `vel_y = -static_cast<int>(rand(20000))` (`worm.cpp:843`).
2. `rand(16000)` — `vel_x = rand(16000) - 8000` (`worm.cpp:844`).
3. `Create1` (`nobject.cpp:41-49`): `distribution=8000` (`shells.cfg:12`) ⇒
   `vel.x += distribution - rand(16000)`; `vel.y += distribution - rand(16000)`
   — **2 rands**.
4. `Create` (`nobject.cpp:24-36`): `start_frame=45 > 0` (`shells.cfg:17`) ⇒
   `cur_frame = rand(num_frames+1) = rand(4)` (`numFrames=3`); `time_to_explo_v=0`
   ⇒ no draw — **1 rand**.

So **shell-drop = 5 rands**: `rand(20000), rand(16000), rand(16000), rand(16000),
rand(4)`, in that order. (Note the asymmetry: the two manual draws use the literal
forms `rand(20000)`/`rand(16000)-8000`, while `Create1` uses
`distribution - rand(distribution*2)` — opposite sign convention; port each
verbatim.)

### Dig tick — `ProcessMovement` (`worm.cpp:889-948`)

**2 rands**: each `DrawDirtEffect` draws exactly one `rand(tex.r_frame) = rand(2)`
at the top (`blit.cpp:537`; texture 7 `rframe=2`), so two calls ⇒ `rand(2), rand(2)`.
No other RNG in the dig path.

### Clean (no new RNG)

`reload` (`worm.cpp:823-827`), the loading countdown, and `load_change`
(`worm.cpp:1079`) draw **no** RNG. `ProcessSight` draws **no** RNG (below).

### RNG-ordering finding (the master-hash risk)

The **placement** of the leave-shell `rand(leave_shells)` as the *first* draw in
`Fire` (before spread) and of the shell-expiry 5-draw burst (in `ProcessWeapons`,
which runs **before** the Fire gate in the same worm pass, `worm.cpp:334` vs `:336`)
is load-bearing: a misordered or missing draw shifts every downstream `rand.last`.
**Within one worm pass the order is: `ProcessWeapons` (shell-expiry burst, if any)
→ Fire gate (leave-shell arm + spread)**, then `ProcessMovement` (dig, if Change not
held). The dig draws come **after** Fire only when both could occur on one tick —
but dig requires Change-not-held + L+R, and Fire requires Fire-pressed; the scenario
keeps these in **separate tick windows** to keep the order trivially auditable
(see *Input scenario design*).

## ProcessSight audit (O — confirm omit)

`Worm::ProcessSight` (`worm.cpp:1190-1212`):

- **Writes only** `make_sight_green` (`:1202`,`:1210`) and `hotspot_x`/`hotspot_y`
  (`:1207-1208`). **None of these are hashed** — `HashGameState`
  (`stateHash.hpp:25-50`) and `HashGameComponents` read neither; the Slice-3 design
  already established this (slice-3 design lines 71, 79-84). The Rust `WormState`
  deliberately omits all three (slice-3 design *Not added*).
- **Draws no RNG.** Its only callee, `CheckForWormHit` → `CheckForSpecWormHit`
  (`worm.cpp:1150-1188`), reads sprite/material pixels (`:1181`) but calls **no**
  `rand()`.
- **Writes no terrain.** It reads `game.level.Mat(...).Background()` (`:1204`) —
  read-only; no `material_id` write.

**Conclusion: keep `ProcessSight` OMITTED** (the Slice-3 decision stands). 4d does
**not** port it. It runs **live in the dumper** for any `laser_sight` weapon (it is
part of the unmodified `Worm::Process`), so the *proof* that omitting it is correct
is to fire a `laser_sight=true` weapon (handgun, `handgun.cfg:laserSight=true`) in
the 4d golden and confirm the Rust (which omits it) still matches the master
tick-for-tick. The full raycast lands only if a future slice needs `hotspot_*`
(none hashed ⇒ likely never).

## load_change gate (confirm + port)

`worm.cpp:1079`: `if (weapons[current_weapon].Available() ||
game.settings->load_change)`. The Slice-3 Rust port dropped the `|| load_change`
term and cycles **unconditionally**, pinned by `debug_assert!(loading_left == 0)`
(`control.rs:447-451`) — valid only while no reload ever arms `loading_left`.

**`load_change` defaults `true`** (`settings.hpp:75`). So with default settings the
gate is *always* true (even mid-reload), and the current unconditional Rust cycle
**happens to match** C++ — but (a) the `debug_assert!` would **panic in debug
builds** the moment a weapon-change is attempted during a reload (`loading_left>0`),
and (b) the port is silently wrong for any `load_change=false` config. 4d ports the
real gate `if ww.available() || load_change { cycle }` and carries `load_change` on
`SimState`.

**Proof in the golden (default `load_change=true`):** with handgun reload making
`loading_left>0`, a `Change`-held weapon-change still cycles `current_weapon`, and
the master hash (which reflects which slot's `ammo`/`loading_left` are read) matches
C++ — proving the gate is entered correctly and the `debug_assert!` is gone.

> **Open question O9 (controller):** to prove the gate is *load-bearing* (i.e. that
> it correctly **blocks** cycling when `Available()` is false), the golden would need
> `load_change=false`, which the default-settings dumper does not produce. Add a
> minimal `load_change <0|1>` scenario directive (sets `settings->load_change` in the
> dumper and `SimState.load_change` in the builder) to cover the blocking path, or
> accept the default-true proof only? *(Recommended: accept default-true for 4d —
> faithful port + no-panic + master match is sufficient; add the directive only if a
> future TC ships `load_change=false`.)*

## CorrectShadow / O4 (shared with 4b)

Both dig `DrawDirtEffect` calls are immediately followed by `CorrectShadow`, gated
on `settings->shadow` (`worm.cpp:932-935`,`:942-945`) — the **same global flag** 4b
omits by setting `settings->shadow=false` in the dumper. 4d inherits that omission
(no extra dumper change): with `shadow=false` the dig writes only `material_id` via
`draw_dirt_effect`, and `CorrectShadow` is skipped in **both** the oracle and the
Rust. The 1–4c re-diff gate (4b plan, Task 4) already proves `shadow=false` inert to
the prior slices; 4d's dig is the first path that *would* call `CorrectShadow`, so
4d simply does not port it (deferred to the dedicated shadow slice, with
`MakeShadow`).

## Datamodel additions (`sim` crate)

No **new hashed field**. `loading_left`/`ammo` are already hashed
(`hash.rs:69-70`); the shell `NObject` is hashed by the existing nobject fold
(`hash.rs:99-108` — `pos.x/y, vel.x/y, cur_frame, ty.id`); `material_id` is already
the level state. Added:

| New field / change | C++ | Why (non-hashed unless noted) |
|---|---|---|
| `SimState.settings_loading_time: i32` | `settings->loading_time` (`settings.hpp:79`, default 100) | reload's `ComputedLoadingTime` |
| `SimState.load_change: bool` | `settings->load_change` (`settings.hpp:75`, default true) | the weapon-change gate |
| `process_weapons` signature gains `weapons: &[Weapon]`, `settings_loading_time`, `rand`, `nobjects`, `nobject_types`, `worm_index`, `cossin` | `ProcessWeapons` reads the current weapon's `loading_time`/`ammo` (reload) and spawns the shell `NObject` (shell-drop) | wiring; the reload reads `Weapon`, the shell calls `NObject::Create1` |
| `process_movement` signature gains `level: &mut LevelSim`, `large_sprites`, `textures`, `cossin`, `rand` | dig's `DrawDirtEffect`(×2) + `cossin[Ftoi(aiming_angle)]` | wiring for the dig body |
| `process_weapon_change` signature gains `load_change: bool` | the `|| load_change` gate | wiring |

`Weapon::ComputedLoadingTime` ports as a free fn (or `assets::object::Weapon`
method) — `assets::object::Weapon` already carries `loading_time` and `ammo` (1e-2).
`nobject_types`/`NObject::Create1`/`NObject::Process` are **4c deliverables** (4d
consumes them).

## Input scenario design

**Recommendation: one combined `golden/sim_slice4d_scenario.txt`** (handgun),
phased so each deferral is exercised in its own tick window and the RNG order stays
trivially auditable. 4d is last and cheap; a single scenario that chains
fire→shell→reload→change and a separate dig window is the most economical proof and
keeps one golden. (Alternative: one scenario per deferral — more files, no extra
rigor; not recommended.)

Reuse `Levels/physics_fall_test.lev` (sky over a solid Dirt floor — the 4a/4b/4c
fixture). `seed 42`, `ticks ≈ 110`, two worms. Slot 0 = **handgun**
(`weapon 0 handgun 2` — see *new directive*), `current_weapon=0`.

Phases for worm 0 (worm 1 a divergent, dig-free/fire-free pattern, kept invisible/far
so it is never hit):

1. **Grounded settle** (empty) — land on the floor so dig/aim are stable.
2. **Fire ×2** (`Fire=16`, spaced ≥ `delay=20` apart): each shot draws the 4-rand
   Fire burst (leave-shell first), arms `leave_shell_timer=1`; the **next**
   `ProcessWeapons` drops a shell (`nobject_types[7]`, 5-rand burst) → `nobjects`
   goes non-empty and then evolves under `NObject::Process`. The **second** fire
   takes `ammo` 2→1→… ; with `weapon 0 handgun 2`, the second shot makes `ammo=0`,
   so the **following** `ProcessWeapons` runs the **reload** (`loading_left =
   ComputedLoadingTime(handgun) = max(100*220/100,1) = 220`, then counts down).
3. **Weapon-change during reload** (`Change|Right=40` held a few ticks while
   `loading_left>0`): exercises the **`load_change` gate** — cycling proceeds
   because `load_change=true` even though `Available()` is false. `current_weapon`
   advances; the master hash (per-slot fields) tracks it.
4. **Dig window** (`Left|Right=12`, Change **not** held, alternating with a
   single-direction or idle tick to re-arm `able_to_dig`): each L+R edge runs the
   dig body — 2× `draw_dirt_effect` (texture 7, carving), 2× `rand(2)`, `level`
   moves. Place the worm over the Dirt floor so the carve actually removes material
   (assert `level` moves). **`able_to_dig` is edge-triggered** (`worm.cpp:890-951`):
   it digs once per L+R press, then re-arms only on a not-both-held tick — so the
   scenario must **toggle** L+R to dig more than once.
5. **Laser-sight live throughout** — handgun `laser_sight=true`, so `ProcessSight`
   runs in the dumper every tick; the Rust omits it; the golden match proves it
   inert.

**Load-bearing constraints (comment them in the file):**
- Keep both worms `health 100`; the non-firing worm **invisible**; keep shots and
  the shell off any *visible* worm (handgun `worm_collide=true`/`detect_distance=0`)
  so no worm-hit RNG (excluded by geometry, as 4a–4c).
- Sequence the phases so **Fire and dig never land on the same tick** (Fire needs
  Fire-pressed; dig needs Change-not-held + L+R) — keeps the per-tick RNG order
  unambiguous.
- The dig must hit **Dirt** cells (carving writes only Dirt/Dirt2 — `blit.cpp:551-583`)
  or `level` will not move; aim/position the worm into the floor.

> **No new dig input grammar is needed** — the existing `input <tick> <w0> <w1>`
> bitmask already encodes Left(4)+Right(8)=12. Slice 3 merely *constrained* scenarios
> to never set L+R; 4d removes that constraint. The **only** candidate new dumper
> capability is the low-ammo override below.

### New dumper capability — minimal: `weapon <slot> <name> [ammo]`

handgun's `ammo=15` would need ~15 fires (× `delay=20` ≈ 300 ticks) to reach reload.
**Recommendation: extend the existing `weapon` directive with an optional 3rd token
`[ammo]`** so `weapon 0 handgun 2` starts slot 0 with `ammo=2` — reload after two
shots. The dumper sets `worm->weapons[slot].ammo = ammo` after `InitWeapons` (one
line beside the existing `.type` assignment, 4a); the Rust builder sets
`WeaponInit { ty: Some(handgun_id), ammo }`. This is the **only** new dumper
capability 4d needs. (Alternative: pick `super_shotgun` (`ammo=2`) and avoid the
directive change — rejected: `parts=40`, `distribution=20000` ⇒ 80 Fire rands and
40 wobjects per shot, far noisier than handgun's `parts=1`.)

Everything else (reload, shell, sight, load_change, dig) is reached by **real game
code** under the handgun loadout + the scripted input; no further dumper change. The
`settings->shadow=false` line is already present from 4b.

## Oracle / golden

Per the 4a/4b/4c pipeline (dumper, `weapon` directive, `process_frame`, the off-by-
one input timing all exist):

- **C++ dumper:** extend the `weapon` directive parse with the optional `[ammo]`
  token (set `worm->weapons[slot].ammo`); **nothing else**. The dig/reload/shell/
  sight/load_change paths are unmodified `Worm::Process`/`ProcessWeapons` game code,
  reached automatically under the scenario. `settings->shadow=false` already set
  (4b). **Required check:** regenerate the slice-1/2/3/4a/4b/4c goldens and confirm
  byte-identical (the `[ammo]` token is opt-in; absent ⇒ no behaviour change).
- **New** `golden/sim_slice4d_scenario.txt`, `gen_sim_slice4d_golden.sh` (copy of
  the 4c gen script; LOCAL/MANUAL), committed `golden/sim_slice4d.txt`.
- **New** `tests/sim_slice4d_golden.rs`: master `state_hash` **and** all 9 component
  columns per tick (input keyed `k-1`, the established off-by-one), with coverage
  guards (below).
- The Rust builder maps `weapon 0 handgun 2` to `WeaponInit { ty: Some(handgun_id),
  ammo: 2 }`.

### Coverage guards (the golden must demonstrably exercise each deferral)

- **reload:** some slot's `loading_left` takes a value `>0` and then a smaller value
  (armed then counting down), and its `ammo` resets from `0` to `w.ammo`.
- **shell-drop:** `nobjects` is empty, then **non-empty** starting one tick after a
  fire, then evolves (≥2 distinct `nobjects` component values) — proving the shell
  spawned **and** `NObject::Process` advanced it.
- **dig:** the `level` component changes during the dig window (≥1, ideally ≥2
  distinct values for two digs) and the `rng` column advances by 2 across a dig tick.
- **load_change:** `current_weapon`-driven master change occurs **while** the
  current slot's `loading_left>0` (cycling during reload).
- **sight:** implicit — the whole golden matches with `ProcessSight` omitted while
  the dumper runs it live (handgun `laser_sight=true`).
- **Fire/leave-shell:** `rng` advances by 4 at each fire tick (leave-shell + 3).

### Input timing (unchanged off-by-one)

Golden line `k` (`k≥1`) = state after `process_frame` with input keyed **`k-1`**;
line 0 = the pre-motion tick-0 state with no call (Slices 2/3/4a–c *Input timing*).

## Definition of done

- [ ] `SimState` carries `settings_loading_time: i32` + `load_change: bool`;
      `SimState::new` takes them; slice-2/3/4a/4b/4c call sites updated (defaults
      `100`/`true` where unspecified).
- [ ] **Reload** ported (`control.rs:357` tripwire → body): `ammo<=0 ⇒ loading_left
      = computed_loading_time; ammo = w.ammo`; `ComputedLoadingTime = max(s*lt/100,
      1)`. Unit-tested (ammo 1→fire→0→reload arms 220→counts down; non-current slots
      untouched).
- [ ] **Shell-drop** ported (`control.rs:378-387` tripwire → body): timer decrement,
      on `<=0` draw `rand(20000), rand(16000)` then `nobject_types[7].create1(...)`;
      RNG order pinned. Unit-tested with the shells config (5-draw burst; one nobject
      spawned). **(Depends on 4c `Create1`/`nobject_types`.)**
- [ ] **Dig** ported (`control.rs:549-552` tripwire → body): `kDir =
      cossin[Ftoi(aiming_angle)]`; two `draw_dirt_effect(..., 7, idig.x, idig.y,
      rand)` at `kDir*2+pos-Itof(7)` and `+kDir*2`; texture-7 carving writes
      `material_id`; 2× `rand(2)`. `CorrectShadow` omitted. Unit-tested (level moves
      over Dirt; rng +2; edge-triggered `able_to_dig`). **(Depends on 4b
      `draw_dirt_effect` carving half.)**
- [ ] **load_change gate** ported (`control.rs:447-451` tripwire → `if available()
      || load_change`). Unit-tested (cycles when `loading_left>0` && `load_change`;
      blocks when `loading_left>0` && `!load_change`).
- [ ] **ProcessSight** confirmed omitted (no code) — the golden proves it inert via a
      `laser_sight=true` weapon.
- [ ] Driver `process_frame` threads the new args into `process_weapons`,
      `process_movement`, `process_weapon_change`.
- [ ] Dumper: `weapon <slot> <name> [ammo]` optional token; slice-1..4c goldens
      byte-identical; new `sim_slice4d_scenario.txt` + gen script + committed
      `sim_slice4d.txt`.
- [ ] `tests/sim_slice4d_golden.rs`: master + 9 components match every tick; all five
      coverage guards hold.
- [ ] `cargo test --workspace` green; `sim` Bevy-free / float-free; deps unchanged
      (`sim-core`, `assets`).
- [ ] Determinism note: only the dumper (oracle-gated, non-sim) C++ changed ⇒
      `test_determinism`/`test_rollback_*` unaffected.

## The hard 10% (this slice)

- **RNG order across two methods in one worm pass.** `ProcessWeapons` (shell-expiry
  burst) runs **before** the Fire gate (`worm.cpp:334` vs `:336`); the leave-shell
  arm is the **first** draw inside `Fire`. Within `Create1`→`Create` the draw order
  is `distribution(x), distribution(y), [start_frame]`. A misordered or missing draw
  desyncs every later `rand.last`. Thread the one `Rand` in exact C++ order.
- **Dig is the first live `n_draw_back=true` carve** — the 4b carving branch
  (`blit.cpp:551-583`) was only unit-tested; dig exercises it against the oracle.
  The texture-7 cases (`6 ⇒ AnyDirt→texel`, `1 ⇒ Dirt2→2 / Dirt→1`) must match
  pixel-exact or the `level` fold diverges.
- **Dig offset / fixed-point.** `kDir*2+pos`, then `-Itof(7)` *before* `Ftoi`
  (pass `idig` directly — do **not** subtract 7 again); the second call adds another
  `kDir*2`. Truncating shifts (`Ftoi=>>16` arithmetic), as 4a–4c.
- **`ComputedLoadingTime` truncation + the `==0 ⇒ 1` clamp** (`weapon.cpp:9-12`):
  `(loading_time * w.loading_time) / 100` is integer `/`, then min 1. Get the clamp
  wrong and `loading_left` (hashed) diverges.
- **Shell `NObject` lifetime via 4c.** The shell is hashed at spawn **and** every
  later tick as `NObject::Process` moves it (gravity/bounce) — the golden only
  matches if 4c's `NObject::Process` is correct; flag this cross-slice coupling.
- **`load_change` default true masks the gate.** The port must add the real gate
  (and remove the panic-prone `debug_assert!`), even though the default-true golden
  cannot by itself prove the *blocking* branch (O9).
- **Object-loop-before-worms ordering** and **`cycles` staying 0** — unchanged from
  4a; the shell spawned during the worm pass first moves in **next** tick's nobjects
  loop.

## Open questions for the controller

- **O9 (new)** — Prove the `load_change=false` *blocking* path via a new
  `load_change <0|1>` scenario directive, or accept the default-true proof for 4d?
  *(Recommended: accept default-true; add the directive only when a TC ships
  `load_change=false`.)*
- **O10 (new)** — One **combined** 4d scenario (handgun: fire→shell→reload→change +
  a dig window), or one scenario per deferral? *(Recommended: combined — last/cheap,
  fewer files, same rigor.)*
- **O11 (new)** — Add the `weapon <slot> <name> [ammo]` low-ammo override (one
  dumper line + builder field), or drive reload by firing handgun's full `ammo=15`
  (~300 ticks)? *(Recommended: add the optional `[ammo]` token — minimal, keeps the
  golden short and the reload window crisp.)*
- **Carried:** O4 (CorrectShadow omitted via `shadow=false` — dig inherits it; 4b
  recommendation stands) and the 4c decisions on `NObject::Create*` signature / RNG
  order (4d's shell-drop hinges on them — revise if 4c shifts).

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice4d-plan.md`.
