//! Step 4½e-1 — where a `Settings::level_file` is read from (design §4.6; plan facts 22, 27, T0
//! addendum). C++ `Level::GenerateFromSettings` opens `FsNode(level_file)` (`level.cpp:401-412`),
//! and the C++ level selector saves a config-root path (`root_label() + "/" + rel`, findings 3
//! and 12), so a C++ user's `liero.cfg` names files Rust must find — or fall back to a random
//! level from, never panic on. The generator itself stays I/O-free (`new_game::generate_level`).

use std::path::Path;

use assets::level::LevelData;
use scenario::storage::ConfigStore;
use sim::levelgen::level_file_name;

/// The level `level_file` names, in rule order (`.LEV` appended first when the whole name has no
/// `.`, `level.cpp:402-404`):
/// 1. it starts with `store.root_label() + "/"`: the rest through the store's merged view — the
///    user layer, else the system layer (design Q4 = A: a picked shipped level plays);
/// 2. an absolute path (not on wasm): that file;
/// 3. a relative path: TC-relative (`try_read_asset`), the 4½d `?level=` convention;
/// 4. otherwise `None`.
///
/// Bytes that do not parse as a level are `None` too; the caller then generates a random level,
/// as C++ does (`level.cpp:416-418`).
pub fn read_level(store: &dyn ConfigStore, tc_root: &Path, level_file: &str) -> Option<LevelData> {
    let name = level_file_name(level_file);
    let label = store.root_label();
    let in_root = (!label.is_empty())
        .then(|| name.strip_prefix(label)?.strip_prefix('/'))
        .flatten();
    let bytes = if let Some(rel) = in_root {
        store.read(rel)
    } else if Path::new(&name).is_absolute() {
        read_absolute(&name)
    } else {
        scenario::assets::try_read_asset(tc_root, &name)
    };
    assets::level::load(&bytes?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_absolute(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// The browser has no filesystem outside its store: rule 4.
#[cfg(target_arch = "wasm32")]
fn read_absolute(_path: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use scenario::paths::TC_ROOT;
    use scenario::settings::Settings;
    use scenario::storage::MemoryStore;

    use super::*;
    use crate::shell::level_slot::LevelSlot;
    use crate::shell::new_game::generate_level;

    const WATER: &str = "Levels/water_stage.lev";

    fn tc() -> &'static Path {
        Path::new(TC_ROOT)
    }

    fn water_bytes() -> Vec<u8> {
        scenario::assets::read_asset(tc(), WATER)
    }

    fn water() -> LevelData {
        assets::level::load(&water_bytes()).unwrap()
    }

    #[test]
    fn rule_1_the_root_form_reads_through_the_merged_view() {
        let bytes = water_bytes();
        let sys = MemoryStore::with_system(&[("TC/openliero/Levels/water_stage.lev", &bytes)]);
        assert_eq!(
            read_level(&sys, tc(), "/openliero/TC/openliero/Levels/water_stage.lev"),
            Some(water()),
            "a system-only level plays (finding 2; Q4 = A)"
        );
        let user = MemoryStore::new().with_root_label("./user");
        user.write("TC/openliero/Levels/water_stage.lev", &bytes)
            .unwrap();
        assert_eq!(
            read_level(&user, tc(), "./user/TC/openliero/Levels/water_stage.lev"),
            Some(water()),
            "the fixture's relative label (T0: `./user`)"
        );
        assert_eq!(
            read_level(&sys, tc(), "/openliero/TC/openliero/Levels/missing.lev"),
            None
        );
        assert_eq!(
            read_level(
                &sys,
                tc(),
                "/openlierox/TC/openliero/Levels/water_stage.lev"
            ),
            None,
            "the label must end at a separator (then rule 2: no such file)"
        );
    }

    #[test]
    fn rule_2_an_absolute_path_is_read_from_disk() {
        let dir = std::env::temp_dir().join(format!("liero_rs_level_path_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("abs.lev");
        std::fs::write(&file, water_bytes()).unwrap();
        let store = MemoryStore::new();
        assert_eq!(
            read_level(&store, tc(), file.to_str().unwrap()),
            Some(water())
        );
        assert_eq!(
            read_level(&store, tc(), dir.join("gone.lev").to_str().unwrap()),
            None
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rule_3_a_relative_path_is_tc_relative() {
        let store = MemoryStore::new();
        assert_eq!(
            read_level(&store, tc(), WATER),
            Some(water()),
            "4½d `?level=`"
        );
        assert_eq!(read_level(&store, tc(), "Levels/missing.lev"), None);
        assert_eq!(
            read_level(&store, tc(), ""),
            None,
            "\"\" is \".LEV\": missing"
        );
    }

    #[test]
    fn dot_lev_is_appended_only_when_the_whole_name_has_no_dot() {
        let bytes = water_bytes();
        let store = MemoryStore::with_system(&[("Levels/X.LEV", &bytes)]);
        assert_eq!(
            read_level(&store, tc(), "/openliero/Levels/X"),
            Some(water())
        );
        let dotted =
            MemoryStore::with_system(&[("Levels/X.LEV", &bytes)]).with_root_label("./user");
        assert_eq!(
            read_level(&dotted, tc(), "./user/Levels/X"),
            None,
            "`./user` holds a '.', so C++ appends nothing (level.cpp:402)"
        );
    }

    #[test]
    fn garbage_bytes_fall_back_to_a_random_level() {
        let store = MemoryStore::with_system(&[("Levels/bad.lev", b"not a level")]);
        assert_eq!(read_level(&store, tc(), "/openliero/Levels/bad.lev"), None);
        let file = Settings {
            random_level: false,
            level_file: "/openliero/Levels/bad.lev".into(),
            ..Settings::default()
        };
        let slot = LevelSlot::generate(tc(), &file, &store, 7);
        assert_eq!(
            slot.level,
            generate_level(tc(), &Settings::default(), None, 7),
            "GenerateRandom from the same Rand (4½b)"
        );
        assert_eq!(slot.provenance.level_file, "/openliero/Levels/bad.lev");
        let missing = Settings {
            level_file: "/openliero/Levels/none.lev".into(),
            ..file
        };
        assert_eq!(
            LevelSlot::generate(tc(), &missing, &store, 7).level,
            slot.level,
            "a missing file never panics (fact 22)"
        );
    }

    #[test]
    fn a_slot_reads_the_file_only_when_the_level_is_not_random() {
        let bytes = water_bytes();
        let store = MemoryStore::with_system(&[("Levels/water_stage.lev", &bytes)]);
        let s = Settings {
            random_level: false,
            level_file: "/openliero/Levels/water_stage.lev".into(),
            ..Settings::default()
        };
        let slot = LevelSlot::generate(tc(), &s, &store, 3);
        assert_eq!(
            (slot.level.width, slot.level.height),
            (water().width, water().height)
        );
        let random = Settings {
            random_level: true,
            ..s
        };
        assert_eq!(
            LevelSlot::generate(tc(), &random, &store, 3).level,
            generate_level(tc(), &Settings::default(), None, 3)
        );
    }
}
