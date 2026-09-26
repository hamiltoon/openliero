//! The C++ menu keyboard (design §4.6): `Gfx::dos_keys` plus `key_buf` (`gfx.hpp:149-219`,
//! `gfx.cpp:598-641`, `:861-976`). A key-down event — OS auto-repeats included — sets the
//! DOS key's flag; a key-up clears it; `test_once` reads and clears (`TestKeyOnce`), so a held
//! key acts once per key-down event (design finding 5). Menus test the SETTINGS' DOS bindings
//! (`controls_ex`) of all three `WormSettings`, as C++ does. Gamepads and `ex_keys` are not
//! modelled (overview §Deferrals).

use scenario::settings::WormSettings;
use sim::state::ControlState;

/// `kMaxDosKey` (`keys.hpp:17`): DOS scancodes are `1..177`; 0 means "unbound".
pub const MAX_DOS_KEY: u32 = 177;
/// DOS scancodes the menus test (`keys.cpp:9-60`: the `liero_to_sdl_keys` index of each key).
pub const DK_ESCAPE: u32 = 1;
pub const DK_RETURN: u32 = 28;
pub const DK_LCTRL: u32 = 29;
pub const DK_F1: u32 = 59;
pub const DK_F2: u32 = 60;
pub const DK_F3: u32 = 61;
pub const DK_F5: u32 = 63;
pub const DK_F6: u32 = 64;
pub const DK_F7: u32 = 65;
pub const DK_F8: u32 = 66;
pub const DK_F9: u32 = 67;
/// `SDLToDOSKey`'s "unknown key" value (`keys.cpp:70-75`).
pub const DK_UNKNOWN: u32 = 89;
pub const DK_KP_ENTER: u32 = 116;
pub const DK_RCTRL: u32 = 117;
pub const DK_UP: u32 = 160;
pub const DK_PGUP: u32 = 161;
pub const DK_LEFT: u32 = 163;
pub const DK_RIGHT: u32 = 165;
pub const DK_DOWN: u32 = 168;
pub const DK_PGDN: u32 = 169;

/// `WormSettingsExtensions::Control` (`worm.hpp:45-55`): indices into `controls_ex`.
pub const K_UP: usize = 0;
pub const K_DOWN: usize = 1;
pub const K_LEFT: usize = 2;
pub const K_RIGHT: usize = 3;
pub const K_FIRE: usize = 4;
pub const K_CHANGE: usize = 5;
pub const K_JUMP: usize = 6;
pub const K_DIG: usize = 7;
/// `WormSettingsExtensions::kInputKeyboard` (`worm.hpp:59`).
pub const INPUT_KEYBOARD: u32 = 0;

/// One `key_buf` entry (`gfx.cpp:601-603`) as `Menu::OnKeys` reads it (`menu.cpp:16-19`): the
/// key's unshifted symbol `SDL_GetKeyFromScancode(sc, NONE)` — only `32..=127` is acted on — or
/// Tab, which is tested by scancode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedKey {
    Sym(u32),
    Tab,
}

/// `Gfx::key_buf` holds 32 scancodes (`gfx.hpp`, `gfx.cpp:601`).
pub const KEY_BUF_LEN: usize = 32;

/// `Gfx::dos_keys` + `key_buf` (see the module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyLatch {
    down: [bool; MAX_DOS_KEY as usize],
    buf: Vec<TypedKey>,
}

impl Default for KeyLatch {
    fn default() -> Self {
        KeyLatch {
            down: [false; MAX_DOS_KEY as usize],
            buf: Vec::with_capacity(KEY_BUF_LEN),
        }
    }
}

impl KeyLatch {
    /// `key_buf_ptr = key_buf` at the top of every frame (`gfx.cpp:1475`, `:729`).
    pub fn begin_frame(&mut self) {
        self.buf.clear();
    }

    /// `SDL_EVENT_KEY_DOWN` (`gfx.cpp:598-611`): buffer the key (up to 32), set its DOS flag
    /// (`if (kDosScan)`: 0 is never set). OS repeats included (finding 5).
    pub fn key_down(&mut self, dos: u32, typed: TypedKey) {
        if self.buf.len() < KEY_BUF_LEN {
            self.buf.push(typed);
        }
        if dos != 0 {
            self.down[dos as usize] = true;
        }
    }

    /// `SDL_EVENT_KEY_UP` (`gfx.cpp:631-641`).
    pub fn key_up(&mut self, dos: u32) {
        if dos != 0 {
            self.down[dos as usize] = false;
        }
    }

    /// This frame's key-downs, for `Menu::on_keys`.
    pub fn typed(&self) -> &[TypedKey] {
        &self.buf
    }

    /// `TestKeyOnce` (`gfx.hpp:149-153`).
    pub fn test_once(&mut self, dos: u32) -> bool {
        std::mem::take(&mut self.down[dos as usize])
    }

    /// `TestKey` (`gfx.hpp:155`).
    pub fn test(&self, dos: u32) -> bool {
        self.down[dos as usize]
    }

    /// `ReleaseKey` (`gfx.hpp:157`).
    pub fn release(&mut self, dos: u32) {
        self.down[dos as usize] = false;
    }

    /// `ClearKeys` (`gfx.cpp:861-867`): the DOS flags (joysticks and `ex_keys` are unmodelled).
    pub fn clear(&mut self) {
        self.down = [false; MAX_DOS_KEY as usize];
    }

    /// `TestAnyKeyOnce` (`gfx.hpp:183-207`) for a DOS key: 0 never matches; extended keys
    /// (gamepad, `>= kMaxDosKey`) are unmodelled and never match.
    fn test_any_once(&mut self, key: u32) -> bool {
        key != 0 && key < MAX_DOS_KEY && self.test_once(key)
    }

    fn test_any(&self, key: u32) -> bool {
        key != 0 && key < MAX_DOS_KEY && self.test(key)
    }

    /// `Gfx::TestControlOnce` (`gfx.cpp:869-883`): the first keyboard player whose
    /// `controls_ex[control]` flag is set, consumed.
    pub fn test_control_once(&mut self, ws: &[WormSettings], control: usize) -> bool {
        for w in ws {
            if w.input_device == INPUT_KEYBOARD && self.test_any_once(w.controls_ex[control]) {
                return true;
            }
        }
        false
    }

    /// `Gfx::TestControl` (`gfx.cpp:954-968`).
    pub fn test_control(&self, ws: &[WormSettings], control: usize) -> bool {
        ws.iter()
            .any(|w| w.input_device == INPUT_KEYBOARD && self.test_any(w.controls_ex[control]))
    }

    /// `Gfx::ReleaseControl` (`gfx.cpp:970-976`): every player, whatever its input device.
    pub fn release_control(&mut self, ws: &[WormSettings], control: usize) {
        for w in ws {
            let key = w.controls_ex[control];
            if key != 0 && key < MAX_DOS_KEY {
                self.down[key as usize] = false;
            }
        }
    }
}

/// `ResetLeftRight` (`mainMenuState.cpp:56-61`): release the arrows and every player's
/// Left/Right, so a behavior that returned false from `OnLeftRight` fires again only on the
/// next key-down event.
pub fn reset_left_right(keys: &mut KeyLatch, ws: &[WormSettings]) {
    keys.release(DK_LEFT);
    keys.release(DK_RIGHT);
    keys.release_control(ws, K_LEFT);
    keys.release_control(ws, K_RIGHT);
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
    mask: [u32; 2],
}

impl ReleaseLatch {
    /// Latch every bit held now.
    pub fn arm(&mut self, held: &[ControlState; 2]) {
        self.mask = held.map(|c| c.pack());
    }

    /// Mask this tick's sampled words in place.
    pub fn apply(&mut self, inputs: &mut [ControlState; 2]) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use scenario::settings::Settings;

    #[test]
    fn a_key_down_sets_the_flag_until_tested_once_or_released() {
        let mut k = KeyLatch::default();
        k.key_down(DK_UP, TypedKey::Sym(0));
        assert!(k.test(DK_UP) && k.test(DK_UP), "test does not clear");
        assert!(k.test_once(DK_UP));
        assert!(
            !k.test_once(DK_UP),
            "cleared: a held key acts once per key-down event"
        );
        k.key_down(DK_UP, TypedKey::Sym(0)); // an OS repeat
        assert!(k.test_once(DK_UP), "finding 5: repeats count");
        k.key_down(DK_DOWN, TypedKey::Sym(0));
        k.key_up(DK_DOWN);
        assert!(!k.test_once(DK_DOWN), "a key-up clears the flag");
        k.key_down(DK_LEFT, TypedKey::Sym(0));
        k.release(DK_LEFT);
        assert!(!k.test(DK_LEFT));
        k.key_down(DK_F1, TypedKey::Sym(0));
        k.clear();
        assert!(!k.test(DK_F1), "ClearKeys");
    }

    #[test]
    fn key_zero_is_never_down() {
        let mut k = KeyLatch::default();
        k.key_down(0, TypedKey::Sym(0));
        assert!(!k.test(0));
    }

    #[test]
    fn the_key_buf_keeps_32_key_downs_per_frame() {
        let mut k = KeyLatch::default();
        for i in 0..40 {
            k.key_down(DK_UNKNOWN, TypedKey::Sym(b'a' as u32 + i));
        }
        assert_eq!(k.typed().len(), KEY_BUF_LEN);
        assert_eq!(k.typed()[0], TypedKey::Sym(b'a' as u32));
        k.key_up(DK_UNKNOWN);
        assert_eq!(k.typed().len(), KEY_BUF_LEN, "key-ups are not buffered");
        k.begin_frame();
        assert!(
            k.typed().is_empty(),
            "key_buf_ptr = key_buf at each frame (gfx.cpp:1475)"
        );
    }

    #[test]
    fn controls_are_tested_over_all_three_keyboard_players() {
        // settings.cpp:23-60: P1 R/F/D/G/LCtrl/LShift/LAlt, P2 arrows/RCtrl/RAlt/RShift, the
        // network player = P1. gfx.cpp:869-883 / :954-968.
        let s = Settings::default();
        let ws = &s.worm_settings;
        let mut k = KeyLatch::default();
        k.key_down(19, TypedKey::Sym(b'r' as u32)); // R: P1 up (and the network player's)
        assert!(k.test_control(ws, K_UP));
        assert!(k.test_control_once(ws, K_UP));
        assert!(
            !k.test_control_once(ws, K_UP),
            "the first matching player consumed it"
        );
        k.key_down(DK_UP, TypedKey::Sym(0)); // the arrow is P2's up
        assert!(k.test_control_once(ws, K_UP));
        k.key_down(56, TypedKey::Sym(0)); // LAlt: P1 jump
        assert!(k.test_control_once(ws, K_JUMP));
        assert!(
            !k.test_control(ws, K_DIG),
            "DIG is unbound (0) and 0 never matches"
        );
    }

    #[test]
    fn a_gamepad_player_is_skipped_by_test_but_not_by_release() {
        let mut s = Settings::default();
        s.worm_settings[0].input_device = 1;
        s.worm_settings[2].input_device = 1;
        let ws = &s.worm_settings;
        let mut k = KeyLatch::default();
        k.key_down(19, TypedKey::Sym(0));
        assert!(
            !k.test_control(ws, K_UP),
            "only keyboard players are tested (gfx.cpp:873)"
        );
        k.release_control(ws, K_UP);
        s.worm_settings[0].input_device = 0;
        assert!(
            !k.test_control(&s.worm_settings, K_UP),
            "ReleaseControl releases every player's key"
        );
    }

    #[test]
    fn reset_left_right_releases_the_arrows_and_every_players_left_right() {
        let s = Settings::default();
        let mut k = KeyLatch::default();
        for dos in [DK_LEFT, DK_RIGHT, 32, 34] {
            k.key_down(dos, TypedKey::Sym(0));
        }
        reset_left_right(&mut k, &s.worm_settings);
        for dos in [DK_LEFT, DK_RIGHT, 32, 34] {
            assert!(!k.test(dos), "mainMenuState.cpp:56-61 released {dos}");
        }
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
