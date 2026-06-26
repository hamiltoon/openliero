# Step 1, slice 1e — TC bundle: overview

Status: **draft for review** · 2026-06-26
Part of: `2026-06-26-liero-rs-step1-data-formats-overview.md`
Follows: `2026-06-26-liero-rs-step1d-sprites-design.md`

Slice 1e is the last row of step 1: read OpenLiero's **TC (total conversion)
bundle** — the configuration, object-parameter, and sound data under
`data/TC/openliero/` — in Rust, reproducing the C++ engine's parsed values. The
TC bundle is the part of step 1 that **most directly feeds the simulation**
(weapon/object parameters, physics constants, material flags, terrain-texture
behaviour, AI tuning, gameplay hacks), so much of it is LOCKED bit-exact and
oracle-tested, while a minority (UI texts, colour-animation ranges, WAV audio,
sound hooks) is rendering/audio.

This document is the 1e altitude ("everything"): the formats, their
**simulation-vs-rendering classification** (with `file:line` citations), the
dependency graph, and a **decomposition into independently buildable,
differential-tested sub-slices**. Each sub-slice gets its own detailed design
spec + plan, built one at a time, as in 1a–1d.

## How C++ loads the bundle (`Common::load`, `src/game/common.cpp:308–489`)

`Common::load(FsNode)` runs, in this exact order:

1. **`tc.cfg`** (`common.cpp:309–323`) — read the whole file to a string, then
   `LoadTcConfig(*this, istringstream)` (`common_model.hpp:565–681`). This
   **populates the `sounds` / `weapons` / `nobject_types` / `sobject_types`
   vectors** (names + `id_str` only) plus all global constants, materials,
   textures, bonuses, AI params, texts, hacks, and sound hooks.
2. **WAV sounds** (`common.cpp:325–363`) — for each `sounds[i]` (slot count and
   name come from step 1), read `sounds/<name>.wav`, validate a fixed RIFF/WAVE
   header, decode 8-bit PCM into `original_data`, then `CreateSound()`
   (`common.cpp:583–602`) upsamples to `int16` playback samples.
3. **Sprites** (`common.cpp:365–435`) — `small`/`large`/`text`/`font` TGAs +
   `modern.pal`. *This is slice 1d* (already specced); 1e does not touch it.
4. **Weapon configs** (`common.cpp:437–452`) — for each `weapons[i]`, read
   `weapons/<id_str>.cfg`, `LoadWeaponConfig` (`common_model.hpp:256–336`).
5. **NObject configs** (`common.cpp:454–469`) — `LoadNObjectConfig`
   (`common_model.hpp:96–138`).
6. **SObject configs** (`common.cpp:471–486`) — `LoadSObjectConfig`
   (`common_model.hpp:160–178`).
7. **`Precompute()`** (`common.cpp:491–556`) — assigns `id` by array index and
   builds `weap_order` (a name-sorted index list), plus derived worm/fire-cone
   sprites (rendering).

The config files are **TOML**, parsed by a custom cereal archive over toml++
(`src/game/serialization/toml_archive.hpp`). `tc.cfg` has sections
(`[types]`, `[constants]`, `[texts]`, `[hacks]`, `[sounds]`); the per-object
`.cfg` files are flat key/value tables. Object configs reference other objects
and sounds **by name**, resolved to **indices** during load against the vectors
built in step 1 (`ObjRefFromStr`/`SoundRefFromStr`, `common_model.hpp:16–52`).

## Format-by-format classification (sim-affecting vs rendering/audio)

LOCKED bit-exact = feeds the deterministic `processFrame`; must reproduce C++
exactly and carry the differential oracle. Rendering/audio = parsed and
verified (we still "read the bytes the same"), but does not gate determinism.

| Format / section | Class | Why (C++ evidence) |
|---|---|---|
| `tc.cfg [types]` (sounds/weapons/nobjects/sobjects name lists) | **SIM** | Define object **identity & index**; `id = array index` (`common.cpp:491–507`), and every cross-ref resolves to these indices (`common_model.hpp:16–35`). |
| `tc.cfg [constants]` scalars (`LIERO_CDEFS`, 72 ints) | **SIM** | Physics/gameplay constants read in `processFrame` via `LC()`, e.g. `vel.y += LC(WormGravity)` (`worm.cpp:197`), `LC(JumpForce)` (`worm.cpp:994`), `LC(MinBounceDown)` (`worm.cpp:166`). |
| `tc.cfg [constants].materials` (256 ints → `u8` flags) | **SIM** | `level.materials[i] = common.materials[material_id[i]]` (`game.cpp:796`, `level.cpp:386`); collision/dirt logic reads the flags (`worm.cpp:1181`). |
| `tc.cfg [constants].textures` (9 × mframe/rframe/sframe/ndrawback) | **SIM** | `DrawDirtEffect` mutates the live material map using `common.textures[dirt_effect]` (`gfx/blit.cpp:534–573`), called from sim (`weapon.cpp:120`, `worm.cpp:783,931`). Changes terrain → changes collision. |
| `tc.cfg [constants].bonuses` (2 × timer/timerV/frame/sobj) | **SIM** | `bonus->timer = rand(bonus_rand_timer[f][1]) + bonus_rand_timer[f][0]` (`game.cpp:252`); `sobject_types[bonus_s_objects[f]].Create(...)` (`bonus.cpp:30`). (`frame` is a sprite index — rendering — but rides along.) |
| `tc.cfg [constants].aiparams` (7 keys × on/off) | **SIM** | Deterministic AI worm input: `rand(ai_params.k[..][..])` (`worm.cpp:516,525,625,…`). AI is part of the locked sim. |
| `tc.cfg [constants].colorAnim` (4 × from/to) | rendering | Only used in `Game::Draw` palette rotation (`game.cpp:175–177`). |
| `tc.cfg [texts]` (`LIERO_SDEFS`, 40 strings) | rendering/UI | Stored in `s[]` (`common.hpp:190`); used for menus/HUD only. |
| `tc.cfg [hacks]` (`LIERO_HDEFS`, 11 bools) | **SIM** | Gameplay toggles read in sim: `h[HFallDamage]` (`worm.cpp:172`), `h[HWormFloat]` (`worm.cpp:240`), `h[HRemExp]` (`weapon.cpp:138`), `h[HBonusDisable]` (`game.cpp:359`). |
| `tc.cfg [sounds]` (`LIERO_SOUNDDEFS`, 8 hook names) | audio | Resolved to indices in `sound_hook[]` (`common_model.hpp:654–680`); used only at play sites. |
| `weapons/*.cfg` (40 files, ~50 fields) | **SIM** | `Weapon` params drive `Weapon::Fire` / `WObject::Process` (`weapon.cpp`). A few fields are rendering/audio (startFrame, numFrames, colorBullets, shadow, laserSight, fireCone, launch/loop/exploSound) but parse together. |
| `nobjects/*.cfg` (24 files, ~30 fields) | **SIM** | `NObjectType` params drive `NObject::Process` (`nobject.cpp`). |
| `sobjects/*.cfg` (14 files, ~12 fields) | **SIM** | `SObjectType` params drive `SObject::Create`/`Process` (`sobject.cpp`); damage/blowAway/detectRange affect worms. (startSound/numSounds/animDelay/frames are audio/rendering.) |
| `sounds/*.wav` (30 files) | audio | PCM decoded into `original_data` + `CreateSound` upsample (`common.cpp:340–361,583–602`). No sim logic reads sample data. |

**Key takeaway for 1e:** the *bulk* of the TC bundle is sim-affecting. The
rendering/audio islands are: `[texts]`, `[constants].colorAnim`, `[sounds]`
hooks, a handful of per-object frame/colour/sound fields, and the WAV payload.
We still parse and golden-verify the rendering/audio data (consistent with 1c,
which read MODERNLV display data byte-exact even though it is rendering), but the
spec labels each so step 2 knows what it may depend on.

## Decomposition into sub-slices

Three sub-slices, ordered so that what step 2 needs first (and what later
sub-slices depend on) comes first:

| Sub-slice | Loads | Produces | Depends on | Oracle input |
|---|---|---|---|---|
| **1e-1** | `tc.cfg` (TOML) | `TcConfig`: types lists, constants (scalars + materials + textures + bonuses + colorAnim + aiparams), texts, hacks, resolved sound hooks | 1a (none new); introduces a TOML crate | real `data/TC/openliero/tc.cfg` (via full `Common::load`) |
| **1e-2** | `weapons/*.cfg`, `nobjects/*.cfg`, `sobjects/*.cfg` | `Vec<Weapon>` / `Vec<NObjectType>` / `Vec<SObjectType>` param tables, with name→index cross-refs resolved | 1e-1 (types lists for cross-ref + sound names) | the 40+24+14 real `.cfg` files |
| **1e-3** | `sounds/*.wav` | decoded `Sound` samples (`original_data` + upsampled `int16`) | 1e-1 (sound name list / slot count) | the 30 real `.wav` files |

### Why this order

1. **1e-1 (tc.cfg) first** — it is the foundation. It supplies the global
   constants, material-flag table, textures, bonuses, AI params and hacks that
   the **simulation (step 2) needs directly**, and it builds the **types lists
   and sound-name list** that 1e-2 and 1e-3 consume for index resolution.
   Nothing else can be index-correct until tc.cfg is parsed. It also introduces
   the TOML crate, which 1e-2 reuses.
2. **1e-2 (object configs) second** — the largest *sim* payload (weapon/nobject/
   sobject parameters that step 2's `processFrame` reads). Each object `.cfg`
   resolves cross-references (`splinterType`, `createOnExp`, `objTrailType`,
   `partTrailObj`, sound refs) into indices using 1e-1's lists, so it **must
   follow 1e-1**.
3. **1e-3 (WAV) last** — pure audio. Nothing in the deterministic sim reads
   sample data, and no later slice depends on it, so it is sequenced last. It
   still needs 1e-1's sound-name list to know which files/slots exist.

This mirrors the step-1 rule "sim-affecting data that step 2 needs comes first."

## Crate layout

The 1e loaders live in the `assets` crate beside `level.rs`/`palette.rs`/
`sprite.rs` (no Bevy, as ever):

```
rust/assets/src/
├── io.rs        (1a)
├── level.rs     (1b, 1c)
├── palette.rs   (1c)
├── sprite.rs    (1d)
├── tc.rs        ← NEW (1e-1): tc.cfg → TcConfig (TOML)
├── object.rs    ← NEW (1e-2): weapon/nobject/sobject .cfg param tables
├── wav.rs       ← NEW (1e-3): RIFF/WAVE sound decode
└── lib.rs       (re-export the three)
```

(The exact module split is finalized per sub-slice; the overview commits only to
"new format loaders live in `assets`".)

## Dependencies decided per slice

- **TOML + serde:** introduced in **1e-1** (the first TOML consumer), reused by
  1e-2. Concretely `toml` + `serde` (derive). This is the idiomatic Rust path —
  **not** a port of the cereal `TomlInputArchive` (`toml_archive.hpp`) or its
  prologue/epilogue machinery. Lands in `assets`, never in `sim-core`.
- **No deflate, no extra deps for 1e-3:** the WAVs are uncompressed 8-bit PCM
  with a fixed header; a hand-rolled reader (like 1d's TGA) suffices. No `hound`
  needed — OpenLiero accepts exactly one WAV shape (`common.cpp:345–349`).
- `sim-core` stays dependency-free.

## Modernization-charter posture

- **Locked / bit-exact:** every sim-affecting parsed value — constants,
  material flags, textures, bonuses, AI params, hacks, and all weapon/nobject/
  sobject parameters (including resolved cross-reference indices) — reproduces
  C++ exactly, golden-proven against the real `Common::load`.
- **Free to modernize:** representation and parsing. We use `serde`/`toml`
  deserialization into typed structs instead of cereal's archive; typed
  `Result`/errors; we drop C++'s in-memory artifacts (e.g. the `sfx_sound`
  handle, the `name_counter` archive bookkeeping). Cross-ref resolution is the
  same name→index semantics (`ObjRefFromStr`: empty→-1, unknown→0, else index;
  `SoundRefFromStr`: empty→-1, unknown→-1, else index), reproduced because those
  indices ARE sim state.
- **Rendering/audio:** parsed and verified for completeness, labeled non-sim.

## Oracle dumper, reused

Same pattern as 1d: each sub-slice's C++ dumper runs the **real
`Common::load(FsNode("data/TC/openliero"))`** and emits FNV-1a digests of the
relevant parsed `Common` fields (e.g. `common.c[]`, `common.materials[].flags`,
`common.weapons[]`, `common.sounds[].original_data`). New CMake targets
(`oracle_dump_tc`, `oracle_dump_object`, `oracle_dump_wav`) sit under the
existing `OPENLIERO_BUILD_ORACLE_DUMP` option. Golden regen is local/manual
(full C++ build); CI runs `cargo test --workspace` against committed goldens.
Because the dumper runs the *whole* loader, every sub-slice's golden requires the
complete shipped `data/TC/openliero` (it does — the in-tree tests load it
routinely).

## Deliberately deferred

- Writing TC bundles back out (`Save*Config`) — we only read.
- `Precompute()`'s derived tables (`weap_order` name-sort, worm/fire-cone
  sprites) — `weap_order` is a menu-ordering convenience (rendering); the derived
  sprites are rendering (step 3). `id = index` assignment is trivially implied by
  vector position and is reproduced in 1e-2.
- The `CreateSound` upsample may be deferred within 1e-3 to step 3 if it proves
  to be playback-only; 1e-3's design will decide (the locked read is
  `original_data`).
- Settings/profile TOML and replay formats (separate future concerns).

## Next concrete artifact

Detailed design for the first sub-slice:
`2026-06-26-liero-rs-step1e1-tc-config-design.md`, and its implementation plan
`plans/2026-06-26-liero-rs-step1e1-tc-config.md`.
