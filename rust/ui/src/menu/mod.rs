//! C++ `Menu` / `MenuItem` (`menu.hpp:26-197`, `menu.cpp`, `menuItem.hpp:10-36`; design §3.4,
//! §4.2, §4.4): one method per C++ method, same names in snake_case, the C++ `int` arithmetic
//! and call graph. `MenuModel` stands for C++ `Menu`'s virtuals (`GetItemBehavior`, `OnUpdate`,
//! `DrawItemOverlay`, `menu.hpp:55-67`). Gated line for line against the real C++ `Menu` by
//! `oracle-tests/tests/menu_widget_golden.rs` (G1, design §6.1).

pub mod behavior;
pub mod search;

pub use behavior::{Behavior, CustomBehavior, Enter, LeftRight, ValueEntry};
pub use search::Search;

use render::bitmap::{Bitmap, Pal32};
use render::font::Font;
use render::menu::{ItemColours, draw_item_value, draw_scrollbar};

/// C++ `MenuItem` (`menuItem.hpp:10-36`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub color: u8,
    pub dis_colour: u8,
    pub string: String,
    pub has_value: bool,
    pub value: String,
    pub visible: bool,
    pub selectable: bool,
    pub id: i32,
}

impl MenuItem {
    /// `MenuItem(color, dis_colour, string, id)` (`menuItem.hpp:11-16`).
    pub fn new(color: u8, dis_colour: u8, string: &str, id: i32) -> MenuItem {
        MenuItem {
            color,
            dis_colour,
            string: string.to_string(),
            has_value: false,
            value: String::new(),
            visible: true,
            selectable: true,
            id,
        }
    }

    /// `MenuItem::Space()` (`menuItem.hpp:18-22`): colours 0, empty, id -1, not selectable.
    pub fn space() -> MenuItem {
        let mut m = MenuItem::new(0, 0, "", -1);
        m.selectable = false;
        m
    }
}

/// The TC's menu sound hooks as sample ids (`sound_hook[SoundMenuMoveUp/Down/Select]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuHooks {
    pub move_up: i32,
    pub move_down: i32,
    pub select: i32,
}

/// What a behavior call reads or writes besides the model and the menu: `gfx.menu_cycles`
/// (`IntegerBehavior`'s scroll gate, `integerBehavior.cpp:15`) and the sound log.
pub struct MenuCx<'a> {
    pub menu_cycles: u32,
    pub hooks: MenuHooks,
    pub sounds: &'a mut Vec<i32>,
}

impl MenuCx<'_> {
    /// `g_sound_player->Play(hook)`: `SoundPlayer::Play` drops a negative id (`mixer/player.hpp:15-22`).
    pub fn play(&mut self, hook: i32) {
        if hook >= 0 {
            self.sounds.push(hook);
        }
    }
}

/// C++ `Menu`'s virtuals (`menu.hpp:55-67`).
pub trait MenuModel {
    /// `GetItemBehavior` (base: a plain `ItemBehavior`).
    fn behavior(&mut self, item_id: i32) -> Behavior<'_>;
    /// `Menu::OnUpdate`, run after every item's `OnUpdate` in `UpdateItems`.
    fn on_update(&mut self, _menu: &mut Menu) {}
    /// `DrawItemOverlay` (the player menu's colour bars, 4½f).
    #[allow(clippy::too_many_arguments)]
    fn draw_item_overlay(
        &self,
        _item: &MenuItem,
        _x: i32,
        _y: i32,
        _selected: bool,
        _disabled: bool,
        _bmp: &mut Bitmap,
        _pal: &Pal32,
    ) {
    }
}

/// A menu whose items all have the base behavior (C++ `MainMenu::GetItemBehavior`,
/// `mainMenu.cpp:6-10`).
pub struct PlainModel;

impl MenuModel for PlainModel {
    fn behavior(&mut self, _item_id: i32) -> Behavior<'_> {
        Behavior::Plain
    }
}

/// C++ `Menu` (`menu.hpp:26-197`). `selection` is private as in C++ (`selection_`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub item_height: i32,
    pub value_offset_x: i32,
    pub x: i32,
    pub y: i32,
    pub height: i32,
    /// A visible index.
    pub top_item: i32,
    /// A visible index.
    pub bottom_item: i32,
    pub visible_item_count: i32,
    pub centered: bool,
    pub search: Search,
    selection: i32,
}

impl Menu {
    /// `Menu(x, y, centered)`: `Init(centered)` + `Place(x, y)` (`menu.hpp:28-50`).
    pub fn new(x: i32, y: i32, centered: bool) -> Menu {
        Menu {
            items: Vec::new(),
            item_height: 8,
            value_offset_x: 0,
            x,
            y,
            height: 15,
            top_item: 0,
            bottom_item: 0,
            visible_item_count: 0,
            centered,
            search: Search::default(),
            selection: 0,
        }
    }

    pub fn selection(&self) -> i32 {
        self.selection
    }

    /// `SetSelection` (`menu.hpp:117-121`): raw, no visibility or scroll adjustment.
    pub fn set_selection(&mut self, selection: i32) {
        self.selection = selection;
    }

    pub fn is_selection_valid(&self) -> bool {
        self.selection >= 0 && (self.selection as usize) < self.items.len()
    }

    pub fn selected(&self) -> Option<&MenuItem> {
        self.is_selection_valid()
            .then(|| &self.items[self.selection as usize])
    }

    pub fn selected_id(&self) -> i32 {
        self.selected().map_or(-1, |i| i.id)
    }

    pub fn index_from_id(&self, id: i32) -> i32 {
        self.items
            .iter()
            .position(|i| i.id == id)
            .map_or(-1, |p| p as i32)
    }

    pub fn item_from_id(&self, id: i32) -> Option<&MenuItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn item_from_id_mut(&mut self, id: i32) -> Option<&mut MenuItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// `AddItem(item)` (`menu.cpp:295-302`).
    pub fn add_item(&mut self, item: MenuItem) -> i32 {
        let idx = self.items.len() as i32;
        if item.visible {
            self.visible_item_count += 1;
        }
        self.items.push(item);
        idx
    }

    /// `AddItem(item, pos)` (`menu.cpp:311-317`): returns the size before the insert (quirk).
    pub fn add_item_at(&mut self, item: MenuItem, pos: usize) -> i32 {
        let idx = self.items.len() as i32;
        if item.visible {
            self.visible_item_count += 1;
        }
        self.items.insert(pos, item);
        idx
    }

    /// `Clear` (`menu.cpp:304-309`).
    pub fn clear(&mut self) {
        self.items.clear();
        self.visible_item_count = 0;
        self.set_top(0);
    }

    /// `SetHeight` (`menu.hpp:106-109`).
    pub fn set_height(&mut self, height: i32) {
        self.height = height;
        self.set_top(self.top_item);
    }

    /// `MoveTo` (`menu.cpp:129-134`).
    pub fn move_to(&mut self, new_selection: i32) {
        let s = new_selection.max(0).min(self.items.len() as i32 - 1);
        self.selection = self.first_visible_from(s);
        self.ensure_in_view(self.selection);
    }

    pub fn move_to_id(&mut self, id: i32) {
        self.move_to(self.index_from_id(id));
    }

    /// `MoveToFirstVisible` (`menu.cpp:136`).
    pub fn move_to_first_visible(&mut self) {
        let first = self.first_visible_from(0);
        self.move_to(first);
    }

    /// `IsInView` (`menu.cpp:138-141`).
    pub fn is_in_view(&self, item: i32) -> bool {
        let v = self.visible_item_index(item);
        v >= self.top_item && v < self.bottom_item
    }

    /// `ItemPosition` (`menu.cpp:143-153`) for the item at `index`.
    pub fn item_position(&self, index: usize) -> Option<(i32, i32)> {
        if !self.is_in_view(index as i32) {
            return None;
        }
        let v = self.visible_item_index(index as i32);
        Some((self.x, self.y + (v - self.top_item) * self.item_height))
    }

    /// `EnsureInView` (`menu.cpp:155-167`).
    pub fn ensure_in_view(&mut self, item: i32) {
        if item < 0 || item as usize >= self.items.len() || !self.items[item as usize].visible {
            return;
        }
        let v = self.visible_item_index(item);
        if v < self.top_item {
            self.set_top(v);
        } else if v >= self.bottom_item {
            self.set_bottom(v + 1);
        }
    }

    /// `FirstVisibleFrom` (`menu.cpp:169-177`): the next visible AND selectable item, or the item
    /// count. A negative start wraps (`std::size_t i = item`) and finds nothing.
    pub fn first_visible_from(&self, item: i32) -> i32 {
        if item < 0 {
            return self.items.len() as i32;
        }
        (item as usize..self.items.len())
            .find(|&i| self.items[i].visible && self.items[i].selectable)
            .map_or(self.items.len() as i32, |i| i as i32)
    }

    /// `LastVisibleFrom` (`menu.cpp:179-187`): one PAST the previous visible and selectable item,
    /// or 0. (C++ reads out of bounds for `item > size`; Rust clamps.)
    pub fn last_visible_from(&self, item: i32) -> i32 {
        let end = item.clamp(0, self.items.len() as i32) as usize;
        (0..end)
            .rev()
            .find(|&i| self.items[i].visible && self.items[i].selectable)
            .map_or(0, |i| i as i32 + 1)
    }

    /// `VisibleItemIndex` (`menu.cpp:189-201`): the number of visible items before `item`.
    pub fn visible_item_index(&self, item: i32) -> i32 {
        let mut idx = 0;
        for (i, it) in self.items.iter().enumerate() {
            if !it.visible {
                continue;
            }
            if i as i32 >= item {
                break;
            }
            idx += 1;
        }
        idx
    }

    /// `ItemFromVisibleIndex` (`menu.cpp:203-216`): the `idx`-th visible item, or the item count.
    pub fn item_from_visible_index(&self, idx: i32) -> i32 {
        let mut idx = idx;
        for (i, it) in self.items.iter().enumerate() {
            if !it.visible {
                continue;
            }
            if idx == 0 {
                return i as i32;
            }
            idx -= 1;
        }
        self.items.len() as i32
    }

    /// `SetBottom` (`menu.cpp:218`).
    pub fn set_bottom(&mut self, new_bottom: i32) {
        self.set_top(new_bottom - self.height);
    }

    /// `SetTop` (`menu.cpp:220-225`).
    pub fn set_top(&mut self, new_top: i32) {
        let t = new_top.min(self.visible_item_count - self.height).max(0);
        self.top_item = t;
        self.bottom_item = (t + self.height).min(self.visible_item_count);
    }

    /// `SetVisibility` (`menu.cpp:227-246`): count, re-anchor the top on the same real item,
    /// then `EnsureInView(selection)`.
    pub fn set_visibility(&mut self, id: i32, state: bool) {
        let item = self.index_from_id(id);
        if item < 0 {
            debug_assert!(false, "SetVisibility: no item with id {id}");
            return;
        }
        let it = item as usize;
        if self.items[it].visible && !state {
            self.visible_item_count -= 1;
        } else if !self.items[it].visible && state {
            self.visible_item_count += 1;
        }
        let real_top = self.item_from_visible_index(self.top_item);
        self.items[it].visible = state;
        let v = self.visible_item_index(real_top);
        self.set_top(v);
        self.ensure_in_view(self.selection);
    }

    /// `Scroll` (`menu.cpp:248`).
    pub fn scroll(&mut self, dir: i32) {
        self.set_top(self.top_item + dir);
    }

    /// `MovementPage` (`menu.cpp:250-261`).
    pub fn movement_page(&mut self, direction: i32) {
        let mut sel = self.visible_item_index(self.selection);
        let offset = direction * (self.height / 2);
        sel += offset;
        self.set_top(self.top_item + offset);
        sel = sel.max(0).min(self.visible_item_count - 1);
        let target = self.item_from_visible_index(sel);
        self.move_to(target);
    }

    /// `Movement` (`menu.cpp:263-293`): the next visible and selectable item, wrapping.
    pub fn movement(&mut self, direction: i32) {
        let n = self.items.len() as i32;
        let ok = |m: &Menu, i: i32| m.items[i as usize].visible && m.items[i as usize].selectable;
        if direction < 0 {
            let first = (0..self.selection)
                .rev()
                .chain(((self.selection + 1)..n).rev());
            for i in first {
                if ok(self, i) {
                    self.move_to(i);
                    return;
                }
            }
        } else if direction > 0 {
            let first = ((self.selection + 1)..n).chain(0..self.selection);
            for i in first {
                if ok(self, i) {
                    self.move_to(i);
                    return;
                }
            }
        }
    }

    /// `OnLeftRight` (`menu.hpp:69-76`): false with no selection; else the behavior's bool.
    /// A requested `UpdateItems` runs after the behavior is dropped (design §4.2).
    pub fn on_left_right<M: MenuModel>(&mut self, m: &mut M, dir: i32, cx: &mut MenuCx) -> bool {
        if !self.is_selection_valid() {
            return false;
        }
        let idx = self.selection as usize;
        let out = m
            .behavior(self.items[idx].id)
            .on_left_right(self, idx, dir, cx);
        if out.update_all {
            self.update_items(m);
        }
        out.keep
    }

    /// `OnEnter` (`menu.hpp:78-85`): `Result(0)` (C++ `return false;`) with no selection.
    pub fn on_enter<M: MenuModel>(&mut self, m: &mut M, cx: &mut MenuCx) -> Enter {
        if !self.is_selection_valid() {
            return Enter::Result(0);
        }
        let idx = self.selection as usize;
        let (enter, update_all) = m.behavior(self.items[idx].id).on_enter(self, idx, cx);
        if update_all {
            self.update_items(m);
        }
        enter
    }

    /// `UpdateItems` (`menu.hpp:89-97`): every item's `OnUpdate`, then `Menu::OnUpdate`.
    pub fn update_items<M: MenuModel>(&mut self, m: &mut M) {
        for idx in 0..self.items.len() {
            m.behavior(self.items[idx].id).on_update(self, idx);
        }
        m.on_update(self);
    }

    /// `Draw` (`menu.cpp:81-125`): up to `height` visible rows from `top_item`, then the
    /// scrollbar iff `visible_item_count > height`. Any negative `x` means `self.x`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw<M: MenuModel>(
        &self,
        m: &M,
        bmp: &mut Bitmap,
        pal: &Pal32,
        font: &Font,
        disabled: bool,
        x: i32,
        show_disabled_selection: bool,
    ) {
        let x = if x < 0 { self.x } else { x };
        let mut items_left = self.height;
        let mut cur_y = self.y;
        let mut c = self.item_from_visible_index(self.top_item);
        while items_left > 0 && (c as usize) < self.items.len() {
            let item = &self.items[c as usize];
            if item.visible {
                items_left -= 1;
                let selected = c == self.selection && (!disabled || show_disabled_selection);
                let value = item.has_value.then_some(item.value.as_str());
                let colours = ItemColours {
                    color: item.color,
                    dis_colour: item.dis_colour,
                };
                draw_item_value(
                    bmp,
                    pal,
                    font,
                    &item.string,
                    value,
                    x,
                    cur_y,
                    selected,
                    disabled,
                    self.centered,
                    self.value_offset_x,
                    colours,
                );
                m.draw_item_overlay(item, x, cur_y, selected, disabled, bmp, pal);
                cur_y += self.item_height;
            }
            c += 1;
        }
        if self.visible_item_count > self.height {
            draw_scrollbar(
                bmp,
                pal,
                font,
                x,
                self.y,
                self.height,
                self.item_height,
                self.top_item,
                self.visible_item_count,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::bitmap::{Bitmap, Pal32};
    use render::font::Font;
    use render::menu::{ItemColours, draw_item_value};

    fn font() -> Font {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sprites/font.tga"
        );
        Font::load(&assets::sprite::Tga::load(&std::fs::read(path).unwrap()).unwrap())
    }

    fn ramp() -> Pal32 {
        std::array::from_fn(|i| 0xFF00_0000 | i as u32)
    }

    /// `n` plain visible, selectable items `ITEM k`, ids `k`, at (40, 10), height `h`.
    fn items(n: i32, h: i32) -> Menu {
        let mut m = Menu::new(40, 10, false);
        m.height = h;
        for k in 0..n {
            m.add_item(MenuItem::new(48, 7, &format!("ITEM {k:02}"), k));
        }
        m
    }

    /// [A, B, space, hidden, grey(unselectable), C] — ids 0, 1, -1, 3, 4, 5.
    fn mixed() -> Menu {
        let mut m = Menu::new(40, 10, false);
        m.add_item(MenuItem::new(48, 7, "A", 0));
        m.add_item(MenuItem::new(48, 7, "B", 1));
        m.add_item(MenuItem::space());
        let mut hidden = MenuItem::new(48, 7, "HIDDEN", 3);
        hidden.visible = false;
        m.add_item(hidden);
        let mut grey = MenuItem::new(48, 7, "GREY", 4);
        grey.selectable = false;
        m.add_item(grey);
        m.add_item(MenuItem::new(48, 7, "C", 5));
        m
    }

    #[test]
    fn add_item_counts_visible_items_but_does_not_set_the_bottom() {
        // menu.cpp:295-302: AddItem never calls SetTop, so bottom_item stays 0 until the first
        // MoveTo -> EnsureInView -> SetBottom (design §3.4).
        let mut m = items(20, 15);
        assert_eq!(
            (m.visible_item_count, m.top_item, m.bottom_item),
            (20, 0, 0)
        );
        assert!(!m.is_in_view(0), "bottom 0: nothing is in view yet");
        m.move_to_first_visible();
        assert_eq!((m.selection(), m.top_item, m.bottom_item), (0, 0, 15));
        assert_eq!(
            mixed().visible_item_count,
            5,
            "the hidden item is not counted"
        );
    }

    #[test]
    fn movement_wraps_over_invisible_and_unselectable_items() {
        let mut m = mixed();
        m.move_to_first_visible();
        let mut seen = vec![m.selection()];
        for _ in 0..3 {
            m.movement(1);
            seen.push(m.selection());
        }
        for _ in 0..3 {
            m.movement(-1);
            seen.push(m.selection());
        }
        assert_eq!(seen, [0, 1, 5, 0, 5, 1, 0], "menu.cpp:263-293");
        m.movement(0);
        assert_eq!(m.selection(), 0, "direction 0 does nothing");
    }

    #[test]
    fn move_to_clamps_then_skips_forward_to_a_selectable_item() {
        let mut m = mixed();
        m.move_to(99);
        assert_eq!(m.selection(), 5, "clamped to the last item");
        m.move_to(-5);
        assert_eq!(m.selection(), 0);
        m.move_to(2);
        assert_eq!(
            m.selection(),
            5,
            "the spacer, hidden and grey items are skipped forward"
        );
        m.move_to_id(1);
        assert_eq!(m.selection(), 1);
        m.move_to_id(77);
        assert_eq!(m.selection(), 0, "IndexFromId -1 clamps to 0");
    }

    #[test]
    fn with_nothing_selectable_the_selection_is_the_item_count() {
        let mut m = Menu::new(0, 0, false);
        m.add_item(MenuItem::space());
        m.add_item(MenuItem::space());
        m.move_to_first_visible();
        assert_eq!(
            m.selection(),
            2,
            "FirstVisibleFrom returns items.size() (menu.cpp:169-177)"
        );
        assert!(m.selected().is_none() && !m.is_selection_valid());
        assert_eq!(m.selected_id(), -1);
        assert_eq!(
            m.first_visible_from(-1),
            2,
            "`std::size_t i = item` wraps a negative start"
        );
    }

    #[test]
    fn last_visible_from_returns_one_past_the_item_it_finds() {
        let m = mixed();
        assert_eq!(m.last_visible_from(6), 6, "C at 5 -> 6 (menu.cpp:179-187)");
        assert_eq!(m.last_visible_from(5), 2, "B at 1 -> 2");
        assert_eq!(m.last_visible_from(0), 0);
    }

    #[test]
    fn set_top_clamps_and_sets_the_bottom() {
        let mut m = items(20, 15);
        m.set_top(10);
        assert_eq!(
            (m.top_item, m.bottom_item),
            (5, 20),
            "clamped to vis - height"
        );
        m.set_top(-3);
        assert_eq!((m.top_item, m.bottom_item), (0, 15));
        m.scroll(2);
        assert_eq!((m.top_item, m.bottom_item), (2, 17));
        m.set_height(10);
        assert_eq!(
            (m.top_item, m.bottom_item),
            (2, 12),
            "SetHeight re-clamps via SetTop"
        );
        let mut short = items(4, 15);
        short.set_top(3);
        assert_eq!((short.top_item, short.bottom_item), (0, 4));
    }

    #[test]
    fn a_page_down_moves_half_a_screen_and_lands_forward_of_a_spacer() {
        // 20 items with a spacer at 7; menu.cpp:250-261: offset = 1 * (15 / 2) = 7.
        let mut m = Menu::new(40, 10, false);
        for k in 0..20 {
            m.add_item(if k == 7 {
                MenuItem::space()
            } else {
                MenuItem::new(48, 7, "X", k)
            });
        }
        m.move_to_first_visible();
        m.movement_page(1);
        assert_eq!(
            m.selection(),
            8,
            "visible index 7 is the spacer: MoveTo skips to 8"
        );
        assert_eq!(
            (m.top_item, m.bottom_item),
            (5, 20),
            "SetTop(0 + 7) clamped to 20 - 15"
        );
        m.movement_page(-1);
        assert_eq!(m.selection(), 1, "visible 8 - 7 = 1");
        assert_eq!(m.top_item, 0, "SetTop(5 - 7) clamps to 0 before the MoveTo");
    }

    #[test]
    fn hiding_the_selected_item_keeps_the_index_and_reanchors_the_top() {
        let mut m = items(10, 5);
        m.move_to(7);
        assert_eq!((m.top_item, m.bottom_item), (3, 8));
        m.set_visibility(7, false);
        assert_eq!(
            m.selection(),
            7,
            "EnsureInView ignores an invisible item (menu.cpp:155-158)"
        );
        assert_eq!((m.visible_item_count, m.top_item, m.bottom_item), (9, 3, 8));
        m.set_visibility(0, false);
        assert_eq!(
            (m.top_item, m.bottom_item),
            (2, 7),
            "the same real item (3) stays on top"
        );
        m.set_visibility(0, true);
        m.set_visibility(7, true);
        assert_eq!((m.visible_item_count, m.top_item), (10, 3));
    }

    #[test]
    fn item_position_is_only_for_items_in_view() {
        let mut m = items(20, 15);
        m.move_to_first_visible();
        assert_eq!(m.item_position(3), Some((40, 10 + 3 * 8)));
        assert_eq!(m.item_position(16), None);
        m.move_to(19);
        assert_eq!(m.item_position(19), Some((40, 10 + 14 * 8)));
    }

    #[test]
    fn add_item_at_returns_the_size_before_the_insert() {
        let mut m = items(3, 15);
        assert_eq!(
            m.add_item_at(MenuItem::new(1, 1, "FIRST", 9), 0),
            3,
            "menu.cpp:311-317"
        );
        assert_eq!(m.items[0].id, 9);
        assert_eq!(m.visible_item_count, 4);
    }

    #[test]
    fn draw_is_the_item_recipe_row_by_row_from_the_top_item() {
        let (font, pal) = (font(), ramp());
        let mut m = mixed();
        m.value_offset_x = 60;
        m.items[1].has_value = true;
        m.items[1].value = "ON".into();
        m.move_to(1);
        let mut got = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut got, &pal, &font, false, -1, false);
        let mut want = Bitmap::new(320, 200);
        let mut y = 10;
        for (i, it) in m.items.iter().enumerate() {
            if !it.visible {
                continue;
            }
            let value = it.has_value.then_some(it.value.as_str());
            let colours = ItemColours {
                color: it.color,
                dis_colour: it.dis_colour,
            };
            draw_item_value(
                &mut want,
                &pal,
                &font,
                &it.string,
                value,
                40,
                y,
                i == 1,
                false,
                false,
                60,
                colours,
            );
            y += 8;
        }
        assert_eq!(got, want);
        let mut at = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut at, &pal, &font, false, -7, false);
        assert_eq!(
            at, got,
            "any negative x means the menu's own x (menu.cpp:86-88)"
        );
    }

    #[test]
    fn a_disabled_draw_selects_only_with_show_disabled_selection() {
        let (font, pal) = (font(), ramp());
        let mut m = items(3, 15);
        m.move_to_first_visible();
        let mut off = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut off, &pal, &font, true, -1, false);
        assert!(!off.pixels.contains(&pal[168]) && off.pixels.contains(&pal[7]));
        let mut on = Bitmap::new(320, 200);
        m.draw(&PlainModel, &mut on, &pal, &font, true, -1, true);
        assert!(
            on.pixels.contains(&pal[7]) && !on.pixels.contains(&pal[168]),
            "disabled: dis_colour"
        );
        assert_ne!(on, off, "the selected row gets its box");
    }

    #[test]
    fn the_scrollbar_draws_iff_more_visible_items_than_rows() {
        let (font, pal) = (font(), ramp());
        // Colour 50 is only ever the arrows here (items are 48, the selected one 168).
        let column = |b: &Bitmap| b.pixels.contains(&pal[50]);
        for (n, want) in [(15, false), (16, true)] {
            let mut m = items(n, 15);
            m.move_to_first_visible();
            let mut b = Bitmap::new(320, 200);
            m.draw(&PlainModel, &mut b, &pal, &font, false, -1, false);
            assert_eq!(column(&b), want, "{n} items: menu.cpp:107");
        }
    }

    struct Counting(u32);
    impl MenuModel for Counting {
        fn behavior(&mut self, _id: i32) -> Behavior<'_> {
            Behavior::Plain
        }
        fn on_update(&mut self, menu: &mut Menu) {
            self.0 += 1;
            menu.items[0].string = "UPDATED".into();
        }
    }

    #[test]
    fn the_plain_behavior_and_the_empty_menu() {
        let mut sounds = Vec::new();
        let mut cx = MenuCx {
            menu_cycles: 0,
            hooks: MenuHooks::default(),
            sounds: &mut sounds,
        };
        let mut m = items(2, 15);
        m.move_to_first_visible();
        assert!(
            m.on_left_right(&mut PlainModel, 1, &mut cx),
            "ItemBehavior: true"
        );
        assert_eq!(m.on_enter(&mut PlainModel, &mut cx), Enter::Result(-1));
        let mut empty = Menu::new(0, 0, false);
        assert!(!empty.on_left_right(&mut PlainModel, 1, &mut cx));
        assert_eq!(
            empty.on_enter(&mut PlainModel, &mut cx),
            Enter::Result(0),
            "`return false;` (menu.hpp:81-84)"
        );
        let mut model = Counting(0);
        m.update_items(&mut model);
        assert_eq!(
            (model.0, m.items[0].string.as_str()),
            (1, "UPDATED"),
            "Menu::OnUpdate after the items"
        );
        assert!(sounds.is_empty());
    }

    use crate::keys::TypedKey;

    const HOOKS: MenuHooks = MenuHooks {
        move_up: 25,
        move_down: 26,
        select: 27,
    };

    /// A model over local variables, one per item id.
    struct Vars {
        int: i32,
        time: i32,
        flag: bool,
        en: u32,
        broken: u32,
        mode: u32,
        entry: bool,
        div: i32,
    }

    impl MenuModel for Vars {
        fn behavior(&mut self, id: i32) -> Behavior<'_> {
            match id {
                1 => {
                    let mut b = behavior::Integer::new(&mut self.int, 0, 10, 3, false);
                    b.allow_entry = self.entry;
                    b.display_div = self.div;
                    Behavior::Integer(b)
                }
                2 => Behavior::time(&mut self.time, 60, 3600, 10, false),
                3 => Behavior::Bool(&mut self.flag),
                4 => Behavior::Enum {
                    v: &mut self.en,
                    min: 0,
                    max: 3,
                    broken: false,
                },
                5 => Behavior::Enum {
                    v: &mut self.broken,
                    min: 1,
                    max: 3,
                    broken: true,
                },
                6 => Behavior::ArrayEnum {
                    v: &mut self.mode,
                    arr: &crate::text::GAME_MODES,
                    broken: false,
                },
                _ => Behavior::Plain,
            }
        }
    }

    fn vars() -> Vars {
        Vars {
            int: 5,
            time: 600,
            flag: false,
            en: 0,
            broken: 2,
            mode: 3,
            entry: true,
            div: 1,
        }
    }

    fn vars_menu() -> Menu {
        let mut m = Menu::new(30, 20, false);
        m.value_offset_x = 60;
        for id in 1..=6 {
            m.add_item(MenuItem::new(48, 7, &format!("V{id}"), id));
        }
        m
    }

    fn values(m: &Menu) -> Vec<String> {
        m.items
            .iter()
            .map(|i| {
                if i.has_value {
                    i.value.clone()
                } else {
                    ".".into()
                }
            })
            .collect()
    }

    #[test]
    fn on_update_fills_every_value() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.update_items(&mut v);
        assert_eq!(
            values(&m),
            ["5", "10:00", "OFF", "0", "2", "Scales of Justice"]
        );
    }

    #[test]
    fn integer_left_right_obeys_the_scroll_interval_and_clamps() {
        // integerBehavior.cpp:14-33: act only when menu_cycles % 5 == 0; clamp(v + dir*step).
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_first_visible();
        let mut sounds = Vec::new();
        for (cycles, dir, want) in [
            (1, 1, 5),
            (0, 1, 8),
            (5, 1, 10),
            (10, 1, 10),
            (0, -1, 7),
            (0, -1, 4),
            (0, -1, 1),
            (0, -1, 0),
            (0, -1, 0),
        ] {
            let mut cx = MenuCx {
                menu_cycles: cycles,
                hooks: HOOKS,
                sounds: &mut sounds,
            };
            assert!(
                m.on_left_right(&mut v, dir, &mut cx),
                "Integer returns true: the key stays held"
            );
            assert_eq!(v.int, want, "cycles {cycles} dir {dir}");
        }
        assert!(sounds.is_empty(), "Integer left/right plays nothing");
        assert_eq!(m.items[0].value, "0", "OnUpdate ran on change");
    }

    #[test]
    fn integer_enter_requests_an_edit_only_when_allowed_and_in_view() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_first_visible();
        let mut sounds = Vec::new();
        let mut cx = MenuCx {
            menu_cycles: 0,
            hooks: HOOKS,
            sounds: &mut sounds,
        };
        let e = m.on_enter(&mut v, &mut cx);
        assert_eq!(
            e,
            Enter::EditValue(ValueEntry {
                item_id: 1,
                initial: "5".into(),
                digits: 2,
                x: 30 + 60 + 2,
                y: 20,
                min: 0,
                max: 10,
                div: 1,
                percentage: false
            }),
            "integerBehavior.cpp:41-78: x = item x + voff + 2, digits = 1 + floor(log10(10))"
        );
        v.entry = false;
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1));
        v.entry = true;
        m.scroll(1);
        m.set_height(1);
        m.scroll(1);
        assert_eq!(
            m.on_enter(&mut v, &mut cx),
            Enter::Result(-1),
            "ItemPosition false: no entry"
        );
        assert_eq!(
            sounds,
            [27, 27, 27],
            "MenuSelect every time (integerBehavior.cpp:37)"
        );
    }

    #[test]
    fn digits_is_one_plus_floor_log10_for_every_power_of_ten() {
        let mut n = 1i64;
        let mut d = 1;
        while n <= i32::MAX as i64 {
            for k in [n, n + 1, 2 * n - 1] {
                if k <= i32::MAX as i64 {
                    assert_eq!(behavior::decimal_digits(k as i32), d, "{k}");
                }
            }
            if n > 1 {
                assert_eq!(behavior::decimal_digits((n - 1) as i32), d - 1, "{}", n - 1);
            }
            n *= 10;
            d += 1;
        }
        assert_eq!(behavior::decimal_digits(i32::MAX), 10);
    }

    #[test]
    fn time_uses_the_integer_step_but_never_allows_entry() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.update_items(&mut v);
        m.move_to_id(2);
        let mut sounds = Vec::new();
        let mut cx = MenuCx {
            menu_cycles: 0,
            hooks: HOOKS,
            sounds: &mut sounds,
        };
        assert!(m.on_left_right(&mut v, -1, &mut cx));
        assert_eq!((v.time, m.items[1].value.as_str()), (590, "09:50"));
        assert_eq!(
            m.on_enter(&mut v, &mut cx),
            Enter::Result(-1),
            "timeBehavior.hpp:11: allow_entry = false"
        );
        assert_eq!(sounds, [27]);
    }

    #[test]
    fn a_boolean_switch_toggles_with_crossed_sounds_and_releases_left_right() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_id(3);
        let mut sounds = Vec::new();
        let mut cx = MenuCx {
            menu_cycles: 1,
            hooks: HOOKS,
            sounds: &mut sounds,
        };
        assert!(
            !m.on_left_right(&mut v, 1, &mut cx),
            "booleanSwitchBehavior.cpp:17: false"
        );
        assert!(v.flag && m.items[2].value == "ON");
        assert!(!m.on_left_right(&mut v, -1, &mut cx));
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1));
        assert!(v.flag);
        assert_eq!(
            sounds,
            [25, 26, 27],
            "dir > 0: MoveUp, else MoveDown; Enter: Select"
        );
    }

    #[test]
    fn an_enum_wraps_in_u32_and_updates_every_item() {
        let (mut m, mut v) = (vars_menu(), vars());
        m.move_to_id(4);
        let mut sounds = Vec::new();
        let mut cx = MenuCx {
            menu_cycles: 3,
            hooks: HOOKS,
            sounds: &mut sounds,
        };
        let mut seen = Vec::new();
        for dir in [-1, -1, 1, 1, 1] {
            assert!(!m.on_left_right(&mut v, dir, &mut cx));
            seen.push(v.en);
        }
        assert_eq!(seen, [3, 2, 3, 0, 1], "enumBehavior.cpp:31-39");
        assert_eq!(
            m.items[5].value, "Scales of Justice",
            "Change -> UpdateItems ran every OnUpdate"
        );
        assert_eq!(m.on_enter(&mut v, &mut cx), Enter::Result(-1));
        assert_eq!(v.en, 2);
        m.move_to_id(5);
        assert!(
            !m.on_left_right(&mut v, 1, &mut cx),
            "broken: false, no change, no sound"
        );
        assert_eq!(v.broken, 2);
        m.on_enter(&mut v, &mut cx);
        assert_eq!(v.broken, 3, "broken only affects left/right");
        m.move_to_id(6);
        m.on_left_right(&mut v, 1, &mut cx);
        assert_eq!(
            (v.mode, m.items[5].value.as_str()),
            (0, "Kill'em All"),
            "ArrayEnum: [0, N-1] wraps"
        );
        assert_eq!(
            sounds.len(),
            5 + 1 + 1 + 1,
            "5 left/right + Enter + broken Enter + array right"
        );
    }

    fn search_menu() -> Menu {
        let mut m = Menu::new(40, 20, false);
        for (id, s) in [
            "APPLE",
            "APRICOT",
            "BANANA",
            "BLUEBERRY",
            "CHERRY GREY",
            "COCONUT",
            "DATE",
            "",
            "apricot jam",
        ]
        .iter()
        .enumerate()
        {
            let mut it = MenuItem::new(48, 7, s, id as i32);
            it.visible = id != 3;
            it.selectable = id != 4;
            m.add_item(it);
        }
        m.move_to_first_visible();
        m
    }

    #[test]
    fn type_to_search_prefix_timeout_retry_and_skips() {
        // menu.cpp:14-79; ms from the caller's clock (design §4.5).
        let sym = |c: char| TypedKey::Sym(c as u32);
        let mut m = search_menu();
        let at = |m: &mut Menu, keys: &[TypedKey], t: u64, contains: bool| {
            m.on_keys(keys, t, contains);
            (m.selection(), m.search.prefix.clone())
        };
        assert_eq!(
            at(&mut m, &[sym('a'), sym('p')], 0, false),
            (0, "ap".into()),
            "APPLE stays (offs 0)"
        );
        assert_eq!(at(&mut m, &[sym('r')], 200, false), (1, "apr".into()));
        assert_eq!(
            at(&mut m, &[sym('b')], 2000, false),
            (2, "b".into()),
            "> 1500 ms: a new prefix"
        );
        assert_eq!(
            at(&mut m, &[sym('l')], 2100, false),
            (2, "".into()),
            "BLUEBERRY is invisible; retry 'l' fails"
        );
        assert_eq!(
            at(&mut m, &[sym('c')], 2200, false),
            (5, "c".into()),
            "CHERRY (unselectable) -> MoveTo skips to COCONUT"
        );
        assert_eq!(
            at(&mut m, &[TypedKey::Tab], 9000, false),
            (5, "c".into()),
            "Tab: no timeout, skip 1, CHERRY again"
        );
        assert_eq!(
            at(&mut m, &[sym('j')], 9100, true),
            (8, "j".into()),
            "'cj' fails; retry 'j' contains"
        );
        assert_eq!(
            at(&mut m, &[TypedKey::Tab], 9200, true),
            (0, "".into()),
            "no other 'j'; retry with '' matches APPLE"
        );
        m.move_to(6);
        assert_eq!(
            at(&mut m, &[TypedKey::Tab], 9300, true),
            (8, "".into()),
            "contains '' never matches an empty string"
        );
        m.move_to(6);
        assert_eq!(
            at(&mut m, &[TypedKey::Tab], 9400, false),
            (7, "".into()),
            "a prefix '' does"
        );
        let before = (m.selection(), m.search.clone());
        m.on_keys(&[TypedKey::Sym(0x4000_0052)], 99_999, false);
        assert_eq!(
            (m.selection(), m.search.clone()),
            before,
            "a non-ASCII symbol is ignored entirely"
        );
    }
}
