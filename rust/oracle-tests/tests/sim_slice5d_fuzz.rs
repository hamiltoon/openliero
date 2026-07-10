//! Slice-5d **T9** — fixed-level multi-seed respawn FUZZ (O21). Four variants on the
//! SAME `physics_fall_test.lev`, each tuned so `BeginRespawn`'s level-reading RNG spawn
//! search takes a DIFFERENT bounded trial count. This is the *variance* proof for the
//! Step-2 desync trap: the milestone (`sim_slice5d_golden.rs`) pins ONE death->respawn
//! bit-exact; T9 proves the trap's trial-count spread is covered bit-exact vs the C++
//! oracle too — not one search replayed four times.
//!
//! ## Why the four variants differ (game.cpp:611-650 `CheckRespawnPosition`)
//!
//! `BeginRespawn` draws `rand(WormSpawnRectW)`,`rand(WormSpawnRectH)` per trial and loops
//! until `CheckRespawnPosition` accepts. Its last-pos reject clause uses a RAW
//! `kDeltaX = old_x` — an engine BUG the port mirrors (`state.rs` `check_respawn_position`)
//! — so it fires only while `abs(death_x) <= WormMinSpawnDistLast (160)`. The 5d MILESTONE
//! kills worm1 at x=115 (<=160), which rejects every floor candidate and CAPS the search
//! at 50000 trials — the SAME degenerate search whatever the seed. The T9 variants move
//! both worms RIGHT so worm1 dies at x>160, DISABLING the last-pos reject; the search is
//! then governed only by the enemy-distance band (`abs(enemy_x - cand_x) <= 160`) plus the
//! live rand stream, giving a SMALL trial count that varies with the killer's x (the enemy
//! band) and with worm1's health (the pre-death drip shifts the rand stream reaching
//! `BeginRespawn`).
//!
//! Measured trial counts (rng burst at the respawn tick / 2), all against the C++ golden:
//!   * fuzz1  worm0 x=235 worm1 x=200 h=12  -> **3** trials
//!   * fuzz2  worm0 x=275 worm1 x=240 h=12  -> **7** trials  (killer x lever)
//!   * fuzz3  worm0 x=335 worm1 x=300 h=12  -> **2** trials  (killer x lever)
//!   * fuzz4  worm0 x=335 worm1 x=300 h= 8  -> **6** trials  (SAME pos as fuzz3, drip lever)
//! => four DISTINCT counts {2,3,6,7} (>=2 required). fuzz3 vs fuzz4 isolate the pure
//! rand-stream (health/drip) lever from the position lever: identical geometry, different
//! count.
//!
//! Each variant asserts master + all 9 component hashes bit-exact for EVERY tick (0..=360),
//! exactly as the milestone. A pure-Rust determinism backstop (two independent `SimState`
//! runs per variant, master hash identical every tick) proves no nondeterminism entered
//! the port. The goldens are LOCAL/MANUAL C++-dumper output (`gen_sim_slice5d_fuzz{1..4}_
//! golden.sh`), the dumper UNCHANGED since 5c.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
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

/// One parsed golden line — all 11 columns (master + 9 components), same as the milestone.
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

/// A T9 fuzz variant: the scenario/golden basenames, the health worm1 starts at, and the
/// trial count the C++ oracle produced (an intent guard — a regenerated golden that lost
/// the tuned search fails HERE, not silently).
struct Variant {
    name: &'static str,
    scenario: &'static str,
    golden: &'static str,
    expected_health: i32,
    expected_trials: u64,
}

const VARIANTS: [Variant; 4] = [
    Variant { name: "fuzz1", scenario: "sim_slice5d_fuzz1_scenario.txt", golden: "sim_slice5d_fuzz1.txt", expected_health: 12, expected_trials: 3 },
    Variant { name: "fuzz2", scenario: "sim_slice5d_fuzz2_scenario.txt", golden: "sim_slice5d_fuzz2.txt", expected_health: 12, expected_trials: 7 },
    Variant { name: "fuzz3", scenario: "sim_slice5d_fuzz3_scenario.txt", golden: "sim_slice5d_fuzz3.txt", expected_health: 12, expected_trials: 2 },
    Variant { name: "fuzz4", scenario: "sim_slice5d_fuzz4_scenario.txt", golden: "sim_slice5d_fuzz4.txt", expected_health: 8, expected_trials: 6 },
];

/// Everything the coverage guards read from the genuinely DRIVEN state (never re-parsed
/// from the golden).
struct RunDiag {
    trials: u64,
    death_tick: usize,
    begin_respawn_tick: usize,
    reborn_tick: usize,
    settings_health: i32,
    worm1_start_lives: i32,
    worm1_final_lives: i32,
    worm1_final_health: i32,
    worm0_max_kills: i32,
    max_nobjects: usize,
    saw_type6_blood: bool,
    death_pos: Vec2,
    spawn_pos: Vec2,
    /// master `hash_game_state` per tick 0..=ticks (for the determinism backstop).
    master_hashes: Vec<u32>,
}

/// Build the tick-0 `SimState` for a variant's scenario — IDENTICAL setup to the
/// milestone harness (real level/tc/objects, EXPLOSIVES slot 0, all death/respawn consts
/// assigned post-`new`).
fn build_state(scenario: &Scenario) -> SimState {
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // id == index invariants (indexed lookups on Fire/explosion/spray/respawn).
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(w.id, i as i32, "weapon id must equal its index");
    }
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index");
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index");
    }

    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);
    let weapon_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` present");
    let weapon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == weapon_name)
        .unwrap_or_else(|| panic!("weapon {weapon_name:?} in TC table"));
    resolved[0] = WeaponInit { ty: Some(weapon_idx as WeaponId), ammo: objects.weapons[weapon_idx].ammo };

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
    // The death/respawn consts (as the milestone) — left at `new`'s 0 the run diverges at
    // the first blood tick OR at BeginRespawn (a forgotten const, not a sim bug).
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_small_sprites();
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state
}

/// Drive a variant end to end. If `golden` is Some, assert master + 9 components bit-exact
/// on EVERY tick (components first, master last — localises a divergence). Returns the
/// driven-state diagnostics for the coverage guards.
fn drive(scenario: &Scenario, golden: Option<&[GoldenTick]>) -> RunDiag {
    let mut state = build_state(scenario);
    let settings_health = state.settings_health;

    let assert_tick = |state: &SimState, g: &GoldenTick| {
        let check = |name: &str, got: u32, want: u32| {
            assert_eq!(got, want, "tick {}: {name}: got {got:08x} expected {want:08x}", g.tick);
        };
        let c = hash_components(state);
        check("rng", c.rng, g.rng);
        check("level", c.level, g.level);
        check("worm0", c.worms[0], g.worm0);
        check("worm1", c.worms[1], g.worm1);
        check("bobjects", c.bobjects, g.pools[0]);
        check("bonuses", c.bonuses, g.pools[1]);
        check("sobjects", c.sobjects, g.pools[2]);
        check("nobjects", c.nobjects, g.pools[3]);
        check("wobjects", c.wobjects, g.pools[4]);
        check("MASTER state_hash", hash_game_state(state), g.master);
    };

    // Per-tick driven-state witnesses.
    let mut worm1_health: Vec<i32> = Vec::new();
    let mut worm1_visible: Vec<bool> = Vec::new();
    let mut worm1_lives: Vec<i32> = Vec::new();
    let mut worm0_kills: Vec<i32> = Vec::new();
    let mut worm1_killed_timer: Vec<i32> = Vec::new();
    let mut worm1_pos: Vec<Vec2> = Vec::new();
    let mut rng_draws: Vec<u64> = Vec::new();
    let mut master_hashes: Vec<u32> = Vec::new();
    let mut max_nobjects = 0usize;
    let mut saw_type6_blood = false;
    let mut bonuses_always_empty = true;

    let mut record = |state: &SimState| {
        worm1_health.push(state.worms[1].health);
        worm1_visible.push(state.worms[1].visible);
        worm1_lives.push(state.worms[1].lives);
        worm0_kills.push(state.worms[0].kills);
        worm1_killed_timer.push(state.worms[1].killed_timer);
        worm1_pos.push(state.worms[1].pos);
        rng_draws.push(state.rand.draws());
        master_hashes.push(hash_game_state(state));
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if state.nobjects.iter().any(|n| n.ty == Some(6)) {
            saw_type6_blood = true;
        }
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
        }
    };

    // Tick 0 (no process_frame).
    if let Some(g) = golden {
        assert_eq!(g[0].tick, 0, "first golden row is tick 0");
        assert_tick(&state, &g[0]);
    }
    record(&state);

    // The off-by-one: golden line k (k>=1) is input[k-1] applied advancing k-1 -> k.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        if let Some(g) = golden {
            assert_tick(&state, &g[k as usize]);
        }
        record(&state);
    }

    // --- Derive the death/respawn structure from the DRIVEN state. ------------------
    let worm1_start_lives = scenario.worms[1].lives;
    let death_tick = (1..worm1_health.len())
        .find(|&k| worm1_health[k] <= 0)
        .expect("worm1 must DIE (health crosses <= 0)");
    let begin_respawn_tick = (death_tick + 1..worm1_killed_timer.len())
        .find(|&k| worm1_killed_timer[k] < 0)
        .expect("BeginRespawn must run (killed_timer -> -1)");
    let first_dead = worm1_visible.iter().position(|&v| !v).expect("worm1 dies (invisible)");
    let reborn_tick = (first_dead..worm1_visible.len())
        .find(|&k| worm1_visible[k])
        .expect("worm1 reborn (DoRespawning completes)");
    // trial count = rng burst on the respawn tick / 2 (each trial draws rand(W),rand(H)).
    let trials = (rng_draws[begin_respawn_tick] - rng_draws[begin_respawn_tick - 1]) / 2;

    assert!(bonuses_always_empty, "bonuses must stay empty (max_bonuses 0)");

    RunDiag {
        trials,
        death_tick,
        begin_respawn_tick,
        reborn_tick,
        settings_health,
        worm1_start_lives,
        worm1_final_lives: *worm1_lives.last().unwrap(),
        worm1_final_health: *worm1_health.last().unwrap(),
        worm0_max_kills: worm0_kills.iter().copied().max().unwrap(),
        max_nobjects,
        saw_type6_blood,
        death_pos: worm1_pos[death_tick],
        spawn_pos: worm1_pos[begin_respawn_tick],
        master_hashes,
    }
}

/// Full per-tick bit-exact assert + single-variant death->respawn coverage. Shared by the
/// four `#[test]` entry points.
fn run_variant(v: &Variant) -> RunDiag {
    let scenario_text = std::fs::read_to_string(format!(
        "{}/golden/{}",
        env!("CARGO_MANIFEST_DIR"),
        v.scenario
    ))
    .expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "{}: seed 42", v.name);
    assert_eq!(scenario.ticks, 360, "{}: 360 ticks", v.name);
    assert_eq!(scenario.max_bonuses, 0, "{}: no bonuses", v.name);
    assert_eq!(scenario.worms.len(), 2, "{}: two worms", v.name);
    assert_eq!(scenario.worms[0].health, 100, "{}: killer full health", v.name);
    assert_eq!(scenario.worms[1].health, v.expected_health, "{}: victim health", v.name);
    // The whole T9 point: worm1 must die at x > WormMinSpawnDistLast(160) so the last-pos
    // reject is disabled and the trial count is a genuine (non-capped) search.
    assert!(
        (scenario.worms[1].pos_x >> 16) > 160,
        "{}: worm1 must start (and die) at x>160 to disable the last-pos reject",
        v.name
    );

    let golden_text = std::fs::read_to_string(format!(
        "{}/golden/{}",
        env!("CARGO_MANIFEST_DIR"),
        v.golden
    ))
    .expect("read golden");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "{}: golden 0..=360", v.name);

    // Golden-shape headline (fails loudly before the per-tick loop if a regen lost the
    // death OR the respawn): worm1 shows >=3 phases, terrain carves, the pools go live.
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(worm1_cols.len() >= 3, "{}: worm1 >=3 phases (alive/dead/reborn)", v.name);
    let levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(levels.len() >= 2, "{}: terrain carves (>=2 level hashes)", v.name);
    assert!(golden.iter().any(|g| g.pools[2] != EMPTY_POOL), "{}: sobject (large_explosion)", v.name);
    assert!(golden.iter().any(|g| g.pools[3] != EMPTY_POOL), "{}: nobjects (blood/gib/dirt)", v.name);
    assert!(golden.iter().any(|g| g.pools[0] != EMPTY_POOL), "{}: bobjects (blood drip)", v.name);
    assert!(golden.iter().any(|g| g.pools[4] != EMPTY_POOL), "{}: wobjects (explosives)", v.name);
    for g in &golden {
        assert_eq!(g.pools[1], EMPTY_POOL, "{}: tick {} bonuses empty", v.name, g.tick);
    }

    // Drive + assert every tick bit-exact.
    let d = drive(&scenario, Some(&golden));

    // ---- Single-variant death -> respawn coverage (from the driven state) ----------
    assert!(!worm1_reborn_before_death(&d), "internal ordering sanity");
    assert_eq!(d.worm1_final_health, d.settings_health, "{}: health restored to settings_health", v.name);
    assert_eq!(d.worm1_final_lives, d.worm1_start_lives - 1, "{}: exactly one life lost", v.name);
    assert_eq!(d.worm0_max_kills, 1, "{}: killer scores exactly one kill", v.name);
    assert!(d.saw_type6_blood, "{}: death spray includes type-6 blood nobjects", v.name);
    assert!(d.max_nobjects < 600, "{}: nobjects under the 600 cap (O3); peaked at {}", v.name, d.max_nobjects);

    // The 150-tick killed_timer countdown gap (hash-silent; only footprint is WHEN
    // BeginRespawn lands).
    let countdown = d.begin_respawn_tick - d.death_tick;
    assert!((140..=160).contains(&countdown), "{}: ~150-tick countdown; saw {countdown}", v.name);

    // worm1 pos JUMPS at BeginRespawn (>100px) — the trial search moved it.
    let dx = ((d.spawn_pos.x - d.death_pos.x) >> 16).abs();
    let dy = ((d.spawn_pos.y - d.death_pos.y) >> 16).abs();
    assert!(dx.max(dy) > 100, "{}: pos JUMPS at BeginRespawn; dx={dx} dy={dy}", v.name);

    // DoRespawning completes AFTER BeginRespawn.
    assert!(d.reborn_tick > d.begin_respawn_tick, "{}: reborn after BeginRespawn", v.name);
    assert!(d.reborn_tick <= scenario.ticks as usize, "{}: reborn inside the window", v.name);

    // The trap is a GENUINE (non-capped) search AND matches the C++ oracle's count.
    assert!(d.trials >= 1 && d.trials < 50000, "{}: bounded non-capped search; trials={}", v.name, d.trials);
    assert_eq!(d.trials, v.expected_trials, "{}: trial count matches the C++ oracle", v.name);

    d
}

fn worm1_reborn_before_death(d: &RunDiag) -> bool {
    d.reborn_tick <= d.death_tick
}

#[test]
fn fuzz1_death_respawn_match_cpp_oracle() {
    run_variant(&VARIANTS[0]);
}
#[test]
fn fuzz2_death_respawn_match_cpp_oracle() {
    run_variant(&VARIANTS[1]);
}
#[test]
fn fuzz3_death_respawn_match_cpp_oracle() {
    run_variant(&VARIANTS[2]);
}
#[test]
fn fuzz4_death_respawn_match_cpp_oracle() {
    run_variant(&VARIANTS[3]);
}

/// The COLLECTIVE T9 coverage: the four variants exhibit >= 2 DISTINCT BeginRespawn trial
/// counts — the rng-burst WIDTH at the respawn tick differs across variants, proving the
/// desync trap's variance is covered vs the C++ oracle (not four copies of one search).
#[test]
fn variants_exhibit_at_least_two_distinct_trial_counts() {
    let counts: Vec<u64> = VARIANTS.iter().map(run_variant).map(|d| d.trials).collect();
    let distinct: std::collections::HashSet<u64> = counts.iter().copied().collect();
    assert!(
        distinct.len() >= 2,
        "T9 requires >=2 distinct trial counts; saw {counts:?} ({} distinct)",
        distinct.len()
    );
    // None may hit the 50000 cap (that would be the degenerate SAME search, not variance).
    assert!(counts.iter().all(|&t| t < 50000), "no variant may cap the search; {counts:?}");
}

/// Pure-Rust determinism backstop: two INDEPENDENT `SimState` runs of each variant must
/// produce an identical master `hash_game_state` on every tick. Proves no nondeterminism
/// (iteration order, uninit reads, time) entered the port — orthogonal to the C++ oracle.
#[test]
fn each_variant_is_internally_deterministic() {
    for v in &VARIANTS {
        let text = std::fs::read_to_string(format!(
            "{}/golden/{}",
            env!("CARGO_MANIFEST_DIR"),
            v.scenario
        ))
        .expect("read scenario");
        let scenario = Scenario::parse(&text).expect("scenario parses");
        let a = drive(&scenario, None);
        let b = drive(&scenario, None);
        assert_eq!(
            a.master_hashes, b.master_hashes,
            "{}: two runs must be hash-identical every tick (determinism)",
            v.name
        );
        assert_eq!(a.trials, b.trials, "{}: trial count is deterministic", v.name);
    }
}
