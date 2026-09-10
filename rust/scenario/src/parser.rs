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
//! render_hud                                     # Slice 3e; draw-time HUD/minimap draw (0 args)
//! render_live                                    # Slice 4d; opt-in live-viewport path (0 args)
//! settings    <file>                             # Step 4½a-1; oracle-only setup sidecar — see
//!                                                 # [`Scenario::settings`]. Excludes `worm`,
//!                                                 # `weapon`, `game_mode`, `max_bonuses`, `render*`
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
    /// Step-4½a-1 `settings <file>` — the oracle-harness-only setup sidecar (a
    /// C++-schema TOML file; the path is relative to the scenario file's directory).
    /// Absent => `None` (every pre-4½ scenario, byte-identical meaning). Present => the
    /// C++ dumper reads it with the real `Settings::FromToml` and starts the worms in
    /// the C++ LocalController state; the Rust golden test uses
    /// `settings_toml::settings_from_toml` + `build::build_match`. A settings scenario
    /// rejects `worm`/`weapon`/`game_mode`/`max_bonuses`/`render*`, and
    /// [`crate::load`] refuses it (design §7.1).
    pub settings: Option<String>,
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
    /// Slice-3e `render_hud` (0-arg) — the draw-time HUD/minimap directive. Absent =>
    /// `false` (the C++ dumper skips the HUD pre-block + minimap; the Rust
    /// `Scene.draw_hud` stays `false`, so the world-only draw is byte-identical). Both
    /// parser sides move together (T6).
    render_hud: bool,
    /// Slice-4d `render_live` (0-arg) — the opt-in live-viewport directive. Absent =>
    /// `false` (prior scenarios untouched). Present (requires `render`) => the C++ dumper
    /// wires the two viewports and drives the real `Game::ProcessFrame` so shake/flash/
    /// banner/centering evolve live from real explosions/spawn/death; the Rust 4d
    /// oracle-test drives the matching live game-layer path. Both parser sides move
    /// together (the shared scenario file must parse on both).
    render_live: bool,
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
        let mut render_hud = false;
        let mut render_live = false;
        let mut settings: Option<String> = None;
        let mut game_mode_given = false;
        let mut max_bonuses_given = false;
        let mut render_given = false;

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
                    max_bonuses_given = true;
                }
                "game_mode" => {
                    expect_args(n, key, &nums, 1)?;
                    game_mode = parse_at(0)? as i32;
                    game_mode_given = true;
                }
                "render" => {
                    // Opt-in render-layout directive for the shared scenario (Slice 3a).
                    // The C++ dumper reads `<layout>` to pick the viewport arrangement and
                    // emit the sidecar frame golden; the Rust frame-hash test hardcodes the
                    // two-viewport player layout, so the arg is validated and ignored here.
                    // Accepted (not rejected) so the shared scenario file parses on BOTH
                    // sides — same discipline as `game_mode`/`max_bonuses` above.
                    expect_args(n, key, &nums, 1)?;
                    render_given = true;
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
                "render_hud" => {
                    // Slice 3e: draw-time HUD/minimap directive (0 args — presence
                    // enables it). Accepted-and-applied on BOTH sides (mirrors the
                    // C++ dumper's `render_hud` arm); the T8 frame-hash test maps it
                    // to `Scene.draw_hud` (and `Scene.map`).
                    expect_args(n, key, &nums, 0)?;
                    render_hud = true;
                }
                "render_live" => {
                    // Slice 4d: opt-in live-viewport directive (0 args — presence enables
                    // it). Accepted-and-stored on BOTH sides (mirrors the C++ dumper's
                    // `render_live` arm); the 4d oracle-test keys the live game-layer path
                    // off it.
                    expect_args(n, key, &nums, 0)?;
                    render_live = true;
                }
                "settings" => {
                    // Step 4½a-1: the oracle-only setup sidecar (design §7.1).
                    expect_args(n, key, &nums, 1)?;
                    if settings.replace(nums[0].to_string()).is_some() {
                        return Err(format!("line {n}: duplicate `settings`"));
                    }
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

        if settings.is_some()
            && (!worms.is_empty()
                || !weapons.is_empty()
                || game_mode_given
                || max_bonuses_given
                || render_given
                || render_shadow
                || !render_shake.is_empty()
                || !render_flash.is_empty()
                || render_hud
                || render_live)
        {
            return Err(
                "`settings` excludes the worm, weapon, game_mode, max_bonuses and render* directives"
                    .to_string(),
            );
        }

        Ok(Scenario {
            seed: seed.ok_or("missing `seed`")?,
            level: level.ok_or("missing `level`")?,
            ticks: ticks.ok_or("missing `ticks`")?,
            max_bonuses,
            game_mode,
            worms,
            settings,
            inputs,
            weapons,
            weapon_ammo,
            render_shadow,
            render_shake,
            render_flash,
            render_hud,
            render_live,
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

    /// Whether the `render_hud` directive is present — the draw-time HUD/minimap
    /// draw. The T8 frame-hash test maps this to `render::frame::Scene.draw_hud`
    /// (and, for the minimap, `Scene.map`).
    pub fn hud(&self) -> bool {
        self.render_hud
    }

    /// Whether the `render_live` directive is present — the opt-in live-viewport path.
    /// The 4d oracle-test keys the live game-layer path (top-of-frame decrement +
    /// shake-event drain + `frame::draw`) off this.
    pub fn live(&self) -> bool {
        self.render_live
    }

    /// Serialize this scenario back to its text form. `Scenario::parse` reads the
    /// result to an **identical** `Scenario` (round-trip identity — see the T0
    /// property test), covering every directive the parser *stores*. Emits the
    /// grammar documented at the top of this module (`seed`/`level`/`ticks`/
    /// `max_bonuses`/`game_mode`/`worm`/`weapon`/`render_*`/`input`).
    ///
    /// `input` lines are **sparse** (spec §3): only ticks whose word is nonzero
    /// for some worm are emitted, in ascending tick order — an all-zero tick
    /// decodes back to `(0, 0)` from its *absence* (see [`Scenario::input`]),
    /// matching the parser's convention and keeping recorded files small. Words
    /// are `sim::state::ControlState::pack()` values (already 7-bit, `state.rs:81`).
    ///
    /// Not emitted: the `render <layout>` directive — the parser validates but
    /// does not store it (`parser.rs:149`), so it is not part of a `Scenario`'s
    /// value and its absence does not affect the round-trip identity.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        // The three required globals, then EITHER the `settings` sidecar OR the two
        // always-defaulted directives it replaces (0 re-parses to 0, so emitting those
        // unconditionally on the classic path still round-trips and is explicit/diffable).
        out.push_str(&format!("seed {}\n", self.seed));
        out.push_str(&format!("level {}\n", self.level));
        out.push_str(&format!("ticks {}\n", self.ticks));
        match &self.settings {
            // A settings scenario carries mode/bonuses in its setup; the parser rejects
            // the two directives alongside `settings`.
            Some(path) => out.push_str(&format!("settings {path}\n")),
            None => {
                out.push_str(&format!("max_bonuses {}\n", self.max_bonuses));
                out.push_str(&format!("game_mode {}\n", self.game_mode));
            }
        }
        // Worms in stored order; `visible` back to 0/1.
        for w in &self.worms {
            out.push_str(&format!(
                "worm {} {} {} {} {} {} {}\n",
                w.index,
                w.pos_x,
                w.pos_y,
                w.health,
                w.lives,
                w.stats_x,
                if w.visible { 1 } else { 0 },
            ));
        }
        // One `weapon` line per slot (ascending for stable output); append the
        // optional ammo token only where a `weapon_ammo` override exists.
        let mut slots: Vec<usize> = self.weapons.keys().copied().collect();
        slots.sort_unstable();
        for slot in slots {
            let name = &self.weapons[&slot];
            match self.weapon_ammo.get(&slot) {
                Some(ammo) => out.push_str(&format!("weapon {slot} {name} {ammo}\n")),
                None => out.push_str(&format!("weapon {slot} {name}\n")),
            }
        }
        // Render directives, preserving the stored order of the injection vectors.
        if self.render_shadow {
            out.push_str("render_shadow\n");
        }
        for (tick, vp, amount) in &self.render_shake {
            out.push_str(&format!("render_shake {tick} {vp} {amount}\n"));
        }
        for (tick, amount) in &self.render_flash {
            out.push_str(&format!("render_flash {tick} {amount}\n"));
        }
        if self.render_hud {
            out.push_str("render_hud\n");
        }
        if self.render_live {
            out.push_str("render_live\n");
        }
        // Sparse `input` lines in ascending tick order; skip all-zero ticks (they
        // decode to (0,0) from absence — the parser convention, `parser.rs:262`).
        let mut input_ticks: Vec<u32> = self.inputs.keys().copied().collect();
        input_ticks.sort_unstable();
        for t in input_ticks {
            let (w0, w1) = self.inputs[&t];
            if w0 == 0 && w1 == 0 {
                continue;
            }
            out.push_str(&format!("input {t} {w0} {w1}\n"));
        }
        out
    }

    /// Build a *recording* scenario from this base: clone the tick-0 metadata
    /// (seed/level/worms/weapons/game_mode/…) and replace the per-tick input
    /// stream with `per_tick` — a positional `(worm0_7bit, worm1_7bit)` word pair
    /// per tick (`ControlState::pack()`), by worm index. `ticks` is set to the
    /// stream length. The stream is stored **sparsely** (all-zero ticks omitted —
    /// they decode to `(0, 0)` from absence, matching the parser and `to_text`),
    /// so a built scenario round-trips through `to_text`/`parse` unchanged
    /// (spec §3 / §4.2).
    pub fn with_recorded_inputs(&self, per_tick: &[(u32, u32)]) -> Scenario {
        let mut s = self.clone();
        s.ticks = per_tick.len() as u32;
        s.inputs = per_tick
            .iter()
            .enumerate()
            .filter(|(_, &(w0, w1))| w0 != 0 || w1 != 0)
            .map(|(t, &pair)| (t as u32, pair))
            .collect();
        s
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

    // ---- Slice 3e draw-time HUD directive (both parser sides move together) ----

    #[test]
    fn render_hud_defaults_off_and_parses() {
        // Absent => off (prior scenarios unchanged).
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert!(!s.hud(), "absent render_hud defaults off");
        // Present (0 args) => on.
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender player\nrender_hud\n")
            .expect("parses");
        assert!(s.hud(), "render_hud enables the draw-time HUD/minimap path");
    }

    #[test]
    fn render_hud_wrong_arity_errors() {
        // render_hud takes NO args; a trailing token is rejected.
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender_hud 1\n").unwrap_err();
        assert!(err.contains("expects 0 args"), "got: {err}");
    }

    // ---- Slice 4d live-viewport directive (both parser sides move together) ----

    #[test]
    fn render_live_defaults_off_and_parses() {
        // Absent => off (prior scenarios unchanged).
        let s = Scenario::parse(SAMPLE).expect("parses");
        assert!(!s.live(), "absent render_live defaults off");
        // Present (0 args) => on.
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender player\nrender_live\n")
            .expect("parses");
        assert!(s.live(), "render_live enables the live-viewport path");
    }

    #[test]
    fn render_live_wrong_arity_errors() {
        // render_live takes NO args; a trailing token is rejected.
        let err =
            Scenario::parse("seed 1\nlevel a.lev\nticks 1\nrender_live 1\n").unwrap_err();
        assert!(err.contains("expects 0 args"), "got: {err}");
    }

    // ---- Slice 4b: `to_text` serializer + `with_recorded_inputs` builder ----

    // A scenario exercising EVERY directive the parser stores: seed/level/ticks,
    // both defaulted globals (max_bonuses, game_mode), two worms (one visible, one
    // not; distinct fields), a bare weapon + a weapon with an ammo token,
    // render_shadow, two render_shake (same tick, order-sensitive), render_flash,
    // render_hud, render_live, and sparse input incl. the max 7-bit word (127). No explicit
    // all-zero `input` line (those don't round-trip — see the risk note in the
    // done-report; they are semantically absence and the recorder never emits them).
    const RICH: &str = "\
seed 42
level Levels/foo.lev
ticks 200
max_bonuses 4
game_mode 1
worm 0 6553600 3276800 100 10 0 1
worm 1 13107200 3276800 90 5 218 0
weapon 0 fan
weapon 1 HANDGUN 2
render_shadow
render_shake 3 1 7
render_shake 3 0 4
render_flash 2 20
render_hud
render_live
input 5 16 0
input 10 127 64
";

    #[test]
    fn to_text_round_trips_every_directive() {
        let s = Scenario::parse(RICH).expect("fixture parses");
        let round = Scenario::parse(&s.to_text()).expect("to_text output re-parses");
        assert_eq!(
            round, s,
            "parse(to_text(s)) must equal s over every directive"
        );
    }

    #[test]
    fn to_text_round_trips_minimal() {
        // Only the three required directives; every optional field at its default.
        let s = Scenario::parse("seed 1\nlevel a.lev\nticks 0\n").expect("parses");
        assert_eq!(Scenario::parse(&s.to_text()).unwrap(), s);
    }

    #[test]
    fn to_text_input_is_sparse_and_ascending() {
        let text = Scenario::parse(RICH).expect("parses").to_text();
        // Both nonzero ticks emitted, worm-index order preserved (w0 then w1), and
        // the max 7-bit word survives verbatim.
        assert!(text.contains("input 5 16 0"), "text: {text}");
        assert!(text.contains("input 10 127 64"), "text: {text}");
        // Ascending tick order.
        let i5 = text.find("input 5 ").expect("tick 5 line");
        let i10 = text.find("input 10 ").expect("tick 10 line");
        assert!(i5 < i10, "input lines ascending by tick: {text}");
        // The `render <layout>` directive is not stored, so it is never emitted.
        assert!(
            !text.contains("\nrender player"),
            "render layout not emitted: {text}"
        );
    }

    #[test]
    fn with_recorded_inputs_builds_sparse_stream() {
        let base = Scenario::parse(RICH).expect("parses");
        let rec = base.with_recorded_inputs(&[(5, 3), (0, 0), (127, 0)]);
        // ticks == stream length; words are positional by worm index (w0 is worm 0).
        assert_eq!(rec.ticks, 3);
        assert_eq!(rec.input(0, 0), 5);
        assert_eq!(rec.input(0, 1), 3);
        // Middle all-zero tick decodes to (0,0) from absence.
        assert_eq!(rec.input(1, 0), 0);
        assert_eq!(rec.input(1, 1), 0);
        assert_eq!(rec.input(2, 0), 127);
        assert_eq!(rec.input(2, 1), 0);
        // Tick-0 metadata carried verbatim from the base.
        assert_eq!(rec.seed, base.seed);
        assert_eq!(rec.level, base.level);
        assert_eq!(rec.worms, base.worms);
        assert_eq!(rec.game_mode, base.game_mode);
        assert_eq!(rec.max_bonuses, base.max_bonuses);
        // The all-zero tick 1 is stored sparsely => it emits NO `input` line.
        let text = rec.to_text();
        assert!(
            !text.contains("input 1 "),
            "all-zero tick emits no input line: {text}"
        );
        assert!(text.contains("input 0 5 3"), "text: {text}");
        assert!(text.contains("input 2 127 0"), "text: {text}");
        // A built scenario also round-trips through text unchanged.
        assert_eq!(Scenario::parse(&rec.to_text()).unwrap(), rec);
    }

    #[test]
    fn with_recorded_inputs_empty_and_all_zero_stream() {
        let base = Scenario::parse(RICH).expect("parses");
        // Empty stream: ticks 0, no stored inputs, round-trips.
        let empty = base.with_recorded_inputs(&[]);
        assert_eq!(empty.ticks, 0);
        assert!(
            !empty.to_text().contains("\ninput "),
            "no input lines for empty stream"
        );
        assert_eq!(Scenario::parse(&empty.to_text()).unwrap(), empty);
        // All-zero stream: ticks set to length, but nothing stored (sparse); round-trips.
        let zeros = base.with_recorded_inputs(&[(0, 0), (0, 0)]);
        assert_eq!(zeros.ticks, 2);
        assert_eq!(zeros.input(0, 0), 0);
        assert!(
            !zeros.to_text().contains("\ninput "),
            "no input lines for all-zero stream"
        );
        assert_eq!(Scenario::parse(&zeros.to_text()).unwrap(), zeros);
    }

    const SETTINGS_SCN: &str =
        "seed 7\nlevel Levels/modern_test.lev\nticks 3\nsettings a_setup.cfg\ninput 1 16 0\n";

    #[test]
    fn settings_directive_parses_and_defaults_absent() {
        let s = Scenario::parse(SETTINGS_SCN).expect("parses");
        assert_eq!(s.settings.as_deref(), Some("a_setup.cfg"));
        assert!(s.worms.is_empty());
        let plain = Scenario::parse("seed 1\nlevel L\nticks 1\n").unwrap();
        assert_eq!(
            plain.settings, None,
            "absent => None: every existing scenario is unchanged"
        );
    }

    #[test]
    fn settings_excludes_the_directives_it_replaces() {
        for extra in [
            "worm 0 0 0 100 10 0 0",
            "weapon 0 DART",
            "game_mode 1",
            "max_bonuses 0",
            "render player",
            "render_shadow",
            "render_shake 1 0 2",
            "render_flash 1 3",
            "render_hud",
            "render_live",
        ] {
            let text = format!("{SETTINGS_SCN}{extra}\n");
            assert!(
                Scenario::parse(&text).is_err(),
                "`settings` + `{extra}` must be rejected"
            );
        }
    }

    #[test]
    fn settings_arity_and_duplicates_error() {
        assert!(Scenario::parse("seed 1\nlevel L\nticks 1\nsettings\n").is_err());
        assert!(Scenario::parse("seed 1\nlevel L\nticks 1\nsettings a b\n").is_err());
        assert!(Scenario::parse("seed 1\nlevel L\nticks 1\nsettings a\nsettings b\n").is_err());
    }

    #[test]
    fn to_text_round_trips_a_settings_scenario() {
        let s = Scenario::parse(SETTINGS_SCN).unwrap();
        let text = s.to_text();
        assert!(text.contains("settings a_setup.cfg\n"));
        assert!(!text.contains("game_mode") && !text.contains("max_bonuses"));
        assert_eq!(Scenario::parse(&text).unwrap(), s);
    }
}
