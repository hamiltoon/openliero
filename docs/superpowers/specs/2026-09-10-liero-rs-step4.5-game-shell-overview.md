# Step 4½ — Game shell: overview / altitude decisions

Status: **OVERVIEW — Step 4½ architecture/strategy** · 2026-09-10 · **4½a LANDED** (4½a-1 + 4½a-2), **4½b complete on `liero-rs-step-4-5`**, **4½c-0 LANDED**, **4½c LANDED**, 4½d–4½h planned
Part of: `2026-06-26-liero-rs-roadmap.md`
Detailing: the "Step 4½ — Game shell" section of `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md`
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` (C++ map, cited as **cpp-map §N**)
and `2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (Rust map, cited as **rust-map §N**)
Precedent template: `2026-07-12-liero-rs-step4-input-replay-overview.md`

This is the Step 4½ architecture/strategy decision document — one level more concrete than the
preliminary breakdown, **not** a per-slice spec and **not** a TDD task list. It locks the
cross-cutting decisions every slice inherits (pixel-exact-menus-first fidelity, the Bevy-free
`ScreenStack`, the frozen scenario format plus a new `MatchConfig` builder layer, the dedicated
level-generation RNG, the C++-mirrored TOML persistence schema, the slice ordering and the
split oracle strategy) so each slice spec can be written against a stable foundation.

Step 4½ is an **insertion**, not a discovery: Steps 0–4 are complete, and 4f deliberately stopped
at a *bare-run default match* (`GenerateFromSettings` and the whole `*State.cpp` tree were an
explicit Step-3/Step-4 deferral — Step 4 overview §Deferrals, §Open Q4). The roadmap's Step 4
done-when was "playable single-player, **feels like Liero**"; what exists feels like a Liero
*match*, launched by a hard-coded fixture. Everything a player touches before and after the match
— the main menu, weapon selection, level choice, settings, profiles, an opponent to play against,
a result screen — is still missing. Step 4½ ports that shell, so the Rust build is a complete
playable game before netplay (Step 5) layers rollback on top of it.

---

## What Steps 0–4 already delivered

The pieces Step 4½ builds the shell out of are all in place and Bevy-free:

- **`sim`** — `SimState::process_frame(&[ControlState; N])` (`sim/src/state.rs:1491`), bit-exact vs
  C++ including the 7505-frame fuzz-parity run (Step 2, PR #3); `WormInit::resolve_weapons(objects, weap_order, settings_weapons)`
  (`state.rs:240-256`) is already the exact C++ `Worm::InitWeapons` mapping — **the weapon-selection
  primitive exists**, the menu only has to produce five 1-based indices instead of `[1; 5]`
  (rust-map §5).
- **`render`** — a pixel-exact CPU pipeline with everything a menu needs: `Bitmap`
  (`render/src/bitmap.rs:65`, `fill_rect` `:129`), `Font::draw_string` (`font.rs:191`), `blit_image`
  (`blit.rs:66`), `draw_bar` (`blit.rs:412`), `pack_pal32` / `build_palette` (`palette.rs:36`, `:47`).
  `frame::draw` restores the full clip on exit (`frame.rs:176`), so overdrawing a menu on a rendered
  world frame — the C++ `frozen_screen` look — is safe (rust-map §2).
- **`scenario`** — `scenario::load` (`scenario/src/loader.rs:100-218`), the *only* `SimState` builder,
  plus the `read_asset` seam that makes every TC read work on wasm (`assets.rs:16-27`, `:48-103`).
- **`game`** — a Bevy app whose `FixedUpdate` `tick_and_render` (`main.rs:573`) is the **only** `Sim`
  mutator, at the C++ cadence `1000/14 Hz`; live keyboard sampling once per tick
  (`input.rs:278-289`); record→replay round-trip bit-exact; audio via an `AudioSink` trait
  (`audio.rs:37-51`); live viewport shake/flash/banners; `.lrp` phase-1 reading.
- **`assets`** — `TcConfig` already parses **every menu string** the C++ shell shows
  (`assets/src/tc.rs:206-246`: `SelWeap`, `SelLevel`, `LevelRandom`, `Randomize`, `Done`, `Weapon`,
  `Availability`, `NoWeaps`, `PressAnyKey`, `Copyright`, plus `onoff`, `game_modes`, `weap_states`,
  `controllers`, `key_names`) and `aiparams`, DumbLieroAI's `k[state][control]` table (rust-map §6).
  A Rust menu is string-faithful and AI-parameter-faithful for free.
- **Harness** — the C++ dumper directive pattern with "absent ⇒ old behaviour" defaults
  (`sim_physics_dump.cpp:42-74`), ~50 golden tests, the `shot` CLI and the `liero-shot` run-skill.

What is **absent** and defines the step: no `States`/screen concept anywhere in `main.rs`
(rust-map §1), no random level generation (rust-map cross-cutting §4), no settings persistence
(§5), no key rebinding or profiles (§3), no match-over detection or stats recorder at all (§5), and
`Levels/` holds only 5 test fixtures (§4).

---

## Goal / done-when

Turn "plays a hard-coded match" into **a complete game**: launch into a main menu over a generated
level, configure the match (settings, weapon availability, level, players, controls), pick weapons,
play against a human or a bot, finish on a stats screen, and come back to the menu — close to or
exactly like openliero, with every sim-affecting piece of that flow bit-exact against a C++ golden
and every existing golden untouched.

**Done when (7):**

1. **Bare `cargo run -p game` opens the main menu over a generated level** — the C++
   `Gfx::InitFrameStepping` shape (`gfx.cpp:1439`: build controller → generate a level → push
   `MainMenuState`, `:1445-1447`, `:1462-1464`), not a match that has already started.
2. **A full match can be configured, played and finished using only the menus**: match setup
   (settings), weapon availability, level (file *or* random), player profiles/colours/controls,
   weapon selection, the match itself, match end, stats screen, back to the menu, QUIT.
3. **Every sim-affecting piece is bit-exact vs a C++ golden** — settings→sim plumbing, random level
   generation + `MakeShadow` + `CorrectShadow`, the weapon-selection RNG stream, DumbLieroAI's
   control-state stream, `IsGameOver` (cpp-map §8, "must be ported bit-exact").
4. **All prior goldens stay byte-identical** — the standing re-diff gate from Steps 2–4; the
   scenario text format stays frozen, so nothing in the corpus can move.
5. **Rust saves the same bytes C++ saves.** For every shipped `data/Setups/*.cfg` and
   `data/Profiles/*.toml`, Rust load → Rust save equals C++ load → C++ save, and any file C++ has
   saved round-trips byte-identical through Rust. **Corrected 2026-09-10 (4½a design §3.4):** the
   shipped files are v5 / 6-bit RGB / hand-written and C++ itself rewrites all ten on load + save,
   so "shipped file round-trips unchanged" is unsatisfiable even in C++.
6. **The same shell runs on wasm**, with localStorage persistence and live keyboard.
7. **Playing alone works**: a human vs. a `DumbAI`-controlled worm, selected from the player menu.

Presentation (menu pixels, stats screen layout) is **not** C++-gated — it cannot be, the C++ menu is
`Gfx`-driven and not dumpable (rust-map §9). It gets Rust-only self-goldens plus PNG eyeballing, as
§Oracle explains.

---

## Locked decisions (inherited by every slice)

1. **Pixel-exact menus first; a modern UI is a separate later step.** The menus are ported onto the
   existing Bevy-free CPU pipeline — `render::font`, `render::blit`, the palette, the same 320×200
   surface uploaded as one texture (rust-map §2) — reproducing `MenuItem::Draw`'s exact recipe
   (`menuItem.cpp:6-42`: shadow at `(x+3,y+2)` colour 0, text at `(x+2,y+1)`, selected colour 168,
   `DrawRoundedBox` behind the selection). A **fully modern UI** (real layout, scaling, mouse) is
   recorded in the roadmap's deferred list as a future post-step, **not** part of 4½. Rationale: the
   pixel-exact path reuses everything Step 3 proved and gives an objective "does it look like
   Liero?" check; a modern UI is a design project, not a port.
2. **A Bevy-free `ScreenStack` resource in `game/src/lib.rs`, mirroring the C++ `StateStack`**
   (`state.hpp:47`, over `AppState` `:12`). Push / pop / replace-top (deferred,
   `state.hpp:75`, checked before the pop `:101-105`), `is_overlay` (`state.hpp:34` —
   `InputStringState` draws over the menu below, `inputState.hpp:23`), "update returns false =
   pop me", draw bottom→top past overlays (`state.hpp:117-131`). It lives in `lib.rs` because that is the established rule for anything
   headlessly testable (`game/src/lib.rs:13-15`, rust-map §1); Bevy glue stays in `main.rs`.
3. **`tick_and_render` stays the ONLY `Sim` mutator**, now gated on a run condition "top screen is
   `Playing`". **Menus never touch `Sim`.** This is the determinism firewall restated structurally:
   a menu that cannot reach `SimState` cannot perturb a golden. Every slice re-asserts it.
4. **The scenario text format stays FROZEN.** A new `MatchConfig` (mirroring C++ `Settings` +
   `WormSettings`, same field names and defaults — `settings.hpp:50`, `:68-90`, `settings.cpp:23-60`,
   `worm.hpp:85`) feeds a **new builder layer in the `scenario` crate** that produces `SimState` and
   closes `scenario::load`'s gaps: per-worm weapons (today `[1u32; NUM_WEAPONS]` for *both* worms,
   `loader.rs:122`, `:127-138`), lives, `loading_time`, health, `max_bonuses` + the bonus consts,
   `weap_table` (exists at `state.rs:1100`, never populated), `sound_hooks`. This is rust-map
   cross-cutting §2 option (b), chosen so **every committed golden stays byte-identical with zero
   dumper work**. C++ dumper directives are added only where a sim oracle genuinely needs one.
5. **`SimState::new`'s signature does not change.** The post-`new` field-assignment convention
   (`state.rs:1272-1288`, rust-map §5) is load-bearing for the goldens; the builder assigns fields
   after `new`, exactly as `scenario::load` and the oracle harnesses already do.
6. **Random level generation uses a dedicated `Rand` seeded from the match seed** — *not* wall-clock
   like C++ single-player. C++ has an asymmetry the port must resolve: single-player generates from
   `gfx.rand`, the presentation RNG seeded from `time(nullptr)` (`gameEntry.cpp:23`, used at
   `gfx.cpp:1446` and `:1520`), while the netplay/test paths generate from `game.rand`, the sim RNG
   (`rollbackController.cpp:381`, `net/session.cpp:696`, `game_harness.hpp:58`) — cpp-map §4.2. We
   take the **netplay shape**: a dedicated `Rand` seeded from the match seed. The generation is
   therefore deterministic, recordable and replayable, and the *level* for seed S equals C++
   `GenerateRandom` driven by a `Rand` seeded with S. The divergence from C++ single-player is the
   *seed source*, which is unobservable. **Correction (4½b design):** it is *not* identical to the
   C++ netplay path in one respect — there, generation advances the sim RNG before frame 0 and the
   host ships map + RNG state (`session.cpp:534-537`, `:693-704`); ours leaves the sim stream
   untouched. Unobservable until Step 5, which runs Rust on both peers; Step 5 must choose.
7. **The weapon-selection phase is part of the match, in `sim`, Bevy-free.** `weapsel.cpp` becomes a
   struct in the `sim` crate with `process_frame(inputs) -> bool`, because **it consumes
   `game.rand`** — the sim RNG (`weapsel.cpp:57-61`, the rejection loop `:66-75`, `Randomize`
   `:316-337`), so the number and order of draws before match frame 0 is load-bearing and depends on
   the players' saved picks, `weap_table` and `select_bot_weapons` (cpp-map §3). C++ confirms this
   by snapshotting `game.rand` inside `WeaponSelectSnap` (`weapsel_snapshot.hpp:37`). The C++
   key-repeat emulation (12 initial / 3 interval frames, `localController.hpp:44-46`,
   `localController.cpp:121-146`) is ported, because it changes which frames register presses.
8. **Settings persistence mirrors the C++ TOML schema byte-for-byte.** `[settings]` (`version`,
   `modernColors`, the scalars, the `weapTable` array), then `[player1]`, `[player2]`,
   `[network_player]`, field names taken from `cereal_types.hpp:161-194` and `:282-310`, including
   the `rgbDepth` marker with its 6-bit expansion `(v&63)<<2` for files without it
   (`cereal_types.hpp:290-302`). Native storage mirrors `paths::Resolve` (`filesystem.hpp:145-160`):
   a merged read view of the user config dir over read-only `data/`, all writes to the user dir;
   wasm uses localStorage. `Settings::UpdateHash` — XXH3-64 over the gameplay-only subset
   (`settings.cpp:92-101`, subset at `cereal_types.hpp:218-238`) — is ported too, so Step 5 can
   exchange it.
9. **Two live bugs are folded in.** (a) `state.sound_hooks` is never assigned by `scenario::load`, so
   every hook sound plays sample 0 — `TcConfig` *does* resolve the hooks
   (`assets/src/tc.rs:467`, including `MenuMoveUp`/`MenuMoveDown`/`MenuSelect`, `:297-306`)
   — rust-map §4. (b) The live `game` binary never draws the HUD: `SceneData::as_scene` returns
   `draw_hud=false, map=false` (`loader.rs:76-77`) and `main.rs:743` never flips them, while
   `shot --hud` does (`shot/src/lib.rs:200-204`) — rust-map §2.
10. **One accumulating PR**, branch `liero-rs-step-4-5`, merged when the step is done — the pattern
    of Steps 2, 3 and 4.

---

## Oracle / verification strategy

Step 4½ splits cleanly in two, and the split *is* the strategy: **~1.5 k of the ~10 k LOC is
sim-affecting** (cpp-map §9) and gets the same bit-exact treatment as every prior step; the rest is
presentation that no C++ oracle can reach.

- **Hard gate 1 — sim goldens for settings→sim plumbing.** Non-default settings (lives,
  `loading_time`, `health`, `max_bonuses`, per-worm weapon loadouts, `weap_table`) must produce the
  same `HashGameState` series as C++. This needs new dumper directives; they follow the established
  "absent ⇒ old behaviour" rule (`sim_physics_dump.cpp:42-74`) so the existing corpus is untouched.
- **Hard gate 2 — `oracle_dump_levelgen` (new).** No dumper covers generation today:
  `oracle_dump_level` covers `Level::load` only, and `oracle_dump_sim` / `_sim_physics` / `_lrp_gen`
  *deliberately* load a fixed level rather than call `GenerateFromSettings` (`sim_dump.cpp:12,76`;
  `sim_physics_dump.cpp:33-35`; `lrp_gen.cpp:233`) — cpp-map §10. The new target runs
  `GenerateRandom` over a matrix of (seed × width × height × shadow) and emits FNV-1a hashes of
  `material_id` **pre- and post-`MakeShadow`**, plus `SelectSpawn` results. The existing C++ test
  `test_random_map_size.cpp:87,110` checks dimensions only, never content.
- **Hard gate 3 — `oracle_dump_weapsel` (new).** Dumps the `game.rand` state and the five picked
  weapon ids over a matrix of (seed × `weap_table` × prior picks × `select_bot_weapons`), pinning
  the draw count and order the rejection loops produce (`weapsel.cpp:35-39`, `:57-75`, `:95`,
  `:316-337`). `test_rollback_weapsel.cpp` is the only existing C++ coverage and asserts snapshot
  round-trip, not content.
- **Hard gate 4 — DumbLieroAI control-state stream.** The AI has its **own** `Rand`
  (`worm.hpp:133`), never `game.rand`, but its *output is worm input*, so it is fully sim-affecting
  (cpp-map §6). A dumper directive with a **fixed AI seed** emits the per-tick control state; Rust
  must reproduce the stream bit-for-bit, after which the ordinary sim golden covers the consequences.
  How the C++ seed is established must be pinned in the 4½f design (§Open Q3).
- **Hard gate 5 — TOML round-trip byte-gate.** C++-saved file → Rust load → Rust save → byte-
  identical, run against the shipped `data/Setups/liero.cfg` and `data/Profiles/*.toml`
  (`{AI,Lefty,Righty,Joystick}*.toml`, cpp-map §5.3). `test_settings.cpp` is the C++ mirror of this
  test and defines the field coverage. `Settings::UpdateHash` gets its own vector.
- **Standing isolation gate.** Every slice re-diffs **every** existing golden. Menus are proven
  sim-inert *structurally* (they have no `&mut SimState`; only `tick_and_render` does, locked
  decision 3) and *empirically* (the re-diff). This is the Step 3/4 discipline continued.
- **Presentation — Rust-only self-goldens + PNG eyeballing.** A "menu golden" is
  `render::hash::hash_frame(&surface, 33)` of a menu drawn at a fixed, deterministic state; it has
  **no C++ counterpart** (rust-map §9). It is a *regression* gate, not a correctness gate:
  correctness comes from a `shot`-rendered PNG compared by eye against the C++ build via the
  `liero-shot` skill. The skill's §7 grows a menus/level-select section the same way it grew a live
  play section in 4g.
- **The wasm parity witness.** Today wasm is Scripted-only and runs a per-tick frame/state
  self-check (`main.rs:697-725`). 4½h enables live keyboard on wasm, which **retires that check on
  that path** — deliberately, and only there; the headless scripted regression keeps the guard.

---

## Slice ordering (4½a–4½h)

Sim-first, thin-vertical-then-widen: the bit-exact substrate (a, b, c) exists before the menu
framework that drives it (d), and the wide menu surface (e, f, g) is layered on a milestone that
already works end to end. Each slice accumulates on `liero-rs-step-4-5` and states its own gate.

- **4½a — `MatchConfig` + settings model + builder + match end rules + persistence + two bug
  fixes.** The `Settings` / `WormSettings` model with C++ field names and defaults; the
  `MatchConfig → SimState` builder in `scenario` that closes `loader.rs`'s gaps (per-worm weapons,
  lives, `loading_time`, `health`, `max_bonuses` + bonus consts, `weap_table`, `sound_hooks`); the
  `Game::IsGameOver` port (`game.cpp:521` — lives ≤ 0 for KillEmAll/Scales, `timer >= time_to_lose`
  for GameOfTag/Holdazone) plus the Scales-of-Justice extra-life rule (`game.cpp:511-566`,
  `:155-166`) and the 180-frame post-mortem fade before the match ends
  (`localController.cpp:213`, `:275-280`, `:186-198`); TOML persistence (schema, paths, `UpdateHash`);
  and the live bugs (`sound_hooks`, HUD off, and — found in 4½b's design — `scenario::load` ignoring
  `LevelData.palette`, so POWERLEVEL palettes never show); plus the `CorrectShadow` port
  (`sim::shadow`, gated on `settings.shadow`; Rust has none today). **Gate:** sim goldens with non-default settings via
  new dumper directives + the TOML byte-gate. **Proves:** the shell can *configure* a match without
  the scenario format moving.
- **4½b — Random level generation.** `GenerateDirtPattern` (`level.cpp:11` — the diffusion noise
  field, the `rand(100)` large-sprite splats `:30-71`, the `rand(15)` stones `:73-82`), the
  `rand(50)+5` dirt-effect tunnels (`:108-135`, each `DrawDirtEffect` call drawing one more
  `rand(tex.r_frame)`, `blit.cpp:534-537`), the rock formations with their `kMaxTries = width*height`
  rejection cap (`:140`, `:155-158`, `:142-170`, `:172-192`), `MakeShadow` (`level.cpp:195`) and
  `generate_from_settings` (random / file / fallback + shadow). All three primitives already exist
  on the Rust side (`sim_core::rng::Rand`, `sim::blit::draw_dirt_effect`, `SpriteSet` — rust-map
  cross-cutting §4). **Gate:** `oracle_dump_levelgen`, bit-exact, incl. a `CorrectShadow` dig
  stage that checks 4½a's port (4½b T9, runs after 4½a lands it). **`SelectSpawn` is deferred with
  Holdazone** — its only caller is `SpawnZone`, reachable only from Holdazone (`game.cpp:455`,
  `:494`, `:516-518`). **Parallel with 4½a** — file-disjoint; `CorrectShadow` (Rust has none today —
  Step 2's O4 omitted it at all 7 call sites) is owned by 4½a. Plan:
  `plans/2026-09-10-liero-rs-step4.5-slice4.5b-plan.md`.
- **4½a is split (4½a design §9):** **4½a-1** = model + TOML reader + builder + sim ports
  (`CorrectShadow`, the missing Scales/`DoHealing` mode rules) + `is_game_over` + `MatchFlow` +
  sim goldens via one new optional scenario directive `settings <file>` (the dumper reads it with
  the real `Settings::FromToml`; `scenario::load` refuses it) + the `sound_hooks` fix. **LANDED**
  (plan: `plans/2026-09-10-liero-rs-step4.5-slice4.5a1-plan.md`). **4½a-2** = TOML writer + the
  byte gate + `UpdateHash` + storage + `TC_ROOT` centralisation + the HUD fix. **LANDED** (plan:
  `plans/2026-09-10-liero-rs-step4.5-slice4.5a2-plan.md`; the binary does no config I/O until
  4½d — slice design §9.3.6). Interim rulings: differing
  per-player health is refused until 4½f (Rust carries one health value); the live game restarts
  (the F5 path) 180 frames after game over until 4½g adds the stats screen.
- **4½c-0 — the unported weapon branches (added 2026-09-10, 4½a design finding).** RIFLE,
  WINCHESTER, LASER, GAUSS GUN and MISSILE enter Step-2 branches that were never ported (the
  `ST_LASER` do-loop and its siblings — the sim panics). Weapon selection makes them choosable, so
  they are ported bit-exact, with sim goldens, **before** 4½c. Until then they are banned in 4½a's
  goldens. **Widened 2026-09-10 by its design (§9 Q1, controller ruling: all in one slice): 13
  weapons, not 5** — also LARPA, BOUNCY LARPA, CRACKLER (particle trails via `Create1`/`Create2`),
  MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER (splinter trails, `nobject.cpp:133-138`) and BOOBY TRAP
  (chain explosions, `sobject.cpp:148-150`); MISSILE's `ProcessSteerables` is a *silent* no-op, not
  a panic, and porting it closes Step 4d's `steerable_sum` camera deferral. Also moves input
  application to the top of the tick (C++ order; the MISSILE Up boost is the first object-loop
  control read). Design: `specs/2026-09-10-liero-rs-step4.5-slice4.5c0-weapon-branches-design.md`;
  plan: `plans/2026-09-10-liero-rs-step4.5-slice4.5c0-plan.md` (T0–T13). 4½a-2 plan:
  `plans/2026-09-10-liero-rs-step4.5-slice4.5a2-plan.md` (T0–T7).
  **Landed** (design `specs/2026-09-10-liero-rs-step4.5-slice4.5c0-weapon-branches-design.md`): the
  inventory found **thirteen** weapons, not five. LARPA, BOUNCY LARPA, CRACKLER, MINI NUKE, BIG
  NUKE, NAPALM, HELLRAIDER and BOOBY TRAP also reached unported branches, and MISSILE was a silent
  divergence, not a panic. All are ported bit-exact; the steerable camera deferral from 4d is
  closed; input application moved to the top of the tick (the C++ order, exposed by the MISSILE
  Up boost). 4½c may offer all forty weapons.
- **4½c — Weapon selection phase.** The `weapsel.cpp` port into `sim` as a Bevy-free struct with
  `process_frame(inputs) -> bool`; the constructor's RNG rejection loops, `Randomize`, bot auto-ready
  (`is_ready[i] = controller != 0 && select_bot_weapons != 1`, `weapsel.cpp:95`), left/right cycling
  that skips disabled weapons (`:245-283`), the hardcoded "Done" index 6 (`:338`), `Finalize` →
  `InitWeapons` + `ReleaseControls` (`:352-361`); key-repeat emulation 12/3
  (`localController.cpp:121-146`); per-viewport menu rendering at
  `vp.rect.CenterX()-31, CenterY()-51` (`weapsel.cpp:41-93`) over the frozen-screen look
  (`:160-209`) with its own palette rotation (`:20-24`). **Gate:** `oracle_dump_weapsel`, bit-exact
  RNG. **Note for Step 5:** this makes concrete the Step-5 deferral of weapon-select-phase rollback
  ("unless the netplay path includes weapon selection") — after 4½c it does, so Step 5 must decide
  whether 5a's snapshot machinery covers `WeaponSelectSnap` (`weapsel_snapshot.hpp:37`).
  **Landed** (design `specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md`, plan
  `plans/2026-09-25-liero-rs-step4.5-slice4.5c-plan.md`). Corrections from its findings:
  (1) the constructor's rejection loop runs only when a saved or rolled pick is DISABLED, and checks
  uniqueness only inside that loop; the "redraw until enabled and unused" description fits RANDOMIZE;
  (2) PICK does not make the bot navigate: the AI never runs during selection, and a PICK bot's menu is
  driven by the keys bound to that worm;
  (3) the phase uses LocalController's 12/3 repeat (RollbackController's differs, finding 5);
  (4) `render::palette::rotate_from` already existed (finding 10); what was missing was `DrawRoundedBox`
  and `Font::GetDims`, both landed here;
  (5) the menu palette is `Origpal` rotated, and `Origpal` carries the worm colour ramps
  (`Game::Focus` → `Palette::SetWormColour`), now `render::palette::set_worm_colour`.
  The screen is gated bit-exact against the REAL C++ `WeaponSelection::Draw` run headlessly in the
  cloud session (361 frames, plan Addendum A), not against Rust self-goldens. The live game starts like
  C++ NEW GAME (default settings, a generated level from a fresh seed, selection, `enter_game`), and F5
  or the match-end restart reuses the played level (`regenerate_level` is off by default).
  **Step 5 note:** the weapon-select snapshot is `SimState` + a `WeaponSelection` clone (plain data;
  `Rand: Clone` landed). Netplay must zero controllers before selection (`rollbackController.cpp:399-401`)
  and finalize on the confirmed done frame (design §8).
- **4½d — Menu framework + `ScreenStack` + main menu. THE MILESTONE.** `Menu` / `MenuItem` /
  `ItemBehavior` (`menu.hpp:26`, `menuItem.cpp:6-42`, `itemBehavior.hpp:8` and the
  Integer/Time/BooleanSwitch/Enum/ArrayEnum family), `DrawRoundedBox`, `Font::GetDims` and the
  `MenuItem::Draw` text arm (landed in 4½c: `render::blit`, `render::font`, `render::menu`), the
  scrollbar (`menu.cpp:107-124`), navigation and `SetVisibility`
  (`menu.cpp:227`, `:250`, `:263`), type-to-search with the 1500 ms prefix timeout
  (`menu.cpp:14-79`), palette rotation on indices 168..174 via `RotateFrom`
  (`gfx.cpp:978-1006`; `render::palette::rotate_from` exists; 4½c's
  `render::weapsel::weapsel_palette` uses it), menu fade in/out
  (`mainMenuState.cpp:153-159`, `:604-609`), menu sounds through the existing `AudioSink`
  (`TcConfig.sound_hooks.MenuMoveUp/MenuMoveDown/MenuSelect`); the `ScreenStack`; `MainMenuState`
  with its item list (`gfx.cpp:505-521`) and the selection dispatch that is the real screen router
  (`gfx.cpp:1493-1593`) — including NEW GAME's rule that the previous level is **reused** unless
  `regenerate_level` is set or `random_level` / `level_file` / map width / height changed
  (`gfx.cpp:1507-1523`), which decides whether 4½b's generator (and its seed) runs at all (4½c's
  live `game::new_game` already has the `regenerate_level` half; 4½d adds the settings-changed half);
  `DrawBasicMenu` (`gfx.cpp:1699`) and a `DrawSpectatorInfo` equivalent
  (`gfx.cpp:1739`). **Milestone:** bare run → main menu → NEW GAME → weapon selection → play → Esc →
  menu (RESUME / NEW GAME) → QUIT. **Gate:** menu frame-hash self-goldens + the standing re-diff.
  Depends on a, b, c.
- **4½e — Settings menu, weapon options, level selector.** All `SettingsMenu` items with their
  behaviors and ranges (`gfx.cpp:485-503`, `:1262-1312`) and the per-game-mode visibility rules
  (`gfx.cpp:1314-1341`: LIVES for KillEmAll/Scales, TIME TO LOSE for GameOfTag, TIME TO WIN + ZONE
  TIMEOUT for Holdazone, MAP WIDTH/HEIGHT only when `random_level`) — including the C++ quirk that
  TIME TO LOSE and TIME TO WIN **bind the same field** (`gfx.cpp:1284-1286`); `WeaponMenuState`
  (weapon *availability* over `weap_table`, `weaponMenuState.cpp:14`, refusing to close with zero
  weapons enabled via an `InfoBoxState`, `:91-109`); the level selector (`fileSelectorState.cpp:71-156`)
  with its synthetic `RANDOM` node (`:78-84`), cursor restore (`fileSelector.hpp:184-214`) and live
  minimap preview (`:105-156`, `level.cpp:489`, `level.hpp:31-34`); SAVE SETUP AS… /
  LOAD SETUP (`mainMenuState.cpp:285-311`, `paths::ShadowsSystem` refusing reserved names,
  `filesystem.hpp:142`); the `InputStringState` and `InfoBoxState` overlays (`inputState.cpp:13`,
  `:166`). **Gate:** menu self-goldens + a settings→`MatchConfig`→sim golden reusing 4½a's directives
  + the **first sim golden on a non-504×350 level** (MAP WIDTH/HEIGHT land here; the sim has only
  ever been gated at 504×350).
  **Parallel with 4½f.**
- **4½f — Player menu, profiles, DumbLieroAI.** The `PlayerMenu` (`gfx.cpp:459-483`, `:1362-1428`):
  NAME (with `GenerateName` for an empty name, `mainMenuState.cpp:323-347`), HEALTH, R/G/B with the
  classic 0..252-step-4 `display_div=4` picker (`gfx.cpp:1376-1388`) and the colour-bar overlay
  (`gfx.cpp:1343-1360`), INPUT, the seven key bindings + DIG via `WaitForKeyState`
  (`inputState.cpp:102-162`, routed at `mainMenuState.cpp:373-389`), WEAPON 1..5 with the
  Levenshtein fuzzy match normalised by name length (`mainMenuState.cpp:390-423`, `:28`), CONTROLLER
  (`texts.controllers` = Human / DumbAI / FollowAI); profile save/load
  (`worm.cpp:60`, `:73` — note load deliberately preserves `color`, `:94`); and the **DumbLieroAI**
  port (`worm.cpp:477-696`, ~220 LOC: nearest-worm targeting, engagement distance from the weapon's
  `time_to_explo`/`speed`/`gravity` with a floor of 90, probabilistic Fire/Jump/Change from
  `common.ai_params.k[state][control]`, aiming by scanning the 128-entry `cossin_table`,
  `worm.cpp:543-556`) with its **own** `Rand` (`worm.hpp:133`). **Gate:** the fixed-seed AI dumper
  directive → bit-exact control-state stream, then an ordinary sim golden of a human-vs-bot match.
  **Parallel with 4½e.**
- **4½g — Match end + compact stats screen + hidden options subset.** The `GamePlayState` end
  routing (`gamePlayState.cpp:53-93`); a **StatsRecorder subset** in `sim` — kills, damage
  dealt/received/self, lives, timers, per-weapon hits/damage — written **hash-inert**, the way 4c's
  sound events and 4d's shake events were (a side-channel the hash never folds); the compact stats
  screen (numbers + the per-weapon table, `statsState.cpp:315-389`, drawn on the raw EXE palette
  with no rotation, `:304-306`); and the hidden-menu subset that is actually meaningful now —
  FULLSCREEN, SHADOWS, POWERLEVEL PALETTES, AUTO-RECORD REPLAYS, BOT WEAPONS
  (RANDOM/PICK/KEEP, `hiddenMenu.cpp:10`, feeding 4½c's `select_bot_weapons`). **Gate:** stats
  self-golden + the standing re-diff; the recorder's hash-inertness is proven by re-diffing every
  sim golden.
- **4½h — wasm: the whole shell in the browser.** Live keyboard on wasm (today Scripted-only,
  `main.rs:323-331`), localStorage persistence via `web_sys::Storage`, and a **widened embedded
  level manifest** — the `include_dir!` catalogue currently embeds exactly one level,
  `Levels/render_stage.lev` (`scenario/src/assets.rs:53-74`), and a miss panics, so a level picker
  needs more. The wasm debug self-check retires on the live path (§Oracle). **Gate:** the existing
  build-only wasm CI job (`.github/workflows/rust.yml:216-229`) plus a headless-Chrome run of the
  menu → match flow, the 3f precedent.
  **Pulled forward 2026-09-25 by the PR-preview track** (`.github/workflows/preview.yml`, PR #11):
  live keyboard on wasm for the default match, `?weapons=` / `?level=` / `?seed=` URL parameters
  (`game::web_params`), three more small levels embedded, Esc no longer quitting on wasm, and a
  size-tuned `wasm-release` profile (14.5 MB). Every PR now deploys a playable build to Cloudflare
  Pages. 4½h keeps the menu-driven shell, persistence and the headless-Chrome gate.
  **Mobile (added 2026-09-25).** The preview opened on a phone crops the 960×600 canvas and has
  no way to play: there is no keyboard. 4½h therefore also covers (1) a **responsive canvas** that
  scales the 320×200 frame to fit the screen at an integer-or-fit scale, preferring landscape and
  (2) **touch controls**: an on-screen pad plus Fire/Jump/Change buttons, mapped to the same 7-bit
  `ControlState` the keyboard produces (Dig = the Left+Right chord), so the sim cannot tell them
  apart and a touch session records and replays like a keyboard one. The natural phone mode is
  one human against DumbLieroAI, so the touch layout is for one player and depends on 4½f's AI;
  a two-thumbs-per-player hotseat on one phone is out of scope. Menus (4½d–4½f) need tap
  navigation too. The responsive canvas alone is small and has no dependency, so it can land
  early on the preview track whenever phone previews become useful; touch play waits for 4½f.

*Parallelism: **4½a ∥ 4½b** (disjoint surfaces); **4½e ∥ 4½f** (both build on 4½d's framework and
touch different menus). 4½c-0 can start as soon as 4½b's sim work lands (it touches weapon code,
not settings). 4½c depends on 4½a's `MatchConfig` for `weap_table` and saved picks, and on 4½c-0. 4½d
depends on a+b+c. 4½g depends on 4½a's `IsGameOver`. 4½h is last by nature.*

---

## Deferrals (explicitly out of Step 4½ scope)

- **A fully modern (non-pixel-exact) UI** — a later, separate post-step; recorded in the roadmap's
  deferred list. 4½ ports the pixel-exact menus.
- **FollowAI / predictive AI** (`ai/predictive_ai.*` + `dijkstra.hpp` + `work_queue.hpp`, ~1.65 k
  LOC, cpp-map §6) — it evaluates plans by *simulating forward on a cloned game*
  (`predictive_ai.cpp:246`, `:546-551`), which needs the Step-5a snapshot machinery. Deferred until
  after 5a.
- **All netplay menus and states** — `NetConnectState`, `OnlineConnectState`, `RematchState`, the
  `kMaHostGame`/`kMaJoinGame`/`kMaHostOnline`/`kMaJoinOnline` dispatch arms
  (`gfx.cpp:1536-1589`) — Step 5.
- **The F8 weapon-randomiser easter egg** (`mainMenuState.cpp:465-579`) — uses
  `std::random_device`/`mt19937` and mutates `Common` in place; non-deterministic *by design* and
  explicitly not for porting (cpp-map §8).
- **Spectator window** — a second renderer/window; the `frozen_spectator_screen` content is
  reproduced only where 4½ needs it (the weapsel/menu info line).
- **TC selector** (`TcSelectorState`, `fileSelectorState.cpp:230-261`, the `kMaTc` re-init loop
  `gfx.cpp:1500-1504`) — only one TC exists (`data/TC/openliero`, rust-map §4).
- **Stats heatmaps and the total-health-difference graph** (`statsState.cpp:209-247`, `:315-389`) —
  the compact stats screen ships in 4½g; the curves do not.
- **`.lrp` replay browser** (`ReplaySelectorState`, `fileSelectorState.cpp:160-184`) — needs `.lrp`
  phase 2 (the cereal `Game` graph), itself a bounded Step-4 follow-on.
- **Gamepad input** (`InputDeviceBehavior` `gfx.cpp:85`, `gamepad_controls` `worm.hpp:74-77`) —
  keyboard first, as in Step 4; additive and un-gated.
- **Holdazone** stays `unimplemented!()` (`sim/src/state.rs:2250-2252`); the settings menu shows its
  items, and choosing it is refused rather than silently broken. `Level::SelectSpawn`
  (`level.cpp:435`) is deferred with it — `SpawnZone` is its only caller.

---

## Open questions for the controller to adjudicate (with recommendations)

1. **Where does the `Menu` framework live — a new crate, or a module in `game`?**
   **Recommendation: a new Bevy-free crate `rust/ui`** (deps `render` + `assets`), not a module in
   `game/src/lib.rs`. Reason: `shot` can then screenshot menus **headlessly**, which is what makes
   the presentation self-goldens and the PNG-vs-C++ eyeball loop cheap; a module inside `game` would
   drag Bevy into `shot`'s dependency graph or force a hand-copy (the 4d harness duplication we
   already know the cost of). The `ScreenStack` still lives in `game/src/lib.rs` — it owns
   *screens*, which know about the match; `ui` owns *widgets*, which do not.
2. **Do `MatchConfig`/`Settings` live in `scenario`, or in a new `settings` crate?**
   **Recommendation: `scenario`** — it already owns loading, already owns the only `SimState`
   builder (`loader.rs:100-218`), and already has the `read_asset` seam that the persistence layer
   needs for its merged-read view. Split out a `settings` crate only if it grows past roughly 1 k
   lines (`scenario` is 1 226 today, rust-map §10). Either way the compile-time `TC_ROOT` const,
   currently duplicated in five places (rust-map cross-cutting §3), gets centralised here.
3. **How is `DumbLieroAI::rand` seeded in C++?** The map establishes only *that* the AI has its own
   `Rand` (`worm.hpp:133`) and that it is excluded from snapshots as "transient, rebuilt on load"
   (`cereal_types.hpp:329`). The exact seeding path must be read out of the C++ source and **pinned
   in the 4½f design** before the dumper is written — the dumper needs a fixed seed to produce a
   reproducible control-state stream. **Recommendation:** treat this as the first task of 4½f, and
   if C++ seeds it from anything non-deterministic, have the dumper directive *override* the seed
   explicitly (the same shape as any other directive) rather than trying to reproduce a wall clock.
4. **Key naming and storage: DOS scancodes or Bevy `KeyCode`?** C++ displays DOS-scancode key names
   from `common.texts.key_names[177]` (`common.hpp:60`, `gfx.cpp:828`) and stores DOS scancodes in
   the profile TOML (defaults `{0x13,0x21,0x20,0x22,0x1D,0x2A,0x38}` / `{0xA0,0xA8,0xA3,0xA5,0x75,0x90,0x36}`,
   `settings.cpp:23-60`); Rust binds `KeyCode` (`input.rs:85-112`). **Recommendation: store DOS
   scancodes in the TOML** (file compatibility is a done-when) with a `KeyCode` ↔ DOS translation
   table ported from `keys.cpp:9-85`, and display `key_names` from the TC — which `assets::tc::Texts`
   already parses (rust-map §6). Bindings that have no DOS equivalent are the only thing that needs
   a decision, and there are none in the default set.
5. **Minimap preview in the level selector on wasm.** The C++ selector *loads each highlighted level*
   to draw its miniature (`fileSelectorState.cpp:105-156`). Natively this is fine. On wasm only the
   embedded manifest exists. **Recommendation:** preview whatever the manifest contains and show the
   RANDOM node's generated preview otherwise; do not attempt network level fetching in 4½.
6. **Level corpus — ship stock levels, or rely on random generation?** `data/TC/openliero/Levels/`
   holds only 5 test fixtures (rust-map §4), so a file picker has almost nothing to pick.
   **This one is John's call**, because it is a content/licensing decision, not a technical one:
   (a) ship a stock level set into `data/TC/openliero/Levels/`, or (b) let RANDOM be the normal path
   and keep the picker for the fixtures plus whatever the user drops in. **Recommendation: (b) for
   4½** — random generation is bit-exact-gated in 4½b and makes the picker meaningful on its own —
   with (a) as a follow-up if John wants the original level pack.

---

## Risks & the hard 10%

- **The weapon-selection RNG stream (the central risk).** It runs *before* match frame 0 and draws
  from the sim RNG in rejection loops whose iteration count depends on `weap_table`, the players'
  saved picks and `select_bot_weapons` (`weapsel.cpp:35-39`, `:57-75`, `:95`). One extra or missing
  draw shifts every subsequent sim tick. `oracle_dump_weapsel` must cover the *matrix*, not one case.
- **Level generation's rejection loops.** `GenerateRandom` places rocks with retry loops capped at
  `kMaxTries = width*height` (`level.cpp:140`, `:155-158`), and `DrawDirtEffect` draws its own
  `rand(tex.r_frame)` per call (`blit.cpp:534-537`). The *count* of draws is data-dependent, so the
  golden matrix must include sizes and seeds that exercise both the cap and the common path.
- **Settings→sim plumbing is a wide, quiet surface.** `SerializeGameplay`'s field list
  (`cereal_types.hpp:218-238`) is ~20 fields, several of which the Rust sim simply does not have yet
  (`lives` as a setting, `flags_to_win`, `shadow`, `bonus_timeout`, `input_delay` — rust-map §5). A
  field that silently keeps a default produces a *plausible* match that diverges from C++. The
  non-default-settings goldens exist precisely to catch this, so they must vary **every** plumbed
  field, not just the convenient ones.
- **The determinism firewall meeting a screen stack.** Introducing screens is the biggest structural
  change to `main.rs` since it was written (rust-map cross-cutting §1). The rule that saves us is
  mechanical: `tick_and_render` stays the only `&mut SimState`. Any "convenient" menu write into the
  sim — pre-seeding a worm, resetting a level from a menu handler — is the bug class this step can
  produce, and the re-diff gate is the tripwire.
- **A hash-visible StatsRecorder.** C++ suppresses stats writes under `setSpeculative` for a reason.
  The Rust subset must be hash-inert by construction (a side-channel, like the 4c sound stream and
  the 4d shake events), or Step 5's rollback will resim it into a desync.
- **TOML byte-identity is stricter than it looks.** The gate is not "loads the same values" but
  "saves the same bytes", including the `rgbDepth` marker, key order, `version`, and number
  formatting. toml++ and any Rust TOML writer will disagree on formatting until forced to agree —
  expect this to be the fiddliest part of 4½a, and gate it on the *shipped* files
  (`data/Setups/liero.cfg`, `data/Profiles/*.toml`), not on files we generate ourselves.
- **DumbLieroAI's `cossin_table` scan.** The aiming loop scans the 128-entry table for the closest
  direction (`worm.cpp:543-556`) and the C++ source carries a comment about the original's `0xC000`
  bug. Step 3b/4b already hit the `cossin[128]` UB twice and masked `&0x7f`; assume this path has a
  third instance and check it before assuming a divergence is an AI-logic bug.
- **Scope. This is the largest step by raw surface** — ~10 k C++ LOC, of which ~1.5 k is
  sim-affecting (cpp-map §9). The mitigation is the ordering: everything bit-exact is in a, b, c;
  d is the milestone that makes the rest *usable but optional*; e, f, g widen. If 4½ has to be cut
  short, it should be cut after d with e/f/g re-scoped, never by weakening a/b/c's gates.

---

## Next artifact

The first slice's detailed spec (companion document, to be written when 4½a starts):
- `specs/2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md`
