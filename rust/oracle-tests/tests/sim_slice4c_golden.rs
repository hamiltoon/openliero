//! Per-tick differential test for the Slice-4c DART -> small_explosion lifecycle
//! against the C++ oracle — THE MILESTONE of 4c. The golden
//! (`golden/sim_slice4c.txt`, 91 lines for ticks 0..=90) is produced by the real
//! C++ `Game` running the *same scenario* (`golden/sim_slice4c_scenario.txt`):
//! seed 42, the LOADED `physics_fall_test.lev`, worm0 visible and grounded with the
//! **DART** in weapon slot 0, worm1 invisible and inert. worm0 falls, lands, raises
//! the gun ~25 ticks and FIRES a single DART (input tick 38) that arcs UP and
//! parabolas back DOWN onto the dirt floor downrange (explode at golden line 51),
//! where `BlowUpObject`'s `create_on_exp` spawns a **`small_explosion`** SObject.
//! That explosion is the whole 4c cluster: the sound `rand(2)`, the dirt-throw
//! (`AnyDirt && rand(8)` row-major, then `rand(128)` + `NObjectType::Create2`
//! spawning dirt-debris `nobjects`), and the carving `draw_dirt_effect(x-7,y-7)`
//! `rand(2)` that DIGS the level (texture 2, `n_draw_back=true`).
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — the object pools go live
//!
//! This is the FIRST slice whose `sobjects` and `nobjects` component columns are
//! NON-EMPTY, non-constant time series matched against C++ tick-for-tick (exactly as
//! 4a did for `wobjects` and 4b for `level`). A bit-exact match over 91 ticks proves
//! the entire 4c port — `BlowUpObject`'s `create_on_exp` branch, `SObject::Create`
//! (sound/dirt-throw/blow-away/crater in C++ statement order), `SObject::Process`
//! (anim cur_frame 0->5 then free), `NObjectType::Create2` (`rand(speed_v)` first,
//! then distribution x2), `NObject::Process` (dirt-debris flight + free), the
//! cross-pool spawn ordering, and the carving `draw_dirt_effect` — reproduces the
//! C++ RNG stream, pool folds and `material_id` writes bit-for-bit.
//!
//! The component columns are asserted FIRST (rng -> level -> worm0 -> worm1 ->
//! bobjects -> bonuses -> sobjects -> nobjects -> wobjects) THEN the master
//! `state_hash`, so a divergence localises to a tick + subsystem before the master
//! flags it: `rng` => a wrong draw count/order in the explosion cluster; `level` =>
//! the carving `draw_dirt_effect`; `sobjects` => the sobject id/cur_frame (anim
//! timing); `nobjects`(pos) => the `Create2` velocity. **O11:** an `nobjects`-column
//! match does NOT prove nobject `vel`/`cur_frame` — those localise via the MASTER
//! only (the component fold is `pos.x,pos.y` only). So if every component matches but
//! the master diverges, suspect a nobject `vel`/`cur_frame` (a `Create2`
//! distribution/speed draw or the kPix->cur_frame mapping).
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
/// from `sprites/large.tga`. Threaded into `SimState` for the carving
/// `draw_dirt_effect`: `small_explosion`'s `dirt_effect = 2` indexes this bank on the
/// explode tick, so an empty bank would panic.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// The empty-pool component hash (FNV-1a of a zero-length pool). Every pool reads
/// this while idle; a pool leaving this value is how we know it went live.
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
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
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
fn sim_slice4c_explosion_objects_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4c_scenario.txt"
    ))
    .expect("read golden/sim_slice4c_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 90, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=90, master + 9 components). ------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4c.txt"
    ))
    .expect("read golden/sim_slice4c.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 4c HEADLINE (mirrors 4b's "level must move"): the explosion both
    // CARVES the terrain and spawns dirt-debris, so the `level`, `sobjects` AND
    // `nobjects` columns must all go live. Read straight from the parsed golden.
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(
        golden_levels.len() >= 2,
        "4c golden must carve: level column takes >=2 distinct values (pristine + dug); saw {:?}",
        golden_levels
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "4c golden must spawn an sobject (sob column leaves the empty-pool hash)"
    );
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "4c golden must spawn dirt-debris nobjects (nob column leaves the empty-pool hash)"
    );

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). --------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants + the object
    // tables. `Objects::load` parses weapons, nobject_types AND sobject_types from
    // the TC exactly as the dumper's `common` holds them; `tc.cfg` carries the
    // materials, the textures table, the large-sprite bank name and the
    // physics/control consts. ------------------------------------------------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // INVARIANT (load-bearing for the Fire path): `weapon.id == array index`. The
    // spawned `WObject.ty` is set to `weapon.id` in weapon_fire, and
    // wobject_process indexes `weapons[ty]` — those only line up if id == index.
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(
            w.id, i as i32,
            "weapon id must equal its index (weapon[{i}] = {:?}, id {})",
            w.name, w.id
        );
    }
    // INVARIANT (load-bearing for `create_on_exp` + the dirt-throw): the object
    // tables are indexed by id (`create_on_exp` -> `sobject_types[2]`; the dirt-throw
    // -> `nobject_types[2]`). If id != index those lookups read the wrong object, so
    // STOP here rather than paper over it.
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index (got id {})", n.id);
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-4b.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with the DART, mirroring the C++ dumper's
    // `ResolveWeapon("DART")`. The name is the *scenario's* `weapon 0` directive
    // (single source of truth) and must match `common->weapons[i].name` exactly —
    // the UPPERCASE "DART", not the filename "dart". The dumper leaves
    // current_weapon = 0 and delay_left/loading_left at 0; `from_init` defaults those.
    let dart_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let dart_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == dart_name)
        .unwrap_or_else(|| panic!("weapon {dart_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(dart_idx as WeaponId),
        ammo: objects.weapons[dart_idx].ammo,
    };

    // --- Build the two worm inits from the scenario. BOTH worms get the DART in
    // slot 0 (the scenario `weapon 0` overrides both); worm1 is invisible/inert so it
    // never fires, but its 5-slot weapon state still folds into worm1's hash, so the
    // override must apply to it too for the master to match the dumper.
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
    // bank, the textures table AND the two new object tables (sobject_types +
    // nobject_types — the final two `SimState::new` args added in Task 0):
    // `SObject::Create` reads sobject_types[2] (small_explosion) and the dirt-throw
    // reads nobject_types[2] (particle__disappearing) on the explode tick. -------
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
        100,
    );

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, the 5 pools incl. sobjects
    // and nobjects) THEN the master, so a divergence localises to a tick + subsystem
    // before the master flags it. O11: the `nobjects` column proves position only;
    // a nobject vel/cur_frame desync shows only in the master.
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
        // The MASTER, last: it folds the explosion cluster's RNG, the live sobject
        // (id+cur_frame), the live nobjects (pos+vel+cur_frame+type — wider than the
        // component fold, O11) AND the carved level.
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage guard: prove the run actually exercises Fire AND the explosion
    // cluster (not just flight). All values are read from the DRIVEN SimState, never
    // re-parsed from the golden, so the asserts below are a genuine witness. -----
    let mut rng_by_tick: Vec<u32> = Vec::with_capacity(golden.len());
    let mut wob_nonempty_ticks = 0usize;
    let mut sob_nonempty_ticks = 0usize;
    let mut nob_nonempty_ticks = 0usize;
    let mut saw_sobject_id2 = false;
    let mut max_nobjects = 0usize;
    let mut bobjects_always_empty = true;
    let mut worm0_health_always_100 = true;
    let mut level_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        let c = hash_components(state);
        rng_by_tick.push(c.rng);
        if c.wobjects != EMPTY_POOL {
            wob_nonempty_ticks += 1;
        }
        if c.sobjects != EMPTY_POOL {
            sob_nonempty_ticks += 1;
        }
        if c.nobjects != EMPTY_POOL {
            nob_nonempty_ticks += 1;
        }
        if state.sobjects.iter().any(|s| s.id == 2) {
            saw_sobject_id2 = true;
        }
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if !state.bobjects.is_empty() {
            bobjects_always_empty = false;
        }
        if state.worms[0].health != 100 {
            worm0_health_always_100 = false;
        }
        level_seen.insert(c.level);
    };
    record(&state);

    // --- Drive each subsequent tick under SCRIPTED input. THE OFF-BY-ONE:
    // golden line `k` (k>=1) is the result of applying input[k-1] on the pass that
    // advances tick k-1 -> k. So produce line `k` by calling process_frame with
    // input keyed `k-1`.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // --- Locate the fire tick + the explode tick from the DRIVEN state. The DART
    // Fire spawns a wobject (`wob` leaves empty) but draws ZERO rand; the explosion
    // is the first tick the rng MOVES. Both are derived, not hard-coded. --------
    let fire_line = (1..golden.len())
        .find(|&k| golden[k].pools[4] != EMPTY_POOL && golden[k - 1].pools[4] == EMPTY_POOL)
        .expect("a wobject must spawn (the DART is fired)");
    let explode_line = (1..golden.len())
        .find(|&k| rng_by_tick[k] != rng_by_tick[k - 1])
        .expect("the rng must move once (the explosion cluster draws)");

    // The DART Fire draws 0 rand: the rng is UNCHANGED across the fire tick.
    assert_eq!(
        rng_by_tick[fire_line], rng_by_tick[fire_line - 1],
        "DART Fire must draw zero rand: rng unchanged at fire tick {fire_line} \
         (got {:08x} vs prev {:08x})",
        rng_by_tick[fire_line], rng_by_tick[fire_line - 1]
    );
    // The explosion cluster MOVES the rng (sound + dirt-throw + Create2 + crater).
    assert_ne!(
        rng_by_tick[explode_line], rng_by_tick[explode_line - 1],
        "explosion must draw rand: rng moves at explode tick {explode_line}"
    );
    assert!(
        explode_line > fire_line,
        "explode tick {explode_line} must come after fire tick {fire_line}"
    );

    // --- Coverage assertions read from the DRIVEN SimState: the matched run
    // genuinely fired, exploded, spawned both pools and carved terrain. ---------
    assert!(
        wob_nonempty_ticks >= 1,
        "wobjects pool must be non-empty for >=1 tick (the DART must spawn); saw {wob_nonempty_ticks}"
    );
    assert!(
        sob_nonempty_ticks >= 1,
        "sobjects pool must be non-empty for >=1 tick (the explosion sobject); saw {sob_nonempty_ticks}"
    );
    assert!(
        saw_sobject_id2,
        "the spawned sobject must be small_explosion (id == 2)"
    );
    assert!(
        nob_nonempty_ticks >= 1,
        "nobjects pool must be non-empty for >=1 tick (dirt debris); saw {nob_nonempty_ticks}"
    );
    assert!(
        level_seen.len() >= 2,
        "level component must take >=2 distinct values (terrain genuinely carved); saw {:?}",
        level_seen
    );
    // O3 guard: a single shot keeps nobjects well under the 600 cap.
    assert!(
        max_nobjects < 600,
        "nobjects must stay under the 600 cap (O3 deferred); peaked at {max_nobjects}"
    );
    // O10 guard: all worms stay outside the explosion's +/-detect_range box, so no
    // worm DoDamage/blow-away/blood -> bobjects stays empty and worm0 stays health 100.
    assert!(
        bobjects_always_empty,
        "bobjects must stay empty (no worm in detect_range -> no blood; O10)"
    );
    assert!(
        worm0_health_always_100,
        "worm0 health must stay 100 (no explosion damage; O10)"
    );
    // O10: the worm columns must NOT deviate across the explode tick (no blow-away
    // nudging worm vel/pos). Read from the parsed golden's worm columns at the
    // explode line vs the line before — both already matched the driven state above.
    assert_eq!(
        golden[explode_line].worm0, golden[explode_line - 1].worm0,
        "worm0 column must be unchanged across the explode tick (no blow-away; O10)"
    );
    assert_eq!(
        golden[explode_line].worm1, golden[explode_line - 1].worm1,
        "worm1 column must be unchanged across the explode tick (invisible/inert)"
    );
}
