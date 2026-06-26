//! # sim-core — Liero-rs deterministic simulation core
//!
//! This is the "source of truth" for the game: all the logic that decides how
//! the world evolves tick by tick. It deliberately has **no** dependency on
//! Bevy, rendering, audio or the standard library's RNG — and above all, **no
//! floating point**.
//!
//! ## Why no floating point?
//! Liero relies on *determinism*: the same starting state plus the same input
//! must produce exactly the same result on any machine, every time. That is
//! what makes replays and rollback netplay possible. `f32`/`f64` can produce
//! slightly different results across CPUs/compilers — fatal when tick 1000 has
//! to be *bit-for-bit* identical everywhere. So everything here is integer math.
//!
//! ## How do we know it is correct?
//! Each module below is a port of the matching C++ code in `src/game/`, and is
//! tested against "golden vectors" generated from the C++ original (see the
//! `oracle-tests` crate). If Rust matches C++ bit-for-bit, the port is proven
//! correct.

pub mod fixed; // 16.16 fixed-point numbers (mirrors `fixed`/Itof/Ftoi in math.hpp)
pub mod math; // integer square root and vector length (Sqr/VectorLength)
pub mod rng; // deterministic MT19937 random number generator (rand.hpp)
pub mod tables; // precomputed sin/cos table (PrecomputeTables)
pub mod vec; // 2D integer vector (IVec2)
