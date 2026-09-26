//! C++ `ItemBehavior` and its subclasses (`itemBehavior.hpp:8-18`, design §3.5 / §4.3). C++
//! builds a fresh behavior per call through the virtual `GetItemBehavior`, holding `int& v` into
//! the settings; Rust's `Behavior<'a>` borrows the one field it edits from the `MenuModel`, is
//! built per call, and is dropped before the `Menu` is touched again.

use super::{Menu, MenuCx};

/// `OnEnter`'s `int` (`-1`: nothing chosen), or the `InputStringState` an `IntegerBehavior`
/// pushes (`integerBehavior.cpp:36-80`) as a request 4½e's overlay consumes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enter {
    Result(i32),
    EditValue(ValueEntry),
}

/// The value-entry request (`integerBehavior.cpp:41-78`): displayed units, `digits` characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueEntry {
    pub item_id: i32,
    pub initial: String,
    pub digits: i32,
    pub x: i32,
    pub y: i32,
    pub min: i32,
    pub max: i32,
    pub div: i32,
    pub percentage: bool,
}

/// `OnLeftRight`'s bool (`keep`: false makes the caller release Left/Right, design §3.5) plus
/// whether the behavior changed a value that needs `menu.UpdateItems` (`EnumBehavior::Change`,
/// `enumBehavior.cpp:31-39`) — run by `Menu::on_left_right` after the behavior is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeftRight {
    pub keep: bool,
    pub update_all: bool,
}

/// A `gfx.cpp`-local behavior (`LevelSelectBehavior`, `OptionsSaveBehavior`, 4½f's
/// `WeaponEnumBehavior`, …). The defaults are the base `ItemBehavior` (`itemBehavior.hpp:13-17`).
pub trait CustomBehavior {
    fn on_left_right(
        &mut self,
        _menu: &mut Menu,
        _idx: usize,
        _dir: i32,
        _cx: &mut MenuCx,
    ) -> LeftRight {
        LeftRight {
            keep: true,
            update_all: false,
        }
    }
    /// The `Enter` plus the "update all items" request.
    fn on_enter(&mut self, _menu: &mut Menu, _idx: usize, _cx: &mut MenuCx) -> (Enter, bool) {
        (Enter::Result(-1), false)
    }
    fn on_update(&self, _menu: &mut Menu, _idx: usize) {}
}

/// One item's behavior for one call (see the module doc). T3 adds `Integer`, `Time`, `Bool`,
/// `Enum` and `ArrayEnum`.
pub enum Behavior<'a> {
    /// The base `ItemBehavior`: `OnLeftRight` true, `OnEnter` -1, `OnUpdate` nothing.
    Plain,
    Custom(Box<dyn CustomBehavior + 'a>),
}

impl Behavior<'_> {
    pub fn on_left_right(
        &mut self,
        menu: &mut Menu,
        idx: usize,
        dir: i32,
        cx: &mut MenuCx,
    ) -> LeftRight {
        match self {
            Behavior::Plain => LeftRight {
                keep: true,
                update_all: false,
            },
            Behavior::Custom(c) => c.on_left_right(menu, idx, dir, cx),
        }
    }

    pub fn on_enter(&mut self, menu: &mut Menu, idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        match self {
            Behavior::Plain => (Enter::Result(-1), false),
            Behavior::Custom(c) => c.on_enter(menu, idx, cx),
        }
    }

    pub fn on_update(&self, menu: &mut Menu, idx: usize) {
        match self {
            Behavior::Plain => {}
            Behavior::Custom(c) => c.on_update(menu, idx),
        }
    }
}
