# Step 4½, Slice 4½c — the weapon selection phase: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** The C++ weapon-selection phase (`weapsel.cpp`) runs in front of every live Rust match with its RNG stream bit-exact against a new C++ dumper. The live game draws it pixel-faithfully, using a menu-drawing subset pulled forward from 4½d, and hands the picks into the match. It works on the keyboard and on the browser's touch pad. `?weapons=` and `--live --record` skip it. On a touch-only page, player 2 is a bot that readies at once.

**Architecture:** A Bevy-free `sim::weapsel` holds the phase: the constructor with its rejection loops, `process_frame` with `LocalController`'s 12/3 key repeat on sampled words, RANDOMIZE, the menu sounds and `finalize`. It mutates `SimState` (the RNG, the control words, the weapons) and is driven only from `tick_and_render` (LD 3). `scenario::build` splits into `new_match` / `enter_game` / `weapsel_config`, and `build_match` is rebuilt on them with unchanged output. The scenario grammar gains one oracle-only directive, `weapsel <frame> <w0> <w1>`, which needs `settings`. On the C++ side, a shared driver header (`weapsel_drive.hpp`) runs the REAL `WeaponSelection` behind a verbatim replica of `LocalController`'s input plumbing. A new `oracle_dump_weapsel` writes one golden line per step and self-checks the replica against a real `LocalController`. `sim_physics_dump`'s `settings` path learns `weapsel`, which gives two 12-column continuation goldens. `render` gains `draw_rounded_box`, `Font::get_dims`, `menu::draw_item` and `render::weapsel` (palette, frozen frame, screen). `game` gains a `MatchPhase::WeaponSelection`, a `ReleaseLatch`, and a Bevy-free `game::selection` that owns the running phase and the in-memory picks.

**Tech Stack:** Rust 2021 (`sim-core`, `sim`, `scenario`, `render`, `shot`, `oracle-tests`) and 2024 (`game`); C++ (`src/tools/oracle_dump/`, preset `linux-x64` here, clang-format 22.1.0 via `uvx`); HTML/JS (`web/index.html`); GitHub Actions YAML (`.github/workflows/preview.yml`).

**Spec:** `docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md` (cited **design §N**; findings cited **finding N**). **Rulings (John, 2026-09-25):** Q1: draw the weapon menu **pixel-exact in 4½c** (pull the rounded box, text-width measuring, the menu-item recipe and the frozen-screen look forward). Q3: `?weapons=` **skips** selection. Q8: on touch-only devices player 2 is an **auto-ready bot**. Q2, Q4–Q7 and Q9 follow the design's recommendations: port `LocalController`'s repeat; skip selection under `--record`; latch at both phase boundaries; self-check the dumper; start the live phase from the launched loadout; write continuation goldens as well as the handoff test. Overview: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md`.

## Plan-time facts checked against the source (the source wins)

The tree moved after the design's baseline. Every task below was written against the current source (HEAD `6abda0d`). Where the design and the source disagree, the plan follows the source:

1. **The worm-colour palette step is unported, and the design understates it.** Design §3.7 says there is "no worm-colour step". That is true of `UpdateWeapselPalette` (`weapsel.cpp:20-24`), but `renderer.Origpal()` already carries the worm colours. `LocalController::Focus` → `Game::Focus` → `Game::UpdateSettings` (`game.cpp:475-487`) runs `Palette::SetWormColour` (`palette.cpp:92-104`, `palette.hpp:69-86`), and Rust ports none of it. For the default RGB, the name colours differ slightly: entry 33 is Rust (144,144,244) vs C++ (136,136,248), and entry 42 is (112,188,112) vs (104,188,104), computed from `sprites/small.tga` and `ScaleAdd` at plan time. The C++ build also draws *random* player names (`random_name = true`), while Rust draws `""`. Both are recorded as pixel caveats (T8, T11), next to the design's own caveats (no render fade; the scenario-path HUD). **Not fixed in 4½c.** SetWormColour belongs with the player colour work (4½f).
2. **The self-goldens cannot be a `render` test.** `scenario` depends on `render`, so a `render` test cannot call `scenario::build::new_match` (design §6.8 says "A `render` test"). They live in `rust/oracle-tests/tests/render_weapsel_selfgolden.rs`.
3. **`render::frame::draw` processes the viewports; C++ `Game::Draw` does not.** The frozen frame is therefore drawn with fresh `Viewport::player_layout()`s. In the weapsel state, worms have `killed_timer = 150` and are not processed, so `process` leaves the camera at (0,0), which is what C++ shows. The live `demo.viewports` are never touched by it.
4. **The scenario-path frozen frame also shows *visible* worms.** `game/scenarios/default_match.txt` spawns visible worms, while C++ has them invisible. Design §5 names only the lives. The `new_match` path (`shot --weapsel`, the self-goldens) is the faithful one.
5. **Golden format refinements.** The `final` line gains the two control words after `Finalize`: `final <l0> <l1> <ctl0> <ctl1> <last> <next>`. This pins `ReleaseControls`. C++ counts draws by `std::mt19937` state equality (`Rand::engine`, `rand.hpp:15`), which is stronger than "last and next match". `recordReplays = false` is forced in the self-check's code, not in the sidecars (the lean sidecar format of 4½c-0 stays unchanged). `WeapselError` gains `WormCount` (the phase indexes worms 0 and 1).
6. **Task split.** The design's T4 changes both parsers. Here T4 is the Rust parser and T5 holds all C++ (both C++ parsers, the driver, the dumper), so each task has one toolchain.
7. **`scenario::load` leaves `SimState::weap_table` empty.** This does not matter, because the phase reads `WeapselConfig`, not `SimState`.
8. **The C++ Dig block in `OnKey` is a no-op on sampled words** (`localController.cpp:69-79`). There is no bit 7, and a Left/Right control bit is only ever set while its clean bit is set. It is replicated in C++ anyway, and omitted in Rust (commented). The design's "DIG differs in sounds only" also applies to every repeat while DIG is held: each repeat plays two net-zero cycles.
9. **Citations.** The strings are at `tc.cfg:245-250` (not `:246-248`). `CenterX` is in `src/game/math/rect.hpp:72-74`.
10. **No `origin/master` in this clone.** `scripts/clang-tidy-diff.sh` defaults to `origin/master`, so every tidy run passes the slice base: `scripts/clang-tidy-diff.sh build/linux-x64 6abda0d`.
11. **Linking.** `weapsel.cpp` references the `gfx` global, so `oracle_dump_sim_physics` now links `gfx.o` (it did not before). `Gfx::Gfx` is light (`gfx.cpp:264-275`), and `test_rollback_weapsel` already links the same objects.

## Global Constraints

- Repo `/home/user/openliero`, branch `claude/cpp-oracle-vcpkg-assets-chcwcm` (PR `hamiltoon/openliero#14` into `liero-rs-step-4-5`). Slice base `6abda0d`. Use absolute paths. There are no Bash-hygiene restrictions (`&&`, redirection and heredocs are fine). No sub-subagents.
- **Cargo:** run from `/home/user/openliero/rust`, in **DEBUG only** (`cd /home/user/openliero/rust && cargo test …`; never `--release`, because some tests are `#[should_panic]` on `debug_assert!`s). **The re-diff** is `cargo test --workspace --exclude game` plus `cargo test -p game`. The wasm gate is `cargo build -p game --target wasm32-unknown-unknown`. The one non-test build is T10's preview bundle, which uses the preview's own `--profile wasm-release`, as CI deploys it.
- **C++ build:** before ANY cmake or gen-script call, `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh`. It sets `PRESET=linux-x64`, the vcpkg asset script and the sdl3 overlay. `build/linux-x64` is already configured with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON` and exports `compile_commands.json`; the binaries are in `build/linux-x64/Release/`. Gen scripts default to `macos-arm64` and honour `PRESET`, as the existing ones do. Run each gen script in the same shell as the `source`: `source …/env.sh && bash /home/user/openliero/rust/oracle-tests/<script>.sh`.
- **clang-format** (every C++ file touched): `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file <abs file>`, run on the whole file (CLAUDE.md: a diff check misses context). **clang-tidy** on changed lines: `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 6abda0d`. It must be clean (CLAUDE.md). Fix the code; add a `NOLINTNEXTLINE(<check>) — <reason>` only where the repo already does so for the same check.
- **Determinism:** no floats, no wall clock, no `HashMap` iteration in `sim`. C++ `uint32_t` wrap is Rust `wrapping_*`/`u32`. C++ `rand(a, b)` is `Rand::bound_range(a, b)` (`sim-core/src/rng.rs:122`, `rand.hpp:30-38`).
- **LD 3:** `tick_and_render` stays the only `SimState` mutator in `game`. **LD 5:** `SimState::new`'s signature does NOT change. `sim-core` stays dependency-free. `sim` gains no dependency. Bevy stays in `game`. `render`, `scenario` and `shot` stay Bevy-free.
- **Grammar:** exactly one new directive, `weapsel <frame> <worm0_7bit> <worm1_7bit>`. It is oracle-only, requires `settings`, and both parsers change it together (T4 Rust, T5 C++). No other grammar change.
- **Golden audit:** `git -C /home/user/openliero diff --name-status 6abda0d -- rust/oracle-tests/golden` may list ONLY `A` lines, and only for the 4½c files: `weapsel_<case>{_scenario.txt,_setup.cfg,.txt}` × 16 and `sim_slice4_5c_match_{humans,bot}.txt`. Any regenerated existing golden must stay byte-identical, proven the way 4½c-0's T8 did it: regenerate with the modified dumper, then `git status --porcelain -- rust/oracle-tests/golden` must be empty. Any diff stops the line: restore with `git -C /home/user/openliero checkout -- rust/oracle-tests/golden`, report the file and the first differing line, and do not commit.
- **C++ changes** are confined to `src/tools/oracle_dump/weapsel_drive.hpp` (new), `src/tools/oracle_dump/weapsel_dump.cpp` (new), `src/tools/oracle_dump/sim_physics_dump.cpp`, and two lines inside the `OPENLIERO_BUILD_ORACLE_DUMP` block of `CMakeLists.txt`.
- **Frozen provenance:** never edit the 4½a-1 / 4½c-0 generators, common modules or goldens (`examples/gen_slice4_5a.rs`, `examples/gen_slice4_5c0.rs`, `tests/sim_slice4_5c0_common/`, `golden/sim_slice4_5a_*`, `golden/sim_slice4_5c0_*`).
- **rustfmt** only files this plan CREATES (`rustfmt --edition 2021 <abs file>`, `--edition 2024` for `game`). Never run rustfmt on an existing file or on a `lib.rs`/`main.rs` (it recurses). Hand-format edits to the surrounding style.
- **The browser pieces keep working:** `game::web_params`, `game::touch`, `game::hud_mode`, `.github/workflows/preview.yml` and `web/index.html`. The preview comment and the page's help text describe selection and its controls (T9).
- **Commits:** use the globally configured identity; do not override it. Every commit message carries two trailer paragraphs as extra `-m` arguments: `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, naming the model that did the work (`Claude Opus 5.5` or `Claude Sonnet 5`; the commit lines below show each task's tier model), and `Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT`. Never write "Generated with Claude Code" anywhere.
- **Do NOT push and do NOT open a PR.** The controller pushes. Task executors never push.

## Model tiers

- **[Opus]:** T1 (the constructor: the RNG stream), T2 (`process_frame`, key repeat, RANDOMIZE), T3 (the builder split), T5 (C++ driver + dumper + self-check + regeneration proof), T6 (corpus + witnesses + goldens), T7 (MILESTONE), T9 (live wiring), T11 (broad review). Every reviewer is Opus.
- **[Sonnet]:** T0 (`Rand: Clone`, the shared `weap_order`), T4 (the Rust parser directive), T8 (the render subset, fully specified), T10 (the headless-Chromium check).

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `rust/sim-core/src/rng.rs` (modify) | T0 | `#[derive(Clone)]` on `Rand` + a clone test |
| `rust/sim/src/weapsel.rs` (create) | T0–T3 | `weap_order`; `WeapselConfig`/`WeapselPlayer`/`PlayerSel`/`KeyRepeat`/`WeapselError`; `WeaponSelection::{new, process_frame, finalize}`; `init_weapons` |
| `rust/sim/src/lib.rs` (modify) | T0 | `pub mod weapsel;` |
| `rust/scenario/src/loader.rs` (modify) | T0, T8 | shared `weap_order`; `SceneData.weapsel_texts` |
| `rust/scenario/src/build.rs` (modify) | T0, T3 | shared `weap_order`; `validate_for_selection`, `new_match`, `enter_game`, `weapsel_config`; `build_match` rebuilt on them |
| `rust/scenario/src/parser.rs` (modify) | T4 | `weapsel` directive, `weapsel_end`, `weapsel_input`, `to_text` |
| `src/tools/oracle_dump/weapsel_drive.hpp` (create) | T5 | `RecordingSoundPlayer`, `CountDraws`, `PeekNext`, `Driver` (OnKey + repeat replica + real `WeaponSelection`), `Run` |
| `src/tools/oracle_dump/weapsel_dump.cpp` (create) | T5 | `oracle_dump_weapsel`: the golden lines + the `LocalController` self-check |
| `src/tools/oracle_dump/sim_physics_dump.cpp` (modify) | T5 | parser `weapsel`; the `settings` path runs the phase |
| `CMakeLists.txt` (modify, oracle block `:372-396`) | T5 | `oracle_dump_weapsel` |
| `rust/oracle-tests/tests/weapsel_common/mod.rs` (create) | T6, T7 | corpus, sidecar + scenario writers, `Script`, `drive`, golden-line formatting, witnesses, 12-column harness |
| `rust/oracle-tests/examples/gen_slice4_5c.rs` (create) | T6 | `check` / `write` |
| `rust/oracle-tests/gen_weapsel_golden.sh`, `gen_sim_slice4_5c_golden.sh` (create) | T6 | C++ golden generation + awk gates |
| `rust/oracle-tests/golden/weapsel_*` ×48, `sim_slice4_5c_match_{humans,bot}.txt` (create) | T6 | the corpus and its goldens |
| `rust/oracle-tests/tests/weapsel_golden.rs` (create) | T7 | MILESTONE: 16 goldens line for line + witness guard |
| `rust/oracle-tests/tests/sim_slice4_5c_continuation_golden.rs` (create) | T7 | the two 12-column continuation goldens |
| `rust/oracle-tests/tests/weapsel_handoff.rs` (create) | T7 | handoff equality (§6.6) incl. a randomized property run |
| `rust/render/src/blit.rs`, `font.rs` (modify) | T8 | `draw_rounded_box`; `Font::get_dims` |
| `rust/render/src/menu.rs`, `weapsel.rs` (create); `lib.rs` (modify) | T8 | `draw_item`; `WeapselTexts`, `level_label`, `weapsel_palette`, `menu_origin`, `build_frozen`, `draw_screen` |
| `rust/oracle-tests/tests/render_weapsel_selfgolden.rs` (create) | T8 | three self-goldens + layout checks |
| `rust/shot/src/lib.rs` (modify) | T8 | `--weapsel` |
| `rust/game/src/match_flow.rs`, `input.rs`, `web_params.rs`, `lib.rs` (modify); `selection.rs` (create) | T9 | `MatchPhase::WeaponSelection`; `ReleaseLatch`; `skips_weapon_selection`; `Selection`, `live_config`, `loadout_picks` |
| `rust/game/src/main.rs` (modify) | T9 | the phase in `tick_and_render`, restart + write-back, the skips, the touch rule, `window.lieroPhase` |
| `web/index.html`, `.github/workflows/preview.yml` (modify) | T9 | `window.lieroTouchOnly`; the selection help text |
| `.claude/skills/liero-shot/SKILL.md`, PROGRESS, overview, design, cpp-map, rust-map (modify) | T11 | status + corrections |

## Task dependency map

```
T0 ─> T1 ─> T2 ─> T3 ─┬───────────────> T6 ─> T7 ─────────────┐
T4 ─> T5 ─────────────┘                                        │
                      └─> T8 ─> T9 ─> T10 (nice-to-have) ──────┴─> T11
```

T6 needs T3 (`new_match`), T4 (the grammar) and T5 (both dumpers). T8 needs T3 (`new_match` in the self-goldens and `shot`). T9 needs T3 and T8. T4 and T5 can run in parallel with T0–T3.

---

### Task 0: `Rand: Clone`, the shared `weap_order`, the unique-names pin  [Sonnet]

**Files:**
- Modify: `rust/sim-core/src/rng.rs` (`:28` the struct; a test in `mod tests`)
- Create: `rust/sim/src/weapsel.rs`
- Modify: `rust/sim/src/lib.rs` (`pub mod weapsel;` after `pub mod weapon;`)
- Modify: `rust/scenario/src/loader.rs` (`:123-125`), `rust/scenario/src/build.rs` (`:130-132`)

**Interfaces:**
- Produces (used by T1–T3, T6–T9): `impl Clone for sim_core::rng::Rand`; `pub fn sim::weapsel::weap_order(weapons: &[assets::object::Weapon]) -> Vec<usize>`.
- Consumes: `assets::object::Weapon::name`; `assets::tc::TcConfig::load`; `assets::object::Objects::load`.

Why: design finding 12. A rand probe (`next`) and Step 5 need `Rand: Clone`. `weap_order` is computed in two places; C++ sorts it unstably (`common.cpp:498-499`) and Rust stably, and the two agree only while weapon names are unique. The phase gets the one shared copy, and a TC test fails loudly if a future TC adds a duplicate name. This task is hash-neutral.

- [ ] **Step 1: Write the failing tests**

(a) Append inside `mod tests` in `rust/sim-core/src/rng.rs`:

```rust
    #[test]
    fn a_clone_continues_the_same_stream_independently() {
        // Step 4½c (design finding 12): the golden harness peeks `next` on a clone, and Step 5
        // snapshots the RNG next to the weapon-selection state.
        let mut a = Rand::new();
        a.seed(7);
        a.next_u32();
        let mut b = a.clone();
        assert_eq!((b.last(), b.draws()), (a.last(), a.draws()));
        assert_eq!(a.next_u32(), b.next_u32(), "same position, same value");
        b.next_u32();
        assert_eq!(b.draws(), a.draws() + 1, "independent copies");
    }
```

(b) Create `rust/sim/src/weapsel.rs`:

```rust
//! Step 4½c — the weapon selection phase (`weapsel.cpp`), Bevy-free (overview LD 7; design
//! `specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md`, cited **design §N**).
//!
//! Every C++ match starts with a pick screen whose constructor and RANDOMIZE item draw from
//! `game.rand`, the SIMULATION RNG (`weapsel.cpp:61`, `:68`, `:323`), so the number and order of
//! draws before match frame 0 decides every later tick. The phase therefore lives next to
//! `SimState`: it draws `state.rand` and writes the worms' control words and weapons (both hashed,
//! `hash.rs:49`, `:65-73`), and Step 5 snapshots it as `SimState` plus a `WeaponSelection` clone
//! (design §8). It is driven only from `tick_and_render` (LD 3); `game` owns the presentation.

use assets::object::Weapon;

/// `Common::weap_order` (`common.cpp:491-499`): weapon indices sorted by name. A saved pick `p`
/// (1-based, `WormSettings::weapons`) names weapon `weap_order[p - 1]`. The one shared copy
/// (design §4.7). C++ sorts unstably and Rust stably; the two agree while the TC's weapon names
/// are unique, which `the_tc_weapon_names_are_unique_so_both_sorts_agree` pins (finding 12).
pub fn weap_order(weapons: &[Weapon]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..weapons.len()).collect();
    order.sort_by(|&a, &b| weapons[a].name.cmp(&weapons[b].name));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc_weapons() -> Vec<Weapon> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc = assets::tc::TcConfig::load(&std::fs::read(format!("{root}/tc.cfg")).unwrap())
            .unwrap();
        assets::object::Objects::load(&tc.types, |sub, id| {
            std::fs::read(format!("{root}/{sub}/{id}.cfg"))
        })
        .unwrap()
        .weapons
    }

    #[test]
    fn the_tc_weapon_names_are_unique_so_both_sorts_agree() {
        let weapons = tc_weapons();
        let mut names: Vec<&str> = weapons.iter().map(|w| w.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            weapons.len(),
            "finding 12: C++ sorts weap_order unstably (common.cpp:498-499), Rust stably — \
             they agree only while weapon names are unique"
        );
    }

    #[test]
    fn weap_order_is_the_byte_order_of_the_names() {
        let weapons = tc_weapons();
        let order = weap_order(&weapons);
        assert_eq!(order.len(), 40);
        let name = |pick: usize| weapons[order[pick - 1]].name.as_str();
        // The picks the 4½c corpus and the live default loadout rely on (T6, T9).
        assert_eq!(
            (name(1), name(12), name(26), name(31), name(40)),
            ("BAZOOKA", "DART", "LASER", "MISSILE", "ZIMM")
        );
        // std::string's `<` is a byte compare: the space (0x20) sorts before letters.
        assert_eq!(
            (name(28), name(29), name(30)),
            ("MINI NUKE", "MINI ROCKETS", "MINIGUN")
        );
    }
}
```

In `rust/sim/src/lib.rs` add `pub mod weapsel;` after `pub mod weapon;`.

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p sim-core a_clone_continues`
Expected: FAIL to compile, `no method named 'clone' found for struct 'Rand'`.

- [ ] **Step 3: Implement**

(1) `rust/sim-core/src/rng.rs`: change `pub struct Rand {` (`:28`) to

```rust
#[derive(Clone)]
pub struct Rand {
```

and extend its doc comment's first paragraph with: `Clone (Step 4½c): a probe copy continues the same stream; Step 5 snapshots it.`

(2) `rust/scenario/src/loader.rs`: replace

```rust
    // weap_order: indices sorted by weapon name; id == index (Common::Precompute).
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
```

with

```rust
    // weap_order: indices sorted by weapon name; id == index (Common::Precompute). The one shared
    // copy since Step 4½c (design finding 12).
    let weap_order = sim::weapsel::weap_order(&objects.weapons);
```

(3) `rust/scenario/src/build.rs`: replace

```rust
    // weap_order: indices sorted by weapon name (Common::Precompute, common.cpp:491-499).
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
```

with

```rust
    // weap_order: indices sorted by weapon name (Common::Precompute, common.cpp:491-499) — the
    // one shared copy since Step 4½c (design finding 12).
    let weap_order = sim::weapsel::weap_order(&objects.weapons);
```

Leave the independent copy in `build.rs`'s test module (`:350-351`) alone: the test re-derives on purpose.

- [ ] **Step 4: GREEN + the re-diff**

Run: `cd /home/user/openliero/rust && rustfmt --edition 2021 /home/user/openliero/rust/sim/src/weapsel.rs`
Run: `cd /home/user/openliero/rust && cargo test -p sim-core a_clone_continues` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo test -p sim --lib weapsel` — Expected: PASS, 2 tests.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS (hash-neutral).
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/sim-core/src/rng.rs rust/sim/src/weapsel.rs rust/sim/src/lib.rs rust/scenario/src/loader.rs rust/scenario/src/build.rs
git -C /home/user/openliero commit -m "sim(4.5c): Rand: Clone + the one shared weap_order (sim::weapsel), unique-names pin" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 1: `sim::weapsel` — the types, the constructor, the refusals  [Opus]

**Files:**
- Modify: `rust/sim/src/weapsel.rs` (imports, constants, types, `impl WeaponSelection { new, accessors }`, the error type; new tests)

**Interfaces:**
- Produces (used by T2, T3, T6–T9):
  - `pub const WEAPON_COUNT: usize = 40`, `MENU_ITEMS: u8 = 7`, `RANDOMIZE_ITEM: u8 = 0`, `DONE_ITEM: u8 = 6`, `KEY_REPEAT_INITIAL: u16 = 12`, `KEY_REPEAT_INTERVAL: u16 = 3`.
  - `pub struct WeapselPlayer { pub weapons: [u32; 5], pub controller: u32 }` (`Clone, Copy, Debug, PartialEq, Eq`).
  - `pub struct WeapselConfig { pub weap_table: [u32; 40], pub select_bot_weapons: u32, pub players: [WeapselPlayer; 2] }` (`Clone, Debug, PartialEq, Eq`).
  - `pub struct PlayerSel { pub picks: [u32; 5], pub cursor: u8, pub ready: bool }` (`Clone, Copy, Debug, Default, PartialEq, Eq`).
  - `pub struct KeyRepeat` (private fields `prev: u32`, `held: [u16; 7]`; `Default`).
  - `pub enum WeapselError { WeaponCount(usize), WormCount(usize), NoWeaponsEnabled, InvalidPick { worm, slot, value } }` + `Display` + `Error`.
  - `pub struct WeaponSelection` (`Clone, Debug, PartialEq, Eq`); `WeaponSelection::new(&mut SimState, &WeapselConfig) -> Result<Self, WeapselError>`; `player(i) -> &PlayerSel`; `weapon_index(i, slot) -> usize`; `enabled_weaps() -> i32`; `held(i) -> [u16; 7]`; `menu_sounds() -> &[i32]`.
- Consumes: T0's `weap_order`; `SimState::{rand, weapons, worms}`; `WormState::{weapons, current_weapon}`; `Rand::bound_range`.

Why: design §3.2, §4.2, §4.3, §4.6. This is the constructor in C++ draw order: player 0's five slots, then player 1's. A slot draws once only if its saved pick is 0 or the player is a RANDOM bot (`weapsel.cpp:60-62`). It enters the redraw loop only if the resulting weapon is DISABLED (`:66`), and uniqueness is checked only inside that loop (`:67-75`, finding 1). Each slot writes `{type, ammo 0}` into the worm (`:82-85`); the frozen HUD reads these. Readiness is `controller != 0 && select_bot_weapons != 1` (`:95`). The three hazards that hang or crash C++ (finding 8), and a worm count other than two, are refused BEFORE any draw.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `rust/sim/src/weapsel.rs` (after the T0 tests). The helpers are shared with T2 and T3:

```rust
    // ---- 4½c T1: the constructor (weapsel.cpp:28-97) --------------------------------

    use crate::control::ControlConsts;
    use crate::physics::PhysicsConsts;
    use crate::state::{WeaponInit, WormInit};
    use assets::level::LevelData;
    use assets::sprite::SpriteSet;
    use assets::tc::SoundHooks;
    use sim_core::rng::Rand;
    use sim_core::vec::Vec2;

    pub(super) const MOVE_UP: i32 = 11;
    pub(super) const MOVE_DOWN: i32 = 12;
    pub(super) const SELECT: i32 = 13;

    /// `n` weapons named W00, W01, … so `weap_order` is the IDENTITY: pick `p` is weapon `p - 1`.
    fn weapons(n: usize) -> Vec<Weapon> {
        (0..n)
            .map(|i| Weapon {
                name: format!("W{i:02}"),
                id: i as i32,
                ammo: 100 + i as i32,
                ..Default::default()
            })
            .collect()
    }

    /// A 4x4 air level, `n_worms` fresh worms (no weapons), `n_weapons` identity-ordered weapons,
    /// menu sound hooks 11/12/13.
    fn state_n(seed: u32, n_weapons: usize, n_worms: usize) -> SimState {
        let level = LevelData {
            width: 4,
            height: 4,
            material_id: vec![0; 16],
            palette: None,
            display: None,
        };
        let worms: Vec<WormInit> = (0..n_worms)
            .map(|i| WormInit {
                index: i as i32,
                health: 100,
                lives: 0,
                stats_x: 0,
                weapons: [WeaponInit::default(); NUM_WEAPONS],
                start_pos: Vec2::zero(),
                visible: false,
            })
            .collect();
        let mut s = SimState::new(
            &level,
            &worms,
            seed,
            &[0u8; 256],
            weapons(n_weapons),
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
        s.sound_hooks = SoundHooks {
            MenuMoveUp: MOVE_UP,
            MenuMoveDown: MOVE_DOWN,
            MenuSelect: SELECT,
            ..SoundHooks::default()
        };
        s
    }

    fn state(seed: u32) -> SimState {
        state_n(seed, WEAPON_COUNT, 2)
    }

    fn cfg(
        weap_table: [u32; WEAPON_COUNT],
        select_bot_weapons: u32,
        p0: ([u32; NUM_WEAPONS], u32),
        p1: ([u32; NUM_WEAPONS], u32),
    ) -> WeapselConfig {
        WeapselConfig {
            weap_table,
            select_bot_weapons,
            players: [
                WeapselPlayer {
                    weapons: p0.0,
                    controller: p0.1,
                },
                WeapselPlayer {
                    weapons: p1.0,
                    controller: p1.1,
                },
            ],
        }
    }

    /// Two humans, every weapon enabled, the `Settings()` default picks `[1; 5]`, PICK.
    fn humans() -> WeapselConfig {
        cfg([0; WEAPON_COUNT], 1, ([1; 5], 0), ([1; 5], 0))
    }

    /// Only the 1-based `enabled` picks are enabled (identity order); the rest alternate 1/2.
    fn only(enabled: &[u32]) -> [u32; WEAPON_COUNT] {
        let mut t = [0u32; WEAPON_COUNT];
        for (i, v) in t.iter_mut().enumerate() {
            *v = 1 + (i as u32 % 2);
        }
        for &p in enabled {
            t[p as usize - 1] = 0;
        }
        t
    }

    /// The raw stream a `SimState` seeded with `seed` draws from.
    fn stream(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    #[test]
    fn saved_enabled_picks_draw_nothing_and_load_type_with_ammo_zero() {
        let mut st = state(1);
        st.worms[0].weapons[2].delay_left = 7; // :82-85 set type + ammo only
        let mut c = humans();
        c.players[0].weapons = [1, 2, 3, 4, 5];
        c.players[1].weapons = [40, 39, 38, 37, 36];
        let ws = WeaponSelection::new(&mut st, &c).expect("legal config");
        assert_eq!((st.rand.draws(), st.rand.last()), (0, 0), "no roll, no loop");
        assert_eq!(ws.player(0).picks, [1, 2, 3, 4, 5]);
        for j in 0..NUM_WEAPONS {
            assert_eq!(st.worms[0].weapons[j].ty, Some(j as i32));
            assert_eq!(st.worms[1].weapons[j].ty, Some(39 - j as i32));
            assert_eq!(st.worms[0].weapons[j].ammo, 0, "weapsel.cpp:84 ammo = 0");
        }
        assert_eq!(st.worms[0].weapons[2].delay_left, 7, "delay_left untouched");
        assert_eq!(st.worms[0].current_weapon, 0);
        assert_eq!(*ws.player(1), PlayerSel { picks: [40, 39, 38, 37, 36], cursor: 0, ready: false });
        assert_eq!(ws.enabled_weaps(), 40);
    }

    #[test]
    fn a_zero_pick_draws_once_and_an_enabled_duplicate_is_kept() {
        let mut st = state(99);
        let mut c = humans();
        c.players[0].weapons = [0, 7, 0, 7, 0];
        c.players[1].weapons = [3; 5];
        let mut r = stream(99);
        let want = [
            r.bound_range(1, 41),
            7,
            r.bound_range(1, 41),
            7,
            r.bound_range(1, 41),
        ];
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(st.rand.draws(), 3, "one draw per zero pick; all enabled => no loop");
        assert_eq!(ws.player(0).picks, want);
        assert_eq!(ws.player(1).picks, [3; 5], "finding 1: a duplicate outside the loop is kept");
    }

    #[test]
    fn a_random_bot_draws_all_five_even_over_saved_picks_and_is_ready() {
        let mut st = state(5);
        let c = cfg([0; WEAPON_COUNT], 0, ([2; 5], 0), ([9; 5], 1));
        let mut r = stream(5);
        let want: [u32; 5] = std::array::from_fn(|_| r.bound_range(1, 41));
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(st.rand.draws(), 5);
        assert_eq!(ws.player(0).picks, [2; 5], "the human keeps its picks");
        assert_eq!(ws.player(1).picks, want);
        assert!(!ws.player(0).ready && ws.player(1).ready, "weapsel.cpp:95");
    }

    #[test]
    fn a_disabled_pick_loops_until_an_enabled_weapon_duplicates_allowed() {
        // Only pick 8 enabled: `enough` is false, so uniqueness is waived (:72).
        let mut st = state(3);
        let c = cfg(only(&[8]), 1, ([1; 5], 0), ([8; 5], 0));
        let mut r = stream(3);
        let mut want_draws = 0;
        for _ in 0..NUM_WEAPONS {
            loop {
                want_draws += 1;
                if r.bound_range(1, 41) == 8 {
                    break;
                }
            }
        }
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(st.rand.draws(), want_draws, "player 1's enabled picks draw nothing");
        assert_eq!(ws.player(0).picks, [8; 5]);
        assert!(want_draws > 5, "non-vacuous: the loop iterated");
    }

    #[test]
    fn with_enough_weapons_the_loop_also_rejects_used_weapons() {
        // Exactly five enabled (picks 1..=5): the loop yields a permutation (:64, :72).
        let mut st = state(11);
        let c = cfg(only(&[1, 2, 3, 4, 5]), 1, ([40; 5], 0), ([1, 2, 3, 4, 5], 0));
        let mut r = stream(11);
        let mut used = [false; 41];
        let mut want = [0u32; 5];
        for slot in want.iter_mut() {
            *slot = loop {
                let p = r.bound_range(1, 41);
                if p <= 5 && !used[p as usize] {
                    break p;
                }
            };
            used[*slot as usize] = true;
        }
        let ws = WeaponSelection::new(&mut st, &c).unwrap();
        assert_eq!(ws.player(0).picks, want);
        let mut sorted = want;
        sorted.sort_unstable();
        assert_eq!(sorted, [1, 2, 3, 4, 5], "a permutation");
    }

    #[test]
    fn readiness_follows_the_raw_select_bot_weapons() {
        // Finding 2: RANDOM iff == 0, auto-ready iff != 1 — a value >= 3 behaves as KEEP.
        for (sbw, bot_ready) in [(0, true), (1, false), (2, true), (7, true)] {
            let mut st = state(2);
            let c = cfg([0; WEAPON_COUNT], sbw, ([1; 5], 0), ([1; 5], 1));
            let ws = WeaponSelection::new(&mut st, &c).unwrap();
            assert!(!ws.player(0).ready, "a human is never auto-ready");
            assert_eq!(ws.player(1).ready, bot_ready, "select_bot_weapons {sbw}");
            assert_eq!(st.rand.draws(), if sbw == 0 { 5 } else { 0 }, "sbw {sbw}");
        }
    }

    #[test]
    fn every_hazard_is_refused_before_any_draw() {
        let mut st = state_n(1, 39, 2);
        assert_eq!(WeaponSelection::new(&mut st, &humans()).unwrap_err(), WeapselError::WeaponCount(39));
        let mut st = state_n(1, WEAPON_COUNT, 3);
        assert_eq!(WeaponSelection::new(&mut st, &humans()).unwrap_err(), WeapselError::WormCount(3));
        let mut st = state(1);
        let none = cfg([1; WEAPON_COUNT], 1, ([0; 5], 0), ([0; 5], 0));
        assert_eq!(WeaponSelection::new(&mut st, &none).unwrap_err(), WeapselError::NoWeaponsEnabled);
        let mut c = humans();
        c.players[1].weapons[4] = 41;
        assert_eq!(
            WeaponSelection::new(&mut st, &c).unwrap_err(),
            WeapselError::InvalidPick { worm: 1, slot: 4, value: 41 }
        );
        assert_eq!(st.rand.draws(), 0, "a refused config draws nothing");
        assert!(st.worms[0].weapons.iter().all(|w| w.ty.is_none()), "and writes nothing");
    }
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p sim --lib weapsel`
Expected: FAIL to compile (`WeaponSelection`, `WeapselConfig`, … not found).

- [ ] **Step 3: Implement** — in `rust/sim/src/weapsel.rs`, replace the `use assets::object::Weapon;` line with the block below, and keep T0's `weap_order` after it:

```rust
use std::fmt;

use assets::object::Weapon;
use sim_core::rng::Rand;

use crate::state::{SimState, NUM_WEAPONS};

/// `rand(1, 41)` hard-codes forty weapons (`weapsel.cpp:61`, `:68`, `:323`, finding 8).
pub const WEAPON_COUNT: usize = 40;
/// RANDOMIZE, five weapon slots, DONE (`weapsel.cpp:49`, `:87`, `:90`).
pub const MENU_ITEMS: u8 = 7;
/// The RANDOMIZE item (`weapsel.cpp:316`).
pub const RANDOMIZE_ITEM: u8 = 0;
/// The DONE item (`weapsel.cpp:338`, "TODO: Unhardcode").
pub const DONE_ITEM: u8 = 6;
/// `LocalController::kKeyRepeatInitial` (`localController.hpp:44`).
pub const KEY_REPEAT_INITIAL: u16 = 12;
/// `LocalController::kKeyRepeatInterval` (`localController.hpp:45`).
pub const KEY_REPEAT_INTERVAL: u16 = 3;
/// The seven packed controls the repeat covers (`localController.cpp:130`).
const REPEAT_BITS: u32 = 7;

/// One player's saved setup the phase starts from (`WormSettings::weapons`, `::controller`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeapselPlayer {
    /// 1-based `weap_order` picks; `0` = unset, rolled by the constructor (`weapsel.cpp:60`).
    pub weapons: [u32; NUM_WEAPONS],
    /// 0 human, 1 DumbLieroAI, 2 FollowAI (`localController.cpp:19-27`).
    pub controller: u32,
}

/// What the phase reads from `Settings` (design §4.2). `scenario::build::weapsel_config` maps a
/// `Settings` onto it; `sim` does not depend on `scenario`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeapselConfig {
    /// `Settings::weap_table` by weapon index: 0 menu, 1 bonus only, 2 banned — enabled iff 0
    /// (`settings.hpp:68` is `uint32_t`, so `> 0`, `!= 0` and `<= 0` all mean the same).
    pub weap_table: [u32; WEAPON_COUNT],
    /// Raw `Settings::select_bot_weapons`: a bot is RANDOM iff `== 0` (`weapsel.cpp:57`) and
    /// readies at once iff `!= 1` (`:95`), so a file value >= 3 behaves as KEEP (finding 2).
    pub select_bot_weapons: u32,
    /// Index = worm index = viewport index (`localController.cpp:47-48`, `weapsel.cpp:44-47`).
    pub players: [WeapselPlayer; 2],
}

/// One player's menu (design §4.2). The menu is a seven-state cursor (finding 3: the items are
/// `emplace_back`ed, so `visible_item_count` stays 0 — no scrolling, no scrollbar), so picks,
/// cursor and the ready flag are the whole menu state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerSel {
    pub picks: [u32; NUM_WEAPONS],
    pub cursor: u8,
    pub ready: bool,
}

/// `LocalController`'s weapsel key repeat for one worm, on SAMPLED words (design §4.5): the
/// previous sampled word (replays `OnKey` from its change) and the per-bit held counters
/// (`worm_held_frames`, `localController.hpp:46`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyRepeat {
    prev: u32,
    held: [u16; REPEAT_BITS as usize],
}

/// A configuration C++ would hang or crash on (finding 8), refused instead (design §4.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeapselError {
    /// The TC does not have exactly 40 weapons (`rand(1, 41)` hard-codes 40).
    WeaponCount(usize),
    /// The phase needs exactly two worms (`localController.cpp:33-51`).
    WormCount(usize),
    /// Every weapon is disabled: the constructor loop, RANDOMIZE and cycling would never end.
    /// Mirrors `LS(NoWeaps)` (`weaponMenuState.cpp:91-109`).
    NoWeaponsEnabled,
    /// A saved pick above 40 indexes `weap_order` out of bounds (`weapsel.cpp:66`).
    InvalidPick { worm: usize, slot: usize, value: u32 },
}

impl fmt::Display for WeapselError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WeapselError::WeaponCount(n) => {
                write!(f, "weapon selection needs exactly {WEAPON_COUNT} weapons, the TC has {n}")
            }
            WeapselError::WormCount(n) => write!(f, "weapon selection needs 2 worms, got {n}"),
            WeapselError::NoWeaponsEnabled => write!(f, "no weapon is enabled"),
            WeapselError::InvalidPick { worm, slot, value } => {
                write!(f, "player {worm} weapon slot {slot}: {value} is not a weapon")
            }
        }
    }
}

impl std::error::Error for WeapselError {}

/// The phase (design §4.2): plain data, `Clone` — Step 5 snapshots it next to `SimState`
/// (design §8). `weap_order`, `weap_table` and `enabled_weaps` are derived once and immutable;
/// `menu_sounds` is per-frame output, not state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponSelection {
    weap_order: Vec<usize>,
    weap_table: [u32; WEAPON_COUNT],
    enabled_weaps: i32,
    players: [PlayerSel; 2],
    repeat: [KeyRepeat; 2],
    menu_sounds: Vec<i32>,
}

/// `game.rand(1, 41)` (`rand.hpp:36-38`: `rand(max - min) + min`, Lemire's bound).
fn roll(rand: &mut Rand) -> u32 {
    rand.bound_range(1, WEAPON_COUNT as u32 + 1)
}

impl WeaponSelection {
    /// The constructor (`weapsel.cpp:28-97`, design §3.2/§4.3), in C++ draw order: player 0's
    /// five slots, then player 1's. Draws `state.rand`, writes each worm's weapons as
    /// `{type, ammo 0}` (`:82-85`, the frozen HUD reads them) and `current_weapon = 0` (`:92`).
    /// Every refusal happens before the first draw.
    pub fn new(state: &mut SimState, cfg: &WeapselConfig) -> Result<Self, WeapselError> {
        if state.weapons.len() != WEAPON_COUNT {
            return Err(WeapselError::WeaponCount(state.weapons.len()));
        }
        if state.worms.len() != 2 {
            return Err(WeapselError::WormCount(state.worms.len()));
        }
        for (worm, p) in cfg.players.iter().enumerate() {
            for (slot, &value) in p.weapons.iter().enumerate() {
                if value as usize > WEAPON_COUNT {
                    return Err(WeapselError::InvalidPick { worm, slot, value });
                }
            }
        }
        // weapsel.cpp:35-39 — over all forty entries.
        let enabled_weaps = cfg.weap_table.iter().filter(|&&v| v == 0).count() as i32;
        if enabled_weaps == 0 {
            return Err(WeapselError::NoWeaponsEnabled);
        }
        let mut ws = WeaponSelection {
            weap_order: weap_order(&state.weapons),
            weap_table: cfg.weap_table,
            enabled_weaps,
            players: [PlayerSel::default(); 2],
            repeat: [KeyRepeat::default(); 2],
            menu_sounds: Vec::new(),
        };
        let enough = ws.enough();
        for i in 0..2 {
            let p = cfg.players[i];
            let random = p.controller != 0 && cfg.select_bot_weapons == 0; // :57
            let mut picks = p.weapons;
            let mut used = [false; WEAPON_COUNT]; // :42, per player
            for j in 0..NUM_WEAPONS {
                if picks[j] == 0 || random {
                    picks[j] = roll(&mut state.rand); // :60-62 — 0 or 1 draw
                }
                // :66 — the loop runs ONLY for a disabled pick, and checks uniqueness only
                // inside (finding 1): an enabled duplicate is kept.
                if ws.weap_table[ws.weapon_of(picks[j])] > 0 {
                    loop {
                        picks[j] = roll(&mut state.rand); // :68
                        let w = ws.weapon_of(picks[j]);
                        if (!enough || !used[w]) && ws.weap_table[w] == 0 {
                            break; // :72
                        }
                    }
                }
                let w = ws.weapon_of(picks[j]);
                used[w] = true; // :80
                let id = state.weapons[w].id;
                let slot = &mut state.worms[i].weapons[j];
                slot.ty = Some(id); // :85
                slot.ammo = 0; // :84
            }
            state.worms[i].current_weapon = 0; // :92
            ws.players[i] = PlayerSel {
                picks,
                cursor: RANDOMIZE_ITEM, // :94 MoveToFirstVisible
                ready: p.controller != 0 && cfg.select_bot_weapons != 1, // :95
            };
        }
        Ok(ws)
    }

    /// `enabled_weaps >= Settings::kSelectableWeapons` (`weapsel.cpp:64`, `:319`).
    fn enough(&self) -> bool {
        self.enabled_weaps >= NUM_WEAPONS as i32
    }

    /// The weapon index a 1-based pick names (`common.weap_order[pick - 1]`).
    fn weapon_of(&self, pick: u32) -> usize {
        self.weap_order[pick as usize - 1]
    }

    /// Player `i`'s menu (for drawing, the goldens and the write-back).
    pub fn player(&self, i: usize) -> &PlayerSel {
        &self.players[i]
    }

    /// The weapon index player `i`'s `slot` names — its menu label (`weapsel.cpp:87`, `:259`).
    pub fn weapon_index(&self, i: usize, slot: usize) -> usize {
        self.weapon_of(self.players[i].picks[slot])
    }

    /// `WeaponSelection::enabled_weaps` (`weapsel.hpp:23`).
    pub fn enabled_weaps(&self) -> i32 {
        self.enabled_weaps
    }

    /// Worm `i`'s seven key-repeat counters (the golden's `held` column).
    pub fn held(&self, i: usize) -> [u16; 7] {
        self.repeat[i].held
    }

    /// The menu sample ids the last `process_frame` played, in C++ call order (design §4.8):
    /// hash-inert, not snapshotted, drained by `game` into its `AudioSink`.
    pub fn menu_sounds(&self) -> &[i32] {
        &self.menu_sounds
    }
}
```

- [ ] **Step 4: GREEN**

Run: `cd /home/user/openliero/rust && cargo test -p sim --lib weapsel` — Expected: PASS, 9 tests.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/sim/src/weapsel.rs
git -C /home/user/openliero commit -m "sim(4.5c): WeaponSelection::new — the weapsel constructor in C++ draw order + the refusals" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

Reviewer (Opus): check line by line against `weapsel.cpp:28-97`: the loop is entered only for a disabled pick; `used` is reset per player; the order of draws is j-major within each player; nothing draws before a refusal; `delay_left`/`loading_left` are untouched.

---

### Task 2: `process_frame` — key repeat, cycling, the cursor, RANDOMIZE, DONE, the menu sounds  [Opus]

**Files:**
- Modify: `rust/sim/src/weapsel.rs` (`impl KeyRepeat`; `WeaponSelection::process_frame`, `play`, `randomize`; imports gain `ControlState`; new tests)

**Interfaces:**
- Produces (used by T3, T6–T9): `KeyRepeat::apply(&mut self, sampled: ControlState, ctl: &mut ControlState)`; `KeyRepeat::held(&self) -> [u16; 7]`; `WeaponSelection::process_frame(&mut self, state: &mut SimState, inputs: &[ControlState; 2]) -> bool` (true on every frame on which both players are ready, starting with the frame the last one readies).
- Consumes: T1's types; `ControlState::{get, set, press, release, pressed_once, pack, unpack}`; `SimState::sound_hooks.{MenuMoveUp, MenuMoveDown, MenuSelect}`.

Why: design §3.3, §3.4, §3.6, §4.4, §4.5, §4.8. One `LocalController::Process` weapsel frame runs in two stages. First the repeat for EVERY worm, ready or not (`localController.cpp:128-148`). Then `ProcessFrame` (`weapsel.cpp:219-350`), where the order inside the frame is load-bearing (finding 6): the slot comes from the cursor at frame start, then Left, Right, Up, Down, and confirm acts on the moved cursor. Fire is `Pressed`, not consumed, so RANDOMIZE re-rolls every held frame and plays no sound (finding 7). The sounds are crossed (finding 11): Up plays `MenuMoveDown` and Down plays `MenuMoveUp`; Left plays `MoveUp` and Right `MoveDown`. A negative hook is skipped, as `SoundPlayer::Play`'s `sound >= 0` guard does (`mixer/player.hpp:19`). The repeat is `LocalController`'s, replayed on sampled words (Q2), and `repeat_edge` pins it against the Rollback variant (finding 5).

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` (after T1's tests):

```rust
    // ---- 4½c T2: process_frame (localController.cpp:128-152, weapsel.cpp:219-350) ------

    use crate::state::ControlState;

    const UP: u32 = 1;
    const DOWN: u32 = 2;
    const LEFT: u32 = 4;
    const RIGHT: u32 = 8;
    const FIRE: u32 = 16;
    const JUMP: u32 = 64;

    fn step(ws: &mut WeaponSelection, st: &mut SimState, a: u32, b: u32) -> bool {
        ws.process_frame(st, &[ControlState::unpack(a), ControlState::unpack(b)])
    }

    /// One worm's repeat over `words`; after each frame the bits `consume(f)` are cleared, as
    /// ProcessFrame's Release/PressedOnce would. Returns the frames on which `bit` was set.
    fn repeat_frames(words: &[u32], bit: u32, consume: impl Fn(usize) -> bool) -> Vec<usize> {
        let mut r = KeyRepeat::default();
        let mut ctl = ControlState::new();
        let mut seen = Vec::new();
        for (f, &w) in words.iter().enumerate() {
            r.apply(ControlState::unpack(w), &mut ctl);
            if ctl.pack() & bit != 0 {
                seen.push(f);
                if consume(f) {
                    ctl = ControlState::unpack(ctl.pack() & !bit);
                }
            }
        }
        seen
    }

    #[test]
    fn a_consumed_held_key_repeats_at_held_frames_12_15_18() {
        // Rising edge on frame 0, then localController.cpp:136-139.
        assert_eq!(repeat_frames(&[RIGHT; 19], RIGHT, |_| true), vec![0, 12, 15, 18]);
    }

    #[test]
    fn an_unconsumed_held_key_keeps_its_counter_at_zero_finding_5() {
        // Left held from frame 0 but only read from frame 21 on (the repeat_edge case): Local
        // cycles on 21, 33, 36 — the Rollback controller would give 21, 24, 27.
        let consumed: Vec<usize> = repeat_frames(&[LEFT; 37], LEFT, |f| f >= 21)
            .into_iter()
            .filter(|&f| f >= 21)
            .collect();
        assert_eq!(consumed, vec![21, 33, 36]);
    }

    #[test]
    fn a_release_clears_the_bit_and_the_counter() {
        let mut r = KeyRepeat::default();
        let mut ctl = ControlState::new();
        for _ in 0..5 {
            r.apply(ControlState::unpack(DOWN), &mut ctl);
            ctl = ControlState::new(); // consumed every frame: the counter climbs
        }
        assert_eq!(r.held()[1], 4);
        r.apply(ControlState::new(), &mut ctl);
        assert_eq!((r.held()[1], ctl.pack()), (0, 0), "key-up: :144-146 + OnKey");
    }

    #[test]
    fn the_cursor_wraps_both_ways_with_the_crossed_sounds() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, UP, 0);
        assert_eq!((ws.player(0).cursor, ws.menu_sounds()), (6, &[MOVE_DOWN][..]));
        step(&mut ws, &mut st, 0, 0);
        assert!(ws.menu_sounds().is_empty(), "cleared every frame");
        step(&mut ws, &mut st, DOWN, 0);
        assert_eq!((ws.player(0).cursor, ws.menu_sounds()), (0, &[MOVE_UP][..]));
    }

    #[test]
    fn cycling_wraps_skips_disabled_weapons_and_updates_the_worm() {
        let mut t = [0u32; WEAPON_COUNT];
        t[39] = 1; // pick 40 disabled (bonus only still counts as disabled)
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &cfg(t, 1, ([2; 5], 0), ([2; 5], 0))).unwrap();
        for (bits, want_pick, want_sound) in [
            (DOWN, 2, MOVE_UP), // cursor 0 -> 1 (slot 0)
            (LEFT, 1, MOVE_UP),
            (LEFT, 39, MOVE_UP), // 1 -> 40 (disabled) -> 39
            (RIGHT, 1, MOVE_DOWN), // 39 -> 40 (disabled) -> 1
        ] {
            step(&mut ws, &mut st, bits, 0);
            assert_eq!(ws.player(0).picks[0], want_pick, "bits {bits}");
            assert_eq!(ws.menu_sounds(), &[want_sound][..]);
            step(&mut ws, &mut st, 0, 0);
        }
        assert_eq!(st.worms[0].weapons[0].ty, Some(0), "weapsel.cpp:258");
        assert_eq!(st.rand.draws(), 0, "cycling never draws");
    }

    #[test]
    fn with_one_weapon_enabled_a_cycle_is_a_full_lap_back() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &cfg(only(&[5]), 1, ([5; 5], 0), ([5; 5], 0))).unwrap();
        step(&mut ws, &mut st, DOWN, 0);
        step(&mut ws, &mut st, 0, 0);
        step(&mut ws, &mut st, LEFT, 0);
        assert_eq!((ws.player(0).picks[0], ws.menu_sounds().len()), (5, 1));
    }

    #[test]
    fn left_and_right_in_one_frame_cancel_with_two_sounds() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, DOWN, 0);
        step(&mut ws, &mut st, 0, 0);
        step(&mut ws, &mut st, LEFT | RIGHT, 0);
        assert_eq!(ws.player(0).picks[0], 1, "Left then Right: net zero");
        assert_eq!(ws.menu_sounds(), &[MOVE_UP, MOVE_DOWN][..]);
    }

    #[test]
    fn up_and_down_in_one_frame_cancel_with_two_sounds() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, UP | DOWN, 0);
        assert_eq!(ws.player(0).cursor, 0);
        assert_eq!(ws.menu_sounds(), &[MOVE_DOWN, MOVE_UP][..], ":293 then :304");
    }

    #[test]
    fn down_and_fire_on_slot_5_readies_in_the_same_frame_finding_6() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, UP, 0); // 0 -> 6
        step(&mut ws, &mut st, 0, 0);
        step(&mut ws, &mut st, UP, 0); // 6 -> 5
        step(&mut ws, &mut st, 0, 0);
        assert!(!step(&mut ws, &mut st, DOWN | FIRE, 0), "player 1 is not ready yet");
        assert!(ws.player(0).ready);
        assert_eq!(ws.menu_sounds(), &[MOVE_UP, SELECT][..]);
    }

    #[test]
    fn up_and_fire_on_slot_1_randomizes_in_the_same_frame() {
        let mut st = state(4);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, DOWN, 0);
        step(&mut ws, &mut st, 0, 0);
        let before = st.rand.draws();
        step(&mut ws, &mut st, UP | FIRE, 0);
        assert_eq!(ws.player(0).cursor, 0);
        assert!(st.rand.draws() - before >= 5, "RANDOMIZE ran on the moved cursor");
    }

    #[test]
    fn a_held_fire_rerolls_every_frame_silently_and_leaves_the_worm_types() {
        // Finding 7: Pressed(kFire) is not consumed; RANDOMIZE plays no sound and leaves
        // worm.weapons[].type stale (weapsel.cpp:316-337).
        let mut st = state(8);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        let types = st.worms[0].weapons.map(|w| w.ty);
        let mut r = stream(8);
        for frame in 0..3 {
            let before = st.rand.draws();
            step(&mut ws, &mut st, FIRE, 0);
            let mut used = [false; 41];
            let mut want = [0u32; 5];
            for slot in want.iter_mut() {
                *slot = loop {
                    let p = r.bound_range(1, 41);
                    if !used[p as usize] {
                        break p;
                    }
                };
                used[*slot as usize] = true;
            }
            assert_eq!(ws.player(0).picks, want, "frame {frame}: 40 enabled => unique picks");
            assert!(st.rand.draws() - before >= 5);
            assert!(ws.menu_sounds().is_empty());
        }
        assert_eq!(st.worms[0].weapons.map(|w| w.ty), types, "types stay stale");
    }

    #[test]
    fn randomize_without_enough_weapons_keeps_duplicates() {
        let mut st = state(6);
        let mut ws = WeaponSelection::new(&mut st, &cfg(only(&[3]), 1, ([3; 5], 0), ([3; 5], 0))).unwrap();
        step(&mut ws, &mut st, FIRE, 0);
        assert_eq!(ws.player(0).picks, [3; 5]);
        assert!(st.rand.draws() > 5);
    }

    #[test]
    fn a_ready_player_ignores_input_but_its_keys_still_repeat_state() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, UP, 0);
        step(&mut ws, &mut st, 0, 0);
        step(&mut ws, &mut st, FIRE, 0);
        assert!(ws.player(0).ready);
        for _ in 0..14 {
            assert!(!step(&mut ws, &mut st, DOWN | JUMP, 0));
        }
        assert_eq!(ws.player(0).cursor, 6, "no menu input once ready (:232)");
        assert_eq!(st.worms[0].control_states.pack(), DOWN | JUMP, "never consumed");
        assert_eq!(ws.held(0), [0; 7], "a set bit resets its counter (:140-143)");
    }

    #[test]
    fn done_needs_both_players_and_stays_true() {
        let mut st = state(1);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        assert!(!step(&mut ws, &mut st, UP, UP));
        assert!(!step(&mut ws, &mut st, FIRE, 0));
        assert!(step(&mut ws, &mut st, 0, FIRE), ":346 all_ready");
        assert_eq!(ws.menu_sounds(), &[SELECT][..]);
        assert!(step(&mut ws, &mut st, 0, 0));
    }

    #[test]
    fn a_negative_hook_plays_nothing() {
        let mut st = state(1);
        st.sound_hooks.MenuMoveUp = -1;
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        step(&mut ws, &mut st, DOWN, 0);
        assert!(ws.menu_sounds().is_empty(), "SoundPlayer::Play's sound >= 0 guard");
    }

    #[test]
    fn the_repeat_edge_cycles_at_frames_21_33_36() {
        // The corpus's repeat_edge case, end to end: Left held from frame 0 on RANDOMIZE,
        // Down at frame 20 (it runs after the Left check, so the first cycle is frame 21).
        let mut st = state(10);
        let mut ws = WeaponSelection::new(&mut st, &humans()).unwrap();
        let mut cycles = Vec::new();
        for f in 0..=36 {
            let before = ws.player(0).picks[0];
            step(&mut ws, &mut st, LEFT | if f == 20 { DOWN } else { 0 }, 0);
            if ws.player(0).picks[0] != before {
                cycles.push(f);
            }
        }
        assert_eq!(cycles, vec![21, 33, 36]);
    }
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p sim --lib weapsel`
Expected: FAIL to compile (`no method named 'apply'`, `no method named 'process_frame'`).

- [ ] **Step 3: Implement** — change the `use crate::state::{SimState, NUM_WEAPONS};` import to `use crate::state::{ControlState, SimState, NUM_WEAPONS};`, then add after `impl std::error::Error for WeapselError {}`:

```rust
impl KeyRepeat {
    /// One frame of `LocalController`'s weapsel input for one worm, on a SAMPLED word (design
    /// §4.5). First the key events the change implies (`OnKey`, `localController.cpp:58-67`:
    /// key-down sets the clean bit and the control bit, key-up clears both). Then the repeat
    /// loop verbatim (`:128-148`): a held key whose control bit was consumed counts up and is
    /// re-pressed at 12, 15, 18, …; a set bit or a released key resets its counter. `OnKey`'s
    /// Dig block (`:69-79`) is a no-op here: a sampled word has no bit 7, and a Left/Right
    /// control bit is only ever set while its clean bit is.
    pub fn apply(&mut self, sampled: ControlState, ctl: &mut ControlState) {
        let cur = sampled.pack();
        let rising = cur & !self.prev;
        let falling = self.prev & !cur;
        for bit in 0..REPEAT_BITS {
            if rising >> bit & 1 != 0 {
                ctl.set(bit, true);
            }
            if falling >> bit & 1 != 0 {
                ctl.set(bit, false);
            }
        }
        for bit in 0..REPEAT_BITS {
            let held = &mut self.held[bit as usize];
            if cur >> bit & 1 != 0 {
                if !ctl.get(bit) {
                    // uint16_t in C++ (localController.hpp:46): wraps, never saturates.
                    *held = held.wrapping_add(1);
                    if *held >= KEY_REPEAT_INITIAL
                        && (*held - KEY_REPEAT_INITIAL) % KEY_REPEAT_INTERVAL == 0
                    {
                        ctl.press(bit);
                    }
                } else {
                    *held = 0;
                }
            } else {
                *held = 0;
            }
        }
        self.prev = cur;
    }

    /// The seven held counters.
    pub fn held(&self) -> [u16; 7] {
        self.held
    }
}
```

and inside `impl WeaponSelection`, after `new`:

```rust
    /// One `LocalController::Process` weapsel frame (design §4.4): the key repeat for EVERY
    /// worm, ready or not (`localController.cpp:128-148`), then `ProcessFrame`
    /// (`weapsel.cpp:219-350`). Returns `all_ready`: true on the frame the last player readies
    /// (and on every later frame; the owner then calls `finalize`).
    pub fn process_frame(&mut self, state: &mut SimState, inputs: &[ControlState; 2]) -> bool {
        self.menu_sounds.clear();
        for (i, input) in inputs.iter().enumerate() {
            self.repeat[i].apply(*input, &mut state.worms[i].control_states);
        }
        let move_up = state.sound_hooks.MenuMoveUp;
        let move_down = state.sound_hooks.MenuMoveDown;
        let select = state.sound_hooks.MenuSelect;
        let n = self.weap_order.len() as u32; // common.weapons.size() (:253, :275)
        let mut all_ready = true;
        for i in 0..2 {
            if !self.players[i].ready {
                // :225 — the slot is fixed by the cursor at FRAME START (finding 6).
                let weap_id = self.players[i].cursor as i32 - 1;
                if (0..NUM_WEAPONS as i32).contains(&weap_id) {
                    let k = weap_id as usize;
                    if state.worms[i].control_states.get(ControlState::LEFT) {
                        state.worms[i].control_states.release(ControlState::LEFT); // :246
                        self.play(move_up); // :248
                        let mut pick = self.players[i].picks[k];
                        loop {
                            // :250-255, a do-while
                            pick = if pick <= 1 { n } else { pick - 1 };
                            if self.weap_table[self.weapon_of(pick)] == 0 {
                                break;
                            }
                        }
                        self.players[i].picks[k] = pick;
                        state.worms[i].weapons[k].ty = Some(state.weapons[self.weapon_of(pick)].id);
                    }
                    if state.worms[i].control_states.get(ControlState::RIGHT) {
                        state.worms[i].control_states.release(ControlState::RIGHT); // :269
                        self.play(move_down); // :271
                        let mut pick = self.players[i].picks[k];
                        loop {
                            // :273-278
                            pick = if pick >= n { 1 } else { pick + 1 };
                            if self.weap_table[self.weapon_of(pick)] == 0 {
                                break;
                            }
                        }
                        self.players[i].picks[k] = pick;
                        state.worms[i].weapons[k].ty = Some(state.weapons[self.weapon_of(pick)].id);
                    }
                }
                // :286-306 — Up plays MenuMoveDown and Down plays MenuMoveUp (finding 11); both
                // wrap over the seven items (finding 3).
                if state.worms[i].control_states.pressed_once(ControlState::UP) {
                    self.play(move_down);
                    self.players[i].cursor = (self.players[i].cursor + MENU_ITEMS - 1) % MENU_ITEMS;
                }
                if state.worms[i].control_states.pressed_once(ControlState::DOWN) {
                    self.play(move_up);
                    self.players[i].cursor = (self.players[i].cursor + 1) % MENU_ITEMS;
                }
                // :309 — Pressed, not consumed: a held Fire confirms every frame (finding 7).
                if state.worms[i].control_states.get(ControlState::FIRE) {
                    match self.players[i].cursor {
                        RANDOMIZE_ITEM => self.randomize(i, &mut state.rand),
                        DONE_ITEM => {
                            self.play(select); // :340
                            self.players[i].ready = true;
                        }
                        _ => {}
                    }
                }
            }
            all_ready = all_ready && self.players[i].ready; // :346
        }
        all_ready
    }

    /// `game.sound_player->Play(hook)`: `SoundPlayer::Play` skips a negative id
    /// (`mixer/player.hpp:19`).
    fn play(&mut self, sound: i32) {
        if sound >= 0 {
            self.menu_sounds.push(sound);
        }
    }

    /// RANDOMIZE (`weapsel.cpp:316-337`, design §3.4): a fresh `weap_used`; per slot the loop
    /// ALWAYS runs (unlike the constructor's) and enforces uniqueness when `enough`. No sound,
    /// and `worm.weapons[].type` is left stale (`finalize` overwrites it).
    fn randomize(&mut self, i: usize, rand: &mut Rand) {
        let enough = self.enough();
        let mut used = [false; WEAPON_COUNT];
        for j in 0..NUM_WEAPONS {
            let w = loop {
                let pick = roll(rand); // :323
                self.players[i].picks[j] = pick;
                let w = self.weapon_of(pick);
                if (!enough || !used[w]) && self.weap_table[w] == 0 {
                    break w; // :327
                }
            };
            used[w] = true; // :334
        }
    }
```

If the borrow checker rejects `state.worms[i].weapons[k].ty = Some(state.weapons[self.weapon_of(pick)].id);`, bind `let id = state.weapons[self.weapon_of(pick)].id;` first.

- [ ] **Step 4: GREEN**

Run: `cd /home/user/openliero/rust && cargo test -p sim --lib weapsel` — Expected: PASS, 25 tests.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/sim/src/weapsel.rs
git -C /home/user/openliero commit -m "sim(4.5c): process_frame — LocalController's 12/3 key repeat, cycling, cursor, RANDOMIZE, DONE, menu sounds" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

Reviewer (Opus): check line by line against `localController.cpp:128-148` and `weapsel.cpp:219-350`. The repeat runs for all worms before any menu; `kWeapId` is taken before the moves; Left runs before Right; Up runs before Down; confirm reads the moved cursor; RANDOMIZE's loop is unconditional; the crossed sounds; `all_ready` is folded over both players on every frame.

---
### Task 3: `finalize` / `init_weapons`, and the builder split `new_match` / `enter_game` / `weapsel_config`  [Opus]

**Files:**
- Modify: `rust/sim/src/weapsel.rs` (`WeaponSelection::finalize`, `pub fn init_weapons`, `fn release_controls`; imports gain `WormWeapon`; a test)
- Modify: `rust/scenario/src/build.rs` (module doc `:1-11`; imports `:13-26`; `BuildError::InvalidWeapon` doc; `validate` `:78-111`; `build_match` `:113-227` split; new tests at the end of `mod tests`)

**Interfaces:**
- Produces (used by T6–T9):
  - `WeaponSelection::finalize(self, state: &mut SimState) -> [[u32; 5]; 2]` (`init_weapons` + release bits 0..6 of every worm; returns the picks for the write-back).
  - `pub fn sim::weapsel::init_weapons(state: &mut SimState, weap_order: &[usize], picks: &[[u32; 5]; 2])` (`Worm::InitWeapons` ×2).
  - `pub fn scenario::build::validate_for_selection(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError>` (as `validate`, but a `0` pick is legal).
  - `pub fn scenario::build::new_match(tc_root: &Path, cfg: &MatchConfig, level: &LevelData) -> Result<Loaded, BuildError>` (the `LocalController` constructor: `lives = 0`, empty weapon slots, the default blood pool).
  - `pub fn scenario::build::enter_game(state: &mut SimState, cfg: &MatchConfig)` (`lives = settings.lives`; `bobjects = BloodPool::new(blood_particle_max)`).
  - `pub fn scenario::build::weapsel_config(s: &Settings) -> sim::weapsel::WeapselConfig`.
  - `build_match` = `new_match` (strict picks) → `init_weapons(cfg picks)` → `enter_game`, with byte-identical output.
- Consumes: T1/T2's `WeaponSelection`; T0's `weap_order`.

Why: design §3.5, §4.7. `Finalize` is `InitWeapons` for every worm plus `ReleaseControls` (`weapsel.cpp:352-361`, `worm.cpp:698-709`, `game.cpp:110-118`), and it draws nothing. The 4½a seam (4½a design §4.4) becomes three functions, so a match can run the phase between the `LocalController` constructor and `kStateGame`. `build_match` keeps its strict validation and its output. Its unit tests and the eight settings-path goldens prove that. A pick of `0` is legal only in front of a selection, which rolls it (`weapsel.cpp:60`).

- [ ] **Step 1: Write the failing tests**

(a) Append inside `mod tests` in `rust/sim/src/weapsel.rs`:

```rust
    // ---- 4½c T3: Finalize (weapsel.cpp:352-361) -----------------------------------------

    #[test]
    fn finalize_loads_every_pick_with_full_ammo_and_releases_the_controls() {
        let mut st = state(5);
        let c = cfg([0; WEAPON_COUNT], 1, ([3, 4, 5, 6, 7], 0), ([8, 9, 10, 11, 12], 0));
        let mut ws = WeaponSelection::new(&mut st, &c).unwrap();
        st.worms[0].weapons[1].delay_left = 9;
        st.worms[1].current_weapon = 3;
        // Left at cursor 0, Jump and Change are never read: their bits stay set (32 = Change).
        step(&mut ws, &mut st, LEFT | JUMP | 32, LEFT);
        assert_ne!(st.worms[0].control_states.pack(), 0);
        let draws = st.rand.draws();
        let picks = ws.finalize(&mut st);
        assert_eq!(picks, [[3, 4, 5, 6, 7], [8, 9, 10, 11, 12]]);
        assert_eq!(st.rand.draws(), draws, "Finalize draws nothing (design §3.5)");
        for (i, worm) in st.worms.iter().enumerate() {
            assert_eq!(worm.control_states.pack(), 0, "ReleaseControls (game.cpp:110-118)");
            assert_eq!(worm.current_weapon, 0, "worm.cpp:700");
            for (j, w) in worm.weapons.iter().enumerate() {
                let id = picks[i][j] as i32 - 1; // identity weap_order
                assert_eq!((w.ty, w.ammo, w.delay_left, w.loading_left), (Some(id), 100 + id, 0, 0));
            }
        }
    }
```

(b) Append inside `mod tests` in `rust/scenario/src/build.rs`:

```rust
    // ---- 4½c T3: the seam split (design §4.7) ------------------------------------------

    use sim::hash::{hash_components, hash_game_state};

    #[test]
    fn a_zero_pick_is_legal_only_in_front_of_a_selection() {
        let mut c = cfg();
        c.settings.worm_settings[0].weapons = [0, 5, 0, 40, 0];
        assert_eq!(validate_for_selection(&c, 40), Ok(()));
        assert!(validate(&c, 40).is_err(), "build_match stays strict");
        c.settings.worm_settings[1].weapons[2] = 41;
        assert_eq!(
            validate_for_selection(&c, 40),
            Err(BuildError::InvalidWeapon { worm: 1, slot: 2, value: 41 })
        );
    }

    #[test]
    fn new_match_is_the_localcontroller_start_before_selection() {
        let mut c = cfg();
        c.settings.worm_settings[0].weapons = [0, 5, 0, 40, 0];
        c.settings.lives = 7;
        let st = new_match(Path::new(TC_ROOT), &c, &level()).expect("a zero pick is legal here").state;
        for (i, w) in st.worms.iter().enumerate() {
            assert_eq!(
                (w.lives, w.health, w.visible, w.stats_x, w.killed_timer),
                (0, 100, false, [0, 218][i], 150),
                "Worm::lives{{0}} (worm.hpp:238) until kStateGame"
            );
            assert!(w.weapons.iter().all(|ww| ww.ty.is_none() && ww.ammo == 0), "no InitWeapons yet");
        }
        assert_eq!(st.rand.last(), 0, "building consumes no RNG");
        assert_eq!(st.weap_table.len(), 40);
    }

    #[test]
    fn build_match_is_new_match_then_init_weapons_then_enter_game() {
        let mut c = cfg();
        c.settings.lives = 7;
        c.settings.blood_particle_max = 300;
        c.settings.worm_settings[0].weapons = [2, 3, 4, 5, 6];
        c.settings.worm_settings[1].weapons = [40, 12, 1, 26, 31];
        let built = build_match(Path::new(TC_ROOT), &c, &level()).unwrap().state;
        let mut split = new_match(Path::new(TC_ROOT), &c, &level()).unwrap().state;
        let order = sim::weapsel::weap_order(&split.weapons);
        sim::weapsel::init_weapons(
            &mut split,
            &order,
            &[c.settings.worm_settings[0].weapons, c.settings.worm_settings[1].weapons],
        );
        enter_game(&mut split, &c);
        assert_eq!(built.worms, split.worms);
        assert_eq!(hash_components(&built), hash_components(&split));
        assert_eq!(hash_game_state(&built), hash_game_state(&split));
        assert_eq!(built.bobjects.capacity(), split.bobjects.capacity());
        assert_eq!(split.bobjects.capacity(), 300);
        assert_eq!(split.worms[1].lives, 7);
    }

    #[test]
    fn weapsel_config_maps_the_settings() {
        let mut s = Settings::default();
        s.weap_table[3] = 2;
        s.select_bot_weapons = 0;
        s.worm_settings[1].controller = 1;
        s.worm_settings[0].weapons = [0, 1, 2, 3, 4];
        s.worm_settings[2].weapons = [9; 5]; // the network player is not a player here
        let w = weapsel_config(&s);
        assert_eq!(w.weap_table, s.weap_table);
        assert_eq!(w.select_bot_weapons, 0);
        assert_eq!(w.players[0].weapons, [0, 1, 2, 3, 4]);
        assert_eq!((w.players[0].controller, w.players[1].controller), (0, 1));
        assert_eq!(w.players[1].weapons, [1; 5]);
    }
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p sim --lib finalize_loads` — Expected: FAIL to compile (`no method named 'finalize'`).
Run: `cd /home/user/openliero/rust && cargo test -p scenario --lib build` — Expected: FAIL to compile (`new_match`, `enter_game`, `weapsel_config`, `validate_for_selection` not found).

- [ ] **Step 3: Implement the sim side** — in `rust/sim/src/weapsel.rs`, change the state import to `use crate::state::{ControlState, SimState, WormWeapon, NUM_WEAPONS};`, add inside `impl WeaponSelection` (after `randomize`):

```rust
    /// `WeaponSelection::Finalize` (`weapsel.cpp:352-361`, design §3.5): `InitWeapons` for
    /// every worm, then `Game::ReleaseControls`. Draws nothing. Returns the picks, which the
    /// owner writes back to its in-memory settings (the C++ `shared_ptr` aliasing, finding 4).
    pub fn finalize(self, state: &mut SimState) -> [[u32; NUM_WEAPONS]; 2] {
        let picks = [self.players[0].picks, self.players[1].picks];
        init_weapons(state, &self.weap_order, &picks);
        release_controls(state);
        picks
    }
```

and after the `impl WeaponSelection` block:

```rust
/// `Worm::InitWeapons` for worms 0 and 1 (`worm.cpp:698-709`): `current_weapon = 0`, and per
/// slot `type = weapons[weap_order[pick - 1]]`, `ammo = type.ammo`, `delay_left =
/// loading_left = 0`. Shared by `finalize` and `scenario::build::build_match` (design §4.7).
/// The caller guarantees two worms and picks in `1..=weap_order.len()`.
pub fn init_weapons(state: &mut SimState, weap_order: &[usize], picks: &[[u32; NUM_WEAPONS]; 2]) {
    for (i, worm_picks) in picks.iter().enumerate() {
        state.worms[i].current_weapon = 0;
        for (j, &pick) in worm_picks.iter().enumerate() {
            let w = &state.weapons[weap_order[pick as usize - 1]];
            let (id, ammo) = (w.id, w.ammo);
            state.worms[i].weapons[j] = WormWeapon {
                ty: Some(id),
                ammo,
                delay_left: 0,
                loading_left: 0,
            };
        }
    }
}

/// `Game::ReleaseControls` (`game.cpp:110-118`): release control bits 0..6 of every worm.
fn release_controls(state: &mut SimState) {
    for worm in state.worms.iter_mut() {
        for bit in 0..REPEAT_BITS {
            worm.control_states.release(bit);
        }
    }
}
```

- [ ] **Step 4: Implement the builder** — in `rust/scenario/src/build.rs`:

(1) Module doc: after the existing paragraph append

```rust
//!
//! Step 4½c (design §4.7) cuts the seam in three: [`new_match`] is the `LocalController`
//! constructor (`localController.cpp:30-54`: `health`, `stats_x`, invisible, `lives = 0`
//! (`worm.hpp:238`), no weapons); `sim::weapsel` runs the selection phase on it; [`enter_game`]
//! is `ChangeState(kStateGame)`'s lives (`:232-235`) + `StartGame`'s pool (`game.cpp:513`).
//! [`build_match`] = `new_match` → `sim::weapsel::init_weapons` (the saved picks) → `enter_game`,
//! with output identical to before.
```

(2) Imports: replace `use sim::state::{SimState, WormInit};` with

```rust
use sim::state::{SimState, WeaponInit, WormInit, NUM_WEAPONS};
use sim::weapsel::{WeapselConfig, WeapselPlayer};
```

and `use crate::settings::{MatchConfig, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE, WEAP_TABLE_LEN};` with

```rust
use crate::settings::{MatchConfig, Settings, GM_HOLDAZONE, GM_SCALES_OF_JUSTICE, WEAP_TABLE_LEN};
```

(3) In `BuildError`, change the `InvalidWeapon` doc to: `/// A weapon pick outside 1..=weap_order.len() (worm.cpp:704 indexes unchecked) — or, in front of a selection (new_match), outside 0..=weap_order.len().`

(4) Replace the `validate` function (`:78-111`) with

```rust
/// How [`validate`] treats a pick of `0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picks {
    /// `build_match`: every pick names a weapon (`InitWeapons` runs at once).
    Strict,
    /// `new_match`: `0` is unset and the selection's constructor rolls it (`weapsel.cpp:60`).
    AllowUnset,
}

/// Check `cfg` against a TC with `n_weapons` weapons. Only the two playing worms
/// (indices 0 and 1) are checked; the network player never plays a local match.
pub fn validate(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError> {
    validate_with(cfg, n_weapons, Picks::Strict)
}

/// [`validate`] for a match that runs weapon selection first: a `0` pick is legal (design §4.7).
pub fn validate_for_selection(cfg: &MatchConfig, n_weapons: usize) -> Result<(), BuildError> {
    validate_with(cfg, n_weapons, Picks::AllowUnset)
}

fn validate_with(cfg: &MatchConfig, n_weapons: usize, picks: Picks) -> Result<(), BuildError> {
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
            if (value == 0 && picks == Picks::Strict) || value as usize > n_weapons {
                return Err(BuildError::InvalidWeapon { worm, slot, value });
            }
        }
    }
    if s.blood_particle_max < 1 {
        return Err(BuildError::InvalidBloodParticleMax(s.blood_particle_max));
    }
    Ok(())
}
```

(5) Replace the whole `build_match` function (`:113-227`) with the four functions below. The body of `new_match_with` is the old `build_match` body with exactly four changes: `validate` becomes `validate_with(…, picks)`; `weap_order` goes away; the `WormInit`s get `lives: 0` and `weapons: [WeaponInit::default(); NUM_WEAPONS]`; and the `state.bobjects = …` line is dropped (it moves to `enter_game`). Every other line stays verbatim, including the TC-constant block and the palette block:

```rust
/// The `LocalController` constructor (`localController.cpp:30-54`): everything
/// [`build_match`] does EXCEPT the weapons, the lives and the blood pool, which weapon
/// selection and [`enter_game`] supply (design §4.7). A `0` pick is legal (the selection rolls
/// it). Worms: `health = ws.health`, `stats_x` 0/218, invisible, `lives = 0` (`worm.hpp:238`),
/// empty weapon slots; the blood pool keeps `SimState::new`'s default until `enter_game`.
pub fn new_match(tc_root: &Path, cfg: &MatchConfig, level: &LevelData) -> Result<Loaded, BuildError> {
    new_match_with(tc_root, cfg, level, Picks::AllowUnset)
}

fn new_match_with(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
    picks: Picks,
) -> Result<Loaded, BuildError> {
    let tc = TcConfig::load(&crate::assets::read_asset(tc_root, "tc.cfg")).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        Ok(crate::assets::read_asset(
            tc_root,
            &format!("{sub}/{id}.cfg"),
        ))
    })
    .expect("object configs load");
    validate_with(cfg, objects.weapons.len(), picks)?;
    let s = &cfg.settings;

    let worms_init: Vec<WormInit> = (0..2)
        .map(|i| WormInit {
            index: i as i32,
            health: s.worm_settings[i].health,
            lives: 0, // Worm::lives{0} (worm.hpp:238): set from settings only at kStateGame
            stats_x: if i == 0 { 0 } else { 218 },
            weapons: [WeaponInit::default(); NUM_WEAPONS], // no InitWeapons before selection
            start_pos: Vec2::zero(),
            visible: false,
        })
        .collect();

    let mut state = SimState::new(
        // … the SimState::new call, verbatim …
    );

    // … the TC-constant block, verbatim (num_blood_colours … sound_hooks) …

    // Settings (design §4.2). The blood pool is `enter_game`'s (StartGame, game.cpp:513).
    state.settings_max_bonuses = s.max_bonuses;
    state.weap_table = s.weap_table.iter().map(|&v| v as i32).collect();
    state.settings_health = s.worm_settings[0].health; // == [1] (validated)
    state.game_mode = s.game_mode;
    state.time_to_lose = s.time_to_lose;
    state.shadow = s.shadow;

    // … the palette block + `scene_data` + `Ok(Loaded { … })`, verbatim …
}

/// `ChangeState(kStateGame)` after weapon selection (`localController.cpp:232-235`: `lives =
/// settings.lives`) and `Game::StartGame`'s blood pool (`game.cpp:513`). Draws nothing.
pub fn enter_game(state: &mut SimState, cfg: &MatchConfig) {
    for worm in state.worms.iter_mut() {
        worm.lives = cfg.settings.lives;
    }
    state.bobjects = BloodPool::new(cfg.settings.blood_particle_max as usize);
}

/// Build the tick-0 match for `cfg` on the ready `level` (design §4.2) — the C++ settings path
/// without a selection phase: [`new_match`] (strict picks) → `InitWeapons` from the saved
/// picks → [`enter_game`]. Output identical to the 4½a-1 builder (the unit tests + the eight
/// settings-path goldens).
pub fn build_match(
    tc_root: &Path,
    cfg: &MatchConfig,
    level: &LevelData,
) -> Result<Loaded, BuildError> {
    let mut loaded = new_match_with(tc_root, cfg, level, Picks::Strict)?;
    let order = sim::weapsel::weap_order(&loaded.state.weapons);
    let s = &cfg.settings;
    sim::weapsel::init_weapons(
        &mut loaded.state,
        &order,
        &[s.worm_settings[0].weapons, s.worm_settings[1].weapons],
    );
    enter_game(&mut loaded.state, cfg);
    Ok(loaded)
}

/// The `WeapselConfig` a `Settings` gives (design §4.7): `weap_table`, the raw
/// `select_bot_weapons`, and players 0/1's picks + controller (the network player never plays
/// a local match).
pub fn weapsel_config(s: &Settings) -> WeapselConfig {
    WeapselConfig {
        weap_table: s.weap_table,
        select_bot_weapons: s.select_bot_weapons,
        players: [0, 1].map(|i| WeapselPlayer {
            weapons: s.worm_settings[i].weapons,
            controller: s.worm_settings[i].controller,
        }),
    }
}
```

(Copy the three "verbatim" regions from the old body; do not retype them. `WormInit::resolve_weapons` is no longer called from `build.rs`, but it stays in `sim::state` for `scenario::load`.)

- [ ] **Step 5: GREEN + the re-diff (the eight settings-path goldens and `build.rs`'s tests are the proof)**

Run: `cd /home/user/openliero/rust && cargo test -p sim --lib weapsel` — Expected: PASS, 26 tests.
Run: `cd /home/user/openliero/rust && cargo test -p scenario --lib build` — Expected: PASS (the 9 prior tests + 4 new).
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test sim_slice4_5a_settings_golden --test sim_slice4_5a_builder_golden --test sim_slice4_5c0_weapons_golden` — Expected: PASS (`build_match`'s output is unchanged).
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 6: Commit**

```
git -C /home/user/openliero add rust/sim/src/weapsel.rs rust/scenario/src/build.rs
git -C /home/user/openliero commit -m "sim+scenario(4.5c): Finalize/init_weapons + the seam split new_match/enter_game/weapsel_config; build_match rebuilt on them" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---

### Task 4: The `weapsel` directive in the Rust scenario parser  [Sonnet]

**Files:**
- Modify: `rust/scenario/src/parser.rs` (grammar doc `:9-29`; `Scenario` field; `parse` local + arm + the final check + the struct literal; two accessors; `to_text`; new tests)

**Interfaces:**
- Produces (used by T6, T7, T8): the grammar line `weapsel <frame> <worm0_7bit> <worm1_7bit>`; `Scenario::weapsel_end(&self) -> Option<u32>` (the last `weapsel` frame; `None` = no phase); `Scenario::weapsel_input(&self, frame: u32, worm: usize) -> u32` (raw word; 0 when absent; callers mask with `ControlState::unpack`, as for `input`); `to_text` writes the `weapsel` lines after `settings`, ascending, ALL of them (an all-zero line is meaningful), before `input`.
- Consumes: nothing new.

Why: design §6.1. The phase needs per-frame inputs, and they have no home in the format. The sidecar is C++-schema TOML (byte-gated). `input` lines are match ticks. A separate file would give the continuation goldens two sources of truth. So there is one directive, shaped like `input`, legal only with `settings`. Its presence opts in (`weapsel 0 0 0` means "run it with no input"). The last line's frame must be exactly the frame the phase ends on; the dumpers and the Rust drivers enforce that (T5, T6). `scenario::load` already refuses a `settings` file (`loader.rs:101-105`). 4½d promotes both directives to the recording format.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `parser.rs`:

```rust
    // ---- Step 4½c: the `weapsel` directive (design §6.1) ------------------------------

    const WEAPSEL: &str = "\
seed 1
level Levels/render_stage.lev
ticks 0
settings s_setup.cfg
weapsel 0 1 0
# frame 1 is absent: both words 0
weapsel 2 16 144
";

    #[test]
    fn weapsel_lines_are_sparse_and_end_at_the_last_frame() {
        let s = Scenario::parse(WEAPSEL).expect("parses");
        assert_eq!(s.weapsel_end(), Some(2));
        assert_eq!(
            (s.weapsel_input(0, 0), s.weapsel_input(1, 0), s.weapsel_input(2, 1)),
            (1, 0, 144)
        );
        assert_eq!(s.weapsel_input(9, 0), 0);
        assert_eq!(
            sim::state::ControlState::unpack(s.weapsel_input(2, 1)).pack(),
            16,
            "masked to 7 bits at use, like `input`"
        );
    }

    #[test]
    fn a_scenario_without_weapsel_has_no_phase() {
        assert_eq!(Scenario::parse(SAMPLE).unwrap().weapsel_end(), None);
        let settings_only = WEAPSEL.lines().filter(|l| !l.starts_with("weapsel")).collect::<Vec<_>>().join("\n");
        assert_eq!(Scenario::parse(&settings_only).unwrap().weapsel_end(), None);
    }

    #[test]
    fn weapsel_is_oracle_only_it_needs_settings() {
        let e = Scenario::parse("seed 1\nlevel a.lev\nticks 0\nworm 0 0 0 100 10 0 1\nworm 1 0 0 100 10 218 1\nweapsel 0 0 0\n")
            .unwrap_err();
        assert!(e.contains("weapsel") && e.contains("settings"), "{e}");
    }

    #[test]
    fn weapsel_rejects_duplicates_bad_arity_and_negative_frames() {
        for (bad, why) in [
            ("weapsel 0 1 0\nweapsel 0 2 0\n", "duplicate"),
            ("weapsel 0 1\n", "expects 3"),
            ("weapsel 0 1 2 3\n", "expects 3"),
            ("weapsel -1 0 0\n", "negative"),
        ] {
            let text = format!("seed 1\nlevel a.lev\nticks 0\nsettings s.cfg\n{bad}");
            let e = Scenario::parse(&text).unwrap_err();
            assert!(e.contains(why), "{bad:?}: {e}");
        }
    }

    #[test]
    fn weapsel_round_trips_through_to_text_including_an_all_zero_line() {
        let text = "seed 3\nlevel a.lev\nticks 5\nsettings s.cfg\nweapsel 4 0 0\nweapsel 1 8 0\ninput 2 16 0\n";
        let s = Scenario::parse(text).unwrap();
        let out = s.to_text();
        let sel = out.find("settings s.cfg").unwrap();
        let w1 = out.find("weapsel 1 8 0\n").expect("sorted");
        let w4 = out.find("weapsel 4 0 0\n").expect("the all-zero end line is kept");
        let inp = out.find("input 2 16 0").unwrap();
        assert!(sel < w1 && w1 < w4 && w4 < inp, "{out}");
        assert_eq!(Scenario::parse(&out).unwrap(), s);
    }
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p scenario --lib parser` — Expected: FAIL to compile (`weapsel_end` / `weapsel_input` not found).

- [ ] **Step 3: Implement**

(1) Grammar doc: after the `settings` block of the module doc add

```rust
//! weapsel     <frame> <worm0_7bit> <worm1_7bit>  # Step 4½c; oracle-only, needs `settings`: the
//!                                                 # weapon-selection phase input, sparse (absent
//!                                                 # => 0); the LAST line's frame is the frame the
//!                                                 # phase ends on — see [`Scenario::weapsel_end`]
```

(2) `Scenario` field, after `settings`:

```rust
    /// Step-4½c `weapsel <frame> <worm0_7bit> <worm1_7bit>` — the weapon-selection phase's
    /// sparse per-frame input (design §6.1). Oracle-only, legal only with `settings`. Present
    /// => the phase runs (constructor + frames `0..=weapsel_end()`) before match tick 0; the
    /// last line's frame is exactly the frame `ProcessFrame` returns true (the dumpers and the
    /// Rust drivers check it). Empty on every pre-4½c scenario.
    weapsel: HashMap<u32, (u32, u32)>,
```

(3) In `parse`: add `let mut weapsel: HashMap<u32, (u32, u32)> = HashMap::new();` next to `inputs`, and the arm (after `"input"`):

```rust
                "weapsel" => {
                    // Step 4½c: shaped like `input`; a frame is 0-based, the first ProcessFrame.
                    expect_args(n, key, &nums, 3)?;
                    let frame = parse_at(0)?;
                    if frame < 0 {
                        return Err(format!("line {n}: weapsel frame {frame} is negative"));
                    }
                    let w0 = parse_at(1)? as u32;
                    let w1 = parse_at(2)? as u32;
                    if weapsel.insert(frame as u32, (w0, w1)).is_some() {
                        return Err(format!("line {n}: duplicate weapsel for frame {frame}"));
                    }
                }
```

After the existing `settings` exclusion check, add

```rust
        if !weapsel.is_empty() && settings.is_none() {
            return Err("`weapsel` is oracle-only: it needs a `settings` directive".to_string());
        }
```

and add `weapsel,` to the `Ok(Scenario { … })` literal after `settings,`.

(4) Accessors, after `input`:

```rust
    /// The frame the weapon-selection phase ends on — the last `weapsel` line's frame — or
    /// `None` when the scenario has no phase (design §6.1).
    pub fn weapsel_end(&self) -> Option<u32> {
        self.weapsel.keys().copied().max()
    }

    /// The raw `weapsel` word for `worm` (0 or 1) at `frame`; `0` when the frame has no line.
    /// Callers mask it through `ControlState::unpack`, as for [`Scenario::input`].
    pub fn weapsel_input(&self, frame: u32, worm: usize) -> u32 {
        let (w0, w1) = self.weapsel.get(&frame).copied().unwrap_or((0, 0));
        match worm {
            0 => w0,
            1 => w1,
            _ => 0,
        }
    }
```

(5) `to_text`: right after the `match &self.settings { … }` block add

```rust
        // Step 4½c: every `weapsel` line, ascending — an all-zero line is kept (its presence
        // opts in, and the last one marks the end frame).
        let mut frames: Vec<u32> = self.weapsel.keys().copied().collect();
        frames.sort_unstable();
        for f in frames {
            let (w0, w1) = self.weapsel[&f];
            out.push_str(&format!("weapsel {f} {w0} {w1}\n"));
        }
```

Also add `weapsel` to the `to_text` doc's directive list.

(6) If a test module or file builds `Scenario { … }` by literal, add `weapsel: HashMap::new()` there. Check with `grep -rn "Scenario {" /home/user/openliero/rust --include=*.rs`; at plan time the only literal is in `parse`.

- [ ] **Step 4: GREEN + the re-diff**

Run: `cd /home/user/openliero/rust && cargo test -p scenario --lib parser` — Expected: PASS (5 new).
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS (no committed scenario has a `weapsel` line).
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS (`round_trip.rs`/`record_regression.rs` round-trip `to_text` unchanged).

- [ ] **Step 5: Commit**

```
git -C /home/user/openliero add rust/scenario/src/parser.rs
git -C /home/user/openliero commit -m "scenario(4.5c): the oracle-only weapsel <frame> <w0> <w1> directive (needs settings) + to_text" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 5: C++ — the shared driver, `oracle_dump_weapsel` (+ self-check), `sim_physics_dump` learns `weapsel`  [Opus]

**Files:**
- Create: `src/tools/oracle_dump/weapsel_drive.hpp`
- Create: `src/tools/oracle_dump/weapsel_dump.cpp`
- Modify: `src/tools/oracle_dump/sim_physics_dump.cpp` (header comment `:37-71`; includes `:151`; `struct Scenario` `:175-243`; `ParseScenario` `:280-341`; the settings path `:450-468`)
- Modify: `CMakeLists.txt` (inside `if(OPENLIERO_BUILD_ORACLE_DUMP)`, before its `endif()` at `:396`)

**Interfaces:**
- Produces (used by T6):
  - `oracle_dump_weapsel <scenario.txt> <out.txt>` (run from the repo root). It writes the golden format of design §6.3, with the plan-time refinement of a `final` line that carries the control words:
    ```
    init <enabled> <p0> <p1> <draws> <last> <next>
    f <frame> <in0> <in1> <p0> <p1> <ctl0> <ctl1> <held0> <held1> <sounds> <draws> <last> <next> <done>
    final <l0> <l1> <ctl0> <ctl1> <last> <next>
    ```
    `pN` = `a,b,c,d,e:cursor:ready` (1-based picks from `WormSettings::weapons`, `Menu::Selection()`, `is_ready` 0/1). `inN` = the masked 7-bit word, in decimal. `ctlN` = `control_states.Pack()` as `%02x`. `heldN` = the seven repeat counters, comma-joined. `sounds` = sample ids in `Play` order, comma-joined, or `-`. `draws` = RNG steps in this step (decimal). `last`/`next` = `rand.last` and the next raw value as `%08x`. `done` = 0/1. `lN` = five `weapon_id:ammo` pairs, comma-joined, then `:current_weapon`. Lines starting with `#` are provenance.
  - `oracle_dump_sim_physics` accepts `weapsel <frame> <w0> <w1>` (only with `settings`). In the settings path the phase runs in place of the bare `InitWeapons`, then `ResetWorms` as before. The 12-column output is unchanged.
- Consumes: T4's grammar. The REAL `WeaponSelection` (`weapsel.cpp`), `LocalController` (`localController.cpp`), `Settings::FromToml`, `Rand` (`rand.hpp`), `SoundPlayer` (`mixer/player.hpp`).

Why: design §6.2, §6.5, Q6, Q9. The dumper drives the REAL constructor, `ProcessFrame` and `Finalize`. Only `LocalController`'s input plumbing is replicated, because the scenario holds sampled words (as Rust does), not key events. A changed bit is one `OnKey` event (`localController.cpp:58-80`), followed by a verbatim copy of the repeat loop (`:128-148`). The replica is proven by a second run through a real `LocalController` (`OnKey` + `Process`) from a fresh read of the same setup. This is the 4½b pattern: the replica slices the oracle's work, it is not the oracle. `sim_physics_dump` shares the driver, so the continuation goldens run the identical phase. With no `weapsel` line (every prior scenario), nothing changes; eight settings-path goldens plus two classic ones regenerate byte-identically.

- [ ] **Step 1: Write the driver header** — create `src/tools/oracle_dump/weapsel_drive.hpp`:

```cpp
// Step 4½c — the weapon-selection phase driver shared by oracle_dump_weapsel (weapsel_dump.cpp:
// the weapsel_* goldens) and oracle_dump_sim_physics's `settings` path (sim_physics_dump.cpp: the
// sim_slice4_5c_* continuation goldens). Design:
// docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md §6.2.
//
// The REAL WeaponSelection (weapsel.cpp) is constructed, stepped and finalized. Only
// LocalController's input plumbing is replicated, because a scenario holds SAMPLED per-frame
// words (as the Rust sampler does), not key events:
//   * a changed bit is one LocalController::OnKey event (localController.cpp:58-80): the clean
//     and the control bit follow it, then the Dig chord block;
//   * then a VERBATIM copy of the weapsel key-repeat loop (localController.cpp:128-148);
//   * then the REAL WeaponSelection::ProcessFrame (weapsel.cpp:219-350).
// oracle_dump_weapsel proves this replica against a real LocalController (its self-check).
#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <string>
#include <vector>

#include "game.hpp"
#include "math/rect.hpp"
#include "mixer/player.hpp"
#include "rand.hpp"
#include "viewport.hpp"
#include "weapsel.hpp"
#include "worm.hpp"

namespace weapsel_drive {

// frame -> the sampled 7-bit word per worm (a scenario's sparse `weapsel` lines).
using Script = std::map<int, std::array<uint32_t, 2>>;

constexpr int kRepeatBits = 7;         // localController.cpp:130
constexpr int kKeyRepeatInitial = 12;  // localController.hpp:44
constexpr int kKeyRepeatInterval = 3;  // localController.hpp:45
constexpr uint64_t kMaxDraws = 1000000;

[[noreturn]] inline void Fail(std::string const& what) {
  std::fprintf(stderr, "weapsel: %s\n", what.c_str());
  std::exit(1);
}

// Logs the sample id of every Play that reaches the mixer (SoundPlayer::Play already dropped a
// negative id, mixer/player.hpp:15-22).
struct RecordingSoundPlayer : SoundPlayer {
  bool IsPlaying(void* /*id*/) override { return false; }
  void Stop(void* /*id*/) override {}
  std::vector<int> played;

 protected:
  void PlayImpl(int sound, void* /*id*/, int /*loops*/) override { played.push_back(sound); }
};

// RNG steps from `before` to `after`. C++ Rand has no counter: step a copy of `before` until its
// mt19937 state equals `after`'s (rand.hpp:15), then require `last` to agree too.
inline uint64_t CountDraws(Rand const& before, Rand const& after) {
  Rand probe = before;
  uint64_t n = 0;
  while (probe.engine != after.engine) {
    probe();
    if (++n > kMaxDraws) {
      Fail("more than 10^6 RNG draws in one step");
    }
  }
  if (probe.last != after.last) {
    Fail("draw count: the engines agree but rand.last does not");
  }
  return n;
}

// The next raw value `r` would return, without advancing it.
inline uint32_t PeekNext(Rand const& r) {
  Rand probe = r;
  return probe();
}

class Driver {
 public:
  // Registers the LocalController viewports (localController.cpp:47-48) — WeaponSelection maps
  // menus[i] to game.viewports[i]->worm_idx (weapsel.cpp:44-47) — then runs the REAL
  // constructor (it draws game.rand).
  explicit Driver(Game& game) : game_(game) {
    if (game_.worms.size() != 2) {
      Fail("the phase needs exactly two worms");
    }
    viewports_[0] = std::make_unique<Viewport>(Rect(0, 0, 158, 158), 0);
    viewports_[1] = std::make_unique<Viewport>(Rect(160, 0, 158 + 160, 158), 1);
    for (auto const& vp : viewports_) {
      game_.AddViewport(vp.get());
    }
    ws_ = std::make_unique<WeaponSelection>(game_);
  }
  Driver(Driver const&) = delete;
  Driver& operator=(Driver const&) = delete;
  Driver(Driver&&) = delete;
  Driver& operator=(Driver&&) = delete;
  // The match runs with the viewports unregistered (the dumper's settings path has none).
  ~Driver() { game_.ClearViewports(); }

  // One LocalController::Process weapsel frame (localController.cpp:124-152) on sampled words.
  bool Frame(std::array<uint32_t, 2> const& words) {
    for (std::size_t wi = 0; wi < 2; ++wi) {
      Worm& worm = *game_.worms[wi];
      uint32_t const kNow = words[wi] & 0x7FU;
      uint32_t const kChanged = kNow ^ prev_[wi];
      for (int bit = 0; bit < kRepeatBits; ++bit) {
        if (((kChanged >> bit) & 1U) == 0) {
          continue;
        }
        // LocalController::OnKey for this key event (localController.cpp:58-80).
        bool const kDown = ((kNow >> bit) & 1U) != 0;
        auto const kControl = static_cast<Worm::Control>(bit);
        worm.clean_control_states.Set(kControl, kDown);
        worm.SetControlState(kControl, kDown);
        if (worm.clean_control_states[WormSettings::kDig]) {
          worm.Press(Worm::kLeft);
          worm.Press(Worm::kRight);
        } else {
          if (!worm.clean_control_states[Worm::kLeft]) {
            worm.Release(Worm::kLeft);
          }
          if (!worm.clean_control_states[Worm::kRight]) {
            worm.Release(Worm::kRight);
          }
        }
      }
      prev_[wi] = kNow;
    }
    // localController.cpp:128-148, verbatim.
    for (std::size_t wi = 0; wi < game_.worms.size(); ++wi) {
      Worm& worm = *game_.worms[wi];
      for (int bit = 0; bit < kRepeatBits; ++bit) {
        bool const kHeld = worm.clean_control_states[bit];
        if (kHeld) {
          if (!worm.control_states[bit]) {
            ++held_[wi][bit];
            if (held_[wi][bit] >= kKeyRepeatInitial &&
                (held_[wi][bit] - kKeyRepeatInitial) % kKeyRepeatInterval == 0) {
              worm.Press(static_cast<Worm::Control>(bit));
            }
          } else {
            held_[wi][bit] = 0;
          }
        } else {
          held_[wi][bit] = 0;
        }
      }
    }
    return ws_->ProcessFrame();
  }

  void Finalize() { ws_->Finalize(); }
  WeaponSelection const& Ws() const { return *ws_; }
  std::array<uint16_t, kRepeatBits> const& Held(std::size_t wi) const { return held_[wi]; }

 private:
  Game& game_;
  std::array<std::unique_ptr<Viewport>, 2> viewports_;
  std::unique_ptr<WeaponSelection> ws_;
  std::array<uint32_t, 2> prev_{};
  std::array<std::array<uint16_t, kRepeatBits>, 2> held_{};
};

// Runs the whole phase: the constructor, frames 0..end (an absent frame is {0, 0}; words are
// masked to 7 bits), then Finalize. `on_init(driver)` runs after the constructor and
// `on_frame(driver, frame, words, done)` after each ProcessFrame. The end-frame invariant (design
// §6.1): ProcessFrame must return true on the last `weapsel` frame and on no earlier one.
template <typename OnInit, typename OnFrame>
void Run(Game& game, Script const& script, OnInit const& on_init, OnFrame const& on_frame) {
  if (script.empty()) {
    Fail("no weapsel lines");
  }
  int const kEnd = script.rbegin()->first;
  Driver driver(game);
  on_init(driver);
  for (int frame = 0; frame <= kEnd; ++frame) {
    std::array<uint32_t, 2> words{0, 0};
    auto const kIt = script.find(frame);
    if (kIt != script.end()) {
      words = {kIt->second[0] & 0x7FU, kIt->second[1] & 0x7FU};
    }
    bool const kDone = driver.Frame(words);
    on_frame(driver, frame, words, kDone);
    if (kDone && frame != kEnd) {
      Fail("the phase ended on frame " + std::to_string(frame) + ", before the last weapsel line (" +
           std::to_string(kEnd) + ")");
    }
    if (!kDone && frame == kEnd) {
      Fail("the phase is not over on the last weapsel frame " + std::to_string(kEnd));
    }
  }
  driver.Finalize();
}

}  // namespace weapsel_drive
```

- [ ] **Step 2: Write the dumper** — create `src/tools/oracle_dump/weapsel_dump.cpp`:

```cpp
// Generates the C++ side of the Rust weapon-selection gate (Step 4½, slice 4½c;
// rust/oracle-tests/tests/weapsel_golden.rs). For one scenario it runs the REAL C++
// WeaponSelection (weapsel.cpp) — the constructor, one ProcessFrame per `weapsel` frame,
// Finalize — through the shared driver (weapsel_drive.hpp) and writes one line per step:
//   init <enabled> <p0> <p1> <draws> <last> <next>
//   f <frame> <in0> <in1> <p0> <p1> <ctl0> <ctl1> <held0> <held1> <sounds> <draws> <last> <next>
//     <done>
//   final <l0> <l1> <ctl0> <ctl1> <last> <next>
// pN = the five 1-based picks (WormSettings::weapons), ',' joined, then ':cursor:ready'; inN = the
// masked 7-bit word; ctlN = the worm's control word after the step (%02x); heldN = the seven
// key-repeat counters; sounds = sample ids in Play order or '-'; draws = RNG steps in the step;
// last/next = rand.last and the next raw value (%08x); lN = five 'weapon_id:ammo' pairs, ','
// joined, then ':current_weapon' (after Finalize).
//
// Usage (run from the repo root): oracle_dump_weapsel <scenario.txt> <out.txt>
// The scenario is a `settings` scenario (seed, level, ticks, settings, weapsel, input, comments;
// `ticks`/`input` are match ticks, oracle_dump_sim_physics's business). The setup is read with
// the REAL Settings::FromToml and the worms start exactly as sim_physics_dump's settings path
// starts them (health = ws.health, stats_x 0/218, no InitWeapons).
//
// Self-check (exits 1 without writing): the case is replayed through a REAL LocalController
// (OnKey + Process, localController.cpp) from a fresh read of the same setup and seed. It must
// agree with the driver on picks, cursors, ready flags, control words, repeat counters and the RNG
// after every frame, and on the loaded weapons after Finalize. Built via
// OPENLIERO_BUILD_ORACLE_DUMP (rust/oracle-tests/gen_weapsel_golden.sh). Not part of the default
// build.
#include <array>
#include <cinttypes>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <fstream>
#include <iterator>
#include <memory>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "common.hpp"
#include "controller/localController.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "gfx.hpp"
#include "io/stream.hpp"
#include "level.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "settings.hpp"
#include "stats_recorder.hpp"
#include "weapsel.hpp"
#include "weapsel_drive.hpp"
#include "worm.hpp"

namespace {

using weapsel_drive::Fail;

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

// The directory part of `path` ("." when there is none): the setup resolves relative to it.
std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

struct Scenario {
  uint32_t seed = 0;
  bool seed_given = false;
  std::string level;
  std::string settings_file;
  weapsel_drive::Script weapsel;
};

Scenario ParseScenario(std::string const& path) {
  std::istringstream in(Slurp(path));
  Scenario s;
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string key;
    if (!(ls >> key) || key[0] == '#') {
      continue;
    }
    if (key == "seed") {
      ls >> s.seed;
      s.seed_given = true;
    } else if (key == "level") {
      ls >> s.level;
    } else if (key == "settings") {
      ls >> s.settings_file;
    } else if (key == "weapsel") {
      int frame = -1;
      std::array<uint32_t, 2> words{0, 0};
      if (!(ls >> frame >> words[0] >> words[1]) || frame < 0) {
        Fail("bad weapsel line: " + line);
      }
      if (!s.weapsel.emplace(frame, words).second) {
        Fail("duplicate weapsel frame: " + line);
      }
    } else if (key != "ticks" && key != "input") {
      Fail("unsupported directive for oracle_dump_weapsel: " + key);
    }
  }
  if (!s.seed_given || s.level.empty() || s.settings_file.empty() || s.weapsel.empty()) {
    Fail("a weapsel scenario needs seed, level, settings and at least one weapsel line");
  }
  return s;
}

std::shared_ptr<Settings> LoadSettings(std::string const& path) {
  auto settings = std::make_shared<Settings>();
  try {
    settings->FromToml(Slurp(path));
  } catch (std::exception const& e) {
    Fail("settings " + path + ": " + e.what());
  }
  return settings;
}

std::string Hex(uint32_t v, int width) {
  std::array<char, 16> buf{};
  std::snprintf(buf.data(), buf.size(), "%0*x", width, v);
  return buf.data();
}

template <typename T, std::size_t N>
std::string Join(std::array<T, N> const& v) {
  std::string out;
  for (std::size_t i = 0; i < N; ++i) {
    if (i != 0) {
      out += ',';
    }
    out += std::to_string(v[i]);
  }
  return out;
}

std::array<uint32_t, 5> Picks(Game const& game, std::size_t i) {
  std::array<uint32_t, 5> p{};
  for (std::size_t j = 0; j < p.size(); ++j) {
    p[j] = game.worms[i]->settings->weapons[j];
  }
  return p;
}

std::string PlayerField(Game const& game, WeaponSelection const& ws, std::size_t i) {
  return Join(Picks(game, i)) + ":" + std::to_string(ws.menus[i].Selection()) + ":" +
         (ws.is_ready[i] ? "1" : "0");
}

std::array<std::pair<int, int>, 5> Loadout(Game const& game, std::size_t i) {
  std::array<std::pair<int, int>, 5> l{};
  for (std::size_t j = 0; j < l.size(); ++j) {
    WormWeapon const& ww = game.worms[i]->weapons[j];
    l[j] = {ww.type->id, ww.ammo};
  }
  return l;
}

// Everything the self-check compares after one step.
struct Snap {
  std::array<std::array<uint32_t, 5>, 2> picks{};
  std::array<int, 2> cursor{};
  std::array<bool, 2> ready{};
  std::array<uint32_t, 2> ctl{};
  std::array<std::array<uint16_t, 7>, 2> held{};
  std::array<std::array<std::pair<int, int>, 5>, 2> loadout{};
  Rand rand;
};

// The design §6.2 self-check: the same case through a REAL LocalController. Exits 1 on the first
// disagreement.
void SelfCheck(std::shared_ptr<Common> const& common, std::string const& cfg_path,
               Scenario const& scn, std::vector<Snap> const& want, Snap const& want_final) {
  gfx.sound_player = std::make_shared<NullSoundPlayer>();
  auto settings = LoadSettings(cfg_path);
  // Keep ChangeState(kStateGame) off the filesystem (localController.cpp:237). Not sim-reaching.
  settings->record_replays = false;
  LocalController lc(common, settings);
  lc.game.stats_recorder = std::make_shared<StatsRecorder>();  // StartGame's Reset: a no-op
  lc.game.rand.Seed(scn.seed);
  lc.Focus();  // kStateInitial -> ChangeState(kStateWeaponSelection): the REAL constructor
  int const kEnd = scn.weapsel.rbegin()->first;
  std::array<uint32_t, 2> prev{0, 0};
  for (int f = 0; f <= kEnd; ++f) {
    std::array<uint32_t, 2> words{0, 0};
    auto const kIt = scn.weapsel.find(f);
    if (kIt != scn.weapsel.end()) {
      words = {kIt->second[0] & 0x7FU, kIt->second[1] & 0x7FU};
    }
    for (std::size_t wi = 0; wi < 2; ++wi) {
      uint32_t const kChanged = words[wi] ^ prev[wi];
      for (int bit = 0; bit < weapsel_drive::kRepeatBits; ++bit) {
        if (((kChanged >> bit) & 1U) != 0) {
          lc.OnKey(static_cast<int>(settings->worm_settings[wi]->controls_ex[bit]),
                   ((words[wi] >> bit) & 1U) != 0);
        }
      }
      prev[wi] = words[wi];
    }
    lc.Process();
    bool const kLast = f == kEnd;
    std::string const kAt = "self-check frame " + std::to_string(f) + ": ";
    if (kLast != (lc.state == kStateGame)) {
      Fail(kAt + "LocalController left weapon selection on a different frame");
    }
    Snap const& w = kLast ? want_final : want[f];
    for (std::size_t i = 0; i < 2; ++i) {
      if (Picks(lc.game, i) != want[f].picks[i]) {
        Fail(kAt + "picks differ for player " + std::to_string(i));
      }
      if (lc.game.worms[i]->control_states.Pack() != w.ctl[i]) {
        Fail(kAt + "control word differs for worm " + std::to_string(i));
      }
      if (lc.worm_held_frames[i] != want[f].held[i]) {
        Fail(kAt + "repeat counters differ for worm " + std::to_string(i));
      }
      if (!kLast && (lc.ws->menus[i].Selection() != want[f].cursor[i] ||
                     static_cast<bool>(lc.ws->is_ready[i]) != want[f].ready[i])) {
        Fail(kAt + "cursor or ready differs for player " + std::to_string(i));
      }
      if (kLast && Loadout(lc.game, i) != want_final.loadout[i]) {
        Fail(kAt + "loaded weapons differ for worm " + std::to_string(i));
      }
    }
    if (lc.game.rand != w.rand) {
      Fail(kAt + "the RNG differs");
    }
  }
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 3) {
    std::fprintf(stderr, "usage: oracle_dump_weapsel <scenario.txt> <out.txt>\n");
    return 1;
  }
  std::string const kScnPath = argv[1];
  Scenario const kScn = ParseScenario(kScnPath);
  std::string const kCfgPath = DirOf(kScnPath) + "/" + kScn.settings_file;

  PrecomputeTables();
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");

  // The sim_physics_dump settings path's setup (sim_physics_dump.cpp: FromToml, the no-op
  // StatsRecorder, the StartGame pool, Seed, Level::load, then the two LocalController worms).
  auto settings = LoadSettings(kCfgPath);
  auto sound = std::make_shared<weapsel_drive::RecordingSoundPlayer>();
  Game game(common, settings, sound);
  game.stats_recorder = std::make_shared<StatsRecorder>();
  game.bobjects.Resize(settings->blood_particle_max);
  game.rand.Seed(kScn.seed);
  {
    std::string const kLevelPath = "data/TC/openliero/" + kScn.level;
    std::string const kBytes = Slurp(kLevelPath);
    std::vector<uint8_t> const kBuf(kBytes.begin(), kBytes.end());
    io::MemReader r(kBuf);
    if (!game.level.load(*common, *settings, r)) {
      Fail("Level::load failed for " + kLevelPath);
    }
  }
  for (int idx = 0; idx < 2; ++idx) {
    auto w = std::make_shared<Worm>();
    w->settings = settings->worm_settings[idx];
    w->health = w->settings->health;
    w->index = idx;
    w->stats_x = idx == 0 ? 0 : 218;
    game.AddWorm(w);
  }

  std::string out = "# oracle_dump_weapsel " + kScnPath +
                    " — the REAL C++ weapon-selection phase (Step 4½c design §6.3)\n"
                    "# init <enabled> <p0> <p1> <draws> <last> <next>\n"
                    "# f <frame> <in0> <in1> <p0> <p1> <ctl0> <ctl1> <held0> <held1> <sounds> "
                    "<draws> <last> <next> <done>\n"
                    "# final <l0> <l1> <ctl0> <ctl1> <last> <next>\n";
  std::vector<Snap> snaps;
  Rand before = game.rand;
  auto const kSnap = [&](weapsel_drive::Driver const& d) {
    Snap s;
    for (std::size_t i = 0; i < 2; ++i) {
      s.picks[i] = Picks(game, i);
      s.cursor[i] = d.Ws().menus[i].Selection();
      s.ready[i] = d.Ws().is_ready[i];
      s.ctl[i] = game.worms[i]->control_states.Pack();
      s.held[i] = d.Held(i);
    }
    s.rand = game.rand;
    return s;
  };
  weapsel_drive::Run(
      game, kScn.weapsel,
      [&](weapsel_drive::Driver const& d) {
        uint64_t const kDraws = weapsel_drive::CountDraws(before, game.rand);
        before = game.rand;
        out += "init " + std::to_string(d.Ws().enabled_weaps) + " " +
               PlayerField(game, d.Ws(), 0) + " " + PlayerField(game, d.Ws(), 1) + " " +
               std::to_string(kDraws) + " " + Hex(game.rand.last, 8) + " " +
               Hex(weapsel_drive::PeekNext(game.rand), 8) + "\n";
        sound->played.clear();
      },
      [&](weapsel_drive::Driver const& d, int frame, std::array<uint32_t, 2> const& words,
          bool done) {
        uint64_t const kDraws = weapsel_drive::CountDraws(before, game.rand);
        before = game.rand;
        std::string sounds;
        for (int const kId : sound->played) {
          sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
        }
        sound->played.clear();
        out += "f " + std::to_string(frame) + " " + std::to_string(words[0]) + " " +
               std::to_string(words[1]) + " " + PlayerField(game, d.Ws(), 0) + " " +
               PlayerField(game, d.Ws(), 1) + " " +
               Hex(game.worms[0]->control_states.Pack(), 2) + " " +
               Hex(game.worms[1]->control_states.Pack(), 2) + " " + Join(d.Held(0)) + " " +
               Join(d.Held(1)) + " " + (sounds.empty() ? "-" : sounds) + " " +
               std::to_string(kDraws) + " " + Hex(game.rand.last, 8) + " " +
               Hex(weapsel_drive::PeekNext(game.rand), 8) + " " + (done ? "1" : "0") + "\n";
        snaps.push_back(kSnap(d));
      });
  // Finalize ran inside Run (InitWeapons + ReleaseControls); it must draw nothing (design §3.5).
  if (weapsel_drive::CountDraws(before, game.rand) != 0) {
    Fail("Finalize drew the RNG");
  }
  Snap fin;
  std::string loadouts[2];
  for (std::size_t i = 0; i < 2; ++i) {
    fin.ctl[i] = game.worms[i]->control_states.Pack();
    fin.loadout[i] = Loadout(game, i);
    for (auto const& slot : fin.loadout[i]) {
      loadouts[i] += (loadouts[i].empty() ? "" : ",") + std::to_string(slot.first) + ":" +
                     std::to_string(slot.second);
    }
    loadouts[i] += ":" + std::to_string(game.worms[i]->current_weapon);
  }
  fin.rand = game.rand;
  out += "final " + loadouts[0] + " " + loadouts[1] + " " + Hex(fin.ctl[0], 2) + " " +
         Hex(fin.ctl[1], 2) + " " + Hex(game.rand.last, 8) + " " +
         Hex(weapsel_drive::PeekNext(game.rand), 8) + "\n";

  SelfCheck(common, kCfgPath, kScn, snaps, fin);

  std::ofstream f(argv[2], std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail(std::string("cannot write ") + argv[2]);
  }
  std::printf("oracle_dump_weapsel: %zu frames, LocalController self-check agreed\n",
              snaps.size());
  return 0;
}
```

Notes for the implementer:
- `Rand` has `operator==`/`!=` (`rand.hpp:59-62`); `std::mt19937` compares its whole state.
- `lc.ws`, `lc.state`, `lc.worm_held_frames`, `WeaponSelection::menus`/`is_ready`/`enabled_weaps` are public (all are `struct`s).
- `is_ready` is a `std::vector<bool>`, so the element is a proxy: hence the `static_cast<bool>`.
- The `std::string loadouts[2]` C array is allowed (`cppcoreguidelines-avoid-c-arrays` is off). Use `std::array<std::string, 2>` if tidy disagrees.
- If tidy flags the `int bit` → `std::size_t` index conversions in the replica loop, make `bit` a `std::size_t` (with a `static_cast<Worm::Control>`), and keep the logic verbatim.
- The golden's provenance line prints the scenario path exactly as passed. The gen script passes a repo-relative path, so the golden never contains a machine path.
- **Headless fallback (design §6.2, Q6):** if `LocalController` cannot run headless (a crash or abort in `Focus` or `Process`), delete `SelfCheck` and its call, and replace the header's "Self-check" paragraph with: `Self-check: none — LocalController does not run headless (<the failure>); the replica in weapsel_drive.hpp was reviewed line by line against localController.cpp:58-80, :128-148.` Report this in the done-report. Do not spend long trying to make it run.

- [ ] **Step 3: `sim_physics_dump` learns `weapsel`** — in `src/tools/oracle_dump/sim_physics_dump.cpp`:

(1) Header comment: after the `settings <file>` entry (the line that ends `Excludes worm/weapon/game_mode/max_bonuses/render*.)`) add

```cpp
//   weapsel <frame> <worm0_7bit> <worm1_7bit>  (Step 4½c; `settings` scenarios only: the
//                                       weapon-selection phase runs in place of InitWeapons —
//                                       the REAL WeaponSelection via weapsel_drive.hpp, frames
//                                       0..the last `weapsel` frame, which must be the frame it
//                                       ends on; sparse, absent => 0. The 12 columns are
//                                       unchanged; the tick-0 rng is the post-selection last.)
```

(2) Includes: add `#include "weapsel_drive.hpp"` between `#include "weapon.hpp"` and `#include "worm.hpp"`.

(3) `struct Scenario`: after `std::string settings_file;` add

```cpp
  // Step 4½c `weapsel <frame> <w0> <w1>` (design §6.1): the weapon-selection phase's sparse
  // per-frame input; `settings` scenarios only. Empty (every prior scenario) => InitWeapons as
  // before, byte-identical.
  weapsel_drive::Script weapsel;
```

(4) `ParseScenario`: add an arm after the `input` arm:

```cpp
    } else if (key == "weapsel") {
      // Step 4½c: exactly 3 numbers, like the Rust parser (a `#` token starts a comment); a
      // negative or duplicate frame is an error.
      int frame = -1;
      std::array<uint32_t, 2> in{0, 0};
      std::string extra;
      if (!(ls >> frame >> in[0] >> in[1]) || frame < 0 || (ls >> extra && extra[0] != '#')) {
        std::fprintf(stderr, "weapsel expects <frame> <worm0_7bit> <worm1_7bit>\n");
        std::exit(1);
      }
      if (!s.weapsel.emplace(frame, in).second) {
        std::fprintf(stderr, "duplicate weapsel frame %d\n", frame);
        std::exit(1);
      }
```

and, directly before `if (!s.settings_file.empty()) {` (the exclusion check after the loop), add

```cpp
  if (!s.weapsel.empty() && s.settings_file.empty()) {
    std::fprintf(stderr, "weapsel is oracle-only: it needs a settings directive\n");
    std::exit(1);
  }
```

(5) The settings path: inside `if (!scn.settings_file.empty()) {` (the block commented "Step 4½a-1: the C++ LocalController start state"), replace

```cpp
    for (auto const& w : game.worms) {
      w->InitWeapons(game);
    }
    game.ResetWorms();
  } else {
```

with

```cpp
    if (scn.weapsel.empty()) {
      for (auto const& w : game.worms) {
        w->InitWeapons(game);
      }
    } else {
      // Step 4½c: the weapon-selection phase runs where C++ runs it — between the
      // LocalController constructor and kStateGame (localController.cpp:224-235) — in place of
      // the bare InitWeapons: the REAL constructor (it draws game.rand), one REAL ProcessFrame
      // per `weapsel` frame and the REAL Finalize (InitWeapons + ReleaseControls), through the
      // driver shared with oracle_dump_weapsel. ResetWorms below then equals kStateGame's lives
      // + StartGame's pool (4½a design §7.2), and the tick-0 row carries the post-selection
      // rand.last. The viewports the phase registers are gone before the first tick.
      weapsel_drive::Run(
          game, scn.weapsel, [](weapsel_drive::Driver const& /*driver*/) {},
          [](weapsel_drive::Driver const& /*driver*/, int /*frame*/,
             std::array<uint32_t, 2> const& /*words*/, bool /*done*/) {});
    }
    game.ResetWorms();
  } else {
```

(The `game.ResetWorms();\n  } else {` text occurs once; the classic branch's `ResetWorms()` is followed by a blank line and a comment.)

(6) `CMakeLists.txt`: directly before the `endif()` that closes `if(OPENLIERO_BUILD_ORACLE_DUMP)` (after the `oracle_dump_settings` pair), add

```cmake
  add_executable(oracle_dump_weapsel src/tools/oracle_dump/weapsel_dump.cpp)
  target_link_libraries(oracle_dump_weapsel PRIVATE game)
```

- [ ] **Step 4: Build, then format**

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && cd /home/user/openliero && cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null && cmake --build build/linux-x64 --config Release --target oracle_dump_weapsel oracle_dump_sim_physics`
Expected: both targets link. (If the link fails on an unresolved symbol from `gfx.cpp`, read the error: `test_rollback_weapsel` links the same objects, see plan-time fact 11. Report and stop; do not stub symbols.)
Run (each file): `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/weapsel_drive.hpp` (likewise `weapsel_dump.cpp`, `sim_physics_dump.cpp`)
Expected: no output, exit 0. Otherwise apply with `-i` (not `--dry-run`), re-run the dry run, and rebuild.

- [ ] **Step 5: The regeneration proof (the edited dumper, every settings-path golden + two classic paths)**

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_sim_slice4_5a_golden.sh`
Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_sim_slice4_5c0_golden.sh`
Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_sim_slice6_fuzz1.sh`
Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_render_slice4d_live.sh`
Expected: each prints its `wrote …` and `gate …` lines.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: **no output**. The eight settings-path goldens (`sim_slice4_5a_*` ×4, `sim_slice4_5c0_*` ×4), the reduced-tail `sim_slice6_fuzz1` and the `render_live` golden regenerate byte-identically. Any diff stops the line (Global Constraints).

- [ ] **Step 6: Exercise the new paths on a scratch case** (not committed; it lives in the scratchpad)

Create `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws5/t5_setup.cfg`:

```toml
[player1]
controller = 0
health = 100
weapons = [ 0, 0, 5, 5, 0 ]

[player2]
controller = 1
health = 100
weapons = [ 3, 3, 3, 3, 3 ]

[settings]
lives = 99
selectBotWeapons = 1
```

and `…/ws5/t5_scenario.txt` (paths in the scenario resolve from the repo root; the setup resolves relative to the scenario):

```
seed 5
level Levels/render_stage.lev
ticks 3
settings t5_setup.cfg
weapsel 0 1 2
weapsel 2 8 8
weapsel 4 0 17
weapsel 6 16 1
```

Frame 0: P0 presses Up (0→6); P1, a PICK bot driven by column 1 (finding 2), presses Down (0→1). Frame 2: P0's Right on DONE is not read; P1's Right cycles slot 0 (3→4). Frame 4: P1 presses Up and Fire together (1→0, then RANDOMIZE on the moved cursor). Frame 6: P0 fires on DONE and is ready; P1 presses Up (0→6). P1 never fires on DONE, so this first run must fail.

Run: `cd /home/user/openliero && build/linux-x64/Release/oracle_dump_weapsel /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws5/t5_scenario.txt /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws5/t5.txt`
Expected: exit 1, `weapsel: the phase is not over on the last weapsel frame 6`, and no `t5.txt` written.

Now make it end: append `weapsel 8 0 16` (P1 fires on DONE on frame 8).
Run the same command. Expected: exit 0, `oracle_dump_weapsel: 9 frames, LocalController self-check agreed`. `t5.txt` has 4 comment lines, then:
- an `init 40 …` line with init draws 3 (P0's three zero picks);
- nine `f` lines, with `done` = 1 only on frame 8;
- frame 2's sounds = the TC's `MenuMoveDown` id (P1's Right);
- frame 4 draws ≥ 5 (RANDOMIZE);
- frame 6's sounds = `MenuSelect`,`MenuMoveDown` in that order (player 0 first);
- a `final` line whose two control words are `00`. If the self-check fails, read its frame and field: the replica or the header's OnKey order is wrong. Fix `weapsel_drive.hpp`; never loosen the check.
Run: `cd /home/user/openliero && build/linux-x64/Release/oracle_dump_sim_physics /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws5/t5_scenario.txt /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws5/t5_sim.txt`
Expected: 4 rows of 12 columns; row 0's `rng` column (field 3) equals `t5.txt`'s `final` `last`, and it is not `00000000`.
Negative checks (each exit 1 with the named message): a `weapsel` line in a copy of the scenario with the `settings` line removed (both dumpers); a duplicate `weapsel 2 …` line; `weapsel 9 0 0` appended after the ending frame (the dumper: `the phase ended on frame 8, before the last weapsel line (9)`).

- [ ] **Step 7: clang-tidy on the changed lines**

Run: `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 6abda0d`
Expected: exit 0, no diagnostics under `src/`. Fix what it reports (re-run Steps 4–5 if code changed). A `NOLINTNEXTLINE(<check>) — <reason>` is acceptable only where the repo already does so for the same check (`settings_dump.cpp:139` for `android-cloexec-fopen`).

- [ ] **Step 8: Commit**

```
git -C /home/user/openliero add src/tools/oracle_dump/weapsel_drive.hpp src/tools/oracle_dump/weapsel_dump.cpp src/tools/oracle_dump/sim_physics_dump.cpp CMakeLists.txt
git -C /home/user/openliero commit -m "oracle(4.5c): oracle_dump_weapsel (real WeaponSelection + LocalController self-check); sim_physics_dump settings path runs the phase" -m "The driver replicates only LocalController's input plumbing (OnKey + the 12/3 repeat) and runs the REAL constructor/ProcessFrame/Finalize. Without a weapsel line nothing changes: 8 settings-path goldens + sim_slice6_fuzz1 + render_slice4d_live regenerate byte-identically." -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

Reviewer (Opus): the replica's repeat loop is character-for-character `localController.cpp:128-148`; each changed bit is one `OnKey` including the Dig block; the constructor, `ProcessFrame` and `Finalize` are the real ones; draws are counted by engine equality; the end-frame invariant is enforced; the settings path is unchanged when `weapsel` is empty (the regeneration evidence); the self-check reads a fresh setup; C++ changes stay confined to the four files.

---
### Task 6: The 16-case corpus, the witness ledger, the C++ goldens  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/weapsel_common/mod.rs`
- Create: `rust/oracle-tests/examples/gen_slice4_5c.rs`
- Create: `rust/oracle-tests/gen_weapsel_golden.sh`, `rust/oracle-tests/gen_sim_slice4_5c_golden.sh`
- Generated (committed): `rust/oracle-tests/golden/weapsel_<case>_scenario.txt`, `weapsel_<case>_setup.cfg`, `weapsel_<case>.txt` (×16), and `sim_slice4_5c_match_humans.txt`, `sim_slice4_5c_match_bot.txt`

**Interfaces:**
- Produces (used by T7):
  - `weapsel_common`: the constants `TC_ROOT`, `GOLDEN`, `LEVEL`, `UP/DOWN/LEFT/RIGHT/FIRE`; `Script`; `Case` + `CASES: [Case; 16]` + `case(name)`; `setup_cfg(&Case, &Objects)`; `scenario_text(&Case, ledger)`; `inputs(seed, ticks)`; `load_objects()`, `load_level(rel)`, `weapon_index(o, name)`, `weapon_name_of_pick(o, pick)`; `read(rel)`, `read_scenario(name)`, `read_settings(&Scenario)`, `golden_file_lines(name)`; `Step`, `Run` (+ `summary()`), `drive(name, &Scenario, &Settings) -> (Run, SimState)` (post-`finalize`, before `enter_game`); `golden_lines(&Run) -> Vec<String>`; `witnesses(&[Run]) -> Vec<(&'static str, bool)>`; the 12-column `Row`, `parse_golden12`, `check12`.
  - the 50 golden files.
- Consumes: T3's `new_match`/`weapsel_config`, T1–T3's `WeaponSelection`, T4's `weapsel_end`/`weapsel_input`, T5's `oracle_dump_weapsel` + `oracle_dump_sim_physics`.

Why: design §6.3, §6.4, §6.5, §6.7. A generator writes the scenario files and the sidecars, so every file reproduces from its header. It builds the menu scripts from `tap`/`hold` helpers and drives every case through the REAL Rust path before writing. It refuses a corpus that breaks the end-frame invariant (§6.1) or misses a witness (§6.7). The C++ scripts then write the goldens and gate their shape. Cases 1–14 have `ticks 0`. Cases 15–16 continue 600 fuzz ticks through `sim_physics_dump` (§6.5). All cases use `Levels/render_stage.lev`. Pick numbers are 1-based `weap_order` positions, pinned by T0's test (1 BAZOOKA, 12 DART, 26 LASER, 31 MISSILE, 40 ZIMM). T7's `the_corpus_picks_and_tables_name_the_intended_weapons` re-checks every name below.

The corpus (design §6.4's table, made concrete):

| # | Case | Seed | Setup | Script (frame: action) | Pins |
|---|---|---|---|---|---|
| 1 | `humans_default` | 1 | all enabled; `[1;5]` both; humans; sbw 1 | Up, Down, Down, Left, Right, Right taps; Right held f12–30; Up, Up, Fire (P0); Down, Up, Up (P1) and Fire f44 | 0 ctor draws; cursor wrap both ways; cycle wrap 1↔40; tap vs hold (cycles f12, 24, 27, 30); P0 DONE first |
| 2 | `unset_picks` | 2 | all; P0 `[0,5,0,40,0]`, P1 `[3;5]` | Up both; Fire both f2 | 3 ctor draws; duplicates kept |
| 3/4 | `disabled_saved_s1`/`_s2` | 3/4 | 12 disabled (1s and 2s: BAZOOKA, BIG NUKE, BLASTER, CHAINGUN, DART, FAN, LASER, MINE, MISSILE, RIFLE, SHOTGUN, ZIMM); P0 `[1,12,5,40,26]`, P1 `[2,3,7,34,35]` | Down both; Right (P0) / Left (P1) held f2–92 (28 cycles = a full lap); Up, Up; Fire f98 | loop only for disabled picks; cycling skips 1s and 2s, wrapping inside a disabled run |
| 5 | `few_enabled` | 5 | only DART, HANDGUN, SHOTGUN | P0 RANDOMIZE; Down, Right, Left; finish | `enough = false`: duplicates in the loop and in RANDOMIZE |
| 6 | `five_enabled` | 6 | only CANNON, DART, GRENADE, HANDGUN, UZI | RANDOMIZE both; Up both; Fire f4 | the ≥ 5 boundary: permutations with long tails |
| 7 | `one_enabled` | 7 | only DART | Down, Left, Right (P0); finish | ~40 draws per slot; a cycle is a full lap |
| 8 | `randomize_held` | 8 | all | P0 Fire held f0–9; finish | a silent re-roll every frame |
| 9 | `same_frame` | 9 | all | Down+Fire on slot 5 (P0); Left+Right, Up+Down, Up+Fire (P1) | finding 6 |
| 10 | `repeat_edge` | 10 | all | P0 Left held f0–36, Down at f20; finish | cycles on 21, 33, 36 (finding 5) |
| 11 | `bot_random` | 11 | all; P1 bot `[5,6,7,8,9]`; sbw 0 | P0 Up, Fire f2 | RANDOM bot: 5 draws, ready at once |
| 12 | `bot_pick` | 12 | all; P1 bot `[1;5]`; sbw 1 | P1 Down, Right; Up both; P0 Fire + P1 Up; P1 Fire f8 | PICK bot driven by worm 1's keys (finding 2) |
| 13 | `bot_keep` | 13 | BAZOOKA 2, LASER 2, MISSILE 1; P0 `[2,3,4,5,6]`; P1 bot `[0,26,5,6,7]`; sbw 2 | P0 Up, Fire f2 | KEEP: ready at once, the zero and the disabled pick still roll |
| 14 | `bots_only_7` | 14 | all; bots `[0,2,3,4,5]` / `[9;5]`; sbw 7 | `weapsel 0 0 0` | ≥ 3 behaves as KEEP; done on frame 0 |
| 15 | `match_humans` | 15 | all; humans; + 600 ticks (input seed 1015) | P0 RANDOMIZE, Down, Right; P1 Down, Down, Left; both to DONE, f12 | continuation |
| 16 | `match_bot` | 16 | all; P1 RANDOM bot; sbw 0; + 600 ticks (input seed 1016) | P0 RANDOMIZE, Up, Fire f4 | continuation; different picks per worm |

- [ ] **Step 1: Create the shared module** `rust/oracle-tests/tests/weapsel_common/mod.rs`:

```rust
//! Step 4½c — the weapon-selection golden helpers, shared by `examples/gen_slice4_5c.rs` (via
//! `#[path]`), `weapsel_golden.rs`, `sim_slice4_5c_continuation_golden.rs` and
//! `weapsel_handoff.rs`: the 16-case corpus (design §6.4), the lean setup sidecar and the
//! scenario writer, the menu scripts, the Rust driver that yields a golden's lines (§6.3), the
//! witness guard (§6.7) and the 12-column settings-path harness (§6.5). One copy, so the
//! generator's corpus and the milestone's non-vacuity checks cannot drift apart.

#![allow(dead_code)] // each includer uses a subset.

use std::path::Path;

use assets::level::LevelData;
use assets::object::Objects;
use assets::tc::TcConfig;
use scenario::build::{new_match, weapsel_config};
use scenario::settings::{MatchConfig, Settings};
use scenario::settings_toml::settings_from_toml;
use scenario::Scenario;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};
use sim::weapsel::{
    weap_order, PlayerSel, WeaponSelection, WeapselConfig, WeapselPlayer, WEAPON_COUNT,
};
use sim_core::rng::Rand;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// Every case's level (design §6.4).
pub const LEVEL: &str = "Levels/render_stage.lev";
/// The lean sidecar's sim settings (4½c-0's set). Lives 99: a continuation never ends.
pub const LIVES: i32 = 99;
pub const HEALTH: i32 = 100;
pub const LOADING_TIME: i32 = 20;
pub const BLOOD: i32 = 100;
pub const MAX_BONUSES: i32 = 4;
pub const BLOOD_PARTICLE_MAX: i32 = 700;

pub const UP: u32 = 1;
pub const DOWN: u32 = 2;
pub const LEFT: u32 = 4;
pub const RIGHT: u32 = 8;
pub const FIRE: u32 = 16;

/// A menu script: one `[worm0, worm1]` word pair per phase frame, frame 0 first.
#[derive(Clone, Debug, Default)]
pub struct Script(pub Vec<[u32; 2]>);

impl Script {
    pub fn new() -> Script {
        Script(Vec::new())
    }
    /// `n` frames holding `a` on worm 0 and `b` on worm 1.
    pub fn both(mut self, a: u32, b: u32, n: u32) -> Script {
        for _ in 0..n {
            self.0.push([a, b]);
        }
        self
    }
    pub fn p0(self, bits: u32, n: u32) -> Script {
        self.both(bits, 0, n)
    }
    pub fn p1(self, bits: u32, n: u32) -> Script {
        self.both(0, bits, n)
    }
    pub fn idle(self, n: u32) -> Script {
        self.both(0, 0, n)
    }
    /// A clean tap: held one frame, released the next.
    pub fn tap0(self, bits: u32) -> Script {
        self.p0(bits, 1).idle(1)
    }
    pub fn tap1(self, bits: u32) -> Script {
        self.p1(bits, 1).idle(1)
    }
    pub fn tap_both(self, a: u32, b: u32) -> Script {
        self.both(a, b, 1).idle(1)
    }
}

pub struct Case {
    pub name: &'static str,
    pub pins: &'static str,
    pub seed: u32,
    /// Match ticks after the phase: 0, except the two continuation cases.
    pub ticks: u32,
    /// Seed of a continuation's per-tick input stream.
    pub input_seed: u32,
    pub select_bot_weapons: u32,
    pub table: fn(&Objects) -> [u32; WEAPON_COUNT],
    pub players: [WeapselPlayer; 2],
    pub script: fn() -> Script,
}

const fn human(weapons: [u32; 5]) -> WeapselPlayer {
    WeapselPlayer { weapons, controller: 0 }
}
const fn bot(weapons: [u32; 5]) -> WeapselPlayer {
    WeapselPlayer { weapons, controller: 1 }
}
/// `Settings()`'s saved picks (`worm.hpp:92`): five copies of the first weapon by name.
const DEFAULT: [u32; 5] = [1; 5];
const MATCH_TICKS: u32 = 600;

pub fn load_objects() -> Objects {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap()
}

pub fn load_level(rel: &str) -> LevelData {
    assets::level::load(&std::fs::read(format!("{TC_ROOT}/{rel}")).unwrap()).unwrap()
}

/// Index into `objects.weapons` (== the `weap_table` index).
pub fn weapon_index(o: &Objects, name: &str) -> usize {
    o.weapons
        .iter()
        .position(|w| w.name == name)
        .unwrap_or_else(|| panic!("no weapon {name:?}"))
}

/// The weapon a 1-based pick names.
pub fn weapon_name_of_pick(o: &Objects, pick: u32) -> String {
    o.weapons[weap_order(&o.weapons)[pick as usize - 1]].name.clone()
}

fn all_enabled(_: &Objects) -> [u32; WEAPON_COUNT] {
    [0; WEAPON_COUNT]
}

/// `names` disabled with their value (1 bonus only, 2 banned); the rest enabled.
fn disabled(o: &Objects, names: &[(&str, u32)]) -> [u32; WEAPON_COUNT] {
    let mut t = [0u32; WEAPON_COUNT];
    for &(n, v) in names {
        t[weapon_index(o, n)] = v;
    }
    t
}

/// Only `names` enabled; the rest alternate bonus-only (1) and banned (2) by weapon index.
fn only(o: &Objects, names: &[&str]) -> [u32; WEAPON_COUNT] {
    let mut t = [0u32; WEAPON_COUNT];
    for (i, v) in t.iter_mut().enumerate() {
        *v = 1 + (i as u32 % 2);
    }
    for n in names {
        t[weapon_index(o, n)] = 0;
    }
    t
}

fn t_twelve(o: &Objects) -> [u32; WEAPON_COUNT] {
    disabled(
        o,
        &[
            ("BAZOOKA", 2),
            ("BIG NUKE", 1),
            ("BLASTER", 2),
            ("CHAINGUN", 1),
            ("DART", 2),
            ("FAN", 1),
            ("LASER", 2),
            ("MINE", 1),
            ("MISSILE", 2),
            ("RIFLE", 1),
            ("SHOTGUN", 2),
            ("ZIMM", 1),
        ],
    )
}
fn t_three(o: &Objects) -> [u32; WEAPON_COUNT] {
    only(o, &["DART", "HANDGUN", "SHOTGUN"])
}
fn t_five(o: &Objects) -> [u32; WEAPON_COUNT] {
    only(o, &["CANNON", "DART", "GRENADE", "HANDGUN", "UZI"])
}
fn t_one(o: &Objects) -> [u32; WEAPON_COUNT] {
    only(o, &["DART"])
}
fn t_keep(o: &Objects) -> [u32; WEAPON_COUNT] {
    disabled(o, &[("BAZOOKA", 2), ("LASER", 2), ("MISSILE", 1)])
}

fn s_humans_default() -> Script {
    Script::new()
        .tap0(UP) // f0: cursor 0 -> 6, wrapping up
        .tap0(DOWN) // f2: 6 -> 0, wrapping down
        .tap0(DOWN) // f4: 0 -> 1 (slot 0)
        .tap0(LEFT) // f6: pick 1 -> 40, wrapping left
        .tap0(RIGHT) // f8: 40 -> 1, wrapping right
        .tap0(RIGHT) // f10: a tap cycles once: 1 -> 2
        .p0(RIGHT, 19) // f12-30: cycles on 12 (the press), 24, 27, 30 (held 12, 15, 18)
        .idle(1) // f31
        .tap0(UP) // f32: 1 -> 0
        .tap0(UP) // f34: 0 -> 6
        .tap0(FIRE) // f36: P0 ready
        .tap1(DOWN) // f38: P1 moves while P0 is ready
        .tap1(UP) // f40: 1 -> 0
        .tap1(UP) // f42: 0 -> 6
        .p1(FIRE, 1) // f44: P1 ready: the phase ends
}
fn s_both_done() -> Script {
    Script::new().tap_both(UP, UP).both(FIRE, FIRE, 1) // f0: both 0 -> 6; f2: done
}
fn s_full_laps() -> Script {
    Script::new()
        .tap_both(DOWN, DOWN) // f0: both to slot 0
        .both(RIGHT, LEFT, 91) // f2-92: 28 cycles each: a full lap of the 28 enabled
        .idle(1) // f93
        .tap_both(UP, UP) // f94: 1 -> 0
        .tap_both(UP, UP) // f96: 0 -> 6
        .both(FIRE, FIRE, 1) // f98: done
}
fn s_few() -> Script {
    Script::new()
        .tap0(FIRE) // f0: P0 RANDOMIZE: five picks from three weapons, duplicates kept
        .tap0(DOWN) // f2: slot 0
        .tap0(RIGHT) // f4: across the disabled run
        .tap0(LEFT) // f6: and back
        .tap_both(UP, UP) // f8: P0 1 -> 0; P1 0 -> 6
        .tap_both(UP, FIRE) // f10: P0 0 -> 6; P1 ready
        .p0(FIRE, 1) // f12: done
}
fn s_five() -> Script {
    Script::new()
        .tap_both(FIRE, FIRE) // f0: both RANDOMIZE: permutations of the five
        .tap_both(UP, UP) // f2: 0 -> 6
        .both(FIRE, FIRE, 1) // f4: done
}
fn s_one() -> Script {
    Script::new()
        .tap0(DOWN) // f0: slot 0
        .tap0(LEFT) // f2: a full lap back to DART
        .tap0(RIGHT) // f4: and the other way
        .tap_both(UP, UP) // f6: P0 1 -> 0; P1 0 -> 6
        .tap_both(UP, FIRE) // f8: P0 0 -> 6; P1 ready
        .p0(FIRE, 1) // f10: done
}
fn s_held() -> Script {
    Script::new()
        .p0(FIRE, 10) // f0-9: a silent re-roll every frame (finding 7)
        .idle(1) // f10
        .tap_both(UP, UP) // f11: 0 -> 6
        .both(FIRE, FIRE, 1) // f13: done
}
fn s_same() -> Script {
    Script::new()
        .tap_both(UP, DOWN) // f0: P0 0 -> 6; P1 0 -> 1
        .tap_both(UP, LEFT | RIGHT) // f2: P0 6 -> 5; P1 Left then Right: net zero, two sounds
        .tap_both(0, UP | DOWN) // f4: P1 Up then Down: net zero, two sounds
        .tap_both(DOWN | FIRE, UP | FIRE) // f6: P0 5 -> 6 -> DONE; P1 1 -> 0 -> RANDOMIZE
        .tap1(UP) // f8: P1 0 -> 6 while P0 is ready
        .p1(FIRE, 1) // f10: done
}
fn s_edge() -> Script {
    Script::new()
        .p0(LEFT, 20) // f0-19: Left held on RANDOMIZE: never read, its counter stays 0
        .p0(LEFT | DOWN, 1) // f20: Down runs after the Left check: 0 -> 1
        .p0(LEFT, 16) // f21-36: cycles on 21, 33, 36 (Rollback would give 21, 24, 27)
        .idle(1) // f37
        .tap0(UP) // f38: 1 -> 0
        .tap_both(UP, UP) // f40: both 0 -> 6
        .both(FIRE, FIRE, 1) // f42: done
}
fn s_p0_done() -> Script {
    Script::new().tap0(UP).p0(FIRE, 1) // player 2 is ready at once; f2: done
}
fn s_bot_pick() -> Script {
    Script::new()
        .tap1(DOWN) // f0: the PICK bot's menu, driven by worm 1's keys (finding 2)
        .tap1(RIGHT) // f2
        .tap_both(UP, UP) // f4: P0 0 -> 6; P1 1 -> 0
        .tap_both(FIRE, UP) // f6: P0 ready; P1 0 -> 6
        .p1(FIRE, 1) // f8: done
}
fn s_none() -> Script {
    Script::new().idle(1) // both ready at construction: done on frame 0 (`weapsel 0 0 0`)
}
fn s_match_humans() -> Script {
    Script::new()
        .tap_both(FIRE, DOWN) // f0: P0 RANDOMIZE; P1 0 -> 1
        .tap_both(DOWN, DOWN) // f2: P0 0 -> 1; P1 1 -> 2
        .tap_both(RIGHT, LEFT) // f4: P0 slot 0 + 1; P1 slot 1: 1 -> 40
        .tap_both(UP, UP) // f6: P0 1 -> 0; P1 2 -> 1
        .tap_both(UP, UP) // f8: P0 0 -> 6; P1 1 -> 0
        .tap_both(FIRE, UP) // f10: P0 ready; P1 0 -> 6
        .p1(FIRE, 1) // f12: done
}
fn s_match_bot() -> Script {
    Script::new().tap0(FIRE).tap0(UP).p0(FIRE, 1) // f0 RANDOMIZE, f2 0 -> 6, f4 done
}

pub const CASES: [Case; 16] = [
    Case { name: "humans_default", pins: "zero constructor draws; cursor wrap both ways; cycle wrap 1<->40; a tap vs Right held (cycles at held 12, 15, 18); P0 DONE first, then P1", seed: 1, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_humans_default },
    Case { name: "unset_picks", pins: "constructor draws only for zero picks; enabled duplicates are kept (finding 1)", seed: 2, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human([0, 5, 0, 40, 0]), human([3; 5])], script: s_both_done },
    Case { name: "disabled_saved_s1", pins: "the loop runs only for disabled saved picks, uniqueness inside it; cycling skips bonus-only and banned weapons over a full lap", seed: 3, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_twelve, players: [human([1, 12, 5, 40, 26]), human([2, 3, 7, 34, 35])], script: s_full_laps },
    Case { name: "disabled_saved_s2", pins: "as disabled_saved_s1, second seed", seed: 4, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_twelve, players: [human([1, 12, 5, 40, 26]), human([2, 3, 7, 34, 35])], script: s_full_laps },
    Case { name: "few_enabled", pins: "3 enabled, enough = false: duplicates in the constructor loop and in RANDOMIZE", seed: 5, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_three, players: [human(DEFAULT), human(DEFAULT)], script: s_few },
    Case { name: "five_enabled", pins: "the >= 5 boundary: loops and RANDOMIZE yield permutations (long tails)", seed: 6, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_five, players: [human(DEFAULT), human(DEFAULT)], script: s_five },
    Case { name: "one_enabled", pins: "1 enabled: ~40 draws per slot; a cycle is a full lap back to the same weapon", seed: 7, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: t_one, players: [human(DEFAULT), human(DEFAULT)], script: s_one },
    Case { name: "randomize_held", pins: "Fire held 10 frames on RANDOMIZE: a silent re-roll every frame (finding 7)", seed: 8, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_held },
    Case { name: "same_frame", pins: "in-frame order (finding 6): Down+Fire on slot 5 readies; Left+Right and Up+Down cancel with two sounds; Up+Fire randomizes", seed: 9, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_same },
    Case { name: "repeat_edge", pins: "LocalController repeat: Left held unread on RANDOMIZE, then cycles on 21, 33, 36 (finding 5)", seed: 10, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_edge },
    Case { name: "bot_random", pins: "a RANDOM bot (select_bot_weapons 0) draws all five over its saved picks and is ready at once", seed: 11, ticks: 0, input_seed: 0, select_bot_weapons: 0, table: all_enabled, players: [human(DEFAULT), bot([5, 6, 7, 8, 9])], script: s_p0_done },
    Case { name: "bot_pick", pins: "a PICK bot (select_bot_weapons 1) is not ready and is driven by worm 1's keys (finding 2)", seed: 12, ticks: 0, input_seed: 0, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), bot(DEFAULT)], script: s_bot_pick },
    Case { name: "bot_keep", pins: "a KEEP bot (select_bot_weapons 2) readies at once with its saved picks; its zero and its disabled pick still roll", seed: 13, ticks: 0, input_seed: 0, select_bot_weapons: 2, table: t_keep, players: [human([2, 3, 4, 5, 6]), bot([0, 26, 5, 6, 7])], script: s_p0_done },
    Case { name: "bots_only_7", pins: "select_bot_weapons 7 behaves as KEEP; both bots: done on frame 0", seed: 14, ticks: 0, input_seed: 0, select_bot_weapons: 7, table: all_enabled, players: [bot([0, 2, 3, 4, 5]), bot([9; 5])], script: s_none },
    Case { name: "match_humans", pins: "continuation: RANDOMIZE + cycling + DONE, then 600 fuzz ticks", seed: 15, ticks: MATCH_TICKS, input_seed: 1015, select_bot_weapons: 1, table: all_enabled, players: [human(DEFAULT), human(DEFAULT)], script: s_match_humans },
    Case { name: "match_bot", pins: "continuation: a RANDOM bot + a human RANDOMIZE (different picks per worm), then 600 fuzz ticks", seed: 16, ticks: MATCH_TICKS, input_seed: 1016, select_bot_weapons: 0, table: all_enabled, players: [human(DEFAULT), bot(DEFAULT)], script: s_match_bot },
];

pub fn case(name: &str) -> &'static Case {
    CASES
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

fn arr(v: &[u32]) -> String {
    let items: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("[ {} ]", items.join(", "))
}

/// The LEAN setup sidecar: the 4½c-0 keys + `controller` and `selectBotWeapons`. Missing keys
/// keep their defaults in both readers (`toml_archive.hpp:177-186`).
pub fn setup_cfg(c: &Case, o: &Objects) -> String {
    let mut out = String::new();
    for (header, p) in [("player1", &c.players[0]), ("player2", &c.players[1])] {
        out.push_str(&format!(
            "[{header}]\ncontroller = {}\nhealth = {HEALTH}\nweapons = {}\n\n",
            p.controller,
            arr(&p.weapons)
        ));
    }
    out.push_str("[settings]\n");
    out.push_str(&format!("blood = {BLOOD}\nbloodParticleMax = {BLOOD_PARTICLE_MAX}\n"));
    out.push_str(&format!("gameMode = 0\nlevelFile = '{LEVEL}'\nlives = {LIVES}\n"));
    out.push_str(&format!("loadChange = true\nloadingTime = {LOADING_TIME}\n"));
    out.push_str(&format!("maxBonuses = {MAX_BONUSES}\nrandomLevel = false\n"));
    out.push_str(&format!("selectBotWeapons = {}\n", c.select_bot_weapons));
    out.push_str("shadow = true\ntimeToLose = 600\nversion = 6\n");
    out.push_str(&format!("weapTable = {}\n", arr(&(c.table)(o))));
    out
}

/// Per tick `Rand(input_seed).next_u32() & 0x7f`, worm 0 then worm 1 (the 4½a-1 fuzz stream).
pub fn inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    (0..ticks)
        .map(|_| [r.next_u32() & 0x7f, r.next_u32() & 0x7f])
        .collect()
}

/// The committed scenario text. `weapsel` lines are sparse, but the last frame is always
/// written (`weapsel 0 0 0` for `bots_only_7`).
pub fn scenario_text(c: &Case, ledger: &str) -> String {
    let frames = (c.script)().0;
    let end = frames.len() - 1;
    let also = if c.ticks > 0 {
        format!(" and oracle_dump_sim_physics (golden/sim_slice4_5c_{}.txt)", c.name)
    } else {
        String::new()
    };
    let mut out = format!(
        "# Step 4½ slice 4½c — weapon-selection case `{name}` (design §6.4): {pins}.\n\
         # Read by oracle_dump_weapsel (golden/weapsel_{name}.txt){also} and by the\n\
         # Rust milestone (weapsel_golden.rs). Written by\n\
         #   cargo run -p oracle-tests --example gen_slice4_5c -- write <golden dir>\n\
         # LEDGER (Rust, driven): {ledger}\n\
         # weapsel <frame> <worm0_7bit> <worm1_7bit>: Up=1 Down=2 Left=4 Right=8 Fire=16; an\n\
         # absent frame is 0; the last line is the frame the phase ends on (design §6.1).\n\
         seed {seed}\nlevel {LEVEL}\nticks {ticks}\nsettings weapsel_{name}_setup.cfg\n",
        name = c.name,
        pins = c.pins,
        seed = c.seed,
        ticks = c.ticks,
    );
    for (f, w) in frames.iter().enumerate() {
        if w[0] != 0 || w[1] != 0 || f == end {
            out.push_str(&format!("weapsel {f} {} {}\n", w[0], w[1]));
        }
    }
    if c.ticks > 0 {
        out.push_str(&format!(
            "# input <tick> <w0> <w1>: per tick Rand({}).next_u32() & 0x7f, worm 0 then worm 1.\n",
            c.input_seed
        ));
        for (t, w) in inputs(c.input_seed, c.ticks).iter().enumerate() {
            if w[0] != 0 || w[1] != 0 {
                out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
            }
        }
    }
    out
}

pub fn read(rel: &str) -> String {
    let path = Path::new(GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

pub fn read_scenario(name: &str) -> Scenario {
    Scenario::parse(&read(&format!("weapsel_{name}_scenario.txt")))
        .unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"))
}

pub fn read_settings(s: &Scenario) -> Settings {
    let rel = s.settings.as_ref().expect("a settings scenario");
    settings_from_toml(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"))
}

/// The non-comment lines of the committed C++ golden `weapsel_<name>.txt`.
pub fn golden_file_lines(name: &str) -> Vec<String> {
    read(&format!("weapsel_{name}.txt"))
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// One phase frame, as a golden `f` line records it.
#[derive(Clone, Debug)]
pub struct Step {
    pub frame: u32,
    pub input: [u32; 2],
    pub before: [PlayerSel; 2],
    pub after: [PlayerSel; 2],
    pub ctl: [u32; 2],
    pub held: [[u16; 7]; 2],
    pub sounds: Vec<i32>,
    pub draws: u64,
    pub last: u32,
    pub next: u32,
    pub done: bool,
}

/// A whole case driven through the REAL Rust path.
#[derive(Clone, Debug)]
pub struct Run {
    pub name: String,
    pub cfg: WeapselConfig,
    pub order: Vec<usize>,
    pub enabled: i32,
    pub init: [PlayerSel; 2],
    pub init_draws: u64,
    pub init_last: u32,
    pub init_next: u32,
    pub steps: Vec<Step>,
    /// After `finalize`: per worm the five `(weapon id, ammo)`.
    pub loadout: [[(i32, i32); 5]; 2],
    pub current_weapon: [i32; 2],
    pub final_ctl: [u32; 2],
    pub final_last: u32,
    pub final_next: u32,
}

impl Run {
    pub fn summary(&self) -> String {
        let draws: u64 = self.steps.iter().map(|s| s.draws).sum();
        format!(
            "init draws {}, {} frames, {} draws in frames, final rng {:08x}",
            self.init_draws,
            self.steps.len(),
            draws,
            self.final_last
        )
    }
}

fn next_of(st: &SimState) -> u32 {
    st.rand.clone().next_u32()
}

fn players(ws: &WeaponSelection) -> [PlayerSel; 2] {
    [*ws.player(0), *ws.player(1)]
}

/// Drive `scenario`'s phase through the REAL Rust path — `new_match`, `weapsel_config`,
/// `WeaponSelection::new` / `process_frame` / `finalize` — recording everything a golden line
/// holds. Enforces the end-frame invariant (design §6.1). Returns the run and the state after
/// `finalize` (before `enter_game`).
pub fn drive(name: &str, scenario: &Scenario, settings: &Settings) -> (Run, SimState) {
    let level = load_level(&scenario.level);
    let cfg = MatchConfig {
        settings: settings.clone(),
        seed: scenario.seed,
    };
    let mut st = new_match(Path::new(TC_ROOT), &cfg, &level)
        .unwrap_or_else(|e| panic!("{name}: builds: {e}"))
        .state;
    let wcfg = weapsel_config(settings);
    let d0 = st.rand.draws();
    let mut ws = WeaponSelection::new(&mut st, &wcfg).unwrap_or_else(|e| panic!("{name}: {e}"));
    let enabled = ws.enabled_weaps();
    let init = players(&ws);
    let (init_draws, init_last, init_next) = (st.rand.draws() - d0, st.rand.last(), next_of(&st));
    let end = scenario
        .weapsel_end()
        .unwrap_or_else(|| panic!("{name}: no weapsel line"));
    let mut steps = Vec::new();
    for f in 0..=end {
        let input = [0, 1].map(|i| ControlState::unpack(scenario.weapsel_input(f, i)).pack());
        let before = players(&ws);
        let d = st.rand.draws();
        let done = ws.process_frame(
            &mut st,
            &[ControlState::unpack(input[0]), ControlState::unpack(input[1])],
        );
        steps.push(Step {
            frame: f,
            input,
            before,
            after: players(&ws),
            ctl: [st.worms[0].control_states.pack(), st.worms[1].control_states.pack()],
            held: [ws.held(0), ws.held(1)],
            sounds: ws.menu_sounds().to_vec(),
            draws: st.rand.draws() - d,
            last: st.rand.last(),
            next: next_of(&st),
            done,
        });
        assert_eq!(
            done,
            f == end,
            "{name}: the phase must end exactly on the last weapsel frame {end} (frame {f})"
        );
    }
    let order = weap_order(&st.weapons);
    let d = st.rand.draws();
    ws.finalize(&mut st);
    assert_eq!(st.rand.draws(), d, "{name}: finalize draws nothing");
    let loadout = [0, 1].map(|i| {
        st.worms[i]
            .weapons
            .map(|w| (w.ty.expect("finalize loads every slot"), w.ammo))
    });
    let run = Run {
        name: name.to_string(),
        cfg: wcfg,
        order,
        enabled,
        init,
        init_draws,
        init_last,
        init_next,
        steps,
        loadout,
        current_weapon: [st.worms[0].current_weapon, st.worms[1].current_weapon],
        final_ctl: [st.worms[0].control_states.pack(), st.worms[1].control_states.pack()],
        final_last: st.rand.last(),
        final_next: next_of(&st),
    };
    (run, st)
}

fn pfield(p: &PlayerSel) -> String {
    let picks: Vec<String> = p.picks.iter().map(u32::to_string).collect();
    format!("{}:{}:{}", picks.join(","), p.cursor, p.ready as u8)
}

fn join<T: ToString>(v: &[T]) -> String {
    v.iter().map(T::to_string).collect::<Vec<_>>().join(",")
}

fn lfield(l: &[(i32, i32); 5], current_weapon: i32) -> String {
    let slots: Vec<String> = l.iter().map(|(w, a)| format!("{w}:{a}")).collect();
    format!("{}:{current_weapon}", slots.join(","))
}

/// The golden lines (design §6.3 + the T5 `final` refinement) a run implies — the exact text
/// `oracle_dump_weapsel` prints.
pub fn golden_lines(r: &Run) -> Vec<String> {
    let mut out = vec![format!(
        "init {} {} {} {} {:08x} {:08x}",
        r.enabled,
        pfield(&r.init[0]),
        pfield(&r.init[1]),
        r.init_draws,
        r.init_last,
        r.init_next
    )];
    for s in &r.steps {
        let sounds = if s.sounds.is_empty() {
            "-".to_string()
        } else {
            join(&s.sounds)
        };
        out.push(format!(
            "f {} {} {} {} {} {:02x} {:02x} {} {} {} {} {:08x} {:08x} {}",
            s.frame,
            s.input[0],
            s.input[1],
            pfield(&s.after[0]),
            pfield(&s.after[1]),
            s.ctl[0],
            s.ctl[1],
            join(&s.held[0]),
            join(&s.held[1]),
            sounds,
            s.draws,
            s.last,
            s.next,
            s.done as u8
        ));
    }
    out.push(format!(
        "final {} {} {:02x} {:02x} {:08x} {:08x}",
        lfield(&r.loadout[0], r.current_weapon[0]),
        lfield(&r.loadout[1], r.current_weapon[1]),
        r.final_ctl[0],
        r.final_ctl[1],
        r.final_last,
        r.final_next
    ));
    out
}

fn weapon(r: &Run, pick: u32) -> usize {
    r.order[pick as usize - 1]
}

fn has_dup(r: &Run, picks: &[u32; 5]) -> bool {
    let mut seen = [false; WEAPON_COUNT];
    picks
        .iter()
        .any(|&p| std::mem::replace(&mut seen[weapon(r, p)], true))
}

/// Player `i` ran RANDOMIZE this frame: not ready, the (moved) cursor on RANDOMIZE, Fire set.
fn randomized(s: &Step, i: usize) -> bool {
    !s.before[i].ready && s.after[i].cursor == 0 && s.ctl[i] & FIRE != 0
}

/// The single cycling direction player `i` pressed (not both, no Fire), if any.
fn dir(s: &Step, i: usize) -> Option<u32> {
    let w = s.input[i];
    match (w & LEFT != 0, w & RIGHT != 0, w & FIRE != 0) {
        (true, false, false) => Some(LEFT),
        (false, true, false) => Some(RIGHT),
        _ => None,
    }
}

/// `(before, after)` of the slot player `i`'s frame-start cursor names, if its pick changed.
fn slot_change(s: &Step, i: usize) -> Option<(u32, u32)> {
    let c = s.before[i].cursor;
    if s.before[i].ready || !(1..=5).contains(&c) {
        return None;
    }
    let k = c as usize - 1;
    let (a, b) = (s.before[i].picks[k], s.after[i].picks[k]);
    (a != b).then_some((a, b))
}

/// The reach witnesses (design §6.7), derived from the driven Rust state over the whole corpus.
pub fn witnesses(runs: &[Run]) -> Vec<(&'static str, bool)> {
    let steps = || runs.iter().flat_map(|r| r.steps.iter().map(move |s| (r, s)));
    let ctor_loop = runs.iter().any(|r| {
        // Every forced slot (a saved disabled pick) draws >= 1 in the loop; every optional slot
        // (zero or RANDOM) draws 1, plus >= 1 if disabled. More than forced + 2 * optional
        // draws means some loop iterated at least twice.
        let (mut forced, mut optional) = (0u64, 0u64);
        for p in &r.cfg.players {
            let random = p.controller != 0 && r.cfg.select_bot_weapons == 0;
            for &pick in &p.weapons {
                if pick == 0 || random {
                    optional += 1;
                } else if r.cfg.weap_table[weapon(r, pick)] != 0 {
                    forced += 1;
                }
            }
        }
        r.init_draws > forced + 2 * optional
    });
    let cycled = |want: fn(u32, u32, u32) -> bool| {
        steps().any(|(_, s)| {
            (0..2).any(|i| match (dir(s, i), slot_change(s, i)) {
                (Some(d), Some((a, b))) => want(d, a, b),
                _ => false,
            })
        })
    };
    let naive = |d: u32, a: u32| -> u32 {
        if d == LEFT {
            if a == 1 { WEAPON_COUNT as u32 } else { a - 1 }
        } else if a == WEAPON_COUNT as u32 {
            1
        } else {
            a + 1
        }
    };
    let across_disabled = steps().any(|(_, s)| {
        (0..2).any(|i| match (dir(s, i), slot_change(s, i)) {
            (Some(d), Some((a, b))) => b != naive(d, a),
            _ => false,
        })
    });
    let wrap_left = cycled(|d, a, b| d == LEFT && b > a);
    let wrap_right = cycled(|d, a, b| d == RIGHT && b < a);
    let cursor_wrap = |pressed: u32, other: u32, from: u8, to: u8| {
        steps().any(|(_, s)| {
            (0..2).any(|i| {
                !s.before[i].ready
                    && s.input[i] & pressed != 0
                    && s.input[i] & other == 0
                    && s.before[i].cursor == from
                    && s.after[i].cursor == to
            })
        })
    };
    let repeat_at = |held: u16| {
        steps().any(|(_, s)| {
            (0..2).any(|i| {
                dir(s, i).is_some()
                    && slot_change(s, i).is_some()
                    && (s.held[i][2] == held || s.held[i][3] == held)
            })
        })
    };
    let edge = runs.iter().filter(|r| r.name == "repeat_edge").any(|r| {
        let frames: Vec<u32> = r
            .steps
            .iter()
            .filter(|s| slot_change(s, 0).is_some())
            .map(|s| s.frame)
            .collect();
        frames == [21, 33, 36]
    });
    let sbw = |f: fn(u32) -> bool| runs.iter().any(|r| f(r.cfg.select_bot_weapons));
    vec![
        ("a constructor loop with >= 2 iterations", ctor_loop),
        (
            "a constructor-kept duplicate (>= 5 enabled)",
            runs.iter()
                .any(|r| r.enabled >= 5 && r.init.iter().any(|p| has_dup(r, &p.picks))),
        ),
        (
            "a RANDOMIZE that rejected a duplicate (40 enabled, one player, > 5 draws)",
            steps().any(|(r, s)| {
                r.enabled == 40
                    && (0..2).filter(|&i| randomized(s, i)).count() == 1
                    && s.draws > 5
            }),
        ),
        (
            "a RANDOMIZE that kept a duplicate",
            steps().any(|(r, s)| (0..2).any(|i| randomized(s, i) && has_dup(r, &s.after[i].picks))),
        ),
        ("a cycle across a disabled weapon", across_disabled),
        ("a cycle wrapping left (up through 1)", wrap_left),
        ("a cycle wrapping right (down through 40)", wrap_right),
        ("a cursor wrap up (0 -> 6)", cursor_wrap(UP, DOWN, 0, 6)),
        ("a cursor wrap down (6 -> 0)", cursor_wrap(DOWN, UP, 6, 0)),
        ("a repeat at held-frame 12", repeat_at(12)),
        ("a repeat at held-frame 15", repeat_at(15)),
        ("the repeat_edge timing 21, 33, 36", edge),
        ("select_bot_weapons 0 (RANDOM)", sbw(|v| v == 0)),
        ("select_bot_weapons 1 (PICK)", sbw(|v| v == 1)),
        ("select_bot_weapons 2 (KEEP)", sbw(|v| v == 2)),
        ("select_bot_weapons >= 3 (as KEEP)", sbw(|v| v >= 3)),
        (
            "one player ready while the other still moves",
            steps().any(|(_, s)| {
                (0..2).any(|a| {
                    let b = 1 - a;
                    s.before[a].ready && !s.before[b].ready && s.after[b] != s.before[b]
                })
            }),
        ),
        ("done on frame 0", runs.iter().any(|r| r.steps.len() == 1)),
        (
            "a RANDOMIZE held over consecutive frames",
            runs.iter().any(|r| {
                r.steps
                    .windows(2)
                    .any(|w| (0..2).any(|i| randomized(&w[0], i) && randomized(&w[1], i)))
            }),
        ),
    ]
}

/// One row of a 12-column settings-path golden: the 10 hashes (master, rng, level, worm0,
/// worm1, bob, bon, sob, nob, wob) + `IsGameOver`.
pub struct Row {
    pub tick: u32,
    pub hashes: [u32; 10],
    pub game_over: u32,
}

/// Parse a 12-column settings-path golden; row `k` must carry tick `k`
/// (`sim_slice4_5a_settings_golden.rs`'s reader).
pub fn parse_golden12(text: &str) -> Vec<Row> {
    let rows: Vec<Row> = text
        .lines()
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
        .collect();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(r.tick, k as u32, "golden row {k} carries tick {}", r.tick);
    }
    rows
}

/// Components first, master last, so a divergence localises; then column 12.
pub fn check12(state: &SimState, row: &Row) {
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
    let names = [
        "master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob",
    ];
    for i in (1..10).chain(0..1) {
        assert_eq!(
            got[i], row.hashes[i],
            "tick {}: {}: got {:08x} want {:08x}",
            row.tick, names[i], got[i], row.hashes[i]
        );
    }
    assert_eq!(is_game_over(state) as u32, row.game_over, "tick {}: IsGameOver", row.tick);
}
```

The rules match design §6.7's list one for one. The only probabilistic one is "a RANDOMIZE that rejected a duplicate"; `randomize_held` is the case meant to reach it.

- [ ] **Step 2: Create the generator** `rust/oracle-tests/examples/gen_slice4_5c.rs`:

```rust
//! Step 4½c T6 — the weapon-selection corpus writer (design §6.4). A dev tool, not a test; not
//! run in CI. Run in a DEBUG build (overflow checks).
//!
//!   cargo run -p oracle-tests --example gen_slice4_5c -- check
//!   cargo run -p oracle-tests --example gen_slice4_5c -- write <golden dir>
//!
//! Both drive every case through the REAL Rust path (new_match + sim::weapsel) and refuse a
//! corpus that breaks the end-frame invariant (§6.1) or misses a witness (§6.7); `write` then
//! writes the 16 `weapsel_<case>_scenario.txt` + `_setup.cfg` pairs. The goldens are C++'s:
//! gen_weapsel_golden.sh + gen_sim_slice4_5c_golden.sh.

#[path = "../tests/weapsel_common/mod.rs"]
mod wc;

use scenario::settings_toml::settings_from_toml;
use scenario::Scenario;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let o = wc::load_objects();
    let mut runs = Vec::new();
    let mut files = Vec::new();
    for case in &wc::CASES {
        let setup = wc::setup_cfg(case, &o);
        let settings = settings_from_toml(&setup).expect("the generated setup parses");
        let scenario =
            Scenario::parse(&wc::scenario_text(case, "")).expect("the generated scenario parses");
        let (run, _) = wc::drive(case.name, &scenario, &settings);
        println!("{:<18} {}", case.name, run.summary());
        files.push((case.name, wc::scenario_text(case, &run.summary()), setup));
        runs.push(run);
    }
    let mut missing = Vec::new();
    for (name, ok) in wc::witnesses(&runs) {
        println!("witness {name:<72} {}", if ok { "reached" } else { "MISSING" });
        if !ok {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "the corpus misses {missing:?}: change the seed of the case meant to reach it (design §6.7)"
    );
    match args.first().map(String::as_str) {
        Some("check") => {}
        Some("write") => {
            let dir = std::path::Path::new(args.get(1).expect("write <golden dir>"));
            for (name, scenario, setup) in &files {
                std::fs::write(dir.join(format!("weapsel_{name}_scenario.txt")), scenario)
                    .expect("write scenario");
                std::fs::write(dir.join(format!("weapsel_{name}_setup.cfg")), setup)
                    .expect("write setup");
            }
            println!("wrote {} cases to {}", files.len(), dir.display());
        }
        _ => panic!("usage: gen_slice4_5c check | write <golden dir>"),
    }
}
```

- [ ] **Step 3: Check the corpus in Rust**

Run: `rustfmt --edition 2021 /home/user/openliero/rust/oracle-tests/tests/weapsel_common/mod.rs /home/user/openliero/rust/oracle-tests/examples/gen_slice4_5c.rs`
Run: `cd /home/user/openliero/rust && cargo run -p oracle-tests --example gen_slice4_5c -- check`
Expected: 16 ledger lines (`bots_only_7`: `init draws 1…, 1 frames`; `humans_default`: `init draws 0, 45 frames`; `unset_picks`: `init draws 3`), then 19 `witness … reached` lines, and exit 0.
- A panic `the phase must end exactly on the last weapsel frame` means a script or the port is wrong. Recount the script's frames against its comments before touching `sim`.
- A MISSING witness: only `a RANDOMIZE that rejected a duplicate` is probabilistic. Its case is `randomize_held` (40 enabled, ten re-rolls: ~93% per seed). Change that case's `seed` (8 → 108 → 208 …) and re-run. Every other witness holds by construction; if one is missing, the script or the rule is wrong, so re-read it.

- [ ] **Step 4: Write the scenario files**

Run: `cd /home/user/openliero/rust && cargo run -p oracle-tests --example gen_slice4_5c -- write /home/user/openliero/rust/oracle-tests/golden`
Expected: `wrote 16 cases to …`. Read `golden/weapsel_bots_only_7_scenario.txt` (Read tool). Its last directive line is `weapsel 0 0 0`, and the sidecar line reads `settings weapsel_bots_only_7_setup.cfg`. Read `golden/weapsel_repeat_edge_setup.cfg`: two player tables, `selectBotWeapons = 1`, and a 40-zero `weapTable`.

- [ ] **Step 5: The C++ gen scripts** — create `rust/oracle-tests/gen_weapsel_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/weapsel_<case>.txt — the C++ weapon-selection phase, line for line (Step 4½
# slice 4½c, design §6.2-§6.4). oracle_dump_weapsel runs the REAL WeaponSelection over every
# committed weapsel_<case>_scenario.txt (+ its _setup.cfg, read by the REAL Settings::FromToml)
# and self-checks its input replica against a real LocalController. The scenario/setup inputs
# are written by `cargo run -p oracle-tests --example gen_slice4_5c -- write <golden dir>` and
# never touched here. Needs the full C++ build (links the `game` target), so this is a
# LOCAL/MANUAL step — NOT run in the lightweight rust.yml CI. Override PRESET for other
# platforms (e.g. linux-x64). cd's to ROOT, so any cwd works.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_weapsel
n=0
for scn in rust/oracle-tests/golden/weapsel_*_scenario.txt; do
  c="$(basename "$scn" _scenario.txt)"
  out="rust/oracle-tests/golden/${c}.txt"
  "build/$PRESET/Release/oracle_dump_weapsel" "$scn" "$out"
  # C++-SIDE GATE (design §6.3): exactly one 7-field `init` first; 15-field `f` lines for
  # frames 0..end in order, done=1 only on the last `weapsel` frame (§6.1); one 7-field `final`.
  end=$(awk '$1 == "weapsel" { f = $2 } END { print f }' "$scn")
  awk -v c="$c" -v end="$end" '
    BEGIN { frames = 0; finals = 0; lines = 0 }
    /^#/ { next }
    { lines++ }
    lines == 1 { if ($1 != "init" || NF != 7) { printf "FAIL %s: line 1 is not a 7-field init\n", c; exit 1 } next }
    $1 == "f" {
      if (NF != 15) { printf "FAIL %s: f line with %d fields\n", c, NF; exit 1 }
      if ($2 != frames) { printf "FAIL %s: frame %s out of order (want %d)\n", c, $2, frames; exit 1 }
      if ($15 != ($2 == end ? "1" : "0")) { printf "FAIL %s: done=%s on frame %s\n", c, $15, $2; exit 1 }
      frames++; next
    }
    $1 == "final" { if (NF != 7 || finals++) { printf "FAIL %s: bad final line\n", c; exit 1 } next }
    { printf "FAIL %s: unexpected line: %s\n", c, $0; exit 1 }
    END {
      if (!finals || frames != end + 1) { printf "FAIL %s: %d frames (want %d), final=%d\n", c, frames, end + 1, finals; exit 1 }
      printf "  gate %s: init + %d frames (done on %d) + final\n", c, frames, end
    }
  ' "$out"
  echo "wrote $out"
  n=$((n + 1))
done
test "$n" -eq 16 || { echo "FAIL: $n weapsel scenarios (want 16)"; exit 1; }
```

and `rust/oracle-tests/gen_sim_slice4_5c_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5c_match_{humans,bot}.txt — 12 columns (the 11
# oracle_dump_sim_physics hashes + Game::IsGameOver) for the Step-4½ slice-4½c CONTINUATION
# matches (design §6.5): the `settings` path runs the REAL weapon-selection phase (the scenario's
# `weapsel` lines, via weapsel_drive.hpp) in place of InitWeapons, then 600 fuzz ticks. Needs the
# full C++ build, so this is a LOCAL/MANUAL step. Override PRESET (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
for v in humans bot; do
  scn="rust/oracle-tests/golden/weapsel_match_${v}_scenario.txt"
  out="rust/oracle-tests/golden/sim_slice4_5c_match_${v}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" "$scn" "$out"
  echo "wrote $out"
  ticks=$(awk '$1 == "ticks" { print $2 }' "$scn")
  # C++-SIDE GATE: 12 columns, IsGameOver constant 0 (lives 99), ticks+1 rows, and a NON-ZERO
  # tick-0 rng (column 3): the selection's draws reached the match (design §6.5).
  awk -v v="$v" -v ticks="$ticks" '
    NF != 12 { printf "FAIL %s: %d columns at tick %s\n", v, NF, $1; bad = 1; exit 1 }
    $12 != "0" { printf "FAIL %s: IsGameOver=%s at tick %s (lives 99)\n", v, $12, $1; bad = 1; exit 1 }
    NR == 1 && $3 == "00000000" { printf "FAIL %s: tick-0 rng is 0 (no selection draw)\n", v; bad = 1; exit 1 }
    END {
      if (bad) { exit 1 }
      if (NR != ticks + 1) { printf "FAIL %s: %d rows (want %d)\n", v, NR, ticks + 1; exit 1 }
      printf "  gate %s: %d rows, 12 columns, IsGameOver 0, tick-0 rng non-zero\n", v, NR
    }
  ' "$out"
done
```

Run: `chmod +x /home/user/openliero/rust/oracle-tests/gen_weapsel_golden.sh /home/user/openliero/rust/oracle-tests/gen_sim_slice4_5c_golden.sh`

- [ ] **Step 6: Generate the goldens**

Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_weapsel_golden.sh`
Expected: for each of the 16 cases, `oracle_dump_weapsel: N frames, LocalController self-check agreed`, a `gate` line and a `wrote` line. No `FAIL`.
Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_sim_slice4_5c_golden.sh`
Expected: two `gate` lines, `601 rows, 12 columns, IsGameOver 0, tick-0 rng non-zero`.

If a self-check fails on a corpus case (T5's scratch case passed), the replica diverges on an input shape only the corpus reaches, most likely a same-frame multi-bit change or a held bit across a ready flip. Fix `weapsel_drive.hpp` (a T5 file, under T5's rules), re-run T5 Step 5's regeneration proof, then re-run this step.

- [ ] **Step 7: Sanity-read the C++ output** (Read tool, not a test)
- `golden/weapsel_humans_default.txt`: the `init` line has draws `0`. The `f 24` line's `held0` has `12` in position 4 (Right, 0-based index 3), and its P0 picks changed from `f 23`.
- `golden/weapsel_repeat_edge.txt`: P0's slot-0 pick changes exactly on `f 21`, `f 33`, `f 36`.
- `golden/weapsel_bots_only_7.txt`: exactly one `f` line, `f 0 0 0 … 1`.
- `golden/weapsel_same_frame.txt`: on `f 2` the sounds are `MenuMoveDown,MenuMoveUp,MenuMoveDown` (the TC ids): P0's Up, then P1's Left and Right. P1's pick is unchanged.
- `golden/sim_slice4_5c_match_bot.txt`: row 0's column 3 equals `weapsel_match_bot.txt`'s `final` `last`.

- [ ] **Step 8: The golden audit**

Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden`
Expected: exactly 50 `??` lines (`weapsel_*` ×48, `sim_slice4_5c_match_{humans,bot}.txt`), and no ` M` line.
Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS (nothing reads the new files yet).

- [ ] **Step 9: Commit**

```
git -C /home/user/openliero add rust/oracle-tests/tests/weapsel_common rust/oracle-tests/examples/gen_slice4_5c.rs rust/oracle-tests/gen_weapsel_golden.sh rust/oracle-tests/gen_sim_slice4_5c_golden.sh rust/oracle-tests/golden/weapsel_* rust/oracle-tests/golden/sim_slice4_5c_match_humans.txt rust/oracle-tests/golden/sim_slice4_5c_match_bot.txt
git -C /home/user/openliero commit -m "oracle(4.5c): the 16-case weapsel corpus + witness ledger; C++ weapsel goldens (self-checked) + 2 continuation goldens" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---

### Task 7: MILESTONE — the 16 goldens line for line, the continuations bit-exact, handoff equality  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/weapsel_golden.rs`
- Create: `rust/oracle-tests/tests/sim_slice4_5c_continuation_golden.rs`
- Create: `rust/oracle-tests/tests/weapsel_handoff.rs`

**Interfaces:**
- Consumes: T6's `weapsel_common` and the committed goldens; T3's `build_match`/`new_match`/`enter_game`/`weapsel_config`.

Why: design §1 done-when 1–3, §6.3, §6.5, §6.6, §6.7. Every golden line (init, every frame, final) must agree with C++ on picks, cursors, ready flags, control words, repeat counters, sounds, draws per step and `rand.last` + next. The corpus must reach every branch (the witness guard). The two continuations must be bit-exact for 600 ticks, with the tick-0 RNG coming from a real selection. Handoff equality localises a failure to the seam: path one is new match → selection → finalize → enter game, and path two is `build_match` with the final picks. They agree on every field except the RNG, and on the master hash once path two's RNG is advanced by the recorded draws. It holds on both continuation cases and on a randomized run over configurations. Everything is re-derived from the committed files through the real readers; nothing is regenerated in memory.

- [ ] **Step 1: Create** `rust/oracle-tests/tests/weapsel_golden.rs`:

```rust
//! Step 4½c T7 — MILESTONE (design §1 done-when 1, §6.3, §6.7): the sixteen weapon-selection
//! goldens line for line vs C++. Each committed `weapsel_<case>_scenario.txt` goes through the
//! real readers (`Scenario::parse`, `settings_from_toml` on its sidecar), `new_match`,
//! `weapsel_config` and `sim::weapsel`; `weapsel_common::golden_lines` prints what
//! `oracle_dump_weapsel` prints. The witness guard then proves the corpus reaches every branch.

mod weapsel_common;

use weapsel_common as wc;

fn run_case(c: &wc::Case) -> wc::Run {
    let scenario = wc::read_scenario(c.name);
    let settings = wc::read_settings(&scenario);
    wc::drive(c.name, &scenario, &settings).0
}

fn compare(name: &str, got: &[String], want: &[String]) {
    for (k, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{name}: golden line {k} (0 = init) differs");
    }
    assert_eq!(got.len(), want.len(), "{name}: line count");
}

#[test]
fn every_weapsel_golden_matches_line_for_line_and_the_corpus_reaches_every_branch() {
    let mut runs = Vec::new();
    for c in &wc::CASES {
        let run = run_case(c);
        compare(c.name, &wc::golden_lines(&run), &wc::golden_file_lines(c.name));
        runs.push(run);
    }
    let missing: Vec<&str> = wc::witnesses(&runs)
        .into_iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| name)
        .collect();
    assert!(missing.is_empty(), "witnesses not reached: {missing:?}");
}

#[test]
#[should_panic(expected = "golden line")]
fn a_perturbed_golden_line_fails() {
    // Non-vacuity of `compare`: one changed character must fail.
    let c = wc::case("same_frame");
    let run = run_case(c);
    let mut want = wc::golden_file_lines(c.name);
    want[3] = want[3].replacen(' ', "  ", 1);
    compare(c.name, &wc::golden_lines(&run), &want);
}

#[test]
fn the_committed_corpus_is_the_sixteen_cases() {
    let mut on_disk: Vec<String> = std::fs::read_dir(wc::GOLDEN)
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().into_string().unwrap();
            n.strip_prefix("weapsel_")
                .and_then(|r| r.strip_suffix("_scenario.txt"))
                .map(String::from)
        })
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = wc::CASES.iter().map(|c| c.name.to_string()).collect();
    want.sort();
    assert_eq!(on_disk, want);
}

#[test]
fn the_corpus_picks_and_tables_name_the_intended_weapons() {
    let o = wc::load_objects();
    let names = |c: &str, i: usize| {
        wc::case(c).players[i].weapons.map(|p| {
            if p == 0 {
                "-".to_string()
            } else {
                wc::weapon_name_of_pick(&o, p)
            }
        })
    };
    assert_eq!(
        names("disabled_saved_s1", 0),
        ["BAZOOKA", "DART", "BOUNCY LARPA", "ZIMM", "LASER"]
    );
    assert_eq!(
        names("disabled_saved_s1", 1),
        ["BIG NUKE", "BLASTER", "CANNON", "RIFLE", "SHOTGUN"]
    );
    assert_eq!(
        names("bot_keep", 1),
        ["-", "LASER", "BOUNCY LARPA", "BOUNCY MINE", "CANNON"]
    );
    let t = (wc::case("disabled_saved_s1").table)(&o);
    let off = |name: &str| t[wc::weapon_index(&o, name)] != 0;
    assert_eq!(
        ["BAZOOKA", "DART", "BOUNCY LARPA", "ZIMM", "LASER", "CANNON"].map(off),
        [true, true, false, true, true, false],
        "saved picks: four disabled + one enabled per player"
    );
    assert_eq!(t.iter().filter(|&&v| v != 0).count(), 12);
    assert!(t.contains(&1) && t.contains(&2), "a mix of bonus-only and banned");
    let enabled = |c: &str| (wc::case(c).table)(&o).iter().filter(|&&v| v == 0).count();
    assert_eq!(
        [enabled("few_enabled"), enabled("five_enabled"), enabled("one_enabled"), enabled("bot_keep")],
        [3, 5, 1, 37]
    );
}
```

- [ ] **Step 2: Create** `rust/oracle-tests/tests/sim_slice4_5c_continuation_golden.rs`:

```rust
//! Step 4½c T7 — the two CONTINUATION goldens (design §1 done-when 2, §6.5): a real weapon
//! selection (`weapsel` lines) in front of 600 fuzz ticks, 12 columns bit-exact vs
//! oracle_dump_sim_physics's settings path. The tick-0 rng is the post-selection `rand.last`.

mod weapsel_common;

use scenario::build::enter_game;
use scenario::settings::MatchConfig;
use sim::state::ControlState;
use weapsel_common as wc;

fn continuation(name: &str) {
    let case = wc::case(name);
    let scenario = wc::read_scenario(name);
    let settings = wc::read_settings(&scenario);
    let (run, mut st) = wc::drive(name, &scenario, &settings);
    enter_game(
        &mut st,
        &MatchConfig {
            settings,
            seed: scenario.seed,
        },
    );
    assert_ne!(
        st.worms[0].weapons.map(|w| w.ty),
        st.worms[1].weapons.map(|w| w.ty),
        "{name}: per-worm picks"
    );
    let rows = wc::parse_golden12(&wc::read(&format!("sim_slice4_5c_{name}.txt")));
    assert_eq!(rows.len() as u32, case.ticks + 1, "{name}: rows 0..=ticks");
    assert_ne!(rows[0].hashes[1], 0, "{name}: a selection draw reached tick 0");
    assert_eq!(rows[0].hashes[1], run.final_last, "{name}: tick-0 rng = post-selection last");
    wc::check12(&st, &rows[0]);
    for k in 1..=scenario.ticks {
        st.process_frame(&[
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ]);
        wc::check12(&st, &rows[k as usize]);
    }
}

#[test]
fn match_humans_continues_bit_exact() {
    continuation("match_humans");
}

#[test]
fn match_bot_continues_bit_exact() {
    continuation("match_bot");
}
```

- [ ] **Step 3: Create** `rust/oracle-tests/tests/weapsel_handoff.rs`:

```rust
//! Step 4½c T7 — handoff equality (design §1 done-when 3, §6.6). Path one: `new_match` → weapon
//! selection → `finalize` → `enter_game`. Path two: `build_match` with the saved picks replaced
//! by the final picks, same seed. They must agree on every hash component but `rng`, on the whole
//! `WormState` (weapons, ammo, lives, `current_weapon`, control words, …) and the pool capacity,
//! and — once path two's RNG is advanced by the phase's draws — on the master hash. On the two
//! continuation cases and on a randomized run of configurations.

mod weapsel_common;

use std::path::Path;

use scenario::build::{build_match, enter_game, new_match, weapsel_config};
use scenario::settings::{MatchConfig, Settings};
use sim::hash::{hash_components, hash_game_state, ComponentHashes};
use sim::state::{ControlState, SimState};
use sim::weapsel::{WeaponSelection, DONE_ITEM, WEAPON_COUNT};
use sim_core::rng::Rand;
use weapsel_common as wc;

fn assert_handoff(label: &str, cfg: &MatchConfig, picks: [[u32; 5]; 2], one: &SimState) {
    let level = wc::load_level(wc::LEVEL);
    let mut cfg2 = cfg.clone();
    cfg2.settings.worm_settings[0].weapons = picks[0];
    cfg2.settings.worm_settings[1].weapons = picks[1];
    let mut two = build_match(Path::new(wc::TC_ROOT), &cfg2, &level)
        .unwrap_or_else(|e| panic!("{label}: {e}"))
        .state;
    let (a, b) = (hash_components(one), hash_components(&two));
    assert_eq!(
        ComponentHashes { rng: 0, ..a },
        ComponentHashes { rng: 0, ..b },
        "{label}: every component but rng"
    );
    assert_eq!(one.worms, two.worms, "{label}: the worms");
    assert_eq!(one.bobjects.capacity(), two.bobjects.capacity(), "{label}: pool");
    for _ in 0..one.rand.draws() {
        two.rand.next_u32();
    }
    assert_eq!(
        hash_game_state(one),
        hash_game_state(&two),
        "{label}: master hash after advancing path two by {} draws",
        one.rand.draws()
    );
}

#[test]
fn the_two_continuation_cases_hand_off_like_build_match() {
    for name in ["match_humans", "match_bot"] {
        let scenario = wc::read_scenario(name);
        let settings = wc::read_settings(&scenario);
        let (run, mut one) = wc::drive(name, &scenario, &settings);
        let cfg = MatchConfig {
            settings,
            seed: scenario.seed,
        };
        enter_game(&mut one, &cfg);
        let last = run.steps.last().expect("at least one frame");
        assert_handoff(name, &cfg, [last.after[0].picks, last.after[1].picks], &one);
        assert!(one.rand.draws() > 0, "{name}: non-vacuous: the phase drew");
    }
}

/// `prefix` frames of random words, then every player not yet ready walks Down to DONE with
/// clean taps and fires. Returns when the phase ends.
fn drive_to_done(ws: &mut WeaponSelection, st: &mut SimState, rng: &mut Rand, prefix: u32) {
    let step = |ws: &mut WeaponSelection, st: &mut SimState, w: [u32; 2]| {
        ws.process_frame(st, &[ControlState::unpack(w[0]), ControlState::unpack(w[1])])
    };
    for _ in 0..prefix {
        let w = [rng.next_u32() & 0x7f, rng.next_u32() & 0x7f];
        if step(ws, st, w) {
            return;
        }
    }
    for _ in 0..64 {
        if step(ws, st, [0, 0]) {
            return;
        }
        let w = [0, 1].map(|i| {
            let p = ws.player(i);
            if p.ready {
                0
            } else if p.cursor == DONE_ITEM {
                wc::FIRE
            } else {
                wc::DOWN
            }
        });
        if step(ws, st, w) {
            return;
        }
    }
    panic!("the finishing walk did not end the phase");
}

#[test]
fn a_randomized_run_of_configurations_hands_off_like_build_match() {
    let level = wc::load_level(wc::LEVEL);
    let mut rng = Rand::new();
    rng.seed(0x4c5c);
    let mut drew = 0;
    for case in 0..48 {
        let mut s = Settings::default();
        for v in s.weap_table.iter_mut() {
            *v = if rng.bound(2) == 0 { 0 } else { 1 + rng.bound(2) };
        }
        let keep = rng.bound(WEAPON_COUNT as u32) as usize;
        s.weap_table[keep] = 0; // at least one enabled
        for i in 0..2 {
            s.worm_settings[i].weapons = std::array::from_fn(|_| {
                if rng.bound(5) == 0 {
                    0
                } else {
                    rng.bound_range(1, 41)
                }
            });
            s.worm_settings[i].controller = rng.bound(2);
        }
        s.select_bot_weapons = rng.bound(4);
        let cfg = MatchConfig {
            settings: s,
            seed: rng.next_u32(),
        };
        let mut one = new_match(Path::new(wc::TC_ROOT), &cfg, &level).unwrap().state;
        let mut ws = WeaponSelection::new(&mut one, &weapsel_config(&cfg.settings)).unwrap();
        let prefix = rng.bound(40);
        drive_to_done(&mut ws, &mut one, &mut rng, prefix);
        let picks = ws.finalize(&mut one);
        enter_game(&mut one, &cfg);
        drew += (one.rand.draws() > 0) as u32;
        assert_handoff(&format!("random config {case}"), &cfg, picks, &one);
    }
    assert!(drew >= 24, "non-vacuous: most random configurations drew ({drew}/48)");
}
```

- [ ] **Step 4: Run the milestone**

Run: `rustfmt --edition 2021 /home/user/openliero/rust/oracle-tests/tests/weapsel_golden.rs /home/user/openliero/rust/oracle-tests/tests/sim_slice4_5c_continuation_golden.rs /home/user/openliero/rust/oracle-tests/tests/weapsel_handoff.rs`
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test weapsel_golden` — Expected: PASS, 4 tests.
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test sim_slice4_5c_continuation_golden` — Expected: PASS, 2 tests.
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test weapsel_handoff` — Expected: PASS, 2 tests.

If a golden line differs, localise it before fixing anything. The assertion names the case and the line (0 = init). Read the C++ golden and the Rust line side by side. `draws`/`last` on `init` points to the constructor (T1: the loop-entry rule, `used` per player, the draw order). A cursor or pick on an `f` line points to `process_frame`'s order (T2). `held` or `ctl` points to `KeyRepeat` (T2) or, if the self-check passed in C++, to the Rust replay of `OnKey`. `sounds` points to the crossed hooks. Fix the PORT, then re-run the whole re-diff; never regenerate a golden to make it pass. If a port fix changes a scenario file's `LEDGER` comment, re-run T6 Step 4 (`write`, comments only), then T6 Step 6 must regenerate byte-identical goldens: `git status --porcelain -- rust/oracle-tests/golden` then shows only the scenario comment changes.

- [ ] **Step 5: The re-diff**

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 6: Commit**

```
git -C /home/user/openliero add rust/oracle-tests/tests/weapsel_golden.rs rust/oracle-tests/tests/sim_slice4_5c_continuation_golden.rs rust/oracle-tests/tests/weapsel_handoff.rs
git -C /home/user/openliero commit -m "oracle(4.5c): MILESTONE — 16 weapsel goldens line for line vs C++, 2 continuations bit-exact, handoff equality" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---
### Task 8: `render` — `draw_rounded_box`, `get_dims`, `menu::draw_item`, the weapon-selection screen; self-goldens; `shot --weapsel`  [Sonnet]

**Files:**
- Modify: `rust/render/src/blit.rs` (`draw_rounded_box` after `draw_bar`; a test)
- Modify: `rust/render/src/font.rs` (`Font::get_dims` after `draw_string`; tests)
- Create: `rust/render/src/menu.rs`, `rust/render/src/weapsel.rs`
- Modify: `rust/render/src/lib.rs` (`pub mod menu;` after `pub mod level_draw;`, `pub mod weapsel;` after `pub mod viewport;`; the doc gains one sentence)
- Modify: `rust/scenario/src/loader.rs` (`SceneData.weapsel_texts`; `scene_data`; a test)
- Create: `rust/oracle-tests/tests/render_weapsel_selfgolden.rs`
- Modify: `rust/shot/src/lib.rs` (`Config.weapsel`, `--weapsel` parsing, `render_weapsel`, the `run` branch; tests)

**Interfaces:**
- Produces (used by T9, T11):
  - `render::blit::draw_rounded_box(scr: &mut Bitmap, pal: &Pal32, x: i32, y: i32, color: u8, height: i32, width: i32)`.
  - `render::font::Font::get_dims(&self, s: &str) -> i32`.
  - `render::menu::{ItemColours { color: u8, dis_colour: u8 }, SELECTED_COLOUR, draw_item(bmp, pal, font, text, x, y, selected, disabled, centered, colours)}`.
  - `render::weapsel::{WeapselTexts (+ from_tc), level_label(&WeapselTexts, level_file) -> String, weapsel_palette(&Palette, menu_cycles: u32) -> Pal32, menu_origin(&Rect) -> (i32, i32), build_frozen(&SimState, &Scene, label, menu_cycles) -> Bitmap, draw_screen(bmp, frozen, pal, font, texts, ws, weapons, names: [&str; 2])}` and the colour constants.
  - `scenario::SceneData.weapsel_texts: render::weapsel::WeapselTexts`.
  - `shot --weapsel --scenario-path <settings scenario> --out <png> [--scale n]`.
- Consumes: T1–T3 (`WeaponSelection::{player, weapon_index}`, `new_match`, `weapsel_config`); `render::palette::{rotate_from, pack_pal32}` (they exist already: design finding 10); `Bitmap::fill_rect` (clip-clamped, `blit.cpp:20-37`).

Why: design §3.7, §5 (Q1 ruling: pixel-exact now), §6.8. The screen needs `DrawRoundedBox` (`blit.cpp:128-140`, three `FillRect`s), `Font::GetDims` (`font.cpp:87-112`) and the text arm of `MenuItem::Draw` (`menuItem.cpp:6-42`). 4½d needs all three anyway, and none carries framework. The frozen background is `game.Draw` plus the level label, cached once as ARGB (`weapsel.cpp:165-182`), so only colour 168 animates. The screen draws with the weapsel palette (`:20-24`). Self-goldens pin it (Rust-only, rust-map §9), and `shot --weapsel` gives the PNG for the eyeball against C++. The self-goldens live in `oracle-tests`, because a `render` test cannot call `scenario::build` (plan-time fact 2).

- [ ] **Step 1: Write the failing tests**

(a) `rust/render/src/blit.rs`, inside `mod tests`:

```rust
    #[test]
    fn draw_rounded_box_is_three_fills_with_open_corners() {
        // blit.cpp:128-140: (x, y+1, w+3, h-2), (x+1, y, w+1, 1), (x+1, y+h-1, w+1, 1).
        let pal = ramp_pal();
        let mut bmp = Bitmap::new(20, 12);
        draw_rounded_box(&mut bmp, &pal, 2, 1, 7, 7, 4);
        let on = |x: i32, y: i32| bmp.get_pixel(x, y) == pal[7];
        assert!(on(2, 2) && on(8, 2) && on(2, 6) && on(8, 6), "the band: x 2..=8, y 2..=6");
        assert!(!on(1, 4) && !on(9, 4));
        assert!(on(3, 1) && on(7, 1) && on(3, 7) && on(7, 7), "top/bottom rows: x 3..=7");
        assert!(!on(2, 1) && !on(8, 1) && !on(2, 7) && !on(8, 7), "open corners");
        assert!(!on(5, 0) && !on(5, 8));
        assert_eq!(bmp.pixels.iter().filter(|&&p| p == pal[7]).count(), 7 * 5 + 5 * 2);
        // Clip-clamped like FillRect: a box hanging off every edge must not panic.
        draw_rounded_box(&mut bmp, &pal, -3, -2, 0, 40, 30);
    }
```

(b) `rust/render/src/font.rs`, inside `mod tests`:

```rust
    #[test]
    fn get_dims_sums_the_advance_widths_of_the_widest_line() {
        // font.cpp:87-112: the widths of the bytes the 2..252 gate passes; NUL breaks a line.
        let font = real_font();
        let w = |c: char| font.chars[c as usize - 2].width;
        assert_eq!(font.get_dims(""), 0);
        assert_eq!(font.get_dims("A"), w('A'));
        assert!(w('A') > 0, "non-vacuous");
        assert_eq!(font.get_dims("DONE!"), "DONE!".chars().map(w).sum::<i32>());
        assert_eq!(font.get_dims("AB\0A"), w('A') + w('B'), "the widest line");
        assert_eq!(font.get_dims("A\u{1}\u{e9}"), w('A'), "skipped exactly as draw_string skips");
    }
```

(c) Create `rust/render/src/menu.rs` with its tests (Step 3 has the code). The tests, inside `#[cfg(test)] mod tests`:

```rust
    use super::*;
    use crate::bitmap::Pal32;

    fn real_font() -> Font {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero/sprites/font.tga");
        Font::load(&assets::sprite::Tga::load(&std::fs::read(path).unwrap()).unwrap())
    }

    fn ramp_pal() -> Pal32 {
        std::array::from_fn(|i| 0xFF00_0000 | i as u32)
    }

    const DONE: ItemColours = ItemColours {
        color: 10,
        dis_colour: 9,
    };

    fn item(selected: bool, disabled: bool, centered: bool, x: i32) -> Bitmap {
        let mut bmp = Bitmap::new(80, 12);
        draw_item(&mut bmp, &ramp_pal(), &real_font(), "DONE!", x, 2, selected, disabled, centered, DONE);
        bmp
    }

    #[test]
    fn a_selected_item_is_a_box_then_colour_168() {
        let pal = ramp_pal();
        let bmp = item(true, false, false, 4);
        assert_eq!(bmp.get_pixel(4, 3), pal[0], "the box band starts at x (menuItem.cpp:15)");
        assert!(bmp.pixels.contains(&pal[168]) && !bmp.pixels.contains(&pal[10]));
    }

    #[test]
    fn an_unselected_item_is_a_colour_0_shadow_then_its_colour() {
        let pal = ramp_pal();
        let bmp = item(false, false, false, 4);
        assert!(bmp.pixels.contains(&pal[10]) && bmp.pixels.contains(&pal[0]));
        assert!(!bmp.pixels.contains(&pal[168]));
        assert_eq!(bmp.get_pixel(4, 3), 0, "no box: (x, y+1) is untouched");
    }

    #[test]
    fn disabled_wins_over_selected() {
        let pal = ramp_pal();
        let bmp = item(true, true, false, 4);
        assert!(bmp.pixels.contains(&pal[9]) && !bmp.pixels.contains(&pal[168]));
    }

    #[test]
    fn centered_shifts_left_by_half_the_width() {
        let half = real_font().get_dims("DONE!") >> 1;
        assert_eq!(item(false, false, true, 40), item(false, false, false, 40 - half));
    }
```

(d) Create `rust/render/src/weapsel.rs` with its tests (Step 3 has the code), inside `#[cfg(test)] mod tests`:

```rust
    use super::*;
    use assets::palette::Color;

    fn texts() -> WeapselTexts {
        WeapselTexts {
            level_random: "Level: Random".into(),
            level_is1: "Level: \"".into(),
            level_is2: "\"".into(),
            ..WeapselTexts::default()
        }
    }

    #[test]
    fn the_level_label_follows_level_file_not_random_level() {
        // weapsel.cpp:171-176 + filesystem.cpp:38-54.
        let t = texts();
        assert_eq!(level_label(&t, ""), "Level: Random");
        assert_eq!(level_label(&t, "Levels/render_stage.lev"), "Level: \"render_stage\"");
        assert_eq!(level_label(&t, "C:\\lev\\a.b.lev"), "Level: \"a.b\"", "the last '.'");
        assert_eq!(level_label(&t, "noext"), "Level: \"noext\"");
    }

    #[test]
    fn the_tc_strings_are_the_cpp_ones() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc = assets::tc::TcConfig::load(&std::fs::read(format!("{root}/tc.cfg")).unwrap())
            .unwrap();
        let t = WeapselTexts::from_tc(&tc.texts);
        assert_eq!((t.sel_weap.as_str(), t.randomize.as_str(), t.done.as_str()),
                   ("Select your weapons:", "Randomize", "DONE!"));
        assert_eq!(level_label(&t, "x/y.lev"), "Level: \"y\"");
    }

    #[test]
    fn the_palette_rotates_only_168_to_174() {
        let mut p = Palette {
            entries: [Color::default(); 256],
        };
        for (i, e) in p.entries.iter_mut().enumerate() {
            e.r = i as u8;
        }
        let pal = weapsel_palette(&p, 2);
        let packed = |i: usize| pack_pal32(&p)[i];
        assert_eq!(pal[168], packed(173), "dst[from+i] = src[from + (i + 7 - 2) % 7]");
        assert_eq!(pal[174], packed(172));
        assert_eq!((pal[167], pal[175], pal[50]), (packed(167), packed(175), packed(50)));
        assert_eq!(weapsel_palette(&p, 7), pack_pal32(&p), "a full turn");
    }

    #[test]
    fn the_menus_sit_at_the_cpp_origins() {
        let vps = Viewport::player_layout();
        assert_eq!(menu_origin(&vps[0].rect), (48, 28));
        assert_eq!(menu_origin(&vps[1].rect), (208, 28));
    }
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p render` — Expected: FAIL to compile (`draw_rounded_box`, `get_dims`, the `menu`/`weapsel` modules).

- [ ] **Step 3: Implement**

(1) `rust/render/src/blit.rs`, after `draw_bar`:

```rust
/// `DrawRoundedBox` (`blit.cpp:128-140`): three clip-clamped `FillRect`s — the band `(x, y+1,
/// width+3, height-2)` and the top and bottom rows `(x+1, y|y+height-1, width+1, 1)`, leaving
/// the four corners open. The menu item box, the weapon-selection header and name boxes
/// (Step 4½c), and 4½d's menus.
pub fn draw_rounded_box(
    scr: &mut Bitmap,
    pal: &Pal32,
    x: i32,
    y: i32,
    color: u8,
    height: i32,
    width: i32,
) {
    scr.fill_rect(x, y + 1, width + 3, height - 2, color, pal);
    scr.fill_rect(x + 1, y, width + 1, 1, color, pal);
    scr.fill_rect(x + 1, y + height - 1, width + 1, 1, color, pal);
}
```

(2) `rust/render/src/font.rs`, after `draw_string`:

```rust
    /// `Font::GetDims` (`font.cpp:87-112`) without the height out-param: the pixel width of
    /// `s` — the widest line (a NUL codepoint breaks a line), summing `chars[c - 2].width` over
    /// the bytes the `2..252` gate passes, decoded exactly as [`draw_string`](Self::draw_string)
    /// decodes them.
    pub fn get_dims(&self, s: &str) -> i32 {
        let mut width = 0;
        let mut max_width = 0;
        for cp in s.chars() {
            if cp == '\0' {
                max_width = max_width.max(width);
                width = 0;
                continue;
            }
            let c = ascii_to_font_byte(cp);
            if (2..252).contains(&c) {
                width += self.chars[(c - 2) as usize].width;
            }
        }
        max_width.max(width)
    }
```

(3) `rust/render/src/menu.rs`:

```rust
//! The text arm of C++ `MenuItem::Draw` (`menuItem.cpp:6-42`), the item recipe every Liero menu
//! draws with. Step 4½c pulls it forward from 4½d (design §5 option A); the `has_value` arm and
//! the `Menu` framework (visibility, scrolling, the scrollbar, type-to-search) stay 4½d's.

use crate::bitmap::{Bitmap, Pal32};
use crate::blit::draw_rounded_box;
use crate::font::Font;

/// The selected item's text colour (`menuItem.cpp:32`), the head of the rotated range 168..174.
pub const SELECTED_COLOUR: i32 = 168;

/// `MenuItem::color` / `dis_colour` (`menuItem.hpp:11-16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemColours {
    pub color: u8,
    pub dis_colour: u8,
}

/// `MenuItem::Draw` with no value (`has_value == false`): a selected item gets a rounded box of
/// its text width, an unselected one a colour-0 shadow at (+3, +2); then the text at (+2, +1)
/// in `dis_colour` if disabled, else 168 if selected, else `color`. `centered` shifts left by
/// half the width (`:10-12`).
#[allow(clippy::too_many_arguments)]
pub fn draw_item(
    bmp: &mut Bitmap,
    pal: &Pal32,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    selected: bool,
    disabled: bool,
    centered: bool,
    colours: ItemColours,
) {
    let wid = font.get_dims(text);
    let x = if centered { x - (wid >> 1) } else { x };
    if selected {
        draw_rounded_box(bmp, pal, x, y, 0, 7, wid);
    } else {
        font.draw_string(bmp, pal, text, x + 3, y + 2, 0, 1);
    }
    let c = if disabled {
        colours.dis_colour as i32
    } else if selected {
        SELECTED_COLOUR
    } else {
        colours.color as i32
    };
    font.draw_string(bmp, pal, text, x + 2, y + 1, c, 1);
}

#[cfg(test)]
mod tests {
    // … Step 1 (c) …
}
```

(4) `rust/render/src/weapsel.rs`:

```rust
//! The weapon-selection screen, normal viewports (`weapsel.cpp:160-209`). Step 4½c pulls it
//! forward from 4½d (design §5 option A; John's Q1 ruling: pixel-exact now). Bevy-free, so
//! `shot` and the self-goldens draw it headlessly.
//!
//! C++ first rebuilds the weapsel palette (`UpdateWeapselPalette`, `:20-24`: `Origpal`, then
//! `RotateFrom(Origpal, 168, 174, menu_cycles)`). On the first draw it caches a FROZEN ARGB
//! copy of `game.Draw` (level, HUD, minimap) plus the level label (`:165-180`), so later
//! rotation only reaches what is drawn on top of it: colour 168 of the selected item. Every
//! frame then copies the frozen screen back (`:182`), draws the header box and "Select your
//! weapons:" (`:188-190`), and per player the name box and name (`:200-203`) and, unless the
//! player is ready, the seven-item menu (`:205-207`).
//!
//! Not here (4½d): the render fade, the spectator variant, `Focus`/`Unfocus`.
//! Pixel caveats vs C++ (plan-time facts 1, 4): the worm-colour palette step
//! (`Palette::SetWormColour`) is unported, so name colours 33/42 differ slightly, and C++
//! draws random player names where Rust draws the settings' names (empty by default).

use assets::object::Weapon;
use assets::palette::Palette;
use assets::tc::Texts;
use sim::state::SimState;
use sim::weapsel::{WeaponSelection, DONE_ITEM, MENU_ITEMS, RANDOMIZE_ITEM};

use crate::bitmap::{Bitmap, Pal32, Rect};
use crate::blit::draw_rounded_box;
use crate::font::Font;
use crate::frame::{self, Scene};
use crate::menu::{draw_item, ItemColours};
use crate::palette::{pack_pal32, rotate_from};
use crate::viewport::Viewport;

/// The menu water rotation (`weapsel.cpp:22`).
pub const ROTATE_FROM: i32 = 168;
pub const ROTATE_TO: i32 = 174;
/// The header and the level label (`weapsel.cpp:172`, `:175`, `:190`).
pub const LABEL_COLOUR: i32 = 50;
/// `Palette::kWormColorBlocks[i].base + 1` (`palette.cpp:77-79`, `weapsel.cpp:203`).
pub const NAME_COLOURS: [i32; 2] = [33, 42];
/// RANDOMIZE, the weapon slots, DONE (`weapsel.cpp:49`, `:87`, `:90`).
pub const RANDOMIZE_COLOURS: ItemColours = ItemColours { color: 57, dis_colour: 57 };
pub const WEAPON_COLOURS: ItemColours = ItemColours { color: 48, dis_colour: 48 };
pub const DONE_COLOURS: ItemColours = ItemColours { color: 10, dis_colour: 10 };
/// `Menu::item_height` (`menu.hpp:37`, `menu.cpp:104`).
pub const ITEM_HEIGHT: i32 = 8;

/// The TC strings the screen draws (`tc.cfg:245-250`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeapselTexts {
    pub sel_weap: String,
    pub level_random: String,
    pub level_is1: String,
    pub level_is2: String,
    pub randomize: String,
    pub done: String,
}

impl WeapselTexts {
    pub fn from_tc(t: &Texts) -> WeapselTexts {
        WeapselTexts {
            sel_weap: t.SelWeap.clone(),
            level_random: t.LevelRandom.clone(),
            level_is1: t.LevelIs1.clone(),
            level_is2: t.LevelIs2.clone(),
            randomize: t.Randomize.clone(),
            done: t.Done.clone(),
        }
    }
}

/// The level label (`weapsel.cpp:171-176`): `LevelRandom` when `level_file` is EMPTY (not when
/// `random_level` is set), else `LevelIs1 + GetBasename(GetLeaf(level_file)) + LevelIs2`
/// (`filesystem.cpp:38-54`: the leaf after the last `/` or `\`, then up to its last `.`).
pub fn level_label(texts: &WeapselTexts, level_file: &str) -> String {
    if level_file.is_empty() {
        return texts.level_random.clone();
    }
    let leaf = level_file
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(level_file);
    let base = leaf.rsplit_once('.').map_or(leaf, |(b, _)| b);
    format!("{}{}{}", texts.level_is1, base, texts.level_is2)
}

/// `UpdateWeapselPalette` (`weapsel.cpp:20-24`): `Origpal`, then `RotateFrom(Origpal, 168, 174,
/// menu_cycles)`. No `color_anim`, no flash. `menu_cycles` is `Gfx::menu_cycles` (`unsigned`).
pub fn weapsel_palette(origpal: &Palette, menu_cycles: u32) -> Pal32 {
    let mut pal = origpal.clone();
    rotate_from(&mut pal, origpal, ROTATE_FROM, ROTATE_TO, menu_cycles);
    pack_pal32(&pal)
}

/// `Menu::Place(vp.rect.CenterX() - 31, vp.rect.CenterY() - 51)` (`weapsel.cpp:51-55`), with
/// the integer `(x1 + x2) / 2` of `math/rect.hpp:72-74`.
pub fn menu_origin(rect: &Rect) -> (i32, i32) {
    ((rect.x1 + rect.x2) / 2 - 31, (rect.y1 + rect.y2) / 2 - 51)
}

/// The frozen background (`weapsel.cpp:165-180`), built once per phase. `game.Draw` becomes
/// `frame::draw` with FRESH viewports, which equal C++'s unprocessed ones (plan-time fact 3),
/// then the level label is drawn at (0, 162) in colour 50 through the weapsel palette.
pub fn build_frozen(state: &SimState, scene: &Scene, label: &str, menu_cycles: u32) -> Bitmap {
    let mut bmp = Bitmap::new(320, 200);
    let mut viewports = Viewport::player_layout();
    frame::draw(&mut bmp, state, &mut viewports, scene);
    bmp.clip = Rect::new(0, 0, bmp.w, bmp.h);
    let pal = weapsel_palette(scene.origpal, menu_cycles);
    scene
        .font
        .draw_string(&mut bmp, &pal, label, 0, 162, LABEL_COLOUR, 1);
    bmp
}

/// One frame of the screen over `frozen` (`weapsel.cpp:182-208`) with the weapsel palette `pal`.
/// `names` are the players' names (`WormSettings::name`).
#[allow(clippy::too_many_arguments)]
pub fn draw_screen(
    bmp: &mut Bitmap,
    frozen: &Bitmap,
    pal: &Pal32,
    font: &Font,
    texts: &WeapselTexts,
    ws: &WeaponSelection,
    weapons: &[Weapon],
    names: [&str; 2],
) {
    bmp.pixels.copy_from_slice(&frozen.pixels); // :182 renderer.bmp.Copy(frozen_screen)
    bmp.clip = Rect::new(0, 0, bmp.w, bmp.h);
    draw_rounded_box(bmp, pal, 114, 2, 0, 7, font.get_dims(&texts.sel_weap)); // :188
    font.draw_string(bmp, pal, &texts.sel_weap, 116, 3, LABEL_COLOUR, 1); // :190
    for (i, vp) in Viewport::player_layout().iter().enumerate() {
        let (mx, my) = menu_origin(&vp.rect);
        let width = font.get_dims(names[i]); // :200-203
        draw_rounded_box(bmp, pal, mx + 29 - width / 2, my - 11, 0, 7, width);
        font.draw_string(bmp, pal, names[i], mx + 31 - width / 2, my - 10, NAME_COLOURS[i], 1);
        let p = ws.player(i);
        if p.ready {
            continue; // :205
        }
        // Menu::Draw (menu.cpp:81-105): all seven items, no scrollbar (finding 3).
        for k in 0..MENU_ITEMS {
            let (text, colours) = match k {
                RANDOMIZE_ITEM => (texts.randomize.as_str(), RANDOMIZE_COLOURS),
                DONE_ITEM => (texts.done.as_str(), DONE_COLOURS),
                slot => (
                    weapons[ws.weapon_index(i, slot as usize - 1)].name.as_str(),
                    WEAPON_COLOURS,
                ),
            };
            let y = my + ITEM_HEIGHT * k as i32;
            draw_item(bmp, pal, font, text, mx, y, p.cursor == k, false, false, colours);
        }
    }
}

#[cfg(test)]
mod tests {
    // … Step 1 (d) …
}
```

(5) `rust/render/src/lib.rs`: add the two `pub mod` lines. Append to the doc: `Step 4½c adds the menu-item recipe (menu) and the weapon-selection screen (weapsel), pulled forward from 4½d.`

(6) `rust/scenario/src/loader.rs`: add to `SceneData` (after `labels`):

```rust
    /// Step 4½c: the weapon-selection screen's TC strings (`tc.cfg:245-250`).
    pub weapsel_texts: render::weapsel::WeapselTexts,
```

In `scene_data`, add `weapsel_texts: render::weapsel::WeapselTexts::from_tc(&tc.texts),` to the literal. Add a test next to `load_yields_font_and_labels`:

```rust
    #[test]
    fn load_yields_the_weapsel_texts() {
        let loaded = load(Path::new(TC_ROOT), &Scenario::parse(SAMPLE).expect("parses"));
        assert_eq!(loaded.scene.weapsel_texts.sel_weap, "Select your weapons:");
        assert_eq!(loaded.scene.weapsel_texts.done, "DONE!");
    }
```

- [ ] **Step 4: GREEN for the unit tests**

Run: `rustfmt --edition 2021 /home/user/openliero/rust/render/src/menu.rs /home/user/openliero/rust/render/src/weapsel.rs`
Run: `cd /home/user/openliero/rust && cargo test -p render` — Expected: PASS (1 + 1 + 4 + 4 new).
Run: `cd /home/user/openliero/rust && cargo test -p scenario --lib loader` — Expected: PASS.

- [ ] **Step 5: `shot --weapsel`** — in `rust/shot/src/lib.rs`:

(1) `Config` gains `pub weapsel: bool` (doc: `` `--weapsel` (Step 4½c): render the initial weapon-selection screen of a `settings` scenario (needs `--scenario-path` and `--out`; no `--tick`). ``). `parse_args` matches `"--weapsel" => weapsel = true,`. The `ticks.is_empty()` check becomes `if ticks.is_empty() && !weapsel`. Add, after the `scale == 0` check:

```rust
    if weapsel && (scenario_path.is_none() || out.is_none() || hashes || !ticks.is_empty()) {
        return Err("--weapsel needs --scenario-path and --out, and takes no --tick/--hashes".to_string());
    }
```

Set `weapsel` in the returned `Config`, and add `weapsel: false` to any `Config { … }` literal in the tests (`grep -n "Config {" /home/user/openliero/rust/shot/src/*.rs`). Update the `USAGE` text if the file has one.

(2) Add

```rust
/// `--weapsel` (Step 4½c): the INITIAL weapon-selection screen of a `settings` scenario, built on
/// the faithful path — `new_match` (invisible worms, lives 0) + `sim::weapsel` +
/// `render::weapsel` — for the eyeball against the C++ build. The label follows
/// `settings.level_file` (the C++ rule); the names are the setup's.
pub fn render_weapsel(tc_root: &Path, scenario_path: &Path, scale: u32) -> Result<Vec<u8>, String> {
    use render::weapsel::{build_frozen, draw_screen, level_label, weapsel_palette};
    use scenario::build::{new_match, weapsel_config};
    use scenario::settings::MatchConfig;
    use sim::weapsel::WeaponSelection;

    let text = std::fs::read_to_string(scenario_path)
        .map_err(|e| format!("read {}: {e}", scenario_path.display()))?;
    let scenario = Scenario::parse(&text)?;
    let rel = scenario
        .settings
        .as_ref()
        .ok_or("--weapsel needs a `settings` scenario")?;
    let dir = scenario_path.parent().unwrap_or(Path::new("."));
    let setup = std::fs::read_to_string(dir.join(rel)).map_err(|e| format!("read {rel}: {e}"))?;
    let settings = scenario::settings_toml::settings_from_toml(&setup)
        .map_err(|e| format!("{rel}: {e:?}"))?;
    let level = assets::level::load(&scenario::assets::read_asset(tc_root, &scenario.level))
        .map_err(|e| format!("{}: {e:?}", scenario.level))?;
    let cfg = MatchConfig {
        settings,
        seed: scenario.seed,
    };
    let mut loaded = new_match(tc_root, &cfg, &level).map_err(|e| e.to_string())?;
    let ws = WeaponSelection::new(&mut loaded.state, &weapsel_config(&cfg.settings))
        .map_err(|e| e.to_string())?;
    let mut scene = loaded.scene.as_scene(0, cfg.settings.shadow);
    scene.draw_hud = true;
    scene.map = cfg.settings.map;
    let label = level_label(&loaded.scene.weapsel_texts, &cfg.settings.level_file);
    let frozen = build_frozen(&loaded.state, &scene, &label, 0);
    let mut surface = Bitmap::new(320, 200);
    let pal = weapsel_palette(&loaded.scene.origpal, 0);
    let names = [
        cfg.settings.worm_settings[0].name.as_str(),
        cfg.settings.worm_settings[1].name.as_str(),
    ];
    draw_screen(
        &mut surface,
        &frozen,
        &pal,
        &loaded.scene.font,
        &loaded.scene.weapsel_texts,
        &ws,
        &loaded.state.weapons,
        names,
    );
    Ok(encode_png(&surface, scale))
}
```

and at the top of `run`, after `tc_root` is resolved:

```rust
    if cfg.weapsel {
        let path = cfg.scenario_path.as_ref().expect("parse_args: --weapsel has a path");
        let out = cfg.out.as_ref().expect("parse_args: --weapsel has --out");
        let png = render_weapsel(&tc_root, path, cfg.scale)?;
        std::fs::write(out, &png).map_err(|e| format!("write {}: {e}", out.display()))?;
        eprintln!("shot: wrote {} (the weapon-selection screen)", out.display());
        return Ok(());
    }
```

(3) Tests in `shot`'s test module:

```rust
    #[test]
    fn weapsel_needs_a_path_and_out_and_takes_no_tick() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let ok = parse_args(&args("--weapsel --scenario-path a.txt --out b.png")).unwrap();
        assert!(ok.weapsel && ok.ticks.is_empty());
        assert!(parse_args(&args("--weapsel --scenario-path a.txt --out b.png --tick 3")).is_err());
        assert!(parse_args(&args("--weapsel --scenario a --out b.png")).is_err());
        assert!(parse_args(&args("--weapsel --scenario-path a.txt --hashes")).is_err());
    }
```

Run: `cd /home/user/openliero/rust && cargo test -p shot` — Expected: PASS.

- [ ] **Step 6: The self-goldens** — create `rust/oracle-tests/tests/render_weapsel_selfgolden.rs`:

```rust
//! Step 4½c T8 — the weapon-selection screen, Rust self-goldens (design §6.8). There is no C++
//! frame counterpart (rust-map §9): the screen was compared by eye with a C++ build before these
//! constants were pinned (T8's done-report and PROGRESS name the result). Built on the faithful
//! path — `new_match` over `Settings::default()` with `level_file` set (invisible worms, lives
//! 0), not the live scenario path.

use std::path::Path;

use render::bitmap::Bitmap;
use render::hash::hash_frame;
use render::viewport::Viewport;
use render::weapsel::{build_frozen, draw_screen, level_label, menu_origin, weapsel_palette};
use scenario::build::{new_match, weapsel_config};
use scenario::settings::{MatchConfig, Settings};
use scenario::Loaded;
use sim::state::ControlState;
use sim::weapsel::WeaponSelection;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
const LEVEL: &str = "Levels/render_stage.lev";

// `hash_frame(&surface, 33)`, pinned at T8 Step 7 after the eyeball.
const INITIAL: u64 = 0x0;
const SLOT_AFTER_5_CYCLES: u64 = 0x0;
const P0_READY: u64 = 0x0;

struct Screen {
    loaded: Loaded,
    ws: WeaponSelection,
    frozen: Bitmap,
}

fn screen() -> Screen {
    let mut settings = Settings::default();
    settings.level_file = LEVEL.to_string();
    let cfg = MatchConfig { settings, seed: 1 };
    let level =
        assets::level::load(&std::fs::read(format!("{TC_ROOT}/{LEVEL}")).unwrap()).unwrap();
    let mut loaded = new_match(Path::new(TC_ROOT), &cfg, &level).unwrap();
    let ws = WeaponSelection::new(&mut loaded.state, &weapsel_config(&cfg.settings)).unwrap();
    let mut scene = loaded.scene.as_scene(0, cfg.settings.shadow);
    scene.draw_hud = true;
    scene.map = cfg.settings.map;
    let label = level_label(&loaded.scene.weapsel_texts, &cfg.settings.level_file);
    let frozen = build_frozen(&loaded.state, &scene, &label, 0);
    Screen { loaded, ws, frozen }
}

fn draw(s: &Screen, menu_cycles: u32) -> Bitmap {
    let mut surface = Bitmap::new(320, 200);
    let pal = weapsel_palette(&s.loaded.scene.origpal, menu_cycles);
    draw_screen(
        &mut surface,
        &s.frozen,
        &pal,
        &s.loaded.scene.font,
        &s.loaded.scene.weapsel_texts,
        &s.ws,
        &s.loaded.state.weapons,
        ["", ""],
    );
    surface
}

fn step(s: &mut Screen, a: u32, b: u32) {
    s.ws.process_frame(
        &mut s.loaded.state,
        &[ControlState::unpack(a), ControlState::unpack(b)],
    );
}

#[test]
fn the_initial_screen() {
    let h = hash_frame(&draw(&screen(), 0), 33);
    assert_eq!(h, INITIAL, "got {h:#018x}");
}

#[test]
fn a_weapon_slot_selected_after_five_menu_cycles() {
    let mut s = screen();
    step(&mut s, 2, 0); // Down: cursor 0 -> 1
    step(&mut s, 0, 0);
    assert_eq!(s.ws.player(0).cursor, 1);
    let h = hash_frame(&draw(&s, 5), 33);
    assert_eq!(h, SLOT_AFTER_5_CYCLES, "got {h:#018x}");
}

#[test]
fn player_one_ready_hides_its_menu() {
    let mut s = screen();
    step(&mut s, 1, 0); // Up: 0 -> 6
    step(&mut s, 0, 0);
    step(&mut s, 16, 0); // Fire on DONE
    assert!(s.ws.player(0).ready);
    let h = hash_frame(&draw(&s, 0), 33);
    assert_eq!(h, P0_READY, "got {h:#018x}");
}

#[test]
fn the_layout_is_the_cpp_one() {
    let s = screen();
    let bmp = draw(&s, 0);
    let pal = weapsel_palette(&s.loaded.scene.origpal, 0);
    let vps = Viewport::player_layout();
    assert_eq!((menu_origin(&vps[0].rect), menu_origin(&vps[1].rect)), ((48, 28), (208, 28)));
    let changed = |x: i32, y: i32| bmp.get_pixel(x, y) != s.frozen.get_pixel(x, y);
    let painted = |x0: i32, y0: i32, x1: i32, y1: i32, c: u32| {
        (y0..y1).any(|y| (x0..x1).any(|x| changed(x, y) && bmp.get_pixel(x, y) == c))
    };
    assert!(painted(116, 3, 250, 11, pal[50]), "the header text (weapsel.cpp:190)");
    assert!(painted(48, 28, 120, 37, pal[168]), "P0's selected RANDOMIZE in colour 168");
    assert!(painted(48, 36, 120, 77, pal[48]), "P0's five weapon rows in colour 48");
    assert!(painted(48, 76, 120, 85, pal[10]), "P0's DONE! in colour 10");
    assert!(painted(208, 28, 280, 37, pal[168]), "P1's menu at x 208");
    assert!(
        !(0..320).any(|x| (100..200).any(|y| changed(x, y))),
        "nothing below the menus is redrawn: the frozen frame (incl. the label) shows through"
    );
    assert!(
        (0..120).any(|x| (162..170).any(|y| s.frozen.get_pixel(x, y) == pal[50])),
        "the level label at (0, 162) in colour 50 is part of the frozen frame"
    );
}
```

Run: `rustfmt --edition 2021 /home/user/openliero/rust/oracle-tests/tests/render_weapsel_selfgolden.rs`
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test render_weapsel_selfgolden`
Expected: `the_layout_is_the_cpp_one` PASSES. The three hash tests FAIL (the constants are `0x0`), each printing `got 0x…`. Do not paste them yet.

- [ ] **Step 7: The eyeball, then pin**

Create `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws8/eyeball_scenario.txt`:

```
seed 1
level Levels/render_stage.lev
ticks 0
settings eyeball_setup.cfg
```

and `…/ws8/eyeball_setup.cfg`:

```toml
[settings]
levelFile = 'Levels/render_stage.lev'
```

Run: `cd /home/user/openliero/rust && cargo run -p shot -- --weapsel --scenario-path /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws8/eyeball_scenario.txt --out /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/ws8/weapsel.png --scale 3`
View the PNG (Read tool). Expected, against the C++ layout (`weapsel.cpp:160-209`):
- the level with the HUD panel and the minimap;
- a black rounded box at the top centre with "Select your weapons:" in light yellow;
- two small empty name boxes above the menus (empty names);
- two menus at x 48 and x 208, y 28: "Randomize" boxed and in white (168) on both; five "BAZOOKA" lines in light blue (48); "DONE!" in green (10);
- `Level: "render_stage"` at the bottom left (y 162).
There is no C++ GUI capture in this container (plan-time fact; T11 records it for John), so this eyeball checks the layout against the C++ code. Record the known pixel caveats in the done-report: SetWormColour is unported; C++ draws random names; there is no fade.
Then paste the three printed `got` values into `INITIAL`, `SLOT_AFTER_5_CYCLES` and `P0_READY`.
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test render_weapsel_selfgolden` — Expected: PASS, 4 tests.
Run: `cd /home/user/openliero/rust && cargo test -p oracle-tests --test render_weapsel_selfgolden` again — Expected: PASS (deterministic).

- [ ] **Step 8: The re-diff**

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS (no render golden moved: nothing in `frame::draw` changed).
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown` — Expected: builds.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 9: Commit**

```
git -C /home/user/openliero add rust/render/src/blit.rs rust/render/src/font.rs rust/render/src/menu.rs rust/render/src/weapsel.rs rust/render/src/lib.rs rust/scenario/src/loader.rs rust/oracle-tests/tests/render_weapsel_selfgolden.rs rust/shot/src/lib.rs
git -C /home/user/openliero commit -m "render(4.5c): draw_rounded_box, get_dims, the MenuItem text arm, the weapon-selection screen; self-goldens; shot --weapsel" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

---

### Task 9: `game` — the live phase, the release latch, restart + write-back, the skips, the touch rule, the page text  [Opus]

**Files:**
- Modify: `rust/game/src/match_flow.rs` (module doc; `MatchPhase::WeaponSelection`; `with_weapon_selection`, `weapsel_frame`, `enter_game`; the `after_frame` guard; tests)
- Modify: `rust/game/src/input.rs` (`ReleaseLatch`; tests)
- Modify: `rust/game/src/web_params.rs` (module doc `:1-16`; `MatchParams::skips_weapon_selection`; a test)
- Create: `rust/game/src/selection.rs`
- Modify: `rust/game/src/lib.rs` (`pub mod selection;` + the doc)
- Modify: `rust/game/src/main.rs` (module doc; imports; `Demo`; `setup`; `tick_and_render`; `render_and_upload`; `restart_match`; new `sample_inputs`, `touch_only`, `publish_phase`)
- Modify: `web/index.html`, `.github/workflows/preview.yml`

**Interfaces:**
- Produces (used by T10, T11):
  - `MatchPhase::WeaponSelection`; `MatchFlow::with_weapon_selection()` (fade 0); `weapsel_frame(&mut self)` (fade += 1 up to 33); `enter_game(&mut self)` (Game, fade 33).
  - `game::input::ReleaseLatch { arm(&mut self, &[ControlState; 2]), apply(&mut self, &mut [ControlState; 2]), is_armed(&self) -> bool }` (`Default`).
  - `MatchParams::skips_weapon_selection(&self) -> bool` (`?weapons=` named at least one weapon).
  - `game::selection::{BOT_WEAPONS_KEEP, CONTROLLER_BOT, loadout_picks(&SimState, worm) -> [u32; 5], live_config(&SimState, touch_only) -> WeapselConfig, Selection}` with `Selection::{new(cfg), config(), is_active(), active(), begin(&mut SimState) -> Result<(), WeapselError>, step(&mut SimState, &[ControlState; 2], &mut Vec<i32>) -> bool, abandon(), render(surface, state, scene, texts, level_file), menu_cycles()}`.
  - Page globals: `window.lieroTouchOnly` (set by the page, read at startup) and `window.lieroPhase` (`"weapsel"`/`"game"`, set by the game).
- Consumes: T1–T3 (`sim::weapsel`, `scenario::build::weapsel_config`), T8 (`render::weapsel`, `SceneData.weapsel_texts`).

Why: design §7. The phase runs inside `tick_and_render` (LD 3): one sample, the touch merge, the latch, `process_frame` on the sim, the menu sounds into the `AudioSink`, and the flow's fade toward 33. It does NOT call `process_frame` or the viewport stepping, so `cycles` does not advance, as in C++. The latch reproduces C++'s key edges at both boundaries (§7.2, Q5): a held DONE never fires on tick 0. It sits before the recorder tap and is only armed by selection boundaries, which a recorded run never has. The live phase selects from `Settings::default()`, with each worm's launched loadout as its saved picks (§7.3, Q7): DART + BAZOOKA ×4, not five BAZOOKAs. F5 and the post-match restart write back the running picks and start a NEW selection from them. That is the C++ NEW GAME loop (finding 4, §7.4). `--live --record` skips selection with a stderr notice (§7.5, Q4). `?weapons=` skips it (§7.6, Q3). A touch-only page makes player 2 a KEEP bot that readies at once (Q8).

- [ ] **Step 1: Write the failing tests**

(a) `rust/game/src/match_flow.rs`, inside `mod tests`:

```rust
    #[test]
    fn weapon_selection_fades_in_from_zero_then_the_game_starts_at_33() {
        // localController.cpp:119 (Focus: fade 0), :195-199 (+1 per Process, up to 33),
        // :284-287 (ChangeState from weapsel: 33).
        let mut f = MatchFlow::with_weapon_selection();
        assert_eq!((f.phase(), f.fade_value()), (MatchPhase::WeaponSelection, 0));
        for n in 1..=40 {
            f.weapsel_frame();
            assert_eq!(f.fade_value(), n.min(FADE_IN_MAX));
        }
        f.enter_game();
        assert_eq!((f.phase(), f.fade_value()), (MatchPhase::Game, FADE_IN_MAX));
    }

    #[test]
    fn entering_the_game_mid_fade_jumps_to_33() {
        let mut f = MatchFlow::with_weapon_selection();
        for _ in 0..3 {
            f.weapsel_frame();
        }
        f.enter_game();
        assert_eq!(f.fade_value(), FADE_IN_MAX);
        assert_eq!(f.after_frame(&state()), FlowStep::Continue);
    }

    #[test]
    #[should_panic(expected = "weapon selection")]
    fn after_frame_is_for_the_match_only() {
        MatchFlow::with_weapon_selection().after_frame(&state());
    }
```

(b) `rust/game/src/input.rs`, inside `mod tests`:

```rust
    // ---- Step 4½c: the release latch (design §7.2) -----------------------------------

    fn cs(bits: u32) -> ControlState {
        ControlState::unpack(bits)
    }

    #[test]
    fn an_unarmed_latch_passes_everything() {
        let mut l = ReleaseLatch::default();
        let mut i = [cs(0x7f), cs(16)];
        l.apply(&mut i);
        assert_eq!((i[0].pack(), i[1].pack()), (0x7f, 16));
        assert!(!l.is_armed());
    }

    #[test]
    fn a_latched_key_does_nothing_until_released_then_a_repress_passes() {
        let mut l = ReleaseLatch::default();
        l.arm(&[cs(16), cs(0)]); // worm 0 held Fire at the boundary (the DONE press)
        for _ in 0..5 {
            let mut i = [cs(16 | 4), cs(16)];
            l.apply(&mut i);
            assert_eq!((i[0].pack(), i[1].pack()), (4, 16), "only worm 0's Fire is masked");
        }
        let mut i = [cs(0), cs(0)];
        l.apply(&mut i); // released
        assert!(!l.is_armed());
        let mut i = [cs(16), cs(0)];
        l.apply(&mut i);
        assert_eq!(i[0].pack(), 16, "pressed again: it passes (gfx.cpp:608 + game.cpp:110-118)");
    }
```

(c) `rust/game/src/web_params.rs`, inside `mod tests`:

```rust
    #[test]
    fn weapons_skips_weapon_selection() {
        // Q3 (John): a preview link with ?weapons= keeps today's path exactly.
        assert!(!MatchParams::parse("").skips_weapon_selection());
        assert!(!MatchParams::parse("?level=water_stage&seed=3").skips_weapon_selection());
        assert!(MatchParams::parse("?weapons=missile").skips_weapon_selection());
        assert!(MatchParams::parse("?weapons=NOPE").skips_weapon_selection(), "a named loadout");
        assert!(!MatchParams::parse("?weapons=").skips_weapon_selection(), "names nothing");
    }
```

(d) Create `rust/game/src/selection.rs` (Step 3 has the code) with these tests in `#[cfg(test)] mod tests`:

```rust
    use std::path::Path;

    use scenario::Scenario;
    use scenario::paths::TC_ROOT;

    use super::*;

    const FIXTURE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios/default_match.txt"));

    fn default_match() -> scenario::Loaded {
        scenario::load(Path::new(TC_ROOT), &Scenario::parse(FIXTURE).unwrap())
    }

    fn cs(bits: u32) -> ControlState {
        ControlState::unpack(bits)
    }

    #[test]
    fn the_launched_loadout_inverts_to_dart_plus_the_first_weapon_by_name() {
        let st = default_match().state;
        for worm in 0..2 {
            assert_eq!(loadout_picks(&st, worm), [12, 1, 1, 1, 1], "DART + BAZOOKA x4 (Q7)");
        }
    }

    #[test]
    fn the_live_config_is_the_cpp_default_plus_the_touch_rule() {
        let st = default_match().state;
        let keys = live_config(&st, false);
        assert_eq!(keys.weap_table, [0; 40]);
        assert_eq!(keys.select_bot_weapons, 1, "Settings(): PICK");
        assert_eq!((keys.players[0].controller, keys.players[1].controller), (0, 0));
        let touch = live_config(&st, true);
        assert_eq!(
            (touch.players[0].controller, touch.players[1].controller, touch.select_bot_weapons),
            (0, CONTROLLER_BOT, BOT_WEAPONS_KEEP)
        );
        assert_eq!(touch.players[1].weapons, keys.players[1].weapons);
    }

    #[test]
    fn on_a_touch_only_page_player_one_alone_ends_the_phase() {
        let mut st = default_match().state;
        let mut sel = Selection::new(live_config(&st, true));
        sel.begin(&mut st).unwrap();
        let ws = sel.active().unwrap();
        assert!(!ws.player(0).ready && ws.player(1).ready, "Q8: player 2 is ready at once");
        assert_eq!(st.rand.draws(), 0, "KEEP with saved picks draws nothing");
        let mut sounds = Vec::new();
        for (bits, done) in [(1, false), (0, false), (16, true)] {
            assert_eq!(sel.step(&mut st, &[cs(bits), cs(0)], &mut sounds), done);
        }
        assert!(!sel.is_active());
    }

    #[test]
    fn finishing_writes_the_picks_back_and_loads_them() {
        let mut st = default_match().state;
        let mut sel = Selection::new(live_config(&st, false));
        sel.begin(&mut st).unwrap();
        let mut sounds = Vec::new();
        // P0: Down to slot 0, Right (DART -> DIRTBALL), Up, Up to DONE, Fire. P1: Up, Fire.
        for w in [[2, 1], [0, 0], [8, 16], [0, 0], [1, 0], [0, 0], [1, 0], [0, 0]] {
            assert!(!sel.step(&mut st, &[cs(w[0]), cs(w[1])], &mut sounds));
        }
        assert!(sel.step(&mut st, &[cs(16), cs(0)], &mut sounds));
        assert_eq!(sel.config().players[0].weapons, [13, 1, 1, 1, 1], "written back");
        let ty = st.worms[0].weapons[0].ty.expect("loaded") as usize;
        assert_eq!(st.weapons[ty].name, "DIRTBALL");
        assert_eq!(st.worms[0].weapons[0].ammo, st.weapons[ty].ammo, "InitWeapons: full ammo");
        assert_eq!(st.worms[0].control_states.pack(), 0, "ReleaseControls");
        assert!(!sounds.is_empty());
    }

    #[test]
    fn abandoning_keeps_the_running_picks_for_the_next_selection() {
        let mut st = default_match().state;
        let mut sel = Selection::new(live_config(&st, false));
        sel.begin(&mut st).unwrap();
        let mut sounds = Vec::new();
        for w in [2, 0, 8] {
            sel.step(&mut st, &[cs(w), cs(0)], &mut sounds);
        }
        sel.abandon(); // F5 mid-selection
        assert!(!sel.is_active());
        assert_eq!(sel.config().players[0].weapons, [13, 1, 1, 1, 1], "finding 4");
        let mut fresh = default_match().state;
        sel.begin(&mut fresh).unwrap();
        assert_eq!(sel.active().unwrap().player(0).picks, [13, 1, 1, 1, 1], "NEW GAME from them");
    }

    #[test]
    fn render_freezes_once_and_counts_menu_cycles_after_the_draw() {
        let scenario::Loaded { mut state, scene, .. } = default_match();
        let mut sel = Selection::new(live_config(&state, false));
        sel.begin(&mut state).unwrap();
        let s = scene.as_scene(0, false);
        let mut surface = Bitmap::new(320, 200);
        sel.render(&mut surface, &state, &s, &scene.weapsel_texts, "Levels/render_stage.lev");
        assert_eq!(sel.menu_cycles(), 1, "gfx.cpp:1646: after the draw");
        let first = surface.clone();
        sel.render(&mut surface, &state, &s, &scene.weapsel_texts, "Levels/render_stage.lev");
        assert_eq!(sel.menu_cycles(), 2);
        assert_ne!(first, surface, "the selected item's colour 168 rotates");
    }
```

- [ ] **Step 2: Run to see RED**

Run: `cd /home/user/openliero/rust && cargo test -p game --lib` — Expected: FAIL to compile.

- [ ] **Step 3: Implement the Bevy-free parts**

(1) `match_flow.rs`. Module doc: replace "Seams: 4½c adds a weapon-selection phase in front, 4½d the Esc fade" with "Since 4½c the flow starts in the weapon-selection phase (`with_weapon_selection`; `LocalController::Focus`, `:112-119`) unless selection is skipped. Seams: 4½d the Esc fade". Then:

```rust
pub enum MatchPhase {
    /// `kStateWeaponSelection` (Step 4½c): `game::selection` runs the phase; the sim does not tick.
    WeaponSelection,
    Game,
    GameEnded,
}
```

and inside `impl MatchFlow`, after `new`:

```rust
    /// A fresh controller's `Focus` (`localController.cpp:112-119`): weapon selection first,
    /// `fade_value = 0`.
    pub fn with_weapon_selection() -> Self {
        MatchFlow {
            phase: MatchPhase::WeaponSelection,
            fade_value: 0,
            going_to_menu: false,
        }
    }

    /// One weapon-selection `Process` (`localController.cpp:195-199`): the fade counts up to 33.
    pub fn weapsel_frame(&mut self) {
        debug_assert_eq!(self.phase, MatchPhase::WeaponSelection);
        if self.fade_value < FADE_IN_MAX {
            self.fade_value += 1;
        }
    }

    /// `ChangeState(kStateGame)` from weapon selection (`:284-287`): fade 33; the next tick is
    /// match tick 0.
    pub fn enter_game(&mut self) {
        debug_assert_eq!(self.phase, MatchPhase::WeaponSelection);
        self.phase = MatchPhase::Game;
        self.fade_value = FADE_IN_MAX;
    }
```

At the top of `after_frame`: `debug_assert_ne!(self.phase, MatchPhase::WeaponSelection, "after_frame runs after a match tick, not during weapon selection");`

(2) `input.rs`, after `impl Recorder`:

```rust
/// Step 4½c (design §7.2, Q5): the release latch at both phase boundaries. C++ keys are EDGES:
/// a key held when the controller starts never reaches the worm, and a key held when weapon
/// selection ends (Fire from DONE) does nothing in the match until pressed again
/// (`ReleaseControls`, `game.cpp:110-118`; SDL repeats are dropped, `gfx.cpp:608`). Rust samples
/// LEVELS, so without this a held DONE would fire the first weapon on tick 0. Armed with the
/// held words at a boundary; each tick it forgets released bits (`mask &= held`) and outputs
/// `sampled & !mask`. It sits BEFORE the recorder tap; it is live-only, and only selection
/// boundaries arm it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReleaseLatch {
    mask: [u32; N_WORMS],
}

impl ReleaseLatch {
    /// Latch every bit held now.
    pub fn arm(&mut self, held: &[ControlState; N_WORMS]) {
        self.mask = held.map(|c| c.pack());
    }

    /// Mask this tick's sampled words in place.
    pub fn apply(&mut self, inputs: &mut [ControlState; N_WORMS]) {
        for (mask, input) in self.mask.iter_mut().zip(inputs.iter_mut()) {
            *mask &= input.pack();
            *input = ControlState::unpack(input.pack() & !*mask);
        }
    }

    /// Whether any bit is still latched.
    pub fn is_armed(&self) -> bool {
        self.mask.iter().any(|&m| m != 0)
    }
}
```

(3) `web_params.rs`. In the module doc, replace "without menus (4½d) or a weapon-selection phase (4½c), so the page URL picks" with "without menus (4½d), so the page URL picks", and add after the example block: `A bare preview opens on weapon selection (4½c); \`weapons=\` skips it (John's Q3 ruling) and keeps the pre-4½c path exactly.` Add to `impl MatchParams`:

```rust
    /// Q3 (Step 4½c): `?weapons=` naming at least one weapon skips weapon selection — the
    /// preview reaches the thing under test in one click. Without it the match opens on the
    /// selection screen.
    pub fn skips_weapon_selection(&self) -> bool {
        !self.weapons.is_empty()
    }
```

(4) Create `rust/game/src/selection.rs`:

```rust
//! Step 4½c — the live weapon-selection phase (design §7), Bevy-free so the tick system's phase
//! logic is headlessly testable (the `lib.rs` rule).
//!
//! C++ runs the phase inside `LocalController` (`localController.cpp:112-152`, `:224-229`): a
//! fresh controller constructs `WeaponSelection`, every `Process` runs the 12/3 key repeat and
//! `ProcessFrame`, and the frame the last player readies calls `Finalize` and enters the game.
//! [`Selection`] owns the running `sim::weapsel::WeaponSelection`, the in-memory picks it starts
//! from and writes back to, and the frozen screen. The picks stand in for the C++
//! `WormSettings` a `shared_ptr` shares with the worms (finding 4): every NEW GAME — here F5 and
//! the post-match restart — starts from them. Seed sourcing and loading a setup are 4½d's.

use render::bitmap::Bitmap;
use render::frame::Scene;
use render::weapsel::{self as screen, WeapselTexts};
use scenario::build::weapsel_config;
use scenario::settings::Settings;
use sim::state::{ControlState, NUM_WEAPONS, SimState};
use sim::weapsel::{WeaponSelection, WeapselConfig, WeapselError, weap_order};

/// `WormSettings::controller` DumbLieroAI (`localController.cpp:20`).
pub const CONTROLLER_BOT: u32 = 1;
/// `Settings::select_bot_weapons` KEEP (`hiddenMenu.cpp:10`): a bot keeps its saved picks and
/// readies at once (`weapsel.cpp:95`).
pub const BOT_WEAPONS_KEEP: u32 = 2;

/// The 1-based `weap_order` picks that reproduce worm `worm`'s launched loadout (design §7.3,
/// Q7): the inverse of `Worm::InitWeapons`. The default match launches DART + four copies of
/// the first weapon by name, i.e. `[12, 1, 1, 1, 1]`.
pub fn loadout_picks(state: &SimState, worm: usize) -> [u32; NUM_WEAPONS] {
    let order = weap_order(&state.weapons);
    state.worms[worm].weapons.map(|w| {
        let id = w.ty.expect("a launched loadout fills every slot") as usize;
        order
            .iter()
            .position(|&i| i == id)
            .expect("every weapon is in weap_order") as u32
            + 1
    })
}

/// What the live match selects from in 4½c (design §7.3, §7.6): `Settings::default()` (every
/// weapon enabled, bots PICK, both human) with each worm's launched loadout as its saved picks.
/// On a touch-only page player 2 has no input, so it becomes a bot with `select_bot_weapons =
/// KEEP`, ready at once (John's Q8 ruling); until 4½f it idles in the match as today. 4½d
/// replaces this with the loaded setup.
pub fn live_config(state: &SimState, touch_only: bool) -> WeapselConfig {
    let mut cfg = weapsel_config(&Settings::default());
    for (i, p) in cfg.players.iter_mut().enumerate() {
        p.weapons = loadout_picks(state, i);
    }
    if touch_only {
        cfg.players[1].controller = CONTROLLER_BOT;
        cfg.select_bot_weapons = BOT_WEAPONS_KEEP;
    }
    cfg
}

/// The presentation state (design §5): the frozen frame, built on the first render of a phase,
/// and `Gfx::menu_cycles`, from 0 per phase (C++ inherits the main menu's; 4½d threads it),
/// incremented once per rendered frame after the draw (`gfx.cpp:1646`).
#[derive(Default)]
struct WeapselScreen {
    frozen: Option<Bitmap>,
    menu_cycles: u32,
}

/// The live phase and the in-memory picks (see the module doc).
pub struct Selection {
    cfg: WeapselConfig,
    /// `WormSettings::name` of players 0/1: `Settings::default()`'s (empty) until 4½f.
    names: [String; 2],
    active: Option<WeaponSelection>,
    screen: WeapselScreen,
}

impl Selection {
    pub fn new(cfg: WeapselConfig) -> Selection {
        Selection {
            cfg,
            names: Default::default(),
            active: None,
            screen: WeapselScreen::default(),
        }
    }

    /// The saved picks and rules every new selection starts from.
    pub fn config(&self) -> &WeapselConfig {
        &self.cfg
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active(&self) -> Option<&WeaponSelection> {
        self.active.as_ref()
    }

    /// `ChangeState(kStateWeaponSelection)` on a freshly loaded tick-0 `state`: a new
    /// `WeaponSelection` from the saved picks (it draws `state.rand`), a new frozen frame, and
    /// `menu_cycles` back to 0.
    pub fn begin(&mut self, state: &mut SimState) -> Result<(), WeapselError> {
        self.active = Some(WeaponSelection::new(state, &self.cfg)?);
        self.screen = WeapselScreen::default();
        Ok(())
    }

    /// One phase tick (design §7.1). Appends the menu sample ids to `sounds`. Returns true on the
    /// frame the last player readies; the selection is then finalized (`InitWeapons` +
    /// `ReleaseControls`), its picks written back, and it is no longer active.
    pub fn step(
        &mut self,
        state: &mut SimState,
        inputs: &[ControlState; 2],
        sounds: &mut Vec<i32>,
    ) -> bool {
        let ws = self.active.as_mut().expect("step needs an active selection");
        let done = ws.process_frame(state, inputs);
        sounds.extend_from_slice(ws.menu_sounds());
        if done {
            let ws = self.active.take().expect("active");
            let picks = ws.finalize(state);
            self.write_back(picks);
        }
        done
    }

    /// F5 or a restart mid-selection (design §7.4): the running picks — constructor rolls and
    /// every Left/Right included — become the saved picks, as the C++ aliasing makes them
    /// (finding 4). No-op when no selection runs: `step` already wrote the finalized picks back.
    pub fn abandon(&mut self) {
        if let Some(ws) = self.active.take() {
            self.write_back([ws.player(0).picks, ws.player(1).picks]);
        }
    }

    fn write_back(&mut self, picks: [[u32; NUM_WEAPONS]; 2]) {
        for (p, picks) in self.cfg.players.iter_mut().zip(picks) {
            p.weapons = picks;
        }
    }

    /// Draw the selection screen into `surface` (`weapsel.cpp:160-209`): build the frozen frame
    /// on the first call of a phase (`scene` carries the live HUD flags, as `game.Draw` would),
    /// draw the menus with the weapsel palette, then count `menu_cycles`.
    pub fn render(
        &mut self,
        surface: &mut Bitmap,
        state: &SimState,
        scene: &Scene,
        texts: &WeapselTexts,
        level_file: &str,
    ) {
        let ws = self.active.as_ref().expect("render needs an active selection");
        let cycles = self.screen.menu_cycles;
        let frozen = self.screen.frozen.get_or_insert_with(|| {
            screen::build_frozen(state, scene, &screen::level_label(texts, level_file), cycles)
        });
        let pal = screen::weapsel_palette(scene.origpal, cycles);
        let names = [self.names[0].as_str(), self.names[1].as_str()];
        screen::draw_screen(surface, frozen, &pal, scene.font, texts, ws, &state.weapons, names);
        self.screen.menu_cycles = cycles.wrapping_add(1);
    }

    pub fn menu_cycles(&self) -> u32 {
        self.screen.menu_cycles
    }
}

#[cfg(test)]
mod tests {
    use render::bitmap::Bitmap;
    // … Step 1 (d) …
}
```

(If the borrow checker rejects `frozen` borrowing `self.screen` while `self.names` is read, bind `names` before `get_or_insert_with`.)

(5) `lib.rs`: add `pub mod selection;` (alphabetical, after `pub mod match_flow;`). In the doc's list add "since Step 4½c the live weapon-selection phase (`selection`)".

Run: `rustfmt --edition 2024 /home/user/openliero/rust/game/src/selection.rs`
Run: `cd /home/user/openliero/rust && cargo test -p game --lib` — Expected: PASS (the new tests + all prior).

- [ ] **Step 4: Wire `main.rs`**

(1) Module doc: after the first paragraph add: `Step 4½c: a Live match opens on weapon selection (\`game::selection\`, drawn by \`render::weapsel\`); \`--record\` and the preview's \`?weapons=\` skip it.`

(2) Imports: `use game::input::{InputSource, Mode, ParsedArgs, Recorder, ReleaseLatch};`, `use game::selection::Selection;`, `use sim::sound::{LoopKey, SoundEvent};`, `use sim::state::{ControlState, SimState};`.

(3) `Demo`: add after `loadout`:

```rust
    /// Step 4½c: the weapon-selection phase (design §7) — `Some` in `Mode::Live` unless
    /// `--record` or `?weapons=` skips it. Holds the running selection, the in-memory picks
    /// every (re)start selects from, and the frozen screen.
    selection: Option<Selection>,
    /// Step 4½c: the release latch at both phase boundaries (design §7.2). Only a selection
    /// boundary arms it, so a skipped selection never masks a key.
    latch: ReleaseLatch,
```

(4) `setup`: after `apply_loadout(&mut state, &loadout);` insert

```rust
    // Step 4½c (design §7.5, §7.6): a Live match opens on weapon selection unless `--record`
    // (the recording starts at match tick 0 — the C++ skip path) or `?weapons=` skips it.
    if *mode == Mode::Live && record_path.0.is_some() {
        eprintln!(
            "--record: weapon selection is skipped — the recording starts at match tick 0 with \
             the scenario loadout (recording a selection is 4½d)"
        );
    }
    let select =
        *mode == Mode::Live && record_path.0.is_none() && !preview.0.skips_weapon_selection();
    let selection = select.then(|| {
        let mut sel = Selection::new(game::selection::live_config(&state, touch_only()));
        sel.begin(&mut state).expect("the live config selects over the 40-weapon TC");
        sel
    });
    publish_phase(if select { "weapsel" } else { "game" });
```

and in the `Demo { … }` literal: `flow: (*mode == Mode::Live).then(|| if select { MatchFlow::with_weapon_selection() } else { MatchFlow::new() }),`, `selection,` and `latch: ReleaseLatch::default(),`.

(5) Add these helpers (next to `touch_mask`):

```rust
/// This tick's sampled words: the input source plus, in the browser's Live mode, the on-screen
/// controls merged into player 1 (before the latch and the recorder tap).
fn sample_inputs(
    source: &InputSource,
    tick: u32,
    keys: &ButtonInput<KeyCode>,
    mode: Mode,
) -> [ControlState; 2] {
    #[allow(unused_mut)] // only the wasm build merges touch input
    let mut inputs = source.sample(tick, keys);
    #[cfg(target_arch = "wasm32")]
    if mode == Mode::Live {
        inputs[0] = game::touch::merge(inputs[0], touch_mask());
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = mode;
    inputs
}

/// The page's touch-only flag (`window.lieroTouchOnly`, set by `web/index.html` before the game
/// starts): player 2 then has no input, so it becomes a bot that readies at once (Q8). Natively
/// always false.
#[cfg(target_arch = "wasm32")]
fn touch_only() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &"lieroTouchOnly".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn touch_only() -> bool {
    false
}

/// Publish the live phase as `window.lieroPhase` (`"weapsel"` / `"game"`) for the page and the
/// headless browser check. A no-op natively.
fn publish_phase(phase: &str) {
    #[cfg(target_arch = "wasm32")]
    let _ = js_sys::Reflect::set(&js_sys::global(), &"lieroPhase".into(), &phase.into());
    #[cfg(not(target_arch = "wasm32"))]
    let _ = phase;
}
```

(6) `tick_and_render`:
- The F5 block becomes

```rust
    if (*mode == Mode::Live || *mode == Mode::Replay) && keys.just_pressed(KeyCode::F5) {
        let held = sample_inputs(&source, demo.tick, &keys, *mode);
        restart_match(&mut sim.0, &mut demo, recorder.as_deref_mut(), &held);
        render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);
        return;
    }
```

- Directly after it, the phase:

```rust
    // Step 4½c: the weapon-selection phase (design §7.1). One sample, the latch, then the REAL
    // `sim::weapsel` step on the sim (LD 3: still the only `Sim` mutator), the menu sounds, and
    // the flow's fade. No `process_frame`, no viewport stepping, no sim sound drain, and
    // `cycles` does not advance, as in C++. No recorder tap: a recorded run never selects.
    if demo.selection.as_ref().is_some_and(Selection::is_active) {
        let sampled = sample_inputs(&source, demo.tick, &keys, *mode);
        let mut inputs = sampled;
        demo.latch.apply(&mut inputs);
        let mut sounds = Vec::new();
        let done = demo
            .selection
            .as_mut()
            .expect("checked above")
            .step(&mut sim.0, &inputs, &mut sounds);
        let events: Vec<SoundEvent> = sounds.into_iter().map(SoundEvent::one_shot).collect();
        audio.0.drain(&events);
        if let Some(flow) = demo.flow.as_mut() {
            flow.weapsel_frame();
            if done {
                flow.enter_game();
            }
        }
        if done {
            // ChangeState(kStateGame): keys still held (Fire from DONE) wait for a new press.
            demo.latch.arm(&sampled);
            publish_phase("game");
        }
        render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);
        return;
    }
```

- In the `if !replay_finished {` block, replace the sampling + touch merge (`let mut inputs = source.sample(demo.tick, &keys);` through the `#[cfg(target_arch = "wasm32")]` merge) with

```rust
        let sampled = sample_inputs(&source, demo.tick, &keys, *mode);
        let mut inputs = sampled;
        // Step 4½c (design §7.2): the release latch, BEFORE the recorder tap, so a recording
        // replays exactly. Only a selection boundary arms it, and a recorded run has none, so
        // recordings and the Scripted/Replay feeds are unchanged.
        demo.latch.apply(&mut inputs);
```

Keep the comment paragraphs that described the sampling, and move the touch-merge comment into `sample_inputs`.
- The match-end restart becomes `restart_match(&mut sim.0, &mut demo, recorder.as_deref_mut(), &sampled);`.

(7) `render_and_upload`: replace the `render::frame::draw(…)` line with

```rust
    match demo.selection.as_mut().filter(|s| s.is_active()) {
        // Step 4½c: the weapon-selection screen over a frozen `frame::draw` (render::weapsel).
        Some(sel) => sel.render(
            &mut demo.surface,
            sim,
            &scene,
            &demo.scene.weapsel_texts,
            &demo.scenario.level,
        ),
        None => render::frame::draw(&mut demo.surface, sim, &mut demo.viewports, &scene),
    }
```

(8) `restart_match` gains `held: &[ControlState; 2]` and becomes

```rust
fn restart_match(
    sim: &mut SimState,
    demo: &mut Demo,
    recorder: Option<&mut Recorder>,
    held: &[ControlState; 2],
) {
    if let Some(recorder) = recorder {
        recorder.clear();
    }
    // Step 4½c (design §7.4): a running selection's picks become the saved picks (finding 4).
    if let Some(sel) = demo.selection.as_mut() {
        sel.abandon();
    }
    let loaded = scenario::load(Path::new(TC_ROOT), &demo.scenario);
    *sim = loaded.state;
    apply_loadout(sim, &demo.loadout);
    demo.viewports = loaded.viewports;
    demo.scene = loaded.scene;
    demo.tick = 0;
    if let Some(sel) = demo.selection.as_mut() {
        // The C++ NEW GAME loop with the menu left out: a NEW selection from the saved picks.
        // The scenario's seed is kept, so a restart stays deterministic (seeding is 4½d's).
        sel.begin(sim).expect("the live config selects over the 40-weapon TC");
        demo.latch.arm(held);
        demo.flow = Some(MatchFlow::with_weapon_selection());
        publish_phase("weapsel");
    } else if demo.flow.is_some() {
        demo.flow = Some(MatchFlow::new());
    }
}
```

Update its doc: `… and, in Live with selection, a new weapon selection from the written-back picks (Step 4½c).`

Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS (unit tests + `passthrough`, `round_trip`, `record_regression`, `viewport_stepping` unchanged).
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown` — Expected: builds (this is the only check for the `#[cfg(target_arch = "wasm32")]` code).
Run: `cd /home/user/openliero/rust && cargo clippy -p game --all-targets 2>&1 | tail -20` (if clippy is installed) — Expected: no new warning in the touched code.

- [ ] **Step 5: The page and the preview comment**

(1) `web/index.html`:
- In the top comment, after `keyboard; the URL picks the match (game::web_params):` add a line: `It opens on weapon selection (Step 4½c); ?weapons= skips it.`
- Right after the `const wantTouch = …;` statement (before `if (wantTouch) setUpTouch();`), add

```js
      // Step 4½c: a touch-only page has no keyboard for player 2, so the game makes player 2 a
      // bot that is ready at once in weapon selection (rust/game/src/selection.rs). ?touch=1
      // emulates a phone, so it counts as touch-only; a touch laptop (a fine pointer too) does not.
      window.lieroTouchOnly = wantTouch &&
        (touchParam === '1' || !matchMedia('(any-pointer: fine)').matches);
```

- `#touch-hint` text: `Pick: pad ↑/↓ line, ←/→ weapon, FIRE on DONE! · Play: move ←/→, aim ↑/↓ · you are the blue worm, player 2 is a bot`.
- In `#help`, insert before `Dig: hold left + right ·`:

```html
      <b>Weapon selection</b> first, both players: up/down picks a line · left/right changes the
      weapon · fire on <i>Randomize</i> re-rolls all five · fire on <i>DONE!</i> when ready
      <br />
```

  and change the `URL:` span's first item to `<code>?weapons=MISSILE,LASER,BIG%20NUKE</code> (skips weapon selection)`, and `<kbd>F5</kbd> restarts the match` to `<kbd>F5</kbd> restarts (back to weapon selection)`.

(2) `.github/workflows/preview.yml`, in the comment `body` array:
- The first example becomes `` `- [MISSILE, LASER, BIG NUKE, BOOBY TRAP, LARPA — skips weapon selection](${url}/?weapons=MISSILE,LASER,BIG%20NUKE,BOOBY%20TRAP,LARPA)`, ``.
- Insert before the `Keys:` line: `` `The match opens on **weapon selection**: up/down picks a line, left/right changes the weapon, fire on Randomize re-rolls all five, fire on DONE! when ready (both players). \`weapons=\` skips it.`, ``
- `URL parameters: \`weapons\` (up to 5 names)` becomes `URL parameters: \`weapons\` (up to 5 names; skips weapon selection)`.
- `F5 restarts.` becomes `F5 restarts (back to weapon selection).`
- The phone line becomes `` `On a phone or tablet: on-screen pad + FIRE/JUMP/WEAPON/DIG drive player 1, and player 2 is a bot that is ready at once (\`?touch=1\` forces them on a desktop).`, ``

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('/home/user/openliero/.github/workflows/preview.yml')); print('yaml ok')"` — Expected: `yaml ok`. (If PyYAML is missing, run `ruby -ryaml -e "YAML.load_file('/home/user/openliero/.github/workflows/preview.yml'); puts 'yaml ok'"`, or read the diff line by line.)
Run: `cd /home/user/openliero && git diff --stat -- web/index.html .github/workflows/preview.yml` — Expected: only those two files, with small hunks.

- [ ] **Step 6: The re-diff**

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown` — Expected: builds.
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 7: Commit**

```
git -C /home/user/openliero add rust/game/src/match_flow.rs rust/game/src/input.rs rust/game/src/web_params.rs rust/game/src/selection.rs rust/game/src/lib.rs rust/game/src/main.rs web/index.html .github/workflows/preview.yml
git -C /home/user/openliero commit -m "game(4.5c): the live weapon-selection phase — MatchPhase, release latch, write-back on restart, --record/?weapons= skips, touch-only P2 bot" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

Reviewer (Opus): `tick_and_render` stays the only `Sim` mutator (LD 3); in the phase, neither `process_frame` nor the viewport stepping runs; the latch sits before the recorder tap and is armed only at selection boundaries; `--record` never selects, so `round_trip.rs`/`record_regression.rs` are untouched; `restart_match` abandons (writes back) before reloading; the touch rule reads the page flag once at startup; the wasm build compiles the `cfg(wasm32)` paths.

---
### Task 10: Headless Chromium — the selection flow on a desktop and an emulated phone  [Sonnet] (nice-to-have)

**Files:**
- Create (scratchpad only, NOT committed): `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/weapsel.mjs`, `…/scratchpad/site10/`

**Interfaces:**
- Consumes: T9's page globals `window.lieroPhase` and `window.lieroTouchOnly`; the bundle recipe in `web/index.html`'s comment; Playwright under `/opt/node22/lib/node_modules/` (the existing `smoke.mjs`/`touch.mjs` in the scratchpad use it).

Why: the preview is how John and reviewers meet this slice. Unit tests cover `game::selection`, but only the browser proves the page flag, the touch pad and the phase flow end to end: bare page → selection → both DONE → match; F5 back to selection; `?weapons=` skips it; on a phone, player 1 alone ends it. If Chromium or Playwright is unavailable, skip this task and say so in the done-report. It never blocks T11.

- [ ] **Step 1: Build and serve the preview bundle** (the preview's own profile; this is the artifact CI deploys, not a test build)

Run: `cd /home/user/openliero/rust && cargo build -p game --profile wasm-release --target wasm32-unknown-unknown`
Run: `mkdir -p /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/site10 && wasm-bindgen --out-dir /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/site10 --out-name game --target web --remove-name-section --remove-producers-section /home/user/openliero/rust/target/wasm32-unknown-unknown/wasm-release/game.wasm && cp /home/user/openliero/web/index.html /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/site10/`
Run (in the background): `python3 -m http.server --directory /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/site10 8090`

- [ ] **Step 2: Write** `/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/weapsel.mjs`:

```js
// Step 4½c T10 — headless check of the weapon-selection flow in the preview bundle.
// Usage: node weapsel.mjs <base-url> <outdir>
import { createRequire } from 'module';
const require = createRequire('/opt/node22/lib/node_modules/');
const { chromium, devices } = require('playwright');

const [base, out] = process.argv.slice(2);
const browser = await chromium.launch({
  args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader',
         '--autoplay-policy=no-user-gesture-required'],
});
let failed = false;
const check = (name, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name} ${detail}`);
  if (!ok) failed = true;
};
const phase = (page) => page.evaluate(() => window.lieroPhase);
const reaches = (page, want, ms) =>
  page.waitForFunction((w) => window.lieroPhase === w, want, { timeout: ms }).then(() => true, () => false);
const tap = async (page, key) => {
  await page.keyboard.down(key);
  await page.waitForTimeout(120); // < 171 ms: no key repeat (12 frames at 70 Hz)
  await page.keyboard.up(key);
  await page.waitForTimeout(120);
};
const errorsOf = (page) => {
  const errors = [];
  page.on('pageerror', (e) => { if (!String(e).includes('control flow')) errors.push(String(e)); });
  return errors;
};

{ // 1. Desktop: the bare page opens on weapon selection; both DONE start the match; F5 returns.
  const page = await browser.newPage({ viewport: { width: 1000, height: 780 } });
  const errors = errorsOf(page);
  await page.goto(base + '/');
  await page.waitForSelector('canvas', { timeout: 60000 });
  check('desktop: opens on weapon selection', await reaches(page, 'weapsel', 30000));
  check('desktop: touch-only is off', (await page.evaluate(() => window.lieroTouchOnly)) === false);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${out}/desktop_weapsel.png` });
  await page.click('canvas');
  await tap(page, 'KeyR');        // P1 Up: RANDOMIZE -> DONE!
  await tap(page, 'ArrowUp');     // P2 Up
  await tap(page, 'ControlLeft'); // P1 ready
  check('desktop: one DONE is not enough', (await phase(page)) === 'weapsel');
  await tap(page, 'ControlRight'); // P2 ready
  check('desktop: both DONE start the match', await reaches(page, 'game', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${out}/desktop_game.png` });
  await page.keyboard.press('F5');
  check('desktop: F5 returns to weapon selection', await reaches(page, 'weapsel', 5000));
  check('desktop: no page errors', errors.length === 0, errors.slice(0, 2).join(' | '));
  await page.close();
}

{ // 2. ?weapons= skips selection (Q3).
  const page = await browser.newPage({ viewport: { width: 1000, height: 780 } });
  await page.goto(base + '/?weapons=BAZOOKA');
  await page.waitForSelector('canvas', { timeout: 60000 });
  check('?weapons= skips weapon selection', await reaches(page, 'game', 30000), `phase=${await phase(page)}`);
  await page.close();
}

{ // 3. Phone: player 2 is a bot that is ready at once (Q8); the pad + FIRE end the phase.
  const ctx = await browser.newContext({ ...devices['iPhone 13'], viewport: { width: 844, height: 390 },
                                         hasTouch: true, isMobile: true });
  const page = await ctx.newPage();
  const errors = errorsOf(page);
  await page.goto(base + '/');
  await page.waitForSelector('canvas', { timeout: 60000 });
  check('phone: opens on weapon selection', await reaches(page, 'weapsel', 30000));
  check('phone: touch-only is on', (await page.evaluate(() => window.lieroTouchOnly)) === true);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${out}/phone_weapsel.png` });
  const pad = await page.locator('#pad').boundingBox();
  const cx = pad.x + pad.width / 2, cy = pad.y + pad.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  await page.mouse.move(cx, cy - pad.height * 0.45, { steps: 3 }); // pad Up: RANDOMIZE -> DONE!
  await page.waitForTimeout(120);
  await page.mouse.up();
  await page.waitForTimeout(150);
  const fire = await page.locator('#b-fire').boundingBox();
  await page.mouse.move(fire.x + fire.width / 2, fire.y + fire.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(120);
  await page.mouse.up();
  check('phone: pad Up + FIRE start the match', await reaches(page, 'game', 5000), `phase=${await phase(page)}`);
  await page.waitForTimeout(1000);
  await page.screenshot({ path: `${out}/phone_game.png` });
  check('phone: no page errors', errors.length === 0, errors.slice(0, 2).join(' | '));
  await ctx.close();
}

await browser.close();
process.exit(failed ? 1 : 0);
```

- [ ] **Step 3: Run it**

Run: `mkdir -p /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/shots10 && cd /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad && node weapsel.mjs http://127.0.0.1:8090 /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/shots10`
Expected: every line `ok`, exit 0. Look at `desktop_weapsel.png` and `phone_weapsel.png` (Read tool): the selection screen, full canvas, with the on-screen controls on the phone. A `FAIL` names the step. `touch-only is on` failing means the emulated page reports a fine pointer; relax the page rule to `wantTouch && (touchParam === '1' || matchMedia('(pointer: coarse)').matches)`, commit that page fix on its own (`web(4.5c): touch-only = a coarse primary pointer`, same trailers), rebuild the bundle (Step 1), and re-run. Any other FAIL is a T9 bug: report it with the phase value.

- [ ] **Step 4: Stop the server** (kill the background job). Paste the `ok`/`FAIL` lines into the done-report. There is no commit unless Step 3 needed the page fix.

---

### Task 11: Full re-diff, wasm, tripwires, PROGRESS + overview + map corrections, broad review  [Opus review]

**Files:**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md` (the header "Last updated" paragraph `:11-35`; the Step 4½ tree's 4½c line `:818-819`; "Open for John" `:845-850`; the Step 4½ headline `:781`, `:454`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md` (status line `:3`; the 4½c bullet `:275-285`; the 4½d bullet `:286-293`)
- Modify: `docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md` (the status line `:3`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` (`:260`, `:434`), `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (`:73`)
- Modify: `.claude/skills/liero-shot/SKILL.md` (§7)

Re-read each doc immediately before editing it: the line numbers are from plan time.

- [ ] **Step 1: The full green board (DEBUG)**

Run: `cd /home/user/openliero/rust && cargo test --workspace --exclude game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo test -p game` — Expected: PASS.
Run: `cd /home/user/openliero/rust && cargo build -p game --target wasm32-unknown-unknown` — Expected: builds.
Run: `cd /home/user/openliero/rust && cargo tree -p sim-core --depth 1` — Expected: `sim-core` with no dependencies.
Run: `cd /home/user/openliero/rust && cargo tree -p sim --depth 1 -e normal` — Expected: only `assets` and `sim-core`.
Run: `cd /home/user/openliero/rust && cargo tree -p render -e normal | grep -c bevy` — Expected: `0`.

- [ ] **Step 2: Tripwires and audits**

Run: `awk '/#\[cfg\(test\)\]/{exit} /unwrap\(|expect\(|panic!|todo!|unimplemented!/' /home/user/openliero/rust/sim/src/weapsel.rs` — Expected: no output. Every refusal is a `WeapselError`, never a panic (design §4.6).
Run: `grep -nE "HashMap|HashSet|f32|f64" /home/user/openliero/rust/sim/src/weapsel.rs` — Expected: no output.
Run: `git -C /home/user/openliero diff --name-status 6abda0d -- rust/oracle-tests/golden | grep -v '^A'` — Expected: no output.
Run: `git -C /home/user/openliero diff --name-status 6abda0d -- rust/oracle-tests/golden | wc -l` — Expected: `50`.
Run: `git -C /home/user/openliero diff --name-only 6abda0d -- src` — Expected: exactly `src/tools/oracle_dump/sim_physics_dump.cpp`, `src/tools/oracle_dump/weapsel_drive.hpp`, `src/tools/oracle_dump/weapsel_dump.cpp`.
Run: `git -C /home/user/openliero diff 6abda0d -- CMakeLists.txt` — Expected: the two `oracle_dump_weapsel` lines, nothing else.
Run: `git -C /home/user/openliero diff --name-only 6abda0d -- rust/sim rust/sim-core` — Expected: `rust/sim-core/src/rng.rs`, `rust/sim/src/lib.rs`, `rust/sim/src/weapsel.rs`.
Run (each file): `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/weapsel_drive.hpp` (and `weapsel_dump.cpp`, `sim_physics_dump.cpp`) — Expected: no output.
Run: `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 6abda0d` — Expected: exit 0.
Reproducibility — regenerate every 4½c golden once more:
Run: `source /tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad/env.sh && bash /home/user/openliero/rust/oracle-tests/gen_weapsel_golden.sh && bash /home/user/openliero/rust/oracle-tests/gen_sim_slice4_5c_golden.sh`
Run: `git -C /home/user/openliero status --porcelain -- rust/oracle-tests/golden` — Expected: no output.
Optional native smoke: `cd /home/user/openliero/rust && xvfb-run -a timeout 20 cargo run -p game; echo "exit $?"`. Expected: `exit 124` (ran until the timeout). If it exits early because the container has no GPU adapter, note that and move on; T10 covers the live path.

- [ ] **Step 3: `liero-shot` §7** — in `.claude/skills/liero-shot/SKILL.md`, §7.1 (Live play), add a paragraph:

```
**Weapon selection (Step 4½c).** A Live match opens on the C++ weapon-selection screen (`sim::weapsel`,
drawn by `render::weapsel`): up/down picks a line (Randomize, five slots, DONE!), left/right changes the
weapon (held keys repeat at 12 then every 3 frames), fire on Randomize re-rolls all five (it draws the
SIM RNG), fire on DONE! readies — both players. F5 and the post-match restart go back to selection with
the picks kept. `--live --record` skips it (stderr says so) and the browser's `?weapons=` skips it; a
touch-only page makes player 2 a bot that is ready at once. Headless: `cargo run -p shot -- --weapsel
--scenario-path <settings scenario> --out x.png` renders the initial screen on the faithful path
(`new_match`, invisible worms); the goldens are `oracle-tests/golden/weapsel_*` (C++
`oracle_dump_weapsel`, self-checked against a real LocalController).
```

- [ ] **Step 4: PROGRESS** — set "Last updated" to the real current date. Prepend a header paragraph (the previous one becomes "Prior (…)"): **🗡️ 4½c (the weapon selection phase) LANDED.**

The paragraph says:
- `sim::weapsel` ports the constructor (finding 1: the loop only for disabled picks), `process_frame` with LocalController's 12/3 repeat on sampled words (finding 5: Local, not Rollback), RANDOMIZE (finding 7), the crossed menu sounds, `Finalize`, and the refusals (finding 8).
- The builder split `new_match`/`enter_game`/`weapsel_config`, with `build_match` byte-identical.
- The oracle-only `weapsel` directive.
- `oracle_dump_weapsel`: the REAL WeaponSelection behind a replica of LocalController's input plumbing, self-checked against a real LocalController.
- The 16 goldens are line for line vs C++ and reach every §6.7 witness; the two continuation goldens are bit-exact over 600 ticks with a non-zero tick-0 rng; handoff equality holds on 48 random configurations.
- The pixel-exact screen was pulled forward (Q1): `draw_rounded_box`, `get_dims`, the MenuItem text arm, the frozen frame. Three self-goldens.
- The live phase: the latch, write-back on F5/restart, `--record` and `?weapons=` skips (Q3), touch-only P2 bot (Q8).
- The 8 settings-path goldens + 2 classic regenerated byte-identically; no prior golden changed; the T10 result.

In the Step 4½ tree replace the 4½c lines with

```
├─ ✅ 4½c  weapon selection phase — sim::weapsel (constructor RNG loops, 12/3 LocalController key repeat,
│          RANDOMIZE, crossed menu sounds, Finalize, refusals) + builder split new_match/enter_game;
│          oracle_dump_weapsel (real WeaponSelection, LocalController self-check); 🎯 16 goldens line for
│          line + 2 continuations bit-exact + handoff equality; pixel-exact screen pulled forward
│          (DrawRoundedBox, GetDims, MenuItem text arm); live phase + latch + ?weapons=/--record skip +
│          touch-only P2 bot                                                            COMPLETE
```

update the Step 4½ headlines (`4½c ✅`), and under "Open for John" add:
- the eyeball of the weapon-selection screen against the C++ build is still owed. This container cannot capture the C++ GUI. The Rust PNG comes from `shot --weapsel`. Known caveats: SetWormColour is unported (name colours 33/42 differ slightly), C++ draws random names, and there is no fade (4½d).
- 4½c ports LocalController's key repeat; C++ netplay's RollbackController repeats differently (finding 5, `repeat_edge` pins the choice). Step 5 runs Rust on both peers.

- [ ] **Step 5: The overview** — status line: `**4½c LANDED**` after `**4½c-0 LANDED**`, and `4½d–4½h planned`. At the end of the 4½c bullet append:

"**Landed** (design `specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md`, plan `plans/2026-09-25-liero-rs-step4.5-slice4.5c-plan.md`). Corrections from its findings:
(1) the constructor's rejection loop runs only when a saved or rolled pick is DISABLED, and checks uniqueness only inside that loop; the "redraw until enabled and unused" description fits RANDOMIZE;
(2) PICK does not make the bot navigate: the AI never runs during selection, and a PICK bot's menu is driven by the keys bound to that worm;
(3) the phase uses LocalController's 12/3 repeat (RollbackController's differs, finding 5).
**Step 5 note:** the weapon-select snapshot is `SimState` + a `WeaponSelection` clone (plain data; `Rand: Clone` landed). Netplay must zero controllers before selection (`rollbackController.cpp:401`) and finalize on the confirmed done frame (design §8)."

In the 4½d bullet, replace "`DrawRoundedBox` (missing from `render`, rust-map §2)" with "`DrawRoundedBox`, `Font::GetDims` and the `MenuItem::Draw` text arm (landed in 4½c: `render::blit`, `render::font`, `render::menu`)", and "(`gfx.cpp:978-1006`; `Palette::RotateFrom` is missing from `render::palette`)" with "(`gfx.cpp:978-1006`; `render::palette::rotate_from` exists; 4½c's `render::weapsel::weapsel_palette` uses it)".

- [ ] **Step 6: The maps and the design**

- cpp-map `:260`: append ` (**Corrected by the 4½c design, finding 1:** the constructor's loop runs only for a DISABLED pick and checks uniqueness only inside it; the "until enabled and unused" wording describes RANDOMIZE, weapsel.cpp:316-337.)`
- cpp-map `:434`: replace "PICK(1) makes the bot navigate the menu" with "PICK(1) leaves the bot not ready, its menu driven by the keys bound to that worm (the AI never runs during selection; 4½c design finding 2)".
- rust-map `:73`: replace "**Missing:** `Palette::RotateFrom(orig, 168, 174, cycles)` (menu water rotation)." with "`render::palette::rotate_from` already exists (`palette.rs:13`; 4½c design finding 10); 4½c adds `render::weapsel::weapsel_palette` over 168..174."
- design `:3`: `Status: **LANDED** · 2026-09-25 · …` (keep the rest; add `plan: plans/2026-09-25-liero-rs-step4.5-slice4.5c-plan.md`).

- [ ] **Step 6b: Stage** — `git -C /home/user/openliero status --short` must list only the docs above (and nothing under `rust/`, `src/`, `web/`).

- [ ] **Step 7: Broad review (Opus)** — re-read the whole slice diff (`git -C /home/user/openliero diff 6abda0d -- rust src web .github CMakeLists.txt`) against the design:
- every §0 finding is either ported and golden-reached (map each to its witness or unit test) or recorded as a deferral (§10);
- `sim::weapsel` transcribes `weapsel.cpp:28-97`, `:219-361` and `localController.cpp:128-148` line for line;
- `SimState::new` is unchanged;
- exactly one new directive;
- the dumper's settings path is unchanged without `weapsel` (T5's evidence);
- the latch is before the recorder tap;
- the browser pieces work (T10's result);
- the design-vs-source list at the top of this plan is reflected in PROGRESS;
- no push, no PR.
Bar: 0 Critical / 0 Important.

- [ ] **Step 8: Commit**

```
git -C /home/user/openliero add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-rust-baseline-map.md .claude/skills/liero-shot/SKILL.md
git -C /home/user/openliero commit -m "docs(4.5c): PROGRESS + overview + maps — slice 4.5c landed (weapon selection phase)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_019Mcsj34x9n7QgLzRg1QGxT"
```

## Addendum A (2026-09-25, John's ruling): the menu is gated against C++ pixels HERE, not on macOS

John ruled that the C++ comparison of the weapon-selection screen must happen in this cloud
session, not be left for a macOS eyeball. Two mechanisms replace the "self-goldens + eyeball on
macOS" parts of T8/T10. They override the plan text wherever the two disagree.

**A1 — a C++ frame gate (folded into T5 and T8; the primary gate).** `WeaponSelection::Draw`
(`weapsel.cpp:211-216`) renders into a `Renderer` bitmap, not a window; it depends on the global
`gfx` for `play_renderer` / `single_screen_renderer`, `frozen_screen`, `menu_cycles` and
`settings` (`weapsel.cpp:20-24`, `:99-209`). The existing `render_live` path of
`sim_physics_dump.cpp` already builds a headless `Renderer` and hashes real C++ frames.
- **T5** extends `oracle_dump_weapsel` with an opt-in render mode: set up exactly the `gfx` state
  `Draw` reads (renderers allocated headlessly, `settings`, `menu_cycles` advanced as
  `Gfx::Process` would, `frozen_screen` empty at the phase start), call the real
  `WeaponSelection::Draw(gfx.play_renderer, state, /*use_spectator_viewports=*/false)` after each
  `process_frame` step, and write per frame `<frame> <frame_hash16>` (the same `hash_frame` the
  render goldens use, incl. the palette the frame is shown with) to a sidecar, plus an optional
  `--png-dir` that writes each frame as a PNG (or PPM, whichever needs no new dependency) for
  eyeballing. If `Draw` needs state that cannot be set up headlessly, T5 **stops and reports** the
  exact blocker instead of weakening the gate.
- **T8** gates the Rust weapon-selection screen **bit-exact against those C++ frame hashes**
  (every frame of every render case), not against Rust self-goldens. The design's known
  differences (the worm-colour palette step, the random player names) must be either ported or
  pinned identically on both sides — decide in T5/T8, record the choice, never paper over a
  mismatch by regenerating. T8 also writes a side-by-side PNG (C++ | Rust) of a few frames to the
  session scratchpad for the controller to show John.

**A2 — the real C++ game on a virtual display (a new task T10b, after T9; eyeball only).** Build
the full `openliero` target (`cmake --build build/linux-x64 --config Release --target openliero`,
with `env.sh` sourced), run it under `Xvfb :99 -screen 0 1280x800x24` with `SDL_VIDEODRIVER=x11
SDL_AUDIODRIVER=dummy`, drive it with `xdotool` (apt-install it if missing) to NEW GAME → weapon
selection, move the cursor / cycle a weapon / RANDOMIZE, and capture screenshots (`import -window
root` or `xwd | convert`). Pair them with the Rust build of the same moment (the `game` binary under
the same Xvfb, or `shot --weapsel`) into side-by-side PNGs in the scratchpad. Not a gate (window
scaling and timing differ) — a human-eyeball artifact for John.

## Done-report (each task)

Each task reports:
- (a) what changed and why;
- (b) the files touched;
- (c) the tests run, with results, and any risks.

One commit per task (none for T10), on `claude/cpp-oracle-vcpkg-assets-chcwcm`; no push, no PR. The final report surfaces:
- the draw-count evidence (T1 unit tests; T5's scratch case);
- the regeneration proof for the edited dumper (T5: 8 settings-path + 2 classic goldens byte-identical);
- the self-check outcome (T5/T6: LocalController agreed on every frame, or the documented headless fallback);
- the chosen seeds and whether `randomize_held`'s seed moved (T6);
- the milestone result (T7: 16 goldens, 2 continuations, handoff on 48 random configurations);
- the pinned self-golden hashes, the eyeball notes and the pixel caveats (T8);
- the T10 browser lines;
- the tripwire sweep and the golden audit (T11: 50 `A` lines, no `M`).
