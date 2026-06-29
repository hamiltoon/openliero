//! Per-tick differential test for the Slice-4b GREENBALL terrain-destruction
//! lifecycle against the C++ oracle — THE MILESTONE. The golden
//! (`golden/sim_slice4b.txt`, 91 lines for ticks 0..=90) is produced by the real
//! C++ `Game` running the *same scenario* (`golden/sim_slice4b_scenario.txt`):
//! seed 42, the LOADED `physics_fall_test.lev`, worm0 visible and grounded with the
//! **GREENBALL** in weapon slot 0, worm1 invisible and inert. worm0 falls, lands,
//! raises the gun ~25 ticks and FIRES an arcing shot (tick 38) that parabolas back
//! DOWN onto the dirt SURFACE and explodes, where `dirt_effect = 6` ADDS dirt into
//! the Background cells of the 16x16 window. It then lowers the gun and FIRES a
//! SECOND arcing shot (tick 58) that digs a fresh spot.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — terrain destruction goes live
//!
//! This is the FIRST slice whose `level` component column is a NON-CONSTANT time
//! series matched against C++ tick-for-tick. Every earlier slice (incl. 4a, whose
//! fan has `dirt_effect = -1`) folded a PRISTINE level — a constant `level` column
//! can never catch a bug in how terrain destruction is computed or hashed. Here the
//! golden's `level` MOVES (`95f63601` -> `ddd76202` at the shot-1 explode tick ->
//! `63307ba3` at the shot-2 explode tick): three distinct values proving the dirt
//! writes landed. A bit-exact match over 91 ticks proves `draw_dirt_effect` + the
//! `blow_up` dirt branch + the driver wiring reproduce the C++ `material_id` writes
//! bit-for-bit, INCLUDING the explode-tick `rand(2)` ordering and the `-7,-7`
//! large-sprite offset/clip/wrap.
//!
//! The component columns are asserted FIRST (rng -> level -> worm0 -> worm1 ->
//! 5 pools) then the master `state_hash`, so a divergence localises to a tick +
//! subsystem before the master flags it: `level` => the `draw_dirt_effect`
//! blit/clip/wrap or the `-7,-7` offset; `rng` => the explode-tick draw order;
//! `wob` => the projectile path; master-only => aiming/weapons/delay_left.
//!
//! The scenario is the single source of truth (parsed via `oracle_tests::scenario`)
//! and the expected values are PARSED from the golden file, never hard-coded.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`)
/// from `sprites/large.tga`. Threaded into `SimState` for Slice-4b's DrawDirtEffect:
/// the greenball's `dirt_effect = 6` indexes this bank on the explode tick, so an
/// empty bank would panic. This is the first slice that actually reads it.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// The empty-pool component hash (FNV-1a of a zero-length pool). Every pool reads
/// this while idle; `wob` leaving this value is how we know a projectile is live.
const EMPTY_POOL: u32 = 0x0000_0001;

/// One parsed golden line — all 11 columns, master included (asserted this slice).
struct GoldenTick {
    tick: u32,
    master: u32,
    rng: u32,
    level: u32,
    worm0: u32,
    worm1: u32,
    pools: [u32; 5], // bob, bon, sob, nob, wob
}

fn parse_golden(text: &str) -> Vec<GoldenTick> {
    let hex = |s: &str| u32::from_str_radix(s, 16).expect("hex column");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let mut it = line.split_whitespace();
            let mut next = || it.next().expect("golden column present");
            let tick: u32 = next().parse().expect("tick");
            let master = hex(next()); // state_hash: ASSERTED this slice (master gate).
            let rng = hex(next());
            let level = hex(next());
            let worm0 = hex(next());
            let worm1 = hex(next());
            let pools = [hex(next()), hex(next()), hex(next()), hex(next()), hex(next())];
            assert!(it.next().is_none(), "golden line has exactly 11 columns");
            GoldenTick { tick, master, rng, level, worm0, worm1, pools }
        })
        .collect()
}

#[test]
fn sim_slice4b_greenball_terrain_matches_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4b_scenario.txt"
    ))
    .expect("read golden/sim_slice4b_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 90, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=90, master + 9 components). ------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4b.txt"
    ))
    .expect("read golden/sim_slice4b.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 4b HEADLINE (inverted from 4a's pristine-terrain guard): the
    // greenball's `dirt_effect = 6` DIGS, so the `level` column must MOVE. Read
    // straight from the parsed golden: it must take >=3 distinct values (pristine,
    // then one dug value per shot). This is the first golden whose `level` is a
    // live, non-constant time series. (NOT asserted constant — the opposite of 4a.)
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(
        golden_levels.len() >= 3,
        "4b golden must dig: level column takes >=3 distinct values (pristine + 2 shots); saw {:?}",
        golden_levels
    );

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). --------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants. ----------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // INVARIANT (load-bearing for the Fire path): `weapon.id == array index`. The
    // spawned `WObject.ty` is set to `weapon.id` in weapon_fire, and
    // wobject_process indexes `weapons[ty]` — those only line up if id == index.
    // If this fails the weapon lookup would silently read the wrong params, so we
    // STOP here rather than paper over it.
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(
            w.id, i as i32,
            "weapon id must equal its index (weapon[{i}] = {:?}, id {})",
            w.name, w.id
        );
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-4a.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with the GREENBALL, mirroring the C++ dumper's
    // `ResolveWeapon("GREENBALL")`. The name is the *scenario's* `weapon 0` directive
    // (single source of truth) and must match `common->weapons[i].name` exactly —
    // the UPPERCASE "GREENBALL", not the filename/"greenball" (Task 5 proved the
    // lowercase forms do not resolve). The dumper leaves current_weapon = 0 and
    // delay_left/loading_left at 0; `from_init` already gives those defaults.
    let gb_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let gb_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == gb_name)
        .unwrap_or_else(|| panic!("weapon {gb_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(gb_idx as WeaponId),
        ammo: objects.weapons[gb_idx].ammo,
    };

    // --- Build the two worm inits from the scenario. BOTH worms get the GREENBALL
    // in slot 0 (the scenario `weapon 0` overrides both); worm1 is invisible/inert
    // so it never fires, but its 5-slot weapon state still folds into worm1's hash,
    // so the override must apply to it too for the master to match the dumper.
    let worms_init: Vec<WormInit> = scenario
        .worms
        .iter()
        .map(|w| WormInit {
            index: w.index,
            health: w.health,
            lives: w.lives,
            stats_x: w.stats_x,
            weapons: resolved,
            start_pos: Vec2::new(w.pos_x, w.pos_y),
            visible: w.visible,
        })
        .collect();

    // --- Build tick-0 state. Pass the FULL weapon table, the real large-sprite
    // bank and the textures table: `draw_dirt_effect` reads the bank + textures on
    // the explode tick to write dirt into the level. -------------------------
    let large_sprites = load_large_sprites();
    let mut state = SimState::new(
        &level,
        &worms_init,
        scenario.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        large_sprites,
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        100,
        true,
    );

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, pools incl. wobjects)
    // THEN the master, so a divergence localises to a tick + subsystem before the
    // master flags it. `level` here is a live time series, not a constant.
    let assert_tick = |state: &SimState, g: &GoldenTick| {
        let c = hash_components(state);
        check(g.tick, "rng", c.rng, g.rng);
        check(g.tick, "level", c.level, g.level);
        check(g.tick, "worm0", c.worms[0], g.worm0);
        check(g.tick, "worm1", c.worms[1], g.worm1);
        check(g.tick, "bobjects", c.bobjects, g.pools[0]);
        check(g.tick, "bonuses", c.bonuses, g.pools[1]);
        check(g.tick, "sobjects", c.sobjects, g.pools[2]);
        check(g.tick, "nobjects", c.nobjects, g.pools[3]);
        check(g.tick, "wobjects", c.wobjects, g.pools[4]);
        // The MASTER, last: it folds the live fire state, the live wobject (pool
        // hash), AND the dug level (which moves this slice).
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage guard: prove the run actually exercises Fire AND the terrain
    // write (not just flight). All values are read from the DRIVEN SimState, never
    // re-parsed from the golden, so the asserts below are a genuine witness.
    let mut wob_nonempty_ticks = 0usize;
    let mut rng_seen = std::collections::HashSet::new();
    let mut ammo_seen = std::collections::HashSet::new();
    let mut level_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        let c = hash_components(state);
        if c.wobjects != EMPTY_POOL {
            wob_nonempty_ticks += 1;
        }
        rng_seen.insert(c.rng);
        level_seen.insert(c.level);
        for w in &state.worms {
            ammo_seen.insert(w.weapons[0].ammo);
        }
    };
    record(&state);

    // --- Drive each subsequent tick under SCRIPTED input. THE OFF-BY-ONE:
    // golden line `k` (k>=1) is the result of applying input[k-1] on the pass that
    // advances tick k-1 -> k (design doc, *Input timing*). So produce line `k` by
    // calling process_frame with input keyed `k-1`.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // --- Coverage assertions (INVERTED from 4a's pristine guard): the matched run
    // genuinely fired AND destroyed terrain, read from the DRIVEN SimState.
    assert!(
        level_seen.len() >= 2,
        "level component must take >=2 distinct values (terrain genuinely went live); saw {:?}",
        level_seen
    );
    assert!(
        wob_nonempty_ticks >= 1,
        "wobjects pool must be non-empty for >=1 tick (a shot must spawn); saw {wob_nonempty_ticks}"
    );
    assert!(
        rng_seen.len() >= 2,
        "rng must take >=2 distinct values (Fire + explode draw rand); saw {:?}",
        rng_seen
    );
    assert!(
        ammo_seen.len() >= 2,
        "some worm's slot-0 weapon ammo must vary (firing decrements ammo); saw {:?}",
        ammo_seen
    );
}
