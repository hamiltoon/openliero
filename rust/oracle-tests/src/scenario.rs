//! Parser for the Slice-2 physics scenario file.
//!
//! The scenario is a small committed text file (`golden/sim_slice2_scenario.txt`,
//! created in a later task) that is the *single source of truth* for the
//! differential test — both the Rust test and the C++ dumper read it, so there
//! are no duplicated fixture constants (unlike Slice 1). See the Slice-2 design
//! doc, *Input-vector / scenario file format*.
//!
//! Grammar (one directive per line; `#` starts a comment; blank lines ignored):
//!
//! ```text
//! seed  <u32>
//! level <path>                                  # relative to the TC root
//! ticks <u32>
//! worm  <index> <pos_x> <pos_y> <health> <lives> <stats_x> <visible>
//! input <tick> <worm0_7bit> <worm1_7bit>        # sparse; absent => 0
//! ```
//!
//! `pos_x`/`pos_y` are 16.16 fixed-point; `visible` is `0`/`1`. A worm's input
//! at a tick is `0` unless an `input` line overrides it — see [`Scenario::input`].

use std::collections::HashMap;

/// One worm's start conditions from a `worm` line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScenarioWorm {
    /// Worm index (0 or 1).
    pub index: i32,
    /// Start position, 16.16 fixed-point.
    pub pos_x: i32,
    pub pos_y: i32,
    pub health: i32,
    pub lives: i32,
    pub stats_x: i32,
    pub visible: bool,
}

/// A parsed scenario: globals, worm start conditions, and sparse input overrides.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scenario {
    pub seed: u32,
    /// Level path relative to the TC root (`data/TC/openliero`).
    pub level: String,
    pub ticks: u32,
    pub worms: Vec<ScenarioWorm>,
    /// Sparse per-tick input overrides: `tick -> (worm0_7bit, worm1_7bit)`.
    inputs: HashMap<u32, (u32, u32)>,
}

impl Scenario {
    /// Parse a scenario from its text form. Returns a human-readable error
    /// (with the 1-based line number) on the first malformed line.
    pub fn parse(text: &str) -> Result<Scenario, String> {
        let mut seed: Option<u32> = None;
        let mut level: Option<String> = None;
        let mut ticks: Option<u32> = None;
        let mut worms = Vec::new();
        let mut inputs = HashMap::new();

        for (lineno, raw) in text.lines().enumerate() {
            let n = lineno + 1;
            // Strip comments (everything from the first `#`) and surrounding ws.
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut tok = line.split_whitespace();
            let key = tok.next().expect("non-empty line has a token");

            // Parse the rest of the line as exactly `count` integers of type T.
            let nums = tok.collect::<Vec<_>>();
            let parse_at = |idx: usize| -> Result<i64, String> {
                nums[idx]
                    .parse::<i64>()
                    .map_err(|e| format!("line {n}: bad number {:?}: {e}", nums[idx]))
            };

            match key {
                "seed" => {
                    expect_args(n, key, &nums, 1)?;
                    seed = Some(parse_at(0)? as u32);
                }
                "level" => {
                    expect_args(n, key, &nums, 1)?;
                    level = Some(nums[0].to_string());
                }
                "ticks" => {
                    expect_args(n, key, &nums, 1)?;
                    ticks = Some(parse_at(0)? as u32);
                }
                "worm" => {
                    expect_args(n, key, &nums, 7)?;
                    let visible = match parse_at(6)? {
                        0 => false,
                        1 => true,
                        v => return Err(format!("line {n}: visible must be 0 or 1, got {v}")),
                    };
                    worms.push(ScenarioWorm {
                        index: parse_at(0)? as i32,
                        pos_x: parse_at(1)? as i32,
                        pos_y: parse_at(2)? as i32,
                        health: parse_at(3)? as i32,
                        lives: parse_at(4)? as i32,
                        stats_x: parse_at(5)? as i32,
                        visible,
                    });
                }
                "input" => {
                    expect_args(n, key, &nums, 3)?;
                    let tick = parse_at(0)? as u32;
                    let w0 = parse_at(1)? as u32;
                    let w1 = parse_at(2)? as u32;
                    if inputs.insert(tick, (w0, w1)).is_some() {
                        return Err(format!("line {n}: duplicate input for tick {tick}"));
                    }
                }
                other => return Err(format!("line {n}: unknown directive {other:?}")),
            }
        }

        Ok(Scenario {
            seed: seed.ok_or("missing `seed`")?,
            level: level.ok_or("missing `level`")?,
            ticks: ticks.ok_or("missing `ticks`")?,
            worms,
            inputs,
        })
    }

    /// The 7-bit input for `worm` (0 or 1) at `tick`. Returns `0` for any tick
    /// without an `input` override — the absence of a line *is* "no keys".
    pub fn input(&self, tick: u32, worm: usize) -> u32 {
        let (w0, w1) = self.inputs.get(&tick).copied().unwrap_or((0, 0));
        match worm {
            0 => w0,
            1 => w1,
            _ => 0,
        }
    }
}

/// Verify a directive got exactly `want` arguments.
fn expect_args(n: usize, key: &str, nums: &[&str], want: usize) -> Result<(), String> {
    if nums.len() != want {
        return Err(format!(
            "line {n}: `{key}` expects {want} args, got {}",
            nums.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A synthetic scenario string (the committed fixture arrives in a later
    // task). Exercises comments, blank lines, two worms, and one sparse input.
    const SAMPLE: &str = "\
# Step 2 Slice 2 scenario — synthetic.
seed 42
level Levels/modern_test.lev
ticks 100

# worm <index> <pos_x> <pos_y> <health> <lives> <stats_x> <visible>
worm 0 6553600 3276800 100 10 0   1
worm 1 13107200 3276800 100 10 218 1
# input <tick> <worm0_7bit> <worm1_7bit>
input 5 16 0
";

    #[test]
    fn parses_globals_and_worms() {
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert_eq!(s.seed, 42);
        assert_eq!(s.level, "Levels/modern_test.lev");
        assert_eq!(s.ticks, 100);
        assert_eq!(s.worms.len(), 2);
        assert_eq!(
            s.worms[0],
            ScenarioWorm {
                index: 0,
                pos_x: 6553600,
                pos_y: 3276800,
                health: 100,
                lives: 10,
                stats_x: 0,
                visible: true,
            }
        );
        assert_eq!(s.worms[1].index, 1);
        assert_eq!(s.worms[1].pos_x, 13107200);
        assert_eq!(s.worms[1].stats_x, 218);
        assert!(s.worms[1].visible);
    }

    #[test]
    fn input_override_is_sparse_and_defaults_to_zero() {
        let s = Scenario::parse(SAMPLE).expect("parses");
        // Tick 5 overrides worm 0 to 16 (Fire bit), worm 1 stays 0.
        assert_eq!(s.input(5, 0), 16);
        assert_eq!(s.input(5, 1), 0);
        // Un-overridden ticks are 0 for both worms.
        assert_eq!(s.input(0, 0), 0);
        assert_eq!(s.input(4, 0), 0);
        assert_eq!(s.input(99, 1), 0);
    }

    #[test]
    fn visible_flag_round_trips_zero() {
        let s = Scenario::parse(
            "seed 1\nlevel a.lev\nticks 1\nworm 0 0 0 100 10 0 0\n",
        )
        .expect("parses");
        assert!(!s.worms[0].visible, "visible 0 => false");
    }

    #[test]
    fn missing_required_field_errors() {
        let err = Scenario::parse("level a.lev\nticks 1\n").unwrap_err();
        assert!(err.contains("seed"), "error mentions missing seed: {err}");
    }

    #[test]
    fn unknown_directive_errors() {
        let err = Scenario::parse("seed 1\nlevel a\nticks 1\nbogus 3\n").unwrap_err();
        assert!(err.contains("unknown directive"), "got: {err}");
    }

    #[test]
    fn wrong_arity_errors() {
        let err = Scenario::parse("seed 1\nlevel a\nticks 1\nworm 0 0 0\n").unwrap_err();
        assert!(err.contains("expects 7 args"), "got: {err}");
    }

    #[test]
    fn bad_visible_value_errors() {
        let err =
            Scenario::parse("seed 1\nlevel a\nticks 1\nworm 0 0 0 100 10 0 2\n").unwrap_err();
        assert!(err.contains("visible must be 0 or 1"), "got: {err}");
    }
}
