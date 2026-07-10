# Step 3 · Slice 3b — shadow + sprite pass: detailed design

Status: **draft for review** · 2026-07-10
Part of: `2026-07-10-liero-rs-step3-rendering-overview.md` (cited as **overview**)
Builds on: `2026-07-10-liero-rs-step3-slice3a-render-foundation-design.md` (**slice3a**, SHIPPED)
Sources: `2026-07-10-liero-rs-step3-cpp-render-pipeline-map.md` (**render-map §N**)

This is the executable design for the second Step 3 slice. 3a shipped the `render`
crate foundation and the first pixel-exact **terrain** frame (PR #4, CI green). 3b
completes the **world view**: it ports the two-pass world block of `Viewport::Draw`
(`viewport.cpp:274-591`) — all shadows, then all sprites — plus the blit primitives,
`ShadowQuery`, the worm-sprite index selection, the fire cone, the ninjarope, the
laser sight with its **viewport-local RNG**, the aim crosshair, and blood. It makes
`LightUp` (screen flash) and screen-shake go live. HUD/banners/minimap stay in 3e;
spectator and Modern stay deferred.

All `file:line` are at master (post PR #3/#4); `viewport.cpp` line numbers are the
current file (read in full during this design).

---

## Goal of 3b

Make the world view **pixel-exact vs C++** over motion, explosions, objects, worms,
and the laser-sight RNG trap — proven by the FNV-1a frame-hash golden differential-tested
tick-by-tick against the C++ dumper (overview *Oracle strategy*), over the reused
Step-2 fire/object scenarios.

3b **proves**:
1. **The two-pass ordering is load-bearing.** All shadows (`:274-398`) composite
   before any sprite (`:400-590`) — the render analog of the sim object-loop order.
   A single family reordered, or a shadow drawn after a sprite, moves the hash.
2. **The viewport-local RNG is live and reproduced exactly.** `DrawLaserSight`
   (`blit.cpp:693-703`) draws sparks off a *separate, default-seeded* `Rand`
   (`viewport.hpp:33`), advanced per Bresenham pixel; the shake branch
   (`viewport.cpp:49-52`) draws off the same RNG when `shake>0`. Seed, per-viewport
   instance, and call order must match or laser/shake frames diverge (overview *Risks*).
3. **Sprite selection is bit-exact.** The worm sprite index
   (`WormSpriteObj(current_frame, direction, index)`, `common.hpp:149`), the
   fire-cone stage/offset, the wobject `shot_type` 2/3 frame remap, and the bonus
   frame map all resolve to the same source art C++ picks.
4. **`ShadowedArgb`/`+4` and index-0 transparency** reproduce the darkened-terrain
   shadow and the sprite-hole semantics exactly.

---

## Exact scope — C++ code ported in 3b

Ported **verbatim in math**, idiomatic in API (overview locked-decision 3). Two
families of work: the **blit primitives** (`blit.cpp`) and the **world block**
(`viewport.cpp:272-591`) that drives them.

### Blit primitives + query (`src/game/gfx/`)

| C++ source | What | notes |
|---|---|---|
| `shadow_query.hpp:16-69` | `ShadowQuery` — `PixelAt`, `ShadowedIndex`, `ShadowedArgb`; Classic `+4` w/ 256-clamp keyed on `material_flags & kSeeShadow` | Classic arm only; `mode`/`cycles`/`display_valid` Modern branch kept in signature, unimplemented |
| `blit.cpp:239-262` | `BlitImage` — index-0-transparent sprite blit, writes `pal32[c]` for `c!=0` | worm/object/bonus/ninjarope/crosshair sprites |
| `blit.cpp:264-287` | `BlitImageTrans(…, phase)` — checkerboard `(x^y^phase)&1` | spawn-preview only; see *Reachability* |
| `blit.cpp:344-373` | `BlitImageR` — draws only where `shadow.PixelAt` ∈ `[160,168)` (water) | sobjects |
| `blit.cpp:375-407` | `BlitFireCone(fc, mem, x, y)` — 16×16, 4 stages (`fc` 0/1/2/default), threshold + `c-5`/`c-3`/`c-1`/`c` index offset | fire cone |
| `blit.cpp:433-460` | `BlitShadowImage` — where sprite `c!=0`, write `shadow.ShadowedArgb(x,y)` if non-zero | shadow pass (all sprite families) |
| `blit.cpp:644-677` | `DO_LINE` Bresenham macro (sign/`dx>dy` major-axis, `c=-(d>>1)` error) | shared by the 4 line drawers |
| `blit.cpp:679-691` | `DrawNinjarope` — color-cycling Bresenham `[NRColourBegin,NRColourEnd)` | rope line |
| `blit.cpp:693-703` | `DrawLaserSight(scr, Rand&, …)` — per pixel `rand(5)==0` → `pal32[rand(2)+83]` (**RNG**) | the trap |
| `blit.cpp:705-717` | `DrawShadowLine(ShadowQuery&, …)` — Bresenham of `ShadowedArgb` | rope shadow |
| `blit.cpp:719-729` | `DrawLine(…, color)` — Bresenham of `pal32[color]` | laser-weapon beam |

### World block (`viewport.cpp`)

| C++ block | What | notes |
|---|---|---|
| `:201-207` | `ShadowQuery kShadow{common, level, pal32, world_offset = -kOffs, mode, cycles}` — constructed once per viewport | screen+offset = world |
| `:274-398` | **Shadow pass** (gated `game.settings->shadow`), families in order: bonuses `:280`, sobjects `:292`, wobjects `:302-343`, nobjects `:346-366`, worms+ninjarope `:368-385`, bobjects `:387-397` | see *Draw order verbatim* |
| `:400-416` | Sprite pass — bonuses: `BlitImage :406`; names `DrawTextSmall :411` (→ 3e, see below) | |
| `:418-426` | Sprite pass — sobjects: `BlitImageR :423` | |
| `:428-481` | Sprite pass — wobjects: `shot_type` 2/3 frame remap `:435-454`; `BlitImage :457` **or** `SetPixel(cur_frame) :462`; weapon-34 names `:465-479` (→ 3e) | |
| `:483-498` | Sprite pass — nobjects: `BlitImage :489` **or** `SetPixel(cur_frame) :494` (Encloses) | |
| `:500-552` | Sprite pass — worms: laser sight `:516`; laser beam `:520`; ninjarope `:529` + rope head `BlitImage large[84] :531`; fire cone `:539`; **worm sprite `:545`** | AI debug `:550` OUT |
| `:566-583` | Sprite pass — aim crosshair `BlitImage small[make_sight_green?44:43] :572`; change-name `DrawTextSmall :580` (→ 3e) | gated `worm.visible` |
| `:585-590` | Sprite pass — bobjects (blood): `SetPixel(color) :588` (Encloses) | |

### Explicitly NOT in 3b

- **HUD bars / text / banners** (`viewport.cpp:78-189`, `:212-270`) → 3e. This includes
  the `DrawTextSmall` object/bonus/change **name labels** (`:411`, `:476`, `:580`),
  which need `Common::DrawTextSmall` + the 4px text-sprite bank (font work). The
  world-block *sprite* draws are in scope; the *name labels* co-located in those blocks
  are deferred with the rest of the font layer. Chosen 3b scenarios keep
  `names_on_bonuses=false` and issue no `kChange` press so these are provably unreached
  (a re-diff-style argument, not a silent omission — see *Reachability*).
- **Minimap** (`:593-635`) → 3e.
- **Holdazone dashed box** (`:229`), **death banners** (`:249-270`) → 3e (need font).
- **Spawn-preview** `BlitImageTrans` (`:243`) — see *Reachability* (out unless a scenario forces it).
- **AI debug** `DrawDebug` (`:550`) — diagnostic, permanently out (overview *Deferrals*).
- **Modern** `ShadowedArgb` display-halve arm (`shadow_query.hpp:62-66`) — deferred.

---

## Draw order verbatim (the heart of 3b)

The world block is **two passes, each iterating the object families in a fixed order**.
Reproduce this exactly; a single reordering diverges (render-map §3 steps 12–13). Screen
coordinate = world + `kOffs`, `kOffs = rect.Ul() - (vp.x, vp.y)` (`viewport.cpp:198`).

**Pass 1 — shadows** (only if `settings.shadow`; `viewport.cpp:274-398`):

1. **Bonuses** (`:276-284`) — for each bonus with `timer > BonusFlickerTime || (cycles&3)==0`:
   `BlitShadowImage(small_sprites[bonus_frames[frame]], Ftoi(x)-5-? , …, 7,7)` at
   `(Ftoi(x)-5+off.x, Ftoi(y)-1+off.y)`.
2. **SObjects** (`:287-298`) — `BlitShadowImage(large_sprites[cur_frame+start_frame],
   x-3+off.x, y+3+off.y, 16,16)` (note the `-3/+3` shadow offset).
3. **WObjects** (`:300-343`) — if `weapon.start_frame > -1`: apply the `shot_type` 2/3
   `cur_frame` remap (identical block to the sprite pass, `:307-326`), then **if
   `weapon.shadow`** `BlitShadowImage(small_sprites[start_frame+cur_frame],
   posX-3+off.x, posY+3+off.y, 7,7)`. Else (`start_frame==-1`) if `cur_frame>0`: a
   single `ShadowedArgb` pixel at `(posX+off.x-3, posY+off.y+3)` gated on
   `clip.Inside`.
4. **NObjects** (`:345-366`) — if `type.start_frame > 0`: `BlitShadowImage(
   small_sprites[start_frame+cur_frame], pos-3+off … +3, 7,7)`. Else if `cur_frame>1`:
   `ShadowedArgb` pixel at `(pos.x-3, pos.y+3)+off` gated on `clip.Encloses`.
5. **Worms + ninjarope** (`:368-385`) — for each visible worm, in `game.worms` order:
   if `ninjarope.out`: `DrawShadowLine(rope-3,+3 → worm+7-3,+4+3)` then
   `BlitShadowImage(large_sprites[84], ropeX-4, ropeY+2, 16,16)`; then
   `BlitShadowImage(WormSprite(current_frame, direction, index), tempX-3, tempY+3, 16,16)`.
6. **BObjects** (`:387-397`) — for each blood particle: `ShadowedArgb` pixel at
   `(pos.x-3, pos.y+3)+off` gated on `clip.Encloses`.

**Pass 2 — sprites** (always; `viewport.cpp:400-590`):

7. **Bonuses** (`:402-415`) — same flicker gate: `BlitImage(small_sprites[bonus_frames[frame]],
   Ftoi(x)-3+off.x, Ftoi(y)-3+off.y)`; *[name label → 3e]*.
8. **SObjects** (`:418-425`) — `BlitImageR(large_sprites[cur_frame+start_frame],
   x+off.x, y+off.y, 16,16)` (water-range gate).
9. **WObjects** (`:428-481`) — if `start_frame > -1`: `shot_type` 2/3 remap, then
   `BlitImage(small_sprites[start_frame+cur_frame], …)`. Else if `cur_frame>0`:
   `SetPixel((PalIdx)cur_frame)`. *[weapon-34 name label → 3e]*.
10. **NObjects** (`:483-498`) — if `type.start_frame > 0`: `BlitImage(small_sprites[
    start_frame+cur_frame], …)`. Else if `cur_frame>1`: `SetPixel((PalIdx)cur_frame)` gated Encloses.
11. **Worms** (`:500-552`, `game.worms` order): for each visible worm —
    a. if current weapon `Available()`: **laser sight** if `weapon.laser_sight`
       (`DrawLaserSight(bmp, vp.rand, hotspotX, hotspotY, tempX+7, tempY+4)`, `:516`);
       **laser beam** if `weapon_index == LaserWeapon-1 && Pressed(kFire)`
       (`DrawLine(…, color_bullets)`, `:520`).
    b. if `ninjarope.out`: `DrawNinjarope(ropeX, ropeY, tempX+7, tempY+4)` then
       `BlitImage(large_sprites[84], ropeX-1, ropeY-1)` (`:529-531`).
    c. if `weapon.fire_cone > 0 && worm.fire_cone > 0`: `BlitFireCone(fire_cone/2,
       FireConeSprite(AngleFrame, direction), fire_cone_offset[dir][af][0..1]+temp)` (`:539`).
    d. **worm sprite**: `BlitImage(WormSpriteObj(current_frame, direction, index), tempX, tempY)` (`:545`).
12. **Aim crosshair** (`:566-583`), gated on `worm.visible` (the viewport's *own* worm):
    `BlitImage(small_sprites[make_sight_green?44:43], cross)`; *[change-name → 3e]*.
13. **BObjects** (blood) (`:585-590`) — `SetPixel((PalIdx)color)` gated Encloses.

Then clip is restored (minimap block `:593-635` is 3e).

**Per-frame outer order** (`game.cpp:171-198`, already in `frame::draw`): palette build
(reset → RotateFrom → **LightUp if screen_flash>0** → pack) → `Fill(0)` → for each
viewport in order: `Process` → clip=rect → shadow pass → sprite pass → restore clip.
Both viewports share the *same* built palette; each carries its *own* `rand`.

---

## Viewport-RNG section (the trap)

The draw path is **not side-effect-free**. Reproduce the RNG exactly (overview *Risks*,
render-map §6).

- **The RNG is `Viewport::rand`** — a *separate*, **default-seeded** `Rand`
  (`viewport.hpp:33`, `rand.hpp:14-18`), one instance **per viewport**, **never**
  `game.rand`. `render/src/viewport.rs` already stores it as `sim_core::rng::Rand::new()`
  (default seed) and never advances it in 3a. 3b advances it.
- **`Rand::operator()(max) = (rand64*max)>>32`** (`rand.hpp:30-32`) — the Rust
  `Rand::bound` already matches (proven in Step 2).
- **Laser-sight call pattern** (`blit.cpp:698-701`): the `DO_LINE` Bresenham walks
  from `(hotspotX,hotspotY)` to `(tempX+7,tempY+4)`; **per stepped pixel** it draws
  `rand(5)` (1 draw, always); **iff that == 0** it draws a second `rand(2)` and writes
  `pal32[rand(2)+83]` at `(cx,cy)` if `clip.Inside`. So the per-pixel draw count is 1
  or 2, data-dependent — the exact Bresenham path length and the RNG sequence jointly
  determine the frame. Port `DO_LINE` bit-exact (major-axis pick, `c=-(d>>1)` error,
  `cx!=to_x`/`cy!=to_y` termination — note the loop steps *before* the body, so the
  start pixel is skipped and the end pixel is the terminator).
- **Per-viewport, per-worm order**: the worm loop (`:500`) iterates **all**
  `game.worms`; each visible worm with a laser-sight weapon advances **this viewport's**
  `rand`. So viewport 0's `rand` is advanced by worm 0's sight then worm 1's sight (if
  both visible+laser), then viewport 1 repeats the same worms against **its own**
  fresh-seeded `rand`. Two viewports never share RNG state; each starts from the default
  seed each *program run* (the `Rand` lives for the whole render session, seeded once at
  viewport construction, **not** re-seeded per frame — so its state accumulates across
  ticks, exactly like C++ where the `Viewport` object persists).
- **Shake branch** (`viewport.cpp:49-52`): `if (Ftoi(shake) > 0) { x += rand(shake*2) -
  shake; y += rand(shake*2) - shake; }` — **two** `rand` draws off the same viewport RNG,
  gated on `shake>0`. `Process` runs before the passes in `frame::draw`. See *Sim-side gap*
  for how `shake` is fed (it is NOT set by the reduced dumper today).
- **Determinism**: because both the C++ dumper and the Rust renderer construct their
  viewports fresh (default seed) and process viewport 0 then viewport 1 identically, the
  RNG streams line up. This is the whole proof of item 2.

---

## Dumper extension — terrain-only → full world draw

**Extend the existing opt-in `render_and_hash` lambda** in
`src/tools/oracle_dump/sim_physics_dump.cpp` (added in 3a, `:393-423`). Today it does
palette-build → `Fill(0)` → per-viewport `Process` + `DrawLevel`. 3b replaces the
`DrawLevel`-only body with the **full `Viewport::Draw` world block** minus HUD/minimap —
i.e. it calls the real C++ `Viewport::Draw` reduced to the world block, or inlines the
two passes. Cleanest: **call the real `Viewport::Draw`** but with a renderer/settings
configured so HUD/minimap are skipped. Because `Viewport::Draw` draws HUD/banners/minimap
unconditionally, inlining the `:196-591` world block into the lambda (as 3a already
inlined `:196-210`) is the surgical choice and keeps the sidecar matching the reduced
Rust `frame::draw`. **Decision: inline the world block** (`:272-591`, minus name labels
`:411/:476/:580`, minus holdazone/banners, minus minimap) into `render_and_hash`,
mirroring the Rust `frame::draw` 1:1.

Still gated on `scn.render_layout` non-empty → **absent ⇒ the 11-column sim line and all
existing `sim_slice*.txt` regenerate byte-identically** (the re-diff gate stands, exactly
as 3a). The sidecar (argv[4]) format is unchanged: `<tick> <frame_hex16> <state_hex8>` +
`total`.

### Shadow-pass gate — the settings->shadow tension (KEY DECISION)

The shadow pass is gated `if (game.settings->shadow)` (`viewport.cpp:274`), but the
dumper sets `settings->shadow = false` (O4) precisely because `settings->shadow` **also**
enables `CorrectShadow` in the **sim** `Process` loop, which mutates `material_id` and
would break the sim re-diff / isolation column. We cannot simply flip it true globally.

**Decision:** decouple the *draw-time* shadow gate from the *sim-time* `settings->shadow`.
The sim keeps `settings->shadow = false` (sim goldens byte-identical). The
`render_and_hash` lambda **temporarily sets `settings->shadow = true` for the duration of
the draw only** (set before the passes, restore immediately after), driven by a new
scenario directive **`render_shadow`** (default off). Since `render_and_hash` runs *after*
the tick's sim `Process` already completed, this flip cannot reach `CorrectShadow` for
that tick, and it is restored before the next `Process`. The Rust `frame::draw` takes a
matching `draw_shadow: bool` param. This is the render analog of the 3a opt-in discipline:
the sim path is untouched; only the draw reads the new flag.

> Alternative considered and rejected: a fully separate draw-only bool on the C++
> `Renderer`. The temporary-flip is smaller and provably sim-neutral (the flip window
> contains no sim mutation). Open question O2 asks the controller to ratify.

### 3a golden ripple — expected byte-identical (document, don't regen)

The 3a scenario has **invisible, frozen worms and empty pools**. Under the upgraded
full-world dumper: the shadow pass either stays off (`render_shadow` absent) or runs
against empty pools + invisible worms → paints nothing; the sprite pass iterates empty
pools, the worm block is `if(w.visible)`-gated → skipped, the aim crosshair is
`if(worm.visible)`-gated → skipped; blood pool empty. **⇒ the full-world draw produces
the identical pixels as the terrain-only draw**, so `render_slice3a.txt` regenerates
**byte-identical** — this is a done-when assertion (a mini re-diff gate for the render
sidecar), not a regen. If a stray pixel *does* move, that is a real bug in the pass
gating, caught immediately. (Prejudicated by the Step-2 O17/5b re-diff discipline; unlike
those, no regen is expected here.)

---

## Scenario corpus for 3b goldens

Reuse the Step-2 scenario **inputs** (the frame hash rides the same input vectors,
overview *Which scenarios*); each gets the `render player` directive added and a sidecar
golden generated. Selection targets one proof per object family plus the RNG trap.

| new render scenario | reuse base | proves | key bevis window |
|---|---|---|---|
| `render_slice3b_fan` | `sim_slice4a_scenario` | wobject sprite pass (floor-shot fan), worm sprites | wobjects non-empty tick ~17; worm sprite every visible tick |
| `render_slice3b_dart` | `sim_slice4c_scenario` | sobject explosion sprite (`BlitImageR`), nobject debris (`BlitImage`/`SetPixel`), worm sprites | sobjects 51-62, nobjects 51-90 (from 4c golden) |
| `render_slice3b_blood` | `sim_slice5b_scenario` | bobject blood `SetPixel(color)`, worm-hit sprites, dirt | bobjects non-empty window |
| `render_slice3b_shadow` | 4a/4c base **+ `render_shadow`** + worm/obj over dirt | the shadow pass + `ShadowedArgb`/`+4` | a tick where a visible worm or object overlaps a `SeeShadow` dirt cell |
| `render_slice3b_laser` (**new**) | new scenario | `DrawLaserSight` + viewport RNG; laser beam `DrawLine` | ticks where a visible worm holds a `laser_sight` weapon (sparks each tick) |
| `render_slice3b_shake` (**new**, if feasible) | new scenario w/ injected `shake` | shake RNG branch + `LightUp` screen flash | a tick with `shake>0` and `screen_flash>0` |

Concrete requirements:

- **Worm visibility.** Unlike 3a (invisible worms), 3b scenarios must have **visible**
  worms so the worm sprite / crosshair / laser blocks are reached. This makes the worm
  physics live — the worms move — which is fine: the frame hash tracks the moving sprite.
  Place worms on stable ground where useful, or accept motion (the C++ dumper and Rust
  renderer see the same sim, so any motion is reproduced identically).
- **Shadow proof needs `SeeShadow` terrain under a sprite.** `physics_fall_test.lev` is
  materials 130 (sky, Background) and 12 (dirt). Verify at implementation whether
  material 12 has `kSeeShadow` (`material.hpp:11`, `= 1<<4`) in the openliero TC; if not,
  author a tiny level whose dirt is `SeeShadow` and place a worm/object touching it.
  Without a `SeeShadow` cell under a non-zero sprite pixel, the shadow pass runs but
  paints nothing and is **unproven** (the exact analog of 3a's RotateFrom open question).
  This is a **done-when** for `render_slice3b_shadow`, not optional. → open question O3.
- **Laser scenario.** A worm with a `laser_sight` weapon (e.g. the TC's laser sight
  weapon; resolve by name via the existing `weapon <slot> <name>` directive) held by a
  **visible** worm produces sparks every tick. `make_sight_green` (worm.cpp:1202) and
  `hotspot_x/y` (worm.cpp:1207) are set by `ProcessWeapons` when the sight is active —
  see *Sim-side gap* (both are missing from `WormState`). The laser *beam* additionally
  needs `Pressed(kFire)` on the `LaserWeapon` slot; add an `input` line firing the laser.
  The bevis window is "hash changes each tick even with a stationary worm, because the
  viewport RNG advances" — i.e. the laser frames are *not* constant, which is itself the
  RNG proof (contrast 3a where a static scene gave a constant hash).
- **Shake / flash scenario.** `screen_flash` (LightUp) and `shake` are set by the sim
  on explosions, but the reduced dumper's viewports are **not registered with the game**
  (`game.viewports` is empty), so the sim never writes `vp->shake`, and `screen_flash`
  is a `Game`/`Renderer` field the reduced path zeroes. Proving these live needs either
  (a) wiring the dumper's viewports into `ProcessViewports` (large), or (b) a directive
  that injects a fixed `shake`/`screen_flash` at a given tick purely for the draw. →
  **Decision: (b)** — a `render_shake <tick> <amount>` / `render_flash <tick> <amount>`
  directive that sets `vp.shake` / `screen_flash` for that draw only (sim untouched,
  same discipline as `render_shadow`). If the controller prefers to defer shake/flash to
  a later slice rather than inject, that is open question O4.

---

## Sim-side gap — fields 3b needs that `SimState`/`WormState` may lack

3b is a **pure consumer**; every field it needs is either present, or must be added as a
**non-hashed render-only field** (prejudicated by `current_frame`/`animate` from Slice 5′,
`state.rs:353-362` — read by draw, omitted from `hash.rs`, so slices stay byte-identical).

**Present (verified in `rust/sim/src/state.rs`):**
- `WormState`: `pos`, `aiming_angle`, `direction`, `visible`, `health`, `killed_timer`,
  `fire_cone`, `make_sight_green`, `current_frame`, `animate`, `steerable_count`,
  `ninjarope{out,pos}`, `weapons`, `current_weapon`, `index` — all there.
- Pools: `bonuses{x,y,timer,weapon,frame}`, `wobjects{pos,cur_frame,ty}`,
  `sobjects{id,x,y,cur_frame}`, `nobjects{pos,cur_frame,ty}`, `bobjects{pos,color}` — all
  present (positions and `color` are the unhashed render fields already carried).
- Sprite banks: `small_sprites`, `large_sprites`, `worm_sprites` (pre-built via
  `build_worm_sprites`, `state.rs:834`), `textures` — present.
- `LevelSim.material_flags` (256-entry) — present (`state.rs:607`), so `ShadowQuery` reads
  `material_flags[material_id] & (1<<4)` with **no sim change**.
- Weapon/object type flags in `assets`: `Weapon{shadow, laser_sight, fire_cone,
  start_frame, shot_type, color_bullets}` (`object.rs:231-270`), `SObjectType{shadow,
  start_frame}`, `NObjectType{start_frame, color_bullets}`, `Constants{NRColourBegin/End,
  LaserWeapon}` (`tc.rs:82-87`), `ColorAnim{from,to}` — all present.

**Missing — must be added (all render-only, hash-neutral):**
1. **`hotspot_x` / `hotspot_y`** on `WormState` (C++ `worm.hpp:225`). Set by the sim in
   `worm.cpp:1085-1095` (Fire) and `:1207-1208` (laser-sight `ProcessWeapons`). Needed by
   `DrawLaserSight` (`:516`) and the laser beam (`:520`). **Currently absent** (grep: no
   `hotspot` in `state.rs`). Add as two `i32` render-only fields, computed in
   `process_weapons` (the laser-sight compute at `worm.cpp:1197-1210` and the Fire path
   at `:1085`). Keep them out of `hash.rs` → slices 1-5 goldens byte-identical.
2. **`fire_cone_sprites`** bank + **`fire_cone_offset`** table. `fire_cone_sprites` is
   built from `large_sprites` in `Common::precomputeSprites` (`common.cpp:539-551`);
   `fire_cone_offset[dir][af][0..1]` is the static table `common.cpp:17`. **Absent from
   sim** (grep: none). Needed only by `render_slice3b_fan`/similar *if* a fire-cone weapon
   is in flight. Add the bank (build alongside `worm_sprites`) + the static table (a
   `render`-crate const or an `assets` table). Render-only.
3. **`bonus_frames`** map (`common.bonus_frames[frame]`). **Absent from sim.** Needed only
   if a scenario spawns bonuses (`max_bonuses>0`). If no bonus render scenario is chosen,
   skip; else add the small map (render-only).
4. **`steerable_sum_x` / `steerable_sum_y`** on `WormState` (C++ `worm.hpp:267`). Used by
   the *centering* arm (`viewport.cpp:30-32`) when `steerable_count > 0`. `render/src/
   viewport.rs:86` currently `debug_assert!(steerable_count == 0)`. **Decision:** choose
   3b scenarios that keep `steerable_count == 0` (no steerable weapon — `shot_type` 2/3 —
   held while its worm is being centered), so the assert stays and the fields are not
   needed. If a chosen scenario needs a live steerable centered, add the two fields
   (render-only, accumulated where `steerable_count` already is). → open question O5.

**Hash-neutrality note.** None of the above is hashed. The `render_shadow`/`render_shake`/
`render_flash` directives touch only draw-time settings, restored before the next tick.
The isolation column (`state_hash`) in the sidecar therefore still equals the Step-2 sim
golden — run jointly as the standing isolation gate (overview *Isolation as a standing gate*).

---

## Module structure — `render` crate (additive)

3a's modules (`bitmap.rs`, `palette.rs`, `level_draw.rs`, `viewport.rs`, `frame.rs`,
`hash.rs`) stay; no breaking changes. Additions:

```
render/src/
  shadow_query.rs   // ShadowQuery { level: &LevelSim, pal32, world_off, mode, cycles }
                    //   pixel_at(), shadowed_index(), shadowed_argb()  (shadow_query.hpp)
  blit.rs           // blit_image, blit_image_r, blit_shadow_image, blit_fire_cone,
                    //   blit_image_trans; the DO_LINE macro as a shared iterator +
                    //   draw_line, draw_ninjarope, draw_laser_sight(rand), draw_shadow_line
  object_draw.rs    // the two-pass world block, split into shadow_pass() + sprite_pass():
                    //   per-family loops (bonuses/sobjects/wobjects/nobjects/worms/bobjects),
                    //   the shot_type 2/3 remap helper, worm/fire-cone sprite selection
  fire_cone.rs      // FIRE_CONE_OFFSET static table (common.cpp:17) + fire_cone_sprites build
```

Extend:
- `bitmap.rs`: add `get_pixel(x,y)` (read, for the `ShadowedArgb` pixel writes), a raw
  `put_argb(x,y,argb)` (the shadow pixel path writes ARGB directly, not via `pal32`), and
  `Rect::encloses` (the bobject/nobject pixel gate uses `Encloses(IVec2)` vs `Inside` —
  confirm the inclusive/half-open semantics against `math/rect.hpp` and port exactly).
- `viewport.rs`: activate the shake RNG branch (already coded, `:98-102`) — no change; it
  goes live once `shake>0` is fed. Add `hotspot`/`make_sight_green` reads via `WormState`.
- `frame.rs`: `draw(...)` gains `draw_shadow: bool` (and, if O4=(b), a way to see the
  injected `shake`/`screen_flash`); after `Process`+clip, call `shadow_pass` (if
  `draw_shadow`) then `sprite_pass` per viewport. `screen_flash` already threads into
  `build_palette` (`frame.rs:24`); wire a non-zero value through for the flash scenario.
- Keep `mode: ColorMode` on `ShadowQuery`/`blit`/`draw` (Classic arm only; Modern later).

---

## Test list

### Unit (in `render`)

- **`shadow_query`**: `pixel_at` returns `material_id` inside / `-1` outside;
  `shadowed_index` = `+4` when `SeeShadow`, clamps at 256 (index 252-255 → `+4` would hit
  256 → returns unshifted `kP`), `-1` when not `SeeShadow`; `shadowed_argb` = `pal32[kP+4]`
  or 0. Hand table over a synthetic `LevelSim` + `material_flags`.
- **`blit_image`** / **`blit_image_trans`**: index-0 transparency; `pitch != w` addressing;
  checkerboard `(x^y^phase)&1` on a hand sprite.
- **`blit_image_r`**: writes only where `PixelAt ∈ [160,168)`; a hand level with cells in
  and out of the water range.
- **`blit_shadow_image`**: writes `ShadowedArgb` only where sprite `c!=0` **and** the
  shadow is non-zero; a sprite hole over a `SeeShadow` cell.
- **`blit_fire_cone`**: the 4 stages — `fc=0` writes `c-5` for `c>116`; `fc=1` `c-3` for
  `c>114`; `fc=2` `c-1` for `c>112`; default writes `c` for `c!=0`.
- **Line drawers**: `DO_LINE` Bresenham on the 8 octants (major-axis pick, start skipped,
  end is terminator); `draw_ninjarope` color-cycles `[NRColourBegin,NRColourEnd)`;
  `draw_line` constant color; `draw_shadow_line` samples `ShadowedArgb`.
- **`draw_laser_sight`**: with a **seeded** `Rand` oracle, assert the exact draw count and
  sequence over a fixed span (per pixel: `rand(5)`, and only if 0 a `rand(2)`), and the
  written index `rand(2)+83`. This is the RNG-order pin.
- **Sprite selection**: `WormSpriteObj(current_frame, direction, index)` index math
  (`f + dir*7*3 + index*2*7*3`, `common.hpp:150`); `FireConeSprite(af, dir)` (`f+dir*7`);
  the `shot_type` 2/3 `cur_frame` remap (`:307-326`) on a hand table.
- **`object_draw`**: two-pass ordering — a synthetic scene with one shadow-casting object
  whose shadow would fall on another object's sprite cell; assert the sprite survives
  (shadow drawn first). Empty pools → no writes (the 3a-ripple guard, in-crate).

### Goldens (in `oracle-tests`)

- One `render_slice3b_<name>_golden.rs` per scenario: drive the Step-2 runner, render each
  tick with `frame::draw`, assert the sidecar **line-for-line + `total`**, and the joint
  `state_hash` column == the Step-2 sim golden (isolation).
- **`render_slice3a` byte-identical guard**: assert the upgraded full-world dumper still
  regenerates `render_slice3a.txt` unchanged (invisible-worms/empty-pools ⇒ world draw ==
  terrain draw). A render re-diff gate.
- **Sim re-diff gate**: every `sim_slice*.txt` regenerates byte-identical (the
  `render`/`render_shadow`/`render_shake` directives absent in sim scenarios; the temporary
  draw-time flips never reach a sim mutation).

---

## Edge cases

- **Index-0 transparency** — every `BlitImage*` skips source index 0; a fully-0 sprite
  writes nothing (a worm sprite's transparent halo must not overwrite terrain).
- **`+4` clamp at 256** — materials 252-255: `kP+4 >= 256` → `ShadowedIndex`/`ShadowedArgb`
  return the *unshifted* index (`shadow_query.hpp:45,67`), not a wrap. Test the boundary.
- **`BlitImageR` water range `[160,168)`** — half-open; index 160 draws, 168 does not.
- **Fire-cone stages** — `fire_cone/2` selects `fc`; the `c > threshold` compares are `>`
  not `>=`; the index offset subtracts (`c-5` etc.), which can underflow if a source pixel
  is below the threshold — but the threshold gate prevents it. Port the compares exactly.
- **`DO_LINE` direction/termination** — the loop advances *before* the body, so the
  **start** pixel is never drawn and the **end** pixel is the loop terminator (also not
  drawn on the `cx!=to_x`/`cy!=to_y` exit). Off-by-one here diverges every line primitive.
- **`Inside` vs `Encloses`** — wobject shadow pixel uses `clip.Inside` (`:338`); nobject/
  bobject pixels use `clip.Encloses(IVec2)` (`:358,391,493,587`). Confirm the two are not
  subtly different (half-open vs inclusive) in `math/rect.hpp` and port each call's exact
  predicate.
- **Double-shadow overlap** — two objects' shadows landing on the same cell: each
  `BlitShadowImage` writes `ShadowedArgb` (the *darkened terrain* ARGB, independent of what
  is already on screen — it reads the *level*, not the framebuffer, `shadow_query.hpp:8-15`),
  so overlapping shadows are **idempotent** (both write the same darkened value). No
  double-darkening — a key consequence of querying the level, not the screen. Test it.
- **Objects outside clip** — `CLIP_IMAGE` (blit.cpp macro) clips image blits to
  `clip_rect`; a sprite straddling the viewport edge draws only the inside rows/cols. The
  Rust blit must clip identically (clamp start/count, not skip the whole sprite).
- **Shadow offset asymmetry** — sobjects shadow at `(x-3, y+3)` but sprite at `(x, y)`
  (the `TODO` at `:294` notes the original didn't offset; the current code does). Port the
  *current* offsets verbatim.
- **`bonus_frames[frame]` + flicker gate** — `timer > BonusFlickerTime || (cycles&3)==0`
  controls both passes; a flickering bonus is present some ticks, absent others — the hash
  must reflect the gate.
- **Fade frame 0** — unchanged from 3a: tick 0 hashed `fade=0` (black), later `fade=33`.
  With sprites now drawn, frame 0 is *still* all-black regardless of content.

---

## Done-when

1. `render` crate gains `shadow_query.rs`, `blit.rs`, `object_draw.rs`, `fire_cone.rs`
   (+ `bitmap`/`viewport`/`frame` extensions); unit tests above pass; no breaking change
   to 3a modules.
2. `WormState` gains `hotspot_x/hotspot_y` (+ any needed of `steerable_sum_x/y`) as
   **non-hashed** fields, computed in `process_weapons`; `hash.rs` unchanged; **every
   `sim_slice*.txt` regenerates byte-identical** (sim re-diff gate) and `test_determinism`
   passes.
3. The dumper's `render_and_hash` renders the **full world block** (minus HUD/minimap/name-
   labels), behind the render directive; **`render_slice3a.txt` regenerates byte-identical**
   (render re-diff gate) and the sim re-diff gate holds.
4. Per-scenario sidecar goldens generated and matched **line-for-line + `total`** by the
   Rust `render_slice3b_*` tests, with the joint `state_hash` column equal to the Step-2
   sim golden (isolation), over scenarios that concretely exercise: wobject/sobject/nobject
   sprites, worm sprites, blood, the shadow pass on `SeeShadow` terrain, and the
   laser-sight viewport RNG (a per-tick-changing hash on a static worm = the RNG proof).
5. `LightUp` screen-flash and (if O4 resolves to inject) shake are exercised by at least
   one scenario and matched.
6. Committed on branch `liero-rs-step-3` (accumulating PR; no push/PR from the worker).
   `docs/superpowers/liero-rs-PROGRESS.md` bumped. Done-report produced.

Out of 3b (next: 3c Bevy window): the `game` crate, live native rendering, HUD/font/
minimap (3e), spectator, Modern.

---

## Open questions for the controller (max 5)

1. **Dumper: inline the world block vs call `Viewport::Draw`.** This spec inlines
   `viewport.cpp:272-591` (minus HUD/minimap/name-labels) into `render_and_hash`, mirroring
   the Rust `frame::draw`, rather than calling the real `Viewport::Draw` (which would drag
   in HUD/minimap and the font layer prematurely). Ratify inline?
2. **Shadow-pass gate decoupling.** Ratify the temporary-flip: sim keeps
   `settings->shadow=false`; `render_and_hash` sets it `true` for the draw window only,
   driven by a `render_shadow` directive (Rust `draw_shadow` param). Provably sim-neutral
   (no sim mutation in the flip window). Or prefer a separate draw-only `Renderer` bool?
3. **Shadow proof terrain.** Does the openliero TC's dirt material (index 12 in
   `physics_fall_test.lev`) carry `kSeeShadow`? If not, author a tiny `SeeShadow` test
   level for `render_slice3b_shadow` (analog of 3a's RotateFrom open question), or widen an
   existing level? The shadow pass is unproven without a `SeeShadow` cell under a sprite.
4. **Shake / screen-flash: inject vs defer.** The reduced dumper never sets `vp->shake` /
   `screen_flash` (its viewports aren't wired into `ProcessViewports`). Inject them via a
   draw-only `render_shake`/`render_flash` directive (recommended), or defer the shake/flash
   RNG+LightUp proof to a later slice and ship 3b with laser-sight as the sole
   viewport-RNG proof?
5. **Steerable centering.** Keep 3b scenarios `steerable_count == 0` (retain the
   `debug_assert`, add no fields), or does a target scenario need a live steerable weapon
   (`shot_type` 2/3) centered — requiring `steerable_sum_x/y` (render-only) now?
