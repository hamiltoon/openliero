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
//! seed        <u32>
//! level       <path>                             # relative to the TC root
//! ticks       <u32>
//! max_bonuses <i32>                              # Settings::max_bonuses; absent => 0
//! game_mode   <i32>                              # Settings::game_mode enum; absent => 0 (KillEmAll)
//! worm        <index> <pos_x> <pos_y> <health> <lives> <stats_x> <visible>
//! input       <tick> <worm0_7bit> <worm1_7bit>   # sparse; absent => 0
//! weapon      <slot> <name> [ammo]               # override worm weapon slot (0..NUM_WEAPONS);
//!                                                 # optional 3rd token overrides starting ammo
//! render      <layout>                           # Slice 3a; only `player` (enables the sidecar)
//! render_shadow                                  # Slice 3b; draw-time-only shadow flip (0 args)
//! render_shake <tick> <vp> <amount>             # Slice 3b; draw-only shake injection
//! render_flash <tick> <amount>                  # Slice 3b; draw-only screen-flash injection
//! ```
//!
//! `pos_x`/`pos_y` are 16.16 fixed-point; `visible` is `0`/`1`. A worm's input
//! at a tick is `0` unless an `input` line overrides it — see [`Scenario::input`].

use std::collections::HashMap;

/// Number of weapon slots per worm — mirrors C++ `NUM_WEAPONS` (worm.hpp:13).
const NUM_WEAPONS: usize = 5;

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
    /// C++ `Settings::max_bonuses` — the cap the per-tick bonus-drop roll gates on
    /// (`game.cpp:359`). Absent => `0` (the roll short-circuits, drawing no rand, so
    /// scenarios without the directive stay byte-identical). Slice 5c sets it `> 0`.
    pub max_bonuses: i32,
    /// C++ `Settings::game_mode` — the game-mode switch (`game.cpp:372-461`). Absent =>
    /// `0` (`kGmKillEmAll`, the switch's `default: break` — inert, so scenarios without
    /// the directive stay byte-identical). Accepted-and-defaulted so a future game-mode
    /// scenario keeps the shared scenario file parsing on both sides (Slice 6, T0).
    pub game_mode: i32,
    pub worms: Vec<ScenarioWorm>,
    /// Sparse per-tick input overrides: `tick -> (worm0_7bit, worm1_7bit)`.
    inputs: HashMap<u32, (u32, u32)>,
    /// Per-slot weapon overrides: `slot -> weapon_name`.
    weapons: HashMap<usize, String>,
    /// Per-slot starting-ammo overrides: `slot -> ammo` (the optional 3rd token of
    /// a `weapon` directive). Absent => use the weapon type's default ammo.
    weapon_ammo: HashMap<usize, i32>,
    /// Slice-3b `render_shadow` (0-arg) — the draw-time-only shadow flip. Absent =>
    /// `false` (the C++ dumper leaves `settings->shadow = false`, no shadow pass; the
    /// Rust `Scene.draw_shadow` stays `false`). Both sides move together.
    render_shadow: bool,
    /// Slice-3b `render_shake <tick> <vp> <amount>` — draw-only screen-shake injections
    /// as `(tick, vp, amount)`. The C++ dumper sets `viewports[vp]->shake = Itof(amount)`
    /// for that draw; the Rust T8 test injects `itof(amount)` into `viewports[vp].shake`.
    render_shake: Vec<(u32, usize, i32)>,
    /// Slice-3b `render_flash <tick> <amount>` — draw-only screen-flash injections as
    /// `(tick, amount)`. Fed into the palette `LightUp` (C++) / `Scene.screen_flash` (Rust).
    render_flash: Vec<(u32, i32)>,
}

impl Scenario {
    /// Parse a scenario from its text form. Returns a human-readable error
    /// (with the 1-based line number) on the first malformed line.
    pub fn parse(text: &str) -> Result<Scenario, String> {
        let mut seed: Option<u32> = None;
        let mut level: Option<String> = None;
        let mut ticks: Option<u32> = None;
        let mut max_bonuses: i32 = 0;
        let mut game_mode: i32 = 0;
        let mut worms = Vec::new();
        let mut inputs = HashMap::new();
        let mut weapons: HashMap<usize, String> = HashMap::new();
        let mut weapon_ammo: HashMap<usize, i32> = HashMap::new();
        let mut render_shadow = false;
        let mut render_shake: Vec<(u32, usize, i32)> = Vec::new();
        let mut render_flash: Vec<(u32, i32)> = Vec::new();

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
                "max_bonuses" => {
                    expect_args(n, key, &nums, 1)?;
                    max_bonuses = parse_at(0)? as i32;
                }
                "game_mode" => {
                    expect_args(n, key, &nums, 1)?;
                    game_mode = parse_at(0)? as i32;
                }
                "render" => {
                    // Opt-in render-layout directive for the shared scenario (Slice 3a).
                    // The C++ dumper reads `<layout>` to pick the viewport arrangement and
                    // emit the sidecar frame golden; the Rust frame-hash test hardcodes the
                    // two-viewport player layout, so the arg is validated and ignored here.
                    // Accepted (not rejected) so the shared scenario file parses on BOTH
                    // sides — same discipline as `game_mode`/`max_bonuses` above.
                    expect_args(n, key, &nums, 1)?;
                }
                "render_shadow" => {
                    // Slice 3b: draw-time-only shadow flip (0 args — presence flips it).
                    // Accepted-and-applied on BOTH sides (mirrors the C++ dumper's
                    // `render_shadow` arm); the Rust T8 test maps it to `Scene.draw_shadow`.
                    expect_args(n, key, &nums, 0)?;
                    render_shadow = true;
                }
                "render_shake" => {
                    // Slice 3b: `render_shake <tick> <vp> <amount>` — draw-only injection.
                    expect_args(n, key, &nums, 3)?;
                    let tick = parse_at(0)? as u32;
                    let vp = parse_at(1)? as usize;
                    let amount = parse_at(2)? as i32;
                    render_shake.push((tick, vp, amount));
                }
                "render_flash" => {
                    // Slice 3b: `render_flash <tick> <amount>` — draw-only injection.
                    expect_args(n, key, &nums, 2)?;
                    let tick = parse_at(0)? as u32;
                    let amount = parse_at(1)? as i32;
                    render_flash.push((tick, amount));
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
                "weapon" => {
                    // 2 args (slot, name) OR 3 args (slot, name, ammo). The optional
                    // 3rd token overrides the weapon type's default starting ammo —
                    // mirrors the C++ dumper's slice-4d `weapon <slot> <name> [ammo]`.
                    if nums.len() != 2 && nums.len() != 3 {
                        return Err(format!(
                            "line {n}: `weapon` expects 2 or 3 args, got {}",
                            nums.len()
                        ));
                    }
                    let slot = parse_at(0)? as usize;
                    if slot >= NUM_WEAPONS {
                        return Err(format!(
                            "line {n}: weapon slot {slot} out of range (0..{NUM_WEAPONS})"
                        ));
                    }
                    let name = nums[1].to_string();
                    if weapons.insert(slot, name).is_some() {
                        return Err(format!("line {n}: duplicate weapon for slot {slot}"));
                    }
                    if nums.len() == 3 {
                        weapon_ammo.insert(slot, parse_at(2)? as i32);
                    }
                }
                other => return Err(format!("line {n}: unknown directive {other:?}")),
            }
        }

        Ok(Scenario {
            seed: seed.ok_or("missing `seed`")?,
            level: level.ok_or("missing `level`")?,
            ticks: ticks.ok_or("missing `ticks`")?,
            max_bonuses,
            game_mode,
            worms,
            inputs,
            weapons,
            weapon_ammo,
            render_shadow,
            render_shake,
            render_flash,
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

    /// The weapon name overriding `slot`, or `None` if no `weapon` directive set it.
    pub fn weapon(&self, slot: usize) -> Option<&str> {
        self.weapons.get(&slot).map(String::as_str)
    }

    /// The starting-ammo override for `slot` (the optional 3rd `weapon` token), or
    /// `None` if absent — in which case the caller uses the weapon type's default ammo.
    pub fn weapon_ammo(&self, slot: usize) -> Option<i32> {
        self.weapon_ammo.get(&slot).copied()
    }

    /// Whether the `render_shadow` directive is present — the draw-time-only shadow
    /// flip. The T8 frame-hash test maps this to `render::frame::Scene.draw_shadow`.
    pub fn shadow(&self) -> bool {
        self.render_shadow
    }

    /// The `render_shake` injections for `tick` as `(vp, amount)` pairs (empty if none).
    /// The T8 test injects `itof(amount)` into `viewports[vp].shake` before drawing `tick`.
    pub fn shake_at(&self, tick: u32) -> Vec<(usize, i32)> {
        self.render_shake
            .iter()
            .filter(|(t, _, _)| *t == tick)
            .map(|(_, vp, amount)| (*vp, *amount))
            .collect()
    }

    /// The `render_flash` amount for `tick`, or `None` if the tick has no injection.
    /// The T8 test passes this as `Scene.screen_flash` for that tick's draw.
    pub fn flash_at(&self, tick: u32) -> Option<i32> {
        self.render_flash
            .iter()
            .find(|(t, _)| *t == tick)
            .map(|(_, amount)| *amount)
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

    #[test]
    fn weapon_directive_parses() {
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nweapon 0 fan\n")
            .expect("parses");
        assert_eq!(s.weapon(0), Some("fan"));
        assert_eq!(s.weapon(1), None);
        // No 3rd token => no ammo override (caller uses the weapon's default ammo).
        assert_eq!(s.weapon_ammo(0), None);
    }

    #[test]
    fn weapon_directive_with_ammo_token_parses() {
        // The optional 3rd token overrides starting ammo (slice-4d `weapon 0 HANDGUN 2`).
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nweapon 0 HANDGUN 2\n")
            .expect("parses");
        assert_eq!(s.weapon(0), Some("HANDGUN"));
        assert_eq!(s.weapon_ammo(0), Some(2));
        assert_eq!(s.weapon_ammo(1), None);
    }

    #[test]
    fn weapon_wrong_arity_errors() {
        // 1 arg (slot only) and 4 args are both rejected; the 3-arg form is valid.
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nweapon 0\n").unwrap_err();
        assert!(err.contains("expects 2 or 3 args"), "got: {err}");
        let err4 =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nweapon 0 fan 2 3\n").unwrap_err();
        assert!(err4.contains("expects 2 or 3 args"), "got: {err4}");
    }

    #[test]
    fn weapon_out_of_range_errors() {
        // slot 5 is >= NUM_WEAPONS (5); line 4 is the offending line.
        let err = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nweapon 5 fan\n").unwrap_err();
        assert!(err.contains("line 4"), "error must mention line number: {err}");
    }

    #[test]
    fn weapon_duplicate_slot_errors() {
        // Second `weapon 0` is line 5.
        let err = Scenario::parse(
            "seed 1\nlevel a.lev\nticks 1\nweapon 0 fan\nweapon 0 dart\n",
        )
        .unwrap_err();
        assert!(err.contains("line 5"), "error must mention line number: {err}");
    }

    #[test]
    fn weapon_no_directive_back_compat() {
        // SAMPLE has no `weapon` lines — existing scenarios must parse unchanged.
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert_eq!(s.weapon(0), None);
    }

    #[test]
    fn max_bonuses_defaults_to_zero_and_parses() {
        // Absent `max_bonuses` => 0 (the roll short-circuits; prior scenarios unchanged).
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert_eq!(s.max_bonuses, 0, "absent max_bonuses defaults to 0");
        // The Slice-5c directive sets it explicitly.
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nmax_bonuses 4\n")
            .expect("parses");
        assert_eq!(s.max_bonuses, 4);
    }

    #[test]
    fn max_bonuses_wrong_arity_errors() {
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nmax_bonuses\n").unwrap_err();
        assert!(err.contains("expects 1 args"), "got: {err}");
    }

    #[test]
    fn game_mode_defaults_to_zero_and_parses() {
        // Absent `game_mode` => 0 (kGmKillEmAll; the switch is inert, prior scenarios
        // unchanged).
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert_eq!(s.game_mode, 0, "absent game_mode defaults to 0 (KillEmAll)");
        // A Slice-6 game-mode scenario sets it explicitly (e.g. 1 = kGmGameOfTag).
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\ngame_mode 1\n")
            .expect("parses");
        assert_eq!(s.game_mode, 1);
    }

    #[test]
    fn game_mode_wrong_arity_errors() {
        let err = Scenario::parse("seed 1\nlevel a.lev\nticks 1\ngame_mode\n").unwrap_err();
        assert!(err.contains("expects 1 args"), "got: {err}");
    }

    // ---- Slice 3b draw-time render directives (both parser sides move together) ----

    #[test]
    fn render_shadow_defaults_off_and_parses() {
        // Absent => off (prior scenarios unchanged).
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert!(!s.shadow(), "absent render_shadow defaults off");
        // Present (0 args) => on.
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender player\nrender_shadow\n")
            .expect("parses");
        assert!(s.shadow(), "render_shadow flips the draw-time shadow flag");
    }

    #[test]
    fn render_shadow_wrong_arity_errors() {
        // render_shadow takes NO args; a trailing token is rejected.
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender_shadow 1\n").unwrap_err();
        assert!(err.contains("expects 0 args"), "got: {err}");
    }

    #[test]
    fn render_shake_parses_and_defaults_empty() {
        // Absent => no shake for any tick.
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert!(s.shake_at(0).is_empty(), "absent render_shake => empty");
        // `render_shake <tick> <vp> <amount>` — read back per tick as (vp, amount).
        let s = Scenario::parse(
            "seed 1\nlevel a.lev\nticks 5\nrender player\nrender_shake 3 1 7\nrender_shake 3 0 4\n",
        )
        .expect("parses");
        let mut at3 = s.shake_at(3);
        at3.sort_unstable();
        assert_eq!(at3, vec![(0usize, 4i32), (1usize, 7i32)], "tick 3 shake per vp");
        assert!(s.shake_at(4).is_empty(), "tick without a render_shake => empty");
    }

    #[test]
    fn render_shake_wrong_arity_errors() {
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender_shake 3 1\n").unwrap_err();
        assert!(err.contains("expects 3 args"), "got: {err}");
    }

    #[test]
    fn render_flash_parses_and_defaults_absent() {
        // Absent => None for any tick.
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert_eq!(s.flash_at(0), None, "absent render_flash => None");
        // `render_flash <tick> <amount>`.
        let s = Scenario::parse(
            "seed 1\nlevel a.lev\nticks 5\nrender player\nrender_flash 2 20\n",
        )
        .expect("parses");
        assert_eq!(s.flash_at(2), Some(20), "tick 2 flash amount");
        assert_eq!(s.flash_at(1), None, "tick without a render_flash => None");
    }

    #[test]
    fn render_flash_wrong_arity_errors() {
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender_flash 2\n").unwrap_err();
        assert!(err.contains("expects 2 args"), "got: {err}");
    }
}
