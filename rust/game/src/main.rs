//! Liero-rs `game` binary — the project's first Bevy code (Slice 3c).
//!
//! T3 (this file): the sim goes live. The `blood` scenario is loaded at startup,
//! ticked once per `FixedUpdate` at `1000/14 Hz`, rendered with the Bevy-free
//! `render` crate into an owned 320×200 ARGB `Bitmap`, blitted into a Bevy
//! `Image`, and shown as one ×3-scaled `Sprite` under a `Camera2d`. When the
//! scenario's tick count is passed the loader is re-run at tick 0 for a
//! bit-identical loop. A `#[cfg(debug_assertions)]` per-tick sim-hash self-check
//! against the committed golden guards the determinism firewall for free.
//!
//! The sim is a plain `Resource` that Bevy only *ticks and presents*: `SimState`
//! is mutated ONLY in `tick_and_render`, only via `process_frame`, and no Bevy
//! value ever flows into it — the Step-5 `bevy_ggrs` rollback shape.
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::WindowResolution;

use render::bitmap::Bitmap;
use render::viewport::Viewport;
use scenario::{Scenario, SceneData};
use sim::state::SimState;

use game::input::{InputSource, Mode, ParsedArgs, Recorder};

mod blit;

/// TC asset root, resolved at compile time relative to this crate so `cargo run
/// -p game` works from any CWD (constraint: CARGO_MANIFEST_DIR, not CWD).
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// Committed golden dir — the scenario text and (debug) the self-check column.
/// Native-only: on wasm the scenario text + sidecar are embedded via `include_str!`
/// (there is no filesystem), so this path constant is not referenced there.
#[cfg(not(target_arch = "wasm32"))]
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");
/// Default demo scenario when no positional arg is given (`cargo run -p game`).
const DEFAULT_SCENARIO: &str = "blood";
/// Native low-res canvas the `render` crate paints (one Liero screen).
const SURFACE_W: u32 = 320;
const SURFACE_H: u32 = 200;

/// The chosen scenario name (positional CLI arg, default `blood`). Read once in
/// `main` (validated before any window opens) and threaded into `setup`.
#[derive(Resource)]
struct ScenarioName(String);

/// The 4b `--record <path>` flush target (`None` unless `--live --record` was
/// given). Threaded into `setup`, which inserts a [`Recorder`] iff this is
/// `Some` and the mode is Live. Always `None` on wasm (no CLI args there).
#[derive(Resource)]
struct RecordPath(Option<PathBuf>);

/// The 4b (T2) `--replay <path>` source target (`None` unless `--replay` was
/// given; always paired with `mode == Mode::Replay`, enforced by
/// `resolve_scenario`). Threaded into `setup`, which reads the scenario text
/// from this arbitrary path instead of `GOLDEN_DIR` when set. Always `None` on
/// wasm (no CLI args there).
#[derive(Resource)]
struct ReplayPath(Option<PathBuf>);

/// The pure-Rust simulation. NO Bevy types inside (the rollback-ready shape).
#[derive(Resource)]
struct Sim(SimState);

/// Everything the tick→render loop needs besides the sim: the parsed scenario
/// (for recorded inputs + `shadow()` + `ticks` + reload), the two fixed-camera
/// viewports, the owned Scene ingredients, the CPU surface, and the loop cursor.
#[derive(Resource)]
struct Demo {
    scenario: Scenario,
    viewports: [Viewport; 2],
    scene: SceneData,
    surface: Bitmap,
    tick: u32,
    /// Per-tick `state_hash` column of the committed golden (index = tick) — the
    /// sim determinism witness, asserted on BOTH targets in debug builds.
    #[cfg(debug_assertions)]
    golden_state: Vec<u32>,
    /// Per-tick `frame_hash` column of the committed golden (index = tick) — the
    /// CPU render-parity witness. **Wasm-only:** the embedded demo (`blood`) has
    /// no `render_flash`/`render_shake` directives, so the demo's world-only draw
    /// (`screen_flash = 0`, no shake) reproduces the sidecar's frame hash exactly.
    /// Not asserted natively: the native demo does not replay a scenario's
    /// flash/shake, so a flashy scenario's frame would (correctly) diverge — and
    /// the native CPU frame is already gated far more thoroughly by the
    /// `render_slice3b_*` oracle-tests, making a native demo frame-hash redundant.
    #[cfg(all(target_arch = "wasm32", debug_assertions))]
    golden_frame: Vec<u64>,
}

/// Handle of the one `Image` the sprite samples; `tick_and_render` writes it.
#[derive(Resource)]
struct FrameImage(Handle<Image>);

fn main() {
    // Resolve + validate the scenario BEFORE opening a window: an unknown name
    // prints the available scenarios and exits non-zero (no window flash).
    // `--live` (native-only, T2) selects Mode::Live; wasm hard-codes Scripted.
    // `--record <path>` (4b, T1) names the recorder's on-exit flush target.
    // `--replay <path>` (4b, T2) selects Mode::Replay and names the arbitrary
    // scenario file `setup` reads instead of `GOLDEN_DIR`.
    let ParsedArgs {
        mode,
        name,
        record,
        replay,
    } = resolve_scenario();
    let title = format!("Liero-rs — 3c demo ({name})");

    App::new()
        .add_plugins(
            DefaultPlugins
                // Global nearest-neighbor sampling for crisp integer upscaling.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: WindowResolution::new(960, 600),
                        title,
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(ScenarioName(name))
        .insert_resource(mode)
        .insert_resource(RecordPath(record))
        .insert_resource(ReplayPath(replay))
        // C++ gfx.cpp kDelay = 14ms => one processFrame per ~71.43 Hz tick. The
        // number only sets perceived speed; determinism is by tick count, not
        // wall-clock. `Time<Fixed>` gives the fixed-timestep accumulator for free.
        .insert_resource(Time::<Fixed>::from_hz(1000.0 / 14.0))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, tick_and_render)
        .add_systems(Update, close_on_esc)
        // 4b: flush the recorder once, on graceful exit. Esc's `AppExit` is written
        // by `close_on_esc` in `Update`, so it is always observable here in `Last`.
        // Window-close (the X button) is different: winit's close request becomes
        // `AppExit` via bevy_window's `exit_on_all_closed`, which itself runs in
        // `Last`, inside the `ExitSystems` set (bevy_window-0.19.0 src/lib.rs:144,
        // src/system.rs:9/18-26). Without an explicit order, two `Last` systems can
        // run in either order, so this system could observe an empty `AppExit`
        // reader and silently drop the recording — hence `.after(ExitSystems)`.
        .add_systems(
            Last,
            flush_recorder_on_exit.after(bevy::window::ExitSystems),
        )
        .run();
}

/// Read the CLI args (`cargo run -p game -- [--live] [--record <path>]
/// [--replay <path>] [<name>]`) into `(Mode, name)` + the 4b record/replay
/// paths (spec §7, T2; 4b T1/T2) — `--live` selects `Mode::Live` (optionally
/// with `--record`), `--replay` plays an arbitrary scenario file, the
/// remaining optional positional is the scenario name (default `blood`) —
/// and, except for `--replay`, validate the name names a committed
/// `render_slice3b_<name>_scenario.txt` under `GOLDEN_DIR`. On an unknown name,
/// print the available scenarios and exit non-zero — done here, before the Bevy
/// app starts, so a typo never flashes a window. Every committed 3b scenario is
/// also a golden, so the debug self-check golden path is guaranteed to resolve
/// (Scripted mode only — Live does not load the golden column, see `setup`).
#[cfg(not(target_arch = "wasm32"))]
fn resolve_scenario() -> ParsedArgs {
    let parsed = match game::input::parse_args(std::env::args().skip(1), DEFAULT_SCENARIO) {
        Ok(parsed) => parsed,
        // A bare trailing `--record`/`--replay` with no path token (T1 review
        // fix; mirrored for `--replay` in T2): report and exit rather than
        // silently falling back to `None`, same pattern as the
        // "--record requires --live" check just below.
        Err(game::input::ParseArgsError::RecordMissingPath) => {
            eprintln!("--record requires a path");
            std::process::exit(2);
        }
        Err(game::input::ParseArgsError::ReplayMissingPath) => {
            eprintln!("--replay requires a path");
            std::process::exit(2);
        }
    };

    // `--replay <path>` (4b, T2) excludes `--live`/`--record` (spec §7/§10 Q2):
    // it is a distinct third mode (an arbitrary scenario file, loop/self-check
    // off) — not a live session and not something to record, so combining it
    // with either is a malformed CLI, checked before any window opens.
    if parsed.replay.is_some() && (parsed.mode == Mode::Live || parsed.record.is_some()) {
        eprintln!("--replay excludes --live/--record");
        std::process::exit(2);
    }
    if let Some(path) = parsed.replay {
        // `--replay` loads an arbitrary path, bypassing the golden-dir name
        // validation below (`available_scenarios`/`GOLDEN_DIR`) that the
        // positional `<name>` keeps for committed goldens (spec §5). `name`
        // is otherwise unused on this path (only the window title reads it).
        return ParsedArgs {
            mode: Mode::Replay,
            name: parsed.name,
            record: None,
            replay: Some(path),
        };
    }

    let available = available_scenarios();
    if !available.iter().any(|n| n == &parsed.name) {
        eprintln!("unknown scenario {:?}. available scenarios:", parsed.name);
        for n in &available {
            eprintln!("  {n}");
        }
        std::process::exit(2);
    }
    // `--record` is a Live-only value flag (spec §7): recording a Scripted run is
    // the vacuous case (§6), so reject it here — before any window opens.
    if parsed.record.is_some() && parsed.mode != Mode::Live {
        eprintln!("--record requires --live (recording is live-mode only)");
        std::process::exit(2);
    }
    parsed
}

/// Wasm has no CLI args and no filesystem to enumerate, so the scenario is the
/// **compile-time default** (`blood`) — its text + (debug) golden sidecar are
/// embedded via `include_str!` (see `load_scenario_text` / `golden_sidecar_text`),
/// and its level lives in the `scenario` crate's embedded TC manifest (Slice 3f
/// T2). A `?scenario=` query-param switch is a documented follow-up (spec §Q4).
/// `--live` is native-only (spec §8): wasm always resolves `Mode::Scripted`, so
/// the scripted witness path (incl. the debug self-check) is untouched by 4a.
#[cfg(target_arch = "wasm32")]
fn resolve_scenario() -> ParsedArgs {
    ParsedArgs {
        mode: Mode::Scripted,
        name: DEFAULT_SCENARIO.to_string(),
        record: None,
        replay: None,
    }
}

/// Enumerate the committed demo scenarios — the `<name>` of every
/// `render_slice3b_<name>_scenario.txt` in `GOLDEN_DIR`, sorted for a stable
/// help listing. Native-only: the wasm entry hard-codes the default scenario, so
/// this `read_dir` (no filesystem in the browser) is never compiled for wasm.
#[cfg(not(target_arch = "wasm32"))]
fn available_scenarios() -> Vec<String> {
    let mut names = Vec::new();
    let dir = std::fs::read_dir(GOLDEN_DIR).unwrap_or_else(|e| panic!("read {GOLDEN_DIR}: {e}"));
    for entry in dir {
        let file = entry.expect("dir entry").file_name();
        let file = file.to_string_lossy();
        if let Some(rest) = file.strip_prefix("render_slice3b_") {
            if let Some(name) = rest.strip_suffix("_scenario.txt") {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}

/// Startup: load the scenario, build the sim + render surface + the one Image,
/// spawn the camera and the ×3 sprite, and render tick 0 so the window shows the
/// first frame immediately (before the first `FixedUpdate`).
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    name: Res<ScenarioName>,
    mode: Res<Mode>,
    record_path: Res<RecordPath>,
    replay_path: Res<ReplayPath>,
) {
    let name = &name.0;
    // 1. Read + parse the scenario text. `Mode::Replay` (4b, T2) reads an
    //    ARBITRARY path (not `GOLDEN_DIR`) — the whole point of `--replay` is to
    //    play back a file that need not be a committed golden (spec §5).
    //    Otherwise the byte source forks by target (native: `std::fs`; wasm:
    //    `include_str!` of the compile-time default) — see `load_scenario_text`.
    let scenario_text = if *mode == Mode::Replay {
        let path = replay_path
            .0
            .as_ref()
            .expect("Mode::Replay implies a --replay path (resolve_scenario invariant)");
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    } else {
        load_scenario_text(name)
    };
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");

    // 2. Tick-0 load (moves `state` into `Sim`; viewports + scene into `Demo`).
    let loaded = scenario::load(Path::new(TC_ROOT), &scenario);
    let scenario::Loaded {
        state,
        viewports,
        scene,
        // `font`/`labels` (Slice 3e T0) are wired into the render path in T5; the
        // interactive `game` binary does not draw the HUD yet.
        ..
    } = loaded;

    // 3. Owned CPU surface the `render` crate paints into.
    let surface = Bitmap::new(SURFACE_W as i32, SURFACE_H as i32);

    // 4. The one Image the sprite samples. `Rgba8UnormSrgb`: the VGA palette RGB
    //    is treated as sRGB (see the texture-format note in the done-report). The
    //    CPU frame hash is the gate; the format is advisory.
    let mut image = Image::new_fill(
        Extent3d {
            width: SURFACE_W,
            height: SURFACE_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);

    // 5. Camera + one sprite scaled ×3 into the 960×600 window.
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite::from_image(handle.clone()),
        Transform::from_scale(Vec3::splat(3.0)),
    ));

    // 6. Assemble the resources. Live and Replay have no golden to self-check
    //    against and do not load the golden column at all (spec §7/T2; Replay's
    //    arbitrary file has no committed sidecar either); Scripted loads it
    //    exactly as 3c did.
    #[cfg(debug_assertions)]
    let golden = if *mode == Mode::Scripted {
        load_golden_hashes(name)
    } else {
        (Vec::new(), Vec::new())
    };

    let mut demo = Demo {
        scenario,
        viewports,
        scene,
        surface,
        tick: 0,
        #[cfg(debug_assertions)]
        golden_state: golden.0,
        #[cfg(all(target_arch = "wasm32", debug_assertions))]
        golden_frame: golden.1,
    };
    let sim = Sim(state);

    // 7. Render tick 0 into the surface and upload once (window shows frame 0).
    render_and_upload(&mut demo, &sim.0, &mut images, &handle);

    // The input source (spec §7/T2; `Replay` 4b T2): Scripted feeds the
    // scenario's recorded inputs as a literal pass-through (behavior unchanged
    // from 3c); `--live` swaps this for the keyboard via the default bindings.
    // `--replay` is `InputSource::Scripted` verbatim (spec §5, no new source
    // variant) over the scenario parsed from the arbitrary `--replay` path
    // above — not a committed golden. All three still load their initial state
    // (level + worms) through the same `scenario::load` above — only the input
    // source differs.
    let source = match *mode {
        Mode::Scripted | Mode::Replay => InputSource::Scripted(demo.scenario.clone()),
        Mode::Live => InputSource::Live(game::input::default_bindings()),
    };
    commands.insert_resource(source);

    // 4b recorder (spec §4.2): Live mode only, and only when `--record` names a
    // path. The recorder carries the base scenario's tick-0 metadata; the flush
    // system writes it on exit. Scripted inserts no recorder, so that path — and
    // its 4a pass-through gate — stays byte-unchanged.
    if *mode == Mode::Live {
        if let Some(path) = &record_path.0 {
            commands.insert_resource(Recorder::new(demo.scenario.clone(), path.clone()));
        }
    }

    commands.insert_resource(sim);
    commands.insert_resource(demo);
    commands.insert_resource(FrameImage(handle));
}

/// FixedUpdate: advance the sim EXACTLY one tick, run the loop step, render the
/// current tick, upload, then (debug) assert the sim hash against the golden.
/// This is the ONLY system that mutates `Sim` (the determinism firewall).
fn tick_and_render(
    mut sim: ResMut<Sim>,
    mut demo: ResMut<Demo>,
    mut images: ResMut<Assets<Image>>,
    frame: Res<FrameImage>,
    source: Res<InputSource>,
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<Mode>,
    // 4b: present only on the Live + `--record` path; `None` (no-op) otherwise.
    mut recorder: Option<ResMut<Recorder>>,
) {
    // 4b (T2): `--replay` has no loop/reload (spec §5/§9) — once `tick` reaches
    // `ticks` the replay HOLDS on the final rendered frame: no further
    // `process_frame`, no further tick advance. Scripted/Live are unaffected
    // (Scripted loops at its boundary below; Live has no bound at all).
    let replay_finished = *mode == Mode::Replay && demo.tick >= demo.scenario.ticks;

    if !replay_finished {
        // 1. Sample the input source EXACTLY ONCE per tick, at the top of the
        //    single FixedUpdate system, before `process_frame` (spec §4.3) — the
        //    central one-snapshot-per-tick determinism invariant, decoupled from
        //    render rate. `Scripted` (the default) is a literal pass-through of
        //    the scenario's RECORDED inputs, so scripted behavior is
        //    byte-unchanged from the 3c inline feed. (The blood golden was
        //    driven with these inputs — the DART-into-own-feet fire is
        //    `input 8 16 0` — so empty inputs would diverge and trip the
        //    self-check.) `Live` (--live, T2) instead polls the held-key set;
        //    `Replay` (--replay, 4b T2) is `Scripted` over the replay file.
        let inputs = source.sample(demo.tick, &keys);
        // 4b recorder seam (spec §4.1): tap the SAMPLED array here — after
        // `sample` (so the Dig→Left+Right chord is already resolved into the
        // words the sim sees) and before `process_frame`. Present only in
        // Live + `--record`, so Scripted/Replay are untouched.
        if let Some(recorder) = recorder.as_mut() {
            recorder.record(&inputs);
        }
        sim.0.process_frame(&inputs);
    }

    // 2. Loop step. Scripted-only reload (spec §4.3 step 4 / §9): when `tick`
    //    passes `ticks`, rebuild from the loader at tick 0 for a bit-identical
    //    loop (fixed seed + fixed inputs). `Replay` advances toward its own
    //    `ticks` bound with NO reload (guard/loop off, spec §5) and stops
    //    advancing once `replay_finished` (holds the final frame). `Live` has
    //    no golden `ticks` boundary and runs indefinitely, so `demo.tick` is
    //    left untouched — it is unused by `InputSource::Live::sample` and by
    //    anything else on the live path.
    match *mode {
        Mode::Scripted => {
            let (next, reload) = blit::next_tick(demo.tick, demo.scenario.ticks);
            demo.tick = next;
            if reload {
                let loaded = scenario::load(Path::new(TC_ROOT), &demo.scenario);
                sim.0 = loaded.state;
                demo.viewports = loaded.viewports;
                demo.scene = loaded.scene;
            }
        }
        Mode::Replay if !replay_finished => demo.tick += 1,
        Mode::Replay | Mode::Live => {}
    }

    // 3 + 4. Render the current tick into the CPU surface, then upload.
    render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);

    // 5. Debug-only determinism self-check, Scripted-only (spec §4.3 step 6 /
    //    §9): the default scenario is a golden, but Live has no golden loaded
    //    (see `setup`) and no `demo.tick` progression to index it with — retired
    //    for live, retained unchanged for scripted so the regression path is
    //    never silently disabled. After the tick+wrap, `demo.tick` names the
    //    tick whose state `sim` now holds: a normal tick `k` matches `golden[k]`;
    //    the wrap tick resets to 0 with a fresh tick-0 sim, matching `golden[0]`.
    #[cfg(debug_assertions)]
    if *mode == Mode::Scripted {
        debug_assert_eq!(
            sim::hash::hash_game_state(&sim.0),
            demo.golden_state[demo.tick as usize],
            "sim-hash diverged from the committed golden at tick {}",
            demo.tick
        );
    }

    // 6. Wasm render-parity witness (debug-only): the CPU frame just rendered into
    //    `demo.surface` must hash to the embedded golden's `frame_hash` column —
    //    the proof that the *render* (not just the sim) is bit-identical to native
    //    in the browser (spec §Q3). The fade rule matches the 3b harness exactly:
    //    tick 0 is the black open (`fade = 0`), every later tick is identity
    //    (`fade = 33`). Wasm-only because the embedded `blood` demo has no
    //    flash/shake (so the demo's world-only draw reproduces the sidecar), and
    //    the native CPU frame is already gated by the `render_slice3b_*` oracle.
    #[cfg(all(target_arch = "wasm32", debug_assertions))]
    {
        let fade = if demo.tick == 0 { 0 } else { 33 };
        let frame_hash = render::hash::hash_frame(&demo.surface, fade);
        debug_assert_eq!(
            frame_hash, demo.golden_frame[demo.tick as usize],
            "CPU frame-hash diverged from the committed golden at tick {}",
            demo.tick
        );
    }
}

/// Render the current sim state into `demo.surface` and blit it (ARGB→RGBA) into
/// the sampled Image. Taking `&mut Demo` lets the borrow checker split the
/// disjoint field borrows (`&demo.scene` for the Scene vs `&mut demo.surface` /
/// `&mut demo.viewports` for the draw) — the reborrow the T8 `render_tick` uses.
fn render_and_upload(
    demo: &mut Demo,
    sim: &SimState,
    images: &mut Assets<Image>,
    handle: &Handle<Image>,
) {
    let draw_shadow = demo.scenario.shadow();
    let scene = demo.scene.as_scene(0, draw_shadow);
    render::frame::draw(&mut demo.surface, sim, &mut demo.viewports, &scene);

    // `get_mut` marks the Image dirty => Bevy re-uploads it to the GPU.
    let mut image = images.get_mut(handle).expect("frame image exists");
    blit::blit_surface_into_bytes(&demo.surface, image.data.as_mut().expect("image has data"));
}

/// The scenario text for `name`. **Native:** read the committed
/// `render_slice3b_<name>_scenario.txt` from `GOLDEN_DIR` (`std::fs`, unchanged).
#[cfg(not(target_arch = "wasm32"))]
fn load_scenario_text(name: &str) -> String {
    let scenario_path = format!("{GOLDEN_DIR}/render_slice3b_{name}_scenario.txt");
    std::fs::read_to_string(&scenario_path).unwrap_or_else(|e| panic!("read {scenario_path}: {e}"))
}

/// The scenario text for the compile-time default. **Wasm:** there is no
/// filesystem, so the default (`blood`) scenario text is embedded at compile time
/// via `include_str!`. `name` is always `DEFAULT_SCENARIO` on wasm (see
/// `resolve_scenario`); only that one scenario is embedded.
#[cfg(target_arch = "wasm32")]
fn load_scenario_text(_name: &str) -> String {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../oracle-tests/golden/render_slice3b_blood_scenario.txt"
    ))
    .to_string()
}

/// Parse the committed frame sidecar into its two per-tick hash columns —
/// `(state_hash: Vec<u32>, frame_hash: Vec<u64>)`, index = tick. The grammar
/// mirrors `render_slice3b_common::parse_frames` (`<tick> <frame_hash_hex16>
/// <state_hash_hex8>`; skip blank / `#` / `total` lines). Native uses only the
/// `state_hash` column (the frame column is the wasm render-parity witness), but
/// both are parsed here so the seam has one source of truth.
#[cfg(debug_assertions)]
fn load_golden_hashes(name: &str) -> (Vec<u32>, Vec<u64>) {
    let text = golden_sidecar_text(name);
    let mut states = Vec::new();
    let mut frames = Vec::new();
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        if it.next() == Some("total") {
            continue;
        }
        let frame_hash = it.next().expect("frame_hash column");
        let state_hash = it.next().expect("state_hash column");
        frames.push(u64::from_str_radix(frame_hash, 16).expect("frame_hash hex"));
        states.push(u32::from_str_radix(state_hash, 16).expect("state_hash hex"));
    }
    (states, frames)
}

/// The committed frame sidecar text for `name`. **Native:** read
/// `render_slice3b_<name>.txt` from `GOLDEN_DIR` (`std::fs`, unchanged).
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
fn golden_sidecar_text(name: &str) -> String {
    let path = format!("{GOLDEN_DIR}/render_slice3b_{name}.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The committed frame sidecar text for the compile-time default. **Wasm+debug:**
/// embedded via `include_str!` so the debug wasm build can run the determinism +
/// frame-hash witness with no filesystem. Only compiled into a *debug* wasm build
/// (`cfg(all(target_arch = "wasm32", debug_assertions))`), so release wasm carries
/// zero sidecar payload (spec §Q3).
#[cfg(all(target_arch = "wasm32", debug_assertions))]
fn golden_sidecar_text(_name: &str) -> String {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../oracle-tests/golden/render_slice3b_blood.txt"
    ))
    .to_string()
}

/// Esc quits. The window's close button already exits via winit.
///
/// Bevy 0.19 renamed the buffered-event API to "messages": the exit signal is
/// sent through a `MessageWriter<AppExit>` (`EventWriter` no longer exists;
/// `AppExit` derives `Message`, and `MessageWriter::write` is the send call).
fn close_on_esc(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

/// Flush the recorded input stream to the `--record` path once, on graceful exit
/// (Esc / window close → `AppExit`). Reads the `AppExit` message in `Last`, ordered
/// `.after(bevy::window::ExitSystems)` (see the `add_systems` call in `main`):
/// Esc's exit is written earlier, by `close_on_esc` in `Update`, so it is always
/// visible by the time `Last` runs; the X-button's exit is written by bevy_window's
/// `exit_on_all_closed`, which itself runs in `Last` — only the explicit ordering
/// guarantees this system observes it in the same frame instead of racing it.
/// A `Local<bool>` guards against a second write if the exit lingers across frames.
///
/// The `Recorder` is inserted only on the native `--live --record` path (see
/// `setup`), so `recorder` is `None` — and this system inert — for every other
/// run, including wasm. Building the scenario and writing `to_text` reuses the
/// serializer proven by the T0 round-trip property test.
///
/// **Caveat (spec §4.2/§9):** the stream is buffered in memory and written only
/// here, so a hard crash or a signal kill (SIGTERM/SIGALRM — e.g. a `timeout` or
/// `alarm` smoke test) never reaches this system and writes **no** file. That is
/// acceptable for a dev/test artifact; the objective record→replay proof is the
/// headless round-trip gate (T3), which never touches this exit path.
fn flush_recorder_on_exit(
    mut exits: MessageReader<AppExit>,
    recorder: Option<Res<Recorder>>,
    mut done: Local<bool>,
) {
    if *done || exits.is_empty() {
        return;
    }
    // React exactly once: consume the exit message(s) and latch `done`.
    exits.clear();
    *done = true;
    let Some(recorder) = recorder else {
        return; // no recording in flight (Scripted, or --record absent).
    };
    let scenario = recorder.build();
    let path = recorder.path();
    match std::fs::write(path, scenario.to_text()) {
        Ok(()) => eprintln!(
            "recording written to {} ({} ticks)",
            path.display(),
            scenario.ticks
        ),
        Err(e) => eprintln!("failed to write recording to {}: {e}", path.display()),
    }
}
