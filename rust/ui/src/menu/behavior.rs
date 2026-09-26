//! C++ `ItemBehavior` and its subclasses (`itemBehavior.hpp:8-18`, design §3.5 / §4.3). C++
//! builds a fresh behavior per call through the virtual `GetItemBehavior`, holding `int& v` into
//! the settings; Rust's `Behavior<'a>` borrows the one field it edits from the `MenuModel`, is
//! built per call, and is dropped before the `Menu` is touched again.

use super::{Menu, MenuCx};
use crate::text::{ONOFF, time_to_string, time_to_string_frames};

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

/// `IntegerBehavior` (`integerBehavior.hpp:8-32`).
pub struct Integer<'a> {
    pub v: &'a mut i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub percentage: bool,
    pub scroll_interval: i32,
    pub display_div: i32,
    pub allow_entry: bool,
}

impl<'a> Integer<'a> {
    pub fn new(v: &'a mut i32, min: i32, max: i32, step: i32, percentage: bool) -> Integer<'a> {
        Integer {
            v,
            min,
            max,
            step,
            percentage,
            scroll_interval: 5,
            display_div: 1,
            allow_entry: true,
        }
    }
}

/// One item's behavior for one call (see the module doc).
pub enum Behavior<'a> {
    /// The base `ItemBehavior`: `OnLeftRight` true, `OnEnter` -1, `OnUpdate` nothing.
    Plain,
    Integer(Integer<'a>),
    /// `TimeBehavior` (`timeBehavior.hpp:9-17`): `IntegerBehavior` with `allow_entry = false` and
    /// the time string; build it with [`Behavior::time`].
    Time {
        int: Integer<'a>,
        frames: bool,
    },
    /// `BooleanSwitchBehavior` with its default setter (`booleanSwitchBehavior.hpp:12-13`).
    Bool(&'a mut bool),
    /// `EnumBehavior` (`enumBehavior.hpp:9-25`).
    Enum {
        v: &'a mut u32,
        min: u32,
        max: u32,
        broken: bool,
    },
    /// `ArrayEnumBehavior` (`arrayEnumBehavior.hpp:9-23`): an enum over `[0, arr.len() - 1]`.
    ArrayEnum {
        v: &'a mut u32,
        arr: &'static [&'static str],
        broken: bool,
    },
    Custom(Box<dyn CustomBehavior + 'a>),
}

const KEEP: LeftRight = LeftRight {
    keep: true,
    update_all: false,
};

impl<'a> Behavior<'a> {
    /// `TimeBehavior(common, v, min, max, step, frames)`.
    pub fn time(v: &'a mut i32, min: i32, max: i32, step: i32, frames: bool) -> Behavior<'a> {
        let mut int = Integer::new(v, min, max, step, false);
        int.allow_entry = false;
        Behavior::Time { int, frames }
    }

    pub fn on_left_right(
        &mut self,
        menu: &mut Menu,
        idx: usize,
        dir: i32,
        cx: &mut MenuCx,
    ) -> LeftRight {
        let (out, update_self) = match self {
            Behavior::Plain => (KEEP, false),
            // integerBehavior.cpp:14-33 (TimeBehavior inherits it).
            Behavior::Integer(b) | Behavior::Time { int: b, .. } => {
                if cx.menu_cycles % (b.scroll_interval as u32) != 0 {
                    return KEEP;
                }
                let mut new_v = *b.v;
                if (dir < 0 && new_v > b.min) || (dir > 0 && new_v < b.max) {
                    new_v = (new_v + dir * b.step).clamp(b.min, b.max);
                }
                let changed = new_v != *b.v;
                *b.v = new_v;
                (KEEP, changed)
            }
            // booleanSwitchBehavior.cpp:8-18.
            Behavior::Bool(v) => {
                let hook = if dir > 0 {
                    cx.hooks.move_up
                } else {
                    cx.hooks.move_down
                };
                cx.play(hook);
                **v = !**v;
                (
                    LeftRight {
                        keep: false,
                        update_all: false,
                    },
                    true,
                )
            }
            // enumBehavior.cpp:9-22.
            Behavior::Enum {
                v,
                min,
                max,
                broken,
            } => {
                if *broken {
                    return LeftRight {
                        keep: false,
                        update_all: false,
                    };
                }
                let hook = if dir > 0 {
                    cx.hooks.move_up
                } else {
                    cx.hooks.move_down
                };
                cx.play(hook);
                (
                    LeftRight {
                        keep: false,
                        update_all: change(v, *min, *max, dir),
                    },
                    false,
                )
            }
            Behavior::ArrayEnum { v, arr, broken } => {
                if *broken {
                    return LeftRight {
                        keep: false,
                        update_all: false,
                    };
                }
                let hook = if dir > 0 {
                    cx.hooks.move_up
                } else {
                    cx.hooks.move_down
                };
                cx.play(hook);
                (
                    LeftRight {
                        keep: false,
                        update_all: change(v, 0, arr.len() as u32 - 1, dir),
                    },
                    false,
                )
            }
            Behavior::Custom(c) => return c.on_left_right(menu, idx, dir, cx),
        };
        if update_self {
            self.on_update(menu, idx);
        }
        out
    }

    pub fn on_enter(&mut self, menu: &mut Menu, idx: usize, cx: &mut MenuCx) -> (Enter, bool) {
        match self {
            Behavior::Plain => (Enter::Result(-1), false),
            // integerBehavior.cpp:36-80.
            Behavior::Integer(b) | Behavior::Time { int: b, .. } => {
                cx.play(cx.hooks.select);
                if !b.allow_entry {
                    return (Enter::Result(-1), false);
                }
                let Some((x, y)) = menu.item_position(idx) else {
                    return (Enter::Result(-1), false);
                };
                let x = x + menu.value_offset_x;
                let (min, max) = (b.min / b.display_div, b.max / b.display_div);
                debug_assert!(max > 0, "C++ floor(log10(<= 0)) is undefined");
                let entry = ValueEntry {
                    item_id: menu.items[idx].id,
                    initial: (*b.v / b.display_div).to_string(),
                    digits: decimal_digits(max),
                    x: x + 2,
                    y,
                    min,
                    max,
                    div: b.display_div,
                    percentage: b.percentage,
                };
                (Enter::EditValue(entry), false)
            }
            // booleanSwitchBehavior.cpp:20-25.
            Behavior::Bool(v) => {
                cx.play(cx.hooks.select);
                **v = !**v;
                self.on_update(menu, idx);
                (Enter::Result(-1), false)
            }
            // enumBehavior.cpp:24-29 (`broken` does not apply to Enter).
            Behavior::Enum { v, min, max, .. } => {
                cx.play(cx.hooks.select);
                (Enter::Result(-1), change(v, *min, *max, 1))
            }
            Behavior::ArrayEnum { v, arr, .. } => {
                cx.play(cx.hooks.select);
                (Enter::Result(-1), change(v, 0, arr.len() as u32 - 1, 1))
            }
            Behavior::Custom(c) => c.on_enter(menu, idx, cx),
        }
    }

    pub fn on_update(&self, menu: &mut Menu, idx: usize) {
        let value = match self {
            Behavior::Plain => return,
            // integerBehavior.cpp:82-88.
            Behavior::Integer(b) => {
                let mut s = (*b.v / b.display_div).to_string();
                if b.percentage {
                    s.push('%');
                }
                s
            }
            // timeBehavior.cpp:8-11.
            Behavior::Time { int, frames } => {
                if *frames {
                    time_to_string_frames(*int.v)
                } else {
                    time_to_string(*int.v)
                }
            }
            Behavior::Bool(v) => ONOFF[**v as usize].to_string(),
            Behavior::Enum { v, .. } => v.to_string(),
            Behavior::ArrayEnum { v, arr, .. } => arr[**v as usize].to_string(),
            Behavior::Custom(c) => return c.on_update(menu, idx),
        };
        let item = &mut menu.items[idx];
        item.value = value;
        item.has_value = true;
    }
}

/// `EnumBehavior::Change` (`enumBehavior.cpp:31-39`): all `uint32_t`, `dir` converted to
/// `uint32_t`. Returns whether `v` changed (C++ then calls `menu.UpdateItems`).
fn change(v: &mut u32, min: u32, max: u32, dir: i32) -> bool {
    let range = max.wrapping_sub(min).wrapping_add(1);
    let new_v = (v
        .wrapping_add(dir as u32)
        .wrapping_add(range)
        .wrapping_sub(min)
        % range)
        .wrapping_add(min);
    let changed = new_v != *v;
    *v = new_v;
    changed
}

/// `1 + floor(log10(n))` for `n >= 1` in integer arithmetic (`integerBehavior.cpp:47`): equal for
/// every positive `i32` (the powers-of-ten sweep in the tests).
pub fn decimal_digits(n: i32) -> i32 {
    let (mut n, mut d) = (n, 1);
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}
