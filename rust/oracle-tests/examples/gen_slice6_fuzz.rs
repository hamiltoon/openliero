//! Scenario **generator + seed scanner** for Slice-6 T7 — the >1000-tick fuzz
//! variants that mirror `src/tests/test_determinism.cpp`. This is the committed,
//! reproducible tool that PRE-EXPANDS the per-tick random inputs into the scenario
//! files (the CONTROLLER DECISION for T7: no dumper change — the scenario stays the
//! single source of truth read by BOTH the C++ dumper and the Rust difftest).
//!
//! ## The input model (mirror of `test_determinism.cpp:95-101`)
//!
//! `test_determinism.cpp`'s fuzz draws, per frame, `input_rng() & 0x7f` for worm 0
//! then worm 1, where `input_rng` is a `Rand` (a `std::mt19937` seeded with the
//! *input seed*). We reproduce that EXACTLY with a faithful `std::mt19937` mirror
//! (canonical MT19937-32 seeding + tempering) and emit one `input <t> <w0> <w1>`
//! line per tick. Because the values are pre-expanded into the committed scenario,
//! the RNG mirror is NOT load-bearing for bit-exactness — the scenario text is the
//! truth both sides read. The mirror only makes each variant *the same random input
//! stream `test_determinism.cpp` would have produced* for that seed, so the fuzz is
//! honest (never hand-scripted). The two draws per tick advance the mt19937 in the
//! worm-0-then-worm-1 order the C++ `ApplyRandomInputs` uses.
//!
//! ## The tick offset (the seam, verified against the dumper + slice-3 convention)
//!
//! `sim_physics_dump.cpp` applies the `input` line keyed on `t` on the Process pass
//! advancing `t -> t+1` (its loop `for t in 0..ticks`). The Rust difftests key the
//! same input as `scenario.input(k - 1, ..)` at tick `k` (k=1..=ticks). This tool
//! drives the Rust sim with the IDENTICAL convention (`inputs[k-1]` at tick `k`), so
//! the ledger it reports is exactly what the committed scenario produces on both
//! sides. Emitting inputs for `t = 0 .. ticks-1` (the ticks the dumper reads).
//!
//! ## Worms seeded DEAD (mirror of the fixture)
//!
//! Both worms start `visible 0`, `pos 0 0`, high `lives`; `SimState::new` /
//! `ResetWorms` set `killed_timer = 150` and `ready = true`, so each worm respawns
//! IN-SIM via the countdown + `DoRespawning` spawn search (the cossin[128]-safe
//! respawn: `DoRespawning` sets `aiming_angle ∈ {32,96}`, never 0). `max_bonuses 4`
//! opens the per-tick bonus-drop roll; a REAL arena (`Levels/modern_test.lev`) gives
//! dirt for ropes/digging and room to move.
//!
//! ## Usage
//!
//!   # Scan a game-seed range (inputs from <input_seed>) and print a ledger line each:
//!   cargo run -p oracle-tests --example gen_slice6_fuzz -- \
//!       scan <level> <game_seed_lo> <game_seed_hi> <input_seed> <ticks>
//!
//!   # Write a committed scenario for a chosen (game_seed, input_seed):
//!   cargo run -p oracle-tests --example gen_slice6_fuzz -- \
//!       gen <variant> <game_seed> <input_seed> <ticks> <level> <out_path>
//!
//!   # Re-drive a committed scenario and print its full event ledger (verification):
//!   cargo run -p oracle-tests --example gen_slice6_fuzz -- ledger <scenario_path>
//!
//! Purely a dev tool: it drives the same `process_frame` the goldens use and never
//! touches sim logic or a generated golden. NOT a test; not run in CI.

use std::fmt::Write as _;

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
const BLOOD_CAP: usize = 700;

// ---------------------------------------------------------------------------
// std::mt19937 mirror (canonical MT19937-32). Matches C++ `Rand`'s engine so
// `next() & 0x7f` reproduces `input_rng() & 0x7f` from test_determinism.cpp.
// ---------------------------------------------------------------------------

struct Mt19937 {
    mt: [u32; 624],
    idx: usize,
}

impl Mt19937 {
    fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Mt19937 { mt, idx: 624 }
    }

    fn generate(&mut self) {
        for i in 0..624 {
            let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7fff_ffff);
            let mut next = self.mt[(i + 397) % 624] ^ (y >> 1);
            if y & 1 != 0 {
                next ^= 0x9908_b0df;
            }
            self.mt[i] = next;
        }
        self.idx = 0;
    }

    fn next_u32(&mut self) -> u32 {
        if self.idx >= 624 {
            self.generate();
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }
}

/// The pre-expanded per-tick inputs: `inputs[t] = [worm0_7bit, worm1_7bit]`, drawn
/// worm-0-then-worm-1 in mt19937 order, each `& 0x7f`. `ticks` entries (t=0..ticks-1).
fn expand_inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut rng = Mt19937::new(input_seed);
    (0..ticks)
        .map(|_| {
            let w0 = rng.next_u32() & 0x7f;
            let w1 = rng.next_u32() & 0x7f;
            [w0, w1]
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sim build — full const wiring (death/respawn + bonus/pickup + blood + rope).
// Merges the slice-6 gametag harness (worm-spawn + blood consts) with the 5'b
// pickup harness (bonus + pickup consts). Rope consts arrive via ControlConsts.
// ---------------------------------------------------------------------------

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

struct Loaded {
    tc: TcConfig,
    objects: Objects,
    level: assets::level::LevelData,
}

fn load(level_path: &str) -> Loaded {
    let lev_bytes = std::fs::read(format!("{TC_ROOT}/{level_path}"))
        .unwrap_or_else(|e| panic!("read {level_path}: {e}"));
    let level = assets::level::load(&lev_bytes).expect("level loads");
    let tc_bytes = std::fs::read(format!("{TC_ROOT}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .expect("object configs load");
    Loaded { tc, objects, level }
}

/// The two DEAD worms the fuzz seeds (mirror of the fixture): `visible 0`, pos 0 0,
/// high lives, default full-loadout weapons (every slot == weap_order[0]).
fn dead_worms(l: &Loaded, health: i32, lives: i32) -> Vec<WormInit> {
    let mut weap_order: Vec<usize> = (0..l.objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| l.objects.weapons[a].name.cmp(&l.objects.weapons[b].name));
    let resolved: [WeaponInit; NUM_WEAPONS] =
        WormInit::resolve_weapons(&l.objects, &weap_order, &[1u32; NUM_WEAPONS]);
    vec![
        WormInit {
            index: 0,
            health,
            lives,
            stats_x: 0,
            weapons: resolved,
            start_pos: Vec2::zero(),
            visible: false,
        },
        WormInit {
            index: 1,
            health,
            lives,
            stats_x: 218,
            weapons: resolved,
            start_pos: Vec2::zero(),
            visible: false,
        },
    ]
}

/// Worm inits straight from a parsed scenario's `worm` lines (default full loadout;
/// the fuzz scenarios carry no `weapon` directive). Mirrors the difftest builders.
fn worms_from_scenario(l: &Loaded, scenario: &Scenario) -> Vec<WormInit> {
    let mut weap_order: Vec<usize> = (0..l.objects.weapons.len()).collect();
    weap_order.sort_by(|&a, &b| l.objects.weapons[a].name.cmp(&l.objects.weapons[b].name));
    let mut resolved: [WeaponInit; NUM_WEAPONS] =
        WormInit::resolve_weapons(&l.objects, &weap_order, &[1u32; NUM_WEAPONS]);
    // Apply any `weapon <slot> <name> [ammo]` overrides (the fuzz scenarios carry
    // none, but keeping this faithful lets `dump`/`ledger` validate ANY scenario).
    for slot in 0..NUM_WEAPONS {
        if let Some(name) = scenario.weapon(slot) {
            let idx = l
                .objects
                .weapons
                .iter()
                .position(|w| w.name == name)
                .unwrap_or_else(|| panic!("weapon {name:?} in TC"));
            let ammo = scenario.weapon_ammo(slot).unwrap_or(l.objects.weapons[idx].ammo);
            resolved[slot] = WeaponInit { ty: Some(idx as WeaponId), ammo };
        }
    }
    scenario
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
        .collect()
}

fn build_state(l: &Loaded, worms_init: &[WormInit], game_seed: u32, max_bonuses: i32) -> SimState {
    let tc = &l.tc;
    let objects = &l.objects;
    let mut state = SimState::new(
        &l.level,
        worms_init,
        game_seed,
        &tc.materials,
        objects.weapons.clone(),
        PhysicsConsts::from_tc(tc),
        ControlConsts::from_tc(tc),
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
    state.settings_max_bonuses = max_bonuses;
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
    state
}

// ---------------------------------------------------------------------------
// Event ledger — every witness read from the DRIVEN state (never a golden).
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Ledger {
    death_ticks: Vec<(u32, usize)>,   // (tick, worm)
    respawn_ticks: Vec<(u32, usize)>, // (tick, worm)  visible false->true
    // A bonus vanished with a worm on its 11x11 AABB (health / reload / booby — all
    // three pickup branches, distinguished from timer expiry by worm proximity).
    pickup_ticks: Vec<(u32, usize)>,
    health_pickup_ticks: Vec<(u32, usize)>, // subset: health strictly rose
    weapon_pickup_ticks: Vec<(u32, usize)>, // subset: a slot's weapon id changed
    inflight_hit_ticks: Vec<(u32, usize)>,
    rope_attach_ticks: Vec<(u32, usize)>, // ninjarope anchored: attached false->true
    bonus_drops: u32,
    peak_bobjects: usize,
    peak_bobjects_tick: u32,
}

impl Ledger {
    fn ok(&self) -> bool {
        !self.death_ticks.is_empty()
            && !self.respawn_ticks.is_empty()
            && !self.pickup_ticks.is_empty()
            && !self.inflight_hit_ticks.is_empty()
            && !self.rope_attach_ticks.is_empty()
            && self.peak_bobjects >= 1
    }
}

struct Snap {
    visible: [bool; 2],
    health: [i32; 2],
    weapon_ty: [[Option<WeaponId>; NUM_WEAPONS]; 2],
    ipos: [(i32, i32); 2],
    rope_attached: [bool; 2],
    wob: Vec<(i32, i32)>,
    bonuses: Vec<(i32, i32)>,
    n_bonuses: usize,
}

fn snap(s: &SimState) -> Snap {
    let w = |i: usize| {
        let ww = &s.worms[i];
        (
            ww.visible,
            ww.health,
            ww.weapons.map(|x| x.ty),
            (ftoi(ww.pos.x), ftoi(ww.pos.y)),
            ww.ninjarope.attached,
        )
    };
    let (v0, h0, t0, p0, r0) = w(0);
    let (v1, h1, t1, p1, r1) = w(1);
    Snap {
        visible: [v0, v1],
        health: [h0, h1],
        weapon_ty: [t0, t1],
        ipos: [p0, p1],
        rope_attached: [r0, r1],
        wob: s.wobjects.iter().map(|o| (ftoi(o.pos.x), ftoi(o.pos.y))).collect(),
        bonuses: s.bonuses.iter().map(|b| (ftoi(b.x), ftoi(b.y))).collect(),
        n_bonuses: s.bonuses.len(),
    }
}

/// The strict 11x11 pickup AABB gate (`worm.cpp:289-290`): `|ipos - b| < 5` on both
/// axes. Used as the witness that a vanished bonus was COLLECTED (not expired).
fn on_bonus(wx: i32, wy: i32, bx: i32, by: i32) -> bool {
    (wx - bx).abs() < 5 && (wy - by).abs() < 5
}

/// Drive `ticks` frames with the given pre-expanded inputs, recording the ledger.
fn drive(l: &Loaded, worms: &[WormInit], game_seed: u32, inputs: &[[u32; 2]], max_bonuses: i32) -> Ledger {
    let mut state = build_state(l, worms, game_seed, max_bonuses);
    let mut led = Ledger::default();
    let mut prev = snap(&state);
    for k in 1..=inputs.len() as u32 {
        let inp = inputs[(k - 1) as usize];
        let ctl = [ControlState::unpack(inp[0]), ControlState::unpack(inp[1])];
        state.process_frame(&ctl);
        let cur = snap(&state);

        for wi in 0..2 {
            // Death: visible true -> false.
            if prev.visible[wi] && !cur.visible[wi] {
                led.death_ticks.push((k, wi));
            }
            // Respawn: visible false -> true (incl. the initial seeded-dead spawn).
            if !prev.visible[wi] && cur.visible[wi] {
                led.respawn_ticks.push((k, wi));
            }
            // Pickup: a bonus that was on this worm's AABB last tick is gone now, and
            // the worm was on it (the collision the pickup block acts on). Catches all
            // three branches (health / reload / booby) and excludes timer expiry (no
            // worm was on the vanished bonus). Prefer the pre-tick worm ipos since the
            // pickup reads `ipos` at the top of the worm's Process.
            if prev.visible[wi] && cur.visible[wi] && cur.n_bonuses < prev.n_bonuses {
                let (wx, wy) = prev.ipos[wi];
                let vanished_on_worm = prev
                    .bonuses
                    .iter()
                    .filter(|&&(bx, by)| on_bonus(wx, wy, bx, by))
                    .any(|&pb| !cur.bonuses.iter().any(|&cb| cb == pb));
                if vanished_on_worm {
                    led.pickup_ticks.push((k, wi));
                    if cur.health[wi] > prev.health[wi] {
                        led.health_pickup_ticks.push((k, wi));
                    }
                    if cur.weapon_ty[wi] != prev.weapon_ty[wi] {
                        led.weapon_pickup_ticks.push((k, wi));
                    }
                }
            }
            // Ninjarope attach: the rope anchored this tick (attached false->true).
            if !prev.rope_attached[wi] && cur.rope_attached[wi] {
                led.rope_attach_ticks.push((k, wi));
            }
            // In-flight hit: health dropped AND a wobject from the previous tick was
            // within 8px of the worm (the per-pixel WObject::Process hit; distinguishes
            // an in-flight projectile hit from a distant blast).
            if prev.visible[wi] && cur.visible[wi] && cur.health[wi] < prev.health[wi] {
                let (wx, wy) = prev.ipos[wi];
                if prev
                    .wob
                    .iter()
                    .any(|&(ox, oy)| (ox - wx).abs() <= 8 && (oy - wy).abs() <= 8)
                {
                    led.inflight_hit_ticks.push((k, wi));
                }
            }
        }
        // Count bonus drops (net increases in the pool population).
        if cur.n_bonuses > prev.n_bonuses {
            led.bonus_drops += (cur.n_bonuses - prev.n_bonuses) as u32;
        }
        let n = state.bobjects.len();
        if n > led.peak_bobjects {
            led.peak_bobjects = n;
            led.peak_bobjects_tick = k;
        }
        prev = cur;
    }
    led
}

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

fn first(v: &[(u32, usize)]) -> String {
    v.first().map(|(t, w)| format!("t{t}/w{w}")).unwrap_or_else(|| "—".into())
}

fn scan(level: &str, lo: u32, hi: u32, input_seed: u32, ticks: u32) {
    let l = load(level);
    let inputs = expand_inputs(input_seed, ticks);
    println!(
        "# level={level} input_seed={input_seed} ticks={ticks}\n\
         # game_seed | deaths resp drops pick hits rope peakBob | first(death/resp/pick/hit/rope) OK?"
    );
    let worms = dead_worms(&l, 100, 50);
    for gs in lo..=hi {
        let led = drive(&l, &worms, gs, &inputs, 4);
        println!(
            "{gs:>6} | {:>3} {:>3} {:>4} {:>3} {:>3} {:>3} {:>4}@t{:<4} | {} {} {} {} {} {}",
            led.death_ticks.len(),
            led.respawn_ticks.len(),
            led.bonus_drops,
            led.pickup_ticks.len(),
            led.inflight_hit_ticks.len(),
            led.rope_attach_ticks.len(),
            led.peak_bobjects,
            led.peak_bobjects_tick,
            first(&led.death_ticks),
            first(&led.respawn_ticks),
            first(&led.pickup_ticks),
            first(&led.inflight_hit_ticks),
            first(&led.rope_attach_ticks),
            if led.ok() { "OK" } else { "--" },
        );
    }
}

fn print_ledger(led: &Ledger) {
    let all = |v: &[(u32, usize)]| {
        v.iter().map(|(t, w)| format!("t{t}/w{w}")).collect::<Vec<_>>().join(" ")
    };
    println!("deaths        ({:>2}): {}", led.death_ticks.len(), all(&led.death_ticks));
    println!("respawns      ({:>2}): {}", led.respawn_ticks.len(), all(&led.respawn_ticks));
    println!("bonus drops       : {}", led.bonus_drops);
    println!("bonus pickups ({:>2}): {}", led.pickup_ticks.len(), all(&led.pickup_ticks));
    println!(
        "  of which health ({:>2}): {}",
        led.health_pickup_ticks.len(),
        all(&led.health_pickup_ticks)
    );
    println!(
        "  of which weapon ({:>2}): {}",
        led.weapon_pickup_ticks.len(),
        all(&led.weapon_pickup_ticks)
    );
    println!(
        "in-flight hits({:>2}): {}",
        led.inflight_hit_ticks.len(),
        all(&led.inflight_hit_ticks)
    );
    println!(
        "rope attaches ({:>2}): {}",
        led.rope_attach_ticks.len(),
        all(&led.rope_attach_ticks)
    );
    println!(
        "peak bobjects     : {} / {BLOOD_CAP} at tick {}",
        led.peak_bobjects, led.peak_bobjects_tick
    );
    println!("ALL EVENTS PRESENT: {}", if led.ok() { "YES" } else { "NO" });
}

/// Re-drive a committed scenario (parsed from disk) and print its full ledger. This
/// is the verification path: it proves the committed input lines produce the events.
fn ledger(scenario_path: &str) {
    let text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&text).expect("scenario parses");
    let l = load(&scenario.level);
    // Re-materialise the pre-expanded inputs straight from the committed scenario
    // (NOT the mt19937 — this checks the FILE, the source of truth both sides read).
    let inputs: Vec<[u32; 2]> = (0..scenario.ticks)
        .map(|t| [scenario.input(t, 0), scenario.input(t, 1)])
        .collect();
    println!("# {scenario_path}");
    println!(
        "# seed={} level={} ticks={} max_bonuses={}",
        scenario.seed, scenario.level, scenario.ticks, scenario.max_bonuses
    );
    let worms = worms_from_scenario(&l, &scenario);
    let led = drive(&l, &worms, scenario.seed, &inputs, scenario.max_bonuses);
    print_ledger(&led);
}

/// Emit the C++ dumper's exact 11-column golden format for a committed scenario, so
/// `diff` against the C++ output proves the fuzz is bit-exact BEFORE the goldens are
/// trusted (the T8 seam check; the dumper reads the same scenario file).
fn dump(scenario_path: &str) {
    let text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&text).expect("scenario parses");
    let l = load(&scenario.level);
    let worms = worms_from_scenario(&l, &scenario);
    let mut state = build_state(&l, &worms, scenario.seed, scenario.max_bonuses);
    state.game_mode = scenario.game_mode as u32;
    let emit = |tick: u32, s: &SimState| {
        let c = hash_components(s);
        println!(
            "{tick} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}",
            hash_game_state(s),
            c.rng,
            c.level,
            c.worms[0],
            c.worms[1],
            c.bobjects,
            c.bonuses,
            c.sobjects,
            c.nobjects,
            c.wobjects
        );
    };
    emit(0, &state);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        emit(k, &state);
    }
}

/// Write a committed scenario file for a chosen (game_seed, input_seed).
fn gen(variant: u32, game_seed: u32, input_seed: u32, ticks: u32, level: &str, out_path: &str) {
    let l = load(level);
    let inputs = expand_inputs(input_seed, ticks);
    // Confirm the events land BEFORE writing (the tool refuses to emit a dud).
    // variant 0 == force (throwaway probes; skips the all-events guard).
    let worms = dead_worms(&l, 100, 50);
    let led = drive(&l, &worms, game_seed, &inputs, 4);
    if !led.ok() && variant != 0 {
        eprintln!("REFUSING to write variant {variant}: not all events present:");
        print_ledger(&led);
        std::process::exit(3);
    }

    let mut s = String::new();
    let hp = led.health_pickup_ticks.first();
    let wp = led.weapon_pickup_ticks.first();
    let pick = match (hp, wp) {
        (Some((t, w)), _) => format!("HEALTH pickup t{t}/w{w}"),
        (None, Some((t, w))) => format!("WEAPON-reload pickup t{t}/w{w}"),
        _ => "—".into(),
    };
    write!(
        s,
        "# Step 2 Slice 6 T7 — >1000-tick FUZZ, VARIANT {variant}. Mirrors\n\
         # src/tests/test_determinism.cpp: 2 worms seeded DEAD (visible 0, pos 0 0,\n\
         # killed_timer 150 via ResetWorms, high lives) that RESPAWN in-sim, driven by\n\
         # per-tick random inputs `input_rng() & 0x7f` (worm0 then worm1) from a faithful\n\
         # std::mt19937 mirror seeded with the INPUT seed. The inputs are PRE-EXPANDED into\n\
         # the `input` lines below (CONTROLLER DECISION: no dumper change — the scenario is\n\
         # the single source of truth read by BOTH the C++ oracle_dump_sim_physics dumper\n\
         # and the Rust difftest). Reproduce with:\n\
         #   cargo run -p oracle-tests --example gen_slice6_fuzz -- \\\n\
         #       gen {variant} {game_seed} {input_seed} {ticks} {level} <out>\n\
         #\n\
         # GAME seed (level/sim RNG): {game_seed}    INPUT seed (mt19937): {input_seed}\n\
         # Control bits (7-bit): Up=1 Down=2 Left=4 Right=8 Fire=16 Change=32 Jump=64.\n\
         #\n\
         # EVENT LEDGER (from the Rust scan of the DRIVEN state; see the T7 report):\n\
         #   deaths={}  respawns={}  bonus pickups={}  in-flight hits={}  rope attaches={}\n\
         #   peak bobjects={}/{}   first death t{}  first respawn t{}  first hit t{}\n\
         #   first rope attach t{}  {}\n\
         # Each worm respawns via the killed_timer countdown + DoRespawning (aiming_angle\n\
         # ∈ {{32,96}}, so cossin[128] stays unreachable — NO hand-set aiming_angle 0).\n\
         seed {game_seed}\n\
         level {level}\n\
         ticks {ticks}\n\
         max_bonuses 4\n\
         # worm <index> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>\n\
         worm 0 0 0 100 50 0   0\n\
         worm 1 0 0 100 50 218 0\n\
         #\n\
         # input <tick> <worm0_7bit> <worm1_7bit> — PRE-EXPANDED mt19937 stream, one per\n\
         # tick t=0..{}, applied on the Process pass advancing t -> t+1 (dumper seam;\n\
         # Rust keys scenario.input(k-1) at tick k).\n",
        led.death_ticks.len(),
        led.respawn_ticks.len(),
        led.pickup_ticks.len(),
        led.inflight_hit_ticks.len(),
        led.rope_attach_ticks.len(),
        led.peak_bobjects,
        BLOOD_CAP,
        led.death_ticks.first().map(|x| x.0).unwrap_or(0),
        led.respawn_ticks.first().map(|x| x.0).unwrap_or(0),
        led.inflight_hit_ticks.first().map(|x| x.0).unwrap_or(0),
        led.rope_attach_ticks.first().map(|x| x.0).unwrap_or(0),
        pick,
        ticks - 1,
    )
    .unwrap();
    for (t, inp) in inputs.iter().enumerate() {
        writeln!(s, "input {t} {} {}", inp[0], inp[1]).unwrap();
    }
    std::fs::write(out_path, s).unwrap_or_else(|e| panic!("write {out_path}: {e}"));
    eprintln!("wrote {out_path} (variant {variant}, {ticks} ticks)");
    print_ledger(&led);
}

/// Print per-tick sobject + nobject population for a tick window (divergence probe).
fn sobtrace(scenario_path: &str, lo: u32, hi: u32) {
    let text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&text).expect("scenario parses");
    let l = load(&scenario.level);
    let worms = worms_from_scenario(&l, &scenario);
    let mut state = build_state(&l, &worms, scenario.seed, scenario.max_bonuses);
    state.game_mode = scenario.game_mode as u32;
    let show = |tick: u32, s: &SimState| {
        if tick < lo || tick > hi {
            return;
        }
        let sob: Vec<String> = s
            .sobjects
            .iter()
            .map(|o| format!("(id{} cf{} ad{} {},{})", o.id, o.cur_frame, o.anim_delay, o.x, o.y))
            .collect();
        let nob: Vec<String> = s
            .nobjects
            .iter()
            .map(|o| format!("(ty{:?} tl{} {},{})", o.ty, o.time_left, ftoi(o.pos.x), ftoi(o.pos.y)))
            .collect();
        println!("t{tick:4} sob[{}]: {}  | nob[{}]: {}", sob.len(), sob.join(" "), nob.len(), nob.join(" "));
    };
    show(0, &state);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        show(k, &state);
    }
}

/// Diagnostic: per tick, fold the sobjects component (h*31+id, h*31+cur_frame — the
/// exact hash.rs recipe) in POOL order and in SORTED (id,cur_frame) order. Comparing
/// the sorted fold to the C++ golden's sobjects column tells us whether the divergence
/// is a pure slot-ORDER difference (sorted matches) or a set/identity difference.
fn sobhash(scenario_path: &str, lo: u32, hi: u32) {
    let text = std::fs::read_to_string(scenario_path).expect("read scenario");
    let scenario = Scenario::parse(&text).expect("scenario parses");
    let l = load(&scenario.level);
    let worms = worms_from_scenario(&l, &scenario);
    let mut state = build_state(&l, &worms, scenario.seed, scenario.max_bonuses);
    state.game_mode = scenario.game_mode as u32;
    let fold = |pairs: &[(i32, i32)]| -> u32 {
        let mut h: u32 = 1;
        for &(id, cf) in pairs {
            h = h.wrapping_mul(31).wrapping_add(id as u32);
            h = h.wrapping_mul(31).wrapping_add(cf as u32);
        }
        h
    };
    let show = |tick: u32, s: &SimState| {
        if tick < lo || tick > hi {
            return;
        }
        let pool: Vec<(i32, i32)> = s.sobjects.iter().map(|o| (o.id, o.cur_frame)).collect();
        let mut sorted = pool.clone();
        sorted.sort();
        println!(
            "t{tick:4} n={} pool_fold={:08x} sorted_fold={:08x}",
            pool.len(),
            fold(&pool),
            fold(&sorted)
        );
    };
    show(0, &state);
    for k in 1..=scenario.ticks {
        let inputs = [
            ControlState::unpack(scenario.input(k - 1, 0)),
            ControlState::unpack(scenario.input(k - 1, 1)),
        ];
        state.process_frame(&inputs);
        show(k, &state);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = |i: usize| a[i].parse().unwrap_or_else(|_| panic!("bad arg {i}: {:?}", a.get(i)));
    match a.get(1).map(String::as_str) {
        Some("scan") => scan(&a[2], p(3), p(4), p(5), p(6)),
        Some("gen") => gen(p(2), p(3), p(4), p(5), &a[6], &a[7]),
        Some("ledger") => ledger(&a[2]),
        Some("dump") => dump(&a[2]),
        Some("sobtrace") => sobtrace(&a[2], p(3), p(4)),
        Some("sobhash") => sobhash(&a[2], p(3), p(4)),
        _ => {
            eprintln!(
                "usage:\n  gen_slice6_fuzz scan <level> <gs_lo> <gs_hi> <input_seed> <ticks>\n  \
                 gen_slice6_fuzz gen <variant> <game_seed> <input_seed> <ticks> <level> <out>\n  \
                 gen_slice6_fuzz ledger <scenario_path>\n  \
                 gen_slice6_fuzz dump <scenario_path>"
            );
            std::process::exit(2);
        }
    }
}
