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

/// [`read_asset`] that reports a miss instead of panicking (Step 4½e-1): the level a settings
/// file names may be absent (`ui::shell::level_path`'s TC-relative rule, fact 22).
///
/// Native impl: `std::fs::read(tc_root.join(rel)).ok()`.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_read_asset(tc_root: &Path, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(tc_root.join(rel)).ok()
}

/// The shipped setups, embedded (Step 4½e-1, plan D11): the browser's config store carries them
/// in its read-only system layer (`game::config`), keyed as config paths, so `liero.cfg` loads
/// at boot as it does from `data/` natively. About 2 KB each; compiled on every target so the
/// native tests pin them. The level catalogue ([`EMBEDDED_LEVELS`]) joins them in 4½e-2.
pub static EMBEDDED_SETUPS: &[(&str, &[u8])] = &[
    (
        "Setups/liero.cfg",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/Setups/liero.cfg"
        )),
    ),
    (
        "Setups/orbmit.cfg",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/Setups/orbmit.cfg"
        )),
    ),
];

/// The four small shipped levels (Step 4½e-2, plan D9, ruling Q6), keyed as config paths
/// (`TC/openliero/Levels/<stem>.lev`) in `game::web_params::LEVELS` order. The browser's config
/// store lists them in the level selector, and the wasm [`try_read_asset`] reads its
/// TC-relative `Levels/<x>` from here, so each level is embedded once. `modern_test.lev`
/// (1.2 MB) is not embedded. Compiled on every target, like [`EMBEDDED_SETUPS`], so the native
/// tests pin them; an unused static is dropped by the linker.
pub static EMBEDDED_LEVELS: &[(&str, &[u8])] = &[
    (
        "TC/openliero/Levels/render_stage.lev",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/Levels/render_stage.lev"
        )),
    ),
    (
        "TC/openliero/Levels/water_stage.lev",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/Levels/water_stage.lev"
        )),
    ),
    (
        "TC/openliero/Levels/see_shadow_test.lev",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/Levels/see_shadow_test.lev"
        )),
    ),
    (
        "TC/openliero/Levels/physics_fall_test.lev",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/Levels/physics_fall_test.lev"
        )),
    ),
];

/// Every directory of the shipped `data/` (Step 4½e-2, plan D9, fact 23), the root excluded:
/// the C++ web build preloads `data/{Profiles,Resources,Setups,TC}` under `/openliero`, so its
/// selectors list these folders even where the browser store holds no file under them.
pub const EMBEDDED_DIRS: &[&str] = &[
    "Profiles",
    "Resources",
    "Setups",
    "TC",
    "TC/openliero",
    "TC/openliero/Levels",
    "TC/openliero/nobjects",
    "TC/openliero/sobjects",
    "TC/openliero/sounds",
    "TC/openliero/sprites",
    "TC/openliero/weapons",
];

/// The object-config folders of the stock TC whose `.cfg` files the browser store carries.
const OBJECT_DIRS: [&str; 3] = ["weapons", "nobjects", "sobjects"];

/// The browser config store's files (Step 4½e-2, plan D9), keyed as config paths and sorted by
/// key: the two setups, the four small levels, `TC/openliero/tc.cfg`, and every `.cfg` of
/// `TC/openliero/{weapons,nobjects,sobjects}` — each `data/` file the level selector's `LEV`
/// or LOAD SETUP's `CFG` filter shows, minus `modern_test.lev` (ruling Q6). On wasm the TC files
/// come from the embedded trees; natively they are read from [`crate::paths::DATA_ROOT`] (tests
/// and parity).
pub fn browser_system_files() -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = EMBEDDED_SETUPS
        .iter()
        .chain(EMBEDDED_LEVELS)
        .map(|(rel, bytes)| (rel.to_string(), bytes.to_vec()))
        .collect();
    files.extend(tc_config_files());
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// Whether `name`'s extension is `cfg`, case-insensitively (the options selector's filter).
fn is_cfg(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("cfg"))
}

/// [`browser_system_files`]' TC part, read from [`crate::paths::TC_ROOT`]. An unreadable file
/// or folder contributes nothing.
#[cfg(not(target_arch = "wasm32"))]
fn tc_config_files() -> Vec<(String, Vec<u8>)> {
    let tc = Path::new(crate::paths::TC_ROOT);
    let mut out = Vec::new();
    if let Ok(bytes) = std::fs::read(tc.join("tc.cfg")) {
        out.push(("TC/openliero/tc.cfg".to_string(), bytes));
    }
    for dir in OBJECT_DIRS {
        let Ok(read) = std::fs::read_dir(tc.join(dir)) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_cfg(&name) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(entry.path()) {
                out.push((format!("TC/openliero/{dir}/{name}"), bytes));
            }
        }
    }
    out
}

/// [`browser_system_files`]' TC part, from the embedded trees.
#[cfg(target_arch = "wasm32")]
fn tc_config_files() -> Vec<(String, Vec<u8>)> {
    let mut out = vec![("TC/openliero/tc.cfg".to_string(), embed::TC_CFG.to_vec())];
    for (dir, tree) in OBJECT_DIRS
        .iter()
        .zip([&embed::WEAPONS, &embed::NOBJECTS, &embed::SOBJECTS])
    {
        for file in tree.files() {
            let name = file.path().to_string_lossy();
            if is_cfg(&name) {
                out.push((
                    format!("TC/openliero/{dir}/{name}"),
                    file.contents().to_vec(),
                ));
            }
        }
    }
    out
}

/// wasm [`read_asset`]: the embedded set's [`try_read_asset`], panicking on a miss (a bug: the
/// key set is build-time-known).
#[cfg(target_arch = "wasm32")]
pub fn read_asset(tc_root: &Path, rel: &str) -> Vec<u8> {
    try_read_asset(tc_root, rel).unwrap_or_else(|| panic!("wasm embed: no asset {rel}"))
}

/// wasm impl (Slice 3f T2; `sounds/` added Slice 4c T5): the browser has no
/// filesystem, so the curated TC set is embedded in the binary at compile time
/// and `rel` is keyed into it. The set is exactly what [`crate::loader::load`]
/// reads for the shipped demo scenario plus the full sample set 4c's audio
/// backend loads: the three `sprites/` TGAs, the three object-config dirs
/// (`weapons/`, `nobjects/`, `sobjects/` — ids are dynamic, driven by
/// `tc.types`, so the whole dir must be present), `tc.cfg`, the four small levels
/// ([`EMBEDDED_LEVELS`], Step 4½e-2), and `sounds/` (31 WAVs, ~505 KB raw — the
/// whole dir, not curated to the demo scenario's reachable set, so a future
/// live-wasm build stays correct without re-touching this file, design §8 "wasm
/// embed decision"). The big
/// `modern_test.lev` is still deliberately excluded. `tc_root` is ignored. A miss is
/// `None` (Step 4½e-1); [`read_asset`] turns it into the `panic!` — the key set is
/// build-time-known, so a miss there is a bug, mirroring the native `read {rel}: {e}`.
///
/// Keying: `include_dir!` indexes each subtree relative to *its own* root, so the
/// stored key for `sprites/small.tga` is `small.tga`. We strip the leading
/// directory segment from `rel` (the same string the fs closure builds) before
/// `Dir::get_file`, so the one source of truth — the loader's `rel` — drives both
/// branches (verified against include_dir 0.7 `Dir::get_file`/`File::contents`).
#[cfg(target_arch = "wasm32")]
pub fn try_read_asset(_tc_root: &Path, rel: &str) -> Option<Vec<u8>> {
    use include_dir::Dir;

    let from_dir = |dir: &Dir<'_>, key: &str| -> Option<Vec<u8>> {
        Some(dir.get_file(key)?.contents().to_vec())
    };

    if let Some(key) = rel.strip_prefix("sprites/") {
        return from_dir(&embed::SPRITES, key);
    }
    if let Some(key) = rel.strip_prefix("weapons/") {
        return from_dir(&embed::WEAPONS, key);
    }
    if let Some(key) = rel.strip_prefix("nobjects/") {
        return from_dir(&embed::NOBJECTS, key);
    }
    if let Some(key) = rel.strip_prefix("sobjects/") {
        return from_dir(&embed::SOBJECTS, key);
    }
    if let Some(key) = rel.strip_prefix("sounds/") {
        return from_dir(&embed::SOUNDS, key);
    }
    // The demo level the shipped `blood` scenario references (`Levels/render_stage.lev`) and
    // the other small stock levels a PR preview may pick with `?level=`: the config-keyed
    // [`EMBEDDED_LEVELS`] (Step 4½e-2), so the bytes are embedded once.
    if rel.starts_with("Levels/") {
        let key = format!("TC/openliero/{rel}");
        return EMBEDDED_LEVELS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, bytes)| bytes.to_vec());
    }
    match rel {
        "tc.cfg" => Some(embed::TC_CFG.to_vec()),
        _ => None,
    }
}

/// The wasm embed (Slice 3f T2): the curated TC subtrees and `tc.cfg`. Module-level since
/// Step 4½e-2, so [`browser_system_files`] shares the object-config trees with
/// [`try_read_asset`].
#[cfg(target_arch = "wasm32")]
mod embed {
    use include_dir::{include_dir, Dir};

    // Curated subtrees (whole dirs: object-config ids come from `tc.types`).
    pub static SPRITES: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sprites");
    pub static WEAPONS: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/weapons");
    pub static NOBJECTS: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/nobjects");
    pub static SOBJECTS: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sobjects");
    // 4c T5: the full sample set (`game::audio::load_sound_table_wasm`'s
    // source). Includes the shipped `sounds/LICENSE` alongside the 30 WAVs —
    // harmless (never keyed by `read_asset`, just extra embedded bytes).
    pub static SOUNDS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sounds");

    // Individual top-level files. `include_bytes!` (unlike `include_dir!`) takes a
    // literal path with no `$CARGO_MANIFEST_DIR` expansion, so `concat!(env!(…))`.
    pub static TC_CFG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/tc.cfg"
    ));
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

    #[test]
    fn try_read_asset_hits_and_misses() {
        let root = Path::new(TC_ROOT);
        let hit = try_read_asset(root, "Levels/water_stage.lev").expect("a shipped level");
        assert_eq!(hit, read_asset(root, "Levels/water_stage.lev"));
        assert!(!hit.is_empty());
        assert_eq!(try_read_asset(root, "Levels/no_such_level.lev"), None);
        assert_eq!(
            try_read_asset(root, "sprites"),
            None,
            "a directory is not a file"
        );
    }

    #[test]
    fn the_embedded_setups_are_the_shipped_files_and_parse() {
        let data = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");
        let rels: Vec<&str> = EMBEDDED_SETUPS.iter().map(|(rel, _)| *rel).collect();
        assert_eq!(rels, ["Setups/liero.cfg", "Setups/orbmit.cfg"]);
        for (rel, bytes) in EMBEDDED_SETUPS {
            assert_eq!(
                *bytes,
                std::fs::read(Path::new(data).join(rel)).unwrap().as_slice(),
                "{rel}"
            );
            let text = std::str::from_utf8(bytes).expect("UTF-8");
            crate::settings_toml::settings_from_toml(text).expect("a shipped setup parses");
        }
    }

    #[test]
    #[should_panic(expected = "read Levels/no_such_level.lev")]
    fn read_asset_still_panics_with_the_read_text() {
        let _ = read_asset(Path::new(TC_ROOT), "Levels/no_such_level.lev");
    }

    // ---- Step 4½e-2: the embedded level and directory catalogue, the browser files ----

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

    /// Every directory (`is_dir`) or file under `data/`, as sorted config paths.
    fn walk(is_dir: bool) -> Vec<String> {
        fn go(root: &Path, rel: &str, is_dir: bool, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(root.join(rel)).unwrap().flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let child = if rel.is_empty() {
                    name
                } else {
                    format!("{rel}/{name}")
                };
                let dir = entry.file_type().unwrap().is_dir();
                if dir == is_dir {
                    out.push(child.clone());
                }
                if dir {
                    go(root, &child, is_dir, out);
                }
            }
        }
        let mut out = Vec::new();
        go(Path::new(DATA), "", is_dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn the_embedded_levels_are_the_four_small_shipped_files() {
        let rels: Vec<&str> = EMBEDDED_LEVELS.iter().map(|(rel, _)| *rel).collect();
        assert_eq!(
            rels,
            [
                "TC/openliero/Levels/render_stage.lev",
                "TC/openliero/Levels/water_stage.lev",
                "TC/openliero/Levels/see_shadow_test.lev",
                "TC/openliero/Levels/physics_fall_test.lev",
            ],
            "modern_test stays out (ruling Q6)"
        );
        for (rel, bytes) in EMBEDDED_LEVELS {
            assert_eq!(
                *bytes,
                std::fs::read(Path::new(DATA).join(rel)).unwrap().as_slice(),
                "{rel}"
            );
            let tc_rel = rel.strip_prefix("TC/openliero/").expect("a TC path");
            assert_eq!(
                try_read_asset(Path::new(TC_ROOT), tc_rel).as_deref(),
                Some(*bytes),
                "the TC-relative read is the same file"
            );
        }
    }

    #[test]
    fn the_embedded_dirs_are_the_data_directory_walk() {
        assert_eq!(EMBEDDED_DIRS, walk(true));
        assert_eq!(
            EMBEDDED_DIRS.len(),
            11,
            "fact 23: 11 directories besides the root"
        );
    }

    #[test]
    fn the_browser_files_are_every_shown_data_file_but_modern_test() {
        let shown = |rel: &String| {
            rel.rsplit_once('.').is_some_and(|(_, ext)| {
                ext.eq_ignore_ascii_case("lev") || ext.eq_ignore_ascii_case("cfg")
            })
        };
        let want: Vec<String> = walk(false)
            .into_iter()
            .filter(shown)
            .filter(|rel| rel != "TC/openliero/Levels/modern_test.lev")
            .collect();
        let files = browser_system_files();
        let keys: Vec<&str> = files.iter().map(|(rel, _)| rel.as_str()).collect();
        assert_eq!(keys, want);
        assert_eq!(
            files.len(),
            2 + 4 + 1 + 78,
            "setups, levels, tc.cfg, object configs"
        );
        for (rel, bytes) in &files {
            assert_eq!(
                *bytes,
                std::fs::read(Path::new(DATA).join(rel)).unwrap(),
                "{rel}"
            );
        }
    }
}
