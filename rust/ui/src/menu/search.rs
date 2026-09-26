//! Type-to-search state (`Menu::search_prefix` / `search_time`, `menu.hpp:175-176`). The search
//! itself (`Menu::OnKeys`, `menu.cpp:14-79`) is T3's `Menu::on_keys`.

/// `search_prefix` + `search_time` (milliseconds from the caller's clock; design §4.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    pub prefix: String,
    pub time_ms: u64,
}
