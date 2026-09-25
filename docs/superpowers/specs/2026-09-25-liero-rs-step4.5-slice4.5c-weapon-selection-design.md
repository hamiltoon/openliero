# Step 4½, Slice 4½c — the weapon selection phase: design

Status: **DESIGN** · 2026-09-25 · branch `liero-rs-step-4-5` (4½a ✅, 4½b ✅, 4½c-0 ✅ landed)
Part of: `2026-09-10-liero-rs-step4.5-game-shell-overview.md` (the 4½c bullet, LD 7, Hard gate 3; cited **overview**)
Built on: `2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` §3, §6, §8 (cited **cpp-map**) and
`2026-09-10-liero-rs-step4.5-rust-baseline-map.md` §1–§5, §9 (cited **rust-map**)
Precedents: `2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` (the `settings <file>`
directive, the §4.4 builder seam; cited **4½a design**), `2026-09-10-liero-rs-step4.5-slice4.5b-level-generation-design.md`
(the RNG-position golden and the replica + self-check dumper shape), `2026-09-10-liero-rs-step4.5-slice4.5c0-weapon-branches-design.md`
(reach witnesses; the dumper-edit regeneration proof)
Next artifact: `plans/2026-09-25-liero-rs-step4.5-slice4.5c-plan.md`

Every match in C++ starts with a pick screen. Each player gets a seven-line menu (RANDOMIZE, five weapon
slots, DONE), cycles weapons with Left/Right and confirms with Fire. The match starts when both players are
ready. The screen is presentation, but its **constructor and its RANDOMIZE item draw from `game.rand`, the
simulation RNG** (`weapsel.cpp:61`, `:68`, `:323`). So the number and order of draws before match frame 0
decides every later tick. 4½c ports the phase into the Bevy-free `sim` crate, bit-exact against a new C++
dumper, and puts it in front of the live match.

---

## 0. Summary of findings (read this first)

Read out of the C++ source for this slice. Items 1, 2, 5 and 10 contradict or sharpen the overview and
the maps.

1. **The constructor's rejection loop runs only when a pick is *disabled*.** For each slot the constructor
   draws `rand(1, 41)` once if the saved pick is 0 or the player is a RANDOM bot (`weapsel.cpp:57-62`). It
   enters the redraw loop only if the resulting weapon is disabled (`:66`). Uniqueness (`!weap_used[kW]`
   when at least five weapons are enabled) is checked **only inside** that loop (`:67-75`). So an enabled
   saved duplicate is kept, and so is an enabled first draw that repeats an earlier slot, even with 40
   weapons enabled. The overview's "redraws until an enabled and (if enough are enabled) unused weapon comes
   up" (overview 4½c bullet, cpp-map §3) describes RANDOMIZE, whose loop always runs (`:322-330`), not the
   constructor. Per slot the constructor draws 0 or 1 times, plus the loop's k draws.
2. **PICK does not make the bot navigate the menu.** cpp-map §6 says "PICK(1) makes the bot navigate the
   menu". The AI never runs during weapon selection. `LocalController::Process` ticks AIs only in
   `kStateGame`/`kStateGameEnded` (`localController.cpp:153-165`); the weapsel branch (`:124-152`) does
   not. With PICK, the bot's menu is driven by **the keys bound to that worm** (`game.cpp:87-108`), so a
   human picks for the bot. PICK (`1`) is also the C++ default (`settings.hpp:24`, `select_bot_weapons{true}`).
   In full: RANDOM iff `== 0` (`weapsel.cpp:57`), auto-ready iff `controller != 0 && != 1` (`:95`). A
   file value ≥ 3 therefore behaves like KEEP (`hiddenMenu.cpp:10` offers only 0..2).
3. **The menu is a seven-state cursor.** Weapsel adds its items with `items.emplace_back`
   (`weapsel.cpp:49`, `:87`, `:90`), not `Menu::AddItem` (`menu.cpp:295-302`), so `visible_item_count`
   stays 0. `SetTop` then clamps `top_item = bottom_item = 0` forever (`menu.cpp:220-225`), and `Draw` never
   draws a scrollbar (`:107`). All seven items are visible and selectable, and `Movement(±1)` wraps
   (`menu.cpp:263-293`). The whole menu state is `cursor ∈ 0..=6`. `WeaponSelectSnap`'s
   `menu_top_item`/`menu_bottom_item` (`weapsel_snapshot.hpp:30-31`) are constant 0.
4. **Picks are written back in memory, not on disk.** `WormSettings` is a `shared_ptr` shared between
   `Gfx::settings` and the worms (`settings.hpp:99`, `localController.cpp:34`, `:41`). Every constructor
   roll and every Left/Right lands in the live settings at once. The next NEW GAME in the same session
   starts from the last picks, and SAVE SETUP would persist them. The TODO at `weapsel.cpp:360` is about
   the disk only (cpp-map §3 states the disk half).
5. **C++ has two key-repeat implementations, and they disagree.** `LocalController` resets a bit's held
   counter while the bit is still set (`localController.cpp:140-143`). `RollbackController` counts every
   held frame that is not a rising edge (`rollbackController.cpp:522-533`). They diverge when a held key is
   not consumed for a while. For example: hold Left from frame 0 with the cursor on RANDOMIZE (Left is not
   read there), and press Down on frame 20. Local then cycles on frames 21, 33, 36, …; Rollback cycles on
   21, 24, 27, …. The overview cites only the Local one (LD 7). §4.5 decides.
6. **Order inside one frame is load-bearing.** The slot for Left/Right is taken from the cursor at frame
   start (`weapsel.cpp:225`). Left runs before Right (`:240-283`), then Up, then Down (`:286-306`). Fire
   then acts on the cursor **after** the move (`:316`, `:338`). Down+Fire on slot 5 in one frame therefore
   readies the player.
7. **RANDOMIZE re-rolls every frame while Fire is held.** Confirm is `Pressed(kFire)`, which does not
   consume the bit (`weapsel.cpp:309`, `worm.hpp:185`). The draw count grows with the hold. RANDOMIZE plays
   no sound and does not update `worm.weapons[j].type` (`:316-337`, cf. `:258`).
8. **Three inputs hang or crash C++.** With zero enabled weapons the constructor loop, RANDOMIZE and both
   cycling do-whiles never end (`:67-75`, `:322-330`, `:250-255`, `:273-278`). Only the UI prevents this
   (`weaponMenuState.cpp:91-109`); a TOML file can still carry it. `rand(1, 41)` hard-codes 40 weapons
   (`:61`, `:68`, `:323`), while cycling wraps at `common.weapons.size()` (`:253`, `:275`). A saved pick
   above 40 indexes `weap_order` out of bounds (`:66`).
9. **Replays start after the phase.** `ChangeState(kStateGame)` runs `Finalize` before `BeginRecord`
   (`localController.cpp:224-226`, `:237-268`, "NOTE: Must do this here before starting recording!"). An
   `.lrp` holds the post-selection `Game`, not the selection.
10. **`render::palette::rotate_from` already exists** (`rust/render/src/palette.rs:13`, used for
    `color_anim`). rust-map §2 and the overview's 4½d bullet call `Palette::RotateFrom` missing. What
    really is missing is `DrawRoundedBox` (`blit.cpp:128-140`) and `Font::GetDims` (`font.cpp:87-112`).
11. **Presentation facts.** The frozen screen is an ARGB copy resolved once (`weapsel.cpp:165-182`), so
    only colour 168 (the selected item, in the rotated range 168..174) animates. The frozen HUD shows
    "LIVES 0", because `Worm::lives{0}` (`worm.hpp:238`) is set from settings only at `kStateGame`
    (`localController.cpp:232-235`). The level label checks `level_file.empty()`, not `random_level`
    (`weapsel.cpp:171`). The menu sounds are crossed: Up plays `MenuMoveDown` and Down plays `MenuMoveUp`
    (`:293`, `:304`). Left plays `MoveUp` and Right plays `MoveDown` (`:248`, `:271`). DONE plays
    `MenuSelect` (`:340`).
12. **Rust gaps.** `sim_core::rng::Rand` does not derive `Clone` (`sim-core/src/rng.rs:28`). A rand probe
    and Step 5 both need it. `weap_order` is computed in two places (`scenario/src/loader.rs:123`,
    `build.rs:131-132`). C++ sorts it with an unstable sort (`common.cpp:498-499`) and Rust with a stable
    one. The two agree only while weapon names are unique.

---

## 1. Goal / done-when

**Goal.** The C++ weapon-selection phase runs in front of every Rust match with its RNG stream bit-exact.
The live game shows it pixel-faithfully and hands the picks into the match. It works on the keyboard and on
the browser's touch pad.

**Done when (9):**

1. **`sim::weapsel` matches C++ on every line of every `weapsel_*` golden (§6.3).** The init line, every
   frame and the final `InitWeapons` loadout agree on: picks, cursors, ready flags, the worms' control
   words, the key-repeat counters, the menu sound ids, draws per step, and `rand.last` plus the next
   value. The corpus (§6.4) is 16 cases, and the witness guard (§6.7) proves it reaches every branch.
2. **Two continuation sim goldens are bit-exact vs C++.** Each is 12 columns, including `IsGameOver`. The
   match runs after a real weapon selection, and the tick-0 row has a non-zero RNG column (§6.5).
3. **A handoff-equality test.** new match → selection → finalize → enter game equals
   `build_match(picks := final picks)` in every field except the RNG (§6.6).
4. **Every hazard in finding 8 is refused with an error.** No hang, no panic (§4.6).
5. **The live game has the phase.** A bare `cargo run -p game` shows weapon selection, then plays with the
   chosen loadouts. F5 and the post-match restart return to selection with the picks carried over. It
   works on the keyboard and with touch. `?weapons=` skips selection. `--live --record` skips it too, and
   the 4b round-trip gates stay green (§7).
6. **The weapon-selection screen is pixel-faithful** (§5): Rust self-goldens, plus a PNG compared by eye
   against the C++ build and noted in PROGRESS.
7. **Every prior golden is byte-identical.** The edited `sim_physics_dump` regenerates the eight
   settings-path goldens byte-identically (§6.8).
8. These are green: `cargo test --workspace --exclude game` (debug), `cargo test -p game`, the wasm build,
   and clang-format/clang-tidy on the dumper changes.
9. PROGRESS and the overview are updated, including the overview corrections (findings 1, 2, 10) and the
   Step-5 snapshot note (§8).

---

## 2. Inherited locked decisions (overview)

- **LD 3:** `tick_and_render` stays the only `Sim` mutator. Weapon selection mutates `SimState`
  (`rand`, worm control words, worm weapons), so it runs **inside** `tick_and_render`, as a phase. It is
  not a menu.
- **LD 4 / 4½a design §7.1:** the scenario format is frozen. The only growth is an oracle-only directive
  that is absent on every existing file (§6.1).
- **LD 5:** `SimState::new` is unchanged. The builder split (§4.7) is a post-`new` rearrangement.
- **LD 7:** the phase is a Bevy-free struct in `sim` with `process_frame(inputs) -> bool`, and the 12/3 key
  repeat is ported.
- **LD 1:** pixel-exact menus first (the §5 decision leans on this).
- **4½a design §4.4:** the builder splits into `new_match` / `enter_game` for this slice. The live default
  match stays on `scenario::load` until 4½d. Recording a `MatchConfig` match belongs to 4½d.

---

## 3. The C++ phase, precisely

### 3.1 Lifecycle

`GamePlayState::Enter` calls `controller->Focus()`, and a fresh controller then does
`ChangeState(kStateWeaponSelection)` (`localController.cpp:112-114`). That constructs
`WeaponSelection(game)` (`:228-229`) and sets `fade_value = 0` (`:119`). The RNG is the `Game`'s own, seeded
from the wall clock in `Game::Game` (`game.cpp:42`). The level was generated earlier from `gfx.rand`
(`gfx.cpp:1446`, `:1520`), so no sim draw comes before the constructor. Each `Process` runs the key
repeat, then `ws->ProcessFrame()` (`localController.cpp:124-152`), and the fade counts up to 33
(`:195-199`). The AI does not run (finding 2) and `game.cycles` does not advance. When `ProcessFrame`
returns true, `ChangeState(kStateGame)` runs, in this order: `Finalize` (`:224-226`), lives
(`:232-235`), replay recording (`:237-274`), `StartGame` (`:276`), `fade_value = 33` (`:284-287`). The
first match frame runs on the next `Process`. Esc during selection unfocuses it (`:94-96`). RESUME
refocuses the **same** object (`:106-108`), with nothing rebuilt. NEW GAME builds a new controller, so a
new selection starts from the in-memory picks (finding 4).

Menus map to worms through the viewports: `menus[i]` belongs to `game.viewports[i]->worm_idx`
(`weapsel.cpp:44-47`). `LocalController` pairs viewport *i* with worm *i* (`localController.cpp:47-48`).

### 3.2 The constructor (`weapsel.cpp:28-97`) — the RNG stream

```
enabled = count(weap_table[k] == 0 for k in 0..40)                       // :35-39
for i in 0..2:                                                           // player order = viewport order
  weap_used[256] = {}                                                    // :42 per player
  random = ws.controller != 0 && select_bot_weapons == 0                 // :57
  for j in 0..5:
    if ws.weapons[j] == 0 || random:  ws.weapons[j] = rand(1, 41)        // :60-62  (0 or 1 draw)
    enough = enabled >= 5                                                // :64
    if weap_table[weap_order[ws.weapons[j]-1]] > 0:                      // :66     disabled?
      loop: ws.weapons[j] = rand(1,41); w = weap_order[..-1]             // :67-70
            break if (!enough || !weap_used[w]) && weap_table[w] <= 0    // :72
    weap_used[weap_order[ws.weapons[j]-1]] = true                        // :80
    worm.weapons[j] = { type: weapons[w], ammo: 0 }                      // :82-85
  worm.current_weapon = 0; cursor = 0 (MoveToFirstVisible)               // :92, :94
  is_ready[i] = ws.controller != 0 && select_bot_weapons != 1            // :95
```

`rand(a, b)` is `rand(b - a) + a` with Lemire's multiply-shift (`rand.hpp:30-38`). Rust has the same as
`Rand::bound_range` (`sim-core/src/rng.rs:122`). `weap_table` is `uint32_t` (`settings.hpp:68`), so
`<= 0`, `== 0` and `!= 0`/`> 0` all mean "enabled iff 0" (the values are 0 menu, 1 bonus only, 2 banned).

### 3.3 `ProcessFrame` (`weapsel.cpp:219-350`)

For each player *i* that is not ready (`:232`). `kWeapId = cursor - 1` is fixed at frame start (`:225`).

- **Left** (`:240-260`), only if `kWeapId ∈ 0..5`: `Pressed(kLeft)`, then `Release(kLeft)`. Plays
  `MenuMoveUp`. `do { --pick; if pick < 1: pick = weapons.size() } while weap_table[weap_order[pick-1]] != 0`.
  Sets `worm.weapons[kWeapId].type`.
- **Right** (`:262-283`): the mirror image. Wraps above `weapons.size()` to 1 and plays `MenuMoveDown`.
- **Up** (`:286-295`): `PressedOnce(kUp)` plays `MenuMoveDown` and does `Movement(-1)`.
  **Down** (`:297-306`): `PressedOnce(kDown)` plays `MenuMoveUp` and does `Movement(+1)`. Both wrap over
  the seven items.
- **Confirm** (`:309`): `Pressed(kFire)`, which is not consumed. With cursor 0 it runs RANDOMIZE
  (§3.4). With cursor 6 it plays `MenuSelect` and sets `is_ready[i] = true` (`:338-342`). Any other
  cursor does nothing.
- `all_ready &= is_ready[i]` (`:346`). The return value is `all_ready`.

The gamepad branches (`:234-313`) are out of scope (§10). A ready player's input is ignored. Jump and
Change are never read.

### 3.4 RANDOMIZE (`weapsel.cpp:316-337`)

A fresh `weap_used`. For each of the five slots, `loop { pick = rand(1,41); break if (!enough ||
!weap_used[w]) && enabled(w) }`, then `weap_used[w] = true` and the item label is set. It always draws at
least five times. Unlike the constructor it always enforces uniqueness when `enough` holds. It plays no
sound and leaves `worm.weapons[].type` stale, which is harmless because `Finalize` overwrites it.

### 3.5 `Finalize` (`weapsel.cpp:352-361`)

`worm.InitWeapons(game)` for every worm: `current_weapon = 0`, and per slot
`type = weapons[weap_order[pick-1]]`, `ammo = type.ammo`, `delay_left = loading_left = 0`
(`worm.cpp:698-709`). Then `game.ReleaseControls()`, which clears control bits 0..6 of every worm
(`game.cpp:110-118`). The whole finalize draws nothing.

### 3.6 Input in `LocalController` (`localController.cpp:58-86`, `:128-148`)

Keys are events. `OnKey` sets `clean_control_states` (the physical state) and `control_states` (what the
worm sees) on key-down and key-up (`:58-67`). SDL repeat events are dropped (`gfx.cpp:608`). Each weapsel
frame, for bits 0..6:

```
if clean[bit]:
   if !control[bit]: ++held[bit]; if held >= 12 && (held-12) % 3 == 0: Press(bit)   // :133-139
   else:             held[bit] = 0                                                   // :140-143
else: held[bit] = 0                                                                   // :144-146
```

`kKeyRepeatInitial = 12` and `kKeyRepeatInterval = 3` (`localController.hpp:44-45`). A key held down
repeats at held-frames 12, 15, 18, … after each consumption (0.17 s, then every 43 ms at 70 Hz). Only
Left, Right, Up and Down are consumed, so only they repeat. Fire stays set while held (finding 7). At the
end, `ReleaseControls` clears `control_states`. A key still held gets no new key-down event, so **it does
nothing in the match until it is released and pressed again**.

### 3.7 Drawing (`weapsel.cpp:160-209`, normal viewports)

- **Palette** (`:20-24`, `:163`): `pal = Origpal; pal.RotateFrom(Origpal, 168, 174, gfx.menu_cycles)`.
  There is no `color_anim` and no worm-colour step. `menu_cycles` carries over from the main menu (reset
  at `mainMenuState.cpp:145`) and increments once per frame after the draw, because `GamePlayState` does
  not want the menu flip (`gamePlayState.hpp:13`, `gfx.cpp:1646`).
- **Frozen background, cached once** (`:165-180`): `game.Draw` (the level, the HUD, and the minimap when
  `settings.map` is set; worms are invisible), the weapsel palette again, then the level label at (0,162)
  in colour 50. The label is `LevelRandom` if `level_file` is empty, else `LevelIs1 + basename + LevelIs2`
  (`:171-176`; strings at `tc.cfg:246-248`). Stored as ARGB (`:178`) and copied back every frame (`:182`).
- **Header:** `DrawRoundedBox(114, 2, 0, 7, GetDims(SelWeap))`, then `SelWeap` at (116,3) in colour 50
  (`:188-190`).
- **Per player:** the name box at (`menu.x + 29 - w/2`, `menu.y - 11`) and the name at
  (`menu.x + 31 - w/2`, `menu.y - 10`), in colour `kWormColorBlocks[index].base + 1`, which is 33 or 42
  (`:200-203`, `palette.cpp:77-79`). Then the menu, unless ready (`:205-207`).
- **Menu placement:** (`vp.rect.CenterX() - 31`, `CenterY() - 51`) (`weapsel.cpp:51-55`), with integer
  `(x1+x2)/2` (`rect.hpp:72-73`). That is (48,28) and (208,28) for the two viewports (`localController.cpp:47-48`).
  Items are 8 px apart (`menu.hpp:37`, `menu.cpp:104`).
- **Item recipe** (`menuItem.cpp:6-42`), with `centered = false` and no value. Selected:
  `DrawRoundedBox(x, y, 0, 7, GetDims(s))`. Otherwise a shadow `s` at (x+3,y+2) in colour 0. Then `s` at
  (x+2,y+1) in colour 168 if selected, else the item colour: RANDOMIZE 57, weapons 48, DONE 10
  (`weapsel.cpp:49`, `:87`, `:90`).
- `DrawRoundedBox` is three `FillRect`s (`blit.cpp:128-140`). `GetDims` sums glyph widths, NUL is a line
  break, bytes 2..251 only (`font.cpp:87-112`).
- The renderer fade (`localController.cpp:211`) is applied when the frame is composed. The spectator
  variant (`:99-158`) is out of scope.

---

## 4. Design — the sim-affecting core

### 4.1 Home: `sim::weapsel` (new module, `rust/sim/src/weapsel.rs`)

The phase lives in `sim`, per LD 7, for four reasons. It draws `SimState.rand`. It writes worm control
words and weapons, both of which the hash reads (`sim/src/hash.rs:49`, `:65-73`). Step 5 must snapshot it
next to `SimState`. And `sim` is Bevy-free. `sim` does not depend on `scenario`, so the configuration is a
plain `sim` type, and the `Settings` → config mapping lives in `scenario::build` (§4.7).

### 4.2 Types and API

```rust
pub const MENU_ITEMS: u8 = 7;        // RANDOMIZE, 5 slots, DONE — weapsel.cpp:49, :87, :90
pub const RANDOMIZE_ITEM: u8 = 0;
pub const DONE_ITEM: u8 = 6;         // weapsel.cpp:338 "TODO: Unhardcode"
pub const KEY_REPEAT_INITIAL: u16 = 12;   // localController.hpp:44
pub const KEY_REPEAT_INTERVAL: u16 = 3;   // localController.hpp:45
const RAND_PICK_END: u32 = 41;            // rand(1, 41): weapsel.cpp:61, :68, :323

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeapselConfig {
    pub weap_table: [u32; 40],       // Settings::weap_table (by weapon index)
    pub select_bot_weapons: u32,     // raw: RANDOM iff 0, auto-ready iff != 1 (finding 2)
    pub players: [WeapselPlayer; 2], // index = worm index = viewport index (§3.1)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeapselPlayer { pub weapons: [u32; 5], pub controller: u32 }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponSelection {
    weap_order: Vec<usize>,          // derived once from state.weapons (§4.7)
    weap_table: [u32; 40],
    enabled_weaps: i32,
    players: [PlayerSel; 2],
    repeat: [KeyRepeat; 2],
    menu_sounds: Vec<i32>,           // this frame's side channel; not snapshotted (§4.8)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerSel { pub picks: [u32; 5], pub cursor: u8, pub ready: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KeyRepeat { prev: u32, held: [u16; 7] }

impl WeaponSelection {
    /// The constructor (§3.2): draws `state.rand`, sets each worm's weapons to {type, ammo 0} and
    /// `current_weapon = 0`.
    pub fn new(state: &mut SimState, cfg: &WeapselConfig) -> Result<Self, WeapselError>;
    /// One `LocalController::Process` weapsel frame: key repeat (§4.5), then `ProcessFrame`.
    /// Returns true on the frame the last player readies.
    pub fn process_frame(&mut self, state: &mut SimState, inputs: &[ControlState; 2]) -> bool;
    /// `Finalize` (§3.5): `init_weapons` + release controls. Returns the picks for write-back.
    pub fn finalize(self, state: &mut SimState) -> [[u32; 5]; 2];
    pub fn player(&self, i: usize) -> &PlayerSel;      // for drawing and write-back
    pub fn menu_sounds(&self) -> &[i32];
}
pub fn init_weapons(state: &mut SimState, weap_order: &[usize], picks: &[[u32; 5]; 2]); // Worm::InitWeapons ×2
pub fn weap_order(weapons: &[Weapon]) -> Vec<usize>;   // common.cpp:491-499, the one shared copy
```

Worm control words live where C++ keeps them, in `state.worms[i].control_states`. `finalize` clears them,
so the tick-0 hash sees exactly what C++ `ReleaseControls` leaves.

### 4.3 The constructor, in draw order

A line-for-line port of §3.2: player 0's five slots, then player 1's. It draws `state.rand.bound_range(1, 41)`,
indexes `self.weap_order`, keeps a per-player `weap_used: [bool; 40]`, uses finding 1's loop-entry rule,
and applies the raw `select_bot_weapons` predicates. It writes `state.worms[i].weapons[j] =
{ ty: Some(w), ammo: 0, .. }` and `current_weapon = 0`. The frozen HUD reads these, so the write is kept
even though finalize overwrites it.

### 4.4 `process_frame`

For each player: `self.repeat[i].apply(inputs[i], &mut state.worms[i].control_states)` (§4.5). Then, if
the player is not ready, §3.3 in exactly that order: take `kWeapId` from the cursor at frame start, then
Left, Right, Up, Down, then confirm on the new cursor. Cycling writes `state.worms[i].weapons[k].ty`, as
C++ does at `:258`/`:281`. RANDOMIZE (§3.4) is a helper shared in spirit with the constructor, but a
separate loop, because its loop-entry rule differs. The cursor is `(cursor ± 1) mod 7` (finding 3).
`all_ready` is folded over both players on every frame, as at `:346`.

### 4.5 Key repeat — `LocalController` semantics on sampled words (decided; §11 Q2)

Rust input is one sampled `ControlState` per tick per worm (`game/src/input.rs:278-289`,
`sim/src/state.rs:65`). `KeyRepeat::apply(cur, ctl)` replays `OnKey` from the change in the sampled word,
then runs the loop in §3.6 verbatim:

```
rising  = cur & !prev ;  falling = prev & !cur
ctl |= rising ; ctl &= !falling                          // OnKey down/up, localController.cpp:58-67
for bit in 0..7:                                          // :128-147
  if cur.bit: if !ctl.bit { held+=1; if held>=12 && (held-12)%3==0 { ctl.press(bit) } } else { held=0 }
  else: held = 0
prev = cur
```

This is the single-player truth, the controller the live Rust game mirrors. The Rollback variant differs
(finding 5). Rust peers run the same code on both sides in Step 5, so C++ netplay parity is not a
constraint. The corpus pins the choice with a case built on finding 5 (§6.4). Sampling loses one thing
events keep: a release and re-press between two ticks. Step 4 already accepted that (the level-sampled
contract). DIG reaches the phase as Left+Right (the sampler's chord, `input.rs:57-62`). Left then Right
cycle back to the same pick (net zero) and play two sounds. C++ drives DIG through `OnKey`'s chord
(`localController.cpp:69-79`) with the same picks, so this is a known difference in sounds only.

### 4.6 Refusals (`WeapselError`, never a panic or a hang)

- `WeaponCount(n)`: `state.weapons.len() != 40` (the `rand(1, 41)` hard-code, finding 8).
  `build.rs:97-99` already refuses more than 40.
- `NoWeaponsEnabled`: `enabled_weaps == 0`. C++ would loop forever (finding 8). This mirrors
  `LS(NoWeaps)` (`weaponMenuState.cpp:91-109`).
- `InvalidPick { worm, slot, value }`: `value > 40`. `0` is legal and means "roll" (`weapsel.cpp:60`).

Nothing else can hang. With at least one enabled weapon, every loop ends with probability 1. When
`!enough`, uniqueness is waived (`:72`, `:327`).

### 4.7 Handoff into the match — the 4½a seam, cut

The seam in 4½a design §4.4 becomes three functions:

| Function | Crate | C++ | Does |
|---|---|---|---|
| `new_match(tc_root, &MatchConfig, &LevelData) -> Result<Loaded, BuildError>` | `scenario::build` | `LocalController` ctor, `localController.cpp:30-54` | everything `build_match` does **except** weapons, lives and pool. Worms get `health`, `stats_x`, invisible, `lives = 0` (`worm.hpp:238`), default weapons. Pick validation is relaxed to `0..=n`. |
| `WeaponSelection::new` / `process_frame` / `finalize` | `sim::weapsel` | `weapsel.cpp` | §4.3–§4.5; `finalize` = `init_weapons` + release |
| `enter_game(&mut SimState, &MatchConfig)` | `scenario::build` | `ChangeState(kStateGame)`, `:232-235`, `StartGame` `game.cpp:513` | `lives = settings.lives`, `bobjects = BloodPool::new(blood_particle_max)` |

`build_match` becomes `new_match` → `sim::weapsel::init_weapons(cfg picks)` → `enter_game`, keeping its
strict validation. Its output is unchanged: its unit tests and the eight settings-path goldens prove this
(`build.rs:330-412`). `weapsel_config(&Settings) -> WeapselConfig` joins `scenario::build`. `loader.rs:123`
and `build.rs:131-132` switch to `sim::weapsel::weap_order` (hash-neutral, and a new unit test asserts that
the TC's weapon names are unique, finding 12). `sim_core::rng::Rand` gains `#[derive(Clone)]` (finding 12).

**Write-back.** `finalize` returns the picks. The owner (the live game in 4½c, the `ScreenStack` in 4½d)
copies them into its in-memory `Settings` at the points where C++ aliasing makes them visible: at
finalize, and when a selection is abandoned (F5, or Esc to the menu in 4½d). This is behaviourally the
same as the `shared_ptr` (finding 4), and it keeps `WeaponSelection` a plain value.

### 4.8 Menu sounds

`process_frame` clears `menu_sounds` and pushes `state.sound_hooks.MenuMoveUp` / `MenuMoveDown` /
`MenuSelect` (`assets/src/tc.rs:298-300`) at the four C++ call sites, crossed as in finding 11. A negative
hook is skipped, as `SoundPlayer::Play`'s `sound >= 0` guard does (`mixer/player.hpp:19`). This is a side
channel like 4c's `sound_events`: hash-inert, not part of the snapshot, drained by `game` into its
`AudioSink`.

---

## 5. Rendering scope — the key scoping decision (§11 Q1, **John**)

Weapon selection draws with the C++ `Menu` machinery, and the full framework is 4½d. There are two
options.

- **A — pull a minimal pixel-exact subset forward (recommended).** 4½c adds four things:
  (1) `render::blit::draw_rounded_box`, a three-`fill_rect` port of `blit.cpp:128-140`.
  (2) `render::font::Font::get_dims`, a port of `font.cpp:87-112`.
  (3) `render::menu::draw_item`, the text arm of `MenuItem::Draw` (`menuItem.cpp:6-42`: box or shadow,
  colour 168 or item colour). The `has_value` arm is left to 4½d.
  (4) `render::weapsel`: the weapsel palette (the existing `rotate_from` over 168..174, then
  `pack_pal32`), the frozen-background build (`frame::draw` plus the level label), and the per-frame
  draw of §3.7.
  That is roughly 150 lines plus tests, all Bevy-free, so `shot` can use it. **Not pulled forward:**
  the generic `Menu` (visibility, scrolling, the scrollbar, type-to-search, `ItemBehavior`), render fade,
  the spectator variant and `Focus`/`Unfocus`. The weapsel menu needs none of them (finding 3).
- **B — sim first, with a placeholder draw.** Draw plain `draw_string` lines and leave pixel parity to
  4½d. That saves about 100 lines but writes code only to delete it, ships a screen that does not look
  like Liero (against LD 1), and moves a second menu self-golden set into 4½d.

**Recommendation: A.** Every primitive in it is one 4½d needs anyway: `DrawRoundedBox` is on 4½d's own
list, and `MenuItem::Draw` is the item recipe of every menu. The subset has no framework in it, so 4½d
builds `Menu` on top of it and changes none of it. If overview Q1 puts widgets in a new `rust/ui` crate,
`ui` depends on `render` and reuses `render::menu::draw_item` as is.

**Presentation state stays out of `sim`.** `game` keeps a `WeapselScreen { frozen: Option<Bitmap>,
menu_cycles: u32 }`. The frozen frame is built on the first render after construction. `menu_cycles`
starts at 0 when the phase is entered (C++ inherits it from the main menu, so 4½d threads the real
counter) and increments once per rendered tick, after the draw (`gfx.cpp:1646`). The live HUD flags
(`game::hud_mode`) apply to the frozen frame, as `game.Draw` would. The level label takes the level's
basename from the launcher. In 4½d it becomes `settings.level_file`, with the `empty()` rule of
`weapsel.cpp:171`. Pixel caveats in 4½c: no render fade (4½d), and the scenario-path frozen HUD shows the
scenario's lives, not C++'s 0 (§7.3). The `new_match` path shows 0.

---

## 6. Oracle

### 6.1 One new directive: `weapsel <frame> <worm0_7bit> <worm1_7bit>`

The phase needs per-frame inputs, and they have no home in the existing format. The sidecar is C++-schema
TOML and byte-gated (4½a-2), so it cannot hold them. `input` lines are match ticks: indexing them from the
selection would change the meaning of every existing file, or need a marker anyway. A separate input file
would give the continuation golden two sources of truth. Hence one directive, shaped like `input`:

- **Sparse:** an absent frame is `0`, and values are masked to 7 bits by `ControlState::unpack`, as
  `input` values are (`parser.rs:248-256`). Frame 0 is the first `ProcessFrame` call. A duplicate frame
  is an error.
- **Legal only with `settings`.** Both parsers reject it otherwise, the same discipline as the
  `settings` exclusions (`parser.rs:285-298`). Like `settings`, it is oracle-only in 4½c:
  `scenario::load` already refuses such a file (`loader.rs:102-104`).
- **Presence opts in.** At least one line means the phase runs (constructor plus frames) before match
  tick 0. `weapsel 0 0 0` is the idiom for "run it with no input", used when all players are
  auto-ready bots.
- **End-frame invariant.** The last `weapsel` line's frame must be exactly the frame on which
  `ProcessFrame` returns true. Both dumpers and the Rust tests check it, so a file with dead input
  (after the end) or too little input (the phase never ends) fails loudly. It also makes each file
  self-describing.
- `ticks` and `input` keep their meaning (match ticks). `to_text` writes `weapsel` lines after
  `settings`, in ascending order, before the `input` lines.

4½d promotes `settings` and `weapsel` together from oracle-only to the recording format (4½a design §4.4).

### 6.2 `oracle_dump_weapsel` (new, `src/tools/oracle_dump/weapsel_dump.cpp`)

Usage: `oracle_dump_weapsel <scenario> <out>`, run from the repo root. It reads the subset of the grammar
a settings scenario can hold (seed, level, ticks, settings, weapsel, input, comments), with the same
strictness as `sim_physics_dump`. Setup matches the settings path exactly (`sim_physics_dump.cpp:417-490`):
the real `Settings::FromToml`, `game.rand.Seed(seed)`, `Level::load`, then two worms with `health =
ws.health` and no `InitWeapons`. Two viewports are registered for the phase only
(`Rect(0,0,158,158)`/`Rect(160,0,318,158)`, as `localController.cpp:47-48`). The driver is shared with
§6.5 through `src/tools/oracle_dump/weapsel_drive.hpp`:

1. Construct the **real** `WeaponSelection(game)`.
2. For each frame f: turn the sampled words into `OnKey` effects (rising: `clean_control_states.Set` and
   `SetControlState(true)`; falling: both false), run a **verbatim replica** of
   `localController.cpp:128-148` over the real `Worm` methods, then call the **real** `ProcessFrame()`.
3. On the end frame, call the **real** `Finalize()`, then unregister the viewports.

**Self-check (recommended, §11 Q6).** Each case is run a second time through a real `LocalController`,
driven by `OnKey(controls_ex[bit], …)` and `Process()`, with `gfx.sound_player` set to a
`NullSoundPlayer` and `game.rand` reseeded before `Focus()`. The dumper exits 1 without writing unless
picks, cursors, ready flags, control words and `rand` agree on every frame. This is the 4½b pattern: the
replica slices the oracle's work, it is not the oracle. The weapsel sidecars set `recordReplays = false`
so `ChangeState(kStateGame)` stays off the filesystem (`localController.cpp:237`). If `LocalController`
proves not to run headless (the `Gfx` coupling in `Focus`/`Process`), the replica stands alone, reviewed
line by line against the cited lines.

**Draw counts.** C++ `Rand` has no counter. The dumper copies the RNG before each step, then steps the
copy until `last` and the next value both match the live RNG; that step count is `draws`. It exits 1 past
10⁶ steps. The next value is `Rand probe = game.rand; probe();`. **Sounds** come from a dumper-local
`RecordingSoundPlayer : SoundPlayer` whose `PlayImpl` logs sample ids (`mixer/player.hpp:39`).

### 6.3 Golden format — `rust/oracle-tests/golden/weapsel_<case>.txt`

```
# provenance comments (ignored)
init <enabled> <p0> <p1> <draws> <last> <next>
f <frame> <in0> <in1> <p0> <p1> <ctl0> <ctl1> <held0> <held1> <sounds> <draws> <last> <next> <done>
final <l0> <l1> <last> <next>
```

- `<pN>` is `a,b,c,d,e:cursor:ready`: picks 1-based, `ready` 0/1.
- `<ctlN>` is the worm's control word after `ProcessFrame` (`%02x`).
- `<heldN>` is the seven repeat counters, comma-joined.
- `<sounds>` is the sample ids in call order, comma-joined, or `-`.
- `<draws>` is the RNG steps taken in this step. `<last>` and `<next>` are `%08x`.
- `<lN>` is five `weaponindex:ammo` pairs, comma-joined, then `:current_weapon`.

There is one `f` line per frame, and exactly one with `done = 1`: the last. It is always the last `weapsel`
line's frame (§6.1).

### 6.4 Corpus (16 cases, generated by `examples/gen_slice4_5c.rs`)

Scenario files and sidecars are written by a generator. As in 4½a, it builds menu scripts from helpers
(`tap(bit)`, `hold(bit, n)`) so every file reproduces from its header. All cases use
`Levels/render_stage.lev`. Cases 1–14 have `ticks 0`.

| # | Case | Varies | Pins |
|---|---|---|---|
| 1 | `humans_default` | `Settings()`, seed 1 | zero constructor draws; cursor wrap both ways; cycle wrap 1↔40; tap vs hold-Right repeat at held-frames 12, 15, 18; DONE by P0 first, then P1 |
| 2 | `unset_picks` | picks `[0,5,0,40,0]` / `[3,3,3,3,3]` | constructor draws only for zeros; **duplicates kept** (finding 1) |
| 3 | `disabled_saved_s1`, 4 `…_s2` | 12 disabled (mix of 1 and 2), saved picks among them | loop entry only when disabled, uniqueness inside the loop; cycling skips both 1 and 2; two seeds |
| 5 | `few_enabled` | 3 enabled | `enough = false`: duplicates allowed in the loop and in RANDOMIZE |
| 6 | `five_enabled` | exactly 5 enabled | the `>= 5` boundary: RANDOMIZE yields a permutation (long tails) |
| 7 | `one_enabled` | 1 enabled | about 40 draws per disabled slot; cycling wraps to the same weapon |
| 8 | `randomize_held` | Fire held 10 frames on RANDOMIZE | a re-roll every frame (finding 7); no sound |
| 9 | `same_frame` | Down+Fire on slot 5; Left+Right; Up+Down | the §3.3 ordering (finding 6) |
| 10 | `repeat_edge` | Left held on RANDOMIZE, Down at frame 20 | Local repeat semantics: cycles at 21, 33, 36 (finding 5) |
| 11 | `bot_random` | P1 `controller 1`, `select_bot_weapons 0` | bot draws all five unconditionally, auto-ready; P0 human |
| 12 | `bot_pick` | P1 bot, `select_bot_weapons 1` | bot not ready; driven by the worm-1 column (finding 2) |
| 13 | `bot_keep` | P1 bot with a zero and a disabled pick, `2` | auto-ready, saved picks kept, zero or disabled still rolled |
| 14 | `bots_only_7` | both bots, `select_bot_weapons 7` | ≥ 3 behaves as KEEP; done on frame 0 with `weapsel 0 0 0` |
| 15 | `match_humans` | humans, RANDOMIZE + cycling + DONE, then `ticks 600` of fuzz input | continuation (§6.5) |
| 16 | `match_bot` | P1 RANDOM bot + P0 human, then `ticks 600` | continuation; different picks per worm |

### 6.5 Continuation goldens — `sim_physics_dump` learns `weapsel`

In the settings path, `weapsel` lines select the §6.2 driver in place of `InitWeapons`. The rest of the
path is unchanged: `ResetWorms` (which equals lives plus `StartGame`'s pool, 4½a design §7.2), then the
tick loop with the viewports already unregistered. The 12-column output is unchanged, and
`gen_sim_slice4_5c_golden.sh` writes `sim_slice4_5c_match_{humans,bot}.txt`. The tick-0 row's `rng`
column is the post-selection `rand.last`. The weapon columns carry per-worm picks. The awk gate checks that
`rng` at tick 0 is non-zero. Absent `weapsel`, the settings path stays byte-identical, proven by §6.8's
regeneration.

### 6.6 Handoff equality (Rust, no C++)

For both continuation cases, and for a randomized property test over configurations, compare two paths.
Path one: `new_match` → the selection script → `finalize` → `enter_game`. Path two: `build_match` with the
config's picks replaced by the final picks, and the same seed. They must agree on every
`hash_components` field except `rng`. They must also agree on per-worm weapons, ammo, lives, pool
capacity, `current_weapon` and control words. After the second path's RNG is advanced by the recorded
draw count, the two master hashes must be equal. This is the "handoff equals the settings path" proof. The
continuation goldens make it end-to-end.

### 6.7 Rust milestone test — `oracle-tests/tests/weapsel_golden.rs`

For each `weapsel_*_scenario.txt`: `Scenario::parse`, `settings_from_toml` on the sidecar, `Level` load,
then `new_match`, `weapsel_config`, `WeaponSelection::new`, and a check of the `init` line. For each frame,
feed the scenario's words to `process_frame` and check the whole `f` line. `draws` comes from
`Rand::draws()` deltas, and `next` from a cloned RNG. Then `finalize` and a check of `final`.
`sim_slice4_5c_continuation_golden.rs` reuses the `sim_slice4_5a` 12-column harness with the phase
inserted.

**Witness guard (non-vacuity, as in 4½c-0 §5.3), derived from the driven Rust state over the whole
corpus:**

- a constructor loop with at least 2 iterations;
- a constructor-kept duplicate;
- a RANDOMIZE that rejected a duplicate, and one that kept a duplicate;
- a cycle across a disabled weapon;
- a cycle wrap in both directions, and a cursor wrap in both directions;
- repeats at held-frames 12 and 15;
- the `repeat_edge` timing;
- every `select_bot_weapons` value 0, 1, 2 and ≥ 3;
- a frame with one player ready and the other still moving;
- a done-on-frame-0 case;
- a multi-frame held RANDOMIZE.

The C++ golden is the correctness gate. The witnesses prove the corpus reaches the branches.

### 6.8 Presentation self-goldens and the standing gates

- **Self-goldens.** A `render` test builds the default match through `scenario::load` plus
  `WeaponSelection` over `Settings::default()`. It draws three states (initial, cursor on a slot with
  `menu_cycles = 5`, P0 ready) and compares `hash_frame(&surface, 33)` with committed constants. There is
  no C++ counterpart (rust-map §9).
- **Eyeball.** `shot --weapsel` renders the initial screen to a PNG. It is compared by eye with a C++
  screenshot on the same level (`level_file` set to `render_stage`). The `liero-shot` skill §7 gains a
  weapsel paragraph.
- **Standing gates.** Every task runs `cargo test --workspace --exclude game` (debug) and
  `cargo test -p game`, and no golden may change. After the `sim_physics_dump` edit, the eight
  settings-path goldens (`sim_slice4_5a_*` ×4, `sim_slice4_5c0_*` ×4) are regenerated, and
  `git status --porcelain -- rust/oracle-tests/golden` must be empty. The other dumper paths are
  untouched by the edit.

---

## 7. The live game

### 7.1 The phase in `tick_and_render`

`MatchFlow` (`game/src/match_flow.rs:22-26`) gains `MatchPhase::WeaponSelection` in front of `Game`, and
a constructor `MatchFlow::with_weapon_selection()` that starts at `fade_value = 0`, like the C++ `Focus`
at `localController.cpp:119`. During the phase, `tick_and_render` samples inputs once, merges touch,
applies the release latch (§7.2), then calls `ws.process_frame(&mut sim.0, &inputs)`, drains
`menu_sounds` into the audio sink, and bumps the flow's fade toward 33. It does **not** call the sim's
`process_frame`, the viewport stepping or the sim sound drain, and `game.cycles` does not advance, as in
C++. When the phase ends: `finalize` (then the write-back), `enter_game` on the builder path, the latch
is re-armed, the phase becomes `Game` at fade 33, and the next tick is match tick 0. Rendering uses
`render::weapsel` instead of `frame::draw` while the phase is on. `WeaponSelection` sits in `Demo` next to
`flow`. The phase's `&mut SimState` borrow only ever comes from `tick_and_render` (LD 3).

### 7.2 A release latch at both phase boundaries (§11 Q5)

C++ keys are edges. Keys held when the controller starts, such as the key that picked NEW GAME, never
reach the worms (§3.6). Keys held when selection ends, such as Fire from DONE, do nothing in the match
until they are pressed again (`ReleaseControls` plus no repeat events, `game.cpp:110-118`, `gfx.cpp:608`).
Rust samples levels. Without a latch, the DONE press would fire the first weapon on tick 0. `game::input`
therefore gains `ReleaseLatch { mask: [u32; 2] }`. It is armed with the held words at each boundary.
Each tick it clears released bits (`mask &= held`) and outputs `sampled & !mask`. It sits before the
recorder tap, so a recording replays exactly. It is live-only: Scripted and Replay feed recorded words.

### 7.3 What the live phase selects from in 4½c

The live default match stays on `scenario::load` (4½a design §4.4). The phase runs over the loaded state
with a `WeapselConfig` built from `Settings::default()`: every weapon enabled, PICK, both human. The
initial picks are **the launched loadout inverted through `weap_order`**, so the default match offers
DART plus the fixture's defaults, not five copies of the first weapon in alphabetical order (§11 Q7).
`finalize` then puts the chosen loadouts over the scenario's. On this path `enter_game` is not needed:
the scenario already set lives and the pool. The frozen HUD shows the scenario's lives (§5 caveat).
4½d moves the live path to `new_match` → weapon selection → `enter_game`, fed by the loaded setup.

### 7.4 F5 and the match-end restart

Both go through `restart_match` (`main.rs:859-873`). It writes back the current picks (from the running
selection if one is active, else the last finalized picks), reloads tick 0, and starts a **new** weapon
selection from those picks. This is the C++ NEW GAME loop with the menu left out (finding 4), and it
drops the frozen frame and resets `menu_cycles`. The seed stays the scenario's, so restarts remain
deterministic, as F5 is today. Seed sourcing for NEW GAME is 4½d's call (overview LD 6).

### 7.5 Record, replay, Scripted (§11 Q4)

Scripted (the golden loop, wasm `?demo`) and `--replay` are unchanged and have no selection. A
`--live --record` session **skips weapon selection**, taking the C++ skip path
(`rollbackController.cpp:384-395`: `InitWeapons` straight away, no draws). A one-line stderr notice says
so. The reason: the scenario format cannot express the post-selection state (per-worm picks, an advanced
RNG), and making `weapsel` a recording format on the scenario path would add a second settings
language. So recordings stay exact, and `round_trip.rs` / `record_regression.rs` hold unchanged. 4½d
records `MatchConfig` matches as scenario + setup sidecar + `weapsel` lines, and drops the skip.

### 7.6 Browser: `?weapons=`, touch, and the second player

- **`?weapons=` skips selection (§11 Q3, John).** A PR preview exists to reach the thing under test in
  one click (`web_params.rs:1-16`). A preview link with `?weapons=` keeps today's path exactly:
  `apply_weapons` on the tick-0 state, the C++ skip path, no selection draws. Without it, the preview shows
  weapon selection. `?level=` and `?seed=` still apply either way.
- **Touch works as is.** `game::touch::merge` produces the same `ControlState` as the keyboard
  (`touch.rs:28-45`), so the pad moves the cursor and cycles weapons, and Fire runs RANDOMIZE or DONE. The
  12/3 repeat applies to a held pad direction too.
- **The second player on a phone.** On a touch-only device, player 2 has no input, so with two human PICK
  players the phase can never end. Recommendation (§11 Q8): when the page reports a touch device, the
  launch config makes worm 1 a bot (`controller = 1`) with `select_bot_weapons = 2` (KEEP). Player 2 then
  auto-readies. That is also the human-vs-DumbAI setup 4½f's phone mode needs, and until 4½f player 2
  idles in the match as it does today. Keyboard browsers keep two human players.

---

## 8. Step 5 note — what rollback must be able to snapshot

With 4½c the netplay path includes weapon selection, so the Step-5 deferral ("unless the netplay path
includes weapon selection", overview 4½c bullet) is now live. The Rust equivalent of `WeaponSelectSnap`
(`weapsel_snapshot.hpp:21-44`) is **`SimState` plus a `WeaponSelection` clone**, and 4½c designs for that:

- `WeaponSelection` is plain data: `#[derive(Clone, PartialEq, Eq, Debug)]`, no references. It holds the
  picks, cursor and ready flag per player, and the `KeyRepeat` state (`prev` word and seven `u16`
  counters per worm). That is C++'s `players[]`, `local/remote_prev_input` and `*_held_frames`, keyed by
  worm, not local/remote. `enabled_weaps` and `weap_order` are derived and immutable during the phase.
  The menu scroll fields are constant (finding 3) and dropped. `menu_sounds` is per-frame output, not
  state.
- `SimState` carries the RNG (`WeaponSelectSnap::rand`), the worm control words (`worm_control_states`),
  `current_weapon` and the constructor's weapon types. Step 5a's `SimState` snapshot covers them
  unchanged. `Rand: Clone` lands in 4½c.
- `ws_done` is the `process_frame` return value. Step 5 records it per frame, as C++ does
  (`rollbackController.cpp:999-1000`), and finalizes on the **confirmed** done frame
  (`:1009-1010`, `:1070-1071`).
- Still for Step 5 to decide: netplay zeroes controllers before selection
  (`rollbackController.cpp:401`), so there are no bots, and the Rust netplay config must do the same.
  Edge state resets at the phase boundary (`ResetForGamePhase`, `:611-651`); in Rust it disappears with
  the struct at `finalize`. C++ netplay has no release latch, since it is level-based there (§7.2 is a
  local-only presentation of the Local controller). And the level-generation RNG order is still open
  (overview LD 6 correction).

---

## 9. Task outline (the plan details each)

| Task | Deliverable | Gate |
|---|---|---|
| T0 | `Rand: Clone`; `sim::weapsel::weap_order` + the two call sites; unique-names test | unit + re-diff |
| T1 | `sim::weapsel` constructor + refusals (§4.3, §4.6) | unit (draw counts on crafted configs) |
| T2 | `process_frame` + `KeyRepeat` + RANDOMIZE + menu sounds (§4.4, §4.5, §4.8) | unit (ordering, wrap, repeat timing, finding 5) |
| T3 | `finalize`/`init_weapons`; builder split `new_match`/`enter_game`/`weapsel_config`; `build_match` rebuilt on them | unit + `build.rs` tests + re-diff |
| T4 | `weapsel` directive in `scenario::parser` (+ `to_text`) and in `sim_physics_dump`'s parser | parser unit tests |
| T5 | C++: `weapsel_drive.hpp`, `oracle_dump_weapsel` (+ self-check, draws, sounds), CMake target; `sim_physics_dump` settings path | clang-format/tidy; 8-golden regen byte-identical |
| T6 | `gen_slice4_5c.rs` corpus (16 cases), `gen_weapsel_golden.sh`, `gen_sim_slice4_5c_golden.sh`, goldens | generator ledgers + awk gates |
| T7 | **MILESTONE** — `weapsel_golden.rs` (+ witness guard), continuation goldens, handoff equality (§6.6) | oracle-tests |
| T8 | `render`: `draw_rounded_box`, `get_dims`, `menu::draw_item`, `weapsel` draw + palette; self-goldens; `shot --weapsel` | render unit + self-golden |
| T9 | `game`: `MatchFlow` phase, `ReleaseLatch`, wiring, restart + write-back, `--record` skip, `?weapons=` skip, touch P2 rule | `cargo test -p game` (headless phase/latch/restart tests) |
| T10 | Full re-diff (debug), wasm build, smoke run, PNG eyeball, liero-shot §7, PROGRESS + overview corrections | CI commands |

Dependencies: T0 → T1 → T2 → T3. T4 is independent. T5 needs T4. T6 needs T3 and T5. T7 needs T6.
T8 needs T2. T9 needs T3 and T8. T10 is last.

---

## 10. Out of scope (deferrals)

- **The generic `Menu` framework**, the main menu, the `ScreenStack`, Esc to the menu with
  `Focus`/`Unfocus`, the render fade and threading `menu_cycles` from the menu: 4½d.
- **Recording and replaying a selection** (the settings sidecar plus `weapsel` as a recording format) and
  moving the live path to `new_match`: 4½d. 4½c skips selection under `--record` (§7.5).
- **Writing picks to disk** (SAVE SETUP / profile save): 4½d–4½f (the write-back hook is here).
- **The BOT WEAPONS toggle** (`hiddenMenu.cpp:10`, `:33-34`): 4½g. The sim honours every value now.
- **DumbLieroAI.** 4½f. The AI never runs during selection anyway (finding 2).
- **Weapon availability** (`WeaponMenuState`, which refuses zero weapons): 4½e. The sim refuses it
  itself (§4.6).
- **The spectator weapsel screen** (`weapsel.cpp:99-158`) and **gamepad input** (`:234-313`):
  overview deferrals.
- **Rollback of the phase:** Step 5 (§8).

---

## 11. Risks

- **The RNG stream (the central risk, overview §Risks).** One extra or missing draw moves every later
  tick. Mitigations: per-step `draws`, `last` and `next` on every golden line; a corpus that runs the
  loops at 1, 3, 5, 28 and 40 enabled weapons across two seeds; the continuation goldens.
- **Replica fidelity in the dumper.** The repeat loop is copied, not called. Mitigation: the
  `LocalController` self-check (§6.2). If that cannot run headless, a line-by-line review against the
  cited lines.
- **Two C++ repeat semantics (finding 5).** Porting Local is a choice. Netplay C++ uses the other one.
  Pinned by `repeat_edge`, and irrelevant to Rust-vs-Rust Step 5.
- **The live input model.** The latch (§7.2) is new live-only logic at a boundary where a mistake costs
  a spurious tick-0 shot. Headless tests cover arm, release and re-press, and the recorder tap order.
- **Builder refactor.** `build_match` is rebuilt from the split. Its unit tests and the eight
  settings-path goldens must stay green. Re-diff at T3.
- **`weap_order` stability.** C++'s unstable sort and Rust's stable one agree only with unique names.
  Pinned by a TC test (T0), which fails loudly if a future TC adds a duplicate name.
- **Grammar growth.** One directive, oracle-only, absent everywhere today. Both parsers change together,
  with parser tests for arity, duplicates, the settings-only rule and the end-frame invariant.
- **Preview UX.** A phone preview stalls on player 2 without the §7.6 rule, and a keyboard preview now
  needs two DONE presses. The first is mitigated by the touch rule, the second by `?weapons=` staying a
  one-click path.

---

## 12. Test strategy

1. **Unit (`sim::weapsel`).** Crafted configurations assert exact draw counts (`Rand::draws()`) for
   every loop-entry combination. Per-frame ordering (finding 6), wrap, repeat timing at 12/15/18 and the
   finding-5 edge, the ready skip, held RANDOMIZE, sound ids and order, the refusals, and that
   `finalize` leaves control words at 0.
2. **Differential (C++).** The 16-case weapsel golden, line for line, plus the two 12-column
   continuation goldens.
3. **Equivalence.** Handoff equality (§6.6) against `build_match`, including a randomized property run.
4. **`game`, headless.** `MatchFlow` phase transitions and fade; `ReleaseLatch`; restart write-back;
   `--record` skip (`round_trip.rs` and `record_regression.rs` unchanged); `?weapons=` skip in
   `web_params` tests; the touch rule.
5. **Presentation.** Three `render` self-goldens and the PNG eyeball.
6. **Standing.** Full re-diff (debug), the 8-golden dumper regeneration, the wasm build, and a native
   smoke run (bare `cargo run -p game`: pick, DONE ×2, play).

---

## 13. Open questions (with recommendations)

1. **Rendering scope (§5): pull the pixel-exact subset forward, or ship a placeholder draw?** *(decision
   for John)* **Recommendation: pull it forward.** That means `draw_rounded_box`, `get_dims`, the
   `MenuItem::Draw` text arm and the weapsel screen, about 150 lines, all of which 4½d needs anyway.
   The generic `Menu` framework, render fade and spectator view stay in 4½d.
2. **Which C++ key repeat to port?** *(plan can settle)* **Recommendation: `LocalController`'s**
   (`localController.cpp:128-148`), applied to sampled words. It is the single-player controller Rust
   mirrors, and Step 5 runs Rust on both peers. `repeat_edge` pins it.
3. **`?weapons=` in the preview: skip selection, or pre-fill it?** *(decision for John)*
   **Recommendation: skip.** It uses the C++ skip path, keeps preview links one-click and stays
   byte-identical to today. A bare preview URL shows the selection screen.
4. **`--live --record` in 4½c: skip selection, or pull 4½d's sidecar recording forward?** *(plan can
   settle)* **Recommendation: skip**, with a stderr notice, until 4½d records `MatchConfig` matches. The
   scenario format cannot hold the post-selection state, and 4½a already assigned recording to 4½d.
5. **A release latch at both phase boundaries?** *(plan can settle)* **Recommendation: yes.** It
   reproduces C++'s edge behaviour: a held DONE key does not fire on tick 0, and a key held from before
   the phase does not count as a press. It is live-only and sits before the recorder tap.
6. **Self-check the dumper's replica against a real `LocalController`?** *(plan can settle)*
   **Recommendation: yes, if `LocalController` constructs and processes headless** (with a
   `NullSoundPlayer` and `recordReplays = false`). Otherwise the replica stands alone, reviewed line by
   line.
7. **The live phase's initial picks in 4½c: the launched loadout, or `Settings::default()`'s `[1;5]`?**
   *(plan can settle)* **Recommendation: the launched loadout, inverted through `weap_order`**, so the
   default match opens on DART rather than five copies of one weapon. 4½d replaces this with the loaded
   setup's picks.
8. **A touch-only browser: auto-ready player 2?** *(plan can settle; John may overrule on UX)*
   **Recommendation: yes.** Make worm 1 `controller = 1` with `select_bot_weapons = 2` (KEEP) when the
   page reports a touch device. That is the phone-mode config 4½f wants, and without it a phone preview
   can never leave weapon selection.
9. **Continuation goldens as well as the handoff-equality test?** *(plan can settle)*
   **Recommendation: both.** Two goldens and about 30 lines in the settings path buy an end-to-end C++
   proof. The equality test localises a failure to the handoff.
