# Step 4½, Slice 4½b — Random level generation (`GenerateRandom` + the shadow passes), bit-exact

Status: **draft for review** · 2026-09-10 · slice 4½b of Step 4½ (**parallel with 4½a**)
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (cited **overview**)
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` §4, §10 (cited **cpp-map**) and
`2026-09-10-liero-rs-step4.5-rust-baseline-map.md` cross-cutting §4 (cited **rust-map**)
Precedents: `2026-06-28-liero-rs-step2-slice4b-dirt-destruction-design.md` (the `DrawDirtEffect`
port this slice reuses, and the **O4** `CorrectShadow` omission it half-closes);
`2026-06-26-liero-rs-step1a-1b-io-and-material-map-design.md` (the level-dump oracle shape)
Next artifact: `plans/2026-09-10-liero-rs-step4.5-slice4.5b-plan.md`

## Purpose

The C++ shell never starts a match on a file level by default: `Gfx::InitFrameStepping` generates
one (`gfx.cpp:1444-1448`), NEW GAME regenerates one (`gfx.cpp:1519-1521`), and the level corpus is
five test fixtures (overview §Open Q6 recommends RANDOM as the normal path). 4½b ports the generator
— `Level::GenerateRandom` (`level.cpp:101-193`) with `GenerateDirtPattern` (`:11-83`), the
`MakeShadow` pass (`:195-216`) and the `GenerateFromSettings` dispatcher (`:397-429`) — into the
Bevy-free `sim` crate, and proves it bit-exact against a new C++ dumper, `oracle_dump_levelgen`.

It is a **pure function of (Rand state, TC assets, dimensions)**: integer-only, no I/O, no
`SimState`. Nothing in the running sim changes, so every existing golden stays byte-identical by
construction. Its RNG draw *count* is data-dependent (rejection loops, a retry cap, one draw inside
every `DrawDirtEffect`), which is why the gate pins the RNG position after every stage, not just
the final material map.

## Goal / done-when

**Done when (8):**

1. `sim::levelgen::generate_random(&assets, w, h, &mut rand)` reproduces C++
   `Level::GenerateRandom` driven by a `Rand` in the same state: `material_id` **and** `rand.last`
   match after each of the six stages (field, splats, stones, tunnels, formations, rocks) on every
   golden line, and the rock-loop statistics (`count/placed/tries`) match.
2. The golden matrix covers **3 seeds × 7 sizes × shadow {0,1} = 42 generation cases**, including
   sizes where the `kMaxTries = width*height` cap fires (`level.cpp:137-158`) and cases where a
   rock is rejected then placed (the common retry path). The dumper refuses to emit a matrix that
   misses either (§10.3).
3. `sim::levelgen::make_shadow` is bit-exact vs `Level::MakeShadow` on all 21 `shadow=1` cases,
   and non-vacuous (changes the map on every one).
4. The golden carries a **function-level C++ oracle for `CorrectShadow`** (`blit.cpp:624-639`): the
   **dig stage** (12 dig-texture stamps + `CorrectShadow` on the generated level, the
   `worm.cpp:931-934` shape). The Rust port of `CorrectShadow` is **4½a's** (`sim::shadow`, 4½a
   design §5.1); 4½b verifies its `shadow=0` dig tokens at the milestone and adds the `shadow=1`
   check (T9) once 4½a's `correct_shadow` is on the branch (§5, Open Q1).
5. `sim::levelgen::generate_from_settings` reproduces `Level::GenerateFromSettings` for the random
   path, the file path (incl. **`MakeShadow` applied to a loaded level**, `level.cpp:426-428`) and
   the missing-file fallback to random (`:406-418`).
6. New C++ target `oracle_dump_levelgen` + `gen_levelgen_golden.sh` + committed
   `golden/levelgen.txt` + `tests/levelgen_golden.rs`. The dumper's stage replica is
   **self-checked** against the real `GenerateDirtPattern`, `GenerateRandom` and
   `GenerateFromSettings` on every case (§10.2).
7. **Standing gates:** every prior golden byte-identical; `cargo test --workspace --exclude game`,
   `cargo test -p game` and `cargo build -p game --target wasm32-unknown-unknown` green;
   `SimState::new` signature unchanged; `sim-core` untouched.
8. `examples/levelgen_snapshot.rs` writes a BMP of a generated level for eyeballing.

**Not in 4½b** (each placed in §Deferrals with its owner): `SelectSpawn`, the `CorrectShadow`
wiring into the 7 in-match call sites, the NEW-GAME reuse-level rule, level-file I/O, the shell's
seed plumbing, the minimap preview.

---

## Inherited locked decisions (overview)

- **#3 Determinism firewall.** Generation runs *before* a `SimState` exists and returns an
  `assets::level::LevelData`, which `SimState::new` already consumes (`state.rs:1272-1301`, it copies
  `material_id` into `LevelSim`). The generator cannot reach a `SimState`.
- **#5 `SimState::new` unchanged.** Nothing here touches `state.rs` at all.
- **#6 Dedicated `Rand` seeded from the match seed** — made precise in §2.
- **#10 One accumulating PR** on `liero-rs-step-4-5`.
- **Parallel with 4½a, file-disjoint.** 4½b takes **plain parameters** (seed via a `Rand`, width,
  height, shadow, random-level flag), never a `MatchConfig`; 4½a/4½d map `MatchConfig` onto
  `LevelGenParams` later. 4½b's footprint: new `rust/sim/src/levelgen.rs`, one `pub mod` line in
  `rust/sim/src/lib.rs`, new `src/tools/oracle_dump/levelgen_dump.cpp` + two lines in the
  `CMakeLists.txt:372-392` block, new gen script / golden / test / example, `rust/README.md`,
  PROGRESS. It does **not** touch `rust/scenario/`, `rust/sim/src/state.rs`,
  `rust/sim/src/blit.rs`, `rust/sim/src/shadow.rs` (created by 4½a, design §5.1) or
  `src/tools/oracle_dump/sim_physics_dump.cpp` (4½a's surface and a known fmt footgun).

---

## Facts established for this slice (read out of the source; several adjust the overview/maps)

- **F1 — `SelectSpawn` is Holdazone-only.** Its sole caller is `Game::SpawnZone`
  (`game.cpp:494`), whose two callers are both Holdazone-gated: `StartGame` under
  `game_mode == kGmHoldazone` (`game.cpp:516-518`) and the Holdazone arm of `ProcessFrame`
  (`game.cpp:453-456`). Holdazone stays `unimplemented!()` (`state.rs:2250-2252`; overview
  §Deferrals). ⇒ **`SelectSpawn` is deferred with Holdazone** (§9), contrary to the overview's 4½b
  scope list and done-when #3.
- **F2 — Rust has no `CorrectShadow` at all.** Step 2 slice 4b's O4 omitted it by running every
  dumper with `settings->shadow = false` (`sim_physics_dump.cpp:381`); the Rust sim carries
  "`CorrectShadow` omitted (O4)" notes at all its dirt-effect sites (`control.rs:721`,
  `sobject.rs:394`, `nobject.rs:478`, `:652`, `weapon.rs:688`, `state.rs:2730`). The promised
  "dedicated shadow slice" never happened, and the overview does not mention `CorrectShadow`. With
  `shadow = true` (the C++ default, `settings.hpp:74`) C++ runs it after **every** in-match
  `DrawDirtEffect` (`weapon.cpp:122`, `nobject.cpp:124`, `:216`, `sobject.cpp:213`, `worm.cpp:785`,
  `:933`, `:943`) and it writes the hashed `material_id`. The parallel 4½a design reached the same
  finding and owns the port **and** the wiring (`sim/src/shadow.rs::correct_shadow` +
  `SimState.shadow` + the seven sites, 4½a design §5.1). ⇒ 4½b does not port it; it contributes
  the only **function-level** C++ oracle for it (the golden's dig stage, §10.4).
- **F3 — `MakeShadow` also runs on file levels.** `GenerateFromSettings` applies it after *either*
  branch (`level.cpp:426-428`). `scenario::load` never applies it (fine for the goldens, whose
  dumpers call `Level::load` directly), so the shell must go through `generate_from_settings`.
- **F4 — the C++ netplay path is level-identical, not stream-identical.** Netplay seeds `game.rand`
  from the session seed (`net/session.cpp:534`), the host generates **from the sim RNG**
  (`session.cpp:537` → `:696`) and ships the map **plus the post-generation RNG state** to the
  client (`session.cpp:700-704`); `RollbackController::Focus` likewise draws from `game.rand`
  (`rollbackController.cpp:379-382`). Single-player generates from the wall-clock `gfx.rand`
  (`gameEntry.cpp:23`, `gfx.cpp:1446`, `:1520`) and seeds `game.rand` from the wall clock too
  (`game.cpp:42`). Overview #6's "identical to the C++ netplay path" therefore holds for the *level*
  only (§2).
- **F5 — arbitrary sizes are already representable.** `LevelData` carries any `1..=4096` size
  (`assets/src/level.rs:48-51`, `:100`); `sim`, `render`, `scenario`, `game` and `shot` contain no
  504/350 constant (`grep -rnw "504\|350"` finds only two comments in `render/src/object_draw.rs`);
  the viewport clamp (`render/src/viewport.rs:80-107`) and minimap step
  (`render/src/hud.rs:268-269`) read `level.width/height` exactly like C++ (`viewport.cpp:23-57`).
  But **no sim golden runs on a non-504×350 level** (every `Levels/*.lev` fixture is a 176 413-byte
  OLLEVEL2 504×350), so in-match behaviour on generated sizes is ungated (Risk R6).
- **F6 — generation reads no stale flags.** C++ reads `materials[]`, a cache kept equal to
  `common.materials[material_id[i]]` by every writer (`SetPixel` `level.hpp:72-80`, `BlitStone`
  `blit.cpp:514-515`, `DrawDirtEffect` `:559-560`). After `Resize` the cache is zero-filled, but the
  field writes every cell before anything reads a flag (the field reads `Pixel` only). So Rust's
  live `material_flags[material_id[i]]` read (`state.rs:640-645`) is exactly equivalent — the same
  argument Step 2 4b made for `DrawDirtEffect`.
- **F7 — no unspecified-order hazards.** Every C++ expression in the generator contains at most one
  `rand()` call; draws are sequenced by statements. The `uint32_t` returned by `rand()` mixes with
  `int` in `rand(width) - 8`, `height - 1 - rand(20)`, `cx += rand(7) - 3`: all wrap modulo 2³² and
  convert back to `int`, which is exactly Rust `rand.bound(n) as i32 - 8` (`bound < n ≤ 4096`).
  Divisions (`/3`, `/2`, `>>1`) only see non-negative operands.
- **F8 — the generated palette is "none".** `GenerateRandom` resets `origpal` to `common.exepal`
  (`level.cpp:102`, `palette.hpp:88-91` is a plain copy) and clears `has_custom_palette` (`:103`).
  The Rust equivalent is `LevelData { palette: None, display: None }`. Today `scenario::load`
  ignores `LevelData.palette` altogether and always uses `small.tga`'s palette (`loader.rs:101-104`);
  the "a level's custom palette wins" rule (`Game::UpdateSettings`, `game.cpp:476-479`) is a
  builder concern for 4½a/4½d, noted in §Notes.

---

## Design

### 1. Crate/module home — `sim::levelgen`

Generation needs `sim_core::rng::Rand`, `sim::blit::draw_dirt_effect` (which mutates a
`sim::state::LevelSim`, `blit.rs:42-151`), the 16×16 `assets::sprite::SpriteSet`, the TC
`assets::tc::Texture` table and the 256-entry material flag table. Its output is the type
`SimState::new` takes, `assets::level::LevelData` (`state.rs:1273`).

- **`sim`** is the only crate that already has all of those, is Bevy-free and float-free, and
  compiles for wasm (it has no I/O). ✔
- `sim-core` must stay dependency-free and knows no assets. ✘
- `assets` has no `Rand` and must not depend on `sim`. ✘
- `scenario` depends on `render` (heavier graph for the oracle test), owns I/O, and is 4½a's
  parallel surface. ✘

**One new module, `rust/sim/src/levelgen.rs`:** the `GenerateRandom` family, `BlitStone` (only
generation calls it: `blit.cpp:462` has no caller other than `level.cpp:81,162-169,191`),
`MakeShadow` (only `GenerateFromSettings` calls it, `level.cpp:426-428`) and `GenerateFromSettings`.
`MakeShadow` lives here rather than in a `shadow` module because 4½a creates `sim/src/shadow.rs`
for `CorrectShadow` in parallel (4½a design §5.1) — one file per slice keeps them merge-free, and
`levelgen.rs` stays out of `state.rs`/`blit.rs` (4½a's surface; `blit.rs` has known rustfmt drift).
The `SeeShadow` bit (`material.hpp:11`, `1 << 4`) is a private const in `levelgen.rs`.

**Internal type:** the stages mutate a `LevelSim` (it carries `material_flags`, which every
predicate needs, and it is what `draw_dirt_effect` takes). The public hand-off is `LevelData` from
`generate_from_settings`, so file levels keep their `palette`/`display` through the same path.

### 2. Seeding — the RNG contract

- **The generator never constructs a `Rand`.** Every entry point takes `rand: &mut Rand`. That lets
  the oracle drive it from any seeded state, and keeps a future cross-play netplay path (Step 5)
  free to pass the *sim* RNG exactly as C++ netplay does (F4).
- **The shell** (4½a builder / 4½d NEW GAME) constructs a fresh one per generation:
  ```rust
  let mut level_rand = Rand::new();
  level_rand.seed(match_seed);          // == C++ Rand::Seed (rand.hpp:21-24): engine.seed(s); last = 0
  let level = generate_from_settings(&assets, &params, file, &mut level_rand);
  // SimState::new(&level, .., match_seed, ..) seeds the sim RNG independently (state.rs:1289-1290)
  ```
  `Rand::seed` is the existing API (`sim-core/src/rng.rs:64-74`, MT19937 `init_genrand`, identical
  to `std::mt19937::seed`); no `sim-core` change.
- **What the golden proves:** *the level for seed S equals C++ `Level::GenerateRandom` (and
  `GenerateFromSettings`) driven by a `Rand` seeded with S.* That is also exactly what C++ netplay
  produces for session seed S (F4).
- **What differs from C++ netplay:** the sim RNG is **not** advanced by generation — Rust re-seeds
  it from the same seed, C++ netplay continues from the post-generation state. The two streams
  start from the same MT state, so the sim's first draws equal the generator's first draws; that
  correlation is harmless (nothing compares them) but is recorded here so nobody "fixes" it by
  accident. **Unobservable until Step 5**, which runs Rust on both peers (both generate from the
  same seed, or the host ships the level as C++ does). **Step-5 note:** C++↔Rust cross-play would
  require passing the sim `Rand` instead — the `&mut Rand` signature already allows it.
- **Level seed = match seed** (recommended, Open Q3): simplest, and makes golden seeds and match
  seeds the same numbers.

### 3. The generator, stage by stage

`generate_random` = `new_level` → `generate_dirt_pattern` (= field + splats + stones) →
`dig_tunnels` → `place_rock_formations` → `place_rocks`. Each stage is a `pub fn` so the golden
test can hash between them.

| Stage | C++ | Rust (`sim::levelgen`) | RNG draws | Load-bearing detail |
|---|---|---|---|---|
| resize | `Resize` `:218-227`, called `:105` | `new_level(w, h, flags)` | 0 | zero-filled; every cell is overwritten by the field |
| field | `:12-26` | `generate_dirt_field` | exactly `w·h` | order: corner `rand(7)+12`; **column x=0 (y=1..h) FIRST**, then row y=0 (x=1..w), then interior row-major; `(prev + rand(7)+12) >> 1`, interior `(left + up + rand(8)+12) / 3` |
| splats | `:30-71` | `splat_large_sprites` | `1 + 3·count`, `count = rand(100)` | per splat `x = rand(w)-8`, `y = rand(h)-8`, sprite `rand(4)+69`; per non-zero texel: dest in **177..=179** ⇒ `(src+dest)/2`, else `src`; rows `my ≥ h` **break**, `my < 0` **continue** (same for columns) |
| stones | `:73-82` | `scatter_stones` | `1 + 3·count`, `count = rand(15)` | `x = rand(w)-8`, `y = rand(h)-8`, sprite `rand(4)+56`, `BlitStone(p1=false)` |
| tunnels | `:108-135` | `dig_tunnels` | `1 + Σ(5 + Σ(3 + count3))` | `count = rand(50)+5`; start `rand(w)-8, rand(h)-8`; `dx = rand(11)-5`, `dy = rand(5)-2`; `count2 = rand(12)` segments; each: `count3 = rand(5)` steps of `+= (dx,dy)` then `DrawDirtEffect(texture 1, cx, cy)` (**window top-left, no −7**); then backtrack `-= (count3+1)·(dx,dy)`; jitter `+= rand(7)-3, rand(15)-7`. **Every stamp draws its own `rand(r_frame)`, even fully clipped** (`blit.cpp:537` precedes the clip at `:545-547`) |
| formations | `:137-170` | `place_rock_formations` → `RockStats` | `1 + Σ(iters·{2,3} + placed)` | `kMaxTries = w·h`; `count = rand(15)+5`; candidate `cx = rand(w)-16`, `cy = rand(4)==0 ? h-1-rand(20) : rand(h)-16`; accept when `IsNoRock(32)`; `++tries` **only on rejection**; cap ⇒ `continue` **before** `rand(3)`; placed ⇒ `kind = rand(3)`, 2×2 `BlitStone` of `stone_tab[kind]` at `(cx,cy)`,`(cx+16,cy)`,`(cx,cy+16)`,`(cx+16,cy+16)` |
| rocks | `:172-192` | `place_rocks` → `RockStats` | `1 + Σ(iters·{2,3} + placed)` | `count = rand(25)+5`; `cx = rand(w)-8`, `cy = rand(5)==0 ? h-1-rand(13) : rand(h)-8`; `IsNoRock(15)`; same cap rule; placed ⇒ sprite `rand(6)+3` |

- **`IsNoRock(size, x, y)`** (`level.cpp:85-99`): rect `(x, y, x+size+1, y+size+1)` — a
  **(size+1)²** window — intersected with the level, false on the first `Rock()` cell. Clipped
  windows (negative `cx`) are normal. Ported verbatim as a private fn over `LevelSim::rock`
  (`state.rs:791-794`).
- **`stone_tab`** (`common.cpp:23`) = `{{98,60,61,62},{63,75,85,86},{89,90,97,96}}`, ported as
  `STONE_TAB`.
- **`RockStats { count, placed, tries }`** is diagnostic output with no C++ return value: `tries`
  counts candidate positions drawn (loop iterations). It exists so the golden can prove the cap path
  and the retry path were exercised and localise a divergence to the predicate vs the draw order.
- **`DrawDirtEffect` reuse.** The Step-2 port (`sim/src/blit.rs:42-151`) already draws
  `rand(r_frame)` first, clips to `Rect(0,0,w,h-1)`, walks `BLITL` and wraps the fill on level
  coordinates. Generation calls it with `dirt_effect = 1` — tc.cfg texture 1 is
  `mframe=1, rframe=2, sframe=73, ndrawback=true` (`tc.cfg:141-146`), i.e. the **carving** branch
  (Dirt→1, Dirt2→2, AnyDirt→fill), which Step-2 4c/4d already exercised live. No change to it.
- **Integer semantics** per F7; the only wrapping arithmetic is `PalIdx` `+4` in the shadow passes.

### 4. `BlitStone` (`blit.cpp:462-532`)

All generation calls pass `p1 = false` (`level.cpp:81`, `:162-169`, `:191`; there is no other
caller), so only that branch is ported: `CLIP_IMAGE` against **`Rect(0, 0, w, h)` — full height**
(unlike `DrawDirtEffect`'s `h-1`), then every non-zero texel overwrites the destination (no
`DirtBack` test in the `p1=false` branch, `:505-531`). The `p1=true` branch has no caller; it is
not ported and the doc comment says so.

### 5. The two shadow passes — `MakeShadow` and `CorrectShadow` (reconciling O4)

Both are gated on the **global** `settings.shadow` and both write the hashed `material_id`. They
share predicates (`SeeShadow` bit `1<<4`; the neighbour at **`(x+3, y−3)`**), but differ in shape:

| | `MakeShadow` (`level.cpp:195-216`) | `CorrectShadow` (`blit.cpp:624-639`) |
|---|---|---|
| when | once, after the level is generated **or loaded** (`:426-428`) | after every in-match `DrawDirtEffect` (7 call sites, F2) |
| area | whole level: `x ∈ [0, w-3)`, `y ∈ [3, h)`, **x outer / y inner**; then the bottom row | caller rect ∩ `Rect(0, 3, w-3, h)`, x outer / y inner |
| rule 1 | `SeeShadow(x,y) && DirtRock(x+3,y-3)` ⇒ `+4` (wrapping `PalIdx`) | same ⇒ `+4` |
| rule 2 | then, **re-reading the possibly updated pixel**: `12..=18 && Rock(x+3,y-3)` ⇒ `−2`, floored at 12 | **else** `164..=167 && !DirtRock(x+3,y-3)` ⇒ `−4` (un-shadow) |
| rule 3 | bottom row: `Background` ⇒ `13` (all x) | — |

- **Read-order subtlety (both):** the neighbour `(x+3, y−3)` lies in a column the x-outer loop has
  **not visited yet**, so it is always read pre-pass. A port that loops y-outer would read
  already-shadowed neighbours and diverge. `MakeShadow`'s rule 2 re-reads `(x,y)` *after* rule 1
  may have written it (a `SeeShadow` pixel can be `+4`'d into `12..=18` and then `−2`'d).
- **Port shape (4½b):** `sim::levelgen::make_shadow(&mut LevelSim)`, writes through
  `LevelSim::set_material` (material only; the C++ `display_valid` clear is render-only, §6).
- **`CorrectShadow` (4½a):** 4½a ports `sim::shadow::correct_shadow(level, x1, y1, x2, y2)` and
  wires it behind a post-`new` `SimState.shadow` (default `false`, every golden byte-identical),
  gated by its own shadow-on sim goldens (4½a design §5.1, plan T3). 4½b's golden adds a
  **function-level** oracle (§10.4) that isolates the pixel rule from the sim: T7 checks the
  `shadow=0` dig tokens (pure `DrawDirtEffect`), and **T9** — sequenced after 4½a's T3 lands — checks
  the `shadow=1` dig tokens through 4½a's function. The same table above is what T9 would diagnose
  against.

### 6. `generate_from_settings` — the `GenerateFromSettings` equivalent

```rust
pub struct LevelGenParams { pub random_level: bool, pub random_map_width: i32,
                            pub random_map_height: i32, pub shadow: bool }   // C++ Settings field names
pub fn generate_from_settings(assets: &LevelGenAssets, params: &LevelGenParams,
                              file: Option<LevelData>, rand: &mut Rand) -> LevelData
pub fn level_file_name(level_file: &str) -> String       // ".LEV" appended when no '.', level.cpp:401-404
```

- `random_level` ⇒ `generate_random(params.random_map_width, params.random_map_height)`, palette
  `None`. Otherwise `file` is **the caller's already-attempted load** of
  `level_file_name(settings.level_file)`: `Some(level)` ⇒ use it (palette/display preserved, **no
  RNG**); `None` (read or parse failure) ⇒ fall back to `generate_random` — the `try/catch` +
  `!loaded` of `level.cpp:405-418`. Then `params.shadow` ⇒ `make_shadow` on either result.
- **Why the I/O stays outside:** `sim` has no filesystem and must build for wasm; the native read,
  the wasm embedded manifest (`scenario/src/assets.rs:48-103`, which *panics* on a miss today) and
  the merged user/data read view are 4½a/4½d/4½h concerns. The `Option<LevelData>` seam lets them
  plug in without touching the generator. `LevelGenParams::default()` mirrors the C++ `Settings`
  defaults (`random_level=true`, `504×350`, `shadow=true`; `settings.hpp:74,80,89-90`).
- **Not ported here:** the `old_random_level/old_level_file/old_random_map_*` provenance
  (`level.cpp:421-424`) — it only feeds the NEW-GAME reuse rule (`gfx.cpp:1512-1518`), so 4½d keeps
  the params next to the level instead. **MODERNLV `display_valid`:** C++ `SetPixel` clears
  `display_valid` for every pixel `MakeShadow` writes on a modern level (`level.hpp:76-78`); Rust
  never renders display layers (no reader in `render`/`game`/`shot`), so the clear is deferred to
  whichever step first renders them (§Deferrals).

### 7. Arbitrary map sizes — supported, gated in the generator

Per F5 no Rust-side work is needed to *represent* or *render* non-default sizes, so 4½b does **not**
restrict itself to 504×350. The generator precondition is `1 ≤ w, h ≤ 4096` (debug-asserted; the
menu range is 64..4096 step 8, `gfx.cpp:1295-1297`, but a hand-edited TOML can hold any value).
The matrix covers the default, the C++ test's 600×350 (`test_random_map_size.cpp:97`), the menu
minimum 64×64, the "overflows with rocks" 128×96 (`:95-96` comment — pre-cap), an odd 101×77, a wide
2000×72 and a tall 72×1000. **4096×4096 is left out** of the golden for Rust debug-test runtime
(≈16.7 M field draws per line); the width-dependence it would exercise is covered by 2000×72
(Open Q4). The in-match consequence of non-default sizes is ungated (R6).

### 8. Palette

`LevelData.palette = None` for a generated level means "the TC's exe palette"; `display = None`.
The generator does no palette work (F8).

### 9. `SelectSpawn` — deferred with Holdazone

Per F1 it is unreachable until Holdazone lands. Porting it now would add ~50 lines and an oracle
line with no consumer; it moves as a unit with `SpawnZone`/`holdazone` state when Holdazone is
un-deferred. Its oracle is cheap to add then: the same dumper can call `level.SelectSpawn(rand, w,
h, pos)` on a generated level (`level.cpp:435-487`, reservoir sampling with `rand(i)`).

### 10. Oracle and gates

#### 10.1 Target

`src/tools/oracle_dump/levelgen_dump.cpp` → `oracle_dump_levelgen`, two lines in the
`OPENLIERO_BUILD_ORACLE_DUMP` block (`CMakeLists.txt:372-392`), linking `game`. Runs from the repo
root (`Common::load(FsNode("data")/"TC"/"openliero")`, as `sim_physics_dump.cpp:365-367`). One
argument: the output path.

#### 10.2 Replica + self-checks (why stage hashes are trustworthy)

C++ exposes only `GenerateDirtPattern`, `GenerateRandom`, `MakeShadow` and `GenerateFromSettings`
as members; the tunnel/rock loops and `IsNoRock` are inside `GenerateRandom` / file-static. To emit
**per-stage** hashes the dumper runs a **stage replica** of `GenerateRandom` built only from the
real primitives (`Level::SetPixel/Pixel/Mat`, `DrawDirtEffect`, `BlitStone`, `CorrectShadow`,
`stone_tab`) plus a verbatim copy of `IsNoRock`, and then **self-checks** it on every case:

1. replica after the stones stage == real `Level::GenerateDirtPattern` (fresh level + identically
   seeded `Rand`): `material_id` hash **and** `rand.last`;
2. replica after the rocks stage == real `Level::GenerateRandom`;
3. replica (+ `MakeShadow` when shadow) == real `Level::GenerateFromSettings` with
   `random_level = true`.

Any mismatch ⇒ `exit(1)` naming the check. The replica is therefore never the oracle; it only
slices the oracle's work, and the golden cannot be written from a replica bug.

#### 10.3 Matrix and coverage guard

Seeds `{1, 42, 2654435769}` × sizes `{504×350, 600×350, 64×64, 128×96, 101×77, 2000×72, 72×1000}`
× shadow `{0, 1}` = 42 `gen` lines; plus 5 `file` lines: `Levels/see_shadow_test.lev` seed 1
shadow 1 and 0, `Levels/render_stage.lev` seed 1 shadow 1, and the missing
`Levels/does_not_exist` (no `.` ⇒ `.LEV` appended ⇒ load fails ⇒ random fallback at the
504×350 defaults) seed 42 shadow 1 and 0. The dumper **exits 1** unless at least one `gen` case hit
the cap (`placed < count`) **and** at least one uncapped case retried (`tries > count`), so the
committed golden is guaranteed to exercise both loop exits. If a guard fails, the remedy is more
small-map seeds (e.g. add seeds for 64×64), recorded in the golden's commit message.

#### 10.4 Dig stage (the `CorrectShadow` oracle, and a second RNG pin)

After generation (and `MakeShadow` when shadow), the dumper applies 12 stamps continuing from the
post-generation `Rand`: for `i in 0..12`, `x = (i·41+5) mod w − 7`, `y = (i·29+7) mod h − 7`,
`DrawDirtEffect(texture 7, x, y)` then — only when shadow — `CorrectShadow(Rect(x−3, y−3, x+18,
y+18))`: exactly the worm dig shape (`worm.cpp:931-934`). The `−7` and the modulo put early stamps
across the top/left edges (clip paths). It is a function-level oracle for 4½a's `correct_shadow`
(T9), re-proves the carving `DrawDirtEffect` on generated terrain, and its `rand.last` (12 draws
later) pins the post-generation RNG position a second time.

#### 10.5 Golden format — `rust/oracle-tests/golden/levelgen.txt`

```
gen <seed> <w> <h> <shadow> <field> <splats> <stones> <tunnels> <formations> <rocks> <shadowed> <dig> <form_count> <form_placed> <form_tries> <rock_count> <rock_placed> <rock_tries>
file <level_file> <seed> <shadow> <w> <h> <final>
```

- A **stage token** is `<fnv1a64(material_id) as %016llx>:<rand.last as %08x>` — hash and RNG
  position after that stage. `<rocks>` is the final pre-shadow `GenerateRandom` result.
- `<shadowed>` is the bare `%016llx` hash after `MakeShadow` (no RNG), or `-` when `shadow = 0`.
- `<dig>` is a stage token after the dig stage.
- counts are decimal; `tries` is `%llu`.
- `file` lines: `<level_file>` is the string passed as `settings.level_file` (repo-root
  relative); `<final>` is a stage token after `GenerateFromSettings` (`rand.last == 00000000` proves
  the file branch drew nothing; non-zero proves the fallback ran).

FNV-1a 64 is the level golden's hash (`level_dump.cpp:21-28`, `level_golden.rs:9-16`).
`rand.last` rather than a draw counter because C++ `Rand` has none (`rand.hpp:13-26`); a 32-bit
`last` after every stage, plus the dig-stage `last` 12 draws later, pins the stream position beyond
any practical collision. Rust additionally has `Rand::draws()` (`rng.rs:137-139`) for unit tests.

#### 10.6 Gen script

`rust/oracle-tests/gen_levelgen_golden.sh`: the exact `macos-arm64` incantation of
`gen_sim_slice4b_golden.sh` / `gen_level_golden.sh` (`PRESET="${PRESET:-macos-arm64}"`,
`cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON`, `cmake --build "$ROOT/build/$PRESET"
--config Release --target oracle_dump_levelgen`, run `build/$PRESET/Release/oracle_dump_levelgen`
from `$ROOT`), except that it `cd "$ROOT"` *before* `cmake --preset` so it works from any cwd.
LOCAL/MANUAL, not run in `rust.yml` CI — the committed golden is.

#### 10.7 Rust golden test — `rust/oracle-tests/tests/levelgen_golden.rs`

Iterates the golden lines (the dumper matrix is the single source of truth). For each `gen` line:
seed a `Rand`, run the six `pub` stages on `new_level`, assert each stage token (the failure message
names the **first diverging stage**), assert both `RockStats`, assert the composed
`generate_random` and `generate_from_settings` agree with the staged result, apply `make_shadow`
(when shadow), assert `<shadowed>`, and on `shadow=0` lines run the dig stage (pure
`draw_dirt_effect`) and assert `<dig>`. For each `file` line: `level_file_name` → `std::fs::read`
(+ `assets::level::load`) → `Option` → `generate_from_settings` → assert. Coverage guards: all 14
(size, shadow) combinations present, 5 `file` lines, ≥1 cap case, ≥1 retry case, `MakeShadow`
changed every shadow line, ≥1 file line with `last == 0` and ≥1 with `last != 0`.

**T9 (after 4½a's `sim::shadow::correct_shadow` lands)** adds a second test in the same file: on
every `shadow=1` line, regenerate + `make_shadow`, run the dig stage with `correct_shadow` and assert
`<dig>`; non-vacuity = on ≥1 line the same dig *without* `correct_shadow` hashes differently.

#### 10.8 Standing gates

4½b edits no existing dumper and no existing Rust code path (only adds modules), so every prior
golden is untouched by construction; the re-diff is `git diff --stat` over
`rust/oracle-tests/golden/` showing only the new `levelgen.txt`, plus the three CI commands.

### 11. Eyeball tool (optional, kept tiny)

`rust/oracle-tests/examples/levelgen_snapshot.rs`: `cargo run -p oracle-tests --example
levelgen_snapshot -- <seed> [<w> <h>] [noshadow]` writes
`rust/target/snapshots/levelgen_<seed>_<w>x<h>[_noshadow].bmp` — the material map 1:1 through
`small.tga`'s palette (= C++ `common.exepal`), reusing `render_snapshot.rs`'s dependency-free BMP
writer. No `shot` flag (a `shot` scenario needs worms/viewports; a raw level dump is simpler), no
C++-side image (the golden is the correctness proof; the BMP only answers "does it look like a
Liero level").

---

## Task outline (the plan details each)

- **T0** — C++ `oracle_dump_levelgen` (replica + self-checks + coverage guard) + CMake + gen
  script + committed golden. **Runs in an isolated worktree in parallel with T1–T6** (disjoint
  files), cherry-picked before T7 — the project's proven pattern.
- **T1** — `levelgen.rs` scaffold: `new_level`, `generate_dirt_field`.
- **T2** — `blit_stone`, `splat_large_sprites`, `scatter_stones`, `generate_dirt_pattern`.
- **T3** — `dig_tunnels` over the Step-2 `draw_dirt_effect`.
- **T4** — `is_no_rock`, `STONE_TAB`, `blit_formation`, `place_rock_formations`, `place_rocks`,
  `RockStats`, `LevelGenAssets`, `generate_random`.
- **T5** — `make_shadow` (in `levelgen.rs`).
- **T6** — `LevelGenParams`, `level_file_name`, `generate_from_settings`.
- **T7 — MILESTONE** — `levelgen_golden.rs`: the full matrix bit-exact, every stage, `MakeShadow`,
  the `shadow=0` dig stage, the file paths, coverage guards.
- **T8** — eyeball example, `rust/README.md`, PROGRESS, full re-diff (workspace tests, `game`
  tests, wasm build).
- **T9 (after 4½a T3)** — the `shadow=1` dig tokens through 4½a's `correct_shadow`: a
  function-level bit-exact check of `CorrectShadow`.

---

## Deferrals

| Item | Why | Owner |
|---|---|---|
| `SelectSpawn` (`level.cpp:435-487`) | only reachable from Holdazone's `SpawnZone` (F1) | with Holdazone (post-4½) |
| `CorrectShadow` port + wiring into the 7 sim call sites + `SimState.shadow` + shadow-on sim goldens | settings→sim plumbing of a `SerializeGameplay` field; `state.rs` / `sim/src/shadow.rs` are 4½a's files | **4½a** (its design §5.1); 4½b's T9 adds the function-level oracle check |
| NEW-GAME reuse-level rule (`gfx.cpp:1507-1523`) and level provenance (`level.cpp:421-424`) | screen-router logic | 4½d |
| Level-file I/O (native read, wasm manifest, merged user/data view) feeding `generate_from_settings(.., file, ..)` | I/O, not `sim` | 4½a/4½d (native), 4½h (wasm) |
| Shell seed plumbing (`MatchConfig` seed → `level_rand`) | needs `MatchConfig` | 4½a/4½d |
| Level-palette-wins rule (`game.cpp:476-479`) in the builder | builder concern (F8) | 4½a |
| Minimap preview of the RANDOM node (`DrawMiniature`, `level.cpp:489`; Rust `render/src/hud.rs:298-330`, private) | level selector | 4½e |
| MODERNLV `display_valid` clearing by `MakeShadow` on loaded modern levels | Rust renders no display layers | first step that renders MODERNLV |
| 4096×4096 golden line | debug-test runtime | revisit if a size-specific bug appears |
| `BlitStone` `p1 = true` branch | no caller | never, unless a caller appears |

## Open questions for the controller (with recommendations)

1. **`CorrectShadow` ownership and T9's sequencing.** The 4½a design (§5.1) ports
   `sim::shadow::correct_shadow(level, x1, y1, x2, y2)` and wires it. **Recommendation:** keep that
   split; schedule 4½b's T9 right after 4½a's T3 lands on the branch, so the pixel rule gets a
   function-level C++ check on top of 4½a's sim-level shadow-on goldens. If 4½a's final signature
   differs from `(&mut LevelSim, i32, i32, i32, i32)`, T9 adapts its one call site. **Naming:** the
   4½a design calls 4½b's entry point `prepare_level`; this design keeps the C++ name
   `generate_from_settings` — the controller picks one (recommend the C++ name, the project's
   convention) and the other doc follows.
2. **Accept the `SelectSpawn` deferral** (it contradicts the overview's 4½b scope and done-when #3)?
   **Recommendation: yes** (F1), and amend the overview's done-when #3 to "…+ `SelectSpawn` with
   Holdazone".
3. **Level seed derivation.** **Recommendation: `level_seed = match_seed`** (§2), documented
   correlation. Alternative: a fixed xor constant — no benefit, breaks the "golden seed = match
   seed" readability.
4. **Max-size golden line.** **Recommendation: leave 4096×4096 out** (§7); add it only behind
   `#[ignore]` if a size bug ever shows up.
5. **A sim golden on a generated non-504×350 level** (R6). **Recommendation:** 4½a or 4½e adds one
   (e.g. a generated 128×96 and 600×350 level driven through the ordinary scenario path) when map
   size becomes menu-reachable.

## Risks & the hard 10%

- **R1 — RNG draw order in the field.** Column x=0 before row y=0 (`level.cpp:14-20`) is the
  easiest thing to get backwards, and it shifts all ~176 k following draws. Pinned by a unit test
  that hand-orders a 3×2 case and by the `field` stage token.
- **R2 — splat clipping and the blend rule.** `rand(w)-8` makes negative origins routine; C++ uses
  `break` for `≥ h`/`≥ w` and `continue` for `< 0` (not a clamp of the source offset as in
  `CLIP_IMAGE`); the blend fires only for destination `177..=179` (`> 176 && < 180`) with
  `(src+dest)/2`. Pinned by unit tests and the `splats` token.
- **R3 — `BlitStone` clip height.** Full `h` (not `h-1` as in `DrawDirtEffect`); non-zero texels
  overwrite unconditionally. Pinned by a bottom-row unit test and the `stones`/`formations` tokens.
- **R4 — `IsNoRock` + the retry cap.** The window is `(size+1)²`; `++tries` only on rejection; on
  the cap the kind/sprite draw is **skipped** (`continue` before `rand(3)` / `rand(6)`). A wrong
  `≤` vs `<` or an extra draw only shows on small maps — hence the cap-forcing matrix, the
  `RockStats` columns and the dumper's coverage guard.
- **R5 — `DrawDirtEffect` draws even when fully clipped.** Tunnel stamps near edges consume
  `rand(2)` and write nothing; content hashes cannot see a missing draw, only `rand.last` can —
  which is why every stage token carries it. The Step-2 port already draws before clipping
  (`blit.rs:54-55`); T3 adds a test that pins it on a tiny level.
- **R6 — in-match behaviour on non-default sizes is ungated** (F5). Out of 4½b's scope; Open Q5.
- **R7 — the shadow passes' read order.** x-outer/y-inner and the post-rule-1 re-read in
  `MakeShadow` (§5). Pinned by a unit test whose neighbour would change if visited early.
- **R8 — the replica drifting from `level.cpp`.** Neutralised by the three self-checks (§10.2).
- **R9 — test runtime.** 42 generations in a debug build; the smallest maps run the cap loops
  (`w·h` candidates × `IsNoRock` scans). Expected seconds, not minutes; 4096² is excluded (Open Q4).

## Notes handed to later slices

- **4½a:** map `MatchConfig` (`random_level`, `random_map_width/height`, `shadow`, `level_file`)
  onto `LevelGenParams`; `CorrectShadow` port + wiring stay 4½a's, and 4½b's golden dig stage is
  available as its function-level oracle (T9, Open Q1); honour `LevelData.palette` (F8); the match
  path must level-prepare via `generate_from_settings`, never `assets::level::load` alone (F3).
- **4½d:** NEW GAME keeps `(LevelGenParams, level_file)` beside the current level to implement the
  reuse rule (`gfx.cpp:1512-1518`); `Gfx::InitFrameStepping` equivalent generates with the default
  params at launch (overview done-when #1).
- **4½e:** the RANDOM node's preview can call `generate_from_settings` with the pending params and
  draw it via a public `DrawMiniature` port (today private in `render/src/hud.rs:301`).
- **Step 5:** generation consumes a dedicated `Rand`, so the sim stream is not advanced (§2); both
  Rust peers regenerate from the seed, or the host ships the level as C++ does
  (`session.cpp:693-704`). Cross-play with C++ would require passing the sim `Rand`.
