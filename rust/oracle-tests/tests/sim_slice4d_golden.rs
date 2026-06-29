//! Per-tick differential test for the Slice-4d HANDGUN scenario against the C++
//! oracle — THE MILESTONE of 4d (and the final differential test of all of Slice
//! 4). The golden (`golden/sim_slice4d.txt`, 126 lines for ticks 0..=125) is
//! produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice4d_scenario.txt`): seed 42, the LOADED `physics_fall_test.lev`,
//! worm0 visible and grounded with the **HANDGUN** in weapon slot 0 carrying
//! **ammo 2**, worm1 invisible and parked high. worm0 lands, raises the gun, FIRES
//! two arced shots (input ticks 38 + 64) that explode downrange (4c cluster), drops
//! a SHELL after each shot, exhausts the magazine on the 2nd shot so ProcessWeapons
//! ARMS a RELOAD (`loading_left = 220`, `ammo = 15`), cycles weapons mid-reload
//! (load_change), then DIGS the dirt floor (Down held, L+R toggled) carving the
//! `level` repeatedly.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — the five 4d deferrals are bit-exact
//!
//! A tick-for-tick match over 126 ticks proves the five Slice-4d deferrals are
//! bit-exact: **dig** (`draw_dirt_effect` carving on the Down-held digs), **shell
//! drop** (`leave_shell` spawning a SHELL nobject after each shot, +5 rng),
//! **reload** (`ProcessWeapons` arming `loading_left = ComputedLoadingTime = 220`
//! and resetting `ammo` to the handgun default 15 when the magazine empties),
//! **load_change** (cycling `current_weapon` WHILE `loading_left > 0`), AND — by the
//! match holding even though handgun has `laserSight = true` — that omitting
//! **ProcessSight** is correct (the sight draws nothing into the sim state).
//!
//! reload (`loading_left`/`ammo`) and load_change (`current_weapon`) are NOT isolated
//! golden columns — they fold into worm0 (and thence the master). So the per-component
//! + master tick match is what proves them bit-exact: a wrong `loading_left`/`ammo`/
//! `current_weapon` shows up as a worm0 (or master-only) divergence.
//!
//! The component columns are asserted FIRST (rng -> level -> worm0 -> worm1 ->
//! bobjects -> bonuses -> sobjects -> nobjects -> wobjects) THEN the master
//! `state_hash`, so a divergence localises to a tick + subsystem before the master
//! flags it: `rng` => a wrong draw (shell 5-draw / dig 2-draw / fire 4-draw count or
//! order); `level` => the dig carve or the explosion crater; `nobjects`(pos) => the
//! shell velocity or the dirt-throw `Create2`; master-only (every component matches)
//! => a worm0 field (loading_left/ammo/current_weapon) or an nobject vel/cur_frame
//! (O11 — the nobject component fold is `pos` only).
//!
//! The scenario is the single source of truth (parsed via `oracle_tests::scenario`,
//! including the optional 3rd ammo token `weapon 0 HANDGUN 2`) and the expected
//! values are PARSED from the golden file, never hard-coded.

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
/// `draw_dirt_effect`: both the explosion (`small_explosion`'s `dirt_effect = 2`) and
/// the digs index this bank, so an empty bank would panic.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
}

/// Load the shipped 7x7 small-sprite bank (C++ `small_sprites.Allocate(7,7,130)`)
/// from `sprites/small.tga`. Threaded into `SimState` for the shell-landing
/// `BlitImageOnMap`: the spent SHELL (`nobject_types[7]`, `draw_on_map=true`,
/// `start_frame=45`) paints `small_sprites[45 + cur_frame]` into the terrain when it
/// lands, so an empty bank would index OOB.
fn load_small_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/small.tga")).expect("read small.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("small.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 7, 7, 130).expect("small sprite bank")
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
fn sim_slice4d_handgun_deferrals_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4d_scenario.txt"
    ))
    .expect("read golden/sim_slice4d_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 125, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=125, master + 9 components). -----
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice4d.txt"
    ))
    .expect("read golden/sim_slice4d.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 4d HEADLINE read straight from the parsed golden: the run must carve
    // terrain (explosion crater + repeated digs => `level` moves), spawn an sobject
    // (the explosion), and spawn nobjects (the shells + dirt debris).
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(
        golden_levels.len() >= 2,
        "4d golden must carve: level column takes >=2 distinct values; saw {:?}",
        golden_levels
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "4d golden must spawn an sobject (sob column leaves the empty-pool hash)"
    );
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "4d golden must spawn nobjects (nob column leaves the empty-pool hash)"
    );

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). --------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants + the object
    // tables (weapons, nobject_types, sobject_types), exactly as the dumper's
    // `common` holds them; `tc.cfg` carries the materials, textures, large-sprite
    // bank name and the physics/control consts. -------------------------------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // INVARIANT (load-bearing for the Fire path): `weapon.id == array index`. The
    // spawned `WObject.ty` is set to `weapon.id` in weapon_fire, and wobject_process
    // indexes `weapons[ty]` — those only line up if id == index.
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(
            w.id, i as i32,
            "weapon id must equal its index (weapon[{i}] = {:?}, id {})",
            w.name, w.id
        );
    }
    // INVARIANT (load-bearing for `create_on_exp` + the dirt-throw): the object
    // tables are indexed by id. If id != index those lookups read the wrong object.
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index (got id {})", n.id);
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-4c.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with the HANDGUN (the scenario's `weapon 0`
    // directive, single source of truth — UPPERCASE "HANDGUN" matching the TC
    // weapon-table name, not the filename "handgun"), carrying the LOW ammo from the
    // scenario's optional 3rd token (`weapon 0 HANDGUN 2`) so the 2nd shot empties
    // the magazine and arms a reload. The dumper leaves current_weapon = 0.
    let hg_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let hg_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == hg_name)
        .unwrap_or_else(|| panic!("weapon {hg_name:?} present in TC weapon table"));
    // The scenario carries the starting ammo as the 3rd `weapon` token (parsed by the
    // extended scenario parser). Prefer it over the weapon's default ammo so the
    // scenario stays the single source of truth (matches the dumper's [ammo] token).
    let start_ammo = scenario
        .weapon_ammo(0)
        .expect("scenario `weapon 0 HANDGUN 2` carries the ammo token");
    assert_eq!(start_ammo, 2, "scenario starts the handgun with ammo 2 (empties on shot 2)");
    // Handgun's DEFAULT ammo (the value a reload resets to) — pinned for the guard.
    let handgun_default_ammo = objects.weapons[hg_idx].ammo;
    assert_eq!(handgun_default_ammo, 15, "handgun.cfg ammo = 15 (reload target)");
    resolved[0] = WeaponInit { ty: Some(hg_idx as WeaponId), ammo: start_ammo };

    // --- Build the two worm inits from the scenario. BOTH worms get the handgun in
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

    // --- Build tick-0 state. The two trailing scalars are the 4d reload/load_change
    // settings, matching THE DUMPER (the single source of truth,
    // `sim_physics_dump.cpp`), NOT the Settings defaults: the dumper sets
    // `settings->loading_time = 0` (it leaves `load_change = true`, the default).
    //
    // loading_time = 0 is load-bearing: `ComputedLoadingTime = max((0 * 220) / 100,
    // 1) = 1`, so a reload arms `loading_left = 1` and the same-tick countdown
    // (`if loading_left > 0 { -- }`) drops it to 0 in the SAME ProcessWeapons call.
    // The reload is therefore EFFECTIVELY INSTANT: `loading_left` is 0 at every tick
    // boundary, `Available()` is always true, and the reload's only boundary-visible
    // effect is the ammo reset (0 -> 15). (Passing the Settings default 100 would arm
    // loading_left = 220 and diverge from the golden at the first reload, tick 66.)
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
    );
    // The 7x7 small-sprite bank is set post-construction (kept out of the
    // SimState::new arg list to leave the other slices' call sites unchanged). The
    // shell-landing BlitImageOnMap indexes it (`small_sprites[45 + cur_frame]`);
    // without it the shell's ground-paint at tick 117 would index an empty bank.
    state.small_sprites = load_small_sprites();

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
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
        // The MASTER, last: it folds the reload/load_change worm0 state, the shell +
        // dirt nobjects (vel+cur_frame, wider than the component fold — O11), the live
        // sobject, the explosion/dig RNG AND the carved level.
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage trackers, all read from the DRIVEN SimState (never re-parsed from
    // the golden) so the asserts below are a genuine witness of the deferrals firing.
    let mut rng_by_tick: Vec<u32> = Vec::with_capacity(golden.len());
    let mut master_by_tick: Vec<u32> = Vec::with_capacity(golden.len());
    let mut slot0_loading_by_tick: Vec<i32> = Vec::with_capacity(golden.len());
    let mut ammo0_by_tick: Vec<i32> = Vec::with_capacity(golden.len());
    let mut slot0_ammo_seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut current_weapon_seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut nob_hashes: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut nob_nonempty_ticks = 0usize;
    let mut sob_nonempty_ticks = 0usize;
    let mut wob_nonempty_ticks = 0usize;
    let mut saw_sobject_id2 = false;
    let mut max_nobjects = 0usize;
    let mut bobjects_always_empty = true;
    let mut worm0_health_always_100 = true;
    let mut level_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        let c = hash_components(state);
        rng_by_tick.push(c.rng);
        master_by_tick.push(hash_game_state(state));
        let w0 = &state.worms[0];
        slot0_loading_by_tick.push(w0.weapons[0].loading_left);
        ammo0_by_tick.push(w0.weapons[0].ammo);
        slot0_ammo_seen.insert(w0.weapons[0].ammo);
        current_weapon_seen.insert(w0.current_weapon);
        if c.nobjects != EMPTY_POOL {
            nob_nonempty_ticks += 1;
            nob_hashes.insert(c.nobjects);
        }
        if c.sobjects != EMPTY_POOL {
            sob_nonempty_ticks += 1;
        }
        if c.wobjects != EMPTY_POOL {
            wob_nonempty_ticks += 1;
        }
        if state.sobjects.iter().any(|s| s.id == 2) {
            saw_sobject_id2 = true;
        }
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if !state.bobjects.is_empty() {
            bobjects_always_empty = false;
        }
        if state.worms[0].health != 100 {
            worm0_health_always_100 = false;
        }
        level_seen.insert(c.level);
    };
    record(&state);

    // --- Drive each subsequent tick under SCRIPTED input. THE OFF-BY-ONE: golden
    // line `k` (k>=1) is the result of applying input[k-1] on the pass that advances
    // tick k-1 -> k. So produce line `k` by calling process_frame with input keyed
    // `k-1`.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // ===== Coverage guards (witness from the DRIVEN state) ====================

    // --- RELOAD: with the dumper's `settings.loading_time = 0`, the reload is
    // INSTANT. `ComputedLoadingTime` clamps to 1 and the same-tick countdown drops
    // `loading_left` back to 0, so loading_left is 0 at EVERY tick boundary even
    // though a reload fires. The boundary-visible witness is the AMMO reset: slot 0
    // ammo runs {2, 1, 0, 15} — depleting to 0 (magazine emptied by shot 2) then
    // resetting to the handgun default (15) on the reload tick.
    assert!(
        slot0_loading_by_tick.iter().all(|&v| v == 0),
        "with loading_time=0 the reload is instant: loading_left must be 0 at every \
         tick boundary; saw {:?}",
        slot0_loading_by_tick.iter().collect::<std::collections::HashSet<_>>()
    );
    assert!(
        slot0_ammo_seen.contains(&0),
        "slot 0 ammo must reach 0 (magazine emptied by shot 2); saw {:?}",
        slot0_ammo_seen
    );
    assert!(
        slot0_ammo_seen.contains(&handgun_default_ammo),
        "reload must reset slot 0 ammo to the handgun default {handgun_default_ammo}; saw {:?}",
        slot0_ammo_seen
    );
    // Locate the reload tick (slot 0 ammo jumps from 0 back up to the default) and
    // assert it is a genuine boundary transition that moves the master (the ammo
    // reset folds into worm0 -> the master), proving the reload branch executed.
    let reload_tick = (1..ammo0_by_tick.len())
        .find(|&k| ammo0_by_tick[k - 1] == 0 && ammo0_by_tick[k] == handgun_default_ammo)
        .expect("a tick where slot 0 ammo resets 0 -> default (the reload)");
    assert_ne!(
        master_by_tick[reload_tick], master_by_tick[reload_tick - 1],
        "the reload must move the master (ammo reset folds into worm0) at tick {reload_tick}"
    );

    // --- LOAD_CHANGE: current_weapon takes >= 2 distinct values, proving
    // ProcessWeaponChange ran and cycled the selected weapon while Change was held
    // (the `Available() || load_change` gate; load_change = true is threaded in).
    assert!(
        current_weapon_seen.len() >= 2,
        "load_change must cycle current_weapon (>=2 distinct values); saw {:?}",
        current_weapon_seen
    );

    // --- SHELL DROP + dirt debris: nobjects go empty -> non-empty -> >= 2 distinct
    // component values (a shell spawns and is Process'd, plus dirt debris). The pool
    // starts empty at tick 0.
    assert_eq!(golden[0].pools[3], EMPTY_POOL, "nobjects start empty at tick 0");
    assert!(
        nob_nonempty_ticks >= 1,
        "nobjects must go non-empty (shell + dirt debris); saw {nob_nonempty_ticks}"
    );
    assert!(
        nob_hashes.len() >= 2,
        "nobjects must take >= 2 distinct non-empty values (spawned + Process'd); saw {}",
        nob_hashes.len()
    );

    // --- The explosion sobject (the 4c cluster reused via createOnExp).
    assert!(
        sob_nonempty_ticks >= 1,
        "sobjects must go non-empty (the explosion); saw {sob_nonempty_ticks}"
    );
    assert!(saw_sobject_id2, "the spawned sobject must be small_explosion (id == 2)");

    // --- FIRE: a wobject spawns (the bullet) AND the rng advances at the fire tick
    // (handgun fire draws spread/speed rand, unlike the 4c DART which drew zero).
    let fire_line = (1..golden.len())
        .find(|&k| golden[k].pools[4] != EMPTY_POOL && golden[k - 1].pools[4] == EMPTY_POOL)
        .expect("a wobject must spawn (the handgun is fired)");
    assert!(
        wob_nonempty_ticks >= 1,
        "wobjects pool must be non-empty for >=1 tick (the bullet); saw {wob_nonempty_ticks}"
    );
    assert_ne!(
        rng_by_tick[fire_line], rng_by_tick[fire_line - 1],
        "rng must advance at the fire tick {fire_line} (handgun fire draws rand)"
    );

    // --- DIG + explosion: the level component takes >= 2 distinct values across the
    // run (the explosion crater AND the repeated digs carve the dirt floor).
    assert!(
        level_seen.len() >= 2,
        "level must take >= 2 distinct values (explosion crater + digs); saw {:?}",
        level_seen
    );

    // --- O3 guard: shells + a single explosion's debris stay well under the 600 cap.
    assert!(
        max_nobjects < 600,
        "nobjects must stay under the 600 cap (O3 deferred); peaked at {max_nobjects}"
    );
    // --- O10 guard: every shot explodes downrange clear of both worms, so no worm
    // DoDamage/blow-away/blood -> bobjects stays empty and worm0 stays health 100.
    assert!(
        bobjects_always_empty,
        "bobjects must stay empty (no worm in detect_range -> no blood; O10)"
    );
    assert!(
        worm0_health_always_100,
        "worm0 health must stay 100 (no explosion/dig damage; O10)"
    );
    // worm1 is invisible/inert: its column is CONSTANT across the whole run.
    let worm1_constant = golden.iter().all(|g| g.worm1 == golden[0].worm1);
    assert!(
        worm1_constant,
        "worm1 column must be constant (invisible/inert; Worm::Process gated on visible)"
    );
}
