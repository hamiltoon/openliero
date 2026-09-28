# Step 4½, Slice 4½f-2: the player menus (LEFT / RIGHT / NETWORK PLAYER), profiles, key capture with the fixed DIG, the live keyboard from the bindings, names in play (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN). This plan names types, fields, files and behaviour; it does not write the code. Where it quotes C++ it quotes the source, and the source wins over this text.

**Goal.** The three player items of the main menu stop being inert and work exactly as in C++:
- **LEFT PLAYER (F5), RIGHT PLAYER (F6), NETWORK PLAYER (F9)** open the player menu (24 rows, 22 or 24 visible, always scrolling): NAME, HEALTH, the Red/Green/Blue rows with their bars, INPUT, the eight key rows (AIM UP … JUMP, DIG) with the `PRESS A KEY` box, WEAPON 1–5 (Left/Right, or a typed name through the fuzzy match), CONTROLLER (Human / CPU / AI);
- **profiles**: LOAD PROFILE (the `Select profile:` tree, opened inside `Profiles`), SAVE PROFILE (always the user copy), SAVE PROFILE AS… (the reserved-name box and its reopen), PROFILE LOADED;
- **the live keyboard follows the bindings** (C++ `FindControlForKey`: first keyboard player, first control, DIG included), natively, in the browser and in the gate;
- **names** are drawn in weapon selection and in the kill banners;
- on a phone the on-screen buttons **always** work (John's Q6) and a profile loaded into RIGHT PLAYER keeps player 2 the CPU (John's Q7).

Every presented frame, every `d` line (with the player menu's cursor and the per-frame state hash) and every saved file on these paths is bit-exact against the **real C++ `Gfx::RunOneFrame`** run headlessly here (**G2f-2**, 8 cases + 🎯 `shell_player_setup`). The one intended difference is John's Q4: binding DIG no longer overwrites WEAPON 1 — a divergence only where C++ is undefined behaviour, which no gate exercises (the gate binds DIG only to a key whose code already equals WEAPON 1).

**Architecture.**
- `ui::keys`: `DosHeld` (the physical keyboard's held DOS keys), `clean_words` (C++ `FindControlForKey` over the held keys: 8-bit words, bit 7 = DIG), `fold_dig` (the selection's DIG → Left+Right), `apply_clean_edges` / `CleanEdges` (C++ `OnKey`'s per-bit edges **and both arms of the DIG rule** over 8-bit words), `ReleaseLatch::{arm_clean, apply_clean}`. The 7-bit `apply_key_edges` / `KeyEdges` / `ReleaseLatch::{arm, apply}` stay for the non-shell paths, byte-for-byte.
- `ui::text`: `KEY_NAMES[177]` verbatim, `get_key_name`, `get_gamepad_key_name`, `CONTROLLERS`, `levenshtein`, `weapon_fuzzy_match`.
- `ui::shell`:
  - `ShellInput { held: &DosHeld, touch: ControlState, .. }` replaces `sampled`; the `Shell` computes the words from the **match's** settings copy (RD-1, RD-2);
  - new `player_menu` module: `player_menu()`, the `PL_*` ids, `PlayerMenuModel` and its behaviours, the colour-bar overlay;
  - `CurMenu::Player(0 | 1 | 2)`, `MenuWorld::{player_menu, profiles}`, the §3.2 Enter arms, F5 / F6 / F9 in C++ order, the network player's slot-0 palette;
  - `Screen::{WaitForKey, ProfileSelect}` (tops `K`, `F`), `InputPurpose::{WormName, WeaponFuzzy, SaveAs { kind }, IntegerEntry { entry, target }}`, `SaveAsKind { Setup, Profile(p) }`, `Picked::Profile`;
  - `Selection` and `Match::resync` carry the names; `Match::draw` passes them to `Scene::names`;
  - the Rust-only F5 restart runs the NEW GAME gate and ignores a bound F5 (RD-6).
- `render`: `Scene::names: [&str; 2]` (the kill banners; empty everywhere a golden is drawn), `menu::menu_palette_with` (the slot-0 override).
- `scenario`: `storage::placeable_leaf(subdir, leaf)`; the browser store gains the eight shipped profiles.
- `oracle_dump_shell`: tops `K`/`F`, `cur` letters `1`/`2`/`N` with the player menu's `ssel`, the `profiles <user|sys>` manifest line, the key name `APPLICATION`, `F` in the search-gap check, a non-`fs` profile guard. No new intervention (RD-8).
- `game`: the shell's live sampler is `DosHeld` + the touch word; the phone buttons' fixed menu keys (Q6); hooks `lieroNames` / `lieroKeys`; the page's help and hints.

**Tech stack.**
- Rust 2021 for `sim-core`, `sim`, `render`, `scenario`, `shot` and `oracle-tests`; Rust 2024 for `ui` and `game`.
- C++ in `src/tools/oracle_dump/shell_dump.cpp` only, preset `linux-x64`, clang-format 22.
- HTML/JS in `web/index.html`; the preview comment in `.github/workflows/preview.yml`.

**Spec:** `docs/superpowers/specs/2026-09-27-liero-rs-step4.5-slice4.5f-player-menu-ai-design.md`, cited as **design §N** / **finding N**. Its section **"4½f-2 refresh (2026-09-28, after 4½f-1 landed)"** wins over §3–§8 wherever they disagree; its findings are cited **R-n**, its re-verified C++ facts **R2-n**, its decisions **RD-n**. The precedent plans are `plans/2026-09-27-liero-rs-step4.5-slice4.5f1-plan.md` (**f-1 plan**; its addenda **f-1 Addendum T0 / T9**), `plans/2026-09-26-liero-rs-step4.5-slice4.5e2-plan.md` (**e-2 plan**) and `plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md` (**e-1 plan**; the shell gate formats are its "Formats pinned").

**Rulings (John).** Every recommendation was accepted:
- **2026-09-27:** Q1 = A (two parts; this is part 2), Q2 = A (an "AI" player gets the `AI PLAYERS ARE NOT SUPPORTED YET` box at NEW GAME — landed in f-1), Q3 = A (the phone's player 2 is the CPU — landed in f-1; this part adds the RIGHT PLAYER menu that shows and changes it), **Q4 = A** (fix the DIG overflow: DIG gets the key, WEAPON 1 is unchanged; an intended divergence only where C++ is UB), **Q5 = A** (Joystick profiles as in C++: INPUT shows the gamepad, the keyboard no longer moves that player, Enter on INPUT switches back to Keyboard). And f-1 Addendum T9: `?cpu=` keeps the setup's BOT WEAPONS.
- **2026-09-28:** **Q6 = A** (the phone buttons always work), **Q7 = A** (a profile loaded into RIGHT PLAYER on a phone keeps player 2 the CPU). Both are encoded precisely in D9 and D10.

**Base.** The slice base is **`d2de489`**, the merge of 4½f-1 (PR #18) into `liero-rs-step-4-5`. It is used for every golden audit, every clang-tidy diff and every review diff. HEAD at plan time is `7c83033` (the design refresh and the Q6/Q7 rulings).

---

## Working environment (read before any task)

- **Repo and branch.** `/home/user/openliero` on `claude/cpp-oracle-vcpkg-assets-chcwcm` (HEAD `7c83033` at plan time). Use absolute paths. No sub-subagents.
- **Scratchpad.** `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad`. Every scratch script, PPM, PNG, probe, fixture copy and log goes here, never into the repo. This slice's scratch dirs are `$S/t0f2/` (T0) and `$S/f2b<N>/` (Batch N); the browser bundle stays `$S/site`.
- **The container has restarted before.** Keep every long command (C++ builds, gen scripts, `cargo test`, the wasm build, the Chromium walks, Xvfb) in the **background** with its output in a log under `$S`, save its PID (`echo $! > $S/f2b<N>/<job>.pid`), and poll the log. Never rely on a foreground command surviving more than a few minutes. **Never `pkill -f`** (the pattern matches the polling shell and the tool's own wrapper): stop a job with `kill "$(cat <pidfile>)"`.
- **C++ oracle build.**
  - In the same shell as every cmake or gen-script call, first `source $S/env.sh` (it sets `PRESET=linux-x64`, `VCPKG_ROOT`, the vcpkg asset script and the sdl3 overlay).
  - The build dir is `build/linux-x64` (Ninja Multi-Config, configured with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON` and `CMAKE_EXPORT_COMPILE_COMMANDS`); binaries in `build/linux-x64/Release/`. Build one with `cmake --build build/linux-x64 --config Release --target oracle_dump_shell`.
  - The gen scripts default to `macos-arm64` and honour `PRESET`: always `source $S/env.sh && bash rust/oracle-tests/<script>.sh` from the repo root.
- **The checked C++ build** (`-D_GLIBCXX_ASSERTIONS -fsanitize=address`, single-config Ninja) is `$S/build-chk`. Rebuild with `source $S/env.sh && cmake --build $S/build-chk --target oracle_dump_shell`. `$S/b7chk/run.sh` is the shell loop template (every `shell_*_script.txt`, `ASAN_OPTIONS=detect_leaks=0`, `cmp` against the golden): **copy it to `$S/f2b<N>/chk.sh` and change its output dir**, never edit or run the template in place.
- **clang-format is pinned to 22.** The system `clang-format` is 18 and must never be used.
  - `CF22=$(uvx --from clang-format==22.1.0 sh -c 'command -v clang-format')`.
  - Whole file (required): `"$CF22" --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/shell_dump.cpp`. Diff: `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh d2de489`. Fix: `"$CF22" -i --style=file <abs file>`.
- **clang-tidy.** `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 d2de489`. This clone has no `origin/master`, so always pass the base. It must exit 0. A `NOLINTNEXTLINE(<check>) — <reason>` only where the repo already uses one for the same check.
- **Rust.** From the repo root, **DEBUG only** (never `--release`: some tests are `#[should_panic]` on `debug_assert!`s).
  - **The full re-diff is both** `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game` **and** `cargo test --manifest-path rust/Cargo.toml -p game`. **Never** `cargo test --workspace` including `game` (disk).
  - **The four frozen generator checks:** `cargo run --manifest-path rust/Cargo.toml -p oracle-tests --example <g> -- check` for `gen_slice4_5d`, `gen_slice4_5e1_shell`, `gen_slice4_5e2_shell`, `gen_slice4_5f1_shell` (a fifth, `gen_slice4_5f2_shell`, from T7 on).
  - The wasm check: `cargo build --manifest-path rust/Cargo.toml --profile wasm-release -p game --target wasm32-unknown-unknown`. Never `--target-dir` or a second target tree.
  - `cargo tree --manifest-path rust/Cargo.toml -p ui -e normal | grep -c bevy` must print `0`.
- **Disk is tight** (about 15 GB free at plan time). After every batch: `rm -rf rust/target/debug/incremental rust/target/wasm32-unknown-unknown/*/incremental` and delete the batch's PPM/PNG dirs. Before a wasm-release build `df -h /` must show ≥ 3 GB free. **Rust batches run sequentially in this one checkout**; the C++-only Batch 3 may run alongside a Rust batch (its files are disjoint, and `build/linux-x64` is not `rust/target`).
- **The native Rust `game` cannot run here** (Bevy: "Unable to find a GPU" under Xvfb). The browser bundle stands in:
  - build: the wasm check, then `wasm-bindgen --target web --out-dir $S/site --out-name game --remove-name-section --remove-producers-section rust/target/wasm32-unknown-unknown/wasm-release/game.wasm && cp web/index.html $S/site/ && rm -f $S/site/*.d.ts`;
  - serve: `cd $S/site && python3 -m http.server 8765` in the background; `curl -sI http://localhost:8765/`, restart it if down;
  - Playwright under `/opt/node22/lib/node_modules/`; templates `$S/b8.mjs` (desktop keys through `frameTap`, an emulated phone, the `lieroPhase`/`lieroTop`/`lieroSel` hooks), `$S/e2.mjs` (the e-2 phone walk), `$S/tap1.mjs` / `$S/deathdiag2.mjs` (real CDP touch). **Copy, never edit.**
- **The real C++ game under Xvfb** (T0 cross-checks and T9's eyeball artefacts; never gates): `Xvfb :99 -screen 0 1280x800x24 &`, then `DISPLAY=:99 SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy OPENLIERO_DATADIR=data build/linux-x64/Release/openliero --config-root <dir> &`; drive with `xdotool` (`$S/xd.sh` has `tap`/`snap`; `$S/e2x/drive_cpp.sh`); screenshot with `import -window <id>`; pair with `$S/x12_pair.py`; convert PPMs with `$S/ppm2png.py`.
- **Golden rule.** New goldens come **only from the real C++**. Never regenerate a golden to make Rust pass; never hand-edit one. Every existing golden stays byte-identical, audited against `d2de489`. If `git status --porcelain -- rust/oracle-tests/golden` ever shows an `M`: **(1)** stop; **(2)** `git checkout -- rust/oracle-tests/golden`; **(3)** report the file and its first differing line; **(4)** do not commit.
- **`data/` never changes.** `git status --short data` must be empty after every batch (a profile saved by a non-`fs` C++ run would land in `data/TC/openliero/Profiles`, pitfall 27).
- **Commits.** The globally configured identity. **Explicit paths only** (never `git add -A` / `git add .`: the C++ batch shares the checkout). Every commit carries exactly these two trailer paragraphs, and no other text names a model:
  ```
  CO='Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>'
  SESS='Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT'
  git -C /home/user/openliero commit -m "<subject>" -m "$CO" -m "$SESS"
  ```
  Never "Generated with …". **Do NOT push and do NOT open a PR.** The orchestrator pushes after each verified batch.

## Plan-time facts checked against the source (the source wins)

Written against HEAD `7c83033`. They sharpen the refresh; T0 re-checks the ones marked **(T0)** with the real C++. T9 records each in PROGRESS and the maps.

**The menu (C++).**
1. **The Enter arms** (`mainMenuState.cpp:316-429`) are as R2-12 says, with one detail the refresh leaves implicit: NAME, SAVE PROFILE AS… and WEAPON n push only when `ItemPosition` finds the item in view (`:327-331`, `:352-355`, `:394-397`), while the eight key rows push `WaitForKeyState` unconditionally (`:367-389`). The cursor's own item is always in view, so this never differs in practice; Rust keeps the same guard.
2. **SAVE PROFILE is not intercepted.** It reaches the `else` arm (`selected_ = cur_menu->OnEnter`, `:424-426`), and `ProfileSaveBehavior(false)::OnEnter` plays `MenuSelect`, writes `user/Profiles/<leaf>` and runs `menu.UpdateItems` (`gfx.cpp:203-217`). Rust behaviours have no store, so Rust intercepts it in `MainMenuState::update` with the same order (sound, write, `update_items`) (D6).
3. **The F keys run in this order** (`mainMenuState.cpp:432-470`): F1, F2, F3, **F5, F6**, F7, **F9**, F8. Rust today consumes F2, F3, F5, F6 in one loop, then F7, then F9 and F8 (`rust/ui/src/shell/main_menu.rs:347-366`). T3 restores the C++ order.
4. **The draw** (`mainMenuState.cpp:614-626`): with main focus the **settings** menu is drawn disabled; with any other focus only `cur_menu` is drawn, enabled. Rust always draws the settings menu (`main_menu.rs:418-430`); T3 switches on `CurMenu`.
5. **`UpdateMenuPalettes`** (`gfx.cpp:978-1005`) runs `SetWormColours(*settings)` and then, when `cur_menu == &player_menu` and `player_menu.ws == settings->worm_settings[2]`, `SetWormColour(0, *player_menu.ws)`. `Palette::SetWormColour` copies `entries[base + (j % 3) - 1]` (`gfx/palette.cpp:92-105`), so the override is **an extra call after both players**, not a reordering: Rust gains `render::menu::menu_palette_with(origpal, cycles, worm_rgb, slot0: Option<[i32; 3]>)` and `menu_palette` delegates (T3).
6. **The network player's defaults are player 1's keys and colour** (`settings.cpp:34`, `:52-59`). So F9's slot-0 colour is visible only after an R/G/B edit in the NETWORK PLAYER menu; `player_nav` edits it (design §6.7 assumed a visible difference at once).
7. **`InputDeviceBehavior`** (`gfx.cpp:85-201`): with no pads `BuildOptions()` is empty, so `OnUpdate` shows `"Keyboard"` or `gamepad_name.empty() ? "Gamepad (none)" : gamepad_name.substr(0, 20)` (**bytes**), and `Cycle` always lands on Keyboard (clearing name and serial) then `menu.UpdateItems`. `OnLeftRight` plays `MenuMoveUp` for `dir > 0`, `MenuMoveDown` for `dir < 0`, then cycles and returns false; `OnEnter` plays `MenuSelect`, cycles `+1`, returns −1.
8. **`WaitForKeyState`** (`inputState.cpp:100-162`): `HandleEvent` calls `ProcessEvent` first, then every `SDL_EVENT_KEY_DOWN` (repeats too) sets `result_ = SDLToDOSKey(sc)`, `done_` (the `extended_ || !IsExtendedKey` test is always true for the player menu, `Settings::kExtensions == true`, `settings.hpp:14`). `Update`: `ClearKeys`, the callback, pop — no sound. `Draw`: `DrawRoundedBox(cx, cy, 0, h + 1, w + 1)` and `"PRESS A KEY"` in colour 50 at `(cx + 2, cy + 2)` with `cx = 160 - w/2 - 2`, `cy = 100 - h/2 - 2`. Not an overlay (R-3: the box shows on the push frame).
9. **`ProfileSelectorState`** (`fileSelectorState.cpp:186-204`) binds `ws_` **at push time** (the player menu's current `ws`), fills the whole config root with `CiCompare(ext, "TOML")`, opens inside `JoinPath(root, "Profiles")`, and on a pick runs `ws_.LoadProfile(node)` + `player_menu.UpdateItems` (no `MoveToFirstVisible`).
10. **`MakeSaveAsState`** (`mainMenuState.cpp:69-91`): a cancel or empty result calls `on_complete("")`; a shadowing name schedules the black `NAME '<leaf>' IS RESERVED` box and **returns without `on_complete`**. So the SAVE PROFILE AS… closing frame plays **two** `MenuSelect`s when it saves or is cancelled (the `InputStringState`'s own, `inputState.cpp:78`, and `on_complete`'s, `mainMenuState.cpp:362`), but **one** when the name is reserved. (The refresh's P3 row reads "2" for both; this plan pins 2 / 2 / **1**, as e-2's setup box already does.)

**The live input (C++ and Rust).**
11. **`LocalController::OnKey`** (`localController.cpp:58-87`) and **`Game::FindControlForKey`** (`game.cpp:75-106`) are as finding 8 / R2-22 say. `FindControlForKey` walks `game.worms` — **two** worms; the network player never takes a key in play (only the menus' `TestControl*` walk all three settings, `gfx.cpp:869-883`, `:954-968`).
12. **C++ weapon selection's key repeat reads clean bits 0..6 only** (`localController.cpp:124-146`): DIG (bit 7) is never repeated, and a DIG press reaches the selection only through `OnKey`'s `Press(kLeft)`/`Press(kRight)`. Rust's selection keeps its 4½c level model with DIG folded into Left+Right (as `words()` does today), which is **not proven for DIG**: no gated case holds a DIG key on a `W` frame (a validator, pitfall 17).
13. **Two non-shell paths also use the 7-bit edge and latch code**: the native `--live <scenario>` loop (`rust/game/src/main.rs:144`, `:955-974`, `:998-1045`). Widening `KeyEdges` / `ReleaseLatch` in place would touch them; the plan adds 8-bit siblings instead (D1), so `--live`, `--replay`, Scripted and `?demo` stay byte-identical.
14. **The `ui` tests feed words, not keys**: `step(sh, sim, events, words)` has 94 call sites in `rust/ui/src/shell/mod.rs` (helpers at `:1197-1214`). T2 keeps them by mapping each word bit to the **default** binding's DOS key in the helper (D1).
15. **`ShellInput` is also built in `game`** (`rust/game/src/main.rs:1266-1272`; tests at `touch.rs:731-737`, `config.rs:489-495`, `web_params.rs:305-325`), always with `..ShellInput::idle()` in the tests.

**The oracle.**
16. **Key names in scripts.** `ScancodeOf` (`src/tools/oracle_dump/shell_dump.cpp:233-282`) and `key_of` (`rust/oracle-tests/tests/shell_common/mod.rs:153-194`) already know `TAB` and `F1`…`F12`; only `APPLICATION` is new on both sides. `ALLOWED` (`:63-127`) lacks `TAB`, `F5`, `F6`, `F9` and gains them with `APPLICATION` (R-7).
17. **A non-`fs` case has no config nodes** (`shell_dump.cpp:669-681`) and runs with CWD `data/TC/openliero` (`:752`). A LOAD PROFILE there would list nothing sensible, and a SAVE PROFILE / SAVE PROFILE AS… would write under the CWD (the repo's `data/`). So the profile rows are reachable only in `fs` cases (D12).
18. **The fixture's system layer has an empty `Profiles`** (`Fs::install()`, `shell_common/mod.rs:871-874`). The eight shipped profiles need the `profiles <layer>` manifest line, because six of them have a space in their name and both manifest parsers split on whitespace (`shell_dump.cpp:535-547`, `shell_common/mod.rs:796-801`), as does `gen_shell_golden.sh`'s `file`-line gate (`NF != 3`, R-5).
19. **The shipped profiles** (`data/Profiles/*.toml`, 8 files, no `rgbDepth`): every one says `color = 0` (LOAD PROFILE restores the colour index, finding 15); `Joystick0/1` say `inputDevice = 1`, `gamepadName = ""` (→ `Gamepad (none)`) and `gamepadControls = [11, 12, 13, 14, 110, 10, 0, 9]`; `AI (L)/(R)` say `controller = 2`; `Lefty (L)` binds W/S/A/D/Y/U/I (`controlsEx = [17, 31, 30, 32, 21, 22, 23, 0]`) with `rgb = [40, 10, 55]` (legacy → ×4).
20. **This container runs as root**, so an unreadable `.toml` cannot be made in a fixture (root reads a `chmod 000` file). R2-18's "unreadable" shape is unit-tested through a store that returns `None`, never gated.

**Names (C++ and Rust).**
21. **Banners** (`viewport.cpp:256-270`): `LS(KilledMsg) + other_worm.settings->name` and `other_worm.settings->name + LS(CommittedSuicideMsg)`, indexed by the **dead** worm. Rust draws the messages alone (`rust/render/src/frame.rs:120-150`). `Scene` is built by literal at seven sites (`render/src/frame.rs:283`, `:392`, `:408`; `scenario/src/loader.rs:75`; `oracle-tests/tests/render_slice3a_golden.rs:195`, `render_slice3b_common/mod.rs:164`, `render_slice3e_common/mod.rs:171`, `render_slice4d_common/mod.rs:201`), each gaining `names: ["", ""]` mechanically (T5).
22. **Weapon selection's names** (`weapsel.cpp:199-203`) are read live every frame. `Selection::names` is `Default` (`rust/ui/src/shell/selection.rs:97-112`) and `Match::resync` refreshes only `cfg`, `hud` and `weap_table` (`playing.rs:300-306`) — R-10.

## Decisions this plan makes (engineering; the design and the refresh left them open)

- **D1. The input seam, concretely** (RD-1, RD-2; facts 11–15).
  ```rust
  // ui::keys
  pub struct DosHeld { down: [bool; MAX_DOS_KEY as usize] }   // Default = nothing held
  impl DosHeld { pub fn set(&mut self, dos: u32, down: bool); pub fn is_down(&self, dos: u32) -> bool;
                 pub fn from_keys(keys: impl IntoIterator<Item = u32>) -> DosHeld; }
  pub const CLEAN_DIG: u8 = 1 << 7;          // bits 0..6 = ControlState's, bit 7 = DIG (controls_ex order)
  /// FindControlForKey over every held key: worms 0 and 1 only, keyboard players only
  /// (`input_device == 0`), the first control 0..8 whose `controls_ex` equals the key.
  pub fn clean_words(held: &DosHeld, ws: &[WormSettings]) -> [u8; 2];
  /// Weapon selection's word: bits 0..6, DIG pressing Left + Right (today's `words()`).
  pub fn fold_dig(w: u8) -> ControlState;
  /// OnKey's edges on 8-bit words: bits 0..6 as `apply_key_edges`; then, if any of the 8
  /// bits changed: DIG held → press Left and Right, else release Left / Right unless held.
  pub fn apply_clean_edges(prev: u8, now: u8, current: ControlState) -> ControlState;
  pub struct CleanEdges { prev: [u8; 2] }      // `apply(&mut self, now: &[u8; 2], worms) -> [ControlState; 2]`
  impl ReleaseLatch { pub fn arm_clean(&mut self, held: &[u8; 2]); pub fn apply_clean(&mut self, w: &mut [u8; 2]); }
  ```
  - `apply_key_edges`, `KeyEdges`, `ReleaseLatch::{arm, apply}` are unchanged (fact 13). A unit test proves `apply_clean_edges(p, n, c) == apply_key_edges(p, n, c)` for **every** `p, n < 128` and a set of `c` (DIG never set), so the shell's move to the new code is behaviour-identical for every prior case.
  - `ShellInput { events, held: &'a DosHeld, touch: ControlState, fresh_seed, now_ms, restart }`; `ShellInput::idle()` uses a `static` empty `DosHeld`.
  - `Shell::words(&self, held, touch, settings) -> [u8; 2]` = `clean_words(held, settings)` (with `ShellDebug::live_bindings == false`, `Settings::default()`'s bindings instead — T7's negative control), then `w[0] |= touch.pack() as u8` (the Rust-only phone overlay, OR-ed into player 1 **whatever its input device**, Q6). It is evaluated where each consumer needs it: for `Match::process` (the `Playing` update) over the running match's copy `Match::settings()` (RD-2); for NEW GAME's latch over the **new** match's settings (the menu's, just cloned into it); for RESUME's latch over `Match::settings()` **after** `resync` (attached) — or the old copy (detached).
  - `Match::process(sim, words: [u8; 2], ais, sounds)`: `latch.apply_clean(&mut w)`; in selection `sel.step(sim, &w.map(fold_dig), sounds)` (and on the finalise frame `latch.arm_clean(&words)`); in a match `CleanEdges::apply` → `run_ais_traced` → `tick_viewports`.
  - The harness passes its `BTreeSet` as `DosHeld::from_keys(..)`. The `ui` test helper `step_ev(.., words)` maps each word bit `c` of worm `i` to `Settings::default().worm_settings[i].controls_ex[c]` (fact 14), so its 94 call sites stay; the rebinding tests pass explicit held keys.
  - `game` (T2, mechanical): the shell tick builds `DosHeld` from `ButtonInput<KeyCode>::get_pressed()` through `dos_of_keycode`, and `touch = touch_state(dig_repeat(weapon_tap(page_touch())))` (0 natively). `sample_inputs` / `sample_inputs_touch` serve only the non-shell paths.
- **D2. The player menu is one `Menu`, re-pointed** (C++ `player_menu.ws`). `MenuWorld::player_menu: Menu` (built by `player_menu()` at (178, 20), `value_offset_x` 95, the 24 rows of R2-1), `CurMenu::Player(p)` names which `worm_settings[p]` it edits. `Shell::player_settings(p)` = `PlayerSettings` (`gfx.cpp:1430-1437`): `update_items(PlayerMenuModel for p)`, `move_to_first_visible`, `cur_menu = Player(p)`.
  ```rust
  pub struct PlayerMenuModel<'a> { pub ws: &'a mut WormSettings, pub profile: Option<&'a ProfileRef>, pub tc: &'a UiTc }
  pub const PL_NAME: i32 = 0; /* … */ pub const PL_UP: i32 = 6; pub const PL_DIG: i32 = 13;
  pub const PL_WEAP0: i32 = 14; pub const PL_CONTROLLER: i32 = 19; pub const PL_SAVE_PROFILE: i32 = 20;
  pub const PL_SAVE_PROFILE_AS: i32 = 21; pub const PL_LOAD_PROFILE: i32 = 22; pub const PL_LOADED_PROFILE: i32 = 23;
  ```
  Behaviours (`MenuModel::behavior`, the §3.1 table): NAME and PROFILE LOADED and the key rows are `Custom` (`on_update` sets `value`, `has_value`, and for PROFILE LOADED / SAVE PROFILE `set_visibility`); HEALTH `Integer(health, 1, 10000, 1, %)` with `scroll_interval` 4; R/G/B `Integer(rgb[c], 0, 252, 4)` with `display_div` 4, `scroll_interval` 4 (the classic picker only, design decision 10); INPUT `Custom` (fact 7); WEAPON n `Enum { v, 1, 40, broken: false }` whose `on_update` shows `tc.weapon_names[v - 1]` (empty when `v` is 0 or > 40: C++ UB, R2-7); CONTROLLER `ArrayEnum { CONTROLLERS }` (a value ≥ 3 shows empty, R2-8). `draw_item_overlay` draws the bars (R2-9).
- **D3. `WaitForKeyState`** (fact 8): `Screen::WaitForKey(WaitForKeyState { target: KeyTarget { player, control }, result: Option<u32> })`, top `K`, `Phase::Menu` (RD-7), not an overlay, `wants_menu_flip`. `handle_key(ev)`: every `down` event (repeats too) sets `result = Some(ev.dos)` (the last one of the frame wins). The frame's update: `Some(k)` → `Closing::Key(target, k)` and pop; `close`: `keys.clear()`, then — only if `k != DK_ESCAPE` — `if control < 7 { ws.controls[control] = k }`, `ws.controls_ex[control] = k` (**Q4 = A**: never `controls[7]`), `player_menu.update_items`. No sound on the close frame.
- **D4. Overlays and continuations.** `InputPurpose` becomes:
  ```rust
  pub enum EntryTarget { Settings, Player(usize) }
  pub enum SaveAsKind { Setup, Profile(usize) }
  pub enum InputPurpose {
      IntegerEntry { entry: ValueEntry, target: EntryTarget },   // R-8
      SaveAs { kind: SaveAsKind, x: i32, y: i32 },               // RD-5; was SaveSetupAs
      WormName { player: usize },
      WeaponFuzzy { player: usize, slot: usize },
  }
  // InfoPurpose::Reserved { kind: SaveAsKind, typed: Vec<u8>, x: i32, y: i32 }
  ```
  | Purpose | Push (after `MenuSelect`) | Close (after the `InputStringState`'s own `MenuSelect` + `keys.clear()`) |
  |---|---|---|
  | `WormName { p }` | `InputString(name bytes, 20, x + 95 + 2, y, no filter)` | accepted → `name = dos_to_text(buf)`; (empty → `GenerateName`, a no-op, finding 4); `random_name = false`; `MenuSelect`; `update_items` |
  | `WeaponFuzzy { p, slot }` | `InputString("", 10, x + 97, y)` | accepted and non-empty → `weapons[slot] = weapon_fuzzy_match(..)`; `update_items`; no sound |
  | `SaveAs { Profile(p) }` | `InputString("", 30, x + 97, y)` (`save_as_box(kind, b"", x, y)`) | as SAVE SETUP AS… with `Profiles` / `.toml` (fact 10): reserved (`shadows_system("Profiles", leaf) \|\| !placeable_leaf("Profiles", leaf)`) → the black box, **no** completion; else write `Profiles/<leaf>` (`worm_settings_to_toml`), on `Ok` `profiles[p] = Some(ProfileRef { rel })`, on `Err` a note; then `MenuSelect` + `player_menu.update_items` |
  | `IntegerEntry { Player(p) }` | as 4½e (`filter_digits`) | as 4½e into `ws.health` / `ws.rgb[c]` (stored ×`div`), the player menu item's value rewritten, no `update_items` |
  The reserved box's dismissal reopens `save_as_box(kind, typed, x, y)` for the **same kind** (a profile box for the same player).
- **D5. Profiles.** `pub struct ProfileRef { pub rel: String }` (a config-relative path such as `Profiles/Lefty (L).toml`); PROFILE LOADED shows `leaf_basename(rel)`. `MenuWorld::profiles: [Option<ProfileRef>; 3]`, `None` at boot, **cleared by LOAD SETUP** (fresh `WormSettings` objects), set by LOAD PROFILE on anything that reads (even a failed parse, R2-18), kept on an unreadable file (`store.read == None`, a note), set by SAVE PROFILE / SAVE PROFILE AS… after a successful write. `Screen::ProfileSelect(ProfileSelectorState { selector, player })` in `files.rs` (top `F`, `toml_filter`, title `Select profile:`, `Phase::Menu`); `Picked::Profile { player, rel }` → `apply_picked`: `profiles[p] = Some(rel)` iff the read returned bytes; then `load_profile(text, ws)` (a UTF-8 or TOML error keeps every field — `load_profile` already restores `color`, `settings_toml.rs:236-244`); then **Q7** (D10); then `player_menu.update_items` (no `move_to_first_visible`).
- **D6. SAVE PROFILE is intercepted** (fact 2): in the player arm of `MainMenuState::update`, `PL_SAVE_PROFILE` → `MenuSelect`, write `Profiles/<leaf(rel)>` to the store (the user layer, no shadow check, R2-19), on `Ok` `profiles[p] = Some(Profiles/<leaf>)`, `update_items`.
- **D7. `storage::placeable_leaf(subdir: &str, leaf: &str)`** (RD-5): the e-2 rule with `format!("{subdir}/{leaf}")`. Both callers (`Shell` and the harness's SAVE-AS validator) pass the subdir. `storage.rs`'s unit test keeps every e-2 vector under `Setups` and adds `Profiles` ones.
- **D8. The F5 restart gate** (RD-6). In `Shell::frame`, `input.restart` with `Playing` on top acts only if (a) no keyboard player 0/1 of `Match::settings()` binds `DK_F5` in `controls_ex`, and (b) `self.gate().refusal(&self.world.settings, MA_NEW_GAME)` is `None`; otherwise nothing happens and `FrameOut::notes` gets one line (`F5 restart ignored: <reason>`). The harness never sets `restart`, so no gate can see it.
- **D9. Q6, the phone buttons, precisely.** `game::touch::TouchKeys::tick(now, phase)` no longer reads `controls_ex`; every press sends a fixed DOS key and its release sends the key the press sent, whatever the phase has become:

  | Button | `Phase::Menu` (tops `M O B L P F K`) | `Phase::Text` (top `I`) | `Phase::Weapsel` / `Phase::Game` |
  |---|---|---|---|
  | pad ↑ ↓ ← → | `DK_UP` / `DK_DOWN` / `DK_LEFT` / `DK_RIGHT` (↑/↓ keep the 12 + 3 tick menu repeat) | the same arrows (an `InputStringState` ignores them; `ClearKeys` drops them at the close) | no key event — the pad is player 1's word (`ShellInput::touch`) |
  | FIRE | `DK_RETURN` | `DK_RETURN` (confirm) | player 1's word |
  | JUMP | `DK_ESCAPE` (C++ menus treat Esc and Jump alike, `mainMenuState.cpp:171-173`, `menu/fileSelector.hpp:279-280`, `weaponMenuState.cpp:91-92`) | nothing | player 1's word |
  | CHANGE, DIG | nothing | nothing | player 1's word (DIG: the Left+Right chord, `DigRepeat`, unchanged) |
  | MENU | `DK_ESCAPE` | `DK_ESCAPE` (cancel) | `DK_ESCAPE` (pause) |

  Consequences, each as John ruled: every C++ menu accepts the arrows, Return and Esc exactly as player 1's keys (`TestSdlKey* || TestControl*`), so the menus behave as before for default keys and keep working after any rebind or a Joystick profile; in the `PRESS A KEY` box the pad binds `DK_UP`/`DK_DOWN`/`DK_LEFT`/`DK_RIGHT`, FIRE binds `DK_RETURN`, JUMP and MENU cancel (Esc), CHANGE and DIG do nothing; in a match the pad drives player 1 directly through `ShellInput::touch` even when player 1 is a pad player (Rust-only; C++ has no touch). The eight key rows keep showing and changing player 1's **keyboard** keys. The mapping is the same on every touch device (the overlay is phone UI); no gate sees it.
- **D10. Q7, precisely.** After a LOAD PROFILE pick for **player index 1** on a touch-only page (`options.touch_only`), `selection::touch_settings(&mut settings)` runs (player 2's `controller = 1`), whatever the file said (Human, CPU or "AI") and whether or not it parsed. Name, health, colour, weapons, keys, input device load as in C++. CONTROLLER stays editable by hand. Player 1 and the network player are untouched; a desktop never runs it. A running match is unaffected either way (CONTROLLER does not reach it, finding 7).
- **D11. Names.** `Selection::new(cfg, names: [String; 2])` from the match settings' `worm_settings[0..2].name`; `Selection::set_names` called by `Match::resync` (R-10); `Match::draw` passes `names: [&cfg.settings.worm_settings[0].name, &…[1].name]` to `Scene::names`; `frame::draw` concatenates `killed_msg + names[dead]` / `names[dead] + committed_suicide_msg` (fact 21). `SceneData::as_scene` and every other literal pass `["", ""]`.
- **D12. Profile rows only in `fs` cases** (fact 17). The generator refuses an Enter on PL_SAVE_PROFILE, PL_SAVE_PROFILE_AS or PL_LOAD_PROFILE in a case without `fs`; the dumper fails a non-`fs` case whose top becomes `F`, and fails a non-`fs` case after any frame on which `std::filesystem::exists("Profiles")` holds under its CWD.
- **D13. `profiles <user|sys>`** (design §6.5, R-5): a manifest line that copies every `data/Profiles/*.toml` (bytewise-sorted, 8 files) into `<layer>/Profiles/<same name>`; refused if a destination exists or the layer is given twice. Rust: `make_fixture_with` handles it and `Fs::profiles(layer)` appends it. `install_coverage` is untouched (the profiles are filtered by `LEV` and `CFG`).
- **D14. `profile_io` also gates R2-18's parse-failure shape** with one extra input golden, `shell_profile_io_user_broken.toml` (a TOML syntax error), installed as `user/Profiles/broken.toml`: PROFILE LOADED `broken`, no field changes. The unreadable shape is unit-tested only (fact 20).
- **D15. The milestone keeps the default BOT WEAPONS (PICK)**: player 2 is readied in weapon selection with player 2's keys (the f-1 `cpu_pick` precedent), so `shell_player_setup` needs no user `liero.cfg`.
- **D16. G2f-2's golden set is 22 files** (§Formats); a batch that needs another input file adds it to §File structure in its own commit and reports the new count.

## Standing ruling (4½c Addendum A, carried forward)

The C++ comparison happens **here**, against the real C++ run headlessly. If a dumper cannot run the real code headlessly, the task **stops and reports the exact blocker**. It never weakens a gate and never regenerates a golden to paper over a mismatch. The real `openliero` under Xvfb produces C++ | Rust side-by-side PNGs; those are eyeball artefacts, never gates, and never committed.

## Global constraints

- **LD 1:** pixel-exact menus on the 320×200 CPU surface.
- **LD 3:** `tick_and_render` stays the only `ResMut<Sim>` holder; menus get no `SimState`. The words are computed in `Shell` (it alone holds the settings a rebind changes, R-1).
- **LD 4:** the scenario format is frozen; the shell script grammar gains no directive (only key names and the manifest line `profiles`).
- **Determinism firewall.** No floats (the fuzzy match is integer, finding 14), no wall clock and no `HashMap`/`HashSet` iteration in `ui` or `render`. `sim` is **unchanged** in this part.
- **Crate rules.** `ui`, `render`, `scenario`, `shot` stay Bevy-free (`cargo tree … -p ui -e normal | grep -c bevy` → `0`); `sim-core` has no dependency; `sim` gains nothing.
- **`render` changes only** at `Scene::names` + the banner concatenation (`frame.rs`) and `menu::menu_palette_with` (additive). Every render golden is unchanged.
- **C++ changes are confined to `src/tools/oracle_dump/shell_dump.cpp`.** `CMakeLists.txt`, the game sources and every other dumper are unchanged.
- **Frozen provenance.** Never edit:
  - `examples/gen_slice4_5a.rs`, `gen_slice4_5c0.rs`, `gen_slice4_5c.rs`, `gen_slice4_5d.rs`, `gen_slice4_5e1_shell.rs`, `gen_slice4_5e1_sim.rs`, `gen_slice4_5e2_shell.rs`, `gen_slice4_5f1_shell.rs`, `gen_slice4_5f1_sim.rs`;
  - `tests/shell_e1_cases/`, `tests/shell_e2_cases/`, `tests/shell_f1_cases/`, `tests/weapsel_common/`, `tests/sim_slice4_5c0_common/`, `tests/sim_slice4_5f_common/`;
  - `menu_dump.cpp`, `weapsel_dump.cpp`, `weapsel_drive.hpp`, `sim_physics_dump.cpp`, `gen_menu_golden.sh`, `gen_sim_slice4_5e_golden.sh`, `gen_sim_slice4_5f_golden.sh`;
  - every `golden/*` that exists at `d2de489`.

  `tests/shell_common/mod.rs` and `tests/shell_golden.rs` change. So the four `-- check`s must stay clean, and all 32 committed scripts must still equal their generators.
- **Golden audit.** `git diff --name-status d2de489 -- rust/oracle-tests/golden` lists **only `A` lines**, exactly the **22 files** of §Formats.
- **rustfmt** only on files a task **creates** (`rustfmt --edition 2024 <abs file>` in `ui`/`game`, `--edition 2021` elsewhere); never on an existing file or a `lib.rs`/`main.rs` (it recurses). Hand-format edits.
- **The non-shell paths stay behaviour-identical:** `--live [<scenario>]`, `--live --record`, `--replay`, Scripted, `?demo`; `game/tests/round_trip.rs` and `record_regression.rs` stay green and untouched.

## Formats pinned (both sides implement exactly this)

**Tops** (`f`-line `upd` and `top`, `Shell::top_char`): `upd` ∈ `M W G O I B L P K F`; `top` ∈ `M G O I B L P K F -`. `K` = `WaitForKeyState` (Rust `Screen::WaitForKey`), `F` = `ProfileSelectorState` (Rust `Screen::ProfileSelect`). `ReplaySelectorState`, `TcSelectorState` and the hidden menu still `Fail` (F2, F3 stay refused). Line shapes are unchanged from the e-1 plan's §Formats.

**The `d` line** `d <frame> <cur> <ssel> <cfg16> <state8|->`: `cur` ∈ `M S 1 2 N` — `1`/`2`/`N` when `gfx.cur_menu == &gfx.player_menu` and `player_menu.ws == settings->worm_settings[0|1|2]` (none equal → `Fail`); `ssel` is `settings_menu.Selection()` for `M`/`S` (every existing `d` line is unchanged) and `player_menu.Selection()` for `1`/`2`/`N`. The `out` header string is byte-identical for every script.

**The manifest line** `profiles <user|sys>` (D13). Every other manifest line is unchanged.

**Key names:** `APPLICATION` → `SDL_SCANCODE_APPLICATION` (C++; `SDLToDOSKey` → 89, not in `liero_to_sdl_keys`) and `(89, TypedKey::Sym(0))` (Rust `key_of`). `ALLOWED` gains `TAB`, `F5`, `F6`, `F9`, `APPLICATION`; F2, F3, F8, F10, F11, F12 stay refused.

**The search-gap check** covers one continuous visit of an `O`, `L`, `P` or `F` state; `B` interludes continue an `O` visit only.

**The non-`fs` profile guard** (D12): `Fail("frame N: LOAD PROFILE needs an fs case")` on a top `F`; `Fail("frame N: a profile was written without an fs fixture")` when `Profiles` exists under the CWD.

**`gen_shell_golden.sh`:** `$3 ~ /^[MWGOIBLPKF]$/`, `$9 ~ /^[MGOIBLPKF-]$/`, the `d` line's `$3 ~ /^[MS12N]$/`; `EXPECTED_SHELL_CASES` defaults to **41** (4½d 11 + e-1 11 + e-2 6 + f-1 4 + f-2 9). Before T7 commits the f-2 scripts, every run uses `EXPECTED_SHELL_CASES=32`. The header comment names the f-2 additions.

**Golden names (22 files):**
- `shell_player_nav{_script.txt,.txt}`, `shell_player_edit{…}`, `shell_player_name{…}`, `shell_player_weapon{…}`, `shell_key_bind{…}`, `shell_dig{…}` — 12 (`setup default`, no `fs`);
- `shell_profile_io{_script.txt,.txt,_fs.txt}` + `shell_profile_io_user_broken.toml` — 4;
- `shell_player_live{_script.txt,.txt,_fs.txt}` — 3;
- `shell_player_setup{_script.txt,.txt,_fs.txt}` (🎯) — 3.

**The script header** (the f-1 shape): `# Step 4½f-2 G2f-2 case <name> (plan Task 7) — written by gen_slice4_5f2_shell.` then `# Rust ledger: …` (frames, end, NEW GAME/RESUME/back-to-menu counts, the f-2 ledger fields, file lines).

**Hooks (T8):** `window.lieroSel` = `1<n>` / `2<n>` / `N<n>` for the player menus (also while `K` is on top), `F<n>` for the profile selector (unchanged `M<n>`, `S<n>`, `L<n>`, `P<n>`); `window.lieroTop` gains `K`, `F`; new `window.lieroNames = [n0, n1]` and `window.lieroKeys = [[8 DOS codes], [8]]` (players 1 and 2's `controls_ex`), published with the other hooks.

## File structure

| File | Task | Responsibility |
|---|---|---|
| `src/tools/oracle_dump/shell_dump.cpp` (temporarily, restored) | T0 | the probe patch |
| `rust/ui/src/text.rs` (modify) | T1 | `KEY_NAMES`, `get_key_name`, `get_gamepad_key_name`, `CONTROLLERS`, `levenshtein`, `weapon_fuzzy_match` |
| `src/tools/oracle_dump/shell_dump.cpp`, `rust/oracle-tests/gen_shell_golden.sh` (modify) | T6 | tops `K`/`F`, `cur` letters + `ssel`, `profiles`, `APPLICATION`, the gap check, the non-`fs` guard, the header; regexes and the count |
| `rust/ui/src/keys.rs` (modify) | T2 | `DosHeld`, `clean_words`, `fold_dig`, `apply_clean_edges`, `CleanEdges`, the latch's clean API |
| `rust/ui/src/shell/{mod.rs, playing.rs}` (modify) | T2 | `ShellInput { held, touch }`, the per-frame words from the match's settings, `ShellDebug::live_bindings`, the restart gate (D8); the test helpers |
| `rust/game/src/main.rs` (modify) | T2 | the shell's `DosHeld` + touch word (mechanical) |
| `rust/oracle-tests/tests/shell_common/mod.rs` (modify) | T2, T4, T7 | `words()` dropped for `DosHeld`; `controls`/`p2_controls` per frame (T2); `placeable_leaf(subdir, ..)` (T4); f-2 validators, ledger, `Fs::profiles`, `APPLICATION`, `ALLOWED` (T7) |
| `rust/ui/src/shell/player_menu.rs` (create) | T3 | `player_menu()`, `PL_*`, `PlayerMenuModel`, behaviours, the bars |
| `rust/ui/src/shell/{mod.rs, main_menu.rs, overlay.rs, stack.rs, settings_menu.rs}` (modify) | T3 | `CurMenu::Player`, `MenuWorld::{player_menu, profiles}`, arms, F keys, draw, `WaitForKeyState`, purposes, the network palette |
| `rust/render/src/menu.rs` (modify) | T3 | `menu_palette_with` |
| `rust/ui/src/shell/{files.rs, mod.rs, main_menu.rs, overlay.rs}` (modify) | T4 | `ProfileSelectorState`, `Picked::Profile`, SAVE PROFILE / SAVE PROFILE AS… / LOAD PROFILE, refs, Q7 |
| `rust/scenario/src/{storage.rs, assets.rs}` (modify) | T4 | `placeable_leaf(subdir, leaf)`; `EMBEDDED_PROFILES` in `browser_system_files` |
| `rust/render/src/frame.rs`, `rust/scenario/src/loader.rs`, the four render test helpers of fact 21 (modify) | T5 | `Scene::names` + banners |
| `rust/ui/src/shell/{selection.rs, playing.rs}` (modify) | T5 | names in `Selection`, `resync`, `draw` |
| `rust/oracle-tests/tests/shell_f2_cases/mod.rs` (create) | T7 | the 9 cases, inputs, validators, witnesses |
| `rust/oracle-tests/examples/gen_slice4_5f2_shell.rs` (create) | T7 | `check` / `write` |
| `rust/oracle-tests/golden/shell_*` f-2 (create, **22**) | T7 | §Formats |
| `rust/oracle-tests/tests/shell_golden.rs` (modify) | T7 | the f-2 cases, the milestone, the negative control, the union of 41 |
| `rust/game/src/{main.rs, touch.rs}` (modify) | T8 | Q6's `TouchKeys`, hooks `lieroNames`/`lieroKeys`, `lieroSel` letters |
| `web/index.html`, `.github/workflows/preview.yml` (modify) | T8 | help lines (R-12), the `K` hint, the phone hint |
| PROGRESS, overview, design status, cpp-map, rust-map, `.claude/skills/liero-shot/SKILL.md` | T9 | status + corrections |

## Batches, order and parallelism

| Batch | Tasks | Needs | Runs alongside | Touches | Batch gate | Push gate (the orchestrator checks before pushing) |
|---|---|---|---|---|---|---|
| **1** | T0 | — | — (first, alone) | `shell_dump.cpp` temporarily; scratch; this plan (addendum) | every probe recorded; contradiction rules applied; restore proven | `git status --short src data rust` empty; one commit touching only this plan; the four `cmp`s of Step 7 `SAME` |
| **2** | T1 | 1 | **3** | `rust/ui/src/text.rs` | the full re-diff; the wasm check | golden status empty; `cargo tree` 0 bevy |
| **3** | T6 | 1 | **2, 4, 5, 6** (C++ + one shell script) | `shell_dump.cpp`, `gen_shell_golden.sh` | builds (release + checked); `$CF22` whole file + diff; clang-tidy; **the 32 shell goldens byte-identical under both builds**; smokes | `git diff --name-only d2de489 -- src CMakeLists.txt` = the one dumper; golden status empty |
| **4** | T2 | 2 | 3 | `rust/ui` (keys, shell), `rust/game/src/main.rs`, `shell_common` | the full re-diff (**all 32 shell cases**, `key_edges`, `record_regression`, `round_trip`); the wasm check; the four `-- check`s | golden status empty; `round_trip.rs`/`record_regression.rs` untouched |
| **5** | T3 | 4 | 3 | `rust/ui`, `rust/render/src/menu.rs` | the full re-diff; the wasm check; G1 + 32 G2 green; four `-- check`s | as Batch 4 |
| **6** | T4, T5 | 5 | 3 | `rust/ui`, `rust/scenario`, `rust/render/src/frame.rs`, the render test helpers, `shell_common` (one call) | the full re-diff after each task; the wasm check | as Batch 4; render goldens untouched |
| **7** | T7 | 3, 6 | — | `rust/oracle-tests` (shell); `rust/ui` / `rust/render` proven fixes only | **🎯 G2f-2 bit-exact** (every `f`/`d`/`file` line, all 9 cases) under **both** C++ builds; all 41 green; the negative control; five `-- check`s | the audit: 22 `A`, 0 `M`; `git status --short data` empty |
| **8** | T8 | 6 (API), 7 by schedule | — | `rust/game`, `web/index.html`, `.github/workflows/preview.yml` | `cargo test -p game`; the wasm check; the bundle + the Chromium walks | walk logs all `ok` |
| **9** | T9 | 7, 8 | — (last) | docs, skill | the full board, every audit, reproducibility, the broad review | `git status --short` lists only the docs |

**Schedule:** 1 → (2 ∥ 3) → 4 → 5 → 6 → 7 → 8 → 9. Batch 3 is C++-only and may keep running through Batches 4–6; it must be pushed before 7 starts. Batch 4 (T2) goes before the menu on purpose: it is the one change on every live player's path, and all 32 prior shell goldens plus `key_edges` must hold before the menu builds on it. Rust batches never overlap. Batch 8 needs only the APIs of T2–T4, so it may go before 7 if 7 is stuck in its fix loop; record the reorder.

**Design ↔ plan mapping (refresh R3):** T0 → T0 (P0 added: C++ vectors for T1); T1 → T1; T2 → T2 (+ D1's 8-bit siblings, D8); T3 → T3 (the three profile Enter arms move to T4); T4 → T4 (+ SAVE PROFILE AS…'s write, D6, D14); T5 → T5; T6 → T6 (+ the non-`fs` guard, D12); T7 → T7; T8 → T8 (Q6 = D9; Q7 = D10 lives in T4); T9 → T9.

---

### Task 0 (Batch 1): the C++ probes

The refresh's P1–P8 plus P0 (the vectors T1 needs), with the real `Gfx::RunOneFrame` in a temporarily patched `oracle_dump_shell`, in the working tree only. No probe adds code to the repo.

**Files:** `src/tools/oracle_dump/shell_dump.cpp` (patched, restored), `$S/t0f2/*`, this plan (the addendum).

- [ ] **Step 1: the probe patch** (save as `$S/t0f2/probe.diff`; build in `build/linux-x64` and `$S/build-chk`):
  - `TopOf`: `WaitForKeyState` → `K`, `ProfileSelectorState` → `F` (check `ProfileSelectorState` before the other selector casts).
  - `DetailLine`: `cur` `1`/`2`/`N` from `player_menu.ws == settings->worm_settings[i]` and `ssel` `player_menu.Selection()` (the §Formats rule, so T6 can reuse the probe's evidence).
  - A probe-only `x <frame>` line after each `d` line: for `i` in 0..3 `w<i> name=<hex bytes> hp=<health> rgb=<r,g,b> col=<color> in=<input_device> gp=<gamepad_name hex> ctl=<controller> keys=<controls[0..7]> ex=<controls_ex[0..8]> wp=<weapons[0..5]> rn=<random_name> prof=<profile_node FullPath or ->`, then `pal0=<%06x of play_renderer.pal entries at kWormColorBlocks[0].base..+3>`; and when a controller exists: per worm `clean=<%02x clean_control_states> cs=<%02x control_states.Pack()>`.
  - The `profiles <user|sys>` manifest line exactly as D13 (scratch; T6 lands its own).
  - `APPLICATION` in `ScancodeOf`.
  - A one-shot `PROBE_VECTORS=<file>` mode that writes, and exits: `Texts::key_names[i]` for `i` in 0..177 as hex bytes; `Gfx::GetKeyName(k)` for `k` ∈ {0, 1, 12, 13, 26, 27, 29, 41, 89, 176, 177, 300, 511, 512, 513, 543, 544, 1000}; `Gfx::GetGamepadKeyName(k)` for `k` ∈ {0…15, 99, 100…113, 114, 130}; `common.texts.controllers[0..3]`; `ptrdiff = (char*)ws.weapons - (char*)ws.controls` on a `WormSettings` (P1).
- [ ] **Step 2: the Levenshtein vectors.** `static int Levenshtein` is file-local in `mainMenuState.cpp`, so copy lines 26-53 **verbatim** into `$S/t0f2/lev.cpp` with a `main` that prints `lev(a, b)` for the pairs below, and prove the copy is verbatim with `diff <(sed -n 26,53p src/game/mainMenuState.cpp) <(sed -n '/^#define MIN3/,/^#undef MIN3/p' $S/t0f2/lev.cpp)`. Pairs: every TC weapon name (the 40 in `weap_order`, read from `data/TC/openliero/weapons/*.cfg`) against `bazoka`, `LSR`, `a`, `zzzzzzzzzz`, `BIG NUKE`, `big nuke`, `""`; plus (`kitten`,`sitting`)=3, (`ABC`,`abc`)=0, (`a`,`""`)=1.
- [ ] **Step 3: the fixtures** (`$S/t0f2/gen.py`; every script `boot_seed 7`, `detail`, ends by QUIT; taps: down at *t*, up at *t*+2; text one char per event):

  | Probe | Script(s) | Setup / fs | Path |
  |---|---|---|---|
  | **P0** fuzzy through the real menu | `v_fuzzy` | default | F5 → WEAPON 2 → Return → `bazoka` Return → again `LSR`, `a`, `zzzzzzzzzz`, `BIG NUKE`, and one tie string the Step-2 table predicts (two names at an equal ratio; record which) → an empty Return → Esc → QUIT; the `x` lines give `wp` after each |
  | **P1** DIG | `d_q`, `d_eq`, `d_z` | default | F5 → DIG → Return → `Q` (WEAPON 1 = 1); `d_eq`: WEAPON 1 Right ×15 (→ 16) first, then DIG = `Q`; `d_z`: DIG = `Z` (44) — run `d_z` **only** under `$S/build-chk` and record the abort |
  | **P2** key box | `k_box` | default | F5 → AIM UP Return (PPM of the push frame) → `Q`; AIM DOWN → Esc; MOVE LEFT → `C` and `V` down in one frame; MOVE RIGHT → X down, then an X `repeat` while `K` is up (the repeat binds); FIRE → `APPLICATION` |
  | **P3** sounds | `s_close` | `fs` + install + `profiles sys` | the closing frame of NAME (Return, Esc, empty Return), SAVE PROFILE AS… (saved as `mine`, cancelled with Esc, reserved with the shipped leaf `Joystick0`), WEAPON n (accepted, empty Return, Esc), a key capture (a key, Esc) |
  | **P4** play | `p_first`, `p_dig` | default | `p_first`: NEW GAME → both DONE with the default keys → 100 frames → release → Esc → F5 → FIRE → `RCTRL` → F1 → press RCTRL: only P1's clean/`cs` Fire bit follows, P2's stays 0 (the rebind comes after selection so that P2 can still ready itself); `p_dig`: WEAPON 1 → 16, DIG = `Q` → NEW GAME → both DONE → hold Q (L+R on every event) → press/release LCTRL while holding → press D (Left) → release Q (Left stays, Right released) → release D |
  | **P5** profiles | `pr_io` | `fs` + install + `profiles sys` + `file user Profiles/broken.toml <scratch TOML with a syntax error>` | F5 → LOAD PROFILE (listing, title, the cursor) → `Lefty (L)` → SAVE PROFILE AS… `Joystick0` (reserved box, reopen with `Joystick0`) → Backspace ×9 → `mine` → Return → SAVE PROFILE → LOAD PROFILE `broken` → LOAD PROFILE `Joystick0` → SAVE PROFILE. The unreadable shape is not attempted (root reads a `chmod 000` file, fact 20): record "desk only" |
  | **P6** pads | `j_pad` | `fs` + install + `profiles sys` | F5 → LOAD PROFILE `Joystick0` → rows (INPUT, key names) → F (the network player's Down) moves the cursor → INPUT Left, Right, Enter (sounds) → LOAD `Joystick0` again → Esc → NEW GAME → P2 DONE, P1's keys do nothing → 120 frames → Esc → QUIT |
  | **P7** live edits | `l_att`, `l_det`, `l_sel` | `fs` + install | `l_att`: play 150 → Esc → F5: NAME `LIVE`, FIRE → `K` → F1 → K fires P1 (cs), LCTRL does not; `l_det`: as `l_att`, then Esc → F7 → LOAD SETUP `orbmit` → F5: FIRE → `J` → F1 → K still fires, J does not (RD-2); `l_sel`: NEW GAME → Esc during selection → F5 NAME `SEL` → F1 → the selection name box shows `SEL` (PPM) |
  | **P8** F9 | `n_slot` | default | F9 → Red → hold Left 40 frames → Esc → F9 again → Esc → QUIT; the `pal0` field per frame |

- [ ] **Step 4: the expected outcomes** (from R2 and facts 1–20). Record each as **confirmed**, or with what differed:
  - **P0.** `key_names[89] == ""`, `[29] == "Left Crtl"`, `[26] == "Å"` (UTF-8 `c3 85`), `[12] == "+"`, `[13] == "`"`; `GetKeyName(512) == "J0_0"`, `(543) == "J0_31"`, `(544) == "J1_0"`, `(300) == ""`; gamepad `(11) == "Up"`, `(110) == "RT+"`, `(9) == "LB"`, `(15) == "Btn15"`, `(113) == "A6-"`; controllers `Human`, `CPU`, `AI`. Fuzzy: `bazoka` → BAZOOKA's 1-based index; each result equals the integer cross-multiplication's (T1 reproduces the table).
  - **P1.** `ptrdiff == 28`; `d_q`: after the bind `wp[0] == 16` and `ex[7] == 16`; `d_eq`: `wp[0]` stays 16; `d_z` aborts under the checked build in `weap_order` (the WEAPON 1 row's `OnUpdate`), recorded, never gated.
  - **P2.** The push frame is presented with the box over the previous frame's pixels and the menu not redrawn (R-3); the pop frame redraws the row with `Q`; Esc leaves `ex` unchanged with no sound; `C`+`V` → `V` (47); the X repeat binds 45; `APPLICATION` binds 89 and the row's value is empty.
  - **P3.** MenuSelect counts on the closing frame: NAME 2 / 2 / 2; SAVE PROFILE AS… saved 2, cancelled 2, **reserved 1**; WEAPON n 1 / 1 / 1; key capture 0 / 0 (fact 10).
  - **P4.** `p_first`: after RESUME only P1's clean Fire bit follows RCTRL (first match); P2's never; `p_dig`: while Q is held every event of worm 0 leaves `cs & 0x0c == 0x0c`; after Q's release with D held `cs` has Left and not Right.
  - **P5.** Opens inside `./user/Profiles` titled `Select profile:` (+ path); rows: `broken` and the 8 shipped in `CiLess` order, `.toml` only; `Lefty (L)` → `prof` = its path, `col` unchanged, rgb ×4; the reserved box text `NAME 'Joystick0.toml' IS RESERVED` and the reopen holds `Joystick0`; `mine` saved to `user/Profiles/mine.toml` (the `file` line); SAVE PROFILE rewrites it; `broken` → `prof` = broken, no field changes; `Joystick0` → `in=1`, SAVE PROFILE → `user/Profiles/Joystick0.toml`.
  - **P6.** INPUT `Gamepad (none)`; key rows `Up, Down, Left, Right, RT+, RB, A, LB`; F moves the cursor (R2-21a); Left plays MoveDown, Right MoveUp, Enter MenuSelect, and each lands on `Keyboard` (the second LOAD re-makes it a pad player); NEW GAME stays in weapon selection with P1 never ready until Esc (R2-21b).
  - **P7.** As RD-2: attached rename and rebind act from the first resumed tick; detached, K still fires and J does not, the name stays `LIVE`; `l_sel` shows `SEL` in the name box on the first resumed selection frame.
  - **P8.** `pal0` equals the network player's ramp on every frame with `cur N` after the Red edit, and player 1's otherwise (including the frame after Esc).
- [ ] **Step 5: the contradiction rules** (the source wins; findings, not product choices):
  - **P0 differs** from R2-16 / R2-17: T1 ports what the real code printed; the plan's text is corrected in the addendum.
  - **P1: the write does not land in `weapons[0]`** (a different layout): the DIG validator follows the observation (bind DIG only where both sides agree) and PROGRESS records it; Q4's fix is unchanged.
  - **P2: the box is not on the push frame**: R-3 is reverted (the design's §3.3 stands) and D3's draw follows the observation.
  - **P3 differs**: D4's close table follows the observation.
  - **P4 contradicted** (no first match, or the DIG rule differs): **stop and report** — D1 is wrong, and T2 must not start.
  - **P5 differs**: D5 / D14 follow the observation.
  - **P6 differs**: D2's INPUT behaviour follows the observation; the Q5 ruling is re-read against it and PROGRESS records any change.
  - **P7 contradicted** (the detached match takes the new bindings): RD-2 is reverted to "the menu's settings", and T2 Step 3 follows.
  - **P8 differs**: D2 / fact 5 follow the observation.
  - **A probe cannot run** (a patch fails to build, the dumper crashes other than `d_z`): stop and report for P0, P1, P2 and P4; P3, P5–P8 may be recorded as "unconfirmed" and the plan proceeds from the source-derived facts.
- [ ] **Step 6: restore.** `git -C /home/user/openliero checkout -- src/tools/oracle_dump/shell_dump.cpp`, rebuild `oracle_dump_shell` in both builds; `git status --short src data` empty; no `/tmp/oracle_shell_fs_*` left.
- [ ] **Step 7: prove the restore.** The restored binaries regenerate `shell_boot_idle.txt`, `shell_milestone.txt`, `shell_setup_save.txt` and `shell_cpu_match.txt` byte-identically (`cmp` against `rust/oracle-tests/golden/`, written to `$S/t0f2/restore/`), release and checked.
- [ ] **Step 8: record and commit.** Append **"## Addendum T0 (probe results)"**: command lines, each verdict with frame numbers and field values, the P0 vector tables in full (T1 embeds them), `d_z`'s abort text, and any changed task text. `git add docs/superpowers/plans/2026-09-28-liero-rs-step4.5-slice4.5f2-plan.md`; commit `docs(4.5f-2): T0 — C++ probes of the key box, DIG, sounds, first match, profiles, pads, live edits and F9 (results and T1's vectors in the plan)`.

**Done when:** every probe is recorded, the contradiction rules are applied, `src/` is clean, and Step 7's four `cmp`s are identical under both builds.

---

### Task 1 (Batch 2): `ui::text` — key names, controllers, the fuzzy match

**Files:** `rust/ui/src/text.rs`.

- [ ] **Step 1: `KEY_NAMES: [&str; 177]`** — `Texts::key_names` (`src/game/common.cpp:25-203`) **verbatim**, UTF-8 as in the source (`"Å"`, `"Left Crtl"`, `""` at 89 and every other blank). **Test (RED first):** each entry's bytes equal Addendum T0 P0's hex, all 177.
- [ ] **Step 2: `get_key_name(key: u32) -> String`** (`gfx.cpp:828-840`): `< 177` the table; `>= 512` `format!("J{}_{}", (key - 512) / 32, (key - 512) % 32)`; else `""`. **`get_gamepad_key_name(k: u32) -> String`** (`:842-859`): `k >= 100` → axis `(k - 100) / 2` named `LX LY RX RY LT RT` (else `A<n>`) + `-` if odd else `+`; `k < 15` the 15 button names; else `Btn<k>`. **Tests:** P0's `GetKeyName` and `GetGamepadKeyName` tables.
- [ ] **Step 3: `CONTROLLERS: [&str; 3] = ["Human", "CPU", "AI"]`** (`common.cpp:214-216`), as `&'static [&'static str]` for `Behavior::ArrayEnum`. **Test:** equals P0.
- [ ] **Step 4: `levenshtein(a: &[u8], b: &[u8]) -> u32`** — a port of `mainMenuState.cpp:28-52` loop for loop (the `(s2len + 1) × (s1len + 1)` matrix, `MIN3`), each byte through `to_ascii_lowercase` (C++ `std::tolower`; a byte ≥ 0x80 is compared as itself, C++ UB, outside the gate). **Tests:** every Step-2 pair of Addendum T0 (all 40 names × the probe strings, plus the three classics).
- [ ] **Step 5: `weapon_fuzzy_match(names: &[String], typed: &[u8], current: u32) -> u32`** (`mainMenuState.cpp:398-417`): `best = current`, no best distance yet; for `i` in `1..=names.len()`: `d = levenshtein(names[i-1].as_bytes(), typed)`, `l = names[i-1].len()`; take `i` if there is no best yet or `d * l_best < d_best * l` (u64). Returns `best`. The caller never calls it on an empty `typed`. **Tests:** P0's menu results (`bazoka`, `LSR`, `a`, `zzzzzzzzzz`, `BIG NUKE`, the tie → the lower index); `current` is returned for an empty name list (unreachable, pinned); a doc comment states why the integer order equals C++'s `double` order (finding 14).
- [ ] **Step 6: gate + commit.** The full re-diff, the wasm check, golden status empty. Commit `ui(4.5f-2): ui::text — C++'s key-name table verbatim, GetKeyName / GetGamepadKeyName, the controller names, Levenshtein and the WEAPON n fuzzy match (integer, same order as C++'s doubles)`.

**Done when:** every table of Addendum T0 P0 is a passing test and nothing else moved.

---

### Task 6 (Batch 3, C++ only): `oracle_dump_shell` — tops `K`/`F`, the `cur` letters, `profiles`, `APPLICATION`

**Files:** `src/tools/oracle_dump/shell_dump.cpp`, `rust/oracle-tests/gen_shell_golden.sh`.

- [ ] **Step 1: `TopOf`**: `K`, `F` (§Formats); the `Fail` text says "a state 4½f-2 does not model".
- [ ] **Step 2: `DetailLine`**: `cur` and `ssel` per §Formats; the `Fail` names the frame.
- [ ] **Step 3: `MakeFixture`**: the `profiles <user|sys>` line (D13): `tok.size() == 2`, the layer checked, `std::filesystem::directory_iterator("data/Profiles")` (called from the repo root, as today) filtered to regular files ending `.toml`, **sorted** by name, each copied to `<layer>/Profiles/<name>` with the "given twice" check; a second `profiles` line for the same layer fails.
- [ ] **Step 4: `ScancodeOf`**: `{"APPLICATION", SDL_SCANCODE_APPLICATION}`.
- [ ] **Step 5: the search-gap check**: `kSearchable` gains `'F'`.
- [ ] **Step 6: the non-`fs` guard** (D12): in the frame loop, for a case with an empty `fs`: `Fail` on a top `F` after the frame, and on `std::filesystem::exists("Profiles")` (the CWD is `data/TC/openliero`).
- [ ] **Step 7: the header comment**: the tops `K`/`F`, the `cur` letters and `ssel`, the `profiles` line, `APPLICATION`, and "4½f-2 adds no intervention (RD-8)". The `out` header string is unchanged.
- [ ] **Step 8: `gen_shell_golden.sh`**: the three regexes, the comment, the default 41 (§Formats).
- [ ] **Step 9: the regeneration proof.** Background, log `$S/f2b3/gen.log`: `source $S/env.sh && EXPECTED_SHELL_CASES=32 bash rust/oracle-tests/gen_shell_golden.sh`; then `git status --porcelain rust/oracle-tests/golden` **empty**. Rebuild `$S/build-chk`; copy `$S/b7chk/run.sh` to `$S/f2b3/chk.sh` (output dir `$S/f2b3/chk/`) and run it: **32 `SAME`**, nothing under ASan.
- [ ] **Step 10: smokes** (`$S/f2b3/`, never committed; each run on release **and** checked, bytes equal):
  1. `setup default`, `detail`: F5 → Down ×3 → F6 → F9 → Esc ×2 → QUIT — `d` lines show `1`, `2`, `N` with the player menu's `ssel`, then `M`;
  2. the same + AIM UP Return → `APPLICATION` → the `K` top on the push frame, `M` after, `cfg16` changes;
  3. `fs` (the install + `profiles sys`) + F5 → LOAD PROFILE → the `F` top → Esc → QUIT;
  4. refusals, each exits non-zero with its message: `profiles sys` twice; `profiles foo`; a non-`fs` F5 → LOAD PROFILE (the top-`F` guard); a key name `F13` (`unknown key name`: `ScancodeOf` stops at F12, while Rust's `key_of` would silently map it to 71 — hence `APPLICATION`, pitfall 22).
- [ ] **Step 11: format, tidy, commit.** `"$CF22" --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/shell_dump.cpp`; `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh d2de489`; `scripts/clang-tidy-diff.sh build/linux-x64 d2de489` exits 0. Commit `oracle(4.5f-2): oracle_dump_shell — tops K (WaitForKeyState) and F (ProfileSelectorState), the player menus' cur letters 1/2/N and ssel, the profiles manifest line, APPLICATION, the non-fs profile guard; gen_shell_golden.sh knows them (count 41)`.

**Done when:** both builds are clean, the 32 prior shell goldens are byte-identical under both, and the smokes behave as described.

---

### Task 2 (Batch 4): the input seam — the live words from the bindings, DIG, the restart gate

**Files:** `rust/ui/src/keys.rs`, `rust/ui/src/shell/{mod.rs, playing.rs}`, `rust/game/src/main.rs` (mechanical), `rust/oracle-tests/tests/shell_common/mod.rs`.

- [ ] **Step 1: `ui::keys`** (D1). **Tests (RED first):**
  - `clean_words`: default settings — R sets worm 0 bit 0, the arrow Up worm 1 bit 0, DIG unbound never set, 0 never matches; P1 FIRE rebound to RCTRL (117) → RCTRL sets worm 0 bit 4 only (first worm wins, worm 1's fire dead); one key bound to two controls of one worm → only the first control's bit; a pad player (`input_device = 1`) takes nothing and the next keyboard player does; the network player (index 2) never takes a key; DIG bound to Q → bit 7;
  - `fold_dig`: bit 7 → Left and Right, bits 0..6 kept;
  - `apply_clean_edges == apply_key_edges` for every `p, n` in `0..128` and `c` in `{0, 0x7f, 0x0c, 0x10, 0x55}` (exhaustive loop; fact 13's equivalence);
  - the DIG rule (finding 9, T0 P4): DIG pressed → Left and Right set; DIG held + a Fire press → Left, Right, Fire; DIG held, Left consumed by the sim (`current` without Left), no event → stays consumed; DIG held + any event → Left and Right pressed again; DIG released with Left clean-held → Left kept, Right released; DIG released alone → both released;
  - `ReleaseLatch::arm_clean` masks bit 7 until DIG is released, and a re-press then passes; the 7-bit `arm`/`apply` tests are unchanged.
- [ ] **Step 2: `ShellInput { held, touch }` and the words** (D1). `Shell::words` (D1) is evaluated for `Match::process` over `Match::settings()` (RD-2), for NEW GAME's latch over the new match's settings, and for RESUME's latch after `resync`; `Settings::default()`'s bindings with `ShellDebug::live_bindings = false`; `touch` OR-ed into player 1. `Match::start(.., held: &[u8; 2])`, `Match::focus(&[u8; 2])`, `Match::process(sim, words: [u8; 2], ais, sounds)` per D1. `ShellDebug` gains `live_bindings: bool` (default `true`).
- [ ] **Step 3: the restart gate** (D8). **Tests:** a paused-and-resumed match whose settings were set to player 2 controller 2 while paused → `restart` does nothing, one note; P1 FIRE bound to F5 (63) in the match's settings → F5 with `restart = true` fires P1 (the word) and does not restart; with neither → restart as today (`f5_restart_is_the_next_new_game_without_the_menu` unchanged).
- [ ] **Step 4: the `ui` test helpers** (fact 14): `step_ev` maps words to the default keys (`DosHeld`); no call site changes. Every existing `ui` test must pass unchanged.
- [ ] **Step 5: headless flows** (`mod.rs` tests, through `Shell::frame`):
  - **Rebind attached:** NEW GAME → both DONE → play → Esc → set `worm_settings[0].controls_ex[4] = 37` (K) in the menu settings (`settings_mut`, T3 adds the menu) → F1 → holding K fires P1 (its word's Fire bit on the next tick); LCTRL no longer does.
  - **Rebind detached** (RD-2): as above, but LOAD SETUP-detach the match first (`Match::detach` through the e-2 path or the test hook) → K does **not** fire, LCTRL still does.
  - **First match:** P1 FIRE = RCTRL while resumed → RCTRL fires P1, P2's Fire bit never set.
  - **DIG in play:** DIG = Q (with WEAPON 1 = 16) → holding Q presses Left+Right on the first tick; the flow of Step 1's DIG rule through `Match` (two ticks each).
  - **A DIG key held at NEW GAME** is latched until released (the latch).
  - **A pad player 1** (`input_device = 1`): R/F/D/G do nothing in play; `touch` Fire still sets P1's Fire bit (Q6's in-match half).
  - **Determinism:** two runs with the same held-key script give identical per-frame `state_hash`es over 600 frames.
- [ ] **Step 6: `game`** (mechanical, D1): the shell tick builds `DosHeld` from `ButtonInput<KeyCode>` and the touch word; `sample_inputs*` keep serving `--live <scenario>` / `--replay` / Scripted / `?demo`. `game`'s own tests keep passing unchanged.
- [ ] **Step 7: the harness** (R-1): drop `words()`; pass `DosHeld::from_keys(held.iter().copied())`; recompute `controls` (all three keyboard players of `sh.settings()`, the menus' `TestControl` set) and `p2_controls` (worm 1 of the current match's settings, else the menu's) **every frame**.
- [ ] **Step 8: the full re-diff.** Both commands (all 32 shell cases in `shell_golden`, `key_edges` among them; `menu_widget_golden`; `record_regression.rs`, `round_trip.rs`), the wasm check, the four `-- check`s, golden status empty. A shell mismatch here is a seam bug: find the first frame and field, fix it in `ui::keys` / `Shell` with a unit test; never touch a golden.
- [ ] **Step 9: commit** `ui+game(4.5f-2): the live keyboard follows the bindings — clean_words (C++ FindControlForKey: first keyboard player, first control, DIG), 8-bit OnKey edges with both arms of the DIG rule, the words from the match's settings copy (old bindings after LOAD SETUP), the touch word into player 1; the F5 restart runs the NEW GAME gate and ignores a bound F5`.

**Done when:** every test above is green, all 32 prior shell cases and `key_edges` are bit-exact, and the non-shell paths are unchanged.

---

### Task 3 (Batch 5): the player menu, key capture, F5/F6/F9, the network palette

**Files:** new `rust/ui/src/shell/player_menu.rs`; `rust/ui/src/shell/{mod.rs, main_menu.rs, overlay.rs, stack.rs, settings_menu.rs}`; `rust/render/src/menu.rs`.

- [ ] **Step 1: `player_menu()`** (R2-1): 24 rows in `gfx.cpp:459-483` order and colours (profile rows 3/7, the rest 48/7), `Red`/`Green`/`Blue` mixed case, `WEAPON 1`…`WEAPON 5`, at (178, 20), `value_offset_x` 95. **Test:** the table, ids and colours; `visible_item_count` 22 after `update_items` with no profile, 24 with one.
- [ ] **Step 2: `PlayerMenuModel`** (D2). **Tests** (a model over `Settings::default()`):
  - values: NAME `""`; HEALTH `100%`; Red/Green/Blue `26 26 63` for player 1 (display ÷4); INPUT `Keyboard`; keys `R F D G Left Crtl Left Shift Left Alt` and DIG `""`; WEAPON n the TC names; CONTROLLER `Human`;
  - PROFILE LOADED and SAVE PROFILE hidden without a profile, shown with one (`profile: Some(..)` → PROFILE LOADED `leaf_basename`);
  - a pad player (`input_device = 1`, Joystick0's `gamepad_controls`): INPUT `Gamepad (none)`, rows `Up Down Left Right RT+ RB A LB`; with `gamepad_name = "A very long pad name that is long"` → its first 20 bytes;
  - INPUT: Left plays MoveDown, Right MoveUp, Enter MenuSelect; each lands on Keyboard and clears name and serial; Left/Right return `keep = false`;
  - HEALTH held Right: +1 every 4th `menu_cycles` (the `scroll_interval`); R/G/B ±4 clamped at 0 and 252; WEAPON n Left from 1 wraps to 40 and plays its sound; CONTROLLER Right cycles Human → CPU → AI → Human; a WEAPON value 0 or 41 and a CONTROLLER 3 show `""` without a panic (R2-7, R2-8);
  - the bars (`draw_item_overlay`, R2-9) against hand-computed pixels: selected frame colour 168, unselected 0, fill `ws.color`, width `rgb >> 2`; `rgb = 0` draws the width −1 box without a panic.
- [ ] **Step 3: `CurMenu::Player(p)` and the arms** (`mainMenuState.cpp:152-626`, facts 1–4):
  - Enter on LEFT/RIGHT/NETWORK PLAYER → `MenuSelect` + `player_settings(0|1|2)`; F5/F6/F9 in C++ order with F1, F2, F3, F7, F8 (fact 3) → `main_menu.move_to_id(..)` + `player_settings`, no sound;
  - Esc or any keyboard player's Jump with player focus → `cur_menu = Main` (cursor kept); F1 → main, start item; Up/Down/PgUp/PgDn and held Left/Right act on the player menu (with `PlayerMenuModel`);
  - the player Enter dispatch in source order: LOAD PROFILE / SAVE PROFILE AS… / SAVE PROFILE → `MenuSelect` **and nothing else in this task** (T4 replaces them; no test reaches them); NAME → `WormName`; the eight key rows → push `WaitForKey`; WEAPON n → `WeaponFuzzy`; else `on_enter` (HEALTH / R / G / B's `EditValue` → `IntegerEntry { target: Player(p) }`, INPUT and CONTROLLER play their own sounds);
  - `draw` per fact 4.
- [ ] **Step 4: `WaitForKeyState`** (D3). `Shell::frame` routes key events to it, the close runs D3's continuation; `draw_stack` draws the box (`draw_rounded_box` + `get_dims_h`). **Tests:** the box's pixels at (160, 100) against a hand-computed rectangle; top `K`; the push frame shows the box over the previous surface and the menu is not redrawn (R-3); the last key-down of a frame wins; a repeat binds; Esc changes nothing and plays nothing; `APPLICATION`'s 89 binds with a blank value; a DIG bind writes `controls_ex[7]` and **never** `weapons[0]` or `controls` (Q4), while a FIRE bind writes both `controls[4]` and `controls_ex[4]`.
- [ ] **Step 5: the overlays** (D4): `WormName`, `WeaponFuzzy`, `IntegerEntry { Player(p) }`, and `SaveAs { kind }` / `InfoPurpose::Reserved { kind, .. }` (the setup arm migrates; `save_setup_as` becomes `save_as(kind, ..)`, whose `Profile` arm is T4's). **Tests:** NAME Return with `WORMY` → 2 MenuSelects, `random_name = false`; Esc → 2 MenuSelects, name kept, `random_name = false`; empty Return → name `""`; 25 typed bytes keep 20; WEAPON: `bazoka` → BAZOOKA's index with 1 MenuSelect, empty Return and Esc keep the value with 1; HEALTH typed `0` → 1, `99999` → 10000, `250` → 250 (value `250%`); Green typed `70` → 63 (stored 252), `63` → 252; every e-2 SAVE SETUP AS… test still passes.
- [ ] **Step 6: the network palette** (fact 5): `render::menu::menu_palette_with` (+ its unit test: the slot-0 override equals a second `set_worm_colour(0, ..)` after both); `update_menu_palettes` passes `Some(ws[2].rgb)` iff `cur_menu == Player(2)`. **Test:** with ws[2].rgb edited, the slot-0 ramp follows it only while `Player(2)` has focus.
- [ ] **Step 7: headless flows** (`mod.rs` tests): F5 → the first visible row is SAVE PROFILE AS… (`selection()` and the drawn cursor); Down ×21 reaches CONTROLLER with a scroll; PgDn/PgUp; F6 from F5's menu re-points and moves to the first visible; P2's Jump returns; Enter on NETWORK PLAYER plays MenuSelect then shows player 3's values; the settings menu is not drawn while a player menu has focus.
- [ ] **Step 8: gate.** The full re-diff, the wasm check, G1 and all 32 G2 cases green, the four `-- check`s, golden status empty.
- [ ] **Step 9: commit** `ui(4.5f-2): the player menus — 24 rows (NAME, HEALTH, the colour bars, INPUT, the key rows with PRESS A KEY, WEAPON n by name, CONTROLLER), F5/F6/F9 in C++ order, the network player's slot-0 colour; DIG binds controls_ex only (Q4)`.

**Done when:** every test above is green, no prior gate moved, and `cargo tree … -p ui -e normal | grep -c bevy` prints `0`.

---

### Task 4 (Batch 6): profiles — LOAD / SAVE / SAVE AS…, the refs, the browser's profiles, Q7

**Files:** `rust/ui/src/shell/{files.rs, mod.rs, main_menu.rs, overlay.rs}`, `rust/scenario/src/{storage.rs, assets.rs}`, `rust/oracle-tests/tests/shell_common/mod.rs` (the one `placeable_leaf` call).

- [ ] **Step 1: `placeable_leaf(subdir, leaf)`** (D7) and its callers. **Test:** every e-2 vector under `Setups`; `Profiles` + `mine.toml` true, `a/b.toml` false.
- [ ] **Step 2: `ProfileSelectorState`** (D5, fact 9). **Tests** (a `MemoryStore` with the 8 profiles under `Profiles/` and a user `mine.toml`): enter opens inside `Profiles` with the root label's path in the title `Select profile:`; the rows are `CiLess` order with folders first, `.toml`/`.TOML` only; Left shows the root with the cursor on `Profiles`; a pick gives `Picked::Profile { player, rel }`.
- [ ] **Step 3: the arms** (D5, D6, D4's `Profile` arm): LOAD PROFILE → `MenuSelect` + push `ProfileSelect(player)`; SAVE PROFILE (D6); SAVE PROFILE AS… → `MenuSelect` + `save_as_box(Profile(p), b"", x + 97, y)` when in view; `apply_picked(Profile)`; `profiles` cleared in LOAD SETUP's `Ok` arm.
- [ ] **Step 4: Q7** (D10). **Tests:** touch-only shell, LOAD PROFILE `Lefty (R)` into RIGHT PLAYER → name/colour/keys loaded, `controller == 1`; `AI (R)` → 1; the same on a desktop shell → `0` and `2`; into LEFT PLAYER on a touch-only shell → the file's controller (`AI (L)` → 2, then NEW GAME shows the FollowAI box, Q2).
- [ ] **Step 5: the browser's profiles.** `assets::EMBEDDED_PROFILES` (the 8 files, `include_bytes!`, keyed `Profiles/<name>.toml`) joins `browser_system_files()`. **Test:** 8 `Profiles/` keys, bytes equal to `data/Profiles`; the single-layer store lists them.
- [ ] **Step 6: headless flows** (a split `MemoryStore`, root label `./user`, plus one `NativeStore` scratch tree):
  - LOAD `Lefty (L)` → PROFILE LOADED `Lefty (L)`, colour index kept, rgb ×4, keys W/S/A/D/Y/U/I, `visible_item_count` 24, the cursor item unchanged (no `move_to_first_visible`);
  - SAVE PROFILE AS… `Joystick0` → the reserved box (1 MenuSelect on the close frame) → any key → the profile box again holding `Joystick0` (not the setup box) → `mine` → 2 MenuSelects, `Profiles/mine.toml` in the user layer with `worm_settings_to_toml`'s bytes, PROFILE LOADED `mine`;
  - SAVE PROFILE of a loaded shipped profile writes `Profiles/<leaf>` into the user layer; Esc in SAVE PROFILE AS… saves nothing (2 MenuSelects);
  - a broken TOML → PROFILE LOADED `broken`, no field changes; a store that returns `None` → PROFILE LOADED unchanged and a note (fact 20);
  - LOAD SETUP clears all three refs; a paused attached match takes a loaded profile's health at RESUME (`apply_live_settings`), its name through `resync` (T5 draws it), and never its controller.
- [ ] **Step 7: gate + commit.** The full re-diff, the wasm check, G1 + 32 G2, four `-- check`s, golden status empty. Commit `ui+scenario(4.5f-2): profiles — LOAD PROFILE (Select profile:, opened in Profiles, loaded even when the file does not parse), SAVE PROFILE (the user copy), SAVE PROFILE AS… (the reserved box reopens the profile box), PROFILE LOADED; the browser lists the 8 shipped profiles; on a phone RIGHT PLAYER stays the CPU (Q7)`.

### Task 5 (Batch 6): names in weapon selection and the kill banners

**Files:** `rust/render/src/frame.rs`, `rust/scenario/src/loader.rs`, the render test helpers of fact 21, `rust/ui/src/shell/{selection.rs, playing.rs}`.

- [ ] **Step 1: `Scene::names`** (D11, fact 21); every literal gets `["", ""]`. **Test** (`frame.rs`): a dead worm 1 killed by worm 0 draws `killed_msg + "B"` in worm 0's viewport; a suicide draws `"A" + committed_suicide_msg`; empty names draw exactly today's pixels.
- [ ] **Step 2: `Selection` names and `resync`** (D11, R-10). **Tests:** NEW GAME with names `A`/`B` → the selection draw's name boxes (compare against `render::weapsel` with the same names); pause in selection → rename → RESUME → the next selection frame shows the new name; detached → the old one.
- [ ] **Step 3: the full re-diff** (every render golden, every shell golden: all names are empty there) and the wasm check.
- [ ] **Step 4: commit** `render+ui(4.5f-2): player names in the kill banners (KilledMsg + name, name + CommittedSuicideMsg) and weapon selection, refreshed at RESUME (hash-neutral: every golden has empty names)`.

**Batch 6 gate:** T4 and T5 together; one more full re-diff on the combined tree; the disk cleanup.

---

### Task 7 (Batch 7): G2f-2 through the real `Gfx::RunOneFrame`, and 🎯 MILESTONE f-2

**Files:** `rust/oracle-tests/tests/shell_common/mod.rs`, new `tests/shell_f2_cases/mod.rs` and `examples/gen_slice4_5f2_shell.rs`, `tests/shell_golden.rs`, the 22 `golden/` files; `rust/ui` / `rust/render` for proven fixes only.

- [ ] **Step 1: `shell_common`.**
  - `ALLOWED` and `key_of` per §Formats; `Fs::profiles(layer)` and `make_fixture_with`'s `profiles` line (D13).
  - The fact-4 validator ("another event in the frame that pushed …") widens to pushes of `K` and `F`; the search validator to `matches!(top0, 'O' | 'L' | 'P' | 'F')` (R-6).
  - The entry mirror (`OpenEntry`) knows the focused menu (`Shell::focused_menu()`, a new read-only accessor) and four kinds: digits (settings or player integer), `WormName` (initial = the name's bytes, any ASCII byte, max 20), `WeaponFuzzy` (initial empty, max 10), `SaveAs { Setup | Profile }` (max 30); the ledger records each close (kind, item, outcome, value).
  - **Validators** (each refused at generate time):
    1. **DIG** (R-15, pitfall 17): a `K` close that binds DIG (control 7) of player `p` to a key whose DOS ≠ `worm_settings[p].weapons[0]` at that frame; and any DIG key (of either player in the match's settings) held on a `W` frame;
    2. a user file written with a space in its path (RD-4), checked after every frame over the fixture's `user/` tree;
    3. a text event that is not one ASCII byte;
    4. an Enter on a profile row in a non-`fs` case (D12);
    5. every f-1 validator (FollowAI at NEW GAME, the selection constructor draws nothing, no RANDOM bot, Holdazone, the Q4 guard, a worm key held through Esc);
    6. a key bound to player 2 pressed in a match phase while player 2 is a CPU, unless the case declares it (`p2_keys`, f-1 pitfall 16).
  - **Ledger fields:** per frame `cur` and the player menu's `selection`; key binds `(frame, player, control, dos, value shown)`; name and weapon changes; profile loads/saves `(frame, player, rel)`; INPUT and CONTROLLER changes; frames with `cur = N` and `ws[2].rgb != ws[0].rgb` (slot-0 witness); match ticks with P1's clean DIG bit; first-match events (a key bound to two players pressed in a match); `W` frames with a non-empty name; dead-named-worm frames (a worm with `health <= 0` and a non-empty name on a presented match frame — the banner's proxy; the frames themselves are the gate).
- [ ] **Step 2: the 9 cases** (`shell_f2_cases::cases()`; all `detail`; every `match_seed` scripted; each ends by QUIT; text one char per event; the generator picks frame numbers and any search-found seeds and records them in the header):
  1. **`player_nav`** (`setup default`): F5 → Down through all 22 visible rows to CONTROLLER (scrolling) → PgUp → PgDn → Esc (main, cursor on LEFT PLAYER) → P1's Jump back from a re-entered F5 → Down to RIGHT PLAYER, Return → F9 while in RIGHT PLAYER → Down to Red → hold Left ~40 frames (the network player's Red: slot 0) → Esc (slot 0 back) → Down to NETWORK PLAYER, Return → P2's Jump (RSHIFT) back → QUIT.
  2. **`player_edit`** (`setup default`): F5 → HEALTH held Right (cadence 4) then Left; Return `250`; Return `0` (→ 1); Return `99999` (→ 10000); Return Esc; Red held Left/Right (step 4, the selected bar's colour-168 frame cycling) then Left to 0 (the −1 box); Green Return `63`, Return `70` (→ 63); CONTROLLER Right ×3 and Left (Human → CPU → AI → Human → AI → back to Human); WEAPON 1 Left (1 → 40) and Right (→ 1); INPUT Enter, Left, Right (stays Keyboard, three sounds) → QUIT. No NEW GAME (no FollowAI reaches NEW GAME).
  3. **`player_name`** (`setup default`): F5 → NAME Return → `WORMY` Return → NAME `X` Esc → NAME Backspace ×5 Return (empty) → NAME 25 characters (the cap) Return → NAME `WORMY` again → HEALTH `1` and WEAPON 1 by name to a weapon that hurts its shooter (the generator picks, recorded) → F6 NAME `B` → NEW GAME → the names in selection → Esc during selection → F5 NAME `SEL` → F1 (R-10: the resumed selection frame shows `SEL`) → both DONE → P1 fires into the ground until it dies (the suicide banner `SEL…`) → release, Esc → QUIT.
  4. **`player_weapon`** (`setup default`): F5 → WEAPON 2: `bazoka`, `LSR`, `a`, `zzzzzzzzzz`, the tie string (T1's function over the TC finds one; the lower index wins), then an empty Return and an Esc (value kept, one MenuSelect each) → QUIT.
  5. **`key_bind`** (`setup default`, both players human): F5 → AIM UP Return → `Q`; AIM UP → Esc (no change, no sound); AIM DOWN Return → an `X` repeat while the box is up (X held down before the Return) binds X; MOVE LEFT → `C` and `V` down in one frame (V wins); MOVE RIGHT → `APPLICATION` (blank row); JUMP → `TAB` → Esc → NEW GAME → both DONE (fire keys unchanged) → play: P1 aims with Q, walks right with `APPLICATION`, jumps with TAB → release all → Esc → F5 → FIRE → `RCTRL` → F1 → RCTRL fires P1 only (first match; P2's Fire never set: state hash) → release → Esc → QUIT.
  6. **`dig`** (`setup default`): F5 → WEAPON 1 Right ×15 (1 → 16) → DIG Return → `Q` (DOS 16 == WEAPON 1: C++'s overflow writes the same value, Q4's fix writes nothing — `cfg16` agrees) → Esc → NEW GAME → both DONE (no Q held on a `W` frame) → hold Q (Left+Right: digging) → while held press and release LCTRL, press D → release Q (Left kept, Right released) → release D → release all → Esc → QUIT.
  7. **`profile_io`** (`fs`: `Fs::install()` + `profiles(sys)` + `user_file("Profiles/broken.toml", "rust/oracle-tests/golden/shell_profile_io_user_broken.toml")`): F5 → LOAD PROFILE → `Lefty (L)` (colours ×4, colour index kept, PROFILE LOADED `Lefty (L)`) → SAVE PROFILE AS… `AI (L)` (reserved box, one MenuSelect) → any key → the reopened box → Backspace ×6 → `mine` Return (saved; PROFILE LOADED `mine`) → SAVE PROFILE (`Profiles/mine.toml` again) → SAVE PROFILE AS… → Esc (two MenuSelects) → LOAD PROFILE `broken` (PROFILE LOADED `broken`, nothing else) → LOAD PROFILE `Joystick0` (INPUT `Gamepad (none)`, the gamepad key names) → `F` moves the cursor through the network player (R2-21a) → SAVE PROFILE (`Profiles/Joystick0.toml`) → INPUT Enter (Keyboard) → Esc → QUIT. `file` lines: `Profiles/Joystick0.toml`, `Profiles/broken.toml`, `Profiles/mine.toml`, `Setups/liero.cfg` (no space in any, RD-4).
  8. **`player_live`** (`fs`: `Fs::install()`): NEW GAME → both DONE → ~200 frames → release → Esc → F5: HEALTH ≤ 30 (typed; the generator may pick lower to reach a death, recorded), NAME `LIVE`, Red to 0, FIRE → `K` → Esc → F1 (the clamp and the palette from the first resumed tick; K fires P1, LCTRL does not) → play until P1 dies (a `LIVE` banner) → release → Esc → F7 → LOAD SETUP → `orbmit` → F5: FIRE → `J`, NAME `GONE` → Esc → F1 (detached: K still fires, J does not, the banner name stays `LIVE`, the max stays) → release → Esc → QUIT (the exit save is orbmit's settings with the second edits).
  9. 🎯 **`player_setup`** (`fs`: `Fs::install()`; about 1,500 frames; done-when 7): F5 → NAME typed → HEALTH typed → Red held → MOVE RIGHT → `K` (a rebind) → WEAPON 1 by a misspelt name → Esc → F6 → CONTROLLER Right (CPU) → SAVE PROFILE AS… `cpu` → Esc → NEW GAME → P1 DONE, P2 (a PICK bot, D15) readied with its arrows + RCTRL on DONE → a human-vs-CPU match where P1 walks with K and fires → release → Esc → F5 → LOAD PROFILE `cpu` (player 1 takes P2's name, colour, health, arrow keys; its controller does not reach the match) → F1 RESUME (P1 now answers the arrows: first match, P2 is a CPU) → release → Esc → QUIT. `file` lines: `Profiles/cpu.toml`, `Setups/liero.cfg`.
- [ ] **Step 3: witnesses** (the generator prints each ledger; `write` refuses a case missing one; every case has no violations):

  | Case | Witnesses |
  |---|---|
  | `player_nav` | `cur` 1, 2, N; every visible row selected once; a scroll; PgUp and PgDn; one Esc and one Jump return each; ≥ 20 slot-0 frames; an Enter entry to each of the three menus |
  | `player_edit` | ≥ 3 HEALTH steps 4 cycles apart; the three typed clamps; Red at 0 on ≥ 1 frame; a bar with the selected frame; CONTROLLER values 0,1,2,0; WEAPON 1 at 40; three INPUT sounds and `in == 0` throughout |
  | `player_name` | the four NAME outcomes; a 20-byte name; `random_name == false` after the Esc; a `W` frame with `SEL`; ≥ 10 dead-named frames |
  | `player_weapon` | five fuzzy results equal to `weapon_fuzzy_match`'s; the tie resolved to the lower index; two kept-value closes |
  | `key_bind` | binds Q, X (from a repeat), V (two downs), 89, TAB; the Esc cancel; ≥ 1 first-match event after RESUME; P1 moved with 89 and jumped with TAB in the match |
  | `dig` | ≥ 30 match ticks with the clean DIG bit; an event while DIG is held; a DIG release with Left clean-held; `weapons[0] == 16` throughout |
  | `profile_io` | 4 loads (incl. `broken`), 3 saves + 1 reserved + 1 cancel; `in == 1` then `0`; the network player's key moved the cursor while P1 was a pad player; 4 `file` lines |
  | `player_live` | HEALTH clamp on the first resumed tick; K fired attached; a `LIVE` death; LOAD SETUP detached; K fired and J did not after the second RESUME |
  | `player_setup` | P2 a CPU on every match frame; a K-driven P1 move; `cpu.toml` saved; LOAD PROFILE while paused; arrows move P1 after RESUME; 2 `file` lines |
- [ ] **Step 4: `gen_slice4_5f2_shell.rs`** (the f-1 shape): `check` or `write <golden dir>`; writes `shell_profile_io_user_broken.toml` (a fixed TOML syntax error, e.g. `name = "unterminated`), the manifests, the 9 scripts with the two-line header, and asserts no violations and every witness. **Run:** `… --example gen_slice4_5f2_shell -- write /home/user/openliero/rust/oracle-tests/golden`, then `-- check`, then the four older `-- check`s.
- [ ] **Step 5: the C++ goldens.** `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh` (count 41; background, `$S/f2b7/gen.log`). Every awk gate passes; the 32 prior goldens stay byte-identical (`git status --porcelain … | grep '^ M'` empty); copy the checked loop to `$S/f2b7/chk.sh` → **41 `SAME`**, nothing under ASan. `git status --short data` empty.
- [ ] **Step 6: `shell_golden.rs`.** `mod shell_f2_cases;`; `the_committed_scripts_are_the_generators` covers the union of 41; `the_f2_milestone_is_bit_exact` (`player_setup`, its ledger asserted); `every_g2f2_case_is_bit_exact`; and the negative control `#[should_panic] the_key_bind_case_without_live_bindings_diverges` (`ShellDebug::live_bindings = false` — the 4½e default bindings — must mismatch on the first match tick where a rebound key acts).
- [ ] **Step 7: the fix loop.** `cargo test … -p oracle-tests --test shell_golden`; on the first mismatch, PPMs of both sides around it (`SHELL_RUST_PPM_DIR=$S/f2b7/r`; C++: `SHELL_PPM_DIR=$S/f2b7/c bash rust/oracle-tests/gen_shell_golden.sh`, which rewrites the same bytes). A `d`-line `cur`/`ssel`/`cfg16` mismatch is a menu bug; a `state8` mismatch with equal frames is an input-seam bug (T2); a `file` mismatch is a TOML/profile bug. Fix in `ui` (or `render` for a banner) with a unit test pinning the C++ line and the full re-diff; commit each fix separately. Never edit a golden.
- [ ] **Step 8: gate.** `shell_golden` (all 41 + the negative control) green; the full re-diff green; `menu_widget_golden` green; the five `-- check`s clean. The audit: `git diff --name-status d2de489 -- rust/oracle-tests/golden | grep -v '^A'` empty and `| wc -l` = **22**.
- [ ] **Step 9: commits.** Fixes first, `ui(4.5f-2): <what> (found by G2f-2 <case>, frame N)`. Then `oracle(4.5f-2): 🎯 MILESTONE f-2 — G2f-2 (the player menus, key capture, the fixed DIG in play, profiles, names, live edits attached and detached, a set-up human-vs-CPU match) bit-exact against the real C++ Gfx frame loop`, staging exactly the 22 golden files and the four source files.

**Done when:** all 9 f-2, 4 f-1, 6 e-2, 11 e-1 and 11 4½d cases match on every line under both C++ builds, and the negative control panics as expected.

---

### Task 8 (Batch 8): `game` and the page — Q6's buttons, the hooks, the help

**Files:** `rust/game/src/{main.rs, touch.rs}`, `web/index.html`, `.github/workflows/preview.yml`.

- [ ] **Step 1: Q6** (D9). `TouchKeys::tick(now, phase)` with D9's table; a release sends what its press sent; the menu repeat sends `DK_UP`/`DK_DOWN` repeats. `touch_key_events` keeps its signature for its existing tests only if still used; otherwise it is removed with its tests replaced. **Tests:** each cell of D9's table; a FIRE pressed in `Menu` and released in `Game` releases `DK_RETURN`; the repeat's 12 + 3 cadence; with player 1's `controls_ex` all rebound (and with `input_device = 1`) the events are identical (the keys are fixed).
- [ ] **Step 2: hooks.** `Hooks::sel` for `CurMenu::Player(p)` → `1<n>`/`2<n>`/`N<n>` and the `F` selector → `F<n>`; `Hooks::names: [String; 2]`, `Hooks::keys: [[u32; 8]; 2]`; published as `window.lieroNames` / `window.lieroKeys`. **Tests:** the letters, names and keys for a shell at each state.
- [ ] **Step 3: the page** (R-12): the help line `Dig: hold left + right · in a match <kbd>F5</kbd> restarts it (Rust only) · in the menu <kbd>F5</kbd> / <kbd>F6</kbd> / <kbd>F9</kbd> open LEFT / RIGHT / NETWORK PLAYER`, plus `Keys, names, colours and weapons can be changed in LEFT/RIGHT PLAYER; profiles save and load there (kept until you reload the page).`; the Phone line: `the buttons always act as the arrows, FIRE = Enter, JUMP/MENU = back in the menus, and move player 1 in a match, whatever keys are set`; `HINTS.key = ['Press a button to set it', 'MENU = cancel']`, chosen when `window.lieroTop === 'K'`; `HINTS.weapsel`/`game`'s `'You: blue worm'` becomes `'You: left view'` (the colour can now change). The preview comment gains one sentence: "LEFT/RIGHT PLAYER (F5/F6) edit names, keys, colours, weapons and the CPU switch; profiles load and save."
- [ ] **Step 4: gate.** `cargo test --manifest-path rust/Cargo.toml -p game`, the wasm check, the re-diff without `game`. Build the bundle, serve it, and run the walks (copies of `$S/b8.mjs` / `$S/tap1.mjs` as `$S/f2b8/{phone,desk}.mjs`; `ok`/`FAIL` lines; screenshots in `$S/f2b8/shots/`):
  - **Phone** (`/?touch=1`, emulated phone, CDP touch): `lieroControllers` `[0,1]`; pad to RIGHT PLAYER, FIRE → `lieroSel` starts `2` and CONTROLLER shows CPU (a screenshot + `lieroControllers[1] == 1`); JUMP → back (`M`); LEFT PLAYER → NAME, FIRE → `lieroPhase === 'text'`, `lieroTextMode === 'text'`; type `PHONE` into the field; FIRE → `lieroNames[0] === 'PHONE'`; AIM UP → FIRE → `lieroTop === 'K'` → pad ↑ → `lieroKeys[0][0] === 160`; FIRE row → FIRE → `lieroKeys[0][4] === 28`; the pad still moves the cursor and FIRE still selects (Q6); RIGHT PLAYER → LOAD PROFILE → `Lefty (R)` → `lieroNames[1]` is the profile's name and `lieroControllers[1] === 1` (Q7); LEFT PLAYER → LOAD PROFILE `Joystick0` → the menus still answer the pad; NEW GAME → DONE → `game` → within 120 s (FIRE while `lieroRespawn`) P1's `lieroWorms[0].x` changes under the pad (Q6, even as a pad player); no page errors.
  - **Desktop** (`/`): F5 → `lieroSel` `1…`; MOVE RIGHT → Return → K → `lieroKeys[0][3] === 37`; FIRE → Return → F5 → `lieroKeys[0][4] === 63`; Esc; NEW GAME, both DONE; K moves P1 right (x increases while visible); F5 fires P1 and does **not** restart (`lieroPhase` stays `game`, RD-6); rebind FIRE back to LCTRL through Esc + F5, F1; F5 now restarts (`weapsel`); LOAD PROFILE lists 8 rows + the folders; no page errors.
  - **Regression:** the e-2 phone walk (a copy of `$S/e2.mjs`'s phone part) still passes (its hint strings updated if it reads them).
- [ ] **Step 5: commit** `game(4.5f-2): the phone buttons always work (arrows, Enter and Esc in the menus, player 1 in a match; Q6), lieroSel for the player menus, lieroNames / lieroKeys; the page and the preview comment`.

**Done when:** the `game` tests and the wasm check pass, every walk line is `ok`, and the non-shell paths are unchanged.

---

### Task 9 (Batch 9): eyeball artefacts, docs, audits, broad review

**Files:** `docs/superpowers/liero-rs-PROGRESS.md`, the overview, the design (status line), the cpp-map, the rust-map, `.claude/skills/liero-shot/SKILL.md`. Re-read each right before editing it.

- [ ] **Step 1: the full green board.** The full re-diff, the wasm check, `cargo tree … -p ui -e normal | grep -c bevy` → `0`, `cargo tree … -p sim --depth 1` as at `d2de489`.
- [ ] **Step 2: tripwires and audits.**
  - `grep -nE "HashMap|HashSet|\bf32\b|\bf64\b|SystemTime|Instant" rust/ui/src/keys.rs rust/ui/src/text.rs rust/ui/src/shell/player_menu.rs` → empty.
  - `git diff --name-status d2de489 -- rust/oracle-tests/golden | grep -v '^A'` → empty; `| wc -l` → **22**.
  - `git diff --name-only d2de489 -- src CMakeLists.txt` → exactly `src/tools/oracle_dump/shell_dump.cpp`.
  - `git diff --name-only d2de489 -- rust/oracle-tests/examples` → exactly `gen_slice4_5f2_shell.rs`; `git diff d2de489 -- rust/sim` → empty; the frozen list untouched (`git diff --name-only d2de489 -- <each frozen path>` empty).
  - `git status --short data` → empty; `grep -c "ResMut<Sim>" rust/game/src/main.rs` as at `d2de489`.
  - `shell_dump.cpp` passes `$CF22` on the whole file; `scripts/clang-tidy-diff.sh build/linux-x64 d2de489` exits 0.
  - **Reproducibility**, one shell: the five `-- check`s; `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh`; `git status --porcelain rust/oracle-tests/golden` → empty; the checked loop → **41 `SAME`**.
- [ ] **Step 3: Xvfb side-by-side** (eyeball only). Copy `data/` to `$S/f2x/root`; run the real `openliero --config-root $S/f2x/root` (with `OPENLIERO_DATADIR=data`) and play `player_setup`'s path with xdotool; the Rust side is the browser bundle on the same keys. About 8 moments: LEFT PLAYER on entry, the colour bars, `PRESS A KEY`, WEAPON 1 typed, CONTROLLER CPU, `Select profile:`, the named selection, a banner with the name. Pair into `$S/f2x/side/`. Never commit.
- [ ] **Step 4: `liero-shot` §7**: one paragraph after the f-1 one (the player menus, profiles, the bindings-driven keyboard, Q4–Q7, G2f-2).
- [ ] **Step 5: PROGRESS.** Real date; a new **"4½f-2 LANDED — 4½f is COMPLETE"** header paragraph (the previous becomes "Prior (…)"); the tree's 4½f-2 line ✅ with the milestone; **Also open for John (4½f-2)** / notes: the rulings Q4–Q7 as executed; the refresh's R-1…R-15 and this plan's facts 1–22 confirmed or sharpened; T0's outcomes (P0–P8, `d_z`'s abort); D1 (the 8-bit siblings), D8 (the restart gate), D9 (the phone's fixed keys), D10 (Q7), D12 (profile rows only in `fs`), D14; the Rust-only divergences (the phone buttons; the restart gate); the known C++ UB list gains "the DIG overflow into WEAPON 1 (fixed, Q4) and the WEAPON 0 / > 40 and CONTROLLER ≥ 3 displays (Rust shows empty)"; the gates' numbers (9 cases, frames, `d` lines, `file` lines, the negative control's frame).
- [ ] **Step 6: the overview**: the status line `4½f LANDED`; the 4½f bullet "4½f-2 landed" with the plan path; **Q4** (key names) answered: ported from C++'s hard-coded table.
- [ ] **Step 7: the maps and the design.** **cpp-map:** facts 1–12, 16–21 at their sections (`MainMenuState` player arms, `WaitForKeyState`, `ProfileSelectorState`, `InputDeviceBehavior`, `UpdateMenuPalettes`, `FindControlForKey`). **rust-map:** `ui::keys` (clean words, `CleanEdges`), `ui::text` key names, `shell::player_menu`, `Screen::{WaitForKey, ProfileSelect}`, `MenuWorld::{player_menu, profiles}`, `SaveAsKind`, `Scene::names`, `menu_palette_with`, `placeable_leaf(subdir, leaf)`, `TouchKeys` (Q6). **Design status line:** `**4½f LANDED** (plans …4.5f1-plan.md, …4.5f2-plan.md)`.
- [ ] **Step 8: the broad review.** Re-read `git diff d2de489 -- rust src web .github` against the design, the refresh and this plan: every R-n / R2-n / fact is ported and gated (name its G2f-2 case or unit test) or recorded in PROGRESS; `MainMenuState::update`'s player arms follow `mainMenuState.cpp:152-612` in source order; `clean_words` + `CleanEdges` follow `game.cpp:75-106` and `localController.cpp:58-87`; the dumper change runs no code of its own under test; LD 1/3/4 and the crate rules hold; `game` is glue only; the non-shell paths are unchanged; no push, no PR. Bar: 0 Critical / 0 Important.
- [ ] **Step 9:** `git status --short` lists only the docs above. **Commit** `docs(4.5f-2): PROGRESS + overview + maps — slice 4.5f-2 landed, 4.5f complete (the player menus, profiles, key capture with the fixed DIG, names; G2f-2 + milestone f-2)`.

**Done when:** the board is green, every audit matches its expected output, the docs are updated, and the review is clean.

---

## Known pitfalls (carried from f-1, e-2, e-1, 4½d and PROGRESS, plus f-2's own; read them before your batch)

1. **Goldens are C++ truth.** Never regenerate one to make Rust pass, never hand-edit one. An `M` under `golden/` means: stop, `git checkout -- rust/oracle-tests/golden`, report the file and first differing line, do not commit. A dumper change re-proves all 32 prior shell goldens under the release **and** the checked build (T6), and all 41 after T7.
2. **DEBUG only.** `--release` breaks the `#[should_panic]` tests. The one non-test build is the wasm-release bundle.
3. **clang-format 18 is on PATH.** Always `$CF22`, and always the whole-file dry run too (the diff script misses what a deletion leaves behind).
4. **This clone has no `origin/master`.** Pass `d2de489` to both diff scripts.
5. **Gen scripts default to `macos-arm64`.** `source $S/env.sh` in the same shell as the script.
6. **rustfmt recurses** through `lib.rs`/`main.rs`: only on files you created.
7. **Long commands in the background** with logs under `$S` and a PID file; poll. The container has restarted before. **Never `pkill -f`** — `kill "$(cat <pidfile>)"`.
8. **`gen_sim_slice5prime_pickup_weapon_golden.sh` hangs here** (the pre-existing C++ `cossin_table[128]` read in `ProcessSight`; the checked build reports an ASan `global-buffer-overflow`). f-2 does not touch the sim dumper; if any batch runs the sim regeneration loop anyway, use `timeout` and report that script by name.
9. **`cargo test --workspace --exclude game` and `cargo test -p game` separately**; never the whole workspace with `game`.
10. **Disk.** Clean the incremental dirs after every batch; `df -h /` ≥ 3 GB before any wasm-release build; Rust batches are sequential.
11. **Explicit `git add` paths only**; never the scratchpad, PPMs, PNGs or fixture copies. **Trailers** `$CO` + `$SESS` on every commit, nothing else naming a model, no "Generated with …".
12. **Headless Chromium** runs at about 7–10 fps: the worms start dead (press FIRE while `lieroRespawn`; wait ~30 s); a tap longer than 12 ticks trips weapon selection's repeat (`frameTap`); iOS raises the keyboard only from a real tap; the CPU may kill player 1 during a wait (f-1 note); restart `http.server 8765` if it is down.
13. **The native `game` cannot run here** (no GPU); the bundle, the headless `Shell` tests and the G2 harness cover the live path.
14. **Shell-case rules from f-1 that bind f-2** (R-14): no RANDOM bot (intervention 3); both human players press DONE; release every worm key before Esc (a key released in the menu stays pressed in C++); no Holdazone NEW GAME/RESUME; no level under ~342 rows; a CPU still receives the keys bound to it (`key_bind` keeps player 2 human; `player_setup` presses P2's keys only in selection).
15. **Search letters are control keys** — R/F/D/G and **every key a case rebinds** (the harness recomputes the set every frame, T2 Step 7). One character per text event; text after Return in the same frame still lands (e-1 fact 6).
16. **The key names are C++'s, verbatim** (`"Left Crtl"`, `"Å"`, `"+"` at 12, `` "`" `` at 13, `""` at 89 and every gap). Never "fix" them; the font maps UTF-8 to CP437.
17. **The DIG rule (Q4 = A).** Rust writes `controls[i]` only for `i < 7`; C++ writes `controls[7]`, i.e. WEAPON 1. A gated DIG binding is only ever to the key whose DOS code equals that player's WEAPON 1 at bind time (so ≤ 40); a DIG key > 40 aborts the checked build (T0 `d_z`); **no DIG key is ever held on a `W` frame** (Rust's selection is unproven for DIG, fact 12).
18. **`cur` letters are `1`/`2`/`N`, never `L`/`R`** (`lieroSel`'s `L<n>` is the level selector, R-4). `ssel` is the player menu's `Selection()` only for them.
19. **File names with spaces** break the `file` line and both manifest parsers (R-5): no case saves to a spaced path; loading a spaced shipped profile is fine; install them with `profiles <layer>`.
20. **The player menu is one object re-pointed per player.** Every entry runs `update_items` + `move_to_first_visible`: the first visible row is SAVE PROFILE AS… without a profile, PROFILE LOADED with one. LOAD PROFILE's `update_items` does **not** move the cursor, though two rows appear above it.
21. **Live input reads the match's settings copy (RD-2); the menus read the menu's settings.** A rebind while paused acts from the first resumed tick when attached and **never** after LOAD SETUP (detached keeps the old bindings).
22. **`APPLICATION` is the unmapped key** (DOS 89, a blank row). Never `F13`: `key_of` turns any `F<n>` into `58 + n` (`F13` → 71, keypad 7).
23. **The key box draws on its push frame** over the previous frame's pixels (R-3); the pop frame redraws the menu with the new name.
24. **Closing sounds** (fact 10, T0 P3): NAME 2, SAVE PROFILE AS… 2 (1 when reserved), WEAPON n 1, a key capture 0. INPUT's Left plays MoveDown and Right MoveUp.
25. **The F5 restart gate (RD-6, D8)** is Rust-only and outside every gate: the harness never sets `restart`. A refused NEW GAME or an F5 bound as a key makes F5 do nothing but a note.
26. **LOAD PROFILE keeps the colour index and shows PROFILE LOADED even on a TOML error**; an unreadable file changes nothing — and cannot be made here (root reads everything), so it is unit-tested only.
27. **Profile rows only in `fs` cases** (D12): a non-`fs` C++ run has no config node and would write under `data/TC/openliero`. `git status --short data` must stay empty.
28. **No gamepads anywhere**: INPUT always lands on Keyboard; a pad player is skipped by the keyboard (menus and play) but **not** by `ReleaseControl`; the network player's default keys equal player 1's, so after a Joystick profile R/F/D/G still drive the menus (R2-21a), and a human pad player never readies in weapon selection (R2-21b).
29. **Q6's fixed touch keys are Rust-only** and never reach a golden (the harness has no touch).
30. **WEAPON n = 0 or > 40 and CONTROLLER ≥ 3 in a file are C++ UB**; Rust shows an empty value; no gate loads one.
31. **Names reach a paused match only at RESUME**, through `resync` (R-10), in selection as in play; a detached match keeps its old names.
32. **`ShellInput::idle()` holds no key**; the `ui` test helpers turn words into the default keys (fact 14) — a test that rebinds must pass explicit held keys.
33. **`EXPECTED_SHELL_CASES=32`** for every `gen_shell_golden.sh` run until T7 commits the f-2 scripts.
34. **The weapon-selection name box and banners index by worm** (`names[dead worm]`), not by viewport.

## Done-report (each batch)

Each batch reports:
- (a) what changed and why (tie each change to its finding / fact / decision);
- (b) the files touched;
- (c) the gate commands run, with their results (test counts; the golden audit line; the `SAME` counts; clang-format / clang-tidy exit codes);
- (d) the commit SHAs;
- (e) anything that contradicted this plan and how the contradiction rules resolved it, and any reorder.

Batch 1 also reports every probe's verdict and P0's vector counts. Batch 3 reports the 32 `SAME` under each build and the smokes. Batch 4 reports the shell-case counts (all 32 bit-exact) and whether any seam fix was needed. Batch 7 reports each case's frames, `d` lines and `file` lines, and the negative control's first diverging frame.

**The final report (Batch 9) surfaces:**
- T0's outcomes: P0 (the vectors), P1 (the `weapons[0]` aliasing and `d_z`'s abort), P2 (the push-frame draw), P3 (the closing sounds, incl. the reserved 1), P4 (first match, the DIG rule in play), P5–P8, and any contradiction rule applied;
- the dumper evidence: T6's regeneration proof (32 prior shell goldens under both builds) and its smokes;
- T2's seam result: all 32 prior shell cases and `key_edges` bit-exact on the new input path, `record_regression` / `round_trip` untouched and green, any seam fix with its case and frame;
- 🎯 the G2f-2 result: 9 cases, their frame / `d`-line / `file`-line counts, `player_setup` named on its own, the negative control's frame, any `ui` fix with its case and frame;
- the Chromium lines (phone: RIGHT PLAYER CPU, NAME through the field, the key box with the pad, Q7, a Joystick player moved by the pad; desktop: F5 menus, a live rebind, F5 bound vs restart; the e-2 regression);
- the Xvfb PNG paths;
- the audit sweep: 22 `A`, 0 `M`; one C++ file; one new example; `rust/sim` untouched; the frozen list untouched; `data/` clean; `cargo tree` 0 bevy; reproducibility (41 `SAME` checked, 0 golden changes);
- the Rust-only divergences as shipped (Q4's DIG, Q6's buttons, Q7, the restart gate) and anything left open.

## Questions for John

None. Every open point in the design and the refresh is engineering, decided above (D1–D16); the two behaviour choices it raised (Q6, Q7) were ruled on 2026-09-28 and are encoded in D9 and D10.

(The T0 addendum is appended below by Batch 1.)

---

## Addendum T0 (probe results)

Run 2026-09-28 on `claude/cpp-oracle-vcpkg-assets-chcwcm` at `c7fffc1`, against the real `Gfx::RunOneFrame` in a temporarily patched `oracle_dump_shell` (release `build/linux-x64` and checked `$S/build-chk`). Every artefact is in `$S/t0f2/`. Nothing but this addendum is committed.

**Verdict.**
- **P0–P8 are all CONFIRMED.** No contradiction rule of Step 5 fired, so D1–D16 and T1–T9 stand as written (§"Changes to later tasks").
- Every probe script ran under **both** builds with byte-identical output (13 scripts; `cmp` equal). The one exception is `d_z`, run under the checked build only, as planned: it aborts (P1).
- The restored dumper reproduces the four Step-7 goldens byte-identically under both builds (§"Restore").

### Method

- **The patch** is `$S/t0f2/probe.diff` (243 lines). It lived in the working tree only. In `oracle_dump_shell` it added:
  - `ScancodeOf`: `APPLICATION` → `SDL_SCANCODE_APPLICATION`;
  - `TopOf`: `WaitForKeyState` → `K`, `ProfileSelectorState` → `F` (both checked right after `MainMenuState`);
  - `DetailLine`: `cur` = `1`/`2`/`N` when `cur_menu == &player_menu` and `player_menu.ws == settings->worm_settings[i]` (else `Fail`); `ssel` = `player_menu.Selection()` for those, `settings_menu.Selection()` for `M`/`S` (the §Formats rule verbatim);
  - `F` in the search-gap `kSearchable` set;
  - the `profiles <user|sys>` manifest line exactly as D13 (sorted `data/Profiles/*.toml`, "given twice" checks);
  - a probe-only `x <frame>` line after every `d` line: for `i` in 0..3 `w<i> name=<hex> hp rgb col in gp=<hex> ctl keys=<controls[0..7]> ex=<controls_ex[0..8]> wp=<weapons[0..5]> rn prof=<profile_node.FullPath() or ->`; while the player menu has focus `rows=[<id>[h]:<value>]…` (every item's `value`, `h` = hidden); `pal0=` the five `play_renderer.pal` entries `kWormColorBlocks[0].base - 2 … + 2`; per worm of `CurrentGame()` `clean=<%02x clean_control_states> cs=<%02x control_states> same=<worm->settings == settings->worm_settings[i]> nm=<hex of the worm's settings name> fire=<its controls_ex[4]>`; and while a `ProfileSelectorState` is on top `| F title=[<title_> <current_node->full_path>] sel=<Selection()> rows=[<child names>]` (read through a member pointer to the protected `selector_` / `title_`);
  - a one-shot `PROBE_VECTORS=<file>` mode (Step 1's list; exits before loading anything).
- **Build:** `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell` and `cmake --build $S/build-chk --target oracle_dump_shell`, both exit 0.
- **Fixtures:** `$S/t0f2/gen.py` writes every script: `setup default`, `boot_seed 7`, `detail`, first key at frame 40, taps down at *t* and up at *t*+2 with the next key at *t*+3, text one character per event and per frame (text events only, no key events), end by QUIT. Player-menu rows are addressed by menu index (0 PROFILE LOADED, 1 SAVE PROFILE, 2 SAVE PROFILE AS…, 3 LOAD PROFILE, 4 NAME, 5 HEALTH, 6–8 R/G/B, 9 INPUT, 10–17 AIM UP … DIG, 18–22 WEAPON 1–5, 23 CONTROLLER); every `ssel` below is that index. The `fs` manifests are the e-2 install (`shell_setup_save_fs.txt` minus comments) = `fs_install.txt`, plus `profiles sys` = `fs_prof.txt`, plus `file user Profiles/broken.toml $S/t0f2/broken.toml` (`name = "broken` + `health = [`, a TOML syntax error) = `fs_broken.txt`.
- **Runs:** `$S/t0f2/runall.sh` / `rerun.sh` (release to `out/<case>_rel.out`, checked with `ASAN_OPTIONS=detect_leaks=0` to `out/<case>_chk.out`, then `cmp`); PPMs (release only) for `k_box`, `pr_io`, `l_sel` under `$S/t0f2/ppm/`. Sound ids in the `f` lines: **25** = MenuMoveUp (the Down key), **26** = MenuMoveDown (the Up key), **27** = MenuSelect.
- **Method deviations (stronger, not weaker):** `v_fuzzy` types `bazoka` a second time after `la` (the first `bazoka` leaves WEAPON 2 at its current 1, which cannot tell the match from "unchanged"); `j_pad` re-loads `Joystick0` before **each** of INPUT Left / Right / Enter, so every one of the three starts from a pad player. The first `p_first`/`l_*` runs pressed their keys during RESUME's 32-frame fade-out (the waits were 20 frames) and were rerun with 45; the first `l_det` run's 15 Downs wrapped the 14-row settings menu onto GAME MODE and was rerun with F7 → Up (LOAD SETUP = index 18). Only the reruns are recorded.

### P0 — the vectors for T1: CONFIRMED

`PROBE_VECTORS` output, release and checked byte-identical (`$S/t0f2/vectors_rel.txt`).

- **`Texts::key_names[i]`, all 177, as hex bytes** (`-` = the empty string). Confirms `[89] = ""`, `[29] = "Left Crtl"`, `[26] = "Å"` (`c3 85`), `[12] = "+"`, `` [13] = "`" ``, and `[117] = "Right Ctrl"` (spelled correctly, unlike 29):
  ```
  0:- 1:457363 2:31 3:32 4:33 5:34 6:35 7:36
  8:37 9:38 10:39 11:30 12:2b 13:60 14:4261636b7370616365 15:546162
  16:51 17:57 18:45 19:52 20:54 21:59 22:55 23:49
  24:4f 25:50 26:c385 27:5e 28:456e746572 29:4c656674204372746c 30:41 31:53
  32:44 33:46 34:47 35:48 36:4a 37:4b 38:4c 39:c396
  40:c384 41:c2bd 42:4c656674205368696674 43:27 44:5a 45:58 46:43 47:56
  48:42 49:4e 50:4d 51:2c 52:2e 53:2d 54:5269676874205368696674 55:2a202850616429
  56:4c65667420416c74 57:- 58:43617073204c6f636b 59:4631 60:4632 61:4633 62:4634 63:4635
  64:4636 65:4637 66:4638 67:4639 68:463130 69:4e756d204c6f636b 70:5363726f6c6c204c6f636b 71:37202850616429
  72:38202850616429 73:39202850616429 74:2d202850616429 75:34202850616429 76:35202850616429 77:36202850616429 78:2b202850616429 79:31202850616429
  80:32202850616429 81:33202850616429 82:30202850616429 83:2c202850616429 84:- 85:- 86:3c 87:463131
  88:463132 89:- 90:- 91:- 92:- 93:- 94:- 95:-
  96:- 97:- 98:- 99:- 100:- 101:- 102:- 103:-
  104:- 105:- 106:- 107:- 108:- 109:- 110:- 111:-
  112:- 113:- 114:- 115:- 116:456e746572202850616429 117:5269676874204374726c 118:- 119:-
  120:- 121:- 122:- 123:- 124:- 125:- 126:- 127:-
  128:- 129:- 130:5072696e742053637265656e 131:- 132:- 133:- 134:- 135:-
  136:- 137:- 138:- 139:- 140:- 141:2f202850616429 142:- 143:5072696e742053637265656e
  144:526967687420416c74 145:- 146:- 147:- 148:- 149:- 150:- 151:-
  152:- 153:- 154:- 155:- 156:- 157:- 158:- 159:486f6d65
  160:5570 161:50616765205570 162:- 163:4c656674 164:- 165:5269676874 166:- 167:456e64
  168:446f776e 169:5061676520446f776e 170:496e73657274 171:44656c657465 172:- 173:- 174:- 175:-
  176:-
  ```
- **`Gfx::GetKeyName(k)`:** 0 → `""`, 1 → `Esc`, 12 → `+`, 13 → `` ` ``, 26 → `Å`, 27 → `^`, 29 → `Left Crtl`, 41 → `½` (`c2 bd`), 89 → `""`, 176 → `""`, 177 → `""`, 300 → `""`, 511 → `""`, 512 → `J0_0`, 513 → `J0_1`, 543 → `J0_31`, 544 → `J1_0`, 1000 → `J15_8`.
- **`Gfx::GetGamepadKeyName(k)`:** 0 `A`, 1 `B`, 2 `X`, 3 `Y`, 4 `Back`, 5 `Guide`, 6 `Start`, 7 `LS`, 8 `RS`, 9 `LB`, 10 `RB`, 11 `Up`, 12 `Down`, 13 `Left`, 14 `Right`, 15 `Btn15`, 99 `Btn99`, 100 `LX+`, 101 `LX-`, 102 `LY+`, 103 `LY-`, 104 `RX+`, 105 `RX-`, 106 `RY+`, 107 `RY-`, 108 `LT+`, 109 `LT-`, 110 `RT+`, 111 `RT-`, 112 `A6+`, 113 `A6-`, 114 `A7+`, 130 `A15+`.
- **`common.texts.controllers`:** `Human`, `CPU`, `AI`.
- **P1's layout vector:** `ptrdiff = (char*)ws.weapons - (char*)ws.controls = 28`, `sizeof ws.controls = 28`.
- **The Levenshtein copy is verbatim.** Step 2's range is **lines 26–54**, not 26–53: `#define MIN3` is line 26 and `#undef MIN3` is line 54 (the function is 28–52). `diff <(sed -n 26,54p src/game/mainMenuState.cpp) <(sed -n '/^#define MIN3/,/^#undef MIN3/p' $S/t0f2/lev.cpp)` is empty; `lev.cpp` built with `g++ -std=c++20 -O1`. Classics: (`kitten`,`sitting`) = 3, (`ABC`,`abc`) = 0, (`a`,`""`) = 1.
- **The 40 names in `weap_order`** (bytewise `std::string <` over the TC's `name` fields, all ASCII) and `Levenshtein(name, typed)` for each probe string (`$S/t0f2/lev_table.txt`):

| # | name (weap_order) | len | `bazoka` | `LSR` | `a` | `zzzzzzzzzz` | `BIG NUKE` | `big nuke` | `""` | `la` |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | BAZOOKA | 7 | 1 | 7 | 6 | 9 | 6 | 6 | 7 | 6 |
| 2 | BIG NUKE | 8 | 6 | 8 | 8 | 10 | 0 | 0 | 8 | 8 |
| 3 | BLASTER | 7 | 5 | 4 | 6 | 10 | 7 | 7 | 7 | 5 |
| 4 | BOOBY TRAP | 10 | 8 | 9 | 9 | 10 | 8 | 8 | 10 | 9 |
| 5 | BOUNCY LARPA | 12 | 10 | 10 | 11 | 12 | 10 | 10 | 12 | 10 |
| 6 | BOUNCY MINE | 11 | 10 | 11 | 11 | 11 | 8 | 8 | 11 | 11 |
| 7 | CANNON | 6 | 5 | 6 | 5 | 10 | 7 | 7 | 6 | 5 |
| 8 | CHAINGUN | 8 | 7 | 8 | 7 | 10 | 7 | 7 | 8 | 7 |
| 9 | CHIQUITA BOMB | 13 | 11 | 13 | 12 | 13 | 11 | 11 | 13 | 12 |
| 10 | CLUSTER BOMB | 12 | 11 | 9 | 12 | 12 | 11 | 11 | 12 | 11 |
| 11 | CRACKLER | 8 | 7 | 6 | 7 | 10 | 8 | 8 | 8 | 7 |
| 12 | DART | 4 | 5 | 3 | 3 | 10 | 8 | 8 | 4 | 3 |
| 13 | DIRTBALL | 8 | 7 | 7 | 7 | 10 | 7 | 7 | 8 | 7 |
| 14 | DOOMSDAY | 8 | 7 | 7 | 7 | 10 | 8 | 8 | 8 | 7 |
| 15 | EXPLOSIVES | 10 | 9 | 8 | 10 | 10 | 9 | 9 | 10 | 9 |
| 16 | FAN | 3 | 5 | 3 | 2 | 10 | 7 | 7 | 3 | 2 |
| 17 | FLAMER | 6 | 6 | 4 | 5 | 10 | 8 | 8 | 6 | 4 |
| 18 | FLOAT MINE | 10 | 9 | 9 | 9 | 10 | 8 | 8 | 10 | 8 |
| 19 | GAUSS GUN | 9 | 8 | 8 | 8 | 10 | 8 | 8 | 9 | 8 |
| 20 | GRASSHOPPER | 11 | 9 | 9 | 10 | 11 | 10 | 10 | 11 | 10 |
| 21 | GREENBALL | 9 | 8 | 9 | 8 | 10 | 8 | 8 | 9 | 8 |
| 22 | GRENADE | 7 | 7 | 7 | 6 | 10 | 6 | 6 | 7 | 6 |
| 23 | HANDGUN | 7 | 6 | 7 | 6 | 10 | 7 | 7 | 7 | 6 |
| 24 | HELLRAIDER | 10 | 9 | 8 | 9 | 10 | 9 | 9 | 10 | 8 |
| 25 | LARPA | 5 | 4 | 3 | 4 | 10 | 8 | 8 | 5 | 3 |
| 26 | LASER | 5 | 5 | 2 | 4 | 10 | 8 | 8 | 5 | 3 |
| 27 | MINE | 4 | 6 | 4 | 4 | 10 | 5 | 5 | 4 | 4 |
| 28 | MINI NUKE | 9 | 8 | 9 | 9 | 10 | 3 | 3 | 9 | 9 |
| 29 | MINI ROCKETS | 12 | 10 | 11 | 12 | 12 | 8 | 8 | 12 | 12 |
| 30 | MINIGUN | 7 | 7 | 7 | 7 | 10 | 6 | 6 | 7 | 7 |
| 31 | MISSILE | 7 | 7 | 6 | 7 | 10 | 6 | 6 | 7 | 6 |
| 32 | NAPALM | 6 | 5 | 6 | 5 | 10 | 8 | 8 | 6 | 5 |
| 33 | RB RAMPAGE | 10 | 8 | 9 | 9 | 10 | 8 | 8 | 10 | 9 |
| 34 | RIFLE | 5 | 6 | 5 | 5 | 10 | 6 | 6 | 5 | 4 |
| 35 | SHOTGUN | 7 | 7 | 7 | 7 | 10 | 7 | 7 | 7 | 7 |
| 36 | SPIKEBALLS | 10 | 9 | 9 | 9 | 10 | 9 | 9 | 10 | 9 |
| 37 | SUPER SHOTGUN | 13 | 12 | 12 | 13 | 13 | 12 | 12 | 13 | 13 |
| 38 | UZI | 3 | 5 | 3 | 3 | 9 | 7 | 7 | 3 | 3 |
| 39 | WINCHESTER | 10 | 10 | 8 | 10 | 10 | 8 | 8 | 10 | 10 |
| 40 | ZIMM | 4 | 5 | 4 | 4 | 9 | 7 | 7 | 4 | 4 |

- **The fuzzy match through the real menu** (`v_fuzzy`: F5 → WEAPON 2 (ssel 19) → Return → text → Return, each box closing with one `27`). `wp[1]` and the WEAPON 2 row's value after each close:

  | Typed | Close frame | `wp[1]` | Row | Integer cross-multiplication (T1) |
  |---|---|---|---|---|
  | `bazoka` (current 1) | 68 | 1 | BAZOOKA | 1 (1/7) |
  | `LSR` | 78 | 26 | LASER | 26 (2/5) |
  | `a` | 86 | 16 | FAN | 16 (2/3) |
  | `zzzzzzzzzz` | 103 | 4 | BOOBY TRAP | 4 — a 14-way tie at ratio 1 (4, 5, 6, 9, 10, 15, 18, 20, 24, 29, 33, 36, 37, 39): the lowest index wins |
  | `BIG NUKE` | 118 | 2 | BIG NUKE | 2 (0) |
  | `la` (the tie string) | 127 | 25 | LARPA | 25 — LARPA 3/5 ties LASER 3/5: the lower index wins |
  | `bazoka` (current 25) | 140 | 1 | BAZOOKA | 1 |
  | empty Return | 146 | 1 | BAZOOKA | unchanged (never called on an empty name) |

  Every result equals the table's integer cross-multiplication with strict `<` from index 1 (`$S/t0f2/levtab.py`).

### P1 — DIG: CONFIRMED

- `ptrdiff == 28` (P0).
- `d_q` (F5 → DIG, ssel 17 → Return on 64 → `Q` on 67): frame 66 `ex=…,0 wp=1,1,1,1,1`; frame 67 `keys` unchanged, `ex=19,33,32,34,29,42,56,16 wp=16,1,1,1,1` — the `controls[7]` write lands in `weapons[0]`; `cfg16` changes on 67.
- `d_eq` (WEAPON 1 Right ×15 → `wp[0] = 16` by frame 110, then DIG = `Q` on 112): `ex[7] = 16`, `wp[0]` stays 16.
- `d_z` (DIG = `Z`, 44), checked build only: **aborts on the bind frame (67), exit 134, no output written**:
  ```
  /usr/include/c++/13/bits/stl_vector.h:1128: constexpr std::vector<_Tp, _Alloc>::reference std::vector<_Tp, _Alloc>::operator[](size_type) [with _Tp = int; _Alloc = std::allocator<int>; reference = int&; size_type = long unsigned int]: Assertion '__n < this->size()' failed.
  ```
  With `ASAN_OPTIONS=handle_abort=1` the stack is `std::__glibcxx_assert_fail` ← `WeaponEnumBehavior::OnUpdate(Menu&, MenuItem&)` ← `MainMenuState::Update()::{lambda(unsigned int, bool)#1}` (the key callback's `UpdateItems`) ← `WaitForKeyState::Update()` ← `Gfx::RunOneFrame()` ← `main`. Recorded, never gated (pitfall 17).

### P2 — the key box: CONFIRMED

`k_box`: F5 → AIM UP (ssel 10).
- **The push frame draws the box over the previous frame's pixels.** Return on 69: `f 69 M … K … 27` (upd `M`, top `K`, one MenuSelect). The presented frame 69 differs from 68 **only** inside (133..187, 94..102), the `PRESS A KEY` box (465 pixels). Frame 67 → 68 changes the selected row's animation (180..276, 85..89), and 68 → 69 does not: the menu was not redrawn. Frames 69, 70 and 71 present the same hash. PPMs: `ppm/k_box/f_0068.png`, `f_0069.png`.
- **The pop frame redraws the menu with the new name:** `Q` on 72: upd `K`, top `M`, **no sound**; `ex[0] = 16`; the frame differs from 71 over the box and the AIM UP row (`f_0072.png` shows `AIM UP Q`).
- **Esc** (AIM DOWN, push 78, Esc 81): upd `K` → top `M`, sounds `-`, `ex` unchanged.
- **`C` + `V` down on one frame** (MOVE LEFT, 90): `ex[2] = 47` (`V`, the last key-down).
- **A repeat binds:** `X` held since 96 (menu), MOVE RIGHT Return on 99, an `X` `repeat` on 102 while `K` is up → `ex[3] = 45`.
- **`APPLICATION`** (FIRE, 111): `keys[4] = ex[4] = 89`; the FIRE row's value is `""` (`rows=…[10:]…`).
- The probe `d` line keeps `cur 1` with the player menu's `ssel` on every `K` frame (`d 64 1 17` … `d 66 1 17` in `d_q`), as §Formats and the `lieroSel` hook assume.

### P3 — closing sounds: CONFIRMED

`s_close` (`fs_prof.txt`); the sounds of each closing frame (the frame whose event closes the box), from `f` field 11:

| Box | Close | Frame | Sounds | Note |
|---|---|---|---|---|
| NAME | Return (`AB`) | 55 | `27,27` | `name = "AB"` |
| NAME | Esc | 63 | `27,27` | name unchanged |
| NAME | empty Return (Backspace ×2) | 75 | `27,27` | `name = ""` (`GenerateName` is a no-op), `rn = 0` |
| SAVE PROFILE AS… | Return (`mine`) | 92 | `27,27` | `prof = ./user/Profiles/mine.toml` |
| SAVE PROFILE AS… | Esc | 100 | `27,27` | |
| SAVE PROFILE AS… | Return (`Joystick0`, reserved) | 116 | **`27`** | upd `I` → top `B` |
| (the reserved box) | dismiss (Return) | 119 | `-` | upd `B` → top `I` (the reopened box) |
| SAVE PROFILE AS… (reopened) | Esc | 122 | `27,27` | |
| WEAPON 1 | Return (`uzi`) | 156 | `27` | `wp[0] = 38` |
| WEAPON 1 | empty Return | 162 | `27` | unchanged |
| WEAPON 1 | Esc (`fan` typed) | 172 | `27` | unchanged |
| JUMP key capture | `Y` | 184 | `-` | `ex[6] = 21` |
| JUMP key capture | Esc | 190 | `-` | unchanged |

Every push frame plays one `27`. NAME 2/2/2, SAVE PROFILE AS… 2/2/**1**, WEAPON n 1/1/1, key capture 0/0 — exactly fact 10 and D4.

### P4 — play: CONFIRMED (first match; the DIG rule)

- `p_first` (`match_seed 4101`): NEW GAME, both DONE, 100 frames, Esc; F5 → FIRE (ssel 14) → `RCTRL` on 260: player 1's `ex[4] = 117`, equal to player 2's FIRE (117 = `Right Ctrl`); F1 (RESUME) on 266, back in play on 300. `RCTRL` down on 314: **`w0 clean=10`, `w1 clean=00`**; up on 324: both `00`. Player 2's clean word never moves. (`cs` stays `00` because the dead worm's `PressedOnce(kFire)` consumes the bit, `worm.cpp:435`; `clean` is `OnKey`'s own record of `FindControlForKey`.)
- `p_dig` (`match_seed 4201`; WEAPON 1 → 16, DIG = `Q` before NEW GAME, so `ex[7] = 16`, `wp[0] = 16`): in play (`ws=0`), worm 0:

  | Frame | Event | `clean` | `cs` |
  |---|---|---|---|
  | 222 | `Q` down | `80` | `0c` |
  | 227 | `LCTRL` down (Q held) | `90` | `0c` |
  | 230 | `LCTRL` up | `80` | `0c` |
  | 234 | `D` down (Left) | `84` | `0c` |
  | 240 | `Q` up (D held) | `04` | `04` |
  | 246 | `D` up | `00` | `00` |

  While `Q` is held every event leaves `cs & 0x0c == 0x0c`; after `Q`'s release with `D` held, Left stays and Right is released. Worm 1 stays `00` throughout. D1's `apply_clean_edges` both arms are as written.

### P5 — profiles: CONFIRMED (the unreadable shape: desk only)

`pr_io` (`fs_broken.txt`):
- **LOAD PROFILE** (ssel 3, Return on 46, one `27`): top `F`, `title=[Select profile: ./user/Profiles] sel=0 rows=[AI (L)][AI (R)][broken][Joystick0][Joystick1][Lefty (L)][Lefty (R)][Righty (L)][Righty (R)]` — opened inside `Profiles`, `.toml` only, `CiLess` order, both layers merged, the cursor on the first row (`ppm/pr_io/f_0047.png`). Each Down plays `25`.
- **`Lefty (L)`** (Return on 66, `27`, top `M`): `name=etc`, `rgb=160,40,220` (×4), `col=32` kept, `keys`/`ex` = W/S/A/D/Y/U/I (`…,0`), `wp=19,36,9,40,31`, `rn=0`, `prof=./user/Profiles/Lefty (L).toml` (the merged config node's path; the file is in the system layer). `ssel` stays **3** although PROFILE LOADED and SAVE PROFILE appear above it (pitfall 20).
- **SAVE PROFILE AS… `Joystick0`** (Return on 87, one `27`): the black box reads `NAME 'Joystick0.toml' IS RESERVED` (`f_0088.png`); dismissed on 92, the reopened box holds `Joystick0_` (`f_0094.png`). Backspace ×9, `mine`, Return on 129: `27,27`, `prof=./user/Profiles/mine.toml`.
- **SAVE PROFILE** (ssel 1, Return on 137): one `27`, `prof` unchanged (`mine`). The rewrite of `mine.toml` is not observable (same bytes); the `Joystick0` save below proves the write path.
- **LOAD PROFILE `broken`** (the listing now has `[mine]` between `Lefty (R)` and `Righty (L)`; Return on 157): `prof=./user/Profiles/broken.toml`, **no other field changes** (R2-18's parse-failure shape).
- **LOAD PROFILE `Joystick0`** (176): `name=chucky`, `in=1`, `rgb=104,104,252`, `col=32`, `prof=./user/Profiles/Joystick0.toml`. **SAVE PROFILE** (187, `27`) writes the user copy.
- `file` lines: `Profiles/Joystick0.toml`, `Profiles/broken.toml`, `Profiles/mine.toml`, `Setups/liero.cfg` — no `Lefty (L).toml` (loading writes nothing, R-5).
- The unreadable `.toml` was not attempted (root reads a `chmod 000` file, fact 20): **desk only**.

### P6 — pads: CONFIRMED

`j_pad` (`fs_prof.txt`, `match_seed 4601`):
- LOAD `Joystick0` (57): `in=1`, rows `[5:Gamepad (none)][6:Up][7:Down][8:Left][9:Right][10:RT+][11:RB][12:A][13:LB]`.
- `F` on 62 (player 1's Down, now a pad player): `ssel 3 → 4`, sound `25` — the network player's default keys drive the menu (R2-21a).
- INPUT (ssel 9): **Left** on 80 → sound `26` (MoveDown), `in 1 → 0`, rows `Keyboard`, `R`, `F`, `D`, `G`; re-load `Joystick0` (114, `in=1`); **Right** on 137 → `25` (MoveUp), `in=0`; re-load (171); **Enter** on 194 → `27`, `in=0`; re-load (228, `in=1`). Every one lands on `Keyboard`; LOAD makes it a pad player again.
- NEW GAME (F1 on 236): weapon selection from 270. Player 2's `UP`, `RCTRL` (271, 274) set `w1 clean=01`, `10`; player 1's `R`, `LCTRL` (277, 280) leave `w0 clean=00`. 165 `W` frames, `ws=1` until Esc (403); the menu is back on 435. Player 1 never readies (R2-21b).

### P7 — live edits: CONFIRMED (RD-2 holds)

- `l_att` (`fs_install.txt`, `match_seed 4701`): after 150 frames of play, Esc; F5 → NAME `LIVE` (close on 351, `27,27`) → FIRE → `K` (387, `ex[4] = 37`); F1 on 393. In play: `K` down on 441 → `w0 clean=10 cs=10`, sound `3` (a shot); `LCTRL` on 457 → nothing. The match's worm 0 has `same=1 nm=LIVE fire=37`: the attached rename and rebind act from the first resumed tick.
- `l_det` (`match_seed 4702`): as `l_att`, then Esc; F7 → Up (LOAD SETUP, ssel 18) → Return (522, top `P`, `Select options: ./user/Setups`, rows `liero`, `orbmit`) → Down → Return on `orbmit` (530): `gfx.settings` is a fresh object (`w0 name=""`, `ex=…,29,…`), while the match's worm keeps `same=0 nm=LIVE fire=37`. F5 → FIRE → `J` (572, the new object's `ex[4] = 36`); F1 on 578. In play: **`K` (626) → `w0 clean=10 cs=10`, sound `3`; `J` (642) → nothing.** The detached match keeps the old bindings and name.
- `l_sel` (`match_seed 4703`): NEW GAME, Esc during weapon selection (83), F5 → NAME `SEL` (202); F1 on 205; selection is back on 239 (fade 1). Frame 240 (fade 2, brightened ×16: `ppm/l_sel/b240.png`) and frame 275 show `SEL` in player 1's name box; frame 100 (before the rename) shows an empty box.

### P8 — F9's slot 0: CONFIRMED

`n_slot`: F9 → Red (ssel 6), `LEFT` held 55–95 → the network player's red steps 104 → 64 every 4 frames (56, 60, …, 92), player 1's stays 104. `pal0` (entries base−2 … base+2):
- frame 40 (`cur N`, no edit yet): `3c3c94,5050c4,6868fc,8888f8,b4b4f8` — player 1's ramp (fact 6: equal defaults);
- frames 56–92 (`cur N`): follows the network player's red (`383c94,…` at 56 … `243c94,3050c4,4068fc,6c88f8,a4b4f8` at 92);
- frame 98 (Esc → `cur M`) and 114 (Esc again): player 1's ramp;
- frame 106 (F9 again, `cur N`): the network player's edited ramp at once.

### Restore

- `git checkout -- src/tools/oracle_dump/shell_dump.cpp`; both builds rebuilt (exit 0). `git status --short src data rust` empty; `git status --porcelain rust/oracle-tests/golden` empty; no `/tmp/oracle_shell_fs_*` left.
- The restored binaries regenerate the four goldens into `$S/t0f2/restore/` (`oracle_dump_shell rust/oracle-tests/golden/<case>_script.txt $S/t0f2/restore/<case>_{rel,chk}.txt`, checked with `ASAN_OPTIONS=detect_leaks=0`), and `cmp` each against `rust/oracle-tests/golden/<case>.txt`:

  | Golden | Release | Checked |
  |---|---|---|
  | `shell_boot_idle.txt` | SAME | SAME |
  | `shell_milestone.txt` | SAME | SAME |
  | `shell_setup_save.txt` | SAME | SAME |
  | `shell_cpu_match.txt` | SAME | SAME |

### Changes to later tasks

**None is required.** No probe contradicted the plan, so no Step-5 rule fired: D1 (P4), D3 (P2), D4 (P3), D5/D14 (P5), D2's INPUT (P6), RD-2 (P7) and fact 5 (P8) stand, and T2 may start. Precisions for the tasks that use these results (no rule changes):
- **T0 Step 2** (this task's own text): the verbatim range is `sed -n 26,54p`, not `26,53p` (`#undef MIN3` is line 54).
- **T1:** embed the tables above as they are: the 177 `key_names` hex entries, the `GetKeyName` / `GetGamepadKeyName` / controller vectors (including the extra `GetKeyName(1000) = "J15_8"`, `(41) = "½"` and the axis names `A6±`, `A7+`, `A15+`), the 40 × 8 Levenshtein table plus the classics, and the eight menu results (`zzzzzzzzzz` → 4 is a second tie witness besides `la` → 25).
- **T3:** the `K` push frame is upd `M` → top `K` with one `27`; each `K` frame presents the push frame's pixels; the pop frame is upd `K` → top `M` with no sound. The `d` line keeps `cur 1|2|N` and the player menu's `ssel` while `K` is on top.
- **T4:** the profile selector's title in an `fs` case reads `Select profile: ./user/Profiles` (the same config-root string Rust's options selector already prints as `Select options: ./user/Setups`); C++'s `profile_node` for a shipped profile is `./user/Profiles/<leaf>.toml` (the merged root), so D5's `ProfileRef { rel: "Profiles/<leaf>.toml" }` and SAVE PROFILE's `user/Profiles/<leaf>` hold for both layers.
- **T7:** a Fire press is visible in `clean_control_states`, but the dead-worm branch can consume it from `control_states` (`p_first`); witnesses on Fire should rest on `state8`, not on the word alone. RESUME needs 34 frames before a key reaches the match (F1 on 266, `G` on 300 in `p_first`; 393 → 427 in `l_att`: the menu's fade-out): the generator's waits must allow for it.
