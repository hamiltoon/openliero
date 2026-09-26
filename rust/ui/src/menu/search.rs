//! Type-to-search state (`Menu::search_prefix` / `search_time`, `menu.hpp:175-176`). The search
//! itself (`Menu::OnKeys`, `menu.cpp:14-79`) is T3's `Menu::on_keys`.

use super::Menu;
use crate::keys::TypedKey;

/// `search_prefix` + `search_time` (milliseconds from the caller's clock; design §4.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    pub prefix: String,
    pub time_ms: u64,
}

/// `std::toupper` in the "C" locale: ASCII only.
fn same(a: u8, b: u8) -> bool {
    a.eq_ignore_ascii_case(&b)
}

/// A case-insensitive prefix match, or with `contains` the `std::ranges::search` of
/// `menu.cpp:44-47`, whose empty needle matches any non-empty string.
fn matches(hay: &str, needle: &str, contains: bool) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if contains {
        if n.is_empty() {
            return !h.is_empty();
        }
        h.windows(n.len())
            .any(|w| w.iter().zip(n).all(|(a, b)| same(*a, *b)))
    } else {
        h[..n.len()].iter().zip(n).all(|(a, b)| same(*a, *b))
    }
}

impl Menu {
    /// `Menu::OnKeys` (`menu.cpp:14-79`; design §3.6). `now_ms` stands for `SDL_GetTicks()`.
    pub fn on_keys(&mut self, keys: &[TypedKey], now_ms: u64, contains: bool) {
        for &key in keys {
            let (sym, is_tab) = match key {
                TypedKey::Tab => (b'\t' as u32, true),
                TypedKey::Sym(s) => (s, false),
            };
            if !((32..=127).contains(&sym) || is_tab) {
                continue;
            }
            if !is_tab && now_ms.wrapping_sub(self.search.time_ms) > 1500 {
                self.search.prefix.clear();
            }
            loop {
                let was_empty = self.search.prefix.is_empty();
                let mut new_prefix = self.search.prefix.clone();
                if !is_tab {
                    new_prefix.push(sym as u8 as char);
                }
                self.search.time_ms = now_ms;
                let n = self.items.len();
                let skip = usize::from(is_tab);
                let mut found = false;
                for offs in skip..n {
                    let i = (self.selection() as usize).wrapping_add(offs) % n;
                    let item = &self.items[i];
                    if item.visible
                        && item.string.len() >= new_prefix.len()
                        && matches(&item.string, &new_prefix, contains)
                    {
                        found = true;
                        self.move_to(i as i32);
                        break;
                    }
                }
                if found {
                    self.search.prefix = new_prefix;
                    break;
                }
                self.search.prefix.clear();
                if was_empty {
                    break;
                }
            }
        }
    }
}
