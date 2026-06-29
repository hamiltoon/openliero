//! Per-tick differential test for the Slice-5a CANNON -> medium_explosion + 5
//! SPLINTERS lifecycle against the C++ oracle — THE MILESTONE of 5a. The golden
//! (`golden/sim_slice5a.txt`, 131 lines for ticks 0..=130) is produced by the real
//! C++ `Game` running the *same scenario* (`golden/sim_slice5a_scenario.txt`):
//! seed 42, the LOADED `physics_fall_test.lev`, worm0 visible and grounded with the
//! **CANNON** in weapon slot 0, worm1 invisible and inert. worm0 falls, lands, raises
//! the gun and FIRES a single CANNON shell (input tick ~43) that arcs flat-LEFT,
//! AWAY from worm0, and parabolas back DOWN onto the dirt floor ~110px downrange
//! (explode at tick 99), where `BlowUpObject`'s `create_on_exp` spawns a
//! **`medium_explosion`** SObject (sobject id 1) AND — the 5a headline — the
//! splinter-scatter arm fires `splinter_amount = 5` `particle__small_damage`
//! splinter **nobjects** via `NObjectType::Create2` (T0, 97857a6). Those 5 splinters
//! fly out, hit the dirt and `expl_ground` into secondary `small_explosion` sobjects
//! (id 2) that carve the terrain again at ticks ~103 and ~109 — so the `level` column
//! takes ~4 distinct values, not 2.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — the splinter arm goes live
//!
//! A bit-exact match over 131 ticks proves the entire T0 splinter port end-to-end:
//! `BlowUpObject`'s splinter loop draws and spawns the 5 `particle__small_damage`
//! nobjects in C++ statement order, INTERLEAVED in the already-proven (4c)
//! `medium_explosion` cluster — the sound `rand(4)`, the dirt-throw, the carving
//! `draw_dirt_effect`, and now the splinter `Create2` draws — reproducing the C++ RNG
//! stream, the cross-pool spawn ordering, the pool folds and the `material_id` writes
//! bit-for-bit. Two RNG facts distinguish 5a from 4c:
//!   * Unlike 4c's DART (recoil 0, distribution 0 ⇒ Fire draws ZERO rand), the CANNON
//!     has `distribution = 300` ⇒ Fire draws **2 rand** (spread x,y). So the rng MUST
//!     MOVE across the fire tick, the OPPOSITE of 4c. The explosion cluster is the
//!     SECOND rng move, not the first.
//!   * The CANNON has `recoil = 40`: worm0 RECOILS at the fire tick and DRIFTS under
//!     friction for ~73 ticks, so worm0's component hash MOVES across the run — that
//!     is physics, faithfully reproduced by a no-damage sim. The no-damage invariant
//!     is therefore read off worm0's **health (== 100 every tick)**, `bobjects` (empty)
//!     and worm1 (constant), NOT off worm0's (always-moving) column.
//!
//! The component columns are asserted FIRST (rng -> level -> worm0 -> worm1 ->
//! bobjects -> bonuses -> sobjects -> nobjects -> wobjects) THEN the master
//! `state_hash`, so a divergence localises to a tick + subsystem before the master
//! flags it: `rng` => a wrong draw count/order in the explosion/splinter cluster (the
//! splinter loop's `Create2` draws localise here); `level` => the carving
//! `draw_dirt_effect` (main + splinter secondaries); `sobjects` => the sobject
//! id/cur_frame; `nobjects`(pos) => the splinter/dirt `Create2` velocity. **O11:** an
//! `nobjects`-column match proves position ONLY — a splinter `vel`/`cur_frame`/`type`
//! desync shows only in the MASTER (the component fold is `pos.x,pos.y` only). So if
//! every component matches but the master diverges, suspect a splinter
//! `vel`/`cur_frame`/`type` (a `Create2` distribution/speed draw or the kPix->cur_frame
//! mapping in the splinter arm).
//!
//! The scenario is the single source of truth (parsed via `oracle_tests::scenario`)
//! and the expected values are PARSED from the golden file, never hard-coded.
//!
//! ## ⚠️ BLOCKED — `#[ignore]`d pending an unported splinter-explosion sim path
//!
//! This milestone test is COMPLETE and CORRECT but currently `#[ignore]`d because the
//! sim cannot yet reproduce the golden — a genuine gap surfaced by this very test
//! (TDD working as intended; the golden is NOT wrong and NO assertion is weakened):
//!
//!   * Through **tick 102** the driven `SimState` matches the C++ golden BIT-EXACT on
//!     all 11 columns (master + 9 components) — so T0's splinter SPAWN + flight is
//!     proven: the 5 splinters spawn at tick 99 (nobjects jump +5, master matches) and
//!     fly correctly.
//!   * First divergence is **tick 103, `rng` column** (release: got `70785b7b`,
//!     expected `5308467f`). Root cause: the splinter type `particle__small_damage`
//!     has `expl_ground = true` + `create_on_exp = "small_explosion"`, so each splinter
//!     EXPLODES into a secondary `small_explosion` sobject when it lands (golden carves
//!     at ticks 99/103/109). `NObject::Process`'s `create_on_exp` arm is still DEFERRED
//!     (`sim/src/nobject.rs:~430`), so the Rust splinter draws no RNG / spawns no
//!     sobject on explGround → the `rng`/`level`/`sobjects` columns diverge at 103.
//!   * In a DEBUG build it panics even earlier, at the deferred O10 worm-hit assert
//!     (`sim/src/nobject.rs:~419`, `debug_assert!(hit_damage <= 0)`): the splinter has
//!     `hit_damage = 2`, so processing it before it explodes trips the assert — even
//!     though it hits no worm (the C++ worm-hit loop draws ZERO rand on a no-hit, so
//!     the correct port is a no-op here, not a panic).
//!
//! Closing this needs a SIM task (out of scope for this test-only task, which must not
//! touch sim files): port `NObject::Process`'s `create_on_exp` spawn and relax the O10
//! worm-hit assert to allow the no-hit path. Once landed, remove the `#[ignore]`.

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
/// `draw_dirt_effect`: the explosions' `dirt_effect` indexes this bank on the carve
/// ticks (main + splinter secondaries), so an empty bank would panic.
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
#[ignore = "BLOCKED: sim diverges at tick 103 rng — splinter `create_on_exp` \
            (small_explosion secondary) unported (sim/src/nobject.rs:~430) + O10 \
            worm-hit assert (nobject.rs:~419) panics on hit_damage>0 splinter. \
            Needs a sim task; un-ignore once NObject::Process create_on_exp lands."]
fn sim_slice5a_splinter_objects_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5a_scenario.txt"
    ))
    .expect("read golden/sim_slice5a_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 130, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=130, master + 9 components). ------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5a.txt"
    ))
    .expect("read golden/sim_slice5a.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 5a HEADLINE: the cannon explosion CARVES the terrain (main +
    // splinter secondaries) and spawns BOTH dirt-debris and 5 splinter nobjects, so
    // the `level`, `sobjects` AND `nobjects` columns must all go live. The level
    // column takes >=2 distinct values (the golden carves ~4 — main + splinters).
    // Read straight from the parsed golden.
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(
        golden_levels.len() >= 2,
        "5a golden must carve: level column takes >=2 distinct values (pristine + dug); saw {:?}",
        golden_levels
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "5a golden must spawn an sobject (sob column leaves the empty-pool hash)"
    );
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "5a golden must spawn splinter/dirt nobjects (nob column leaves the empty-pool hash)"
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
    // INVARIANT (load-bearing for `create_on_exp`, the dirt-throw AND the splinter
    // arm): the object tables are indexed by id (`create_on_exp` -> the medium/small
    // `sobject_types`; the dirt-throw + splinters -> the `nobject_types`). If
    // id != index those lookups read the wrong object, so STOP here rather than paper
    // over it.
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index (got id {})", n.id);
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-4d.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with the CANNON, mirroring the C++ dumper's
    // `ResolveWeapon("CANNON")`. The name is the *scenario's* `weapon 0` directive
    // (single source of truth) and must match `common->weapons[i].name` exactly —
    // the UPPERCASE "CANNON", not the filename "cannon". The dumper leaves
    // current_weapon = 0 and delay_left/loading_left at 0; `from_init` defaults those.
    let cannon_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let cannon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == cannon_name)
        .unwrap_or_else(|| panic!("weapon {cannon_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(cannon_idx as WeaponId),
        ammo: objects.weapons[cannon_idx].ammo,
    };

    // --- Build the two worm inits from the scenario. BOTH worms get the CANNON in
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
    // bank, the textures table AND the two object tables (sobject_types +
    // nobject_types): `SObject::Create` reads the medium/small explosion
    // sobject_types and the dirt-throw + splinter arm read the nobject_types on the
    // explode tick. The trailing `100, true` are settings_loading_time / load_change
    // (cannon doesn't reload here — unchanged from 4c). --------------------------
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

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, the 5 pools incl. sobjects
    // and nobjects) THEN the master, so a divergence localises to a tick + subsystem
    // before the master flags it. O11: the `nobjects` column proves position only; a
    // splinter vel/cur_frame/type desync shows only in the master.
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
        // The MASTER, last: it folds the explosion+splinter cluster's RNG, the live
        // sobjects (id+cur_frame), the live nobjects (pos+vel+cur_frame+type — wider
        // than the component fold, O11) AND the carved level.
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage guard: prove the run actually exercises Fire AND the explosion +
    // splinter cluster (not just flight). All values are read from the DRIVEN
    // SimState, never re-parsed from the golden, so the asserts below are a genuine
    // witness. ----------------------------------------------------------------
    let mut rng_by_tick: Vec<u32> = Vec::with_capacity(golden.len());
    let mut nob_count_by_tick: Vec<usize> = Vec::with_capacity(golden.len());
    let mut wob_nonempty_ticks = 0usize;
    let mut sob_nonempty_ticks = 0usize;
    let mut nob_nonempty_ticks = 0usize;
    let mut saw_medium_explosion = false; // sobject id 1 = the cannon's create_on_exp
    let mut max_nobjects = 0usize;
    let mut bobjects_always_empty = true;
    let mut bonuses_always_empty = true;
    let mut worm0_health_always_100 = true;
    let mut level_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        let c = hash_components(state);
        rng_by_tick.push(c.rng);
        nob_count_by_tick.push(state.nobjects.len());
        if c.wobjects != EMPTY_POOL {
            wob_nonempty_ticks += 1;
        }
        if c.sobjects != EMPTY_POOL {
            sob_nonempty_ticks += 1;
        }
        if c.nobjects != EMPTY_POOL {
            nob_nonempty_ticks += 1;
        }
        if state.sobjects.iter().any(|s| s.id == 1) {
            saw_medium_explosion = true;
        }
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if !state.bobjects.is_empty() {
            bobjects_always_empty = false;
        }
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
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

    // --- Locate the fire tick + the explode tick from the DRIVEN state. The CANNON
    // Fire spawns a wobject (`wob` leaves empty) AND draws 2 rand (distribution=300),
    // so the rng MOVES at the fire tick — the OPPOSITE of 4c's dart. The explosion is
    // therefore the SECOND rng move: the first rng move AFTER the fire tick. Both are
    // derived from the driven state, not hard-coded. --------------------------------
    let fire_line = (1..golden.len())
        .find(|&k| golden[k].pools[4] != EMPTY_POOL && golden[k - 1].pools[4] == EMPTY_POOL)
        .expect("a wobject must spawn (the CANNON is fired)");
    let explode_line = (fire_line + 1..golden.len())
        .find(|&k| rng_by_tick[k] != rng_by_tick[k - 1])
        .expect("the rng must move after the fire tick (the explosion+splinter cluster draws)");

    // FLIPPED from 4c: the CANNON Fire draws 2 rand (spread x,y), so the rng MUST
    // CHANGE across the fire tick.
    assert_ne!(
        rng_by_tick[fire_line], rng_by_tick[fire_line - 1],
        "CANNON Fire must draw rand (distribution spread x,y): rng moves at fire tick {fire_line} \
         (got {:08x} vs prev {:08x})",
        rng_by_tick[fire_line], rng_by_tick[fire_line - 1]
    );
    // The explosion+splinter cluster MOVES the rng (sound + dirt-throw + the 5
    // splinter Create2 draws + crater).
    assert_ne!(
        rng_by_tick[explode_line], rng_by_tick[explode_line - 1],
        "explosion+splinter cluster must draw rand: rng moves at explode tick {explode_line}"
    );
    assert!(
        explode_line > fire_line,
        "explode tick {explode_line} must come after fire tick {fire_line}"
    );

    // --- THE 5a SPLINTER GUARD: the 5 `particle__small_damage` splinters spawn at
    // the explode tick (alongside dirt-debris), so the DRIVEN nobjects count must
    // JUMP by >= 5 across the explode tick. Read from `state.nobjects.len()` per tick
    // (a genuine witness that the splinter arm actually ran), never the golden's
    // pos-only `nob` fold (O11). >= 5 is a safe lower bound (splinters + dirt). ------
    let splinter_jump = nob_count_by_tick[explode_line] as i64
        - nob_count_by_tick[explode_line - 1] as i64;
    assert!(
        splinter_jump >= 5,
        "the 5 splinters must spawn at the explode tick: nobjects count must jump by >= 5 \
         (was {} at tick {}, {} at tick {} — jump {})",
        nob_count_by_tick[explode_line - 1],
        explode_line - 1,
        nob_count_by_tick[explode_line],
        explode_line,
        splinter_jump
    );

    // --- Coverage assertions read from the DRIVEN SimState: the matched run
    // genuinely fired, exploded, spawned both pools, scattered splinters and carved
    // terrain. ----------------------------------------------------------------
    assert!(
        wob_nonempty_ticks >= 1,
        "wobjects pool must be non-empty for >=1 tick (the CANNON must spawn); saw {wob_nonempty_ticks}"
    );
    assert!(
        sob_nonempty_ticks >= 1,
        "sobjects pool must be non-empty for >=1 tick (the explosion sobject); saw {sob_nonempty_ticks}"
    );
    assert!(
        saw_medium_explosion,
        "the spawned sobject must be medium_explosion (id == 1, the cannon's create_on_exp)"
    );
    assert!(
        nob_nonempty_ticks >= 1,
        "nobjects pool must be non-empty for >=1 tick (splinters + dirt debris); saw {nob_nonempty_ticks}"
    );
    assert!(
        level_seen.len() >= 2,
        "level component must take >=2 distinct values (terrain genuinely carved: main + \
         splinter secondaries); saw {:?}",
        level_seen
    );
    // O3 guard: a single shot + 5 splinters + dirt keeps nobjects well under the cap.
    assert!(
        max_nobjects < 600,
        "nobjects must stay under the 600 cap (O3 deferred); peaked at {max_nobjects}"
    );
    // No-damage proof part 1: no worm sits in any explosion's +/-detect_range box, so
    // the per-worm DoDamage/blow-away/blood loop draws NOTHING and spawns no blood ->
    // bobjects (and bonuses) stay empty every tick.
    assert!(
        bobjects_always_empty,
        "bobjects must stay empty (no worm in detect_range -> no blood)"
    );
    assert!(
        bonuses_always_empty,
        "bonuses must stay empty (5a spawns no bonuses)"
    );
    // No-damage proof part 2 (THE definitive one): worm0 health == 100 EVERY tick.
    // Unlike 4c's frozen dart-worm, worm0's *hash* MOVES here (the cannon's recoil=40
    // drifts it under friction for ~73 ticks — physics, faithfully reproduced) — but
    // its `health` must stay 100, proving NO `DoDamage` ever fired despite the drift.
    assert!(
        worm0_health_always_100,
        "worm0 health must stay 100 every tick (no explosion/splinter damage, despite \
         the cannon's recoil drift moving worm0's hash)"
    );
    // worm1 (invisible/inert) must NOT deviate across the explode tick — its column is
    // constant the whole run, so equality here also witnesses no splinter reached it.
    // (worm0 is deliberately NOT checked: recoil drift legitimately moves its column.)
    assert_eq!(
        golden[explode_line].worm1, golden[explode_line - 1].worm1,
        "worm1 column must be unchanged across the explode tick (invisible/inert, no splinter hit)"
    );
}
