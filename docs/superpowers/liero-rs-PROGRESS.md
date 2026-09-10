# Liero-rs — progress at a glance

> Two tracks:
> **🔁 Rewrite** — a faithful port of existing OpenLiero (C++/SDL3) to Rust (+ Bevy),
> proven **bit-for-bit** against the C++ engine as a truth oracle.
> **✨ New** — capabilities the original never had (enabled once the rewrite lands).
>
> The headline % tracks the **rewrite**; the **new** track is exploratory/future.
> The dense machine ledger lives in `.superpowers/sdd/progress.md` (gitignored).
>
> **Last updated:** 2026-09-10 · **⚙️ 4½a-1 (match config + settings → sim) LANDED — 🎯
> settings-driven matches bit-exact vs C++, incl. `IsGameOver`.** 4½a split in two (design §0):
> 4½a-1 is the sim side, 4½a-2 (TOML writer + byte gate + `UpdateHash` + storage + HUD fix) is
> still planned. 4½a-1 ships `Settings`/`WormSettings`/`MatchConfig` (C++ names + defaults) and a
> C++-schema TOML reader with `TomlInputArchive` semantics (all 10 shipped setups/profiles load;
> `liero.cfg` == `Settings::default()`); `build_match` (`MatchConfig → SimState`, with refusals:
> Holdazone, asymmetric health, health < 1), which reproduces `sim_slice6_fuzz5` (1501 rows) first
> run; `CorrectShadow` at all 7 sites (4½b T9 proved it bit-exact vs the C++ dig stage, 21/21);
> the Scales death/respawn rules, `DoHealing` and the GameOfTag guard; `is_game_over`; `MatchFlow`
> (180-frame post-mortem, then the live game restarts via the F5 path until 4½g); and the live
> `sound_hooks` bug fix. **Gate:** one new optional scenario directive `settings <file>` — the C++
> dumper reads it with the real `Settings::FromToml` and emits a 12th `IsGameOver` column (the
> absent-directive path regenerates every existing golden byte-identically) — plus four
> settings-driven goldens: defaults (seed 7, 401 rows), killemall (game seed 11, flips t934),
> scales (seed 5, health 40, flips t2282) and gametag (seed 43, flips t1610, a bonus picked up at
> t487). 🎯 **MILESTONE: 4/4 variants bit-exact incl. `IsGameOver`, 5830 rows**, driven through
> `Scenario::parse → settings_from_toml → build_match` on the committed files (a one-tick
> mutation is proven to fail). **The matrix found a Step-2 port gap:** Rust's `wobject_process`
> omitted C++ `WObject::Process`' `collide_with_objects` impulse loop (`weapon.cpp:212-232`,
> reached by FAN) — ported in T8b, and no prior golden moved. RIFLE/WINCHESTER/LASER/GAUSS
> GUN/MISSILE still hit unported Step-2 branches: banned from 4½a's goldens, ported in the new
> slice **4½c-0** before 4½c. Developed on branch `liero-rs-4-5a` in parallel with 4½b, then
> cherry-picked onto `liero-rs-step-4-5` (the CorrectShadow commit was already there as
> `42a45a2`); no prior golden changed. Step 4½ now: **4½a-1 ✅ + 4½b ✅**, 4½a-2 + 4½c-0…4½h planned.
>
> Prior (2026-09-10): **🗺️ 4½b (random level generation) golden DONE — bit-exact.**
> The Rust generator (`sim::levelgen`) reproduces C++ `GenerateRandom` stage by stage (noise
> field, splats, stones, worm tunnels, rock formations, rocks — level hash AND `rand.last` after
> each) over 3 seeds × 7 sizes (incl. maps small enough to hit the `kMaxTries` cap) × shadow
> on/off, plus `MakeShadow` and `GenerateFromSettings`' file and missing-file paths — 49/49 lines
> bit-exact. The golden's dig stage is a function-level oracle for 4½a's `CorrectShadow` port:
> T9 replays the 21 shadow=1 dig tokens through `sim::shadow::correct_shadow` and they match
> bit-exact, so 4½b is COMPLETE. SelectSpawn deferred with Holdazone. Spec
> `specs/2026-09-10-liero-rs-step4.5-slice4.5b-level-generation-design.md`.
>
> Prior (2026-09-10): **📐 STEP 4½ (game shell) PLANNED — inserted between Step 4
> and Step 5.** Step 4 left a bare run that plays a hard-coded default match; "playable
> single-player that feels like Liero" also needs the shell around it. Step 4½ ports it:
> main menu over a generated level, weapon selection, level select + random generation,
> settings/profiles (C++ TOML schema, so C++-saved `liero.cfg`/profiles load), DumbLieroAI for
> solo play, match end + a compact stats screen — pixel-exact menus first, a modern UI later.
> Eight slices **4½a–4½h**, none started; every sim-affecting part (settings plumbing, levelgen,
> weapon-select RNG, DumbLieroAI, `IsGameOver`) gets a C++ golden. Overview:
> `specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md`. Rewrite % re-based to include
> 4½ (~80% → ~70%). Branch `liero-rs-step-4-5`.
> Prior (2026-07-13): **🎉 STEP 4 COMPLETE — slices 4f+4g SHIPPED: bare run =
> playable match; the harness covers the whole loop.** 🎯 **`cargo run -p game` now starts a
> genuinely PLAYABLE default match** — no flags — closing the loop the whole step built toward:
> live input (4a) → record/replay (4b) → audio (4c) → live viewport (4d) → `.lrp` reading (4e)
> all converge on a bare invocation that just works. **4f** ships the default-match fixture
> (Live mode, `render_stage`-level, two visible worms on blood-bevised positions, DART, seed 42;
> the fixture sits outside the golden dir so it cannot perturb any golden enumeration), F5-restart
> (adjudicated to also zero an in-flight recording — restart means restart), a clean positional-arg
> rejection for `default_match` (fixed after an initial review pass), and a tidied window title; the
> wasm arm stays byte-unchanged (the parity witness holds). `GenerateFromSettings`/random-level
> generation stays deferred. **4g** ships `shot --scenario-path <file>` (renders any recording or
> arbitrary scenario file, not just the golden corpus — the golden path itself stays byte-identical,
> 19 shot tests green), a new SKILL.md §7 (live play + the full key-binding table, the
> record→replay loop incl. the flush caveat — Esc/window-close flush, a killed process writes
> nothing, F5 zeroes an in-flight recording — headless verification, and a CI-coverage inventory
> showing every gate is already wired into CI); the CI half of 4g was in fact delivered
> incrementally across 4a–4e, so 4g's own diff is thin by design. **All 5 commits** (`80cb75c` plan /
> `7146490` T0 / `41a1af2` T0-fix / `f566404` T1 / `e22c90a` T2) landed; T0 was reviewed
> NEEDS-FIX → fixed by 2 Important-severity implementer fixes → clean, T1/T2 were self-verified
> under the broad step-4 review (T4) that follows. **STEP 4 (4a–4g) IS NOW COMPLETE.** What remains
> for John: the PR #5 merge decision and the `.lrp` phase-2 (cereal `Game` graph) follow-on decision
> — both bookkept, neither blocking. Branch `liero-rs-step-4`.
> Prior: **STEP 4 — slice 4e phase 1 SHIPPED: `.lrp` REPLAY READER.**
> 🎯 **Liero-rs LÄSER RIKTIGA `.lrp`-REPLAYS byte-troget** — en genuin C++-producerad replay
> driver Rust-simmen och reproducerar C++-facit **bit-exakt över 1120 ticks**, inklusive
> `WideRollbackChecksum`-ordet vid cycle 1050 (`0xf6211087`), och `Prior:`-kedjan nedan förblir
> byte-identisk. **Fas-gräns-upptäckten:** `.lrp`-containerns per-worm-blobbar (cereal
> `Game`-grafen) är **längd-prefixade**, så fas 1 kan skippa hela cereal-grafen rakt av —
> initialtillståndet kommer istället från befintliga `scenario::load` — och
> **`WideRollbackChecksum` är en HÅRDARE gate än `HashGameState`**, eftersom den foldar hela
> rollback-inventariet (21 worm-fält + pooler + materialbuffert + `prev_control_states` +
> terräng), inte bara tickens synliga tillstånd. **T0** byggde `lrp_gen`, en C++
> headless-replay-writer (setup verbatim från befintliga dumpern) plus en tvåfixtur-korpus
> (320t + 1120t, checksum-ord vid cycle 0 och 1050) och bevisade tick-0-alignment. **T1** porterade
> själva `WideRollbackChecksum`-porten (Mix32 exakt, alla 21 worm-fält + pooler + materialbuffert,
> `prev_istates` caller-supplied) — cycle-0-facit (`0x031057d7`) matchade **första körningen**.
> **T2** byggde den Bevy-fria `rust/replay`-craten (flate2): BE-container, cereal-blob-**skip** via
> dess längdprefix, taggar `0x80`–`0x83`, XOR-delta-avkodning, checksum-extraktion — verifierad mot
> alla 1120×2 input-ord via scenario-grammatiken (en XOR-baslinje-caveat — muterande fixtures —
> dokumenterad, routad till fas 2). **T3 MILSTOLPE:** fas-1-gaten gick **GRÖN FÖRSTA KÖRNINGEN** —
> 1120 ticks uppspelade bit-exakt inkl. cycle-1050-checksumordet, backad av en per-tick-tripwire mot
> en ny sidecar plus ett negativtest (muterat ord). **T4** härdade läsaren mot fientlig indata
> (10 tester — ingen panik på skräp, `O(1)` `Err` på ett jätte-längdprefix etc.) plus
> `debug_assert`, och bokförde formellt fas 2 (spec §8: cereal-grafen / ett version-<7-lyft
> [**adjudikerat: avvisas för fas 1** — den enda version-<7-skillnaden inuti cereal-blobben är
> palett-only] / `decode_with_baseline` / framehash-diffen). **Scope-notering:** fas 1:s orakel är
> `WideRollbackChecksum`-tidsserien, inte den C++-`framehash`-diff det ursprungliga done-when-läget
> förutsåg — se overview-dokumentets uppdaterade done-when §5 nedan. Alla 6 commits (`fb9b9b7` plan /
> `d0cca91` T0 / `0a08d54` T1 / `b5fb884` T2 / `cd711fc` T3 / `b8bd8cb` T4) granskade **READY på 0
> Critical / 0 Important** genomgående. Deferred: fas 2 i sin helhet (cereal `Game`-grafens
> deserialisering, möjligen egen slice/post-step-punkt per overview-dokumentets Open Q1-scope-gate).
> Branch `liero-rs-step-4`.
> Prior: **STEP 4 — slice 4d SHIPPED: LIVE VIEWPORT.** 🎯 **flash,
> shake, camera and death banners now evolve from real explosions and a real spawn/death** —
> not injected directives — and match a new C++ `render_live` golden **bit-exact over 321
> ticks**. The **T0 headline correction**: C++ `HashGameState` never folds `screen_flash`
> (`stateHash.hpp` omits it; it lives only in the rollback `GameSnapshot`) — so adding it to
> `SimState` (decrement `game.cpp:271-273`, sobject-create max-write `sobject.cpp:41`, drained
> by `flash.rs`) is **hash-neutral**, gated by re-diff, not re-fuzz. **T1** added the mirror
> seam for `shake`: the sole writer (`sobject.cpp:31`) emits **raw** blast-coordinate + amount
> events; the game layer does the `itof`/rect-test/max, keeping the sim itself viewport-blind.
> **T2** is the game-layer live-stepping core (`viewport_step.rs`): decrement-without-floor
> `-4000` on **raw** values, banner-walk on **pre-frame** state, **then** `process_frame`,
> **then** apply the shake max, **then** `as_scene(sim.screen_flash)` — reviewed as genuinely
> C++-trogen against `ProcessViewports`'s real call site (`game.cpp:463`, after the worm loop).
> **T3** built the C++ dumper's opt-in `render_live` directive (full `ProcessFrame` + wired
> viewports) with a **green re-diff** (every prior golden byte-identical) plus the Rust parser
> arm. **T4 MILESTONE:** the `render_slice4d_live` golden matched on the **first run** — 321
> ticks bit-exact across a death→respawn window, flash/shake tracking real explosions, the
> camera moving live as the respawned worm falls (the `SetCenter` arm reached via a fire-triggered
> spawn — the `killed_timer` vacuity trap from the design doc solved), triple-isolation held
> throughout, and four non-vacuity witnesses (flash-decay-to-zero, shake RNG jitter, banner-range
> walk, live centering) rule out an accidental all-static pass. The harness is a documented
> hand-copy of `viewport_step.rs` (Bevy's `!Send`/ECS shape forces the duplication), **double-
> anchored** against drift (byte-comparison assertion + a doc comment cross-referencing the
> source). **T5:** death-banner **text**, via the 3e font — a genuinely cross-viewport effect
> (the dying worm's banner draws in the *other* player's viewport, `viewport.cpp:256-270`;
> shadow colour 0 + text colour 50). **Finding + fix:** `frame::draw` interleaved
> process-then-draw **per viewport**, which is wrong relative to C++'s process-**all**-viewports-
> first (`ProcessViewports`, `game.cpp:463`) — refactored to match, and the refactor is
> **hash-neutral** (each viewport's RNG stream is fully local; the 3a/3b/3e goldens are
> unchanged). The 4d golden was regenerated with **only the frame-hash column** touched, and
> **only** inside the death window (ticks 93–236) — the state-hash column stayed byte-identical.
> The `YoureIt`/GameOfTag banner arm stays deferred (a comment, not a hard tripwire). All 6
> commits (`370be59`/`6f01ea6`/`e72b561`/`e6d483a`/`ad8eaeb`/`bf36497`) reviewed **READY /
> MILESTONE-READY at 0 Critical / 0 Important**. Deferred: `steerable_sum`/steerable centering
> (a real DEFER call — `ProcessSteerables` mutates the *hashed* `wobject.cur_frame`, is unported,
> and no scenario reaches it — the non-steerable `SetCenter` + its `debug_assert` guard stand);
> the `YoureIt`/GameOfTag banner arm; a spec-flagged minor — the 15-line harness hand-copy is a
> theoretical frame-level blind spot, mitigated by the double-anchoring above, not eliminated.
> Branch `liero-rs-step-4`.
> Prior: **STEP 4 — slice 4c SHIPPED: audio.** 🎯 **Liero-rs LÅTER** —
> sound plays natively AND in the browser, driven end-to-end by a sim-emitted per-tick
> event stream that adds **zero new `rand`** (every variant draw already existed pre-4c;
> `hash_game_state` stays byte-identical, proven structurally, not by a runtime flag).
> **T1** laid the stream itself: a new Bevy-free `sim::sound` module records `Play`/`Stop` at
> all 10 `ProcessFrame` callsites via a thread-local per-frame collector (deep call trees made
> threading an out-param prohibitive), drained into `SimState.sound_events` at
> `process_frame`'s tail — hash-inert, with the Step-5 speculative-suppression hook already
> documented (not wired; no resim exists yet). **T2 — the classification finding:** re-reading
> C++ `SoundPlayer::Play(int, void* id = nullptr, int loops = 0)` (`player.hpp:15`) against
> every callsite showed `loops` **defaults to `0`** — only `worm.cpp:1120-1121`
> (`Play(launch_sound, &weapons[cur], -1)`) passes an explicit `loops = -1`. It is the **only
> true loop** `ProcessFrame` reaches. The design's other 5 "worm-keyed loops" were `loops=0`
> one-shots, deduplicated only by the caller's `IsPlaying` guard — felklassade in the original
> spec, now corrected there (`specs/2026-07-13-liero-rs-step4-slice4c-audio-design.md` §3.4,
> "CORRECTED (T2)") and in `sim/src/sound.rs`'s `LoopKey::Worm` doc comment. Per John's
> Väg-A call, the hit/blood trio (`weapon.cpp:311-312`/`nobject.cpp:183-184`/
> `sobject.cpp:108-109`) is emitted as `key=None` one-shots — correct sound, C++'s restart-dedup
> lost, an audio-advisory-only difference. **T3** built the backend seam: an `AudioSink` trait
> mirroring C++ `SoundPlayer`, `rodio` 0.19 (`default-features=false`) behind it, a `Drainer`
> that makes `Play` idempotent per loop key (the game-side analog of C++'s `IsPlaying` gate) with
> a belt-and-braces reaper, and a `SoundTable` in the TC's `tc.types.sounds` order. **T4
> MILESTONE:** audio wired live in windowed play — `RodioSink` lives behind a Bevy `NonSend`
> resource (the `cpal::Stream` is `!Send`), falls back to a silent `NullSink` on init failure
> instead of crashing, drains/reaps inside the existing `!replay_finished` guard, and a
> liveness-superset reap closes the worm-death channel-leak edge; headless stays structurally
> silent (no sink constructed); every suite plus the wasm build stayed green. John's ear-check
> remains advisory (same posture as prior eyeball checks). **T5:** `rodio` ships on **wasm too**
> — the `wasm-bindgen` cargo feature routes `cpal` to its WebAudio backend, so the *same*
> `RodioSink` covers both targets (no `kira` fallback needed after all); `sounds/` (~505 KB) is
> now embedded via the existing `read_asset` seam; autoplay stays silent until a user gesture,
> no panic. One real deviation: `load_sound_table_wasm` panics on a missing embedded file (the
> embed catalogue is exhaustively verified at build time) where the native loader silently
> leaves the slot empty — a debug/wasm inconsistency, not a correctness bug (documented, not yet
> unified). Commits `5f10312`(T1)/`966c362`(T2)/`59b6fef`(T3)/`1ffc68e`(T4)/`2b8fc6c`(T5), every
> review READY / MILESTONE-READY at **0 Critical / 0 Important**. Branch `liero-rs-step-4`.
> Prior: **STEP 4 — slice 4b SHIPPED: record→replay round-trip,
> bit-exact.** 🎯 The **Step 5 / ggrs precondition is delivered**: a `Recorder` taps the same
> sampled `ControlState` array `--live` already feeds into `process_frame` (Dig-chord already
> resolved, never re-derived), flushes on `AppExit` (ordered `.after(bevy::window::ExitSystems)`
> — a review-caught race: without that ordering, closing the window by the X button can race
> `exit_on_all_closed` and drop the recording; the Esc quit path was always safe), and writes the
> **same scenario grammar** `scenario::to_text` already parses — **the recorded artifact IS a
> scenario file**, no new format. `--replay <path>` drives *any* scenario headless through the
> **unchanged** `InputSource::Scripted` (play once, then hold — `demo.tick` can never advance past
> the recorded ticks). The headline proof, `round_trip.rs`: a synthetic key-stream through the
> **real** `--live` sampler → `Recorder` → `to_text` → `parse` → replay reproduces the **live**
> `HashGameState` series **tick-for-tick**, non-vacuously (3 asymmetric fixtures + an
> `assert_ne` against the empty series + hardcoded raw-text assertions pinning the Dig chord's
> `"input 6 12 0"` encoding); a mutated word diverges at the exact expected tick (RED-proven). A
> committed corpus (`record_slice4b_blood_scenario.txt`, 12 ticks off the T3 stream) backstops
> drift against a sim-produced 2-column `state_hash` sidecar. **Hardening finding (H1,
> escalated out of the T3 review):** the C++ `cossin[128]` UB from 3b was **reachable in `--live`**
> after all — spawn → walk right → fire — because the masking clamp is gated on
> `aiming_speed != 0`, which live aiming can hit (falsifying John's Decision #3 "unreachable"
> premise from before 4a); three call sites (`worm_fire`, ninjarope-throw, dig) now mask the
> table index `&0x7f` (the C++ side already builds the table with the same mask, `math.cpp:91` —
> matching 3b's precedent for the same table), full sim golden suite stayed **green (313 sim
> ticks)** — a neutral, state-unchanging fix. All 7 commits (`8bf17cc`/`d6feea7`/`1869a43`/
> `04c3d2c`/`c7d53d1`/`4c73119`/`7936ce3`) landed with **0 Critical / 0 Important** across all
> reviews (per-task reviews for T0–T3/H1, T4 covered by the broad slice review; one early
> NEEDS-FIX on T1 was fixed and re-reviewed clean; H1 was re-committed clean after an
> initial messy diff). Deferred: flush-on-crash (by design — Esc/close only), windowed
> `--replay`-hold not automation-tested, `.lrp` interop stays 4e. Branch `liero-rs-step-4`.
> Prior: **STEP 4 — slice 4a SHIPPED: live input core.** 🎯 The game is
> **playable from the keyboard, native** (`cargo run -p game -- --live`) — 1-player and 2-player
> hotseat, one input snapshot sampled per `FixedUpdate` tick (never `Update`/edges), Dig = a pure
> Left+Right chord (never a stored bit), default bindings mirror the decoded C++ keymap
> (`settings.cpp:36-37` via `keys.cpp:9-58`, P0 = R/F/D/G+LCtrl/LShift/LAlt, P1 = arrows+RCtrl/
> RAlt/RShift, Dig unbound both — index-for-index verified against C++). The headless
> **pass-through determinism gate** (`game/tests/passthrough.rs`) proves the new sampler
> reproduces every one of the **7** `render_slice3b_*` scenario goldens **bit-exact**, tick-for-tick,
> in CI (`cargo test -p game`); the scripted path, its debug self-check, and the `Prior:`-chain
> below are all preserved byte-identical. A real finding: **no focus-loss backstop was needed** —
> Bevy 0.19 already releases held keys on window-focus-loss (`bevy_winit` → synthetic `Released` →
> `bevy_input::release_all`). John's manual 30-second play-test remains an open advisory (same
> posture as 3c's Srgb eyeball check). Deferred: live-wasm (browser keyboard input; wasm stays on
> the scripted witness), gamepad, and recording (→ 4b). Commits `072817b`/`642adeb`/`e189de2`/
> `d9a54a3`, each reviewed READY (0 Critical / 0 Important). Branch `liero-rs-step-4`.
> Prior: **🎉 STEP 3 COMPLETE — slice 3f SHIPPED: wasm bring-up.
> Liero-rs now renders in the BROWSER (WebGL2), closing the rendering step (3a–3f).**
> 🎯 **MILESTONE (3f):** the browser milestone is **automated-proven** — a headless-Chrome
> controller ran the debug wasm bundle (SwiftShader-WebGL2, 30 s virtual time) and the
> screenshot shows the **blood** demo's split-screen world (sky / terrain / worms / blood),
> the console **PANIC-FREE**, and the determinism guard (per-tick `state_hash` **+** a wasm-only
> `frame_hash`) stayed **GREEN in the browser** — the wasm-parity witness that the same CPU
> frame renders identically off-native. How it was built: a **`scenario::assets::read_asset`
> seam** (native = verbatim `std::fs::read`, all goldens green = no-op proof; wasm = embed) with
> `include_dir` (wasm-only dep) embedding sprites/weapons/nobjects/sobjects + `include_bytes!`
> tc.cfg + the demo level `render_stage.lev` — a **curated 276 KB total** (sounds/ and the big levels
> excluded); a **target-scoped feature split** (`x11`/`wayland` native-only, `webgl2` wasm-only,
> union verified per target with `cargo tree`) so `cargo build -p game --target
> wasm32-unknown-unknown` is **GREEN**; an entry-fork (`const DEFAULT="blood"`, `include_str!`
> scenario+sidecar, **no** `env::args`/`read_dir`/`fs` on wasm; canvas auto-append verified
> against the `bevy_window` source); a `.cargo/config.toml` (target-scoped `wasm-server-runner`)
> + `web/index.html` dev-loop (127.0.0.1:1334, 200 html+wasm) and a static `wasm-bindgen`
> **0.2.126** (lock-matched) bundle (game.js 97 KB + game_bg.wasm 52.9 MB **release**); and a new
> **`game-wasm` CI job** (build-only, no browser, own job independent of the determinism gate).
> **Fyndet** that saved the dev-loop: cargo reads `.cargo/config.toml` from **CWD**, not
> `--manifest-path` — so the wasm dev-loop runs from `rust/`. Prior (3e): 🖥️ **slice 3e SHIPPED:
> HUD / font / bars / minimap — the full player view is PIXEL-EXACT vs C++.** 🎯 **MILESTONE (3e):**
> the in-game overlay
> is ported verbatim and difference-tested green on the **first run**: a new **`render::font::Font`**
> (font.tga loader, `common.cpp:414-433` width-detect + 0/50→0/8 palette remap) draws the HUD labels
> (`font.cpp:8-80` verbatim — the double `c>=2 && c<252` guard preserved, CLIP_IMAGE inlined,
> newline on codepoint 0; ASCII-decode is identity for `cp<0x80`, which a bevis-test pins as the exact
> reach of the shipped label corpus); **`blit::draw_bar`** (`blit.cpp:105-113`, unclipped + a `width>0`
> anti-clamp witness); **`render::hud::draw_hud`** (`viewport.cpp:84-153` verbatim — the two-arm life
> bar (`health*100/settings_health`; `100-(killed_timer*25)/37` clamped), the two-arm ammo/loading bar,
> the blinking **Reloading** label (`(cycles%20)>10 && visible`, y=`164*multiplier` **absolute** — a
> plan-deviation caught against C++), kills-always / lives-on-KillEmAll+Scales, the `w/10+234` /
> `w/10+245` / 50 / 10 / 6 colour columns); and **`draw_minimap`+`draw_miniature`** (`viewport.cpp:593-613`
> + `level.cpp:489-507` — the two *different* `step`/`bounds` ceil-vs-round idioms preserved,
> worm-dots at `ftoi(pos)/step` in colour `129+worm.index*4`, clip-gated, `AppearanceAt` inlined).
> `Scene`/`frame::draw` widened to composite per viewport — **HUD (full clip) → world (world clip) →
> minimap** with the verbatim double-draw — and every prior render golden stays **byte-identical**
> (anti-bleed proof). The C++ dumper gained an opt-in **`render_hud`** directive mirroring `frame::draw`
> exactly; the RE-DIFF gate is **empty** (3a/blood/shake/sim_slice2 all regenerate byte-for-byte).
> Three new scenarios + gen-scripts + goldens landed — **hud** (41 ticks), **reload** (71t), **death**
> (141t) — and the **MILESTONE difftests are GREEN**: hud + death matched **first run** (per-tick +
> total + triple-isolation + suppression controls + non-vacuity); reload was first **blocked** (the T7
> RIFLE = `ST_LASER` tripped the deferred laser do-loop), then re-cut to a **GRENADE** scenario
> (`ST_NORMAL`, inert hit-arm, no in-window explosion) and turned **GREEN** (71 rows + total; a
> settled 50/51-tick witness). Finding: tick 0 is a fade-to-black, so the HUD witness reads tick 1.
> Milestone review: **MILESTONE READY, 0 Critical / 0 Important.** Prior (3d): 📸 **slice 3d — the headless `shot` CLI + golden-test + in-repo run-skill,
> the agent screenshot/compare loop:** a Bevy-free **`shot` crate** (lib+bin, deps
> `scenario`/`render`/`sim`/`assets`/`sim-core` + `image` png-only — `cargo tree` proves **no
> bevy/jpeg/gif/rayon**) PNG-encodes any 3b scenario at a **fixed tick** (raw RGB, nearest ×scale, no
> fade so tick 0 is not black) + dumps **machine-readable** per-tick `frame_hash`+`state_hash` behind
> `--hashes`; a golden-faithfulness test (`rust/shot/tests/golden.rs`) locks the CLI render bit-for-bit
> vs the C++ sidecars for **blood** + **shake** (**GREEN first run**); the **first in-repo run-skill**
> (`.claude/skills/liero-shot`) drives change→screenshot→judge→compare (960×600 PNG, hashes match
> exactly); per-tick driver a deliberate CLI-local copy of the T8 harness (option B); **11 shot tests
> green** in the unchanged CI command. Prior (3c): 🖼️ **slice 3c — `cargo run -p game` shows Liero LIVE, the project's FIRST Bevy code:** a native window opens the
> blood scenario running in real time — the sim ticks on `FixedUpdate` at the **exact C++ cadence**
> (`1000/14 ≈ 71.43 Hz`, `kDelay=14ms`, `gfx.cpp:1176` — **not** the 60 Hz first assumed), the CPU
> frame from the Bevy-free `render` crate is copied into one `Image` and presented as a `Sprite` at
> **×3 nearest** in a 960×600 window, and the scenario loops **bit-identically** off its own recorded
> inputs while a debug determinism guard (per-tick `state_hash` vs the golden) stays **GREEN** over
> ~26 loops / 15 s. Delivered: a new **Bevy-free `scenario` crate** (the parser lifted verbatim out of
> `oracle-tests` + the loader factored out of the T8 harness — **3d reuses it**); the **`game` binary**
> (Bevy 0.19, `default-features=false` + `bevy_sprite`/`winit`/`window`/`x11`/`wayland` +
> `bevy_render`/`core_pipeline`/`sprite_render` — the sprite feature alone ships **no** GPU backend);
> pure helpers (ARGB→RGBA blit + `next_tick`, unit-tested with discrimination proofs); a CLI scenario
> picker (default `blood`, 7 selectable); CI keeps the **determinism gate Bevy-free** (`--exclude game`,
> proven via `cargo tree`) with a separate `cargo build -p game` step. Dev tool: `render_snapshot.rs`, a
> headless BMP dumper — the first images of Rust-Liero. Two review-caught finds: (1) Bevy 0.19's
> `bevy_sprite` feature has **no GPU backend** (`sprite_render`→`core_pipeline`→`render`→`wgpu`/`naga`
> are required — the window had opened with no renderer and the report carried false lock-claims); (2)
> a brief bug — empty inputs had **diverged** from the golden, so the scenario's **recorded** inputs are
> fed (the debug guard caught it live). Prior (3b): 🎯 **MILESTONE (3b):** the full two-pass world block
> — shadow pass + sprite pass (all 6 object families in C++ order, `viewport.cpp:274-590`),
> worm sprites, ninjarope, fire cone, laser sight, aim crosshair, blood — matches C++
> **tick-for-tick** across **7 golden scenarios** (laser/shadow/shake/fan/dart/blood/
> dart_water), **225 frame rows, ALL matched on the first run**. The draw-time RNG traps go
> live and are proven **non-vacuous**: laser draws **6 distinct per-tick hashes** with both
> viewport RNGs (laser-sparks + shake) active; shadow ON≠OFF (`SeeShadow` pixels land);
> shake steps the frame + a `LightUp` flash blip; draining a pool changes the frame (incl.
> the positive `BlitImageR`-over-water witness). **Triple isolation proof** holds per tick
> (`state_hash` Rust == sidecar == the pre-existing sim golden — rendering never perturbs
> the sim), and the **re-diff gate stays GREEN** (every prior `sim_slice*` byte-identical +
> `render_slice3a.txt` byte-identical; the new `render_slice3b_*` files are the only
> additions). Two real findings en route: an inverted laser `rand(2)` order (caught in the
> T3 review — the plan had misread C++; `rand(2)` is drawn **only inside** the clip) and a
> C++ `cossin[128]` UB on facing-flip (caught by the T0b insert; Rust masks `&0x7f`).
> Ported: the blit primitives (`BlitImage`/`Trans`/`R`, `BlitShadowImage`, `FireCone`,
> `DO_LINE` Bresenham, ninjarope/laser-sight/shadow-line/line), `ShadowQuery` (+4/clamp/
> `SeeShadow`), the fire-cone table/bank, sprite selectors, `wobj_remap`, a widened
> `frame::draw` (Scene + LightUp/shake live), plus the render-only non-hashed `hotspot_x/y`
> + the `ProcessSight` port (closed the laser-origin gap). Step 3 lands on **PR #4** (branch
> `liero-rs-step-3`, accumulating); remaining slices **3c–3f**. Next: **3c** (Bevy window —
> native). Prior (3a): the **Bevy-free `render` crate** stood up the CPU renderer (`Bitmap`
> ARGB8888/pitch/clip, the per-frame palette build reset→RotateFrom→LightUp→pal32, `DrawLevel`
> Classic, the two-viewport 320×200 layout, the FNV-1a **frame hash** + `FadeChannel`) and
> shipped the first pixel-exact **terrain** frame (`render_slice3a_golden`, all 27 hashes
> bit-exact first run; RotateFrom proven observable, flipping on `cycles>>3` @8/16/24).
> Prior: **🎉 STEP 2 COMPLETE** — the full deterministic sim core is ported and bit-exact vs
> the C++ oracle (all of `ProcessFrame`: worms, weapons, all object families, ninjarope,
> bonuses+pickup, death/respawn, GameOfTag+Scales; proven by 24 goldens + 5 fuzz variants ×
> 1500 ticks = 7505 frames of master+9-component hash parity, incl. 2 real sim bugs the final
> fuzz caught and fixed). **PR #3 merged into master.**
> Historik: Step 2, Slice 5 (remaining object
> families) — **5a splinters + 5b damage+blood SHIPPED** (PR #3) and **5c bonuses
> MILESTONE difftest GREEN** (`sim_slice5c_golden` matches the C++ master + all 9
> components, all 501 ticks; the **`bonuses` pool goes live** — under seed 42 the per-tick
> bonus-drop roll fires at tick 252, `CreateBonus` drops a health bonus that **falls and
> bounces** under `Bonus::Process`, settling with its timer still counting down at the
> window's end). The worms stay clear of the bonus (no pickup), and the spawn-flash
> `teleport_flash` has `detectRange=0` so the deferred **chain-loop is inert** — the
> all-ticks match proves nothing chained. Slices 1–5b goldens stay **byte-identical** (the
> roll short-circuits when `max_bonuses==0`). **Deferred:** bonus **pickup** (health/weapon/
> booby worm-loop RNG) + the chain-loop port/tripwire (borrow-threading the bonus pool into
> `sobject_create`) → slice 6 / follow-up. **5d death+respawn MILESTONE difftest is now
GREEN** (`sim_slice5d_golden`, master + all 9 components bit-exact over the full 361-tick
death→respawn window; worm1 dies from the blast, counts down the invisible 150-tick
`killed_timer`, then `BeginRespawn`'s level-reading RNG spawn-search teleports it and
`DoRespawning` rebirths it at full health; slices 1–5c stay byte-identical). The
**fixed-level multi-seed respawn fuzz** then landed (4 variants, distinct bounded trial
counts `{2,3,6,7}`, each master+9 components bit-exact — proving the desync trap's
level-dependent trial-count variance vs the C++ oracle). **All of Slice 5d is
shipped to PR #3.** Then **Slice 5′a landed the open-gate worm-hit path**: the real
**per-pixel `CheckForSpecWormHit`** (worm sprite bank + `MAT_WORM`, replacing 5a's box
over-approximation) plus **both in-flight worm-hit arms** (wobject: blood-before-sound;
nobject: sound-before-blood — the opposite RNG order, each with its own golden). Two
milestones bit-exact vs C++ on the first run: a **dart hits a worm's solid silhouette**
(`sim_slice5prime_golden`, 71 ticks) and a **cannon splinter** does too
(`sim_slice5prime_nobj_golden`, 156 ticks) — each flanked by **near-miss ticks** where the
projectile sits in the worm's 16×16 box on transparent pixels and fires **nothing** (the
anti-false-positive witness: a box would have fired — this pins the deferred `fd33bbc`
blocker as FIXED). Pure-Rust slices, no C++ dumper change; slices 1–5d byte-identical.
Then **Slice 5′b closed 5c's pickup deferral**: the bonus-pickup block (`worm.cpp:287-322`)
is live — two walk-on golden milestones bit-exact vs C++ on the first run (**health heal**
pickup @tick 87/110t on a wounded worm, **weapon reload** @tick 67/90t with the booby
discriminator health-flat guard; booby branch unit-test-pinned). Pure-Rust slice; slices
1–5d + 5′a byte-identical. **All of 5′ is shipped to PR #3.** Then the **moving-worms
fuzz** (T10) landed: 4 DART-duel variants where both worms walk+fire, per-pixel hits on
MOVING worms at 7 distinct `current_frame`×`direction` combos, all bit-exact — the
slice-6 precondition, proving 5′a's sprite selection across frames/directions (no sim
change needed).
Next: **Slice 6** (full ProcessFrame + game modes + chain-loop + >1000-tick fuzz) —
the LAST slice of step 2.

---

## 🔁 Rewrite track — faithful port (~70%)

Strangler-style: the C++ engine is the oracle, every piece differential-tested
bit-for-bit before moving on. Steps 0–2 merged (the deterministic sim core — the
hardest part — is bit-exact vs C++); **step 3 (rendering) is COMPLETE** (3a–3f): slices
3a + 3b shipped the pixel-exact terrain frame **and** the full world view (shadow +
sprite pass), **3c** put it LIVE on screen (`cargo run -p game` — the first Bevy
code), **3d** the headless screenshot CLI + run-skill, **3e** the HUD/font/bars/
minimap overlay (the full player view pixel-exact), and **3f** the wasm bring-up —
the same CPU frame now renders in the **browser** (WebGL2), automated-proven in
headless Chrome. **Step 4 (input/replay/audio) is COMPLETE** (4a–4g): 4a (live input),
4b (record→replay round-trip, the Step 5 precondition), 4c (audio, native + wasm),
4d (live viewport — flash/shake/camera/banners), 4e phase 1 (`.lrp` replay reader —
container + delta stream + `WideRollbackChecksum`, bit-exact over 1120 ticks), 4f
(minimal start flow — bare `cargo run -p game` is a playable default match), and 4g
(run/verify-skill extension + CI replay-checksum regression) are all shipped; 4e phase 2
(cereal `Game` graph) is a bounded follow-on. **Step 4½ (game shell) is in progress** — inserted
2026-09-10 because a playable default match is not yet a game: the menus, weapon selection, level
select/generation, settings/profiles, DumbLieroAI and the stats screen are the remaining
single-player surface. **4½a-1** (settings → sim, bit-exact incl. `IsGameOver`) and **4½b** (random
level generation, bit-exact) are done; 4½a-2 and 4½c-0…4½h are planned. Step 5 (netplay) is not
started.

The % was re-based on 2026-09-10: the denominator is now **steps 0–5 plus Step 4½** (~10 k C++
LOC of shell, ~1.5 k of it sim-affecting), so the same finished work (steps 0–4) reads ~70%
instead of ~80%.

```
REWRITE (steg 0–5, incl. 4½)                                ~70%
├─ ✅ Step 0  sim-core primitives (RNG/fixed/vec/math/tables)   DONE — merged (PR #1)
├─ ✅ Step 1  asset IO, slices 1a–1e (level/palette/sprites/    DONE — merged (PR #2)
│             tc.cfg/objects/WAV)
├─ ✅ Step 2  deterministic sim core                            COMPLETE — merged (PR #3)
├─ ✅ Step 3  Bevy rendering / window (reproduce the SDL3 view) DONE — 3a–3f shipped (PR #4)
├─ ✅ Step 4  input + replay (.lrp) + audio                     COMPLETE — 4a–4g shipped (4e-phase2 bounded follow-on)
├─ 🟡 Step 4½ game shell (menus, weapsel, level select/gen,     4½a-1 ✅ 4½b ✅, rest planned  ◀── YOU ARE HERE
│             settings/profiles, DumbLieroAI, match end + stats)
└─ ⬜ Step 5  native netplay (ENet + rollback + Go relay)       not started
```

> Everything above EXISTS in the original openliero — this track reproduces it in
> Rust (new engine/idioms = modernization, not new game functionality).

### Step 2 — deterministic sim core (✅ COMPLETE)

Six slices, each differential-tested against a per-tick `HashGameState` /
`HashGameComponents` oracle.

```
├─ ✅ Slice 1  Level → sim-state + state-hash harness (tick 0)
├─ ✅ Slice 2  one worm, physics only
├─ ✅ Slice 3  worm control + aiming (master hash turns on)
├─ ✅ Slice 4  one weapon, full lifecycle (4a–4d)              SHIPPED
│   ├─ ✅ 4a  projectile lifecycle — fan (RNG goes live)         SHIPPED
│   ├─ ✅ 4b  terrain destruction — greenball / DrawDirtEffect   SHIPPED (level hash live, 91 ticks bit-exact)
│   ├─ ✅ 4c  explosion sobjects + nobjects — dart               SHIPPED (sobjects/nobjects live + carving, 91 ticks bit-exact)
│   └─ ✅ 4d  slice-3/4 deferrals (dig, reload, shell-drop+land, load_change)  SHIPPED (handgun, master+9 components bit-exact 126 ticks vs C++)
├─ ✅ Slice 5  remaining object families — decomposed 5a–5d
│   ├─ ✅ 5a  splinters (cannon → medium_explosion + 5 splinters)  SHIPPED (PR #3, 131 ticks bit-exact)
│   ├─ ✅ 5b  worm damage + blood (O10)  SHIPPED (PR #3, explosives wound → blood → live bobjects, 121 ticks; cycles live)
│   ├─ ✅ 5c  bonuses (CreateBonus + bonus-drop roll + Bonus::Process)  MILESTONE GREEN (bonus drops/falls/bounces, 501 ticks; pickup + chain-loop deferred)
│   └─ ✅ 5d  death + respawn (BeginRespawn RNG-search; fuzzed)  MILESTONE GREEN (death→respawn 361 ticks bit-exact + 4-variant respawn fuzz {2,3,6,7} trials)
├─ ✅ Slice 5′ (open-gate worm-hit follow-up)  decomposed 5′a + 5′b
│   ├─ ✅ 5′a  per-pixel CheckForSpecWormHit + wobject/nobject in-flight worm-hit arms  MILESTONE GREEN (dart 71t + cannon-splinter 156t bit-exact, near-miss anti-box witness)
│   ├─ ✅ 5′b  bonus pickup (health/weapon/booby) — closes 5c's deferral  MILESTONE GREEN (walk-on health heal 110t + weapon reload 90t bit-exact vs C++; booby unit-test-only)
│   └─ ✅ T10 moving-worms fuzz (slice-6 precondition)  GREEN (4 dart-duel variants, hits on MOVING worms at 7 frame×direction combos, 4×76t bit-exact; no sim change)
└─ ✅ Slice 6  full ProcessFrame + game modes + >1000-tick fuzz match  COMPLETE — STEP 2 DONE
    ├─ ✅ T0  dumper → full ProcessFrame tail + re-diff gate (23/24 priors byte-identical; sim_slice3 regen @tick 94, prefix proven)
    ├─ ✅ T1  ninjarope datamodel + throw un-skip (unhashed fields, NR* consts threaded)
    ├─ ✅ T2  Ninjarope::Process ported + wired (sim_slice3 un-ignored, 146 ticks bit-exact; 11×rand(128) dirt-attach burst)
    ├─ ✅ T3  chain-loop (sobject → bonus re-trigger; closes the 5c borrow-conflict deferral)
    ├─ ✅ T4  new_object_reuse for sobjects/wobjects (+ blood pool via T7b)
    ├─ ✅ T5  GameOfTag + Scales ported with goldens (gametag 431t, scales 121t bit-exact; Holdazone deferred past step 2)
    ├─ ✅ T6  cossin disposition doc + pickup truncation test
    ├─ ✅ T7+T7b  5 fuzz scenarios ×1500 ticks + the 2 sim bugs the fuzz caught FIXED (bazooka obj_trail; blood-pool NewObjectReuse)
    ├─ ✅ T8  fuzz difftest MILESTONE (5×1501 ticks, 7505 frames, master+9 components bit-exact; deferral #7 empirically closed)
    └─ ✅ T9  step-2-wide broad review: READY — step 2 COMPLETE (merge = John's call)
```

| Level | Done |
|---|---|
| Rewrite track (steps 0–5, incl. 4½) | **~70%** (steps 0–4 done; step 4 (input/replay/audio, 4a–4g) COMPLETE — 4e-phase2 bounded follow-on + step 4½ game shell (in progress: 4½a-1 + 4½b done 2026-09-10) + step 5 netplay remain; re-based from ~80% when 4½ was added to the denominator) |
| Step 3 (rendering) | **✅ COMPLETE** (slices 3a + 3b + 3c + 3d + 3e + 3f all shipped; PR #4 ready to merge) |
| Slice 3f (wasm bring-up) | **✅ MILESTONE GREEN** (🎉 the **browser milestone**: the same CPU frame renders in the browser via **WebGL2**, **automated-proven** — a headless-Chrome controller ran the debug wasm bundle (SwiftShader-WebGL2, 30 s virtual time) and the screenshot shows the **blood** demo's split-screen world (sky/terrain/worms/blood), the console **PANIC-FREE**, and the determinism guard (per-tick `state_hash` **+** a wasm-only `frame_hash`) stayed **GREEN in the browser** = the wasm-parity witness. Built: a **`scenario::assets::read_asset` seam** (native = verbatim `std::fs::read` — all goldens green = no-op proof; wasm = embed) + `include_dir` (wasm-only dep) embedding sprites/weapons/nobjects/sobjects + `include_bytes!` tc.cfg + the demo level `render_stage.lev` — a **curated 276 KB total** (sounds/ and big levels excluded); a **target-scoped feature split** (base 6 draw-features; `x11`/`wayland` native-only; `webgl2` wasm-only — union verified per target via `cargo tree`) making `cargo build -p game --target wasm32-unknown-unknown` **GREEN first try**; an entry-fork (`const DEFAULT="blood"`, `include_str!` scenario+sidecar, **no** `env::args`/`read_dir`/`fs` on wasm; `canvas=None` auto-append verified against the `bevy_window` source; determinism guard hardened with a per-tick wasm-only `frame_hash`); a `.cargo/config.toml` (target-scoped `wasm-server-runner`) + `web/index.html` dev-loop (127.0.0.1:1334, 200 html+wasm) + a static `wasm-bindgen` **0.2.126** (lock-matched) bundle (game.js 97 KB + game_bg.wasm 52.9 MB release); a new **`game-wasm` CI job** (build-only, no browser/apt, own job independent of the determinism gate, mirrors `sim-core`'s no-cache posture). Fynd: cargo reads `.cargo/config.toml` from **CWD**, not `--manifest-path` — the wasm dev-loop runs from `rust/`) |
| Slice 3e (HUD / font / bars / minimap) | **✅ MILESTONE GREEN** (🎯 the **full player view is PIXEL-EXACT** vs C++ — the in-game overlay ported verbatim + difftest green first run: new **`render::font::Font`** (font.tga loader `common.cpp:414-433`, width-detect + 0/50→0/8 remap) draws HUD labels (`font.cpp:8-80` verbatim — double `c>=2 && c<252` guard, CLIP_IMAGE inlined, newline on cp 0; ASCII-decode **identity for `cp<0x80`**, bevis-tested as the exact reach of the label corpus); **`blit::draw_bar`** (`blit.cpp:105-113`, unclipped + `width>0` anti-clamp witness); **`render::hud::draw_hud`** (`viewport.cpp:84-153` verbatim — two-arm life bar (`health*100/settings_health`, `100-(killed_timer*25)/37` clamped), two-arm ammo/loading bar, blinking **Reloading** (`(cycles%20)>10 && visible`, y=`164*multiplier` **absolute** — a plan-deviation caught vs C++), kills-always / lives-on-KillEmAll+Scales, `w/10+234` / `w/10+245` / 50 / 10 / 6 colour columns); **`draw_minimap`+`draw_miniature`** (`viewport.cpp:593-613` + `level.cpp:489-507` — the two *different* `step` ceil vs `bounds` round idioms preserved, worm-dots `ftoi(pos)/step` colour `129+worm.index*4`, clip-gated, `AppearanceAt` inlined). `Scene`/`frame::draw` composites per viewport (**HUD full-clip → world world-clip → minimap**, verbatim double-draw); C++ dumper gained opt-in **`render_hud`** mirroring `frame::draw`, **RE-DIFF gate empty** (3a/blood/shake/sim_slice2 byte-identical). 3 scenarios + goldens — **hud** 41t / **reload** 71t / **death** 141t — difftests **GREEN**: hud + death first run (per-tick + total + triple-isolation + suppression + non-vacuity); reload first **blocked** (T7 RIFLE = `ST_LASER` tripped the deferred laser do-loop), re-cut to **GRENADE** (`ST_NORMAL`, inert hit-arm, no in-window explosion) → GREEN (71 rows + total, settled 50/51 witness). Find: tick 0 is fade-to-black ⇒ the HUD witness reads tick 1. Holdazone/GameOfTag/replay HUD-arms tripwired. Milestone review: **0 Critical / 0 Important**; minors on the deferral track: DRY the inlined `clip_image`, a `size>1`-advance font test, minimap dot-index discrimination, reload's own suppression control (deliberately omitted)) |
| Slice 3d (headless `shot` CLI + run-skill) | **✅ MILESTONE GREEN** (🎯 the agent screenshot/compare loop lands: new **Bevy-free `shot` crate** (lib+bin; deps `scenario`/`render`/`sim`/`assets`/`sim-core` + `image` png-only, **no bevy/jpeg/gif/rayon** per `cargo tree`) PNG-encodes any 3b scenario at a **fixed tick** to a known path — raw RGB, nearest ×scale, pitch-correct, **no fade** (tick 0 is not black) — and dumps **machine-readable** per-tick `frame_hash`+`state_hash` to stdout behind `--hashes` (info to stderr); paths via `CARGO_MANIFEST_DIR`, one tick ⇒ file / many ⇒ dir, `--scale 0` rejected. **Golden-faithfulness test** `rust/shot/tests/golden.rs` locks the CLI render bit-for-bit vs the committed C++ sidecars for **blood** (base path) + **shake** (the CLI copy's ONE new path — per-tick `render_flash`/`render_shake` injection): every per-tick frame+state hash + the folded FNV `total` + row count, **GREEN first run**. Project's **first in-repo run-skill** `.claude/skills/liero-shot` drives change→screenshot→judge→compare (all commands verified run; 960×600 PNG, hashes match golden exactly). Per-tick driver is a deliberate **CLI-local copy** of the T8 harness (**option B** — T8 untouched, the golden-test guards the drift; factoring into `scenario` deferred → third consumer). **CI unchanged** — `--workspace --exclude game` already sweeps `shot` (a member), png-encode is pure Rust (miniz_oxide/flate2, no apt pkg), goldens committed; **11 shot tests (9 unit + 2 golden) green in the exact CI command**. Reviews: T0/T1/T2 two-stage, **0 Critical / 0 Important** blocking (T2's Important fixed in `5dfa9c0`). Minors on the deferral track: parse-quirks (flag-as-value consumed silently, last-wins dup), pitch≠w test's `h=1` non-vacuity) |
| Slice 3c (Bevy window — native) | **✅ MILESTONE GREEN** (🎯 `cargo run -p game` shows **Liero LIVE** — the project's **first Bevy code**: a native 960×600 window presents the blood scenario in real time, sim on `FixedUpdate` at the **exact C++ cadence** `1000/14 ≈ 71.43 Hz` (`kDelay=14ms`, `gfx.cpp:1176` — **not** the 60 Hz first assumed), CPU frame (Bevy-free `render`) → one `Image` → `Sprite` at **×3 nearest**, scenario **loops bit-identically** off its recorded inputs with a debug determinism guard (per-tick `state_hash` vs golden) **GREEN** over ~26 loops/15 s. New **Bevy-free `scenario` crate** (parser lifted verbatim from oracle-tests + loader factored out of the T8 harness — **3d reuses it**); `game` binary (Bevy 0.19, `default-features=false` + `bevy_sprite`/`winit`/`window`/`x11`/`wayland` + `bevy_render`/`core_pipeline`/`sprite_render`); pure ARGB→RGBA blit + `next_tick` helpers (unit-tested w/ discrimination proofs); CLI scenario picker (default `blood`, 7 selectable); CI **determinism gate stays Bevy-free** (`--exclude game`, proven via `cargo tree`) + separate `cargo build -p game`; dev `render_snapshot.rs` headless BMP dumper. 2 review finds: Bevy `bevy_sprite` alone ships **no GPU backend** (needs `sprite_render`→`core_pipeline`→`render`→`wgpu`/`naga`; window had opened renderer-less) + a brief bug where empty inputs **diverged** (recorded inputs fed instead; guard caught it live)) |
| Slice 3b (shadow + sprite pass) | **✅ MILESTONE GREEN** (🎯 the **world view is PIXEL-EXACT** vs C++: **7 goldens** `render_slice3b_{laser,shadow,shake,fan,dart,blood,dart_water}` — **225 frame rows, ALL matched first run**; two-pass shadow+sprite block, all 6 object families in C++ order (`viewport.cpp:274-590`), worm sprites/ninjarope/fire cone/laser sight/crosshair/blood; blit primitives + `ShadowQuery` (+4/clamp/`SeeShadow`) + line drawers + fire-cone table + render-only `hotspot_x/y` + `ProcessSight` ported, `LightUp`/shake live; **both viewport RNGs live & non-vacuous** — laser 6 distinct per-tick hashes, shadow ON≠OFF, shake+flash blip, pool-drain changes the frame (positive `BlitImageR`-over-water witness); **triple isolation per tick** (`state_hash` Rust == sidecar == sim golden); **re-diff GREEN** (all `sim_slice*` + `render_slice3a` byte-identical, new `render_slice3b_*` the only additions). 2 real finds: inverted laser `rand(2)` order (T3 review; drawn only inside clip) + C++ `cossin[128]` UB on facing-flip (T0b; Rust masks `&0x7f`)) |
| Slice 3a (render foundation) | **✅ SHIPPED** (🎯 first pixel-exact **terrain** frame vs C++: `render_slice3a_golden` — all 27 frame hashes + total accumulator bit-exact, **first run**; Bevy-free `render` crate: `Bitmap`/palette build (RotateFrom/LightUp/pal32)/`DrawLevel` Classic/two-viewport `Viewport::process`/FNV-1a hash+`FadeChannel`; C++ dumper's opt-in `render player` directive → sidecar frame golden, **re-diff gate GREEN** (29 priors byte-identical); **RotateFrom observable** — hash constant per 8-tick window, flips on `cycles>>3` @8/16/24; **triple isolation proof** — `state_hash` Rust == sidecar == sim golden, untouched by rendering) |
| Step 2 | **✅ COMPLETE** (all slices bit-exact; merged in PR #3) |
| Slice 5′b (bonus pickup) | **✅ MILESTONE GREEN** (the 5c-deferred pickup block `worm.cpp:287-322` live: 11×11 AABB gate + health/weapon/booby branches, TC consts threaded unhashed; `sim_slice5prime_pickup_health` — a worm wounded to 50 **walks onto** a dropped health bonus, heals @tick 87/110t, pool 1→0 — and `sim_slice5prime_pickup_weapon` — walk-on reload @tick 67/90t, `ww` 3→2, health flat all ticks (the booby discriminator) — both master+9 components bit-exact vs C++ **first run**; booby branch pinned by RED-first unit tests with exact per-branch draw counts; pure-Rust slice, priors byte-identical; chain-loop → slice 6) |
| Slice 5′a (per-pixel worm-hit + in-flight arms) | **✅ MILESTONE GREEN** (real per-pixel `CheckForSpecWormHit` replaces 5a's box; both **in-flight worm-hit arms** live — wobject **blood-before-sound**, nobject **sound-before-blood** (opposite RNG order, each golden-witnessed); `sim_slice5prime_golden` (dart, 71 ticks) + `sim_slice5prime_nobj_golden` (cannon splinter, 156 ticks) master+9 components bit-exact vs C++ **first run**; **near-miss ticks** (projectile in the 16×16 box on transparent pixels) fire **nothing** — the anti-box witness pinning `fd33bbc` as FIXED; pure-Rust slice, slices 1–5d byte-identical; **pickup → 5′b**) |
| Slice 5d (death + respawn) | **✅ MILESTONE GREEN + fuzzed** (`sim_slice5d_golden` master+9 components **all 361 ticks bit-exact** vs C++; the **worm death→respawn path goes live** — worm1 (health 12) dies from the explosives blast @death-tick [`rng` bursts 120-blood+8-gib spray, `visible`→false, `lives`−1, worm0 `kills`+1], the invisible 150-tick `killed_timer` counts down to `BeginRespawn` @tick 237 [the level-reading RNG spawn search: `pos` JUMPS, trial-count `rng` burst], then `DoRespawning` completes @tick 304 [`visible`→true, `health`→100]; slices 1–5c stay byte-identical. **4-variant fixed-level respawn fuzz** exhibits distinct bounded trial counts {2,3,6,7} — the desync trap's variance proven vs the C++ oracle) |
| Slice 5c (bonuses) | **✅ MILESTONE GREEN** (`sim_slice5c_golden` master+9 components 501 ticks; **`bonuses` pool live** — drop @tick 252 → falls/bounces under `Bonus::Process`, timer still counting at window end; worms clear (no pickup); spawn-flash `detectRange=0` ⇒ chain-loop inert & proven neutral; slices 1–5b byte-identical; pickup + chain-loop port deferred → slice 6) |
| Slice 5b (worm damage + blood) | **✅ SHIPPED** (PR #3; `sim_slice5b_golden` master+9 components 121 ticks; worm wounded 100→82 + bleeds, **`bobjects` pool live**; `cycles` now advances; wobject bounce+animation flight branches ported; per-pixel worm-hit deferred → follow-up) |
| Slice 5a (splinters) | **✅ SHIPPED** (`sim_slice5a_golden` master+9 components 131 ticks, debug+release; `BlowUpObject` splinter arm + `NObject::Process` `create_on_exp`/explode arms live; on PR #3) |
| Slice 4 (weapon lifecycle) | **✅ SHIPPED** (4a + 4b + 4c + 4d all bit-exact vs C++) |
| Slice 4c | **✅ SHIPPED** (sobjects/nobjects pools live + carving DrawDirtEffect, master+9 components bit-exact 91 ticks vs C++, on PR #3) |
| Slice 4d | **✅ SHIPPED** (dig + shell-drop/landing-blit + reload + load_change; HANDGUN, master+9 components bit-exact 126 ticks vs C++; `BlitImageOnMap` + small-sprite bank added) |

---

### Step 3 — Rendering (✅ COMPLETE · PR #4)

Port the C++ CPU-bitmap renderer into a **Bevy-free `render` crate**, pixel-gated by an
FNV-1a **frame hash** differential-tested against C++ over the reused Step 2 scenarios
(Classic color mode). The authoritative frame is the CPU `Bitmap`; Bevy only presents it
(not bit-gated). Six slices accumulate on branch `liero-rs-step-3`.

```
├─ ✅ 3a  render-crate foundation — Bitmap/palette-build/DrawLevel/Viewport/frame-hash   SHIPPED
│         🎯 FIRST pixel-exact TERRAIN frame vs C++: render_slice3a_golden — all 27 frame
│         hashes + total accumulator bit-exact (matched first run); RotateFrom proven
│         observable (hash constant per 8-tick window, flips on cycles>>3 @8/16/24); C++
│         dumper opt-in `render player` directive → sidecar frame golden, re-diff gate GREEN
│         (29 priors byte-identical); triple isolation proof (state_hash untouched)
├─ ✅ 3b  shadow + sprite pass — two-pass world block; LightUp/shake/laser-sight RNG live   SHIPPED
│         🎯 the WORLD VIEW is pixel-exact vs C++: 7 goldens (laser/shadow/shake/fan/dart/
│         blood/dart_water), 225 frame rows, ALL matched first run; both viewport RNGs live
│         & proven non-vacuous (laser 6 distinct hashes; shadow ON≠OFF; shake+flash blip;
│         pool-drain incl. positive BlitImageR-over-water); triple isolation per tick;
│         re-diff GREEN (all sim_slice* + render_slice3a byte-identical). 2 real finds:
│         inverted laser rand(2) order (T3 review) + C++ cossin[128] UB on facing-flip (T0b)
├─ ✅ 3c  Bevy window (native) — FixedUpdate sim tick, CPU buffer → Image → Sprite/Camera2d   SHIPPED
│         🎯 `cargo run -p game` shows Liero LIVE — the project's FIRST Bevy code: a 960×600
│         window presents the blood scenario in real time, sim on FixedUpdate at the exact C++
│         cadence 1000/14 ≈ 71.43 Hz (kDelay=14ms, gfx.cpp:1176 — NOT the 60 Hz first assumed),
│         CPU frame (Bevy-free render) → Image → Sprite ×3 nearest; scenario loops bit-identically
│         off its recorded inputs, debug determinism guard (per-tick state_hash vs golden) GREEN
│         over ~26 loops/15 s. New Bevy-free `scenario` crate (parser verbatim from oracle-tests +
│         loader factored out of the T8 harness — 3d reuses it); CI keeps the determinism gate
│         Bevy-free (--exclude game, proven via cargo tree) + separate cargo build -p game. 2 finds:
│         bevy_sprite alone ships no GPU backend (needs sprite_render/core_pipeline/render/wgpu) +
│         empty-inputs divergence (recorded inputs fed instead; guard caught it live)
├─ ✅ 3d  headless screenshot CLI + in-repo run-skill (change → screenshot → judge, no GPU)   SHIPPED
│         🎯 Bevy-free `shot` crate PNG-encodes any 3b scenario at a fixed tick to a known path
│         (raw RGB, nearest ×scale, pitch-correct, no fade so tick 0 is not black) + dumps
│         machine-readable per-tick frame_hash+state_hash to stdout behind --hashes; golden-test
│         (rust/shot/tests/golden.rs) locks the CLI render bit-for-bit vs the C++ sidecars for
│         blood + shake (the copy's one new path: flash/shake injection), GREEN first run; first
│         in-repo run-skill .claude/skills/liero-shot drives change→screenshot→judge→compare.
│         Per-tick driver is a CLI-LOCAL copy of the T8 harness (option B — T8 untouched, golden
│         guards drift; factoring → third consumer). CI unchanged (--workspace sweeps shot, pure-Rust
│         png-encode, goldens committed; 11 shot tests green in the exact CI command)
├─ ✅ 3e  HUD / font / bars / minimap — the full player view, pixel-gated   SHIPPED
│         🎯 the FULL PLAYER VIEW is pixel-exact vs C++: render::font::Font (font.tga loader
│         + font.cpp:8-80 verbatim, ASCII-identity for cp<0x80 bevis-tested), blit::draw_bar
│         (blit.cpp:105-113 + width>0 anti-clamp witness), render::hud::draw_hud (viewport.cpp:
│         84-153 verbatim: 2-arm life/ammo bars, blinking Reloading, kills/lives, colour columns),
│         draw_minimap+draw_miniature (viewport.cpp:593-613 + level.cpp:489-507, two ceil-vs-round
│         idioms + worm-dots). frame::draw composites HUD→world→minimap per viewport; C++ dumper
│         opt-in render_hud, RE-DIFF gate empty (all priors byte-identical). 3 goldens: hud 41t +
│         death 141t GREEN first run (triple-isolation + suppression + non-vacuity); reload re-cut
│         RIFLE→GRENADE (dodged the deferred ST_LASER do-loop) → GREEN 71t. Milestone review 0/0.
└─ ✅ 3f  wasm bring-up — WebGL2 browser build, embedded assets, CI wasm-build job   SHIPPED
          🎉 the BROWSER milestone, automated-proven: a headless-Chrome controller ran the
          debug wasm bundle (SwiftShader-WebGL2, 30 s virtual time) — the screenshot shows the
          blood demo's split-screen world (sky/terrain/worms/blood), the console PANIC-FREE, and
          the determinism guard (per-tick state_hash + a wasm-only frame_hash) stayed GREEN in
          the browser = the wasm-parity witness. read_asset seam (native = verbatim std::fs::read,
          all goldens green = no-op proof; wasm = embed) + include_dir (wasm-only) sprites/weapons/
          nobjects/sobjects + include_bytes! tc.cfg + curated 276 KB Levels (sounds/ + big levels
          excluded); target-scoped feature split (x11/wayland native-only, webgl2 wasm-only, union
          verified per target via cargo tree) ⇒ cargo build -p game --target wasm32 GREEN; entry-fork
          (const DEFAULT=blood, include_str! scenario+sidecar, no env::args/read_dir/fs on wasm;
          canvas auto-append); .cargo/config.toml (wasm-server-runner) + web/index.html dev-loop
          (127.0.0.1:1334) + static wasm-bindgen 0.2.126 bundle (game.js 97 KB + game_bg.wasm 52.9 MB
          debug); new game-wasm CI job (build-only, no browser/apt, independent of the determinism
          gate). Fynd: cargo reads .cargo/config.toml from CWD, not --manifest-path (dev-loop runs
          from rust/)
```

**Deferrals carried after 3b:** Modern `ColorMode` display-halve/`ResolveDisplayAt` arms;
steerable centering (`WormState` has no `steerable_sum` — assert kept); `bonus_frames` empty
+ `BONUS_FLICKER_TIME` hardcoded (no bonus scenario yet); spawn-preview `BlitImageTrans`
unreached (`names_on_bonuses=false`, no `kChange`); Rust-parser layout-token value validation
(inert until more layouts exist); C++ render-path edits are **not** caught by CI's re-diff
(the gen-scripts are local); the `cossin`-UB sites (ninjarope/`worm_fire`) left unmasked;
`MAT_SEE_SHADOW`-constant placement in `render`. Name labels / font (`DrawTextSmall`) +
HUD/bars/banners/holdazone/minimap → **3e**. (3a's `LightUp`/shake/laser-sight RNG are now
live; steerable centering stays deferred.) A future scenario reaching any of these must lift
the deferral with a matching golden.

**Deferrals carried after 3c** (each routed to the slice that needs it): keyboard **input** /
game-loop-with-input and **audio** (Step 4); **render interpolation** (`overstep_fraction` lerp —
draw the latest tick for now); **resize-aware integer-fit camera** (fixed ×3 window shipped);
**follow-cam** (`--follow` / `killed_timer` zeroing — diverges from goldens, allowed later, not the
default); **live shake/flash wiring** (the `ProcessViewports` equivalent — Step 4); **Srgb-vs-Unorm
texture-format** visual verification (advisory — resolved by eyeball); plus minor tidy carried:
`blit.rs` fmt-drift fixup, a scenario-without-sidecar debug-panic comment, and the setup-tick-0
assert gap. **HUD/font/bars/minimap → 3e; wasm (WebGL2, embedded assets) → 3f** (no wasm feature
pulled in 3c). The shared `scenario::load` is exactly what **3d** reuses for the headless CLI.

**Deferrals carried after 3d** (each routed to where it lands): **HUD/font/bars/minimap** → 3e;
**wasm / headless-browser-canvas** (WebGL2) → 3f; **.lrp-replay + input-timelines** → Step 4 (`shot`
becomes the replay-regression driver then); **driver factorisation into `scenario`** (the CLI-local
per-tick copy stays until a third consumer needs it); **video / GIF output** — out of scope (`image`
is png-only by design); a **standing CI diff-job for `--hashes`** — redundant, the golden **test**
already gates the render in the exact CI command. Plus review minors: parse-quirks (flag-as-value
consumed silently, last-wins duplicates), the pitch≠w test's `h=1` non-vacuity. A future consumer
reaching any of these lifts the deferral with a matching golden/test.

**Deferrals carried after 3e** (each routed to where it lands): **death banners** (`viewport.cpp:236-267`)
— structurally unreachable (`banner_y`-state steps in viewport processing that neither the dumper nor the
Rust sim runs) → **Step 4** (adjudicated, *not* part of 3e); **name labels / `DrawTextSmall` + text.tga**
→ Step 4 / future (the 3e labels are ASCII-proven, so the full CP437 table is not yet needed);
**Holdazone / GameOfTag / replay HUD-arms** — tripwired (no scenario exercises them); **spawn-preview
`BlitImageTrans`** — unreachable (`names_on_bonuses=false`, no `kChange`); **`fill_rect`** — replay-only,
still deferred; the **RIFLE / `ST_LASER` laser-do-loop** — a *sim* deferral (the simmen panics on the
deferred laser do-loop, so the reload golden uses **GRENADE** until the do-loop is ported — then reload
can move back to RIFLE); C++ render-path edits are still **not** caught by CI's re-diff (gen-scripts are
local). Plus review minors carried: DRY the inlined `clip_image`, a `size>1`-advance font test, minimap
dot-index discrimination, and the reload difftest's own suppression control (deliberately omitted — hud +
death already carry it). A future scenario reaching any of these lifts the deferral with a matching golden.
(3e closed the after-3b/3c/3d HUD/font/bars/minimap deferral in full; the draw_char OOB-doc minor from T1
was fixed in `d4c195f`.)

**Deferrals carried after 3f** (STEP 3 complete — each routed to where it lands): **`?scenario=`
query-param** (the wasm entry hardcodes `DEFAULT="blood"`; a URL-selectable scenario is a small
follow-up, not needed to prove the browser milestone); **wasm-opt / size optimisation** (the release
`game_bg.wasm` is 52.9 MB — that is the **Bevy-wasm baseline**, *not* the embedded assets, which are
only **276 KB**; `wasm-opt`/`--release` trimming is a later polish, not a correctness gate); **browser
input + audio** → **Step 4** (the wasm demo is scenario-driven off recorded inputs, same as native 3c);
the **wasm release-guard is OFF by design** (the per-tick `frame_hash` determinism guard is **debug-only**
— the native frame is already oracle-gated, the wasm guard is the debug-build parity witness, so release
carries no guard); the **RIFLE / `ST_LASER` laser-do-loop** still stands (a *sim* deferral inherited from
3e — unrelated to wasm). C++ render-path edits remain **not** caught by CI's re-diff (gen-scripts are
local). A future consumer reaching any of these lifts the deferral with a matching golden/test.

---

### Step 4 — Input / `.lrp` replay / audio (✅ COMPLETE · branch `liero-rs-step-4`)

Close the play loop: real keyboard input at the fixed cadence, record/replay determinism,
audio as a pure sim-event consumer, the live viewport (shake/flash/banners), and byte-faithful
`.lrp` reading. Overview: `specs/2026-07-12-liero-rs-step4-input-replay-overview.md`; C++ fact
map: `specs/2026-07-12-liero-rs-step4-cpp-input-replay-map.md`.

```
├─ ✅ 4a  live input core — keyboard → per-tick ControlState snapshot into process_frame   SHIPPED
│         🎯 native `cargo run -p game -- --live` is PLAYABLE (1P + 2P hotseat): a pure
│         generic per-worm sampler (PlayerBindings<K>+control_state), Dig = a pure L+R chord
│         (never stored), default bindings mirror C++ index-for-index; sampled once per
│         FixedUpdate tick via a new InputSource{Scripted,Live}; the headless pass-through
│         gate (game/tests/passthrough.rs) proves the sampler reproduces all 7 render_slice3b_*
│         goldens bit-exact in CI. Focus-loss needs no backstop (Bevy 0.19 auto-releases held
│         keys). Deferred: live-wasm, gamepad, recording (→4b)
├─ ✅ 4b  record/replay round-trip + CI regression — THE HARD GATE (self-checking, no C++)   SHIPPED
│         🎯 the Step 5 / ggrs precondition is delivered: Recorder taps the same sampled
│         ControlState array --live feeds process_frame, flushes on AppExit (.after(ExitSystems) —
│         review-caught window-close race); the recorded artifact IS a scenario file (to_text
│         serializer, no new format); --replay <path> drives any scenario headless through the
│         unchanged InputSource::Scripted (play once then hold). round_trip.rs: a synthetic
│         key-stream through the REAL --live sampler → Recorder → to_text → parse → replay
│         reproduces the live HashGameState series tick-for-tick, non-vacuously (3 asymmetric
│         fixtures + assert_ne vs empty + hardcoded raw-text facit pinning the Dig chord); RED
│         (mutated word) diverges at the exact tick. Committed corpus
│         (record_slice4b_blood_scenario.txt, 12 ticks) + sim-produced state_hash sidecar
│         drift-backstop test + reproducible generator (gen_record_slice4b_corpus). Hardening
│         finding H1 (escalated from the T3 review): C++ cossin[128] UB was reachable in --live
│         after all (spawn→walk→fire; clamp gated on aiming_speed!=0) — 3 sites masked &0x7f
│         (worm_fire, ninjarope-throw, dig), full sim golden suite green (313 sim tests, neutral
│         fix). 7 commits, 0 Critical/0 Important throughout (one early NEEDS-FIX fixed + re-reviewed)
├─ ✅ 4c  audio — sim emits a sound-event stream (zero new rand; variant RNG already in sim),
│         thin Bevy-free rodio sink in game (native + wasm); isolation gate      SHIPPED
│         🎯 Liero-rs LÅTER (native + browser): T1 sim::sound event stream (10 callsites,
│         thread-local collector, hash-inert). T2 CLASSIFICATION FINDING — C++
│         `Play(sound, id, loops=0)` default means the design's 5 "worm-keyed loops" were
│         actually `IsPlaying`-dedup'd ONE-SHOTS; the only true loop is the weapon-launch
│         site (`worm.cpp:1120-1121`, WormWeapon-keyed, 3 verbatim Stop sites) — design spec
│         + `LoopKey::Worm` doc corrected; hit/blood trio emitted as `key=None` one-shots
│         (Väg A, dedup lost = audio-advisory). T3 `AudioSink` trait + rodio 0.19 (no default
│         features) + idempotent Drainer + reaper + `SoundTable`. T4 MILESTONE: wired live in
│         windowed play (`NonSend` cpal stream, Null-fallback, liveness-superset reap closes
│         the death-leak edge); headless stays structurally silent. T5: rodio on wasm too
│         (same `RodioSink`, no kira needed) — `sounds/` (~505 KB) embedded, silent-until-gesture
│         autoplay, no panic (one native/wasm asset-miss inconsistency documented, not unified).
│         Commits `5f10312`/`966c362`/`59b6fef`/`1ffc68e`/`2b8fc6c`, 0 Critical/0 Important
│         throughout. Ear-check advisory (not gated)
├─ ✅ 4d  live shake/flash/banners — the ProcessViewports port Step 3 deferred      SHIPPED
│         🎯 MILESTONE: flash/shake/camera/banners evolve from real explosions + a real
│         spawn/death, bit-exact vs a new C++ render_live golden over 321 ticks. T0 headline
│         correction: `screen_flash` is NOT in C++ HashGameState (stateHash.hpp omits it,
│         rollback-only) — added to SimState hash-neutrally (decrement game.cpp:271-273,
│         sobject-create max-write sobject.cpp:41), gated by re-diff not re-fuzz. T1: sole
│         shake writer (sobject.cpp:31) emits raw blast-coord+amount events; game layer does
│         itof/rect/max, keeping sim viewport-blind. T2: game-layer live stepping
│         (viewport_step.rs) — decrement-without-floor + banner-walk on pre-frame state, THEN
│         process_frame, THEN apply shake-max, THEN as_scene(sim.screen_flash) — matches
│         ProcessViewports's real call site (game.cpp:463, after the worm loop). T3: C++
│         dumper's opt-in render_live directive (full ProcessFrame + wired viewports), green
│         re-diff (every prior golden byte-identical) + Rust parser arm. T4 MILESTONE: golden
│         matched first run — flash+shake from real explosions, camera moves live on respawn
│         (killed_timer vacuity trap from the design doc solved via fire-triggered spawn),
│         triple-isolation + 4 non-vacuity witnesses; harness is a documented, double-anchored
│         hand-copy of viewport_step.rs (Bevy !Send/ECS forces the duplication). T5: death-
│         banner text via the 3e font (cross-viewport: dying worm's banner in the OTHER
│         viewport, viewport.cpp:256-270). Finding+fix: frame::draw interleaved process+draw
│         PER viewport (wrong) — refactored to process-ALL-first (matches ProcessViewports),
│         hash-neutral (each viewport RNG fully local; 3a/3b/3e goldens unchanged); golden
│         regen touched ONLY the frame-hash column, ONLY the death window (ticks 93-236) —
│         state-hash column stayed byte-identical. 6 commits, 0 Critical/0 Important
│         throughout. Deferred: steerable_sum centering (real DEFER — ProcessSteerables
│         mutates hashed wobject.cur_frame, unported, no reaching scenario; non-steerable
│         SetCenter + debug_assert guard stand); YoureIt/GameOfTag banner arm (comment, not a
│         hard tripwire); minor — the harness hand-copy is a theoretical frame-level blind
│         spot, mitigated (not eliminated) by double-anchoring
├─ ✅ 4e  .lrp byte-faithful reader — phase 1 SHIPPED (container + XOR-delta stream +
│         WideRollbackChecksum); phase 2 (cereal Game graph) is a bounded follow-on
│         🎯 MILESTONE: a real C++-produced .lrp drives the Rust sim bit-exact over 1120 ticks,
│         incl. the WideRollbackChecksum word at cycle 1050 (0xf6211087). Phase-boundary finding:
│         cereal blobs are length-prefixed, so phase 1 skips the whole cereal Game graph outright
│         — initial state comes from scenario::load instead — and WideRollbackChecksum is a
│         HARDER gate than HashGameState (folds the whole rollback inventory incl.
│         prev_control_states + terrain, not just the tick's visible state). T0: lrp_gen (C++
│         headless replay-writer, setup verbatim from the dumper) + 320t/1120t corpus,
│         tick-0-alignment proven. T1: the WideRollbackChecksum port (Mix32 exact, 21 worm fields
│         + pools + material buffer, prev_istates caller-supplied), cycle-0 facit matched first
│         run. T2: Bevy-free rust/replay crate (flate2) — BE container, cereal-skip, tags
│         0x80-0x83, XOR-delta decode, checksum extraction, verified against all 1120x2 input
│         words via the scenario grammar (XOR-baseline caveat routed to phase 2). T3 MILESTONE:
│         phase-1 gate GREEN first run — 1120 ticks bit-exact incl. the cycle-1050 word, per-tick
│         tripwire + negative test. T4: hostile-input hardening (10 tests, no panic on garbage,
│         O(1) Err on a huge length-prefix) + debug_asserts + phase-2 bookkeeping (spec §8: cereal
│         graph / version<7 lift [adjudicated: rejected for phase 1 — palette-only inside the
│         cereal blob] / decode_with_baseline / framehash diff). 6 commits (fb9b9b7 plan/
│         d0cca91 T0/0a08d54 T1/b5fb884 T2/cd711fc T3/b8bd8cb T4), 0 Critical/0 Important
│         throughout. Deferred: phase 2 in full (cereal Game graph — possibly its own post-step
│         item, spec Open Q1)
├─ ✅ 4f  minimal start flow — new match with defaults + respawn + quit/restart (no menu tree)   SHIPPED
│         🎯 MILESTONE: bare `cargo run -p game` (no flags) now starts a SPELBAR default match —
│         Live mode, the `render_stage`-level default_match fixture (blood-bevised worm
│         positions, DART, seed 42; the fixture sits outside the golden dir so it can't perturb
│         any golden enumeration). F5 restarts (adjudicated to also zero an in-flight recording —
│         restart means restart); a positional `default_match` arg is rejected cleanly (T0 review
│         NEEDS-FIX → fixed); the window title is tidied. wasm arm stays byte-unchanged (the
│         parity witness holds). Deferred: `GenerateFromSettings`/random-level generation
└─ ✅ 4g  run/verify-skill extension + CI replay-checksum regression (woven in per gate)   SHIPPED
          🎯 `shot --scenario-path <file>` renders any recording or arbitrary scenario file (the
          golden path itself stays byte-identical, 19 shot tests green); SKILL.md gains §7 —
          live play + the full key-binding table, the record→replay loop (incl. the flush caveat:
          Esc/window-close flushes, a killed process writes nothing, F5 zeroes an in-flight
          recording), headless verification, and a CI-coverage inventory (every gate already
          wired into CI). The CI half was de facto delivered incrementally across 4a–4e, so 4g's
          own diff is thin by design. 5 commits (80cb75c plan/7146490 T0/41a1af2 T0-fix/f566404
          T1/e22c90a T2); T0 NEEDS-FIX → 2 Important fixes → clean; T1/T2 self-verified, covered
          by the broad step-4 review (T4)
```

**STEP 4 = COMPLETE (4a–4g).** Remaining for John: the PR #5 merge decision + the `.lrp`
phase-2 (cereal `Game` graph) follow-on decision — both bookkept, neither blocking.

Adjudicated (controller, 2026-07-12): 4e split as above; corpus generated from C++ over the
existing scenario inputs; audio backend chosen at 4c start (kira or rodio, NOT bevy_audio);
minimal menu only; steerable_sum decided at 4d start — **resolved DEFER** (not the leaning-add
premise: `ProcessSteerables` mutates the hashed `wobject.cur_frame`, is unported, and no
scenario reaches it, so the accumulators would be vacuous without also authoring a
steerable-weapon scenario + golden + re-fuzz; the non-steerable `SetCenter` + guard stand);
TWO input formats by design (Rust-native round-trip artifact ≠ foreign `.lrp`).

---

### Step 4½ — Game shell (🟡 IN PROGRESS — 4½a-1 ✅, 4½b ✅ · branch `liero-rs-step-4-5`, PR #7)

Turn "plays a hard-coded match" into a complete game, close to or exactly like openliero: bare
`cargo run -p game` opens the main menu over a generated level; a match is configured, weapon-
selected, played (vs a human or DumbLieroAI), ended and summarised on a stats screen using only
the menus; C++-saved `liero.cfg`/profiles round-trip byte-identical; the same shell runs on wasm.
Sim-affecting parts are C++-golden-gated; menus get Rust-only frame-hash self-goldens + PNG
eyeballing vs the C++ build. Overview: `specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md`;
fact maps: `specs/2026-09-10-liero-rs-step4.5-cpp-game-shell-map.md` (C++) and
`specs/2026-09-10-liero-rs-step4.5-rust-baseline-map.md` (Rust).

```
├─ 🔶 4½a  split (design §0). 4½a-1 ✅ LANDED: Settings/WormSettings/MatchConfig + C++ TOML reader
│          + build_match (reproduces sim_slice6_fuzz5) + CorrectShadow + Scales/GameOfTag rules +
│          IsGameOver + MatchFlow (180-frame post-mortem) + sound_hooks fix. Gate: `settings <file>`
│          dumper directive, 4 settings-driven goldens bit-exact incl. IsGameOver.
│          🎯 MILESTONE GREEN 2026-09-10: 4/4 variants, 5830 rows (defaults 401, killemall 1135,
│          scales 2483, gametag 1811). The matrix exposed a Step-2 gap — WObject
│          collide_with_objects impulse loop (weapon.cpp:212-232) — ported in T8b.
│          4½a-2 ⬜: TOML writer + byte gate (vs C++ load+save) + UpdateHash + storage + HUD fix
├─ ✅ 4½b  random level generation (sim::levelgen: GenerateRandom stages, MakeShadow,
│          generate_from_settings), dedicated Rand seeded from the match seed; SelectSpawn
│          deferred with Holdazone. 🎯 MILESTONE GREEN 2026-09-10: new oracle_dump_levelgen
│          golden — 49/49 lines (42 gen × 3 seeds × 7 sizes × shadow on/off + 7 file/fallback,
│          incl. a MakeShadow fixture) bit-exact across every stage + rand.last + rock stats,
│          FIRST RUN. T8 (eyeball example, README) done. T9 done: the 21 shadow=1 dig tokens
│          now run through 4½a's correct_shadow — bit-exact, first run          COMPLETE
├─ ⬜ 4½c-0 unported Step-2 weapon branches (RIFLE, WINCHESTER, LASER, GAUSS GUN, MISSILE — the
│          laser do-loop + ProcessSteerables) ported bit-exact with sim goldens BEFORE 4½c makes
│          them choosable; banned from 4½a's goldens (added 2026-09-10, 4½a design §11 Q1) not started
├─ ⬜ 4½c  weapon selection phase in sim (draws the sim RNG) + 12/3 key repeat + per-viewport
│          menu + frozen-screen look. Gate: new oracle_dump_weapsel, bit-exact RNG     not started
├─ ⬜ 4½d  menu framework (Menu/MenuItem/behaviors, DrawRoundedBox, scrollbar, type-to-search,
│          168..174 palette rotation, fades, menu sounds) + ScreenStack + main menu
│          🎯 MILESTONE: bare run → main menu → NEW GAME → weapsel → play → Esc → menu
│          (RESUME/NEW) → QUIT. Gate: menu self-goldens + full re-diff             not started
├─ ⬜ 4½e  settings menu (per-game-mode visibility) + weapon availability + level selector (file
│          picker, RANDOM node, minimap preview) + save/load setups + InputString/InfoBox
│          overlays                                              (parallel with 4½f)   not started
├─ ⬜ 4½f  player menu (name, health, RGB bar, key bindings, weapons via Levenshtein,
│          controller Human/DumbAI) + profiles + DumbLieroAI port (own Rand). Gate: fixed-seed
│          AI control-state stream, bit-exact                    (parallel with 4½e)   not started
├─ ⬜ 4½g  match end + compact stats screen (hash-inert StatsRecorder subset; no heatmaps/graph)
│          + hidden options subset (fullscreen, shadows, powerlevel palettes, auto-record, bot
│          weapons)                                                                      not started
└─ ⬜ 4½h  wasm: live keyboard, localStorage persistence, wider embedded level manifest — the
           whole shell in the browser (wasm debug self-check retires on that path)   not started
```

Deferred out of 4½: modern (non-pixel-exact) UI (later post-step), FollowAI (needs Step 5a
snapshots), netplay menus/states (Step 5), the F8 weapon-randomiser easter egg, spectator window,
TC selector, stats heatmaps/graphs, `.lrp` replay browser (needs `.lrp` phase 2), gamepad.
Open for John: the level corpus — ship stock levels, or rely on random generation (overview
§Open Q6); where the laser/steerable weapons deferred from Step 2 get ported — the controller
ruled a dedicated slice 4½c-0 before 4½c (4½a design §11 Q1; the alternative is hiding them
with `weap_table = 2` until ported); and the restated TOML byte gate — the shipped files are
legacy formats C++ itself rewrites, so 4½a-2's gate is "Rust save == C++ save for the same load",
which rewords a signed-off done-when (4½a design §3.4, §11 Q3).

---

## ✨ New capabilities — beyond the original (not started; future)

These do **not** exist in openliero today. They become possible once the rewrite
gives a clean, deterministic, embeddable Rust sim. Tracked separately — they do
not count toward the rewrite %.

```
✨ NEW
├─ ⬜ RL / self-play training (local, M2 Max)        needs the sim (step 2) first
├─ ⬜ Web / wasm build (browser reach)               new platform
├─ ⬜ WebRTC P2P netplay (serverless, lightweight)   ← NEW (original uses ENet + a Go relay)
└─ ⬜ Agent-driven dev harness                        new tooling
      (run/observe/iterate + headless screenshots
       + checksum-regression gate)
```

> Note: native ENet netplay, rollback, replay (.lrp), and video export already
> exist in openliero → those live in the **rewrite** track above, not here.
> "New" is specifically web/wasm + WebRTC, RL/self-play, and the agentic dev loop.

---

## How it's built

Subagent-driven: per-task implement + two-stage review (spec + quality), a broad
whole-slice review before each push, all bit-exact vs the C++ oracle. PR #3
accumulated all of step 2 (merged); PR #4 accumulates all of step 3 (rendering)
and is merged only when the whole step is complete.
