use sim_core::tables::precompute_cossin;

#[test]
fn cossin_table_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/cossin.txt"
    ))
    .unwrap();
    let table = precompute_cossin();
    let mut lines = golden.lines();
    for (i, entry) in table.iter().enumerate() {
        let line = lines.next().unwrap();
        let mut it = line.split_whitespace();
        let wx: i32 = it.next().unwrap().parse().unwrap();
        let wy: i32 = it.next().unwrap().parse().unwrap();
        assert_eq!(entry.x, wx, "x mismatch at index {i}");
        assert_eq!(entry.y, wy, "y mismatch at index {i}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
