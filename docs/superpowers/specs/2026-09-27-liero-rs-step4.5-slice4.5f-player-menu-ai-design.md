# Step 4½, Slice 4½f — the player menu, profiles and DumbLieroAI: design

Status: **4½f-1 LANDED** (plan plans/2026-09-27-liero-rs-step4.5-slice4.5f1-plan.md); 4½f-2 planned · **DESIGN — John's rulings recorded at the end** · 2026-09-27 · branch `claude/cpp-oracle-vcpkg-assets-chcwcm` (on `liero-rs-step-4-5` at `c2d58fe`; 4½a ✅, 4½b ✅, 4½c-0 ✅, 4½c ✅, 4½d ✅, 4½e-1 ✅, 4½e-2 ✅ landed)
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

## Rulings (John, 2026-09-27)

All five recommendations were accepted:

- **Q1 → A, two parts, the CPU first.** 4½f-1: DumbLieroAI (`sim::ai`), the phone's player 2 as the real CPU, per-worm health, persisted `reacts`, `?cpu=`. 4½f-2: the LEFT/RIGHT PLAYER menus, profiles, key capture with the full DIG rule, names. Each part has its own milestone, PR and preview.
- **Q2 → A.** Starting a match with an "AI" (FollowAI) player shows an "AI PLAYERS ARE NOT SUPPORTED YET" box and stays in the menu, with the menu unchanged from C++ (the same approach as Holdazone).
- **Q3 → A.** On a phone, player 2 starts as the real CPU, gets random weapons each match, and fights back. The RIGHT PLAYER menu shows it and it can be changed. The stand-in (`BotRespawn`) is removed.
- **Q4 → A.** Fix the C++ DIG-binding overflow: DIG gets the key and WEAPON 1 is unchanged. This is an intended divergence only where C++ is undefined behaviour.
- **Q5 → A.** Joystick profiles behave as in the original: the INPUT row shows the gamepad, the keyboard no longer moves that player, and Enter on INPUT switches it back to Keyboard.

---

## 4½f-2 refresh (2026-09-28, after 4½f-1 landed)

Read at HEAD `d2de489` (the merge of 4½f-1 into `liero-rs-step-4-5`) against the f-1 plan
(`plans/2026-09-27-liero-rs-step4.5-slice4.5f1-plan.md`, its Addendum T0, Addendum T9 and Known pitfalls), PROGRESS
("4½f-1 LANDED", "Also open for John (4½f-1)"), the landed Rust and dumper code, and the C++ source again. Where this
section and §3–§8 disagree, **this section wins**; John's rulings (Q1–Q5 above, and Addendum T9's `?cpu=`) stand
unchanged. Findings of this refresh are cited **R-n**, its decisions **RD-n**.

### R1. What changed since the design

#### R1.1 Already landed in 4½f-1 — no longer 4½f-2 work

- `sim::ai::DumbLieroAi` + `run_ais` (`rust/sim/src/ai.rs`), `WormState::{reacts, max_health}`, `SimState::ai_params`
  (design §4.1 `sim`, §4.2, §4.3).
- `AsymmetricHealth` removed, `apply_live_settings` writes both maxes, `refuse_follow_ai` at the NEW GAME gate only
  (plan D1; `rust/ui/src/shell/playing.rs:72-112` `bootable`, `:186-192` the `debug_assert!`) — design §4.12 is done.
- `Match` owns and runs the AIs (`playing.rs:143-150`, `:359-397`) — design §4.4 is done.
- **RESUME's `Game::Focus` palette** (the colour part of design §4.11): `Match::focus` re-runs `focus_palette` on every
  RESUME (`playing.rs:334-341`, plan D9, T0 P6). f-2 only has to *gate* the colour edit (`player_live`).
- The touch rule split (plan D3): `selection::touch_settings` (`selection.rs:43-46`) at boot through
  `MatchParams::apply_cpu` (`rust/game/src/web_params.rs:149-162`, called at `main.rs:540`) and again after every LOAD
  SETUP on a touch-only page (`rust/ui/src/shell/mod.rs:751-753`); `new_game_config` sets BOT WEAPONS RANDOM on a
  touch-only page (`selection.rs:76-82`); `BotRespawn` is gone — design §7.4 bullets 1–2 and §7.5 are done.
- Oracle: `oracle_dump_shell` intervention 3′, check 3″ and the FollowAI guard
  (`src/tools/oracle_dump/shell_dump.cpp:821-857`) — design §6.5's "Intervention 3′" and "FollowAI guard" bullets are
  done.

#### R1.2 Stale, wrong or underspecified

- **R-1. The live-input seam is not where §4.5 puts it.** Three samplers exist, and none reads the menu's live settings:
  - the shell harness ORs every binding (`rust/oracle-tests/tests/shell_common/mod.rs:1194-1210`, `words()`), called
    with the **boot** settings (`:1539`, `words(&held, &settings)`), not `sh.settings()`; its menu-key validator set
    `controls` (`:1385-1390`) and the `p2_keys` set `p2_controls` (`:1392-1397`) are also frozen at boot. After a
    rebind all three are stale;
  - the live game samples hard-coded Bevy keys (`InputSource::Live(default_bindings())`, `rust/game/src/input.rs:90-128`,
    `:311-322`) in `sample_inputs_touch` (`main.rs:1403-1422`) and hands the words in as `ShellInput::sampled`
    (`mod.rs:97-103`);
  - `Match::process(sim, sampled, ais, sounds)` (`playing.rs:359-397`) feeds `KeyEdges` (`keys.rs:247-261`) and
    `ReleaseLatch` (`keys.rs:184-207`), both 7-bit `ControlState`s: the DIG bit has nowhere to live.

  Only the `Shell` holds the settings a rebind changes, so the words must be computed inside it (**RD-1**, §R3).
- **R-2. §4.11's "unported subtlety" can be ported for free.** C++ `FindControlForKey` reads the *game's* worms'
  `WormSettings` (`game.cpp:87-106`): the menu's objects while attached, the old ones after LOAD SETUP. Key events
  reach `OnKey` only while `GamePlayState` is on top (`gamePlayState.cpp:16` passes the controller; the menu's
  `HandleEvent` does not, `mainMenuState.cpp:150`), and edits happen only while paused. So reading the **match's copy**
  (`Match::settings()`, `playing.rs:320-322`), which RESUME refreshes while attached (`resync`, `:300-306`), is exact in
  both cases. **RD-2:** live clean words read `Match::settings()`; `player_live`'s detached half now gates that the old
  bindings keep working after LOAD SETUP (T0 P7 confirms).
- **R-3. The WaitForKey box appears on the push frame, not the next one** (design §3.3 is wrong there). The Enter arm
  pushes inside `MainMenuState::Update` (`mainMenuState.cpp:371-389`); `StateStack::Draw` then starts at the topmost
  non-overlay (`state.hpp:117-130`), which is the new `WaitForKeyState`, so the push frame presents the previous frame's
  pixels with the box on top, and the menu is not redrawn. The pop frame redraws the menu with the new key name. The
  same holds for `ProfileSelectorState` (as 4½e-2's selectors already do).
- **R-4. The `d` line's `cur` letters `L`/`R`/`N` (§6.5) collide with the page hook.** `game::touch::hooks` publishes
  `lieroSel` as `L<n>` for the level selector (`rust/game/src/touch.rs:353-359`), so `L3` would mean two things.
  **RD-3:** `cur` ∈ {`M`, `S`, `1`, `2`, `N`} (player 1, player 2, the network player, from
  `player_menu.ws == settings->worm_settings[i]`), in the `d` line and in `lieroSel` alike; the `d` line's `ssel` is
  then `player_menu.Selection()`. The awk gate's `$3 !~ /^[MS]$/` (`rust/oracle-tests/gen_shell_golden.sh:71`) and the
  upd/top sets `[MWGOIBLP]` / `[MGOIBLP-]` (`:63`) widen to `[MS12N]`, `[MWGOIBLPKF]`, `[MGOIBLPKF-]`.
- **R-5. A saved file whose name has a space breaks the `file` line gate.** Six of the eight shipped profiles have a
  space (`AI (L)`, `Lefty (R)`, …). SAVE PROFILE of a loaded one writes `user/Profiles/Lefty (L).toml`, and the dumper's
  `file <rel> <fnv16>` line then has 4 fields, which `gen_shell_golden.sh:82` (`NF != 3`) refuses. The manifest side is
  the same limit (`shell_dump.cpp:538-547` splits on whitespace), which the design's `profiles <user|sys>` line already
  answers for inputs. **RD-4:** no format change; the harness refuses a case whose run writes a user file with a space
  in its path, and `profile_io` (§6.7) SAVEs over `Joystick0` or a user-saved `mine` instead of `Lefty (L)` (LOADing
  `Lefty (L)` is fine: it writes nothing).
- **R-6. The search-gap check and its mirror must learn the profile selector.** `kSearchable` is `O|L|P`
  (`shell_dump.cpp:788`), and the harness's "a player's key typed in a searchable menu" validator is
  `matches!(top0, 'O' | 'L' | 'P')` (`shell_common/mod.rs:1507`). Both gain `F`.
- **R-7. Key names for the scripts.** The dumper's `ScancodeOf` (`shell_dump.cpp:233-282`) and the harness's `key_of`
  (`shell_common/mod.rs:153-194`) need a key that `SDLToDOSKey` maps to 89: **not** `F13`, because `key_of` turns any
  `F<n>` into `58 + n` (`F13` → 71, keypad 7). Use `APPLICATION` (`SDL_SCANCODE_APPLICATION` is not in
  `liero_to_sdl_keys`, `keys.cpp:9-60`, so it is 89 on both sides) and add `TAB` to `ALLOWED` (`:63-127`; C++ has it).
  F5, F6 and F9 join `ALLOWED` (they were refused because Rust was inert); F10/F11 stay refused (C++ acts on them in
  `ProcessEvent` whatever the state, `gfx.cpp:613-624`, and Rust has no F10 before 4½g). The harness never sets
  `ShellInput::restart`, so the Rust-only F5 restart cannot fire in a gate.
- **R-8. `IntegerEntry` cannot tell a player row from a settings row.** `ValueEntry` carries only `item_id`
  (`rust/ui/src/menu/behavior.rs:19-30`) and `input_done` resolves it through `SettingsModel::int_field` and
  `settings_menu.item_from_id_mut` (`mod.rs:652-677`); player ids 1–4 (HEALTH, Red, Green, Blue) are also settings ids.
  The design's "`IntegerEntry` gains a target" (§4.1) is therefore required, not optional: `InputPurpose::IntegerEntry
  { entry, target: Settings | Player(p) }`.
- **R-9. The Save-As chain is hard-wired to setups.** `InfoPurpose::Reserved { typed, x, y }` (`overlay.rs:225-233`),
  `save_as_box` (`mod.rs:283`), `save_setup_as` (`mod.rs:688-718`) and `storage::placeable_leaf` (which formats
  `Setups/{leaf}`, `rust/scenario/src/storage.rs:151-156`) all assume SAVE SETUP AS…. The reserved box's reopen must
  reopen the **profile** box for the same player, with `initial = typed` and the `Profiles`/`.toml` pair
  (`mainMenuState.cpp:69-91`, `:356`). **RD-5:** one `SaveAsKind { Setup, Profile(player) }` threads through purpose,
  box and reopen; `placeable_leaf(subdir, leaf)`.
- **R-10. Names need a resync, not only a start value.** `Selection` keeps its own `names` (`selection.rs:97-99`,
  default at `:112`); `Match::resync` refreshes only `cfg`, `hud` and `weap_table` (`playing.rs:300-306`). C++ draws
  `ws.name` live on every weapon-selection frame (`weapsel.cpp:199-203`), so a match paused in selection must show a
  renamed player after RESUME: `resync` sets the selection's names too.
- **R-11. The Rust-only F5 restart bypasses the NEW GAME gate.** `Shell::frame` calls `new_game` directly on
  `input.restart` (`mod.rs:415-418`), and `Match::start` `debug_assert!`s that no player is FollowAI
  (`playing.rs:186-192`). D1 lets a paused match RESUME with a CONTROLLER = AI edit, so with the player menu a
  pause → CONTROLLER AI → RESUME → F5 reaches that assert (a debug panic; natively a LOAD SETUP of a user file with
  `controller = 2` already could). **RD-6:** the restart runs `RefusalGate::refusal(MA_NEW_GAME)` first and is ignored
  (with a console note) when it would be refused. And since F5 can now be bound as a key, the restart also ignores an
  F5 that is one of the running match's keyboard bindings (C++ has no restart to collide with).
- **R-12. The page text is stale in one place.** `web/index.html:219` says "F5 restarts a match"; in the menu F5/F6/F9
  now open the player menus. The help line becomes "in a match, F5 restarts it (Rust only); in the menu F5 / F6 / F9 open
  LEFT / RIGHT / NETWORK PLAYER", plus the design's "Keys can be changed in LEFT/RIGHT PLAYER." (§7.3). The touch hint
  table (`web/index.html:403-409`) gains a line for the key box (top `K`).
- **R-13. The shell case count and the frozen set moved.** 32 prior shell cases (not 28) must regenerate
  byte-identically under both C++ builds; `EXPECTED_SHELL_CASES` (`gen_shell_golden.sh:98`) becomes 32 + 9 = 41 (the 8
  §6.7 cases and the milestone); `shell_f1_cases/`, `gen_slice4_5f1_shell.rs` and `gen_slice4_5f1_sim.rs` join the
  frozen provenance, and the four `-- check`s (4½d, e-1, e-2, f-1) must stay clean.
- **R-14. f-1's pitfalls that bind the f-2 corpus** (f-1 plan, Known pitfalls): no RANDOM bot in a shell case (15);
  a CPU still receives the keys bound to it (16) — `key_bind` keeps player 2 human; both human players press DONE (T0 P5
  note); release every worm key before Esc (19); no Holdazone NEW GAME/RESUME (20); no level under ~342 rows (21).
- **R-15. Two DIG constraints for every generated case, not just `dig`.** A DIG binding may only be to the key whose
  DOS code equals that player's WEAPON 1 at bind time (Q4 = A: Rust leaves WEAPON 1 alone, C++ overwrites it, R2-14), and
  never to a key above 40 (the checked C++ build aborts on the out-of-range `weap_order` read in WEAPON 1's row, R2-14).
  The generator validates both.

### R2. Re-verified C++ facts for 4½f-2

| # | Fact (C++) | Verdict | Source |
|---|---|---|---|
| R2-1 | 24 rows in the §0-12 order; ids `kPlName`=0 … `kPlLoadedProfile`=23; the four profile rows colour 3, dis 7, the rest 48/7; menu at (178, 20), `value_offset_x` 95 | CONFIRMED | `gfx.cpp:459-483`, `:268`, `:525`; `gfx.hpp:41-62` |
| R2-2 | PROFILE LOADED and SAVE PROFILE are visible iff `profile_node`; PROFILE LOADED's value is `GetBasename(GetLeaf(FullPath))` | CONFIRMED | `gfx.cpp:223-227`, `:237-248` |
| R2-3 | `PlayerSettings`: point `player_menu.ws`, `UpdateItems`, `MoveToFirstVisible`, focus. First visible = SAVE PROFILE AS… with no profile (f-1 T0 P5 walked Down ×3 to HEALTH), PROFILE LOADED with one | CONFIRMED | `gfx.cpp:1430-1437` |
| R2-4 | HEALTH `IntegerBehavior(1, 10000, 1, %)`, `scroll_interval` 4; R/G/B classic `0..252` step 4, `display_div` 4, `scroll_interval` 4 (modern mode needs F10, which C++ reads globally in `ProcessEvent`) | CONFIRMED | `gfx.cpp:1370-1389`, `:621-624` |
| R2-5 | INPUT: "Keyboard"; else a connected pad's display name, else `gamepad_name.substr(0, 20)`, else `"Gamepad (none)"`. Left/Right plays MoveUp for `dir > 0`, MoveDown for `dir < 0`, cycles, returns false; Enter plays MenuSelect, cycles +1, returns −1; with no pad the cycle always lands on Keyboard and clears name and serial, then `UpdateItems` | CONFIRMED, sharpened (the sound follows the direction) | `gfx.cpp:149-199` |
| R2-6 | Key rows show `GetKeyName(controls_ex[i])`, or `GetGamepadKeyName(gamepad_controls[i])` for a pad player; DIG's behaviour uses `controls_ex[7]` for both refs. A Joystick profile's `[11, 12, 13, 14, 110, 10, 0, 9]` shows Up, Down, Left, Right, RT+, RB, A, LB | CONFIRMED | `gfx.cpp:56-63`, `:1392-1408`, `:842-859` |
| R2-7 | WEAPON n: `EnumBehavior(v, 1, common.weapons.size(), broken_left_right = false)`, value `weapons[weap_order[v - 1]].name`. A file value of 0 or > 40 is an out-of-range `vector` read (UB); Rust shows an empty value there (safe edges) | CONFIRMED, sharpened | `gfx.cpp:253-262`; `common.hpp:157` |
| R2-8 | CONTROLLER: `ArrayEnumBehavior` over `{"Human", "CPU", "AI"}`; a file value ≥ 3 is an out-of-range read (UB; Rust shows empty) | CONFIRMED | `gfx.cpp:1410-1411`; `common.cpp:214-216` |
| R2-9 | The colour bar: `DrawRoundedBox(x + 24, y, selected ? 168 : 0, 7, (rgb >> 2) - 1)`, `FillRect(x + 25, y + 1, rgb >> 2, 5, ws->color)`; `color` is 32 / 41 / 32 by default | CONFIRMED | `gfx.cpp:1343-1360`; `settings.cpp:32-34` |
| R2-10 | `UpdateMenuPalettes` re-applies `SetWormColours(*settings)` on **every** menu frame (so an R/G/B edit recolours the bars and the menu at once), then slot 0 from the network player while its menu is open. Rust's `menu_palette` already takes both players' rgb (`mod.rs:899-910`); it gains the slot-0 case | CONFIRMED | `gfx.cpp:978-1005`; `gfx/palette.cpp:92-118` |
| R2-11 | Entry: Enter on LEFT/RIGHT/NETWORK PLAYER after the main MenuSelect; F5/F6/F9 from any focus, no sound, main cursor moved to the item. Esc or any keyboard player's Jump: back to main, cursor kept. Draw: the player menu enabled only while focused; with main focus the **settings** menu is drawn disabled | CONFIRMED | `mainMenuState.cpp:171-179`, `:207-216`, `:447-462`, `:614-626` |
| R2-12 | The Enter arms: LOAD PROFILE, NAME (`InputString(ws.name, 20, x + 97, y)`), SAVE PROFILE AS… (`MakeSaveAsState("Profiles", ".toml", "", …)`, 30 bytes), the eight key rows (`WaitForKeyState(extended = true)`), WEAPON n (`InputString("", 10, x + 97, y)`), else `OnEnter` | CONFIRMED | `mainMenuState.cpp:316-429` |
| R2-13 | **Sounds at close:** `InputStringState` plays MenuSelect itself before its callback (`inputState.cpp:78`), so NAME (accepted or Esc) and SAVE PROFILE AS… play **two** MenuSelects on the closing frame (the callbacks' `mainMenuState.cpp:343`, `:362`), WEAPON n one, and a key capture none (`WaitForKeyState::Update` plays nothing, `inputState.cpp:143-150`); each arm's push frame plays one | CONFIRMED, sharpened | as cited |
| R2-14 | **The DIG overflow.** The callback writes `ws.controls[kEyIdx] = k` whenever `!IsExtendedKey(k)` and then `controls_ex[kEyIdx] = k` (`mainMenuState.cpp:375-388`). For DIG, `kEyIdx = 13 - 6 = 7`; `uint32_t controls[kMaxControl = 7]` is followed directly by `uint32_t weapons[5]` (`worm.hpp:54`, `:106-107`; all `uint32_t`, no padding), so the write lands in `weapons[0]` = WEAPON 1. Every keyboard key is < 177 (`SDLToDOSKey` returns a table index or 89, `keys.cpp:70-84`), so it is **always** written: formally UB for every DIG binding. In practice WEAPON 1 becomes the key's DOS code — a valid weapon for 2..40 (1 is Esc, which cancels), and for 41..176 an out-of-range `weap_order` read right away in WEAPON 1's `OnUpdate` (the callback's `UpdateItems`), then in weapon selection (`weapsel.cpp:66`) and `InitWeapons` (`worm.cpp:704`). Every default player 2 key (160, 168, 163, 165, 117, 144, 54) and every key from Z (44) on is > 40. T0 P1 probes the aliasing (`offsetof` delta 28) | CONFIRMED, sharpened | as cited |
| R2-15 | `WaitForKeyState`: every `KEY_DOWN` (OS repeats included) sets `result_ = SDLToDOSKey(sc)` — the last key-down of the frame wins; Esc (1) is a no-op close; `Update` = `ClearKeys`, callback, pop; not an overlay; the box `DrawRoundedBox(cx, cy, 0, h + 1, w + 1)` + "PRESS A KEY" at colour 50 around (160, 100); gamepad arms unreachable without pads. **The box shows on the push frame** (R-3) | CONFIRMED except the push frame (CORRECTED) | `inputState.cpp:102-162`; `state.hpp:117-130` |
| R2-16 | Key names: `Texts::key_names[177]` is static, Finnish-layout (`12 "+"`, `13 "`"`, `26 "Å"`, `27 "^"`, `29 "Left Crtl"`), and **`key_names[89]` is `""`**: an unmapped key binds as 89 and its row shows blank. `GetKeyName`: < 177 the table, ≥ 512 `"J<n>_<b>"`, 177..511 `""` | CONFIRMED, sharpened (the blank name) | `common.cpp:25-203`; `gfx.cpp:828-840` |
| R2-17 | The fuzzy match: `Levenshtein` over bytes with `std::tolower` both sides, an `unsigned` matrix; `best` starts at the current value with distance `DBL_MAX`, strict `<` (ties to the lowest index; the first candidate always wins); divided by `name.length()` bytes. The openliero TC's 40 names are ASCII (no byte ≥ 0x80 in `data/TC/openliero/weapons/*.cfg`; at most 13 bytes), so the design's integer cross-multiplication is exact and **design T0 P9 is closed on the desk** | CONFIRMED | `mainMenuState.cpp:28-52`, `:398-417` |
| R2-18 | **A failed profile load.** `LoadProfile` saves `color`, calls `ToReader()` **before** `profile_node = node` — an unreadable file throws first, so PROFILE LOADED does not change (a console warning only); a TOML syntax error throws in the `TomlInputArchive` constructor (`toml_archive.hpp:161-167`) **after** `profile_node` is set, so PROFILE LOADED shows the file and no field changes; a file that parses is lenient — a missing key or a wrong type keeps the field (`toml_archive.hpp:217-268`), and a file without `rgbDepth` shifts the rgb values `(v & 63) << 2` even when `rgb` itself is missing. `color` is restored in every case. Rust's `load_profile` already matches (`rust/scenario/src/settings_toml.rs:236-244`, `:150-156`) | CONFIRMED, sharpened (design finding 15 holds only once the file opens) | `worm.cpp:73-95`; `serialization/cereal_types.hpp:282-308` |
| R2-19 | Profile schema: root keys `name, health, controller, randomName, color, inputDevice, gamepadName, gamepadSerial, rgbDepth` (written 8, read default 6), `rgb[3], weapons[5], controls[7], controlsEx[8], gamepadControls[8]`, sorted by `toml::table` on save; `SaveProfile` sets `profile_node` after `ToWriter()` succeeds. Locations: LOAD PROFILE lists the whole config root filtered to `TOML` (case-insensitive) and opens inside `<root>/Profiles`, title `Select profile:`, `OnSelected` = `LoadProfile` + `UpdateItems` (no `MoveToFirstVisible`); SAVE PROFILE writes `user/Profiles/<leaf>` with **no** shadow check; SAVE PROFILE AS… checks `ShadowsSystem(user, "Profiles", leaf)` | CONFIRMED | `worm.cpp:60-71`; `fileSelectorState.cpp:186-206`; `gfx.cpp:207-221`; `mainMenuState.cpp:69-91`, `:348-366` |
| R2-20 | Names are drawn in the weapon-selection name box (colour `kWormColorBlocks[index].base + 1`) and the kill banners (`KilledMsg + name`, `name + CommittedSuicideMsg`); the HUD name is replay-only (`viewport.cpp:134-136`), the stats screen is 4½g, spectator/rematch unported | CONFIRMED | `weapsel.cpp:199-203`; `viewport.cpp:256-270` |
| R2-21 | **No gamepad present.** The dumper inits `SDL_INIT_EVENTS` only (`shell_dump.cpp:664`), so `gfx.joysticks` is empty: WaitForKey's pad arms, `DispatchGamepadInput` and every `TestGamepad*` never fire. A pad-input player is skipped by `TestControl(Once)` (menus, `gfx.cpp:869-883`, `:954-968`) and `FindControlForKey` (play, `game.cpp:87-93`), but not by `ReleaseControl` (`:970-976`). **New consequences:** (a) the network player's default keys equal player 1's (`settings.cpp:52-60`), so after a Joystick profile in LEFT PLAYER, R/F/D/G/LCtrl/LAlt still drive the menus through the network player; (b) a human pad player can never press DONE, so a NEW GAME stays in weapon selection until Esc | CONFIRMED + two consequences (T0 P6) | as cited |
| R2-22 | `Game::OnKey` (`game.cpp:58-72`, `controls[]`, every match) has no caller; only `LocalController::OnKey` → `FindControlForKey` (first match over `controls_ex[0..8]`) matters. OS repeats never reach it (`gfx.cpp:608`) | CONFIRMED | `localController.cpp:58-80` |

### R3. The refreshed 4½f-2 task table

**Decisions this refresh makes (engineering, "match the original"):**
- **RD-1 (the input seam).** `ShellInput::sampled` becomes `held: &DosHeld` (the physical keyboard's held DOS keys: the
  harness's `held` set; natively and on wasm Bevy's `ButtonInput<KeyCode>` through `dos_of_keycode`) plus `touch:
  ControlState` (the Rust-only phone overlay, OR-ed into player 1's word exactly as today, `touch.rs` `merge`). The
  `Shell` computes `clean_words(held, match_settings) -> [u8; 2]` (first keyboard player 0/1, first control 0..8, bit 7
  DIG; pad players skipped, Q5), folds DIG into Left+Right for weapon selection (as `words()` does today), and hands
  8-bit words to `Match`; `KeyEdges` and `ReleaseLatch` widen to 8 bits and `KeyEdges` ports both arms of the DIG rule
  (a DIG key held across NEW GAME or RESUME stays latched, as C++ never saw its key-down). The harness drops `words()`
  and recomputes `controls` / `p2_controls` from `sh.settings()` / the match every frame. `--live <scenario>`,
  `--replay`, `Scripted` and `?demo` keep `default_bindings()` and stay byte-unchanged.
- **RD-2** (R-2), **RD-3** (R-4), **RD-4** (R-5), **RD-5** (R-9), **RD-6** (R-11) as above.
- **RD-7.** `WaitForKeyState` and `ProfileSelectorState` map to `Phase::Menu` (`mod.rs:983-1002`); the page's hint for top
  `K` is its own line.
- **RD-8.** No new dumper intervention is needed: every f-2 path is real C++ code with no clock or uninitialised read
  (the numbering stays 1–9 with 3′, 3″ and 6′). The dumper gains only tops `K`/`F`, the `cur` letters, the `profiles
  <user|sys>` manifest line (design §6.5), the key names of R-7, and `F` in the search-gap check.

**T0 — the C++ probes** (the f-1 method: a temporarily patched `oracle_dump_shell` in the working tree and
`$S/build-chk`, restored and re-proven byte-identical; results in the f-2 plan's Addendum T0):

| Probe | Question | Expected (from R2) |
|---|---|---|
| P1 DIG | Where does the DIG write land, and what does each build do with a code > 40? | `offsetof(weapons) - offsetof(controls) == 28`; DIG = Q with WEAPON 1 = 1 → WEAPON 1 = 16 in `cfg16`; DIG = Z under the checked build aborts in `weap_order` (recorded, never gated) |
| P2 key box | Push-frame draw; last key-down of a frame; a repeat binds; Esc; `APPLICATION` | box on the push frame (R-3); last wins; repeat binds; Esc changes nothing, no sound; `APPLICATION` → 89, blank name |
| P3 sounds | The closing frame of NAME (Return, Esc, empty), SAVE PROFILE AS… (saved, reserved), WEAPON n, a key capture | 2 / 2 / 1 / 0 MenuSelects (R2-13) |
| P4 play | First match and the DIG rule in play | P1 FIRE bound to RCtrl: RCtrl moves P1 only, P2's FIRE is dead; DIG held → L+R on every event of that worm; release → L/R released unless cleanly held |
| P5 profiles | Selector root/title/listing in an `fs` fixture with `profiles sys`; a malformed and an unreadable `.toml`; SAVE PROFILE of a shipped one; SAVE PROFILE AS… a shipped name | opens inside `Profiles`, `.toml` only; R2-18's two failure shapes; the user copy; the reserved box and its reopen |
| P6 pads | `Joystick0` in LEFT PLAYER with no pad | INPUT "Gamepad (none)"; rows Up … LB; Enter/Left/Right → Keyboard with sounds; R/F/D/G still move the menu (network player); a NEW GAME stays in selection until Esc |
| P7 live edits | RESUME attached (a rename in a banner, a rebind acts at once) and detached after LOAD SETUP | attached: both reach the match; detached: the **old** bindings still act (RD-2), the new ones do not |
| P8 F9 | The network player's slot-0 palette | slot 0 shows its colours only while its menu has focus |

(Design T0 P9 — TC weapon names ASCII — is closed on the desk, R2-17.)

| Task | Deliverable | Gate | Size |
|---|---|---|---|
| T0 | P1–P8 above; addendum | recorded; `src/` clean; prior goldens byte-identical | S–M |
| T1 | `ui::text`: `KEY_NAMES[177]` verbatim (blank 89, "Left Crtl", "Å"), `get_key_name` (incl. `J<n>_<b>` and the blank 177..511), `get_gamepad_key_name`, `CONTROLLERS`, `levenshtein`, `weapon_fuzzy_match` (integer cross-multiplication) | unit (C++ vectors, T0) | S |
| T2 | The input seam (RD-1, RD-2): `clean_words`, 8-bit `KeyEdges` + `ReleaseLatch` with the full DIG rule, `ShellInput { held, touch }`, `Match` reads its own settings copy; the harness and the live game switched; RD-6's restart gate | unit + **full re-diff** (all 32 shell goldens, `key_edges`, `record_regression`, `round_trip`); wasm | M–L (the risky one) |
| T3 | `shell::player_menu` (24 rows, behaviours, bars, `CurMenu::Player(0\|1\|2)`), the §3.2 arms, F5/F6/F9, `WaitForKeyState` (top `K`), overlays `WormName` / `WeaponFuzzy` / `SaveProfileAs` / `IntegerEntry { target }` (R-8), `SaveAsKind` (R-9), the network palette slot, the Q4 fix (`controls[i]` only for `i < 7`) | headless unit flows | L |
| T4 | Profiles: `ProfileSelectorState` (top `F`, `toml_filter`), a per-player loaded-profile ref in `MenuWorld` (set by LOAD even on a parse error, kept on an unreadable file, set by SAVE / SAVE AS, cleared by LOAD SETUP and the boot), `placeable_leaf(subdir, leaf)`, the eight shipped profiles in `browser_system_files` (~3 KB), Q7's phone rule | unit | M |
| T5 | Names: `Selection` names from the match settings and refreshed by `resync` (R-10); `Scene::names` for the kill banners (empty → old goldens hold) | unit + **full re-diff** | S |
| T6 | C++ `oracle_dump_shell`: `TopOf` K/F, `DetailLine` cur `1`/`2`/`N` + the player menu's `ssel`, `F` in the search-gap check, the `profiles` manifest line, `APPLICATION`/`TAB` key names; `gen_shell_golden.sh` regexes and the count 41 (32 until T7) | prior 32 shell goldens byte-identical under release + checked; clang-format 22 + clang-tidy | M |
| T7 | G2f-2: `shell_f2_cases`, `gen_slice4_5f2_shell.rs`, the 8 §6.7 cases with R-5/R-14/R-15 applied + 🎯 `shell_player_setup`; validators (DIG rules, no space in a saved path, F10/F11 refused, ASCII text, no controller-2 NEW GAME); a negative control with `ShellDebug::live_bindings = false` (the 4½e default bindings) that must diverge in `key_bind` | **G2f-2 bit-exact** (every `f`/`d`/`file` line) under both C++ builds | L |
| T8 | `game` + page: RD-1's live sampler, Q6's touch mapping, Q7, hooks (`lieroSel` with `1`/`2`/`N`, `F<n>`; a `lieroProfiles` or `lieroNames` read-back for the walk), the help lines (R-12), the `K` hint | `cargo test -p game`; wasm; Chromium: phone (RIGHT PLAYER shows CPU, NAME through the text field, LOAD PROFILE, a match), desktop (a live rebind acts at once; F5 in the menu opens LEFT PLAYER, in a match restarts) | M |
| T9 | Xvfb side-by-sides of the milestone path, PROGRESS / overview / maps, broad review | CI board; golden audit (only `A`) | S–M |

**Order:** T0 → (T1 ∥ T6, C++ only) → T2 → T3 → T4 → T5 → T7 → T8 → T9. T2 goes before the menu on purpose: it is
the one change on every live player's path, and all 32 prior shell goldens plus `key_edges` must hold before the menu
builds on it.

### R4. The phone and the browser

- **Reaching the menus.** A phone has no F5/F6/F9, but the pad reaches LEFT/RIGHT/NETWORK PLAYER and FIRE is Enter
  (the main menu's `TestControlOnce(kFire)`), so the menus are reachable as C++ would have them. RIGHT PLAYER shows
  "CPU" (the touch rule, R1.1).
- **NAME and WEAPON n** use the 4½e phone text field unchanged: `Shell::text_mode` already picks the letter keyboard for
  a box without a digit filter (`mod.rs:1080-1086`), `TouchKeys` makes FIRE = Return and MENU = Esc in phase `text`
  (`touch.rs:123-165`), and `HINTS.name` fits. HEALTH and R/G/B get the number keyboard, as SETUP numbers do. Decided;
  no question.
- **Key capture — a real phone hazard (Q6).** To the menus the pad *is* player 1's keyboard: each button sends player
  1's current `controls_ex` key (`touch.rs:74-97`). Binding one of player 1's controls to another button's key makes
  both buttons one key: e.g. AIM UP = FIRE's Left Ctrl makes FIRE move the cursor up (the Up test consumes the key first,
  `mainMenuState.cpp:181-197`), so nothing can be selected any more, and there is no keyboard to undo it. Today a reload
  resets it (the browser store is in memory); once 4½h keeps settings in localStorage it would survive the reload.
- **Joystick profiles on a phone (Q6).** A Joystick profile in LEFT PLAYER makes player 1 a pad player: the menus still
  react only because the network player happens to share player 1's default keys (R2-21a), and in a match RD-1's
  keyboard words skip player 1 while the touch overlay would still drive it. The Q5 ruling ("as the original") covers
  keyboards; what the phone's buttons do is Q6.
- **LOAD PROFILE into player 2 on a phone (Q7).** Six of the eight shipped profiles say "Human" (`controller = 0`),
  two say "AI". Loaded into RIGHT PLAYER on a phone, a Human profile leaves player 2 without controls: the next NEW GAME
  waits in weapon selection until MENU. LOAD SETUP already puts the CPU back on a phone (plan D3); LOAD PROFILE does not
  yet.
- **Profiles in the browser.** Decided (design §5, §9; the setups precedent): the browser store lists the eight shipped
  profiles; SAVE PROFILE / SAVE PROFILE AS… write into the session's in-memory store, a single layer as in the C++ web
  build, where only `liero.cfg` is reserved (4½e-2 D9) — so saving over a shipped name replaces it for the session; all
  of it is lost on reload until 4½h adds localStorage for setups and profiles together. Desktop wasm behaves the same.
- **The INPUT row.** Rust has no gamepad support and does not use the browser Gamepad API; Q5 (as the original with no pad
  connected) applies on desktop and wasm alike: "Gamepad (none)", and Enter/Left/Right switch back to Keyboard.
- **Desktop wasm key capture** works like native, except for keys the browser keeps for itself; the canvas already gets
  F5 (the 4½c restart works there). RD-6 keeps a bound F5 from restarting the match.

### R5. New open questions for John

The recommendation is listed first each time. Q1–Q5 are answered above; these two are new.

**Q6. On a phone, the on-screen buttons act as player 1's keys, and the new menu can change those keys. If a key is
set to the same key as another button (for example AIM UP set to the FIRE button), that button stops working in the
menus, and a phone has no keyboard to undo it — only reloading the page does, and not even that once settings are
remembered in the browser (4½h). Loading a "Joystick" profile into LEFT PLAYER has a similar effect. What should the
phone's buttons do?**
- **A (recommended):** The buttons always work, whatever keys are set: in the menus they act as the arrow keys, Enter and
  Esc; in a match they move player 1 directly, as they do today, even after a Joystick profile. The key rows still show
  and change player 1's keyboard keys (for a keyboard plugged in later); on a phone, a button pressed in the PRESS A KEY
  box sets the arrow or Enter key it stands for, and MENU cancels.
- **B:** Hide the eight key rows and INPUT on a phone, so they cannot be changed there. Everything else as the original.
- **C:** Exactly as the original: the buttons send player 1's current keys, and a bad choice is undone by reloading the
  page.

**Q7. On a phone, player 2 is the CPU. Most shipped profiles (Lefty, Righty, Joystick) are marked "Human". If one is
loaded into RIGHT PLAYER on a phone, what should happen?**
- **A (recommended):** Player 2 stays the CPU; the profile's name, colour, health, weapons and keys load. This is what
  LOAD SETUP already does on a phone. You can still switch CONTROLLER to Human yourself.
- **B:** As the original: player 2 becomes Human. A phone has no controls for it, so the next match waits in weapon
  selection until you set CONTROLLER back to CPU.

## Rulings (John, 2026-09-28, 4½f-2)

Both recommendations were accepted:

- **Q6 → A, the phone buttons always work.** On a touch-only page the on-screen buttons act as the arrow keys, Enter and Esc in the menus and move player 1 directly in a match, whatever keys are set and even after a Joystick profile. The eight key rows still show and change player 1's keyboard keys (for a keyboard plugged in later). In the PRESS A KEY box a button sets the arrow or Enter key it stands for, and MENU cancels.
- **Q7 → A, player 2 stays the CPU on a phone.** Loading a Human profile into RIGHT PLAYER on a touch-only page loads its name, colour, health, weapons and keys, and keeps player 2 as CPU, as LOAD SETUP already does. CONTROLLER can still be switched to Human by hand.
