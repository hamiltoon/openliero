//! Preview-build match parameters (the "mini" playable web build).
//!
//! A PR preview needs to reach the feature under test without menus (4½d) or a
//! weapon-selection phase (4½c), so the page URL picks the loadout, level and
//! seed of the live default match:
//!
//! ```text
//! ?weapons=MISSILE,LASER,BIG%20NUKE&level=water_stage&seed=7
//! ?demo            (the old scripted `blood` demo instead of a live match)
//! ```
//!
//! This module is Bevy-free and target-independent so it is unit-tested natively;
//! `main.rs` feeds it `window.location.search` on wasm. Nothing here touches the
//! simulation after tick 0: [`MatchParams::scenario_text`] only chooses the start
//! scenario and [`apply_weapons`] only rewrites the tick-0 loadout, so a preview
//! match is exactly as deterministic as the default match.

use sim::state::{NUM_WEAPONS, SimState, WormWeapon};

/// The level used when no (or an unknown) `level=` is given — the default
/// match's level.
pub const DEFAULT_LEVEL: &str = "render_stage";

/// Levels a preview may pick: the stem of each `Levels/<stem>.lev` embedded in
/// the wasm build (`scenario::assets`). `modern_test` (1.2 MB) is left out to
/// keep the download small.
pub const LEVELS: [&str; 4] = [
    "render_stage",
    "water_stage",
    "see_shadow_test",
    "physics_fall_test",
];

/// The default match's seed (`scenarios/default_match.txt`).
pub const DEFAULT_SEED: u32 = 42;

/// Parsed URL parameters. Unknown keys are ignored; bad values fall back to the
/// default match and are reported in [`MatchParams::warnings`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchParams {
    /// `demo`: play the scripted `blood` demo instead of a live match.
    pub demo: bool,
    /// `weapons=`: up to five weapon names for slots 0..4, applied to both worms.
    pub weapons: Vec<String>,
    /// `level=`: one of [`LEVELS`].
    pub level: Option<String>,
    /// `seed=`: the match seed.
    pub seed: Option<u32>,
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

    /// The live match's start scenario. The worms start dead at (0,0) and
    /// respawn in-sim at a free spot, as a real C++ match starts (and as the
    /// default match does), so no level needs hand-picked spawn points and the
    /// camera follows them (it only follows a worm with `killed_timer <= 0`).
    pub fn scenario_text(&self) -> String {
        let level = self.level.as_deref().unwrap_or(DEFAULT_LEVEL);
        let seed = self.seed.unwrap_or(DEFAULT_SEED);
        format!(
            "seed {seed}\nlevel Levels/{level}.lev\nticks 0\n\
             worm 0 0 0 100 10 0   0\nworm 1 0 0 100 10 218 0\nweapon 0 DART\n"
        )
    }
}

/// Rewrite both worms' tick-0 loadout: slot `i` gets `names[i]` (matched against
/// the TC weapon table, case-insensitively) with that weapon's starting ammo,
/// exactly as `Worm::InitWeapons` would. Slots beyond `names` keep their default.
/// Returns the names that matched no weapon (their slots are left unchanged).
pub fn apply_weapons(state: &mut SimState, names: &[String]) -> Vec<String> {
    let mut unknown = Vec::new();
    for (slot, name) in names.iter().enumerate().take(NUM_WEAPONS) {
        let Some(id) = state
            .weapons
            .iter()
            .position(|w| w.name.eq_ignore_ascii_case(name))
        else {
            unknown.push(name.clone());
            continue;
        };
        let ammo = state.weapons[id].ammo;
        for worm in &mut state.worms {
            worm.weapons[slot] = WormWeapon {
                ty: Some(id as _),
                ammo,
                delay_left: 0,
                loading_left: 0,
            };
        }
    }
    unknown
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
    fn empty_query_is_the_default_match() {
        for q in ["", "?"] {
            let p = MatchParams::parse(q);
            assert_eq!(p, MatchParams::default());
            let text = p.scenario_text();
            assert!(text.contains("seed 42\n"), "{text}");
            assert!(text.contains("level Levels/render_stage.lev\n"), "{text}");
            assert!(text.contains("worm 0 0 0 100 10 0   0\n"), "{text}");
        }
    }

    #[test]
    fn default_match_text_matches_the_fixture() {
        // The URL-less preview must be the same match as `cargo run -p game`.
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scenarios/default_match.txt"
        ));
        let a = scenario::Scenario::parse(fixture).expect("fixture parses");
        let b = scenario::Scenario::parse(&MatchParams::default().scenario_text())
            .expect("generated text parses");
        assert_eq!(a, b);
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
        let text = p.scenario_text();
        assert!(
            text.contains("seed 7\nlevel Levels/water_stage.lev\n"),
            "{text}"
        );
        assert!(
            text.contains("worm 0 0 0 100 10 0   0\n"),
            "respawn in-sim: {text}"
        );
        scenario::Scenario::parse(&text).expect("generated text parses");
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
        let scn = scenario::Scenario::parse(&MatchParams::default().scenario_text()).unwrap();
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
}
