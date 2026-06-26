use sim_core::math::vector_length;

const CASES: [(i32, i32); 7] = [
    (0, 0), (3, 4), (100, 0), (0, 255), (1000, 1000), (-1234, 5678), (32767, 32767),
];

#[test]
fn vector_length_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sqrt.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    for (x, y) in CASES {
        let want: i32 = lines.next().unwrap().parse().unwrap();
        assert_eq!(vector_length(x, y), want, "mismatch for ({x},{y})");
    }
}
