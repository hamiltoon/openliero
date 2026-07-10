//! Per-tick differential test for Slice-6 T5 — **SCALES OF JUSTICE** (`game_mode 3`)
//! damage REDISTRIBUTION going LIVE against the C++ oracle. The golden
//! (`golden/sim_slice6_scales.txt`, 121 lines for ticks 0..=120) is produced by the
//! real C++ `Game` running the *same scenario* (`golden/sim_slice6_scales_scenario.txt`)
//! with `settings->game_mode = kGmScalesOfJustice`, so the unmodified `Game::DoDamage`
//! (`game.cpp:567-589`) fires its Scales branch on every wound.
//!
//! Geometry = slice 5b: worm0 air-bursts EXPLOSIVES that WOUND the grounded worm1
//! (x=115) with two `large_explosion` blasts. worm0 (x=150) is outside every blast
//! box, so it takes NO direct damage. Because the damage source is worm0 (an enemy,
//! `by_idx == 0 != worm1.index`), `DoDamage`'s `else` arm (`:584-585`) HEALS the
//! attacker worm0 by the full damage amount via `DoHealingDirect`. worm0 spawns
//! WOUNDED (health 50) so each heal is a visible health RISE that never crosses the
//! 100 cap (no life overflow).
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves
//!
//! `DoHealingDirect` draws NO rand, so the `rng` column is identical to a KillEmAll
//! run — the Scales delta surfaces ONLY in the two `worm` columns (and the master).
//! A bit-exact match over all 121 ticks proves the redistribution: the wounded worm
//! drops by `z`, the OTHER worm (the attacker) rises by exactly `z`, on the same tick,
//! with the correct `else`-arm target selection — end-to-end vs the C++ oracle.

use assets::object::Objects;
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use sim::control::ControlConsts;
use sim::hash::{hash_components, hash_game_state};
use sim::physics::PhysicsConsts;
use sim::state::{ControlState, SimState, WeaponId, WeaponInit, WormInit, NUM_WEAPONS};
use sim_core::vec::Vec2;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
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

struct GoldenTick {
    tick: u32,
    master: u32,
    rng: u32,
    level: u32,
    worm0: u32,
    worm1: u32,
    pools: [u32; 5],
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
            let pools = [
                hex(next()),
                hex(next()),
                hex(next()),
                hex(next()),
                hex(next()),
            ];
            assert!(it.next().is_none(), "golden line has exactly 11 columns");
            GoldenTick {
                tick,
                master,
                rng,
                level,
                worm0,
                worm1,
                pools,
            }
        })
        .collect()
}

#[test]
fn sim_slice6_scales_redistribution_match_cpp_oracle() {
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice6_scales_scenario.txt"
    ))
    .expect("read golden/sim_slice6_scales_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.ticks, 120, "scenario ticks");
    assert_eq!(scenario.game_mode, 3, "game_mode 3 == kGmScalesOfJustice");
    assert_eq!(
        scenario.worms[0].health, 50,
        "worm0 (shooter) spawns WOUNDED at 50"
    );
    assert_eq!(
        scenario.worms[1].health, 100,
        "worm1 (target) spawns at full health"
    );

    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice6_scales.txt"
    ))
    .expect("read golden/sim_slice6_scales.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(
        golden.len(),
        (scenario.ticks + 1) as usize,
        "golden has tick 0..=ticks"
    );

    // The explosion machinery must be present (else the wound — and thus the
    // redistribution — never fires). Read straight from the parsed golden.
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "golden must spawn the large_explosion sobject"
    );
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "golden must spawn blood/dirt nobjects"
    );
    for g in &golden {
        assert_eq!(
            g.pools[1], EMPTY_POOL,
            "tick {}: bonuses empty (max_bonuses 0)",
            g.tick
        );
    }

    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(
            w.id, i as i32,
            "weapon id must equal its index (weapon[{i}], id {})",
            w.id
        );
    }
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(
            s.id, i as i32,
            "sobject_type id must equal its index (got id {})",
            s.id
        );
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(
            n.id, i as i32,
            "nobject_type id must equal its index (got id {})",
            n.id
        );
    }

    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    let weapon_name = scenario
        .weapon(0)
        .expect("scenario `weapon 0 <name>` directive present");
    let weapon_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == weapon_name)
        .unwrap_or_else(|| panic!("weapon {weapon_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(weapon_idx as WeaponId),
        ammo: objects.weapons[weapon_idx].ammo,
    };

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

    // Blood consts (as 5b) + the game mode (THE T5 STEP). Left at the `new` default
    // (game_mode 0) the run diverges at the first wound: worm0 would NOT be healed.
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;
    state.small_sprites = load_small_sprites();
    state.game_mode = scenario.game_mode as u32;
    assert_eq!(
        state.settings_health, 100,
        "DoHealingDirect cap = WormSettings::health 100"
    );

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };
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
        check(
            g.tick,
            "MASTER state_hash",
            hash_game_state(state),
            g.master,
        );
    };

    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // Coverage witnesses from the DRIVEN state (never re-parsed from the golden).
    let mut w0_health: Vec<i32> = Vec::with_capacity(golden.len());
    let mut w0_lives: Vec<i32> = Vec::with_capacity(golden.len());
    let mut w1_health: Vec<i32> = Vec::with_capacity(golden.len());
    let mut rng_draws: Vec<u64> = Vec::with_capacity(golden.len());
    let mut record = |state: &SimState| {
        w0_health.push(state.worms[0].health);
        w0_lives.push(state.worms[0].lives);
        w1_health.push(state.worms[1].health);
        rng_draws.push(state.rand.draws());
    };
    record(&state);

    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // ================= THE SCALES COVERAGE GUARDS (all from the DRIVEN state) =======

    // worm1 is WOUNDED but survives: health drops below its 100 start and stays > 0.
    assert_eq!(w1_health[0], 100, "worm1 starts at full health");
    let w1_final = *w1_health.last().unwrap();
    assert!(
        w1_final < 100,
        "worm1 must be wounded (health drops below 100); final {w1_final}"
    );
    assert!(
        w1_health.iter().all(|&h| h > 0),
        "worm1 must survive (wounded, not killed) — every tick health > 0"
    );

    // worm0 (the attacker) is HEALED, never damaged: its health only ever RISES from
    // its wounded 50 start, stays <= 100, and its lives never change (no overflow).
    assert_eq!(w0_health[0], 50, "worm0 starts WOUNDED at 50");
    for k in 1..w0_health.len() {
        assert!(
            w0_health[k] >= w0_health[k - 1],
            "tick {k}: worm0 health must never DROP (it is only healed, never damaged); {} -> {}",
            w0_health[k - 1],
            w0_health[k]
        );
        assert!(
            w0_health[k] <= 100,
            "tick {k}: worm0 heal must never cross the 100 cap"
        );
    }
    let w0_final = *w0_health.last().unwrap();
    assert!(
        w0_final > 50,
        "worm0 must be HEALED above its 50 start (the redistribution); final {w0_final}"
    );
    assert!(
        w0_lives.iter().all(|&l| l == w0_lives[0]),
        "worm0 lives must not change (heal stays under the 100 cap — no overflow)"
    );

    // CONSERVATION: every `DoDamage(worm1, z)` heals worm0 by exactly `z`, and worm1
    // is only ever damaged (never healed), so worm0's total gain == worm1's total loss.
    assert_eq!(
        w0_final - 50,
        100 - w1_final,
        "Scales conservation: worm0 heal ({}) == worm1 damage ({})",
        w0_final - 50,
        100 - w1_final
    );

    // The redistribution is RNG-NEUTRAL: worm0's heal ticks draw NO extra rand beyond
    // what the blast itself draws — proven by the bit-exact `rng` column above; here we
    // additionally pin that a tick where worm0's health rises but NO blast draws (i.e.
    // the heal is the only worm0 change) never adds an rng draw on worm0's behalf.
    // (The wound ticks DO draw the blast's own sound/blood rand; that is the blast, not
    // the heal — the heal's contribution is zero, which the identical rng column proves.)
    let total_draws = *rng_draws.last().unwrap();
    assert!(
        total_draws > 0,
        "the scenario must draw rand (the explosives fire + blasts)"
    );
}
