# Step 2, Slice 4 — One weapon, full lifecycle: decomposition overview

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice3-control-aiming-design.md`
(the proven `sim` crate, the `oracle_dump_sim_physics` per-tick oracle, the
scenario pipeline, the `process_worms` driver this slice extends).

## Why this is an overview, not a single spec

Slice 4 is the roadmap's headline milestone — *"bullet fired → moves → explodes →
destroys terrain, checksum matches C++"* — and it is **large**: it ports
`Worm::Fire`, `WObject::Process`/`BlowUpObject`, `SObject::Process`/`Create`,
`NObject::Process`/`Create*`, and `DrawDirtEffect`, **and** it ends three
invariants that Slices 1–3 leaned on:

1. **RNG goes live.** `Worm::Fire` (spread, leave-shell), `WObject` trails/bounce-
   spread, `SObject::Create` (sound, dirt-throw, blood), `NObject::Create*`
   (speed/spread/colour), and `DrawDirtEffect` (texture-frame pick) all draw
   `rand()`. The `rng` component hash and the master's `rand.last` term stop being
   a constant `0`. The Rust sim must now reproduce the C++ **call order** of every
   `rand()` exactly.
2. **The level-hash goes live.** `DrawDirtEffect` writes `material_id`, so the
   `level` component hash (constant since Slice 1) becomes a **time series** the
   diff test must match — pixel-exact crater destruction.
3. **The tick is no longer "worms only".** The object `Process` loops run in
   `Game::ProcessFrame` (`game.cpp:333-351`), **before** the worm loop, not inside
   `worm->Process`. So the oracle/driver must run a **subset of `ProcessFrame`**,
   not just the worm pass (see *Oracle / driver decision*).

Because each of those is independently riskful, this doc **decomposes Slice 4 into
sub-slices 4a–4d**, picks the concrete weapon for each, fixes the cross-cutting
decisions (driver, RNG order, pool semantics), and **fully specs only 4a** (its
own design + plan are companion docs). 4b–4d are sketched here and detailed
just-in-time, per the charter — exactly as Step 1 split `1e` into `1e1/1e2/1e3`.

## Decomposition (thin-vertical, simplest object graph first)

The objects form a spawn tree:
`Worm::Fire → WObject ──explode──▶ BlowUpObject ──┬─▶ SObject (create_on_exp) ──▶ NObject (dirt-throw, blood) + worm damage`
`                                                 ├─▶ NObject (splinters)`
`                                                 └─▶ DrawDirtEffect (crater → level)`
`SObject::Create` and `NObject::Process` both re-enter the tree (child sobjects,
splinters). The decomposition climbs that tree one layer at a time so each
sub-slice adds **one** new pool / one new RNG cluster against its component hash.

| Sub-slice | Adds | Weapon | New pool(s) | RNG goes live | Level-hash |
|---|---|---|---|---|---|
| **4a** *(✓ DONE — bit-exact, master+components 93 ticks, on PR #3)* | `Worm::Fire` + `WObject::Process`/`BlowUpObject` for a projectile that **explodes into nothing** (free only) | **fan** | `wobjects` | **yes** (Fire spread/colour/time-var) | still **pristine** |
| **4b** *(✓ DONE — bit-exact, master+components 91 ticks, on PR #3)* | `BlowUpObject`'s own `dirt_effect` → **`DrawDirtEffect`** (the level-hash goes live); greenball's texture-6 is *additive* (`n_draw_back=false` ⇒ creates dirt in Background, not carving — `blit.hpp:40`) | **greenball** | — | + 1 `rand(tex.r_frame)=rand(2)` per explode | **goes live** |
| **4c** *(✓ DONE — bit-exact, master+components 91 ticks, on PR #3; first live `sobjects`/`nobjects` + first live carving `DrawDirtEffect`; O5 confirmed, O9/O10/O11 recorded; needed a small `shot_type=1` enablement in `wobject_process`)* | `create_on_exp` **`SObject::Create`** (sound, screen_flash, dirt-throw → **`NObject`** dirt-debris, crater) + `SObject::Process` + `NObject::Process`/`Create*`. **Deferred:** worm `DoDamage`+blow-away+blood (O10, worms kept out of range; the `cycles=0` blood-trail trap) and the splinter path (O9, dart spawns none). **`small_explosion`'s `dirtEffect=2` is a *carving* texture (`n_draw_back=true`)** ⇒ 4c reuses 4b's `draw_dirt_effect` and is its first *live* carve | **dart→small_explosion** (O5 confirmed; `bazooka→large_explosion` adds the splinter path — O9) | `sobjects`, `nobjects` | + sound `rand(2)` / dirt-throw `rand(8)`+`rand(128)`+`Create2` / crater `rand(2)` cluster (dart Fire = **0** rand) | live (carving) |
| **4d** *(planned — design+plan written; **executes LAST**, after 4b+4c land)* | the **Slice-3 deferrals** that belong here: dig-body `DrawDirtEffect` (texture 7 carving — first **live** `n_draw_back=true`), reload branch (`ammo<=0 → loading_left`/`ComputedLoadingTime`), `leave_shell_timer` shell-drop (`nobject_types[7]`), `ProcessSight` (laser — **audited inert, stays omitted**), weapon-change `load_change` gate | **handgun** (+ dig, no weapon) | — (reuses `wobjects`/`nobjects`) | + leave-shell `rand(leave_shells)` (4a-guarded→live) + 5-draw shell-expiry burst + 2× dig `rand(2)` | live (dig carve + reload `loading_left`) |

**Ordering rationale (sim-first, simplest weapon first):**

- **4a keeps the level pristine on purpose.** The single heaviest, most error-prone
  piece of Slice 4 is `DrawDirtEffect` (a clipped 16×16 texture blit that reads the
  large-sprite bank, picks a frame with `rand()`, and rewrites `material_id`
  per-pixel — and it is *shared* with the deferred dig body). Isolating it **out**
  of the first sub-slice lets 4a prove the genuinely new machinery — `Worm::Fire`,
  the `Pool` spawn/free during the object loop, `WObject::Process` movement /
  collision / explode, the ProcessFrame-subset driver, and the **first live RNG
  sequence** — while the `level` component hash stays the constant it has been since
  Slice 1 (one fewer new thing to get right at once). 4b then turns that one
  invariant — and only it — on.
- **A damage-bearing explosion couples in NObjects.** `SObject::Create`'s terrain
  block (`sobject.cpp:188-205`) spawns dirt-throw NObjects whenever the crater
  overlaps dirt — so "one explosion sobject that destroys terrain" with a normal
  explosion (`damage>0`) **cannot** avoid NObjects. The only way to destroy terrain
  with zero secondary objects is the **WObject's own `dirt_effect`** (no
  `create_on_exp`), which is exactly what 4b does with **greenball**. Real
  explosion sobjects (sound, damage, blow-away, dirt-throw, splinters) therefore
  land together in **4c**, after both the projectile lifecycle (4a) and
  `DrawDirtEffect` (4b) are proven.
- **The Slice-3 deferrals (4d) reuse 4a/4b/4c machinery.** The dig body is
  `DrawDirtEffect` (4b) under a control gate (already ported, guarded by a
  `debug_assert!`); the shell-drop spawns an NObject (4c); the reload branch is pure
  scalar state. Doing them last means they cost almost nothing new.
  **4d decomposition (planned — see the 4d design+plan):** five deferrals close
  here — (1) **dig** (`worm.cpp:889-948`) = 2× `draw_dirt_effect` texture-7 carving,
  the **first live** `n_draw_back=true` path (4b only unit-tested it); (2) **reload**
  (`worm.cpp:823-827`, `ComputedLoadingTime` `weapon.cpp:8-14`) — pure scalar, no dep;
  (3) **shell-drop** (`worm.cpp:841-847` → `nobject_types[7]`/`NObject::Create1`) —
  **depends on 4c**; (4) **`ProcessSight`** (`worm.cpp:1190-1212`) — **audited inert**
  (writes only non-hashed `hotspot_*`/`make_sight_green`, no RNG, no terrain write) ⇒
  **stays omitted**, proven by running a `laser_sight=true` weapon live in the dumper;
  (5) **`load_change` gate** (`worm.cpp:1079`) — the `|| settings->load_change` term
  Slice-3 dropped (default `true`; only load-bearing once reload makes `Available()`
  false). **4d is LAST in Slice 4** (it references 4b's `draw_dirt_effect` carving
  half and 4c's `NObject::Create1`/`Process` + `nobject_types`). Chosen weapon:
  **handgun** (shotType 0 ⇒ 4a flight; `createOnExp=small_explosion` ⇒ 4c sobject;
  `leaveShells=1`/`leaveShellDelay=1` ⇒ clean shell; `laserSight=true` ⇒ sight
  coverage), with a low-ammo override to reach reload fast.

> This deliberately **deviates from the planning brief's "4a = one sobject, level-
> hash matched"** suggestion. The brief's example bundles `DrawDirtEffect` **and**
> an `SObject` into the first step; the analysis above shows a `damage>0` sobject
> also drags in NObjects, making that "first" slice nearly all of Slice 4. Splitting
> "projectile lifecycle" (4a, level pristine) from "terrain destruction" (4b) from
> "explosion sobjects + nobjects" (4c) is the thinner, better-de-risked ladder.
> **Open question O1** (for the controller): accept this finer 4a, or hold to the
> brief's sobject-in-4a?

## The chosen weapons (and why each is simplest for its rung)

Picked from `data/TC/openliero/weapons/` by auditing every weapon's
`parts / distribution / splinterAmount / explGround / bounce / timeToExplo /
dirtEffect / createOnExp / shotType` (table built during planning):

- **4a — `fan`** (`weapons/fan.cfg`): the unique *"explodes into nothing"*
  projectile. `parts=1`, `splinterAmount=0`, `dirtEffect=-1`, **no `createOnExp`
  line** (→ `-1`), `bounce=0`, `gravity=0`, `shotType=0`, `timeToExplo=45`,
  `explGround=true`. So `BlowUpObject` *only frees the wobject* (no sobject, no
  splinter, no crater). It still fires the full `Worm::Fire` RNG (it has
  `distribution=12000` spread + `startFrame=-1` colour + `timeToExploV=10`), so 4a
  exercises live RNG end-to-end while the level stays pristine. `gravity=0` makes
  the trajectory a clean straight line; `timeToExplo=45` guarantees a deterministic
  timeout-explosion even in open sky, and aiming it into the floor also exercises
  the ground-collision explode path. (`collide_with_objects=true` is **inert**: its
  loop skips same-`type`+same-`owner`, which all of 4a's shots are.)
- **4b — `greenball`** (`greenball.cfg`): `dirtEffect=6` (its own crater), **no
  `createOnExp`**, `splinterAmount=0`, `explGround=true`, `bounce=0`. Destroys
  terrain via `BlowUpObject`'s `DrawDirtEffect` with **zero** secondary objects —
  the minimal "fire → explode → destroy terrain" graph.
- **4c — `dart`** (`dart.cfg`) → `small_explosion`: `dart` has the cleanest *Fire*
  of any explosive (`distribution=0`, `shotType=1` ⇒ deterministic `cur_frame` from
  angle, `timeToExplo=0`, `leaveShells=0` ⇒ **Fire draws no RNG**), so the new RNG
  in 4c is *all* in the explosion (`small_explosion`: sound `rand(2)`, dirt-throw,
  blood, `dirtEffect=2` crater). `bazooka` is the richer alternative (12 splinters,
  `large_explosion`) if 4c wants the full splinter path in one go.

> **Default-loadout gotcha (load-bearing for the scenario).** The Slice-2/3 fixture
> uses `settings.weapons = [1;5]` and a name-sorted `weap_order`, so
> `current_weapon=0` resolves to the alphabetically-first weapon, **`BAZOOKA`** —
> not `fan`. So 4a's scenario **must override the loadout** so slot 0 is the chosen
> weapon. See *Oracle / driver decision* → scenario `weapons` directive.

## Oracle / driver decision (the central Slice-4 mechanism)

**Extend the existing dumper to run a *subset* of `ProcessFrame`; keep one binary;
add a scenario weapon-loadout override.** Concretely:

- **The driver becomes a ProcessFrame-subset, not worms-only.** The Rust driver
  `process_worms` (Slice 3) is renamed/extended to `process_frame` running, in
  exact `game.cpp:333-355` order **and nothing else**:
  1. `sobjects` loop — `SObject::Process` each (empty until 4c)
  2. `wobjects` loop — `WObject::Process` each
  3. `nobjects` loop — `NObject::Process` each (empty until 4c)
  4. `bobjects` loop — blood `Process`/free (empty until 4c)
  5. worms loop — input-apply + full `Worm::Process` (Slices 2–3, **now incl. the
     Fire gate**, `worm.cpp:336-340`)
  **Deliberately excluded** (deferred to Slice 6, and load-bearing that they stay
  out): `++cycles`, the **bonus-drop RNG roll** (`rand(c[CBonusDropChance])` every
  tick — would consume RNG and desync), the ninjarope `Process` loop, and the
  game-mode logic. So `cycles` stays `0` throughout Slice 4 (the master's `cycles`
  term stays `0`), and the only RNG consumed is what `Fire`/objects/`DrawDirtEffect`
  draw — honest, isolated weapon lifecycle.
- **Object loops run BEFORE worms — load-bearing.** On the tick a worm fires, its
  `Worm::Fire` spawns the wobject *during the worm loop*, **after** the wobject loop
  already ran, so the new wobject sits still that tick and first moves next tick —
  exactly as C++. The Rust and the dumper must both put the object loops first.
- **C++ dumper: extend `sim_physics_dump.cpp` in place (keep the binary name).**
  Add the four object loops before the worm loop (as above). Because Slices 1–3
  scenarios never spawn an object, the loops are no-ops there ⇒ the slice-1/2/3
  goldens are **byte-identical** after the change (re-run `gen_sim_*` and diff to
  confirm — a required check). Keeping `oracle_dump_sim_physics` as the name avoids
  touching the slice-2/3 gen scripts. *(Lower-risk alternative, **O2**: fork a new
  `oracle_dump_sim_frame` to freeze the slice-2/3 dumper untouched. Recommended:
  extend in place + re-diff, since the no-op property is easy to verify.)*
- **Scenario gains a weapon-loadout override.** A new directive, e.g.
  `weapon <slot> <weapon_name>` (applied to both worms; `ammo` from the weapon
  def), consumed by **both** the dumper (set `worm->weapons[slot].type`/`ammo`
  after `InitWeapons`) and the Rust builder (a `WeaponInit`). `current_weapon`
  stays `0`, so `weapon 0 fan` makes the worm fire `fan`. This is the minimal
  change that makes the firing weapon explicit and decoupled from the default
  profile.
- **`PixelMat`/`Inside` already exist.** `WObject::Process` collision uses
  `game.PixelMat(x,y).DirtRock()` and `game.level.Inside(...)`. The Rust
  `LevelSim::checked_mat_background` (Slice 2) is the analogous probe;
  Slice 4 adds a `DirtRock()`-flavoured probe over the same `material_flags`
  (audit the exact `Material` bit, `material.hpp`).

## RNG audit — every `rand()` site, in C++ call order

`rand()` consumption order **is** the contract. Audited from source (the `last`
written by each call is what the next reads):

### `Worm::Fire` (`worm.cpp:1099-1148`) → `Weapon::Fire` (`weapon.cpp:16-76`)

In call order, **per fired part** (`parts` times):
1. **Leave-shell** (once, before the parts loop, `worm.cpp:1112`): `if
   (leave_shells>0) rand(leave_shells)`. *(fan/greenball/dart: `leave_shells=0` ⇒
   skipped.)*
2. **Spread** (`weapon.cpp:34-37`): `if (distribution) { vel.x += rand(distribution*2)
   - distribution; vel.y += rand(distribution*2) - distribution; }` — **2 rands, x
   then y**. *(fan/greenball draw these; dart `distribution=0` ⇒ skipped.)*
3. **Frame** (`weapon.cpp:39-69`): if `start_frame>=0` & `shot_type==kStNormal` &
   `loop_anim` ⇒ `rand(num_frames+1)` or `rand(2)`; **else if `start_frame<0`** ⇒
   `cur_frame = color_bullets - rand(2)` — **1 rand**. *(fan/greenball: `start_frame
   =-1` ⇒ 1 colour rand. dart: `shot_type==kStdType1`, deterministic, **0 rand**.)*
4. **Time-to-explo variance** (`weapon.cpp:73-75`): `if (time_to_explo_v) time_left
   -= rand(time_to_explo_v)` — **1 rand**. *(fan: `=10` ⇒ 1 rand. greenball/dart:
   `=0` ⇒ skipped.)*

So **fan Fire = 4 rands** (spread x, spread y, colour, time-var); **greenball Fire
= 3** (spread x, spread y, colour); **dart Fire = 0**.

### `WObject::Process` (`weapon.cpp:127-338`)

- `shot_type==3` drunk spread (`weapon.cpp:163-166`): `rand(distribution*2)` x2.
  *(fan `shot_type=0`; greenball `shot_type=0` ⇒ none. dirtball/bazooka are
  shot_type 0/3 — relevant only if chosen.)*
- part-trail crackler (`weapon.cpp:206`): `rand(128)`. *(none of 4a/4b weapons set
  a part-trail.)*
- worm-hit cluster (`weapon.cpp:303-324`, only if a worm is hit): blood
  `rand(128)` ×`kBloodAmount`; then `if (hit_damage>0 && health>0 && rand(3)==0) {
  rand(3) }`; then `if (worm_collide) rand(worm_collide)`. **Excluded in 4a/4b by
  geometry** (no worm within `detect_distance`; the non-firing worm is invisible ⇒
  `CheckForSpecWormHit` returns false, `worm.cpp:1163`).

### `BlowUpObject` (`weapon.cpp:78-125`)

- splinters scatter-0 (`weapon.cpp:100-106`): per splinter `rand(128)` then
  `rand(2)`, then `NObject::Create2` (more rands, below). scatter-1
  (`weapon.cpp:108-114`): per splinter `rand(2)` then `Create1`. *(fan/greenball:
  `splinter_amount=0` ⇒ none. 4c.)*
- `dirt_effect>=0` ⇒ `DrawDirtEffect` (below). *(greenball=6 ⇒ 4b; fan=-1 ⇒ none.)*

### `SObjectType::Create` (`sobject.cpp:16-228`) — 4c

In order: sound `if (start_sound>=0) rand(num_sounds)` (top); then **inside
`if (damage>0)`**: per in-range worm — blood `rand(128)`×`kBloodAmount`, then
`if (rand(3)==0) { rand(3) }`; then the wobject/nobject blow-away loops (**no
rand**); then the **dirt-throw** double loop (`sobject.cpp:195-204`): per cell
`if (AnyDirt(cell) && rand(8)==0) { rand(128); Create2 }` — note `&&` **short-
circuits**, so `rand(8)` is drawn *only* for dirt cells. Then (outside the damage
block) `dirt_effect>=0` ⇒ `DrawDirtEffect`. Bonus loop (no rand on empty pool).

### `NObjectType::Create / Create1 / Create2` (`nobject.cpp:7-66`) — 4c

- `Create` (`nobject.cpp:24-35`): `if (start_frame>0) rand(num_frames+1)`; then
  `if (time_to_explo_v) rand(time_to_explo_v)`.
- `Create1` (`:41-49`): `if (distribution) { rand(distribution*2)×2 }` then `Create`.
- `Create2` (`:51-66`): `rand(speed_v)` (always, **first**), then `if (distribution)
  { rand(distribution*2)×2 }`, then `Create`.

### `NObject::Process` (`nobject.cpp:68-234`) — 4c

worm-hit `rand(3)` + `rand(3)` + blood `rand(128)`×N (`:180-193`); on explode:
splinters `rand(128)`+`rand(2)`×`splinter_amount` + `Create2` (`:221-228`);
`dirt_effect>=0` ⇒ `DrawDirtEffect`.

### `DrawDirtEffect` (`gfx/blit.cpp:534`) — 4b

**Exactly one rand at the top:** `large_sprites.SpritePtr(tex.s_frame +
rand(tex.r_frame))` (`blit.cpp:537`). Everything after is deterministic per-pixel
material logic (`n_draw_back` branch; cases 1/2/6/10) writing `material_id`. No
further RNG. The crater shape is a function of (texture frame, clip rect, current
materials) — **pixel-exact** is the 4b challenge.

> **Audit conclusion for 4a:** with `fan`, worms kept out of `detect_distance`, and
> the non-firing worm invisible, the **only** RNG in 4a is the **4 Fire rands** at
> each fire tick. The `rng` column moves the instant a shot is fired; the `level`
> column stays constant (no `DrawDirtEffect`). This is the cleanest possible "RNG
> goes live" slice.

## Pool fill + iteration determinism

- **Spawn → `Pool::spawn` (lowest free slot).** C++ `Weapon::Fire`/`*::Create` call
  `NewObjectReuse()` (`exactObjectList.hpp:57`), which — when **not** full — calls
  `GetFreeObject()`: a free-list bitmap scanned with `countr_zero`, i.e. the
  **lowest free index**. The Rust `Pool::spawn` already reuses the lowest free slot
  (`pool.rs:60`) — a match. Iteration `All()` walks slots `0..Limit` skipping
  `!used` via the sentinel; `Pool::iter` filter-maps slots in index order — a match.
  The hash walks pools in this order, so it is part of the contract.
- **`NewObjectReuse` full-pool semantics differ — flag for later, harmless in 4a.**
  When the pool is **full**, C++ `NewObjectReuse` returns `&arr[Limit-1]`
  (**overwrites the last slot without freeing**, `count` unchanged), whereas Rust
  `Pool::spawn` returns `None`. The wobjects/sobjects/nobjects pools (600/700/600)
  never fill in any Slice-4 scenario, so 4a–4c are unaffected, but **Slice 6 fuzzing
  can hit it** — `Pool` needs a `spawn_reuse_last()` (or `spawn` must overwrite the
  last slot when full) to match. Recorded as **O3**; out of scope for 4a, noted in
  the 4a design so the contract is documented where the pool is first filled.
- **Free during the object loop.** C++ `Range`/`Free(this)` lets `Process` free the
  current object mid-iteration; the Rust driver walks slot indices and frees the
  current slot, copying the (Copy) element out to process it (so the rest of
  `SimState` — level, other pools, worms — can be borrowed mutably). New objects
  spawned into the **same** pool during its loop would be visited the same tick
  (C++ `Range.end == &arr[Limit]`); in 4a nothing spawns into `wobjects` during the
  wobjects loop, so this is a 4c concern (sobjects/nobjects re-enter) — flagged
  there.

## Datamodel additions (sketch; 4a details in its design)

- **`WObject`** gains `owner_idx: i32` (self-exclusion in the collide loop, owner for
  worm damage/recoil). `has_hit`/`fired_by` are **stats-only** (not hashed, no sim
  effect) → omitted. (4a.)
- **`SObject`** gains `anim_delay: i32` (read/written by `Process`; not hashed but
  load-bearing). (4c.)
- **`NObject`** gains `owner_idx`, `time_left` (Process/Create read them). (4c.)
- **`SimState`** carries the resolved object definition tables (`weapons`,
  `sobject_types`, `nobject_types`) and the `cossin` table (sim-core
  `precompute_cossin()`), plus a `DirtRock`-flavoured material probe and the
  textures + large-sprite bank for `DrawDirtEffect` (4b). 4a needs only `weapons` +
  `cossin` + the existing material probe.

## The hard 10% (carried across Slice 4)

- **RNG order across spawns** — the spread-x-then-y, then-colour, then-time-var
  order in `Fire`, and the short-circuited `AnyDirt && rand(8)` in the dirt-throw,
  are the desync traps. Thread one `sim-core::Rand` through the tick in C++ order;
  never pull ad hoc.
- **`DrawDirtEffect` pixel-exact** (4b, shared with the deferred dig) — the clipped
  16×16 blit, the `((my&15)<<4)+(mx&15)` texture wrap, and the per-material
  `n_draw_back` cases must rewrite `material_id` byte-for-byte, or the `level` hash
  diverges. Needs the large-sprite bank + `textures[]` from `assets` — **confirm
  coverage before 4b** (dependency).
- **Object-loop-before-worms ordering** and **cycles staying 0** — a reordered loop
  or an accidental `++cycles` (which gates trail/anim `% delay`) desyncs.
- **Pool free/spawn during iteration** + the **`NewObjectReuse` full semantics**
  (O3).
- **`CorrectShadow`** (`gfx/blit.cpp:624-639`) runs after `DrawDirtEffect` inside the
  same code path. **RESOLVED (pre-4b audit): it DOES write `material_id` (hashed)** —
  `SetPixel` at `blit.cpp:632/635` → `level.hpp:74`, reading `SeeShadow`/`DirtRock`. It
  is gated on the **GLOBAL `settings->shadow`** (default `true`, `settings.hpp:74`),
  **NOT a per-weapon shadow flag** (the earlier premise here was wrong). 4a is
  **unaffected**: fan has `dirt_effect=-1` ⇒ no `DrawDirtEffect` ⇒ no `CorrectShadow`.
  **4b decision (made — see the 4b design):** **OMIT via `settings->shadow=false` in
  the dumper.** Verified inert to 1-4a: the only other `settings->shadow` sim reader is
  `MakeShadow` (`level.cpp:426`, also writes `material_id`), reached **only** through
  `GenerateFromSettings` (`level.cpp:397`), which the dumper never calls (it uses
  `level.load()`). Re-diff the 1-4a goldens to prove it. Port `MakeShadow`+`CorrectShadow`
  together in a later dedicated shadow slice. **O4 — controller decision still requested.**
- **Object↔object / object↔worm collision** geometry (4c).

## Open questions for the controller

- **O1** — Accept the finer 4a (projectile lifecycle, level pristine, **fan**) over
  the brief's "4a = one sobject + level-hash"? *(Recommended: yes — thinner, isolates
  `DrawDirtEffect`.)*
- **O2** — Extend `sim_physics_dump.cpp` in place (re-diff slice-2/3 goldens) vs a
  new `oracle_dump_sim_frame`? *(Recommended: extend + re-diff.)*
- **O3** — When to make `Pool` match `NewObjectReuse`'s full-pool overwrite?
  *(Recommended: Slice 6, when fuzzing can fill pools; document in 4a.)*
- **O4** — RESOLVED (pre-4b audit): `CorrectShadow` **writes `material_id`** (hashed,
  `SetPixel` `blit.cpp:632/635`), gated on the GLOBAL `settings->shadow` (default
  `true`), not a per-weapon flag. 4a defers it naturally (fan `dirt_effect=-1` ⇒ no
  `DrawDirtEffect`/`CorrectShadow`). **4b recommendation (design written): OMIT via
  `settings->shadow=false` in the dumper** — proven inert to 1-4a (its other reader
  `MakeShadow` is only reached via `GenerateFromSettings`, never called by the dumper;
  re-diff gate). Port `MakeShadow`+`CorrectShadow` together later. *(Controller: confirm
  omit vs port-now.)*
- **O7** (new, 4b) — greenball's texture-6 dirt-effect is **additive** (`n_draw_back=
  false` creates dirt in Background; `blit.hpp:40`), so 4b proves "level-hash goes live"
  via *adding* terrain, not carving. Sufficient for the milestone, or also exercise a
  *carving* (`n_draw_back=true`) texture in 4b? *(Recommended: 4b additive + unit-tests
  the carving half; carving lands live in 4c/4d where sobject/dig textures use it.)*
- **O8** (new, 4b) — reuse `physics_fall_test.lev` (fire greenball at its sky-over-floor
  surface so the impact window has Background cells), or add a crater-fixture?
  *(Recommended: reuse it.)*
- **O5** (resolved → recommendation, 4c design written) — 4c weapon =
  **`dart→small_explosion`**: dart Fire draws **0** rand (`distribution=0`,
  `shotType=1` deterministic frame, `timeToExploV=0`, `wormCollide=false`) so every
  new `rand()` in 4c is in the explosion — clean isolation of the sobject + dirt-throw
  + dirt-debris cluster. `bazooka→large_explosion` (12 wobject splinters) is the
  splinter-path alternative — see O9. *(Recommended: dart→small_explosion; confirm.)*
- **O6** — Scenario weapon-override grammar: `weapon <slot> <name>` per-worm vs a
  single `loadout` line? *(Recommended: `weapon <slot> <name>`, applied to both
  worms; minimal.)*
- **O9** (new, 4c) — dart→small_explosion does **not** exercise the **splinter** RNG
  path (wobject `BlowUpObject` splinters `weapon.cpp:96-114`; nobject explode-splinters
  `nobject.cpp:221-228`): dart `splinterAmount=0`, sobjects don't splinter, and the
  dirt-debris `particle__disappearing` `splinterAmount=0`. Port the splinter code
  (guarded) in 4c, exercise it **live** in a 4c follow-up / 4d via
  **`bazooka→large_explosion`** — or fold a bazooka shot into the 4c scenario?
  *(Recommended: defer to a small bazooka follow-up; keep 4c one new RNG cluster.)*
- **O10** (new, 4c) — keep all worms **outside** every explosion's `±detect_range`
  box in 4c (no `DoDamage`/blow-away/blood; mirrors 4a/4b worm-inert posture) vs
  exercise explosion **damage** now? Damage drags in `DoDamage` mutating hashed worm
  fields **and** the **`cycles=0` blood-trail trap** (blood nobject 6 has
  `blood_trail`+`delay=10` ⇒ with frozen `cycles` it spawns a `BObject` *every* tick,
  storming `bobjects`). *(Recommended: worms out of range for 4c; land explosion
  damage+blood once `cycles` is freed (Slice 6) or with an explicit frozen-`cycles`
  decision.)*
- **O11** (new, 4c finding) — the **`nobjects` component hash fold is weaker than the
  master**: component folds only `pos.x,pos.y` (`stateHash.hpp:195-196`) while master
  folds `pos`+`vel`+`cur_frame`+`type->id` (`:85-92`). Nobject `vel`/`cur_frame`/`type`
  desyncs are invisible to the `nobjects` column and localise via the master only.
  *(Recommended: accept — it is the C++ contract; document, don't change
  `stateHash.hpp`.)*
- **O12** (new, 4d) — Prove the `load_change=false` *blocking* path via a new
  `load_change <0|1>` scenario directive, or accept the default-true (always-cycles)
  proof for 4d? *(Recommended: accept default-true; add the directive only when a TC
  ships `load_change=false`.)*
- **O13** (new, 4d) — One **combined** 4d scenario (handgun:
  fire→shell→reload→change + a dig window), or one scenario per deferral?
  *(Recommended: combined — last/cheap, fewer files, same rigor.)*
- **O14** (new, 4d) — Extend the `weapon` directive to `weapon <slot> <name> [ammo]`
  (optional low-ammo override, one dumper line) so reload is reached in a couple
  shots, or drive reload by firing handgun's full `ammo=15` (~300 ticks)?
  *(Recommended: add the optional `[ammo]` token.)*

## Next artifacts

- 4a design: `specs/2026-06-28-liero-rs-step2-slice4a-wobject-fire-lifecycle-design.md`
- 4a plan: `plans/2026-06-28-liero-rs-step2-slice4a-plan.md`
- **4b design: `specs/2026-06-28-liero-rs-step2-slice4b-dirt-destruction-design.md`** (planned)
- **4b plan: `plans/2026-06-28-liero-rs-step2-slice4b-plan.md`** (planned)
- **4c design: `specs/2026-06-28-liero-rs-step2-slice4c-explosion-objects-design.md`** (planned)
- **4c plan: `plans/2026-06-28-liero-rs-step2-slice4c-plan.md`** (planned)
- **4d design: `specs/2026-06-28-liero-rs-step2-slice4d-deferrals-design.md`** (planned)
- **4d plan: `plans/2026-06-28-liero-rs-step2-slice4d-plan.md`** (planned — executes LAST, after 4b+4c)

All four sub-slices are now specced. 4a is shipped; 4b is implemented (this PR); 4c
and 4d execute next, in order (4d last, since it reuses 4b's `draw_dirt_effect` and
4c's `NObject::Create`).
