# Step 4½, Slice 4½e-1: the settings menu, weapon options, number entry, `liero.cfg`, the small labels (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN). This plan names types, fields, files and behaviour; it does not write the code. Where it quotes C++ it quotes the source, and the source wins over this text.

**Goal.** The C++ settings menu is live in Rust. MATCH SETUP (F7, or Enter on it) gives the settings menu focus, and every `SettingsMenu` item works with the C++ keys, sounds and quirks. That covers number entry (`InputStringState`), WEAPON OPTIONS (`WeaponMenuState` and its `InfoBoxState`), and edits that reach a paused match at RESUME (finding 1). `liero.cfg` is read at boot and written at exit, and the three `DrawTextSmall` labels are drawn. Every presented frame on those paths is bit-exact against the **real C++ `Gfx::RunOneFrame`** (G2e-1, 🎯 `shell_match_setup`). The sim is bit-exact against C++ on four generated levels that are not 504×350 (G3).

**Architecture.**
- `render` gains the text helpers `get_dims_h`, `draw_framed_text` and `blit_bitmap`, and `small_text::draw_text_small`. The three labels sit behind a new `Scene::small_labels: Option<SmallLabels>`, which is `None` on every golden path.
- `scenario` gains:
  - `build::apply_live_settings`, the one choke point `new_match_with` and RESUME share;
  - `assets::try_read_asset`;
  - `ConfigStore: Send + Sync` with `root_label()`, and a `Mutex` `MemoryStore`;
  - the oracle-only `generate <level_seed>` directive.
- `sim::weapsel` gains a pure `validate` and `set_weap_table`.
- `ui::shell` gains:
  - an ordered `InputEvent` stream;
  - the `InputString`, `InfoBox` and `WeaponOptions` screens, with push-after-update, overlays and scheduled replacement;
  - `cur_menu` in `MenuWorld`, the settings focus, the Enter dispatch and number entry;
  - Rust-only refusal boxes, and the RESUME resync;
  - `level_path`, pulled forward from 4½e-2 (fact 27);
  - a `ConfigStore` in `Shell` with `save_on_exit`.
- `oracle_dump_shell` grows opt-in `text`, `detail` (`d` lines) and `fs` (a two-layer file-system fixture through the real `paths::Resolve`, with `file` lines and intervention 9, the exit save). `oracle_dump_sim_physics` grows `generate`.
- `game` wires the stores (`--config-root`, the C++ config root natively, an in-memory store with the embedded setups in the browser), boot load and exit save, typed text, touch auto-repeat and the phone text field.

**Tech stack.**
- Rust 2021 for `sim-core`, `sim`, `render`, `scenario`, `shot` and `oracle-tests`; Rust 2024 for `ui` and `game`.
- C++ in `src/tools/oracle_dump/`, preset `linux-x64`, clang-format 22.
- HTML/JS in `web/index.html`.

**Spec:** `docs/superpowers/specs/2026-09-26-liero-rs-step4.5-slice4.5e-settings-menu-design.md`, cited as **design §N**; its findings are cited as **finding N**. The precedents are `plans/2026-09-26-liero-rs-step4.5-slice4.5d-plan.md` (**4½d plan**) and `plans/2026-09-25-liero-rs-step4.5-slice4.5c-plan.md` with its Addendum A.

**Rulings (John, 2026-09-26, design §14).** All six recommendations were accepted:
- **Q1.** Two parts; this is part 1.
- **Q2.** Starting or resuming Holdazone shows a Rust-only box, and the menu stays up.
- **Q3.** Desktop Rust uses the C++ config root (`paths::Resolve` semantics).
- **Q4.** Fix the shipped-level bug. A picked level is played. This is the one intended difference, and it is documented, not gated.
- **Q5.** On a phone, a text box raises the device keyboard; FIRE confirms and MENU cancels.
- **Q6.** Today's level set.

**Base.** The slice base is **`2377b0c`**, the 4½d merge. It is used for every golden audit, every clang-tidy diff and every review diff. HEAD at plan time was `b3f48d4` (the design and its rulings).

---

## Working environment (read before any task)

- **Repo and branch.** `/home/user/openliero` on `claude/cpp-oracle-vcpkg-assets-chcwcm`, with open draft PR `hamiltoon/openliero#16` into `liero-rs-step-4-5`. Use absolute paths. No sub-subagents.
- **Scratchpad.** `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad`. Every scratch script, PPM, PNG, probe and fixture copy goes here, never into the repo.
- **C++ oracle build.**
  - In the same shell as every cmake or gen-script call, first run `source $S/env.sh`. It sets `PRESET=linux-x64`, `VCPKG_ROOT`, the vcpkg asset script and the sdl3 overlay.
  - The build dir is `build/linux-x64` (Ninja Multi-Config, already configured with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON` and `CMAKE_EXPORT_COMPILE_COMMANDS`). Binaries go to `build/linux-x64/Release/`: `oracle_dump_shell`, `oracle_dump_sim_physics`, `oracle_dump_menu`, `oracle_dump_settings`, `oracle_dump_weapsel` and the real `openliero`.
  - Build a target with `cmake --build build/linux-x64 --config Release --target <t>`.
  - The gen scripts default to `macos-arm64` and honour `PRESET`, so always run `source $S/env.sh && bash rust/oracle-tests/<script>.sh` from the repo root.
- **clang-format is pinned to 22.** The system `clang-format` is 18 and must never be used.
  - Get the binary with `CF22=$(uvx --from clang-format==22.1.0 sh -c 'command -v clang-format')`.
  - Diff check: `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh 2377b0c`.
  - Whole-file check, for every touched C++ file (CLAUDE.md: a diff check misses context): `"$CF22" --dry-run -Werror --style=file <abs file>`.
  - Fix: `"$CF22" -i --style=file <abs file>`.
- **clang-tidy.** `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 2377b0c`. This clone has no `origin/master`, so always pass the base. It must exit 0. Fix the code. A `NOLINTNEXTLINE(<check>) — <reason>` is allowed only where the repo already uses one for the same check. `setenv` is fine: `concurrency-mt-unsafe` is disabled (`.clang-tidy:91`, `:117`).
- **Rust.** Run everything from the repo root, in **DEBUG only**. Never use `--release`: some tests are `#[should_panic]` on `debug_assert!`s.
  - The re-diff is **both**:
    - `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game`
    - `cargo test --manifest-path rust/Cargo.toml -p game`
  - **NEVER** run `cargo test --workspace` including `game`, because of disk space.
  - The wasm check is `cargo build --manifest-path rust/Cargo.toml --profile wasm-release -p game --target wasm32-unknown-unknown`.
  - Never use `--target-dir` or a second target tree.
- **Disk is tight.** About 5 GB is free, and `rust/target` is 24 GB. After every batch run `rm -rf rust/target/debug/incremental rust/target/wasm32-unknown-unknown/*/incremental`, and delete the scratch PPM/PNG dirs you created. Before a wasm-release build, check `df -h /` shows at least 3 GB free. If it does not, delete the incremental dirs first.
- **The native Rust `game` cannot run here.** Bevy panics with "Unable to find a GPU" under Xvfb. The **browser bundle** stands in for live smoke tests:
  - Build it with the wasm check above, then `wasm-bindgen --target web --out-dir $S/site --out-name game --remove-name-section --remove-producers-section rust/target/wasm32-unknown-unknown/wasm-release/game.wasm && cp web/index.html $S/site/ && rm -f $S/site/*.d.ts`.
  - Serve it with `cd $S/site && python3 -m http.server 8765`, run in the background. Check it with `curl -sI http://localhost:8765/`, and restart it if it is down.
  - Playwright lives under `/opt/node22/lib/node_modules/`. The scratchpad scripts `shell.mjs`, `t11.mjs` and `touch.mjs` are the templates: copy one, never edit it.
- **The real C++ game under Xvfb** (eyeball artefacts, never gates):
  - `Xvfb :99 -screen 0 1280x800x24 &`, then `DISPLAY=:99 SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy build/linux-x64/Release/openliero [--config-root <dir>] &`.
  - Drive it with `xdotool`: `$S/xd.sh` has `tap` / `snap`, and `$S/xd12.sh` is the 4½d path.
  - Screenshot with `import -window <id>`. Pair images with `$S/x12_pair.py`, and convert PPMs with `$S/ppm2png.py`.
- **Golden rule.** Never regenerate a golden to make Rust pass. The C++ output is the truth. Every existing golden must stay byte-identical, audited against `2377b0c`. If `git status --porcelain -- rust/oracle-tests/golden` ever shows an `M`:
  1. stop;
  2. `git checkout -- rust/oracle-tests/golden`;
  3. report the file and the first differing line;
  4. do not commit.
- **Commits.** Use the globally configured identity. Stage **explicit paths only**: never `git add -A` or `git add .`, because batches may run side by side. Every commit carries exactly these two trailer paragraphs, and no other text anywhere names a model:
  ```
  CO='Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>'
  SESS='Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT'
  git -C /home/user/openliero commit -m "<subject>" -m "$CO" -m "$SESS"
  ```
  Never write "Generated with …". **Do NOT push and do NOT open a PR.** The orchestrator pushes after each verified batch.

## Plan-time facts checked against the source (the source wins)

Every task below was written against HEAD `b3f48d4`. Where the design and the source disagree, the plan follows the source, and T11 records each item in PROGRESS and the maps.

1. **`cur_menu` is a `Gfx` member, not `MainMenuState` state** (`gfx.hpp:321`).
   - Readers:
     - `Gfx::DrawBasicMenu` draws the main menu *disabled* whenever `cur_menu != &main_menu` (`gfx.cpp:1702`);
     - `UpdateMenuPalettes` reads it (`gfx.cpp:990`, `:1000`; player menu only, 4½f).
   - `MainMenuState::Enter` resets it to the main menu (`mainMenuState.cpp:129`).
   - `WeaponMenuState::Draw` calls `DrawBasicMenu` while `cur_menu == &settings_menu` (`weaponMenuState.cpp:114-127`), so its main menu is drawn disabled.
   - **Rust:** `MenuWorld.cur_menu: CurMenu`, not a `MainMenuState` field (design §4.3 amended).
2. **The settings-focus Enter plays `MenuSelect` itself only in the four push arms** (LEVEL, WEAPON OPTIONS, LOAD SETUP, SAVE SETUP AS…; `mainMenuState.cpp:275-311`). The `default:` arm calls `settings_menu.OnEnter(common)` (`:313-314`), and the *behaviour* plays the sound: `integerBehavior.cpp:37`, `booleanSwitchBehavior.cpp:20-25`, `enumBehavior.cpp:24-29`. Every Enter still plays exactly one `MenuSelect` on its frame. Design §3.2's "MenuSelect, then the §3.1 Enter action" is exact only for the push arms.
3. **`menuStatePtr_` keeps pointing at the `MainMenuState` while sub-states sit above it.** It is set at push (`gfx.cpp:1463`, `:1623`) and cleared only at the router's dispatch (`:1494`). `kMenuSelection` and `kMenuFadingOut` (`:1488-1489`) therefore come from the main menu even when a `WeaponMenuState`, `InputStringState` or `InfoBoxState` is on top. Rust `Shell::frame` today reads them from the *top* only (`ui/src/shell/mod.rs:303-306`, "the menu state is on the stack only while it is the top"), which becomes false. **Rust:** take them from the stack's `MainMenu` screen, wherever it is.
4. **Sub-states are pushed *inside* `Update`, and `Push` runs `Enter` at once** (`state.hpp:50-54`). This happens in `mainMenuState.cpp:275-309` and `weaponMenuState.cpp:108`, and the rest of that `Update` keeps running.
   - **Rust:** the updating screen records a push request; the shell pushes it *after* the update and runs its `enter` then.
   - This is equivalent, because no pushed `Enter` reads anything the rest of that update changes:
     - `WeaponMenuState::Enter` reads only `weap_table` and the weapon names (`:26-42`);
     - `InputStringState::Enter` only calls `SDL_StartTextInput` (`inputState.cpp:25`);
     - `InfoBoxState::Enter` is empty.
   - The G2 generator also refuses any other key-down in a frame whose update pushed a screen (T8).
5. **`StateStack::Update` applies a scheduled replacement before the pop test** (`state.hpp:92-111`), and the replacement's `Enter` runs in `Push`. Rust `Shell::frame` has `AfterUpdate::Replaced => unreachable!` (`ui/src/shell/mod.rs:328`). It becomes: run the new top's `enter`, then continue to the draw. e-1 never schedules a replacement (the Save-As chain is e-2), but the arm is ported and unit-tested. A pop that leaves a non-empty stack (`Popped { empty: false }`) continues to the draw with the new top, as C++ does (`gfx.cpp:1631-1650`).
6. **`InputStringState::HandleEvent` handles every event in order, even after `done_`** (`inputState.cpp:27-72`). Text typed after Return in the same poll is still appended; `Update` acts on `done_` afterwards (`:75-84`). Backspace, Return, KP Enter and Esc are tested by **SDL scancode** on every key-down, repeats included.
   - **Rust:** tests the DOS codes 14, 28, 116 and 1 (`keys.cpp:9-36`). The mapping is one-to-one for those four keys.
7. **`Utf8ToDos` turns one whole `SDL_EVENT_TEXT_INPUT` string into one byte** (`text.cpp:99-118`):
   - a 1-byte string gives that byte;
   - the six 2-byte sequences å ä ö Å Ä Ö give CP437 0x86 0x84 0x94 0x8f 0x8e 0x99;
   - anything else, **including a multi-character string**, gives `'?'`.

   Design §4.2's "a frame that types "ab"" therefore means two text events. The filter test is `k && size < max_len && (!filter || (k = filter(k)))` (`inputState.cpp:58-61`).
8. **C++ does not draw a typed å.** `Font::DrawString` UTF-8-decodes (`font.cpp:62-64`), so a lone byte ≥ 0x80 decodes to U+FFFD (`cp437.cpp:189-217`). Every byte `Utf8ToDos` produces is a continuation byte (0x84–0x99), never a lead byte. **Rust:** the display string maps each buffer byte < 0x80 to itself and each byte ≥ 0x80 to `'\u{FFFD}'`, then draws through the existing `codepoint_to_font_byte`. This is exact. Design §4.5's "decodes them through the CP437 table" is wrong. Non-ASCII input stays outside the gate.
9. **The `InputStringState` strip restore is wider than the design says.** The call is `BlitBitmap(bmp, frozen, kClrX, y, kClrX + 10 + kWidth, 8)`, whose **width argument is `kClrX + 10 + kWidth`** (`inputState.cpp:92`, `kClrX = x - 10 - adjust`). Design §3.3 quotes the width as `10+w`. `BlitBitmap` clips through `CLIP_IMAGE` (`blit.cpp:217-233`; `render::blit::clip_image` already exists privately).
10. **`WeaponMenuState`'s two headers are exactly `Font::DrawFramedText`** (`font.cpp:82-85` against `weaponMenuState.cpp:120-124`): `(179, 20, "Weapon", 50)` and `(249, 20, "Availability", 50)`. `Settings::kExtensions` is `true` (`settings.hpp:14`), so PgUp and PgDn are live there (`weaponMenuState.cpp:77-87`).
11. **`WeaponMenuState` close rule** (`weaponMenuState.cpp:91-110`). On Esc, any keyboard player's Jump, or pad East/South: count the `weap_table` entries equal to 0 over all 40.
    - If the count is > 0, return false: the state pops that same frame.
    - Otherwise push `InfoBoxState(LS(NoWeaps), 223, 68, false)` on top, with no replace.

    Enter and Fire do nothing. Left and Right are *once* keys (`TestSdlKeyOnce`, `:66-76`).
12. **`InfoBoxState`** (`inputState.cpp:166-217`):
    - It is not an overlay.
    - It is done on any `SDL_EVENT_KEY_DOWN`, repeats included; a key-up never dismisses it.
    - `Update`: `ClearKeys`, the optional `Fill(bmp, 0)`, `on_dismiss`, pop.
    - `Draw`, when `clear_screen`: `pal = common.exepal` → `UpdatePal32` → `Fill 0`. Then `GetDims(text, &h)` and `DrawRoundedBox(x - w/2 - 2, y - h/2 - 2, 0, h + 1, w + 1)`, and the text in colour 6 at `(+2, +2)`.
    - `NoWeaps` is `"At least one weapon must\u0000be available in the menu!"` (`tc.cfg:258`). The Rust font already splits lines on `'\0'` (`render/src/font.rs:208`, `:231`), but `get_dims` returns no height; C++ `GetDims` gives `8 + 8 × NULs` (`font.cpp:87-112`).
13. **Number entry** (`integerBehavior.cpp:36-80`):
    - `kDigits = 1 + floor(log10(max / display_div))`; Rust `ui::menu::behavior::decimal_digits` already ports it.
    - The filter is `FilterDigits = isdigit(k) ? k : 0` (`:34`).
    - The callback runs on accept with a non-empty result: `atoi`, clamp to `[min/div, max/div]`, write `val × div`.
    - It then **always** rewrites the item's value from the field, with `%` when it is a percentage (`:60-75`). It never runs `UpdateItems`.
    - The request is already modelled as `ui::menu::Enter::EditValue(ValueEntry)` (`ui/src/menu/behavior.rs:9-29`).
14. **`lives` is not read per tick by a local match.** `game.cpp:159` is `Game::ResetWorms`, which only `RollbackController` calls (`rollbackController.cpp:392`, `:665`, `:749`). `LocalController` reads `settings->lives` once, at `kStateGame` (`localController.cpp:234`). Design §3.7 lists `lives` as live-read, which is wrong. It matters only when a match is paused *during weapon selection*, which the RESUME resync covers by refreshing `Match`'s whole `MatchConfig` (T4).
15. **A running `WeaponSelection` reads the live settings too.** It reads `game.settings->weap_table` on every cycle and RANDOMIZE step (`weapsel.cpp:255`, `:278`, `:327`) and `level_file` in `Draw` (`:107`, `:171`). But it counts `enabled_weaps` once, in the constructor (`:35-39`), and that count is never updated. Rust `sim::weapsel::WeaponSelection` stores `weap_table` and `enabled_weaps` at construction (`sim/src/weapsel.rs:201-208`). **Rust:** a `set_weap_table` that replaces `weap_table` only, called at RESUME (T2, T4).
16. **The live-read set** is re-derived with `grep -n 'settings->' game.cpp worm.cpp weapon.cpp nobject.cpp sobject.cpp bonus.cpp viewport.cpp`.
    - **Per-tick sim fields:**
      - `max_bonuses` (`game.cpp:219`, `:359`);
      - `weap_table` (`:258`);
      - `game_mode` (`:372`, `:522-537`, `worm.cpp:215`, `:384`, `:396`, `:794`);
      - `time_to_lose` (`game.cpp:385`, `:531`, `:537`);
      - `blood` (`worm.cpp:410`, `weapon.cpp:301`, `nobject.cpp:188`, `sobject.cpp:96`);
      - `loading_time` (`weapon.cpp:9`);
      - `load_change` (`worm.cpp:1079`);
      - `shadow` (`worm.cpp:784`, `:932`, `:942`, `weapon.cpp:121`, `nobject.cpp:123`, `:215`, `sobject.cpp:212`).
    - **Draw-only fields:** `map` (`viewport.cpp:593`), `names_on_bonuses` (`:408`, `:466`), `allow_viewing_spawn_point` (`:239`; hidden menu, 4½g).
    - **Holdazone only** (unported): `zone_timeout` (`game.cpp:427-432`, `:499`).
    - **Read at kStateGame / StartGame only:** `lives`, `blood_particle_max` (`game.cpp:513`).
    - **Per-worm** (`worm.settings->health`): 4½f.

    The Rust per-tick fields are `SimState::{settings_max_bonuses, weap_table, game_mode, time_to_lose, blood, settings_loading_time, load_change, shadow}` (`sim/src/state.rs:1045-1233`). None of them is hashed, so `apply_live_settings` is hash-neutral by construction.
17. **`Scene` already has a field called `labels`** (`render/src/frame.rs:47`, `&HudLabels`). The new switch is `Scene::small_labels: Option<SmallLabels<'a>>`.
    - `text.tga` (4×4, 26 frames) is loaded by no production path; only `oracle-tests/tests/sprite_golden.rs:49` loads it. `SceneData` gains `text_sprites`.
    - `sprite_pass` has 16 unit-test call sites (`render/src/object_draw.rs:915-1194`). It keeps its signature and delegates to a new `sprite_pass_with(…, labels)`.
18. **The three `DrawTextSmall` sites, exactly:**
    - **Bonus** (`viewport.cpp:408-413`): inside the flicker gate. Drawn when `names_on_bonuses && frame == 0`, with name `weapons[bonus.weapon]`, at `(Ftoi(x) - len*4/2, Ftoi(y) - 10) + offs`.
    - **Booby trap** (`:466-479`): per wobject, after its sprite or pixel. Drawn when `!h[HRemExp] && type index == 34 && names_on_bonuses && cur_frame == 0`, with name `weapons[slot % weapons.size()]`. The slot is `&*i - wobjects.arr`, which Rust reads as the `Pool` slot through `get(slot)` over `0..capacity()` (`sim/src/pool.rs:102-105`), so `sim` does not change.
    - **Change held** (`:575-581`): inside the viewport's *own* visible-worm crosshair block, after the crosshair blit. The test is `worm.Pressed(kChange)`, the **current** `control_states` bit (`worm.hpp:185`), not prev/current as design §4.10 says. Name `worm.weapons[current_weapon].type->name`, at `(Ftoi(pos.x) - len*4/2 + 1, Ftoi(pos.y) - 10) + offs`.

    `DrawTextSmall` (`common.cpp:227-237`) advances 4 px per byte and blits `text_sprites[c - 'A']` only for `c - 'A' < 26`, as an unsigned char. `tc.cfg:269` has `RemExp = false`, so the booby-trap label is reachable.
19. **`MemoryStore` is not `Sync`.** It uses a `RefCell` (`rust/scenario/src/storage.rs:11`, `:232`), while `ShellRes` is a Bevy `Resource` (`rust/game/src/main.rs:161`). A store inside `Shell` must be `Send + Sync`. **Rust:** use a `std::sync::Mutex`, and make it a `ConfigStore: Send + Sync` supertrait.
20. **`scenario::build::validate(cfg: &MatchConfig, n_weapons)` already exists** (`build.rs:98`), with `validate_for_selection` (`:103`). Design §4.1's new `validate(&Settings)` would collide. The shell composes the existing validators with `sim::weapsel`'s refusals (T2's `WeaponSelection::validate`) into `ui::shell::refusal` (T4).
21. **`boot_state` panics on a C++-saved setup with unequal health.** It calls `new_match(...).expect(..)` (`ui/src/shell/playing.rs:59`), and `new_match` refuses `AsymmetricHealth`. Under Q3, Rust now boots from the C++ user's `liero.cfg`, so this becomes reachable. **Rust:** the boot game is never processed, so the boot build sanitises a copy: Holdazone → Kill'em All (4½d), health → player 1's, and any other refusal → `Settings::default()` level-independent fields. It never panics. The refusal box comes later, at NEW GAME (T4).
22. **`generate_level` panics on a missing level file** (`ui/src/shell/new_game.rs:38`, `read_asset`). Under Q3, a C++ user's `level_file` is a config-root path such as `/home/u/.local/share/openliero/openliero/TC/openliero/Levels/x.lev` (finding 3), and the file may exist only in the system layer (finding 2). That is a live **crash** in e-1. **Fact 27** pulls the design's e-2 `level_path` forward.
23. **`gen_shell_golden.sh` hard-codes the 4½d shape.** It checks `test "$n" -eq 11`, 11-field `f` lines, `upd ∈ {M,W,G}` and `top ∈ {M,G,-}` (`rust/oracle-tests/gen_shell_golden.sh:25-58`). `shell_golden.rs::the_committed_scripts_are_the_generators` asserts that the on-disk `shell_*_script.txt` set equals the 4½d 11 (`rust/oracle-tests/tests/shell_golden.rs:150-172`). Both must learn the e-1 case list.
24. **`oracle_dump_shell` knows only 4½d.**
    - `TopOf` `Fail`s on any state other than main menu or game-play (`shell_dump.cpp`, `TopOf`).
    - `ScancodeOf` has no BACKSPACE.
    - The header comment block is written unconditionally and must stay byte-identical for the 4½d cases.
    - `settings->record_replays = false` (intervention 6) would leak into an exit save and into any settings hash (T5's `cfg16`). The Rust harness mirrors it (T8).
25. **C++ internals a dumper cannot read.** `WeaponMenuState::weaponMenu_` and `InputStringState::buffer_` are private (`weaponMenuState.hpp`, `inputState.hpp:26-35`). Design §6.1's `d … <sub_sel>` field is therefore dropped. The `d` line instead pins the *whole settings model* every frame with `cfg16`, an FNV-1a-64 of `gfx.settings->ToToml()`. Rust's `settings_to_toml` is byte-identical to it (4½a-2 G5a). `HashGameState` is `uint32_t` (`stateHash.hpp:15`), so the state field is 8 hex digits, not 16.
26. **C++ `Level::GenerateFromSettings`** (`level.cpp:397-429`) runs `GenerateRandom` or the file; on a failed file it falls back to random. It records `old_*` and runs `MakeShadow` when `shadow` is on. `oracle_dump_sim_physics` only *loads* levels today (`sim_physics_dump.cpp:505-514`, and its header "must NOT call … GenerateFromSettings" at `:33-36`). `generate` changes that for its own directive only, with a dedicated `Rand` so `game.rand` is untouched.
27. **Plan decision, from facts 21 and 22:** the design's 4½e-2 `ui::shell::level_path` (rules 1–4), `ConfigStore::root_label` and `generate_level(…, file: Option<LevelData>, …)` move into e-1 (T2, T3), so a C++ user's `liero.cfg` never crashes the Rust boot. e-2's T0 shrinks to `list`, the wasm system layer and the canonical `?level=`. No G2e-1 case uses a file level through `fs`; the e-1 fixtures are all random-level. The rules are unit-gated here and C++-gated in e-2.
28. **The fixture root label is relative.** `paths::UserDataRoot` with `OPENLIERO_TEST_USER_DIR=user` returns `FsNode("user")`, and the joined config node's `FullPath()` is the user node's (`filesystem.cpp:356-362`, `:659-667`). T0 records the exact strings. `SystemDataRoot` needs the directory to exist (`:681-691`), and `Resolve` checks for `portable.txt` next to the binary (`:822-825`); `build/linux-x64/Release/portable.txt` is absent at plan time.
29. **`FsNode` writes create parent directories** (`FsNodeFilesystem::TryToWriter`, `filesystem.cpp:572-584`, as `storage.rs`'s `write` doc says). T0 re-verifies this: `cfg_default`'s boot save writes `user/Setups/liero.cfg` with no `Setups/` present.
30. **Refusal-relevant `sim::weapsel` order.** `WeaponSelection::new` refuses `WeaponCount`, `WormCount`, `InvalidPick`, then `NoWeaponsEnabled`, all before the first draw (`sim/src/weapsel.rs:186-204`). T2 factors that check out, unchanged, as `WeaponSelection::validate`.

## Decisions this plan makes (the design left them open)

- **D1. The `d` line.**
  - Format: `d <frame> <cur> <ssel> <cfg16> <state8|->`.
  - `cur`: `M` or `S`, from `gfx.cur_menu`.
  - `ssel`: `settings_menu.Selection()`.
  - `cfg16`: FNV-1a-64 of `ToToml()`.
  - `state8`: `%08x` `HashGameState`, only on frames whose top after the frame is `G` and whose controller is not in weapon selection; otherwise `-`.
  - There is no `sub_sel` (fact 25).
- **D2. The `fs` fixture is a manifest file**, `shell_<case>_fs.txt`, with `dir <user|sys> <rel>` and `file <user|sys> <rel> <repo-relative source>` lines (§Formats). `fs` excludes `setup`.
- **D3. `file` lines are written for every `fs` case after the `end` line.** The exit save (intervention 9) runs first, and only when the case ended by QUIT. This also pins `cfg_default`'s boot-time defaults save.
- **D4. The search-gap check** (design §6.1). The dumper fails a case when two frames of one continuous `O`-on-top visit carry printable key-downs ≥ 1000 ms apart by `SDL_GetTicks()`. The Rust harness passes `now_ms = 0`, so neither side ever reaches the 1500 ms timeout (`ui/src/menu/search.rs:44`).
- **D5. The Rust-only refusal texts, drawn at (160, 100) with `clear_screen = false`:**

  | Refusal | Text |
  |---|---|
  | `HoldazoneUnsupported` | `HOLDAZONE IS NOT\0SUPPORTED YET` (Q2) |
  | `AsymmetricHealth` | `BOTH PLAYERS NEED\0THE SAME HEALTH` |
  | `NoWeaponsEnabled` | the TC's own `NoWeaps` |
  | anything else | `THIS SETUP CANNOT\0BE PLAYED YET` |

  The Enter that triggered the refusal already played its `MenuSelect`; the box adds no second sound, and F1 plays none, as C++'s F1 plays none. `eprintln!`/`console.warn` logs the `Display` text.
- **D6. The in-e-1 LEVEL / LOAD SETUP / SAVE SETUP AS… Enters are Rust-inert.** They play the `MenuSelect` C++ plays, and push nothing until e-2. They are unit-tested only, and the G2 generator refuses them (C++ would push unported selectors).
- **D7. `attached: bool` lands on `Match` in e-1.** It is set true at NEW GAME. No e-1 path clears it (e-2's LOAD SETUP will). A test-only detach covers the false arm.
- **D8. Live typed text** (native and browser keyboard): every `KeyboardInput` key-down whose `text` is `Some(t)` with no control characters (`< 0x20`, `0x7f`) becomes one `InputEvent::Text` **per `char`**, after that key's `InputEvent::Key`. SDL would send an IME multi-char commit as one event, which gives `'?'` (fact 7); splitting is a Rust-only live convenience and stays outside the gate.
- **D9. The phone text field** (Q5). While `window.lieroPhase === "text"` on a touch page (`window.lieroTouchOnly`):
  - The page shows a hidden `<input id="text-entry">`, pre-filled with a two-space sentinel, and tries `focus()` at once. That works on Android.
  - It also shows a "TAP TO TYPE" button that focuses the input from a real tap. iOS raises its keyboard only from a user gesture.
  - `input` events are diffed against the sentinel: appended chars become text entries and deletions become Backspace. Enter becomes Return. The sentinel is then restored.
  - Entries go into `window.lieroText` (an array the glue drains each tick).
  - While the phase is `text`, FIRE sends Return and MENU sends Esc.
- **D10. Native store selection** (T10).
  - `--config-root <dir>` or `--config-root=<dir>` → `NativeStore::single_dir`.
  - Otherwise `NativeStore::resolve_default()`, with `OPENLIERO_TEST_USER_DIR` honoured.
  - If neither resolves, `MemoryStore::new()` with a warning.
  - `portable.txt` next to the Rust binary is not supported. There is no Rust install layout yet; this is recorded.
- **D11. The browser's in-memory store** carries only `Setups/liero.cfg` and `Setups/orbmit.cfg` in its system layer in e-1. The level catalogue is e-2. The root label is `/openliero`.
- **D12. The `rust/settings` crate move** (PROGRESS's old 4½e line, "move settings/settings_toml/toml_fmt/storage into rust/settings") is not in the design and not in e-1. T11 records it as still open.

## Standing ruling (4½c Addendum A, carried forward)

The C++ comparison happens **here**, against the real C++ run headlessly. If a dumper cannot run the real code headlessly, the task **stops and reports the exact blocker**. It never weakens a gate and never regenerates a golden to paper over a mismatch. The real `openliero` under Xvfb produces C++ | Rust side-by-side PNGs. Those are eyeball artefacts, never gates, and are never committed.

## Global constraints

- **LD 1:** pixel-exact menus on the 320×200 CPU surface.
- **LD 3, amended by finding 1.** `tick_and_render` stays the only `ResMut<Sim>` holder, and menus get no `SimState`. Inside the shell, only `Match::process` and the router touch the sim. The router *replaces* it at NEW GAME and, since this slice, runs `apply_live_settings` at RESUME.
- **LD 4, amended.** One new oracle-only directive, `generate <level_seed>`. It needs `settings`, excludes `level`, and `scenario::load` refuses it. Both parsers change in lockstep: Rust in T2, C++ in T6. There is no other grammar change.
- **LD 5.** `SimState::new`'s signature is unchanged.
- **Crate rules.**
  - `sim-core` stays dependency-free, and `sim` gains no dependency.
  - `ui`, `render`, `scenario` and `shot` stay Bevy-free.
  - `ui` adds no external dependency: `cargo tree -p ui -e normal | grep -c bevy` prints `0`.
- **Determinism.** Use no floats, no wall clock, and no `HashMap`/`HashSet` iteration in `ui` or `render`. The only clocks are the caller's `ShellInput::{fresh_seed, now_ms}`.
- **`sim` may change only for:**
  - T2's `WeaponSelection::{validate, set_weap_table}`;
  - a bug a G3 or G2 case *proves*. That fix gets its own unit test and its own commit, and the full re-diff must pass.
- **C++ changes are confined to** `src/tools/oracle_dump/shell_dump.cpp` and `src/tools/oracle_dump/sim_physics_dump.cpp`. `CMakeLists.txt` is unchanged, because no new target is needed. `weapsel_drive.hpp` is included, never modified.
- **Frozen provenance.** Never edit these:
  - `examples/gen_slice4_5a.rs`, `gen_slice4_5c0.rs`, `gen_slice4_5c.rs`;
  - `tests/weapsel_common/`, `tests/sim_slice4_5c0_common/`;
  - `menu_dump.cpp`, `weapsel_dump.cpp`, `weapsel_drive.hpp`, `gen_menu_golden.sh`, and every `golden/*` that exists at `2377b0c`.

  4½d's `examples/gen_slice4_5d.rs` is **not** edited. It must still regenerate its 11 scripts byte-identically, because `tests/shell_common/mod.rs`, which it shares, changes.
- **Golden audit.** `git diff --name-status 2377b0c -- rust/oracle-tests/golden` may list **only `A` lines**, exactly the **39 files** in §File structure: 27 `shell_*` and 12 `sim_slice4_5e_*`. *(As landed: **42** — 30 `shell_*`, because Batch 7 added the `key_edges` case's script, golden and setup, plus the 12 `sim_slice4_5e_*`.)*
- **rustfmt.** Run it only on files a task *creates*: `rustfmt --edition 2024 <abs file>` for `ui`/`game`, `--edition 2021` elsewhere. Never run it on an existing file or on a `lib.rs`/`main.rs`, because it recurses. Hand-format edits to match the surrounding style.
- **The non-shell paths stay behaviour-identical:** `--live [<scenario>]`, `--live --record`, `--replay`, `Scripted`, `?demo`. `game/tests/round_trip.rs` and `record_regression.rs` stay green and untouched.

## Formats pinned (both sides implement exactly this)

**Script directives** (`shell_<case>_script.txt`). The 4½d directives are unchanged: `setup`, `boot_seed`, `match_seed`, `frames`, `expect`, and `key <frame> down|up|repeat <NAME>`. New:

```
detail                         # opt-in: a `d` line after every `f` line
fs <manifest>                  # opt-in, golden-dir-relative; requires `setup default`
text <frame> <hex>             # one SDL_EVENT_TEXT_INPUT; <hex> = the UTF-8 bytes, lowercase,
                               # 2..8 hex digits; the dumper keeps the string alive all run
```

`key` and `text` lines are merged into one event list and ordered by frame. **Within a frame, the file order is kept** (a stable sort), and that is the SDL event order. Key names gain `BACKSPACE`: SDL `SDL_SCANCODE_BACKSPACE`, DOS 14, `key_buf` symbol 8. That symbol is outside 32..127, so a search ignores it.

**`fs` manifest** (`shell_<case>_fs.txt`; `#` comments and blank lines allowed):

```
dir  <user|sys> <rel>                 # an empty directory
file <user|sys> <rel> <source>        # <source> is repo-relative (e.g. data/Setups/liero.cfg or
                                      # rust/oracle-tests/golden/shell_cfg_boot_user_liero.cfg)
```

- `<rel>` uses forward slashes. It is non-empty, with no `.`, `..`, absolute path or `\`.
- Both sides always create `user/` and `sys/`. Either side refuses any other line.
- **C++ side:** materialise into `std::filesystem::temp_directory_path() / "oracle_shell_fs_<case>"` (`remove_all` first), with copies and no symlinks. Load the TC from the repo *before* the `chdir`. Then `chdir` into the fixture, `setenv OPENLIERO_TEST_USER_DIR=user` and `OPENLIERO_DATADIR=sys`, and run the real `paths::Resolve` (argv `{"oracle_dump_shell"}`). Then `gfx.SetConfigNodes`, and boot as `GameEntry` does (`gameEntry.cpp:52-58`): `if (!gfx.LoadSettings(config/Setups/liero.cfg)) { settings = make_shared; SaveSettings(user/Setups/liero.cfg); }`. Intervention 6 comes next. `remove_all` the fixture after writing the output.
- **Rust side:** materialise into `std::env::temp_dir()/liero_rs_shell_fs_<case>_<pid>`. Build `NativeStore::split(tmp/user, Some(tmp/sys)).with_root_label("./user")`, then run `load_setup(&store)`, then set `record_replays = false` (the harness mirror of intervention 6). Boot the shell with that store, and remove the directory at the end.

**Golden lines** (`shell_<case>.txt`):

```
# oracle_dump_shell … (the 4½d header, byte-identical)             always
# d <frame> <cur> <ssel> <cfg16> <state8|->                        only with `detail`
# file <rel> <fnv16>                                               only with `fs`
boot <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> <sel>             unchanged
f <frame> <upd> <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> <sel> <sounds>
d <frame> <M|S> <ssel> <cfg16:%016x> <state8:%08x or ->            with `detail`, after its f
end <frame> quit|frames                                            unchanged
file <rel> <fnv16:%016x>                                           with `fs`, after end, rel sorted bytewise
```

- `upd` ∈ `M W G O I B`; `top` ∈ `M G O I B -`. `O` is `WeaponMenuState`, `I` is `InputStringState`, `B` is `InfoBoxState`.
- `cfg16` and `fnv16` use the frames' FNV-1a-64 constants (`kFnvOffset`, `kFnvPrime`) over the bytes.
- `file` lines cover every regular file under `<fixture>/user`, recursively, with `rel` relative to `user/`.

**Intervention 9** (the exit save). When an `fs` case ends by `quit`, the dumper runs `gfx.settings->save(gfx.GetUserConfigNode() / "Setups" / "liero.cfg", gfx.rand)`. That is `gameEntry.cpp:78` verbatim, which the dumper never reaches. The Rust side is `Shell::save_on_exit()`.

**`generate <level_seed>`** (sim scenarios):
- exactly one `u32`; a `#` token starts a comment;
- requires `settings`;
- excludes `level`, and exactly one of the two is required.

The dumper builds the level with the **real** `game.level.GenerateFromSettings(*common, *settings, r)`, where `Rand r; r.Seed(level_seed)`, and requires `settings->random_level == true`. Rust uses `sim::levelgen::generate_from_settings` with `file = None` and a `Rand` seeded with `level_seed`, then `build_match`. Rust `Scenario` keeps `level: String` (empty for a `generate` scenario) and gains `generate: Option<u32>`. `to_text` writes `generate N` where `level` would be.

## File structure

| File | Task | Responsibility |
|---|---|---|
| `rust/render/src/font.rs` (modify) | T1 | `get_dims_h` (width, height), `draw_framed_text` |
| `rust/render/src/blit.rs` (modify) | T1 | `blit_bitmap` (the ARGB copy through `clip_image`) |
| `rust/render/src/small_text.rs` (create), `lib.rs` (modify) | T1 | `draw_text_small` |
| `rust/render/src/object_draw.rs`, `frame.rs` (modify) | T1 | `SmallLabels`, `sprite_pass_with`, the three labels; `Scene::small_labels` |
| `rust/scenario/src/loader.rs` (modify) | T1 | `SceneData::text_sprites`; `as_scene` gives `small_labels: None` |
| `rust/oracle-tests/tests/render_slice{3a_golden.rs,3b_common/mod.rs,3e_common/mod.rs,4d_common/mod.rs}` (modify) | T1 | the `small_labels: None` field in the `Scene` literals |
| `rust/scenario/src/build.rs` (modify) | T2 | `apply_live_settings` + the fact-16 table; `new_match_with` uses it |
| `rust/scenario/src/assets.rs` (modify) | T2, T10 | `try_read_asset` (T2); the embedded `Setups/*.cfg` for wasm (T10) |
| `rust/scenario/src/storage.rs` (modify) | T2 | `ConfigStore: Send + Sync` + `root_label`; `NativeStore::with_root_label`; `MemoryStore` with `Mutex` + `with_root_label` |
| `rust/scenario/src/parser.rs` (modify) | T2 | `generate` |
| `rust/sim/src/weapsel.rs` (modify) | T2 | `WeaponSelection::validate` (factored from `new`), `set_weap_table` |
| `rust/ui/src/text.rs` (modify) | T3, T4 | `utf8_to_dos`, `WEAP_STATES`, `UiTc` weapon names/order + texts + `exepal` (T3); refusal texts (T4) |
| `rust/ui/src/keys.rs` (modify) | T3 | `DK_BACKSPACE` |
| `rust/ui/src/shell/overlay.rs` (create) | T3 | `InputStringState`, `InfoBoxState`, `InputPurpose`, `InfoPurpose` |
| `rust/ui/src/shell/level_path.rs` (create) | T3 | `read_level` (rules 1–4) |
| `rust/ui/src/shell/{mod.rs,stack.rs,new_game.rs,level_slot.rs,main_menu.rs}` (modify) | T3 | `InputEvent`, the sub-screen stack, `cur_menu`, the store in `Shell`, `generate_level(…, Option<LevelData>, …)` |
| `rust/ui/src/shell/weapon_options.rs` (create) | T4 | `WeaponMenuState`, `WeaponModel` |
| `rust/ui/src/shell/{main_menu.rs,settings_menu.rs,playing.rs,mod.rs}` (modify) | T4 | focus, Enter dispatch, entry continuation, refusals, resync, `boot_state` sanitising, `save_on_exit`, debug hooks |
| `rust/game/src/{main.rs,input.rs,touch.rs}`, `rust/shot/src/lib.rs` (modify) | T3 (mechanical), T10 | `InputEvent` wrapping + the store argument (T3); the live wiring (T10) |
| `rust/game/src/config.rs` (create), `lib.rs` (modify) | T10 | store selection (`--config-root`, resolve, fallback), the `window.lieroText` drain |
| `web/index.html`, `.github/workflows/preview.yml` (modify) | T10 | the text field, the hint, the help text |
| `src/tools/oracle_dump/shell_dump.cpp` (modify) | T5 | tops `O/I/B`, `text`, `detail`, `fs`, intervention 9, the gap check, BACKSPACE |
| `src/tools/oracle_dump/sim_physics_dump.cpp` (modify) | T6 | `generate` |
| `rust/oracle-tests/gen_shell_golden.sh` (modify) | T5 | the new tops and lines; the count 21 |
| `rust/oracle-tests/examples/gen_slice4_5e1_sim.rs` (create) | T7 | G3 `cfg` / `scan` / `gen` |
| `rust/oracle-tests/gen_sim_slice4_5e_golden.sh` (create) | T7 | G3 C++ goldens + awk gates |
| `rust/oracle-tests/tests/sim_slice4_5e_generated_golden.rs` (create) | T7 | the G3 gate |
| `rust/oracle-tests/golden/sim_slice4_5e_{small,odd,tall,banned}{_scenario.txt,_setup.cfg,.txt}` (create, **12**) | T7 | G3 corpus + goldens |
| `rust/oracle-tests/tests/shell_common/mod.rs` (modify) | T3 (mechanical), T8 | events/`text`/`detail`/`fs` in the script model and driver; `d` + `file` lines |
| `rust/oracle-tests/tests/shell_e1_cases/mod.rs` (create) | T8 | the 10 e-1 cases (as landed: 11, with `key_edges`), their builders, validators, ledgers |
| `rust/oracle-tests/examples/gen_slice4_5e1_shell.rs` (create) | T8 | `check` / `write` |
| `rust/oracle-tests/golden/shell_*` e-1 (create, **27**; as landed **30**) | T8 | 10 scripts + 10 goldens; `shell_{weapon_options,labels}_setup.cfg`; `shell_{cfg_boot,cfg_default,match_setup}_fs.txt`; `shell_{cfg_boot,match_setup}_user_liero.cfg`; as landed also `shell_key_edges{_script.txt,.txt,_setup.cfg}` |
| `rust/oracle-tests/tests/shell_golden.rs` (modify) | T9 | the e-1 cases, `d` + `file` comparison, the union case list, the milestone |
| PROGRESS, overview, design status, cpp-map, rust-map, `.claude/skills/liero-shot/SKILL.md` | T11 | status + corrections |

## Batches, order and parallelism

| Batch | Tasks | Needs | Runs alongside | Touches | Gate |
|---|---|---|---|---|---|
| **1** | T0 probes | — | — (first, alone) | `shell_dump.cpp` temporarily (restored), scratchpad, this plan (addendum) | the probe outputs recorded; the contradiction rule applied |
| **2** | T1, T2 | 1 | **3** | `rust/render`, `rust/scenario`, `rust/sim/src/weapsel.rs`, 4 render-test harnesses | the full re-diff (both commands); `git status --porcelain rust/oracle-tests/golden` empty |
| **3** | T5, T6 | 1 | **2, 4, 5** (C++ only) | `shell_dump.cpp`, `sim_physics_dump.cpp`, `gen_shell_golden.sh` | builds; clang-format 22 (diff + whole file) + clang-tidy; the 11 4½d shell goldens and all 39 `oracle_dump_sim_physics` gen scripts regenerate byte-identically; the scratch smokes |
| **4** | T3 | 2 | 3 | `rust/ui`; mechanical: `rust/game/src/{main,input,touch}.rs`, `rust/shot/src/lib.rs`, `tests/shell_common/mod.rs` | the full re-diff; the wasm check; `gen_slice4_5d -- check`; `shell_golden` (4½d G2) green |
| **5** | T4 | 4 | 3 | `rust/ui` (+ callers if a signature moves) | the full re-diff; the wasm check; G1 + 4½d G2 green |
| **6** | T7 (G3) | 2, 3 | 4/5 **only in a separate worktree** | new G3 files in `rust/oracle-tests`; `rust/sim` (proven fixes only) | **G3 bit-exact**; the full re-diff |
| **7** | T8, T9 | 3, 5 | 8 **only in a separate worktree** | `rust/oracle-tests` (shell); `rust/ui` / `rust/render` (proven fixes only, no pub-API change `game` uses) | **🎯 G2e-1 bit-exact** (every `f`/`d`/`file` line); the full re-diff |
| **8** | T10 | 5 | 7 **only in a separate worktree** | `rust/game`, `web/index.html`, `.github/workflows/preview.yml`, `rust/scenario/src/assets.rs` (the wasm embed) | `cargo test -p game`; the wasm check; the bundle + Chromium walk |
| **9** | T11 | 6, 7, 8 | — (last) | docs, skill | the full board + every audit + the broad review |

**Recommended schedule:** 1 → (2 ∥ 3) → 6 → 4 → 5 → 7 → 8 → 9.
- G3 goes early on purpose. It is the risk-first gate (design §10: "Expect a real find here"), and a sim fix is best landed before the UI builds on it.
- Batch 3 may keep running while Batches 4 and 5 run, because it is C++-only. Its build tree (`build/linux-x64`) is disjoint from `rust/target`.

**Rust ∥ Rust needs separate worktrees.** Batches 6 ∥ 4/5 and 7 ∥ 8 are file-disjoint, but in one checkout one agent's half-edited crate breaks the other's `cargo test`. Run such pairs **only** in separate `git worktree`s sharing `CARGO_TARGET_DIR=/home/user/openliero/rust/target`, and only after `df -h /` shows at least 8 GB free. Otherwise run them sequentially.

**Design ↔ plan mapping:**

| Design §8 (4½e-1) | This plan |
|---|---|
| T0 | T0 |
| T1 | T1 |
| T2 | T2 |
| T3 | T3 |
| T4 | T4 |
| T5 | T5 + T6 |
| T6 | T7 |
| T7 | T8 |
| T8 | T9 |
| T9 | T10 |
| T10 | T11 |

---

### Task 0 (Batch 1): the C++ probes of findings 1 and 2

Finding 1 is probed with the **real** `Gfx::RunOneFrame`, through a temporary, never-committed patch of `oracle_dump_shell`. Finding 2 is probed with the **real** `openliero` under Xvfb in the default split layout. Neither probe adds code to the repo.

**Files:** `src/tools/oracle_dump/shell_dump.cpp` (patched temporarily, restored at the end), everything under `$S/t0e1/`, this plan (the addendum).

- [ ] **Step 1: patch the dumper, in the working tree only.**
  - `TopOf` returns `'?'` for an unknown state instead of `Fail`.
  - Every `f` line gets one extra field: `%08x` of `HashGameState(*gfx.controller->CurrentGame())` when the top is `G` and `!gfx.controller->InWeaponSelection()`, else `-`.

  Build: `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell`.
- [ ] **Step 2: three scratch scripts** in `$S/t0e1/`, identical except the edit, with equal frame counts: `p_ctl`, `p_bonus`, `p_map`.
  - Common path: `setup default`, `boot_seed 7`, one `match_seed 301`, frames about 700, `expect frames`. Then idle 40 → `RETURN` → +31 → both players `R`+`UP`, then `LCTRL`+`RCTRL` (DONE) → about 200 match frames with P1 `G` + `LCTRL` taps and P2 `LEFT` + `RCTRL` taps, all released → `ESC` → +31 (back in the menu) → idle 20 → **edit** → `ESC` (focus main) → `F1` (RESUME) → +31 → about 150 frames.
  - **Edits.** Each begins with `F7`. The settings cursor starts on GAME MODE, and the default visible order is GAME MODE, LIVES, LEVEL, MAP WIDTH, MAP HEIGHT, LOADING TIMES, WEAPON OPTIONS, MAX BONUSES, NAMES ON BONUSES, MAP, …
    - `p_ctl`: no further key; the same number of idle frames.
    - `p_bonus`: `DOWN` ×7 (MAX BONUSES), then hold `LEFT` for 25 frames. MAX BONUSES steps 4 → 0 at the `menu_cycles % 5` cadence, all within 20 frames.
    - `p_map`: `DOWN` ×9 (MAP), then `RETURN` (the toggle).
- [ ] **Step 3: run and diff.** `build/linux-x64/Release/oracle_dump_shell $S/t0e1/p_<v>.txt $S/t0e1/p_<v>.out` for each script, then diff `p_ctl.out` against the other two with `diff <(cut -d' ' -f1-12 …)` and friends. **Expected (finding 1 confirmed):**
  - every line before the edit is identical;
  - `p_map`: the first resumed `f` line's `bmp16` differs (the minimap is gone), and its state hashes equal `p_ctl`'s for at least 50 ticks;
  - `p_bonus`: the state hash differs within the first 3 resumed ticks. With `max_bonuses 0`, the `game.cpp:359` roll no longer draws `rand`.
- [ ] **Step 4: restore.** `git -C /home/user/openliero checkout -- src/tools/oracle_dump/shell_dump.cpp`, then rebuild `oracle_dump_shell`. `git status --short src` must be empty.
- [ ] **Step 5: finding 2 with the real game.**
  - Start `Xvfb :99 …`. For each run R ∈ {A, B, C}, drive the game with `$S/xd.sh`'s `tap`/`snap` and save screenshots to `$S/t0e2/<R>_*.png`.
    - **A (split):** `U=$S/t0e2/A/user; rm -rf $S/t0e2/A; mkdir -p $U`; run `OPENLIERO_TEST_USER_DIR=$U OPENLIERO_DATADIR=/home/user/openliero/data … build/linux-x64/Release/openliero`.
    - **B:** A, plus `mkdir -p $U/TC/openliero/Levels && cp data/TC/openliero/Levels/water_stage.lev $U/TC/openliero/Levels/`.
    - **C (portable):** `cp -r data $S/t0e2/C/root` and run with `--config-root $S/t0e2/C/root`.
  - Path, in each run: `F7`, then `DOWN` ×2 to LEVEL, then `Return`.
    - Snap the root listing: `[RANDOM]`, Profiles, Resources, Setups, TC; the title shows the full path.
    - Then `DOWN` to TC, `Right` into `openliero`, `Right` into `Levels`; for C, move to the `Levels` row first.
    - Type `water` (the substring search), `Return`, and snap: LEVEL shows `"water_stage"`.
    - `Escape` → `F1` (NEW GAME), and snap the weapon-selection screen with the level behind it.
    - `Escape`, `Escape`, and `Return` on QUIT TO OS.
  - Then `grep -E 'levelFile|randomLevel' <user or root>/Setups/liero.cfg`.
  - Adjust the key count if a listing differs, and record what the screen showed. **Expected (findings 2, 3, 12 confirmed):**
    - A's selection screen shows a *random* level, and B's and C's show `water_stage`;
    - every saved `levelFile` is `<user-or-root FullPath>/TC/openliero/Levels/water_stage.lev`. Record the exact prefix string: the trailing-separator form fixes `NativeStore::root_label`.
- [ ] **Step 6: fixture facts.** In the A run's user dir, the boot fallback wrote `Setups/liero.cfg` with no `Setups/` directory present beforehand (fact 29). Record `ls -R $U` after the run.
- [ ] **Step 7: record and commit.** Append **"## Addendum T0 (probe results)"** to this plan. It holds the command lines, the first differing frame of each variant, the three screenshots' verdicts (the PNGs stay in `$S/t0e2`), the saved `levelFile` strings and the `ls -R`. Then `git add docs/superpowers/plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md` and commit `docs(4.5e-1): T0 — C++ probes of findings 1 and 2 (results in the plan)` with the trailers.

**If a probe contradicts the design** (the source wins; no question for John, because these are findings, not product choices):
- **Finding 1 fully contradicted** (`p_map` and `p_bonus` both match `p_ctl` after RESUME):
  - T4 drops the RESUME resync, and `Match` keeps its start-time copy.
  - T2's `apply_live_settings` still lands, as `new_match_with`'s choke point.
  - `live_settings` (T8) stays in the corpus and then gates the *absence* of sharing.
  - Rewrite fact 16's conclusion and design finding 1 in the addendum, and tell the orchestrator.
- **Finding 1 partly contradicted** (draw-read fields shared but sim fields not, or the reverse):
  - Split the resync to exactly the proven set: `Match::resync` for the draw fields, `apply_live_settings` for the sim fields.
  - Add a third probe script for whichever of `blood` / `loading_time` / `weap_table` was not separately shown, before T4 relies on it.
- **Finding 2 contradicted** (A plays `water_stage`): Q4's fix is a no-op. `level_path` rule 1 reads the merged view either way. Record "Q4 moot", and nothing else changes.
- **Finding 3 or 12 differs** (another `levelFile` form, or no save at exit): adopt the observed form for `root_label` (T2), and record it.
- **A probe cannot run** (the patched dumper fails to build, or the game does not start under Xvfb):
  - For finding 1: **stop and report the blocker**. T4's resync depends on it.
  - For finding 2: record "unconfirmed" and proceed. Q4 = A covers both outcomes, and e-2's `fs` G2 cases will show it.

**Done when:** the addendum records each probe's outcome, the contradiction rule has been applied where needed, `src/` is clean, and the addendum commit exists.

---

### Task 1 (Batch 2): `render`, the text helpers and the three small labels (hash-neutral)

**Files:**
- `rust/render/src/{font.rs, blit.rs, object_draw.rs, frame.rs, lib.rs}`;
- `rust/render/src/small_text.rs` (new);
- `rust/scenario/src/loader.rs`;
- the four `Scene`-literal harnesses in `rust/oracle-tests/tests/` (see §File structure).

- [ ] **Step 1: `Font::get_dims_h(&self, s) -> (i32, i32)`.** Width as `get_dims`; height `8 + 8 × ('\0' count)` (`font.cpp:87-112`). `get_dims` keeps its signature and delegates. **Test:** `"A\0BB"` gives `(w('B')·2, 16)`, and `""` gives `(0, 8)`.
- [ ] **Step 2: `Font::draw_framed_text(bmp, pal, s, x, y, color)`** = `draw_rounded_box(x, y, 0, 7, get_dims(s))` + `draw_string(s, x+2, y+1, color, 1)` (`font.cpp:82-85`). **Test:** the box pixels and the text origin.
- [ ] **Step 3: `blit::blit_bitmap(scr, src, x, y, w, h)`** = the ARGB rectangle copy at identical coordinates through `clip_image` (`blit.cpp:217-233`). **Tests:** an interior copy; clipping on each edge, including a negative `x` (the `InputString` case); no write outside `scr.clip`.
- [ ] **Step 4: `small_text::draw_text_small(bmp, pal, bank, s: &[u8], x, y)`** (`common.cpp:227-237`). Per byte, `c = b.wrapping_sub(b'A')`; blit `bank[c]` when `c < 26`; always `x += 4`. **Test:** `"A Z"` blits frames 0 and 25 at `x` and `x + 8`; the space and a lowercase letter only advance.
- [ ] **Step 5: `SmallLabels<'a> { text: &'a SpriteSet, names_on_bonuses: bool }` and `Scene::small_labels: Option<SmallLabels<'a>>`.** `SceneData` gains `text_sprites: SpriteSet`, loaded by `scene_data` as `load_sprites(tc_root, "text.tga", 4, 4, 26)` after the existing loads. `as_scene` returns `small_labels: None`. Add `small_labels: None` to every other `Scene { … }` literal: `frame.rs` tests `:277`, `:385`, `:400`, and the four oracle-tests harnesses.
- [ ] **Step 6: `sprite_pass_with(…, labels: Option<&SmallLabels>)`.** `sprite_pass` keeps its exact signature and calls it with `None`. `frame::draw` calls `sprite_pass_with(…, scene.small_labels.as_ref())`. The three labels go in at fact 18's exact points and coordinates:
  - the bonus label inside the flicker gate;
  - the booby-trap label per wobject after its sprite/pixel. Iterate the wobject pool as `(0..capacity()).filter_map(|s| get(s).map(|o| (s, o)))`, which is the same order as `iter()`, so the sprite draw is unchanged. `h_rem_exp` is `state.wobject_consts.h_rem_exp`;
  - the Change label after the crosshair blit, in the viewport's-own-worm block, when `control_states.get(ControlState::CHANGE)`.

  Fix the module doc (`object_draw.rs:247-250`: "omitted … belong to 3e").
- [ ] **Step 7: label tests** (unit, in `object_draw.rs`):
  - each label draws only under its full gate;
  - `None` gives a surface identical to today's;
  - the booby name is `weapons[slot % n]`, pinned with a wobject in slot 3 and weapon 34;
  - the Change label uses the current bit only: prev set with current clear draws nothing;
  - a worm that is not the viewport's own draws no Change label.
- [ ] **Step 8: re-diff.** `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game`, then `cargo test --manifest-path rust/Cargo.toml -p game`. Both pass, and `git status --porcelain rust/oracle-tests/golden` is empty. Every render golden is unchanged because the flag is `None` on every golden path.
- [ ] **Step 9: commit** `render(4.5e-1): get_dims_h, draw_framed_text, blit_bitmap, DrawTextSmall and the three small labels behind Scene::small_labels (hash-neutral)`.

**Done when:** the new unit tests pass, both re-diff commands are green, and no golden changed.

### Task 2 (Batch 2): `scenario` and `sim`, the live-settings choke point, stores, `generate`, weapsel validation (hash-neutral)

**Files:** `rust/scenario/src/{build.rs, assets.rs, storage.rs, parser.rs}`, `rust/sim/src/weapsel.rs`.

- [ ] **Step 1: `build::apply_live_settings(state: &mut SimState, s: &Settings)`.** It writes exactly `settings_max_bonuses`, `weap_table` (as `i32`, as today), `game_mode`, `time_to_lose`, `blood`, `settings_loading_time`, `load_change` and `shadow`, and nothing else.
  - Its doc comment is **the fact-16 table**: every C++ `settings->` read site with its Rust field, or "draw (`Match`)", "kStateGame only", "Holdazone (unported)" or "per-worm (4½f)". The table is re-derived by running the fact-16 `grep` now.
  - `new_match_with` replaces its "Settings (design §4.2)" block with one call placed at the same point. `settings_health` stays where it is (per-worm).
  - **Tests:** every field set from a non-default `Settings`; no other field touched (the worms, `rand`, `level`, `cycles` and the pools compare equal); `build_match` and `new_match` produce the same `SimState` as before. The existing builder tests plus the full re-diff prove it: every sim and settings golden is unchanged.
- [ ] **Step 2: `assets::try_read_asset(tc_root, rel) -> Option<Vec<u8>>`.**
  - Native: `std::fs::read(...).ok()`.
  - wasm: the same key match, returning `None` on a miss.
  - `read_asset` becomes `try_read_asset(..).unwrap_or_else(|| panic!("read {rel}: …"))`, and native keeps the `io::Error` text.
  - **Test** (native): a hit, and a miss giving `None`.
- [ ] **Step 3: `ConfigStore: Send + Sync` and `fn root_label(&self) -> &str`.**
  - `NativeStore`: by default the user root as T0 recorded it (no trailing separator unless T0 showed one). `with_root_label(self, label) -> Self` overrides it.
  - `MemoryStore`: `user: Mutex<BTreeMap<..>>`; the label defaults to `"/openliero"`; `with_root_label`.
  - All existing storage tests pass unchanged.
  - **New tests:** the labels; `MemoryStore` is `Send + Sync` (a compile-time assert); `Box<dyn ConfigStore>` is `Send + Sync`.
- [ ] **Step 4: the Rust `generate` directive** (§Formats; LD 4 amended).
  - The parser errors are: `generate` without `settings`; `generate` together with `level`; neither `level` nor `generate` (the existing "missing `level`"); a bad argument.
  - `Scenario::generate() -> Option<u32>`; `to_text` round-trips.
  - `scenario::load` refuses it. It already refuses every `settings` scenario (`loader.rs:136-140`); keep a test that says so.
  - Tests go in `parser.rs`'s test module.
- [ ] **Step 5: `sim::weapsel`.**
  - `WeaponSelection::validate(n_weapons: usize, n_worms: usize, cfg: &WeapselConfig) -> Result<(), WeapselError>` holds the four refusals, in fact 30's order, factored out of `new`, which now calls it first.
  - `set_weap_table(&mut self, t: [u32; WEAPON_COUNT])` replaces `weap_table` only; `enabled_weaps` is untouched (fact 15).
  - **Tests:** `validate` matches `new`'s errors on each refusal; after `set_weap_table` bans the next weapon, a Left/Right cycle skips it; `enabled_weaps()` is unchanged; RANDOMIZE honours the new table. The 4½c `weapsel_golden`, `weapsel_handoff` and the continuations stay green in the re-diff.
- [ ] **Step 6: re-diff** (both commands) and the golden status (empty). **Commit** `scenario(4.5e-1): apply_live_settings (the new_match_with choke point), try_read_asset, a Sync ConfigStore with root_label, the oracle-only generate directive; sim: WeaponSelection::validate/set_weap_table (hash-neutral)`.

**Done when:** everything above is unit-tested, both re-diff commands are green, no golden changed, and `git diff 2377b0c -- rust/sim` shows only `weapsel.rs`.

**Batch 2 gate:** the Task 1 and Task 2 done-whens hold together. Run the re-diff once more on the combined tree, then do the disk cleanup.

---

### Task 5 (Batch 3): `oracle_dump_shell` grows `O/I/B`, `text`, `detail`, `fs`, intervention 9 and the gap check

**Files:** `src/tools/oracle_dump/shell_dump.cpp`, `rust/oracle-tests/gen_shell_golden.sh`.

- [ ] **Step 1: the parser.** The case gains an ordered `events` list of `key` or `text`, stable-sorted by frame so the file order within a frame is kept. It also gains `bool detail` and `std::string fs`.
  - `text` hex is decoded into a `std::deque<std::string>` that lives for the whole run.
  - `ScancodeOf` gains `BACKSPACE`. `fs` together with `setup != default` is a `Fail`.
  - A `text` line becomes `SDL_EVENT_TEXT_INPUT` with `ev.text.text = s.c_str()`, pushed in the list order.
- [ ] **Step 2: `TopOf` grows** `WeaponMenuState` → `O`, `InputStringState` → `I`, `InfoBoxState` → `B`. Include `weaponMenuState.hpp` and `inputState.hpp`; the classes are public. Any other state still `Fail`s. `upd` comes from the same `TopOf` at frame start, with the `W` refinement.
- [ ] **Step 3: `detail`.** After each `f` line, write a `d` line exactly as §Formats says. `cur` is `S` iff `gfx.cur_menu == &gfx.settings_menu`, `M` iff it is `&gfx.main_menu`, and anything else is a `Fail`.
- [ ] **Step 4: `fs`** (§Formats). It replaces the `setup`/intervention-8 block for that case only. The level-file refusal generalises: when `!random_level`, `FsNode(level_file)` must open from the current CWD, or the case `Fail`s.
- [ ] **Step 5: intervention 9 and the `file` lines**, after the `end` line, as §Formats says.
- [ ] **Step 6: the gap check** (D4). `Fail("search keys N ms apart on frames a, b")`.
- [ ] **Step 7: the header comment.**
  - Document intervention 9, the `fs` fixture (environment variables and CWD only, no code intervention), the gap check, and the opt-in lines. The 4½d `out` header string must be written **byte-identically** when neither `detail` nor `fs` is set; the new `# d` / `# file` header lines are emitted only when their feature is on.
  - Update the "Interventions" list and the usage line.
  - Also note that `InputStringState::Enter`'s `SDL_StartTextInput(nullptr)` fails harmlessly headless (no window). Check that it does not crash.
- [ ] **Step 8: `gen_shell_golden.sh`.**
  - Accept `upd ∈ [MWGOIB]` and `top ∈ [MGOIB-]`.
  - A script with `detail` must have exactly one 6-field `d` line right after each `f` line, with the same frame.
  - A script with `fs` must end with at least one 3-field `file` line after `end`, in sorted order.
  - With neither, the 4½d rules apply unchanged.
  - The count becomes `test "$n" -eq 21`.
  - Before Batch 7 commits the e-1 scripts, the count is 11. So add a temporary `EXPECTED_SHELL_CASES` env override that defaults to 21, and use `EXPECTED_SHELL_CASES=11` in this task's gate.
- [ ] **Step 9: the regeneration proof.** `source $S/env.sh && EXPECTED_SHELL_CASES=11 bash rust/oracle-tests/gen_shell_golden.sh`, then `git status --porcelain rust/oracle-tests/golden` must be **empty**: all 11 4½d goldens are byte-identical.
- [ ] **Step 10: scratch smokes** (in `$S/t5/`, never committed; they need no Rust).
  1. `detail` + `text`: `F7`, `DOWN` (LIVES), `RETURN` (an `I` top), `key 3 down` + `text 33` + `key 3 up`, `BACKSPACE`, `text 34`, `text 32`, `RETURN`. Check that `cfg16` changes on the close frame, and that the close frame's sounds hold two `MenuSelect`s across the Enter frame and the close frame.
  2. Weapon options: a setup with `weapTable` all 2 except one 0 and all picks on that weapon. `F7` → `DOWN` ×6 (WEAPON OPTIONS) → `RETURN` (`O`) → `RIGHT` ×2 on the enabled weapon (it becomes Banned) → `ESC` (`B`) → any key → `LEFT` → `ESC` (pops to `M`).
  3. `fs` with no `liero.cfg` anywhere, `expect quit` via `ESC` + `RETURN`: exactly one `file Setups/liero.cfg …` line.
  4. `fs` with a user `liero.cfg` holding `gameMode = 1`: the boot line differs from smoke 3 (TIME TO LOSE is visible).

  Run each with `--ppm-dir` and eyeball a few frames with `$S/ppm2png.py`.
- [ ] **Step 11: format and tidy.**
  - `"$CF22" --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/shell_dump.cpp` prints nothing.
  - `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh 2377b0c` is clean.
  - `scripts/clang-tidy-diff.sh build/linux-x64 2377b0c` exits 0.
- [ ] **Step 12: commit** `oracle(4.5e-1): oracle_dump_shell — O/I/B tops, text, detail (d lines), the fs fixture through the real paths::Resolve, intervention 9 (the exit save + file lines), the search-gap check`.

**Done when:** it builds, format and tidy are clean, the 4½d goldens are byte-identical, and the four smokes behave as described. Record their outputs in the done-report.

### Task 6 (Batch 3): `oracle_dump_sim_physics` gains `generate <level_seed>`

**Files:** `src/tools/oracle_dump/sim_physics_dump.cpp`.

- [ ] **Step 1: parse `generate`** to match the Rust parser exactly (§Formats and T2 Step 4): one `u32`; a `#` token is a comment; it requires `settings`; it excludes `level`; exactly one of them. Refuse with `std::fprintf(stderr, …); std::exit(1)`, as the file does elsewhere.
- [ ] **Step 2: the level.** With `generate`, `Rand level_rand; level_rand.Seed(level_seed); game.level.GenerateFromSettings(*common, *settings, level_rand);` replaces the `level.load` block. Refuse `!settings->random_level`. `game.rand` keeps `game.rand.Seed(seed)`, untouched. Everything after the level is the existing `settings` path.
- [ ] **Step 3: the header.** Update the file-header comment (`:33-36`, the scenario-grammar list) to say that `generate` is the one sanctioned `GenerateFromSettings` call, with its own `Rand`.
- [ ] **Step 4: the regeneration proof.** In one shell:
  - `source $S/env.sh && for s in $(grep -l oracle_dump_sim_physics rust/oracle-tests/*.sh); do bash "$s" || { echo "FAIL $s"; break; }; done`. That is 39 scripts.
  - Then `git status --porcelain rust/oracle-tests/golden` must be **empty**.
  - A script failing for an environmental reason that has nothing to do with this change (a missing tool) is reported by name. The settings-path ones (`gen_sim_slice4_5a`, `4_5c0`, `4_5c`) and `gen_render_slice4d_live.sh` are mandatory.
- [ ] **Step 5: a smoke.** A scratch scenario in `$S/t6/`: `seed 5`, `generate 77`, `ticks 50`, `settings` with `randomMapWidth = 96`, `randomMapHeight = 64`. It runs and writes 51 rows of 12 columns.
- [ ] **Step 6: format and tidy**, as in T5 Step 11, for this file. **Commit** `oracle(4.5e-1): oracle_dump_sim_physics — the oracle-only generate <level_seed> directive (the real GenerateFromSettings, its own Rand)`.

**Done when:** every existing sim_physics golden regenerates byte-identically, the smoke runs, and format and tidy are clean.

**Batch 3 gate:** T5 and T6 are both done. `git diff --name-only 2377b0c -- src CMakeLists.txt` lists exactly the two dumper files.

---

### Task 3 (Batch 4): `ui` foundations, events, overlays, the sub-screen stack, `cur_menu`, `level_path`, the store in `Shell`

**Files:**
- `rust/ui/src/{text.rs, keys.rs}`;
- `rust/ui/src/shell/{mod.rs, stack.rs, main_menu.rs, new_game.rs, level_slot.rs, playing.rs}`;
- new `rust/ui/src/shell/overlay.rs` and `level_path.rs`;
- mechanical edits to `rust/game/src/{main.rs, input.rs, touch.rs}`, `rust/shot/src/lib.rs` and `rust/oracle-tests/tests/shell_common/mod.rs`.

- [ ] **Step 1: `InputEvent::{Key(KeyEvent), Text(String)}`**, and `ShellInput.events: &'a [InputEvent]`.
  - `Shell::frame` walks the events in order. A `Key` goes to `KeyLatch` (`ProcessEvent`), plus the existing Esc → `Match::esc` rule while `Playing`, and then to the top screen's `handle_key`. A `Text` goes only to the top's `handle_text`, since `ProcessEvent` ignores text.
  - Update every caller mechanically, with no behaviour change: the `game` glue wraps `KeyEvent`s (the queue holds `InputEvent`, and `take_tick` defers only a key's second key event, keeping FIFO order); `touch_key_events` output is wrapped at the call site; `shot`; and the `shell_common` driver (keys only for now).
- [ ] **Step 2: `ui::text`.**
  - `utf8_to_dos(&str) -> u8`, fact 7 exactly. **Tests:** `"a"`, `"7"`, the six Swedish letters, `"ab"`, `"é"`, `"€"`, `""` → 0.
  - `WEAP_STATES = ["Menu", "Bonus", "Banned"]` (`common.cpp:222-224`).
  - `UiTc` gains `weapon`, `availability`, `no_weaps` (the TC texts), `exepal: Palette` (the `small.tga` palette), and `weapon_names: Vec<String>` in `weap_order` order, with `weap_order: Vec<usize>` loaded through `assets::object::Objects` + `sim::weapsel::weap_order`. **Test:** 40 names, `weap_order` matches the sim's, and `no_weaps` holds one NUL.
  - `ui::keys::DK_BACKSPACE = 14`.
- [ ] **Step 3: `overlay::InputStringState`** (§Formats, facts 6–9):
  - fields `buffer: Vec<u8>, max_len, x, y, filter: Option<fn(u8) -> u8>, prefix, centered, purpose: InputPurpose, done, accepted`;
  - `handle_key(ev)`: on a key-down (repeat included), Backspace pops, Return/KP Enter accepts, and Esc cancels;
  - `handle_text(s)`: `k = utf8_to_dos(s)`, then C++'s short-circuit (fact 7);
  - `is_done() -> Option<(bool, Vec<u8>)>`;
  - `draw(surface, frozen, pal, font)`, with fact 9's strip width and fact 8's display mapping; the rounded box is `(x - 2 - adj, y, 0, 7, w)` and the string colour 50 at `(x - adj, y + 1)`.
  - `InputPurpose::IntegerEntry(ValueEntry)` for now; e-2 adds `SaveSetupAs`.
- [ ] **Step 4: `overlay::InfoBoxState`** (fact 12): `{text, x, y, clear_screen, purpose: InfoPurpose, done}`.
  - `handle_key` is done on any key-down.
  - `draw(surface, pal: &mut Pal32, exepal, font)`: when clearing, `*pal = pack_pal32(exepal)` and fill 0. The next menu frame's `update_menu_palettes` restores the rotation, as C++'s does.
  - `InfoPurpose::{NoWeapons, Refused(Refusal)}` (T4 defines `Refusal`; use a placeholder type for now if needed).
- [ ] **Step 5: `Screen` and the stack.**
  - `Screen` gains `WeaponOptions(WeaponMenuState)` (a stub `struct` until T4 is fine, but no dead variants survive T4), `InputString(InputStringState)` and `InfoBox(InfoBoxState)`.
  - `is_overlay` is true for `InputString` only; `wants_menu_flip` is true for everything but `Playing`.
  - In `Shell::frame`:
    - (a) `(sel, fading)` come from the stack's `MainMenu`, wherever it is (fact 3);
    - (b) a screen's update may leave **one push request** in its context; after `finish_update`, the shell runs the pushed screen's `enter` and pushes it (fact 4);
    - (c) `AfterUpdate::Replaced` runs the new top's `enter` (fact 5);
    - (d) `Popped { empty: false }` draws the new top;
    - (e) an overlay's continuation runs **inside** its update step, before `finish_update`: the `MenuSelect`, `keys.clear()`, the continuation, then keep = false. That order is `inputState.cpp:75-84`, and it lets a continuation schedule a replacement;
    - (f) `draw_stack` dispatches the new variants.
  - `Phase::Text` (`"text"`) when the top is `InputString`; `WeaponOptions` and `InfoBox` are `Phase::Menu`. `top_char` returns `O`/`I`/`B`.
- [ ] **Step 6: `MenuWorld.cur_menu: CurMenu::{Main, Settings}`** (fact 1). `MainMenuState::enter` sets `Main`. `MainMenuState::draw` passes `disabled = cur_menu != Main` for the main menu, and draws the settings menu disabled when `cur_menu == Main`, else enabled. `Shell::cur_menu()` exposes it. Behaviour is unchanged while nothing sets `Settings`.
- [ ] **Step 7: `level_path::read_level(store: &dyn ConfigStore, tc_root, level_file) -> Option<LevelData>`**. `.LEV` is appended when the name has no `.`, via `sim::levelgen::level_file_name`.
  1. It starts with `root_label() + "/"` → `store.read(rest)` through the merged view (Q4 = A).
  2. It is an absolute path (not wasm) → `std::fs::read`.
  3. It is relative → `try_read_asset(tc_root, …)`, the legacy TC-relative form.
  4. Otherwise `None`.

  Bytes that do not parse also give `None`. `new_game::generate_level(tc_root, settings, file: Option<LevelData>, seed)` never reads files. `LevelSlot::generate(tc_root, settings, store, seed)` resolves the file only when `!random_level`. **Tests:** each rule (a `MemoryStore` with a system level under `TC/openliero/Levels/`; a scratch absolute file; the 4½d `Levels/water_stage.lev`), a missing file, garbage bytes (falls back to random, as 4½b does), and `.LEV` appending.
- [ ] **Step 8: the store in `Shell`.** `Shell { store: Box<dyn ConfigStore> }` (`Send + Sync` via the supertrait). `Shell::boot` and `Shell::boot_playing` take `store` after `settings`. The callers pass `Box::new(MemoryStore::new())` for now, so the `game` behaviour is unchanged until T10. `boot_state` is unchanged here; T4 sanitises it.
- [ ] **Step 9: unit tests** (in `overlay.rs`, `stack.rs`, `mod.rs`):
  - input editing: the filter, `max_len`, Backspace on empty, a repeat Backspace, accept, cancel, text after Return still appended on that frame, `ClearKeys` on close;
  - the strip width, pinned by the first pixel outside the box (fact 9);
  - the info box: dismissed by a repeat, not by a key-up; the NUL two-line box geometry; colour 6; `clear_screen` swaps the palette for that frame only;
  - the stack: an overlay draws over `MainMenu`, a non-overlay `InfoBox` draws alone on the stale surface, `Replaced` runs `enter`, and `(sel, fading)` come from the buried `MainMenu`.

  All 4½d unit tests stay green unchanged.
- [ ] **Step 10: gate.**
  - The re-diff (both commands).
  - The wasm check (`game` changed).
  - `cargo run --manifest-path rust/Cargo.toml -p oracle-tests --example gen_slice4_5d -- check` shows no violation. The 4½d generator is unchanged; its cases still drive the extended shell.
  - `cargo test --manifest-path rust/Cargo.toml -p oracle-tests --test shell_golden` is green: 4½d G2 is still bit-exact.
  - The golden status is empty.
- [ ] **Step 11: commit** `ui(4.5e-1): input events + Utf8ToDos, InputStringState / InfoBoxState, the sub-screen stack (overlay draw, push after update, replace, the buried main menu), cur_menu in MenuWorld, level_path, the store in Shell`.

**Done when:** all of the above passes and no behaviour changed on any existing path (the 4½d G2 is the proof).

---

### Task 4 (Batch 5): `ui` behaviour, settings focus, Enter dispatch, number entry, WEAPON OPTIONS, refusals, the RESUME resync, the exit save

**Files:**
- `rust/ui/src/shell/{main_menu.rs, settings_menu.rs, playing.rs, mod.rs, overlay.rs}`;
- new `rust/ui/src/shell/weapon_options.rs`;
- `rust/ui/src/text.rs`;
- callers only if a signature moves.

- [ ] **Step 1: `MainMenuState::update` in C++ source order** (`mainMenuState.cpp:152-612`). The 4½d `||` short-circuits are kept.
  1. Esc / any Jump: in main focus, the cursor goes to QUIT; in settings focus, `cur_menu = Main`, with no sound.
  2. Up / Down (+ P1/P2) act on `cur_menu`: `MoveDown`/`MoveUp`, `movement(∓1)`.
  3. Enter / KP Enter / any Fire:
     - **main focus:** `MenuSelect`, then:
       - MATCH SETUP → `cur_menu = Settings`;
       - `default:` (RESUME / NEW GAME / QUIT) → `cur_menu = Main; selected = id`;
       - the 4½d placeholders are unchanged;
     - **settings focus:**
       - WEAPON OPTIONS: `MenuSelect` + push `WeaponOptions`;
       - LEVEL / LOAD SETUP / SAVE SETUP AS…: `MenuSelect` only (D6);
       - `default:` → `settings_menu.on_enter(SettingsModel, cx)`, result ignored. `Enter::EditValue(e)` pushes `InputString { initial: e.initial as bytes, max_len: e.digits, x: e.x, y: e.y, filter: digits, prefix: "", centered: false, purpose: IntegerEntry(e) }`.
  4. F1: `cur_menu = Main`, `move_to_id(start)`, `selected = start`.
  5. F2, F3, F5, F6: consumed and inert, as 4½d.
  6. **F7:** `main_menu.move_to_id(MA_SETTINGS)`, `cur_menu = Settings`.
  7. F9, F8: consumed.
  8. Held Left / Right: `cur_menu.on_left_right` with its model (`PlainModel` or `SettingsModel`); `reset_left_right` on false.
  9. PgUp / PgDn on `cur_menu`.
  10. `selected >= 0` → **the refusal check** (Step 5) → the fade-out.
- [ ] **Step 2: `MainMenuState::draw`** per fact 1 and `mainMenuState.cpp:614-626`.
- [ ] **Step 3: the `IntegerEntry` continuation** (fact 13). On accept with a non-empty buffer, parse the digits (at most 4, so there is no overflow), clamp to `[min, max]`, and write `val × div` into the field named by `item_id`. `SettingsModel` gains `int_field(item_id) -> Option<&mut i32>`. Then **always** rewrite only that item's `value` (`% ` when a percentage) and set `has_value`, with no `update_items`. The `MenuSelect` and `ClearKeys` come before it, in the overlay step (T3 Step 5e).
- [ ] **Step 4: `WeaponMenuState`** (facts 10, 11; design §4.4).
  - `enter`: `Menu::new(179, 28, false)`, `set_height(14)`, `value_offset_x = 89`. Forty `MenuItem::new(48, 7, weapon_names[i], i)`; `move_to_first_visible`; `update_items(WeaponModel)`.
  - `WeaponModel { weap_table: &mut [u32; 40], weap_order: &[usize] }`, whose `behavior(id)` is `ArrayEnum { v: &mut weap_table[weap_order[id]], arr: &WEAP_STATES, broken: false }`.
  - `update`, in order:
    - Up and Down (once, + controls) with the crossed sounds;
    - Left and Right **once** → `on_left_right(±1)`, result ignored;
    - PgUp and PgDn;
    - `on_keys(keys.typed(), now_ms, false)`;
    - Esc / Jump → the close rule: keep = false this frame, or push `InfoBox(NoWeapons, 223, 68, false)` with text `tc.no_weaps`.
  - `draw`: `surface ← frozen`; `DrawBasicMenu` (main menu disabled, selection shown); `draw_framed_text` ×2 (fact 10); the menu enabled.
- [ ] **Step 5: refusals** (Q2, D5).
  - `pub enum Refusal { Build(BuildError), Weapsel(WeapselError) }`.
  - `fn refusal(settings, route, opts, n_weapons) -> Option<Refusal>`.
    - NEW GAME: `build::validate_for_selection` (or `validate` when `opts.skip_selection`), then `WeaponSelection::validate(n_weapons, 2, &new_game_config(..))`.
    - RESUME of an `attached` match: `game_mode == GM_HOLDAZONE` → `HoldazoneUnsupported`, then `validate_for_selection`.
  - `MainMenuState` reaches it through a `MenuCtx` field; the shell supplies `running`, `attached`, `skip_selection` and `n_weapons`. On a refusal: `selected = -1`, push `InfoBox(Refused(r), 160, 100, false)`, no fade, no extra sound.
  - `text::refusal_text(&Refusal, &UiTc) -> String`.
- [ ] **Step 6: RESUME, the live settings** (finding 1, facts 14–16, D7; adjusted by T0's addendum if it said so).
  - In `route(MA_RESUME_GAME)`, before `focus`: `if m.attached { apply_live_settings(sim, &world.settings); m.resync(&world.settings); }`.
  - `Match::resync(s)` replaces `self.cfg.settings` with a clone (the `kStateGame` lives, `blood_particle_max`, the selection's `level_file` draw, `shadow`), sets `hud.map = s.map`, sets the small-labels `names_on_bonuses`, and, if a selection is active, calls `set_weap_table(s.weap_table)`.
  - `Match::attached` is set true in `start`, and there is a `#[doc(hidden)] fn detach_for_test`.
  - `Match::draw` now passes `small_labels: Some(SmallLabels { text: &scene.text_sprites, names_on_bonuses })`, and so does `draw_boot` (C++ runs the same `Game::Draw`).
- [ ] **Step 7: `boot_state` never panics** (fact 21). It builds a sanitised copy: Holdazone → Kill'em All (4½d); unequal health → both take player 1's; any other `validate_for_selection` error → that field from `Settings::default()`. `state.game_mode` keeps the real mode (4½d). **Tests:** each C++-saved oddity (Holdazone, unequal health, `bloodParticleMax = 0`, a weapon pick of 41) boots, and the NEW GAME that follows shows the refusal box.
- [ ] **Step 8: `Shell` API.**
  - `save_on_exit(&self) -> io::Result<()>` is `storage::save_setup(&*self.store, &self.world.settings)`.
  - `cur_menu()`, `settings_menu()`.
  - `#[doc(hidden)] debug_mut() -> &mut ShellDebug { resume_sync: bool, small_labels: bool }`, both true by default. It is used only by T8's witnesses; a `false` skips the step it names.
- [ ] **Step 9: headless unit flows** (in `ui/src/shell/mod.rs` tests, driven through `Shell::frame` as 4½d's are):
  - F7 and Enter-on-MATCH-SETUP focus: Esc returns; the settings cursor survives Esc/F7 and resets after a match (`MoveToFirstVisible` in `enter`); F1 from settings focus starts.
  - Every §3.1 Enter arm, with its sound count.
  - The held Left/Right cadence, plus the Bool/Enum release.
  - Entry:
    - LIVES `7`, Backspace, `42`, Return → 42, with two `MenuSelect`s;
    - MAP WIDTH `9999` → 4096, `0` → 64, `333`;
    - Esc keeps the value, an empty Return keeps the value;
    - `R` (P1 Up) typed during entry never moves the cursor after the close.
  - Weapon options: the 40 rows in `weap_order` order; once-Left/Right; PgDn; the `LA` search; all-banned → the box over the stale frame → any key → the box pops → re-enable → Esc closes.
  - Refusals: Holdazone by NEW GAME, F1 and RESUME; zero weapons from a loaded setup; asymmetric health.
  - RESUME: sim fields before and after; the pause-during-selection `set_weap_table` path; detached leaves them unchanged.
  - `save_on_exit` bytes equal `settings_to_toml`.
- [ ] **Step 10: gate.** The re-diff, the wasm check, `-p oracle-tests --test menu_widget_golden` (G1) and `--test shell_golden` (4½d G2) green. The 4½d corpus never holds Change and never turns on NAMES ON BONUSES, so the labels change no 4½d frame. The golden status is empty, and `gen_slice4_5d -- check` shows no violations.
- [ ] **Step 11: commit** `ui(4.5e-1): the settings focus, the Enter dispatch, number entry, WEAPON OPTIONS, refusal boxes, RESUME live settings, liero.cfg save at exit`.

**Done when:** every flow above is unit-tested and green, every prior gate is unchanged, and `cargo tree -p ui -e normal | grep -c bevy` prints `0`.

---

### Task 7 (Batch 6): G3, sim goldens on four generated levels that are not 504×350

**Files:**
- `rust/oracle-tests/examples/gen_slice4_5e1_sim.rs`;
- `rust/oracle-tests/gen_sim_slice4_5e_golden.sh`;
- `rust/oracle-tests/tests/sim_slice4_5e_generated_golden.rs`;
- the 12 `golden/sim_slice4_5e_*` files;
- `rust/sim/**` only for a proven fix.

- [ ] **Step 1: the generator.** It takes the shape of the frozen `gen_slice4_5a.rs` but is a new file, with subcommands `cfg <case> <out>`, `scan <case> <input_seed> <lo> <hi>` and `gen <case> <game_seed> <input_seed> <out>`.
  - Inputs are per tick `Rand(input_seed).next_u32() & 0x7f`, worm 0 then worm 1, as in 4½a.
  - The level comes from `sim::levelgen::generate_from_settings(&assets, &params, None, &mut rand)` with `rand = Rand::new()` then `rand.seed(level_seed)`. It does **not** go through `ui` (that API is stable), and `build_match` follows.
  - The ledger header in each scenario is modelled on 4½a's. Each case must have its witnesses, and `scan` skips seeds that trip a `debug_assert!`.

  | Case | Size | Settings | Witnesses (required) |
  |---|---|---|---|
  | `small` | 96×64 (**as landed 96×344**: C++ spawning reads past a level below ~342 rows, Addendum G3) | Kill'em All, lives 3 | ≥1 death and ≥1 respawn; an object freed while its last position lay outside the level rectangle (slot diff between ticks) |
  | `odd` | 333×211 | Scales, LOADING TIMES 37, blood 300, `shadow = true` | a reload (`loading_left > 0` seen); ≥50 live bobjects at some tick; a Scales health transfer |
  | `tall` | 160×1000 | Game of Tag, TIME TO LOSE 60 | `is_game_over` flips 0 → 1 exactly once and stays 1 for ≥200 rows (**as landed: 4,960 ticks**, over at tick 4,760) |
  | `banned` | 1024×256 | MAX BONUSES 20, 36/40 weapons `2` (the players' picks untouched) | ≥3 weapon bonuses spawned (statistical witness of the `game.cpp:256-258` re-draw loop: with 90% banned, P(no re-draw in 3 spawns) ≈ 10⁻³; state that in the ledger) |

  Each case runs about 1,500 ticks (plus the ≥200-row margin for `tall`). `level_seed` is fixed per case and recorded; `seed` is the game seed.
- [ ] **Step 2: write** `sim_slice4_5e_<case>_setup.cfg` (C++-schema TOML via `cfg`) and `_scenario.txt` (`seed`, `generate`, `ticks`, `settings`, `input` lines, the ledger header).
- [ ] **Step 3: `gen_sim_slice4_5e_golden.sh`.** Same shape as `gen_sim_slice4_5a_golden.sh`. It builds `oracle_dump_sim_physics` and loops over the four cases. Its awk gates: 12 columns; row `k` carries tick `k`; `ticks + 1` rows; `tall` flips exactly once with a ≥200-row tail; the other three follow their ledger's expectation, "never over" or "flips once". Run it with `source $S/env.sh && bash rust/oracle-tests/gen_sim_slice4_5e_golden.sh`.
- [ ] **Step 4: the gate test.** `sim_slice4_5e_generated_golden.rs` has the 4½a harness shape: parse the committed scenario, `settings_from_toml` on its sidecar, generate the level from `generate`, `build_match`, then per tick apply the inputs and compare all 11 hash columns and `is_game_over` against column 12. Add a witness-guard test that re-runs each ledger claim.
- [ ] **Step 5: the fix loop.** On a mismatch, find the first differing tick and column. Compare the Rust `levelgen` level against C++ (`oracle_dump_levelgen` exists but is unbuilt; `cmake --build … --target oracle_dump_levelgen` if needed) before touching the sim. A sim fix goes in `rust/sim` with a unit test that pins the C++ line, followed by the full re-diff. **Never** edit a golden. A dumper bug may only be fixed with T6's regeneration proof re-run.
- [ ] **Step 6: gate.** `cargo test --manifest-path rust/Cargo.toml -p oracle-tests --test sim_slice4_5e_generated_golden` passes (**G3 bit-exact**), and so does the full re-diff. The golden audit shows exactly the 12 G3 `A` lines, plus any shell ones already landed.
- [ ] **Step 7: commits.**
  - A sim fix first, if any: `sim(4.5e-1): <what> (found by G3 <case>)`.
  - Then `oracle(4.5e-1): G3 — four generated-level sim goldens at 96x64, 333x211, 160x1000, 1024x256, bit-exact vs the real GenerateFromSettings` (landed as `6c2dfd8`, at 96x344).

**Done when:** all four cases are bit-exact on every row and column, every witness holds, and any sim fix is separate and re-diffed.

---

### Task 8 (Batch 7): G2e-1, the shell corpus and the C++ goldens

**Files:** `rust/oracle-tests/tests/shell_common/mod.rs`, new `tests/shell_e1_cases/mod.rs` and `examples/gen_slice4_5e1_shell.rs`, the 27 `golden/shell_*` e-1 files.

- [ ] **Step 1: `shell_common`.**
  - `ShellScript` replaces `keys` with ordered `events` (key | text). The file order within a frame is kept. It gains `detail: bool` and `fs: Option<String>`. `parse` and `to_text` must round-trip the 11 4½d scripts **byte-identically**; `gen_slice4_5d -- check` and `the_committed_scripts_are_the_generators` prove it.
  - `key_of("BACKSPACE") = (14, TypedKey::Sym(8))`.
  - `drive` feeds `InputEvent`s in order. For `fs` it builds §Formats' store; otherwise `MemoryStore::new()` and `settings_for(script)` as today.
  - For every script, `detail` or not, set `record_replays = false` after boot (the intervention-6 mirror). That field is not in any 4½d line, so the 4½d lines are unaffected.
  - `upd` becomes `sh.top_char()` read before the frame, refined to `W` by `sh.phase() == Weapsel`. It is identical for the 4½d tops.
  - `d` lines: `cur_menu`, `settings_menu().selection()`, FNV-1a-64 of `settings_to_toml(sh.settings())`, and `hash_game_state(&sim)` when `top_char() == 'G' && phase() == Game`.
  - `file` lines: `save_on_exit()` when the case quit, then the sorted walk of `tmp/user`.
  - `B` gains `type_digits("333")`: per char, `key down <c>` + `text <hex>` on one frame and `key up` two frames later, so 3 frames per char. It also gains `text(bytes)`.
- [ ] **Step 2: validators** in `drive`, as `Ledger.violations`. Each is refused at generate time:
  - a key the C++ acts on but e-1 Rust leaves inert: F2, F3, F5, F6, F8, F9, F10, F11;
  - Enter on LEVEL, LOAD SETUP or SAVE SETUP AS… (D6);
  - a NEW GAME or RESUME that Rust refuses (C++ would play it): Holdazone, unequal health, zero weapons;
  - a NEW GAME whose selection constructor drew `rand` (`sim.rand.draws() != 0` on the route frame; the precondition of intervention 3);
  - a back-to-menu pop frame with a worm key held (4½d fact 12);
  - any key-down besides the push trigger in a frame whose update pushed a screen (fact 4);
  - a `text` event while the top is not `I`;
  - a letter typed on an `O` frame that is any keyboard player's control. P1 is R, F, D, G; P2 is arrows and RCTRL/RALT/RSHIFT; the `ALLOWED` set is extended with explicit letters and digits.
- [ ] **Step 3: the 10 cases** (`shell_e1_cases::cases()`). All have `detail`; all `match_seed`s are scripted, and each boot seed is distinct. *(As landed: 11 — Batch 7 added `key_edges`: held Change + one Right steps one weapon, a Change+Jump rope throw, and a dead worm's Fire-ready press, the C++ `OnKey` edge semantics of `8291511`.)*
  1. `settings_nav`:
     - Enter on MATCH SETUP;
     - Up/Down with wrap over hidden rows;
     - P1 `R`/`F` in settings focus;
     - Esc (the main cursor unchanged);
     - F7 (the cursor kept);
     - PgUp/PgDn;
     - F1 from settings focus → NEW GAME → selection → Esc → menu → QUIT.
  2. `settings_edit`:
     - held Right/Left on LOADING TIMES (the cadence);
     - GAME MODE Enter ×4 through Holdazone: 16 rows, so the scrollbar shows, and TIME TO WIN/ZONE TIMEOUT appear;
     - held Right on TIME TO LOSE under Game of Tag;
     - NAMES ON BONUSES Left, which releases and needs a re-press;
     - MAP Enter;
     - AMOUNT OF BLOOD Enter (sound only);
     - REGENERATE LEVEL toggle.

     There is no NEW GAME, and it ends with the mode back on Kill'em All.
  3. `int_entry`:
     - LIVES `7`, BACKSPACE, `42`, RETURN;
     - MAP WIDTH `9999` → 4096; `0` → 64; `333`;
     - MAX BONUSES `5` then ESC (cancel);
     - LOADING TIMES BACKSPACE ×3 then RETURN (empty keeps);
     - `R` key + text typed into an entry (filtered, never leaks).
  4. `weapon_options`. Setup sidecar: `weapTable` has one weapon at 0 (a non-steerable, sim-ported weapon) and the rest at 2; both players' five picks are that weapon.
     - Open: 40 rows; DOWN ×20; PAGEDOWN; PAGEUP; search `L`,`A` in one visit; once-LEFT/RIGHT sounds.
     - Ban the one weapon → ESC → the `B` box → any key → re-enable → ESC.
     - NEW GAME → selection (only that weapon) → ESC during selection → F7 → WEAPON OPTIONS → enable a second weapon → ESC → ESC → F1 (RESUME) → P1 RIGHT cycles onto the second weapon (the live `weap_table`, fact 15) → DONE → 60 ticks → QUIT.
  5. `map_size`: typed 333×211 → NEW GAME → 300 ticks → ESC → typed 96×64 → NEW GAME → 300 ticks → QUIT. *(As landed: 333×360, then 184×420 — heights ≥ 342 keep C++ spawning inside the level, Addendum G3.)*
  6. `live_settings`:
     - play 200;
     - ESC;
     - LOADING TIMES `0` (typed), AMOUNT OF BLOOD held Right to 300, MAX BONUSES `20` (typed), MAP Enter, NAMES ON BONUSES Enter, GAME MODE → Game of Tag;
     - ESC, F1 (RESUME), 300 ticks.
  7. `labels`. Setup sidecar: `namesOnBonuses = true`, `maxBonuses = 8`, P1's picks all BOOBY TRAP.
     - NEW GAME → play with P1 firing booby traps, P1 holding LSHIFT (Change) and P2 holding RALT for stretches, releasing all before ESC → QUIT.
  8. `cfg_boot` (`fs`): sys `Setups/liero.cfg` from `data/Setups/liero.cfg`, `dir sys Profiles`, `dir sys Resources`; user `Setups/liero.cfg` from `shell_cfg_boot_user_liero.cfg`, which has Game of Tag, `timeToLose = 120`, 400×300, `namesOnBonuses = true` and a random level.
     - Boot → F7 → change LOADING TIMES (held) → ESC → QUIT, so the exit save writes `file` lines.
  9. `cfg_default` (`fs`): no `liero.cfg` anywhere, plus the two sys dirs.
     - Boot (the defaults saved) → idle 40 → QUIT.
  10. `match_setup` 🎯 (`fs`, the done-when-2 path). The user `liero.cfg` is `shell_match_setup_user_liero.cfg`: one weapon at 0 and the rest banned, the picks on it, a random level.
      - F7 → GAME MODE Enter ×3 (Kill'em All → Scales, via Tag and Holdazone) → LIVES typed `3` → LOADING TIMES held → MAP WIDTH `333` / MAP HEIGHT `211` typed. *(As landed: 333×352, for the same Addendum G3 reason.)*
      - WEAPON OPTIONS: ban the last weapon → ESC → the box → any key → re-enable → ESC → ESC.
      - NEW GAME → selection → 300 ticks → ESC → AMOUNT OF BLOOD (held) and MAX BONUSES (typed) → ESC → F1 (RESUME) → 200 ticks → ESC → QUIT, so the exit save writes `file` lines.
- [ ] **Step 4: ledgers and witnesses** (printed by the generator; `write` refuses a case that lacks its own). Every case needs no violations. Per case:
  - **`int_entry`:** entries accepted/cancelled/empty; the clamp to 4096 and to 64 hit.
  - **`weapon_options`:**
    - an `O` visit;
    - a `B` box;
    - the search moved the cursor;
    - **counterfactual:** with `debug_mut().resume_sync = false`, P1's weapon after the RESUME cycle differs.
  - **`map_size`:** both level sizes seen.
  - **`live_settings`:** **counterfactual:** with `resume_sync = false`, the `state8` sequence diverges within the resumed 300 ticks (record the tick).
  - **`labels`:** `small_labels = false` changes at least one presented frame. Separately, from state: ≥1 frame with a frame-0 bonus while `names_on_bonuses`, ≥1 frame with a booby-trap wobject with `cur_frame == 0`, and ≥1 frame per player with a visible worm and the Change bit.
  - **`cfg_*`:** `file` lines present.
  - **`match_setup`:** the `live_settings` counterfactual plus the `B` box.
- [ ] **Step 5: `gen_slice4_5e1_shell.rs`.** `check` or `write <golden dir>`. It writes the setups, manifests and user files first, then the 10 scripts with a two-line header (case + ledger), then asserts no violations and every witness. **Run:**
  1. `cargo run --manifest-path rust/Cargo.toml -p oracle-tests --example gen_slice4_5e1_shell -- write /home/user/openliero/rust/oracle-tests/golden`;
  2. `… -- check`;
  3. `cargo run … --example gen_slice4_5d -- check`, which must still be clean.
- [ ] **Step 6: the C++ goldens.** `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh`, with the default count of 21. All awk gates pass, and the 11 4½d goldens are still byte-identical (the golden status shows only the new files as `??`).
- [ ] **Step 7: commit** `oracle(4.5e-1): G2e-1 — 10 shell cases (generator-validated, with the finding-1 and label witnesses) and their C++ goldens from the real headless Gfx frame loop`, staging exactly the 27 golden files and the three source files. *(Landed as `ca72317`: 11 cases — the plan's 10 plus `key_edges`, which pins `8291511`'s live key edges — and 30 golden files.)*

**Done when:** the generator is clean with every witness, the C++ gen script passes, the 4½d goldens are unchanged, and there are 27 new golden files.

### Task 9 (Batch 7): 🎯 MILESTONE e-1, every G2e-1 frame, `d` line and saved file bit-exact

**Files:** `rust/oracle-tests/tests/shell_golden.rs`, plus `rust/ui` / `rust/render` for proven fixes only.

- [ ] **Step 1: `shell_golden.rs`.**
  - `golden()` also parses `d` and `file` lines.
  - `compare` checks every `f` field as in 4½d (a `G` frame's sounds skipped), then its `d` line field by field (names: `cur`, `ssel`, `cfg16`, `state8`), then the `file` lines (count, `rel`, `fnv16`).
  - The error names the case, frame, column and both lines.
  - `the_committed_scripts_are_the_generators` covers the union of the 4½d 11 and the e-1 10, each against its own generator.
  - New tests:
    - `the_e1_milestone_is_bit_exact` (`match_setup`, with an assertion on its ledger: 1 NEW GAME, 1 RESUME, quit, file lines);
    - `every_g2e1_case_is_bit_exact`;
    - `the_gate_sees_a_d_line_change` and `the_gate_sees_a_saved_byte_change` (`#[should_panic]`, flipping one `cfg16` or `fnv16` bit, like 4½d's pixel test).
- [ ] **Step 2: the fix loop.**
  - Run `cargo test --manifest-path rust/Cargo.toml -p oracle-tests --test shell_golden`.
  - On the first mismatch, get PPMs of both sides around it. Rust: `SHELL_RUST_PPM_DIR=$S/g2r`. C++: `SHELL_PPM_DIR=$S/g2c bash rust/oracle-tests/gen_shell_golden.sh`, which rewrites the same bytes (the golden status stays empty).
  - Fix the Rust side (`ui`/`render`). Each fix gets a unit test pinning the C++ line it follows, and the full re-diff.
  - Never edit a golden. A dumper bug is fixed in T5's file with its regeneration proof re-run, and only the e-1 goldens are regenerated.
- [ ] **Step 3: gate.** `shell_golden` (all 21 cases) green, the full re-diff green, `menu_widget_golden` green. The golden audit: `git diff --name-status 2377b0c -- rust/oracle-tests/golden | grep -v '^A'` is empty, and the count is 39 once G3 has landed. *(As landed: 22 cases and 42 files, with `key_edges`.)*
- [ ] **Step 4: commits.**
  - Fixes first, each `ui(4.5e-1): <what> (found by G2e-1 <case>, frame N)`.
  - Then `oracle(4.5e-1): 🎯 MILESTONE e-1 — every G2e-1 case bit-exact against the real C++ Gfx frame loop (frames, d lines, saved files)`.

**Done when:** all 10 e-1 and all 11 4½d cases match on every line, and the four negative tests panic as expected.

---

### Task 10 (Batch 8): `game`, the live settings menu

**Files:**
- `rust/game/src/{main.rs, input.rs, touch.rs, lib.rs}`;
- new `rust/game/src/config.rs`;
- `rust/scenario/src/assets.rs` (the wasm setups);
- `web/index.html`, `.github/workflows/preview.yml`.

- [ ] **Step 1: `game::config::store_for(args, env) -> Box<dyn ConfigStore>`** (D10). `--config-root` is parsed in `parse_args`, both forms, and it is accepted with any mode. **Tests:** both flag forms; `OPENLIERO_TEST_USER_DIR` honoured, using a scratch dir under `std::env::temp_dir()`; the fallback. No test resolves the real user folder.
- [ ] **Step 2: boot and exit.**
  - The shell path (`setup`, `main.rs:476-527`): `store = store_for(..)`, `settings = load_setup(&*store)` (errors logged, defaults on failure). Then apply the preview's `?level=` to that in-memory copy (TC-relative, rule 3; e-2 makes it canonical). `new_game::start_settings` is retired or reduced to that override.
  - A `Last` system ordered like `flush_recorder_on_exit` (after `ExitSystems`) calls `ShellRes.shell.save_on_exit()` once on `AppExit`, logging an error. The non-shell paths have no `ShellRes`, so they do no I/O.
- [ ] **Step 3: wasm.**
  - `scenario::assets` embeds `data/Setups/liero.cfg` and `data/Setups/orbmit.cfg`, about 2 KB each (D11).
  - The wasm store is `MemoryStore::with_system([("Setups/liero.cfg", …), ("Setups/orbmit.cfg", …)])`, root label `/openliero`.
  - `web/index.html` shows one quiet line under the help text: "Settings are kept until you reload the page."
- [ ] **Step 4: typed text** (D8). The key reader turns Bevy `KeyboardInput` into `InputEvent::Key` and, on key-down with printable `text`, one `InputEvent::Text` per char, pushed right after the key. `KeyQueue` keeps FIFO order and defers only a key's second key event (fact 19 of 4½d). **Tests:** control characters are dropped; the order is key then text; the deferral never reorders text ahead of a deferred key.
- [ ] **Step 5: touch.**
  - (a) Menu auto-repeat (design §7.4): while the phase is `menu`, a held pad Up or Down emits `repeat: true` key-downs of `controls_ex[0/1]` after 12 ticks, then every 3.
  - (b) While the phase is `text`, FIRE's rising edge emits Return (DOS 28) instead of `controls_ex[4]`, and MENU stays Esc.
  - (c) Drain `window.lieroText` each tick, the same way as `touch_mask`: a string entry becomes one `Text`, and `{k:"Backspace"|"Enter"}` becomes a key down/up pair.
  - **Unit tests:** the repeat cadence (12, 15, 18, …); no repeat outside `menu`; FIRE → Return only in `text`.
- [ ] **Step 6: `web/index.html`** (D9).
  - The hidden `#text-entry` input and the "TAP TO TYPE" button (touch pages only).
  - The diff-against-sentinel handler.
  - `HINTS.text = ['Type, FIRE = OK', 'MENU = cancel']`.
  - Hints for `menu` that mention MATCH SETUP.
  - `.github/workflows/preview.yml`'s comment/help text gains a line on MATCH SETUP (F7) and typing.
- [ ] **Step 7: gate.**
  - `cargo test --manifest-path rust/Cargo.toml -p game`, the wasm check, and the re-diff without `game`.
  - Build the bundle and serve it on 8765.
  - A scratch Playwright script `$S/e1.mjs`, derived from `shell.mjs` and `touch.mjs`.
    - **Desktop:** F7 → `lieroPhase` stays `menu`; Down to LIVES → Enter → `text` → type `12` → Enter → `menu`; WEAPON OPTIONS → Esc; NEW GAME → selection → play → Esc → `menu` → F1 → `game`; no page errors.
    - **Emulated phone** (touch): pad Down ×14 to MATCH SETUP, FIRE; pad to LIVES, FIRE → `text`; fill `#text-entry` with `7` and FIRE → `menu`; MENU dismisses the settings focus; an info box closes on any button.
    - Report `ok`/`FAIL` lines, with screenshots in `$S/e1shots/`.
- [ ] **Step 8: commit** `game(4.5e-1): the live settings menu — the C++ config root + liero.cfg at boot/exit, --config-root, typed text events, touch menu repeat, the phone text field, refusal boxes; the page`.

**Done when:** the `game` tests and the wasm check pass, the Chromium walk is all `ok`, and the non-shell paths are unchanged (`round_trip`, `record_regression` green).

---

### Task 11 (Batch 9): eyeball artefacts, docs, audits, broad review

**Files:** `docs/superpowers/liero-rs-PROGRESS.md`, the overview, the design (status line), the cpp-map, the rust-map, `.claude/skills/liero-shot/SKILL.md`. Re-read each immediately before editing it: line numbers move.

- [ ] **Step 1: the full green board.**
  - The re-diff, both commands.
  - The wasm check.
  - `cargo tree -p ui -e normal | grep -c bevy` → `0`; `cargo tree -p ui --depth 1 -e normal` shows workspace crates only; `cargo tree -p sim-core --depth 1` has no dependencies.
- [ ] **Step 2: tripwires and audits.**
  - `grep -rnE "HashMap|HashSet|\bf32\b|\bf64\b|SystemTime|Instant" rust/ui/src rust/render/src/small_text.rs` → empty.
  - `git diff --name-status 2377b0c -- rust/oracle-tests/golden | grep -v '^A'` → empty, and `| wc -l` → **39** (as landed **42**, with `key_edges`).
  - `git diff --name-only 2377b0c -- src CMakeLists.txt` → exactly the two dumpers.
  - `git diff --name-only 2377b0c -- rust/sim` → `weapsel.rs`, plus only the listed G3/G2 fixes.
  - `grep -c "ResMut<Sim>" rust/game/src/main.rs` equals its count at `2377b0c`.
  - Both dumpers pass clang-format 22 on the whole file, and `scripts/clang-tidy-diff.sh build/linux-x64 2377b0c` exits 0.
  - **Reproducibility**, in one shell:
    - `gen_slice4_5d -- check`, `gen_slice4_5e1_shell -- check`, then `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh && bash rust/oracle-tests/gen_sim_slice4_5e_golden.sh`;
    - then `git status --porcelain rust/oracle-tests/golden` → empty.
- [ ] **Step 3: Xvfb side-by-side** (eyeball only).
  - Copy `data` into `$S/x13/root` and put `shell_match_setup_user_liero.cfg` in as `Setups/liero.cfg`.
  - Run the real `openliero --config-root $S/x13/root` under Xvfb and drive the `match_setup` path with xdotool; digits come from `xdotool type`.
  - Snap about 8 moments: the settings focus, the entry box, weapon options, the info box, selection, play, the pause edit, the resumed game.
  - The Rust side is the same frames from `SHELL_RUST_PPM_DIR` on `shell_match_setup`, converted with `ppm2png.py`. Pair them with `x12_pair.py` into `$S/x13/side/`.
  - Note what differs: timing and the level/seed are expected, anything else is not. Never commit the PNGs.
- [ ] **Step 4: `liero-shot` §7.** Add one paragraph after the 4½d one: MATCH SETUP (F7) and the settings focus, number entry and the phone text field, WEAPON OPTIONS, `liero.cfg` at boot/exit and `--config-root`, the refusal boxes, the small labels, and the gates (G2e-1, G3).
- [ ] **Step 5: PROGRESS.**
  - Real date. A new header paragraph, **"4½e-1 LANDED"**; the previous one becomes "Prior (…)".
  - The Step 4½ tree: split the 4½e line into ✅ 4½e-1 (with the milestone) and ⬜ 4½e-2.
  - **Open for John** / notes:
    - the plan-time facts that corrected the design (1–3, 7–9, 14, 15, 17, 18, 21, 22, 25);
    - the `level_path` pull-forward (fact 27);
    - D5's texts;
    - D9's iOS "TAP TO TYPE" rule;
    - D10's missing `portable.txt`;
    - D12's `rust/settings` move, still open;
    - the known live-only divergences: the D8 per-char split, and the refusal boxes being Rust-only;
    - T0's results.
- [ ] **Step 6: the overview.**
  - The status line: `4½e-1 LANDED`.
  - At the end of the 4½e bullet: "**4½e-1 landed**", with the plan path and the corrections: finding 1 (shared settings; LD 3's RESUME write), finding 5 (no RANDOM preview: overview Q5 corrected), and fact 14.
  - Q6: "ruled A (today's set)".
- [ ] **Step 7: the maps and the design.**
  - **cpp-map:** append, at the relevant sections, finding 1 (a paused match shares `gfx.settings`; LOAD SETUP breaks it), finding 2/3's path forms (with T0's strings), and facts 1, 3, 6, 7, 9, 11, 12, 14, 15.
  - **rust-map:** `ConfigStore` is `Send + Sync` with `root_label`; `level_path`; `Scene::small_labels`; `InputEvent`.
  - **Design status line:** `**4½e-1 LANDED** (plan plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md); 4½e-2 planned`.
- [ ] **Step 8: the broad review.** Re-read `git diff 2377b0c -- rust src web .github` against the design and this plan:
  - each finding and each fact is either ported and gated (name its G2e-1 case, G3 case or unit test) or recorded in PROGRESS;
  - the settings focus follows `mainMenuState.cpp:152-626` in order;
  - `Shell::frame` follows `gfx.cpp:1467-1652` with the buried main menu, push-after-update and replace;
  - the dumpers' interventions are each documented, touch no code under test, and refuse what they cannot run faithfully;
  - LD 3, LD 4 and LD 5 and the crate rules hold;
  - `game` is glue only;
  - the non-shell paths are unchanged;
  - no push, no PR.

  Bar: 0 Critical / 0 Important.
- [ ] **Step 9:** `git status --short` lists only the docs above. **Commit** `docs(4.5e-1): PROGRESS + overview + maps — slice 4.5e-1 landed (the settings menu, weapon options, number entry, liero.cfg, the small labels; G2e-1 + G3)`.

**Done when:** the board is green, every audit matches its expected output, the docs are updated, and the review is clean.

---

## Known pitfalls (from 4½c, 4½d and PROGRESS; read them before your batch)

1. **Goldens are C++ truth.** Never regenerate one to make Rust pass, and never hand-edit one. An `M` under `golden/` means: stop, restore, report. A dumper change must re-prove byte-identity for every golden its binary writes (T5: the 11 shell goldens; T6: all 39 sim_physics scripts).
2. **DEBUG only.** `--release` breaks the `#[should_panic]` tests. The one non-test build is the wasm-release bundle.
3. **clang-format 18 is on PATH.** Always use `$CF22`. Always also run the whole-file dry run: the diff script misses a blank line that a deletion leaves behind (CLAUDE.md).
4. **This clone has no `origin/master`.** Pass `2377b0c` to both diff scripts.
5. **Gen scripts default to `macos-arm64`.** Source `env.sh` in the same shell, or the script configures a preset that does not exist here.
6. **rustfmt recurses** through `lib.rs`/`main.rs`. Use it only on files you created; hand-format the rest.
7. **The selection constructor must draw nothing** (intervention 3). Every NEW GAME's picks must name *enabled* weapons (`weapsel.cpp:66`). A setup that bans weapons must move the picks (T8 cases 4, 7 and 10).
8. **A worm key held through Esc and released in the menu** stays pressed in C++ and not in Rust (4½d fact 12). The corpus refuses it: release every worm key before the Esc.
9. **The search timeout is wall-clock in C++** (1500 ms) and `now_ms = 0` in the Rust harness. Keep each search sequence within one weapon-options visit and fast. The dumper's gap check fails anything slower. Never write a case that relies on the timeout.
10. **Letters typed in WEAPON OPTIONS are also control keys.** R, F, D and G move P1 (Up, Down, Left, Right). Search only with unbound letters.
11. **SDL text events are one string each.** A multi-char string becomes `'?'` in C++. The corpus types one char per event (fact 7).
12. **Text after Return in the same frame still lands** (fact 6). Do not "fix" it.
13. **The 4½d header and line shapes must not move** for cases without `detail`/`fs`, or all 11 4½d goldens flip.
14. **`Scene` literals exist outside `render`.** Add `small_labels: None` everywhere (fact 17). The flag must be `None` on every golden path, or old render goldens flip.
15. **Headless Chromium runs at about 7–10 fps.** A tap longer than 12 ticks trips weapon selection's key repeat, and `KeyQueue` defers a key's second event (4½d fact 19). Keep taps short in `e1.mjs`. Focusing an input without a user gesture does nothing on iOS (D9).
16. **The native `game` cannot run here** (no GPU). Do not burn time on it. The browser bundle and the G2 harness cover the live path; restart `http.server 8765` if it is down.
17. **Disk.** Clean the incremental dirs after every batch; `df -h /` before any wasm-release build; no second target dir unless the worktree rule's 8 GB is free.
18. **Explicit `git add` paths only**, because batches may share a checkout. Never commit the scratchpad, PPMs or PNGs.
19. **The settings live-read set is not hashed.** A missed field shows up only as a drift *after* an edit. The counterfactual witnesses in T8 exist to prove the resync matters. Keep them.
20. **`Utf8ToDos` returns `char` in C++** (signed on x86): `k` can be negative for å. It is not gated (ASCII only). Do not widen the gate to non-ASCII names.
21. **Holdazone is refused in Rust and plays in C++.** No G2 case may NEW GAME or RESUME into Holdazone; passing through it in the menu is fine.
22. **Trailers.** Every commit carries `$CO` and `$SESS`, and nothing else names a model. No "Generated with …".

## Done-report (each batch)

Each batch reports:
- (a) what changed and why;
- (b) the files touched;
- (c) the gate commands run, with their results;
- (d) the commit SHAs.

Batch 1 also reports the addendum's verdicts. The final report (Batch 9) surfaces:
- T0's outcomes;
- the G3 result, with any sim fixes and each case's witnesses;
- the dumper evidence: the four T5 smokes and T6's regeneration proof;
- 🎯 the G2e-1 result: 10 cases, the frame and `d`-line counts, the saved-file lines, `match_setup` named on its own;
- the counterfactual divergence ticks;
- the Chromium lines;
- the Xvfb PNG paths;
- the audit sweep: 39 `A`, 0 `M`; the two dumpers; `cargo tree` 0 bevy; reproducibility byte-identical.

---

## Addendum T0 (probe results)

Run 2026-09-26 on `claude/cpp-oracle-vcpkg-assets-chcwcm` at `64db8af`. `$S` is the scratchpad from §Working environment. Every probe artefact stays in `$S/t0e1/` and `$S/t0e2/`; nothing but this addendum is committed. **Verdict: findings 1, 2, 3, 12 and fact 29 are confirmed. No contradiction rule fired, and no task changes.** There are two precisions: the relative root-label form (fact 28) and which save wrote run A's `liero.cfg` (Step 6).

### Finding 1: CONFIRMED, for both a sim field and a draw field

- **Dumper patch.** It lived in the working tree only and is kept as `$S/t0e1/probe_patch.diff`. It makes `TopOf` return `'?'`, and it adds a 12th `f` field: `%08x` `HashGameState(*gfx.controller->CurrentGame())` when the top is `G` and `!InWeaponSelection()`, else `-`.
- **Build and restore.** `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell`. It was restored with `git checkout -- src/tools/oracle_dump/shell_dump.cpp` and rebuilt. `git status --short src` is empty. The restored binary regenerates `shell_boot_idle.txt` and `shell_milestone.txt` byte-identically (`cmp`).
- **Scripts.** `$S/t0e1/p_{ctl,bonus,map}.txt`, generated by `$S/t0e1/gen.py`. Each has `setup default`, `boot_seed 7`, `match_seed 301`, `frames 600` and `expect frames`. The common key path is:
  - `RETURN` at 40 (NEW GAME); weapon selection is on top from frame 73;
  - `R`+`UP` at 75, then `LCTRL`+`RCTRL` at 78 (DONE); the match runs from frame 79;
  - P1 `G` held 82–150 with `LCTRL` taps at 90/110/130, P2 `LEFT` held 152–250 with `RCTRL` taps at 160/190/220, and an `LCTRL` tap at 256. Everything is released by 258;
  - `ESC` at 285, and the menu is on top from frame 316;
  - `F7` at 336, then the edit, then `ESC` at 392 and `F1` (RESUME) at 396;
  - frame 429 is the router frame (`upd M`, top `G`). **Frame 430 is the first resumed tick**, and 171 frames run from 429 to 599.
- **The edits.**
  - `p_ctl` has none.
  - `p_bonus`: `DOWN` ×7 taps at 340+3i, then `LEFT` held 362–387.
  - `p_map`: `DOWN` ×9 taps at 340+3i, then `RETURN` at 368.
  - The dumper's own PPMs of frame 390 (`$S/t0e1/png/{bonus,map}_0390.png`) show MAX BONUSES `0` and MAP `OFF`, with every other value unchanged.
- **Commands.** `build/linux-x64/Release/oracle_dump_shell $S/t0e1/p_<v>.txt $S/t0e1/p_<v>.out` for each variant. `paste`/`awk` then compared the three outputs field by field.

**Results:**

| | `p_bonus` vs `p_ctl` | `p_map` vs `p_ctl` |
|---|---|---|
| boot, `f 0` … `f 339` | byte-identical (the `F7` frame 336 included) | byte-identical |
| first differing line | `f 340` (bmp16 and sounds: the first edit key) | `f 340` (same) |
| frame 429 (router, no tick) | state `1ef36b1a` = `1ef36b1a` | state `1ef36b1a` = `1ef36b1a` |
| **frame 430, first resumed tick** | state **`b835f450` ≠ `bd41b2d9`** | state `b835f450` = `b835f450`; bmp16 **`ff42bbdfdb2663eb` ≠ `a58e06f52cb45e6f`** |
| frames 429–599 (171) | state equal on 1/171 (only 429) | state equal on **171/171**; bmp16 differs on **171/171** |

- **`p_bonus`.** The state hash differs on the first resumed tick. With `max_bonuses 0`, the `game.cpp:359` roll no longer draws `rand`. The *sim* read of a paused match sees the menu edit.
- **`p_map`.** The sim stays identical for all 171 ticks, and every presented play bitmap differs. `$S/t0e1/png/{ctl,map}_0450.png` show the minimap present and gone. The *draw* read sees the edit too.
- **Conclusion.** The running `Game`'s `settings` *is* `gfx.settings`. Design finding 1 and fact 16 stand as written. T2's `apply_live_settings`, T4's RESUME resync (sim fields and draw fields) and T8's `live_settings` witnesses stay as planned. `blood`, `loading_time` and `weap_table` were not probed on their own. That is not needed: a full contradiction or a split would have required it, and neither happened. T8's counterfactual witnesses cover them.

### Finding 2: CONFIRMED (Q4's fix is live, not moot)

- **Setup.** `Xvfb :99 -screen 0 1280x800x24`. Every run used `DISPLAY=:99 SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy build/linux-x64/Release/openliero`, started by `$S/t0e2/start.sh <R>` and driven by `$S/t0e2/k.sh` (xdotool taps, one typed char per `xdotool type`, `import` snaps).
- **The runs.** A, B and C are the plan's. A0 and D were added for facts 28/29 and finding 3:
  - **A:** `OPENLIERO_TEST_USER_DIR=$S/t0e2/A/user` (absolute) and `OPENLIERO_DATADIR=/home/user/openliero/data`.
  - **B:** A, plus `user/TC/openliero/Levels/water_stage.lev` copied in.
  - **C:** `--config-root $S/t0e2/C/root`, with `root` a copy of `data/`.
  - **A0:** cwd `$S/t0e2/A0`, `OPENLIERO_TEST_USER_DIR=user` (relative, the fixture form), `OPENLIERO_DATADIR=$S/t0e2/A0/sys` (a copy of `data/` **without `Setups/`**), and no `user/` beforehand.
  - **D:** the production default. `XDG_DATA_HOME=$S/t0e2/D/xdg` makes `SDL_GetPrefPath` the user root (no `OPENLIERO_TEST_USER_DIR`), with `OPENLIERO_DATADIR=/home/user/openliero/data`.
- **Key path, the same in every run.**
  - `F7`, `Down` ×2 (LEVEL), `Return`. The root listing is `[RANDOM]`, Profiles, Resources, Setups, TC, and the title shows the full path.
  - `Down` ×4 to TC, then **`Right` ×3**: TC → `openliero` → `Levels`. The plan's text counts two, but `Right` on the TC row enters TC, which lists only `openliero`. `Levels` is the first row of `openliero` in every run, C included, so no extra move is needed.
  - Type `water`, then `Return`. The `r` (P1 Up) did not disturb the cursor; the search left it on `water_stage`.
  - `Escape`, `F1` (weapon selection), `Escape`, `Escape`, `Return` (QUIT TO OS). The game exited each time.
- **Verdicts.** PNGs are in `$S/t0e2/<R>_*.png`.
  - **A:** `A_6_weapsel.png` shows `Level: "water_stage"` over a **random** dirt level. Its minimap differs from `A_4_typed.png`'s water_stage preview.
  - **B:** `B_6_weapsel.png` shows the real **water_stage**: the flat level with the water band, matching the preview minimap. The merged listing shows `water_stage` once.
  - **C:** `C_6_weapsel.png` shows **water_stage**.
  - **A0 and D:** random, like A.
  - A system-only level chosen in the split layout therefore plays random. The saved path names the user layer, where the file is absent, and `GenerateFromSettings` falls back to random (fact 26). With the file in the user layer (B), or in single-directory mode (C), it plays.
  - A side observation (C++ behaviour, not a finding): with a file level, SettingsMenu hides MAP WIDTH and MAP HEIGHT, and REGENERATE LEVEL reads **RELOAD LEVEL** (`A_5_level_set.png`).

### Findings 3 and 12: CONFIRMED, and the exact `levelFile` strings

Every run saved `Setups/liero.cfg` at exit (`gameEntry.cpp:78`, finding 12) with `randomLevel = false` and:

| Run | Root as given | Saved `levelFile` |
|---|---|---|
| A | `$S/t0e2/A/user` | `'/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/t0e2/A/user/TC/openliero/Levels/water_stage.lev'` |
| B | `$S/t0e2/B/user` | `'…/t0e2/B/user/TC/openliero/Levels/water_stage.lev'` |
| C | `$S/t0e2/C/root` (`--config-root`) | `'…/t0e2/C/root/TC/openliero/Levels/water_stage.lev'` |
| A0 | `user` (relative) | **`'./user/TC/openliero/Levels/water_stage.lev'`** (the selector title read `./user/TC/openliero/Levels`) |
| D | `SDL_GetPrefPath` = `…/t0e2/D/xdg/openliero/openliero/` (trailing `/`) | `'…/t0e2/D/xdg/openliero/openliero/TC/openliero/Levels/water_stage.lev'` (**a single `/`**) |

**`root_label` form, decided by this probe.** The label is the root's `FsNode::FullPath()`, and the `level_path` rule-1 prefix is `root_label() + "/"`. The table matches the source: `FsNode(std::string)` (`filesystem.cpp:596-650`) splits on separators and re-joins through `Go` → `JoinPath` (`:283-288`), with these consequences:
- **No trailing separator, ever.** D's `SDL_GetPrefPath` ends in `/` and the saved path has exactly one. T2's `NativeStore` default label strips any trailing separator.
- **A relative root gains a `./` prefix.** `user` becomes `./user`. This corrects fact 28's "`FsNode("user")`" wording: the string is `./user`, which is exactly what §Formats' `with_root_label("./user")` already uses. The fixture needs no change.
- **An absolute root is unchanged**, apart from the trailing-separator strip.

T2's `NativeStore::root_label` follows these three rules. The fixture overrides its label explicitly anyway.

### Fact 29 and the Step-6 fixture facts

- **A's `ls -R` after the run:** `user:` contains `Setups`; `user/Setups:` contains `liero.cfg`. `user/` was empty before (`$S/t0e2/A_ls_before.txt`). This file came from the **exit** save (`gameEntry.cpp:78`), not the boot fallback. A's system layer has `data/Setups/liero.cfg`, so the boot `LoadSettings` of the merged view (`:55`) succeeded and nothing was saved at boot.
- **The boot fallback itself is shown by A0.** A0's system layer has no `Setups/` and there is no `user/`, so `A0_ls_before.txt` lists only `sys`. `ls -laR user` 3 s after launch, before any key, shows `user/Setups/liero.cfg` (2008 bytes, `levelFile = ''`, `randomLevel = true`; the copy is `$S/t0e2/A0_boot_liero.cfg`). `UserDataRoot` created `user/` (`CreateDirectories`), and the writer created `Setups/` (fact 29).
- **Fixture consequence.** `cfg_default` must have no `liero.cfg` in **either** layer, as T8 case 9 already says ("no `liero.cfg` anywhere"). A `sys/Setups/liero.cfg` would suppress the boot save.

### What changed in this plan

Nothing in any task. Fact 28's string is `./user`, and T2's root-label rules are the three above. Both agree with §Formats as written.

## Addendum G3 (Batch 6 finding + John's ruling, 2026-09-26)

**Finding.** C++ spawning reads outside the level on maps smaller than the TC's `WormSpawnRect` (5,5 + 494×340):
`Worm::BeginRespawn` (`worm.cpp:724-739`) draws candidates from the fixed rect, and `CheckRespawnPosition`
(`game.cpp:611-649`) clamps only the max corner and walks with `!=` bounds, so a candidate row below the level walks
past `materials[]`; the drop-down `Mat(x, y+4)` can too. A candidate right of the level reads on through following rows
via the flat index — defined while it stays inside the vector. C++ segfaults or silently reads garbage; Rust indexes
the same flat array and panics where C++ leaves the vector. Any MAP HEIGHT below ~342 can hit it; below ~165 every
first spawn does. G3's `small` case therefore moved from 96×64 to 96×344 (`6c2dfd8`), and `tall` runs 4,960 ticks.

**Ruling (John): safe edges.** In Rust, a material read whose flat index falls outside the level's array counts as
solid rock (not background), so a spawn candidate touching it is rejected and the worm picks another. In-array reads —
including C++'s defined flat-index wrap for `x ≥ width` — are unchanged, so every run on which C++ stays inside the
vector stays bit-exact (all existing goldens and G3 re-diff unchanged). This is a documented, intended divergence only
where C++ has undefined behaviour. Implement it at the sim's material accessor(s) used by the spawn path (and any
other path Batch 6 showed reaching past the array), with unit tests on a 96×64 and a 160×120 level proving no panic and
a successful spawn, plus a full re-diff. Batch 7's `map_size` G2 case must still use a UB-free size (height ≥ 342),
run against an assertions/ASan build of the C++ dumper (`$S/build-chk/`).
