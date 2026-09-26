//! C++ `StateStack` (`state.hpp:47-139`; design §4.9). A `Screen` enum rather than trait objects:
//! every dispatch is an exhaustive `match`, so a screen 4½e/4½f/4½g adds cannot be forgotten.
//! The stack is plain data; `Shell` runs `enter` before `push` (`Push` calls `Enter`,
//! `state.hpp:50-54`). No screen has a `Leave`. Step 4½e-1 adds the settings menu's sub-screens.

use super::main_menu::MainMenuState;
use super::overlay::{InfoBoxState, InputStringState};
use super::weapon_options::WeaponMenuState;

pub enum Screen {
    MainMenu(MainMenuState),
    /// `GamePlayState`; its controller is `Shell::current`, as C++'s is `gfx.controller`.
    Playing,
    /// `WeaponMenuState` (4½e-1), pushed by WEAPON OPTIONS over the main menu.
    WeaponOptions(WeaponMenuState),
    /// `InputStringState` (4½e-1): number entry, over the main menu.
    InputString(InputStringState),
    /// `InfoBoxState` (4½e-1): WEAPON OPTIONS' "no weapons" box, the refusal boxes.
    InfoBox(InfoBoxState),
}

impl Screen {
    /// `AppState::IsOverlay` (`state.hpp:34`): only `InputStringState` is one
    /// (`inputState.hpp:21-23`).
    pub fn is_overlay(&self) -> bool {
        matches!(self, Screen::InputString(_))
    }

    /// `AppState::WantsMenuFlip` (`state.hpp:38`, `gamePlayState.hpp:119`): every screen but
    /// `GamePlayState`.
    pub fn wants_menu_flip(&self) -> bool {
        !matches!(self, Screen::Playing)
    }
}

/// What `StateStack::Update` did after the top's `Update` (`state.hpp:92-112`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterUpdate {
    Running,
    /// A scheduled replacement was pushed; the caller runs its `enter`.
    Replaced,
    /// The top returned false and was popped; `empty` = `Update()` returned false.
    Popped {
        empty: bool,
    },
}

#[derive(Default)]
pub struct ScreenStack {
    stack: Vec<Screen>,
    pending_replace: Option<Screen>,
}

impl ScreenStack {
    pub fn push(&mut self, s: Screen) {
        self.stack.push(s);
    }

    pub fn pop(&mut self) -> Option<Screen> {
        self.stack.pop()
    }

    /// `ScheduleReplaceTop` (`state.hpp:75`): applied after the top's update.
    pub fn schedule_replace_top(&mut self, s: Screen) {
        self.pending_replace = Some(s);
    }

    /// `StateStack::Update` after the top ran (`state.hpp:92-112`).
    pub fn finish_update(&mut self, keep_running: bool) -> AfterUpdate {
        if let Some(s) = self.pending_replace.take() {
            self.pop();
            self.push(s);
            return AfterUpdate::Replaced;
        }
        if !keep_running {
            self.pop();
            return AfterUpdate::Popped {
                empty: self.stack.is_empty(),
            };
        }
        AfterUpdate::Running
    }

    pub fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn screens(&self) -> &[Screen] {
        &self.stack
    }

    /// `StateStack::Draw`'s bottom (`state.hpp:117-125`): walk down past overlays.
    pub fn draw_from_by(&self, is_overlay: impl Fn(&Screen) -> bool) -> usize {
        let mut bottom = self.stack.len().saturating_sub(1);
        while bottom > 0 && is_overlay(&self.stack[bottom]) {
            bottom -= 1;
        }
        bottom
    }

    pub fn draw_from(&self) -> usize {
        self.draw_from_by(Screen::is_overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> Screen {
        Screen::MainMenu(MainMenuState::new())
    }

    #[test]
    fn update_pops_on_false_and_reports_an_empty_stack() {
        let mut s = ScreenStack::default();
        s.push(menu());
        assert_eq!(s.finish_update(true), AfterUpdate::Running);
        assert_eq!(s.finish_update(false), AfterUpdate::Popped { empty: true });
        s.push(Screen::Playing);
        s.push(menu());
        assert_eq!(s.finish_update(false), AfterUpdate::Popped { empty: false });
        assert!(matches!(s.top(), Some(Screen::Playing)));
    }

    #[test]
    fn a_scheduled_replacement_wins_over_the_pop() {
        let mut s = ScreenStack::default();
        s.push(Screen::Playing);
        s.schedule_replace_top(menu());
        assert_eq!(
            s.finish_update(false),
            AfterUpdate::Replaced,
            "state.hpp:101-105 runs first"
        );
        assert_eq!(s.len(), 1);
        assert!(matches!(s.top(), Some(Screen::MainMenu(_))));
    }

    #[test]
    fn draw_walks_down_past_overlays() {
        let mut s = ScreenStack::default();
        s.push(menu());
        s.push(Screen::Playing);
        s.push(Screen::Playing);
        let playing_is_overlay = |x: &Screen| matches!(x, Screen::Playing);
        assert_eq!(s.draw_from_by(playing_is_overlay), 0);
        assert_eq!(s.draw_from(), 2, "no 4½d screen is an overlay");
        assert!(!Screen::Playing.wants_menu_flip() && menu().wants_menu_flip());
    }

    fn input() -> Screen {
        use crate::menu::ValueEntry;
        use crate::shell::overlay::InputPurpose;
        Screen::InputString(InputStringState::new(
            b"",
            3,
            0,
            0,
            None,
            "",
            false,
            InputPurpose::IntegerEntry(ValueEntry {
                item_id: 0,
                initial: String::new(),
                digits: 3,
                x: 0,
                y: 0,
                min: 0,
                max: 999,
                div: 1,
                percentage: false,
            }),
        ))
    }

    fn info() -> Screen {
        use crate::shell::overlay::InfoPurpose;
        Screen::InfoBox(InfoBoxState::new("X", 0, 0, false, InfoPurpose::NoWeapons))
    }

    #[test]
    fn only_the_input_string_is_an_overlay_and_only_play_skips_the_menu_flip() {
        let weapons = Screen::WeaponOptions(WeaponMenuState::default());
        assert!(input().is_overlay());
        assert!(!info().is_overlay() && !weapons.is_overlay() && !menu().is_overlay());
        for sc in [menu(), weapons, input(), info()] {
            assert!(sc.wants_menu_flip());
        }
        let mut s = ScreenStack::default();
        s.push(menu());
        s.push(input());
        assert_eq!(s.draw_from(), 0, "the entry draws over the main menu");
        s.pop();
        s.push(Screen::WeaponOptions(WeaponMenuState::default()));
        s.push(info());
        assert_eq!(s.draw_from(), 2, "the info box draws alone");
    }
}
