//! # Integer square root and vector length
//!
//! Port of `Sqr` and `VectorLength` in `src/game/math.cpp`.
//!
//! Used for distances in the game (e.g. "is the bullet within X pixels of the
//! worm?"). Since determinism forbids floating point, we cannot use `f64::sqrt`
//! — instead the square root is computed with pure integer math.

/// Integer square root, rounded down (`isqrt(17) == 4`).
///
/// This is the classic "bitwise" sqrt algorithm: it builds the answer one bit at
/// a time, from the highest possible power of four downward, testing for each
/// bit whether it fits without exceeding `op`. No floating point at all.
///
/// Bit-for-bit port of C++ `Sqr()` — the loop structure is intentionally
/// identical so the result is exactly the same.
pub fn isqrt(mut op: u32) -> u32 {
    let mut res: u32 = 0;
    let mut one: u32 = 1 << 30; // highest power of four that fits in a u32
    // Find the largest power of four that is <= op (start of the bit search).
    while one > op {
        one >>= 2;
    }
    // Work downward bit by bit, assembling the root in `res`.
    while one != 0 {
        if op >= res + one {
            op -= res + one;
            res += 2 * one;
        }
        res >>= 1;
        one >>= 2;
    }
    res
}

/// Length of the vector (x, y): `sqrt(x² + y²)`, rounded down.
///
/// `x² + y²` is computed as `i32` (with `wrapping_*` to match C++'s `int`
/// arithmetic on overflow) and then reinterpreted as `u32` before the root is
/// taken — exactly as C++ implicitly converts `int` → `uint32_t` in the call to
/// `Sqr`.
pub fn vector_length(x: i32, y: i32) -> i32 {
    let sum = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
    isqrt(sum as u32) as i32
}
