//! Step 4½d — the Bevy-free menu framework and the C++ frame loop (design
//! `specs/2026-09-25-liero-rs-step4.5-slice4.5d-menu-framework-design.md`, cited **design §N**).
//!
//! `ui::menu` ports C++ `Menu` / `MenuItem` / the item behaviors; `ui::keys` the `dos_keys`
//! table the menus read; `ui::text` the hardcoded C++ texts; `ui::shell` the `StateStack`, the
//! main menu, the router and `Gfx::RunOneFrame`. Bevy-free so `oracle-tests` (the C++ frame
//! gates) and `shot` drive the whole shell headlessly (design §2, amended LD 2).
pub mod keys;
pub mod menu;
pub mod text;
