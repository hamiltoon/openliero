# Simulation flow — walkthrough

A guided walkthrough of how OpenLiero's core fits together, plus a concrete
example that follows **one shot** all the way from trigger to destroyed
terrain. All file:line references are verified against the code (may drift with
future changes).

## Contents

1. [How the parts interact](#how-the-parts-interact)
2. [One shot, all the way](#one-shot-all-the-way)

---

## How the parts interact

Three layers. The arrows show who calls/reads whom. The golden rule: information
flows bottom-up for rendering — rendering and net **never** write
back into the simulation outside the deterministic path.

```
                       ┌──────────────────────────────────────┐
   keys/SDL ──────────►│  Gfx + *State  (gfx.cpp, *State.cpp)  │   UI / FLOW
                       │  MainMenu, GamePlay, WeaponMenu …      │
                       └───────────────┬──────────────────────┘
                                       │ owns a Controller, calls Process()/Draw()
                       ┌───────────────▼──────────────────────┐
                       │   Controller  (controller/)           │   "WHO DRIVES THE SIMULATION?"
                       │  Local / Replay / Rollback            │
                       └───────────────┬──────────────────────┘
                          input ↓      │   ↑ world state (read)
            ┌──────────────────────────▼───────────────┐  ┌────────────────────────┐
            │   Game::processFrame   (game.cpp)         │  │  Gfx rendering         │
            │   = THE TRUTH, deterministic              │─►│  viewport/blit/gfx     │──► screen
            │   Level · Worm · WObject · NObject ·       │  │  (reads, paints)       │
            │   bonus · Rand · StatsRecorder            │  └────────────────────────┘
            └───────────────┬──────────────────────────┘
                            │ snapshot (save/load)
            ┌───────────────▼──────────────────────────┐
            │  rollback/ + net/  (transport, session)   │   NETPLAY
            │  ring buffer, checksum, ENet/ICE          │
            └───────────────────────────────────────────┘
```

### The four couplings that carry everything

**1. State → Controller** (`gfx.cpp` ↔ `controller/`)
`Gfx` doesn't know *how* the game is driven. It holds a `Controller*` and only calls
`controller->Process()` (tick one step) and `controller->Draw()` (render). Swap
`LocalController` for `RollbackController` and the same menus/rendering work
unchanged. Hotseat, replay, and netplay are **the same game seen through different
controllers**.

**2. Controller → Game** (`controller/` → `game.cpp`)
The controller feeds in the players' button presses and calls `Game::processFrame()`.
That is the *only* way into the simulation. `Game` doesn't know which controller
drives it, doesn't know a screen exists.

**3. Game → Rendering** (`game.cpp` → `viewport/blit/gfx`)
Rendering **reads** the world state (worm positions, `level.material_id`,
projectile lists) and paints pixels. Never writes back. That's why
`SDL_VIDEODRIVER=dummy` works — cut off the rendering and the simulation notices nothing.

**4. Game ↔ Rollback/Net** (`game.cpp` ↔ `rollback/` + `net/`)
`RollbackController`:
- **reads** input, sends only button presses over `NetTransport` (ENet);
  `NetSession` is the state machine around it (`Idle→Handshaking→Playing`).
- **guesses** the opponent's input and runs `Game::processFrame` forward with
  `setSpeculative(true)` (mutes sound/stats).
- on a misguess: `loadSnapshotFast` rewinds to the last confirmed
  tick (ring buffer in `rollback/buffer.hpp`) and re-runs quickly.
- **`MarkDirty`** makes the terrain rollback-able — the snapshot only needs the
  changed cells.
- each tick a **checksum** is exchanged; if peers drift apart, desync is flagged.

### Why the split works

Determinism is the glue. Since `Game` is a pure function of
*(start state + input sequence)*:
- **Replay** = save the input sequence, play the same input → identical match.
- **Netplay** = send only input, both run the same `Game` → bit-exact equal.
- **Test** = run two `Game`s with the same input, compare checksums
  (`test_determinism`).

Stats is the special case: rollback runs the simulation speculatively (whose
`StatsRecorder` is a no-op), so the real numbers are taken from a
**shadow `Game`** that only follows confirmed frames — which is why
`Controller::statsGame()` exists.

---

## One shot, all the way

Everything below happens **inside one tick** (`Game::processFrame`), the fixed 71-Hz pulse.

### Step 1 — The trigger: `Worm::Fire` (`worm.cpp:1100`)

```cpp
--ww.ammo;                          // worm.cpp:1105  consume ammo
ww.delay_left = w.delay;            // cooldown

fixedvec const kFiring(cossin_table[Ftoi(aiming_angle)]   // worm.cpp:1110
                       * (w.detect_distance + 5) + pos - fixedvec(0, Itof(1)));
```

`kFiring` is where the bullet is born — the aiming angle is looked up in `cossin_table`
(precomputed sin/cos, no floats) and projected out in front of the worm. Then the projectile is born,
one per "part" (shotgun = several):

```cpp
for (int i = 0; i < kParts; ++i)                          // worm.cpp:1137
  w.Fire(game, Ftoi(aiming_angle), firing_vel, speed, kFiring, index, &ww);

vel -= cossin_table[...] * recoil / 100;                  // worm.cpp:1147  recoil on the worm
```

### Step 2 — The bullet is born: `Weapon::Fire` (`weapon.cpp:16`)

```cpp
WObject* obj = game.wobjects.NewObjectReuse();            // weapon.cpp:18  take from the pool
obj->type = this;  obj->pos = pos;
obj->vel = cossin_table[angle] * speed / 100 + vel;       // weapon.cpp:32  direction × speed

if (distribution) {                                       // weapon.cpp:34  spread
  obj->vel.x += game.rand(distribution * 2) - distribution;
  obj->vel.y += game.rand(distribution * 2) - distribution;
}
obj->time_left = time_to_explo;                           // weapon.cpp:71  countdown to self-detonation
```

- `NewObjectReuse()` — **no `new`**. Objects are reused from a pool.
- `game.rand(...)` — draws the shared, deterministic RNG. The order must
  be identical on both peers, otherwise desync.

### Step 3 — The bullet moves every tick: `WObject::Process` (`weapon.cpp:127`)

For an ordinary bullet the `do { … }` block runs once (the `while` condition on line 336
applies only to the laser):

```cpp
pos += vel;                                               // weapon.cpp:146  move
...
if (w.bounce > 0) { … vel.x = -vel.x * w.bounce / 100; }  // weapon.cpp:169  bounce
...
auto inew_pos = Ftoi(pos + vel);                          // weapon.cpp:234  where are we headed?
```

The collision decision (line 249) — the whole game in five lines:

```cpp
if (!game.level.Inside(inew_pos) ||                       // outside the level, OR
    game.PixelMat(inew_pos.x, inew_pos.y).DirtRock()) {   //   solid material ahead of us?
  if (w.bounce == 0) {
    if (w.expl_ground) do_explode = true;                 // weapon.cpp:252  → EXPLODE
    else               vel.Zero();                        //   otherwise: come to rest
  }
} else {
  vel.y += w.gravity;                                     // weapon.cpp:258  open air → fall
}
```

`PixelMat(x,y).DirtRock()` asks "is the pixel here solid?" — the level *is* a
material map. No rand here; the terrain is predetermined data.

Other exits in the same iteration:
- **Time-triggered:** `if (--time_left < 0) do_explode = true;` (`weapon.cpp:281`).
- **Worm hit:** `CheckForSpecWormHit(...)` → `game.DoDamage(...)` + blood spatter
  via `rand(128)` (`weapon.cpp:287–304`).

At the bottom: if `do_explode` → `BlowUpObject` and `break` (`weapon.cpp:328`).

### Step 4 — The explosion: `BlowUpObject` (`weapon.cpp:78`)

```cpp
game.wobjects.Free(this);                                 // weapon.cpp:87  back to the pool
if (w.create_on_exp >= 0) common.sobject_types[...].Create(...);  // light/smoke (sobject)
game.sound_player->Play(w.explo_sound);                  // weapon.cpp:94

for (int i = 0; i < kSplinters; ++i) {                   // weapon.cpp:100  splinters
  int const kAngle = game.rand(128);                     //   random angle
  common.nobject_types[...].Create2(game, kAngle, …);    //   → new nobjects
}

if (w.dirt_effect >= 0)                                   // weapon.cpp:117
  DrawDirtEffect(common, game.rand, game.level, w.dirt_effect, Ftoi(kX)-7, Ftoi(kY)-7);
```

The explosion is several spawns: sound, a visual `sobject`, a swarm of
`nobject` splinters (which run their own `Process`/can explode further), and — the key —
a change to the level itself.

### Step 5 — The terrain disappears: `DrawDirtEffect` (`gfx/blit.cpp:534`)

Despite the name ("draw"), this is **simulation**, not rendering — it writes to the
material map:

```cpp
PalIdx const* t_frame =
    common.large_sprites.SpritePtr(tex.s_frame + rand(tex.r_frame));  // blit.cpp:537  random crater variant

BLITL(level.material_id.data(), …, {                     // loop over the 16×16 area
  switch (c) {                                           // c = current material
    case 6:                                              // DirtRock (solid)
      if (rowmatdest->AnyDirt()) {
        *rowdest    = t_frame[((my&15)<<4) + (mx&15)];   // blit.cpp:559  new pixel
        *rowmatdest = common.materials[*rowdest];        //   new material (= hole/background)
        level.MarkDirty(rowdest - kBase);                // blit.cpp:562  ← catches rollback
      }
      break;
```

Three concepts become concrete at once:
1. **`rand(tex.r_frame)`** — the crater's appearance is randomized, must go through the shared
   RNG so both machines get exactly the same hole.
2. **`MarkDirty`** — every changed pixel is flagged. Exactly what `saveSnapshotFast`/
   the ring buffer needs to be able to rewind the terrain.
3. **The material map = the truth.** Rendering reads it later and paints pixels.

### The whole chain in one picture

```
TICK (game.cpp processFrame, 71 Hz)
  │
  ├─ Worm::Fire ........ ammo--, spawn pos via cossin_table, recoil      worm.cpp:1100
  │     └─ Weapon::Fire . NewObjectReuse, vel, [rand: spread]           weapon.cpp:16
  │
  └─ WObject::Process ... pos += vel                                   weapon.cpp:146
        ├─ PixelMat().DirtRock()?  ── no ──► vel.y += gravity          weapon.cpp:258
        │                          └─ yes ─► do_explode                weapon.cpp:249
        ├─ --time_left < 0 ───────────────► do_explode                 weapon.cpp:281
        └─ do_explode ─► BlowUpObject                                  weapon.cpp:78
              ├─ sound + sobject (smoke/light)
              ├─ nobject splinters       [rand: angle]                 weapon.cpp:101
              └─ DrawDirtEffect ─► write material_id + MarkDirty       blit.cpp:559
                                          [rand: crater variant]
```

Everything — `rand`, fixed-point, `MarkDirty` — sits on exactly the lines where determinism
and rollback are earned.
