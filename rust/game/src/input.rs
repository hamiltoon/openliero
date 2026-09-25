//! Live-input core (Slice 4a, T0): the **pure, Bevy-free** per-worm sampler.
//!
//! The correctness core — the 7-bit mapping and the synthetic Dig chord — is
//! made *generic over the key type* [`PlayerBindings<K>`] so it is unit-testable
//! with a mock key + a `HashSet` "pressed" closure (no window, no `ButtonInput`),
//! and so it runs in the fast CI test set. `game` instantiates it with Bevy's
//! `KeyCode` via [`default_bindings`]. See spec §4.1.

use std::path::{Path, PathBuf};

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::Resource;

use scenario::Scenario;
use sim::state::ControlState;

/// Number of worms sampled per tick — positional `[ControlState; N]`, the same
/// index `process_frame` reads and `Viewport::worm_idx` maps (spec §4.2).
pub const N_WORMS: usize = 2;

/// One worm's key bindings. Generic over the key type so the bit-mapping + Dig
/// chord in [`control_state`](PlayerBindings::control_state) are Bevy-free and
/// unit-testable; `game` uses `K = KeyCode`.
///
/// `dig` is `Option<K>` because it is **unbound by default** (spec §2): the C++
/// defaults leave `controls_ex[kDig]` zeroed (`settings.cpp:41-45` loops `j < 7`;
/// `worm.hpp:62` `WormSettingsExtensions()` zeroes the array), so players dig by
/// holding Left+Right (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBindings<K> {
    pub up: K,
    pub down: K,
    pub left: K,
    pub right: K,
    pub fire: K,
    pub change: K,
    pub jump: K,
    /// Optional Dig key — unbound by default (§2). When bound and held, it OR's
    /// into Left AND Right (§3); it is never a stored bit.
    pub dig: Option<K>,
}

impl<K: Copy> PlayerBindings<K> {
    /// This worm's [`ControlState`] for one tick, level-triggered from the
    /// held-key query `pressed`.
    ///
    /// Dig is a **pure Left+Right chord, never a stored bit**: it OR's into the
    /// Left and Right bits at sample time. This is the steady state of the C++
    /// `LocalController::OnKey` chord expansion (`localController.cpp:69-79`:
    /// `if clean[kDig] { Press(kLeft); Press(kRight); } else { release L/R if
    /// their own key is up }`), which is exactly
    /// `Left = pressed(left) || pressed(dig)`, `Right = pressed(right) ||
    /// pressed(dig)`. `kDig` (`worm.hpp:45-55`, index 7) lives outside the seven
    /// packed bits, so the produced word is always 7-bit (`pack() < 0x80`).
    pub fn control_state(&self, pressed: impl Fn(K) -> bool) -> ControlState {
        let dig = self.dig.is_some_and(|k| pressed(k));
        let mut cs = ControlState::new();
        cs.set(ControlState::UP, pressed(self.up));
        cs.set(ControlState::DOWN, pressed(self.down));
        cs.set(ControlState::LEFT, pressed(self.left) || dig);
        cs.set(ControlState::RIGHT, pressed(self.right) || dig);
        cs.set(ControlState::FIRE, pressed(self.fire));
        cs.set(ControlState::CHANGE, pressed(self.change));
        cs.set(ControlState::JUMP, pressed(self.jump));
        cs
    }
}

/// The full keyboard binding set — one [`PlayerBindings`] per worm, positional
/// by worm index (the same index `process_frame`/`Viewport::worm_idx` read).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputMap {
    pub players: Vec<PlayerBindings<KeyCode>>,
}

/// The C++ default bindings (spec §2), decoded from the DOS scancodes in
/// `settings.cpp:36-37` through the `liero_to_sdl_keys[]` table (`keys.cpp:9-58`)
/// to the SDL scancode, then to the Bevy 0.19 `KeyCode`. **Dig is unbound** for
/// both worms (`settings.cpp:41-45` defaults only `kUp..kJump`; `worm.hpp:62`
/// leaves `controls_ex[kDig]` zeroed) — players dig by holding Left+Right (§3).
///
/// Player 0 is the `R/F/D/G` diamond (Up/Down/Left/Right around `F`); Player 1
/// is the arrow cluster.
pub fn default_bindings() -> InputMap {
    InputMap {
        players: vec![
            // Player 0 (worm 0) — DOS scancode -> SDL scancode -> Bevy KeyCode.
            PlayerBindings {
                up: KeyCode::KeyR,          // 0x13 -> SDL_SCANCODE_R -> KeyR
                down: KeyCode::KeyF,        // 0x21 -> SDL_SCANCODE_F -> KeyF
                left: KeyCode::KeyD,        // 0x20 -> SDL_SCANCODE_D -> KeyD
                right: KeyCode::KeyG,       // 0x22 -> SDL_SCANCODE_G -> KeyG
                fire: KeyCode::ControlLeft, // 0x1D -> SDL_SCANCODE_LCTRL -> ControlLeft
                change: KeyCode::ShiftLeft, // 0x2A -> SDL_SCANCODE_LSHIFT -> ShiftLeft
                jump: KeyCode::AltLeft,     // 0x38 -> SDL_SCANCODE_LALT -> AltLeft
                dig: None,                  // unbound in C++ defaults (§2)
            },
            // Player 1 (worm 1) — DOS scancode -> SDL scancode -> Bevy KeyCode.
            PlayerBindings {
                up: KeyCode::ArrowUp,        // 0xA0 -> SDL_SCANCODE_UP -> ArrowUp
                down: KeyCode::ArrowDown,    // 0xA8 -> SDL_SCANCODE_DOWN -> ArrowDown
                left: KeyCode::ArrowLeft,    // 0xA3 -> SDL_SCANCODE_LEFT -> ArrowLeft
                right: KeyCode::ArrowRight,  // 0xA5 -> SDL_SCANCODE_RIGHT -> ArrowRight
                fire: KeyCode::ControlRight, // 0x75 -> SDL_SCANCODE_RCTRL -> ControlRight
                change: KeyCode::AltRight,   // 0x90 -> SDL_SCANCODE_RALT -> AltRight
                jump: KeyCode::ShiftRight,   // 0x36 -> SDL_SCANCODE_RSHIFT -> ShiftRight
                dig: None,                   // unbound in C++ defaults (§2)
            },
        ],
    }
}

/// Where a tick's `[ControlState; N_WORMS]` snapshot comes from — the 4b-extensible
/// seam (spec §4.2). `Scripted` makes the existing recorded scenario path a literal
/// pass-through source (Bevy-free, drives the pass-through determinism gate);
/// `Live` polls the held-key set through the per-worm [`PlayerBindings`].
///
/// 4b's `--replay <path>` (T2) does **not** add a `Replay` arm here: a
/// recorded/replayed file IS a scenario (spec §3), so replay is `Scripted`
/// verbatim over the file parsed from an arbitrary path — no new source
/// variant, no new replay engine (spec §5). [`Mode::Replay`] is the CLI-level
/// distinction (loop/self-check retired); the sampler stays two-armed.
#[derive(Resource)]
pub enum InputSource {
    /// Recorded scenario inputs — the existing 3c feed (`main.rs:257-261`), now
    /// behind the source. Ignores live keys. Also the `--replay` source (4b,
    /// T2), fed the parsed replay-file scenario instead of a committed golden.
    Scripted(Scenario),
    /// Live keyboard: one [`PlayerBindings`] per worm, polled level-triggered.
    Live(InputMap),
}

/// The CLI-selected run mode (spec §7/§9, T2; `Replay` added 4b T2). `Scripted`
/// is the existing 3c default: recorded inputs, the loop/reload, and the debug
/// determinism self-check. `Live` swaps the source for the keyboard and has no
/// golden to check or loop against, so both are retired for it. `Replay` also
/// feeds `InputSource::Scripted`, but over an **arbitrary** scenario file named
/// by `--replay <path>` (bypassing the golden-dir name validation the
/// positional `<name>` keeps) — it has no committed golden either, so the
/// self-check is retired the same way as `Live`, and it has no loop (plays once
/// through `ticks`, then holds on the final frame — spec §5). Native-only:
/// wasm's `resolve_scenario` hard-codes `Scripted` (no CLI args there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub enum Mode {
    Scripted,
    Live,
    Replay,
}

/// The scenario `name` [`parse_args`] returns for a **bare** invocation
/// (`cargo run -p game`, no flags and no positional) — the Slice-4f playable
/// default match. It is a sentinel, not a committed golden name: the caller
/// (`main.rs::resolve_scenario`) treats it specially — skipping the golden-dir
/// name validation the positional `<name>` keeps — and `setup` sources the
/// committed `scenarios/default_match.txt` fixture for it (Mode::Live, so no
/// golden column and no self-check). Chosen to never collide with a
/// `render_slice3b_<name>` golden, so a stray positional `default_match` is
/// still rejected by `available_scenarios` before it can reach `setup`.
pub const DEFAULT_MATCH: &str = "default_match";

/// The parsed native CLI args (spec §7): the run [`Mode`], the scenario `name`
/// (positional, defaulting to the caller's default), and the optional 4b
/// `--record <path>` / `--replay <path>` flush/load targets. Bevy-free and pure
/// — `name` is not validated here (the caller re-uses `available_scenarios`),
/// and the "record requires live" / "replay excludes live/record" rules are
/// enforced by the caller (`main.rs::resolve_scenario`), not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArgs {
    pub mode: Mode,
    pub name: String,
    /// `Some(path)` iff `--record <path>` was given (4b, T1) — the scenario file
    /// the recorder flushes on exit. Live-mode only (enforced by the caller).
    pub record: Option<PathBuf>,
    /// `Some(path)` iff `--replay <path>` was given (4b, T2) — an arbitrary
    /// scenario file (not necessarily a committed golden) to play back windowed,
    /// guard/loop off. Mutually exclusive with `--live`/`--record` (enforced by
    /// the caller).
    pub replay: Option<PathBuf>,
}

/// A syntactic error `parse_args` can detect on its own, with no external
/// state (spec §7, T1 review fix). Semantic validation that needs the
/// caller's context — e.g. "`--record` requires `--live`" — stays the
/// caller's job (`main.rs::resolve_scenario`), not this parser's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseArgsError {
    /// `--record` was the last token — no path token followed it. Previously
    /// this silently parsed as `record: None`, so a dropped/typo'd path was
    /// indistinguishable from not passing `--record` at all.
    RecordMissingPath,
    /// `--replay` was the last token — no path token followed it (4b, T2),
    /// mirroring [`ParseArgsError::RecordMissingPath`].
    ReplayMissingPath,
}

/// Parse the native CLI args (post `argv[0]`): leading flags — `--live`
/// (selects [`Mode::Live`]), `--record <path>` (4b, T1), and `--replay <path>`
/// (4b, T2) — in any order, then an optional positional scenario name
/// defaulting to `default_name` (spec §7). Pure and Bevy-free.
/// `Err(ParseArgsError::RecordMissingPath)` / `Err(ParseArgsError::ReplayMissingPath)`
/// if `--record`/`--replay` has no following path token. The "`--replay`
/// excludes `--live`/`--record`" combination rule needs no external state, but
/// (like "record requires live") stays the caller's job (`main.rs::resolve_scenario`)
/// for consistency — this parser only builds the flag shape.
pub fn parse_args<I: IntoIterator<Item = String>>(
    args: I,
    default_name: &str,
) -> Result<ParsedArgs, ParseArgsError> {
    let mut it = args.into_iter().peekable();
    let mut mode = Mode::Scripted;
    let mut record: Option<PathBuf> = None;
    let mut replay: Option<PathBuf> = None;
    // Whether any leading flag was consumed — distinguishes a truly bare
    // invocation (the 4f default match) from `--live` (Live over a named
    // scenario, default `blood`).
    let mut saw_flag = false;
    while let Some(arg) = it.peek().map(String::as_str) {
        match arg {
            "--live" => {
                it.next();
                mode = Mode::Live;
                saw_flag = true;
            }
            "--record" => {
                it.next();
                // The following token is the flush path — required, not optional;
                // a bare trailing `--record` is a malformed CLI, not "no target".
                let path = it.next().ok_or(ParseArgsError::RecordMissingPath)?;
                record = Some(PathBuf::from(path));
                saw_flag = true;
            }
            "--replay" => {
                it.next();
                // Same "required, not optional" rule as `--record`.
                let path = it.next().ok_or(ParseArgsError::ReplayMissingPath)?;
                replay = Some(PathBuf::from(path));
                saw_flag = true;
            }
            _ => break,
        }
    }
    let positional = it.next();
    // Slice 4f: a bare invocation — no leading flag AND no positional name —
    // is the playable default match: `Mode::Live` over the [`DEFAULT_MATCH`]
    // sentinel scenario. `main.rs::resolve_scenario` reads the sentinel to source
    // the committed `scenarios/default_match.txt` fixture (bypassing the
    // golden-dir name validation). `--live` / `--replay` / `--record` / a
    // positional `<name>` all take their existing paths, unchanged.
    if !saw_flag && positional.is_none() {
        return Ok(ParsedArgs {
            mode: Mode::Live,
            name: DEFAULT_MATCH.to_string(),
            record: None,
            replay: None,
        });
    }
    let name = positional.unwrap_or_else(|| default_name.to_string());
    Ok(ParsedArgs {
        mode,
        name,
        record,
        replay,
    })
}

impl InputSource {
    /// The single per-tick input snapshot (spec §4.2/§4.3), sampled once at the
    /// top of the FixedUpdate tick system.
    ///
    /// - `Scripted` is a **literal pass-through** of the scenario's recorded inputs
    ///   (`parser.rs:261-268`), masked through [`ControlState::unpack`] exactly as
    ///   the 3c inline feed did — `keys` is ignored, so the scripted path (and its
    ///   determinism gate) is byte-unchanged.
    /// - `Live` maps each worm's held-key set to a [`ControlState`] via the pure
    ///   sampler, with `ButtonInput::pressed` as the level-triggered "pressed"
    ///   query (spec §4.4).
    pub fn sample(&self, tick: u32, keys: &ButtonInput<KeyCode>) -> [ControlState; N_WORMS] {
        match self {
            InputSource::Scripted(scenario) => [
                ControlState::unpack(scenario.input(tick, 0)),
                ControlState::unpack(scenario.input(tick, 1)),
            ],
            InputSource::Live(map) => [
                map.players[0].control_state(|k| keys.pressed(k)),
                map.players[1].control_state(|k| keys.pressed(k)),
            ],
        }
    }
}

/// The 4b input recorder (spec §4.2): buffers the per-tick **sampled**
/// `[ControlState; N_WORMS]` array — tapped at the 4a seam, *after* `sample` and
/// *before* `process_frame` (`main.rs:290-292`), so the Dig→Left+Right chord is
/// already resolved into the words the sim saw — and, on exit, builds a scenario
/// file that replays byte-for-byte through `InputSource::Scripted`.
///
/// Live-mode only: `Scripted` already *is* a recorded stream, so recording it is
/// the vacuous case (spec §6). No `Recorder` is inserted in Scripted mode, so the
/// scripted path and its 4a pass-through gate stay byte-unchanged.
///
/// Bevy-free-testable: the buffer + the scenario build carry no Bevy types (only
/// the `Resource` derive and the thin flush system in `main.rs` touch Bevy).
///
/// **Flush caveat (spec §4.2/§9):** the stream is held in memory and written once
/// on *graceful* exit (Esc / window close → `AppExit`). A hard crash or a signal
/// kill (SIGTERM/SIGALRM — e.g. a `timeout`/`alarm` smoke test) bypasses the flush
/// and writes no file. Acceptable for a dev/test artifact; a periodic flush is a
/// deferred optimization.
#[derive(Resource)]
pub struct Recorder {
    /// The base scenario the live match was launched from — its tick-0 metadata
    /// (seed / level / worms / loadout / settings) is carried verbatim into the
    /// recording so replay reconstructs the identical initial state.
    base: Scenario,
    /// One `[ControlState; N_WORMS]` snapshot per recorded tick, in tick order.
    snapshots: Vec<[ControlState; N_WORMS]>,
    /// The `--record <path>` flush target.
    path: PathBuf,
}

impl Recorder {
    /// A recorder over `base`'s tick-0 metadata, flushing to `path` on exit.
    pub fn new(base: Scenario, path: PathBuf) -> Self {
        Recorder {
            base,
            snapshots: Vec::new(),
            path,
        }
    }

    /// Tap one tick's sampled array (called at the 4a seam, Live only).
    pub fn record(&mut self, inputs: &[ControlState; N_WORMS]) {
        self.snapshots.push(*inputs);
    }

    /// Build the recording scenario: each snapshot's two `ControlState`s are
    /// `pack()`ed to 7-bit words and handed to `Scenario::with_recorded_inputs`
    /// (positional by worm index; `ticks` = snapshot count; sparse storage).
    pub fn build(&self) -> Scenario {
        let per_tick: Vec<(u32, u32)> = self
            .snapshots
            .iter()
            .map(|snap| (snap[0].pack(), snap[1].pack()))
            .collect();
        self.base.with_recorded_inputs(&per_tick)
    }

    /// The `--record <path>` flush target.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Empty the buffered snapshots (4f T0 fix: an F5 restart in Live mode).
    ///
    /// **F5 semantics:** F5 does not merely reload the sim — it also RESTARTS
    /// the recording. `snapshots` is cleared, so the eventual `build()` covers
    /// only ticks recorded SINCE the latest restart; `base`/`path` are left
    /// untouched (the restart reloads the same base scenario, so its tick-0
    /// metadata is identical to what a fresh recording would carry — replaying
    /// the built file reproduces the session from the last F5, not the
    /// original launch). Without this, snapshots kept appending across the
    /// restart boundary and the replay silently diverged from the live match
    /// at the point of restart (see the flush caveat above for the sibling
    /// caveat on *when* the buffer is written).
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }
}

/// Step 4½c (design §7.2, Q5): the release latch at both phase boundaries. C++ keys are EDGES:
/// a key held when the controller starts never reaches the worm, and a key held when weapon
/// selection ends (Fire from DONE) does nothing in the match until pressed again
/// (`ReleaseControls`, `game.cpp:110-118`; SDL repeats are dropped, `gfx.cpp:608`). Rust samples
/// LEVELS, so without this a held DONE would fire the first weapon on tick 0. Armed with the
/// held words at a boundary; each tick it forgets released bits (`mask &= held`) and outputs
/// `sampled & !mask`. It sits BEFORE the recorder tap; it is live-only, and only selection
/// boundaries arm it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReleaseLatch {
    mask: [u32; N_WORMS],
}

impl ReleaseLatch {
    /// Latch every bit held now.
    pub fn arm(&mut self, held: &[ControlState; N_WORMS]) {
        self.mask = held.map(|c| c.pack());
    }

    /// Mask this tick's sampled words in place.
    pub fn apply(&mut self, inputs: &mut [ControlState; N_WORMS]) {
        for (mask, input) in self.mask.iter_mut().zip(inputs.iter_mut()) {
            *mask &= input.pack();
            *input = ControlState::unpack(input.pack() & !*mask);
        }
    }

    /// Whether any bit is still latched.
    pub fn is_armed(&self) -> bool {
        self.mask.iter().any(|&m| m != 0)
    }
}

/// Headless per-tick `hash_game_state` time series for `scenario`, driven
/// entirely through `InputSource::Scripted` (spec §5, 4b T2) — the
/// library-testable twin of `tests/passthrough.rs`'s harness and the building
/// block both the T3 round-trip gate and `--replay <path>` (windowed) reuse.
/// No Bevy window/app: `scenario::load` reconstructs tick-0, then each tick
/// `k` in `1..=scenario.ticks` samples `Scripted`'s recorded input for `k - 1`
/// (an empty `ButtonInput` — Scripted ignores keys) and feeds it into
/// `process_frame`. `series[0]` is tick 0, hashed BEFORE any `process_frame`,
/// so `series.len() == scenario.ticks as usize + 1`, matching the golden
/// sidecar's `<tick> <frame_hash> <state_hash>` grammar (index = tick).
pub fn replay_state_series(tc_root: &Path, scenario: &Scenario) -> Vec<u32> {
    let mut state = scenario::load(tc_root, scenario).state;
    let source = InputSource::Scripted(scenario.clone());
    let empty = ButtonInput::<KeyCode>::default();

    let mut series = Vec::with_capacity(scenario.ticks as usize + 1);
    series.push(sim::hash::hash_game_state(&state));
    for k in 1..=scenario.ticks {
        let inputs = source.sample(k - 1, &empty);
        state.process_frame(&inputs);
        series.push(sim::hash::hash_game_state(&state));
    }
    series
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Mock key type: a small `Copy` string tag driving a `HashSet` "pressed"
    /// closure — no Bevy, no window.
    type Key = &'static str;

    const UP: Key = "up";
    const DOWN: Key = "down";
    const LEFT: Key = "left";
    const RIGHT: Key = "right";
    const FIRE: Key = "fire";
    const CHANGE: Key = "change";
    const JUMP: Key = "jump";
    const DIG: Key = "dig";

    /// Bindings over the mock keys, with an optional Dig key.
    fn bindings(dig: Option<Key>) -> PlayerBindings<Key> {
        PlayerBindings {
            up: UP,
            down: DOWN,
            left: LEFT,
            right: RIGHT,
            fire: FIRE,
            change: CHANGE,
            jump: JUMP,
            dig,
        }
    }

    /// Sample the bindings with exactly the given keys held.
    fn sample(b: &PlayerBindings<Key>, held: &[Key]) -> ControlState {
        let set: HashSet<Key> = held.iter().copied().collect();
        b.control_state(|k| set.contains(k))
    }

    /// (a) Each of the seven bits is set iff — and only iff — its own key is held.
    #[test]
    fn each_bit_set_iff_its_key_pressed() {
        let b = bindings(None);
        let cases = [
            (UP, ControlState::UP),
            (DOWN, ControlState::DOWN),
            (LEFT, ControlState::LEFT),
            (RIGHT, ControlState::RIGHT),
            (FIRE, ControlState::FIRE),
            (CHANGE, ControlState::CHANGE),
            (JUMP, ControlState::JUMP),
        ];
        for (key, bit) in cases {
            let cs = sample(&b, &[key]);
            for (_, other) in cases {
                let want = other == bit;
                assert_eq!(
                    cs.get(other),
                    want,
                    "holding {key:?}: bit {other} should be {want}"
                );
            }
        }
        // Nothing held => empty state.
        assert_eq!(sample(&b, &[]), ControlState::new());
    }

    /// (b) Dig held => Left AND Right set, even with neither movement key down.
    #[test]
    fn dig_held_sets_left_and_right() {
        let b = bindings(Some(DIG));
        let cs = sample(&b, &[DIG]);
        assert!(cs.get(ControlState::LEFT), "Dig held => Left set");
        assert!(cs.get(ControlState::RIGHT), "Dig held => Right set");
        // Only Left+Right, nothing else.
        assert!(!cs.get(ControlState::UP));
        assert!(!cs.get(ControlState::DOWN));
        assert!(!cs.get(ControlState::FIRE));
        assert!(!cs.get(ControlState::CHANGE));
        assert!(!cs.get(ControlState::JUMP));
    }

    /// (c) Dig released => Left/Right follow their own keys again.
    #[test]
    fn dig_released_left_right_follow_own_keys() {
        let b = bindings(Some(DIG));
        // Only Left held (Dig up): Left set, Right clear.
        let cs = sample(&b, &[LEFT]);
        assert!(cs.get(ControlState::LEFT));
        assert!(!cs.get(ControlState::RIGHT));
        // Nothing held (Dig up): both clear — chord does not linger.
        let cs = sample(&b, &[]);
        assert!(!cs.get(ControlState::LEFT));
        assert!(!cs.get(ControlState::RIGHT));
    }

    /// (d) `dig: None` => the chord never fires; Left/Right are purely their keys.
    #[test]
    fn unbound_dig_never_chords() {
        let b = bindings(None);
        // Even holding a key literally named "dig" does nothing (no binding).
        let cs = sample(&b, &[DIG]);
        assert!(!cs.get(ControlState::LEFT));
        assert!(!cs.get(ControlState::RIGHT));
    }

    /// (e) No 8th bit is ever produced — `pack() < 0x80` even with everything
    /// (incl. Dig) held.
    #[test]
    fn pack_is_always_seven_bit() {
        let b = bindings(Some(DIG));
        let all = [UP, DOWN, LEFT, RIGHT, FIRE, CHANGE, JUMP, DIG];
        let cs = sample(&b, &all);
        assert!(cs.pack() < 0x80, "pack() = {:#x} must be 7-bit", cs.pack());
        // All seven bits actually set (the maximal 7-bit word).
        assert_eq!(cs.pack(), 0x7f);
    }

    /// `InputSource::Scripted` is a **literal pass-through** of the scenario's
    /// recorded inputs (spec §4.2): for every tick, `sample` returns exactly
    /// `[unpack(scn.input(t,0)), unpack(scn.input(t,1))]`, and an empty
    /// `ButtonInput` is ignored (Scripted never reads keys). This is the sampler
    /// half of the pass-through determinism gate (spec §5.1).
    #[test]
    fn scripted_sample_is_scenario_passthrough() {
        // A minimal parsed scenario with sparse per-tick input overrides. The
        // absence of an `input` line for a tick is "no keys" (parser.rs:262).
        let text = "\
seed 1
level foo/bar
ticks 6
input 0 5 3
input 2 127 0
input 5 64 96
";
        let scn = Scenario::parse(text).expect("scenario parses");
        let source = InputSource::Scripted(scn.clone());
        let empty = ButtonInput::<KeyCode>::default();

        for t in 0..scn.ticks {
            let got = source.sample(t, &empty);
            let want = [
                ControlState::unpack(scn.input(t, 0)),
                ControlState::unpack(scn.input(t, 1)),
            ];
            assert_eq!(got, want, "Scripted pass-through diverged at tick {t}");
        }

        // Spot-check the concrete decoded values so a silent unpack/index swap
        // is caught, not just self-consistency.
        assert_eq!(
            source.sample(0, &empty),
            [ControlState::unpack(5), ControlState::unpack(3)]
        );
        assert_eq!(
            source.sample(2, &empty),
            [ControlState::unpack(127), ControlState::unpack(0)]
        );
        // A tick with no `input` line => both worms empty.
        assert_eq!(
            source.sample(1, &empty),
            [ControlState::new(), ControlState::new()]
        );
    }

    /// `parse_args` (spec §7): a leading `--live` flag selects `Mode::Live`;
    /// otherwise `Mode::Scripted`. The remaining positional arg is the scenario
    /// name, defaulting to the caller-supplied default when absent. With no
    /// `--record`, the record target is `None`. (The **bare** no-arg case is the
    /// 4f default match — covered separately by `parse_args_bare_is_default_match`.)
    #[test]
    fn parse_args_live_flag_and_scenario_name() {
        let cases: [(&[&str], Mode, &str); 3] = [
            (&["--live"], Mode::Live, "blood"),
            (&["--live", "dart"], Mode::Live, "dart"),
            (&["dart"], Mode::Scripted, "dart"),
        ];
        for (args, want_mode, want_name) in cases {
            let p = parse_args(args.iter().map(|s| s.to_string()), "blood").unwrap();
            assert_eq!(p.mode, want_mode, "args {args:?}: mode");
            assert_eq!(p.name, want_name, "args {args:?}: name");
            assert_eq!(p.record, None, "args {args:?}: no --record => record None");
            assert_eq!(p.replay, None, "args {args:?}: no --replay => replay None");
        }
    }

    /// Slice 4f: a **bare** invocation (no flags AND no positional name) resolves
    /// to the playable default match — `Mode::Live` over the [`DEFAULT_MATCH`]
    /// sentinel — while a positional `<name>` still resolves `Mode::Scripted`
    /// (the self-check demo path) and `--live` alone stays Live over the caller's
    /// default name (NOT the default match). This is the parse-level pin of the
    /// default-mode flip; `main.rs::resolve_scenario` reads the sentinel to source
    /// the `scenarios/default_match.txt` fixture and skip golden-name validation.
    #[test]
    fn parse_args_bare_is_default_match() {
        // Bare: no args at all => Live over the default-match sentinel.
        let p = parse_args(std::iter::empty::<String>(), "blood").unwrap();
        assert_eq!(
            p.mode,
            Mode::Live,
            "bare invocation is the playable default match (Live)"
        );
        assert_eq!(
            p.name, DEFAULT_MATCH,
            "bare invocation names the default-match fixture"
        );
        assert_eq!(p.record, None, "bare invocation has no --record");
        assert_eq!(p.replay, None, "bare invocation has no --replay");

        // A positional name still resolves Scripted (self-check demo unaffected).
        let p = parse_args(["blood"].iter().map(|s| s.to_string()), "blood").unwrap();
        assert_eq!(p.mode, Mode::Scripted, "positional <name> stays Scripted");
        assert_eq!(p.name, "blood");

        // `--live` alone is Live over the DEFAULT name (blood), not the default match.
        let p = parse_args(["--live"].iter().map(|s| s.to_string()), "blood").unwrap();
        assert_eq!(p.mode, Mode::Live, "--live is Live");
        assert_eq!(
            p.name, "blood",
            "--live alone uses the caller default, not the sentinel"
        );
    }

    /// `--record <path>` (4b, T1) is parsed into `ParsedArgs::record`, alongside
    /// `--live`, with the positional scenario name still recognized after it.
    #[test]
    fn parse_args_record_flag_carries_path() {
        // --live --record <path>: Live, default name, record set.
        let p = parse_args(
            ["--live", "--record", "/tmp/r.txt"]
                .iter()
                .map(|s| s.to_string()),
            "blood",
        )
        .unwrap();
        assert_eq!(p.mode, Mode::Live);
        assert_eq!(p.name, "blood");
        assert_eq!(p.record, Some(PathBuf::from("/tmp/r.txt")));

        // --record before the positional name still leaves the name positional.
        let p = parse_args(
            ["--live", "--record", "/tmp/r.txt", "dart"]
                .iter()
                .map(|s| s.to_string()),
            "blood",
        )
        .unwrap();
        assert_eq!(p.name, "dart");
        assert_eq!(p.record, Some(PathBuf::from("/tmp/r.txt")));

        // Plain --live => no record target.
        let p = parse_args(["--live"].iter().map(|s| s.to_string()), "blood").unwrap();
        assert_eq!(p.record, None);
    }

    /// `--replay <path>` (4b, T2) is parsed into `ParsedArgs::replay`, leaving
    /// `mode`/`record` at their defaults when given alone — replay's mode is
    /// resolved by the caller (Scripted, via `InputSource::Scripted`), not by
    /// this parser.
    #[test]
    fn parse_args_replay_flag_carries_path() {
        let p = parse_args(
            ["--replay", "/tmp/x.txt"].iter().map(|s| s.to_string()),
            "blood",
        )
        .unwrap();
        assert_eq!(
            p.mode,
            Mode::Scripted,
            "--replay alone leaves mode Scripted"
        );
        assert_eq!(p.name, "blood");
        assert_eq!(p.record, None);
        assert_eq!(p.replay, Some(PathBuf::from("/tmp/x.txt")));

        // The positional name still parses after --replay <path>.
        let p = parse_args(
            ["--replay", "/tmp/x.txt", "dart"]
                .iter()
                .map(|s| s.to_string()),
            "blood",
        )
        .unwrap();
        assert_eq!(p.name, "dart");
        assert_eq!(p.replay, Some(PathBuf::from("/tmp/x.txt")));

        // No --replay => None, matching the --record default-None precedent.
        let p = parse_args(["--live"].iter().map(|s| s.to_string()), "blood").unwrap();
        assert_eq!(p.replay, None);
    }

    /// `--replay` with no following path token (4b, T2): mirrors
    /// `parse_args_record_missing_path_is_error` — a bare trailing `--replay`
    /// must not silently parse as "no replay target".
    #[test]
    fn parse_args_replay_missing_path_is_error() {
        let err = parse_args(["--replay"].iter().map(|s| s.to_string()), "blood").unwrap_err();
        assert_eq!(err, ParseArgsError::ReplayMissingPath);
    }

    /// `--record` with no following path token (spec §7, T1 review fix): a bare
    /// trailing `--record` must not silently parse as "no record target" — that
    /// would let `--record` typos through unnoticed. `parse_args` reports it as
    /// an error, which `main.rs::resolve_scenario` turns into an `eprintln!` +
    /// `exit(2)`, the same pattern as the "--record requires --live" check.
    #[test]
    fn parse_args_record_missing_path_is_error() {
        let err = parse_args(
            ["--live", "--record"].iter().map(|s| s.to_string()),
            "blood",
        )
        .unwrap_err();
        assert_eq!(err, ParseArgsError::RecordMissingPath);
    }

    /// A `Recorder` fed a hand-built sequence of `[ControlState; N]` arrays
    /// `build()`s a scenario whose per-tick `input(t, w)` equals each fed word's
    /// `pack()` and whose `ticks` equals the sequence length (spec §4.2). The
    /// tick-0 metadata is carried verbatim from the base, and the built scenario
    /// is a valid file (round-trips through `to_text`).
    #[test]
    fn recorder_build_maps_snapshots_to_scenario_inputs() {
        use std::path::{Path, PathBuf};

        let base = Scenario::parse("seed 7\nlevel a.lev\nticks 0\n").expect("base parses");
        let mut rec = Recorder::new(base.clone(), PathBuf::from("/tmp/rec.txt"));

        let mut fire = ControlState::new();
        fire.set(ControlState::FIRE, true);
        let mut left = ControlState::new();
        left.set(ControlState::LEFT, true);

        // tick 0: worm0 fire, worm1 idle; tick 1: both idle; tick 2: worm0 left, worm1 fire.
        rec.record(&[fire, ControlState::new()]);
        rec.record(&[ControlState::new(), ControlState::new()]);
        rec.record(&[left, fire]);

        let built = rec.build();
        assert_eq!(built.ticks, 3, "ticks == recorded snapshot count");
        assert_eq!(built.input(0, 0), fire.pack());
        assert_eq!(built.input(0, 1), 0);
        // The all-idle middle tick decodes to (0,0) from absence (sparse storage).
        assert_eq!(built.input(1, 0), 0);
        assert_eq!(built.input(1, 1), 0);
        assert_eq!(built.input(2, 0), left.pack());
        assert_eq!(built.input(2, 1), fire.pack());
        // Tick-0 metadata carried verbatim from the base scenario.
        assert_eq!(built.seed, base.seed);
        assert_eq!(built.level, base.level);
        // The built scenario is a valid file: it round-trips through text.
        assert_eq!(Scenario::parse(&built.to_text()).unwrap(), built);
        // The recorder targets the CLI path.
        assert_eq!(rec.path(), Path::new("/tmp/rec.txt"));
    }

    /// `Recorder::clear` (4f T0 fix): an F5 restart nukes the buffered
    /// snapshots so the recording begins again from the restart point,
    /// instead of silently splicing the pre-restart and post-restart streams
    /// into one incoherent replay. Snapshots recorded before `clear()` must
    /// not surface in `build()`; snapshots recorded after must.
    #[test]
    fn recorder_clear_resets_buffered_snapshots() {
        use std::path::PathBuf;

        let base = Scenario::parse("seed 7\nlevel a.lev\nticks 0\n").expect("base parses");
        let mut rec = Recorder::new(base, PathBuf::from("/tmp/rec.txt"));

        let mut fire = ControlState::new();
        fire.set(ControlState::FIRE, true);

        // Pre-restart: two ticks buffered.
        rec.record(&[fire, ControlState::new()]);
        rec.record(&[fire, ControlState::new()]);
        assert_eq!(rec.build().ticks, 2, "pre-clear: two buffered ticks");

        rec.clear();
        assert_eq!(rec.build().ticks, 0, "clear empties the buffer");
        assert_eq!(
            rec.build().input(0, 0),
            0,
            "clear leaves no stale snapshot to read back"
        );

        // Post-restart: the buffer accumulates fresh from zero.
        let mut left = ControlState::new();
        left.set(ControlState::LEFT, true);
        rec.record(&[left, ControlState::new()]);
        rec.record(&[left, fire]);
        let built = rec.build();
        assert_eq!(built.ticks, 2, "post-clear: only the two new ticks count");
        assert_eq!(built.input(0, 0), left.pack());
        assert_eq!(built.input(1, 1), fire.pack());
    }

    /// Original-Liero TC data root (relative to this crate's manifest — same
    /// resolution `main.rs`/`tests/passthrough.rs` use).
    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
    /// Committed golden dir (mirrors `tests/passthrough.rs::read_golden`).
    const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden");

    /// The committed `render_slice3b_<name>` golden's per-tick `state_hash`
    /// column (3rd column, hex `u32`) — index = tick. Same grammar as
    /// `tests/passthrough.rs::golden_state_hashes`; duplicated here (rather than
    /// shared) because `tests/` integration tests and this `src`-internal unit
    /// test compile as separate crates.
    fn golden_state_hashes(name: &str) -> Vec<u32> {
        let path = format!("{GOLDEN_DIR}/render_slice3b_{name}.txt");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let mut states = Vec::new();
        for l in text.lines() {
            let t = l.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let mut it = t.split_whitespace();
            if it.next() == Some("total") {
                continue;
            }
            let _frame_hash = it.next().expect("frame_hash column");
            let state_hash = it.next().expect("state_hash column");
            states.push(u32::from_str_radix(state_hash, 16).expect("state_hash hex"));
        }
        states
    }

    /// `replay_state_series` (headless, 4b T2) over a committed 3b scenario must
    /// reproduce that scenario's golden `state_hash` column exactly — proving the
    /// library-testable replay path is equivalent to the pass-through gate
    /// (`tests/passthrough.rs`) T3's round-trip gate builds on.
    #[test]
    fn replay_state_series_matches_passthrough_golden() {
        let scenario_path = format!("{GOLDEN_DIR}/render_slice3b_blood_scenario.txt");
        let scenario_text = std::fs::read_to_string(&scenario_path)
            .unwrap_or_else(|e| panic!("read {scenario_path}: {e}"));
        let scenario = Scenario::parse(&scenario_text).expect("scenario parses");

        let golden = golden_state_hashes("blood");
        assert_eq!(
            golden.len(),
            (scenario.ticks + 1) as usize,
            "one golden state_hash per tick 0..=ticks"
        );

        let series = replay_state_series(Path::new(TC_ROOT), &scenario);
        assert_eq!(
            series, golden,
            "replay_state_series diverged from the passthrough golden"
        );
    }

    /// The default table mirrors the decoded C++ defaults exactly (spec §2),
    /// including `dig: None` for both worms.
    #[test]
    fn default_bindings_match_cpp_table() {
        let map = default_bindings();
        assert_eq!(map.players.len(), 2, "two default worms");

        let expected = InputMap {
            players: vec![
                PlayerBindings {
                    up: KeyCode::KeyR,
                    down: KeyCode::KeyF,
                    left: KeyCode::KeyD,
                    right: KeyCode::KeyG,
                    fire: KeyCode::ControlLeft,
                    change: KeyCode::ShiftLeft,
                    jump: KeyCode::AltLeft,
                    dig: None,
                },
                PlayerBindings {
                    up: KeyCode::ArrowUp,
                    down: KeyCode::ArrowDown,
                    left: KeyCode::ArrowLeft,
                    right: KeyCode::ArrowRight,
                    fire: KeyCode::ControlRight,
                    change: KeyCode::AltRight,
                    jump: KeyCode::ShiftRight,
                    dig: None,
                },
            ],
        };
        assert_eq!(map, expected);
        // Dig explicitly unbound for both (§2).
        assert_eq!(map.players[0].dig, None);
        assert_eq!(map.players[1].dig, None);
    }

    // ---- Step 4½c: the release latch (design §7.2) -----------------------------------

    fn cs(bits: u32) -> ControlState {
        ControlState::unpack(bits)
    }

    #[test]
    fn an_unarmed_latch_passes_everything() {
        let mut l = ReleaseLatch::default();
        let mut i = [cs(0x7f), cs(16)];
        l.apply(&mut i);
        assert_eq!((i[0].pack(), i[1].pack()), (0x7f, 16));
        assert!(!l.is_armed());
    }

    #[test]
    fn a_latched_key_does_nothing_until_released_then_a_repress_passes() {
        let mut l = ReleaseLatch::default();
        l.arm(&[cs(16), cs(0)]); // worm 0 held Fire at the boundary (the DONE press)
        for _ in 0..5 {
            let mut i = [cs(16 | 4), cs(16)];
            l.apply(&mut i);
            assert_eq!(
                (i[0].pack(), i[1].pack()),
                (4, 16),
                "only worm 0's Fire is masked"
            );
        }
        let mut i = [cs(0), cs(0)];
        l.apply(&mut i); // released
        assert!(!l.is_armed());
        let mut i = [cs(16), cs(0)];
        l.apply(&mut i);
        assert_eq!(
            i[0].pack(),
            16,
            "pressed again: it passes (gfx.cpp:608 + game.cpp:110-118)"
        );
    }
}
