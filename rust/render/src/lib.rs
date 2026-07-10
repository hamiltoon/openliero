//! CPU bitmap renderer for Liero-rs (no Bevy, no floating point in the hashed
//! path). Ports the C++ `renderer`/`blit`/`viewport`/`palette` math verbatim,
//! idiomatic in API: drawing takes 8-bit palette indices and resolves them to
//! ARGB through a `pal32` LUT at each write (`bitmap.hpp:50-54`); the LUT is
//! finalized once per frame before any blit (`game.cpp:171-183`). The FNV-1a
//! frame hash (`hash`) is the regression primitive differential-tested against
//! C++. Slice 3a scope: `Bitmap`, per-frame palette build, `DrawLevel` Classic
//! arm, the two-viewport 320x200 player layout, and the frame hash. Shadows,
//! sprites, HUD, minimap arrive in 3b/3e.
pub mod bitmap;
pub mod frame;
pub mod hash;
pub mod level_draw;
pub mod palette;
pub mod viewport;
