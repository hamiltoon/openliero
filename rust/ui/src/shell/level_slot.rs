//! The router's level and seeds (design §4.11, finding 16; LD 6). `LevelSlot` is C++ `Level` plus
//! its `old_*` provenance (`level.cpp:421-424`); NEW GAME reuses it — as the last match left it,
//! craters and all (`SwapLevel(*old_level)`) — unless `regenerate_level` is set or a level
//! setting changed (`gfx.cpp:1512-1516`). `SeedSource` splits the boot seed (the level behind
//! the menu) from the per-NEW-GAME match seed. Supersedes 4½c's `NewGame` (plan-time fact 17).

use std::collections::VecDeque;
use std::path::Path;

use assets::level::LevelData;
use scenario::settings::Settings;
use sim::state::LevelSim;

use super::new_game::generate_level;

/// `Level::old_random_level / old_level_file / old_random_map_width / old_random_map_height`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelProvenance {
    pub random_level: bool,
    pub level_file: String,
    pub random_map_width: i32,
    pub random_map_height: i32,
}

impl LevelProvenance {
    pub fn of(s: &Settings) -> LevelProvenance {
        LevelProvenance {
            random_level: s.random_level,
            level_file: s.level_file.clone(),
            random_map_width: s.random_map_width,
            random_map_height: s.random_map_height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelSlot {
    pub level: LevelData,
    pub provenance: LevelProvenance,
}

impl LevelSlot {
    /// `Level::GenerateFromSettings(common, settings, rand)` with the level `Rand` seeded from
    /// `seed` (4½b; `new_game::generate_level`), recording the provenance (`level.cpp:421-424`) —
    /// also when a file level failed to load and fell back to random (finding 16).
    pub fn generate(tc_root: &Path, settings: &Settings, seed: u32) -> LevelSlot {
        LevelSlot {
            level: generate_level(tc_root, settings, seed),
            provenance: LevelProvenance::of(settings),
        }
    }

    /// The reuse test (`gfx.cpp:1512-1516`): the width and height count even for a file level.
    pub fn reusable(&self, settings: &Settings) -> bool {
        !settings.regenerate_level && self.provenance == LevelProvenance::of(settings)
    }

    /// `SwapLevel(*old_level)`: keep the level as the last match left it.
    pub fn take_played(&mut self, played: &LevelSim) {
        debug_assert_eq!(
            (played.width, played.height),
            (self.level.width, self.level.height)
        );
        self.level.material_id.clone_from(&played.material_id);
    }
}

/// Where the seeds come from. C++ seeds `gfx.rand` (levels) from the clock at start-up
/// (`gameEntry.cpp:23`) and every `Game` (the sim) from `time(nullptr)` (`game.cpp:42`); Rust
/// takes one boot seed and one seed per NEW GAME (the sim seed and, when the level is
/// generated, the level seed): `Fixed` (`?seed=`), `Fresh` (the caller's clock value), or
/// `Scripted` (the oracle harness; the C++ dumper reseeds at the same points, T8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedSource {
    Fixed(u32),
    Fresh,
    Scripted { boot: u32, matches: VecDeque<u32> },
}

impl SeedSource {
    pub fn boot(&self, fresh: u32) -> u32 {
        match self {
            SeedSource::Fixed(s) => *s,
            SeedSource::Fresh => fresh,
            SeedSource::Scripted { boot, .. } => *boot,
        }
    }

    pub fn next_match(&mut self, fresh: u32) -> u32 {
        match self {
            SeedSource::Fixed(s) => *s,
            SeedSource::Fresh => fresh,
            SeedSource::Scripted { matches, .. } => matches
                .pop_front()
                .expect("a NEW GAME with no scripted match seed left"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scenario::paths::TC_ROOT;

    fn tc() -> &'static Path {
        Path::new(TC_ROOT)
    }

    #[test]
    fn a_slot_records_the_settings_it_was_generated_from() {
        let s = Settings::default();
        let slot = LevelSlot::generate(tc(), &s, 7);
        assert_eq!(slot.level, generate_level(tc(), &s, 7));
        assert!(slot.reusable(&s));
    }

    #[test]
    fn each_of_the_five_fields_forces_a_new_level() {
        let s = Settings::default();
        let slot = LevelSlot::generate(tc(), &s, 7);
        for changed in [
            Settings {
                regenerate_level: true,
                ..s.clone()
            },
            Settings {
                random_level: false,
                ..s.clone()
            },
            Settings {
                level_file: "Levels/water_stage.lev".into(),
                ..s.clone()
            },
            Settings {
                random_map_width: 512,
                ..s.clone()
            },
            Settings {
                random_map_height: 352,
                ..s.clone()
            },
        ] {
            assert!(!slot.reusable(&changed), "gfx.cpp:1512-1516");
        }
    }

    #[test]
    fn a_file_level_is_reloaded_when_the_map_size_changes() {
        // Finding 16: the width/height compare applies even to a file level.
        let s = Settings {
            random_level: false,
            level_file: "Levels/water_stage.lev".into(),
            ..Settings::default()
        };
        let slot = LevelSlot::generate(tc(), &s, 1);
        assert!(slot.reusable(&s));
        assert!(!slot.reusable(&Settings {
            random_map_width: 600,
            ..s
        }));
    }

    #[test]
    fn take_played_keeps_the_craters() {
        let s = Settings::default();
        let mut slot = LevelSlot::generate(tc(), &s, 7);
        let mut played = LevelSim {
            width: slot.level.width,
            height: slot.level.height,
            material_id: slot.level.material_id.clone(),
            material_flags: [0; 256],
        };
        played.material_id[1234] ^= 0xff;
        slot.take_played(&played);
        assert_eq!(slot.level.material_id, played.material_id);
    }

    #[test]
    fn seed_sources() {
        let mut fixed = SeedSource::Fixed(7);
        assert_eq!(
            (fixed.boot(1), fixed.next_match(2), fixed.next_match(3)),
            (7, 7, 7),
            "?seed=7 is 4½c's first match"
        );
        let mut fresh = SeedSource::Fresh;
        assert_eq!((fresh.boot(1), fresh.next_match(2)), (1, 2));
        let mut s = SeedSource::Scripted {
            boot: 5,
            matches: VecDeque::from([8, 9]),
        };
        assert_eq!((s.boot(0), s.next_match(0), s.next_match(0)), (5, 8, 9));
    }
}
