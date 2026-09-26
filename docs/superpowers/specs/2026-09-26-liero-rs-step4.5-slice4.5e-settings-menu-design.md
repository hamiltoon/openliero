# Step 4½, Slice 4½e — the settings menu, weapon options, the level selector and setup files: design

Status: **DESIGN — rulings in §14** · 2026-09-26 · branch `claude/cpp-oracle-vcpkg-assets-chcwcm` (on `liero-rs-step-4-5`; 4½a ✅, 4½b ✅, 4½c-0 ✅, 4½c ✅, 4½d ✅ landed) · **4½e-1 LANDED** (plan `plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md`); 4½e-2 planned
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (the 4½e bullet, §4½h, §Deferrals, open Q5/Q6; cited **overview**)
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` §1–§5 (cited **cpp-map**) and
`2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (cited **rust-map**)
Precedents: `2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md` (cited **4½d design**; its findings are
**4½d F1…F16**) and its plan `plans/2026-09-26-liero-rs-step4.5-slice4.5d-plan.md` (cited **4½d plan**);
`2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` (cited **4½a design**)
Next artifact: `plans/2026-09-2x-liero-rs-step4.5-slice4.5e1-plan.md` (and `…4.5e2-plan.md` if Q1 splits the slice)

4½d boots Rust like C++: the main menu, NEW GAME, weapon selection, play, pause, RESUME, QUIT. It draws the settings
menu disabled beside the main menu, but MATCH SETUP (F7) is inert, so nothing in the menu can change a setting, and the
binary reads and writes no config file. 4½e makes the settings menu live: every `SettingsMenu` item and behaviour,
number entry, WEAPON OPTIONS, the level selector with its RANDOM node, cursor restore and minimap preview, SAVE
SETUP AS… / LOAD SETUP, the `InputStringState` and `InfoBoxState` overlays, and `liero.cfg` loaded at boot and saved
at exit. The standing rule is John's: *it should work like the original OpenLiero.* So, as in 4½c and 4½d, every
screen is gated bit-exact against the **real C++** run headlessly in this session, and the sim consequences
(settings → match, the first map that is not 504×350) against the C++ sim oracle. Self-goldens are not used.

---

## 0. Summary of findings (read this first)

These were read out of the C++ source for this slice. Items 1, 2, 3, 5, 7, 9 and 12 contradict or sharpen the
overview, the maps or 4½d.

1. **Menu edits reach a paused match.** C++ `Game` holds `std::shared_ptr<Settings> settings`, the same object as
   `gfx.settings` (`game.hpp:124`; `LocalController(common, settings)` → `game(common, settings, …)`,
   `controller/localController.cpp:31-32`). The settings menu edits that object in place (`gfx.cpp:1262-1312`). Pause a
   match, change LOADING TIMES, AMOUNT OF BLOOD, MAX BONUSES, GAME MODE, TIME TO LOSE or a weapon's availability, and
   the resumed match runs with the new value from its next tick. The sim reads these through `settings->` every tick:
   `game_mode` and `time_to_lose` (`game.cpp:385`, `:531`, `:537`), `max_bonuses` (`:219`, `:359`), `weap_table`
   (`:256-258`), `lives` (`:159`), `blood` (`nobject.cpp`, `sobject.cpp`, `weapon.cpp`, `worm.cpp`), `loading_time`
   (`weapon.cpp`), `load_change` (`worm.cpp`), and the draw reads `map` and `names_on_bonuses` (`viewport.cpp:408`,
   `:466`, `:593`). Rust's builder copies these into `SimState` once (`scenario/src/build.rs:236-242`,
   `new_match_with`), so today a Rust RESUME would ignore the edits. **LOAD SETUP breaks the sharing:**
   `Gfx::LoadSettings` replaces `gfx.settings` with a new object (`gfx.cpp:1693-1697`), so the paused game keeps the old
   one, and later menu edits no longer reach it. §4.9 ports both halves. This amends 4½d LD 3 ("the router only
   replaces the state"): RESUME now also writes the live-read settings fields into the running `SimState`.
2. **Picking a shipped level in a normal install plays a random level.** The level selector fills its tree from
   `gfx.GetConfigNode()` (`fileSelectorState.cpp:80-81`). In the default layout that node joins the user folder over
   the shipped data, and its `FullPath()` is the user folder's (`FsNodeJoin::FullPath` returns `a->FullPath()`,
   `filesystem.cpp:356-362`). A node's `full_path` is that string joined with the names below it
   (`fileSelector.hpp:137`), and `OnSelected` stores it as `level_file` (`fileSelectorState.cpp:93-103`). NEW GAME opens
   it with a plain `FsNode(path)` (`level.cpp:401-411`), which looks only in the user folder. A level that exists only
   in the shipped data fails to open, and `GenerateFromSettings` quietly generates a random level instead (`:416-418`),
   while LEVEL still shows the file's name. The preview works, because it reads through the joined node
   (`fileSelectorState.cpp:113`). Portable mode, `--config-root` and the C++ web build (`--config-root /openliero`,
   `gameEntry.cpp:25-28`) have one layer and are not affected. CMake ships no `portable.txt`, so a default native C++
   install has the bug. **Open Q4 (John).** The plan's first C++ task confirms it with a probe before anything relies
   on it.
3. **`level_file` is a config-root path, not TC-relative.** Level-selector paths start with the config root's
   `FullPath()`: `/home/<u>/.local/share/openliero/openliero/TC/openliero/Levels/water_stage.lev` natively, and
   `/openliero/TC/openliero/Levels/…` on the C++ web build. `level.cpp:401-404` appends `.LEV` when the name has no
   dot. The selector's title shows the same string (`fileSelectorState.cpp:147-151`). Rust today reads `level_file`
   relative to the TC (`ui/src/shell/new_game.rs:56-58`, the 4½d `?level=` and `shell_level_file` convention, which the
   dumper matches with CWD = the TC, 4½d intervention 8). §4.6 keeps that as a legacy rule and adds the C++ form.
4. **The level selector lists the whole config tree.** `FileNode::Fill` adds **every** directory and only the files
   the filter accepts (`fileSelector.hpp:132-154`). The root therefore shows `[RANDOM]` and then Profiles, Resources,
   Setups and TC, plus any user folder such as Replays. Reaching a level takes TC → openliero → Levels. Folders sort
   first, then case-insensitive by name (`ChildSort`, `:121-130`). Each listing is byte-sorted and de-duplicated across
   the two layers first (`DirectoryListing::Sort`, `filesystem.cpp:335-340`). A `.zip` counts as a folder (`:314-318`).
   The RANDOM node is inserted after sorting, at index 0 (`fileSelectorState.cpp:78-84`). Folder rows use colour 47,
   files 48 (`fileSelector.hpp:56`).
5. **The RANDOM node has no preview, and the preview lands one frame late and stays.** `DrawExtra` previews only a
   non-RANDOM file that differs from the last previewed node (`fileSelectorState.cpp:109`). It draws **into
   `frozen_screen`** (`:124-125`), *after* `FileSelectorState::Draw` has copied `frozen_screen` to the surface
   (`:53`, `:67`). So a new miniature appears on the next frame, and it stays in the frozen screen after the selector
   closes. The main menu keeps showing it until the next `MainMenuState::Enter` recaptures the background. The miniature
   sits at (134, 162), where the HUD minimap is, with the HUD step `ceil(w/52) × ceil(h/36)`. It first clears the
   previous footprint, and on the first preview that is the full 52×36 (`:89-90`, `:124-127`). It goes through the
   frozen screen's `pal32`, which is the live play-renderer palette (`Bitmap::Copy` copies the pointer,
   `gfx/bitmap.hpp:56-60`), so it bakes that frame's menu rotation. The overview's Q5 ("show the RANDOM node's generated
   preview") is corrected: C++ shows none.
6. **Cursor restore is a string match on `level_file`.** `Select(level_file)` walks the tree with case-insensitive
   `full_path` prefix tests, opens the file's folder, and moves every ancestor's cursor onto the path
   (`fileSelector.hpp:184-214`). A random level (empty `level_file`) or a path under another root finds nothing, so the
   selector opens at the root on `[RANDOM]`.
7. **WEAPON OPTIONS closes on Esc/Jump, never on Enter.** `WeaponMenuState` (`weaponMenuState.cpp:26-127`) is a
   40-row menu at (179, 28), height 14, `value_offset_x = 89`, one row per weapon in `weap_order`. Each row is an
   `ArrayEnumBehavior` over `weap_table[weap_order[i]]` with `weap_states` = Menu / Bonus / Banned
   (`common.cpp:222-224`). Here Left and Right are *once* keys (`TestSdlKeyOnce`, `:66-76`), not held as in the main
   menu. PgUp/PgDn page. Type-to-search is live, prefix mode (`:89`). Esc, P1/P2 Jump or pad B/A close it (`:91-110`)
   if at least one weapon is in state 0 ("Menu"). Otherwise an `InfoBoxState(NoWeaps, 223, 68, clear_screen = false)`
   is pushed (`:108`). `NoWeaps` is two lines split by a NUL (`tc.cfg:258`). Enter is ignored. The screen is
   `frozen_screen` + `DrawBasicMenu` + the "Weapon" / "Availability" headers + the menu (`:114-127`); the settings menu
   is not drawn.
8. **`InfoBoxState` is not an overlay and draws over a stale surface.** Only `InputStringState` overrides `IsOverlay`
   (`inputState.hpp:23`). While an info box is on top, `StateStack::Draw` draws only the box (`state.hpp:117-131`), on
   top of whatever the surface held from the previous frame (`inputState.cpp:202-217`). The WEAPON OPTIONS box
   therefore sits on a frozen image of the weapon menu. The reserved-name box passes `clear_screen = true`, so each frame
   it switches the palette to the raw `exepal` and fills black first (`:203-207`). Any key-down dismisses the box,
   including an OS repeat (`:176-182`).
9. **Number entry accepts any value in range, not only the step.** Enter on an `IntegerBehavior` with `allow_entry`
   (LIVES, LOADING TIMES, MAX BONUSES, MAP WIDTH, MAP HEIGHT; FLAGS TO WIN is never visible) plays `MenuSelect` and
   pushes a digit-only `InputStringState` at the value's position (`integerBehavior.cpp:36-80`). On accept it clamps to
   `[min, max]` and writes, so MAP WIDTH 333 is reachable although Left/Right step by 8. Its callback rewrites only
   `item.value`, not `UpdateItems` (`:60-75`). AMOUNT OF BLOOD (`allow_entry = false`, `gfx.cpp:1275-1279`) and every
   `TimeBehavior` (`timeBehavior.hpp`) only play the sound. `InputStringState` plays `MenuSelect` again when it closes
   (`inputState.cpp:75-84`).
10. **Typing is SDL text input.** `InputStringState` edits on `SDL_EVENT_TEXT_INPUT` through `Utf8ToDos` (ASCII plus
    å ä ö Å Ä Ö, `text.cpp:99-118`), up to `max_len`, through an optional filter. Backspace deletes, Return/KP Enter
    accepts, and Esc cancels (`inputState.cpp:27-72`). It draws `prefix + buffer + '_'` in colour 50 in a rounded
    box, after restoring its strip from `frozen_screen` (`:86-97`). On close it clears every key (`ClearKeys`, `:79`),
    so neither the Return nor the typed letters reach the menu underneath.
11. **SAVE SETUP AS… starts from the current name, so a bare Enter is always refused.** The dialog opens on
    `GetBasename(settings_node)`, which is `liero` after boot (`mainMenuState.cpp:290-311`). `MakeSaveAsState`
    (`:69-91`) refuses `Setups/liero.cfg` (reserved) and any name the shipped data already has, through
    `paths::ShadowsSystem` (`filesystem.cpp:736-763`). The box `NAME '<leaf>' IS RESERVED` is shown with
    `clear_screen = true` at (160, 100) and replaces the input. Dismissing it reopens the input with what was typed.
    Saving sets `settings_node`, so SAVE SETUP AS… then shows the new name (`gfx.cpp:1688-1691`). Accept and cancel
    both play `MenuSelect` twice: once when the input closes and once in the completion (`mainMenuState.cpp:300-307`).
12. **Exit always saves `Setups/liero.cfg`, whatever setup was loaded.** Boot runs `LoadSettings(config/Setups/
    liero.cfg)`. When that fails it uses defaults and writes them to the user folder (`gameEntry.cpp:55-58`). The exit
    save is `settings->save(user/Setups/liero.cfg)` after `MainLoop` returns (`:78`), i.e. after QUIT TO OS, a window
    close or Alt+F4. After LOAD SETUP `orbmit`, the menu shows `orbmit`, but the next boot shows `liero` with orbmit's
    values. The C++ web build never reaches that line: `emscripten_set_main_loop_arg(…, simulate_infinite_loop =
    true)` does not return (`gfx.cpp:1658-1670`), and its `/openliero` is in-memory MEMFS, so setups saved in the
    browser last only for the session. `Settings::load` / `save` take a `Rand&` but never draw it
    (`settings.cpp:62`, `:158`). Setup files are RNG-free.
13. **LOAD SETUP opens inside `Setups`.** `OptionsSelectorState` fills the whole config tree with `.CFG` files and
    selects `JoinPath(root, "Setups")`, a folder, so it opens there with the cursor on the first entry
    (`fileSelectorState.cpp:212-220`). Its title is framed text: `"Select options: " + full_path` (`:52-66`). Selecting
    a file calls `LoadSettings` and `UpdateItems` (`:222-226`). A file that fails to parse still replaces `gfx.settings`
    with a half-read object whose missing player tables are null (`settings.cpp:62-90`), and C++ would crash at the
    next NEW GAME.
14. **The settings focus lives inside `MainMenuState`.** It is not a separate state. `cur_menu` switches between
    `main_menu` and `settings_menu` (`mainMenuState.cpp:202-205`, `:455-458`). While settings has focus:
    - Esc/Jump returns focus to the main menu (`:171-179`);
    - Up/Down/PgUp/PgDn and held Left/Right act on the settings menu (`:181-192`, `:581-602`);
    - Enter dispatches LEVEL, WEAPON OPTIONS, LOAD SETUP and SAVE SETUP AS… itself and sends every other item to the
      behaviour's `OnEnter` (`:272-316`);
    - F1 jumps straight to NEW GAME / RESUME (`:432-436`);
    - `Draw` draws the main menu disabled with its selection shown and the settings menu enabled (`:614-626`,
      `gfx.cpp:1699-1704`).

    The main menu's `settings_menu.MoveToFirstVisible()` runs in every `Enter` (`:123-125`), so the settings cursor
    survives Esc/F7 inside one menu visit and resets after a match.
15. **`DrawTextSmall` is unported, and the settings menu makes it reachable.** C++ draws small A–Z labels
    (`text.tga`, `common.cpp:227-237`) in three places:
    - above weapon bonuses when NAMES ON BONUSES is on (`viewport.cpp:408-413`);
    - above booby traps (`:466-479`);
    - above a worm **while its Change key is held** (`:575-581`).

    Rust omits all three (`render/src/object_draw.rs:247-250`). The last one is a live-play gap today, and the 4½d G2
    corpus never holds a Change key (LSHIFT / RALT appear in no `shell_*_script.txt`), which is why it went unseen.
16. **TIME TO LOSE and TIME TO WIN bind the same field, and that is consistent.** Both are
    `TimeBehavior(time_to_lose, 60, 3600, 10)` (`gfx.cpp:1284-1286`). Only one is visible at a time (Game of Tag shows
    TIME TO LOSE, Holdazone shows TIME TO WIN, `:1314-1341`), and `IsGameOver` uses `time_to_lose` as the limit for
    both modes (`game.cpp:531`, `:537`). ZONE TIMEOUT is separate (10..3600 step 10). FLAGS TO WIN is always hidden.
    This is a port-verbatim item, not a bug.
17. **Holdazone becomes choosable.** GAME MODE cycles through all four modes (`ArrayEnumBehavior` over `game_modes`),
    and a loaded setup can carry it. The Rust sim's Holdazone arm is `unimplemented!()` (`sim/src/state.rs`, overview
    §Deferrals), and `Match::start` would panic in `expect("the settings build a match")`
    (`ui/src/shell/playing.rs:59`, `:126`). The same applies to unequal player health, which a C++-saved setup can
    carry until 4½f (4½a interim ruling). **Open Q2 (John).**
18. **Nothing in 4½e draws `gfx.rand`.** Previews load levels without RNG. Setup files and number entry are RNG-free,
    and `Settings::GenerateName` belongs to the player menu (4½f). 4½d's intervention 2 (reseed while the main menu is
    up) stays valid with the new sub-screens on top of it.

---

## 1. Goal / done-when

**Goal.** Every setting the C++ settings menu offers can be changed from the Rust menu, with the same keys, screens,
sounds and quirks. The changes reach the next match (and a paused one, as in C++). Levels can be picked from files or
left RANDOM, and named setups can be saved and loaded. `liero.cfg` persists between runs on desktop. Every presented
frame on those paths is bit-exact against the real C++ frame loop, and the sim is bit-exact against C++ on maps of
other sizes.

**Done when** (numbered by sub-slice, per the Q1 recommendation; merged if John prefers one slice):

**4½e-1: settings menu, weapon options, entry, `liero.cfg`, labels**

1. **G2e-1** (§6.2). Every new `shell_*` case matches the real C++ `Gfx::RunOneFrame` on every frame, including the new
   `d` lines (focus, sub-screen cursor, and the match's `HashGameState` on every match frame). The saved files match
   byte for byte.
2. **🎯 MILESTONE e-1** (`shell_match_setup`, §6.4). Boot from a fixture `liero.cfg` → F7 → change GAME MODE, LIVES
   (typed), LOADING TIMES (held), MAP WIDTH/HEIGHT (typed 333 × 211) → WEAPON OPTIONS: ban all, get the info box,
   re-enable one → NEW GAME → selection → 300 match ticks → Esc → change AMOUNT OF BLOOD and MAX BONUSES → RESUME →
   200 ticks → QUIT → `liero.cfg` saved. Every frame, every state hash and the saved bytes are bit-exact.
3. **G3** (§6.5). Four settings-driven sim goldens on generated levels of sizes other than 504×350 are bit-exact on all
   12 columns (the first non-504×350 sim gate).
4. The live game loads `liero.cfg` at boot and saves it at exit natively (§7.1). The browser keeps settings for the
   session (§7.2). The keyboard and the touch controls reach every 4½e-1 screen (§7.3–§7.4).
5. The three `DrawTextSmall` labels are drawn in the shell and the live game. Every prior render golden is
   byte-identical, because they sit behind a `Scene` flag (§4.10).

**4½e-2: level selector and setup files**

6. **G2e-2** (§6.3). The level selector (root listing, folders, RANDOM, cursor restore, type-to-search, the late and
   persistent preview), LOAD SETUP, SAVE SETUP AS… with both refusals, and a file level played by NEW GAME are
   bit-exact on every frame. The user folder's files match byte for byte.
7. **🎯 MILESTONE e-2** (`shell_setups_and_levels`, §6.4). LEVEL → TC/openliero/Levels → water_stage → NEW GAME (the
   file is played) → Esc → LEVEL reopens on water_stage → SAVE SETUP AS… `liero` (refused) → `mine` → LOAD SETUP
   `orbmit` → NEW GAME → QUIT. Every frame is bit-exact, and so are the files `mine.cfg` and `liero.cfg`.
8. The level selector lists the embedded levels on wasm. `?level=` stores the canonical path, so LEVEL shows it and
   the selector opens on it (§7.5).

**Both**

9. Every prior golden is byte-identical (`git diff --name-status` shows only `A` under `golden/`, plus the regenerated
   4½d shell goldens, which must come out byte-identical).
10. Green: `cargo test --workspace --exclude game` (debug), `cargo test -p game`, the wasm build, and clang-format 22
    plus clang-tidy on the dumpers. `ui` stays Bevy-free.
11. Eyeball artefacts (not gates): Xvfb C++ | Rust PNGs of both milestone paths, and a headless-Chromium walk on
    desktop and an emulated phone.
12. PROGRESS, the overview (4½e bullet, Q5 correction, Q6 status) and the maps carry the §0 corrections.

---

## 2. Inherited locked decisions, and the ones this slice amends

- **LD 1:** pixel-exact menus. Everything draws on the 320×200 CPU surface with `render`.
- **LD 3, amended (finding 1).** `tick_and_render` stays the only `ResMut<Sim>` holder, and menus get no `SimState`.
  The router already replaces the state at NEW GAME. It now also runs one narrow write at RESUME:
  `scenario::build::apply_live_settings(&mut SimState, &Settings)`, which copies exactly the fields C++ reads through
  `game.settings` per tick (§4.9). `new_match_with` calls the same function, so "built" and "resumed" cannot disagree.
- **LD 4, amended.** The scenario format stays frozen for replayable scenarios. G3 adds one more **oracle-only**
  directive, `generate <level_seed>`, of the same class as 4½a's `settings` and 4½c's `weapsel`: `scenario::load`
  refuses it (§6.5).
- **LD 6:** the level seed equals the match seed (4½d `SeedSource`), unchanged.
- **4½d §6 posture:** the C++ `Gfx` frame loop is the pixel oracle, with documented interventions only where no real
  code runs. 4½e adds a file-system fixture (no code intervention, §6.1) and an exit-save replica of
  `gameEntry.cpp:78`.

---

## 3. The C++ surface, precisely

### 3.1 `SettingsMenu` items (`gfx.cpp:485-503`, behaviours `:1262-1312`, visibility `:1314-1341`)

At (178, 20), `value_offset_x = 100`, all items colour 48 / disabled 7, height 15. They are listed here in
`LoadMenus` order, with the Enter action from `mainMenuState.cpp:272-316`:

| # | Item (id) | Behaviour | Range / step | Enter | Visible |
|---|---|---|---|---|---|
| 0 | GAME MODE (0) | `ArrayEnum(game_mode, game_modes)` | 0..3, wraps | `MenuSelect`, next mode, `UpdateItems` | always |
| 1 | TIME TO LOSE (2) | `Time(time_to_lose)` | 60..3600 / 10 | sound only | Game of Tag |
| 2 | TIME TO WIN (3) | `Time(time_to_lose)` (**same field**) | 60..3600 / 10 | sound only | Holdazone |
| 3 | ZONE TIMEOUT (4) | `Time(zone_timeout)` | 10..3600 / 10 | sound only | Holdazone |
| 4 | FLAGS TO WIN (5) | `Integer(flags_to_win)` | 1..999 / 1 | entry | never |
| 5 | LIVES (1) | `Integer(lives)` | 1..999 / 1 | **digit entry** | Kill'em All, Scales |
| 6 | LEVEL (11) | `LevelSelect` (value `Random` or `"<name>"`, relabels #16) | — | **push level selector** | always |
| 7 | MAP WIDTH (12) | `Integer(random_map_width)` | 64..4096 / 8 | **digit entry** | `random_level` |
| 8 | MAP HEIGHT (13) | `Integer(random_map_height)` | 64..4096 / 8 | **digit entry** | `random_level` |
| 9 | LOADING TIMES (6) | `Integer(loading_time, %)` | 0..9999 / 1 | **digit entry** | always |
| 10 | WEAPON OPTIONS (15) | plain | — | **push `WeaponMenuState`** | always |
| 11 | MAX BONUSES (7) | `Integer(max_bonuses)` | 0..99 / 1 | **digit entry** | always |
| 12 | NAMES ON BONUSES (8) | `BooleanSwitch` | — | toggle | always |
| 13 | MAP (9) | `BooleanSwitch` | — | toggle | always |
| 14 | AMOUNT OF BLOOD (10) | `Integer(blood, %)`, `allow_entry = false` | 0..`BloodLimit` / `BloodStepUp` | sound only | always |
| 15 | LOAD+CHANGE (18) | `BooleanSwitch` | — | toggle | always |
| 16 | REGENERATE LEVEL / RELOAD LEVEL (14) | `BooleanSwitch(regenerate_level)` | — | toggle | always |
| 17 | SAVE SETUP AS… (17) | `OptionsSave` (value = setup name) | — | **push Save-As input** | always |
| 18 | LOAD SETUP (16) | plain | — | **push options selector** | always |

Held Left/Right runs `OnLeftRight`. Integer and Time act only when `menu_cycles % 5 == 0` and return true, so a held key
repeats at that cadence. Bool and Enum play MoveUp/MoveDown, act, and return false; `ResetLeftRight` then releases
Left, Right and both players' Left/Right controls (`mainMenuState.cpp:56-61`). All of this is already ported in
`ui::shell::settings_menu` and gated by 4½d G1 (`menu_settings_*`). 4½e adds focus, the Enter dispatch and the
pushes.

### 3.2 `MainMenuState` with settings focus (`mainMenuState.cpp:152-612`)

The additions to 4½d's key table (4½d §7.2), in source order:

| Key | Settings focus | Main focus (new in 4½e) |
|---|---|---|
| Esc / any Jump | focus → main menu (no sound) | cursor → QUIT (4½d) |
| Up / Down (+ P1/P2 up/down) | settings `Movement(∓1)` + MoveDown/MoveUp | — |
| Enter / KP Enter / any Fire | `MenuSelect`, then the §3.1 Enter action | MATCH SETUP: `MenuSelect`, focus → settings |
| F1 | focus → main; `MoveToId(start)`; select (fade out) | (4½d) |
| F7 | `MoveToId(MATCH SETUP)`; focus → settings | same |
| held Left / Right | `OnLeftRight(∓1)`; `ResetLeftRight` on false | (4½d) |
| PgUp / PgDn | settings `MovementPage(∓1)` + sounds | (4½d) |

LEVEL, WEAPON OPTIONS and LOAD SETUP play `MenuSelect` and then push. SAVE SETUP AS… plays `MenuSelect` and pushes only
if the item is in view (`ItemPosition`). Every other item goes through `settings_menu.OnEnter` with its result ignored
(`:314`).

### 3.3 Overlays (`inputState.cpp`)

- **`InputStringState(initial, max_len, x, y, filter, prefix, centered, cb)`** (`:13-97`), an overlay. Events: TEXT
  → filter → append; Backspace; Return / KP Enter = accept; Esc = cancel. `Update`: when done, `MenuSelect`,
  `ClearKeys`, `cb(accepted, buffer)`, pop. `Draw`: `BlitBitmap(bmp, frozen, x−10−adj, y, 10+w, 8)`,
  `DrawRoundedBox(x−2−adj, y, 0, 7, w)`, string colour 50 at (x−adj, y+1), where `w = GetDims(prefix+buffer+'_')`.
  Users in 4½e: number entry (`max_len = digits`, digit filter) and Save As (`max_len = 30`, no filter).
- **`InfoBoxState(text, x, y, clear_screen, on_dismiss)`** (`:166-217`), not an overlay. Any key-down is done.
  `Update`: `ClearKeys`, the optional black fill, `on_dismiss` (which may schedule a replacement), pop. `Draw`: when
  clearing, `pal = exepal` + `UpdatePal32` + fill 0; then `GetDims(text, &h)`, a box at `(x − w/2 − 2, y − h/2 − 2)`
  of size `(w+1, h+1)`, and the text in colour 6.
- `StateStack::Update` applies a scheduled replacement before the pop (`state.hpp:92-111`), which is how the Save-As
  chain works: input → box → input.

### 3.4 `WeaponMenuState` (`weaponMenuState.cpp`)

See finding 7. Update order: Up, Down, Left (once), Right (once), PgUp, PgDn, `OnKeys` (prefix search over `key_buf`),
then Esc/Jump/B/A → close-or-box. `Draw`: `bmp = frozen`, `DrawBasicMenu` (the main menu disabled, selection shown),
two framed headers at 179/249, and the menu.

### 3.5 File selectors (`fileSelectorState.cpp`, `menu/fileSelector.hpp`)

- `FileSelectorState::Update`: `Process` first (Up/Down/PgUp/PgDn with sounds; Esc/Jump/B → leave; once-Left =
  parent folder; once-Right = enter folder; `OnKeys(contains = true)`), then Enter/Fire/A: `MenuSelect`, then
  `selector.Enter()`. A folder is entered; a file goes to `OnSelected`, and true closes.
- `Draw`: `bmp = frozen`; the framed title if any; `DrawExtra`; the parent pane ("Parent directory" framed at (28, 20),
  the parent's menu at x = 28, disabled, selection shown); the current menu at x = 178. Each `FileNode` has its own
  `Menu(178, 28)`, height 14, built lazily.
- **Level selector** (`:73-156`): finding 4–6. `OnSelected`: RANDOM → `random_level = true`, `level_file = ""`;
  a file → `random_level = false`, `level_file = full_path`; then `settings_menu.UpdateItems`. Its title is its own:
  `DrawRoundedBox(178, 20, 0, 7, w)` + `"Select level: <full_path>"` at (180, 21), colour 50.
- **Options selector** (`:212-226`): finding 13.

### 3.6 Paths and files (`filesystem.cpp`, `gameEntry.cpp`)

`paths::Resolve` (`:767-845`) gives one layer (`--config-root`, `portable.txt`) or the user folder
(`SDL_GetPrefPath`, or `OPENLIERO_TEST_USER_DIR`, `:659-679`) joined over the system data (`OPENLIERO_DATADIR`, the
compiled-in path, or `SDL_GetBasePath`, `:681-700`). Reads go through the join and writes to the user folder.
`FsNode("x")` has `FullPath` `./x` (`:596-640`). `ShadowsSystem(user, subdir, leaf)`: the reserved `Setups/liero.cfg`
(case-insensitive leaf) → true; no system layer, or system == user → false; else "the system file exists". Rust's
`scenario::storage` already ports `Resolve`'s split, `pref_path` and `ShadowsSystem` (4½a-2).

### 3.7 Shared settings (finding 1)

C++ live-read set, the fields `Game` and the draw read through `settings->` after the match starts: `game_mode`,
`time_to_lose`, `zone_timeout`, `max_bonuses`, `weap_table`, `lives` (Scales re-grant, `game.cpp:159`), `blood`,
`loading_time`, `load_change`, `shadow`, `map`, `names_on_bonuses`, `allow_viewing_spawn_point`. Fields read only at
construction or at `kStateGame`: worm `health`, `lives` (`localController.cpp:232-235`), `blood_particle_max`
(`StartGame`), the level fields, and `select_bot_weapons`. The plan re-derives this list mechanically: a `grep`
over `settings->` in the files `Game::ProcessFrame` and `Game::Draw` reach, checked in as a table in the
`apply_live_settings` doc comment. The per-worm `WormSettings` are shared the same way (`worm1->settings =
settings->worm_settings[0]`, `localController.cpp:34`), which is 4½f's concern.

---

## 4. Design

### 4.1 Crates and modules

- **`render`**:
  - `font`: `get_dims_h` (width and height, `font.cpp:87-112`) and `draw_framed_text` (`:82-85`).
  - `blit`: `blit_bitmap` (the ARGB rectangle copy, `blit.cpp:217-233`).
  - `hud::draw_miniature` becomes `pub`.
  - `small_text`: `draw_text_small` over the `text.tga` bank (4×4, 26 frames; already parsed, `sprite_golden.rs:49`).
    The three labels are in `object_draw`, behind a new `Scene::labels` flag, false in `SceneData::as_scene` (§4.10).
- **`scenario`**:
  - `storage`: `ConfigStore` grows `list` and `root_label` (§4.6). `MemoryStore` derives directories from its keys.
  - `build`: `apply_live_settings` (§4.9); `validate(&Settings) -> Result<(), BuildError>`, the refusals without
    building.
  - `assets`: the wasm system layer (embedded `Setups/*.cfg` plus the embedded TC files under their config-root
    paths).
- **`ui`**:
  - `ui::shell::overlay`: `InputStringState`, `InfoBoxState`.
  - `ui::shell::weapon_options`: `WeaponMenuState`.
  - `ui::shell::files`: `FileNode`, `FileSelector`, `LevelSelectorState`, `SetupSelectorState` (C++
    `OptionsSelectorState`).
  - `ui::shell::level_path`: `level_file` ⇄ store resolution.
  - `MainMenuState` gains `cur_menu`.
  - `Shell` gains the store, the `settings_node` name, the refusal check and the RESUME sync.
- **`game`**: the store (native / wasm), boot load, exit save, text events, touch auto-repeat, the phone text field
  (per Q5), `--config-root`.

### 4.2 Screens and input

```rust
pub enum Screen {
    MainMenu(MainMenuState), Playing,
    WeaponOptions(WeaponMenuState),                 // 4½e-1
    InputString(InputStringState),                  // 4½e-1, the only overlay
    InfoBox(InfoBoxState),                          // 4½e-1
    LevelSelect(FileSelectorState), SetupSelect(FileSelectorState),   // 4½e-2
}
```

`is_overlay` is true for `InputString` only. `wants_menu_flip` is true for everything but `Playing`. The stack's
`schedule_replace_top` already exists (4½d `stack.rs`).

**`ShellInput.events`** becomes one ordered list of `InputEvent::{Key(KeyEvent), Text(String)}`, which is SDL's event
order. A frame that types "ab", Backspace and Return must apply them in that order. `KeyEvent` is unchanged. The glue
derives `Text` from Bevy `KeyboardInput.text` on key-down, and from the phone text field (§7.4).

**Continuations instead of closures.** C++ passes lambdas that capture `gfx`. Rust keeps a purpose tag on each
overlay, and the shell runs the continuation when the overlay pops (`AfterUpdate` carries the result):

```rust
pub enum InputPurpose {
    IntegerEntry { item_id: i32, min: i32, max: i32, div: i32, pct: bool },   // integerBehavior.cpp:56-76
    SaveSetupAs,                                                              // mainMenuState.cpp:290-311
}
pub enum InfoPurpose { NoWeapons, Reserved { typed: String }, Refused(BuildError) }
```

`IntegerEntry` writes the clamped value times `div` into the settings field named by `item_id` and rewrites only that
item's value string (finding 9). `SaveSetupAs` runs `MakeSaveAsState`'s callback: validate, then either schedule
`InfoBox(Reserved)` or save, then `MenuSelect` and `settings_menu.update_items`. `Reserved` on dismiss schedules a new
`InputString(SaveSetupAs)` with `typed` as its initial text.

### 4.3 `MainMenuState` focus

`cur_menu: CurMenu::{Main, Settings}`, with 4½f adding `Player` and 4½g `Hidden`. `update` follows §3.2 in C++ source
order, with the same `||` short-circuit rules as 4½d. MATCH SETUP stops being a placeholder. F7 is live. F2, F3, F5, F6
and F9 stay consumed and inert. `draw` is `DrawBasicMenu` (main disabled when focus is elsewhere, selection shown)
and then either the settings menu disabled (main focus) or `cur_menu` enabled.

### 4.4 `WeaponMenuState`

It is a plain `Menu` built in `enter` (40 rows from `sim::weapsel::weap_order`), with a model whose `behavior(id)` is
`ArrayEnum { v: &mut settings.weap_table[weap_order[id]], arr: &WEAP_STATES }` (`ui::text` gains `WEAP_STATES`,
finding 3 of 4½d). `update` follows §3.4. The live search uses `ShellInput.now_ms` (4½d §4.5). Close is refused with
`InfoBox(NoWeapons)` at (223, 68), no clear.

### 4.5 Overlays

`InputStringState { buffer, max_len, x, y, filter: Option<fn(u8) -> u8>, prefix, centered, purpose, done, accepted }`
and `InfoBoxState { text, x, y, clear_screen, purpose, done }` are line-for-line ports of §3.3. The buffer holds CP437
bytes, as C++ does (`Utf8ToDos`). The draw decodes them through the 4½d CP437 table. Filenames made from it convert
back to Unicode (§4.8). `InfoBox` with `clear_screen` swaps the shell's `pal32` for `exepal` for that frame's draw.
The next menu frame's `update_menu_palettes` restores the rotation, as C++'s does.

### 4.6 `ConfigStore` grows a listing; `level_file` resolution

```rust
pub struct DirEntry { pub name: String, pub is_dir: bool }
pub trait ConfigStore {
    fn read(&self, rel: &str) -> Option<Vec<u8>>;               // 4½a-2
    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()>;  // 4½a-2
    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool;  // 4½a-2
    /// DirectoryListing over the merged layers: byte-sorted, de-duplicated by name
    /// (filesystem.cpp:335-340). `rel = ""` is the root.
    fn list(&self, rel: &str) -> Vec<DirEntry>;
    /// The config node's FullPath(): what C++ prefixes onto selector paths and shows in titles.
    fn root_label(&self) -> &str;
}
```

- `NativeStore::root_label`: the user root with no trailing separator. That is C++'s `FsNode(pref_path)` form
  (§3.6), e.g. `/home/j/.local/share/openliero/openliero`. `single_dir` uses the directory as given. A
  `with_root_label` override exists for the oracle fixture (`./user`, §6.1).
- `MemoryStore`: directories are the key prefixes. The wasm system layer is a `MemoryStore::with_system` over the
  embedded files, with root label `/openliero` (the C++ web build's root, `gameEntry.cpp:27`).
- **Level catalogue = the file selector over the store.** No separate trait is needed: the selector takes
  `&dyn ConfigStore`, a filter (`LEV` / `CFG`, case-insensitive extension), and the RANDOM flag.

**Resolving `level_file`** (`ui::shell::level_path::read_level(store, tc_root, level_file) -> Option<Vec<u8>>`) goes
in order. `.LEV` is appended when the name has no dot (`level.cpp:402-404`).

1. **Root form.** It starts with `root_label()` + `/`: strip the prefix and `store.read(rest)` through the merged view.
   This fixes finding 2 when Q4 says so. With Q4 = copy the bug, it reads the user layer only.
2. **Absolute** (native only): read that file, as C++ `FsNode(path)` does.
3. **Relative, legacy:** TC-relative through `read_asset`, the 4½d convention that `?level=` links and the
   `shell_level_file` golden use.
4. Nothing reads, or the bytes do not parse: `None`, and `generate_level` falls back to random (`level.cpp:416-418`).
   It never panics. Today a missing file panics in `read_asset`.

`generate_level(tc_root, settings, seed)` becomes `generate_level(assets, settings, file: Option<LevelData>, seed)`.
The caller resolves the file and the generator stays I/O-free, as `sim::levelgen` already is. `LevelSlot` provenance
compares the raw `level_file` string, as C++ does (4½d F16).

### 4.7 The level selector (4½e-2)

`FileNode { name, full_path, folder, children: Vec<usize>, parent: Option<usize>, menu: Option<Menu>, filled }` lives
in an arena `Vec<FileNode>` owned by the `FileSelector`, so there are no `Rc` cycles. `fill` is `FileNode::Fill`
(finding 4): `store.list(rel)`, every directory, the files the filter accepts, and a `ChildSort` port (`CiLess`,
`text.cpp:81`). `select`, `cur_sel`, `enter`, `exit` and `process` are §3.5 verbatim. `full_path` strings are built
with `JoinPath` semantics from `root_label()`, so titles and `level_file` match C++ character for character.

`LevelSelectorState::draw_extra` is finding 5 verbatim. It writes into the shell's `frozen` bitmap through the current
`pal32`, *after* the surface copy, and keeps `prev_hud_cols/rows`. The preview reads bytes via
`store.read(rel_of(node))`, the joined node, like C++'s `GetFsNode()`. The spectator miniature is skipped (spectator
deferred).

### 4.8 Setups (4½e-2)

- **SAVE SETUP AS…** It opens at the value's position with `initial = setup_name`, `max_len = 30`, and no filter. On
  accept with a non-empty name, `leaf = name + ".cfg"`. `store.shadows_system("Setups", leaf)`, or a name the store
  cannot place (`/`, `\`, `:`, `..`, a control byte; C++ would create subdirectories or escape the root), gives
  `InfoBox(Reserved)` with text `NAME '<leaf>' IS RESERVED`. Otherwise `store.write("Setups/"+leaf,
  settings_to_toml(&settings))` and `setup_name = name` (C++ `settings_node`). A write error is logged and changes
  nothing. Non-ASCII names turn into Unicode filenames (C++ writes CP437 bytes). That only differs for non-ASCII
  names, which are outside the gate.
- **LOAD SETUP.** A `FileSelectorState` with the `CFG` filter opens on `root/Setups`, titled `"Select options: "` +
  `full_path`. `OnSelected` parses with `settings_from_toml`. On success it replaces `shell.settings`, sets
  `setup_name` to the file's basename, detaches a running match (§4.9) and runs `update_items`. On a parse error it
  keeps the current settings and logs a warning (C++ would crash later, finding 13).

### 4.9 NEW GAME, RESUME, refusals and live settings

- **NEW GAME** is 4½d's router with the loaded/edited `shell.settings`. The level comes from `LevelSlot` reuse or
  `generate_level(…, read_level(…))`. The match seed is `SeedSource`. `MatchConfig { settings: shell.settings.clone(),
  seed }`. The `Match` records `attached = true`.
- **RESUME.** If `match.attached`, `apply_live_settings(sim, &shell.settings)`, then 4½d's focus + push. The `Match`'s
  HUD flags (`map`) and the new `labels` input (`names_on_bonuses`) also refresh from the same settings.
- **LOAD SETUP** sets `match.attached = false`. The paused game keeps what it has, as C++'s old `shared_ptr` does, and
  later edits no longer reach it.
- **`apply_live_settings(state, s)`** writes the §3.7 live-read set into `SimState` and nothing else. `new_match_with`
  calls it, which is a refactor proven by the full re-diff (every sim golden byte-identical).
- **Refusals (Q2).** On Enter/F1 for NEW GAME, and on RESUME with an attached match, the main menu calls
  `build::validate(&settings)` (plus `settings.game_mode != HOLDAZONE` for RESUME) **before** it starts the fade-out.
  A refusal plays `MenuSelect` and pushes `InfoBox(Refused(e), 160, 100, clear = false)` with a Rust-only text (for
  example `HOLDAZONE IS NOT\0SUPPORTED YET`). The menu stays up. C++ has no such box. It is unit-tested, never
  C++-gated, and every G2 case avoids it. Zero enabled weapons cannot reach NEW GAME, because WEAPON OPTIONS refuses
  to close (finding 7), but a loaded setup can carry it, so it is refused the same way (4½c finding 8's error, surfaced
  as a box).

### 4.10 The small-text labels (finding 15)

`render::small_text::draw_text_small(bmp, pal, bank, s, x, y)` is `common.cpp:227-237`. It advances 4 px per byte and
blits A–Z only. `object_draw::sprite_pass` draws the three labels at the C++ positions when `scene.labels` is set:

- bonus names, gated on `names_on_bonuses && frame == 0`;
- booby-trap names, `weapons[obj_index % n]`, gated on `names_on_bonuses` and `!h[HRemExp]` for type 34;
- the Change-held weapon name, gated on the worm's Change bit in `prev/current` control state (C++
  `worm.Pressed(kChange)`).

`SceneData::as_scene` keeps `labels = false`, so every scenario golden, and the C++ render dumpers that mirror
`frame::draw`, are unchanged. The shell and the live game set `labels = true`. The G2 cases hold Change and show
bonus names against the real `Game::Draw`.

### 4.11 Config I/O at boot and exit (4½e-1)

`Shell::boot*` takes `store: Box<dyn ConfigStore + Send + Sync>` and the loaded `Settings`. `game` calls
`scenario::storage::load_setup(store)`, which already does `gameEntry.cpp:55-58`: read the merged `Setups/liero.cfg`,
else defaults written to the user layer. `setup_name = "liero"`. At exit, `Shell::save_on_exit()` writes
`Setups/liero.cfg` (finding 12) from the current settings. Only the shell paths do config I/O. `--live <scenario>`,
`--replay`, `--record` and Scripted stay I/O-free and keep `Settings::default().map` (`game/src/main.rs:611-613`). A
loaded `tc` other than `openliero` is kept in the file but not used (one TC exists; the label shows the value, as C++
would). A loaded `modern_colors = true` is kept but not used (4½g).

---

## 5. What is live, and what stays a placeholder

| Surface | 4½e | Owner |
|---|---|---|
| MATCH SETUP (F7), settings focus, every §3.1 behaviour, number entry | **live** | 4½e-1 |
| WEAPON OPTIONS + its info box | **live** | 4½e-1 |
| `liero.cfg` at boot/exit; refusal boxes; `DrawTextSmall` labels; RESUME live settings | **live** | 4½e-1 |
| LEVEL selector (RANDOM, tree, preview, restore, search); LOAD SETUP; SAVE SETUP AS… + refusals | **live** | 4½e-2 |
| LEFT/RIGHT PLAYER (F5/F6), NETWORK PLAYER (F9), profiles | placeholder | 4½f / Step 5 |
| OPTIONS (F2) hidden menu, F10/F11 | placeholder | 4½g |
| REPLAYS (F3), TC selector, the spectator miniatures | placeholder / skipped | deferred (overview) |
| Holdazone play | refused with a box (Q2) | later slice |
| `.zip` folders in the selector | not ported (no zips ship) | — |
| localStorage persistence in the browser | session only (MemoryStore) | 4½h |

---

## 6. Oracle

Every gate is the real C++. 4½d's G1 is unchanged. G2 grows new cases, and G3 is new.

### 6.1 `oracle_dump_shell` extensions (backwards compatible)

The existing 11 `shell_*` cases must regenerate **byte-identically**. So every new output is opt-in.

- **Tops.** `<top>` gains `O` (WeaponMenuState), `I` (InputString), `B` (InfoBox), `L` (level selector) and `P`
  (options selector). `TopOf` stops failing on them.
- **Script directives (new):**
  - `text <frame> <hex-utf8>` pushes `SDL_EVENT_TEXT_INPUT`, with the string kept alive by the dumper. Hex avoids
    quoting.
  - `detail` emits a `d` line after each `f` line: `d <frame> <cur_menu M|S> <settings_sel> <sub_sel|-> <state16|->`.
    `sub_sel` is the top sub-screen's menu selection. `state16` is `HashGameState(*controller->CurrentGame())` (the
    `stateHash.hpp` the sim dumpers use) on every frame whose top is `G` after the match left selection.
  - `fs <manifest>` switches the case to a **file-system fixture**, described next.
- **File-system fixture.** The dumper materialises the manifest into a fresh directory with two layers:
  - `user/` holds the case's own files;
  - `sys/` holds `Setups/liero.cfg`, `Setups/orbmit.cfg`, empty `Profiles` / `Resources` and a copy of
    `TC/openliero/Levels` (copies, not symlinks, so it is portable).

  It then `chdir`s there, sets `OPENLIERO_TEST_USER_DIR=user` and `OPENLIERO_DATADIR=sys`, and calls the **real**
  `paths::Resolve` and `gfx.SetConfigNodes`. The root label is then `./user`. It boots like `GameEntry`: the real
  `gfx.LoadSettings(config/Setups/liero.cfg)`, with the `SaveSettings` fallback (`gameEntry.cpp:52-58`), in place of
  `setup <file>`. The TC is still loaded from its absolute repo path before the `chdir`. This needs no code
  intervention, only environment variables and a working directory. Selectable levels are also copied into `user/`,
  so C++ can open them (finding 2). The Rust harness materialises the same manifest in a temp directory and builds
  `NativeStore::split(tmp/user, tmp/sys).with_root_label("./user")`.
- **Intervention 9, the exit save.** When a `fs` case ends by QUIT, the dumper runs
  `gfx.settings->save(user_config / "Setups" / "liero.cfg", gfx.rand)`. That is `gameEntry.cpp:78` verbatim, which
  the dumper cannot reach, because it does not run `GameEntry`. Then it writes one `file <rel> <fnv16>` line per file
  under `user/`, sorted. Rust writes the same lines from its store.
- **The search clock.** `Menu::OnKeys` reads `SDL_GetTicks` (4½d F6). The dumper records the wall time of every
  frame that carries a typed key and **fails** the case if two typed keys aimed at the same menu are ≥ 1000 ms apart.
  The Rust harness passes `now_ms = 0`, so neither side ever times out. G1 already gates the timeout itself.
- **Refused-case guards.** A case that reaches a NEW GAME or RESUME that Rust refuses (Holdazone, unequal health) is
  refused by the generator and by the dumper (4½d's rule for intervention 3).

### 6.2 G2e-1 corpus (`examples/gen_slice4_5e.rs`, about 9 cases, all with `detail`)

| Case | Pins |
|---|---|
| `settings_nav` | F7 / Enter on MATCH SETUP; wrap over hidden items; Esc back; F7 again (cursor kept); PgUp/PgDn; F1 from settings focus |
| `settings_edit` | held Left/Right on Integer/Time (the `menu_cycles % 5` cadence), Bool / Enum release, GAME MODE through all four modes (Holdazone: 16 rows, scrollbar), RELOAD/REGENERATE relabel, the disabled-selected main menu |
| `int_entry` | LIVES typed `7`, Backspace, `42`, Return; MAP WIDTH `9999` → 4096, `0` → 64, `333`; Esc cancels; empty Return keeps; the double `MenuSelect`; typed letters never leaking to the menu |
| `weapon_options` | 40 rows, scrolling, PgDn, prefix search `LA` (typed within the gap rule), once-Left/Right, ban everything → Esc → the two-line box over a frozen image → any key → re-enable → Esc; NEW GAME's selection honouring `weap_table` |
| `map_size` | typed 333 × 211 and 96 × 64 (narrower than a viewport: black margins, negative `max_x`), NEW GAME, 300 ticks with the state hash |
| `live_settings` | play → Esc → LOADING TIMES 0, AMOUNT OF BLOOD 300, MAX BONUSES 20, a weapon banned, GAME MODE → Game of Tag → RESUME → 300 ticks with the state hash (finding 1) |
| `labels` | NAMES ON BONUSES on, a weapon bonus dropped in view, P1 and P2 holding Change (finding 15) |
| `cfg_boot` | `fs` with a user `liero.cfg` (Game of Tag, `random_level = false`, water level copied to user): boot values, then QUIT → `file` lines (the exit save) |
| `cfg_default` | `fs` with no `liero.cfg` anywhere: the defaults save at boot and the exit save |

**As landed (4½e-1, `ca72317` / `77406bb`):** 11 cases in `examples/gen_slice4_5e1_shell.rs` +
`tests/shell_e1_cases/` — the plan's 10 (the nine above plus the `match_setup` milestone) and `key_edges` (the C++
`OnKey` edges of a live match: held Change + one Right steps one weapon, a Change+Jump rope throw, a dead worm's
Fire-ready press), 6,099 frames, 6,099 `d` lines and 3 `file` lines, 30 golden files. `map_size` types 333 × 360, then
184 × 420, and `match_setup` 333 × 352: C++ spawning reads past a level shorter than ~342 rows (plan Addendum G3), so
no case may play one. `cfg_boot` keeps a random level (the `fs` file-level path is e-2's, plan fact 27), and
`live_settings` bans no weapon (`weapon_options` and `match_setup` cover the live `weap_table`).

### 6.3 G2e-2 corpus (about 5 cases, all `fs` + `detail`)

| Case | Pins |
|---|---|
| `level_tree` | LEVEL → root rows (`[RANDOM]`, Profiles, Resources, Setups, TC; folder colour 47) → Right into TC/openliero/Levels → Down through levels (the one-frame-late preview, the 52×36 first clear) → Left to the parent (the parent pane) → substring search `stage` → Esc; the main menu then still shows the last preview |
| `level_pick` | pick water_stage → LEVEL `"water_stage"`, RELOAD LEVEL, MAP W/H hidden → NEW GAME plays the file (state hash) → Esc → LEVEL reopens on water_stage (cursor restore) → RANDOM → NEW GAME generates |
| `level_missing` | a `liero.cfg` naming a missing `./user/…/gone.lev`: NEW GAME falls back to random; LEVEL still shows `"gone"` |
| `setup_save` | SAVE SETUP AS… → `liero` (reserved, black `exepal` box) → any key → the input reopens on `liero` → Backspace ×5, `mine` → saved; the value shows `mine`; `orbmit` → shadows the shipped file → box; `file` lines |
| `setup_load` | LOAD SETUP opens in `Setups` (liero, mine, orbmit) → orbmit → values change → a paused match is detached (RESUME keeps the old settings, state hash) → QUIT → `liero.cfg` holds orbmit's values |

### 6.4 🎯 The milestones

- **`shell_match_setup` (e-1).** The done-when-2 path in one script, about 1,100 frames, every line bit-exact.
- **`shell_setups_and_levels` (e-2).** The done-when-7 path, about 900 frames.

Both run in CI as ordinary `oracle-tests` tests (`shell_golden.rs` gains the fixture and the `d` / `file` lines).

### 6.5 G3: sim goldens on other map sizes (`oracle_dump_sim_physics` + `generate`)

`generate <level_seed>` is oracle-only and requires `settings`. It replaces `level`: the dumper builds the level with
the **real** `Level::GenerateFromSettings(common, settings, rand)`, with `rand` seeded from `level_seed`, before the
settings path's usual start state. That covers MakeShadow when `shadow` is on. Rust:
`generate_level(assets, &settings, None, level_seed)` → `build_match`, in a harness shaped like 4½a's
`sim_slice4_5a_settings_golden.rs`. The 12-column format and the 4½a generator style (input seed, ledger, witnesses)
are reused.

| Case | Size | Settings | Witnesses (ledger) |
|---|---|---|---|
| `sim_slice4_5e_small` | 96 × 64 | Kill'em All, lives 3 | a death + respawn search on a tiny map; objects leaving the level |
| `sim_slice4_5e_odd` | 333 × 211 | Scales, LOADING TIMES 37%, blood 300 | a non-multiple-of-8 size (typed entry); reload; blood |
| `sim_slice4_5e_tall` | 160 × 1000 | Game of Tag, TIME TO LOSE 60 s | long falls; `IsGameOver` flips |
| `sim_slice4_5e_banned` | 1024 × 256 | MAX BONUSES 20, most weapons Banned | the bonus `do … while weap_table == 2` loop (`game.cpp:256-258`) drawing several times |

About 1,500 ticks each. The absent-directive path regenerates every existing sim golden byte-identically (the
standing re-diff). **As landed (`6c2dfd8`):** `small` is 96 × 344 and `tall` runs 4,960 ticks (game over at 4,760,
then a 200-row tail): C++ `Worm::BeginRespawn` / `CheckRespawnPosition` read past `materials[]` on a map shorter than
the TC's spawn rectangle (undefined behaviour), so no C++ golden can exist for 96 × 64 (plan Addendum G3 and John's
safe-edges ruling: Rust reads an out-of-array material as rock). The G2 `map_size` and `live_settings` cases then cover the same sim through the real menu.

### 6.6 Standing gates

- Every task runs `cargo test --workspace --exclude game` (debug) and `cargo test -p game`, and no golden changes. The
  4½d `shell_*` and `menu_*` goldens regenerate byte-identically with the extended dumpers.
- The `apply_live_settings` refactor and the `Scene::labels` flag each get their own full re-diff task.
- The golden audit shows only `A` lines. C++ changes are confined to the two dumpers and `CMakeLists.txt`. Both
  dumpers pass clang-format 22 (whole file) and clang-tidy (diff).

### 6.7 Eyeball artefacts (not gates)

- Xvfb: the real `openliero` run with `--config-root <fixture>` and Rust `game --config-root <fixture>`, driven by
  the same `xdotool` path (text typed with `xdotool type`), give C++ | Rust PNGs.
- Headless Chromium on desktop and an emulated phone walks the settings menu, weapon options, number entry (the phone
  text field per Q5), the level selector and Save As.

---

## 7. The live game

### 7.1 Native

The store is `--config-root <dir>` (`NativeStore::single_dir`, C++ flag parity) or `NativeStore::resolve_default()`
(the user folder over `data/`, `OPENLIERO_TEST_USER_DIR` honoured, per Q3). `setup` loads the settings. The `Last`
system that flushes recordings on `AppExit` also calls `Shell::save_on_exit`, which covers QUIT TO OS and window
close. A crash saves nothing, as in C++. `cargo test -p game` never touches the real user folder: its shell tests use
`MemoryStore`.

### 7.2 wasm

The store is `MemoryStore::with_system(embedded)`, root label `/openliero`. The embedded set is `Setups/liero.cfg`,
`Setups/orbmit.cfg` (about 2 KB each) and the TC levels already embedded, keyed `TC/openliero/Levels/*.lev` (per Q6).
`liero.cfg` loads from it. Saves and SAVE SETUP AS… live in memory for the session, exactly like the C++ web build's
MEMFS (finding 12). 4½h replaces the user layer with localStorage behind the same trait. `web/index.html` shows a
one-line hint that settings are not kept after a reload.

### 7.3 Keyboard

Key events are 4½d's. Typed text comes from Bevy `KeyboardInput.text` on key-down events (not on repeats of
non-printing keys), in event order with the key events. F7 may be taken by a browser, but Enter on MATCH SETUP always
works.

### 7.4 Touch

The 4½d mapping stays: pad = P1 Up/Down/Left/Right DOS keys, FIRE = P1 Fire, JUMP = P1 Jump, MENU = Esc, and rising
edges are key-downs. Two Rust-only additions, presentation-only:

- **Auto-repeat for pad Up/Down on menu screens**: 12 frames, then every 3 (the LocalController cadence), emitted as
  `repeat` key events, the same kind the OS keyboard repeat already produces and C++ honours. A held pad Left/Right is
  already a held key, which Integer items repeat on.
- **Text entry (Q5, recommended A).** While an `InputString` is on top, `window.lieroPhase = "text"`. The page focuses
  a hidden `<input>` to raise the phone keyboard, forwards its text and Enter/Backspace as events, and maps FIRE to
  Return and MENU to Esc.

Every 4½e screen is reachable with pad, FIRE, JUMP and MENU. WEAPON OPTIONS closes with JUMP or MENU, as in C++ (Fire
does nothing there). An info box closes on any button.

### 7.5 Preview parameters

`?level=<stem>` now stores the canonical root form `/openliero/TC/openliero/Levels/<stem>.lev` (rule 1 of §4.6). The
LEVEL item then shows it, and the selector opens on it. `?seed=`, `?weapons=` and `?menu=1` are unchanged (4½d §7.5).
URL parameters change the in-memory settings only and are never saved (relevant from 4½h).

---

## 8. Split recommendation and task outline (Q1)

**Recommendation: two sub-slices, each with its own plan, milestone and PR.**

- **4½e-1** needs no directory listing. It does touch the file-system fixture (boot and exit only), the overlays that
  e-2 reuses, the sim-affecting finding 1, the G3 sim gate and the labels.
- **4½e-2** is all file-system: listing, selector, preview, setups, the wasm catalogue.

The cut follows the risk: e-1 carries the sim-affecting work, and e-2 carries the file-system determinism. Each is
about the size of 4½d.

**4½e-1**

| Task | Deliverable | Gate |
|---|---|---|
| T0 | C++ probe of finding 2 (split fixture, a system-only level → random) and of finding 1 (edit while paused changes the next ticks) | probe output recorded in the plan |
| T1 | `render`: `get_dims_h`, `draw_framed_text`, `blit_bitmap`, `draw_text_small`, the three labels behind `Scene::labels` | unit + **full re-diff** |
| T2 | `scenario`: `apply_live_settings` (refactor of `new_match_with`), `validate` | unit + **full re-diff** |
| T3 | `ui`: `InputEvent`, `InputStringState`, `InfoBoxState`, `Screen` variants, continuations | unit |
| T4 | `ui`: settings focus in `MainMenuState`, the §3.1 Enter dispatch, number entry, `WeaponMenuState`, refusals, RESUME sync + attach, store + boot/exit I/O in `Shell` | headless unit flows |
| T5 | C++: `oracle_dump_shell` tops / `text` / `detail` / `fs` / intervention 9 / gap check; `oracle_dump_sim_physics` `generate` | clang; 4½d shell goldens regenerate byte-identically |
| T6 | G3 corpus (`gen_slice4_5e.rs`), goldens, Rust harness | **G3 bit-exact** |
| T7 | G2e-1 corpus, goldens | generator ledgers + awk gates |
| T8 | 🎯 MILESTONE e-1: `shell_golden.rs` with `d` / `file` lines | **G2e-1 bit-exact** |
| T9 | `game`: stores, `--config-root`, boot load / exit save, text events, touch repeat, phone text field (Q5), refusal text, phase `text`; `web/index.html` | `cargo test -p game`; native smoke |
| T10 | Xvfb PNGs, Chromium walk, PROGRESS / overview / maps, broad review | CI commands |

**4½e-2**

| Task | Deliverable | Gate |
|---|---|---|
| T0 | `scenario`: `list`, `root_label`, the wasm system layer; `ui::shell::level_path`; `generate_level` over `Option<LevelData>` | unit (the four resolution rules, the Q4 variant) |
| T1 | `ui::shell::files`: arena `FileNode`, `FileSelector`, `LevelSelectorState` (RANDOM, restore, late preview), `SetupSelectorState`, SAVE SETUP AS… chain | headless unit flows |
| T2 | C++: split-layer fixture listings, `L` / `P` tops | clang; prior shell goldens byte-identical |
| T3 | G2e-2 corpus, goldens | ledgers + awk gates |
| T4 | 🎯 MILESTONE e-2 | **G2e-2 bit-exact** |
| T5 | `game`: wasm catalogue, `?level=` canonical path, selector touch paths | wasm build; Chromium |
| T6 | Xvfb PNGs, docs, broad review | CI commands |

---

## 9. Out of scope

- The player menu, profiles, key rebinding, `GenerateName`, per-worm live settings: 4½f.
- The hidden menu (SHADOWS, POWERLEVEL PALETTES, BOT WEAPONS, …), modern colours, F10/F11: 4½g.
- localStorage, and remembering URL parameters: 4½h.
- Holdazone play and `SelectSpawn`, unless Q2 = C.
- The replay and TC selectors, `.zip` folders, the spectator miniatures.
- Recording menu-driven matches (4½d Q6, still postponed).
- The original Liero level pack (Q6 C is a separate follow-up).

---

## 10. Risks

- **The live-settings set (finding 1).** A missed field makes a resumed match drift only after an edit. Mitigations:
  the mechanical `grep` table, `live_settings` with a per-tick state hash, and the T0 probe. Rust's per-tick reads come
  from `SimState` fields, so `apply_live_settings` is the single choke point.
- **Latent 504×350 assumptions in the sim or render.** Nothing has ever been gated at another size. G3 covers four
  shapes, including one smaller than a viewport (a negative `max_x` clamp) and a non-multiple of 8. Expect a real find
  here, as 4½a's matrix found FAN's impulse loop.
- **File-system determinism in the dumper.** Listing order, de-duplication, case-insensitive sorting, relative
  `FullPath` strings and symlinks can all drift. Mitigations: a manifest of copies (no symlinks), the real `Resolve`
  with environment variables only, and titles that show `./user` on both sides.
- **The wall-clock search timeout inside G2.** It is guarded by the gap check, with `now_ms = 0` in Rust.
- **Text input across SDL, Bevy and phones.** Key and text event ordering, and Android IMEs that send key code 229
  with no printable key. The gate uses scripted events. The live glue diffs the hidden input's value rather than
  trusting key codes, and the Chromium walk covers it.
- **Shared user folder (if Q3 = A).** A C++ user's `liero.cfg` can hold Holdazone, unequal health, another TC or modern
  colours. §4.9 / §4.11 handle each, and Rust writes byte-identical files (4½a-2 G5).
- **Scope.** Two 4½d-sized sub-slices. The milestones keep each one end-to-end, and e-2 can slip without blocking
  4½f, 4½g or 4½h.

---

## 11. Test strategy

1. **Unit.** Overlay editing (filter, `max_len`, Backspace, accept, cancel, `ClearKeys`); the continuations; settings
   focus key table; each Enter arm; `WeaponMenuState` close rule; `apply_live_settings` against the §3.7 table; attach
   / detach; the refusals; `level_path` rules 1–4; `list` merge / sort / dedupe on `NativeStore` and `MemoryStore`;
   `ChildSort` / `CiLess`; `Select` restore; the late preview; Save-As validation; boot / exit I/O on `MemoryStore`.
2. **Differential G3.** Four generated-level sim goldens, 12 columns.
3. **Differential G2e-1 / G2e-2** and the two milestones, every frame plus `d` and `file` lines.
4. **`game`, headless.** Text events from `KeyboardInput`; touch repeat; the phase `text`; `--config-root`; `?level=`
   canonical path; `round_trip.rs` and `record_regression.rs` unchanged.
5. **Eyeball.** Xvfb PNGs and the Chromium walk.
6. **Standing.** Full re-diff, golden audit, wasm build, native smoke with a scratch `OPENLIERO_TEST_USER_DIR`.

---

## 12. Decided in this design

1. Port the settings focus and every `SettingsMenu` arm verbatim, including: the shared TIME TO LOSE/WIN field;
   FLAGS TO WIN never shown; number entry that ignores the step and rewrites only its own row; no entry for blood and
   times; the double `MenuSelect` (findings 9, 14, 16).
2. Menu edits reach a paused match at RESUME, and LOAD SETUP detaches it (finding 1). LD 3 is amended for that one
   write.
3. Port `DrawTextSmall` and all three labels, behind a `Scene` flag so the old goldens stay (finding 15).
4. Gate with the real C++, not self-goldens: G2 extended (e-1, e-2) and G3 on generated levels. The overview's
   "self-goldens" wording is superseded, as it was in 4½d.
5. G3 uses a new oracle-only `generate` directive (LD 4 amended), not level files committed into `data/`, which would
   show up in the level selector.
6. `liero.cfg`: loaded at boot (defaults written when missing), saved to `Setups/liero.cfg` at exit whatever was
   loaded, only on the shell paths. Save errors are logged.
7. The browser keeps settings and saved setups for the session (MemoryStore), like the C++ web build, until 4½h.
8. One `ConfigStore` trait gains `list` and `root_label` for native, wasm and tests. `level_file` keeps C++'s string
   form (root-label prefix), and resolution never panics (§4.6).
9. The level selector is faithful: the whole config tree, `[RANDOM]` first, folders in colour 47, the parent pane,
   and the one-frame-late preview that persists into the menu. There is no preview for RANDOM (overview Q5 corrected).
10. Touch: auto-repeat for pad Up/Down on menus only (presentation-only). Every screen can be driven by pad, FIRE,
    JUMP and MENU.
11. Setup names the store cannot place are refused like reserved names. Non-ASCII names are outside the gate.
12. A setup that fails to parse leaves the settings unchanged. A foreign TC or modern colours are kept in the file and
    not used.
13. `?level=` stores the canonical path. URL parameters are never saved.
14. The G2 format grows by opt-in lines only, so the 4½d goldens regenerate byte-identically.
15. `game` gains `--config-root`, and tests never touch the real user folder.

---

## 13. Open questions for John

Batch 1 (Q1–Q4), batch 2 (Q5–Q6). The recommendation is listed first each time.

**Q1. Should 4½e land in two parts?**
- **A (recommended):** Two parts. **4½e-1:** the settings menu, weapon options, typing numbers, settings kept between
  runs, bonus/weapon name labels. **4½e-2:** the level picker and saving/loading named setups. Each part gets its own
  milestone and PR, and the first is playable sooner.
- **B:** One slice, one PR, as the overview planned.

**Q2. GAME MODE can now be set to Holdazone, which the Rust game cannot play yet. What should happen when you start
(or resume) a Holdazone match?**
- **A (recommended):** A short box says "HOLDAZONE IS NOT SUPPORTED YET", and you stay in the menu. The menu itself
  looks exactly like the original.
- **B:** GAME MODE skips Holdazone when cycling. It is simpler, but the settings menu then differs from the original.
- **C:** Port Holdazone as part of 4½e. It is real game-logic work with its own C++ checks, and it makes 4½e noticeably
  bigger.

**Q3. Where should the Rust game keep your settings (liero.cfg and saved setups) on desktop?**
- **A (recommended):** The same folder as the original OpenLiero. Both games read and write the same files, which Rust
  already writes byte-identically, so switching between them keeps your setup.
- **B:** Its own separate folder, starting from the default settings.
- **C:** Its own folder, but copy the original's settings the first time it starts.

**Q4. The original has a bug: in a normal (non-portable) desktop install, picking one of the shipped levels in the
level picker quietly plays a random level instead. The picker still shows the level's name. Should Rust copy that?**
- **A (recommended):** Fix it. The picked level is played. This is the only intended difference, and it cannot occur in
  the portable or web versions of the original anyway.
- **B:** Copy the bug exactly, to stay 100% identical.

**Q5. Some settings need typing: a number such as MAP WIDTH, or a name for SAVE SETUP AS…. How should that work on a
phone?**
- **A (recommended):** The phone's own keyboard pops up while the text box is open. FIRE confirms and MENU cancels.
- **B:** No typing on phones for now. Numbers change only by holding left/right, and SAVE SETUP AS… is keyboard-only
  until the web-persistence slice (4½h).
- **C:** A Liero-style letter picker drawn in the game (a new, Rust-only screen).

**Q6. Which levels should the level picker offer?**
- **A (recommended):** Today's set. RANDOM stays the normal choice, plus the 5 test levels on desktop and the 4 small
  ones on the web (the 1.2 MB `modern_test` level stays out of the web download).
- **B:** Also put `modern_test` in the web build (+1.2 MB before compression).
- **C:** Add the original Liero level pack. That needs a licence check first and would be a separate follow-up, not
  part of 4½e.

## 14. Rulings (John, 2026-09-26)

All six recommendations were accepted:

- **Q1 → A, two parts.** 4½e-1 (settings menu, weapon options, number entry, `liero.cfg` load/save, small-text labels, live settings on RESUME, G3) and 4½e-2 (file listing, level selector, SAVE SETUP AS… / LOAD SETUP, the wasm catalogue), each with its own milestone, PR and preview.
- **Q2 → A.** Starting or resuming a Holdazone match shows a "HOLDAZONE IS NOT SUPPORTED YET" box and stays in the menu; the settings menu itself is unchanged from C++.
- **Q3 → A.** Desktop Rust reads and writes the same config root as C++ OpenLiero (`paths::Resolve` semantics).
- **Q4 → A.** Fix the shipped-level bug: a picked level is played. This is the one intended difference from C++, and the oracle case that would show it is documented rather than gated.
- **Q5 → A.** On a phone, a text box opens the device keyboard; FIRE confirms and MENU cancels.
- **Q6 → A.** Today's level set: RANDOM, the 5 test levels on desktop, the 4 small ones on the web.
