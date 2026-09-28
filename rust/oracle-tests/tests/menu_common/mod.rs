//! Step 4½d G1 — the menu-widget scripts replayed through `ui::menu` (design §6.1): the Rust half
//! of `oracle_dump_menu` (grammar and line format: plan Task 4). The clock for type-to-search is
//! synthetic: it starts at 0 and advances by each `sleep_ms` (C++ uses SDL_GetTicks with the same
//! sleeps; the allowed sleep values keep the 1500 ms test on the same side).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use render::bitmap::{Bitmap, Pal32};
use render::font::Font;
use render::hash::hash_frame;
use render::palette::pack_pal32;
use scenario::settings::Settings;
use scenario::settings_toml::settings_from_toml;
use ui::keys::TypedKey;
use ui::menu::behavior::Integer;
use ui::menu::{Behavior, Enter, Menu, MenuCx, MenuItem, MenuModel};
use ui::shell::settings_menu::{settings_menu, SettingsModel};
use ui::text::{UiTc, GAME_MODES, ONOFF};

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

/// The 15 G1 scripts (`golden/menu_<s>_script.txt`).
pub const SCRIPTS: [&str; 15] = [
    "nav_wrap",
    "page_scroll",
    "visibility",
    "scrollbar_positions",
    "integer",
    "time",
    "bool_enum",
    "values_draw",
    "search",
    "settings_killemall",
    "settings_gametag",
    "settings_holdazone",
    "settings_scales",
    "settings_file_level",
    "settings_interact",
];

#[derive(Clone, Debug, Default)]
pub struct Bind {
    kind: String,
    i: i32,
    u: u32,
    b: bool,
    min: i32,
    max: i32,
    step: i32,
    interval: i32,
    div: i32,
    pct: bool,
    entry: bool,
    frames: bool,
    broken: bool,
    arr: String,
}

impl Bind {
    fn value(&self) -> String {
        match self.kind.as_str() {
            "bool" => if self.b { "1" } else { "0" }.to_string(),
            "enum" | "array" => self.u.to_string(),
            _ => self.i.to_string(),
        }
    }
}

/// A widget menu's model: the dumper's `ScriptMenu::GetItemBehavior`.
pub struct Binds(pub BTreeMap<i32, Bind>);

impl MenuModel for Binds {
    fn behavior(&mut self, id: i32) -> Behavior<'_> {
        let Some(b) = self.0.get_mut(&id) else {
            return Behavior::Plain;
        };
        match b.kind.as_str() {
            "integer" => {
                let mut r = Integer::new(&mut b.i, b.min, b.max, b.step, b.pct);
                r.scroll_interval = b.interval;
                r.display_div = b.div;
                r.allow_entry = b.entry;
                Behavior::Integer(r)
            }
            "time" => Behavior::time(&mut b.i, b.min, b.max, b.step, b.frames),
            "bool" => Behavior::Bool(&mut b.b),
            "enum" => Behavior::Enum {
                v: &mut b.u,
                min: b.min as u32,
                max: b.max as u32,
                broken: b.broken,
            },
            _ => Behavior::ArrayEnum {
                v: &mut b.u,
                arr: if b.arr == "gamemodes" {
                    &GAME_MODES
                } else {
                    &ONOFF
                },
                broken: b.broken,
            },
        }
    }
}

pub fn font() -> Font {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/font.tga")).unwrap();
    Font::load(&assets::sprite::Tga::load(&bytes).unwrap())
}

/// `Renderer::LoadPalette` (`renderer.cpp:14-19`): the TC palette (small.tga's, as the loader).
pub fn tc_pal() -> Pal32 {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).unwrap();
    pack_pal32(&assets::sprite::Tga::load(&bytes).unwrap().palette)
}

/// The rest of `line` after `n` whitespace-separated tokens (an item's label).
fn rest_after(line: &str, n: usize) -> String {
    let mut s = line.trim_start();
    for _ in 0..n {
        s = s.trim_start();
        s = &s[s.find(char::is_whitespace).unwrap_or(s.len())..];
    }
    let label = s.trim();
    if label == "\"\"" {
        String::new()
    } else {
        label.to_string()
    }
}

fn typed(tok: &str) -> TypedKey {
    match tok {
        "SPACE" => TypedKey::Sym(32),
        "TAB" => TypedKey::Tab,
        "MINUS" => TypedKey::Sym(b'-' as u32),
        "UP" => TypedKey::Sym(0x4000_0052),
        t if t.len() == 1 => TypedKey::Sym(t.as_bytes()[0] as u32),
        t => panic!("unknown key token {t}"),
    }
}

fn token(s: &str) -> String {
    if s.is_empty() {
        "-".into()
    } else {
        s.replace(' ', "_")
    }
}

fn line(
    n: usize,
    op: &str,
    m: &Menu,
    bound: &str,
    sounds: &mut Vec<i32>,
    ret: &str,
    push: usize,
    hash: &str,
) -> String {
    let shown: String = m
        .items
        .iter()
        .map(|i| if i.visible { '1' } else { '0' })
        .collect();
    let vals: Vec<String> = m
        .items
        .iter()
        .map(|i| {
            if i.has_value {
                token(&i.value)
            } else {
                ".".into()
            }
        })
        .collect();
    let snd: Vec<String> = sounds.drain(..).map(|s| s.to_string()).collect();
    format!(
        "{n} {op} {} {} {} {} {} {} {} {} {} {ret} {push} {hash}",
        m.selection(),
        m.top_item,
        m.bottom_item,
        m.visible_item_count,
        if shown.is_empty() { "-".into() } else { shown },
        token(&m.search.prefix),
        if vals.is_empty() {
            "-".into()
        } else {
            vals.join("|")
        },
        if bound.is_empty() { "-" } else { bound },
        if snd.is_empty() {
            "-".into()
        } else {
            snd.join(",")
        },
    )
}

/// One op on `m` with `model`: (ret, push, hash, drawn bitmap).
#[allow(clippy::too_many_arguments)]
fn op<M: MenuModel>(
    m: &mut Menu,
    model: &mut M,
    name: &str,
    args: &[&str],
    cycles: &mut u32,
    now: &mut u64,
    tc: &UiTc,
    sounds: &mut Vec<i32>,
    font: &Font,
    pal: &Pal32,
) -> (String, usize, String, Option<Bitmap>) {
    let int = |k: usize| args[k].parse::<i32>().unwrap();
    let mut cx = MenuCx {
        menu_cycles: *cycles,
        hooks: tc.hooks,
        sounds,
    };
    let (mut ret, mut push, mut hash, mut bmp) = ("-".to_string(), 0, "-".to_string(), None);
    match name {
        "move" => m.movement(int(0)),
        "page" => m.movement_page(int(0)),
        "scroll" => m.scroll(int(0)),
        "set_height" => m.set_height(int(0)),
        "visible" => m.set_visibility(int(0), int(1) != 0),
        "move_to" => m.move_to(int(0)),
        "move_to_id" => m.move_to_id(int(0)),
        "first_visible" => m.move_to_first_visible(),
        "cycles" => *cycles = args[0].parse().unwrap(),
        "left" | "right" => {
            let dir = if name == "left" { -1 } else { 1 };
            ret = if m.on_left_right(model, dir, &mut cx) {
                "1"
            } else {
                "0"
            }
            .into();
        }
        "enter" => match m.on_enter(model, &mut cx) {
            Enter::Result(r) => ret = r.to_string(),
            Enter::EditValue(_) => (ret, push) = ("-1".into(), 1),
        },
        "keys" | "keys_contains" => {
            let keys: Vec<TypedKey> = args.iter().map(|t| typed(t)).collect();
            m.on_keys(&keys, *now, name == "keys_contains");
        }
        "sleep_ms" => *now += args[0].parse::<u64>().unwrap(),
        "update_items" => m.update_items(model),
        "draw" => {
            let mut b = Bitmap::new(320, 200);
            b.fill(0, pal);
            m.draw(model, &mut b, pal, font, int(0) != 0, int(1), int(2) != 0);
            hash = format!("{:016x}", hash_frame(&b, 33));
            bmp = Some(b);
        }
        other => panic!("bad op {other}"),
    }
    (ret, push, hash, bmp)
}

/// Replay `golden/menu_<name>_script.txt`: the expected-format lines + every drawn bitmap.
pub fn replay(name: &str) -> (Vec<String>, Vec<(usize, Bitmap)>) {
    let text = std::fs::read_to_string(format!("{GOLDEN}/menu_{name}_script.txt")).unwrap();
    let tc = UiTc::load(Path::new(TC_ROOT));
    let (font, pal) = (font(), tc_pal());
    let (mut menu, mut binds, mut settings) =
        (None::<Menu>, Binds(BTreeMap::new()), None::<Settings>);
    let (mut cycles, mut now, mut sounds) = (0u32, 0u64, Vec::new());
    let (mut lines, mut shots) = (Vec::new(), Vec::new());
    for raw in text.lines() {
        let toks: Vec<&str> = raw.split_whitespace().collect();
        let Some(&name_) = toks.first() else { continue };
        if name_.starts_with('#') {
            continue;
        }
        let args = &toks[1..];
        match name_ {
            "menu" => {
                let v: Vec<i32> = args.iter().map(|a| a.parse().unwrap()).collect();
                let mut m = Menu::new(v[0], v[1], v[4] != 0);
                m.height = v[2];
                m.value_offset_x = v[3];
                menu = Some(m);
                continue;
            }
            "settings" => {
                settings = Some(if args[0] == "default" {
                    Settings::default()
                } else {
                    settings_from_toml(
                        &std::fs::read_to_string(format!("{GOLDEN}/{}", args[0])).unwrap(),
                    )
                    .unwrap()
                });
                menu = Some(settings_menu());
                continue;
            }
            "space" => {
                menu.as_mut().unwrap().add_item(MenuItem::space());
                continue;
            }
            "item" => {
                let v: Vec<i32> = args[..5].iter().map(|a| a.parse().unwrap()).collect();
                let mut it = MenuItem::new(v[1] as u8, v[2] as u8, &rest_after(raw, 6), v[0]);
                it.visible = v[3] != 0;
                it.selectable = v[4] != 0;
                menu.as_mut().unwrap().add_item(it);
                continue;
            }
            "bind" => {
                let id: i32 = args[0].parse().unwrap();
                let n = |k: usize| args[k].parse::<i32>().unwrap();
                let mut b = Bind {
                    kind: args[1].to_string(),
                    step: 1,
                    interval: 5,
                    div: 1,
                    entry: true,
                    ..Bind::default()
                };
                match args[1] {
                    "integer" => {
                        (b.i, b.min, b.max, b.step) = (n(2), n(3), n(4), n(5));
                        (b.pct, b.interval, b.div, b.entry) = (n(6) != 0, n(7), n(8), n(9) != 0);
                    }
                    "time" => {
                        (b.i, b.min, b.max, b.step, b.frames) = (n(2), n(3), n(4), n(5), n(6) != 0)
                    }
                    "bool" => b.b = n(2) != 0,
                    "enum" => (b.u, b.min, b.max, b.broken) = (n(2) as u32, n(3), n(4), n(5) != 0),
                    "array" => {
                        (b.u, b.arr, b.broken) = (n(2) as u32, args[3].to_string(), n(4) != 0)
                    }
                    k => panic!("bad bind kind {k}"),
                }
                assert!(binds.0.insert(id, b).is_none(), "duplicate bind {id}");
                continue;
            }
            _ => {}
        }
        let m = menu.as_mut().expect("menu or settings comes first");
        let n = lines.len();
        let (ret, push, hash, bmp) = match settings.as_mut() {
            Some(s) => {
                let mut model = SettingsModel {
                    settings: s,
                    tc: &tc,
                    setup_name: "liero",
                };
                op(
                    m,
                    &mut model,
                    name_,
                    args,
                    &mut cycles,
                    &mut now,
                    &tc,
                    &mut sounds,
                    &font,
                    &pal,
                )
            }
            None => op(
                m,
                &mut binds,
                name_,
                args,
                &mut cycles,
                &mut now,
                &tc,
                &mut sounds,
                &font,
                &pal,
            ),
        };
        let bound: Vec<String> = binds.0.values().map(Bind::value).collect();
        lines.push(line(
            n,
            name_,
            m,
            &bound.join(","),
            &mut sounds,
            &ret,
            push,
            &hash,
        ));
        if let Some(b) = bmp {
            shots.push((n, b));
        }
    }
    (lines, shots)
}

/// The data lines of `golden/menu_<name>.txt`.
pub fn golden(name: &str) -> Vec<String> {
    std::fs::read_to_string(format!("{GOLDEN}/menu_{name}.txt"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}
