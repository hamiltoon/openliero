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
//!
//! Step 4½d: the controls also drive the menus (design §7.3), as key events on player 1's bound
//! DOS keys ([`touch_key_events`]): to the menu, touch is exactly player 1's keyboard. The new
//! MENU button (bit 8, [`TOUCH_MENU`]) is Esc; [`touch_state`] ignores it, so it never reaches
//! the sim.

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
/// Step 4½d (Q8): the MENU button — Esc. Ignored by [`touch_state`].
pub const TOUCH_MENU: u32 = 1 << 8;

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

/// The menu key events of a touch-mask change (design §7.3): each bit's rising edge is a key-down
/// of player 1's `controls_ex[bit]`, its falling edge a key-up; DIG is Left + Right; MENU is Esc.
/// Touch has no OS repeat. The caller wraps each in `ui::shell::InputEvent::Key` (Step 4½e-1).
pub fn touch_key_events(prev: u32, now: u32, controls_ex: &[u32; 8]) -> Vec<ui::shell::KeyEvent> {
    let keys = |bit: u32| -> Vec<u32> {
        match bit {
            7 => vec![controls_ex[2], controls_ex[3]],
            8 => vec![ui::keys::DK_ESCAPE],
            b => vec![controls_ex[b as usize]],
        }
    };
    let mut out = Vec::new();
    for bit in 0..=8u32 {
        let (was, is) = ((prev >> bit) & 1 != 0, (now >> bit) & 1 != 0);
        if was != is {
            for dos in keys(bit) {
                out.push(ui::shell::KeyEvent {
                    dos,
                    down: is,
                    repeat: false,
                    typed: ui::keys::TypedKey::Sym(0),
                });
            }
        }
    }
    out
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

    #[test]
    fn touch_edges_are_key_events_on_player_ones_dos_keys() {
        // design §7.3: a rising edge is a key-down of controls_ex[bit], a falling edge a key-up;
        // DIG presses Left and Right; MENU is Esc; no repeats.
        use ui::keys::DK_ESCAPE;
        let ex = scenario::settings::Settings::default().worm_settings[0].controls_ex;
        let down = |e: &ui::shell::KeyEvent| (e.dos, e.down);
        let evs = touch_key_events(0, TOUCH_UP | TOUCH_FIRE, &ex);
        assert_eq!(
            evs.iter().map(down).collect::<Vec<_>>(),
            [(ex[0], true), (ex[4], true)]
        );
        assert!(evs.iter().all(|e| !e.repeat));
        assert!(
            touch_key_events(TOUCH_UP, TOUCH_UP, &ex).is_empty(),
            "held: no events"
        );
        assert_eq!(
            touch_key_events(TOUCH_UP, 0, &ex)
                .iter()
                .map(down)
                .collect::<Vec<_>>(),
            [(ex[0], false)]
        );
        assert_eq!(
            touch_key_events(0, TOUCH_DIG, &ex)
                .iter()
                .map(down)
                .collect::<Vec<_>>(),
            [(ex[2], true), (ex[3], true)]
        );
        assert_eq!(
            touch_key_events(0, TOUCH_MENU, &ex)
                .iter()
                .map(down)
                .collect::<Vec<_>>(),
            [(DK_ESCAPE, true)]
        );
        assert_eq!(
            touch_state(TOUCH_MENU),
            ControlState::new(),
            "MENU never reaches the sim"
        );
    }
}
