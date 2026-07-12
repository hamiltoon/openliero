use std::io::Cursor;
use std::path::{Path, PathBuf};

use render::bitmap::Bitmap;
use render::hash::{FNV_OFFSET, FNV_PRIME};
use render::viewport::Viewport;
use scenario::{Scenario, SceneData};
use sim::hash::hash_game_state;
use sim::state::{ControlState, SimState};
use sim_core::fixed::itof;

/// Parsed configuration for the headless screenshot CLI.
///
/// Only the argument-parsing surface is populated in this slice (T0); the
/// render/PNG behaviour lands in later tasks.
pub struct Config {
    /// Scenario name to load.
    pub scenario: String,
    /// One or more `--tick` values, in the order they appeared.
    pub ticks: Vec<u32>,
    /// PNG path (single tick) or directory (multiple ticks). `None` when only
    /// `--hashes` was requested.
    pub out: Option<PathBuf>,
    /// Pixel scale factor (default 3).
    pub scale: u32,
    /// Whether `--hashes` was requested.
    pub hashes: bool,
    /// Optional `--tc-root` override.
    pub tc_root: Option<PathBuf>,
    /// Whether `--hud` was requested — opt-in full player view (HUD bars +
    /// minimap). Default false keeps the world-only path byte-identical.
    pub hud: bool,
}

/// Parse the CLI arguments (already stripped of the program name).
///
/// Recognised flags:
/// - `--scenario <name>` (required)
/// - `--tick <u32>` (required, repeatable — accumulates)
/// - `--out <path>`
/// - `--scale <u32>` (default 3)
/// - `--hashes`
/// - `--tc-root <path>`
/// - `--hud` (opt-in full player view: HUD bars + minimap; default off)
///
/// At least one of `--out` / `--hashes` must be present.
pub fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut scenario: Option<String> = None;
    let mut ticks: Vec<u32> = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut scale: u32 = 3;
    let mut hashes = false;
    let mut tc_root: Option<PathBuf> = None;
    let mut hud = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--scenario" => {
                let v = value_for(args, &mut i, "--scenario")?;
                scenario = Some(v);
            }
            "--tick" => {
                let v = value_for(args, &mut i, "--tick")?;
                let tick: u32 = v
                    .parse()
                    .map_err(|_| format!("invalid --tick value: {v}"))?;
                ticks.push(tick);
            }
            "--out" => {
                let v = value_for(args, &mut i, "--out")?;
                out = Some(PathBuf::from(v));
            }
            "--scale" => {
                let v = value_for(args, &mut i, "--scale")?;
                scale = v
                    .parse()
                    .map_err(|_| format!("invalid --scale value: {v}"))?;
            }
            "--hashes" => {
                hashes = true;
            }
            "--hud" => {
                hud = true;
            }
            "--tc-root" => {
                let v = value_for(args, &mut i, "--tc-root")?;
                tc_root = Some(PathBuf::from(v));
            }
            other => {
                return Err(format!("unknown flag: {other}"));
            }
        }
        i += 1;
    }

    let scenario = scenario.ok_or_else(|| "missing required --scenario".to_string())?;
    if ticks.is_empty() {
        return Err("missing required --tick".to_string());
    }
    if out.is_none() && !hashes {
        return Err("require --out or --hashes".to_string());
    }
    // `--scale 0` is geometrically meaningless (a 0x0 image, `w*0` PNG). Reject it
    // here at the argument boundary — a pure argument-validity concern — so `run`
    // and `render_scenario` keep `scale >= 1` as a clean precondition, and the
    // error surfaces alongside the other flag errors + USAGE (T0 review finding).
    if scale == 0 {
        return Err("--scale must be >= 1".to_string());
    }

    Ok(Config {
        scenario,
        ticks,
        out,
        scale,
        hashes,
        tc_root,
        hud,
    })
}

/// Consume the value following a `--flag`, advancing the index past it.
fn value_for(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    let next = args
        .get(*i + 1)
        .ok_or_else(|| format!("missing value for {flag}"))?;
    *i += 1;
    Ok(next.clone())
}

/// One rendered tick: its frame hash (the render-crate FNV-1a over the ARGB
/// buffer), the sim `state_hash`, and — only for the ticks the caller asked to
/// capture — the encoded PNG bytes.
pub struct Frame {
    pub tick: u32,
    pub frame_hash: u64,
    pub state_hash: u32,
    /// PNG bytes iff this tick was in `wanted` (PNG encode is expensive, so it is
    /// skipped for the driven-but-not-captured ticks).
    pub png: Option<Vec<u8>>,
}

/// Render one tick's frame off the current state, mirroring the T8 dumper's
/// per-tick semantics: inject `render_shake` (`shake = itof(amount)`) for the
/// draw and restore it after, feed `render_flash` into `Scene.screen_flash`, and
/// gate the shadow pass on `scenario.shadow()`. Returns the frame hash (fade 0 on
/// tick 0 — the black first frame — else 33, the identity fade).
///
/// This is a deliberate CLI-local copy of `render_slice3b_common::render_tick`;
/// a T2 golden test guards the two against drift.
fn render_tick(
    bmp: &mut Bitmap,
    state: &SimState,
    viewports: &mut [Viewport],
    scene_data: &SceneData,
    scenario: &Scenario,
    tick: u32,
    hud: bool,
) -> u64 {
    let draw_shadow = scenario.shadow();
    let screen_flash = scenario.flash_at(tick).unwrap_or(0);

    // Inject shake for THIS draw (set before, restore after — dumper semantics).
    let shakes = scenario.shake_at(tick);
    for &(vp, amount) in &shakes {
        viewports[vp].shake = itof(amount);
    }

    // `as_scene` returns the world-only Scene (draw_hud/map false). Opt-in `--hud`
    // flips both to paint the full player view (3e's HUD bars + minimap) via the
    // existing `Scene.draw_hud`/`map` path — no new render code. When `hud` is
    // false the Scene is untouched, so the frame stays byte-identical (the golden
    // faithfulness test proves it).
    let mut scene = scene_data.as_scene(screen_flash, draw_shadow);
    if hud {
        scene.draw_hud = true;
        scene.map = true;
    }
    render::frame::draw(bmp, state, viewports, &scene);

    // Restore shake so the next tick's Process is clean (mirror the dumper).
    for &(vp, _) in &shakes {
        viewports[vp].shake = 0;
    }

    let fade = if tick == 0 { 0 } else { 33 };
    render::hash::hash_frame(bmp, fade)
}

/// Drive `scenario` from tick 0 to `up_to`, rendering the FULL world view on
/// EVERY tick (the viewport-local RNG steps per draw, so a skipped tick would
/// desync every later frame). Returns one [`Frame`] per tick `0..=up_to`; PNG
/// bytes are attached only for the ticks in `wanted`. `scale` is the nearest
/// integer upscale factor for those PNGs.
///
/// Tick 0 is rendered/hashed BEFORE the first `process_frame` (as the sim
/// goldens). Inputs are the scenario's recorded 7-bit vectors, never defaults.
pub fn render_scenario(
    tc_root: &Path,
    scenario: &Scenario,
    up_to: u32,
    wanted: &[u32],
    scale: u32,
) -> Vec<Frame> {
    // Public entry: the world-only path (`hud = false`). This is the signature the
    // golden faithfulness + determinism tests drive; keeping it fixed is what lets
    // the no-`--hud` behaviour stay provably byte-identical.
    render_scenario_hud(tc_root, scenario, up_to, wanted, scale, false)
}

/// `render_scenario` with the opt-in `hud` toggle threaded to every per-tick
/// draw. `hud = false` reproduces the public `render_scenario` byte-for-byte;
/// `hud = true` paints the full player view (HUD bars + minimap).
fn render_scenario_hud(
    tc_root: &Path,
    scenario: &Scenario,
    up_to: u32,
    wanted: &[u32],
    scale: u32,
    hud: bool,
) -> Vec<Frame> {
    let mut loaded = scenario::load(tc_root, scenario);
    let mut bmp = Bitmap::new(320, 200);
    let mut frames = Vec::with_capacity((up_to + 1) as usize);

    // tick 0: rendered/hashed BEFORE the first ProcessFrame.
    let fh = render_tick(
        &mut bmp,
        &loaded.state,
        &mut loaded.viewports,
        &loaded.scene,
        scenario,
        0,
        hud,
    );
    let sh = hash_game_state(&loaded.state);
    let png = wanted.contains(&0).then(|| encode_png(&bmp, scale));
    frames.push(Frame {
        tick: 0,
        frame_hash: fh,
        state_hash: sh,
        png,
    });

    for k in 1..=up_to {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        loaded.state.process_frame(&inputs);
        let fh = render_tick(
            &mut bmp,
            &loaded.state,
            &mut loaded.viewports,
            &loaded.scene,
            scenario,
            k,
            hud,
        );
        let sh = hash_game_state(&loaded.state);
        let png = wanted.contains(&k).then(|| encode_png(&bmp, scale));
        frames.push(Frame {
            tick: k,
            frame_hash: fh,
            state_hash: sh,
            png,
        });
    }

    frames
}

/// Encode a `Bitmap` as a nearest ×`scale` RGB PNG: raw colours, alpha dropped,
/// NO fade (the frame-hash fade would blacken tick 0). Per source pixel the RGB
/// is `[(px>>16)&0xff, (px>>8)&0xff, px&0xff]`; the source is addressed by
/// `pitch` (which may exceed `w`), never by `w`.
pub fn encode_png(bmp: &Bitmap, scale: u32) -> Vec<u8> {
    let img = scale_to_rgb(bmp, scale);
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("PNG encode");
    out
}

/// The nearest ×`scale` upscale of `bmp` into an `RgbImage` (the pre-encode
/// surface). Split out so the encode step stays a one-liner and the scaling /
/// channel-order logic is unit-testable in isolation.
fn scale_to_rgb(bmp: &Bitmap, scale: u32) -> image::RgbImage {
    let sc = scale as i32;
    let w = (bmp.w * sc) as u32;
    let h = (bmp.h * sc) as u32;
    let mut img = image::RgbImage::new(w, h);
    for y in 0..h {
        let sy = y as i32 / sc;
        for x in 0..w {
            let sx = x as i32 / sc;
            let px = bmp.pixels[(sy * bmp.pitch + sx) as usize];
            let r = ((px >> 16) & 0xff) as u8;
            let g = ((px >> 8) & 0xff) as u8;
            let b = (px & 0xff) as u8;
            img.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }
    img
}

/// The `oracle-tests/golden/` directory that holds every committed scenario
/// sidecar (both the 3b and 3e corpora).
fn golden_dir() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../oracle-tests/golden"
    ))
}

/// Resolve a scenario name to its committed golden sidecar text. The 3e corpus
/// (`render_slice3e_<name>_scenario.txt`) is tried FIRST, then the 3b corpus
/// (`render_slice3b_<name>_scenario.txt`) as a fallback. The two corpora share
/// no names today, so the order is only a documented tie-break: a name present
/// in both would resolve to its 3e sidecar. On miss, the error lists both paths
/// tried plus every available name scanned from each corpus.
fn resolve_scenario_text(name: &str) -> Result<String, String> {
    let dir = golden_dir();
    // Order: 3e first (full player view / newer corpus), then 3b fallback.
    let candidates = [
        dir.join(format!("render_slice3e_{name}_scenario.txt")),
        dir.join(format!("render_slice3b_{name}_scenario.txt")),
    ];
    for path in &candidates {
        if let Ok(text) = std::fs::read_to_string(path) {
            return Ok(text);
        }
    }
    Err(format!(
        "unknown scenario {:?} (looked for {} then {}). available 3b: [{}]; available 3e: [{}]",
        name,
        candidates[0].display(),
        candidates[1].display(),
        available_names("render_slice3b_").join(", "),
        available_names("render_slice3e_").join(", "),
    ))
}

/// Scan the golden dir for `<prefix><name>_scenario.txt` files and return the
/// sorted `<name>` list — the diagnostic surface for an unknown-scenario error.
fn available_names(prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(golden_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|f| {
            f.strip_prefix(prefix)
                .and_then(|r| r.strip_suffix("_scenario.txt"))
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// Top-level CLI entry: resolve the TC root and scenario-text paths from `cfg`,
/// load + drive + render the scenario, write the requested PNG(s), and (for
/// `--hashes`) emit the sidecar grammar to STDOUT. All info/progress goes to
/// STDERR so `--hashes` stdout stays machine-parseable.
pub fn run(cfg: &Config) -> Result<(), String> {
    let tc_root: PathBuf = cfg.tc_root.clone().unwrap_or_else(|| {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero"
        ))
    });

    let text = resolve_scenario_text(&cfg.scenario)?;
    let scenario = Scenario::parse(&text)?;

    let up_to = *cfg
        .ticks
        .iter()
        .max()
        .expect("parse_args guarantees at least one --tick");
    // PNG encode only when an output path was given (and only for the wanted
    // ticks); a bare `--hashes` run drives+hashes every tick but encodes nothing.
    let wanted: Vec<u32> = if cfg.out.is_some() {
        cfg.ticks.clone()
    } else {
        Vec::new()
    };

    eprintln!(
        "shot: scenario={} up_to={} scale={} hud={} capture={:?}",
        cfg.scenario, up_to, cfg.scale, cfg.hud, cfg.ticks
    );
    let frames = render_scenario_hud(&tc_root, &scenario, up_to, &wanted, cfg.scale, cfg.hud);

    if let Some(out) = &cfg.out {
        let find = |tick: u32| -> &Frame {
            frames
                .iter()
                .find(|f| f.tick == tick)
                .expect("wanted tick was rendered")
        };
        if cfg.ticks.len() == 1 {
            // Single tick: `--out` is the exact PNG file path.
            let f = find(cfg.ticks[0]);
            let png = f.png.as_ref().expect("captured tick has PNG");
            std::fs::write(out, png).map_err(|e| format!("write {}: {e}", out.display()))?;
            eprintln!("shot: wrote {} ({} bytes)", out.display(), png.len());
        } else {
            // Multiple ticks: `--out` is a directory of `<name>_tick<N>.png`.
            std::fs::create_dir_all(out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
            for &tick in &cfg.ticks {
                let f = find(tick);
                let png = f.png.as_ref().expect("captured tick has PNG");
                let path = out.join(format!("{}_tick{}.png", cfg.scenario, tick));
                std::fs::write(&path, png).map_err(|e| format!("write {}: {e}", path.display()))?;
                eprintln!("shot: wrote {} ({} bytes)", path.display(), png.len());
            }
        }
    }

    if cfg.hashes {
        // Sidecar grammar to STDOUT: one `<tick> <fh_hex16> <sh_hex8>` line per
        // tick, then `total <n> <acc_hex16>` (acc: FNV_OFFSET seed, folded per
        // tick) — the exact grammar `render_slice3b_common::parse_frames` reads.
        let mut acc = FNV_OFFSET;
        for f in &frames {
            println!("{} {:016x} {:08x}", f.tick, f.frame_hash, f.state_hash);
            acc = (acc ^ f.frame_hash).wrapping_mul(FNV_PRIME);
        }
        println!("total {} {:016x}", frames.len(), acc);
    }

    Ok(())
}

#[cfg(test)]
mod parse_tests {
    use super::*;
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn minimal_single_tick() {
        let c = parse_args(&v(&[
            "--scenario",
            "blood",
            "--tick",
            "35",
            "--out",
            "/tmp/x.png",
        ]))
        .unwrap();
        assert_eq!(c.scenario, "blood");
        assert_eq!(c.ticks, vec![35]);
        assert_eq!(c.out.as_deref(), Some(std::path::Path::new("/tmp/x.png")));
        assert_eq!(c.scale, 3);
        assert!(!c.hashes);
    }
    #[test]
    fn repeated_tick_and_scale_and_hashes() {
        let c = parse_args(&v(&[
            "--scenario",
            "laser",
            "--tick",
            "5",
            "--tick",
            "9",
            "--scale",
            "4",
            "--hashes",
        ]))
        .unwrap();
        assert_eq!(c.ticks, vec![5, 9]);
        assert_eq!(c.scale, 4);
        assert!(c.hashes);
        assert!(c.out.is_none());
    }
    #[test]
    fn hud_flag_sets_config() {
        // `--hud` is opt-in; default is false, and the flag flips it true.
        let base = parse_args(&v(&[
            "--scenario",
            "hud",
            "--tick",
            "30",
            "--out",
            "/tmp/x.png",
        ]))
        .unwrap();
        assert!(!base.hud, "default: hud off");
        let c = parse_args(&v(&[
            "--scenario",
            "hud",
            "--tick",
            "30",
            "--out",
            "/tmp/x.png",
            "--hud",
        ]))
        .unwrap();
        assert!(c.hud, "--hud sets Config.hud = true");
    }

    #[test]
    fn missing_scenario_is_error() {
        assert!(parse_args(&v(&["--tick", "1", "--out", "/tmp/x.png"])).is_err());
    }
    #[test]
    fn missing_tick_is_error() {
        assert!(parse_args(&v(&["--scenario", "blood", "--out", "/tmp/x.png"])).is_err());
    }
    #[test]
    fn no_out_and_no_hashes_is_error() {
        assert!(parse_args(&v(&["--scenario", "blood", "--tick", "1"])).is_err());
    }
    #[test]
    fn scale_zero_is_error() {
        // `--scale 0` is rejected at the argument boundary (T0 review finding).
        let r = parse_args(&v(&[
            "--scenario",
            "blood",
            "--tick",
            "1",
            "--out",
            "/tmp/x.png",
            "--scale",
            "0",
        ]));
        match r {
            Err(e) => assert!(e.contains("scale"), "error mentions scale: {e}"),
            Ok(_) => panic!("--scale 0 must be rejected"),
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use render::bitmap::{Bitmap, Rect};

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

    fn two_px(pitch: i32, pixels: Vec<u32>) -> Bitmap {
        Bitmap {
            w: 2,
            h: 1,
            pitch,
            pixels,
            clip: Rect::new(0, 0, 2, 1),
            cycles: 0,
        }
    }

    fn decode(png: &[u8]) -> image::RgbImage {
        image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .expect("decode PNG")
            .to_rgb8()
    }

    #[test]
    fn encode_png_scales_and_orders_channels() {
        // Red then green, ×2 nearest. Channel order must be R,G,B (alpha dropped).
        let bmp = two_px(2, vec![0xFF_2A_00_00, 0xFF_00_2A_00]);
        let img = decode(&encode_png(&bmp, 2));
        assert_eq!(img.dimensions(), (4, 2), "×2 nearest upscale");
        assert_eq!(img.get_pixel(0, 0).0, [0x2A, 0, 0], "top-left is red");
        assert_eq!(img.get_pixel(2, 0).0, [0, 0x2A, 0], "x=2 is green");
    }

    #[test]
    fn encode_png_respects_pitch_not_w() {
        // Logical 2x1 backed by pitch=4 (two dead columns). The dead columns must
        // NOT leak into the encoded image — addressing is by pitch, never w.
        let bmp = two_px(
            4,
            vec![0xFF_2A_00_00, 0xFF_00_2A_00, 0xFF_DE_AD_BE, 0xFF_DE_AD_BF],
        );
        let img = decode(&encode_png(&bmp, 1));
        assert_eq!(img.dimensions(), (2, 1));
        assert_eq!(img.get_pixel(0, 0).0, [0x2A, 0, 0]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 0x2A, 0], "no dead-column leak");
    }

    #[test]
    fn resolves_3e_hud_scenario() {
        // The 3e corpus committed `render_slice3e_hud_scenario.txt`; the CLI must
        // resolve the bare name "hud" against it (3b-only lookup misses it).
        assert!(
            resolve_scenario_text("hud").is_ok(),
            "scenario \"hud\" resolves to the 3e golden sidecar"
        );
    }

    #[test]
    fn resolves_existing_3b_scenario() {
        // Existing 3b names keep resolving unchanged.
        assert!(
            resolve_scenario_text("blood").is_ok(),
            "3b \"blood\" resolves"
        );
    }

    #[test]
    fn unknown_scenario_lists_both_corpora() {
        // The diagnostic for a bad name lists what was tried plus the available
        // names from BOTH the 3b and 3e corpora.
        let err = resolve_scenario_text("nope").unwrap_err();
        assert!(
            err.contains("render_slice3e_nope"),
            "lists the 3e path tried: {err}"
        );
        assert!(
            err.contains("render_slice3b_nope"),
            "lists the 3b path tried: {err}"
        );
        assert!(err.contains("blood"), "lists a 3b name: {err}");
        assert!(err.contains("hud"), "lists a 3e name: {err}");
    }

    #[test]
    fn render_scenario_is_deterministic() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../oracle-tests/golden/render_slice3b_blood_scenario.txt"
        ))
        .expect("read blood scenario");
        let scenario = Scenario::parse(&text).expect("parse");
        let a = render_scenario(Path::new(TC_ROOT), &scenario, 5, &[5], 3);
        let b = render_scenario(Path::new(TC_ROOT), &scenario, 5, &[5], 3);
        let fa = a.iter().find(|f| f.tick == 5).unwrap();
        let fb = b.iter().find(|f| f.tick == 5).unwrap();
        assert_eq!(fa.frame_hash, fb.frame_hash, "frame hash is deterministic");
        assert!(fa.png.is_some(), "tick 5 was captured");
        assert_eq!(fa.png, fb.png, "PNG bytes are deterministic");
        assert_eq!(a.len(), 6, "one frame per tick 0..=5");
    }
}
