//! Differential test for the fixed-point module against the C++ oracle.
//!
//! The pattern (the same in every `*_golden.rs`): the C++ dumper has already run
//! the matching C++ functions and written the results to `golden/fixed.txt`.
//! Here we run the ported Rust code on the *exact same* inputs and compare line
//! by line. If every value matches, the port is proven bit-identical to the
//! original.
//!
//! `FIXED_INPUTS` must be identical to the dumper's `kFixedInputs` — otherwise
//! we would be comparing apples and oranges. The list deliberately includes edge
//! cases: negative numbers, exactly ±1.0 (±65536), and values that make `itof`
//! overflow.

use sim_core::fixed::{ftoi, itof};

const FIXED_INPUTS: [i32; 12] = [
    -2000000, -65537, -65536, -100, -1, 0, 1, 100, 65535, 65536, 65537, 2000000,
];

#[test]
fn fixed_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/fixed.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    for v in FIXED_INPUTS {
        for expected in [itof(v), ftoi(v), ftoi(itof(v))] {
            let want: i32 = lines.next().unwrap().parse().unwrap();
            assert_eq!(expected, want, "mismatch for input {v}");
        }
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
