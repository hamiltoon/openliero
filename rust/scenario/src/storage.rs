//! Step 4½a-2 — settings storage (design §3.5, §9.3.7): the [`ConfigStore`] seam; the native
//! [`NativeStore`] mirroring C++ `paths::Resolve` (`filesystem.cpp:767-845`: reads see the
//! per-user directory layered over the read-only system `data/`, writes go to the user
//! directory only); the in-memory [`MemoryStore`] (unit tests, and the wasm stand-in until
//! 4½h's localStorage store implements the same trait); and [`load_setup`] / [`save_setup`]
//! (`gameEntry.cpp:55-58`, `:78`).
//!
//! A config path is a forward-slash name under the config root — `"Setups/liero.cfg"`,
//! `"Profiles/AI (L).toml"` — the C++ `configNode / "Setups" / "liero.cfg"`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::settings::Settings;
use crate::settings_toml::{settings_from_toml, settings_to_toml};

/// The setup the game reads at startup and writes at exit (`gameEntry.cpp:55`, `:78`).
pub const SETUP_REL: &str = "Setups/liero.cfg";
/// `paths::UserDataRoot`'s test-only override (`filesystem.cpp:658-667`).
pub const TEST_USER_DIR_ENV: &str = "OPENLIERO_TEST_USER_DIR";
/// The `SDL_GetPrefPath` organisation and application (`filesystem.cpp:669`).
pub const PREF_ORG: &str = "openliero";
pub const PREF_APP: &str = "openliero";

/// Where settings files are read from and written to.
pub trait ConfigStore {
    /// The file at `rel` through the merged view: the user layer, else the system layer.
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
    /// Write `rel` into the user layer (never the system layer), creating parent directories.
    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()>;
    /// `paths::ShadowsSystem`: would saving `subdir/leaf` clobber a reserved name or hide a
    /// shipped file? (4½e's Save As dialogs refuse such names.)
    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool;
}

/// `ShadowsSystem`'s reserved names (`filesystem.cpp:740-747`): `Setups/liero.cfg`, the game's
/// own auto-write target — the subdir compared exactly, the leaf case-insensitively.
pub fn is_reserved(subdir: &str, leaf: &str) -> bool {
    subdir == "Setups" && leaf.eq_ignore_ascii_case("liero.cfg")
}

/// `rel` under `root`, or `None` for a path that is empty, absolute, drive-qualified,
/// backslashed, or has an empty, `.` or `..` component — nothing may leave the config root.
fn under(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return None;
    }
    let mut path = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

fn invalid(rel: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("invalid config path {rel:?}"),
    )
}

/// Which SDL3 `SDL_GetPrefPath` rule applies (`src/filesystem/{cocoa,unix,windows}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefOs {
    MacOs,
    Unix,
    Windows,
    Other,
}

impl PrefOs {
    /// The rule for the platform this was compiled for.
    pub fn current() -> PrefOs {
        if cfg!(target_os = "macos") {
            PrefOs::MacOs
        } else if cfg!(target_os = "windows") {
            PrefOs::Windows
        } else if cfg!(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        )) {
            PrefOs::Unix
        } else {
            PrefOs::Other
        }
    }
}

/// `SDL_GetPrefPath(org, app)` rebuilt from environment variables, with SDL's trailing
/// separator (design §9.3.7): macOS `$HOME/Library/Application Support/org/app/`; Linux/BSD
/// `$XDG_DATA_HOME/org/app/`, else `$HOME/.local/share/org/app/`; Windows
/// `%APPDATA%\org\app\`; anything else `None`. A missing or empty variable counts as unset.
pub fn pref_path_for(
    os: PrefOs,
    env: &dyn Fn(&str) -> Option<String>,
    org: &str,
    app: &str,
) -> Option<String> {
    let var = |key: &str| env(key).filter(|v| !v.is_empty());
    match os {
        PrefOs::MacOs => {
            let home = var("HOME")?;
            Some(format!(
                "{}/Library/Application Support/{org}/{app}/",
                home.trim_end_matches('/')
            ))
        }
        PrefOs::Unix => {
            let base = match var("XDG_DATA_HOME") {
                Some(xdg) => xdg.trim_end_matches('/').to_string(),
                None => format!("{}/.local/share", var("HOME")?.trim_end_matches('/')),
            };
            Some(format!("{base}/{org}/{app}/"))
        }
        PrefOs::Windows => {
            let appdata = var("APPDATA")?;
            Some(format!(
                "{}\\{org}\\{app}\\",
                appdata.trim_end_matches('\\')
            ))
        }
        PrefOs::Other => None,
    }
}

/// [`pref_path_for`] for this platform and the process environment.
pub fn pref_path(org: &str, app: &str) -> Option<PathBuf> {
    pref_path_for(
        PrefOs::current(),
        &|key: &str| std::env::var(key).ok(),
        org,
        app,
    )
    .map(PathBuf::from)
}

/// The native store: `paths::Resolve`'s XDG split (`filesystem.cpp:833-845`) or one directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStore {
    user_root: PathBuf,
    system_root: Option<PathBuf>,
}

impl NativeStore {
    /// Reads: `user_root`, then `system_root`. Writes: `user_root`.
    pub fn split(user_root: PathBuf, system_root: Option<PathBuf>) -> NativeStore {
        NativeStore {
            user_root,
            system_root,
        }
    }

    /// One directory for reads and writes — the `--config-root` / `portable.txt` layout
    /// (`filesystem.cpp:811-816`, `:827-831`; both still deferred, design §9.3.7).
    pub fn single_dir(root: PathBuf) -> NativeStore {
        NativeStore::split(root, None)
    }

    /// `paths::Resolve` without flags: the per-user directory (`OPENLIERO_TEST_USER_DIR`, else
    /// `SDL_GetPrefPath("openliero", "openliero")`) over [`crate::paths::DATA_ROOT`]. `None`
    /// when no user directory can be determined.
    pub fn resolve_default() -> Option<NativeStore> {
        NativeStore::resolve_with(PrefOs::current(), &|key: &str| std::env::var(key).ok())
    }

    /// [`NativeStore::resolve_default`] over an explicit platform and environment.
    pub fn resolve_with(os: PrefOs, env: &dyn Fn(&str) -> Option<String>) -> Option<NativeStore> {
        let user = match env(TEST_USER_DIR_ENV).filter(|dir| !dir.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(pref_path_for(os, env, PREF_ORG, PREF_APP)?),
        };
        Some(NativeStore::split(
            user,
            Some(PathBuf::from(crate::paths::DATA_ROOT)),
        ))
    }

    pub fn user_root(&self) -> &Path {
        &self.user_root
    }

    pub fn system_root(&self) -> Option<&Path> {
        self.system_root.as_deref()
    }
}

impl ConfigStore for NativeStore {
    /// `FsNodeJoin::TryToReader` (`filesystem.cpp:372-378`): the user file if it reads, else
    /// the system one.
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(under(&self.user_root, rel)?)
            .ok()
            .or_else(|| std::fs::read(under(self.system_root.as_ref()?, rel)?).ok())
    }

    /// `FsNodeFilesystem::TryToWriter` (`filesystem.cpp:572-584`) on the user node: parent
    /// directories are created.
    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()> {
        let path = under(&self.user_root, rel).ok_or_else(|| invalid(rel))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)
    }

    /// `paths::ShadowsSystem` (`filesystem.cpp:736-763`).
    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool {
        if is_reserved(subdir, leaf) {
            return true;
        }
        match &self.system_root {
            // Single-directory layouts have no separate layer to shadow (`:754-759`).
            Some(system) if *system != self.user_root => {
                under(system, &format!("{subdir}/{leaf}")).is_some_and(|p| p.exists())
            }
            _ => false,
        }
    }
}

/// An in-memory store: a writable user layer over a fixed system layer.
#[derive(Debug, Default)]
pub struct MemoryStore {
    user: RefCell<BTreeMap<String, Vec<u8>>>,
    system: BTreeMap<String, Vec<u8>>,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore::default()
    }

    /// A store whose read-only system layer holds `files`.
    pub fn with_system(files: &[(&str, &[u8])]) -> MemoryStore {
        MemoryStore {
            user: RefCell::default(),
            system: files
                .iter()
                .map(|(rel, bytes)| (rel.to_string(), bytes.to_vec()))
                .collect(),
        }
    }

    /// The user layer's copy of `rel` (what a write left there), for tests.
    pub fn user_file(&self, rel: &str) -> Option<Vec<u8>> {
        self.user.borrow().get(rel).cloned()
    }
}

impl ConfigStore for MemoryStore {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        self.user
            .borrow()
            .get(rel)
            .cloned()
            .or_else(|| self.system.get(rel).cloned())
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()> {
        self.user
            .borrow_mut()
            .insert(rel.to_string(), bytes.to_vec());
        Ok(())
    }

    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool {
        is_reserved(subdir, leaf) || self.system.contains_key(&format!("{subdir}/{leaf}"))
    }
}

/// `gameEntry.cpp:55-58`: read `Setups/liero.cfg` through the merged view. When it is missing
/// or does not parse (invalid UTF-8 is a toml++ parse error too), fall back to
/// `Settings::default()` AND write those defaults to the user layer
/// (`gfx.SaveSettings(userConfigNode / "Setups" / "liero.cfg")`).
pub fn load_setup(store: &dyn ConfigStore) -> io::Result<Settings> {
    let parsed = store
        .read(SETUP_REL)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| settings_from_toml(&text).ok());
    match parsed {
        Some(s) => Ok(s),
        None => {
            let s = Settings::default();
            save_setup(store, &s)?;
            Ok(s)
        }
    }
}

/// `gameEntry.cpp:78`: `settings->save(userConfigNode / "Setups" / "liero.cfg")` — the C++
/// bytes ([`settings_to_toml`]) into the user layer.
pub fn save_setup(store: &dyn ConfigStore, s: &Settings) -> io::Result<()> {
    store.write(SETUP_REL, settings_to_toml(s).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::DATA_ROOT;

    /// A fresh per-process scratch directory (the `shot` tests' `temp_dir` + pid pattern).
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("liero_rs_storage_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, bytes).expect("write");
    }

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    fn shipped(rel: &str) -> Vec<u8> {
        std::fs::read(Path::new(DATA_ROOT).join(rel)).expect("shipped file")
    }

    #[test]
    fn pref_path_mirrors_sdl_get_pref_path_per_platform() {
        let home = env(&[("HOME", "/Users/j")]);
        assert_eq!(
            pref_path_for(PrefOs::MacOs, &home, "openliero", "openliero").as_deref(),
            Some("/Users/j/Library/Application Support/openliero/openliero/")
        );
        assert_eq!(
            pref_path_for(PrefOs::Unix, &home, "openliero", "openliero").as_deref(),
            Some("/Users/j/.local/share/openliero/openliero/")
        );
        let xdg = env(&[("HOME", "/home/j"), ("XDG_DATA_HOME", "/data/")]);
        assert_eq!(
            pref_path_for(PrefOs::Unix, &xdg, "o", "a").as_deref(),
            Some("/data/o/a/")
        );
        let win = env(&[("APPDATA", "C:\\Users\\j\\AppData\\Roaming")]);
        assert_eq!(
            pref_path_for(PrefOs::Windows, &win, "openliero", "openliero").as_deref(),
            Some("C:\\Users\\j\\AppData\\Roaming\\openliero\\openliero\\")
        );
        assert_eq!(pref_path_for(PrefOs::Other, &home, "o", "a"), None);
    }

    #[test]
    fn pref_path_treats_missing_and_empty_variables_as_unset() {
        let none = env(&[]);
        for os in [PrefOs::MacOs, PrefOs::Unix, PrefOs::Windows] {
            assert_eq!(pref_path_for(os, &none, "o", "a"), None, "{os:?}");
        }
        let empty = env(&[("HOME", "/home/j"), ("XDG_DATA_HOME", "")]);
        assert_eq!(
            pref_path_for(PrefOs::Unix, &empty, "o", "a").as_deref(),
            Some("/home/j/.local/share/o/a/")
        );
        assert_eq!(
            pref_path_for(PrefOs::MacOs, &env(&[("HOME", "")]), "o", "a"),
            None
        );
    }

    #[test]
    fn resolve_honours_the_test_user_dir_then_the_pref_path() {
        let over = env(&[("HOME", "/Users/j"), ("OPENLIERO_TEST_USER_DIR", "/tmp/u")]);
        let store = NativeStore::resolve_with(PrefOs::MacOs, &over).expect("a store");
        assert_eq!(store.user_root(), Path::new("/tmp/u"));
        assert_eq!(store.system_root(), Some(Path::new(DATA_ROOT)));
        let pref = env(&[("HOME", "/Users/j")]);
        let store = NativeStore::resolve_with(PrefOs::MacOs, &pref).expect("a store");
        assert_eq!(
            store.user_root(),
            Path::new("/Users/j/Library/Application Support/openliero/openliero")
        );
        assert!(NativeStore::resolve_with(PrefOs::MacOs, &env(&[])).is_none());
    }

    #[test]
    fn native_reads_see_the_user_layer_over_the_system_layer() {
        let root = scratch("merged");
        let (user, system) = (root.join("user"), root.join("system"));
        put(&system, "Setups/liero.cfg", b"system setup");
        put(&system, "Profiles/a.toml", b"system a");
        put(&user, "Profiles/a.toml", b"user a");
        let store = NativeStore::split(user, Some(system));
        assert_eq!(
            store.read("Setups/liero.cfg").as_deref(),
            Some(&b"system setup"[..])
        );
        assert_eq!(
            store.read("Profiles/a.toml").as_deref(),
            Some(&b"user a"[..])
        );
        assert_eq!(store.read("Profiles/none.toml"), None);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn native_writes_go_to_the_user_layer_only() {
        let root = scratch("write");
        let (user, system) = (root.join("user"), root.join("system"));
        put(&system, "Setups/liero.cfg", b"system setup");
        let store = NativeStore::split(user.clone(), Some(system.clone()));
        store
            .write("Setups/liero.cfg", b"mine")
            .expect("write creates Setups/");
        assert_eq!(
            std::fs::read(user.join("Setups/liero.cfg")).expect("user file"),
            b"mine"
        );
        assert_eq!(
            std::fs::read(system.join("Setups/liero.cfg")).expect("system file"),
            b"system setup"
        );
        assert_eq!(
            store.read("Setups/liero.cfg").as_deref(),
            Some(&b"mine"[..])
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn native_shadows_system_mirrors_paths_shadows_system() {
        let root = scratch("shadows");
        let (user, system) = (root.join("user"), root.join("system"));
        put(&system, "Profiles/Stock.toml", b"x");
        put(&user, "Profiles/Mine.toml", b"x");
        let split = NativeStore::split(user, Some(system.clone()));
        assert!(split.shadows_system("Setups", "liero.cfg"), "reserved");
        assert!(
            split.shadows_system("Setups", "LIERO.CFG"),
            "reserved, leaf case-insensitive"
        );
        assert!(
            split.shadows_system("Profiles", "Stock.toml"),
            "the system layer has it"
        );
        assert!(
            !split.shadows_system("Profiles", "Mine.toml"),
            "user-only files may be overwritten"
        );
        assert!(!split.shadows_system("Profiles", "New.toml"));
        let single = NativeStore::single_dir(system.clone());
        assert!(
            single.shadows_system("Setups", "liero.cfg"),
            "reserved even in one directory"
        );
        assert!(
            !single.shadows_system("Profiles", "Stock.toml"),
            "no separate layer to shadow"
        );
        let same = NativeStore::split(system.clone(), Some(system));
        assert!(
            !same.shadows_system("Profiles", "Stock.toml"),
            "user root == system root (filesystem.cpp:754-759)"
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn paths_that_could_leave_the_root_are_refused() {
        let root = scratch("refuse");
        let store = NativeStore::single_dir(root.join("cfg"));
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "Setups/../../x",
            "Setups//x",
            "./x",
            "C:/x",
            "a\\b",
        ] {
            assert_eq!(store.read(bad), None, "{bad:?}");
            let err = store.write(bad, b"x").expect_err(bad);
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
        assert!(
            !root.join("x").exists() && !root.join("cfg").exists(),
            "nothing was written"
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn load_setup_reads_the_shipped_setup_through_the_system_layer() {
        let liero = shipped("Setups/liero.cfg");
        let store = MemoryStore::with_system(&[(SETUP_REL, liero.as_slice())]);
        assert_eq!(load_setup(&store).expect("load"), Settings::default());
        assert_eq!(
            store.user_file(SETUP_REL),
            None,
            "a readable setup is not rewritten"
        );
    }

    #[test]
    fn load_setup_writes_the_defaults_when_the_setup_is_missing_or_broken() {
        let defaults = settings_to_toml(&Settings::default()).into_bytes();
        let broken: [Option<&[u8]>; 3] = [None, Some(b"[settings\nlives = "), Some(&[0xff, 0xfe])];
        for bytes in broken {
            let store = MemoryStore::new();
            if let Some(b) = bytes {
                store.write(SETUP_REL, b).expect("seed the user layer");
            }
            assert_eq!(
                load_setup(&store).expect("load"),
                Settings::default(),
                "{bytes:?}"
            );
            assert_eq!(
                store.user_file(SETUP_REL),
                Some(defaults.clone()),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn a_user_setup_shadows_the_shipped_one_and_save_setup_writes_the_cpp_bytes() {
        let liero = shipped("Setups/liero.cfg");
        let store = MemoryStore::with_system(&[(SETUP_REL, liero.as_slice())]);
        store
            .write(SETUP_REL, &shipped("Setups/orbmit.cfg"))
            .expect("seed");
        let mut s = load_setup(&store).expect("load");
        assert_eq!(s.lives, 9, "the user layer's orbmit values win");
        s.map = false;
        save_setup(&store, &s).expect("save");
        assert_eq!(
            store.user_file(SETUP_REL),
            Some(settings_to_toml(&s).into_bytes())
        );
        assert!(!load_setup(&store).expect("reload").map);
    }

    #[test]
    fn memory_store_shadows_system_like_the_native_store() {
        let store = MemoryStore::with_system(&[("Profiles/Stock.toml", &b"x"[..])]);
        assert!(store.shadows_system("Setups", "Liero.CFG"));
        assert!(
            !store.shadows_system("setups", "liero.cfg"),
            "the subdir compare is exact (filesystem.cpp:744)"
        );
        assert!(store.shadows_system("Profiles", "Stock.toml"));
        assert!(!store.shadows_system("Profiles", "Mine.toml"));
    }

    #[test]
    fn load_setup_on_disk_creates_the_user_setup_only_when_needed() {
        let root = scratch("load_setup");
        let with_data = NativeStore::split(root.join("u1"), Some(PathBuf::from(DATA_ROOT)));
        assert_eq!(load_setup(&with_data).expect("load"), Settings::default());
        assert!(
            !root.join("u1/Setups/liero.cfg").exists(),
            "read from data/, nothing written"
        );
        let bare = NativeStore::split(root.join("u2"), Some(root.join("empty")));
        assert_eq!(load_setup(&bare).expect("load"), Settings::default());
        assert_eq!(
            std::fs::read(root.join("u2/Setups/liero.cfg")).expect("the written setup"),
            settings_to_toml(&Settings::default()).into_bytes()
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
