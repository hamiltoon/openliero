//! On-screen (touch) controls for the browser build.
//!
//! `web/index.html` draws a pad and buttons on touch devices and keeps the held
//! set in one global number, `window.lieroTouch`, using the bit layout below.
//! Once per tick, the live loop reads it and ORs the result into player 1's
//! sampled [`ControlState`], so touch and keyboard can be used together and the
//! sim cannot tell them apart: a touch session records and replays exactly like
//! a keyboard one.
//!
//! The page's layout is deliberately separate from `ControlState`'s packed bits,
//! so a change to either side is caught by [`touch_state`]'s tests, not by a
//! silently swapped key. Dig is a Left+Right chord, as it is on the keyboard
//! (`input::PlayerBindings::control_state`); it is never a stored bit.

use sim::state::ControlState;

/// The page's bit layout (`TOUCH` in `web/index.html` must match).
pub const TOUCH_UP: u32 = 1 << 0;
pub const TOUCH_DOWN: u32 = 1 << 1;
pub const TOUCH_LEFT: u32 = 1 << 2;
pub const TOUCH_RIGHT: u32 = 1 << 3;
pub const TOUCH_FIRE: u32 = 1 << 4;
pub const TOUCH_CHANGE: u32 = 1 << 5;
pub const TOUCH_JUMP: u32 = 1 << 6;
pub const TOUCH_DIG: u32 = 1 << 7;

/// The control state a touch mask holds (unknown bits are ignored).
pub fn touch_state(mask: u32) -> ControlState {
    let held = |bit: u32| mask & bit != 0;
    let dig = held(TOUCH_DIG);
    let mut cs = ControlState::new();
    cs.set(ControlState::UP, held(TOUCH_UP));
    cs.set(ControlState::DOWN, held(TOUCH_DOWN));
    cs.set(ControlState::LEFT, held(TOUCH_LEFT) || dig);
    cs.set(ControlState::RIGHT, held(TOUCH_RIGHT) || dig);
    cs.set(ControlState::FIRE, held(TOUCH_FIRE));
    cs.set(ControlState::CHANGE, held(TOUCH_CHANGE));
    cs.set(ControlState::JUMP, held(TOUCH_JUMP));
    cs
}

/// Player 1's input with the touch mask merged in (a button held on either
/// device counts as held).
pub fn merge(keyboard: ControlState, mask: u32) -> ControlState {
    ControlState::unpack(keyboard.pack() | touch_state(mask).pack())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_button_maps_to_its_control() {
        let cases = [
            (TOUCH_UP, ControlState::UP),
            (TOUCH_DOWN, ControlState::DOWN),
            (TOUCH_LEFT, ControlState::LEFT),
            (TOUCH_RIGHT, ControlState::RIGHT),
            (TOUCH_FIRE, ControlState::FIRE),
            (TOUCH_CHANGE, ControlState::CHANGE),
            (TOUCH_JUMP, ControlState::JUMP),
        ];
        for (bit, control) in cases {
            let mut want = ControlState::new();
            want.set(control, true);
            assert_eq!(touch_state(bit), want, "touch bit {bit:#x}");
        }
    }

    #[test]
    fn dig_is_the_left_right_chord_and_unknown_bits_are_ignored() {
        let mut lr = ControlState::new();
        lr.set(ControlState::LEFT, true);
        lr.set(ControlState::RIGHT, true);
        assert_eq!(touch_state(TOUCH_DIG), lr);
        assert_eq!(touch_state(0xFFFF_FF00), ControlState::new());
        assert!(touch_state(u32::MAX).pack() < 0x80, "always a 7-bit word");
    }

    #[test]
    fn merge_ors_touch_into_the_keyboard_state() {
        let mut keys = ControlState::new();
        keys.set(ControlState::JUMP, true);
        let merged = merge(keys, TOUCH_FIRE | TOUCH_LEFT);
        let mut want = keys;
        want.set(ControlState::FIRE, true);
        want.set(ControlState::LEFT, true);
        assert_eq!(merged, want);
        assert_eq!(merge(keys, 0), keys, "no touch leaves the keyboard alone");
    }
}
