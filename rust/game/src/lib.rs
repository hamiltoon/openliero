//! Library surface of the `game` crate (Slice 4a, T3).
//!
//! `game` is a Bevy **binary** (`main.rs`), and a binary crate cannot be
//! consumed by an integration test under `tests/`. To let the headless
//! pass-through determinism gate (`tests/passthrough.rs`) drive the **real**
//! [`input::InputSource`] — not a re-implementation of the recorded-input read —
//! this thin library re-exports the Bevy-free-testable cores: the input sampler
//! (`input`), since Slice 4d T2 the live viewport stepping (`viewport_step`
//! — the ordering-critical shake/banner/flash split around `process_frame`, so
//! its ordering can be gated headlessly), and since Step 4½a-1 T10 the
//! post-weapon-selection match lifecycle (`match_flow` — the Bevy-free
//! `LocalController` game/game-ended tail). All Bevy binary logic (the app, the
//! CPU blit, the tick/render loop) stays in `main.rs`, which uses these modules
//! via `game::input` / `game::viewport_step` / `game::match_flow`.
pub mod audio;
pub mod input;
pub mod match_flow;
pub mod viewport_step;
