# Step 1, sub-slice 1e-2 — object configs (weapon / nobject / sobject): design

Status: **draft for review** · 2026-06-26
Part of: `2026-06-26-liero-rs-step1e-tc-bundle-overview.md`
Follows: `2026-06-26-liero-rs-step1e1-tc-config-design.md`

Sub-slice 1e-2 of the Liero-rs rewrite: read OpenLiero's per-object **`.cfg`
files** (`weapons/*.cfg`, `nobjects/*.cfg`, `sobjects/*.cfg`, all TOML) in Rust,
reproducing the C++ engine's parsed values **and every resolved cross-reference
index**. This adds an `object` module to the `assets` crate. It reuses the
`serde`/`toml` dependency 1e-1 introduced and **consumes 1e-1's type-name lists**
(`TcConfig.types`) to resolve object/sound names to indices. As always: the C++
engine is the oracle; a C++ dumper runs the real `Common::load` and a golden
differential test pins the parsed values.

## Scope

In scope (LOCKED bit-exact — these parameters feed the deterministic
`processFrame`; the resolved cross-ref indices ARE sim state):

- **`Weapon`** — parse `weapons/<id_str>.cfg` mirroring `LoadWeaponConfig`
  (`src/game/common_model.hpp:256–336`). 50 config fields + `id` (= array index)
  + `id_str` (from 1e-1). 40 files in the shipped TC.
- **`NObjectType`** — parse `nobjects/<id_str>.cfg` mirroring `LoadNObjectConfig`
  (`common_model.hpp:96–138`). 28 config fields + `id` + `id_str`. 24 files.
- **`SObjectType`** — parse `sobjects/<id_str>.cfg` mirroring `LoadSObjectConfig`
  (`common_model.hpp:160–178`). 12 config fields + `id` + `id_str`. 14 files.
- **Cross-reference resolution by name → index**, reusing 1e-1's exact semantics
  (`ObjRefFromStr` / `SoundRefFromStr`, `common_model.hpp:16–52`), against the
  name lists 1e-1 built:
  - object refs (`createOnExp`, `splinterType`, `objTrailType`, `partTrailObj`,
    `leaveObj`) → `obj_ref_from_str(name, list)`: **empty → -1, unknown → 0,
    hit → index**.
  - sound refs (`launchSound`, `loopSound`, `exploSound`, `startSound`) →
    `sound_ref_from_str(name, sounds)`: **empty → -1, unknown → -1, hit → index**.
- **`id = array index`** assignment (the `Precompute` step that matters here,
  `common.cpp:491–507`).

Explicitly out of scope (deferred):

- `tc.cfg` — **1e-1** (done; supplies the lists this slice consumes).
- WAV decode — **1e-3**.
- `Precompute()`'s `weap_order` name-sort and derived worm/fire-cone sprites
  (rendering, step 3). Only `id = index` is reproduced here.
- Writing configs back out (`Save*Config`).
- Reproducing toml++/cereal type-mismatch *leniency* on malformed input (a
  wrong-typed key is silently ignored in C++; `serde` errors). The locked
  contract is "read the *real shipped* assets identically"; the shipped `.cfg`
  files are well-formed. Same accepted divergence as 1e-1.

## Dependency on 1e-1 (the cross-ref lists)

The C++ load loop (`common.cpp:437–486`) iterates the `weapons[]` /
`nobject_types[]` / `sobject_types[]` slots that **`LoadTcConfig` already built
from `tc.cfg [types]`** (each slot carries only its `id_str` at this point), reads
`<id_str>.cfg`, and resolves every name reference **against those same vectors**:

| Ref field | Resolves against | C++ list | 1e-1 source |
|---|---|---|---|
| `createOnExp`, `objTrailType`, `leaveObj` | sobject types | `common.sobject_types` | `TcConfig.types.sobjects` |
| `splinterType`, `partTrailObj` | nobject types | `common.nobject_types` | `TcConfig.types.nobjects` |
| `launchSound`, `loopSound`, `exploSound`, `startSound` | sounds | `common` (`SoundIndex`) | `TcConfig.types.sounds` |

So 1e-2 **cannot be index-correct without 1e-1**: the resolution lists are exactly
`TcConfig.types.{sobjects, nobjects, sounds}`, and the *order* in those lists fixes
the resolved indices, which are sim state. 1e-2 also needs the ordered
`TcConfig.types.{weapons, nobjects, sobjects}` `id_str` lists to know **which
files to read and in what order** (the order = the `id`).

### How 1e-2 obtains the resolution helpers (decision)

1e-1's `tc.rs` already contains `obj_ref_from_str` and `sound_index`, **but they
are private** to that module, and this slice **must not modify `rust/`'s existing
files** (parallel-agent constraint). Therefore 1e-2 **re-implements** the two
tiny helpers in `object.rs` from the same C++ source (3 lines each), rather than
importing them. They are transcribed identically:

```rust
// ObjRefFromStr (common_model.hpp:24): empty -> -1, unknown -> 0, else index.
fn obj_ref_from_str(s: &str, list: &[String]) -> i32 {
    if s.is_empty() { return -1; }
    match list.iter().position(|n| n == s) { Some(i) => i as i32, None => 0 }
}
// SoundRefFromStr (common_model.hpp:47): empty -> -1, else SoundIndex (-1 if absent).
fn sound_ref_from_str(s: &str, sounds: &[String]) -> i32 {
    if s.is_empty() { return -1; }
    sounds.iter().position(|n| n == s).map_or(-1, |i| i as i32)
}
```

Note the **two different unknown-name behaviours** (object → `0`, sound → `-1`):
this is deliberate in C++ (`common_model.hpp:37–39` comment: an unknown sound
must be -1 so it does not spuriously play sound 0). 1e-2 preserves both.

> **Controller note:** a later cleanup could promote `tc.rs`'s helpers to `pub`
> and have `object.rs` import them, deduping the ~6 lines. Out of scope now
> because we cannot touch `rust/`. Listed as open question #1.

The loader API takes the relevant name slices as parameters (decoupled from the
whole `TcConfig`), so each struct is unit-testable with synthetic lists. A thin
aggregate (`load_all`) bridges from a `TcConfig` + a file-reader closure.

## Background: the C++ format (oracle truth)

### The three loaders (`common_model.hpp`)

Each `Load*Config` constructs a `cereal::TomlInputArchive` over the flat
key/value `.cfg` table and reads each field by camelCase TOML key into the
default-zero-initialized struct slot (the slot came from
`vector::resize(types.size())`, so all members are value-initialized to
`0`/`false`/`""` before reading; absent keys keep that default —
`toml_archive.hpp:217–268` only assigns present, correctly-typed keys). String
ref fields are read into a local `std::string ref` (default `""`) and then passed
through `ObjRefFromStr` / `SoundRefFromStr`.

The struct member types come from `weapon.hpp:12–332`, `nobject.hpp:18–199`,
`sobject.hpp:18–87`. Note `using fixed = int;` (`math.hpp:7`) — so `fixed`
fields (`gravity`, `addSpeed`) are plain 32-bit ints; every numeric field is
`i32` in Rust.

### The load loop and index assignment (`common.cpp:437–507`)

```
for (auto& w : weapons)        { read weapons/<w.id_str>.cfg;  LoadWeaponConfig(*this, w, is); }
for (auto& w : nobject_types)  { read nobjects/<w.id_str>.cfg; LoadNObjectConfig(*this, w, is); }
for (auto& w : sobject_types)  { read sobjects/<w.id_str>.cfg; LoadSObjectConfig(*this, w, is); }
Precompute();   // weapons[i].id = i; nobject_types[i].id = i; sobject_types[i].id = i;
```

`id == array index == position in `tc.cfg [types]``. The resolved cross-ref
indices point into these same arrays. 1e-2 reproduces `id = index` in `load_all`.

### Field-by-field classification (sim vs rendering/audio)

All fields are **golden-locked regardless** (we "read the bytes the same"), but
the charter wants each labelled so step 2 knows what it may depend on. The key
research finding: several nominally "rendering/audio" fields **feed RNG draws**
and are therefore sim-critical for determinism, not free to drift.

**`Weapon`** (50 fields, in `LoadWeaponConfig` read order):

| Field(s) | Class | Evidence |
|---|---|---|
| `name` | rendering/UI | menu label; `weap_order` sort key (`common.cpp:499`) |
| `affectByWorm`, `wormExplode`, `explGround`, `wormCollide`, `collideWithObjects`, `affectByExplosions` | **SIM** | `Weapon::Fire` / `WObject::Process` collision/explosion logic (`weapon.cpp`) |
| `shadow`, `laserSight`, `fireCone`, `loopAnim` | rendering | sprite/laser/firecone draw only |
| `playReloadSound`, `launchSound`, `loopSound`, `exploSound` | audio | `sound_player->Play` sites (`weapon.cpp:94`); **see `loopSound` quirk below** |
| `detectDistance`, `blowAway`, `gravity`, `speed`, `addSpeed`, `distribution`, `parts`, `recoil`, `multSpeed`, `delay`, `loadingTime`, `ammo`, `bounce`, `timeToExplo`, `timeToExploV`, `hitDamage`, `bloodOnHit`, `shotType`, `splinterAmount`, `splinterColour`, `splinterScatter`, `objTrailDelay`, `partTrailType`, `partTrailDelay`, `leaveShells`, `leaveShellDelay`, `chainExplosion` | **SIM** | physics/damage/firing in `weapon.cpp`; several draw RNG (`distribution`, `timeToExploV`, `leaveShells`) |
| `dirtEffect` | **SIM** | `DrawDirtEffect` mutates the live material map → collision (`gfx/blit.cpp`, called from `weapon.cpp:120`) |
| `startFrame`, `numFrames`, `colorBullets` | **SIM (via RNG)** | `obj->cur_frame = game.rand(num_frames + 1)` gated by `start_frame >= 0` (`weapon.cpp:39–43`); `color_bullets - game.rand(2)` (`weapon.cpp:68`) — these consume the deterministic RNG |
| `splinterType` → nobject, `objTrailType` → sobject, `partTrailObj` → nobject, `createOnExp` → sobject | **SIM (resolved index)** | spawn calls `common.{n,s}object_types[idx].Create*` (`weapon.cpp:89–110,196–207`) |

**`NObjectType`** (28 fields, `LoadNObjectConfig` read order): essentially all
**SIM**. `wormExplode`/`explGround`/`wormDestroy`/`affectByExplosions`/`bloodTrail`
(bools), `detectDistance`/`gravity`/`speed`/`speedV`/`distribution`/`blowAway`/
`bounce`/`hitDamage`/`bloodOnHit`/`splinterAmount`/`splinterColour`/
`bloodTrailDelay`/`leaveObjDelay`/`timeToExplo`/`timeToExploV` drive
`NObject::Process` physics/damage; `drawOnMap` removes the object from the sim and
writes terrain (`nobject.cpp:119`); `dirtEffect` mutates terrain; `startFrame`/
`numFrames` feed RNG (`obj.cur_frame = game.rand(num_frames + 1)` gated
`start_frame > 0`, `nobject.cpp:24–25`); `colorBullets` is colour (rendering, but
golden-locked); `createOnExp` → sobject, `splinterType` → nobject, `leaveObj` →
sobject are resolved indices used by `Create*`/spawn (`nobject.cpp:137,206–225`).

**`SObjectType`** (12 fields, `LoadSObjectConfig` read order):

| Field(s) | Class | Evidence |
|---|---|---|
| `detectRange`, `damage`, `blowAway` | **SIM** | worm damage / pushback (`sobject.cpp:45–84`) |
| `dirtEffect` | **SIM** | terrain mutation |
| `numSounds` | **SIM (via RNG)** | `game.rand(num_sounds)` is drawn even though the `Play` is audio (`sobject.cpp:24`) |
| `startSound` | audio | resolved sound index; `Play(rand(num_sounds) + start_sound)` (`sobject.cpp:24`) |
| `shadow`, `animDelay`, `startFrame`, `numFrames`, `shake`, `flash` | rendering | shadow/animation/screen-shake/screen-flash (`sobject.cpp:31,39,41`) |

The takeaway mirrors 1e: the *bulk* is sim, and even the frame/sound-count fields
participate in determinism via RNG, so 1e-2 locks **every** parsed value and
**every** resolved index.

### The `loopSound` int-into-bool quirk (must reproduce exactly)

`Weapon::loop_sound` is declared **`bool`** (`weapon.hpp:67`, comment literally
says "Buggy."), but `LoadWeaponConfig` assigns it the *int* result of
`SoundRefFromStr` (`common_model.hpp:277–281`):

```cpp
std::string ref; ar(make_nvp("loopSound", ref));
w.loop_sound = SoundRefFromStr(ref, common);   // int -> bool
```

C++ `int → bool` is `(value != 0)`. So the stored `loop_sound` is:
`""` → `-1` → **true**; an unknown name → `-1` → **true**; a sound at index `0`
→ **false**; any sound at index ≥ 1 → **true**. This is a genuine engine quirk
and the value is sim/parsed state, so 1e-2 reproduces it bit-exactly:

```rust
loop_sound: sound_ref_from_str(raw.loopSound, sounds) != 0   // bool, mirrors int->bool
```

`launchSound` and `exploSound` are plain `int` (`weapon.hpp:63,71`) and stay `i32`.
The golden hashes `loop_sound` as a single bool byte, identical on both sides.

## Rust design

### Crate layout

```
rust/assets/src/
├── …            (1a–1d, unchanged)
├── tc.rs        (1e-1, unchanged — consumed read-only via `assets::tc`)
├── object.rs    ← NEW (1e-2): Weapon/NObjectType/SObjectType + loaders
└── lib.rs       (MODIFY: `pub mod object;`)
```

No new dependencies (`serde` + `toml` already present from 1e-1). No Bevy.

### Two-layer serde approach (mirrors 1e-1's raw → public split)

A flat `.cfg` is `#[derive(Deserialize)]`'d into a **private `Raw*` mirror** whose
field names are the camelCase TOML keys verbatim (`#![allow(non_snake_case)]`,
`#[serde(default)]` for missing-key → zero/empty defaults, matching the C++
value-initialized slot). The ref fields are `String` in the raw mirror. `load()`
then post-processes into the **public struct** (idiomatic snake_case matching the
C++ member names, which is what step 2 reads), resolving each ref `String` to its
`i32` index (and the `loopSound` bool). This is the analogue of `Load*Config`'s
"read into local, resolve, store" step.

### Public types (`object.rs`)

```rust
pub struct Weapon {
    pub name: String,
    pub affect_by_worm: bool, pub shadow: bool, pub laser_sight: bool,
    pub play_reload_sound: bool, pub worm_explode: bool, pub expl_ground: bool,
    pub worm_collide: bool, pub collide_with_objects: bool,
    pub affect_by_explosions: bool, pub loop_anim: bool,
    pub detect_distance: i32, pub blow_away: i32, pub gravity: i32,
    pub launch_sound: i32,            // SoundRefFromStr
    pub loop_sound: bool,             // SoundRefFromStr(...) != 0  (int->bool quirk)
    pub explo_sound: i32,             // SoundRefFromStr
    pub speed: i32, pub add_speed: i32, pub distribution: i32, pub parts: i32,
    pub recoil: i32, pub mult_speed: i32, pub delay: i32, pub loading_time: i32,
    pub ammo: i32, pub dirt_effect: i32, pub leave_shells: i32,
    pub leave_shell_delay: i32, pub fire_cone: i32, pub bounce: i32,
    pub time_to_explo: i32, pub time_to_explo_v: i32, pub hit_damage: i32,
    pub blood_on_hit: i32, pub start_frame: i32, pub num_frames: i32,
    pub shot_type: i32, pub color_bullets: i32, pub splinter_amount: i32,
    pub splinter_colour: i32,
    pub splinter_type: i32,           // ObjRefFromStr against nobjects
    pub splinter_scatter: i32,
    pub obj_trail_type: i32,          // ObjRefFromStr against sobjects
    pub obj_trail_delay: i32, pub part_trail_type: i32,
    pub part_trail_obj: i32,          // ObjRefFromStr against nobjects
    pub part_trail_delay: i32,
    pub create_on_exp: i32,           // ObjRefFromStr against sobjects
    pub chain_explosion: bool,
    pub id: i32,                      // = array index (Precompute)
    pub id_str: String,              // from 1e-1 types.weapons
}

pub struct NObjectType {
    pub worm_explode: bool, pub expl_ground: bool, pub worm_destroy: bool,
    pub draw_on_map: bool, pub affect_by_explosions: bool, pub blood_trail: bool,
    pub detect_distance: i32, pub gravity: i32, pub speed: i32, pub speed_v: i32,
    pub distribution: i32, pub blow_away: i32, pub bounce: i32, pub hit_damage: i32,
    pub blood_on_hit: i32, pub start_frame: i32, pub num_frames: i32,
    pub color_bullets: i32,
    pub create_on_exp: i32,           // ObjRefFromStr against sobjects
    pub dirt_effect: i32, pub splinter_amount: i32, pub splinter_colour: i32,
    pub splinter_type: i32,           // ObjRefFromStr against nobjects
    pub blood_trail_delay: i32,
    pub leave_obj: i32,               // ObjRefFromStr against sobjects
    pub leave_obj_delay: i32, pub time_to_explo: i32, pub time_to_explo_v: i32,
    pub id: i32, pub id_str: String,
}

pub struct SObjectType {
    pub shadow: bool,
    pub start_sound: i32,             // SoundRefFromStr
    pub num_sounds: i32, pub anim_delay: i32, pub start_frame: i32,
    pub num_frames: i32, pub detect_range: i32, pub damage: i32, pub blow_away: i32,
    pub shake: i32, pub flash: i32, pub dirt_effect: i32,
    pub id: i32, pub id_str: String,
}

pub enum ObjectError { Parse(String) }     // TOML/UTF-8 error (message carried)

impl Weapon {
    /// Mirrors LoadWeaponConfig (common_model.hpp:256). `id`/`id_str` set by caller.
    pub fn load(bytes: &[u8], nobjects: &[String], sobjects: &[String],
                sounds: &[String]) -> Result<Weapon, ObjectError>;
}
impl NObjectType {
    /// Mirrors LoadNObjectConfig (common_model.hpp:96).
    pub fn load(bytes: &[u8], nobjects: &[String], sobjects: &[String])
        -> Result<NObjectType, ObjectError>;
}
impl SObjectType {
    /// Mirrors LoadSObjectConfig (common_model.hpp:160).
    pub fn load(bytes: &[u8], sounds: &[String]) -> Result<SObjectType, ObjectError>;
}
```

### Aggregate loader (mirrors the `common.cpp:437–507` loop)

```rust
pub struct Objects {
    pub weapons: Vec<Weapon>,
    pub nobject_types: Vec<NObjectType>,
    pub sobject_types: Vec<SObjectType>,
}

impl Objects {
    /// Read every object .cfg named in `types` and resolve cross-refs, assigning
    /// id = array index (the part of Precompute, common.cpp:491-507, that 1e-2
    /// reproduces). `read(subdir, id_str)` returns the raw bytes of
    /// `<subdir>/<id_str>.cfg` (subdir ∈ {"weapons","nobjects","sobjects"}),
    /// letting the caller choose std::fs / include / FsNode without coupling the
    /// loader to an IO backend.
    pub fn load(
        types: &assets::tc::TcTypes,
        read: impl Fn(&str, &str) -> std::io::Result<Vec<u8>>,
    ) -> Result<Objects, ObjectError>;
}
```

`Objects::load` reads files in `types.weapons` / `types.nobjects` /
`types.sobjects` order, calls the per-struct loaders with
`types.nobjects` / `types.sobjects` / `types.sounds` as the resolution lists, sets
`id = index` and `id_str = types.<list>[index]`. (An `io::Error` from `read` is
surfaced; the locked golden uses the shipped files, which all exist.)

Idiomatic Rust: `serde`/`toml` deserialization, typed `Result`, value structs —
**not** a port of the cereal `TomlInputArchive` per-field `make_nvp` machinery.

## The oracle: real `Common::load`

A new dumper (`src/tools/oracle_dump/object_dump.cpp`, links `game`) runs
`Common::load(FsNode(argv[1]))` (the real loop that calls all three `Load*Config`
+ `Precompute`) and emits FNV-1a digests of `common.weapons[]`,
`common.nobject_types[]`, `common.sobject_types[]`. New CMake target
`oracle_dump_object` under `OPENLIERO_BUILD_ORACLE_DUMP`.

### Golden format (`golden/object.txt`) — three lines

```
weapons  <hash>   # count(u32); then per weapon, fields in LoadWeaponConfig order
nobjects <hash>   # count(u32); then per nobject, fields in LoadNObjectConfig order
sobjects <hash>   # count(u32); then per sobject, fields in LoadSObjectConfig order
```

Per entry the digest includes, in this exact order, **`id` (i32 LE)** and
**`id_str` (u32 LE length + bytes)** first — pinning the `id = index` assignment
and the id_str round-trip — then every config field in the loader's read order:

- ints/`fixed` → `i32` little-endian (`PushI32`);
- bools → 1 byte `0/1` (incl. `Weapon::loop_sound`, the int-into-bool quirk);
- `Weapon::name` → `u32` LE length + bytes (`PushStr`);
- resolved cross-ref fields → their **`i32` index** (LE), so the digest pins the
  resolution (`empty → -1`, `unknown → 0` for objects / `-1` for sounds, hit →
  position).

Hashing the full array (count + every entry in order) pins the **count, the
order, and therefore the index space** in one digest, matching the 1e-1 style
(one digest per group). The byte layout reuses the shared FNV-1a /
`PushI32`/`PushU32`/`PushStr` helpers from the 1c/1d/1e-1 dumpers.

> **Debuggability note (open question #3):** a single combined hash over 40
> weapons is coarse to debug on mismatch. The plan keeps one digest per group
> (1e-1 parity) but notes per-index digests can be added if a regression needs
> bisecting.

### Field order = a single hand-written list on each side

Unlike `tc.cfg` (which has `LIERO_*DEFS` macros), the object structs have **no
field-list macro**. So both sides hand-list the fields in `Load*Config` order:
the C++ dumper inlines the pushes; the Rust golden test has `encode_weapon` /
`encode_nobject` / `encode_sobject` helpers (test-only, not in the lib — the
digest layout is a test concern). Any drift between the two lists is caught by the
golden. The order is transcribed directly from `common_model.hpp:258–335` /
`98–137` / `162–177`.

### Oracle input

The real shipped files: `data/TC/openliero/{weapons,nobjects,sobjects}/*.cfg`
(40 + 24 + 14), exercised through the full `Common::load`. They cover every path:
present and absent ref keys (e.g. `bazooka.cfg` omits `partTrailObj` → `""` →
`-1`; `blood.cfg` omits `createOnExp`/`splinterType`/`leaveObj` → `-1`), hit and
empty sound refs (`exploSound = ""`), the `loopSound` int→bool path, and the
`id = index` assignment for all three arrays. Error paths (malformed TOML) are
covered by `object.rs` unit tests on small synthetic buffers; resolution edge
cases (empty/unknown/hit for both ref kinds) by unit tests with synthetic lists.

## Testing

1. **Smoke test** (TDD de-risk): `toml::from_str` on a real flat `.cfg`
   (`weapons/bazooka.cfg`) must succeed — proves the crate parses the flat-table
   dialect before any struct work (cheap, `toml` already proven on `tc.cfg`).
2. **Unit tests** in `object.rs`, one struct at a time:
   - each loader parses an inline sample, checks scalar/bool fields and defaults
     (missing key → 0/false);
   - `obj_ref_from_str`: empty → -1, unknown → 0, hit → index;
   - `sound_ref_from_str`: empty → -1, unknown → -1, hit → index;
   - **`loop_sound` quirk**: `loopSound=""` → `true` (-1≠0), a sound at index 0 →
     `false`, a sound at index ≥1 → `true`;
   - malformed TOML → `ObjectError::Parse`;
   - a real-file load (`bazooka.cfg` with synthetic lists) for the absent-key path.
3. **Golden differential test** `oracle-tests/tests/object_golden.rs`: load the
   real `TcConfig` (1e-1) + every real `.cfg` via `Objects::load`, reproduce the
   three digests. Regenerated by `gen_object_golden.sh` against the real C++ build
   (local/manual, like 1b–1e-1).
4. CI (`rust.yml`) runs `cargo test --workspace` against the committed golden; it
   does not rebuild the C++ oracle.

**Done when:** the full Rust workspace suite is green and every 1e-2 golden digest
matches C++ bit-for-bit.

## Modernization-charter check

- **Locked / bit-exact:** every parsed weapon/nobject/sobject field AND every
  resolved cross-reference index (the indices ARE sim state), plus `id = index`,
  reproduce C++ exactly — golden-proven against the real `Common::load`. The
  `loopSound` int→bool quirk is preserved.
- **Free to modernize:** `serde`/`toml` + a raw→public two-layer split replace
  the cereal `TomlInputArchive` per-field `make_nvp`; typed `Result`/structs; a
  file-reader closure replaces `FsNode` coupling. None of it is observable in the
  parsed values.
- **Rendering/audio parsed but labelled:** `shadow`/`laserSight`/`fireCone`/
  `loopAnim`/`name`, the sound refs, and the sobject `shadow`/`animDelay`/
  `startFrame`/`numFrames`/`shake`/`flash` are read and golden-verified but
  flagged non-sim — with the caveat that `startFrame`/`numFrames`/`numSounds`/
  `colorBullets` feed RNG draws and are sim-critical for determinism.
- **Accepted divergence:** serde errors on wrong-typed keys where C++ silently
  keeps the default; the locked contract is the real (well-formed) shipped assets
  used by the golden. Same posture as 1e-1.

## Open questions for the controller

1. **Helper duplication vs promoting 1e-1's helpers to `pub`.** This slice
   re-implements `obj_ref_from_str` / `sound_ref_from_str` in `object.rs` (can't
   touch `tc.rs` under the parallel-agent rule). Recommendation: accept the ~6
   duplicated lines now; a follow-up can promote `tc.rs`'s helpers to `pub` and
   import them. They are transcribed from the same C++ source, so they cannot
   diverge silently (golden would catch it).
2. **`loop_sound` representation.** Keep it a `bool` reproducing the C++ int→bool
   bug (recommended — matches `weapon.hpp` and the engine's stored value), vs
   storing the raw `i32` and exposing a different shape to step 2. Recommendation:
   `bool`, bit-exact with the engine.
3. **Golden granularity.** One digest per array (1e-1 parity) vs per-object
   digests for easier bisection on mismatch. Recommendation: one per array; add
   per-index only if a regression needs it.
4. **`Objects::load` IO shape.** A `read(subdir, id_str)` closure (recommended —
   no coupling to 1a's `io.rs` or `FsNode`) vs a concrete `&Path`/`FsNode` API.
   The golden test supplies a `std::fs` closure rooted at `data/TC/openliero`.

## Next concrete artifact

Implementation plan:
`docs/superpowers/plans/2026-06-26-liero-rs-step1e2-object-configs.md`.
