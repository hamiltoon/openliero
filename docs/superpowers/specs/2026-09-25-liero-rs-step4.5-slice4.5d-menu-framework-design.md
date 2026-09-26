# Step 4½, Slice 4½d — the menu framework, `ScreenStack` and the main menu (THE MILESTONE): design

Status: **DESIGN** · 2026-09-25 · branch `claude/cpp-oracle-vcpkg-assets-chcwcm` (PR #14 into `liero-rs-step-4-5`; 4½a ✅, 4½b ✅, 4½c-0 ✅, 4½c ✅ landed)
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (the 4½d bullet, LD 1–3, §Oracle, done-when 1–2; cited **overview**)
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` §0–§2, §8 (cited **cpp-map**) and
`2026-09-10-liero-rs-step4.5-rust-baseline-map.md` §1–§3, §7–§9 (cited **rust-map**)
Precedents: `2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md` (cited **4½c design**) and its plan
`plans/2026-09-25-liero-rs-step4.5-slice4.5c-plan.md`, **Addendum A** (the real C++ draw run headlessly as the pixel
oracle; cited **4½c A1/A2**); `2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` §4.4, §9.3.6
(cited **4½a design**)
Next artifact: `plans/2026-09-25-liero-rs-step4.5-slice4.5d-plan.md`

Today the Rust game boots straight into weapon selection. C++ boots into a main menu drawn over a generated level.
From the menu, NEW GAME starts weapon selection, Esc during play fades back to the menu with RESUME GAME shown,
and QUIT TO OS ends the program. 4½d ports that loop: the `Menu` widget framework, a Bevy-free `ScreenStack`
mirroring C++ `StateStack`, `MainMenuState`, and the frame router in `Gfx::RunOneFrame`. The result is the Step 4½
milestone: bare run → main menu → NEW GAME → weapon selection → play → Esc → menu (RESUME / NEW GAME) → QUIT.
John ruled that pixels are checked against C++ in this cloud session, so every frame of the milestone is gated
bit-exact against the **real C++ `Gfx` frame loop**, run headlessly.

---

## 0. Summary of findings (read this first)

These were read out of the C++ source for this slice. Items 1–6, 9 and 13 contradict or sharpen the overview or
the maps.

1. **The main-menu screen also draws the SETTINGS menu, disabled, on the right.** `MainMenuState::Draw` calls
   `settings_menu.Draw(..., disabled=true)` whenever the main menu has focus (`mainMenuState.cpp:621-622`). That
   screen shows all visible settings items (GAME MODE, LIVES, LEVEL, MAP WIDTH, …) with their values, in
   `dis_colour` 7 at (178, 20), with `value_offset_x = 100` (`gfx.cpp:267`, `:485-503`, `:523`). A pixel-exact main
   menu therefore needs the settings menu's *display*: its item list, every behavior's `OnUpdate` value string
   (`gfx.cpp:1262-1312`), `LevelSelectBehavior`'s REGENERATE/RELOAD relabel (`:1222-1234`), `OptionsSaveBehavior`'s
   setup name (`:1245-1251`), and `SettingsMenu::OnUpdate`'s per-mode visibility (`:1314-1341`). The overview's
   4½d bullet does not mention it. 4½e still owns *interacting* with that menu.
2. **`DrawSpectatorInfo` draws nothing into the main window.** It writes only into `single_screen_renderer`, the
   640×400 spectator renderer (`gfx.cpp:1739-1780`), which the window never shows unless the spectator window is on
   (`gfx.cpp:1158-1174`). The overview's "a `DrawSpectatorInfo` equivalent" is therefore not needed. It stays with
   the deferred spectator window.
3. **`onoff`, `game_modes`, `controllers`, `weap_states`, `input_devices` and `key_names` are hardcoded in C++, not
   read from `tc.cfg`.** They are set in `Texts::Texts()` (`common.cpp:205-225`), and `key_names` is at
   `common.cpp:25`. Rust `assets::tc::Texts` holds only the forty `[texts]` strings (`rust/assets/src/tc.rs:205-246`).
   The overview ("What Steps 0–4 delivered") and rust-map §6 say `TcConfig` parses them; it does not. The
   controller names are `"Human"`, `"CPU"`, `"AI"` (`common.cpp:215-217`), not the "Human / DumbAI / FollowAI" of
   cpp-map §2.4. 4½d ports `game_modes` and `onoff`.
4. **The copyright bar needs the CP437 high half.** `MainMenuState::Enter` draws `LS(Copyright2)`
   (`mainMenuState.cpp:107-108`), which is `"Liero v1.33 (c) MetsänEläket 1998,1999"` (`tc.cfg:244`). The `ä` goes
   through `cp437::UnicodeToByte`, which maps it via `kHighHalf` (`cp437.cpp:11`, `:177-187`; `font.cpp:52-55`). The
   Rust font is ASCII-only; the table was deferred (`rust/render/src/font.rs` `draw_string` doc).
5. **Menu keys are level flags set by key-down events, and OS auto-repeat counts.** `ProcessEvent` sets
   `dos_keys[k] = true` on every `SDL_EVENT_KEY_DOWN`, including OS repeats. Only the controller path drops repeats
   (`gfx.cpp:605-611`). `TestSdlKeyOnce` reads and clears the flag (`gfx.hpp:149-153`, `:165-168`). Menu navigation
   therefore auto-repeats at the OS rate while a key is held. There is no 12/3 emulation in menus.
   rust-map §3's "`just_pressed` on Up/Down/Enter/Esc" would lose the repeat. Left/Right are read *held*
   (`TestSdlKey`, `mainMenuState.cpp:581-592`). A behavior that returns false from `OnLeftRight` releases the key
   (`ResetLeftRight`, `:56-61`), so it fires again only on the next key-down event (a real press or an OS repeat).
6. **Type-to-search is not used by the main menu.** `Menu::OnKeys` is called only from `WeaponMenuState`
   (`weaponMenuState.cpp:89`) and `FileSelector` (`fileSelector.hpp:297`). In 4½d it is framework only, and its first
   live use is 4½e. Its timeout is wall-clock milliseconds (`SDL_GetTicks`, `menu.cpp:21-24`), not frames.
7. **The stack never holds the menu and the game together.** Dispatch runs only when an `Update` returning false
   empties the stack (`state.hpp:107-111`, `gfx.cpp:1491-1493`). The handoff is pop → router → push, and the pop
   frame presents nothing (`gfx.cpp:1626`). One exception: going back to the menu, `MainMenuState::Enter` calls
   `Flip()` at `fade_value = 0` (`mainMenuState.cpp:102-105`), which presents one black frame. At boot that black
   frame comes from `InitFrameStepping`'s push (`gfx.cpp:1462-1464`).
8. **`menu_cycles` counts every frame, not only menu frames.** Menu frames increment it before drawing, in
   `UpdateMenuPalettes` (`gfx.cpp:986`). Every other frame increments it after drawing (`gfx.cpp:1645-1647`), and that
   includes each match frame. It is reset only in `MainMenuState::Enter` (`mainMenuState.cpp:145`). So the first
   menu frame draws with `menu_cycles = 1`, and weapon selection inherits the menu's count (the 4½c design §5
   caveat).
9. **Esc keeps the simulation running for 32 frames.** `OnKey(kDkEscape)` sets `fade_value = 31` and
   `going_to_menu` (`localController.cpp:82-85`). `Process` keeps calling `game.ProcessFrame()` while the fade counts
   down (`:153-181`, `:185-194`). That is 31 presented frames, then a pop frame that also ticks. The same holds
   during weapon selection: `ws->ProcessFrame()` keeps running (`:124-152`). `OnKey` runs on key-*up* too
   (`gfx.cpp:634-639`), so releasing Esc in play also pauses.
10. **RESUME refocuses the same controller.** On RESUME the router pushes a new `GamePlayState` whose `Enter` calls
    `controller->Focus()` (`gfx.cpp:1524-1530`, `:1593`; `gamePlayState.cpp:14`). `Focus` sets
    `going_to_menu = false` and `fade_value = 0` (`localController.cpp:100-120`), so play fades back in while the
    match ticks. RESUME GAME is shown iff `Running()`, which is `state != kStateGameEnded && != kStateInitial`
    (`:304`; `mainMenuState.cpp:110-119`). After game over, and at boot, it is hidden.
11. **Weapon selection and the menu share one frozen screen.** Weapon selection caches its background in
    `gfx.frozen_screen` (`weapsel.cpp:165-179`). `MainMenuState::Enter` overwrites that buffer with the menu
    background plus the copyright bar (`mainMenuState.cpp:107-108`, `:131`). Now Esc in the middle of selection,
    then RESUME. `cached_background` is still true, so the selection screen redraws over the *menu's* frozen
    screen, and the copyright bar stays visible (`weapsel.cpp:182`). The Rust port reproduces this.
12. **The menu background is a draw of the pop frame's tick.** When play pops, the router draws the controller once
    more (`gfx.cpp:1614-1616`). In C++ that draw shows the state the pop frame's `ProcessFrame` produced, a tick that
    was never presented. Rust's `frame::draw` folds in C++ `ProcessViewports` (4½c plan-time fact 3), so the port must
    draw every tick exactly once. Here that single draw is the background capture (§4.11).
13. **C++ game over goes to the stats screen, not the menu.** `GamePlayState` replaces itself with `StatsState` when
    `NormalStatsRecorder::game_time > 0` (`gamePlayState.cpp:57-71`). With no stats recorder it pops to the menu
    (`:93`), and 4½d takes that path until 4½g. The oracle dumper forces it (§6.2). This replaces 4½c's automatic
    restart after game over.
14. **Selection routing has order effects.** In `MainMenuState::Update`, selecting an item in a frame sets
    `fade_value = 32` (`mainMenuState.cpp:604-609`). The same frame's `UpdateMenuPalettes` does not raise it, because
    it reads the fading flag captured *before* `Update` (`gfx.cpp:1488-1489`, `:1634`). The fade-out then presents
    32, 31, …, 0. The next frame pops, and that frame presents nothing. Input is ignored while fading out
    (`mainMenuState.cpp:153-160`) but accepted during the fade-in.
15. **Two dead or quirky bits to port verbatim.** `MenuItem::Draw`'s `c = disabled ? 7 : 168` sits inside the
    `else if (selected)` branch, which runs only when not disabled, so it is always 168 (`menuItem.cpp:29-35`).
    `Up || R` short-circuits (`mainMenuState.cpp:181-182`): with both Up and R down in one frame, the second key's
    flag survives and moves the cursor again on the next frame. `Menu::Process` is declared but never defined
    (`menu.hpp:53`).
16. **The level-reuse test compares the map size even for a file level.** The check is
    `!regenerate_level && random_level == old && level_file == old && width == old && height == old`
    (`gfx.cpp:1512-1516`), against provenance that `GenerateFromSettings` records after loading or generating
    (`level.cpp:421-424`). Changing MAP WIDTH while a file level is chosen therefore reloads the file. A file that
    failed to load and fell back to random still records `random_level = false`, so the fallback is reused.

---

## 1. Goal / done-when

**Goal.** Rust boots like C++: a main menu over a generated level, NEW GAME, weapon selection, play, Esc to a pause
menu with RESUME, and QUIT. It is driven by a Bevy-free menu framework and screen stack, and every presented frame
on that path is bit-exact against the real C++ frame loop.

**Done when (10):**

1. **G1: the widget gate.** The `menu_*` goldens match line for line (§6.1): navigation, visibility, scrolling, the
   scrollbar, every generic behavior, type-to-search, and draw hashes. The real C++ `Menu` / `MenuItem` /
   behaviors produce them, and so does the real `SettingsMenu` display for all four game modes and both level kinds.
2. **G2: the shell gate.** For every `shell_*` case (§6.4), each frame matches the real C++ `Gfx::RunOneFrame`
   on: presents (0 or 1), the faded hash of the presented frame, the unfaded back-buffer hash, `fade_value`,
   `menu_cycles`, the top screen, the main-menu cursor, and the menu sound ids. The real C++ boot frame matches as
   well.
3. **🎯 MILESTONE (§6.6).** The `shell_milestone` case is bit-exact on every frame: boot → menu → NEW GAME → weapon
   selection → play → Esc → menu (RESUME GAME (F1) / NEW GAME) → RESUME → play → Esc → NEW GAME (level reused) →
   selection → Esc → menu → QUIT.
4. **The live game boots to the menu.** A bare `cargo run -p game` and a bare preview URL both open the main menu.
   Keyboard (§7.2) and touch (§7.3) both drive it. QUIT exits natively and stops cleanly in the browser (§7.4). The
   end of a match returns to the menu.
5. **The preview parameters behave per §7.5**, and existing preview links keep today's behavior. `?demo`,
   Scripted, `--replay` and `--live --record` are unchanged, and `round_trip.rs` / `record_regression.rs` are green.
6. **Every prior golden is byte-identical.** That includes the 4½c weapon-selection frame gate, and the CP437 font
   change is hash-neutral. Only `A` lines are added under `golden/`.
7. **The `ui` crate is Bevy-free.** `cargo tree -p ui` names no `bevy*` crate.
8. These are green: `cargo test --workspace --exclude game` (debug), `cargo test -p game`, the wasm build, and
   clang-format / clang-tidy on the two new dumpers.
9. **Eyeball artefacts (not gates).** C++ | Rust side-by-side PNGs come from both binaries under Xvfb + xdotool
   (4½c A2). A headless-Chromium walk covers desktop and an emulated phone (§6.7).
10. PROGRESS, the overview and the maps are updated with the §0 corrections, and `liero-shot` §7 gains a menu
    paragraph.

---

## 2. Inherited locked decisions (overview), and the two this slice amends

- **LD 1:** pixel-exact menus first. Everything is drawn on the 320×200 CPU surface with `render::font` /
  `render::blit` and the item recipe 4½c already landed (`render::menu::draw_item`).
- **LD 3, restated.** `tick_and_render` stays the only system with `ResMut<Sim>`. It now calls the shell once per
  fixed tick (one C++ `RunOneFrame`). Inside the shell, only the Playing screen's update and the router receive
  `&mut SimState`. The router only *replaces* the state with a freshly built one; it never edits it. Menu screens
  get a context with no path to the sim (§4.9). The determinism firewall is kept by types, not by a run condition.
- **LD 4 / LD 5:** the scenario format stays frozen, and `SimState::new` is unchanged. 4½d adds no directive.
- **LD 6:** a dedicated level `Rand` seeded from a seed. 4½d splits the boot seed (the level behind the menu) from
  the per-NEW-GAME match seed (§4.11).
- **Amended — LD 2 (location).** The overview puts `ScreenStack` in `game/src/lib.rs` so that it can be tested
  headless. The C++ frame gate needs `oracle-tests` and `shot` to drive the whole shell. They cannot depend on
  `game`, which pulls in Bevy through `game::input`. So the stack lives in a new Bevy-free crate `rust/ui` (§4.1),
  which also answers overview Q1 (a `ui` crate for widgets). The semantics are unchanged. **Open Q1 (John).**
- **Amended — overview §Oracle (presentation).** "Presentation is not C++-gated … the C++ menu is `Gfx`-driven and
  not dumpable" was already overturned for weapon selection by 4½c A1. 4½d gates the whole frame loop the same way:
  the real C++ `Gfx` runs headlessly (§6.2). Rust self-goldens are not used.

---

## 3. The C++ shell, precisely

### 3.1 The frame loop (`Gfx::RunOneFrame`, `gfx.cpp:1467-1652`)

```
poll events: QUIT or Alt+F4 → return false (quit)                      :1476-1483
             else top->HandleEvent(ev)                                  :1484
sel = menu->Selection(); fading = menu->IsFadingOut()   (captured BEFORE Update) :1488-1489
if !stack.Update():                        (the top popped and the stack is empty)
    if sel >= 0: router(sel)                                           :1493-1593
    else: back to the menu                                             :1594-1625
    return true                             (no Draw, no Flip on this frame)
if top->WantsMenuFlip(): UpdateMenuPalettes(fading)                    :1631-1635
stack.Draw()                                                           :1643
if !WantsMenuFlip(): ++menu_cycles                                     :1645-1647
Flip()                                        (present with the renderer's fade_value) :1648
```

`StateStack` (`state.hpp:47-139`): `Push` calls `Enter` (`:50-54`). `Update` runs the top, then applies a
scheduled replacement (`:101-105`) or pops on false (`:107-111`). It returns `!empty`. `Draw` walks down past
overlays and draws bottom to top (`:117-131`). `MainMenuState` and the states 4½e adds sit on the stack together;
`MainMenuState` and `GamePlayState` never do (finding 7). `WantsMenuFlip` is false for `GamePlayState`
(`gamePlayState.hpp:119`) and true otherwise (`state.hpp:38`).

**Boot** (`InitFrameStepping`, `gfx.cpp:1439-1465`). `LocalController(common, settings)`, which is never focused.
Then `GenerateFromSettings(gfx.rand)` and `SwapLevel`, then `Focus` both renderers (this sets the palette:
`Game::UpdateSettings`, `game.cpp:475-488`). Then `controller->Draw(play_renderer)` of the unstarted game. Finally a
`MainMenuState` is pushed, and its `Enter` presents the black frame.

**Presentation** (`Flip`, `gfx.cpp:1152-1190`). `Gfx::Draw` copies the ARGB back buffer through
`ScaleDraw(..., fade)` (`gfx.cpp:1033-1038`). The fade is `(v * fade) >> 5` per channel when `fade < 32`, otherwise
the identity (`FadeArgb` / `ScaleDraw`, `blit.cpp:791-815`). Rust already has the same formula in
`render::hash::fade_channel`. The pacing is 14 ms (`:1176-1189`).

### 3.2 `MainMenuState` (`mainMenuState.cpp`)

- **`Enter`** (`:95-148`):
  1. `fade_value = 0` and `Flip()`, which presents black; then `Process()`, which polls events (`:102-105`).
  2. `FillRect(0,151,160,7, 0)` and `Copyright2` at (2, 152) in colour 19 (`:107-108`). Both go through whatever
     `pal32` the last draw left behind (§4.10).
  3. If `Running()`: show RESUME, labelled `"RESUME GAME (F1)"`, relabel NEW GAME as `"NEW GAME"`, and start on
     RESUME. Otherwise hide RESUME, label `"NEW GAME (F1)"`, and start on NEW GAME (`:110-119`).
  4. `"TC (" + tc + ")"` (`:121`).
  5. `MoveToFirstVisible` on both menus, then `settings_menu.UpdateItems` (`:123-125`).
  6. `fade_value = 0`, `cur_menu = main_menu`, `frozen_screen = play_renderer.bmp` (`:127-131`). The spectator
     minimap goes to the spectator renderer only (`:132-143`).
  7. `menu_cycles = 0`, `selected_ = -1`, `Active` (`:145-147`).
- **`Update`** (`:152-612`). While fading out: `--fade_value`, and pop when it reaches 0 (`:153-160`). While active,
  the keys are handled in source order (§7.2). Any selection sets `fade_value = 32` and starts the fade-out
  (`:604-609`).
- **`Draw`** (`:614-626`):
  1. `DrawBasicMenu`: `bmp = frozen_screen`, then `main_menu.Draw(disabled = cur_menu != main_menu, x = -1,
     show_disabled_selection = true)` (`gfx.cpp:1699-1704`).
  2. `DrawSpectatorInfo`, which draws to the spectator renderer only (finding 2).
  3. The settings menu, disabled, if the main menu has focus (`:621-622`).

### 3.3 The items

**Main menu** (`gfx.cpp:505-521`, ids `mainMenu.hpp:9-25`), at (53, 20) (`gfx.cpp:266`), 8 px rows, height 15:

| # | Id | Label | color / dis | 4½d |
|---|---|---|---|---|
| 0 | `kMaResumeGame` (0) | `RESUME GAME (F1)` (set in `Enter`) | 10/10 | **live** |
| 1 | `kMaNewGame` (1) | `NEW GAME (F1)` / `NEW GAME` | 10/10 | **live** |
| 2 | `kMaHostGame` (10) | `HOST LAN GAME` | 48/48 | placeholder (Step 5) |
| 3 | `kMaJoinGame` (11) | `JOIN LAN GAME` | 48/48 | placeholder (Step 5) |
| 4 | `kMaHostOnline` (13) | `HOST ONLINE` | 48/48 | placeholder (Step 5) |
| 5 | `kMaJoinOnline` (14) | `JOIN ONLINE` | 48/48 | placeholder (Step 5) |
| 6 | `kMaAdvanced` (5) | `OPTIONS (F2)` | 48/48 | placeholder (4½g) |
| 7 | `kMaReplays` (7) | `REPLAYS (F3)` | 48/48 | placeholder (deferred: `.lrp` phase 2) |
| 8 | `kMaTc` (9) | `TC (openliero)` | 48/48 | placeholder (deferred: one TC) |
| 9 | `kMaQuit` (6) | `QUIT TO OS` | 6/6 | **live** |
| 10 | — | `Space()` (not selectable, `menuItem.hpp:18-22`) | 0/0 | — |
| 11 | `kMaPlayer1Settings` (3) | `LEFT PLAYER (F5)` | 48/48 | placeholder (4½f) |
| 12 | `kMaPlayer2Settings` (4) | `RIGHT PLAYER (F6)` | 48/48 | placeholder (4½f) |
| 13 | `kMaNetPlayerSettings` (12) | `NETWORK PLAYER (F9)` | 48/48 | placeholder (Step 5) |
| 14 | `kMaSettings` (2) | `MATCH SETUP (F7)` | 48/48 | placeholder (4½e) |

There are 14 or 15 visible items against a height of 15, so the main menu never shows a scrollbar
(`menu.cpp:107`).

**Settings menu, display only** (items `gfx.cpp:485-503`, all 48/7, at (178, 20), `value_offset_x = 100`). Values
come from each behavior's `OnUpdate` (`gfx.cpp:1262-1312`):

- GAME MODE is `game_modes[v]`.
- LIVES, FLAGS TO WIN and MAX BONUSES are decimal.
- TIME TO LOSE, TIME TO WIN and ZONE TIMEOUT use `TimeToString` (`text.cpp:5-16`). TIME TO LOSE and TIME TO WIN both
  bind `time_to_lose`.
- LEVEL is `Random2`, or `"<basename>"` when `random_level` is off, and it relabels item 16 as `RegenLevel` or
  `ReloadLevel`.
- MAP WIDTH and MAP HEIGHT are decimal.
- LOADING TIMES and AMOUNT OF BLOOD are decimal plus `%`.
- The four switches show `onoff[v]`.
- SAVE SETUP AS… shows the setup's basename, which is `liero` at boot (`gameEntry.cpp:55-58`).
- WEAPON OPTIONS and LOAD SETUP have no value.

`OnUpdate` (`:1314-1341`) hides LIVES, TIME TO LOSE, TIME TO WIN, ZONE TIMEOUT and FLAGS TO WIN, then shows LIVES
(KillEmAll, Scales), TIME TO LOSE (Game of Tag), or TIME TO WIN + ZONE TIMEOUT (Holdazone). MAP WIDTH and HEIGHT are
shown iff `random_level`. That leaves 15 visible items in three modes, and 16 in Holdazone, which shows a scrollbar.

### 3.4 `Menu` (`menu.hpp:26-197`, `menu.cpp`)

- **State.** `items`, `item_height = 8`, `value_offset_x`, `x`, `y`, `height = 15`, `top_item`, `bottom_item` (both
  visible indices), `visible_item_count`, `centered`, the private `selection_`, `search_prefix` and `search_time`
  (`menu.hpp:36-49`, `:177-196`).
- **`AddItem`** counts visible items and does not call `SetTop` (`menu.cpp:295-302`). So `bottom_item` stays 0
  until the first `MoveTo` → `EnsureInView` → `SetBottom` (`:155-167`, `:218`).
- **`Draw`** (`:81-125`). Start at `ItemFromVisibleIndex(top_item)` and skip invisible items. Draw up to `height`
  rows with `selected = c == selection_ && (!disabled || show_disabled_selection)`, and call `DrawItemOverlay` after
  each row. Then draw the scrollbar iff `visible_item_count > height` (`:107-124`):
  - the two arrows are glyphs 22 and 23, with a colour-0 shadow at (x−6, …) and colour 50 at (x−7, …);
  - `bar = height*8 + 1 − 17`;
  - `tab = clamp(height*bar/vis, 0, bar)` at `y + top*bar/vis`;
  - the tab is two `FillRect`s: colour 0 at (x−7, +9) and colour 7 at (x−8, +8).
- **`MenuItem::Draw`** (`menuItem.cpp:6-42`). This is 4½c's text arm plus the value arm. When selected, a second box
  of the value's width is drawn at `x + voff − vw/2`. When not selected, the value gets a colour-0 shadow at
  `+3,+2`. The value text goes at `x + voff − vw/2 + 2, y + 1` in the item colour.
- **Navigation.**
  - `Movement(±1)` wraps over items that are visible **and** selectable (`:263-293`).
  - `MovementPage(d)` moves the visible index by `d * (height/2)`, calls `SetTop(top + offset)`, clamps, then
    `MoveTo` (`:250-261`).
  - `MoveTo` clamps, then `FirstVisibleFrom` (the next visible and selectable item), then `EnsureInView`
    (`:129-134`).
  - `SetTop` clamps to `[0, vis − height]` and sets `bottom = min(top + height, vis)` (`:220-225`).
  - `SetVisibility` updates the count, re-anchors `top` on the same real item, and calls
    `EnsureInView(selection)` (`:227-246`).
- **Behaviors.** A fresh behavior is built per call through the virtual `GetItemBehavior` (`menu.hpp:69-97`). The
  base class returns `true` from `OnLeftRight`, `-1` from `OnEnter`, and does nothing in `OnUpdate`
  (`itemBehavior.hpp:13-17`).

### 3.5 The behaviors

| Behavior | `OnLeftRight(dir)` | `OnEnter` | `OnUpdate` (value) |
|---|---|---|---|
| base (`itemBehavior.hpp:8`) | `true`, no-op | `-1` | — |
| `Integer(v,min,max,step,pct)` (`integerBehavior.cpp:14-88`) | acts only if `menu_cycles % scroll_interval == 0` (default 5). If `dir<0 && v>min` or `dir>0 && v<max`, `v = clamp(v + dir*step, min, max)`; `OnUpdate` if it changed. Returns `true` (the key stays held, so it repeats at the `menu_cycles` cadence) | plays `MenuSelect`. If `allow_entry`, opens a digit `InputStringState` at the value (4½e); returns `-1` | `v/display_div` in decimal, plus `%` if `pct` |
| `Time : Integer` (`timeBehavior.hpp:9-12`) | as Integer; `allow_entry = false` | as Integer, entry off | `TimeToString(v)` (`text.cpp:5-16`), or `TimeToStringFrames` if `frames` (`:18-49`) |
| `BooleanSwitch(v[,set])` (`booleanSwitchBehavior.cpp:8-30`) | `dir>0`: `MenuMoveUp`, else `MenuMoveDown`; `set(!v)`; `OnUpdate`; returns **`false`** | `MenuSelect`; `set(!v)`; `OnUpdate`; `-1` | `onoff[v]` |
| `Enum(v,min,max,broken)` (`enumBehavior.cpp:9-44`) | `broken`: returns `false` and does nothing. Otherwise the sound as Boolean, then `Change(dir)`, and returns `false` | `MenuSelect`, `Change(+1)`, `-1` | decimal `v` |
| `Enum::Change` (`:31-39`) | `v' = ((v + dir + range − min) % range) + min` in `u32`; if changed, `menu.UpdateItems` (every item, then `Menu::OnUpdate`) | | |
| `ArrayEnum : Enum` (`arrayEnumBehavior.hpp:12-20`) | as Enum, range `[0, N−1]` | as Enum | `arr[v]` |

`OnLeftRight` returning false makes the caller release Left and Right (`mainMenuState.cpp:581-592`, `:56-61`).

### 3.6 Type-to-search (`Menu::OnKeys`, `menu.cpp:14-79`)

This runs for each scancode in the frame's `key_buf`, which holds up to 32 key-down scancodes including OS repeats
(`gfx.cpp:601-603`):

1. The key is `sym = SDL_GetKeyFromScancode(sc, NONE)`. It is acted on only for `32..=127` or Tab.
2. If it is not Tab and `now − search_time > 1500`, clear the prefix.
3. Loop. The candidate prefix is the old prefix plus `sym` (Tab adds nothing), and `search_time = now`.
4. Scan `offs` from `skip` (1 for Tab, else 0) over `(selection + offs) % n` for a **visible** item. The match is a
   case-insensitive prefix, or a substring when `contains`. On a match, `MoveTo` it and keep the prefix.
5. If nothing matches, clear the prefix. If it was empty, stop. Otherwise retry with the bare `sym`.

### 3.7 Palette, fades and sounds

- **`UpdateMenuPalettes(quitting)`** (`gfx.cpp:978-1005`), run on menu frames only:
  1. `if fade < 32 && !quitting: ++fade`;
  2. `++menu_cycles`;
  3. `pal = Origpal`, then `RotateFrom(Origpal, 168, 174, menu_cycles)`;
  4. `SetWormColours(settings)` for both worms (`palette.cpp:114`);
  5. the network-player swatch, in the player menu only (4½f);
  6. repack `pal32`.

  `Origpal` is the focused palette: the level palette plus the worm ramps (`game.cpp:475-488`). This is 4½c's
  `weapsel_palette` followed by `set_worm_colour` ×2.
- **Menu fades.** In: `Enter` sets 0 and each menu frame adds 1, up to 32. Out: 32, then down to 0, then pop
  (finding 14).
- **Controller fade** (`localController.cpp:185-199`, applied in `Draw`, `:211`):
  - `going_to_menu` counts down and returns false at 0;
  - otherwise the fade counts up to 33;
  - Esc sets 31 (`:82-85`);
  - game over sets 180 (`:277-282`);
  - `Focus` sets 0 (`:118-119`);
  - entering the game from selection sets 33 (`:284-287`), even while `going_to_menu`, which restarts an Esc fade
    at 33.
- **Sounds** (`g_sound_player->Play(hook)`, which skips a negative hook, `mixer/player.hpp`):
  - Up plays `MenuMoveDown` (`mainMenuState.cpp:183`) and Down plays `MenuMoveUp` (`:190`);
  - Enter plays `MenuSelect` for every main-menu item (`:198`), and the LAN/online arms play it a second time
    (`:234`, `:248`, `:254`);
  - PgUp plays `MenuMoveDown` and PgDn plays `MenuMoveUp` (`:595`, `:600`);
  - Esc, F1 and a no-op Left/Right play nothing.

### 3.8 The router (`gfx.cpp:1493-1625`)

- **`kMaQuit`** → quit (`:1496`). This comes after the fade-out, so the last presented frame is black.
- **`kMaNewGame`** (`:1507-1523`):
  1. build a new `LocalController`;
  2. reuse the old controller's level (`SwapLevel(*old_level)`, with craters and provenance) iff the finding-16
     test passes, else `GenerateFromSettings(gfx.rand)`;
  3. push `GamePlayState`, whose `Enter` → `Focus` → `ChangeState(kStateWeaponSelection)`
     (`localController.cpp:112-114`) constructs the selection at fade 0.
- **`kMaResumeGame`** → push `GamePlayState` on the same controller (the replay branch at `:1525-1530` is out of
  scope).
- **Back to the menu** (no selection pending, `:1594-1625`):
  1. `controller->Unfocus()`, which unfocuses the selection (`localController.cpp:90-97`);
  2. `ClearKeys()`;
  3. `play_renderer.Clear(); controller->Draw(play_renderer)`, the finding-12 draw: the game, or the unfocused
     selection's frozen copy (`weapsel.cpp:182-186`);
  4. push a fresh `MainMenuState`.
- The other ids are Step 5 or deferred.

---

## 4. Design

### 4.1 Crates and module layout

```
sim-core ← sim ← render ← scenario ← ui (NEW, Bevy-free) ← game (Bevy glue)
                                      ↑
                                shot, oracle-tests
```

- **`render`** (extend, Bevy-free).
  - `font`: the CP437 high half (`kHighHalf`, `cp437.cpp:11`), replacing `ascii_to_font_byte`. It is hash-neutral:
    every existing string is ASCII, and the re-diff proves it.
  - `menu`:
    - `draw_item` gains the value arm (`value: Option<&str>`, `value_offset_x`);
    - new `draw_scrollbar` (`menu.cpp:107-124`);
    - new `menu_palette(origpal, menu_cycles)`, which `render::weapsel::weapsel_palette` becomes a thin alias of.
  - new `present::fade_argb`, a port of `FadeArgb`, shared by the live upload and `hash::fade_channel`.
- **`ui`** (new crate; depends on `render`, `assets`, `scenario`, `sim`, `sim-core`):
  - `ui::menu`: `Menu`, `MenuItem`, `MenuModel`, `Behavior` and its family, `search` (§4.2–§4.5).
  - `ui::keys`:
    - `KeyLatch`, the `dos_keys` table plus `key_buf`;
    - the control-key tests over `WormSettings::controls_ex`;
    - the DOS constants the menus read (§4.6).
  - `ui::text`: `game_modes`, `onoff` (finding 3), `time_to_string`, `time_to_string_frames`.
  - `ui::shell`:
    - `ScreenStack` and `Screen`;
    - `MainMenuState` and the two concrete menus (main, settings display);
    - the router;
    - `Match` (the `LocalController` analog);
    - `Shell` (the `Gfx` analog: `RunOneFrame`, `menu_cycles`, the play fade, the shared frozen screen);
    - `LevelSlot` (§4.11).
  - **Moved from `game`, unchanged in behavior:** `new_game`, `selection`, `match_flow` and `input::ReleaseLatch`.
    `game/src/lib.rs` re-exports them (`pub use ui::shell::{new_game, selection, match_flow};`), so existing `game`
    tests and paths keep compiling.
- **`game`** (Bevy glue only):
  - it collects `KeyboardInput` messages into a per-tick queue and maps them with `KeyCode → DOS`, a port of
    `keys.cpp:9-60` including the "unknown → 89" rule (`:70-75`);
  - it turns the touch mask into key events (§7.3);
  - it calls `Shell::frame` from `tick_and_render`, uploads the present with its fade, drives `AudioSink` from
    `FrameOut`, handles QUIT, and keeps the `Scripted` / `Replay` / `--record` paths off the shell.
- **`shot`** gains `--menu [--frames N]`: the boot menu, or the menu after N idle frames, to PNG.

### 4.2 `Menu`, `MenuItem`, `MenuModel`

```rust
pub struct MenuItem { pub color: u8, pub dis_colour: u8, pub string: String,
                      pub has_value: bool, pub value: String,
                      pub visible: bool, pub selectable: bool, pub id: i32 }   // menuItem.hpp:10-36
impl MenuItem { pub fn new(color, dis, s, id) -> Self; pub fn space() -> Self } // :11-22

pub struct Menu { pub items: Vec<MenuItem>, pub item_height: i32, pub value_offset_x: i32,
                  pub x: i32, pub y: i32, pub height: i32, pub top_item: i32, pub bottom_item: i32,
                  pub visible_item_count: i32, pub centered: bool,
                  selection: i32, search: Search }                              // menu.hpp:36-49, :177-196
impl Menu {
    pub fn new(x: i32, y: i32, centered: bool) -> Self;                         // Init + Place
    // menu.cpp, one fn per C++ method, same names in snake_case, same i32 arithmetic:
    // add_item, add_item_at, clear, movement, movement_page, move_to, move_to_id,
    // move_to_first_visible, set_visibility, set_top, set_bottom, scroll, ensure_in_view,
    // is_in_view, item_position, visible_item_index, item_from_visible_index,
    // first_visible_from, last_visible_from, selection, selected_id, index_from_id, item_from_id
    pub fn on_left_right<M: MenuModel>(&mut self, m: &mut M, dir: i32, cx: &mut MenuCx) -> bool;
    pub fn on_enter<M: MenuModel>(&mut self, m: &mut M, cx: &mut MenuCx) -> Enter;
    pub fn update_items<M: MenuModel>(&mut self, m: &mut M, cx: &MenuCx);
    pub fn on_keys(&mut self, keys: &[TypedKey], now_ms: u64, contains: bool);   // §4.5
    pub fn draw<M: MenuModel>(&self, m: &M, bmp: &mut Bitmap, pal: &Pal32, font: &Font,
                              disabled: bool, x: Option<i32>, show_disabled_selection: bool);
}
/// C++ `Menu`'s virtuals: GetItemBehavior, OnUpdate, DrawItemOverlay (menu.hpp:55-67).
pub trait MenuModel {
    fn behavior(&mut self, item_id: i32) -> Behavior<'_>;
    fn on_update(&mut self, _menu: &mut Menu) {}
    fn draw_item_overlay(&self, _item: &MenuItem, _x: i32, _y: i32, _sel: bool, _dis: bool,
                         _bmp: &mut Bitmap, _pal: &Pal32) {}
}
pub struct MenuCx<'a> { pub menu_cycles: u32, pub hooks: MenuHooks, pub sounds: &'a mut Vec<i32>,
                        pub texts: &'a UiTexts }
pub enum Enter { Result(i32), EditValue(ValueEntry) }   // ValueEntry = the InputStringState request (4½e)
```

**Ownership mirrors C++ `GetItemBehavior`.** A `Behavior<'a>` borrows the one field it edits from the model (an
`&'a mut i32`, `bool` or `u32`, like C++ `int& v`). It is built per call and dropped before the `Menu` is touched
again. `Enum::Change` reports "changed", and `on_left_right` / `on_enter` then call `update_items` with the model
borrowed afresh. This is the C++ `menu.UpdateItems(common)` done after the behavior returns: the same effect, with
no aliasing. `on_left_right` returns the behavior's bool, and the caller does `ResetLeftRight` (§4.6), as
`mainMenuState.cpp:583-591` does.

### 4.3 Behaviors (`ui::menu::behavior`)

```rust
pub enum Behavior<'a> {
    Plain,                                            // ItemBehavior
    Integer(Integer<'a>),                             // v, min, max, step, pct, scroll_interval=5,
                                                      // display_div=1, allow_entry=true
    Time(Integer<'a>, /*frames*/ bool),               // allow_entry=false
    Bool(&'a mut bool),                               // no custom setter needed before 4½g
    Enum { v: &'a mut u32, min: u32, max: u32, broken: bool },
    ArrayEnum { v: &'a mut u32, arr: &'a [&'a str], broken: bool },
    Custom(Box<dyn CustomBehavior + 'a>),             // gfx.cpp-local behaviors
}
```

Each arm is a line-for-line port of §3.5, with `u32` wrapping for `Enum::Change`. `menu_cycles` comes from `MenuCx`
(`gfx.menu_cycles`, `integerBehavior.cpp:15`). The value strings are `ui::text` ports: `ToString`, `TimeToString`
with its `'0' + sec/600` first digit, `TimeToStringFrames` = `TimeToStringEx(frames*14)`. `Integer::on_enter` plays
`MenuSelect` and, with `allow_entry` and `ItemPosition` true, returns `Enter::EditValue { x: x + voff + 2, y, digits
= 1 + floor(log10(max/div)), min/div, max/div, div, pct }` (`integerBehavior.cpp:36-80`). 4½e implements the
overlay that consumes it. `digits` is computed with an integer digit count, which equals C++'s
`floor(log10(double))` for every positive `i32`. A unit test sweeps the powers of ten.

**Custom behaviors in 4½d**, display only: `LevelSelect` (`OnUpdate`, including the relabel of
`kSiRegenerateLevel`) and `OptionsSave` (`OnUpdate`, the setup basename). Their `OnEnter` pushes are 4½e's.
`WeaponEnum`, `Key`, `WormName`, `InputDevice` and the profile behaviors are 4½f's.

### 4.4 Navigation, visibility, scrolling, scrollbar

These are ports of §3.4 with the C++ `int` arithmetic and the C++ call graph kept intact. For example, `movement`
calls `move_to`, which calls `first_visible_from` and `ensure_in_view`, even where a shortcut would give the same
result. The G1 corpus (§6.1) exercises the order-sensitive corners:

- `bottom_item = 0` before the first `move_to`;
- `last_visible_from` returning `i + 1` (`menu.cpp:179-187`);
- `MovementPage` landing on the spacer and moving forward;
- `SetVisibility` hiding the selected item;
- scrolling with `vis > height`;
- the scrollbar's integer division at every `top_item`.

`draw` renders rows through `render::menu::draw_item` and the bar through `render::menu::draw_scrollbar`, whose
colours are 0/50 for the arrows and 0/7 for the tab.

### 4.5 Type-to-search

`Search { prefix: String, time_ms: u64 }` is a port of §3.6. **The clock is milliseconds supplied by the caller.**
The live binary passes a monotonic ms counter since app start (`Time<Real>`), and tests pass synthetic values. It is
not a frame count, because C++ compares `SDL_GetTicks` against 1500 (finding 6). `TypedKey` is the key's unshifted
ASCII symbol (`SDL_GetKeyFromScancode(sc, NONE)`) or Tab. The glue derives it from `KeyboardInput.logical_key` with
shift ignored; a US fallback table on the physical `KeyCode` is the plan's choice. The frame's `key_buf` (up to 32
key-downs, repeats included) is carried in the shell input. Nothing in 4½d calls `on_keys` live.

### 4.6 Keys: `KeyLatch` (`gfx.hpp:149-179`, `gfx.cpp:861-976`)

```rust
pub struct KeyLatch { down: [bool; 256], buf: ArrayVec<TypedKey, 32> }   // dos_keys + key_buf
impl KeyLatch {
    pub fn key_down(&mut self, dos: u32, typed: Option<TypedKey>);  // incl. OS repeats (finding 5)
    pub fn key_up(&mut self, dos: u32);
    pub fn test_once(&mut self, dos: u32) -> bool; pub fn test(&self, dos: u32) -> bool;
    pub fn release(&mut self, dos: u32); pub fn clear(&mut self);
    /// TestControlOnce / TestControl / ReleaseControl: over the three WormSettings with
    /// input_device == keyboard, controls_ex[control]; key 0 never matches (gfx.hpp TestAnyKey*).
    pub fn test_control_once(&mut self, ws: &[WormSettings; 3], control: usize) -> bool;
    pub fn test_control(&self, ws: &[WormSettings; 3], control: usize) -> bool;
    pub fn release_control(&mut self, ws: &[WormSettings; 3], control: usize);
}
```

The menu tests use the **settings' DOS bindings**, as C++ does. The defaults are P1
`{0x13,0x21,0x20,0x22,0x1D,0x2A,0x38}`, P2 `{0xA0,0xA8,0xA3,0xA5,0x75,0x90,0x36}`, and the network player = P1
(`settings.cpp:23-60`). When 4½f adds rebinding, the menu follows it for free. The sim input path keeps sampling
`game::input::default_bindings()` levels. 4½f unifies the two.

### 4.7 Palette, `menu_cycles`, fades, presents

`Shell` owns `menu_cycles: u32`, `play_fade: i32` (the play renderer's `fade_value`) and one `frozen_screen`
bitmap. Each frame follows §3.1 exactly:

- a menu frame runs `update_menu_palettes(fading)`: `play_fade += 1` up to 32 unless fading, `menu_cycles += 1`,
  then `pal32 = menu_palette(origpal, menu_cycles)` + `set_worm_colour` ×2 from `settings.worm_settings[i].rgb`;
- a Playing frame adds 1 to `menu_cycles` after the draw.

`Selection` stops counting its own `menu_cycles` (4½c's `WeapselScreen`). It reads the shell's value, which closes
the 4½c §5 caveat. The render gate from 4½c is unaffected: it passes its own start value.

`Shell::frame` returns a `FrameOut`:

```rust
pub struct FrameOut { pub present: Option<Present>, pub menu_sounds: Vec<i32>, pub quit: bool, pub phase: Phase }
pub enum Present { Frame { fade: i32 }, Black }   // Black = Enter's Flip at fade 0 (finding 7)
```

`None` is the pop frame, which presents nothing, and the window keeps its image. The presented pixels are
`fade_argb(surface, fade)`: the Playing screen uses `MatchFlow::fade_value()` and menus use `play_fade`. This is
the first time the live game shows any fade: weapon selection fading in, Esc and game-over fading out, and the menu
fading in and out.

### 4.8 Sounds

Menus push `hooks.menu_move_up` / `menu_move_down` / `menu_select` into `MenuCx.sounds` at the §3.7 call sites, in
call order and including the double `MenuSelect` of the LAN/online placeholders (§5). The glue plays them through
the existing `AudioSink` (a negative hook is skipped at the sink, as in 4½c §4.8). This is a side channel with no
sim involvement. A menu keypress also unlocks WebAudio on the page (rust-map §7).

### 4.9 `ScreenStack` and the screens

```rust
pub enum Screen { MainMenu(MainMenuState), Playing }          // 4½e/f/g add variants (overlays, stats)
pub struct ScreenStack { stack: Vec<Screen>, pending_replace: Option<Screen> }
// push (calls enter), pop (calls leave), schedule_replace_top, top, is_empty; update/draw mirror
// state.hpp:92-131 — draw walks down past is_overlay() screens; update returns `!empty` after a pop.
```

A `Screen` enum is used rather than trait objects, so every dispatch is an exhaustive `match` and a new screen
cannot be forgotten. `is_overlay` and `wants_menu_flip` are per-variant `const` answers
(`MainMenu: false/true`, `Playing: false/false`). The firewall is in the signatures:

- `MainMenuState::update(&mut self, cx: &mut ShellCx) -> bool`. `ShellCx` holds the settings, the two menus,
  `cur_menu`, the key latch, `menu_cycles`, `play_fade`, sounds, texts and `running`. It has **no `SimState`**.
- `Playing` updates through `Match::process(&mut self, sim: &mut SimState, input, ...) -> bool`.
- The router gets `&mut SimState` only to assign a freshly built one (`*sim = loaded.state`).

### 4.10 `MainMenuState` and the concrete menus

- **`MainMenuState { phase: Active | FadingOut, selected: i32, start_item_id: i32 }`.**
  - `enter` is §3.2 step by step. The copyright bar is drawn through the palette the last draw left behind (finding
    12's draw). For a game frame that is `frame::draw`'s `build_palette(origpal, color_anim, cycles, flash)`, which
    `render::frame` exposes. For a selection frame it is the weapsel palette at the current `menu_cycles`. Colours 0
    and 19 lie outside every rotated range (`tc.cfg:197-215`), so only a flash can change them, and the gate pins
    that.
  - `update` is §7.2 in source order.
  - `draw` is `DrawBasicMenu` plus the disabled settings menu.
- **`main_menu()`** builds the table in §3.3. The `MainModel` is a unit model: every item is `Behavior::Plain`,
  as in `mainMenu.cpp:6-10`, because everything is intercepted in `update`.
- **`settings_menu()` + `SettingsModel<'a>(&'a mut Settings)`.** `behavior(id)` is `gfx.cpp:1262-1312`
  item for item, and `on_update` is `:1314-1341`. In 4½d the settings menu is only drawn disabled. `update_items`
  runs at every `enter`, as C++ does (`mainMenuState.cpp:125`).

### 4.11 The router, level reuse, and the shared frozen screen

**`LevelSlot { level: LevelData, provenance: LevelProvenance }`** is C++ `Level` plus its `old_*` fields
(`level.cpp:421-424`). `LevelProvenance { random_level, level_file, random_map_width, random_map_height }` is
recorded whenever the slot is (re)generated.

- **Boot.** `level = generate_level(settings, boot_seed)` (4½b; LD 6). The background is a transient
  `new_match(settings, level)` state drawn with fresh viewports: the never-focused boot `LocalController`,
  `Running() == false`. This is `render::weapsel::build_frozen` without the label. `Sim` holds that state until the
  first NEW GAME replaces it.
- **NEW GAME** (the reconciliation with 4½c's `NewGame::next`):
  1. `seed = seeds.next_match()`.
  2. If `!settings.regenerate_level && provenance == LevelProvenance::of(&settings)`, keep the slot, updated with
     the **played** level: the current sim's `material_id`, craters included, as 4½c's `next` already does.
     Otherwise `level = generate_level(settings, seed)` and the provenance is re-recorded.
  3. `*sim = new_match(settings, level)`. A new `Match` gets a new `Selection` from the in-memory picks and
     `MatchFlow::with_weapon_selection()`. With `skip_selection` (`?weapons=`) it gets 4½c's `build_match` path
     instead.
  4. Push `Playing` and arm the release latch.

  4½c's `NewGame::next` held only the `regenerate_level` half and assumed the settings never change. 4½d adds the
  provenance half, and `NewGame` becomes `LevelSlot` plus `SeedSource`. **The seeds:** `SeedSource::{Fixed(s),
  Fresh}`. `boot_seed` and every match seed are `s` when fixed (`?seed=`), or fresh from the clock. So `?seed=7`
  still gives exactly 4½c's first match: the level from 7 and the sim seeded with 7. Tests inject a scripted
  sequence.
- **RESUME.** `match.focus()`, then push `Playing` and arm the latch. `MatchFlow::focus` is
  `localController.cpp:100-120`: in GameEnded, `going_to_menu = true, fade = 0`; otherwise `going_to_menu = false,
  fade = 0`. A running selection is refocused. C++ key-downs that happened in the menu never reached the
  controller, so the latch masks keys held at the boundary. That is the same edge rule as 4½c §7.2.
- **QUIT.** `FrameOut.quit = true` after the fade-out.
- **Back to the menu** (Playing popped with no selection pending):
  1. `match.unfocus()`;
  2. `keys.clear()`;
  3. the finding-12 draw: `frame::draw` of the current sim with the match's own viewports, which is the only draw
     that tick gets. An unfocused selection copies the frozen screen instead (`weapsel.cpp:182-186`);
  4. push a new `MainMenuState`, whose `enter` yields `Present::Black`.
- **The shared frozen screen (finding 11).** `Selection` keeps only `cached_background: bool`. Its pixels are the
  shell's `frozen_screen`, which `MainMenuState::enter` overwrites. So RESUME into a selection shows the copyright
  bar, exactly as C++ does.
- **Placeholders.** They play `MenuSelect` (the switch-level sound, `mainMenuState.cpp:198`) plus, for the three
  network arms, their second `MenuSelect`. They change no state and never reach the router: the C++ `default:` arm
  would push `GamePlayState` for an unknown id (`gfx.cpp:1593`), and Rust must not. Their F-keys are ignored (§7.2).

### 4.12 `Match` (the `Playing` screen)

`Match { flow: MatchFlow, selection: Option<Selection>, viewports: [Viewport; 2], scene: SceneData, latch:
ReleaseLatch, hud: HudFlags, loadout: Vec<String>, focused: bool }`. Its `process` is 4½c's `tick_and_render`
body, moved verbatim: selection step or `tick_viewports` + drain + `after_frame`. Three changes:

- **Esc** from the frame's key events: a non-repeat key-down **or** a key-up of DOS 1 (finding 9) calls
  `flow.esc()` if `!going_to_menu`. The touch MENU bit counts the same way (§7.3).
- **One shared fade tail for all phases** (`localController.cpp:185-199`). 4½c's `weapsel_frame` only counted up.
  `enter_game` keeps its unconditional 33.
- **`process` returns false** when the tail pops. That is how the screen pops: it replaces 4½c's automatic restart
  on `Finished`.

`running()` is `flow.phase() != GameEnded`. `draw` is 4½c's `render_and_upload` body into the shell surface, with
`menu_cycles` read from the shell. The F5 in-match restart, if kept (Open Q5), is a router call that skips the
menu. It does not recurse into `restart_match`.

---

## 5. What is live, and what is a placeholder

| Surface | 4½d | Owner |
|---|---|---|
| RESUME GAME (F1), NEW GAME (F1), QUIT TO OS | **live** | 4½d |
| Esc / JUMP → cursor to QUIT; Up/Down/PgUp/PgDn; Enter/FIRE; F1 | **live** | 4½d |
| Settings menu, drawn disabled with live values | **live (display)** | 4½d |
| MATCH SETUP (F7): focus and edit settings; LEVEL; WEAPON OPTIONS; SAVE/LOAD SETUP | placeholder | 4½e |
| LEFT/RIGHT PLAYER (F5/F6) | placeholder | 4½f |
| OPTIONS (F2), F10 colour mode, F11 fullscreen | placeholder / ignored | 4½g (hidden subset) |
| HOST/JOIN LAN, HOST/JOIN ONLINE, NETWORK PLAYER (F9) | placeholder | Step 5 |
| REPLAYS (F3), TC | placeholder | deferred (overview §Deferrals) |
| F8 weapon randomiser | never | overview §Deferrals |
| Type-to-search, `Integer::on_enter` entry, overlays (`InputStringState`, `InfoBoxState`) | framework only, not wired | 4½e |

---

## 6. Oracle

Two new C++ dumpers, both on the 4½c A1 pattern: the **real** C++ code runs headless and writes hashes plus
optional PPMs, and the Rust side replays the same script and must match every line. No Rust self-golden is
committed.

### 6.1 G1 — `oracle_dump_menu` (widgets + the settings display)

`src/tools/oracle_dump/menu_dump.cpp`: `oracle_dump_menu <script> <out> [--ppm-dir <dir>]`. It loads the TC's
`Common` (font, palette, texts, sound hooks), `gfx.play_renderer.Init(320,200)`, and a `RecordingSoundPlayer` (the
4½c `weapsel_drive.hpp:51` one). There are two script kinds:

- **Widget scripts** build a real `Menu` subclass whose `GetItemBehavior` maps ids to real behaviors over dumper
  variables:
  - `menu x y height voff centered`;
  - `item id color dis visible selectable "<string>"`;
  - `bind id integer|time|bool|enum|array …`.

  Then they run ops: `move ±1`, `page ±1`, `visible id 0|1`, `move_to i`, `move_to_id id`, `first_visible`,
  `cycles n` (sets `gfx.menu_cycles`), `left`, `right`, `enter`, `keys <names…>` (calls `OnKeys`), `sleep_ms n`
  (a real `SDL_Delay`; only 0, ≤ 1000 or ≥ 1600 are allowed, so wall-clock jitter cannot flip the 1500 ms test),
  `update_items`, and `draw disabled x|-1 show_dis_sel`.

  Output per op: `op <n> sel <s> top <t> bottom <b> vis <c> prefix <p> vals <…> bound <…> sounds <…> ret <r>
  [hash <16>]`. A draw clears the bitmap, draws with the plain TC palette, and hashes with `hash_frame(bmp, 33)`.
- **Settings scripts** load a setup sidecar with the real `Settings::FromToml` into `gfx.settings`, run
  `gfx.LoadMenus()`, set `gfx.settings_node` to `…/Setups/liero.cfg`, and call the real
  `settings_menu.UpdateItems(common)` + `Draw(disabled=true)`. They also run the four `Movement`/`MovementPage`
  sequences. That covers all four game modes (Holdazone draws the scrollbar), `random_level` on and off, and odd
  values such as `time_to_lose = 3599`, `loading_time = 9999` and `blood = 0`.

The Rust `oracle-tests/tests/menu_widget_golden.rs` replays each script through `ui::menu` and
`ui::shell::settings_menu()` and compares each line. **Corpus** (about 10 scripts): `nav_wrap`, `page_scroll`
(20 items, height 15), `visibility`, `scrollbar_positions`, `integer`, `time`, `bool_enum`, `values_draw`
(selected, unselected, disabled, centered, value arm), `search` (prefix, contains, Tab, timeout, single-character
retry), and `settings_{killemall,gametag,holdazone,scales,file_level}`.

### 6.2 G2 — `oracle_dump_shell` (the real `Gfx` frame loop, headless)

`src/tools/oracle_dump/shell_dump.cpp`: `oracle_dump_shell <case> <out> [--ppm-dir <dir>]`, run from the repo
root.

**Setup, as `GameEntry` does** (`gameEntry.cpp:21-76`):

1. `SDL_Init(SDL_INIT_EVENTS)`, `InitKeys()`, `PrecomputeTables()`.
2. `gfx.LoadMenus()`, the settings sidecar (`Settings::FromToml`), and `gfx.settings_node = …/Setups/liero.cfg`.
3. `Common::load(TC)`, `play_renderer.Init(320,200)` + `LoadPalette`, and `single_screen_renderer.Init(640,400)`.
4. **Headless presentation:** `sdl_draw_surface = SDL_CreateSurface(320,200,ARGB8888)`, a software renderer
   (`SDL_CreateSoftwareRenderer` on a second surface), and a streaming texture. `FitScreen` then gives magnification
   1 (`blit.cpp:860-876`), and the real `Flip` → `Gfx::Draw` → `ScaleDraw` writes the **faded presented frame** into
   `sdl_draw_surface`. The spectator window is off.
5. Fallback 1: the SDL `dummy` video driver with the real `SetVideoMode`, which the CLAUDE.md smoke test proves
   runs. Fallback 2: stop and report the blocker (4½c A1's rule), never weaken the gate.

**Driving.** `InitFrameStepping()` runs first, and the dumper writes the `boot` line. Then, for each frame, the
dumper `SDL_PushEvent`s the script's key events for that frame (`KEY_DOWN`/`KEY_UP` with the scancode and the
`repeat` flag) and calls the **real `gfx.RunOneFrame()`**. It stops at `frames` or when `RunOneFrame` returns false
(QUIT).

**Interventions**, each at a point where no real code runs, and each justified:

1. **Boot seed.** `gfx.rand.Seed(boot_seed)` before `InitFrameStepping`. C++ seeds it from the clock
   (`gameEntry.cpp:23`).
2. **Match seeds.** Before every frame whose top is `MainMenuState`, `gfx.rand.Seed(next_match_seed)`. Nothing in
   the main menu draws `gfx.rand`, so reseeding while it is up is invisible. The draw happens only in the dispatch
   (`gfx.cpp:1520`), and there it equals Rust's `generate_level(settings, match_seed)`.
3. **Game seed.** After the frame in which `gfx.controller` changes, `controller->CurrentGame()->rand.Seed(
   match_seed)`. `Game::Game` seeds from `time(nullptr)` (`game.cpp:42`), and that frame already ran the
   `WeaponSelection` constructor (`gamePlayState.cpp:14` → `localController.cpp:112-114`, `:228-229`). The reseed is
   exact **only if the constructor drew nothing** (4½c finding 1: every pick nonzero and enabled, and no RANDOM
   bot). Both the dumper and the Rust test **refuse** a case whose setup violates this.
4. **No stats screen.** At the same point, `game.stats_recorder` becomes the base no-op `StatsRecorder`
   (`game.hpp:125`, `stats_recorder.hpp:10`). The `dynamic_cast` in `gamePlayState.cpp:59` then fails, and game over
   pops to the menu, as 4½d does (finding 13). 4½g drops this intervention.
5. **Pacing and presents.** `gfx.last_frame = 0` before each `RunOneFrame`. `Flip` adds exactly 14
   (`gfx.cpp:1178-1189`), so the frame's present count is `last_frame / 14`, and `Flip` never sleeps.
6. **No replays.** `settings->record_replays = false`, so there are no `.lrp` writes (`localController.cpp:237`).
7. **Sounds.** `gfx.sound_player` is a `RecordingSoundPlayer`, installed before `InitFrameStepping`. It becomes
   `g_sound_player` through each `Game` (`game.cpp:30-39`).

Everything else runs unmodified: the stack, `MainMenuState`, `GamePlayState`, `LocalController` (its 12/3 repeat,
the selection, the Esc fade), `Game::ProcessFrame`, `Game::Draw`, `UpdateMenuPalettes`, `DrawBasicMenu`, `Flip`.

### 6.3 Formats

**Case file** `rust/oracle-tests/golden/shell_<case>_script.txt`. The setup sidecar is
`shell_<case>_setup.cfg` (C++-schema TOML) or `default`.

```
setup <file|default>
boot_seed <u32>
match_seed <u32>            # one per NEW GAME, consumed in order
frames <n>
key <frame> down|up|repeat <NAME>   # NAME = a DOS-table key name (ESC, UP, RETURN, LCTRL, R, F1, …)
```

**Golden** `shell_<case>.txt`:

```
boot <presents> <presented16>
f <frame> <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> <sel> <sounds>
end <frame> quit|frames
```

- `<presented16>` is FNV-1a over `sdl_draw_surface`'s RGB, the identical hash to `render::hash::hash_frame`.
  When `presents = 0` it is `-`.
- `<bmp16>` is `hash_frame(play_renderer.bmp, 33)`.
- `<top>` is `M` for the main menu, `G` for Playing, or `-` for empty.
- `<sel>` is `main_menu.Selection()`.
- `<sounds>` is the logged hook ids in order, or `-`.

### 6.4 G2 corpus (generated by `examples/gen_slice4_5d.rs`, about 10 cases)

| Case | Pins |
|---|---|
| `boot_idle` | the boot black frame; fade-in 1…32; rotation through 168…174; the settings display at defaults; the copyright bar with `ä` |
| `nav` | wrap both ways over the spacer; R/F as Up/Down; OS `repeat` events; PgUp/PgDn; Esc and LAlt → QUIT; the Up+R short-circuit (finding 15); held Left/Right as a no-op; placeholder Enters (sounds only); F2/F7 ignored |
| `level_file` | `random_level = false` + `level_file = Levels/water_stage.lev`: the boot level from the file; LEVEL `"water_stage"`; RELOAD LEVEL; MAP W/H hidden |
| `gametag` | TIME TO LOSE `10:00` visible, LIVES hidden (boot only) |
| `milestone` | §6.6 |
| `esc_in_weapsel` | Esc mid-selection: 32 selection frames keep running; the background is the unfocused frozen screen; RESUME shows the copyright bar (finding 11) |
| `reuse` | play → Esc → NEW GAME: the level is reused with its craters under a new match seed |
| `regenerate` | setup `regenerateLevel = true`: NEW GAME generates from the match seed |
| `game_over` | setup `lives = 1` plus a scripted self-kill (the generator searches seeds/scripts, as 4½a did): 180 post-mortem frames (the last 32 fading), → the menu with RESUME hidden and `NEW GAME (F1)` |
| `f1_quit` | F1 at boot (NEW GAME), F1 in the pause menu (RESUME), then QUIT with its full fade-out and `end quit` |

Every case satisfies intervention 3's no-draw precondition, and the generator asserts it. `gen_shell_golden.sh`
and `gen_menu_golden.sh` follow the 4½c scripts (`PRESET`, `env.sh`, and awk gates such as "exactly one `boot`
line" and "`presents ∈ {0,1}`").

### 6.5 The Rust side of G2 — `oracle-tests/tests/shell_golden.rs`

The harness parses the script and builds a `Shell` over `SeedSource::Scripted(boot, matches)` and the sidecar
`Settings`. Per frame it:

1. turns the script's events into `ShellInput` key events;
2. derives the **sampled** control words from the keys held at the frame start, through the settings' DOS
   bindings. Key-down and key-up take effect at the frame boundary in both implementations, which is the 4½c
   equivalence argument; the corpus avoids a key-down and key-up in the same frame;
3. calls `Shell::frame(&mut sim, …)`;
4. compares the whole `f` line:
   - `presents` and the presented hash from `FrameOut.present`, where `Black` is the all-black hash;
   - `bmp16` from the shell surface;
   - `fade`, `menu_cycles`, `top` and `sel` from shell accessors;
   - `sounds`, strictly on `M` frames and weapon-selection frames only. Match-frame sim sounds belong to 4c's
     parity and are recorded but not gated.

A mismatch prints the first differing column and writes both PPMs (`SHELL_RUST_PPM_DIR`, the 4½c convention).

### 6.6 🎯 The milestone test

`shell_milestone` is one script that walks the done-when path. Seeds are scripted; the default setup is used.

1. Boot, idle 40 frames.
2. Down, Up.
3. Enter on NEW GAME (F1): fade-out 32 frames, then the pop frame.
4. Weapon selection: P1 Up + Fire, P2 Up + Fire.
5. Play 150 ticks of scripted movement and fire.
6. Esc: 31 fade frames, then the pop frame and its black flip. The menu shows `RESUME GAME (F1)` and `NEW GAME` over
   the game frame and the bar.
7. Enter on RESUME: fade-out, then play fades back in. Play 60 ticks.
8. Esc, then Down, Enter on NEW GAME. The level is reused, and selection starts again.
9. Esc during selection, then Esc again (cursor to QUIT), then Enter: the quit fade, then `end quit`.

About 700 frames. **The milestone is this case bit-exact on every line**, and it runs in CI as an ordinary
`oracle-tests` test.

### 6.7 Eyeball artefacts and the browser (not gates)

- **Xvfb.** Both the real `openliero` and the Rust `game` run under `Xvfb :99` (`SDL_VIDEODRIVER=x11
  SDL_AUDIODRIVER=dummy`), driven by the same `xdotool` sequence (boot, NEW GAME, DONE ×2, play, Esc, RESUME, Esc,
  QUIT). The screenshots are paired side by side in the scratchpad for John (4½c A2).
- **Headless Chromium** (the 4½c T10 precedent), on desktop and on an emulated phone in portrait and landscape: boot
  menu → NEW GAME → selection → play → MENU/Esc → RESUME → QUIT, which shows the page's play-again overlay. It also
  checks the `?weapons=`, `?level=` and `?menu=1` routes and `window.lieroPhase` at each step.

### 6.8 Standing gates

- Every task runs `cargo test --workspace --exclude game` (debug) and `cargo test -p game`, and no golden may
  change.
- The CP437 font change is re-diffed on its own task (§9 T0).
- `git diff --name-status <base> -- rust/oracle-tests/golden` shows only `A` lines for `menu_*` and `shell_*`.
- The C++ changes are confined to the two new dumpers (plus an optional shared `shell_drive.hpp`) and two target
  lines in the `OPENLIERO_BUILD_ORACLE_DUMP` block of `CMakeLists.txt`.
- Both dumpers pass clang-format 22 (whole file) and clang-tidy (diff).

---

## 7. The live game

### 7.1 Boot and the frame

`setup` builds a `Shell` for a bare native run or the default preview, and starts it with either the menu (the C++
boot) or a skip route (§7.5). `tick_and_render` does four things:

1. It drains the key queue. An `Update` system fills the queue from `KeyboardInput` messages; the C++
   `SDL_PollEvent` loop is the model. With several fixed ticks per render frame, the first tick takes all queued
   events, as a C++ frame polls everything pending.
2. It adds the touch edges.
3. It calls `shell.frame(&mut sim.0, &input)`.
4. It uploads per `FrameOut.present` (`None`: no upload), plays `menu_sounds`, drains the sim's sound events while
   Playing, and handles `quit`.

`close_on_esc` runs only on the non-shell paths (Scripted, `--replay`, `--live --record`), where Esc still quits
and flushes.

### 7.2 Keyboard (C++-exact; `mainMenuState.cpp:171-602`, in that order)

| Keys (default bindings) | In the main menu | In play / selection |
|---|---|---|
| Esc; LAlt (P1 jump), RShift (P2 jump) | cursor to QUIT TO OS (`:171-179`) | Esc: pause (fade 31 → menu); the jump keys stay game input |
| ↑; R (P1 up) — ↑ is also P2 up | `Movement(-1)`, `MenuMoveDown` (`:181-185`) | game input |
| ↓; F | `Movement(+1)`, `MenuMoveUp` (`:187-192`) | game input |
| Enter, KP Enter; LCtrl, RCtrl (fire) | select + `MenuSelect` (`:194-198`) | game input |
| F1 | NEW GAME or RESUME at once (`:432-436`) | — |
| F2, F3, F5, F6, F7, F9 | ignored in 4½d (placeholders, §5) | F5: see Open Q5 |
| ←/→; D/G (held) | `OnLeftRight`, a no-op on the main menu (`:581-592`) | game input |
| PgUp / PgDn | `MovementPage(∓1)` + sounds (`:594-602`) | — |
| OS key repeat | repeats Up/Down/PgUp/PgDn/Esc (finding 5) | ignored (the controller drops repeats) |

In a browser the page may claim F1 and F5. Enter covers F1, and the help text says so.

### 7.3 Touch (`game::touch`, `web/index.html`)

The on-screen controls feed the menu as **key events on P1's bound DOS keys**. A rising edge of a touch bit is a
key-down of `settings.worm_settings[0].controls_ex[bit]`, a falling edge is a key-up, and there are no repeats. To
the menu, touch is then exactly P1's keyboard, via C++'s `TestControlOnce` path. The sim path is unchanged: the
mask ORs into P1's sampled word (`touch.rs:28-45`).

| Touch control | Menu (C++ path) | Selection | Match |
|---|---|---|---|
| pad ↑ / ↓ | cursor up / down + sound (`kUp`/`kDown` control) | cursor | aim |
| pad ← / → | `OnLeftRight` (no-op on the main menu) | cycle weapon | move |
| FIRE | select (`kFire`, `:195`) | RANDOMIZE / DONE | fire |
| JUMP | cursor to QUIT (`kJump`, `:172`) | — | jump |
| WEAPON | — | — | change |
| DIG | ← + → held (no-op) | net-zero cycle | dig |
| **MENU (new, bit 8)** | Esc (cursor to QUIT) | pause | pause |

Without MENU a phone cannot leave a match, because Esc exists only on a keyboard. `TOUCH_MENU = 1 << 8` is ignored
by `touch_state` (unknown bits, `touch.rs:27-40`), so it never reaches the sim. The hint line follows
`window.lieroPhase` (`menu`, `weapsel`, `game`, `quit`). The touch-only rule from 4½c (Q8), under which P2 is an
auto-ready KEEP bot, still applies to every NEW GAME.

### 7.4 QUIT

- **Native.** QUIT writes `AppExit::Success` after its fade-out. Closing the window quits at once (C++ `SDL_EVENT_QUIT`,
  `gfx.cpp:1477`). There is no settings save, because 4½d does no config I/O (Open Q7). C++ saves on exit
  (`gameEntry.cpp:78`).
- **Browser.** C++'s own emscripten build cancels the main loop and leaves the last frame, which is black
  (`gfx.cpp:1657-1669`). Rust does the same: the shell stops, the canvas keeps the black frame, and
  `window.lieroPhase = "quit"`. The page then shows a "Quit — tap or press a key to play again" overlay that reloads
  the page. **Open Q3 (John).**

### 7.5 Preview parameters and the native CLI

| Start | 4½c | 4½d |
|---|---|---|
| bare URL / bare `cargo run -p game` / `--live` | weapon selection | **main menu** (the C++ boot) |
| `?weapons=A,B` | match at tick 0 (selection skipped) | unchanged: the menu is skipped too |
| `?level=x` | selection on `Levels/x.lev` | unchanged: the menu is skipped; the level comes from the file |
| `?seed=N` | selection; level and sim from N | unchanged: the menu is skipped; boot/level seed N, first match seed N |
| `?menu=1` (new) | — | force the menu even with the parameters above: they configure the boot level and every NEW GAME |
| `?demo` | scripted `blood` | unchanged (no shell) |
| `?touch=0/1` | page controls; touch-only P2 bot | unchanged, plus the MENU button |
| `--live --record p` | default-match fixture, selection skipped | unchanged: no shell; Esc quits and flushes |
| `<scenario>` / `--replay p` | Scripted / Replay | unchanged (no shell) |

A skipped start builds the shell with `[Playing]` in place of `[MainMenu]`, so Esc/MENU still opens the pause menu.
NEW GAME from that menu keeps the parameters: the file level, the fixed seed, and the selection skip for
`?weapons=`. **Open Q4 (John).**

### 7.6 Recording

`--record` keeps 4½c's path, a scenario match outside the shell. **Menu-driven matches are not recordable in 4½d.**
A reused level carries the craters of a *played* match, so settings plus seed cannot reproduce it. Recording such a
match needs a level snapshot. 4½a design §4.4 and 4½c §7.5 assigned this to 4½d. **Open Q6 (John)** proposes to
re-defer it.

---

## 8. Out of scope (deferrals)

- Everything marked placeholder in §5, with its owner.
- Settings interaction (MATCH SETUP), the level selector, weapon options, save/load setup, the `InputStringState` and
  `InfoBoxState` overlays, and wiring type-to-search: 4½e.
- **Config I/O** (loading and saving `liero.cfg` through `ConfigStore`): 4½e, with the settings menu (Open Q7).
  4½d uses `Settings::default()`, which is byte-equal in content to the shipped `data/Setups/liero.cfg` (4½a-1).
  The binary still writes nothing (4½a design §9.3.6).
- The player menu, key rebinding, and unifying `KeyCode` and DOS bindings: 4½f.
- The stats screen (and dropping dumper intervention 4): 4½g.
- localStorage: 4½h.
- The spectator window and `DrawSpectatorInfo` (finding 2), gamepads, F10/F11, the F8 easter egg, `kMaTc`
  re-initialisation, the replay browser, and single-screen replay.
- Recording menu-driven matches (§7.6).
- Step 5: the network states and dispatch arms. The `ScreenStack` gains variants and none of this design changes.

---

## 9. Task outline (the plan details each)

| Task | Deliverable | Gate |
|---|---|---|
| T0 | `render`: CP437 high half; `draw_item` value arm; `draw_scrollbar`; `menu_palette`; `present::fade_argb`; `frame` exposes its palette | unit + **full re-diff** (CP437 hash-neutral) |
| T1 | `ui` crate: `menu` (items, navigation, visibility, scroll, draw), `text`, `keys::KeyLatch` | unit; `cargo tree` has no Bevy |
| T2 | `ui::menu` behaviors + type-to-search + the settings display model | unit |
| T3 | C++ `oracle_dump_menu` + CMake | clang-format / clang-tidy |
| T4 | G1 corpus, `gen_menu_golden.sh`, goldens, `menu_widget_golden.rs` | **G1 line for line** |
| T5 | `ui::shell`: `ScreenStack`, `MainMenuState`, router, `LevelSlot` / `SeedSource`, `Match` (moved modules, `MatchFlow` Esc/focus/tail, shared frozen screen), `Shell::frame` / `FrameOut` | headless unit flow tests; `cargo test -p game` via the re-exports |
| T6 | C++ `oracle_dump_shell` (headless `Gfx`, the seven interventions, fallbacks) + CMake | clang; a boot-only smoke case |
| T7 | G2 corpus (`gen_slice4_5d.rs`), `gen_shell_golden.sh`, goldens | generator ledgers + awk gates |
| T8 | **MILESTONE**: `shell_golden.rs`, all cases, `shell_milestone` included | **G2 bit-exact** |
| T9 | `game` glue: event queue + `KeyCode → DOS`, touch edges + MENU, shell in `tick_and_render`, faded present, QUIT (native / wasm), `?menu=` and the skip routes, `close_on_esc` scope, F5 (per Q5); `web/index.html` (MENU button, phase hints, quit overlay, help text), `preview.yml` text; `shot --menu` | `cargo test -p game` (glue unit tests); native smoke run |
| T10 | Xvfb side-by-side PNGs; headless-Chromium walk | eyeball artefacts |
| T11 | Full re-diff, wasm build, golden audit, PROGRESS + overview + maps corrections, `liero-shot` §7, broad review | CI commands |

Dependencies: T0 → T1 → T2 → T4 (and T3 → T4); T2 → T5 → T8 (and T6 → T7 → T8); T5 → T9 → T10; T11 is last.
T3 and T6 (C++) can run in parallel with T0–T2.

---

## 10. Risks

- **Running the real `Gfx` headless.** `MainMenuState::Enter` calls `Flip` and `Process` directly, and `Flip`
  needs an SDL surface, renderer and texture. Mitigation: a software renderer on plain surfaces; fallback to the
  dummy video driver, which the smoke test proves; then stop and report. The whole gate depends on this. T6 proves
  it on a boot-only case before any corpus work.
- **The interventions could hide a divergence.** Each one sits where no real code runs, and each is documented.
  Intervention 3's precondition is machine-checked on both sides, and intervention 4 is removed in 4½g.
- **Present semantics are subtle.** They include the pop frame's non-present, the black flip, and a fade read from
  two different counters. The `presents` and `fade` columns pin them, and the milestone crosses every transition
  twice.
- **Drawing each tick exactly once (finding 12).** An extra `frame::draw` moves the camera and the shake RNG, and the
  frames after RESUME then diverge. `milestone` and `reuse` catch it on the first frame after RESUME.
- **The shared frozen screen and `menu_cycles` threading.** Both are cross-screen state that is easy to "tidy up"
  wrongly. They are pinned by `esc_in_weapsel` and by the `menu_cycles` column.
- **Input model.** Menus use events with OS repeat, and the sim uses sampled levels. OS repeat rates differ between
  native and browser, which affects presentation only. The corpus uses explicit `repeat` events. The Esc key-up
  quirk (finding 9) and the `Up || R` quirk are ported deliberately.
- **Refactor churn.** Four modules move from `game` into `ui`, and `main.rs` gets its biggest restructure since 4f.
  Mitigation: re-exports, moves with no behavior change in their own commit, and the 4½c `game` tests unchanged.
- **The boot draw with empty weapon slots.** The boot `LocalController` has no weapons. The Rust HUD's
  `expect("current weapon has a type")` is guarded by `ammo > 0`, as in C++ (`viewport.cpp:101-103`;
  `render/src/hud.rs:133-138`). `boot_idle` proves it.
- **Loop sounds across the pause.** C++ does not process the controller while the menu is up. The plan checks what
  its `SoundPlayer` does with running loops, and matches it in the `Drainer`'s reap.
- **Browser keys.** F1 and F5 may be taken by the browser. Enter and the touch controls always work.

---

## 11. Test strategy

1. **Unit (`ui::menu`, `ui::keys`, `ui::text`).** Every navigation corner in §4.4; each behavior's
   left/right/enter/update, including the `scroll_interval` gate, the `Enum` `u32` wrap and `broken`; `digits`; the
   `TimeToString` digits; search timing with synthetic ms; the `KeyLatch` once/held/release/clear rules and the
   control-key tests over the default DOS bindings.
2. **Differential G1.** About 10 widget and settings-display scripts, line for line against the real C++.
3. **Headless shell (`ui::shell`).** Stack push/pop/replace/overlay order; router arms; `LevelSlot` reuse versus
   regenerate for each of the five fields; `SeedSource` sequences (`?seed=` equals 4½c's first match); `MatchFlow`
   Esc, focus and the shared tail (including Esc during selection with the fade restarting at 33); a placeholder
   never reaching the router; the latch at RESUME.
4. **Differential G2 and the milestone.** About 10 cases of the real C++ frame loop, every frame.
5. **`game`, headless.** `KeyCode → DOS` (unknown → 89); touch edges → key events and MENU → Esc;
   `web_params` `?menu=` and the skip routes; `round_trip.rs` and `record_regression.rs` unchanged.
6. **Eyeball.** Xvfb C++ | Rust PNGs and the headless-Chromium walk.
7. **Standing.** Full re-diff (debug), the golden audit, the wasm build, and a native smoke run: bare
   `cargo run -p game`, then NEW GAME, DONE ×2, play, Esc, RESUME, Esc, QUIT.

---

## 12. Open questions (with recommendations)

**Rulings (John, 2026-09-26):** show all 15 main-menu items, the unported ones inert; QUIT in
the browser stops on a black frame with a "Play again" overlay that reloads; `?weapons=` /
`?level=` / `?seed=` skip the menu (plain link → main menu; `?menu=1` forces it); the touch
controls gain a MENU (Esc) button. The other John-questions follow the recommendations below
(the Bevy-free `rust/ui` crate; F5 as a Rust-only restart in play and selection; recording of
menu-driven matches postponed; `liero.cfg` load/save in 4½e). The plan settles the rest.

1. **Where does the shell live?** *(John — it amends LD 2's location)* **Recommendation: a new Bevy-free crate
   `rust/ui`** holding the widgets and the shell, plus `new_game`, `selection`, `match_flow` and `ReleaseLatch` moved
   out of `game` and re-exported there. The C++ frame gate needs `oracle-tests` and `shot` to drive the whole
   shell, and they cannot depend on the Bevy crate. It also settles overview Q1.
2. **Show the unported main-menu items, or hide them?** *(John)* **Recommendation: show all 15, as C++ does.**
   Selecting a placeholder plays the select sound and does nothing, and its F-key is ignored. The pixel gate needs
   the real layout, and each item lights up in the slice that ports it.
3. **What does QUIT do in a browser?** *(John)* **Recommendation: what C++'s own web build does.** The game stops on
   the final black frame, and the page shows a "tap or press a key to play again" overlay that reloads.
4. **Should preview parameters skip the menu?** *(John)* **Recommendation: yes.** `?weapons=`, `?level=` and
   `?seed=` skip it, so every existing preview link behaves exactly as today. A bare URL shows the menu, and a new
   `?menu=1` forces it. Esc (or MENU on touch) still opens the pause menu in every case.
5. **Keep F5 as "restart the match"?** *(John)* C++ has no such key: in the C++ menu F5 opens LEFT PLAYER. 4½c added
   the restart for testing. **Recommendation: keep it during play and weapon selection only**, as a documented
   Rust-only shortcut straight to the next NEW GAME. In the menu it stays inert until 4½f gives it its C++ meaning.
6. **Record menu-driven matches in 4½d?** *(John)* 4½a and 4½c had promised it to 4½d. **Recommendation:
   re-defer.** A reused level carries the craters of a played match, so settings plus seed cannot reproduce it.
   `--record` keeps working on the scenario path. The natural home is `.lrp` writing (a later slice or Step 5).
7. **Load and save `liero.cfg` in 4½d?** *(John)* **Recommendation: no, 4½e.** Nothing in 4½d can change a
   setting, and the defaults equal the shipped `liero.cfg`. Loading would only matter for someone with a customised
   C++ config, and it drags in 4½a's "writes defaults into the user folder" behaviour early. 4½e wires loading and
   saving together with the settings menu.
8. **Add a MENU button to the touch controls?** *(John — UI)* **Recommendation: yes.** It is a small button, top
   right, acting as Esc. Without it a phone cannot pause or quit a match.
9. **Menu key repeat.** *(plan)* **Recommendation: follow C++.** Keyboard OS repeat events set the key flag, using
   Bevy's `KeyboardInput::repeat`. Touch gives edges only, like a C++ gamepad, and 4½e can add repeat for long
   lists if needed.
10. **Draw the disabled settings menu in 4½d?** *(plan)* **Recommendation: yes** (finding 1), with the display
    half of the settings behaviors. The screen is not C++-faithful without it. Focus and editing stay in 4½e: LEVEL,
    WEAPON OPTIONS and SAVE/LOAD need the overlays.
11. **The C++ headless strategy and its interventions.** *(plan)* **Recommendation: §6.2 as written.** Use a
    software renderer, fall back to the dummy video driver, then stop and report. Keep exactly the seven
    interventions, and refuse any case that breaks intervention 3's precondition.
12. **Is the widget gate G1 worth a second dumper?** *(plan)* **Recommendation: yes.** The scrollbar, the
    behaviors' left/right/enter, the value arm on a selected item, scrolling and search are not reachable from the
    main menu in 4½d. 4½e and 4½f build on them, and gating them now against the real `Menu` is cheaper than
    debugging them through a settings screen later.
13. **The type-to-search clock.** *(plan)* **Recommendation: caller-supplied milliseconds** (`Time<Real>` live,
    synthetic in tests), matching C++'s `SDL_GetTicks` comparison. Not frames.
14. **Port the Esc key-up pause, the `Up || R` double move, and the copyright bar surviving RESUME into selection?**
    *(plan)* **Recommendation: yes, all three.** They are C++ behaviour, cheap to reproduce, and the G2 cases
    `nav` and `esc_in_weapsel` pin them.
