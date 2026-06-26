//! Differential test for the sprite loader against the C++ oracle. The golden
//! (3 bank lines `<label> <count> <w> <h> <data_hash>` + `exepal <hash>`) is
//! produced by the real C++ `Common::load`; the Rust loader must reproduce every
//! digest and bank shape from the same shipped TGA files.

use assets::palette::Palette;
use assets::sprite::{SpriteSet, Tga};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hash_palette(p: &Palette) -> u64 {
    let mut b = Vec::with_capacity(256 * 3);
    for e in &p.entries {
        b.push(e.r);
        b.push(e.g);
        b.push(e.b);
    }
    fnv1a(&b)
}

fn read_tga(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../data/TC/openliero/sprites/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn sprites_match_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sprite.txt"
    ))
    .unwrap();

    // (label, file, sprite_width, sprite_height, count)
    let banks = [
        ("small", "small.tga", 7, 7, 130),
        ("large", "large.tga", 16, 16, 110),
        ("text", "text.tga", 4, 4, 26),
    ];

    let mut lines = golden.lines();

    // small.tga carries exepal; keep its parsed Tga to check the palette line.
    let mut small_palette: Option<Palette> = None;

    for (label, file, sw, sh, count) in banks {
        let line = lines.next().unwrap_or_else(|| panic!("missing line for {label}"));
        let mut it = line.split_whitespace();
        assert_eq!(it.next().unwrap(), label, "label order");
        let want_count: i32 = it.next().unwrap().parse().unwrap();
        let want_w: i32 = it.next().unwrap().parse().unwrap();
        let want_h: i32 = it.next().unwrap().parse().unwrap();
        let want_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();

        let tga = Tga::load(&read_tga(file)).unwrap();
        let set = SpriteSet::from_tga(&tga, sw, sh, count).unwrap();
        if label == "small" {
            small_palette = Some(tga.palette.clone());
        }

        assert_eq!(set.count, want_count, "{label} count");
        assert_eq!(set.width, want_w, "{label} width");
        assert_eq!(set.height, want_h, "{label} height");
        assert_eq!(fnv1a(&set.data), want_hash, "{label} data hash");
    }

    // exepal line: small.tga's colour map.
    let line = lines.next().expect("missing exepal line");
    let mut it = line.split_whitespace();
    assert_eq!(it.next().unwrap(), "exepal");
    let want_pal = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
    assert_eq!(hash_palette(&small_palette.unwrap()), want_pal, "exepal hash");

    assert!(lines.next().is_none(), "extra golden lines");
}
