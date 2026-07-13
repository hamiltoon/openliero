"""Python smoke for the T5 eval-recording surface (design §1.5, plan T5).

Records a short scripted episode from Python (both through the raw `RawEnv`
and the `LieroGymEnv` wrapper), saves it, and verifies the file exists and
genuinely parses through the real Rust grammar (`liero_env.scenario_parses`,
not a hand-rolled text heuristic). The byte-for-byte "replays to an identical
checksum trace" assertion lives in the Rust
`recorded_episode_round_trips_through_replay` gate (`src/env.rs`), which does
not cross the FFI boundary — this smoke only proves the Python-side plumbing
(start/save, the "no active recording" error path, and the wrapper passthrough)
actually reaches it.

No pytest dependency (the pinned venv has none — constraints.txt owns the
deps): each `test_*` is a plain function, run by the `__main__` block below.
Invoke with the project venv after `maturin develop`:

    rust/liero-env/.venv/bin/python rust/liero-env/python/tests/test_t5_recording_smoke.py
"""

import os
import random
import tempfile

import liero_env
from liero_env import LieroGymEnv, RawEnv


def _action_stream(n, seed=555):
    rng = random.Random(seed)
    return [
        (
            [rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)],
            [rng.randint(0, 1) for _ in range(liero_env.N_ACTION_BITS)],
        )
        for _ in range(n)
    ]


def _tmp_path(suffix):
    fd, path = tempfile.mkstemp(prefix=f"liero_env_t5_smoke_{suffix}_", suffix=".txt")
    os.close(fd)
    os.remove(path)  # RawEnv.save_recording must create it, not just truncate it.
    return path


def test_save_recording_without_start_raises():
    """`save_recording` before `start_recording` must raise, not silently write a
    garbage/empty file."""
    env = RawEnv(max_ticks=10)
    env.reset(1)
    path = _tmp_path("no_start")
    try:
        raised = False
        try:
            env.save_recording(path)
        except Exception:
            raised = True
        assert raised, "save_recording must raise without an active recording"
        assert not os.path.exists(path), "no file must be written when not recording"
    finally:
        if os.path.exists(path):
            os.remove(path)


def test_is_recording_flag():
    """`is_recording` reflects `start_recording` and is cleared by `reset`."""
    env = RawEnv(max_ticks=200)
    env.reset(1)
    assert not env.is_recording
    env.start_recording()
    assert env.is_recording
    env.reset(2)
    assert not env.is_recording, "reset must clear an in-progress recording"


def test_raw_env_record_save_and_parses():
    """Record a scripted episode via `RawEnv`, save it, and verify the file
    exists and parses through the real scenario grammar."""
    env = RawEnv(max_ticks=200)
    env.reset(42)
    env.start_recording()
    assert env.is_recording

    actions = _action_stream(30)
    for a0, a1 in actions:
        env.step(a0, a1)

    path = _tmp_path("raw")
    try:
        env.save_recording(path)
        assert os.path.exists(path), "save_recording must create the file"
        with open(path) as f:
            text = f.read()
        assert liero_env.scenario_parses(text), "saved recording must parse as a scenario"
        assert f"ticks {len(actions)}" in text
    finally:
        if os.path.exists(path):
            os.remove(path)


def test_gym_wrapper_record_save_and_parses():
    """The `LieroGymEnv` passthrough (`start_recording`/`save_recording`) reaches
    the same Rust surface end-to-end."""
    env = LieroGymEnv(max_ticks=200)
    env.reset(seed=7)
    env.start_recording()

    for _ in range(20):
        _obs, _r, term, trunc, _info = env.step(env.action_space.sample())
        if term or trunc:
            break

    path = _tmp_path("gym")
    try:
        env.save_recording(path)
        assert os.path.exists(path)
        with open(path) as f:
            text = f.read()
        assert liero_env.scenario_parses(text), "saved recording must parse as a scenario"
    finally:
        if os.path.exists(path):
            os.remove(path)


TESTS = [
    test_save_recording_without_start_raises,
    test_is_recording_flag,
    test_raw_env_record_save_and_parses,
    test_gym_wrapper_record_save_and_parses,
]


def main():
    failures = 0
    for t in TESTS:
        try:
            t()
        except Exception as exc:  # noqa: BLE001 (smoke runner: report every failure)
            failures += 1
            print(f"FAIL {t.__name__}: {type(exc).__name__}: {exc}")
        else:
            print(f"ok   {t.__name__}")
    if failures:
        raise SystemExit(f"\n{failures}/{len(TESTS)} T5 smoke tests failed")
    print(f"\nall {len(TESTS)} T5 smoke tests passed")


if __name__ == "__main__":
    main()
