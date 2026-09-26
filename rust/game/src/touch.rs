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
//!
//! Step 4½e-1 (design §7.4, plan D9 and T10 step 5): [`TouchKeys`] adds the menu auto-repeat of a
//! held pad Up/Down and FIRE-as-Return while a text box is up, and [`page_text_events`] turns the
//! phone text field's entries (`window.lieroText`) into input events.

use sim::state::ControlState;
use ui::keys::{DK_BACKSPACE, DK_RETURN, TypedKey};
use ui::shell::{InputEvent, KeyEvent, Phase};

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

/// Ticks a pad Up/Down is held on a menu before its first repeat, then the repeat period: the
/// `LocalController` cadence (design §7.4).
pub const MENU_REPEAT_DELAY: u32 = 12;
pub const MENU_REPEAT_PERIOD: u32 = 3;

/// The live touch → shell key events, one [`TouchKeys::tick`] per fixed tick (Step 4½e-1):
/// [`touch_key_events`], plus two Rust-only, presentation-only additions (design §7.4):
///
/// - while the phase is `menu`, a held pad Up or Down repeats: `repeat` key-downs of
///   `controls_ex[0]` / `[1]` on the 12th tick after the press, then every 3rd — the kind of
///   event the OS keyboard repeat already sends;
/// - while the phase is `text` (an `InputStringState` is on top), FIRE's press is Return (DOS 28)
///   instead of `controls_ex[4]`, so FIRE confirms; MENU stays Esc, which cancels (Q5). A
///   release sends the key its press sent, whatever the phase has become.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TouchKeys {
    prev: u32,
    fire_dos: Option<u32>,
    /// Ticks since pad Up / Down was pressed (0 on the press tick).
    held: [u32; 2],
}

impl TouchKeys {
    /// This tick's key events for the mask `now`; `phase` is the screen that updates this tick.
    pub fn tick(&mut self, now: u32, phase: Phase, controls_ex: &[u32; 8]) -> Vec<KeyEvent> {
        let prev = self.prev;
        self.prev = now;
        let mut out = touch_key_events(prev & !TOUCH_FIRE, now & !TOUCH_FIRE, controls_ex);
        let key = |dos, down, repeat| KeyEvent {
            dos,
            down,
            repeat,
            typed: TypedKey::Sym(0),
        };
        match (prev & TOUCH_FIRE != 0, now & TOUCH_FIRE != 0) {
            (false, true) => {
                let dos = if phase == Phase::Text {
                    DK_RETURN
                } else {
                    controls_ex[4]
                };
                self.fire_dos = Some(dos);
                out.push(key(dos, true, false));
            }
            (true, false) => {
                let dos = self.fire_dos.take().unwrap_or(controls_ex[4]);
                out.push(key(dos, false, false));
            }
            _ => {}
        }
        for (i, bit) in [TOUCH_UP, TOUCH_DOWN].into_iter().enumerate() {
            if now & bit == 0 {
                self.held[i] = 0;
                continue;
            }
            self.held[i] = if prev & bit != 0 { self.held[i] + 1 } else { 0 };
            let t = self.held[i];
            if phase == Phase::Menu
                && t >= MENU_REPEAT_DELAY
                && (t - MENU_REPEAT_DELAY).is_multiple_of(MENU_REPEAT_PERIOD)
            {
                out.push(key(controls_ex[i], true, true));
            }
        }
        out
    }
}

/// The longest WEAPON press (in ticks, 0.3 s) that still counts as a tap for [`WeaponTap`].
pub const WEAPON_TAP_MAX_TICKS: u32 = 21;

/// A Rust-only touch convenience (John: weapon change is "only one touch" on a phone): during
/// play, a quick tap on WEAPON on its own steps to the next weapon.
///
/// C++ changes weapon only with Change held and Left/Right pressed (`worm.cpp:1065-1096`), and
/// a lone Change tap just shows the weapon's name, which is what a phone player saw. So a tap is
/// turned into exactly the key sequence a keyboard player would press. The press ticks are Change
/// alone, which latches `key_change_pressed`. The tick after the release is Change + Right, one
/// step. The next tick is Change alone, and then nothing. The sim sees ordinary
/// [`ControlState`] words (recorded and replayed like any other input), so nothing
/// downstream changes. A press held longer, or one that also touched the pad or another
/// button, is left alone: hold WEAPON + pad Left/Right and WEAPON + JUMP (the rope) work as in
/// the original. Outside `game` the mask passes through untouched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponTap {
    /// Ticks WEAPON has been held (0 on the press tick); `None` while it is up.
    held: Option<u32>,
    /// Another control was down at some point during this press.
    other: bool,
    /// Ticks of the synthesized step still to send (2: Change + Right, 1: Change).
    pulse: u8,
}

impl WeaponTap {
    /// This tick's touch mask for sampling, from the page's raw mask `now`.
    pub fn apply(&mut self, now: u32, phase: Phase) -> u32 {
        if phase != Phase::Game {
            *self = WeaponTap::default();
            return now;
        }
        let others = now & !(TOUCH_CHANGE | TOUCH_MENU) != 0;
        match (self.held, now & TOUCH_CHANGE != 0) {
            (None, true) => {
                self.held = Some(0);
                self.other = others;
                self.pulse = 0;
            }
            (Some(t), true) => {
                self.held = Some(t + 1);
                self.other |= others;
            }
            (Some(t), false) => {
                if t <= WEAPON_TAP_MAX_TICKS && !self.other && !others {
                    self.pulse = 2;
                }
                self.held = None;
            }
            (None, false) => {}
        }
        match self.pulse {
            2 => {
                self.pulse = 1;
                now | TOUCH_CHANGE | TOUCH_RIGHT
            }
            1 => {
                self.pulse = 0;
                now | TOUCH_CHANGE
            }
            _ => now,
        }
    }
}

/// One entry of the phone text field's queue (`window.lieroText`, plan D9): typed text, or a
/// named key (`{k: "Backspace"}` / `{k: "Enter"}`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageEntry {
    Text(String),
    Key(String),
}

/// The input events of the text field's entries, in order: a text entry is one
/// [`InputEvent::Text`] (the page sends one char per entry); Backspace and Enter are a key
/// down/up pair of DOS 14 / 28; anything else is ignored.
pub fn page_text_events(entries: &[PageEntry]) -> Vec<InputEvent> {
    let key = |dos, down| {
        InputEvent::Key(KeyEvent {
            dos,
            down,
            repeat: false,
            typed: TypedKey::Sym(0),
        })
    };
    let mut out = Vec::new();
    for e in entries {
        match e {
            PageEntry::Text(t) if !t.is_empty() => out.push(InputEvent::Text(t.clone())),
            PageEntry::Key(k) => {
                let dos = match k.as_str() {
                    "Backspace" => DK_BACKSPACE,
                    "Enter" => DK_RETURN,
                    _ => continue,
                };
                out.extend([key(dos, true), key(dos, false)]);
            }
            PageEntry::Text(_) => {}
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

    fn ex() -> [u32; 8] {
        scenario::settings::Settings::default().worm_settings[0].controls_ex
    }

    /// The ticks (0 = the press) on which a pad button held from tick 0 emits a repeat.
    fn repeat_ticks(bit: u32, phase: Phase, ticks: u32) -> Vec<u32> {
        let mut t = TouchKeys::default();
        (0..ticks)
            .filter(|_| t.tick(bit, phase, &ex()).iter().any(|e| e.repeat))
            .collect()
    }

    #[test]
    fn a_held_pad_up_or_down_repeats_on_menus_at_12_then_every_3() {
        for (bit, dos) in [(TOUCH_UP, ex()[0]), (TOUCH_DOWN, ex()[1])] {
            assert_eq!(
                repeat_ticks(bit, Phase::Menu, 22),
                [12, 15, 18, 21],
                "bit {bit}"
            );
            let mut t = TouchKeys::default();
            let evs: Vec<Vec<KeyEvent>> =
                (0..13).map(|_| t.tick(bit, Phase::Menu, &ex())).collect();
            assert_eq!(
                evs[0],
                [KeyEvent {
                    dos,
                    down: true,
                    repeat: false,
                    typed: TypedKey::Sym(0)
                }]
            );
            assert!(evs[1..12].iter().all(Vec::is_empty));
            assert_eq!(
                evs[12],
                [KeyEvent {
                    dos,
                    down: true,
                    repeat: true,
                    typed: TypedKey::Sym(0)
                }]
            );
        }
        // A release restarts the count.
        let mut t = TouchKeys::default();
        for _ in 0..14 {
            t.tick(TOUCH_DOWN, Phase::Menu, &ex());
        }
        t.tick(0, Phase::Menu, &ex());
        let again: Vec<bool> = (0..13)
            .map(|_| {
                t.tick(TOUCH_DOWN, Phase::Menu, &ex())
                    .iter()
                    .any(|e| e.repeat)
            })
            .collect();
        assert_eq!(again.iter().position(|&r| r), Some(12));
    }

    #[test]
    fn no_repeat_outside_menus_nor_for_other_buttons() {
        for phase in [Phase::Text, Phase::Weapsel, Phase::Game, Phase::Quit] {
            assert!(repeat_ticks(TOUCH_UP, phase, 40).is_empty(), "{phase:?}");
        }
        for bit in [TOUCH_LEFT, TOUCH_RIGHT, TOUCH_FIRE, TOUCH_JUMP, TOUCH_MENU] {
            assert!(repeat_ticks(bit, Phase::Menu, 40).is_empty(), "bit {bit}");
        }
    }

    #[test]
    fn fire_is_return_only_while_a_text_box_is_up() {
        use ui::keys::DK_ESCAPE;
        let down_up = |phase_down: Phase, phase_up: Phase| {
            let mut t = TouchKeys::default();
            let d = t.tick(TOUCH_FIRE, phase_down, &ex());
            let u = t.tick(0, phase_up, &ex());
            [d, u].map(|evs| evs.iter().map(|e| (e.dos, e.down)).collect::<Vec<_>>())
        };
        assert_eq!(
            down_up(Phase::Text, Phase::Text),
            [vec![(DK_RETURN, true)], vec![(DK_RETURN, false)]]
        );
        for phase in [Phase::Menu, Phase::Weapsel, Phase::Game] {
            assert_eq!(
                down_up(phase, phase),
                [vec![(ex()[4], true)], vec![(ex()[4], false)]],
                "{phase:?}"
            );
        }
        assert_eq!(
            down_up(Phase::Text, Phase::Menu),
            [vec![(DK_RETURN, true)], vec![(DK_RETURN, false)]],
            "the release is the press's key"
        );
        assert_eq!(
            down_up(Phase::Menu, Phase::Text),
            [vec![(ex()[4], true)], vec![(ex()[4], false)]]
        );
        let mut t = TouchKeys::default();
        assert_eq!(
            t.tick(TOUCH_MENU, Phase::Text, &ex())
                .iter()
                .map(|e| (e.dos, e.down))
                .collect::<Vec<_>>(),
            [(DK_ESCAPE, true)],
            "MENU stays Esc"
        );
    }

    #[test]
    fn the_text_field_entries_are_text_and_key_pairs() {
        let key = |dos, down| {
            InputEvent::Key(KeyEvent {
                dos,
                down,
                repeat: false,
                typed: TypedKey::Sym(0),
            })
        };
        let entries = [
            PageEntry::Text("1".into()),
            PageEntry::Key("Backspace".into()),
            PageEntry::Text(String::new()),
            PageEntry::Key("Tab".into()),
            PageEntry::Text("7".into()),
            PageEntry::Key("Enter".into()),
        ];
        assert_eq!(
            page_text_events(&entries),
            [
                InputEvent::Text("1".into()),
                key(DK_BACKSPACE, true),
                key(DK_BACKSPACE, false),
                InputEvent::Text("7".into()),
                key(DK_RETURN, true),
                key(DK_RETURN, false),
            ]
        );
    }

    /// Drives the real sim weapon-change code the way the live shell does: the tap's mask →
    /// [`touch_state`] → C++ `OnKey`'s edges (`ui::keys::apply_key_edges`) → the change/movement
    /// gate (`worm.cpp:348-353`). Returns the worm's weapon after each tick.
    fn run_taps(masks: &[u32]) -> Vec<i32> {
        use sim::control::process_weapon_change;
        use sim::state::{NUM_WEAPONS, WeaponInit, WormInit, WormState};
        use sim_core::vec::Vec2;
        let mut w = WormState::from_init(&WormInit {
            index: 0,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit {
                ty: Some(0),
                ammo: 10,
            }; NUM_WEAPONS],
            start_pos: Vec2::zero(),
            visible: true,
        });
        let (mut tap, mut prev) = (WeaponTap::default(), ControlState::new());
        masks
            .iter()
            .map(|&m| {
                let now = touch_state(tap.apply(m, Phase::Game));
                w.control_states = ui::keys::apply_key_edges(prev, now, w.control_states);
                prev = now;
                if w.control_states.get(ControlState::CHANGE) {
                    process_weapon_change(&mut w, true);
                } else {
                    w.key_change_pressed = false;
                }
                w.current_weapon
            })
            .collect()
    }

    #[test]
    fn a_lone_weapon_tap_steps_exactly_one_weapon() {
        // John: "only one touch". A 5-tick tap, then nothing: one step, on the tick after the
        // release, and no more.
        let mut masks = vec![TOUCH_CHANGE; 5];
        masks.extend([0; 10]);
        let w = run_taps(&masks);
        assert_eq!(w[..5], [0; 5], "holding WEAPON alone never steps");
        assert_eq!(w[5..], [1; 10], "the release steps once");
        // Three taps: three weapons.
        let tap = [TOUCH_CHANGE, TOUCH_CHANGE, TOUCH_CHANGE, 0, 0, 0, 0];
        let three: Vec<u32> = tap.iter().chain(&tap).chain(&tap).copied().collect();
        assert_eq!(*run_taps(&three).last().unwrap(), 3);
    }

    #[test]
    fn a_long_press_or_a_chord_is_the_original_hold() {
        // Held past the tap limit: only the name shows, as in C++.
        let mut long = vec![TOUCH_CHANGE; WEAPON_TAP_MAX_TICKS as usize + 2];
        long.extend([0; 5]);
        assert_eq!(*run_taps(&long).last().unwrap(), 0);
        // WEAPON + one pad Right: the original's one step, and no extra step on release.
        let chord = [
            TOUCH_CHANGE,
            TOUCH_CHANGE | TOUCH_RIGHT,
            TOUCH_CHANGE | TOUCH_RIGHT,
            TOUCH_CHANGE,
            0,
            0,
            0,
        ];
        assert_eq!(*run_taps(&chord).last().unwrap(), 1);
        // WEAPON + JUMP (the rope): no step either.
        let rope = [
            TOUCH_CHANGE,
            TOUCH_CHANGE | TOUCH_JUMP,
            TOUCH_CHANGE,
            0,
            0,
            0,
        ];
        assert_eq!(*run_taps(&rope).last().unwrap(), 0);
    }

    #[test]
    fn the_tap_is_only_for_play() {
        for phase in [Phase::Menu, Phase::Text, Phase::Weapsel, Phase::Quit] {
            let mut tap = WeaponTap::default();
            for m in [TOUCH_CHANGE, TOUCH_CHANGE, 0, 0] {
                assert_eq!(tap.apply(m, phase), m, "{phase:?} passes the mask through");
            }
        }
    }
}
