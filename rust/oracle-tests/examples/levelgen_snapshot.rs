//! Eyeball a generated level: run `sim::levelgen::generate_from_settings` for a seed and
//! size and write the material map 1:1 as a BMP through `small.tga`'s embedded palette
//! (= C++ `common.exepal`). Dev tool only — `tests/levelgen_golden.rs` is the correctness
//! gate; this only answers "does it look like a Liero level?".
//!
//! Usage: cargo run -p oracle-tests --example levelgen_snapshot -- <seed> [<w> <h>] [noshadow]
//!   e.g. cargo run -p oracle-tests --example levelgen_snapshot -- 42 600 350
//! Output: rust/target/snapshots/levelgen_<seed>_<w>x<h>[_noshadow].bmp

use std::io::Write as _;

use assets::palette::Palette;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams};
use sim_core::rng::Rand;

/// 24-bit bottom-up BMP (same dependency-free writer as `render_snapshot.rs`, 1:1 scale).
fn write_bmp(path: &str, w: usize, h: usize, ids: &[u8], pal: &Palette) {
    let row_bytes = w * 3;
    let pad = (4 - row_bytes % 4) % 4;
    let data_size = (row_bytes + pad) * h;
    let file_size = 54 + data_size;
    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    for y in (0..h).rev() {
        for x in 0..w {
            let c = pal.entries[ids[y * w + x] as usize];
            out.extend_from_slice(&[c.b, c.g, c.r]);
        }
        out.extend_from_slice(&vec![0u8; pad]);
    }
    let mut f = std::fs::File::create(path).expect("create bmp");
    f.write_all(&out).expect("write bmp");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed: u32 = args
        .first()
        .map(|s| s.parse().expect("seed is a u32"))
        .unwrap_or(42);
    let (w, h): (i32, i32) = if args.len() >= 3 && args[1] != "noshadow" {
        (
            args[1].parse().expect("width"),
            args[2].parse().expect("height"),
        )
    } else {
        (504, 350)
    };
    let shadow = !args.iter().any(|a| a == "noshadow");

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let tc_root = format!("{root}/data/TC/openliero");
    let tc_bytes = std::fs::read(format!("{tc_root}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let large_bytes =
        std::fs::read(format!("{tc_root}/sprites/large.tga")).expect("read large.tga");
    let large_tga = Tga::load(&large_bytes).expect("large.tga parses");
    let large = SpriteSet::from_tga(&large_tga, 16, 16, 110).expect("large sprite bank");
    let small_bytes =
        std::fs::read(format!("{tc_root}/sprites/small.tga")).expect("read small.tga");
    let small_tga = Tga::load(&small_bytes).expect("small.tga parses");

    let assets = LevelGenAssets {
        large_sprites: &large,
        textures: &tc.textures,
        material_flags: &tc.materials,
    };
    let params = LevelGenParams {
        random_level: true,
        random_map_width: w,
        random_map_height: h,
        shadow,
    };
    let mut rand = Rand::new();
    rand.seed(seed);
    let level = generate_from_settings(&assets, &params, None, &mut rand);

    let out_dir = format!("{root}/rust/target/snapshots");
    std::fs::create_dir_all(&out_dir).expect("mkdir snapshots");
    let suffix = if shadow { "" } else { "_noshadow" };
    let path = format!("{out_dir}/levelgen_{seed}_{w}x{h}{suffix}.bmp");
    write_bmp(
        &path,
        w as usize,
        h as usize,
        &level.material_id,
        &small_tga.palette,
    );
    println!("{path}");
}
