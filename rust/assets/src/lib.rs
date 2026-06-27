//! On-disk data-format loaders for Liero-rs (no Bevy). Behaviour that feeds the
//! simulation is differential-tested against the C++ engine; the implementation
//! is idiomatic Rust, not a port of the C++ `io` layer.
pub mod level;
pub mod palette;
pub mod sprite;
pub mod tc;
pub mod object;
