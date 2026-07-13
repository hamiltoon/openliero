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

/// wasm impl (Slice 3f T2; `sounds/` added Slice 4c T5): the browser has no
/// filesystem, so the curated TC set is embedded in the binary at compile time
/// and `rel` is keyed into it. The set is exactly what [`crate::loader::load`]
/// reads for the shipped demo scenario plus the full sample set 4c's audio
/// backend loads: the three `sprites/` TGAs, the three object-config dirs
/// (`weapons/`, `nobjects/`, `sobjects/` — ids are dynamic, driven by
/// `tc.types`, so the whole dir must be present), `tc.cfg`, the one demo level,
/// and `sounds/` (31 WAVs, ~505 KB raw — the whole dir, not curated to the demo
/// scenario's reachable set, so a future live-wasm build stays correct without
/// re-touching this file, design §8 "wasm embed decision"). The unused big
/// levels are still deliberately excluded. `tc_root` is ignored. A miss
/// `panic!`s — the key set is build-time-known, so a miss is a bug, mirroring
/// the native `read {rel}: {e}`.
///
/// Keying: `include_dir!` indexes each subtree relative to *its own* root, so the
/// stored key for `sprites/small.tga` is `small.tga`. We strip the leading
/// directory segment from `rel` (the same string the fs closure builds) before
/// `Dir::get_file`, so the one source of truth — the loader's `rel` — drives both
/// branches (verified against include_dir 0.7 `Dir::get_file`/`File::contents`).
#[cfg(target_arch = "wasm32")]
pub fn read_asset(_tc_root: &Path, rel: &str) -> Vec<u8> {
    use include_dir::{include_dir, Dir};

    // Curated subtrees (whole dirs: object-config ids come from `tc.types`).
    static SPRITES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sprites");
    static WEAPONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/weapons");
    static NOBJECTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/nobjects");
    static SOBJECTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sobjects");
    // 4c T5: the full sample set (`game::audio::load_sound_table_wasm`'s
    // source). Includes the shipped `sounds/LICENSE` alongside the 30 WAVs —
    // harmless (never keyed by `read_asset`, just extra embedded bytes).
    static SOUNDS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sounds");

    // Individual top-level files. `include_bytes!` (unlike `include_dir!`) takes a
    // literal path with no `$CARGO_MANIFEST_DIR` expansion, so `concat!(env!(…))`.
    static TC_CFG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/tc.cfg"
    ));
    // The demo level the shipped `blood` scenario references (`level` line):
    // `Levels/render_stage.lev`. The other big levels are NOT embedded.
    const DEMO_LEVEL_REL: &str = "Levels/render_stage.lev";
    static DEMO_LEVEL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/Levels/render_stage.lev"
    ));

    let from_dir = |dir: &Dir<'_>, key: &str| -> Vec<u8> {
        dir.get_file(key)
            .unwrap_or_else(|| panic!("wasm embed: no asset {rel}"))
            .contents()
            .to_vec()
    };

    if let Some(key) = rel.strip_prefix("sprites/") {
        return from_dir(&SPRITES, key);
    }
    if let Some(key) = rel.strip_prefix("weapons/") {
        return from_dir(&WEAPONS, key);
    }
    if let Some(key) = rel.strip_prefix("nobjects/") {
        return from_dir(&NOBJECTS, key);
    }
    if let Some(key) = rel.strip_prefix("sobjects/") {
        return from_dir(&SOBJECTS, key);
    }
    if let Some(key) = rel.strip_prefix("sounds/") {
        return from_dir(&SOUNDS, key);
    }
    match rel {
        "tc.cfg" => TC_CFG.to_vec(),
        DEMO_LEVEL_REL => DEMO_LEVEL.to_vec(),
        _ => panic!("wasm embed: no asset {rel}"),
    }
}
