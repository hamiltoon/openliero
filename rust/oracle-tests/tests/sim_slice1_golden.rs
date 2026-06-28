//! Differential test for the tick-0 simulation state hash against the C++
//! oracle — the integration crux of Step 2 Slice 1. The golden line is produced
//! by the real C++ `Game` built to the exact tick-0 fixture (seed 42, a LOADED
//! `modern_test.lev`, 2 worms, `InitWeapons`, `ResetWorms`) and dumped BEFORE any
//! `ProcessFrame` (see `src/tools/oracle_dump/sim_dump.cpp`). The Rust
//! `SimState` must reproduce every one of the 13 columns bit-for-bit.
//!
//! Golden columns:
//!   `seed width height state_hash rng level worm0 worm1 bob bon sob nob wob`
//!
//! ## How each worm-init value is sourced (traceable to the C++ fixture)
//!
//! `sim_dump.cpp` builds each worm as the determinism fixture does:
//!   * `w->settings = settings->worm_settings[idx]`  (sim_dump.cpp:91)
//!   * `w->health   = w->settings->health`           (sim_dump.cpp:92)
//!   * `w->InitWeapons(game)`                         (sim_dump.cpp:96)
//!   * `game.ResetWorms()` then sets lives etc.       (sim_dump.cpp:98)
//! with `settings->lives = 10` (sim_dump.cpp:70).
//!
//! The values feeding the hash therefore resolve to C++ defaults:
//!   * health = 100  — `WormSettings::health{100}` default (`worm.hpp:104`); the
//!     worm-settings ctor never overrides it.
//!   * lives  = 10   — `settings->lives = 10` (`sim_dump.cpp:70`); `ResetWorms`
//!     copies `settings->lives` into each worm.
//!   * weapon selection `settings->weapons[j]` = 1 for every slot —
//!     `WormSettings` ctor sets `for (weapon : weapons) weapon = 1`
//!     (`worm.hpp:91-92`). So `InitWeapons` (`worm.cpp:702-708`) resolves EVERY
//!     slot to `weapons[weap_order[1 - 1]]` = `weapons[weap_order[0]]`, i.e. the
//!     same weapon (alphabetically-first name) in all five slots.
//!   * `weap_order` = weapon indices sorted by `weapon.name`, with `id = index`,
//!     reproducing `Common::Precompute` (`common.cpp:492-499`).
//!   * each slot's `ammo`/`id` come from that resolved weapon's parsed `.cfg`
//!     (`InitWeapons`: `ww.ammo = ww.type->ammo`, `ww.type = &weapons[...]`).
//!
//! All other hashed worm fields are zero/empty at tick 0 (pos/vel/aim/kills/
//! timer/visible(false)/control_states(0)/ninjarope) — see `WormState::from_init`.

use assets::object::Objects;
use assets::tc::TcConfig;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{SimState, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

#[test]
fn sim_slice1_tick0_hash_matches_cpp_oracle() {
    // --- Parse the golden line: 13 whitespace-separated columns. -------------
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice1.txt"
    ))
    .expect("read golden/sim_slice1.txt");
    let line = golden.lines().next().expect("golden has a line");
    let mut it = line.split_whitespace();
    let mut next = || it.next().expect("golden column present");
    let seed: u32 = next().parse().expect("seed");
    let want_w: i32 = next().parse().expect("width");
    let want_h: i32 = next().parse().expect("height");
    let hex = |s: &str| u32::from_str_radix(s, 16).expect("hex column");
    let want_state = hex(next());
    let want_rng = hex(next());
    let want_level = hex(next());
    let want_worm0 = hex(next());
    let want_worm1 = hex(next());
    let want_bob = hex(next());
    let want_bon = hex(next());
    let want_sob = hex(next());
    let want_nob = hex(next());
    let want_wob = hex(next());
    assert!(it.next().is_none(), "golden has exactly 13 columns");

    // --- Load the SAME fixed level the C++ dumper loaded. --------------------
    // Path mirrors level_golden.rs; loading (not generating) keeps rand.last==0.
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/Levels/modern_test.lev"))
        .expect("read modern_test.lev");
    let level = assets::level::load(&lev_bytes).expect("level loads");
    assert_eq!(level.width, want_w, "level width");
    assert_eq!(level.height, want_h, "level height");

    // --- Load the real TC weapon table + reproduce weap_order. ---------------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499). C++ std::string `<` and Rust
    // `str::cmp` are both byte-wise lexicographic; weapon names are ASCII.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 (worm.hpp:91-92), so every slot
    // selects order index 0 -> the alphabetically-first weapon.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // Two worms exactly as sim_dump.cpp:87-94 (health=100, lives=10). Slice 2
    // added `start_pos`/`visible` to WormInit; the tick-0 fixture keeps the
    // Slice-1 defaults (origin, invisible) so this golden is unchanged.
    let worms_init = vec![
        WormInit {
            index: 0,
            health: 100,
            lives: 10,
            stats_x: 0,
            weapons: resolved,
            start_pos: Vec2::zero(),
            visible: false,
        },
        WormInit {
            index: 1,
            health: 100,
            lives: 10,
            stats_x: 218,
            weapons: resolved,
            start_pos: Vec2::zero(),
            visible: false,
        },
    ];

    // --- Build tick-0 state and hash it. -------------------------------------
    let state = SimState::new(&level, &worms_init, seed);
    let got_state = hash_game_state(&state);
    let c = hash_components(&state);

    // Assert components FIRST to localize any divergence (debugging ladder).
    let check = |name: &str, got: u32, want: u32| {
        assert_eq!(got, want, "{name}: got {got:08x} expected {want:08x}");
    };
    check("rng", c.rng, want_rng);
    check("level", c.level, want_level);
    check("worm0", c.worms[0], want_worm0);
    check("worm1", c.worms[1], want_worm1);
    check("bobjects", c.bobjects, want_bob);
    check("bonuses", c.bonuses, want_bon);
    check("sobjects", c.sobjects, want_sob);
    check("nobjects", c.nobjects, want_nob);
    check("wobjects", c.wobjects, want_wob);
    // Then the master hash (covers weapons / aiming / kills / pack / ninjarope).
    check("state_hash", got_state, want_state);
}
