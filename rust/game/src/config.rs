//! Where the live shell keeps `liero.cfg` (Step 4½e-1, T10; plan D10, D11; design §7.1, §7.2).
//!
//! Q3 (John): desktop Rust reads and writes the same config root as C++ OpenLiero, so a player's
//! `Setups/liero.cfg` is shared between the two. The native store follows C++ `paths::Resolve`
//! (`filesystem.cpp:767-845`):
//!
//! - `--config-root <dir>` (or `=<dir>`): one directory for reads and writes (`:812-817`);
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
//! localStorage), with the shipped setups in its system layer (plan D11).

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
    /// `--config-root`: one directory for reads and writes.
    SingleDir(PathBuf),
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
            StoreSpec::SingleDir(root) => Box::new(NativeStore::single_dir(root)),
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
    if let Some(root) = config_root.filter(|r| !r.as_os_str().is_empty()) {
        return StoreSpec::SingleDir(root.to_path_buf());
    }
    let var = |key: &str| env(key).filter(|v| !v.is_empty());
    let user = match var(TEST_USER_DIR_ENV) {
        Some(dir) => PathBuf::from(dir),
        None => match storage::pref_path_for(os, env, PREF_ORG, PREF_APP) {
            Some(p) => PathBuf::from(p),
            None => return StoreSpec::Memory,
        },
    };
    let system = var(DATADIR_ENV)
        .map(PathBuf::from)
        .filter(|d| is_dir(d))
        .or_else(|| is_dir(data_root).then(|| data_root.to_path_buf()));
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

/// The browser's store (plan D11): in memory for the session, root label `/openliero`, with the
/// shipped setups in its system layer.
pub fn browser_store() -> MemoryStore {
    MemoryStore::with_system(scenario::assets::EMBEDDED_SETUPS)
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
            StoreSpec::SingleDir(PathBuf::from("/r"))
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

    /// A scratch directory under the system temp dir, removed first.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openliero-b8-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_cpp_liero_cfg_with_unequal_health_boots_refuses_and_is_saved_back() {
        // Q3: Rust boots from the C++ user's liero.cfg. Unequal health must not crash the boot
        // (plan fact 21); NEW GAME shows the refusal box instead; the exit save writes the
        // user layer only.
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
        assert_eq!(sh.top_char(), 'B');
        assert_eq!(
            sh.top_refusal(),
            Some(&ui::shell::overlay::Refusal::Build(
                scenario::build::BuildError::AsymmetricHealth { p1: 100, p2: 50 }
            ))
        );
        for _ in 0..40 {
            sh_frame(&mut sh, &mut sim, &[]);
            assert!(sh.current().is_none(), "no match started");
        }

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
