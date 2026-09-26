# Step 4½, Slice 4½e-2: the level selector, SAVE SETUP AS… / LOAD SETUP, the wasm level catalogue, the shipped-level fix (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN). This plan names types, fields, files and behaviour; it does not write the code. Where it quotes C++ it quotes the source, and the source wins over this text.

**Goal.** The last three settings-menu items that 4½e-1 left sound-only become live, exactly as in C++:
- **LEVEL** pushes the level selector: the whole config tree, `[RANDOM]` first, folders in colour 47, the parent pane, type-to-search, cursor restore on `level_file`, and the minimap preview that lands one frame late and stays in the main menu.
- **SAVE SETUP AS…** opens the name box on the current setup name. A reserved or shipped name gives the black `NAME '<leaf>' IS RESERVED` box, and the input then reopens with what was typed. Any other name is saved to `Setups/<name>.cfg`.
- **LOAD SETUP** opens the options selector inside `Setups` and loads the picked file. That also detaches a paused match (finding 1's other half).

Every presented frame on these paths is bit-exact against the **real C++ `Gfx::RunOneFrame`**, and so is every file in the user folder (G2e-2, 🎯 `shell_setups_and_levels`). The one intended difference (John's Q4) is that **a picked shipped level is played**, where a default C++ install plays a random level. That difference is proven, and it is never gated as a mismatch (D1). The browser lists the embedded levels, and `?level=` stores the canonical config-root path. On a phone, the name box raises the text keyboard and the number box the numeric one.

**Architecture.**
- `render`: `hud::draw_miniature_ids`, a `pub` form of the private HUD miniature over raw material ids (hash-neutral).
- `scenario`:
  - `ConfigStore::list(rel) -> Vec<DirEntry>`, which is C++ `DirectoryListing` over the merged layers;
  - `MemoryStore` directories (key prefixes plus explicit empty dirs) and a single-layer mode (the C++ web build's `--config-root /openliero`);
  - `NativeStore::with_shadow_root` (C++ `ShadowsSystem` consults `SystemDataRoot()`);
  - `storage::placeable_leaf`;
  - `assets::EMBEDDED_LEVELS`, shared by the wasm TC reads and the browser store, plus the browser tree (`browser_system_files`, `EMBEDDED_DIRS`).
- `ui::shell`:
  - a new `files` module: an arena `FileNode` tree filled lazily through the store, `FileSelector` (`Fill`, `ChildSort`/`CiLess`, `Find`/`Select`, `CurSel`, `Enter`, `Exit`, `Process`, `Draw`), `LevelSelectorState` (RANDOM, restore, the late preview) and `SetupSelectorState` (C++ `OptionsSelectorState`);
  - `Screen::{LevelSelect, SetupSelect}` (tops `L`, `P`);
  - the selectors' `Picked` continuation;
  - `InputPurpose::SaveSetupAs` and `InfoPurpose::Reserved` (the Save-As chain through the e-1 scheduled replacement);
  - the LOAD SETUP continuation (replace the settings, `setup_name`, detach a running match, `write_back_picks` only while attached);
  - `level_path::cpp_accepts` (C++ `Level::load`'s accept rules for reads and previews).
- `oracle_dump_shell` gains:
  - the tops `L`/`P`;
  - the search-gap check for them;
  - **intervention 6′** (`record_replays = false` after every frame, because LOAD SETUP swaps the settings object);
  - **the Q4 guard**: an `fs` case must never NEW GAME into C++'s shipped-level bug.
- `game`:
  - the browser store (single layer: the setups, the 4 small levels, `tc.cfg`, the object `.cfg`s and the `data/` directory skeleton, root `/openliero`);
  - `?level=` in the canonical root form;
  - the per-box `window.lieroTextMode` (`numeric` / `text`) and its page;
  - selector hooks for the headless walk;
  - the `--config-root` shadow root.

**Tech stack.**
- Rust 2021 for `sim-core`, `sim`, `render`, `scenario`, `shot` and `oracle-tests`; Rust 2024 for `ui` and `game`.
- C++ in `src/tools/oracle_dump/shell_dump.cpp` only, preset `linux-x64`, clang-format 22.
- HTML/JS in `web/index.html`.

**Spec:** `docs/superpowers/specs/2026-09-26-liero-rs-step4.5-slice4.5e-settings-menu-design.md`, cited as **design §N**; its findings are cited as **finding N**. The precedent plan is `plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md` (**e-1 plan**; its facts are **e-1 fact N**, its decisions **e-1 DN**, its addenda **e-1 Addendum T0 / G3**).

**Rulings (John, 2026-09-26, design §14).** These are the ones this part executes:
- **Q4 = A.** Fix the shipped-level bug. A picked level is played. It is the one intended difference, documented and not gated.
- **Q5 = A.** On a phone a text box raises the device keyboard, and SAVE SETUP AS… needs letters. So the page's input mode is set **per box** (D11).
- **Q6 = A.** Today's level set: RANDOM plus the 5 test levels on desktop, and RANDOM plus the 4 small ones on the web. `modern_test` stays out of the wasm build.
- Q1, Q2 and Q3 stand as in e-1: two parts, the Holdazone refusal box, and the C++ config root.

**Base.** The slice base is **`f39b5ac`**, the merge of 4½e-1 (PR #16) into `liero-rs-step-4-5`. It is used for every golden audit, every clang-tidy diff and every review diff. HEAD at plan time is `f39b5ac`.

---

## Working environment (read before any task)

- **Repo and branch.** `/home/user/openliero` on `claude/cpp-oracle-vcpkg-assets-chcwcm`, just reset onto `origin/liero-rs-step-4-5` at `f39b5ac`. Use absolute paths. No sub-subagents.
- **Scratchpad.** `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad`. Every scratch script, PPM, PNG, probe, fixture copy and log goes here, never into the repo.
- **C++ oracle build.**
  - In the same shell as every cmake or gen-script call, first run `source $S/env.sh`. It sets `PRESET=linux-x64`, `VCPKG_ROOT`, the vcpkg asset script and the sdl3 overlay.
  - The build dir is `build/linux-x64` (Ninja Multi-Config, already configured with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON` and `CMAKE_EXPORT_COMPILE_COMMANDS`). Binaries go to `build/linux-x64/Release/`: `oracle_dump_shell`, `oracle_dump_sim_physics`, `oracle_dump_menu`, `oracle_dump_settings`, `oracle_dump_weapsel` and the real `openliero`.
  - Build a target with `cmake --build build/linux-x64 --config Release --target <t>`.
  - The gen scripts default to `macos-arm64` and honour `PRESET`, so always run `source $S/env.sh && bash rust/oracle-tests/<script>.sh` from the repo root.
- **The checked C++ build** (`-D_GLIBCXX_ASSERTIONS -fsanitize=address`, single-config Ninja) is at `$S/build-chk`, with the binary at `$S/build-chk/oracle_dump_shell`.
  - Rebuild it with `source $S/env.sh && cmake --build $S/build-chk --target oracle_dump_shell`.
  - `$S/b7chk/run.sh` is the loop template: copy it, never edit it. It runs every `shell_*_script.txt` with `ASAN_OPTIONS=detect_leaks=0` and `cmp`s each output against its golden.
  - e-2 must run the whole shell corpus through it (T5).
- **clang-format is pinned to 22.** The system `clang-format` is 18 and must never be used.
  - Get the binary with `CF22=$(uvx --from clang-format==22.1.0 sh -c 'command -v clang-format')`.
  - Diff check: `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh f39b5ac`.
  - Whole-file check, for every touched C++ file (CLAUDE.md: a diff check misses context): `"$CF22" --dry-run -Werror --style=file <abs file>`.
  - Fix: `"$CF22" -i --style=file <abs file>`.
- **clang-tidy.** `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 f39b5ac`. This clone has no `origin/master`, so always pass the base. It must exit 0. Fix the code. A `NOLINTNEXTLINE(<check>) — <reason>` is allowed only where the repo already uses one for the same check.
- **Rust.** Run everything from the repo root, in **DEBUG only**. Never use `--release`: some tests are `#[should_panic]` on `debug_assert!`s.
  - The re-diff is **both**:
    - `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game`
    - `cargo test --manifest-path rust/Cargo.toml -p game`
  - **NEVER** run `cargo test --workspace` including `game` (disk).
  - The wasm check is `cargo build --manifest-path rust/Cargo.toml --profile wasm-release -p game --target wasm32-unknown-unknown`.
  - Never use `--target-dir` or a second target tree.
- **Disk is tight.** About 16 GB is free at plan time, and `rust/target` is 14 GB.
  - After every batch run `rm -rf rust/target/debug/incremental rust/target/wasm32-unknown-unknown/*/incremental`, and delete the scratch PPM/PNG dirs you created.
  - Before a wasm-release build, `df -h /` must show at least 3 GB free.
  - **Rust batches run sequentially in this one checkout.** A C++-only batch may run alongside a Rust batch, because its files are disjoint and `build/linux-x64` is not `rust/target`.
- **The native Rust `game` cannot run here.** Bevy panics with "Unable to find a GPU" under Xvfb. The **browser bundle** stands in for live smoke tests:
  - Build it with the wasm check above, then `wasm-bindgen --target web --out-dir $S/site --out-name game --remove-name-section --remove-producers-section rust/target/wasm32-unknown-unknown/wasm-release/game.wasm && cp web/index.html $S/site/ && rm -f $S/site/*.d.ts`.
  - Serve it with `cd $S/site && python3 -m http.server 8765`, run in the background. Check it with `curl -sI http://localhost:8765/`, and restart it if it is down.
  - Playwright lives under `/opt/node22/lib/node_modules/`. The templates are `$S/b8.mjs` (the e-1 walk: desktop keys through `frameTap`, an emulated phone, the `lieroPhase`/`lieroTop`/`lieroSel` hooks) and `$S/tap1.mjs` (real CDP touch). Copy one, never edit it.
  - Headless Chromium runs at about 7–10 fps under SwiftShader. **The worms start dead and respawn**, so wait about 30 s after a match starts before any in-match check.
- **The real C++ game under Xvfb** (eyeball artefacts and T0 probes, never gates):
  - `Xvfb :99 -screen 0 1280x800x24 &`, then `DISPLAY=:99 SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy build/linux-x64/Release/openliero [--config-root <dir>] &`.
  - Drive it with `xdotool`: `$S/xd.sh` has `tap` / `snap`, `$S/t0e2/k.sh` is e-1 T0's key driver, and `xdotool type` types text.
  - Screenshot with `import -window <id>`. Pair images with `$S/x12_pair.py`, and convert PPMs with `$S/ppm2png.py`.
- **Golden rule.** New goldens come **only from the real C++**. Never regenerate a golden to make Rust pass, and never hand-edit one. Every existing golden must stay byte-identical, audited against `f39b5ac`. If `git status --porcelain -- rust/oracle-tests/golden` ever shows an `M`:
  1. stop;
  2. `git checkout -- rust/oracle-tests/golden`;
  3. report the file and the first differing line;
  4. do not commit.
- **Commits.** Use the globally configured identity. Stage **explicit paths only**: never `git add -A` or `git add .`, because a C++ batch may share the checkout. Every commit carries exactly these two trailer paragraphs, and no other text anywhere names a model:
  ```
  CO='Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>'
  SESS='Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT'
  git -C /home/user/openliero commit -m "<subject>" -m "$CO" -m "$SESS"
  ```
  Never write "Generated with …". **Do NOT push and do NOT open a PR.** The orchestrator pushes after each verified batch.

## Plan-time facts checked against the source (the source wins)

Every task below was written against HEAD `f39b5ac`. Where the design and the source disagree, the plan follows the source, and T8 records each item in PROGRESS and the maps. T0 re-checks the facts marked **(T0)** with the real C++.

**The selectors (C++).**

1. **Titles** (T0). `LevelSelectorState`'s `title_` is empty (`fileSelectorState.cpp:71`). `DrawExtra` draws its own title: `DrawRoundedBox(178, 20, 0, 7, GetDims(t))` plus the string at (180, 21) in colour 50 (`:146-155`). Those are the pixels of `Font::DrawFramedText(…, 178, 20, 50)`, which e-1 ported as `draw_framed_text`.
   - `OptionsSelectorState` uses the base `FileSelectorState::Draw`'s framed title, `"Select options:"` (`:52-63`, `:210`).
   - Both titles are text + `' '` + the current folder's `full_path`, and the space and path are dropped when the path is empty.
   - Texts: `SelLevel = "Select level:"` (`tc.cfg:255`) and `Random = "[RANDOM]"` (`tc.cfg:239`). `"Select options:"` and `"Parent directory"` are C++ literals (`fileSelectorState.cpp:210`, `menu/fileSelector.hpp:173`).
2. **Draw order** (`fileSelectorState.cpp:52-67`, `fileSelector.hpp:171-178`):
   1. `bmp ← frozen_screen`;
   2. the framed title (options only);
   3. `DrawExtra`: the level selector's preview goes **into `frozen_screen`**, then its title onto `bmp`;
   4. the parent pane, **only when the current node has a parent**: `"Parent directory"` framed at (28, 20), and the parent's menu drawn disabled at x = 28 with its selection shown;
   5. the current menu, enabled, at x = 178.
3. **`FileNode` menus** (`fileSelector.hpp:22-63`). Each node has a `Menu(178, 28)` with `SetHeight(14)` (`centered = false`, `value_offset_x = 0`, `menu.hpp:26-49`).
   - The menu is built on the first `GetMenu()` from the children: `MenuItem(folder ? 47 : 48, 7, name)` with id -1, then `MoveToFirstVisible()`.
   - An empty folder's menu is rebuilt on every `GetMenu()`. That is harmless.
   - Movement, PgUp/PgDn and search on an empty menu are UB-free in C++. `FirstVisibleFrom(-1)` iterates from `size_t(-1)` and returns `size()`, `MovementPage` ends in `MoveTo(size())` → `FirstVisibleFrom(-1)` (`menu.cpp:129-134`, `:169-177`, `:250-261`). The Rust port matches: `first_visible_from(<0)` returns `len` (`ui/src/menu/mod.rs:264-271`).
4. **`FileNode::Fill`** (`fileSelector.hpp:132-154`).
   - It walks `DirectoryListing(fs_node.Iter())` and adds **every directory**, and each file whose `filter(name, GetExtension(name))` is true.
   - The filter is `CiCompare(ext, "LEV")` or `"CFG"`. `GetExtension` is the text after the **last** `.`, so a name without a dot has ext `""` and is rejected.
   - A file node's `name` is `GetBasename(entry)`, i.e. up to the last `.` (`filesystem.cpp:47-63`). Its `full_path` is `JoinPath(parent.full_path, entry name)`.
   - Then `ChildSort`: folders first, then `CiLess` on `name` (`:121-130`). `CiLess` compares per byte with `std::toupper` (the C locale, i.e. ASCII only), and a proper prefix sorts first (`text.cpp:51`, `:81-96`).
   - The sort is `std::ranges::sort`, which is unstable: names equal under `CiLess` have unspecified order. (libstdc++ insertion-sorts ≤ 16 elements, so it is stable in practice there.)
5. **`DirectoryListing`** (`filesystem.cpp:294-327`, `:335-340`; `filesystem.hpp:47-53`).
   - It skips only `.` and `..`, so **dotfiles are listed**.
   - `stat` follows symlinks, and an entry whose `stat` fails (a broken link) is dropped.
   - A name ending in `.zip` (case-sensitive) becomes a *folder* named without `.zip`.
   - The config node's `Iter()` is `user.Iter() | system.Iter()`: the user entries, then the system entries, re-sorted **bytewise** by name and de-duplicated by name. The first equal entry is kept, and after an unstable sort "first" is unspecified.
6. **The root's `full_path`** is the config node's `FullPath()`, which is the user node's (`FsNodeJoin::FullPath`, `filesystem.cpp:356-362`; `fileSelector.hpp:164-169`). That is `./user` in the fixture, `/openliero` on the C++ web build, and natively the pref path without a trailing separator (e-1 Addendum T0).
7. **RANDOM** (`fileSelectorState.cpp:78-103`).
   - It is `FileNode(LS(Random), "", "", /*folder=*/false, &root)`, inserted at `root.children[0]` **after** the root's `Fill` and before any `GetMenu`. Its `full_path` is `""`.
   - `OnSelected(RANDOM)`: `random_level = true` and `level_file.clear()`.
   - `OnSelected(a file)`: `random_level = false` and `level_file = node.full_path`.
   - Both then run `settings_menu.UpdateItems`. They play no sound besides the Enter's `MenuSelect`.
8. **`Select` / `Find`** (`fileSelector.hpp:66-87`, `:184-214`) (T0).
   - `Find` matches on `CiCompare(full_path, path)`, or recurses when `CiStartsWith(path, full_path)` holds, with **no separator check**. It `EnsureFilled`s every prefix-matching folder on the way. RANDOM's `""` prefix-matches every path but is not a folder, so it is skipped.
   - On a hit, every ancestor's `selected_child` is set, and its cursor moves onto the path (`GetMenu().MoveTo(i)`, which builds that menu). The current folder becomes the hit if it is a folder, else the hit's parent.
   - A miss leaves the root current, on its first entry. Misses include `""` (random), 4½d's TC-relative `Levels/x.lev`, and a path under another root. So the level selector opens on `[RANDOM]`.
   - The options selector `Select`s `JoinPath(root, "Setups")`, a folder, so it opens *inside* `Setups`. The cursor is on its first entry, built by the first draw's `GetMenu` → `MoveToFirstVisible`.
9. **`Update` + `Process`** (`fileSelectorState.cpp:25-50`, `fileSelector.hpp:251-300`), in this order (T0):
   1. Up plays MoveDown and Down plays MoveUp; PgUp plays MoveDown and PgDn plays MoveUp. Each is `TestSdlKeyOnce(...) || TestControlOnce(...)`, and PgUp/PgDn are keyboard-only.
   2. Esc or any Jump: leave. `done_`, the state pops that frame, with no sound and no `ClearKeys`.
   3. Once-Left: `Exit()`, to the parent, with no sound; at the root it does nothing.
   4. Once-Right: `Enter()`. A folder row enters the folder with no sound; a file row does nothing.
   5. `OnKeys(key_buf, contains = true)`.
   6. Then, in `Update`: Return, KP Enter or any Fire plays `MenuSelect`, then `Enter()`. A folder is entered and the state stays. A file goes to `OnSelected`, which returns true, and the state pops. Nothing else in that `Update` runs after `OnSelected`.
10. **The preview** (`fileSelectorState.cpp:105-145`) (T0).
    - It runs only for a **file ≠ RANDOM** that differs from `previewNode_`.
    - It reads through the joined node (`GetFsNode` = parent `fs_node / path_name`) and calls `Level::load(common, *gfx->settings, r)`. An exception is caught: `EndOfStream` is a `std::runtime_error` (`io/stream.hpp:18`, `:41-46`).
    - On success:
      - it runs `FillRect(frozen, 134, 162, prev_cols, prev_rows, 0)`;
      - then `DrawMiniature(frozen, 134, 162, sx, sy)`, with `sx = max(ceil(w/52), 1)` and `sy = max(ceil(h/36), 1)`;
      - then sets `prev_cols = (w + sx/2)/sx` and `prev_rows = (h + sy/2)/sy`.
    - `previewNode_ = sel` is set **even when the load failed**. `prev` starts at 52×36 at `Enter` (`:88-90`).
    - `DrawMiniature` writes `AppearanceAt(idx, dest.mode, dest.pal32, dest.cycles)` (`level.cpp:489-507`, `level.hpp:59-64`). In classic mode that is `pal32[material_id]`, and the frozen screen's `pal32` is the play renderer's live array (`gfx/bitmap.hpp:56-60`): **this frame's menu palette**.
    - The spectator miniature (`:128-136`) is skipped, because the spectator window is deferred.
11. **`Level::load` accepts or rejects** (`level.cpp:229-392`). It is:
    - (a) `OLLEVEL2`: 5 header bytes present, and `1 ≤ w, h ≤ 4096`; otherwise legacy 504×350;
    - (b) the material bytes complete (`r.Get` throws otherwise);
    - (c) if `settings.load_powerlevel_palette` (default `true`, `settings.hpp:17`): `TryGet` 10 bytes. If they are `"POWERLEVEL"`, 768 palette bytes must follow (`Palette::Read`, `gfx/palette.cpp:61-72`), and the MODERNLV probe then reads 8 fresh bytes. Otherwise the 10 bytes are pre-read, and when ≥ 8 were read their first 8 are the MODERNLV probe.
    - (d) without (c), the probe is 8 fresh bytes; with 1–7 bytes pre-read there is no probe;
    - (e) on `"MODERNLV"`: `w·h·4` display bytes (minus the pre-read tail) plus `w·h` valid bytes must follow (`Get`). The animation extension after them is lenient (`TryGet`).

    Rust `assets::level::load` keeps a level whose POWERLEVEL or MODERNLV block is truncated (`rust/assets/src/level.rs:55-65`; lenient by a Step-1 decision), where C++ rejects the whole level. That rejection means a random level at NEW GAME and no preview. Levels are user-picked from e-2 on, so this now matters (D6). The shipped levels are four `OLLEVEL2` 504×350 files (176,413 bytes) and `modern_test` (legacy + MODERNLV at 176,400; 1,234,828 bytes). None has POWERLEVEL.

**Setups (C++).**

12. **SAVE SETUP AS…** (`mainMenuState.cpp:69-91`, `:287-311`) (T0).
    - It plays `MenuSelect`. Then, **only if `ItemPosition` finds the item in view**, it pushes `InputStringState(GetBasename(GetLeaf(settings_node.FullPath())), 30, item_x + value_offset_x + 2, item_y, /*filter*/ nullptr, "", false, cb)`. That x is 178 + 100 + 2 = 280.
    - `cb(accepted, result)`:
      - `!accepted || result.empty()` → `on_complete("")`;
      - else `leaf = result + ".cfg"`, and if `ShadowsSystem(user, "Setups", leaf)` → `ScheduleReplaceTop(InfoBoxState("NAME '" + leaf + "' IS RESERVED", 160, 100, /*clear*/ true, on_dismiss))`, where `on_dismiss` = `ScheduleReplaceTop(MakeSaveAsState(..., initial = result, same x, y))`;
      - else `on_complete(result)`.
    - `on_complete(r)`: if `r` is non-empty, `SaveSettings(user / "Setups" / (r + ".cfg"))`; **then always** `MenuSelect` + `settings_menu.UpdateItems`.
    - Sound counts, together with `InputStringState::Update`'s own `MenuSelect` (e-1 fact 6): accept → 2 on the close frame; cancel or empty → 2; reserved → 1 (no completion).
    - The selected item is always in view (`EnsureInView`), so from the keyboard the push always happens. It is ported verbatim anyway.
13. **`SaveSettings` / `LoadSettings`** (`gfx.cpp:1688-1697`). Each sets `settings_node` first. `LoadSettings` then replaces `gfx.settings` with a **fresh object**, even when the load fails (a half-read object, finding 13).
    - `OptionsSelectorState::OnSelected` = `LoadSettings(node)` + `settings_menu.UpdateItems` (`fileSelectorState.cpp:222-226`), with no `MoveToFirstVisible`.
    - The shown name is `GetBasename(GetLeaf(...))`: `mine`, `orbmit`, `a.b` for `a.b.cfg`.
14. **`ShadowsSystem`** (`filesystem.cpp:736-763`) calls `SystemDataRoot()` afresh. It does not consult the resolved config layers.
    - So `--config-root <copy>` still refuses names the *install's* data has, unless the two `FullPath`s are equal.
    - The C++ web build runs `--config-root /openliero` (`gameEntry.cpp:25-28`). Its `SystemDataRoot()` falls through to `SDL_GetBasePath()` (`/` on Emscripten), which has no `Setups`, so it refuses **only the reserved name**. This is source-derived; the web build cannot run here.
    - Rust today: `NativeStore::single_dir` has no system layer, so it refuses only the reserved name. `MemoryStore` refuses its system keys (`rust/scenario/src/storage.rs:264-276`, `:355-357`).
15. **The InfoBox chain** (`inputState.cpp:184-200`, `state.hpp:92-111`): `ClearKeys`, the `Fill(bmp, 0)` when clearing, `on_dismiss` (which schedules the replacement), then `return false`. The replacement wins over the pop.
    - e-1 ported and unit-tested it (e-1 fact 5), but **no G2 case has run a `clear_screen` box or a replacement**. G2e-2 `setup_save` is their first C++ gate.
16. **LOAD SETUP breaks `record_replays = false`** (T0). Both shipped setups have `recordReplays = true` (`data/Setups/liero.cfg:71`, `orbmit.cfg:71`). The dumper sets `record_replays = false` once, at boot (intervention 6, `shell_dump.cpp:647`), and `LoadSettings` then swaps in a fresh object.
    - The next `kStateGame` writes `user/Replays/<local date and time> <names>.lrp` (`controller/localController.cpp:237-270`). That is a wall-clock file name in the `file` lines, and a recording C++ runs and Rust does not.
    - Hence intervention 6′ (D2).

**Rust at `f39b5ac`.**

17. **`Shell::new_game` writes the old match's picks back unconditionally** (`ui/src/shell/mod.rs:671-674` → `Match::write_back_picks`, `playing.rs:376-382`). That is right only while the old match is attached. After LOAD SETUP, C++'s old worm settings belong to the old `Settings` object, and the new `gfx.settings` is untouched. **Rust:** gate it on `attached()` (T4).
18. **The e-1 placeholders e-2 replaces:**
    - the D6 arms (`main_menu.rs:287-288`: `SI_LEVEL | LOAD_OPTIONS | SAVE_OPTIONS => play(select)`);
    - the harness's D6 validator (`oracle-tests/tests/shell_common/mod.rs:1149-1154`);
    - `InputPurpose` with one arm (`overlay.rs:25-30`), and `InfoPurpose` with no `on_dismiss` arm (`overlay.rs:221-230`, `mod.rs` `close`);
    - `Screen` (`stack.rs:10-20`);
    - `Match::detach_for_test` (`playing.rs:237-241`);
    - the unit test that asserts the D6 arms are sound-only (`mod.rs` tests, "the e-2 pushes (D6)").
19. **`render::hud::draw_miniature` is private and takes `&LevelSim`** (`render/src/hud.rs:353`). The preview holds `LevelData`. `Bitmap::fill_rect(x, y, w, h, idx, pal)` exists (`render/src/bitmap.rs:129`).
20. **`ConfigStore` has no listing** (`storage.rs:33-49`), and `MemoryStore` has keys only, with no directories. `MenuCtx` has no store (`main_menu.rs:81-89`). `draw_stack` walks the screens immutably (`mod.rs:719-723`), but the level selector's draw mutates it: `previewNode_`, `prev_*`, and the lazy `GetMenu`.
21. **wasm.**
    - It embeds the 4 small levels as separate `include_bytes!`, keyed TC-relative (`scenario/src/assets.rs`, the wasm `try_read_asset`).
    - The browser store is `MemoryStore::with_system(EMBEDDED_SETUPS)` (`game/src/config.rs:100-102`).
    - `?level=` stores TC-relative `Levels/<stem>.lev` (`game/src/web_params.rs:103-119`).
    - The page's field is `inputmode="numeric"` (`web/index.html:167`).
    - `window.lieroSel` knows only `M<n>`/`S<n>` (`game/src/main.rs:1488-1510`).
22. **The dumper at `f39b5ac`.**
    - `TopOf` `Fail`s on any other state (`shell_dump.cpp:429`).
    - The `fs` boot check is strict: the level file must open (`:553-566`, `:675`).
    - Intervention 6 runs once (`:647`).
    - The search-gap check covers `O` only (`:713`, `:722`).
    - `DirOf(script) + "/" + fs` resolves the manifest, and manifest sources are repo-relative or absolute (`MakeFixture`, `:355-393`).
23. **The C++ web build preloads `data/{Profiles,Resources,Setups,TC}` under `/openliero`** (`CMakeLists.txt:301-304`). `data/` holds 81 `.cfg`: `tc.cfg`, the 2 setups, and 78 object configs under `TC/openliero/{weapons,nobjects,sobjects}`. LOAD SETUP's CFG filter shows all of them under `TC/openliero`. `data/` has 11 directories (`.`, `Profiles`, `Resources`, `Setups`, `TC`, `TC/openliero`, and its `Levels`, `nobjects`, `sobjects`, `sounds`, `sprites`, `weapons`).
24. **Letters in a selector search are also controls.** R, F, D and G are P1's Up, Down, Left and Right, tested (`TestControlOnce`) in `Process` **before** `OnKeys`. e-1 T0 saw `r` move the cursor up before the search re-placed it. The design's `stage` contains `g`, which is a Right: it would enter a folder row.
25. **Carried from e-1 Addendum T0 and G3:**
    - three Rights from the TC row reach `Levels`;
    - a file level hides MAP WIDTH and MAP HEIGHT and relabels REGENERATE to RELOAD LEVEL;
    - the saved `levelFile` is `<root label>/TC/openliero/Levels/x.lev` (a relative root gains `./`; no trailing separator);
    - finding 2 was reproduced in the split layout (A plays random; B, with a user copy, and C, `--config-root`, play the level);
    - **no case may play a level shorter than ~342 rows** (C++ spawning reads past `materials[]`, which is UB). The ASan/assertions build checks the corpus.

## Decisions this plan makes (the design left them open)

- **D1. How G2e-2 shows Q4, the one intended difference, without gating a mismatch.**
  1. **Gated, bit-exact.** Every G2e-2 case that *plays* a picked level has that level in **both** layers: `file user TC/openliero/Levels/water_stage.lev` next to the system copy.
     - C++ opens it from `./user/…`, which is the finding-2 workaround e-1 T0 run B proved. Rust reads it through the merged view.
     - The merged listing de-duplicates it, so every `f`, `d` and `file` line is the real C++ output, and Rust must match it.
  2. **Gated, Rust-only, against the same C++ golden: the Q4 twin.** `shell_golden.rs::the_q4_fix_plays_a_system_only_level` re-drives `level_pick` and the milestone with every `file user TC/openliero/Levels/<x>` line dropped whose `<x>` the system layer also has. That is the default-install layout, where C++ plays random. It asserts:
     - every `f` and `d` line equals **the committed C++ golden of the both-layers run**;
     - the `file` lines equal the golden's minus the dropped copies;
     - the harness's Q4 validator fired at least once.

     So Rust in the buggy layout plays exactly what C++ plays when it *can* open the file. No golden records a divergence, and no divergence is compared as a mismatch.
  3. **Documented, not gated: the C++ side.** T0 probe P6 runs the real dumper in the sys-only layout and records that C++'s match state hashes equal those of a `[RANDOM]` pick with the same seeds. The result goes in the addendum, PROGRESS and the cpp-map.
  4. **Self-policing.** The dumper refuses an `fs` case whose boot level or NEW GAME level falls into the bug (D3). The Rust generator refuses the same case (T5), so a divergence can never sneak into a gated case.

  *Why not a `--config-root` (single-dir) fixture?* It would need a third dumper mode. It would also lose `ShadowsSystem`'s system layer, which `setup_save` needs for the `orbmit` refusal. And it would gate nothing about the level path that the both-layers copy does not already gate. *Why not commit a C++ golden of the buggy run?* It would be a golden Rust must *not* match, which muddles the golden audit and the "every golden is truth" rule.
- **D2. Intervention 6′** (fact 16).
  - After **every** `RunOneFrame`, before the `d` line: `gfx.settings->record_replays = false`. The Rust harness mirrors it after every `Shell::frame`, before its `d` line.
  - For the 22 existing cases it is a no-op, because nothing swaps the settings. T2's regeneration proof shows it.
  - It reaches `cfg16`, `mine.cfg` and the exit save on both sides identically, and the live game is unaffected.
- **D3. The Q4 guard replaces the strict `fs` level check** (fact 22).
  - For an `fs` case, at boot and after every frame that made a new controller: when `!random_level`, let `p = level_file` (+ `.LEV` without a dot).
    - Pass when `FsNode(p)` opens.
    - Otherwise, when `p` starts with `GetConfigNode().FullPath() + "/"` and the merged node (the config node `/` each remaining part) `Exists()`, `Fail("frame N: Q4 — C++ plays random where Rust plays <p>; copy the level into user/ (plan D1)")`.
    - Otherwise pass: both sides fall back to random, as in `level_missing`.
  - Non-`fs` cases keep intervention 8's strict check unchanged.
- **D4. Selector continuations.** A selector's `update` returns `SelectorOut { keep: bool, picked: Option<Picked> }`, with `Picked::{Random, Level(String /* full_path */), Setup { rel: String, name: String }}`.
  - `Shell::frame` runs the `OnSelected` continuation after the update and **before** `finish_update`, as it does for an overlay's close. C++ `OnSelected` is the last thing its `Update` does (fact 9).
- **D5. The tree is an arena, filled lazily through the store.**
  - `FileSelector { nodes: Vec<FileNode>, root: 0, current: usize }` and `FileNode { name, rel, full_path, folder, children: Vec<usize>, parent: Option<usize>, selected_child: Option<usize>, menu: Option<Menu>, filled }`.
  - `rel` is the store path (`""` for the root) and `full_path` the C++ string, from `JoinPath` over `root_label()`.
  - Lazy, like C++: a real user folder may hold thousands of replays or a symlink loop.
  - `MenuCtx` gains `store: &dyn ConfigStore`. `draw_stack` gets the top screen mutably (split borrows of `stack`, `world` and `store`).
- **D6. `ui::shell::level_path::cpp_accepts(bytes, load_powerlevel_palette) -> bool`** is fact 11's rules as a pure function over the bytes.
  - `read_level` and the preview parse only what it accepts, with `assets::level::load` doing the parse. `assets` is not changed.
  - Every shipped level passes it, so every existing golden is unaffected.
  - G2e-2 `level_missing` gates a truncated-MODERNLV file.
- **D7. The SAVE SETUP AS… name rules.**
  - A name the store cannot place gets the same `Reserved` box, a Rust-only refusal (design §4.8, §12.11). That is any byte `< 0x20` or `0x7f`, `/`, `\` or `:`. C++ would create sub-directories or escape the root.
  - The rule is `scenario::storage::placeable_leaf(leaf)`, which also requires `under()` to accept `Setups/<leaf>`.
  - A write error goes to `FrameOut::notes`. The name is left unchanged, and the `MenuSelect` and `UpdateItems` tail still runs, as C++'s completion does.
  - Bytes ≥ 0x80 become CP437 → Unicode in the file name (design §4.8). This is outside the gate.
- **D8. LOAD SETUP of a file that does not parse** keeps the current settings **and** `setup_name`, and adds a note. C++ swaps in a half-read object and crashes at the next NEW GAME (finding 13). This is Rust-only and unit-tested, and no G2 case does it.
- **D9. The browser tree is the C++ web build's** (facts 14, 21, 23).
  - `MemoryStore::single_layer(files).with_dirs(EMBEDDED_DIRS)`, root label `/openliero`. `shadows_system` returns only the reserved name, like the web build's `--config-root /openliero`.
  - The files are `browser_system_files()`: the 2 setups, the 4 small levels (`EMBEDDED_LEVELS`), `tc.cfg`, and every `weapons/`, `nobjects/`, `sobjects/` `.cfg`, all keyed as config paths. The directories are the 11 of `data/`.
  - The level selector and LOAD SETUP therefore show the same rows as the C++ web build, minus `modern_test` (Q6).
  - A native test pins the set against `data/`; the Chromium walk checks the wasm build.
- **D10. `--config-root` keeps C++'s shadow check** (fact 14).
  - `NativeStore::with_shadow_root(Option<PathBuf>)`. The split store's shadow root is its system root, as today.
  - `game::config` gives the single-dir store the `SystemDataRoot()` resolution: `OPENLIERO_DATADIR` if it is a directory, else `DATA_ROOT` if it is one, else none. It refuses nothing extra when `fs_node_full_path(root) == fs_node_full_path(shadow)`.
- **D11. Per-box keyboard on phones** (Q5).
  - `Shell::text_mode() -> Option<TextMode>` returns `Numeric` when the top `InputString` has a filter (number entry) and `Text` otherwise (SAVE SETUP AS…).
  - The glue publishes `window.lieroTextMode = "numeric" | "text"` together with `lieroPhase`. The page sets `entry.inputMode` from it before each `focus()`.
  - More read-only hooks for the walk:
    - `window.lieroSel = "L<n>" | "P<n>"` for the selector's cursor;
    - `window.lieroFolder` = the current folder's `full_path`;
    - `window.lieroSetup` = `setup_name`;
    - `window.lieroLevel` = `settings.level_file`.
- **D12. Touch in the selectors needs no new mapping.**
  - Pad Up and Down are P1's controls, which `Process` tests, and they auto-repeat in phase `menu` (e-1).
  - Pad Left and Right are once-keys: the parent folder, and into a folder.
  - FIRE picks, and JUMP or MENU leave.
  - Type-to-search is not reachable on a phone, because no keyboard is up outside a text box. That is recorded, not built.
- **D13. `.zip` entries** are listed as folders named without `.zip` (fact 5), so rows match C++. Their contents list as empty in Rust (zip reading is unported, design §5). No gated case holds one.
- **D14. File names** go through `to_string_lossy`. Among entries with equal byte names, the user layer's is kept. Equal-`CiLess` names keep the byte order (a stable sort). This is recorded, and no gated case depends on it.
- **D15. The `rust/settings` crate move** (e-1 D12) stays out of e-2. It is a pure refactor with no C++ surface, best done when 4½h reworks storage. It is recorded as open.
- **D16. No new script directive.** e-2 needs only `key`, `text`, `detail` and `fs`. The case inputs are:
  - generator-written files in the golden dir: two tiny levels and two user setups;
  - repo files under new names, e.g. `data/README.md` as `broken.lev` and `notes.txt`, and `water_stage.lev` as `Zeta.lev`.
- **D17. `ShellDebug::load_detach`** (default `true`) is T5's counterfactual switch. `false` skips LOAD SETUP's detach, so RESUME resyncs the paused match to the loaded setup.

## Standing ruling (4½c Addendum A, carried forward)

The C++ comparison happens **here**, against the real C++ run headlessly. If a dumper cannot run the real code headlessly, the task **stops and reports the exact blocker**. It never weakens a gate and never regenerates a golden to paper over a mismatch. The real `openliero` under Xvfb produces C++ | Rust side-by-side PNGs. Those are eyeball artefacts, never gates, and are never committed.

## Global constraints

- **LD 1:** pixel-exact menus on the 320×200 CPU surface.
- **LD 3, as amended in e-1.** `tick_and_render` stays the only `ResMut<Sim>` holder, and menus get no `SimState`. LOAD SETUP only flips `Match::attached`. It never touches the sim.
- **LD 4.** No grammar change in e-2, for scenarios or for shell scripts (D16).
- **LD 5.** `SimState::new`'s signature is unchanged.
- **Crate rules.**
  - `sim-core` stays dependency-free, and `sim` gains no dependency.
  - `ui`, `render`, `scenario` and `shot` stay Bevy-free.
  - `ui` adds no external dependency: `cargo tree -p ui -e normal | grep -c bevy` prints `0`.
- **Determinism.** Use no floats, no wall clock, and no `HashMap`/`HashSet` iteration in `ui`, `render` or `scenario::storage::list`. Use `BTreeMap` and sorted `Vec`s. The only clocks are the caller's `ShellInput::{fresh_seed, now_ms}`.
- **`sim`, `assets` and `sim-core` do not change**, except for a bug a G2e-2 case *proves*. Such a fix gets its own unit test and its own commit, and the full re-diff must pass.
- **`render` changes only by adding `hud::draw_miniature_ids`.** The existing private function delegates to it, and every render golden is unchanged.
- **C++ changes are confined to** `src/tools/oracle_dump/shell_dump.cpp`. `CMakeLists.txt` and `sim_physics_dump.cpp` are unchanged.
- **Frozen provenance.** Never edit these:
  - `examples/gen_slice4_5a.rs`, `gen_slice4_5c0.rs`, `gen_slice4_5c.rs`, `gen_slice4_5d.rs`, `gen_slice4_5e1_shell.rs`, `gen_slice4_5e1_sim.rs`;
  - `tests/shell_e1_cases/`, `tests/weapsel_common/`, `tests/sim_slice4_5c0_common/`;
  - `menu_dump.cpp`, `weapsel_dump.cpp`, `weapsel_drive.hpp`, `sim_physics_dump.cpp`, `gen_menu_golden.sh`, `gen_sim_slice4_5e_golden.sh`;
  - every `golden/*` that exists at `f39b5ac`.

  `tests/shell_common/mod.rs` changes. So `gen_slice4_5d -- check` and `gen_slice4_5e1_shell -- check` must stay clean, and all 22 committed scripts must still equal their generators (`the_committed_scripts_are_the_generators`).
- **Golden audit.** `git diff --name-status f39b5ac -- rust/oracle-tests/golden` may list **only `A` lines**, exactly the **22 files** in §File structure: 6 scripts, 6 goldens, 6 `fs` manifests and 4 generator-written inputs.
- **rustfmt.** Run it only on files a task *creates*: `rustfmt --edition 2024 <abs file>` for `ui`/`game`, `--edition 2021` elsewhere. Never run it on an existing file or on a `lib.rs`/`main.rs`, because it recurses. Hand-format edits to match the surrounding style.
- **The non-shell paths stay behaviour-identical:** `--live [<scenario>]`, `--live --record`, `--replay`, `Scripted`, `?demo`. `game/tests/round_trip.rs` and `record_regression.rs` stay green and untouched.

## Formats pinned (both sides implement exactly this)

**Tops.** `f`-line `upd` ∈ `M W G O I B L P`; `top` ∈ `M G O I B L P -`. `L` is `LevelSelectorState` and `P` is `OptionsSelectorState` (Rust `Screen::LevelSelect` / `Screen::SetupSelect`). `ReplaySelectorState`, `ProfileSelectorState` and `TcSelectorState` still `Fail`: e-2 cannot reach them. The `boot`, `f`, `d`, `end` and `file` line shapes are **unchanged** from the e-1 plan's §Formats.

**Interventions.**
- **6′** (D2): after every `RunOneFrame`, `gfx.settings->record_replays = false`.
- **8, `fs` form** (D3): the Q4 guard at boot and after every frame that made a controller.
- The header comment documents both. The `out` header string is byte-identical to e-1's for every script, because no new `#` header line is added.

**The search-gap check** (e-1 D4) covers one continuous visit of an `O`, `L` or `P` state, keyed by the state pointer. `B` interludes continue an `O` visit only, since the selectors push nothing.

**`gen_shell_golden.sh`:** `upd` matches `^[MWGOIBLP]$`, `top` matches `^[MGOIBLP-]$`, and `EXPECTED_SHELL_CASES` defaults to **28**. Before T5 commits the e-2 scripts, T2's gate runs it with `EXPECTED_SHELL_CASES=22`.

**`fs` manifests** keep e-1's grammar (`dir <user|sys> <rel>`, `file <user|sys> <rel> <source>`). The e-2 generator writes every manifest from one builder, `Fs::install()`: the system layer of a real install, restricted to what the two filters can show:
```
dir  sys Profiles
dir  sys Resources
file sys Setups/liero.cfg                     data/Setups/liero.cfg
file sys Setups/orbmit.cfg                    data/Setups/orbmit.cfg
file sys TC/openliero/tc.cfg                  data/TC/openliero/tc.cfg
file sys TC/openliero/Levels/<each of the 5>  data/TC/openliero/Levels/<same>
dir  sys TC/openliero/{nobjects,sobjects,sounds,sprites,weapons}
```
Each case then adds its user lines. A generator test asserts that every `data/` file left out is filtered by both `LEV` and `CFG`, or lies under a `TC/openliero/{weapons,nobjects,sobjects}` folder no case enters with the `CFG` filter. The validator refuses any P-top entry into those folders.

## File structure

| File | Task | Responsibility |
|---|---|---|
| `rust/render/src/hud.rs` (modify) | T1 | `pub fn draw_miniature_ids(scr, pal, ids, width, height, map_x, map_y, step_x, step_y)`; the private one delegates |
| `rust/scenario/src/storage.rs` (modify) | T1 | `DirEntry`, `ConfigStore::list`; `NativeStore::list` + `with_shadow_root`; `MemoryStore` dirs, `with_dirs`, `single_layer`; `placeable_leaf` |
| `rust/scenario/src/assets.rs` (modify) | T1 | `EMBEDDED_LEVELS` (4, config-root keys; the wasm `try_read_asset` reads them), `EMBEDDED_DIRS`, `browser_system_files()` |
| `rust/ui/src/text.rs` (modify) | T3 | `UiTc::{random, sel_level}`; `ci_less`, `ci_compare`, `ci_starts_with`, `join_path`, `get_basename`, `get_extension` |
| `rust/ui/src/shell/files.rs` (create) | T3 | `FileNode`, `FileSelector`, `LevelSelectorState`, `SetupSelectorState`, `Picked`, `SelectorOut` |
| `rust/ui/src/shell/level_path.rs` (modify) | T3 | `cpp_accepts`; `read_level` gains `load_powerlevel_palette` |
| `rust/ui/src/shell/{mod.rs, stack.rs, main_menu.rs, level_slot.rs}` (modify) | T3 | the `Screen` variants, tops `L`/`P`, `MenuCtx.store`, mutable draw, the `Picked` continuation hook, `selector_view()` |
| `rust/ui/src/shell/{main_menu.rs, overlay.rs, mod.rs, playing.rs}` (modify) | T4 | the live Enter arms, the Save-As chain, the LOAD SETUP continuation, `detach`, the write-back gate, `FrameOut::notes`, `text_mode()`, `setup_name()`, `level_from_file()`, `ShellDebug::load_detach` |
| `src/tools/oracle_dump/shell_dump.cpp` (modify) | T2 | tops `L`/`P`, the gap check for them, intervention 6′, the Q4 guard, the header |
| `rust/oracle-tests/gen_shell_golden.sh` (modify) | T2 | the tops and the count 28 |
| `rust/oracle-tests/tests/shell_common/mod.rs` (modify) | T5 | 6′'s mirror, the Q4 validator, the L/P validators, `type_chars`, `Fs::install`, the twin option, ledger fields; the D6 validator dropped |
| `rust/oracle-tests/tests/shell_e2_cases/mod.rs` (create) | T5 | the 6 cases, their inputs, validators, witnesses |
| `rust/oracle-tests/examples/gen_slice4_5e2_shell.rs` (create) | T5 | `check` / `write` |
| `rust/oracle-tests/golden/shell_*` e-2 (create, **22**) | T5 | 6 `_script.txt` + 6 `.txt` + 6 `_fs.txt`; `shell_level_tree_tiny.lev`, `shell_level_missing_trunc.lev`, `shell_level_missing_user_liero.cfg`, `shell_setup_load_user_mine.cfg` |
| `rust/oracle-tests/tests/shell_golden.rs` (modify) | T6 | the e-2 cases, the milestone, the Q4 twin, the union list |
| `rust/game/src/{config.rs, main.rs, web_params.rs, touch.rs}` (modify) | T7 | the browser store, the shadow root, canonical `?level=`, the text mode and the hooks, notes → warn |
| `web/index.html`, `.github/workflows/preview.yml` (modify) | T7 | per-box `inputmode`, hints, help text |
| PROGRESS, overview, design status, cpp-map, rust-map, `.claude/skills/liero-shot/SKILL.md` | T8 | status + corrections |

## Batches, order and parallelism

| Batch | Tasks | Needs | Runs alongside | Touches | Gate |
|---|---|---|---|---|---|
| **1** | T0 probes | — | — (first, alone) | `shell_dump.cpp` temporarily (restored), scratchpad, this plan (addendum) | every probe recorded; the contradiction rules applied |
| **2** | T1 | 1 | **3** | `rust/render/src/hud.rs`, `rust/scenario/src/{storage,assets}.rs` | the full re-diff (both commands); the wasm check; golden status empty |
| **3** | T2 | 1 | **2, 4, 5** (C++ only) | `shell_dump.cpp`, `gen_shell_golden.sh` | builds; clang-format 22 (diff + whole file) + clang-tidy; the 22 shell goldens regenerate byte-identically (release and ASan builds); the scratch smokes |
| **4** | T3 | 2 | 3 | `rust/ui` (+ mechanical callers of `read_level`) | the full re-diff; the wasm check; G1 + 4½d/e-1 G2 green |
| **5** | T4 | 4 | 3 | `rust/ui` (+ callers if a signature moves) | the full re-diff; the wasm check; G1 + all 22 G2 cases green |
| **6** | T5, T6 | 3, 5 | — | `rust/oracle-tests` (shell); `rust/ui` / `rust/render` (proven fixes only, no pub-API change `game` uses) | **🎯 G2e-2 bit-exact** (every `f`/`d`/`file` line) + the Q4 twin; the full re-diff |
| **7** | T7 | 5 | — | `rust/game`, `web/index.html`, `.github/workflows/preview.yml` | `cargo test -p game`; the wasm check; the bundle + Chromium walk |
| **8** | T8 | 6, 7 | — (last) | docs, skill | the full board + every audit + the broad review |

**Schedule:** 1 → (2 ∥ 3) → 4 → 5 → 6 → 7 → 8. Batch 3 is C++-only and may keep running through Batches 4 and 5, but it must be done before 6. Rust batches never overlap: disk and one checkout. Batch 7 needs only T4's API, so it may go before 6 if 6 is blocked in its fix loop. Record the reorder.

**Design ↔ plan mapping (design §8, 4½e-2):**

| Design | This plan |
|---|---|
| T0 (`list`, `root_label`, the wasm system layer, `level_path`, `Option<LevelData>`) | T1; `root_label`, `level_path` and `Option<LevelData>` landed in e-1 (e-1 fact 27); probes in T0 |
| T1 (files, selectors, Save-As chain) | T3 + T4 |
| T2 (C++ fixture listings, `L`/`P`) | T2 |
| T3 (G2e-2 corpus) | T5 |
| T4 (🎯 milestone) | T6 |
| T5 (`game`: wasm catalogue, `?level=`, touch) | T7 |
| T6 (Xvfb, docs, review) | T8 |

---

### Task 0 (Batch 1): the C++ probes

Everything the plan read from the source for e-2 is probed with the **real** `Gfx::RunOneFrame`. The tool is a temporary, never-committed patch of `oracle_dump_shell`, driven through `fs` fixtures and `--ppm-dir`. One optional probe (P9) uses the real `openliero` under Xvfb. No probe adds code to the repo.

**Files:** `src/tools/oracle_dump/shell_dump.cpp` (patched temporarily, restored at the end), everything under `$S/t0e2x/`, this plan (the addendum).

- [ ] **Step 1: patch the dumper, in the working tree only.** Save the diff as `$S/t0e2x/probe_patch.diff`.
  - `TopOf` returns `L` for `LevelSelectorState` and `P` for `OptionsSelectorState`. Include `fileSelectorState.hpp`.
  - Both `CheckLevelFile` calls are skipped for `fs` cases, so P6 and P10 can reach the fallback.
  - Intervention 6 stays as it is (once, at boot), so P7 can see the replay file.
  - Build: `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell`.
- [ ] **Step 2: the fixture inputs**, all in `$S/t0e2x/` and made with a short Python script `$S/t0e2x/mk.py`:
  - `tiny.lev`: `OLLEVEL2`, version 1, w = 60, h = 40 little-endian, then 2,400 material bytes `(x + y) % 64 + 160`;
  - `trunc.lev`: `OLLEVEL2`, 1, 8×8, 64 bytes of `0xA0`, `MODERNLV`, and 10 bytes of `0`.

  Manifest sources may be absolute (fact 22). Every manifest starts with the §Formats `Fs::install()` lines, written out by hand.
- [ ] **Step 3: the scripts** in `$S/t0e2x/`. All use `setup default`, `detail` and `fs <case>_fs.txt`. Seeds are free, but equal between the scripts that are compared. Key counts follow fact 25 and T0's listings. Adjust them if a listing differs, and record that.

  | Probe | Script(s) | User layer beyond `install` | Path |
  |---|---|---|---|
  | **P1** listing, filter, sort, parent pane | `t_tree` | `dir user Replays`; `TC/openliero/Levels/{Zeta.lev ← water_stage.lev, alpha.LEV ← water_stage.lev, .hidden.lev ← render_stage.lev, notes.txt ← data/README.md, tiny.lev}` | F7, Down ×2 (LEVEL), Return; snap the root; Down to Profiles, Right (empty), Left; Down to TC, Right ×3; Down one row at a time to the end, 3 frames apart; PgUp; Left (parent pane: `openliero`); Right; Esc (leave); idle 10; Esc (focus main); Esc (cursor → QUIT); Return |
  | **P2** cursor restore | `t_restore` | `TC/openliero/Levels/water_stage.lev` (a user copy) | as P1 to Levels, cursor to `water_stage`, Return; then Return on LEVEL again; snap the first `L` frame |
  | **P3** the late preview | (from `t_tree`'s PPMs) | — | — |
  | **P4** LOAD SETUP | `o_open` | `Setups/mine.cfg` (a copy of `data/Setups/orbmit.cfg`) | F7, Down to LOAD SETUP, Return; snap; Left (root), Right; Return on `orbmit`; snap the settings |
  | **P5** Save-As chain | `s_save` | — | F7, Down to SAVE SETUP AS…, Return (`I` on `liero`), Return (`B`), SPACE, BACKSPACE ×5, `m i n e` (key + `text` per char), Return; Return (`I` on `mine`), BACKSPACE ×4, `o r b m i t`, Return (`B`), SPACE, ESC (cancel); Esc; QUIT |
  | **P6** Q4 in C++ | `q_sys`, `q_rand`, `q_both` | `q_both`: the user copy of `water_stage`; `q_sys` and `q_rand`: none | `q_sys`/`q_both`: pick `water_stage`; `q_rand`: pick `[RANDOM]`; then Esc, NEW GAME, the P1/P2 DONE, 200 match frames, Esc, QUIT |
  | **P7** replays after LOAD SETUP | `r_rec` | — | LOAD SETUP → `orbmit`; Esc; NEW GAME; DONE; 60 frames; Esc; QUIT |
  | **P8** LOAD SETUP detaches | `l_ctl`, `l_load` | — | NEW GAME, DONE, 200 frames, Esc; then F7 + idle (`l_ctl`) or F7 + LOAD SETUP → `orbmit` (`l_load`), padded to equal frames; Esc; F1 (RESUME); 200 frames; Esc; QUIT |
  | **P10** rejected files | `b_bad` | `TC/openliero/Levels/{broken.lev ← data/README.md, trunc.lev}` | to Levels; cursor onto `broken`, then `trunc` (no preview); Return on `trunc`; NEW GAME; 100 frames; Esc; QUIT |

  Run each script with `build/linux-x64/Release/oracle_dump_shell $S/t0e2x/<s>.txt $S/t0e2x/<s>.out --ppm-dir $S/t0e2x/ppm_<s>`, and convert the frames you look at with `$S/ppm2png.py`. `$S/t0e2x/region.py` hashes the preview rectangle (134..186 × 162..198) of each PPM, so P3 is checked by numbers, not by eye.
- [ ] **Step 4: the expected outcomes** (from facts 1–16). Record each as **confirmed**, or with what differed.
  - **P1.** The root lists `[RANDOM]`, Profiles, Replays, Resources, Setups, TC, and the title reads `Select level: ./user`.
    - Profiles is an empty menu with the parent pane (the root).
    - `TC/openliero` lists Levels, nobjects, sobjects, sounds, sprites, weapons.
    - Levels lists `.hidden`, `alpha`, `modern_test`, `physics_fall_test`, `render_stage`, `see_shadow_test`, `tiny`, `water_stage`, `Zeta`. `notes.txt` is absent, the file rows are colour 48 and the folder rows colour 47.
    - The title reads `Select level: ./user/TC/openliero/Levels`.
  - **P3.** On the frame whose Down lands on a new file, the preview rectangle equals the previous frame's. On the next frame it changes.
    - The first preview clears the full 52×36.
    - Going from a 504×350 level (footprint 50×35) onto `tiny` (step 2×2, footprint 30×20) leaves the 30..50 × 20..35 band black. `alpha`/`Zeta` (both water) repaint only the rectangle.
    - `[RANDOM]` and folders repaint nothing.
    - After Esc, the main menu's frames still show the last preview.
  - **P2.** The first `L` frame of the reopen is inside Levels with the cursor on `water_stage`, and the parent pane shows `openliero` with the cursor on Levels. The frame after it shows the `water_stage` preview (the late preview again).
  - **P4.** The first `P` frame is inside Setups, with rows `liero`, `mine`, `orbmit`, the cursor on `liero`, and the title `Select options: ./user/Setups`.
    - Left lists Profiles, Resources, Setups, TC with the cursor on Setups.
    - After `orbmit`, the `d` line's `cfg16` changes, and the settings menu shows lives 9, loading 20 %, max bonuses 0 and blood 25 %. SAVE SETUP AS… reads `orbmit`.
  - **P5.**
    - The first `B` frame is black with the box, drawn through `exepal`, and reads `NAME 'liero.cfg' IS RESERVED`.
    - SPACE brings back `I` on `liero`.
    - The `mine` Return frame's sounds hold **two** `MenuSelect`s, and SAVE SETUP AS… then reads `mine`.
    - `orbmit` gives the box. ESC plays two `MenuSelect`s and saves nothing.
    - The `file` lines are exactly `Setups/liero.cfg` (the exit save) and `Setups/mine.cfg`.
  - **P6.** `q_sys`'s match `state8` sequence **equals `q_rand`'s** and differs from `q_both`'s: C++ played random, which is finding 2 in the dumper. The selection screen of `q_sys` still reads `Level: "water_stage"`.
  - **P7.** A `file Replays/<date> <time> <names>.lrp …` line appears, which proves intervention 6′ (D2) is needed.
  - **P8.** `l_load`'s `state8` after RESUME **equals `l_ctl`'s** on every resumed frame, although `orbmit` has `maxBonuses = 0`: the paused game kept the old settings (finding 1's LOAD SETUP half). Its `cfg16` differs from `l_ctl`'s from the load frame on.
  - **P10.** Neither `broken` nor `trunc` changes the preview rectangle. The `trunc` NEW GAME's `state8` equals that of a `[RANDOM]` NEW GAME with the same seeds; re-run `q_rand` with P10's seeds if they differ.
- [ ] **Step 5 (optional; skip if Xvfb flakes): P9, the `--config-root` shadow check** (fact 14), with the real game.
  - `cp -r data $S/t0e2x/cr/root`. Run `OPENLIERO_DATADIR=/home/user/openliero/data … openliero --config-root $S/t0e2x/cr/root`, then F7 → SAVE SETUP AS… → Backspace ×5 → `xdotool type orbmit` → Return.
  - **Expected:** the `RESERVED` box, from `SystemDataRoot()`.
  - Repeat with `OPENLIERO_DATADIR` unset. The build's compiled-in data dir must not exist; check `grep OPENLIERO_DATADIR build/linux-x64/CMakeCache.txt`. **Expected:** saved, with `root/Setups/orbmit.cfg` overwritten.
- [ ] **Step 6: restore.** `git -C /home/user/openliero checkout -- src/tools/oracle_dump/shell_dump.cpp`, then rebuild `oracle_dump_shell`. `git status --short src` must be empty, and the restored binary must regenerate `shell_cfg_boot.txt` byte-identically (`cmp`).
- [ ] **Step 7: record and commit.** Append **"## Addendum T0 (probe results)"** to this plan. It holds:
  - the command lines;
  - each probe's verdict, with the frame numbers and field values that show it;
  - the listings as observed;
  - P6's and P8's equal and unequal `state8` counts;
  - P7's file line;
  - P9's outcome, or "skipped".

  The PPMs and PNGs stay in `$S/t0e2x`. Then `git add docs/superpowers/plans/2026-09-26-liero-rs-step4.5-slice4.5e2-plan.md` and commit `docs(4.5e-2): T0 — C++ probes of the selectors, setups and the shipped-level bug (results in the plan)`.

**If a probe contradicts the plan** (the source wins; these are findings, not product choices):
- **P1 or P2 differ** (order, filter, dotfiles, colours, titles, the parent pane, restore): the observed behaviour is the spec. Rewrite the facts it touches in the addendum. T3's unit tests assert the observed form, and the G2 cases will gate it.
- **P3 differs** (the preview is not late, does not persist, or clears differently): port what was observed. If the preview lands the same frame, it is drawn onto the surface too. Fix fact 10 and design finding 5 in the addendum.
- **P4 or P5 differ** (where LOAD SETUP opens, the box, the reopen, the sound counts, the saved files): port what was observed, and rewrite facts 12, 13 and 15.
- **P6 contradicted** (`q_sys` plays `water_stage`): Q4 is moot in the dumper layout. D3's guard is then dropped, the twin (D1.2) stays as a Rust-only regression test, and PROGRESS records it.
- **P7 contradicted** (no replay file): drop intervention 6′ (D2) and its mirror. No intervention exists without a need.
- **P8 contradicted** (`l_load` diverges): LOAD SETUP does not detach. T4 keeps the match attached, D17's switch and the `setup_load` counterfactual are dropped, and design finding 1 is rewritten.
- **P10 contradicted** (C++ loads `trunc`, or previews `broken`): `cpp_accepts` follows the observation (D6). If C++ *plays* an 8×8 level, `level_missing` must not NEW GAME it (e-1 Addendum G3's UB).
- **P9 contradicted:** D10 follows the observation.
- **A probe cannot run** (the patch fails to build, or the dumper crashes on a selector): **stop and report**, because T3 and T4 depend on P1–P5 and P8. P6, P7, P9 and P10 may be recorded as "unconfirmed", and the plan proceeds with the source-derived facts.

**Done when:** the addendum records each probe, the contradiction rules have been applied, `src/` is clean, and the addendum commit exists.

---

### Task 1 (Batch 2): `render` and `scenario`, listing, the browser tree, the shadow root (hash-neutral)

**Files:** `rust/render/src/hud.rs`, `rust/scenario/src/storage.rs`, `rust/scenario/src/assets.rs`.

- [ ] **Step 1: `render::hud::draw_miniature_ids(scr, pal, ids: &[u8], width, height, map_x, map_y, step_x, step_y)`**. This is `level.cpp:489-507` over raw ids, with the unsigned index cast and the bound check before the clip test, exactly as today. The private `draw_miniature(…, &LevelSim, …)` delegates to it. **Test:** a 60×40 id grid at step 2×2 writes 30×20 cells at (134, 162), and no pixel outside that footprint changes. The render and HUD goldens stay unchanged in the re-diff.
- [ ] **Step 2: `pub struct DirEntry { pub name: String, pub is_dir: bool }` and `ConfigStore::list(&self, rel: &str) -> Vec<DirEntry>`** (facts 5 and 6). `rel = ""` is the root. The result is sorted bytewise by name and de-duplicated by name, keeping the user layer's entry (D14).
  - **`NativeStore::list`:**
    - `std::fs::read_dir` on each layer, the user layer first. The root is `""`, and anything else must pass `under()`.
    - `metadata` follows symlinks, and an entry whose metadata fails is dropped.
    - Names use `to_string_lossy`. A name ending in `.zip` becomes a directory named without it (D13).
    - A missing or unreadable directory contributes nothing.
  - **`MemoryStore::list`:** the direct children of `rel` among the user keys, the system keys and the explicit dirs. A key `a/b/c` makes `a` a directory under `""` and `b` one under `a`.
  - **Tests** (scratch dirs as in `storage.rs`'s tests):
    - merge and dedupe across the two layers;
    - byte order: `B` < `a`, and `.hidden` is listed;
    - a broken symlink is dropped;
    - `x.zip` becomes the folder `x`, but `x.ZIP` stays a file;
    - a missing dir gives `[]`;
    - `MemoryStore` directories come from keys and from `with_dirs`;
    - `list("")` on a fresh `MemoryStore` is `[]`.
- [ ] **Step 3: `MemoryStore::with_dirs(&[&str])` and `MemoryStore::single_layer(files)`** (D9). `single_layer` fills the read-only layer from `files` and sets `shadows_preloaded = false`. Then `shadows_system` answers only `is_reserved`, which is the C++ web build (fact 14). `with_system` keeps today's behaviour. **Tests:** both constructors' `shadows_system` on `Setups/liero.cfg` and `Setups/orbmit.cfg`; writes shadow reads in both.
- [ ] **Step 4: `NativeStore::with_shadow_root(Option<PathBuf>)`** (D10). `split` defaults the shadow root to the system root. `shadows_system` checks, in order: reserved; no shadow root → `false`; `fs_node_full_path` of the user root equals that of the shadow root → `false`; else `<shadow>/<subdir>/<leaf>` exists. **Tests:** `single_dir` with and without a shadow root; equal roots.
- [ ] **Step 5: `storage::placeable_leaf(leaf: &str) -> bool`** (D7): non-empty; no `/`, `\` or `:`; no byte `< 0x20` or `0x7f`; and `under(root, "Setups/<leaf>")` accepts it. **Tests:** `mine.cfg`, `a b.cfg`, `x.cfg.cfg` and `..cfg` (the name `.`: one part, which `under` accepts, and C++ writes it too) pass; `a/b.cfg`, `a\b.cfg`, `c:.cfg` and `\t.cfg` fail.
- [ ] **Step 6: `assets`.**
  - `pub static EMBEDDED_LEVELS: &[(&str, &[u8])]` is the four small levels keyed `TC/openliero/Levels/<stem>.lev`, via `include_bytes!`. It is compiled on every target, like `EMBEDDED_SETUPS`, and unused statics are dropped by the linker.
  - The wasm `try_read_asset` maps `Levels/<x>` to `EMBEDDED_LEVELS`' `TC/openliero/Levels/<x>`, so the level bytes are embedded once. `modern_test` stays out (Q6).
  - `pub const EMBEDDED_DIRS: &[&str]` holds the 11 `data/` directories (fact 23), `""` excluded.
  - `pub fn browser_system_files() -> Vec<(String, Vec<u8>)>`:
    - on wasm, `EMBEDDED_SETUPS` + `EMBEDDED_LEVELS` + `tc.cfg` + every `.cfg` of the `WEAPONS`, `NOBJECTS` and `SOBJECTS` `include_dir!` trees, keyed `TC/openliero/<dir>/<file>`;
    - natively, the same set read from `DATA_ROOT`, for tests and parity.
  - **Tests (native):**
    - `EMBEDDED_LEVELS` equals the 4 files byte for byte, and excludes `modern_test`;
    - `EMBEDDED_DIRS` equals the directory walk of `data/`;
    - `browser_system_files()` equals every `data/` file whose extension is `lev` or `cfg`, case-insensitively, minus `modern_test.lev`;
    - `game::web_params::LEVELS` equals the `EMBEDDED_LEVELS` stems. That assertion lives in `game`'s tests, in T7.
- [ ] **Step 7: re-diff** (both commands), the wasm check, and the golden status (empty). **Commit** `scenario(4.5e-2): ConfigStore::list (DirectoryListing over the merged layers), MemoryStore dirs + the single-layer web store, the --config-root shadow root, placeable_leaf, the embedded level/dir catalogue; render: draw_miniature_ids (hash-neutral)`.

**Done when:** every test above passes, both re-diff commands and the wasm check are green, and `git diff f39b5ac --stat -- rust/render` shows only `hud.rs`.

---

### Task 2 (Batch 3): `oracle_dump_shell` grows `L`/`P`, intervention 6′ and the Q4 guard

**Files:** `src/tools/oracle_dump/shell_dump.cpp`, `rust/oracle-tests/gen_shell_golden.sh`.

- [ ] **Step 1: `TopOf`.** `LevelSelectorState` → `L` and `OptionsSelectorState` → `P`, with `#include "fileSelectorState.hpp"`. Every other state still `Fail`s, with the message updated to "4½e-2". `upd` comes from the same `TopOf` at frame start.
- [ ] **Step 2: the gap check** (§Formats). A visit is one continuous stretch of the same `O`, `L` or `P` state pointer. `B` continues an `O` visit only. `IsSearchKey` is unchanged.
- [ ] **Step 3: intervention 6′** (D2). Right after `gfx.RunOneFrame()`, and before the `new controller` block and the `d` line: `gfx.settings->record_replays = false`. Keep the boot-time line, because the boot `d` state and the defaults save depend on it.
- [ ] **Step 4: the Q4 guard** (D3). `CheckLevelQ4(std::string const& where)`:
  1. Random → pass.
  2. `p = level_file` (+ `.LEV`); `FsNode(p).ToReader()` succeeds → pass.
  3. Else, if `p` starts with `GetConfigNode().FullPath() + "/"`, split the rest on `/` and walk `node = node / part` from `GetConfigNode()`. If `node.Exists()`, `Fail`.
  4. Else pass.

  Call it for `fs` cases at boot, in place of `CheckLevelFile("the fs fixture")`, and after every frame that made a controller. Non-`fs` cases keep `CheckLevelFile("data/TC/openliero")` unchanged.
- [ ] **Step 5: the header.** Add 6′ to intervention 6's entry and the Q4 guard to intervention 8's `fs` form. Add the `L`/`P` tops to the opt-in paragraph and the selectors to the gap-check sentence. The `out` header string is unchanged.
- [ ] **Step 6: `gen_shell_golden.sh`** (§Formats). Update the comment block: 4½e-2 adds `L`/`P`, and the default count is 28 (the 4½d 11 + the e-1 11 + the e-2 6).
- [ ] **Step 7: the regeneration proof.** `source $S/env.sh && EXPECTED_SHELL_CASES=22 bash rust/oracle-tests/gen_shell_golden.sh`, then `git status --porcelain rust/oracle-tests/golden` must be **empty**. Then rebuild `$S/build-chk`, and run the `b7chk` loop into `$S/e2chk/`: all 22 `SAME`.
- [ ] **Step 8: scratch smokes** (in `$S/t2/`, never committed). Reuse T0's `t_tree`, `s_save`, `o_open` and `q_sys`, with T0's fixture inputs:
  1. `t_tree`, `s_save` and `o_open` run to `end` with tops `L`/`P`/`I`/`B` in their `f` lines, and the `awk` gate of `gen_shell_golden.sh` passes on each output (run the awk block by hand).
  2. `q_sys` **fails** with the Q4 message on its NEW GAME frame, and `q_both` passes.
  3. `r_rec` passes, and its `file` lines hold no `Replays/` line.
  4. A search with two keys more than 1 s apart in one `L` visit fails the gap check. Use `sleep`-free padding: 80 idle frames at the dumper's speed; measure it, and pad until it fails.
- [ ] **Step 9: format and tidy.**
  - `"$CF22" --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/shell_dump.cpp` prints nothing.
  - `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh f39b5ac` is clean.
  - `scripts/clang-tidy-diff.sh build/linux-x64 f39b5ac` exits 0.
- [ ] **Step 10: commit** `oracle(4.5e-2): oracle_dump_shell — the L/P selector tops, the search-gap check for them, intervention 6′ (record_replays after every frame), the Q4 guard for fs cases`.

**Done when:** it builds, format and tidy are clean, the 22 goldens are byte-identical under both builds, and the four smokes behave as described. Record their outputs in the done-report.

**Batch 3 gate:** `git diff --name-only f39b5ac -- src CMakeLists.txt` lists exactly `src/tools/oracle_dump/shell_dump.cpp`.

---

### Task 3 (Batch 4): `ui`, the file tree, the two selectors, the screens

**Files:**
- `rust/ui/src/text.rs`;
- new `rust/ui/src/shell/files.rs`;
- `rust/ui/src/shell/{level_path.rs, level_slot.rs, stack.rs, mod.rs, main_menu.rs}`;
- mechanical callers of `read_level`.

- [ ] **Step 1: `ui::text`.**
  - `UiTc` gains `random: String` (`LS(Random)`, `"[RANDOM]"`) and `sel_level: String` (`"Select level:"`).
  - Add the C++ string helpers: `ci_compare`, `ci_starts_with` and `ci_less` (ASCII `to_ascii_uppercase` per byte, fact 4), `join_path` (`filesystem.cpp:283-288`), `get_basename` and `get_extension` (the last `.`).
  - **Tests:**
    - `ci_less("a", "B")`, `ci_less("ab", "a") == false`, `ci_less("", "a")`;
    - `_` (0x5F) sorts after `Z`;
    - `join_path("./user", "TC") == "./user/TC"` and `join_path("/", "x") == "/x"`;
    - `get_basename("a.b.lev") == "a.b"` and `get_extension("noext") == ""`;
    - the two new `UiTc` texts.
- [ ] **Step 2: `level_path`** (D6).
  - `cpp_accepts(bytes: &[u8], load_powerlevel_palette: bool) -> bool` is fact 11, clause by clause.
  - `read_level(store, tc_root, level_file, load_powerlevel_palette)` returns `Some` only when the bytes are accepted and `assets::level::load` parses them.
  - `LevelSlot::generate` passes `settings.load_powerlevel_palette`.
  - **Tests** (synthetic bytes):
    - legacy exact; legacy one byte short;
    - `OLLEVEL2` with w = 0 or 4097; the header truncated;
    - POWERLEVEL + 768 bytes accepted; POWERLEVEL + 767 bytes rejected **only** when the flag is on;
    - MODERNLV complete, and short by one display byte;
    - a lone `MODERNLV` magic at the end;
    - 1–7 trailing bytes;
    - the anim extension truncated, which is accepted;
    - every shipped level accepted with the flag on and off;
    - the 10-byte `README`-as-level rejected.
- [ ] **Step 3: `files::FileSelector`** (facts 3–9; D5). It is the arena, with `fill(store, idx, filter)` (fact 4 over `store.list(rel)`), `get_menu(store, idx)` (lazy build, fact 3), `find`, `select`, `cur_sel`, `enter`, `exit`, `process` and `draw`.
  - `filter` is `fn(&str /*name*/, &str /*ext*/) -> bool`: `LEV` or `CFG`, compared with `ci_compare`.
  - `select` builds each ancestor menu through `get_menu`, then `move_to(i)`, the fact-8 order.
  - `process(cx) -> bool` runs fact 9's steps 1–5, in order, with `now_ms` for `on_keys(…, contains = true)`.
  - **Tests** (a `MemoryStore` tree with `with_dirs`, plus one `NativeStore` scratch tree):
    - P1's exact listings, root through Levels, with P1's inputs;
    - dotfiles are listed, `notes.txt` is filtered, and the colours are 47 and 48;
    - `select` on a nested file, a folder, `""`, `Levels/water_stage.lev` (a miss) and a case-changed path (a hit);
    - `find` fills only the prefix folders; count `list` calls through a counting store wrapper;
    - empty-folder Up/Down/PgUp/PgDn/search do not panic;
    - `exit` at the root does nothing;
    - `enter` on a file row does nothing;
    - the key order: Esc wins over a Left in the same frame;
    - a once-Left and a once-Right in the same frame, inside a child folder: `Exit` runs first, then `Enter` re-enters the folder under the parent's cursor, so the net folder is unchanged (fact 9's order).
- [ ] **Step 4: `files::LevelSelectorState`** (facts 1, 2, 7, 10).
  - `enter(w, store)`: fill the root with `LEV`, insert RANDOM at `children[0]` (name `w.tc.random`, `full_path ""`, not a folder), `set_folder(root)`, `select(&w.settings.level_file)`, `preview = None`, `prev = (52, 36)`.
  - `update(cx) -> SelectorOut`: `process`, then Return / KP Enter / any Fire → `MenuSelect` → `enter()`. A file gives `Picked::Random` or `Picked::Level(full_path)`.
  - `draw(w, store, font)`:
    1. `surface ← frozen`;
    2. `draw_extra`: the preview into `w.frozen` through `w.pal32`, with `fill_rect(134, 162, prev…, 0)` and then `draw_miniature_ids`, reading `store.read(node.rel)` and gating it on `cpp_accepts(…, w.settings.load_powerlevel_palette)`; then the title via `draw_framed_text(surface, …, 178, 20, 50)`;
    3. `selector.draw`.
  - **Tests:**
    - the preview lands in `frozen` and not in this frame's `surface`, and appears in the next frame's;
    - the first clear is 52×36; the water → `tiny` footprint (fact 10);
    - RANDOM, folders and rejected files leave `frozen` unchanged, while `preview` still moves on a rejected file;
    - the title strings at the root and in Levels;
    - the restore frame's cursor is on `water_stage` in Levels, with the parent pane showing `openliero`.
- [ ] **Step 5: `files::SetupSelectorState`** (facts 1, 2, 8, 13).
  - `enter`: fill with `CFG` (no RANDOM), `set_folder(root)`, `select(join_path(root_label, "Setups"))`.
  - `update`: as the level selector's, with a file giving `Picked::Setup { rel, name: get_basename(leaf) }`.
  - `draw`: `surface ← frozen`; the framed `"Select options:"` title (+ `' '` + the path); `selector.draw`.
  - **Tests:** it opens inside Setups on the first entry (`liero`, then `mine`, `orbmit`); Left shows the root with the cursor on Setups; the title.
- [ ] **Step 6: the screens and the stack.**
  - `Screen::LevelSelect(LevelSelectorState)` and `Screen::SetupSelect(SetupSelectorState)`: not overlays; `wants_menu_flip`; `Phase::Menu`; `top_char` `L` / `P`.
  - `MenuCtx` gains `store: &'a dyn ConfigStore`. Every construction site passes `&*self.store`.
  - In `Shell::frame`:
    - the new arms call `update` and keep `picked`;
    - after the update and before `finish_update`, a `picked` runs `self.apply_picked(p)`. In this task that is only the RANDOM/level arm: `random_level`, `level_file`, `settings_menu.update_items(SettingsModel)`. The setup arm is `todo!` until T4, behind a `debug_assert!` that no T3 test reaches it;
    - `enter()` runs each selector's `enter(&mut self.world, &*self.store)`;
    - `draw_stack` gets the screen `&mut` through a split borrow (fact 20).
  - `Shell::selector_view() -> Option<SelectorView { top: char, folder: String, selection: i32 }>` is for the harness and the glue.
- [ ] **Step 7: headless flows** (`ui/src/shell/mod.rs` tests, through `Shell::frame`, on a `MemoryStore` fixture with root label `./user`), pushing the selectors directly (T4 wires the Enter arms):
  - the level selector's full P1 path, frame by frame: `top_char`, `selector_view`, the sounds per key (fact 9), and no sound for Right/Left/Esc;
  - pick `water_stage` → the LEVEL value `"water_stage"`, RELOAD LEVEL, MAP W/H hidden;
  - pick RANDOM → `Random`;
  - the preview persists into the main menu after Esc;
  - the stack's `(sel, fading)` still come from the buried main menu (e-1 fact 3).
- [ ] **Step 8: gate.**
  - The re-diff, the wasm check.
  - `-p oracle-tests --test menu_widget_golden` (G1) and `--test shell_golden` (all 22) green.
  - `gen_slice4_5d -- check` and `gen_slice4_5e1_shell -- check` clean.
  - The golden status empty.
- [ ] **Step 9: commit** `ui(4.5e-2): the file tree (DirectoryListing, ChildSort/CiLess, Find/Select, lazy fill), the level selector (RANDOM, restore, the one-frame-late preview) and the setup selector; C++ Level::load's accept rules for reads and previews`.

**Done when:** every test above is green, no prior gate moved, and `cargo tree -p ui -e normal | grep -c bevy` prints `0`.

---

### Task 4 (Batch 5): `ui`, the live Enter arms, the Save-As chain, LOAD SETUP, detach

**Files:** `rust/ui/src/shell/{main_menu.rs, overlay.rs, mod.rs, playing.rs, files.rs}`, and callers only if a signature moves.

- [ ] **Step 1: the Enter arms** (`mainMenuState.cpp:275-311`; facts 12, 18). They replace D6:
  - `SI_LEVEL` and `LOAD_OPTIONS`: `MenuSelect`, then push `LevelSelect` or `SetupSelect`.
  - `SAVE_OPTIONS`: `MenuSelect`. Then, if `settings_menu.item_position(idx)` is `Some((x, y))`, push `InputString(InputStringState::new(w.setup_name.as_bytes(), 30, x + value_offset_x + 2, y, None, "", false, InputPurpose::SaveSetupAs { x: x + value_offset_x + 2, y }))`.
  - Update the e-1 unit test that pinned D6 (fact 18) to the new pushes.
- [ ] **Step 2: the Save-As chain** (facts 12, 15; D7).
  - `InputPurpose::SaveSetupAs { x, y }` and `InfoPurpose::Reserved { typed: Vec<u8>, x, y }`.
  - `input_done(SaveSetupAs)`:
    - `!accepted || buffer.is_empty()` → the completion: `MenuSelect` + `update_items`;
    - else `leaf = <buffer as text> + ".cfg"`. If `store.shadows_system("Setups", &leaf) || !placeable_leaf(&leaf)` → `stack.schedule_replace_top(InfoBox(format!("NAME '{leaf}' IS RESERVED"), 160, 100, true, Reserved { typed: buffer, x, y }))`, with no sound;
    - else `store.write("Setups/" + leaf, settings_to_toml(&settings))`. On `Ok`, `setup_name = name`; on `Err`, a note. Then `MenuSelect` + `update_items`.
  - `close(Info(Reserved{..}))`, after `keys.clear()` and the fill: `stack.schedule_replace_top(InputString(new(typed, 30, x, y, None, "", false, SaveSetupAs { x, y })))`.
  - The buffer → text conversion is ASCII as-is, and bytes ≥ 0x80 go through CP437 → Unicode. The box text shows the same string through the fact-8 display mapping (e-1).
- [ ] **Step 3: LOAD SETUP** (fact 13; D8, D17). `apply_picked(Picked::Setup { rel, name })`:
  - `store.read(&rel)`, then UTF-8, then `settings_from_toml`.
  - On `Ok(s)`: `world.settings = s`; `setup_name = name`; if a match exists and `debug.load_detach`, `m.detach()`; `settings_menu.update_items(SettingsModel)`.
  - On `Err`: a note, and nothing changes.
  - `Match::detach()` replaces `detach_for_test`.
- [ ] **Step 4: the write-back gate** (fact 17). `new_game` calls `old.write_back_picks(..)` only when `old.attached()`. **Test:** pause during selection with P1's picks moved, LOAD SETUP, NEW GAME: the menu's picks are the loaded setup's, not the old selection's.
- [ ] **Step 5: `Shell` API.**
  - `FrameOut::notes: Vec<String>` (Rust-only messages for the glue to log);
  - `text_mode() -> Option<TextMode>` (D11);
  - `setup_name() -> &str`;
  - `level_from_file() -> bool`, which `LevelSlot` records when `read_level` returned `Some`;
  - `ShellDebug::load_detach` (D17).
- [ ] **Step 6: headless flows** (`mod.rs` tests):
  - LEVEL → the tree → `water_stage` → NEW GAME: `level_from_file()`, and the sim's level material equals `water_stage`'s (after `MakeShadow` when `shadow` is on: compare against `generate_level` with the same file).
  - **Q4 (Rust-only):** the same with the level **only in the system layer** of a split `MemoryStore` → the file is played.
  - Reopen LEVEL → restore.
  - RANDOM → NEW GAME generates.
  - SAVE SETUP AS…:
    - a bare Return → the reserved box (`clear_screen`), with one `MenuSelect` on the close frame;
    - any key → `I` again, holding `liero`;
    - Backspace ×5 + `mine` + Return → two `MenuSelect`s, the store has `Setups/mine.cfg` with `settings_to_toml`'s bytes, and the value reads `mine`;
    - `orbmit` → the box;
    - Esc → two `MenuSelect`s, and nothing saved;
    - `a/b` → the box (Rust-only, D7).
  - LOAD SETUP:
    - it opens in Setups;
    - `orbmit` → the settings equal `data/Setups/orbmit.cfg`'s parse, and the name is `orbmit`;
    - a paused match is detached: RESUME leaves the sim's live-read fields unchanged. With `load_detach = false`, they change;
    - an unparseable user `Setups/bad.cfg` → a note, and the settings and the name are unchanged.
  - The single-layer store refuses only `liero`, so `orbmit` saves there (D9).
- [ ] **Step 7: gate.** The re-diff, the wasm check, G1 and all 22 G2 cases green, `gen_slice4_5d -- check` and `gen_slice4_5e1_shell -- check` clean, the golden status empty.
- [ ] **Step 8: commit** `ui(4.5e-2): LEVEL / LOAD SETUP / SAVE SETUP AS… live — the Save-As chain (reserved box, reopen), LOAD SETUP (fresh settings, name, detach), picks written back only while attached; the Q4 fix plays a system-only level`.

**Done when:** every flow above is unit-tested and green, and every prior gate is unchanged.

---

### Task 5 (Batch 6): G2e-2, the shell corpus and the C++ goldens

**Files:** `rust/oracle-tests/tests/shell_common/mod.rs`, new `tests/shell_e2_cases/mod.rs` and `examples/gen_slice4_5e2_shell.rs`, and the 22 `golden/shell_*` e-2 files.

- [ ] **Step 1: `shell_common`.**
  - **Intervention 6′'s mirror:** after every `sh.frame(...)` and before the `d` line, `sh.settings_mut().record_replays = false`.
  - `upd` and `top` grow `L`/`P` through `top_char()`, with no code change.
  - `B::type_chars(s)`: per char, `key down <UPPER>` + `text <hex>` on one frame and `key up` two frames later.
  - `Fs::install()` and the user-line builders (§Formats).
  - `Opts.q4_twin: bool`. `make_fixture` then skips each `file user TC/openliero/Levels/<x>` line whose `<x>` also has a `file sys` line, the Q4 validator records instead of refusing, and the run reports the count (D1.2).
  - **Ledger fields:**
    - `selector: Vec<Option<SelectorView>>` per frame;
    - `previews: Vec<(u32, String)>`: frames whose `frozen` preview rectangle changed, with the node;
    - `reserved_boxes`;
    - `saves` and `loads`;
    - `level_from_file` per NEW GAME;
    - `q4_hits`.
- [ ] **Step 2: validators** (`Ledger.violations`; each refused at generate time):
  - drop e-1's D6 validator (fact 18). Keep every other e-1 validator;
  - **Q4** (D3's mirror): at boot and on every NEW GAME route with `!random_level` and a root-form `level_file`, a violation when the file is absent from `tmp/user` but `store.read(rel)` succeeds;
  - a search letter that is a keyboard player's control, on an `L` or `P` frame as on `O` (fact 24);
  - a `P`-top entry into `TC/openliero/{weapons,nobjects,sobjects}` (§Formats: the fixture leaves those files out);
  - a NEW GAME on an accepted level shorter than 342 rows (e-1 Addendum G3);
  - LOAD SETUP of a setup whose NEW GAME Rust would refuse (Holdazone, unequal health, no weapon);
  - a Save-As accept whose name is not ASCII, or not `placeable_leaf` (D7 is Rust-only);
  - any non-ASCII `text`.
- [ ] **Step 3: the 6 cases** (`shell_e2_cases::cases()`). All have `detail` and `fs`. Every `match_seed` is scripted, each boot seed is distinct, and every case ends by `QUIT`, so the exit save writes `file` lines.
  1. **`level_tree`** (P1, P3). User: `dir user Replays`; `TC/openliero/Levels/{Zeta.lev ← water, alpha.LEV ← water, .hidden.lev ← render_stage, notes.txt ← data/README.md, tiny.lev ← golden/shell_level_tree_tiny.lev}`.
     - LEVEL → the root rows → Profiles (empty; the parent pane) → back → TC → Right ×3 → Down through every Levels row, 3 frames apart (the late preview, the 52×36 first clear, the `tiny` footprint);
     - PgDn, PgUp;
     - Left (the parent pane) → Right;
     - a `contains` search with unbound letters, e.g. `h`,`i`,`d`, typed within the gap;
     - Esc → the menu shows the last preview → Jump (P1) back to the main focus → QUIT.
  2. **`level_pick`** (D1.1). User: `TC/openliero/Levels/water_stage.lev` (a copy of the sys file).
     - LEVEL → `water_stage` → the value `"water_stage"`, RELOAD LEVEL, MAP W/H hidden;
     - NEW GAME → selection (`Level: "water_stage"`) → DONE → 300 ticks with `state8` → Esc;
     - F7 → LEVEL reopens on `water_stage` (restore) → Left ×3 to the root → `[RANDOM]` → Enter;
     - Esc (focus main) → the cursor to NEW GAME → Return → selection → DONE → 150 ticks → Esc → QUIT.
  3. **`level_missing`**. User: `Setups/liero.cfg ← golden/shell_level_missing_user_liero.cfg` (`randomLevel = false`, `levelFile = './user/TC/openliero/Levels/gone.lev'`), `TC/openliero/Levels/{broken.lev ← data/README.md, trunc.lev ← golden/shell_level_missing_trunc.lev}`.
     - Boot: the random fallback. LEVEL shows `"gone"`;
     - NEW GAME → random → 150 ticks → Esc;
     - LEVEL opens at the root on `[RANDOM]`, because `gone` is not found;
     - to Levels → `broken` (no preview) → `trunc` (no preview) → Enter;
     - NEW GAME → random again (C++ rejects `trunc`, D6) → 150 ticks → Esc → QUIT.
  4. **`setup_save`** (P5). User: none. The T0 P5 path, then Esc → QUIT. The `file` lines are `Setups/liero.cfg` and `Setups/mine.cfg`.
  5. **`setup_load`** (P4, P8). User: `Setups/mine.cfg ← golden/shell_setup_load_user_mine.cfg` (Game of Tag, `timeToLose = 120`, lives 3, a random level, everything else default).
     - NEW GAME → DONE → 300 ticks → Esc;
     - F7 → LOAD SETUP (opens in Setups on `liero`) → Down ×2 → Left → Right → `orbmit` → the values change → Esc;
     - F1 (RESUME) → 300 ticks: detached, with the old settings;
     - Esc → F7 → LOAD SETUP → `mine` → Esc (focus main) → the cursor to NEW GAME → Return → selection → DONE → 200 ticks: Game of Tag from `mine`;
     - Esc → QUIT → `liero.cfg` holds `mine`'s values.
  6. **`setups_and_levels`** 🎯 (done-when 7). User: the `water_stage` copy.
     - LEVEL → TC/openliero/Levels → `water_stage` → NEW GAME (the file is played) → DONE → 200 ticks → Esc;
     - F7 → LEVEL reopens on `water_stage` → Esc;
     - SAVE SETUP AS… → Return (`liero` refused) → key → Backspace ×5 → `mine` → Return;
     - LOAD SETUP → `orbmit`;
     - Esc (focus main) → the cursor to NEW GAME → Return → selection → DONE → 100 ticks → Esc → QUIT.
     - The `file` lines are `Setups/liero.cfg` (orbmit's values), `Setups/mine.cfg` (the `water_stage` settings) and the `water_stage` copy.
- [ ] **Step 4: witnesses** (the generator prints each case's ledger; `write` refuses a case that lacks its own). Every case needs no violations.

  | Case | Witnesses |
  |---|---|
  | `level_tree` | tops `L` seen; ≥ 6 previews, each landing exactly one frame after its Down; one preview on `tiny` after a 504×350 one; the menu frame after Esc shows the last preview; an empty-folder visit; the search moved the cursor |
  | `level_pick` | `level_from_file` true for NEW GAME 1 and false for NEW GAME 2; a restore frame with the cursor on `water_stage` in Levels; the Q4 twin fires ≥ 1 (T6) |
  | `level_missing` | the boot and both NEW GAMEs random; the LEVEL value `"gone"`, then `"trunc"`; no preview on `broken` or `trunc` |
  | `setup_save` | 2 reserved boxes; 1 save (`mine`); 1 cancel; the value `mine` |
  | `setup_load` | 2 loads; a `P` frame inside Setups on `liero`; **counterfactual:** with `load_detach = false`, the resumed `state8` diverges (record the tick) |
  | `setups_and_levels` | all of the above in one run |

- [ ] **Step 5: `gen_slice4_5e2_shell.rs`.** `check` or `write <golden dir>`. It writes the inputs (the two `.lev` files from fixed bytes and the two user `.cfg`s via `settings_to_toml` of the stated `Settings`) and the manifests first, then the 6 scripts with a two-line header (case + ledger). It then asserts no violations and every witness. **Run:**
  1. `cargo run --manifest-path rust/Cargo.toml -p oracle-tests --example gen_slice4_5e2_shell -- write /home/user/openliero/rust/oracle-tests/golden`;
  2. `… -- check`;
  3. `gen_slice4_5d -- check` and `gen_slice4_5e1_shell -- check`, both still clean.
- [ ] **Step 6: the C++ goldens.**
  - `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh`, with the default count of 28. All `awk` gates pass, and the 22 prior goldens are still byte-identical (the golden status shows only the new files as `??`).
  - Then run the `$S/build-chk` loop over all 28: every output is `SAME` as its golden, and nothing is reported under ASan.
- [ ] **Step 7: commit** `oracle(4.5e-2): G2e-2 — 6 shell cases (the level tree, a picked file level, a missing and a rejected level, SAVE SETUP AS…, LOAD SETUP; generator-validated, with the late-preview, restore and detach witnesses) and their C++ goldens from the real headless Gfx frame loop`. Stage exactly the 22 golden files and the three source files.

**Done when:** the generator is clean with every witness, the C++ gen script passes under both builds, the 22 prior goldens are unchanged, and there are 22 new golden files.

### Task 6 (Batch 6): 🎯 MILESTONE e-2, every G2e-2 frame, `d` line and saved file bit-exact, and the Q4 twin

**Files:** `rust/oracle-tests/tests/shell_golden.rs`, plus `rust/ui` / `rust/render` for proven fixes only.

- [ ] **Step 1: `shell_golden.rs`.**
  - `mod shell_e2_cases;`.
  - `the_committed_scripts_are_the_generators` covers the union of 28, each script against its own generator.
  - New tests:
    - `the_e2_milestone_is_bit_exact` (`setups_and_levels`, with its ledger asserted: 2 NEW GAMEs, 1 reserved box, 1 save, 1 load, quit, 3 `file` lines);
    - `every_g2e2_case_is_bit_exact`;
    - **`the_q4_fix_plays_a_system_only_level`** (D1.2), for `level_pick` and `setups_and_levels`: `drive_with(…, Opts { q4_twin: true, .. })`. Every `f` and `d` line must equal the committed golden, the `file` lines must equal the golden's minus `TC/openliero/Levels/water_stage.lev`, and `q4_hits ≥ 1`;
    - `#[should_panic] the_q4_twin_sees_a_missing_level`: the twin with the system copy dropped too, so Rust plays random. It must fail, which shows the twin is not vacuous.
- [ ] **Step 2: the fix loop.**
  - Run `cargo test --manifest-path rust/Cargo.toml -p oracle-tests --test shell_golden`.
  - On the first mismatch, get PPMs of both sides around it. Rust: `SHELL_RUST_PPM_DIR=$S/g2r`. C++: `SHELL_PPM_DIR=$S/g2c bash rust/oracle-tests/gen_shell_golden.sh`, which rewrites the same bytes (the golden status stays empty).
  - Fix the Rust side (`ui`/`render`). Each fix gets a unit test pinning the C++ line it follows, and the full re-diff.
  - Never edit a golden. A dumper bug is fixed in T2's file, with its regeneration proof (22 byte-identical) re-run, and only the e-2 goldens are regenerated.
- [ ] **Step 3: gate.**
  - `shell_golden` (all 28 cases plus the twin) green, the full re-diff green, `menu_widget_golden` green.
  - The golden audit: `git diff --name-status f39b5ac -- rust/oracle-tests/golden | grep -v '^A'` is empty, and `| wc -l` is **22**.
- [ ] **Step 4: commits.**
  - Fixes first, each `ui(4.5e-2): <what> (found by G2e-2 <case>, frame N)`.
  - Then `oracle(4.5e-2): 🎯 MILESTONE e-2 — every G2e-2 case bit-exact against the real C++ Gfx frame loop (frames, d lines, saved files); the Q4 twin plays a system-only level exactly as C++ plays a user copy`.

**Done when:** all 6 e-2, all 11 e-1 and all 11 4½d cases match on every line, the twin passes, and the negative twin panics as expected.

---

### Task 7 (Batch 7): `game`, the wasm catalogue, `?level=`, the phone keyboard per box

**Files:** `rust/game/src/{config.rs, main.rs, web_params.rs, touch.rs}`, `web/index.html`, `.github/workflows/preview.yml`.

- [ ] **Step 1: the browser store** (D9). `browser_store()` = `MemoryStore::single_layer(&browser_system_files()).with_dirs(EMBEDDED_DIRS)`, root label `/openliero`. **Tests (native):**
  - `list("")` is Profiles, Resources, Setups, TC;
  - `list("TC/openliero/Levels")` is the 4 small levels;
  - `shadows_system("Setups", "orbmit.cfg") == false`;
  - `load_settings` reads `Setups/liero.cfg`;
  - `web_params::LEVELS` equals the `EMBEDDED_LEVELS` stems.
- [ ] **Step 2: the native shadow root** (D10). `StoreSpec::SingleDir { root, shadow: Option<PathBuf> }`: `resolve_store` fills `shadow` with the `SystemDataRoot()` resolution, and `open` applies `with_shadow_root`. **Tests:** `OPENLIERO_DATADIR` set and present; unset (`DATA_ROOT`); a missing directory.
- [ ] **Step 3: `?level=` in the canonical form** (design §7.5). `MatchParams::apply_level(&self, s, root_label)` stores `format!("{root_label}/TC/openliero/Levels/{stem}.lev")`, and `setup` passes `store.root_label()`. **Tests:** `/openliero/TC/openliero/Levels/water_stage.lev`; then `read_level` through `browser_store()` gives `water_stage`; and a `Shell` booted with `?level=water_stage&menu=1` settings opens LEVEL with the cursor on `water_stage` (`selector_view`).
- [ ] **Step 4: notes and hooks.**
  - Each tick's `FrameOut::notes` goes to `game::config::warn`.
  - On wasm, `publish_top` also publishes (D11):
    - `lieroSel` = `L<n>` / `P<n>` for a selector;
    - `lieroFolder`;
    - `lieroSetup`;
    - `lieroLevel`;
    - `lieroTextMode`.
  - **Test (native):** a pure `hooks(shell) -> Hooks` struct that the wasm publisher uses, over each top.
- [ ] **Step 5: `web/index.html`** (D11, D12).
  - In `focusEntry()`, set `entry.inputMode = window.lieroTextMode === 'text' ? 'text' : 'numeric'` before `focus()`. The phase watcher also refocuses when `lieroTextMode` changes while the phase stays `text`, i.e. from the reserved box back to the input.
  - Keep `autocapitalize="off"`, `autocorrect="off"` and `spellcheck="false"`.
  - `HINTS.text` shows `['Type a name, FIRE = OK', 'MENU = cancel']` when the mode is `text`, and today's hint when it is `numeric`.
  - `HINTS.menu` stays as it is. The help text gains one line: LEVEL / LOAD SETUP (↑↓ pick, → open a folder, ← back, Enter picks, type to search) and SAVE SETUP AS… (type a name).
  - `.github/workflows/preview.yml`'s help text gains the same line.
- [ ] **Step 6: gate.**
  - `cargo test --manifest-path rust/Cargo.toml -p game`, the wasm check, and the re-diff without `game`.
  - Build the bundle and serve it on 8765.
  - `$S/e2.mjs` is a copy of `b8.mjs`, with `tap1.mjs`'s CDP touch where noted.
  - **Desktop:**
    - F7, Down ×2, Enter → `lieroTop === 'L'` and `lieroFolder === '/openliero'`;
    - Down ×4 (TC), Right ×3 → `lieroFolder === '/openliero/TC/openliero/Levels'`, with 4 rows;
    - `w`,`a`,`t` (keys) → the cursor on `water_stage` → Enter → `lieroLevel === '/openliero/TC/openliero/Levels/water_stage.lev'`;
    - Esc, F1 → selection → `lieroPhase === 'weapsel'`; wait about 30 s after DONE before any in-match check → Esc;
    - F7 → SAVE SETUP AS… → `lieroPhase === 'text'` and `lieroTextMode === 'text'` → Backspace ×5, type `mine`, Enter → `lieroSetup === 'mine'`;
    - LOAD SETUP → `lieroTop === 'P'` → Down ×2 → Enter → `lieroSetup === 'orbmit'` and `lieroMode === 0`;
    - no page errors.
  - **`?level=water_stage&menu=1`:** F7, LEVEL → `lieroFolder` ends in `/Levels`, and `lieroSel` points at `water_stage`.
  - **Emulated phone** (touch, portrait and landscape):
    - pad to MATCH SETUP, FIRE → pad to LEVEL, FIRE → `L`;
    - pad Down held → auto-repeat moves more than one row;
    - pad Right ×3 into Levels, FIRE on a level → back to `M`;
    - pad to SAVE SETUP AS…, FIRE → `#text-entry.inputMode === 'text'`; a number entry → `'numeric'`;
    - fill the field (Backspace ×5, `mine`) and FIRE → `lieroSetup === 'mine'`;
    - LOAD SETUP, FIRE → pad Down, FIRE → `lieroSetup` changes;
    - MENU leaves a selector;
    - an info box closes on any button.
  - Report `ok`/`FAIL` lines, with screenshots in `$S/e2shots/`.
- [ ] **Step 7: commit** `game(4.5e-2): the browser level catalogue (the C++ web tree minus modern_test), ?level= stores the canonical path, the phone keyboard per box (numeric / text), selector hooks, the --config-root shadow check; the page`.

**Done when:** the `game` tests and the wasm check pass, the Chromium walk is all `ok`, and the non-shell paths are unchanged (`round_trip` and `record_regression` green).

---

### Task 8 (Batch 8): eyeball artefacts, docs, audits, broad review

**Files:** `docs/superpowers/liero-rs-PROGRESS.md`, the overview, the design (status line), the cpp-map, the rust-map, `.claude/skills/liero-shot/SKILL.md`. Re-read each immediately before editing it, because line numbers move.

- [ ] **Step 1: the full green board.**
  - The re-diff, both commands.
  - The wasm check.
  - `cargo tree -p ui -e normal | grep -c bevy` → `0`; `cargo tree -p sim-core --depth 1` has no dependencies.
- [ ] **Step 2: tripwires and audits.**
  - `grep -rnE "HashMap|HashSet|\bf32\b|\bf64\b|SystemTime|Instant" rust/ui/src rust/scenario/src/storage.rs` → empty.
  - `git diff --name-status f39b5ac -- rust/oracle-tests/golden | grep -v '^A'` → empty, and `| wc -l` → **22**.
  - `git diff --name-only f39b5ac -- src CMakeLists.txt` → exactly `src/tools/oracle_dump/shell_dump.cpp`.
  - `git diff --name-only f39b5ac -- rust/sim rust/assets rust/sim-core` → empty, unless there is a listed proven fix.
  - `grep -c "ResMut<Sim>" rust/game/src/main.rs` equals its count at `f39b5ac`.
  - The dumper passes clang-format 22 on the whole file, and `scripts/clang-tidy-diff.sh build/linux-x64 f39b5ac` exits 0.
  - **Reproducibility**, in one shell:
    - `gen_slice4_5d -- check`, `gen_slice4_5e1_shell -- check`, `gen_slice4_5e2_shell -- check`;
    - `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh`;
    - then `git status --porcelain rust/oracle-tests/golden` → empty;
    - then the `$S/build-chk` loop → 28 `SAME`.
- [ ] **Step 3: Xvfb side-by-side** (eyeball only).
  - Build a two-layer root:
    - `U=$S/x14/user`; `mkdir -p $U/TC/openliero/Levels`;
    - `cp data/TC/openliero/Levels/water_stage.lev $U/TC/openliero/Levels/`.
  - Run the real `openliero` with `OPENLIERO_TEST_USER_DIR=$U OPENLIERO_DATADIR=/home/user/openliero/data` and drive the milestone path with xdotool.
  - Snap about 8 moments: the root listing, Levels with a preview, the preview persisting in the menu, the restore, the reserved box, the saved name, the options selector, the loaded values.
  - The Rust side is the browser bundle on the same keys (e-1's note: `SHELL_RUST_PPM_DIR` writes only on a mismatch). Pair them with `x12_pair.py` into `$S/x14/side/`.
  - Note what differs: the root label (`$U…` vs `/openliero`), the level list (4 vs 5, Q6), the level/seed and timing are expected. Anything else is not. Never commit the PNGs.
- [ ] **Step 4: `liero-shot` §7.** Add one paragraph after the e-1 one: the level selector (tree, RANDOM, the late preview, restore), SAVE SETUP AS… / LOAD SETUP, the Q4 fix, the browser catalogue, `?level=`'s canonical form, the per-box phone keyboard, and the gates (G2e-2, the Q4 twin).
- [ ] **Step 5: PROGRESS.**
  - Real date. A new header paragraph, **"4½e-2 LANDED"**; the previous one becomes "Prior (…)".
  - The Step 4½ tree: ✅ 4½e-2, with the milestone.
  - **Also open for John (4½e-2)** / notes:
    - the plan-time facts that corrected or sharpened the design (1, 5, 8, 11, 13, 14, 16, 17);
    - T0's outcomes, P6 above all: C++'s shipped-level bug confirmed in the dumper;
    - how Q4 is shown (D1);
    - intervention 6′ and the Q4 guard;
    - D6 (C++'s stricter level accept rules, now ported);
    - D7/D8's Rust-only refusals;
    - D9 (the browser saves over shipped names, like the C++ web build) and D10;
    - D12 (no type-to-search on phones), D13 (`.zip`), D14;
    - D15 (the `rust/settings` move still open);
    - the session-only browser store (4½h).
- [ ] **Step 6: the overview.**
  - The status line: `4½e LANDED`.
  - At the end of the 4½e bullet: "**4½e-2 landed**", with the plan path and the corrections (finding 2 fixed per Q4, with how it is proven; finding 5 confirmed by T0).
  - Q6: "ruled A; RANDOM + 5 levels on desktop, 4 on the web".
- [ ] **Step 7: the maps and the design.**
  - **cpp-map:** append facts 1–16 at the relevant sections, with T0's observed listings, P6 and P8.
  - **rust-map:** `ConfigStore::list`, the single-layer store, `ui::shell::files`, `cpp_accepts`, the new `Screen`s, `MenuCtx.store`, `FrameOut::notes`.
  - **Design status line:** `**4½e-1 and 4½e-2 LANDED** (plans …e1-plan.md, …e2-plan.md)`.
- [ ] **Step 8: the broad review.** Re-read `git diff f39b5ac -- rust src web .github` against the design and this plan:
  - each fact and decision is either ported and gated (name its G2e-2 case, the twin or a unit test) or recorded in PROGRESS;
  - `FileSelector::process` follows `fileSelector.hpp:251-300` in order;
  - the preview follows `fileSelectorState.cpp:105-145` line for line;
  - the Save-As chain follows `mainMenuState.cpp:69-91` and `:287-311`;
  - LOAD SETUP follows `gfx.cpp:1693-1697`, plus the detach;
  - the dumper's interventions are each documented, touch no code under test, and refuse what they cannot run faithfully;
  - LD 3, LD 4 and LD 5 and the crate rules hold;
  - `game` is glue only;
  - the non-shell paths are unchanged;
  - no push, no PR.

  Bar: 0 Critical / 0 Important.
- [ ] **Step 9:** `git status --short` lists only the docs above. **Commit** `docs(4.5e-2): PROGRESS + overview + maps — slice 4.5e-2 landed (the level selector, SAVE SETUP AS… / LOAD SETUP, the Q4 fix, the browser catalogue; G2e-2)`.

**Done when:** the board is green, every audit matches its expected output, the docs are updated, and the review is clean.

---

## Known pitfalls (carried from e-1, 4½d and PROGRESS, plus e-2's own; read them before your batch)

1. **Goldens are C++ truth.** Never regenerate one to make Rust pass, and never hand-edit one. An `M` under `golden/` means: stop, restore, report. A dumper change must re-prove byte-identity for every golden it writes: all 22 prior shell goldens, under the release **and** the ASan build.
2. **DEBUG only.** `--release` breaks the `#[should_panic]` tests. The one non-test build is the wasm-release bundle.
3. **clang-format 18 is on PATH.** Always use `$CF22`. Always also run the whole-file dry run: the diff script misses a blank line that a deletion leaves behind (CLAUDE.md).
4. **This clone has no `origin/master`.** Pass `f39b5ac` to both diff scripts.
5. **Gen scripts default to `macos-arm64`.** Source `env.sh` in the same shell.
6. **rustfmt recurses** through `lib.rs`/`main.rs`. Use it only on files you created; hand-format the rest.
7. **The selection constructor must draw nothing** (intervention 3). Every NEW GAME's picks must name enabled weapons, including after a LOAD SETUP. The validator checks `sim.rand.draws() == 0` on the route frame.
8. **A worm key held through Esc and released in the menu** stays pressed in C++ and not in Rust (4½d fact 12). Release every worm key before the Esc.
9. **The search timeout is wall-clock in C++** (1500 ms), and the Rust harness uses `now_ms = 0`. Keep each search inside one `O`/`L`/`P` visit and fast. The gap check fails anything slower.
10. **Search letters are control keys:** R, F, D and G move P1, in WEAPON OPTIONS **and in the selectors** (fact 24). The design's `stage` contains `g`, which is a Right. Search only with unbound letters.
11. **SDL text events are one string each.** A multi-char string becomes `'?'` in C++, so the corpus types one char per event (`type_chars`).
12. **Text after Return in the same frame still lands** (e-1 fact 6). Do not "fix" it.
13. **Q4.** Every gated case that *plays* a picked level needs the level in `user/` too. The dumper's guard and the generator's validator refuse it otherwise. The twin is the only place the system-only layout is exercised.
14. **`record_replays` comes back with every LOAD SETUP** (shipped setups say `true`). Intervention 6′ and its mirror run after every frame, before the `d` line. Skip it and C++ writes a wall-clock `.lrp` into `user/Replays`.
15. **The preview is late on purpose.** It is drawn into `frozen` after the surface copy, and it persists into the main menu until the next `MainMenuState::Enter`. Do not "fix" it.
16. **Titles and `level_file` strings come from `root_label()`:** `./user` in the fixture, `/openliero` in the browser, the pref path natively. Never build a path by hand.
17. **The fill is lazy.** Never walk the whole tree: a real user folder may be huge or hold a symlink loop.
18. **Two sorts, two orders:** `list` is bytewise (dedupe), and `ChildSort` is folders first, then `CiLess`. Dotfiles are listed. A file with no extension is filtered out.
19. **`write_back_picks` only while attached** (fact 17). After LOAD SETUP, the old selection's picks belong to the old settings.
20. **Level files shorter than ~342 rows are never played** (e-1 Addendum G3, C++ UB). `tiny.lev` (60×40) is preview-only, and `trunc.lev` is rejected by both sides.
21. **Holdazone is refused in Rust and plays in C++.** No case may NEW GAME or RESUME into Holdazone, including via a loaded setup.
22. **Headless Chromium runs at about 7–10 fps.**
    - A tap longer than 12 ticks trips weapon selection's key repeat, so keep taps short (`frameTap`).
    - The worms start dead: wait about 30 s after DONE before any in-match check.
    - iOS raises the keyboard only from a real tap (TAP TO TYPE).
23. **The native `game` cannot run here** (no GPU). The browser bundle and the G2 harness cover the live path. Restart `http.server 8765` if it is down.
24. **Disk.** Clean the incremental dirs after every batch, and check `df -h /` before any wasm-release build. Rust batches are sequential.
25. **Explicit `git add` paths only.** Never commit the scratchpad, PPMs, PNGs or fixture copies.
26. **Trailers.** Every commit carries `$CO` and `$SESS`, and nothing else names a model. No "Generated with …".

## Done-report (each batch)

Each batch reports:
- (a) what changed and why;
- (b) the files touched;
- (c) the gate commands run, with their results;
- (d) the commit SHAs.

Batch 1 also reports the addendum's verdicts. The final report (Batch 8) surfaces:
- T0's outcomes: the listings, the late preview, restore, the Save-As chain, P6 (C++ random), P7 and P8;
- the dumper evidence: T2's four smokes, and the 22-byte-identical proof under both builds;
- 🎯 the G2e-2 result: 6 cases, the frame, `d`-line and `file`-line counts, with `setups_and_levels` named on its own;
- the Q4 twin: both cases green, and the negative twin panicking;
- the `setup_load` counterfactual tick;
- the Chromium lines;
- the Xvfb PNG paths;
- the audit sweep: 22 `A`, 0 `M`; the one dumper; `cargo tree` 0 bevy; reproducibility byte-identical;
- any sim/render fix, with the case that proved it.

## Addendum T0 (probe results)

Run 2026-09-26 on `claude/cpp-oracle-vcpkg-assets-chcwcm` at `cc50554`, with the real `Gfx::RunOneFrame` in a temporarily patched `oracle_dump_shell`. **Directory:** every artefact is in `$S/t0e2b/`, not `$S/t0e2x/` (the orchestrator's name; `$S/t0e2/` holds e-1's T0). Nothing but this addendum is committed.

**Verdict.**
- P1–P9 are **confirmed**.
- P10 is **partly contradicted**: C++ previews neither file, but a NEW GAME on the truncated-MODERNLV `trunc.lev` is **undefined behaviour in C++** (a heap overflow; it crashed every run). It does not fall back to random. The P10 rule fired and changes T5 (§"Changes to later tasks" below).
- Two precisions change no rule but correct text:
  - fact 24's example letters (`d` is bound);
  - D1.3's control: a plain `[RANDOM]` pick does not regenerate the level.

### Method

- **The patch** is `$S/t0e2b/probe_patch.diff`. It lived in the working tree only.
  - `TopOf` returns `L` for `LevelSelectorState` and `P` for `OptionsSelectorState` (with `#include "fileSelectorState.hpp"`).
  - The `fs` boot `CheckLevelFile` is skipped. Intervention 6 stays once, at boot.
  - A probe-only `x <frame>` line follows each `d` line. It holds:
    - `pvf` and `pvb`: FNV-1a-64 of the preview rectangle (134..185 × 162..197) of `gfx.frozen_screen` and of this frame's `play_renderer.bmp`;
    - `random_level`, `level_file` and `settings_node.FullPath()`;
    - `lives`, `loading_time`, `max_bonuses`, `blood`, `game_mode` and `record_replays`;
    - for a selector on top: the current folder's `full_path` and its menu as `sel[name:colour|…]`, plus the parent's.
  - A `Peek : FileSelectorState` member-pointer accessor reads `selector_`. It reads `node->menu` directly and never calls `GetMenu`, so the probe has no side effects on the tree.
- **Build:** `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell`, and the same patch in `$S/build-chk` for the checked runs.
- **Inputs.**
  - `$S/t0e2b/mk.py` wrote `tiny.lev` (`OLLEVEL2`, 60×40, 2,413 bytes) and `trunc.lev` (`OLLEVEL2` 8×8 + `MODERNLV` + 10 zero bytes, 95 bytes).
  - `$S/t0e2b/gen.py` wrote every script and manifest. Each manifest is §Formats' `Fs::install()` lines plus the probe's user lines.
  - Every script has `setup default`, `boot_seed 7`, `detail` and `fs <s>_fs.txt`, and ends by QUIT. Taps are down at *t*, up at *t*+2, next key at *t*+3.
- **Run:** `build/linux-x64/Release/oracle_dump_shell $S/t0e2b/<s>.txt $S/t0e2b/<s>.out --ppm-dir $S/t0e2b/ppm_<s>`.
- **Analysis:** `$S/t0e2b/an.py <s>.out` prints each frame where `upd`, top, sounds, `d` or `x` changed. The PNG grids are `$S/t0e2b/{t_tree,o_open,s_save}_grid.png` and `q_weapsel.png`.
- **Sound ids in `f` lines:** 25 = Down (C++ `MenuMoveUp`), 26 = Up and PgUp (`MenuMoveDown`), 27 = `MenuSelect`.

### P1: listing, filter, sort, parent pane (`t_tree`): CONFIRMED

The user layer adds `dir Replays` and, in `TC/openliero/Levels/`, `Zeta.lev` and `alpha.LEV` (water copies), `.hidden.lev` (a render_stage copy), `notes.txt` and `tiny.lev`.

- **The root** (f 49, the Return frame: `upd M`, top `L`, sound 27) is `0[[RANDOM]:48|Profiles:47|Replays:47|Resources:47|Setups:47|TC:47]`. The cursor is on `[RANDOM]`, which is colour 48, like a file.
  - The title reads `Select level: ./user` (`t_tree_grid.png`).
  - `Replays` (user only) and `TC` (both layers, listed once) are merged.
- **Profiles.**
  - Down (f 58, sound 25), then Right (f 61, no sound): folder `./user/Profiles`, menu `0[]`.
  - The parent pane shows the root with its cursor on Profiles (`pcur=1`). The titles are `Parent directory` and `Select level: ./user/Profiles`.
  - Left (f 67, no sound): the root, with the cursor still on Profiles.
- **Down ×4 to TC**, then **Right ×3** at f 85, 89 and 93:
  - `./user/TC` = `[openliero:47]`;
  - `./user/TC/openliero` = `[Levels|nobjects|sobjects|sounds|sprites|weapons]`, all 47, with the cursor on Levels;
  - `./user/TC/openliero/Levels` = `0[.hidden|alpha|modern_test|physics_fall_test|render_stage|see_shadow_test|tiny|water_stage|Zeta]`, all 48.
  - `notes.txt` is absent: the filter confirmed. Dotfiles are listed, and `alpha.LEV` shows as `alpha`.
  - The Levels title `Select level: ./user/TC/openliero/Levels` is **clipped at x = 320** (the PNG shows `…/Level`). Rust must clip it the same way.
- **PgUp** (f 127, sound 26) from row 8: row 1 (`MovementPage(-1)`, height 14).
- **Left** (f 133): `openliero`, with the cursor on Levels and the parent pane on `TC`. **Right** (f 139): Levels, with the cursor kept on row 1.
- **Search** (`contains`), keys at f 145, 148 and 151:
  - `h`: the cursor stays on `alpha`, because it contains `h` and the search starts at the current row;
  - `i`: `hi` moves it to `.hidden` (row 0);
  - `d`: **P1's Left**. The folder became `./user/TC/openliero` (fact 24: `TestControlOnce` runs before `OnKeys`). The `d` search then ran on that menu and moved its cursor to `sounds` (row 3): `hid` missed, so the prefix restarted as `d`.
- **Esc** (f 157): `upd L`, top `M`, no sound.

### P3: the late preview (`t_tree`, `b_broken`): CONFIRMED

- **One frame late, every time.** On each frame that lands on a new file, `pvf` (frozen) changes while `pvb` (this frame's surface) still holds the old preview. `pvb` catches up on the next frame.
  - Frames: 93 (entering Levels onto `.hidden`), 100 `alpha`, 103 `modern_test`, 106 `physics_fall_test`, 109 `render_stage`, 112 `see_shadow_test`, 115 `tiny`, 118 `water_stage`, and 148 `.hidden` (by search).
  - `Zeta` (f 121) and `water_stage` redraw identical pixels, so `pvf` does not change.
- **No repaint.** `[RANDOM]`, Profiles and the folders never change `pvf` (`84d3a955…` from boot until f 93).
- **The `tiny` footprint** (PPM f 116): 30×20 drawn at (134, 162). Everything else in the 50×35 box is black: the band x 164..183 and y 182..196 are all `(0,0,0)`. `water_stage` after it (f 119) repaints all 1,750 cells.
- **The first 52×36 clear is not visible in this fixture.** Column x 184..185 and row y 197 are already black in the boot frozen screen (PPM f 92). Only T3's unit test can pin it, per the source.
- **`pal32` is this frame's menu palette.** `modern_test` was previewed at f 146 and again at f 182 (`b_broken`). The two differ (`22dbf89e…` vs `31f013e1…`) in exactly 200 pixels, all in rows 193..196 and all one colour: (252,252,252) vs (188,188,188), a cycling entry. The frozen pixels are fixed ARGB between redraws.
- **The preview persists** into the main menu after Esc: f 157–177 all show `pvb=0a5e9922…`, the `.hidden` preview.

### P2: cursor restore (`t_restore`): CONFIRMED

- Picking `water_stage` (f 91): `upd L`, top `M`, **one** sound 27.
  - `level_file = './user/TC/openliero/Levels/water_stage.lev'` and `random_level` is 0. `cfg16` becomes `efe5c8ad…`, which equals the exit save's `file Setups/liero.cfg efe5c8ad5d5ce4b5`.
- Return on LEVEL again (f 100, `upd M`, top `L`, sound 27): the first `L` frame is **inside Levels, with the cursor on `water_stage`** (row 4). The parent pane is `./user/TC/openliero`, with its cursor on Levels.
- The reopen previews `water_stage` over identical frozen pixels, so its lateness is not visible there. P1 shows it.

### P4: LOAD SETUP (`o_open`, user `Setups/mine.cfg` ← `orbmit.cfg`): CONFIRMED

- **Opening** (f 85, Down ×14 to `ssel` 18, Return: `upd M`, top `P`, sound 27): folder `./user/Setups`, `0[liero:48|mine:48|orbmit:48]`, cursor on `liero`.
  - The parent is `./user` with `2[Profiles|Resources|Setups|TC]`.
  - The title reads `Select options: ./user/Setups`, and the parent pane is drawn (`o_open_grid.png`).
- **Left** (f 92): the root, with the cursor on Setups (row 2); the title is `Select options: ./user`. **Right** (f 98): Setups, with the cursor on `liero`.
- **Down ×2, Return on `orbmit`** (f 110): `upd P`, top `M`, one sound 27.
  - `cfg16` goes `bc29c9d5…` → `ac0f00da…`, with lives 9, loading 20, `max_bonuses` 0 and blood 25.
  - **`record_replays` becomes 1** (P7).
  - `settings_node = './user/Setups/orbmit.cfg'`: the **config-node** path, although the file lives in `sys`. SAVE SETUP AS… reads `orbmit`.
- The exit save (`file Setups/liero.cfg ac0f00dae90d61cd`) equals the loaded `cfg16`.

### P5: the Save-As chain (`s_save`): CONFIRMED

| Frame | Event | `upd` → top | Sounds |
|---|---|---|---|
| 82 | Return on SAVE SETUP AS… (`ssel` 17) | M → I (on `liero`) | 27 |
| 88 | Return | **I → B** | 27 |
| 94 | SPACE | **B → I** (on `liero`) | — |
| 127 | Backspace ×5, `mine`, Return | I → M | **27,27** |
| 134 | Return | M → I (on `mine`) | 27 |
| 170 | Backspace ×4, `orbmit`, Return | I → B | 27 |
| 176 | SPACE | B → I (on `orbmit`) | — |
| 182 | ESC | I → M | **27,27** |

- The box reads `NAME 'liero.cfg' IS RESERVED`, and later `NAME 'orbmit.cfg' IS RESERVED`, on a black screen (`s_save_grid.png`).
- **Replacements are presented in the frame that schedules them.**
  - f 88 (`upd I`) presents the box, and f 89 presents identical bytes.
  - f 94 (`upd B`) presents the reopened input with `liero_`: bytes `c7696b39…`, identical to f 87's input frame.
- After the save, `settings_node = './user/Setups/mine.cfg'` and the value reads `mine`. The cancel changes nothing.
- The `file` lines are exactly `Setups/liero.cfg` and `Setups/mine.cfg`, both `bc29c9d5ce487a40` (the same settings).

### P6: Q4 in the dumper (`q_sys`, `q_rand`, `q_both`; `match_seed 601`): CONFIRMED

- All three share one key path:
  - F7, Down ×12 (REGENERATE LEVEL), then Return (`q_rand` only) or a wait, then Up ×10 to LEVEL;
  - Return, then the pick at f 154, Esc, and F1 (NEW GAME) at f 167. The router frame is f 200;
  - R+UP and LCTRL+RCTRL (DONE), then 234 `state8` ticks, f 205–438.
- **`q_sys` equals `q_rand` on 234/234 ticks and differs from `q_both` on 234/234.**
  - `q_sys` has `water_stage` in `sys` only. `q_rand` picks `[RANDOM]` with REGENERATE LEVEL on. `q_both` has the user copy.
  - `q_weapsel.png` (f 204): `q_sys` shows `Level: "water_stage"` over the same dirt level as `q_rand` (`Level: Random`). `q_both` shows the water stage.
  - **Finding 2 is reproduced in the dumper layout, and D3's guard is needed.**
- **A precision for D1.3's control.** A plain `[RANDOM]` pick is **not** a random regeneration. `q_rand0` (the same path without REGENERATE) differs from `q_sys` on 234/234 ticks.
  - The NEW GAME router reuses the menu's current level when `regenerate_level` is off and `random_level`, `level_file` and the map sizes equal its `old_*` fields (`gfx.cpp:1512-1522`). `q_rand0` therefore played the boot level (boot seed).
  - The documented comparison is therefore "`q_sys` = a `[RANDOM]` pick with REGENERATE LEVEL on, same seeds".

### P7: replays after LOAD SETUP (`r_rec`): CONFIRMED

LOAD SETUP → `orbmit` (f 95, `rr=1`), then NEW GAME, DONE and 60 ticks. The `file` lines are:
```
file Replays/2026-09-26 20.29.52 -.lrp 14650fb0739d0383
file Setups/liero.cfg ac0f00dae90d61cd
```
- A wall-clock name. It also contains spaces, which would break the `file <rel> <fnv16>` shape.
- **Intervention 6′ (D2) is needed** and stays.

### P8: LOAD SETUP detaches (`l_ctl`, `l_load`; `match_seed 801`): CONFIRMED

- The shared path: NEW GAME, DONE, 234 ticks, then Esc.
- Then F7 at f 319, and one of:
  - `l_load`: LOAD SETUP → `orbmit` at f 374;
  - `l_ctl`: an equal idle.
- Then Esc, F1 (RESUME) at f 387, 201 resumed frames, Esc, QUIT.
- **Results:**
  - Pre-pause `state8`: equal on 234/234.
  - Resumed ticks f 420–620: `state8` equal on **201/201**, and the presented frames equal on 201/201.
    - `orbmit`'s `maxBonuses = 0` changes the `rand` draws of a *shared* settings object on the first tick (e-1 Addendum T0 `p_bonus`), so the paused game kept its own settings.
    - The router frame f 420 differs only in `bmp16`, because the menu under it differs.
  - `cfg16` differs from f 374, the load frame, on (291 frames). The exit saves are `bc29c9d5…` and `ac0f00da…`.
- **Finding 1's LOAD SETUP half stands.** T4's detach and D17 stay as planned.

### P10: rejected files (`b_bad`, `b_broken`, `b_rand`): PARTLY CONTRADICTED

User: `TC/openliero/Levels/{broken.lev ← data/README.md (302 bytes), trunc.lev}`. The Levels rows are `broken, modern_test, physics_fall_test, render_stage, see_shadow_test, trunc, water_stage`.

- **Previews: confirmed.**
  - Entering Levels onto `broken` (f 138) leaves `pvf` at the boot value. Landing on `trunc` (f 162) leaves `see_shadow_test`'s `e167d4c8…`. Returning onto `broken` (f 186) leaves `modern_test`'s.
  - The preview's local `Level` is discarded after the exception, so hovering `trunc` is safe: the checked build ran `b_broken`, which hovers it, clean.
- **`broken` at NEW GAME: random, confirmed.** `b_broken` picks `broken`, and `b_rand` picks `[RANDOM]` with REGENERATE on (same seeds, same frames). They are equal on **134/134** ticks (f 245–378), and the checked build is clean.
  - The legacy material read throws after `Resize(504, 350)`, with the display arrays already cleared. `GenerateRandom` then rewrites every cell.
- **`trunc` at NEW GAME: C++ UB. Contradiction.**
  - `b_bad` dies on its NEW GAME router frame in every build:
    - release, run 1: SIGSEGV;
    - release, run 2: `free(): invalid pointer`;
    - checked build: `stl_vector.h:1128 … Assertion '__n < this->size()' failed` in `Level::SetPixel` ← `GenerateDirtPattern` ← `GenerateRandom` ← `GenerateFromSettings` ← `Gfx::RunOneFrame` (gdb).
  - **Cause** (`level.cpp:312-327`, `level.hpp:72-80`):
    1. `load` resizes `display_data` and `display_valid` to w·h = 64 as soon as the `MODERNLV` magic matches;
    2. the following `r.Get` throws;
    3. `GenerateFromSettings` catches it and calls `GenerateRandom`, whose `Resize` resizes only `material_id` and `materials`;
    4. every `SetPixel` then writes `display_valid[idx] = 0` past a 64-byte vector.
  - Any file that passes the `MODERNLV` magic and then fails clause (e) of fact 11 does this. With w·h ≥ the random map's cells it would not overflow, but it would leave a stale, non-empty `display_valid`/`display_data` on the random level, which is a divergence as well.
- **Fact 11's closing sentence is amended.**
  - A C++ rejection under clauses (a)–(d), or a missing file, means a random level at NEW GAME (proven for (b) by `broken`) and no preview.
  - A clause-(e) rejection means no preview (safe) and **UB at NEW GAME**.
  - Rust's `cpp_accepts` → random fallback for (e) is therefore a documented divergence only where C++ is UB, like e-1 Addendum G3's safe edges. No gated case may reach it.

### P9: the `--config-root` shadow check (real `openliero` under Xvfb): CONFIRMED

- **Setup.** `Xvfb :99`. `$S/t0e2b/p9.sh <run>` copies `data/` to `$S/t0e2b/p9/<run>/root` and starts `openliero --config-root <root>`. It then sends F7, Down ×13, Return, Backspace ×5, `xdotool type` of `orbmit` one character at a time, and Return, and snaps.
- **`env`** (`OPENLIERO_DATADIR=/home/user/openliero/data`): the black `NAME 'orbmit.cfg' IS RESERVED` box (`p9/env_3_after.png`). `root/Setups/orbmit.cfg` is unchanged (md5 `9a2e9c4a4069…` before and after).
- **`noenv`** (unset): **saved**. The value reads `orbmit` (`p9/noenv_3_after.png`), and `root/Setups/orbmit.cfg` was rewritten (2,008 bytes, md5 `d6a26e1b9da1…`).
  - The compiled-in `OPENLIERO_DATADIR` is `/home/user/openliero/install/linux-x64/share/openliero`, which does not exist. `SDL_GetBasePath()` (`build/linux-x64/Release/`) has no `Setups`.
- D10 stands as written. Xvfb and the game were stopped afterwards.

### Restore

- `git checkout -- src/tools/oracle_dump/shell_dump.cpp` (the patch was re-applied once for `q_rand0` and restored again), then both builds were rebuilt. `git status --short src` is empty.
- The restored binaries regenerate `shell_boot_idle.txt`, `shell_milestone.txt`, `shell_cfg_boot.txt` and `shell_match_setup.txt` byte-identically (`cmp`), under **both** the release build and `$S/build-chk`.
- The stale fixture `/tmp/oracle_shell_fs_b_bad.txt`, left by the crashes, was removed.

### Changes to later tasks (the contradiction rules applied; these supersede the task text above)

1. **T5 Step 3, case 3 `level_missing`** (the P10 rule: never NEW GAME a clause-(e) level). The path becomes:
   - boot: the random fallback, and LEVEL shows `"gone"`;
   - NEW GAME → random. The router does not regenerate: the settings still equal the boot level's `old_*` fields, so it reuses the boot level, which is itself the random fallback. Then 150 ticks → Esc;
   - LEVEL opens at the root on `[RANDOM]`;
   - to Levels → the cursor on `broken` (no preview) → Down to `trunc` (no preview) → **Up back to `broken` → Enter**;
   - NEW GAME → random (C++ rejects `broken` under clause (b)) → 150 ticks → Esc → QUIT.

   The `level_missing` witness in T5 Step 4 becomes: the LEVEL value `"gone"`, then **`"broken"`**; no preview on `broken` or `trunc`; the boot and both NEW GAMEs random. `trunc.lev` stays in the corpus as a **preview-only** input.
2. **T5 Step 2 gains a validator:** a NEW GAME (or a boot) whose `level_file` passes the `MODERNLV` magic but fails `cpp_accepts` is a violation (C++ UB, P10). Rust's random fallback there stays, is unit-tested in T3 Step 2, and is never gated.
3. **T5 Step 3, case 1 `level_tree`:** the search letters are **`h`,`i`**, not `h`,`i`,`d`. `d` is P1's Left and leaves Levels (P1, f 151).
   - Observed: `h` keeps the cursor on `alpha`, and `hi` moves it to `.hidden`.
   - Any search-moved witness must start from a row that does not contain the first letter, or count the `hi` move.
   - Fact 24 and pitfall 10 stand. Only the example was wrong.
4. **Pitfall 20** now reads: `trunc.lev` is preview-only in both languages. Its NEW GAME is C++ UB, and no case may pick-and-play it.
5. **D1.3 / T8's PROGRESS line for P6:** state it as "`q_sys` = a `[RANDOM]` pick with REGENERATE LEVEL on (same seeds), 234/234". A plain `[RANDOM]` pick reuses the menu's level (router reuse).
6. **For T3's unit tests:**
   - `[RANDOM]` is colour 48;
   - the Levels title is clipped at x = 320;
   - the replacement box and the reopened input each present on the frame that scheduled them (P5).

   No task text changes for these: they are how C++ already behaves, and the G2 cases gate them.

No rule fired for P1–P9. T1, T2, T4, T6, T7 and D1–D17 are otherwise unchanged. Intervention 6′ (P7) and the Q4 guard (P6) stay.
