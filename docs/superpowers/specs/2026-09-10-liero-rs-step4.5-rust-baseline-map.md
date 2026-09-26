# Step 4½ — Rust baseline map (what Steps 0–4 left for the game shell to build on)

Status: **RESEARCH MAP** · 2026-09-10 · feeds `2026-09-10-liero-rs-step4.5-game-shell-overview.md`
Part of: `2026-06-26-liero-rs-roadmap.md` (Step 4½ — game shell)
Companion: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` (the C++ side)

All paths are relative to `rust/`; line refs are on `master` (4aba9f8).

---

## 1. `game` crate structure

- `game/src/main.rs` (892 lines) — the whole Bevy app.
- `game/src/lib.rs` (15) — thin library surface so integration tests can drive binary logic: `pub mod audio; pub mod input; pub mod viewport_step;` (`lib.rs:13-15`). **Rule established here:** anything that must be headlessly testable lives in `lib.rs` modules, Bevy-app glue stays in `main.rs`.
- `game/src/input.rs` (881), `game/src/audio.rs` (516), `game/src/viewport_step.rs`, `game/src/blit.rs` (ARGB→RGBA + loop step).
- `game/scenarios/default_match.txt` — the 4f fixture.
- `game/tests/`: `passthrough.rs`, `round_trip.rs`, `record_regression.rs`, `viewport_stepping.rs`.

**Resources** (all plain `#[derive(Resource)]`, no plugin, no `States`):
`ScenarioName(String)` `main.rs:51`, `RecordPath(Option<PathBuf>)` `:57`, `ReplayPath` `:65`,
`Sim(SimState)` `:69` — pure sim, no Bevy types inside (deliberate rollback-ready firewall, doc `:11-13`),
`Demo { scenario, viewports: [Viewport;2], scene: SceneData, surface: Bitmap, tick, golden_state, golden_frame }` `:75-98`,
`FrameImage(Handle<Image>)` `:102`, `AudioDrainer(Drainer<NativeAudio>)` — **NonSend** (`main.rs:157`).
`Mode` and `InputSource` are Resources declared in `input.rs:125,145`; `Recorder` `input.rs:311`.

**Schedule** (`main.rs:181-225`):
```
DefaultPlugins(ImagePlugin::default_nearest, WindowPlugin{960x600, resizable:false, title})
insert_resource(Time::<Fixed>::from_hz(1000.0/14.0))      // C++ gfx.cpp kDelay=14ms
Startup:     setup                    (main.rs:357)
Startup:     setup_audio (exclusive)  (main.rs:491)
FixedUpdate: tick_and_render          (main.rs:573)   <- the ONLY Sim mutator
Update:      close_on_esc             (main.rs:843)
Last:        flush_recorder_on_exit.after(bevy::window::ExitSystems)  (main.rs:868)
```

**Boot / setup** (`main.rs:357-479`): read scenario text (three sources: `--replay` path, the
`DEFAULT_MATCH` fixture, or `GOLDEN_DIR/render_slice3b_<name>_scenario.txt`) → `Scenario::parse` →
`scenario::load(TC_ROOT, &scenario)` → owned 320×200 `Bitmap` → one `Image` (`Rgba8UnormSrgb`,
nearest) → `Camera2d` + one `Sprite` with `Transform::from_scale(Vec3::splat(3.0))` (`:419-423`) →
insert `InputSource`, optional `Recorder`, `Sim`, `Demo`, `FrameImage`.

**Default match (4f)**: `game/scenarios/default_match.txt` — `seed 42`, `level Levels/render_stage.lev`,
two visible worms at fixed positions, `weapon 0 DART`. Bare `cargo run -p game` → `Mode::Live`.

**CLI parsing**: Bevy-free `game::input::parse_args` (`input.rs:206-265`) handles `--live`,
`--record <path>`, `--replay <path>` + one optional positional name; semantic rules in
`main.rs::resolve_scenario` (`:240-314`). Wasm arm hard-codes Scripted/`blood` (`:323-331`).

**Modes** (`input.rs:145-149`): `Scripted`, `Live`, `Replay` (plays once then holds the final frame).

**Restart (F5)**: `main.rs:605-616`. **Quit**: `close_on_esc` writes `AppExit::Success` (`main.rs:843-847`).

**Screen/state concept: there is no Bevy `States` usage anywhere.** The natural slot: a
`ScreenStack` resource (Bevy-free, in `lib.rs`) mirroring the C++ `StateStack`; put
`tick_and_render` behind a run condition on the top screen being `Playing`; an `Update` menu system
draws into the same `Demo.surface`/`FrameImage`. `setup` would be split: the Image/camera/sprite
creation is screen-independent; the `scenario::load` half becomes a "start match" action that
consumes a `MatchConfig` instead of a parsed `Scenario`.

---

## 2. Frame → screen path (a menu can ride it)

Path: `render::frame::draw(&mut Bitmap, &SimState, &mut [Viewport], &Scene)` →
`game::blit::blit_surface_into_bytes` (`blit.rs:9-21`) → `images.get_mut(handle)` → Bevy re-uploads →
one `Sprite` × 3. See `render_and_upload` (`main.rs:732-749`).

Everything a menu needs is already Bevy-free in `render`:
- `render::bitmap::Bitmap` (`bitmap.rs:65`): `new` `:80`, `set_pixel` `:92`, `put_argb` `:111`, `fill(idx,pal)` `:120`, `fill_rect` (clip-clamped) `:129`, `Rect` `:21` + `bmp.clip`.
- `render::font::Font` (`font.rs:40`): `Font::load(&Tga)` `:51`, `draw_char` `:107`, **`draw_string(scr, pal, &str, x, y, color, size)` `:191`** — 7×8 glyphs, `\0` line-break, ASCII-only. Colour is a palette index (`50` = C++ default text colour, `0` for the drop-shadow — see the death-banner draw `frame.rs:139-146` for the "shadowed string" idiom). ~~**Missing:** a `get_dims` equivalent~~ — `Font::get_dims` (`font.GetDims`, `font.cpp:87-112`) landed in 4½c.
- `render::blit::draw_bar` `blit.rs:412`, `draw_line` `:288`, `blit_image(scr, pal, spriteset, frame, x, y)` `:66`. ~~**Missing:** `DrawRoundedBox`~~ — `blit::draw_rounded_box` (`blit.cpp:128-140`) landed in 4½c, with `render::menu::draw_item` (the `MenuItem::Draw` text arm).
- Palette: `render::palette::pack_pal32(&Palette) -> Pal32` (`palette.rs:36`) or `build_palette(origpal, color_anim, cycles, screen_flash)` (`:47`). `render::palette::rotate_from` already exists (`palette.rs:13`; 4½c design finding 10); 4½c adds `render::weapsel::weapsel_palette` over 168..174, and `render::palette::set_worm_colour` (C++ `Palette::SetWormColour`, reached via `Game::Focus`).
- The loaded `SceneData` already owns `origpal`, `color_anim`, and the `font` (`scenario/src/loader.rs:40-53`).
- `frame::draw` restores `bmp.clip = full_clip` at the end (`frame.rs:176`), so overdrawing a menu on top of a rendered world frame (the C++ `frozen_screen` look) is safe.
- (**4½e-1.**) `Scene` gained `small_labels: Option<SmallLabels>` (the existing `labels` field is the HUD's
  `&HudLabels`): the three `DrawTextSmall` labels — a weapon bonus's name, a booby trap's disguise name, and the
  current weapon over a worm whose Change bit is held (`viewport.cpp:408-413`, `:466-479`, `:575-581`) — drawn through
  `render::small_text::draw_text_small` with `SceneData::text_sprites` (`text.tga`). It is `None` on every
  pre-4½e-1 golden path (`as_scene`, `shot`, the render harnesses), so no old golden moved; the shell's `Match`
  sets it. `render::font` also gained `get_dims_h` and `draw_framed_text`, and `render::blit` `blit_bitmap`.

**320×200 / scale**: `SURFACE_W/H = 320/200` (`main.rs:45-46`); window fixed at `960×600`; scale is the
sprite `Transform` ×3 (`main.rs:422`). HUD geometry derives the multiplier from the surface (`frame.rs:70-73`).

**Note:** the live `game` binary currently **does not draw the HUD** — `SceneData::as_scene` returns
`draw_hud=false, map=false` (`loader.rs:76-77`) and `main.rs:743` never flips them. `shot --hud`
does (`shot/src/lib.rs:200-204`). Cheap 4½ win.

---

## 3. Input

- Pure core: `PlayerBindings<K>` (`input.rs:31-42`) → `control_state(|k| pressed(k)) -> ControlState` (`:56-67`), Dig = Left+Right chord (`:57-62`).
- `InputMap { players: Vec<PlayerBindings<KeyCode>> }` (`:73`), `default_bindings()` (`:85-112`): P0 = R/F/D/G + LCtrl/LShift/LAlt; P1 = arrows + RCtrl/RAlt/RShift; **dig unbound for both**. `N_WORMS = 2` (`:20`).
- `InputSource::sample(tick, &ButtonInput<KeyCode>) -> [ControlState; 2]` (`:278-289`) — once per FixedUpdate tick.
- **No general key-event path.** Everything polls `Res<ButtonInput<KeyCode>>`; the only edge reads are `just_pressed(F5)` and `Escape`. No text input, no key-repeat emulation. Menu navigation: `just_pressed` on Up/Down/Enter/Esc (matches existing style) plus a `KeyboardInput` reader for typing (profile names); the C++ weapsel key-repeat (12/3) must be emulated explicitly (deferred in 4f, `docs/…slice4f4g…md:146`). (**4½d:** the menus take key *events* — a `KeyEvent` queue with `KeyCode → DOS` and typed symbols, fed to `ui::keys::KeyLatch` — not `just_pressed`; `just_pressed(F5)` remains only for the Rust-only restart.) (**4½e-1:** the queue carries `ui::shell::InputEvent` — `Key(KeyEvent)` or `Text(String)`, in SDL order: a printing key-down is followed by one `Text` per `char` of its typed text (plan D8), and the phone text field's `window.lieroText` entries join it. A live match's worm input is C++ `OnKey`'s *edges* over the sampled words (`ui::keys::KeyEdges`): a bit the sim consumed stays clear while its key is held — `--live`'s non-shell path too.)
- No binding persistence, no rebinding UI, no per-worm profile: `default_bindings()` is the single hard-coded source. C++: `data/Profiles/*.toml` + `src/game/inputState.cpp`; DOS-scancode ↔ SDL tables in `src/game/keys.cpp`.

---

## 4. `scenario` crate

`scenario/`: `lib.rs`, `parser.rs` (852), `loader.rs` (256), `assets.rs` (103). Deps: `sim-core, assets, sim, render`; wasm-only `include_dir`.

**Format** (grammar `parser.rs:9-30`):
```
seed <u32>            level <path relative to TC root>     ticks <u32>
max_bonuses <i32>     game_mode <i32>
worm <index> <pos_x> <pos_y> <health> <lives> <stats_x> <visible>   # pos 16.16 fixed
input <tick> <w0_7bit> <w1_7bit>                                    # sparse
weapon <slot> <name> [ammo]
render <layout> | render_shadow | render_shake <t> <vp> <amt> | render_flash <t> <amt>
render_hud | render_live
```
**Not representable:** per-worm weapon loadouts (a `weapon` directive applies to *both* worms,
`loader.rs:127-138`), lives/health as *settings*, `loading_time`, `blood`, `shadow` as a real
setting, `map`, `time_to_lose`, `flags_to_win`, level *generation* parameters, worm names/colours.

**`scenario::load(tc_root, &Scenario) -> Loaded`** (`loader.rs:100-218`) is the *only* state builder.
It hard-codes `settings_weapons = [1u32; NUM_WEAPONS]` (`loader.rs:122`) then overrides slot 0 from
`weapon 0 <name>` for **both** worms; calls `SimState::new(...)` with `settings_loading_time = 0`,
`load_change = true`, `blood = 100`; post-assigns blood colours, `bobj_gravity`, `small_sprites`,
spawn-rect consts, min-spawn-dists, `game_mode` (`:172-182`).

**Gaps in `load` that matter for 4½** (fields left at defaults; the oracle harnesses set them
themselves, e.g. `oracle-tests/tests/sim_slice5c_golden.rs:243`):
- `settings_max_bonuses` / `bonus_drop_chance` / `bonus_*` consts / `weap_table` → **bonuses never spawn in the live game**.
- `state.sound_hooks` left `SoundHooks::default()` (all zeros) → `play_bump/alive/reloaded/ninjarope_throw` all fire sample index 0 (`"shotgun"`). **Confirmed live bug** — `TcConfig` *does* resolve the hooks (`assets/src/tc.rs:467-…`), including `MenuMoveUp`/`MenuMoveDown`/`MenuSelect`.
- `settings_health` stays 100, `settings_loading_time` 0 (instant reload), `laser_weapon` 0.

**Asset seam / wasm**: every TC read funnels through `scenario::assets::read_asset(tc_root, rel)`
(`assets.rs:16-27` native; `:48-103` wasm). The wasm embed is a curated manifest: `include_dir!` of
`sprites/`, `weapons/`, `nobjects/`, `sobjects/`, `sounds/` + `include_bytes!` of `tc.cfg` and the
single level `Levels/render_stage.lev` (`assets.rs:53-74`); a miss panics.

**Storage (4½a-2, grown in 4½e-1):** `scenario::storage::ConfigStore` is now `Send + Sync` (the shell owns one
inside a Bevy resource; `MemoryStore` uses a `Mutex`) and has `root_label()`, the config root as C++ prints it
(`FsNode::FullPath()`: no trailing separator, a relative root gains `./`; `MemoryStore`'s is `/openliero`, the C++
web build's root). `storage::load_setup` / `save_setup` do `gameEntry.cpp:55-58` and `:78`. `game::config` picks the
native store like C++ `paths::Resolve` (`--config-root`, else `OPENLIERO_TEST_USER_DIR` / `SDL_GetPrefPath` over
`OPENLIERO_DATADIR` / `data/`; no `portable.txt`); the browser's is a `MemoryStore` with the two shipped setups
(`scenario::assets::EMBEDDED_SETUPS`), for the session only. `ui::shell::level_path::read_level` resolves a
`level_file` through the store (a C++ config-root path — `root_label() + "/"` — through the store's merged view,
user layer then system layer, which is John's Q4 fix; else an absolute path natively; else TC-relative, the 4½d
`?level=` convention; plan fact 27 pulled it forward from 4½e-2), and `generate_level` takes the file as
`Option<LevelData>` — a missing file falls back to a random level, as C++ does, instead of panicking.

**Level paths / `data/` layout**: TC root = `concat!(CARGO_MANIFEST_DIR, "/../../data/TC/openliero")`
(`main.rs:36`, duplicated in `input.rs:791`, `shot`, tests). `Levels/` has only 5 test fixtures.
There is no shipped stock-level corpus for a level picker, and only one TC.

---

## 5. `sim` public surface for match setup

- **Constructor**: `SimState::new(level, worms_init, seed, material_flags, weapons, physics, control, h_signed_recoil, large_sprites, textures, sobject_types, nobject_types, settings_loading_time, load_change, blood) -> SimState` (`state.rs:1272-1288`). **No builder**; the convention is "post-`new` field assignment" for everything else so old call sites/goldens stay byte-identical. A 4½ `MatchConfig → SimState` builder should be a new layer in `scenario`, not a `new()` signature change — that convention is load-bearing for the goldens.
- `WormInit { index, health, lives, stats_x, weapons: [WeaponInit; 5], start_pos, visible }` (`state.rs:211-228`) + `WormInit::resolve_weapons(objects, weap_order, settings_weapons: &[u32;5])` (`state.rs:240-256`) — exactly the C++ `Worm::InitWeapons` mapping, i.e. **the weapon-selection primitive already exists**; the menu just has to produce the five 1-based indices instead of `[1;5]`.
- `process_frame(&mut self, inputs: &[ControlState])` `state.rs:1491`.

**Settings represented on `SimState`** (all unhashed, post-`new`-assignable):
`settings_loading_time` `:999`, `load_change` `:1003`, `blood` `:1009`, `settings_max_bonuses` `:1046`,
`bonus_drop_chance` `:1054`, bonus spawn rect + `h_bonus_*` hacks `:1063-1094`, `weap_table: Vec<i32>`
`:1100` (exists, never populated), bonus process consts, `settings_health` `:1158`, `game_mode: u32`
`:1171` (0 KillEmAll / 1 GameOfTag / 2 Holdazone / 3 Scales), `time_to_lose` `:1177`,
`last_killed_idx`, `got_changed`, worm spawn-rect consts, `sound_hooks` `:1226`.

**Missing vs C++ `Settings`**: `lives` (per-worm only via `WormInit.lives`), `flags_to_win`
(Holdazone), `shadow` (draw-time only), `names_on_bonuses`, `regenerate_level`, `random_level`,
`level_file`, `map`, `screen_sync`, `bonus_timeout`, `input_delay`, `random_map_width/height`,
`weap_table` *contents*, the whole `WormSettings` (name, colours, controls, per-worm weapon
choices), `GameplayExtensions`/`AppSettings`. Game mode 2 (Holdazone) is an explicit
`unimplemented!()` (`state.rs:2250-2252`).

**Match-over detection: none.** The game-mode tail switch (`state.rs:2226-2256`) only bumps the
GameOfTag "it" timer. The lives gate merely *skips processing* a worm with `lives <= 0`
(`state.rs:1917-1919`). No winner/game-over/stats state anywhere (no `StatsRecorder` analog).

---

## 6. `assets`

`assets/src/` (2346 lines): `level.rs`, `palette.rs`, `sprite.rs`, `tc.rs` (653), `object.rs` (710), `wav.rs`.

- `assets::level::load(bytes) -> Result<LevelData, LevelError>` (`level.rs:65`) — bytes, not a path; no directory listing, no level saving, **no random generation**.
- `assets::tc::TcConfig::load(bytes)` (`tc.rs:417`); `TcConfig { types, constants, materials, textures, bonuses, color_anim, aiparams, texts: Texts, hacks, sound_hooks }` (`tc.rs:321-333`). `Texts` (`tc.rs:206-246`) carries the forty `[texts]` strings of `tc.cfg` (`SelWeap, SelLevel, LevelRandom, LevelIs1/2, Randomize, Random, RegenLevel, ReloadLevel, Done, Weapon, Availability, NoWeaps, PressFire, PressAnyKey, OK, Copyright…`) — and none of `weap_states`, `controllers` or `key_names`. **Corrected in 4½d (design finding 3):** `onoff`, `game_modes`, `weap_states`, `controllers`, `input_devices` and `key_names` are hard-coded in C++ (`common.cpp:25`, `:205-225`), not `tc.cfg` texts; `ui::text` carries `onoff` and `game_modes` as constants (4½d T1), the rest land with the screens that show them. `aiparams` (DumbLieroAI's `k[state][control]` table) is parsed.
- `assets::object::Objects::load(&tc.types, read_fn)` gives `objects.weapons[i].name/ammo/id` — the weapon-menu list source.
- **Nothing for listing/choosing**: no level enumeration, no TC enumeration, no path helpers.

---

## 7. Audio (menu sounds)

`game/src/audio.rs`: `trait AudioSink { play_one_shot(&mut self, sound: i32); play_loop(..); stop_loop(..) }`
(`:37-51`); impls `NullSink` `:53`, `MockSink` `:63-90`, `RodioSink::try_new(SoundTable)` `:244-287`
(same struct on native and wasm). `SoundTable = Vec<Vec<i16>>` `:184`, built by
`load_sound_table(tc_root, &tc.types.sounds)` `:193` / `load_sound_table_wasm` `:222`.
`Drainer<S>` `:93-170` only consumes `sim` events; a menu calls the sink directly with
`TcConfig.sound_hooks.MenuMoveUp / MenuMoveDown / MenuSelect` (`assets/src/tc.rs:297-306`).
Wasm: WebAudio starts `suspended` until a user gesture (`main.rs:510-522`) — a menu keypress is the
ideal unlock point.

---

## 8. wasm

- **No CLI args, no filesystem**: `resolve_scenario` wasm arm hard-codes `Mode::Scripted` + `blood`; scenario text via `include_str!`; TC assets from the `include_dir!` manifest — **only one level is embedded**, so a level picker on wasm needs the manifest widened.
- **Settings persistence in the browser needs `web_sys::Storage` (localStorage)** — nothing exists today; native has no persistence either.
- **Keyboard is not wired on wasm** — Scripted-only by design (`docs/…slice4f4g…md:131`). A menu on wasm implies enabling Live-style key polling there, which retires the wasm debug frame/state self-check (`main.rs:697-725`) for that path.
- CI: build-only wasm gate (`.github/workflows/rust.yml:216-229`). Dev loop via `rust/.cargo/config.toml` (`wasm-server-runner`); static bundle recipe in `web/index.html`.

---

## 9. Testing / oracle patterns (what a new slice copies)

1. **C++ dumper directive** — `src/tools/oracle_dump/` (`sim_physics_dump.cpp` is the workhorse). The dumper parses **the same scenario text file** as Rust; new capabilities are new *directives* with an "absent ⇒ old behaviour" default so every existing golden stays byte-identical (`sim_physics_dump.cpp:42-74`). Both parsers move together (`scenario/src/parser.rs:88-96`).
2. **Generator script** — `oracle-tests/gen_*.sh` (50 of them): `cmake --preset $PRESET -DOPENLIERO_BUILD_ORACLE_DUMP=ON`, build, loop scenarios, write `golden/<name>_sim.txt` + sidecar. Local/manual.
3. **Rust golden test** — `oracle-tests/tests/*.rs` + shared harness modules. Sidecar grammar: `<tick> <frame_hash_hex16> <state_hash_hex8>` per line + `total <n> <acc_hex16>`.

**`game`-crate gates** (headless, `cargo test -p game`): `passthrough.rs`, `round_trip.rs`
(record via Live sampler → `to_text` → `parse` → replay via Scripted), `record_regression.rs`,
`viewport_stepping.rs`. CI: `cargo test --workspace --exclude game` + `cargo test -p game` + wasm build.

**Run-skill** — `.claude/skills/liero-shot/SKILL.md`: `shot` screenshot/compare loop; §7 documents
the `game` binary's live/record/replay loop. **A 4½ slice extends §7 (menus/level select) and adds
scenarios/goldens the same way.**

**Pattern implication for menus**: menus are not hashed state, so the sim oracle doesn't apply — but
the *frame hash* does. A "menu golden" is `render::hash::hash_frame(&surface, 33)` of a menu drawn
at a fixed state, with no C++ counterpart (the C++ menu is `Gfx`-driven and not dumpable). Expect a
Rust-only self-golden (regression) plus `shot`-style PNG eyeballing against the C++ build.

---

## 10. Rough LOC per crate

| crate | `src/` | tests + examples | notes |
|---|---:|---:|---|
| `sim-core` | 447 | 0 | fixed/vec/rng/math/tables |
| `assets` | 2 346 | 0 | tc 653, object 710, level 421 |
| `sim` | 21 837 | 2 205 | state.rs 6 069, weapon 3 327, control 2 556, nobject 2 383 |
| `render` | 5 193 | 0 | object_draw 1 199, blit 1 033, hud 769, font 486, frame 445 |
| `scenario` | 1 226 | 0 | parser 852, loader 256, assets 103 |
| `replay` | 680 | 121 | `.lrp` phase-1 reader |
| `shot` | 748 | 218 | headless CLI |
| `game` | 2 527 | 1 042 | main 892, input 881, audio 516 |
| `oracle-tests` | 8 | 12 830 | ~50 golden tests + 50 gen scripts |
| **total** | **~35 000** | **~16 400** | |

---

## Cross-cutting notes for the 4½ overview

1. **No `States`, no plugin structure** — `main.rs` is one flat app. Introducing the screen stack is the single biggest structural change; keep `tick_and_render` the only `Sim` mutator to preserve the determinism firewall.
2. **`scenario::load` is the only state builder** and it hard-codes weapons, skips bonuses/sound-hooks/health/loading-time. Weapon + level selection means either (a) widening the scenario format with new directives (keeps dumper/golden symmetry) or (b) introducing a `MatchConfig` that *produces* the sim state (keeps the format frozen). **Decided: (b)** — every committed golden stays byte-identical with zero dumper work; directives are added only where a sim oracle needs them.
3. **`TC_ROOT` is a compile-time const duplicated in 5 places** — a settings/paths layer should centralise it.
4. **Random level generation is unported**; the C++ source is `level.cpp:11-135`, `:397-428`, driven by `Rand` + `DrawDirtEffect` + `large_sprites` — all three primitives already exist on the Rust side (`sim_core::rng::Rand`, `sim::blit::draw_dirt_effect`, `SpriteSet`), so it is portable and oracle-testable (seed → material-map hash golden, analogous to `golden/level.txt`).
5. **No settings persistence at all**; C++ uses toml++ (`src/game/serialization/`, `data/Setups/liero.cfg`, `data/Profiles/*.toml`) — a ready-made schema.
6. **Confirmed bug to fold in:** `state.sound_hooks` is never assigned by `scenario::load`.
