# Step 4½, Slice 4½f — the player menu, profiles and DumbLieroAI: design

Status: **DESIGN — awaiting John's rulings (§13)** · 2026-09-27 · branch `claude/cpp-oracle-vcpkg-assets-chcwcm` (on `liero-rs-step-4-5` at `c2d58fe`; 4½a ✅, 4½b ✅, 4½c-0 ✅, 4½c ✅, 4½d ✅, 4½e-1 ✅, 4½e-2 ✅ landed)
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (the 4½f bullet, Hard gate 4, open Q3, §4½h Mobile; cited **overview**)
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` §2.4, §5.4, §6 (cited **cpp-map**) and
`2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (cited **rust-map**)
Precedents: `2026-09-26-liero-rs-step4.5-slice4.5e-settings-menu-design.md` (cited **4½e design**; its findings are **4½e F1…F16**,
its rulings §14) and its plans `plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md` / `…e2-plan.md` (cited **e-1 plan**, **e-2 plan**;
the shell gate formats are the e-1 plan's "Formats pinned")
Next artifact: `plans/2026-09-2x-liero-rs-step4.5-slice4.5f1-plan.md` (and `…4.5f2-plan.md` if Q1 splits the slice)

After 4½e every setting of the match can be changed from the Rust menu, but LEFT PLAYER (F5), RIGHT PLAYER (F6) and
NETWORK PLAYER (F9) are inert, both players are humans, and on a phone player 2 is a stand-in that only presses FIRE to
respawn. 4½f ports the player menu (every row of `PlayerMenu`: profiles, NAME, HEALTH, the colour picker with its bars,
INPUT, the eight key bindings, WEAPON 1–5 with the fuzzy name match, CONTROLLER), profile save/load, the per-player live
input the key bindings imply, and the original's CPU opponent, `DumbLieroAI`. The standing rule is John's: *it should
work like the original OpenLiero.* As in 4½c–4½e, every screen is gated bit-exact against the **real C++ frame loop**
run headlessly here, and every sim consequence (the AI's control stream, per-player health) against the C++ sim oracle.

---

## 0. Summary of findings (read this first)

Read out of the C++ source for this slice. Items 1, 2, 3, 4, 5, 6, 9 and 12 contradict or sharpen the overview, the maps,
PROGRESS or the Rust baseline.

1. **The AI seed question (overview Q3) is answered: it is the constant `0x1337`.** `DumbLieroAI` holds a
   default-constructed `Rand rand` (`worm.hpp:130-134`), and `Rand`'s engine is `std::mt19937 engine{0x1337U}` with
   `last = 0` (`rand.hpp:15-16`). Nothing ever reseeds it: the only `Seed` calls in the game are `game.rand` from the
   clock (`game.cpp:42`) and `gfx.rand` from the clock (`gameEntry.cpp:23`). `CreateAi` builds a fresh
   `new DumbLieroAI()` for controller 1 (`controller/localController.cpp:19-27`) in the `LocalController` constructor
   (`:38`, `:45`), i.e. **once per NEW GAME, per CPU player**. So every CPU player of every match starts from
   `mt19937(0x1337)`, and two CPU players in one match draw identical but separate streams. It is not serialised
   (`serialization/cereal_types.hpp:329`), and replays do not need it: `ReplayWriter::RecordFrame` runs after the AI
   and stores the resulting control words (`localController.cpp:161-168`). The dumper therefore needs **no seed
   override**, and Rust uses `sim_core::rng::Rand::new()`, which is already `0x1337` (`rust/sim-core/src/rng.rs:48-57`).
   The AI's output still varies from match to match, because the match seed, level and spawns vary.
2. **The AI reads a C++ value that is never initialised: `Worm::reacts`.** `reacts[4]` has no initialiser
   (`worm.hpp:260`), the `Worm()` constructor initialises only `ready`, `movable` and `killed_timer` (`:178-183`), and
   `std::make_shared<Worm>()` (`localController.cpp:33`, `:40`) does not zero a class with a user-provided constructor.
   `reacts` is written only while the worm is visible (`worm.cpp:218-280`). The AI reads it from the very first match
   tick (`worm.cpp:680-694`: `Pressed(kLeft) && reacts[kRfRight]` …), while the worm is still waiting to spawn, and it
   usually has Left or Right pressed then (it walks toward a far target, `:652-655`). A nonzero leftover presses Right or
   Jump, and `control_states` is in `HashGameState` (`stateHash.hpp:36`). This is C++ undefined behaviour that can move
   the gate. **Decision:** Rust starts `reacts` at 0 (the snapshot default, `serialization/fast_snapshot.hpp:48`), and
   both dumpers zero it right after the controller is built, where no real code has run yet (§6.2, §6.5). Plan T0 P2
   records what the real build holds there. Rust also has to *keep* `reacts` between ticks: today it is tick-local
   (`rust/sim/src/state.rs:2022-2025`), and the AI reads last tick's value.
3. **Binding DIG in C++ writes past the end of `controls` and overwrites WEAPON 1.** The key-binding callback writes
   `ws.controls[kEyIdx] = k` for any DOS key (`mainMenuState.cpp:381-383`), with `kEyIdx = kItemId - kPlUp`
   (`:371`). For DIG that is `13 - 6 = 7` (`gfx.hpp:48`, `:55`), but `controls` has 7 entries (`worm.hpp:106`,
   `kMaxControl = kDig = 7`, `:54`). The next member is `weapons[5]` (`:107`), so in practice WEAPON 1 becomes the DOS
   scancode: another weapon for keys 1..40, and for keys above 40 an out-of-bounds `weap_order` read in the WEAPON 1 row
   (`gfx.cpp:259`), in weapon selection (`weapsel.cpp:66`) and in `InitWeapons` (`worm.cpp:704`). **Open Q4** (recommended:
   fix it, writing only `controls_ex[7]`). The gate binds DIG only to a key whose code equals the current WEAPON 1 value,
   so both sides agree (§6.7). T0 P3 confirms where the write lands.
4. **C++ has no random player names.** `Settings::GenerateName` is compiled out (`settings.cpp:165-207`, `#if 0`).
   An empty NAME stays empty, and both shipped setups have `name = ''` with `randomName = true`
   (`data/Setups/liero.cfg:11`, `:26`, `:41`). The PROGRESS items "C++'s default `random_name` gives the players random
   names where Rust shows none (4½f)" and "Still absent: the C++ random player names (4½f)" do not hold for this source:
   whatever the 4½c Xvfb run showed must have come from that run's own `liero.cfg`. T0 P4 re-checks it on a fresh config
   root. Rust ports the no-op: `random_name` is stored and never used.
5. **The key names are hard-coded in C++, not read from the TC.** `Texts::key_names[177]` is a static table
   (`common.cpp:25-203`, UTF-8 strings such as `"Å"`, `"Left Crtl"` drawn through `cp437::UnicodeToByte`, `gfx/font.cpp:52`),
   and `Gfx::GetKeyName` (`gfx.cpp:828-840`) reads it. The overview's Q4 ("display `key_names` from the TC") is corrected:
   Rust ports the table into `ui::text`. The controller names are hard-coded too ("Human", "CPU", "AI",
   `common.cpp:214-216`), as 4½d already recorded.
6. **Health is per player in the sim, and read live.** Besides the start value (`localController.cpp:35`, `:42`;
   `Game::ResetWorms`, `game.cpp:158`), the sim reads `worm.settings->health` every tick: the clamp
   (`worm.cpp:213`), the health bonus (`:292-296`), the low-health blood (`:355`), Scales' extra lives (`:386`), the
   respawn (`:795`) and `DoHealingDirect` / `DoHealing` (`game.cpp:555-565`, `:607`). Rust carries one
   `settings_health` scalar (`rust/sim/src/state.rs:1221-1231`) and refuses unequal healths (`BuildError::AsymmetricHealth`,
   `rust/scenario/src/build.rs:122`; the boot copies player 1's, `rust/ui/src/shell/playing.rs:52`). 4½f makes it per
   worm, which lifts the 4½a interim ruling. Because the value is shared through `shared_ptr<WormSettings>`
   (`worm.hpp:255`), a HEALTH edit while paused reaches the match at RESUME, like 4½e F1.
7. **Everything the player menu edits is shared with a paused match.** The game's worms hold the same `WormSettings`
   objects the menu edits. Health is read by the sim (item 6); the key bindings are read by `FindControlForKey` on every
   key event (`game.cpp:75-106`); the name is drawn in the kill banners (`viewport.cpp:256-270`); the colour reaches the
   palette at RESUME through `LocalController::Focus` → `Game::Focus` → `UpdateSettings` (`localController.cpp:115-117`,
   `game.cpp:473-487`). CONTROLLER does **not** reach a paused match: the AI objects are made in the constructor only.
   LOAD SETUP still detaches (4½e F1): the paused game keeps the old `WormSettings`. LOAD PROFILE edits the current
   object in place (`WormSettings::LoadProfile`, `worm.cpp:73-95`), so it does reach an attached match.
8. **The live input follows the bindings, first match wins.** C++ turns a key event into one (worm, control) pair:
   `FindControlForKey` walks the worms in order, skips non-keyboard players, and returns the **first** control whose
   `controls_ex` equals the key (`game.cpp:87-106`). `OnKey` then sets that clean bit, copies it to the real bit if it is
   one of the seven, and applies the DIG rule (`localController.cpp:58-80`). Rust's live game still samples hard-coded
   Bevy `KeyCode`s (`rust/game/src/input.rs:85-116`), and the shell harness ORs every binding
   (`rust/oracle-tests/tests/shell_common/mod.rs:1151-1166`). With rebinding, both must follow `controls_ex` with
   first-match semantics (§4.5).
9. **DIG is a real control, and the key-binding menu makes it reachable.** When DIG is cleanly held, *every* event for
   that worm presses Left and Right; when it is not, every event releases a Left or Right that is not cleanly held
   (`localController.cpp:69-79`). 4½e ported only the `else` arm (`rust/ui/src/keys.rs`, `apply_key_edges`), because DIG
   was unbound. 4½f ports both arms with an 8-bit clean word (§4.5).
10. **The AI runs in the controller, before the tick, in alternating order.** `LocalController::Process` runs every
    worm's AI before `ProcessFrame`, in the order `(i + game.cycles % 2) % 2` (`localController.cpp:156-165`), only in
    `kStateGame` / `kStateGameEnded` — never in weapon selection, but through the Esc fade and the 180-frame post-mortem.
    Human key events have already been applied (they arrive in `Gfx::ProcessEvent` before `Process`), and a CPU player
    **still receives the key events bound to it** (`FindControlForKey` does not look at `controller`). The order does not
    matter for two worms: each AI changes only its own worm's `control_states` and reads the other worm's position,
    which the AI step never changes. Rust keeps the order anyway.
11. **What the AI reads.** The nearest other worm by `SqrVectorLength` of the integer positions (`worm.cpp:480-493`);
    the current weapon's engagement distance, `(time_to_explo - time_to_explo_v / 2) * speed / 130` when
    `0 < time_to_explo < 500`, else `speed - gravity / 10`, at least 90 (`:497-506`); `VectorLength` of the integer delta
    (`math.cpp:7-28`, integer square root; Rust `sim_core::math::vector_length` is already gated, `sqrt_golden.rs`); the
    fixed-point delta divided by that distance (`:538-542`); the first `cossin_table` entry from 1 to 127 within `0xC00`
    on both axes (`:544-552` — the loop never reaches 128, so the 4b/4½e `cossin[128]` hazard does not apply here); else a
    quadrant fallback with `rand(16)` (`:557-593`); `ninjarope.out`/`attached`, `direction`, `aiming_angle`, `visible`,
    and `reacts` (`:622-695`). Every `rand(k)` is drawn whatever its bound (`rand(0)` and `rand(1)` still draw), and
    the TC's jump "on" value is 1 (`data/TC/openliero/tc.cfg:115-118`), so a pressed jump always releases next tick. A
    dead CPU worm toggles Fire (`:513-518`, `!worm.visible`), and the dead arm's `PressedOnce(kFire)` readies it
    (`worm.cpp:435-436`): the CPU respawns by itself, roughly a second after death.
12. **The player menu has profile rows, INPUT and DIG that the overview omits.** `LoadMenus` (`gfx.cpp:459-483`) builds
    24 rows: PROFILE LOADED, SAVE PROFILE, SAVE PROFILE AS..., LOAD PROFILE (colour 3), NAME, HEALTH, `Red`, `Green`,
    `Blue` (mixed case), INPUT, AIM UP, AIM DOWN, MOVE LEFT, MOVE RIGHT, FIRE, CHANGE, JUMP, DIG, WEAPON 1–5,
    CONTROLLER. The menu sits at (178, 20) (`gfx.cpp:268`), 15 rows high (`menu/menu.hpp:43`), `value_offset_x` 95
    (`gfx.cpp:524`). PROFILE LOADED and SAVE PROFILE are visible only while a profile is loaded (`gfx.cpp:223-247`), so
    the menu has 22 or 24 visible rows and always scrolls. Every entry into it (Enter on LEFT/RIGHT/NETWORK PLAYER, F5,
    F6, F9) runs `Gfx::PlayerSettings`: point the menu at that player's `WormSettings`, `UpdateItems`,
    `MoveToFirstVisible` (`gfx.cpp:1430-1437`).
13. **The shipped profiles are mostly legacy, and two of them pick FollowAI.** `data/Profiles/*.toml` have no
    `rgbDepth` marker, so their 0..63 colours expand on load (`cereal_types.hpp:288-303`, already ported in 4½a-2).
    `AI (L).toml` and `AI (R).toml` set `controller = 2` — the predictive **FollowAI**, which the overview defers past
    Step 5a. `Joystick0/1.toml` set `inputDevice = 1`, which makes the keyboard ignore that player
    (`game.cpp:91-93`). Q2 and Q5 decide what Rust does with them.
14. **WEAPON n's fuzzy match divides doubles.** `Levenshtein` (`mainMenuState.cpp:28-52`, case-folded with `std::tolower`)
    divided by the weapon name's byte length, smallest wins, ties to the lowest index (`:407-417`). The quotients are
    ratios of small integers, so an integer cross-multiplication gives exactly the same order (§4.7), and Rust uses no
    float here. `std::tolower` on a typed byte ≥ 0x80 is C++ UB (a negative `char`); the gate types ASCII only.
15. **Several small C++ behaviours the port must keep.** NAME's callback sets `random_name = false` and plays
    `MenuSelect` even on Esc (`mainMenuState.cpp:333-345`). WEAPON n's callback plays no sound and does nothing on an
    empty or cancelled entry (`:401-421`). `WaitForKeyState` takes the **last** key-down of its frame, OS repeats
    included, and Esc cancels (so Esc cannot be bound); an unmapped key binds as DOS 89 (`inputState.cpp:107-150`,
    `keys.cpp:70-75`). `LoadProfile` preserves `color` (`worm.cpp:74`, `:94`) and sets `profile_node` before it parses,
    so a file that fails to parse still shows as PROFILE LOADED (`:78`). SAVE PROFILE always writes the **user** copy,
    `user/Profiles/<leaf>` (`gfx.cpp:210-216`). While the NETWORK PLAYER menu is open, palette slot 0 shows that player's
    colour (`gfx.cpp:990-993`, `:1000-1003`).
16. **The colour bars.** `PlayerMenu::DrawItemOverlay` (`gfx.cpp:1343-1360`) draws, for the Red/Green/Blue rows,
    `DrawRoundedBox(x + 24, y, selected ? 168 : 0, 7, (rgb >> 2) - 1)` and `FillRect(x + 25, y + 1, rgb >> 2, 5,
    ws->color)`. Colour 168 is the first index of the menu's water rotation, so the selected bar's frame cycles. A value
    below 4 gives a width of −1 for the box; the gate covers Red = 0.
17. **The classic colour picker only.** The R/G/B rows are `IntegerBehavior(rgb, 0, 252, 4)` with `display_div = 4`
    and `scroll_interval = 4` in classic mode, and `0..255` step 1 in modern mode (`gfx.cpp:1376-1389`). Rust has no
    modern colour mode (F10 is 4½g), so the classic picker is the only one, as the 4½e design's decision 12 already
    implies for `modernColors`.

---

## 1. Goal / done-when

**Goal.** Each player's name, health, colour, keys, saved weapons and controller can be changed from the Rust menu with
the same keys, screens, sounds and quirks as C++; profiles save and load; and the CPU opponent plays exactly as the C++
`DumbLieroAI` does, on desktop and as the phone's player 2. Every presented frame on those paths is bit-exact against the
real C++ frame loop, the AI's control stream is bit-exact against C++, and matches with a CPU player or unequal healths
are bit-exact against the C++ sim.

**Done when** (numbered by sub-slice, per the Q1 recommendation; merged if John prefers one slice):

**4½f-1: the CPU player (sim-affecting)**

1. **G-AI** (§6.3). Every `sim_slice4_5f_ai_*` golden (the per-tick AI control words and `rand.last`, plus the 12
   state columns) is bit-exact against `oracle_dump_sim_physics` with the new `ai` directive. It includes the
   human-vs-CPU match golden that overview Hard gate 4 asks for.
2. **G-HP** (§6.4). Unequal-health goldens (Kill'em All with health bonuses, Scales of Justice, Game of Tag) are
   bit-exact on all 12 columns.
3. **G2f-1** (§6.6). The CPU shell cases (a human against the CPU, and CPU against CPU to game over) match the real
   `Gfx::RunOneFrame` on every frame, including the `d` lines' per-frame state hash — the real `LocalController::Process`
   runs the real AI there, with no dumper step in between.
4. The live game plays the CPU wherever the settings say so (a `liero.cfg` with `controller = 1`, and `?cpu=1`). On a
   touch-only page player 2 is the CPU, per Q3, and the 4½c stand-in is gone.
5. A C++ `liero.cfg` with unequal healths boots and plays (the `AsymmetricHealth` refusal and the boot's sanitising
   copy are removed).

**4½f-2: the player menu, profiles and key bindings**

6. **G2f-2** (§6.7). Every player-menu case (navigation, each row's behaviour, NAME, WEAPON n, HEALTH and colour entry,
   key capture, profiles, the network player, live edits at RESUME, play with rebound keys and DIG) matches the real
   `Gfx::RunOneFrame` on every frame, with `d` lines, and every saved file matches byte for byte.
7. **🎯 MILESTONE f-2** (`shell_player_setup`, §6.8): F5 → name, health, colour, a rebind, WEAPON 1 by fuzzy name → F6 →
   CONTROLLER CPU → SAVE PROFILE AS… → NEW GAME → a human-vs-CPU match with the rebound keys → Esc → LOAD PROFILE for
   player 1 → RESUME → QUIT, every frame, state hash and saved file bit-exact.
8. The live game's keyboard follows the bindings in the settings (native and wasm); the phone's text field serves NAME
   and WEAPON n (the 4½e Q5 ruling); the browser lists the eight shipped profiles.

**Both**

9. Every prior golden is byte-identical (`git diff --name-status` under `golden/` shows only `A`, plus prior shell goldens
   that must regenerate byte-identically).
10. Green: `cargo test --workspace --exclude game` (debug), `cargo test -p game`, the wasm build, clang-format 22 and
    clang-tidy on the dumpers. `ui` stays Bevy-free; `sim::ai` is float-free, `HashMap`-free and clock-free.
11. PROGRESS, the overview (4½f bullet, Q3 answer, Q4 correction) and the maps carry the §0 corrections.

---

## 2. Inherited locked decisions, and the ones this slice amends

- **LD 1:** pixel-exact menus on the 320×200 CPU surface (`render`).
- **LD 3, amended again.** `tick_and_render` stays the only `ResMut<Sim>` holder, and menus get no `SimState`. The AI is
  not a menu: it runs inside `Match::process` (the `LocalController` analog, which already turns sampled words into the
  sim's inputs) and only **reads** `&SimState`; its output is the worm's input word. `apply_live_settings` (4½e) grows the
  per-worm health (§4.11).
- **LD 4:** the scenario format stays frozen. The AI is not a scenario feature: a recorded CPU match stores the AI's
  words, like any input, so replays need no AI. The sim oracle gains one more **oracle-only** directive, `ai`, of the same
  class as `settings`, `weapsel` and `generate`; `scenario::load` refuses it (§6.2).
- **LD 6:** the level seed equals the match seed; the AI seed is the constant `0x1337` (finding 1), not derived from it.
- **Determinism firewall.** `sim::ai` obeys the sim rules: fixed-point only, no floats, no `HashMap`, no clock, its own
  `Rand`. The AI object is **not** part of `SimState` and not hashed, exactly as C++ keeps `Worm::ai` out of snapshots
  (`cereal_types.hpp:329`); Step 5's rollback forces controller 0 as C++ does (`rollbackController.cpp:401`).
- **4½d/4½e §6 posture:** the C++ `Gfx` frame loop is the pixel oracle; interventions only where no real code runs. 4½f
  adds one (zeroing `reacts`, finding 2).
- **John's safe-edges ruling (4½e-1 Addendum G3):** Rust may diverge only where C++ is undefined behaviour. Findings 2,
  3 and 14 are such places.

---

## 3. The C++ surface, precisely

### 3.1 `PlayerMenu` rows and behaviours (`gfx.cpp:459-483`, `:1343-1428`, behaviour classes `:46-262`, `:1215-1220`)

| Row (id) | Behaviour | Left/Right | Enter |
|---|---|---|---|
| PROFILE LOADED (23), colour 3 | `ProfileLoadedBehavior` (`:234-251`): value = basename of `profile_node`'s leaf; visible iff a profile is loaded | base (keep) | base, -1 |
| SAVE PROFILE (20), colour 3 | `ProfileSaveBehavior(false)` (`:203-232`): visible iff a profile is loaded | base | `MenuSelect`; save to `user/Profiles/<leaf>`; `UpdateItems` |
| SAVE PROFILE AS... (21), colour 3 | `ProfileSaveBehavior(true)` | base | intercepted (§3.2) |
| LOAD PROFILE (22), colour 3 | `ProfileLoadBehavior` (`:1215-1220`, no overrides) | base | intercepted |
| NAME (0) | `WormNameBehavior` (`:73-83`): value = `name` | base | intercepted |
| HEALTH (1) | `IntegerBehavior(health, 1, 10000, 1, %)`, `scroll_interval` 4 (`:1370-1374`) | ±1 every 4 menu cycles | number entry, 5 digits |
| Red/Green/Blue (2–4) | `IntegerBehavior(rgb[c], 0, 252, 4)`, `display_div` 4, `scroll_interval` 4 (`:1376-1389`) + the bar (finding 16) | ±4 (clamped) every 4 cycles | number entry 0..63, 2 digits, stored ×4 |
| INPUT (5) | `InputDeviceBehavior` (`:85-201`): "Keyboard", else the gamepad's name or "Gamepad (none)" | sound + cycle, returns false | `MenuSelect` + cycle(+1) |
| AIM UP … JUMP (6–12) | `KeyBehavior` (`:46-71`): `GetKeyName(controls_ex[i])`, or `GetGamepadKeyName(gamepad_controls[i])` for a gamepad player | base | intercepted |
| DIG (13) | `KeyBehavior` on `controls_ex[7]` (`:1405-1408`) | base | intercepted |
| WEAPON 1–5 (14–18) | `WeaponEnumBehavior` (`:253-262`): `EnumBehavior(v, 1, 40)`, value = the weapon's name via `weap_order` | sound, wrap, returns false | intercepted |
| CONTROLLER (19) | `ArrayEnumBehavior(controller, {"Human","CPU","AI"})` (`:1410-1411`) | sound, wrap, returns false | `MenuSelect` + next |

`IntegerBehavior` (`menu/integerBehavior.cpp:14-88`), `EnumBehavior` (`menu/enumBehavior.cpp:9-44`) and
`ArrayEnumBehavior` (`menu/arrayEnumBehavior.hpp`) are ported (4½d/4½e); `display_div` and `scroll_interval` exist in
`rust/ui/src/menu/behavior.rs:64-89`. With no gamepads (the dumper has none, and Rust has no gamepad support),
`InputDeviceBehavior::Cycle` always lands on Keyboard and clears `gamepad_name`/`gamepad_serial` (`:178-197`).

### 3.2 `MainMenuState` with player focus (`mainMenuState.cpp:152-626`)

- **Entry.** Enter on LEFT/RIGHT PLAYER → `PlayerSettings(0|1)` (`:207-211`), on NETWORK PLAYER → `PlayerSettings(2)`
  (`:213-216`), after the main menu's `MenuSelect` (`:198`). F5/F6/F9 move the main cursor to the item and call
  `PlayerSettings` (`:447-463`), from any focus.
- **Esc / any keyboard player's Jump** → back to the main menu, cursor kept on the player item (`:171-179`). F1 → main
  menu on the start item (`:432-436`). Up/Down, PgUp/PgDn and held Left/Right act on the player menu (`:181-192`,
  `:581-602`).
- **Enter arms** (`:317-426`, after `MenuSelect` in each intercepted arm):
  - LOAD PROFILE → push `ProfileSelectorState(ws)` (`:320-322`).
  - NAME → push `InputStringState(ws.name, 20, x + 95 + 2, y, no filter)`; on close: accepted → `name = result`; empty
    → `GenerateName` (a no-op, finding 4); `random_name = false`; `MenuSelect`; `UpdateItems` (`:323-347`).
  - SAVE PROFILE AS... → `MakeSaveAsState("Profiles", ".toml", "", …)` (`:348-366`; the reserved-name box and its
    reopen are 4½e-2's, `:69-91`); a non-empty result → `SaveProfile(user/Profiles/<r>.toml)`; always `MenuSelect` +
    `UpdateItems`.
  - The eight key rows → push `WaitForKeyState(extended = true)` (`:367-389`); on a key other than Esc: a gamepad
    result writes `gamepad_controls[i]`; a keyboard result writes `controls[i]` (if not extended — always, for a DOS key,
    so DIG overflows, finding 3) and `controls_ex[i]`; `UpdateItems`.
  - WEAPON n → push `InputStringState("", 10, x + 97, y, no filter)`; on an accepted non-empty result, the fuzzy match
    (finding 14) sets `weapons[n]` and `UpdateItems`; no sound (`:390-423`).
  - Anything else → `selected_ = player_menu.OnEnter(common)` (`:424-426`): the behaviour plays its own sound.
- **Draw** (`:614-626`): `DrawBasicMenu`, then the player menu enabled when it has focus (it is never drawn disabled).
- **`UpdateMenuPalettes`** (`gfx.cpp:978-1005`): the network player's colour in slot 0 while its menu is open.

### 3.3 `WaitForKeyState` (`inputState.cpp:100-162`)

Not an overlay (`IsOverlay` is false, only `InputStringState` is one, `inputState.hpp:23`), so the stack draws only it,
over whatever the last frame left: the "PRESS A KEY" box centred at (160, 100) (`:152-162`). `HandleEvent` passes the
event to `ProcessEvent` first; every `SDL_EVENT_KEY_DOWN` (repeats too) sets `result_ = SDLToDOSKey(scancode)` and `done_`
(`:111-118`); the gamepad arms are unreachable without gamepads. `Update`: when done, `ClearKeys`, then the callback, then
pop (`:143-150`). The frame that pushed it (the Enter) presents the menu; the box appears on the next frame.

### 3.4 Profiles (`worm.cpp:60-95`, `fileSelectorState.cpp:186-206`)

- `ProfileSelectorState` = the 4½e-2 file selector with the title "Select profile:", filter `TOML` (case-insensitive),
  opened inside `<config>/Profiles` (`:191-199`). `OnSelected` → `LoadProfile(node)` + `player_menu.UpdateItems` (no
  `MoveToFirstVisible`) → pop.
- `LoadProfile` reads the keys at the file's root (4½a-2's `load_profile`), keeps `color`, sets `profile_node` first.
  `SaveProfile` sets `profile_node = node` and writes `ToToml()` (4½a-2's `worm_settings_to_toml`, byte-gated).
- `profile_node` is runtime state, not in any file. LOAD SETUP and the boot load make fresh `WormSettings` objects, so no
  profile is loaded after them.

### 3.5 Live input (`gfx.cpp:596-641`, `localController.cpp:58-80`, `game.cpp:75-116`)

A key-down (not a repeat, `gfx.cpp:608`) or key-up goes to `LocalController::OnKey(dos, state)`:
`FindControlForKey` → `(worm, control)` by first match over keyboard players and `controls_ex[0..8]`; set
`clean_control_states[control]`; for control < 7 also `control_states[control]`; then the DIG rule (finding 9) on that
worm. `ReleaseControls` clears all seven bits of every worm when weapon selection finalises (`weapsel.cpp:358`,
`game.cpp:110-118`), so a CPU worm also starts its first match tick from an empty word.

### 3.6 `DumbLieroAI::Process` (`worm.cpp:477-696`) and its driver

As findings 1, 2, 10 and 11. Per tick, per CPU worm, in `LocalController::Process` (`:153-176`): the AI, then (once for
all worms) `RecordFrame`, then `ProcessFrame`. `stats_recorder->AiProcessTime` times it with `steady_clock` (`:160-163`):
stats only, a no-op under the dumpers' base `StatsRecorder` (shell intervention 4), and absent in Rust until 4½g, which
will not port the timing.

### 3.7 Names in the draw

The weapon-selection name box (`weapsel.cpp:195-203`, already ported with `names` in `rust/render/src/weapsel.rs:140`,
fed empty strings by `Selection`, `rust/ui/src/shell/selection.rs:80`), the kill banners `KilledMsg + name` and
`name + CommittedSuicideMsg` (`viewport.cpp:256-270`; Rust draws the messages without the name,
`rust/render/src/frame.rs:130-150`), and the stats screen (4½g). Empty names draw exactly what Rust draws today, so no
prior golden moves.

---

## 4. Design

### 4.1 Crates and modules

- **`sim`**:
  - `ai` (new): `DumbLieroAi` (§4.2), `run_ais` (the `(i + cycles % 2)` order).
  - `state`: `WormState::reacts: [i32; 4]` kept between ticks, not hashed; `settings_health` becomes per worm
    (`WormState::max_health`, not hashed); `SimState::ai_params: [[i32; 7]; 2]` from the TC (not hashed).
- **`scenario`**:
  - `build`: per-worm max health; `AsymmetricHealth` removed; `apply_live_settings` gains both players' health; the
    Q2 refusal (`BuildError::FollowAiUnsupported { worm }` if Q2 = A).
  - `storage`: `placeable_leaf(subdir, leaf)` (today it hard-codes `Setups/`, `storage.rs:151-156`).
  - `assets`: the browser store gains the eight shipped profiles under `Profiles/` (about 3 KB).
- **`render`**: `Scene::names: [&str; 2]` for the kill banners (empty in `SceneData::as_scene`, so old goldens hold);
  the HUD lifebar divides by the worm's own `max_health` (`hud.rs:148`).
- **`ui`**:
  - `keys`: `clean_words` (first-match, 8-bit, §4.5); `KeyEdges` takes the 8-bit words and ports the DIG rule.
  - `text`: `KEY_NAMES` (`common.cpp:25-203`), `get_key_name`, `get_gamepad_key_name` (`gfx.cpp:828-859`),
    `CONTROLLERS`, `levenshtein`.
  - `shell::player_menu` (new): `player_menu()` (the 24 rows), `PlayerMenuModel` and its behaviours, the colour-bar
    overlay, `weapon_fuzzy_match`.
  - `shell::overlay`: `WaitForKeyState`; `InputPurpose` gains `WormName`, `WeaponFuzzy { slot }`, `SaveProfileAs`, and
    `IntegerEntry` gains a target (settings field or `(player, field)`).
  - `shell::files`: `ProfileSelectorState` (the setup selector generalised by title, filter and start folder).
  - `shell::main_menu`: `CurMenu::Player(usize)`, the §3.2 arms, F5/F6/F9.
  - `shell::playing::Match`: the AIs, names, and the RESUME focus palette (§4.4, §4.11).
  - `shell::mod`: `MenuWorld::profiles: [Option<ProfileRef>; 3]`; `update_menu_palettes` with the network-player case.
- **`game`**: the live keyboard through `clean_words` (§4.5, §7.3); the touch-only player 2 (§7.4); `?cpu=` (§7.5).
- **C++**: `oracle_dump_sim_physics` (`ai`), `oracle_dump_shell` (tops, `cur` letters, the `reacts` intervention, the
  FollowAI guard, the `profiles` manifest line). No game code changes.

### 4.2 `sim::ai::DumbLieroAi`

```rust
pub struct DumbLieroAi { pub rand: Rand }          // Rand::new() == mt19937(0x1337), last 0 (finding 1)

impl DumbLieroAi {
    /// `DumbLieroAI::Process` (worm.cpp:477-696): `cs` is the worm's control word as the tick will start it
    /// (last tick's post-sim word plus this tick's key edges); returns the word the tick applies.
    pub fn process(&mut self, state: &SimState, worm: usize, cs: ControlState) -> ControlState;
}

/// `LocalController::Process`'s AI loop (localController.cpp:156-165): for k in 0..n, worm (k + cycles % 2) % n.
pub fn run_ais(ais: &mut [Option<DumbLieroAi>; 2], state: &SimState, inputs: &mut [ControlState; 2]);
```

A line-for-line port of §3.6: `Press`/`Release`/`SetControlState`/`ToggleControlState` on the local word;
`sqr_vector_length` and the delta with `wrapping_*` `i32` arithmetic (C++ `int`); `Vec2 / i32` truncating toward zero
(`math/rect.hpp:41-45`); `ftoi` as `>> 16`; the scan over `state.cossin[1..128]`; `rand.bound(k)` for every draw,
bound `k = ai_params[pressed as usize][control]` (`k[1]` is the TC's `on`, `k[0]` its `off`,
`common_model.hpp:521-531`; `assets::tc::AiParams::ordered`). The weapon is `state.weapons[w.weapons[w.current_weapon].ty]`.
The only mutable state is `rand`.

### 4.3 Per-worm health and `reacts`

- `WormState::max_health` replaces `SimState::settings_health`; every call site (the clamp, the health bonus, Scales,
  `do_healing_direct`, respawn, the HUD lifebar) reads the worm's own. A task re-diffs every golden (they all use 100).
- `WormState::reacts` is written where `worm_reactions` runs (`state.rs:2025`), initialised to 0, and kept when the worm
  is not processed (C++ keeps it while dead). It is not hashed (C++ `HashGameState` does not hash it), so every golden
  holds; a task re-diffs anyway.

### 4.4 `Match` drives the AI

`Match::start` creates `ais[i] = (settings.worm_settings[i].controller == 1).then(DumbLieroAi::new)` — once per match,
like the constructor. In `Match::process`, on a match tick: `inputs = latch → edges` (unchanged), then
`run_ais(&mut self.ais, sim, &mut inputs)`, then `tick_viewports` (the tick). Never in selection. The AI runs on every
tick the match processes, including the Esc fade and the post-mortem. The selection's bot rules (`weapsel.cpp:57`,
`:95`) already read `controller`. Changing CONTROLLER while paused does not add or remove an AI (finding 7).

### 4.5 Live input from the settings (4½f-2)

```rust
/// C++ OnKey's clean state for each worm, from the held DOS keys (finding 8): for every held key,
/// `FindControlForKey` (first keyboard player, first control 0..8 whose controls_ex matches) sets one bit;
/// bit 7 is DIG. OS repeats are not events (gfx.cpp:608).
pub fn clean_words(held: &DosHeld, ws: &[WormSettings]) -> [u8; 2];
```

`KeyEdges::apply(now: [u8; 2], worms)` keeps the 4½e per-bit rule over the seven real bits and then, when any of the
eight bits changed for that worm (C++ applies the rule on each of its events, and the result depends only on the final
clean state, so one application per tick is exact for the per-tick words; a press and release inside one frame is the
known 4½d fact 19): DIG held → press Left and Right; else release Left or Right unless cleanly held. The shell harness drops its own OR sampler (`shell_common/mod.rs:1151-1166`) for `clean_words`; the live
game builds `DosHeld` from Bevy's held keys through the existing `dos_of_keycode`, replacing `default_bindings()` on the
shell path. The touch pad keeps OR-ing into player 1's word, and the touch DIG button keeps its Left+Right chord (a
Rust-only convenience, unchanged). `--live <scenario>` uses the default settings' bindings, which are today's.

### 4.6 Screens

```rust
pub enum Screen {
    /* 4½d/4½e … */
    WaitForKey(WaitForKeyState),          // top K (4½f-2)
    ProfileSelect(ProfileSelectorState),  // top F (4½f-2)
}
pub enum CurMenu { Main, Settings, Player(usize) }   // 0 left, 1 right, 2 network
```

`WaitForKeyState { purpose: KeyTarget { player, control }, result: Option<u32>, done }`: each `Key` event with `down`
(repeats too) sets `result`; `update` → `keys.clear()`, the continuation, pop. `draw` = the box (4½c's
`draw_rounded_box`, 4½e's `get_dims_h`).

### 4.7 The player-menu model

`PlayerMenuModel { ws: &mut WormSettings, profile: Option<&ProfileRef>, tc: &UiTc }` implements `MenuModel` with the
§3.1 behaviours as `CustomBehavior`s (the trait was left for these, `behavior.rs:43-60`) and `draw_item_overlay` for the
bars (`menu/mod.rs:84-95`). The main menu's Enter arms follow §3.2 in source order.

**Fuzzy match** (`weapon_fuzzy_match(names_in_weap_order, typed) -> Option<u32>`): ASCII case-folded Levenshtein (bytes);
best = the first `i` with `d_i * len_best < d_best * len_i` (the first candidate always wins). This is exactly C++'s
double comparison: equal ratios give equal doubles, and two different ratios of numbers this small differ by far more
than a double's rounding error. `levenshtein` treats bytes ≥ 0x80 as themselves (C++ is UB there; outside the gate).

**DIG** (per Q4 = A): the key continuation writes `controls[i]` only for `i < 7`, and `controls_ex[i]` always.

### 4.8 Overlays and continuations

| Purpose | Push | Close |
|---|---|---|
| `WormName { player }` | `InputString(name, 20, x + 97, y)` | accepted → name; `random_name = false`; `MenuSelect`; `update_items` |
| `WeaponFuzzy { player, slot }` | `InputString("", 10, x + 97, y)` | accepted & non-empty → fuzzy match; `update_items`; no sound |
| `SaveProfileAs { player }` | `InputString("", 30, x + 97, y)` | as `SaveSetupAs` with `Profiles` / `.toml`: reserved → the box and the reopen; else save, set the profile; always `MenuSelect` + `update_items` |
| `IntegerEntry { target: Player(p, Health \| Rgb(c)) }` | as 4½e | as 4½e, into the player's field |
| key capture | `WaitForKey` | not Esc → write the binding; `update_items` |

The typed name keeps C++'s CP437 bytes (`Utf8ToDos`, 4½e-1); non-ASCII names are outside the gate (4½e decision 11).

### 4.9 Profiles in the store

`ProfileRef { rel: String, label: String }`: `rel` is the config-relative path the store reads (`Profiles/AI (L).toml`),
`label` the C++ `FullPath` string whose leaf's basename PROFILE LOADED shows. `MenuWorld::profiles[p]` is set by LOAD
PROFILE (even when the parse fails, finding 15; the settings then stay unchanged, as 4½a-2's `load_profile` does) and SAVE
PROFILE AS…, and cleared by LOAD SETUP and the boot load. SAVE PROFILE writes `Profiles/<leaf>` into the user layer.
`shadows_system("Profiles", leaf)` refuses shipped names natively (the C++ rule); the browser refuses only `liero.cfg`, as
the C++ web build does (4½e-2 D9).

### 4.10 Names

`Selection::new` takes the players' names from the settings; the match draw passes `settings.worm_settings[i].name` as
`Scene::names`. Both read the match's copy of the settings, which RESUME refreshes while attached.

### 4.11 RESUME with player edits (finding 7)

While attached, RESUME already runs `apply_live_settings` + `Match::resync`. 4½f adds: both players' health into the sim
(the next tick's clamp applies it), the names (via the refreshed copy), and **`Game::Focus`'s palette**: `focus_palette`
re-runs on the match's scene with the attached settings' colours (C++ `LocalController::Focus`, `localController.cpp:115`).
Key bindings need nothing: live input reads the menu's settings on every tick, as `FindControlForKey` reads the shared
object. Detached (after LOAD SETUP), the match keeps its own copy: its old colours, health and names, as in C++. One C++
subtlety stays unported: a detached match's key bindings in C++ are the *old* object's, while Rust reads the menu's. It
only shows if a LOAD SETUP changes the bindings and then RESUMEs, and no case does that; it is recorded, not gated.

### 4.12 Refusals

`validate_for_selection` loses `AsymmetricHealth` and keeps `InvalidHealth` (health < 1 would divide by zero in C++'s
lifebar, `viewport.cpp:85`). With Q2 = A it gains `FollowAiUnsupported`: the box "AI PLAYERS ARE NOT\0SUPPORTED YET" at
NEW GAME, the menu unchanged. `bootable()` (`playing.rs:69-100`) keeps its other arms; a controller 2 boot game is built as
Human, since the boot game never runs.

---

## 5. What is live, and what stays a placeholder

| Surface | 4½f | Owner |
|---|---|---|
| CPU player (DumbLieroAI), unequal healths, `?cpu=` | **live** | 4½f-1 |
| Phone player 2 = CPU (per Q3); the stand-in removed | **live** | 4½f-1 |
| LEFT/RIGHT PLAYER (F5/F6), NETWORK PLAYER (F9) menus, every row | **live** | 4½f-2 |
| Profiles: LOAD / SAVE / SAVE AS…, PROFILE LOADED | **live** | 4½f-2 |
| Key bindings + DIG in live play | **live** | 4½f-2 |
| Names in weapon selection and kill banners | **live** | 4½f-2 |
| CONTROLLER = AI (FollowAI) | per Q2 (refused) | after Step 5a |
| INPUT = gamepad, gamepad key names | display only (per Q5) | gamepad follow-on |
| Modern colour picker (0..255) | not ported | 4½g (F10) |
| Network play with the network player's settings | — | Step 5 |
| Profiles kept across browser reloads | session only | 4½h |

---

## 6. Oracle

### 6.1 The AI seed, pinned

Finding 1. **No dumper seed directive exists or is needed**: C++ constructs every `DumbLieroAI` at `mt19937(0x1337)`, the
dumpers use the real class, and Rust uses `Rand::new()`. T0 P1 proves it at runtime: after `LocalController`
construction, `ai->rand == Rand()` for each CPU worm, and nothing between construction and the first `Process` touches it.
The streams still differ across cases through the match seed, `generate`'s level seed, the loadouts and the human's input.

### 6.2 `oracle_dump_sim_physics`: the `ai` directive (backwards compatible)

```
ai        # oracle-only; requires `settings`. The setup's player controllers decide: 1 → a real
          # std::make_shared<DumbLieroAI>() (as CreateAi, localController.cpp:19-22); 2 → refused.
          # At least one worm must be a CPU player.
```

- After the worms are built (and after `weapsel`, if present), **intervention (reacts)**: `std::fill(w->reacts, 0)` for
  both worms (finding 2).
- Each tick: `input` words are applied to the **human** worms only (an `input` line with a non-zero word for a CPU worm
  is refused); then each CPU worm's `ai->Process(game, worm)` in `(i + game.cycles % 2) % 2` order; then the unchanged
  tail.
- Every tick line gains four columns after the 12: `<aiw0> <ail0> <aiw1> <ail1>` — the CPU worm's `control_states.Pack()`
  right after the AI step (`%02x`) and its `rand.last` (`%08x`), or `-` for a human worm; `-` on the tick-0 line. Without
  `ai` the output is byte-identical (the established "absent ⇒ old behaviour" rule, `sim_physics_dump.cpp:42-74`).
- `weapsel` with a CPU player works unchanged (PICK leaves the bot's menu to its worm's `weapsel` words; RANDOM and KEEP
  ready it at once).

Rust harness `tests/sim_slice4_5f_ai_golden.rs`: build as 4½e's G3 (`settings` + `generate` + `build_match`, or 4½c's
selection harness when `weapsel` is present), then per tick `inputs[h] = scripted` for humans,
`inputs[c] = sim.worms[c].control_states` for CPU worms, `run_ais`, compare the four columns, `process_frame`, compare the
12. Both sides run the corpus under the checked build (`$S/build-chk`) as well.

### 6.3 G-AI corpus (`examples/gen_slice4_5f_ai.rs`, 4½a generator style: input seed, ledger, witnesses)

| Case | Setup | Witnesses (ledger) |
|---|---|---|
| `ai_idle` | P2 CPU, P1 human idle, default loadouts (KEEP), 504×350 generated | spawn (the CPU readies through Fire toggles), walking toward the target, the `kRealDist < max_dist` fire arm, kills |
| `ai_vs_human` | P1 scripted (walk, fire, rope, dig), P2 CPU, lives 3 | the human-vs-CPU match golden of Hard gate 4; deaths both ways; the CPU's respawn; game over |
| `ai_vs_ai` | both CPU, lives 2, RANDOM bot weapons | two identical-seeded streams diverging by position; the alternating order; game over |
| `ai_weapons` | CPU loadouts covering `time_to_explo` in (0, 500), ≥ 500 / 0, the floor 90, a fast laser | each `max_dist` arm |
| `ai_close` | 333×360 level, spawns near each other | `kRealDist` 0 and tiny (the non-unit delta → the `rand(16)` quadrant fallback, `dir = 12`), the rope arm (Up/Down toggles while attached), `reacts` presses |

About 3,000 ticks each. The ledger counts the fallback branches, rope-attached ticks, respawns and `reacts`-driven presses,
and the generator refuses a case whose ledger misses its witnesses. No map below 342 rows (the C++ spawn UB, 4½e-1 G3).

### 6.4 G-HP: unequal health (`oracle_dump_sim_physics` + `settings` + `generate`)

| Case | Settings | Witnesses |
|---|---|---|
| `hp_killemall` | P1 50, P2 300, MAX BONUSES 10 | the clamp, health bonuses scaled by each worm's max (`worm.cpp:292-296`), low-health blood (`:355`), respawn health |
| `hp_scales` | Scales of Justice, P1 30, P2 200 | `DoHealingDirect` extra lives per worm (`game.cpp:555-565`), the redistribution |
| `hp_tag` | Game of Tag, P1 1000, P2 10 | long survival vs instant death; `IsGameOver` |

These need no new directive: the dumper's `settings` path already starts each worm at its own `settings->health`
(`sim_physics_dump.cpp:584-585`).

### 6.5 `oracle_dump_shell` extensions (backwards compatible)

- **Tops:** `K` (`WaitForKeyState`), `F` (`ProfileSelectorState`).
- **`d` lines:** `<cur>` gains `L`, `R`, `N` (player menu on worm settings 0, 1, 2); `<ssel>` is then the player menu's
  `Selection()` (it stays the settings menu's for `M`/`S`, so every existing `d` line is unchanged). `cfg16` already
  covers every player field (`Settings::ToToml` writes all three `WormSettings`); `state8` covers the AI's words.
- **Intervention 3′ (reacts):** in intervention 3's slot (after a frame that made a new controller), zero both worms'
  `reacts` (finding 2).
- **FollowAI guard** (if Q2 = A): a case that reaches NEW GAME with a controller-2 player is refused by the dumper and the
  generator, like Holdazone (4½e-1).
- **Manifest line** `profiles <user|sys>`: copies `data/Profiles/*.toml` into that layer (the `file` line cannot name
  files with spaces).
- The key names used by scripts gain the letters and keys the binding cases press (e.g. `Q`, `K`, `KP_5`, and one key
  that `SDLToDOSKey` maps to DOS 89).

### 6.6 G2f-1 corpus (`examples/gen_slice4_5f1_shell.rs`, all `detail`)

| Case | Pins |
|---|---|
| `cpu_match` | `fs` `liero.cfg` with P2 `controller = 1`, BOT WEAPONS KEEP: NEW GAME → selection (P1 alone) → ~1,500 match frames with P1 keys scripted (move, fire, rope) → Esc → RESUME → a kill each way (the CPU's respawn) → QUIT |
| `cpu_vs_cpu` | both CPU, RANDOM, lives 2: selection ends at once → play to game over → the 180-frame post-mortem → back to the menu |
| `cpu_pick` | P2 CPU with PICK: the bot's weapon menu driven by P2's keys, then the match (the AI starts only after selection) |
| `hp_boot` | `fs` `liero.cfg` with healths 40 / 250: boots (no sanitising), NEW GAME, the lifebars, a death |

🎯 **MILESTONE f-1** is `cpu_match`: the real C++ `LocalController::Process` runs the real `DumbLieroAI` there, so every
frame's state hash equal to Rust's proves the AI end to end.

### 6.7 G2f-2 corpus (`examples/gen_slice4_5f2_shell.rs`, all `detail`)

| Case | Pins |
|---|---|
| `player_nav` | F5, F6, F9; Enter on the three main items; `MoveToFirstVisible` each time; scrolling 22 rows (scrollbar), PgUp/PgDn; Esc and P2's Jump back; F6 while in F5's menu; F9's slot-0 colour |
| `player_edit` | HEALTH held (cadence 4), typed 250, `0` → 1, `99999` → 10000; R/G/B held (step 4, the bar, frame colour 168), Red to 0 (the −1 box), typed Green 63 and 70 → 63; CONTROLLER Human → CPU → AI → Human; WEAPON 1 Left/Right wrap; INPUT Enter/Left/Right (stays Keyboard, sounds) |
| `player_name` | NAME typed, Backspace, Return; Esc (still `random_name = false`, still `MenuSelect`); empty Return; the 20-byte cap; the name in weapon selection and in a kill banner |
| `player_weapon` | WEAPON 2 typed `bazoka`, `LSR`, `a`, `zzzzzzzzzz`; a tie; empty Return and Esc keep the value, no sound |
| `key_bind` | AIM UP → the box → `Q`; Esc cancels; an OS repeat binds; two key-downs in one frame (the last wins); an unmapped key (DOS 89's name); P1 FIRE bound to P2's fire key (first match) → NEW GAME → both players' keys in play (state hash) |
| `dig` | WEAPON 1 set to 16, then DIG bound to `Q` (DOS 16: C++'s overflow writes the same value, finding 3) → NEW GAME → hold DIG (L+R), press other keys while holding, release (the full rule) |
| `profile_io` | `fs` + `profiles sys`: LOAD PROFILE opens in Profiles → `Lefty (L)` (legacy colours expanded, colour index kept, PROFILE LOADED `Lefty (L)`) → SAVE PROFILE (the user copy) → SAVE PROFILE AS… `AI (L)` (reserved box, reopen) → `mine` → Esc cancel → LOAD PROFILE `Joystick0` → INPUT shows the gamepad, Enter → Keyboard; `file` lines |
| `player_live` | play → Esc → F5: HEALTH 30, rename, Red 0, rebind FIRE → RESUME (the clamp, the palette, the banner name, the new key) → Esc → F7 LOAD SETUP `orbmit` → F5 edits → RESUME (detached: nothing reaches the match) |

### 6.8 🎯 The milestones

- **`shell_cpu_match` (f-1)** — §6.6, about 2,000 frames.
- **`shell_player_setup` (f-2)** — done-when 7 in one script, about 1,500 frames, `fs` with the exit save.

Both run in CI as ordinary `oracle-tests` tests in `shell_golden.rs`.

### 6.9 Standing gates

- Every task: `cargo test --workspace --exclude game` (debug) and `cargo test -p game`; no golden changes. The prior
  `shell_*` goldens regenerate byte-identically with the extended dumper; every sim golden regenerates byte-identically
  without `ai`.
- Own full re-diff tasks for: `reacts` persistence, per-worm health, `Scene::names`, and `clean_words` + the DIG rule
  (the 4½e `key_edges` case must hold).
- C++ changes confined to the two dumpers (and `CMakeLists.txt` if needed); clang-format 22 on whole files, clang-tidy
  on the diff; the AI corpus also under the checked build.

### 6.10 Eyeball artefacts (not gates)

Xvfb C++ | Rust PNGs of both milestone paths (`--config-root <fixture>`, xdotool), and a headless-Chromium walk on an
emulated phone playing against the CPU.

---

## 7. The live game

### 7.1 Native

The CPU plays whenever `liero.cfg` says so (4½f-1), and the player menu edits and saves it (4½f-2). SAVE PROFILE / SAVE
PROFILE AS… write into the user layer of the shared C++ config root (4½e Q3 ruling).

### 7.2 wasm

The browser store lists the eight shipped profiles; LOAD PROFILE works; saved profiles last for the session (4½h adds
localStorage). URL parameters never reach a saved file.

### 7.3 Keyboard

4½f-2 replaces the hard-coded bindings on the shell path with `clean_words` over the settings (§4.5), so a rebinding
works at once, and a gamepad player (a Joystick profile) ignores the keyboard (per Q5). `web/index.html`'s key help keeps
the default keys and gains one line: "Keys can be changed in LEFT/RIGHT PLAYER." `--live <scenario>` keeps the default
bindings.

### 7.4 Touch and the phone's player 2 (per Q3 = A)

- A touch-only page sets player 2's CONTROLLER to CPU in the in-memory settings at boot and forces BOT WEAPONS to RANDOM
  for player 2 at NEW GAME (the 4½c touch rule, which forced KEEP, changes its value). The RIGHT PLAYER menu shows CPU.
- `game::touch::BotRespawn` is deleted: the CPU readies itself (finding 11). `WeaponTap` and `DigRepeat` stay (they are
  player 1's).
- If the player sets player 2 to Human on a phone, player 2 has no input, as in C++ without a keyboard.
- The phone text field (4½e Q5) serves NAME, WEAPON n and SAVE PROFILE AS…; FIRE confirms, MENU cancels. Key capture on a
  phone binds whatever DOS key the touch button sends, as C++ would; the design does not hide the rows.

### 7.5 Preview parameters

`?cpu=1` makes player 2 a CPU, `?cpu=2` both; like every URL parameter it changes the in-memory settings only. A
touch-only page implies `?cpu=1`.

---

## 8. Split recommendation and task outline (Q1)

**Recommendation: two sub-slices, CPU first.** 4½f-1 carries everything sim-affecting (the AI, per-worm health,
`reacts`) and the thing John can play at once (a phone game against a real opponent); it needs no menu, because the
controller and the healths already come from `liero.cfg`. 4½f-2 is the wide menu surface (like 4½e-2) plus the live-input
rewrite that key bindings need. Each is about the size of 4½e-1.

**4½f-1**

| Task | Deliverable | Gate |
|---|---|---|
| T0 | C++ probes P1 (AI seed), P2 (`reacts` at construction and first AI read), P4 (no generated names), P5 (HEALTH edit reaches RESUME), P6 (colour at RESUME) | output recorded in the plan |
| T1 | `sim`: `reacts` kept; `max_health` per worm; `ai_params` | unit + **full re-diff** each |
| T2 | `sim::ai::DumbLieroAi` + `run_ais` | unit (each arm, fixed words) |
| T3 | `scenario`: per-worm health build, `AsymmetricHealth` removed, `apply_live_settings`, the Q2 refusal; `ui` boot | unit |
| T4 | C++: `ai` directive + reacts intervention; G-AI + G-HP generators; goldens | clang; prior sim goldens byte-identical |
| T5 | Rust harnesses | **G-AI + G-HP bit-exact** |
| T6 | `ui::Match` AIs, RESUME palette; `render` lifebar | headless unit flows |
| T7 | C++ shell: intervention 3′, FollowAI guard; G2f-1 corpus | prior shell goldens byte-identical |
| T8 | 🎯 MILESTONE f-1 | **G2f-1 bit-exact** |
| T9 | `game`: touch-only P2 = CPU, `BotRespawn` removed, `?cpu=`, page text | wasm build; Chromium phone walk |
| T10 | PROGRESS / overview / maps, broad review | CI commands |

**4½f-2**

| Task | Deliverable | Gate |
|---|---|---|
| T0 | C++ probes P3 (DIG overflow target), P7 (first match), P8 (a failed profile load), P9 (TC weapon names ASCII) | recorded |
| T1 | `ui::text` key names, controllers, Levenshtein + fuzzy match | unit (C++ vectors from T0) |
| T2 | `ui::keys::clean_words` + the DIG rule; harness switch; live game | unit + **full re-diff** (incl. `key_edges`) |
| T3 | `ui::shell::player_menu`, `CurMenu::Player`, arms, `WaitForKeyState`, overlays, network palette | headless unit flows |
| T4 | profiles: `ProfileSelectorState`, store refs, save/load, browser profiles, `placeable_leaf` | unit |
| T5 | `render`/`ui`: names in selection and banners | unit + **full re-diff** |
| T6 | C++ shell: tops `K`/`F`, `cur` letters, `profiles` manifest line; G2f-2 corpus | prior shell goldens byte-identical |
| T7 | 🎯 MILESTONE f-2 | **G2f-2 bit-exact** |
| T8 | `game`: phone text for NAME/WEAPON/profile, page help line | wasm; Chromium |
| T9 | Xvfb PNGs, docs, broad review | CI commands |

---

## 9. Out of scope

- FollowAI and the hidden menu's AI FRAMES / MUTATIONS / PARALLELS / TRACES (after Step 5a; the rows are 4½g's).
- Gamepads: input, the INPUT cycle over real devices, gamepad capture in `WaitForKeyState`.
- The modern colour picker and F10 (4½g); BOT WEAPONS in the hidden menu (4½g).
- The stats screen's names (4½g), replay file names with player names (menu recording, 4½d Q6, still postponed).
- Netplay use of the network player (Step 5).
- The F8 weapon randomiser (overview deferral).
- localStorage (4½h).

---

## 10. Risks

- **`reacts` (finding 2).** An uninitialised C++ read sits on the AI's path from the first tick. Mitigation: T0 P2, the
  zeroing intervention in both dumpers, the checked build, and the `ai_close` witnesses for `reacts`-driven presses.
- **Draw-order drift in the AI.** One extra or missing `rand` call shifts the whole stream. Every draw is unconditional
  except the fallback's `rand(16)` and the rope and Change arms; the `rand.last` column catches a drift on the tick it
  happens, and the ledger forces each conditional arm.
- **Per-worm health is a wide, quiet change** (about ten call sites across `worm`, `bonus`, `weapon`, `sobject`,
  `game`). Mitigation: the scalar is removed, not shadowed, so the compiler finds every site; the full re-diff; G-HP
  varies every site's witness.
- **The live-input rewrite (§4.5)** touches the path every live player uses. Mitigation: the 4½e `key_edges` case and
  `record_regression` must hold; `clean_words` has C++-derived unit vectors (T0 P7).
- **The DIG overflow (finding 3)** could leak into a gate as a false mismatch. The `dig` case pins it with equal values,
  and the generator refuses any other DIG binding.
- **FollowAI profiles (Q2).** The shipped `AI (L/R)` profiles are the obvious thing to load; whatever Q2 decides must be
  visible, not silent.
- **Scope.** Two 4½e-1-sized sub-slices; f-2 can slip without blocking 4½g or 4½h, and f-1 alone gives the phone its
  opponent.

---

## 11. Test strategy

1. **Unit.** `DumbLieroAi` arms on hand-built states (target choice, each `max_dist` arm, the scan, each fallback
   quadrant, the rope arm, `reacts`); `run_ais` order; per-worm health sites; `clean_words` (first match, gamepad skip,
   DIG); the DIG rule; Levenshtein and fuzzy match against T0 vectors; each player-menu behaviour; `WaitForKeyState` (last
   key wins, Esc, repeat); the continuations; profile refs across LOAD SETUP; the network palette; the refusals.
2. **Differential G-AI / G-HP** (§6.3, §6.4).
3. **Differential G2f-1 / G2f-2** and the two milestones, every frame plus `d` and `file` lines.
4. **`game`, headless.** A recorded human-vs-CPU session replays to the same state hashes (the AI words are the record);
   the touch-only boot makes player 2 a CPU; `?cpu=`; `round_trip.rs` and `record_regression.rs` unchanged.
5. **Eyeball.** Xvfb PNGs; the Chromium phone walk against the CPU.
6. **Standing.** Full re-diff, golden audit, wasm build, native smoke with a scratch `OPENLIERO_TEST_USER_DIR`.

---

## 12. Decided in this design

1. The AI seed is the C++ constant `mt19937(0x1337)`, fresh per CPU player per match (finding 1). No dumper seed
   override; Rust uses `Rand::new()`. Overview Q3 is closed.
2. `DumbLieroAi` lives in `sim::ai`, reads `&SimState`, owns only its `Rand`, is not hashed and not in `SimState`; the
   `Match` owns one per CPU player, made at match start, and runs them after the key edges, before the tick, in C++'s
   alternating order.
3. `WormState::reacts` persists between ticks and starts at 0; both dumpers zero C++'s uninitialised copy (finding 2;
   safe-edges ruling).
4. Health is per worm in the sim; unequal healths play; the `AsymmetricHealth` refusal and the boot's sanitising copy go.
5. The sim oracle gains one oracle-only directive, `ai`, with four opt-in columns; the shell dumper gains tops `K`/`F`,
   `cur` letters `L`/`R`/`N`, intervention 3′ and the `profiles` manifest line — all byte-compatible with every existing
   golden.
6. The live keyboard follows `controls_ex` with C++'s first-match rule and the full DIG rule; the harness uses the same
   function. The touch pad and the touch DIG chord are unchanged.
7. The key names and controller names are ported as C++'s hard-coded tables (overview Q4 corrected).
8. No random player names: C++'s `GenerateName` is compiled out, so Rust ports the no-op (PROGRESS corrected).
9. The fuzzy weapon match uses integer cross-multiplication, provably the same order as C++'s doubles.
10. The classic colour picker only (0..252 step 4); a `modernColors` setup is kept in the file and not used.
11. NETWORK PLAYER (F9) opens the same player menu for the third `WormSettings`, including the slot-0 palette case; its
    settings have no other use until Step 5.
12. Profiles: LOAD PROFILE keeps the colour index and marks the profile loaded even if parsing fails; SAVE PROFILE writes
    the user copy; SAVE PROFILE AS… refuses shipped names natively and only `liero.cfg` in the browser; the browser lists
    the eight shipped profiles; LOAD SETUP forgets loaded profiles.
13. RESUME while attached brings health, names and colours (`Game::Focus`) into the match; CONTROLLER does not change a
    running match.
14. `?cpu=1|2` is a preview parameter, never saved.
15. Non-ASCII names and typed weapon names are outside the gate (4½e decision 11).

---

## 13. Open questions for John

The recommendation is listed first each time.

**Q1. Should 4½f land in two parts?**
- **A (recommended):** Two parts, the computer opponent first. **4½f-1:** the CPU player (the original's bot), the phone's
  player 2 becomes that bot, and players can have different health. **4½f-2:** the LEFT/RIGHT PLAYER menus — names,
  health, colours, keys, saved weapons, the CPU/Human switch — and saving/loading player profiles. You can play the phone
  against a real opponent after the first part.
- **B:** Two parts, the menus first, then the CPU player.
- **C:** One slice, one PR.

**Q2. The CONTROLLER setting has three choices: Human, CPU and "AI". "AI" is the original's smarter bot, which cannot be
ported until the netplay groundwork (Step 5a) exists. Two shipped profiles, "AI (L)" and "AI (R)", pick it. What should
happen when a match would start with an "AI" player?**
- **A (recommended):** A short box says "AI PLAYERS ARE NOT SUPPORTED YET" and you stay in the menu, where you can switch
  that player to CPU. The menu looks exactly like the original. (The same approach you chose for Holdazone.)
- **B:** Quietly play "AI" players with the CPU bot instead; the menu still says "AI".
- **C:** Leave "AI" out of the CONTROLLER choices, and load the "AI" profiles as CPU players.

**Q3. On a phone, player 2 has no controls. Today it is a stand-in that only respawns. What should it become?**
- **A (recommended):** The real CPU player. It starts as CPU on a phone (the RIGHT PLAYER menu shows it and you can change
  it), gets random weapons each match, and fights back. The stand-in is removed.
- **B:** The real CPU player, but it keeps its saved weapons (by default five copies of the first weapon), as the
  stand-in does today.
- **C:** Keep today's stand-in, and make the CPU available only when chosen in the menu.

**Q4. The original has a bug: assigning a key to DIG also changes the player's WEAPON 1 to a different weapon (and for
some keys it can crash). Should Rust copy it?**
- **A (recommended):** Fix it. DIG gets the key and WEAPON 1 stays as it was. This only differs where the original's
  behaviour is undefined, like the earlier map-edge fix.
- **B:** Copy it, to stay identical wherever the original does not crash.

**Q5. Rust has no gamepad support yet. Loading one of the shipped "Joystick" profiles switches that player to a gamepad,
so the keyboard no longer moves it (the original does the same when no gamepad is plugged in). What should Rust do?**
- **A (recommended):** Same as the original. The INPUT row shows the gamepad, and pressing Enter on INPUT switches the
  player back to Keyboard.
- **B:** Treat a gamepad player as a keyboard player until gamepad support arrives; the INPUT row still shows the
  gamepad.
