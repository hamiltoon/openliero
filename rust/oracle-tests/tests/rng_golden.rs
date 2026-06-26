//! Differential test for the MT19937 RNG against the C++ oracle. See
//! `fixed_golden.rs` for the golden pattern. The sequence — 10000 raw values,
//! then bounded values for several maxes, then a reseed — must match
//! `golden/rng.txt` exactly, and in the same order, since each draw advances the
//! shared generator state.

use sim_core::rng::Rand;

#[test]
fn rng_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/rng.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    let mut next_golden = || -> u32 { lines.next().unwrap().parse().unwrap() };

    let mut r = Rand::new(); // seed 0x1337
    for i in 0..10000 {
        assert_eq!(r.next_u32(), next_golden(), "raw mismatch at {i}");
    }
    for m in [1u32, 2, 7, 100, 128, 65536] {
        for i in 0..100 {
            assert_eq!(r.bound(m), next_golden(), "bound({m}) mismatch at {i}");
        }
    }
    r.seed(42);
    for i in 0..100 {
        assert_eq!(r.next_u32(), next_golden(), "reseed mismatch at {i}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
