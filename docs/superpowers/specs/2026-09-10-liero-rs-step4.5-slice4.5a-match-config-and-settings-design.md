# Step 4½ · Slice 4½a — `MatchConfig`, settings model, builder, match end, persistence: detailed design

Status: **DESIGN — slice 4½a** (split into **4½a-1** and **4½a-2**, §0) · 2026-09-10 · 4½a-1 and 4½a-2
LANDED
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (cited **overview §N** / **LD N** for its
locked decisions)
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` (**cpp-map §N**) and
`2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (**rust-map §N**)
Precedents: the 4b record/replay design (the "one artifact, frozen grammar" argument), the 4e `.lrp` design
(the phase split + a new C++ oracle tool), the slice-6 fuzz goldens (the seeded-dead, fuzz-input match shape)
Companion plans: `plans/2026-09-10-liero-rs-step4.5-slice4.5a1-plan.md` (4½a-1) and
`plans/2026-09-10-liero-rs-step4.5-slice4.5a2-plan.md` (4½a-2)

4½a is the substrate every later 4½ slice configures a match through: a Rust model of the C++ `Settings` /
`WormSettings`, a `MatchConfig → SimState` builder that is **bit-exact against a C++ setup file**, the
match-end rules (`Game::IsGameOver`, the 180-frame post-mortem), the C++ TOML persistence, and two live bug
fixes. Reading the C++ closely turned up more sim surface than the overview budgeted (§1.3: `shadow` is
sim-reaching, the Scales/GameOfTag death-side rules are unported, the shipped settings files are *legacy*
formats), so the slice is split and the byte-identity gate is restated (§3.4).

---

## 0. Scope and the 4½a-1 / 4½a-2 split

4½a as the overview scopes it is ~17 tasks — too many for one reviewable plan. It splits cleanly along the
two hard gates, with one ordering constraint: the sim goldens read their match configuration from a
**C++-schema setup file** (§7.1), so the TOML *reader* must land in the first half.

| | 4½a-1 — sim side (plan written) | 4½a-2 — persistence side (outlined, §9.2) |
|---|---|---|
| Model | `Settings`/`WormSettings`/`MatchConfig` (C++ names + defaults) | — |
| TOML | **reader** (`TomlInputArchive` semantics, §3.2) | **writer** (toml++ formatter port, §3.3) + byte gate + `UpdateHash` |
| Sim | `CorrectShadow`; Scales death/respawn rules; `DoHealing`; GameOfTag guard; `IsGameOver` | — |
| Builder | `build_match` + validation + refusals | — |
| Oracle | `settings <file>` dumper directive + 12th column; 4 settings-driven goldens; builder-reproduces-fuzz5 | `oracle_dump_settings` tool + TOML/hash goldens |
| Lifecycle | `MatchFlow` (IsGameOver → 180 → finished), live wiring | — |
| Storage | — | `ConfigStore` seam, native merged-read store, `TC_ROOT`/`DATA_ROOT` centralised |
| Bug fixes | `sound_hooks` never assigned | HUD off in the live binary (needs the loaded `map`) |
| Hard gate | **1** (settings→sim goldens) | **5** (TOML byte gate) |

Each half is independently shippable and ends green on the standing re-diff. 4½b (level generation) runs in
parallel with both; 4½c needs 4½a-1 (`MatchConfig`, `weap_table`, per-worm picks); 4½e needs 4½a-2 (save).

## Goal / done-when (4½a as a whole)

1. **Model.** `scenario::settings::{Settings, WormSettings, MatchConfig}` carry every C++ field
   (`settings.hpp:11-102`, `worm.hpp:44-118`) under the C++ member name, the C++ integer width, and the C++
   default (`settings.cpp:17-60`, `worm.hpp:61-95`, `worm.cpp:21-32`).
2. **Builder.** `scenario::build::build_match(tc_root, &MatchConfig, &LevelData) -> Result<Loaded, BuildError>`
   produces the C++ `LocalController` start state (§4) and closes every `scenario::load` gap: per-worm weapons,
   lives, `loading_time`, health, `max_bonuses` + all bonus consts, `weap_table`, `sound_hooks`, `shadow`,
   `blood_particle_max`, `time_to_lose`, `game_mode` — and honours the level's POWERLEVEL palette gated on
   `load_powerlevel_palette` (a render bug `scenario::load` has, §4.1). It takes a ready `LevelData`;
   level preparation is 4½b's `sim::levelgen::generate_from_settings`, composed in 4½d. Holdazone and invalid configs are **refused with an
   error**, never a runtime panic (§4.3). `SimState::new`'s signature is unchanged (LD 5).
3. **Sim completion.** `CorrectShadow` (§5.1), the Scales death + respawn rules, `DoHealing`'s Scales
   redistribution, the GameOfTag "it" guard (§5.2), and `sim::game_over::is_game_over` (§5.3) are ported.
4. **Lifecycle.** `game::match_flow::MatchFlow` reproduces `LocalController`'s game → game-ended → finished
   sequence: `IsGameOver` flips, the sim keeps ticking for exactly 180 frames, then the flow reports
   finished (§6). The live binary restarts the match there until 4½g routes it to the stats screen.
5. **Hard gate 1.** Four settings-driven sim goldens (`defaults`, `killemall`, `scales`, `gametag`) match C++
   bit-for-bit on all 11 hash columns **plus a 12th `IsGameOver` column**, and between them vary every
   sim-reaching field (§7.3). The builder also reproduces the existing `sim_slice6_fuzz5` golden unchanged
   (§7.4). Every prior golden stays byte-identical; the dumper's absent-directive path regenerates
   byte-identical output.
6. **Hard gate 5 (4½a-2).** For the shipped `data/Setups/{liero,orbmit}.cfg`, the eight `data/Profiles/*.toml`,
   the 4½a-1 setup sidecars and a synthetic quoting/edge corpus: *Rust load + Rust save* equals *C++ load +
   C++ save* byte-for-byte, and a C++-saved file round-trips through Rust byte-identical (§3.4).
   `Settings::UpdateHash` / `WormSettings::UpdateHash` match C++ vectors.
7. **Storage (4½a-2).** A `ConfigStore` seam with a native merged-read store mirroring `paths::Resolve`
   (reads: user dir over `data/`; writes: user dir only); the wasm localStorage store is 4½h.
8. **Bugs.** `state.sound_hooks` is assigned (4½a-1); the live binary draws the HUD honouring `settings.map`
   (4½a-2).

**Not in 4½a:** random level generation and `MakeShadow` (4½b), weapon selection (4½c), menus (4½d–g), AI
(4½f), key-binding translation DOS-scancode ↔ `KeyCode` (4½f — 4½a only *stores* `controls`), the stats
screen (4½g), wasm persistence (4½h).

---

## Locked decisions inherited (from the overview, restated where 4½a leans on them)

- **LD 3** — `tick_and_render` stays the only `Sim` mutator. `MatchFlow` is called *from* it and only reads
  the state.
- **LD 4** — the scenario text format stays frozen. §7.1 resolves the tension with the oracle corpus: one
  optional directive the game path refuses.
- **LD 5** — `SimState::new`'s signature does not change; every new setting is a post-`new` assignment
  (`state.rs:1272-1288`).
- **LD 6** — the match seed seeds the sim `Rand`; `MatchConfig.seed` carries it.
- **LD 8** — persistence mirrors the C++ TOML schema byte-for-byte; `UpdateHash` is ported.
- **LD 9** — the two live bugs are folded in.
- **Standing gates** — every golden re-diffed per slice; sim-core stays dependency-free; Bevy stays in `game`.

---

## 1. What reaches the sim — the C++ facts 4½a is built on

### 1.1 Settings fields read by `Game::ProcessFrame`, `StartGame` and worm start

Established by grepping every `settings->` / `settings.` read under `src/game/` and following each into the
frame or the match start:

| Field | C++ readers | Sim-reaching? | Rust today |
|---|---|---|---|
| `game_mode` | `game.cpp:372`, `:521-541`, `:557`, `:571`, `:594`; `worm.cpp:215-216`, `:384`, `:396`, `:794` | yes | `SimState.game_mode` (post-`new`) |
| `time_to_lose` | `game.cpp:385`, `:531`, `:537` | yes | `SimState.time_to_lose` |
| `zone_timeout` | `game.cpp:427-432`, `:499` (Holdazone only) | Holdazone only | — (Holdazone refused) |
| `max_bonuses` | `game.cpp:219`, `:359` | yes | `settings_max_bonuses` |
| `weap_table` | `game.cpp:258` (bonus reject loop); `weapsel.cpp` (4½c) | yes | `weap_table: Vec<i32>`, never populated |
| `blood` | `nobject.cpp:188`, `sobject.cpp:96`, `weapon.cpp:301`, `worm.cpp:410` | yes | `blood` (`new` arg) |
| `shadow` | **`CorrectShadow`** at `nobject.cpp:123`, `:215`, `sobject.cpp:212`, `weapon.cpp:121`, `worm.cpp:784`, `:932`, `:942` | **yes** (writes `material_id`) | **unported** — every site "omitted (O4)" |
| `load_change` | `worm.cpp:1079` | yes | `load_change` (`new` arg) |
| `loading_time` | `weapon.cpp:8-14` (`ComputedLoadingTime`) | yes | `settings_loading_time` (`new` arg) |
| `lives` | `game.cpp:159` (`ResetWorms`), `localController.cpp:234` (enter game) | match start only | `WormInit.lives` |
| `blood_particle_max` | `game.cpp:513` (`StartGame` pool size) | match start (pool cap) | `BLOOD_CAPACITY = 700` const |
| `WormSettings.health` | `worm.cpp:213`, `:292-296`, `:355`, `:384-386`, `:795`; `game.cpp:158`, `:558-563`, `:607`; `localController.cpp:35`, `:42` | yes, **per worm** | one scalar `SimState.settings_health` |
| `WormSettings.weapons` | `worm.cpp:704` (`InitWeapons`) | match start | `WormInit::resolve_weapons` (`state.rs:240-256`) |
| `WormSettings.controller` | `localController.cpp:38`, `:45` (`CreateAi`) | via AI input (4½f) | — |
| `controls`/`controls_ex`/`input_device` | `game.cpp:63-96` (key → control) | via input (4½f) | — |

Hash-only or shell-only (no sim reader anywhere): `flags_to_win` (menus + `net/session.cpp` only),
`bonus_timeout` (declared, **never read**), `input_delay` (netplay), `names_on_bonuses` / `map` /
`allow_viewing_spawn_point` (render), `random_level` / `level_file` / `random_map_*` / `regenerate_level`
(level preparation, 4½b/d), `select_bot_weapons` (4½c), `ai_*` (FollowAI, deferred), `record_replays`,
`load_powerlevel_palette` (palette only, `level.cpp:281`), `tc`, and all `AppSettings` except
`blood_particle_max`.

### 1.2 The match start (`LocalController`)

`LocalController`'s constructor (`localController.cpp:30-54`) creates two fresh worms (`visible=false`,
`killed_timer=150`, `pos=(0,0)` — `worm.hpp:178-183`, `rect.hpp:10-11`), sets `health = ws.health`,
`stats_x = 0 / 218`. Weapon selection's `Finalize` runs `InitWeapons` (`weapsel.cpp:352-356`,
`worm.cpp:698-709`); entering the game sets `lives = settings->lives` (`localController.cpp:232-235`) and
calls `StartGame` (`:276`, `game.cpp:511-519`: pool resize + Holdazone `SpawnZone`). `LocalController` never
calls `ResetWorms`, but the state it produces is exactly `ResetWorms`' (`game.cpp:155-166`), which is what
the dumper uses (§7.2). This is also exactly the Rust fuzz goldens' "seeded dead at (0,0)" shape
(`sim_slice6_fuzz.rs:129-238`) — which is why §7.4 works.

### 1.3 Findings that contradict or sharpen the overview and the maps

1. **`shadow` is sim-reaching.** rust-map §5 lists it as "draw-time only". It is not: `CorrectShadow`
   (`blit.cpp:624-639`) rewrites `material_id` (via `Level::SetPixel`, `level.hpp:72-79`) at seven call sites
   inside the frame. The oracle dumper forces `settings->shadow = false` (`sim_physics_dump.cpp:375-381`) and
   the Rust sim omits every site. The C++ **default is `true`** (`settings.hpp:74`), so any real match
   diverges at its first crater without a port. §5.1.
2. **The Scales-of-Justice "extra-life rule" is half ported, and the other half is not where the overview
   points.** `DoHealingDirect`'s overflow-to-lives (`game.cpp:555-565`) is live in Rust
   (`bonus.rs:417-428`, golden `sim_slice6_scales`). Unported: the Scales **death** branch
   (`worm.cpp:384-388`, `while (health <= 0) { health += settings->health; --lives; }` — Rust always
   `lives -= 1`, `state.rs:2892-2897`), the Scales **respawn** guard (`worm.cpp:794-796`, no health restore
   in Scales — Rust always restores, `state.rs:2740-2741`), and **`DoHealing`** (`game.cpp:591-609`): the
   bonus pickup calls `DoHealing`, whose Scales arm *damages the other worms* — Rust calls
   `do_healing_direct` (`bonus.rs:517`). All three are reachable in a real Scales match. §5.2.
3. **The GameOfTag "it" guard is unobservable with two worms.** `state.rs:2900-2904` records it as a
   divergent unported path. Enumerating the guard (`worm.cpp:396-398`) over two worms shows the guarded and
   unguarded assignments always agree (a kill that leaves "it" unchanged needs a killer that is neither the
   victim nor "it" — a third worm). Ported anyway for fidelity (it costs four lines), §5.2.
4. **Health is per worm in C++, one scalar in Rust.** Every health read in the frame goes through
   `w.settings->health` (table above). The Rust sim carries one `settings_health`. A match with different
   player healths is not representable; the builder refuses it (§4.3) and the sim refactor lands with the
   player-menu HEALTH item in 4½f (§11 Q2).
5. **`flags_to_win`, `bonus_timeout` and `input_delay` have no sim reader** (§1.1). The overview's risk list
   names them as "missing sim fields"; they are persistence + hash only.
6. **The shipped settings files are legacy formats, not current C++ output.** `data/Setups/liero.cfg` and
   `orbmit.cfg` say `version = 5` (current `kConfigVersion` is 6, `settings.hpp:98`), carry 6-bit RGB with no
   `rgbDepth` marker (so load expands them, `cereal_types.hpp:288-303`), lack `maxSpectatorRenderHeight`, and
   end in `\n\n` (the current writer ends in one `\n`, §3.3). The eight `data/Profiles/*.toml` are
   hand-written: insertion order, double-quoted strings, `[20, 20, 40]` without inner spaces, no trailing
   newline, no `rgbDepth`. **C++ itself rewrites all ten on load + save**, so overview done-when 5 /
   Hard gate 5's "shipped file round-trips byte-identical" is impossible as worded; §3.4 restates it.
   (`orbmit.cfg` is a second shipped setup the maps do not mention.)
7. **The post-mortem keeps simulating.** `LocalController::Process` runs `ProcessFrame` while the state is
   `kStateGame` **or** `kStateGameEnded` (`localController.cpp:153-155`); `IsGameOver` flips to
   `GameEnded`, which sets `fade_value = 180` (`:277-282`); each `Process` then decrements it and returns
   `false` on the call that finds it at 0 (`:185-194`). Net: **exactly 180 further `ProcessFrame` calls**
   after the game-over frame. §6.
8. **Five TC weapons hit deferred Step-2 sim branches.** RIFLE, WINCHESTER, LASER and GAUSS GUN are
   `shotType = 4` (the laser do-loop, `debug_assert!`ed deferred at `weapon.rs:343-346`); MISSILE is
   `shotType = 2` whose worm-side `ProcessSteerables` is unported (PROGRESS "steerable_sum … real DEFER").
   Today they are reachable only through a random weapon bonus; once players choose loadouts (4½c) they are
   one menu press away — a debug-build panic or a silent divergence. 4½a-1's goldens ban them
   (`weap_table = 2`, not in any loadout); the port is §11 Q1 (**John**).
9. **`weap_table` indexing.** It is indexed by *weapon index* (`common.weapons` order = `tc.cfg [types]
   weapons`), not `weap_order`; `CreateBonus` rejects `== 2` only (`game.cpp:256-258`), so `1`
   ("bonus only") is sim-inert until weapon selection (4½c).

---

## 2. Settings model

### 2.1 Types

`rust/scenario/src/settings.rs` (pure data, no I/O):

- `WormSettings` — `health: i32`, `controller: u32`, `controls: [u32; 7]`, `controls_ex: [u32; 8]`,
  `gamepad_controls: [u32; 8]`, `input_device: u32`, `gamepad_name`, `gamepad_serial`, `weapons: [u32; 5]`,
  `name`, `rgb: [i32; 3]`, `random_name: bool`, `color: i32`. `profile_node` (a filesystem handle) and `hash`
  (a cache) are runtime state, not data.
- `Settings` — every `GameplayExtensions` / `AppSettings` / `Settings` field in C++ declaration order,
  `weap_table: [u32; 40]`, `worm_settings: [WormSettings; 3]` (0 left, 1 right, 2 network).
- `MatchConfig { settings: Settings, seed: u32 }` — what the builder consumes. The seed is not a C++
  setting (C++ single-player seeds `game.rand` implicitly); LD 6 makes it explicit.
- Constants mirroring C++: `SELECTABLE_WEAPONS`, `WEAP_TABLE_LEN`, `NUM_WORM_SETTINGS`,
  `NETWORK_PLAYER_IDX`, `CONFIG_VERSION = 6`, `GM_KILL_EM_ALL..GM_SCALES_OF_JUSTICE`, and
  `DEFAULT_GAMEPAD_CONTROLS = [11, 12, 13, 14, 110, 10, 0, 9]` (the SDL3 enum values
  `InitDefaultGamepadControls` stores, `worm.cpp:21-32`; every shipped file carries exactly this array).

**Widths are C++ widths.** `int`/`int32_t` → `i32`, `uint32_t` → `u32`, so the reader's
`static_cast<int32_t>(int64)` truncation (`toml_archive.hpp:224-235`) is `v as i32` / `v as u32`.

**Defaults.** `WormSettings::default()` is the bare C++ constructor (rgb `{104,104,248}`, weapons `1`×5,
controls zero, `random_name = true`); `Settings::default()` is `Settings::Settings()` (colours 32/41/32, the
DOS-scancode controls `{0x13,…}` / `{0xA0,…}` into both `controls` and the first seven `controls_ex`,
RGB `{104,104,252}` / `{60,172,60}`, the network player cloned from the left player). A strong check falls
out of finding 6: **the shipped `liero.cfg`, read with the C++ semantics, is exactly `Settings::default()`**
(its 6-bit `26,26,63` / `15,43,15` expand to `104,104,252` / `60,172,60`; its missing
`maxSpectatorRenderHeight` keeps 1080). 4½a-1 pins that as a test.

### 2.2 Location — the `scenario` crate (overview Open Q2, decided)

`scenario` already owns the only `SimState` builder (`loader.rs:100-218`) and the `read_asset` seam; the
settings modules have no Bevy, no sim and no render dependency of their own. Estimated 4½a footprint in
`scenario`: model ~200, reader ~250, writer ~250, storage ~200, builder ~250 lines of non-test code — below
the overview's ~1 k split threshold **for the settings part** (model + TOML + storage ≈ 900). Rule: if the
settings modules (`settings`, `settings_toml`, `storage`) exceed ~1 k non-test lines at the end of 4½a-2,
move them mechanically into a new `rust/settings` crate in 4½e (the builder stays in `scenario`).

`TC_ROOT` centralisation (overview Open Q2): `scenario::paths::{DATA_ROOT, TC_ROOT}` in **4½a-2**, where
`DATA_ROOT` is genuinely needed (the read-only system layer, §3.5). Production users switch
(`game/src/main.rs:36`, `shot/src/lib.rs:392-396`, `game/src/input.rs:791`, `scenario/src/loader.rs:225`);
the ~45 per-test `TC_ROOT` consts in `oracle-tests`/`sim` tests stay — a golden test's self-contained setup
is a feature, and touching 45 goldens' harnesses buys nothing.

---

## 3. TOML persistence

### 3.1 What C++ actually writes

`Settings::ToToml` (`settings.cpp:103-131`) drives a cereal `TomlOutputArchive` (`toml_archive.hpp`) that
builds a `toml::table` tree and, in its destructor, streams `out_ << root_ << "\n"` (`:47`). The bytes are
therefore **toml++ 3.4.0's `toml_formatter` output** (vcpkg `tomlplusplus` v3.4.0) plus one `"\n"`:

- **Keys are sorted.** `toml::table` is a `std::map<toml::key, …, std::less<>>`, so both the table headers
  (`network_player`, `player1`, `player2`, `settings`) and the keys inside each table come out in byte order
  — the `make_nvp` order in `cereal_types.hpp` is irrelevant to the bytes. The shipped files confirm it.
- **Layout** (`toml_formatter.inl:238-373`, `formatter.inl:82-113`): a root holding only sub-tables prints
  `[name]` headers, `key = value` lines, a blank line between tables, and **no newline after the last
  value** (the archive's `"\n"` is the only terminator). A root holding only key/values (a profile,
  `WormSettings::ToToml`, `worm.cpp:45-52`; the `UpdateHash` subset, `settings.cpp:92-101`) prints the
  key/value lines with no leading newline.
- **Arrays** (`toml_formatter.inl:174-235`): inline `[ 1, 2, 3 ]` (inner spaces), unless the estimated width
  `3 + Σ(len(elem) + 2)` plus the indent bias (0 at our nesting) is **`>= 120`**
  (`toml_formatter_forces_multiline`, `:109-113`) — then one element per line, 4-space indent, `,` after all
  but the last, `]` on its own line. `weapTable` (40 entries ⇒ ≥ 123) is always multiline; every worm array
  (≤ 8 × 10 digits ⇒ ≤ 99) is always inline.
- **Integers** decimal with `-`; **booleans** `true`/`false`; `uint32_t` values are stored as `int64`
  (`toml_archive.hpp:73`), so they are never negative.
- **Strings** (`formatter.inl:116-358`, value context: multi-line allowed, bare not allowed, literal
  whitespace allowed): empty ⇒ `''`; otherwise a literal `'…'` unless the string contains a `'` (without a
  newline), a control character (`<= 0x1F` except tab/newline, `0x7F`, or U+0085/U+2028/U+2029), then a basic
  `"…"` with `\"`, `\\`, `\u007F`, the `control_char_escapes` table (`\b \t \n \f \r`, else `\u00XX`,
  `forward_declarations.hpp:127-160`); a string with a newline and no other blocker is `'''…'''`.
  Real tabs stay raw. Non-ASCII UTF-8 stays raw (unicode allowed by `toml_formatter::default_flags`,
  `toml_formatter.hpp:83-91`).
- **Content.** `[settings]` holds `version = 6` (always `kConfigVersion`; the read value is discarded,
  `settings.cpp:139-140`), `modernColors`, the scalars (`cereal_types.hpp:161-194`) and `weapTable`
  (`:51-60`). Each worm table holds `SerializeWormSettingsToml`'s keys **including `rgbDepth = 8`** on save
  (`:288-295`).

### 3.2 Reader (4½a-1) — the `toml` crate + the `TomlInputArchive` semantics by hand

**Decision:** parse with the `toml` 0.8 crate already in the workspace (`assets/Cargo.toml`, `Cargo.lock`
4605) into a generic `toml::Table`, then walk it by hand mirroring `TomlInputArchive` exactly. Serde derive
is rejected: the C++ semantics are not serde's — every one of these is a deliberate `TomlInputArchive`
behaviour the reader reproduces:

| Case | C++ (`toml_archive.hpp`) | Rust reader |
|---|---|---|
| missing key | `Lookup` ⇒ null ⇒ value untouched (`:281-309`, `:217-268`) | keep the prior value |
| wrong type (`lives = "x"`, `shadow = 1`) | `is_integer()`/`is_boolean()` false ⇒ untouched | keep |
| int to `int32_t`/`uint32_t` | `static_cast` truncation (`:224-235`) | `v as i32` / `v as u32` |
| array element | positional, `f.index` advances even past a wrong-typed element (`:299-316`) | element `i` if present and integer, else keep |
| short / long array | missing tail kept / extra ignored (`SerializeArray` loops `N`, `cereal_types.hpp:51-60`) | same |
| non-array where an array is expected | `Lookup` on a non-table/non-array ⇒ null | keep all |
| missing / scalar `[player1]` | child frame null ⇒ every load untouched | treat as an empty table |
| array-valued `settings` / `playerN` | `startNode` keeps the array node; `Lookup` on an array frame ignores the name and returns slot `index` (`toml_archive.hpp:177-202`, `:299-306`) ⇒ fields read **by position** in serialization order | read positionally (corrected 2026-09-10 by the 4½a-1 T2 review; 4½a-2's G5 golden should include one such input to confirm against the real C++) |
| `rgbDepth` | default **6** on load, `< 8` ⇒ `v = (v & 63) << 2`, then clamp 0..255 (`:288-303`) | same — **including** on a missing table, where it mangles the *defaults* (`104 ⇒ 160`): a C++ quirk, reproduced and tested |
| `version` | read into a local, discarded | ignored |
| parse error | `TomlParseError` (`:162-167`) ⇒ `Settings::load` false (`settings.cpp:76-78`) | `Err(TomlError)`, value untouched |
| profile load | `LoadProfile` restores the pre-load `color` (`worm.cpp:73-95`) | `load_profile` restores `color` |

Parser-acceptance differences between toml++ 3.4 and `toml` 0.8 on exotic inputs (both are TOML 1.0) are
out of the gate's reach and irrelevant to files C++ writes.

### 3.3 Writer (4½a-2) — a hand-rolled port of the toml++ formatter subset

**Decision:** a small hand-rolled writer (`BTreeMap<&str, Val>` for byte-ordered keys; `Val` =
integer / bool / string / integer array; the §3.1 layout, array and string rules), not `toml_edit` /
`toml::to_string`. Every Rust TOML serializer disagrees with toml++ on at least one of: key order (insertion
vs sorted), array inner spaces, the multiline threshold, literal vs basic quoting, the trailing newline. The
formatter subset we need is ~150 lines of direct transcription of `toml_formatter.inl`/`formatter.inl`, and
the gate (SAVED BYTES) is checked against C++ output, so a faithful transcription is the least-risk path.
No new dependency.

### 3.4 The byte gate, restated (finding 6)

Because the shipped files are not current C++ output, "shipped file → Rust → same bytes" cannot hold (C++
fails it too). The gate becomes three checks, all against a **C++ oracle**:

- **G5a — load + save parity.** For every input `I` (shipped setups and profiles, the three 4½a-1 setup
  sidecars, a synthetic corpus: quoting — `'`, `"`, `\`, tab, `é`, U+2028, `\u0001`, a newline; edge —
  missing keys, short/long arrays, wrong types, `rgbDepth = 7`, out-of-range ints, a missing `[player2]`):
  `rust_save(rust_load(I)) == cpp_save(cpp_load(I))` byte-for-byte. This proves the reader *and* the writer
  on the real inputs, legacy expansion included.
- **G5b — C++-saved round trip** (the overview's wording, now meaningful): for every C++ output `O`,
  `rust_save(rust_load(O)) == O`.
- **G5c — hashes.** `gameplay_toml(rust_load(I))` equals the C++ `SerializeGameplay` bytes and
  `update_hash` equals `Settings::UpdateHash()`; same for profiles and `WormSettings::UpdateHash()`.

The oracle is a new `oracle_dump_settings` tool (4½a-2, §9.2) that runs the real `Settings::FromToml` /
`ToToml` / `UpdateHash` and `WormSettings::LoadProfile` / `SaveProfile` / `UpdateHash` over the inputs and
writes the outputs as committed goldens. `test_settings.cpp` stays the C++ field-coverage reference.

`UpdateHash` = XXH3-64 (seed 0) over the `SerializeGameplay` TOML (`settings.cpp:92-101`, subset
`cereal_types.hpp:218-239`: GameplayExtensions + the gameplay scalars + `weapTable` + `bonusTimeout` +
`inputDelay`, rendered at root, sorted, `+ "\n"`). **Crate: `twox-hash` 2.1.3** with
`default-features = false, features = ["xxhash3_64"]` — pure Rust, no dependencies in that configuration,
`no_std`-capable, and wasm-safe (its SIMD dispatch is gated on `target_arch = "aarch64" | "x86_64"` plus
`std`; wasm32 takes the scalar path — verified in the crate source, `src/xxhash3/large.rs`). It is already
in the local cargo registry. API: `twox_hash::XxHash3_64::oneshot(&[u8]) -> u64`. Known-answer vector:
`XXH3_64("") = 0x2d06_8005_38d3_94c2` (the crate's own test, `xxhash3_64.rs:382`; the C++ tool prints the
same line as a cross-check). `xxhash-rust` would be equally valid; `twox-hash` wins on being already
vetted offline.

### 3.5 Storage (4½a-2) — the `ConfigStore` seam

```rust
pub trait ConfigStore {
    fn read(&self, rel: &str) -> Option<Vec<u8>>;          // merged: user layer, then system layer
    fn write(&self, rel: &str, bytes: &[u8]) -> std::io::Result<()>; // user layer only
    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool;       // paths::ShadowsSystem
}
```

- `NativeStore::split(user_root, Some(system_root))` — the XDG split (`filesystem.cpp:833-845`); reads see
  the user file if present, else the system file; writes go to the user dir (parent dirs created).
- `NativeStore::single_dir(root)` — `--config-root` / portable semantics (`filesystem.cpp:811-816`,
  `:827-831`): one directory, no system layer, `shadows_system` only honours the reserved names.
- `NativeStore::resolve_default()` — user root = a `pref_path("openliero", "openliero")` mirroring
  `SDL_GetPrefPath` (macOS `~/Library/Application Support/openliero/openliero/`, Linux/BSD
  `$XDG_DATA_HOME` or `~/.local/share` + `/openliero/openliero/`, Windows `%APPDATA%\openliero\openliero\`),
  with the C++ test-only `OPENLIERO_TEST_USER_DIR` override (`filesystem.cpp:658-667`); system root =
  `scenario::paths::DATA_ROOT` (the repo's `data/`, the Rust analog of `SystemDataRoot`'s binary-adjacent
  data). Hand-rolled from environment variables — no `dirs` crate.
- `MemoryStore` — an in-memory map for unit tests and the wasm placeholder until 4½h's `LocalStorageStore`.
- `load_setup(&dyn ConfigStore)` mirrors `gameEntry.cpp:55-58` (read `Setups/liero.cfg`; missing or
  unparseable ⇒ defaults **and** write them to the user layer); `save_setup` mirrors `:78`.
- `shadows_system` mirrors `filesystem.cpp:736-763`: the reserved `Setups/liero.cfg` (case-insensitive),
  else "the system layer has this file", skipped in single-dir mode.

Deferred: `portable.txt` detection (there is no Rust install layout yet), a `--config-root` CLI flag, and
directory listing (4½e's file pickers).

---

## 4. The builder — `MatchConfig → SimState`

### 4.1 Shape

```rust
pub fn build_match(tc_root: &Path, cfg: &MatchConfig, level: &LevelData) -> Result<Loaded, BuildError>;
pub fn validate(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError>;
```

It returns the same `Loaded { state, viewports, scene }` that `scenario::load` returns, so `game` and `shot`
can render a built match with no new plumbing. The level is a parameter: **level preparation is not the
builder's job.** C++ `GenerateFromSettings` (`level.cpp:397-428`) chooses random-vs-file and applies
`MakeShadow` when `shadow` is on — to file levels too — and that is 4½b's surface:
`sim::levelgen::generate_from_settings(&LevelGenAssets, &LevelGenParams, file: Option<LevelData>,
&mut Rand) -> LevelData` (4½b design §6, the C++ name, controller-adjudicated). 4½a does **not** call it
and does **not** depend on 4½b landing first (the two run in parallel): `build_match` takes a ready
`LevelData`, and 4½d's NEW GAME composes `generate_from_settings` (4½b) then `build_match` (4½a), mapping
`MatchConfig` (`random_level`, `random_map_width/height`, `shadow`, `level_file`) onto `LevelGenParams` and
seeding the level `Rand` from `MatchConfig.seed`. The 4½a-1 goldens load a fixed 504×350 level on both
sides, exactly as every prior dumper golden does (`sim_physics_dump.cpp:409-419`); a sim golden on a
generated non-504×350 level is assigned to 4½e.

**Palette (new bug, folded in).** `scenario::load` ignores `LevelData.palette` and always uses
`small.tga`'s palette (`loader.rs:101-104`), so a level's POWERLEVEL palette never shows. C++ `Level::load`
adopts the POWERLEVEL palette only when `settings.load_powerlevel_palette` (`level.cpp:281-294`), otherwise
resets to `common.exepal` (`:385-392`), and `Game::UpdateSettings` makes the level palette the renderer's
(`game.cpp:476-479`). The builder therefore sets `scene.origpal = level.palette` iff
`settings.load_powerlevel_palette && level.palette.is_some()`, else the `small.tga` palette (= `exepal`).
(`assets::level::load` always parses the block, so the flag gate must live in the builder.) Render-only —
no sim hash moves. `scenario::load` keeps its historical behaviour (its render goldens depend on it); the
live path gets the fix when 4½d switches it to the builder.

### 4.2 Field mapping (the `LocalController` start state, §1.2)

| `SimState` | from | C++ |
|---|---|---|
| `worms[i]` via `WormInit { index: i, health: ws[i].health, lives: s.lives, stats_x: 0/218, weapons: resolve_weapons(ws[i].weapons), start_pos: (0,0), visible: false }` | per worm | `localController.cpp:33-45`, `:234`; `worm.cpp:698-709` |
| `SimState::new(level, …, cfg.seed, …, s.loading_time, s.load_change, s.blood)` | seed + three `new` args | LD 5, LD 6 |
| `settings_max_bonuses`, `weap_table` (40 entries, `as i32`), `game_mode`, `time_to_lose`, `shadow` (new) | settings | §1.1 |
| `settings_health = ws[0].health` (both equal, §4.3) | worm settings | finding 4 |
| `bobjects = BloodPool::new(s.blood_particle_max)` | settings | `game.cpp:513` |
| `sound_hooks = tc.sound_hooks` | TC | bug fix, §8 |
| blood / spawn / bonus-drop / `CreateBonus` / `Bonus::Process` / pickup consts, `small_sprites`, `laser_weapon` | TC | the full post-`new` set `sim_slice6_fuzz.rs:199-236` assigns, plus `laser_weapon` (unhashed) |
| `scene.origpal` = level POWERLEVEL palette iff `load_powerlevel_palette`, else `small.tga` palette | level + settings | `level.cpp:281-294`, `:385-392`; `game.cpp:476-479` (render-only) |

`controller`, `controls`, `input_device`, names and colours are not `SimState` inputs; they are carried in
`MatchConfig` for the input layer (4½f) and the renderer.

### 4.3 Validation and refusals (`BuildError`)

Refused, never panicked on — a menu can produce any of these:

- `HoldazoneUnsupported` — `game_mode == 2` (the sim arm is `unimplemented!()`, `state.rs:2250-2252`).
- `InvalidGameMode(m)` — `m > 3`.
- `InvalidHealth(h)` — `h < 1` for player 0 or 1 (C++ would divide by it in the HUD and loop forever in the
  Scales death branch).
- `AsymmetricHealth { p1, p2 }` — the one-scalar sim limitation (finding 4); lifted in 4½f.
- `InvalidWeapon { worm, slot, value }` — `value == 0 || value > weap_order.len()` (C++ indexes
  `weap_order[value - 1]` unchecked).
- `InvalidBloodParticleMax(n)` — `n < 1` (a zero-cap pool underflows `spawn_reuse`).
- `TooManyWeapons(n)` — a TC with more than 40 weapons overruns `weap_table[40]`.

The network player (index 2) is not validated: it never plays in a local match.

### 4.4 Seams

- **Weapon selection (4½c).** The builder applies `InitWeapons` from the saved picks. 4½c inserts the
  weapsel phase between "worms created" and "enter game" (it draws `game.rand`); the builder then splits
  into `new_match` (constructor state) and `enter_game` (InitWeapons + lives + pool), which 4½c calls from
  `Finalize`. Not split in 4½a — nothing needs the halves yet.
- **The default match.** The live binary keeps launching `game/scenarios/default_match.txt` through
  `scenario::load` in 4½a (decided). The builder cannot reproduce it (fixed visible positions and a slot-0
  DART override are not settings) and does not need to: 4½d replaces it with the menu. Keeping it also keeps
  `--record` honest.
- **Recording a built match.** A `MatchConfig` match is not representable in the frozen scenario grammar, so
  4b's recorder cannot capture one. 4½d decides (recommendation: the recorder writes the scenario plus a
  setup sidecar, i.e. §7.1's directive promoted from oracle-only to recording format).

---

## 5. Sim completion

### 5.1 `CorrectShadow` (finding 1)

**Stable public signature (4½b's T9 calls it):**
`pub fn sim::shadow::correct_shadow(level: &mut sim::state::LevelSim, x1: i32, y1: i32, x2: i32, y2: i32)`
— the C++ `Rect(x1, y1, x2, y2)` as four half-open bounds, always applied (no flag). 4½b's golden has a
function-level `CorrectShadow` oracle (its "dig stage", 4½b design §10.4) and its T9, scheduled right after
this port lands, checks the `shadow=1` dig tokens through exactly this function. Rust has no `CorrectShadow`
anywhere today (O4 omitted all seven sites); 4½a owns the port **and** the wiring; `MakeShadow` is 4½b's
(`sim::levelgen::make_shadow`, a separate file, so the two slices never touch the same module).

`sim/src/shadow.rs`: `correct_shadow(level, x1, y1, x2, y2)` is a line-for-line port of `blit.cpp:624-639`
(intersect with `(0, 3, width-3, height)`, x-outer/y-inner, `SeeShadow` pixel under a `DirtRock` at
`(x+3, y-3)` ⇒ `+4`, pixels 164..167 not under one ⇒ `-4`, `u8` wrapping as C++ `PalIdx`). A new unhashed
`SimState.shadow: bool` (post-`new`, default `false` ⇒ every golden byte-identical) gates it.

**Plumbing: a per-tick thread-local flag, not a parameter.** `process_frame` publishes `self.shadow` at the
top (`shadow::begin_frame`) and clears it at the bottom (`end_frame`); the seven sites call
`correct_shadow_if_enabled`. Threading a `shadow: bool` parameter instead would ripple through
`sobject_create`'s fan-in (called from weapon, nobject, bonus and state code) and ~30 unit-test call sites.
The precedent is `sound::begin_frame`'s hook indices (`sound.rs:159-176`). The difference — this flag *does*
change sim results — is safe because the flag is re-published from the `SimState` field at the top of every
tick and cleared at the bottom, so inside a tick it always equals `self.shadow`, never leaks between states
or threads, and a direct unit-test call outside `process_frame` sees `false` (today's behaviour). Sites:
`nobject.rs:481-489` (`ipos±(8,9)`), `nobject.rs:654-664` (`pos±(10,11)`), `sobject.rs:395-405`,
`weapon.rs:804-814`, `control.rs:722-744` (twice, `dig±(3,18)`), `state.rs:2728-2730` (respawn).

### 5.2 Game-mode rules (finding 2, 3)

- `worm_death` gains `game_mode, settings_health`: Scales ⇒ `while health <= 0 { health += settings_health;
  lives -= 1 }`, else `lives -= 1`; the GameOfTag guard on the `last_killed_idx` assignment.
- `do_respawning` gains `game_mode`: no health restore in Scales.
- New `state::do_healing(worms, w_idx, amount, game_mode, settings_health)` = `game.cpp:591-609`
  (`do_healing_direct`, then Scales ⇒ split `amount` as `do_damage_direct` onto the others with the healer as
  `by_idx`, else clamp); `worm_pickup_bonuses` calls it. KillEmAll behaviour is unchanged (the clamp is
  already applied), so every prior golden is byte-identical.

### 5.3 `IsGameOver`

`sim/src/game_over.rs::is_game_over(&SimState) -> bool` — `game.cpp:521-544`: modes 0/3 ⇒ any worm
`lives <= 0`; modes 1/2 ⇒ any worm `timer >= time_to_lose` (Holdazone checks `time_to_lose`, not a
"time to win" — the cpp-map §7.1 quirk); anything else ⇒ `false`. It lives in `sim` because it is a pure
read of sim state, and it is **total** (no panic for Holdazone): the builder is the single refusal point,
and a total predicate is strictly safer than an error path nobody can reach.

---

## 6. Match lifecycle — `MatchFlow`

`game/src/match_flow.rs` (a `lib.rs` module: Bevy-free, headless-testable — rust-map §1's rule; the
`ScreenStack` of 4½d will own it):

```rust
pub enum MatchPhase { Game, GameEnded }
pub enum FlowStep { Continue, Finished }
pub struct MatchFlow { phase, fade_value: i32, going_to_menu: bool }
impl MatchFlow {
    pub fn new() -> Self;                                    // entering kStateGame from weapsel: fade 33
    pub fn after_frame(&mut self, state: &SimState) -> FlowStep;
    pub fn phase(&self) -> MatchPhase;  pub fn fade_value(&self) -> i32;
}
```

`after_frame` runs once per tick **after** `process_frame` and is the tail of `LocalController::Process`
(`localController.cpp:177-199`): the first game-over frame sets `GameEnded`, `fade_value = 180`,
`going_to_menu` (`:277-282`); then `going_to_menu` ⇒ decrement, or `Finished` on the call that finds 0;
otherwise the fade-in counts up to 33. Result: `Finished` is returned on the 181st call counting the
detection call — 180 more simulated frames, matching finding 7. `fade_value` is exposed for 4½d's render
fade (the C++ renderer's `fade_value`, `:211`); 4½a does not draw it.

Seams: 4½c adds a `WeaponSelection` phase in front (`ChangeState(kStateWeaponSelection)`, the 12/3 key
repeat); 4½d adds the Esc path (`OnKey` `kDkEscape`: `fade_value = 31`, `going_to_menu`, `:82-85`) and
`Focus()`; 4½g routes `Finished` to the stats screen.

**Live wiring (4½a-1).** In `Mode::Live` only, `tick_and_render` calls `after_frame` after the sim tick;
`Finished` restarts the match through the existing F5 path (the recorder is cleared exactly as F5 does).
Scripted (golden loop) and Replay (fixed `ticks`) are untouched.

---

## 7. Oracle

### 7.1 The frozen format vs the oracle corpus — one sidecar directive

The tension: LD 4 freezes the scenario grammar, but a settings→sim golden must tell **both** the C++ dumper
and the Rust side what the settings are, and the corpus files are parsed by both.

Rejected: one directive per setting (`loading_time`, `blood`, `shadow`, `weap_table` × 40, per-worm
weapons, per-worm health, …) — that grows a second settings language inside the scenario format, which is
exactly what LD 4 exists to prevent, and it would hand-map each field on the C++ side instead of exercising
the real C++ reader.

**Chosen: a single optional directive `settings <file>`** naming a C++-schema setup file (path relative to
the scenario file's directory). The C++ dumper feeds it to the **real** `Settings::FromToml`; the Rust
golden test feeds it to 4½a-1's reader and `build_match`. Consequences:

- Every existing scenario file parses to the identical value (absent ⇒ `None`, old behaviour); the frozen
  promise is honoured as "no existing file changes meaning".
- `scenario::load` (the game / `shot` path) **refuses** a scenario carrying `settings` — the directive is
  oracle-only, so the game path grows no settings language.
- The sim golden now also cross-checks the Rust reader against C++ on every sim-reaching field: a reader
  that drops `loadingTime` fails the golden.
- A `settings` scenario rejects `worm`, `weapon`, `game_mode`, `max_bonuses` and every `render*` directive
  (both parsers, so there is never an ambiguous precedence rule).

### 7.2 Dumper changes (`src/tools/oracle_dump/sim_physics_dump.cpp` only)

With `settings` present: load the file with `Settings::FromToml` instead of the hand-set overrides
(`:369-387`); refuse Holdazone (`exit 1`, mirroring the builder); create the two worms from
`worm_settings[0/1]` (`health = ws.health`, `stats_x` 0/218), `InitWeapons`, `ResetWorms` — and skip the
worm-line start conditions (`:437-464`); keep `bobjects.Resize(settings->blood_particle_max)` (now the
loaded value). Output gains a **12th column**, `Game::IsGameOver()` as `0`/`1`, only on this path — the
11-column format and every existing golden are untouched. The absent path is proven byte-identical by
regenerating `sim_slice6_fuzz5`, `sim_slice6_scales`, `render_slice4d_live` and `render_slice3e_hud` with the
modified dumper and checking `git status` stays clean.

### 7.3 The golden matrix — every sim-reaching field varied

Four scenarios, all on `Levels/modern_test.lev` (the fuzz arena), worms seeded dead at the C++ start state,
the 12-column golden asserted every tick:

| field | `defaults` | `killemall` | `scales` | `gametag` |
|---|---|---|---|---|
| setup source | shipped `data/Setups/liero.cfg` (legacy v5) | generated sidecar | generated sidecar | generated sidecar |
| `gameMode` | 0 | 0 | 3 | 1 |
| `lives` | 15 | 1 | 2 | 3 |
| worm `health` (both) | 100 | 150 | 40 (amended 2026-09-10 from 120: at 120 Scales trades lives forever and the match never ends) | 80 |
| `loadingTime` | 100 | 37 | 150 | 0 |
| `blood` | 100 | 250 | 60 | 0 |
| `loadChange` | true | false | true | true |
| `maxBonuses` | 4 | 6 | 8 | 3 |
| `shadow` | true | true | true | false |
| `timeToLose` | 600 | 600 | 600 | 12 |
| `bloodParticleMax` | 700 | 300 | 500 | 700 |
| `weapTable` | zeros | 5 bans (=2) + 3 bonus-only (=1) | same | same |
| worm `weapons` | `1`×5 each | distinct per worm | distinct | distinct |
| input | none, 400 ticks | fuzz, until game over + 200 | fuzz | fuzz |

The generated sidecars also set every *non*-sim field to a non-default value (names, RGB with
`rgbDepth = 8`, controls, gamepad, AI params, `map = false`, …), so the goldens prove those fields inert on
both sides, and they double as 4½a-2 byte-gate inputs. Loadouts use only weapons already covered by C++
goldens (BAZOOKA, DART, CANNON, GRENADE, HANDGUN, EXPLOSIVES, GREENBALL, FAN); the five deferred-branch
weapons are `weap_table = 2` so no bonus can hand one out (finding 8).

A generator (`oracle-tests/examples/gen_slice4_5a.rs`) writes the sidecars (resolving weapon names to
`weap_order` / weapon indices at run time), scans game seeds with a pre-expanded 7-bit random input stream
through the **Rust builder**, and keeps the first seed whose driven run shows the variant's witnesses:
game over by tick 2800 (so ≥ 200 post-mortem ticks follow); killemall — `CorrectShadow` fired (the level
column differs from a `shadow = false` re-run), the 300-cap blood pool filled, a reload started, a bonus
dropped; scales — a death that left `health > 0` (the Scales death branch) and a life gained; gametag — an
"it" timer bump. Seeds that trip a deferred-branch `debug_assert!` are skipped (`catch_unwind`). The same
witnesses are re-asserted from the driven state in the milestone test, so the goldens are non-vacuous.

### 7.4 The free cross-check — the builder reproduces `sim_slice6_fuzz5`

The slice-6 fuzz goldens were produced by the *old* dumper path with worms seeded dead at `(0,0)`,
`lives 50`, `max_bonuses 4`, `loading_time 0`, `shadow false`, default everything else — which is precisely a
`MatchConfig`. `build_match` with those settings and `seed 43` must reproduce `sim_slice6_fuzz5.txt` (1501
ticks, 11 columns) **before any C++ change**. This pins the builder's TC-const and worm-start mapping
against an existing C++ golden in the builder task itself.

### 7.5 Presentation — nothing new

4½a draws nothing new except the HUD in the live binary (4½a-2), which is already gated by the 3e
`render_hud` goldens; the Scripted demo (and the wasm parity witness) stay world-only (§8).

---

## 8. The two live bugs

- **`sound_hooks` (4½a-1).** `scenario::load` adds `state.sound_hooks = tc.sound_hooks.clone()` after the
  `game_mode` assignment (`loader.rs:182`); the builder does the same. Unhashed ⇒ every golden unchanged.
  Test: equality with the TC's hooks, and `Bump != 0` (the all-zero default was the bug).
- **HUD (4½a-2).** A pure `game::hud_mode::hud_flags(mode, scenario_hud, settings_map) -> (draw_hud, map)`:
  `Live`/`Replay` ⇒ `(true, settings.map)`; `Scripted` ⇒ `(scenario.hud(), scenario.hud())`, so the
  `blood` demo and its wasm frame-parity witness (`main.rs:716-725`) stay world-only.
  `render_and_upload` (`main.rs:743`) sets `scene.draw_hud` / `scene.map` from it; the binary loads the
  setup through `NativeStore::resolve_default()` (native) or uses `Settings::default()` (wasm).

---

## 9. Task outline

### 9.1 4½a-1 (plan: `plans/2026-09-10-liero-rs-step4.5-slice4.5a1-plan.md`)

| Task | Deliverable | Gate |
|---|---|---|
| T0 | `scenario::load` assigns `sound_hooks` | unit + re-diff |
| T1 | `settings.rs` model + C++ defaults | unit |
| T2 | `settings_toml.rs` reader (`settings_from_toml`, `load_profile`) | unit incl. shipped files |
| T3 | `CorrectShadow` + `SimState.shadow` + 7 sites (stable `correct_shadow` signature; 4½b's T9 follows) | unit + re-diff |
| T4 | Scales death/respawn, `do_healing`, GameOfTag guard | unit + re-diff |
| T5 | `sim::game_over::is_game_over` | unit |
| T6 | `build_match` + `validate` + scene factoring + level-palette rule | unit + **reproduces `sim_slice6_fuzz5`** |
| T7 | `settings` directive (Rust parser + C++ dumper, 12th column) + `defaults` smoke golden | parser unit + absent-path regen byte-identical |
| T8 | generator + 3 sidecars + 3 scenarios + C++ goldens | generator ledger |
| T9 | **MILESTONE** — 4 settings-driven goldens bit-exact incl. `IsGameOver` | oracle-tests |
| T10 | `MatchFlow` + live wiring | unit |
| T11 | full re-diff, wasm build, PROGRESS, overview status | CI commands |

### 9.2 4½a-2 (outline — its plan is written when 4½a-1 lands)

| Task | Deliverable |
|---|---|
| T0 | C++ `oracle_dump_settings` (`src/tools/oracle_dump/settings_dump.cpp` + one `add_executable` line in the `OPENLIERO_BUILD_ORACLE_DUMP` block of `CMakeLists.txt:372-391` — the only edit outside `src/tools/oracle_dump/`): modes `setup <in> <out.cfg> <out.gameplay.toml> <out.hash>`, `profile <in> <out.toml> <out.hash>`, `defaults <dir>` (the `Settings()` / default-profile bytes + `XXH3_64("")`); synthetic inputs; `gen_settings_golden.sh`; goldens under `rust/oracle-tests/golden/settings/` |
| T1 | writer: `settings_to_toml`, `worm_settings_to_toml`, `gameplay_toml` (§3.3), tested first against the `defaults` goldens (writer-only) |
| T2 | **the byte gate** G5a + G5b over every input (`oracle-tests/tests/settings_toml_golden.rs`) |
| T3 | `update_hash` / `worm_update_hash` (`twox-hash`) + KAT + G5c |
| T4 | `scenario::paths` (`DATA_ROOT`, `TC_ROOT`; production users switched) + `scenario::storage` (`ConfigStore`, `NativeStore`, `MemoryStore`, `pref_path`, `load_setup`, `save_setup`, `shadows_system`) with temp-dir tests |
| T5 | HUD fix (`hud_flags` + `render_and_upload`), the binary loads the setup through the store |
| T6 | **MILESTONE** — byte gate + hashes green; full re-diff; wasm build; PROGRESS |

### 9.3 4½a-2 plan-time decisions (2026-09-10)

Made while writing `plans/2026-09-10-liero-rs-step4.5-slice4.5a2-plan.md`. Each refines §3.3–§3.5, §8 or
§9.2; none changes a gate. (The plan splits §9.2's T6 into a MILESTONE task and a final re-diff task.)

1. **The oracle is corpus-driven.** One CLI, `oracle_dump_settings <corpus.txt> <out-dir>`, replaces §9.2's
   three modes. `golden/settings/corpus.txt` lists `default-setup <id>`, `default-profile <id>`,
   `setup <id> <path>` and `profile <id> <path>` lines (paths repo-root-relative, spaces allowed) and is
   read by **both** the tool and the Rust test, so the corpus has one source of truth. Outputs:
   `<id>.cfg` + `<id>.gameplay.toml` per setup, `<id>.toml` per profile, and one `hashes.txt`
   (`xxh3-empty - <hash>` first, then `<kind> <id> <hash>` per entry). The tool runs the real
   `Settings::load`/`save`, `WormSettings::LoadProfile`/`SaveProfile` and both `UpdateHash`es and
   self-checks that each `UpdateHash` is XXH3-64 of the bytes it wrote and that C++ load + save of its
   own output reproduces it (the premise of G5b). Profiles load over a bare `WormSettings()` (colour 0);
   the Rust test does the same. CMake links `game cereal::cereal`: `game` does not export cereal.
2. **Synthetic corpus** (`golden/settings/in/`): `quoting.cfg` + `quoting_profile.toml` (one string per
   §3.1 rule), `edge.cfg` + `edge_profile.toml` (the §3.2 table: wrong types, narrowing, short / long /
   empty arrays, `rgbDepth = 7`, clamping, a missing `[player2]`), `missing_tables.cfg` (the default-rgb
   quirk and a 41-entry `weapTable`) and `array_player.cfg` (§3.2's array-valued tables, confirmed
   against the real C++). Kept out: a multi-line string that starts with a newline (TOML drops it on
   read, so not even C++ round-trips it) and `'''` inside a multi-line literal (toml++ writes invalid
   TOML). Both sides would write the same bytes; the tool's idempotence check rejects either input.
3. **Formatter facts pinned from the toml++ 3.4.0 source** (vcpkg `buildtrees/tomlplusplus/src/
   v3.4.0-52dc53a92a.clean/include/toml++/impl/`): keys byte-sorted (`table.hpp:224`, a
   `std::map<key, …, std::less<>>`); `print_newline` prints nothing when nothing was printed since the
   last newline, hence no leading newline (`formatter.inl:82-89`); the table separator is two forced
   newlines (`toml_formatter.inl:120-128`); `indentation` indents array elements by `"    "` while
   headers and keys stay at column 0 at our nesting (`:238-373`); an array wraps when
   `3 + Σ(width + 2)` reaches 120 (`:27-113`, `:174-235`; integer width = digit count plus a sign —
   C++ uses `log10(double)`, exact below 2^53); strings as §3.1: literal iff no control character and
   (no `'` or a newline), multi-line iff a newline; control = `<= 0x1F` except tab and newline, `0x7F`,
   U+0085/U+2028/U+2029 (`unicode.hpp:97-106`, `unicode_autogenerated.hpp:57-60`); basic-string escapes
   `\"`, `\\`, `\u007F`, the `control_char_escapes` table with **uppercase** hex
   (`forward_declarations.hpp:127-160`), `\uXXXX` for the three vertical spaces, tab and newline raw;
   integers always decimal (archive-created values carry `value_flags::none`, `value.hpp:242`).
   **Reachability:** through `Settings`, `weapTable` (40 entries, ≥ 123 columns) is always multiline
   and every worm array (≤ 8 × 10 digits, ≤ 99 columns) always inline; the 117/120 edge is a unit test
   of the port only.
4. **Modules.** The toml++ port is a private `scenario::toml_fmt`; `settings_to_toml`,
   `worm_settings_to_toml`, `gameplay_toml`, `update_hash` and `worm_update_hash` join the reader in
   `settings_toml.rs`.
5. **`twox-hash` is pinned `=2.1.3`** (`default-features = false, features = ["xxhash3_64"]`, in
   `scenario` only). It is not in `Cargo.lock` yet; the local cargo cache holds the 2.1.3 `.crate` and
   its index entry (2.1.3 is the newest version the cache knows), so the exact pin resolves offline and
   keeps the vetted source. `oracle-tests` compares against the C++ vectors and needs no dependency.
6. **HUD: the binary passes `Settings::default().map` and does no config I/O in 4½a-2.** §8's "the
   binary loads the setup through `NativeStore::resolve_default()`" moves to 4½d, when the live path
   switches to `build_match` and a menu can change settings. Two reasons. The live match still comes
   from `default_match.txt`, so loading `liero.cfg` now would honour `map` but ignore `lives`, `health`
   and the loadouts. And `load_setup` writes defaults into the per-user directory when the file is
   missing — the same directory the C++ game uses — which would make running a dev binary write to
   it, with no Rust way to change the value afterwards. `hud_flags(mode, scenario_hud, settings_map)`
   already takes the value, so 4½d changes one argument.
7. **User directory.** `pref_path_for(PrefOs, env, org, app)` mirrors SDL3 `SDL_GetPrefPath` from
   environment variables: macOS `$HOME/Library/Application Support/o/a/`, Linux/BSD `$XDG_DATA_HOME`
   or `$HOME/.local/share`, then `/o/a/`, Windows `%APPDATA%\o\a\`, anything else `None`. Empty
   variables count as unset, as the XDG spec says (SDL would use an empty `XDG_DATA_HOME` as is). It
   is the C++ game's own directory, on purpose (LD 8: one byte-compatible `liero.cfg`). Not mirrored:
   `OPENLIERO_DATADIR` (packaging) and creating the directory eagerly (Rust creates parents on first
   write). `--config-root` and `portable.txt` stay deferred; `NativeStore::single_dir` is the
   constructor they will use. One nuance vs §3.5: C++ `ShadowsSystem` skips the system-layer check
   only when the user root *is* the system root (`filesystem.cpp:754-759`), so a `--config-root`
   pointing elsewhere still checks `data/`. That gets decided when the flag lands.
8. **`TC_ROOT` has exactly two production users**: `game/src/main.rs:37` and `shot/src/lib.rs:392-396`.
   `game/src/input.rs:791` and `scenario/src/loader.rs:244` (§2.2's `:225`) are `#[cfg(test)]` consts
   and stay, like the other per-test consts. The wasm `include_dir!`/`include_bytes!` literals in
   `scenario/src/assets.rs` can't take a `const`, so they stay too.
9. **Hash-subset fact** (for Step 5): `bloodParticleMax` (a sim-reaching pool cap, §1.1) and
   `randomMapWidth`/`randomMapHeight` are outside `SerializeGameplay`, so C++ `UpdateHash` doesn't
   cover them. Ported as is.
10. **(2026-09-25, at landing) wasm now shows the HUD in its live match.** The PR-preview track (PR #11)
    made the wasm build default to `Mode::Live` (the keyboard default match), so §8's `Live` arm
    applies there and the preview draws the stats panel and minimap. `?demo` still selects the
    `Scripted` `blood` demo, which stays world-only together with its debug frame-parity witness. The
    witness is only loaded for `Scripted`, so no gate changes. §3.1/§3.2 held on the full corpus: G5a,
    G5b and G5c passed on the first run with no reader or writer fix.

---

## 10. Deferrals

- **`MakeShadow` and level preparation** → 4½b (`sim::levelgen::generate_from_settings`), composed with
  `build_match` in 4½d. `SelectSpawn` is deferred with Holdazone (its only caller is `SpawnZone`).
- **A sim golden on a generated non-504×350 level** → 4½e.
- **The palette bug on the scenario path** (`scenario::load` ignores `LevelData.palette`) → fixed in the
  builder only; the live path picks it up when 4½d switches to the builder.
- **Asymmetric player health** (per-worm `settings_health` in the sim) → 4½f, with the HEALTH menu item;
  refused by the builder until then.
- **The laser do-loop and `ProcessSteerables`** (RIFLE, WINCHESTER, LASER, GAUSS GUN, MISSILE) → a sim
  task before 4½c (§11 Q1); banned in 4½a's goldens.
- **Holdazone** → stays refused (`BuildError::HoldazoneUnsupported`, dumper `exit 1`).
- **Esc-to-menu fade, weapon-selection phase, stats routing** in `MatchFlow` → 4½d / 4½c / 4½g.
- **Recording a `MatchConfig` match** → 4½d (§4.4).
- **Key-binding translation** (DOS scancodes in `controls` ↔ `KeyCode`) → 4½f; 4½a stores the values.
- **`portable.txt`, `--config-root`, directory listing, wasm `LocalStorageStore`** → packaging / 4½e / 4½h.
- **Render fade** (`MatchFlow::fade_value` drawn) → 4½d.

---

## 11. Open questions (with recommendations)

1. **Where do the laser do-loop and `ProcessSteerables` get ported?** (**John**) Five of forty weapons
   (finding 8) hit deferred Step-2 sim branches; weapon selection (4½c) makes them one menu press away (a
   debug panic, a release-build divergence). **Recommendation:** a dedicated sim task — call it **4½c-0** —
   before 4½c's weapsel, with its own dumper golden per shot type, and 4½c's weapon menu refusing nothing.
   Alternative: 4½c ships with those weapons forced to `weap_table = 2` (hidden) until ported — cheaper,
   but the menu would not match C++.
2. **Asymmetric health.** **Recommendation:** refuse in the builder now (`AsymmetricHealth`), refactor
   `settings_health` onto `WormState` in 4½f alongside the HEALTH item that is the only way to produce it.
   Controller-decidable; John only if he wants the menu item earlier.
3. **The restated byte gate (§3.4).** Done-when 5's "shipped files round-trip byte-identical" is impossible
   even for C++ (finding 6). **Recommendation:** accept G5a/G5b/G5c as the gate and update the overview's
   wording when 4½a-2 lands. Flagged for John because it rewords a done-when he signed off.
4. **What the live binary does at match end until 4½g.** **Recommendation:** restart (the F5 path),
   because it is the closest thing to C++'s stats → menu → NEW GAME and it exercises the full 180-frame
   post-mortem live. Alternative: hold the last frame. Controller-decidable.

## 12. Risks

- **A settings field that silently keeps its default** — the overview's named risk. Mitigated by §7.1 (the
  real C++ reader on one side, the Rust reader on the other) and §7.3's coverage table, re-asserted as
  intent guards in the milestone test (each sidecar's non-default values are checked after parsing).
- **The thread-local shadow flag** (§5.1) is sim-affecting hidden state. Mitigated by publish-at-top /
  clear-at-bottom, a unit test that the flag is off outside a frame and after `process_frame`, and the
  shadow-on goldens (killemall, scales, defaults).
- **Fuzz seeds that walk into deferred sim branches** (finding 8, the RemExp hack, particle trails). The
  generator runs in debug (every deferred branch is a `debug_assert!`) and skips panicking seeds; loadouts
  use golden-proven weapons; the five known offenders are banned from bonuses.
- **toml++ formatter details** (4½a-2): the multiline threshold edge, the literal-vs-basic string decision,
  the trailing newline. The gate is C++ output, not a spec reading; the synthetic corpus targets each rule.
- **The builder–loader duplication.** The builder assigns the full TC-const set; `scenario::load` keeps its
  historical subset (its goldens depend on it). The scene construction is factored into one helper shared by
  both (a pure refactor gated by the render goldens); the TC-const assignment is deliberately not shared.
- **The worktree's first C++ configure bootstraps vcpkg** (no `build/` in this worktree yet): slow once;
  `VCPKG_ROOT=/Users/john/code/openliero/tools/vcpkg/vcpkg` reuses the main checkout's.
