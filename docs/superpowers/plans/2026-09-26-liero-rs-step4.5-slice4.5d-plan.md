# Step 4½, Slice 4½d — the menu framework, `ScreenStack` and the main menu (THE MILESTONE): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** Rust boots like C++: a main menu drawn over a generated level, NEW GAME, weapon selection, play, Esc to a pause menu with RESUME, and QUIT. A Bevy-free menu framework and screen stack drive it. Every presented frame on that path is bit-exact against the **real C++ `Gfx::RunOneFrame`**, run headlessly in this cloud session. This is the Step 4½ milestone: bare run → main menu → NEW GAME → weapon selection → play → Esc → menu (RESUME / NEW GAME) → QUIT.

**Architecture:** A new Bevy-free crate `rust/ui` sits between `scenario` and `game` (`sim-core ← sim ← render ← scenario ← ui ← game`; `shot` and `oracle-tests` also depend on `ui`). It holds:
- `ui::menu`: `Menu`, `MenuItem`, `MenuModel`, the behavior family and type-to-search. Each is a line-for-line port of `menu.cpp`, `menuItem.cpp` and the `*Behavior.cpp` files.
- `ui::keys`: `KeyLatch` (the `dos_keys` table plus `key_buf`), the DOS constants, and `ReleaseLatch` (moved out of `game`).
- `ui::text`: `game_modes`, `onoff`, the `TimeToString` family and `UiTc`.
- `ui::shell`, which contains:
  - `ScreenStack` and `Screen`;
  - `MainMenuState`;
  - the main and settings menus, where the settings menu is display only;
  - the router, with `LevelSlot` and `SeedSource`;
  - `Match`, the `LocalController` analog;
  - `Shell`, the `Gfx` analog: one `frame()` per C++ `RunOneFrame`, `menu_cycles`, the play renderer's fade, and the shared frozen screen;
  - the 4½c modules `new_game`, `selection`, `match_flow` and `viewport_step`, moved from `game` and re-exported there.

`render` gains four things:
- the CP437 high half (hash-neutral);
- the `MenuItem::Draw` value arm;
- the scrollbar;
- `menu_palette` and `present::fade_argb`.

Two new C++ dumpers run the real C++ headlessly, on the 4½c Addendum A1 pattern:
- `oracle_dump_menu` (G1) drives real `Menu` subclasses, the real behaviors and the real `SettingsMenu`.
- `oracle_dump_shell` (G2) drives the real `Gfx::RunOneFrame` with a software SDL renderer and eight documented interventions.

The Rust side replays each script and must match every line. `game` becomes Bevy glue only:
- an event queue with `KeyCode → DOS`;
- touch edges, plus a new MENU (Esc) button;
- `Shell::frame` called from `tick_and_render`, with the faded present;
- QUIT: `AppExit` natively, and a black frame plus a "Play again" overlay in the browser;
- `?menu=1`;
- F5 as a Rust-only restart.

**Tech Stack:**
- Rust 2021 for `sim-core`, `sim`, `render`, `scenario`, `shot` and `oracle-tests`; Rust 2024 for `game` and the new `ui`.
- C++ in `src/tools/oracle_dump/`, preset `linux-x64` here, with clang-format 22.1.0 via `uvx`.
- HTML/JS in `web/index.html`.
- GitHub Actions YAML in `.github/workflows/preview.yml`.

**Spec:** `docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md`, cited as **design §N**, with its findings cited as **finding N**.

**Rulings (John, 2026-09-26)** (the design's §12 records them):
- **Q2:** show all 15 main-menu items. The unported ones stay inert.
- **Q3:** browser QUIT stops on the black frame and shows a "Play again" overlay that reloads the page.
- **Q4:** `?weapons=`, `?level=` and `?seed=` skip the menu. A plain link opens the main menu, and `?menu=1` forces it.
- **Q8:** the touch controls gain a MENU (Esc) button.
- **Q1:** use the Bevy-free `rust/ui` crate.
- **Q5:** F5 is a Rust-only restart during play and selection.
- **Q6:** recording menu-driven matches is postponed.
- **Q7:** loading and saving `liero.cfg` moves to 4½e.
- **Q9–Q14** follow the design's recommendations.

Overview: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md`.

## Plan-time facts checked against the source (the source wins)

Every task below was written against the current source (HEAD `a61cbb6`). Where the design and the source disagree, the plan follows the source, and T14 records each item in PROGRESS.

1. **`--live` alone is not the default match.** `parse_args` maps a *bare* invocation to `Mode::Live` + `DEFAULT_MATCH`. `--live` without a name is Live over the `blood` scenario (`game/src/input.rs:206-265`). The design's "bare `cargo run -p game` / `--live` → main menu" (§1 done-when 4, the §7.5 table) is therefore only true for the bare run. **The shell boots only for `Mode::Live && name == DEFAULT_MATCH && --record absent`**, which is exactly the 4½c `new_game_start` condition. `--live [<scenario>]` keeps the 4½c scenario-live path unchanged: selection via `live_config`, F5, and Esc quits.
2. **C++ acts on every non-live main-menu item and on the F-keys; "inert" is Rust-only behavior.** In `mainMenuState.cpp:199-270`:
   - MATCH SETUP → `cur_menu = settings_menu`;
   - LEFT/RIGHT/NETWORK PLAYER → `PlayerSettings`;
   - OPTIONS → the hidden menu;
   - REPLAYS/TC → push a selector state;
   - JOIN LAN / JOIN ONLINE → push an `InputStringState`;
   - HOST ONLINE → `pending_menu_selection` → the router on the next frame;
   - HOST LAN falls into `default:` → `selected_ = kMaHostGame` → fade-out → the router (`gfx.cpp:1551-1554`).

   F2/F3/F5/F6/F7/F9 move the cursor and switch menus or push states (`:432-460`). So the design's G2 `nav` pins "placeholder Enters (sounds only); F2/F7 ignored" (§6.4) **cannot be gated against C++**. The corpus never selects a placeholder and never presses those F-keys, and the generator refuses a script that does (T9). Rust's inert placeholders are unit-tested only (T7). The "second `MenuSelect`" belongs to exactly **JOIN LAN, HOST ONLINE and JOIN ONLINE** (`:234`, `:248`, `:254`). HOST LAN plays one `MenuSelect`, because it goes through `default:`. Design §4.11's "the three network arms" means those three.
3. **A file level resolves against the process CWD in C++ and against the TC in Rust.** C++ uses `FsNode(path).ToReader()` with the raw `settings.level_file` (`level.cpp:401-411`). Rust uses `read_asset(tc_root, level_file_name(..))` (4½b). So `oracle_dump_shell` runs its frames with **CWD = `data/TC/openliero`**. This is intervention 8, new and documented, and it sits where no real code runs. The dumper refuses a setup whose level file does not open from there, because a silent C++ fallback to a random level would otherwise look like a Rust bug.
4. **`render::palette::build_palette` is already public.** Design §4.1's "`frame` exposes its palette" is not needed. The shell recomputes the `pal32` a game draw leaves behind with `build_palette(origpal, color_anim, state.cycles, screen_flash)`, exactly as `frame::draw` does (`frame.rs:60`).
5. **`menu_palette` is not an alias of `weapsel_palette`.** `UpdateMenuPalettes` applies `SetWormColours` after the rotation (`gfx.cpp:988-990`); `UpdateWeapselPalette` does not (`weapsel.cpp:20-24`). `render::weapsel::weapsel_palette` stays as it is. The new `render::menu::menu_palette` is the rotation plus `set_worm_colour` ×2. The two are equal whenever `origpal` already carries the ramps (after `Game::Focus`), but the port stays faithful.
6. **The copyright string** is `"Liero v1.33 (c) MetsänElämet 1998,1999"` (`tc.cfg:244`): "MetsänElämet", with **two** `ä`, not the design's "MetsänEläket".
7. **`SettingsMenu`'s item order differs from its id order.** The ids are `gfx.hpp:73-93` (`kSiLives` = 1). The items are added in `LoadMenus` order (`gfx.cpp:485-503`), where LIVES is item 5. There are 19 items. The plan pins both orders.
8. **No `arrayvec` dependency.** `KeyLatch::buf` is a `Vec<TypedKey>` capped at 32 (design §4.6 says `ArrayVec`).
9. **C++ `StartGame` plays `SoundBegin`** (`game.cpp:500-503`) on the frame weapon selection ends. Rust never did. `Match` now pushes `sound_hooks.Begin` into the frame's menu sounds when selection finishes, and G2's sounds column gates it. The live game now plays the "begin" sample at match start.
10. **The G2 golden format gains two things** (design §6.3 refined):
    - an **`<upd>` column** (`M`/`W`/`G`: the screen that ran `Update` this frame, read *before* the frame). A pop frame's sounds belong to the screen that updated, which `<top>` (after the frame) cannot name.
    - the `boot` line also carries `bmp16 fade menu_cycles top sel`.

    The script gains an `expect quit|frames` directive for the generator's awk gate.
11. **Small C++ quirks, ported verbatim:**
    - `Menu::OnEnter` returns `false` (0) when nothing is selected (`menu.hpp:81-87`).
    - `Menu::Draw` treats *any* negative `x` as "use my own x" (`menu.cpp:86-88`). Rust takes `x: i32`, not `Option<i32>`.
    - `Menu::AddItem(item, pos)` returns the size *before* the insert (`menu.cpp:311-317`).
    - Type-to-search tests `visible`, not `selectable` (`menu.cpp:40`).
    - `contains` with an empty needle matches any non-empty string (`std::ranges::search`, `menu.cpp:44-47`).
12. **The worm control state goes stale across the pause.** C++ worms keep whatever bits they had when play lost the event feed, and key-downs or key-ups in the menu never reach them. Rust samples levels. The release latch covers keys pressed in the menu (design §4.11). A key held through Esc and released in the menu differs, and only in the live game. The corpus refuses it: every back-to-menu pop frame must have no worm key held (T9). T14 records the live divergence.
13. **`Selection` owns its frozen screen and its own `menu_cycles` today** (`WeapselScreen`, `game/src/selection.rs:79-86`). It becomes `cached_background` + `focused`, and reads both from the shell (T7). The 4½c render gate is unaffected, because `oracle-tests/tests/render_weapsel_golden.rs` drives `render::weapsel` directly.
14. **More modules must move than the design lists.**
    - `Match::process` needs `viewport_step::tick_viewports`.
    - `Match::draw` needs `HudFlags`, whose `hud_flags(Mode, …)` stays in `game` because `Mode` is Bevy-coupled.
    - NEW GAME re-applies `?weapons=` through `web_params::apply_weapons`.

    All three move to `ui`, and `game` re-exports them (T6).
15. **The selection tests read the default-match fixture through `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios/default_match.txt"))`.** After the move the path is `"/../game/scenarios/default_match.txt"` (T6).
16. **`MatchFlow::weapsel_frame` only counts up.** The shared tail (`localController.cpp:185-199`) replaces it, together with `esc()`, `focus()` and `check_game_over()` (T7). `after_frame` keeps its semantics, as `check_game_over` + `tail`.
17. **4½c's `NewGame` is superseded by `LevelSlot` + `SeedSource`** (T7). `new_game::generate_level` and `start_settings` stay. The `NewGame` tests are ported to `LevelSlot`.
18. **Exactly one existing test sees the CP437 change.** `render/src/font.rs` asserts `get_dims("A\u{1}\u{e9}") == w('A')` ("skipped"). `é` is CP437 0x82 and is now drawn (T0). Every golden is ASCII-only, so the re-diff is hash-neutral.
19. **At a low frame rate the browser can lose a tap.** A fixed tick that takes a key's down *and* up in one poll sets and clears the menu flag before `Update` runs. C++ has the same property, but it polls at 70 Hz, while headless Chromium runs at about 7 fps. The glue's key queue therefore **defers a key's second event in one tick to the next tick** (T11). This is presentation only. G2 feeds events per frame and never hits it.
20. **`clang-tidy-diff.sh` defaults to `origin/master`, which this clone lacks.** Every tidy run passes the slice base: `scripts/clang-tidy-diff.sh build/linux-x64 a61cbb6`.
21. **The spectator draws are side-effect free for the gate.** `GamePlayState::Draw` also draws `single_screen_renderer` (`gamePlayState.cpp:98-103`), and `MainMenuState::Enter` draws a minimap into it. Both touch only that renderer and the spectator viewport's own `rand` (`spectatorviewport.cpp:133-136`), never `game.rand` or `play_renderer`. So the headless dumper keeps them, and Rust does not port them (finding 2 extended).

## Standing ruling (4½c Addendum A, carried forward)

The C++ comparison happens **here**, against the real C++ code run headlessly (A1). It is never left to a macOS eyeball. If a dumper cannot run the real code headlessly, the task **stops and reports the exact blocker**. It never weakens a gate, and never regenerates a golden to paper over a mismatch. The real `openliero` runs under Xvfb + xdotool for C++ | Rust side-by-side PNGs (A2; T12 here). Those PNGs are eyeball artefacts, not gates, and are never committed.

## Global Constraints

- **Repo and branch.** Repo `/home/user/openliero`, branch `claude/cpp-oracle-vcpkg-assets-chcwcm` (PR `hamiltoon/openliero#15` into `liero-rs-step-4-5`). The slice base is `a61cbb6`. Use absolute paths. There are no Bash-hygiene restrictions. No sub-subagents.
- **Cargo.** Run from `/home/user/openliero/rust`, in **DEBUG only**. Never use `--release`, because some tests are `#[should_panic]` on `debug_assert!`s.
  - **The re-diff** is `cargo test --workspace --exclude game`, then `cargo test -p game`. The wasm gate is `cargo build -p game --target wasm32-unknown-unknown`.
  - The only non-test build is T13's preview bundle, which uses the preview's own `--profile wasm-release`.
- **Disk.** About 4 GB is free, so do not create new build trees: no `--target-dir`, no second CMake preset, no copy of `build/`. T12 and T13 write only small PNG/PPM files into the scratchpad.
- **C++ build.**
  - Before ANY cmake or gen-script call, `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh`. It sets `PRESET=linux-x64`, the git-based vcpkg asset source and the sdl3 overlay.
  - `build/linux-x64` is already configured with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON` and exports `compile_commands.json`. The binaries go to `build/linux-x64/Release/`.
  - Gen scripts default to `macos-arm64` and honour `PRESET`. Run each one in the same shell as the `source`: `source …/env.sh && bash /home/user/openliero/rust/oracle-tests/<script>.sh`.
- **clang-format and clang-tidy.**
  - **clang-format**: for every C++ file touched, run `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file <abs file>` on the whole file.
  - **clang-tidy**: run `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 a61cbb6`. It must be clean. Fix the code. Add a `NOLINTNEXTLINE(<check>) — <reason>` only where the repo already does so for the same check (for example `android-cloexec-fopen` in `weapsel_dump.cpp`).
- **Determinism.** Use no floats, no wall clock and no `HashMap` iteration in `ui::menu`, `ui::keys` or `ui::shell`. The only clocks are values the caller passes in: `ShellInput::fresh_seed` and `now_ms`. C++ `unsigned` wrap is Rust `wrapping_*` on `u32`.
- **LD 3.** `tick_and_render` stays the only system with `ResMut<Sim>`. Inside the shell, only `Match::process` and the router get `&mut SimState`, and the router only *replaces* it (`*sim = state`). `MainMenuState` gets a `MenuCtx` with no path to the sim.
- **LD 4 / LD 5.** The scenario grammar is unchanged (no new directive) and `SimState::new` is unchanged.
- **Crate rules.**
  - `sim-core` stays dependency-free.
  - `sim` gains no dependency.
  - `ui`, `render`, `scenario` and `shot` stay Bevy-free.
  - `ui` adds no external dependency.
  - `cargo tree -p ui -e normal | grep -c bevy` must print `0`.
- **Golden audit.** `git -C /home/user/openliero diff --name-status a61cbb6 -- rust/oracle-tests/golden` may list ONLY `A` lines, and only these files:
  - `menu_<s>_script.txt` + `menu_<s>.txt` for the 15 G1 scripts;
  - `menu_settings_{gametag,holdazone,scales,file_level}_setup.cfg`;
  - `shell_<case>_script.txt` + `shell_<case>.txt` for the 11 G2 cases;
  - `shell_{level_file,gametag,holdazone_boot,regenerate,game_over}_setup.cfg`.

  That is **61 `A` lines**. No existing dumper is modified, so no existing golden is regenerated. If `git status --porcelain -- rust/oracle-tests/golden` ever shows an `M`, stop: restore with `git -C /home/user/openliero checkout -- rust/oracle-tests/golden`, report the file and the first differing line, and do not commit.
- **C++ changes** are confined to:
  - `src/tools/oracle_dump/menu_dump.cpp` (new);
  - `src/tools/oracle_dump/shell_dump.cpp` (new);
  - four lines inside the `OPENLIERO_BUILD_ORACLE_DUMP` block of `CMakeLists.txt` (`:372-398`).

  Both new dumpers `#include "weapsel_drive.hpp"` for `RecordingSoundPlayer` and `Fail`, but never modify it. They copy the small FNV/PPM helpers locally.
- **Frozen provenance.** Never edit the 4½a/4½c generators, common modules, dumpers or goldens:
  - `examples/gen_slice4_5a.rs`, `gen_slice4_5c0.rs`, `gen_slice4_5c.rs`;
  - `tests/weapsel_common/`, `tests/sim_slice4_5c0_common/`;
  - `weapsel_dump.cpp`, `weapsel_drive.hpp`, `sim_physics_dump.cpp`;
  - `golden/weapsel_*`, `golden/sim_slice4_5*`.
- **rustfmt.** Run it only on files this plan CREATES, and on the moved files under `rust/ui/` (they were formatted as 2024 in `game`, so it is a no-op there): `rustfmt --edition 2024 <abs file>` for `ui` and `game`, `--edition 2021` elsewhere. Never run rustfmt on an existing file outside `rust/ui/`, or on a `lib.rs`/`main.rs`, because it recurses. Hand-format edits to the surrounding style.
- **The browser pieces keep working:** `game::web_params`, `game::touch`, `game::hud_mode`, `.github/workflows/preview.yml` and `web/index.html`. `?demo`, Scripted, `--replay`, `--live --record` and `--live [<scenario>]` are unchanged. `game/tests/round_trip.rs` and `record_regression.rs` stay green and untouched.
- **Commits.**
  - Use the globally configured identity; do not override it.
  - Every commit message carries two trailer paragraphs as extra `-m` arguments:
    - `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, naming the model that did the work (`Claude Opus 5.5` or `Claude Sonnet 5`, per the task's tier);
    - `Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT`.
  - Never write "Generated with Claude Code" anywhere.
- **Do NOT push and do NOT open a PR.** The controller pushes; task executors never push.

## Model tiers

- **[Opus]:** T2 (the `Menu` port), T3 (behaviors, search and the settings display), T4 (C++ `oracle_dump_menu`), T5 (the G1 corpus and gate), T7 (`ui::shell`), T8 (C++ `oracle_dump_shell`), T9 (the G2 corpus and generator), T10 (MILESTONE), T11 (live wiring), T14 (broad review). Every reviewer is Opus.
- **[Sonnet]:** T0 (the `render` additions, fully specified), T1 (`ui` skeleton, `text` and `keys`), T6 (the mechanical module moves), T12 (the Xvfb side-by-side), T13 (headless Chromium, nice-to-have).

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `rust/render/src/font.rs` (modify) | T0 | `HIGH_HALF` + `codepoint_to_font_byte` replace `ascii_to_font_byte` |
| `rust/render/src/menu.rs` (modify) | T0 | `draw_item_value` (value arm), `draw_item` delegates; `draw_scrollbar`; `menu_palette` |
| `rust/render/src/present.rs` (create), `lib.rs` (modify) | T0 | `fade_argb`, `fade_into_rgba` |
| `rust/Cargo.toml` (modify) | T1 | workspace member `ui` |
| `rust/ui/Cargo.toml`, `src/lib.rs` (create) | T1 | the crate |
| `rust/ui/src/text.rs` (create) | T1, T3 | `GAME_MODES`, `ONOFF`, `time_to_string*`, `leaf_basename`; `UiTc` (T3) |
| `rust/ui/src/keys.rs` (create) | T1, T6 | DOS constants, control indices, `TypedKey`, `KeyLatch`, `reset_left_right`; `ReleaseLatch` (T6, moved) |
| `rust/ui/src/menu/mod.rs`, `behavior.rs`, `search.rs` (create) | T2, T3 | `MenuItem`, `Menu`, `MenuModel`, `PlainModel`, `MenuCx`, `MenuHooks`; `Behavior`, `Integer`, `CustomBehavior`, `Enter`, `ValueEntry`; `Search` + `Menu::on_keys` |
| `rust/ui/src/shell/mod.rs` (create) | T3, T6, T7 | module root; `HudFlags` (T6); `Shell`, `ShellInput`, `KeyEvent`, `FrameOut`, `Present`, `Phase`, `Route` (T7) |
| `rust/ui/src/shell/main_menu.rs`, `settings_menu.rs` (create) | T3, T7 | `main_menu()`, ids, `MainModel`; `settings_menu()`, ids, `SettingsModel`, `LevelSelect`, `OptionsSave`; `MainMenuState`, `MenuWorld`, `MenuCtx` (T7) |
| `rust/ui/src/shell/{match_flow,new_game,selection,viewport_step}.rs` (git mv from `game/src/`) | T6, T7 | moved unchanged (T6), then extended (T7) |
| `rust/ui/src/shell/loadout.rs` (create; body moved from `game/src/web_params.rs`) | T6 | `apply_weapons` |
| `rust/ui/src/shell/level_slot.rs`, `playing.rs`, `stack.rs` (create) | T7 | `LevelSlot`, `LevelProvenance`, `SeedSource`; `Match`, `StartOptions`, `boot_state`, `draw_boot`; `ScreenStack`, `Screen`, `AfterUpdate` |
| `rust/game/Cargo.toml`, `src/lib.rs`, `input.rs`, `hud_mode.rs`, `web_params.rs` (modify) | T6, T11 | `ui` dep + re-exports; `dos_of_keycode`, `typed_of_keycode`, `KeyQueue`; `?menu=` |
| `rust/game/src/touch.rs`, `blit.rs`, `main.rs` (modify) | T11 | `TOUCH_MENU`, `touch_key_events`; faded upload; the shell path |
| `src/tools/oracle_dump/menu_dump.cpp` (create) | T4 | `oracle_dump_menu` (G1) |
| `src/tools/oracle_dump/shell_dump.cpp` (create) | T8 | `oracle_dump_shell` (G2) |
| `CMakeLists.txt` (modify, oracle block) | T4, T8 | 2 + 2 target lines |
| `rust/oracle-tests/Cargo.toml` (modify) | T5 | dev-dep `ui` |
| `rust/oracle-tests/tests/menu_common/mod.rs`, `menu_widget_golden.rs` (create) | T5 | G1 script parser + Rust replay + the gate |
| `rust/oracle-tests/gen_menu_golden.sh` (create) | T5 | C++ G1 goldens + awk gates |
| `rust/oracle-tests/golden/menu_*` ×34 (create) | T5 | 15 scripts, 4 setups, 15 goldens |
| `rust/oracle-tests/tests/shell_common/mod.rs` (create) | T9, T10 | G2 script model, `Script` builder, the case table, validators, the Rust driver, line format |
| `rust/oracle-tests/examples/gen_slice4_5d.rs` (create) | T9 | `check` / `write` |
| `rust/oracle-tests/gen_shell_golden.sh` (create) | T9 | C++ G2 goldens + awk gates |
| `rust/oracle-tests/golden/shell_*` ×27 (create) | T9 | 11 scripts, 5 setups, 11 goldens |
| `rust/oracle-tests/tests/shell_golden.rs` (create) | T10 | 🎯 MILESTONE: every G2 case line for line |
| `rust/shot/Cargo.toml`, `src/lib.rs` (modify) | T11 | `--menu [--frames N] [--seed S]` |
| `web/index.html`, `.github/workflows/preview.yml` (modify) | T11 | MENU button, phase hint, quit overlay, help/preview text |
| `.claude/skills/liero-shot/SKILL.md`, PROGRESS, overview, design, cpp-map, rust-map (modify) | T14 | status + corrections |

## Task dependency map

```
T0 ─┐
T1 ─┴─> T2 ─> T3 ─┬──────────────> T5 (G1 gate) ───────────────────────────┐
T4 (C++, ∥) ──────┘                                                        │
T6 (moves) ─> T7 (ui::shell; needs T3) ─┬─> T9 (G2 corpus; needs T8) ─> T10 🎯 ─┤
T8 (C++, ∥) ────────────────────────────┘                                   │
                                        └─> T11 (live) ─> T12 (Xvfb) ───────┼─> T14
                                                       └─> T13 (Chromium, nice-to-have) ┘
```

- T4 and T8 are C++ only and can run in parallel with T0–T3 and T6–T7.
- T5 needs T3 and T4.
- T7 needs T3 (the menus) and T6 (the moved modules).
- T9 needs T7 (the generator drives the Rust shell to validate each case) and T8 (the goldens).
- T11 needs T7.
- T14 is last.

| Design §9 | This plan |
|---|---|
| T0 | T0 |
| T1 | T1 + T2 |
| T2 | T3 |
| T3 | T4 |
| T4 | T5 |
| T5 | T6 + T7 |
| T6 | T8 |
| T7 | T9 |
| T8 | T10 |
| T9 | T11 |
| T10 | T12 + T13 |
| T11 | T14 |

---
### Task 0: `render` — the CP437 high half, the value arm, the scrollbar, `menu_palette`, `present::fade_argb`  [Sonnet]

**Files:**
- Modify: `rust/render/src/font.rs` (`draw_string` `:191-221`, `get_dims` `:227-243`, `ascii_to_font_byte` `:245-260`, the test at `:285-292`)
- Modify: `rust/render/src/menu.rs` (the module doc, `draw_item`, new fns, tests)
- Create: `rust/render/src/present.rs`
- Modify: `rust/render/src/lib.rs` (`pub mod present;` after `pub mod palette;`, plus a doc sentence)

**Interfaces:**
- Produces (used by T2, T3, T7, T11):
  - `pub fn render::menu::draw_item_value(bmp: &mut Bitmap, pal: &Pal32, font: &Font, text: &str, value: Option<&str>, x: i32, y: i32, selected: bool, disabled: bool, centered: bool, value_offset_x: i32, colours: ItemColours)`. `draw_item` keeps its signature and becomes `draw_item_value(.., None, .., 0, ..)`.
  - `pub fn render::menu::draw_scrollbar(bmp: &mut Bitmap, pal: &Pal32, font: &Font, x: i32, y: i32, height: i32, item_height: i32, top_item: i32, visible_item_count: i32)`.
  - `pub fn render::menu::menu_palette(origpal: &Palette, menu_cycles: u32, worm_rgb: [[i32; 3]; 2]) -> Pal32`.
  - `pub fn render::present::fade_argb(c: u32, amount: i32) -> u32`.
  - `pub fn render::present::fade_into_rgba(bmp: &Bitmap, amount: i32, out: &mut [u8])`.
- Consumes: `render::blit::draw_rounded_box`, `render::palette::{rotate_from, set_worm_colour, pack_pal32}`, `render::hash::fade_channel`, `Font::{draw_char, draw_string, get_dims}`, `Bitmap::fill_rect`.

Why:
- Finding 4: the copyright bar draws `ä` (twice, plan-time fact 6), and the C++ font maps every codepoint through `cp437::UnicodeToByte` (`cp437.cpp:177-187`, `font.cpp:51-54`).
- The settings menu on the main screen needs the `has_value` arm of `MenuItem::Draw` (`menuItem.cpp:6-42`).
- Holdazone and every long 4½e list need the scrollbar (`menu.cpp:107-124`).
- The menu palette is `UpdateMenuPalettes` (`gfx.cpp:986-990`, plan-time fact 5).
- The live game shows its first fade through `fade_argb` (`FadeArgb`/`ScaleDraw`, `blit.cpp:788-826`).

The whole task is hash-neutral: every golden string is ASCII. The CP437 change is re-diffed on its own, as design §6.8 asks.

- [ ] **Step 1: Write the failing tests**

(a) In `rust/render/src/font.rs`, replace the test lines

```rust
        assert_eq!(
            font.get_dims("A\u{1}\u{e9}"),
            w('A'),
            "skipped exactly as draw_string skips"
        );
```

with

```rust
        // cp437::UnicodeToByte (cp437.cpp:177-187): U+00E9 is CP437 0x82 and is DRAWN (Step
        // 4½d); U+0001 fails the 2..252 gate; U+20AC (€) has no CP437 byte and is skipped.
        let hi = |b: u8| font.chars[b as usize - 2].width;
        assert_eq!(font.get_dims("A\u{1}\u{e9}"), w('A') + hi(0x82));
        assert_eq!(font.get_dims("A\u{20ac}"), w('A'), "no CP437 byte: skipped");
```

and append inside `mod tests`:

```rust
    #[test]
    fn codepoints_map_through_the_cp437_table() {
        // cp437.cpp:11-44 (kHighHalf) + :177-187 (UnicodeToByte) + font.cpp:51-54 (1 = skip).
        assert_eq!(codepoint_to_font_byte('A'), b'A');
        assert_eq!(codepoint_to_font_byte('\u{7f}'), 0x7f);
        assert_eq!(codepoint_to_font_byte('\u{c7}'), 0x80, "Ç");
        assert_eq!(codepoint_to_font_byte('\u{e4}'), 0x84, "ä (tc.cfg Copyright2)");
        assert_eq!(codepoint_to_font_byte('\u{e9}'), 0x82, "é");
        assert_eq!(codepoint_to_font_byte('\u{2591}'), 0xb0, "░");
        assert_eq!(codepoint_to_font_byte('\u{a0}'), 0xff, "the last entry");
        assert_eq!(codepoint_to_font_byte('\u{20ac}'), 1, "no CP437 byte");
        assert_eq!(HIGH_HALF.len(), 128);
    }

    #[test]
    fn the_copyright_string_measures_its_umlauts() {
        let font = real_font();
        let w = |b: u8| font.chars[b as usize - 2].width;
        let want: i32 = "Mets".bytes().map(w).sum::<i32>() + w(0x84) + w(b'n');
        assert_eq!(font.get_dims("Mets\u{e4}n"), want);
    }
```

(b) In `rust/render/src/menu.rs`, append inside `mod tests`:

```rust
    #[test]
    fn draw_item_is_the_value_arm_without_a_value() {
        for (sel, dis, cen) in [(false, false, false), (true, false, true), (true, true, false)] {
            let mut a = Bitmap::new(80, 12);
            let mut b = Bitmap::new(80, 12);
            draw_item(&mut a, &ramp_pal(), &real_font(), "DONE!", 40, 2, sel, dis, cen, DONE);
            draw_item_value(
                &mut b, &ramp_pal(), &real_font(), "DONE!", None, 40, 2, sel, dis, cen, 77, DONE,
            );
            assert_eq!(a, b);
        }
    }

    #[test]
    fn a_selected_value_gets_its_own_box_centred_on_value_offset_x() {
        // menuItem.cpp:17-19: DrawRoundedBox(x + voff - vw/2, y, 0, 7, vw).
        let pal = ramp_pal();
        let font = real_font();
        let mut bmp = Bitmap::new(200, 12);
        draw_item_value(&mut bmp, &pal, &font, "LIVES", Some("15"), 4, 2, true, false, false, 100, DONE);
        let vx = 4 + 100 - (font.get_dims("15") >> 1);
        assert_eq!(bmp.get_pixel(vx, 3), pal[0], "the value box band starts at vx");
        assert_eq!(bmp.get_pixel(vx - 1, 3), 0, "and not before");
    }

    #[test]
    fn an_unselected_value_gets_a_shadow_then_the_item_colour() {
        let pal = ramp_pal();
        let mut bmp = Bitmap::new(200, 12);
        draw_item_value(
            &mut bmp, &pal, &real_font(), "MAP", Some("ON"), 4, 2, false, true, false, 100,
            ItemColours { color: 48, dis_colour: 7 },
        );
        assert!(bmp.pixels.contains(&pal[7]) && !bmp.pixels.contains(&pal[48]), "disabled: dis_colour");
        assert!(bmp.pixels[100..].contains(&pal[0]), "the colour-0 shadows");
    }

    #[test]
    fn the_scrollbar_is_two_arrows_and_a_two_tone_tab() {
        // menu.cpp:107-124: height 15, 20 visible, top 5 -> bar 104, tab 78, tab_y = y + 26.
        let pal = ramp_pal();
        let font = real_font();
        let mut bmp = Bitmap::new(320, 200);
        draw_scrollbar(&mut bmp, &pal, &font, 178, 20, 15, 8, 5, 20);
        let (x, tab_y) = (178, 20 + 5 * 104 / 20);
        assert_eq!(bmp.get_pixel(x - 8, tab_y + 8), pal[7], "the light face at (x-8, tab_y+8)");
        assert_eq!(bmp.get_pixel(x - 2, tab_y + 8 + 77), pal[7], "78 rows tall, 7 wide");
        assert_eq!(bmp.get_pixel(x - 1, tab_y + 9 + 77), pal[0], "the colour-0 shadow at +1,+1");
        assert_eq!(bmp.get_pixel(x - 8, tab_y + 8 + 78), 0, "nothing below the tab");
        assert!(bmp.pixels.contains(&pal[50]), "the arrows are colour 50");
        let mut none = Bitmap::new(320, 200);
        draw_scrollbar(&mut none, &pal, &font, 178, 20, 15, 8, 0, 15);
        assert!(none.pixels.contains(&pal[7]), "the caller decides visibility; the tab still draws");
    }

    #[test]
    fn the_menu_palette_rotates_168_to_174_then_sets_the_worm_ramps() {
        use assets::palette::{Color, Palette};
        let mut p = Palette { entries: [Color::default(); 256] };
        for (i, e) in p.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        let rgb = [[104, 104, 252], [60, 172, 60]];
        let got = menu_palette(&p, 2, rgb);
        let mut want = p.clone();
        crate::palette::rotate_from(&mut want, &p, 168, 174, 2);
        crate::palette::set_worm_colour(&mut want, 0, rgb[0]);
        crate::palette::set_worm_colour(&mut want, 1, rgb[1]);
        assert_eq!(got, crate::palette::pack_pal32(&want));
        // With the ramps already in origpal (after Game::Focus) it equals the weapsel palette.
        let mut focused = p.clone();
        crate::palette::set_worm_colour(&mut focused, 0, rgb[0]);
        crate::palette::set_worm_colour(&mut focused, 1, rgb[1]);
        assert_eq!(menu_palette(&focused, 9, rgb), crate::weapsel::weapsel_palette(&focused, 9));
    }
```

(c) Create `rust/render/src/present.rs` with the module doc and tests only (the functions come in Step 3):

```rust
//! Presentation (Step 4½d): the composition fade C++ applies when it copies the back buffer to
//! the window. `Gfx::Flip` → `Gfx::Draw` → `ScaleDraw(..., renderer.fade_value)`
//! (`gfx.cpp:1033-1038`, `blit.cpp:798-826`): at `fade >= 32` the pixels are copied verbatim;
//! below it every pixel goes through `FadeArgb` (`blit.cpp:788-796`), `(v * fade) >> 5` per
//! channel with alpha forced to 0xFF. The same arithmetic as the frame hash's `fade_channel`
//! (`hash.rs`), so `hash_frame(bmp, f)` is the hash of what the window shows.

use crate::bitmap::Bitmap;
use crate::hash::fade_channel;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_frame;

    #[test]
    fn fade_argb_is_fadeargb() {
        assert_eq!(fade_argb(0x12C8_6420, 33), 0x12C8_6420, "fade >= 32: verbatim, alpha kept");
        assert_eq!(fade_argb(0x12C8_6420, 32), 0x12C8_6420);
        assert_eq!(fade_argb(0x00C8_6420, 16), 0xFF64_3210, "(v * 16) >> 5, alpha 0xFF");
        assert_eq!(fade_argb(0xFFFF_FFFF, 0), 0xFF00_0000, "fade 0: black");
    }

    #[test]
    fn the_hash_of_the_faded_frame_is_hash_frame_at_that_fade() {
        let mut bmp = Bitmap::new(7, 3);
        for (i, p) in bmp.pixels.iter_mut().enumerate() {
            *p = 0xFF00_0000 | (i as u32 * 0x0102_03);
        }
        for fade in [0, 1, 17, 31, 32, 33] {
            let mut faded = bmp.clone();
            for p in faded.pixels.iter_mut() {
                *p = fade_argb(*p, fade);
            }
            assert_eq!(hash_frame(&faded, 33), hash_frame(&bmp, fade), "fade {fade}");
        }
    }

    #[test]
    fn fade_into_rgba_writes_the_faded_pixels_as_rgba() {
        let mut bmp = Bitmap::new(2, 1);
        bmp.pixels = vec![0xFF10_2030, 0xFFFF_FFFF];
        let mut out = [0u8; 8];
        fade_into_rgba(&bmp, 16, &mut out);
        assert_eq!(out, [8, 16, 24, 255, 127, 127, 127, 255]);
    }
}
```

Add `pub mod present;` to `rust/render/src/lib.rs` after `pub mod palette;`, and extend the crate doc's last sentence with ` Step 4½d adds the value arm and the scrollbar (menu) and the composition fade (present).`

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p render --lib 2>&1 | tail -20`
Expected: FAIL to compile, with `cannot find function 'codepoint_to_font_byte'`, `'draw_item_value'`, `'draw_scrollbar'`, `'menu_palette'` and `'fade_argb'`.

- [ ] **Step 3: Implement**

(1) `rust/render/src/font.rs`. Replace `ascii_to_font_byte` and its doc comment (`:245-260`) with

```rust
/// `cp437.cpp:11-44` `kHighHalf`: the Unicode codepoint of each CP437 byte 0x80..=0xFF.
const HIGH_HALF: [u32; 128] = [
    0x00C7, 0x00FC, 0x00E9, 0x00E2, 0x00E4, 0x00E0, 0x00E5, 0x00E7,
    0x00EA, 0x00EB, 0x00E8, 0x00EF, 0x00EE, 0x00EC, 0x00C4, 0x00C5,
    0x00C9, 0x00E6, 0x00C6, 0x00F4, 0x00F6, 0x00F2, 0x00FB, 0x00F9,
    0x00FF, 0x00D6, 0x00DC, 0x00A2, 0x00A3, 0x00A5, 0x20A7, 0x0192,
    0x00E1, 0x00ED, 0x00F3, 0x00FA, 0x00F1, 0x00D1, 0x00AA, 0x00BA,
    0x00BF, 0x2310, 0x00AC, 0x00BD, 0x00BC, 0x00A1, 0x00AB, 0x00BB,
    0x2591, 0x2592, 0x2593, 0x2502, 0x2524, 0x2561, 0x2562, 0x2556,
    0x2555, 0x2563, 0x2551, 0x2557, 0x255D, 0x255C, 0x255B, 0x2510,
    0x2514, 0x2534, 0x252C, 0x251C, 0x2500, 0x253C, 0x255E, 0x255F,
    0x255A, 0x2554, 0x2569, 0x2566, 0x2560, 0x2550, 0x256C, 0x2567,
    0x2568, 0x2564, 0x2565, 0x2559, 0x2558, 0x2552, 0x2553, 0x256B,
    0x256A, 0x2518, 0x250C, 0x2588, 0x2584, 0x258C, 0x2590, 0x2580,
    0x03B1, 0x00DF, 0x0393, 0x03C0, 0x03A3, 0x03C3, 0x00B5, 0x03C4,
    0x03A6, 0x0398, 0x03A9, 0x03B4, 0x221E, 0x03C6, 0x03B5, 0x2229,
    0x2261, 0x00B1, 0x2265, 0x2264, 0x2320, 0x2321, 0x00F7, 0x2248,
    0x00B0, 0x2219, 0x00B7, 0x221A, 0x207F, 0x00B2, 0x25A0, 0x00A0,
];

/// `font.cpp:51-54` `CodepointToFontByte` over `cp437::UnicodeToByte` (`cp437.cpp:177-187`):
/// identity below 0x80, else the first `HIGH_HALF` index + 0x80, else `1` (the skip-no-draw
/// sentinel that fails the `>= 2` gate). Step 4½d (design finding 4): the copyright bar's `ä`.
fn codepoint_to_font_byte(cp: char) -> u8 {
    let u = cp as u32;
    if u < 0x80 {
        return u as u8;
    }
    match HIGH_HALF.iter().position(|&h| h == u) {
        Some(i) => 0x80 + i as u8,
        None => 1,
    }
}
```

In `draw_string` and `get_dims`, replace `ascii_to_font_byte(cp)` with `codepoint_to_font_byte(cp)`. In `draw_string`'s doc, replace "via [`ascii_to_font_byte`] (ASCII-only: `< 0x80` identity, else skip — the full CP437 high-half table is deferred, spec §7 Q2)" with "via [`codepoint_to_font_byte`] (the CP437 table, Step 4½d)". In `draw_char`'s CAUTION note, replace "(ASCII decode caps at 125 after the decrement)" with "(the CP437 decode yields at most 0xFF, which the `< 252` gate drops before the decrement)".

(2) `rust/render/src/menu.rs`. Replace the module doc's first sentence with `//! C++ \`MenuItem::Draw\` (\`menuItem.cpp:6-42\`), the item recipe every Liero menu draws with, the \`Menu::Draw\` scrollbar (\`menu.cpp:107-124\`) and the menu palette (\`gfx.cpp:986-990\`). Step 4½c pulled the text arm forward; Step 4½d adds the value arm, the scrollbar and \`menu_palette\`. The \`Menu\` framework itself lives in \`ui::menu\`.`. Replace the imports and `draw_item` with:

```rust
use assets::palette::Palette;

use crate::bitmap::{Bitmap, Pal32};
use crate::blit::draw_rounded_box;
use crate::font::Font;
use crate::palette::{pack_pal32, rotate_from, set_worm_colour};

/// The menu water rotation (`gfx.cpp:987`, `weapsel.cpp:22`).
pub const ROTATE_FROM: i32 = 168;
pub const ROTATE_TO: i32 = 174;

/// `MenuItem::Draw` with no value (`has_value == false`); see [`draw_item_value`].
#[allow(clippy::too_many_arguments)]
pub fn draw_item(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    selected: bool,
    disabled: bool,
    centered: bool,
    colours: ItemColours,
) {
    draw_item_value(bmp, pal, font, text, None, x, y, selected, disabled, centered, 0, colours);
}

/// `MenuItem::Draw` (`menuItem.cpp:6-42`). `value` is `Some` iff `has_value`. A selected item
/// gets a rounded box of its text width, plus one of the value's width centred on
/// `x + value_offset_x`; an unselected one gets colour-0 shadows at (+3, +2). Then the text at
/// (+2, +1) and the value at `x + value_offset_x - vw/2 + 2`, both in `dis_colour` if disabled,
/// else 168 if selected, else `color` — C++'s `c = disabled ? 7 : 168` sits inside the
/// `else if (selected)` arm, which only runs when not disabled, so it is always 168 (finding 15).
/// `centered` shifts `x` left by half the text width first (`:10-12`).
#[allow(clippy::too_many_arguments)]
pub fn draw_item_value(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    text: &str,
    value: Option<&str>,
    x: i32,
    y: i32,
    selected: bool,
    disabled: bool,
    centered: bool,
    value_offset_x: i32,
    colours: ItemColours,
) {
    let wid = font.get_dims(text);
    let vwid = value.map_or(0, |v| font.get_dims(v));
    let x = if centered { x - (wid >> 1) } else { x };
    let vx = x + value_offset_x - (vwid >> 1);
    if selected {
        draw_rounded_box(bmp, pal, x, y, 0, 7, wid);
        if value.is_some() {
            draw_rounded_box(bmp, pal, vx, y, 0, 7, vwid);
        }
    } else {
        font.draw_string(bmp, pal, text, x + 3, y + 2, 0, 1);
        if let Some(v) = value {
            font.draw_string(bmp, pal, v, vx + 3, y + 2, 0, 1);
        }
    }
    let c = if disabled {
        colours.dis_colour as i32
    } else if selected {
        SELECTED_COLOUR
    } else {
        colours.color as i32
    };
    font.draw_string(bmp, pal, text, x + 2, y + 1, c, 1);
    if let Some(v) = value {
        font.draw_string(bmp, pal, v, vx + 2, y + 1, c, 1);
    }
}

/// `Menu::Draw`'s scrollbar (`menu.cpp:107-124`), drawn by the caller iff
/// `visible_item_count > height`: glyphs 22 (up) and 23 (down) — passed straight to `DrawChar`,
/// i.e. CP437 bytes 24/25 — with a colour-0 shadow at (x-6, ..) under colour 50 at (x-7, ..),
/// then the tab as two clip-clamped `FillRect`s, colour 0 at (x-7, tab_y+9) under colour 7 at
/// (x-8, tab_y+8). All C++ `int` arithmetic.
#[allow(clippy::too_many_arguments)]
pub fn draw_scrollbar(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    x: i32,
    y: i32,
    height: i32,
    item_height: i32,
    top_item: i32,
    visible_item_count: i32,
) {
    let menu_height = height * item_height + 1;
    font.draw_char(bmp, pal, 22, x - 6, y + 2, 0, 1);
    font.draw_char(bmp, pal, 22, x - 7, y + 1, 50, 1);
    font.draw_char(bmp, pal, 23, x - 6, y + menu_height - 7, 0, 1);
    font.draw_char(bmp, pal, 23, x - 7, y + menu_height - 8, 50, 1);
    let bar = menu_height - 17;
    let tab = (height * bar / visible_item_count).min(bar).max(0);
    let tab_y = y + top_item * bar / visible_item_count;
    bmp.fill_rect(x - 7, tab_y + 9, 7, tab, 0, pal);
    bmp.fill_rect(x - 8, tab_y + 8, 7, tab, 7, pal);
}

/// `Gfx::UpdateMenuPalettes`'s palette (`gfx.cpp:987-993`): `Origpal`, `RotateFrom(Origpal, 168,
/// 174, menu_cycles)`, then `SetWormColours(settings)` for worms 0 and 1 (`palette.cpp:114-118`).
/// `menu_cycles` is `Gfx::menu_cycles` (`unsigned`). Not the weapsel palette: that one has no
/// worm step (plan-time fact 5).
pub fn menu_palette(origpal: &Palette, menu_cycles: u32, worm_rgb: [[i32; 3]; 2]) -> Pal32 {
    let mut pal = origpal.clone();
    rotate_from(&mut pal, origpal, ROTATE_FROM, ROTATE_TO, menu_cycles);
    for (i, rgb) in worm_rgb.iter().enumerate() {
        set_worm_colour(&mut pal, i, *rgb);
    }
    pack_pal32(&pal)
}
```

Keep `SELECTED_COLOUR` and `ItemColours` as they are. The existing test module's `use super::*;` covers the new fns. The value-arm tests use `Bitmap::new` from `crate::bitmap`, which `super::*` already brings in.

(3) `rust/render/src/present.rs`. Insert between the `use` lines and `#[cfg(test)]`:

```rust
/// `FadeArgb` (`blit.cpp:788-796`) as `ScaleDraw` applies it (`:801-815`): `fade >= 32` is the
/// verbatim copy, anything below fades each channel and forces alpha to 0xFF.
pub fn fade_argb(c: u32, amount: i32) -> u32 {
    if amount >= 32 {
        return c;
    }
    let r = fade_channel(((c >> 16) & 0xFF) as u8, amount) as u32;
    let g = fade_channel(((c >> 8) & 0xFF) as u8, amount) as u32;
    let b = fade_channel((c & 0xFF) as u8, amount) as u32;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// The window image of `bmp` at `amount`, as RGBA bytes (row-major, `bmp.w * bmp.h * 4`): what
/// the live game uploads (Step 4½d: the first live fade).
pub fn fade_into_rgba(bmp: &Bitmap, amount: i32, out: &mut [u8]) {
    debug_assert_eq!(out.len(), (bmp.w * bmp.h * 4) as usize);
    for y in 0..bmp.h {
        for x in 0..bmp.w {
            let px = fade_argb(bmp.pixels[(y * bmp.pitch + x) as usize], amount);
            let o = ((y * bmp.w + x) * 4) as usize;
            out[o] = ((px >> 16) & 0xFF) as u8;
            out[o + 1] = ((px >> 8) & 0xFF) as u8;
            out[o + 2] = (px & 0xFF) as u8;
            out[o + 3] = ((px >> 24) & 0xFF) as u8;
        }
    }
}
```

- [ ] **Step 4: GREEN, then the CP437 re-diff (design §6.8: on its own task)**

Run: `rustfmt --edition 2021 /home/user/openliero/rust/render/src/present.rs`
Run: `cd /home/user/openliero/rust && cargo test -p render --lib` — Expected: PASS (the new tests plus every old one).
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game 2>&1 | grep -E '^test result|FAILED|panicked' | sort | uniq -c` — Expected: only `ok` results. Every render golden (3a/3b/3e/4d/4½c-0/4½c weapsel frames) is unchanged: the CP437 change is hash-neutral.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/render/src/font.rs rust/render/src/menu.rs rust/render/src/present.rs rust/render/src/lib.rs
git -C /home/user/openliero commit -m "render(4.5d): CP437 high half (hash-neutral), MenuItem value arm, scrollbar, menu_palette, present::fade_argb" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 1: The `ui` crate — `text` and `keys` (`KeyLatch`)  [Sonnet]

**Files:**
- Modify: `rust/Cargo.toml` (`members`: add `"ui"` after `"scenario"`)
- Create: `rust/ui/Cargo.toml`, `rust/ui/src/lib.rs`, `rust/ui/src/text.rs`, `rust/ui/src/keys.rs`

**Interfaces:**
- Produces (used by T2–T11):
  - `ui::text::{GAME_MODES: [&str; 4], ONOFF: [&str; 2], time_to_string(i32) -> String, time_to_string_ex(i32, bool, bool) -> String, time_to_string_frames(i32) -> String, leaf_basename(&str) -> &str}`.
  - `ui::keys::{MAX_DOS_KEY, DK_* constants, K_UP..K_DIG, INPUT_KEYBOARD, TypedKey, KEY_BUF_LEN, KeyLatch, reset_left_right}`.
- Consumes: `scenario::settings::WormSettings` (`controls_ex`, `input_device`).

Why:
- Finding 3: `game_modes` and `onoff` are hardcoded in `Texts::Texts()` (`common.cpp:205-225`), not read from `tc.cfg`.
- Finding 5: menu keys are *level flags set by key-down events*, OS repeats included, cleared by `TestKeyOnce` (`gfx.hpp:141-179`).
- The control tests walk all three `WormSettings` over `controls_ex` (`gfx.cpp:861-976`).

This is the smallest Bevy-free base the `Menu` port (T2) and the shell (T7) stand on.

- [ ] **Step 1: Create the crate with failing tests**

`rust/Cargo.toml`: `members = ["sim-core", "assets", "sim", "render", "scenario", "ui", "game", "oracle-tests", "shot", "replay"]`.

`rust/ui/Cargo.toml`:

```toml
[package]
name = "ui"
version = "0.1.0"
edition = "2024"

# Step 4½d: the Bevy-free menu framework and the C++ Gfx/StateStack shell (design §4.1, Q1).
# Bevy stays in `game`; `oracle-tests` and `shot` drive this crate headlessly.
[dependencies]
sim-core = { path = "../sim-core" }
assets = { path = "../assets" }
sim = { path = "../sim" }
render = { path = "../render" }
scenario = { path = "../scenario" }
```

`rust/ui/src/lib.rs`:

```rust
//! Step 4½d — the Bevy-free menu framework and the C++ frame loop (design
//! `specs/2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md`, cited **design §N**).
//!
//! `ui::menu` ports C++ `Menu` / `MenuItem` / the item behaviors; `ui::keys` the `dos_keys`
//! table the menus read; `ui::text` the hardcoded C++ texts; `ui::shell` the `StateStack`, the
//! main menu, the router and `Gfx::RunOneFrame`. Bevy-free so `oracle-tests` (the C++ frame
//! gates) and `shot` drive the whole shell headlessly (design §2, amended LD 2).
pub mod keys;
pub mod text;
```

`rust/ui/src/text.rs`:

```rust
//! Texts the C++ hardcodes in `Texts::Texts()` (`common.cpp:205-225`, design finding 3 — not
//! read from `tc.cfg`), the `text.cpp` time formatters, and `GetBasename(GetLeaf(..))`.

/// `Texts::game_modes` (`common.cpp:206-209`), indexed by `Settings::game_mode`.
pub const GAME_MODES: [&str; 4] = ["Kill'em All", "Game of Tag", "Holdazone", "Scales of Justice"];
/// `Texts::onoff` (`common.cpp:211-212`).
pub const ONOFF: [&str; 2] = ["OFF", "ON"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_to_string_is_minutes_colon_seconds() {
        // text.cpp:5-16: '0' + sec/600, '0' + (sec%600)/60, ':', tens, units.
        assert_eq!(time_to_string(600), "10:00", "Settings(): time_to_lose");
        assert_eq!(time_to_string(3599), "59:59");
        assert_eq!(time_to_string(60), "01:00");
        assert_eq!(time_to_string(3600), "60:00");
        assert_eq!(time_to_string(45), "00:45");
    }

    #[test]
    fn time_to_string_frames_is_ms_at_14_per_frame() {
        // text.cpp:18-49: TimeToStringEx(frames * 14, false, false).
        assert_eq!(time_to_string_frames(70), "00.98");
        assert_eq!(time_to_string_frames(4286), "01:00.00", "60004 ms: minutes appear");
        // An exact hour: the hours digit, then (ms == 0, no force) the minutes block is skipped.
        assert_eq!(time_to_string_ex(6_000_000, false, false), "100.00");
        assert_eq!(time_to_string_ex(1234, true, true), "000:01.23");
    }

    #[test]
    fn leaf_basename_is_getbasename_of_getleaf() {
        // filesystem.cpp:38-54: the leaf after the last '/' or '\', up to its last '.'.
        assert_eq!(leaf_basename("Levels/water_stage.lev"), "water_stage");
        assert_eq!(leaf_basename("C:\\a\\b.c.lev"), "b.c");
        assert_eq!(leaf_basename("noext"), "noext");
        assert_eq!(leaf_basename(""), "");
        assert_eq!(leaf_basename("data/Setups/liero.cfg"), "liero");
    }
}
```

`rust/ui/src/keys.rs`:

```rust
//! The C++ menu keyboard (design §4.6): `Gfx::dos_keys` plus `key_buf` (`gfx.hpp:141-179`,
//! `gfx.cpp:595-640`, `:861-976`). A key-down event — OS auto-repeats included — sets the
//! DOS key's flag; a key-up clears it; `test_once` reads and clears (`TestKeyOnce`), so a held
//! key acts once per key-down event (design finding 5). Menus test the SETTINGS' DOS bindings
//! (`controls_ex`) of all three `WormSettings`, as C++ does. Gamepads and `ex_keys` are not
//! modelled (overview §Deferrals).

use scenario::settings::WormSettings;

/// `kMaxDosKey` (`keys.hpp:17`): DOS scancodes are `1..177`; 0 means "unbound".
pub const MAX_DOS_KEY: u32 = 177;
/// DOS scancodes the menus test (`keys.cpp:9-60`: the `liero_to_sdl_keys` index of each key).
pub const DK_ESCAPE: u32 = 1;
pub const DK_RETURN: u32 = 28;
pub const DK_LCTRL: u32 = 29;
pub const DK_F1: u32 = 59;
pub const DK_F2: u32 = 60;
pub const DK_F3: u32 = 61;
pub const DK_F5: u32 = 63;
pub const DK_F6: u32 = 64;
pub const DK_F7: u32 = 65;
pub const DK_F8: u32 = 66;
pub const DK_F9: u32 = 67;
/// `SDLToDOSKey`'s "unknown key" value (`keys.cpp:70-75`).
pub const DK_UNKNOWN: u32 = 89;
pub const DK_KP_ENTER: u32 = 116;
pub const DK_RCTRL: u32 = 117;
pub const DK_UP: u32 = 160;
pub const DK_PGUP: u32 = 161;
pub const DK_LEFT: u32 = 163;
pub const DK_RIGHT: u32 = 165;
pub const DK_DOWN: u32 = 168;
pub const DK_PGDN: u32 = 169;

/// `WormSettingsExtensions::Control` (`worm.hpp:45-55`): indices into `controls_ex`.
pub const K_UP: usize = 0;
pub const K_DOWN: usize = 1;
pub const K_LEFT: usize = 2;
pub const K_RIGHT: usize = 3;
pub const K_FIRE: usize = 4;
pub const K_CHANGE: usize = 5;
pub const K_JUMP: usize = 6;
pub const K_DIG: usize = 7;
/// `WormSettingsExtensions::kInputKeyboard` (`worm.hpp:59`).
pub const INPUT_KEYBOARD: u32 = 0;

/// One `key_buf` entry (`gfx.cpp:601-603`) as `Menu::OnKeys` reads it (`menu.cpp:16-19`): the
/// key's unshifted symbol `SDL_GetKeyFromScancode(sc, NONE)` — only `32..=127` is acted on — or
/// Tab, which is tested by scancode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedKey {
    Sym(u32),
    Tab,
}

/// `Gfx::key_buf` holds 32 scancodes (`gfx.hpp`, `gfx.cpp:601`).
pub const KEY_BUF_LEN: usize = 32;

#[cfg(test)]
mod tests {
    use super::*;
    use scenario::settings::Settings;

    #[test]
    fn a_key_down_sets_the_flag_until_tested_once_or_released() {
        let mut k = KeyLatch::default();
        k.key_down(DK_UP, TypedKey::Sym(0));
        assert!(k.test(DK_UP) && k.test(DK_UP), "test does not clear");
        assert!(k.test_once(DK_UP));
        assert!(!k.test_once(DK_UP), "cleared: a held key acts once per key-down event");
        k.key_down(DK_UP, TypedKey::Sym(0)); // an OS repeat
        assert!(k.test_once(DK_UP), "finding 5: repeats count");
        k.key_down(DK_DOWN, TypedKey::Sym(0));
        k.key_up(DK_DOWN);
        assert!(!k.test_once(DK_DOWN), "a key-up clears the flag");
        k.key_down(DK_LEFT, TypedKey::Sym(0));
        k.release(DK_LEFT);
        assert!(!k.test(DK_LEFT));
        k.key_down(DK_F1, TypedKey::Sym(0));
        k.clear();
        assert!(!k.test(DK_F1), "ClearKeys");
    }

    #[test]
    fn key_zero_is_never_down() {
        let mut k = KeyLatch::default();
        k.key_down(0, TypedKey::Sym(0));
        assert!(!k.test(0));
    }

    #[test]
    fn the_key_buf_keeps_32_key_downs_per_frame() {
        let mut k = KeyLatch::default();
        for i in 0..40 {
            k.key_down(DK_UNKNOWN, TypedKey::Sym(b'a' as u32 + i));
        }
        assert_eq!(k.typed().len(), KEY_BUF_LEN);
        assert_eq!(k.typed()[0], TypedKey::Sym(b'a' as u32));
        k.key_up(DK_UNKNOWN);
        assert_eq!(k.typed().len(), KEY_BUF_LEN, "key-ups are not buffered");
        k.begin_frame();
        assert!(k.typed().is_empty(), "key_buf_ptr = key_buf at each frame (gfx.cpp:1475)");
    }

    #[test]
    fn controls_are_tested_over_all_three_keyboard_players() {
        // settings.cpp:23-60: P1 R/F/D/G/LCtrl/LShift/LAlt, P2 arrows/RCtrl/RAlt/RShift, the
        // network player = P1. gfx.cpp:861-876 / :947-960.
        let s = Settings::default();
        let ws = &s.worm_settings;
        let mut k = KeyLatch::default();
        k.key_down(19, TypedKey::Sym(b'r' as u32)); // R: P1 up (and the network player's)
        assert!(k.test_control(ws, K_UP));
        assert!(k.test_control_once(ws, K_UP));
        assert!(!k.test_control_once(ws, K_UP), "the first matching player consumed it");
        k.key_down(DK_UP, TypedKey::Sym(0)); // the arrow is P2's up
        assert!(k.test_control_once(ws, K_UP));
        k.key_down(56, TypedKey::Sym(0)); // LAlt: P1 jump
        assert!(k.test_control_once(ws, K_JUMP));
        assert!(!k.test_control(ws, K_DIG), "DIG is unbound (0) and 0 never matches");
    }

    #[test]
    fn a_gamepad_player_is_skipped_by_test_but_not_by_release() {
        let mut s = Settings::default();
        s.worm_settings[0].input_device = 1;
        s.worm_settings[2].input_device = 1;
        let ws = &s.worm_settings;
        let mut k = KeyLatch::default();
        k.key_down(19, TypedKey::Sym(0));
        assert!(!k.test_control(ws, K_UP), "only keyboard players are tested (gfx.cpp:866)");
        k.release_control(ws, K_UP);
        s.worm_settings[0].input_device = 0;
        assert!(!k.test_control(&s.worm_settings, K_UP), "ReleaseControl releases every player's key");
    }

    #[test]
    fn reset_left_right_releases_the_arrows_and_every_players_left_right() {
        let s = Settings::default();
        let mut k = KeyLatch::default();
        for dos in [DK_LEFT, DK_RIGHT, 32, 34] {
            k.key_down(dos, TypedKey::Sym(0));
        }
        reset_left_right(&mut k, &s.worm_settings);
        for dos in [DK_LEFT, DK_RIGHT, 32, 34] {
            assert!(!k.test(dos), "mainMenuState.cpp:56-61 released {dos}");
        }
    }
}
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p ui 2>&1 | tail -8`
Expected: FAIL to compile, with `cannot find function 'time_to_string'`, `'leaf_basename'`, `'reset_left_right'`, and `cannot find type 'KeyLatch'`.

- [ ] **Step 3: Implement**

Append to `rust/ui/src/text.rs` (above `#[cfg(test)]`):

```rust
/// C++ `'0' + n` stored into a `char` (`text.cpp`): the digit for 0..=9, and the same byte
/// arithmetic (wrapping) outside it.
fn digit(n: i32) -> char {
    (b'0' as i32 + n) as u8 as char
}

/// `TimeToString(sec)` (`text.cpp:5-16`): `M M : S S` with the first digit `sec / 600`.
pub fn time_to_string(sec: i32) -> String {
    [
        digit(sec / 600),
        digit((sec % 600) / 60),
        ':',
        digit((sec % 60) / 10),
        digit(sec % 10),
    ]
    .iter()
    .collect()
}

/// `TimeToStringEx(ms, force_hours, force_minutes)` (`text.cpp:18-45`).
pub fn time_to_string_ex(ms: i32, force_hours: bool, force_minutes: bool) -> String {
    let mut ms = ms;
    let mut s = String::new();
    if ms >= 6_000_000 || force_hours {
        s.push(digit(ms / 6_000_000));
        ms %= 6_000_000;
    }
    if ms >= 60_000 || force_minutes {
        s.push(digit(ms / 600_000));
        ms %= 600_000;
        s.push(digit(ms / 60_000));
        ms %= 60_000;
        s.push(':');
    }
    s.push(digit(ms / 10_000));
    ms %= 10_000;
    s.push(digit(ms / 1000));
    ms %= 1000;
    s.push('.');
    s.push(digit(ms / 100));
    ms %= 100;
    s.push(digit(ms / 10));
    s
}

/// `TimeToStringFrames(frames)` (`text.cpp:47-49`): 14 ms per frame.
pub fn time_to_string_frames(frames: i32) -> String {
    time_to_string_ex(frames * 14, false, false)
}

/// `GetBasename(GetLeaf(path))` (`filesystem.cpp:38-54`).
pub fn leaf_basename(path: &str) -> &str {
    let leaf = path.rsplit(['/', '\\']).next().unwrap_or(path);
    leaf.rsplit_once('.').map_or(leaf, |(b, _)| b)
}
```

The two `time_to_string_ex` literals were computed from `text.cpp:18-45` by hand at plan time. For `(6_000_000, false, false)`: `'1'`, then `ms = 0` skips the minutes block, then `"00.00"`. For `(1234, true, true)`: `'0'`, `"00:"`, then `"01.23"`. If one fails, re-derive it from the C++ before changing either side.

Append to `rust/ui/src/keys.rs` (above `#[cfg(test)]`):

```rust
/// `Gfx::dos_keys` + `key_buf` (see the module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyLatch {
    down: [bool; MAX_DOS_KEY as usize],
    buf: Vec<TypedKey>,
}

impl Default for KeyLatch {
    fn default() -> Self {
        KeyLatch {
            down: [false; MAX_DOS_KEY as usize],
            buf: Vec::with_capacity(KEY_BUF_LEN),
        }
    }
}

impl KeyLatch {
    /// `key_buf_ptr = key_buf` at the top of every frame (`gfx.cpp:1475`, `:721`).
    pub fn begin_frame(&mut self) {
        self.buf.clear();
    }

    /// `SDL_EVENT_KEY_DOWN` (`gfx.cpp:597-608`): buffer the key (up to 32), set its DOS flag
    /// (`if (kDosScan)`: 0 is never set). OS repeats included (finding 5).
    pub fn key_down(&mut self, dos: u32, typed: TypedKey) {
        if self.buf.len() < KEY_BUF_LEN {
            self.buf.push(typed);
        }
        if dos != 0 {
            self.down[dos as usize] = true;
        }
    }

    /// `SDL_EVENT_KEY_UP` (`gfx.cpp:634-639`).
    pub fn key_up(&mut self, dos: u32) {
        if dos != 0 {
            self.down[dos as usize] = false;
        }
    }

    /// This frame's key-downs, for `Menu::on_keys`.
    pub fn typed(&self) -> &[TypedKey] {
        &self.buf
    }

    /// `TestKeyOnce` (`gfx.hpp:141-145`).
    pub fn test_once(&mut self, dos: u32) -> bool {
        std::mem::take(&mut self.down[dos as usize])
    }

    /// `TestKey` (`gfx.hpp:147`).
    pub fn test(&self, dos: u32) -> bool {
        self.down[dos as usize]
    }

    /// `ReleaseKey` (`gfx.hpp:149`).
    pub fn release(&mut self, dos: u32) {
        self.down[dos as usize] = false;
    }

    /// `ClearKeys` (`gfx.cpp:856-862`): the DOS flags (joysticks and `ex_keys` are unmodelled).
    pub fn clear(&mut self) {
        self.down = [false; MAX_DOS_KEY as usize];
    }

    /// `TestAnyKeyOnce` (`gfx.hpp:171-184`) for a DOS key: 0 never matches; extended keys
    /// (gamepad, `>= kMaxDosKey`) are unmodelled and never match.
    fn test_any_once(&mut self, key: u32) -> bool {
        key != 0 && key < MAX_DOS_KEY && self.test_once(key)
    }

    fn test_any(&self, key: u32) -> bool {
        key != 0 && key < MAX_DOS_KEY && self.test(key)
    }

    /// `Gfx::TestControlOnce` (`gfx.cpp:861-876`): the first keyboard player whose
    /// `controls_ex[control]` flag is set, consumed.
    pub fn test_control_once(&mut self, ws: &[WormSettings], control: usize) -> bool {
        for w in ws {
            if w.input_device == INPUT_KEYBOARD && self.test_any_once(w.controls_ex[control]) {
                return true;
            }
        }
        false
    }

    /// `Gfx::TestControl` (`gfx.cpp:947-960`).
    pub fn test_control(&self, ws: &[WormSettings], control: usize) -> bool {
        ws.iter()
            .any(|w| w.input_device == INPUT_KEYBOARD && self.test_any(w.controls_ex[control]))
    }

    /// `Gfx::ReleaseControl` (`gfx.cpp:962-968`): every player, whatever its input device.
    pub fn release_control(&mut self, ws: &[WormSettings], control: usize) {
        for w in ws {
            let key = w.controls_ex[control];
            if key != 0 && key < MAX_DOS_KEY {
                self.down[key as usize] = false;
            }
        }
    }
}

/// `ResetLeftRight` (`mainMenuState.cpp:56-61`): release the arrows and every player's
/// Left/Right, so a behavior that returned false from `OnLeftRight` fires again only on the
/// next key-down event.
pub fn reset_left_right(keys: &mut KeyLatch, ws: &[WormSettings]) {
    keys.release(DK_LEFT);
    keys.release(DK_RIGHT);
    keys.release_control(ws, K_LEFT);
    keys.release_control(ws, K_RIGHT);
}
```

`test_once`, `test` and `release` index the table directly. A DOS key `>= 177` is a caller bug and panics, like the C++ out-of-bounds read it would be.

- [ ] **Step 4: GREEN**

Run: `rustfmt --edition 2024 /home/user/openliero/rust/ui/src/lib.rs /home/user/openliero/rust/ui/src/text.rs /home/user/openliero/rust/ui/src/keys.rs`
Run: `cd /home/user/openliero/rust && cargo test -p ui` — Expected: PASS, 9 tests.
Run: `cd /home/user/openliero/rust && cargo tree -p ui -e normal | grep -c bevy` — Expected: `0`.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game 2>&1 | grep -cE 'test result: FAILED'` — Expected: `0`.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/Cargo.toml rust/Cargo.lock rust/ui
git -C /home/user/openliero commit -m "ui(4.5d): new Bevy-free crate — text (game_modes, onoff, TimeToString) + keys (KeyLatch over dos_keys/key_buf)" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 2: `ui::menu` — `MenuItem`, `Menu` (navigation, visibility, scrolling, draw), `MenuModel`  [Opus]

**Files:**
- Create: `rust/ui/src/menu/mod.rs`, `rust/ui/src/menu/behavior.rs` (the `Plain` and `Custom` arms only; T3 adds the rest), `rust/ui/src/menu/search.rs` (the `Search` state only; T3 adds `on_keys`)
- Modify: `rust/ui/src/lib.rs` (`pub mod menu;`)

**Interfaces:**
- Produces (used by T3, T5, T7):
  - `MenuItem { color: u8, dis_colour: u8, string: String, has_value: bool, value: String, visible: bool, selectable: bool, id: i32 }` with `new(color, dis, &str, id)` and `space()`.
  - `MenuHooks { move_up, move_down, select }`.
  - `MenuCx<'a> { menu_cycles: u32, hooks: MenuHooks, sounds: &'a mut Vec<i32> }` with `play(hook)`.
  - `trait MenuModel { behavior, on_update, draw_item_overlay }` and `PlainModel`.
  - `Menu` with the public C++ fields and one method per C++ method: `new, selection, set_selection, is_selection_valid, selected, selected_id, index_from_id, item_from_id(_mut), add_item, add_item_at, clear, set_height, move_to, move_to_id, move_to_first_visible, is_in_view, item_position, ensure_in_view, first_visible_from, last_visible_from, visible_item_index, item_from_visible_index, set_bottom, set_top, set_visibility, scroll, movement_page, movement, on_left_right, on_enter, update_items, draw`.
  - `behavior::{Behavior::{Plain, Custom}, CustomBehavior, Enter, ValueEntry, LeftRight}` and `search::Search`.
- Consumes: T0's `render::menu::{draw_item_value, draw_scrollbar, ItemColours}`, `render::font::Font`.

Why: design §3.4 and §4.2–§4.4. Every later menu builds on this: the main menu and the disabled settings menu in 4½d, the settings, player and hidden menus in 4½e/4½f. It is gated line for line against the real C++ `Menu` in T5. It keeps the C++ `int` arithmetic and **the C++ call graph**: `movement` calls `move_to`, which calls `first_visible_from` and `ensure_in_view`, even where a shortcut would give the same result (design §4.4).

- [ ] **Step 1: Write the failing tests**

Create `rust/ui/src/menu/search.rs`:

```rust
//! Type-to-search state (`Menu::search_prefix` / `search_time`, `menu.hpp:175-176`). The search
//! itself (`Menu::OnKeys`, `menu.cpp:14-79`) is T3's `Menu::on_keys`.

/// `search_prefix` + `search_time` (milliseconds from the caller's clock; design §4.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    pub prefix: String,
    pub time_ms: u64,
}
```

Create `rust/ui/src/menu/behavior.rs`:

```rust
//! C++ `ItemBehavior` and its subclasses (`itemBehavior.hpp:8-18`, design §3.5 / §4.3). C++
//! builds a fresh behavior per call through the virtual `GetItemBehavior`, holding `int& v` into
//! the settings; Rust's `Behavior<'a>` borrows the one field it edits from the `MenuModel`, is
//! built per call, and is dropped before the `Menu` is touched again.

use super::{Menu, MenuCx};

/// `OnEnter`'s `int` (`-1`: nothing chosen), or the `InputStringState` an `IntegerBehavior`
/// pushes (`integerBehavior.cpp:36-80`) as a request 4½e's overlay consumes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enter {
    Result(i32),
    EditValue(ValueEntry),
}

/// The value-entry request (`integerBehavior.cpp:41-78`): displayed units, `digits` characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueEntry {
    pub item_id: i32,
    pub initial: String,
    pub digits: i32,
    pub x: i32,
    pub y: i32,
    pub min: i32,
    pub max: i32,
    pub div: i32,
    pub percentage: bool,
}

/// `OnLeftRight`'s bool (`keep`: false makes the caller release Left/Right, design §3.5) plus
/// whether the behavior changed a value that needs `menu.UpdateItems` (`EnumBehavior::Change`,
/// `enumBehavior.cpp:31-39`) — run by `Menu::on_left_right` after the behavior is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeftRight {
    pub keep: bool,
    pub update_all: bool,
}

/// A `gfx.cpp`-local behavior (`LevelSelectBehavior`, `OptionsSaveBehavior`, 4½f's
/// `WeaponEnumBehavior`, …). The defaults are the base `ItemBehavior` (`itemBehavior.hpp:13-17`).
pub trait CustomBehavior {
    fn on_left_right(&mut self, _menu: &mut Menu, _idx: usize, _dir: i32, _cx: &mut MenuCx) -> LeftRight {
        LeftRight { keep: true, update_all: false }
    }
    /// The `Enter` plus the "update all items" request.
    fn on_enter(&mut self, _menu: &mut Menu, _idx: usize, _cx: &mut MenuCx) -> (Enter, bool) {
        (Enter::Result(-1), false)
    }
    fn on_update(&self, _menu: &mut Menu, _idx: usize) {}
}

/// One item's behavior for one call (see the module doc). T3 adds `Integer`, `Time`, `Bool`,
/// `Enum` and `ArrayEnum`.
pub enum Behavior<'a> {
    /// The base `ItemBehavior`: `OnLeftRight` true, `OnEnter` -1, `OnUpdate` nothing.
    Plain,
    Custom(Box<dyn CustomBehavior + 'a>),
}

impl Behavior<'_> {
    pub fn on_left_right(&mut self, menu: &mut Menu, idx: usize, dir: i32, cx: &mut MenuCx) -> LeftRight {
        match self {
            Behavior::Plain => LeftRight { keep: true, update_all: false },
            Behavior::Custom(c) => c.on_left_right(menu, idx, dir, cx),
        }
    }

    pub fn on_enter(&mut self, menu: &mut Menu, idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        match self {
            Behavior::Plain => (Enter::Result(-1), false),
            Behavior::Custom(c) => c.on_enter(menu, idx, cx),
        }
    }

    pub fn on_update(&self, menu: &mut Menu, idx: usize) {
        match self {
            Behavior::Plain => {}
            Behavior::Custom(c) => c.on_update(menu, idx),
        }
    }
}
```

Create `rust/ui/src/menu/mod.rs` with the module doc, the `pub mod`/`pub use` lines and the test module below. The types and `impl Menu` come in Step 3.

```rust
//! C++ `Menu` / `MenuItem` (`menu.hpp:26-197`, `menu.cpp`, `menuItem.hpp:10-36`; design §3.4,
//! §4.2, §4.4): one method per C++ method, same names in snake_case, the C++ `int` arithmetic
//! and call graph. `MenuModel` stands for C++ `Menu`'s virtuals (`GetItemBehavior`, `OnUpdate`,
//! `DrawItemOverlay`, `menu.hpp:55-67`). Gated line for line against the real C++ `Menu` by
//! `oracle-tests/tests/menu_widget_golden.rs` (G1, design §6.1).

pub mod behavior;
pub mod search;

pub use behavior::{Behavior, CustomBehavior, Enter, LeftRight, ValueEntry};
pub use search::Search;

#[cfg(test)]
mod tests {
    use super::*;
    use render::bitmap::{Bitmap, Pal32};
    use render::font::Font;
    use render::menu::{ItemColours, draw_item_value};

    fn font() -> Font {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero/sprites/font.tga");
        Font::load(&assets::sprite::Tga::load(&std::fs::read(path).unwrap()).unwrap())
    }

    fn ramp() -> Pal32 {
        std::array::from_fn(|i| 0xFF00_0000 | i as u32)
    }

    /// `n` plain visible, selectable items `ITEM k`, ids `k`, at (40, 10), height `h`.
    fn items(n: i32, h: i32) -> Menu {
        let mut m = Menu::new(40, 10, false);
        m.height = h;
        for k in 0..n {
            m.add_item(MenuItem::new(48, 7, &format!("ITEM {k:02}"), k));
        }
        m
    }

    /// [A, B, space, hidden, grey(unselectable), C] — ids 0, 1, -1, 3, 4, 5.
    fn mixed() -> Menu {
        let mut m = Menu::new(40, 10, false);
        m.add_item(MenuItem::new(48, 7, "A", 0));
        m.add_item(MenuItem::new(48, 7, "B", 1));
        m.add_item(MenuItem::space());
        let mut hidden = MenuItem::new(48, 7, "HIDDEN", 3);
        hidden.visible = false;
        m.add_item(hidden);
        let mut grey = MenuItem::new(48, 7, "GREY", 4);
        grey.selectable = false;
        m.add_item(grey);
        m.add_item(MenuItem::new(48, 7, "C", 5));
        m
    }

    #[test]
    fn add_item_counts_visible_items_but_does_not_set_the_bottom() {
        // menu.cpp:295-302: AddItem never calls SetTop, so bottom_item stays 0 until the first
        // MoveTo -> EnsureInView -> SetBottom (design §3.4).
        let mut m = items(20, 15);
        assert_eq!((m.visible_item_count, m.top_item, m.bottom_item), (20, 0, 0));
        assert!(!m.is_in_view(0), "bottom 0: nothing is in view yet");
        m.move_to_first_visible();
        assert_eq!((m.selection(), m.top_item, m.bottom_item), (0, 0, 15));
        assert_eq!(mixed().visible_item_count, 5, "the hidden item is not counted");
    }

    #[test]
    fn movement_wraps_over_invisible_and_unselectable_items() {
        let mut m = mixed();
        m.move_to_first_visible();
        let mut seen = vec![m.selection()];
        for _ in 0..3 {
            m.movement(1);
            seen.push(m.selection());
        }
        for _ in 0..3 {
            m.movement(-1);
            seen.push(m.selection());
        }
        assert_eq!(seen, [0, 1, 5, 0, 5, 1, 0], "menu.cpp:263-293");
        m.movement(0);
        assert_eq!(m.selection(), 0, "direction 0 does nothing");
    }

    #[test]
    fn move_to_clamps_then_skips_forward_to_a_selectable_item() {
        let mut m = mixed();
        m.move_to(99);
        assert_eq!(m.selection(), 5, "clamped to the last item");
        m.move_to(-5);
        assert_eq!(m.selection(), 0);
        m.move_to(2);
        assert_eq!(m.selection(), 5, "the spacer, hidden and grey items are skipped forward");
        m.move_to_id(1);
        assert_eq!(m.selection(), 1);
        m.move_to_id(77);
        assert_eq!(m.selection(), 0, "IndexFromId -1 clamps to 0");
    }

    #[test]
    fn with_nothing_selectable_the_selection_is_the_item_count() {
        let mut m = Menu::new(0, 0, false);
        m.add_item(MenuItem::space());
        m.add_item(MenuItem::space());
        m.move_to_first_visible();
        assert_eq!(m.selection(), 2, "FirstVisibleFrom returns items.size() (menu.cpp:169-177)");
        assert!(m.selected().is_none() && !m.is_selection_valid());
        assert_eq!(m.selected_id(), -1);
        assert_eq!(m.first_visible_from(-1), 2, "`std::size_t i = item` wraps a negative start");
    }

    #[test]
    fn last_visible_from_returns_one_past_the_item_it_finds() {
        let m = mixed();
        assert_eq!(m.last_visible_from(6), 6, "C at 5 -> 6 (menu.cpp:179-187)");
        assert_eq!(m.last_visible_from(5), 2, "B at 1 -> 2");
        assert_eq!(m.last_visible_from(0), 0);
    }

    #[test]
    fn set_top_clamps_and_sets_the_bottom() {
        let mut m = items(20, 15);
        m.set_top(10);
        assert_eq!((m.top_item, m.bottom_item), (5, 20), "clamped to vis - height");
        m.set_top(-3);
        assert_eq!((m.top_item, m.bottom_item), (0, 15));
        m.scroll(2);
        assert_eq!((m.top_item, m.bottom_item), (2, 17));
        m.set_height(10);
        assert_eq!((m.top_item, m.bottom_item), (2, 12), "SetHeight re-clamps via SetTop");
        let mut short = items(4, 15);
        short.set_top(3);
        assert_eq!((short.top_item, short.bottom_item), (0, 4));
    }

    #[test]
    fn a_page_down_moves_half_a_screen_and_lands_forward_of_a_spacer() {
        // 20 items with a spacer at 7; menu.cpp:250-261: offset = 1 * (15 / 2) = 7.
        let mut m = Menu::new(40, 10, false);
        for k in 0..20 {
            m.add_item(if k == 7 { MenuItem::space() } else { MenuItem::new(48, 7, "X", k) });
        }
        m.move_to_first_visible();
        m.movement_page(1);
        assert_eq!(m.selection(), 8, "visible index 7 is the spacer: MoveTo skips to 8");
        assert_eq!((m.top_item, m.bottom_item), (5, 20), "SetTop(0 + 7) clamped to 20 - 15");
        m.movement_page(-1);
        assert_eq!(m.selection(), 1, "visible 8 - 7 = 1");
        assert_eq!(m.top_item, 0, "SetTop(5 - 7) clamps to 0 before the MoveTo");
    }

    #[test]
    fn hiding_the_selected_item_keeps_the_index_and_reanchors_the_top() {
        let mut m = items(10, 5);
        m.move_to(7);
        assert_eq!((m.top_item, m.bottom_item), (3, 8));
        m.set_visibility(7, false);
        assert_eq!(m.selection(), 7, "EnsureInView ignores an invisible item (menu.cpp:155-158)");
        assert_eq!((m.visible_item_count, m.top_item, m.bottom_item), (9, 3, 8));
        m.set_visibility(0, false);
        assert_eq!((m.top_item, m.bottom_item), (2, 7), "the same real item (3) stays on top");
        m.set_visibility(0, true);
        m.set_visibility(7, true);
        assert_eq!((m.visible_item_count, m.top_item), (10, 3));
    }

    #[test]
    fn item_position_is_only_for_items_in_view() {
        let mut m = items(20, 15);
        m.move_to_first_visible();
        assert_eq!(m.item_position(3), Some((40, 10 + 3 * 8)));
        assert_eq!(m.item_position(16), None);
        m.move_to(19);
        assert_eq!(m.item_position(19), Some((40, 10 + 14 * 8)));
    }

    #[test]
    fn add_item_at_returns_the_size_before_the_insert() {
        let mut m = items(3, 15);
        assert_eq!(m.add_item_at(MenuItem::new(1, 1, "FIRST", 9), 0), 3, "menu.cpp:311-317");
        assert_eq!(m.items[0].id, 9);
        assert_eq!(m.visible_item_count, 4);
    }

    #[test]
    fn draw_is_the_item_recipe_row_by_row_from_the_top_item() {
        let (font, pal) = (font(), ramp());
        let mut m = mixed();
        m.value_offset_x = 60;
        m.items[1].has_value = true;
        m.items[1].value = "ON".into();
        m.move_to(1);
        let mut got = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut got, &pal, &font, false, -1, false);
        let mut want = Bitmap::new(320, 200);
        let mut y = 10;
        for (i, it) in m.items.iter().enumerate() {
            if !it.visible {
                continue;
            }
            let value = it.has_value.then_some(it.value.as_str());
            let colours = ItemColours { color: it.color, dis_colour: it.dis_colour };
            draw_item_value(&mut want, &pal, &font, &it.string, value, 40, y, i == 1, false, false, 60, colours);
            y += 8;
        }
        assert_eq!(got, want);
        let mut at = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut at, &pal, &font, false, -7, false);
        assert_eq!(at, got, "any negative x means the menu's own x (menu.cpp:86-88)");
    }

    #[test]
    fn a_disabled_draw_selects_only_with_show_disabled_selection() {
        let (font, pal) = (font(), ramp());
        let mut m = items(3, 15);
        m.move_to_first_visible();
        let mut off = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut off, &pal, &font, true, -1, false);
        assert!(!off.pixels.contains(&pal[168]) && off.pixels.contains(&pal[7]));
        let mut on = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut on, &pal, &font, true, -1, true);
        assert!(on.pixels.contains(&pal[7]) && !on.pixels.contains(&pal[168]), "disabled: dis_colour");
        assert_ne!(on, off, "the selected row gets its box");
    }

    #[test]
    fn the_scrollbar_draws_iff_more_visible_items_than_rows() {
        let (font, pal) = (font(), ramp());
        // Colour 50 is only ever the arrows here (items are 48, the selected one 168).
        let column = |b: &Bitmap| b.pixels.contains(&pal[50]);
        for (n, want) in [(15, false), (16, true)] {
            let mut m = items(n, 15);
            m.move_to_first_visible();
            let mut b = Bitmap::new(320, 200);
            m.draw(&PlainModel, &mut b, &pal, &font, false, -1, false);
            assert_eq!(column(&b), want, "{n} items: menu.cpp:107");
        }
    }

    struct Counting(u32);
    impl MenuModel for Counting {
        fn behavior(&mut self, _id: i32) -> Behavior<'_> {
            Behavior::Plain
        }
        fn on_update(&mut self, menu: &mut Menu) {
            self.0 += 1;
            menu.items[0].string = "UPDATED".into();
        }
    }

    #[test]
    fn the_plain_behavior_and_the_empty_menu() {
        let mut sounds = Vec::new();
        let mut cx = MenuCx { menu_cycles: 0, hooks: MenuHooks::default(), sounds: &mut sounds };
        let mut m = items(2, 15);
        m.move_to_first_visible();
        assert!(m.on_left_right(&mut PlainModel, 1, &mut cx), "ItemBehavior: true");
        assert_eq!(m.on_enter(&mut PlainModel, &mut cx), Enter::Result(-1));
        let mut empty = Menu::new(0, 0, false);
        assert!(!empty.on_left_right(&mut PlainModel, 1, &mut cx));
        assert_eq!(empty.on_enter(&mut PlainModel, &mut cx), Enter::Result(0), "`return false;` (menu.hpp:81-84)");
        let mut model = Counting(0);
        m.update_items(&mut model);
        assert_eq!((model.0, m.items[0].string.as_str()), (1, "UPDATED"), "Menu::OnUpdate after the items");
        assert!(sounds.is_empty());
    }
}
```

In `rust/ui/src/lib.rs`, add `pub mod menu;` between `pub mod keys;` and `pub mod text;`.

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p ui menu 2>&1 | tail -8`
Expected: FAIL to compile, with `cannot find type 'Menu'`, `'MenuItem'`, `'MenuCx'`, `'MenuHooks'`, `'PlainModel'` and trait `'MenuModel'`.

- [ ] **Step 3: Implement** (in `rust/ui/src/menu/mod.rs`, above `#[cfg(test)]`)

```rust
use render::bitmap::{Bitmap, Pal32};
use render::font::Font;
use render::menu::{ItemColours, draw_item_value, draw_scrollbar};

/// C++ `MenuItem` (`menuItem.hpp:10-36`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub color: u8,
    pub dis_colour: u8,
    pub string: String,
    pub has_value: bool,
    pub value: String,
    pub visible: bool,
    pub selectable: bool,
    pub id: i32,
}

impl MenuItem {
    /// `MenuItem(color, dis_colour, string, id)` (`menuItem.hpp:11-16`).
    pub fn new(color: u8, dis_colour: u8, string: &str, id: i32) -> MenuItem {
        MenuItem {
            color,
            dis_colour,
            string: string.to_string(),
            has_value: false,
            value: String::new(),
            visible: true,
            selectable: true,
            id,
        }
    }

    /// `MenuItem::Space()` (`menuItem.hpp:18-22`): colours 0, empty, id -1, not selectable.
    pub fn space() -> MenuItem {
        let mut m = MenuItem::new(0, 0, "", -1);
        m.selectable = false;
        m
    }
}

/// The TC's menu sound hooks as sample ids (`sound_hook[SoundMenuMoveUp/Down/Select]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuHooks {
    pub move_up: i32,
    pub move_down: i32,
    pub select: i32,
}

/// What a behavior call reads or writes besides the model and the menu: `gfx.menu_cycles`
/// (`IntegerBehavior`'s scroll gate, `integerBehavior.cpp:15`) and the sound log.
pub struct MenuCx<'a> {
    pub menu_cycles: u32,
    pub hooks: MenuHooks,
    pub sounds: &'a mut Vec<i32>,
}

impl MenuCx<'_> {
    /// `g_sound_player->Play(hook)`: `SoundPlayer::Play` drops a negative id (`mixer/player.hpp:15-22`).
    pub fn play(&mut self, hook: i32) {
        if hook >= 0 {
            self.sounds.push(hook);
        }
    }
}

/// C++ `Menu`'s virtuals (`menu.hpp:55-67`).
pub trait MenuModel {
    /// `GetItemBehavior` (base: a plain `ItemBehavior`).
    fn behavior(&mut self, item_id: i32) -> Behavior<'_>;
    /// `Menu::OnUpdate`, run after every item's `OnUpdate` in `UpdateItems`.
    fn on_update(&mut self, _menu: &mut Menu) {}
    /// `DrawItemOverlay` (the player menu's colour bars, 4½f).
    #[allow(clippy::too_many_arguments)]
    fn draw_item_overlay(
        &self,
        _item: &MenuItem,
        _x: i32,
        _y: i32,
        _selected: bool,
        _disabled: bool,
        _bmp: &mut Bitmap,
        _pal: &Pal32,
    ) {
    }
}

/// A menu whose items all have the base behavior (C++ `MainMenu::GetItemBehavior`,
/// `mainMenu.cpp:6-10`).
pub struct PlainModel;

impl MenuModel for PlainModel {
    fn behavior(&mut self, _item_id: i32) -> Behavior<'_> {
        Behavior::Plain
    }
}

/// C++ `Menu` (`menu.hpp:26-197`). `selection` is private as in C++ (`selection_`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub item_height: i32,
    pub value_offset_x: i32,
    pub x: i32,
    pub y: i32,
    pub height: i32,
    /// A visible index.
    pub top_item: i32,
    /// A visible index.
    pub bottom_item: i32,
    pub visible_item_count: i32,
    pub centered: bool,
    pub search: Search,
    selection: i32,
}

impl Menu {
    /// `Menu(x, y, centered)`: `Init(centered)` + `Place(x, y)` (`menu.hpp:28-50`).
    pub fn new(x: i32, y: i32, centered: bool) -> Menu {
        Menu {
            items: Vec::new(),
            item_height: 8,
            value_offset_x: 0,
            x,
            y,
            height: 15,
            top_item: 0,
            bottom_item: 0,
            visible_item_count: 0,
            centered,
            search: Search::default(),
            selection: 0,
        }
    }

    pub fn selection(&self) -> i32 {
        self.selection
    }

    /// `SetSelection` (`menu.hpp:117-121`): raw, no visibility or scroll adjustment.
    pub fn set_selection(&mut self, selection: i32) {
        self.selection = selection;
    }

    pub fn is_selection_valid(&self) -> bool {
        self.selection >= 0 && (self.selection as usize) < self.items.len()
    }

    pub fn selected(&self) -> Option<&MenuItem> {
        self.is_selection_valid().then(|| &self.items[self.selection as usize])
    }

    pub fn selected_id(&self) -> i32 {
        self.selected().map_or(-1, |i| i.id)
    }

    pub fn index_from_id(&self, id: i32) -> i32 {
        self.items.iter().position(|i| i.id == id).map_or(-1, |p| p as i32)
    }

    pub fn item_from_id(&self, id: i32) -> Option<&MenuItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn item_from_id_mut(&mut self, id: i32) -> Option<&mut MenuItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// `AddItem(item)` (`menu.cpp:295-302`).
    pub fn add_item(&mut self, item: MenuItem) -> i32 {
        let idx = self.items.len() as i32;
        if item.visible {
            self.visible_item_count += 1;
        }
        self.items.push(item);
        idx
    }

    /// `AddItem(item, pos)` (`menu.cpp:311-317`): returns the size before the insert (quirk).
    pub fn add_item_at(&mut self, item: MenuItem, pos: usize) -> i32 {
        let idx = self.items.len() as i32;
        if item.visible {
            self.visible_item_count += 1;
        }
        self.items.insert(pos, item);
        idx
    }

    /// `Clear` (`menu.cpp:304-309`).
    pub fn clear(&mut self) {
        self.items.clear();
        self.visible_item_count = 0;
        self.set_top(0);
    }

    /// `SetHeight` (`menu.hpp:106-109`).
    pub fn set_height(&mut self, height: i32) {
        self.height = height;
        self.set_top(self.top_item);
    }

    /// `MoveTo` (`menu.cpp:129-134`).
    pub fn move_to(&mut self, new_selection: i32) {
        let s = new_selection.max(0).min(self.items.len() as i32 - 1);
        self.selection = self.first_visible_from(s);
        self.ensure_in_view(self.selection);
    }

    pub fn move_to_id(&mut self, id: i32) {
        self.move_to(self.index_from_id(id));
    }

    /// `MoveToFirstVisible` (`menu.cpp:136`).
    pub fn move_to_first_visible(&mut self) {
        let first = self.first_visible_from(0);
        self.move_to(first);
    }

    /// `IsInView` (`menu.cpp:138-141`).
    pub fn is_in_view(&self, item: i32) -> bool {
        let v = self.visible_item_index(item);
        v >= self.top_item && v < self.bottom_item
    }

    /// `ItemPosition` (`menu.cpp:143-153`) for the item at `index`.
    pub fn item_position(&self, index: usize) -> Option<(i32, i32)> {
        if !self.is_in_view(index as i32) {
            return None;
        }
        let v = self.visible_item_index(index as i32);
        Some((self.x, self.y + (v - self.top_item) * self.item_height))
    }

    /// `EnsureInView` (`menu.cpp:155-167`).
    pub fn ensure_in_view(&mut self, item: i32) {
        if item < 0 || item as usize >= self.items.len() || !self.items[item as usize].visible {
            return;
        }
        let v = self.visible_item_index(item);
        if v < self.top_item {
            self.set_top(v);
        } else if v >= self.bottom_item {
            self.set_bottom(v + 1);
        }
    }

    /// `FirstVisibleFrom` (`menu.cpp:169-177`): the next visible AND selectable item, or the item
    /// count. A negative start wraps (`std::size_t i = item`) and finds nothing.
    pub fn first_visible_from(&self, item: i32) -> i32 {
        if item < 0 {
            return self.items.len() as i32;
        }
        (item as usize..self.items.len())
            .find(|&i| self.items[i].visible && self.items[i].selectable)
            .map_or(self.items.len() as i32, |i| i as i32)
    }

    /// `LastVisibleFrom` (`menu.cpp:179-187`): one PAST the previous visible and selectable item,
    /// or 0. (C++ reads out of bounds for `item > size`; Rust clamps.)
    pub fn last_visible_from(&self, item: i32) -> i32 {
        let end = item.clamp(0, self.items.len() as i32) as usize;
        (0..end)
            .rev()
            .find(|&i| self.items[i].visible && self.items[i].selectable)
            .map_or(0, |i| i as i32 + 1)
    }

    /// `VisibleItemIndex` (`menu.cpp:189-201`): the number of visible items before `item`.
    pub fn visible_item_index(&self, item: i32) -> i32 {
        let mut idx = 0;
        for (i, it) in self.items.iter().enumerate() {
            if !it.visible {
                continue;
            }
            if i as i32 >= item {
                break;
            }
            idx += 1;
        }
        idx
    }

    /// `ItemFromVisibleIndex` (`menu.cpp:203-216`): the `idx`-th visible item, or the item count.
    pub fn item_from_visible_index(&self, idx: i32) -> i32 {
        let mut idx = idx;
        for (i, it) in self.items.iter().enumerate() {
            if !it.visible {
                continue;
            }
            if idx == 0 {
                return i as i32;
            }
            idx -= 1;
        }
        self.items.len() as i32
    }

    /// `SetBottom` (`menu.cpp:218`).
    pub fn set_bottom(&mut self, new_bottom: i32) {
        self.set_top(new_bottom - self.height);
    }

    /// `SetTop` (`menu.cpp:220-225`).
    pub fn set_top(&mut self, new_top: i32) {
        let t = new_top.min(self.visible_item_count - self.height).max(0);
        self.top_item = t;
        self.bottom_item = (t + self.height).min(self.visible_item_count);
    }

    /// `SetVisibility` (`menu.cpp:227-246`): count, re-anchor the top on the same real item,
    /// then `EnsureInView(selection)`.
    pub fn set_visibility(&mut self, id: i32, state: bool) {
        let item = self.index_from_id(id);
        if item < 0 {
            debug_assert!(false, "SetVisibility: no item with id {id}");
            return;
        }
        let it = item as usize;
        if self.items[it].visible && !state {
            self.visible_item_count -= 1;
        } else if !self.items[it].visible && state {
            self.visible_item_count += 1;
        }
        let real_top = self.item_from_visible_index(self.top_item);
        self.items[it].visible = state;
        let v = self.visible_item_index(real_top);
        self.set_top(v);
        self.ensure_in_view(self.selection);
    }

    /// `Scroll` (`menu.cpp:248`).
    pub fn scroll(&mut self, dir: i32) {
        self.set_top(self.top_item + dir);
    }

    /// `MovementPage` (`menu.cpp:250-261`).
    pub fn movement_page(&mut self, direction: i32) {
        let mut sel = self.visible_item_index(self.selection);
        let offset = direction * (self.height / 2);
        sel += offset;
        self.set_top(self.top_item + offset);
        sel = sel.max(0).min(self.visible_item_count - 1);
        let target = self.item_from_visible_index(sel);
        self.move_to(target);
    }

    /// `Movement` (`menu.cpp:263-293`): the next visible and selectable item, wrapping.
    pub fn movement(&mut self, direction: i32) {
        let n = self.items.len() as i32;
        let ok = |m: &Menu, i: i32| m.items[i as usize].visible && m.items[i as usize].selectable;
        if direction < 0 {
            let first = (0..self.selection).rev().chain(((self.selection + 1)..n).rev());
            for i in first {
                if ok(self, i) {
                    self.move_to(i);
                    return;
                }
            }
        } else if direction > 0 {
            let first = ((self.selection + 1)..n).chain(0..self.selection);
            for i in first {
                if ok(self, i) {
                    self.move_to(i);
                    return;
                }
            }
        }
    }

    /// `OnLeftRight` (`menu.hpp:69-76`): false with no selection; else the behavior's bool.
    /// A requested `UpdateItems` runs after the behavior is dropped (design §4.2).
    pub fn on_left_right<M: MenuModel>(&mut self, m: &mut M, dir: i32, cx: &mut MenuCx) -> bool {
        if !self.is_selection_valid() {
            return false;
        }
        let idx = self.selection as usize;
        let out = m.behavior(self.items[idx].id).on_left_right(self, idx, dir, cx);
        if out.update_all {
            self.update_items(m);
        }
        out.keep
    }

    /// `OnEnter` (`menu.hpp:78-85`): `Result(0)` (C++ `return false;`) with no selection.
    pub fn on_enter<M: MenuModel>(&mut self, m: &mut M, cx: &mut MenuCx) -> Enter {
        if !self.is_selection_valid() {
            return Enter::Result(0);
        }
        let idx = self.selection as usize;
        let (enter, update_all) = m.behavior(self.items[idx].id).on_enter(self, idx, cx);
        if update_all {
            self.update_items(m);
        }
        enter
    }

    /// `UpdateItems` (`menu.hpp:89-97`): every item's `OnUpdate`, then `Menu::OnUpdate`.
    pub fn update_items<M: MenuModel>(&mut self, m: &mut M) {
        for idx in 0..self.items.len() {
            m.behavior(self.items[idx].id).on_update(self, idx);
        }
        m.on_update(self);
    }

    /// `Draw` (`menu.cpp:81-125`): up to `height` visible rows from `top_item`, then the
    /// scrollbar iff `visible_item_count > height`. Any negative `x` means `self.x`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw<M: MenuModel>(
        &self,
        m: &M,
        bmp: &mut Bitmap,
        pal: &Pal32,
        font: &Font,
        disabled: bool,
        x: i32,
        show_disabled_selection: bool,
    ) {
        let x = if x < 0 { self.x } else { x };
        let mut items_left = self.height;
        let mut cur_y = self.y;
        let mut c = self.item_from_visible_index(self.top_item);
        while items_left > 0 && (c as usize) < self.items.len() {
            let item = &self.items[c as usize];
            if item.visible {
                items_left -= 1;
                let selected = c == self.selection && (!disabled || show_disabled_selection);
                let value = item.has_value.then_some(item.value.as_str());
                let colours = ItemColours { color: item.color, dis_colour: item.dis_colour };
                draw_item_value(
                    bmp, pal, font, &item.string, value, x, cur_y, selected, disabled,
                    self.centered, self.value_offset_x, colours,
                );
                m.draw_item_overlay(item, x, cur_y, selected, disabled, bmp, pal);
                cur_y += self.item_height;
            }
            c += 1;
        }
        if self.visible_item_count > self.height {
            draw_scrollbar(
                bmp, pal, font, x, self.y, self.height, self.item_height, self.top_item,
                self.visible_item_count,
            );
        }
    }
}
```

`on_left_right` and `on_enter` build the behavior from `m` while `self` is borrowed mutably by the call. That is legal because `Behavior<'_>` borrows `m`, not the menu. Should the borrow checker still reject the one-expression form, bind it first: `let mut b = m.behavior(id); let out = b.on_left_right(self, …); drop(b);`.

- [ ] **Step 4: GREEN**

Run: `rustfmt --edition 2024 /home/user/openliero/rust/ui/src/menu/mod.rs /home/user/openliero/rust/ui/src/menu/behavior.rs /home/user/openliero/rust/ui/src/menu/search.rs`
Run: `cd /home/user/openliero/rust && cargo test -p ui` — Expected: PASS (T1's 9 and T2's 14).
Run: `cd /home/user/openliero/rust && cargo clippy -p ui --all-targets 2>&1 | grep -E '^(warning|error)' | sort | uniq -c` — Expected: nothing new beyond `too_many_arguments`, which is allowed where annotated. Clippy is advisory, not a gate; fix only what is cheap.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/ui/src
git -C /home/user/openliero commit -m "ui(4.5d): the C++ Menu port — items, navigation, visibility, scrolling, draw + scrollbar, MenuModel" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 3: Behaviors, type-to-search, the settings display model, the main-menu table, `UiTc`  [Opus]

**Files:**
- Modify: `rust/ui/src/menu/behavior.rs` (the `Integer`, `Time`, `Bool`, `Enum` and `ArrayEnum` arms; `change`; `decimal_digits`)
- Modify: `rust/ui/src/menu/search.rs` (`Menu::on_keys`)
- Modify: `rust/ui/src/text.rs` (`UiTc`)
- Create: `rust/ui/src/shell/mod.rs`, `rust/ui/src/shell/main_menu.rs`, `rust/ui/src/shell/settings_menu.rs`
- Modify: `rust/ui/src/lib.rs` (`pub mod shell;`)

**Interfaces:**
- Produces (used by T5, T7, T11):
  - `Behavior::{Integer(Integer), Time { int, frames }, Bool, Enum, ArrayEnum}`, `Behavior::time(..)` and `menu::behavior::Integer`.
  - `Menu::on_keys(&mut self, keys: &[TypedKey], now_ms: u64, contains: bool)`.
  - `text::UiTc { copyright2, random2, regen_level, reload_level, blood_limit, blood_step_up, hooks: MenuHooks, begin }` with `UiTc::{from_tc, load}`.
  - `shell::main_menu::{MA_* ids, main_menu(), MainModel}`.
  - `shell::settings_menu::{SI_* ids, LOAD_OPTIONS, SAVE_OPTIONS, LOAD_CHANGE, settings_menu(), SettingsModel { settings, tc, setup_name }}`.
- Consumes: T2's `Menu`, `MenuCx`, `MenuModel`, `CustomBehavior`; T1's `text` and `keys`; `scenario::settings::{Settings, GM_*}`; `assets::tc::TcConfig`; `scenario::assets::read_asset`.

Why:
- Design §3.5–§3.6 and §4.3–§4.5.
- Finding 1: the main screen draws the SETTINGS menu, disabled, with every visible item's value. That needs each behavior's `OnUpdate`, `LevelSelectBehavior`'s relabel and `SettingsMenu::OnUpdate`'s per-mode visibility (`gfx.cpp:1222-1341`).
- The left/right/enter arms and type-to-search are framework for 4½e/4½f, gated now in G1 (design Q12).

- [ ] **Step 1: Write the failing tests**

(a) Append inside `mod tests` of `rust/ui/src/menu/mod.rs`:

```rust
    use crate::keys::TypedKey;

    const HOOKS: MenuHooks = MenuHooks { move_up: 25, move_down: 26, select: 27 };

    /// A model over local variables, one per item id.
    struct Vars {
        int: i32,
        time: i32,
        flag: bool,
        en: u32,
        broken: u32,
        mode: u32,
        entry: bool,
        div: i32,
    }

    impl MenuModel for Vars {
        fn behavior(&mut self, id: i32) -> Behavior<'_> {
            match id {
                1 => {
                    let mut b = behavior::Integer::new(&mut self.int, 0, 10, 3, false);
                    b.allow_entry = self.entry;
                    b.display_div = self.div;
                    Behavior::Integer(b)
                }
                2 => Behavior::time(&mut self.time, 60, 3600, 10, false),
                3 => Behavior::Bool(&mut self.flag),
                4 => Behavior::Enum { v: &mut self.en, min: 0, max: 3, broken: false },
                5 => Behavior::Enum { v: &mut self.broken, min: 1, max: 3, broken: true },
                6 => Behavior::ArrayEnum { v: &mut self.mode, arr: &crate::text::GAME_MODES, broken: false },
                _ => Behavior::Plain,
            }
        }
    }

    fn vars() -> Vars {
        Vars { int: 5, time: 600, flag: false, en: 0, broken: 2, mode: 3, entry: true, div: 1 }
    }

    fn vars_menu() -> Menu {
        let mut m = Menu::new(30, 20, false);
        m.value_offset_x = 60;
        for id in 1..=6 {
            m.add_item(MenuItem::new(48, 7, &format!("V{id}"), id));
        }
        m
    }

    fn values(m: &Menu) -> Vec<String> {
        m.items.iter().map(|i| if i.has_value { i.value.clone() } else { ".".into() }).collect()
    }

    #[test]
    fn on_update_fills_every_value() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.update_items(&mut v);
        assert_eq!(values(&m), ["5", "10:00", "OFF", "0", "2", "Scales of Justice"]);
    }

    #[test]
    fn integer_left_right_obeys_the_scroll_interval_and_clamps() {
        // integerBehavior.cpp:14-33: act only when menu_cycles % 5 == 0; clamp(v + dir*step).
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_first_visible();
        let mut sounds = Vec::new();
        for (cycles, dir, want) in [(1, 1, 5), (0, 1, 8), (5, 1, 10), (10, 1, 10), (0, -1, 7), (0, -1, 4), (0, -1, 1), (0, -1, 0), (0, -1, 0)] {
            let mut cx = MenuCx { menu_cycles: cycles, hooks: HOOKS, sounds: &mut sounds };
            assert!(m.on_left_right(&mut v, dir, &mut cx), "Integer returns true: the key stays held");
            assert_eq!(v.int, want, "cycles {cycles} dir {dir}");
        }
        assert!(sounds.is_empty(), "Integer left/right plays nothing");
        assert_eq!(m.items[0].value, "0", "OnUpdate ran on change");
    }

    #[test]
    fn integer_enter_requests_an_edit_only_when_allowed_and_in_view() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_first_visible();
        let mut sounds = Vec::new();
        let mut cx = MenuCx { menu_cycles: 0, hooks: HOOKS, sounds: &mut sounds };
        let e = m.on_enter(&mut v, &mut cx);
        assert_eq!(
            e,
            Enter::EditValue(ValueEntry { item_id: 1, initial: "5".into(), digits: 2, x: 30 + 60 + 2, y: 20, min: 0, max: 10, div: 1, percentage: false }),
            "integerBehavior.cpp:41-78: x = item x + voff + 2, digits = 1 + floor(log10(10))"
        );
        v.entry = false;
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1));
        v.entry = true;
        m.scroll(1);
        m.set_height(1);
        m.scroll(1);
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1), "ItemPosition false: no entry");
        assert_eq!(sounds, [27, 27, 27], "MenuSelect every time (integerBehavior.cpp:37)");
    }

    #[test]
    fn digits_is_one_plus_floor_log10_for_every_power_of_ten() {
        let mut n = 1i64;
        let mut d = 1;
        while n <= i32::MAX as i64 {
            for k in [n, n + 1, 2 * n - 1] {
                if k <= i32::MAX as i64 {
                    assert_eq!(behavior::decimal_digits(k as i32), d, "{k}");
                }
            }
            if n > 1 {
                assert_eq!(behavior::decimal_digits((n - 1) as i32), d - 1, "{}", n - 1);
            }
            n *= 10;
            d += 1;
        }
        assert_eq!(behavior::decimal_digits(i32::MAX), 10);
    }

    #[test]
    fn time_uses_the_integer_step_but_never_allows_entry() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.update_items(&mut v);
        m.move_to_id(2);
        let mut sounds = Vec::new();
        let mut cx = MenuCx { menu_cycles: 0, hooks: HOOKS, sounds: &mut sounds };
        assert!(m.on_left_right(&mut v, -1, &mut cx));
        assert_eq!((v.time, m.items[1].value.as_str()), (590, "09:50"));
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1), "timeBehavior.hpp:11: allow_entry = false");
        assert_eq!(sounds, [27]);
    }

    #[test]
    fn a_boolean_switch_toggles_with_crossed_sounds_and_releases_left_right() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_id(3);
        let mut sounds = Vec::new();
        let mut cx = MenuCx { menu_cycles: 1, hooks: HOOKS, sounds: &mut sounds };
        assert!(!m.on_left_right(&mut v, 1, &mut cx), "booleanSwitchBehavior.cpp:17: false");
        assert!(v.flag && m.items[2].value == "ON");
        assert!(!m.on_left_right(&mut v, -1, &mut cx));
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1));
        assert!(v.flag);
        assert_eq!(sounds, [25, 26, 27], "dir > 0: MoveUp, else MoveDown; Enter: Select");
    }

    #[test]
    fn an_enum_wraps_in_u32_and_updates_every_item() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_id(4);
        let mut sounds = Vec::new();
        let mut cx = MenuCx { menu_cycles: 3, hooks: HOOKS, sounds: &mut sounds };
        let mut seen = Vec::new();
        for dir in [-1, -1, 1, 1, 1] {
            assert!(!m.on_left_right(&mut v, dir, &mut cx));
            seen.push(v.en);
        }
        assert_eq!(seen, [3, 2, 3, 0, 1], "enumBehavior.cpp:31-39");
        assert_eq!(m.items[5].value, "Scales of Justice", "Change -> UpdateItems ran every OnUpdate");
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1));
        assert_eq!(v.en, 2);
        m.move_to_id(5);
        assert!(!m.on_left_right(&mut v, 1, &mut cx), "broken: false, no change, no sound");
        assert_eq!(v.broken, 2);
        m.on_enter(&mut v, &mut cx);
        assert_eq!(v.broken, 3, "broken only affects left/right");
        m.move_to_id(6);
        m.on_left_right(&mut v, 1, &mut cx);
        assert_eq!((v.mode, m.items[5].value.as_str()), (0, "Kill'em All"), "ArrayEnum: [0, N-1] wraps");
        assert_eq!(sounds.len(), 5 + 1 + 1 + 1, "5 left/right + Enter + broken Enter + array right");
    }

    fn search_menu() -> Menu {
        let mut m = Menu::new(40, 20, false);
        for (id, s) in ["APPLE", "APRICOT", "BANANA", "BLUEBERRY", "CHERRY GREY", "COCONUT", "DATE", "", "apricot jam"]
            .iter()
            .enumerate()
        {
            let mut it = MenuItem::new(48, 7, s, id as i32);
            it.visible = id != 3;
            it.selectable = id != 4;
            m.add_item(it);
        }
        m.move_to_first_visible();
        m
    }

    #[test]
    fn type_to_search_prefix_timeout_retry_and_skips() {
        // menu.cpp:14-79; ms from the caller's clock (design §4.5).
        let sym = |c: char| TypedKey::Sym(c as u32);
        let mut m = search_menu();
        let mut at = |m: &mut Menu, keys: &[TypedKey], t: u64, contains: bool| {
            m.on_keys(keys, t, contains);
            (m.selection(), m.search.prefix.clone())
        };
        assert_eq!(at(&mut m, &[sym('a'), sym('p')], 0, false), (0, "ap".into()), "APPLE stays (offs 0)");
        assert_eq!(at(&mut m, &[sym('r')], 200, false), (1, "apr".into()));
        assert_eq!(at(&mut m, &[sym('b')], 2000, false), (2, "b".into()), "> 1500 ms: a new prefix");
        assert_eq!(at(&mut m, &[sym('l')], 2100, false), (2, "".into()), "BLUEBERRY is invisible; retry 'l' fails");
        assert_eq!(at(&mut m, &[sym('c')], 2200, false), (5, "c".into()), "CHERRY (unselectable) -> MoveTo skips to COCONUT");
        assert_eq!(at(&mut m, &[TypedKey::Tab], 9000, false), (5, "c".into()), "Tab: no timeout, skip 1, CHERRY again");
        assert_eq!(at(&mut m, &[sym('j')], 9100, true), (8, "j".into()), "'cj' fails; retry 'j' contains");
        assert_eq!(at(&mut m, &[TypedKey::Tab], 9200, true), (0, "".into()), "no other 'j'; retry with '' matches APPLE");
        m.move_to(6);
        assert_eq!(at(&mut m, &[TypedKey::Tab], 9300, true), (8, "".into()), "contains '' never matches an empty string");
        m.move_to(6);
        assert_eq!(at(&mut m, &[TypedKey::Tab], 9400, false), (7, "".into()), "a prefix '' does");
        let before = (m.selection(), m.search.clone());
        m.on_keys(&[TypedKey::Sym(0x4000_0052)], 99_999, false);
        assert_eq!((m.selection(), m.search.clone()), before, "a non-ASCII symbol is ignored entirely");
    }
```

(b) Create `rust/ui/src/shell/mod.rs`:

```rust
//! Step 4½d — the C++ shell: `StateStack` (`state.hpp`), `MainMenuState`
//! (`mainMenuState.cpp`), the router and `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`). T3 adds the
//! two concrete menus; T6 moves the 4½c live modules here; T7 adds the stack, the screens and
//! `Shell`.
pub mod main_menu;
pub mod settings_menu;
```

Create `rust/ui/src/shell/main_menu.rs` with the module doc and tests:

```rust
//! The main menu (`MainMenu`, `mainMenu.hpp:9-25`; items `gfx.cpp:505-521`; design §3.3). T7 adds
//! `MainMenuState`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_menu_is_the_cpp_table() {
        let m = main_menu();
        assert_eq!((m.x, m.y, m.height, m.item_height), (53, 20, 15, 8), "gfx.cpp:266");
        let rows: Vec<(i32, u8, u8, &str, bool)> =
            m.items.iter().map(|i| (i.id, i.color, i.dis_colour, i.string.as_str(), i.selectable)).collect();
        assert_eq!(
            rows,
            [
                (MA_RESUME_GAME, 10, 10, "", true),
                (MA_NEW_GAME, 10, 10, "", true),
                (MA_HOST_GAME, 48, 48, "HOST LAN GAME", true),
                (MA_JOIN_GAME, 48, 48, "JOIN LAN GAME", true),
                (MA_HOST_ONLINE, 48, 48, "HOST ONLINE", true),
                (MA_JOIN_ONLINE, 48, 48, "JOIN ONLINE", true),
                (MA_ADVANCED, 48, 48, "OPTIONS (F2)", true),
                (MA_REPLAYS, 48, 48, "REPLAYS (F3)", true),
                (MA_TC, 48, 48, "TC", true),
                (MA_QUIT, 6, 6, "QUIT TO OS", true),
                (-1, 0, 0, "", false),
                (MA_PLAYER1_SETTINGS, 48, 48, "LEFT PLAYER (F5)", true),
                (MA_PLAYER2_SETTINGS, 48, 48, "RIGHT PLAYER (F6)", true),
                (MA_NET_PLAYER_SETTINGS, 48, 48, "NETWORK PLAYER (F9)", true),
                (MA_SETTINGS, 48, 48, "MATCH SETUP (F7)", true),
            ]
        );
        assert_eq!(m.visible_item_count, 15, "15 visible against height 15: never a scrollbar");
        assert_eq!(
            [MA_RESUME_GAME, MA_NEW_GAME, MA_SETTINGS, MA_QUIT, MA_REPLAY, MA_TC, MA_JOIN_ONLINE],
            [0, 1, 2, 6, 8, 9, 14],
            "mainMenu.hpp:9-25"
        );
    }
}
```

Create `rust/ui/src/shell/settings_menu.rs` with the module doc and tests:

```rust
//! The settings menu, display only in 4½d (`SettingsMenu`, `gfx.hpp:72-100`; items
//! `gfx.cpp:485-503`; behaviors `:1262-1312`; `OnUpdate` `:1314-1341`; design finding 1, §4.10).
//! The main screen draws it disabled at (178, 20) with `value_offset_x = 100`. 4½e gives it
//! focus, the LEVEL / WEAPON OPTIONS / SAVE / LOAD pushes and config I/O.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Enter, MenuCx};
    use crate::text::UiTc;
    use scenario::paths::TC_ROOT;
    use scenario::settings::{GM_GAME_OF_TAG, GM_HOLDAZONE, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE, Settings};
    use std::path::Path;

    fn tc() -> UiTc {
        UiTc::load(Path::new(TC_ROOT))
    }

    fn shown(m: &Menu) -> Vec<&str> {
        m.items.iter().filter(|i| i.visible).map(|i| i.string.as_str()).collect()
    }

    fn updated(s: &mut Settings) -> Menu {
        let tc = tc();
        let mut m = settings_menu();
        m.move_to_first_visible();
        m.update_items(&mut SettingsModel { settings: s, tc: &tc, setup_name: "liero" });
        m
    }

    #[test]
    fn the_settings_menu_is_19_items_in_loadmenus_order() {
        let m = settings_menu();
        assert_eq!((m.x, m.y, m.value_offset_x), (178, 20, 100), "gfx.cpp:267, :523");
        let ids: Vec<i32> = m.items.iter().map(|i| i.id).collect();
        assert_eq!(ids, [0, 2, 3, 4, 5, 1, 11, 12, 13, 6, 15, 7, 8, 9, 10, 18, 14, 17, 16]);
        assert!(m.items.iter().all(|i| (i.color, i.dis_colour) == (48, 7)));
        assert_eq!(m.items[16].string, "REGENERATE LEVEL");
        assert_eq!(m.visible_item_count, 19);
    }

    #[test]
    fn the_defaults_show_15_items_with_their_values() {
        let mut s = Settings::default();
        let m = updated(&mut s);
        let rows: Vec<(&str, &str)> =
            m.items.iter().filter(|i| i.visible).map(|i| (i.string.as_str(), i.value.as_str())).collect();
        assert_eq!(
            rows,
            [
                ("GAME MODE", "Kill'em All"),
                ("LIVES", "15"),
                ("LEVEL", "Random"),
                ("MAP WIDTH", "504"),
                ("MAP HEIGHT", "350"),
                ("LOADING TIMES", "100%"),
                ("WEAPON OPTIONS", ""),
                ("MAX BONUSES", "4"),
                ("NAMES ON BONUSES", "OFF"),
                ("MAP", "ON"),
                ("AMOUNT OF BLOOD", "100%"),
                ("LOAD+CHANGE", "ON"),
                ("REGENERATE LEVEL", "OFF"),
                ("SAVE SETUP AS...", "liero"),
                ("LOAD SETUP", ""),
            ]
        );
        assert!(!m.items[10].has_value && !m.items[18].has_value, "WEAPON OPTIONS, LOAD SETUP: no value");
        assert_eq!(m.visible_item_count, 15);
    }

    #[test]
    fn visibility_follows_the_game_mode_and_the_level_kind() {
        for (mode, random, count, extra) in [
            (GM_KILL_EM_ALL, true, 15, "LIVES"),
            (GM_SCALES_OF_JUSTICE, true, 15, "LIVES"),
            (GM_GAME_OF_TAG, true, 15, "TIME TO LOSE"),
            (GM_HOLDAZONE, true, 16, "ZONE TIMEOUT"),
            (GM_KILL_EM_ALL, false, 13, "LIVES"),
            (GM_HOLDAZONE, false, 14, "TIME TO WIN"),
        ] {
            let mut s = Settings { game_mode: mode, random_level: random, ..Settings::default() };
            let m = updated(&mut s);
            assert_eq!(m.visible_item_count, count, "mode {mode} random {random}");
            assert!(shown(&m).contains(&extra));
            assert!(!shown(&m).contains(&"FLAGS TO WIN"), "never shown (gfx.cpp:1319)");
            assert_eq!(shown(&m).contains(&"MAP WIDTH"), random);
        }
    }

    #[test]
    fn a_file_level_shows_its_quoted_basename_and_relabels_regenerate() {
        let mut s = Settings { random_level: false, level_file: "Levels/water_stage.lev".into(), ..Settings::default() };
        let m = updated(&mut s);
        let level = m.item_from_id(SI_LEVEL).unwrap();
        assert_eq!(level.value, "\"water_stage\"", "gfx.cpp:1227");
        assert_eq!(m.item_from_id(SI_REGENERATE_LEVEL).unwrap().string, "RELOAD LEVEL");
        let mut s = Settings::default();
        assert_eq!(updated(&mut s).item_from_id(SI_REGENERATE_LEVEL).unwrap().string, "REGENERATE LEVEL");
    }

    #[test]
    fn odd_values_render_as_cpp_does() {
        let mut s = Settings { game_mode: GM_GAME_OF_TAG, time_to_lose: 3599, loading_time: 9999, blood: 0, ..Settings::default() };
        let m = updated(&mut s);
        let v = |id| m.item_from_id(id).unwrap().value.clone();
        assert_eq!((v(SI_TIME_TO_LOSE), v(SI_LOADING_TIMES), v(SI_AMOUNT_OF_BLOOD)), ("59:59".into(), "9999%".into(), "0%".into()));
    }

    #[test]
    fn the_behaviors_edit_the_settings() {
        let tc = tc();
        let mut s = Settings::default();
        let mut m = settings_menu();
        m.move_to_first_visible();
        let mut sounds = Vec::new();
        let mut cx = MenuCx { menu_cycles: 0, hooks: tc.hooks, sounds: &mut sounds };
        let mut model = SettingsModel { settings: &mut s, tc: &tc, setup_name: "liero" };
        m.update_items(&mut model);
        m.on_left_right(&mut model, 1, &mut cx); // GAME MODE: Kill'em All -> Game of Tag
        assert!(m.item_from_id(SI_TIME_TO_LOSE).unwrap().visible, "Change -> UpdateItems -> OnUpdate");
        m.move_to_id(SI_AMOUNT_OF_BLOOD);
        m.on_left_right(&mut model, 1, &mut cx);
        assert_eq!(m.on_enter(&mut model, &mut cx), Enter::Result(-1), "blood: allow_entry = false");
        m.move_to_id(SI_LIVES);
        assert!(m.selected_id() != SI_LIVES, "LIVES is hidden in Game of Tag");
        drop(model);
        assert_eq!((s.game_mode, s.blood), (GM_GAME_OF_TAG, 125), "BloodStepUp = 25 (tc.cfg:70)");
    }
}
```

(c) Append inside `mod tests` of `rust/ui/src/text.rs`:

```rust
    #[test]
    fn ui_tc_is_read_from_the_tc() {
        let tc = UiTc::load(std::path::Path::new(scenario::paths::TC_ROOT));
        assert_eq!(tc.copyright2, "Liero v1.33 (c) Mets\u{e4}nEl\u{e4}met 1998,1999", "tc.cfg:244");
        assert_eq!((tc.random2.as_str(), tc.regen_level.as_str(), tc.reload_level.as_str()), ("Random", "REGENERATE LEVEL", "RELOAD LEVEL"));
        assert_eq!((tc.blood_limit, tc.blood_step_up), (500, 25));
        assert_eq!((tc.hooks.move_up, tc.hooks.move_down, tc.hooks.select, tc.begin), (25, 26, 27, 22), "tc.cfg [sounds]");
    }
```

In `rust/ui/src/lib.rs`, add `pub mod shell;` after `pub mod menu;`.

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p ui 2>&1 | tail -8`
Expected: FAIL to compile, with `no variant 'Integer'`, `no function 'time'`, `no method 'on_keys'`, `cannot find 'main_menu'`, `'settings_menu'`, `'SettingsModel'` and `'UiTc'`.

- [ ] **Step 3: Implement**

(1) `rust/ui/src/menu/behavior.rs`. Add `use crate::text::{ONOFF, time_to_string, time_to_string_frames};`, then replace the `Behavior` enum and its `impl` with:

```rust
/// `IntegerBehavior` (`integerBehavior.hpp:8-32`).
pub struct Integer<'a> {
    pub v: &'a mut i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub percentage: bool,
    pub scroll_interval: i32,
    pub display_div: i32,
    pub allow_entry: bool,
}

impl<'a> Integer<'a> {
    pub fn new(v: &'a mut i32, min: i32, max: i32, step: i32, percentage: bool) -> Integer<'a> {
        Integer { v, min, max, step, percentage, scroll_interval: 5, display_div: 1, allow_entry: true }
    }
}

pub enum Behavior<'a> {
    /// The base `ItemBehavior`: `OnLeftRight` true, `OnEnter` -1, `OnUpdate` nothing.
    Plain,
    Integer(Integer<'a>),
    /// `TimeBehavior` (`timeBehavior.hpp:9-17`): `IntegerBehavior` with `allow_entry = false` and
    /// the time string; build it with [`Behavior::time`].
    Time { int: Integer<'a>, frames: bool },
    /// `BooleanSwitchBehavior` with its default setter (`booleanSwitchBehavior.hpp:12-13`).
    Bool(&'a mut bool),
    /// `EnumBehavior` (`enumBehavior.hpp:9-25`).
    Enum { v: &'a mut u32, min: u32, max: u32, broken: bool },
    /// `ArrayEnumBehavior` (`arrayEnumBehavior.hpp:9-23`): an enum over `[0, arr.len() - 1]`.
    ArrayEnum { v: &'a mut u32, arr: &'static [&'static str], broken: bool },
    Custom(Box<dyn CustomBehavior + 'a>),
}

const KEEP: LeftRight = LeftRight { keep: true, update_all: false };

impl<'a> Behavior<'a> {
    /// `TimeBehavior(common, v, min, max, step, frames)`.
    pub fn time(v: &'a mut i32, min: i32, max: i32, step: i32, frames: bool) -> Behavior<'a> {
        let mut int = Integer::new(v, min, max, step, false);
        int.allow_entry = false;
        Behavior::Time { int, frames }
    }

    pub fn on_left_right(&mut self, menu: &mut Menu, idx: usize, dir: i32, cx: &mut MenuCx) -> LeftRight {
        let (out, update_self) = match self {
            Behavior::Plain => (KEEP, false),
            // integerBehavior.cpp:14-33 (TimeBehavior inherits it).
            Behavior::Integer(b) | Behavior::Time { int: b, .. } => {
                if cx.menu_cycles % (b.scroll_interval as u32) != 0 {
                    return KEEP;
                }
                let mut new_v = *b.v;
                if (dir < 0 && new_v > b.min) || (dir > 0 && new_v < b.max) {
                    new_v = (new_v + dir * b.step).clamp(b.min, b.max);
                }
                let changed = new_v != *b.v;
                *b.v = new_v;
                (KEEP, changed)
            }
            // booleanSwitchBehavior.cpp:8-18.
            Behavior::Bool(v) => {
                let hook = if dir > 0 { cx.hooks.move_up } else { cx.hooks.move_down };
                cx.play(hook);
                **v = !**v;
                (LeftRight { keep: false, update_all: false }, true)
            }
            // enumBehavior.cpp:9-22.
            Behavior::Enum { v, min, max, broken } => {
                if *broken {
                    return LeftRight { keep: false, update_all: false };
                }
                let hook = if dir > 0 { cx.hooks.move_up } else { cx.hooks.move_down };
                cx.play(hook);
                (LeftRight { keep: false, update_all: change(v, *min, *max, dir) }, false)
            }
            Behavior::ArrayEnum { v, arr, broken } => {
                if *broken {
                    return LeftRight { keep: false, update_all: false };
                }
                let hook = if dir > 0 { cx.hooks.move_up } else { cx.hooks.move_down };
                cx.play(hook);
                (LeftRight { keep: false, update_all: change(v, 0, arr.len() as u32 - 1, dir) }, false)
            }
            Behavior::Custom(c) => return c.on_left_right(menu, idx, dir, cx),
        };
        if update_self {
            self.on_update(menu, idx);
        }
        out
    }

    pub fn on_enter(&mut self, menu: &mut Menu, idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        match self {
            Behavior::Plain => (Enter::Result(-1), false),
            // integerBehavior.cpp:36-80.
            Behavior::Integer(b) | Behavior::Time { int: b, .. } => {
                cx.play(cx.hooks.select);
                if !b.allow_entry {
                    return (Enter::Result(-1), false);
                }
                let Some((x, y)) = menu.item_position(idx) else {
                    return (Enter::Result(-1), false);
                };
                let x = x + menu.value_offset_x;
                let (min, max) = (b.min / b.display_div, b.max / b.display_div);
                debug_assert!(max > 0, "C++ floor(log10(<= 0)) is undefined");
                let entry = ValueEntry {
                    item_id: menu.items[idx].id,
                    initial: (*b.v / b.display_div).to_string(),
                    digits: decimal_digits(max),
                    x: x + 2,
                    y,
                    min,
                    max,
                    div: b.display_div,
                    percentage: b.percentage,
                };
                (Enter::EditValue(entry), false)
            }
            // booleanSwitchBehavior.cpp:20-25.
            Behavior::Bool(v) => {
                cx.play(cx.hooks.select);
                **v = !**v;
                self.on_update(menu, idx);
                (Enter::Result(-1), false)
            }
            // enumBehavior.cpp:24-29 (`broken` does not apply to Enter).
            Behavior::Enum { v, min, max, .. } => {
                cx.play(cx.hooks.select);
                (Enter::Result(-1), change(v, *min, *max, 1))
            }
            Behavior::ArrayEnum { v, arr, .. } => {
                cx.play(cx.hooks.select);
                (Enter::Result(-1), change(v, 0, arr.len() as u32 - 1, 1))
            }
            Behavior::Custom(c) => c.on_enter(menu, idx, cx),
        }
    }

    pub fn on_update(&self, menu: &mut Menu, idx: usize) {
        let value = match self {
            Behavior::Plain => return,
            // integerBehavior.cpp:82-88.
            Behavior::Integer(b) => {
                let mut s = (*b.v / b.display_div).to_string();
                if b.percentage {
                    s.push('%');
                }
                s
            }
            // timeBehavior.cpp:8-11.
            Behavior::Time { int, frames } => {
                if *frames { time_to_string_frames(*int.v) } else { time_to_string(*int.v) }
            }
            Behavior::Bool(v) => ONOFF[**v as usize].to_string(),
            Behavior::Enum { v, .. } => v.to_string(),
            Behavior::ArrayEnum { v, arr, .. } => arr[**v as usize].to_string(),
            Behavior::Custom(c) => return c.on_update(menu, idx),
        };
        let item = &mut menu.items[idx];
        item.value = value;
        item.has_value = true;
    }
}

/// `EnumBehavior::Change` (`enumBehavior.cpp:31-39`): all `uint32_t`, `dir` converted to
/// `uint32_t`. Returns whether `v` changed (C++ then calls `menu.UpdateItems`).
fn change(v: &mut u32, min: u32, max: u32, dir: i32) -> bool {
    let range = max.wrapping_sub(min).wrapping_add(1);
    let new_v = (v.wrapping_add(dir as u32).wrapping_add(range).wrapping_sub(min) % range)
        .wrapping_add(min);
    let changed = new_v != *v;
    *v = new_v;
    changed
}

/// `1 + floor(log10(n))` for `n >= 1` in integer arithmetic (`integerBehavior.cpp:47`): equal for
/// every positive `i32` (the powers-of-ten sweep in the tests).
pub fn decimal_digits(n: i32) -> i32 {
    let (mut n, mut d) = (n, 1);
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}
```

In the `Behavior::Bool` arm of `on_enter`, the borrow of `v` ends before `self.on_update`. If the compiler disagrees, toggle through a local (`let b = !**v; **v = b;`) and call `on_update` after the `match`, as `on_left_right` does.

(2) `rust/ui/src/menu/search.rs`. Add after the struct:

```rust
use super::Menu;
use crate::keys::TypedKey;

/// `std::toupper` in the "C" locale: ASCII only.
fn same(a: u8, b: u8) -> bool {
    a.to_ascii_uppercase() == b.to_ascii_uppercase()
}

/// A case-insensitive prefix match, or with `contains` the `std::ranges::search` of
/// `menu.cpp:44-47`, whose empty needle matches any non-empty string.
fn matches(hay: &str, needle: &str, contains: bool) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if contains {
        if n.is_empty() {
            return !h.is_empty();
        }
        h.windows(n.len()).any(|w| w.iter().zip(n).all(|(a, b)| same(*a, *b)))
    } else {
        h[..n.len()].iter().zip(n).all(|(a, b)| same(*a, *b))
    }
}

impl Menu {
    /// `Menu::OnKeys` (`menu.cpp:14-79`; design §3.6). `now_ms` stands for `SDL_GetTicks()`.
    pub fn on_keys(&mut self, keys: &[TypedKey], now_ms: u64, contains: bool) {
        for &key in keys {
            let (sym, is_tab) = match key {
                TypedKey::Tab => (b'\t' as u32, true),
                TypedKey::Sym(s) => (s, false),
            };
            if !((32..=127).contains(&sym) || is_tab) {
                continue;
            }
            if !is_tab && now_ms.wrapping_sub(self.search.time_ms) > 1500 {
                self.search.prefix.clear();
            }
            loop {
                let was_empty = self.search.prefix.is_empty();
                let mut new_prefix = self.search.prefix.clone();
                if !is_tab {
                    new_prefix.push(sym as u8 as char);
                }
                self.search.time_ms = now_ms;
                let n = self.items.len();
                let skip = usize::from(is_tab);
                let mut found = false;
                for offs in skip..n {
                    let i = (self.selection() as usize).wrapping_add(offs) % n;
                    let item = &self.items[i];
                    if item.visible
                        && item.string.len() >= new_prefix.len()
                        && matches(&item.string, &new_prefix, contains)
                    {
                        found = true;
                        self.move_to(i as i32);
                        break;
                    }
                }
                if found {
                    self.search.prefix = new_prefix;
                    break;
                }
                self.search.prefix.clear();
                if was_empty {
                    break;
                }
            }
        }
    }
}
```

(3) `rust/ui/src/text.rs`. Add above the tests:

```rust
use std::path::Path;

use assets::tc::TcConfig;

use crate::menu::MenuHooks;

/// What the menus read from the TC: `common.s[..]` strings (`tc.cfg [texts]`), `common.c[..]`
/// constants, and `common.sound_hook[..]` as sample ids (`tc.cfg [sounds]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiTc {
    pub copyright2: String,
    pub random2: String,
    pub regen_level: String,
    pub reload_level: String,
    pub blood_limit: i32,
    pub blood_step_up: i32,
    pub hooks: MenuHooks,
    /// `SoundBegin` (`game.cpp:500-503`, played by `StartGame`).
    pub begin: i32,
}

impl UiTc {
    pub fn from_tc(tc: &TcConfig) -> UiTc {
        UiTc {
            copyright2: tc.texts.Copyright2.clone(),
            random2: tc.texts.Random2.clone(),
            regen_level: tc.texts.RegenLevel.clone(),
            reload_level: tc.texts.ReloadLevel.clone(),
            blood_limit: tc.constants.BloodLimit,
            blood_step_up: tc.constants.BloodStepUp,
            hooks: MenuHooks {
                move_up: tc.sound_hooks.MenuMoveUp,
                move_down: tc.sound_hooks.MenuMoveDown,
                select: tc.sound_hooks.MenuSelect,
            },
            begin: tc.sound_hooks.Begin,
        }
    }

    pub fn load(tc_root: &Path) -> UiTc {
        let bytes = scenario::assets::read_asset(tc_root, "tc.cfg");
        UiTc::from_tc(&TcConfig::load(&bytes).expect("tc.cfg parses"))
    }
}
```

(4) `rust/ui/src/shell/main_menu.rs`. Add above the tests:

```rust
use crate::menu::{Menu, MenuItem, PlainModel};

/// `MainMenu` item ids (`mainMenu.hpp:9-25`).
pub const MA_RESUME_GAME: i32 = 0;
pub const MA_NEW_GAME: i32 = 1;
pub const MA_SETTINGS: i32 = 2;
pub const MA_PLAYER1_SETTINGS: i32 = 3;
pub const MA_PLAYER2_SETTINGS: i32 = 4;
pub const MA_ADVANCED: i32 = 5;
pub const MA_QUIT: i32 = 6;
pub const MA_REPLAYS: i32 = 7;
pub const MA_REPLAY: i32 = 8;
pub const MA_TC: i32 = 9;
pub const MA_HOST_GAME: i32 = 10;
pub const MA_JOIN_GAME: i32 = 11;
pub const MA_NET_PLAYER_SETTINGS: i32 = 12;
pub const MA_HOST_ONLINE: i32 = 13;
pub const MA_JOIN_ONLINE: i32 = 14;

/// `Gfx::LoadMenus`'s main menu (`gfx.cpp:505-521`) at (53, 20) (`gfx.cpp:266`). RESUME and NEW
/// GAME get their strings in `MainMenuState::enter`, TC its `"TC (<tc>)"`.
pub fn main_menu() -> Menu {
    let mut m = Menu::new(53, 20, false);
    for (c, s, id) in [
        (10, "", MA_RESUME_GAME),
        (10, "", MA_NEW_GAME),
        (48, "HOST LAN GAME", MA_HOST_GAME),
        (48, "JOIN LAN GAME", MA_JOIN_GAME),
        (48, "HOST ONLINE", MA_HOST_ONLINE),
        (48, "JOIN ONLINE", MA_JOIN_ONLINE),
        (48, "OPTIONS (F2)", MA_ADVANCED),
        (48, "REPLAYS (F3)", MA_REPLAYS),
        (48, "TC", MA_TC),
        (6, "QUIT TO OS", MA_QUIT),
    ] {
        m.add_item(MenuItem::new(c, c, s, id));
    }
    m.add_item(MenuItem::space());
    for (s, id) in [
        ("LEFT PLAYER (F5)", MA_PLAYER1_SETTINGS),
        ("RIGHT PLAYER (F6)", MA_PLAYER2_SETTINGS),
        ("NETWORK PLAYER (F9)", MA_NET_PLAYER_SETTINGS),
        ("MATCH SETUP (F7)", MA_SETTINGS),
    ] {
        m.add_item(MenuItem::new(48, 48, s, id));
    }
    m
}

/// `MainMenu::GetItemBehavior` (`mainMenu.cpp:6-10`): every item is the base behavior;
/// `MainMenuState::Update` intercepts them all.
pub type MainModel = PlainModel;
```

(5) `rust/ui/src/shell/settings_menu.rs`. Add above the tests:

```rust
use scenario::settings::{GM_GAME_OF_TAG, GM_HOLDAZONE, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE, Settings};

use crate::menu::behavior::Integer;
use crate::menu::{Behavior, CustomBehavior, Menu, MenuItem, MenuModel};
use crate::text::{GAME_MODES, UiTc, leaf_basename};

/// `SettingsMenu` item ids (`gfx.hpp:73-93`).
pub const SI_GAME_MODE: i32 = 0;
pub const SI_LIVES: i32 = 1;
pub const SI_TIME_TO_LOSE: i32 = 2;
pub const SI_TIME_TO_WIN: i32 = 3;
pub const SI_ZONE_TIMEOUT: i32 = 4;
pub const SI_FLAGS_TO_WIN: i32 = 5;
pub const SI_LOADING_TIMES: i32 = 6;
pub const SI_MAX_BONUSES: i32 = 7;
pub const SI_NAMES_ON_BONUSES: i32 = 8;
pub const SI_MAP: i32 = 9;
pub const SI_AMOUNT_OF_BLOOD: i32 = 10;
pub const SI_LEVEL: i32 = 11;
pub const SI_RANDOM_MAP_WIDTH: i32 = 12;
pub const SI_RANDOM_MAP_HEIGHT: i32 = 13;
pub const SI_REGENERATE_LEVEL: i32 = 14;
pub const SI_WEAPON_OPTIONS: i32 = 15;
pub const LOAD_OPTIONS: i32 = 16;
pub const SAVE_OPTIONS: i32 = 17;
pub const LOAD_CHANGE: i32 = 18;

/// `Gfx::LoadMenus`'s settings menu (`gfx.cpp:485-503`, `:523`) at (178, 20) (`gfx.cpp:267`).
pub fn settings_menu() -> Menu {
    let mut m = Menu::new(178, 20, false);
    for (s, id) in [
        ("GAME MODE", SI_GAME_MODE),
        ("TIME TO LOSE", SI_TIME_TO_LOSE),
        ("TIME TO WIN", SI_TIME_TO_WIN),
        ("ZONE TIMEOUT", SI_ZONE_TIMEOUT),
        ("FLAGS TO WIN", SI_FLAGS_TO_WIN),
        ("LIVES", SI_LIVES),
        ("LEVEL", SI_LEVEL),
        ("MAP WIDTH", SI_RANDOM_MAP_WIDTH),
        ("MAP HEIGHT", SI_RANDOM_MAP_HEIGHT),
        ("LOADING TIMES", SI_LOADING_TIMES),
        ("WEAPON OPTIONS", SI_WEAPON_OPTIONS),
        ("MAX BONUSES", SI_MAX_BONUSES),
        ("NAMES ON BONUSES", SI_NAMES_ON_BONUSES),
        ("MAP", SI_MAP),
        ("AMOUNT OF BLOOD", SI_AMOUNT_OF_BLOOD),
        ("LOAD+CHANGE", LOAD_CHANGE),
        ("REGENERATE LEVEL", SI_REGENERATE_LEVEL),
        ("SAVE SETUP AS...", SAVE_OPTIONS),
        ("LOAD SETUP", LOAD_OPTIONS),
    ] {
        m.add_item(MenuItem::new(48, 7, s, id));
    }
    m.value_offset_x = 100;
    m
}

/// `SettingsMenu`'s virtuals over the live `Settings` (C++ `gfx.settings`). `setup_name` is
/// `GetBasename(GetLeaf(gfx.settings_node.FullPath()))`: `"liero"` until 4½e loads setups.
pub struct SettingsModel<'a> {
    pub settings: &'a mut Settings,
    pub tc: &'a UiTc,
    pub setup_name: &'a str,
}

/// `LevelSelectBehavior::OnUpdate` (`gfx.cpp:1222-1238`), which relabels REGENERATE LEVEL.
struct LevelSelect<'a> {
    random_level: bool,
    level_file: &'a str,
    tc: &'a UiTc,
}

impl CustomBehavior for LevelSelect<'_> {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].has_value = true;
        let label = if self.random_level {
            menu.items[idx].value = self.tc.random2.clone();
            &self.tc.regen_level
        } else {
            menu.items[idx].value = format!("\"{}\"", leaf_basename(self.level_file));
            &self.tc.reload_level
        };
        menu.item_from_id_mut(SI_REGENERATE_LEVEL)
            .expect("the settings menu has REGENERATE LEVEL")
            .string = label.clone();
    }
}

/// `OptionsSaveBehavior::OnUpdate` (`gfx.cpp:1245-1253`).
struct OptionsSave<'a> {
    name: &'a str,
}

impl CustomBehavior for OptionsSave<'_> {
    fn on_update(&self, menu: &mut Menu, idx: usize) {
        menu.items[idx].value = self.name.to_string();
        menu.items[idx].has_value = true;
    }
}

impl MenuModel for SettingsModel<'_> {
    /// `SettingsMenu::GetItemBehavior` (`gfx.cpp:1262-1312`), item for item.
    fn behavior(&mut self, item_id: i32) -> Behavior<'_> {
        let SettingsModel { settings: s, tc, setup_name } = self;
        match item_id {
            SI_NAMES_ON_BONUSES => Behavior::Bool(&mut s.names_on_bonuses),
            SI_MAP => Behavior::Bool(&mut s.map),
            SI_REGENERATE_LEVEL => Behavior::Bool(&mut s.regenerate_level),
            SI_LOADING_TIMES => Behavior::Integer(Integer::new(&mut s.loading_time, 0, 9999, 1, true)),
            SI_MAX_BONUSES => Behavior::Integer(Integer::new(&mut s.max_bonuses, 0, 99, 1, false)),
            SI_AMOUNT_OF_BLOOD => {
                let mut b = Integer::new(&mut s.blood, 0, tc.blood_limit, tc.blood_step_up, true);
                b.allow_entry = false;
                Behavior::Integer(b)
            }
            SI_LIVES => Behavior::Integer(Integer::new(&mut s.lives, 1, 999, 1, false)),
            SI_TIME_TO_LOSE | SI_TIME_TO_WIN => Behavior::time(&mut s.time_to_lose, 60, 3600, 10, false),
            SI_ZONE_TIMEOUT => Behavior::time(&mut s.zone_timeout, 10, 3600, 10, false),
            SI_FLAGS_TO_WIN => Behavior::Integer(Integer::new(&mut s.flags_to_win, 1, 999, 1, false)),
            SI_LEVEL => Behavior::Custom(Box::new(LevelSelect {
                random_level: s.random_level,
                level_file: &s.level_file,
                tc,
            })),
            SI_RANDOM_MAP_WIDTH => Behavior::Integer(Integer::new(&mut s.random_map_width, 64, 4096, 8, false)),
            SI_RANDOM_MAP_HEIGHT => Behavior::Integer(Integer::new(&mut s.random_map_height, 64, 4096, 8, false)),
            SI_GAME_MODE => Behavior::ArrayEnum { v: &mut s.game_mode, arr: &GAME_MODES, broken: false },
            SAVE_OPTIONS => Behavior::Custom(Box::new(OptionsSave { name: setup_name })),
            LOAD_CHANGE => Behavior::Bool(&mut s.load_change),
            // WEAPON OPTIONS, LOAD SETUP: behaviors with no display (their pushes are 4½e's).
            _ => Behavior::Plain,
        }
    }

    /// `SettingsMenu::OnUpdate` (`gfx.cpp:1314-1341`).
    fn on_update(&mut self, menu: &mut Menu) {
        for id in [SI_LIVES, SI_TIME_TO_LOSE, SI_TIME_TO_WIN, SI_ZONE_TIMEOUT, SI_FLAGS_TO_WIN] {
            menu.set_visibility(id, false);
        }
        menu.set_visibility(SI_RANDOM_MAP_WIDTH, self.settings.random_level);
        menu.set_visibility(SI_RANDOM_MAP_HEIGHT, self.settings.random_level);
        match self.settings.game_mode {
            GM_KILL_EM_ALL | GM_SCALES_OF_JUSTICE => menu.set_visibility(SI_LIVES, true),
            GM_GAME_OF_TAG => menu.set_visibility(SI_TIME_TO_LOSE, true),
            GM_HOLDAZONE => {
                menu.set_visibility(SI_TIME_TO_WIN, true);
                menu.set_visibility(SI_ZONE_TIMEOUT, true);
            }
            _ => {}
        }
    }
}
```

The destructuring gives `s: &mut &mut Settings`, `tc: &mut &UiTc` and `setup_name: &mut &str`. Where a `&UiTc` or `&str` is needed, pass `*tc` / `*setup_name` if inference does not auto-deref. `ArrayEnum`'s `arr` is `&'static [&'static str]`, so `&GAME_MODES` coerces.

- [ ] **Step 4: GREEN**

Run: `rustfmt --edition 2024 /home/user/openliero/rust/ui/src/shell/mod.rs /home/user/openliero/rust/ui/src/shell/main_menu.rs /home/user/openliero/rust/ui/src/shell/settings_menu.rs /home/user/openliero/rust/ui/src/menu/behavior.rs /home/user/openliero/rust/ui/src/menu/search.rs /home/user/openliero/rust/ui/src/menu/mod.rs /home/user/openliero/rust/ui/src/text.rs`
Run: `cd /home/user/openliero/rust && cargo test -p ui` — Expected: PASS (T1 9 + T2 14 + T3 16).
Run: `cd /home/user/openliero/rust && cargo tree -p ui -e normal | grep -c bevy` — Expected: `0`.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/ui/src
git -C /home/user/openliero commit -m "ui(4.5d): behaviors (Integer/Time/Bool/Enum/ArrayEnum), type-to-search, the settings display model, the main-menu table, UiTc" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 4: C++ — `oracle_dump_menu` (G1: the real `Menu`, behaviors and `SettingsMenu`)  [Opus]

**Files:**
- Create: `src/tools/oracle_dump/menu_dump.cpp`
- Modify: `CMakeLists.txt` (inside the `if(OPENLIERO_BUILD_ORACLE_DUMP)` block, after the `oracle_dump_weapsel` lines `:396-397`)

**Interfaces:**
- Produces (used by T5): `build/linux-x64/Release/oracle_dump_menu <script.txt> <out.txt> [--ppm-dir <dir>]`, run from the repo root. The **G1 script grammar** and the **G1 line format**:

```
# comment                                  (a '#' line; blank lines are skipped)
menu <x> <y> <height> <value_offset_x> <centered 0|1>     widget script: the first directive
settings <setup.cfg|default>                              settings script: the first directive
item <id> <color> <dis> <visible 0|1> <selectable 0|1> <label…>   widget: label = rest of line; "" = empty
space                                                     widget: MenuItem::Space()
bind <id> integer <init> <min> <max> <step> <pct> <interval> <div> <entry>
bind <id> time <init> <min> <max> <step> <frames>
bind <id> bool <init>
bind <id> enum <init> <min> <max> <broken>
bind <id> array <init> gamemodes|onoff <broken>
--- ops (after the declarations; each writes one output line) ---
move <d> | page <d> | scroll <d> | set_height <h> | visible <id> <0|1> | move_to <i> |
move_to_id <id> | first_visible | cycles <n> | left | right | enter | keys <tok…> |
keys_contains <tok…> | sleep_ms <n> | update_items | draw <disabled> <x> <show_dis_sel>
key tokens: a..z 0..9 SPACE TAB MINUS UP
```

```
<n> <op> <sel> <top> <bottom> <vis> <shown> <prefix> <vals> <bound> <sounds> <ret> <push> <hash>
```

- **`shown`**: one `0`/`1` per item (`visible`).
- **`prefix`**: `search_prefix` with spaces written as `_`, or `-` if empty.
- **`vals`**: the items joined with `|`. Each entry is `.` without `has_value`, `-` for an empty value, or the value with spaces written as `_`.
- **`bound`**: the bound variables in id order, joined with `,` (bool `0`/`1`). It is `-` for settings scripts or when nothing is bound.
- **`sounds`**: the sample ids played during this op, joined with `,`, or `-`.
- **`ret`**:
  - `left` / `right`: `0` or `1`;
  - `enter`: the `int`;
  - every other op: `-`.
- **`push`**: how many states an `enter` pushed (always popped straight away).
- **`hash`**: `draw`: `hash_frame(bmp, 33)` as `%016x` over the 320×200 `play_renderer` bitmap, filled with colour 0 first and drawn through the plain TC palette; every other op: `-`.

- Consumes: the real `Menu`, `MenuItem`, `IntegerBehavior`, `TimeBehavior`, `BooleanSwitchBehavior`, `EnumBehavior`, `ArrayEnumBehavior`, `Gfx::LoadMenus`, `SettingsMenu`, `Settings::FromToml`, `Common::load`, `Renderer`, and `weapsel_drive::{RecordingSoundPlayer, Fail}`.

Why:
- Design §6.1 and Q12: the scrollbar, every behavior's left/right/enter, the selected value arm, scrolling and type-to-search are not reachable from the 4½d main menu, and 4½e/4½f build on them.
- The settings display on the main screen (finding 1) has to be gated for all four game modes and both level kinds before G2 can pass.

This dumper runs the real code: nothing is replicated.

- [ ] **Step 1: Write the dumper**

Create `src/tools/oracle_dump/menu_dump.cpp`:

```cpp
// Generates the C++ side of the Rust menu-widget gate G1 (Step 4½, slice 4½d; design §6.1;
// rust/oracle-tests/tests/menu_widget_golden.rs). A script drives either
//   * a widget menu: a REAL Menu subclass whose GetItemBehavior maps item ids to the REAL
//     Integer/Time/BooleanSwitch/Enum/ArrayEnum behaviors over variables this dumper owns, or
//   * the REAL SettingsMenu (gfx.settings_menu after the REAL Gfx::LoadMenus) over the REAL
//     Settings::FromToml of a setup sidecar, with gfx.settings_node = data/Setups/liero.cfg,
// through a list of ops, and writes one line per op:
//   <n> <op> <sel> <top> <bottom> <vis> <shown> <prefix> <vals> <bound> <sounds> <ret> <push>
//   <hash>
// (grammar and fields: docs/superpowers/plans/2026-09-26-liero-rs-step4.5-slice4.5d-plan.md,
// Task 4). Sounds are logged by a RecordingSoundPlayer installed as g_sound_player; an
// InputStringState an IntegerBehavior::OnEnter pushes is counted and popped at once; `draw`
// fills play_renderer.bmp with colour 0, draws through the plain TC palette, and hashes the
// identity pixels (render/src/hash.rs). `sleep_ms` really sleeps (SDL_Delay) because OnKeys reads
// SDL_GetTicks; only 0, 1..1000 and >= 1600 are allowed, so scheduler jitter cannot flip the
// 1500 ms test. Usage (from the repo root):
//   oracle_dump_menu <script.txt> <out.txt> [--ppm-dir <dir>]
// Built via OPENLIERO_BUILD_ORACLE_DUMP (rust/oracle-tests/gen_menu_golden.sh). Not part of the
// default build.
#include <SDL3/SDL.h>

#include <array>
#include <cinttypes>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"
#include "gfx.hpp"
#include "gfx/bitmap.hpp"
#include "gfx/blit.hpp"
#include "keys.hpp"
#include "math.hpp"
#include "menu/arrayEnumBehavior.hpp"
#include "menu/booleanSwitchBehavior.hpp"
#include "menu/enumBehavior.hpp"
#include "menu/integerBehavior.hpp"
#include "menu/itemBehavior.hpp"
#include "menu/menu.hpp"
#include "menu/menuItem.hpp"
#include "menu/timeBehavior.hpp"
#include "mixer/player.hpp"
#include "settings.hpp"
#include "weapsel_drive.hpp"

namespace {

using weapsel_drive::Fail;
using weapsel_drive::RecordingSoundPlayer;

constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr uint64_t kFnvPrime = 1099511628211ULL;
constexpr int kIdentityFade = 33;

uint64_t FnvByte(uint64_t h, uint8_t b) { return (h ^ b) * kFnvPrime; }

uint8_t FadeChannel(uint8_t v, int amount) {
  return amount >= 32 ? v : static_cast<uint8_t>((v * amount) >> 5);
}

uint64_t HashFrame(Bitmap const& bmp, int fade) {
  uint64_t h = kFnvOffset;
  for (int y = 0; y < bmp.h; ++y) {
    for (int x = 0; x < bmp.w; ++x) {
      uint32_t const kC = bmp.GetPixel(x, y);
      h = FnvByte(h, FadeChannel((kC >> 16) & 0xFFU, fade));
      h = FnvByte(h, FadeChannel((kC >> 8) & 0xFFU, fade));
      h = FnvByte(h, FadeChannel(kC & 0xFFU, fade));
    }
  }
  return h;
}

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

void WritePpm(std::string const& path, Bitmap const& bmp) {
  std::string data = "P6\n" + std::to_string(bmp.w) + " " + std::to_string(bmp.h) + "\n255\n";
  for (int y = 0; y < bmp.h; ++y) {
    for (int x = 0; x < bmp.w; ++x) {
      uint32_t const kC = bmp.GetPixel(x, y);
      data += static_cast<char>((kC >> 16) & 0xFFU);
      data += static_cast<char>((kC >> 8) & 0xFFU);
      data += static_cast<char>(kC & 0xFFU);
    }
  }
  std::ofstream f(path, std::ios::binary | std::ios::trunc);
  f << data;
  if (!f) {
    Fail("cannot write " + path);
  }
}

std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

// One bound variable and its behavior's parameters.
struct Bind {
  std::string kind;  // integer | time | bool | enum | array
  int i = 0;
  uint32_t u = 0;
  bool b = false;
  int min = 0;
  int max = 0;
  int step = 1;
  int interval = 5;
  int div = 1;
  bool pct = false;
  bool entry = true;
  bool frames = false;
  bool broken = false;
  std::string arr;  // gamemodes | onoff

  std::string Value() const {
    if (kind == "bool") {
      return b ? "1" : "0";
    }
    if (kind == "enum" || kind == "array") {
      return std::to_string(u);
    }
    return std::to_string(i);
  }
};

// A widget menu: GetItemBehavior builds the REAL behavior over the bound variable, as C++ menus
// do (menu.hpp:69-97).
struct ScriptMenu : Menu {
  ScriptMenu(int x, int y, bool centered, std::map<int, Bind>* binds)
      : Menu(x, y, centered), binds_(binds) {}

  ItemBehavior* GetItemBehavior(Common& common, MenuItem& item) override {
    auto it = binds_->find(item.id);
    if (it == binds_->end()) {
      return Menu::GetItemBehavior(common, item);
    }
    Bind& b = it->second;
    if (b.kind == "integer") {
      auto* r = new IntegerBehavior(common, b.i, b.min, b.max, b.step, b.pct);
      r->scroll_interval = b.interval;
      r->display_div = b.div;
      r->allow_entry = b.entry;
      return r;
    }
    if (b.kind == "time") {
      return new TimeBehavior(common, b.i, b.min, b.max, b.step, b.frames);
    }
    if (b.kind == "bool") {
      return new BooleanSwitchBehavior(common, b.b);
    }
    if (b.kind == "enum") {
      return new EnumBehavior(common, b.u, static_cast<uint32_t>(b.min),
                              static_cast<uint32_t>(b.max), b.broken);
    }
    if (b.arr == "gamemodes") {
      return new ArrayEnumBehavior(common, b.u, common.texts.game_modes, b.broken);
    }
    return new ArrayEnumBehavior(common, b.u, common.texts.onoff, b.broken);
  }

 private:
  std::map<int, Bind>* binds_;
};

SDL_Scancode ScancodeOf(std::string const& t) {
  if (t.size() == 1 && t[0] >= 'a' && t[0] <= 'z') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_A + (t[0] - 'a'));
  }
  if (t.size() == 1 && t[0] >= '1' && t[0] <= '9') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_1 + (t[0] - '1'));
  }
  if (t == "0") {
    return SDL_SCANCODE_0;
  }
  if (t == "SPACE") {
    return SDL_SCANCODE_SPACE;
  }
  if (t == "TAB") {
    return SDL_SCANCODE_TAB;
  }
  if (t == "MINUS") {
    return SDL_SCANCODE_MINUS;
  }
  if (t == "UP") {
    return SDL_SCANCODE_UP;
  }
  Fail("unknown key token " + t);
}

std::string Token(std::string s) {
  if (s.empty()) {
    return "-";
  }
  for (char& c : s) {
    if (c == ' ') {
      c = '_';
    }
  }
  return s;
}

std::string Line(int n, std::string const& op, Menu& m, std::map<int, Bind> const& binds,
                 RecordingSoundPlayer& rec, std::string const& ret, std::size_t push,
                 std::string const& hash) {
  std::string shown;
  std::string vals;
  for (std::size_t i = 0; i < m.items.size(); ++i) {
    MenuItem const& it = m.items[i];
    shown += it.visible ? '1' : '0';
    if (i != 0) {
      vals += '|';
    }
    vals += it.has_value ? Token(it.value) : ".";
  }
  std::string bound;
  for (auto const& [id, b] : binds) {
    bound += (bound.empty() ? "" : ",") + b.Value();
  }
  std::string sounds;
  for (int const kId : rec.played) {
    sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
  }
  rec.played.clear();
  return std::to_string(n) + " " + op + " " + std::to_string(m.Selection()) + " " +
         std::to_string(m.top_item) + " " + std::to_string(m.bottom_item) + " " +
         std::to_string(m.visible_item_count) + " " + (shown.empty() ? "-" : shown) + " " +
         Token(m.search_prefix) + " " + (vals.empty() ? "-" : vals) + " " +
         (bound.empty() ? "-" : bound) + " " + (sounds.empty() ? "-" : sounds) + " " + ret + " " +
         std::to_string(push) + " " + hash + "\n";
}

}  // namespace

int main(int argc, char** argv) {
  std::vector<std::string> const kArgs(argv + 1, argv + argc);
  std::vector<std::string> positional;
  std::string ppm_dir;
  for (std::size_t i = 0; i < kArgs.size(); ++i) {
    if (kArgs[i] == "--ppm-dir" && i + 1 < kArgs.size()) {
      ppm_dir = kArgs[++i];
    } else if (kArgs[i].starts_with("--")) {
      Fail("unknown or incomplete option " + kArgs[i]);
    } else {
      positional.push_back(kArgs[i]);
    }
  }
  if (positional.size() != 2) {
    Fail("usage: oracle_dump_menu <script.txt> <out.txt> [--ppm-dir <dir>]");
  }
  std::string const& script_path = positional[0];

  // OnKeys reads SDL_GetKeyFromScancode (menu.cpp:17) and SDL_GetTicks (:21).
  if (!SDL_Init(SDL_INIT_EVENTS)) {
    Fail(std::string("SDL_Init: ") + SDL_GetError());
  }
  if (SDL_GetKeyFromScancode(SDL_SCANCODE_A, SDL_KMOD_NONE, false) != SDLK_A) {
    Fail("SDL_GetKeyFromScancode(A) is not 'a' without a video subsystem");
  }
  InitKeys();
  PrecomputeTables();
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");
  gfx.common = common;
  gfx.play_renderer.Init(320, 200);
  gfx.play_renderer.LoadPalette(*common);  // the plain TC palette (renderer.cpp:14-19)
  auto rec = std::make_shared<RecordingSoundPlayer>();
  gfx.sound_player = rec;
  g_sound_player = rec.get();

  std::istringstream in(Slurp(script_path));
  std::map<int, Bind> binds;
  std::unique_ptr<ScriptMenu> own;
  Menu* menu = nullptr;
  std::string out = "# oracle_dump_menu " + script_path +
                    " — the REAL C++ Menu / behaviors / SettingsMenu (Step 4½d design §6.1)\n"
                    "# <n> <op> <sel> <top> <bottom> <vis> <shown> <prefix> <vals> <bound> "
                    "<sounds> <ret> <push> <hash>\n";
  int n = 0;
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string op;
    if (!(ls >> op) || op[0] == '#') {
      continue;
    }
    if (op == "menu" || op == "settings") {
      if (menu != nullptr) {
        Fail("a second menu/settings directive: " + line);
      }
      if (op == "menu") {
        int x = 0;
        int y = 0;
        int height = 0;
        int voff = 0;
        int centered = 0;
        if (!(ls >> x >> y >> height >> voff >> centered)) {
          Fail("bad menu line: " + line);
        }
        own = std::make_unique<ScriptMenu>(x, y, centered != 0, &binds);
        own->height = height;
        own->value_offset_x = voff;
        menu = own.get();
      } else {
        std::string setup;
        ls >> setup;
        auto settings = std::make_shared<Settings>();
        if (setup != "default") {
          try {
            settings->FromToml(Slurp(DirOf(script_path) + "/" + setup));
          } catch (std::exception const& e) {
            Fail("settings " + setup + ": " + e.what());
          }
        }
        gfx.settings = settings;
        gfx.settings_node = FsNode("data") / "Setups" / "liero.cfg";  // OptionsSave: "liero"
        gfx.LoadMenus();
        menu = &gfx.settings_menu;
      }
      continue;
    }
    if (menu == nullptr) {
      Fail("the first directive must be menu or settings: " + line);
    }
    if (op == "item" || op == "space" || op == "bind") {
      if (!own) {
        Fail(op + " is for widget scripts: " + line);
      }
      if (op == "space") {
        own->AddItem(MenuItem::Space());
      } else if (op == "item") {
        int id = 0;
        int color = 0;
        int dis = 0;
        int visible = 0;
        int selectable = 0;
        if (!(ls >> id >> color >> dis >> visible >> selectable)) {
          Fail("bad item line: " + line);
        }
        std::string label;
        std::getline(ls >> std::ws, label);
        if (label == "\"\"") {
          label.clear();
        }
        MenuItem item(static_cast<PalIdx>(color), static_cast<PalIdx>(dis), label, id);
        item.visible = visible != 0;
        item.selectable = selectable != 0;
        own->AddItem(item);
      } else {
        int id = 0;
        Bind b;
        ls >> id >> b.kind;
        bool ok = true;
        if (b.kind == "integer") {
          int pct = 0;
          int entry = 0;
          ok = static_cast<bool>(ls >> b.i >> b.min >> b.max >> b.step >> pct >> b.interval >>
                                 b.div >> entry);
          b.pct = pct != 0;
          b.entry = entry != 0;
        } else if (b.kind == "time") {
          int frames = 0;
          ok = static_cast<bool>(ls >> b.i >> b.min >> b.max >> b.step >> frames);
          b.frames = frames != 0;
        } else if (b.kind == "bool") {
          int v = 0;
          ok = static_cast<bool>(ls >> v);
          b.b = v != 0;
        } else if (b.kind == "enum") {
          int broken = 0;
          ok = static_cast<bool>(ls >> b.u >> b.min >> b.max >> broken);
          b.broken = broken != 0;
        } else if (b.kind == "array") {
          int broken = 0;
          ok = static_cast<bool>(ls >> b.u >> b.arr >> broken) &&
               (b.arr == "gamemodes" || b.arr == "onoff");
          b.broken = broken != 0;
        } else {
          ok = false;
        }
        if (!ok || !binds.emplace(id, b).second) {
          Fail("bad or duplicate bind line: " + line);
        }
      }
      continue;
    }

    std::string ret = "-";
    std::size_t push = 0;
    std::string hash = "-";
    int a = 0;
    int b = 0;
    int c = 0;
    if (op == "move" && (ls >> a)) {
      menu->Movement(a);
    } else if (op == "page" && (ls >> a)) {
      menu->MovementPage(a);
    } else if (op == "scroll" && (ls >> a)) {
      menu->Scroll(a);
    } else if (op == "set_height" && (ls >> a)) {
      menu->SetHeight(a);
    } else if (op == "visible" && (ls >> a >> b)) {
      menu->SetVisibility(a, b != 0);
    } else if (op == "move_to" && (ls >> a)) {
      menu->MoveTo(a);
    } else if (op == "move_to_id" && (ls >> a)) {
      menu->MoveToId(a);
    } else if (op == "first_visible") {
      menu->MoveToFirstVisible();
    } else if (op == "cycles") {
      std::string v;
      ls >> v;
      gfx.menu_cycles = static_cast<unsigned>(std::strtoul(v.c_str(), nullptr, 10));
    } else if (op == "left" || op == "right") {
      ret = menu->OnLeftRight(*common, op == "left" ? -1 : 1) ? "1" : "0";
    } else if (op == "enter") {
      std::size_t const kBefore = gfx.state_stack.Size();
      ret = std::to_string(menu->OnEnter(*common));
      push = gfx.state_stack.Size() - kBefore;
      while (gfx.state_stack.Size() > kBefore) {
        gfx.state_stack.Pop();
      }
    } else if (op == "keys" || op == "keys_contains") {
      std::vector<SDL_Scancode> keys;
      std::string t;
      while (ls >> t) {
        keys.push_back(ScancodeOf(t));
      }
      menu->OnKeys(keys.data(), keys.data() + keys.size(), op == "keys_contains");
    } else if (op == "sleep_ms" && (ls >> a)) {
      if (!(a == 0 || (a >= 1 && a <= 1000) || a >= 1600)) {
        Fail("sleep_ms must be 0, 1..1000 or >= 1600 (the 1500 ms search timeout): " + line);
      }
      SDL_Delay(static_cast<uint32_t>(a));
    } else if (op == "update_items") {
      menu->UpdateItems(*common);
    } else if (op == "draw" && (ls >> a >> b >> c)) {
      Fill(gfx.play_renderer.bmp, 0);
      menu->Draw(*common, gfx.play_renderer, a != 0, b, c != 0);
      std::array<char, 24> buf{};
      std::snprintf(buf.data(), buf.size(), "%016" PRIx64,
                    HashFrame(gfx.play_renderer.bmp, kIdentityFade));
      hash = buf.data();
      if (!ppm_dir.empty()) {
        std::array<char, 32> name{};
        std::snprintf(name.data(), name.size(), "/menu_%04d.ppm", n);
        WritePpm(ppm_dir + name.data(), gfx.play_renderer.bmp);
      }
    } else {
      Fail("bad op line: " + line);
    }
    out += Line(n, op, *menu, binds, *rec, ret, push, hash);
    ++n;
  }

  std::ofstream f(positional[1], std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail("cannot write " + positional[1]);
  }
  std::printf("oracle_dump_menu: %d ops\n", n);
  g_sound_player = nullptr;
  SDL_Quit();
  return 0;
}
```

`PalIdx` comes from `gfx/color.hpp`, which `menuItem.hpp` includes. If `SDLK_A` is not the SDL3 spelling in the pinned SDL, use `SDLK_A` → `'a'` (the same value, 0x61).

`CMakeLists.txt`: after `target_link_libraries(oracle_dump_weapsel PRIVATE game)` add

```cmake
  add_executable(oracle_dump_menu src/tools/oracle_dump/menu_dump.cpp)
  target_link_libraries(oracle_dump_menu PRIVATE game)
```

- [ ] **Step 2: Build it and prove it runs**

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && cd /home/user/openliero && cmake --build build/linux-x64 --config Release --target oracle_dump_menu 2>&1 | tail -5`
Expected: builds.

Write a scratch script `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g1_smoke.txt`:

```
menu 40 20 15 60 0
item 1 48 7 1 1 COUNT
item 2 48 7 1 1 SWITCH
bind 1 integer 5 0 10 3 0 5 1 1
bind 2 bool 0
update_items
first_visible
right
enter
move 1
left
keys s
draw 0 -1 0
```

Run: `cd /home/user/openliero && build/linux-x64/Release/oracle_dump_menu /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g1_smoke.txt /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g1_smoke_out.txt && cat /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g1_smoke_out.txt`
Expected: `oracle_dump_menu: 8 ops`, then 2 header lines and 8 op lines. Check these by hand against the C++ source, because they are the first G1 evidence:
- `0 update_items 0 0 0 2 11 - 5|OFF 5,0 - - 0 -`;
- `right` at `menu_cycles` 0 gives `8`, with `ret 1`;
- `enter` gives `ret -1`, `push 1`, `sounds 27`;
- `left` on SWITCH gives `ret 0`, `sounds 26`, `bound 8,1`;
- `keys s` keeps SWITCH (offset 0 matches) with prefix `s`;
- `draw` gives a 16-hex hash.

If `SDL_GetKeyFromScancode(A)` fails without video, add `SDL_INIT_VIDEO` and run with `SDL_VIDEODRIVER=dummy` (record that in the header comment and in the gen script). If that also fails, **stop and report** (Addendum A).

Settings smoke: write `…/scratchpad/g1_settings.txt` containing `settings default`, `update_items`, `first_visible`, `draw 1 -1 0`. Run the dumper on it.
Expected: 3 lines. The `update_items` line has `vis 15` and `shown 1000011111111111111`: the item order is `gfx.cpp:485-503`, and `OnUpdate` hides TIME TO LOSE, TIME TO WIN, ZONE TIMEOUT and FLAGS TO WIN. Its `vals` is `Kill'em_All|10:00|10:00|00:30|20|15|Random|504|350|100%|.|4|OFF|ON|100%|ON|OFF|liero|.`. `UpdateItems` gives every item its value, hidden ones included; only WEAPON OPTIONS and LOAD SETUP have none. Compare it against the Rust `the_defaults_show_15_items_with_their_values` test.

- [ ] **Step 3: clang-format + clang-tidy**

Run: `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/menu_dump.cpp` — Expected: no output (else apply `-i` and re-check).
Run: `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 a61cbb6` — Expected: exit 0.

- [ ] **Step 4: Commit**

```
git -C /home/user/openliero add src/tools/oracle_dump/menu_dump.cpp CMakeLists.txt
git -C /home/user/openliero commit -m "oracle(4.5d): oracle_dump_menu — the REAL Menu, behaviors and SettingsMenu driven by G1 scripts" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 5: G1 — the widget corpus, `gen_menu_golden.sh`, the C++ goldens, `menu_widget_golden.rs`  [Opus]

**Files:**
- Create: `rust/oracle-tests/golden/menu_<s>_script.txt` for 15 scripts, and `menu_settings_{gametag,holdazone,scales,file_level}_setup.cfg`
- Create: `rust/oracle-tests/gen_menu_golden.sh`
- Create (generated by the script, then committed): `rust/oracle-tests/golden/menu_<s>.txt` ×15
- Create: `rust/oracle-tests/tests/menu_common/mod.rs`, `rust/oracle-tests/tests/menu_widget_golden.rs`
- Modify: `rust/oracle-tests/Cargo.toml` (`[dev-dependencies]`: `ui = { path = "../ui" }`)

**Interfaces:**
- Consumes: T4's `oracle_dump_menu` and its grammar and line format; T2/T3's `ui::menu`, `ui::shell::settings_menu`, `ui::text::UiTc`; `scenario::settings_toml::settings_from_toml`.
- Produces: the **G1 gate** (design done-when 1). `menu_common::replay(name) -> (Vec<String>, Vec<(usize, Bitmap)>)`.

Why: design §6.1. The scripts are hand-written, because each one targets named corners (design §4.4, §11.1). The goldens are C++'s, and there is no Rust self-golden.

- [ ] **Step 1: Write the 15 scripts and 4 setups** (in `rust/oracle-tests/golden/`)

Every script starts with a `#` header naming the slice and what it pins. The op lines are exactly as below. In `values_draw` the `ä` is literal UTF-8.

`menu_nav_wrap_script.txt`:

```
# Step 4½d G1 nav_wrap: Movement wraps over hidden/unselectable items; MoveTo clamps and skips
# forward; IndexFromId -1; bottom_item 0 before the first move; disabled / x-override draws.
menu 40 20 15 0 0
item 10 48 7 1 1 ALPHA
item 11 48 7 1 1 BETA
space
item 12 48 7 0 1 HIDDEN
item 13 48 7 1 0 GREY
item 14 48 7 1 1 OMEGA
draw 0 -1 0
first_visible
move 1
move 1
move 1
move 1
move -1
move -1
move -1
move_to 99
move_to -5
move_to 2
move_to_id 13
move_to_id 12
move_to_id 77
draw 0 -1 0
draw 1 -1 1
draw 1 -1 0
draw 0 60 0
```

`menu_page_scroll_script.txt`: `menu 40 10 15 0 0`, then 20 items: `item k 48 7 1 1 ITEM kk` for k = 0..6 (for example `item 0 48 7 1 1 ITEM 00`), a `space`, and `item k 48 7 1 1 ITEM kk` for k = 8..19. Then:

```
first_visible
page 1
page 1
page 1
draw 0 -1 0
page -1
page -1
page -1
move -1
draw 0 -1 0
move 1
scroll 3
draw 0 -1 0
scroll -10
set_height 10
draw 0 -1 0
move_to 15
set_height 15
draw 0 -1 0
```

`menu_visibility_script.txt`: `menu 40 10 5 0 0`, then `item k 48 7 1 1 ROW k` for k = 0..9. Then:

```
draw 0 -1 0
move_to 4
visible 4 0
draw 0 -1 0
visible 0 0
visible 1 0
draw 0 -1 0
move_to 9
visible 2 0
visible 9 0
draw 0 -1 0
visible 9 1
visible 0 1
visible 4 1
visible 1 1
visible 2 1
draw 0 -1 0
move 1
move -1
```

`menu_scrollbar_positions_script.txt`: `menu 60 12 15 0 0`, then `item k 48 7 1 1 LINE k` for k = 0..22. Then, for k = 0..22 in order, the two lines `move_to k` and `draw 0 -1 0` (46 lines). Generate the loop part with `for k in $(seq 0 22); do echo "move_to $k"; echo "draw 0 -1 0"; done`.

`menu_integer_script.txt`:

```
# Step 4½d G1 integer: the menu_cycles % scroll_interval gate, clamping at both ends, the
# percentage and display_div values, Enter with and without allow_entry, Enter out of view.
menu 30 20 15 60 0
item 1 48 7 1 1 COUNT
item 2 48 7 1 1 PERCENT
item 3 48 7 1 1 COLOUR
item 4 48 7 1 1 NOENTRY
bind 1 integer 5 0 10 3 0 5 1 1
bind 2 integer 100 0 9999 1 1 5 1 1
bind 3 integer 252 0 252 4 0 4 4 1
bind 4 integer 1 1 999 1 0 5 1 0
update_items
first_visible
cycles 1
right
cycles 0
right
right
right
left
left
left
left
left
enter
move 1
cycles 10
left
right
right
enter
move 1
cycles 4
left
left
right
enter
move 1
enter
left
draw 0 -1 0
move_to 0
set_height 2
scroll 1
enter
set_height 15
draw 0 -1 0
```

`menu_time_script.txt`:

```
# Step 4½d G1 time: TimeToString / TimeToStringFrames values, the Integer step and clamp,
# TimeBehavior never allowing entry.
menu 30 20 15 60 0
item 1 48 7 1 1 TIME
item 2 48 7 1 1 FRAMES
item 3 48 7 1 1 ZONE
bind 1 time 600 60 3600 10 0
bind 2 time 4286 0 100000 70 1
bind 3 time 3599 10 3600 10 0
update_items
first_visible
cycles 0
left
right
right
enter
move 1
right
left
left
move 1
right
right
draw 0 -1 0
```

`menu_bool_enum_script.txt`:

```
# Step 4½d G1 bool_enum: BooleanSwitch left/right/enter sounds; Enum u32 wrap both ways and
# UpdateItems; broken left/right; ArrayEnum over game_modes and onoff.
menu 30 20 15 80 0
item 1 48 7 1 1 SWITCH
item 2 48 7 1 1 ENUM
item 3 48 7 1 1 BROKEN
item 4 48 7 1 1 MODE
item 5 48 7 1 1 ONOFF
bind 1 bool 0
bind 2 enum 0 0 3 0
bind 3 enum 2 1 3 1
bind 4 array 3 gamemodes 0
bind 5 array 0 onoff 1
update_items
first_visible
right
left
enter
move 1
left
left
right
enter
enter
enter
move 1
left
right
enter
move 1
right
right
left
enter
move 1
left
enter
draw 0 -1 0
```

`menu_values_draw_script.txt`:

```
# Step 4½d G1 values_draw: a centred menu with the value arm — selected (two boxes), unselected
# (shadows), disabled with and without show_disabled_selection, an x override — and CP437 text.
menu 160 30 15 50 1
item 1 10 9 1 1 FIRST
item 2 48 7 1 1 SECOND LONGER
item 3 6 6 1 1 Metsän Elämet
bind 1 integer 42 0 99 1 1 5 1 1
bind 2 bool 1
update_items
first_visible
draw 0 -1 0
draw 1 -1 1
draw 1 -1 0
draw 0 100 0
move 1
draw 0 -1 0
move 1
draw 0 -1 0
```

`menu_search_script.txt`:

```
# Step 4½d G1 search: prefix, continuation, the 1500 ms timeout, the single-character retry,
# invisible items skipped, an unselectable match moving forward, Tab, contains (and its
# empty-needle quirk), a non-ASCII key ignored.
menu 40 20 15 0 0
item 1 48 7 1 1 APPLE
item 2 48 7 1 1 APRICOT
item 3 48 7 1 1 BANANA
item 4 48 7 0 1 BLUEBERRY
item 5 48 7 1 0 CHERRY GREY
item 6 48 7 1 1 COCONUT
item 7 48 7 1 1 DATE
item 8 48 7 1 1 ""
item 9 48 7 1 1 apricot jam
space
first_visible
keys a p
keys r
sleep_ms 1000
keys i
sleep_ms 1600
keys b
keys l
keys c
keys h
sleep_ms 1600
keys TAB
keys TAB TAB
keys SPACE
keys UP
keys_contains j a m
sleep_ms 1600
keys_contains TAB
move_to 6
keys_contains TAB
move_to 6
keys TAB
keys 0 MINUS
draw 0 -1 0
```

`menu_settings_killemall_script.txt`: `settings default`, then:

```
draw 1 -1 0
update_items
draw 1 -1 0
first_visible
draw 1 -1 0
page 1
draw 0 -1 0
```

then 16 lines of `move 1`, then `draw 0 -1 0`. The first draw is before `UpdateItems`: 19 visible items, no values, a scrollbar and `bottom_item` 0.

`menu_settings_gametag_script.txt`: `settings menu_settings_gametag_setup.cfg`, then `update_items`, `first_visible`, `draw 1 -1 0`, `move 1`, `draw 0 -1 0`, `move -1`, `cycles 0`, `left`, `draw 1 -1 0`.

`menu_settings_holdazone_script.txt`: `settings menu_settings_holdazone_setup.cfg`, then `update_items`, `first_visible`, `draw 1 -1 0`, `page 1`, `draw 0 -1 0`, `page 1`, `draw 0 -1 0`, `move -1`, `move_to 18`, `draw 1 -1 0`.

`menu_settings_scales_script.txt`: `settings menu_settings_scales_setup.cfg`, then `update_items`, `first_visible`, `draw 1 -1 0`, `move 1`, `cycles 0`, `right`, `left`, `left`, `enter`, `draw 0 -1 0`.

`menu_settings_file_level_script.txt`: `settings menu_settings_file_level_setup.cfg`, then `update_items`, `first_visible`, `draw 1 -1 0`, `draw 0 -1 0`, `move_to_id 14`, `right`, `draw 0 -1 0`.

`menu_settings_interact_script.txt`: `settings default`, then:

```
update_items
first_visible
cycles 0
right
right
right
right
left
move 1
right
enter
move_to_id 11
enter
move_to_id 10
right
enter
move_to_id 9
enter
left
move_to_id 17
enter
cycles 3
move_to_id 6
right
cycles 5
right
draw 0 -1 0
```

The setups (C++-schema TOML, `[settings]` only; missing keys keep the defaults, as the 4½c sidecars already rely on):

```toml
# menu_settings_gametag_setup.cfg
[settings]
version = 6
gameMode = 1
timeToLose = 3599
```

```toml
# menu_settings_holdazone_setup.cfg
[settings]
version = 6
gameMode = 2
zoneTimeout = 45
```

```toml
# menu_settings_scales_setup.cfg
[settings]
version = 6
gameMode = 3
lives = 1
```

```toml
# menu_settings_file_level_setup.cfg
[settings]
version = 6
blood = 0
levelFile = 'Levels/water_stage.lev'
loadingTime = 9999
map = false
maxBonuses = 99
namesOnBonuses = true
randomLevel = false
regenerateLevel = true
```

(The `# …` first line is for this plan only; the files start at `[settings]`.) Check each key's spelling against `scenario/src/settings_toml.rs` before relying on it. If C++ `FromToml` rejects a partial file (the dumper fails with the error), write the full default setup with the overrides instead: `scenario::settings_toml`'s writer over `Settings { .. }` in a scratch example. Record that in the done-report.

- [ ] **Step 2: Write `gen_menu_golden.sh` and generate**

Create `rust/oracle-tests/gen_menu_golden.sh` (then `chmod +x`):

```bash
#!/usr/bin/env bash
# Regenerates golden/menu_<s>.txt — the Step-4½d G1 menu-widget gate (design §6.1):
# oracle_dump_menu runs the REAL C++ Menu / behaviors / SettingsMenu over every committed
# golden/menu_<s>_script.txt (settings scripts read their _setup.cfg with the REAL
# Settings::FromToml). The scripts are hand-written and never touched here. Needs the full C++
# build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run in the lightweight
# rust.yml CI. Override PRESET for other platforms (e.g. linux-x64). cd's to ROOT, so any cwd
# works. MENU_PPM_DIR=<dir> also writes every draw as <dir>/<script>/menu_NNNN.ppm (eyeballing
# only; never committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_menu
n=0
for scn in rust/oracle-tests/golden/menu_*_script.txt; do
  c="$(basename "$scn" _script.txt)"
  out="rust/oracle-tests/golden/${c}.txt"
  ppm=()
  if [ -n "${MENU_PPM_DIR:-}" ]; then
    mkdir -p "$MENU_PPM_DIR/$c"
    ppm=(--ppm-dir "$MENU_PPM_DIR/$c")
  fi
  "build/$PRESET/Release/oracle_dump_menu" "$scn" "$out" "${ppm[@]}"
  ops=$(awk '!/^#/ && NF && $1 !~ /^(menu|settings|item|space|bind)$/' "$scn" | wc -l)
  # C++-SIDE GATE: one 14-field line per op, in order; a 16-hex hash exactly on draws; a push
  # only on enter.
  awk -v c="$c" -v ops="$ops" '
    BEGIN { k = 0 }
    /^#/ { next }
    {
      if (NF != 14) { printf "FAIL %s: %d fields: %s\n", c, NF, $0; exit 1 }
      if ($1 != k) { printf "FAIL %s: op %s out of order (want %d)\n", c, $1, k; exit 1 }
      if (($2 == "draw") != ($14 ~ /^[0-9a-f]{16}$/)) { printf "FAIL %s: hash vs op: %s\n", c, $0; exit 1 }
      if ($13 != "0" && $2 != "enter") { printf "FAIL %s: a push on %s\n", c, $2; exit 1 }
      k++
    }
    END {
      if (k != ops) { printf "FAIL %s: %d lines for %d ops\n", c, k, ops; exit 1 }
      printf "  gate %s: %d ops\n", c, k
    }
  ' "$out"
  echo "wrote $out"
  n=$((n + 1))
done
test "$n" -eq 15 || { echo "FAIL: $n menu scripts (want 15)"; exit 1; }
```

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_menu_golden.sh`
Expected: 15 `gate … ops` lines and 15 `wrote` lines; exit 0.

Read three goldens by hand against the C++:
- `menu_nav_wrap.txt`: the four `move 1` lines give `sel` 1, 5, 0, 1 and the three `move -1` lines 0, 5, 1. The first `draw` has `bottom 0`.
- `menu_integer.txt`: `right` at `cycles 1` leaves `bound` `5,…`. `enter` on COUNT has `push 1`, and on NOENTRY `push 0` with `sounds 27`.
- `menu_settings_killemall.txt`: its `update_items` line has `vis 15`.

Run it a second time and check the output is byte-identical: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden`, after a first `git add` of the goldens, must show no `M`. This proves the `sleep_ms` margins hold.

- [ ] **Step 3: Write the failing Rust gate**

`rust/oracle-tests/Cargo.toml`: add `ui = { path = "../ui" }` to `[dev-dependencies]`.

Create `rust/oracle-tests/tests/menu_common/mod.rs`:

```rust
//! Step 4½d G1 — the menu-widget scripts replayed through `ui::menu` (design §6.1): the Rust half
//! of `oracle_dump_menu` (grammar and line format: plan Task 4). The clock for type-to-search is
//! synthetic: it starts at 0 and advances by each `sleep_ms` (C++ uses SDL_GetTicks with the same
//! sleeps; the allowed sleep values keep the 1500 ms test on the same side).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use render::bitmap::{Bitmap, Pal32};
use render::font::Font;
use render::hash::hash_frame;
use render::palette::pack_pal32;
use scenario::settings::Settings;
use scenario::settings_toml::settings_from_toml;
use ui::keys::TypedKey;
use ui::menu::behavior::Integer;
use ui::menu::{Behavior, Enter, Menu, MenuCx, MenuItem, MenuModel};
use ui::shell::settings_menu::{SettingsModel, settings_menu};
use ui::text::{GAME_MODES, ONOFF, UiTc};

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

/// The 15 G1 scripts (`golden/menu_<s>_script.txt`).
pub const SCRIPTS: [&str; 15] = [
    "nav_wrap", "page_scroll", "visibility", "scrollbar_positions", "integer", "time", "bool_enum",
    "values_draw", "search", "settings_killemall", "settings_gametag", "settings_holdazone",
    "settings_scales", "settings_file_level", "settings_interact",
];

#[derive(Clone, Debug, Default)]
pub struct Bind {
    kind: String,
    i: i32,
    u: u32,
    b: bool,
    min: i32,
    max: i32,
    step: i32,
    interval: i32,
    div: i32,
    pct: bool,
    entry: bool,
    frames: bool,
    broken: bool,
    arr: String,
}

impl Bind {
    fn value(&self) -> String {
        match self.kind.as_str() {
            "bool" => if self.b { "1" } else { "0" }.to_string(),
            "enum" | "array" => self.u.to_string(),
            _ => self.i.to_string(),
        }
    }
}

/// A widget menu's model: the dumper's `ScriptMenu::GetItemBehavior`.
pub struct Binds(pub BTreeMap<i32, Bind>);

impl MenuModel for Binds {
    fn behavior(&mut self, id: i32) -> Behavior<'_> {
        let Some(b) = self.0.get_mut(&id) else {
            return Behavior::Plain;
        };
        match b.kind.as_str() {
            "integer" => {
                let mut r = Integer::new(&mut b.i, b.min, b.max, b.step, b.pct);
                r.scroll_interval = b.interval;
                r.display_div = b.div;
                r.allow_entry = b.entry;
                Behavior::Integer(r)
            }
            "time" => Behavior::time(&mut b.i, b.min, b.max, b.step, b.frames),
            "bool" => Behavior::Bool(&mut b.b),
            "enum" => Behavior::Enum { v: &mut b.u, min: b.min as u32, max: b.max as u32, broken: b.broken },
            _ => Behavior::ArrayEnum {
                v: &mut b.u,
                arr: if b.arr == "gamemodes" { &GAME_MODES } else { &ONOFF },
                broken: b.broken,
            },
        }
    }
}

pub fn font() -> Font {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/font.tga")).unwrap();
    Font::load(&assets::sprite::Tga::load(&bytes).unwrap())
}

/// `Renderer::LoadPalette` (`renderer.cpp:14-19`): the TC palette (small.tga's, as the loader).
pub fn tc_pal() -> Pal32 {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).unwrap();
    pack_pal32(&assets::sprite::Tga::load(&bytes).unwrap().palette)
}

/// The rest of `line` after `n` whitespace-separated tokens (an item's label).
fn rest_after(line: &str, n: usize) -> String {
    let mut s = line.trim_start();
    for _ in 0..n {
        s = s.trim_start();
        s = &s[s.find(char::is_whitespace).unwrap_or(s.len())..];
    }
    let label = s.trim();
    if label == "\"\"" { String::new() } else { label.to_string() }
}

fn typed(tok: &str) -> TypedKey {
    match tok {
        "SPACE" => TypedKey::Sym(32),
        "TAB" => TypedKey::Tab,
        "MINUS" => TypedKey::Sym(b'-' as u32),
        "UP" => TypedKey::Sym(0x4000_0052),
        t if t.len() == 1 => TypedKey::Sym(t.as_bytes()[0] as u32),
        t => panic!("unknown key token {t}"),
    }
}

fn token(s: &str) -> String {
    if s.is_empty() { "-".into() } else { s.replace(' ', "_") }
}

fn line(n: usize, op: &str, m: &Menu, bound: &str, sounds: &mut Vec<i32>, ret: &str, push: usize, hash: &str) -> String {
    let shown: String = m.items.iter().map(|i| if i.visible { '1' } else { '0' }).collect();
    let vals: Vec<String> = m.items.iter().map(|i| if i.has_value { token(&i.value) } else { ".".into() }).collect();
    let snd: Vec<String> = sounds.drain(..).map(|s| s.to_string()).collect();
    format!(
        "{n} {op} {} {} {} {} {} {} {} {} {} {ret} {push} {hash}",
        m.selection(),
        m.top_item,
        m.bottom_item,
        m.visible_item_count,
        if shown.is_empty() { "-".into() } else { shown },
        token(&m.search.prefix),
        if vals.is_empty() { "-".into() } else { vals.join("|") },
        if bound.is_empty() { "-" } else { bound },
        if snd.is_empty() { "-".into() } else { snd.join(",") },
    )
}

/// One op on `m` with `model`: (ret, push, hash, drawn bitmap).
#[allow(clippy::too_many_arguments)]
fn op<M: MenuModel>(
    m: &mut Menu, model: &mut M, name: &str, args: &[&str], cycles: &mut u32, now: &mut u64,
    tc: &UiTc, sounds: &mut Vec<i32>, font: &Font, pal: &Pal32,
) -> (String, usize, String, Option<Bitmap>) {
    let int = |k: usize| args[k].parse::<i32>().unwrap();
    let mut cx = MenuCx { menu_cycles: *cycles, hooks: tc.hooks, sounds };
    let (mut ret, mut push, mut hash, mut bmp) = ("-".to_string(), 0, "-".to_string(), None);
    match name {
        "move" => m.movement(int(0)),
        "page" => m.movement_page(int(0)),
        "scroll" => m.scroll(int(0)),
        "set_height" => m.set_height(int(0)),
        "visible" => m.set_visibility(int(0), int(1) != 0),
        "move_to" => m.move_to(int(0)),
        "move_to_id" => m.move_to_id(int(0)),
        "first_visible" => m.move_to_first_visible(),
        "cycles" => *cycles = args[0].parse().unwrap(),
        "left" | "right" => {
            let dir = if name == "left" { -1 } else { 1 };
            ret = if m.on_left_right(model, dir, &mut cx) { "1" } else { "0" }.into();
        }
        "enter" => match m.on_enter(model, &mut cx) {
            Enter::Result(r) => ret = r.to_string(),
            Enter::EditValue(_) => (ret, push) = ("-1".into(), 1),
        },
        "keys" | "keys_contains" => {
            let keys: Vec<TypedKey> = args.iter().map(|t| typed(t)).collect();
            m.on_keys(&keys, *now, name == "keys_contains");
        }
        "sleep_ms" => *now += args[0].parse::<u64>().unwrap(),
        "update_items" => m.update_items(model),
        "draw" => {
            let mut b = Bitmap::new(320, 200);
            b.fill(0, pal);
            m.draw(model, &mut b, pal, font, int(0) != 0, int(1), int(2) != 0);
            hash = format!("{:016x}", hash_frame(&b, 33));
            bmp = Some(b);
        }
        other => panic!("bad op {other}"),
    }
    (ret, push, hash, bmp)
}

/// Replay `golden/menu_<name>_script.txt`: the expected-format lines + every drawn bitmap.
pub fn replay(name: &str) -> (Vec<String>, Vec<(usize, Bitmap)>) {
    let text = std::fs::read_to_string(format!("{GOLDEN}/menu_{name}_script.txt")).unwrap();
    let tc = UiTc::load(Path::new(TC_ROOT));
    let (font, pal) = (font(), tc_pal());
    let (mut menu, mut binds, mut settings) = (None::<Menu>, Binds(BTreeMap::new()), None::<Settings>);
    let (mut cycles, mut now, mut sounds) = (0u32, 0u64, Vec::new());
    let (mut lines, mut shots) = (Vec::new(), Vec::new());
    for raw in text.lines() {
        let toks: Vec<&str> = raw.split_whitespace().collect();
        let Some(&name_) = toks.first() else { continue };
        if name_.starts_with('#') {
            continue;
        }
        let args = &toks[1..];
        match name_ {
            "menu" => {
                let v: Vec<i32> = args.iter().map(|a| a.parse().unwrap()).collect();
                let mut m = Menu::new(v[0], v[1], v[4] != 0);
                m.height = v[2];
                m.value_offset_x = v[3];
                menu = Some(m);
                continue;
            }
            "settings" => {
                settings = Some(if args[0] == "default" {
                    Settings::default()
                } else {
                    settings_from_toml(&std::fs::read_to_string(format!("{GOLDEN}/{}", args[0])).unwrap()).unwrap()
                });
                menu = Some(settings_menu());
                continue;
            }
            "space" => {
                menu.as_mut().unwrap().add_item(MenuItem::space());
                continue;
            }
            "item" => {
                let v: Vec<i32> = args[..5].iter().map(|a| a.parse().unwrap()).collect();
                let mut it = MenuItem::new(v[1] as u8, v[2] as u8, &rest_after(raw, 6), v[0]);
                it.visible = v[3] != 0;
                it.selectable = v[4] != 0;
                menu.as_mut().unwrap().add_item(it);
                continue;
            }
            "bind" => {
                let id: i32 = args[0].parse().unwrap();
                let n = |k: usize| args[k].parse::<i32>().unwrap();
                let mut b = Bind { kind: args[1].to_string(), step: 1, interval: 5, div: 1, entry: true, ..Bind::default() };
                match args[1] {
                    "integer" => {
                        (b.i, b.min, b.max, b.step) = (n(2), n(3), n(4), n(5));
                        (b.pct, b.interval, b.div, b.entry) = (n(6) != 0, n(7), n(8), n(9) != 0);
                    }
                    "time" => (b.i, b.min, b.max, b.step, b.frames) = (n(2), n(3), n(4), n(5), n(6) != 0),
                    "bool" => b.b = n(2) != 0,
                    "enum" => (b.u, b.min, b.max, b.broken) = (n(2) as u32, n(3), n(4), n(5) != 0),
                    "array" => (b.u, b.arr, b.broken) = (n(2) as u32, args[3].to_string(), n(4) != 0),
                    k => panic!("bad bind kind {k}"),
                }
                assert!(binds.0.insert(id, b).is_none(), "duplicate bind {id}");
                continue;
            }
            _ => {}
        }
        let m = menu.as_mut().expect("menu or settings comes first");
        let n = lines.len();
        let (ret, push, hash, bmp) = match settings.as_mut() {
            Some(s) => {
                let mut model = SettingsModel { settings: s, tc: &tc, setup_name: "liero" };
                op(m, &mut model, name_, args, &mut cycles, &mut now, &tc, &mut sounds, &font, &pal)
            }
            None => op(m, &mut binds, name_, args, &mut cycles, &mut now, &tc, &mut sounds, &font, &pal),
        };
        let bound: Vec<String> = binds.0.values().map(Bind::value).collect();
        lines.push(line(n, name_, m, &bound.join(","), &mut sounds, &ret, push, &hash));
        if let Some(b) = bmp {
            shots.push((n, b));
        }
    }
    (lines, shots)
}

/// The data lines of `golden/menu_<name>.txt`.
pub fn golden(name: &str) -> Vec<String> {
    std::fs::read_to_string(format!("{GOLDEN}/menu_{name}.txt"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}
```

Create `rust/oracle-tests/tests/menu_widget_golden.rs`:

```rust
//! Step 4½d G1 — the menu-widget gate (design §6.1, done-when 1): every G1 script replayed
//! through `ui::menu` matches the REAL C++ `Menu` / behaviors / `SettingsMenu`
//! (`oracle_dump_menu`, `gen_menu_golden.sh`) line for line. `MENU_RUST_PPM_DIR=<dir>` writes the
//! Rust draws of a failing script as `<dir>/<script>/menu_NNNN.ppm` (the dumper's --ppm-dir
//! layout).

mod menu_common;

use menu_common as mc;

fn check(name: &str, want: &[String]) {
    let (got, shots) = mc::replay(name);
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        if g != w {
            if let Ok(dir) = std::env::var("MENU_RUST_PPM_DIR") {
                let dir = std::path::Path::new(&dir).join(format!("menu_{name}"));
                std::fs::create_dir_all(&dir).unwrap();
                for (n, bmp) in &shots {
                    let mut data = format!("P6\n{} {}\n255\n", bmp.w, bmp.h).into_bytes();
                    for p in &bmp.pixels {
                        data.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
                    }
                    std::fs::write(dir.join(format!("menu_{n:04}.ppm")), data).unwrap();
                }
            }
            panic!("{name}: G1 line {i} differs\n  C++:  {w}\n  Rust: {g}");
        }
    }
    assert_eq!(got.len(), want.len(), "{name}: G1 line count");
}

#[test]
fn every_g1_script_matches_the_cpp_menu_line_for_line() {
    for name in mc::SCRIPTS {
        check(name, &mc::golden(name));
    }
}

#[test]
fn the_corpus_is_exactly_the_15_scripts() {
    let mut on_disk: Vec<String> = std::fs::read_dir(mc::GOLDEN)
        .unwrap()
        .filter_map(|e| {
            let f = e.unwrap().file_name().to_string_lossy().into_owned();
            Some(f.strip_prefix("menu_")?.strip_suffix("_script.txt")?.to_string())
        })
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = mc::SCRIPTS.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(on_disk, want);
}

#[test]
#[should_panic(expected = "G1 line")]
fn the_gate_sees_a_one_field_change() {
    let mut want = mc::golden("nav_wrap");
    want[3].push_str(" x");
    check("nav_wrap", &want);
}
```

- [ ] **Step 4: RED, then GREEN**

Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test menu_widget_golden 2>&1 | tail -15`
Expected on the first run: either PASS, or a precise `G1 line i differs` naming C++ and Rust. A difference is a Rust porting bug in T2/T3. Fix the Rust against the C++ source, and never touch a golden. Re-run until all 3 tests pass. Set `MENU_RUST_PPM_DIR` and `MENU_PPM_DIR` (the gen script) to the scratchpad to compare the two draws of a failing line by eye.

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game 2>&1 | grep -cE 'test result: FAILED'` — Expected: `0`.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden | grep -v '^??'` — Expected: no output (only new, untracked files).

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/oracle-tests/Cargo.toml rust/Cargo.lock rust/oracle-tests/gen_menu_golden.sh rust/oracle-tests/golden/menu_* rust/oracle-tests/tests/menu_common rust/oracle-tests/tests/menu_widget_golden.rs
git -C /home/user/openliero commit -m "oracle(4.5d): G1 — 15 menu-widget scripts, the C++ goldens, ui::menu line for line" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 6: Move the 4½c live modules from `game` to `ui::shell`, with no behavior change  [Sonnet]

**Files:**
- Move (`git mv`): `rust/game/src/{match_flow,new_game,selection,viewport_step}.rs` → `rust/ui/src/shell/`
- Create: `rust/ui/src/shell/loadout.rs` (the body of `apply_weapons`, moved from `game/src/web_params.rs:112-138`)
- Modify: `rust/ui/src/shell/mod.rs` (the new `pub mod`s + `HudFlags`), `rust/ui/src/keys.rs` (`ReleaseLatch` + its two tests, moved from `game/src/input.rs:370-402`, `:913-953`)
- Modify: `rust/game/Cargo.toml` (`ui = { path = "../ui" }`), `rust/game/src/lib.rs`, `rust/game/src/input.rs`, `rust/game/src/hud_mode.rs`, `rust/game/src/web_params.rs`

**Interfaces:**
- Produces: `ui::shell::{match_flow, new_game, selection, viewport_step, loadout::apply_weapons, HudFlags}` and `ui::keys::ReleaseLatch`. Every old `game::…` path keeps compiling through re-exports:
  - `game::{match_flow, new_game, selection, viewport_step}`;
  - `game::input::ReleaseLatch`;
  - `game::hud_mode::HudFlags`;
  - `game::web_params::apply_weapons`.
- Consumes: nothing new.

Why:
- The shell (T7) must drive the whole live loop Bevy-free, and `oracle-tests`/`shot` cannot depend on `game` (design §2, amended LD 2).
- `Match` needs `tick_viewports`, `HudFlags` and `apply_weapons` as well as the three modules the design names (plan-time fact 14).
- Design §10 asks for the moves "with no behavior change in their own commit". This task changes no logic, and every moved test must still pass in its new home.

- [ ] **Step 1: Move and re-export**

```
git -C /home/user/openliero mv rust/game/src/match_flow.rs rust/ui/src/shell/match_flow.rs
git -C /home/user/openliero mv rust/game/src/new_game.rs rust/ui/src/shell/new_game.rs
git -C /home/user/openliero mv rust/game/src/selection.rs rust/ui/src/shell/selection.rs
git -C /home/user/openliero mv rust/game/src/viewport_step.rs rust/ui/src/shell/viewport_step.rs
```

`rust/ui/src/shell/mod.rs`: extend the module doc with ` T6 moved the 4½c live modules here from \`game\` (\`new_game\`, \`selection\`, \`match_flow\`, \`viewport_step\`, \`loadout\`), unchanged; \`game\` re-exports them.`, and add:

```rust
pub mod loadout;
pub mod match_flow;
pub mod new_game;
pub mod selection;
pub mod viewport_step;

use render::frame::Scene;

/// The two HUD switches of a `render::frame::Scene` (moved from `game::hud_mode`, Step 4½a-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudFlags {
    pub draw_hud: bool,
    pub map: bool,
}

impl HudFlags {
    /// Set `scene.draw_hud` and `scene.map`.
    pub fn apply(self, scene: &mut Scene<'_>) {
        scene.draw_hud = self.draw_hud;
        scene.map = self.map;
    }
}
```

Create `rust/ui/src/shell/loadout.rs`: the module doc `//! The preview's \`?weapons=\` loadout (moved from \`game::web_params\`, Step 4½d): NEW GAME re-applies it to every fresh tick-0 state.`, then `use sim::state::{NUM_WEAPONS, SimState, WormWeapon};`, then the `apply_weapons` fn and its doc comment verbatim from `game/src/web_params.rs:112-138`.

In `rust/game/src/web_params.rs`, delete that fn and add `pub use ui::shell::loadout::apply_weapons;` after the `use` line. Drop `WormWeapon` and `NUM_WEAPONS` from its `use sim::state::…` only if they become unused (`NUM_WEAPONS` is still used by `parse`). Its test `apply_weapons_sets_both_worms_and_reports_unknown_names` stays in `game` and calls the re-export.

`rust/ui/src/keys.rs`: add `use sim::state::ControlState;`. Paste `ReleaseLatch` with its doc comment and `impl` verbatim from `game/src/input.rs`, with `N_WORMS` replaced by the literal `2` in both array types. Paste its two tests (`an_unarmed_latch_passes_everything`, `a_latched_key_does_nothing_until_released_then_a_repress_passes`) and their `cs` helper into `keys.rs`'s `mod tests`. In `game/src/input.rs`, delete them and add `pub use ui::keys::ReleaseLatch;` after the `use` lines. Keep the section comment `// ---- Step 4½c: the release latch` out of `input.rs`.

`rust/game/src/hud_mode.rs`: delete `HudFlags` and its `impl`, add `pub use ui::shell::HudFlags;` after `use crate::input::Mode;`, and delete the now-unused `use render::frame::Scene;`. Its tests keep using `crate::new_game::NewGame`, which still resolves (the re-export).

`rust/game/src/lib.rs`: replace

```rust
pub mod match_flow;
pub mod new_game;
pub mod selection;
pub mod touch;
pub mod viewport_step;
pub mod web_params;
```

with

```rust
pub mod touch;
pub mod web_params;

/// Step 4½d: moved to the Bevy-free `ui` crate (the shell drives them headlessly); the paths stay.
pub use ui::shell::{match_flow, new_game, selection, viewport_step};
```

and add one sentence to the crate doc: `Since Step 4½d the Bevy-free cores \`match_flow\`, \`new_game\`, \`selection\` and \`viewport_step\` live in \`ui::shell\` (re-exported here), next to the shell that drives them.`

`rust/game/Cargo.toml`: add `ui       = { path = "../ui" }` after the `scenario` line.

Inside the moved files, fix only paths:
- `ui/src/shell/new_game.rs:156` and `:322`: `use crate::selection::{Selection, new_game_config};` → `use crate::shell::selection::{Selection, new_game_config};`.
- `:349`: `crate::viewport_step::tick_viewports` → `crate::shell::viewport_step::tick_viewports`.
- `ui/src/shell/selection.rs:217-220`: the fixture path `"/scenarios/default_match.txt"` → `"/../game/scenarios/default_match.txt"` (plan-time fact 15).
- Doc comments that say "`game::selection`", "`main.rs`" or "the `lib.rs` rule" stay true enough. Only replace "Bevy-free so it is headlessly testable (the `lib.rs` rule)" with "Bevy-free (moved to `ui::shell` in Step 4½d)" in the four moved module docs.

- [ ] **Step 2: GREEN** (this task has no new behavior, so there is no RED; the moved tests are the check)

Run: `rustfmt --edition 2024 /home/user/openliero/rust/ui/src/shell/*.rs /home/user/openliero/rust/ui/src/keys.rs`
Run: `cd /home/user/openliero/rust && cargo test -p ui 2>&1 | grep -E '^test result'` — Expected: PASS. The count grows by the moved tests:
- `match_flow` 7;
- `new_game` 9 + 1;
- `selection` 7;
- `viewport_step`'s own unit tests, if any;
- `keys` 2.

Run: `cd /home/user/openliero/rust && cargo test -p game 2>&1 | grep -E '^test result|FAILED'` — Expected: PASS. `game`'s lib test count drops by exactly the moved tests; `tests/{passthrough,record_regression,round_trip,viewport_stepping}.rs` are unchanged and green.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown 2>&1 | tail -2` — Expected: builds.
Run: `cd /home/user/openliero/rust && cargo tree -p ui -e normal | grep -c bevy` — Expected: `0`.
Run: `git -C /home/user/openliero diff --stat HEAD -M -- rust/game/src rust/ui/src/shell | tail -3` — Expected: the four files show as renames (`=>`) with small deltas.

- [ ] **Step 3: Commit**

```
git -C /home/user/openliero add -A rust/game rust/ui rust/Cargo.lock
git -C /home/user/openliero commit -m "ui(4.5d): move new_game/selection/match_flow/viewport_step/ReleaseLatch/HudFlags/apply_weapons from game to ui::shell (re-exported, no behavior change)" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 7: `ui::shell` — `MatchFlow` Esc/focus/tail, the shared frozen screen, `LevelSlot`/`SeedSource`, `Match`, `ScreenStack`, `MainMenuState`, the router, `Shell::frame`  [Opus]

**Files:**
- Modify: `rust/ui/src/shell/match_flow.rs` (`esc`, `focus`, `check_game_over`, `tail`, `going_to_menu`, `running`; remove `weapsel_frame`; tests)
- Modify: `rust/ui/src/shell/selection.rs` (`cached_background` + `focused`; `render` with the shared frozen screen and the caller's `menu_cycles`; `focus`/`unfocus`; tests)
- Create: `rust/ui/src/shell/level_slot.rs`, `rust/ui/src/shell/playing.rs`, `rust/ui/src/shell/stack.rs`
- Modify: `rust/ui/src/shell/main_menu.rs` (`MainMenuState`, `MenuCtx`), `rust/ui/src/shell/mod.rs` (`Shell` and its types; flow tests)
- Modify: `rust/game/src/main.rs` (only the two call sites the API changes break: `flow.weapsel_frame()` becomes `flow.tail();` and `sel.render(..)` gets the new arguments; behavior unchanged until T11)

**Interfaces:**
- Produces (used by T9–T11):
  - `ui::shell::{Shell, ShellInput, KeyEvent, FrameOut, Present, Phase, Route, MenuWorld, SURFACE_W, SURFACE_H}`;
  - `Shell::{boot, boot_playing, frame, surface, frozen, fade, menu_cycles, pal32, top_char, main_selection, menu_fading, main_menu, main_menu_mut, settings, settings_mut, current, phase}`;
  - `level_slot::{LevelSlot, LevelProvenance, SeedSource}`;
  - `playing::{Match, StartOptions, boot_state, draw_boot}`;
  - `stack::{Screen, ScreenStack, AfterUpdate}`;
  - `main_menu::{MainMenuState, MenuCtx}`;
  - `MatchFlow::{esc, focus, check_game_over, tail, going_to_menu, running}`;
  - `Selection::{render(surface, frozen, state, scene, texts, level_file, menu_cycles) -> Pal32, focus, unfocus}`.
- Consumes:
  - T3's menus and `UiTc`; T6's moved modules;
  - T0's `menu_palette`;
  - `render::palette::{build_palette, set_worm_colour}`, `render::frame::draw`;
  - `scenario::build::{new_match, build_match, enter_game}`.

Why: design §3.1–§3.8 and §4.7–§4.12. This is the C++ frame loop in Rust: one `Shell::frame` per `Gfx::RunOneFrame`, the `StateStack` semantics, `MainMenuState`, the router and `LocalController`'s flow.

The C++ call graph is kept everywhere a later frame can observe it:
- `menu_cycles` counts every frame (finding 8);
- the fade-out order (finding 14);
- the pop frame presents nothing and the black flip comes from `Enter` (finding 7);
- Esc keeps the sim running for 32 frames, key-up included (finding 9);
- RESUME refocuses the same match (finding 10);
- one shared frozen screen (finding 11);
- the menu background is the one draw of the pop frame's tick (finding 12);
- game over pops to the menu (finding 13, until 4½g);
- the level-reuse test (finding 16).

The firewall is in the types: `MainMenuState` gets a `MenuCtx` with no `SimState`, only `Match::process` and the router see `&mut SimState`, and the router only replaces it (LD 3). T10 gates it line for line against the real `Gfx`. If a count below differs when you run it, re-derive it from the cited C++ lines. Never tune Rust to a test, because the C++ gate is the arbiter.

- [ ] **Step 1: `MatchFlow` — Esc, focus and the shared tail (RED, then GREEN)**

In `rust/ui/src/shell/match_flow.rs`, replace the tests `weapon_selection_fades_in_from_zero_then_the_game_starts_at_33` and `entering_the_game_mid_fade_jumps_to_33` with versions that call `f.tail()` where they called `f.weapsel_frame()` (assert `FlowStep::Continue` each time), and append:

```rust
    #[test]
    fn esc_fades_out_over_31_presented_frames_then_finishes() {
        // localController.cpp:82-85 (fade 31, going_to_menu) + :185-194 (the tail).
        let mut f = MatchFlow::new();
        f.esc();
        assert!(f.going_to_menu());
        for want in (0..=30).rev() {
            assert_eq!(f.tail(), FlowStep::Continue);
            assert_eq!(f.fade_value(), want);
        }
        assert_eq!(f.tail(), FlowStep::Finished, "the 32nd call: the pop frame");
    }

    #[test]
    fn esc_is_ignored_while_already_leaving() {
        let s = state();
        let mut f = MatchFlow::new();
        let mut dead = s.clone();
        dead.worms[1].lives = 0;
        f.after_frame(&dead);
        let fade = f.fade_value();
        f.esc();
        assert_eq!(f.fade_value(), fade, "`!going_to_menu` guard (localController.cpp:82)");
    }

    #[test]
    fn done_during_an_esc_fade_restarts_it_at_33() {
        // ChangeState(kStateGame) sets fade 33 even while going_to_menu (:284-287, design §3.7).
        let mut f = MatchFlow::with_weapon_selection();
        f.esc();
        f.tail();
        f.enter_game();
        assert_eq!(f.tail(), FlowStep::Continue);
        assert_eq!((f.fade_value(), f.going_to_menu()), (32, true));
    }

    #[test]
    fn focus_fades_back_in_from_zero_unless_the_game_ended() {
        let mut f = MatchFlow::new();
        f.esc();
        for _ in 0..5 {
            f.tail();
        }
        f.focus();
        assert_eq!((f.going_to_menu(), f.fade_value()), (false, 0), "localController.cpp:118-119");
        assert_eq!(f.tail(), FlowStep::Continue);
        assert_eq!(f.fade_value(), 1);
        let mut dead = state();
        dead.worms[0].lives = 0;
        let mut g = MatchFlow::new();
        g.after_frame(&dead);
        assert!(!g.running(), "Running(): not after game over (:304)");
        g.focus();
        assert_eq!((g.going_to_menu(), g.fade_value()), (true, 0), ":101-105");
        assert_eq!(g.tail(), FlowStep::Finished);
    }
```

`SimState` is `Clone`: `state().clone()` works, and `sim_core::rng::Rand: Clone` landed in 4½c.

Run: `cd /home/user/openliero/rust && cargo test -p ui match_flow 2>&1 | tail -5` — Expected: FAIL to compile, with `no method named 'esc'`, `'tail'`, `'focus'`, `'going_to_menu'` and `'running'`.

Implement. Replace `weapsel_frame` and `after_frame` with:

```rust
    /// `LocalController::OnKey(kDkEscape, _)` (`localController.cpp:82-85`): a key-down OR a
    /// key-up of Esc (finding 9) starts the 32-frame return to the menu, unless already leaving.
    pub fn esc(&mut self) {
        if !self.going_to_menu {
            self.fade_value = 31;
            self.going_to_menu = true;
        }
    }

    /// The flow half of `LocalController::Focus` (`localController.cpp:100-120`): after game over
    /// straight back to the menu; otherwise fade in from 0.
    pub fn focus(&mut self) {
        self.going_to_menu = self.phase == MatchPhase::GameEnded;
        self.fade_value = 0;
    }

    /// After a match tick: `IsGameOver` → `ChangeState(kStateGameEnded)` (`:177-179`, `:277-282`).
    pub fn check_game_over(&mut self, state: &SimState) {
        debug_assert_ne!(
            self.phase,
            MatchPhase::WeaponSelection,
            "after_frame runs after a match tick, not during weapon selection"
        );
        if self.phase == MatchPhase::Game && sim::game_over::is_game_over(state) {
            self.phase = MatchPhase::GameEnded;
            if !self.going_to_menu {
                self.fade_value = POST_MORTEM_FRAMES;
                self.going_to_menu = true;
            }
        }
    }

    /// The tail of every `LocalController::Process` (`localController.cpp:185-199`), in every
    /// phase: leaving counts down and finishes at 0; otherwise the fade counts up to 33.
    pub fn tail(&mut self) -> FlowStep {
        if self.going_to_menu {
            if self.fade_value > 0 {
                self.fade_value -= 1;
                FlowStep::Continue
            } else {
                FlowStep::Finished
            }
        } else {
            if self.fade_value < FADE_IN_MAX {
                self.fade_value += 1;
            }
            FlowStep::Continue
        }
    }

    /// One match tick's bookkeeping: [`check_game_over`](Self::check_game_over) then
    /// [`tail`](Self::tail) (the C++ order).
    pub fn after_frame(&mut self, state: &SimState) -> FlowStep {
        self.check_game_over(state);
        self.tail()
    }

    pub fn going_to_menu(&self) -> bool {
        self.going_to_menu
    }

    /// `LocalController::Running` (`:304`) for a started controller: false only after game over
    /// (the shell has no `kStateInitial` controller: the boot has no `Match`).
    pub fn running(&self) -> bool {
        self.phase != MatchPhase::GameEnded
    }
```

Update the module doc's last sentences: "Seams: 4½d the Esc fade (`OnKey`, `:82-85`)…" becomes "Since 4½d: the Esc fade (`esc`, `OnKey` `:82-85`), `focus` (RESUME) and one shared `tail` for every phase; 4½g routes `Finished` to the stats screen."

In `rust/game/src/main.rs:808-813`, replace `flow.weapsel_frame();` with `flow.tail();`. The 4½c `--live <scenario>` path has no Esc, so `tail` counts up exactly as `weapsel_frame` did.

Run: `cd /home/user/openliero/rust && cargo test -p ui match_flow` — Expected: PASS (7 old, with 2 rewritten, plus 4 new).

- [ ] **Step 2: `Selection` — the shared frozen screen and the caller's `menu_cycles`**

In `rust/ui/src/shell/selection.rs`, replace the test `render_freezes_once_and_counts_menu_cycles_after_the_draw` with:

```rust
    #[test]
    fn render_builds_the_shared_frozen_screen_once_and_reads_the_callers_menu_cycles() {
        // weapsel.cpp:160-186 + finding 11: the pixels live in the SHELL's frozen screen.
        let scenario::Loaded { mut state, scene, .. } = default_match();
        let mut sel = Selection::new(live_config(&state, false));
        sel.begin(&mut state).unwrap();
        let s = scene.as_scene(0, false);
        let (mut surface, mut frozen) = (Bitmap::new(320, 200), Bitmap::new(320, 200));
        let lf = "Levels/render_stage.lev";
        let pal = sel.render(&mut surface, &mut frozen, &state, &s, &scene.weapsel_texts, lf, 5);
        assert_eq!(pal, render::weapsel::weapsel_palette(&scene.origpal, 5), "UpdateWeapselPalette");
        let (first_surface, first_frozen) = (surface.clone(), frozen.clone());
        sel.render(&mut surface, &mut frozen, &state, &s, &scene.weapsel_texts, lf, 6);
        assert_eq!(frozen, first_frozen, "cached_background: built once");
        assert_ne!(surface, first_surface, "the selected item's colour 168 rotates");
        sel.unfocus();
        sel.render(&mut surface, &mut frozen, &state, &s, &scene.weapsel_texts, lf, 7);
        assert_eq!(surface.pixels, frozen.pixels, "unfocused: the frozen copy only (:184-186)");
        frozen.pixels.fill(0xFF12_3456); // the main menu's Enter overwrote the shared screen
        sel.focus();
        sel.render(&mut surface, &mut frozen, &state, &s, &scene.weapsel_texts, lf, 8);
        assert_eq!(surface.get_pixel(0, 199), 0xFF12_3456, "RESUME redraws over the menu's frozen screen");
    }
```

Run: `cd /home/user/openliero/rust && cargo test -p ui selection 2>&1 | tail -5` — Expected: FAIL to compile (the `render` arity, `unfocus`, `focus`).

Implement:
- Delete `WeapselScreen`.
- `Selection` gets `cached_background: bool, focused: bool` instead of `screen`.
- `new` sets both false/true.
- `begin` sets `self.cached_background = false; self.focused = true;` (a fresh C++ `WeaponSelection`, `weapsel.hpp:27-28`).
- Delete `menu_cycles()`.
- Replace `render`:

```rust
    /// `WeaponSelection::DrawNormalViewports` (`weapsel.cpp:160-209`) into `surface`: the weapsel
    /// palette at the shell's `menu_cycles`, then, on the first draw of this selection, the frozen
    /// background INTO THE SHELL'S shared frozen screen (`gfx.frozen_screen`, which the main
    /// menu's `Enter` also writes — finding 11), then the frozen copy and, while focused, the
    /// header, names and menus. Returns the palette the draw leaves behind.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        surface: &mut Bitmap,
        frozen: &mut Bitmap,
        state: &SimState,
        scene: &Scene,
        texts: &WeapselTexts,
        level_file: &str,
        menu_cycles: u32,
    ) -> Pal32 {
        let ws = self.active.as_ref().expect("render needs an active selection");
        let pal = screen::weapsel_palette(scene.origpal, menu_cycles);
        if !self.cached_background {
            let label = screen::level_label(texts, level_file);
            *frozen = screen::build_frozen(state, scene, &label, menu_cycles);
            self.cached_background = true;
        }
        if self.focused {
            let names = [self.names[0].as_str(), self.names[1].as_str()];
            screen::draw_screen(surface, frozen, &pal, scene.font, texts, ws, &state.weapons, names);
        } else {
            surface.pixels.copy_from_slice(&frozen.pixels);
            surface.clip = Rect::new(0, 0, surface.w, surface.h);
        }
        pal
    }

    /// `WeaponSelection::Focus` / `Unfocus` (`weapsel.cpp:363-365`).
    pub fn focus(&mut self) {
        self.focused = true;
    }

    pub fn unfocus(&mut self) {
        self.focused = false;
    }
```

Add `use render::bitmap::{Pal32, Rect};` and update the module doc: the frozen screen and `menu_cycles` now come from the shell (design §4.7, §4.11).

In `rust/game/src/main.rs`, the 4½c `--live <scenario>` path is the only other caller. Add `frozen: Bitmap` and `weapsel_cycles: u32` to `Demo`, initialised to `Bitmap::new(SURFACE_W as i32, SURFACE_H as i32)` and `0`. In `render_and_upload`, call `sel.render(&mut demo.surface, &mut demo.frozen, sim, &scene, &demo.scene.weapsel_texts, &demo.level_file, demo.weapsel_cycles);` and then `demo.weapsel_cycles = demo.weapsel_cycles.wrapping_add(1);`. In `restart_match`'s selection branch, set `demo.weapsel_cycles = 0;`. This keeps 4½c's per-phase count on that path; T11 retires it for the default match.

Run: `cd /home/user/openliero/rust && cargo test -p ui selection && cargo test -p game` — Expected: PASS.

- [ ] **Step 3: `LevelSlot` and `SeedSource`**

Create `rust/ui/src/shell/level_slot.rs`:

```rust
//! The router's level and seeds (design §4.11, finding 16; LD 6). `LevelSlot` is C++ `Level` plus
//! its `old_*` provenance (`level.cpp:421-424`); NEW GAME reuses it — as the last match left it,
//! craters and all (`SwapLevel(*old_level)`) — unless `regenerate_level` is set or a level
//! setting changed (`gfx.cpp:1512-1516`). `SeedSource` splits the boot seed (the level behind
//! the menu) from the per-NEW-GAME match seed. Supersedes 4½c's `NewGame` (plan-time fact 17).

use std::collections::VecDeque;
use std::path::Path;

use assets::level::LevelData;
use scenario::settings::Settings;
use sim::state::LevelSim;

use super::new_game::generate_level;

/// `Level::old_random_level / old_level_file / old_random_map_width / old_random_map_height`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelProvenance {
    pub random_level: bool,
    pub level_file: String,
    pub random_map_width: i32,
    pub random_map_height: i32,
}

impl LevelProvenance {
    pub fn of(s: &Settings) -> LevelProvenance {
        LevelProvenance {
            random_level: s.random_level,
            level_file: s.level_file.clone(),
            random_map_width: s.random_map_width,
            random_map_height: s.random_map_height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelSlot {
    pub level: LevelData,
    pub provenance: LevelProvenance,
}

impl LevelSlot {
    /// `Level::GenerateFromSettings(common, settings, rand)` with the level `Rand` seeded from
    /// `seed` (4½b; `new_game::generate_level`), recording the provenance (`level.cpp:421-424`) —
    /// also when a file level failed to load and fell back to random (finding 16).
    pub fn generate(tc_root: &Path, settings: &Settings, seed: u32) -> LevelSlot {
        LevelSlot {
            level: generate_level(tc_root, settings, seed),
            provenance: LevelProvenance::of(settings),
        }
    }

    /// The reuse test (`gfx.cpp:1512-1516`): the width and height count even for a file level.
    pub fn reusable(&self, settings: &Settings) -> bool {
        !settings.regenerate_level && self.provenance == LevelProvenance::of(settings)
    }

    /// `SwapLevel(*old_level)`: keep the level as the last match left it.
    pub fn take_played(&mut self, played: &LevelSim) {
        debug_assert_eq!((played.width, played.height), (self.level.width, self.level.height));
        self.level.material_id.clone_from(&played.material_id);
    }
}

/// Where the seeds come from. C++ seeds `gfx.rand` (levels) from the clock at start-up
/// (`gameEntry.cpp:23`) and every `Game` (the sim) from `time(nullptr)` (`game.cpp:42`); Rust
/// takes one boot seed and one seed per NEW GAME (the sim seed and, when the level is
/// generated, the level seed): `Fixed` (`?seed=`), `Fresh` (the caller's clock value), or
/// `Scripted` (the oracle harness; the C++ dumper reseeds at the same points, T8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedSource {
    Fixed(u32),
    Fresh,
    Scripted { boot: u32, matches: VecDeque<u32> },
}

impl SeedSource {
    pub fn boot(&self, fresh: u32) -> u32 {
        match self {
            SeedSource::Fixed(s) => *s,
            SeedSource::Fresh => fresh,
            SeedSource::Scripted { boot, .. } => *boot,
        }
    }

    pub fn next_match(&mut self, fresh: u32) -> u32 {
        match self {
            SeedSource::Fixed(s) => *s,
            SeedSource::Fresh => fresh,
            SeedSource::Scripted { matches, .. } => {
                matches.pop_front().expect("a NEW GAME with no scripted match seed left")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scenario::paths::TC_ROOT;

    fn tc() -> &'static Path {
        Path::new(TC_ROOT)
    }

    #[test]
    fn a_slot_records_the_settings_it_was_generated_from() {
        let s = Settings::default();
        let slot = LevelSlot::generate(tc(), &s, 7);
        assert_eq!(slot.level, generate_level(tc(), &s, 7));
        assert!(slot.reusable(&s));
    }

    #[test]
    fn each_of_the_five_fields_forces_a_new_level() {
        let s = Settings::default();
        let slot = LevelSlot::generate(tc(), &s, 7);
        for changed in [
            Settings { regenerate_level: true, ..s.clone() },
            Settings { random_level: false, ..s.clone() },
            Settings { level_file: "Levels/water_stage.lev".into(), ..s.clone() },
            Settings { random_map_width: 512, ..s.clone() },
            Settings { random_map_height: 352, ..s.clone() },
        ] {
            assert!(!slot.reusable(&changed), "gfx.cpp:1512-1516");
        }
    }

    #[test]
    fn a_file_level_is_reloaded_when_the_map_size_changes() {
        // Finding 16: the width/height compare applies even to a file level.
        let s = Settings { random_level: false, level_file: "Levels/water_stage.lev".into(), ..Settings::default() };
        let slot = LevelSlot::generate(tc(), &s, 1);
        assert!(slot.reusable(&s));
        assert!(!slot.reusable(&Settings { random_map_width: 600, ..s }));
    }

    #[test]
    fn take_played_keeps_the_craters() {
        let s = Settings::default();
        let mut slot = LevelSlot::generate(tc(), &s, 7);
        let mut played = LevelSim {
            width: slot.level.width,
            height: slot.level.height,
            material_id: slot.level.material_id.clone(),
            material_flags: Vec::new(),
        };
        played.material_id[1234] ^= 0xff;
        slot.take_played(&played);
        assert_eq!(slot.level.material_id, played.material_id);
    }

    #[test]
    fn seed_sources() {
        let mut fixed = SeedSource::Fixed(7);
        assert_eq!((fixed.boot(1), fixed.next_match(2), fixed.next_match(3)), (7, 7, 7), "?seed=7 is 4½c's first match");
        let mut fresh = SeedSource::Fresh;
        assert_eq!((fresh.boot(1), fresh.next_match(2)), (1, 2));
        let mut s = SeedSource::Scripted { boot: 5, matches: VecDeque::from([8, 9]) };
        assert_eq!((s.boot(0), s.next_match(0), s.next_match(0)), (5, 8, 9));
    }
}
```

If `LevelSim`'s `material_flags` has another type, build `played` from `scenario::build::new_match(..).state.level` instead, as 4½c's `the_next_new_game_reuses_the_played_level_with_a_new_seed` does.

Add `pub mod level_slot;` to `shell/mod.rs`. Run: `cd /home/user/openliero/rust && cargo test -p ui level_slot` — Expected: PASS (5).

- [ ] **Step 4: `Match` (the `Playing` screen) and the boot draw**

Create `rust/ui/src/shell/playing.rs`:

```rust
//! `Match`, the `LocalController` analog behind the `Playing` screen (design §4.12): the 4½c
//! live loop (`game/src/main.rs` `tick_and_render`, moved), with the Esc fade, `Focus`/`Unfocus`
//! and one shared fade tail. `process` is `LocalController::Process` (`localController.cpp:
//! 122-200`); `draw` is `LocalController::Draw` for the main window (`:203-212`), which also sets
//! the play renderer's fade. Plus the boot: the never-focused `LocalController` whose game the
//! first menu is drawn over (`gfx.cpp:1439-1465`).

use std::path::Path;

use assets::level::LevelData;
use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::palette::{build_palette, set_worm_colour};
use render::viewport::Viewport;
use scenario::build::{build_match, enter_game, new_match};
use scenario::settings::{MatchConfig, Settings};
use scenario::{Loaded, SceneData};
use sim::state::{ControlState, SimState};

use super::HudFlags;
use super::loadout::apply_weapons;
use super::match_flow::{FlowStep, MatchFlow};
use super::selection::{Selection, new_game_config};
use super::viewport_step::tick_viewports;
use crate::keys::ReleaseLatch;

/// How a match starts (design §7.5): `skip_selection` (`?weapons=`), the preview loadout,
/// and the touch-only rule (4½c Q8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartOptions {
    pub skip_selection: bool,
    pub loadout: Vec<String>,
    pub touch_only: bool,
}

/// `Game::Focus` → `UpdateSettings` (`game.cpp:475-488`): both worms' ramps into `origpal`.
fn focus_palette(scene: &mut SceneData, settings: &Settings) {
    for i in 0..2 {
        set_worm_colour(&mut scene.origpal, i, settings.worm_settings[i].rgb);
    }
}

/// The boot controller (`gfx.cpp:1441-1450`): `LocalController(common, settings)` on the boot
/// level, focused for its palette only. Its sim RNG is never used (C++ seeds it from the clock).
pub fn boot_state(tc_root: &Path, settings: &Settings, level: &LevelData, seed: u32) -> Loaded {
    let cfg = MatchConfig { settings: settings.clone(), seed };
    let mut loaded = new_match(tc_root, &cfg, level).expect("the settings build a match");
    focus_palette(&mut loaded.scene, settings);
    loaded
}

/// `controller->Draw(play_renderer)` of the unstarted boot game (`gfx.cpp:1452-1454`): a
/// `frame::draw` with fresh viewports (C++ never processed them), the HUD per `settings.map`.
/// Returns the palette it leaves behind (the copyright bar's, finding 12).
pub fn draw_boot(surface: &mut Bitmap, state: &SimState, scene: &SceneData, settings: &Settings) -> Pal32 {
    let mut s = scene.as_scene(state.screen_flash, settings.shadow);
    HudFlags { draw_hud: true, map: settings.map }.apply(&mut s);
    render::frame::draw(surface, state, &mut Viewport::player_layout(), &s);
    build_palette(s.origpal, s.color_anim, state.cycles, s.screen_flash)
}

pub struct Match {
    flow: MatchFlow,
    /// `None` when selection was skipped; kept (inactive) after DONE for the picks.
    selection: Option<Selection>,
    viewports: [Viewport; 2],
    scene: SceneData,
    latch: ReleaseLatch,
    hud: HudFlags,
    cfg: MatchConfig,
    /// `sound_hook[SoundBegin]` (`game.cpp:500-503`).
    begin: i32,
}

impl Match {
    /// NEW GAME's controller on `level` (`gfx.cpp:1507-1523`) and `GamePlayState::Enter` →
    /// `LocalController::Focus` (`gamePlayState.cpp:14`, `localController.cpp:100-120`):
    /// `kStateInitial` → weapon selection (the `WeaponSelection` constructor draws `state.rand`),
    /// `Game::Focus`'s worm ramps, fade 0. `held` arms the release latch: key-downs made in the
    /// menu never reached the controller (design §4.11). The skip route (`?weapons=`, Rust only)
    /// starts at match tick 0 like 4½c.
    pub fn start(
        tc_root: &Path,
        settings: &Settings,
        level: &LevelData,
        seed: u32,
        opts: &StartOptions,
        begin: i32,
        held: &[ControlState; 2],
    ) -> (Match, SimState) {
        let cfg = MatchConfig { settings: settings.clone(), seed };
        let built = if opts.skip_selection {
            build_match(tc_root, &cfg, level)
        } else {
            new_match(tc_root, &cfg, level)
        };
        let Loaded { mut state, viewports, mut scene } = built.expect("the settings build a match");
        apply_weapons(&mut state, &opts.loadout);
        focus_palette(&mut scene, settings);
        let (flow, selection) = if opts.skip_selection {
            (MatchFlow::new(), None)
        } else {
            let mut sel = Selection::new(new_game_config(settings, opts.touch_only));
            sel.begin(&mut state).expect("the settings select over the TC");
            (MatchFlow::with_weapon_selection(), Some(sel))
        };
        let mut latch = ReleaseLatch::default();
        latch.arm(held);
        let hud = HudFlags { draw_hud: true, map: settings.map };
        let m = Match { flow, selection, viewports, scene, latch, hud, cfg, begin };
        (m, state)
    }

    pub fn in_selection(&self) -> bool {
        self.selection.as_ref().is_some_and(Selection::is_active)
    }

    pub fn running(&self) -> bool {
        self.flow.running()
    }

    pub fn fade(&self) -> i32 {
        self.flow.fade_value()
    }

    pub fn flow(&self) -> &MatchFlow {
        &self.flow
    }

    /// `renderer.Origpal()` after this match's `Game::Focus`.
    pub fn origpal(&self) -> &Palette {
        &self.scene.origpal
    }

    /// `OnKey(kDkEscape, _)`.
    pub fn esc(&mut self) {
        self.flow.esc();
    }

    /// RESUME (`gfx.cpp:1525-1530` → `LocalController::Focus`): the flow, a running selection's
    /// `Focus`, and the latch over the keys held at the boundary.
    pub fn focus(&mut self, held: &[ControlState; 2]) {
        self.flow.focus();
        if let Some(sel) = self.selection.as_mut().filter(|s| s.is_active()) {
            sel.focus();
        }
        self.latch.arm(held);
    }

    /// `LocalController::Unfocus` (`localController.cpp:90-97`).
    pub fn unfocus(&mut self) {
        if let Some(sel) = self.selection.as_mut().filter(|s| s.is_active()) {
            sel.unfocus();
        }
    }

    /// `LocalController::Process` (`localController.cpp:122-200`) on this frame's sampled words:
    /// the latch, then the selection step (the frame the last player readies runs
    /// `ChangeState(kStateGame)`: Finalize, lives, `StartGame`'s blood pool and `SoundBegin`,
    /// fade 33) or one match tick (`tick_viewports` + game over), then the shared tail. Returns
    /// (keep running, the sim ticked).
    pub fn process(&mut self, sim: &mut SimState, sampled: [ControlState; 2], sounds: &mut Vec<i32>) -> (bool, bool) {
        let mut inputs = sampled;
        self.latch.apply(&mut inputs);
        let mut ticked = false;
        if self.in_selection() {
            let sel = self.selection.as_mut().expect("in selection");
            if sel.step(sim, &inputs, sounds) {
                enter_game(sim, &self.cfg);
                if self.begin >= 0 {
                    sounds.push(self.begin);
                }
                self.flow.enter_game();
                self.latch.arm(&sampled);
            }
        } else {
            tick_viewports(&mut self.viewports, sim, &inputs);
            self.flow.check_game_over(sim);
            ticked = true;
        }
        (self.flow.tail() == FlowStep::Continue, ticked)
    }

    /// `GamePlayState::Draw` for the main window (`gamePlayState.cpp:98-101` →
    /// `LocalController::Draw`, `localController.cpp:203-212`): the selection screen or the game.
    /// Returns the palette the draw leaves behind; the caller takes `fade()` as the play
    /// renderer's fade (`:211`).
    pub fn draw(&mut self, surface: &mut Bitmap, frozen: &mut Bitmap, sim: &SimState, menu_cycles: u32) -> Pal32 {
        let mut scene = self.scene.as_scene(sim.screen_flash, self.cfg.settings.shadow);
        self.hud.apply(&mut scene);
        match self.selection.as_mut().filter(|s| s.is_active()) {
            Some(sel) => sel.render(
                surface,
                frozen,
                sim,
                &scene,
                &self.scene.weapsel_texts,
                &self.cfg.settings.level_file,
                menu_cycles,
            ),
            None => {
                render::frame::draw(surface, sim, &mut self.viewports, &scene);
                build_palette(scene.origpal, scene.color_anim, sim.cycles, scene.screen_flash)
            }
        }
    }

    /// C++ `WeaponSelection` edits the shared `WormSettings::weapons` in place (4½c finding 4): a
    /// running selection's picks — or the finalized ones — become the settings' picks for the
    /// next NEW GAME.
    pub fn write_back_picks(mut self, settings: &mut Settings) {
        if let Some(sel) = self.selection.as_mut() {
            sel.abandon();
            for i in 0..2 {
                settings.worm_settings[i].weapons = sel.config().players[i].weapons;
            }
        }
    }
}
```

`Selection::step` already writes the finalized picks back into its own config (4½c), and `abandon` covers a running selection.

Add `pub mod playing;` to `shell/mod.rs`. The unit coverage is Step 7's flow tests.

- [ ] **Step 5: `ScreenStack`**

Create `rust/ui/src/shell/stack.rs`:

```rust
//! C++ `StateStack` (`state.hpp:47-139`; design §4.9). A `Screen` enum rather than trait objects:
//! every dispatch is an exhaustive `match`, so a screen 4½e/4½f/4½g adds cannot be forgotten.
//! The stack is plain data; `Shell` runs `enter` before `push` (`Push` calls `Enter`,
//! `state.hpp:50-54`). No 4½d screen has a `Leave`.

use super::main_menu::MainMenuState;

pub enum Screen {
    MainMenu(MainMenuState),
    /// `GamePlayState`; its controller is `Shell::current`, as C++'s is `gfx.controller`.
    Playing,
}

impl Screen {
    /// `AppState::IsOverlay` (`state.hpp:34`): no 4½d screen is one (4½e's `InputStringState` is).
    pub fn is_overlay(&self) -> bool {
        false
    }

    /// `AppState::WantsMenuFlip` (`state.hpp:38`, `gamePlayState.hpp:119`).
    pub fn wants_menu_flip(&self) -> bool {
        matches!(self, Screen::MainMenu(_))
    }
}

/// What `StateStack::Update` did after the top's `Update` (`state.hpp:92-112`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterUpdate {
    Running,
    /// A scheduled replacement was pushed; the caller runs its `enter`.
    Replaced,
    /// The top returned false and was popped; `empty` = `Update()` returned false.
    Popped { empty: bool },
}

#[derive(Default)]
pub struct ScreenStack {
    stack: Vec<Screen>,
    pending_replace: Option<Screen>,
}

impl ScreenStack {
    pub fn push(&mut self, s: Screen) {
        self.stack.push(s);
    }

    pub fn pop(&mut self) -> Option<Screen> {
        self.stack.pop()
    }

    /// `ScheduleReplaceTop` (`state.hpp:75`): applied after the top's update.
    pub fn schedule_replace_top(&mut self, s: Screen) {
        self.pending_replace = Some(s);
    }

    /// `StateStack::Update` after the top ran (`state.hpp:92-112`).
    pub fn finish_update(&mut self, keep_running: bool) -> AfterUpdate {
        if let Some(s) = self.pending_replace.take() {
            self.pop();
            self.push(s);
            return AfterUpdate::Replaced;
        }
        if !keep_running {
            self.pop();
            return AfterUpdate::Popped { empty: self.stack.is_empty() };
        }
        AfterUpdate::Running
    }

    pub fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn screens(&self) -> &[Screen] {
        &self.stack
    }

    /// `StateStack::Draw`'s bottom (`state.hpp:117-125`): walk down past overlays.
    pub fn draw_from_by(&self, is_overlay: impl Fn(&Screen) -> bool) -> usize {
        let mut bottom = self.stack.len().saturating_sub(1);
        while bottom > 0 && is_overlay(&self.stack[bottom]) {
            bottom -= 1;
        }
        bottom
    }

    pub fn draw_from(&self) -> usize {
        self.draw_from_by(Screen::is_overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> Screen {
        Screen::MainMenu(MainMenuState::new())
    }

    #[test]
    fn update_pops_on_false_and_reports_an_empty_stack() {
        let mut s = ScreenStack::default();
        s.push(menu());
        assert_eq!(s.finish_update(true), AfterUpdate::Running);
        assert_eq!(s.finish_update(false), AfterUpdate::Popped { empty: true });
        s.push(Screen::Playing);
        s.push(menu());
        assert_eq!(s.finish_update(false), AfterUpdate::Popped { empty: false });
        assert!(matches!(s.top(), Some(Screen::Playing)));
    }

    #[test]
    fn a_scheduled_replacement_wins_over_the_pop() {
        let mut s = ScreenStack::default();
        s.push(Screen::Playing);
        s.schedule_replace_top(menu());
        assert_eq!(s.finish_update(false), AfterUpdate::Replaced, "state.hpp:101-105 runs first");
        assert_eq!(s.len(), 1);
        assert!(matches!(s.top(), Some(Screen::MainMenu(_))));
    }

    #[test]
    fn draw_walks_down_past_overlays() {
        let mut s = ScreenStack::default();
        s.push(menu());
        s.push(Screen::Playing);
        s.push(Screen::Playing);
        let playing_is_overlay = |x: &Screen| matches!(x, Screen::Playing);
        assert_eq!(s.draw_from_by(playing_is_overlay), 0);
        assert_eq!(s.draw_from(), 2, "no 4½d screen is an overlay");
        assert!(!Screen::Playing.wants_menu_flip() && menu().wants_menu_flip());
    }
}
```

Add `pub mod stack;` to `shell/mod.rs`. (`MainMenuState::new` comes in Step 6, so run these tests after Step 6.)

- [ ] **Step 6: `MainMenuState` and `MenuCtx`**

Add to `rust/ui/src/shell/main_menu.rs` (above its tests):

```rust
use render::bitmap::Rect;
use render::font::Font;

use super::MenuWorld;
use super::settings_menu::SettingsModel;
use crate::keys::{
    DK_DOWN, DK_ESCAPE, DK_F1, DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9, DK_KP_ENTER,
    DK_LEFT, DK_PGDN, DK_PGUP, DK_RETURN, DK_RIGHT, DK_UP, K_DOWN, K_FIRE, K_JUMP, K_LEFT,
    K_RIGHT, K_UP, reset_left_right,
};
use crate::menu::MenuCx;

/// What `MainMenuState` may touch: the menu world (C++ `Gfx` members), the font, whether the
/// current controller `Running()`, and this frame's sound log. No `SimState` (LD 3, §4.9).
pub struct MenuCtx<'a> {
    pub w: &'a mut MenuWorld,
    pub font: &'a Font,
    pub running: bool,
    pub sounds: &'a mut Vec<i32>,
}

fn play(sounds: &mut Vec<i32>, hook: i32) {
    if hook >= 0 {
        sounds.push(hook);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuPhase {
    Active,
    FadingOut,
}

/// C++ `MainMenuState` (`mainMenuState.hpp`, `mainMenuState.cpp:95-626`). In 4½d the main menu
/// always has focus (`cur_menu == &main_menu`); 4½e adds the settings focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MainMenuState {
    phase: MenuPhase,
    selected: i32,
    start_item_id: i32,
}

impl Default for MainMenuState {
    fn default() -> Self {
        MainMenuState::new()
    }
}

impl MainMenuState {
    pub fn new() -> MainMenuState {
        MainMenuState { phase: MenuPhase::Active, selected: -1, start_item_id: 0 }
    }

    /// `Selection()`: the item chosen this menu, or -1.
    pub fn selection(&self) -> i32 {
        self.selected
    }

    pub fn is_fading_out(&self) -> bool {
        self.phase == MenuPhase::FadingOut
    }

    /// `Enter` (`mainMenuState.cpp:95-148`) after its `Flip` at fade 0 — the caller presents that
    /// black frame. The copyright bar goes through whatever palette the last draw left in
    /// `w.pal32` (finding 12); `Process()` (`:105`) has nothing left to poll.
    pub fn enter(&mut self, cx: &mut MenuCtx) {
        let w = &mut *cx.w;
        w.fade = 0;
        w.surface.clip = Rect::new(0, 0, w.surface.w, w.surface.h);
        w.surface.fill_rect(0, 151, 160, 7, 0, &w.pal32);
        cx.font.draw_string(&mut w.surface, &w.pal32, &w.tc.copyright2, 2, 152, 19, 1);
        let m = &mut w.main_menu;
        if cx.running {
            m.set_visibility(MA_RESUME_GAME, true);
            m.item_from_id_mut(MA_RESUME_GAME).expect("RESUME").string = "RESUME GAME (F1)".into();
            m.item_from_id_mut(MA_NEW_GAME).expect("NEW GAME").string = "NEW GAME".into();
            self.start_item_id = MA_RESUME_GAME;
        } else {
            m.set_visibility(MA_RESUME_GAME, false);
            m.item_from_id_mut(MA_NEW_GAME).expect("NEW GAME").string = "NEW GAME (F1)".into();
            self.start_item_id = MA_NEW_GAME;
        }
        m.item_from_id_mut(MA_TC).expect("TC").string = format!("TC ({})", w.settings.tc);
        m.move_to_first_visible();
        w.settings_menu.move_to_first_visible();
        w.settings_menu.update_items(&mut SettingsModel {
            settings: &mut w.settings,
            tc: &w.tc,
            setup_name: &w.setup_name,
        });
        w.fade = 0;
        w.frozen.pixels.copy_from_slice(&w.surface.pixels);
        w.menu_cycles = 0;
        self.selected = -1;
        self.phase = MenuPhase::Active;
    }

    /// `Update` (`mainMenuState.cpp:152-612`): the fade-out, else the keys in C++ source order.
    pub fn update(&mut self, cx: &mut MenuCtx) -> bool {
        let MenuCtx { w, sounds, .. } = cx;
        if self.phase == MenuPhase::FadingOut {
            if w.fade > 0 {
                w.fade -= 1;
                return true;
            }
            return false;
        }
        let hooks = w.tc.hooks;
        let ws = &w.settings.worm_settings;
        // :171-179 (Esc, or any keyboard player's jump): the cursor to QUIT TO OS.
        if w.keys.test_once(DK_ESCAPE) || w.keys.test_control_once(ws, K_JUMP) {
            w.main_menu.move_to_id(MA_QUIT);
        }
        // :181-192: Up plays MenuMoveDown, Down plays MenuMoveUp. `||` short-circuits (finding 15).
        if w.keys.test_once(DK_UP) || w.keys.test_control_once(ws, K_UP) {
            play(sounds, hooks.move_down);
            w.main_menu.movement(-1);
        }
        if w.keys.test_once(DK_DOWN) || w.keys.test_control_once(ws, K_DOWN) {
            play(sounds, hooks.move_up);
            w.main_menu.movement(1);
        }
        // :194-270.
        if w.keys.test_once(DK_RETURN)
            || w.keys.test_once(DK_KP_ENTER)
            || w.keys.test_control_once(ws, K_FIRE)
        {
            play(sounds, hooks.select);
            match w.main_menu.selected_id() {
                id @ (MA_RESUME_GAME | MA_NEW_GAME | MA_QUIT) => self.selected = id, // `default:`
                // Their second MenuSelect (:234, :248, :254); inert until Step 5 (plan fact 2).
                MA_JOIN_GAME | MA_HOST_ONLINE | MA_JOIN_ONLINE => play(sounds, hooks.select),
                // Inert placeholders (§5, Q2): MATCH SETUP (4½e), LEFT/RIGHT PLAYER (4½f), OPTIONS
                // (4½g), NETWORK PLAYER / HOST LAN (Step 5), REPLAYS / TC (deferred).
                _ => {}
            }
        }
        // :432-436.
        if w.keys.test_once(DK_F1) {
            w.main_menu.move_to_id(self.start_item_id);
            self.selected = self.start_item_id;
        }
        // :437-461 and the F8 easter egg (:463): consumed as C++ does, inert in 4½d (§5).
        for k in [DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F9, DK_F8] {
            w.keys.test_once(k);
        }
        let mut mcx = MenuCx { menu_cycles: w.menu_cycles, hooks, sounds };
        // :581-592: held; a behavior returning false releases Left/Right (never, on MainModel).
        if w.keys.test(DK_LEFT) || w.keys.test_control(ws, K_LEFT) {
            if !w.main_menu.on_left_right(&mut MainModel, -1, &mut mcx) {
                reset_left_right(&mut w.keys, ws);
            }
        }
        if w.keys.test(DK_RIGHT) || w.keys.test_control(ws, K_RIGHT) {
            if !w.main_menu.on_left_right(&mut MainModel, 1, &mut mcx) {
                reset_left_right(&mut w.keys, ws);
            }
        }
        // :594-602.
        if w.keys.test_once(DK_PGUP) {
            mcx.play(hooks.move_down);
            w.main_menu.movement_page(-1);
        }
        if w.keys.test_once(DK_PGDN) {
            mcx.play(hooks.move_up);
            w.main_menu.movement_page(1);
        }
        // :604-609: start the fade-out.
        if self.selected >= 0 {
            self.phase = MenuPhase::FadingOut;
            w.fade = 32;
        }
        true
    }

    /// `Draw` (`mainMenuState.cpp:614-626`): `DrawBasicMenu` (`gfx.cpp:1699-1704`), then — the main
    /// menu having focus — the settings menu, disabled. `DrawSpectatorInfo` draws into the
    /// spectator renderer only (finding 2).
    pub fn draw(&self, cx: &mut MenuCtx) {
        let w = &mut *cx.w;
        w.surface.pixels.copy_from_slice(&w.frozen.pixels);
        w.surface.clip = Rect::new(0, 0, w.surface.w, w.surface.h);
        w.main_menu.draw(&MainModel, &mut w.surface, &w.pal32, cx.font, false, -1, true);
        w.settings_menu.draw(&MainModel, &mut w.surface, &w.pal32, cx.font, true, -1, false);
    }
}
```

`reset_left_right(&mut w.keys, ws)` borrows `w.keys` mutably while `ws` borrows `w.settings`. These are disjoint fields of `*w`. If the compiler still objects (for example because `ws` was bound through `w`), clone the three `WormSettings` once at the top of `update` (`let ws = w.settings.worm_settings.clone();`). They are small, and the menu does not change them in 4½d.

- [ ] **Step 7: `Shell` — the frame loop, the router, the boots**

Add to `rust/ui/src/shell/mod.rs` (after the `pub mod` lines and `HudFlags`):

```rust
use std::path::{Path, PathBuf};

use assets::palette::Palette;
use render::bitmap::{Bitmap, Pal32};
use render::menu::menu_palette;
use scenario::SceneData;
use scenario::settings::Settings;
use sim::state::{ControlState, SimState};

use crate::keys::{DK_ESCAPE, KeyLatch, TypedKey};
use crate::menu::Menu;
use crate::text::UiTc;
use level_slot::{LevelSlot, SeedSource};
use main_menu::{MA_NEW_GAME, MA_QUIT, MA_RESUME_GAME, MainMenuState, MenuCtx, main_menu};
use playing::{Match, StartOptions};
use settings_menu::settings_menu;
use stack::{AfterUpdate, Screen, ScreenStack};

/// The play renderer's size (`gfx.cpp:277`).
pub const SURFACE_W: i32 = 320;
pub const SURFACE_H: i32 = 200;

/// One keyboard event of a frame, as `Gfx::ProcessEvent` sees it (`gfx.cpp:595-640`): the DOS
/// scancode (`SDLToDOSKey`), down/up, the OS-repeat flag, and the key's `key_buf` symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub dos: u32,
    pub down: bool,
    pub repeat: bool,
    pub typed: TypedKey,
}

/// One frame's input: its events in order, the sim's sampled control words (the 4½c sampler plus
/// touch — C++ key events reach the worms at the frame boundary, the 4½c equivalence), a fresh
/// seed for `SeedSource::Fresh`, the type-to-search clock, and the Rust-only F5 restart (Q5).
#[derive(Clone, Copy, Debug)]
pub struct ShellInput<'a> {
    pub events: &'a [KeyEvent],
    pub sampled: [ControlState; 2],
    pub fresh_seed: u32,
    pub now_ms: u64,
    pub restart: bool,
}

impl ShellInput<'static> {
    pub fn idle() -> ShellInput<'static> {
        ShellInput { events: &[], sampled: [ControlState::new(); 2], fresh_seed: 0, now_ms: 0, restart: false }
    }
}

/// What a frame shows (design §4.7): the surface at `fade` (`Gfx::Draw`'s `ScaleDraw`), or the
/// black frame of `MainMenuState::Enter`'s `Flip` at fade 0 (finding 7). `None` in `FrameOut` is
/// a pop frame: nothing is presented and the window keeps its image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Present {
    Frame { fade: i32 },
    Black,
}

/// The screen as the page and the tests see it (`window.lieroPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Menu,
    Weapsel,
    Game,
    Quit,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Menu => "menu",
            Phase::Weapsel => "weapsel",
            Phase::Game => "game",
            Phase::Quit => "quit",
        }
    }
}

/// What the router did on a pop frame (`gfx.cpp:1491-1625`), or the F5 restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    NewGame { seed: u32 },
    Resume,
    Menu,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameOut {
    pub present: Option<Present>,
    /// Menu, selection and `StartGame` sounds (sample ids, in call order). The sim's own sounds
    /// stay in `SimState::sound_events` (drain them when `sim_ticked`).
    pub menu_sounds: Vec<i32>,
    pub sim_ticked: bool,
    pub quit: bool,
    /// The screen that ran `Update` this frame (read before the frame).
    pub upd: Phase,
    /// The screen after the frame.
    pub phase: Phase,
    pub routed: Option<Route>,
}

impl FrameOut {
    fn new(upd: Phase) -> FrameOut {
        FrameOut { present: None, menu_sounds: Vec::new(), sim_ticked: false, quit: false, upd, phase: upd, routed: None }
    }
}

/// Everything `MainMenuState` touches — C++ `Gfx` members: the menus, `settings`,
/// `settings_node`'s name, `dos_keys`, the play renderer's `fade_value`, `bmp`, `pal32` and
/// `Origpal()`, `frozen_screen`, `menu_cycles`. No `SimState`.
pub struct MenuWorld {
    pub main_menu: Menu,
    pub settings_menu: Menu,
    pub settings: Settings,
    pub tc: UiTc,
    pub setup_name: String,
    pub keys: KeyLatch,
    pub fade: i32,
    pub menu_cycles: u32,
    pub surface: Bitmap,
    pub frozen: Bitmap,
    pub pal32: Pal32,
    pub origpal: Palette,
}

/// C++ `Gfx` driving `StateStack` (design §4.1, §4.7): one [`Shell::frame`] per
/// `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`).
pub struct Shell {
    tc_root: PathBuf,
    world: MenuWorld,
    /// The boot game's scene: the font every screen draws with, and the boot palette.
    boot_scene: SceneData,
    stack: ScreenStack,
    /// `gfx.controller` once a NEW GAME made one (the boot controller is never focused).
    current: Option<Match>,
    level: LevelSlot,
    seeds: SeedSource,
    options: StartOptions,
}

impl Shell {
    fn new(tc_root: &Path, settings: Settings, seeds: SeedSource, fresh: u32, options: StartOptions) -> (Shell, SimState) {
        let tc = UiTc::load(tc_root);
        let boot_seed = seeds.boot(fresh);
        let level = LevelSlot::generate(tc_root, &settings, boot_seed);
        let boot = playing::boot_state(tc_root, &settings, &level.level, boot_seed);
        let world = MenuWorld {
            main_menu: main_menu(),
            settings_menu: settings_menu(),
            origpal: boot.scene.origpal.clone(),
            settings,
            tc,
            setup_name: "liero".into(),
            keys: KeyLatch::default(),
            fade: 0,
            menu_cycles: 0,
            surface: Bitmap::new(SURFACE_W, SURFACE_H),
            frozen: Bitmap::new(SURFACE_W, SURFACE_H),
            pal32: [0; 256],
        };
        let shell = Shell {
            tc_root: tc_root.to_path_buf(),
            world,
            boot_scene: boot.scene,
            stack: ScreenStack::default(),
            current: None,
            level,
            seeds,
            options,
        };
        (shell, boot.state)
    }

    /// `InitFrameStepping` (`gfx.cpp:1439-1465`): the boot level (the boot seed), the unstarted
    /// boot game drawn once, then the main menu, whose `Enter` presents the black frame. Returns
    /// the shell, the boot state (the `Sim` until the first NEW GAME), and the boot frame.
    pub fn boot(tc_root: &Path, settings: Settings, seeds: SeedSource, fresh: u32, options: StartOptions) -> (Shell, SimState, FrameOut) {
        let (mut shell, state) = Shell::new(tc_root, settings, seeds, fresh, options);
        let mut out = FrameOut::new(Phase::Menu);
        shell.world.pal32 = playing::draw_boot(&mut shell.world.surface, &state, &shell.boot_scene, &shell.world.settings);
        shell.world.fade = 0;
        shell.push_main_menu(&mut out);
        out.phase = shell.phase();
        (shell, state, out)
    }

    /// The skip route (design §7.5, Q4; Rust only): `[Playing]` from the start, a NEW GAME on the
    /// boot level (so `?seed=N` plays 4½c's first match: level and sim from N). Esc still opens
    /// the pause menu.
    pub fn boot_playing(tc_root: &Path, settings: Settings, seeds: SeedSource, fresh: u32, options: StartOptions) -> (Shell, SimState, FrameOut) {
        let (mut shell, mut state) = Shell::new(tc_root, settings, seeds, fresh, options);
        let mut out = FrameOut::new(Phase::Game);
        let input = ShellInput { fresh_seed: fresh, ..ShellInput::idle() };
        let seed = shell.new_game(&mut state, &input);
        shell.stack.push(Screen::Playing);
        out.routed = Some(Route::NewGame { seed });
        out.phase = shell.phase();
        (shell, state, out)
    }

    /// `Gfx::RunOneFrame` (`gfx.cpp:1467-1652`; design §3.1).
    pub fn frame(&mut self, sim: &mut SimState, input: &ShellInput) -> FrameOut {
        let mut out = FrameOut::new(self.phase());
        if self.stack.is_empty() {
            out.quit = true;
            return out;
        }
        if input.restart && matches!(self.stack.top(), Some(Screen::Playing)) {
            let seed = self.new_game(sim, input);
            out.routed = Some(Route::NewGame { seed });
        }
        // :1473-1485 — every event reaches ProcessEvent (dos_keys, key_buf); while Playing a
        // non-repeat key-down or any key-up also reaches LocalController::OnKey, whose only
        // non-worm effect is Esc (finding 9). Worm keys reach the sim as `sampled`.
        self.world.keys.begin_frame();
        let playing = matches!(self.stack.top(), Some(Screen::Playing));
        for ev in input.events {
            if ev.down {
                self.world.keys.key_down(ev.dos, ev.typed);
            } else {
                self.world.keys.key_up(ev.dos);
            }
            if playing && ev.dos == DK_ESCAPE && (!ev.down || !ev.repeat) {
                self.current.as_mut().expect("Playing has a match").esc();
            }
        }
        // :1487-1489, captured BEFORE Update: the menu state is on the stack only while it is the
        // top (finding 7), and C++ clears menuStatePtr_ at its dispatch.
        let (sel, fading) = match self.stack.top() {
            Some(Screen::MainMenu(s)) => (s.selection(), s.is_fading_out()),
            _ => (-1, false),
        };
        let running = self.current.as_ref().is_some_and(Match::running);
        let keep = match self.stack.top_mut().expect("non-empty") {
            Screen::MainMenu(s) => s.update(&mut MenuCtx {
                w: &mut self.world,
                font: &self.boot_scene.font,
                running,
                sounds: &mut out.menu_sounds,
            }),
            Screen::Playing => {
                let m = self.current.as_mut().expect("Playing has a match");
                let (keep, ticked) = m.process(sim, input.sampled, &mut out.menu_sounds);
                out.sim_ticked = ticked;
                keep
            }
        };
        match self.stack.finish_update(keep) {
            AfterUpdate::Popped { empty: true } => {
                self.route(sel, sim, input, &mut out);
                out.phase = self.phase();
                return out;
            }
            AfterUpdate::Replaced => unreachable!("no 4½d screen schedules a replacement"),
            AfterUpdate::Popped { empty: false } | AfterUpdate::Running => {}
        }
        // :1631-1647.
        let menu_flip = self.stack.top().expect("non-empty").wants_menu_flip();
        if menu_flip {
            self.update_menu_palettes(fading);
        }
        self.draw_stack(sim);
        if !menu_flip {
            self.world.menu_cycles = self.world.menu_cycles.wrapping_add(1);
        }
        out.present = Some(Present::Frame { fade: self.world.fade });
        out.phase = self.phase();
        out
    }

    /// The router (`gfx.cpp:1491-1625`), on the frame the top popped and left the stack empty.
    fn route(&mut self, sel: i32, sim: &mut SimState, input: &ShellInput, out: &mut FrameOut) {
        if sel >= 0 {
            match sel {
                MA_QUIT => {
                    // :1496-1498 — no present: the last one was the fade-out's black frame.
                    out.quit = true;
                    out.routed = Some(Route::Quit);
                    return;
                }
                MA_NEW_GAME => {
                    let seed = self.new_game(sim, input);
                    out.routed = Some(Route::NewGame { seed });
                }
                MA_RESUME_GAME => {
                    // :1525-1530 + GamePlayState::Enter → Focus.
                    self.current.as_mut().expect("RESUME is shown only while a match runs").focus(&input.sampled);
                    out.routed = Some(Route::Resume);
                }
                other => unreachable!("main-menu item {other} never selects in 4½d (§5)"),
            }
            self.stack.push(Screen::Playing);
        } else {
            // :1594-1625 — back to the menu: Unfocus, ClearKeys, one draw of the pop frame's
            // tick (finding 12; the only draw it gets), then a new MainMenuState.
            let m = self.current.as_mut().expect("only Playing pops without a selection");
            m.unfocus();
            self.world.keys.clear();
            self.world.pal32 = m.draw(&mut self.world.surface, &mut self.world.frozen, sim, self.world.menu_cycles);
            self.world.fade = m.fade();
            self.push_main_menu(out);
            out.routed = Some(Route::Menu);
        }
    }

    /// NEW GAME (`gfx.cpp:1507-1523`; design §4.11): the old controller's picks, the next seed,
    /// the level (reused as played, or generated from the seed), a new `Match` — already focused,
    /// its `WeaponSelection` constructed — and the router's only write to the sim: replace it.
    fn new_game(&mut self, sim: &mut SimState, input: &ShellInput) -> u32 {
        if let Some(old) = self.current.take() {
            old.write_back_picks(&mut self.world.settings);
        }
        let seed = self.seeds.next_match(input.fresh_seed);
        if self.level.reusable(&self.world.settings) {
            self.level.take_played(&sim.level);
        } else {
            self.level = LevelSlot::generate(&self.tc_root, &self.world.settings, seed);
        }
        let (m, state) = Match::start(
            &self.tc_root,
            &self.world.settings,
            &self.level.level,
            seed,
            &self.options,
            self.world.tc.begin,
            &input.sampled,
        );
        self.world.origpal = m.origpal().clone();
        *sim = state;
        self.current = Some(m);
        seed
    }

    /// `Push(MainMenuState)`: its `Enter` (with the black present) then the push.
    fn push_main_menu(&mut self, out: &mut FrameOut) {
        let mut s = MainMenuState::new();
        out.present = Some(Present::Black);
        let running = self.current.as_ref().is_some_and(Match::running);
        s.enter(&mut MenuCtx { w: &mut self.world, font: &self.boot_scene.font, running, sounds: &mut out.menu_sounds });
        self.stack.push(Screen::MainMenu(s));
    }

    /// `Gfx::UpdateMenuPalettes(quitting)` (`gfx.cpp:978-1005`) for the play renderer.
    fn update_menu_palettes(&mut self, fading: bool) {
        let w = &mut self.world;
        if w.fade < 32 && !fading {
            w.fade += 1;
        }
        w.menu_cycles = w.menu_cycles.wrapping_add(1);
        let rgb = [w.settings.worm_settings[0].rgb, w.settings.worm_settings[1].rgb];
        w.pal32 = menu_palette(&w.origpal, w.menu_cycles, rgb);
    }

    /// `StateStack::Draw` (`state.hpp:114-131`).
    fn draw_stack(&mut self, sim: &SimState) {
        let running = self.current.as_ref().is_some_and(Match::running);
        for i in self.stack.draw_from()..self.stack.len() {
            match &self.stack.screens()[i] {
                Screen::MainMenu(s) => {
                    let mut sounds = Vec::new();
                    s.draw(&mut MenuCtx { w: &mut self.world, font: &self.boot_scene.font, running, sounds: &mut sounds });
                }
                Screen::Playing => {
                    let m = self.current.as_mut().expect("Playing has a match");
                    self.world.pal32 = m.draw(&mut self.world.surface, &mut self.world.frozen, sim, self.world.menu_cycles);
                    self.world.fade = m.fade();
                }
            }
        }
    }

    pub fn phase(&self) -> Phase {
        match self.stack.top() {
            None => Phase::Quit,
            Some(Screen::MainMenu(_)) => Phase::Menu,
            Some(Screen::Playing) => {
                if self.current.as_ref().is_some_and(Match::in_selection) { Phase::Weapsel } else { Phase::Game }
            }
        }
    }

    /// `M` / `G` / `-` (the G2 golden's `<top>`).
    pub fn top_char(&self) -> char {
        match self.stack.top() {
            None => '-',
            Some(Screen::MainMenu(_)) => 'M',
            Some(Screen::Playing) => 'G',
        }
    }

    pub fn surface(&self) -> &Bitmap {
        &self.world.surface
    }

    pub fn frozen(&self) -> &Bitmap {
        &self.world.frozen
    }

    pub fn pal32(&self) -> &Pal32 {
        &self.world.pal32
    }

    /// `play_renderer.fade_value`.
    pub fn fade(&self) -> i32 {
        self.world.fade
    }

    pub fn menu_cycles(&self) -> u32 {
        self.world.menu_cycles
    }

    /// `main_menu.Selection()`.
    pub fn main_selection(&self) -> i32 {
        self.world.main_menu.selection()
    }

    /// Whether the main menu is fading out after a selection (the G2 generator's placeholder check).
    pub fn menu_fading(&self) -> bool {
        matches!(self.stack.top(), Some(Screen::MainMenu(s)) if s.is_fading_out())
    }

    pub fn main_menu(&self) -> &Menu {
        &self.world.main_menu
    }

    /// For tests and 4½e's settings focus.
    pub fn main_menu_mut(&mut self) -> &mut Menu {
        &mut self.world.main_menu
    }

    pub fn settings(&self) -> &Settings {
        &self.world.settings
    }

    /// For tests and 4½e (MATCH SETUP edits the settings).
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.world.settings
    }

    pub fn current(&self) -> Option<&Match> {
        self.current.as_ref()
    }
}
```

In `draw_stack`, `self.stack.screens()[i]` borrows `self.stack` while `self.world` and `self.current` are borrowed mutably. These are disjoint fields and legal, as long as nothing calls a `&mut self` method inside the loop.

- [ ] **Step 8: The headless flow tests (RED first)**

Append to `rust/ui/src/shell/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use scenario::paths::TC_ROOT;

    use super::*;
    use crate::keys::{DK_DOWN, DK_ESCAPE, DK_F1, DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9, DK_PGUP, DK_RETURN, DK_UP};
    use crate::shell::main_menu::MA_TC;
    use crate::shell::new_game::generate_level;

    fn tc() -> &'static Path {
        Path::new(TC_ROOT)
    }

    fn hooks() -> UiTc {
        UiTc::load(tc())
    }

    fn boot() -> (Shell, SimState, FrameOut) {
        let seeds = SeedSource::Scripted { boot: 11, matches: VecDeque::from([21, 22, 23]) };
        Shell::boot(tc(), Settings::default(), seeds, 0, StartOptions::default())
    }

    fn ev(dos: u32, down: bool) -> KeyEvent {
        KeyEvent { dos, down, repeat: false, typed: TypedKey::Sym(0) }
    }

    fn step(sh: &mut Shell, sim: &mut SimState, events: &[KeyEvent], words: [u32; 2]) -> FrameOut {
        let input = ShellInput { events, sampled: words.map(ControlState::unpack), ..ShellInput::idle() };
        sh.frame(sim, &input)
    }

    fn idle(sh: &mut Shell, sim: &mut SimState, n: usize) -> Vec<FrameOut> {
        (0..n).map(|_| step(sh, sim, &[], [0, 0])).collect()
    }

    /// A key-down this frame, its key-up the next; returns the key-down frame.
    fn tap(sh: &mut Shell, sim: &mut SimState, dos: u32) -> FrameOut {
        let out = step(sh, sim, &[ev(dos, true)], [0, 0]);
        step(sh, sim, &[ev(dos, false)], [0, 0]);
        out
    }

    /// Tap `dos`, then run until the router acts; every frame from the tap on.
    fn until_routed(sh: &mut Shell, sim: &mut SimState, dos: u32) -> Vec<FrameOut> {
        let mut outs = vec![step(sh, sim, &[ev(dos, true)], [0, 0]), step(sh, sim, &[ev(dos, false)], [0, 0])];
        while outs.last().unwrap().routed.is_none() {
            outs.push(step(sh, sim, &[], [0, 0]));
            assert!(outs.len() < 300, "never routed");
        }
        outs
    }

    /// Boot → NEW GAME → both players Up + Fire → the match's first tick.
    fn start_match(sh: &mut Shell, sim: &mut SimState) {
        idle(sh, sim, 40);
        until_routed(sh, sim, DK_RETURN);
        step(sh, sim, &[], [1, 1]);
        step(sh, sim, &[], [0, 0]);
        step(sh, sim, &[], [16, 16]);
        step(sh, sim, &[], [0, 0]);
        assert_eq!(sh.phase(), Phase::Game);
    }

    #[test]
    fn boot_presents_one_black_frame_then_the_menu_fades_in() {
        let (mut sh, mut sim, out) = boot();
        assert_eq!((out.present, out.phase), (Some(Present::Black), Phase::Menu), "Enter's Flip (finding 7)");
        assert_eq!((sh.fade(), sh.menu_cycles(), sh.top_char(), sh.main_selection()), (0, 0, 'M', 1));
        let m = sh.main_menu();
        assert!(!m.items[0].visible, "RESUME hidden at boot (Running() is false)");
        assert_eq!(m.items[1].string, "NEW GAME (F1)");
        assert_eq!(m.item_from_id(MA_TC).unwrap().string, "TC (openliero)");
        let outs = idle(&mut sh, &mut sim, 40);
        for (k, o) in outs.iter().enumerate() {
            assert_eq!(o.present, Some(Present::Frame { fade: (k as i32 + 1).min(32) }), "frame {k}");
        }
        assert_eq!(sh.menu_cycles(), 40, "finding 8");
    }

    #[test]
    fn the_boot_background_carries_the_copyright_bar() {
        let (sh, _sim, _) = boot();
        let pal = *sh.pal32();
        let mut text = false;
        for y in 151..158 {
            for x in 0..160 {
                let p = sh.frozen().get_pixel(x, y);
                assert!(p == pal[0] || p == pal[19], "({x},{y}) is the bar (mainMenuState.cpp:107-108)");
                text |= p == pal[19];
            }
        }
        assert!(text);
    }

    #[test]
    fn the_keys_move_the_cursor_with_crossed_sounds() {
        let (mut sh, mut sim, _) = boot();
        let h = hooks().hooks;
        let o = tap(&mut sh, &mut sim, DK_DOWN);
        assert_eq!((sh.main_selection(), o.menu_sounds), (2, vec![h.move_up]), "Down plays MenuMoveUp");
        let o = tap(&mut sh, &mut sim, DK_UP);
        assert_eq!((sh.main_selection(), o.menu_sounds), (1, vec![h.move_down]));
        tap(&mut sh, &mut sim, DK_UP);
        assert_eq!(sh.main_selection(), 14, "wraps past the hidden RESUME to MATCH SETUP");
        tap(&mut sh, &mut sim, DK_ESCAPE);
        assert_eq!(sh.main_selection(), 9, "Esc: QUIT TO OS");
        tap(&mut sh, &mut sim, DK_UP);
        tap(&mut sh, &mut sim, 56);
        assert_eq!(sh.main_selection(), 9, "LAlt is P1 jump");
        tap(&mut sh, &mut sim, DK_DOWN);
        assert_eq!(sh.main_selection(), 11, "over the spacer");
        let o = tap(&mut sh, &mut sim, DK_PGUP);
        assert_eq!((sh.main_selection(), o.menu_sounds), (4, vec![h.move_down]), "visible 10 - 7 = 3 -> HOST ONLINE");
    }

    #[test]
    fn placeholders_play_select_but_never_select_and_f_keys_are_inert() {
        let (mut sh, mut sim, _) = boot();
        let s = hooks().hooks.select;
        for (idx, want) in [(2, 1), (3, 2), (4, 2), (5, 2), (6, 1), (7, 1), (8, 1), (11, 1), (12, 1), (13, 1), (14, 1)] {
            sh.main_menu_mut().move_to(idx);
            let o = tap(&mut sh, &mut sim, DK_RETURN);
            assert_eq!(o.menu_sounds, vec![s; want], "item {idx} (plan-time fact 2)");
        }
        sh.main_menu_mut().move_to(1);
        for dos in [DK_F2, DK_F3, DK_F5, DK_F6, DK_F7, DK_F8, DK_F9] {
            let o = tap(&mut sh, &mut sim, dos);
            assert!(o.menu_sounds.is_empty());
        }
        assert_eq!(sh.main_selection(), 1);
        assert!(idle(&mut sh, &mut sim, 40).iter().all(|o| o.routed.is_none() && o.phase == Phase::Menu));
    }

    #[test]
    fn new_game_fades_out_then_the_pop_frame_routes_into_selection() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        let boot_level = sim.level.material_id.clone();
        let mc = sh.menu_cycles();
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        let got: Vec<Option<Present>> = outs.iter().map(|o| o.present).collect();
        let mut want: Vec<Option<Present>> = (0..=32).rev().map(|f| Some(Present::Frame { fade: f })).collect();
        want.push(None);
        assert_eq!(got, want, "32 on the select frame, 31..0, then a pop frame with no present (finding 14)");
        let pop = outs.last().unwrap();
        assert_eq!((pop.routed, pop.upd, pop.phase), (Some(Route::NewGame { seed: 21 }), Phase::Menu, Phase::Weapsel));
        assert_eq!(sh.menu_cycles(), mc + 33, "the pop frame counts nothing");
        assert_eq!(sim.rand.draws(), 0, "default picks: the constructor draws nothing (T8 intervention 3)");
        assert_eq!(sim.level.material_id, boot_level, "the first NEW GAME reuses the boot level");
    }

    #[test]
    fn selection_inherits_menu_cycles_and_done_plays_begin() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        until_routed(&mut sh, &mut sim, DK_RETURN);
        let mc = sh.menu_cycles();
        let origpal = sh.current().unwrap().origpal().clone();
        let o = step(&mut sh, &mut sim, &[], [1, 1]);
        assert_eq!(*sh.pal32(), render::weapsel::weapsel_palette(&origpal, mc), "finding 8: the menu's count");
        assert_eq!((sh.menu_cycles(), o.present), (mc + 1, Some(Present::Frame { fade: 1 })));
        step(&mut sh, &mut sim, &[], [0, 0]);
        let o = step(&mut sh, &mut sim, &[], [16, 16]);
        assert_eq!(o.menu_sounds.last(), Some(&hooks().begin), "StartGame's SoundBegin (plan-time fact 9)");
        assert_eq!((o.upd, o.phase, sh.fade()), (Phase::Weapsel, Phase::Game, 33));
        assert!(sim.worms.iter().all(|w| w.lives == 15));
    }

    fn to_menu(sh: &mut Shell, sim: &mut SimState) -> Vec<FrameOut> {
        until_routed(sh, sim, DK_ESCAPE)
    }

    #[test]
    fn esc_in_play_ticks_31_faded_frames_then_returns_to_a_pause_menu() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        idle(&mut sh, &mut sim, 5);
        let cycles = sim.cycles;
        let outs = to_menu(&mut sh, &mut sim);
        let got: Vec<Option<Present>> = outs.iter().map(|o| o.present).collect();
        let mut want: Vec<Option<Present>> = (0..=30).rev().map(|f| Some(Present::Frame { fade: f })).collect();
        want.push(Some(Present::Black));
        assert_eq!(got, want, "OnKey: 31, the tail 30..0, then the pop frame's Enter flip");
        assert!(outs.iter().all(|o| o.sim_ticked), "32 ticks, the pop frame's included (finding 9)");
        assert_eq!(sim.cycles, cycles + 32);
        let m = sh.main_menu();
        assert_eq!((m.items[0].visible, m.items[0].string.as_str(), m.items[1].string.as_str()), (true, "RESUME GAME (F1)", "NEW GAME"));
        assert_eq!((sh.main_selection(), sh.fade(), sh.menu_cycles(), sh.top_char()), (0, 0, 0, 'M'));
    }

    #[test]
    fn an_esc_key_up_pauses_but_an_os_repeat_does_not() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        let rep = KeyEvent { dos: DK_ESCAPE, down: true, repeat: true, typed: TypedKey::Sym(0) };
        assert_eq!(step(&mut sh, &mut sim, &[rep], [0, 0]).present, Some(Present::Frame { fade: 33 }), "repeats never reach OnKey");
        assert_eq!(step(&mut sh, &mut sim, &[ev(DK_ESCAPE, false)], [0, 0]).present, Some(Present::Frame { fade: 30 }), "finding 9");
    }

    #[test]
    fn resume_refocuses_the_same_match_and_fades_in_from_zero() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        to_menu(&mut sh, &mut sim);
        let cycles = sim.cycles;
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(outs.last().unwrap().routed, Some(Route::Resume));
        assert_eq!(sim.cycles, cycles, "the menu never ticks the match");
        assert_eq!(step(&mut sh, &mut sim, &[], [0, 0]).present, Some(Present::Frame { fade: 1 }));
        assert_eq!(sim.cycles, cycles + 1);
    }

    #[test]
    fn new_game_after_play_reuses_the_played_level_with_the_next_seed() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        sim.level.material_id[4321] ^= 0x5a; // a crater
        to_menu(&mut sh, &mut sim); // 32 more ticks, then the pause menu
        let played = sim.level.material_id.clone();
        assert_eq!(played[4321], sim.level.material_id[4321]);
        tap(&mut sh, &mut sim, DK_DOWN);
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        assert_eq!(outs.last().unwrap().routed, Some(Route::NewGame { seed: 22 }));
        assert_eq!(sim.level.material_id, played, "SwapLevel(*old_level)");
        assert_eq!(sh.phase(), Phase::Weapsel);
    }

    #[test]
    fn a_changed_level_setting_generates_from_the_match_seed() {
        let (mut sh, mut sim, _) = boot();
        sh.settings_mut().random_map_width = 512;
        until_routed(&mut sh, &mut sim, DK_RETURN);
        let want = generate_level(tc(), sh.settings(), 21);
        assert_eq!((sim.level.width, &sim.level.material_id), (512, &want.material_id));
    }

    #[test]
    fn resume_into_selection_draws_over_the_menus_frozen_screen() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        until_routed(&mut sh, &mut sim, DK_RETURN);
        step(&mut sh, &mut sim, &[], [1, 0]);
        step(&mut sh, &mut sim, &[], [0, 0]);
        to_menu(&mut sh, &mut sim);
        assert!(sh.main_menu().items[0].visible, "selection is Running()");
        let bar = |b: &Bitmap| -> Vec<u32> { (151..158).flat_map(|y| (0..160).map(move |x| (x, y))).map(|(x, y)| b.get_pixel(x, y)).collect() };
        let menu_bar = bar(sh.frozen());
        until_routed(&mut sh, &mut sim, DK_RETURN);
        step(&mut sh, &mut sim, &[], [0, 0]);
        assert_eq!(sh.phase(), Phase::Weapsel);
        assert_eq!(bar(sh.surface()), menu_bar, "finding 11: the copyright bar stays");
    }

    #[test]
    fn quit_fades_out_and_ends_without_a_present() {
        let (mut sh, mut sim, _) = boot();
        idle(&mut sh, &mut sim, 40);
        tap(&mut sh, &mut sim, DK_ESCAPE);
        let outs = until_routed(&mut sh, &mut sim, DK_RETURN);
        let last = outs.last().unwrap();
        assert_eq!((last.quit, last.present, last.routed, last.phase), (true, None, Some(Route::Quit), Phase::Quit));
        assert_eq!(outs[outs.len() - 2].present, Some(Present::Frame { fade: 0 }), "the last present is black");
        assert!(step(&mut sh, &mut sim, &[], [0, 0]).quit);
    }

    #[test]
    fn game_over_runs_180_frames_then_returns_to_a_menu_without_resume() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        sim.worms[1].lives = 0;
        let mut n = 0;
        loop {
            let o = step(&mut sh, &mut sim, &[], [0, 0]);
            n += 1;
            if let Some(r) = o.routed {
                assert_eq!(r, Route::Menu);
                break;
            }
            assert!(n < 400);
        }
        assert_eq!(n, 181, "the detection frame (179) ... 0, then the pop frame");
        let m = sh.main_menu();
        assert_eq!((m.items[0].visible, m.items[1].string.as_str(), sh.main_selection()), (false, "NEW GAME (F1)", 1));
    }

    #[test]
    fn f1_is_new_game_at_boot_and_resume_when_paused() {
        let (mut sh, mut sim, _) = boot();
        assert_eq!(until_routed(&mut sh, &mut sim, DK_F1).last().unwrap().routed, Some(Route::NewGame { seed: 21 }));
        to_menu(&mut sh, &mut sim);
        assert_eq!(until_routed(&mut sh, &mut sim, DK_F1).last().unwrap().routed, Some(Route::Resume));
    }

    #[test]
    fn f5_restart_is_the_next_new_game_without_the_menu() {
        let (mut sh, mut sim, _) = boot();
        start_match(&mut sh, &mut sim);
        let o = sh.frame(&mut sim, &ShellInput { restart: true, ..ShellInput::idle() });
        assert_eq!((o.routed, o.phase, sh.top_char()), (Some(Route::NewGame { seed: 22 }), Phase::Weapsel, 'G'));
    }

    #[test]
    fn a_skip_route_boots_straight_into_play() {
        let opts = StartOptions { skip_selection: true, loadout: vec!["BAZOOKA".into()], touch_only: false };
        let (sh, sim, out) = Shell::boot_playing(tc(), Settings::default(), SeedSource::Fixed(7), 0, opts);
        assert_eq!((out.phase, out.routed, sh.top_char()), (Phase::Game, Some(Route::NewGame { seed: 7 }), 'G'));
        let want = generate_level(tc(), &Settings::default(), 7);
        assert_eq!(sim.level.material_id, want.material_id, "?seed=7: 4½c's first match");
        let (_, _, sel) = Shell::boot_playing(tc(), Settings::default(), SeedSource::Fixed(7), 0, StartOptions::default());
        assert_eq!(sel.phase, Phase::Weapsel, "?level= / ?seed= keep selection");
    }
}
```

Two expectations need care:
- In `the_keys_move_the_cursor_with_crossed_sounds`, the `tap(DK_UP)` right before `tap(56)` moves the cursor from QUIT (9) to 8 first. LAlt then sends it back to 9.
- In `placeholders_…`, JOIN LAN is index 3, HOST ONLINE 4 and JOIN ONLINE 5. These three get two sounds; HOST LAN (2), OPTIONS (6), REPLAYS (7), TC (8), LEFT/RIGHT/NETWORK PLAYER (11–13) and MATCH SETUP (14) get one.

Run: `cd /home/user/openliero/rust && cargo test -p ui shell 2>&1 | tail -20` — Expected, first: FAIL to compile until Steps 6–7 are in, then the failing assertions show what is left.

- [ ] **Step 9: GREEN, the firewall, the re-diff**

Run: `rustfmt --edition 2024 /home/user/openliero/rust/ui/src/shell/*.rs`
Run: `cd /home/user/openliero/rust && cargo test -p ui` — Expected: PASS (every earlier test plus Step 1's 4, Step 2's rewrite, Step 3's 5, Step 5's 3 and Step 8's 17).
Run: `grep -n 'SimState' /home/user/openliero/rust/ui/src/shell/main_menu.rs` — Expected: no output (LD 3: the menu cannot reach the sim).
Run: `grep -rnE 'f32|f64|HashMap|HashSet|SystemTime|Instant' /home/user/openliero/rust/ui/src` — Expected: no output.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game 2>&1 | grep -cE 'test result: FAILED'` — Expected: `0`.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS (the `--live <scenario>` path through the new `render` signature and `tail`).
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown 2>&1 | tail -2` — Expected: builds.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 10: Commit**

```
git -C /home/user/openliero add rust/ui rust/game/src/main.rs
git -C /home/user/openliero commit -m "ui(4.5d): the shell — ScreenStack, MainMenuState, the router + LevelSlot/SeedSource, Match (Esc/focus/tail, SoundBegin), shared frozen screen, Shell::frame" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

Reviewer (Opus): check the following.
- Every `Shell::frame` branch cites `gfx.cpp:1467-1652` and follows its order: events, then `sel`/`fading` captured before update, then update, then pop → router (no draw and no ++), else palettes, draw, `++menu_cycles` for Playing, and the present.
- The Esc condition is `!down || !repeat` while Playing.
- The router only assigns `*sim`.
- `Match::draw` is called exactly once per tick, on the frame loop or on the back-to-menu pop, never both.
- `MainMenuState` has no path to `SimState`.
- The placeholder sounds match plan-time fact 2.

---
### Task 8: C++ — `oracle_dump_shell` (G2: the real `Gfx::RunOneFrame`, headless)  [Opus]

**Files:**
- Create: `src/tools/oracle_dump/shell_dump.cpp`
- Modify: `CMakeLists.txt` (after T4's two `oracle_dump_menu` lines)

**Interfaces:**
- Produces (used by T9, T10): `build/linux-x64/Release/oracle_dump_shell <script.txt> <out.txt> [--ppm-dir <dir>]`, run from the repo root. The **G2 script** and **golden** formats (design §6.3, refined by plan-time fact 10):

```
# comment
setup <file|default>              the setup sidecar (C++-schema TOML, next to the script) or Settings()
boot_seed <u32>
match_seed <u32>                  one per NEW GAME, consumed in order (all must be consumed)
frames <n>
expect quit|frames
key <frame> down|up|repeat <NAME> NAME: ESC RETURN KP_ENTER UP DOWN LEFT RIGHT PAGEUP PAGEDOWN
                                  LCTRL RCTRL LALT RALT LSHIFT RSHIFT SPACE TAB F1..F12 A..Z 0..9
```

```
boot <presents> <presented16|-> <bmp16> <fade> <menu_cycles> <top> <sel>
f <frame> <upd> <presents> <presented16|-> <bmp16> <fade> <menu_cycles> <top> <sel> <sounds|->
end <frame> quit|frames
```

The fields:
- `presents` is 0 or 1.
- `presented16` is FNV-1a over the RGB of `sdl_draw_surface`, the faded presented frame (the same function as `render::hash::hash_frame`), or `-` when nothing was presented.
- `bmp16` is `hash_frame(play_renderer.bmp, 33)`.
- `fade` is `play_renderer.fade_value`.
- `menu_cycles` is `gfx.menu_cycles` (`%u`).
- `upd` is the screen that ran `Update`, read before the frame: `M`, or `W` (GamePlay in weapon selection), or `G`.
- `top` is the screen after the frame: `M`, `G`, or `-` for empty.
- `sel` is `gfx.main_menu.Selection()`.
- `sounds` are the logged sample ids in order.
- `end <n> quit` names the frame whose `RunOneFrame` returned false; `end <frames> frames` means the budget ran out.

- Consumes: the real `Gfx` (`LoadMenus`, `InitFrameStepping`, `RunOneFrame`, `Flip`/`Draw`), `StateStack`, `MainMenuState`, `GamePlayState`, `LocalController`, `Game`, `WeaponSelection`, `Settings::FromToml`, `Common::load`, and `weapsel_drive::{RecordingSoundPlayer, Fail}`.

Why: design §6.2 and done-when 2/3. This is the pixel oracle for the whole shell. It is the real frame loop with only the eight interventions (the design's seven plus plan-time fact 3). Design §10's first risk is "running the real `Gfx` headless", so this task proves it on smoke cases **before** any corpus work (T9). If it cannot run headlessly, it stops and reports (Addendum A).

- [ ] **Step 1: Write the dumper**

Create `src/tools/oracle_dump/shell_dump.cpp`:

```cpp
// Generates the C++ side of the Rust shell gate G2 and the 4½d MILESTONE (Step 4½, slice 4½d;
// design §6.2-§6.6; rust/oracle-tests/tests/shell_golden.rs). The REAL Gfx::RunOneFrame runs
// headlessly — the REAL StateStack, MainMenuState, GamePlayState, LocalController (weapon
// selection with its 12/3 repeat, the Esc fade, game over), Game::ProcessFrame / Draw,
// UpdateMenuPalettes, DrawBasicMenu and Flip -> Gfx::Draw -> ScaleDraw into a 320x200 ARGB surface
// through a software renderer. Each frame the dumper pushes that frame's script key events
// (SDL_PushEvent) and calls RunOneFrame; it writes a `boot` line after InitFrameStepping, one
// `f` line per frame and an `end` line (formats: docs/superpowers/plans/
// 2026-09-26-liero-rs-step4.5-slice4.5d-plan.md, Task 8).
//
// Interventions, each at a point where no real code runs:
//   1. boot seed: gfx.rand.Seed(boot_seed) before InitFrameStepping (C++ seeds from the clock);
//   2. match seeds: gfx.rand.Seed(<next match seed>) before every frame whose top is the main menu
//      (nothing in the menu draws gfx.rand; only the router's GenerateFromSettings does);
//   3. game seed: after a frame that made a new controller, its Game's rand.Seed(<that seed>). The
//      Game ctor seeds from time(nullptr) and GamePlayState::Enter already ran the
//      WeaponSelection constructor: the reseed is exact only if it drew nothing, which is CHECKED
//      (the rand must still equal a fresh Rand seeded with a time value of that frame);
//   4. no stats screen: that Game's stats_recorder becomes the base StatsRecorder, so game over
//      pops to the menu (gamePlayState.cpp:57-71, :93); 4½g drops this;
//   5. pacing: gfx.last_frame = 0 before each frame; Flip adds 14 per present
//      (gfx.cpp:1176-1189), so presents = last_frame / 14;
//   6. no replays: settings->record_replays = false (localController.cpp:237);
//   7. sounds: gfx.sound_player is a RecordingSoundPlayer (every Game installs it globally);
//   8. the frames run with CWD = data/TC/openliero, so a TC-relative level_file resolves as the
//      Rust port's read_asset does (level.cpp:401-411); a setup whose level file does not open
//      from there is refused. Common, the menus and the setup are loaded before the chdir.
// Usage (from the repo root): oracle_dump_shell <script.txt> <out.txt> [--ppm-dir <dir>]
// Built via OPENLIERO_BUILD_ORACLE_DUMP (rust/oracle-tests/gen_shell_golden.sh). Not part of the
// default build.
#include <SDL3/SDL.h>

#include <algorithm>
#include <array>
#include <cinttypes>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <ctime>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "common.hpp"
#include "controller/controller.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "gamePlayState.hpp"
#include "gfx.hpp"
#include "gfx/bitmap.hpp"
#include "keys.hpp"
#include "mainMenuState.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "rand.hpp"
#include "settings.hpp"
#include "state.hpp"
#include "stats_recorder.hpp"
#include "weapsel_drive.hpp"

namespace {

using weapsel_drive::Fail;
using weapsel_drive::RecordingSoundPlayer;

constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr uint64_t kFnvPrime = 1099511628211ULL;
constexpr int kIdentityFade = 33;
constexpr int kW = 320;
constexpr int kH = 200;

uint64_t FnvByte(uint64_t h, uint8_t b) { return (h ^ b) * kFnvPrime; }

uint8_t FadeChannel(uint8_t v, int amount) {
  return amount >= 32 ? v : static_cast<uint8_t>((v * amount) >> 5);
}

uint64_t HashBitmap(Bitmap const& bmp, int fade) {
  uint64_t h = kFnvOffset;
  for (int y = 0; y < bmp.h; ++y) {
    for (int x = 0; x < bmp.w; ++x) {
      uint32_t const kC = bmp.GetPixel(x, y);
      h = FnvByte(h, FadeChannel((kC >> 16) & 0xFFU, fade));
      h = FnvByte(h, FadeChannel((kC >> 8) & 0xFFU, fade));
      h = FnvByte(h, FadeChannel(kC & 0xFFU, fade));
    }
  }
  return h;
}

uint32_t SurfacePixel(SDL_Surface const& s, int x, int y) {
  auto const* row = static_cast<uint8_t const*>(s.pixels) + (static_cast<std::size_t>(y) * s.pitch);
  uint32_t c = 0;
  std::memcpy(&c, row + (static_cast<std::size_t>(x) * 4), sizeof c);
  return c;
}

// The presented frame: what Gfx::Draw left in sdl_draw_surface (ARGB8888, already faded).
uint64_t HashSurface(SDL_Surface const& s) {
  uint64_t h = kFnvOffset;
  for (int y = 0; y < s.h; ++y) {
    for (int x = 0; x < s.w; ++x) {
      uint32_t const kC = SurfacePixel(s, x, y);
      h = FnvByte(h, (kC >> 16) & 0xFFU);
      h = FnvByte(h, (kC >> 8) & 0xFFU);
      h = FnvByte(h, kC & 0xFFU);
    }
  }
  return h;
}

std::string Hex16(uint64_t v) {
  std::array<char, 24> buf{};
  std::snprintf(buf.data(), buf.size(), "%016" PRIx64, v);
  return buf.data();
}

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

void WriteSurfacePpm(std::string const& path, SDL_Surface const& s) {
  std::string data = "P6\n" + std::to_string(s.w) + " " + std::to_string(s.h) + "\n255\n";
  for (int y = 0; y < s.h; ++y) {
    for (int x = 0; x < s.w; ++x) {
      uint32_t const kC = SurfacePixel(s, x, y);
      data += static_cast<char>((kC >> 16) & 0xFFU);
      data += static_cast<char>((kC >> 8) & 0xFFU);
      data += static_cast<char>(kC & 0xFFU);
    }
  }
  std::ofstream f(path, std::ios::binary | std::ios::trunc);
  f << data;
  if (!f) {
    Fail("cannot write " + path);
  }
}

SDL_Scancode ScancodeOf(std::string const& t) {
  static std::map<std::string, SDL_Scancode> const kNames = {
      {"ESC", SDL_SCANCODE_ESCAPE},     {"RETURN", SDL_SCANCODE_RETURN},
      {"KP_ENTER", SDL_SCANCODE_KP_ENTER}, {"UP", SDL_SCANCODE_UP},
      {"DOWN", SDL_SCANCODE_DOWN},      {"LEFT", SDL_SCANCODE_LEFT},
      {"RIGHT", SDL_SCANCODE_RIGHT},    {"PAGEUP", SDL_SCANCODE_PAGEUP},
      {"PAGEDOWN", SDL_SCANCODE_PAGEDOWN}, {"LCTRL", SDL_SCANCODE_LCTRL},
      {"RCTRL", SDL_SCANCODE_RCTRL},    {"LALT", SDL_SCANCODE_LALT},
      {"RALT", SDL_SCANCODE_RALT},      {"LSHIFT", SDL_SCANCODE_LSHIFT},
      {"RSHIFT", SDL_SCANCODE_RSHIFT},  {"SPACE", SDL_SCANCODE_SPACE},
      {"TAB", SDL_SCANCODE_TAB},        {"F1", SDL_SCANCODE_F1},
      {"F2", SDL_SCANCODE_F2},          {"F3", SDL_SCANCODE_F3},
      {"F4", SDL_SCANCODE_F4},          {"F5", SDL_SCANCODE_F5},
      {"F6", SDL_SCANCODE_F6},          {"F7", SDL_SCANCODE_F7},
      {"F8", SDL_SCANCODE_F8},          {"F9", SDL_SCANCODE_F9},
      {"F10", SDL_SCANCODE_F10},        {"F11", SDL_SCANCODE_F11},
      {"F12", SDL_SCANCODE_F12},
  };
  auto const kIt = kNames.find(t);
  if (kIt != kNames.end()) {
    return kIt->second;
  }
  if (t.size() == 1 && t[0] >= 'A' && t[0] <= 'Z') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_A + (t[0] - 'A'));
  }
  if (t.size() == 1 && t[0] >= '1' && t[0] <= '9') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_1 + (t[0] - '1'));
  }
  if (t == "0") {
    return SDL_SCANCODE_0;
  }
  Fail("unknown key name " + t);
}

struct KeyEv {
  int frame = 0;
  bool down = false;
  bool repeat = false;
  SDL_Scancode sc = SDL_SCANCODE_UNKNOWN;
};

struct Case {
  std::string setup = "default";
  uint32_t boot_seed = 0;
  bool boot_given = false;
  std::vector<uint32_t> match_seeds;
  int frames = -1;
  std::string expect;
  std::vector<KeyEv> keys;
};

Case ParseCase(std::string const& path) {
  std::istringstream in(Slurp(path));
  Case c;
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string key;
    if (!(ls >> key) || key[0] == '#') {
      continue;
    }
    if (key == "setup") {
      ls >> c.setup;
    } else if (key == "boot_seed" && (ls >> c.boot_seed)) {
      c.boot_given = true;
    } else if (key == "match_seed") {
      uint32_t s = 0;
      if (!(ls >> s)) {
        Fail("bad match_seed line: " + line);
      }
      c.match_seeds.push_back(s);
    } else if (key == "frames" && (ls >> c.frames)) {
    } else if (key == "expect" && (ls >> c.expect) && (c.expect == "quit" || c.expect == "frames")) {
    } else if (key == "key") {
      KeyEv k;
      std::string kind;
      std::string name;
      if (!(ls >> k.frame >> kind >> name) || k.frame < 0 ||
          (kind != "down" && kind != "up" && kind != "repeat")) {
        Fail("bad key line: " + line);
      }
      k.down = kind != "up";
      k.repeat = kind == "repeat";
      k.sc = ScancodeOf(name);
      c.keys.push_back(k);
    } else {
      Fail("bad script line: " + line);
    }
  }
  if (!c.boot_given || c.frames <= 0 || c.expect.empty()) {
    Fail("a shell script needs boot_seed, frames and expect");
  }
  std::stable_sort(c.keys.begin(), c.keys.end(),
                   [](KeyEv const& a, KeyEv const& b) { return a.frame < b.frame; });
  return c;
}

void Push(KeyEv const& k) {
  SDL_Event ev{};
  ev.type = k.down ? SDL_EVENT_KEY_DOWN : SDL_EVENT_KEY_UP;
  ev.key.scancode = k.sc;
  ev.key.key = SDL_GetKeyFromScancode(k.sc, SDL_KMOD_NONE, false);
  ev.key.mod = SDL_KMOD_NONE;
  ev.key.down = k.down;
  ev.key.repeat = k.repeat;
  if (!SDL_PushEvent(&ev)) {
    Fail(std::string("SDL_PushEvent: ") + SDL_GetError());
  }
}

char TopOf(AppState* s) {
  if (s == nullptr) {
    return '-';
  }
  if (dynamic_cast<MainMenuState*>(s) != nullptr) {
    return 'M';
  }
  if (dynamic_cast<GamePlayState*>(s) != nullptr) {
    return 'G';
  }
  Fail("a state 4½d does not model is on the stack");
}

// The fields every line shares: bmp16 fade menu_cycles top sel.
std::string Tail() {
  return Hex16(HashBitmap(gfx.play_renderer.bmp, kIdentityFade)) + " " +
         std::to_string(gfx.play_renderer.fade_value) + " " + std::to_string(gfx.menu_cycles) +
         " " + TopOf(gfx.state_stack.Top()) + " " + std::to_string(gfx.main_menu.Selection());
}

std::string Presented(std::string const& ppm_dir, std::string const& name) {
  uint64_t const kPresents = gfx.last_frame / 14;
  if (kPresents > 1) {
    Fail("more than one present in a frame");
  }
  if (kPresents == 0) {
    return "0 -";
  }
  if (!ppm_dir.empty()) {
    WriteSurfacePpm(ppm_dir + "/" + name + ".ppm", *gfx.sdl_draw_surface);
  }
  return "1 " + Hex16(HashSurface(*gfx.sdl_draw_surface));
}

}  // namespace

int main(int argc, char** argv) {
  std::vector<std::string> const kArgs(argv + 1, argv + argc);
  std::vector<std::string> positional;
  std::string ppm_dir;
  for (std::size_t i = 0; i < kArgs.size(); ++i) {
    if (kArgs[i] == "--ppm-dir" && i + 1 < kArgs.size()) {
      ppm_dir = std::filesystem::absolute(kArgs[++i]).string();
    } else if (kArgs[i].starts_with("--")) {
      Fail("unknown or incomplete option " + kArgs[i]);
    } else {
      positional.push_back(kArgs[i]);
    }
  }
  if (positional.size() != 2) {
    Fail("usage: oracle_dump_shell <script.txt> <out.txt> [--ppm-dir <dir>]");
  }
  std::string const kScript = positional[0];
  std::string const kOut = std::filesystem::absolute(positional[1]).string();
  Case const kCase = ParseCase(kScript);

  // GameEntry's setup (gameEntry.cpp:21-76), headless.
  if (!SDL_Init(SDL_INIT_EVENTS)) {
    Fail(std::string("SDL_Init: ") + SDL_GetError());
  }
  InitKeys();
  PrecomputeTables();
  gfx.LoadMenus();
  auto settings = std::make_shared<Settings>();
  if (kCase.setup != "default") {
    try {
      settings->FromToml(Slurp(DirOf(kScript) + "/" + kCase.setup));
    } catch (std::exception const& e) {
      Fail("setup " + kCase.setup + ": " + e.what());
    }
  }
  settings->record_replays = false;  // intervention 6
  gfx.settings = settings;
  gfx.settings_node = FsNode("data") / "Setups" / "liero.cfg";
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");
  gfx.common = common;
  gfx.play_renderer.Init(kW, kH);
  gfx.play_renderer.LoadPalette(*common);
  gfx.single_screen_renderer.Init(640, 400);
  ColorMode const kMode = settings->modern_colors ? ColorMode::kModern : ColorMode::kClassic;
  gfx.play_renderer.mode = kMode;
  gfx.single_screen_renderer.mode = kMode;

  // Headless presentation (design §6.2 step 4): a 320x200 draw surface makes FitScreen pick
  // magnification 1 at offset 0 (blit.cpp:860-876), so the real Flip -> Gfx::Draw -> ScaleDraw
  // writes exactly the faded play_renderer frame into it.
  gfx.sdl_draw_surface = SDL_CreateSurface(kW, kH, SDL_PIXELFORMAT_ARGB8888);
  SDL_Surface* target = SDL_CreateSurface(kW, kH, SDL_PIXELFORMAT_ARGB8888);
  gfx.sdl_renderer = target != nullptr ? SDL_CreateSoftwareRenderer(target) : nullptr;
  gfx.sdl_texture = gfx.sdl_renderer != nullptr
                        ? SDL_CreateTexture(gfx.sdl_renderer, SDL_PIXELFORMAT_ARGB8888,
                                            SDL_TEXTUREACCESS_STREAMING, kW, kH)
                        : nullptr;
  if (gfx.sdl_draw_surface == nullptr || gfx.sdl_texture == nullptr) {
    Fail(std::string("headless presentation: ") + SDL_GetError() +
         " (plan T8: try the dummy-video fallback)");
  }

  auto rec = std::make_shared<RecordingSoundPlayer>();  // intervention 7
  gfx.sound_player = rec;
  g_sound_player = rec.get();

  std::filesystem::current_path("data/TC/openliero");  // intervention 8
  if (!settings->random_level) {
    std::string path = settings->level_file;
    if (!path.contains('.')) {
      path += ".LEV";
    }
    if (!std::ifstream(path).good()) {
      Fail("the setup's level file " + path + " does not open from data/TC/openliero");
    }
  }

  std::string out = "# oracle_dump_shell " + kScript +
                    " — the REAL C++ Gfx frame loop, headless (Step 4½d design §6.2)\n"
                    "# boot <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> <sel>\n"
                    "# f <frame> <upd> <presents> <presented16> <bmp16> <fade> <menu_cycles> "
                    "<top> <sel> <sounds>\n";
  gfx.rand.Seed(kCase.boot_seed);  // intervention 1
  gfx.last_frame = 0;              // intervention 5
  gfx.InitFrameStepping();
  out += "boot " + Presented(ppm_dir, "boot") + " " + Tail() + "\n";

  std::size_t next_seed = 0;
  std::size_t next_key = 0;
  int end = kCase.frames;
  bool quit = false;
  for (int frame = 0; frame < kCase.frames; ++frame) {
    char const kTop = TopOf(gfx.state_stack.Top());
    if (kTop == 'M' && next_seed < kCase.match_seeds.size()) {
      gfx.rand.Seed(kCase.match_seeds[next_seed]);  // intervention 2
    }
    char upd = kTop;
    if (kTop == 'G') {
      upd = gfx.controller->InWeaponSelection() ? 'W' : 'G';
    }
    while (next_key < kCase.keys.size() && kCase.keys[next_key].frame == frame) {
      Push(kCase.keys[next_key++]);
    }
    Controller const* const kBefore = gfx.controller.get();
    std::time_t const kT0 = std::time(nullptr);
    rec->played.clear();
    gfx.last_frame = 0;  // intervention 5
    bool const kGo = gfx.RunOneFrame();
    std::time_t const kT1 = std::time(nullptr);
    if (gfx.controller.get() != kBefore) {
      // A NEW GAME made a controller: interventions 3 and 4.
      std::string const kAt = "frame " + std::to_string(frame) + ": ";
      if (next_seed >= kCase.match_seeds.size()) {
        Fail(kAt + "a NEW GAME with no match_seed left");
      }
      if (settings->game_mode == Settings::kGmHoldazone) {
        Fail(kAt + "a Holdazone match (unported in Rust): use Holdazone setups for boot cases only");
      }
      Game& game = *gfx.controller->CurrentGame();
      bool untouched = false;
      for (std::time_t t = kT0 - 1; t <= kT1 + 1; ++t) {
        Rand probe;
        probe.Seed(static_cast<uint32_t>(t));
        untouched = untouched || (probe.engine == game.rand.engine && probe.last == game.rand.last);
      }
      if (!untouched) {
        Fail(kAt + "the WeaponSelection constructor drew the RNG (intervention 3's precondition)");
      }
      game.rand.Seed(kCase.match_seeds[next_seed++]);
      game.stats_recorder = std::make_shared<StatsRecorder>();
    }
    std::string sounds;
    for (int const kId : rec->played) {
      sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
    }
    std::array<char, 16> name{};
    std::snprintf(name.data(), name.size(), "f_%04d", frame);
    out += "f " + std::to_string(frame) + " " + upd + " " + Presented(ppm_dir, name.data()) + " " +
           Tail() + " " + (sounds.empty() ? "-" : sounds) + "\n";
    if (!kGo) {
      end = frame;
      quit = true;
      break;
    }
  }
  if (next_key != kCase.keys.size()) {
    Fail("key events scheduled after the run ended");
  }
  if (next_seed != kCase.match_seeds.size()) {
    Fail("unused match seeds");
  }
  if ((kCase.expect == "quit") != quit) {
    Fail("expected to end by " + kCase.expect);
  }
  out += "end " + std::to_string(end) + " " + (quit ? "quit" : "frames") + "\n";

  std::ofstream f(kOut, std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail("cannot write " + kOut);
  }
  std::printf("oracle_dump_shell: %d frames, %zu NEW GAMEs, end %s\n", quit ? end + 1 : end,
              next_seed, quit ? "quit" : "frames");
  gfx.controller.reset();
  g_sound_player = nullptr;
  return 0;
}
```

Add `#include <cstring>` for `std::memcpy`. `ColorMode` comes through `gfx.hpp`.

`CMakeLists.txt`: after the two `oracle_dump_menu` lines add

```cmake
  add_executable(oracle_dump_shell src/tools/oracle_dump/shell_dump.cpp)
  target_link_libraries(oracle_dump_shell PRIVATE game)
```

- [ ] **Step 2: Build it and prove it on smoke cases (before any corpus work)**

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && cd /home/user/openliero && cmake --build build/linux-x64 --config Release --target oracle_dump_shell 2>&1 | tail -5`
Expected: builds.

Smoke A (boot only). Write `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/boot_script.txt`:

```
setup default
boot_seed 11
frames 40
expect frames
```

Run: `mkdir -p /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/ppm && cd /home/user/openliero && build/linux-x64/Release/oracle_dump_shell /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/boot_script.txt /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/boot.txt --ppm-dir /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/ppm && head -5 /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/boot.txt && tail -2 /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/boot.txt`

Expected:
- `oracle_dump_shell: 40 frames, 0 NEW GAMEs, end frames`.
- The `boot` line is `boot 1 <black> <bmp16> 0 0 M 1`, where `<black>` must equal the all-black hash. Compute it with `python3 -c "h=1469598103934665603; exec('for _ in range(192000): h=(h*1099511628211)&0xFFFFFFFFFFFFFFFF'); print('%016x'%h)"` (FNV over 192000 zero bytes: `h ^ 0` is `h`).
- `f 0 M 1 <hash> <bmp16> 1 1 M 1 -` … `f 39 M 1 … 32 40 M 1 -`: the fade climbs 1..32 and holds; `menu_cycles` counts 1..40.
- `end 40 frames`.

Convert `boot.ppm` and `f_0039.ppm` to PNG with the scratchpad's `ppm2png.py`, then look at them (Read tool): the boot frame is black; frame 39 is the menu over a generated level, the settings column on the right, and the copyright bar with its `ä`s at the bottom left.

If Smoke A fails at "headless presentation", switch to **fallback 1**:
- `SDL_Init(SDL_INIT_EVENTS | SDL_INIT_VIDEO)` under `SDL_VIDEODRIVER=dummy`.
- Set `gfx.double_res = false` (so `OnWindowResize` makes a 320×200 `sdl_draw_surface`, `gfx.cpp:380-388`).
- Call the real `gfx.SetVideoMode()` instead of creating the three SDL objects.
- Assert `gfx.sdl_draw_surface->w == 320 && ->h == 200`.

Record the switch in the header comment and in the gen script's environment. If fallback 1 fails too, **stop and report** the exact error (Addendum A).

Smoke B (NEW GAME, selection, Esc, menu). `…/g2/ng_script.txt`:

```
setup default
boot_seed 11
match_seed 21
frames 150
expect frames
key 40 down RETURN
key 42 up RETURN
key 76 down R
key 78 up R
key 76 down UP
key 78 up UP
key 80 down LCTRL
key 82 up LCTRL
key 80 down RCTRL
key 82 up RCTRL
key 100 down ESC
key 102 up ESC
```

Expected:
- 1 NEW GAME, `end 150 frames`.
- Frames 40..72 are `M` with fade 32 then 31..0. Frame 73 is the pop: `M 0 - … G`, and its `menu_cycles` equals frame 72's.
- Frames 74..: `W` with fade 1, 2, …; the frame with both Fire presses shows `W`, then `G` on the next.
- The DONE frame's sounds end in `22` (SoundBegin).
- From frame 100 the fade goes 30..0, and the pop frame shows `1 <black>` with top `M` and `sel 0`.
- The dumper did not refuse intervention 3's precondition.

Smoke C (`level_file`). A sidecar `…/g2/lf_setup.cfg` with `[settings]`, `version = 6`, `randomLevel = false`, `levelFile = 'Levels/water_stage.lev'`, and a script `setup lf_setup.cfg`, `boot_seed 1`, `frames 5`, `expect frames`. Expected: it runs. Also run it with `levelFile = 'Levels/nope.lev'`: it must refuse ("does not open from data/TC/openliero").

- [ ] **Step 3: clang-format + clang-tidy**

Run: `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/shell_dump.cpp` — Expected: no output (else `-i`, re-check).
Run: `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 a61cbb6` — Expected: exit 0.

- [ ] **Step 4: Commit** (the smoke files stay in the scratchpad)

```
git -C /home/user/openliero add src/tools/oracle_dump/shell_dump.cpp CMakeLists.txt
git -C /home/user/openliero commit -m "oracle(4.5d): oracle_dump_shell — the REAL Gfx::RunOneFrame headless (software renderer), eight documented interventions" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 9: G2 — the shell corpus (`gen_slice4_5d.rs`), `gen_shell_golden.sh`, the C++ goldens  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/shell_common/mod.rs`
- Create: `rust/oracle-tests/examples/gen_slice4_5d.rs`
- Create: `rust/oracle-tests/gen_shell_golden.sh`
- Create (generated, committed):
  - `rust/oracle-tests/golden/shell_<case>_script.txt` ×11;
  - `shell_{level_file,gametag,holdazone_boot,regenerate,game_over}_setup.cfg`;
  - `shell_<case>.txt` ×11.

**Interfaces:**
- Produces (used by T10):
  - `shell_common::{TC_ROOT, GOLDEN, CASES_NAMES, ALLOWED, key_of, Kind, KeyLine, ShellScript, B, Case, cases, find_game_over_seed, settings_for, drive, Run, Ledger}`;
  - the 11 committed cases and their C++ goldens.
- Consumes: T7's `ui::shell::Shell`; T8's `oracle_dump_shell` and its formats; `scenario::settings_toml::settings_from_toml`.

Why: design §6.4–§6.5. The cases walk every transition the milestone crosses, plus the corners each finding names.

The generator drives every case through the real Rust shell and refuses a case that the C++ gate cannot fairly judge:
- a placeholder or an F-key the C++ menu acts on (plan-time fact 2);
- a worm key held at a back-to-menu pop (plan-time fact 12);
- a key with two events in one frame;
- a NEW GAME whose selection constructor draws the RNG (intervention 3);
- a Holdazone match.

The goldens are C++'s (4½c A1): there is no Rust self-golden.

- [ ] **Step 1: `shell_common` — the script model, the builder, the driver, the validators**

Create `rust/oracle-tests/tests/shell_common/mod.rs`:

```rust
//! Step 4½d G2 — the shell cases (design §6.4; plan Tasks 8-10): the script model and builder,
//! the case table, the Rust driver that produces the golden lines through `ui::shell::Shell`,
//! and the validators the generator enforces. The C++ side is `oracle_dump_shell`.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;

use render::bitmap::Bitmap;
use render::hash::hash_frame;
use scenario::settings::{GM_HOLDAZONE, Settings};
use scenario::settings_toml::settings_from_toml;
use sim::state::ControlState;
use ui::keys::TypedKey;
use ui::shell::level_slot::SeedSource;
use ui::shell::playing::StartOptions;
use ui::shell::{KeyEvent, Phase, Present, Route, Shell, ShellInput};
use ui::text::UiTc;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

pub const CASES_NAMES: [&str; 11] = [
    "boot_idle", "nav", "level_file", "gametag", "holdazone_boot", "milestone", "esc_in_weapsel",
    "reuse", "regenerate", "game_over", "f1_quit",
];

/// The keys a case may press: nothing the C++ menu would act on where the Rust 4½d menu is inert
/// (plan-time fact 2 — no F-key but F1). R/F/D/G are P1's up/down/left/right.
pub const ALLOWED: [&str; 20] = [
    "ESC", "RETURN", "KP_ENTER", "UP", "DOWN", "LEFT", "RIGHT", "PAGEUP", "PAGEDOWN", "LCTRL",
    "RCTRL", "LALT", "RALT", "LSHIFT", "RSHIFT", "R", "F", "D", "G", "F1",
];

/// DOS index of each letter (`keys.cpp:9-35`).
const LETTERS: [(u8, u32); 26] = [
    (b'Q', 16), (b'W', 17), (b'E', 18), (b'R', 19), (b'T', 20), (b'Y', 21), (b'U', 22), (b'I', 23),
    (b'O', 24), (b'P', 25), (b'A', 30), (b'S', 31), (b'D', 32), (b'F', 33), (b'G', 34), (b'H', 35),
    (b'J', 36), (b'K', 37), (b'L', 38), (b'Z', 44), (b'X', 45), (b'C', 46), (b'V', 47), (b'B', 48),
    (b'N', 49), (b'M', 50),
];

/// `SDLToDOSKey` (`keys.cpp:9-75`) of a script key name, and its `key_buf` symbol (only the
/// printable ones matter: nothing in 4½d reads key_buf).
pub fn key_of(name: &str) -> (u32, TypedKey) {
    let none = TypedKey::Sym(0);
    match name {
        "ESC" => (1, none),
        "RETURN" => (28, none),
        "KP_ENTER" => (116, none),
        "UP" => (160, none),
        "DOWN" => (168, none),
        "LEFT" => (163, none),
        "RIGHT" => (165, none),
        "PAGEUP" => (161, none),
        "PAGEDOWN" => (169, none),
        "LCTRL" => (29, none),
        "RCTRL" => (117, none),
        "LALT" => (56, none),
        "RALT" => (144, none),
        "LSHIFT" => (42, none),
        "RSHIFT" => (54, none),
        "SPACE" => (57, TypedKey::Sym(32)),
        "TAB" => (15, TypedKey::Tab),
        "F11" => (87, none),
        "F12" => (88, none),
        f if f.len() > 1 && f.starts_with('F') => (58 + f[1..].parse::<u32>().expect("F1..F10"), none),
        c if c.len() == 1 => {
            let b = c.as_bytes()[0];
            let dos = match b {
                b'1'..=b'9' => (b - b'1') as u32 + 2,
                b'0' => 11,
                _ => LETTERS.iter().find(|(l, _)| *l == b).expect("a letter").1,
            };
            (dos, TypedKey::Sym(b.to_ascii_lowercase() as u32))
        }
        other => panic!("unknown key name {other}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Down,
    Up,
    Repeat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyLine {
    pub frame: u32,
    pub kind: Kind,
    pub name: String,
}

/// A `shell_<case>_script.txt` (format: plan Task 8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellScript {
    /// The setup sidecar's file name, `None` = `default`.
    pub setup: Option<String>,
    pub boot_seed: u32,
    pub match_seeds: Vec<u32>,
    pub frames: u32,
    pub expect_quit: bool,
    pub keys: Vec<KeyLine>,
}

impl ShellScript {
    pub fn parse(text: &str) -> ShellScript {
        let mut s = ShellScript::default();
        for line in text.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            match t.as_slice() {
                [] => {}
                [c, ..] if c.starts_with('#') => {}
                ["setup", f] => s.setup = (*f != "default").then(|| f.to_string()),
                ["boot_seed", v] => s.boot_seed = v.parse().unwrap(),
                ["match_seed", v] => s.match_seeds.push(v.parse().unwrap()),
                ["frames", v] => s.frames = v.parse().unwrap(),
                ["expect", v] => s.expect_quit = *v == "quit",
                ["key", f, k, n] => s.keys.push(KeyLine {
                    frame: f.parse().unwrap(),
                    kind: match *k {
                        "down" => Kind::Down,
                        "up" => Kind::Up,
                        "repeat" => Kind::Repeat,
                        other => panic!("bad key kind {other}"),
                    },
                    name: n.to_string(),
                }),
                other => panic!("bad script line {other:?}"),
            }
        }
        s.keys.sort_by_key(|k| k.frame); // stable: the file order within a frame, as the dumper
        s
    }

    pub fn to_text(&self, header: &[String]) -> String {
        let mut out: String = header.iter().map(|h| format!("# {h}\n")).collect();
        out += &format!("setup {}\n", self.setup.as_deref().unwrap_or("default"));
        out += &format!("boot_seed {}\n", self.boot_seed);
        for m in &self.match_seeds {
            out += &format!("match_seed {m}\n");
        }
        out += &format!("frames {}\nexpect {}\n", self.frames, if self.expect_quit { "quit" } else { "frames" });
        for k in &self.keys {
            let kind = match k.kind {
                Kind::Down => "down",
                Kind::Up => "up",
                Kind::Repeat => "repeat",
            };
            out += &format!("key {} {kind} {}\n", k.frame, k.name);
        }
        out
    }
}

/// The script builder: a frame cursor `t`; `at` schedules relative to it without moving it.
pub struct B {
    s: ShellScript,
    t: u32,
}

impl B {
    pub fn new(setup: Option<&str>, boot_seed: u32) -> B {
        B { s: ShellScript { setup: setup.map(str::to_string), boot_seed, ..ShellScript::default() }, t: 0 }
    }

    pub fn at(mut self, dt: u32, kind: Kind, name: &str) -> B {
        self.s.keys.push(KeyLine { frame: self.t + dt, kind, name: name.to_string() });
        self
    }

    pub fn idle(mut self, n: u32) -> B {
        self.t += n;
        self
    }

    /// Down now, up two frames later; the cursor moves 3.
    pub fn tap(self, name: &str) -> B {
        self.at(0, Kind::Down, name).at(2, Kind::Up, name).idle(3)
    }

    /// Several keys down on the same frame (Up + R; both players' DONE).
    pub fn taps(self, names: &[&str]) -> B {
        let mut b = self;
        for n in names {
            b = b.at(0, Kind::Down, n).at(2, Kind::Up, n);
        }
        b.idle(3)
    }

    pub fn hold(self, name: &str, n: u32) -> B {
        self.at(0, Kind::Down, name).at(n, Kind::Up, name).idle(n + 1)
    }

    /// Down, then `count` OS repeats every `every` frames from `first`, then up.
    pub fn repeats(self, name: &str, first: u32, every: u32, count: u32) -> B {
        let mut b = self.at(0, Kind::Down, name);
        for i in 0..count {
            b = b.at(first + i * every, Kind::Repeat, name);
        }
        let up = first + count * every;
        b.at(up, Kind::Up, name).idle(up + 1)
    }

    pub fn seed(mut self, s: u32) -> B {
        self.s.match_seeds.push(s);
        self
    }

    /// After a `tap` that SELECTS a menu item: the select frame shows fade 32, 32 more fade
    /// frames follow, then the pop; the next Playing frame is the tap frame + 34 (finding 14).
    pub fn after_menu_select(self) -> B {
        self.idle(31)
    }

    /// After a `tap` of Esc in play or selection: fades 30..0 from the key-down, the pop at +31;
    /// the first menu frame is the key-down frame + 32 (finding 9).
    pub fn after_esc(self) -> B {
        self.idle(29)
    }

    pub fn end(mut self, extra: u32, quit: bool) -> ShellScript {
        self.s.frames = self.t + extra;
        self.s.expect_quit = quit;
        self.s
    }
}

/// One case: its name, its setup sidecar text (C++-schema TOML), its script.
pub struct Case {
    pub name: &'static str,
    pub setup: Option<&'static str>,
    pub script: ShellScript,
}

const LEVEL_FILE_SETUP: &str = "[settings]\nversion = 6\nrandomLevel = false\nlevelFile = 'Levels/water_stage.lev'\n";
const GAMETAG_SETUP: &str = "[settings]\nversion = 6\ngameMode = 1\n";
const HOLDAZONE_SETUP: &str = "[settings]\nversion = 6\ngameMode = 2\n";
const REGENERATE_SETUP: &str = "[settings]\nversion = 6\nregenerateLevel = true\n";
const GAME_OVER_SETUP: &str = "[player1]\nhealth = 1\n\n[settings]\nversion = 6\nlives = 1\n";

fn setup_name(case: &str) -> String {
    format!("shell_{case}_setup.cfg")
}

/// Both players Up (RANDOMIZE -> DONE!), then both Fire: the selection ends on the Fire frame.
fn both_done(b: B) -> B {
    b.taps(&["R", "UP"]).taps(&["LCTRL", "RCTRL"])
}

/// Movement and fire for `n` match frames (P1 right + fire, P2 left + fire), all keys released
/// by the end.
fn play(b: B, n: u32) -> B {
    assert!(n >= 120);
    b.at(0, Kind::Down, "G")
        .at(10, Kind::Down, "LCTRL")
        .at(12, Kind::Up, "LCTRL")
        .at(25, Kind::Down, "LCTRL")
        .at(27, Kind::Up, "LCTRL")
        .at(40, Kind::Up, "G")
        .at(45, Kind::Down, "LEFT")
        .at(50, Kind::Down, "RCTRL")
        .at(52, Kind::Up, "RCTRL")
        .at(80, Kind::Up, "LEFT")
        .at(90, Kind::Down, "F")
        .at(110, Kind::Up, "F")
        .at(113, Kind::Down, "LCTRL")
        .at(115, Kind::Up, "LCTRL")
        .idle(n)
}

/// The game_over script for a match seed (lives 1, P1 health 1): P1 aims down and keeps firing.
pub fn game_over_script(seed: u32) -> ShellScript {
    let mut b = B::new(Some(&setup_name("game_over")), 41).idle(40).seed(seed).tap("RETURN").after_menu_select();
    b = both_done(b).hold("F", 25);
    for _ in 0..12 {
        b = b.tap("LCTRL").idle(12);
    }
    // The pop to the menu must come before frame 700 (checked by the generator).
    let t = b.t;
    b.idle(700 - t).tap("ESC").idle(5).tap("RETURN").end(40, true)
}

pub fn cases(game_over_seed: u32) -> Vec<Case> {
    let d = None;
    vec![
        Case { name: "boot_idle", setup: None, script: B::new(d, 11).idle(60).end(0, false) },
        Case {
            name: "nav",
            setup: None,
            script: B::new(d, 12)
                .idle(35)
                .tap("DOWN").tap("DOWN").tap("UP").tap("UP").tap("UP").tap("DOWN")
                .tap("R").tap("F")
                .repeats("DOWN", 15, 3, 4)
                .tap("PAGEDOWN").tap("PAGEUP")
                .tap("ESC").tap("UP").tap("LALT").tap("UP").tap("RSHIFT")
                .taps(&["UP", "R"]).idle(3)
                .hold("LEFT", 20).hold("RIGHT", 20).hold("D", 10)
                .idle(20)
                .end(0, false),
        },
        Case { name: "level_file", setup: Some(LEVEL_FILE_SETUP), script: B::new(Some(&setup_name("level_file")), 13).idle(40).end(0, false) },
        Case { name: "gametag", setup: Some(GAMETAG_SETUP), script: B::new(Some(&setup_name("gametag")), 14).idle(40).end(0, false) },
        Case { name: "holdazone_boot", setup: Some(HOLDAZONE_SETUP), script: B::new(Some(&setup_name("holdazone_boot")), 15).idle(40).tap("DOWN").idle(10).end(0, false) },
        Case {
            name: "milestone",
            setup: None,
            script: {
                // design §6.6: boot, idle 40; Down, Up; NEW GAME; selection; play 150; Esc; RESUME;
                // play 60; Esc; Down; NEW GAME (level reused); selection; Esc; Esc; Enter (QUIT).
                let b = B::new(d, 1).idle(40).tap("DOWN").tap("UP").seed(101).tap("RETURN").after_menu_select();
                let b = play(both_done(b), 150).tap("ESC").after_esc();
                let b = b.idle(40).tap("RETURN").after_menu_select();
                let b = b.at(10, Kind::Down, "RCTRL").at(12, Kind::Up, "RCTRL").idle(60).tap("ESC").after_esc();
                let b = b.idle(20).tap("DOWN").seed(102).tap("RETURN").after_menu_select();
                let b = b.idle(15).tap("ESC").after_esc();
                b.idle(20).tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "esc_in_weapsel",
            setup: None,
            script: {
                // Both cursors to slot 1 (P1 F, P2 Down), P2 holds Right (the 12/3 repeat cycles
                // its weapon), Esc mid-selection, RESUME (the copyright bar stays, finding 11),
                // Up twice (1 -> RANDOMIZE -> DONE!), both Fire, play, Esc, QUIT.
                let b = B::new(d, 2).idle(40).seed(111).tap("RETURN").after_menu_select();
                let b = b.taps(&["F", "DOWN"]).hold("RIGHT", 20).idle(5).tap("ESC").after_esc();
                let b = b.idle(20).tap("RETURN").after_menu_select();
                let b = b.idle(10).taps(&["R", "UP"]).taps(&["R", "UP"]).taps(&["LCTRL", "RCTRL"]);
                let b = play(b, 120).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "reuse",
            setup: None,
            script: {
                let b = B::new(d, 3).idle(40).seed(201).tap("RETURN").after_menu_select();
                let b = play(both_done(b), 120).tap("ESC").after_esc();
                let b = b.idle(20).tap("DOWN").seed(202).tap("RETURN").after_menu_select();
                let b = b.idle(20).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case {
            name: "regenerate",
            setup: Some(REGENERATE_SETUP),
            script: {
                let b = B::new(Some(&setup_name("regenerate")), 4).idle(40).seed(301).tap("RETURN").after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                let b = b.idle(10).tap("DOWN").seed(302).tap("RETURN").after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
        Case { name: "game_over", setup: Some(GAME_OVER_SETUP), script: game_over_script(game_over_seed) },
        Case {
            name: "f1_quit",
            setup: None,
            script: {
                let b = B::new(d, 5).idle(30).seed(401).tap("F1").after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                let b = b.idle(10).tap("F1").after_menu_select();
                let b = b.idle(10).tap("ESC").after_esc();
                b.tap("ESC").idle(5).tap("RETURN").end(40, true)
            },
        },
    ]
}

/// The case's `Settings`: `Settings::default()` or the committed sidecar via `settings_from_toml`.
pub fn settings_for(script: &ShellScript) -> Settings {
    match &script.setup {
        None => Settings::default(),
        Some(f) => settings_from_toml(&std::fs::read_to_string(Path::new(GOLDEN).join(f)).unwrap()).unwrap(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pub new_games: u32,
    pub resumes: u32,
    pub menus: u32,
    pub game_over_menu: bool,
    pub quit: bool,
    pub black: u32,
    pub pops: u32,
    pub violations: Vec<String>,
}

pub struct Run {
    pub boot: String,
    pub lines: Vec<String>,
    pub end: String,
    pub ledger: Ledger,
    /// (frame, the surface, its presented fade) for frames in `keep`.
    pub shots: Vec<(u32, Bitmap, i32)>,
}

fn upd_char(p: Phase) -> char {
    match p {
        Phase::Menu => 'M',
        Phase::Weapsel => 'W',
        Phase::Game => 'G',
        Phase::Quit => '-',
    }
}

fn present_fields(sh: &Shell, p: Option<Present>) -> (String, Option<i32>) {
    match p {
        None => ("0 -".into(), None),
        Some(Present::Frame { fade }) => (format!("1 {:016x}", hash_frame(sh.surface(), fade)), Some(fade)),
        Some(Present::Black) => (format!("1 {:016x}", hash_frame(sh.surface(), 0)), Some(0)),
    }
}

fn tail(sh: &Shell) -> String {
    format!(
        "{:016x} {} {} {} {}",
        hash_frame(sh.surface(), 33),
        sh.fade(),
        sh.menu_cycles(),
        sh.top_char(),
        sh.main_selection()
    )
}

/// The sampled words from the held DOS keys through the settings' bindings (design §6.5): bit
/// `c` iff `controls_ex[c]` is held; DIG (`controls_ex[7]`) presses Left + Right.
fn words(held: &BTreeSet<u32>, s: &Settings) -> [ControlState; 2] {
    [0, 1].map(|i| {
        let ex = &s.worm_settings[i].controls_ex;
        let on = |k: u32| k != 0 && held.contains(&k);
        let mut cs = ControlState::new();
        for c in 0..7 {
            cs.set(c as u32, on(ex[c]));
        }
        if on(ex[7]) {
            cs.set(ControlState::LEFT, true);
            cs.set(ControlState::RIGHT, true);
        }
        cs
    })
}

/// Drive `script` through `ui::shell::Shell`: the golden-format lines, the ledger and the
/// validators' findings (plan Task 9). `keep` = an inclusive frame range whose surfaces to keep.
pub fn drive(script: &ShellScript, keep: Option<(u32, u32)>) -> Run {
    let settings = settings_for(script);
    let select = UiTc::load(Path::new(TC_ROOT)).hooks.select;
    let seeds = SeedSource::Scripted { boot: script.boot_seed, matches: script.match_seeds.iter().copied().collect() };
    let (mut sh, mut sim, out) = Shell::boot(Path::new(TC_ROOT), settings.clone(), seeds, 0, StartOptions::default());
    let (p, _) = present_fields(&sh, out.present);
    let mut run = Run { boot: format!("boot {p} {}", tail(&sh)), lines: Vec::new(), end: String::new(), ledger: Ledger::default(), shots: Vec::new() };
    let mut held: BTreeSet<u32> = BTreeSet::new();
    let v = |run: &mut Run, frame: u32, what: String| run.ledger.violations.push(format!("frame {frame}: {what}"));
    for frame in 0..script.frames {
        let mut seen = BTreeSet::new();
        let mut events = Vec::new();
        for k in script.keys.iter().filter(|k| k.frame == frame) {
            if !ALLOWED.contains(&k.name.as_str()) {
                v(&mut run, frame, format!("key {} is not menu-safe (plan-time fact 2)", k.name));
            }
            let (dos, typed) = key_of(&k.name);
            if !seen.insert(dos) {
                v(&mut run, frame, format!("two events for {} in one frame", k.name));
            }
            match k.kind {
                Kind::Down if !held.insert(dos) => v(&mut run, frame, format!("{} down while held", k.name)),
                Kind::Repeat if !held.contains(&dos) => v(&mut run, frame, format!("{} repeat while up", k.name)),
                Kind::Up if !held.remove(&dos) => v(&mut run, frame, format!("{} up while up", k.name)),
                _ => {}
            }
            events.push(KeyEvent { dos, down: k.kind != Kind::Up, repeat: k.kind == Kind::Repeat, typed });
        }
        let sampled = words(&held, &settings);
        let input = ShellInput { events: &events, sampled, fresh_seed: 0, now_ms: u64::from(frame) * 14, restart: false };
        let out = sh.frame(&mut sim, &input);
        if out.upd == Phase::Menu && out.menu_sounds.contains(&select) && !sh.menu_fading() && out.routed.is_none() {
            v(&mut run, frame, "a placeholder was selected (the C++ menu acts on it)".into());
        }
        match out.routed {
            Some(Route::NewGame { .. }) => {
                run.ledger.new_games += 1;
                run.ledger.pops += 1;
                if sim.rand.draws() != 0 {
                    v(&mut run, frame, "the selection constructor drew the RNG (intervention 3)".into());
                }
                if settings.game_mode == GM_HOLDAZONE {
                    v(&mut run, frame, "a Holdazone match (unported)".into());
                }
            }
            Some(Route::Resume) => {
                run.ledger.resumes += 1;
                run.ledger.pops += 1;
            }
            Some(Route::Menu) => {
                run.ledger.menus += 1;
                run.ledger.pops += 1;
                if sampled.iter().any(|c| c.pack() != 0) {
                    v(&mut run, frame, "a worm key is held at the back-to-menu pop (plan-time fact 12)".into());
                }
                if !sh.main_menu().items[0].visible {
                    run.ledger.game_over_menu = true;
                }
            }
            Some(Route::Quit) => run.ledger.quit = true,
            None => {}
        }
        if out.present == Some(Present::Black) {
            run.ledger.black += 1;
        }
        let (p, fade) = present_fields(&sh, out.present);
        let sounds: Vec<String> = out.menu_sounds.iter().map(i32::to_string).collect();
        let sounds = if sounds.is_empty() { "-".to_string() } else { sounds.join(",") };
        run.lines.push(format!("f {frame} {} {p} {} {sounds}", upd_char(out.upd), tail(&sh)));
        if let (Some((a, b)), Some(f)) = (keep, fade) {
            if (a..=b).contains(&frame) {
                run.shots.push((frame, sh.surface().clone(), f));
            }
        }
        if out.quit {
            if script.keys.iter().any(|k| k.frame > frame) {
                v(&mut run, frame, "key events after the quit".into());
            }
            run.end = format!("end {frame} quit");
            break;
        }
    }
    if run.end.is_empty() {
        run.end = format!("end {} frames", script.frames);
    }
    if run.ledger.new_games as usize != script.match_seeds.len() {
        v(&mut run, script.frames, format!("{} NEW GAMEs for {} match seeds", run.ledger.new_games, script.match_seeds.len()));
    }
    if run.ledger.quit != script.expect_quit {
        v(&mut run, script.frames, "the expect line disagrees with the run".into());
    }
    run
}

/// The first match seed for which `game_over_script` ends the match (lives 1, health 1) and pops
/// back to a RESUME-less menu before the scripted Esc at frame 700, with no violation.
pub fn find_game_over_seed() -> u32 {
    for seed in 1..=64 {
        let run = drive(&game_over_script(seed), None);
        // `f <frame> <upd> <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> ...`: 0-based 8.
        let back_by_700 = run.lines.get(700).is_some_and(|l| l.split_whitespace().nth(8) == Some("M"));
        if run.ledger.violations.is_empty() && run.ledger.game_over_menu && back_by_700 {
            return seed;
        }
    }
    panic!("no game_over seed in 1..=64: make P1 fire more (plan Task 9)");
}

/// The committed script of `name`.
pub fn read_script(name: &str) -> ShellScript {
    ShellScript::parse(&std::fs::read_to_string(Path::new(GOLDEN).join(format!("shell_{name}_script.txt"))).unwrap())
}
```

- [ ] **Step 2: The generator**

Create `rust/oracle-tests/examples/gen_slice4_5d.rs`:

```rust
//! Step 4½d T9 — the G2 shell corpus writer (design §6.4). A dev tool, not a test; not run in CI.
//! Run in a DEBUG build.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5d -- check
//!   cargo run -p oracle-tests --example gen_slice4_5d -- write <golden dir>
//!
//! Both drive every case through the REAL Rust shell and refuse a case with any violation
//! (plan Task 9); `write` then writes the 11 `shell_<case>_script.txt` + the 5 `_setup.cfg`.
//! The goldens are C++'s: gen_shell_golden.sh.

#[path = "../tests/shell_common/mod.rs"]
mod sc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first().map(String::as_str) {
        Some("check") => None,
        Some("write") => Some(std::path::PathBuf::from(args.get(1).expect("write <golden dir>"))),
        _ => panic!("usage: gen_slice4_5d check | write <golden dir>"),
    };
    // Setups first: `drive` reads them from the golden dir.
    let seed = {
        if let Some(d) = &dir {
            for c in sc::cases(1) {
                if let Some(text) = c.setup {
                    std::fs::write(d.join(format!("shell_{}_setup.cfg", c.name)), text).unwrap();
                }
            }
        }
        sc::find_game_over_seed()
    };
    println!("game_over match seed: {seed}");
    let mut bad = Vec::new();
    for c in sc::cases(seed) {
        let run = sc::drive(&c.script, None);
        let l = &run.ledger;
        let summary = format!(
            "{} frames, end `{}`, NEW GAME {}, RESUME {}, back-to-menu {} (game over: {}), black presents {}",
            run.lines.len(), run.end, l.new_games, l.resumes, l.menus, l.game_over_menu, l.black
        );
        println!("{:<15} {summary}", c.name);
        for v in &l.violations {
            println!("  VIOLATION {v}");
            bad.push(format!("{}: {v}", c.name));
        }
        if let Some(d) = &dir {
            let header = vec![
                format!("Step 4½d G2 case {} (design §6.4; plan Task 9) — written by gen_slice4_5d.", c.name),
                format!("Rust ledger: {summary}"),
            ];
            std::fs::write(d.join(format!("shell_{}_script.txt", c.name)), c.script.to_text(&header)).unwrap();
        }
    }
    assert!(bad.is_empty(), "the corpus has violations: {bad:?}");
    if let Some(d) = dir {
        println!("wrote 11 cases to {}", d.display());
    }
}
```

`find_game_over_seed` reads the `game_over` setup from the golden dir, so `check` needs the setups written once. On a first `check` in a clean tree it panics reading the file. Run `write` first; `write` writes the setups before the search.

Run: `cd /home/user/openliero/rust && cargo run -p oracle-tests --example gen_slice4_5d -- write /home/user/openliero/rust/oracle-tests/golden`
Expected:
- `game_over match seed: <s>`;
- 11 summary lines and no `VIOLATION`, with:
  - `milestone`: NEW GAME 2, RESUME 1, back-to-menu 3, `end … quit`;
  - `esc_in_weapsel`: NEW GAME 1, RESUME 1, back-to-menu 2;
  - `reuse` and `regenerate`: NEW GAME 2;
  - `game_over`: game over true;
  - `f1_quit`: NEW GAME 1, RESUME 1;
  - the four boot cases: `end … frames` with 0 NEW GAMEs;
- then `wrote 11 cases`.

If `find_game_over_seed` panics, strengthen `game_over_script` (more `LCTRL` taps, or P2 firing at P1) and re-run. Record the change.

Run it again with `check` — Expected: the same lines, exit 0.

- [ ] **Step 3: `gen_shell_golden.sh` and the C++ goldens**

Create `rust/oracle-tests/gen_shell_golden.sh` (`chmod +x`):

```bash
#!/usr/bin/env bash
# Regenerates golden/shell_<case>.txt — the Step-4½d G2 shell gate and the MILESTONE (design
# §6.2-§6.6): oracle_dump_shell runs the REAL C++ Gfx::RunOneFrame headlessly over every committed
# golden/shell_<case>_script.txt (+ its _setup.cfg, read by the REAL Settings::FromToml). The
# scripts are written by `cargo run -p oracle-tests --example gen_slice4_5d -- write <golden dir>`
# and never touched here. Needs the full C++ build (links the `game` target), so this is a
# LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET for other platforms
# (e.g. linux-x64). cd's to ROOT, so any cwd works. SHELL_PPM_DIR=<dir> also writes every
# presented frame as <dir>/<case>/f_NNNN.ppm (eyeballing only; never committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_shell
n=0
for scn in rust/oracle-tests/golden/shell_*_script.txt; do
  c="$(basename "$scn" _script.txt)"
  out="rust/oracle-tests/golden/${c}.txt"
  ppm=()
  if [ -n "${SHELL_PPM_DIR:-}" ]; then
    mkdir -p "$SHELL_PPM_DIR/$c"
    ppm=(--ppm-dir "$SHELL_PPM_DIR/$c")
  fi
  "build/$PRESET/Release/oracle_dump_shell" "$scn" "$out" "${ppm[@]}"
  expect=$(awk '$1 == "expect" { print $2 }' "$scn")
  # C++-SIDE GATE (design §6.4): one 8-field boot line presenting once; 11-field f lines in frame
  # order with presents in {0,1}, '-' exactly when nothing was presented, upd in {M,W,G}, top in
  # {M,G,-}; one end line agreeing with `expect`.
  awk -v c="$c" -v expect="$expect" '
    BEGIN { k = 0; boots = 0; ends = 0 }
    /^#/ { next }
    $1 == "boot" {
      if (NF != 8 || $2 != "1" || boots++ || k) { printf "FAIL %s: bad boot line\n", c; exit 1 }
      next
    }
    $1 == "f" {
      if (NF != 11) { printf "FAIL %s: f line with %d fields\n", c, NF; exit 1 }
      if ($2 != k) { printf "FAIL %s: frame %s out of order (want %d)\n", c, $2, k; exit 1 }
      if ($3 !~ /^[MWG]$/ || $9 !~ /^[MG-]$/) { printf "FAIL %s: bad upd/top on frame %s\n", c, $2; exit 1 }
      if ($4 != "0" && $4 != "1") { printf "FAIL %s: presents %s\n", c, $4; exit 1 }
      if (($4 == "0") != ($5 == "-")) { printf "FAIL %s: presented vs presents on frame %s\n", c, $2; exit 1 }
      k++; next
    }
    $1 == "end" {
      if (ends++ || $3 != expect) { printf "FAIL %s: end %s, expected %s\n", c, $3, expect; exit 1 }
      next
    }
    { printf "FAIL %s: unexpected line: %s\n", c, $0; exit 1 }
    END {
      if (!boots || !ends) { printf "FAIL %s: missing boot or end\n", c; exit 1 }
      printf "  gate %s: boot + %d frames, end %s\n", c, k, expect
    }
  ' "$out"
  echo "wrote $out"
  n=$((n + 1))
done
test "$n" -eq 11 || { echo "FAIL: $n shell scripts (want 11)"; exit 1; }
```

The `f` fields in awk are: `$1` f, `$2` frame, `$3` upd, `$4` presents, `$5` presented16, `$6` bmp16, `$7` fade, `$8` menu_cycles, `$9` top, `$10` sel, `$11` sounds.

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && SHELL_PPM_DIR=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/g2/ppm bash /home/user/openliero/rust/oracle-tests/gen_shell_golden.sh`
Expected: 11 `gate` lines and 11 `wrote` lines; the dumper refuses nothing. A dumper refusal ("drew the RNG", "Holdazone match", "unused match seeds", "expected to end by") is a corpus bug the Rust validators missed. Fix the validator first, then the case, and re-run both scripts.

Run it a second time and check the goldens are byte-identical: `git -C /home/user/openliero add rust/oracle-tests/golden/shell_*` after the first run, then `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` shows no `M` after the second.

Eyeball (Read tool, after `ppm2png.py`): `milestone`'s first menu frame, a selection frame, a play frame, the pause menu (RESUME GAME (F1) over the game with the copyright bar), and the RESUMEd selection in `esc_in_weapsel` (with the bar, finding 11).

- [ ] **Step 4: Commit**

```
git -C /home/user/openliero add rust/oracle-tests/tests/shell_common rust/oracle-tests/examples/gen_slice4_5d.rs rust/oracle-tests/gen_shell_golden.sh rust/oracle-tests/golden/shell_*
git -C /home/user/openliero commit -m "oracle(4.5d): G2 — 11 shell cases (generator-validated) and their C++ goldens from the real headless Gfx frame loop" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 10: 🎯 MILESTONE — every G2 case bit-exact against the real C++ frame loop (`shell_golden.rs`)  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/shell_golden.rs`

**Interfaces:**
- Consumes: T9's `shell_common` (`read_script`, `drive`, `cases`, `CASES_NAMES`) and the 11 committed C++ goldens; T0's `render::present::fade_argb` (for mismatch PPMs).
- Produces: design done-when 2 (**G2**) and 3 (**🎯 the milestone**) as ordinary `oracle-tests` tests, which run in CI's `cargo test --workspace --exclude game`.

Why: design §6.5–§6.6. For every frame of every case, the Rust shell must match the real `Gfx::RunOneFrame` on:
- `presents` and the presented (faded) frame;
- the unfaded back buffer;
- `fade`, `menu_cycles`, the top screen and the main-menu cursor;
- the sounds, strictly on frames the menu or weapon selection updated (`upd` `M`/`W`).

A match frame's sim sounds belong to 4c's audio parity, not to this gate (design §6.5). `shell_milestone` is the done-when path, and it is named on its own.

- [ ] **Step 1: Write the test**

Create `rust/oracle-tests/tests/shell_golden.rs`:

```rust
//! Step 4½d — 🎯 THE MILESTONE (design §6.6, done-when 2-3). Every G2 case — boot → main menu →
//! NEW GAME → weapon selection → play → Esc → menu (RESUME / NEW GAME) → … → QUIT, and the
//! corners around it — replayed through `ui::shell::Shell` matches the REAL C++
//! `Gfx::RunOneFrame` (`oracle_dump_shell`, `gen_shell_golden.sh`) on EVERY frame: presents, the
//! presented (faded) frame, the back buffer, fade, menu_cycles, the top screen, the main-menu
//! cursor, and the sounds of every frame the menu or weapon selection updated (`upd` M/W; a match
//! frame's sim sounds are 4c's parity, design §6.5). `SHELL_RUST_PPM_DIR=<dir>` writes the Rust
//! presented frames around a mismatch as `<dir>/shell_<case>/f_NNNN.ppm` (the dumper's
//! --ppm-dir names).

mod shell_common;

use render::present::fade_argb;
use shell_common as sc;

const FIELDS: [&str; 11] = [
    "f", "frame", "upd", "presents", "presented16", "bmp16", "fade", "menu_cycles", "top", "sel",
    "sounds",
];

struct Golden {
    boot: String,
    frames: Vec<String>,
    end: String,
}

fn golden(name: &str) -> Golden {
    let text = std::fs::read_to_string(format!("{}/shell_{name}.txt", sc::GOLDEN)).unwrap();
    let mut g = Golden { boot: String::new(), frames: Vec::new(), end: String::new() };
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        match l.split_whitespace().next() {
            Some("boot") => g.boot = l.to_string(),
            Some("f") => g.frames.push(l.to_string()),
            Some("end") => g.end = l.to_string(),
            _ => panic!("{name}: unexpected golden line {l}"),
        }
    }
    g
}

/// `Err((frame, message))` at the first difference.
fn compare(name: &str, want: &Golden, run: &sc::Run) -> Result<(), (u32, String)> {
    if run.boot != want.boot {
        return Err((0, format!("{name}: G2 boot line differs\n  C++:  {}\n  Rust: {}", want.boot, run.boot)));
    }
    for (i, (g, w)) in run.lines.iter().zip(&want.frames).enumerate() {
        let (gf, wf): (Vec<&str>, Vec<&str>) = (g.split_whitespace().collect(), w.split_whitespace().collect());
        for k in 0..FIELDS.len() {
            if k == 10 && wf[2] == "G" {
                continue; // a match frame's sim sounds (design §6.5)
            }
            if gf.get(k) != wf.get(k) {
                return Err((
                    i as u32,
                    format!("{name}: G2 line {i} (frame {}), column `{}` differs\n  C++:  {w}\n  Rust: {g}", wf[1], FIELDS[k]),
                ));
            }
        }
    }
    if run.lines.len() != want.frames.len() || run.end != want.end {
        let n = run.lines.len().min(want.frames.len()) as u32;
        return Err((n, format!("{name}: G2 line count / end: C++ {} `{}`, Rust {} `{}`", want.frames.len(), want.end, run.lines.len(), run.end)));
    }
    Ok(())
}

fn check(name: &str) {
    let script = sc::read_script(name);
    let want = golden(name);
    let run = sc::drive(&script, None);
    assert!(run.ledger.violations.is_empty(), "{name}: {:?}", run.ledger.violations);
    if let Err((frame, msg)) = compare(name, &want, &run) {
        if let Ok(dir) = std::env::var("SHELL_RUST_PPM_DIR") {
            let dir = std::path::Path::new(&dir).join(format!("shell_{name}"));
            std::fs::create_dir_all(&dir).unwrap();
            let keep = sc::drive(&script, Some((frame.saturating_sub(3), frame + 1)));
            for (f, bmp, fade) in &keep.shots {
                let mut data = format!("P6\n{} {}\n255\n", bmp.w, bmp.h).into_bytes();
                for p in &bmp.pixels {
                    let c = fade_argb(*p, *fade);
                    data.extend([(c >> 16) as u8, (c >> 8) as u8, c as u8]);
                }
                std::fs::write(dir.join(format!("f_{f:04}.ppm")), data).unwrap();
            }
        }
        panic!("{msg}");
    }
}

#[test]
fn the_milestone_is_bit_exact_on_every_frame() {
    // design §6.6: boot → menu → NEW GAME → selection → play → Esc → menu (RESUME GAME (F1) /
    // NEW GAME) → RESUME → play → Esc → NEW GAME (level reused) → selection → Esc → menu → QUIT.
    let l = sc::drive(&sc::read_script("milestone"), None).ledger;
    assert_eq!((l.new_games, l.resumes, l.menus, l.quit), (2, 1, 3, true), "the milestone path");
    check("milestone");
}

#[test]
fn every_g2_case_is_bit_exact() {
    for name in sc::CASES_NAMES.iter().filter(|n| **n != "milestone") {
        check(name);
    }
}

#[test]
fn the_committed_scripts_are_the_generators() {
    let seed = sc::read_script("game_over").match_seeds[0];
    for c in sc::cases(seed) {
        assert_eq!(sc::read_script(c.name), c.script, "{}: regenerate with gen_slice4_5d", c.name);
    }
    let mut on_disk: Vec<String> = std::fs::read_dir(sc::GOLDEN)
        .unwrap()
        .filter_map(|e| {
            let f = e.unwrap().file_name().to_string_lossy().into_owned();
            Some(f.strip_prefix("shell_")?.strip_suffix("_script.txt")?.to_string())
        })
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = sc::CASES_NAMES.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(on_disk, want);
}

#[test]
#[should_panic(expected = "G2 line")]
fn the_gate_sees_a_one_pixel_change() {
    let mut want = golden("boot_idle");
    let mut f: Vec<String> = want.frames[5].split_whitespace().map(str::to_string).collect();
    f[5] = format!("{:016x}", u64::from_str_radix(&f[5], 16).unwrap() ^ 1);
    want.frames[5] = f.join(" ");
    let run = sc::drive(&sc::read_script("boot_idle"), None);
    if let Err((_, msg)) = compare("boot_idle", &want, &run) {
        panic!("{msg}");
    }
}
```

- [ ] **Step 2: Run it: this is the milestone**

Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test shell_golden 2>&1 | tail -20`
Expected: 4 passed. On a failure, the message names the case, the frame and the first differing column. Diagnose against the C++ source, never against the golden:

| Column | Where to look |
|---|---|
| `boot` line | `Shell::boot` / `draw_boot` / `MainMenuState::enter` (the copyright bar's palette is the boot game's, finding 12) |
| `presents` | the pop frames (finding 7): only the back-to-menu pop presents (Black) |
| `presented16` with `bmp16` equal | the fade: menu frames use the menu fade, Playing frames `MatchFlow`'s |
| `bmp16` on `M` frames | `MainMenuState::draw`, the settings display (G1 should already agree), `menu_palette` |
| `bmp16` on `W`/`G` frames | `menu_cycles` threading (finding 8), the shared frozen screen (11), "one draw per tick" (12) |
| `fade` | `update_menu_palettes(fading)` with `fading` captured before update (finding 14); the Esc tail (9) |
| `menu_cycles` | menu frames count before the draw, Playing frames after it, pop frames not at all, `Enter` resets |
| `sel` | the key order in `MainMenuState::update`; Up \|\| R (finding 15); `SetVisibility` at `Enter` |
| `sounds` | the crossed menu sounds; the placeholder never selects; `SoundBegin` on DONE |

Set `SHELL_RUST_PPM_DIR` and `SHELL_PPM_DIR` (T9's gen script) to scratchpad dirs, convert both PPMs of the failing frame with `ppm2png.py`, and look at them side by side. Fix the Rust and re-run. **Never regenerate a golden to match Rust.** If the evidence points at a dumper intervention hiding or causing a difference, stop and report it (Addendum A).

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game 2>&1 | grep -E 'test result' | awk '{s+=$4; f+=$6} END {print s " passed, " f " failed"}'` — Expected: `0 failed`.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 3: Commit**

```
git -C /home/user/openliero add rust/oracle-tests/tests/shell_golden.rs
git -C /home/user/openliero commit -m "oracle(4.5d): 🎯 MILESTONE — every G2 case (milestone included) bit-exact against the real C++ Gfx frame loop" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

If Step 2 needed Rust fixes in `ui`/`render`, commit them first with their own message (`ui(4.5d): fix … found by the G2 gate (frame N of <case>: <column>)`, same trailers), then commit the test.

---
### Task 11: `game` — the live shell: key events, touch edges + MENU, the faded present, QUIT, `?menu=`, F5, the page; `shot --menu`  [Opus]

**Files:**
- Modify: `rust/game/src/input.rs` (`dos_of_keycode`, `typed_of_keycode`, `KeyQueue`, tests)
- Modify: `rust/game/src/touch.rs` (`TOUCH_MENU`, `touch_key_events`, tests)
- Modify: `rust/game/src/web_params.rs` (`menu`, `skips_menu`, tests; the module doc)
- Modify: `rust/game/src/main.rs` (the shell path; drop `NewGame`)
- Modify: `rust/ui/src/shell/new_game.rs` (delete `NewGame`, port its tests), `rust/game/src/hud_mode.rs` (port the test that used `NewGame`)
- Modify: `web/index.html`, `.github/workflows/preview.yml`
- Modify: `rust/shot/Cargo.toml` (`ui = { path = "../ui" }`), `rust/shot/src/lib.rs` (`--menu [--frames N] [--seed S]`)

**Interfaces:**
- Produces:
  - `game::input::{dos_of_keycode(KeyCode) -> u32, typed_of_keycode(KeyCode) -> TypedKey, KeyQueue}`;
  - `game::touch::{TOUCH_MENU, touch_key_events(prev, now, &[u32; 8]) -> Vec<KeyEvent>}`;
  - `MatchParams::{menu, skips_menu()}`;
  - `shot::render_menu`;
  - the page globals `window.lieroPhase ∈ {"menu", "weapsel", "game", "quit"}` (already there for `weapsel`/`game`) and `window.lieroTouch`'s bit 8 (MENU).
- Consumes: T7's `Shell` and its types; T0's `render::present::fade_into_rgba`.

Why: design §7 and done-when 4–5. A bare `cargo run -p game` and a bare preview URL boot to the main menu. Keyboard (§7.2) and touch (§7.3) drive it. QUIT exits natively and stops on the black frame in the browser (Q3). The preview parameters skip the menu (Q4), and `?menu=1` forces it. F5 is a Rust-only restart (Q5).

`?demo`, Scripted, `--replay`, `--live --record` and `--live [<scenario>]` stay exactly as they are (plan-time fact 1). `tick_and_render` stays the only system with `ResMut<Sim>` (LD 3): the shell path is a plain function it calls.

- [ ] **Step 1: Write the failing unit tests**

(a) `rust/game/src/input.rs`, inside `mod tests`:

```rust
    #[test]
    fn dos_of_keycode_is_the_keys_cpp_table() {
        // keys.cpp:9-75 (the index of each SDL scancode; unknown -> 89; PRINTSCREEN's second
        // entry wins in sdl_to_dos_scan_codes).
        for (k, dos) in [
            (KeyCode::Escape, 1), (KeyCode::Digit1, 2), (KeyCode::Digit0, 11), (KeyCode::KeyR, 19),
            (KeyCode::Enter, 28), (KeyCode::ControlLeft, 29), (KeyCode::KeyF, 33),
            (KeyCode::ShiftLeft, 42), (KeyCode::ShiftRight, 54), (KeyCode::AltLeft, 56),
            (KeyCode::Space, 57), (KeyCode::F1, 59), (KeyCode::F10, 68), (KeyCode::F11, 87),
            (KeyCode::NumpadEnter, 116), (KeyCode::ControlRight, 117), (KeyCode::PrintScreen, 143),
            (KeyCode::AltRight, 144), (KeyCode::ArrowUp, 160), (KeyCode::PageUp, 161),
            (KeyCode::ArrowLeft, 163), (KeyCode::ArrowRight, 165), (KeyCode::ArrowDown, 168),
            (KeyCode::PageDown, 169), (KeyCode::Delete, 171), (KeyCode::SuperLeft, 89),
        ] {
            assert_eq!(dos_of_keycode(k), dos, "{k:?}");
        }
    }

    #[test]
    fn the_default_bindings_are_the_settings_dos_keys() {
        // The sim samples KeyCodes, the menus test DOS keys (design §4.6): they must agree.
        let s = scenario::settings::Settings::default();
        for (p, b) in default_bindings().players.iter().enumerate() {
            let keys = [b.up, b.down, b.left, b.right, b.fire, b.change, b.jump];
            for (c, k) in keys.iter().enumerate() {
                assert_eq!(dos_of_keycode(*k), s.worm_settings[p].controls_ex[c], "player {p} control {c}");
            }
        }
    }

    #[test]
    fn typed_keys_are_the_unshifted_us_symbols() {
        use ui::keys::TypedKey;
        assert_eq!(typed_of_keycode(KeyCode::KeyA), TypedKey::Sym(b'a' as u32));
        assert_eq!(typed_of_keycode(KeyCode::Digit7), TypedKey::Sym(b'7' as u32));
        assert_eq!(typed_of_keycode(KeyCode::Space), TypedKey::Sym(32));
        assert_eq!(typed_of_keycode(KeyCode::Minus), TypedKey::Sym(b'-' as u32));
        assert_eq!(typed_of_keycode(KeyCode::Tab), TypedKey::Tab);
        assert_eq!(typed_of_keycode(KeyCode::ArrowUp), TypedKey::Sym(0), "not 32..=127: ignored");
    }

    #[test]
    fn a_tick_takes_the_queue_but_defers_a_keys_second_event() {
        use ui::shell::KeyEvent;
        let e = |dos, down| KeyEvent { dos, down, repeat: false, typed: ui::keys::TypedKey::Sym(0) };
        let mut q = KeyQueue::default();
        for ev in [e(28, true), e(160, true), e(28, false), e(160, false), e(1, true)] {
            q.push(ev);
        }
        assert_eq!(q.take_tick(), vec![e(28, true), e(160, true)], "RETURN's up waits (plan-time fact 19)");
        assert_eq!(q.take_tick(), vec![e(28, false), e(160, false), e(1, true)]);
        assert!(q.take_tick().is_empty());
    }
```

(b) `rust/game/src/touch.rs`, inside `mod tests`:

```rust
    #[test]
    fn touch_edges_are_key_events_on_player_ones_dos_keys() {
        // design §7.3: a rising edge is a key-down of controls_ex[bit], a falling edge a key-up;
        // DIG presses Left and Right; MENU is Esc; no repeats.
        use ui::keys::DK_ESCAPE;
        let ex = scenario::settings::Settings::default().worm_settings[0].controls_ex;
        let down = |e: &ui::shell::KeyEvent| (e.dos, e.down);
        let evs = touch_key_events(0, TOUCH_UP | TOUCH_FIRE, &ex);
        assert_eq!(evs.iter().map(down).collect::<Vec<_>>(), [(ex[0], true), (ex[4], true)]);
        assert!(evs.iter().all(|e| !e.repeat));
        assert!(touch_key_events(TOUCH_UP, TOUCH_UP, &ex).is_empty(), "held: no events");
        assert_eq!(touch_key_events(TOUCH_UP, 0, &ex).iter().map(down).collect::<Vec<_>>(), [(ex[0], false)]);
        assert_eq!(touch_key_events(0, TOUCH_DIG, &ex).iter().map(down).collect::<Vec<_>>(), [(ex[2], true), (ex[3], true)]);
        assert_eq!(touch_key_events(0, TOUCH_MENU, &ex).iter().map(down).collect::<Vec<_>>(), [(DK_ESCAPE, true)]);
        assert_eq!(touch_state(TOUCH_MENU), ControlState::new(), "MENU never reaches the sim");
    }
```

(c) `rust/game/src/web_params.rs`, inside `mod tests`:

```rust
    #[test]
    fn match_parameters_skip_the_menu_unless_menu_is_forced() {
        // Q4 (John, 2026-09-26): a plain link opens the menu; weapons/level/seed skip it.
        assert!(!MatchParams::parse("").skips_menu());
        assert!(!MatchParams::parse("?touch=1").skips_menu());
        for q in ["?weapons=BAZOOKA", "?level=water_stage", "?seed=7"] {
            assert!(MatchParams::parse(q).skips_menu(), "{q}");
            let forced = MatchParams::parse(&format!("{q}&menu=1"));
            assert!(forced.menu && !forced.skips_menu(), "{q}&menu=1");
        }
        assert!(MatchParams::parse("?menu").menu);
    }
```

Run: `cd /home/user/openliero/rust && cargo test -p game --lib 2>&1 | tail -6` — Expected: FAIL to compile, with `cannot find function 'dos_of_keycode'`, `'typed_of_keycode'`, `'touch_key_events'`, `KeyQueue`, `TOUCH_MENU`, `skips_menu` and field `menu`.

- [ ] **Step 2: Implement the Bevy-free glue**

(1) `rust/game/src/input.rs` (add `use std::collections::VecDeque;` and `use ui::keys::TypedKey; use ui::shell::KeyEvent;`):

```rust
/// `SDLToDOSKey` (`keys.cpp:9-75`) over Bevy's physical `KeyCode`: the index of the key in C++'s
/// `liero_to_sdl_keys`; a key C++ does not know is 89 (Step 4½d, design §4.1).
pub fn dos_of_keycode(k: KeyCode) -> u32 {
    use KeyCode::*;
    match k {
        Escape => 1,
        Digit1 => 2, Digit2 => 3, Digit3 => 4, Digit4 => 5, Digit5 => 6, Digit6 => 7,
        Digit7 => 8, Digit8 => 9, Digit9 => 10, Digit0 => 11,
        Minus => 12, Equal => 13, Backspace => 14, Tab => 15,
        KeyQ => 16, KeyW => 17, KeyE => 18, KeyR => 19, KeyT => 20, KeyY => 21, KeyU => 22,
        KeyI => 23, KeyO => 24, KeyP => 25, BracketLeft => 26, BracketRight => 27, Enter => 28,
        ControlLeft => 29, KeyA => 30, KeyS => 31, KeyD => 32, KeyF => 33, KeyG => 34, KeyH => 35,
        KeyJ => 36, KeyK => 37, KeyL => 38, Semicolon => 39, Quote => 40, Backquote => 41,
        ShiftLeft => 42, Backslash => 43, KeyZ => 44, KeyX => 45, KeyC => 46, KeyV => 47,
        KeyB => 48, KeyN => 49, KeyM => 50, Comma => 51, Period => 52, Slash => 53,
        ShiftRight => 54, NumpadMultiply => 55, AltLeft => 56, Space => 57, CapsLock => 58,
        F1 => 59, F2 => 60, F3 => 61, F4 => 62, F5 => 63, F6 => 64, F7 => 65, F8 => 66, F9 => 67,
        F10 => 68, NumLock => 69, ScrollLock => 70, Numpad7 => 71, Numpad8 => 72, Numpad9 => 73,
        NumpadSubtract => 74, Numpad4 => 75, Numpad5 => 76, Numpad6 => 77, NumpadAdd => 78,
        Numpad1 => 79, Numpad2 => 80, Numpad3 => 81, Numpad0 => 82, NumpadDecimal => 83,
        IntlBackslash => 86, F11 => 87, F12 => 88, NumpadEnter => 116, ControlRight => 117,
        NumpadDivide => 141, PrintScreen => 143, AltRight => 144, Home => 159, ArrowUp => 160,
        PageUp => 161, ArrowLeft => 163, ArrowRight => 165, End => 167, ArrowDown => 168,
        PageDown => 169, Insert => 170, Delete => 171,
        _ => ui::keys::DK_UNKNOWN,
    }
}

/// The key's `key_buf` symbol: `SDL_GetKeyFromScancode(sc, SDL_KMOD_NONE)` on a US layout —
/// the unshifted ASCII of printable keys, Tab, and 0 (ignored by `Menu::on_keys`) for the rest.
/// A physical US table rather than Bevy's logical key: layout-independent and deterministic.
pub fn typed_of_keycode(k: KeyCode) -> TypedKey {
    use KeyCode::*;
    let c = match k {
        Tab => return TypedKey::Tab,
        KeyA => 'a', KeyB => 'b', KeyC => 'c', KeyD => 'd', KeyE => 'e', KeyF => 'f', KeyG => 'g',
        KeyH => 'h', KeyI => 'i', KeyJ => 'j', KeyK => 'k', KeyL => 'l', KeyM => 'm', KeyN => 'n',
        KeyO => 'o', KeyP => 'p', KeyQ => 'q', KeyR => 'r', KeyS => 's', KeyT => 't', KeyU => 'u',
        KeyV => 'v', KeyW => 'w', KeyX => 'x', KeyY => 'y', KeyZ => 'z',
        Digit0 => '0', Digit1 => '1', Digit2 => '2', Digit3 => '3', Digit4 => '4', Digit5 => '5',
        Digit6 => '6', Digit7 => '7', Digit8 => '8', Digit9 => '9',
        Space => ' ', Minus => '-', Equal => '=', BracketLeft => '[', BracketRight => ']',
        Semicolon => ';', Quote => '\'', Backquote => '`', Backslash => '\\', Comma => ',',
        Period => '.', Slash => '/',
        _ => return TypedKey::Sym(0),
    };
    TypedKey::Sym(c as u32)
}

/// The keyboard events waiting for the next fixed tick (design §7.1). C++ polls every pending
/// event at the top of a frame; a tick takes the queue in order, except that a key's second
/// event waits for the next tick (plan-time fact 19: a tap inside one slow browser frame would
/// otherwise set and clear the menu flag before `Update` runs).
#[derive(Resource, Default, Debug)]
pub struct KeyQueue(VecDeque<KeyEvent>);

impl KeyQueue {
    pub fn push(&mut self, ev: KeyEvent) {
        self.0.push_back(ev);
    }

    pub fn take_tick(&mut self) -> Vec<KeyEvent> {
        let mut out: Vec<KeyEvent> = Vec::new();
        while let Some(ev) = self.0.front() {
            if out.iter().any(|e| e.dos == ev.dos) {
                break;
            }
            out.push(self.0.pop_front().expect("front exists"));
        }
        out
    }
}
```

(2) `rust/game/src/touch.rs`: extend the module doc with a Step 4½d paragraph. The page's controls also drive the menus, as key events on player 1's bound DOS keys (design §7.3). The new MENU button (bit 8) is Esc. `touch_state` ignores it, so it never reaches the sim. Then add:

```rust
/// Step 4½d (Q8): the MENU button — Esc. Ignored by [`touch_state`].
pub const TOUCH_MENU: u32 = 1 << 8;

/// The menu key events of a touch-mask change (design §7.3): each bit's rising edge is a key-down
/// of player 1's `controls_ex[bit]`, its falling edge a key-up; DIG is Left + Right; MENU is Esc.
/// Touch has no OS repeat.
pub fn touch_key_events(prev: u32, now: u32, controls_ex: &[u32; 8]) -> Vec<ui::shell::KeyEvent> {
    let mut out = Vec::new();
    let keys = |bit: u32| -> Vec<u32> {
        match bit {
            7 => vec![controls_ex[2], controls_ex[3]],
            8 => vec![ui::keys::DK_ESCAPE],
            b => vec![controls_ex[b as usize]],
        }
    };
    for bit in 0..=8u32 {
        let (was, is) = (prev >> bit & 1 != 0, now >> bit & 1 != 0);
        if was != is {
            for dos in keys(bit) {
                out.push(ui::shell::KeyEvent { dos, down: is, repeat: false, typed: ui::keys::TypedKey::Sym(0) });
            }
        }
    }
    out
}
```

(3) `rust/game/src/web_params.rs`:
- Add `pub menu: bool` to `MatchParams`, with the doc `` `menu`: force the main menu even with `weapons`/`level`/`seed` (Step 4½d, Q4). ``.
- Parse `"menu" => p.menu = value.is_empty() || value == "1" || value == "true",`.
- Add:

```rust
    /// Q4 (John, 2026-09-26): `?weapons=`, `?level=` or `?seed=` skip the main menu, so every
    /// earlier preview link behaves as before; a plain link opens the menu; `?menu=1` forces it
    /// (the parameters then configure the boot level and every NEW GAME).
    pub fn skips_menu(&self) -> bool {
        !self.menu && (!self.weapons.is_empty() || self.level.is_some() || self.seed.is_some())
    }
```

Update the module doc's first paragraphs: a bare preview opens the C++ main menu (4½d), and the parameters skip it unless `menu=1` is given.

Run: `cd /home/user/openliero/rust && cargo test -p game --lib` — Expected: PASS.

- [ ] **Step 3: Retire `NewGame`**

In `rust/ui/src/shell/new_game.rs`, delete `NewGame` and its `impl`. Keep `start_settings` and `generate_level`. Rewrite the module doc: "the C++ NEW GAME start's level generation; the NEW GAME loop is `ui::shell` (`LevelSlot`, `SeedSource`, `Match`) since 4½d".

Port or delete its tests:
- Keep `the_default_level_is_generated_from_a_rand_seeded_with_the_match_seed`, using `generate_level` directly.
- Keep `a_stock_level_file_is_loaded_not_generated`, with `generate_level(tc(), &s, 5)`.
- Keep `the_start_is_the_cpp_local_controller`, `new_match_then_selection_then_enter_game` and `skipping_selection_loads_the_saved_picks_and_enters_the_game`, over `let level = generate_level(tc(), &settings, 11); let cfg = MatchConfig { settings, seed: 11 };` and `scenario::build::{new_match, build_match, enter_game}`.
- Keep `play_tests::a_new_game_plays_and_draws_its_bonuses` in the same way (seed `3488140121`).
- Delete `a_fixed_seed_wins_over_the_fresh_one`, `the_next_new_game_reuses_the_played_level_with_a_new_seed`, `regenerate_level_makes_a_new_level_from_the_new_seed` and `a_fixed_seed_is_kept_across_new_games`: `level_slot`'s and the shell's tests cover them (T7).

In `rust/game/src/hud_mode.rs`, port `the_flags_reach_the_frame_of_the_live_default_match` to `build_match(tc, &MatchConfig { settings, seed: 42 }, &generate_level(tc, &settings_clone, 42))`.

- [ ] **Step 4: `main.rs` — the shell path**

Restructure `rust/game/src/main.rs` as follows. The non-shell paths stay byte-for-byte in behavior.

(1) **Resources.**

```rust
/// Step 4½d: the C++ frame loop (`ui::shell::Shell`) for the live default match — a bare run
/// or a bare preview. `phase` is the last `window.lieroPhase` published; `stopped` is the
/// browser's QUIT (the canvas keeps the black frame, Q3); `touch_prev` the last touch mask.
#[derive(Resource)]
struct ShellRes {
    shell: ui::shell::Shell,
    phase: ui::shell::Phase,
    stopped: bool,
    touch_prev: u32,
}
```

`game::input::KeyQueue` is the other new resource.

(2) **`setup`.** Compute `let shell_start = *mode == Mode::Live && name == game::input::DEFAULT_MATCH && record_path.0.is_none();` (the 4½c `new_game_start`, plan-time fact 1). Factor the Image, camera and sprite creation (steps 4–5) into `fn spawn_view(commands: &mut Commands, images: &mut Assets<Image>) -> Handle<Image>`. When `shell_start`:

```rust
        let settings = game::new_game::start_settings(preview.0.level_file());
        let seeds = preview.0.seed.map_or(SeedSource::Fresh, SeedSource::Fixed);
        let options = StartOptions {
            skip_selection: preview.0.skips_weapon_selection(),
            loadout: preview.0.weapons.clone(),
            touch_only: touch_only(),
        };
        let boot = if preview.0.skips_menu() { Shell::boot_playing } else { Shell::boot };
        let (shell, state, out) = boot(Path::new(TC_ROOT), settings, seeds, fresh_seed(), options);
        let mut probe = state.clone();
        apply_loadout(&mut probe, &preview.0.weapons); // report unknown names once
        let handle = spawn_view(&mut commands, &mut images);
        present(&shell, out.present, &mut images, &handle);
        publish_phase(out.phase.as_str());
        if let Some(Route::NewGame { seed }) = out.routed {
            log_new_game(seed, shell.settings());
        }
        commands.insert_resource(InputSource::Live(game::input::default_bindings()));
        commands.insert_resource(game::input::KeyQueue::default());
        commands.insert_resource(ShellRes { shell, phase: out.phase, stopped: false, touch_prev: 0 });
        commands.insert_resource(Sim(state));
        commands.insert_resource(FrameImage(handle));
        return;
```

Every other start runs the existing `setup` body unchanged, except:
- `new_game`, `new_game_start` and `start_new_game` go (only the shell starts a NEW GAME now);
- `Demo` loses its `new_game` field;
- `draw_shadow` / `level_file` come from the scenario;
- `restart_match` loses its `NewGame` arm (it always reloads the scenario).

(3) **`collect_keys`** in `PreUpdate`, after Bevy's input systems (in 0.19 the set is `bevy::input::InputSystems`; check the exact name in the bevy_input source in `~/.cargo/registry` and use it):

```rust
/// Step 4½d: the frame's keyboard events for the shell (design §7.1) — every key-down including
/// OS repeats, every key-up — in order. Only the shell path reads them.
fn collect_keys(mut reader: MessageReader<KeyboardInput>, mut queue: ResMut<game::input::KeyQueue>) {
    for ev in reader.read() {
        queue.push(ui::shell::KeyEvent {
            dos: game::input::dos_of_keycode(ev.key_code),
            down: ev.state == ButtonState::Pressed,
            repeat: ev.repeat,
            typed: game::input::typed_of_keycode(ev.key_code),
        });
    }
}
```

Register it with `.add_systems(PreUpdate, collect_keys.after(bevy::input::InputSystems).run_if(resource_exists::<game::input::KeyQueue>))`.

(4) **`tick_and_render`.** Its parameters become `demo: Option<ResMut<Demo>>` plus `shell: Option<ResMut<ShellRes>>`, `queue: Option<ResMut<game::input::KeyQueue>>`, `time: Res<Time<Real>>` and `mut exit: MessageWriter<AppExit>`. The first lines:

```rust
    if let Some(mut sh) = shell {
        let queue = queue.expect("the shell path has a key queue");
        tick_shell(&mut sim.0, &mut sh, queue.into_inner(), &source, &keys, &time, &mut images, &frame.0, &mut audio.0, &mut exit);
        return;
    }
    let mut demo = demo.expect("the non-shell paths have a Demo");
```

The existing body follows unchanged. With the default match now on the shell, the only live path left in it is `--live [<scenario>]`, whose 4½c F5 and selection logic stays.

(5) **`tick_shell`** (a plain fn, not a system):

```rust
/// One C++ `Gfx::RunOneFrame` of the live shell (design §7.1): this tick's key events (+ the
/// touch edges), the sampled words (keyboard levels + touch, as 4½c), `Shell::frame`, then the
/// present, the menu and sim sounds, the phase, a NEW GAME's log line, and QUIT.
#[allow(clippy::too_many_arguments)]
fn tick_shell(
    sim: &mut SimState,
    sh: &mut ShellRes,
    queue: &mut game::input::KeyQueue,
    source: &InputSource,
    keys: &ButtonInput<KeyCode>,
    time: &Time<Real>,
    images: &mut Assets<Image>,
    handle: &Handle<Image>,
    audio: &mut Drainer<Sink>,
    exit: &mut MessageWriter<AppExit>,
) {
    if sh.stopped {
        return; // the browser's QUIT: the canvas keeps the black frame (Q3)
    }
    #[allow(unused_mut)]
    let mut events = queue.take_tick();
    #[cfg(target_arch = "wasm32")]
    {
        let mask = touch_mask();
        let ex = sh.shell.settings().worm_settings[0].controls_ex;
        events.extend(game::touch::touch_key_events(sh.touch_prev, mask, &ex));
        sh.touch_prev = mask;
    }
    // Q5: F5 restarts during play and selection (Rust only; inert in the menu until 4½f).
    let restart = events.iter().any(|e| e.dos == ui::keys::DK_F5 && e.down && !e.repeat);
    let input = ShellInput {
        events: &events,
        sampled: sample_inputs(source, 0, keys, Mode::Live),
        fresh_seed: fresh_seed(),
        now_ms: time.elapsed().as_millis() as u64,
        restart,
    };
    let out = sh.shell.frame(sim, &input);
    present(&sh.shell, out.present, images, handle);
    if !out.menu_sounds.is_empty() {
        let events: Vec<SoundEvent> = out.menu_sounds.iter().map(|&s| SoundEvent::one_shot(s)).collect();
        audio.drain(&events);
    }
    if out.sim_ticked {
        audio.drain(&sim.sound_events);
        audio.reap(&live_loop_keys(sim));
    }
    if let Some(Route::NewGame { seed }) = out.routed {
        log_new_game(seed, sh.shell.settings());
    }
    if out.phase != sh.phase {
        publish_phase(out.phase.as_str());
        sh.phase = out.phase;
    }
    if out.quit {
        #[cfg(not(target_arch = "wasm32"))]
        exit.write(AppExit::Success);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = exit;
            sh.stopped = true;
        }
    }
}

/// Upload what the frame presents (design §4.7): the shell surface at the frame's fade
/// (`Gfx::Draw`'s `ScaleDraw`), the black `Enter` flip, or nothing (a pop frame keeps the image).
fn present(shell: &Shell, p: Option<Present>, images: &mut Assets<Image>, handle: &Handle<Image>) {
    let fade = match p {
        None => return,
        Some(Present::Frame { fade }) => fade,
        Some(Present::Black) => 0,
    };
    let mut image = images.get_mut(handle).expect("frame image exists");
    render::present::fade_into_rgba(shell.surface(), fade, image.data.as_mut().expect("image has data"));
}
```

Audio across the pause: the menu does not tick the match, so no reap runs. A loop that was sounding keeps sounding until the next match tick reaps it, as C++ never stops it either. On NEW GAME, Rust's first reap stops the old match's loops. C++ would leave them to the mixer, which is audio only; record it as a known divergence in T14.

(6) **`log_new_game(seed: u32, s: &Settings)`** replaces the `&NewGame` version, with the same text.

(7) **`close_on_esc`** runs only off the shell: `.run_if(|| !cfg!(target_arch = "wasm32")).run_if(not(resource_exists::<ShellRes>))`.

(8) **The file doc** gains a Step 4½d paragraph. A bare run and the bare preview boot the C++ main menu through `ui::shell::Shell`. Esc pauses to the menu (RESUME), QUIT exits (native) or stops on the black frame (browser), and F5 is the Rust-only restart. The Scripted / Replay / `--live [<scenario>]` / `--live --record` paths are unchanged.

Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS, with `passthrough`, `record_regression`, `round_trip` and `viewport_stepping` untouched and green.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown 2>&1 | tail -2` — Expected: builds.

- [ ] **Step 5: The page and the preview text**

`web/index.html`:

1. In `#touch`, before `#pad`, add `<div id="b-menu" class="ctl" data-bit="menu">MENU</div>`. Add the CSS:

   ```css
   #b-menu { top: max(8px, env(safe-area-inset-top)); right: max(8px, env(safe-area-inset-right));
     width: 64px; height: 34px; border-radius: 17px; font-size: 11px; }
   ```

   In landscape it sits in the right gutter above `#btns`; in portrait it sits above the game's top-right corner. Check both in T13.
2. `TOUCH` gains `menu: 256`. The button wiring and `update()` select `#touch .ctl[data-bit]` instead of `#btns .ctl` (the pad has no `data-bit`).
3. The hint follows the phase. Replace the four static `<span>`s with an empty `#touch-hint`, and add:

   ```js
   const HINTS = {
     menu: ['↑↓ pick', 'FIRE selects', 'JUMP: to QUIT'],
     weapsel: ['Pick ↑↓ ←→', 'FIRE on DONE!', 'You: blue worm', 'P2: a bot'],
     game: ['MENU pauses', 'You: blue worm'],
   };
   ```

   A 200 ms `setInterval` re-renders `#touch-hint` when `window.lieroPhase` changes (one `<span>` per entry).
4. The QUIT overlay (Q3). Add `<div id="quit" hidden>Quit — tap or press a key to play again</div>` with CSS `#quit { position: fixed; inset: 0; z-index: 20; display: flex; align-items: center; justify-content: center; background: rgba(0,0,0,0.6); color: #fff; font-size: 16px; } #quit[hidden] { display: none; }`. In the same interval: when `window.lieroPhase === 'quit'`, un-hide it once, and add one-shot `keydown` and `pointerdown` listeners that call `location.reload()`.
5. `#help`:

   ```html
   <b>Main menu</b>: <kbd>↑</kbd>/<kbd>↓</kbd> (or P1 aim) pick · <kbd>Enter</kbd> (or fire) selects ·
   <kbd>Esc</kbd> (or jump) goes to QUIT TO OS · NEW GAME starts weapon selection; <kbd>Esc</kbd> in a
   match pauses to the menu (RESUME GAME)
   ```

   Keep the two player lines and the weapon-selection line. Replace the F5 line with `Dig: hold left + right · <kbd>F5</kbd> restarts a match (back to weapon selection)`. The URL line adds `<code>?menu=1</code> shows the menu even with weapons/level/seed` and says that `weapons`/`level`/`seed` skip the menu.

`.github/workflows/preview.yml` (the comment body, `:125-135`):
- "It starts like the original's NEW GAME: …" becomes "It opens on the original's **main menu** (NEW GAME → weapon selection → play; Esc pauses to RESUME / NEW GAME; QUIT TO OS)".
- The URL-parameters line says `weapons`, `level` and `seed` skip the menu and adds `menu` (forces it).
- Add a `[Main menu with a water level](${url}/?level=water_stage&menu=1)` example.
- The phone line mentions the MENU button (Esc).

- [ ] **Step 6: `shot --menu`**

In `rust/shot/Cargo.toml` add `ui = { path = "../ui" }`. In `rust/shot/src/lib.rs`:
- `Config` gains `menu: bool`, `menu_frames: u32` (default 40) and `menu_seed: u32` (default 1).
- `parse_args` accepts `--menu`, `--frames <u32>` and `--seed <u32>`. With `--menu`, the `--scenario`/`--scenario-path` requirement is skipped. It needs `--out`, and it refuses `--scenario`, `--scenario-path`, `--tick`, `--hashes` and `--weapsel`.
- Add:

```rust
/// `--menu` (Step 4½d): the main menu after `frames` idle frames of `ui::shell::Shell::boot` with
/// the default settings and boot seed `seed` (frame 0 is the boot background with the copyright
/// bar, before the first menu draw), as the UNFADED 320x200 surface. The pixels are gated against
/// the real C++ frame loop by `oracle-tests/tests/shell_golden.rs` (`boot_idle`).
pub fn render_menu(tc_root: &Path, frames: u32, seed: u32, scale: u32) -> Vec<u8> {
    use ui::shell::level_slot::SeedSource;
    use ui::shell::playing::StartOptions;
    use ui::shell::{Shell, ShellInput};
    let settings = scenario::settings::Settings::default();
    let (mut sh, mut sim, _) = Shell::boot(tc_root, settings, SeedSource::Fixed(seed), 0, StartOptions::default());
    for _ in 0..frames {
        sh.frame(&mut sim, &ShellInput::idle());
    }
    encode_png(sh.surface(), scale)
}
```

`run` dispatches `--menu` before the scenario path (like `--weapsel`). Add tests:
- `parse_args` accepts `--menu --out a.png` (frames 40, seed 1) and `--menu --frames 3 --seed 9 --out a.png`, and rejects `--menu` alone, `--menu --tick 1 --out a.png` and `--menu --scenario x --out a.png`;
- `render_menu(&tc, 40, 1, 1)` twice gives equal bytes that start with the PNG signature.

Run: `cd /home/user/openliero/rust && cargo test -p shot && cargo run -p shot -- --menu --out /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/menu.png` — Expected: PASS; the PNG shows the menu (Read tool).

- [ ] **Step 7: GREEN and a native smoke run**

Run: `rustfmt --edition 2024 /home/user/openliero/rust/ui/src/shell/new_game.rs`. `game`'s edited files are hand-formatted: never run rustfmt on `main.rs`.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game 2>&1 | grep -cE 'test result: FAILED'` — Expected: `0`.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown 2>&1 | tail -2` — Expected: builds.
Run: `cd /home/user/openliero/rust && xvfb-run -a timeout 20 cargo run -p game; echo "exit $?"` — Expected: `exit 124` (it ran until the timeout on the menu). If the container has no GPU adapter and it exits early, record that and rely on T12/T13.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 8: Commit**

```
git -C /home/user/openliero add rust/game rust/ui/src/shell/new_game.rs rust/shot rust/Cargo.lock web/index.html .github/workflows/preview.yml
git -C /home/user/openliero commit -m "game(4.5d): the live game boots the C++ main menu — key events + KeyCode→DOS, touch edges + MENU, faded presents, QUIT (native exit / browser black frame + Play again), ?menu=1, F5 restart; shot --menu" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

Reviewer (Opus): check the following.
- `tick_and_render` is still the only system with `ResMut<Sim>`.
- The non-shell paths are behavior-identical (diff the old and new `setup`/`tick_and_render` for them).
- `close_on_esc` never runs on the shell path, so Esc pauses there instead of quitting.
- The key queue drops nothing (`PreUpdate` + `take_tick`).
- F5 acts only while Playing.
- The wasm build compiles the `cfg(wasm32)` touch and QUIT paths.
- The page's `data-bit` selector still wires the four old buttons.

---

### Task 12: The real C++ `openliero` under Xvfb — C++ | Rust side-by-side PNGs of the milestone path  [Sonnet]

**Files:**
- Create (scratchpad only, NOT committed): `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/xd12.sh`, `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/x12_cpp/`, `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/x12_rust/`, `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/x12_side/`, `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/cppdata/` (a 2.3 MB copy of `data/`)

**Interfaces:**
- Consumes: the C++ `openliero` binary (`build/linux-x64/Release/openliero`, already built for 4½c; the game library is unchanged by this slice, so it needs no rebuild); T11's live shell; the 4½c helpers `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/xd.sh` (window lookup, `tap`, `snap`) and `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ppm2png.py`.

Why: the standing ruling (A2). G2 gates every frame bit-exact, but John sees the result as a window. This task puts the real C++ game and the Rust game side by side at the same six moments of the milestone path. It is an eyeball artefact, **not a gate**: window scaling, timing and the random boot level differ. Nothing is committed. If Xvfb or xdotool is missing, `apt-get install -y xvfb xdotool imagemagick`; if that fails, skip the task and say so in the done-report. It never blocks T14.

- [ ] **Step 1: Start the display and the C++ game**

Run: `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad; test -x /home/user/openliero/build/linux-x64/Release/openliero || (source $S/env.sh && cmake --build /home/user/openliero/build/linux-x64 --config Release --target openliero)`
Run: `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad; rm -rf $S/cppdata && cp -r /home/user/openliero/data $S/cppdata && mkdir -p $S/x12_cpp $S/x12_rust $S/x12_side && (pgrep -x Xvfb || (nohup Xvfb :99 -screen 0 1280x800x24 > $S/xvfb.log 2>&1 &))`
Run: `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad; (DISPLAY=:99 SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy /home/user/openliero/build/linux-x64/Release/openliero --config-root $S/cppdata > $S/cpp_game12.log 2>&1 &); timeout 10 bash -c 'until DISPLAY=:99 xdotool search --name "^Liero" >/dev/null 2>&1; do :; done'`

`--config-root` keeps the C++ `liero.cfg` writes inside the scratchpad copy.

- [ ] **Step 2: Drive the C++ game and snap six moments.** Write `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/xd12.sh`: it sources `xd.sh`, takes the output directory as `$1`, and redefines `snap` to `import -window $W "$1/$2.png"`. Then it runs:

```bash
sleep 1.5;           snap "$1" 1_menu            # boot: the main menu over the generated level
tap Return; sleep 1; snap "$1" 2_weapsel         # NEW GAME -> weapon selection (after the fade)
tap r; tap Up; tap Control_L; tap Control_R      # P1 (R) and P2 (Up): Randomize -> DONE! (wraps); both fire: ready
sleep 2;             snap "$1" 3_game            # play
tap Escape; sleep 1; snap "$1" 4_pause           # Esc -> the pause menu: RESUME GAME (F1) + the copyright bar
tap Return; sleep 1; snap "$1" 5_resumed         # RESUME -> play
tap Escape; sleep 1; tap Escape; sleep 0.3
                     snap "$1" 6_quit_cursor     # Esc again in the menu: the cursor on QUIT TO OS
tap Return; sleep 1.5
pgrep -x openliero >/dev/null && echo "still running" || echo "exited"   # QUIT -> the process exits
```

The keys are the default bindings (the 4½c walk used the same): P1 aim up `r`, fire `Control_L`; P2 aim up `Up`, fire `Control_R`. The menus take `Up`/`Down`/`Return`/`Escape`. Weapon selection opens with both cursors on Randomize, so one Up wraps to DONE!.

Run: `DISPLAY=:99 bash /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/xd12.sh /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/x12_cpp` — Expected: six PNGs and `exited`. If the window title differs, `xdotool search --name .` lists it; fix the pattern in the script, not in `xd.sh`.

- [ ] **Step 3: The Rust side at the same moments.** First try the native game under the same display: `cd /home/user/openliero/rust && cargo build -p game && (DISPLAY=:99 ./target/debug/game > $S/rust_game12.log 2>&1 &)`, find its window (`xdotool search --name .` — the Bevy window title), and run the same script with `W` pointing at it, into `x12_rust/`. If the native game cannot open a window (no GPU adapter: the log says so within a few seconds), use the preview bundle instead: run T13 Step 1, then take the same six moments in headless Chromium at 640x520 with `page.locator('canvas').screenshot()` after each step (the key taps of T13's `tap`), naming the files the same. Record which path produced the Rust PNGs.

- [ ] **Step 4: Pair them**

Run: `cd /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad && for f in x12_cpp/*.png; do n=$(basename $f); convert \( $f -filter point -resize 640x400\! \) \( x12_rust/$n -filter point -resize 640x400\! \) +append x12_side/$n; done`

Look at every `x12_side/*.png` (Read tool). Expected: the same screens in the same order — the menu (15 items, NEW GAME selected, the copyright bar), weapon selection, play, the pause menu with RESUME GAME (F1), play again, the cursor on QUIT TO OS. The levels differ (both are random). The palettes, fonts, item positions and the menu water must look the same. Anything else is a finding: report it with the file name; do not change code in this task.

- [ ] **Step 5: Clean up.** Kill the games (`pkill -x openliero; pkill -x game`) and Xvfb if this task started it, and `rm -rf /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/cppdata`. The PNGs stay in the scratchpad for the controller to show John. No commit.

---

### Task 13: Headless Chromium — the shell flow on a desktop and an emulated phone  [Sonnet] (nice-to-have)

**Files:**
- Create (scratchpad only, NOT committed): `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/shell.mjs`, `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/site13/`, `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/shots13/`

**Interfaces:**
- Consumes: T11's page (`window.lieroPhase ∈ {menu, weapsel, game, quit}`, `window.lieroTouchOnly`, `window.lieroTouch` with MENU = 256, `#b-menu`, `#touch-hint`, `#quit`); the bundle recipe in `web/index.html`'s comment; Playwright under `/opt/node22/lib/node_modules/` (as in `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/t9.mjs`).

Why: the preview is how John and reviewers meet the milestone. G2 proves the frames; only the browser proves the page end to end: the bare URL boots the menu, the whole path walks, QUIT stops on the black frame with the overlay, the reload boots again, the parameters route (Q4), and the touch MENU button pauses (Q8). If Chromium or Playwright is unavailable, skip this task and say so in the done-report. It never blocks T14.

Two SwiftShader facts from 4½c apply (the comments in `t9.mjs`):
- At 1000x780 the page runs about 7 fps and a slow frame runs up to 17 fixed ticks with one key state, so a 110 ms tap trips the 12/3 weapon-selection repeat. **Use a 640x520 viewport** (about 24 fps) everywhere, and a `deviceScaleFactor: 1` phone.
- A real touch is held for hundreds of ms under SwiftShader. **Drive the phone with exact `window.lieroTouch` pulses** injected in page time. On the menu a pulse may be long (touch has no OS repeat, and a long pulse gives the key queue's deferral (fact 19) room): hold at least 300 ms. In weapon selection a held pad bit repeats, so the pulse there is two animation frames.

- [ ] **Step 1: Build and serve the preview bundle** (the preview's own profile)

Run: `cd /home/user/openliero/rust && cargo build -p game --profile wasm-release --target wasm32-unknown-unknown`
Run: `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad; mkdir -p $S/site13 $S/shots13 && wasm-bindgen --out-dir $S/site13 --out-name game --target web --remove-name-section --remove-producers-section /home/user/openliero/rust/target/wasm32-unknown-unknown/wasm-release/game.wasm && cp /home/user/openliero/web/index.html $S/site13/`
Run (in the background): `python3 -m http.server --directory /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/site13 8091` (not 8765: a server for `scratchpad/site` may already hold that port).

- [ ] **Step 2: Write** `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/shell.mjs`:

```js
// Step 4½d T13 — headless check of the main-menu shell in the preview bundle.
// Usage: node shell.mjs <base-url> <outdir>
import { createRequire } from 'module';
const require = createRequire('/opt/node22/lib/node_modules/');
const { chromium, devices } = require('playwright');

const [base, out] = process.argv.slice(2);
const browser = await chromium.launch({
  args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader',
         '--autoplay-policy=no-user-gesture-required'],
});
let failed = false;
const check = (name, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name} ${detail}`);
  if (!ok) failed = true;
};
const SMALL = { width: 640, height: 520 };
const phase = (page) => page.evaluate(() => window.lieroPhase);
const reaches = (page, want, ms) =>
  page.waitForFunction((w) => window.lieroPhase === w, want, { timeout: ms }).then(() => true, () => false);
const tap = async (page, key) => {
  await page.keyboard.down(key);
  await page.waitForTimeout(110); // < 171 ms: no weapon-selection key repeat
  await page.keyboard.up(key);
  await page.waitForTimeout(150);
};
const watch = (page) => {
  const errors = [];
  page.on('pageerror', (e) => { if (!String(e).includes('control flow')) errors.push(String(e)); });
  return errors;
};
const shot = (page, name) => page.screenshot({ path: `${out}/${name}.png` });
const open = async (page, query, want) => {
  await page.goto(base + '/' + query);
  await page.waitForSelector('canvas', { timeout: 60000 });
  return reaches(page, want, 30000);
};

{ // 1. Desktop: the milestone path, QUIT, the overlay, the reload.
  const page = await browser.newPage({ viewport: SMALL });
  const errors = watch(page);
  check('desktop: a bare URL boots the main menu', await open(page, '', 'menu'), `phase=${await phase(page)}`);
  check('desktop: touch-only is off', (await page.evaluate(() => window.lieroTouchOnly)) === false);
  await page.waitForTimeout(1200);
  await shot(page, 'd1_menu');
  await page.click('canvas');
  await tap(page, 'Enter');                       // NEW GAME (the boot cursor)
  check('desktop: NEW GAME -> weapon selection', await reaches(page, 'weapsel', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(800);
  await shot(page, 'd2_weapsel');
  await tap(page, 'KeyR'); await tap(page, 'ArrowUp');           // both: Randomize -> DONE!
  await tap(page, 'ControlLeft'); await tap(page, 'ControlRight'); // both ready
  check('desktop: both DONE -> play', await reaches(page, 'game', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(1500);
  await shot(page, 'd3_game');
  await tap(page, 'Escape');
  check('desktop: Esc pauses to the menu', await reaches(page, 'menu', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(800);
  await shot(page, 'd4_pause');                   // RESUME GAME (F1) over the game
  await tap(page, 'Enter');                       // RESUME (the pause cursor)
  check('desktop: RESUME returns to play', await reaches(page, 'game', 5000), `phase=${await phase(page)}`);
  await page.keyboard.press('F5');
  check('desktop: F5 restarts to weapon selection', await reaches(page, 'weapsel', 5000), `phase=${await phase(page)}`);
  await tap(page, 'Escape');                      // Esc in selection -> the menu (RESUME resumes it)
  check('desktop: Esc in selection -> the menu', await reaches(page, 'menu', 5000), `phase=${await phase(page)}`);
  await tap(page, 'Escape');                      // the cursor to QUIT TO OS
  await tap(page, 'Enter');                       // QUIT
  check('desktop: QUIT stops the page', await reaches(page, 'quit', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(600);
  check('desktop: the Play again overlay is shown', await page.locator('#quit').isVisible());
  await shot(page, 'd5_quit');
  // The black frame is eyeballed on d5_quit.png: a WebGL canvas without preserveDrawingBuffer
  // reads back blank, so a pixel read here would pass whatever was drawn. T7 unit-tests Present::Black.
  await Promise.all([page.waitForNavigation({ timeout: 30000 }), page.keyboard.press('Space')]);
  check('desktop: a key reloads to the main menu', await reaches(page, 'menu', 30000), `phase=${await phase(page)}`);
  check('desktop: no page errors', errors.length === 0, errors.slice(0, 2).join(' | '));
  await page.close();
}

for (const [query, want] of [['?weapons=BAZOOKA', 'game'], ['?level=water_stage', 'weapsel'],
                             ['?seed=7', 'weapsel'], ['?seed=7&menu=1', 'menu']]) { // 2. Q4 routes
  const page = await browser.newPage({ viewport: SMALL });
  check(`${query} opens on ${want}`, await open(page, query, want), `phase=${await phase(page)}`);
  if (query.includes('menu=1')) { await page.waitForTimeout(1000); await shot(page, 'r_seed7_menu'); }
  await page.close();
}

{ // 3. Phone: the MENU button, FIRE selects, JUMP to QUIT (Q8).
  const ctx = await browser.newContext({ ...devices['iPhone 13'], viewport: { width: 844, height: 390 },
                                         deviceScaleFactor: 1, hasTouch: true, isMobile: true });
  const page = await ctx.newPage();
  const errors = watch(page);
  check('phone: boots the main menu', await open(page, '', 'menu'), `phase=${await phase(page)}`);
  check('phone: touch-only is on', (await page.evaluate(() => window.lieroTouchOnly)) === true);
  check('phone: the MENU button is shown', await page.locator('#b-menu').isVisible());
  await page.waitForTimeout(1200);
  await shot(page, 'p1_menu');
  // Exact pulses in page time: the mask for at least `ms` and two animation frames, then released.
  const pulse = (bits, ms) => page.evaluate(([b, t]) => new Promise((r) => {
    window.lieroTouch = b;
    const t0 = performance.now();
    const off = () => (performance.now() - t0 >= t
      ? (window.lieroTouch = 0, setTimeout(r, 300)) : requestAnimationFrame(off));
    requestAnimationFrame(() => requestAnimationFrame(off));
  }), [bits, ms]);
  await pulse(16, 300);                            // FIRE on NEW GAME
  check('phone: FIRE selects NEW GAME', await reaches(page, 'weapsel', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(500);
  await pulse(1, 0);                               // pad Up: Randomize -> DONE! (wraps; 2 rAF: no repeat)
  await pulse(16, 0);                              // FIRE: ready; P2 is a bot that is ready at once
  check('phone: pad + FIRE start play', await reaches(page, 'game', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(1500);
  await shot(page, 'p2_game');
  await pulse(256, 300);                           // MENU
  check('phone: MENU pauses to the menu', await reaches(page, 'menu', 5000), `phase=${await phase(page)}`);
  const hint = await page.locator('#touch-hint').innerText();
  check('phone: the hint follows the phase', hint.includes('FIRE selects'), JSON.stringify(hint));
  await shot(page, 'p3_pause');
  await pulse(16, 300);                            // FIRE on RESUME
  check('phone: FIRE resumes', await reaches(page, 'game', 5000), `phase=${await phase(page)}`);
  await pulse(256, 300);                           // MENU
  await reaches(page, 'menu', 5000);
  await pulse(64, 300);                            // JUMP: the cursor to QUIT TO OS
  await pulse(16, 300);                            // FIRE: QUIT
  check('phone: JUMP + FIRE quit', await reaches(page, 'quit', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(600);
  await shot(page, 'p4_quit');
  await Promise.all([page.waitForNavigation({ timeout: 30000 }), page.locator('#quit').tap()]);
  check('phone: a tap reloads to the main menu', await reaches(page, 'menu', 30000), `phase=${await phase(page)}`);
  check('phone: no page errors', errors.length === 0, errors.slice(0, 2).join(' | '));
  await ctx.close();
}

{ // 4. Portrait phone: the MENU button sits above the game, clear of the pad and the buttons.
  const ctx = await browser.newContext({ ...devices['iPhone 13'], viewport: { width: 390, height: 844 },
                                         deviceScaleFactor: 1, hasTouch: true, isMobile: true });
  const page = await ctx.newPage();
  await open(page, '', 'menu');
  await page.waitForTimeout(1000);
  const [m, pad, fire] = await Promise.all(['#b-menu', '#pad', '#b-fire'].map((s) => page.locator(s).boundingBox()));
  const apart = (a, b) => a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y;
  check('portrait: MENU overlaps neither the pad nor FIRE', apart(m, pad) && apart(m, fire), JSON.stringify(m));
  await shot(page, 'p5_portrait');
  await ctx.close();
}

await browser.close();
process.exit(failed ? 1 : 0);
```

- [ ] **Step 3: Run it**

Run: `cd /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad && node shell.mjs http://127.0.0.1:8091 /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/shots13`
Expected: every line `ok`, exit 0. Look at `d1_menu`, `d4_pause`, `d5_quit`, `p1_menu`, `p3_pause` and `p5_portrait` (Read tool): the menu over the level with the copyright bar, the pause menu over the game, the black frame under the overlay, and the MENU button placed as T11 Step 5 says in both orientations.

A `FAIL` names the step:
- A route or a phase that never comes is a T11 bug. Report it with the phase value; fix it in T11's files with a commit of its own (`game(4.5d): …`, same trailers), rebuild (Step 1) and re-run.
- A lost phone press that a longer pulse fixes is the low-fps tap loss (fact 19): report the pulse length that worked. Do not weaken the key queue.
- The black frame under the overlay is checked by eye on `d5_quit.png` (a WebGL canvas reads back blank, so the script cannot test it).

- [ ] **Step 4: Stop the server** (kill the background job). Paste the `ok`/`FAIL` lines into the done-report. There is no commit unless Step 3 needed a T11 fix.

---

### Task 14: Full re-diff, wasm, audits, PROGRESS + overview + map corrections, broad review  [Opus review]

**Files:**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md` (the header "Last updated" paragraph `:11-49`; the rewrite-track status `:481` and the tree headline `:495`; the Step 4½ heading `:822`; the Step 4½ tree's 4½d line `:867-868`; "Open for John" `:905-921`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md` (the status line `:3`; the 4½d bullet `:304-320`)
- Modify: `docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md` (the status line `:3`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` (`:116-118`, `:209`), `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (`:91`, `:168`)
- Modify: `.claude/skills/liero-shot/SKILL.md` (§7)

Re-read each doc immediately before editing it: the line numbers are from plan time.

- [ ] **Step 1: The full green board (DEBUG)**

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown` — Expected: builds.
Run: `cd /home/user/openliero/rust && cargo tree -p sim-core --depth 1` — Expected: `sim-core` with no dependencies.
Run: `cd /home/user/openliero/rust && cargo tree -p sim --depth 1 -e normal` — Expected: only `assets` and `sim-core`.
Run: `cd /home/user/openliero/rust && cargo tree -p ui --depth 1 -e normal` — Expected: only workspace crates (`sim-core`, `assets`, `sim`, `render`, `scenario`): no external dependency.
Run: `cd /home/user/openliero/rust && cargo tree -p ui -e normal | grep -c bevy; cargo tree -p render -e normal | grep -c bevy` — Expected: `0` and `0`.

- [ ] **Step 2: Tripwires and audits**

Run: `grep -rnE "HashMap|HashSet|\bf32\b|\bf64\b|SystemTime|Instant" /home/user/openliero/rust/ui/src` — Expected: no output (Global Constraints: determinism; the only clocks are `ShellInput::fresh_seed` and `now_ms`).
Run: `grep -rn "bevy" /home/user/openliero/rust/ui` — Expected: no output outside comments.
Run: `git -C /home/user/openliero diff --name-status a61cbb6 -- rust/oracle-tests/golden | grep -v '^A'` — Expected: no output.
Run: `git -C /home/user/openliero diff --name-status a61cbb6 -- rust/oracle-tests/golden | wc -l` — Expected: `61` (34 `menu_*` + 27 `shell_*`).
Run: `git -C /home/user/openliero diff --name-only a61cbb6 -- src` — Expected: exactly `src/tools/oracle_dump/menu_dump.cpp` and `src/tools/oracle_dump/shell_dump.cpp`.
Run: `git -C /home/user/openliero diff a61cbb6 -- CMakeLists.txt | grep -c '^+[^+]'` — Expected: `4` (the `oracle_dump_menu` and `oracle_dump_shell` lines), and no `-` line.
Run: `git -C /home/user/openliero diff --name-only a61cbb6 -- rust/sim rust/sim-core rust/scenario/src/parser.rs rust/oracle-tests/examples/gen_slice4_5a.rs rust/oracle-tests/examples/gen_slice4_5c0.rs rust/oracle-tests/examples/gen_slice4_5c.rs rust/oracle-tests/tests/weapsel_common rust/oracle-tests/tests/sim_slice4_5c0_common src/tools/oracle_dump/weapsel_dump.cpp src/tools/oracle_dump/weapsel_drive.hpp src/tools/oracle_dump/sim_physics_dump.cpp` — Expected: no output (LD 4/5, the crate rules, frozen provenance).
Run: `grep -rn "ResMut<Sim>" /home/user/openliero/rust/game/src | wc -l` — Expected: the same count as at `a61cbb6` (`git -C /home/user/openliero show a61cbb6:rust/game/src/main.rs | grep -c "ResMut<Sim>"`): LD 3, `tick_and_render` only.
Run (each file): `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/menu_dump.cpp` (and `shell_dump.cpp`) — Expected: no output.
Run: `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 a61cbb6` — Expected: exit 0 (fact 20: pass the base).
Reproducibility — regenerate every 4½d golden once more, in one shell:
Run: `cd /home/user/openliero/rust && cargo run -p oracle-tests --example gen_slice4_5d -- write /home/user/openliero/rust/oracle-tests/golden && cargo run -p oracle-tests --example gen_slice4_5d -- check`
Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_menu_golden.sh && bash /home/user/openliero/rust/oracle-tests/gen_shell_golden.sh`
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output (byte-identical).
Optional native smoke: `cd /home/user/openliero/rust && xvfb-run -a timeout 20 cargo run -p game; echo "exit $?"`. Expected: `exit 124`. If it exits early because the container has no GPU adapter, note that; T12/T13 cover the live path.

- [ ] **Step 3: `liero-shot` §7** — in `.claude/skills/liero-shot/SKILL.md`, §7.1 (Live play), after the 4½c weapon-selection paragraph, add:

```
**Main menu (Step 4½d).** A bare `cargo run -p game` and a bare preview URL boot the C++ main menu
(`ui::shell::Shell`, the `Gfx::RunOneFrame` analog) over a generated level: all 15 items are
shown, the unported ones are inert (Rust-only: C++ acts on them). NEW GAME → weapon selection →
play; Esc pauses to the menu with RESUME GAME (F1); QUIT TO OS fades out and exits natively, or
stops on a black frame with a "Play again" overlay in the browser. F5 is a Rust-only restart
(play and selection). `?weapons=`, `?level=` and `?seed=` skip the menu; `?menu=1` forces it; the
touch pad drives the menu as player 1's keys and MENU (bit 256) is Esc. `--live [<scenario>]`
and `--live --record` never show the menu. Headless: `cargo run -p shot -- --menu [--frames N]
[--seed S] --out x.png`. The gates are `oracle-tests/golden/menu_*` (G1: C++ `oracle_dump_menu`,
the real `Menu`/behaviors/`SettingsMenu`) and `shell_*` (G2: C++ `oracle_dump_shell`, the real
`Gfx::RunOneFrame` headless, every presented frame bit-exact).
```

- [ ] **Step 4: PROGRESS** — set "Last updated" to the real current date. Prepend a header paragraph (the previous one becomes "Prior (…)"): **🗡️ 4½d (the menu framework, `ScreenStack` and the main menu — THE STEP 4½ MILESTONE) LANDED.**

The paragraph says:
- The new Bevy-free `rust/ui` crate (Q1): `ui::menu` (the `Menu`/`MenuItem` port, the behaviors, type-to-search), `ui::keys` (`KeyLatch`, `ReleaseLatch`), `ui::text`, and `ui::shell` (`Shell` = `Gfx`, `ScreenStack`, `MainMenuState`, the display-only settings menu, the router with `LevelSlot`/`SeedSource`, `Match` = LocalController with the Esc fade and the shared tail; the 4½c live modules moved in).
- `render` gained the CP437 high half (hash-neutral), the value arm, the scrollbar, `menu_palette` and `present::fade_argb`.
- G1: `oracle_dump_menu` drives the real C++ `Menu`, behaviors and `SettingsMenu`; 15 scripts match line for line.
- 🎯 G2: `oracle_dump_shell` runs the real `Gfx::RunOneFrame` headless (a software renderer, eight documented interventions); 11 cases, every presented frame bit-exact, including `shell_milestone` (boot → menu → NEW GAME → selection → play → Esc → menu → RESUME / NEW GAME → QUIT).
- The live game: key events + `KeyCode → DOS`, touch edges + the MENU button (Q8), the faded present, QUIT (Q3), the Q4 routes and `?menu=1`, F5 (Q5); `NewGame` retired for `LevelSlot` + `SeedSource`.
- The golden audit: 61 `A` lines, no `M`; no existing golden changed. The T12 side-by-side and the T13 browser result.

In the Step 4½ tree replace the 4½d lines with

```
├─ ✅ 4½d  menu framework + ScreenStack + main menu — THE MILESTONE. Bevy-free rust/ui: Menu/MenuItem/
│          behaviors/type-to-search, KeyLatch, Shell (= Gfx::RunOneFrame), MainMenuState, router,
│          Match (Esc fade, tail); render CP437/value arm/scrollbar/menu_palette/fade; 🎯 G1 15 widget
│          scripts vs real C++ Menu + G2 11 shell cases bit-exact vs the real headless RunOneFrame;
│          live: key events, touch MENU, QUIT (native exit / web Play again), ?menu=1, F5 COMPLETE
```

update the Step 4½ headlines (`4½d ✅`, and "the Step 4½ milestone reached"), and under "Open for John" add:
- **The design facts that turned out wrong against the source** (plan facts 1, 2, 3, 5, 6, 9, 10, 14, 18): `--live` alone is not the default match; C++ acts on the placeholder Enters and the F-keys, so "inert" is Rust-only and is unit-tested, not gated; only JOIN LAN / HOST ONLINE / JOIN ONLINE play a second select; a file level resolves against the CWD in C++ (intervention 8); `menu_palette` ≠ `weapsel_palette`; the copyright reads "MetsänElämet"; C++ `StartGame` plays `SoundBegin` (now ported, so the live game plays the begin sample); the G2 format gained `upd`; more modules moved.
- **Known live divergences** (not gated): a worm key held through Esc and released in the menu stays pressed in C++ and not in Rust (fact 12); a looping sound across NEW GAME is stopped by Rust's first reap, while C++ leaves it to the mixer (T11); the tap deferral at low frame rates (fact 19) is presentation only.
- **Still absent:** the C++ random player names (4½f); recording a menu-driven match (Q6, postponed); `liero.cfg` load/save (Q7, 4½e). `--live [<scenario>]` keeps the 4½c scenario-live semantics (selection, F5, Esc quits), with no menu.
- Remove the 4½c "Open for John" caveats that 4½d settled: no render fade, and `menu_cycles` starting at 0 (`:914-915`).

- [ ] **Step 5: The overview** — status line: add `**4½d LANDED (the Step 4½ milestone)**` after `**4½c LANDED**`, and make it `4½e–4½h planned`. At the end of the 4½d bullet append:

"**Landed** (design `specs/2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md`, plan `plans/2026-09-26-liero-rs-step4.5-slice4.5d-plan.md`). The code lives in the new Bevy-free `rust/ui` crate (Q1). Corrections from the plan's source check:
(1) the menu boots for a bare run and a bare preview only; `--live [<scenario>]` keeps the 4½c path;
(2) C++ acts on every placeholder item and on F2/F3/F5/F6/F7/F9, so Rust's inert items are unit-tested, never gated; HOST LAN goes through `default:` and plays one `MenuSelect`;
(3) C++ reads `level_file` relative to the process CWD;
(4) the menu palette is the rotation plus `SetWormColours` (not the weapsel palette);
(5) `StartGame` plays `SoundBegin`, which Rust now ports.
**Step 5 note:** netplay's rollback runs `Match` only; the menus never run inside a rollback frame. The shell's `Route` is the seam where 4½g's network screens push states."

- [ ] **Step 6: The maps and the design**

- cpp-map `:116-118`: append ` (**4½d:** `DrawSpectatorInfo` and `MainMenuState::Enter`'s minimap touch only `single_screen_renderer` and the spectator viewport's own `rand`, never `game.rand` or `play_renderer`, so the port does not need them; plan fact 21.)`
- cpp-map `:209`: replace "(Human / DumbAI / FollowAI)" with the three strings of `texts.controllers` as `tc.cfg` spells them (re-read `data/TC/openliero/tc.cfg` and copy them exactly).
- rust-map `:91`: append ` (**4½d:** the menus take key *events* — a `KeyEvent` queue with `KeyCode → DOS` and typed symbols, fed to `ui::keys::KeyLatch` — not `just_pressed`; `just_pressed(F5)` remains only for the Rust-only restart.)`
- rust-map `:168`: correct the `Texts` list: `onoff` and `game_modes` are hard-coded in C++ (`common.cpp`), not `tc.cfg` texts, and `ui::text` carries them as constants (T1). Re-read `assets/src/tc.rs` first and name exactly which of `weap_states`, `controllers` and `key_names` `Texts` does carry.
- design `:3`: `Status: **LANDED** · 2026-09-26 · …` (keep the rest; add `plan: plans/2026-09-26-liero-rs-step4.5-slice4.5d-plan.md`).

- [ ] **Step 6b: Stage** — `git -C /home/user/openliero status --short` must list only the docs above (and nothing under `rust/`, `src/`, `web/`).

- [ ] **Step 7: Broad review (Opus)** — re-read the whole slice diff (`git -C /home/user/openliero diff a61cbb6 -- rust src web .github CMakeLists.txt`) against the design and this plan:
- every design finding and every plan-time fact is either ported and gated (map each to its G1 script, G2 case or unit test) or recorded in PROGRESS as a divergence or deferral;
- `ui::menu` transcribes `menu.cpp`, `menuItem.cpp` and the `*Behavior.cpp` files line for line, including the fact-11 quirks;
- `Shell::frame` follows `gfx.cpp:1467-1652` in order: poll, capture `sel`/`fading` before Update, Update, the empty-stack pop to the router with no draw, `UpdateMenuPalettes` for menu screens, Draw, `++menu_cycles` for play, present;
- the eight G2 interventions are each documented in `shell_dump.cpp`, touch no code under test, and the dumper refuses the cases it cannot run faithfully (the time-seed probe, the level-file open);
- LD 3/4/5 and the crate rules hold (Step 1–2 evidence); `game` is glue only;
- the non-shell paths (`?demo`, Scripted, `--replay`, `--live --record`, `--live [<scenario>]`) are behavior-identical;
- the browser pieces work (T13's result) and the T12 eyeball found nothing unexplained;
- the design-vs-source list at the top of this plan is reflected in PROGRESS;
- no push, no PR.
Bar: 0 Critical / 0 Important.

- [ ] **Step 8: Commit**

```
git -C /home/user/openliero add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-rust-baseline-map.md .claude/skills/liero-shot/SKILL.md
git -C /home/user/openliero commit -m "docs(4.5d): PROGRESS + overview + maps — slice 4.5d landed (menu framework, ScreenStack, main menu: the Step 4.5 milestone)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---

## Done-report (each task)

Each task reports:
- (a) what changed and why;
- (b) the files touched;
- (c) the tests run, with results, and any risks.

One commit per task (none for T12 and T13, unless T13 needed a T11 fix), on `claude/cpp-oracle-vcpkg-assets-chcwcm`; no push, no PR. The final report surfaces:
- the CP437 re-diff (T0: every render golden unchanged; the one `font.rs` test updated, fact 18);
- the G1 result (T5: 15 scripts line for line; the awk gates and the negative test);
- the dumper evidence (T4/T8: the smoke cases, the time-seed probe showing the selection constructor drew nothing, and each of the eight interventions);
- the chosen seeds and case lengths (T9), and every refused script;
- 🎯 the milestone result (T10: 11 G2 cases, every presented frame bit-exact, `shell_milestone` named on its own);
- the live wiring (T11: the native smoke exit code, the wasm build);
- the T12 side-by-side PNG paths and which path produced the Rust side;
- the T13 browser `ok`/`FAIL` lines;
- the audit sweep (T14: 61 `A` golden lines, no `M`; `src` = the two new dumpers; 4 `CMakeLists.txt` lines; `cargo tree -p ui | grep -c bevy` = 0; the regeneration byte-identical).

