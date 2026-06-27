//! Per-object `.cfg` (TOML) loading: weapon / nobject / sobject parameter
//! tables. Reproduces the values C++ `LoadWeaponConfig` / `LoadNObjectConfig` /
//! `LoadSObjectConfig` (`src/game/common_model.hpp`) parse, including every
//! resolved name→index cross-reference. Idiomatic Rust via `serde`/`toml`, not a
//! port of the cereal `TomlInputArchive`. Consumes 1e-1's type-name lists
//! (`crate::tc::TcTypes`) for cross-ref resolution.
#![allow(non_snake_case)]

#[cfg(test)]
mod smoke {
    /// A real flat object .cfg must parse with the `toml` crate.
    #[test]
    fn real_bazooka_cfg_parses() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/weapons/bazooka.cfg"
        ));
        let text = std::str::from_utf8(bytes).expect("bazooka.cfg is UTF-8");
        let doc: toml::Table = toml::from_str(text).expect("toml crate parses bazooka.cfg");
        assert_eq!(doc.get("name").and_then(|v| v.as_str()), Some("BAZOOKA"));
        assert!(doc.contains_key("splinterType"));
    }
}
