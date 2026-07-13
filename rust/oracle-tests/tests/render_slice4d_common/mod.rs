//! Shared harness for the Slice-4d LIVE flash/shake/banner/centering golden
//! (the MILESTONE): the first render test that drives the scenario through the
//! **live game-layer viewport path** (Slice 4d T2 `game::viewport_step`) rather
//! than the 3b injection or the 3e fixed-camera path.
//!
//! Per tick the drive reproduces C++ `Game::ProcessFrame`'s phase order by
//! splitting the viewport work around the sim's atomic `process_frame`
//! (design §3), byte-for-byte mirroring `game/src/viewport_step.rs`
//! (`tick_viewports`) — the canonical owner in the binary, which `oracle-tests`
//! cannot depend on (it would pull Bevy into the Bevy-free test crate), so the
//! ~15 ordering-critical lines are reproduced here under the same citations:
//!
//! ```text
//! step_before_frame(viewports, sim.cycles, sim.worms)   // shake-- + banner walk (BEFORE)
//! sim.process_frame(inputs)                              // phase-1 flash-- + phases 2-6
//! apply_shake_events(viewports, sim.drain_shake_events())// explosion-max (AFTER, no reader between)
//! frame::draw(.., screen_flash = sim.screen_flash, ..)   // phase-5 centering + shake-RNG + banner reset
//! ```
//!
//! Each tick matches the committed C++ sidecar
//! (`golden/render_slice4d_live.txt`) line-for-line INCLUDING the `total` line;
//! column 3 (`state_hash`) is asserted against the Rust `hash_game_state` (render
//! is a pure consumer) AND the sim golden master (`render_slice4d_live_sim.txt`)
//! — the isolation triple. The per-tick viewport snapshots + `screen_flash` are
//! returned for the scenario-specific non-vacuity proofs (flash decay, shake
//! jitter, banner walk, camera follow).

#![allow(dead_code)] // the test binary uses a subset of the shared surface.

use std::path::Path;

use assets::palette::Palette;
use assets::sprite::SpriteSet;
use assets::tc::ColorAnim;
use oracle_tests::scenario::Scenario;
use sim::hash::hash_game_state;
use sim::shake::ShakeEvent;
use sim::state::{ControlState, SimState, WormState};
use sim_core::fixed::itof;

use render::font::Font;
use render::frame::Scene;
use render::hud::HudLabels;
use render::viewport::Viewport;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

// --- The live viewport stepping, reproduced verbatim from game/src/viewport_step.rs
// (the binary's `tick_viewports`), which oracle-tests cannot import (Bevy). ----------

/// `game.cpp:275-285`.
const SHAKE_DECREMENT: i32 = 4000;
/// `game.cpp:298-311`: banner walks in `[-8, 2]`, resting hidden at `-8`.
const BANNER_LOW: i32 = -8;
const BANNER_HIGH: i32 = 2;
/// `game.cpp:302`: the `killed_timer` above which the death banner walks into view.
const BANNER_KILLED_TIMER: i32 = 16;

/// C++ top-of-frame viewport stepping (`game.cpp:271-332`), run BEFORE
/// `process_frame` on the PREVIOUS tick's `shake`, the pre-`++cycles` `cycles`,
/// and the pre-worm-loop `killed_timer` (mirrors `viewport_step::step_before_frame`).
fn step_before_frame(viewports: &mut [Viewport], cycles: i32, worms: &[WormState]) {
    for vp in viewports.iter_mut() {
        if vp.shake > 0 {
            vp.shake -= SHAKE_DECREMENT;
        }
    }
    if (cycles & 1) == 0 {
        for vp in viewports.iter_mut() {
            let down = worms[vp.worm_idx].killed_timer > BANNER_KILLED_TIMER;
            if down {
                if vp.banner_y < BANNER_HIGH {
                    vp.banner_y += 1;
                }
            } else if vp.banner_y > BANNER_LOW {
                vp.banner_y -= 1;
            }
        }
    }
}

/// Apply the sim's drained explosion-shake events (`sobject.cpp:27-33`, moved
/// game-side) AFTER `process_frame` (mirrors `viewport_step::apply_shake_events`):
/// for every viewport whose CURRENT window contains the raw blast, `shake =
/// max(itof(amount), shake)`.
fn apply_shake_events(viewports: &mut [Viewport], events: &[ShakeEvent]) {
    for ev in events {
        for vp in viewports.iter_mut() {
            let contains = ev.x > vp.x
                && ev.x < vp.x + vp.rect.width()
                && ev.y > vp.y
                && ev.y < vp.y + vp.rect.height();
            if contains {
                vp.shake = vp.shake.max(itof(ev.amount));
            }
        }
    }
}

/// A viewport's post-`frame::draw` state for a tick (centering + shake jitter
/// already folded into `x`/`y`). Captured every tick for the non-vacuity proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VpSnap {
    pub x: i32,
    pub y: i32,
    pub shake: i32,
    pub banner_y: i32,
}

pub struct FrameLine {
    pub tick: u32,
    pub frame_hash: u64,
    pub state_hash: u32,
}

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

pub fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/golden/render_slice4d_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

struct Built {
    scenario: Scenario,
    state: SimState,
    viewports: [Viewport; 2],
    bmp: render::bitmap::Bitmap,
    origpal: Palette,
    color_anim: Vec<ColorAnim>,
    fire_cone: SpriteSet,
    nr_begin: i32,
    nr_end: i32,
    laser_weapon: i32,
    font: Font,
    labels: HudLabels,
}

fn build(name: &str) -> Built {
    let scenario_text = read_golden(name, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "4d scenario uses seed 42");
    assert!(
        scenario.live(),
        "4d scenario carries the `render_live` directive"
    );

    let loaded = scenario::load(Path::new(TC_ROOT), &scenario);
    Built {
        scenario,
        state: loaded.state,
        viewports: loaded.viewports,
        bmp: render::bitmap::Bitmap::new(320, 200),
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

/// Render one tick's world frame with the given `screen_flash` (the 4d scenario
/// carries no `render_hud`, so `draw_hud`/`map` stay false — world-only, as the
/// C++ `render_live` path draws). Returns the frame hash; the viewports are
/// mutated in place (centering + shake-RNG), so the caller snapshots them after.
fn render_tick(b: &mut Built, tick: u32, screen_flash: i32) -> u64 {
    let scene = Scene {
        origpal: &b.origpal,
        color_anim: &b.color_anim,
        fire_cone_sprites: &b.fire_cone,
        bonus_frames: &[],
        nr_begin: b.nr_begin,
        nr_end: b.nr_end,
        laser_weapon: b.laser_weapon,
        screen_flash,
        draw_shadow: b.scenario.shadow(),
        font: &b.font,
        labels: &b.labels,
        draw_hud: false,
        map: false,
    };
    render::frame::draw(&mut b.bmp, &b.state, &mut b.viewports, &scene);
    let fade = if tick == 0 { 0 } else { 33 };
    render::hash::hash_frame(&b.bmp, fade)
}

pub struct RunResult {
    pub ticks: u32,
    pub frame_hashes: Vec<u64>,
    /// Per-tick `[vp0, vp1]` post-draw snapshots (index == tick).
    pub vps: Vec<[VpSnap; 2]>,
    /// Per-tick live `sim.screen_flash` fed into the palette LightUp (index == tick).
    pub screen_flash: Vec<i32>,
}

fn snap(vps: &[Viewport; 2]) -> [VpSnap; 2] {
    [
        VpSnap {
            x: vps[0].x,
            y: vps[0].y,
            shake: vps[0].shake,
            banner_y: vps[0].banner_y,
        },
        VpSnap {
            x: vps[1].x,
            y: vps[1].y,
            shake: vps[1].shake,
            banner_y: vps[1].banner_y,
        },
    ]
}

/// Drive the scenario tick-by-tick through the LIVE game-layer viewport path,
/// matching the committed sidecar golden line-for-line + `total` + the isolation
/// triple. Returns the per-tick frame hashes, viewport snapshots, and live
/// `screen_flash` for the caller's non-vacuity proofs.
pub fn run(name: &str) -> RunResult {
    let mut b = build(name);
    let ticks = b.scenario.ticks;

    let (frames, total_n, total_acc) = parse_frames(&read_golden(name, ".txt"));
    assert_eq!(
        frames.len(),
        (ticks + 1) as usize,
        "{name}: one frame line per tick 0..=ticks"
    );

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
    let mut vps = Vec::with_capacity((ticks + 1) as usize);
    let mut screen_flash = Vec::with_capacity((ticks + 1) as usize);

    // tick 0: rendered/hashed BEFORE the first ProcessFrame (as the sim goldens).
    // No stepping runs yet; frame::draw's Viewport::process is a no-op for
    // centering (every worm still `killed_timer=150`), so the camera stays at
    // origin exactly as the C++ render_live tick-0 draw (ProcessFrame not yet run).
    assert_eq!(frames[0].tick, 0, "{name}: first golden line is tick 0");
    let sf0 = b.state.screen_flash;
    let fh0 = render_tick(&mut b, 0, sf0);
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
    vps.push(snap(&b.viewports));
    screen_flash.push(sf0);

    for k in 1..=ticks {
        let inputs = [
            ControlState::unpack(b.scenario.input(k - 1, 0)),
            ControlState::unpack(b.scenario.input(k - 1, 1)),
        ];
        // The live game-layer tick (viewport_step::tick_viewports order).
        step_before_frame(&mut b.viewports, b.state.cycles, &b.state.worms);
        b.state.process_frame(&inputs);
        let events = b.state.drain_shake_events();
        apply_shake_events(&mut b.viewports, &events);

        let sf = b.state.screen_flash;
        let fh = render_tick(&mut b, k, sf);
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
        vps.push(snap(&b.viewports));
        screen_flash.push(sf);
    }

    assert_eq!(total_n, ticks + 1, "{name}: total frame count");
    assert_eq!(acc, total_acc, "{name}: total accumulator matches C++");

    RunResult {
        ticks,
        frame_hashes,
        vps,
        screen_flash,
    }
}

/// Re-drive the scenario EXACTLY as [`run`] (rendering — hence `Viewport::process`
/// — every tick, so the incremental `scroll_to` arm reached during the respawn's
/// invisible window evolves identically) and apply a control override ONLY on the
/// final `target_tick` draw — the non-vacuity controls. `force_flash` overrides
/// that draw's palette `screen_flash` (e.g. `Some(0)` suppresses the flash);
/// `suppress_shake` zeroes every viewport's `shake` right before that draw
/// (centering-only, no shake jitter). Returns `(frame_hash, [vp0 (x,y), vp1
/// (x,y)])` — the post-draw camera positions. All prior ticks render with their
/// real live values, so the state entering `target_tick` matches `run` bit-for-bit.
pub fn modified(
    name: &str,
    target_tick: u32,
    force_flash: Option<i32>,
    suppress_shake: bool,
) -> (u64, [(i32, i32); 2]) {
    let mut b = build(name);
    assert!(target_tick <= b.scenario.ticks, "target tick in range");

    // Applies the target-tick override (if this is `target_tick`) then draws.
    let draw = |b: &mut Built, k: u32| -> u64 {
        let mut sf = b.state.screen_flash;
        if k == target_tick {
            if let Some(f) = force_flash {
                sf = f;
            }
            if suppress_shake {
                for vp in b.viewports.iter_mut() {
                    vp.shake = 0;
                }
            }
        }
        render_tick(b, k, sf)
    };

    let mut fh = draw(&mut b, 0);
    if target_tick == 0 {
        return (
            fh,
            [
                (b.viewports[0].x, b.viewports[0].y),
                (b.viewports[1].x, b.viewports[1].y),
            ],
        );
    }
    for k in 1..=target_tick {
        let inputs = [
            ControlState::unpack(b.scenario.input(k - 1, 0)),
            ControlState::unpack(b.scenario.input(k - 1, 1)),
        ];
        step_before_frame(&mut b.viewports, b.state.cycles, &b.state.worms);
        b.state.process_frame(&inputs);
        let events = b.state.drain_shake_events();
        apply_shake_events(&mut b.viewports, &events);
        fh = draw(&mut b, k);
    }
    (
        fh,
        [
            (b.viewports[0].x, b.viewports[0].y),
            (b.viewports[1].x, b.viewports[1].y),
        ],
    )
}
