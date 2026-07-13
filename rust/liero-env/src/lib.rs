//! `liero-env` — the RL harness's Rust core (design
//! `docs/superpowers/specs/2026-07-13-liero-rs-rl-harness-pettingzoo-design.md`).
//!
//! T1 lands the **pure-Rust** [`LieroEnv`]: a thin, deterministic consumer of the
//! Bevy-free `sim`/`scenario` surfaces exposing the classic RL episode contract
//! (`reset` / `step` / `terminated` / `truncated`) over the real Liero tick. No
//! PyO3 yet — the `RawEnv` binding, obs/action/reward mappers, and the eval
//! recorder are later tasks (T2/T3/T5) that build on this core.
//!
//! The crate is `cdylib` + `rlib` (T0 §3): the `rlib` is what `cargo test
//! --workspace` runs; the `cdylib` (built by maturin with the `extension-module`
//! feature) is the importable Python extension.

pub mod action;
pub mod env;
pub mod obs;
pub mod reward;
#[cfg(test)]
mod test_fixtures;

pub use action::{decode as decode_action, encode as encode_action};
pub use env::{LieroEnv, StepOutcome, N_WORMS};
pub use obs::{observe, OBS_DIM};
pub use reward::{reward, RewardConfig, WormSnapshot};
