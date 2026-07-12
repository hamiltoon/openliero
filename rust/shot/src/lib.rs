use std::path::PathBuf;

/// Parsed configuration for the headless screenshot CLI.
///
/// Only the argument-parsing surface is populated in this slice (T0); the
/// render/PNG behaviour lands in later tasks.
pub struct Config {
    /// Scenario name to load.
    pub scenario: String,
    /// One or more `--tick` values, in the order they appeared.
    pub ticks: Vec<u32>,
    /// PNG path (single tick) or directory (multiple ticks). `None` when only
    /// `--hashes` was requested.
    pub out: Option<PathBuf>,
    /// Pixel scale factor (default 3).
    pub scale: u32,
    /// Whether `--hashes` was requested.
    pub hashes: bool,
    /// Optional `--tc-root` override.
    pub tc_root: Option<PathBuf>,
}

/// Parse the CLI arguments (already stripped of the program name).
///
/// Recognised flags:
/// - `--scenario <name>` (required)
/// - `--tick <u32>` (required, repeatable — accumulates)
/// - `--out <path>`
/// - `--scale <u32>` (default 3)
/// - `--hashes`
/// - `--tc-root <path>`
///
/// At least one of `--out` / `--hashes` must be present.
pub fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut scenario: Option<String> = None;
    let mut ticks: Vec<u32> = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut scale: u32 = 3;
    let mut hashes = false;
    let mut tc_root: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--scenario" => {
                let v = value_for(args, &mut i, "--scenario")?;
                scenario = Some(v);
            }
            "--tick" => {
                let v = value_for(args, &mut i, "--tick")?;
                let tick: u32 = v
                    .parse()
                    .map_err(|_| format!("invalid --tick value: {v}"))?;
                ticks.push(tick);
            }
            "--out" => {
                let v = value_for(args, &mut i, "--out")?;
                out = Some(PathBuf::from(v));
            }
            "--scale" => {
                let v = value_for(args, &mut i, "--scale")?;
                scale = v
                    .parse()
                    .map_err(|_| format!("invalid --scale value: {v}"))?;
            }
            "--hashes" => {
                hashes = true;
            }
            "--tc-root" => {
                let v = value_for(args, &mut i, "--tc-root")?;
                tc_root = Some(PathBuf::from(v));
            }
            other => {
                return Err(format!("unknown flag: {other}"));
            }
        }
        i += 1;
    }

    let scenario = scenario.ok_or_else(|| "missing required --scenario".to_string())?;
    if ticks.is_empty() {
        return Err("missing required --tick".to_string());
    }
    if out.is_none() && !hashes {
        return Err("require --out or --hashes".to_string());
    }

    Ok(Config {
        scenario,
        ticks,
        out,
        scale,
        hashes,
        tc_root,
    })
}

/// Consume the value following a `--flag`, advancing the index past it.
fn value_for(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    let next = args
        .get(*i + 1)
        .ok_or_else(|| format!("missing value for {flag}"))?;
    *i += 1;
    Ok(next.clone())
}

#[cfg(test)]
mod parse_tests {
    use super::*;
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn minimal_single_tick() {
        let c = parse_args(&v(&[
            "--scenario",
            "blood",
            "--tick",
            "35",
            "--out",
            "/tmp/x.png",
        ]))
        .unwrap();
        assert_eq!(c.scenario, "blood");
        assert_eq!(c.ticks, vec![35]);
        assert_eq!(c.out.as_deref(), Some(std::path::Path::new("/tmp/x.png")));
        assert_eq!(c.scale, 3);
        assert!(!c.hashes);
    }
    #[test]
    fn repeated_tick_and_scale_and_hashes() {
        let c = parse_args(&v(&[
            "--scenario",
            "laser",
            "--tick",
            "5",
            "--tick",
            "9",
            "--scale",
            "4",
            "--hashes",
        ]))
        .unwrap();
        assert_eq!(c.ticks, vec![5, 9]);
        assert_eq!(c.scale, 4);
        assert!(c.hashes);
        assert!(c.out.is_none());
    }
    #[test]
    fn missing_scenario_is_error() {
        assert!(parse_args(&v(&["--tick", "1", "--out", "/tmp/x.png"])).is_err());
    }
    #[test]
    fn missing_tick_is_error() {
        assert!(parse_args(&v(&["--scenario", "blood", "--out", "/tmp/x.png"])).is_err());
    }
    #[test]
    fn no_out_and_no_hashes_is_error() {
        assert!(parse_args(&v(&["--scenario", "blood", "--tick", "1"])).is_err());
    }
}
