# Step 4½, Slice 4½f-1: DumbLieroAI (the CPU player), per-player health, persisted `reacts`, the phone's player 2 as the real CPU, the FollowAI refusal, `?cpu=` (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN). This plan names types, fields, files and behaviour; it does not write the code. Where it quotes C++ it quotes the source, and the source wins over this text.

**Goal.** The original's CPU opponent plays in Rust, exactly as C++ `DumbLieroAI` plays it:
- a `liero.cfg` with `controller = 1` makes that player the CPU, natively and in the browser;
- on a touch-only page player 2 **is** the CPU: it gets random weapons every match, fights back and respawns by itself, and the 4½c stand-in (`game::touch::BotRespawn`) is gone (John's Q3);
- `?cpu=1` makes player 2 a CPU in the preview, `?cpu=2` both players;
- each player has its own health in the sim, so a C++ `liero.cfg` with unequal healths boots and plays (the 4½a interim refusal and the boot's sanitising copy go);
- a match that would start with an "AI" (FollowAI) player shows `AI PLAYERS ARE NOT SUPPORTED YET` and the menu stays (John's Q2).

The AI's per-tick control words and its RNG are bit-exact against C++ (**G-AI**, a new oracle-only `ai` directive in `oracle_dump_sim_physics`); unequal-health matches are bit-exact on all 12 columns (**G-HP**); and CPU matches driven through the real C++ `Gfx::RunOneFrame` match the Rust shell on every frame, `d` line and state hash (**G2f-1**, 🎯 `shell_cpu_match`).

**Architecture.**
- `sim`:
  - new module `ai`: `DumbLieroAi` (owns only its `Rand`, `mt19937(0x1337)`), `run_ais` (C++'s alternating order), and `AiTrace` (which arm each call took, for unit tests and ledgers; behaviour-free);
  - `state`: `WormState::reacts: [i32; 4]` kept between ticks (not hashed); `WormState::max_health` replaces the `SimState::settings_health` scalar at every read site (not hashed); `SimState::ai_params: [[i32; 7]; 2]` from the TC (not hashed).
- `scenario`:
  - `build`: per-worm `max_health`; `AsymmetricHealth` removed; `apply_live_settings` writes both worms' `max_health`; `ai_params` set by both builders; `refuse_follow_ai` (the Q2 check, called by the menu's NEW GAME gate only, D1);
  - `parser`: the oracle-only `ai` directive (`Scenario::ai()`).
- `render`: the HUD lifebar divides by the worm's own `max_health` (`hud.rs:148`).
- `ui::shell`:
  - `playing::Match` owns `ais: [Option<DumbLieroAi>; 2]`, made at `Match::start` from the match's settings, and runs them after the key edges and before the tick, never in weapon selection;
  - `Match::focus` re-runs `Game::Focus`'s worm-colour ramps from the match's settings (every RESUME, like C++);
  - the RefusalGate's NEW GAME arm refuses FollowAI; the boot no longer equalises healths;
  - the touch rule splits: the **settings** make player 2 a CPU (boot, and again after LOAD SETUP on a touch-only page), and NEW GAME's **selection config** sets BOT WEAPONS to RANDOM.
- `oracle_dump_sim_physics`: the `ai` directive, the `reacts` intervention, four opt-in columns.
- `oracle_dump_shell`: intervention 3′ (zero `reacts` on every new controller), check 3″ (every new `DumbLieroAI`'s RNG equals a fresh `Rand()`), the FollowAI guard. No format change.
- `game`: the touch-only boot makes player 2 a CPU, `BotRespawn` is deleted, `?cpu=`, read-only hooks `window.lieroWorms` / `window.lieroControllers` for the headless walk, the "FIRE to respawn" banner only for a human player 1; the page and the preview comment say "player 2 is the CPU".

**Tech stack.**
- Rust 2021 for `sim-core`, `sim`, `render`, `scenario`, `shot` and `oracle-tests`; Rust 2024 for `ui` and `game`.
- C++ in `src/tools/oracle_dump/sim_physics_dump.cpp` and `src/tools/oracle_dump/shell_dump.cpp` only, preset `linux-x64`, clang-format 22.
- HTML/JS in `web/index.html`; the preview comment in `.github/workflows/preview.yml`.

**Spec:** `docs/superpowers/specs/2026-09-27-liero-rs-step4.5-slice4.5f-player-menu-ai-design.md`, cited as **design §N**; its findings are **finding N**; its 4½f-1 task outline is design §8. The precedent plans are `plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md` (**e-1 plan**; its addenda **e-1 Addendum T0 / G3**) and `plans/2026-09-26-liero-rs-step4.5-slice4.5e2-plan.md` (**e-2 plan**; **e-2 Addendum T0**).

**Rulings (John, 2026-09-27, design "Rulings").** All five recommendations were accepted. This part executes:
- **Q1 = A.** Two parts, the CPU first. This is part 1 (4½f-1); the player menu, profiles, key capture and names are 4½f-2. Each part has its own milestone, PR and preview.
- **Q2 = A.** Starting a match with an "AI" (FollowAI) player shows `AI PLAYERS ARE NOT SUPPORTED YET` and the menu stays, unchanged from C++ (the Holdazone approach).
- **Q3 = A.** On a phone, player 2 starts as the real CPU, gets random weapons each match and fights back. The stand-in is removed. (The RIGHT PLAYER menu that shows and changes it is 4½f-2.)
- Q4 (the DIG-binding overflow fix) and Q5 (Joystick profiles) are 4½f-2's.

**Base.** The slice base is **`c2d58fe`**, the merge of 4½e (PR #17) into `liero-rs-step-4-5`. It is used for every golden audit, every clang-tidy diff and every review diff. HEAD at plan time is `341e620` (the design and its rulings), on the draft PR `hamiltoon/openliero#18`.

---

## Working environment (read before any task)

- **Repo and branch.** `/home/user/openliero` on `claude/cpp-oracle-vcpkg-assets-chcwcm` (HEAD `341e620`, pushed; draft PR #18 → `liero-rs-step-4-5` at `c2d58fe`). Use absolute paths. No sub-subagents.
- **Scratchpad.** `S=/tmp/claude-0/-home-user-openliero/b39c30ae-bf21-5647-a4a8-0b8d032b3f3f/scratchpad`. Every scratch script, PPM, PNG, probe, fixture copy and log goes here, never into the repo. This slice's scratch dirs are `$S/t0f1/`, `$S/f1b<N>/` per batch, `$S/f1site/` is **not** used (the bundle stays `$S/site`).
- **The container restarted once during 4½e.** Keep every long command (C++ builds, gen scripts, `cargo test`, the wasm build, the Chromium walks) in the **background** with its output in a log under `$S`, and poll the log. Never rely on a foreground command surviving more than a few minutes.
- **C++ oracle build.**
  - In the same shell as every cmake or gen-script call, first run `source $S/env.sh`. It sets `PRESET=linux-x64`, `VCPKG_ROOT`, the vcpkg asset script and the sdl3 overlay.
  - The build dir is `build/linux-x64` (Ninja Multi-Config, already configured with `-DOPENLIERO_BUILD_ORACLE_DUMP=ON` and `CMAKE_EXPORT_COMPILE_COMMANDS`). Binaries go to `build/linux-x64/Release/`: `oracle_dump_shell`, `oracle_dump_sim_physics`, `oracle_dump_menu`, `oracle_dump_settings`, `oracle_dump_weapsel` and the real `openliero`.
  - Build a target with `cmake --build build/linux-x64 --config Release --target <t>`.
  - The gen scripts default to `macos-arm64` and honour `PRESET`, so always run `source $S/env.sh && bash rust/oracle-tests/<script>.sh` from the repo root.
- **The checked C++ build** (`-D_GLIBCXX_ASSERTIONS -fsanitize=address`, single-config Ninja) is at `$S/build-chk`, with `$S/build-chk/oracle_dump_shell` and `$S/build-chk/oracle_dump_sim_physics`.
  - Rebuild with `source $S/env.sh && cmake --build $S/build-chk --target oracle_dump_shell oracle_dump_sim_physics`.
  - `$S/b7chk/run.sh` is the shell loop template: copy it, never edit it. It runs every `shell_*_script.txt` with `ASAN_OPTIONS=detect_leaks=0` and `cmp`s each output against its golden.
  - The sim gen script takes `CHECKED_DUMPER=$S/build-chk/oracle_dump_sim_physics` (the `gen_sim_slice4_5e_golden.sh` pattern).
  - ASan does **not** see uninitialised reads, so it cannot catch the `reacts` UB (finding 2); the interventions handle that (D5).
- **clang-format is pinned to 22.** The system `clang-format` is 18 and must never be used.
  - Get the binary with `CF22=$(uvx --from clang-format==22.1.0 sh -c 'command -v clang-format')`.
  - Diff check: `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh c2d58fe`.
  - Whole-file check, for every touched C++ file (CLAUDE.md: a diff check misses context): `"$CF22" --dry-run -Werror --style=file <abs file>`.
  - Fix: `"$CF22" -i --style=file <abs file>`.
- **clang-tidy.** `cd /home/user/openliero && scripts/clang-tidy-diff.sh build/linux-x64 c2d58fe`. This clone has no `origin/master`, so always pass the base. It must exit 0. Fix the code. A `NOLINTNEXTLINE(<check>) — <reason>` is allowed only where the repo already uses one for the same check.
- **Rust.** Run everything from the repo root, in **DEBUG only**. Never use `--release`: some tests are `#[should_panic]` on `debug_assert!`s.
  - The re-diff is **both**:
    - `cargo test --manifest-path rust/Cargo.toml --workspace --exclude game`
    - `cargo test --manifest-path rust/Cargo.toml -p game`
  - **NEVER** run `cargo test --workspace` including `game` (disk).
  - The wasm check is `cargo build --manifest-path rust/Cargo.toml --profile wasm-release -p game --target wasm32-unknown-unknown`.
  - Never use `--target-dir` or a second target tree.
- **Disk is tight.** About 15 GB is free at plan time.
  - After every batch run `rm -rf rust/target/debug/incremental rust/target/wasm32-unknown-unknown/*/incremental`, and delete the scratch PPM/PNG dirs you created.
  - Before a wasm-release build, `df -h /` must show at least 3 GB free.
  - **Rust batches run sequentially in this one checkout.** A C++-only batch may run alongside a Rust batch, because its files are disjoint and `build/linux-x64` is not `rust/target`.
- **The native Rust `game` cannot run here.** Bevy panics with "Unable to find a GPU" under Xvfb. The **browser bundle** stands in for live smoke tests:
  - Build it with the wasm check above, then `wasm-bindgen --target web --out-dir $S/site --out-name game --remove-name-section --remove-producers-section rust/target/wasm32-unknown-unknown/wasm-release/game.wasm && cp web/index.html $S/site/ && rm -f $S/site/*.d.ts`.
  - Serve it with `cd $S/site && python3 -m http.server 8765`, run in the background. Check it with `curl -sI http://localhost:8765/`, and restart it if it is down.
  - Playwright lives under `/opt/node22/lib/node_modules/`. The templates are `$S/b8.mjs` (desktop keys through `frameTap`, an emulated phone, the `lieroPhase`/`lieroTop`/`lieroSel` hooks), `$S/e2.mjs` (the e-2 walk), and `$S/tap1.mjs` / `$S/deathdiag2.mjs` (real CDP touch; the death → "FIRE to respawn" diagnosis). Copy one, never edit it.
  - Headless Chromium runs at about 7–10 fps under SwiftShader. **The worms start dead and respawn**, so wait about 30 s after a match starts before any in-match check.
- **The real C++ game under Xvfb** (eyeball artefacts and T0 probes, never gates):
  - `Xvfb :99 -screen 0 1280x800x24 &`, then `DISPLAY=:99 SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy build/linux-x64/Release/openliero [--config-root <dir>] &`.
  - Drive it with `xdotool`: `$S/xd.sh` has `tap` / `snap`; `$S/e2x/drive_cpp.sh` is e-2's key driver.
  - Screenshot with `import -window <id>`. Pair images with `$S/x12_pair.py`, and convert PPMs with `$S/ppm2png.py`.
- **Golden rule.** New goldens come **only from the real C++**. Never regenerate a golden to make Rust pass, and never hand-edit one. Every existing golden must stay byte-identical, audited against `c2d58fe`. If `git status --porcelain -- rust/oracle-tests/golden` ever shows an `M`:
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

Every task below was written against HEAD `341e620`. Where the design and the source disagree, the plan follows the source, and T10 records each item in PROGRESS and the maps. T0 re-checks the facts marked **(T0)** with the real C++.

**The AI (C++).**

1. **The AI seed** (T0 P1). `DumbLieroAI` holds a default-constructed `Rand rand` (`worm.hpp:130-134`); `Rand`'s engine is `std::mt19937 engine{0x1337U}` with `last = 0` (`rand.hpp:15-16`). `CreateAi` builds `new DumbLieroAI()` for controller 1 and `FollowAI` for 2 (`controller/localController.cpp:19-28`), called once per worm in the `LocalController` constructor (`:30-54`, the calls at `:38` and `:45`), i.e. once per NEW GAME per CPU player. The `Seed` calls in the game are `game.rand` from the clock (`game.cpp:42`), `gfx.rand` from the clock (`gameEntry.cpp:23`) and — **one the design did not list** — the F8 weapon randomiser's *local* `Rand r; r.Seed(14)` (`mainMenuState.cpp:465-470`), which is not an AI. So nothing reseeds an AI. Rust `sim_core::rng::Rand::new()` seeds `0x1337` with `last = 0` (`rust/sim-core/src/rng.rs:49-58`), and `Rand::bound` is the same Lemire multiply-shift (`rng.rs:119`, `rand.hpp:30-32`).
2. **`reacts` is uninitialised in C++, and only the AI reads it stale** (T0 P2). `int reacts[4];` has no initialiser (`worm.hpp:260`), the constructor sets only `ready`, `movable`, `killed_timer` (`:178-183`), and `std::make_shared<Worm>()` (`localController.cpp:33`, `:40`; `sim_physics_dump.cpp:583`, `:613`) does not zero it. `CalculateReactionForce` sets `reacts[dir] = 0` and then counts (`worm.cpp:135-144`), and runs only inside `Worm::Process`'s visible branch (`:218-280`). Every other reader (`:152`, `:163-164`, `:196-205`, `:993`) runs later in that same visible `Process`, after the four directions were rewritten. **Only `DumbLieroAI::Process` reads it before any write** (`:680-694`), from the first match tick while the worm still waits to spawn. `reacts` is snapshotted (`serialization/fast_snapshot.hpp:48`, zero default; `cereal_types.hpp:354`) but **not hashed** (`stateHash.hpp:19-50` has no `reacts`).
   - The AI's other reads are initialised: `pos`/`vel` (`math/rect.hpp:11-12`, `T x = T(0)`), `aiming_angle{0}`, `visible{false}`, `direction{0}` (`worm.hpp:226-262`), `ninjarope.out{false}`, `attached{false}` (`worm.hpp:22-23`). `steerable_sum_x/y` are uninitialised too (`worm.hpp:267`), but the AI never reads them.
   - **Rust today:** `reacts` is a tick-local `let mut reacts = [0i32; 4]` (`rust/sim/src/physics.rs:212-213`) computed at `state.rs:2025` and passed to `process_tasks` / `worm_process_physics` (`:2085`, `:2124`).
3. **What one `DumbLieroAI::Process` draws, in order** (`worm.cpp:477-696`). This sharpens design §10's risk list, which called the Fire draw unconditional:
   1. **Fire**: `rand(k[Pressed(kFire)][kFire])` **only** when `kRealDist < max_dist || !worm.visible` (`:513-518`); otherwise, if visible, `Release(kFire)` and **no draw** (`:519-521`).
   2. **Jump**: one draw, always (`:524-527`).
   3. **Change**: one draw, always (`:529-532`).
   4. **Fallback**: `rand(16)` only when the scan found no direction (`dir >= 128`) **and** the quadrant arm is one of the six `64/80/96/48/32/12 + rand(16)` arms (`:557-593`). The fallback has ten arms; the other four — `dir = 80` (x > 0, y < 0, equal magnitudes), `116` (x > 0, y ≥ 0, |x| ≤ |y|), `48` (x ≤ 0, y < 0, equal magnitudes) and `12` (x ≤ 0, y ≥ 0, |x| ≤ |y|; this is also where `real_dist == 0` lands) — draw nothing.
   5. **When Change is pressed** (read again after step 3, `:622`): Left, then Right, one draw each (`:625-631`); then, if `ninjarope.out && ninjarope.attached`, Up then Down, one draw each (`:633-644`); else `Release(kUp)`, `Release(kDown)` (`:647-648`).
   6. **Else** (Change not pressed): no draw. Walk (`:652-659`), aim (`:661-677`), the two `reacts` arms (`:680-694`).

   Every `rand(k)` draws whatever `k` is: `rand(0)` and `rand(1)` both return 0 after drawing, so `== 0` holds and the toggle happens (`rand.hpp:30-32`). `ai_params.k[1]` is the TC's `on`, `k[0]` its `off` (`common_model.hpp:521-531`); the TC's values are up/down 20/120, left/right 20/50, fire 80/80, change 60/300, jump 1/400 (`data/TC/openliero/tc.cfg:85-118`), and Rust's `assets::tc::AiParams::ordered()` (`rust/assets/src/tc.rs:188-201`) is already gated against C++ by `tc_golden.rs:126-129`.
4. **The arithmetic.** `SqrVectorLength(x, y) = x*x + y*y` on the `Ftoi` pixel positions (`worm.cpp:475`, `:480-493`); first worm or strictly closer wins. `max_dist`: `(time_to_explo - time_to_explo_v / 2) * speed / 130` when `0 < time_to_explo < 500`, else `speed - gravity / 10`, then `max(…, 90)` (`:497-506`). `kRealDist = VectorLength(Ftoi(delta))` (integer square root, `math.cpp:7-28`; Rust `sim_core::math::vector_length`, `math.rs:42`). `delta /= kRealDist` is `BasicVec::operator/=` — per-component `int` division, truncating toward zero (`math/rect.hpp:41-45`; Rust `Vec2::div`, `sim-core/src/vec.rs:58`), and `delta.Zero()` when `kRealDist == 0`. The scan is `for (dir = 1; dir < 128; ++dir)` with `abs(cossin_table[dir] - delta) < 0xC00` on both axes (`:544-552`): index 128 is never read.
5. **The driver** (`localController.cpp:122-181`). Only in `kStateGame` / `kStateGameEnded`, per sim step (`frame_skip` defaults to 1, `commonController.hpp:9-10`): `kPhase = game.cycles % 2`; for `i` in `0..worms.size()`, worm `(i + kPhase) % size` runs its AI if it has one (`:156-164`); then `RecordFrame` (`:165-172`); then `game.ProcessFrame()` (`:175`). Weapon selection never runs an AI; the frame that finalises selection runs no tick (`if … else if`, `:123-153`). The Esc fade (`going_to_menu`) and the 180-frame post-mortem are `kStateGame` / `kStateGameEnded`, so the AI runs on every tick the match processes. `AiProcessTime` is stats only (`:159-163`); both dumpers install the base no-op `StatsRecorder`.
6. **A CPU player still receives the key events bound to it.** `Game::FindControlForKey` skips only players whose `input_device` is not the keyboard (`game.cpp:87-106`); it never reads `controller`. `WeaponSelection::Finalize` runs `InitWeapons` then `game.ReleaseControls()` (`weapsel.cpp:350-358`, `game.cpp:110-118`), so every worm, CPU included, starts its first match tick from an empty word.

**CPU weapon selection (C++ and Rust).**

7. **BOT WEAPONS is one global setting, `0` RANDOM / `1` PICK / `2` KEEP** (`menu/hiddenMenu.cpp:10`, `:34`). A bot (`controller != 0`) draws all five picks with `game.rand(1, 41)` in the `WeaponSelection` constructor iff `select_bot_weapons == 0` (`weapsel.cpp:57-61`), and is ready at once iff `select_bot_weapons != 1` (`:95`). The C++ default is `select_bot_weapons{true}` = 1 = **PICK** (`settings.hpp:24`), and both shipped setups say `selectBotWeapons = 1` (`data/Setups/liero.cfg:74`, `orbmit.cfg:74`). Rust ports both rules (`rust/sim/src/weapsel.rs:203`, `:232`), gated by 4½c's `weapsel_bot_{random,pick,keep}` and `weapsel_match_bot`.
8. **A RANDOM bot cannot appear in a shell case.** `oracle_dump_shell`'s intervention 3 reseeds the new game's RNG after the NEW GAME frame and **checks** that the `WeaponSelection` constructor drew nothing (`shell_dump.cpp:20-23`, `:810-833`); the Rust harness refuses the same (`rust/oracle-tests/tests/shell_common/mod.rs:1667-1673`). A RANDOM bot draws there, so the design's G2f-1 `cpu_vs_cpu` "RANDOM" cannot run (D2). A KEEP bot with non-zero, enabled saved picks draws nothing.
9. **The Rust touch rule today** (4½c Q8): `apply_touch_rule` forces player 2's `controller = 1` and `select_bot_weapons = KEEP` in the selection config (`rust/ui/src/shell/selection.rs:66-74`), used by `new_game_config` (`:61-65`), `live_config` (`:50-56`) and the RefusalGate (`overlay.rs:199-206`). The 4½c stand-in `game::touch::BotRespawn` (`rust/game/src/touch.rs:171-203`) is held in `ShellRes` (`main.rs:189-191`, `:585`) and presses FIRE for player 2 (`main.rs:1267-1273`). `waiting_to_respawn` (`touch.rs:167`) feeds `window.lieroRespawn` for player 1 (`main.rs:1321-1326`, `:1509-1514`), which shows the page's "FIRE to respawn" banner (`web/index.html:71-76`, `:160`, `:477-487`).
10. **`live_config` serves only the native `--live <scenario>` path.** On wasm every live run is the shell (`main.rs:531-534`: `Mode::Live && name == DEFAULT_MATCH && no --record`), and `touch_only()` is always `false` natively (`main.rs:1447-1450`). So `live_config`'s touch arm is unreachable in both builds today.

**Health (C++ and Rust).**

11. **The C++ per-worm health read sites** (`grep -n 'settings->health' *.cpp`): `game.cpp:158` (`ResetWorms`), `:558`, `:560`, `:563` (`DoHealingDirect`), `:607` (`DoHealing`); `worm.cpp:213` (the clamp), `:292`, `:296` (the health bonus), `:355` (low-health blood), `:386` (Scales' extra life on death), `:795` (respawn); `viewport.cpp:85` (the lifebar). `spectatorviewport.cpp:784` is unported, and `stats_recorder.cpp:139`, `:142` are 4½g's. **Scales heals the *other* worm with *its own* max:** `DoDamage` calls `DoHealingDirect(*other, k_)` (`game.cpp:566-589`).
12. **The Rust scalar and every site that threads it:** `SimState::settings_health` (`rust/sim/src/state.rs:1231`, default 100 at `:1465`); `do_damage` (`state.rs:511-533`), `do_healing` (`:550-566`), the clamp (`:2008`), respawn (`:2874`), the pre-death drip (`:2921`), Scales' extra life (`:3034`); `bonus::do_healing_direct` (`bonus.rs:417-426`) and the health-bonus pickup (`:509-518`); the threading through `nobject.rs:387`, `:598` and `sobject.rs:128`, `:237`; `render/src/hud.rs:148`; `scenario/src/build.rs:239` (set from player 1, "== [1] (validated)"). **Outside the non-test code:** `sim/src/hash.rs:250` (a test literal), `oracle-tests/tests/{sim_slice5d_golden.rs:287-288, sim_slice5d_fuzz.rs:220, sim_slice6_scales_golden.rs:234, sim_slice4_5e_generated_golden.rs:250}`, and the **frozen** generator `oracle-tests/examples/gen_slice4_5e1_sim.rs:308` (D6).
13. **The refusal and the sanitising copy:** `BuildError::AsymmetricHealth` (`scenario/src/build.rs:46-48`, `:122`), its text arm `BOTH PLAYERS NEED\0THE SAME HEALTH` (`ui/src/text.rs:36-38`), the boot's `bootable` arm (`ui/src/shell/playing.rs:92`), and the tests that pin them (`ui/src/text.rs:491-495`, `ui/src/shell/mod.rs:2521-2534` and `:2717`, `scenario/src/build.rs:400`, `game/src/config.rs:506`).
14. **The sim dumper already starts each worm at its own health** on the `settings` path (`sim_physics_dump.cpp:580-589`, `w->health = w->settings->health`), so G-HP needs no new directive. It applies `input` words **wholesale** (`control_states.Unpack`, `:1270-1283`) at the top of each tick, and builds the worms without any AI.

**FollowAI and the refusal gate.**

15. **Three committed sim setups have a FollowAI player 2.** `golden/sim_slice4_5a_{killemall,gametag,scales}_setup.cfg` set `[player1] controller = 1` and `[player2] controller = 2` (lines 19 and 35), and `sim_slice4_5a_settings_golden.rs` builds them with `build_match`. The C++ dumper never creates an AI, so they play as two humans. **So the Q2 refusal cannot live in `build::validate` / `validate_with`**, or that prior gate breaks (D1). The 4½c `weapsel_*` setups have bots too (`controller = 1`), and no shell setup has a CPU or FollowAI player.
16. **The RefusalGate's RESUME arm re-runs `validate_for_selection`** (`ui/src/shell/overlay.rs:210-215`). CONTROLLER never reaches a running match in C++ (finding 7), so a FollowAI check there would refuse a RESUME that C++ allows (D1).
17. **`boot_playing` (the preview's skip route) builds a match without the menu's gate** (`ui/src/shell/mod.rs:382-404`). In 4½f-1 it cannot meet a FollowAI player: the browser store holds only the shipped setups (`controller = 0`), `?cpu=` sets 0 or 1, and the skip route exists only in the browser (D1).

**The Rust shell.**

18. **Where the AI goes.** `Match::process` (`ui/src/shell/playing.rs:301-347`): the latch, then either the selection step, or `self.edges.apply(&inputs, &sim.worms)` (`KeyEdges`, `ui/src/keys.rs:247-259`; `apply_key_edges`, `:230-243`) followed by `tick_viewports` (`ui/src/shell/viewport_step.rs:121`). The edge word of a worm with no key change is its post-tick `control_states` — exactly the C++ `control_states` the AI starts from.
19. **RESUME** (`ui/src/shell/mod.rs:807-822`): while attached, `apply_live_settings` + `Match::resync`, then `Match::focus` (`playing.rs:280-286`). `focus_palette` (`playing.rs:38-42`) runs only at `Match::start`. C++ `LocalController::Focus` → `Game::Focus` → `UpdateSettings` rewrites the worm ramps from the game's settings on **every** focus (`localController.cpp:100-120`, `game.cpp:473-487`).
20. **The shell boot and the page.** `setup` loads the settings, applies `?level=` and builds `StartOptions { touch_only }` (`rust/game/src/main.rs:531-549`); `MatchParams` has no `cpu` (`game/src/web_params.rs:41-56`). The page: the touch-only flag (`web/index.html:261-266`), the weapon-selection hint `'P2: a bot'` (`:400`), the URL help block (`:115-128`), the Player 2 key help (`:200-202`); the preview comment (`.github/workflows/preview.yml:124-140`, "player 2 is a bot that is ready at once", "on a phone the player-2 bot does that by itself").
21. **No random player names in C++** (finding 4, desk-checked): `Settings::GenerateName` is `#if 0` (`settings.cpp:165-207`), its one caller is NAME's callback (`mainMenuState.cpp:339`), and the committed C++ golden `shell_cfg_default` saves the boot defaults with `name = ''` for every player, byte-equal to Rust's. PROGRESS's two "random names (4½f)" lines are corrected in T10.

## Decisions this plan makes (the design left them open)

- **D1. The FollowAI refusal (Q2) lives in the menu's NEW GAME gate only** (facts 15–17).
  - `scenario::build::refuse_follow_ai(s: &Settings) -> Result<(), BuildError>` returns `BuildError::FollowAiUnsupported { worm }` for the first of players 0 and 1 whose `controller == 2`. `validate` / `validate_with` / `validate_for_selection` do **not** call it, so `build_match` and every sim harness are unchanged.
  - The RefusalGate's `MA_NEW_GAME` arm calls it first (both routes). The RESUME arm never refuses FollowAI. `refusal_text` maps it to `"AI PLAYERS ARE NOT\0SUPPORTED YET"` at (160, 100), no clear, like the Holdazone box (e-1 D5).
  - `Match::start` makes an AI only for `controller == 1`; a `2` plays as an input-less human there. That arm is unreachable through the menu, and in 4½f-1 unreachable through `boot_playing` (fact 17); a `debug_assert!` in `Match::start` names the reason.
  - The dumper's FollowAI guard (T5) and the harness validator (T8) refuse any shell case that reaches NEW GAME with a `controller == 2` player.
- **D2. G2f-1 uses KEEP and PICK bots, never RANDOM** (fact 8). The design's `cpu_vs_cpu` becomes KEEP with the default picks. RANDOM with the AI is gated by G-AI `ai_vs_ai` through the sim dumper's `weapsel` (no intervention-3 constraint there), and the RANDOM constructor itself by 4½c's `weapsel_bot_random` / `weapsel_match_bot`. The phone's RANDOM path is exercised by the unit tests (T7) and the Chromium walk (T9).
- **D3. The touch rule splits into a settings rule and a selection rule** (facts 9, 10; Q3).
  - `ui::shell::selection::touch_settings(s: &mut Settings)` sets `worm_settings[1].controller = 1`. `game` applies it at boot (through `?cpu=`'s default, D4), and `Shell` applies it after every successful LOAD SETUP when `options.touch_only` (a shipped or desktop-saved setup has a human player 2, which a phone cannot drive). It is Rust-only; on a desktop nothing changes.
  - `new_game_config(settings, touch_only)` no longer forces the controller. On a touch-only page it sets `select_bot_weapons = 0` (RANDOM) — the global C++ setting, so it applies to every bot, as it would in C++. A touch-only player 2 that is Human (4½f-2's menu) then has no input, as in C++ without a keyboard.
  - `live_config` keeps its 4½c rule (controller 1 + KEEP) unchanged under a private helper, because its only caller is the native `--live <scenario>` path (fact 10). Its test stays as it is.
  - `BOT_WEAPONS_RANDOM: u32 = 0` joins `BOT_WEAPONS_KEEP`.
- **D4. `?cpu=`** (design §7.5). `MatchParams::cpu: Option<u8>`, parsed from `cpu=0|1|2` (anything else: a warning, ignored). `MatchParams::apply_cpu(&self, s: &mut Settings, touch_only: bool)`:
  - `Some(0)` → both players Human; `Some(1)` → player 1 Human, player 2 CPU; `Some(2)` → both CPU;
  - `None` → a touch-only page applies `touch_settings` (player 2 CPU); otherwise the loaded settings are kept.
  - It changes only the in-memory settings (the file is saved only by SAVE SETUP AS… or the exit save, from what the menu holds, like every URL parameter). `?cpu=` does not skip the menu.
- **D5. `reacts`: Rust persists it; both dumpers zero C++'s uninitialised copy** (finding 2, fact 2).
  - `WormState::reacts: [i32; 4]`, 0 in `from_init`; `worm_reactions` stays the pure computation and `state.rs` stores its result into `worms[i].reacts` right where it computes it (`:2025`), then passes `&worms[i].reacts` on. A worm that is not processed or not visible keeps it. Not hashed, not reset on death or respawn.
  - `oracle_dump_sim_physics` with `ai`: `std::ranges::fill(w->reacts, 0)` for both worms right after both `AddWorm` calls, before `weapsel` / `InitWeapons` (nothing between construction and there reads it).
  - `oracle_dump_shell`: intervention 3′, in intervention 3's block (after a frame that made a controller): the same fill for every worm of `CurrentGame()`. The boot controller is never processed, so it needs none.
  - Without `ai`, and on every existing shell case, it is a no-op on the output, because no human path reads `reacts` stale (fact 2). T4's and T5's regeneration proofs show it.
- **D6. `max_health` and the frozen generator.**
  - `WormState::max_health: i32`, **100** in `from_init` (the C++ `WormSettings::health{100}` default, `worm.hpp:104`), **not** `WormInit::health` (a scenario's `worm` line sets the start health against default settings). `new_match_with` and `apply_live_settings` write `worm_settings[i].health` into `worms[i].max_health` for i in 0..2.
  - `SimState::settings_health` is **removed**, not shadowed, so the compiler lists every site (fact 12). Each site reads the worm it acts on; `do_healing_direct(w, amount, game_mode)` reads `w.max_health`.
  - **One documented exception to frozen provenance:** `oracle-tests/examples/gen_slice4_5e1_sim.rs:308` becomes `i64::from(s.worms[i].max_health)`. It is a mechanical rename of a value that is equal on every G3 case (equal healths), proven by re-running that generator's `gen` for each committed G3 case into `$S/f1b2/g3/` and `cmp`-ing against the committed `sim_slice4_5e_*_scenario.txt` (T1 Step 6). No other frozen file changes.
- **D7. The `sim::ai` API.**
  ```rust
  pub struct DumbLieroAi { pub rand: Rand }       // Rand::new(): mt19937(0x1337), last 0
  impl DumbLieroAi {
      pub fn new() -> Self;
      /// DumbLieroAI::Process (worm.cpp:477-696). `cs` is the worm's word as the tick will
      /// start it (last tick's post-tick word + this tick's key edges); returns the word the
      /// tick applies. Reads `state` only (the target, the weapon, pos, visible, direction,
      /// aiming_angle, ninjarope, reacts, ai_params, cossin).
      pub fn process(&mut self, state: &SimState, worm: usize, cs: ControlState) -> ControlState;
      /// The same, filling `trace` (behaviour-identical; for unit tests and ledgers).
      pub fn process_traced(&mut self, state: &SimState, worm: usize, cs: ControlState,
                            trace: &mut AiTrace) -> ControlState;
  }
  /// LocalController::Process's AI loop (localController.cpp:156-164): for k in 0..2,
  /// worm (k + cycles % 2) % 2 runs its AI on inputs[worm].
  pub fn run_ais(ais: &mut [Option<DumbLieroAi>; 2], state: &SimState, inputs: &mut [ControlState; 2]);
  pub fn run_ais_traced(ais: &mut [Option<DumbLieroAi>; 2], state: &SimState,
                        inputs: &mut [ControlState; 2], traces: &mut [AiTrace; 2]);
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
  pub struct AiTrace {
      pub ran: bool, pub target: usize, pub max_dist: i32,
      pub max_dist_arm: MaxDistArm,         // Explo | Speed, plus `floored: bool`
      pub real_dist: i32, pub fire_drew: bool,
      pub dir: i32, pub fallback: Option<Fallback>,   // which of fact 3's ten arms, with `drew: bool`
      pub change: bool, pub rope_attached: bool,
      pub reacts_press: [bool; 2],          // the Left / Right `reacts` arms fired
      pub draws: u32,
  }
  ```
  The word is local: `Press`/`Release`/`SetControlState`/`ToggleControlState` act on it, never on `state` (**the AI must read `cs`, not `state.worms[w].control_states`**, which is last tick's word before the edges). Arithmetic is `i32` with `wrapping_*` where C++ `int` could overflow; `ftoi` is `>> 16`; the weapon is `state.weapons[w.weapons[w.current_weapon].ty.expect(..)]`.
- **D8. The AI's home is `Match`.** `Match { ais: [Option<DumbLieroAi>; 2], .. }`, set at `Match::start` (both routes, the skip route included) from the match's `settings.worm_settings[i].controller == 1`, once per NEW GAME (F5's Rust-only restart builds a new `Match`, so fresh AIs). `process`: selection → unchanged; match tick → `edges.apply` → `run_ais` → `tick_viewports`. `resync` and LOAD SETUP never touch `ais` (CONTROLLER does not reach a running match, finding 7). The non-shell paths (`--live <scenario>`, `--replay`, `Scripted`, `?demo`) get no AI.
- **D9. RESUME's palette.** `Match::focus` re-runs `focus_palette(&mut self.scene, &self.cfg.settings)` after the flow and the latch, i.e. after `resync` refreshed the attached copy. It is idempotent while the colours are unchanged (every prior case), and it is what C++'s `Game::Focus` does on every RESUME. f-2's `player_live` gates the colour edit; 4½f-1 unit-tests it by editing `rgb` in the settings directly.
- **D10. The oracle-only `ai` directive** (§Formats). Rust `Scenario::ai() -> bool`; `scenario::load` already refuses any `settings` scenario (`loader.rs:113-117`), and `ai` requires `settings`, so no second refusal is needed.
- **D11. `ai_params` are set by both builders** (`new_match_with` and `loader::load`) from `tc.aiparams.ordered()` as `[[off; 7], [on; 7]]` (index `[pressed as usize][control]`). Hash-neutral.
- **D12. The ledgers see the AI through `AiTrace`**, never by re-deriving it. The G-AI generator and the G2f-1 generator record arm counts from `run_ais_traced` over the driven state.
- **D13. No recorded-CPU replay test.** Recording a menu-driven match is postponed (4½d Q6), and the non-shell paths have no AI (D8), so design §11.4's "a recorded human-vs-CPU session replays" has no path yet. It is replaced by: two `Shell` runs with the same seeds and a CPU player give identical per-frame state hashes (T7), plus the G2f-1 gate itself. `round_trip.rs` and `record_regression.rs` stay untouched and green.
- **D14. Check 3″: the dumper self-polices the AI seed.** After every frame that made a controller, every worm whose `ai` is a `DumbLieroAI` must have `rand == Rand()`; otherwise `Fail`. So "no seed override is needed" is re-proven on every G2f-1 run, not only by T0.
- **D15. The respawn banner is player 1's, and only a human's.** `publish_respawn` keeps `waiting_to_respawn(worm 0)` and adds `&& shell.settings().worm_settings[0].controller == 0` (a `?cpu=2` player 1 readies itself). The banner, `lieroRespawn` and `waiting_to_respawn` otherwise stay as they are.
- **D16. Hooks for the walk.** `game::touch::worm_hooks(sim: &SimState) -> [WormHook; 2]` with `WormHook { x: i32, y: i32 /* ftoi */, visible: bool, word: u32, health: i32, lives: i32 }`; the wasm glue publishes `window.lieroWorms` (an array of two objects) every presented frame during play and `window.lieroControllers` (`[c0, c1]` from `shell.settings()`) with the other hooks. Pure, unit-tested natively, read-only.
- **D17. The seeded death-and-respawn walk.** A native `game` test searches (once, by the implementer) and then **pins** a match seed `S` for `?touch=1&seed=S&weapons=<W>` such that, with no input at all, player 2 (the CPU) is visible, dies, and is visible again within 1,500 ticks, through `Shell::boot_playing` over the browser store's settings with `apply_cpu(.., touch_only = true)`. The Chromium walk then waits for exactly that on the real bundle (T9). `<W>` is five weapons the TC has; the test asserts the pinned run, not the search.

## Standing ruling (4½c Addendum A, carried forward)

The C++ comparison happens **here**, against the real C++ run headlessly. If a dumper cannot run the real code headlessly, the task **stops and reports the exact blocker**. It never weakens a gate and never regenerates a golden to paper over a mismatch. The real `openliero` under Xvfb produces C++ | Rust side-by-side PNGs. Those are eyeball artefacts, never gates, and are never committed.

## Global constraints

- **LD 1:** pixel-exact menus on the 320×200 CPU surface.
- **LD 3, amended by the design (§2).** `tick_and_render` stays the only `ResMut<Sim>` holder, and menus get no `SimState`. The AI is not a menu: it runs inside `Match::process` and only **reads** `&SimState`; its output is the worm's input word.
- **LD 4, amended.** One new oracle-only directive, `ai`, of the same class as `settings`, `weapsel` and `generate`. Both parsers change in lockstep: Rust in T3, C++ in T4. The shell script grammar does not change.
- **LD 5.** `SimState::new`'s signature is unchanged.
- **LD 6.** The level seed equals the match seed; the AI seed is the constant `0x1337`, not derived from either.
- **Determinism firewall.** `sim::ai` is fixed-point only: no `f32`/`f64`, no `HashMap`/`HashSet`, no clock, its own `Rand`. The AI is **not** in `SimState` and not hashed (C++ keeps `Worm::ai` out of snapshots, `cereal_types.hpp:329`). No floats, no wall clock and no `HashMap`/`HashSet` iteration in `ui` or `render`.
- **Crate rules.** `sim-core` stays dependency-free and `sim` gains no dependency. `ui`, `render`, `scenario` and `shot` stay Bevy-free: `cargo tree -p ui -e normal | grep -c bevy` prints `0`.
- **`sim` changes are this plan's list only** (T1, T2), plus a fix a G-AI / G-HP / G2f-1 case *proves*, with its own unit test and commit and the full re-diff.
- **`render` changes only at the lifebar** (`hud.rs:148`) and its comment. Every render golden is unchanged.
- **C++ changes are confined to** `src/tools/oracle_dump/sim_physics_dump.cpp` and `src/tools/oracle_dump/shell_dump.cpp`. `CMakeLists.txt` is unchanged. `weapsel_drive.hpp` is included, never modified.
- **Frozen provenance.** Never edit these:
  - `examples/gen_slice4_5a.rs`, `gen_slice4_5c0.rs`, `gen_slice4_5c.rs`, `gen_slice4_5d.rs`, `gen_slice4_5e1_shell.rs`, `gen_slice4_5e2_shell.rs`; `gen_slice4_5e1_sim.rs` except D6's one line;
  - `tests/shell_e1_cases/`, `tests/shell_e2_cases/`, `tests/weapsel_common/`, `tests/sim_slice4_5c0_common/`;
  - `menu_dump.cpp`, `weapsel_dump.cpp`, `weapsel_drive.hpp`, `gen_menu_golden.sh`, `gen_sim_slice4_5e_golden.sh`;
  - every `golden/*` that exists at `c2d58fe`.

  `tests/shell_common/mod.rs` changes (T8). So `gen_slice4_5d -- check`, `gen_slice4_5e1_shell -- check` and `gen_slice4_5e2_shell -- check` must stay clean, and all 28 committed scripts must still equal their generators.
- **Golden audit.** `git diff --name-status c2d58fe -- rust/oracle-tests/golden` may list **only `A` lines**, exactly the **38 files** in §File structure: 24 `sim_slice4_5f_*` and 14 `shell_*`.
- **rustfmt.** Run it only on files a task *creates*: `rustfmt --edition 2024 <abs file>` for `ui`/`game`, `--edition 2021` elsewhere. Never run it on an existing file or on a `lib.rs`/`main.rs`, because it recurses. Hand-format edits to match the surrounding style.
- **The non-shell paths stay behaviour-identical:** `--live [<scenario>]`, `--live --record`, `--replay`, `Scripted`, `?demo`. `game/tests/round_trip.rs` and `record_regression.rs` stay green and untouched.

## Formats pinned (both sides implement exactly this)

**The `ai` scenario directive** (`oracle_dump_sim_physics` and `rust/scenario/src/parser.rs`):
```
ai        # oracle-only; no argument; at most once; requires `settings`. The setup's players 0
          # and 1 decide: controller 1 -> a real std::make_shared<DumbLieroAI>() (CreateAi's
          # controller-1 arm, localController.cpp:20-22); controller 2 -> refused; at least one
          # of the two must be 1. Works with `level` or `generate`, with or without `weapsel`.
```
- **Refusals** (both sides, same conditions; the Rust parser can check only the syntax, the Rust harness the rest): an argument; a duplicate; no `settings`; a controller 2 in player 0 or 1; no CPU player; an `input` line whose word for a CPU worm is non-zero.
- **The reacts intervention** (D5): right after both `AddWorm`s, `std::ranges::fill(w->reacts, 0)` for both worms. Only with `ai`.
- **Each tick** (the reduced tail; `render*` is excluded by `settings` already): apply the `input` words to the **human** worms only (`Unpack`), leave each CPU worm's `control_states` as the last tick left it; then, for `i` in `0..2`, worm `(i + game.cycles % 2) % 2` runs `ai->Process(game, worm)` if it is a CPU; then the unchanged tail (bonuses, objects, `++cycles`, …).
- **The line.** With `ai`, every line gains four columns after the 12: `<aiw0> <ail0> <aiw1> <ail1>` — for a CPU worm its `control_states.Pack()` right after this tick's AI step as `%02x`, and its AI's `rand.last` as `%08x`; `-` and `-` for a human worm. The tick-0 line has `- - - -`. The AI columns on line *k* belong to the pass that produced line *k*. Without `ai`, every line is byte-identical to today's (the "absent ⇒ old behaviour" rule).
- **The header comment** gains the `ai` entry in the grammar list and one paragraph on the intervention.

**Golden names.**
- G-AI: `sim_slice4_5f_ai_{idle,vs_human,vs_ai,weapons,close}{_scenario.txt,_setup.cfg,.txt}` (15 files).
- G-HP: `sim_slice4_5f_hp_{killemall,scales,tag}{_scenario.txt,_setup.cfg,.txt}` (9 files).
- G2f-1: `shell_cpu_match{_script.txt,.txt,_fs.txt,_user_liero.cfg}`, `shell_cpu_vs_cpu{_script.txt,.txt,_setup.cfg}`, `shell_cpu_pick{_script.txt,.txt,_setup.cfg}`, `shell_hp_boot{_script.txt,.txt,_fs.txt,_user_liero.cfg}` (14 files).

**The G-AI / G-HP scenario ledger header** (the 4½e G3 shape, one `#` line each, written by the generator):
```
# LEDGER (Rust, driven state): never over | game over at tick <N>, over for <M> rows
# LEDGER ai: runs <n> per worm; fire draws <n>; fallback <arm>:<n> ...; change <n>; rope-attached <n>;
#            reacts presses L <n> R <n>; max_dist arms explo <n> speed <n> floored <n>; deaths <d0> <d1>;
#            respawns <r0> <r1>
# LEDGER hp: max <h0> <h1>; clamps <n>; health bonuses <n0> <n1>; drips <n>; scales wraps <n0> <n1>
```
(the `ai` / `hp` lines only where they apply).

**`gen_sim_slice4_5f_golden.sh`** (new; the `gen_sim_slice4_5e_golden.sh` shape): builds `oracle_dump_sim_physics`, runs the 8 cases, honours `CHECKED_DUMPER`, and its awk gate checks 16 columns for `ai` cases (columns 13–16: `-` for a human, `^[0-9a-f]{2}$` / `^[0-9a-f]{8}$` for a CPU, `-` on row 1) and 12 for `hp` cases, row *k* carries tick *k*, `ticks + 1` rows (or the game-over tick + the 200-row margin), and the ledger's game-over expectation.

**`oracle_dump_shell`** — no new directive, line or top; the `out` header string is byte-identical for every script:
- **Intervention 3′** (D5): in intervention 3's block, after `game.rand.Seed(…)`: zero `reacts` for every worm of `CurrentGame()`.
- **Check 3″** (D14): in the same block, for every worm with an `ai`: a `FollowAI` → `Fail("frame N: a FollowAI player (unported; 4½f Q2): make it Human or CPU")` (the FollowAI guard); a `DumbLieroAI` whose `rand != Rand()` → `Fail("frame N: a new DumbLieroAI's RNG is not mt19937(0x1337) (D14)")`.
- **`gen_shell_golden.sh`:** `EXPECTED_SHELL_CASES` defaults to **32** (the 4½d 11 + e-1 11 + e-2 6 + f-1 4); the `upd`/`top` classes are unchanged. Before T8 commits the f-1 scripts, T5's gate runs it with `EXPECTED_SHELL_CASES=28`.

**`?cpu=`** (D4): `cpu=0|1|2`. **Hooks** (D16): `window.lieroWorms = [{x, y, visible, word, health, lives}, {…}]`, `window.lieroControllers = [c0, c1]`.

## File structure

| File | Task | Responsibility |
|---|---|---|
| `rust/sim/src/state.rs` (modify) | T1 | `WormState::{reacts, max_health}`, `SimState::ai_params`; `settings_health` removed; every health site per worm; `reacts` stored where computed |
| `rust/sim/src/{bonus,nobject,sobject,hash}.rs` (modify) | T1 | the health threading (mechanical); `hash.rs`'s test literal |
| `rust/render/src/hud.rs` (modify) | T1 | the lifebar per worm |
| `rust/scenario/src/{build.rs,loader.rs}` (modify) | T1 (mechanical: `max_health`, `ai_params`), T3 | per-worm health set; `ai_params` in both builders (T1); `AsymmetricHealth` removed, `apply_live_settings` health, `refuse_follow_ai` (T3) |
| `rust/oracle-tests/tests/{sim_slice5d_golden,sim_slice5d_fuzz,sim_slice6_scales_golden,sim_slice4_5e_generated_golden}.rs`, `examples/gen_slice4_5e1_sim.rs:308` (modify) | T1 | mechanical `settings_health` → `worms[i].max_health` (D6) |
| `rust/sim/src/ai.rs` (create), `rust/sim/src/lib.rs` (modify) | T2 | `DumbLieroAi`, `run_ais`, `AiTrace` |
| `rust/scenario/src/parser.rs` (modify) | T3 | `ai` |
| `rust/ui/src/{text.rs, shell/overlay.rs, shell/playing.rs, shell/mod.rs}` (modify), `rust/game/src/config.rs` (test only) | T3 | refusal text + gate arm (D1); `bootable` without the health arm; the updated refusal tests |
| `src/tools/oracle_dump/sim_physics_dump.cpp` (modify) | T4 | `ai`, the reacts intervention, the four columns, the header |
| `src/tools/oracle_dump/shell_dump.cpp` (modify), `rust/oracle-tests/gen_shell_golden.sh` (modify) | T5 | intervention 3′, check 3″ + the FollowAI guard, the header; the count 32 |
| `rust/oracle-tests/examples/gen_slice4_5f1_sim.rs` (create) | T6 | G-AI + G-HP `cfg` / `scan` / `gen` |
| `rust/oracle-tests/gen_sim_slice4_5f_golden.sh` (create) | T6 | the C++ goldens + awk gates |
| `rust/oracle-tests/tests/sim_slice4_5f_golden.rs` (create) | T6 | the G-AI + G-HP gate and the witness guard |
| `rust/oracle-tests/golden/sim_slice4_5f_*` (create, **24**) | T6 | §Formats golden names |
| `rust/ui/src/shell/{playing.rs, selection.rs, mod.rs, overlay.rs}` (modify) | T7 | the AIs in `Match`, `focus` palette, the touch split (D3), LOAD SETUP's touch re-apply, headless flows |
| `rust/oracle-tests/tests/shell_common/mod.rs` (modify) | T8 | the FollowAI validator, the CPU ledger fields (AI traces, deaths, respawns, P2-key-free) |
| `rust/oracle-tests/tests/shell_f1_cases/mod.rs` (create) | T8 | the 4 cases, their inputs, validators, witnesses |
| `rust/oracle-tests/examples/gen_slice4_5f1_shell.rs` (create) | T8 | `check` / `write` |
| `rust/oracle-tests/golden/shell_*` f-1 (create, **14**) | T8 | §Formats golden names |
| `rust/oracle-tests/tests/shell_golden.rs` (modify) | T8 | the f-1 cases, the milestone, the union list of 32 |
| `rust/game/src/{main.rs, touch.rs, web_params.rs}` (modify), `rust/game/tests/` (a new test file only if the pinned-seed test does not fit `web_params.rs`/`touch.rs` tests) | T9 | the touch boot, `BotRespawn` deleted, `?cpu=`, hooks, the banner rule, the pinned walk seed |
| `web/index.html`, `.github/workflows/preview.yml` (modify) | T9 | "player 2 is the CPU", `?cpu=`, hints |
| PROGRESS, overview, design status, cpp-map, rust-map, `.claude/skills/liero-shot/SKILL.md` | T10 | status + corrections |

## Batches, order and parallelism

| Batch | Tasks | Needs | Runs alongside | Touches | Gate |
|---|---|---|---|---|---|
| **1** | T0 probes | — | — (first, alone) | the two dumpers temporarily (restored), scratchpad, this plan (addendum) | every probe recorded; the contradiction rules applied |
| **2** | T1, T2 | 1 | **4** | `rust/sim`, `rust/render/src/hud.rs`, `rust/scenario/src/{build,loader}.rs` (mechanical), the D6 test/example lines | the full re-diff (both commands); the wasm check; golden status empty; D6's G3 `cmp` proof |
| **3** | T3 | 2 | **4** | `rust/scenario/src/{build,parser}.rs`, `rust/ui` (text, overlay, playing `bootable`, mod tests), `rust/game/src/config.rs` (test) | the full re-diff; the wasm check; G1 + all 28 G2 cases green |
| **4** | T4, T5 | 1 | **2, 3** (C++ only) | `sim_physics_dump.cpp`, `shell_dump.cpp`, `gen_shell_golden.sh` | builds (release + checked); clang-format 22 (diff + whole file) + clang-tidy; all 40 `oracle_dump_sim_physics` gen scripts and all 28 shell goldens regenerate byte-identically (the known hang reported by name); the scratch smokes |
| **5** | T6 | 2, 3, 4 | — | new G-AI/G-HP files in `rust/oracle-tests`; `rust/sim` (proven fixes only) | **G-AI + G-HP bit-exact** (release and checked dumpers); the full re-diff |
| **6** | T7 | 3 (and 5 by schedule) | — | `rust/ui` (+ callers if a signature moves) | the full re-diff; the wasm check; G1 + all 28 G2 cases green |
| **7** | T8 | 4, 6 | — | `rust/oracle-tests` (shell); `rust/ui` / `rust/sim` (proven fixes only, no pub-API change `game` uses) | **🎯 G2f-1 bit-exact** (every `f`/`d`/`file` line) under both C++ builds; the full re-diff |
| **8** | T9 | 6 | — | `rust/game`, `web/index.html`, `.github/workflows/preview.yml` | `cargo test -p game`; the wasm check; the bundle + the three Chromium walks + the e-2 phone walk |
| **9** | T10 | 7, 8 | — (last) | docs, skill | the full board + every audit + the broad review |

**Schedule:** 1 → (2 ∥ 4) → 3 → 5 → 6 → 7 → 8 → 9. Batch 4 is C++-only and may keep running through Batches 2 and 3; it must be done before 5. G-AI/G-HP (Batch 5) go before the UI on purpose: they are the risk-first gate (design §10: the draw order and the `reacts` UB), and a sim fix is best landed before the shell builds on it. Rust batches never overlap (disk, one checkout). Batch 8 needs only T7's API, so it may go before 7 if 7 is stuck in its fix loop; record the reorder.

**Design ↔ plan mapping (design §8, 4½f-1):**

| Design | This plan |
|---|---|
| T0 (P1, P2, P4, P5, P6) | T0 (P1–P7; P3 and P7 added, P4 a desk check + a committed golden) |
| T1 (`reacts`, `max_health`, `ai_params`) | T1 |
| T2 (`sim::ai`) | T2 |
| T3 (scenario health, `AsymmetricHealth`, `apply_live_settings`, Q2, the boot) | T3 (Q2 moved to the gate, D1) |
| T4 (C++ `ai` + intervention; generators; goldens) | T4 (C++) + T6 (generator, goldens) |
| T5 (Rust harnesses) | T6 |
| T6 (`Match` AIs, RESUME palette; lifebar) | T7 (lifebar in T1) |
| T7 (C++ shell 3′, FollowAI guard; G2f-1 corpus) | T5 (C++) + T8 (corpus) |
| T8 (🎯 milestone f-1) | T8 |
| T9 (`game`) | T9 |
| T10 (docs, review) | T10 |

---

### Task 0 (Batch 1): the C++ probes

Everything the design and this plan read from the source that a later gate depends on is probed with the **real** code: `Gfx::RunOneFrame` in a temporarily patched `oracle_dump_shell`, and the real `Worm::Process` in a temporarily patched `oracle_dump_sim_physics`. No probe adds code to the repo.

**Files:** `src/tools/oracle_dump/shell_dump.cpp` and `src/tools/oracle_dump/sim_physics_dump.cpp` (patched temporarily, restored at the end), everything under `$S/t0f1/`, this plan (the addendum).

- [ ] **Step 1: patch `oracle_dump_shell`, in the working tree only.** Save the diff as `$S/t0f1/shell_probe.diff`.
  - `DetailLine` accepts `cur_menu == &gfx.player_menu` as `P` (the player menu, for P5/P6) instead of `Fail`.
  - A probe-only `x <frame>` line after each `d` line (or after each `f` line when there is no `detail`), for the current controller's game when there is one: per worm, `ctl=<controller> ai=<0 none|1 dumb|2 follow> fresh=<rand == Rand()> last=<%08x rand.last> rs=<%016x FNV of rand.serialize()> reacts=<r0,r1,r2,r3> cs=<%02x control_states.Pack()> vis=<0|1> hp=<health>/<settings->health> lives=<lives> ws=<0|1 InWeaponSelection>`.
  - An env switch `PROBE_ZERO_REACTS=1` applies D5's fill in intervention 3's block.
  - Build: `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell`, and the same patch in `$S/build-chk`.
- [ ] **Step 2: patch `oracle_dump_sim_physics`, in the working tree only.** Save as `$S/t0f1/sim_probe.diff`. With env `PROBE_HP=1`, print to stderr per tick and per worm `t w health max lives visible` (after `dump`). Nothing else changes.
- [ ] **Step 3: the fixtures**, written by `$S/t0f1/gen.py` (setups via plain TOML text; every script `setup <file>`, `boot_seed 7`, `detail`, ends by QUIT):

  | Probe | Script(s) | Setup | Path |
  |---|---|---|---|
  | **P1** AI seed, **P2** `reacts` | `s_seed` | P2 `controller = 1`, `selectBotWeapons = 2` (KEEP), default picks | NEW GAME → P1 DONE → 300 frames → Esc → NEW GAME again (a second controller over the freed heap) → P1 DONE → 300 frames → Esc → QUIT; run it with and without `PROBE_ZERO_REACTS`, twice on the release build and once on the checked build |
  | **P3** CPU weapon selection | `s_random`, `s_keep`, `s_pick` | P2 CPU with `selectBotWeapons` 0 / 2 / 1 | NEW GAME → (PICK: P2's arrow keys + Right Ctrl onto DONE) → P1 DONE → 120 frames → Esc → QUIT |
  | **P5** HEALTH at RESUME | `s_hp`, `s_hp0` | default | NEW GAME → DONE → 200 frames → Esc → F5 → Down ×3 (HEALTH) → Return → BACKSPACE ×3, `text` `3`, `0` → Return → Esc → F1 (RESUME) → 200 frames → Esc → QUIT; `s_hp0` idles for the same frames instead of the edit |
  | **P6** colour at RESUME | `s_rgb`, `s_rgb0` | default | as `s_hp` to F5 → Down ×4 (Red) → hold Left 40 frames → Esc → F1 → 100 frames → Esc → QUIT; `s_rgb0` idles |

  Key counts come from the source (design finding 12: with no profile loaded the player menu opens on SAVE PROFILE AS…, so HEALTH is 3 rows down and Red 4); if the observed menu differs, adjust and record it. Run each with `build/linux-x64/Release/oracle_dump_shell $S/t0f1/<s>.txt $S/t0f1/<s>.out`. The `x` lines are the evidence; `$S/t0f1/an.py` prints the frames where a field of interest changes.
- [ ] **Step 4: P7, per-worm health in the sim.** Two scratch scenarios in `$S/t0f1/`, `settings` + `generate` (504×350) + 1,500 ticks of 4½a-style fuzz input for both worms: `hp_scales` (Scales of Justice, P1 30, P2 200) and `hp_bonus` (Kill'em All, P1 50, P2 300, MAX BONUSES 10). Run with `PROBE_HP=1`.
- [ ] **Step 5: the expected outcomes** (facts 1–14). Record each as **confirmed**, or with what differed.
  - **P1.** On the frame that made each controller, P2's `ai=1 fresh=1 last=00000000`; it stays `fresh=1` through every `W` frame and the finalise frame; the first `G` tick changes `last`. The second NEW GAME's AI is `fresh=1` again. Both builds agree.
  - **P2.** Record the `reacts` values P2 holds on the new-controller frame and on the finalise frame, for each run (release ×2, checked, the second NEW GAME). Then compare `s_seed`'s `state8` with and without `PROBE_ZERO_REACTS`: equal on every frame (the garbage is 0 here) or the first differing frame (the garbage moved the AI). Either way D5 stands; the addendum records which.
  - **P3.** `s_random` **fails** with intervention 3's "drew the RNG" message on its NEW GAME frame (D2's premise). `s_keep`: P2 ready on the first `W` frame; no failure. `s_pick`: P2 is not ready, its cursor moves with P2's keys, and P2's `cs` during `W` frames changes only on P2's key frames (the AI does not run in selection).
  - **P5.** From the first resumed tick on, P1's `hp` is `≤ 30` and its max reads `30` in `s_hp`, while `s_hp0` keeps 100; `state8` differs from the first resumed tick.
  - **P6.** After RESUME, `s_rgb`'s presented frames differ from `s_rgb0`'s while `state8` is equal on every frame (the palette only).
  - **P7.** In `hp_scales`, each worm's health never exceeds its own max, a Scales transfer onto P1 wraps at 30 and onto P2 at 200 (a life gained); a respawn restores each worm to its own max. In `hp_bonus`, the health bonus heals `(rand(var) + min) * max / 100` of *that* worm's max (at least one pickup, or record "no pickup in 1,500 ticks"), and low-health blood starts under 12 for P1 and under 75 for P2.
  - **P4 (desk).** Confirm fact 21 by reading `settings.cpp:165-207` and the `name = ''` lines of `shell_cfg_default`'s saved file (regenerate that case into `$S/t0f1/` and read the fixture's `user/Setups/liero.cfg` before the dumper removes it, or decode it through `settings_from_toml` of the saved bytes the Rust harness produces for the same case).
- [ ] **Step 6: restore.** `git -C /home/user/openliero checkout -- src/tools/oracle_dump/shell_dump.cpp src/tools/oracle_dump/sim_physics_dump.cpp`, then rebuild both targets in both builds. `git status --short src` must be empty. The restored binaries must regenerate `shell_boot_idle.txt`, `shell_milestone.txt` and one `sim_slice4_5e_*` golden byte-identically (`cmp`).
- [ ] **Step 7: record and commit.** Append **"## Addendum T0 (probe results)"** to this plan: the command lines, each probe's verdict with frame numbers and field values, P2's observed `reacts` values, and any changed task text. Then `git add docs/superpowers/plans/2026-09-27-liero-rs-step4.5-slice4.5f1-plan.md` and commit `docs(4.5f-1): T0 — C++ probes of the AI seed, reacts, CPU weapon selection and per-worm health (results in the plan)`.

**If a probe contradicts the plan** (the source wins; these are findings, not product choices):
- **P1 contradicted** (the AI's RNG is not fresh, is shared between CPUs, or changes before the first tick): find what seeds or draws it and record it. If it is deterministic, Rust mirrors it (D7 changes; D14's check follows the observation). If it depends on the clock, **stop and report**: G2f-1 would need a seed intervention, and design decision 1 reopens.
- **P2: the fill changes nothing** (the build holds 0): D5 stays (it is UB; a no-op here), recorded. **The fill moves `state8`**: D5 is confirmed as necessary. **`reacts` is written before the first AI read** (some path the plan missed): name it; Rust ports that write, and D5's intervention is dropped from both dumpers.
- **P3 contradicted** (RANDOM does not trip intervention 3): D2 is relaxed, and `cpu_vs_cpu` uses RANDOM as the design wanted. KEEP draws: T8 gives the CPU non-zero, enabled picks. The AI runs in selection, or P2's keys do not drive a PICK bot: fact 5 or 6 is rewritten, and T7's selection arm follows the observation.
- **P5 contradicted** (the HEALTH edit does not reach RESUME): `apply_live_settings` does not write `max_health` (T3 Step 2 changes); record it for 4½f-2.
- **P6 contradicted** (no palette change at RESUME): D9 is dropped.
- **P7 contradicted:** the sim follows the observation; fact 11 is rewritten, and T1's tests pin the observed form.
- **A probe cannot run** (a patch fails to build, the dumper crashes): **stop and report** for P1–P3, because T2, T4, T5 and T8 depend on them. P5–P7 may be recorded as "unconfirmed", and the plan proceeds with the source-derived facts.

**Done when:** the addendum records each probe, the contradiction rules have been applied, `src/` is clean, and the addendum commit exists.

---

### Task 1 (Batch 2): `sim` state — `reacts` kept, `max_health` per worm, `ai_params` (hash-neutral)

**Files:** `rust/sim/src/{state.rs, bonus.rs, nobject.rs, sobject.rs, hash.rs}`, `rust/render/src/hud.rs`, `rust/scenario/src/{build.rs, loader.rs}` (the two setter lines only), the D6 lines in `rust/oracle-tests/tests/{sim_slice5d_golden,sim_slice5d_fuzz,sim_slice6_scales_golden,sim_slice4_5e_generated_golden}.rs` and `examples/gen_slice4_5e1_sim.rs:308`.

- [ ] **Step 1: `reacts`** (D5, fact 2). `WormState::reacts: [i32; 4]`, `[0; 4]` in `from_init`, doc-commented "not hashed (C++ `HashGameState` has no `reacts`); kept across ticks; read stale only by `sim::ai`". At `state.rs:2025` store `worm_reactions`' result into `worms[i].reacts` and pass the stored array to `process_tasks` and `worm_process_physics`. **Tests (RED first):**
  - a visible worm's `reacts` after a tick equals `worm_reactions` of the pre-tick state;
  - an invisible worm's `reacts` is unchanged by a tick (set it to `[1, 2, 3, 4]`, tick, assert);
  - a skipped worm (Kill'em All, `lives == 0`) keeps it;
  - `state_hash` ignores it (two states differing only in `reacts` hash equal).
- [ ] **Step 2: `max_health`** (D6, facts 11, 12). Add `WormState::max_health: i32` (100 in `from_init`), remove `SimState::settings_health`, and let the compiler list the sites. Each site reads the worm it acts on:
  - the clamp (`:2008`) and respawn (`:2874`) read `worms[i].max_health`;
  - the drip (`:2921`) reads `w.max_health`;
  - Scales' extra life on death (`:3034`) reads the dying worm's;
  - `do_healing_direct(w, amount, game_mode)` reads `w.max_health`; `do_damage`'s Scales redistribution heals each *other* worm by its own max (`game.cpp:566-589`); `do_healing`'s clamp reads the healed worm's;
  - the health-bonus pickup (`bonus.rs:509-518`) scales by the picking worm's;
  - `nobject`/`sobject` stop threading the scalar;
  - `render::hud` (`hud.rs:148`): `worm.health * 100 / worm.max_health`, with the comment rewritten.
  - `scenario::build::new_match_with` sets `state.worms[i].max_health = s.worm_settings[i].health` for i in 0..2 in place of `:239` (the refusal still makes them equal in this task).
  - **Tests (RED first):**
    - the clamp caps each worm at its own max (worm 0 max 50 at health 80 → 50; worm 1 max 300 at 280 stays);
    - respawn restores each worm's own max;
    - `do_healing_direct` in Scales wraps at the worm's own max (max 30, health 25, +40 → lives +1, health 35 → +1 more → 5… pin the exact arithmetic of `game.cpp:556-561`);
    - a Scales `do_damage` on worm 0 heals worm 1 against worm 1's max;
    - the health-bonus amount uses the picking worm's max (port `bonus.rs:1652`'s test to two different maxes);
    - the drip gate is `health < max_health / 4` per worm;
    - the HUD lifebar width for health 150 of max 300 is 50.
- [ ] **Step 3: `ai_params`** (D11). `SimState::ai_params: [[i32; 7]; 2]`, zeros in `new`; `new_match_with` and `loader::load` set it from `tc.aiparams.ordered()` (`[0][c] = off`, `[1][c] = on`). **Test:** the openliero TC gives `[[120, 120, 50, 50, 80, 300, 400], [20, 20, 20, 20, 80, 60, 1]]` (up, down, left, right, fire, change, jump).
- [ ] **Step 4: the mechanical call sites** outside `sim` (fact 12): `sim/src/hash.rs:250`'s literal; the four `oracle-tests` tests read `state.worms[1].max_health` (or `[0]`, the same value there) where they read the scalar, keeping every assertion's meaning; `gen_slice4_5e1_sim.rs:308` per D6.
- [ ] **Step 5: the re-diff.** Both commands, the wasm check, and `git status --porcelain rust/oracle-tests/golden` empty. Every existing golden has equal healths, so every existing gate must stay green unchanged.
- [ ] **Step 6: D6's proof.** For each of `small`, `odd`, `tall`, `banned`, read `seed`, `generate` and the input seed from the committed `sim_slice4_5e_<case>_scenario.txt` header, run `cargo run --manifest-path rust/Cargo.toml -p oracle-tests --example gen_slice4_5e1_sim -- gen <case> <game_seed> <input_seed> $S/f1b2/g3/<case>_scenario.txt` (and `cfg` for the setup), and `cmp` each against the committed file. All 8 must be identical. Record the commands.
- [ ] **Step 7: commit** `sim(4.5f-1): per-worm max_health replaces the settings_health scalar at every read site; reacts kept between ticks (unhashed); the TC's ai_params; render: the lifebar per worm (all hash-neutral)`.

**Done when:** every test above passes, both re-diff commands and the wasm check are green, `grep -rn settings_health rust --include=*.rs | grep -v target` is empty (comments included, except `gen_slice4_5a.rs`'s frozen comments), and D6's 8 `cmp`s are identical.

---

### Task 2 (Batch 2): `sim::ai::DumbLieroAi` and `run_ais`

**Files:** new `rust/sim/src/ai.rs`, `rust/sim/src/lib.rs` (`pub mod ai;`).

- [ ] **Step 1: the port** (D7; facts 3, 4). A line-for-line port of `worm.cpp:477-696`, in the source's order, with the source's line numbers in comments:
  - target: the other worm with the smallest `sqr_vector_length` of the `ftoi` positions (first wins ties);
  - `max_dist` (fact 4), `real_dist = vector_length(ftoi(delta))`;
  - Fire (conditional draw), Jump, Change (always), each with bound `ai_params[pressed as usize][control]` through `rand.bound(k as u32)`;
  - `delta = if real_dist > 0 { delta.div(real_dist) } else { zero }`; the scan over `state.cossin[1..128]`; the fallback arms with their `rand(16)` exactly where C++ has them;
  - the Change arm (read Change again), the rope arm, the walk/aim arm and the two `reacts` arms.
  - `run_ais` / `run_ais_traced` in `(k + cycles % 2) % 2` order.
- [ ] **Step 2: unit tests on hand-built states** (a `SimState` from `scenario::build::build_match` on a flat generated level, then positions, weapons, `reacts`, rope and words set by hand; pin C++ lines in each test's comment). Each asserts the returned word **and** the exact number of draws (`rand.draws()` delta) **and** the `AiTrace`:
  - target choice: three worms are impossible, so: the other worm at equal and at different distances;
  - each `max_dist` arm: a weapon with `time_to_explo` in `(0, 500)`, one with `0`, one with `≥ 500`, and a weak one floored to 90;
  - Fire: in range (a draw; toggled iff the draw is 0), out of range and visible (released, no draw), dead (`!visible`: a draw even out of range);
  - Jump and Change: `rand(1)` (`jump on = 1`) always returns 0, so a pressed Jump is always released next call;
  - the scan: a delta along `cossin[37]` finds `dir = 37`; `real_dist == 0` gives `dir = 12` with no draw;
  - each of the 10 fallback arms (fact 3), the six that draw `rand(16)` and the four that do not (craft deltas that miss every table entry: e.g. a `real_dist` of 1 or 2 makes `delta` non-unit; equal magnitudes for the `80` / `48` arms);
  - Change pressed: Left then Right draws; rope `out && attached` adds Up then Down draws; rope not attached releases Up and Down with no draw;
  - Change not pressed: walk toward (`real_dist > max_dist`) sets Right/Left from `delta.x`; in range releases both; `direction` 0 and 1 with `dir` above and below 64 and the `aiming_angle` comparisons;
  - the `reacts` arms: Left pressed with `reacts[RF_RIGHT] > 0` → Right if `reacts[RF_DOWN] > 0` else Jump; the mirrored Right arm;
  - the AI reads `cs`, not `state.worms[w].control_states` (set them different; the result follows `cs`);
  - `run_ais`: on even `cycles` worm 0 runs first, on odd worm 1 (observe through each AI's `rand.draws()` order with a shared log in the test, or by giving both AIs the same seed and checking the trace order); `None` AIs leave their word unchanged;
  - `DumbLieroAi::new().rand` equals `Rand::new()` (`last() == 0`) and its first value is mt19937(0x1337)'s first (`0x…` — take it from `rng_golden.rs`'s vector).
- [ ] **Step 3: a 2,000-tick smoke** (a unit test): two CPUs on a generated 504×350 level from `build_match`, fed only by `run_ais` — no panic, both worms are visible at some tick, each word is non-zero on some tick, and the run is deterministic (run twice, compare every `state_hash`).
- [ ] **Step 4: tripwire.** `grep -nE "f32|f64|HashMap|HashSet|Instant|SystemTime" rust/sim/src/ai.rs` is empty.
- [ ] **Step 5: gate + commit.** The re-diff (both), the wasm check, the golden status empty. Commit `sim(4.5f-1): sim::ai — DumbLieroAI line for line (worm.cpp:477-696), its own mt19937(0x1337), run_ais in LocalController's alternating order, AiTrace for tests and ledgers`.

**Done when:** every arm above is unit-tested with its draw count, the smoke is deterministic, and `sim` has no new dependency.

**Batch 2 gate:** T1 and T2 together, one more re-diff on the combined tree, then the disk cleanup.

---

### Task 3 (Batch 3): `scenario` and `ui` — unequal health plays, the FollowAI gate, the `ai` directive

**Files:** `rust/scenario/src/{build.rs, parser.rs}`, `rust/ui/src/{text.rs, shell/overlay.rs, shell/playing.rs, shell/mod.rs}`, `rust/game/src/config.rs` (its test only).

- [ ] **Step 1: `AsymmetricHealth` goes** (fact 13). Remove the variant, its `Display` arm and the check in `validate_with`; `InvalidHealth` stays. Remove `bootable`'s arm and its doc line ("Unequal healths both take player 1's"), and `refusal_text`'s arm. Update the tests that pinned them: `build.rs:400` now asserts `Ok` for 100 / 150; `text.rs:491-495` is replaced by D1's text test; `mod.rs:2521-2534` keeps the zero-weapons and blood-max refusals and asserts that unequal healths start a match; `mod.rs:2717` and `game/src/config.rs:506` follow.
- [ ] **Step 2: health reaches the sim live** (finding 6; T0 P5). `apply_live_settings` writes `worms[i].max_health = s.worm_settings[i].health` for i in 0..2 and the doc table's health row points here. **Tests:** after `apply_live_settings` with 30 / 250 the two maxes are 30 / 250; a tick then clamps a worm above 30 (the clamp is the sim's, from T1); it draws nothing (`rand.draws()` unchanged).
- [ ] **Step 3: the FollowAI gate** (D1). `BuildError::FollowAiUnsupported { worm: usize }` with a `Display`; `pub fn refuse_follow_ai(s: &Settings) -> Result<(), BuildError>` (players 0 and 1; the network player is never checked). `RefusalGate::refusal`'s `MA_NEW_GAME` arm calls it first; the RESUME arm is unchanged. `refusal_text` maps it to `"AI PLAYERS ARE NOT\0SUPPORTED YET"`. **Tests:**
  - `refuse_follow_ai` for each player, for 0/1 (Ok), and the network player's 2 (Ok);
  - `validate`, `validate_for_selection` and `build_match` still accept a controller-2 setup (fact 15: build `sim_slice4_5a_killemall_setup.cfg`);
  - the shell: NEW GAME with player 2 on controller 2 → the box, the text, the menu stays (the Holdazone test's shape, `mod.rs:2475`); F1 on RESUME of an attached match whose settings were changed to controller 2 while paused → **resumes**; the boot with a controller-2 `liero.cfg` boots.
- [ ] **Step 4: the `ai` directive** (§Formats, D10). The parser accepts `ai` with no argument, at most once, and only with `settings`, refusing with messages in the file's style; `Scenario::ai() -> bool`; `to_text` writes `ai` after `settings`. **Tests:** round trip; each refusal; `scenario::load` panics on an `ai` scenario (through its `settings` check).
- [ ] **Step 5: gate.** The re-diff (both), the wasm check, `-p oracle-tests --test menu_widget_golden` (G1) and `--test shell_golden` (all 28) green, `gen_slice4_5d -- check`, `gen_slice4_5e1_shell -- check`, `gen_slice4_5e2_shell -- check` clean, the golden status empty.
- [ ] **Step 6: commit** `scenario+ui(4.5f-1): unequal healths play (AsymmetricHealth and the boot's copy removed; RESUME brings both maxes); the FollowAI refusal at NEW GAME only; the oracle-only ai directive`.

**Done when:** every test above is green, no prior gate moved, and `cargo tree -p ui -e normal | grep -c bevy` prints `0`.

---

### Task 4 (Batch 4): `oracle_dump_sim_physics` gains `ai`

**Files:** `src/tools/oracle_dump/sim_physics_dump.cpp`.

- [ ] **Step 1: parse `ai`** exactly as §Formats and the Rust parser (T3 Step 4): no argument, once, needs `settings`. Refuse with `std::fprintf(stderr, …); return 1`, as the file does.
- [ ] **Step 2: build the AIs.** After both `AddWorm` calls on the `settings` path: refuse a controller 2 in players 0/1 and a setup with no controller 1; for each controller-1 worm `w->ai = std::make_shared<DumbLieroAI>()`; then D5's fill of `reacts` for both worms. Refuse an `input` line with a non-zero word for a CPU worm (check the parsed map once, before the loop).
- [ ] **Step 3: the tick.** On the reduced tail, `Unpack` only the human worms' words; then the AI loop of `localController.cpp:156-164` verbatim (`kPhase = game.cycles % 2`, `(i + kPhase) % game.worms.size()`, `if (worm.ai.get()) worm.ai->Process(game, worm)`), record each CPU worm's `control_states.Pack()` and `dynamic_cast<DumbLieroAI&>(*worm.ai).rand.last`; then the unchanged tail. `dump(t + 1)` prints the four columns (§Formats); `dump(0)` prints `- - - -`.
- [ ] **Step 4: the header.** The grammar list gains `ai`; one paragraph explains the CPU worms' persisted words, the order, the four columns and the reacts intervention (finding 2, D5).
- [ ] **Step 5: the regeneration proof.** In one shell, in the background with a log:
  - `source $S/env.sh && for s in $(grep -l oracle_dump_sim_physics rust/oracle-tests/*.sh); do timeout 900 bash "$s" || echo "FAIL $s"; done` — 40 scripts;
  - `git status --porcelain rust/oracle-tests/golden` must be **empty**;
  - `gen_sim_slice5prime_pickup_weapon_golden.sh` hangs on this machine (pre-existing C++ UB, PROGRESS 4½e-1): its `timeout` is expected and reported by name; any other failure stops the task.
- [ ] **Step 6: smokes** (`$S/f1b4/`, never committed):
  1. `settings` (P2 `controller = 1`) + `generate 77` + `ticks 200` + `ai`: 201 rows of 16 columns, row 1 ends `- - - -`, columns 13–14 are `-` on every row, 15–16 are hex, and column 16 changes on most rows;
  2. the same with `weapsel` (P2 CPU, `selectBotWeapons = 0`, one `weapsel 0 0 0` line — or as many frames as the phase needs): runs;
  3. refusals: `ai` without `settings`; `ai ai`; `ai 1`; a controller 2; no CPU; `input 5 0 1` with worm 1 a CPU — each exits 1 with its message;
  4. the same scenario as (1) under `$S/build-chk/oracle_dump_sim_physics` writes the same bytes.
- [ ] **Step 7: format, tidy, commit.** `"$CF22" --dry-run -Werror --style=file /home/user/openliero/src/tools/oracle_dump/sim_physics_dump.cpp`; `CLANG_FORMAT="$CF22" scripts/clang-format-diff.sh c2d58fe`; `scripts/clang-tidy-diff.sh build/linux-x64 c2d58fe` exits 0. Commit `oracle(4.5f-1): oracle_dump_sim_physics — the oracle-only ai directive (real DumbLieroAI in LocalController's order, the reacts intervention, four AI columns)`.

**Done when:** it builds in both builds, format and tidy are clean, every existing sim_physics golden regenerates byte-identically (the known hang aside), and the smokes behave as described.

### Task 5 (Batch 4): `oracle_dump_shell` — intervention 3′, check 3″, the FollowAI guard

**Files:** `src/tools/oracle_dump/shell_dump.cpp`, `rust/oracle-tests/gen_shell_golden.sh`.

- [ ] **Step 1: the block.** In the "A NEW GAME made a controller" block (`shell_dump.cpp:810-833`), after `game.rand.Seed(…)`: for every worm of `CurrentGame()`: the FollowAI guard, check 3″ (D14), and intervention 3′ (D5), with §Formats' messages.
- [ ] **Step 2: the header.** Interventions 3′ and check 3″ join intervention 3's entry; the opening sentence names `DumbLieroAI` among the real code that runs. The `out` header string is unchanged.
- [ ] **Step 3: `gen_shell_golden.sh`** — the comment and the default count 32 (§Formats).
- [ ] **Step 4: the regeneration proof.** `source $S/env.sh && EXPECTED_SHELL_CASES=28 bash rust/oracle-tests/gen_shell_golden.sh` (background, log), then `git status --porcelain rust/oracle-tests/golden` must be **empty**. Rebuild `$S/build-chk`, copy `$S/b7chk/run.sh` to `$S/f1b4/chk.sh`, and run all 28: all `SAME`.
- [ ] **Step 5: smokes** (`$S/f1b4/`, never committed), each a `setup` script:
  1. P2 CPU with KEEP: NEW GAME → P1 DONE → 300 frames → Esc → QUIT runs to `end`, and a copy with the intervention-3′ lines commented out (a scratch build, not committed) gives the same or different `d` lines — record which (it repeats T0 P2 under the final code);
  2. P2 CPU with RANDOM fails with intervention 3's message;
  3. P2 on controller 2 fails with the FollowAI message on its NEW GAME frame;
  4. check 3″ fails if the scratch build calls `Seed(1)` on the new AI's `rand` first (a negative control; scratch only).
- [ ] **Step 6: format, tidy, commit.** As T4 Step 7 for this file. Commit `oracle(4.5f-1): oracle_dump_shell — intervention 3′ (zero the new game's reacts), check 3″ (every new DumbLieroAI starts at mt19937(0x1337)), the FollowAI guard`.

**Done when:** both builds are clean, the 28 shell goldens are byte-identical under both, and the four smokes behave as described.

**Batch 4 gate:** T4 and T5 both done; `git diff --name-only c2d58fe -- src CMakeLists.txt` lists exactly the two dumper files.

---

### Task 6 (Batch 5): G-AI and G-HP, the AI's control stream and unequal health bit-exact against C++

**Files:** new `rust/oracle-tests/examples/gen_slice4_5f1_sim.rs`, `rust/oracle-tests/gen_sim_slice4_5f_golden.sh`, `rust/oracle-tests/tests/sim_slice4_5f_golden.rs`; the 24 `golden/sim_slice4_5f_*` files; `rust/sim/**` only for a proven fix.

- [ ] **Step 1: the generator** (the `gen_slice4_5e1_sim.rs` shape, a new file): `cfg <case> <out>`, `scan <case> <input_seed> <lo> <hi>`, `gen <case> <game_seed> <input_seed> <out>`. The human's input is 4½a's `Rand(input_seed).next_u32() & 0x7f` per tick; a CPU worm's input word is always 0 in the file. The level is `generate <level_seed>` (504×350 unless the case says otherwise). It drives the Rust side exactly as the gate does (Step 4) and writes the §Formats ledger; `scan` skips seeds whose ledger misses a witness or that trip a `debug_assert!`.

  | Case | Setup | Ticks | Witnesses (the ledger must show each) |
  |---|---|---|---|
  | `ai_idle` | P2 CPU, P1 human with input 0, default picks, Kill'em All lives 3 | 3,000 | the CPU readies by its own Fire toggles (visible true); the walk arm; the in-range Fire arm; ≥ 1 death of P1 caused by P2 (`last_killed_by_idx == 1`) or, if none, ≥ 1 P2 death — record which |
  | `ai_vs_human` | P1 fuzz, P2 CPU, lives 3 (Hard gate 4's human-vs-CPU golden) | until game over + 200 | deaths both ways; ≥ 1 CPU respawn; game over |
  | `ai_vs_ai` | both CPU, `selectBotWeapons = 0` (RANDOM) through one `weapsel` frame, lives 2 | until game over + 200 | the constructor's RANDOM draws (the tick-0 `rng` ≠ 0); the two AI streams equal on tick 1's `rand.last` columns only if their draws match (record), and diverging later; both orders of `cycles % 2`; game over |
  | `ai_weapons` | P2 CPU with five picks that cover `time_to_explo` in `(0, 500)`, `0`, `≥ 500`, a weapon floored to 90, and a laser (chosen from the TC's weapon table by the generator and recorded by name) | 3,000 | every `max_dist` arm and the floor; ≥ 3 distinct `current_weapon`s through the Change arm |
  | `ai_close` | 333×360 generated level, P2 CPU, P1 fuzz | 3,000 | a `rand(16)` fallback; a non-drawing fallback arm; `real_dist == 0` or `< 3` at least once, or record the minimum; ≥ 1 rope-attached Change tick; ≥ 1 `reacts` press |
  | `hp_killemall` | P1 50, P2 300, MAX BONUSES 10, fuzz both | 3,000 | clamps; ≥ 1 health bonus picked by each worm (else record the one that did); P1 drips under 12; each respawn to its own max |
  | `hp_scales` | Scales of Justice, P1 30, P2 200, lives 5 | until game over + 200 or 3,000 | Scales transfers onto each worm; a wrap into an extra life for P1 (at 30) |
  | `hp_tag` | Game of Tag, P1 1000, P2 10, TIME TO LOSE 60 | until game over + 200 | P2 deaths ≫ P1 deaths; `IsGameOver` flips once |

  No level below 342 rows (e-1 Addendum G3). The ledger witnesses come from `run_ais_traced` and the driven state (D12).
- [ ] **Step 2: write** each case's `_setup.cfg` (via `cfg`, C++-schema TOML from `settings_to_toml`) and `_scenario.txt` (`seed`, `generate`, `ticks`, `settings`, `ai` for the `ai_*` cases, `weapsel` for `ai_vs_ai`, `input` lines for humans, the ledger header).
- [ ] **Step 3: `gen_sim_slice4_5f_golden.sh`** (§Formats). Run it with `source $S/env.sh && CHECKED_DUMPER=$S/build-chk/oracle_dump_sim_physics bash rust/oracle-tests/gen_sim_slice4_5f_golden.sh` (background, log). Every awk gate passes, and the checked dumper writes the same bytes for all 8.
- [ ] **Step 4: the gate test** `sim_slice4_5f_golden.rs`. Parse the committed scenario, `settings_from_toml` on its sidecar, generate the level, then `build_match` — or, with `weapsel`, 4½c's selection harness (`sim_slice4_5c_continuation_golden.rs`'s route through `weapsel_common`, read-only) — and compare row 0. Then per tick:
  1. `inputs[h] = scripted` for a human, `inputs[c] = sim.worms[c].control_states` for a CPU (the persisted word, fact 14);
  2. with `ai`: `run_ais(&mut ais, &sim, &mut inputs)` with `ais[c] = Some(DumbLieroAi::new())` built once; compare columns 13–16 of the next row (`inputs[c].pack()` and `ais[c].rand.last()`);
  3. `sim.process_frame(&inputs)`; compare the 11 hash columns and `is_game_over` (column 12).
  Report the first differing tick and column. A witness-guard test re-runs every ledger claim from the committed header.
- [ ] **Step 5: the fix loop.** On a mismatch, find the first tick and column. An AI-column mismatch with equal hash columns is an AI bug: diff the traced arm against the C++ source line by line (the draw list of fact 3 first). A hash mismatch with equal AI columns is a per-worm health or `reacts` bug (T1). Each fix gets a unit test pinning the C++ line and the full re-diff; commit it separately. **Never** edit a golden; a dumper bug is fixed in T4's file with its regeneration proof re-run, and only the f-1 goldens are regenerated.
- [ ] **Step 6: gate.** `cargo test --manifest-path rust/Cargo.toml -p oracle-tests --test sim_slice4_5f_golden` green (**G-AI + G-HP bit-exact**), and the full re-diff green. The golden audit shows exactly the 24 new `A` lines.
- [ ] **Step 7: commits.** Fixes first: `sim(4.5f-1): <what> (found by G-AI|G-HP <case>, tick N)`. Then `oracle(4.5f-1): G-AI + G-HP — DumbLieroAI's per-tick words and RNG and unequal-health matches bit-exact vs the real C++ (8 generated cases, the human-vs-CPU golden of Hard gate 4)`.

**Done when:** all 8 cases are bit-exact on every row and column under the release and the checked dumper, every witness holds, and any sim fix is separate and re-diffed.

---

### Task 7 (Batch 6): `ui` — `Match` drives the AIs; RESUME's palette; the touch rule split

**Files:** `rust/ui/src/shell/{playing.rs, selection.rs, mod.rs, overlay.rs}`, and callers only if a signature moves.

- [ ] **Step 1: the AIs in `Match`** (D8; facts 5, 18). `ais: [Option<DumbLieroAi>; 2]` built in `Match::start` (both routes) from `settings.worm_settings[i].controller == 1`, with D1's `debug_assert!` for 2. In `process`, on a match tick: `let mut inputs = self.edges.apply(&inputs, &sim.worms); run_ais(&mut self.ais, sim, &mut inputs); tick_viewports(…, &inputs)`. Nothing in selection. `pub fn is_cpu(&self, i: usize) -> bool` for the harness and the glue.
- [ ] **Step 2: RESUME's palette** (D9). `Match::focus` ends with `focus_palette(&mut self.scene, &self.cfg.settings)`.
- [ ] **Step 3: the touch split** (D3). In `selection.rs`: `pub const BOT_WEAPONS_RANDOM: u32 = 0`; `pub fn touch_settings(s: &mut Settings)`; `new_game_config(settings, touch_only)` sets only `select_bot_weapons = BOT_WEAPONS_RANDOM` on a touch-only page; `live_config` keeps the 4½c rule under a private helper. In `Shell::apply_picked`'s `Picked::Setup` arm, after a successful load and before `update_items`: `if self.options.touch_only { touch_settings(&mut self.world.settings) }`. Update the module docs (the 4½c Q8 text) and `the_new_game_config_is_the_settings_plus_the_touch_rule`.
- [ ] **Step 4: headless flows** (`ui/src/shell/mod.rs` tests, through `Shell::frame`; the existing `boot` / `start_match` / `tap` helpers):
  - **CPU match:** settings with P2 `controller = 1`, KEEP → NEW GAME → selection: P2 ready on its first frame, P1 DONE → 1,500 ticks with **no P2 input**: P2's word is non-zero on some tick, P2's position changes while visible, P2 dies and becomes visible again at least once (pin a match seed found by the test's author and assert on it), `match.is_cpu(1)`.
  - **Equivalence:** for the same run, a hand driver that replays `edges.apply` + `DumbLieroAi` in `(k + cycles % 2) % 2` order over a cloned sim gives the same word every tick.
  - **Not in selection:** P2 CPU with PICK → during selection P2's word is exactly the sampled P2 keys' (no toggles), and P2's keys drive its menu; the AI starts on the first match tick.
  - **The Esc fade and the post-mortem:** ticks during the fade and after game over still run the AI (P2's word changes on some of them).
  - **Fresh per NEW GAME:** two NEW GAMEs with the same seeds give the same first 200 P2 words (the AI's RNG restarts at 0x1337).
  - **Determinism** (D13): two `Shell` runs with the same seeds and inputs give identical per-frame `state_hash`es over 1,000 frames.
  - **The skip route** (`boot_playing`, `?weapons=`) with P2 CPU: the AI runs from tick 0.
  - **RESUME palette:** attached, edit `worm_settings[0].rgb` in the settings while paused → RESUME → `match.origpal()` has the new ramp; detached (LOAD SETUP) → the old ramp; unchanged colours → `origpal` byte-identical.
  - **Unequal health:** NEW GAME with 40 / 250 → the two maxes; RESUME after changing P1 to 30 while attached → max 30 and the clamp on the next tick.
  - **Touch:** `new_game_config(&s, true)` has `select_bot_weapons == 0` and P2's controller from `s`; a touch-only shell after LOAD SETUP of `orbmit` has P2 `controller == 1`; a desktop shell keeps 0; a touch-only NEW GAME with P2 CPU makes P2 ready at once with RANDOM picks (the constructor draws — allowed live).
- [ ] **Step 5: gate.** The re-diff (both), the wasm check, G1 and all 28 G2 cases green (none has a CPU player, fact 15, so the AI must be invisible to them), the three `-- check`s clean, the golden status empty.
- [ ] **Step 6: commit** `ui(4.5f-1): Match runs DumbLieroAI after the key edges and before the tick (never in selection), RESUME re-applies Game::Focus's worm ramps, the touch rule makes player 2 a CPU in the settings and BOT WEAPONS RANDOM at NEW GAME`.

**Done when:** every flow above is green and every prior gate is unchanged.

---

### Task 8 (Batch 7): G2f-1, the CPU through the real `Gfx::RunOneFrame`, and 🎯 MILESTONE f-1

**Files:** `rust/oracle-tests/tests/shell_common/mod.rs`, new `tests/shell_f1_cases/mod.rs` and `examples/gen_slice4_5f1_shell.rs`, `tests/shell_golden.rs`, the 14 `golden/shell_*` f-1 files; `rust/ui` / `rust/sim` for proven fixes only.

- [ ] **Step 1: `shell_common`.**
  - **Validators** (each refused at generate time): a NEW GAME with a `controller == 2` player (the FollowAI guard's mirror); every existing validator stays (the constructor-draw one is D2's guard).
  - **Ledger fields:** per frame, which players are CPUs (`Match::is_cpu`); per match tick, the `AiTrace` arms (through a `ShellDebug` hook that asks `Match` to keep its last traces, or by re-running `run_ais_traced` on a clone — pick the one that does not change `Match`'s behaviour, and unit-test that it does not); per worm deaths (`visible` true → false) and respawns (false → true); `p2_keys`: whether any P2-bound key event occurred in a match phase.
- [ ] **Step 2: the 4 cases** (`shell_f1_cases::cases()`), all `detail`, every `match_seed` scripted, each ending by QUIT unless stated:
  1. **`cpu_match`** 🎯. `fs` with `user/Setups/liero.cfg ← golden/shell_cpu_match_user_liero.cfg` (defaults, but P2 `controller = 1`, `selectBotWeapons = 2`). NEW GAME → selection (P2 ready at once; P1 DONE) → ~1,500 match frames with P1's keys scripted (walk toward, fire, rope, change) → Esc (every worm key released first, pitfall 8) → the fade → F1 RESUME → play until each worm has died at least once and the CPU has respawned → Esc → QUIT. About 2,000 frames. The `file` line is the exit save.
  2. **`cpu_vs_cpu`**. `setup shell_cpu_vs_cpu_setup.cfg`: both CPU, KEEP (D2), lives 2 (lives 1 if the generator's search finds no game over within 8,000 frames at 2). NEW GAME → selection ends on its first frames with no key → play to game over → the 180-frame post-mortem → back to the menu → QUIT.
  3. **`cpu_pick`**. `setup shell_cpu_pick_setup.cfg`: P2 CPU with PICK. NEW GAME → P2's arrows move its cursor and change a weapon, Right Ctrl on DONE (P2's keys drive the bot's menu) → P1 DONE → 300 match frames (the AI starts only now) → Esc → QUIT.
  4. **`hp_boot`**. `fs` with `user/Setups/liero.cfg ← golden/shell_hp_boot_user_liero.cfg` (healths 40 / 250). Boot (no sanitising: the boot `d` line's `cfg16` is the file's) → NEW GAME → DONE → play until a death and a respawn (the lifebars of two different maxes) → Esc → QUIT.
- [ ] **Step 3: witnesses** (the generator prints each ledger; `write` refuses a case that lacks its own; every case has no violations):

  | Case | Witnesses |
  |---|---|
  | `cpu_match` | P2 is a CPU on every match frame; `p2_keys` false; ≥ 1 death of each worm; ≥ 1 CPU respawn; AI arms seen: Fire toggle, walk, Change; the Esc fade frames carry changing P2 words |
  | `cpu_vs_cpu` | both CPUs; no worm key in the match; game over; the post-mortem's P2 words change; back to the menu |
  | `cpu_pick` | P2's cursor moved by P2's keys on `W` frames; P2's word during `W` equals its sampled keys; the first non-zero AI-made word on the first match tick or later |
  | `hp_boot` | the boot `cfg16` equals the fixture file's; maxes 40 and 250; a lifebar drawn for each; ≥ 1 death and respawn |
- [ ] **Step 4: `gen_slice4_5f1_shell.rs`.** `check` or `write <golden dir>`: it writes the user inputs (`settings_to_toml` of the stated `Settings`), the setups and manifests, then the 4 scripts with the two-line header (case + ledger), and asserts no violations and every witness. **Run:** `… --example gen_slice4_5f1_shell -- write /home/user/openliero/rust/oracle-tests/golden`, then `-- check`, then the three older `-- check`s.
- [ ] **Step 5: the C++ goldens.** `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh` (count 32; background, log). Every awk gate passes, the 28 prior goldens stay byte-identical, and the checked loop over all 32 is `SAME` with nothing reported under ASan.
- [ ] **Step 6: `shell_golden.rs`.** `mod shell_f1_cases;`; `the_committed_scripts_are_the_generators` covers the union of 32; new tests `the_f1_milestone_is_bit_exact` (`cpu_match`, with its ledger asserted) and `every_g2f1_case_is_bit_exact`; and a negative control `#[should_panic] the_cpu_match_without_its_ai_diverges` (the same drive with a `ShellDebug` switch that disables `Match`'s AIs — it must mismatch on the first match tick where the CPU's word differs from its keys', which shows the gate sees the AI).
- [ ] **Step 7: the fix loop.** `cargo test … -p oracle-tests --test shell_golden`; on the first mismatch, PPMs of both sides around it (`SHELL_RUST_PPM_DIR=$S/f1b7/r`; C++: `SHELL_PPM_DIR=$S/f1b7/c bash rust/oracle-tests/gen_shell_golden.sh`, which rewrites the same bytes). A `d`-line `state8` mismatch with equal frames is a sim/AI bug; fix it in `ui`/`sim` with a unit test pinning the C++ line and the full re-diff. Never edit a golden.
- [ ] **Step 8: gate.** `shell_golden` (all 32 + the negative control) green; the full re-diff green; `menu_widget_golden` green. The golden audit: `git diff --name-status c2d58fe -- rust/oracle-tests/golden | grep -v '^A'` empty, and `| wc -l` = **38**.
- [ ] **Step 9: commits.** Fixes first, `ui(4.5f-1): <what> (found by G2f-1 <case>, frame N)`. Then `oracle(4.5f-1): 🎯 MILESTONE f-1 — G2f-1 (a human vs the CPU, CPU vs CPU to game over, a PICK bot driven by its keys, unequal healths from liero.cfg) bit-exact against the real C++ Gfx frame loop, the real LocalController running the real DumbLieroAI`. Stage exactly the 14 golden files and the four source files.

**Done when:** all 4 f-1, 6 e-2, 11 e-1 and 11 4½d cases match on every line, and the negative control panics as expected.

---

### Task 9 (Batch 8): `game` — the phone's player 2 is the CPU, `?cpu=`, the page

**Files:** `rust/game/src/{main.rs, touch.rs, web_params.rs}`, `web/index.html`, `.github/workflows/preview.yml`.

- [ ] **Step 1: `?cpu=`** (D4). `MatchParams::cpu`, its parse (with the file's warning style), `apply_cpu`. **Tests:** `cpu=0|1|2`; `cpu=3` and `cpu=x` warn and leave `None`; `apply_cpu` over the loaded settings for each value and for `None` with `touch_only` true/false; `skips_menu()` is unchanged by `cpu`.
- [ ] **Step 2: the boot.** In `setup` (`main.rs:531-549`), after `apply_level`: `preview.0.apply_cpu(&mut settings, touch_only())`. `StartOptions.touch_only` is unchanged.
- [ ] **Step 3: the stand-in goes** (Q3). Delete `BotRespawn`, its `ShellRes` field and construction, the `sampled[1]` override in `tick_and_render`, and its test. Keep `waiting_to_respawn` and its doc, reworded (it now serves player 1 only). `publish_respawn` follows D15.
- [ ] **Step 4: hooks** (D16). `worm_hooks` + `WormHook` in `touch.rs` with a native test; the wasm publisher sets `window.lieroWorms` each presented frame in phase `game` and `window.lieroControllers` with the other hooks.
- [ ] **Step 5: the pinned walk seed** (D17). A native test (`touch.rs` tests, or `rust/game/tests/cpu_walk.rs` if it needs the store): the browser store's settings, `?touch=1&seed=S&weapons=<W>` through `MatchParams`, `apply_cpu(.., true)`, `Shell::boot_playing`, then idle frames: assert P2 visible, then invisible, then visible again within 1,500 match ticks, and P2 moved while visible. `S` and `<W>` are constants with a comment on how they were found.
- [ ] **Step 6: the page and the preview comment** ("player 2 is the CPU"):
  - `web/index.html`: the URL help block gains `?cpu=1` (player 2 is the CPU) and `?cpu=2` (both); the Player 2 key help gains "(on a phone, or with `?cpu=1`, player 2 is the CPU)"; the 4½c comment above `lieroTouchOnly` says the game makes player 2 the CPU (random weapons each match); `HINTS.weapsel`'s `'P2: a bot'` becomes `'P2: CPU'`.
  - `.github/workflows/preview.yml`: a new example `- [Play against the CPU (desktop keys)](${url}/?cpu=1)`; the URL-parameter line gains `cpu` (`1` player 2 is the CPU, `2` both); the respawn sentence ends "on a phone the CPU (player 2) respawns by itself"; the phone sentence says "player 2 is the CPU (random weapons each match)"; one line: "With `?cpu=1` on a keyboard, player 2's weapons are picked with player 2's keys, as in the original (BOT WEAPONS = PICK)".
- [ ] **Step 7: gate.** `cargo test --manifest-path rust/Cargo.toml -p game`, the wasm check, the re-diff without `game`. Build the bundle, serve it on 8765, and run three walks (copies of `$S/b8.mjs` / `$S/tap1.mjs` / `$S/deathdiag2.mjs` as `$S/f1b8/{phone,seed,desk}.mjs`; `ok`/`FAIL` lines; screenshots in `$S/f1b8/shots/`):
  - **Phone, the real flow** (`/?touch=1`, emulated phone, CDP touch): `lieroControllers` is `[0,1]` at boot; pad to NEW GAME, FIRE → `lieroPhase === 'weapsel'`, P2 already ready (the selection screen shows it); pad to DONE!, FIRE → `game`; within 120 s: P2 visible; P2's `x`/`y` change by ≥ 8 pixels while visible (it moves); P2's `word` has the Fire bit on some sample while visible (it fires); the P1 banner appears while P1 waits for FIRE and never for P2; no page errors.
  - **Phone, death and respawn** (`/?touch=1&seed=S&weapons=<W>`, no input at all): within 240 s P2 goes visible → invisible → visible (the CPU respawned by itself); no page errors.
  - **Desktop** (`/?cpu=1`): `lieroControllers === [0,1]`; NEW GAME → selection: P2 not ready (PICK) → P2's keys (↓ to DONE!, Right Ctrl) ready it → P1 DONE → P2 acts as above. `/?cpu=2&seed=S&weapons=<W>`: both worms move with no input.
  - **Regression:** the e-2 phone walk (a copy of `$S/e2.mjs`'s phone part) still passes, with its `'P2: a bot'` expectation updated if it reads the hint.
- [ ] **Step 8: commit** `game(4.5f-1): the phone's player 2 is the real CPU (random weapons each match; the stand-in is gone), ?cpu=1|2, the walk hooks, the respawn banner for a human player 1 only; the page and the preview comment`.

**Done when:** the `game` tests and the wasm check pass, every walk line is `ok`, and the non-shell paths are unchanged (`round_trip`, `record_regression` green).

---

### Task 10 (Batch 9): eyeball artefacts, docs, audits, broad review

**Files:** `docs/superpowers/liero-rs-PROGRESS.md`, the overview, the design (status line), the cpp-map, the rust-map, `.claude/skills/liero-shot/SKILL.md`. Re-read each immediately before editing it, because line numbers move.

- [ ] **Step 1: the full green board.** The re-diff (both), the wasm check, `cargo tree -p ui -e normal | grep -c bevy` → `0`, `cargo tree -p sim-core --depth 1` has no dependencies, `cargo tree -p sim --depth 1` lists what it listed at `c2d58fe`.
- [ ] **Step 2: tripwires and audits.**
  - `grep -rnE "HashMap|HashSet|\bf32\b|\bf64\b|SystemTime|Instant" rust/sim/src/ai.rs rust/ui/src` → empty.
  - `git diff --name-status c2d58fe -- rust/oracle-tests/golden | grep -v '^A'` → empty, and `| wc -l` → **38**.
  - `git diff --name-only c2d58fe -- src CMakeLists.txt` → exactly the two dumpers.
  - `git diff --name-only c2d58fe -- rust/oracle-tests/examples` → the two new generators and `gen_slice4_5e1_sim.rs` (D6's line only: `git diff c2d58fe -- rust/oracle-tests/examples/gen_slice4_5e1_sim.rs` is one changed line).
  - `grep -c "ResMut<Sim>" rust/game/src/main.rs` equals its count at `c2d58fe`.
  - Both dumpers pass clang-format 22 on the whole file; `scripts/clang-tidy-diff.sh build/linux-x64 c2d58fe` exits 0.
  - **Reproducibility**, in one shell: the four `-- check`s (4½d, e-1 shell, e-2, f-1); `source $S/env.sh && bash rust/oracle-tests/gen_shell_golden.sh && CHECKED_DUMPER=$S/build-chk/oracle_dump_sim_physics bash rust/oracle-tests/gen_sim_slice4_5f_golden.sh`; then `git status --porcelain rust/oracle-tests/golden` → empty; then the `$S/build-chk` shell loop → 32 `SAME`.
- [ ] **Step 3: Xvfb side-by-side** (eyeball only). Copy `data/` to `$S/f1x/root`, put `shell_cpu_match_user_liero.cfg` in as `Setups/liero.cfg`, run the real `openliero --config-root $S/f1x/root` (with `OPENLIERO_DATADIR=data`) and play `cpu_match`'s opening with xdotool; the Rust side is the browser bundle with `?cpu=1` on the same keys. Snap about 6 moments: selection with the CPU ready (KEEP) or picking (PICK), the CPU walking, firing, dying, respawning, the pause. Pair with `x12_pair.py` into `$S/f1x/side/`. Timing, the level and the seed differ by nature; the CPU's behaviour class (walks toward, fires in range, respawns alone) must be recognisably the same. Never commit the PNGs.
- [ ] **Step 4: `liero-shot` §7.** One paragraph after the e-2 one: the CPU player (`sim::ai`, `Match`), per-worm health, the phone's CPU, `?cpu=`, the `ai` directive, and the gates (G-AI, G-HP, G2f-1).
- [ ] **Step 5: PROGRESS.**
  - Real date. A new header paragraph, **"4½f-1 LANDED"**; the previous one becomes "Prior (…)".
  - The Step 4½ tree: split 4½f into ✅ 4½f-1 (with the milestone) and ⬜ 4½f-2.
  - Correct the two "random player names (4½f)" lines (fact 21, finding 4).
  - **Also open for John (4½f-1)** / notes: the design findings confirmed or sharpened (1, 2, 6, 10, 11) and the plan-time facts (1's F8 `Seed`, 3's conditional Fire draw, 8, 15–17, 21); T0's outcomes, P2's observed `reacts` above all; D1 (the refusal at NEW GAME only), D2 (no RANDOM in G2), D3 (the touch split and LOAD SETUP's re-apply), D4 (`?cpu=`), D6 (the one frozen-file line), D13 (no recorded-CPU test yet), D15; the Rust-only box; the known C++ UB (`reacts`) as a documented divergence only where C++ is UB; "`?cpu=1` on a keyboard needs P2's keys in weapon selection (PICK)".
- [ ] **Step 6: the overview.** The status line: `4½f-1 LANDED`. The 4½f bullet: "**4½f-1 landed**", with the plan path. **Q3** (the AI seed): answered — `mt19937(0x1337)` per CPU per NEW GAME, no seed override (finding 1, T0 P1, check 3″). **Q4** (key names): corrected to C++'s hard-coded table (finding 5; the port itself is 4½f-2).
- [ ] **Step 7: the maps and the design.** **cpp-map:** facts 1–8, 11, 14–16 and finding 2 at the relevant sections (the AI, `LocalController::Process`, weapon selection, health). **rust-map:** `sim::ai`, `WormState::{reacts, max_health}`, `SimState::ai_params`, `Match::ais`, `refuse_follow_ai`, the touch split, `?cpu=`. **Design status line:** `**4½f-1 LANDED** (plan plans/2026-09-27-liero-rs-step4.5-slice4.5f1-plan.md); 4½f-2 planned`.
- [ ] **Step 8: the broad review.** Re-read `git diff c2d58fe -- rust src web .github` against the design and this plan:
  - each finding and fact is ported and gated (name its G-AI/G-HP case, G2f-1 case or unit test) or recorded in PROGRESS;
  - `sim::ai` follows `worm.cpp:477-696` line for line, with fact 3's draw list;
  - `Match::process` follows `localController.cpp:122-181`;
  - the dumpers' interventions are documented, touch no code under test, and refuse what they cannot run faithfully;
  - LD 3–6 and the crate rules hold; `game` is glue only; the non-shell paths are unchanged;
  - no push, no PR.

  Bar: 0 Critical / 0 Important.
- [ ] **Step 9:** `git status --short` lists only the docs above. **Commit** `docs(4.5f-1): PROGRESS + overview + maps — slice 4.5f-1 landed (DumbLieroAI, per-worm health, the phone's CPU, ?cpu=; G-AI + G-HP + G2f-1)`.

**Done when:** the board is green, every audit matches its expected output, the docs are updated, and the review is clean.

---

## Known pitfalls (carried from e-1, e-2, 4½d and PROGRESS, plus f-1's own; read them before your batch)

1. **Goldens are C++ truth.** Never regenerate one to make Rust pass, and never hand-edit one. An `M` under `golden/` means: stop, restore, report. A dumper change must re-prove byte-identity for every golden its binary writes: all 40 sim_physics gen scripts (T4) and all 28 shell goldens under the release **and** the checked build (T5).
2. **DEBUG only.** `--release` breaks the `#[should_panic]` tests. The one non-test build is the wasm-release bundle.
3. **clang-format 18 is on PATH.** Always use `$CF22`, and always run the whole-file dry run too.
4. **This clone has no `origin/master`.** Pass `c2d58fe` to both diff scripts.
5. **Gen scripts default to `macos-arm64`.** Source `env.sh` in the same shell.
6. **rustfmt recurses** through `lib.rs`/`main.rs`. Use it only on files you created.
7. **Long commands in the background.** The container restarted once; C++ builds, gen scripts, `cargo test` and the walks go to a log under `$S` that you poll.
8. **`gen_sim_slice5prime_pickup_weapon_golden.sh` hangs here** (a pre-existing C++ `cossin[128]` read, PROGRESS 4½e-1). Run the sim regeneration loop with `timeout` and report it by name; its committed golden still passes the Rust test.
9. **The AI reads `cs`, not `state.worms[w].control_states`.** The state's word is last tick's, before this tick's key edges; C++'s AI sees the post-key word.
10. **Every `rand(k)` draws, even `k` 0 or 1.** Never short-circuit `rand(k) == 0`. The Fire draw is conditional (fact 3), the Jump and Change draws are not, and six of the ten fallback arms draw.
11. **`delta /= kRealDist` truncates toward zero per component**, and index 128 of `cossin` is never read by the scan.
12. **`reacts` persists and is not hashed.** Never reset it on death, respawn, `ResetWorms` or a new tick; only a visible `Process` rewrites it.
13. **`max_health` defaults to 100, not the start health.** A legacy scenario's `worm` line sets `health` against default settings.
14. **FollowAI is refused at NEW GAME only**, never in `build::validate` (three committed sim setups have a controller-2 player 2) and never at RESUME.
15. **No RANDOM bot in a shell case** (intervention 3's precondition, D2). BOT WEAPONS is global: the phone's RANDOM applies to every bot.
16. **A CPU still receives the keys bound to it.** A case that must show the CPU alone presses no P2 key in a match phase (`p2_keys` false); `cpu_pick` presses them on purpose in selection only.
17. **CPU worms have no `input` in a sim scenario.** Their words persist from the last tick; a non-zero `input` for a CPU is refused on both sides.
18. **The selection constructor must draw nothing in a shell case** (intervention 3), and every NEW GAME's picks must name enabled weapons.
19. **A worm key held through Esc and released in the menu** stays pressed in C++ and not in Rust (4½d fact 12). Release every worm key before the Esc.
20. **Holdazone is refused in Rust and plays in C++.** No case may NEW GAME or RESUME into Holdazone.
21. **No level shorter than ~342 rows is ever played** (e-1 Addendum G3). `ai_close` uses 333×360.
22. **Headless Chromium runs at about 7–10 fps.** The worms start dead: wait about 30 s after a match starts. A tap longer than 12 ticks trips weapon selection's key repeat (`frameTap`). iOS raises the keyboard only from a real tap.
23. **The native `game` cannot run here** (no GPU). The browser bundle, the headless `Shell` tests and the G2 harness cover the live path. Restart `http.server 8765` if it is down.
24. **Disk.** Clean the incremental dirs after every batch, and check `df -h /` before any wasm-release build. Rust batches are sequential.
25. **Explicit `git add` paths only.** Never commit the scratchpad, PPMs, PNGs or fixture copies.
26. **Trailers.** Every commit carries `$CO` and `$SESS`, and nothing else names a model. No "Generated with …".

## Done-report (each batch)

Each batch reports:
- (a) what changed and why;
- (b) the files touched;
- (c) the gate commands run, with their results;
- (d) the commit SHAs.

Batch 1 also reports the addendum's verdicts. The final report (Batch 9) surfaces:
- T0's outcomes: P1 (the fresh AI RNG), P2 (the observed `reacts` values and whether zeroing moved the gate), P3, P5, P6, P7;
- the dumper evidence: T4's and T5's regeneration proofs (40 sim scripts with the known hang named; 28 shell goldens under both builds) and their smokes;
- the G-AI / G-HP result: 8 cases, their rows and witnesses, the Hard-gate-4 human-vs-CPU golden named, any sim fix with its case and tick;
- 🎯 the G2f-1 result: 4 cases, the frame and `d`-line counts, `cpu_match` named on its own, the negative control;
- the Chromium lines (phone real flow, phone death/respawn with the pinned seed, desktop `?cpu=1|2`, the e-2 phone regression);
- the Xvfb PNG paths;
- the audit sweep: 38 `A`, 0 `M`; the two dumpers; the one frozen-file line; `cargo tree` 0 bevy; reproducibility byte-identical.

---

## Addendum T0 (probe results)

Run 2026-09-27 on `claude/cpp-oracle-vcpkg-assets-chcwcm` at `277b4a1`, with the real `Gfx::RunOneFrame` in a temporarily patched `oracle_dump_shell` and the real `Worm::Process` in a temporarily patched `oracle_dump_sim_physics`. Every artefact is in `$S/t0f1/`. Nothing but this addendum is committed.

**Verdict.**
- P1–P7 are **confirmed**. No contradiction rule fired, so no task text changes (§"Changes to later tasks").
- **P2 took the "the fill moves `state8`" branch: D5 is necessary**, and it is more than a formality. The C++ garbage is non-zero in most runs, it depends on ASLR in the release build, and the checked build fills it with `0xBEBEBEBE`. Without the fill, a CPU match's `d` lines are not reproducible run to run, and the release and checked builds disagree. With the fill, they agree byte for byte.
- Two findings sharpen later tasks without changing a rule:
  - the boot controller also builds a `DumbLieroAI` (never processed);
  - two KEEP bots finalise selection on the first `W` frame.

  Both are in the notes below.

### Method

- **The patch** is `$S/t0f1/probe_patch.diff` (split as `shell_probe.diff` and `sim_probe.diff`). It lived in the working tree only.
  - `oracle_dump_shell`:
    - `#include "ai/predictive_ai.hpp"` and `"worm.hpp"`;
    - `DetailLine` prints `P` for `cur_menu == &gfx.player_menu`;
    - after every `d` line, a probe-only `x <frame> ws=<InWeaponSelection> cyc=<cycles>` line, then per worm of `CurrentGame()`: `ctl ai(0 none | 1 dumb | 2 follow) fresh(rand == Rand()) last(%08x) rs(FNV-1a-64 of rand.serialize()) reacts cs(%02x Pack) vis hp=<health>/<settings->health> lives wpn x y`;
    - in intervention 3's block, after `game.rand.Seed`: every worm's `reacts` is recorded as `|| raw w0=… w1=…` on that frame's `x` line, then zeroed only under `PROBE_ZERO_REACTS=1`.
  - `oracle_dump_sim_physics`:
    - under `PROBE_HP=1`: the RNG draws of each `worm->Process` are counted by stepping a copy of the pre-call engine until it equals `game.rand.engine`;
    - after each `dump`: `H <t> <w> <health> <max> <lives> <visible> <draws> <cs> <bonuses>` to stderr;
    - under `PROBE_PASSIVE=<w>`: worm `w`'s word becomes Fire only while it is invisible and 0 while visible, so it respawns but never fires (a clean blood-gate signal).
- **Build:** `source $S/env.sh && cmake --build build/linux-x64 --config Release --target oracle_dump_shell oracle_dump_sim_physics`, and `cmake --build $S/build-chk --target oracle_dump_shell oracle_dump_sim_physics` with the same patch.
- **Fixtures:**
  - `$S/t0f1/gen.py` writes the shell scripts and setups:
    - setups: `keep.cfg`, `random.cfg` and `pick.cfg` are `data/Setups/liero.cfg` with `[player2] controller = 1` and `selectBotWeapons` 2 / 0 / 1; `both.cfg` has both players on 1 with KEEP; `def.cfg` is the shipped file;
    - every script has `setup <cfg>`, `boot_seed 7` and `detail`, and ends by QUIT (Esc, 35 frames, Esc, Return);
    - taps: down at *t*, up at *t*+2, the next key at *t*+3.
  - `$S/t0f1/gensim.py` writes the P7 scenarios: `settings` + `generate <level_seed>` (504×350) + 4½a fuzz (`mt19937(input_seed)() & 0x7f`, worm 0 then worm 1; its seed-4545 stream reproduces `sim_slice4_5a_scales_scenario.txt`'s `input` lines).
  - `$S/t0f1/scan.py` is a seed scan of Kill'em All, P1 50 / P2 300, MAX BONUSES 10, 3,000 ticks. Its log is `scan.log` / `scan2.log`.
- **Runs:**
  - `build/linux-x64/Release/oracle_dump_shell $S/t0f1/<s>.txt $S/t0f1/<s>.out [--ppm-dir …]`;
  - checked: `ASAN_OPTIONS=detect_leaks=0 $S/build-chk/oracle_dump_shell …`;
  - `PROBE_HP=1 [PROBE_PASSIVE=w] build/linux-x64/Release/oracle_dump_sim_physics $S/t0f1/<c>.txt $S/t0f1/<c>.out 2> $S/t0f1/<c>.h`.
- **Analysis:** `p1.py` (the AI-RNG freshness per frame class), `anh.py` (P7 events) and `scan.py` (P7 bands). The PNGs are in `$S/t0f1/png/`.

### P1: the AI seed (`s_seed`, `s_both`): CONFIRMED

`s_seed` (P2 CPU, KEEP, `match_seed 1101, 1102`):
- **NEW GAME 1:**
  - F1 at f 40; the new controller at **f 73** (upd `M`, `ws=1`).
  - P2 is `ai=1 fresh=1 last=00000000 rs=80fdc060c80f19ce` on f 73 and on every `W` frame (74–77).
  - It stays so on the finalise frame **f 78** (P1's LCTRL; `ws=0`, `lives=15`).
  - The first `G` tick (**f 79**, `cycles` 1) changes it to `last=2af09813`.
- **NEW GAME 2** (Down, Return at f 419/422, over the freed first controller):
  - the new controller at **f 455**, `fresh=1 last=00000000`, the same `rs` as a fresh `Rand()`;
  - the same through the `W` frames and the finalise frame f 460;
  - the first tick (f 461) again gives `last=2af09813`.
- **Both release runs and the checked run** agree on every `last` / `rs` value (the AI's draws do not depend on `reacts`; see P2).

**Two CPUs** (`s_both`, both KEEP):
- On f 73 both are `fresh=1`, with equal `rs`.
- From the first tick both carry **identical, separate** streams: `2af09813`, `5fcd70c1`, `3e6cd695`, …, equal on every tick. A shared RNG would put one worm a draw ahead.

**Also observed** (no rule; notes for Batch 4):
- The **boot controller** (`InitFrameStepping`, the menu background) also builds a `DumbLieroAI` for a controller-1 player: f 0–72 show `ai=1 fresh=1`. It is never processed, and it is replaced at NEW GAME, so D14's "every frame that made a controller" check is the right scope.
- With **two KEEP bots, selection finalises on f 74**, the first frame after the NEW GAME frame, with no key.

### P2: `reacts` (`s_seed` ×2 release, ×1 checked, each with and without `PROBE_ZERO_REACTS`): the fill MOVES `state8`, so D5 is CONFIRMED AS NECESSARY

**What the new controller's worms held** (`raw`, on the new-controller frame; unchanged on the finalise frame without the fill; `r0,r1,r2,r3` = Down, Left, Up, Right):

| Run | NEW GAME 1 (f 73 / finalise f 78), P2 | NEW GAME 2 (f 455 / f 460), P2 | P1 (human) |
|---|---|---|---|
| release, twice (identical) | `0,0,0,0` | `1919958048,1852142437,540963700,1701998652` (`0x72703c20 …`: bytes of the dumper's own `# f … <presents>` header text, from the reused heap) | NG1 `0,0,0,0`; NG2 `0,0,1331314944,4279119` |
| checked (ASan) | `-1094795586` ×4 (`0xBEBEBEBE`, ASan's malloc fill) | the same | the same |
| release, other scripts (NG1) | `s_keep` `0,0,668206608,21953`; `s_pick` `0,0,-1038300864,22016`; `s_hp` `-170268880,22035,-170259344,22035`; `s_rgb`, `s_rgb0`, `s_both` `0,0,0,0` | — | pointer halves (`21953` = `0x55c1`, `22016` = `0x5600`: **ASLR-dependent**, different on every run) |

**The effect** (f/d lines, without vs with the fill):
- Release NEW GAME 1 held 0 and matches.
- Release NEW GAME 2: the first `state8` difference is **f 611 = its tick 150**. On f 610 P2 is placed for its first spawn (`x=468 y=191`, still `vis=0`), so its AI walks. The garbage `reacts[kRfRight]` / `[kRfDown] > 0` then presses Right: P2's word is **`0d`** (Up+Left+Right) against **`05`** with the fill (`worm.cpp:680-686`).
- Checked NEW GAME 1: the first difference is **f 229 = tick 150**. P2's word is **`49`** (Up+Right+Jump, because `reacts[kRfDown]` is negative) against **`09`**.
- **With the fill, the release and checked outputs are byte-identical** on every `f`/`d` line. Without it they differ from f 229.

So:
- The stale read happens in the **placed-but-invisible** window before the first spawn (and after every death), exactly where D5's persisted Rust `reacts` (0 from `from_init`, then last visible tick's value) is read.
- **No `reacts` write before the first AI read was found**: the value on the finalise frame equals the constructor's garbage, so fact 2 stands.
- **Human worms:** `s_rgb` and `s_rgb0` hold different P1 garbage (`0,0,945293472,21861` vs `-699465776,21920,-697395120,21920`) and give **equal `state8` on all 558 frames** (two idle visible humans for about 330 ticks). This is consistent with fact 2 ("no human path reads it stale") and with D5's claim that intervention 3′ is a no-op on every existing case, which T5 Step 4 proves.

### P3: CPU weapon selection (`s_random`, `s_keep`, `s_pick`): CONFIRMED

- **RANDOM** fails on its NEW GAME frame: `frame 73: the WeaponSelection constructor drew the RNG (intervention 3's precondition)`. D2's premise holds.
- **KEEP** runs to `end` (276 frames). P2 needs no key: selection finalises on P1's DONE (f 78). With two KEEP bots it finalises on f 74 with no key (P1).
- **PICK** (P2's keys: Down f 75, Right f 78, Up f 81 and f 84, Right Ctrl f 87; then P1's R at f 90 and LCTRL at f 93):
  - selection finalises only at **f 93**;
  - the f 86 PNG (`png/pick_f0086.png`) shows P2's WEAPON 1 changed from BAZOOKA to **BIG NUKE**, with the cursor on **DONE!**;
  - P2's `cs` on `W` frames is `00`, except **`10` on f 87–88** (Right Ctrl held). The arrows are consumed by `PressedOnce` / `Release`, so they never show on a post-frame word;
  - P2's AI is `fresh=1 last=0` on every `W` frame and on f 93;
  - the first draw is on the first `G` tick, f 94 (`last=2af09813`).

  So the AI does not run in selection, and P2's keys drive a PICK bot's menu (facts 5, 6).

### P4 (desk + a real boot): no generated names: CONFIRMED

- `Settings::GenerateName`'s body is `#if 0` (`settings.cpp:165-207`).
- **A real boot:** `data/` was copied to `$S/t0f1/p4root` without `Setups/`, then `SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy timeout 8 build/linux-x64/Release/openliero --config-root $S/t0f1/p4root` (exit 124). The boot saved the defaults with `name = ''` and `randomName = true` for all three players.
- That file, with `recordReplays = true` → `false` (intervention 6), hashes to **`bc29c9d5ce487a40`**. This is byte-for-byte the committed `shell_cfg_default.txt` line `file Setups/liero.cfg bc29c9d5ce487a40`, and the `cfg16` of the shipped `liero.cfg` in the probes' `d` lines. Fact 21 holds.

### P5: HEALTH reaches RESUME (`s_hp`, `s_hp0`; `match_seed 1501`): CONFIRMED

- **The fixture.** A first attempt pressed only P1's DONE, and with a human P2 the match stayed in selection. The committed fixture presses `R`+`UP` then `LCTRL`+`RCTRL` (both DONE, f 75/78).
- **The path:** play f 79–280 → Esc f 281 → F5 f 319 (`d` cur `P`) → Down ×3 → Return f 331 (top `I`) → Backspace ×3, `3`, `0` → Return f 352 → Esc → F1 f 361.
- **While paused,** P1's `settings->health` reads **30** (from f 352) and its `health` stays 100.
- **The first resumed tick is f 395** (`cycles` 234 → 235): P1 `hp=30/30`. `s_hp0` keeps `100/100`.
- `state8` is equal through f 394 and **differs from f 395 on** (200 `G` frames differ, 235 equal).
- So T3 Step 2 (`apply_live_settings` writes `max_health`) and the next tick's clamp are right.

### P6: the colour reaches the palette at RESUME (`s_rgb`, `s_rgb0`; `match_seed 1601`): CONFIRMED

- **The path:** F5 f 319 → Down ×4 (Red) → Left held f 334–374 (`cfg16` changes at f 337, 370, 373) → Esc → F1 f 380.
- **`state8` is equal on all 558 frames,** and the `x` lines are equal except the `reacts` garbage.
- **The presented frames:**
  - equal up to the edit;
  - different on f 337–376 (the Red bar);
  - **equal again on f 377–413**: the main menu after Esc still shows the old palette;
  - **different on every `G` frame from f 414** (the first presented game frame after RESUME) to f 512: 31–32 pixels of P1's worm, e.g. (33, 133) `1a2a66` vs `2a2a66`, the red channel only (`png/rgb_f0420.png` vs `png/rgb0_f0420.png`);
  - equal on f 513–514 (fade 0, black);
  - different again in the menu from f 515.
- So `Game::Focus` re-applies the ramps at RESUME and not before, and D9 stands.

### P7: per-worm health in the sim: CONFIRMED

**`hp_scales`** (Scales, P1 30, P2 200, lives 5, `seed 5`, `generate 9701`, input seed 4545, 1,500 ticks):
- Neither worm ever exceeds its own max (peaks 30 / 200).
- **Wraps into an extra life:**
  - P1 at 30: t202 30 → 11, lives 5 → 6; also t278, t509, t701, t934, t1362, t1438;
  - P2 at 200: t229 200 → 1, lives 5 → 6; also t394, t613, t796, t1054, t1305.
- **Deaths re-add the dying worm's own max:** P1 t219 (→ 30); P2 t278 1 → 190 (−10 + 200).
- **Scales respawns keep the health** (`worm.cpp:795` resets only outside Scales): for example, P1 respawns at 11, 20 and 8.

**`hp_bonus`** (Kill'em All, P1 50, P2 300, MAX BONUSES 10):
- The clamp holds (peaks 50 / 300).
- **Respawns restore each worm's own max** (P1 50 at t180, t592 and t948; P2 300 at t215).
- No pickup happened in 1,500 ticks, nor in 3,000 with a passive worm, so a seed scan was run (`scan.py`, 120 seeds × 3,000 ticks, P1 50 / P2 300).

**The health bonus scales by the picking worm's max** (`(rand(51) + 10) * max / 100`: 5..30 for 50, 30..180 for 300, and 10..60 if it used 100):
- P2 unclamped:
  - seed 46: 221 → 290 (+69);
  - seed 91: 194 → 284 (+90);
  - seed 94: 190 → 262 (+72) and 258 → 300;
  - seed 97: 207 → 285 (+78);
  - seed 61: 174 → 300 (≥ 126).

  Every one is a multiple of 3 and above 60.
- P1:
  - seed 37: 14 → 29 (+15 = 30 × 50 / 100);
  - seed 107: 21 → 29 (+8);
  - seed 10: 28 → 50 (clamped).

**The low-health blood gate is `health < own max / 4`.** On visible ticks without Fire, a pickup, a life change or a bonus change:
- every tick below the worm's own quarter drew (0 non-drawing ticks in 120 runs);
- **P2 at 25 ≤ health < 75:** every such tick drew, in all 8 seeds that reached the band (1,775 of 1,775 ticks; seed 5 alone 786). A gate at 100 / 4 would draw on none of them;
- **P1 at 12 ≤ health < 25:** about 90 % drew nothing (for example seed 0: 568 vs 62). The ~10 % is the same background rate as above the band (730 vs 79), from other draws. A gate at 100 / 4 would draw on all of them.

**Also recorded for Batch 2's tests** (the source, no rule): Scales `DoDamage` has two arms (`game.cpp:566-589`):
- unattributed or self damage splits the amount among the other worms: `DoHealingDirect(*other, k_)`;
- damage by another worm heals **that attacker** by the full amount: `DoHealingDirect(*worms[by_idx], amount)`.

Both arms wrap at the healed worm's own max. With two worms, "heals worm 1 against worm 1's max" holds in both.

### Restore

- `git checkout -- src/tools/oracle_dump/shell_dump.cpp src/tools/oracle_dump/sim_physics_dump.cpp`, then both targets were rebuilt in `build/linux-x64` (Release) and in `$S/build-chk`. `git status --short src` is empty.
- **The restored binaries are byte-identical** (`cmp`) to the goldens, under both builds: `shell_boot_idle.txt`, `shell_milestone.txt`, `sim_slice4_5e_small.txt` and `sim_slice4_5e_tall.txt`. `git status --porcelain rust/oracle-tests/golden` is empty.
- No Xvfb was started. No `/tmp/oracle_shell_fs_*` remains. The PPM dirs and `p4root` were deleted.

### Changes to later tasks (the contradiction rules applied; these supersede the task text above)

**None.** No probe contradicted the plan: P2 took its "D5 confirmed as necessary" branch. D1–D17 and T1–T10 stand as written.

Notes for the batches (clarifications, not rule changes):
1. **Batch 4, T5 Step 5 smoke 1** (the scratch build without intervention 3′): expect **different** `d` lines under the checked build from the CPU's first placement (tick 150), and ASLR-dependent results under release. Record it as P2 did. A release run that happens to match proves nothing.
2. **Batch 4, T4:** the `ai` directive's `reacts` fill is load-bearing for the checked `CHECKED_DUMPER` `cmp` in T6 Step 3. Without it the checked dumper would write different bytes (`0xBEBEBEBE`).
3. **Batch 4, T5 check 3″:** scoped to controllers made by a NEW GAME, as D14 says. The boot controller's never-processed `DumbLieroAI` needs no check.
4. **Batch 7, T8 `cpu_vs_cpu`:** with two KEEP bots, selection finalises on the first frame after the NEW GAME frame with no key (f 74 here), so the match starts at once.
5. **Batch 2, T1 / T2:** the AI's first stale `reacts` read comes at the CPU's first placement (about tick 150, pos set, `visible` false). Rust's `reacts` must be 0 there (from `from_init`) and must not be reset on death or respawn (pitfall 12). The P7 blood-gate and bonus bands above are the numbers to pin in T1 Step 2's tests.

## Addendum T9 (John's ruling on `?cpu=`, 2026-09-27)

The plan left open whether `?cpu=` should also make the CPU's weapons pick themselves on a keyboard. **John's ruling:** `?cpu=` keeps the original's weapon picking.
- `?cpu=` only switches players to the CPU (`WormSettings::controller`). BOT WEAPONS follows the setup: PICK by default, as in C++ (`settings.hpp:24`, both shipped setups). So on a keyboard you pick the CPU's weapons with player 2's keys: the arrows, then Right Ctrl on DONE!.
- The preview comment says so, and so does the page's Player 2 key help.
- The plan's other defaults stand (D4): `?cpu=0` makes both players human, `1` makes player 2 the CPU, `2` makes both CPUs. A touch-only page with no `?cpu=` acts as `1`. `?cpu=` does not skip the menu. On a touch-only page LOAD SETUP sets player 2 back to the CPU (T7).
- T9 follows this. `MatchParams::apply_cpu` leaves `select_bot_weapons` alone; only a touch-only page's NEW GAME sets RANDOM (D3).
