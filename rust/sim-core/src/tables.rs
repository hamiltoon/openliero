//! # Precomputed sin/cos table
//!
//! Port of `PrecomputeTables` (and the helper struct `FP`) in
//! `src/game/math.cpp`.
//!
//! The game divides a full turn into **128 angle steps** and looks up directions
//! in a table instead of computing them at runtime (fast, and above all
//! *deterministic*). Each entry is a [`Vec2`] — a direction/unit vector for the
//! angle in 16.16 fixed point (so `1.0` becomes `65536`). The function only
//! computes the *sine* of each angle and reuses it both as `x` and (a quarter
//! turn later) as `y`, since cosine is sine shifted by 90°.
//!
//! ## Why an integer Taylor series instead of `f64::sin`?
//! If we built the table with floating-point `sin`/`cos`, the result could
//! differ slightly between platforms (different libm implementations) — and then
//! determinism would already be broken at startup. So the sine is computed with
//! a **Taylor series in pure integer math** (i64): `sin(x) = x − x³/3! + x⁵/5! −
//! …`. The result is bit-identical everywhere.

use crate::vec::Vec2;

/// A small sliding fixed-point number: the value is `s`, and `bits` says how many
/// bits of `s` are fractional. This lets the Taylor series keep high precision
/// without overflowing i64 — `reduce` scales `s` back down when it grows too big.
struct Fp {
    s: i64,
    bits: i32,
}

impl Fp {
    /// Scale `s` down until it fits within `tobits` bits (adjusting the `bits`
    /// counter accordingly). Keeps the intermediate results from overflowing.
    fn reduce(&mut self, tobits: i32) {
        let lim: i64 = 1i64 << tobits;
        while self.s < (-lim - 1) || self.s > lim {
            self.s >>= 1;
            self.bits -= 1;
        }
    }
    /// Return `s` rescaled to have exactly `tobits` fractional bits — i.e. the
    /// term's contribution expressed in a common fixed-point format so it can be
    /// summed with the other Taylor terms.
    fn reducedfrac(&self, tobits: i32) -> i64 {
        let mut rs = self.s;
        let mut rbits = self.bits;
        while rbits > 60 {
            rs >>= 1;
            rbits -= 1;
        }
        rs << (tobits - rbits)
    }
}

/// Build the whole table: 128 entries with (cos, sin) in 16.16 fixed point.
pub fn precompute_cossin() -> [Vec2; 128] {
    const SCALE_BITS: i32 = 28;
    const SCALE: i32 = 13176795; // (2π / 128) << 28 — one angle step in fixed point
    let mut table = [Vec2::zero(); 128];
    for i in 0..128i32 {
        // Compute sin(angle) for angle i = i·(2π/128) using the Taylor series.
        let mut rf: i64 = 0; // the sum (the sine value we are building up)
        let mut c: i32 = -1; // sign flipper: −1, +1, −1, … for − x³/3! + x⁵/5! …
        let xf: i32 = i * SCALE; // the angle in fixed point
        let mut num = Fp {
            s: xf as i64,
            bits: SCALE_BITS,
        }; // "num" is the running term x^(2k+1)/(2k+1)!
        let mut t: i32 = 1;
        while t < 26 {
            // Add the current term (with sign) to the sum.
            // `wrapping_*` is required: the intermediate terms actually overflow
            // i64, and C++ relies on two's complement wrap. Since modular
            // addition is associative, the final sum still comes out bit-exact.
            rf = rf.wrapping_add((c as i64).wrapping_mul(num.reducedfrac(60)));

            // Advance to the next term: divide by (t+1) and (t+2) and multiply by
            // x² (here ×x is done twice, one per `t += 1`). This builds up
            // x^(2k+1)/(2k+1)! step by step. `reduce` keeps `s` in check.
            t += 1;
            num.s /= t as i64;
            num.reduce(31);
            num.s = num.s.wrapping_mul(xf as i64);
            num.bits += SCALE_BITS;

            t += 1;
            num.s /= t as i64;
            num.reduce(31);
            num.s = num.s.wrapping_mul(xf as i64);
            num.bits += SCALE_BITS;

            c = -c; // flip the sign for the next term
        }
        // Convert the sum from the Taylor precision (60 fractional bits) down to
        // 16.16 fixed point, with correct rounding (add half a unit before the
        // shift). Here rf is already ~2⁶⁰ and does not overflow.
        const SHIFT: i32 = 60 - 16;
        rf += 1i64 << (SHIFT - 1);
        let r = (rf >> SHIFT) as i32;

        // r = sin(angle_i). Store it as the x component at index i, and as the y
        // component 32 steps away (32 steps = 90° = a quarter turn). Thanks to
        // the 90° offset between sin and cos, computing the sine once per angle is
        // enough — it fills both x here and y at the offset index.
        table[i as usize].x = r;
        table[((i + 32) & 0x7f) as usize].y = r;
    }
    table
}
