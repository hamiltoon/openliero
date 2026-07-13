# Liero-rs — progress at a glance

> Two tracks:
> **🔁 Rewrite** — a faithful port of existing OpenLiero (C++/SDL3) to Rust (+ Bevy),
> proven **bit-for-bit** against the C++ engine as a truth oracle.
> **✨ New** — capabilities the original never had (enabled once the rewrite lands).
>
> The headline % tracks the **rewrite**; the **new** track is exploratory/future.
> The dense machine ledger lives in `.superpowers/sdd/progress.md` (gitignored).
>
> **Last updated:** 2026-07-13 · **STEP 4 — slice 4b SHIPPED: record→replay round-trip,
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

## 🔁 Rewrite track — faithful port (~76%)

Strangler-style: the C++ engine is the oracle, every piece differential-tested
bit-for-bit before moving on. Steps 0–2 merged (the deterministic sim core — the
hardest part — is bit-exact vs C++); **step 3 (rendering) is COMPLETE** (3a–3f): slices
3a + 3b shipped the pixel-exact terrain frame **and** the full world view (shadow +
sprite pass), **3c** put it LIVE on screen (`cargo run -p game` — the first Bevy
code), **3d** the headless screenshot CLI + run-skill, **3e** the HUD/font/bars/
minimap overlay (the full player view pixel-exact), and **3f** the wasm bring-up —
the same CPU frame now renders in the **browser** (WebGL2), automated-proven in
headless Chrome. Step 4 (input/replay/audio) is in progress — slices 4a (live input)
and 4b (record→replay round-trip, the Step 5 precondition) are shipped; step 5 (netplay)
is not started.

```
REWRITE (steg 0–5)                                          ~76%
├─ ✅ Step 0  sim-core primitives (RNG/fixed/vec/math/tables)   DONE — merged (PR #1)
├─ ✅ Step 1  asset IO, slices 1a–1e (level/palette/sprites/    DONE — merged (PR #2)
│             tc.cfg/objects/WAV)
├─ ✅ Step 2  deterministic sim core                            COMPLETE — merged (PR #3)
├─ ✅ Step 3  Bevy rendering / window (reproduce the SDL3 view) DONE — 3a–3f shipped (PR #4)
├─ 🟡 Step 4  input + replay (.lrp) + audio                     IN PROGRESS — 4a+4b shipped, 4c–4g remain  ◀── YOU ARE HERE
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
| Rewrite track (steps 0–5) | **~76%** (steps 0–3 done; step 4 input/replay slices 4a+4b shipped, 4c–4g + step 5 netplay remain) |
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

### Step 4 — Input / `.lrp` replay / audio (🟡 IN PROGRESS · branch `liero-rs-step-4`)

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
├─ ⬜ 4c  audio — sim emits a sound-event stream (zero new rand; variant RNG already in sim),
│         thin Bevy-free kira/rodio sink in game; isolation gate; wasm Web Audio
├─ ⬜ 4d  live shake/flash/banners — the ProcessViewports port Step 3 deferred; render golden
│         on a live (not injected) flash/shake scenario; steerable_sum decision at slice start
├─ ⬜ 4e  .lrp byte-faithful reader vs C++ framehash — SPLIT: container + XOR-delta stream +
│         WideRollbackChecksum first (vs Rust-produced initial state), cereal Game graph as a
│         bounded follow-on; corpus GENERATED from C++ (none exist in-repo)
├─ ⬜ 4f  minimal start flow — new match with defaults + respawn + quit/restart (no menu tree)
└─ ⬜ 4g  run/verify-skill extension + CI replay-checksum regression (woven in per gate)
```

Adjudicated (controller, 2026-07-12): 4e split as above; corpus generated from C++ over the
existing scenario inputs; audio backend chosen at 4c start (kira or rodio, NOT bevy_audio);
minimal menu only; steerable_sum decided at 4d start (leaning add + re-fuzz); TWO input formats
by design (Rust-native round-trip artifact ≠ foreign `.lrp`).

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
