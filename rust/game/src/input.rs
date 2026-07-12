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
/// `Live` polls the held-key set through the per-worm [`PlayerBindings`]. 4b later
/// adds a `Replay(Recording)` arm here, symmetric with `Scripted`.
#[derive(Resource)]
pub enum InputSource {
    /// Recorded scenario inputs — the existing 3c feed (`main.rs:257-261`), now
    /// behind the source. Ignores live keys.
    Scripted(Scenario),
    /// Live keyboard: one [`PlayerBindings`] per worm, polled level-triggered.
    Live(InputMap),
    // 4b adds: Replay(Recording) — reads back the recorded-input artifact,
    // symmetric with Scripted. (Not built in 4a.)
}

/// The CLI-selected run mode (spec §7/§9, T2). `Scripted` is the existing 3c
/// default: recorded inputs, the loop/reload, and the debug determinism
/// self-check. `Live` swaps the source for the keyboard and has no golden to
/// check or loop against, so both are retired for it — but kept, unchanged,
/// for `Scripted` (spec §9, the guard-retirement risk). Native-only: wasm's
/// `resolve_scenario` hard-codes `Scripted` (no CLI args there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub enum Mode {
    Scripted,
    Live,
}

/// The parsed native CLI args (spec §7): the run [`Mode`], the scenario `name`
/// (positional, defaulting to the caller's default), and the optional 4b
/// `--record <path>` flush target. Bevy-free and pure — `name` is not validated
/// here (the caller re-uses `available_scenarios`), and the "record requires
/// live" rule is enforced by the caller (`main.rs::resolve_scenario`), not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArgs {
    pub mode: Mode,
    pub name: String,
    /// `Some(path)` iff `--record <path>` was given (4b, T1) — the scenario file
    /// the recorder flushes on exit. Live-mode only (enforced by the caller).
    pub record: Option<PathBuf>,
}

/// Parse the native CLI args (post `argv[0]`): leading flags — `--live`
/// (selects [`Mode::Live`]) and `--record <path>` (4b, T1) — in any order,
/// then an optional positional scenario name defaulting to `default_name`
/// (spec §7). Pure and Bevy-free.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I, default_name: &str) -> ParsedArgs {
    let mut it = args.into_iter().peekable();
    let mut mode = Mode::Scripted;
    let mut record: Option<PathBuf> = None;
    while let Some(arg) = it.peek().map(String::as_str) {
        match arg {
            "--live" => {
                it.next();
                mode = Mode::Live;
            }
            "--record" => {
                it.next();
                // The following token is the flush path (absent => no target).
                record = it.next().map(PathBuf::from);
            }
            _ => break,
        }
    }
    let name = it.next().unwrap_or_else(|| default_name.to_string());
    ParsedArgs { mode, name, record }
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
    /// `--record`, the record target is `None`.
    #[test]
    fn parse_args_live_flag_and_scenario_name() {
        let cases: [(&[&str], Mode, &str); 4] = [
            (&["--live"], Mode::Live, "blood"),
            (&["--live", "dart"], Mode::Live, "dart"),
            (&["dart"], Mode::Scripted, "dart"),
            (&[], Mode::Scripted, "blood"),
        ];
        for (args, want_mode, want_name) in cases {
            let p = parse_args(args.iter().map(|s| s.to_string()), "blood");
            assert_eq!(p.mode, want_mode, "args {args:?}: mode");
            assert_eq!(p.name, want_name, "args {args:?}: name");
            assert_eq!(p.record, None, "args {args:?}: no --record => record None");
        }
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
        );
        assert_eq!(p.mode, Mode::Live);
        assert_eq!(p.name, "blood");
        assert_eq!(p.record, Some(PathBuf::from("/tmp/r.txt")));

        // --record before the positional name still leaves the name positional.
        let p = parse_args(
            ["--live", "--record", "/tmp/r.txt", "dart"]
                .iter()
                .map(|s| s.to_string()),
            "blood",
        );
        assert_eq!(p.name, "dart");
        assert_eq!(p.record, Some(PathBuf::from("/tmp/r.txt")));

        // Plain --live => no record target.
        let p = parse_args(["--live"].iter().map(|s| s.to_string()), "blood");
        assert_eq!(p.record, None);
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
}
