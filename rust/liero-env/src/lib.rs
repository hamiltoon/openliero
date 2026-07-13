//! `liero-env` — the RL harness's Rust core (design
//! `docs/superpowers/specs/2026-07-13-liero-rs-rl-harness-pettingzoo-design.md`).
//!
//! T1 lands the **pure-Rust** [`LieroEnv`]: a thin, deterministic consumer of the
//! Bevy-free `sim`/`scenario` surfaces exposing the classic RL episode contract
//! (`reset` / `step` / `terminated` / `truncated`) over the real Liero tick. T2
//! adds the obs/action/reward value-mappers; T3 (`python`) exposes the raw
//! [`python::RawEnv`] PyO3 surface Python's PettingZoo/Gymnasium glue builds on.
//!
//! The crate is `cdylib` + `rlib` (T0 §3): the `rlib` is what `cargo test
//! --workspace` runs; the `cdylib` (built by maturin with the `extension-module`
//! feature) is the importable Python extension. The `python` binding compiles in
//! both configurations — with `extension-module` OFF (the workspace-test build)
//! PyO3 links libpython, so it type-checks under `cargo test` too; only the
//! importable `.so` needs the feature ON (T0 §3 membership decision).

pub mod action;
pub mod env;
pub mod obs;
pub mod python;
pub mod record;
pub mod reward;
#[cfg(test)]
mod test_fixtures;

pub use action::{decode as decode_action, encode as encode_action};
pub use env::{LieroEnv, StepOutcome, N_WORMS};
pub use obs::{observe, OBS_DIM};
pub use python::RawEnv;
pub use record::Recording;
pub use reward::{reward, RewardConfig, WormSnapshot};
