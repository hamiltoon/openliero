# Step 2, Slice 4c — Explosion objects: `SObject::Create` + `NObject` (the object pools go live)

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-slice4-weapon-lifecycle-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice4b-dirt-destruction-design.md`
(the proven `sim` crate, `oracle_dump_sim_physics` per-tick oracle, the scenario
pipeline incl. the `weapon <slot> <name>` directive, the `process_frame`
ProcessFrame-subset driver, `worm_fire`/`wobject_process`/`blow_up`, and
**`draw_dirt_effect`** with **both** `n_draw_back` branches ported — all delivered
by 4a/4b, which this slice extends).

## Purpose

4c is the **third sub-slice** of the weapon-lifecycle milestone and the slice where
the **`sobjects` and `nobjects` pools go non-empty + hashed for the first time**
(exactly as 4a did for `wobjects`). It ports `BlowUpObject`'s `create_on_exp>=0`
branch (`weapon.cpp:89-92`) → **`SObjectType::Create`** (`sobject.cpp:16-228`: the
full explosion — sound, screen-flash/shake, the worm `DoDamage`+blow-away loop, the
wobject/nobject blow-away loops, the **dirt-throw** block that spawns dirt-debris
`NObject`s, blood, and the crater `DrawDirtEffect`) and **`NObject::Process`** +
`NObjectType::Create*` (`nobject.cpp:7-234`), using **`dart` → `small_explosion`**
— the cleanest weapon for isolating the explosion path (`dart` Fire draws **zero**
RNG, so every new `rand()` in 4c is in the explosion).

So 4c brings live, against their component+master hashes:

1. **A new RNG cluster** — the SObject sound `rand`, the dirt-throw `rand(8)`/
   `rand(128)` + the dirt-debris `NObject::Create2` draws, and (deferred, see O10)
   the worm blood. This is the single hardest contract in the slice: the C++ call
   order across `SObject::Create → NObject::Create2 → DrawDirtEffect` is exact.
2. **The first non-empty `sobjects` + `nobjects` hash folds** — validating the
   Slice-1 `hash.rs` folds for those two pools (the wobjects fold was proven in 4a).
3. **The first *live* exercise of the carving `DrawDirtEffect`** (`n_draw_back=true`,
   texture 2) — 4b ported the carving branch but only unit-tested it; `small_explosion`'s
   `dirtEffect=2` drives it for real, so the **`level` component hash moves here too**
   (4c reuses 4b's `draw_dirt_effect` verbatim — see *Level interaction*).

Everything else (Fire, flight, the `WObject` collide/explode path, the driver, the
pool free, `draw_dirt_effect`) is **unchanged from 4a/4b**; 4c adds the SObject and
NObject machinery plus the two new pools' iteration.

### What changes vs Slice 4b

| Invariant | 4b (greenball) | **4c (dart→small_explosion)** |
|---|---|---|
| `sobjects` pool | empty | **non-empty** — the explosion sobject spawns on explode, animates `num_frames=5`, frees |
| `nobjects` pool | empty | **non-empty** — dirt-debris `particle__disappearing` (nobject 2) spawned by the dirt-throw; flies + falls + frees |
| new RNG | Fire (3) + `DrawDirtEffect` `rand(2)` | **Fire 0** (dart) + the whole `SObject::Create` cluster (sound, dirt-throw, dirt-debris `Create2`) + `DrawDirtEffect` `rand(2)` |
| `BlowUpObject` | free + `dirt_effect` | + **`create_on_exp` → `SObject::Create`** (sound/sobject/splinter branches still skipped on the *wobject*; the sobject does the work) |
| `level` hash | live (additive, texture 6) | **live (carving, texture 2, `n_draw_back=true`)** — first live carve |
| `bobjects` / blood | empty | **still empty** (worm-damage/blood deferred — O10) |
| `cycles` | `0` | still `0` (load-bearing — see *The `cycles=0` blood-trail trap*) |

## The chosen weapon: `dart` → `small_explosion` (O5 confirmed)

Audited against the alternatives; **dart→small_explosion is the right rung** and
isolates the explosion RNG cleanly:

- **`dart`** (`weapons/dart.cfg`): `distribution=0` (no spread rand), `startFrame=113`
  + `shotType=1` (`kStdType1` ⇒ deterministic `cur_frame` from angle, `weapon.cpp:50-66`,
  **no rand**), `timeToExplo=0`/`timeToExploV=0` (no timeout, no time-var rand),
  `leaveShells=0`, `recoil=0`, `splinterAmount=0` (the **wobject** spawns no
  splinters), `dirtEffect=-1` (the wobject digs nothing), `wormCollide=false`
  (the `WObject::Process` worm-hit RNG loop is *never entered* — see *RNG audit*),
  `collideWithObjects=false`, `affectByExplosions=false`. So **`dart` Fire = 0 rands
  and `dart` `WObject::Process` = 0 rands**: *every* new `rand()` in 4c is inside the
  explosion. `gravity=200` + `explGround=true` + `bounce=0` + `timeToExplo=0` ⇒ it
  arcs and explodes on the **first** dirt/rock cell it touches (`weapon.cpp:249-256`),
  exactly like greenball — a deterministic, easy-to-aim ground explosion.
  `createOnExp="small_explosion"`.
- **`small_explosion`** (`sobjects/small_explosion.cfg`): `startSound="exp3a"` +
  `numSounds=2` ⇒ a `rand(2)` (sound; no-op for hashing but **the draw is consumed**
  — load-bearing); `damage=5`, `detectRange=8`, `blowAway=3000`; `dirtEffect=2`
  (texture 2, `n_draw_back=true` ⇒ **carving** crater); `numFrames=5`, `animDelay=2`
  (the sobject lives ~12 ticks ⇒ the `sobjects` hash actually moves over several
  ticks); `flash=0`, `shake=1` (both non-hashed).

**Alternatives (rejected for 4c):**
- **`bazooka` → `large_explosion`** is the richer path (the *wobject* carries
  `splinterAmount=12`, so `BlowUpObject` exercises the **splinter** RNG +
  `NObject::Create1`/`Create2` scatter, `weapon.cpp:96-114`). dart→small_explosion
  does **not** exercise the splinter path at all (dart `splinterAmount=0`;
  `small_explosion` is a sobject and sobjects never splinter; the dirt-debris nobject
  `particle__disappearing` has `splinterAmount=0`). Deferring splinters keeps 4c a
  single new RNG cluster (sobject + dirt-throw); the splinter path lands in a 4c
  follow-up or 4d — **O9**.
- **`small_explosion__silent`** drops the sound `rand(2)` (`startSound=-1`). Not
  worth it: the sound draw is a deterministic `rand(2)` and exercising it proves the
  `start_sound>=0` branch; keep the canonical `small_explosion`.

## Object spawn graph for 4c (one dart, worms out of range)

```
worm fires dart  ──(Fire, 0 rand)──▶ WObject (dart)
WObject::Process arcs (gravity 200) ──hits floor──▶ BlowUpObject (weapon.cpp:78-125)
   ├─ wobjects.Free(this)                                  (the dart removes itself first)
   ├─ create_on_exp=2 ▶ SObjectType::Create (small_explosion)   ← the whole 4c cluster
   │      ├─ sound rand(2)                                  (sobject.cpp:24)
   │      ├─ damage>0 worm loop ─ NO worm in detect_range ⇒ no draws (O10 posture)
   │      ├─ wobjects/nobjects blow-away loops ─ inert (single shot, affect_by_explosions=false)
   │      ├─ dirt-throw (sobject.cpp:188-205): per AnyDirt cell rand(8); on ==0 ▶
   │      │      rand(128) + nobject_types[2].Create2  ▶ NObject (particle__disappearing)
   │      │            Create2: rand(40)=rand(speed_v), rand(20000)×2=distribution     (nobject.cpp:51-66)
   │      └─ dirt_effect=2 ▶ DrawDirtEffect rand(2)  (carving, texture 2)  (sobject.cpp:209-210)
   ├─ explo_sound (dart "" ⇒ no-op, no rand)
   ├─ splinterAmount=0 ⇒ no wobject splinters
   └─ dirtEffect=-1 ⇒ the wobject itself digs nothing
NObject::Process (dirt debris) over later ticks ─ fly, gravity 700, free on ground (no rand)
SObject::Process (small_explosion) over later ticks ─ anim_delay/cur_frame, free at cur_frame>5 (no rand)
```

## Scope

### IN — ported this slice (C++ references)

- **`BlowUpObject`'s `create_on_exp>=0` branch** (`weapon.cpp:89-92`): `if
  (w.create_on_exp>=0) common.sobject_types[create_on_exp].Create(game, Ftoi(kX),
  Ftoi(kY), cause_idx, fired_by, this)`. Runs **before** the `explo_sound`, the
  splinters, and the `dirt_effect` branch (`weapon.cpp:94/96-115/117-124`) — the
  order is load-bearing. (The dart hits none of the latter three, but they stay
  guarded as in 4a/4b.)
- **`SObjectType::Create`** (`sobject.cpp:16-228`), the whole function, in C++
  statement order:
  - **sound** (`:23-25`): `if (start_sound>=0) rand(num_sounds)` — `small_explosion`
    `rand(2)`. The `sound_player->Play` is a no-op for hashing; **the `rand` is not**.
  - **shake** (`:27-33`) and **`screen_flash`** (`:41`): non-hashed (viewport shake;
    `game.screen_flash`); no rand. Skip the writes; keep none.
  - **`obj.{id,x,y,cur_frame,anim_delay}`** (`:35-39`): `x=x-8`, `y=y-8`, `cur_frame=0`,
    `anim_delay=anim_delay`. `id` + `cur_frame` are the hashed sobject fields
    (`stateHash.hpp:76-77`); `x`/`y`/`anim_delay` are **not hashed** but load-bearing
    for `Process` (so `SObject` carries them).
  - **`if (damage>0)` block** (`:47-207`) — for `small_explosion` (`damage=5`) the
    block runs; its three parts:
    - **per-worm loop** (`:48-114`): for each worm with `Ftoi(pos)` inside the
      `±detect_range` box (`:54-55`): blow-away `vel` nudge (`:60-80`, guarded on
      `|vel|<Itof(2)`, **no rand**), `DoDamage` (`:92-93`, **no rand** in normal mode
      — `game.cpp:567-589`), **blood** (`:96-103`: `kBloodAmount × {rand(128) +
      nobject_types[6].Create2}`), and the hit-sound `if (rand(3)==0){18+rand(3)}`
      (`:105-111`). **Deferred to O10**: in 4c the scenario keeps **all** worms
      outside the `±8`px box ⇒ this loop draws nothing and mutates no worm.
    - **wobjects blow-away loop** (`:118-153`) + **nobjects blow-away loop**
      (`:155-186`): mutate `vel` of *other* objects in range; **no rand**. Inert in
      the single-shot scenario (the dart already freed itself; `affect_by_explosions`
      is false for dart and `particle__disappearing`).
    - **dirt-throw** (`:188-205`): `kWidth=detect_range/2=4`; `Rect(x-4,y-4,x+5,y+5)`
      intersected with `level.Bounds()`; **row-major `for y { for x { … } }`**: `if
      (Mat(x,y).AnyDirt() && rand(8)==0) { kPix=Pixel(x,y); rand(128); nobject_types
      [2].Create2(angle=kPix-angle…, …, color=kPix, …) }`. The `&&` **short-circuits**
      — `rand(8)` is drawn **only** for `AnyDirt` cells, in scan order. This is the
      `nobjects`-pool source in 4c.
  - **`if (dirt_effect>=0)`** (`:209-215`, **outside** the damage block): `DrawDirtEffect
    (common, rand, level, dirt_effect, x-7, y-7)` (`:210`) — reuse 4b's `draw_dirt_effect`
    (texture 2, carving). `CorrectShadow` (`:212-214`) is gated on `settings->shadow`
    — **already `false`** from 4b's dumper line ⇒ omitted (O4).
  - **bonuses loop** (`:217-227`): empty pool in 4c ⇒ no rand, no recursion.
- **`SObject::Process`** (`sobject.cpp:230-241`): `if (--anim_delay<=0){ anim_delay=
  t.anim_delay; ++cur_frame; if (cur_frame>t.num_frames) sobjects.Free(this); }`. No
  rand. Drives the hashed `cur_frame` and the pool free.
- **`NObjectType::Create` / `Create1` / `Create2`** (`nobject.cpp:7-66`): 4c uses
  **`Create2`** (the dirt-throw call, `:51-66`): `rand(speed_v)` (always, **first**,
  `:53`); `if (distribution){ rand(distribution*2)×2 }` (`:58-61`); then `Create`
  (`:63`): `cur_frame` (start_frame>0 ⇒ `rand(num_frames+1)`, else color/`color_bullets`
  — `particle__disappearing` `start_frame=0`, `color=kPix≠0` ⇒ `cur_frame=kPix`,
  **no rand**, `:24-30`); `time_to_explo` (`:32`); `if (time_to_explo_v) rand(…)`
  (`:34-36`, `=0` ⇒ skip); finally `obj.pos += obj.vel` (`:65`). **Port `Create`/
  `Create1`/`Create2` whole** (the splinter path needs `Create1`/the `start_frame>0`
  `Create` rand later — O9 — so port the complete family now, guard the
  un-exercised draws with coverage notes).
- **`NObject::Process`** (`nobject.cpp:68-234`), the dirt-debris path:
  `pos+=vel` (`:74`); bounce (`:81-93`, `particle__disappearing` `bounce=0` ⇒ skipped,
  ported guarded); `blood_trail` (`:95-97`, `false` ⇒ skipped — **but see the
  `cycles=0` trap**); boundary clamp (`:100-113`); ground collision `if (!Inside ||
  PixelMat.DirtRock()){ vel.Zero(); if (expl_ground) do_explode }` (`:115-131`); else
  `vel.y += gravity` (`:140`); frame anim (`:143-158`, `num_frames=0` ⇒ skip); timeout
  (`:160-164`, `=0` ⇒ skip); worm-hit (`:166-203`, `hit_damage=0` ⇒ skip; ported
  guarded); explode (`:205-233`: `create_on_exp=-1`/`dirt_effect=-1`/`splinter_amount=0`
  ⇒ just `Free`). The dirt debris draws **no rand** in `Process`.
- **The driver's `sobjects` + `nobjects` loops go live** (`game.cpp:334-347`): they
  already exist (no-ops since 4a). 4c only makes them non-empty + threads the new
  args. **No dumper structure change** beyond what 4a/4b added (confirmed — see
  *Oracle / golden*).

### OUT — deferred (with target sub-slice)

| C++ | What | Deferred to |
|---|---|---|
| `weapon.cpp:96-114` | **wobject splinters** (`BlowUpObject`) — `Create1`/`Create2` scatter | **O9** (bazooka follow-up / 4d); dart `splinterAmount=0` |
| `nobject.cpp:221-228` | **nobject splinters** on explode (`rand(128)+rand(2)`+`Create2`) | O9 (`particle__disappearing` `splinterAmount=0`); ported guarded |
| `sobject.cpp:48-112` | **worm `DoDamage` + blow-away + blood + hit-sound** | **O10** (worms kept out of `detect_range` in 4c); ported guarded |
| `nobject.cpp:166-203` | NObject worm-hit damage/blood | O10 (`particle__disappearing` `hit_damage=0` ⇒ unreached) |
| `sobject.cpp:118-186` | wobject/nobject **blow-away** loops | inert in the single-shot scenario; ported (mutate `vel`, no rand) |
| `nobject.cpp:95-97`, `:119-128`, `:137` | `blood_trail` BObject, `draw_on_map` `BlitImageOnMap`, `leave_obj` sobject trail | Slice 6 / later (dirt debris sets none; **but see the `cycles=0` trap**) |
| `sobject.cpp:217-227` | bonus → sobject chain (`sobject_types[0].Create` recursion) | when bonuses go live (Slice 6); empty pool ⇒ unreached |
| `weapon.cpp:121-123`, `sobject.cpp:212-214`, `nobject.cpp:215-218` | `CorrectShadow` | omitted via `settings->shadow=false` (O4, 4b) |

> **Stats / audio / display are no-ops**, as in 4a/4b. `sound_player`,
> `screen_flash`, viewport `shake`, `DamagePotential`/`Hit`/`DamageDealt`, and the
> `has_hit`/`fired_by` fields are non-hashed — omitted. Only `material_id` (level)
> and the pool folds (`stateHash.hpp:72-110/179-210`) are hashed.

## RNG audit — the SObject::Create → NObject::Create2 → DrawDirtEffect order (the contract)

`rand()` consumption order **is** the contract. Audited from source (the `last`
written by each call is what the next reads). For 4c (dart→small_explosion, **worms
out of `detect_range`**), the **only** RNG is inside `SObject::Create`, in this exact
order at the explode tick:

1. **Sound** (`sobject.cpp:24`): `rand(num_sounds) = rand(2)` (`start_sound>=0`). **1
   draw.** *(Drawn even though the sound is a hashing no-op.)*
2. **Worm-damage block** (`sobject.cpp:47-114`): `damage=5>0`, but **no worm inside
   the `±detect_range=8`px box** (scenario constraint, O10) ⇒ the per-worm loop draws
   **nothing** (`DoDamage` draws nothing in normal mode anyway, `game.cpp:567-589`).
3. **Dirt-throw** (`sobject.cpp:188-205`), `kWidth=4`, `Rect(x-4,y-4,x+5,y+5) ∩
   Bounds()`, **row-major `y` outer, `x` inner**: for each cell, `Mat(x,y).AnyDirt()
   && rand(8)==0` — **`rand(8)` is drawn only for `AnyDirt` cells**, in scan order.
   On a `0` result, in order:
   a. `kPix = Pixel(x,y)` (no rand);
   b. `kAngle = rand(128)` (`:199`);
   c. `nobject_types[2].Create2(game, kAngle, fixedvec(), Itof(x,y), kPix, …)`
      (`:200`) → **`Create2`** (`nobject.cpp:51-66`): `rand(speed_v)=rand(40)` (`:53`,
      always first), then `distribution=10000>0` ⇒ `rand(20000)` then `rand(20000)`
      (`:59-60`), then `Create` (`:63`, **no rand**: `start_frame=0`, `color=kPix≠0` ⇒
      `cur_frame=kPix`; `time_to_explo_v=0`), then `obj.pos += obj.vel` (`:65`).
   So **per *spawned* dirt-debris**: `rand(8)`(==0) + `rand(128)` + `rand(40)` +
   `rand(20000)` + `rand(20000)` = **5 draws**; per *non-spawning* `AnyDirt` cell:
   `rand(8)` only (≠0); per *non-`AnyDirt`* cell: **0 draws** (short-circuit).
4. **Crater** (`sobject.cpp:209-210`): `DrawDirtEffect(…, x-7, y-7)` → `rand(tex.r_frame)
   = rand(2)` at the **top, before any pixel** (`blit.cpp:537`; reuse 4b's
   `draw_dirt_effect`). **1 draw.** `CorrectShadow` omitted (`settings->shadow=false`).
5. **Bonuses** (`sobject.cpp:217-227`): empty ⇒ 0 draws.

### The desync traps in this order

- **Sound `rand(2)` is consumed even though hashing ignores sound.** Skip it and
  every later `rand.last` shifts. Port the `start_sound>=0` guard + the draw; drop
  only the `Play`.
- **The dirt-throw `rand(8)` count is terrain-dependent.** It is drawn once per
  `AnyDirt` cell in the `9×9` box, **in row-major scan order**, reading the
  **pre-crater** material. A one-cell difference in which cells are `AnyDirt` (or a
  wrong scan order, or clipping the `Rect` to `Bounds()` differently) changes the draw
  count and desyncs everything downstream. The scenario's impact location + level
  geometry are therefore **load-bearing**; pin them and assert the `rng` column.
- **Order: dirt-throw reads terrain, *then* `DrawDirtEffect` carves it.** The
  dirt-throw scan (step 3) must run on the **original** material; `draw_dirt_effect`
  (step 4) writes after. Carving first would change which cells are `AnyDirt` and
  mis-count the `rand(8)`s.
- **`Create2` draws `rand(speed_v)` *unconditionally first*, then the two
  distribution draws** (`nobject.cpp:53` before `:58-61`). A common slip is to draw
  distribution first (that is `Create1`'s order, `:44-45`, where there is no speed
  draw) — `Create2` ≠ `Create1`.
- **`cur_frame=kPix` (no rand) for the dirt debris**, because `start_frame=0` and the
  passed `color=kPix≠0` (`nobject.cpp:26-27`). A texture with `start_frame>0` would
  draw `rand(num_frames+1)` here instead — irrelevant for `particle__disappearing`
  but the `Create` branch must be exact for the splinter path (O9).

## Pool fill + iteration determinism (sobjects/nobjects go live)

- **Spawn → `Pool::spawn` (lowest free slot)**, iterate `All()` in slot order — same
  `Pool<T>` contract proven for `wobjects` in 4a (`exactObjectList.hpp:36-94`;
  `pool.rs`). The `sobjects`/`nobjects` pools (caps **700**/**600**,
  `exactObjectList.hpp`) are first filled here; the **`NewObjectReuse` full-pool
  overwrite-vs-`None` divergence (O3)** is still deferred to Slice 6 — **but 4c can
  *approach* a cap**: a single `small_explosion` can spawn up to one dirt-debris per
  `AnyDirt` cell in the `9×9` box (≤81), and `particle__disappearing` lives several
  ticks; multiple shots compound. Keep the scenario's shot count low enough that
  `nobjects` stays well under 600 (assert it), so O3 stays out of scope. Recorded as
  a note where the pool is first filled.
- **Cross-pool spawn during the object loops — load-bearing ordering.** The dart
  explodes during the **`wobjects` loop**; its `SObject::Create` spawns into
  **`sobjects`** and **`nobjects`**. Because the loops run in fixed order
  `sobjects → wobjects → nobjects → bobjects` (`game.cpp:334-355`):
  - the **`sobjects` loop already ran** this tick ⇒ the new explosion sobject is
    **not** `Process`-ed on its birth tick (first anim next tick — `cur_frame`/
    `anim_delay` start at `0`/`anim_delay`). Correct and load-bearing.
  - the **`nobjects` loop has *not* run yet** ⇒ the new dirt debris **is** `Process`-ed
    on its birth tick (it moves immediately). The Rust driver must take its `nobjects`
    iteration snapshot **after** the `wobjects` loop completes (sequential loops each
    capturing the pool at their own start naturally satisfy this — but it is the exact
    behaviour to preserve, mirroring 4a's "Fire spawns after the wobjects loop" note,
    inverted across pools).
- **Free during the loop.** `SObject::Process`/`NObject::Process` free the current
  slot (`sobjects.Free(this)`/`nobjects.Free(this)`) mid-iteration — the same
  copy-out / write-back-or-free slot walk 4a established for `wobjects`. The dirt
  debris also frees itself on ground contact within its own `Process`.

## Hash-fold notes (first non-empty `sobjects` / `nobjects`)

The Slice-1 folds (`stateHash.hpp`) are exercised for the first time:

- **`sobjects`** — **master** folds `id` + `cur_frame` (`:76-77`); **component**
  `c.sobjects` folds the same two (`:184-185`). So the sobject diagnostic is
  *complete* (matches the master's sobject contribution) — a sobject divergence
  localises in the `sobjects` column. `x`/`y`/`anim_delay` are **not** hashed (so the
  `x=x-8`, `y=y-8` offset and `anim_delay` are load-bearing for `Process` timing but
  invisible to the hash except through `cur_frame`'s advance rate).
- **`nobjects`** — **master** folds `pos.x, pos.y, vel.x, vel.y, cur_frame, type->id`
  (`:85-92`); **component** `c.nobjects` folds **only `pos.x, pos.y`** (`:195-196`).
  **Fold subtlety (surface to the controller):** a nobject divergence in
  `vel`/`cur_frame`/`type` is **invisible to the `nobjects` component column** and
  shows up **only in the master**. So when localising a 4c desync, the `nobjects`
  column proves *position* but not *velocity/frame*; if the master diverges while
  `rng`/`level`/`sobjects`/`nobjects(pos)` all match, suspect a nobject `vel` or
  `cur_frame` (i.e. a `Create2` distribution/speed draw or the `kPix→cur_frame`
  mapping). Do **not** change `stateHash.hpp` to widen the diagnostic — it is the C++
  contract; just document the weaker localisation (O11).

## Level interaction — 4c reuses 4b's `draw_dirt_effect`; the level-hash moves here too

`small_explosion`'s `dirtEffect=2` ⇒ the explosion **carves** a crater (texture 2:
`s_frame=73`, `r_frame=2`, `m_frame=2`, **`n_draw_back=true`**, `tc.cfg:148-153`). So:

- **The `level` component hash moves in 4c** (at each explode tick), via the **carving**
  branch of `draw_dirt_effect` (`blit.cpp:551-583`) — the branch 4b ported but only
  *unit-tested*. **4c is its first *live* exercise**, which is the natural confirmation
  of 4b's **O7** ("the carving path lands live in 4c"). **Recommendation: reuse 4b's
  `draw_dirt_effect` verbatim** (do not pick a no-dig sobject) — porting the full
  function in 4b was *for* this. There is no clean no-dig explosion sobject anyway
  (the openliero explosions all set `dirtEffect`), and the dirt-throw already reads
  terrain, so the level is inherently coupled here.
- **No new level machinery.** 4c needs `set_material` + the flag reads + the texture/
  large-sprite assets — **all delivered in 4b**. The dirt-throw additionally calls
  `Level::Pixel(x,y)` (== `material_id[idx]`) and `Mat(x,y).AnyDirt()` — both already
  available (`any_dirt` from 4b, the pixel read is `material_id[idx]`).
- **Light-revision risk once 4b fully ships:** 4c *depends on* 4b's carving branch
  being bit-exact. If 4b's `n_draw_back=true` port has a bug (it is unit-tested but
  not yet golden-proven live), 4c is where it first fails. This is the only
  non-orthogonal coupling; otherwise 4c's object-spawn focus is independent of 4b.

## The `cycles=0` blood-trail trap (why worm-damage/blood is deferred — O10)

The driver freezes `cycles` at `0` (no `++cycles`, overview *Oracle / driver
decision*). Several `NObject::Process` side-effects are gated on `(cycles % delay)==0`
— which is **always true when `cycles==0`**:

- `blood_trail`: `if (blood_trail && blood_trail_delay>0 && (cycles % delay)==0)
  CreateBObject(…)` (`nobject.cpp:95-97`). The **blood** nobject (`nobject 6`) has
  `bloodTrail=true`, `bloodTrailDelay=10` ⇒ with `cycles=0` it would spawn a `BObject`
  **every** tick of its life, immediately driving the `bobjects` pool non-empty.
- `leave_obj` sobject trail (`:133-138`) and the `num_frames` anim cadence (`:144`)
  are similarly `cycles`-gated.

The dirt-debris (`particle__disappearing`) has `bloodTrail=false`, `leaveObjDelay=0`,
`numFrames=0` ⇒ **none of these fire**, so 4c's nobjects are clean. But the **blood**
path (only reached by a *worm hit* inside `SObject::Create`) would drag in the
`cycles=0` blood-trail BObject storm **plus** `DoDamage` mutating hashed worm fields.
**Decision (O10): keep all worms outside the explosion's `±detect_range` box in 4c**
— provably no worm-damage, no blow-away, no blood, no `bobjects` — mirroring 4a/4b's
"worm-hit inert" posture. Exercising explosion *damage* (and its blood/blow-away,
which mutate hashed `worm.health/vel/pos`) is its own sub-slice, and should land only
once `cycles` is freed (Slice 6) **or** with a deliberate decision on the frozen-
`cycles` blood-trail behaviour. The scenario asserts every worm's `health`/`vel`/`pos`
stay on their no-explosion trajectory.

## Datamodel additions (`sim` crate)

| New field / type | C++ | Why (non-hashed unless noted) |
|---|---|---|
| `SObject { id, x, y, cur_frame, anim_delay }` | `sobject.hpp` / `SObject::Create` | `id`+`cur_frame` **hashed** (`stateHash.hpp:76-77`); `x`/`y`/`anim_delay` non-hashed but read/written by `Process` (`sobject.cpp:36-39/234-240`) |
| `SimState.sobject_types: Vec<SObjectType>` | `common.sobject_types` | `Create`/`Process` read `start_sound`, `num_sounds`, `damage`, `detect_range`, `blow_away`, `dirt_effect`, `anim_delay`, `num_frames`, `flash`, `shake` |
| `NObject.owner_idx: i32`, `NObject.time_left: i32` | `NObject` | `Process`/`Create` read/write them (`time_left` non-hashed; `owner_idx` non-hashed) |
| `SimState.nobject_types: Vec<NObjectType>` | `common.nobject_types` | `Create*`/`Process` read `speed`, `speed_v`, `distribution`, `gravity`, `bounce`, `expl_ground`, `start_frame`, `num_frames`, `time_to_explo(_v)`, `hit_damage`, `splinter_*`, `dirt_effect`, `create_on_exp`, `color_bullets`, `blood_trail*` |
| `Pool<SObject>` cap 700, `Pool<NObject>` cap 600 | `exactObjectList.hpp` | the two pools go live; `spawn` asserts `Some` (O3 deferred) |

`NObject` already carries `{pos, vel, cur_frame, ty}` (the master-hashed fields,
`stateHash.hpp:85-92`) from the Slice-1 model; 4c adds `owner_idx`/`time_left`.
`SObjectType`/`NObjectType` are `assets::object::*` (parsed + golden-tested in 1e-2)
— 4c only **wires the tables into `SimState::new`** (the differential test loads them
from the TC exactly as the dumper's `common` already holds them). The `cossin` table
(needed by `NObjectType::Create2`, `nobject.cpp:55`) is already in `SimState` from 4a.

## Input scenario design

A **new** `golden/sim_slice4c_scenario.txt`, same grammar as 4a/4b (incl. `weapon
<slot> <name>`). Reuse `Levels/physics_fall_test.lev` (sky band over a solid Dirt
floor — the slice-2/3/4a/4b fixture). `seed 42`, `ticks ≈ 90`, two worms.

- `weapon 0 dart` (both worms; `current_weapon=0`).
- **Worm 0: aim toward the floor and Fire.** dart `gravity=200`, `timeToExplo=0` ⇒ it
  **must** hit the dirt to explode; aim down/forward so it arcs into the **dirt
  surface** within ~10–20 ticks. The impact must be **in/at dirt** so (a) the
  `WObject` explodes there, and (b) the dirt-throw's `9×9` box overlaps `AnyDirt`
  cells (so `nobjects` actually spawn). **Verify in the golden that `nobjects`
  goes non-empty and `level` changes at the explode tick.** Optionally fire a second
  dart so both pools and `level` move twice.
- **Worm 1: a Fire-free / divergent pattern**, kept invisible or far so it is never
  hit and never inside any explosion's `±8`px box.

**Load-bearing constraints (comment them in the file):**
- **All worms outside every explosion's `±detect_range=8`px box** (O10): no worm
  `DoDamage`/blow-away/blood ⇒ no `bobjects`, no hashed-worm mutation, no
  `cycles=0` blood-trail storm. Assert the worm `health`/`vel`/`pos` columns follow
  the no-explosion trajectory.
- **Impact straddles/enters dirt** so the dirt-throw box has `AnyDirt` cells (else
  `nobjects` never spawn — the point of 4c) **and** the `WObject` explodes (dirt/rock
  contact). The exact impact cell fixes the `rand(8)` count — pin it.
- **Keep `nobjects` well under 600** (O3 out of scope): a small shot count; assert the
  pool max.
- Health 100; never Left(4)+Right(8) together (dig deferred); non-firing worm invisible.
- Tune ticks so the golden shows: a fire tick (`rng` **does not** move — dart Fire is
  0 rand — but `ammo`↓, `delay_left=30`, a wobject appears), flight (wobject arcs
  under gravity), the **explode tick** (wobject gone; `rng` moves by the whole
  explosion cluster; `sobjects` non-empty (`id=2`); `nobjects` non-empty;
  `level` changes), sobject anim (`cur_frame` 0→5 over ~12 ticks then frees), and
  dirt-debris flight→free.

## Oracle / golden

The dumper already drives the ProcessFrame subset incl. the `sobjects`/`nobjects`
`Process` loops (4a) and `settings->shadow=false` (4b). **No dumper change is needed
for 4c** — `BlowUpObject`'s `create_on_exp` branch and `SObject::Create`/`NObject`
are real game code reached automatically once a dart explodes. Confirm by reading the
dumper (it should already contain the four object loops before the worm loop, no
`++cycles`, no bonus roll).

- **No C++ change in 4c** (the design's strongest property): unlike 4a (object loops +
  `weapon` directive) and 4b (`shadow=false`), 4c needs **zero** dumper edits. The
  only new code is Rust (`sim` + the test). **Required check:** regenerate the
  `sim_slice1/2/3/4a/4b` goldens and confirm **byte-identical** (a pure-Rust slice
  cannot perturb them — a trivial but worthwhile guard that the shared scenario/dumper
  path is untouched).
- **New** `golden/sim_slice4c_scenario.txt`, `gen_sim_slice4c_golden.sh` (copy of the
  4b gen script, pointed at the 4c files; LOCAL/MANUAL), committed
  `golden/sim_slice4c.txt`.
- **New** `tests/sim_slice4c_golden.rs`: assert the master `state_hash` **and** all 9
  component columns per tick (input keyed `k-1`, the established off-by-one), with a
  coverage guard that `sobjects` is non-empty for ≥1 tick (and `id=2`), `nobjects`
  goes non-empty for ≥1 tick, the `level` column changes ≥1 time, and `rng` moves at
  the explode tick (the cluster) **and does not move at the fire tick** (dart Fire = 0
  rand — a sharp, weapon-specific assertion). Assert `bobjects` stays empty and worm
  fields follow the no-explosion path (the O10 guard).
- The Rust builder maps `weapon 0 dart` to `WeaponInit { ty: Some(dart_id), ammo }` —
  same machinery as 4a/4b.

### Input timing (unchanged off-by-one)

Golden line `k` (`k≥1`) = state after `process_frame` with input keyed **`k-1`**;
line 0 = the pre-motion tick-0 state with no call (Slices 2/3/4a/4b *Input timing*).

## Definition of done

- [ ] `SObject` (`id,x,y,cur_frame,anim_delay`) + `Pool<SObject>` (cap 700);
      `NObject` gains `owner_idx`/`time_left`; `SimState` carries
      `sobject_types`/`nobject_types` (cossin already present); `SimState::new` updated;
      slice-2/3/4a/4b call sites updated.
- [ ] `sobject_create` ported (`sobject.cpp:16-228`): sound `rand`, obj init
      (`x-8,y-8`), the `damage>0` worm loop (**guarded** / inert in 4c via geometry),
      the wobject/nobject blow-away loops (no rand), the **dirt-throw** (`AnyDirt &&
      rand(8)`, short-circuit, row-major; `rand(128)`+`Create2`), and the
      `dirt_effect>=0 ⇒ draw_dirt_effect(x-7,y-7)` reuse — in **C++ order**. Bonus +
      `CorrectShadow` skipped. Unit-tested with `small_explosion` constants.
- [ ] `sobject_process` ported (`sobject.cpp:230-241`): `anim_delay`/`cur_frame`/free.
- [ ] `nobject_create`/`create1`/`create2` ported whole (`nobject.cpp:7-66`): `Create2`
      `rand(speed_v)` first, then distribution ×2, then `Create` (`cur_frame` branch,
      `time_to_explo_v`), then `pos+=vel`. Unit-tested (RNG order vs C++ stream).
- [ ] `nobject_process` ported (`nobject.cpp:68-234`): move, bounce (guarded),
      boundary clamp, ground/`expl_ground` explode, gravity, anim (guarded), timeout
      (guarded), worm-hit (**guarded**, O10), explode (`create_on_exp`/`dirt_effect`/
      splinters guarded — O9). Dirt-debris path draws no rand. Unit-tested.
- [ ] Driver: `sobjects`/`nobjects` loops go live (thread `sobject_types`/
      `nobject_types`/`cossin`/`large_sprites`/`textures` into them); cross-pool spawn
      ordering preserved (sobject not processed birth tick; nobject **is**); free-
      during-iteration. **No** cycles/bonus/ninjarope.
- [ ] **No C++ dumper change.** slice-1/2/3/4a/4b goldens **byte-identical**; new
      `sim_slice4c_scenario.txt` + gen script + committed `sim_slice4c.txt`.
- [ ] `tests/sim_slice4c_golden.rs`: master + 9 components match every tick; coverage
      guard — `sobjects`/`nobjects` go non-empty, `level` changes, `rng` moves at
      explode but **not** at the dart fire tick, `bobjects` empty, worm fields on the
      no-explosion path, `nobjects` max < 600.
- [ ] `cargo test --workspace` green; `sim` Bevy-free / float-free; deps unchanged.
- [ ] Determinism note: **no** sim-critical C++ changed (no dumper edit) ⇒
      `test_determinism`/`test_rollback_*` unaffected.

## The hard 10% (this slice)

- **The `SObject::Create → Create2 → DrawDirtEffect` RNG order** — sound `rand(2)`;
  the **terrain-dependent** dirt-throw `rand(8)` (one per `AnyDirt` cell, row-major,
  short-circuited, on **pre-crater** terrain) + `rand(128)` + `Create2`'s
  `rand(speed_v)`-**first**-then-distribution; then `DrawDirtEffect`'s `rand(2)`. Any
  mis-order, missing sound draw, wrong scan order, or carving-before-scan mis-counts
  the stream and desyncs every later `rand.last`.
- **Cross-pool spawn-during-loop ordering** — the explosion (in the `wobjects` loop)
  spawns a sobject (the `sobjects` loop already ran ⇒ **not** processed this tick) and
  dirt debris (the `nobjects` loop runs next ⇒ **is** processed this tick).
- **First non-empty `sobjects`/`nobjects` folds** — sobject `id`+`cur_frame` (complete
  diagnostic); nobject master folds `vel`/`cur_frame`/`type` that the **component fold
  omits** (O11) ⇒ localise nobject vel/frame desyncs via the master.
- **The `cycles=0` blood-trail trap** — exercising explosion damage would spawn blood
  (nobject 6, `blood_trail`+`delay=10`) that, with frozen `cycles`, storms `bobjects`
  every tick; deferred (O10) by keeping worms out of range.
- **Carving `DrawDirtEffect` goes live** — 4c is the first golden proof of 4b's
  `n_draw_back=true` branch (texture 2); a 4b carving bug surfaces here.
- **Pool caps approached** — a single explosion can spawn ≤81 dirt debris; keep the
  shot count low so `nobjects` < 600 and O3 stays deferred (assert it).
- **Truncating fixed-point** — `cossin*speed/100`, `Ftoi(x)-7/-8`, `vel/3` — Rust `/`,
  arithmetic `>>` for `Ftoi`; same discipline as 4a/4b.

## Open questions for the controller

- **O5 (resolved → recommendation):** 4c weapon = **dart→small_explosion** (dart Fire
  = 0 rand ⇒ all new RNG is in the explosion; clean isolation of the sobject + dirt-
  throw + nobject cluster). *(Recommended; confirmed against `bazooka` and the silent
  sobject variants.)* **Confirm.**
- **O9 (new):** dart→small_explosion does **not** exercise the **splinter** RNG path
  (wobject `BlowUpObject` splinters `weapon.cpp:96-114`; nobject explode-splinters
  `nobject.cpp:221-228`) — dart `splinterAmount=0`, the sobject doesn't splinter, and
  `particle__disappearing` `splinterAmount=0`. Port the splinter code (guarded) now,
  and exercise it **live** in a 4c follow-up / 4d via **bazooka→large_explosion** (12
  splinters), or fold a second bazooka shot into the 4c scenario? *(Recommended:
  defer to a small bazooka follow-up so 4c stays one new RNG cluster.)*
- **O10 (new):** keep all worms **outside** every explosion's `±detect_range` box in
  4c (no `DoDamage`/blow-away/blood; mirrors 4a/4b worm-inert posture) — vs exercise
  explosion **damage** now? Damage drags in `DoDamage` mutating hashed worm fields
  **and** the `cycles=0` blood-trail BObject storm (nobject 6). *(Recommended: worms
  out of range for 4c; land explosion damage + blood + blow-away once `cycles` is
  freed (Slice 6) or with an explicit frozen-`cycles` blood-trail decision.)*
- **O11 (new, finding):** the **`nobjects` component fold is weaker than the master**
  — component folds only `pos.x,pos.y` (`stateHash.hpp:195-196`) while master folds
  `pos`+`vel`+`cur_frame`+`type->id` (`:85-92`). Nobject `vel`/`cur_frame`/`type`
  desyncs are invisible to the `nobjects` column and show only in the master. Accept
  the weaker localisation (it is the C++ contract), or widen the diagnostic fold?
  *(Recommended: accept — do not change `stateHash.hpp`; document it.)*
- **O3 (carried):** `NewObjectReuse` full-pool overwrite vs `Pool::spawn → None` — 4c
  can *approach* the `nobjects` cap (≤81 debris/explosion). Still deferred to Slice 6;
  4c keeps the shot count low and asserts `nobjects < 600`. *(Recommended: hold to
  Slice 6.)*

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice4c-plan.md`.
