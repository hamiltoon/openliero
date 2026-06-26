//! # 2D integer vector
//!
//! Port of `IVec2` in `src/game/math/rect.hpp`.
//!
//! This is the game's fundamental "position" and "velocity". The `x`/`y`
//! components are usually [fixed-point](crate::fixed) values — so a moving worm
//! stores its position as a `Vec2` of 16.16 numbers.
//!
//! The operations mirror exactly what C++'s `IVec2` offers: component-wise
//! addition/subtraction, and multiplication/division by a **scalar** (an
//! integer). Note that there is deliberately *no* vector·vector multiplication —
//! the original game only uses `vector * speed / 100` patterns, so we port only
//! what is actually needed (YAGNI).

/// A 2D vector of integers (usually fixed-point). `Copy` because it is small and
/// gets passed around everywhere in the simulation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

impl Vec2 {
    #[inline]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The zero vector. Mirrors `IVec2::Zero()`.
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }

    // Every arithmetic operation uses `wrapping_*`. That matches C++'s two's
    // complement wrap on overflow and guarantees the port is bit-identical even
    // for inputs that overflow an `i32` — instead of panicking in Rust's debug
    // build. Determinism over "safety" here: we want the *exact* same result as
    // the original, even when the original wraps around.

    #[inline]
    pub fn add(self, r: Vec2) -> Vec2 {
        Vec2::new(self.x.wrapping_add(r.x), self.y.wrapping_add(r.y))
    }
    #[inline]
    pub fn sub(self, r: Vec2) -> Vec2 {
        Vec2::new(self.x.wrapping_sub(r.x), self.y.wrapping_sub(r.y))
    }
    /// Multiply both components by a scalar (e.g. `direction * speed`).
    #[inline]
    pub fn mul(self, s: i32) -> Vec2 {
        Vec2::new(self.x.wrapping_mul(s), self.y.wrapping_mul(s))
    }
    /// Integer division by a scalar. Truncates toward zero (like C++ `int`
    /// division), which differs from the arithmetic shift in
    /// [`ftoi`](crate::fixed::ftoi).
    #[inline]
    pub fn div(self, s: i32) -> Vec2 {
        Vec2::new(self.x.wrapping_div(s), self.y.wrapping_div(s))
    }
    #[inline]
    pub fn neg(self) -> Vec2 {
        Vec2::new(self.x.wrapping_neg(), self.y.wrapping_neg())
    }
}
