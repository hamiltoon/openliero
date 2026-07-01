# Step 2, Slice 5′ — Per-pixel worm-hit + in-flight worm-hit arms + pickup (the open gate lands)

Status: **draft for review** · 2026-07-01
Part of: `2026-06-29-liero-rs-step2-slice5-object-families-overview.md`
Follows: `2026-07-01-liero-rs-step2-slice5d-death-respawn-design.md`
(the proven `sim` crate; `oracle_dump_sim_physics` with the `weapon`/`max_bonuses`/`worm`
directives; the object loops + `++cycles` + the bonus-drop roll live since 5b/5c; the
sobject `DoDamage` worm-damage arm + blood `nobject_types[6]` + `bobjects` pool live since
5b; the worm-loop **death/respawn** path live since 5d; `NObject::Create1`/`Create2` live
since 5a; the `bonuses` pool + `Bonus::Process` live since 5c.)

## Purpose

5′ is the **deferred follow-up slice** that lands the single largest gate deliberately held
open through slices 4c/4d/5a/5b/5d: the **per-pixel `CheckForSpecWormHit`** and the two
**in-flight worm-hit arms** (wobject + nobject) that hang off it, plus the 5c-deferred
**bonus pickup**. Until now a worm could be wounded **only** via the sobject-explosion AABB
(`sobject.cpp:54-55`, a coarse box, closed-gate weapon `explosives`). 5′ makes a **moving
projectile wound a worm on contact with its actual sprite silhouette**, exactly as C++:

1. **Per-pixel `CheckForSpecWormHit`** (`worm.cpp:1162-1188`) — the worm-sprite bank +
   the `Worm` material flag + the worm's `current_frame`/`direction`, replacing 5a's 16×16
   **box over-approximation** (`nobject.rs:250-265`). This is the predicate every in-flight
   arm gates on.
2. **wobject in-flight worm-hit arm** (`weapon.cpp:287-326`) — ready to re-apply from
   commit `fd33bbc` (ported + unit-tested in 5b T5b, then pulled because the box
   over-approximation over-fired; see below). Dart / fan / any open-gate projectile.
3. **nobject in-flight worm-hit arm** (`nobject.cpp:166-203`) — replaces the DEFERRED
   `debug_assert!(false)` body 5a left inside the loop skeleton (`nobject.rs:496-509`).
   Cannon splinters, any `hit_damage>0` nobject.
4. **Bonus pickup** (`worm.cpp:287-322`) — the health/weapon/booby branches, closing 5c's
   pickup deferral (the worm-loop bonus-box test + its RNG).

It is the first slice where a **flying projectile that grazes a worm** changes the master
hash, and the first where a **worm walking onto a bonus** does. Both are needed for
**open-gate weapons** (dart/cannon) and for the **slice-6 fuzz with moving worms that hit
each other**.

### Why the box over-approximation cannot stand (the `fd33bbc` / 5b-T5b blocker)

5b T5b ported the wobject arm and drove it with 5a's box `check_for_spec_worm_hit`. It
diverged **two ways** (ledger `progress.md:480`, commit `fd33bbc`):

- **Over-fires on a transparent corner.** At tick 49 the descending dart at pixel
  `(123,199)` grazes the **transparent** bottom-right corner (col 15, row 8) of worm1's
  16×16 sprite box at `(115,196)`. C++'s **per-pixel** test reads
  `materials[worm_sprite[8*16+15]].Worm()` → the corner pixel is background → **false**.
  The box returns **true** (any non-empty rect intersection). The Rust wounds a worm C++
  does not → `rng` diverges at tick 49.
- **Over-fires at the muzzle.** The box also opens for the **firing** worm near its own
  muzzle (the fan has `worm_collide=true`, `blow_away=30`, so its gate opens ~6px from
  worm0), which would regress the already-green 4a/4b/5a/5b goldens.

So the arm **requires** the exact per-pixel predicate — the worm sprite bank + the `Worm`
material flag + the worm's live `current_frame`/`direction`. That is 5′ T1–T2.

### What changes vs Slice 5d

| Invariant | 5d | **5′** |
|---|---|---|
| worm-hit predicate | 5a box over-approx (`nobject.rs:250-265`), inert | **per-pixel** (`worm.cpp:1162-1188`) — sprite bank + `Worm` flag |
| wobject in-flight arm (`weapon.cpp:287-326`) | DEFERRED (comment `weapon.rs:212`) | **PORTED** (re-apply `fd33bbc`, gated by per-pixel) |
| nobject in-flight arm (`nobject.cpp:166-203`) | DEFERRED (`debug_assert!(false)` body, `nobject.rs:503`) | **PORTED** (replaces the panic) |
| bonus pickup (`worm.cpp:287-322`) | DEFERRED (tripwire, 5c) | **PORTED** (health/weapon/booby) |
| worm `current_frame` / `animate` | render-only, **skipped** (`control.rs:469`) | **TRACKED** (feeds the sprite selection) |
| worm sprite bank + `Worm` material flag | absent from sim | **loaded / added** (T1) |
| how a worm is wounded | sobject-explosion AABB only (closed gate) | AABB **+ in-flight sprite contact** (open gate) |
| C++ dumper | no change (5d) | **NO CHANGE** — arms + pickup live inside the unmodified `Process` functions |
| prior goldens (1–5d) | byte-identical | **byte-identical (git diff empty)** — no dumper change |

## ✅ The dumper needs NO change — 5′ is a pure-Rust slice (like 5a and 5d)

The wobject/nobject in-flight arms and the pickup all live **inside** `WObject::Process`
(`weapon.cpp:287-326`), `NObject::Process` (`nobject.cpp:166-203`), and `Worm::Process`
(`worm.cpp:287-322`) — the three functions the dumper already drives **unmodified** every
tick (`sim_physics_dump.cpp:394-398` calls `w->Process(game)`; the object loops call the
object `Process`). No branch is gated behind anything the dumper omits:

- **`CheckForSpecWormHit`** reads `common.WormSprite(...)` + `common.materials[...]` — data
  the dumper's `Common` already fully loads (it loads the whole TC).
- **`settings->blood == 100`** (default) — the blood fans already matched in 5b.
- **Pickup** reads `settings->health`, `LC(Bonus*)` constants, `common.weapons` — all
  loaded.

Consequently **slices 1–5d re-diff byte-identical trivially** (nothing in the C++ path
moved). The 5a/5b/5c/5d *"git diff empty"* prior-slice gate holds with the weakest possible
justification. All 5′ work is on the Rust side.

## Scope

### IN — ported this slice

#### 1. Per-pixel `CheckForSpecWormHit` (`worm.cpp:1162-1188`)

```cpp
bool CheckForSpecWormHit(Game& game, int x, int y, int dist, Worm& w) {
  if (!w.visible) return false;                                    // :1165-1167
  PalIdx const* worm_sprite = common.WormSprite(w.current_frame, w.direction, 0);  // :1169
  int const kDeltaX = x - Ftoi(w.pos.x) + 7;                       // :1171
  int const kDeltaY = y - Ftoi(w.pos.y) + 5;                       // :1172
  Rect r(kDeltaX - dist, kDeltaY - dist, kDeltaX + dist + 1, kDeltaY + dist + 1);  // :1174
  r.Intersect(Rect(0, 0, 16, 16));                                 // :1176
  for (int cy = r.y1; cy < r.y2; ++cy)                             // :1178
    for (int cx = r.x1; cx < r.x2; ++cx)                           // :1179
      if (common.materials[worm_sprite[cy * 16 + cx]].Worm())      // :1181  ← the per-pixel test
        return true;
  return false;
}
```

**Difference vs 5a's box over-approx:** the box returns `true` on **any** non-empty rect
intersection (treats the whole 16×16 as solid). The per-pixel test additionally requires a
pixel inside the intersected sub-rect whose **palette-index maps to a `Worm`-flagged
material**. Transparent/background pixels of the silhouette do **not** count — this is what
kills the tick-49 false positive and the muzzle false positive.

**Data needed (and its current sim status):**
- **The worm-sprite bank** `common.WormSprite(f, dir, 0)` — a 16×16 `PalIdx` bank,
  `2*2*21 = 84` sprites, precomputed in `common.cpp:509-537` from `large_sprites[16+i]`
  (`i=0..20`) with a horizontal mirror for `dir=0` and a colour-swap (`pix∈[30,34] → +9`)
  for the `w=1` colour variant. **`CheckForSpecWormHit` always passes `w=0`** (`:1169`),
  so 5′ only needs the **colour-0** sub-bank (`dir=0` mirror + `dir=1` straight, 21 frames
  each = 42 sprites). Port the full bank for fidelity, but only colour 0 is load-bearing.
  **Not in the sim yet** (5a: "neither lives in the sim yet").
- **The `Worm` material flag** `kWormM = 1 << 5` (`material.hpp:12`, `Material::Worm()` at
  `:25`). The Rust sim has `material_flags: [u8;256]` with `MAT_BACKGROUND/DIRT/DIRT2/ROCK`
  consts (`state.rs:524`); **add `MAT_WORM = 1 << 5` (0x20)** and a per-pixel accessor.
  Note the sprite pixel is a **palette index used to index `material_flags` DIRECTLY**
  (C++ `common.materials[palIdx]`), **NOT** through `material_id[]` — that indirection is
  only for *level* pixels, not sprite pixels.
- **The worm's `current_frame` + `direction`.** `direction` exists (`state.rs:272`,
  added 5d, default 0). **`current_frame` does NOT** — it was render-only and never ported.
  `current_frame = AngleFrame() + kWormAnimTab[animate ? ((cycles & 31) >> 3) : 0]`
  (`worm.cpp:429-430`); `AngleFrame()` (`worm.cpp:454-473`) is a pure function of
  `aiming_angle` + `direction` → `0..6`; `kWormAnimTab = {0, 7, 0, 14}` (`settings.cpp:15`),
  so `current_frame ∈ 0..20` (21 frames). **`animate` is also currently
  render-only-SKIPPED** (`control.rs:469-470,560,595,659`) — 5′ must **promote it to a
  tracked field** and wire the `animate=true` walk sites (`worm.cpp:870,886`) +
  `animate=false` idle/fire sites (`worm.cpp:954,1073`), because it gates the anim offset.
  See §Datamodel + O-A below.

#### 2. wobject in-flight worm-hit arm (`weapon.cpp:287-326`) — re-apply `fd33bbc`

Per-worm loop **after** the flight/timeout, RNG order (VERIFY against `:287-326`):

- gate: `(hit_damage || blow_away || blood_on_hit || worm_collide) &&
  CheckForSpecWormHit(pos, detect_distance, worm)` (`:290-291`);
- `worm.vel += vel * blow_away / 100` (`:292`, **no rand**);
- `DoDamage(worm, hit_damage, owner_idx)` (`:294`, **no rand** in normal mode — the 5b
  `do_damage` port; sets `last_killed_by_idx` iff it drops `<=0`);
- `stats_recorder->DamageDealt/Hit` (`:295-298`, base no-op); `has_hit = true`;
- **blood fan** (`:301-306`): `kBloodAmount = blood_on_hit * settings_blood / 100`;
  `for i in 0..kBloodAmount { rand(128) angle; nobject_types[6].Create2(...) }`
  (`Create2` = `rand(speed_v)` + `rand(dist*2)×2` for blood ⇒ **4 draws/particle**);
- **hit-sound gate** (`:308-314`): `if hit_damage > 0 && worm.health > 0 && rand(3) == 0
  { rand(3) sound 18+…; if !IsPlaying Play }` — the **inner `rand(3)` is ALWAYS taken on
  the outer gate** (the `NOTE: MUST be outside the unpredictable branch` comment);
- **worm_collide** (`:316-324`): `if worm_collide { if rand(worm_collide) == 0 { if
  worm_explode do_explode; do_remove } }`.

**⚠ Order note:** wobject does **blood fan BEFORE the sound gate** (`:301` then `:308`).

#### 3. nobject in-flight worm-hit arm (`nobject.cpp:166-203`)

Guarded by `if (!do_explode && t.hit_damage > 0)` then the per-worm loop, RNG order:

- gate: `CheckForSpecWormHit(pos, detect_distance, w)` (`:171`);
- `w.vel += vel * blow_away / 100` (`:172`); `DoDamage(w, hit_damage, owner_idx)` (`:174`);
  `DamageDealt` (`:177`, no-op); `has_hit = true`;
- **hit-sound gate** (`:180-186`): `if hit_damage > 0 && w.health > 0 && rand(3) == 0 {
  rand(3) sound 18+…; if !IsPlaying Play }` — inner `rand(3)` always taken on the gate;
- **blood fan** (`:188-193`): `kBlood = blood_on_hit * settings_blood / 100`;
  `for i in 0..kBlood { rand(128); nobject_types[6].Create2(...) }`;
- **worm_explode / worm_destroy** (`:195-199`): `if worm_explode { do_explode } else if
  worm_destroy && used { nobjects.Free(this) }`.

**⚠ Order note:** nobject does **sound gate BEFORE the blood fan** (`:180` then `:188`) —
the **opposite order to wobject**. This asymmetry is load-bearing; a copy-paste that
mirrors the wobject order would desync. Both are exact ports of their respective C++
functions — the two C++ functions genuinely differ, and 5′ must reproduce each.

#### 4. Bonus pickup (`worm.cpp:287-322`) — closes 5c

Inside the `if (visible)` worm body (after the reaction-force block, `:285`), loop over
`bonuses`; the box test is an **11×11 AABB** (`ipos ± 5` vs `Ftoi(bonus)`), NOT per-pixel:

- gate: `ipos.x+5 > bx && ipos.x-5 < bx && ipos.y+5 > by && ipos.y-5 < by` (`:289-290`);
- **health bonus** (`i->frame == 1`, `:291-297`): iff `health < settings_health`:
  `bonuses.Free`; `DoHealing(worm, (rand(BonusHealthVar) + BonusMinHealth) * settings_health
  / 100)` — **1 `rand(BonusHealthVar)`** (`:295`), then `DoHealing` (rand-free direct clamp
  to `settings_health`). **If `health >= settings_health` the whole branch is skipped — no
  free, no rand.**
- **weapon bonus** (`i->frame == 0`, `:298-320`): **always** `rand(BonusExplodeRisk)`
  (`:299`);
  - `> 1` (the common case): iff `!h[HBonusReloadOnly]`: `fire_cone = 0`;
    `ww.type = &weapons[i->weapon]`; `ww.ammo = ww.type->ammo` (weapon swap — **hashed**
    via `type→id` + `ammo`); `Play(SoundReloaded)`; `bonuses.Free`; `ww.loading_left = 0`
    (no further rand);
  - `<= 1` (the **booby-trap** branch): `bonuses.Free`; `sobject_types[0].Create(bx, by,
    index)` — the booby explosion, its **own** draws (sound `rand(num_sounds)` +
    dirt-throw + crater, per the 5b sobject `Create` port). Because the booby sobject can
    then wound worms, this couples pickup to the sobject damage path (already live).

**RNG per pickup:** health → `rand(BonusHealthVar)` (or 0 draws if full); weapon →
`rand(BonusExplodeRisk)` (+ the booby sobject's draws on `<= 1`). Multiple bonuses are
iterated in pool order (each its own draws).

### OUT — deferred / kept-guarded

- **The recursive bonus chain-loop** (`sobject.cpp:217-227`) — still a tripwire (5c). The
  booby-trap pickup **spawns** `sobject_types[0]`, which *could* reach the chain-loop if it
  overlaps another bonus; keep the 5c tripwire and tune the pickup scenario so the booby
  explosion is not adjacent to a second bonus (thin path). Full chain-loop port stays
  slice-6.
- **`worm_collide` remove/explode as the FIRING worm's self-hit** — the C++ gate opens for
  every worm incl. the firer near the muzzle; per-pixel makes this faithful, but scenario
  positions keep the muzzle clear of the firer's own silhouette on the fire tick (the
  muzzle spawns at `detect_distance+5` ahead, `weapon.rs:166-169`) — verify it does not
  self-hit on tick 0 unless C++ does too.
- **`ProcessSight` laser worm-hit** (`worm.cpp:1150-1160` `CheckForWormHit`) and the
  **ninjarope** worm-hit (`ninjarope.cpp:19`) — both call `CheckForSpecWormHit` but their
  callers (`ProcessSight`/rope Process) are still outside the dumped subset; per-pixel makes
  them *available* but they stay unexercised. No new surface.
- **`Scales`/`GameOfTag`** pickup/damage mode branches — guarded, `KillEmAll` only.

## Datamodel

New / promoted fields (cross-check `stateHash.hpp` — only already-hashed fields move goldens):

- **`SimState.worm_sprites: SpriteSet`** — the colour-0 worm bank (16×16 ×42, or full ×84),
  built in `SimState::new` from `large_sprites` per `common.cpp:509-537`. **Not hashed**
  (a sprite bank, like `large_sprites`). Kept out of the `new` arg list or added as a
  derived build step (no `new` signature churn — mirror how `worm_sprites` is *precomputed*
  in C++ `Common` load, not passed).
- **`MAT_WORM: u8 = 1 << 5`** + `fn worm_pixel(pal_idx: u8) -> bool` (or inline
  `material_flags[pal] & MAT_WORM`) — the per-pixel accessor. **Not hashed.**
- **`WormState.current_frame: i32`** — computed at the END of the visible arm; read by
  `check_for_spec_worm_hit`. **Not hashed** (render field). **Load-bearing** because it
  selects the sprite → hit/no-hit → damage → master hash. Default matches a fresh worm.
- **`WormState.animate: bool`** — **promoted from render-only-skipped.** Set `true` at the
  walk sites, `false` at idle/fire. **Not hashed.** Load-bearing only through
  `current_frame`'s anim offset.
- **Consts:** `WORM_ANIM_TAB = [0, 7, 0, 14]` (`settings.cpp:15`); `BonusHealthVar`,
  `BonusMinHealth`, `BonusExplodeRisk` (`LC(...)` — thread from the TC like the 5c bonus
  consts, added AFTER `new`, no signature churn); `HBonusReloadOnly` hack flag.
- **Threading:** `wobject_process` must receive `&worms`, `&mut nobjects`, `nobject_types`,
  `settings_blood`, `cossin`, `worm_sprites`, `material_flags` (the arm's blood fan +
  per-pixel test) — this is the `fd33bbc` threading, re-applied. `nobject_process` already
  receives `worms` (the loop skeleton is there); add the blood/DoDamage plumbing. Pickup
  runs in the worm loop in `process_frame`, which already owns `bonuses` + `worms`.

## Hash-fold implications (`stateHash.hpp`)

| Field mutated by 5′ | Master | Component |
|---|---|---|
| worm `health` (DoDamage on hit; DoHealing on pickup) | yes (`:31`) | yes (`:149`) |
| worm `vel` (blow-away kick) | yes | yes |
| worm `lives`/`kills` (only if the in-flight hit KILLS → 5d death path) | yes | lives yes / kills master-only |
| worm weapon `type→id` + `ammo` (weapon-bonus pickup) | yes (`:40-48`) | **no** (component drops per-weapon) |
| `nobjects` (blood fan: `type 6`; `pos,vel,cur_frame,type→id`) | yes (`:86-92`) | pos.x/pos.y only (`:194-197`) |
| `bonuses` pool shrink (Free on pickup) | yes (`:65-69`) | yes (`:171-175`) |
| `sobjects` (booby `sobject_types[0]`) | id,cur_frame (`:76`) | id,cur_frame (`:184`) |
| worm `current_frame` / `animate` | **NO** | **NO** |
| `rng` (every fan/gate/pickup draw) | global | global |

**The load-bearing asymmetry:** `current_frame` and `animate` are **invisible to both
hashes**, exactly like `killed_timer` in 5d. Yet `current_frame` **decides whether a hit
lands** — so a wrong `current_frame` is undetectable *directly* but surfaces **transitively**
as a wrong `rng` burst (a hit that should/shouldn't have fired the blood fan) and a wrong
worm `health`. This is the same "hash-silent selector, RNG-witnessed effect" pattern as 5d's
timer. The difftest must therefore **not** rely on comparing `current_frame` (it is not in
the golden); it is pinned only through the hit's downstream `rng`+`health`+`nobjects`.

Global terms: `rng` (every draw), `cycles` (every tick, since 5b). `material_id[]` moves
only if a booby-trap sobject carves dirt (its `dirt_effect`).

## O-A — promoting `animate` from render-only (resolved here)

Slices 2/3 deliberately treated `animate` as render-only and **skipped** it
(`control.rs:469-470`), because nothing hashed or downstream-of-sim read it — `current_frame`
was never computed. 5′ makes `current_frame` load-bearing, so `animate` must be **correct**.
**Resolution:** add `animate: bool` to `WormState`; wire the four C++ sites
(`worm.cpp:870,886` → true on walk/move; `:954` idle → false; `:1073` fire → false); default
`false` (a fresh worm is idle). This is a small, RNG-free, unhashed change, but it touches
the walk/control code — its own T1 sub-task with a unit test asserting `current_frame`
matches C++ `AngleFrame() + kWormAnimTab[...]` for a moving vs idle worm. **Risk:** if any
`animate` site is missed, `current_frame` drifts by ±7/±14 on a moving worm and the
in-flight hit can mis-fire; the difftest's moving-worm fuzz (T10) is the guard. **Scenario
mitigation:** the *milestone* keeps the **victim idle** (`animate=false` ⇒ anim offset 0 ⇒
`current_frame = AngleFrame()` only), shrinking the T1 surface the milestone depends on; the
**fuzz** exercises a walking victim (the full `animate` path).

## Scenario + golden strategy

**No C++ dumper change** (as established). Reuse the 5b/5c/5d scenario machinery.

- **`sim_slice5prime_scenario.txt` (milestone — wobject in-flight arm):** resurrect the
  **discarded 5b dart** insight (ledger `:477`, `:482`). Open-gate weapon **`dart`**
  (`hit_damage=5`, `blood_on_hit=10`, `blow_away=28`, `detect_distance=0`,
  `worm_collide=false`) in worm0 slot 0; worm0 fires so the descending dart's pixel path
  crosses worm1's **solid silhouette** (not the transparent corner). worm1 **idle**
  (`animate=false`) at reduced-but-survivable health so the hit **wounds without killing**
  (keeps the death path out of the milestone — that is 5d's, already proven; a kill here
  would just re-exercise 5d). Expect: `rng` bursts on the **contact tick** (10 blood ×4
  draws + the `rand(3)` gate), worm1 `health` drops by `5`, worm1 `vel` kicked,
  `nobjects` gains 10 blood (type 6). **Window ~60–120 ticks.** The key non-vacuous
  witness: the contact tick fires the fan **only because the per-pixel test passed on a
  solid pixel** — and a second, near-miss variant (dart grazing the transparent corner,
  the tick-49 case) fires **nothing**, proving the per-pixel discrimination.
- **`sim_slice5prime_nobj_scenario.txt` (nobject in-flight arm):** **`cannon`** (splinters
  are `hit_damage=2` nobjects, `nobject.rs:493`) fired so a splinter's flight crosses
  worm1's silhouette. Expect the nobject-arm RNG order (sound gate **then** blood fan) —
  the mirror-image order to the dart, a deliberate second witness. May be folded into the
  fuzz if a single milestone golden is preferred.
- **`sim_slice5prime_pickup_scenario.txt` (pickup — the 5′b cut-point):** reuse 5c's
  bonus-drop (`max_bonuses ≥ 1`, seed tuned to drop early), then **walk worm0 onto the
  bonus's 11×11 box**. Two sub-variants: (a) **health bonus** (frame 1) with worm0 at
  `health < settings_health` → `rand(BonusHealthVar)` + heal (worm0 `health` jumps up,
  bonus pool shrinks); (b) **weapon bonus** (frame 0) → `rand(BonusExplodeRisk) > 1` reload
  (worm0's `ww.type`/`ammo` change, bonus freed). Force the frame via `HBonusOnlyHealth`/
  `HBonusOnlyWeapon` or seed. Keep the booby (`<= 1`) branch as a unit test (its sobject +
  potential chain-loop is the thin path) unless the controller wants it in-golden.
- **Goldens** `golden/sim_slice5prime*.txt` — N+1 rows × 11 columns (same schema as 5d).
  Inspect directly (4b/4c/5b/5c/5d discipline). **Plus** slices 1–5d re-run
  **byte-identical** (no dumper change).

## Difftest (the 5′ milestone) + fuzz

- **`sim_slice5prime_golden.rs` (MILESTONE):** mirror `sim_slice5d_golden.rs`; all 11
  columns; actual from a genuinely driven `SimState` (real `.lev`/`tc.cfg`/`Objects::load`,
  weapon **by name**, `id==index`, full `SimState::new`); components before master; input
  keyed `k-1`; all ticks. Non-vacuous guards from driven state:
  - worm1 `health` **drops by exactly the hit_damage** on the contact tick (a real hit);
  - `rng` **bursts on the contact tick** (the 10-blood fan + `rand(3)` gate) and is flat on
    the near-miss ticks (the per-pixel discrimination witness);
  - `nobjects` **> 0** on the contact tick (blood type 6) and drains;
  - worm1 `vel` **changes** on the contact tick (the blow-away kick);
  - the **near-miss variant** fires **nothing** (a dart grazing the transparent corner
    leaves `rng`/`health`/`nobjects` flat — this is the anti-false-positive proof that
    directly pins the fd33bbc blocker as FIXED).
- **`sim_slice5prime_pickup_golden.rs`:** worm0 `health` jumps up (health bonus) or `ww`
  changes (weapon bonus); `bonuses` pool shrinks by 1; `rng` draws the pickup roll on the
  pickup tick; both worms otherwise consistent.
- **Fuzz (`sim_slice5prime_fuzz.rs`) — moving worms hit each other (O21-style):** 3–4
  fixed-level multi-seed variants where **both worms move and fire**, so hits land at
  **varied `current_frame`/`direction`/positions** — this is the coverage the milestone
  (idle victim) cannot give, and the guard for the `animate`/`current_frame` port
  (O-A risk). Each variant: master + 9 components bit-exact all ticks. Requires the O3
  `new_object_reuse` overwrite (live since 5d) since aggressive fans can storm `nobjects`.
  This fuzz is the **direct precondition for slice-6 fuzz with moving worms**.

**Milestone gate:** master + all 9 component hashes bit-exact for every tick vs the C++
golden, **and** slices 1–5d re-run byte-identical (git diff empty). On failure,
`systematic-debugging` against the diverging column: a false-positive/negative hit localises
via `rng` + worm `health` + `nobjects` on the suspect tick; a `current_frame` drift shows as
a *mis-fired* fan (recall `current_frame` is hash-invisible — witnessed only transitively);
the wobject-vs-nobject blood/sound **order** shows as an `rng` transposition on the contact
tick.

## JOHN-BESLUT KRÄVS (genuine scope forks)

1. **Slice decomposition + pickup placement (the primary fork).** 5′ bundles two
   *independent* verticals: (A) the **per-pixel machinery + both in-flight worm-hit arms**
   (sprite bank, `Worm` flag, `current_frame`/`animate`, wobject+nobject arms — one tightly
   coupled unit, all sharing `CheckForSpecWormHit`); and (B) **bonus pickup** (a worm-loop
   11×11 AABB test + health/weapon/booby RNG — touches **no** sprite/silhouette code, closes
   5c). **Recommendation: DECOMPOSE into `5′a` (per-pixel + in-flight arms; T0–T7) and
   `5′b` (pickup; T8–T9), shipped back-to-back.** Rationale: (A) is the load-bearing
   open-gate/slice-6 prerequisite and carries the whole sprite-bank/`animate` risk surface;
   (B) is orthogonal and small. Splitting keeps each milestone a single clean witness and
   lets (A) land (unblocking slice-6 fuzz) even if (B) needs scenario iteration. The plan
   below is written as one numbered stream with the **`5′a | 5′b` cut marked at T8** so the
   controller can ship it either way. *This is the one genuine John-scope fork — everything
   else is controller-adjudicable.* (If John accepts the split, the controller proceeds with
   no further input.)

2. **(Sub-question, folded into #1 — recommendation only, not a hard fork):** the
   **nobject-arm** (cannon splinters) milestone — its own golden (T7) **vs** folded into the
   fuzz. Recommendation: give it a **small dedicated golden** (T7) so the wobject-vs-nobject
   blood/sound **order asymmetry** is pinned by a named milestone, not buried in fuzz.
   Controller-adjudicable; listed here only because it rides on #1's structure.

Everything else — the no-dumper-change finding, the per-pixel algorithm, the `animate`
promotion, the two arms' exact RNG order, the milestone scenario tuning, the near-miss
anti-false-positive witness, the pickup branch RNG — is decidable from the C++ oracle + the
recommendations above.

## Deferrals (tripwire / guard, not this slice)

- Recursive bonus chain-loop (`sobject.cpp:217-227`) — 5c tripwire retained; booby pickup
  kept non-adjacent to a second bonus.
- `ProcessSight` laser + ninjarope `CheckForSpecWormHit` callers — available but unexercised
  (callers outside the dumped subset).
- `Scales`/`GameOfTag` pickup/damage branches — guarded, `KillEmAll` only.
- The `w=1` colour-1 worm sub-bank (colour-swap) — port for fidelity but note only colour 0
  is read by `CheckForSpecWormHit`; if omitted, tripwire the `w != 0` path.

## Tasks

See the companion plan (`plans/2026-07-01-liero-rs-step2-slice5prime-plan.md`).
Sketch: **T0** dumper no-change verification + re-diff 1–5d byte-identical → **T1** data
foundation (worm sprite bank + `MAT_WORM` + `current_frame`/`animate`/`AngleFrame`/anim-tab;
priors byte-identical) → **T2** per-pixel `CheckForSpecWormHit` (replace box; near-miss unit
tests) → **T3** wobject in-flight arm (re-apply `fd33bbc`, per-pixel-gated) → **T4** nobject
in-flight arm (replace the panic; sound-before-blood order) → **T5** wobject scenario + gen +
golden → **T6** `sim_slice5prime_golden` difftest (MILESTONE 5′a) → **T7** nobject-arm
scenario + golden → **`— 5′a | 5′b cut —`** → **T8** pickup (`worm.cpp:287-322`) → **T9**
pickup scenario + gen + golden + difftest (MILESTONE 5′b) → **T10** moving-worms fuzz (O21;
slice-6 precondition) → **T11** done-check + ledger + PROGRESS.
</content>
</invoke>
