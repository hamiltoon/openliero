//! Per-tick differential test for the Slice-5d **worm DEATH -> RESPAWN path going
//! LIVE** against the C++ oracle — THE MILESTONE of 5d (and of Step-2's death/respawn
//! surface). The golden (`golden/sim_slice5d.txt`, 361 lines for ticks 0..=360) is
//! produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice5d_scenario.txt`): seed 42, the LOADED `physics_fall_test.lev`,
//! `max_bonuses 0`, both worms with EXPLOSIVES in slot 0. worm0 (KILLER, health 100)
//! lands, raises the gun (Up ticks 13-20) and FIRES (tick 38) — the identical kill
//! geometry as slice 5b. worm1 (VICTIM) is VISIBLE + grounded at x=115 but starts at
//! health **12** (below `settings->health/4 = 25`), so:
//!   * the pre-death BLOOD DRIP (`worm.cpp:355-367`) fires on the pre-explosion ticks
//!     (sparse `rand`, early `bobjects` drip);
//!   * the first catching air-burst drives health `<= 0` => the DEATH BLOCK
//!     (`worm.cpp:369-426`): death sound + `--lives` + worm0 `kills++` +
//!     `visible=false` + `killed_timer=150` + the 120-blood + 8-gib spray (`rng`
//!     BURSTS, `nobjects` spikes);
//!   * after the 150-tick `killed_timer` countdown the dead `else` arm runs
//!     `BeginRespawn` (`worm.cpp:711-742` — the level-reading RNG spawn search, the
//!     Step-2 desync trap: `rng` bursts `2*trials`, worm1 `pos` JUMPS) then
//!     `DoRespawning` (`worm.cpp:755-809` — the drop-in convergence + the lone
//!     `rand()&1` aiming draw + dirt carve), and worm1 flips `visible` back to `true`
//!     with `health` restored to `settings->health`.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match over all 361 ticks proves
//!
//! It proves the ENTIRE death+respawn port end-to-end vs the C++ oracle: T0-T2's
//! pre-death drip + death block (`worm_death`), T3's dead-arm `ready` latch +
//! `killed_timer` countdown, T4's `BeginRespawn`/`CheckRespawnPosition` level-reading
//! trial search (the canonical Step-2 desync trap — trial count = f(live level, live
//! enemy pos)), and T5's `DoRespawning` convergence + completion — all riding on the
//! 5b explosives kill chain (fire -> fly/bounce -> air-burst -> `large_explosion`
//! AABB damage -> blood). Because `killed_timer` is **invisible** (in NEITHER the
//! master nor any component fold), a countdown desync would surface ONLY as a
//! mis-timed `BeginRespawn` `rng` burst — so the bit-exact `rng` column across the
//! whole window is what pins the 150-tick countdown.
//!
//! ## The critical 5d setup — all the death/respawn consts post-`new`
//!
//! `SimState::new` defaults the blood consts (`num_blood_colours`/`first_blood_colour`
//! /`bobj_gravity`, as in 5b) AND the `WormSpawnRect*`/`WormMinSpawnDist*` respawn
//! consts to 0. This harness assigns them from the loaded TC AFTER `new` — the exact
//! `LC(...)` values the C++ dumper holds. `settings_health` defaults to 100 (the
//! dumper never overrides `WormSettings::health`), the value `DoRespawning` restores.
//! Both `small_sprites` (7x7 bank) and `large_sprites` (16x16 bank) + `textures` are
//! loaded so the death-spray land-blits and the respawn dirt carve index a real bank.
//! **If any respawn const is left default-0 the run diverges at `BeginRespawn` — that
//! means a const was forgotten, not a sim bug.**
//!
//! Components are asserted FIRST (rng -> level -> worm0 -> worm1 -> the 5 pools) THEN
//! the master `state_hash`, so a divergence localises to a tick + subsystem before the
//! master flags it. The scenario is the single source of truth (parsed via
//! `oracle_tests::scenario`) and the expected values are PARSED from the golden file,
//! never hard-coded; the coverage guards are read from the genuinely DRIVEN
//! `SimState`, never re-parsed from the golden.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`).
/// Read by `DrawDirtEffect`: the `large_explosion` crater carve AND the `DoRespawning`
/// dirt puff index this bank, so an empty bank would panic.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Load the shipped 7x7 small-sprite bank (C++ `small_sprites.Allocate(7,7,130)`).
/// A landed `draw_on_map` nobject (e.g. a spent shell / dirt) blits a 7x7 stamp from
/// here; loaded to match the dumper's `common.small_sprites` in case a spray/dirt
/// nobject lands within the 361-tick window.
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
}

/// The empty-pool component hash (FNV-1a of a zero-length pool). Every pool reads this
/// while idle; a pool leaving this value is how we know it went live.
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
fn sim_slice5d_death_respawn_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5d_scenario.txt"
    ))
    .expect("read golden/sim_slice5d_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 360, "scenario ticks");
    assert_eq!(scenario.max_bonuses, 0, "5d runs with no bonuses (death/respawn only)");
    // worm1 (index 1) starts BELOW settings->health/4 so the pre-death drip + death
    // are reachable; worm0 is the full-health killer.
    assert_eq!(scenario.worms[1].health, 12, "worm1 (victim) starts at low health 12");
    assert_eq!(scenario.worms[0].health, 100, "worm0 (killer) starts at full health 100");
    let worm1_start_lives = scenario.worms[1].lives;

    // --- Parse the golden vectors (ticks 0..=360, master + 9 components). ------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5d.txt"
    ))
    .expect("read golden/sim_slice5d.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 5d HEADLINE in the golden: the full death->respawn signature is present.
    // Read straight from the parsed golden so a regenerated golden that lost the death
    // OR the respawn fails loudly HERE before the per-tick loop. --------------------
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(
        worm1_cols.len() >= 3,
        "5d golden worm1 must show >=3 phases (alive, dead-frozen, reborn); saw {} distinct",
        worm1_cols.len()
    );
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(
        golden_levels.len() >= 2,
        "5d golden must carve (explosion craters + respawn dirt); level took {} values",
        golden_levels.len()
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "5d golden must spawn an sobject (the large_explosion)"
    );
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "5d golden must spawn nobjects (blood/gib/dirt spray)"
    );
    assert!(
        golden.iter().any(|g| g.pools[0] != EMPTY_POOL),
        "5d golden must drip the bobjects blood pool"
    );
    assert!(
        golden.iter().any(|g| g.pools[4] != EMPTY_POOL),
        "5d golden must fly the explosives wobjects"
    );
    for g in &golden {
        assert_eq!(g.pools[1], EMPTY_POOL, "tick {}: bonuses must stay empty (max_bonuses 0)", g.tick);
    }

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). --------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants + object tables.
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // INVARIANTS: the object/weapon tables are indexed by id (the Fire path, the
    // explosion `create_on_exp`, the dirt-throw, the per-worm blood/gib spray, and the
    // respawn all index by id). If id != index those lookups read the wrong object.
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(w.id, i as i32, "weapon id must equal its index (weapon[{i}], id {})", w.id);
    }
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index (got id {})", n.id);
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-5c.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with EXPLOSIVES (the scenario `weapon 0` directive),
    // resolving BY NAME against the loaded TC weapon table (id == index). ----------
    let weapon_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let weapon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == weapon_name)
        .unwrap_or_else(|| panic!("weapon {weapon_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(weapon_idx as WeaponId),
        ammo: objects.weapons[weapon_idx].ammo,
    };

    // Both worms carry EXPLOSIVES in slot 0 (the override folds into each worm's hash).
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

    // --- Build tick-0 state (same `new` signature as 5b/5c). Trailing `0, true, 100`
    // = settings_loading_time / load_change / blood, matching the dumper. -----------
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
        0,
        true,
        100,
    );

    // --- THE CRITICAL 5d STEP: set the blood consts (as 5b) AND the respawn consts.
    // Left at their `new` defaults (0) the run diverges at the first bobject tick
    // (blood) OR at `BeginRespawn` (respawn) — a forgotten const, not a sim bug. -----
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    // The 7x7 bank a landed spray/dirt nobject blits (mirrors the dumper).
    state.small_sprites = load_small_sprites();
    // BeginRespawn / CheckRespawnPosition consts (worm.cpp:711-742, game.cpp:611-650).
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    // `settings_health` (the DoRespawning restore target) defaults to 100 in `new`;
    // pin it here so the "returns to settings_health" guard reads the real target.
    let settings_health = state.settings_health;
    assert_eq!(settings_health, 100, "dumper uses the default WormSettings::health 100");

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(got, want, "tick {tick}: {name}: got {got:08x} expected {want:08x}");
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, the 5 pools) THEN the master,
    // so a divergence localises to a tick + subsystem before the master flags it.
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
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage witnesses read from the genuinely DRIVEN SimState, never re-parsed
    // from the golden. -------------------------------------------------------------
    let mut worm1_health: Vec<i32> = Vec::with_capacity(golden.len());
    let mut worm1_visible: Vec<bool> = Vec::with_capacity(golden.len());
    let mut worm1_lives: Vec<i32> = Vec::with_capacity(golden.len());
    let mut worm0_kills: Vec<i32> = Vec::with_capacity(golden.len());
    let mut worm1_killed_timer: Vec<i32> = Vec::with_capacity(golden.len());
    let mut worm1_pos: Vec<Vec2> = Vec::with_capacity(golden.len());
    let mut nobj_count: Vec<usize> = Vec::with_capacity(golden.len());
    let mut rng_draws: Vec<u64> = Vec::with_capacity(golden.len());
    let mut max_nobjects = 0usize;
    let mut bonuses_always_empty = true;
    let mut saw_type6_blood_nobject = false;
    let mut record = |state: &SimState| {
        worm1_health.push(state.worms[1].health);
        worm1_visible.push(state.worms[1].visible);
        worm1_lives.push(state.worms[1].lives);
        worm0_kills.push(state.worms[0].kills);
        worm1_killed_timer.push(state.worms[1].killed_timer);
        worm1_pos.push(state.worms[1].pos);
        nobj_count.push(state.nobjects.len());
        rng_draws.push(state.rand.draws());
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
        }
        if state.nobjects.iter().any(|n| n.ty == Some(6)) {
            saw_type6_blood_nobject = true;
        }
    };
    record(&state);

    // --- Drive each subsequent tick under SCRIPTED input. THE OFF-BY-ONE: golden line
    // `k` (k>=1) is the result of applying input[k-1] on the pass advancing tick
    // k-1 -> k. ---------------------------------------------------------------------
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // ================= THE 5d COVERAGE GUARDS (all from the DRIVEN state) ==========

    // --- DEATH witness: locate the death tick = first tick worm1 health <= 0. ------
    assert_eq!(worm1_health[0], 12, "worm1 starts at health 12 (from the scenario)");
    let death_tick = (1..worm1_health.len())
        .find(|&k| worm1_health[k] <= 0)
        .expect("worm1 must DIE (health crosses <= 0 from the explosion)");

    // worm1 health CROSSES <= 0 (the death) ...
    let worm1_min = *worm1_health.iter().min().expect("ticks recorded");
    assert!(worm1_min <= 0, "worm1 health must cross <= 0 (it dies); min was {worm1_min}");
    // ... AND RETURNS to settings_health after the respawn completes.
    let worm1_final = *worm1_health.last().expect("ticks recorded");
    assert_eq!(
        worm1_final, settings_health,
        "worm1 health must be restored to settings_health ({settings_health}) after respawn"
    );

    // On the death tick: the death block ran — worm1 hidden, `killed_timer` armed to
    // 150, `lives` decremented by one, worm0 credited one kill, and the spray made
    // `nobjects` non-empty. These are genuine driven-state witnesses (not the golden).
    assert!(!worm1_visible[death_tick], "worm1 must be hidden on the death tick");
    assert_eq!(
        worm1_killed_timer[death_tick], 150,
        "the death block arms killed_timer = 150 on the death tick"
    );
    assert_eq!(
        worm1_lives[death_tick],
        worm1_start_lives - 1,
        "the death block decrements worm1 lives by exactly one"
    );
    assert_eq!(worm0_kills[death_tick], 1, "the killer (worm0) is credited one kill on the death tick");
    assert!(nobj_count[death_tick] > 0, "the death spray makes nobjects non-empty on the death tick");

    // worm1 `lives` never drops below start-1 (it dies exactly once), and worm0
    // `kills` never exceeds one (one kill in the whole window).
    assert_eq!(
        *worm1_lives.last().unwrap(),
        worm1_start_lives - 1,
        "worm1 loses exactly one life over the window"
    );
    assert_eq!(worm1_lives.iter().min().copied().unwrap(), worm1_start_lives - 1, "one death only");
    assert_eq!(worm0_kills.iter().max().copied().unwrap(), 1, "worm0 scores exactly one kill");

    // The death spray sprays type-6 blood nobjects (worm.cpp:414 Create2), never
    // over the O3 pool cap.
    assert!(saw_type6_blood_nobject, "the death spray must include type-6 (blood) nobjects");
    assert!(max_nobjects < 600, "nobjects must stay under the 600 cap (O3); peaked at {max_nobjects}");

    // --- RNG BURST on the death tick: the 120-blood + 8-gib spray draws a large burst
    // vs a normal tick. Read from the monotonic draw counter (diagnostic-only, not
    // hashed) — a genuine driven-state burst witness, not a golden re-parse. --------
    let death_burst = rng_draws[death_tick] - rng_draws[death_tick - 1];
    assert!(
        death_burst > 100,
        "the death spray must BURST the rng on the death tick (>100 draws); saw {death_burst}"
    );

    // --- BeginRespawn witness: after the 150-tick countdown, the dead arm runs
    // BeginRespawn. It is the FIRST tick worm1's killed_timer goes negative (-1) — the
    // hash-silent countdown surfaces here. Derived from the driven state, not the
    // golden. -----------------------------------------------------------------------
    let begin_respawn_tick = (death_tick + 1..worm1_killed_timer.len())
        .find(|&k| worm1_killed_timer[k] < 0)
        .expect("BeginRespawn must run (killed_timer counts down to 0 then flips to -1)");
    // The countdown is a real ~150-tick gap between death and BeginRespawn (the
    // invisible timer's only observable footprint is WHEN this lands).
    let countdown = begin_respawn_tick - death_tick;
    assert!(
        (140..=160).contains(&countdown),
        "the killed_timer countdown before BeginRespawn must be ~150 ticks; saw {countdown}"
    );

    // worm1 `pos` JUMPS at BeginRespawn (the trial-count witness): frozen at the death
    // pos through the whole dead phase, then teleported to the new spawn. -----------
    let death_pos = worm1_pos[death_tick];
    for (k, &pos) in worm1_pos
        .iter()
        .enumerate()
        .take(begin_respawn_tick)
        .skip(death_tick)
    {
        assert_eq!(
            pos, death_pos,
            "tick {k}: worm1 pos must stay frozen at the death pos through the dead phase"
        );
    }
    let spawn_pos = worm1_pos[begin_respawn_tick];
    let dx_px = ((spawn_pos.x - death_pos.x) >> 16).abs();
    let dy_px = ((spawn_pos.y - death_pos.y) >> 16).abs();
    assert!(
        dx_px.max(dy_px) > 100,
        "worm1 pos must JUMP at BeginRespawn (>100px in a axis; the spawn search moved it); \
         dx={dx_px}px dy={dy_px}px"
    );
    // And the trial search BURSTS the rng on that tick (>=2 draws = at least one trial
    // of rand(W)+rand(H)).
    let respawn_burst = rng_draws[begin_respawn_tick] - rng_draws[begin_respawn_tick - 1];
    assert!(
        respawn_burst >= 2,
        "BeginRespawn must draw >=2 rand (trial-count witness: rand(W),rand(H) per trial); saw {respawn_burst}"
    );

    // --- DoRespawning witness: worm1 goes visible -> invisible -> visible, with the
    // final respawn completing (health restored). The visibility must have all three
    // phases. -----------------------------------------------------------------------
    assert!(worm1_visible[0], "worm1 starts VISIBLE (alive)");
    let first_dead = worm1_visible.iter().position(|&v| !v).expect("worm1 goes invisible (dies)");
    let reborn_tick = (first_dead..worm1_visible.len())
        .find(|&k| worm1_visible[k])
        .expect("worm1 goes VISIBLE again (DoRespawning completes)");
    assert!(
        reborn_tick > begin_respawn_tick,
        "DoRespawning completes AFTER BeginRespawn (reborn {reborn_tick} > begin {begin_respawn_tick})"
    );
    assert_eq!(
        worm1_health[reborn_tick], settings_health,
        "worm1 health is restored to settings_health on the reborn tick"
    );
    // DoRespawning completion draws the lone `rand()&1` aiming bit (+ the dirt carve):
    // a small rng footprint on the reborn tick.
    let reborn_burst = rng_draws[reborn_tick] - rng_draws[reborn_tick - 1];
    assert!(
        reborn_burst >= 1,
        "DoRespawning completion must draw at least the aiming rand()&1; saw {reborn_burst}"
    );
    // worm1 stays visible + at full health from the reborn tick to the end.
    for k in reborn_tick..worm1_visible.len() {
        assert!(worm1_visible[k], "tick {k}: worm1 must stay visible after respawn");
        assert_eq!(worm1_health[k], settings_health, "tick {k}: worm1 stays at full health after respawn");
    }

    // The window genuinely carved terrain (explosion craters + respawn dirt) and never
    // spawned a bonus.
    let level_seen: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(level_seen.len() >= 2, "terrain must carve (>=2 distinct level hashes)");
    assert!(bonuses_always_empty, "bonuses must stay empty (max_bonuses 0)");
}
