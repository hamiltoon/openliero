//! Differential test for the level material-map loader against the C++ oracle.
//! See `fixed_golden.rs` for the golden pattern. The golden stores an FNV-1a hash
//! of each material map (computed by the real C++ `Level::load`); the Rust loader
//! must reproduce the same dimensions and hash for all three inputs.

use assets::level::load;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// MUST match the C++ dumper's synthetic inputs exactly.
fn make_legacy() -> Vec<u8> {
    (0..504 * 350).map(|i| (i % 251) as u8).collect()
}

fn make_ollevel2() -> Vec<u8> {
    let mut b = b"OLLEVEL2".to_vec();
    b.push(0);
    b.extend_from_slice(&13u16.to_le_bytes());
    b.extend_from_slice(&11u16.to_le_bytes());
    for i in 0..13 * 11 {
        b.push(((i * 5 + 2) % 256) as u8);
    }
    b
}

#[test]
fn level_material_map_matches_cpp_oracle() {
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

    let inputs: [Vec<u8>; 3] = [modern, make_legacy(), make_ollevel2()];
    let mut lines = golden.lines();

    for (idx, buf) in inputs.iter().enumerate() {
        let line = lines.next().unwrap_or_else(|| panic!("missing golden line {idx}"));
        let mut it = line.split_whitespace();
        let want_w: i32 = it.next().unwrap().parse().unwrap();
        let want_h: i32 = it.next().unwrap().parse().unwrap();
        let want_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();

        let lvl = load(buf).unwrap_or_else(|e| panic!("input {idx} failed: {e:?}"));
        assert_eq!(lvl.width, want_w, "width mismatch input {idx}");
        assert_eq!(lvl.height, want_h, "height mismatch input {idx}");
        assert_eq!(lvl.material_id.len(), (want_w * want_h) as usize);
        assert_eq!(fnv1a(&lvl.material_id), want_hash, "material hash mismatch input {idx}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
