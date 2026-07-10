//! Slice-6 **T8** — the >1000-tick full-`ProcessFrame` FUZZ MILESTONE. Five variants
//! (`sim_slice6_fuzz{1..5}`), each a 1500-tick game of two worms seeded DEAD that
//! respawn in-sim and are then driven by a per-tick `input_rng() & 0x7f` random input
//! stream (the mirror of `src/tests/test_determinism.cpp`, pre-expanded into the
//! committed scenario). Each variant asserts master + all 9 component hashes bit-exact
//! for EVERY tick 0..=1500 vs the C++ oracle golden — the widest, longest differential
//! coverage in Step 2. This is the closing milestone of step 2.
//!
//! ## What the milestone proves (vs the C++ `Game`)
//!
//! Over 5 × 1501 ticks the full `ProcessFrame` — worm physics, firing, ninjarope
//! (terrain + worm anchor, the `rand(128)` dirt spray), all object pools
//! (bobjects/nobjects/sobjects/wobjects/bonuses), death → blood/gib spray →
//! killed-timer countdown → `BeginRespawn` spawn search → `DoRespawning` descent,
//! bonus drop-roll + pickup, and the game-mode tail — stays bit-for-bit identical to
//! the C++ engine. The scenario file is the single source of truth read by BOTH the
//! Rust difftest here and the C++ `oracle_dump_sim_physics` dumper (no dumper change;
//! the T7 controller decision), so a match is a true cross-implementation equality.
//!
//! ## Deferral #7 (T4/slice-5) re-confirmed
//!
//! The fuzz's explosions happen with BOTH worms alive and near each other (multi-worm
//! `destroy` + `explode`). Rust defers `do_remove` to the end of the object loop where
//! C++ frees mid-loop; slice-5 deferred proving these orderings agree under multi-worm
//! blasts (deferral #7). Here every variant's explosions pass bit-exact across all 1500
//! ticks — the empirical closure of deferral #7 (see the T8 report).
//!
//! ## Coverage guards (non-vacuous, ALL derived FRESH from the DRIVEN state)
//!
//! The T7-era scenario-header event ticks were read off the PRE-FIX (buggy) Rust stream
//! and are wrong post-fix; NONE are re-parsed here. Every witness below is recomputed
//! from the genuinely driven `SimState`:
//!   * a worm's `lives` DECREASES (a death cost a life);
//!   * the `rng` component moves on a respawn-search (`BeginRespawn`) tick;
//!   * the `bonuses` pool GROWS then SHRINKS (a drop landed, then a bonus left);
//!   * a worm's `health` DROPS on an in-flight projectile-hit tick;
//!   * the `nobjects` column BUMPS on a ninjarope terrain-attach tick (the `rand(128)`
//!     11-object dirt spray);
//!   * every pool stays `<= capacity` every tick (blood approaches its 700 cap — the
//!     `spawn_reuse`/`NewObjectReuse` path).
//! A pure-Rust determinism backstop (two independent `SimState` runs per variant, master
//! hash identical every tick) proves no nondeterminism entered the port.
//!
//! The goldens (`golden/sim_slice6_fuzz{1..5}.txt`) are the C++-dumper oracle — never
//! edited here. The scenarios were authored by `examples/gen_slice6_fuzz.rs`.

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

/// The empty-pool component hash (fold of a zero-length pool == seed 1).
const EMPTY_POOL: u32 = 0x0000_0001;
/// Blood pool capacity (`BLOOD_CAPACITY`; C++ `game.cpp:513`, default 700).
const BLOOD_CAP: usize = 700;
/// Nobject pool capacity (`NOBJECT_CAPACITY`).
const NOBJ_CAP: usize = 600;

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

/// One parsed golden line — all 11 columns (master + 9 components).
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

/// A T8 fuzz variant: scenario/golden basenames and the game seed (an intent guard —
/// a regenerated scenario that lost the tuned seed fails HERE, not silently).
struct Variant {
    name: &'static str,
    scenario: &'static str,
    golden: &'static str,
    seed: u32,
}

const VARIANTS: [Variant; 5] = [
    Variant { name: "fuzz1", scenario: "sim_slice6_fuzz1_scenario.txt", golden: "sim_slice6_fuzz1.txt", seed: 385 },
    Variant { name: "fuzz2", scenario: "sim_slice6_fuzz2_scenario.txt", golden: "sim_slice6_fuzz2.txt", seed: 290 },
    Variant { name: "fuzz3", scenario: "sim_slice6_fuzz3_scenario.txt", golden: "sim_slice6_fuzz3.txt", seed: 175 },
    Variant { name: "fuzz4", scenario: "sim_slice6_fuzz4_scenario.txt", golden: "sim_slice6_fuzz4.txt", seed: 222 },
    Variant { name: "fuzz5", scenario: "sim_slice6_fuzz5_scenario.txt", golden: "sim_slice6_fuzz5.txt", seed: 43 },
];

/// Build the tick-0 `SimState` for a variant's scenario — IDENTICAL setup to the
/// `examples/gen_slice6_fuzz.rs` harness (real level/tc/objects, worms from the
/// scenario `worm` lines seeded DEAD with the default full loadout, ALL death/respawn
/// + blood + bonus-drop + bonus-pickup consts assigned post-`new`).
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

    // Default full loadout (every slot == weap_order[0]); the fuzz scenarios carry no
    // `weapon` directive, but honour one if present (keeps the builder faithful).
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &[1u32; NUM_WEAPONS]);
    for slot in 0..NUM_WEAPONS {
        if let Some(name) = scenario.weapon(slot) {
            let idx = objects
                .weapons
                .iter()
                .position(|w| w.name == name)
                .unwrap_or_else(|| panic!("weapon {name:?} in TC table"));
            let ammo = scenario.weapon_ammo(slot).unwrap_or(objects.weapons[idx].ammo);
            resolved[slot] = WeaponInit { ty: Some(idx as WeaponId), ammo };
        }
    }

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
        0,    // settings_loading_time (dumper: 0)
        true, // load_change
        100,  // blood
    );
    // Blood / small-sprite consts (blood storms, worm-hit fans).
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_small_sprites();
    // Worm-spawn consts (BeginRespawn / DoRespawning spawn search).
    state.worm_spawn_rect_x = tc.constants.WormSpawnRectX;
    state.worm_spawn_rect_y = tc.constants.WormSpawnRectY;
    state.worm_spawn_rect_w = tc.constants.WormSpawnRectW;
    state.worm_spawn_rect_h = tc.constants.WormSpawnRectH;
    state.worm_min_spawn_dist_last = tc.constants.WormMinSpawnDistLast;
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    // Bonus-drop + bonus-object consts.
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
    // Bonus-pickup consts.
    state.bonus_health_var = tc.constants.BonusHealthVar;
    state.bonus_min_health = tc.constants.BonusMinHealth;
    state.bonus_explode_risk = tc.constants.BonusExplodeRisk;
    state.h_bonus_reload_only = tc.hacks.BonusReloadOnly;
    state.game_mode = scenario.game_mode as u32;
    state
}

/// The strict 11x11 pickup AABB gate (`worm.cpp:289-290`): `|ipos - b| < 5` on both
/// axes — the witness that a vanished bonus was COLLECTED (not timer-expired).
fn on_bonus(wx: i32, wy: i32, bx: i32, by: i32) -> bool {
    (wx - bx).abs() < 5 && (wy - by).abs() < 5
}

/// A per-tick snapshot of the driven state used by the coverage guards. Nothing here is
/// ever read from a golden — it is the genuinely simulated `SimState`.
struct Snap {
    visible: [bool; 2],
    health: [i32; 2],
    killed_timer: [i32; 2],
    lives: [i32; 2],
    ipos: [(i32, i32); 2],
    rope_attached: [bool; 2],
    wob: Vec<(i32, i32)>,
    bonuses_xy: Vec<(i32, i32)>,
    n_bonuses: usize,
    n_nobjects: usize,
    n_bobjects: usize,
    comp_rng: u32,
    comp_nobjects: u32,
    comp_bonuses: u32,
}

fn snap(s: &SimState) -> Snap {
    let c = hash_components(s);
    let w = |i: usize| {
        let ww = &s.worms[i];
        (ww.visible, ww.health, ww.killed_timer, ww.lives, (ftoi(ww.pos.x), ftoi(ww.pos.y)), ww.ninjarope.attached)
    };
    let (v0, h0, kt0, l0, p0, r0) = w(0);
    let (v1, h1, kt1, l1, p1, r1) = w(1);
    Snap {
        visible: [v0, v1],
        health: [h0, h1],
        killed_timer: [kt0, kt1],
        lives: [l0, l1],
        ipos: [p0, p1],
        rope_attached: [r0, r1],
        wob: s.wobjects.iter().map(|o| (ftoi(o.pos.x), ftoi(o.pos.y))).collect(),
        bonuses_xy: s.bonuses.iter().map(|b| (ftoi(b.x), ftoi(b.y))).collect(),
        n_bonuses: s.bonuses.len(),
        n_nobjects: s.nobjects.len(),
        n_bobjects: s.bobjects.len(),
        comp_rng: c.rng,
        comp_nobjects: c.nobjects,
        comp_bonuses: c.bonuses,
    }
}

/// Everything the coverage guards read from the genuinely DRIVEN state.
#[derive(Default)]
struct RunDiag {
    master_hashes: Vec<u32>,
    // lives-decrease (death) witness.
    start_lives: [i32; 2],
    final_lives: [i32; 2],
    lives_decreased: bool,
    // respawn-search rng-move witness: (tick, worm) of a BeginRespawn tick where the rng
    // component hash moved vs the previous tick.
    rng_move_on_respawn: Option<(u32, usize)>,
    n_respawns: u32,
    // bonus pool grow-then-shrink witness.
    bonus_grew: bool,
    bonus_shrank: bool,
    peak_bonuses: usize,
    bonus_col_moved: bool,
    // in-flight-hit health-drop witness.
    inflight_hit: Option<(u32, usize)>,
    n_inflight_hits: u32,
    // ninjarope terrain-attach nobjects-bump witness.
    rope_spray: Option<(u32, usize, usize)>, // (tick, worm, nobjects_delta)
    n_rope_attaches: u32,
    // pool caps.
    peak_bobjects: usize,
    peak_bobjects_tick: u32,
    peak_nobjects: usize,
    cap_never_exceeded: bool,
    // strict pickup detector (health/weapon).
    n_pickups: u32,
}

/// Drive a variant end to end. If `golden` is Some, assert master + 9 components bit-exact
/// on EVERY tick (components before master — localises a divergence). Returns the
/// driven-state diagnostics for the coverage guards.
fn drive(scenario: &Scenario, golden: Option<&[GoldenTick]>) -> RunDiag {
    let mut state = build_state(scenario);

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

    let mut d = RunDiag {
        start_lives: [state.worms[0].lives, state.worms[1].lives],
        cap_never_exceeded: true,
        ..RunDiag::default()
    };

    // Tick 0 (no process_frame).
    if let Some(g) = golden {
        assert_eq!(g[0].tick, 0, "first golden row is tick 0");
        assert_tick(&state, &g[0]);
    }
    d.master_hashes.push(hash_game_state(&state));
    let mut prev = snap(&state);
    if prev.n_bobjects > BLOOD_CAP || prev.n_nobjects > NOBJ_CAP {
        d.cap_never_exceeded = false;
    }

    // Golden line k (k>=1) is input[k-1] applied advancing k-1 -> k (the dumper seam).
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        if let Some(g) = golden {
            assert_tick(&state, &g[k as usize]);
        }
        d.master_hashes.push(hash_game_state(&state));
        let cur = snap(&state);

        for wi in 0..2 {
            // Death cost a life: lives strictly decreased for this worm.
            if cur.lives[wi] < prev.lives[wi] {
                d.lives_decreased = true;
            }
            // Respawn: visible false -> true (incl. the initial seeded-dead spawn).
            if !prev.visible[wi] && cur.visible[wi] {
                d.n_respawns += 1;
            }
            // BeginRespawn tick: killed_timer just went negative — the spawn search runs
            // here (draws rand(W),rand(H) per trial). Witness that the rng column moved.
            if prev.killed_timer[wi] >= 0 && cur.killed_timer[wi] < 0 {
                if cur.comp_rng != prev.comp_rng && d.rng_move_on_respawn.is_none() {
                    d.rng_move_on_respawn = Some((k, wi));
                }
            }
            // Ninjarope attach: attached false -> true this tick. A TERRAIN attach over
            // dirt sprays 11 type-2 nobjects (rand(128) each) — so the nobjects column
            // bumps and the pool grows. A worm-anchor attach does NOT spray.
            if !prev.rope_attached[wi] && cur.rope_attached[wi] {
                d.n_rope_attaches += 1;
                if cur.n_nobjects > prev.n_nobjects
                    && cur.comp_nobjects != prev.comp_nobjects
                    && d.rope_spray.is_none()
                {
                    d.rope_spray = Some((k, wi, cur.n_nobjects - prev.n_nobjects));
                }
            }
            // In-flight hit: health dropped AND a wobject from the previous tick was within
            // 8px of the worm (an in-flight projectile hit, not a distant blast).
            if prev.visible[wi] && cur.visible[wi] && cur.health[wi] < prev.health[wi] {
                let (wx, wy) = prev.ipos[wi];
                if prev.wob.iter().any(|&(ox, oy)| (ox - wx).abs() <= 8 && (oy - wy).abs() <= 8) {
                    d.n_inflight_hits += 1;
                    if d.inflight_hit.is_none() {
                        d.inflight_hit = Some((k, wi));
                    }
                }
            }
            // Strict pickup detector (a bonus on this worm's AABB last tick is gone now).
            if prev.visible[wi] && cur.visible[wi] && cur.n_bonuses < prev.n_bonuses {
                let (wx, wy) = prev.ipos[wi];
                let vanished_on_worm = prev
                    .bonuses_xy
                    .iter()
                    .filter(|&&(bx, by)| on_bonus(wx, wy, bx, by))
                    .any(|&pb| !cur.bonuses_xy.iter().any(|&cb| cb == pb));
                if vanished_on_worm {
                    d.n_pickups += 1;
                }
            }
        }

        // Bonus pool grow / shrink + column-moved witness.
        if cur.n_bonuses > prev.n_bonuses {
            d.bonus_grew = true;
        }
        if cur.n_bonuses < prev.n_bonuses {
            d.bonus_shrank = true;
        }
        if cur.comp_bonuses != prev.comp_bonuses {
            d.bonus_col_moved = true;
        }
        d.peak_bonuses = d.peak_bonuses.max(cur.n_bonuses);

        // Pool caps.
        if cur.n_bobjects > d.peak_bobjects {
            d.peak_bobjects = cur.n_bobjects;
            d.peak_bobjects_tick = k;
        }
        d.peak_nobjects = d.peak_nobjects.max(cur.n_nobjects);
        if cur.n_bobjects > BLOOD_CAP || cur.n_nobjects > NOBJ_CAP {
            d.cap_never_exceeded = false;
        }

        prev = cur;
    }

    d.final_lives = [state.worms[0].lives, state.worms[1].lives];
    d
}

/// Read + parse a variant's scenario and golden, assert the golden's shape, drive it with
/// the golden attached (bit-exact every tick), then assert the driven-state coverage
/// guards. Shared by the five `#[test]` entry points.
fn run_variant(v: &Variant) -> RunDiag {
    let scenario_text =
        std::fs::read_to_string(format!("{}/golden/{}", env!("CARGO_MANIFEST_DIR"), v.scenario))
            .expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, v.seed, "{}: game seed", v.name);
    assert_eq!(scenario.ticks, 1500, "{}: 1500 ticks (>1000 milestone)", v.name);
    assert_eq!(scenario.max_bonuses, 4, "{}: max_bonuses 4 (drop roll open)", v.name);
    assert_eq!(scenario.game_mode, 0, "{}: game_mode 0 (kill'em all)", v.name);
    assert_eq!(scenario.worms.len(), 2, "{}: two worms", v.name);
    for w in &scenario.worms {
        assert!(!w.visible, "{}: worms seeded DEAD (visible 0)", v.name);
    }

    let golden_text =
        std::fs::read_to_string(format!("{}/golden/{}", env!("CARGO_MANIFEST_DIR"), v.golden))
            .expect("read golden");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "{}: golden 0..=1500", v.name);
    assert_eq!(golden[0].rng, 0, "{}: tick 0 has drawn no rand yet", v.name);

    // Golden-shape headline (fails loudly before the per-tick loop if a regen lost the
    // whole game): worms show many phases, terrain carves, every pool goes live.
    let worm0_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm0).collect();
    let worm1_cols: std::collections::HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(worm0_cols.len() >= 3 && worm1_cols.len() >= 3, "{}: both worms move/live/die", v.name);
    let levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(levels.len() >= 2, "{}: terrain carves (>=2 level hashes)", v.name);
    assert!(golden.iter().any(|g| g.pools[0] != EMPTY_POOL), "{}: bobjects (blood)", v.name);
    assert!(golden.iter().any(|g| g.pools[1] != EMPTY_POOL), "{}: bonuses drop", v.name);
    assert!(golden.iter().any(|g| g.pools[2] != EMPTY_POOL), "{}: sobjects (explosions)", v.name);
    assert!(golden.iter().any(|g| g.pools[3] != EMPTY_POOL), "{}: nobjects (spray/gib)", v.name);
    assert!(golden.iter().any(|g| g.pools[4] != EMPTY_POOL), "{}: wobjects (in-flight)", v.name);

    // Drive + assert every tick bit-exact (THE MILESTONE), then return the diagnostics.
    drive(&scenario, Some(&golden))
}

#[test]
fn fuzz1_full_processframe_match_cpp_oracle() {
    assert_guards("fuzz1", &run_variant(&VARIANTS[0]));
}
#[test]
fn fuzz2_full_processframe_match_cpp_oracle() {
    assert_guards("fuzz2", &run_variant(&VARIANTS[1]));
}
#[test]
fn fuzz3_full_processframe_match_cpp_oracle() {
    assert_guards("fuzz3", &run_variant(&VARIANTS[2]));
}
#[test]
fn fuzz4_full_processframe_match_cpp_oracle() {
    assert_guards("fuzz4", &run_variant(&VARIANTS[3]));
}
#[test]
fn fuzz5_full_processframe_match_cpp_oracle() {
    assert_guards("fuzz5", &run_variant(&VARIANTS[4]));
}

/// Per-variant coverage guards that hold for EVERY variant (from the driven state).
fn assert_guards(name: &str, d: &RunDiag) {
    // (death) a worm's lives strictly decreased.
    assert!(d.lives_decreased, "{name}: a worm's lives must DECREASE (a death cost a life)");
    // (respawn) the rng column moved on a BeginRespawn spawn-search tick.
    assert!(
        d.rng_move_on_respawn.is_some(),
        "{name}: the rng column must move on a respawn-search (BeginRespawn) tick"
    );
    // (in-flight hit) a worm's health dropped on an in-flight projectile-hit tick.
    assert!(
        d.inflight_hit.is_some(),
        "{name}: a worm's health must DROP on an in-flight-hit tick"
    );
    // (bonus pool) a drop landed (grew) and the column moved. NOTE: the SHRINK half of
    // "grows then shrinks" is variant-specific — with max_bonuses 4 and long bonus timers,
    // fuzz1/fuzz3 fill the pool and never drain inside 1500 ticks (see
    // `bonus_pool_grows_then_shrinks_and_a_pickup_occurs` for the collective proof).
    assert!(d.bonus_grew, "{name}: the bonuses pool must GROW (a drop landed)");
    assert!(d.bonus_col_moved, "{name}: the bonuses column must move");
    // (rope spray) the nobjects column bumped on a ninjarope terrain-attach tick.
    assert!(
        d.rope_spray.is_some(),
        "{name}: the nobjects column must BUMP on a ninjarope terrain-attach tick (rand(128) spray)"
    );
    // (pool caps) blood + nobjects never exceed their capacity.
    assert!(d.cap_never_exceeded, "{name}: no pool may exceed capacity");
    assert!(d.peak_bobjects <= BLOOD_CAP, "{name}: blood <= {BLOOD_CAP}");
    assert!(d.peak_nobjects <= NOBJ_CAP, "{name}: nobjects <= {NOBJ_CAP}");
}

/// Diagnostic printer for the per-variant guard matrix (run with `--nocapture`). Not an
/// assertion — it exists so the report's matrix is reproducible from the driven state.
#[test]
fn print_guard_matrix() {
    println!("\n=== T8 per-variant guard matrix (from the DRIVEN state) ===");
    for v in &VARIANTS {
        let d = run_variant(v);
        println!(
            "{}: lives {:?}->{:?} dec={} | respawns={} rng_move={:?} | bonus grew={} shrank={} peak={} colmov={} | inflight={} first={:?} | rope_att={} spray={:?} | peakBob={}@t{} peakNob={} capOK={} | pickups={}",
            v.name,
            d.start_lives, d.final_lives, d.lives_decreased,
            d.n_respawns, d.rng_move_on_respawn,
            d.bonus_grew, d.bonus_shrank, d.peak_bonuses, d.bonus_col_moved,
            d.n_inflight_hits, d.inflight_hit,
            d.n_rope_attaches, d.rope_spray,
            d.peak_bobjects, d.peak_bobjects_tick, d.peak_nobjects, d.cap_never_exceeded,
            d.n_pickups,
        );
    }
}

/// Pure-Rust determinism backstop: two INDEPENDENT `SimState` runs of each variant must
/// produce an identical master `hash_game_state` on every tick. Proves no nondeterminism
/// (iteration order, uninit reads, time) entered the port — orthogonal to the C++ oracle.
#[test]
fn each_variant_is_internally_deterministic() {
    for v in &VARIANTS {
        let text =
            std::fs::read_to_string(format!("{}/golden/{}", env!("CARGO_MANIFEST_DIR"), v.scenario))
                .expect("read scenario");
        let scenario = Scenario::parse(&text).expect("scenario parses");
        let a = drive(&scenario, None);
        let b = drive(&scenario, None);
        assert_eq!(a.master_hashes.len(), 1501, "{}: 1501 master hashes", v.name);
        assert_eq!(
            a.master_hashes, b.master_hashes,
            "{}: two runs must be hash-identical every tick (determinism)",
            v.name
        );
    }
}

/// Collective coverage: at least one variant drives the blood pool close to its 700 cap
/// (the `spawn_reuse`/`NewObjectReuse` path). T7 measured V1 700/700 PRE-FIX; this pins
/// the honest POST-FIX peak so a regression that stops filling the pool fails here.
#[test]
fn blood_pool_approaches_capacity_in_at_least_one_variant() {
    let peaks: Vec<usize> = VARIANTS.iter().map(|v| run_variant(v).peak_bobjects).collect();
    let max_peak = *peaks.iter().max().unwrap();
    assert!(
        peaks.iter().all(|&p| p <= BLOOD_CAP),
        "no variant may exceed the blood cap; peaks {peaks:?}"
    );
    assert!(
        max_peak >= 600,
        "at least one variant must drive blood near its {BLOOD_CAP} cap; peaks {peaks:?}"
    );
}

/// Collective bonus coverage (the SHRINK + PICKUP halves the per-variant guards can't own,
/// since with max_bonuses 4 and long bonus timers fuzz1/fuzz3 fill the pool and never drain
/// inside 1500 ticks). At least one variant must show the pool GROW then SHRINK (a drop then
/// a departure) AND at least one variant must show a genuine worm PICKUP (a bonus vanishing
/// off a worm's 11x11 AABB). Both are read fresh from the driven state.
#[test]
fn bonus_pool_grows_then_shrinks_and_a_pickup_occurs() {
    let diags: Vec<RunDiag> = VARIANTS.iter().map(run_variant).collect();
    assert!(
        diags.iter().all(|d| d.bonus_grew),
        "every variant must land a bonus drop (pool grows)"
    );
    assert!(
        diags.iter().any(|d| d.bonus_grew && d.bonus_shrank),
        "at least one variant must show the bonuses pool grow THEN shrink"
    );
    assert!(
        diags.iter().any(|d| d.n_pickups >= 1),
        "at least one variant must show a genuine worm bonus pickup"
    );
}
