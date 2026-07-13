//! The PyO3 binding: `RawEnv`, the raw single-env surface Python's PettingZoo /
//! Gymnasium glue (T4) sits on top of (design §6 "division of labour"). It is a
//! thin wrapper over the pure-Rust [`LieroEnv`] core (T1) plus the obs (T2),
//! action (T2), and reward (T2) value-mappers — the hot path stays in Rust and
//! only compact `f32` numpy arrays cross the FFI boundary (design §8).
//!
//! ## Surface (design §5/§6, plan T3)
//! - `RawEnv(max_ticks=3000, frame_skip=1, reward_config=None)` — construct.
//!   `reward_config` is an optional `dict` of the [`RewardConfig`] weights
//!   (`w_damage_dealt`/`w_damage_taken`/`w_kill`/`w_death`/`w_time`); missing
//!   keys keep the [`RewardConfig::default`] value, so reward tuning/annealing is
//!   a Python-side experiment with no recompile (design §4).
//! - `reset(seed) -> (obs0, obs1)` — two `float32` arrays of length [`OBS_DIM`].
//! - `step(action0, action1) -> ((obs0, obs1), (r0, r1), (term0, term1),
//!   truncated, info)` — the raw per-agent 5-tuple T4's `ParallelEnv` re-shapes
//!   into per-agent dicts. `action{0,1}` is any length-7 sequence of 0/1 (a
//!   Python list, tuple, or a `MultiBinary(7)` numpy sample — accepted by
//!   truthiness, so `int8`/`bool`/`int` dtypes all work). `info["checksum"]` is
//!   the per-tick [`wide_rollback_checksum`](crate::env) determinism fingerprint.
//! - `observe(agent) -> obs` — re-extract one agent's current observation.
//! - `ticks` / `checksum` — read-only accessors mirroring the core.
//!
//! ## GIL discipline
//! [`RawEnv::step`] releases the GIL (`Python::detach`, pyo3 0.29's
//! `allow_threads`) around the pure-Rust sim tick loop: the actions are decoded
//! and the pre-step reward snapshot is taken while the GIL is held, then the
//! (Python-object-free) `process_frame` × `frame_skip` advance runs GIL-free so
//! other Python threads / parallel envs progress, and the GIL is re-acquired only
//! to build the numpy obs arrays and the info dict. Nothing that touches a Python
//! object crosses the `detach` boundary (the `Ungil` bound enforces it).
//!
//! ## No raw state mutation
//! The binding exposes **no** mutable access to `SimState` — obs/reward read
//! through the core's immutable accessor only (the core's `state_mut` is
//! `#[cfg(test)]`, so it cannot even be named here). The determinism contract
//! (fixed→float one-directional, sim RNG untouched from Python) holds by
//! construction.

use numpy::{IntoPyArray, PyArray1};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::action::{decode, N_ACTION_BITS};
use crate::env::{LieroEnv, N_WORMS};
use crate::obs::{observe as extract_obs, OBS_DIM};
use crate::reward::{reward, snapshot_all, RewardConfig};
use sim::state::ControlState;

/// A pair of per-agent `float32` observation arrays (worm 0, worm 1).
type ObsPair<'py> = (Bound<'py, PyArray1<f32>>, Bound<'py, PyArray1<f32>>);

/// Read a length-`N_ACTION_BITS` action from any Python sequence (list, tuple,
/// or a `MultiBinary(7)` numpy sample) into a [`ControlState`]. Each element is
/// read by **truthiness** (`0`/`1`, `False`/`True`, `np.int8`, `np.bool_` all
/// work) — this is the robust cross-dtype path, avoiding a rigid
/// `PyReadonlyArray` dtype match. Must be called with the GIL held (it reads
/// Python objects), i.e. never inside `allow_threads`.
fn extract_action(obj: &Bound<'_, PyAny>) -> PyResult<ControlState> {
    let mut bits = [false; N_ACTION_BITS as usize];
    for (n, bit) in bits.iter_mut().enumerate() {
        *bit = obj.get_item(n)?.is_truthy()?;
    }
    Ok(decode(bits))
}

/// Build a [`RewardConfig`] from an optional Python `dict`, defaulting each
/// missing key to [`RewardConfig::default`]. Unknown keys are ignored (forward
/// compatible with Python-side experiments).
fn reward_config_from_dict(d: &Bound<'_, PyDict>) -> PyResult<RewardConfig> {
    let mut cfg = RewardConfig::default();
    if let Some(v) = d.get_item("w_damage_dealt")? {
        cfg.w_damage_dealt = v.extract()?;
    }
    if let Some(v) = d.get_item("w_damage_taken")? {
        cfg.w_damage_taken = v.extract()?;
    }
    if let Some(v) = d.get_item("w_kill")? {
        cfg.w_kill = v.extract()?;
    }
    if let Some(v) = d.get_item("w_death")? {
        cfg.w_death = v.extract()?;
    }
    if let Some(v) = d.get_item("w_time")? {
        cfg.w_time = v.extract()?;
    }
    Ok(cfg)
}

/// The raw, single-env RL surface exposed to Python (design §6). Thin over the
/// pure-Rust [`LieroEnv`]; see the module docs for the full contract.
#[pyclass]
pub struct RawEnv {
    env: LieroEnv,
    cfg: RewardConfig,
}

impl RawEnv {
    /// Both agents' current observations as `float32` numpy arrays. The
    /// extractor runs Rust-side (design §8); only the two compact arrays cross
    /// the boundary.
    fn obs_pair<'py>(&self, py: Python<'py>) -> ObsPair<'py> {
        let o0 = extract_obs(self.env.state(), 0).into_pyarray(py);
        let o1 = extract_obs(self.env.state(), 1).into_pyarray(py);
        (o0, o1)
    }
}

#[pymethods]
impl RawEnv {
    /// Construct over the embedded `rl_default_match` fixture. `frame_skip` is
    /// clamped to `>= 1` by the core. `reward_config` overrides individual
    /// [`RewardConfig`] weights; omitted keys keep the default.
    #[new]
    #[pyo3(signature = (max_ticks=LieroEnv::DEFAULT_MAX_TICKS, frame_skip=1, reward_config=None))]
    fn new(
        max_ticks: u32,
        frame_skip: u32,
        reward_config: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let cfg = match reward_config {
            Some(d) => reward_config_from_dict(&d)?,
            None => RewardConfig::default(),
        };
        Ok(RawEnv {
            env: LieroEnv::new(max_ticks, frame_skip),
            cfg,
        })
    }

    /// Reset to a fresh tick-0 episode with `seed` and return both agents'
    /// observations. Determinism is per seed (design §5): same seed + same
    /// action stream ⇒ identical `checksum` trace; different seed diverges.
    fn reset<'py>(&mut self, py: Python<'py>, seed: u32) -> ObsPair<'py> {
        self.env.reset(seed);
        self.obs_pair(py)
    }

    /// Advance one env step. Returns `((obs0, obs1), (r0, r1), (term0, term1),
    /// truncated, info)` — see the module docs. The sim tick loop runs with the
    /// GIL released.
    #[allow(clippy::type_complexity)]
    fn step<'py>(
        &mut self,
        py: Python<'py>,
        action0: &Bound<'py, PyAny>,
        action1: &Bound<'py, PyAny>,
    ) -> PyResult<(
        ObsPair<'py>,
        (f32, f32),
        (bool, bool),
        bool,
        Bound<'py, PyDict>,
    )> {
        // GIL held: decode actions + capture the pre-step reward baseline.
        let actions = [extract_action(action0)?, extract_action(action1)?];
        let prev = snapshot_all(self.env.state());
        let cfg = self.cfg;

        // GIL released: the pure-Rust `process_frame` × frame_skip advance holds
        // no Python objects, so other Python threads / parallel envs run.
        // (`Python::detach` is pyo3 0.29's `allow_threads`.)
        let outcome = py.detach(|| self.env.step(&actions));

        // GIL re-acquired: reward from (prev, post-step state), then obs + info.
        let rewards = (
            reward(&prev, self.env.state(), 0, &cfg),
            reward(&prev, self.env.state(), 1, &cfg),
        );
        let obs = self.obs_pair(py);
        let info = PyDict::new(py);
        info.set_item("checksum", outcome.checksum)?;

        Ok((
            obs,
            rewards,
            (outcome.terminated[0], outcome.terminated[1]),
            outcome.truncated,
            info,
        ))
    }

    /// Re-extract `agent`'s current observation without stepping.
    fn observe<'py>(&self, py: Python<'py>, agent: usize) -> Bound<'py, PyArray1<f32>> {
        extract_obs(self.env.state(), agent).into_pyarray(py)
    }

    /// Sim frames processed since the last `reset`.
    #[getter]
    fn ticks(&self) -> u32 {
        self.env.ticks()
    }

    /// The determinism fingerprint of the most recent `reset`/`step`.
    #[getter]
    fn checksum(&self) -> u32 {
        self.env.checksum()
    }
}

/// The importable extension module `liero_env` (maturin builds it with the
/// `extension-module` feature on; the module name must match the library name).
/// Exposes [`RawEnv`] plus the layout constants Python needs to size spaces
/// (`OBS_DIM`, `N_ACTION_BITS`, `N_WORMS`).
#[pymodule]
fn liero_env(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RawEnv>()?;
    m.add("OBS_DIM", OBS_DIM)?;
    m.add("N_ACTION_BITS", N_ACTION_BITS)?;
    m.add("N_WORMS", N_WORMS)?;
    Ok(())
}
