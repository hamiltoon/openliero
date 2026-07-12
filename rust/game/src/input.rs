//! Live-input core (Slice 4a, T0): the **pure, Bevy-free** per-worm sampler.
//!
//! The correctness core — the 7-bit mapping and the synthetic Dig chord — is
//! made *generic over the key type* [`PlayerBindings<K>`] so it is unit-testable
//! with a mock key + a `HashSet` "pressed" closure (no window, no `ButtonInput`),
//! and so it runs in the fast CI test set. `game` instantiates it with Bevy's
//! `KeyCode` via [`default_bindings`]. See spec §4.1.

use bevy::input::keyboard::KeyCode;

use sim::state::ControlState;

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
