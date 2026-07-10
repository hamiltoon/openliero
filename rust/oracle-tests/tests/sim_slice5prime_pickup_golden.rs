//! Per-tick differential test for Slice-5'b (T9) — the visible-worm **bonus PICKUP**
//! block (`worm.cpp:287-322`, ported T8) going LIVE in a full walk-on scenario against
//! the C++ oracle — **THE MILESTONE of 5'b**. Two variants, each its own scenario +
//! golden, both asserted bit-exact (master + 9 components) EVERY tick:
//!
//!   * **variant a (HEALTH)** — `sim_slice5prime_pickup_health*`: worm0, WOUNDED to
//!     health 50, walks RIGHT onto a dropped FRAME-1 (health) bonus (seed 29, drop tick
//!     41, ix=199) and HEALS on the WALK-ON tick 87 (`health < settings->health` opens
//!     the gate → Free + one `rand(BonusHealthVar)` → `DoHealingDirect`). worm0 health
//!     JUMPS UP; the `bonuses` pool shrinks 1→0.
//!   * **variant b (WEAPON/RELOAD)** — `sim_slice5prime_pickup_weapon*`: worm0 walks
//!     RIGHT onto a dropped FRAME-0 (weapon) bonus (seed 27, drop tick 44, ix=394); on
//!     the WALK-ON tick 67 `rand(BonusExplodeRisk)` rolls > 1 → the RELOAD branch
//!     (`ww.type`/`ww.ammo` flip to `weapons[bonus.weapon]`, `fire_cone=0`,
//!     `loading_left=0`; the bonus is Freed). worm0's current weapon CHANGES; health is
//!     UNCHANGED (the reload path never damages — the discriminator vs the booby branch,
//!     which stays unit-test-only per T8). The `bonuses` pool shrinks 1→0.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves
//!
//! Walking draws NO rand, so the bonus DROP (roll + `CreateBonus` placement search +
//! frame/timer/weapon reject loop + spawn flash) and the FALL are a pure function of the
//! seed, and the worm's walk carries it into the 11×11 pickup box (`ipos.x±5`, `ipos.y±5`,
//! all strict) at a deterministic tick. A bit-exact match over ALL ticks proves the pickup
//! block's per-branch RNG accounting (the health `rand(BonusHealthVar)` / the weapon
//! `rand(BonusExplodeRisk)`), the `Free`-during-iteration slot walk, and the heal/reload
//! side effects, end-to-end vs the C++ oracle. The ONLY sobject is the bonus spawn FLASH
//! (`sobject_types[7]`, detect_range=0 ⇒ inert); `nobjects`/`wobjects` stay EMPTY and the
//! `level` is CONSTANT the whole window — no booby explosion, no blood, no carve.
//!
//! ## Setup — ALL the bonus + pickup consts post-`new`
//!
//! `SimState::new` defaults every bonus/pickup const to 0/false; this harness assigns them
//! from the loaded TC AFTER `new` (as slice 5c did for the drop consts, plus the T8 pickup
//! consts `bonus_health_var`/`bonus_min_health`/`bonus_explode_risk`/`h_bonus_reload_only`).
//! Left default the run diverges at the drop tick (a forgotten const, not a sim bug). The
//! scenario is the single source of truth (parsed via `oracle_tests::scenario`); expected
//! hashes are PARSED from the golden, never hard-coded; the coverage guards are read from
//! the genuinely DRIVEN `SimState`.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

/// The empty-pool component hash (FNV-1a of a zero-length pool).
const EMPTY_POOL: u32 = 0x0000_0001;

fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
}

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
            let master = hex(next());
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

/// Per-tick witnesses recorded from the genuinely DRIVEN `SimState` (never re-parsed
/// from the golden). Index 0 = tick 0, index k = after the pass advancing k-1 → k.
struct Driven {
    scenario: Scenario,
    golden: Vec<GoldenTick>,
    worm0_health: Vec<i32>,
    worm0_weapon: Vec<(Option<WeaponId>, i32)>, // (ty, ammo) of the current weapon
    worm1_hash: Vec<u32>,
    bonus_count: Vec<usize>,
    bonus_frame: Vec<Option<i32>>,
    rng_draws: Vec<u64>,
    saw_flash: bool,
    level_constant: bool,
    nob_always_empty: bool,
    wob_always_empty: bool,
}

/// Build tick-0 state (bonus + pickup consts wired), then drive the scenario tick-for-tick,
/// asserting master + 9 components bit-exact EVERY tick (THE MILESTONE), recording the
/// witnesses the per-variant guards read.
fn drive(scenario_path: &str, golden_path: &str) -> Driven {
    let scenario_text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    let golden_text = std::fs::read_to_string(golden_path).expect("read golden");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");
    assert_eq!(scenario.max_bonuses, 4, "max_bonuses opens the bonus-drop roll");

    // --- Load the SAME level + TC the C++ dumper loaded. ------------------------------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // id == index invariants (the bonus weapon draw + flash sobject lookup index by id).
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(w.id, i as i32, "weapon id must equal its index (weapon[{i}], id {})", w.id);
    }
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }

    // Default loadout (no `weapon` directive): every slot selects weap_order[0].
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let resolved = WormInit::resolve_weapons(&objects, &weap_order, &[1u32; NUM_WEAPONS]);

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

    let mut state = SimState::new(
        &level,
        &worms_init,
        scenario.seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(&tc),
        ControlConsts::from_tc(&tc),
        tc.hacks.SignedRecoil,
        load_large_sprites(),
        tc.textures.clone(),
        objects.sobject_types.clone(),
        objects.nobject_types.clone(),
        0,
        true,
        100,
    );

    // --- The 5c bonus-drop consts (post-`new`). -----------------------------------
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
    state.bonus_rand_timer = [
        [tc.bonuses[0].timer, tc.bonuses[0].timer_v],
        [tc.bonuses[1].timer, tc.bonuses[1].timer_v],
    ];
    state.bonus_s_objects = [tc.bonuses[0].sobj, tc.bonuses[1].sobj];
    state.bonus_gravity = tc.constants.BonusGravity;
    state.bonus_bounce_mul = tc.constants.BonusBounceMul;
    state.bonus_bounce_div = tc.constants.BonusBounceDiv;
    state.weap_table = vec![0i32; objects.weapons.len()];
    // --- THE T9 STEP: the pickup consts (T8). Left default the pickup diverges. -----
    state.bonus_health_var = tc.constants.BonusHealthVar;
    state.bonus_min_health = tc.constants.BonusMinHealth;
    state.bonus_explode_risk = tc.constants.BonusExplodeRisk;
    state.h_bonus_reload_only = tc.hacks.BonusReloadOnly;
    // Blood/small-sprite consts: set from the TC for faithfulness (unread — no blood/carve
    // in either walk-on; a divergence here would flag a stray blood event).
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_small_sprites();

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(got, want, "tick {tick}: {name}: got {got:08x} expected {want:08x}");
    };
    // Components FIRST (localise a divergence to a subsystem), THEN the master.
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

    let mut d = Driven {
        scenario,
        golden,
        worm0_health: Vec::new(),
        worm0_weapon: Vec::new(),
        worm1_hash: Vec::new(),
        bonus_count: Vec::new(),
        bonus_frame: Vec::new(),
        rng_draws: Vec::new(),
        saw_flash: false,
        level_constant: true,
        nob_always_empty: true,
        wob_always_empty: true,
    };
    let level0 = hash_components(&state).level;
    let mut record = |state: &SimState| {
        let cw = state.worms[0].current_weapon as usize;
        d.worm0_health.push(state.worms[0].health);
        d.worm0_weapon.push((state.worms[0].weapons[cw].ty, state.worms[0].weapons[cw].ammo));
        d.worm1_hash.push(hash_components(state).worms[1]);
        d.bonus_count.push(state.bonuses.len());
        d.bonus_frame.push(state.bonuses.iter().next().map(|b| b.frame));
        d.rng_draws.push(state.rand.draws());
        if state.sobjects.iter().any(|s| s.id == 7) {
            d.saw_flash = true;
        }
        if hash_components(state).level != level0 {
            d.level_constant = false;
        }
        if !state.nobjects.is_empty() {
            d.nob_always_empty = false;
        }
        if !state.wobjects.is_empty() {
            d.wob_always_empty = false;
        }
    };

    assert_eq!(d.golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &d.golden[0]);
    record(&state);
    for k in 1..=d.scenario.ticks {
        let inputs = [
            ControlState::unpack(d.scenario.input(k - 1, 0)),
            ControlState::unpack(d.scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &d.golden[k as usize]);
        record(&state);
    }
    d
}

/// The pickup tick: the first (and only) tick the `bonuses` pool shrinks from 1 → 0.
fn pickup_tick(d: &Driven) -> usize {
    let t = (1..d.bonus_count.len())
        .find(|&k| d.bonus_count[k - 1] == 1 && d.bonus_count[k] == 0)
        .expect("the bonus must be PICKED UP (pool shrinks 1 → 0)");
    // …and it never re-drops in-window (exactly one live bonus, one pickup).
    for k in (t + 1)..d.bonus_count.len() {
        assert_eq!(d.bonus_count[k], 0, "tick {k}: no 2nd bonus in the clean window");
    }
    t
}

/// Guards shared by BOTH variants: pool goes live then shrinks, the spawn flash appears,
/// worm1 (the bystander) is FLAT, level constant, no explosion/blood pools.
fn assert_common(d: &Driven, pt: usize) {
    assert_eq!(d.bonus_count[0], 0, "bonuses empty at tick 0");
    let drop = (1..d.bonus_count.len())
        .find(|&k| d.bonus_count[k] == 1)
        .expect("a bonus must drop");
    assert!(drop < pt, "the bonus drops ({drop}) before the walk-on pickup ({pt})");
    // The `bonuses` component leaves the empty-pool hash while live (drop..pt), and is back
    // to empty on the pickup tick — read straight from the golden `bon` column.
    assert_ne!(d.golden[drop].pools[1], EMPTY_POOL, "bon column non-empty at the drop");
    assert_eq!(d.golden[pt].pools[1], EMPTY_POOL, "bon column empty again on the pickup tick");

    assert!(d.saw_flash, "the bonus spawn flash (sobject id 7) must appear");
    assert!(d.level_constant, "level must stay CONSTANT (no carve — no booby/explosion)");
    assert!(d.nob_always_empty, "nobjects must stay empty (no blood fan)");
    assert!(d.wob_always_empty, "wobjects must stay empty (no weapon fired)");

    // The OTHER worm (worm1) is a bystander: its component hash is FLAT all ticks.
    let w1: std::collections::HashSet<u32> = d.worm1_hash.iter().copied().collect();
    assert_eq!(w1.len(), 1, "worm1 (bystander) column must be FLAT; saw {:?}", w1);

    // The pickup tick draws MORE than a bare drop-roll: the pickup's own roll fires. Every
    // non-drop / non-pickup tick draws exactly 1 (the per-tick bonus-drop roll).
    let pickup_draws = d.rng_draws[pt] - d.rng_draws[pt - 1];
    assert!(
        pickup_draws >= 2,
        "the pickup tick must draw the drop-roll + the pickup roll (>=2); saw {pickup_draws}"
    );
}

#[test]
fn sim_slice5prime_pickup_health_walkon_match_cpp_oracle() {
    let d = drive(
        concat!(env!("CARGO_MANIFEST_DIR"), "/golden/sim_slice5prime_pickup_health_scenario.txt"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/golden/sim_slice5prime_pickup_health.txt"),
    );
    assert_eq!(d.scenario.seed, 29, "health variant seed");
    assert_eq!(d.scenario.worms[0].health, 50, "worm0 spawns WOUNDED (health 50)");

    let pt = pickup_tick(&d);
    assert_common(&d, pt);

    // The dropped bonus is a FRAME-1 (health) bonus.
    let live_frame = (0..d.bonus_count.len())
        .find_map(|k| d.bonus_frame[k])
        .expect("a live bonus exists");
    assert_eq!(live_frame, 1, "the seed-29 drop is a FRAME-1 (health) bonus");

    // worm0 health JUMPS UP on the pickup tick, from the wounded 50.
    assert_eq!(d.worm0_health[pt - 1], 50, "worm0 stays wounded (50) until the pickup");
    assert!(
        d.worm0_health[pt] > d.worm0_health[pt - 1],
        "worm0 health must JUMP UP on the pickup tick (heal); {} -> {}",
        d.worm0_health[pt - 1],
        d.worm0_health[pt]
    );
    assert!(d.worm0_health[pt] <= 100, "heal clamps to settings->health (100)");
    // Health stays flat AFTER the heal (no further pickup / damage).
    for k in (pt + 1)..d.worm0_health.len() {
        assert_eq!(d.worm0_health[k], d.worm0_health[pt], "tick {k}: worm0 health flat after heal");
    }
    // The health branch draws EXACTLY the drop-roll + one `rand(BonusHealthVar)` = 2.
    assert_eq!(
        d.rng_draws[pt] - d.rng_draws[pt - 1],
        2,
        "health pickup draws the drop-roll + one heal roll (exactly 2)"
    );
    // worm0's current weapon is UNTOUCHED by a health pickup.
    assert_eq!(
        d.worm0_weapon[pt], d.worm0_weapon[pt - 1],
        "a health pickup must NOT change worm0's weapon"
    );
}

#[test]
fn sim_slice5prime_pickup_weapon_reload_walkon_match_cpp_oracle() {
    let d = drive(
        concat!(env!("CARGO_MANIFEST_DIR"), "/golden/sim_slice5prime_pickup_weapon_scenario.txt"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/golden/sim_slice5prime_pickup_weapon.txt"),
    );
    assert_eq!(d.scenario.seed, 27, "weapon variant seed");
    assert_eq!(d.scenario.worms[0].health, 100, "worm0 spawns at full health");

    let pt = pickup_tick(&d);
    assert_common(&d, pt);

    // The dropped bonus is a FRAME-0 (weapon) bonus.
    let live_frame = (0..d.bonus_count.len())
        .find_map(|k| d.bonus_frame[k])
        .expect("a live bonus exists");
    assert_eq!(live_frame, 0, "the seed-27 drop is a FRAME-0 (weapon) bonus");

    // worm0's CURRENT WEAPON CHANGES on the pickup tick (the reload flips ww.type/ammo).
    assert_ne!(
        d.worm0_weapon[pt], d.worm0_weapon[pt - 1],
        "the weapon RELOAD must change worm0's current weapon (ty/ammo); {:?} -> {:?}",
        d.worm0_weapon[pt - 1], d.worm0_weapon[pt]
    );
    // The weapon change persists after the pickup (a genuine reload, not a one-tick blip).
    for k in (pt + 1)..d.worm0_weapon.len() {
        assert_eq!(d.worm0_weapon[k], d.worm0_weapon[pt], "tick {k}: reloaded weapon persists");
    }
    // worm0 health is UNCHANGED — the RELOAD branch never damages (the discriminator vs the
    // booby branch, which drops health + bursts an explosion; booby stays unit-test-only).
    for (k, &h) in d.worm0_health.iter().enumerate() {
        assert_eq!(h, 100, "tick {k}: worm0 health must stay 100 (reload, not booby)");
    }
    // The weapon branch draws EXACTLY the drop-roll + one `rand(BonusExplodeRisk)` = 2 (the
    // reload path draws nothing further; a booby would BURST the explosion's RNG cluster).
    assert_eq!(
        d.rng_draws[pt] - d.rng_draws[pt - 1],
        2,
        "weapon reload draws the drop-roll + one explode-risk roll (exactly 2, NOT a booby burst)"
    );
}
