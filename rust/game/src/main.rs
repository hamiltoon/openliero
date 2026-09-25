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
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::WindowResolution;

use render::bitmap::Bitmap;
use render::viewport::Viewport;
use scenario::{Scenario, SceneData};
use sim::sound::LoopKey;
use sim::state::SimState;

use game::audio::{AudioSink, Drainer, NullSink, RodioSink};
use game::input::{InputSource, Mode, ParsedArgs, Recorder};
use game::match_flow::{FlowStep, MatchFlow};
use game::web_params::MatchParams;

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

/// The PR-preview URL parameters (`web_params`). Parsed from the page URL on wasm;
/// always the default (no overrides) natively, where the CLI picks the scenario.
#[derive(Resource)]
struct Preview(MatchParams);

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
    /// Step 4½a-1: the match lifecycle (`LocalController`'s game → game-ended tail) —
    /// `Some` in `Mode::Live` only. Scripted loops its golden and Replay plays a fixed
    /// `ticks`, so neither ends a match.
    flow: Option<MatchFlow>,
    /// PR-preview loadout (`?weapons=`, `web_params`): re-applied to the tick-0 state
    /// on every (re)start of the live match. Empty natively and without the parameter.
    loadout: Vec<String>,
    /// Per-tick `state_hash` column of the committed golden (index = tick) — the
    /// sim determinism witness, asserted on BOTH targets in debug builds.
    #[cfg(debug_assertions)]
    golden_state: Vec<u32>,
    /// Per-tick `frame_hash` column of the committed golden (index = tick) — the
    /// CPU render-parity witness. **Wasm-only:** the embedded demo (`blood`) has
    /// no `render_flash`/`render_shake` directives, and since 4d's live stepping
    /// its tick-10 explosion DOES fire the shake path (amount 1) — frame parity
    /// survives because the `rand(2)-1 <= 0` jitter is eaten by the origin-pinned
    /// camera clamp, netting zero pixels (NOT because "no shake runs"). A demo
    /// change to `shake >= 2` or a non-origin camera would break this witness.
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

/// The audio backend, native AND wasm (Slice 4c, T4 native / T5 wasm): either
/// a real device (`RodioSink` — since T5, the SAME struct on both targets,
/// `audio.rs`) or the C++ `NullSoundPlayer` analog when `RodioSink::try_new`
/// finds no output device (`setup_audio`'s fallback — audio §4c GREEN bullet:
/// no crash without a sound card; on wasm, no `AudioContext`, e.g. a very old
/// browser). `Drainer<Sink>` needs one concrete sink type per build; an enum
/// (rather than `Box<dyn AudioSink>`) keeps every call statically dispatched.
enum NativeAudio {
    Rodio(RodioSink),
    Null(NullSink),
}

impl AudioSink for NativeAudio {
    fn play_one_shot(&mut self, sound: i32) {
        match self {
            NativeAudio::Rodio(s) => s.play_one_shot(sound),
            NativeAudio::Null(s) => s.play_one_shot(sound),
        }
    }
    fn play_loop(&mut self, key: LoopKey, sound: i32) {
        match self {
            NativeAudio::Rodio(s) => s.play_loop(key, sound),
            NativeAudio::Null(s) => s.play_loop(key, sound),
        }
    }
    fn stop_loop(&mut self, key: LoopKey) {
        match self {
            NativeAudio::Rodio(s) => s.stop_loop(key),
            NativeAudio::Null(s) => s.stop_loop(key),
        }
    }
}

/// The `AudioDrainer`'s sink type: `NativeAudio` (Rodio-or-Null) on BOTH
/// targets since T5 — `RodioSink` compiles for wasm32 too (`audio.rs`'s
/// unconditional `impl`; `game/Cargo.toml`'s wasm target table adds `rodio`
/// with the `wasm-bindgen` feature, which is what selects `cpal`'s WebAudio
/// host for `wasm32`). `RodioSink::try_new` failing (no output device / no
/// `AudioContext`) is the one remaining fallback to `NullSink`, on either
/// target — see `open_native_audio`.
type Sink = NativeAudio;

/// Drains `sim.0.sound_events` into the sink each real tick and runs the
/// liveness reaper (`tick_and_render`, spec §4.2). `NonSend` (not an ordinary
/// `Resource`) because the backend holds a platform audio handle that is
/// **not** `Send`: natively, a `cpal::Stream` inside `RodioSink`'s
/// `OutputStream` (confirmed against `cpal` 0.15's CoreAudio backend:
/// `StreamInner` holds a raw `AudioUnit` handle with no `unsafe impl Send`);
/// on wasm, the `AudioContext`/`web_sys` types inside cpal's WebAudio host are
/// built on `JsValue`, which is `!Send` by construction (wasm32 is
/// single-threaded, so this costs nothing either way). `NonSend`/`NonSendMut`
/// pin the resource to the main thread instead of requiring `Send`, unlike
/// `Resource`/`Res`/`ResMut`.
struct AudioDrainer(Drainer<Sink>);

fn main() {
    // Resolve + validate the scenario BEFORE opening a window: an unknown name
    // prints the available scenarios and exits non-zero (no window flash).
    // `--live` (native-only, T2) selects Mode::Live; wasm hard-codes Scripted.
    // `--record <path>` (4b, T1) names the recorder's on-exit flush target.
    // `--replay <path>` (4b, T2) selects Mode::Replay and names the arbitrary
    // scenario file `setup` reads instead of `GOLDEN_DIR`.
    let preview = preview_params();
    let ParsedArgs {
        mode,
        name,
        record,
        replay,
    } = resolve_scenario(&preview);
    // Title: drop the stale "3c demo" string (flagged since 4a T2). The bare 4f
    // default match reads as "default match" (its sentinel name); every other
    // run shows its scenario name.
    let title = if name == game::input::DEFAULT_MATCH {
        "Liero-rs — default match".to_string()
    } else {
        format!("Liero-rs — {name}")
    };

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
        .insert_resource(Preview(preview))
        // C++ gfx.cpp kDelay = 14ms => one processFrame per ~71.43 Hz tick. The
        // number only sets perceived speed; determinism is by tick count, not
        // wall-clock. `Time<Fixed>` gives the fixed-timestep accumulator for free.
        .insert_resource(Time::<Fixed>::from_hz(1000.0 / 14.0))
        .add_systems(Startup, setup)
        // Exclusive (main-thread-only) Startup system: builds the audio
        // backend and inserts it as a NonSend resource (see `AudioDrainer`).
        // Independent of `setup`'s Commands-based resource inserts, so
        // ordering between the two Startup systems is unconstrained — both
        // complete before the first FixedUpdate regardless.
        .add_systems(Startup, setup_audio)
        .add_systems(FixedUpdate, tick_and_render)
        // Esc quits natively; in a browser tab it would only leave a dead canvas.
        .add_systems(
            Update,
            close_on_esc.run_if(|| !cfg!(target_arch = "wasm32")),
        )
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
fn resolve_scenario(_preview: &MatchParams) -> ParsedArgs {
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

    // 4f: the default match (a bare invocation, or an explicit `--live
    // default_match`) bypasses the golden-dir name validation below — its
    // scenario is the committed `scenarios/default_match.txt` fixture, NOT a
    // `render_slice3b_*` golden (spec §1.2). `parse_args` already set
    // `Mode::Live` for the bare case; `setup` sources the fixture by seeing this
    // sentinel name. No `--record` check needed: the bare path carries none.
    //
    // T0 fix (4f review): gated on `Mode::Live`, not on the name alone. A
    // POSITIONAL `default_match` (e.g. `cargo run -p game -- default_match`,
    // no `--live`) parses to `Mode::Scripted` with this same sentinel name —
    // without the mode check it slipped through this bypass too, then panicked
    // in `setup`'s debug self-check (`load_golden_hashes`), since the sentinel
    // has no committed `render_slice3b_default_match` golden to load (only
    // `--live`/bare skip the golden column, see `setup`). Gating on
    // `Mode::Live` sends that case into `available_scenarios` below instead,
    // where it is correctly rejected (unknown scenario, exit 2) — the doc
    // comment on `DEFAULT_MATCH` already promised this outcome.
    if parsed.name == game::input::DEFAULT_MATCH && parsed.mode == Mode::Live {
        return parsed;
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

/// Wasm has no CLI args and no filesystem to enumerate. Since the PR-preview build
/// the page URL decides (`web_params`): by default the browser plays the LIVE
/// default match on the keyboard (two players, the native `--live` bindings), with
/// `?weapons=` / `?level=` / `?seed=` applied; `?demo` keeps the pre-preview
/// behavior — the scripted **compile-time default** (`blood`), whose text + (debug)
/// golden sidecar are embedded via `include_str!` (see `load_scenario_text` /
/// `golden_sidecar_text`), so the scripted witness path (incl. the debug
/// self-check) is unchanged.
#[cfg(target_arch = "wasm32")]
fn resolve_scenario(preview: &MatchParams) -> ParsedArgs {
    if preview.demo {
        return ParsedArgs {
            mode: Mode::Scripted,
            name: DEFAULT_SCENARIO.to_string(),
            record: None,
            replay: None,
        };
    }
    ParsedArgs {
        mode: Mode::Live,
        name: game::input::DEFAULT_MATCH.to_string(),
        record: None,
        replay: None,
    }
}

/// The PR-preview parameters: the page URL's query string on wasm (a missing
/// `window`/`location` degrades to the default match).
#[cfg(target_arch = "wasm32")]
fn preview_params() -> MatchParams {
    let query = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let params = MatchParams::parse(&query);
    for w in &params.warnings {
        preview_warn(w);
    }
    params
}

/// Natively the CLI picks the scenario; the preview parameters are always empty.
#[cfg(not(target_arch = "wasm32"))]
fn preview_params() -> MatchParams {
    MatchParams::default()
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
    preview: Res<Preview>,
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
    } else if name == game::input::DEFAULT_MATCH {
        // 4f: the bare-invocation default match — the committed default-match
        // fixture (Mode::Live), sourced here instead of a `render_slice3b_*`
        // golden. See `default_match_text`. The PR-preview build derives it from
        // the URL instead (`web_params`; with no parameters it is the same match).
        if cfg!(target_arch = "wasm32") {
            preview.0.scenario_text()
        } else {
            default_match_text()
        }
    } else {
        load_scenario_text(name)
    };
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");

    // 2. Tick-0 load (moves `state` into `Sim`; viewports + scene into `Demo`).
    let loaded = scenario::load(Path::new(TC_ROOT), &scenario);
    let scenario::Loaded {
        mut state,
        viewports,
        scene,
        // `font`/`labels` (Slice 3e T0) are wired into the render path in T5; the
        // interactive `game` binary does not draw the HUD yet.
        ..
    } = loaded;

    // PR preview: the `?weapons=` loadout (Live only; empty natively).
    let loadout = if *mode == Mode::Live {
        preview.0.weapons.clone()
    } else {
        Vec::new()
    };
    apply_loadout(&mut state, &loadout);

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
        flow: (*mode == Mode::Live).then(MatchFlow::new),
        loadout,
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

/// Startup (T4, exclusive — see `AudioDrainer`): build the native audio
/// backend and insert it as a `NonSend` resource. Split out of `setup`
/// because `Commands`-queued insertion goes through `bevy_ecs`'s `Command`
/// trait, which requires `Send + 'static`, and `RodioSink` is not `Send`;
/// `World::insert_non_send` carries no such bound, and an exclusive
/// system (`fn(&mut World)`) is guaranteed to run on the main thread, so it is
/// a safe place to open the audio device. Runs unconditionally (Live,
/// Scripted, and Replay are all windowed — `main.rs` never runs headless;
/// the headless callers — `replay_state_series`, the round-trip/passthrough
/// tests, `shot` — never construct this App at all, spec §6.4).
fn setup_audio(world: &mut World) {
    let sink = build_native_sink(Path::new(TC_ROOT));
    world.insert_non_send(AudioDrainer(Drainer::new(sink)));
}

/// Load the TC's sample table and open the default output device (design
/// §5). `Err` (no output device — e.g. a headless CI box or a machine with no
/// sound card) falls back to `NullSink` and reports why on stderr instead of
/// crashing (T4 GREEN bullet).
#[cfg(not(target_arch = "wasm32"))]
fn build_native_sink(tc_root: &Path) -> NativeAudio {
    let tc_cfg_path = tc_root.join("tc.cfg");
    let tc_bytes = std::fs::read(&tc_cfg_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", tc_cfg_path.display()));
    let tc = assets::tc::TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let table = game::audio::load_sound_table(tc_root, &tc.types.sounds);
    open_native_audio(table)
}

/// Wasm (T5): the same `RodioSink` (`audio.rs`), but the sample table is
/// loaded through the `read_asset` embed seam (`load_sound_table_wasm`)
/// instead of the filesystem the browser doesn't have — mirroring how
/// `setup` already loads sprites/level/tc.cfg for wasm (`scenario::load`).
/// `RodioSink::try_new` opens a Web Audio `AudioContext` synchronously (no
/// `wasm-bindgen-futures` needed — verified against cpal 0.15.3's
/// `webaudio/mod.rs`), but the context starts `suspended` per the browser's
/// autoplay policy until a user gesture calls `resume()` (which
/// `rodio::Sink::play`/`append` triggers via `cpal::Stream::play`) — so a
/// fresh page plays **silently** until the first click/keypress, with no
/// error and no console spam (T5 task requirement). `open_native_audio`'s
/// `Err` fallback additionally covers a browser with no Web Audio support at
/// all.
#[cfg(target_arch = "wasm32")]
fn build_native_sink(tc_root: &Path) -> NativeAudio {
    let tc_bytes = scenario::assets::read_asset(tc_root, "tc.cfg");
    let tc = assets::tc::TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let table = game::audio::load_sound_table_wasm(tc_root, &tc.types.sounds);
    open_native_audio(table)
}

/// Shared `RodioSink`-or-`NullSink` fallback (T4/T5): `Err` from
/// `RodioSink::try_new` (no output device / no `AudioContext`) degrades to
/// silent play rather than a crash, on either target.
fn open_native_audio(table: game::audio::SoundTable) -> NativeAudio {
    match RodioSink::try_new(table) {
        Ok(sink) => NativeAudio::Rodio(sink),
        Err(e) => {
            eprintln!("audio: no output device available ({e}); running with sound disabled");
            NativeAudio::Null(NullSink)
        }
    }
}

/// This tick's live loop-channel keys, read from `sim` right after
/// `process_frame` — the belt-and-braces reaper's input (design §4.2). A
/// `Worm(i)` key is live iff worm `i` is currently visible; a
/// `WormWeapon(i, slot)` key is live iff the worm is ALSO visible and `slot`
/// is its current weapon slot. Gating the weapon key on visibility too (not
/// just the slot match the design text spells out) closes the leak the
/// design's risk item 1 names directly ("a worm dies... the loop leaks"): a
/// dead worm's `current_weapon` field is untouched by death, so a
/// slot-only check would never reap a weapon loop whose explicit death-site
/// `Stop` (`sim/src/state.rs`) was somehow missed. This is a strict superset
/// of the design's literal "current_weapon != w" example — it still reaps a
/// live worm's stale weapon-switch key exactly the same way, via the current
/// slot simply not matching (so the stale key is absent from this set).
fn live_loop_keys(sim: &SimState) -> HashSet<LoopKey> {
    let mut keys = HashSet::new();
    for (i, w) in sim.worms.iter().enumerate() {
        if !w.visible {
            continue;
        }
        let idx = i as u8;
        keys.insert(LoopKey::Worm(idx));
        keys.insert(LoopKey::WormWeapon(idx, w.current_weapon as u8));
    }
    keys
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
    // T4: the audio backend (see `AudioDrainer`) — NonSend because native
    // `RodioSink` is not `Send`.
    mut audio: NonSendMut<AudioDrainer>,
) {
    // 4f: F5 restarts a Live/Replay match — rebuild tick 0 through the SAME
    // `scenario::load` reload the `Mode::Scripted` loop arm uses (reset `sim.0`,
    // `demo.viewports`, `demo.scene`, `demo.tick`), then render the fresh frame
    // and skip this tick's advance so the restart shows tick 0. `F5` is
    // verified-unbound (the default bindings use R/F/D/G + arrows + modifiers —
    // `input::default_bindings`; `R` is P0 fire, so it is NOT reused). Scripted
    // is untouched: it has its own bit-identical loop reload at the `ticks`
    // boundary below.
    //
    // T0 fix (4f review): F5 also restarts the RECORDING, not just the sim.
    // `Recorder` is only present on the Live + `--record` path (`setup`); when
    // it is, `clear()` drops every snapshot buffered before this restart so
    // the eventual flush covers only ticks since the latest F5 — replaying it
    // reproduces the session from that restart point, not the original
    // launch. Without this the buffer kept appending across the restart
    // boundary and a replay of the flushed file silently diverged from the
    // live session at the restart tick (see `Recorder::clear`'s doc for the
    // full semantics).
    if (*mode == Mode::Live || *mode == Mode::Replay) && keys.just_pressed(KeyCode::F5) {
        restart_match(&mut sim.0, &mut demo, recorder.as_deref_mut());
        render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);
        return;
    }

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
        // 4d T2: the live viewport stepping owns the C++ `Game::ProcessFrame`
        // phase order around the sim's atomic `process_frame` — top-of-frame
        // shake decrement + banner walk BEFORE, explosion-shake event-max AFTER
        // (`viewport_step::tick_viewports`, design §3). The per-viewport
        // `shake`/`banner_y` live on `demo.viewports`; `sim.0.screen_flash`
        // (decremented inside `process_frame`, raised at sobject-create) is read
        // by the render below. Scripted/Live/Replay all drive it — the shake
        // events only fire on real explosions, so a no-explosion scenario steps
        // nothing (byte-identical to the pre-4d `process_frame` call).
        game::viewport_step::tick_viewports(&mut demo.viewports, &mut sim.0, &inputs);

        // T4 (spec §4.2): drain this tick's sound-event stream into the sink,
        // then run the liveness reaper. Both are gated behind the SAME
        // `!replay_finished` real-tick condition as `process_frame` itself —
        // a held replay's final frame must not re-fire one-shots (or
        // spuriously reap live loops) every FixedUpdate while holding.
        audio.0.drain(&sim.0.sound_events);
        let live = live_loop_keys(&sim.0);
        audio.0.reap(&live);

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
    //    (`fade = 33`). Since 4d, blood's tick-10 explosion fires the live shake
    //    path — parity survives via the origin-clamped `<= 0` jitter (see the
    //    `golden_frame` doc); the native CPU frame is already gated by the
    //    `render_slice3b_*` oracle.
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
    // 4d T2: thread the LIVE `screen_flash` (drop the 3c hardcoded `0`) — a real
    // explosion raises it in `process_frame`, decrementing one per tick, driving
    // the palette `LightUp` blip at draw. For a no-flash scenario it stays 0, so
    // the frame is byte-identical to the pre-4d hardcoded `0`.
    let scene = demo.scene.as_scene(sim.screen_flash, draw_shadow);
    render::frame::draw(&mut demo.surface, sim, &mut demo.viewports, &scene);

    // `get_mut` marks the Image dirty => Bevy re-uploads it to the GPU.
    let mut image = images.get_mut(handle).expect("frame image exists");
    blit::blit_surface_into_bytes(&demo.surface, image.data.as_mut().expect("image has data"));
}

/// Rebuild tick 0 through `scenario::load` — the 4f F5 restart, shared since 4½a-1 with
/// the live match-end restart. Restarts an in-flight recording too (the 4f T0 fix: the
/// flushed file then covers only ticks since the restart) and, in Live, the `MatchFlow`.
fn restart_match(sim: &mut SimState, demo: &mut Demo, recorder: Option<&mut Recorder>) {
    if let Some(recorder) = recorder {
        recorder.clear();
    }
    let loaded = scenario::load(Path::new(TC_ROOT), &demo.scenario);
    *sim = loaded.state;
    apply_loadout(sim, &demo.loadout);
    demo.viewports = loaded.viewports;
    demo.scene = loaded.scene;
    demo.tick = 0;
    if demo.flow.is_some() {
        demo.flow = Some(MatchFlow::new());
    }
}

/// The scenario text for `name`. **Native:** read the committed
/// `render_slice3b_<name>_scenario.txt` from `GOLDEN_DIR` (`std::fs`, unchanged).
/// Apply a PR-preview loadout to a freshly loaded tick-0 state (no-op when empty);
/// names that match no weapon are reported and leave their slot unchanged.
fn apply_loadout(state: &mut SimState, loadout: &[String]) {
    let unknown = game::web_params::apply_weapons(state, loadout);
    if !unknown.is_empty() {
        preview_warn(&format!(
            "unknown weapon name(s) {unknown:?}; those slots keep the default"
        ));
    }
}

/// Report an ignored preview parameter: the browser console on wasm (where
/// `eprintln!` goes nowhere), stderr natively.
fn preview_warn(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::warn_1(&format!("openliero preview: {msg}").into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("openliero preview: {msg}");
}

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

/// The committed default-match scenario text (Slice 4f) — bare `cargo run -p
/// game` loads this in `Mode::Live`. Embedded via `include_str!` so the single
/// `setup` call site compiles on BOTH targets; on wasm it is never reached
/// (that arm hard-codes `Mode::Scripted`/`blood` and never yields the
/// `DEFAULT_MATCH` sentinel), so embedding the tiny fixture is harmless. Unlike
/// `load_scenario_text`, this fixture lives OUTSIDE `oracle-tests/golden/`
/// (`game/scenarios/`), so it never enters `available_scenarios` or any golden
/// enumeration and carries no frame/state sidecar (Live loads no golden column).
fn default_match_text() -> String {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scenarios/default_match.txt"
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
