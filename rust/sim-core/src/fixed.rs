//! # Fixed-point numbers (16.16)
//!
//! Port of `Itof`/`Ftoi` in `src/game/math.hpp`.
//!
//! Instead of decimals, the game stores positions and velocities as integers
//! where the **low 16 bits are the fractional part**. So `1.0` is stored as
//! `1 << 16 = 65536`, and `1.5` as `98304`. This is *fixed point* (16.16 = 16
//! integer bits, 16 fractional bits packed into one `i32`).
//!
//! Why? See the [`crate`] docs: integers compute identically on every platform,
//! which floats do not. Determinism requires it.

/// A fixed-point number. Just an `i32` — the alias exists for readability, so a
/// signature reads "this is 16.16 fixed point", not a plain integer.
pub type Fixed = i32;

/// Number of fractional bits. So `1.0` is `1 << 16`.
pub const FRAC_BITS: u32 = 16;

/// Integer → fixed-point: shift up 16 places (`v * 65536`).
///
/// `wrapping_shl` is used (not a plain `<<`) to match C++'s behaviour: for large
/// `v` the result wraps around in two's complement instead of panicking. This is
/// deliberate — the port must stay bit-identical even on overflow.
#[inline]
pub fn itof(v: i32) -> Fixed {
    v.wrapping_shl(FRAC_BITS)
}

/// Fixed-point → integer: shift down 16 places (the integer part).
///
/// `>>` on an `i32` is an *arithmetic* shift in Rust — it preserves the sign and
/// rounds toward negative infinity (e.g. `-1 >> 16 == -1`), exactly like C++'s
/// signed `>>`. (That differs from division, which truncates toward zero.)
#[inline]
pub fn ftoi(v: Fixed) -> i32 {
    v >> FRAC_BITS
}
