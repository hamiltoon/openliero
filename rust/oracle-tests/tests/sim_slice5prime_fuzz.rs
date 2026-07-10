//! Slice-5' **T10** — MOVING-WORMS per-pixel worm-hit FUZZ (O21; the direct precondition
//! for the slice-6 fuzz with moving worms). Four DART-duel variants on the SAME
//! `physics_fall_test.lev`, each tuned so the in-flight per-pixel `CheckForSpecWormHit`
//! (`worm.cpp:1162-1188`, gated by the wobject arm `weapon.cpp:287-326`) reads the
//! victim's sprite at a **moving** `current_frame`/`direction`.
//!
//! ## Why this exists (the coverage the 5'a MILESTONE cannot give)
//!
//! The 5'a milestone (`sim_slice5prime_golden.rs`) landed the arm on an IDLE victim: worm1
//! stood still at frame-0/dir-0, so every hit was read against the SAME `cf0`/`dir0`
//! silhouette. That pins the per-pixel predicate but exercises the `animate`/`current_frame`
//! promotion (design **O-A risk**) at a single point. T10 makes BOTH worms **walk**
//! (`animate=true`) and **fire** flat DARTs at each other; the crossing darts per-pixel-hit
//! the opposing MOVING worm at a walk-animation `current_frame`/`direction`. A bit-exact
//! match over all 76 ticks × 4 variants proves `SimState::worm_sprite(current_frame,
//! direction)` — the moving-worm sprite selection — is right across frames AND directions,
//! not just the idle corner.
//!
//! ## The symmetric duel (see `golden/sim_slice5prime_fuzz1_scenario.txt`)
//!
//! worm1 (LEFT, flips `dir1` to face RIGHT) and worm0 (RIGHT, `dir0` faces LEFT) both raise
//! the gun ~27 ticks to the FLAT skim so each dart skims at worm height (a low/steep dart
//! plunges and ground-explodes next to the worms — blast falloff, NOT the per-pixel arm; the
//! flat raise keeps the surface ISOLATED to the in-flight hit), then both fire on tick 40.
//! The two darts cross in the gap and each per-pixel-hits the opposing worm while it walks:
//! worm0 (`dir0`,`animate`) by worm1's rightward dart, worm1 (`dir1`,`animate`) by worm0's
//! leftward dart. Each takes clean `DoDamage(5)` wounds + a 10-blood fan; both SURVIVE
//! (worm1 starts at health 60). The variants differ ONLY in the starting SPACING, which
//! shifts the hit tick into a different `kWormAnimTab` bucket => a different `current_frame`:
//!
//!   * fuzz1  gap 30px  -> hits ~tick 45 at **current_frame 9**  (dir0 & dir1)
//!   * fuzz2  gap 50px  -> hits ~tick 52 at **current_frame 2**  (+ cf3 on worm0)
//!   * fuzz3  gap 60px  -> hits ~tick 56 at **current_frame 17** (dir0 & dir1)
//!   * fuzz4  gap 40px  -> hits ~tick 48 at **current_frame 2**  (dir0 & dir1)
//!
//! Collectively the variants land moving-worm hits at frames {2, 9, 17} × directions {0, 1}
//! — far more than the **>= 2 distinct `current_frame`/direction combos** the task requires.
//! Every recorded hit is on a worm with `animate == true` (a genuine walk frame). The
//! coverage combos are DERIVED from the driven `SimState` (health-drop tick + the victim's
//! `current_frame`/`direction`/`animate` there), never re-parsed from the golden.
//!
//! Each variant asserts master `HashGameState` + all 9 component hashes bit-exact for EVERY
//! tick (0..=75). A pure-Rust determinism backstop (two independent `SimState` runs per
//! variant, master hash identical every tick) proves no nondeterminism entered the port. The
//! goldens are LOCAL/MANUAL C++-dumper output (`gen_sim_slice5prime_fuzz{1..4}_golden.sh`),
//! the dumper UNCHANGED since 5c.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;
use std::collections::HashSet;

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

/// A T10 fuzz variant: scenario/golden basenames + an INTENT GUARD `(current_frame,
/// direction)` — the moving-worm hit combo the C++ oracle produced. A regenerated golden
/// that lost the moving-worm hit (e.g. a worm frozen idle) fails the guard HERE, loudly,
/// not silently.
struct Variant {
    name: &'static str,
    scenario: &'static str,
    golden: &'static str,
    /// A `(current_frame, direction)` combo that MUST appear among this variant's
    /// moving-worm hits (the victim was `animate == true` at the hit).
    expected_hit: (i32, i32),
}

const VARIANTS: [Variant; 4] = [
    Variant { name: "fuzz1", scenario: "sim_slice5prime_fuzz1_scenario.txt", golden: "sim_slice5prime_fuzz1.txt", expected_hit: (9, 1) },
    Variant { name: "fuzz2", scenario: "sim_slice5prime_fuzz2_scenario.txt", golden: "sim_slice5prime_fuzz2.txt", expected_hit: (2, 1) },
    Variant { name: "fuzz3", scenario: "sim_slice5prime_fuzz3_scenario.txt", golden: "sim_slice5prime_fuzz3.txt", expected_hit: (17, 1) },
    Variant { name: "fuzz4", scenario: "sim_slice5prime_fuzz4_scenario.txt", golden: "sim_slice5prime_fuzz4.txt", expected_hit: (2, 1) },
];

/// Everything the coverage guards read from the genuinely DRIVEN state (never re-parsed
/// from the golden).
struct RunDiag {
    /// `(current_frame, direction)` of every hit that landed while the victim was
    /// `animate == true` (a MOVING-worm hit) — the T10 coverage.
    moving_hit_combos: Vec<(i32, i32)>,
    /// Both worms walked (`animate == true` on >= 1 tick)?
    worm_moved: [bool; 2],
    /// Both worms fired (slot-0 ammo dropped below its start)?
    worm_fired: [bool; 2],
    /// Final health per worm (both must stay > 0 — no death/respawn this fuzz).
    final_health: [i32; 2],
    /// How many distinct ticks a worm's health dropped (>= 1 hit landed at all).
    hit_ticks: usize,
    max_nobjects: usize,
    saw_type6_blood: bool,
    /// master `hash_game_state` per tick 0..=ticks (for the determinism backstop).
    master_hashes: Vec<u32>,
}

/// Build the tick-0 `SimState` for a variant's scenario — IDENTICAL setup to the 5'a
/// milestone harness (real level/tc/objects, DART slot 0, all blood/respawn consts assigned
/// post-`new`).
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

    // id == index invariants (indexed lookups on Fire/blood spray/worm-hit).
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
    assert_eq!(objects.weapons[weapon_idx].hit_damage, 5, "DART hit_damage is 5");
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
    // Blood consts (as 5'a/5b/5d); left at `new`'s 0 the blood fan diverges at the first
    // hit tick — a forgotten const, not a sim bug. Respawn consts mirrored (harmless: no
    // respawn runs — the worms survive) to match the unchanged C++ dumper's held consts.
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
    let start_ammo = [state.worms[0].weapons[0].ammo, state.worms[1].weapons[0].ammo];

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

    let mut prev_health = [state.worms[0].health, state.worms[1].health];
    let mut moving_hit_combos: Vec<(i32, i32)> = Vec::new();
    let mut worm_moved = [false; 2];
    let mut worm_fired = [false; 2];
    let mut hit_ticks = 0usize;
    let mut max_nobjects = 0usize;
    let mut saw_type6_blood = false;
    let mut master_hashes: Vec<u32> = Vec::new();
    let mut bonuses_always_empty = true;

    let mut record_witnesses = |state: &SimState, at_tick: bool| {
        for wi in 0..2 {
            if state.worms[wi].animate {
                worm_moved[wi] = true;
            }
            if state.worms[wi].weapons[0].ammo < start_ammo[wi] {
                worm_fired[wi] = true;
            }
        }
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if state.nobjects.iter().any(|n| n.ty == Some(6)) {
            saw_type6_blood = true;
        }
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
        }
        if at_tick {
            // A hit landed this tick iff a worm's health dropped. Record the victim's
            // (current_frame, direction) — and, when it was animate, count it as a
            // MOVING-worm hit (the T10 coverage).
            for wi in 0..2 {
                let h = state.worms[wi].health;
                if h < prev_health[wi] {
                    hit_ticks += 1;
                    if state.worms[wi].animate {
                        moving_hit_combos
                            .push((state.worms[wi].current_frame, state.worms[wi].direction));
                    }
                }
                prev_health[wi] = h;
            }
        }
        master_hashes.push(hash_game_state(state));
    };

    // Tick 0 (no process_frame).
    if let Some(g) = golden {
        assert_eq!(g[0].tick, 0, "first golden row is tick 0");
        assert_tick(&state, &g[0]);
    }
    record_witnesses(&state, false);

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
        record_witnesses(&state, true);
    }

    assert!(bonuses_always_empty, "bonuses must stay empty (max_bonuses 0)");

    RunDiag {
        moving_hit_combos,
        worm_moved,
        worm_fired,
        final_health: [state.worms[0].health, state.worms[1].health],
        hit_ticks,
        max_nobjects,
        saw_type6_blood,
        master_hashes,
    }
}

/// Full per-tick bit-exact assert + single-variant moving-worm coverage. Shared by the four
/// `#[test]` entry points.
fn run_variant(v: &Variant) -> RunDiag {
    let scenario_text = std::fs::read_to_string(format!(
        "{}/golden/{}",
        env!("CARGO_MANIFEST_DIR"),
        v.scenario
    ))
    .expect("read scenario");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "{}: seed 42", v.name);
    assert_eq!(scenario.ticks, 75, "{}: 75 ticks", v.name);
    assert_eq!(scenario.max_bonuses, 0, "{}: no bonuses", v.name);
    assert_eq!(scenario.worms.len(), 2, "{}: two worms", v.name);
    assert_eq!(scenario.worms[0].health, 100, "{}: worm0 full health", v.name);
    assert_eq!(scenario.worms[1].health, 60, "{}: worm1 starts at 60 (survives its wounds)", v.name);
    assert_eq!(scenario.weapon(0), Some("DART"), "{}: DART slot 0", v.name);

    let golden_text = std::fs::read_to_string(format!(
        "{}/golden/{}",
        env!("CARGO_MANIFEST_DIR"),
        v.golden
    ))
    .expect("read golden");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "{}: golden 0..=75", v.name);

    // Golden-shape headline (fails loudly before the per-tick loop if a regen lost the
    // duel): BOTH worm columns show >=2 phases (idle, then wounded+moving), the darts fly
    // (wobjects), the blood fan spawns (nobjects + bobjects), terrain carves; bonuses stay
    // empty. (sobjects DO go live — the spent darts explode on far terrain at the tail — so
    // they are NOT asserted empty, unlike the isolated milestone.)
    let worm0_cols: HashSet<u32> = golden.iter().map(|g| g.worm0).collect();
    let worm1_cols: HashSet<u32> = golden.iter().map(|g| g.worm1).collect();
    assert!(worm0_cols.len() >= 2, "{}: worm0 >=2 phases (moving, then wounded)", v.name);
    assert!(worm1_cols.len() >= 2, "{}: worm1 >=2 phases (moving, then wounded)", v.name);
    let levels: HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(levels.len() >= 2, "{}: terrain carves (>=2 level hashes)", v.name);
    assert!(golden.iter().any(|g| g.pools[4] != EMPTY_POOL), "{}: darts fly (wobjects)", v.name);
    assert!(golden.iter().any(|g| g.pools[3] != EMPTY_POOL), "{}: blood fan (nobjects)", v.name);
    assert!(golden.iter().any(|g| g.pools[0] != EMPTY_POOL), "{}: blood drip (bobjects)", v.name);
    for g in &golden {
        assert_eq!(g.pools[1], EMPTY_POOL, "{}: tick {} bonuses empty", v.name, g.tick);
    }

    // Drive + assert every tick bit-exact.
    let d = drive(&scenario, Some(&golden));

    // ---- Single-variant MOVING-WORM coverage (from the driven state) ----------------
    // Both worms genuinely MOVED (walked; animate=true) and FIRED — "both worms move and
    // fire", the plan's headline. Neither is an idle copy of the milestone victim.
    assert!(d.worm_moved[0] && d.worm_moved[1], "{}: BOTH worms must walk (animate); saw {:?}", v.name, d.worm_moved);
    assert!(d.worm_fired[0] && d.worm_fired[1], "{}: BOTH worms must fire a DART; saw {:?}", v.name, d.worm_fired);

    // >=1 hit landed, and >=1 landed on a MOVING worm (the whole T10 point).
    assert!(d.hit_ticks >= 1, "{}: a per-pixel worm-hit must land", v.name);
    assert!(!d.moving_hit_combos.is_empty(), "{}: >=1 hit must land on a MOVING worm (animate)", v.name);
    assert!(d.saw_type6_blood, "{}: the hit spawns type-6 blood nobjects", v.name);

    // The INTENT GUARD: the C++-oracle-measured moving-worm combo is present (a regenerated
    // golden that froze a worm idle, or lost the hit, fails HERE).
    assert!(
        d.moving_hit_combos.contains(&v.expected_hit),
        "{}: expected moving-worm hit at (current_frame {}, dir {}); saw {:?}",
        v.name, v.expected_hit.0, v.expected_hit.1, d.moving_hit_combos
    );

    // No death/respawn mixes in — both worms survive the wound-only duel.
    assert!(d.final_health[0] > 0 && d.final_health[1] > 0, "{}: both worms survive; final {:?}", v.name, d.final_health);
    // Blood stays under the O3 pool cap.
    assert!(d.max_nobjects < 600, "{}: nobjects under the 600 cap; peaked at {}", v.name, d.max_nobjects);

    d
}

#[test]
fn fuzz1_moving_worm_hit_match_cpp_oracle() {
    run_variant(&VARIANTS[0]);
}
#[test]
fn fuzz2_moving_worm_hit_match_cpp_oracle() {
    run_variant(&VARIANTS[1]);
}
#[test]
fn fuzz3_moving_worm_hit_match_cpp_oracle() {
    run_variant(&VARIANTS[2]);
}
#[test]
fn fuzz4_moving_worm_hit_match_cpp_oracle() {
    run_variant(&VARIANTS[3]);
}

/// The COLLECTIVE T10 coverage requirement: across the four variants, the moving-worm hits
/// land at **>= 2 distinct `(current_frame, direction)` combos** — the moving-worm sprite
/// selection is genuinely exercised across frames AND directions, not one silhouette
/// replayed. (In practice frames {2, 9, 17} × directions {0, 1} appear — well over 2.)
#[test]
fn variants_land_at_least_two_distinct_frame_direction_combos() {
    let combos: HashSet<(i32, i32)> = VARIANTS
        .iter()
        .flat_map(|v| run_variant(v).moving_hit_combos)
        .collect();
    assert!(
        combos.len() >= 2,
        "T10 requires >=2 distinct moving-worm (current_frame,direction) combos; saw {combos:?}"
    );
    // Both directions must be represented (a dir0 victim AND a dir1 victim were hit while
    // moving) — the milestone only ever saw dir0.
    assert!(combos.iter().any(|&(_, d)| d == 0), "a dir0 worm must be hit while moving; {combos:?}");
    assert!(combos.iter().any(|&(_, d)| d == 1), "a dir1 worm must be hit while moving; {combos:?}");
    // At least two distinct current_frames (the milestone only ever saw cf0).
    let frames: HashSet<i32> = combos.iter().map(|&(f, _)| f).collect();
    assert!(frames.len() >= 2, ">=2 distinct current_frames across variants; {frames:?}");
    assert!(!frames.contains(&0), "no moving hit is the milestone's idle cf0; {frames:?}");
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
    }
}
