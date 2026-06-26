//! Differential test for `Vec2` against the C++ oracle (`IVec2`). See
//! `fixed_golden.rs` for an explanation of the golden pattern. Here we run
//! add/sub/mul/div on the same cases as the dumper and compare both components
//! against `golden/vec.txt`.

use sim_core::vec::Vec2;

// (ax, ay, bx, by, s) — must be identical to kVecCases in the dumper, otherwise
// the lines in the golden file would not line up.
const CASES: [(i32, i32, i32, i32, i32); 5] = [
    (0, 0, 0, 0, 1),
    (100, -50, 7, 9, 3),
    (-65536, 65536, 100, -100, 100),
    (123456, -789012, -3, 5, 7),
    (2000000, -2000000, 1, 1, 2),
];

#[test]
fn vec_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/vec.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    let mut next = || -> i32 { lines.next().unwrap().parse().unwrap() };
    for (ax, ay, bx, by, s) in CASES {
        let a = Vec2::new(ax, ay);
        let b = Vec2::new(bx, by);
        for v in [a.add(b), a.sub(b), a.mul(s), a.div(s)] {
            assert_eq!(v.x, next());
            assert_eq!(v.y, next());
        }
    }
}
