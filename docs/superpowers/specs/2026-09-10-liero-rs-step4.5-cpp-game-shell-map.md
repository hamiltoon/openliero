# Step 4½ — C++ game-shell map (everything outside the in-match simulation)

Status: **RESEARCH MAP** · 2026-09-10 · feeds `2026-09-10-liero-rs-step4.5-game-shell-overview.md`
Part of: `2026-06-26-liero-rs-roadmap.md` (Step 4½ — game shell)
Precedent: `2026-07-12-liero-rs-step4-cpp-input-replay-map.md`

This maps what the ORIGINAL C++ game does between launch and quit, outside `Game::ProcessFrame`:
the state machine / screen flow, every menu, the weapon-selection phase, level selection + random
level generation, settings/profile persistence, AI-controlled worms, and match end. The Rust
rewrite (Steps 0–4) currently only has a hard-coded default match; Step 4½ ports the rest.
All paths are relative to the repo root; line refs are `file:line` on `master` (4aba9f8).

---

## 0. Entry point and top-level loop

| Step | Where |
|---|---|
| `main` → `GameEntry` | `src/game/main.cpp:10`, `src/game/gameEntry.cpp:21` |
| Seed the **presentation** RNG from `time(nullptr)` | `gameEntry.cpp:23` (`gfx.rand.Seed(...)`) |
| Resolve data/user paths (`--config-root`, `portable.txt`, `SDL_GetPrefPath`) | `gameEntry.cpp:30` → `paths::Resolve` (`src/game/filesystem.hpp:160`) |
| `SDL_Init(VIDEO|GAMEPAD)`, `InitKeys()`, `PrecomputeTables()` | `gameEntry.cpp:42-46` |
| **Build all menus** (static item lists) | `gameEntry.cpp:48` → `Gfx::LoadMenus` (`src/game/gfx.cpp:440`) |
| Load `Setups/liero.cfg`; write defaults if missing | `gameEntry.cpp:55-58` |
| Load the TC (`TC/<settings->tc>`), palette, colour mode | `gameEntry.cpp:65-70` |
| `SetVideoMode()`, sound player | `gameEntry.cpp:72-74` |
| `gfx.MainLoop()` | `gameEntry.cpp:76` → `gfx.cpp:1654` |
| On exit: save settings to **user** dir | `gameEntry.cpp:78` |

`Gfx::MainLoop` (`gfx.cpp:1654`) is `InitFrameStepping(); while(RunOneFrame()){}` with an outer
loop that re-inits after a **TC change** (`gfx.cpp:1672-1685`). Emscripten uses
`emscripten_set_main_loop_arg` on the same `RunOneFrame` (`gfx.cpp:1657`).

---

## 1. Screen / state flow

### 1.1 The stack machine

`AppState` + `StateStack` — `src/game/state.hpp:12` / `:47`.

- Virtuals: `Enter/Leave/HandleEvent/Update/Draw`, `IsOverlay()` (`state.hpp:34`), `WantsMenuFlip()` (`state.hpp:38`).
- `Update()` returning **false** = "pop me".
- `ScheduleReplaceTop()` (`state.hpp:75`) — deferred replace, used from inside `Update()` so a state doesn't destroy itself; checked before the pop (`state.hpp:101-105`).
- `Draw()` walks down past overlay states and draws bottom→top (`state.hpp:117-131`).

Overrides: `GamePlayState::WantsMenuFlip() == false` (`gamePlayState.hpp:13`), `StatsState` too
(`statsState.hpp:18`), `RematchState` explicitly true (`rematchState.hpp:26`).
`InputStringState::IsOverlay() == true` (`inputState.hpp:23`) — the menu below repaints beneath the text field.

(**4½e-1**, plan facts 3–7, 9, 12; the source checked for the port.) Sub-states are pushed *inside*
`MainMenuState::Update` / `WeaponMenuState::Update` and `Push` runs `Enter` at once (`state.hpp:50-54`); the
rest of that `Update` keeps running. `menuStatePtr_` (set at push, `gfx.cpp:1463`, `:1623`; cleared only at the
router's dispatch, `:1494`) keeps pointing at the `MainMenuState` while a `WeaponMenuState`, `InputStringState` or
`InfoBoxState` sits above it, so `kMenuSelection` / `kMenuFadingOut` (`:1488-1489`) come from the main menu
wherever it is. **`InputStringState`** (`inputState.cpp:13-99`): `HandleEvent` handles every event in order, even
after `done_` — text typed after Return in the same poll still lands; Backspace / Return / KP Enter / Esc are tested
by SDL scancode on every key-down, repeats included; each `SDL_EVENT_TEXT_INPUT` string goes through `Utf8ToDos`
(`text.cpp:99-118`), which makes the **whole string one byte** — a 1-byte string that byte, å ä ö Å Ä Ö their CP437
codes, anything else (a multi-character string included) `'?'`; `Font::DrawString` UTF-8-decodes, so a typed å is
drawn as U+FFFD; the strip restore is `BlitBitmap(bmp, frozen, kClrX, y, kClrX + 10 + kWidth, 8)` — the width
argument is `kClrX + 10 + kWidth` (`:92`). **`InfoBoxState`** (`:166-217`) is not an overlay; any key-down
(repeats included) dismisses it, a key-up never; `Update` does `ClearKeys`, the optional `Fill(bmp, 0)`,
`on_dismiss`, pop; `Draw` with `clear_screen` switches to `exepal` first.

### 1.2 The state classes

| State | File | Role |
|---|---|---|
| `MainMenuState` | `mainMenuState.cpp:93` (+ `.hpp:6`) | main/settings/player/hidden menus, all F-keys |
| `GamePlayState` | `gamePlayState.cpp:14` | drives `Controller::Process()`; net polling; routes to Stats/Rematch/InfoBox |
| `WeaponMenuState` | `weaponMenuState.cpp:24` | weapon *availability* table (weap_table) |
| `FileSelectorState` (+ `LevelSelectorState`, `ReplaySelectorState`, `ProfileSelectorState`, `OptionsSelectorState`, `TcSelectorState`) | `fileSelectorState.cpp:21,71,160,188,210,230` | directory pickers |
| `InputStringState`, `WaitForKeyState`, `InfoBoxState` | `inputState.cpp:13,102,166` | text entry, key binding capture, modal message |
| `StatsState` | `statsState.cpp:206` | post-match stats screen |
| `RematchState` | `rematchState.cpp:14` | netplay rematch lobby (Step 5) |
| `NetConnectState` | `netConnectState.cpp:16` | LAN host/join (Step 5) |
| `OnlineConnectState` | `onlineConnectState.cpp:16` | room-code / ICE / signaling (Step 5) |

### 1.3 Initialisation and the "menu background is a live match"

`Gfx::InitFrameStepping` (`gfx.cpp:1439`):
1. `controller = LocalController(common, settings)` (`:1442`)
2. Generate a level **immediately** and `SwapLevel` it in (`:1445-1447`) — this is why the menu has a real level behind it.
3. `Focus()` both renderers, draw one frame into both (`:1450-1459`).
4. Push `MainMenuState` (`:1462-1464`).

### 1.4 `Gfx::RunOneFrame` (`gfx.cpp:1467`) — the per-frame loop

```
poll SDL events → SDL_EVENT_QUIT or Alt+F4 → return false        (:1476-1485)
snapshot menuSelection + isFadingOut BEFORE Update              (:1488-1489)
state_stack.Update()
  if it returned false (top popped):
    if a menu selection was pending → dispatch (below)
    else → back to menu: reset net_session, primary_renderer,
           controller->Unfocus(), ClearKeys(), redraw background,
           push a new MainMenuState                              (:1594-1625)
  else:
    if top->WantsMenuFlip() → UpdateMenuPalettes(fading)         (:1631-1635)
    single_screen_renderer.gpu_world_* reset                     (:1640-1641)
    state_stack.Draw(); Flip()                                   (:1643-1648)
```

The **menu-selection dispatch** (`gfx.cpp:1493-1593`) is the real "screen router":

| Selection | Action |
|---|---|
| `kMaQuit` | `return false` — quits (`:1496`) |
| `kMaTc` | `tcChangeRequested_ = true`, drop controller, return false → outer loop re-inits (`:1500-1504`) |
| `kMaNewGame` | new `LocalController`; **reuse the old level** unless `regenerate_level` or random/level_file/w/h changed, else `GenerateFromSettings` (`:1507-1523`) |
| `kMaResumeGame` | if replay, switch primary renderer to `single_screen_renderer` @640×400 (`:1524-1530`) |
| `kMaReplay` | same if `single_screen_replay` (`:1531-1535`) |
| `kMaHostGame` / `kMaJoinGame` / `kMaHostOnline` / `kMaJoinOnline` | net states (`:1536-1589`) — Step 5 |
| anything else | push `GamePlayState` (`:1593`) |

`MainMenuState` communicates results via `gfx->pending_menu_selection` (`gfx.hpp:381`, read at
`mainMenuState.cpp:163`) and `gfx->pending_net_address` (`gfx.hpp:376`).

### 1.5 `MainMenuState` details

- `Enter()` (`mainMenuState.cpp:95`): fade to black, draw the copyright bar (`:107-108`), show/hide `RESUME GAME` based on `controller->Running()` and set F1 target (`:110-119`), set the TC item label (`:121`), snapshot `frozen_screen` + `frozen_spectator_screen` and draw the level minimap into the spectator screen (`:131-143`).
- `Update()` keys (`:171-602`):
  - **Esc / Jump / gamepad EAST**: if in main menu → jump cursor to QUIT; else → back to main menu (`:171-179`)
  - **Up/Down / gamepad DPad**: `cur_menu->Movement(∓1)` + `SoundMenuMoveDown/Up` (`:181-192`)
  - **Enter / Fire / gamepad SOUTH**: `OnEnter` dispatch, per-menu (`:194-430`)
  - **F1** start/resume, **F2** hidden menu, **F3** replays, **F5/F6** player 1/2 settings, **F7** match setup, **F9** network player, **F8** = *weapon randomiser easter egg* (`:432-579`)
  - **Left/Right (held)**: `cur_menu->OnLeftRight` (`:581-592`)
  - **PageUp/PageDown**: `MovementPage` (`:594-602`)
- Selecting anything sets `selected_` → `phase_ = kFadingOut`, `fade_value = 32` (`:604-609`); fade-out counts down in `Update()` (`:153-159`) then pops.
- `Draw()` (`:614`): `DrawBasicMenu()` (frozen screen + main menu), `DrawSpectatorInfo()`, then `cur_menu->Draw` — settings menu is drawn *disabled* when main menu has focus (`:621-625`).
- (**4½e-1**, plan facts 1, 2.) `cur_menu` is a **`Gfx` member** (`gfx.hpp:321`), not `MainMenuState` state:
  `DrawBasicMenu` draws the main menu *disabled* whenever `cur_menu != &main_menu` (`gfx.cpp:1702`), so
  `WeaponMenuState::Draw` (which calls it) shows a disabled main menu; `MainMenuState::Enter` resets it to the main
  menu (`mainMenuState.cpp:129`). With settings focus, Enter plays `MenuSelect` itself only in the four push arms
  (LEVEL, WEAPON OPTIONS, LOAD SETUP, SAVE SETUP AS…, `:275-311`); the `default:` arm calls
  `settings_menu.OnEnter` and the *behavior* plays it (`integerBehavior.cpp:37`, `booleanSwitchBehavior.cpp:20-25`,
  `enumBehavior.cpp:24-29`) — one `MenuSelect` per Enter either way.

`Gfx::DrawBasicMenu` — `gfx.cpp:1699`. `Gfx::DrawSpectatorInfo` (level name, "P1 vs P2" + colour
swatches, "PAUSED"/"SETUP") — `gfx.cpp:1739`. (**4½d:** `DrawSpectatorInfo` and
`MainMenuState::Enter`'s minimap touch only `single_screen_renderer` and the spectator viewport's
own `rand`, never `game.rand` or `play_renderer`, so the port does not need them; plan fact 21.)

---

## 2. Menu system (`src/game/menu/`)

### 2.1 `Menu` base — `menu/menu.hpp:26`, `menu/menu.cpp`

State: `items`, `item_height=8`, `value_offset_x`, `x,y`, `height=15` (rows), `top_item`/`bottom_item`
(visible indices), `visible_item_count`, `centered`, private `selection_` (global index) —
`menu.hpp:36-49, 177-196`.

Key methods:
- `Draw(common, renderer, disabled, x=-1, show_disabled_selection=false)` — `menu.cpp:81`. Iterates from `ItemFromVisibleIndex(top_item)`, skips invisible items, draws `height` rows, then a **scrollbar** (chars 22/23 as arrows, a `FillRect` tab) when `visible_item_count > height` (`menu.cpp:107-124`).
- Navigation: `Movement` wraps around visible+selectable items (`menu.cpp:263`); `MovementPage` moves ±height/2 (`:250`); `MoveTo` / `MoveToId` / `MoveToFirstVisible` (`:127-136`); `EnsureInView`/`SetTop`/`SetBottom`/`Scroll` (`:155-248`).
- `SetVisibility(id, state)` maintains `visible_item_count` and re-anchors the scroll (`menu.cpp:227`).
- `OnKeys(begin, end, contains=false)` — **type-to-search** with a 1500 ms prefix timeout, Tab = "next match", case-insensitive prefix or substring match (`menu.cpp:14-79`). Used by `WeaponMenuState` (`weaponMenuState.cpp:89`) and `FileSelector` (`fileSelector.hpp:297`, with `contains=true`).
- `OnLeftRight` / `OnEnter` / `UpdateItems` construct a fresh `ItemBehavior` per call via the virtual `GetItemBehavior` (`menu.hpp:69-97`).
- `Menu::Process()` is declared (`menu.hpp:53`) but never defined — dead declaration.

### 2.2 `MenuItem` — `menu/menuItem.hpp:10`, `menuItem.cpp:6`

Fields: `color`, `dis_colour`, `string`, `has_value`+`value`, `visible`, `selectable`, `id`.
`MenuItem::Space()` = unselectable blank (`menuItem.hpp:18`).

Rendering (`menuItem.cpp:6-42`) — exact recipe (the pixel-fidelity contract for the Rust port):
1. width = `font.GetDims(string)`; if `centered`, `x -= wid>>1`.
2. **selected** → `DrawRoundedBox(bmp, x, y, 0, 7, wid)` (and a second box for the value at `x + value_offset_x - valueWid/2`).
3. **not selected** → draw the text at `(x+3, y+2)` in colour **0** (the drop shadow).
4. colour = `dis_colour` if disabled, `168` if selected, else `color`.
5. Draw text at `(x+2, y+1)` in that colour; same for the value at `x + value_offset_x - valueWid/2 + 2`.

### 2.3 `ItemBehavior` hierarchy

| Behavior | File | Semantics |
|---|---|---|
| `ItemBehavior` (base, no-op) | `menu/itemBehavior.hpp:8` | `OnLeftRight→true`, `OnEnter→-1`, `OnUpdate` nothing |
| `IntegerBehavior` | `menu/integerBehavior.hpp:8`, `.cpp` | `v` clamped `[min,max]`, step, `scroll_interval` (default 5 — gated on `gfx.menu_cycles % scroll_interval`, `integerBehavior.cpp:15`), `display_div` (shown = `v/display_div`), `percentage` suffix, `allow_entry` → Enter opens an `InputStringState` digit field (`integerBehavior.cpp:36-80`) |
| `TimeBehavior : IntegerBehavior` | `menu/timeBehavior.hpp:8` | value rendered as `TimeToString`/`TimeToStringFrames`; `allow_entry=false` |
| `BooleanSwitchBehavior` | `menu/booleanSwitchBehavior.hpp:10` | toggle via optional `std::function<void(bool)>` setter; value = `common.texts.onoff[v]`; **returns false from OnLeftRight** (no key repeat) |
| `EnumBehavior` | `menu/enumBehavior.hpp:10` | `uint32_t` cycled modulo `[min,max]`, `broken_left_right` flag; `Change()` calls `menu.UpdateItems` (`enumBehavior.cpp:31-39`) |
| `ArrayEnumBehavior : EnumBehavior` | `menu/arrayEnumBehavior.hpp:11` | value = `arr[v]` from a `std::string[N]` |

Gfx-local behaviors (defined in `gfx.cpp`): `KeyBehavior` (`:46`), `WormNameBehavior` (`:73`),
`InputDeviceBehavior` (`:85` — enumerates gamepads), `ProfileSaveBehavior` (`:203`),
`ProfileLoadedBehavior` (`:234`), `WeaponEnumBehavior` (`:253`), `ProfileLoadBehavior` (`:1215`),
`LevelSelectBehavior` (`:1222` — also rewrites the REGENERATE/RELOAD LEVEL label),
`WeaponOptionsBehavior` (`:1239`), `OptionsSaveBehavior` (`:1245`), `OptionsSelectBehavior` (`:1256`).

### 2.4 The four persistent menus (built once in `Gfx::LoadMenus`, `gfx.cpp:440-526`)

Placement (`Gfx::Gfx`, `gfx.cpp:264-271`): `main_menu(53,20)`, `settings_menu(178,20)`,
`player_menu(178,20)`, `hidden_menu(178,20)`. `value_offset_x`: settings 100, player 95, hidden 120 (`gfx.cpp:523-525`).

**MainMenu** (`menu/mainMenu.hpp:8`; ids `kMaResumeGame..kMaJoinOnline`). Items in order (`gfx.cpp:505-521`):
`RESUME GAME (F1)` (string set at runtime), `NEW GAME (F1)`, `HOST LAN GAME`, `JOIN LAN GAME`,
`HOST ONLINE`, `JOIN ONLINE`, `OPTIONS (F2)`, `REPLAYS (F3)`, `TC (<name>)`, `QUIT TO OS`, *spacer*,
`LEFT PLAYER (F5)`, `RIGHT PLAYER (F6)`, `NETWORK PLAYER (F9)`, `MATCH SETUP (F7)`.
`MainMenu::GetItemBehavior` is a pass-through — everything is intercepted in `MainMenuState` (`menu/mainMenu.cpp:6`).

**SettingsMenu** (`gfx.hpp:72`, items `gfx.cpp:485-503`, behaviors `gfx.cpp:1262-1312`):

| Item | Behavior / range |
|---|---|
| `GAME MODE` | `ArrayEnumBehavior(game_mode, texts.game_modes)` |
| `TIME TO LOSE` / `TIME TO WIN` | `TimeBehavior(time_to_lose, 60..3600, step 10)` — **both ids bind the same field** (`gfx.cpp:1284-1286`) |
| `ZONE TIMEOUT` | `TimeBehavior(zone_timeout, 10..3600, 10)` |
| `FLAGS TO WIN` | `IntegerBehavior(flags_to_win, 1..999)` |
| `LIVES` | `IntegerBehavior(lives, 1..999)` |
| `LEVEL` | `LevelSelectBehavior` → pushes `LevelSelectorState` (`mainMenuState.cpp:275-278`) |
| `MAP WIDTH` / `MAP HEIGHT` | `IntegerBehavior(random_map_width/height, 64..4096, step 8)` |
| `LOADING TIMES` | `IntegerBehavior(loading_time, 0..9999, %)` |
| `WEAPON OPTIONS` | pushes `WeaponMenuState` (`mainMenuState.cpp:280-283`) |
| `MAX BONUSES` | `IntegerBehavior(max_bonuses, 0..99)` |
| `NAMES ON BONUSES`, `MAP`, `LOAD+CHANGE`, `REGENERATE LEVEL` | `BooleanSwitchBehavior` |
| `AMOUNT OF BLOOD` | `IntegerBehavior(blood, 0..LC(BloodLimit), step LC(BloodStepUp), %)`, `allow_entry=false` |
| `SAVE SETUP AS...` | `MakeSaveAsState("Setups", ".cfg", ...)` (`mainMenuState.cpp:290-311`) |
| `LOAD SETUP` | pushes `OptionsSelectorState` (`mainMenuState.cpp:285-288`) |

`SettingsMenu::OnUpdate` (`gfx.cpp:1314-1341`) does per-game-mode item visibility: LIVES for
KillEmAll/ScalesOfJustice, TIME TO LOSE for GameOfTag, TIME TO WIN + ZONE TIMEOUT for Holdazone;
MAP WIDTH/HEIGHT only when `random_level`.

**PlayerMenu** (`gfx.hpp:38`, items `gfx.cpp:459-483`, behaviors `gfx.cpp:1362-1428`):
`PROFILE LOADED`, `SAVE PROFILE`, `SAVE PROFILE AS...`, `LOAD PROFILE`, `NAME`, `HEALTH`, `Red`,
`Green`, `Blue`, `INPUT`, `AIM UP`, `AIM DOWN`, `MOVE LEFT`, `MOVE RIGHT`, `FIRE`, `CHANGE`, `JUMP`,
`DIG`, `WEAPON 1..5`, `CONTROLLER`.
- `HEALTH`: `IntegerBehavior(1..10000, %)`, `scroll_interval=4`.
- `Red/Green/Blue`: classic mode → `0..252 step 4`, `display_div=4`, `scroll_interval=4` (reproduces the VGA 0..63 picker); modern → `0..255 step 1` (`gfx.cpp:1376-1388`).
- `PlayerMenu::DrawItemOverlay` (`gfx.cpp:1343-1360`) draws the colour bar: `DrawRoundedBox(x+24, y, selected?168:0, 7, rgb>>2 - 1)` + `FillRect(x+25,y+1, barWidth, 5, ws->color)`.
- `INPUT` → `InputDeviceBehavior`; `AIM UP..JUMP` and `DIG` → `KeyBehavior`; `CONTROLLER` → `ArrayEnumBehavior(ws->controller, texts.controllers)` ("Human" / "CPU" / "AI" — hard-coded in `Texts::Texts()`, `common.cpp:215-217`; `tc.cfg` has no controller texts).
- `NAME` → `InputStringState` (20 chars); empty name → `Settings::GenerateName` (`mainMenuState.cpp:323-347`).
- `WEAPON n` → `InputStringState` (10 chars) then **Levenshtein fuzzy-match** against weapon names, normalised by name length (`mainMenuState.cpp:390-423`, `Levenshtein` at `:28`).
- Key items → `WaitForKeyState`; gamepad presses write `gamepad_controls[i]`, keyboard writes `controls[i]` (only if not extended) and `controls_ex[i]` (`mainMenuState.cpp:367-389`).

**HiddenMenu** ("OPTIONS", `menu/hiddenMenu.hpp:8`, items `gfx.cpp:441-457`, behaviors `menu/hiddenMenu.cpp:12-61`):
`FULLSCREEN (F11)`, `MODERN COLORS (F10)`, `DOUBLE SIZE`, `POWERLEVEL PALETTES`, `SHADOWS`,
`AUTO-RECORD REPLAYS`, `AI FRAMES` (1..350), `AI MUTATIONS` (1..20), `AI PARALLELS` (1..16),
`AI TRACES`, `PALETTE` (0..255, with a colour swatch overlay at `hiddenMenu.cpp:65-75`),
`BOT WEAPONS` (RANDOM/PICK/KEEP, `hiddenMenu.cpp:10`), `SEE SPAWN POINT`, `SINGLE SCREEN REPLAY`,
`SPECTATOR WINDOW`, `MAX SPECTATOR RES (H)` (0..4320 step 120).

**WeaponMenu** (weapon *availability*) — local class in `weaponMenuState.cpp:14`, placed at
`(179,28)`, `height 14`, `value_offset_x 89`, one item per weapon in `common.weap_order` order,
each an `ArrayEnumBehavior(weap_table[idx], texts.weap_states)`.

### 2.5 Menu palette animation

`Gfx::UpdateMenuPalettes` (`gfx.cpp:978-1006`), called once per frame before drawing when the top state wants menu flip:
- fade-in `fade_value++` up to 32 unless quitting;
- `++menu_cycles`; `pal = Origpal(); pal.RotateFrom(Origpal(), 168, 174, menu_cycles)` (the classic "water" rotation on indices 168..174);
- `SetWormColours(*settings, mode)`; special-case: when editing the *network* player, slot 0 shows that player's colour;
- `UpdatePal32()` on both renderers.

Frame pacing: `Gfx::Flip` (`gfx.cpp:1152`) presents and busy-sleeps to a fixed **14 ms** delay
(`:1176-1189`) — ~71.4 fps, the Liero tick rate.

---

## 3. Weapon selection phase

Two different things share the word "weapon menu":

**(a) `WeaponMenuState`** (`weaponMenuState.cpp`) — the *settings* screen for which weapons are
enabled (`settings->weap_table[40]`, 0 = available). Pure UI; Esc/Jump refuses to close if zero
weapons are enabled and pushes `InfoBoxState(LS(NoWeaps))` (`:91-109`). Drawn over `frozen_screen`
with `WEAPON` / `AVAILABILITY` headers (`:114-127`).
(**4½e-1**, plan facts 10, 11.) The close rule, exactly (`:91-110`): on Esc, any keyboard player's Jump or pad
East/South, count the `weap_table` entries equal to 0 over all 40; > 0 → `Update` returns false and the state pops
that frame; 0 → push `InfoBoxState(LS(NoWeaps), 223, 68, false)` on top (no replace). Enter and Fire do nothing;
Left/Right are *once* keys (`:66-76`); PgUp/PgDn are live (`Settings::kExtensions`); the headers are exactly
`Font::DrawFramedText` `(179, 20, "Weapon", 50)` and `(249, 20, "Availability", 50)`.

**(b) `WeaponSelection`** (`src/game/weapsel.cpp`, `weapsel.hpp:9`) — the in-match pick screen.
**This is inside the controller, not the state stack.**

Flow: `LocalController::Focus()` → `ChangeState(kStateWeaponSelection)` (`localController.cpp:108-110`)
→ constructs `WeaponSelection(game)` (`localController.cpp:227`). `LocalController::Process` runs
key-repeat emulation for held keys (`kKeyRepeatInitial=12`, `kKeyRepeatInterval=3`,
`localController.hpp:44-46`; `localController.cpp:121-146`) then `ws->ProcessFrame()`; when it
returns true → `ChangeState(kStateGame)`, which calls `ws->Finalize()`, resets lives, starts replay
recording, `game.StartGame()` (`localController.cpp:213-286`).
(**4½e-1**, plan fact 15.) A running `WeaponSelection` reads the live settings too: `game.settings->weap_table` on
every cycle and RANDOMIZE step (`weapsel.cpp:255`, `:278`, `:327`) and `level_file` in `Draw` (`:107`, `:171`) —
but it counts `enabled_weaps` once, in its constructor (`:35-39`), and never again. Its picks are the shared
`WormSettings::weapons` (`:66`, `:255-282`), so the settings menu sees a cycled pick at once.

### ⚠ Sim-affecting RNG in weapon selection

**`WeaponSelection` draws from `game.rand`, the simulation RNG:**
- Constructor: `ws.weapons[j] = game.rand(1, 41)` for any unset weapon or when a bot is set to RANDOM (`weapsel.cpp:57-61`), plus a rejection loop that redraws until an *enabled* and (if enough are enabled) unused weapon comes up (`weapsel.cpp:66-75`). (**Corrected by the 4½c design, finding 1:** the constructor's loop runs only for a DISABLED pick and checks uniqueness only inside it; the "until enabled and unused" wording describes RANDOMIZE, weapsel.cpp:316-337.)
- The `Randomize` menu item re-rolls all 5 in the same rejection loop (`weapsel.cpp:316-337`).
- `enabled_weaps` counts `weap_table[i] == 0` (`weapsel.cpp:35-39`).
- Bots auto-ready: `is_ready[i] = (ws.controller != 0 && select_bot_weapons != 1)` (`weapsel.cpp:95`).

**Therefore the number of RNG draws consumed before frame 0 depends on the players' saved weapon
picks, `weap_table`, and `select_bot_weapons` — this must be ported bit-exact.** Confirmed by the
rollback design: `WeaponSelectSnap` snapshots `game.rand` explicitly
(`src/game/serialization/weapsel_snapshot.hpp:37`), and `test_rollback_weapsel.cpp` exercises it.

Left/right cycling walks `ws.weapons[]` skipping disabled weapons (`weapsel.cpp:245-283`) — no RNG.
Menu layout: one `Menu` per viewport, placed at `vp.rect.CenterX()-31, CenterY()-51`, items =
`Randomize`, 5 weapon names, `Done` (`weapsel.cpp:41-93`); "Done" is hardcoded as index 6 (`weapsel.cpp:338`).

Rendering: `DrawNormalViewports` caches the game frame + level-name line into `gfx.frozen_screen`
once (`weapsel.cpp:160-209`); `DrawSpectatorViewports` caches level name, "P1 vs P2", colour
blocks, "WEAPON SELECTION", and the 252×175 minimap into `frozen_spectator_screen`
(`weapsel.cpp:99-158`). Own palette rotation via `UpdateWeapselPalette` (`weapsel.cpp:20-24`).

`Finalize()` → `worm.InitWeapons(game)` for every worm + `game.ReleaseControls()`
(`weapsel.cpp:352-361`). Note the TODO at `:360`: picks are **not** written back to settings on disk.

---

## 4. Level selection + generation

### 4.1 `Level::GenerateFromSettings` — `src/game/level.cpp:397`

```
if settings.random_level        → GenerateRandom(common, settings, rand)
else                            → append ".LEV" if no '.', FsNode(path).ToReader(), load()
                                   any failure → fall back to GenerateRandom
record old_random_level / old_level_file / old_random_map_width / old_random_map_height
if settings.shadow              → MakeShadow(common)
```

### 4.2 `GenerateRandom` — `level.cpp:101`

1. `origpal.ResetPalette(common.exepal, settings)`, `has_custom_palette = false`.
2. `Resize(random_map_width, random_map_height)` (`level.cpp:218`).
3. `GenerateDirtPattern` (`level.cpp:11`):
   - seed corner `rand(7)+12`; first column and first row are running averages of `rand(7)+12` with the previous pixel; interior `(left + up + rand(8)+12)/3` — a diffusion noise field.
   - `rand(100)` "large sprite" splats: sprite `rand(4)+69` blitted at `(rand(width)-8, rand(height)-8)`, blending when the destination pixel is in 177..179 (`level.cpp:30-71`).
   - `rand(15)` stones: `BlitStone(large_sprites[rand(4)+56])` (`level.cpp:73-82`).
4. `rand(50)+5` **dirt-effect worm tunnels**: random start, direction `(rand(11)-5, rand(5)-2)`, `rand(12)` segments × `rand(5)` steps each calling `DrawDirtEffect(..., effect 1, ...)`, then backtracking and jittering by `rand(7)-3, rand(15)-7` (`level.cpp:108-135`).
5. `rand(15)+5` 32×32 rock formations from `stone_tab[3][4]` (`src/game/common.cpp:23`), placed with a rejection loop `IsNoRock(...,32,...)` (`level.cpp:85-99`, `:142-170`). **Retry cap `kMaxTries = width*height`** (`level.cpp:140,155-158`).
6. `rand(25)+5` 16×16 rocks `large_sprites[rand(6)+3]`, same rejection scheme with size 15 (`level.cpp:172-192`).

`DrawDirtEffect` itself draws `rand(tex.r_frame)` — one more RNG draw per call
(`src/game/gfx/blit.cpp:534-537`). `BlitStone` is deterministic (`blit.cpp:462`).

`MakeShadow` (`level.cpp:195`) is pure/deterministic: +4 to any `SeeShadow` pixel whose (x+3, y−3)
neighbour is `DirtRock`; −2 (floored at 12) for pixels 12..18 shadowed by rock; bottom row background → 13.

**Determinism:** given the same `Rand` state, `Common` (sprites/materials/textures) and `Settings`
(`random_map_width/height`, `shadow`), generation is fully deterministic and integer-only.
**But which RNG feeds it differs by path:**

| Path | RNG |
|---|---|
| `Gfx::InitFrameStepping` (`gfx.cpp:1446`) and NEW GAME (`gfx.cpp:1520`) | `gfx.rand` — the **presentation** RNG, seeded from wall-clock time (`gameEntry.cpp:23`) |
| `RollbackController::Focus` (`controller/rollbackController.cpp:381`) | `game.rand` — **sim** RNG |
| `NetSession` (`net/session.cpp:696`) | `game.rand` |
| Tests / harness (`src/tests/game_harness.hpp:58` etc.) | `game.rand` |

So in single-player the *level content* is sim-affecting input but the *generation draws* do not
perturb the sim stream; in netplay they do. The oracle dumpers deliberately avoid
`GenerateFromSettings` for exactly this reason (`src/tools/oracle_dump/sim_dump.cpp:12`,
`sim_physics_dump.cpp:33-35`, `lrp_gen.cpp:233`).

### 4.3 Level file loading — `Level::load`, `level.cpp:229`

Already ported bit-exact in Step 1 (`assets::level::load`). Format probe order: `OLLEVEL2` sized
header → legacy 504×350 → optional `POWERLEVEL` palette (gated on `load_powerlevel_palette`) →
optional `MODERNLV` display data → optional animation ramps → `materials[i] = common.materials[material_id[i]]`.

### 4.4 The level file picker

`LevelSelectorState` — `fileSelectorState.cpp:71-156`:
- `FileSelector::Fill(gfx->GetConfigNode(), filter = ext == "LEV")` — recursive lazy directory tree (`menu/fileSelector.hpp:132-154`, `DirectoryListing` via `FsNode::Iter()`).
- Prepends a synthetic `RANDOM` node (`LS(Random)`, `id=1`) at the front (`:78-84`).
- `Select(settings->level_file)` restores the previous cursor by walking the tree (`fileSelector.hpp:184-214`).
- `OnSelected` sets `random_level` / `level_file` and `settings_menu.UpdateItems` (`:93-103`).
- (**4½e design findings 2, 3; confirmed by the 4½e-1 T0 probe** under Xvfb.) `level_file` is a **config-root
  path**: the tree's root is `gfx.GetConfigNode()`, whose `FullPath()` in the split layout is the *user* folder's
  (`FsNodeJoin::FullPath`, `filesystem.cpp:356-362`), and NEW GAME opens it with a plain `FsNode(path)`
  (`level.cpp:401-411`). The saved strings: `<user dir>/TC/openliero/Levels/water_stage.lev` for an absolute
  `OPENLIERO_TEST_USER_DIR`; `<pref path>/TC/openliero/Levels/water_stage.lev` with a **single** `/` for
  `SDL_GetPrefPath` (its trailing `/` is dropped: `FsNode(std::string)` re-joins the parts); `./user/TC/openliero/
  Levels/water_stage.lev` for the relative `OPENLIERO_TEST_USER_DIR=user` (a relative root gains `./`); and
  `<root>/TC/…` under `--config-root`. So a level that exists only in the shipped data **plays random** in a
  default split install (the file is not in the user folder and `GenerateFromSettings` falls back, `:416-418`),
  while LEVEL still shows its name; with the file in the user layer, or with `--config-root`, it plays. With a file
  level, SettingsMenu hides MAP WIDTH/HEIGHT and REGENERATE LEVEL reads RELOAD LEVEL. (John's Q4: Rust fixes this in
  4½e-2 — a picked level is played.)
- `DrawExtra` (`:105-156`) **live-previews** the highlighted level: loads it, draws a 52×36-target minimap into `frozen_screen` at (134,162) and a 252×175-target one into `frozen_spectator_screen`, clearing the previous footprint first. `Level::DrawMiniature` at `level.cpp:489`; the bounding-box constants are `Level::kHudMinimapW/H` and `kSpecMinimapW/H` (`level.hpp:31-34`).

`FileSelector::Process` key handling (`fileSelector.hpp:251-300`): Up/Down, PgUp/PgDn, Esc/Jump =
leave, Left = parent dir, Right = enter dir, plus substring type-search.

Other pickers: replays (`.LRP`, `<config>/Replays`, `fileSelectorState.cpp:160-184`), profiles
(`.TOML` → `<config>/Profiles`, `:188-206`), setups (`.CFG` → `<config>/Setups`, `:210-226`), TCs
(dirs under `TC/` that contain `tc.cfg`, `:230-261`).

---

## 5. Settings / profile persistence

### 5.1 Data model — `src/game/settings.hpp`

`Settings : GameplayExtensions, AppSettings` (`settings.hpp:50`).

- `GameplayExtensions` (`:11`, hashed & replayed): `record_replays=true`, `load_powerlevel_palette=true`, `ai_frames=140`, `ai_mutations=2`, `ai_traces=false`, `ai_parallels=3`, `zone_timeout=30`, `select_bot_weapons=1`, `allow_viewing_spawn_point=false`, `tc="openliero"`.
- `AppSettings` (`:31`, **not** hashed/replayed): `fullscreen=false`, `single_screen_replay=false`, `spectator_window=false`, `blood_particle_max=700`, `modern_colors=false`, `max_spectator_render_height=1080`.
- `Settings` proper (`:68-90`): `weap_table[40]` (zeroed = all available), `max_bonuses=4`, `blood=100`, `time_to_lose=600`, `flags_to_win=20`, `game_mode=0`, `shadow=true`, `load_change=true`, `names_on_bonuses=false`, `regenerate_level=false`, `lives=15`, `loading_time=100`, `random_level=true`, `level_file=""`, `map=true`, `screen_sync=true`, `bonus_timeout=0`, `input_delay=1`, `random_map_width=504`, `random_map_height=350`.
- `kSelectableWeapons=5`, `kZoneCaptureTime=70`, `kNumWormSettings=3` (0=left, 1=right, **2=network**), `kNetworkPlayerIdx=2`, `kConfigVersion=6`.
- Constructor defaults (`settings.cpp:23-60`): worm colours 32 / 41 / 32; default DOS scancodes `{0x13,0x21,0x20,0x22,0x1D,0x2A,0x38}` (P1) and `{0xA0,0xA8,0xA3,0xA5,0x75,0x90,0x36}` (P2); default RGB `{104,104,252}` / `{60,172,60}`.

`WormSettings : WormSettingsExtensions` — `src/game/worm.hpp:85` / `:44`: `health=100`, `controller`
(0 human / 1 DumbAI / 2 FollowAI), `controls[7]`, `controls_ex[8]` (adds DIG), `gamepad_controls[8]`
(encoding: 0..99 = SDL button, `100 + axis*2 (+1)` = axis pos/neg, `worm.hpp:74-77`), `input_device`,
`gamepad_name`/`gamepad_serial`, `weapons[5]`, `name`, `rgb[3]` (0..255), `random_name`, `color`,
`profile_node`. Default gamepad binds at `worm.cpp:21-31`.

### 5.2 File format = **TOML via a cereal archive adapter**

- `Settings::ToToml` / `FromToml` (`settings.cpp:103` / `:133`) use `cereal::TomlOutputArchive`/`TomlInputArchive` (`src/game/serialization/toml_archive.hpp`).
- Layout: a `[settings]` table (`version`, `modernColors`, then `SerializeSettingsScalars`, then a `weapTable` array), followed by `[player1]`, `[player2]`, `[network_player]` tables via `SerializeWormSettingsToml`.
- Field list: `src/game/serialization/cereal_types.hpp:161-194` (scalars), `:282-310` (worm TOML). Worm TOML carries an `rgbDepth` marker; files without it are treated as 6-bit and expanded `(v&63)<<2` on load (`cereal_types.hpp:290-302`).
- (**4½e design finding 1**, confirmed by the 4½e-1 T0 probe through the real `Gfx::RunOneFrame`.) A running
  `Game` holds the **same** `std::shared_ptr<Settings>` as `gfx.settings` (`game.hpp:124`,
  `localController.cpp:31-32`), and the settings menu edits it in place, so a paused match sees menu edits from its
  next tick — both sim reads (`max_bonuses`, `weap_table`, `game_mode`, `time_to_lose`, `blood`, `loading_time`,
  `load_change`, `shadow`) and draw reads (`map`, `names_on_bonuses`). `Gfx::LoadSettings` (LOAD SETUP) replaces
  `gfx.settings` with a new object (`gfx.cpp:1693-1697`), which **breaks the sharing**: the paused game keeps the
  old one. (Plan fact 14:) `lives` is *not* read per tick by a local match — `game.cpp:159` is `ResetWorms`, which
  only `RollbackController` calls; `LocalController` reads `settings->lives` once, at `kStateGame`
  (`localController.cpp:234`), as `StartGame` reads `blood_particle_max` (`game.cpp:513`).
- At exit `gameEntry.cpp:78` saves `gfx.settings` to `<user config>/Setups/liero.cfg`; at boot (`:52-58`) a failed
  `LoadSettings` of the merged view saves the defaults there (`FsNode` writes create the parent directories).
- `Settings::load/save` are byte-level wrappers (`settings.cpp:62-90`, `:158-163`). `Settings::UpdateHash` = XXH3-64 over the *gameplay-only* subset (`settings.cpp:92-101`, subset at `cereal_types.hpp:218-238`) — the match-compatibility hash for netplay/replays.
- `WormSettings::SaveProfile` / `LoadProfile` — `worm.cpp:60` / `:73`. **Load deliberately preserves `color`** (`worm.cpp:94`). `UpdateHash` = XXH3 over the profile TOML (`worm.cpp:38`).
- Binary (replay/net) path is a separate `serialize()` with indexed keys (`cereal_types.hpp:197-212`, `:255-280`), `CEREAL_CLASS_VERSION(Settings, 3)`.

### 5.3 Where files live

`paths::Resolve` (`src/game/filesystem.hpp:145-160`): either fully portable (`--config-root <p>` or
`portable.txt` next to the binary), or a **merged read view** of `SDL_GetPrefPath` layered over
read-only stock data, with **all writes going to the user dir**. `Gfx::SetConfigNodes`/`GetConfigNode`/`GetUserConfigNode` (`gfx.hpp:287-294`).

Subdirs: `Setups/*.cfg`, `Profiles/*.toml`, `Replays/*.lrp`, `TC/<name>/tc.cfg`, `Resources/`.
Shipped examples: `data/Setups/liero.cfg`, `data/Profiles/{AI,Lefty,Righty,Joystick}*.toml`.

`paths::ShadowsSystem` (`filesystem.hpp:142`) rejects Save-As names that would shadow shipped files;
`MakeSaveAsState` (`mainMenuState.cpp:69-91`) shows `NAME '<x>' IS RESERVED` and re-opens the input.

### 5.4 Controls binding UI

`PlayerMenu` items `kPlUp..kPlJump` + `kPlDig` → `WaitForKeyState` (`inputState.cpp:102-162`; drawn
as a "PRESS A KEY" box centred at 160,100). Result routed at `mainMenuState.cpp:373-389`. Key names
come from `Gfx::GetKeyName` (`gfx.cpp:828`) / `GetGamepadKeyName` (`gfx.cpp:842`) via
`common.texts.key_names[177]` (`common.hpp:60`). SDL↔DOS scancode tables in `src/game/keys.cpp:9-85`.

---

## 6. AI worms

There **are** two bots, selected per player by `WormSettings::controller`:

`CreateAi(controller, worm, settings)` — `src/game/controller/localController.cpp:19-27`:
- `0` → none (human)
- `1` → `DumbLieroAI` (`worm.hpp:130`)
- `2` → `FollowAI` (`src/game/ai/predictive_ai.hpp:320`), constructed with `Weights()`, `settings.ai_parallels`, and `worm.index == 0`

Wired at controller construction (`localController.cpp:38`, `:45`); ticked in
`LocalController::Process` *before* `game.ProcessFrame()`, alternating worm order by
`game.cycles % 2` (`localController.cpp:155-165`), with wall-clock timing fed to
`stats_recorder->AiProcessTime` (`:162-163`).

**`DumbLieroAI::Process`** — `src/game/worm.cpp:477-696` (~220 LOC). Faithful port of the original:
picks the nearest worm, computes a max engagement distance from the current weapon's
`time_to_explo`/`speed`/`gravity` (min 90), then probabilistically toggles Fire/Jump/Change using
`common.ai_params.k[state][control]` (from `tc.cfg`), and aims by scanning the 128-entry
`cossin_table` for the direction closest to the normalised delta (`worm.cpp:543-556`; note the
comment about the original's `0xC000` bug).

**`FollowAI`** — `predictive_ai.cpp` (898 LOC) + `.hpp` (365) + `dijkstra.hpp` (258) +
`work_queue.hpp` (130). A predictive planner: generates candidate input plans from a weighted model
(`Generate`, `predictive_ai.cpp:246`), mutates them (`:546-551`), evaluates by *simulating forward*
(clones the game) — governed by `ai_frames`, `ai_mutations`, `ai_parallels`, `ai_traces`.
**Deferred past Step 4½** (needs the Step 5a snapshot machinery).

### RNG / determinism notes

- **Both AIs use their own `Rand` member**, not `game.rand`: `DumbLieroAI::rand` (`worm.hpp:133`), `FollowAI::rand` (`predictive_ai.hpp:342`). They never disturb the sim RNG stream — but their *outputs are worm control states*, so they are fully sim-affecting via input. (How `DumbLieroAI::rand` is seeded must be pinned down in the 4½f design — the dumper needs a fixed seed.)
- `Worm::ai` is a `shared_ptr<WormAI>` explicitly **excluded from snapshots** ("transient, rebuilt on load", `cereal_types.hpp:329`); its RNG state is not serialised. Consequently AI is single-player only.
- Netplay force-disables it: `RollbackController::Focus` sets `w->settings->controller = 0` for all worms before weapon select (`controller/rollbackController.cpp:401`).
- `HiddenMenu::kSelectBotWeapons` (RANDOM/PICK/KEEP) controls the weapsel interaction: RANDOM(0) re-rolls bot weapons from `game.rand`, PICK(1) leaves the bot not ready, its menu driven by the keys bound to that worm (the AI never runs during selection; 4½c design finding 2), KEEP(2) auto-readies with saved picks (`weapsel.cpp:57`, `:95`).

---

## 7. Match end / stats / results

### 7.1 End conditions — `Game::IsGameOver` (`src/game/game.cpp:521`)

- `kGmKillEmAll` / `kGmScalesOfJustice`: any worm with `lives <= 0`
- `kGmGameOfTag`: any worm with `timer >= settings->time_to_lose`
- `kGmHoldazone`: same check against `time_to_lose` (note: **not** `time_to_win`, and the SettingsMenu label for Holdazone says "TIME TO WIN" while both ids bind `time_to_lose` — `gfx.cpp:1284-1286`)

Checked every tick in `LocalController::Process` (`localController.cpp:167-169`).

### 7.2 Transition

`ChangeState(kStateGameEnded)` (`localController.cpp:213`, `:275-280`) sets `fade_value = 180` and
`going_to_menu = true` — i.e. a ~180-frame post-mortem where the match keeps rendering. When the fade
reaches 0, `EndRecord()` (closes the `.lrp`) and `stats_recorder->Finish(game)` run and `Process()`
returns false (`localController.cpp:186-198`).

`GamePlayState::Update` (`gamePlayState.cpp:18`) then decides (`:53-93`):
1. pending error → `InfoBoxState`
2. `controller->StatsGame()->stats_recorder` is a `NormalStatsRecorder` with `game_time > 0` → `ScheduleReplaceTop(StatsState(stats, game, isMultiplayer))`
3. multiplayer END-MATCH during weapon select (no stats) → straight to `RematchState` (`:80-85`)
4. otherwise pop back to the main menu

### 7.3 `StatsState` — `src/game/statsState.cpp` (390 LOC)

Three horizontally-scrolling panes (combined + one per worm), vertically scrollable, with eased
interpolation `pane_ = pane_*0.89 + destPane_*0.11` (`:292-293`).
- `Enter()` precomputes stretched/cumulative/normalised damage curves, the total-HP-difference curve, and sorts weapon stats by `actual_hp` (`:209-247`).
- Content (`:315-389`): game mode + elapsed time, AI processing ms, lives-left *or* timer, kills, damage dealt/received/self, shortest/longest life, loading efficiency %, per-weapon hits/damage tables, a total-health-difference graph, and presence/damage **heatmaps**.
- Keys: Up/Down scroll, Left/Right switch pane, Enter/Esc/Fire/Jump exit → pops (single-player) or replaces with `RematchState` (multiplayer) (`:262-290`).
- Draws on the raw EXE palette, no rotation (`:304-306`).

Underlying recorders: `src/game/stats_recorder.{cpp,hpp}` (347 LOC) and `stats.hpp`.
**Step 4½ scope decision (John 2026-09-10): compact stats — numbers + per-weapon table; the HP graph
and heatmaps are deferred.**

### 7.4 Replay saving — there is **no prompt**

Recording is fully automatic and starts at `kStateGame`, gated on `Settings::kExtensions &&
settings->record_replays` (the `AUTO-RECORD REPLAYS` hidden-menu toggle). Filename =
`"%Y-%m-%d %H.%M.%S"` + `" P1nm-P2nm"` (first 4 alnum chars of each name) + `.lrp`, written to
`<user>/Replays/` (`localController.cpp:232-266`). Playback is `ReplayController`
(`controller/replayController.cpp`), entered from `ReplaySelectorState::OnSelected`
(`fileSelectorState.cpp:175-184`).

---

## 8. Sim-affecting vs presentation split

### Must be ported bit-exact (and oracled)

| Item | Where | Note |
|---|---|---|
| `WeaponSelection` weapon rolls | `weapsel.cpp:57-75`, `:316-337` | draws `game.rand` — **count and order of draws are load-bearing** |
| `Level::GenerateRandom` + `GenerateDirtPattern` + `DrawDirtEffect` calls | `level.cpp:11-193`, `blit.cpp:534` | deterministic given `Rand`, `Common`, `random_map_width/height`; in netplay it consumes `game.rand` |
| `Level::MakeShadow` | `level.cpp:195` | pure; gated on `settings.shadow` |
| `Level::SelectSpawn` | `level.cpp:435` | reservoir-samples with `rand(i)`; called from `Game::SpawnZone` (`game.cpp:494`) |
| Every field in `SerializeGameplay` | `cereal_types.hpp:218-238` | the hashed gameplay subset: `weap_table`, `lives`, `loading_time`, `blood`, `max_bonuses`, `game_mode`, `time_to_lose`, `flags_to_win`, `shadow`, `load_change`, `names_on_bonuses`, `random_level`, `level_file`, `map`, `bonus_timeout`, `input_delay`, `zone_timeout`, `select_bot_weapons`, `ai_*`, `allow_viewing_spawn_point`, `tc` |
| `WormSettings.{health, controller, weapons[5], controls, controls_ex}` | `worm.hpp:85` | feed `Worm` init and key→control mapping |
| `Settings::UpdateHash` / `WormSettings::UpdateHash` | `settings.cpp:92`, `worm.cpp:38` | XXH3 over the TOML serialisation — byte-identical TOML output required if the hash is exchanged over the wire (Step 5) |
| `Game::IsGameOver`, `StartGame`, `ResetWorms`, `DoHealingDirect` lives logic | `game.cpp:511-566`, `:155-166` | match end + Scales-of-Justice extra-life rule |
| AI control-state production | `worm.cpp:477`, `predictive_ai.cpp` | own RNGs, but outputs are sim inputs; `common.ai_params` from tc.cfg |
| Key→control mapping and `Worm::ControlState` packing | `localController.cpp:57-83`, `worm.hpp:150-176` | including the DIG-implies-Left+Right rule (already ported in Step 4a) |
| Weapon-select key repeat (12 / 3 frames) | `localController.hpp:44-46`, `localController.cpp:121-146` | changes which frames register presses |

### Pure presentation (safe to modernise)

Everything in `menu/` rendering; `MenuItem::Draw` colours/boxes; `Gfx::UpdateMenuPalettes` rotation
and fade; `menu_cycles`-gated `IntegerBehavior::scroll_interval`; the 14 ms `Flip` pacing;
`StatsState` entirely; `DrawSpectatorInfo`; `DrawMiniature` previews; `frozen_screen` caching;
window/renderer/texture management; `AppSettings` fields (`fullscreen`, `modern_colors`,
`spectator_window`, `single_screen_replay`, `max_spectator_render_height`, `blood_particle_max` —
the last one *is* read by `Game::StartGame` at `game.cpp:512` to size the blood pool, so treat it as
borderline); `Levenshtein` weapon-name matching; the F8 weapon randomiser easter egg
(`mainMenuState.cpp:465-579` — uses `std::random_device`/`mt19937` and mutates `Common` in place;
explicitly non-deterministic, not for porting).

`gfx.rand` is documented as "PRNG for things that don't affect the game" (`gfx.hpp:296`) — **except**
that in single-player it generates the level (`gfx.cpp:1446`, `:1520`). That's the single most
important asymmetry to decide on in the port (decided in the overview: dedicated `Rand` seeded from the match seed).

---

## 9. Rough LOC per area

| Area | Files | LOC |
|---|---|---|
| Menu framework | `src/game/menu/*` (20 files) | **1 369** (menu.cpp 317, fileSelector.hpp 309, menu.hpp 197, rest ≤88) |
| App states | `*State.{cpp,hpp}` + `state.hpp` | **2 983** (mainMenuState 626+26, statsState 390+~30, fileSelectorState 261+79, onlineConnectState 287+57, rematchState 241+~35, inputState 217+75, netConnectState 211+~35, weaponMenuState 127+18, gamePlayState 106+~20, state.hpp 139) |
| `Gfx` shell (menus glue, windows, input, frame loop) | `gfx.cpp` + `gfx.hpp` | **2 198** (1 800 + 398) |
| Weapon selection | `weapsel.{cpp,hpp}` + `weapsel_snapshot.hpp` | **440** |
| Level gen + load + minimap | `level.{cpp,hpp}` | **743** (of which `GenerateRandom`+`GenerateDirtPattern`+`MakeShadow` ≈ 205, `load` ≈ 165) |
| Settings/profile persistence | `settings.{cpp,hpp}` + `toml_archive.hpp` | **774** (+ ~160 lines of field lists in `cereal_types.hpp`) |
| AI | `ai/*` + `DumbLieroAI::Process` (`worm.cpp:477-696`) | **1 655 + ~220 = ~1 875** |
| Stats + stats screen | `stats.hpp`, `stats_recorder.*`, `statsState.*` | **877** |
| Controllers (state machine per match) | `controller/*` | **2 381** |
| Filesystem / paths | `filesystem.{cpp,hpp}` | **1 009** |
| Netplay (Step 5) | `net/*` | **3 863** |

**"Everything between launch and quit, excluding netplay and stats internals"** ≈ **menu 1.4k +
states 3.0k + gfx 2.2k + weapsel 0.4k + level-gen 0.4k + settings 0.9k + AI 1.9k ≈ 10 k LOC**, of
which roughly **1.5 k is sim-affecting** (weapsel rolls, level generation, settings plumbing, AI).

---

## 10. What the existing oracle/test infrastructure already covers

**Oracle dumpers** — `src/tools/oracle_dump/`, gated by `OPENLIERO_BUILD_ORACLE_DUMP`
(`CMakeLists.txt:372-391`):

| Target | Covers |
|---|---|
| `oracle_dump_level` (`level_dump.cpp`, 223) | **`Level::load` only** — 7 synthetic inputs hashed FNV-1a. **No coverage of `GenerateRandom`, `GenerateDirtPattern`, or `MakeShadow`.** |
| `oracle_dump_palette`, `_sprite`, `_tc`, `_object`, `_wav` | asset/TC parsing |
| `oracle_dump_sim`, `_sim_physics`, `_lrp_gen` | simulation — all three **deliberately load a fixed level rather than call `GenerateFromSettings`** (`sim_dump.cpp:12,76`; `sim_physics_dump.cpp:33-35,409`; `lrp_gen.cpp:233`) |
| `oracle_dump/main.cpp` | fixed-point math, `Rand` |

**Nothing exposes menus, settings UI, weapon selection, or random level generation to the Rust
differential harness.**

**Catch2 tests** (`OPENLIERO_BUILD_TESTS=ON`) that touch this territory:
- `src/tests/test_random_map_size.cpp:87,110` — `GenerateFromSettings` honours `random_map_width/height` (dimension checks only, **no content hash**)
- `src/tests/test_settings.cpp` — full `Settings` TOML round-trip incl. all extension fields, weap_table and worm settings
- `src/tests/test_rollback_weapsel.cpp` — the only real coverage of the weapon-select phase: snapshot round-trip (weapons, cursor, scroll, `is_ready`, `game.rand`, edge state), zero-jitter parity
- `src/tests/test_paths.cpp` — `paths::Resolve` / `ShadowsSystem`
- `src/tests/test_determinism.cpp`, `test_full_game.cpp`, `game_harness.hpp:58` — call `GenerateFromSettings(game.rand)` then compare two games; determinism is asserted **relative to itself**, not against a golden

**Gap for Step 4½:** a new `oracle_dump_levelgen` that runs `GenerateRandom` for a matrix of
(seed × width × height × shadow) and emits FNV-1a hashes of `material_id` (and the post-`MakeShadow`
map), plus an `oracle_dump_weapsel` that dumps the `game.rand` state and the 5 picked weapon ids for
a matrix of (seed × weap_table × prior picks × `select_bot_weapons`). Neither exists today.
