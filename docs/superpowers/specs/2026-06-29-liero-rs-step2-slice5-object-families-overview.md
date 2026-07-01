# Step 2, Slice 5 — Remaining object families: decomposition overview

Status: **draft for review** · 2026-06-29
Part of: `2026-06-28-liero-rs-step2-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice4-weapon-lifecycle-overview.md`
(the proven `sim` crate, the `oracle_dump_sim_physics` ProcessFrame-subset oracle,
the `weapon <slot> <name> [ammo]` scenario directive, the `process_frame` driver
this slice extends).

## Why this is an overview, not a single spec

Slice 4 proved **one** weapon end-to-end (fire → fly → explode → terrain), keeping
worms damage-inert and the ProcessFrame tail (`++cycles`, the bonus-drop roll,
ninjarope, game-mode) deliberately excluded. Slice 5 widens to the **remaining
object families** and, in doing so, ends three invariants Slices 1–4 leaned on:

1. **Worms take damage and bleed.** The sobject/wobject/nobject explosion paths
   call `Game::DoDamage` (mutating hashed `health`/`lives`) and spray **blood
   nobjects** (`nobject_types[6]`), which in turn spawn **`bobjects`** (the blood
   pool — its first non-empty appearance). The `worms`, `bobjects`, and `nobjects`
   component hashes all gain new live terms.
2. **`cycles` stops being `0`.** Blood-trail, `leave_obj`, `obj_trail`, and
   `part_trail` all gate on `cycles % delay == 0`. With the dumper's frozen
   `cycles=0`, `0 % d == 0` fires **every** tick — wrong vs a real game. Damage/blood
   (5b) forces the dumper to **advance `cycles`** (`game.cpp:357`), which also brings
   the **bonus-drop roll** (5c) and the worm-loop death path (5d) into reach.
3. **The bonus pool goes live.** `CreateBonus` (RNG position search), `Bonus::Process`,
   bonus pickup, and the recursive `sobject.cpp:217-227` bonus chain-loop (a known
   omitted, un-tripwired path) all land — and the per-tick bonus-drop roll
   (`rand(c[CBonusDropChance])`, `game.cpp:360`) starts consuming RNG every tick.

Plus the deepest path of all: **death + respawn** (`BeginRespawn`'s level-reading
RNG search, `worm.cpp:711`), which the Step-2 overview originally pencilled for
Slice 6 but which the controller (John, 2026-06-29) has pulled **into Slice 5** as
its final rung.

Because each is independently riskful, this doc **decomposes Slice 5 into sub-slices
5a–5d**, fixes the cross-cutting decisions (stats suppression, `cycles` advancement,
viewport count), and **fully specs only 5a** (companion design + plan). 5b–5d are
sketched here and detailed just-in-time, exactly as Slice 4 did 4b–4d.

## Decomposition (thin-vertical, simplest / lowest-coupling first)

| Sub-slice | Adds | Weapon / driver | New pool(s) | C++ dumper change | Traps |
|---|---|---|---|---|---|
| **5a — Splinters (O9)** ✅ *SHIPPED (PR #3, 131 ticks bit-exact)* | `WObject::BlowUpObject`'s **splinter arm** (`weapon.cpp:96-114`): the `splinter_scatter==0` branch → per splinter `rand(128)`+`rand(2)`+`NObject::Create2`. Replaces the `splinter_amount<=0` tripwire ported in 4c. **Under-scoped at planning:** cannon's `particle__small_damage` splinters `explGround`+`createOnExp` ⇒ they explode secondarily, so 5a ALSO ported `NObject::Process`'s `create_on_exp` (nobject→sobject) + restructured the worm-hit deferral to per-worm (DoDamage body stays O10/5b). This is intrinsic to splinters (all damaging splinter types explode). **`loading_time` golden (O19) DROPPED from 5a** → carryover (kept 5a focused after the scope growth). | **cannon** → `medium_explosion` + 5 splinters (scatter 0 ⇒ Create2). Worms **out of range** ⇒ no `DoDamage`. | — (reuses `nobjects`) | **none** (`weapon`+`[ammo]` already supported) | **none** — free of stats/cycles/viewport traps; first **live** exercise of the (already-ported) blow-away kick via cannon's `affect_by_explosions=true` (hash-neutral, freed-after) |
| **5b — Worm damage + blood (O10, headline)** ✅ *SHIPPED (PR #3, 121 ticks bit-exact)* | `Game::DoDamage`/`DoDamageDirect`; the **sobject** worm-damage loop (blow-away + `DoDamage` + blood spray + `rand(3)` gate/sound); **blood nobject (type 6)**; `CreateBObject` + `BObject::Process` (the `bobjects` pool goes live, swap-remove); nobject **blood-trail** arm. **Delivered via the sobject explosion path only** (closed-gate weapon **explosives**, wounding worm 100→82). **`cycles` advanced** (dumper + Rust; the prior goldens' master columns regenerated — O17 was a RIPPLE, not transparent; components stayed byte-identical). The dumper got the base `StatsRecorder` (O15) + a 1-line `bobjects.Resize` mirroring `StartGame`. Also ported the wobject **bounce + animation flight branches** (every closed-gate weapon needs them). **O16 dissolved** (no viewport-nesting — the damage loop is NOT inside `for(viewport)`). **DEFERRED to a follow-up slice (5′):** the per-pixel `CheckForSpecWormHit` + the wobject/nobject **in-flight** worm-hit arms (needed for open-gate weapons + slice-6 fuzz). | **explosives** → `large_explosion`; **`++cycles` advanced** in driver+dumper. | `bobjects` | base `StatsRecorder` (O15) + `++cycles` (O17) + `bobjects.Resize` | **`cycles` ripple** onto 2–5a goldens (master regenerated, components identical) |
| **5c — Bonuses** ✅ *MILESTONE GREEN (501 ticks bit-exact; pickup + chain-loop deferred)* | `++cycles` + **bonus-drop roll** (`game.cpp:360`) wired into the tick tail; `CreateBonus` (search RNG: `rand(W)`,`rand(H)` per trial, `rand(2)` frame, `rand(timer)`, weapon `rand(size)` reject-loop); `Bonus::Process` (fall/bounce/expire). **Delivered:** drop→fall→bounce→settle (frame-1 health bonus, worms clear ⇒ no pickup); spawn-flash `teleport_flash` `detectRange=0` ⇒ chain-loop inert & proven neutral by the match. **DEFERRED to slice 6 / follow-up:** **pickup** (health/weapon/booby, `worm.cpp:287-322`) + the recursive `sobject.cpp:217-227` bonus chain-loop (port + tripwire; needs the bonus pool threaded into `sobject_create`). | bonus-drop scenario; pickup scenario. | `bonuses` | extend driver tail (bonus-drop roll draw) | RNG-stream insertion point (between `++cycles` and worm loop) |
| **5d — Death + respawn** | death block (`worm.cpp:369-426`: death sound `rand(3)`, blood spray ×kMax, worm-parts spray ×7); pre-death drip (`rand(health+6)`/`rand(3)`/`rand(3)`); `killed_timer` countdown + lives gate; **`BeginRespawn`** (2 `rand` per trial, **live-level-dependent trial count**); `DoRespawning` (`DrawDirtEffect` + raw `rand()&1`). | a worm killed by an explosion, then respawns; **fuzz** the respawn search. | — | extend driver (full worm-loop death path; `quick_sim=false`) | **deepest desync risk** — trial count depends on live level + enemy pos |

**Ordering rationale (lowest-coupling first):**

- **5a is the only piece free of all three traps.** Splinters are spawned in
  `BlowUpObject` (**not** viewport-nested), draw no `DoDamage` (worms kept out of
  range, as in 4c), and don't touch `cycles`. The Create2 splinter arm is the one
  genuinely new code path; everything it calls (`Create2`, `medium_explosion`'s
  `SObject::Create`, the pools, the dirt-throw) is already proven by 4c. So 5a ships
  the splinter milestone while the stats/cycles/viewport decisions are still open —
  a clean thin-vertical first rung. Picking **cannon** (not bazooka) keeps it trap-
  free: cannon is `shotType=0`, `bounce=0`, no `objTrail`/`partTrail`, so the only
  new draws are the 5 splinters; bazooka/blaster/missile all carry a cycle-gated
  `objTrail` (`delay=4/8`) or a non-zero `shotType` that would drag in the very
  cycles trap 5a avoids. cannon's `affect_by_explosions=true` additionally drives the
  (already-ported, rand-free) wobject blow-away kick **live** for the first time —
  hash-neutral here because the cannon wobject is freed the same tick (before the
  hash), exactly as 4c proved the free-after ordering neutral (see O22). The
  independent `loading_time>0` golden (a 4d coverage gap) folds in here as cheap
  filler.
- **5b is the headline and the trap-bearer.** Damage couples in everything riskful at
  once: the **stats crash** (a C++-dumper fix), **`cycles` advancement** (for the
  blood-trail gate), and the **viewport-nesting** of the damage+dirt loops. Doing it
  second — after 5a proves the splinter/nobject machinery — isolates these three
  cross-cutting mechanisms into one slice whose golden is the O10 milestone. blood
  nobjects and `bobjects` land together because `bobjects` are spawned *only* by the
  blood-trail arm of blood nobjects, which are spawned *only* by damage.
- **5c needs the tick tail 5b builds.** The bonus-drop roll lives between `++cycles`
  and the worm loop, so it can only be wired once 5b has advanced `cycles`. Bonuses
  are otherwise self-contained (one new pool, deterministic `Bonus::Process`).
- **5d is deepest and last.** The death sprays live inside the worm loop (after
  `++cycles` + bonus roll), and `BeginRespawn`'s trial count depends on the
  *post-object-loop* live level (dirt/blood already applied) and the live enemy pos —
  the single most desync-sensitive path in Step 2. It needs blood (5b) and the full
  tick tail (5c) underneath it, and it is the rung that earns a **fuzz** pass.

## The chosen weapons

- **5a — `cannon`** (`weapons/cannon.cfg`): `splinterAmount=5`, `splinterScatter=0`
  ⇒ the **Create2** splinter branch, `splinterColour=66`,
  `splinterType=particle__small_damage`, `createOnExp=medium_explosion`,
  `shotType=0`, `bounce=0`, `timeToExplo=0`, `distribution=300`, **no `objTrail`/
  `partTrail`**, `gravity=700`, `explGround=true`. Firing it with worms out of range
  arcs into the floor, carves terrain via `medium_explosion` (proven 4c
  `SObject::Create` + dirt-throw + `dirtEffect=1` carve) **and** throws 5
  `particle__small_damage` splinters (the new arm). Fire draws **2 rand** (spread x,y;
  `startFrame=83>=0` with `loopAnim=false` ⇒ no frame draw, `timeToExploV=0`,
  `leaveShells=0`). **bazooka rejected** (`shotType=3` drunk + `objTrail delay=4`
  cycle-gated — not trap-free); blaster/missile likewise carry `objTrail`. Fire-RNG
  re-audited in the 5a design.
- **5b — bazooka or handgun aimed at a worm** (chosen at 5b spec): the worm sits
  inside `detect_range` so the damage loop fires. Needs the low-health worm to
  survive (no death yet — death is 5d), so damage is tuned to wound, not kill.
- **5c / 5d** — chosen just-in-time.

## Oracle / driver decision (the cross-cutting Slice-5 mechanisms)

### 5a — no dumper change

The dumper already accepts `weapon <slot> bazooka [ammo]` (4d). 5a's only sim change
is replacing `blow_up`'s `splinter_amount<=0` tripwire with the real Create2 splinter
loop. Re-diff the 1–4d goldens to prove byte-identity (pure-Rust slice, as 4c was).

### 5b — stats suppression + `cycles` advancement (the two dumper changes)

- **Stats suppression (bit-neutral).** `NormalStatsRecorder::DamageDealt`
  (`stats_recorder.cpp:44-77`) crashes headless: `worm_frame_stats.back()` on an
  empty vector (it is filled only by `PreTick`, which the subset dumper never calls)
  and OOB `weapons[type->id]`/`worms[index]`. Stats are **never hashed** (absent from
  `stateHash.hpp`), so suppressing them is bit-neutral. **Recommendation (O15):**
  construct the dumper's `Game` with the **base `StatsRecorder`** (the no-op,
  `stats_recorder.cpp:8-29`) — exactly what game clones do (`game.cpp:656`) — rather
  than `SetSpeculative(true)` (which would also gate the no-op base and additionally
  suppress sound; still bit-neutral but broader). The Rust sim models **no stats at
  all** (already the case).
- **`cycles` advancement.** Increment `cycles` at the `game.cpp:357` point (after the
  object loops, before the worm loop) in **both** the dumper subset and the Rust
  `process_frame`. This makes the `cycles % delay` trail gates (blood-trail delay 10,
  `leave_obj`, `obj_trail`, `part_trail`) fire on the correct ticks. **Ripple
  (O17):** any 4a–4c weapon that used a cycle-gated trail would change once `cycles`
  advances ⇒ its golden would need regenerating. Audit fan/greenball/dart/handgun for
  `obj_trail`/`part_trail`/`leave_obj` with a delay **before** turning `cycles` on;
  if none used them, the 1–4d goldens stay byte-identical and the re-diff is the gate.
  If any did, regenerate the affected golden(s) as part of 5b and note it.
- **Viewport count (O16).** The sobject worm-damage loop (`sobject.cpp:48-114`) **and**
  the dirt-throw block (`:188-205`) are nested inside `for (auto& viewport :
  game.viewports)` — they run **once per viewport**, so a 2-viewport game would apply
  damage/dirt **twice**. 4c's dirt-throw already fired, so the dumper has ≥1 viewport;
  **pin the exact count** and replicate it in the Rust driver before generating any
  damage golden. (4c's dirt-throw matched, so whatever count the dumper uses is
  self-consistent for the dirt loops; the damage loop's per-viewport repetition is
  the new thing to verify.)

### 5c — extend the tick tail

Add the single `rand(c[CBonusDropChance])` draw at `game.cpp:360` (between `++cycles`
and the worm loop) to both dumper and Rust. It draws **every** tick (short-circuit
only on `HBonusDisable`/`max_bonuses==0`); on a 0 result, `CreateBonus` runs.

### 5d — full worm-loop death path

The death sprays + `BeginRespawn` + `DoRespawning` run inside the worm loop. Set the
dumper's `quick_sim=false` (so `BeginRespawn` is reached, `worm.cpp:443`). The Rust
driver wires the death/respawn branch of `Worm::Process`.

## RNG audit — every new `rand()` site, in C++ call order

(Existing Fire / object / `DrawDirtEffect` draws audited in the Slice-4 overview.)

### 5a — `BlowUpObject` splinter arm (`weapon.cpp:96-115`)

After `create_on_exp`'s `SObject::Create` (its own proven draws — `medium_explosion`
sound `rand(4)`, dirt-throw, crater `rand(2)`) and **before** `dirt_effect`'s
`DrawDirtEffect` (cannon `dirt_effect=-1` ⇒ none): `if ((kSplinters = splinter_amount)
> 0)`, `splinter_scatter==0` branch, **per splinter** (×5 for cannon):
`rand(128)` (angle) → `rand(2)` (colour-sub) → `NObject::Create2(angle, vel=(),
pos=(kX,kY), splinter_colour - sub, ...)`, whose internals draw `rand(speed_v)`
(`particle__small_damage` `speedV=140` ⇒ `rand(140)`) → `rand(distribution*2)` ×2
(`distribution=2000` ⇒ `rand(4000)` x,y). `start_frame=0` ⇒ no frame draw;
`time_to_explo_v=0` ⇒ none. **5 draws × 5 splinters = 25 consecutive draws**, sandwiched
in the otherwise-proven `medium_explosion` stream (so a golden match validates exactly
the splinter port). The `scatter!=0` branch (`Create1`, no speed/angle draw) has **no**
weapon in this TC (only `mini_nuke` uses `scatter=1`, with the special `small_nukes`
splinter — out of 5a scope) ⇒ stays guarded + unit-tested (O18), like ProcessSight's
omission.

### 5b — worm damage + blood (`sobject.cpp:24-114`, `weapon.cpp:287-326`)

Per `SObjectType::Create`: sound `rand(num_sounds)` (top, if `start_sound>=0`) →
per in-range worm: blow-away vel-kick (**no rand**) → `DoDamage` (**no rand**) →
**blood** `kBloodAmount = settings->blood * power_sum / 100` times `[rand(128)` angle
+ `nobject_types[6].Create2` (= `rand(40)` speedV + `rand(40000)` ×2 dist)`]` → then
`if (rand(3)==0) rand(3)` (sound, inner draw always taken on the gate). `DoDamage`
itself is RNG-free in normal modes (verified). The wobject-hit path
(`weapon.cpp:287-326`) mirrors this. `BObject::Process` draws **≤1 `rand(3)`** on the
tick it lands/dies (`bobject.cpp:36/41/45`); `CreateBObject` draws **1 `rand` colour**
per bobject (`bobject.cpp:12`) — but bobjects are spawned only via the nobject
blood-trail arm (`nobject.cpp:95-97`, `cycles % 10 == 0`), not directly by damage.

### 5c — bonus drop + create (`game.cpp:360`, `:216-265`) + pickup (`worm.cpp:287-322`)

Drop roll: **1 `rand(CBonusDropChance)` every tick** at `:360`. On 0 → `CreateBonus`:
per trial `rand(BonusSpawnRectW)` + `rand(BonusSpawnRectH)` (×trials until a valid
5×5 non-Rock spot), then `rand(2)` frame (unless Only-Health/Weapon hacks),
`rand(timer)`, and **for weapon bonuses** a `do { rand(weapons.size()) } while
(banned)` reject loop (variable draws). Then `sobject_types[7].Create` flash.
Pickup: health bonus `rand(BonusHealthVar)` (only if `health<max`); weapon bonus
`rand(BonusExplodeRisk)` (always, `>1` reload / else booby `sobject_types[0].Create`).

### 5d — death + respawn (`worm.cpp:355-426`, `:711-808`)

Pre-death drip (`health < settings->health/4`): `rand(health+6)==0` → `rand(3)==0` →
`rand(3)` sound → `Create1` blood. Death block (`health<=0`): `rand(3)` death sound;
blood spray `kMax = 120*blood/100` times `[rand(128)` + `Create2`-internals`]`;
worm-parts `for i=7;i<=105;i+=14` (7×) `[rand(14)` + `Create2`-internals`]`.
`BeginRespawn`: **2 `rand` per trial** (`rand(WormSpawnRectW)`, `rand(WormSpawnRectH)`),
trial count = live-level + enemy-pos dependent (the desync trap). `DoRespawning`:
`DrawDirtEffect` draws + a single **raw `rand()&1`** (the only no-arg `rand()` in
this family) for aiming angle.

## Hash-fold table (`stateHash.hpp`) — master vs component, per family

| Family | Master `HashGameState` | Component hash |
|---|---|---|
| **worms** | pos,vel,aiming_angle,health,lives,kills,timer,visible,Pack(); per-weapon ammo,delay_left,loading_left,type→id; ninjarope out,pos (`:26-50`) | pos,vel,health,lives,visible,timer (`:145-153`), first 2 worms |
| **bobjects** | pos.x, pos.y (`:55-56`) | pos.x, pos.y (`:160-161`) — color/vel **never** hashed |
| **bonuses** | x, y, timer, weapon, **frame** (`:65-69`) | x, y, timer, weapon (**no frame**) (`:171-175`) |
| **sobjects** | id, cur_frame (`:76-77`) | id, cur_frame (`:184-185`) |
| **nobjects** | pos,vel,cur_frame,type→id (`:86-92`) | **pos.x,pos.y only** (`:194-197`) |
| **wobjects** | pos,vel,cur_frame,time_left,type→id (`:99-109`) | pos.x,pos.y only (`:204-208`) |
| global | rand.last, cycles, full material_id[] (`:18-23`) | rng=rand.last, level (`:132-139`) |

Asymmetries to remember: **bobjects fold pos only** (the blood pool's color and vel
are invisible to both hashes — a color/vel desync there is undetectable, so the swap-
remove **slot order** is the only thing the hash pins); **nobjects/wobjects component
folds pos only** (vel/frame/type desyncs localise via master); **bonuses frame** is
master-only.

## The hard 10% (carried across Slice 5)

- **`cycles` advancement is a tick-wide reordering.** Turning `cycles` on touches
  every `% delay` gate in already-live nobject/wobject trail code. Audit + re-diff
  before committing (O17).
- **Viewport-nested damage/dirt loops** — the per-viewport repetition is a silent
  RNG-count multiplier if the dumper's viewport count ≠ the Rust driver's (O16).
- **`bobjects` hash pins only slot order** — the `FastObjectList` **swap-remove**
  order (Slice-1 `BloodPool`) is the entire contract; a wrong free order desyncs even
  with correct positions.
- **`BeginRespawn` live-level-dependent trial count** (5d) — the canonical Step-2
  desync trap; the drop-down `while` reads post-mutation level pixels and the enemy
  pos, so the number of `rand` pairs consumed varies. Fuzz it (the C++ death-fuzz
  test is the template).
- **`NewObjectReuse` full-pool overwrite (O3, carried)** — blood/dirt storms in 5b/5d
  can push `bobjects`/`nobjects` toward their caps (700/600); the full-pool overwrite
  semantics (`exactObjectList.hpp:57` returns `&arr[Limit-1]`, Rust returns `None`)
  finally matters. Decide the `spawn_reuse_last` fix in whichever sub-slice first
  approaches a cap (likely 5b/5d).
- **The recursive bonus chain-loop** (`sobject.cpp:217-227`) — currently omitted with
  no tripwire; 5c must thread the bonus pool into `sobject_create` and either port
  the recursion or add a real tripwire before any bonus can sit near an explosion.
- **`blow_up` free-before vs free-after** (carried from 4c): the Rust driver frees the
  wobject *after* `blow_up`; C++ frees *before*. Hash-neutral while explosions don't
  chain (proven 4c), but 5b/5d with `affect_by_explosions=true` / `chain_explosion`
  **must** move the free before `blow_up`.

## Open questions for the controller

- **O15** (5b) — Stats suppression: install the base `StatsRecorder` (no-op) on the
  dumper's `Game` vs `SetSpeculative(true)`? *(Recommended: base `StatsRecorder` —
  what clones use; narrowest; bit-neutral. Rust models no stats.)*
- **O16** (5b) — Pin the dumper's **viewport count** and replicate it in the Rust
  driver before any damage golden, since the worm-damage + dirt-throw loops are
  viewport-nested (per-viewport repetition). *(Recommended: verify the dumper's count,
  pin it explicitly, mirror it; add a comment where the loop runs.)*
- **O17** (5b) — Before advancing `cycles`, audit fan/greenball/dart/handgun for
  cycle-gated trails (`obj_trail`/`part_trail`/`leave_obj` with a delay). *(Recommended:
  if none, the 1–4d goldens stay byte-identical (re-diff gate); if any, regenerate the
  affected golden as part of 5b and document it.)*
- **O18** (5a) — 5a weapon = **cannon** (scatter 0 ⇒ Create2 splinter arm live; the
  only no-trail, `shotType=0`, generic-splinter weapon). bazooka/blaster/missile carry
  cycle-gated `objTrail`s (would breach the cycles trap 5a avoids); the `scatter!=0` →
  `Create1` splinter branch has only `mini_nuke` (special `small_nukes` type) in this
  TC. *(Recommended: cannon; keep the `Create1` splinter branch guarded + unit-tested,
  land it live only if a later slice/TC needs it — the O9/ProcessSight omission
  pattern.)*
- **O19** (5a) — Fold the non-default `loading_time>0` golden (4d coverage gap:
  multi-tick reload countdown + `load_change=false` blocking path, currently unit-only)
  into 5a as cheap independent filler, or run it as a standalone cleanup task?
  *(Recommended: fold into 5a — it shares no code with splinters but is light and 5a is
  otherwise small.)*
- **O20** (5b) — Tune explosion damage so the hit worm is **wounded, not killed**
  (death is 5d), keeping 5b's golden free of the death path. *(Recommended: low-damage
  weapon or a high-health setting; verify `health>0` holds across the golden.)*
- **O21** (5d) — `BeginRespawn` fuzz coverage: how many seeds × ticks, and does the
  fuzz reuse the C++ `test_determinism` death-fuzz scenario? *(Recommended: finalise at
  the 5d spec; mirror the C++ death-fuzz loop.)*
- **O22** (5a) — cannon's `affect_by_explosions=true` makes the `medium_explosion`
  blow-away loop nudge the still-pooled cannon wobject (the driver frees it *after*
  `blow_up`, C++ frees *before*, `weapon.cpp:87`). The nudge draws **no rand** and the
  wobject is freed before the hash ⇒ **hash-neutral** (provable, as 4c proved free-after
  neutral for `affect=false`). Keep free-after in 5a (single new code path = the
  splinter loop) and document the neutrality, or land the free-before fix now?
  *(Recommended: keep free-after + document; land free-before in 5b/5d where a
  **surviving** `affect_by_explosions` object makes the ordering hash-relevant —
  re-diff 1–4d to prove either choice byte-neutral.)*

## Next artifacts

- 5a design: `specs/2026-06-29-liero-rs-step2-slice5a-splinters-design.md`
- 5a plan: `plans/2026-06-29-liero-rs-step2-slice5a-plan.md`
- **5b design+plan** (planned — damage + blood; the two dumper changes)
- **5c design+plan** (planned — bonuses; the tick tail)
- **5d design+plan** (planned — death + respawn; executes LAST, fuzzed)

Only 5a is fully specced here; 5b–5d are detailed just-in-time at their start (the
controller adjudicates the open questions then), exactly as Slice 4 did 4b–4d.
