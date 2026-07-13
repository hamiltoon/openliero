//! Integration test for `shot --scenario-path <file>` — the 4g headless
//! replay-screenshot capability. A 4b recording IS a scenario file (no
//! sidecar), so pointing `--scenario-path` at the committed recorded corpus
//! proves the "screenshot a recording" use case end to end: read an
//! arbitrary file directly (bypassing the name→golden-dir lookup), parse it,
//! render a tick, and write a PNG of plausible size.

use std::path::PathBuf;

/// The committed 4b recorded corpus — a scenario file with no golden sidecar
/// (it's a recording, not a `render_slice*` fixture), which is exactly the
/// case `--scenario-path` exists to unlock.
const RECORDED_SCENARIO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../oracle-tests/golden/record_slice4b_blood_scenario.txt"
);

fn unique_out_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "shot_scenario_path_test_{}_{}.png",
        std::process::id(),
        name
    ))
}

#[test]
fn scenario_path_renders_recorded_corpus() {
    let out = unique_out_path("recorded_corpus");
    let args: Vec<String> = ["--scenario-path", RECORDED_SCENARIO, "--tick", "3", "--out"]
        .into_iter()
        .map(str::to_string)
        .chain(std::iter::once(out.to_string_lossy().into_owned()))
        .collect();

    let cfg = shot::parse_args(&args).expect("--scenario-path parses");
    assert_eq!(
        cfg.scenario_path.as_deref(),
        Some(std::path::Path::new(RECORDED_SCENARIO))
    );
    assert!(cfg.scenario.is_none());

    shot::run(&cfg).expect("run succeeds against a sidecar-less recording");

    let bytes = std::fs::read(&out).expect("PNG was written");
    // A sane lower bound: a real 320x200x3 PNG is comfortably >1KB even
    // heavily compressed; this guards against an empty/near-empty file
    // silently "succeeding".
    assert!(
        bytes.len() > 1024,
        "PNG is a plausible size, got {} bytes",
        bytes.len()
    );
    // PNG magic bytes.
    assert_eq!(
        &bytes[..8],
        &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
    );

    let _ = std::fs::remove_file(&out);
}

#[test]
fn scenario_path_hashes_only_recorded_corpus() {
    // `--hashes` without `--out` on a scenario-path source: dump-only, no
    // golden to compare against (there is no `render_slice*` sidecar for a
    // recording) — matches the documented "no golden = just dump" semantics
    // for `--hashes`.
    let args: Vec<String> = [
        "--scenario-path",
        RECORDED_SCENARIO,
        "--tick",
        "5",
        "--hashes",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    let cfg = shot::parse_args(&args).expect("--scenario-path + --hashes parses");
    shot::run(&cfg).expect("run succeeds; hashes are dumped to stdout, no comparison performed");
}
