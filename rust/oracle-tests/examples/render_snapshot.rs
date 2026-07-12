//! Headless world-view snapshot: drive a golden scenario N ticks with the
//! pure-Rust renderer (no Bevy/GPU) and write ×3 nearest-scaled BMP frames.
//! Dev tool only — the real screenshot CLI is slice 3d.
//!
//! Usage: cargo run -p oracle-tests --example render_snapshot -- <name> <tick> [<tick>...]
//!   e.g. cargo run -p oracle-tests --example render_snapshot -- blood 10 20 35
//! Output: rust/target/snapshots/<name>_tick<N>.bmp

use std::io::Write as _;
use std::path::Path;

use render::bitmap::Bitmap;
use sim::state::ControlState;

const SCALE: usize = 3;

fn write_bmp(path: &Path, bmp: &Bitmap) {
    let w = bmp.w as usize * SCALE;
    let h = bmp.h as usize * SCALE;
    let row_bytes = w * 3;
    let pad = (4 - row_bytes % 4) % 4;
    let data_size = (row_bytes + pad) * h;
    let file_size = 54 + data_size;

    let mut out = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    // Pixel rows, bottom-up, BGR, ×SCALE nearest in both axes.
    for y in (0..h).rev() {
        let sy = y / SCALE;
        for x in 0..w {
            let sx = x / SCALE;
            let px = bmp.pixels[sy * bmp.pitch as usize + sx];
            out.push((px & 0xff) as u8); // B
            out.push(((px >> 8) & 0xff) as u8); // G
            out.push(((px >> 16) & 0xff) as u8); // R
        }
        out.extend_from_slice(&vec![0u8; pad]);
    }
    let mut f = std::fs::File::create(path).expect("create bmp");
    f.write_all(&out).expect("write bmp");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args.first().map(String::as_str).unwrap_or("blood");
    let ticks: Vec<u32> = if args.len() > 1 {
        args[1..].iter().map(|s| s.parse().expect("tick number")).collect()
    } else {
        vec![10, 20, 35]
    };

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let scenario_path = format!(
        "{root}/rust/oracle-tests/golden/render_slice3b_{name}_scenario.txt"
    );
    let text = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("read {scenario_path}: {e}"));
    let scenario = scenario::Scenario::parse(&text).expect("scenario parses");
    let tc_root = format!("{root}/data/TC/openliero");
    let mut loaded = scenario::load(Path::new(&tc_root), &scenario);

    let out_dir = format!("{root}/rust/target/snapshots");
    std::fs::create_dir_all(&out_dir).expect("mkdir snapshots");

    let mut bmp = Bitmap::new(320, 200);
    let max_tick = *ticks.iter().max().unwrap_or(&0);
    for k in 0..=max_tick {
        if k > 0 {
            let inputs = [
                ControlState::unpack(scenario.input(k - 1, 0)),
                ControlState::unpack(scenario.input(k - 1, 1)),
            ];
            loaded.state.process_frame(&inputs);
        }
        if ticks.contains(&k) {
            let scene = loaded.scene.as_scene(0, scenario.shadow());
            render::frame::draw(&mut bmp, &loaded.state, &mut loaded.viewports, &scene);
            let path = format!("{out_dir}/{name}_tick{k}.bmp");
            write_bmp(Path::new(&path), &bmp);
            println!("{path}");
        }
    }
}
