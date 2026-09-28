//! Step 4½c T8 — the weapon-selection screen, gated BIT-EXACT against the REAL C++
//! `WeaponSelection::Draw` (plan Addendum A1, which replaces the plan's Rust self-goldens).
//! `oracle_dump_weapsel --frames` (T5/T6, `gen_weapsel_golden.sh`) drew every frame of the 15
//! render cases headlessly and wrote `golden/weapsel_<case>_frames.txt`: per drawn frame
//! `<frame> <hash_frame(bmp, 33)> <menu_cycles> <fade>`, under a `#` header naming the
//! `--menu-cycles` start. This test drives the same scenario through the faithful Rust path —
//! `new_match` (invisible worms, lives 0), `Game::Focus`'s worm colour ramps
//! (`render::palette::set_worm_colour`), `sim::weapsel`, `render::weapsel` — and compares every
//! line. A drawn frame is the state after that frame's `process_frame`; the frame the phase ends
//! on is not drawn (the C++ controller has already left the state). The frozen background is
//! built once, on frame 0, with that draw's `menu_cycles`.
//!
//! `WEAPSEL_RUST_PPM_DIR=<dir>` also writes every Rust frame as `<dir>/<case>/weapsel_NNNN.ppm`
//! (the C++ dumper's `--ppm-dir` layout) for diffing a mismatch by eye.

mod weapsel_common;

use std::path::Path;

use render::bitmap::Bitmap;
use render::hash::hash_frame;
use render::palette::set_worm_colour;
use render::weapsel::{build_frozen, draw_screen, level_label, weapsel_palette};
use scenario::build::{new_match, weapsel_config};
use scenario::settings::MatchConfig;
use sim::state::ControlState;
use sim::weapsel::WeaponSelection;
use weapsel_common as wc;

/// One committed C++ frame line.
#[derive(Debug, PartialEq, Eq)]
struct FrameLine {
    frame: u32,
    hash: u64,
    menu_cycles: u32,
    fade: i32,
}

/// The `--menu-cycles` start from the header and the data lines of `weapsel_<name>_frames.txt`
/// (`None` for a case with no render sidecar: `bots_only_7` ends on frame 0).
fn golden_frames(name: &str) -> Option<(u32, Vec<FrameLine>)> {
    let path = Path::new(wc::GOLDEN).join(format!("weapsel_{name}_frames.txt"));
    let text = std::fs::read_to_string(path).ok()?;
    let start = text
        .lines()
        .filter(|l| l.starts_with('#'))
        .find_map(|l| l.split("--menu-cycles ").nth(1))
        .and_then(|rest| rest.split_whitespace().next())
        .map(|n| n.parse::<u32>().expect("--menu-cycles is a u32"))
        .unwrap_or_else(|| panic!("{name}: the header names --menu-cycles"));
    let lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(f.len(), 4, "{name}: `{l}`");
            FrameLine {
                frame: f[0].parse().unwrap(),
                hash: u64::from_str_radix(f[1], 16).unwrap(),
                menu_cycles: f[2].parse().unwrap(),
                fade: f[3].parse().unwrap(),
            }
        })
        .collect();
    Some((start, lines))
}

/// Draw every frame of `name`'s phase in Rust, `menu_cycles` counting up from `start` (unsigned
/// wrap, `Gfx::menu_cycles`), as `FrameLine`s plus the surfaces.
fn rust_frames(name: &str, start: u32) -> Vec<(FrameLine, Bitmap)> {
    let scenario = wc::read_scenario(name);
    let settings = wc::read_settings(&scenario);
    let level = wc::load_level(&scenario.level);
    let cfg = MatchConfig {
        settings: settings.clone(),
        seed: scenario.seed,
    };
    let mut loaded = new_match(Path::new(wc::TC_ROOT), &cfg, &level)
        .unwrap_or_else(|e| panic!("{name}: builds: {e}"));
    let mut ws = WeaponSelection::new(&mut loaded.state, &weapsel_config(&settings))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    // LocalController::Focus -> Game::Focus (game.cpp:473-488), right after the constructor.
    for (i, w) in settings.worm_settings.iter().take(2).enumerate() {
        set_worm_colour(&mut loaded.scene.origpal, i, w.rgb);
    }
    let label = level_label(&loaded.scene.weapsel_texts, &settings.level_file);
    let names = [
        settings.worm_settings[0].name.as_str(),
        settings.worm_settings[1].name.as_str(),
    ];
    let end = scenario.weapsel_end().expect("a weapsel line");
    let mut frozen: Option<Bitmap> = None;
    let mut out = Vec::new();
    for f in 0..=end {
        let input = [0, 1].map(|i| ControlState::unpack(scenario.weapsel_input(f, i)));
        if ws.process_frame(&mut loaded.state, &input) {
            assert_eq!(f, end, "{name}: the phase ends on its last weapsel frame");
            break;
        }
        let menu_cycles = start.wrapping_add(f);
        let frozen = frozen.get_or_insert_with(|| {
            let mut scene = loaded.scene.as_scene(0, settings.shadow);
            scene.draw_hud = true;
            scene.map = settings.map;
            build_frozen(&loaded.state, &scene, &label, menu_cycles)
        });
        let mut surface = Bitmap::new(320, 200);
        draw_screen(
            &mut surface,
            frozen,
            &weapsel_palette(&loaded.scene.origpal, menu_cycles),
            &loaded.scene.font,
            &loaded.scene.weapsel_texts,
            &ws,
            &loaded.state.weapons,
            names,
        );
        let line = FrameLine {
            frame: f,
            hash: hash_frame(&surface, 33),
            menu_cycles,
            fade: (f as i32 + 1).min(33),
        };
        out.push((line, surface));
    }
    out
}

fn write_ppm(path: &Path, bmp: &Bitmap) {
    let mut data = format!("P6\n{} {}\n255\n", bmp.w, bmp.h).into_bytes();
    for &p in &bmp.pixels {
        data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    std::fs::write(path, data).unwrap();
}

#[test]
fn every_weapsel_frame_is_bit_exact_against_the_cpp_draw() {
    let ppm_dir = std::env::var_os("WEAPSEL_RUST_PPM_DIR").map(std::path::PathBuf::from);
    let mut cases = 0;
    let mut frames = 0;
    let mut bad = Vec::new();
    for c in &wc::CASES {
        let Some((start, want)) = golden_frames(c.name) else {
            assert_eq!(c.name, "bots_only_7", "only bots_only_7 draws nothing");
            continue;
        };
        cases += 1;
        let got = rust_frames(c.name, start);
        if let Some(dir) = &ppm_dir {
            let dir = dir.join(c.name);
            std::fs::create_dir_all(&dir).unwrap();
            for (line, bmp) in &got {
                write_ppm(&dir.join(format!("weapsel_{:04}.ppm", line.frame)), bmp);
            }
        }
        assert_eq!(got.len(), want.len(), "{}: drawn frame count", c.name);
        for ((g, _), w) in got.iter().zip(&want) {
            frames += 1;
            if g != w {
                bad.push(format!(
                    "{} frame {}: rust {:016x} (cycles {}) vs C++ {:016x} (cycles {})",
                    c.name, w.frame, g.hash, g.menu_cycles, w.hash, w.menu_cycles
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {frames} frames differ:\n{}",
        bad.len(),
        bad.join("\n")
    );
    assert_eq!(
        (cases, frames),
        (15, 361),
        "the whole C++ render corpus is gated"
    );
}

#[test]
fn the_gate_sees_a_one_pixel_change() {
    // Non-vacuity: one pixel of one frame must move the hash the gate compares.
    let (start, want) = golden_frames("few_enabled").unwrap();
    let (line, mut bmp) = rust_frames("few_enabled", start).swap_remove(0);
    assert_eq!(line.hash, want[0].hash);
    bmp.pixels[320 * 100 + 160] ^= 1;
    assert_ne!(hash_frame(&bmp, 33), want[0].hash);
}
