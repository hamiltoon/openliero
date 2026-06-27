//! Differential test for the level loader (material map + POWERLEVEL palette +
//! MODERNLV display layers/animation) against the C++ oracle. The golden line is
//! `w h mat pal dd dv ramp anim` with `-` for absent optional fields, produced
//! by the real C++ `Level::load`; the Rust loader must reproduce every digest.

use assets::level::{load, ArgbRamp, DisplayLayers, LevelData};
use assets::palette::Palette;

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

fn hash_u32_le(v: &[u32]) -> u64 {
    let mut b = Vec::with_capacity(v.len() * 4);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    fnv1a(&b)
}

fn hash_ramps(ramps: &[ArgbRamp]) -> u64 {
    let mut b = Vec::new();
    for r in ramps {
        b.push(r.shift);
        for c in &r.colors {
            b.extend_from_slice(&c.to_le_bytes());
        }
    }
    fnv1a(&b)
}

// MUST match the C++ dumper's synthetic inputs exactly.
fn make_legacy() -> Vec<u8> {
    (0..504 * 350).map(|i| (i % 251) as u8).collect()
}

fn make_ollevel2_base(w: i32, h: i32) -> Vec<u8> {
    let mut b = b"OLLEVEL2".to_vec();
    b.push(0);
    b.extend_from_slice(&(w as u16).to_le_bytes());
    b.extend_from_slice(&(h as u16).to_le_bytes());
    for i in 0..(w * h) {
        b.push(((i * 5 + 2) % 256) as u8);
    }
    b
}

fn powerlevel() -> Vec<u8> {
    let mut b = b"POWERLEVEL".to_vec();
    for i in 0..256 * 3 {
        b.push((i % 64) as u8);
    }
    b
}

// anim_kind: 0=none, 2=bad index (matches the C++ dumper's Modernlv).
fn modernlv(cells: usize, anim_kind: u8) -> Vec<u8> {
    let mut b = b"MODERNLV".to_vec();
    for i in 0..cells {
        b.extend_from_slice(&(0x11223300u32.wrapping_add(i as u32)).to_le_bytes());
    }
    for i in 0..cells {
        b.push((i % 2) as u8);
    }
    if anim_kind != 0 {
        b.push(1); // ramp_count
        b.push(3); // shift
        b.extend_from_slice(&2u16.to_le_bytes()); // color_count
        b.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        b.extend_from_slice(&0x01020304u32.to_le_bytes());
        for i in 0..cells {
            let idx = if anim_kind == 2 && i == 1 { 2 } else { (i % 2) as u8 };
            b.push(idx);
        }
    }
    b
}

fn with_tail(mut base: Vec<u8>, tail: Vec<u8>) -> Vec<u8> {
    base.extend_from_slice(&tail);
    base
}

// Parse a golden hash column: `-` => None, else u64 hex.
fn col(s: &str) -> Option<u64> {
    if s == "-" {
        None
    } else {
        Some(u64::from_str_radix(s, 16).unwrap())
    }
}

#[test]
fn level_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/level.txt"
    ))
    .unwrap();
    let modern = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/Levels/modern_test.lev"
    ))
    .unwrap();

    let inputs: [Vec<u8>; 7] = [
        modern,
        make_legacy(),
        make_ollevel2_base(13, 11),
        with_tail(make_ollevel2_base(4, 4), powerlevel()),
        with_tail(make_ollevel2_base(2, 2), modernlv(4, 0)),
        with_tail(make_ollevel2_base(2, 2), modernlv(4, 2)),
        with_tail(make_ollevel2_base(2, 2), {
            let mut t = powerlevel();
            t.extend_from_slice(&modernlv(4, 0));
            t
        }),
    ];
    let mut lines = golden.lines();

    for (idx, buf) in inputs.iter().enumerate() {
        let line = lines.next().unwrap_or_else(|| panic!("missing golden line {idx}"));
        let mut it = line.split_whitespace();
        let want_w: i32 = it.next().unwrap().parse().unwrap();
        let want_h: i32 = it.next().unwrap().parse().unwrap();
        let want_mat = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        let want_pal = col(it.next().unwrap());
        let want_dd = col(it.next().unwrap());
        let want_dv = col(it.next().unwrap());
        let want_ramp = col(it.next().unwrap());
        let want_anim = col(it.next().unwrap());

        let lvl: LevelData = load(buf).unwrap_or_else(|e| panic!("input {idx} failed: {e:?}"));
        assert_eq!(lvl.width, want_w, "width input {idx}");
        assert_eq!(lvl.height, want_h, "height input {idx}");
        assert_eq!(fnv1a(&lvl.material_id), want_mat, "material input {idx}");

        assert_eq!(lvl.palette.as_ref().map(hash_palette), want_pal, "palette input {idx}");

        let dd = lvl.display.as_ref().map(|d: &DisplayLayers| hash_u32_le(&d.data));
        let dv = lvl.display.as_ref().map(|d| fnv1a(&d.valid));
        assert_eq!(dd, want_dd, "display_data input {idx}");
        assert_eq!(dv, want_dv, "display_valid input {idx}");

        let ramp = lvl
            .display
            .as_ref()
            .filter(|d| !d.ramps.is_empty())
            .map(|d| hash_ramps(&d.ramps));
        let anim = lvl
            .display
            .as_ref()
            .filter(|d| !d.ramps.is_empty())
            .map(|d| fnv1a(&d.anim));
        assert_eq!(ramp, want_ramp, "ramp input {idx}");
        assert_eq!(anim, want_anim, "anim input {idx}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
