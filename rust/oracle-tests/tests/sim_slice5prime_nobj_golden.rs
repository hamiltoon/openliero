//! Per-tick differential test for Slice-5'a T7 — the per-pixel IN-FLIGHT **nobject**
//! worm-hit arm (`nobject.cpp:166-203` + the `CheckForSpecWormHit` predicate,
//! `worm.cpp:1162-1188`) going LIVE against the C++ oracle — **THE nobject-arm
//! MILESTONE of 5'a**, the MIRROR of T6's wobject-arm golden.
//!
//! The golden (`golden/sim_slice5prime_nobj.txt`, 156 lines for ticks 0..=155) is
//! produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice5prime_nobj_scenario.txt`): seed 42, the LOADED
//! `physics_fall_test.lev`, `max_bonuses 0`, both worms carrying `CANNON` in slot 0.
//! worm0 (SHOOTER, health 100) lands, raises the gun (Up ticks 13-46) and FIRES a
//! single cannon shell (tick 47) that flies flat-LEFT ~14px ABOVE worm1's head
//! (clearing it — NOT a wobject hit), strikes the LEFT WALL at (2,179), and there
//! `medium_explosion` spawns + BlowUpObject scatters 5 SPLINTERS
//! (`particle__small_damage`, nobject_types[3]: `hit_damage=2`, `detect_distance=0`,
//! `worm_destroy=true`). One splinter flies back EAST at worm-height and transits
//! worm1's 16x16 silhouette. worm1 (VICTIM) starts VISIBLE + grounded at health **50**.
//!
//! ## THE nobject RNG ORDER: SOUND BEFORE BLOOD (the mirror of T6)
//!
//! The nobject arm draws the **hit-sound gate FIRST** (`rand(3)`; on 0 also `rand(3)`)
//! **THEN the blood fan** (`blood_on_hit * blood / 100` × [`rand(128)` + `Create2`]) —
//! the OPPOSITE order to the wobject arm (T6's dart: blood-then-sound). A bit-exact
//! match of the `rng` component on the contact tick is the direct proof the two arms'
//! transposed RNG orders are BOTH faithful (a blood-first port would produce a
//! different `rng` hash here and FAIL). This asymmetry is load-bearing and named by
//! this milestone.
//!
//! ## THE PER-PIXEL DISCRIMINATION WITNESS (near-miss + contact in ONE run)
//!
//! The splinter's `detect_distance = 0`, so `CheckForSpecWormHit` scans a SINGLE pixel
//! per tick. As the east-bound splinter transits worm1's box it grazes SIX TRANSPARENT
//! pixels before landing on a solid one:
//!   * ticks 140-145 -> TRANSPARENT -> NEAR-MISS (a 16x16 box over-approx WOULD fire on
//!                      every one of these six ticks). `rng`/`worm1` FLAT.
//!   * tick 146       -> SOLID       -> CONTACT: the ONLY tick the arm fires. health
//!                      50->48, vel kicked, the sound gate + 1-blood fan burst `rng`,
//!                      the splinter is FREED (worm_destroy, no small_explosion), 1
//!                      blood (type 6) nobject spawns.
//! A box over-approximation would fire on EVERY in-box tick (140-146), draining worm1
//! far below 48. The SINGLE clean -2, with `rng` FLAT on the six near-miss ticks, is
//! the anti-false-positive witness — now exercised through the NOBJECT arm. **A box
//! over-approx would FAIL the near-miss ticks (rng burst there -> golden mismatch); the
//! per-pixel port passes them.** This is what a bit-exact match over all 156 ticks
//! proves.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
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
use sim_core::fixed::ftoi;
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`).
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Load the shipped 7x7 small-sprite bank (C++ `small_sprites.Allocate(7,7,130)`).
/// A landed blood/debris nobject (`draw_on_map`) and a spent-shell blit index this bank.
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
}

/// The empty-pool component hash (FNV-1a of a zero-length pool).
const EMPTY_POOL: u32 = 0x0000_0001;

/// The near-miss ticks named by the scenario: a type-3 splinter's fixed pixel is INSIDE
/// worm1's 16x16 box but on a TRANSPARENT pixel, so the per-pixel arm must fire NOTHING.
/// The guard reads the DRIVEN state on these ticks (rng flat + health unchanged +
/// splinter in-box) — the anti-box witness for the NOBJECT arm.
const NEAR_MISS_TICKS: [usize; 6] = [140, 141, 142, 143, 144, 145];

/// The splinter nobject type id (`particle__small_damage`) — the damaging in-flight
/// nobject whose per-pixel hit is the milestone surface. Index into the TC nobject list
/// (`worm_1_parts, worm_2_parts, particle__disappearing, particle__small_damage, ...`).
const SPLINTER_TY: i32 = 3;

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

/// Does any type-3 splinter's Ftoi pixel land inside worm1's 16x16 sprite box (top-left
/// at `Ftoi(pos) - (7, 5)`)? A 16x16 box over-approximation would fire the arm whenever
/// this is true; the per-pixel predicate fires only on a SOLID pixel. Used to make the
/// near-miss guard non-vacuous (there IS a splinter in the box on those ticks).
fn splinter_in_worm_box(state: &SimState) -> bool {
    let w1 = &state.worms[1];
    let bx = ftoi(w1.pos.x) - 7;
    let by = ftoi(w1.pos.y) - 5;
    state.nobjects.iter().any(|n| {
        if n.ty != Some(SPLINTER_TY) {
            return false;
        }
        let dx = ftoi(n.pos.x) - bx;
        let dy = ftoi(n.pos.y) - by;
        (0..16).contains(&dx) && (0..16).contains(&dy)
    })
}

#[test]
fn sim_slice5prime_nobj_perpixel_worm_hit_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5prime_nobj_scenario.txt"
    ))
    .expect("read golden/sim_slice5prime_nobj_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 155, "scenario ticks");
    assert_eq!(scenario.max_bonuses, 0, "T7 runs with no bonuses (in-flight hit only)");
    assert_eq!(scenario.worms[1].health, 50, "worm1 (victim) starts at health 50");
    assert_eq!(scenario.worms[0].health, 100, "worm0 (shooter) starts at full health 100");

    // --- Parse the golden vectors (ticks 0..=155, master + 9 components). -----------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5prime_nobj.txt"
    ))
    .expect("read golden/sim_slice5prime_nobj.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE T7 HEADLINE in the golden, read straight from the parsed golden. --------
    // The single wound gives worm1 >=2 distinct hashes (idle-flat, then hit+moving).
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(
        worm1_cols.len() >= 2,
        "T7 golden worm1 must show >=2 phases (idle, then wounded+moving); saw {} distinct",
        worm1_cols.len()
    );
    // The shell flies: the wobjects pool goes live.
    assert!(
        golden.iter().any(|g| g.pools[4] != EMPTY_POOL),
        "T7 golden must fly the CANNON shell wobject"
    );
    // The explosion + splinters spawn nobjects and sobjects.
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "T7 golden must spawn nobjects (splinters + debris + blood)"
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "T7 golden must spawn sobjects (the medium/small explosions)"
    );
    // bonuses stay empty the whole window (max_bonuses 0).
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

    // INVARIANTS: the object/weapon tables are indexed by id (the Fire path, the in-flight
    // per-pixel arm's blood spray, the splinter scatter, and the blood nobjects all index
    // by id). If id != index those lookups read the wrong object.
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
    // (common.cpp:492-499), exactly as slices 1-5prime.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with CANNON (the scenario `weapon 0` directive),
    // resolving BY NAME against the loaded TC weapon table (id == index). -----------
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

    // The SPLINTER's hit_damage — the exact wound the per-pixel NOBJECT arm applies
    // (`DoDamage`). Read from the nobject type table, not hard-coded.
    let splinter_hit_damage = objects.nobject_types[SPLINTER_TY as usize].hit_damage;
    assert_eq!(
        splinter_hit_damage, 2,
        "particle__small_damage hit_damage is 2 (the wound the nobject arm applies)"
    );

    // Both worms carry CANNON in slot 0 (the override folds into each worm's hash).
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

    // --- Build tick-0 state (same `new` signature as 5prime). ------------------------
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

    // --- Set the blood consts (as 5b/5d/5prime). Left at their `new` defaults (0) the
    // blood fan diverges at the first bobject/type-6 tick — a forgotten const, not a sim
    // bug. The respawn consts are mirrored too to match the unchanged C++ dumper. -----
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    // The 7x7 bank a landed blood/debris nobject blits (the terrain carve near the wall).
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

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, the 5 pools) THEN the master.
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

    // --- Coverage witnesses read from the genuinely DRIVEN SimState. ----------------
    let mut worm1_health: Vec<i32> = Vec::with_capacity(golden.len());
    let mut worm1_vel: Vec<Vec2> = Vec::with_capacity(golden.len());
    let mut nobj_count: Vec<usize> = Vec::with_capacity(golden.len());
    let mut wobj_count: Vec<usize> = Vec::with_capacity(golden.len());
    let mut rng_draws: Vec<u64> = Vec::with_capacity(golden.len());
    let mut splinter_in_box: Vec<bool> = Vec::with_capacity(golden.len());
    let mut max_nobjects = 0usize;
    let mut bonuses_always_empty = true;
    let mut saw_type6_blood_nobject = false;
    let mut saw_sobject = false;
    let mut record = |state: &SimState| {
        worm1_health.push(state.worms[1].health);
        worm1_vel.push(state.worms[1].vel);
        nobj_count.push(state.nobjects.len());
        wobj_count.push(state.wobjects.len());
        rng_draws.push(state.rand.draws());
        splinter_in_box.push(splinter_in_worm_box(state));
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
        }
        if !state.sobjects.is_empty() {
            saw_sobject = true;
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

    // ============ THE T7 COVERAGE GUARDS (all from the DRIVEN state) ================

    // --- CONTACT witness: worm1 wounds exactly ONCE, by exactly the splinter's
    // hit_damage (2). --------------------------------------------------------------
    assert_eq!(worm1_health[0], 50, "worm1 starts at health 50 (from the scenario)");
    let contact_tick = (1..worm1_health.len())
        .find(|&k| worm1_health[k] < worm1_health[k - 1])
        .expect("worm1 must be WOUNDED (health drops on the per-pixel splinter contact tick)");
    assert_eq!(
        worm1_health[contact_tick - 1] - worm1_health[contact_tick],
        splinter_hit_damage,
        "the contact tick must drop worm1 health by exactly the splinter hit_damage ({splinter_hit_damage})"
    );
    assert_eq!(worm1_health[contact_tick], 48, "worm1 is wounded to 48 (50 - 2)");
    // worm1 NEVER drops again — the per-pixel predicate fired ONCE (not a box that would
    // fire every in-box tick and drain worm1 far below 48; and the splinter is freed by
    // worm_destroy so it spawns NO small_explosion on worm1).
    for k in (contact_tick + 1)..worm1_health.len() {
        assert_eq!(
            worm1_health[k], 48,
            "tick {k}: worm1 health must stay FLAT at 48 (the arm fired exactly once)"
        );
    }
    assert_eq!(
        *worm1_health.iter().min().unwrap(),
        48,
        "worm1 health floor is 48 (a single -2 wound, never killed, never over-drained)"
    );

    // --- vel-KICK witness: the arm's `blow_away` impulse changes worm1's velocity on the
    // contact tick. -----------------------------------------------------------------
    assert_ne!(
        worm1_vel[contact_tick], worm1_vel[contact_tick - 1],
        "the per-pixel NOBJECT arm must KICK worm1 velocity on the contact tick (blow_away)"
    );

    // --- BLOOD-FAN witness: the contact tick spawns the type-6 blood fan; the splinter
    // scatter kept nobjects well under the O3 cap the whole window. ------------------
    assert!(saw_type6_blood_nobject, "the blood fan must include type-6 (blood) nobjects");
    assert!(saw_sobject, "the shell + splinters must spawn sobjects (medium/small explosions)");
    assert!(max_nobjects < 600, "nobjects must stay under the 600 cap (O3); peaked at {max_nobjects}");

    // --- RNG BURST on the contact tick: the sound gate (rand(3) [+ rand(3)]) + the
    // 1-blood fan (rand(128) + Create2's rand(speed_v) + two rand(dist*2)) draw a burst
    // vs the FLAT surrounding near-miss ticks. Read from the monotonic draw counter. --
    let contact_burst = rng_draws[contact_tick] - rng_draws[contact_tick - 1];
    assert!(
        contact_burst >= 4,
        "the nobject arm must BURST the rng on the contact tick (sound gate + blood fan, \
         >=4 draws); saw {contact_burst}"
    );

    // ============ THE PER-PIXEL DISCRIMINATION WITNESS (fd33bbc, FIXED) =============
    // On each NEAR-MISS tick a type-3 splinter's fixed pixel is INSIDE worm1's 16x16 box
    // but on a TRANSPARENT pixel, so the per-pixel arm fires NOTHING:
    //   * the rng is FLAT (ZERO draws) — a box over-approx would burst here;
    //   * worm1 health is UNCHANGED — a box would wound worm1 on every in-box tick;
    //   * a splinter IS in the box (non-vacuous — there is a projectile to (not) fire on).
    assert!(!NEAR_MISS_TICKS.contains(&contact_tick), "contact tick is not a near-miss");
    for &nm in &NEAR_MISS_TICKS {
        assert!(
            splinter_in_box[nm],
            "tick {nm}: a type-3 splinter must be INSIDE worm1's 16x16 box (else the \
             near-miss witness is vacuous — nothing for a box over-approx to fire on)"
        );
        let nm_draws = rng_draws[nm] - rng_draws[nm - 1];
        assert_eq!(
            nm_draws, 0,
            "tick {nm}: NEAR-MISS must draw ZERO rng (per-pixel arm did NOT fire; a box would \
             burst a fan here — the anti-false-positive witness)"
        );
        assert_eq!(
            worm1_health[nm], worm1_health[nm - 1],
            "tick {nm}: NEAR-MISS must leave worm1 health UNCHANGED (a box would wound it here)"
        );
    }
    // The contrast is real: the contact tick is flanked by near-miss ticks (all six
    // precede it here), so the ONE burst is genuinely the single solid-pixel hit.
    assert!(
        NEAR_MISS_TICKS.iter().all(|&nm| nm < contact_tick)
            && contact_tick - NEAR_MISS_TICKS.iter().max().unwrap() == 1,
        "the contact tick must immediately follow the six-tick near-miss graze (isolated hit)"
    );
    // The splinter that fired is GONE on the contact tick (worm_destroy freed it) — the
    // in-box splinter of the graze is not still there wounding again.
    assert!(
        !splinter_in_box[contact_tick] || nobj_count[contact_tick] > 0,
        "sanity: the contact tick advanced the nobject pool (splinter freed + blood spawned)"
    );

    // The window carved terrain near the wall and never spawned a bonus.
    let level_seen: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(level_seen.len() >= 2, "terrain must carve (wall explosion; >=2 distinct level hashes)");
    assert!(bonuses_always_empty, "bonuses must stay empty (max_bonuses 0)");
    let _ = wobj_count;
}
