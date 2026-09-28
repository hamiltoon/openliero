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
/// Bytes C++ `Level::load` rejects ([`cpp_accepts`], Step 4½e-2 plan D6) or that do not parse
/// as a level are `None` too; the caller then generates a random level, as C++ does
/// (`level.cpp:416-418`).
pub fn read_level(
    store: &dyn ConfigStore,
    tc_root: &Path,
    level_file: &str,
    load_powerlevel_palette: bool,
) -> Option<LevelData> {
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
    let bytes = bytes?;
    if !cpp_accepts(&bytes, load_powerlevel_palette) {
        return None;
    }
    assets::level::load(&bytes).ok()
}

/// C++ `Level::load`'s accept rules (`level.cpp:229-392`; Step 4½e-2 plan fact 11, D6) over the
/// whole file: would it return `true` without a `Reader::Get` throwing? `assets::level::load`
/// keeps a level whose POWERLEVEL or MODERNLV block is truncated (a Step-1 leniency); C++
/// rejects it, which is a random level at NEW GAME and no preview. (A rejection under clause (e)
/// is C++ UB at NEW GAME, T0 P10; Rust falls back to random and no gated case reaches it.)
///
/// (a) `OLLEVEL2`: the 5 header bytes present and `1 ≤ w, h ≤ 4096`; otherwise legacy 504×350,
/// whose first up-to-8 probe bytes are material bytes. (b) The material bytes complete.
/// (c) With `load_powerlevel_palette`: `TryGet` 10 bytes; `"POWERLEVEL"` needs 768 palette
/// bytes, and the MODERNLV probe then reads 8 fresh bytes; otherwise the ≥ 8 pre-read bytes'
/// first 8 are the probe (1–7 pre-read: no probe). (d) Without (c) the probe is 8 fresh bytes.
/// (e) `"MODERNLV"`: `w·h·4` display bytes (less the pre-read tail) and `w·h` valid bytes must
/// follow; the animation extension after them is lenient (`TryGet`).
pub fn cpp_accepts(bytes: &[u8], load_powerlevel_palette: bool) -> bool {
    const MAX_DIM: usize = 4096;
    let (w, h, mut pos) = if bytes.len() >= 8 && &bytes[..8] == b"OLLEVEL2" {
        let Some(hdr) = bytes.get(8..13) else {
            return false;
        };
        let w = usize::from(hdr[1]) | (usize::from(hdr[2]) << 8);
        let h = usize::from(hdr[3]) | (usize::from(hdr[4]) << 8);
        if !(1..=MAX_DIM).contains(&w) || !(1..=MAX_DIM).contains(&h) {
            return false;
        }
        (w, h, 13)
    } else {
        (504, 350, 0)
    };
    let cells = w * h;
    // (b)
    pos += cells;
    if bytes.len() < pos {
        return false;
    }
    let rest = |p: usize| bytes.len() - p;
    // (c) / (d): the MODERNLV probe and how many of its trailing bytes were pre-read.
    let probe: Option<&[u8]> = if load_powerlevel_palette {
        let n = rest(pos).min(10);
        if n == 10 && &bytes[pos..pos + 10] == b"POWERLEVEL" {
            pos += 10;
            if rest(pos) < 768 {
                return false;
            }
            pos += 768;
            let m = rest(pos).min(8);
            let p = &bytes[pos..pos + m];
            pos += m;
            (m == 8).then_some(p)
        } else if n >= 8 {
            // The pre-read bytes past the magic (`prepend`, `level.cpp:317-321`) are the first
            // display bytes, so the display block starts right after the magic.
            let p = &bytes[pos..pos + 8];
            pos += 8;
            Some(p)
        } else {
            None
        }
    } else {
        let m = rest(pos).min(8);
        let p = &bytes[pos..pos + m];
        pos += m;
        (m == 8).then_some(p)
    };
    // (e)
    if probe == Some(b"MODERNLV".as_slice()) {
        return rest(pos) >= cells * 4 + cells;
    }
    true
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
            read_level(
                &sys,
                tc(),
                "/openliero/TC/openliero/Levels/water_stage.lev",
                true
            ),
            Some(water()),
            "a system-only level plays (finding 2; Q4 = A)"
        );
        let user = MemoryStore::new().with_root_label("./user");
        user.write("TC/openliero/Levels/water_stage.lev", &bytes)
            .unwrap();
        assert_eq!(
            read_level(
                &user,
                tc(),
                "./user/TC/openliero/Levels/water_stage.lev",
                true
            ),
            Some(water()),
            "the fixture's relative label (T0: `./user`)"
        );
        assert_eq!(
            read_level(
                &sys,
                tc(),
                "/openliero/TC/openliero/Levels/missing.lev",
                true
            ),
            None
        );
        assert_eq!(
            read_level(
                &sys,
                tc(),
                "/openlierox/TC/openliero/Levels/water_stage.lev",
                true
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
            read_level(&store, tc(), file.to_str().unwrap(), true),
            Some(water())
        );
        assert_eq!(
            read_level(&store, tc(), dir.join("gone.lev").to_str().unwrap(), true),
            None
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rule_3_a_relative_path_is_tc_relative() {
        let store = MemoryStore::new();
        assert_eq!(
            read_level(&store, tc(), WATER, true),
            Some(water()),
            "4½d `?level=`"
        );
        assert_eq!(read_level(&store, tc(), "Levels/missing.lev", true), None);
        assert_eq!(
            read_level(&store, tc(), "", true),
            None,
            "\"\" is \".LEV\": missing"
        );
    }

    #[test]
    fn dot_lev_is_appended_only_when_the_whole_name_has_no_dot() {
        let bytes = water_bytes();
        let store = MemoryStore::with_system(&[("Levels/X.LEV", &bytes)]);
        assert_eq!(
            read_level(&store, tc(), "/openliero/Levels/X", true),
            Some(water())
        );
        let dotted =
            MemoryStore::with_system(&[("Levels/X.LEV", &bytes)]).with_root_label("./user");
        assert_eq!(
            read_level(&dotted, tc(), "./user/Levels/X", true),
            None,
            "`./user` holds a '.', so C++ appends nothing (level.cpp:402)"
        );
    }

    #[test]
    fn garbage_bytes_fall_back_to_a_random_level() {
        let store = MemoryStore::with_system(&[("Levels/bad.lev", b"not a level")]);
        assert_eq!(
            read_level(&store, tc(), "/openliero/Levels/bad.lev", true),
            None
        );
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

    /// An `OLLEVEL2` level of `w`×`h` material bytes `0xA0`.
    fn sized(w: u16, h: u16) -> Vec<u8> {
        let mut v = b"OLLEVEL2".to_vec();
        v.push(1);
        v.extend(w.to_le_bytes());
        v.extend(h.to_le_bytes());
        v.extend(std::iter::repeat_n(0xA0, usize::from(w) * usize::from(h)));
        v
    }

    fn cat(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    fn both(bytes: &[u8]) -> (bool, bool) {
        (cpp_accepts(bytes, true), cpp_accepts(bytes, false))
    }

    #[test]
    fn cpp_accepts_the_legacy_and_sized_headers_as_level_load_does() {
        // level.cpp:229-265 (plan fact 11 (a), (b)).
        let legacy = vec![0xA0; 504 * 350];
        assert_eq!(both(&legacy), (true, true), "legacy exact");
        assert_eq!(both(&legacy[1..]), (false, false), "legacy one byte short");
        assert_eq!(both(&[]), (false, false), "empty");
        assert_eq!(both(&sized(8, 8)), (true, true));
        assert_eq!(both(&sized(4096, 1)), (true, true));
        let mut zero = sized(8, 8);
        zero[9] = 0;
        assert_eq!(both(&zero), (false, false), "w = 0");
        let mut big = sized(1, 1);
        big[9..11].copy_from_slice(&4097u16.to_le_bytes());
        assert_eq!(both(&big), (false, false), "w = 4097");
        assert_eq!(
            both(b"OLLEVEL2\x01\x08\x00"),
            (false, false),
            "header truncated"
        );
        assert_eq!(both(b"OLLEVEL2"), (false, false), "no header at all");
        let short = sized(8, 8);
        assert_eq!(
            both(&short[..short.len() - 1]),
            (false, false),
            "materials short"
        );
        let readme =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/README.md")).unwrap();
        assert_eq!(
            both(&readme),
            (false, false),
            "README.md as a level (T0 P10 `broken`)"
        );
    }

    #[test]
    fn cpp_accepts_powerlevel_only_whole_and_only_when_the_flag_reads_it() {
        // level.cpp:281-294, gfx/palette.cpp:61-72 (fact 11 (c), (d)).
        let lv = sized(8, 8);
        let pal = vec![0x3F; 768];
        assert_eq!(both(&cat(&[&lv, b"POWERLEVEL", &pal])), (true, true));
        let short = cat(&[&lv, b"POWERLEVEL", &pal[1..]]);
        assert_eq!(
            both(&short),
            (false, true),
            "767 palette bytes: rejected only when the flag reads the block"
        );
        for n in 1..=7 {
            assert_eq!(
                both(&cat(&[&lv, &vec![7; n]])),
                (true, true),
                "{n} trailing bytes"
            );
        }
        assert_eq!(
            both(&cat(&[&lv, b"POWERLEVE"])),
            (true, true),
            "9 bytes: no magic"
        );
    }

    #[test]
    fn cpp_accepts_modernlv_only_with_its_display_and_valid_bytes() {
        // level.cpp:297-330 (fact 11 (e)); the animation extension is lenient (:332-384).
        let lv = sized(8, 8);
        let cells = 64;
        let display = vec![0x11; cells * 4];
        let valid = vec![1; cells];
        let whole = cat(&[&lv, b"MODERNLV", &display, &valid]);
        assert_eq!(both(&whole), (true, true));
        let short = cat(&[&lv, b"MODERNLV", &display[1..], &valid]);
        assert_eq!(both(&short), (false, false), "short by one display byte");
        let no_valid = cat(&[&lv, b"MODERNLV", &display, &valid[1..]]);
        assert_eq!(both(&no_valid), (false, false), "short by one valid byte");
        assert_eq!(
            both(&cat(&[&lv, b"MODERNLV"])),
            (false, false),
            "a lone magic at the end"
        );
        assert_eq!(
            both(&cat(&[&lv, b"MODERNLV", &[0]])),
            (false, false),
            "9 bytes: the pre-read tail is display bytes (prepend)"
        );
        // T0's `trunc.lev`: OLLEVEL2 8×8 + MODERNLV + 10 zero bytes.
        assert_eq!(both(&cat(&[&lv, b"MODERNLV", &[0; 10]])), (false, false));
        // A truncated animation extension: a ramp count with no ramp.
        assert_eq!(both(&cat(&[&whole, &[3, 0]])), (true, true));
        assert_eq!(
            both(&cat(&[
                &lv,
                b"POWERLEVEL",
                &[0x3F; 768],
                b"MODERNLV",
                &display,
                &valid
            ])),
            (true, true),
            "POWERLEVEL, then a fresh MODERNLV probe"
        );
        assert_eq!(
            both(&cat(&[
                &lv,
                b"POWERLEVEL",
                &[0x3F; 768],
                b"MODERNLV",
                &display[1..],
                &valid
            ])),
            (false, true),
            "without the flag the probe reads `POWERLEV`"
        );
        assert!(
            assets::level::load(&short).is_ok(),
            "the Step-1 loader is lenient here; `read_level` must not be"
        );
    }

    #[test]
    fn every_shipped_level_is_accepted_with_the_flag_on_and_off() {
        for name in [
            "modern_test",
            "physics_fall_test",
            "render_stage",
            "see_shadow_test",
            "water_stage",
        ] {
            let bytes = scenario::assets::read_asset(tc(), &format!("Levels/{name}.lev"));
            assert_eq!(both(&bytes), (true, true), "{name}");
        }
    }

    #[test]
    fn read_level_refuses_what_cpp_rejects() {
        let lv = sized(8, 8);
        let trunc = cat(&[&lv, b"MODERNLV", &[0; 10]]);
        let short_pal = cat(&[&lv, b"POWERLEVEL", &[0x3F; 767]]);
        let store = MemoryStore::with_system(&[
            ("Levels/trunc.lev", &trunc),
            ("Levels/pal.lev", &short_pal),
        ]);
        assert_eq!(
            read_level(&store, tc(), "/openliero/Levels/trunc.lev", true),
            None
        );
        assert_eq!(
            read_level(&store, tc(), "/openliero/Levels/trunc.lev", false),
            None
        );
        assert_eq!(
            read_level(&store, tc(), "/openliero/Levels/pal.lev", true),
            None
        );
        assert!(read_level(&store, tc(), "/openliero/Levels/pal.lev", false).is_some());
        let s = Settings {
            random_level: false,
            level_file: "/openliero/Levels/pal.lev".into(),
            ..Settings::default()
        };
        assert_eq!(
            LevelSlot::generate(tc(), &s, &store, 7).level,
            generate_level(tc(), &Settings::default(), None, 7),
            "the slot passes `load_powerlevel_palette` (on by default): random"
        );
        let off = Settings {
            load_powerlevel_palette: false,
            ..s
        };
        assert_eq!(LevelSlot::generate(tc(), &off, &store, 7).level.width, 8);
    }
}
