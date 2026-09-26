//! CPU bitmap renderer for Liero-rs (no Bevy, no floating point in the hashed
//! path). Ports the C++ `renderer`/`blit`/`viewport`/`palette` math verbatim,
//! idiomatic in API: drawing takes 8-bit palette indices and resolves them to
//! ARGB through a `pal32` LUT at each write (`bitmap.hpp:50-54`); the LUT is
//! finalized once per frame before any blit (`game.cpp:171-183`). The FNV-1a
//! frame hash (`hash`) is the regression primitive differential-tested against
//! C++. Slice 3a scope: `Bitmap`, per-frame palette build, `DrawLevel` Classic
//! arm, the two-viewport 320x200 player layout, and the frame hash. Shadows,
//! sprites, HUD, minimap arrive in 3b/3e. Step 4½c adds the menu-item recipe
//! (menu) and the weapon-selection screen (weapsel), pulled forward from 4½d.
//! Step 4½d adds the value arm and the scrollbar (menu) and the composition fade (present).
//! Step 4½e-1 adds the text helpers (`get_dims_h`, `draw_framed_text`, `blit_bitmap`) and the
//! three `DrawTextSmall` labels (small_text), behind `Scene::small_labels`.
pub mod bitmap;
pub mod blit;
pub mod fire_cone;
pub mod font;
pub mod frame;
pub mod hash;
pub mod hud;
pub mod level_draw;
pub mod menu;
pub mod object_draw;
pub mod palette;
pub mod present;
pub mod shadow_query;
pub mod small_text;
pub mod viewport;
pub mod weapsel;
