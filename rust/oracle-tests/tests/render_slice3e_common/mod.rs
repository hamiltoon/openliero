//! Shared harness for the Slice-3e per-scenario FRAME-HASH golden tests (T8) —
//! the MILESTONE that proves the **full in-game player view** (world block +
//! HUD bars/text + 52×36 minimap) is pixel-exact vs C++.
//!
//! Factored off `render_slice3b_common`: same tick-by-tick drive of the Step-2
//! `SimState`, same full-world render, but the `render` `Scene` now runs with
//! `draw_hud = map = scenario.hud()` (the `render_hud` directive) and the
//! `font`/`labels` threaded out of `scenario::load`. Each
//! `render_slice3e_<name>_golden.rs` declares `mod render_slice3e_common;` and
//! calls [`run`] to match the committed C++ sidecar golden
//! (`golden/render_slice3e_<name>.txt`) **line-for-line INCLUDING the `total`
//! line**. Column 3 (`state_hash`) is asserted against the Rust `hash_game_state`
//! (rendering is a pure consumer) AND the sim golden's master column
//! (`render_slice3e_<name>_sim.txt`) — the isolation triple, a standing gate that
//! rendering never perturbs the sim.
//!
//! [`modified_frame_hash`] re-drives a scenario to a target tick and renders it
//! with the HUD or minimap suppressed (`draw_hud=false` / `map=false`) — the
//! non-vacuity controls that prove the HUD/minimap pixels are LIVE (a golden that
//! passed while the HUD painted nothing would be a failed proof, spec §4).
//!
//! ## The fixed-camera invariant (inherited from 3b — load-bearing)
//!
//! Every 3e worm keeps its `WormInit` `killed_timer` default, so
//! `viewport::process` takes neither centering arm and the camera stays pinned at
//! the constructed origin — the harness never resets `killed_timer`. (The `death`
//! scenario's victim reaches `killed_timer` via a real in-sim kill, not a reset.)

#![allow(dead_code)] // each test binary uses a subset of the shared surface.

use std::path::Path;

use assets::palette::Palette;
use assets::sprite::SpriteSet;
use assets::tc::ColorAnim;
use oracle_tests::scenario::Scenario;
use sim::hash::hash_game_state;
use sim::state::{ControlState, SimState};

use render::bitmap::Bitmap;
use render::font::Font;
use render::frame::Scene;
use render::hud::HudLabels;
use render::viewport::Viewport;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// One parsed frame-sidecar line: `<tick> <frame_hash_hex16> <state_hash_hex8>`.
pub struct FrameLine {
    pub tick: u32,
    pub frame_hash: u64,
    pub state_hash: u32,
}

/// Parse a frame sidecar into (per-tick lines, total_count, total_accumulator).
/// Identical grammar to the 3a/3b goldens.
pub fn parse_frames(text: &str) -> (Vec<FrameLine>, u32, u64) {
    let mut lines = Vec::new();
    let mut total_n = 0u32;
    let mut total_acc = 0u64;
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        let head = it.next().unwrap();
        if head == "total" {
            total_n = it.next().unwrap().parse().unwrap();
            total_acc = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        } else {
            let tick: u32 = head.parse().unwrap();
            let frame_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            let state_hash = u32::from_str_radix(it.next().unwrap(), 16).unwrap();
            lines.push(FrameLine {
                tick,
                frame_hash,
                state_hash,
            });
        }
    }
    (lines, total_n, total_acc)
}

/// Read a `render_slice3e_<name><suffix>` golden (e.g. `_scenario.txt`, `.txt`,
/// `_sim.txt`) as a string.
pub fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/golden/render_slice3e_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The tick-1 frame hash of a `render_slice3b_<name>.txt` sidecar — read (never
/// hard-coded) so the HUD-on-vs-frozen-3b non-vacuity assert stays honest even if
/// the 3b golden is regenerated.
pub fn read_slice3b_frame_hash(name: &str, tick: u32) -> u64 {
    let path = format!(
        "{}/golden/render_slice3b_{name}.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("total") {
            continue;
        }
        let mut it = t.split_whitespace();
        if it.next().unwrap().parse::<u32>().unwrap() == tick {
            return u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        }
    }
    panic!("render_slice3b_{name}.txt has no tick {tick}");
}

/// Everything built once per scenario: the driven sim, the render surface, the two
/// viewports (fresh, default-seeded RNG), and the owned scene ingredients —
/// including the `font`/`labels` the HUD path now reads.
struct Built {
    scenario: Scenario,
    state: SimState,
    viewports: [Viewport; 2],
    bmp: Bitmap,
    origpal: Palette,
    color_anim: Vec<ColorAnim>,
    fire_cone: SpriteSet,
    nr_begin: i32,
    nr_end: i32,
    laser_weapon: i32,
    font: Font,
    labels: HudLabels,
}

/// Build tick-0 state + render harness for a 3e scenario via `scenario::load`
/// (Bevy-free), exactly as the 3b harness does — the HUD `font`/`labels` come out
/// of the same load.
fn build(name: &str) -> Built {
    let scenario_text = read_golden(name, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "3e scenarios all use seed 42");
    assert!(
        scenario.hud(),
        "3e scenarios carry the `render_hud` directive"
    );

    let loaded = scenario::load(Path::new(TC_ROOT), &scenario);

    Built {
        scenario,
        state: loaded.state,
        viewports: loaded.viewports,
        bmp: Bitmap::new(320, 200),
        origpal: loaded.scene.origpal,
        color_anim: loaded.scene.color_anim,
        fire_cone: loaded.scene.fire_cone,
        nr_begin: loaded.scene.nr_begin,
        nr_end: loaded.scene.nr_end,
        laser_weapon: loaded.scene.laser_weapon,
        font: loaded.scene.font,
        labels: loaded.scene.labels,
    }
}

/// Render one tick's FULL player frame off the current `Built` state, with the HUD
/// and minimap gated by `draw_hud`/`map`. The 3e scenarios carry no
/// `render_shake`/`render_flash`, so (unlike 3b) there is no per-tick shake/flash
/// injection; the shadow pass follows `scenario.shadow()` (off for every 3e
/// scenario). `fade == 0` on tick 0 (matching the sim/frame goldens), 33 after.
fn render_tick(b: &mut Built, tick: u32, draw_hud: bool, map: bool) -> u64 {
    let scene = Scene {
        origpal: &b.origpal,
        color_anim: &b.color_anim,
        fire_cone_sprites: &b.fire_cone,
        bonus_frames: &[],
        nr_begin: b.nr_begin,
        nr_end: b.nr_end,
        laser_weapon: b.laser_weapon,
        screen_flash: 0,
        draw_shadow: b.scenario.shadow(),
        font: &b.font,
        labels: &b.labels,
        draw_hud,
        map,
        small_labels: None,
    };
    render::frame::draw(&mut b.bmp, &b.state, &mut b.viewports, &scene);

    let fade = if tick == 0 { 0 } else { 33 };
    render::hash::hash_frame(&b.bmp, fade)
}

/// The outcome of a full scenario run: the per-tick FULL-view frame hashes (for the
/// non-vacuity asserts) and the tick count.
pub struct RunResult {
    pub ticks: u32,
    pub frame_hashes: Vec<u64>,
}

/// Drive the scenario tick-by-tick, render the full player view (world + HUD +
/// minimap) per tick, and match the committed sidecar golden line-for-line +
/// `total` + the isolation triple (`state_hash` column == `hash_game_state` == sim
/// golden master). Returns the [`RunResult`] for the caller's scenario-specific
/// non-vacuity asserts.
pub fn run(name: &str) -> RunResult {
    let mut b = build(name);
    let ticks = b.scenario.ticks;
    let draw_hud = b.scenario.hud();

    let (frames, total_n, total_acc) = parse_frames(&read_golden(name, ".txt"));
    assert_eq!(
        frames.len(),
        (ticks + 1) as usize,
        "{name}: one frame line per tick 0..=ticks"
    );

    // Isolation source: the sim golden's master column (2nd column, hex).
    let sim_text = read_golden(name, "_sim.txt");
    let sim_master: Vec<u32> = sim_text
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|l| u32::from_str_radix(l.split_whitespace().nth(1).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(
        sim_master.len(),
        (ticks + 1) as usize,
        "{name}: sim golden has one line per tick 0..=ticks"
    );

    let mut acc = render::hash::FNV_OFFSET;
    let mut frame_hashes = Vec::with_capacity((ticks + 1) as usize);

    // tick 0: rendered/hashed BEFORE the first ProcessFrame (as the sim goldens).
    assert_eq!(frames[0].tick, 0, "{name}: first golden line is tick 0");
    let fh0 = render_tick(&mut b, 0, draw_hud, draw_hud);
    assert_eq!(fh0, frames[0].frame_hash, "{name} tick 0: frame hash");
    assert_eq!(
        hash_game_state(&b.state),
        frames[0].state_hash,
        "{name} tick 0: state hash == hash_game_state"
    );
    assert_eq!(
        frames[0].state_hash, sim_master[0],
        "{name} tick 0: isolation vs sim golden master"
    );
    acc = (acc ^ fh0).wrapping_mul(render::hash::FNV_PRIME);
    frame_hashes.push(fh0);

    for k in 1..=ticks {
        let inputs = [
            ControlState::unpack(b.scenario.input(k - 1, 0)),
            ControlState::unpack(b.scenario.input(k - 1, 1)),
        ];
        b.state.process_frame(&inputs);
        let fh = render_tick(&mut b, k, draw_hud, draw_hud);
        let g = &frames[k as usize];
        assert_eq!(g.tick, k, "{name}: golden tick column");
        assert_eq!(fh, g.frame_hash, "{name} tick {k}: frame hash");
        assert_eq!(
            hash_game_state(&b.state),
            g.state_hash,
            "{name} tick {k}: state hash == hash_game_state"
        );
        assert_eq!(
            g.state_hash, sim_master[k as usize],
            "{name} tick {k}: isolation vs sim golden master"
        );
        acc = (acc ^ fh).wrapping_mul(render::hash::FNV_PRIME);
        frame_hashes.push(fh);
    }

    assert_eq!(total_n, ticks + 1, "{name}: total frame count");
    assert_eq!(acc, total_acc, "{name}: total accumulator matches C++");

    RunResult {
        ticks,
        frame_hashes,
    }
}

/// Re-drive the scenario to `target_tick` and render THAT tick with the HUD and/or
/// minimap suppressed. `force_hud`/`force_map` override the scenario defaults
/// (`scenario.hud()`); leaving both `None` reproduces the main run's frame. The sim
/// is driven identically to [`run`] so `target_tick`'s state matches; only the
/// render differs — the non-vacuity controls (HUD off, or minimap off with HUD on).
pub fn modified_frame_hash(
    name: &str,
    target_tick: u32,
    force_hud: Option<bool>,
    force_map: Option<bool>,
) -> u64 {
    let mut b = build(name);
    assert!(target_tick <= b.scenario.ticks, "target tick in range");
    for k in 1..=target_tick {
        let inputs = [
            ControlState::unpack(b.scenario.input(k - 1, 0)),
            ControlState::unpack(b.scenario.input(k - 1, 1)),
        ];
        b.state.process_frame(&inputs);
    }
    let draw_hud = force_hud.unwrap_or(b.scenario.hud());
    let map = force_map.unwrap_or(b.scenario.hud());
    render_tick(&mut b, target_tick, draw_hud, map)
}
