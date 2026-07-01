//! Per-tick differential test for Slice-5'a — the per-pixel IN-FLIGHT wobject
//! worm-hit arm (`weapon.cpp:287-326` + the `CheckForSpecWormHit` predicate,
//! `worm.cpp:1162-1188`) going LIVE against the C++ oracle — **THE MILESTONE of 5'a**.
//! The golden (`golden/sim_slice5prime.txt`, 71 lines for ticks 0..=70) is produced by
//! the real C++ `Game` running the *same scenario* (`golden/sim_slice5prime_scenario.txt`):
//! seed 42, the LOADED `physics_fall_test.lev`, `max_bonuses 0`, both worms carrying
//! `DART` in slot 0. worm0 (SHOOTER, health 100) lands, raises the gun (Up ticks 13-39)
//! and FIRES a single dart (tick 40) that skims LOW-LEFT and transits worm1's 16x16
//! silhouette. worm1 (VICTIM) starts VISIBLE + grounded at health **50**.
//!
//! ## THE PER-PIXEL DISCRIMINATION WITNESS (the fd33bbc / 5b-T5b blocker, FIXED)
//!
//! The dart's `detect_distance = 0`, so `CheckForSpecWormHit` scans a SINGLE pixel per
//! tick and fires the arm iff that pixel is a WORM-material pixel of worm1's sprite.
//! worm1's frame-0/dir-0 silhouette is SOLID only in the upper-centre rows; the rest of
//! the box is TRANSPARENT. As the dart transits worm1's box it lands on:
//!   * tick 41 -> TRANSPARENT  -> NEAR-MISS (a 16x16 box over-approx WOULD fire here).
//!   * tick 42 -> TRANSPARENT  -> NEAR-MISS (box would fire).
//!   * tick 43 -> SOLID        -> CONTACT: the ONLY tick the arm fires. health 50->45,
//!                                vel kicked, 10 blood (type 6) spawn, `rng` BURSTS.
//!   * ticks 45,46,47 -> TRANSPARENT -> NEAR-MISS (dart still in the box, grazing
//!                                transparent pixels).
//! A box over-approximation would fire on EVERY in-bounds tick (41-47), draining worm1
//! far below 45 and spraying ~7 fans. The SINGLE clean -5 + one 10-blood fan, with the
//! `rng` FLAT on the near-miss ticks, is the direct proof the per-pixel predicate
//! discriminates solid from transparent — the anti-false-positive witness. **A box
//! over-approx would FAIL the near-miss ticks (rng burst there -> golden mismatch); the
//! per-pixel port passes them.** This is what a bit-exact match over all 71 ticks proves.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## The 5'a setup — consts post-`new`
//!
//! `SimState::new` defaults the blood consts (`num_blood_colours`/`first_blood_colour`/
//! `bobj_gravity`) to 0; this harness assigns them from the loaded TC AFTER `new` — the
//! blood fan (bobjects + type-6 nobjects) diverges immediately if they are left 0, a
//! forgotten const not a sim bug. The respawn consts are set too (harmless here — no
//! respawn runs) to mirror the unchanged C++ dumper exactly. Both sprite banks + textures
//! are loaded so the landed-blood `draw_on_map` blit (which carves the level at the tail)
//! indexes a real bank.
//!
//! Components are asserted FIRST (rng -> level -> worm0 -> worm1 -> the 5 pools) THEN the
//! master `state_hash`, so a divergence localises to a tick + subsystem before the master
//! flags it. The scenario is the single source of truth (parsed via `oracle_tests::scenario`)
//! and the expected values are PARSED from the golden file, never hard-coded; the coverage
//! guards are read from the genuinely DRIVEN `SimState`, never re-parsed from the golden.

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
/// Read by `DrawDirtEffect`; loaded to match the dumper's `common.large_sprites`.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Load the shipped 7x7 small-sprite bank (C++ `small_sprites.Allocate(7,7,130)`).
/// A landed blood nobject (`draw_on_map`) blits a 7x7 stamp from here — the tail-of-window
/// level carve indexes this bank, so an empty bank would panic.
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
}

/// The empty-pool component hash (FNV-1a of a zero-length pool). Every pool reads this
/// while idle; a pool leaving this value is how we know it went live.
const EMPTY_POOL: u32 = 0x0000_0001;

/// The near-miss ticks named by the scenario: the dart's fixed pixel is INSIDE worm1's
/// 16x16 box but on a TRANSPARENT pixel, so the per-pixel arm must fire NOTHING. Derived
/// straight from the scenario comment; the guard reads the DRIVEN state on these ticks
/// (rng flat + health unchanged + dart present) — the anti-box witness.
const NEAR_MISS_TICKS: [usize; 5] = [41, 42, 45, 46, 47];

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
fn sim_slice5prime_perpixel_worm_hit_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5prime_scenario.txt"
    ))
    .expect("read golden/sim_slice5prime_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 70, "scenario ticks");
    assert_eq!(scenario.max_bonuses, 0, "5'a runs with no bonuses (in-flight hit only)");
    // worm1 (index 1) starts at health 50 so the single 5-damage hit WOUNDS to 45 without
    // killing (the death path is 5d's, already proven); worm0 is the full-health shooter.
    assert_eq!(scenario.worms[1].health, 50, "worm1 (victim) starts at health 50");
    assert_eq!(scenario.worms[0].health, 100, "worm0 (shooter) starts at full health 100");

    // --- Parse the golden vectors (ticks 0..=70, master + 9 components). -----------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5prime.txt"
    ))
    .expect("read golden/sim_slice5prime.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 5'a HEADLINE in the golden, read straight from the parsed golden so a
    // regenerated golden that lost the isolated-hit signature fails loudly HERE. -----
    // The single wound gives worm1 >=2 distinct hashes (alive-flat, then hit+moving).
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(
        worm1_cols.len() >= 2,
        "5'a golden worm1 must show >=2 phases (idle, then wounded+moving); saw {} distinct",
        worm1_cols.len()
    );
    // The dart flies: the wobjects pool goes live.
    assert!(
        golden.iter().any(|g| g.pools[4] != EMPTY_POOL),
        "5'a golden must fly the DART wobject"
    );
    // The blood fan spawns nobjects (type-6 blood) and drips the bobjects pool.
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "5'a golden must spawn nobjects (the 10-blood fan)"
    );
    assert!(
        golden.iter().any(|g| g.pools[0] != EMPTY_POOL),
        "5'a golden must drip the bobjects blood pool"
    );
    // THE ISOLATION INVARIANT: NO explosion mixes in — the sobjects pool stays EMPTY the
    // whole window (`worm_collide=false` dart never explodes; the ONLY damage event is the
    // single per-pixel in-flight hit).
    for g in &golden {
        assert_eq!(
            g.pools[2], EMPTY_POOL,
            "tick {}: sobjects must stay empty (no explosion; isolated per-pixel hit)",
            g.tick
        );
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

    // INVARIANTS: the object/weapon tables are indexed by id (the Fire path, the in-flight
    // per-pixel arm's blood spray, and the blood nobjects all index by id). If id != index
    // those lookups read the wrong object.
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(w.id, i as i32, "weapon id must equal its index (weapon[{i}], id {})", w.id);
    }
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index (got id {})", n.id);
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors Common::Precompute
    // (common.cpp:492-499), exactly as slices 1-5d.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with DART (the scenario `weapon 0` directive), resolving
    // BY NAME against the loaded TC weapon table (id == index). ----------------------
    let weapon_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let weapon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == weapon_name)
        .unwrap_or_else(|| panic!("weapon {weapon_name:?} present in TC weapon table"));
    // The dart's hit_damage — the exact wound the per-pixel arm applies (`DoDamage`).
    let dart_hit_damage = objects.weapons[weapon_idx].hit_damage;
    assert_eq!(dart_hit_damage, 5, "DART hit_damage is 5 (the wound the arm applies)");
    resolved[0] = WeaponInit {
        ty: Some(weapon_idx as WeaponId),
        ammo: objects.weapons[weapon_idx].ammo,
    };

    // Both worms carry DART in slot 0 (the override folds into each worm's hash).
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

    // --- Build tick-0 state (same `new` signature as 5d). Trailing `0, true, 100`
    // = settings_loading_time / load_change / blood, matching the dumper. ------------
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

    // --- THE CRITICAL 5'a STEP: set the blood consts (as 5b/5d). Left at their `new`
    // defaults (0) the blood fan diverges at the first bobject/type-6 tick — a forgotten
    // const, not a sim bug. The respawn consts are mirrored too (harmless: no respawn
    // runs in this wound-only window) to match the unchanged C++ dumper's held consts. --
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    // The 7x7 bank a landed blood nobject blits (the tail-of-window level carve).
    state.small_sprites = load_small_sprites();
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;

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
    let mut worm1_vel: Vec<Vec2> = Vec::with_capacity(golden.len());
    let mut nobj_count: Vec<usize> = Vec::with_capacity(golden.len());
    let mut wobj_count: Vec<usize> = Vec::with_capacity(golden.len());
    let mut rng_draws: Vec<u64> = Vec::with_capacity(golden.len());
    let mut max_nobjects = 0usize;
    let mut bonuses_always_empty = true;
    let mut sobjects_always_empty = true;
    let mut saw_type6_blood_nobject = false;
    let mut record = |state: &SimState| {
        worm1_health.push(state.worms[1].health);
        worm1_vel.push(state.worms[1].vel);
        nobj_count.push(state.nobjects.len());
        wobj_count.push(state.wobjects.len());
        rng_draws.push(state.rand.draws());
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
        }
        if !state.sobjects.is_empty() {
            sobjects_always_empty = false;
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

    // ================= THE 5'a COVERAGE GUARDS (all from the DRIVEN state) ==========

    // --- CONTACT witness: worm1 wounds exactly ONCE, by exactly hit_damage. ---------
    assert_eq!(worm1_health[0], 50, "worm1 starts at health 50 (from the scenario)");
    // The contact tick = the first (and only) tick worm1 health drops.
    let contact_tick = (1..worm1_health.len())
        .find(|&k| worm1_health[k] < worm1_health[k - 1])
        .expect("worm1 must be WOUNDED (health drops on the per-pixel contact tick)");
    // The drop is EXACTLY hit_damage (5): 50 -> 45.
    assert_eq!(
        worm1_health[contact_tick - 1] - worm1_health[contact_tick],
        dart_hit_damage,
        "the contact tick must drop worm1 health by exactly hit_damage ({dart_hit_damage})"
    );
    assert_eq!(worm1_health[contact_tick], 45, "worm1 is wounded to 45 (50 - 5)");
    // worm1 NEVER drops again — the per-pixel predicate fired ONCE (not a box that would
    // fire every in-bounds tick and drain worm1 far below 45).
    for k in (contact_tick + 1)..worm1_health.len() {
        assert_eq!(
            worm1_health[k], 45,
            "tick {k}: worm1 health must stay FLAT at 45 (the arm fired exactly once)"
        );
    }
    assert_eq!(
        *worm1_health.iter().min().unwrap(),
        45,
        "worm1 health floor is 45 (a single -5 wound, never killed, never over-drained)"
    );

    // --- vel-KICK witness: the arm's `blow_away` impulse changes worm1's velocity on the
    // contact tick (a genuine driven-state vector delta, not a golden re-parse). ------
    assert_ne!(
        worm1_vel[contact_tick], worm1_vel[contact_tick - 1],
        "the per-pixel arm must KICK worm1 velocity on the contact tick (blow_away impulse)"
    );

    // --- BLOOD-FAN witness: the contact tick spawns the 10-blood fan (type-6 nobjects),
    // non-empty and under the O3 pool cap. ------------------------------------------
    assert_eq!(
        nobj_count[contact_tick - 1],
        0,
        "no nobjects before contact (no false-positive fan on the near-miss ticks)"
    );
    assert!(
        nobj_count[contact_tick] > 0,
        "the contact tick must spawn the blood fan (nobjects non-empty)"
    );
    assert!(saw_type6_blood_nobject, "the blood fan must include type-6 (blood) nobjects");
    assert!(max_nobjects < 600, "nobjects must stay under the 600 cap (O3); peaked at {max_nobjects}");

    // --- RNG BURST on the contact tick: the 10-blood fan (10*[rand(128)+Create2] + the
    // rand(3) hit-sound gate) draws a large burst vs the flat surrounding ticks. Read from
    // the monotonic draw counter (diagnostic-only, not hashed). ----------------------
    let contact_burst = rng_draws[contact_tick] - rng_draws[contact_tick - 1];
    assert!(
        contact_burst > 20,
        "the blood fan must BURST the rng on the contact tick (>20 draws); saw {contact_burst}"
    );

    // ============ THE PER-PIXEL DISCRIMINATION WITNESS (fd33bbc, FIXED) =============
    // On each NEAR-MISS tick the dart's fixed pixel is INSIDE worm1's 16x16 box (the dart
    // wobject is alive) but on a TRANSPARENT pixel, so the per-pixel arm fires NOTHING:
    //   * the rng is FLAT (ZERO draws) — a box over-approx would burst a fan here;
    //   * worm1 health is UNCHANGED — a box would wound worm1 on every in-bounds tick.
    // This directly pins the box-over-approx blocker as FIXED. -----------------------
    assert!(!NEAR_MISS_TICKS.contains(&contact_tick), "contact tick is not a near-miss");
    for &nm in &NEAR_MISS_TICKS {
        // The dart wobject is present (in flight, crossing worm1's box) on this tick —
        // makes the "no fire" witness non-vacuous (there IS a projectile in the box).
        assert!(
            wobj_count[nm] > 0,
            "tick {nm}: the DART wobject must be in flight (present in worm1's box) — else the \
             near-miss witness is vacuous"
        );
        // ZERO rng draws on the near-miss tick: the arm did not fire.
        let nm_draws = rng_draws[nm] - rng_draws[nm - 1];
        assert_eq!(
            nm_draws, 0,
            "tick {nm}: NEAR-MISS must draw ZERO rng (per-pixel arm did NOT fire; a box would \
             burst a fan here — the anti-false-positive witness)"
        );
        // worm1 health unchanged across the near-miss tick.
        assert_eq!(
            worm1_health[nm], worm1_health[nm - 1],
            "tick {nm}: NEAR-MISS must leave worm1 health UNCHANGED (a box would wound it here)"
        );
    }
    // And the contrast is real: the contact tick is sandwiched by near-miss ticks (42 and
    // 45 are both flat), so the ONE burst is genuinely the single solid-pixel hit.
    assert!(
        NEAR_MISS_TICKS.iter().any(|&nm| nm < contact_tick)
            && NEAR_MISS_TICKS.iter().any(|&nm| nm > contact_tick),
        "the contact tick must be flanked by near-miss ticks (isolated single per-pixel hit)"
    );

    // The window carved terrain (landed blood) and never spawned an explosion or a bonus.
    let level_seen: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(level_seen.len() >= 2, "terrain must carve (landed blood; >=2 distinct level hashes)");
    assert!(sobjects_always_empty, "sobjects must stay empty (no explosion — isolated per-pixel hit)");
    assert!(bonuses_always_empty, "bonuses must stay empty (max_bonuses 0)");
}
