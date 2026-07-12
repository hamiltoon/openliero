//! Library surface of the `game` crate (Slice 4a, T3).
//!
//! `game` is a Bevy **binary** (`main.rs`), and a binary crate cannot be
//! consumed by an integration test under `tests/`. To let the headless
//! pass-through determinism gate (`tests/passthrough.rs`) drive the **real**
//! [`input::InputSource`] — not a re-implementation of the recorded-input read —
//! this thin library re-exports only the Bevy-free-testable input core. All
//! binary logic (the Bevy app, the CPU blit, the tick/render loop) stays in
//! `main.rs`, which uses this module via `game::input`.
pub mod input;
