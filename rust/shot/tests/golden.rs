//! Golden-faithfulness test for the headless screenshot CLI.
//!
//! `shot::render_scenario` is a deliberate CLI-local copy of the Slice-3b
//! frame-hash harness (`render_slice3b_common::run`): it drives a 3b scenario
//! tick-by-tick, renders the full world per tick, and folds the per-tick frame
//! hashes into the FNV `total` accumulator. The value of a copy is only as good
//! as the guard against drift — this test LOCKS the CLI render against the
//! committed C++ sidecar goldens for two scenarios:
//!
//! - `blood` — the plain `render_snapshot` path (no injection), so it pins the
//!   base render + hashing + state-hash agreement.
//! - `shake` — the ONE path the CLI copy adds over that base: the per-tick
//!   `render_shake` / `render_flash` injection in `render_tick`. If that
//!   injection ever diverges from the dumper, `shake` goes red while `blood`
//!   stays green, localising the drift.
//!
//! For each scenario every per-tick `frame_hash` AND `state_hash` is asserted
//! equal to the golden column, the recomputed `total` accumulator is asserted
//! equal to the golden `total` line, and the row count is asserted equal to the
//! scenario's `ticks + 1`. The goldens are the C++ dumper's committed output;
//! a red here means the CLI render has DRIFTED — fix the render, never the test.

use std::path::Path;

use render::hash::{FNV_OFFSET, FNV_PRIME};
use scenario::Scenario;

/// Original-Liero TC data root (relative to this crate's manifest).
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// One parsed sidecar frame line: `<tick> <frame_hash_hex16> <state_hash_hex8>`.
struct GoldenLine {
    tick: u32,
    frame_hash: u64,
    state_hash: u32,
}

/// Local copy of the `render_slice3b_common::parse_frames` grammar — the test
/// crate cannot see that harness module. Parses a frame sidecar into
/// `(per-tick lines, total_count, total_accumulator)`; blank / `#` lines skipped.
fn parse_frames(text: &str) -> (Vec<GoldenLine>, u32, u64) {
    let mut lines = Vec::new();
    let mut total_n = 0u32;
    let mut total_acc = 0u64;
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        let head = it.next().unwrap();
        if head == "total" {
            total_n = it.next().unwrap().parse().unwrap();
            total_acc = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        } else {
            let tick: u32 = head.parse().unwrap();
            let frame_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            let state_hash = u32::from_str_radix(it.next().unwrap(), 16).unwrap();
            lines.push(GoldenLine {
                tick,
                frame_hash,
                state_hash,
            });
        }
    }
    (lines, total_n, total_acc)
}

/// Read a `render_slice3b_<name><suffix>` golden from the `oracle-tests` crate.
fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/../oracle-tests/golden/render_slice3b_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// Drive `name` through the CLI render path and lock every column + the folded
/// `total` against the committed C++ sidecar golden.
fn assert_golden_faithful(name: &str) {
    // Scenario text is loaded exactly as `shot::run` does: read the sidecar
    // `_scenario.txt` and hand it to the `scenario` crate's `Scenario::parse`.
    let scenario_text = read_golden(name, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "{name}: 3b scenarios all use seed 42");

    let (golden, total_n, total_acc) = parse_frames(&read_golden(name, ".txt"));
    assert_eq!(
        golden.len(),
        (scenario.ticks + 1) as usize,
        "{name}: one golden line per tick 0..=ticks"
    );
    assert_eq!(
        total_n,
        scenario.ticks + 1,
        "{name}: golden total count == ticks + 1"
    );

    // Drive the FULL scenario through the CLI render path. Empty `wanted` => no
    // PNG encode; scale is irrelevant with nothing captured.
    let frames = shot::render_scenario(Path::new(TC_ROOT), &scenario, scenario.ticks, &[], 3);
    assert_eq!(
        frames.len(),
        golden.len(),
        "{name}: CLI produced one frame per golden line"
    );

    // Per-tick lock + recompute the FNV `total` accumulator (seed FNV_OFFSET,
    // fold each frame hash) exactly as `shot::run` / the harness emit it.
    let mut acc = FNV_OFFSET;
    for (k, g) in golden.iter().enumerate() {
        let f = &frames[k];
        assert_eq!(f.tick, g.tick, "{name} row {k}: tick column");
        assert_eq!(
            f.frame_hash, g.frame_hash,
            "{name} tick {}: frame hash drifted from golden",
            g.tick
        );
        assert_eq!(
            f.state_hash, g.state_hash,
            "{name} tick {}: state hash drifted from golden",
            g.tick
        );
        acc = (acc ^ f.frame_hash).wrapping_mul(FNV_PRIME);
    }
    assert_eq!(acc, total_acc, "{name}: folded total accumulator vs golden");
}

#[test]
fn blood_render_matches_golden() {
    assert_golden_faithful("blood");
}

#[test]
fn shake_render_matches_golden() {
    assert_golden_faithful("shake");
}
