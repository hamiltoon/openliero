//! The deterministic Liero simulation core.
//!
//! This crate is pure Rust with no Bevy and no floating point: it advances the
//! game one fixed-step tick at a time using only integer / fixed-point math, so
//! the simulation is bit-for-bit reproducible across platforms. The canonical
//! state hash used for desync detection and oracle comparison lives here too.
//!
//! It depends only on [`sim_core`] (dep-free determinism primitives) and
//! [`assets`] (data parsers); rendering, audio, input, and networking live in
//! other crates layered on top.

pub mod blit;
pub mod control;
pub mod hash;
pub mod nobject;
pub mod physics;
pub mod pool;
pub mod state;
pub mod weapon;
