# Step 2, Slice 4b — Terrain destruction: `DrawDirtEffect` (the level-hash goes live)

Status: **draft for review** · 2026-06-28
Part of: `2026-06-28-liero-rs-step2-slice4-weapon-lifecycle-overview.md`
Follows: `2026-06-28-liero-rs-step2-slice4a-wobject-fire-lifecycle-design.md`
(the proven `sim` crate, `oracle_dump_sim_physics` per-tick oracle, the scenario
pipeline incl. the `weapon <slot> <name>` directive, and the `process_frame`
ProcessFrame-subset driver — all delivered by 4a, which this slice extends).

## Purpose

4b is the **second sub-slice** of the weapon-lifecycle milestone and the slice
where the **`level` component hash goes live** — the first time it changes
*mid-run* since Slice 1. It ports `BlowUpObject`'s `dirt_effect>=0` branch
(`weapon.cpp:117-124`) → **`DrawDirtEffect`** (`gfx/blit.cpp:534-622`), the clipped
16×16 texture blit that rewrites `material_id` per pixel, using **`greenball`**
(`weapons/greenball.cfg`) — the one weapon that destroys/modifies terrain with
**zero** secondary objects (`create_on_exp=-1`, `splinter_amount=0`).

So 4b proves the genuinely new machinery — the first `material_id` **writer**, the
large-sprite-bank + texture-table read inside the sim, and the **one extra
`rand()`** that `DrawDirtEffect` draws — while still spawning **no**
sobjects/nobjects/blood (those are 4c). Everything else (the Fire path, the
`WObject::Process` flight/collision/explode path, the driver, the pool free) is
**unchanged from 4a**; 4b only adds the dirt-effect tail of `BlowUpObject` and the
assets it needs.

> **Naming note (load-bearing for the scenario).** The slice's headline is "destroys
> terrain", but greenball's texture (`dirt_effect=6`, `n_draw_back=false`) actually
> **creates** dirt: per the `Texture` struct comment (`gfx/blit.hpp:40-42`),
> `n_draw_back` is *"turned false for creating dirt and rock & turned true for
> cleaning dirt."* Greenball deposits a dirt blob into **Background** cells around
> the impact (`blit.cpp:584-621` — every write is guarded on
> `rowmatdest->Background()`). The defining property — *the `level` component hash
> becomes a time series* — holds identically whether the effect adds or removes
> material; but the scenario **must** place the explosion where Background cells
> exist in the 16×16 window, or `material_id` never changes (see *Input scenario
> design*). A true *carving* dirt-effect (`n_draw_back=true`, e.g. the dig texture 7
> / sobject texture 1/2) is exercised later (4c/4d) and is the **same code path** —
> only the texture's `n_draw_back` bit and the cases differ.

### What changes vs Slice 4a

| Invariant | 4a | **4b** |
|---|---|---|
| `level` component / master level-fold | constant Slice-1 value | **live** — `material_id` rewritten at each explode tick over Background cells |
| `rng` per fire | 4 Fire rands (fan) | **3 Fire rands** (greenball: spread x, spread y, colour) **+ 1** `rand(2)` inside `DrawDirtEffect` at each explode tick |
| weapon | fan (`dirt_effect=-1`) | greenball (`dirt_effect=6`, `gravity=700`, `timeToExplo=0`) |
| `SimState` assets | `weapons` + `cossin` | **+ `large_sprites` bank + `textures` table** (for `DrawDirtEffect`) |
| `BlowUpObject` | free only | free + **`dirt_effect` branch** (sound/sobject/splinters still skipped) |
| sobjects / nobjects / blood | empty | still empty (4c) |
| `cycles` | `0` | still `0` |

## Scope

### IN — ported this slice (C++ references)

- **`BlowUpObject`'s `dirt_effect>=0` branch** (`weapon.cpp:117-124`): when
  `w.dirt_effect >= 0`, call `DrawDirtEffect(common, rand, level, w.dirt_effect,
  Ftoi(kX)-7, Ftoi(kY)-7)`. The **`x-7,y-7`** offset (top-left of the 16×16 window
  centred on the impact) is load-bearing. The `CorrectShadow` call immediately after
  (`weapon.cpp:121-123`) is **gated on `settings->shadow`** — see *CorrectShadow
  decision (O4)*; **recommended: omit by setting `settings->shadow=false` in the
  dumper.**
- **`DrawDirtEffect`** (`gfx/blit.cpp:534-622`), the whole function:
  - `tex = common.textures[dirt_effect]` (`:536`); greenball `dirt_effect=6`.
  - **the one RNG draw, at the top** (`:537`): `t_frame =
    large_sprites.SpritePtr(tex.s_frame + rand(tex.r_frame))` — `rand(tex.r_frame)`
    is consumed **before any pixel is touched**. Greenball texture 6: `r_frame=2` ⇒
    `rand(2)`, `s_frame=82` ⇒ fill sprite `82 + rand(2)` ∈ {82,83}.
  - `m_frame = large_sprites.SpritePtr(tex.m_frame)` (`:538`); texture 6 `m_frame=38`
    — the hole-shape mask (16×16) whose pixel values `c` drive the `switch`.
  - clip to `Rect(0, 0, level.width, level.height - 1)` (`:545`), then the
    `CLIP_IMAGE`/`BLITL` walk over `material_id` with the parallel `materials` cache.
  - **`n_draw_back` branch** (`:551`). Greenball texture 6 has **`n_draw_back=false`**
    ⇒ the **else** block (`:584-621`):
    - `case 10: case 6:` if `rowmatdest->Background()` ⇒ `*rowdest = t_frame[((my&15)
      <<4)+(mx&15)]` (texture-wrapped fill), `*rowmatdest = materials[*rowdest]`.
    - `case 2:` if `Background()` ⇒ `*rowdest = 2`, recompute material.
    - `case 1:` if `Background()` ⇒ `*rowdest = 1`, recompute material.
    - `default:` nothing.
    - `my = y + y_`, `mx = x + x_` are the **level** coords (`x,y` = the passed
      top-left); `(my&15)<<4 | (mx&15)` is the **texture** wrap (`:559/593`).
  - (the `n_draw_back=true` block `:551-583` — `case 6` AnyDirt→texel, `case 1`
    Dirt2→2 / Dirt→1 — is **ported but not exercised by greenball**; it is the
    *carving* path used by the dig/sobject textures in 4c/4d. Port it now so the one
    function is complete, guard the un-exercised half with a coverage note.)
- **A `DirtRock` material probe** and **`Background`/`AnyDirt`/`Dirt`/`Dirt2`** flag
  helpers over the existing `material_flags` (4a added `dirt_rock`/`inside`; 4b adds
  the rest as 1-liners; `material.hpp:7-25`).
- **`SimState` carries the large-sprite bank + the texture table** so `DrawDirtEffect`
  can read them (datamodel, below).

### OUT — deferred (with target sub-slice)

| C++ | What | Deferred to |
|---|---|---|
| `weapon.cpp:121-123` `CorrectShadow` | shadow-correction pixel pass (writes `material_id`) | **omitted via `settings->shadow=false`** (O4); ported with `MakeShadow` in a dedicated shadow slice |
| `weapon.cpp:89-92` `create_on_exp` SObject | explosion sobject (sound, damage, dirt-throw, blood) | 4c (greenball `create_on_exp=-1` ⇒ none) |
| `weapon.cpp:96-115` splinters | splinter NObjects | 4c (greenball `splinter_amount=0` ⇒ none) |
| `weapon.cpp:94` `Play(explo_sound)` | explosion sound | no hashed effect; skipped (stats/audio) |
| `blit.cpp:551-583` `n_draw_back=true` block | the *carving* dirt-effect (dig, sobject dirt) | ported now (complete fn) but exercised in 4c/4d |
| `nobject.cpp:212`, `sobject.cpp:210`, `worm.cpp:783/931/941`, `level.cpp:125` | the **other** `DrawDirtEffect` call sites | their owning slices (4c sobject/nobject; 4d dig; level-gen never in the dumper) |
| worm-hit damage/blood/`worm_collide` (`weapon.cpp:287-326`) | greenball `worm_collide=true` ⇒ reachable | **excluded by geometry** (as 4a) — shots kept off visible worms |

> **Stats / audio are no-ops**, as in 4a. `explo_sound`, `DamagePotential`, the
> `materials` *display* cache, `display_valid`, and `MarkDirty` (rollback dirty
> tracking) have **no hashed effect** — only `material_id` is hashed (`hash.rs:36`,
> `fold_level`). The Rust port writes `material_id` and (for the branch reads)
> derives `Material` from `material_flags[material_id]`; it keeps **no** `materials`
> cache and **no** `display_valid`/dirty list.

## CorrectShadow decision (O4 — RESOLVED to a 4b choice)

`CorrectShadow` (`gfx/blit.cpp:624-639`) runs right after `DrawDirtEffect` inside the
same `if (w.dirt_effect>=0)` block, **gated on the GLOBAL `game.settings->shadow`**
(`weapon.cpp:121`; default `true`, `settings.hpp:74` — **not** a per-weapon flag;
greenball's own `shadow=true` field drives only sprite rendering, never this).
**It WRITES `material_id`** (hashed): `SetPixel(x, y, kPix±4, common)` at
`blit.cpp:632/635` → `level.hpp:72-80`, reading `Mat(x,y).SeeShadow()`
(`material.hpp:19`, bit `1<<4`) and `Mat(x+3,y-3).DirtRock()` (`material.hpp:22`,
bits 0|1|2) over the rect `Rect(kIx-10,kIy-10,kIx+11,kIy+11)` intersected with
`Rect(0,3,width-3,height)` (`blit.cpp:625`).

**Recommendation: OMIT it for 4b by setting `settings->shadow=false` in the dumper
(1 line).** Reasoning:

1. **It is provably harmless to Slices 1–4a.** `settings->shadow` has only two sim
   readers: (a) `CorrectShadow` at every `DrawDirtEffect` call site — none fire in
   1–4a; (b) `MakeShadow` (`level.cpp:426`, which *also* writes `material_id`) — but
   that is reached **only** through `Level::GenerateFromSettings` (`level.cpp:397`),
   which **the dumper never calls** (it loads a fixed `.lev` via `level.load()`
   directly — `sim_physics_dump.cpp` comment: *"must NOT call … GenerateFromSettings"*).
   So flipping the flag changes **nothing** in 1–4a. **Gate: regenerate and re-diff
   the slice-1/2/3/4a goldens — they must be byte-identical** (the same re-diff gate
   4a used for the dumper extension).
2. **It is not the slice's defining surface.** `CorrectShadow` is a *second*,
   independently-fiddly pixel pass (neighbour offset `+3,-3`, palette `±4`
   arithmetic, the `164..=167` un-shadow range, its own clip-rect intersection). 4b's
   value is proving the **first `material_id` writer** and the live level-fold with
   **exactly one** new pixel-writer. Adding a second doubles the risk for no
   milestone gain.
3. **It defers cleanly and amortizes.** `CorrectShadow` is shared by *every*
   `DrawDirtEffect` caller and is the near-twin of `MakeShadow` (same
   `SeeShadow && DirtRock ⇒ +4` logic). Porting both together in a dedicated
   shadow slice — tested against several callers and against level-load — is cheaper
   and better-covered than bolting a half-tested copy onto 4b.

**Cost / documented divergence:** with `shadow=false` the oracle diverges from
default *gameplay* (where shadow is on). That is acceptable — the oracle's contract
is "match the Rust sim", and **both** omit it. Recorded as a known divergence to
revisit when shadows are ported; at that point the 4b golden is regenerated with
`shadow=true` + the ported `CorrectShadow`. **The harmlessness depends on the dumper
continuing to use `level.load()` (never `GenerateFromSettings`)** — if a future
slice switches to random generation, `shadow=false` would change the initial level
and this decision must be revisited.

*(Alternative, not recommended: keep `shadow=true` and port `CorrectShadow`
bit-for-bit from `blit.cpp:624-639` now. Correct, but front-loads the shadow surface
into the slice that should isolate `DrawDirtEffect`.)*

## RNG audit — 4b adds exactly one draw

Greenball under the 4b scenario consumes RNG at two moments (audited from source;
the `last` each call writes is what the next reads):

1. **`Worm::Fire`** (`worm.cpp:1099-1148` → `weapon.cpp:16-76`) — **3 rands per fire
   tick**, in order: spread `vel.x` `rand(distribution*2)=rand(16000)`, spread `vel.y`
   `rand(16000)` (`weapon.cpp:34-37`, `distribution=8000`); colour `cur_frame =
   color_bullets - rand(2)` (`weapon.cpp:39-69`, `start_frame=-1` path). **No
   time-var rand** (`time_to_explo_v=0`). *(One fewer than fan, which adds the
   time-var draw.)*
2. **`DrawDirtEffect`** at the explode tick (`blit.cpp:537`) — **exactly 1 rand**:
   `rand(tex.r_frame) = rand(2)`. Drawn **at the top, before any pixel write**;
   everything after is deterministic per-pixel material logic. This is the **only**
   RNG `DrawDirtEffect` draws.

Excluded by construction (assert in the golden), same posture as 4a:

| Other `rand()` site | Reached when | Excluded by |
|---|---|---|
| `WObject` worm-hit (`weapon.cpp:287-326`) | worm within `detect_distance=3`; greenball `worm_collide=true` ⇒ guard true | non-firing worm **invisible** (`CheckForSpecWormHit→false`, `worm.cpp:1163`) + firer kept off the flight path |
| `WObject::Process` timeout (`weapon.cpp:281-285`) | `time_to_explo>0` | greenball `time_to_explo=0` ⇒ never (explodes only on ground/worm) |
| splinters / `create_on_exp` dirt-throw / blood | `splinter_amount>0` / sobject created | greenball `=0` / `create_on_exp=-1` |
| `n_draw_back` part-trail / drunk spread | shot_type 3 / part-trail set | greenball `shot_type=0`, no trail |

### RNG-ordering finding (the master-hash risk)

The draw order **greenball Fire (3) … flight … DrawDirtEffect `rand(2)` (1)** is
load-bearing: the `rng` component column (and the master's `rand.last` term) moves
at the explode tick *because of* the `rand(2)` — get the draw **placement** wrong
(e.g. draw it after the blit, or skip it) and every subsequent tick's `rand.last`
desyncs. **`r_frame=2` (not 0)** for greenball's texture, so the draw is a genuine
`rand(2)` and the `rand(0)` edge case does **not** arise here — but note for the
general port: `sim_core::Rand::operator()(0)` / `rand(0)` behaviour (typically
returns 0 with a defined `last` update, or is UB-adjacent) **is** load-bearing for
any texture with `r_frame==0`; verify `sim-core`'s `rand(0)` matches the C++
`Rand::operator()` (`rand.hpp`) before a `r_frame==0` texture is ever exercised
(none of the 9 openliero textures has `r_frame==0` — all are `r_frame=2`,
`tc.cfg:134-195` — so this is a forward-looking note, not a 4b blocker).

## Datamodel additions (`sim` crate)

No **new hashed field** — `material_id` is already the hashed level state
(`hash.rs:36`). Added to `SimState` (and threaded through `process_frame` into
`blow_up`/`draw_dirt_effect`):

| New field / type | C++ | Why (non-hashed unless noted) |
|---|---|---|
| `SimState.large_sprites: assets::sprite::SpriteSet` | `common.large_sprites` | `DrawDirtEffect` reads the fill (`s_frame+rand`) and hole-mask (`m_frame`) 16×16 sprites via `SpriteSet::sprite(frame)` (≡ `SpritePtr`) |
| `SimState.textures: Vec<assets::tc::Texture>` | `common.textures` (9 entries) | `DrawDirtEffect` reads `textures[dirt_effect]` `{s_frame, r_frame, m_frame, n_draw_back}` |
| `LevelSim::background(x,y)`, `any_dirt`, `dirt(x,y)`, `dirt2(x,y)` | `Material::Background/AnyDirt/Dirt/Dirt2` | the `DrawDirtEffect` per-pixel branch reads them; 1-liners over `material_flags[material_id[idx]]` |
| `LevelSim::set_material(idx, palidx)` | the blit's `*rowdest = …` write | the **first** `material_id` writer; sets `material_id[idx]` (no `materials`/`display_valid`/dirty list — non-hashed) |
| flag consts `MAT_DIRT=1<<0`, `MAT_DIRT2=1<<1`, `MAT_ROCK=1<<2`, `MAT_BACKGROUND=1<<3` | `material.hpp:7-10` | already present from 4a (`dirt_rock`); `background`/`any_dirt` reuse them |

`assets::sprite::SpriteSet` and `assets::tc::Texture` are already parsed and
golden-tested (sprite.rs; `tc.rs:145-150` `Texture`, `tc.rs:325` `textures`,
`tc.rs:324` `material_flags`). 4b's only assets work is **wiring** them into
`SimState::new` (and the differential test loads them from the TC exactly as the
dumper's `common` already holds them). **No prequel/asset task is needed.**

### `DrawDirtEffect` port shape

A free function `draw_dirt_effect(level, large_sprites, textures, dirt_effect, x, y,
rand)`:

1. `tex = textures[dirt_effect]`; `fill_base = tex.s_frame + rand(tex.r_frame)`
   (**draw RNG here, first**); `fill = large_sprites.sprite(fill_base)` (256 bytes,
   16×16); `mask = large_sprites.sprite(tex.m_frame)`.
2. clip the 16×16 window at top-left `(x,y)` to `Rect(0,0,width,height-1)` — port
   `CLIP_IMAGE` exactly (the per-edge start/extent adjustment; `height-1`, **not**
   `height`).
3. for each in-clip mask pixel `c` at window offset `(x_, y_)` mapping to level
   `(mx,my)=(x+x_, y+y_)` and `idx = mx + my*width`:
   - if `tex.n_draw_back`: `match c { 6 => if any_dirt(idx) { set fill_texel }, 1 =>
     if dirt2(idx){set 2} else if dirt(idx){set 1}, _ => {} }`
   - else: `match c { 6|10 => if background(idx){set fill_texel}, 2 => if
     background(idx){set 2}, 1 => if background(idx){set 1}, _ => {} }`
   - `fill_texel = fill[((my&15)<<4) + (mx&15)]` (**texture wrap on level coords**).
   - `set v` ⇒ `level.set_material(idx, v)` (writes `material_id[idx]` only).

Truncating arithmetic and the **exact** `CLIP_IMAGE`/`BLITL` index math are the
hard part (below).

## Input scenario design

A **new** `golden/sim_slice4b_scenario.txt`, same grammar as 4a (incl. `weapon
<slot> <name>`). Reuse `Levels/physics_fall_test.lev` (open sky band, Background
mat 130, over a solid Dirt floor at y=200px, mat 12 — the slice-2/3/4a fixture).
`seed 42`, `ticks ≈ 90`, two worms.

- `weapon 0 greenball` (both worms; `current_weapon=0`).
- **Worm 0: aim toward the floor and Fire.** Greenball has `gravity=700` (a
  parabola, **not** fan's straight line) and `time_to_explo=0` (**no** timeout
  explosion), so it **must** hit the ground to explode (`expl_ground=true`,
  `bounce=0` ⇒ `do_explode` on the first dirt/rock cell, `weapon.cpp:249-256`).
  Aim down/forward so the ball arcs into the floor within ~10–20 ticks.
- **Worm 1: a Fire-free / divergent pattern**, kept invisible or far so it is never
  hit.

**Load-bearing constraints (comment them in the file):**
- **The impact window must straddle the dirt surface** so the 16×16 region contains
  **Background** cells — otherwise greenball's `n_draw_back=false` effect writes
  nothing and the `level` hash never moves (the whole point of 4b). Firing into the
  floor *surface* (sky above, dirt below) guarantees background cells in the upper
  rows of the window. **Verify in the golden that the `level` column actually changes
  at the explode tick**; if it does not, the impact landed in solid dirt (no
  background) — move the impact up to the surface.
- Keep both worms `health 100`; never set Left(4)+Right(8) together (dig deferred);
  place worms so **no shot passes within `detect_distance=3`+sprite of a *visible*
  worm** (greenball `worm_collide=true` ⇒ the worm-hit RNG is reachable otherwise);
  the non-firing worm invisible.
- Tune ticks so the golden shows: a fire tick (`rng` moves by 3 draws, `ammo`↓,
  `delay_left`=4, a wobject appears), flight (wobject `pos`/`vel` arc under gravity),
  and the **explode tick** (wobject disappears, `rng` moves by the `DrawDirtEffect`
  `rand(2)`, **`level` column changes**). Optionally fire a second greenball so the
  level changes **twice** (proves a genuine time series, not a one-off).

## Oracle / golden

Per the 4a-established pipeline (the dumper, `weapon` directive, and `process_frame`
all already exist):

- **C++ dumper: minimal extension of `sim_physics_dump.cpp`** — (a) set
  `settings->shadow = false` once after constructing `Settings` (the O4 omission);
  (b) **nothing else** — the object loops, `weapon` directive, and ProcessFrame
  subset are already in place from 4a. `BlowUpObject`'s `dirt_effect` branch is real
  game code reached automatically once a greenball explodes. **Required check:**
  regenerate `sim_slice1/2/3/4a` goldens and confirm **byte-identical** (the
  `shadow=false` flip is inert for them — proves it didn't perturb the prior proofs).
- **New** `golden/sim_slice4b_scenario.txt`, `gen_sim_slice4b_golden.sh` (copy of
  the 4a gen script, pointed at the 4b files; LOCAL/MANUAL), committed
  `golden/sim_slice4b.txt`.
- **New** `tests/sim_slice4b_golden.rs`: assert the master `state_hash` **and** all
  9 component columns per tick (input keyed `k-1`, the established off-by-one), with
  a coverage guard that the **`level` column changes ≥1 time** (and ideally takes ≥3
  distinct values for two shots), `wobjects` is non-empty for some ticks, and
  `rng`/`ammo` each take ≥2 distinct values.
- The Rust builder maps `weapon 0 greenball` to `WeaponInit { ty: Some(greenball_id),
  ammo }` for slot 0 (same machinery as 4a's fan).

### Input timing (unchanged off-by-one)

Golden line `k` (`k≥1`) = state after `process_frame` with input keyed **`k-1`**;
line 0 = the pre-motion tick-0 state with no call (Slices 2/3/4a *Input timing*).

## Definition of done

- [ ] `SimState` carries `large_sprites: SpriteSet` + `textures: Vec<Texture>`;
      `SimState::new` takes them; `LevelSim` gains `background`/`any_dirt`/`dirt`/
      `dirt2` + `set_material`.
- [ ] `draw_dirt_effect` ported (whole `blit.cpp:534-622`): the top `rand(r_frame)`
      first, `CLIP_IMAGE` to `Rect(0,0,w,h-1)`, both `n_draw_back` branches (cases
      1/2/6/10), the `((my&15)<<4)+(mx&15)` texture wrap, writing `material_id` only.
      Unit-tested with the greenball texture (6, `n_draw_back=false`) **and** a
      `n_draw_back=true` texture (the carving half), incl. a clip-at-edge case.
- [ ] `blow_up` extended: `dirt_effect>=0` ⇒ `draw_dirt_effect(... Ftoi(x)-7,
      Ftoi(y)-7 ...)`; `create_on_exp`/splinters/sound still skipped (greenball
      hits none). `CorrectShadow` **omitted** (O4 — dumper `shadow=false`).
- [ ] Dumper: `settings->shadow=false` added; slice-1/2/3/4a goldens **byte-
      identical** after; new `sim_slice4b_scenario.txt` + gen script + committed
      `sim_slice4b.txt`.
- [ ] `tests/sim_slice4b_golden.rs`: master + 9 components match every tick; coverage
      guard — **`level` column changes** (≥1, ideally ≥2 distinct post-explode
      values), `wobjects` non-empty, `rng`/`ammo` move.
- [ ] `cargo test --workspace` green; `sim` Bevy-free / float-free; deps unchanged
      (`sim-core`, `assets`).
- [ ] Determinism note: only the dumper (oracle-gated, non-sim, `shadow=false`) C++
      changed ⇒ `test_determinism`/`test_rollback_*` unaffected.

## The hard 10% (this slice)

- **`DrawDirtEffect` pixel-exact.** The clipped 16×16 blit must rewrite `material_id`
  byte-for-byte: the `CLIP_IMAGE` start/extent math (clip to `height-1`, **not**
  `height`), the `((my&15)<<4)+(mx&15)` **level-coord** texture wrap, the per-material
  `n_draw_back` cases, and **which** cells the `Background()`/`AnyDirt()` guard skips.
  A one-pixel offset or a wrong guard diverges the `level` fold.
- **The `rand(r_frame)` placement.** Drawn **first, before the blit** (`blit.cpp:537`).
  Greenball `r_frame=2` ⇒ a real `rand(2)`; misplacing or skipping it desyncs every
  later `rand.last`. (`r_frame==0` `rand(0)` semantics: forward-looking note.)
- **Background-vs-dirt scenario geometry.** `n_draw_back=false` writes **only**
  Background cells; the impact must straddle the surface or `level` never moves —
  assert it does in the golden.
- **Reading `Material` from `material_flags`, writing `material_id` only.** No
  `materials` cache, no `display_valid`, no dirty list (non-hashed). The branch reads
  `material_flags[material_id[idx]]`; the write sets `material_id[idx]` (the next
  read re-derives).
- **Truncating fixed-point** (`Ftoi(x)-7` = `(x>>16)-7`; arithmetic shift) — same
  discipline as 4a; `Ftoi` is `>>16`, not `/65536` on negatives differing.
- **CorrectShadow stays out.** The `settings->shadow=false` flip must be re-diffed
  against 1–4a goldens to prove it is inert.

## Open questions for the controller

- **O4 (resolved → recommendation):** OMIT `CorrectShadow` via `settings->shadow=false`
  in the dumper (provably inert to 1–4a; re-diff gate), vs port it bit-for-bit now.
  *(Recommended: omit; port `MakeShadow`+`CorrectShadow` together in a later shadow
  slice.)* **Decision needed.**
- **O7 (new):** greenball's texture-6 effect **adds** dirt (`n_draw_back=false`), so
  4b's headline is more precisely "level-hash goes live", not "destroys terrain". Is
  proving the **additive** path sufficient for the milestone, or should 4b *also*
  exercise a **carving** texture (`n_draw_back=true`) in the same golden (e.g. via the
  dig/sobject texture through a second weapon) to cover both `switch` branches? *(
  Recommended: 4b proves additive + unit-tests the carving half; the carving path
  lands live in 4c (sobject dirt-throw textures 1/2) / 4d (dig texture 7) where it is
  naturally exercised — no extra weapon in 4b.)*
- **O8 (new):** keep the 4a fixture `physics_fall_test.lev`, or add a fixture whose
  surface guarantees a known background-above-dirt window for a stable, easy-to-read
  crater? *(Recommended: reuse `physics_fall_test.lev`; its sky-over-floor band
  already gives a clean surface — just aim the shot at it and assert `level` moves.)*

## Next artifact

The TDD plan: `plans/2026-06-28-liero-rs-step2-slice4b-plan.md`.
