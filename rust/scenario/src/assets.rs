//! The single asset-read seam for the scenario loader (Step 3, Slice 3f, T1).
//!
//! Every TC read in [`crate::loader::load`] funnels through [`read_asset`] so the
//! filesystem-vs-embed divergence lives in exactly one place — and so `assets`,
//! `sim`, and `render` are never touched (the bit-exactness invariant).
//!
//! The **native** branch is byte-for-byte the reads it replaced —
//! `std::fs::read(tc_root.join(rel))`, in the same order — so every committed
//! render/sim frame golden and `test_determinism` stay identical (the shim is a
//! native no-op). The **wasm** branch is the stub filled by Slice 3f T2 (the
//! curated `include_dir`/`include_bytes!` manifest); the wasm target is not built
//! until T3/CI, so the native crate keeps building with the stub in place.

use std::path::Path;

/// Read the TC asset at `rel` — a forward-slash relative path under `tc_root`
/// (e.g. `"sprites/small.tga"`, `"tc.cfg"`, `"weapons/DART.cfg"`) — returning its
/// raw bytes.
///
/// Native impl: `std::fs::read(tc_root.join(rel))`, panicking on failure with the
/// `read {rel}: {e}` message. This is the harmonised shape of the prior call
/// sites (`load_sprites`'s `read sprites/<file>: <e>`, the level's `read <lev>: <e>`,
/// and the `.expect("read …")` sites, whose panic text was likewise `read …: <e>`).
#[cfg(not(target_arch = "wasm32"))]
pub fn read_asset(tc_root: &Path, rel: &str) -> Vec<u8> {
    std::fs::read(tc_root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// wasm impl: filled by Slice 3f T2 (curated embedded manifest). Stubbed here so
/// the native crate builds; the wasm target is not compiled until T3/CI.
#[cfg(target_arch = "wasm32")]
pub fn read_asset(_tc_root: &Path, _rel: &str) -> Vec<u8> {
    unimplemented!("wasm embed: T2")
}
