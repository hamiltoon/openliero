//! Step 4½e-2 — C++ `FileNode` / `FileSelector` (`menu/fileSelector.hpp`) and the two selector
//! states the settings menu pushes (`fileSelectorState.cpp`): `LevelSelectorState` (LEVEL) and
//! `OptionsSelectorState` (LOAD SETUP, here [`SetupSelectorState`]). Plan facts 1-10, D4, D5.
//!
//! The tree is an arena of [`FileNode`]s filled lazily through the [`ConfigStore`] (C++
//! `EnsureFilled` / `GetMenu`): a real user folder may hold thousands of replays or a symlink
//! loop, so nothing ever walks the whole tree. A node's `rel` is its store path (`""` for the
//! root), its `full_path` the C++ string, built by `JoinPath` over the store's `root_label()`.
//! Node indices never move, so an index is C++'s `FileNode*` (`previewNode_`, `CurSel`).

use assets::level::load as load_level;
use render::bitmap::{Bitmap, Pal32, Rect};
use render::font::Font;
use render::hud::draw_miniature_ids;
use scenario::storage::ConfigStore;

use super::MenuWorld;
use super::level_path::cpp_accepts;
use super::main_menu::{MenuCtx, play};
use crate::keys::{
    DK_DOWN, DK_ESCAPE, DK_KP_ENTER, DK_LEFT, DK_PGDN, DK_PGUP, DK_RETURN, DK_RIGHT, DK_UP, K_DOWN,
    K_FIRE, K_JUMP, K_LEFT, K_RIGHT, K_UP,
};
use crate::menu::{Menu, MenuItem, PlainModel};
use crate::text::{ci_compare, ci_less, ci_starts_with, get_basename, get_extension, join_path};

/// `FileFilter` (`fileSelector.hpp:20`): `(name, extension)` → keep a file row.
pub type FileFilter = fn(&str, &str) -> bool;

/// The level selector's filter (`fileSelectorState.cpp:83-84`).
pub fn lev_filter(_name: &str, ext: &str) -> bool {
    ci_compare(ext, "LEV")
}

/// The options selector's filter (`fileSelectorState.cpp:216-217`).
pub fn cfg_filter(_name: &str, ext: &str) -> bool {
    ci_compare(ext, "CFG")
}

/// The HUD minimap's box (`Level::kHudMinimapW/H`): the preview's first clear
/// (`fileSelectorState.hpp:43-44`) and its step divisor.
const HUD_MINIMAP_W: i32 = 52;
const HUD_MINIMAP_H: i32 = 36;
/// Where the preview goes in the frozen screen (`fileSelectorState.cpp:120-121`).
const PREVIEW_X: i32 = 134;
const PREVIEW_Y: i32 = 162;

/// C++ `FileNode` (`fileSelector.hpp:22-119`).
#[derive(Clone, Debug)]
pub struct FileNode {
    /// The row text: a folder's name, a file's `GetBasename` (fact 4), `[RANDOM]`.
    pub name: String,
    /// The store path (`""` for the root and for RANDOM).
    pub rel: String,
    /// `JoinPath(parent.full_path, entry)`; the root's is `root_label()`, RANDOM's `""`.
    pub full_path: String,
    pub folder: bool,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
    /// `selected_child` (`Select` and `Enter` set it; nothing reads it, as in C++).
    pub selected_child: Option<usize>,
    menu: Menu,
    filled: bool,
}

impl FileNode {
    fn new(
        name: String,
        rel: String,
        full_path: String,
        folder: bool,
        parent: Option<usize>,
    ) -> Self {
        // `menu(178, 28)` + `SetHeight(14)` (`fileSelector.hpp:23-47`).
        let mut menu = Menu::new(178, 28, false);
        menu.set_height(14);
        FileNode {
            name,
            rel,
            full_path,
            folder,
            children: Vec::new(),
            parent,
            selected_child: None,
            menu,
            filled: false,
        }
    }

    /// The node's menu as last built (no `GetMenu`: no fill, no rebuild).
    pub fn menu(&self) -> &Menu {
        &self.menu
    }
}

/// `ChildSort` (`fileSelector.hpp:121-130`): folders first, then `CiLess` on the name. Rust's
/// stable sort keeps names equal under `CiLess` in listing (byte) order (plan D14).
fn child_order(a: &FileNode, b: &FileNode) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a.folder != b.folder {
        return if a.folder {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if ci_less(&a.name, &b.name) {
        Ordering::Less
    } else if ci_less(&b.name, &a.name) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// A store path's child.
fn join_rel(rel: &str, name: &str) -> String {
    if rel.is_empty() {
        name.to_string()
    } else {
        format!("{rel}/{name}")
    }
}

/// C++ `FileSelector` (`fileSelector.hpp:156-306`) over a node arena; node 0 is `root_node`.
#[derive(Clone, Debug)]
pub struct FileSelector {
    nodes: Vec<FileNode>,
    current: usize,
    filter: FileFilter,
}

impl FileSelector {
    pub const ROOT: usize = 0;

    /// `FileSelector` + `Fill(GetConfigNode(), filter)` (`fileSelector.hpp:158-169`): the root's
    /// `full_path` is the config node's `FullPath()` (`root_label()`, fact 6), and it is filled
    /// at once. The current folder is the root.
    pub fn new(store: &dyn ConfigStore, filter: FileFilter) -> FileSelector {
        let root = FileNode::new(
            String::new(),
            String::new(),
            store.root_label().to_string(),
            true,
            None,
        );
        let mut sel = FileSelector {
            nodes: vec![root],
            current: Self::ROOT,
            filter,
        };
        sel.fill(store, Self::ROOT);
        sel
    }

    /// A selector with nothing listed (the state before its `Enter`).
    fn empty() -> FileSelector {
        let root = FileNode::new(String::new(), String::new(), String::new(), true, None);
        FileSelector {
            nodes: vec![root],
            current: Self::ROOT,
            filter: lev_filter,
        }
    }

    pub fn node(&self, idx: usize) -> &FileNode {
        &self.nodes[idx]
    }

    pub fn current(&self) -> usize {
        self.current
    }

    /// Insert a node as child `pos` of `parent` (the level selector's RANDOM, inserted after the
    /// root's `Fill`, `fileSelectorState.cpp:79-91`).
    fn insert_child(&mut self, parent: usize, pos: usize, mut node: FileNode) -> usize {
        node.parent = Some(parent);
        let idx = self.nodes.len();
        self.nodes.push(node);
        self.nodes[parent].children.insert(pos, idx);
        idx
    }

    /// `FileNode::Fill` (`fileSelector.hpp:132-154`; fact 4): every directory and each file the
    /// filter keeps, as `store.list` gives them, then `ChildSort`.
    fn fill(&mut self, store: &dyn ConfigStore, idx: usize) {
        let (rel, full) = (
            self.nodes[idx].rel.clone(),
            self.nodes[idx].full_path.clone(),
        );
        let mut kids = Vec::new();
        for e in store.list(&rel) {
            let child_rel = join_rel(&rel, &e.name);
            let child_full = join_path(&full, &e.name);
            if e.is_dir {
                kids.push(FileNode::new(
                    e.name,
                    child_rel,
                    child_full,
                    true,
                    Some(idx),
                ));
            } else if (self.filter)(&e.name, get_extension(&e.name)) {
                let name = get_basename(&e.name).to_string();
                kids.push(FileNode::new(name, child_rel, child_full, false, Some(idx)));
            }
        }
        kids.sort_by(child_order);
        for k in kids {
            let c = self.nodes.len();
            self.nodes.push(k);
            self.nodes[idx].children.push(c);
        }
        self.nodes[idx].filled = true;
    }

    /// `EnsureFilled` (`fileSelector.hpp:106-111`): the root is filled by `Fill` only.
    fn ensure_filled(&mut self, store: &dyn ConfigStore, idx: usize) {
        if !self.nodes[idx].filled && self.nodes[idx].parent.is_some() {
            self.fill(store, idx);
        }
    }

    /// `GetMenu` (`fileSelector.hpp:51-63`; fact 3): fill, then (re)build an empty menu from the
    /// children — `MenuItem(folder ? 47 : 48, 7, name)` — and `MoveToFirstVisible`.
    pub fn get_menu(&mut self, store: &dyn ConfigStore, idx: usize) -> &mut Menu {
        self.ensure_filled(store, idx);
        if self.nodes[idx].menu.items.is_empty() {
            let items: Vec<MenuItem> = self.nodes[idx]
                .children
                .iter()
                .map(|&c| {
                    let n = &self.nodes[c];
                    MenuItem::new(if n.folder { 47 } else { 48 }, 7, &n.name, -1)
                })
                .collect();
            let menu = &mut self.nodes[idx].menu;
            for it in items {
                menu.add_item(it);
            }
            menu.move_to_first_visible();
        }
        &mut self.nodes[idx].menu
    }

    /// `CurrentMenu()`.
    pub fn current_menu(&mut self, store: &dyn ConfigStore) -> &mut Menu {
        self.get_menu(store, self.current)
    }

    /// `FileNode::Find` (`fileSelector.hpp:66-87`; fact 8): a `CiCompare` hit, else recurse into
    /// a folder whose `full_path` case-insensitively prefixes `path` — with no separator check —
    /// filling it on the way.
    fn find(&mut self, store: &dyn ConfigStore, idx: usize, path: &str) -> Option<usize> {
        if ci_compare(&self.nodes[idx].full_path, path) {
            return Some(idx);
        }
        if !ci_starts_with(path, &self.nodes[idx].full_path) || !self.nodes[idx].folder {
            return None;
        }
        self.ensure_filled(store, idx);
        for i in 0..self.nodes[idx].children.len() {
            let c = self.nodes[idx].children[i];
            if let Some(r) = self.find(store, c, path) {
                return Some(r);
            }
        }
        None
    }

    /// `SetFolder`.
    pub fn set_folder(&mut self, idx: usize) {
        self.current = idx;
    }

    /// `FileSelector::Select` (`fileSelector.hpp:184-214`; fact 8): on a hit every ancestor's
    /// `selected_child` is the hit and its cursor moves onto the path (its `GetMenu` built
    /// first); the current folder becomes the hit if it is a folder, else its parent. A miss (or
    /// the root itself) leaves everything as it was.
    pub fn select(&mut self, store: &dyn ConfigStore, path: &str) -> bool {
        let Some(fnode) = self.find(store, Self::ROOT, path) else {
            return false;
        };
        let Some(parent) = self.nodes[fnode].parent else {
            return true;
        };
        let mut p = fnode;
        while let Some(up) = self.nodes[p].parent {
            let ch = p;
            p = up;
            self.nodes[p].selected_child = Some(fnode);
            for i in 0..self.nodes[p].children.len() {
                if self.nodes[p].children[i] == ch {
                    self.get_menu(store, p).move_to(i as i32);
                }
            }
        }
        self.set_folder(if self.nodes[fnode].folder {
            fnode
        } else {
            parent
        });
        true
    }

    /// `CurSel` (`fileSelector.hpp:216-224`): the child under the current menu's cursor.
    pub fn cur_sel(&mut self, store: &dyn ConfigStore) -> Option<usize> {
        let cur = self.current;
        let menu = self.get_menu(store, cur);
        if !menu.is_selection_valid() {
            return None;
        }
        let i = menu.selection() as usize;
        Some(self.nodes[cur].children[i])
    }

    /// `FileSelector::Enter` (`fileSelector.hpp:226-240`): into a folder row (`None`), or the file
    /// row picked.
    pub fn enter(&mut self, store: &dyn ConfigStore) -> Option<usize> {
        let c = self.cur_sel(store)?;
        if self.nodes[c].folder {
            let cur = self.current;
            self.nodes[cur].selected_child = Some(c);
            self.set_folder(c);
            return None;
        }
        Some(c)
    }

    /// `FileSelector::Exit` (`fileSelector.hpp:242-249`): to the parent; nothing at the root.
    pub fn exit(&mut self) -> bool {
        match self.nodes[self.current].parent {
            None => false,
            Some(p) => {
                self.set_folder(p);
                true
            }
        }
    }

    /// `FileSelector::Process` (`fileSelector.hpp:251-300`; fact 9), in C++ order: Up (plays
    /// MoveDown), Down (MoveUp), PgUp, PgDn — each a once-key or a keyboard player's control, the
    /// pages keyboard-only; Esc or any Jump → `false` (leave, no sound); once-Left `Exit`;
    /// once-Right `Enter` (no sound); then the `contains` type-to-search over `key_buf`.
    pub fn process(&mut self, cx: &mut MenuCtx) -> bool {
        let store = cx.store;
        let now_ms = cx.now_ms;
        let MenuCtx { w, sounds, .. } = cx;
        let hooks = w.tc.hooks;
        let ws = &w.settings.worm_settings;
        if w.keys.test_once(DK_UP) || w.keys.test_control_once(ws, K_UP) {
            play(sounds, hooks.move_down);
            self.current_menu(store).movement(-1);
        }
        if w.keys.test_once(DK_DOWN) || w.keys.test_control_once(ws, K_DOWN) {
            play(sounds, hooks.move_up);
            self.current_menu(store).movement(1);
        }
        if w.keys.test_once(DK_PGUP) {
            play(sounds, hooks.move_down);
            self.current_menu(store).movement_page(-1);
        }
        if w.keys.test_once(DK_PGDN) {
            play(sounds, hooks.move_up);
            self.current_menu(store).movement_page(1);
        }
        if w.keys.test_once(DK_ESCAPE) || w.keys.test_control_once(ws, K_JUMP) {
            return false;
        }
        if w.keys.test_once(DK_LEFT) || w.keys.test_control_once(ws, K_LEFT) {
            self.exit();
        }
        if w.keys.test_once(DK_RIGHT) || w.keys.test_control_once(ws, K_RIGHT) {
            self.enter(store);
        }
        self.current_menu(store)
            .on_keys(w.keys.typed(), now_ms, true);
        true
    }

    /// `FileSelector::Draw` (`fileSelector.hpp:171-178`; fact 2): with a parent, the framed
    /// `"Parent directory"` at (28, 20) and the parent's menu disabled at x = 28, its selection
    /// shown; then the current menu enabled at x = 178.
    pub fn draw(
        &mut self,
        store: &dyn ConfigStore,
        surface: &mut Bitmap,
        pal: &Pal32,
        font: &Font,
    ) {
        if let Some(p) = self.nodes[self.current].parent {
            font.draw_framed_text(surface, pal, "Parent directory", 28, 20, 50);
            self.get_menu(store, p)
                .draw(&PlainModel, surface, pal, font, true, 28, true);
        }
        self.current_menu(store)
            .draw(&PlainModel, surface, pal, font, false, 178, false);
    }
}

/// What a selector's file pick asks of the shell (plan D4): C++ `OnSelected`, run by
/// `Shell::frame` after the update and before the stack's pop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// `[RANDOM]`: `random_level = true`, `level_file` cleared.
    Random,
    /// A level file: `random_level = false`, `level_file = full_path`.
    Level(String),
    /// A setup file (LOAD SETUP, 4½e-2 T4): its store path and `GetBasename(GetLeaf(..))`.
    Setup { rel: String, name: String },
}

/// A selector's `Update` result (plan D4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorOut {
    pub keep: bool,
    pub picked: Option<Picked>,
}

/// What the harness and the glue read of a selector on top (`Shell::selector_view`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorView {
    /// `L` or `P` (the G2 top).
    pub top: char,
    /// The current folder's `full_path`.
    pub folder: String,
    /// The current menu's cursor.
    pub selection: i32,
}

/// `FileSelectorState::Update` (`fileSelectorState.cpp:25-50`): `Process`, then Return, KP Enter
/// or any Fire plays `MenuSelect` and `Enter`s; a file row is the pick.
fn selector_update(sel: &mut FileSelector, cx: &mut MenuCtx) -> (bool, Option<usize>) {
    if !sel.process(cx) {
        return (false, None);
    }
    let w = &mut *cx.w;
    if w.keys.test_once(DK_RETURN)
        || w.keys.test_once(DK_KP_ENTER)
        || w.keys.test_control_once(&w.settings.worm_settings, K_FIRE)
    {
        play(cx.sounds, w.tc.hooks.select);
        if let Some(n) = sel.enter(cx.store) {
            return (false, Some(n));
        }
    }
    (true, None)
}

/// `bmp.Copy(frozen_screen)` (`fileSelectorState.cpp:53`).
fn copy_frozen(w: &mut MenuWorld) {
    w.surface.pixels.copy_from_slice(&w.frozen.pixels);
    w.surface.clip = Rect::new(0, 0, w.surface.w, w.surface.h);
}

/// A selector title: `text` + `' '` + the current folder's `full_path`, the space and path
/// dropped when the path is empty (fact 1).
fn title(text: &str, path: &str) -> String {
    if path.is_empty() {
        text.to_string()
    } else {
        format!("{text} {path}")
    }
}

/// C++ `LevelSelectorState` (`fileSelectorState.cpp:69-156`; facts 1, 2, 7, 10).
#[derive(Clone, Debug)]
pub struct LevelSelectorState {
    selector: FileSelector,
    random: usize,
    /// `previewNode_`.
    preview: Option<usize>,
    /// `prev_hud_cols_`, `prev_hud_rows_`.
    prev: (i32, i32),
}

impl Default for LevelSelectorState {
    fn default() -> Self {
        LevelSelectorState::new()
    }
}

impl LevelSelectorState {
    /// The state before its `Enter` (C++ makes `selector_` there).
    pub fn new() -> LevelSelectorState {
        LevelSelectorState {
            selector: FileSelector::empty(),
            random: usize::MAX,
            preview: None,
            prev: (HUD_MINIMAP_W, HUD_MINIMAP_H),
        }
    }

    pub fn selector(&self) -> &FileSelector {
        &self.selector
    }

    /// The RANDOM node.
    pub fn random_node(&self) -> usize {
        self.random
    }

    /// `previewNode_`.
    pub fn preview_node(&self) -> Option<usize> {
        self.preview
    }

    /// `Enter` (`fileSelectorState.cpp:76-93`): fill the root with `LEV`, insert `[RANDOM]` (full
    /// path `""`, a file row) first, the root current, `Select(level_file)` (the cursor restore;
    /// a miss — random, a TC-relative path, another root — opens on `[RANDOM]`), no preview yet,
    /// and the first clear the full 52×36.
    pub fn enter(&mut self, w: &mut MenuWorld, store: &dyn ConfigStore) {
        let mut sel = FileSelector::new(store, lev_filter);
        let node = FileNode::new(
            w.tc.random.clone(),
            String::new(),
            String::new(),
            false,
            None,
        );
        self.random = sel.insert_child(FileSelector::ROOT, 0, node);
        sel.set_folder(FileSelector::ROOT);
        sel.select(store, &w.settings.level_file);
        self.selector = sel;
        self.preview = None;
        self.prev = (HUD_MINIMAP_W, HUD_MINIMAP_H);
    }

    /// `FileSelectorState::Update` + `OnSelected` (`:78-103`): `[RANDOM]` or the file's
    /// `full_path`.
    pub fn update(&mut self, cx: &mut MenuCtx) -> SelectorOut {
        let (keep, pick) = selector_update(&mut self.selector, cx);
        let picked = pick.map(|n| {
            if n == self.random {
                Picked::Random
            } else {
                Picked::Level(self.selector.node(n).full_path.clone())
            }
        });
        SelectorOut { keep, picked }
    }

    /// `FileSelectorState::Draw` with `DrawExtra` (`:52-67`, `:105-156`; fact 2): the frozen
    /// screen, then the preview — into the FROZEN screen, so this frame's surface shows it one
    /// frame late (T0 P3) — then the framed title, then the selector.
    pub fn draw(&mut self, w: &mut MenuWorld, store: &dyn ConfigStore, font: &Font) {
        copy_frozen(w);
        self.draw_preview(w, store);
        let path = &self.selector.node(self.selector.current()).full_path;
        let t = title(&w.tc.sel_level, path);
        font.draw_framed_text(&mut w.surface, &w.pal32, &t, 178, 20, 50);
        self.selector.draw(store, &mut w.surface, &w.pal32, font);
    }

    /// `DrawExtra`'s preview (`:108-145`; fact 10): only for a file row other than RANDOM that is
    /// not `previewNode_`. A level `Level::load` accepts ([`cpp_accepts`] with this settings'
    /// `load_powerlevel_palette`) clears what the last preview drew and draws its miniature
    /// through this frame's palette; `previewNode_` moves on even when the load failed. (The
    /// spectator miniature is deferred with the spectator window.)
    fn draw_preview(&mut self, w: &mut MenuWorld, store: &dyn ConfigStore) {
        let Some(sel) = self.selector.cur_sel(store) else {
            return;
        };
        let node = self.selector.node(sel);
        if Some(sel) == self.preview || sel == self.random || node.folder {
            return;
        }
        let level = store
            .read(&node.rel)
            .filter(|b| cpp_accepts(b, w.settings.load_powerlevel_palette))
            .and_then(|b| load_level(&b).ok());
        if let Some(lv) = level {
            let sx = ((lv.width + HUD_MINIMAP_W - 1) / HUD_MINIMAP_W).max(1);
            let sy = ((lv.height + HUD_MINIMAP_H - 1) / HUD_MINIMAP_H).max(1);
            let (cols, rows) = self.prev;
            w.frozen
                .fill_rect(PREVIEW_X, PREVIEW_Y, cols, rows, 0, &w.pal32);
            draw_miniature_ids(
                &mut w.frozen,
                &w.pal32,
                &lv.material_id,
                lv.width,
                lv.height,
                PREVIEW_X,
                PREVIEW_Y,
                sx,
                sy,
            );
            self.prev = ((lv.width + sx / 2) / sx, (lv.height + sy / 2) / sy);
        }
        self.preview = Some(sel);
    }

    pub fn view(&self) -> SelectorView {
        view('L', &self.selector)
    }
}

fn view(top: char, sel: &FileSelector) -> SelectorView {
    let cur = sel.node(sel.current());
    SelectorView {
        top,
        folder: cur.full_path.clone(),
        selection: cur.menu().selection(),
    }
}

/// C++ `OptionsSelectorState` (`fileSelectorState.cpp:206-227`; facts 1, 2, 8, 13): LOAD SETUP.
#[derive(Clone, Debug)]
pub struct SetupSelectorState {
    selector: FileSelector,
}

impl Default for SetupSelectorState {
    fn default() -> Self {
        SetupSelectorState::new()
    }
}

impl SetupSelectorState {
    /// The C++ literal title (`fileSelectorState.cpp:210`).
    pub const TITLE: &'static str = "Select options:";

    pub fn new() -> SetupSelectorState {
        SetupSelectorState {
            selector: FileSelector::empty(),
        }
    }

    pub fn selector(&self) -> &FileSelector {
        &self.selector
    }

    /// `Enter` (`:212-220`): fill the root with `CFG`, the root current, then
    /// `Select(JoinPath(root, "Setups"))` — a folder, so it opens inside `Setups`, its cursor
    /// placed by the first draw's `GetMenu`.
    pub fn enter(&mut self, _w: &mut MenuWorld, store: &dyn ConfigStore) {
        let mut sel = FileSelector::new(store, cfg_filter);
        sel.set_folder(FileSelector::ROOT);
        let setups = join_path(store.root_label(), "Setups");
        sel.select(store, &setups);
        self.selector = sel;
    }

    /// `FileSelectorState::Update`; a file row is `Picked::Setup` (`OnSelected`, `:222-226`).
    pub fn update(&mut self, cx: &mut MenuCtx) -> SelectorOut {
        let (keep, pick) = selector_update(&mut self.selector, cx);
        let picked = pick.map(|n| {
            let node = self.selector.node(n);
            Picked::Setup {
                rel: node.rel.clone(),
                name: crate::text::leaf_basename(&node.full_path).to_string(),
            }
        });
        SelectorOut { keep, picked }
    }

    /// `FileSelectorState::Draw` (`:52-67`): the frozen screen, the framed `"Select options:"`
    /// title (+ the path), the selector.
    pub fn draw(&mut self, w: &mut MenuWorld, store: &dyn ConfigStore, font: &Font) {
        copy_frozen(w);
        let path = &self.selector.node(self.selector.current()).full_path;
        let t = title(Self::TITLE, path);
        font.draw_framed_text(&mut w.surface, &w.pal32, &t, 178, 20, 50);
        self.selector.draw(store, &mut w.surface, &w.pal32, font);
    }

    pub fn view(&self) -> SelectorView {
        view('P', &self.selector)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;
    use std::sync::Mutex;

    use render::bitmap::Bitmap;
    use scenario::paths::TC_ROOT;
    use scenario::settings::Settings;
    use scenario::storage::{DirEntry, MemoryStore};

    use super::*;
    use crate::keys::{KeyLatch, TypedKey};
    use crate::shell::main_menu::main_menu;
    use crate::shell::overlay::RefusalGate;
    use crate::shell::settings_menu::settings_menu;
    use crate::shell::{CurMenu, SURFACE_H, SURFACE_W};
    use crate::text::UiTc;

    const LEVELS: &str = "TC/openliero/Levels";

    fn data(rel: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../data")
                .join(rel),
        )
        .unwrap()
    }

    fn level(name: &str) -> Vec<u8> {
        data(&format!("{LEVELS}/{name}.lev"))
    }

    /// T0's `tiny.lev`: `OLLEVEL2` 60×40, material `(x + y) % 64 + 160`.
    pub(crate) fn tiny() -> Vec<u8> {
        let mut v = b"OLLEVEL2\x01".to_vec();
        v.extend(60u16.to_le_bytes());
        v.extend(40u16.to_le_bytes());
        for y in 0..40u32 {
            for x in 0..60u32 {
                v.push(((x + y) % 64 + 160) as u8);
            }
        }
        v
    }

    /// T0's `trunc.lev`: `OLLEVEL2` 8×8 + `MODERNLV` + 10 zero bytes.
    pub(crate) fn trunc() -> Vec<u8> {
        let mut v = b"OLLEVEL2\x01\x08\x00\x08\x00".to_vec();
        v.extend([0xA0; 64]);
        v.extend(b"MODERNLV");
        v.extend([0; 10]);
        v
    }

    /// The plan's `Fs::install()` system layer (§Formats), root label `./user`.
    pub(crate) fn install() -> MemoryStore {
        let mut files: Vec<(String, Vec<u8>)> = ["Setups/liero.cfg", "Setups/orbmit.cfg"]
            .iter()
            .map(|r| (r.to_string(), data(r)))
            .collect();
        files.push(("TC/openliero/tc.cfg".into(), data("TC/openliero/tc.cfg")));
        for l in [
            "modern_test",
            "physics_fall_test",
            "render_stage",
            "see_shadow_test",
            "water_stage",
        ] {
            files.push((format!("{LEVELS}/{l}.lev"), level(l)));
        }
        let refs: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        MemoryStore::with_system(&refs)
            .with_dirs(&[
                "Profiles",
                "Resources",
                "TC/openliero/nobjects",
                "TC/openliero/sobjects",
                "TC/openliero/sounds",
                "TC/openliero/sprites",
                "TC/openliero/weapons",
            ])
            .with_root_label("./user")
    }

    /// T0 P1's user layer over [`install`].
    pub(crate) fn p1_store() -> MemoryStore {
        let s = install().with_dirs(&["Replays"]);
        let water = level("water_stage");
        for (name, bytes) in [
            ("Zeta.lev", water.clone()),
            ("alpha.LEV", water),
            (".hidden.lev", level("render_stage")),
            ("notes.txt", data("README.md")),
            ("tiny.lev", tiny()),
        ] {
            s.write(&format!("{LEVELS}/{name}"), &bytes).unwrap();
        }
        s
    }

    /// Counts `list` calls (fact 4's laziness).
    struct Counting {
        inner: MemoryStore,
        listed: Mutex<Vec<String>>,
    }

    impl ConfigStore for Counting {
        fn read(&self, rel: &str) -> Option<Vec<u8>> {
            self.inner.read(rel)
        }
        fn write(&self, rel: &str, bytes: &[u8]) -> std::io::Result<()> {
            self.inner.write(rel, bytes)
        }
        fn shadows_system(&self, subdir: &str, leaf: &str) -> bool {
            self.inner.shadows_system(subdir, leaf)
        }
        fn root_label(&self) -> &str {
            self.inner.root_label()
        }
        fn list(&self, rel: &str) -> Vec<DirEntry> {
            self.listed.lock().unwrap().push(rel.to_string());
            self.inner.list(rel)
        }
    }

    fn rows(sel: &mut FileSelector, store: &dyn ConfigStore) -> Vec<(String, u8)> {
        sel.current_menu(store)
            .items
            .iter()
            .map(|i| (i.string.clone(), i.color))
            .collect()
    }

    fn names(sel: &mut FileSelector, store: &dyn ConfigStore) -> Vec<String> {
        rows(sel, store).into_iter().map(|r| r.0).collect()
    }

    fn cur_path(sel: &FileSelector) -> &str {
        &sel.node(sel.current()).full_path
    }

    fn cursor(sel: &mut FileSelector, store: &dyn ConfigStore) -> i32 {
        sel.current_menu(store).selection()
    }

    pub(crate) fn world() -> MenuWorld {
        let tc = UiTc::load(Path::new(TC_ROOT));
        MenuWorld {
            main_menu: main_menu(),
            settings_menu: settings_menu(),
            player_menu: crate::shell::player_menu::player_menu(),
            cur_menu: CurMenu::Settings,
            profiles: Default::default(),
            settings: Settings::default(),
            origpal: tc.exepal.clone(),
            tc,
            setup_name: "liero".into(),
            keys: KeyLatch::default(),
            fade: 32,
            menu_cycles: 0,
            surface: Bitmap::new(SURFACE_W, SURFACE_H),
            frozen: Bitmap::new(SURFACE_W, SURFACE_H),
            pal32: std::array::from_fn(|i| 0xFF00_0000 | (i as u32 * 0x010101)),
        }
    }

    fn font() -> Font {
        let tga = assets::sprite::Tga::load(&data("TC/openliero/sprites/font.tga")).unwrap();
        Font::load(&tga)
    }

    fn gate() -> RefusalGate {
        RefusalGate {
            attached: false,
            skip_selection: false,
            touch_only: false,
            n_weapons: 40,
        }
    }

    /// One `Process` with `keys` down this frame (`typed` = the key's symbol).
    fn process_keys(
        sel: &mut FileSelector,
        w: &mut MenuWorld,
        store: &dyn ConfigStore,
        keys: &[(u32, u32)],
    ) -> (bool, Vec<i32>) {
        w.keys.begin_frame();
        for &(dos, sym) in keys {
            w.keys.key_down(dos, TypedKey::Sym(sym));
        }
        let font = font();
        let mut sounds = Vec::new();
        let keep = sel.process(&mut MenuCtx {
            w,
            store,
            font: &font,
            running: false,
            sounds: &mut sounds,
            pushes: Vec::new(),
            now_ms: 0,
            gate: gate(),
        });
        (keep, sounds)
    }

    fn to_levels(sel: &mut FileSelector, store: &dyn ConfigStore) {
        assert!(sel.select(store, "./user/TC/openliero/Levels"));
        assert_eq!(cur_path(sel), "./user/TC/openliero/Levels");
    }

    #[test]
    fn a_native_store_tree_merges_both_layers_lazily() {
        // The same tree over real directories (fact 5): the user layer over the system one, a
        // dotfile, a `.zip` as a folder named without it, an extension-less file filtered, the
        // root's `full_path` the user root's `FullPath()`.
        use scenario::storage::NativeStore;
        let dir =
            std::env::temp_dir().join(format!("liero_rs_files_native_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (user, sys) = (dir.join("user"), dir.join("sys"));
        for d in [
            user.join("TC/openliero/Levels"),
            sys.join("TC/openliero/Levels"),
            sys.join("Setups"),
        ] {
            std::fs::create_dir_all(d).unwrap();
        }
        let lv = |root: &Path, name: &str| {
            std::fs::write(
                root.join("TC/openliero/Levels").join(name),
                level("water_stage"),
            )
            .unwrap()
        };
        lv(&sys, "water_stage.lev");
        lv(&user, "water_stage.lev");
        lv(&user, ".dot.lev");
        lv(&sys, "b.LEV");
        std::fs::write(user.join("TC/openliero/Levels/noext"), b"x").unwrap();
        std::fs::write(sys.join("pack.zip"), b"PK").unwrap();
        let store = NativeStore::split(user.clone(), Some(sys.clone()));
        let mut sel = FileSelector::new(&store, lev_filter);
        assert_eq!(cur_path(&sel), store.root_label());
        assert_eq!(names(&mut sel, &store), ["pack", "Setups", "TC"]);
        let root = store.root_label().to_string();
        assert!(sel.select(&store, &join_path(&root, "TC/openliero/Levels/b.LEV")));
        assert_eq!(names(&mut sel, &store), [".dot", "b", "water_stage"]);
        assert_eq!(cursor(&mut sel, &store), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_tree_lists_p1s_rows_with_the_cpp_filter_sort_and_colours() {
        // T0 P1 (plan facts 3-6): folders first then CiLess; dotfiles listed; `notes.txt`
        // filtered; `alpha.LEV` shows as `alpha`; folders colour 47, files 48; the merged
        // layers list `TC` once and the user-only `Replays` too.
        let store = p1_store();
        let mut sel = FileSelector::new(&store, lev_filter);
        assert_eq!(cur_path(&sel), "./user");
        let folder = |n: &str| (n.to_string(), 47u8);
        assert_eq!(
            rows(&mut sel, &store),
            ["Profiles", "Replays", "Resources", "Setups", "TC"].map(folder)
        );
        assert_eq!(sel.enter(&store), None);
        assert_eq!(cur_path(&sel), "./user/Profiles");
        assert!(rows(&mut sel, &store).is_empty(), "an empty folder");
        assert!(sel.exit());
        for _ in 0..4 {
            sel.current_menu(&store).movement(1);
        }
        sel.enter(&store);
        assert_eq!(rows(&mut sel, &store), [folder("openliero")]);
        sel.enter(&store);
        assert_eq!(
            rows(&mut sel, &store),
            [
                "Levels", "nobjects", "sobjects", "sounds", "sprites", "weapons"
            ]
            .map(folder),
            "tc.cfg is not a LEV"
        );
        sel.enter(&store);
        assert_eq!(cur_path(&sel), "./user/TC/openliero/Levels");
        let file = |n: &str| (n.to_string(), 48u8);
        assert_eq!(
            rows(&mut sel, &store),
            [
                ".hidden",
                "alpha",
                "modern_test",
                "physics_fall_test",
                "render_stage",
                "see_shadow_test",
                "tiny",
                "water_stage",
                "Zeta"
            ]
            .map(file)
        );
        let alpha = sel.node(sel.node(sel.current()).children[1]).clone();
        assert_eq!(
            (alpha.rel.as_str(), alpha.full_path.as_str()),
            (
                "TC/openliero/Levels/alpha.LEV",
                "./user/TC/openliero/Levels/alpha.LEV"
            )
        );
        let mut cfg = FileSelector::new(&store, cfg_filter);
        cfg.select(&store, "./user/TC/openliero");
        assert_eq!(
            rows(&mut cfg, &store).last(),
            Some(&file("tc")),
            "the CFG filter: `tc.cfg` after the folders"
        );
    }

    #[test]
    fn select_restores_a_nested_file_a_folder_and_misses_the_rest() {
        // Plan fact 8 / T0 P2: every ancestor's cursor moves onto the path; the current folder
        // is the file's parent.
        let store = p1_store();
        let mut sel = FileSelector::new(&store, lev_filter);
        assert!(sel.select(&store, "./user/TC/openliero/Levels/water_stage.lev"));
        assert_eq!(cur_path(&sel), "./user/TC/openliero/Levels");
        assert_eq!(cursor(&mut sel, &store), 7, "water_stage");
        let levels = sel.current();
        let openliero = sel.node(levels).parent.unwrap();
        assert_eq!(sel.get_menu(&store, openliero).selection(), 0, "Levels");
        assert_eq!(
            sel.get_menu(&store, FileSelector::ROOT).selection(),
            4,
            "TC"
        );
        let hit = sel.node(levels).children[7];
        assert_eq!(
            sel.node(openliero).selected_child,
            Some(hit),
            "C++ sets the hit"
        );
        // A folder: the folder itself becomes current.
        let mut sel = FileSelector::new(&store, cfg_filter);
        assert!(sel.select(&store, "./user/Setups"));
        assert_eq!(cur_path(&sel), "./user/Setups");
        assert_eq!(sel.get_menu(&store, FileSelector::ROOT).selection(), 3);
        // Misses leave the root current on its first entry.
        for miss in [
            "",
            "Levels/water_stage.lev",
            "/openliero/TC/openliero/Levels/water_stage.lev",
            "./user/TC/openliero/Levels/gone.lev",
        ] {
            let mut sel = FileSelector::new(&store, lev_filter);
            assert!(!sel.select(&store, miss), "{miss:?}");
            assert_eq!(
                (sel.current(), cursor(&mut sel, &store)),
                (FileSelector::ROOT, 0)
            );
        }
        // CiCompare: a case-changed path is a hit.
        let mut sel = FileSelector::new(&store, lev_filter);
        assert!(sel.select(&store, "./USER/tc/OPENLIERO/levels/WATER_STAGE.LEV"));
        assert_eq!(cursor(&mut sel, &store), 7);
        // The root itself: a hit that changes nothing.
        let mut sel = FileSelector::new(&store, lev_filter);
        assert!(sel.select(&store, "./user"));
        assert_eq!(sel.current(), FileSelector::ROOT);
    }

    #[test]
    fn find_fills_only_the_folders_that_prefix_the_path() {
        let store = Counting {
            inner: p1_store(),
            listed: Mutex::default(),
        };
        let mut sel = FileSelector::new(&store, lev_filter);
        sel.select(&store, "./user/TC/openliero/Levels/water_stage.lev");
        assert_eq!(
            *store.listed.lock().unwrap(),
            ["", "TC", "TC/openliero", "TC/openliero/Levels"]
        );
        // CiStartsWith with no separator check (fact 8): `./user/TC` prefixes `./user/TCx`.
        let store = Counting {
            inner: p1_store(),
            listed: Mutex::default(),
        };
        let mut sel = FileSelector::new(&store, lev_filter);
        assert!(!sel.select(&store, "./user/TCx/y.lev"));
        assert_eq!(*store.listed.lock().unwrap(), ["", "TC"]);
        // Building a menu fills only that folder.
        store.listed.lock().unwrap().clear();
        sel.set_folder(FileSelector::ROOT);
        sel.enter(&store);
        sel.current_menu(&store);
        assert_eq!(*store.listed.lock().unwrap(), ["Profiles"]);
    }

    #[test]
    fn an_empty_folder_takes_every_key_without_panicking() {
        let store = p1_store();
        let mut w = world();
        let mut sel = FileSelector::new(&store, lev_filter);
        sel.enter(&store); // Profiles
        assert_eq!(cur_path(&sel), "./user/Profiles");
        for k in [DK_UP, DK_DOWN, DK_PGUP, DK_PGDN, DK_RIGHT] {
            let (keep, _) = process_keys(&mut sel, &mut w, &store, &[(k, 0)]);
            assert!(keep);
        }
        process_keys(&mut sel, &mut w, &store, &[(0, u32::from(b'x'))]);
        assert_eq!(sel.cur_sel(&store), None);
        assert_eq!(sel.enter(&store), None);
        assert_eq!(cur_path(&sel), "./user/Profiles");
    }

    #[test]
    fn process_runs_the_keys_in_cpp_order() {
        // fileSelector.hpp:251-300 (fact 9).
        let store = p1_store();
        let mut w = world();
        let hooks = w.tc.hooks;
        let mut sel = FileSelector::new(&store, lev_filter);
        assert!(!sel.exit(), "Exit at the root does nothing");
        let (keep, s) = process_keys(&mut sel, &mut w, &store, &[(DK_LEFT, 0)]);
        assert!(keep && s.is_empty() && sel.current() == FileSelector::ROOT);
        let (_, s) = process_keys(&mut sel, &mut w, &store, &[(DK_DOWN, 0)]);
        assert_eq!(s, [hooks.move_up], "Down plays MenuMoveUp");
        let (_, s) = process_keys(&mut sel, &mut w, &store, &[(DK_UP, 0)]);
        assert_eq!(s, [hooks.move_down], "Up plays MenuMoveDown");
        let (_, s) = process_keys(&mut sel, &mut w, &store, &[(DK_PGDN, 0), (DK_PGUP, 0)]);
        assert_eq!(s, [hooks.move_down, hooks.move_up], "PgUp is tested first");
        // Esc wins over a Left in the same frame: Process returns before testing it.
        to_levels(&mut sel, &store);
        let (keep, s) = process_keys(&mut sel, &mut w, &store, &[(DK_LEFT, 0), (DK_ESCAPE, 0)]);
        assert!(!keep && s.is_empty());
        assert_eq!(cur_path(&sel), "./user/TC/openliero/Levels");
        assert!(w.keys.test(DK_LEFT), "the Left was never tested");
        // A P1 Jump leaves too.
        let jump = w.settings.worm_settings[0].controls_ex[K_JUMP];
        let (keep, _) = process_keys(&mut sel, &mut w, &store, &[(jump, 0)]);
        assert!(!keep);
        // Right on a file row does nothing.
        let (keep, s) = process_keys(&mut sel, &mut w, &store, &[(DK_RIGHT, 0)]);
        assert!(keep && s.is_empty());
        assert_eq!(cur_path(&sel), "./user/TC/openliero/Levels");
        // Left and Right in one frame inside a child: Exit, then Enter back in under the
        // parent's cursor.
        let (_, s) = process_keys(&mut sel, &mut w, &store, &[(DK_RIGHT, 0), (DK_LEFT, 0)]);
        assert!(s.is_empty());
        assert_eq!(cur_path(&sel), "./user/TC/openliero/Levels");
        process_keys(&mut sel, &mut w, &store, &[(DK_LEFT, 0)]);
        assert_eq!(cur_path(&sel), "./user/TC/openliero");
    }

    #[test]
    fn the_search_is_contains_and_runs_after_the_controls() {
        // T0 P1: from `alpha`, `h` stays (it contains `h`), `hi` moves to `.hidden`; P1's `d`
        // is a Left before the search.
        let store = p1_store();
        let mut w = world();
        let mut sel = FileSelector::new(&store, lev_filter);
        to_levels(&mut sel, &store);
        sel.current_menu(&store).move_to(1);
        process_keys(&mut sel, &mut w, &store, &[(35, u32::from(b'h'))]);
        assert_eq!(cursor(&mut sel, &store), 1, "alpha contains h");
        process_keys(&mut sel, &mut w, &store, &[(23, u32::from(b'i'))]);
        assert_eq!(cursor(&mut sel, &store), 0, ".hidden");
        let d = w.settings.worm_settings[0].controls_ex[K_LEFT];
        process_keys(&mut sel, &mut w, &store, &[(d, u32::from(b'd'))]);
        assert_eq!(cur_path(&sel), "./user/TC/openliero", "d is P1's Left");
        assert_eq!(
            names(&mut sel, &store)[cursor(&mut sel, &store) as usize],
            "sounds"
        );
    }

    const WATER_PATH: &str = "./user/TC/openliero/Levels/water_stage.lev";

    /// A picked-level world: `level_file` = water_stage (the restore).
    fn water_world() -> MenuWorld {
        let mut w = world();
        w.settings.random_level = false;
        w.settings.level_file = WATER_PATH.into();
        w
    }

    fn rect(b: &Bitmap, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<u32> {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| b.get_pixel(x, y)))
            .collect()
    }

    fn preview_rect(b: &Bitmap) -> Vec<u32> {
        rect(b, PREVIEW_X, PREVIEW_Y, PREVIEW_X + 52, PREVIEW_Y + 36)
    }

    fn ids(bytes: &[u8]) -> assets::level::LevelData {
        assets::level::load(bytes).unwrap()
    }

    #[test]
    fn the_level_selector_restores_the_picked_file_and_opens_on_random_otherwise() {
        // T0 P2: inside Levels on the file, the parent pane on `openliero` at Levels.
        let store = p1_store();
        let mut w = water_world();
        let mut ls = LevelSelectorState::new();
        ls.enter(&mut w, &store);
        assert_eq!(
            ls.view(),
            SelectorView {
                top: 'L',
                folder: "./user/TC/openliero/Levels".into(),
                selection: 7
            }
        );
        let parent = ls.selector.node(ls.selector.current()).parent.unwrap();
        assert_eq!(ls.selector.node(parent).full_path, "./user/TC/openliero");
        assert_eq!(ls.selector.node(parent).menu().selection(), 0);
        // Random, a TC-relative path, another root: the root, on `[RANDOM]` (colour 48).
        for (random, file) in [
            (true, ""),
            (false, "Levels/water_stage.lev"),
            (false, "/openliero/TC/openliero/Levels/water_stage.lev"),
        ] {
            let mut w = world();
            w.settings.random_level = random;
            w.settings.level_file = file.into();
            let mut ls = LevelSelectorState::new();
            ls.enter(&mut w, &store);
            assert_eq!(ls.view().folder, "./user");
            assert_eq!(
                rows(&mut ls.selector, &store)[..2],
                [("[RANDOM]".into(), 48), ("Profiles".into(), 47)]
            );
            assert_eq!(ls.view().selection, 0);
            let r = ls.random_node();
            assert_eq!(ls.selector.node(r).full_path, "");
            assert_eq!(ls.selector.cur_sel(&store), Some(r));
        }
    }

    #[test]
    fn the_preview_lands_in_the_frozen_screen_one_frame_late() {
        // fileSelectorState.cpp:52-67, :105-145 (fact 10; T0 P3): the surface is copied from
        // the frozen screen BEFORE the preview is drawn into it.
        let store = p1_store();
        let mut w = water_world();
        let font = font();
        let mut ls = LevelSelectorState::new();
        ls.enter(&mut w, &store);
        let pal = w.pal32;
        w.frozen.fill_rect(PREVIEW_X, PREVIEW_Y, 52, 36, 5, &pal);
        let before = w.frozen.clone();
        ls.draw(&mut w, &store, &font);
        assert_eq!(
            preview_rect(&w.surface),
            preview_rect(&before),
            "not this frame"
        );
        let water = ids(&level("water_stage"));
        let mut want = before.clone();
        want.fill_rect(PREVIEW_X, PREVIEW_Y, 52, 36, 0, &pal);
        draw_miniature_ids(
            &mut want,
            &pal,
            &water.material_id,
            504,
            350,
            134,
            162,
            10,
            10,
        );
        assert_eq!(
            w.frozen, want,
            "the first clear is the full 52×36, then the 50×35 map"
        );
        assert!(
            rect(&w.frozen, 184, 162, 186, 198)
                .iter()
                .all(|&p| p == pal[0])
        );
        assert!(
            rect(&w.frozen, 134, 197, 186, 198)
                .iter()
                .all(|&p| p == pal[0])
        );
        let water_node = ls.selector.cur_sel(&store);
        assert_eq!(ls.preview_node(), water_node);
        assert_eq!(ls.prev, (50, 35));
        ls.draw(&mut w, &store, &font);
        assert_eq!(
            preview_rect(&w.surface),
            preview_rect(&want),
            "the next frame shows it"
        );
        assert_eq!(w.frozen, want, "the same node draws nothing again");
        // Onto `tiny` (60×40 at step 2×2 → 30×20): the last 50×35 is cleared first.
        ls.selector.current_menu(&store).move_to(6);
        ls.draw(&mut w, &store, &font);
        let tiny = ids(&tiny());
        let mut want2 = want.clone();
        want2.fill_rect(PREVIEW_X, PREVIEW_Y, 50, 35, 0, &pal);
        draw_miniature_ids(&mut want2, &pal, &tiny.material_id, 60, 40, 134, 162, 2, 2);
        assert_eq!(w.frozen, want2);
        assert!(
            rect(&w.frozen, 164, 162, 184, 197)
                .iter()
                .all(|&p| p == pal[0])
        );
        assert!(
            rect(&w.frozen, 134, 182, 184, 197)
                .iter()
                .all(|&p| p == pal[0])
        );
        assert_ne!(
            rect(&w.frozen, 134, 162, 164, 182),
            rect(&want, 134, 162, 164, 182)
        );
        assert_eq!(ls.prev, (30, 20));
        // The palette is this frame's (the frozen pixels are ARGB).
        let mut w2 = water_world();
        w2.pal32[162] = 0xFF12_3456;
        let mut ls2 = LevelSelectorState::new();
        ls2.enter(&mut w2, &store);
        ls2.selector.current_menu(&store).move_to(6);
        ls2.draw(&mut w2, &store, &font);
        assert_eq!(
            w2.frozen.get_pixel(134, 162),
            0xFF12_3456,
            "the cell at (1, 1) of `tiny` is material 162, through this frame's pal32"
        );
    }

    #[test]
    fn random_folders_and_rejected_files_leave_the_frozen_screen_alone() {
        let store = p1_store();
        store
            .write(&format!("{LEVELS}/broken.lev"), &data("README.md"))
            .unwrap();
        store
            .write(&format!("{LEVELS}/trunc.lev"), &trunc())
            .unwrap();
        let mut w = world();
        let font = font();
        let mut ls = LevelSelectorState::new();
        ls.enter(&mut w, &store);
        let frozen = w.frozen.clone();
        ls.draw(&mut w, &store, &font);
        assert_eq!(
            (w.frozen == frozen, ls.preview_node()),
            (true, None),
            "[RANDOM]"
        );
        ls.selector.current_menu(&store).move_to(1);
        ls.draw(&mut w, &store, &font);
        assert_eq!(
            (w.frozen == frozen, ls.preview_node()),
            (true, None),
            "a folder"
        );
        ls.selector
            .select(&store, "./user/TC/openliero/Levels/broken.lev");
        let broken = ls.selector.cur_sel(&store);
        ls.draw(&mut w, &store, &font);
        assert!(w.frozen == frozen, "broken: no preview");
        assert_eq!(
            ls.preview_node(),
            broken,
            "previewNode_ moves on a failed load"
        );
        ls.selector
            .select(&store, "./user/TC/openliero/Levels/trunc.lev");
        ls.draw(&mut w, &store, &font);
        assert!(w.frozen == frozen, "trunc (clause (e)): no preview");
        assert_eq!(ls.prev, (52, 36), "no load, no footprint");
    }

    #[test]
    fn the_level_selector_draws_the_frozen_screen_its_title_and_both_panes() {
        // Fact 2's order; fact 1's title (T0: clipped at x = 320).
        let store = p1_store();
        let mut w = water_world();
        let font = font();
        let mut ls = LevelSelectorState::new();
        ls.enter(&mut w, &store);
        ls.preview = ls.selector.cur_sel(&store);
        let frozen = w.frozen.clone();
        ls.draw(&mut w, &store, &font);
        let t = "Select level: ./user/TC/openliero/Levels";
        let mut want = frozen.clone();
        font.draw_framed_text(&mut want, &w.pal32, t, 178, 20, 50);
        font.draw_framed_text(&mut want, &w.pal32, "Parent directory", 28, 20, 50);
        let mut sel = ls.selector.clone();
        let parent = sel.node(sel.current()).parent.unwrap();
        sel.get_menu(&store, parent)
            .draw(&PlainModel, &mut want, &w.pal32, &font, true, 28, true);
        sel.current_menu(&store)
            .draw(&PlainModel, &mut want, &w.pal32, &font, false, 178, false);
        assert_eq!(w.surface, want);
        assert!(
            178 + font.get_dims(t) > 320,
            "the title runs past the surface"
        );
        assert_eq!(title(&w.tc.sel_level, "./user"), "Select level: ./user");
        assert_eq!(title(&w.tc.sel_level, ""), "Select level:");
        // At the root: no parent pane.
        let mut w = world();
        let mut ls = LevelSelectorState::new();
        ls.enter(&mut w, &store);
        let frozen = w.frozen.clone();
        ls.draw(&mut w, &store, &font);
        let mut want = frozen.clone();
        font.draw_framed_text(&mut want, &w.pal32, "Select level: ./user", 178, 20, 50);
        ls.selector.current_menu(&store).draw(
            &PlainModel,
            &mut want,
            &w.pal32,
            &font,
            false,
            178,
            false,
        );
        assert_eq!(w.surface, want);
    }

    fn update_keys(
        ls: &mut LevelSelectorState,
        w: &mut MenuWorld,
        store: &dyn ConfigStore,
        dos: u32,
    ) -> (SelectorOut, Vec<i32>) {
        w.keys.begin_frame();
        w.keys.key_down(dos, TypedKey::Sym(0));
        let font = font();
        let mut sounds = Vec::new();
        let o = ls.update(&mut MenuCtx {
            w,
            store,
            font: &font,
            running: false,
            sounds: &mut sounds,
            pushes: Vec::new(),
            now_ms: 0,
            gate: gate(),
        });
        (o, sounds)
    }

    #[test]
    fn return_plays_select_then_enters_a_folder_or_picks_a_file() {
        // fileSelectorState.cpp:36-47, :95-103.
        let store = p1_store();
        let mut w = world();
        let select = w.tc.hooks.select;
        let mut ls = LevelSelectorState::new();
        ls.enter(&mut w, &store);
        let (o, s) = update_keys(&mut ls, &mut w, &store, DK_RETURN);
        assert_eq!(
            (o.keep, o.picked, s),
            (false, Some(Picked::Random), vec![select])
        );
        ls.selector.current_menu(&store).move_to(5);
        let (o, s) = update_keys(&mut ls, &mut w, &store, DK_KP_ENTER);
        assert_eq!((o.keep, o.picked, s), (true, None, vec![select]), "into TC");
        assert_eq!(ls.view().folder, "./user/TC");
        ls.selector.select(&store, WATER_PATH);
        let fire = w.settings.worm_settings[0].controls_ex[K_FIRE];
        let (o, s) = update_keys(&mut ls, &mut w, &store, fire);
        assert_eq!(
            (o.keep, o.picked, s),
            (false, Some(Picked::Level(WATER_PATH.into())), vec![select])
        );
        let (o, s) = update_keys(&mut ls, &mut w, &store, DK_ESCAPE);
        assert_eq!(
            (o.keep, o.picked, s),
            (false, None, vec![]),
            "Esc: no sound"
        );
    }

    #[test]
    fn the_setup_selector_opens_inside_setups() {
        // T0 P4: `./user/Setups` on `liero`; Left shows the root on Setups.
        let store = install();
        store
            .write("Setups/mine.cfg", &data("Setups/orbmit.cfg"))
            .unwrap();
        let mut w = world();
        let font = font();
        let mut ps = SetupSelectorState::new();
        ps.enter(&mut w, &store);
        let frozen = w.frozen.clone();
        ps.draw(&mut w, &store, &font);
        assert_eq!(
            ps.view(),
            SelectorView {
                top: 'P',
                folder: "./user/Setups".into(),
                selection: 0
            }
        );
        assert_eq!(names(&mut ps.selector, &store), ["liero", "mine", "orbmit"]);
        let mut want = frozen.clone();
        font.draw_framed_text(
            &mut want,
            &w.pal32,
            "Select options: ./user/Setups",
            178,
            20,
            50,
        );
        let mut sel = ps.selector.clone();
        font.draw_framed_text(&mut want, &w.pal32, "Parent directory", 28, 20, 50);
        sel.get_menu(&store, FileSelector::ROOT).draw(
            &PlainModel,
            &mut want,
            &w.pal32,
            &font,
            true,
            28,
            true,
        );
        sel.current_menu(&store)
            .draw(&PlainModel, &mut want, &w.pal32, &font, false, 178, false);
        assert_eq!(w.surface, want);
        assert!(ps.selector.exit());
        assert_eq!(
            names(&mut ps.selector, &store),
            ["Profiles", "Resources", "Setups", "TC"]
        );
        assert_eq!(ps.view().selection, 2);
        ps.selector.enter(&store);
        ps.selector.current_menu(&store).move_to(2);
        let mut w2 = world();
        w2.keys.begin_frame();
        w2.keys.key_down(DK_RETURN, TypedKey::Sym(0));
        let mut sounds = Vec::new();
        let o = ps.update(&mut MenuCtx {
            w: &mut w2,
            store: &store,
            font: &font,
            running: false,
            sounds: &mut sounds,
            pushes: Vec::new(),
            now_ms: 0,
            gate: gate(),
        });
        assert_eq!(
            o.picked,
            Some(Picked::Setup {
                rel: "Setups/orbmit.cfg".into(),
                name: "orbmit".into()
            })
        );
    }
}
