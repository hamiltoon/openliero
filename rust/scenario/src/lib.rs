//! The Bevy-free scenario crate: the Slice-2 scenario **parser** (moved verbatim
//! from `oracle-tests/src/scenario.rs`) plus the tick-0 **loader** factored out
//! of the T8 harness `render_slice3b_common::build()`.
//!
//! Deps are `sim`/`assets`/`render`/`sim-core` — all Bevy-free — so `game` (3c)
//! and the headless PNG CLI (3d) can consume the load path without depending on
//! `oracle-tests`. `oracle-tests` re-exports this crate (`pub use scenario;`) so
//! the existing `oracle_tests::scenario::Scenario` references compile unchanged.

pub mod assets;
pub mod loader;
pub mod parser;

pub use loader::{load, HudLabels, Loaded, SceneData};
pub use parser::{Scenario, ScenarioWorm};
