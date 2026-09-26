# Step 4½, Slice 4½a-1 — `MatchConfig` builder, sim completion, settings-driven goldens: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** Configure a Liero match from a C++-schema setup file and prove the resulting simulation bit-exact against C++ — a `Settings`/`WormSettings`/`MatchConfig` model, the C++ TOML *reader*, a `MatchConfig → SimState` builder, the sim rules a real configured match reaches (`CorrectShadow`, the Scales death/respawn rules, `DoHealing`, the GameOfTag guard, `IsGameOver`), a `MatchFlow` for the 180-frame post-mortem, the `sound_hooks` live-bug fix, and the level-palette rule (`load_powerlevel_palette`) that `scenario::load` gets wrong.

**Architecture:** Model, reader and builder live in the Bevy-free `scenario` crate (`settings.rs`, `settings_toml.rs`, `build.rs`); the sim completions live in `sim` (`shadow.rs`, `game_over.rs`, edits in `state.rs`/`bonus.rs` and the seven `CorrectShadow` sites); `MatchFlow` is a `game` lib module. The oracle is ONE new optional scenario directive, `settings <file>`, which the C++ dumper feeds to the real `Settings::FromToml` and the Rust test feeds to the new reader + `build_match`; the dumper then emits a 12th column, `Game::IsGameOver()`. Four settings-driven goldens (`defaults` from the shipped `liero.cfg`, plus generated `killemall`/`scales`/`gametag`) vary every sim-reaching field; the builder additionally reproduces the existing `sim_slice6_fuzz5` golden. The builder takes a READY `LevelData`: level preparation is 4½b's `sim::levelgen::generate_from_settings` (parallel slice), composed with `build_match` in 4½d; `sim::shadow::correct_shadow`'s signature (T3) is the contract 4½b's T9 checks against its dig-stage oracle.

**Tech Stack:** Rust 2021 (`scenario`, `sim`, `oracle-tests`) and 2024 (`game`); the `toml` 0.8 crate (already in `Cargo.lock` via `assets`); C++ (`src/tools/oracle_dump/sim_physics_dump.cpp`, preset `macos-arm64`, clang-format 22).

**Spec:** `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` (cited **design §N**). Its §0 splits 4½a; this plan is **4½a-1** only. 4½a-2 (TOML writer, byte gate, `UpdateHash`, storage, HUD fix) is outlined in design §9.2 and gets its own plan.

## Global Constraints

- Worktree `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5`, branch `liero-rs-step-4-5`. Never `cd`; use `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 …` and absolute paths. Every cargo command passes `--manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml`.
- Bash hygiene: one simple command per call; no `&&`, `;`, `$VAR`, heredocs, `>`/`>>` redirection, `find -exec`. Create and edit files with the editor tools. No sub-subagents.
- Determinism: no floats, no wall-clock, no `HashMap` iteration in `sim`; C++ `static_cast` integer narrowing is Rust `as`; C++ `PalIdx` (u8) arithmetic is `wrapping_add`/`wrapping_sub`.
- `SimState::new`'s signature does NOT change; every new setting is a post-`new` field assignment (design LD 5).
- The scenario text format is frozen: the only grammar addition is the optional `settings <file>` directive (absent ⇒ `None`, old behaviour); `scenario::load` refuses a scenario that carries it; every existing scenario file parses to the identical value.
- All prior goldens stay byte-identical: `git -C … status --porcelain -- rust/oracle-tests/golden` may show only NEW `sim_slice4_5a_*` files. The dumper's absent-directive path must regenerate existing goldens byte-identically (T7).
- `sim-core` stays dependency-free; `sim` gains no dependency; Bevy stays confined to `game`.
- C++ changes are confined to `src/tools/oracle_dump/sim_physics_dump.cpp`; it must pass `clang-format --dry-run -Werror` (clang-format 22) on the whole file.
- Holdazone (`game_mode 2`) is refused with `BuildError::HoldazoneUnsupported` (Rust) / `exit 1` (dumper) — never a runtime panic from a configuration choice.
- Weapons with deferred sim branches (RIFLE, WINCHESTER, LASER, GAUSS GUN, MISSILE — design §1.3 finding 8) never appear in a golden loadout and are `weap_table = 2` in every generated setup.
- rustfmt only files this plan CREATES (`rustfmt --edition 2021 <abs file>`, `--edition 2024` for `game`); never run rustfmt on an existing file or on a `lib.rs`/`main.rs` (it recurses) — hand-format edits to the surrounding style.
- Commits use the globally-configured identity (john.alm.martensson@pm.me); do not override it. Every commit message ends with the trailer `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>` (pass it as a second `-m`). Never write "Generated with Claude Code" anywhere.
- Do NOT push and do NOT open a PR — the controller owns push + PR.

## Model tiers

- **[Opus]:** T2 (reader semantics), T3, T4 (sim ports), T6 (builder + first C++ cross-check), T7 (C++ dumper), T8, T9 (goldens + milestone), T11 (broad review) — and every reviewer.
- **[Sonnet]:** T0, T1, T5, T10 (mechanical, fully specified).

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `rust/scenario/src/loader.rs` (modify) | T0, T6, T7 | assign `sound_hooks`; `load_sprites` → `pub(crate)`; factor `scene_data`; refuse `settings` scenarios |
| `rust/scenario/src/settings.rs` (create) | T1 | `Settings`, `WormSettings`, `MatchConfig`, C++ constants + defaults |
| `rust/scenario/src/settings_toml.rs` (create) | T2 | `TomlError`, `settings_from_toml`, `read_settings_into`, `load_profile` |
| `rust/scenario/src/build.rs` (create) | T6 | `BuildError`, `validate`, `build_match` |
| `rust/scenario/src/lib.rs` (modify) | T1, T2, T6 | module declarations |
| `rust/scenario/Cargo.toml` (modify) | T2 | `toml = "0.8"` |
| `rust/scenario/src/parser.rs` (modify) | T7 | the `settings` directive |
| `rust/sim/src/shadow.rs` (create) | T3 | `correct_shadow` + the per-tick enable flag |
| `rust/sim/src/game_over.rs` (create) | T5 | `is_game_over` |
| `rust/sim/src/lib.rs` (modify) | T3, T5 | module declarations |
| `rust/sim/src/state.rs` (modify) | T3, T4 | `MAT_SEE_SHADOW`, `SimState.shadow`, frame flag, respawn site; `worm_death`/`do_respawning` mode rules; `do_healing` |
| `rust/sim/src/{nobject,sobject,weapon,control}.rs` (modify) | T3 | the six other `CorrectShadow` sites |
| `rust/sim/src/bonus.rs` (modify) | T4 | pickup heal → `do_healing` |
| `rust/sim/src/{hash,wide_checksum}.rs` (modify) | T3 | test-literal `shadow: false` |
| `src/tools/oracle_dump/sim_physics_dump.cpp` (modify) | T7 | `settings` path + 12th column |
| `rust/oracle-tests/gen_sim_slice4_5a_golden.sh` (create) | T7, T8 | C++ golden generation |
| `rust/oracle-tests/golden/sim_slice4_5a_defaults{_scenario.txt,.txt}` (create) | T7 | the shipped-defaults smoke |
| `rust/oracle-tests/examples/gen_slice4_5a.rs` (create) | T8 | setup-sidecar writer, seed scanner, scenario writer |
| `rust/oracle-tests/golden/sim_slice4_5a_{killemall,scales,gametag}{_setup.cfg,_scenario.txt,.txt}` (create) | T8 | the generated goldens |
| `rust/oracle-tests/tests/sim_slice4_5a_builder_golden.rs` (create) | T6 | builder reproduces `sim_slice6_fuzz5` |
| `rust/oracle-tests/tests/sim_slice4_5a_settings_golden.rs` (create) | T9 | the MILESTONE difftest |
| `rust/game/src/match_flow.rs` (create), `rust/game/src/lib.rs`, `rust/game/src/main.rs` (modify) | T10 | `MatchFlow` + live wiring |
| `docs/superpowers/liero-rs-PROGRESS.md`, the overview (modify) | T11 | status |

## Task dependency map

```
T0 ─────────────────────────────────────────────┐
T1 ─> T2 ──────────────┐                        │
T3 ─┐                  ├─> T6 ─┐                │
T4 ─┼──────────────────┘       ├─> T8 ─> T9 ─> T11
T5 ─┴─> T10                    │
T7 (C++ + parser) ─────────────┘
```

---

### Task 0: `scenario::load` assigns the TC `sound_hooks` (live bug)  [Sonnet]

**Files:**
- Modify: `rust/scenario/src/loader.rs:182` (after `state.game_mode = …`) and its `#[cfg(test)] mod tests` (`:220-256`)

**Interfaces:**
- Consumes: `assets::tc::TcConfig::sound_hooks` (`assets/src/tc.rs:297-306`, `:467-476`), `SimState.sound_hooks` (`sim/src/state.rs:1226`).
- Produces: `scenario::load` returns a state whose `sound_hooks == tc.sound_hooks`.

Why: `SoundHooks::default()` is all zeros, so every hook sound (bump, reloaded, alive, rope throw) plays sample 0 ("shotgun") in the live game (design §8). Unhashed ⇒ every golden unchanged.

- [ ] **Step 1: Write the failing test** — append inside `mod tests` in `loader.rs`:

```rust
    #[test]
    fn load_assigns_the_tc_sound_hooks() {
        let scenario = Scenario::parse(SAMPLE).expect("scenario parses");
        let loaded = load(Path::new(TC_ROOT), &scenario);
        let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
        let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
        assert_eq!(
            loaded.state.sound_hooks, tc.sound_hooks,
            "state.sound_hooks must be the TC's resolved hook indices"
        );
        // Non-vacuity: the all-zero default was the bug (every hook played sample 0,
        // "shotgun"); the real TC's `bump` is sound index 14.
        assert_ne!(loaded.state.sound_hooks.Bump, 0);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario load_assigns_the_tc_sound_hooks`
Expected: FAIL — `assertion `left == right` failed: state.sound_hooks must be the TC's resolved hook indices` (left all zeros).

- [ ] **Step 3: Implement** — in `load`, directly after `state.game_mode = scenario.game_mode as u32;` insert:

```rust
    // Step 4½a-1 (live bug, design §8): the worm-hook sound indices. Left at
    // `SoundHooks::default()` every hook played sample 0. Unhashed (sound never
    // enters the hash), so every golden stays byte-identical.
    state.sound_hooks = tc.sound_hooks.clone();
```

- [ ] **Step 4: Run the test, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario load_assigns_the_tc_sound_hooks` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (all goldens).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/loader.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-1): scenario::load assigns the TC sound_hooks (live bug)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 1: The `Settings` / `WormSettings` / `MatchConfig` model  [Sonnet]

**Files:**
- Create: `rust/scenario/src/settings.rs`
- Modify: `rust/scenario/src/lib.rs` (add `pub mod settings;` after `pub mod parser;`)

**Interfaces:**
- Produces (used by T2, T6, T8, T9):
  - consts `SELECTABLE_WEAPONS: usize = 5`, `WEAP_TABLE_LEN: usize = 40`, `NUM_WORM_SETTINGS: usize = 3`, `NETWORK_PLAYER_IDX: usize = 2`, `CONFIG_VERSION: i32 = 6`, `MAX_CONTROL: usize = 7`, `MAX_CONTROL_EX: usize = 8`, `GM_KILL_EM_ALL/GM_GAME_OF_TAG/GM_HOLDAZONE/GM_SCALES_OF_JUSTICE: u32 = 0/1/2/3`, `DEFAULT_GAMEPAD_CONTROLS: [u32; 8]`.
  - `pub struct WormSettings { pub health: i32, pub controller: u32, pub controls: [u32; 7], pub controls_ex: [u32; 8], pub gamepad_controls: [u32; 8], pub input_device: u32, pub gamepad_name: String, pub gamepad_serial: String, pub weapons: [u32; 5], pub name: String, pub rgb: [i32; 3], pub random_name: bool, pub color: i32 }` — `Clone, Debug, PartialEq, Eq, Default`.
  - `pub struct Settings { … every field listed in Step 3 …, pub worm_settings: [WormSettings; 3] }` — `Clone, Debug, PartialEq, Eq, Default`.
  - `pub struct MatchConfig { pub settings: Settings, pub seed: u32 }` — `Clone, Debug, PartialEq, Eq`.

Why: design §2. Names are the C++ member names, widths the C++ widths (so T2's `as` casts are the C++ `static_cast`s), defaults the C++ constructors.

- [ ] **Step 1: Write the failing tests** — create `rust/scenario/src/settings.rs` containing ONLY this test module for now, and add `pub mod settings;` to `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worm_settings_default_is_the_cpp_constructor() {
        // worm.hpp:61-66 (extensions) + :86-95, :104-112; worm.cpp:21-32 (gamepad).
        let ws = WormSettings::default();
        assert_eq!(ws.health, 100);
        assert_eq!(ws.controller, 0);
        assert_eq!(ws.controls, [0; 7]);
        assert_eq!(ws.controls_ex, [0; 8]);
        assert_eq!(ws.gamepad_controls, [11, 12, 13, 14, 110, 10, 0, 9]);
        assert_eq!(ws.input_device, 0);
        assert!(ws.gamepad_name.is_empty() && ws.gamepad_serial.is_empty() && ws.name.is_empty());
        assert_eq!(ws.weapons, [1; 5]);
        assert_eq!(ws.rgb, [104, 104, 248], "bare WormSettings() blue is 248, not 252");
        assert!(ws.random_name);
        assert_eq!(ws.color, 0);
    }

    #[test]
    fn settings_default_is_the_cpp_constructor() {
        // settings.hpp:16-28, :34-45, :68-90 in-class initialisers; settings.cpp:23-60.
        let s = Settings::default();
        assert!(s.record_replays && s.load_powerlevel_palette && !s.ai_traces);
        assert_eq!((s.ai_frames, s.ai_mutations, s.ai_parallels), (140, 2, 3));
        assert_eq!(s.zone_timeout, 30);
        assert_eq!(s.select_bot_weapons, 1, "uint32_t select_bot_weapons{true}");
        assert!(!s.allow_viewing_spawn_point);
        assert_eq!(s.tc, "openliero");
        assert!(!s.fullscreen && !s.single_screen_replay && !s.spectator_window && !s.modern_colors);
        assert_eq!(s.blood_particle_max, 700);
        assert_eq!(s.max_spectator_render_height, 1080);
        assert_eq!(s.weap_table, [0; WEAP_TABLE_LEN]);
        assert_eq!((s.max_bonuses, s.blood, s.time_to_lose, s.flags_to_win), (4, 100, 600, 20));
        assert_eq!(s.game_mode, GM_KILL_EM_ALL);
        assert!(s.shadow && s.load_change && !s.names_on_bonuses && !s.regenerate_level);
        assert_eq!((s.lives, s.loading_time), (15, 100));
        assert!(s.random_level && s.level_file.is_empty() && s.map && s.screen_sync);
        assert_eq!((s.bonus_timeout, s.input_delay), (0, 1));
        assert_eq!((s.random_map_width, s.random_map_height), (504, 350));

        let [p1, p2, net] = &s.worm_settings;
        assert_eq!((p1.color, p2.color, net.color), (32, 41, 32));
        assert_eq!(p1.controls, [0x13, 0x21, 0x20, 0x22, 0x1D, 0x2A, 0x38]);
        assert_eq!(p1.controls_ex, [0x13, 0x21, 0x20, 0x22, 0x1D, 0x2A, 0x38, 0]);
        assert_eq!(p2.controls, [0xA0, 0xA8, 0xA3, 0xA5, 0x75, 0x90, 0x36]);
        assert_eq!(p2.controls_ex, [0xA0, 0xA8, 0xA3, 0xA5, 0x75, 0x90, 0x36, 0]);
        assert_eq!(net.controls, p1.controls, "network player clones the left controls");
        assert_eq!(net.controls_ex, p1.controls_ex);
        assert_eq!(p1.rgb, [104, 104, 252]);
        assert_eq!(p2.rgb, [60, 172, 60]);
        assert_eq!(net.rgb, [104, 104, 252]);
        for ws in &s.worm_settings {
            assert_eq!(ws.health, 100);
            assert_eq!(ws.weapons, [1; 5]);
            assert_eq!(ws.gamepad_controls, DEFAULT_GAMEPAD_CONTROLS);
        }
    }

    #[test]
    fn game_mode_and_layout_constants_match_cpp() {
        assert_eq!((GM_KILL_EM_ALL, GM_GAME_OF_TAG, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE), (0, 1, 2, 3));
        assert_eq!((SELECTABLE_WEAPONS, WEAP_TABLE_LEN, NUM_WORM_SETTINGS), (5, 40, 3));
        assert_eq!((NETWORK_PLAYER_IDX, CONFIG_VERSION, MAX_CONTROL, MAX_CONTROL_EX), (2, 6, 7, 8));
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings::tests`
Expected: FAIL to compile — `cannot find type WormSettings in this scope` (and the consts).

- [ ] **Step 3: Implement** — put this ABOVE the test module in `settings.rs`:

```rust
//! Step 4½a-1 — the C++ `Settings` / `WormSettings` model (`settings.hpp:10-102`,
//! `worm.hpp:44-118`) with the C++ member names and defaults (`settings.cpp:17-60`,
//! `worm.hpp:61-95`, `worm.cpp:21-32`), plus the [`MatchConfig`] the builder consumes
//! (design §2). Integer widths are the C++ widths (`int`/`int32_t` -> `i32`,
//! `uint32_t` -> `u32`) so the reader's `static_cast` truncation
//! (`toml_archive.hpp:224-235`) is a plain `as`. Pure data — no I/O here.

/// `Settings::kSelectableWeapons` (`settings.hpp:53`) == C++ `NUM_WEAPONS` (`worm.hpp:13`).
pub const SELECTABLE_WEAPONS: usize = 5;
/// `Settings::weap_table[40]` (`settings.hpp:68`), indexed by weapon index (not `weap_order`).
pub const WEAP_TABLE_LEN: usize = 40;
/// `Settings::kNumWormSettings` (`settings.hpp:92`): 0 = left, 1 = right, 2 = network.
pub const NUM_WORM_SETTINGS: usize = 3;
/// `Settings::kNetworkPlayerIdx` (`settings.hpp:93`).
pub const NETWORK_PLAYER_IDX: usize = 2;
/// `Settings::kConfigVersion` (`settings.hpp:98`) — what `[settings].version` is saved as.
pub const CONFIG_VERSION: i32 = 6;
/// `WormSettingsExtensions::kMaxControl` (`worm.hpp:54`): the seven classic controls.
pub const MAX_CONTROL: usize = 7;
/// `WormSettingsExtensions::kMaxControlEx` (`worm.hpp:55`): the seven + DIG.
pub const MAX_CONTROL_EX: usize = 8;

/// `Settings::GameModes` (`settings.hpp:51`).
pub const GM_KILL_EM_ALL: u32 = 0;
pub const GM_GAME_OF_TAG: u32 = 1;
pub const GM_HOLDAZONE: u32 = 2;
pub const GM_SCALES_OF_JUSTICE: u32 = 3;

/// `InitDefaultGamepadControls` (`worm.cpp:21-32`) as SDL3 enum values: DPAD_UP 11,
/// DPAD_DOWN 12, DPAD_LEFT 13, DPAD_RIGHT 14, fire = `GamepadAxisPositive(RIGHT_TRIGGER
/// = 5)` = 100 + 5*2 = 110, change = RIGHT_SHOULDER 10, jump = SOUTH 0, dig =
/// LEFT_SHOULDER 9 — the `gamepadControls` array every shipped file carries.
pub const DEFAULT_GAMEPAD_CONTROLS: [u32; MAX_CONTROL_EX] = [11, 12, 13, 14, 110, 10, 0, 9];

/// Default DOS scancodes (`settings.cpp:36-37`): left player, right player.
const DEF_CONTROLS: [[u32; MAX_CONTROL]; 2] = [
    [0x13, 0x21, 0x20, 0x22, 0x1D, 0x2A, 0x38],
    [0xA0, 0xA8, 0xA3, 0xA5, 0x75, 0x90, 0x36],
];
/// Default 8-bit RGB (`settings.cpp:39`): left player, right player.
const DEF_RGB: [[i32; 3]; 2] = [[104, 104, 252], [60, 172, 60]];

/// C++ `WormSettings` (`worm.hpp:85-118`) + its `WormSettingsExtensions` base
/// (`:44-83`). `profile_node` (a filesystem handle) and `hash` (a cache) are runtime
/// state, not data, and are not modelled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WormSettings {
    pub health: i32,
    /// 0 human, 1 DumbLieroAI, 2 FollowAI (`localController.cpp:19-27`).
    pub controller: u32,
    /// DOS scancodes for Up/Down/Left/Right/Fire/Change/Jump.
    pub controls: [u32; MAX_CONTROL],
    /// `controls` + DIG.
    pub controls_ex: [u32; MAX_CONTROL_EX],
    /// 0..99 SDL button, `100 + axis*2 (+1)` axis (`worm.hpp:72-77`).
    pub gamepad_controls: [u32; MAX_CONTROL_EX],
    /// 0 keyboard, 1 gamepad 0, 2 gamepad 1, … (`worm.hpp:58-59`).
    pub input_device: u32,
    pub gamepad_name: String,
    pub gamepad_serial: String,
    /// 1-based `weap_order` indices (`worm.cpp:704`).
    pub weapons: [u32; SELECTABLE_WEAPONS],
    pub name: String,
    /// 0..255 per channel (`worm.hpp:109`).
    pub rgb: [i32; 3],
    pub random_name: bool,
    pub color: i32,
}

impl Default for WormSettings {
    /// `WormSettings()` over `WormSettingsExtensions()` (`worm.hpp:61-66`, `:86-95`).
    fn default() -> Self {
        WormSettings {
            health: 100,
            controller: 0,
            controls: [0; MAX_CONTROL],
            controls_ex: [0; MAX_CONTROL_EX],
            gamepad_controls: DEFAULT_GAMEPAD_CONTROLS,
            input_device: 0,
            gamepad_name: String::new(),
            gamepad_serial: String::new(),
            weapons: [1; SELECTABLE_WEAPONS],
            name: String::new(),
            rgb: [104, 104, 248],
            random_name: true,
            color: 0,
        }
    }
}

/// C++ `Settings` (`settings.hpp:50-102`) = `GameplayExtensions` (`:11-28`) +
/// `AppSettings` (`:31-46`) + its own fields, in C++ declaration order. `hash` is a
/// cache and not modelled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    // --- GameplayExtensions (hashed + replayed) ---
    pub record_replays: bool,
    pub load_powerlevel_palette: bool,
    pub ai_frames: i32,
    pub ai_mutations: i32,
    pub ai_traces: bool,
    pub ai_parallels: i32,
    pub zone_timeout: i32,
    pub select_bot_weapons: u32,
    pub allow_viewing_spawn_point: bool,
    pub tc: String,
    // --- AppSettings (not hashed) ---
    pub fullscreen: bool,
    pub single_screen_replay: bool,
    pub spectator_window: bool,
    pub blood_particle_max: i32,
    pub modern_colors: bool,
    pub max_spectator_render_height: i32,
    // --- Settings ---
    pub weap_table: [u32; WEAP_TABLE_LEN],
    pub max_bonuses: i32,
    pub blood: i32,
    pub time_to_lose: i32,
    pub flags_to_win: i32,
    pub game_mode: u32,
    pub shadow: bool,
    pub load_change: bool,
    pub names_on_bonuses: bool,
    pub regenerate_level: bool,
    pub lives: i32,
    pub loading_time: i32,
    pub random_level: bool,
    pub level_file: String,
    pub map: bool,
    pub screen_sync: bool,
    pub bonus_timeout: i32,
    pub input_delay: i32,
    pub random_map_width: i32,
    pub random_map_height: i32,
    /// 0 = left, 1 = right, 2 = network (`settings.hpp:92-99`).
    pub worm_settings: [WormSettings; NUM_WORM_SETTINGS],
}

impl Default for Settings {
    /// `Settings::Settings()` (`settings.cpp:23-60`) over the in-class initialisers.
    fn default() -> Self {
        let mut worm_settings = [
            WormSettings::default(),
            WormSettings::default(),
            WormSettings::default(),
        ];
        worm_settings[0].color = 32;
        worm_settings[1].color = 41;
        worm_settings[2].color = 32;
        for (i, ws) in worm_settings.iter_mut().take(2).enumerate() {
            ws.controls = DEF_CONTROLS[i];
            ws.controls_ex[..MAX_CONTROL].copy_from_slice(&DEF_CONTROLS[i]);
            ws.rgb = DEF_RGB[i];
        }
        // The network player defaults to the left player's controls and colour (:52-59).
        worm_settings[2].controls = DEF_CONTROLS[0];
        worm_settings[2].controls_ex[..MAX_CONTROL].copy_from_slice(&DEF_CONTROLS[0]);
        worm_settings[2].rgb = DEF_RGB[0];

        Settings {
            record_replays: true,
            load_powerlevel_palette: true,
            ai_frames: 70 * 2,
            ai_mutations: 2,
            ai_traces: false,
            ai_parallels: 3,
            zone_timeout: 30,
            select_bot_weapons: 1,
            allow_viewing_spawn_point: false,
            tc: "openliero".to_string(),
            fullscreen: false,
            single_screen_replay: false,
            spectator_window: false,
            blood_particle_max: 700,
            modern_colors: false,
            max_spectator_render_height: 1080,
            weap_table: [0; WEAP_TABLE_LEN],
            max_bonuses: 4,
            blood: 100,
            time_to_lose: 600,
            flags_to_win: 20,
            game_mode: GM_KILL_EM_ALL,
            shadow: true,
            load_change: true,
            names_on_bonuses: false,
            regenerate_level: false,
            lives: 15,
            loading_time: 100,
            random_level: true,
            level_file: String::new(),
            map: true,
            screen_sync: true,
            bonus_timeout: 0,
            input_delay: 1,
            random_map_width: 504,
            random_map_height: 350,
            worm_settings,
        }
    }
}

/// A configured match: the settings plus the match seed. The seed seeds the sim
/// `Rand` (overview LD 6); C++ single-player seeds it implicitly, so it is not a
/// `Settings` field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchConfig {
    pub settings: Settings,
    pub seed: u32,
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings::tests`
Expected: PASS (3 tests).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/settings.rs` — Expected: no output (if it prints a diff, run it without `--check` and re-run the tests).

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/settings.rs rust/scenario/src/lib.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-1): Settings/WormSettings/MatchConfig model with C++ names and defaults" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: The C++ setup/profile TOML reader  [Opus]

**Files:**
- Create: `rust/scenario/src/settings_toml.rs`
- Modify: `rust/scenario/src/lib.rs` (add `pub mod settings_toml;` after `pub mod settings;`), `rust/scenario/Cargo.toml` (add `toml = "0.8"` under `[dependencies]`)
- Test: in-module tests reading `data/Setups/*.cfg` and `data/Profiles/*.toml`

**Interfaces:**
- Consumes: T1's `Settings`, `WormSettings`.
- Produces (used by T8, T9, and 4½a-2):
  - `pub struct TomlError(pub String)` — `Clone, Debug, PartialEq, Eq`, `Display`, `std::error::Error`.
  - `pub const WORM_TABLE_NAMES: [&str; 3] = ["player1", "player2", "network_player"];`
  - `pub fn settings_from_toml(text: &str) -> Result<Settings, TomlError>` — `Settings::FromToml` over `Settings::default()`.
  - `pub fn read_settings_into(text: &str, s: &mut Settings) -> Result<(), TomlError>` — `FromToml` over an existing value (untouched on `Err`).
  - `pub fn load_profile(text: &str, ws: &mut WormSettings) -> Result<(), TomlError>` — `LoadProfile`: root-level keys, `color` preserved, untouched on `Err`.

Why: design §3.2. The `toml` crate parses; the semantics are cereal's `TomlInputArchive` by hand (missing/wrong-typed keys keep the prior value, `static_cast` truncation, positional arrays, the `rgbDepth` 6-bit expansion). The strongest check is that the shipped legacy `liero.cfg` reads as exactly `Settings::default()`.

- [ ] **Step 1: Write the failing tests** — create `settings_toml.rs` with only this module, add the `lib.rs` line and the `toml` dependency:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, WormSettings};

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

    fn shipped(rel: &str) -> String {
        std::fs::read_to_string(format!("{DATA}/{rel}")).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

    #[test]
    fn shipped_liero_cfg_reads_as_exactly_the_cpp_defaults() {
        // A legacy v5 file: no rgbDepth (6-bit 26,26,63 / 15,43,15 expand to the default
        // 104,104,252 / 60,172,60), no maxSpectatorRenderHeight (keeps 1080).
        let s = settings_from_toml(&shipped("Setups/liero.cfg")).expect("liero.cfg parses");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn shipped_orbmit_cfg_differs_from_the_defaults_in_four_fields() {
        let s = settings_from_toml(&shipped("Setups/orbmit.cfg")).expect("orbmit.cfg parses");
        let mut want = Settings::default();
        want.blood = 25;
        want.lives = 9;
        want.loading_time = 20;
        want.max_bonuses = 0;
        assert_eq!(s, want);
    }

    #[test]
    fn shipped_ai_profile_loads_expands_rgb_and_keeps_the_colour() {
        let mut ws = Settings::default().worm_settings[0].clone(); // colour 32
        load_profile(&shipped("Profiles/AI (L).toml"), &mut ws).expect("profile parses");
        assert_eq!(ws.name, "AI Joe");
        assert_eq!(ws.health, 100);
        assert_eq!(ws.controller, 2);
        assert!(!ws.random_name);
        assert_eq!(ws.color, 32, "LoadProfile restores the pre-load colour (worm.cpp:94)");
        assert_eq!(ws.input_device, 0);
        assert_eq!(ws.gamepad_name, "");
        assert_eq!(ws.rgb, [80, 80, 160], "no rgbDepth => 6-bit: (v & 63) << 2");
        assert_eq!(ws.weapons, [19, 31, 40, 29, 36]);
        assert_eq!(ws.controls, [17, 31, 30, 32, 20, 21, 22]);
        assert_eq!(ws.controls_ex, [17, 31, 30, 32, 20, 21, 22, 0]);
        assert_eq!(ws.gamepad_controls, [11, 12, 13, 14, 110, 10, 0, 9]);
    }

    #[test]
    fn every_shipped_profile_parses() {
        for name in [
            "AI (L)", "AI (R)", "Joystick0", "Joystick1", "Lefty (L)", "Lefty (R)", "Righty (L)",
            "Righty (R)",
        ] {
            let mut ws = WormSettings::default();
            load_profile(&shipped(&format!("Profiles/{name}.toml")), &mut ws)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!ws.name.is_empty(), "{name}: every shipped profile is named");
        }
        let mut joy = WormSettings::default();
        load_profile(&shipped("Profiles/Joystick1.toml"), &mut joy).unwrap();
        assert_eq!(joy.input_device, 1);
        assert_eq!(joy.rgb, [104, 104, 252]);
        assert_eq!(joy.controls, [160, 33, 32, 34, 29, 42, 56]);
    }

    #[test]
    fn missing_and_wrongly_typed_keys_keep_their_prior_values() {
        let text = shipped("Setups/liero.cfg")
            .replace("lives = 15", "lives = \"nine\"")
            .replace("shadow = true", "shadow = 1")
            .replace("blood = 100", "blood = 42")
            .replace("zoneTimeout = 30\n", "");
        let s = settings_from_toml(&text).unwrap();
        let mut want = Settings::default();
        want.blood = 42;
        assert_eq!(s, want);
    }

    #[test]
    fn integers_truncate_like_static_cast() {
        // 4294967311 = 2^32 + 15 -> int32 15; -1 -> uint32 0xFFFF_FFFF.
        let text = shipped("Setups/liero.cfg")
            .replace("lives = 15", "lives = 4294967311")
            .replace("gameMode = 0", "gameMode = -1");
        let s = settings_from_toml(&text).unwrap();
        assert_eq!(s.lives, 15);
        assert_eq!(s.game_mode, u32::MAX);
    }

    #[test]
    fn arrays_are_positional_short_long_and_typed_per_element() {
        let text = "[settings]\nweapTable = [2, \"x\", 1]\n\
                    [player1]\nrgbDepth = 8\nweapons = [5, 6, 7, 8, 9, 10, 11]\n\
                    [player2]\nrgbDepth = 8\nweapons = [3]\n";
        let s = settings_from_toml(text).unwrap();
        assert_eq!(&s.weap_table[..4], &[2, 0, 1, 0], "\"x\" keeps 0 and the index still advances");
        assert_eq!(s.worm_settings[0].weapons, [5, 6, 7, 8, 9], "extra elements ignored");
        assert_eq!(s.worm_settings[1].weapons, [3, 1, 1, 1, 1], "missing tail kept");
        let t = settings_from_toml("[settings]\nweapTable = 7\n").unwrap();
        assert_eq!(t.weap_table, [0; 40], "a non-array keeps the whole array");
    }

    #[test]
    fn the_rgb_depth_marker_controls_the_expansion_and_values_clamp() {
        let text = "[player1]\nrgbDepth = 8\nrgb = [300, -5, 70]\n\
                    [player2]\nrgbDepth = 7\nrgb = [63, 64, 1]\n\
                    [network_player]\nrgb = [10, 20, 30]\n";
        let s = settings_from_toml(text).unwrap();
        assert_eq!(s.worm_settings[0].rgb, [255, 0, 70], "8-bit: clamp only");
        assert_eq!(s.worm_settings[1].rgb, [252, 0, 4], "rgbDepth 7 < 8 => (v & 63) << 2");
        assert_eq!(s.worm_settings[2].rgb, [40, 80, 120], "absent marker => 6-bit");
    }

    #[test]
    fn a_missing_worm_table_still_expands_its_default_rgb() {
        // C++ quirk: startNode on a missing table gives a null frame, the rgbDepth read is a
        // no-op so rgb_depth stays 6, and the DEFAULT colour is then 6-bit expanded.
        let s = settings_from_toml("[settings]\nlives = 3\n").unwrap();
        assert_eq!(s.lives, 3);
        assert_eq!(s.worm_settings[0].rgb, [160, 160, 240]); // 104&63=40<<2, 252&63=60<<2
        assert_eq!(s.worm_settings[1].rgb, [240, 176, 240]); // 60->240, 172&63=44<<2=176
        assert_eq!(s.worm_settings[2].rgb, [160, 160, 240]);
        let t = settings_from_toml("player1 = 5\n").unwrap();
        assert_eq!(t.worm_settings[0].rgb, [160, 160, 240], "a non-table child = a missing one");
    }

    #[test]
    fn version_is_read_and_discarded() {
        let text = shipped("Setups/liero.cfg").replace("version = 5", "version = 99");
        assert_eq!(settings_from_toml(&text).unwrap(), Settings::default());
    }

    #[test]
    fn a_parse_error_is_an_error_and_leaves_values_untouched() {
        assert!(settings_from_toml("[settings\nlives = 3").is_err());
        let mut s = Settings::default();
        s.lives = 77;
        assert!(read_settings_into("lives = ", &mut s).is_err());
        assert_eq!(s.lives, 77);
        let mut ws = WormSettings::default();
        ws.name = "keep".to_string();
        assert!(load_profile("name = \"x\"\nhealth = ", &mut ws).is_err());
        assert_eq!(ws.name, "keep");
    }

    #[test]
    fn profile_keys_are_read_at_the_root_and_the_colour_is_preserved() {
        let mut ws = WormSettings::default();
        ws.color = 41;
        load_profile("color = 7\nname = 'x'\nrgbDepth = 8\nrgb = [1, 2, 3]\n", &mut ws).unwrap();
        assert_eq!(ws.color, 41);
        assert_eq!(ws.name, "x");
        assert_eq!(ws.rgb, [1, 2, 3]);
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings_toml::tests`
Expected: FAIL to compile — `cannot find function settings_from_toml in this scope`.

- [ ] **Step 3: Implement** — put this ABOVE the test module:

```rust
//! Step 4½a-1 — the C++ setup/profile TOML **reader** (`Settings::FromToml`,
//! `settings.cpp:133-156`; `WormSettings::LoadProfile`, `worm.cpp:73-95`), design §3.2.
//!
//! Parsing is the `toml` 0.8 crate; the SEMANTICS are hand-written to mirror cereal's
//! `TomlInputArchive` (`toml_archive.hpp:158-317`): a missing or wrongly-typed key
//! leaves the prior value, integers narrow like `static_cast`, arrays are positional,
//! a missing/non-table worm table behaves like an empty one, and the `rgbDepth` marker
//! drives the legacy 6-bit expansion (`cereal_types.hpp:288-303`). The writer is 4½a-2.

use toml::{Table, Value};

use crate::settings::{Settings, WormSettings};

/// A TOML parse failure (C++ `TomlParseError`, `toml_archive.hpp:154-156`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TomlError(pub String);

impl std::fmt::Display for TomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TOML parse error: {}", self.0)
    }
}

impl std::error::Error for TomlError {}

/// The worm tables in `Settings::FromToml` order (`settings.cpp:146`).
pub const WORM_TABLE_NAMES: [&str; 3] = ["player1", "player2", "network_player"];

fn parse_root(text: &str) -> Result<Table, TomlError> {
    toml::from_str::<Table>(text).map_err(|e| TomlError(e.to_string()))
}

/// `startNode()` on a missing or non-table child yields a null frame (`Lookup`,
/// `toml_archive.hpp:281-309`): every load under it is a no-op.
fn sub_table<'a>(root: &'a Table, key: &str) -> Option<&'a Table> {
    root.get(key).and_then(Value::as_table)
}

fn get<'a>(t: Option<&'a Table>, key: &str) -> Option<&'a Value> {
    t.and_then(|t| t.get(key))
}

/// `loadValue(bool&)` (`:217-222`): only a boolean node assigns.
fn rd_bool(t: Option<&Table>, key: &str, dst: &mut bool) {
    if let Some(Value::Boolean(v)) = get(t, key) {
        *dst = *v;
    }
}

/// `loadValue(int32_t&)` (`:224-229`): only an integer node assigns, `static_cast`.
fn rd_i32(t: Option<&Table>, key: &str, dst: &mut i32) {
    if let Some(Value::Integer(v)) = get(t, key) {
        *dst = *v as i32;
    }
}

/// `loadValue(uint32_t&)` (`:230-235`).
fn rd_u32(t: Option<&Table>, key: &str, dst: &mut u32) {
    if let Some(Value::Integer(v)) = get(t, key) {
        *dst = *v as u32;
    }
}

/// `loadValue(std::string&)` (`:263-268`).
fn rd_string(t: Option<&Table>, key: &str, dst: &mut String) {
    if let Some(Value::String(v)) = get(t, key) {
        dst.clone_from(v);
    }
}

/// `SerializeArray` on load (`cereal_types.hpp:51-60`): loops all `N` slots; slot `i`
/// assigns only when the array has an integer at `i` (the index advances regardless).
fn rd_array<T: Copy, const N: usize>(
    t: Option<&Table>,
    key: &str,
    dst: &mut [T; N],
    cast: fn(i64) -> T,
) {
    if let Some(Value::Array(a)) = get(t, key) {
        for (i, slot) in dst.iter_mut().enumerate() {
            if let Some(Value::Integer(v)) = a.get(i) {
                *slot = cast(*v);
            }
        }
    }
}

fn as_i32(v: i64) -> i32 {
    v as i32
}

fn as_u32(v: i64) -> u32 {
    v as u32
}

/// `SerializeWormSettingsToml` on load (`cereal_types.hpp:282-308`).
fn read_worm(t: Option<&Table>, ws: &mut WormSettings) {
    rd_string(t, "name", &mut ws.name);
    rd_i32(t, "health", &mut ws.health);
    rd_u32(t, "controller", &mut ws.controller);
    rd_bool(t, "randomName", &mut ws.random_name);
    rd_i32(t, "color", &mut ws.color);
    rd_u32(t, "inputDevice", &mut ws.input_device);
    rd_string(t, "gamepadName", &mut ws.gamepad_name);
    rd_string(t, "gamepadSerial", &mut ws.gamepad_serial);
    // Files without the marker predate 8-bit colours (default 6 on load, :290-294).
    let mut rgb_depth: i32 = 6;
    rd_i32(t, "rgbDepth", &mut rgb_depth);
    rd_array(t, "rgb", &mut ws.rgb, as_i32);
    for v in ws.rgb.iter_mut() {
        if rgb_depth < 8 {
            *v = (*v & 63) << 2;
        }
        *v = (*v).clamp(0, 255);
    }
    rd_array(t, "weapons", &mut ws.weapons, as_u32);
    rd_array(t, "controls", &mut ws.controls, as_u32);
    rd_array(t, "controlsEx", &mut ws.controls_ex, as_u32);
    rd_array(t, "gamepadControls", &mut ws.gamepad_controls, as_u32);
}

/// `Settings::FromToml` over an existing value (`settings.cpp:133-156`). On a parse
/// error `s` is untouched (C++ throws before any assignment).
pub fn read_settings_into(text: &str, s: &mut Settings) -> Result<(), TomlError> {
    let root = parse_root(text)?;
    let st = sub_table(&root, "settings");
    // `version` is read into a local and discarded (:139-140) — a no-op here.
    rd_bool(st, "modernColors", &mut s.modern_colors);
    // SerializeSettingsScalars (cereal_types.hpp:161-194).
    rd_bool(st, "recordReplays", &mut s.record_replays);
    rd_bool(st, "loadPowerlevelPalette", &mut s.load_powerlevel_palette);
    rd_i32(st, "aiFrames", &mut s.ai_frames);
    rd_i32(st, "aiMutations", &mut s.ai_mutations);
    rd_bool(st, "aiTraces", &mut s.ai_traces);
    rd_i32(st, "aiParallels", &mut s.ai_parallels);
    rd_i32(st, "zoneTimeout", &mut s.zone_timeout);
    rd_u32(st, "selectBotWeapons", &mut s.select_bot_weapons);
    rd_bool(st, "allowViewingSpawnPoint", &mut s.allow_viewing_spawn_point);
    rd_string(st, "tc", &mut s.tc);
    rd_bool(st, "fullscreen", &mut s.fullscreen);
    rd_bool(st, "singleScreenReplay", &mut s.single_screen_replay);
    rd_bool(st, "spectatorWindow", &mut s.spectator_window);
    rd_i32(st, "bloodParticleMax", &mut s.blood_particle_max);
    rd_i32(st, "maxBonuses", &mut s.max_bonuses);
    rd_i32(st, "blood", &mut s.blood);
    rd_i32(st, "timeToLose", &mut s.time_to_lose);
    rd_i32(st, "flagsToWin", &mut s.flags_to_win);
    rd_u32(st, "gameMode", &mut s.game_mode);
    rd_bool(st, "shadow", &mut s.shadow);
    rd_bool(st, "loadChange", &mut s.load_change);
    rd_bool(st, "namesOnBonuses", &mut s.names_on_bonuses);
    rd_bool(st, "regenerateLevel", &mut s.regenerate_level);
    rd_i32(st, "lives", &mut s.lives);
    rd_i32(st, "loadingTime", &mut s.loading_time);
    rd_bool(st, "randomLevel", &mut s.random_level);
    rd_string(st, "levelFile", &mut s.level_file);
    rd_bool(st, "map", &mut s.map);
    rd_bool(st, "screenSync", &mut s.screen_sync);
    rd_i32(st, "bonusTimeout", &mut s.bonus_timeout);
    rd_i32(st, "inputDelay", &mut s.input_delay);
    rd_i32(st, "randomMapWidth", &mut s.random_map_width);
    rd_i32(st, "randomMapHeight", &mut s.random_map_height);
    rd_i32(st, "maxSpectatorRenderHeight", &mut s.max_spectator_render_height);
    rd_array(st, "weapTable", &mut s.weap_table, as_u32);
    for (i, name) in WORM_TABLE_NAMES.iter().enumerate() {
        read_worm(sub_table(&root, name), &mut s.worm_settings[i]);
    }
    Ok(())
}

/// `Settings::FromToml` over `Settings::default()` — what `Settings::load` does on a
/// fresh `Settings` (`settings.cpp:62-90`).
pub fn settings_from_toml(text: &str) -> Result<Settings, TomlError> {
    let mut s = Settings::default();
    read_settings_into(text, &mut s)?;
    Ok(s)
}

/// `WormSettings::LoadProfile` (`worm.cpp:73-95`): the profile keys sit at the root;
/// the pre-load `color` is restored; on a parse error `ws` is untouched.
pub fn load_profile(text: &str, ws: &mut WormSettings) -> Result<(), TomlError> {
    let root = parse_root(text)?;
    let old_color = ws.color;
    read_worm(Some(&root), ws);
    ws.color = old_color;
    Ok(())
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings_toml::tests`
Expected: PASS (12 tests).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/settings_toml.rs` — Expected: no output (else format and re-test).
Run: `cargo tree --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario -i toml` — Expected: `toml v0.8.23` (the version already locked; no new lockfile entry beyond the edge).

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/settings_toml.rs rust/scenario/src/lib.rs rust/scenario/Cargo.toml rust/Cargo.lock
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-1): C++ setup/profile TOML reader with TomlInputArchive semantics" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Reviewer (Opus): every `cereal_types.hpp:161-194` / `:282-308` key is read with the C++ width; the 12 tests pin the shipped files and each `TomlInputArchive` edge; no serde derive.

---

### Task 3: `CorrectShadow` port + `SimState.shadow` + the seven sites  [Opus]

**Files:**
- Create: `rust/sim/src/shadow.rs`
- Modify: `rust/sim/src/lib.rs` (add `pub mod shadow;` after `pub mod shake;`)
- Modify: `rust/sim/src/state.rs` — `MAT_SEE_SHADOW` after `MAT_WORM` (`:660-663`); field `shadow` after `time_to_lose` (`:1172-1177`); `shadow: false` in `new` after `time_to_lose: 600,` (`:1390`); `begin_frame` after `crate::shake::begin_frame();` (`:1508`); `end_frame` after `*screen_flash = crate::flash::take_frame();` (`:2276`); respawn site (`:2728-2730`); a test at the end of `mod tests`
- Modify: `rust/sim/src/nobject.rs:476-489` and `:652-664`, `rust/sim/src/sobject.rs:392-405`, `rust/sim/src/weapon.rs:804-814`, `rust/sim/src/control.rs:720-744`
- Modify: `rust/sim/src/hash.rs` (test literal `:215-280`) and `rust/sim/src/wide_checksum.rs` (test literal `:152-217`): add `shadow: false,` after `time_to_lose: 600,`

**Interfaces:**
- Produces (STABLE — 4½b's T9 calls it; design §5.1):
  - `pub fn sim::shadow::correct_shadow(level: &mut sim::state::LevelSim, x1: i32, y1: i32, x2: i32, y2: i32)` — `CorrectShadow(Rect(x1, y1, x2, y2))`, always applied.
  - `pub fn sim::shadow::correct_shadow_if_enabled(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32)` — applies iff the current tick's flag is on.
  - `pub fn sim::shadow::begin_frame(enabled: bool)`, `pub fn sim::shadow::end_frame()`, `pub fn sim::shadow::enabled() -> bool`.
  - `pub const sim::state::MAT_SEE_SHADOW: u8 = 1 << 4;`
  - `pub shadow: bool` on `SimState` (post-`new`, default `false`, unhashed).
- Consumes: `LevelSim` (`state.rs:640-645`), `LevelSim::dirt_rock` (`:714-723`).

Why: design §1.3 finding 1 / §5.1 — `settings->shadow` (C++ default `true`) runs `CorrectShadow` (`blit.cpp:624-639`) after every in-frame crater, rewriting `material_id`. The flag is a per-tick thread-local published from `SimState.shadow` because threading a parameter would ripple through `sobject_create`'s fan-in (precedent: `sound::begin_frame`'s hook indices, `sound.rs:159-176`). Default `false` ⇒ every golden byte-identical.

- [ ] **Step 1: Write the failing tests** — create `rust/sim/src/shadow.rs` with only this module and add `pub mod shadow;` to `sim/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{LevelSim, MAT_ROCK, MAT_SEE_SHADOW};

    const SEE: u8 = 10; // a SeeShadow material
    const ROCK: u8 = 20; // a Rock material (DirtRock)
    const WRAP: u8 = 254; // SeeShadow near the top of the palette: +4 wraps

    fn lvl(w: i32, h: i32) -> LevelSim {
        let mut flags = [0u8; 256];
        flags[SEE as usize] = MAT_SEE_SHADOW;
        flags[WRAP as usize] = MAT_SEE_SHADOW;
        flags[ROCK as usize] = MAT_ROCK;
        LevelSim { width: w, height: h, material_id: vec![0; (w * h) as usize], material_flags: flags }
    }

    fn at(l: &LevelSim, x: i32, y: i32) -> u8 {
        l.material_id[(x + y * l.width) as usize]
    }

    fn put(l: &mut LevelSim, x: i32, y: i32, v: u8) {
        let w = l.width;
        l.material_id[(x + y * w) as usize] = v;
    }

    #[test]
    fn see_shadow_pixel_under_rock_darkens_by_four() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, SEE);
        put(&mut l, 5, 2, ROCK); // (x+3, y-3)
        correct_shadow(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), SEE + 4);
    }

    #[test]
    fn shadow_pixel_without_rock_lightens_by_four_and_with_rock_stays() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, 165);
        put(&mut l, 3, 6, 166);
        put(&mut l, 6, 3, ROCK); // shadows (3,6) only
        correct_shadow(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 161, "164..=167 not under DirtRock => -4");
        assert_eq!(at(&l, 3, 6), 166, "still under rock => unchanged");
    }

    #[test]
    fn the_rect_is_clipped_to_0_3_w_minus_3_h() {
        // Pixels whose (x+3, y-3) probe would leave the level are never visited.
        let mut l = lvl(10, 10);
        put(&mut l, 8, 5, 165); // x >= w-3
        put(&mut l, 2, 1, 165); // y < 3
        put(&mut l, 2, 5, 165); // inside
        correct_shadow(&mut l, -20, -20, 50, 50);
        assert_eq!(at(&l, 8, 5), 165);
        assert_eq!(at(&l, 2, 1), 165);
        assert_eq!(at(&l, 2, 5), 161);
    }

    #[test]
    fn the_caller_rect_limits_the_pass() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, 165);
        correct_shadow(&mut l, 3, 3, 10, 10); // x starts at 3
        assert_eq!(at(&l, 2, 5), 165);
    }

    #[test]
    fn palidx_arithmetic_wraps_like_cpp() {
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, WRAP);
        put(&mut l, 5, 2, ROCK);
        correct_shadow(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 2, "254 + 4 wraps to 2 (C++ PalIdx)");
    }

    #[test]
    fn the_if_enabled_variant_follows_the_frame_flag() {
        end_frame();
        let mut l = lvl(10, 10);
        put(&mut l, 2, 5, 165);
        correct_shadow_if_enabled(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 165, "outside a frame the flag is off (today's behaviour)");
        begin_frame(true);
        assert!(enabled());
        correct_shadow_if_enabled(&mut l, 0, 0, 10, 10);
        assert_eq!(at(&l, 2, 5), 161);
        end_frame();
        assert!(!enabled());
    }
}
```

and append to the end of `mod tests` in `state.rs`:

```rust
    #[test]
    fn a45_shadow_defaults_off_and_process_frame_clears_the_frame_flag() {
        let mut state = idle_state(3);
        assert!(!state.shadow, "post-new default false => every golden byte-identical");
        state.shadow = true;
        state.process_frame(&[ControlState::new(), ControlState::new()]);
        assert!(!crate::shadow::enabled(), "end_frame clears the flag after the tick");
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib shadow`
Expected: FAIL to compile — `cannot find function correct_shadow`, `cannot find value MAT_SEE_SHADOW`, `no field shadow on SimState`.

- [ ] **Step 3: Implement `shadow.rs`** — above the test module:

```rust
//! Port of `CorrectShadow` (`gfx/blit.cpp:624-639`) and its per-tick enable flag
//! (Step 4½a-1, design §5.1).
//!
//! C++ runs `CorrectShadow` after every in-frame crater when `settings->shadow` is on
//! (default ON, `settings.hpp:74`): `nobject.cpp:123,215`, `sobject.cpp:212`,
//! `weapon.cpp:121`, `worm.cpp:784,932,942`. It rewrites `material_id`, which the
//! level hash reads. `SimState.shadow` carries the setting; `process_frame` publishes
//! it here at the top of every tick ([`begin_frame`]) and clears it at the bottom
//! ([`end_frame`]), so inside a tick the flag always equals `self.shadow` and never
//! leaks between states or threads. A thread-local instead of a parameter because
//! `sobject_create` (one of the sites) has a wide fan-in — the precedent is
//! `sound::begin_frame`'s hook indices. `MakeShadow` (level preparation) is 4½b's
//! `sim::levelgen::make_shadow`, not here.

use std::cell::Cell;

use crate::state::{LevelSim, MAT_SEE_SHADOW};

thread_local! {
    /// This thread's `settings->shadow` for the current tick. Off outside a tick.
    static ENABLED: Cell<bool> = const { Cell::new(false) };
}

/// Open a tick: publish `SimState.shadow`. Called at the top of `process_frame`.
pub fn begin_frame(enabled: bool) {
    ENABLED.with(|e| e.set(enabled));
}

/// Close a tick: the flag is off again. Called at the bottom of `process_frame`.
pub fn end_frame() {
    ENABLED.with(|e| e.set(false));
}

/// The current tick's flag (tests and diagnostics).
pub fn enabled() -> bool {
    ENABLED.with(|e| e.get())
}

/// `CorrectShadow(common, level, Rect(x1, y1, x2, y2))` — always applied. The rect is
/// intersected with `Rect(0, 3, width - 3, height)` (`blit.cpp:625`) so the
/// `(x + 3, y - 3)` probe is always in range; x-outer / y-inner like C++ (the probe's
/// column is never visited before it is read). `SeeShadow` under `DirtRock` ⇒ `+4`;
/// else `164..=167` not under `DirtRock` ⇒ `-4`. `PalIdx` arithmetic wraps.
pub fn correct_shadow(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32) {
    let x1 = x1.max(0);
    let y1 = y1.max(3);
    let x2 = x2.min(level.width - 3);
    let y2 = y2.min(level.height);
    for x in x1..x2 {
        for y in y1..y2 {
            let idx = (x + y * level.width) as usize;
            let pix = level.material_id[idx];
            let see_shadow = level.material_flags[pix as usize] & MAT_SEE_SHADOW != 0;
            let shaded = level.dirt_rock(x + 3, y - 3);
            if see_shadow && shaded {
                level.set_material(idx, pix.wrapping_add(4));
            } else if (164..=167).contains(&pix) && !shaded {
                level.set_material(idx, pix.wrapping_sub(4));
            }
        }
    }
}

/// [`correct_shadow`] iff the current tick's `settings->shadow` flag is on — the form
/// the seven sim call sites use.
pub fn correct_shadow_if_enabled(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32) {
    if enabled() {
        correct_shadow(level, x1, y1, x2, y2);
    }
}
```

Note: C++ evaluates `DirtRock(x+3, y-3)` only in the tested branches; the probe is pure, so reading it once is equivalent.

- [ ] **Step 4: Implement the `state.rs` plumbing**

After `pub const MAT_WORM: u8 = 1 << 5;` add:

```rust
/// `Material::kSeeShadow` (`material.hpp:11`, `1 << 4`): a pixel that shows a shadow
/// when a `DirtRock` pixel sits at `(x + 3, y - 3)` — `CorrectShadow`'s predicate.
pub const MAT_SEE_SHADOW: u8 = 1 << 4;
```

After the `pub time_to_lose: i32,` field add:

```rust
    /// C++ `Settings::shadow` (`settings.hpp:74`, in-game default `true`): gates
    /// `CorrectShadow` after every in-frame crater (`sim::shadow`, Step 4½a-1). The
    /// oracle dumper's classic path forces it `false`, so it defaults to **false**
    /// post-`new` and every prior golden stays byte-identical; the 4½a builder assigns
    /// the setting. **Not hashed** (its effect on `material_id` is).
    pub shadow: bool,
```

In `SimState::new`, after `time_to_lose: 600,` add `shadow: false,`. In `process_frame`, after `crate::shake::begin_frame();` add:

```rust
        // Step 4½a-1: publish this tick's settings->shadow for the CorrectShadow sites.
        crate::shadow::begin_frame(self.shadow);
```

and after `*screen_flash = crate::flash::take_frame();` add:

```rust
        // Step 4½a-1: the CorrectShadow flag is off again outside the tick.
        crate::shadow::end_frame();
```

In `do_respawning`, replace the comment line `// :784-786 CorrectShadow — gated on settings->shadow (false) => OMITTED.` with:

```rust
        // :784-786 CorrectShadow behind settings->shadow (Step 4½a-1).
        crate::shadow::correct_shadow_if_enabled(
            level,
            ipos_x - 10,
            ipos_y - 10,
            ipos_x + 11,
            ipos_y + 11,
        );
```

and in its doc comment (`:2670-2671`) change "`CorrectShadow` (`:784-786`, gated on `settings->shadow`, **false**)," to "`CorrectShadow` is live since 4½a-1 (behind `SimState.shadow`);".

Add `shadow: false,` after `time_to_lose: 600,` in the hand-built `SimState` literals in `hash.rs` and `wide_checksum.rs`.

- [ ] **Step 5: Wire the six other sites**

`nobject.rs` (ground-explode `BlitImageOnMap` arm): replace the comment block `// :119-128 BlitImageOnMap-on-ground arm …` so the `if ty.start_frame > 0 && ty.draw_on_map { … }` block reads:

```rust
            // :119-128 BlitImageOnMap-on-ground arm (Slice-4d): a `draw_on_map`
            // object with `start_frame > 0` (the spent SHELL) paints its 7x7 image
            // into `material_id` at `(ipos - 3)` before exploding, then (:123-127)
            // CorrectShadow behind settings->shadow (Step 4½a-1).
            if ty.start_frame > 0 && ty.draw_on_map {
                blit_image_on_map(
                    level,
                    small_sprites,
                    (ty.start_frame + obj.cur_frame) as usize,
                    ipos_x - 3,
                    ipos_y - 3,
                );
                crate::shadow::correct_shadow_if_enabled(
                    level,
                    ipos_x - 8,
                    ipos_y - 8,
                    ipos_x + 9,
                    ipos_y + 9,
                );
            }
```

`nobject.rs` (dirt-effect crater): the block becomes

```rust
        // :211-219 dirt_effect crater + CorrectShadow behind settings->shadow (4½a-1).
        // Inert for the dirt particle (dirt_effect=-1).
        if ty.dirt_effect >= 0 {
            draw_dirt_effect(
                level,
                large_sprites,
                textures,
                ty.dirt_effect,
                ftoi(obj.pos.x) - 7,
                ftoi(obj.pos.y) - 7,
                rand,
            );
            let (px, py) = (ftoi(obj.pos.x), ftoi(obj.pos.y));
            crate::shadow::correct_shadow_if_enabled(level, px - 10, py - 10, px + 11, py + 11);
        }
```

`sobject.rs` (crater): replace `// CorrectShadow omitted (settings->shadow = false, O4).` with `// :212-214 CorrectShadow behind settings->shadow (Step 4½a-1).` and add, directly after the `draw_dirt_effect(…);` call inside `if ty.dirt_effect >= 0 { … }`:

```rust
        crate::shadow::correct_shadow_if_enabled(level, x - 10, y - 10, x + 11, y + 11);
```

`weapon.rs` (`blow_up` tail): the block becomes

```rust
    if weapon.dirt_effect >= 0 {
        draw_dirt_effect(
            level,
            large_sprites,
            textures,
            weapon.dirt_effect,
            ftoi(pos.x) - 7,
            ftoi(pos.y) - 7,
            rand,
        );
        // weapon.cpp:121-123 CorrectShadow behind settings->shadow (Step 4½a-1).
        let (ix, iy) = (ftoi(pos.x), ftoi(pos.y));
        crate::shadow::correct_shadow_if_enabled(level, ix - 10, iy - 10, ix + 11, iy + 11);
    }
```

and in its doc comment change "**`CorrectShadow` is omitted (O4)**" to "**`CorrectShadow` is live since 4½a-1 (behind `SimState.shadow`)**".

`control.rs` (dig): replace `// CorrectShadow (worm.cpp:932-935) OMITTED: shadow off, render-only.` with `// worm.cpp:932-935 CorrectShadow behind settings->shadow (Step 4½a-1).` and add after EACH of the two `draw_dirt_effect(… 7, ftoi(dig_pos.x), ftoi(dig_pos.y), rand);` calls:

```rust
            crate::shadow::correct_shadow_if_enabled(
                level,
                ftoi(dig_pos.x) - 3,
                ftoi(dig_pos.y) - 3,
                ftoi(dig_pos.x) + 18,
                ftoi(dig_pos.y) + 18,
            );
```

(the second one after the `worm.cpp:940-941` crater, mirroring `:942-945`). In the doc comment at `:641` change "`CorrectShadow` is **OMITTED** (shadow=false," to "`CorrectShadow` runs behind `SimState.shadow` (4½a-1;".

- [ ] **Step 6: Run the tests and the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib shadow` — Expected: PASS (6 shadow tests + `a45_shadow_defaults_off…`).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS; every golden byte-identical (shadow stays false on every existing path).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/shadow.rs` — Expected: no output.

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/shadow.rs rust/sim/src/lib.rs rust/sim/src/state.rs rust/sim/src/nobject.rs rust/sim/src/sobject.rs rust/sim/src/weapon.rs rust/sim/src/control.rs rust/sim/src/hash.rs rust/sim/src/wide_checksum.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5a-1): port CorrectShadow behind SimState.shadow at all seven sites" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Reviewer (Opus): the intersect rect, loop order and both rules match `blit.cpp:624-639`; each of the seven rects matches its C++ site; the flag is published before any sim work and cleared at the end; the signature in **Interfaces** is exactly what shipped (4½b T9 depends on it). **After this commit, tell the controller: 4½b's T9 can run.**

---

### Task 4: Game-mode rules — Scales death/respawn, `DoHealing`, the GameOfTag guard  [Opus]

**Files:**
- Modify: `rust/sim/src/state.rs` — `worm_death` (`:2856-2920`) + its call (`:2091-2101`); `do_respawning` (`:2676-2742`) + its call (`:2172-2179`); new `do_healing` after `do_damage` (`:525`); the existing test calls of `worm_death` (11, `:4507-4715`) and `do_respawning` (6, `:5039-5178`); new tests at the end of `mod tests`
- Modify: `rust/sim/src/bonus.rs:517` (pickup heal → `do_healing`)

**Interfaces:**
- Produces: `pub fn sim::state::do_healing(worms: &mut [WormState], w_idx: usize, amount: i32, game_mode: u32, settings_health: i32)`.
- Changes (private): `worm_death(w, index, blood, nobject_types, cossin, rand, nobjects, last_killed_idx, got_changed, game_mode: u32, settings_health: i32)`; `do_respawning(worm, level, large_sprites, textures, settings_health, game_mode: u32, rand)`.

Why: design §1.3 findings 2–3 / §5.2. In a real Scales match a death wraps the overkill into lives (`worm.cpp:384-388`), a respawn does NOT restore health (`:794-796`), and a health-bonus pickup goes through `Game::DoHealing` (`game.cpp:591-609`), which damages the other worm. The GameOfTag guard (`worm.cpp:396-398`) is unobservable with two worms but is ported for fidelity. KillEmAll behaviour is unchanged ⇒ every prior golden byte-identical.

- [ ] **Step 1: Write the failing tests** — append to the end of `mod tests` in `state.rs`:

```rust
    #[test]
    fn a45_scales_death_wraps_the_overkill_into_lives() {
        let mut w = dying_worm(0, -130, -1); // lives 5 (two_worms)
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(11);
        let (mut lki, mut gc) = (-1i32, false);
        worm_death(&mut w, 0, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc, 3, 100);
        assert_eq!(w.lives, 3, "two whole settings_health of overkill => two lives (worm.cpp:385-388)");
        assert_eq!(w.health, 70, "-130 + 100 + 100");
    }

    #[test]
    fn a45_killemall_death_keeps_the_negative_health() {
        let mut w = dying_worm(0, -130, -1);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(11);
        let (mut lki, mut gc) = (-1i32, false);
        worm_death(&mut w, 0, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc, 0, 100);
        assert_eq!(w.lives, 4);
        assert_eq!(w.health, -130);
    }

    #[test]
    fn a45_gameoftag_guard_keeps_it_for_a_third_party_killer() {
        // "it" = worm 1; worm 0 is killed by worm 2 (neither victim nor "it") => "it" stays.
        let mut w = dying_worm(0, 0, 2);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(11);
        let (mut lki, mut gc) = (1i32, false);
        worm_death(&mut w, 0, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc, 1, 100);
        assert_eq!(lki, 1, "worm.cpp:396-398: the guard is false => no reassignment");
        assert!(!gc);
    }

    #[test]
    fn a45_gameoftag_guard_reassigns_when_it_is_the_killer() {
        let mut w = dying_worm(0, 0, 1);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(11);
        let (mut lki, mut gc) = (1i32, false);
        worm_death(&mut w, 0, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc, 1, 100);
        assert_eq!(lki, 0, "killer == it => the victim becomes it");
        assert!(gc);
    }

    #[test]
    fn a45_scales_respawn_does_not_restore_health() {
        let sprites = dirt_sprites();
        let tex = [dirt_texture()];
        let mut level = flat_level(400, 400);
        let mut w = respawn_worm(150, 150, (70, 70), true);
        w.health = 37;
        let mut rand = seeded_rand(7);
        do_respawning(&mut w, &mut level, &sprites, &tex, 100, 3, &mut rand);
        assert!(w.visible, "converged + ready => respawned");
        assert_eq!(w.health, 37, "Scales: no restore (worm.cpp:794-796)");

        let mut level2 = flat_level(400, 400);
        let mut w2 = respawn_worm(150, 150, (70, 70), true);
        w2.health = 37;
        let mut rand2 = seeded_rand(7);
        do_respawning(&mut w2, &mut level2, &sprites, &tex, 100, 0, &mut rand2);
        assert_eq!(w2.health, 100, "KillEmAll: health = settings->health");
    }

    #[test]
    fn a45_do_healing_scales_damages_the_other_worm() {
        let mut worms: Vec<WormState> = two_worms().iter().map(WormState::from_init).collect();
        worms[0].health = 50;
        do_healing(&mut worms, 0, 30, 3, 100);
        assert_eq!(worms[0].health, 80);
        assert_eq!(worms[1].health, 70, "game.cpp:594-605: the healed amount is dealt to the others");
        assert_eq!(worms[1].last_killed_by_idx, -1, "not killed => no attribution");

        worms[1].health = 10;
        do_healing(&mut worms, 0, 30, 3, 100);
        assert_eq!(worms[1].health, -20);
        assert_eq!(worms[1].last_killed_by_idx, 0, "the healer is the killer (DoDamageDirect by_idx)");
    }

    #[test]
    fn a45_do_healing_killemall_clamps_and_leaves_the_others() {
        let mut worms: Vec<WormState> = two_worms().iter().map(WormState::from_init).collect();
        worms[0].health = 90;
        do_healing(&mut worms, 0, 30, 0, 100);
        assert_eq!(worms[0].health, 100);
        assert_eq!(worms[1].health, 100);
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib a45_`
Expected: FAIL to compile — `this function takes 9 arguments but 11 arguments were supplied` (`worm_death`), `takes 6 arguments but 7` (`do_respawning`), `cannot find function do_healing`.

- [ ] **Step 3: Implement**

`worm_death`: append two parameters after `got_changed: &mut bool,`:

```rust
    game_mode: u32,
    settings_health: i32,
```

replace the `// :384-391 lives …` comment block and `w.lives -= 1;` with:

```rust
    // :384-391 lives. Scales (game_mode 3): wrap every whole settings->health of
    // overkill into a lost life, leaving health in (0, settings_health]; every other
    // mode: one life. `settings_health >= 1` is guaranteed by the 4½a builder
    // (BuildError::InvalidHealth), so the loop terminates.
    if game_mode == 3 {
        while w.health <= 0 {
            w.health += settings_health;
            w.lives -= 1;
        }
    } else {
        w.lives -= 1;
    }
```

and replace the `// :393-401 …` comment block plus the two statements `let old_last_killed = *last_killed_idx; *last_killed_idx = index;` with:

```rust
    // :393-401 last_killed_idx / got_changed (no rand; unhashed). GameOfTag keeps "it"
    // when the killer is neither the victim nor "it" (:396-398) — unobservable with two
    // worms (design §1.3 finding 3), ported for fidelity.
    let old_last_killed = *last_killed_idx;
    if game_mode != 1
        || *last_killed_idx < 0
        || w.last_killed_by_idx < 0
        || w.last_killed_by_idx == index
        || w.last_killed_by_idx == *last_killed_idx
    {
        *last_killed_idx = index;
    }
```

(keep the following `*got_changed = old_last_killed != *last_killed_idx;`). In `process_frame`'s call, add `game_mode, settings_health,` after `got_changed,`.

`do_respawning`: insert `game_mode: u32,` between `settings_health: i32,` and `rand: &mut Rand,`; replace `// :794-796 health = settings->health (Scales guard folds away; KillEmAll).` and `worm.health = settings_health;` with:

```rust
        // :794-796 health = settings->health, except in Scales (game_mode 3).
        if game_mode != 3 {
            worm.health = settings_health;
        }
```

update the doc sentence "The Scales-of-Justice guard on the health restore (`:794`) folds away (the TC is KillEmAll), so `health` is always restored here." to "The Scales-of-Justice guard on the health restore (`:794`) is live since 4½a-1.", and in `process_frame`'s call insert `game_mode,` after `settings_health,`.

Existing tests: append `, 0, 100` before the closing `)` of each of the 11 existing `worm_death(` test calls, and insert `0, ` after the `100, ` argument of each of the 6 existing `do_respawning(` test calls (KillEmAll + default health = today's behaviour).

`do_healing` — insert after `do_damage`:

```rust
/// Port of `Game::DoHealing` (`game.cpp:591-609`) — the pickup heal. `DoHealingDirect`
/// on the healed worm, then in Scales (`game_mode == 3`) the SAME amount is dealt to
/// the other worms, split by truncating division in source order (`parts`/`left`,
/// `:595-604`) through `DoDamageDirect(other, k, w.index)` — the healer is the
/// attributed killer if one dies. Every other mode clamps (`:607`; already applied by
/// `do_healing_direct`, so KillEmAll is byte-identical to the prior pickup port).
pub fn do_healing(
    worms: &mut [WormState],
    w_idx: usize,
    amount: i32,
    game_mode: u32,
    settings_health: i32,
) {
    crate::bonus::do_healing_direct(&mut worms[w_idx], amount, game_mode, settings_health);
    if game_mode == 3 {
        let healer = worms[w_idx].index;
        let mut parts = worms.len() as i32 - 1;
        let mut left = amount;
        for j in 0..worms.len() {
            if j != w_idx {
                let k = left / parts;
                worms[j].do_damage_direct(k, healer);
                parts -= 1;
                left -= k;
            }
        }
    } else {
        worms[w_idx].health = worms[w_idx].health.min(settings_health);
    }
}
```

`bonus.rs:517`: replace `do_healing_direct(&mut worms[wi], amount, game_mode, settings_health);` with

```rust
                // worm.cpp:295 calls Game::DoHealing (Scales redistributes, 4½a-1).
                crate::state::do_healing(worms, wi, amount, game_mode, settings_health);
```

- [ ] **Step 4: Run the tests and the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim` — Expected: PASS (the 7 `a45_` tests + every existing test).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS; `sim_slice6_scales` and `sim_slice6_gametag` unchanged (their windows reach none of the new branches: scales wounds without killing, gametag has a single kill).

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/state.rs rust/sim/src/bonus.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5a-1): Scales death/respawn rules, Game::DoHealing, GameOfTag it-guard" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Reviewer (Opus): each branch against `worm.cpp:384-401`, `:794-796`, `game.cpp:591-609`; the pickup call site now matches `worm.cpp:295`; KillEmAll paths are unchanged (re-diff green).

---

### Task 5: `sim::game_over::is_game_over`  [Sonnet]

**Files:**
- Create: `rust/sim/src/game_over.rs`
- Modify: `rust/sim/src/lib.rs` (add `pub mod game_over;` after `pub mod flash;`)

**Interfaces:**
- Produces: `pub fn sim::game_over::is_game_over(state: &SimState) -> bool` (used by T8, T9, T10).

Why: design §5.3 — `Game::IsGameOver` (`game.cpp:521-544`), a pure read of sim state, total (Holdazone checks the timer like C++; the builder is the refusal point).

- [ ] **Step 1: Write the failing tests** — create `game_over.rs` with only this module, add the `lib.rs` line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::ControlConsts;
    use crate::physics::PhysicsConsts;
    use crate::state::{WeaponInit, WormInit, NUM_WEAPONS};
    use assets::level::LevelData;
    use assets::sprite::SpriteSet;
    use sim_core::vec::Vec2;

    fn state(game_mode: u32) -> SimState {
        let level = LevelData {
            width: 4,
            height: 4,
            material_id: vec![0; 16],
            palette: None,
            display: None,
        };
        let worm = |index: i32, stats_x: i32| WormInit {
            index,
            health: 100,
            lives: 5,
            stats_x,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::zero(),
            visible: true,
        };
        let mut s = SimState::new(
            &level,
            &[worm(0, 0), worm(1, 218)],
            1,
            &[0u8; 256],
            Vec::new(),
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            100,
            true,
            100,
        );
        s.game_mode = game_mode;
        s
    }

    #[test]
    fn kill_em_all_and_scales_end_when_any_worm_has_no_lives() {
        for mode in [0, 3] {
            let mut s = state(mode);
            assert!(!is_game_over(&s));
            s.worms[1].timer = 10_000; // timers are irrelevant in these modes
            assert!(!is_game_over(&s));
            s.worms[1].lives = 0;
            assert!(is_game_over(&s), "mode {mode}: lives <= 0 (game.cpp:522-528)");
            s.worms[1].lives = -2;
            assert!(is_game_over(&s));
        }
    }

    #[test]
    fn game_of_tag_and_holdazone_end_on_time_to_lose() {
        for mode in [1, 2] {
            let mut s = state(mode);
            s.time_to_lose = 12;
            s.worms[0].lives = 0; // lives are irrelevant in these modes
            s.worms[1].timer = 11;
            assert!(!is_game_over(&s));
            s.worms[1].timer = 12;
            assert!(is_game_over(&s), "mode {mode}: timer >= time_to_lose (game.cpp:529-540)");
        }
    }

    #[test]
    fn an_unknown_mode_never_ends() {
        let mut s = state(7);
        s.worms[0].lives = 0;
        s.worms[0].timer = 10_000;
        assert!(!is_game_over(&s));
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib game_over`
Expected: FAIL to compile — `cannot find function is_game_over`.

- [ ] **Step 3: Implement** — above the tests:

```rust
//! Port of `Game::IsGameOver` (`game.cpp:521-544`), Step 4½a-1 (design §5.3).
//!
//! A pure read of sim state, checked by the match flow after every tick
//! (`localController.cpp:177-179`). Total: Holdazone (mode 2) checks `time_to_lose`
//! exactly like C++ (not a "time to win" — cpp-map §7.1), and an unknown mode is never
//! over. The builder refuses Holdazone and unknown modes, so neither reaches a match.

use crate::state::SimState;

/// `true` once the match has ended: KillEmAll (0) / ScalesOfJustice (3) when any worm
/// has `lives <= 0`; GameOfTag (1) / Holdazone (2) when any worm's `timer >=
/// time_to_lose`; any other mode never.
pub fn is_game_over(state: &SimState) -> bool {
    match state.game_mode {
        0 | 3 => state.worms.iter().any(|w| w.lives <= 0),
        1 | 2 => state.worms.iter().any(|w| w.timer >= state.time_to_lose),
        _ => false,
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib game_over` — Expected: PASS (3 tests).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/game_over.rs` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/game_over.rs rust/sim/src/lib.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5a-1): port Game::IsGameOver as sim::game_over::is_game_over" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: The builder — `build_match`, `validate`, the level-palette rule  [Opus]

**Files:**
- Create: `rust/scenario/src/build.rs`
- Modify: `rust/scenario/src/lib.rs` (add `pub mod build;` after `pub mod assets;`)
- Modify: `rust/scenario/src/loader.rs` — `fn load_sprites` → `pub(crate) fn load_sprites`; delete `let color_anim = tc.color_anim.clone();` (`:110`); replace the scene tail (`:187-217`) with a call to a new `pub(crate) fn scene_data`
- Test: in-module tests in `build.rs`; Create `rust/oracle-tests/tests/sim_slice4_5a_builder_golden.rs`

**Interfaces:**
- Consumes: T1 `MatchConfig`, `Settings`, `GM_*`, `WEAP_TABLE_LEN`; T3 `SimState.shadow`; `WormInit::resolve_weapons` (`state.rs:240-256`); `scenario::Loaded` (`loader.rs:86-90`); `assets::level::LevelData`.
- Produces (used by T8, T9, 4½c, 4½d):
  - `pub enum BuildError { HoldazoneUnsupported, InvalidGameMode(u32), InvalidHealth(i32), AsymmetricHealth { p1: i32, p2: i32 }, InvalidWeapon { worm: usize, slot: usize, value: u32 }, InvalidBloodParticleMax(i32), TooManyWeapons(usize) }` — `Clone, Debug, PartialEq, Eq`, `Display`, `std::error::Error`.
  - `pub fn validate(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError>`.
  - `pub fn build_match(tc_root: &Path, cfg: &MatchConfig, level: &LevelData) -> Result<Loaded, BuildError>` — takes a READY `LevelData`; level preparation is 4½b's `sim::levelgen::generate_from_settings`, composed in 4½d (this task does not reference it).
  - `pub(crate) fn scene_data(tc_root: &Path, tc: &TcConfig, origpal: Palette, large_sprites: &SpriteSet) -> SceneData` (loader.rs).

Why: design §4. The C++ `LocalController` start state (worms invisible at `(0,0)`, `killed_timer` 150, `health = ws.health`, `lives = settings.lives`, `InitWeapons` from each worm's picks, pool sized by `blood_particle_max`) plus every TC const and setting post-`new`. Two C++ checks land in this task: the unit test pins the mapping, and `sim_slice4_5a_builder_golden.rs` proves the builder reproduces the EXISTING `sim_slice6_fuzz5` golden (design §7.4) — before any dumper change. Also folds in the palette bug (design §4.1): the level's POWERLEVEL palette wins iff `load_powerlevel_palette`.

- [ ] **Step 1: Write the failing unit tests** — create `build.rs` with only this module, add `pub mod build;` to `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE};
    use assets::palette::{Color, Palette};
    use sim_core::vec::Vec2;

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

    fn cfg() -> MatchConfig {
        MatchConfig { settings: Settings::default(), seed: 1234 }
    }

    fn level() -> LevelData {
        let bytes = std::fs::read(format!("{TC_ROOT}/Levels/render_stage.lev")).expect("read level");
        assets::level::load(&bytes).expect("level loads")
    }

    fn tc_and_objects() -> (TcConfig, Objects) {
        let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
        let objects =
            Objects::load(&tc.types, |sub, id| std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg")))
                .unwrap();
        (tc, objects)
    }

    #[test]
    fn the_cpp_defaults_validate() {
        assert_eq!(validate(&cfg(), 40), Ok(()));
    }

    #[test]
    fn holdazone_and_unknown_modes_are_refused() {
        let mut c = cfg();
        c.settings.game_mode = GM_HOLDAZONE;
        assert_eq!(validate(&c, 40), Err(BuildError::HoldazoneUnsupported));
        c.settings.game_mode = 4;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidGameMode(4)));
    }

    #[test]
    fn health_must_be_positive_and_equal_for_both_players() {
        let mut c = cfg();
        c.settings.worm_settings[0].health = 0;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidHealth(0)));
        let mut c = cfg();
        c.settings.worm_settings[1].health = 150;
        assert_eq!(validate(&c, 40), Err(BuildError::AsymmetricHealth { p1: 100, p2: 150 }));
    }

    #[test]
    fn weapon_picks_must_index_weap_order() {
        let mut c = cfg();
        c.settings.worm_settings[1].weapons[3] = 41;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidWeapon { worm: 1, slot: 3, value: 41 }));
        c.settings.worm_settings[1].weapons[3] = 0;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidWeapon { worm: 1, slot: 3, value: 0 }));
    }

    #[test]
    fn pool_and_table_limits_are_refused() {
        let mut c = cfg();
        c.settings.blood_particle_max = 0;
        assert_eq!(validate(&c, 40), Err(BuildError::InvalidBloodParticleMax(0)));
        assert_eq!(validate(&cfg(), 41), Err(BuildError::TooManyWeapons(41)));
    }

    #[test]
    fn the_network_player_is_not_validated() {
        let mut c = cfg();
        c.settings.worm_settings[2].health = 0;
        c.settings.worm_settings[2].weapons = [0; 5];
        assert_eq!(validate(&c, 40), Ok(()));
    }

    #[test]
    fn build_match_maps_every_setting_onto_the_state() {
        let mut c = cfg();
        let s = &mut c.settings;
        s.lives = 7;
        s.loading_time = 37;
        s.blood = 250;
        s.load_change = false;
        s.max_bonuses = 6;
        s.shadow = false;
        s.time_to_lose = 99;
        s.game_mode = GM_SCALES_OF_JUSTICE;
        s.blood_particle_max = 300;
        s.weap_table[5] = 2;
        s.worm_settings[0].health = 150;
        s.worm_settings[1].health = 150;
        s.worm_settings[0].weapons = [2, 3, 4, 5, 6];
        s.worm_settings[1].weapons = [7, 8, 9, 10, 11];
        let loaded = build_match(Path::new(TC_ROOT), &c, &level()).expect("valid config builds");
        let st = &loaded.state;
        let (tc, objects) = tc_and_objects();
        let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
        weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
        for i in 0..2 {
            let w = &st.worms[i];
            assert_eq!(w.index, i as i32);
            assert_eq!((w.lives, w.health, w.killed_timer), (7, 150, 150));
            assert!(!w.visible, "LocalController start: invisible, respawns in-sim");
            assert_eq!(w.pos, Vec2::zero());
            assert_eq!(w.stats_x, [0, 218][i]);
            let want =
                WormInit::resolve_weapons(&objects, &weap_order, &c.settings.worm_settings[i].weapons);
            for j in 0..5 {
                assert_eq!((w.weapons[j].ty, w.weapons[j].ammo), (want[j].ty, want[j].ammo));
            }
        }
        assert_ne!(st.worms[0].weapons[0].ty, st.worms[1].weapons[0].ty, "per-worm loadouts");
        assert_eq!((st.settings_loading_time, st.blood, st.load_change), (37, 250, false));
        assert_eq!(st.settings_max_bonuses, 6);
        assert_eq!(st.weap_table.len(), 40);
        assert_eq!(st.weap_table[5], 2);
        assert_eq!((st.settings_health, st.game_mode, st.time_to_lose), (150, 3, 99));
        assert!(!st.shadow);
        assert_eq!(st.bobjects.capacity(), 300);
        assert_eq!(st.sound_hooks, tc.sound_hooks);
        assert_eq!(st.bonus_drop_chance, tc.constants.BonusDropChance);
        assert_eq!(st.bonus_health_var, tc.constants.BonusHealthVar);
        assert_eq!(st.worm_spawn_rect_w, tc.constants.WormSpawnRectW);
        assert_eq!(st.laser_weapon, tc.constants.LaserWeapon);
        assert_eq!(st.rand.last(), 0, "building consumes no RNG");
    }

    #[test]
    fn build_match_refuses_holdazone_without_panicking() {
        let mut c = cfg();
        c.settings.game_mode = GM_HOLDAZONE;
        assert_eq!(
            build_match(Path::new(TC_ROOT), &c, &level()).err(),
            Some(BuildError::HoldazoneUnsupported)
        );
    }

    #[test]
    fn a_powerlevel_palette_wins_only_when_the_setting_allows_it() {
        let mut custom = Palette { entries: [Color::default(); 256] };
        custom.entries[1] = Color { r: 1, g: 2, b: 3 };
        let mut lev = level();
        lev.palette = Some(custom.clone());
        let mut c = cfg();
        assert!(c.settings.load_powerlevel_palette, "C++ default: on");
        let on = build_match(Path::new(TC_ROOT), &c, &lev).unwrap();
        assert_eq!(on.scene.origpal, custom, "level.cpp:281-294 + game.cpp:476");
        c.settings.load_powerlevel_palette = false;
        let off = build_match(Path::new(TC_ROOT), &c, &lev).unwrap();
        let small = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).unwrap();
        let exepal = assets::sprite::Tga::load(&small).unwrap().palette;
        assert_eq!(off.scene.origpal, exepal, "level.cpp:385-392: reset to exepal (small.tga's)");
        assert_ne!(exepal, custom, "non-vacuous: the two palettes differ");
    }
}
```

- [ ] **Step 2: Write the failing C++ cross-check** — create `rust/oracle-tests/tests/sim_slice4_5a_builder_golden.rs`:

```rust
//! Step 4½a-1 T6 — the `MatchConfig` builder reproduces an EXISTING C++ golden
//! (design §7.4). `sim_slice6_fuzz5` was produced by the classic dumper path with both
//! worms seeded dead at (0,0), health 100, lives 50, max_bonuses 4, loading_time 0,
//! shadow false and every other C++ default — which is exactly a `MatchConfig`. So
//! `build_match` with those settings must drive the committed 1501-row golden
//! bit-for-bit on all 11 columns, before any dumper change.

use std::path::Path;

use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::{MatchConfig, Settings};
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{GOLDEN}/{name}")).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// `<tick> <master> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`.
fn parse_golden(text: &str) -> Vec<(u32, [u32; 10])> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(cols.len(), 11, "11-column golden line");
            let mut h = [0u32; 10];
            for (i, c) in cols[1..].iter().enumerate() {
                h[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            (cols[0].parse().expect("tick"), h)
        })
        .collect()
}

fn check(state: &SimState, tick: u32, want: &[u32; 10]) {
    let c = hash_components(state);
    let got = [
        hash_game_state(state),
        c.rng,
        c.level,
        c.worms[0],
        c.worms[1],
        c.bobjects,
        c.bonuses,
        c.sobjects,
        c.nobjects,
        c.wobjects,
    ];
    let names = ["master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob"];
    // Components first, master last, so a divergence localises.
    for i in (1..10).chain(0..1) {
        assert_eq!(got[i], want[i], "tick {tick}: {}: got {:08x} want {:08x}", names[i], got[i], want[i]);
    }
}

#[test]
fn build_match_reproduces_sim_slice6_fuzz5() {
    let scenario = Scenario::parse(&read("sim_slice6_fuzz5_scenario.txt")).expect("parses");
    assert_eq!((scenario.seed, scenario.max_bonuses, scenario.game_mode), (43, 4, 0));
    for w in &scenario.worms {
        assert_eq!((w.pos_x, w.pos_y, w.health, w.lives, w.visible), (0, 0, 100, 50, false));
    }
    let mut settings = Settings::default();
    settings.lives = 50;
    settings.max_bonuses = 4;
    settings.loading_time = 0; // the classic dumper path forces 0
    settings.shadow = false; // the classic dumper path forces false
    let cfg = MatchConfig { settings, seed: scenario.seed };
    let bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level)).expect("read level");
    let level = assets::level::load(&bytes).expect("level loads");
    let mut state = build_match(Path::new(TC_ROOT), &cfg, &level).expect("builds").state;

    let golden = parse_golden(&read("sim_slice6_fuzz5.txt"));
    assert_eq!(golden.len(), 1501);
    check(&state, 0, &golden[0].1);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        check(&state, k, &golden[k as usize].1);
    }
}
```

- [ ] **Step 3: Run both to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib build::tests` — Expected: FAIL to compile (`cannot find function validate`).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test sim_slice4_5a_builder_golden` — Expected: FAIL to compile (`unresolved import scenario::build::build_match`).

- [ ] **Step 4: Factor `scene_data` out of the loader** — in `loader.rs` make `load_sprites` `pub(crate)`, delete `let color_anim = tc.color_anim.clone();`, replace everything from `let fire_cone = build_fire_cone_sprites(&state.large_sprites);` to the end of `load` with:

```rust
    let scene = scene_data(tc_root, &tc, origpal, &state.large_sprites);
    Loaded {
        state,
        viewports: Viewport::player_layout(),
        scene,
    }
}

/// The owned Scene ingredients for a loaded TC — shared by [`load`] and
/// `crate::build::build_match` (Step 4½a-1; a pure factor-out of the former `load` tail,
/// gated by the render goldens). `origpal` is the palette the caller chose.
pub(crate) fn scene_data(
    tc_root: &Path,
    tc: &TcConfig,
    origpal: Palette,
    large_sprites: &SpriteSet,
) -> SceneData {
    let fire_cone = build_fire_cone_sprites(large_sprites);
    // HUD font: `sprites/font.tga` is a plain uncompressed indexed TGA, so the
    // generic `Tga::load` parses it (7 × 250*8, de-flipped); `Font::load` runs the
    // `common.cpp:414-433` per-glyph post-process.
    let font_bytes = crate::assets::read_asset(tc_root, "sprites/font.tga");
    let font_tga = assets::sprite::Tga::load(&font_bytes).expect("font.tga parses");
    let font = Font::load(&font_tga);
    // HUD labels carried verbatim from the TC's `[texts]`.
    let labels = HudLabels {
        kills: tc.texts.Kills.clone(),
        lives: tc.texts.Lives.clone(),
        reloading: tc.texts.Reloading.clone(),
        killed_msg: tc.texts.KilledMsg.clone(),
        committed_suicide_msg: tc.texts.CommittedSuicideMsg.clone(),
    };
    SceneData {
        origpal,
        color_anim: tc.color_anim.clone(),
        fire_cone,
        nr_begin: tc.constants.NRColourBegin,
        nr_end: tc.constants.NRColourEnd,
        laser_weapon: tc.constants.LaserWeapon,
        font,
        labels,
    }
}
```

(keep the `// NOTE: killed_timer is left at its WormInit default …` comment above the `let scene` line).

- [ ] **Step 5: Implement `build.rs`** — above the tests:

```rust
//! Step 4½a-1 — the `MatchConfig -> SimState` builder (design §4).
//!
//! Produces the C++ `LocalController` start state: fresh worms (`visible = false`,
//! `killed_timer = 150`, `pos = (0,0)`, `worm.hpp:178-183`), `health = ws.health`
//! (`localController.cpp:35,42`), `stats_x` 0/218, `InitWeapons` from each worm's picks
//! (`worm.cpp:698-709`), `lives = settings->lives` (`localController.cpp:232-235`) and
//! the `StartGame` blood-pool size (`game.cpp:513`) — identical to `ResetWorms`
//! (`game.cpp:155-166`), which the oracle dumper's `settings` path runs.
//! `SimState::new`'s signature is unchanged; every other setting is a post-`new`
//! assignment (LD 5). The level arrives READY: level preparation (random vs file +
//! `MakeShadow`) is 4½b's `sim::levelgen::generate_from_settings`, composed in 4½d.

use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use render::viewport::Viewport;
use sim::control::ControlConsts;
use sim::physics::PhysicsConsts;
use sim::pool::BloodPool;
use sim::state::{SimState, WormInit};
use sim_core::vec::Vec2;

use crate::loader::{load_sprites, scene_data, Loaded};
use crate::settings::{MatchConfig, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE, WEAP_TABLE_LEN};

/// Why a `MatchConfig` cannot become a match. Every variant is a configuration a menu
/// or a file can produce, so it is an error, never a panic (design §4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// `game_mode == 2`: the sim's Holdazone arm is unported (`state.rs:2250-2252`).
    HoldazoneUnsupported,
    /// `game_mode > 3`.
    InvalidGameMode(u32),
    /// A playing worm's `health < 1` (C++ divides by it and loops on it in Scales).
    InvalidHealth(i32),
    /// The two players' healths differ: the sim carries one `settings_health` (design
    /// §1.3 finding 4); lifted with the player-menu HEALTH item in 4½f.
    AsymmetricHealth { p1: i32, p2: i32 },
    /// A weapon pick outside `1..=weap_order.len()` (`worm.cpp:704` indexes unchecked).
    InvalidWeapon { worm: usize, slot: usize, value: u32 },
    /// `blood_particle_max < 1` (a zero-cap blood pool).
    InvalidBloodParticleMax(i32),
    /// The TC has more weapons than `weap_table[40]` can index.
    TooManyWeapons(usize),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::HoldazoneUnsupported => write!(f, "Holdazone is not supported yet"),
            BuildError::InvalidGameMode(m) => write!(f, "unknown game mode {m}"),
            BuildError::InvalidHealth(h) => write!(f, "worm health {h} must be at least 1"),
            BuildError::AsymmetricHealth { p1, p2 } => {
                write!(f, "player healths differ ({p1} vs {p2}); not supported yet")
            }
            BuildError::InvalidWeapon { worm, slot, value } => {
                write!(f, "player {worm} weapon slot {slot}: {value} is not a weapon")
            }
            BuildError::InvalidBloodParticleMax(n) => {
                write!(f, "blood particle max {n} must be at least 1")
            }
            BuildError::TooManyWeapons(n) => write!(f, "the TC has {n} weapons; at most 40"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Check `cfg` against a TC with `n_weapons` weapons. Only the two playing worms
/// (indices 0 and 1) are checked; the network player never plays a local match.
pub fn validate(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError> {
    let s = &cfg.settings;
    if s.game_mode == GM_HOLDAZONE {
        return Err(BuildError::HoldazoneUnsupported);
    }
    if s.game_mode > GM_SCALES_OF_JUSTICE {
        return Err(BuildError::InvalidGameMode(s.game_mode));
    }
    let (p1, p2) = (s.worm_settings[0].health, s.worm_settings[1].health);
    for h in [p1, p2] {
        if h < 1 {
            return Err(BuildError::InvalidHealth(h));
        }
    }
    if p1 != p2 {
        return Err(BuildError::AsymmetricHealth { p1, p2 });
    }
    if n_weapons > WEAP_TABLE_LEN {
        return Err(BuildError::TooManyWeapons(n_weapons));
    }
    for worm in 0..2 {
        for (slot, &value) in s.worm_settings[worm].weapons.iter().enumerate() {
            if value == 0 || value as usize > n_weapons {
                return Err(BuildError::InvalidWeapon { worm, slot, value });
            }
        }
    }
    if s.blood_particle_max < 1 {
        return Err(BuildError::InvalidBloodParticleMax(s.blood_particle_max));
    }
    Ok(())
}

/// Build the tick-0 match for `cfg` on the ready `level` (design §4.2).
pub fn build_match(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
) -> Result<Loaded, BuildError> {
    let tc = TcConfig::load(&crate::assets::read_asset(tc_root, "tc.cfg")).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        Ok(crate::assets::read_asset(tc_root, &format!("{sub}/{id}.cfg")))
    })
    .expect("object configs load");
    validate(cfg, objects.weapons.len())?;
    let s = &cfg.settings;

    // weap_order: indices sorted by weapon name (Common::Precompute, common.cpp:491-499).
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let worms_init: Vec<WormInit> = (0..2)
        .map(|i| {
            let ws = &s.worm_settings[i];
            WormInit {
                index: i as i32,
                health: ws.health,
                lives: s.lives,
                stats_x: if i == 0 { 0 } else { 218 },
                weapons: WormInit::resolve_weapons(&objects, &weap_order, &ws.weapons),
                start_pos: Vec2::zero(),
                visible: false,
            }
        })
        .collect();

    let mut state = SimState::new(
        level,
        &worms_init,
        cfg.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_sprites(tc_root, "large.tga", 16, 16, 110),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        s.loading_time,
        s.load_change,
        s.blood,
    );

    // TC consts — the full post-`new` set (sim_slice6_fuzz.rs:199-235) + laser_weapon.
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_sprites(tc_root, "small.tga", 7, 7, 130);
    state.laser_weapon = tc.constants.LaserWeapon;
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state.bonus_drop_chance = tc.constants.BonusDropChance;
    state.bonus_spawn_rect_w = tc.constants.BonusSpawnRectW;
    state.bonus_spawn_rect_h = tc.constants.BonusSpawnRectH;
    state.bonus_spawn_rect_x = tc.constants.BonusSpawnRectX;
    state.bonus_spawn_rect_y = tc.constants.BonusSpawnRectY;
    state.h_bonus_spawn_rect = tc.hacks.BonusSpawnRect;
    state.h_bonus_only_health = tc.hacks.BonusOnlyHealth;
    state.h_bonus_only_weapon = tc.hacks.BonusOnlyWeapon;
    state.h_bonus_disable = tc.hacks.BonusDisable;
    state.bonus_rand_timer = [
        [tc.bonuses[0].timer, tc.bonuses[0].timer_v],
        [tc.bonuses[1].timer, tc.bonuses[1].timer_v],
    ];
    state.bonus_s_objects = [tc.bonuses[0].sobj, tc.bonuses[1].sobj];
    state.bonus_gravity = tc.constants.BonusGravity;
    state.bonus_bounce_mul = tc.constants.BonusBounceMul;
    state.bonus_bounce_div = tc.constants.BonusBounceDiv;
    state.bonus_health_var = tc.constants.BonusHealthVar;
    state.bonus_min_health = tc.constants.BonusMinHealth;
    state.bonus_explode_risk = tc.constants.BonusExplodeRisk;
    state.h_bonus_reload_only = tc.hacks.BonusReloadOnly;
    state.sound_hooks = tc.sound_hooks.clone();

    // Settings (design §4.2).
    state.settings_max_bonuses = s.max_bonuses;
    state.weap_table = s.weap_table.iter().map(|&v| v as i32).collect();
    state.settings_health = s.worm_settings[0].health; // == [1] (validated)
    state.game_mode = s.game_mode;
    state.time_to_lose = s.time_to_lose;
    state.shadow = s.shadow;
    state.bobjects = BloodPool::new(s.blood_particle_max as usize);

    // Palette (design §4.1): the level's POWERLEVEL palette only when
    // load_powerlevel_palette (level.cpp:281-294), else exepal == small.tga's palette
    // (level.cpp:385-392); Game::UpdateSettings makes it the renderer's (game.cpp:476).
    let small_tga = assets::sprite::Tga::load(&crate::assets::read_asset(tc_root, "sprites/small.tga"))
        .expect("small.tga parses");
    let origpal = match (&level.palette, s.load_powerlevel_palette) {
        (Some(pal), true) => pal.clone(),
        _ => small_tga.palette.clone(),
    };
    let scene = scene_data(tc_root, &tc, origpal, &state.large_sprites);
    Ok(Loaded {
        state,
        viewports: Viewport::player_layout(),
        scene,
    })
}
```

- [ ] **Step 6: Run the tests and the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario` — Expected: PASS (9 `build::tests` + everything prior).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test sim_slice4_5a_builder_golden` — Expected: PASS (1501 rows bit-exact). If it diverges, the first differing tick + component names the missing mapping; compare against `sim_slice6_fuzz.rs:129-238` field by field.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (the `scene_data` factor-out keeps every render golden byte-identical).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/build.rs` and the same for `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/sim_slice4_5a_builder_golden.rs` — Expected: no output (else format both and re-test).

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/build.rs rust/scenario/src/lib.rs rust/scenario/src/loader.rs rust/oracle-tests/tests/sim_slice4_5a_builder_golden.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-1): MatchConfig builder with refusals; reproduces sim_slice6_fuzz5" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Reviewer (Opus): the mapping table in design §4.2 is complete; `validate` refuses every `BuildError` case before any state is built; the loader factor-out is behaviour-neutral (render goldens green); the fuzz5 reproduction is the proof the TC-const set is complete.

---

### Task 7: The `settings <file>` directive — Rust parser + C++ dumper (12th column) + the `defaults` smoke golden  [Opus]

**Files:**
- Modify: `rust/scenario/src/parser.rs` — grammar doc (`:9-27`), `Scenario` field (`:53-98`), `parse` (`:103-275`), `to_text` (`:353-414`), tests at the end of `mod tests`
- Modify: `rust/scenario/src/loader.rs` — refuse at the top of `load`; a test
- Modify: `src/tools/oracle_dump/sim_physics_dump.cpp` — header grammar (`:38-60`), `Scenario` (`:169-232`), helpers, `ParseScenario` (`:244-338`), settings (`:369-387`), worms (`:421-464`), `dump` (`:1030-1043`)
- Create: `rust/oracle-tests/gen_sim_slice4_5a_golden.sh`, `rust/oracle-tests/golden/sim_slice4_5a_defaults_scenario.txt`, and (generated) `rust/oracle-tests/golden/sim_slice4_5a_defaults.txt`

**Interfaces:**
- Produces: `pub settings: Option<String>` on `scenario::Scenario` (used by T8, T9); the dumper's `settings` path (12-column output: the 11 classic columns + `Game::IsGameOver()` as `0`/`1`); `gen_sim_slice4_5a_golden.sh` looping over a variant list (T8 extends the list).
- Consumes: nothing from T1–T6 (the parser only stores the path).

Why: design §7.1–7.2. One optional directive instead of a settings language inside the frozen grammar; the C++ side exercises the REAL `Settings::FromToml`. Absent ⇒ every existing file parses to the same value and the dumper's classic path is byte-identical (proven by regenerating four existing goldens).

- [ ] **Step 1: Write the failing Rust tests** — append to the end of `mod tests` in `parser.rs`:

```rust
    const SETTINGS_SCN: &str =
        "seed 7\nlevel Levels/modern_test.lev\nticks 3\nsettings a_setup.cfg\ninput 1 16 0\n";

    #[test]
    fn settings_directive_parses_and_defaults_absent() {
        let s = Scenario::parse(SETTINGS_SCN).expect("parses");
        assert_eq!(s.settings.as_deref(), Some("a_setup.cfg"));
        assert!(s.worms.is_empty());
        let plain = Scenario::parse("seed 1\nlevel L\nticks 1\n").unwrap();
        assert_eq!(plain.settings, None, "absent => None: every existing scenario is unchanged");
    }

    #[test]
    fn settings_excludes_the_directives_it_replaces() {
        for extra in [
            "worm 0 0 0 100 10 0 0",
            "weapon 0 DART",
            "game_mode 1",
            "max_bonuses 0",
            "render player",
            "render_shadow",
            "render_shake 1 0 2",
            "render_flash 1 3",
            "render_hud",
            "render_live",
        ] {
            let text = format!("{SETTINGS_SCN}{extra}\n");
            assert!(Scenario::parse(&text).is_err(), "`settings` + `{extra}` must be rejected");
        }
    }

    #[test]
    fn settings_arity_and_duplicates_error() {
        assert!(Scenario::parse("seed 1\nlevel L\nticks 1\nsettings\n").is_err());
        assert!(Scenario::parse("seed 1\nlevel L\nticks 1\nsettings a b\n").is_err());
        assert!(Scenario::parse("seed 1\nlevel L\nticks 1\nsettings a\nsettings b\n").is_err());
    }

    #[test]
    fn to_text_round_trips_a_settings_scenario() {
        let s = Scenario::parse(SETTINGS_SCN).unwrap();
        let text = s.to_text();
        assert!(text.contains("settings a_setup.cfg\n"));
        assert!(!text.contains("game_mode") && !text.contains("max_bonuses"));
        assert_eq!(Scenario::parse(&text).unwrap(), s);
    }
```

and append to `mod tests` in `loader.rs`:

```rust
    #[test]
    #[should_panic(expected = "settings")]
    fn load_refuses_a_settings_scenario() {
        let s = Scenario::parse("seed 1\nlevel Levels/render_stage.lev\nticks 1\nsettings x.cfg\n")
            .expect("parses");
        let _ = load(Path::new(TC_ROOT), &s);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario settings`
Expected: FAIL to compile — `no field settings on type Scenario`.

- [ ] **Step 3: Implement the Rust parser**

Grammar doc: after the `render_live` line add
`//! settings    <file>                             # Step 4½a-1; oracle-only setup sidecar (see below)`.

`Scenario`: after `pub worms: Vec<ScenarioWorm>,` add

```rust
    /// Step-4½a-1 `settings <file>` — the oracle-harness-only setup sidecar (a
    /// C++-schema TOML file; the path is relative to the scenario file's directory).
    /// Absent => `None` (every pre-4½ scenario, byte-identical meaning). Present => the
    /// C++ dumper reads it with the real `Settings::FromToml` and starts the worms in
    /// the C++ LocalController state; the Rust golden test uses
    /// `settings_toml::settings_from_toml` + `build::build_match`. A settings scenario
    /// rejects `worm`/`weapon`/`game_mode`/`max_bonuses`/`render*`, and
    /// [`crate::load`] refuses it (design §7.1).
    pub settings: Option<String>,
```

In `parse`, next to the other locals add:

```rust
        let mut settings: Option<String> = None;
        let mut game_mode_given = false;
        let mut max_bonuses_given = false;
        let mut render_given = false;
```

set `max_bonuses_given = true;` in the `"max_bonuses"` arm, `game_mode_given = true;` in the `"game_mode"` arm, `render_given = true;` in the `"render"` arm, and add a new arm before `"worm"`:

```rust
                "settings" => {
                    // Step 4½a-1: the oracle-only setup sidecar (design §7.1).
                    expect_args(n, key, &nums, 1)?;
                    if settings.replace(nums[0].to_string()).is_some() {
                        return Err(format!("line {n}: duplicate `settings`"));
                    }
                }
```

After the `for` loop, before `Ok(Scenario { … })`:

```rust
        if settings.is_some()
            && (!worms.is_empty()
                || !weapons.is_empty()
                || game_mode_given
                || max_bonuses_given
                || render_given
                || render_shadow
                || !render_shake.is_empty()
                || !render_flash.is_empty()
                || render_hud
                || render_live)
        {
            return Err(
                "`settings` excludes the worm, weapon, game_mode, max_bonuses and render* directives"
                    .to_string(),
            );
        }
```

and add `settings,` to the `Ok(Scenario { … })` literal. In `to_text`, replace the two unconditional lines `out.push_str(&format!("max_bonuses {}\n", self.max_bonuses));` and `out.push_str(&format!("game_mode {}\n", self.game_mode));` with:

```rust
        match &self.settings {
            // A settings scenario carries mode/bonuses in its setup; the parser rejects
            // the two directives alongside `settings`.
            Some(path) => out.push_str(&format!("settings {path}\n")),
            None => {
                out.push_str(&format!("max_bonuses {}\n", self.max_bonuses));
                out.push_str(&format!("game_mode {}\n", self.game_mode));
            }
        }
```

In `loader.rs`, make the first statement of `load`:

```rust
    assert!(
        scenario.settings.is_none(),
        "scenario::load refuses a `settings` scenario: build it with \
         scenario::build::build_match (design §7.1)"
    );
```

- [ ] **Step 4: Run the Rust tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario` — Expected: PASS (4 new parser tests + the loader refusal + every prior test, incl. `to_text_round_trips_every_directive`).

- [ ] **Step 5: Write the smoke scenario and the generator script (the C++ RED)**

Create `rust/oracle-tests/golden/sim_slice4_5a_defaults_scenario.txt`:

```text
# Step 4½ slice 4½a-1 T7 — the SETTINGS-DRIVEN smoke (design §7.3 `defaults`): the
# shipped C++ default setup `data/Setups/liero.cfg` (a legacy v5 file: no rgbDepth,
# 6-bit rgb) read by BOTH the C++ dumper's `settings` path (the real
# Settings::FromToml) and the Rust `settings_toml::settings_from_toml`, then built into
# the C++ LocalController start state (worms invisible at (0,0), killed_timer 150,
# health = WormSettings::health 100, lives = Settings::lives 15) by the dumper /
# `scenario::build::build_match`. No input: both worms count down and respawn
# (`ready` is true from the Worm ctor) with the DoRespawning dirt puff + CorrectShadow
# (shadow = true, the C++ default), under the default bonus roll (max_bonuses 4).
# The golden has 12 columns: the 11 classic hashes + Game::IsGameOver() (0/1).
seed 7
level Levels/modern_test.lev
ticks 400
settings ../../../data/Setups/liero.cfg
```

Create `rust/oracle-tests/gen_sim_slice4_5a_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5a_<variant>.txt — 12 columns: the 11 oracle_dump_sim_physics
# hash columns + Game::IsGameOver() — for the Step-4½ slice-4½a-1 SETTINGS-DRIVEN scenarios.
# Each scenario's `settings <file>` sidecar is read by the REAL C++ Settings::FromToml and the
# worms start in the C++ LocalController state (sim_physics_dump.cpp `settings` path).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run
# in the lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
for v in defaults; do
  "build/$PRESET/Release/oracle_dump_sim_physics" \
    "rust/oracle-tests/golden/sim_slice4_5a_${v}_scenario.txt" \
    "rust/oracle-tests/golden/sim_slice4_5a_${v}.txt"
  echo "wrote rust/oracle-tests/golden/sim_slice4_5a_${v}.txt"
done
```

Run: `chmod +x /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice4_5a_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice4_5a_golden.sh`
Expected: FAIL — the (unchanged) dumper exits 1 with `unknown scenario key: settings`. (The FIRST configure in this worktree bootstraps vcpkg and is slow; to reuse the main checkout's vcpkg, run the same command prefixed with `VCPKG_ROOT=/Users/john/code/openliero/tools/vcpkg/vcpkg `.)

- [ ] **Step 6: Implement the dumper `settings` path** (`src/tools/oracle_dump/sim_physics_dump.cpp`)

Header grammar: after the `render_live` lines (`:58-60`) add

```cpp
//   settings <file>                    (Step 4½a-1; a C++-schema setup file, relative to the
//                                       scenario's directory, read by the REAL
//                                       Settings::FromToml. Worms then start in the C++
//                                       LocalController state (no `worm` lines allowed) and
//                                       every dump line gains a 12th column, IsGameOver 0/1.
//                                       Excludes worm/weapon/game_mode/max_bonuses/render*.)
```

`struct Scenario`: after `bool render_live = false;` add

```cpp
  // Step 4½a-1 `settings <file>` (design §7.1): the setup file read by the REAL
  // Settings::FromToml; the worms start in the C++ LocalController state and the dump gains a
  // 12th IsGameOver column. Empty (every pre-4½ scenario) => the classic path, byte-identical.
  std::string settings_file;
  // Presence flags for the directives a `settings` scenario must not carry.
  bool game_mode_given = false;
  bool max_bonuses_given = false;
```

In the anonymous namespace, after `SlurpFile`, add

```cpp
// Step 4½a-1: the directory part of `path` ("." when there is none), so the `settings`
// file resolves relative to the scenario file.
std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

// Step 4½a-1: read a whole text file (the `settings` sidecar).
std::string ReadTextFile(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    std::fprintf(stderr, "cannot open settings %s\n", path.c_str());
    std::exit(1);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}
```

`ParseScenario`: in the `max_bonuses` arm add `s.max_bonuses_given = true;`; in the `game_mode` arm add `s.game_mode_given = true;`; add an arm before `worm`:

```cpp
    } else if (key == "settings") {
      // Step 4½a-1: the oracle-only setup sidecar (see Scenario::settings_file).
      if (!s.settings_file.empty()) {
        std::fprintf(stderr, "duplicate settings directive\n");
        std::exit(1);
      }
      ls >> s.settings_file;
```

and replace the `if (s.worms.size() != 2) { … }` check with

```cpp
  if (!s.settings_file.empty()) {
    bool const kForbidden = !s.worms.empty() || !s.weapon_overrides.empty() || s.game_mode_given ||
                            s.max_bonuses_given || !s.render_layout.empty() || s.render_shadow ||
                            !s.render_shake.empty() || !s.render_flash.empty() || s.render_hud ||
                            s.render_live;
    if (kForbidden) {
      std::fprintf(stderr, "settings excludes worm/weapon/game_mode/max_bonuses/render* directives\n");
      std::exit(1);
    }
  } else if (s.worms.size() != 2) {
    std::fprintf(stderr, "scenario must define exactly 2 worms (got %zu)\n", s.worms.size());
    std::exit(1);
  }
```

`main`, settings: keep `auto settings = std::make_shared<Settings>();`, then wrap the existing lines from `// Game-mode switch (game.cpp:372-461) is driven by …` through `settings->max_bonuses = scn.max_bonuses;` UNCHANGED (only re-indented two spaces) as the `else` branch of:

```cpp
  if (!scn.settings_file.empty()) {
    // Step 4½a-1: the REAL C++ setup reader. Every sim-reaching field (game_mode, lives,
    // loading_time, blood, load_change, shadow, max_bonuses, weap_table, time_to_lose,
    // blood_particle_max, per-worm health + weapons) comes from the file.
    std::string const kCfgPath = DirOf(argv[1]) + "/" + scn.settings_file;
    try {
      settings->FromToml(ReadTextFile(kCfgPath));
    } catch (std::exception const& e) {
      std::fprintf(stderr, "settings %s: %s\n", kCfgPath.c_str(), e.what());
      return 1;
    }
    if (settings->game_mode == Settings::kGmHoldazone) {
      std::fprintf(stderr, "settings %s: Holdazone is refused (unported in Rust)\n",
                   kCfgPath.c_str());
      return 1;
    }
  } else {
    // … the existing classic-path lines, verbatim …
  }
```

`main`, worms: wrap the existing three blocks — from `// Add 2 worms exactly as the determinism fixture` through the end of the `// Apply scenario start conditions …` loop — UNCHANGED (re-indented) as the `else` branch of:

```cpp
  if (!scn.settings_file.empty()) {
    // Step 4½a-1: the C++ LocalController start state (localController.cpp:30-54: health =
    // ws.health, stats_x 0/218) + weapsel Finalize's InitWeapons (weapsel.cpp:352-356) + the
    // kStateGame lives (localController.cpp:232-235), reached via ResetWorms
    // (game.cpp:155-166), which yields the identical state: visible = false, pos (0,0),
    // killed_timer 150, current_weapon 0. No worm-line overrides.
    for (int idx = 0; idx < 2; ++idx) {
      auto w = std::make_shared<Worm>();
      w->settings = settings->worm_settings[idx];
      w->health = w->settings->health;
      w->index = idx;
      w->stats_x = idx == 0 ? 0 : 218;
      game.AddWorm(w);
    }
    for (auto const& w : game.worms) {
      w->InitWeapons(game);
    }
    game.ResetWorms();
  } else {
    // … the existing worm-creation, InitWeapons/ResetWorms and start-condition blocks, verbatim …
  }
```

`dump`: replace its `std::fprintf(out, "%d %08x … %08x\n", …);` with

```cpp
    std::fprintf(out, "%d %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x", tick, state_hash,
                 c.rng, c.level, c.worms[0], c.worms[1], c.bobjects, c.bonuses, c.sobjects,
                 c.nobjects, c.wobjects);
    // Step 4½a-1: the settings path adds Game::IsGameOver() (game.cpp:521-544) as column 12.
    if (!scn.settings_file.empty()) {
      std::fprintf(out, " %d", game.IsGameOver() ? 1 : 0);
    }
    std::fprintf(out, "\n");
```

- [ ] **Step 7: Generate the smoke golden (GREEN) and prove the classic path byte-identical**

Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice4_5a_golden.sh` — Expected: `wrote rust/oracle-tests/golden/sim_slice4_5a_defaults.txt`; the file has 401 lines of 12 columns, column 12 all `0` (lives 15).
Run each of: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_fuzz5.sh`, `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_scales_golden.sh`, `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice4d_live.sh`, `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice3e_hud.sh`.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden`
Expected: exactly `?? rust/oracle-tests/golden/sim_slice4_5a_defaults.txt` and `?? rust/oracle-tests/golden/sim_slice4_5a_defaults_scenario.txt` — no modified (`M`) golden.
Run: `clang-format --version` — must report version 22; then `clang-format --dry-run -Werror /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/sim_physics_dump.cpp` — Expected: no output, exit 0 (fix any report with `clang-format -i` on that one file and re-run step 7's gen scripts).
If `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/build/macos-arm64/compile_commands.json` exists, run `clang-tidy -p /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/build/macos-arm64 /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/sim_physics_dump.cpp` — Expected: no new warnings in the edited regions (CI's tidy job is the backstop).

- [ ] **Step 8: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/parser.rs rust/scenario/src/loader.rs src/tools/oracle_dump/sim_physics_dump.cpp rust/oracle-tests/gen_sim_slice4_5a_golden.sh rust/oracle-tests/golden/sim_slice4_5a_defaults_scenario.txt rust/oracle-tests/golden/sim_slice4_5a_defaults.txt
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5a-1): settings <file> directive (C++ FromToml path + IsGameOver column) + defaults smoke golden" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Reviewer (Opus): both parsers accept/reject the same set; the classic path is untouched (four regenerated goldens byte-identical); the settings path's worm setup is `LocalController` + `ResetWorms`; column 12 exists only on the settings path.

---

### Task 8: Generator + setup sidecars + scenarios + C++ goldens for `killemall` / `scales` / `gametag`  [Opus]

**Files:**
- Create: `rust/oracle-tests/examples/gen_slice4_5a.rs`
- Create (generated, committed): `rust/oracle-tests/golden/sim_slice4_5a_{killemall,scales,gametag}_setup.cfg`, `…_scenario.txt`, and (C++) `…/sim_slice4_5a_{killemall,scales,gametag}.txt`
- Modify: `rust/oracle-tests/gen_sim_slice4_5a_golden.sh` (the `for v in …` list)

**Interfaces:**
- Consumes: T2 `settings_from_toml`; T1 `MatchConfig`; T6 `build_match`; T5 `is_game_over`; T3/T4 sim rules (so the Rust scan is faithful); T7's directive and dumper path.
- Produces: three committed settings-driven scenarios + sidecars + 12-column goldens (read by T9).

Why: design §7.3. The sidecars set every sim-reaching field to a non-default value (the coverage table) and every non-sim field to a non-default value too; loadouts use only golden-proven weapons; the five deferred-branch weapons are banned from bonuses. The generator resolves weapon NAMES to indices at run time, scans game seeds through the Rust builder with a pre-expanded random input stream, and keeps the first seed whose run shows the variant's witnesses. It runs in a DEBUG build on purpose: every deferred sim branch is a `debug_assert!`, and a seed that trips one is skipped, never silently kept.

- [ ] **Step 1: Write the generator** — create `rust/oracle-tests/examples/gen_slice4_5a.rs`:

```rust
//! Step 4½a-1 T8 — setup-sidecar writer, seed scanner and scenario writer for the
//! SETTINGS-DRIVEN goldens (design §7.3). A dev tool, not a test; not run in CI.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5a -- cfg  <variant> <out_setup.cfg>
//!   cargo run -p oracle-tests --example gen_slice4_5a -- scan <variant> <input_seed> <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5a -- gen  <variant> <game_seed> <input_seed> <out_scenario.txt>
//!
//! Variants: killemall, scales, gametag. Run it in a DEBUG build: every deferred sim
//! branch is a `debug_assert!`, and `scan` skips (and reports) a seed that trips one.
//! Inputs: per tick `Rand(input_seed).next_u32() & 0x7f` for worm 0 then worm 1,
//! applied on the pass advancing t -> t+1 (the dumper seam, as the slice-6 fuzz).

use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};
use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use scenario::build::build_match;
use scenario::settings::MatchConfig;
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::hash_components;
use sim::state::ControlState;
use sim_core::rng::Rand;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const LEVEL: &str = "Levels/modern_test.lev";
/// Scan horizon; a kept seed ends its match by `MAX_TICKS - POST_MORTEM_MARGIN`.
const MAX_TICKS: u32 = 3000;
/// Ticks recorded after the game-over tick (>= the 180-frame C++ post-mortem).
const POST_MORTEM_MARGIN: u32 = 200;
/// Deferred Step-2 sim branches (design §1.3 finding 8): shotType 4 (the laser
/// do-loop) + MISSILE (ProcessSteerables). `weap_table = 2`, never in a loadout.
const BANNED: [&str; 5] = ["RIFLE", "WINCHESTER", "LASER", "GAUSS GUN", "MISSILE"];
/// `weap_table = 1` (bonus only): sim-inert until weapon selection, varied for coverage.
const BONUS_ONLY: [&str; 3] = ["DOOMSDAY", "HELLRAIDER", "CHIQUITA BOMB"];

struct Variant {
    name: &'static str,
    game_mode: u32,
    lives: i32,
    health: i32,
    loading_time: i32,
    blood: i32,
    load_change: bool,
    max_bonuses: i32,
    shadow: bool,
    time_to_lose: i32,
    blood_particle_max: i32,
    p1: [&'static str; 5],
    p2: [&'static str; 5],
}

/// Design §7.3's coverage table. Loadouts: golden-proven weapons only.
const VARIANTS: [Variant; 3] = [
    Variant {
        name: "killemall",
        game_mode: 0,
        lives: 1,
        health: 150,
        loading_time: 37,
        blood: 250,
        load_change: false,
        max_bonuses: 6,
        shadow: true,
        time_to_lose: 600,
        blood_particle_max: 300,
        p1: ["BAZOOKA", "DART", "CANNON", "GRENADE", "HANDGUN"],
        p2: ["EXPLOSIVES", "GREENBALL", "FAN", "BAZOOKA", "DART"],
    },
    Variant {
        name: "scales",
        game_mode: 3,
        lives: 2,
        health: 120,
        loading_time: 150,
        blood: 60,
        load_change: true,
        max_bonuses: 8,
        shadow: true,
        time_to_lose: 600,
        blood_particle_max: 500,
        p1: ["GRENADE", "FAN", "EXPLOSIVES", "HANDGUN", "CANNON"],
        p2: ["DART", "BAZOOKA", "GREENBALL", "GRENADE", "FAN"],
    },
    Variant {
        name: "gametag",
        game_mode: 1,
        lives: 3,
        health: 80,
        loading_time: 0,
        blood: 0,
        load_change: true,
        max_bonuses: 3,
        shadow: false,
        time_to_lose: 12,
        blood_particle_max: 700,
        p1: ["CANNON", "BAZOOKA", "DART", "FAN", "GREENBALL"],
        p2: ["HANDGUN", "EXPLOSIVES", "GRENADE", "CANNON", "BAZOOKA"],
    },
];

fn variant(name: &str) -> &'static Variant {
    VARIANTS.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("unknown variant {name}"))
}

fn load_objects() -> Objects {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    Objects::load(&tc.types, |sub, id| std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))).unwrap()
}

fn load_level() -> LevelData {
    assets::level::load(&std::fs::read(format!("{TC_ROOT}/{LEVEL}")).unwrap()).unwrap()
}

/// Index into `objects.weapons` (== the `weap_table` index, tc.cfg `[types] weapons`).
fn weapon_index(o: &Objects, name: &str) -> usize {
    o.weapons.iter().position(|w| w.name == name).unwrap_or_else(|| panic!("no weapon {name:?}"))
}

/// The 1-based `weap_order` index a `WormSettings.weapons` entry stores.
fn menu_index(o: &Objects, name: &str) -> u32 {
    let mut order: Vec<usize> = (0..o.weapons.len()).collect();
    order.sort_by(|&a, &b| o.weapons[a].name.cmp(&o.weapons[b].name));
    order.iter().position(|&i| o.weapons[i].name == name).unwrap() as u32 + 1
}

fn arr(v: &[u32]) -> String {
    let items: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("[ {} ]", items.join(", "))
}

struct WormCfg {
    header: &'static str,
    color: i32,
    controls: [u32; 7],
    dig: u32,
    gamepad: [u32; 8],
    pad_name: &'static str,
    pad_serial: &'static str,
    input_device: u32,
    name: &'static str,
    rgb: [u32; 3],
}

/// Non-default values for every non-sim worm field (fixed across variants).
const WORMS: [WormCfg; 3] = [
    WormCfg {
        header: "network_player",
        color: 44,
        controls: [19, 33, 32, 34, 29, 42, 56],
        dig: 58,
        gamepad: [11, 12, 13, 14, 110, 10, 0, 9],
        pad_name: "Pad Net",
        pad_serial: "SN-3",
        input_device: 2,
        name: "Netty",
        rgb: [200, 100, 50],
    },
    WormCfg {
        header: "player1",
        color: 33,
        controls: [76, 80, 79, 81, 163, 168, 165],
        dig: 54,
        gamepad: [1, 2, 3, 4, 5, 6, 7, 8],
        pad_name: "Pad One",
        pad_serial: "SN-1",
        input_device: 0,
        name: "Lefty",
        rgb: [250, 10, 128],
    },
    WormCfg {
        header: "player2",
        color: 42,
        controls: [17, 31, 30, 32, 20, 21, 22],
        dig: 57,
        gamepad: [21, 22, 23, 24, 125, 20, 1, 19],
        pad_name: "Pad Two",
        pad_serial: "SN-2",
        input_device: 1,
        name: "Righty",
        rgb: [12, 240, 99],
    },
];

/// The setup sidecar, in toml++'s canonical layout (sorted tables and keys, literal
/// strings, `weapTable` multiline) so 4½a-2's byte gate can reuse it as an input.
fn setup_cfg(v: &Variant, o: &Objects) -> String {
    for n in v.p1.iter().chain(v.p2.iter()) {
        assert!(!BANNED.contains(n), "{n} is a deferred-branch weapon");
    }
    let mut table = [0u32; 40];
    for n in BANNED {
        table[weapon_index(o, n)] = 2;
    }
    for n in BONUS_ONLY {
        table[weapon_index(o, n)] = 1;
    }
    let pick = |names: &[&str; 5]| -> [u32; 5] { names.map(|n| menu_index(o, n)) };
    let loadouts = [[2, 3, 4, 5, 6], pick(&v.p1), pick(&v.p2)];
    let healths = [100, v.health, v.health];

    let mut out = String::new();
    for (i, w) in WORMS.iter().enumerate() {
        let mut ex = [0u32; 8];
        ex[..7].copy_from_slice(&w.controls);
        ex[7] = w.dig;
        out.push_str(&format!("[{}]\n", w.header));
        out.push_str(&format!("color = {}\n", w.color));
        out.push_str("controller = 0\n");
        out.push_str(&format!("controls = {}\n", arr(&w.controls)));
        out.push_str(&format!("controlsEx = {}\n", arr(&ex)));
        out.push_str(&format!("gamepadControls = {}\n", arr(&w.gamepad)));
        out.push_str(&format!("gamepadName = '{}'\n", w.pad_name));
        out.push_str(&format!("gamepadSerial = '{}'\n", w.pad_serial));
        out.push_str(&format!("health = {}\n", healths[i]));
        out.push_str(&format!("inputDevice = {}\n", w.input_device));
        out.push_str(&format!("name = '{}'\n", w.name));
        out.push_str("randomName = false\n");
        out.push_str(&format!("rgb = {}\n", arr(&w.rgb)));
        out.push_str("rgbDepth = 8\n");
        out.push_str(&format!("weapons = {}\n\n", arr(&loadouts[i])));
    }
    let s = |b: bool| if b { "true" } else { "false" };
    out.push_str("[settings]\n");
    out.push_str("aiFrames = 99\naiMutations = 5\naiParallels = 7\naiTraces = true\n");
    out.push_str("allowViewingSpawnPoint = true\n");
    out.push_str(&format!("blood = {}\n", v.blood));
    out.push_str(&format!("bloodParticleMax = {}\n", v.blood_particle_max));
    out.push_str("bonusTimeout = 45\nflagsToWin = 7\nfullscreen = true\n");
    out.push_str(&format!("gameMode = {}\n", v.game_mode));
    out.push_str("inputDelay = 3\nlevelFile = 'Levels/modern_test.lev'\n");
    out.push_str(&format!("lives = {}\n", v.lives));
    out.push_str(&format!("loadChange = {}\n", s(v.load_change)));
    out.push_str("loadPowerlevelPalette = false\n");
    out.push_str(&format!("loadingTime = {}\n", v.loading_time));
    out.push_str("map = false\n");
    out.push_str(&format!("maxBonuses = {}\n", v.max_bonuses));
    out.push_str("maxSpectatorRenderHeight = 720\nmodernColors = true\nnamesOnBonuses = true\n");
    out.push_str("randomLevel = false\nrandomMapHeight = 400\nrandomMapWidth = 640\n");
    out.push_str("recordReplays = false\nregenerateLevel = true\nscreenSync = false\n");
    out.push_str("selectBotWeapons = 2\n");
    out.push_str(&format!("shadow = {}\n", s(v.shadow)));
    out.push_str("singleScreenReplay = true\nspectatorWindow = true\ntc = 'openliero'\n");
    out.push_str(&format!("timeToLose = {}\n", v.time_to_lose));
    out.push_str("version = 6\nweapTable = [\n");
    let rows: Vec<String> = table.iter().map(|t| format!("    {t}")).collect();
    out.push_str(&rows.join(",\n"));
    out.push_str("\n]\nzoneTimeout = 90\n");
    out
}

fn inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    (0..ticks).map(|_| [r.next_u32() & 0x7f, r.next_u32() & 0x7f]).collect()
}

/// Everything the witnesses read, from the genuinely driven Rust state.
#[derive(Default, Debug)]
struct Ledger {
    game_over_tick: Option<u32>,
    deaths: u32,
    respawns: u32,
    peak_bobjects: usize,
    reload_started: bool,
    bonus_dropped: bool,
    scales_death_kept_health: bool,
    life_gained: bool,
    timer_bumped: bool,
    level_series: Vec<u32>,
}

fn drive(cfg: &MatchConfig, level: &LevelData, ins: &[[u32; 2]]) -> Ledger {
    let mut st = build_match(Path::new(TC_ROOT), cfg, level).expect("variant config builds").state;
    let mut l = Ledger { level_series: vec![hash_components(&st).level], ..Ledger::default() };
    for (t, w) in ins.iter().enumerate() {
        let prev: Vec<(bool, i32, i32)> = st.worms.iter().map(|w| (w.visible, w.lives, w.timer)).collect();
        st.process_frame(&[ControlState::unpack(w[0]), ControlState::unpack(w[1])]);
        let k = t as u32 + 1;
        l.level_series.push(hash_components(&st).level);
        for (i, w) in st.worms.iter().enumerate() {
            let (was_visible, lives, timer) = prev[i];
            if was_visible && !w.visible {
                l.deaths += 1;
                if w.health > 0 {
                    l.scales_death_kept_health = true;
                }
            }
            if !was_visible && w.visible {
                l.respawns += 1;
            }
            l.life_gained |= w.lives > lives;
            l.timer_bumped |= w.timer > timer;
            l.reload_started |= w.weapons.iter().any(|ww| ww.loading_left > 0);
        }
        l.peak_bobjects = l.peak_bobjects.max(st.bobjects.len());
        l.bonus_dropped |= !st.bonuses.is_empty();
        if l.game_over_tick.is_none() && is_game_over(&st) {
            l.game_over_tick = Some(k);
        }
        if let Some(g) = l.game_over_tick {
            if k >= g + POST_MORTEM_MARGIN {
                break;
            }
        }
    }
    l
}

/// The variant's witnesses (design §7.3). `shadow_fired`: the level column differs
/// from a `shadow = false` re-run of the same inputs.
fn ok(v: &Variant, l: &Ledger, shadow_fired: bool) -> bool {
    let ends = matches!(l.game_over_tick, Some(g) if g + POST_MORTEM_MARGIN <= MAX_TICKS);
    let base = ends && l.deaths >= 1 && l.respawns >= 2;
    base && match v.name {
        "killemall" => {
            shadow_fired
                && l.peak_bobjects == v.blood_particle_max as usize
                && l.reload_started
                && l.bonus_dropped
        }
        "scales" => shadow_fired && l.scales_death_kept_health && l.life_gained,
        "gametag" => l.timer_bumped,
        _ => unreachable!(),
    }
}

struct Run {
    ledger: Ledger,
    shadow_fired: bool,
}

fn run(v: &Variant, game_seed: u32, input_seed: u32) -> Option<Run> {
    let objects = load_objects();
    let level = load_level();
    let settings = settings_from_toml(&setup_cfg(v, &objects)).expect("sidecar parses");
    let cfg = MatchConfig { settings, seed: game_seed };
    let ins = inputs(input_seed, MAX_TICKS);
    catch_unwind(AssertUnwindSafe(|| {
        let ledger = drive(&cfg, &level, &ins);
        let mut off = cfg.clone();
        off.settings.shadow = false;
        let off_ledger = drive(&off, &level, &ins);
        let n = ledger.level_series.len().min(off_ledger.level_series.len());
        let shadow_fired = ledger.level_series[..n] != off_ledger.level_series[..n];
        Run { ledger, shadow_fired }
    }))
    .ok()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).unwrap_or_else(|| panic!("missing argument {i}")).as_str();
    let num = |i: usize| arg(i).parse::<u32>().unwrap_or_else(|e| panic!("argument {i}: {e}"));
    match arg(0) {
        "cfg" => {
            let text = setup_cfg(variant(arg(1)), &load_objects());
            std::fs::write(arg(2), text).expect("write setup");
            println!("wrote {}", arg(2));
        }
        "scan" => {
            let v = variant(arg(1));
            let input_seed = num(2);
            set_hook(Box::new(|_| {})); // a deferred-branch panic is reported below
            for game_seed in num(3)..=num(4) {
                match run(v, game_seed, input_seed) {
                    None => println!("{} game_seed={game_seed} PANIC (deferred sim branch)", v.name),
                    Some(r) => println!(
                        "{} game_seed={game_seed} ok={} go={:?} deaths={} respawns={} peak_bob={} \
                         reload={} bonus={} scales_death={} life_gained={} timer={} shadow={}",
                        v.name,
                        ok(v, &r.ledger, r.shadow_fired),
                        r.ledger.game_over_tick,
                        r.ledger.deaths,
                        r.ledger.respawns,
                        r.ledger.peak_bobjects,
                        r.ledger.reload_started,
                        r.ledger.bonus_dropped,
                        r.ledger.scales_death_kept_health,
                        r.ledger.life_gained,
                        r.ledger.timer_bumped,
                        r.shadow_fired,
                    ),
                }
            }
        }
        "gen" => {
            let v = variant(arg(1));
            let (game_seed, input_seed) = (num(2), num(3));
            let r = run(v, game_seed, input_seed).expect("the seed must not trip a deferred branch");
            assert!(ok(v, &r.ledger, r.shadow_fired), "seed fails the witnesses: {:?}", r.ledger);
            let g = r.ledger.game_over_tick.unwrap();
            let ticks = g + POST_MORTEM_MARGIN;
            let l = &r.ledger;
            let mut out = format!(
                "# Step 4½ slice 4½a-1 T8 — SETTINGS-DRIVEN match `{name}` (design §7.3). Read by BOTH\n\
                 # the C++ dumper (oracle_dump_sim_physics `settings` path: the real Settings::FromToml\n\
                 # + the LocalController start state) and the Rust golden test (settings_toml reader +\n\
                 # scenario::build::build_match). Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5a -- gen {name} {game_seed} {input_seed} <this file>\n\
                 # LEDGER (Rust, driven state): game over at tick {g}; deaths={} respawns={} peak_bobjects={}\n\
                 #   reload={} bonus={} scales_death_kept_health={} life_gained={} timer_bumped={} shadow_fired={}\n\
                 # Inputs: per tick Rand(input_seed).next_u32() & 0x7f, worm 0 then worm 1 (zero pairs omitted).\n\
                 seed {game_seed}\nlevel {LEVEL}\nticks {ticks}\nsettings sim_slice4_5a_{name}_setup.cfg\n",
                l.deaths,
                l.respawns,
                l.peak_bobjects,
                l.reload_started,
                l.bonus_dropped,
                l.scales_death_kept_health,
                l.life_gained,
                l.timer_bumped,
                r.shadow_fired,
                name = v.name,
            );
            for (t, w) in inputs(input_seed, ticks).iter().enumerate() {
                if w[0] != 0 || w[1] != 0 {
                    out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
                }
            }
            std::fs::write(arg(4), out).expect("write scenario");
            println!("wrote {} (game over at tick {g}, ticks {ticks})", arg(4));
        }
        other => panic!("unknown command {other:?} (cfg | scan | gen)"),
    }
}
```

- [ ] **Step 2: Build it and write the sidecars**

Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5a` — Expected: builds without warnings.
Run (three times, one per variant): `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5a -- cfg killemall /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/sim_slice4_5a_killemall_setup.cfg` (then `scales` → `…_scales_setup.cfg`, `gametag` → `…_gametag_setup.cfg`).
Expected: `wrote …`; each file starts `[network_player]` and ends `zoneTimeout = 90`.

- [ ] **Step 3: Scan for seeds**

Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5a -- scan killemall 4545 1 200` (and the same for `scales`, `gametag`).
Expected: one line per game seed; pick the SMALLEST game seed with `ok=true` per variant. If a variant has none in 1..=200, scan 201..=1000 with the same input seed; if still none, repeat 1..=1000 with input seed 9191. Record the chosen `(game_seed, input_seed)` in the done-report.

- [ ] **Step 4: Write the scenarios**

Run (per variant, with its chosen seeds): `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5a -- gen killemall <game_seed> <input_seed> /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/sim_slice4_5a_killemall_scenario.txt`
Expected: `wrote … (game over at tick G, ticks G+200)`.

- [ ] **Step 5: Generate the C++ goldens**

In `gen_sim_slice4_5a_golden.sh` change `for v in defaults; do` to `for v in defaults killemall scales gametag; do`.
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice4_5a_golden.sh`
Expected: four `wrote …` lines; `sim_slice4_5a_defaults.txt` is unchanged (`git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` lists only the nine new `killemall/scales/gametag` files as `??`); in each new golden, column 12 flips from `0` to `1` on exactly one line and stays `1` for the remaining ≥ 200 lines.

- [ ] **Step 6: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/examples/gen_slice4_5a.rs rust/oracle-tests/gen_sim_slice4_5a_golden.sh rust/oracle-tests/golden/sim_slice4_5a_killemall_setup.cfg rust/oracle-tests/golden/sim_slice4_5a_killemall_scenario.txt rust/oracle-tests/golden/sim_slice4_5a_killemall.txt rust/oracle-tests/golden/sim_slice4_5a_scales_setup.cfg rust/oracle-tests/golden/sim_slice4_5a_scales_scenario.txt rust/oracle-tests/golden/sim_slice4_5a_scales.txt rust/oracle-tests/golden/sim_slice4_5a_gametag_setup.cfg rust/oracle-tests/golden/sim_slice4_5a_gametag_scenario.txt rust/oracle-tests/golden/sim_slice4_5a_gametag.txt
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5a-1): settings-driven killemall/scales/gametag scenarios, sidecars and C++ goldens" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Reviewer (Opus): every sim-reaching field in design §7.3's table is non-default in some sidecar; no BANNED weapon in any loadout; the scenarios carry no `worm`/`weapon` lines; each golden's IsGameOver flip is followed by ≥ 200 rows.

---

### Task 9: MILESTONE — four settings-driven matches bit-exact vs C++, incl. `IsGameOver`  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/sim_slice4_5a_settings_golden.rs`

**Interfaces:**
- Consumes: T7 `Scenario::settings`, T2 `settings_from_toml`, T6 `build_match`, T5 `is_game_over`, the goldens of T7 + T8.
- Produces: Hard gate 1 (design done-when 5).

Why: design §7. Each golden row is asserted on all 11 hash columns (components first, then master) plus column 12 against `is_game_over`; intent guards pin each sidecar's non-default values after parsing (so a regenerated sidecar that lost a value fails loudly); coverage witnesses are recomputed from the driven Rust state (never read from the golden); a second, `shadow = false` run proves `CorrectShadow` fired; a determinism backstop runs each variant twice.

- [ ] **Step 1: Write the test** — create `rust/oracle-tests/tests/sim_slice4_5a_settings_golden.rs`:

```rust
//! Step 4½a-1 T9 — MILESTONE (design §7): four SETTINGS-DRIVEN matches bit-exact vs C++.
//!
//! Each scenario names a C++-schema setup file (`settings <file>`). The C++ dumper read it
//! with the real `Settings::FromToml` and started the worms in the C++ LocalController
//! state; here the same file goes through `settings_toml::settings_from_toml` and
//! `scenario::build::build_match`. Every golden row is asserted on the 11 hash columns and
//! on column 12, `Game::IsGameOver()`, against `sim::game_over::is_game_over`.
//!
//! Variants (design §7.3): `defaults` (the shipped legacy `data/Setups/liero.cfg`, no
//! input), `killemall` (lives 1, health 150, loading 37, blood 250, !loadChange, bonuses
//! 6, shadow, pool 300), `scales` (mode 3, lives 2, health 120, …), `gametag` (mode 1,
//! timeToLose 12, blood 0, shadow off, …).

use std::path::Path;

use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::{
    MatchConfig, Settings, GM_GAME_OF_TAG, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE,
};
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// C++ `LocalController`: 180 simulated frames after the game-over frame.
const POST_MORTEM: usize = 180;

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{GOLDEN}/{rel}")).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

struct Row {
    tick: u32,
    hashes: [u32; 10], // master, rng, level, worm0, worm1, bob, bon, sob, nob, wob
    game_over: u32,
}

fn parse_golden(text: &str) -> Vec<Row> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(cols.len(), 12, "settings-path golden lines have 12 columns");
            let mut hashes = [0u32; 10];
            for (i, c) in cols[1..11].iter().enumerate() {
                hashes[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            Row {
                tick: cols[0].parse().expect("tick"),
                hashes,
                game_over: cols[11].parse().expect("game-over column"),
            }
        })
        .collect()
}

fn check(state: &SimState, row: &Row) {
    let c = hash_components(state);
    let got = [
        hash_game_state(state),
        c.rng,
        c.level,
        c.worms[0],
        c.worms[1],
        c.bobjects,
        c.bonuses,
        c.sobjects,
        c.nobjects,
        c.wobjects,
    ];
    let names = ["master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob"];
    for i in (1..10).chain(0..1) {
        assert_eq!(
            got[i], row.hashes[i],
            "tick {}: {}: got {:08x} want {:08x}",
            row.tick, names[i], got[i], row.hashes[i]
        );
    }
    assert_eq!(is_game_over(state) as u32, row.game_over, "tick {}: IsGameOver", row.tick);
}

struct Case {
    scenario: Scenario,
    cfg: MatchConfig,
}

fn case(name: &str) -> Case {
    let scenario = Scenario::parse(&read(&format!("sim_slice4_5a_{name}_scenario.txt"))).unwrap();
    let rel = scenario.settings.clone().expect("a settings-driven scenario");
    assert!(scenario.worms.is_empty(), "worms come from the setup");
    let settings = settings_from_toml(&read(&rel)).expect("setup parses");
    let cfg = MatchConfig { settings, seed: scenario.seed };
    Case { scenario, cfg }
}

/// What the coverage witnesses read, from the genuinely driven Rust state.
#[derive(Default)]
struct Witness {
    game_over_tick: Option<u32>,
    deaths: u32,
    respawns: u32,
    peak_bobjects: usize,
    reload_started: bool,
    bonus_dropped: bool,
    scales_death_kept_health: bool,
    life_gained: bool,
    timer_bumped: bool,
    level_series: Vec<u32>,
    masters: Vec<u32>,
}

fn drive(c: &Case, cfg: &MatchConfig, golden: Option<&[Row]>) -> Witness {
    let bytes = std::fs::read(format!("{TC_ROOT}/{}", c.scenario.level)).expect("read level");
    let level = assets::level::load(&bytes).expect("level loads");
    let mut st = build_match(Path::new(TC_ROOT), cfg, &level).expect("builds").state;
    let mut w = Witness::default();
    if let Some(g) = golden {
        assert_eq!(g.len() as u32, c.scenario.ticks + 1, "golden rows 0..=ticks");
        check(&st, &g[0]);
    }
    w.level_series.push(hash_components(&st).level);
    w.masters.push(hash_game_state(&st));
    for k in 1..=c.scenario.ticks {
        let prev: Vec<(bool, i32, i32)> = st.worms.iter().map(|x| (x.visible, x.lives, x.timer)).collect();
        st.process_frame(&[
            ControlState::unpack(c.scenario.input(k - 1, 0)),
            ControlState::unpack(c.scenario.input(k - 1, 1)),
        ]);
        if let Some(g) = golden {
            check(&st, &g[k as usize]);
        }
        w.level_series.push(hash_components(&st).level);
        w.masters.push(hash_game_state(&st));
        for (i, x) in st.worms.iter().enumerate() {
            let (was_visible, lives, timer) = prev[i];
            if was_visible && !x.visible {
                w.deaths += 1;
                w.scales_death_kept_health |= x.health > 0;
            }
            if !was_visible && x.visible {
                w.respawns += 1;
            }
            w.life_gained |= x.lives > lives;
            w.timer_bumped |= x.timer > timer;
            w.reload_started |= x.weapons.iter().any(|ww| ww.loading_left > 0);
        }
        w.peak_bobjects = w.peak_bobjects.max(st.bobjects.len());
        w.bonus_dropped |= !st.bonuses.is_empty();
        if w.game_over_tick.is_none() && is_game_over(&st) {
            w.game_over_tick = Some(k);
        }
    }
    w
}

/// `CorrectShadow` fired: the same match with `shadow = false` carves a different level.
fn shadow_fired(c: &Case, on: &Witness) -> bool {
    let mut off = c.cfg.clone();
    off.settings.shadow = false;
    drive(c, &off, None).level_series != on.level_series
}

fn assert_post_mortem(name: &str, golden: &[Row], w: &Witness) {
    let g = w.game_over_tick.unwrap_or_else(|| panic!("{name}: the match must end"));
    let first = golden.iter().position(|r| r.game_over == 1).expect("golden game over");
    assert_eq!(first as u32, g, "{name}: Rust and C++ agree on the game-over tick");
    assert!(golden.len() - first > POST_MORTEM, "{name}: >= 180 post-mortem ticks recorded");
    assert!(golden[first..].iter().all(|r| r.game_over == 1), "{name}: IsGameOver stays true");
}

#[test]
fn defaults_the_shipped_setup_matches_cpp() {
    let c = case("defaults");
    assert_eq!(c.cfg.settings, Settings::default(), "shipped liero.cfg == C++ Settings()");
    let golden = parse_golden(&read("sim_slice4_5a_defaults.txt"));
    let w = drive(&c, &c.cfg, Some(&golden));
    assert!(w.respawns >= 2, "both worms spawn in-sim from the LocalController start");
    assert_eq!(w.game_over_tick, None, "lives 15, no input: never over");
}

#[test]
fn killemall_matches_cpp() {
    let c = case("killemall");
    let s = &c.cfg.settings;
    assert_eq!((s.game_mode, s.lives, s.loading_time, s.blood), (GM_KILL_EM_ALL, 1, 37, 250));
    assert!(!s.load_change && s.shadow);
    assert_eq!((s.max_bonuses, s.blood_particle_max), (6, 300));
    assert_eq!((s.worm_settings[0].health, s.worm_settings[1].health), (150, 150));
    assert_ne!(s.worm_settings[0].weapons, s.worm_settings[1].weapons, "per-worm loadouts");
    assert_eq!(s.weap_table.iter().filter(|&&t| t == 2).count(), 5, "deferred weapons banned");
    assert_eq!(s.weap_table.iter().filter(|&&t| t == 1).count(), 3, "bonus-only weapons");
    let golden = parse_golden(&read("sim_slice4_5a_killemall.txt"));
    let w = drive(&c, &c.cfg, Some(&golden));
    assert_post_mortem("killemall", &golden, &w);
    assert_eq!(w.peak_bobjects, 300, "the blood pool reaches its settings cap");
    assert!(w.reload_started, "loading_time 37 is exercised");
    assert!(w.bonus_dropped, "max_bonuses 6 opens the drop roll");
    assert!(shadow_fired(&c, &w), "CorrectShadow changed material_id");
}

#[test]
fn scales_matches_cpp() {
    let c = case("scales");
    let s = &c.cfg.settings;
    assert_eq!((s.game_mode, s.lives, s.loading_time, s.blood), (GM_SCALES_OF_JUSTICE, 2, 150, 60));
    assert_eq!((s.max_bonuses, s.blood_particle_max), (8, 500));
    assert_eq!(s.worm_settings[0].health, 120);
    assert!(s.shadow);
    let golden = parse_golden(&read("sim_slice4_5a_scales.txt"));
    let w = drive(&c, &c.cfg, Some(&golden));
    assert_post_mortem("scales", &golden, &w);
    assert!(w.scales_death_kept_health, "the Scales death branch (worm.cpp:384-388)");
    assert!(w.life_gained, "the Scales overflow-to-lives rule");
    assert!(shadow_fired(&c, &w), "CorrectShadow changed material_id");
}

#[test]
fn gametag_matches_cpp() {
    let c = case("gametag");
    let s = &c.cfg.settings;
    assert_eq!((s.game_mode, s.time_to_lose, s.blood, s.loading_time), (GM_GAME_OF_TAG, 12, 0, 0));
    assert_eq!((s.lives, s.max_bonuses), (3, 3));
    assert_eq!(s.worm_settings[0].health, 80);
    assert!(!s.shadow);
    let golden = parse_golden(&read("sim_slice4_5a_gametag.txt"));
    let w = drive(&c, &c.cfg, Some(&golden));
    assert_post_mortem("gametag", &golden, &w);
    assert!(w.timer_bumped, "the it-timer runs to time_to_lose");
    assert!(w.deaths >= 1);
}

#[test]
fn every_variant_is_internally_deterministic() {
    for name in ["defaults", "killemall", "scales", "gametag"] {
        let c = case(name);
        assert_eq!(drive(&c, &c.cfg, None).masters, drive(&c, &c.cfg, None).masters, "{name}");
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test sim_slice4_5a_settings_golden`
Expected: PASS (5 tests). This task has no separate RED step: T7/T8 produced the oracle and T1–T6 the implementation; a failure here is a real divergence — the first failing tick and component localise it (a `worm*` column ⇒ T4's rules or T6's worm mapping; `level` ⇒ T3's `CorrectShadow`; `bon` ⇒ `weap_table`/bonus consts; column 12 ⇒ T5). Fix the implementation, never the golden.

- [ ] **Step 3: Re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/sim_slice4_5a_settings_golden.rs` — Expected: no output (else format and re-run).

- [ ] **Step 4: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/sim_slice4_5a_settings_golden.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5a-1): MILESTONE — settings-driven matches bit-exact vs C++ incl. IsGameOver" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

**MILESTONE.** A match configured from a C++ setup file — every sim-reaching field non-default in some variant — is bit-exact against C++ through game over and the post-mortem, and the shipped default setup matches too.

---

### Task 10: `MatchFlow` — the post-mortem, and the live match-end restart  [Sonnet]

**Files:**
- Create: `rust/game/src/match_flow.rs`
- Modify: `rust/game/src/lib.rs` (add `pub mod match_flow;` after `pub mod input;` and mention it in the module doc)
- Modify: `rust/game/src/main.rs` — imports (`:29-30`), `Demo` (`:75-98`), `setup`'s `Demo { … }` (`:436-446`), the F5 block (`:605-616`), the tick body after `audio.0.reap(&live);` (`:661`), a new `restart_match` fn after `render_and_upload` (`:749`)

**Interfaces:**
- Consumes: T5 `sim::game_over::is_game_over`.
- Produces (4½c/d/g build on it): `pub enum MatchPhase { Game, GameEnded }`, `pub enum FlowStep { Continue, Finished }` (both `Clone, Copy, Debug, PartialEq, Eq`), `pub const POST_MORTEM_FRAMES: i32 = 180`, `pub const FADE_IN_MAX: i32 = 33`, `pub struct MatchFlow` with `pub fn new() -> Self`, `pub fn after_frame(&mut self, state: &SimState) -> FlowStep`, `pub fn phase(&self) -> MatchPhase`, `pub fn fade_value(&self) -> i32`; `impl Default for MatchFlow`.

Why: design §6 / §1.3 finding 7. `LocalController::Process` keeps simulating in `GameEnded` and returns `false` on the call that finds `fade_value == 0` after the 180 set on game over (`localController.cpp:153-199`, `:277-282`): exactly 180 more frames. Live mode restarts the match there (the stand-in for 4½g's stats → menu route); Scripted and Replay are untouched.

- [ ] **Step 1: Write the failing tests** — create `match_flow.rs` with only this module and add the `lib.rs` line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use scenario::Scenario;

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
    const SAMPLE: &str = "seed 42\nlevel Levels/render_stage.lev\nticks 1\n\
                          worm 0 6553600 7602176 100 10 0   1\n\
                          worm 1 3276800 7602176 100 10 218 1\nweapon 0 DART\n";

    fn state() -> SimState {
        scenario::load(Path::new(TC_ROOT), &Scenario::parse(SAMPLE).unwrap()).state
    }

    #[test]
    fn a_running_match_continues_at_the_post_weapon_selection_fade() {
        let s = state();
        let mut f = MatchFlow::new();
        assert_eq!(f.fade_value(), FADE_IN_MAX, "kStateWeaponSelection -> kStateGame: 33");
        for _ in 0..50 {
            assert_eq!(f.after_frame(&s), FlowStep::Continue);
        }
        assert_eq!(f.phase(), MatchPhase::Game);
        assert_eq!(f.fade_value(), 33);
    }

    #[test]
    fn game_over_runs_exactly_180_more_frames_then_finishes() {
        let mut s = state();
        let mut f = MatchFlow::new();
        assert_eq!(f.after_frame(&s), FlowStep::Continue);
        s.worms[1].lives = 0;
        // The detection call (the game-over frame): fade 180, then decremented once.
        assert_eq!(f.after_frame(&s), FlowStep::Continue);
        assert_eq!(f.phase(), MatchPhase::GameEnded);
        assert_eq!(f.fade_value(), 179);
        for n in 1..180 {
            assert_eq!(f.after_frame(&s), FlowStep::Continue, "post-mortem call {n}");
        }
        assert_eq!(f.fade_value(), 0);
        assert_eq!(f.after_frame(&s), FlowStep::Finished, "the 180th frame after game over");
    }

    #[test]
    fn a_revived_worm_does_not_cancel_the_post_mortem() {
        let mut s = state();
        let mut f = MatchFlow::new();
        s.worms[0].lives = 0;
        assert_eq!(f.after_frame(&s), FlowStep::Continue);
        s.worms[0].lives = 3; // C++ never leaves kStateGameEnded
        let mut steps = 1;
        while f.after_frame(&s) == FlowStep::Continue {
            steps += 1;
            assert!(steps <= 180, "must finish on schedule");
        }
        assert_eq!(steps, 180);
        assert_eq!(f.phase(), MatchPhase::GameEnded);
    }

    #[test]
    fn game_of_tag_ends_on_the_timer() {
        let mut s = state();
        s.game_mode = 1;
        s.time_to_lose = 5;
        let mut f = MatchFlow::new();
        s.worms[0].timer = 4;
        f.after_frame(&s);
        assert_eq!(f.phase(), MatchPhase::Game);
        s.worms[0].timer = 5;
        f.after_frame(&s);
        assert_eq!(f.phase(), MatchPhase::GameEnded);
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --lib match_flow`
Expected: FAIL to compile — `cannot find type MatchFlow`.

- [ ] **Step 3: Implement** — above the tests:

```rust
//! The match lifecycle after weapon selection (Step 4½a-1, design §6): the tail of C++
//! `LocalController::Process` (`localController.cpp:153-199`) and `ChangeState`
//! (`:214-290`), Bevy-free so it is headlessly testable (the `lib.rs` rule).
//!
//! Once `Game::IsGameOver` holds (`:177-179`) the state becomes `GameEnded` with
//! `fade_value = 180` (`:277-282`); the sim KEEPS TICKING (the frame loop runs for
//! `kStateGame || kStateGameEnded`, `:153-155`); each call decrements the fade and the
//! call that finds it at 0 ends the match (`:185-194`) — 180 more simulated frames.
//! `fade_value` is also the C++ renderer fade (`:211`); 4½d draws it. Seams: 4½c adds a
//! weapon-selection phase in front, 4½d the Esc fade (`OnKey`, `:82-85`), 4½g routes
//! `Finished` to the stats screen.

use sim::state::SimState;

/// `ChangeState(kStateGameEnded)`'s fade (`localController.cpp:279`).
pub const POST_MORTEM_FRAMES: i32 = 180;
/// The fade-in ceiling (`localController.cpp:196`) and the value entering the game from
/// weapon selection (`:285`).
pub const FADE_IN_MAX: i32 = 33;

/// `kStateGame` / `kStateGameEnded`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchPhase {
    Game,
    GameEnded,
}

/// `LocalController::Process`'s return value: `Continue` = true, `Finished` = false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowStep {
    Continue,
    Finished,
}

#[derive(Clone, Debug)]
pub struct MatchFlow {
    phase: MatchPhase,
    fade_value: i32,
    going_to_menu: bool,
}

impl Default for MatchFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchFlow {
    /// Entering `kStateGame` from weapon selection: `fade_value = 33` (`:284-287`).
    pub fn new() -> Self {
        MatchFlow {
            phase: MatchPhase::Game,
            fade_value: FADE_IN_MAX,
            going_to_menu: false,
        }
    }

    pub fn phase(&self) -> MatchPhase {
        self.phase
    }

    pub fn fade_value(&self) -> i32 {
        self.fade_value
    }

    /// Call once per tick AFTER `process_frame` (the C++ order: `ProcessFrame`, then
    /// `IsGameOver`, then the fade bookkeeping).
    pub fn after_frame(&mut self, state: &SimState) -> FlowStep {
        if self.phase == MatchPhase::Game && sim::game_over::is_game_over(state) {
            self.phase = MatchPhase::GameEnded;
            if !self.going_to_menu {
                self.fade_value = POST_MORTEM_FRAMES;
                self.going_to_menu = true;
            }
        }
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
}
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --lib match_flow` — Expected: PASS (4 tests).
Run: `rustfmt --edition 2024 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/game/src/match_flow.rs` — Expected: no output.

- [ ] **Step 5: Wire it into the live binary** (`main.rs`)

Imports: after `use game::input::{InputSource, Mode, ParsedArgs, Recorder};` add `use game::match_flow::{FlowStep, MatchFlow};`.

`Demo`: after `tick: u32,` add

```rust
    /// Step 4½a-1: the match lifecycle (`LocalController`'s game → game-ended tail) —
    /// `Some` in `Mode::Live` only. Scripted loops its golden and Replay plays a fixed
    /// `ticks`, so neither ends a match.
    flow: Option<MatchFlow>,
```

and in `setup`'s `Demo { … }` literal add `flow: (*mode == Mode::Live).then(MatchFlow::new),` after `tick: 0,`.

Replace the body of the F5 `if` block (`if let Some(recorder) … return;`) with:

```rust
        restart_match(&mut sim.0, &mut demo, recorder.as_deref_mut());
        render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);
        return;
```

(keep the long comment above it). Inside `if !replay_finished { … }`, after `audio.0.reap(&live);` add:

```rust
        // 4½a-1: match end (LocalController::Process tail, localController.cpp:177-199) —
        // Live only. IsGameOver => 180 more simulated frames => restart through the F5
        // path (the stand-in for 4½g's stats -> menu route, design §6).
        let finished = demo
            .flow
            .as_mut()
            .is_some_and(|flow| flow.after_frame(&sim.0) == FlowStep::Finished);
        if finished {
            restart_match(&mut sim.0, &mut demo, recorder.as_deref_mut());
            render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);
            return;
        }
```

After `render_and_upload` add:

```rust
/// Rebuild tick 0 through `scenario::load` — the 4f F5 restart, shared since 4½a-1 with
/// the live match-end restart. Restarts an in-flight recording too (the 4f T0 fix: the
/// flushed file then covers only ticks since the restart) and, in Live, the `MatchFlow`.
fn restart_match(sim: &mut SimState, demo: &mut Demo, recorder: Option<&mut Recorder>) {
    if let Some(recorder) = recorder {
        recorder.clear();
    }
    let loaded = scenario::load(Path::new(TC_ROOT), &demo.scenario);
    *sim = loaded.state;
    demo.viewports = loaded.viewports;
    demo.scene = loaded.scene;
    demo.tick = 0;
    if demo.flow.is_some() {
        demo.flow = Some(MatchFlow::new());
    }
}
```

- [ ] **Step 6: Build and run the game gates**

Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: builds, no warnings.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS (passthrough, round-trip, record-regression, viewport stepping unchanged; the 4 `match_flow` tests).
Manual (advisory, not gated): `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game`, kill one worm 10 times (lives 10 in the default match); ~2.6 s (180 ticks) after the tenth death the match restarts at tick 0. Note the observation in the done-report.

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/game/src/match_flow.rs rust/game/src/lib.rs rust/game/src/main.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "game(4.5a-1): MatchFlow post-mortem (IsGameOver + 180 frames); live match-end restart" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 11: Full re-diff, wasm build, PROGRESS, overview status  [Opus review]

**Files:**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md` (header "Last updated" + the Step 4½ tree, `:696-738`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md` (status line `:3`, the 4½a bullet `:215-225`)

- [ ] **Step 1: The full green board**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --target wasm32-unknown-unknown` — Expected: builds (the `toml` dep in `scenario` is pure Rust; `MatchFlow` is target-agnostic).
Run: `cargo tree --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim-core --depth 1` — Expected: `sim-core` with no dependencies.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --stat master -- rust/oracle-tests/golden` — Expected: only added `sim_slice4_5a_*` files — 11 of them: 4 scenarios, 3 setup sidecars, 4 goldens — and no modified file; every pre-existing golden unchanged.

- [ ] **Step 2: Update PROGRESS** — set "Last updated" to the real current date and a one-paragraph 4½a-1 summary (settings model + C++ reader; builder reproducing `sim_slice6_fuzz5`; `CorrectShadow` + the Scales/GameOfTag rules + `IsGameOver`; the `settings` directive; four settings-driven goldens bit-exact incl. the IsGameOver column; `MatchFlow`; the `sound_hooks` fix; the seeds chosen in T8). In the Step 4½ tree change the 4½a line to:

```
├─ 🔶 4½a  split (design §0). 4½a-1 ✅ LANDED: Settings/WormSettings/MatchConfig + C++ TOML reader
│          + build_match (reproduces sim_slice6_fuzz5) + CorrectShadow + Scales/GameOfTag rules +
│          IsGameOver + MatchFlow (180-frame post-mortem) + sound_hooks fix. Gate: `settings <file>`
│          dumper directive, 4 settings-driven goldens bit-exact incl. IsGameOver.
│          4½a-2 ⬜: TOML writer + byte gate (vs C++ load+save) + UpdateHash + storage + HUD fix
```

and under "Open for John" add: the laser/steerable deferred weapons before 4½c (design §11 Q1) and the restated TOML byte gate (design §11 Q3).

- [ ] **Step 3: Update the overview** — status line: "slices 4½a–4½h ALL PLANNED, NONE STARTED" → "4½a-1 LANDED (4½a split into a-1/a-2), 4½b–4½h planned". Append to the 4½a bullet: "**Split** (slice design §0): 4½a-1 = model + reader + builder + sim completion (`CorrectShadow`, Scales/GameOfTag rules) + `IsGameOver`/post-mortem + goldens via one `settings <file>` directive; 4½a-2 = TOML writer, byte gate, `UpdateHash`, storage, HUD fix. The byte gate is restated against C++ load+save because the shipped files are legacy formats (slice design §3.4)."

- [ ] **Step 4: Broad review (Opus)** — re-read the whole 4½a-1 diff against the design: every §1.1 sim-reaching field is plumbed and varied (§7.3 table); `SimState::new` unchanged; the scenario grammar gained only `settings`; `scenario::load` refuses it; the dumper's classic path is byte-identical; Holdazone refused everywhere; `correct_shadow`'s signature matches T3's Interfaces (4½b T9); no BANNED weapon in any loadout; no push, no PR. Bar: 0 Critical / 0 Important.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "docs(4.5a-1): PROGRESS + overview — slice 4.5a-1 landed, 4.5a split" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

## Done-report (each task)

(a) what changed and why, (b) files touched, (c) tests run + risks. Per-task commit on `liero-rs-step-4-5`; no push, no PR. The final report surfaces: the fuzz5 reproduction (T6), the absent-path regen evidence (T7), the chosen seeds + ledgers (T8), the milestone result (T9), the manual match-end observation (T10), and confirmation that no pre-existing golden changed.

