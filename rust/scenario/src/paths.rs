//! Step 4½a-2 — the two compile-time data roots (design §2.2, §9.3.8), resolved relative to
//! this crate so every binary works from any CWD. `DATA_ROOT` is the read-only system layer
//! of `crate::storage::NativeStore` (the Rust analog of C++ `paths::SystemDataRoot`, the
//! binary-adjacent `data/`); `TC_ROOT` is the stock TC. Production code uses these; the
//! self-contained per-test `TC_ROOT` consts in `sim`/`oracle-tests`/… stay as they are.

/// The repository's `data/` directory.
pub const DATA_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");
/// The stock total conversion, `data/TC/openliero`.
pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_roots_point_at_the_repo_data() {
        assert!(Path::new(DATA_ROOT).join("Setups/liero.cfg").is_file());
        assert!(Path::new(TC_ROOT).join("tc.cfg").is_file());
        assert_eq!(TC_ROOT.strip_prefix(DATA_ROOT), Some("/TC/openliero"));
    }
}
