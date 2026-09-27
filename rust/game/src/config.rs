//! Where the live shell keeps `liero.cfg` (Step 4½e-1, T10; plan D10, D11; design §7.1, §7.2).
//!
//! Q3 (John): desktop Rust reads and writes the same config root as C++ OpenLiero, so a player's
//! `Setups/liero.cfg` is shared between the two. The native store follows C++ `paths::Resolve`
//! (`filesystem.cpp:767-845`):
//!
//! - `--config-root <dir>` (or `=<dir>`): one directory for reads and writes (`:812-817`); its
//!   Save As shadow check still consults the install's system data (Step 4½e-2, plan D10);
//! - otherwise the per-user directory — `OPENLIERO_TEST_USER_DIR`, else `SDL_GetPrefPath`
//!   (`UserDataRoot`, `:659-679`) — layered over the system data: `OPENLIERO_DATADIR` when set
//!   and present, else the compiled-in `data/` (`SystemDataRoot`, `:681-700`; Rust has no install
//!   layout, so there is no `SDL_GetBasePath` fallback);
//! - neither: an in-memory store, with a warning (nothing is kept after exit).
//!
//! `portable.txt` next to the binary (`:819-831`) is not supported: there is no Rust install
//! layout yet (plan D10).
//!
//! The browser has no filesystem: its store is in memory for the session (4½h adds
//! localStorage). Step 4½e-2 (plan D9) makes it the C++ web build's one directory,
//! `--config-root /openliero` over the preloaded `data/` tree minus `modern_test.lev`.

use std::path::{Path, PathBuf};

use scenario::settings::Settings;
use scenario::storage::{
    self, ConfigStore, MemoryStore, NativeStore, PREF_APP, PREF_ORG, PrefOs, TEST_USER_DIR_ENV,
};

/// `SystemDataRoot`'s runtime override (`filesystem.cpp:684`).
pub const DATADIR_ENV: &str = "OPENLIERO_DATADIR";

/// The store [`resolve_store`] picked, before it is opened (so the choice is testable).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreSpec {
    /// `--config-root`: one directory for reads and writes. `shadow` is C++ `SystemDataRoot()`,
    /// which `ShadowsSystem` consults whatever the config root is (Step 4½e-2, plan D10).
    SingleDir {
        root: PathBuf,
        shadow: Option<PathBuf>,
    },
    /// The per-user directory (writes) over the system data (reads fall back to it).
    Split {
        user: PathBuf,
        system: Option<PathBuf>,
    },
    /// No config root could be determined: in memory, nothing kept.
    Memory,
}

impl StoreSpec {
    pub fn open(self) -> Box<dyn ConfigStore> {
        match self {
            StoreSpec::SingleDir { root, shadow } => {
                Box::new(NativeStore::single_dir(root).with_shadow_root(shadow))
            }
            StoreSpec::Split { user, system } => Box::new(NativeStore::split(user, system)),
            StoreSpec::Memory => Box::new(MemoryStore::new()),
        }
    }
}

/// C++ `paths::Resolve` without `portable.txt` (module docs), over an explicit platform,
/// environment and directory test, with `data_root` as the compiled-in system data.
pub fn resolve_store(
    config_root: Option<&Path>,
    os: PrefOs,
    env: &dyn Fn(&str) -> Option<String>,
    is_dir: &dyn Fn(&Path) -> bool,
    data_root: &Path,
) -> StoreSpec {
    let var = |key: &str| env(key).filter(|v| !v.is_empty());
    // `SystemDataRoot` (`filesystem.cpp:681-700`): OPENLIERO_DATADIR when it is a directory,
    // else the compiled-in data when it is one.
    let system = var(DATADIR_ENV)
        .map(PathBuf::from)
        .filter(|d| is_dir(d))
        .or_else(|| is_dir(data_root).then(|| data_root.to_path_buf()));
    if let Some(root) = config_root.filter(|r| !r.as_os_str().is_empty()) {
        return StoreSpec::SingleDir {
            root: root.to_path_buf(),
            shadow: system,
        };
    }
    let user = match var(TEST_USER_DIR_ENV) {
        Some(dir) => PathBuf::from(dir),
        None => match storage::pref_path_for(os, env, PREF_ORG, PREF_APP) {
            Some(p) => PathBuf::from(p),
            None => return StoreSpec::Memory,
        },
    };
    StoreSpec::Split { user, system }
}

/// The native store for this process: [`resolve_store`] over the real environment and
/// `scenario::paths::DATA_ROOT`, with a warning when it falls back to memory.
#[cfg(not(target_arch = "wasm32"))]
pub fn store_for(config_root: Option<&Path>) -> Box<dyn ConfigStore> {
    let spec = resolve_store(
        config_root,
        PrefOs::current(),
        &|key: &str| std::env::var(key).ok(),
        &|p: &Path| p.is_dir(),
        Path::new(scenario::paths::DATA_ROOT),
    );
    if spec == StoreSpec::Memory {
        warn("no config directory could be determined (no HOME?); settings are not kept");
    }
    spec.open()
}

/// The browser's store (Step 4½e-2, plan D9): the C++ web build's `--config-root /openliero`,
/// in memory for the session. One layer holding the preloaded `data/` files the selectors show
/// (`scenario::assets::browser_system_files`: the setups, the 4 small levels, the TC `.cfg`s)
/// and its 11 directories, so LEVEL and LOAD SETUP list what the C++ web build lists minus
/// `modern_test` (Q6). Saves shadow the preloaded files, and only the reserved `liero.cfg` is
/// refused: a browser player may save over `orbmit`, kept until the page reloads.
pub fn browser_store() -> MemoryStore {
    MemoryStore::single_layer(scenario::assets::browser_system_files())
        .with_dirs(scenario::assets::EMBEDDED_DIRS)
}

/// `gameEntry.cpp:55-58` through [`storage::load_setup`]: the merged `Setups/liero.cfg`, else
/// the defaults (written to the user layer). A failed write is reported and the defaults kept.
pub fn load_settings(store: &dyn ConfigStore) -> Settings {
    storage::load_setup(store).unwrap_or_else(|e| {
        warn(&format!(
            "could not write the default {} under {}: {e}",
            storage::SETUP_REL,
            store.root_label()
        ));
        Settings::default()
    })
}

/// Report a config problem: the browser console on wasm (where `eprintln!` goes nowhere),
/// stderr natively.
pub fn warn(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::warn_1(&format!("openliero: {msg}").into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("openliero: {msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    const DATA: &str = "/opt/data";
    fn dirs(present: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |p| present.iter().any(|d| Path::new(d) == p)
    }

    #[test]
    fn config_root_is_one_directory_whatever_the_environment() {
        let e = env(&[("HOME", "/home/j"), ("OPENLIERO_TEST_USER_DIR", "/tmp/u")]);
        assert_eq!(
            resolve_store(
                Some(Path::new("/r")),
                PrefOs::Unix,
                &e,
                &dirs(&[DATA]),
                Path::new(DATA)
            ),
            StoreSpec::SingleDir {
                root: PathBuf::from("/r"),
                shadow: Some(PathBuf::from(DATA)),
            }
        );
    }

    #[test]
    fn the_default_is_the_pref_path_over_the_data_dir() {
        let e = env(&[("HOME", "/home/j")]);
        assert_eq!(
            resolve_store(None, PrefOs::Unix, &e, &dirs(&[DATA]), Path::new(DATA)),
            StoreSpec::Split {
                user: PathBuf::from("/home/j/.local/share/openliero/openliero/"),
                system: Some(PathBuf::from(DATA)),
            }
        );
        let e = env(&[("HOME", "/home/j"), ("XDG_DATA_HOME", "/x")]);
        assert_eq!(
            resolve_store(None, PrefOs::Unix, &e, &dirs(&[]), Path::new(DATA)),
            StoreSpec::Split {
                user: PathBuf::from("/x/openliero/openliero/"),
                system: None,
            },
            "no system data: reads see the user directory only"
        );
    }

    #[test]
    fn the_test_user_dir_and_the_datadir_override_are_honoured() {
        let e = env(&[
            ("HOME", "/home/j"),
            ("OPENLIERO_TEST_USER_DIR", "user"),
            ("OPENLIERO_DATADIR", "/sys"),
        ]);
        assert_eq!(
            resolve_store(
                None,
                PrefOs::Unix,
                &e,
                &dirs(&["/sys", DATA]),
                Path::new(DATA)
            ),
            StoreSpec::Split {
                user: PathBuf::from("user"),
                system: Some(PathBuf::from("/sys")),
            }
        );
        assert_eq!(
            resolve_store(None, PrefOs::Unix, &e, &dirs(&[DATA]), Path::new(DATA)),
            StoreSpec::Split {
                user: PathBuf::from("user"),
                system: Some(PathBuf::from(DATA)),
            },
            "a missing OPENLIERO_DATADIR falls back to the compiled-in data (SystemDataRoot)"
        );
    }

    #[test]
    fn nothing_resolves_to_memory() {
        let e = env(&[]);
        let spec = resolve_store(None, PrefOs::Unix, &e, &dirs(&[DATA]), Path::new(DATA));
        assert_eq!(spec, StoreSpec::Memory);
        let store = spec.open();
        assert_eq!(store.root_label(), "/openliero");
        assert_eq!(load_settings(&*store), Settings::default());
    }

    #[test]
    fn a_test_user_dir_gets_the_defaults_saved_and_reads_them_back() {
        // gameEntry.cpp:55-58 end to end on a scratch user dir over the real data/: the shipped
        // liero.cfg is read through the system layer and nothing is written at boot; with no
        // system layer the defaults are written to the user dir.
        let scratch = scratch("defaults");
        let user = scratch.join("user");
        let user_s = user.to_string_lossy().into_owned();
        let e = move |key: &str| (key == "OPENLIERO_TEST_USER_DIR").then(|| user_s.clone());
        let data = Path::new(scenario::paths::DATA_ROOT);
        let spec = resolve_store(None, PrefOs::Unix, &e, &|p: &Path| p.is_dir(), data);
        assert_eq!(
            spec,
            StoreSpec::Split {
                user: user.clone(),
                system: Some(data.to_path_buf()),
            }
        );
        let shipped = load_settings(&*spec.open());
        assert!(
            !user.join("Setups/liero.cfg").exists(),
            "the shipped liero.cfg was read, so nothing was saved at boot"
        );
        let text = std::fs::read_to_string(data.join("Setups/liero.cfg")).unwrap();
        assert_eq!(
            shipped,
            scenario::settings_toml::settings_from_toml(&text).unwrap()
        );

        let alone = StoreSpec::Split {
            user: user.clone(),
            system: None,
        };
        assert_eq!(load_settings(&*alone.clone().open()), Settings::default());
        assert!(
            user.join("Setups/liero.cfg").is_file(),
            "the defaults were saved"
        );
        assert_eq!(load_settings(&*alone.open()), Settings::default());
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    #[test]
    fn the_browser_store_reads_the_shipped_liero_cfg() {
        let store = browser_store();
        assert_eq!(store.root_label(), "/openliero");
        let text = std::str::from_utf8(scenario::assets::EMBEDDED_SETUPS[0].1).unwrap();
        assert_eq!(
            load_settings(&store),
            scenario::settings_toml::settings_from_toml(text).unwrap()
        );
        assert_eq!(
            store.user_file(storage::SETUP_REL),
            None,
            "nothing saved at boot"
        );
    }

    #[test]
    fn the_browser_store_is_the_cpp_web_tree_minus_modern_test() {
        // Plan D9: `data/{Profiles,Resources,Setups,TC}` under `/openliero`, one layer.
        let store = browser_store();
        let names = |rel: &str| -> Vec<(String, bool)> {
            store
                .list(rel)
                .into_iter()
                .map(|e| (e.name, e.is_dir))
                .collect()
        };
        let dirs = |n: &[&str]| -> Vec<(String, bool)> {
            n.iter().map(|n| (n.to_string(), true)).collect()
        };
        assert_eq!(names(""), dirs(&["Profiles", "Resources", "Setups", "TC"]));
        assert_eq!(
            names("Profiles"),
            [],
            "an empty shipped folder is still listed"
        );
        assert_eq!(
            names("TC/openliero/Levels"),
            [
                "physics_fall_test.lev",
                "render_stage.lev",
                "see_shadow_test.lev",
                "water_stage.lev"
            ]
            .map(|n| (n.to_string(), false)),
            "the 4 small levels, no modern_test (Q6)"
        );
        assert_eq!(
            names("Setups"),
            [
                ("liero.cfg".to_string(), false),
                ("orbmit.cfg".to_string(), false)
            ]
        );
    }

    #[test]
    fn the_browser_store_refuses_only_the_reserved_name() {
        // Plan D9 / fact 14: the C++ web build's SystemDataRoot() (`/`) has no Setups, so a
        // browser player may save over `orbmit` (kept until the page reloads).
        let store = browser_store();
        assert!(store.shadows_system("Setups", "liero.cfg"));
        assert!(!store.shadows_system("Setups", "orbmit.cfg"));
        assert!(!store.shadows_system("Setups", "mine.cfg"));
        store.write("Setups/orbmit.cfg", b"x").unwrap();
        assert_eq!(store.read("Setups/orbmit.cfg").as_deref(), Some(&b"x"[..]));
        assert_eq!(
            browser_store().read("Setups/orbmit.cfg").as_deref(),
            Some(scenario::assets::EMBEDDED_SETUPS[1].1),
            "a fresh store (a reload) has the shipped file again"
        );
    }

    #[test]
    fn the_preview_levels_are_the_embedded_levels_in_order() {
        let stems: Vec<&str> = scenario::assets::EMBEDDED_LEVELS
            .iter()
            .map(|(rel, _)| {
                rel.strip_prefix("TC/openliero/Levels/")
                    .and_then(|r| r.strip_suffix(".lev"))
                    .expect("a config-root level path")
            })
            .collect();
        assert_eq!(stems, crate::web_params::LEVELS);
    }

    #[test]
    fn a_config_root_keeps_the_system_data_root_for_the_shadow_check() {
        // Plan D10 (fact 14): C++ ShadowsSystem consults SystemDataRoot() afresh —
        // OPENLIERO_DATADIR when it is a directory, else the install data, else nothing.
        let root = Path::new("/r");
        let spec = |e: &dyn Fn(&str) -> Option<String>, present: &[&'static str]| {
            let present: Vec<&'static str> = present.to_vec();
            resolve_store(
                Some(root),
                PrefOs::Unix,
                e,
                &move |p: &Path| present.iter().any(|d| Path::new(d) == p),
                Path::new(DATA),
            )
        };
        let single = |shadow: Option<&str>| StoreSpec::SingleDir {
            root: root.to_path_buf(),
            shadow: shadow.map(PathBuf::from),
        };
        let with_datadir = env(&[("OPENLIERO_DATADIR", "/sys")]);
        assert_eq!(spec(&with_datadir, &["/sys", DATA]), single(Some("/sys")));
        assert_eq!(
            spec(&with_datadir, &[DATA]),
            single(Some(DATA)),
            "a missing OPENLIERO_DATADIR falls back to the install data"
        );
        assert_eq!(spec(&env(&[]), &[DATA]), single(Some(DATA)), "unset");
        assert_eq!(
            spec(&env(&[]), &[]),
            single(None),
            "no data directory at all"
        );
    }

    #[test]
    fn a_config_root_store_refuses_the_shipped_names_unless_it_is_the_data() {
        let data = PathBuf::from(scenario::paths::DATA_ROOT);
        let scratch = scratch("shadow");
        let copy = StoreSpec::SingleDir {
            root: scratch.clone(),
            shadow: Some(data.clone()),
        }
        .open();
        assert!(
            copy.shadows_system("Setups", "orbmit.cfg"),
            "the install has it"
        );
        assert!(!copy.shadows_system("Setups", "mine.cfg"));
        assert!(copy.shadows_system("Setups", "liero.cfg"), "reserved");
        let itself = StoreSpec::SingleDir {
            root: data.clone(),
            shadow: Some(data),
        }
        .open();
        assert!(
            !itself.shadows_system("Setups", "orbmit.cfg"),
            "the root is the system data: nothing separate to shadow"
        );
        let bare = StoreSpec::SingleDir {
            root: scratch,
            shadow: None,
        }
        .open();
        assert!(!bare.shadows_system("Setups", "orbmit.cfg"));
        assert!(bare.shadows_system("Setups", "liero.cfg"));
    }

    /// A scratch directory under the system temp dir, removed first.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openliero-b8-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_cpp_liero_cfg_with_unequal_health_boots_plays_and_is_saved_back() {
        // Q3: Rust boots from the C++ user's liero.cfg. Unequal health boots with each worm at
        // its own max and NEW GAME starts the match (4½f-1 T3: the refusal is gone); the exit
        // save writes the user layer only.
        use ui::shell::level_slot::SeedSource;
        use ui::shell::playing::StartOptions;
        use ui::shell::{InputEvent, KeyEvent, Shell, ShellInput};

        let root = scratch("health");
        let user = root.join("user");
        let mut cpp = Settings::default();
        cpp.worm_settings[1].health = 50;
        cpp.lives = 9;
        std::fs::create_dir_all(user.join("Setups")).unwrap();
        std::fs::write(
            user.join("Setups/liero.cfg"),
            scenario::settings_toml::settings_to_toml(&cpp),
        )
        .unwrap();
        let data = Path::new(scenario::paths::DATA_ROOT);
        let spec = StoreSpec::Split {
            user: user.clone(),
            system: Some(data.to_path_buf()),
        };
        let store = spec.open();
        let settings = load_settings(&*store);
        assert_eq!(settings, cpp, "the user layer wins over the shipped one");

        let (mut sh, mut sim, _) = Shell::boot(
            Path::new(scenario::paths::TC_ROOT),
            settings,
            store,
            SeedSource::Fixed(5),
            0,
            StartOptions::default(),
        );
        let key = |dos, down| {
            InputEvent::Key(KeyEvent {
                dos,
                down,
                repeat: false,
                typed: ui::keys::TypedKey::Sym(0),
            })
        };
        fn sh_frame(sh: &mut Shell, sim: &mut sim::state::SimState, events: &[InputEvent]) {
            sh.frame(
                sim,
                &ShellInput {
                    events,
                    ..ShellInput::idle()
                },
            );
        }
        for _ in 0..40 {
            sh_frame(&mut sh, &mut sim, &[]);
        }
        sh_frame(&mut sh, &mut sim, &[key(ui::keys::DK_RETURN, true)]);
        sh_frame(&mut sh, &mut sim, &[key(ui::keys::DK_RETURN, false)]);
        assert_eq!(sh.top_refusal(), None, "no refusal");
        for _ in 0..300 {
            if sh.current().is_some() {
                break;
            }
            sh_frame(&mut sh, &mut sim, &[]);
        }
        assert!(sh.current().is_some(), "the match started");
        assert_eq!(
            (sim.worms[0].max_health, sim.worms[1].max_health),
            (100, 50),
            "each worm at its own max"
        );

        sh.settings_mut().lives = 11;
        sh.save_on_exit().unwrap();
        let saved = std::fs::read_to_string(user.join("Setups/liero.cfg")).unwrap();
        let back = scenario::settings_toml::settings_from_toml(&saved).unwrap();
        assert_eq!((back.lives, back.worm_settings[1].health), (11, 50));
        let shipped = std::fs::read_to_string(data.join("Setups/liero.cfg")).unwrap();
        assert!(
            !shipped.contains("lives = 11"),
            "the system layer is never written"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
