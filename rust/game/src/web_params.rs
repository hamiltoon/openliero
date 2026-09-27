//! Preview-build match parameters (the "mini" playable web build).
//!
//! Since Step 4½d a bare preview opens the C++ main menu (`ui::shell::Shell`), like a bare
//! `cargo run -p game`. The page URL can instead jump straight to a match with a chosen loadout,
//! level and seed — those parameters skip the menu (John's Q4 ruling), unless `menu=1` forces it:
//!
//! ```text
//! ?weapons=MISSILE,LASER,BIG%20NUKE&level=water_stage&seed=7
//! ?level=water_stage&menu=1   (the main menu, over the water level)
//! ?demo            (the old scripted `blood` demo instead of a live match)
//! ```
//!
//! Every NEW GAME starts like C++ NEW GAME (`ui::shell`): a generated level, a fresh seed, then
//! weapon selection (4½c). `weapons=` skips selection (John's Q3 ruling) and loads the named
//! weapons; `level=` loads a stock level instead of generating one; `seed=` fixes the seed.
//! With `menu=1` they configure the boot level and every NEW GAME from the menu.
//!
//! This module is Bevy-free and target-independent so it is unit-tested natively;
//! `main.rs` feeds it `window.location.search` on wasm. Nothing here touches the
//! simulation after tick 0: [`MatchParams::level_file`] and [`MatchParams::seed`] only choose
//! the start and [`apply_weapons`] only rewrites the tick-0 loadout, so a preview match is
//! exactly as deterministic as any other match with the same seed.

use sim::state::NUM_WEAPONS;

pub use ui::shell::loadout::apply_weapons;

/// Levels a preview may pick: the stem of each `Levels/<stem>.lev` embedded in
/// the wasm build (`scenario::assets::EMBEDDED_LEVELS`, in its order; `config`'s tests pin
/// it). `modern_test` (1.2 MB) is left out to keep the download small (ruling Q6).
pub const LEVELS: [&str; 4] = [
    "render_stage",
    "water_stage",
    "see_shadow_test",
    "physics_fall_test",
];

/// Parsed URL parameters. Unknown keys are ignored; bad values fall back to the
/// NEW GAME defaults and are reported in [`MatchParams::warnings`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchParams {
    /// `demo`: play the scripted `blood` demo instead of a live match.
    pub demo: bool,
    /// `weapons=`: up to five weapon names for slots 0..4, applied to both worms.
    pub weapons: Vec<String>,
    /// `level=`: one of [`LEVELS`] (default: a generated level).
    pub level: Option<String>,
    /// `seed=`: the match seed (default: a fresh one per match).
    pub seed: Option<u32>,
    /// `menu`: force the main menu even with `weapons`/`level`/`seed` (Step 4½d, Q4).
    pub menu: bool,
    /// Human-readable notes about ignored values (logged to the console).
    pub warnings: Vec<String>,
}

impl MatchParams {
    /// Parse a URL query string (`?a=b&c`, the leading `?` optional).
    pub fn parse(query: &str) -> MatchParams {
        let mut p = MatchParams::default();
        let query = query.strip_prefix('?').unwrap_or(query);
        for pair in query.split('&').filter(|s| !s.is_empty()) {
            let (key, value) = match pair.split_once('=') {
                Some((k, v)) => (decode(k), decode(v)),
                None => (decode(pair), String::new()),
            };
            match key.as_str() {
                "demo" => p.demo = value.is_empty() || value == "1" || value == "true",
                "menu" => p.menu = value.is_empty() || value == "1" || value == "true",
                "weapons" => {
                    p.weapons = value
                        .split(',')
                        .map(|w| w.trim().to_uppercase())
                        .filter(|w| !w.is_empty())
                        .collect();
                    if p.weapons.len() > NUM_WEAPONS {
                        p.warnings.push(format!(
                            "weapons: only the first {NUM_WEAPONS} of {} are used",
                            p.weapons.len()
                        ));
                        p.weapons.truncate(NUM_WEAPONS);
                    }
                }
                "level" => {
                    let stem = value.trim().trim_end_matches(".lev").to_string();
                    if LEVELS.contains(&stem.as_str()) {
                        p.level = Some(stem);
                    } else {
                        p.warnings.push(format!(
                            "level: unknown {stem:?} (available: {})",
                            LEVELS.join(", ")
                        ));
                    }
                }
                "seed" => match value.trim().parse::<u32>() {
                    Ok(s) => p.seed = Some(s),
                    Err(_) => p.warnings.push(format!("seed: not a number: {value:?}")),
                },
                _ => {}
            }
        }
        p
    }

    /// The stock level file `level=` names, in the canonical config-root form the C++ level
    /// selector saves (Step 4½e-2, design §7.5): `<root_label>/TC/openliero/Levels/<stem>.lev`,
    /// which the NEW GAME start loads instead of generating a level (C++ `random_level = false`
    /// + `level_file`) and LEVEL opens on. `None`: generate one.
    pub fn level_file(&self, root_label: &str) -> Option<String> {
        self.level
            .as_ref()
            .map(|stem| format!("{root_label}/TC/openliero/Levels/{stem}.lev"))
    }

    /// Step 4½e-1: `?level=` on the loaded settings (the in-memory copy only; URL parameters are
    /// never saved, design §7.5): a stock level instead of a generated one. Step 4½e-2: the path
    /// is the canonical one under the store's `root_label` (`level_path` rule 1), so the level
    /// selector's restore finds it.
    pub fn apply_level(&self, s: &mut scenario::settings::Settings, root_label: &str) {
        if let Some(file) = self.level_file(root_label) {
            s.random_level = false;
            s.level_file = file;
        }
    }

    /// Q3 (Step 4½c): `?weapons=` naming at least one weapon skips weapon selection — the
    /// preview reaches the thing under test in one click. Without it the match opens on the
    /// selection screen.
    pub fn skips_weapon_selection(&self) -> bool {
        !self.weapons.is_empty()
    }

    /// Q4 (John, 2026-09-26): `?weapons=`, `?level=` or `?seed=` skip the main menu, so every
    /// earlier preview link behaves as before; a plain link opens the menu; `?menu=1` forces it
    /// (the parameters then configure the boot level and every NEW GAME).
    pub fn skips_menu(&self) -> bool {
        !self.menu && (!self.weapons.is_empty() || self.level.is_some() || self.seed.is_some())
    }
}

/// Percent-decode a query component (`+` is a space, as in form encoding).
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_is_a_plain_new_game() {
        // No parameter: a generated level, a fresh seed, weapon selection (as `cargo run -p game`).
        for q in ["", "?"] {
            let p = MatchParams::parse(q);
            assert_eq!(p, MatchParams::default());
            assert_eq!((p.level_file("/openliero"), p.seed), (None, None));
            assert!(!p.skips_weapon_selection());
        }
    }

    #[test]
    fn parses_weapons_level_seed_and_demo() {
        let p =
            MatchParams::parse("?weapons=missile,Big%20Nuke,+laser+&level=water_stage&seed=7&demo");
        assert_eq!(p.weapons, ["MISSILE", "BIG NUKE", "LASER"]);
        assert_eq!(p.level.as_deref(), Some("water_stage"));
        assert_eq!(p.seed, Some(7));
        assert!(p.demo);
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        assert_eq!(
            p.level_file("/openliero").as_deref(),
            Some("/openliero/TC/openliero/Levels/water_stage.lev")
        );
    }

    #[test]
    fn level_overrides_the_loaded_settings_only_when_given() {
        use scenario::settings::Settings;
        let loaded = Settings {
            lives: 7,
            ..Settings::default()
        };
        let mut s = loaded.clone();
        MatchParams::parse("?seed=3").apply_level(&mut s, "/openliero");
        assert_eq!(s, loaded, "no ?level=: the loaded setup as it is");
        MatchParams::parse("?level=water_stage").apply_level(&mut s, "/openliero");
        assert_eq!(
            (s.random_level, s.level_file.as_str(), s.lives),
            (false, "/openliero/TC/openliero/Levels/water_stage.lev", 7),
            "the canonical config-root path the C++ level selector saves (design §7.5)"
        );
    }

    #[test]
    fn the_canonical_level_reads_through_the_browser_store() {
        use scenario::storage::ConfigStore;
        let store = crate::config::browser_store();
        let mut s = scenario::settings::Settings::default();
        MatchParams::parse("?level=water_stage").apply_level(&mut s, store.root_label());
        let tc = std::path::Path::new(scenario::paths::TC_ROOT);
        let level = ui::shell::level_path::read_level(&store, tc, &s.level_file, false)
            .expect("the embedded water_stage");
        let shipped = std::fs::read(tc.join("Levels/water_stage.lev")).unwrap();
        assert_eq!(level, assets::level::load(&shipped).unwrap());
    }

    #[test]
    fn level_opens_on_the_url_level() {
        // `?level=water_stage&menu=1`: the boot level is the file, and LEVEL opens in Levels
        // with the cursor on it (the selector's restore, fileSelectorState.cpp:73-103).
        use scenario::storage::ConfigStore;
        use ui::keys::{DK_F7, DK_RETURN, TypedKey};
        use ui::shell::level_slot::SeedSource;
        use ui::shell::playing::StartOptions;
        use ui::shell::{InputEvent, KeyEvent, Shell, ShellInput};

        let p = MatchParams::parse("?level=water_stage&menu=1");
        assert!(!p.skips_menu());
        let store = crate::config::browser_store();
        let mut settings = crate::config::load_settings(&store);
        p.apply_level(&mut settings, store.root_label());
        let (mut sh, mut sim, _) = Shell::boot(
            std::path::Path::new(scenario::paths::TC_ROOT),
            settings,
            Box::new(store),
            SeedSource::Fixed(5),
            0,
            StartOptions::default(),
        );
        assert!(
            sh.level_from_file(),
            "the boot level is water_stage, not generated"
        );
        let mut frame = |sh: &mut Shell, dos: Option<u32>| {
            let events: Vec<InputEvent> = dos
                .into_iter()
                .map(|dos| {
                    InputEvent::Key(KeyEvent {
                        dos,
                        down: true,
                        repeat: false,
                        typed: TypedKey::Sym(0),
                    })
                })
                .collect();
            sh.frame(
                &mut sim,
                &ShellInput {
                    events: &events,
                    ..ShellInput::idle()
                },
            );
            if let Some(dos) = dos {
                let up = [InputEvent::Key(KeyEvent {
                    dos,
                    down: false,
                    repeat: false,
                    typed: TypedKey::Sym(0),
                })];
                sh.frame(
                    &mut sim,
                    &ShellInput {
                        events: &up,
                        ..ShellInput::idle()
                    },
                );
            }
        };
        for _ in 0..40 {
            frame(&mut sh, None);
        }
        frame(&mut sh, Some(DK_F7));
        sh.settings_menu_mut()
            .move_to_id(ui::shell::settings_menu::SI_LEVEL);
        frame(&mut sh, Some(DK_RETURN));
        let view = sh.selector_view().expect("the level selector is up");
        assert_eq!(
            (view.top, view.folder.as_str(), view.selection),
            ('L', "/openliero/TC/openliero/Levels", 3),
            "physics_fall_test, render_stage, see_shadow_test, water_stage"
        );
    }

    #[test]
    fn weapons_skips_weapon_selection() {
        // Q3 (John): a preview link with ?weapons= skips selection.
        assert!(!MatchParams::parse("").skips_weapon_selection());
        assert!(!MatchParams::parse("?level=water_stage&seed=3").skips_weapon_selection());
        assert!(MatchParams::parse("?weapons=missile").skips_weapon_selection());
        assert!(
            MatchParams::parse("?weapons=NOPE").skips_weapon_selection(),
            "a named loadout"
        );
        assert!(
            !MatchParams::parse("?weapons=").skips_weapon_selection(),
            "names nothing"
        );
    }

    #[test]
    fn bad_values_fall_back_with_a_warning() {
        let p = MatchParams::parse("level=nowhere&seed=abc&weapons=a,b,c,d,e,f");
        assert_eq!((p.level.clone(), p.seed), (None, None));
        assert_eq!(p.weapons.len(), NUM_WEAPONS);
        assert_eq!(p.warnings.len(), 3, "{:?}", p.warnings);
    }

    #[test]
    fn decode_handles_plus_percent_and_malformed_escapes() {
        assert_eq!(decode("BIG+NUKE"), "BIG NUKE");
        assert_eq!(decode("BIG%20NUKE"), "BIG NUKE");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }

    #[test]
    fn apply_weapons_sets_both_worms_and_reports_unknown_names() {
        let tc = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero"
        ));
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scenarios/default_match.txt"
        ));
        let scn = scenario::Scenario::parse(fixture).unwrap();
        let mut state = scenario::load(tc, &scn).state;
        let names: Vec<String> = ["MISSILE", "big nuke", "NOPE"].map(String::from).to_vec();
        let before = state.worms[0].weapons[2];
        let unknown = apply_weapons(&mut state, &names);
        assert_eq!(unknown, ["NOPE"]);
        for worm in &state.worms {
            for (slot, want) in [(0, "MISSILE"), (1, "BIG NUKE")] {
                let ty = worm.weapons[slot].ty.expect("slot filled") as usize;
                assert_eq!(state.weapons[ty].name, want);
                assert_eq!(worm.weapons[slot].ammo, state.weapons[ty].ammo);
            }
        }
        assert_eq!(
            state.worms[0].weapons[2], before,
            "an unknown name leaves its slot"
        );
    }

    #[test]
    fn match_parameters_skip_the_menu_unless_menu_is_forced() {
        // Q4 (John, 2026-09-26): a plain link opens the menu; weapons/level/seed skip it.
        assert!(!MatchParams::parse("").skips_menu());
        assert!(!MatchParams::parse("?touch=1").skips_menu());
        for q in ["?weapons=BAZOOKA", "?level=water_stage", "?seed=7"] {
            assert!(MatchParams::parse(q).skips_menu(), "{q}");
            let forced = MatchParams::parse(&format!("{q}&menu=1"));
            assert!(forced.menu && !forced.skips_menu(), "{q}&menu=1");
        }
        assert!(MatchParams::parse("?menu").menu);
    }
}
