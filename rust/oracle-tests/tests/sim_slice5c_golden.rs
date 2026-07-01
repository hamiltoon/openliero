//! Per-tick differential test for the Slice-5c **bonuses pool going LIVE** against the
//! C++ oracle — THE MILESTONE of 5c. The golden (`golden/sim_slice5c.txt`, 501 lines for
//! ticks 0..=500) is produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice5c_scenario.txt`): seed 42, the LOADED `physics_fall_test.lev`,
//! `max_bonuses 4`, NO weapon, both worms VISIBLE + grounded + stationary at the level
//! edges (x=60, x=462). With `max_bonuses > 0` the per-tick **bonus-drop roll**
//! (`game.cpp:359`, T0) draws `rand(CBonusDropChance)` (CBonusDropChance=1700) every tick;
//! under seed 42 the first 0-roll lands at **tick 252**, where `Game::CreateBonus` (T2)
//! drops a frame-1 (health) bonus that then FALLS + bounces under `Bonus::Process` (T3,
//! `BonusGravity=1500`, `BounceMul/Div=40/100`). The bonus's hash folds `x,y,timer,weapon,
//! frame`, so the `bonuses` column leaves the empty-pool hash at tick 252 and EVOLVES every
//! tick after — first the fall (x,y move), then a steady `--timer` countdown once settled.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — and why the window is CLEAN
//!
//! A bit-exact match over 501 ticks proves the T0 gated bonus-drop roll, the T2
//! `CreateBonus` placement search (`rand(BonusSpawnRectW)`+`rand(BonusSpawnRectH)` per
//! trial → `rand(2)` frame → `rand(timer_v)` timer → the `frame==0` weapon `do/while`),
//! the spawn flash, AND the T3 `Bonus::Process` fall/gravity/bounce/timer port end-to-end
//! vs the C++ oracle. The ONLY sobject is the bonus **spawn flash** `sobject_types[7]` =
//! teleport_flash (`detect_range=0`, `damage=0`, `num_sounds=0`): RNG-free AND the deferred
//! chain-loop (`sobject.cpp:217-227`) is **INERT** — its detect box `kIx > x-0 && kIx <
//! x+0` is empty, so no flash ever catches a bonus. **The all-ticks bit-exact match IS the
//! proof that no chain-loop fired** (a divergence on `rng`/`sobjects`/`bonuses` at a flash
//! tick would mean a bonus was caught → the chain-loop would then need porting, out of 5c
//! scope).
//!
//! The window stops at tick 500 — well before (a) the SECOND roll-drop (~tick 2096) and
//! (b) this bonus's timer-EXPIRY (`timer=rand(2000)+2000`, so ~tick 2250-4250). The plan
//! marks expiry "(optionally)"; reaching it needs a multi-thousand-tick golden, and the
//! LONG run also drops further bonuses whose frame-0 expiry spawns a large_explosion
//! (`detect_range=20`) that carves dirt AND arms the deferred chain-loop. So 5c captures
//! the clean LIVE portion: a single bonus, drop → fall → settle → timer countdown, with
//! `level` CONSTANT, no damage, both worms FLAT.
//!
//! ## The critical 5c step — set ALL the bonus consts post-`new`
//!
//! `SimState::new` defaults every bonus const to 0/false/empty (they are TC constants /
//! `Settings::max_bonuses`, not in the `new` arg list — mirrors 5b's blood consts and 4d's
//! `small_sprites`). This harness assigns them from the loaded TC AFTER `new` — the exact
//! values the C++ dumper's `common->c[...]` / `common->h[...]` / `common->bonus_rand_timer`
//! / `common->bonus_s_objects` / `settings->weap_table` hold. **If any is left default-0 the
//! difftest diverges at the drop tick (252) — that means a const was forgotten, not a sim
//! bug.** `settings_max_bonuses` comes from the scenario's `max_bonuses` directive; the rest
//! from `tc.constants.*` / `tc.hacks.*`. `weap_table` mirrors the dumper's default
//! `Settings` (`memset 0`): all-zero, so the weapon `do/while` never rejects.
//!
//! The components are asserted FIRST (rng → level → worm0 → worm1 → the 5 pools) THEN the
//! master `state_hash`, so a divergence localises to a tick + subsystem before the master
//! flags it. The scenario is the single source of truth (parsed via `oracle_tests::scenario`)
//! and the expected values are PARSED from the golden file, never hard-coded.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// Load the shipped 16x16 large-sprite bank (C++ `large_sprites.Allocate(16,16,110)`).
/// Threaded into `SimState` for `draw_dirt_effect`; inert in 5c (no carving) but the
/// `SimState::new` signature requires it.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// The empty-pool component hash (FNV-1a of a zero-length pool). Every pool reads this
/// while idle; the `bonuses` pool leaving this value is how we know it went live.
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
fn sim_slice5c_bonuses_pool_goes_live_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5c_scenario.txt"
    ))
    .expect("read golden/sim_slice5c_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 500, "scenario ticks");
    assert_eq!(scenario.max_bonuses, 4, "scenario max_bonuses opens the bonus-drop roll");
    assert!(
        scenario.weapon(0).is_none(),
        "5c fires NO weapon (no `weapon` directive)"
    );

    // --- Parse the golden vectors (ticks 0..=500, master + 9 components). ------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5c.txt"
    ))
    .expect("read golden/sim_slice5c.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 5c HEADLINE in the golden: the `bonuses` pool goes LIVE (leaves the
    // empty-pool hash) AND a flash sobject appears, WHILE the level stays CONSTANT and
    // bobjects/nobjects/wobjects stay empty (a clean drop+fall, no carve, no blood, no
    // splinters). Read straight from the parsed golden so a regenerated golden that lost
    // the bonus drop — or that drifted into the messy expiry/chain-loop window — fails
    // loudly HERE before the per-tick loop. -----------------------------------------
    assert!(
        golden.iter().any(|g| g.pools[1] != EMPTY_POOL),
        "5c golden must drop a bonus (bon column leaves the empty-pool hash)"
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "5c golden must spawn the bonus flash sobject (sob column leaves the empty-pool hash)"
    );
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert_eq!(
        golden_levels.len(),
        1,
        "5c golden level must be CONSTANT (no carve in the clean window); saw {:?}",
        golden_levels
    );
    for g in &golden {
        assert_eq!(g.pools[0], EMPTY_POOL, "tick {}: bobjects must stay empty (no blood)", g.tick);
        assert_eq!(g.pools[3], EMPTY_POOL, "tick {}: nobjects must stay empty", g.tick);
        assert_eq!(g.pools[4], EMPTY_POOL, "tick {}: wobjects must stay empty (no weapon)", g.tick);
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

    // INVARIANT: object/weapon tables indexed by id (the bonus weapon draw, the flash +
    // expiry sobject lookups, and the dirt-throw all index by id).
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
    // (common.cpp:492-499), exactly as slices 1-5b.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0. No
    // `weapon` override in 5c, so the worms keep their default InitWeapons loadout (the
    // dumper's path before the empty weapon_overrides loop).
    let settings_weapons = [1u32; NUM_WEAPONS];
    let resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

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

    // --- Build tick-0 state (same `new` signature as 5b). The trailing `0, true, 100`
    // are settings_loading_time / load_change / blood — matching the dumper. The blood
    // consts (`num_blood_colours`/`first_blood_colour`/`bobj_gravity`) are left at the
    // `new` default 0: no blood spawns in 5c, so they are never read. ----------------
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

    // --- THE CRITICAL 5c STEP: set ALL the bonus consts post-`new` (defaulted to
    // 0/false/empty by `SimState::new`). `settings_max_bonuses` is the scenario's
    // `max_bonuses`; the rest are the exact `LC(...)` / `h[...]` / `bonus_rand_timer` /
    // `bonus_s_objects` / `weap_table` values the C++ dumper holds. LEFT AT 0 the run
    // diverges at the drop tick (252). --------------------------------------------
    state.settings_max_bonuses = scenario.max_bonuses;
    state.bonus_drop_chance = tc.constants.BonusDropChance;
    state.bonus_spawn_rect_w = tc.constants.BonusSpawnRectW;
    state.bonus_spawn_rect_h = tc.constants.BonusSpawnRectH;
    state.bonus_spawn_rect_x = tc.constants.BonusSpawnRectX;
    state.bonus_spawn_rect_y = tc.constants.BonusSpawnRectY;
    state.h_bonus_spawn_rect = tc.hacks.BonusSpawnRect;
    state.h_bonus_only_health = tc.hacks.BonusOnlyHealth;
    state.h_bonus_only_weapon = tc.hacks.BonusOnlyWeapon;
    state.h_bonus_disable = tc.hacks.BonusDisable;
    // bonus_rand_timer[frame] = [timer, timer_v] from constants.bonuses[frame].
    assert!(
        tc.bonuses.len() >= 2,
        "TC must define 2 bonus types (weapon, health)"
    );
    state.bonus_rand_timer = [
        [tc.bonuses[0].timer, tc.bonuses[0].timer_v],
        [tc.bonuses[1].timer, tc.bonuses[1].timer_v],
    ];
    // bonus_s_objects[frame] = the resolved expiry-sobject index (frame 0 -> large_explosion
    // id 0; frame 1 -> zimm_flash id 4). Inert in the clean window (no expiry reached).
    state.bonus_s_objects = [tc.bonuses[0].sobj, tc.bonuses[1].sobj];
    // weap_table mirrors the dumper's default `Settings` (memset 0): all-zero, so the
    // `frame==0` weapon `do/while` (`while weap_table[w] == 2`) never rejects. Sized to
    // the weapon table so the drawn `rand(weapons.len())` index is always in range.
    state.bonus_gravity = tc.constants.BonusGravity;
    state.bonus_bounce_mul = tc.constants.BonusBounceMul;
    state.bonus_bounce_div = tc.constants.BonusBounceDiv;
    state.weap_table = vec![0i32; objects.weapons.len()];

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(got, want, "tick {tick}: {name}: got {got:08x} expected {want:08x}");
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, the 5 pools incl. bonuses) THEN
    // the master, so a divergence localises to a tick + subsystem before the master flags
    // it. The bonus component fold reads `x,y,timer,weapon,frame`; a `vel_y` desync shows
    // only in the master (O11-style: the component drops `vel_y`).
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

    // --- Coverage guards read from the DRIVEN SimState (genuine witnesses), never
    // re-parsed from the golden. --------------------------------------------------
    let mut bonus_count_by_tick: Vec<usize> = Vec::with_capacity(golden.len());
    let mut bonus_timer_by_tick: Vec<Option<i32>> = Vec::with_capacity(golden.len());
    let mut max_bonuses_live = 0usize;
    let mut worm0_health_always_100 = true;
    let mut worm1_health_always_100 = true;
    let mut saw_bonus_flash = false; // sobject id 7 = teleport_flash, the spawn flash
    let mut bobjects_always_empty = true;
    let mut nobjects_always_empty = true;
    let mut wobjects_always_empty = true;
    let mut dropped_bonus_frame: Option<i32> = None;
    let mut record = |state: &SimState| {
        bonus_count_by_tick.push(state.bonuses.len());
        // The single dropped bonus's timer (when present) — its countdown is the T3
        // witness. `iter().next()` is the only live bonus in this scenario.
        let b = state.bonuses.iter().next();
        bonus_timer_by_tick.push(b.map(|b| b.timer));
        if let Some(b) = b {
            dropped_bonus_frame = Some(b.frame);
        }
        max_bonuses_live = max_bonuses_live.max(state.bonuses.len());
        if state.worms[0].health != 100 {
            worm0_health_always_100 = false;
        }
        if state.worms[1].health != 100 {
            worm1_health_always_100 = false;
        }
        if state.sobjects.iter().any(|s| s.id == 7) {
            saw_bonus_flash = true;
        }
        if !state.bobjects.is_empty() {
            bobjects_always_empty = false;
        }
        if !state.nobjects.is_empty() {
            nobjects_always_empty = false;
        }
        if !state.wobjects.is_empty() {
            wobjects_always_empty = false;
        }
    };
    record(&state);

    // --- Drive each subsequent tick under EMPTY scripted input. THE OFF-BY-ONE: golden
    // line `k` (k>=1) is the result of applying input[k-1] on the pass advancing tick
    // k-1 -> k. ------------------------------------------------------------------
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // --- THE 5c LIVE GUARDS (from the DRIVEN SimState). --------------------------
    // The pool goes LIVE: it is empty at tick 0 and non-empty at the drop tick.
    assert_eq!(bonus_count_by_tick[0], 0, "bonuses pool empty at tick 0");
    let drop_tick = (1..bonus_count_by_tick.len())
        .find(|&k| bonus_count_by_tick[k] > 0)
        .expect("a bonus must drop (the bonuses pool goes non-empty)");
    assert!(
        max_bonuses_live >= 1,
        "the bonuses pool must go live (count >= 1 after the drop); peaked at {max_bonuses_live}"
    );
    // Exactly ONE bonus lives in the clean window (the second roll-drop is ~tick 2096,
    // far past tick 500) — and it never frees (expiry is ~tick 2861).
    assert_eq!(
        max_bonuses_live, 1,
        "the clean window holds exactly ONE bonus (no 2nd drop, no expiry); peaked at {max_bonuses_live}"
    );
    for (k, &n) in bonus_count_by_tick.iter().enumerate().skip(drop_tick) {
        assert_eq!(n, 1, "tick {k}: the dropped bonus persists (1 live, not freed)");
    }
    // The dropped bonus is a frame-1 (HEALTH) bonus — its expiry sobject is
    // `bonus_s_objects[1]` = zimm_flash (id 4, detect_range=0 => inert), out of the clean
    // window. (frame 1 also means the weapon `do/while` is never reached, so `weap_table`
    // is inert here — it is still set for faithfulness.)
    assert_eq!(
        dropped_bonus_frame,
        Some(1),
        "the seed-42 drop is a frame-1 (health) bonus"
    );

    // T3 witness: the bonus `timer` strictly DECREMENTS by 1 each tick after the drop
    // (`--timer` in `Bonus::Process`), and stays well above 0 (no expiry in-window).
    let drop_timer = bonus_timer_by_tick[drop_tick].expect("bonus present at drop tick");
    for k in (drop_tick + 1)..bonus_timer_by_tick.len() {
        let prev = bonus_timer_by_tick[k - 1].expect("bonus present");
        let cur = bonus_timer_by_tick[k].expect("bonus present");
        assert_eq!(cur, prev - 1, "tick {k}: bonus timer must decrement by exactly 1");
    }
    let final_timer = bonus_timer_by_tick.last().unwrap().expect("bonus present at end");
    assert!(
        final_timer > 0,
        "bonus must NOT expire in the clean window (timer stays > 0); drop_timer={drop_timer}, final={final_timer}"
    );

    // The bonus spawn FLASH appeared (sobject id 7 = teleport_flash).
    assert!(
        saw_bonus_flash,
        "the bonus spawn flash (sobject id 7 = teleport_flash) must appear"
    );

    // No worm is touched: both stay health 100 every tick (no pickup, no damage), and
    // their component columns are CONSTANT (flat — stationary, no RNG, no motion).
    assert!(worm0_health_always_100, "worm0 health must stay 100 (no pickup/damage)");
    assert!(worm1_health_always_100, "worm1 health must stay 100 (no pickup/damage)");
    let worm0_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm0).collect();
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert_eq!(worm0_cols.len(), 1, "worm0 column must be FLAT (constant); saw {:?}", worm0_cols);
    assert_eq!(worm1_cols.len(), 1, "worm1 column must be FLAT (constant); saw {:?}", worm1_cols);

    // The clean window: no blood/dirt/projectiles ever, level never carves.
    assert!(bobjects_always_empty, "bobjects must stay empty (no blood)");
    assert!(nobjects_always_empty, "nobjects must stay empty");
    assert!(wobjects_always_empty, "wobjects must stay empty (no weapon)");
}
