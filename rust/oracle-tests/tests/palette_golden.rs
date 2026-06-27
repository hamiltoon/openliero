//! Differential test for the palette loader against the C++ oracle.
//! The golden (one line per synthetic buffer: `vga full expand`) is produced by
//! the real C++ `Palette::Read`/`ReadFull`/`ExpandToFullRange`; the Rust ops
//! must reproduce each FNV-1a digest.

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
    let mut bytes = Vec::with_capacity(256 * 3);
    for e in &p.entries {
        bytes.push(e.r);
        bytes.push(e.g);
        bytes.push(e.b);
    }
    fnv1a(&bytes)
}

// MUST match the C++ dumper's synthetic buffers exactly.
fn buf(modulo: usize) -> Vec<u8> {
    (0..256 * 3).map(|i| (i % modulo) as u8).collect()
}

#[test]
fn palette_ops_match_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/palette.txt"
    ))
    .unwrap();

    let bufs = [buf(64), buf(256)];
    let mut lines = golden.lines();

    for (idx, b) in bufs.iter().enumerate() {
        let line = lines.next().unwrap_or_else(|| panic!("missing golden line {idx}"));
        let mut it = line.split_whitespace();
        let want_vga = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        let want_full = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        let want_expand = u64::from_str_radix(it.next().unwrap(), 16).unwrap();

        let vga = Palette::load_vga(b).unwrap();
        let full = Palette::load_full(b).unwrap();
        let mut expand = Palette::load_vga(b).unwrap();
        expand.expand_to_full_range();

        assert_eq!(hash_palette(&vga), want_vga, "vga mismatch buf {idx}");
        assert_eq!(hash_palette(&full), want_full, "full mismatch buf {idx}");
        assert_eq!(hash_palette(&expand), want_expand, "expand mismatch buf {idx}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
