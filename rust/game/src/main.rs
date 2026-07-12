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
use std::path::Path;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::WindowResolution;

use render::bitmap::Bitmap;
use render::viewport::Viewport;
use scenario::{Scenario, SceneData};
use sim::state::{ControlState, SimState};

mod blit;

/// TC asset root, resolved at compile time relative to this crate so `cargo run
/// -p game` works from any CWD (constraint: CARGO_MANIFEST_DIR, not CWD).
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// Committed golden dir — the scenario text and (debug) the self-check column.
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");
/// The hard-coded demo scenario for T3 (the `--<name>` arg is T4).
const SCENARIO_NAME: &str = "blood";
/// Native low-res canvas the `render` crate paints (one Liero screen).
const SURFACE_W: u32 = 320;
const SURFACE_H: u32 = 200;

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
    /// Per-tick `state_hash` column of the committed golden (index = tick).
    #[cfg(debug_assertions)]
    golden: Vec<u32>,
}

/// Handle of the one `Image` the sprite samples; `tick_and_render` writes it.
#[derive(Resource)]
struct FrameImage(Handle<Image>);

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                // Global nearest-neighbor sampling for crisp integer upscaling.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: WindowResolution::new(960, 600),
                        title: "Liero-rs — 3c demo".into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        // C++ gfx.cpp kDelay = 14ms => one processFrame per ~71.43 Hz tick. The
        // number only sets perceived speed; determinism is by tick count, not
        // wall-clock. `Time<Fixed>` gives the fixed-timestep accumulator for free.
        .insert_resource(Time::<Fixed>::from_hz(1000.0 / 14.0))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, tick_and_render)
        .add_systems(Update, close_on_esc)
        .run();
}

/// Startup: load the scenario, build the sim + render surface + the one Image,
/// spawn the camera and the ×3 sprite, and render tick 0 so the window shows the
/// first frame immediately (before the first `FixedUpdate`).
fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // 1. Read + parse the committed scenario text.
    let scenario_path = format!("{GOLDEN_DIR}/render_slice3b_{SCENARIO_NAME}_scenario.txt");
    let scenario_text = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("read {scenario_path}: {e}"));
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");

    // 2. Tick-0 load (moves `state` into `Sim`; viewports + scene into `Demo`).
    let loaded = scenario::load(Path::new(TC_ROOT), &scenario);
    let scenario::Loaded {
        state,
        viewports,
        scene,
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

    // 6. Assemble the resources.
    #[cfg(debug_assertions)]
    let golden = load_golden_state_hashes(SCENARIO_NAME);

    let mut demo = Demo {
        scenario,
        viewports,
        scene,
        surface,
        tick: 0,
        #[cfg(debug_assertions)]
        golden,
    };
    let sim = Sim(state);

    // 7. Render tick 0 into the surface and upload once (window shows frame 0).
    render_and_upload(&mut demo, &sim.0, &mut images, &handle);

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
) {
    // 1. Advance the sim one tick, feeding the scenario's RECORDED inputs for the
    //    tick we are leaving. (The blood golden was driven with these inputs — the
    //    DART-into-own-feet fire is `input 8 16 0` — so empty inputs would diverge
    //    and trip the self-check. "No input" in 3c means no LIVE player input; the
    //    scenario's replay inputs are part of the deterministic corpus. See the
    //    done-report for this correction to the brief's `ControlState::new()`.)
    let t = demo.tick;
    let inputs = [
        ControlState::unpack(demo.scenario.input(t, 0)),
        ControlState::unpack(demo.scenario.input(t, 1)),
    ];
    sim.0.process_frame(&inputs);

    // 2. Loop step: when `tick` passes `ticks`, rebuild from the loader at tick 0
    //    for a bit-identical loop (fixed seed + fixed inputs).
    let (next, reload) = blit::next_tick(demo.tick, demo.scenario.ticks);
    demo.tick = next;
    if reload {
        let loaded = scenario::load(Path::new(TC_ROOT), &demo.scenario);
        sim.0 = loaded.state;
        demo.viewports = loaded.viewports;
        demo.scene = loaded.scene;
    }

    // 3 + 4. Render the current tick into the CPU surface, then upload.
    render_and_upload(&mut demo, &sim.0, &mut images, &frame.0);

    // 5. Debug-only determinism self-check (the default scenario is a golden).
    //    After the tick+wrap, `demo.tick` names the tick whose state `sim` now
    //    holds: a normal tick `k` matches `golden[k]`; the wrap tick resets to 0
    //    with a fresh tick-0 sim, matching `golden[0]`.
    #[cfg(debug_assertions)]
    debug_assert_eq!(
        sim::hash::hash_game_state(&sim.0),
        demo.golden[demo.tick as usize],
        "sim-hash diverged from the committed golden at tick {}",
        demo.tick
    );
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

/// Parse the committed frame sidecar's 3rd column (`state_hash`, hex u32) into a
/// per-tick `Vec<u32>` (index = tick). Grammar mirrors
/// `render_slice3b_common::parse_frames`: skip blank / `#` / `total` lines.
#[cfg(debug_assertions)]
fn load_golden_state_hashes(name: &str) -> Vec<u32> {
    let path = format!("{GOLDEN_DIR}/render_slice3b_{name}.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut out = Vec::new();
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        if it.next() == Some("total") {
            continue;
        }
        let _frame_hash = it.next().expect("frame_hash column");
        let state_hash = it.next().expect("state_hash column");
        out.push(u32::from_str_radix(state_hash, 16).expect("state_hash hex"));
    }
    out
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
