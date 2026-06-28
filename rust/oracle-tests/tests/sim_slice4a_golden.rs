//! Per-tick differential test for the Slice-4a fan projectile lifecycle against
//! the C++ oracle — THE MILESTONE. The golden (`golden/sim_slice4a.txt`, 93 lines
//! for ticks 0..=92) is produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice4a_scenario.txt`): seed 42, the LOADED `physics_fall_test.lev`,
//! worm0 visible and grounded with the **FAN** in weapon slot 0, worm1 invisible
//! and inert. worm0 falls, lands, lowers the gun and FIRES a ground shot (tick 16),
//! then raises the gun ~30 ticks and FIRES a timeout shot (tick 47) that flies
//! straight up and explodes on its time_to_explo countdown.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — the Fire path lights up
//!
//! Slices 1-3 never fired: the five object pools stayed empty (`00000001`) and the
//! weapon table was unused. Slice 4a is the FIRST slice where a worm actually
//! fires, so it is the first slice that exercises `worm_fire` + `weapon_fire` +
//! `wobject_process` + `blow_up` + the driver's object-loop-before-worms ordering +
//! the Fire gate, end to end. Crucially it is also the first slice whose
//! `wobjects` component column (`wob`) goes NON-EMPTY and is HASHED, so a green
//! `wob` column here is the first real validation of the slice-1 `hash.rs` wobject
//! fold — every earlier golden folded an empty wobject pool, which can never catch
//! a bug in how a live projectile is hashed.
//!
//! The component columns are asserted FIRST (rng → level → worm0 → worm1 → 5 pools)
//! then the master `state_hash`, so a divergence localises to a tick + subsystem
//! before the master flags it: `wob` => wobject fields/`ty`/the hash fold; `rng` =>
//! the Fire RNG draw order; `worm0` => recoil/ammo/firecone; master-only =>
//! aiming/weapons/delay_left.
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
/// from `sprites/large.tga`. Threaded into `SimState` for Slice-4b's DrawDirtEffect;
/// the fan's `dirt_effect = -1` so it is never indexed here, and it is not hashed,
/// so this golden is unchanged.
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
fn sim_slice4a_fan_fire_matches_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4a_scenario.txt"
    ))
    .expect("read golden/sim_slice4a_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 92, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=92, master + 9 components). ------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4a.txt"
    ))
    .expect("read golden/sim_slice4a.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- The 4a pristine-terrain guard: dirt_effect=-1 means NEITHER shot digs,
    // so the `level` column must be CONSTANT across all 93 golden lines. If it ever
    // moves, a dig/dirt-effect leaked into the Fire path. (Read straight from the
    // parsed golden.)
    assert!(
        golden.iter().all(|g| g.level == golden[0].level),
        "4a guarantee: level column must be constant (no dig) across all ticks"
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
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-3.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with the FAN, mirroring the C++ dumper's
    // `ResolveWeapon("FAN")`. The name is the *scenario's* `weapon 0` directive
    // (single source of truth) and must match `common->weapons[i].name` exactly —
    // the UPPERCASE "FAN", not the filename/"fan" (Task 6 proved the lowercase
    // forms do not resolve). The dumper leaves current_weapon = 0 and
    // delay_left/loading_left at 0; `from_init` already gives those defaults
    // (verified: from_init sets delay_left = loading_left = 0, current_weapon = 0).
    let fan_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let fan_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == fan_name)
        .unwrap_or_else(|| panic!("weapon {fan_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(fan_idx as WeaponId),
        ammo: objects.weapons[fan_idx].ammo,
    };

    // --- Build the two worm inits from the scenario. BOTH worms get the FAN in
    // slot 0 (the scenario `weapon 0` overrides both); worm1 is invisible/inert so
    // it never fires, but its 5-slot weapon state still folds into worm1's hash, so
    // the override must apply to it too for the master to match the dumper.
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

    // --- Build tick-0 state. Slice 4a passes the FULL weapon table (slices 1-3
    // passed `Vec::new()` — no firing); wobject_process/worm_fire look up weapon
    // params by `ty`, which is the verified id == index. ----------------------
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
    );

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, pools incl. wobjects)
    // THEN the master, so a divergence localises to a tick + subsystem before the
    // master flags it.
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
        // The MASTER, last: it folds the live fire state (aiming, the 5-slot weapon
        // state incl. delay_left, and the spawned wobject via the pool hash).
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage guard: prove the run actually exercises Fire (not just flight).
    // All values are read from the DRIVEN SimState, never re-parsed from the
    // golden, so the asserts below are a genuine witness of fire activity.
    let mut wob_nonempty_ticks = 0usize;
    let mut rng_seen = std::collections::HashSet::new();
    let mut ammo_seen = std::collections::HashSet::new();
    let mut delay_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        let c = hash_components(state);
        if c.wobjects != EMPTY_POOL {
            wob_nonempty_ticks += 1;
        }
        rng_seen.insert(c.rng);
        for w in &state.worms {
            ammo_seen.insert(w.weapons[0].ammo);
            for wpn in &w.weapons {
                delay_seen.insert(wpn.delay_left);
            }
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

    // --- Coverage assertions: the matched run genuinely fired, flew, and exploded.
    assert!(
        wob_nonempty_ticks >= 1,
        "wobjects pool must be non-empty for >=1 tick (a shot must spawn); saw {wob_nonempty_ticks}"
    );
    assert!(
        rng_seen.len() >= 2,
        "rng must take >=2 distinct values (Fire draws rand for time_to_explo); saw {:?}",
        rng_seen
    );
    assert!(
        ammo_seen.len() >= 2,
        "some worm's slot-0 weapon ammo must vary (firing decrements ammo); saw {:?}",
        ammo_seen
    );
    assert!(
        delay_seen.len() >= 2,
        "some weapon delay_left must vary across the run (firing sets the delay); saw {:?}",
        delay_seen
    );
}
