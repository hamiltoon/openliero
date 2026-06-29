//! Per-tick differential test for the Slice-5b DART -> small_explosion -> worm1
//! WOUND -> blood spray -> live `bobjects` blood pool against the C++ oracle — THE
//! MILESTONE of 5b. The golden (`golden/sim_slice5b.txt`, 91 lines for ticks 0..=90)
//! is produced by the real C++ `Game` running the *same scenario*
//! (`golden/sim_slice5b_scenario.txt`): seed 42, the LOADED `physics_fall_test.lev`,
//! worm0 visible + grounded with the **DART** in weapon slot 0, worm1 VISIBLE +
//! grounded + stationary, placed just LEFT of the dart's impact. worm0 raises the gun
//! and FIRES a single DART (input tick 38) that skims flat-LEFT a few px above the
//! dirt floor and explodes on the first dirt cell at x~116 (explode tick ~50), where
//! `BlowUpObject`'s `create_on_exp` spawns a **`small_explosion`** SObject. worm1's
//! centre (x=115) sits INSIDE the explosion's +/-8 detect box, so the per-worm damage
//! loop (T2: `DoDamage` + sobject arm) WOUNDS worm1 (health drops, but stays > 0 —
//! death is 5d) and sprays `kBloodAmount` **type-6 blood nobjects** [`rand(128)` +
//! `Create2`]. Those blood nobjects, on the `cycles % 10` drip cadence, push the
//! **`bobjects` blood pool LIVE for the first time** (T3) — the headline of 5b. worm0
//! (x=150, 34px away) is far outside the box and takes NO damage.
//!
//! Golden columns (hashes hex):
//!   `<tick> <state_hash> <rng> <level> <worm0> <worm1> <bob> <bon> <sob> <nob> <wob>`
//!
//! ## What a bit-exact match proves — the whole O10 damage+blood+bobjects chain
//!
//! A bit-exact match over 91 ticks proves the entire T2 (sobject worm-damage arm +
//! `DoDamage`) + T3 (blood-trail -> `BObject` -> bobjects driver) port end-to-end vs
//! the C++ oracle. Two RNG facts characterise 5b:
//!   * Like 4c's DART (distribution=0, recoil=0, splinter_amount=0, time_to_explo=0):
//!     **DART Fire = 0 rand**, and worm0 does NOT recoil — so `rng` is FLAT until the
//!     explode tick and worm0's column is FLAT after it lands. Every new `rand()` this
//!     run is inside the EXPLOSION + the worm blood + the bobject drip/land.
//!   * At the explode tick the cluster draws: explosion sound `rand(2)`; the per-worm
//!     `DoDamage(z)` (no rand) + blow-away (no rand) + `kBloodAmount` x [`rand(128)` +
//!     `Create2`(`rand(40)`+`rand(40000)`x2)] + the `rand(3)` gate; then the 9x9
//!     dirt-throw; then the crater `DrawDirtEffect` `rand(2)`. The `rng` keeps moving
//!     for several ticks while blood + bobjects drip and land.
//!
//! The component columns are asserted FIRST (rng -> level -> worm0 -> worm1 ->
//! bobjects -> bonuses -> sobjects -> nobjects -> wobjects) THEN the master
//! `state_hash`, so a divergence localises to a tick + subsystem before the master
//! flags it: `rng` => a wrong draw count/order in the explosion/blood cluster (the
//! `CreateBObject` colour `rand(NumBloodColours)` draw + the blood `Create2` draws
//! localise here); `level` => the carving `DrawDirtEffect` OR a bobject gravity/landing
//! desync; `worm1` => a wrong DoDamage `z`/health or blow-away `vel`; `bobjects`(pos)
//! => the blood-trail `CreateBObject` pos/gravity; `nobjects`(pos) => the blood/dirt
//! `Create2` velocity. **O11:** an `nobjects`/`bobjects`-column match proves position
//! ONLY (the component fold is `pos.x,pos.y` only) — a blood `vel`/`cur_frame`/`type`
//! or bobject `vel`/`color` desync shows only in the MASTER.
//!
//! THE CRITICAL 5b-SPECIFIC STEP: `SimState::new` defaults
//! `num_blood_colours`/`first_blood_colour`/`bobj_gravity` to 0 (they are TC
//! constants, not in the `new` arg list — mirrors 4d's `small_sprites`). They are read
//! by `CreateBObject` (colour `rand(NumBloodColours) + FirstBloodColour`) and
//! `BObject::Process` (gravity `vel.y += BObjGravity`). This harness assigns them from
//! the loaded TC (`tc.constants.{NumBloodColours,FirstBloodColour,BObjGravity}`) AFTER
//! `SimState::new` — the exact `LC(...)` values the C++ dumper's `common.c[...]` holds
//! (`src/game/bobject.cpp:12,31`). Left at 0 the difftest WOULD diverge at the first
//! bobject tick on the `rng` (the missing colour draw) and `level` (gravity/landing)
//! columns — that divergence means the consts were forgotten, not a sim bug.
//!
//! The scenario is the single source of truth (parsed via `oracle_tests::scenario`)
//! and the expected values are PARSED from the golden file, never hard-coded.
//!
//! ## BLOCKED (deeper than T5b) — the in-flight arm is ported, but the 5a worm-hit
//! ##                              BOX over-approximation OVER-FIRES during descent
//!
//! Slice 5b T5b **ported the wobject in-flight worm-hit arm** (`weapon.cpp:287-326`)
//! in `sim::weapon::wobject_process` — the per-worm gate `(hit_damage || blow_away ||
//! blood_on_hit || worm_collide) && check_for_spec_worm_hit`, then `DoDamage` + the
//! `kBloodAmount × [rand(128) + Create2]` blood fan + the `hit_damage>0 && health>0`
//! `rand(3)` hit-sound gate (the load-bearing difference from the sobject arm, which
//! draws `rand(3)` unconditionally) — and its unit tests passed in isolation. But the
//! port was **REVERTED** (see below) because it diverges **EARLIER, at tick 49 (the
//! `rng` column)** AND regresses 4a/4b/5a:
//!
//!   * The gate's in-range test reuses 5a's `check_for_spec_worm_hit`, a **16x16
//!     sprite-BOX over-approximation**, NOT C++'s per-pixel `CheckForSpecWormHit`
//!     (`worm.cpp:1162-1188`), which tests `materials[worm_sprite[...]].Worm()` —
//!     needing the worm sprite bank + the `Worm` material flag, NEITHER of which
//!     lives in the sim yet.
//!   * Diagnostic (DIAG instrumentation, since removed): at the tick producing
//!     golden line 49 the descending dart sits at pixel **(123,199)** and worm1 at
//!     **(115,196)** → box offset **col=15, row=8**, the extreme bottom-right corner
//!     of worm1's sprite box (8px right, 3px below centre). That corner pixel is
//!     **transparent** in the real worm sprite, so C++ `CheckForSpecWormHit` returns
//!     `false` (the golden's `rng` stays flat 0 through tick 50 and worm1's column is
//!     flat `fbae6f96`, first firing at tick 51). The box over-approximation returns
//!     `true` and **OVER-FIRES at tick 49** — spraying blood + drawing `rand` three
//!     ticks before the real hit, so `rng` goes nonzero at 49 vs the golden's 0.
//!   * WIDER BLAST RADIUS: the box also over-fires for the **firing worm near the
//!     muzzle**, regressing the already-green 4a/4b/5a goldens. The **fan** has
//!     `wormCollide = true` + `blowAway = 30` (gate opens with `hitDamage =
//!     bloodOnHit = 0`), and its projectile spawns `detect_distance + 5 ≈ 6px` from
//!     worm0 — inside worm0's box, over the transparent sprite halo. So the brief's
//!     premise ("the arm is inert for 1–5a") is false; the box check is unusable for
//!     the wobject arm in general, not just 5b's grazing dart.
//!
//! Per the task constraints (no geometry hack, no golden/scenario/C++ change, no
//! `SimState::new` change), the fix is OUT OF SCOPE here: it needs the **per-pixel
//! `CheckForSpecWormHit`** (the worm sprite bank + `Worm` material flag + worm
//! current_frame/direction — a larger sim addition that also touches `SimState`). The
//! arm port was therefore reverted to keep `cargo test --workspace` green; the impl
//! is documented in `.superpowers/sdd/step2-slice5b-task-5b-report.md`, ready to
//! re-apply once the per-pixel oracle lands. The assertions below stay STRICT (master
//! + all 9 components, every tick); drop the `#[ignore]` then.

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
/// `draw_dirt_effect`: the small_explosion's `dirt_effect` indexes this bank on the
/// crater carve tick, so an empty bank would panic.
fn load_large_sprites() -> assets::sprite::SpriteSet {
    let bytes = std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).expect("read large.tga");
    let tga = assets::sprite::Tga::load(&bytes).expect("large.tga parses");
    assets::sprite::SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank")
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
#[ignore = "BLOCKED (deeper than T5b): the wobject in-flight worm-hit arm \
            (weapon.cpp:287-326) was ported & unit-tested in T5b (DoDamage + the \
            rand(128) blood fan + the hit_damage>0 && health>0 rand(3) gate), but \
            REVERTED — it cannot land against the 5a `check_for_spec_worm_hit`, a \
            16x16 sprite-BOX over-approximation, vs C++'s per-pixel \
            `CheckForSpecWormHit` (needs the worm sprite bank + Worm material \
            flag, absent from the sim). With the arm live this milestone diverges \
            EARLIER, at tick 49 (rng column): at the tick producing golden line \
            49 the descending dart sits at pixel (123,199) and worm1 at (115,196) \
            — box offset col=15,row=8, the extreme bottom-right corner of worm1's \
            sprite box. That corner pixel is TRANSPARENT in the real worm sprite, \
            so C++ returns false (rng flat 0 through tick 50, first fires at 51); \
            the box returns true and OVER-FIRES. WORSE: the box also over-fires \
            for the FIRING worm near the muzzle, regressing 4a/4b/5a (the fan has \
            wormCollide=true + blowAway=30, so its gate opens 6px from worm0). \
            Closing this needs the per-pixel CheckForSpecWormHit (larger scope: \
            worm sprite bank — and a SimState change this task forbids). See \
            .superpowers/sdd/step2-slice5b-task-5b-report.md; the arm impl is \
            ready to re-apply once the per-pixel oracle lands."]
fn sim_slice5b_blood_bobjects_match_cpp_oracle() {
    // --- Parse the scenario (single source of truth, shared with the C++ dumper).
    let scenario_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5b_scenario.txt"
    ))
    .expect("read golden/sim_slice5b_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "scenario seed");
    assert_eq!(scenario.worms.len(), 2, "scenario has two worms");
    assert_eq!(scenario.ticks, 90, "scenario ticks");

    // --- Parse the golden vectors (ticks 0..=90, master + 9 components). -------
    let golden_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sim_slice5b.txt"
    ))
    .expect("read golden/sim_slice5b.txt");
    let golden = parse_golden(&golden_text);
    assert_eq!(golden.len(), (scenario.ticks + 1) as usize, "golden has tick 0..=ticks");

    // --- THE 5b HEADLINE: the dart explosion WOUNDS worm1 -> blood nobjects -> the
    // live `bobjects` blood pool. The golden must therefore carve (`level` moves),
    // spawn an sobject (the small_explosion), spawn nobjects (blood + dirt) AND leave
    // the bobjects empty-pool hash (the first live blood pool). Read straight from the
    // parsed golden so a regenerated golden that lost the blood arm fails loudly here
    // before the per-tick loop. ------------------------------------------------
    let golden_levels: std::collections::HashSet<u32> = golden.iter().map(|g| g.level).collect();
    assert!(
        golden_levels.len() >= 2,
        "5b golden must carve: level column takes >=2 distinct values; saw {:?}",
        golden_levels
    );
    assert!(
        golden.iter().any(|g| g.pools[2] != EMPTY_POOL),
        "5b golden must spawn an sobject (sob column leaves the empty-pool hash)"
    );
    assert!(
        golden.iter().any(|g| g.pools[3] != EMPTY_POOL),
        "5b golden must spawn blood/dirt nobjects (nob column leaves the empty-pool hash)"
    );
    assert!(
        golden.iter().any(|g| g.pools[0] != EMPTY_POOL),
        "5b golden must drip the bobjects blood pool (bob column leaves the empty-pool hash)"
    );

    // --- Load the SAME level the C++ dumper loaded (the fall fixture). --------
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{}", scenario.level))
        .unwrap_or_else(|e| panic!("read {}: {e}", scenario.level));
    let level = assets::level::load(&lev_bytes).expect("level loads");

    // --- Load the real TC weapon table + physics/control constants + object tables.
    // `Objects::load` parses weapons, nobject_types AND sobject_types; `tc.cfg`
    // carries the materials, textures, the large-sprite bank name, the
    // physics/control consts AND the `[constants]` scalars (incl. the three blood
    // consts set below). ------------------------------------------------------
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");

    // INVARIANT (load-bearing for the Fire path): `weapon.id == array index`.
    for (i, w) in objects.weapons.iter().enumerate() {
        assert_eq!(
            w.id, i as i32,
            "weapon id must equal its index (weapon[{i}] = {:?}, id {})",
            w.name, w.id
        );
    }
    // INVARIANT (load-bearing for `create_on_exp` + the dirt-throw + the blood arm):
    // the object tables are indexed by id. If id != index those lookups read the
    // wrong object, so STOP here rather than paper over it.
    for (i, s) in objects.sobject_types.iter().enumerate() {
        assert_eq!(s.id, i as i32, "sobject_type id must equal its index (got id {})", s.id);
    }
    for (i, n) in objects.nobject_types.iter().enumerate() {
        assert_eq!(n.id, i as i32, "nobject_type id must equal its index (got id {})", n.id);
    }

    // weap_order: indices sorted by weapon name; id == index. Mirrors
    // Common::Precompute (common.cpp:492-499), exactly as slices 1-5a.
    let mut weap_order: Vec<usize> = (0..objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| objects.weapons[a].name.cmp(&objects.weapons[b].name));

    // WormSettings::weapons default = all 1 -> every slot selects order index 0.
    let settings_weapons = [1u32; NUM_WEAPONS];
    let mut resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);

    // --- Override weapon slot 0 with the DART, mirroring the C++ dumper's
    // `ResolveWeapon("DART")`. The name is the *scenario's* `weapon 0` directive and
    // must match `common->weapons[i].name` exactly (UPPERCASE "DART"). ----------
    let dart_name = scenario.weapon(0).expect("scenario `weapon 0 <name>` directive present");
    let dart_idx = objects
        .weapons
        .iter()
        .position(|w| w.name == dart_name)
        .unwrap_or_else(|| panic!("weapon {dart_name:?} present in TC weapon table"));
    resolved[0] = WeaponInit {
        ty: Some(dart_idx as WeaponId),
        ammo: objects.weapons[dart_idx].ammo,
    };

    // --- Build the two worm inits from the scenario. BOTH worms get the DART in slot
    // 0 (the scenario `weapon 0` overrides both); worm1 is the visible victim that
    // stands in the blast — its 5-slot weapon state still folds into worm1's hash, so
    // the override must apply to it too for the master to match the dumper. -------
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

    // --- Build tick-0 state. The trailing `0, true, 100` are
    // settings_loading_time / load_change / blood — matching the dumper
    // (`settings->loading_time = 0`; `load_change`/`blood` left at the Settings
    // defaults true/100; the dart fires once and never reloads in 90 ticks, so
    // loading_time is inert, but 0 matches the dumper exactly). `blood = 100` is the
    // per-worm `kBloodAmount = blood * power_sum / 100` multiplier the damage arm
    // reads (sobject.cpp:96). -------------------------------------------------
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

    // --- THE CRITICAL 5b STEP: set the three blood TC consts post-`new` (defaulted
    // to 0 by `SimState::new`, like 4d's `small_sprites`). They are the exact
    // `LC(NumBloodColours)` / `LC(FirstBloodColour)` / `LC(BObjGravity)` values the
    // C++ dumper's `common.c[...]` holds, read by `CreateBObject` (blood colour draw)
    // and `BObject::Process` (gravity). LEFT AT 0 the run diverges at the first
    // bobject tick on `rng` (missing colour draw) and `level` (wrong gravity/landing).
    state.num_blood_colours = tc.constants.NumBloodColours;
    state.first_blood_colour = tc.constants.FirstBloodColour;
    state.bobj_gravity = tc.constants.BObjGravity;

    let check = |tick: u32, name: &str, got: u32, want: u32| {
        assert_eq!(
            got, want,
            "tick {tick}: {name}: got {got:08x} expected {want:08x}"
        );
    };

    // Assert COMPONENTS FIRST (rng, level, worm0, worm1, the 5 pools incl. bobjects)
    // THEN the master, so a divergence localises to a tick + subsystem before the
    // master flags it. O11: the `nobjects`/`bobjects` columns prove position only; a
    // blood vel/color/cur_frame desync shows only in the master.
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
        // The MASTER, last: it folds the explosion+blood cluster's RNG, the live
        // sobject, the live blood/dirt nobjects (pos+vel+cur_frame+type — wider than
        // the component fold, O11), the live bobjects (pos+vel+color, O11) AND the
        // carved level + the wounded worm1.
        check(g.tick, "MASTER state_hash", hash_game_state(state), g.master);
    };

    // --- Tick 0: assert against the freshly-built state, NO process_frame. ----
    assert_eq!(golden[0].tick, 0, "first golden row is tick 0");
    assert_tick(&state, &golden[0]);

    // --- Coverage guards read from the DRIVEN SimState (the T5-only wound proofs),
    // never re-parsed from the golden. -----------------------------------------
    let mut worm1_health_by_tick: Vec<i32> = Vec::with_capacity(golden.len());
    let mut bob_count_by_tick: Vec<usize> = Vec::with_capacity(golden.len());
    let mut max_bobjects = 0usize;
    let mut max_nobjects = 0usize;
    let mut worm0_health_always_100 = true;
    let mut worm1_health_always_positive = true;
    let mut bonuses_always_empty = true;
    let mut saw_small_explosion = false; // sobject id 2 = the dart's create_on_exp
    let mut saw_type6_blood_nobject = false;
    let mut level_seen = std::collections::HashSet::new();
    let mut record = |state: &SimState| {
        worm1_health_by_tick.push(state.worms[1].health);
        bob_count_by_tick.push(state.bobjects.len());
        max_bobjects = max_bobjects.max(state.bobjects.len());
        max_nobjects = max_nobjects.max(state.nobjects.len());
        if state.worms[0].health != 100 {
            worm0_health_always_100 = false;
        }
        if state.worms[1].health <= 0 {
            worm1_health_always_positive = false;
        }
        if !state.bonuses.is_empty() {
            bonuses_always_empty = false;
        }
        if state.sobjects.iter().any(|s| s.id == 2) {
            saw_small_explosion = true;
        }
        if state.nobjects.iter().any(|n| n.ty == Some(6)) {
            saw_type6_blood_nobject = true;
        }
        level_seen.insert(hash_components(state).level);
    };
    record(&state);

    // --- Drive each subsequent tick under SCRIPTED input. THE OFF-BY-ONE: golden
    // line `k` (k>=1) is the result of applying input[k-1] on the pass advancing tick
    // k-1 -> k. So produce line `k` by calling process_frame with input keyed `k-1`.
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        assert_tick(&state, &golden[k as usize]);
        record(&state);
    }

    // --- Locate the wound tick from the DRIVEN state: the first tick worm1's health
    // drops below 100 (the explosion damage landing). Derived, not hard-coded. ----
    let wound_tick = (1..worm1_health_by_tick.len())
        .find(|&k| worm1_health_by_tick[k] < 100)
        .expect("worm1 must be wounded (health drops below 100 at the explode tick)");

    // --- THE 5b WOUND GUARDS (read from the DRIVEN SimState — genuine witnesses). --
    // worm1 wounded-not-killed: health > 0 EVERY tick (O20 survives) AND < 100 from
    // the wound tick onward (the damage actually landed, by <= 5).
    assert!(
        worm1_health_always_positive,
        "worm1 health must stay > 0 every tick (wounded, NOT killed — death is 5d)"
    );
    let worm1_min = *worm1_health_by_tick.iter().min().expect("ticks recorded");
    assert!(
        (1..100).contains(&worm1_min),
        "worm1 health min must be in (0,100): wounded but alive; saw {worm1_min}"
    );
    for (k, &h) in worm1_health_by_tick.iter().enumerate().skip(wound_tick) {
        assert!(
            (1..100).contains(&h),
            "worm1 health must stay in (0,100) from the wound tick onward; tick {k} = {h}"
        );
    }
    // No self-damage: worm0 health == 100 EVERY tick (it fired but stayed clear of the
    // +/-detect_range box and the dart's flight corridor).
    assert!(
        worm0_health_always_100,
        "worm0 health must stay 100 every tick (out of range -> no self-damage)"
    );

    // The blood pool goes LIVE — the headline of 5b. bobjects count > 0 at/after the
    // first drip tick, and stays under the O3 caps.
    assert!(
        max_bobjects > 0,
        "the bobjects blood pool must go live (count > 0 at the drip tick); peaked at {max_bobjects}"
    );
    assert!(
        max_bobjects < 700,
        "bobjects must stay under the 700 cap (O3); peaked at {max_bobjects}"
    );
    assert!(
        max_nobjects < 600,
        "nobjects must stay under the 600 cap (O3); peaked at {max_nobjects}"
    );

    // The dart's create_on_exp is the small_explosion sobject (id 2), and the per-worm
    // blood arm spawns type-6 blood nobjects.
    assert!(
        saw_small_explosion,
        "the spawned sobject must be small_explosion (id == 2, the dart's create_on_exp)"
    );
    assert!(
        saw_type6_blood_nobject,
        "a type-6 (blood) nobject must spawn from the per-worm damage loop"
    );

    // Terrain genuinely carved (the crater); no bonuses this slice.
    assert!(
        level_seen.len() >= 2,
        "level component must take >=2 distinct values (terrain carved); saw {:?}",
        level_seen
    );
    assert!(
        bonuses_always_empty,
        "bonuses must stay empty (5b spawns no bonuses)"
    );
}
