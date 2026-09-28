//! Step 4½c-0 — the weapon-branch golden helpers, shared by `examples/gen_slice4_5c0.rs`
//! (via `#[path]`), `sim_slice4_5c0_weapons_golden.rs` and `render_slice4_5c0_steer.rs`:
//! the variant table (design §5.2), the lean setup sidecar, the input streams and the
//! per-branch REACH WITNESSES (design §5.3). One copy, so the generator's seed choice and
//! the milestone's non-vacuity assertions cannot drift apart.

#![allow(dead_code)] // each includer uses a subset.

use std::collections::BTreeSet;

use assets::object::Objects;
use assets::tc::TcConfig;
use sim::state::{ControlState, NObject, SimState, WObject};
use sim_core::fixed::ftoi;
use sim_core::rng::Rand;

pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// The 4½a-1 fuzz arena.
pub const LEVEL: &str = "Levels/modern_test.lev";
/// Every variant's sim settings (design §5.2). Lives 99: the match never ends, so column
/// 12 (`IsGameOver`) stays 0 — these goldens are about weapons, not match end.
pub const LIVES: i32 = 99;
pub const HEALTH: i32 = 100;
pub const LOADING_TIME: i32 = 20;
pub const BLOOD: i32 = 100;
pub const MAX_BONUSES: i32 = 4;
pub const BLOOD_PARTICLE_MAX: i32 = 700;

/// The thirteen weapons whose C++ branches were unported before 4½c-0 (design §2.2).
pub const FORMERLY_DEFERRED: [&str; 13] = [
    "RIFLE",
    "WINCHESTER",
    "LASER",
    "GAUSS GUN",
    "MISSILE",
    "LARPA",
    "BOUNCY LARPA",
    "CRACKLER",
    "MINI NUKE",
    "BIG NUKE",
    "NAPALM",
    "HELLRAIDER",
    "BOOBY TRAP",
];

pub struct Variant {
    pub name: &'static str,
    /// Seed of the per-tick 7-bit input stream (fixed per variant).
    pub input_seed: u32,
    /// Scenario length; lengthen a variant here if its scan finds no seed (design §10).
    pub ticks: u32,
    pub p1: [&'static str; 5],
    pub p2: [&'static str; 5],
}

/// Design §5.2's table: one variant per mechanism group, witnesses per weapon.
pub const VARIANTS: [Variant; 4] = [
    Variant {
        name: "laser",
        input_seed: 1001,
        ticks: 1500,
        p1: ["RIFLE", "WINCHESTER", "LASER", "GAUSS GUN", "HANDGUN"],
        p2: ["LASER", "GAUSS GUN", "RIFLE", "WINCHESTER", "DART"],
    },
    Variant {
        name: "missile",
        input_seed: 1002,
        ticks: 1500,
        p1: ["MISSILE", "BAZOOKA", "MISSILE", "GRENADE", "MISSILE"],
        p2: ["MISSILE", "DART", "MISSILE", "CANNON", "MISSILE"],
    },
    Variant {
        name: "trails",
        input_seed: 1003,
        ticks: 2000,
        p1: ["LARPA", "CRACKLER", "NAPALM", "MINI NUKE", "HELLRAIDER"],
        p2: [
            "BOUNCY LARPA",
            "BIG NUKE",
            "NAPALM",
            "MINI NUKE",
            "CRACKLER",
        ],
    },
    Variant {
        name: "booby",
        input_seed: 1004,
        ticks: 2000,
        p1: [
            "BOOBY TRAP",
            "BAZOOKA",
            "BOOBY TRAP",
            "GRENADE",
            "BOOBY TRAP",
        ],
        p2: ["BOOBY TRAP", "CANNON", "BOOBY TRAP", "DART", "BOOBY TRAP"],
    },
];

pub fn variant(name: &str) -> &'static Variant {
    VARIANTS
        .iter()
        .find(|v| v.name == name)
        .unwrap_or_else(|| panic!("unknown variant {name}"))
}

pub fn load_objects() -> Objects {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap()
}

/// Index into `objects.weapons` (== the `weap_table` index == C++ `Weapon::id`).
pub fn weapon_index(o: &Objects, name: &str) -> i32 {
    o.weapons
        .iter()
        .position(|w| w.name == name)
        .unwrap_or_else(|| panic!("no weapon {name:?}")) as i32
}

/// The 1-based `weap_order` index a `WormSettings.weapons` entry stores.
pub fn menu_index(o: &Objects, name: &str) -> u32 {
    let mut order: Vec<usize> = (0..o.weapons.len()).collect();
    order.sort_by(|&a, &b| o.weapons[a].name.cmp(&o.weapons[b].name));
    order
        .iter()
        .position(|&i| o.weapons[i].name == name)
        .unwrap() as u32
        + 1
}

fn arr(v: &[u32]) -> String {
    let items: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("[ {} ]", items.join(", "))
}

/// The LEAN setup sidecar (design §5.2): only the sim-reaching keys. Missing keys keep
/// their defaults in both readers (`toml_archive.hpp:177-186`, `:224-229`), so the golden
/// cross-checks exactly what matters. `weapTable` all zero: every weapon may drop (§6).
pub fn setup_cfg(v: &Variant, o: &Objects) -> String {
    let pick = |names: &[&str; 5]| -> [u32; 5] { names.map(|n| menu_index(o, n)) };
    let mut out = String::new();
    for (header, names) in [("player1", &v.p1), ("player2", &v.p2)] {
        out.push_str(&format!(
            "[{header}]\nhealth = {HEALTH}\nweapons = {}\n\n",
            arr(&pick(names))
        ));
    }
    out.push_str("[settings]\n");
    out.push_str(&format!(
        "blood = {BLOOD}\nbloodParticleMax = {BLOOD_PARTICLE_MAX}\n"
    ));
    out.push_str(&format!(
        "gameMode = 0\nlevelFile = '{LEVEL}'\nlives = {LIVES}\n"
    ));
    out.push_str(&format!(
        "loadChange = true\nloadingTime = {LOADING_TIME}\n"
    ));
    out.push_str(&format!(
        "maxBonuses = {MAX_BONUSES}\nrandomLevel = false\n"
    ));
    out.push_str("shadow = true\ntimeToLose = 600\nversion = 6\n");
    out.push_str(&format!("weapTable = {}\n", arr(&[0u32; 40])));
    out
}

/// Per tick `Rand(input_seed).next_u32() & 0x7f` for worm 0 then worm 1 (the 4½a-1 fuzz
/// stream), applied on the pass advancing t -> t+1.
pub fn inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    (0..ticks)
        .map(|_| [r.next_u32() & 0x7f, r.next_u32() & 0x7f])
        .collect()
}

/// The steer render stream (design §5.4): Up/Down/Left/Right/Jump random, Change NEVER
/// (the current weapon stays MISSILE), Fire one tick in eight.
pub fn steer_inputs(input_seed: u32, ticks: u32) -> Vec<[u32; 2]> {
    let mut r = Rand::new();
    r.seed(input_seed);
    let mut one = || {
        let v = r.next_u32();
        (v & 0x4f) | if (v >> 8) & 7 == 0 { 0x10 } else { 0 }
    };
    (0..ticks).map(|_| [one(), one()]).collect()
}

/// The object/weapon ids the witnesses read, resolved by name from the real TC.
pub struct Ids {
    pub rifle: i32,
    pub winchester: i32,
    pub laser: i32,
    pub gauss: i32,
    pub missile: i32,
    pub larpa: i32,
    pub bouncy_larpa: i32,
    pub crackler: i32,
    pub mini_nuke: i32,
    pub booby: i32,
    pub napalm_fireballs: i32,
    pub small_nukes: i32,
    pub large_nukes: i32,
    pub hellraider_bullets: i32,
    /// LASER's `create_on_exp` (`very_small_explosion__silent`; LASER is its only user).
    pub laser_boom: i32,
    /// The thirteen formerly-deferred weapon indices.
    pub deferred: BTreeSet<i32>,
}

impl Ids {
    pub fn new(o: &Objects) -> Ids {
        let w = |name: &str| weapon_index(o, name);
        let n = |id: &str| {
            o.nobject_types
                .iter()
                .position(|t| t.id_str == id)
                .unwrap_or_else(|| panic!("no nobject {id:?}")) as i32
        };
        let laser = w("LASER");
        Ids {
            rifle: w("RIFLE"),
            winchester: w("WINCHESTER"),
            laser,
            gauss: w("GAUSS GUN"),
            missile: w("MISSILE"),
            larpa: w("LARPA"),
            bouncy_larpa: w("BOUNCY LARPA"),
            crackler: w("CRACKLER"),
            mini_nuke: w("MINI NUKE"),
            booby: w("BOOBY TRAP"),
            napalm_fireballs: n("napalm_fireballs"),
            small_nukes: n("small_nukes"),
            large_nukes: n("large_nukes"),
            hellraider_bullets: n("hellraider_bullets"),
            laser_boom: o.weapons[laser as usize].create_on_exp,
            deferred: FORMERLY_DEFERRED.iter().map(|&x| w(x)).collect(),
        }
    }
}

/// The pre-tick facts the witnesses compare against the post-tick state.
pub struct Pre {
    /// The `cycles` the tick's object loops read (pre-`++cycles`).
    pub cycles: i32,
    pub visible: [bool; 2],
    pub ipos: [(i32, i32); 2],
    pub wobjects: Vec<(usize, WObject)>,
    pub nobjects: Vec<NObject>,
    /// `(id, x, y)` of every live sobject.
    pub sobjects: Vec<(i32, i32, i32)>,
}

impl Pre {
    pub fn capture(st: &SimState) -> Pre {
        Pre {
            cycles: st.cycles,
            visible: [st.worms[0].visible, st.worms[1].visible],
            ipos: [0usize, 1].map(|i| (ftoi(st.worms[i].pos.x), ftoi(st.worms[i].pos.y))),
            wobjects: (0..st.wobjects.capacity())
                .filter_map(|s| st.wobjects.get(s).map(|w| (s, *w)))
                .collect(),
            nobjects: st.nobjects.iter().copied().collect(),
            sobjects: st.sobjects.iter().map(|s| (s.id, s.x, s.y)).collect(),
        }
    }
}

/// Centres `(x + 8, y + 8)` of the sobjects of type `id` that appeared this tick.
fn new_sobjects(pre: &Pre, st: &SimState, id: i32) -> Vec<(i32, i32)> {
    st.sobjects
        .iter()
        .filter(|s| s.id == id && !pre.sobjects.contains(&(s.id, s.x, s.y)))
        .map(|s| (s.x + 8, s.y + 8))
        .collect()
}

/// The wobject that sat in `slot` before the tick is gone (freed, or the slot reused).
fn vanished(slot: usize, a: &WObject, st: &SimState) -> bool {
    st.wobjects
        .get(slot)
        .map_or(true, |b| b.ty != a.ty || b.owner_idx != a.owner_idx)
}

/// Everything the witnesses read (design §5.3), from the genuinely driven state.
#[derive(Default, Debug)]
pub struct Ledger {
    pub deaths: u32,
    pub respawns: u32,
    pub bonus_dropped: bool,
    /// A bonus offering one of the thirteen appeared (the ban lift).
    pub deferred_bonus: bool,
    /// Every weapon type that entered an object loop.
    pub in_flight: BTreeSet<i32>,
    /// M1: RIFLE / WINCHESTER / GAUSS GUN survived a tick with Δvel.y == 8 * gravity.
    pub multi_iter: [bool; 3],
    /// M1: a LASER explosion >= 10 px from every LASER that entered the tick.
    pub laser_long: bool,
    /// M1: LASER / RIFLE / WINCHESTER removed by a worm hit (vanished, no own blast).
    pub laser_hit: [bool; 3],
    /// M2: a steering tick with Left / Right held, on an even / odd cycle.
    pub steer_left: bool,
    pub steer_right: bool,
    pub steer_step1: bool,
    pub steer_step2: bool,
    /// M3: the owner held Up while its missile entered the object loop.
    pub boosted: bool,
    /// M5: LARPA / BOUNCY LARPA / CRACKLER entered a tick on its trail delay.
    pub part_trail: [bool; 3],
    /// M7: napalm fireball / small nuke / large nuke / hellraider bullet trail spawned.
    pub leave_obj: [bool; 4],
    /// M6: a MINI NUKE vanished and small nukes appeared.
    pub scatter_create1: bool,
    /// M8: a BOOBY TRAP was in flight / went off far from any worm with its timer running.
    pub booby_in_flight: bool,
    pub chain: bool,
}

impl Ledger {
    pub fn observe(&mut self, ids: &Ids, o: &Objects, pre: &Pre, st: &SimState, input: [u32; 2]) {
        for i in 0..2 {
            if pre.visible[i] && !st.worms[i].visible {
                self.deaths += 1;
            }
            if !pre.visible[i] && st.worms[i].visible {
                self.respawns += 1;
            }
        }
        for (_, a) in &pre.wobjects {
            if let Some(t) = a.ty {
                self.in_flight.insert(t);
            }
        }
        self.bonus_dropped |= !st.bonuses.is_empty();
        for b in st.bonuses.iter() {
            if b.frame == 0 && ids.deferred.contains(&b.weapon) {
                self.deferred_bonus = true;
            }
        }

        // M1 — multi-step survivors: kept the slot, Δvel.x == 0, Δvel.y == 8 * gravity
        // (eight air steps; a single-step port gives 1 * gravity).
        for (k, w) in [ids.rifle, ids.winchester, ids.gauss]
            .into_iter()
            .enumerate()
        {
            let g = o.weapons[w as usize].gravity;
            for (slot, a) in &pre.wobjects {
                if a.ty != Some(w) {
                    continue;
                }
                if let Some(b) = st.wobjects.get(*slot) {
                    if b.ty == a.ty
                        && b.owner_idx == a.owner_idx
                        && b.vel.x == a.vel.x
                        && b.vel.y.wrapping_sub(a.vel.y) == 8 * g
                    {
                        self.multi_iter[k] = true;
                    }
                }
            }
        }
        // M1 — the unbounded LASER (id 28): one of this tick's LASER blasts landed >= 10 px
        // (Chebyshev) from every LASER that entered the tick — beyond 8 one-pixel steps.
        let lasers: Vec<(i32, i32)> = pre
            .wobjects
            .iter()
            .filter(|(_, a)| a.ty == Some(ids.laser))
            .map(|(_, a)| (ftoi(a.pos.x), ftoi(a.pos.y)))
            .collect();
        if !lasers.is_empty() {
            for (bx, by) in new_sobjects(pre, st, ids.laser_boom) {
                if lasers
                    .iter()
                    .all(|&(px, py)| (bx - px).abs().max((by - py).abs()) >= 10)
                {
                    self.laser_long = true;
                }
            }
        }
        // M1 — a worm hit inside the loop: the wobject vanished and no sobject of its
        // create_on_exp type appeared anywhere this tick (the Remove arm, weapon.cpp:316-322).
        for (k, w) in [ids.laser, ids.rifle, ids.winchester]
            .into_iter()
            .enumerate()
        {
            let exp = o.weapons[w as usize].create_on_exp;
            let gone = pre
                .wobjects
                .iter()
                .any(|(slot, a)| a.ty == Some(w) && vanished(*slot, a, st));
            if gone && new_sobjects(pre, st, exp).is_empty() {
                self.laser_hit[k] = true;
            }
        }

        // M2 — steering: `steerable_count` is set ONLY by ProcessSteerables, so a post-tick
        // count > 0 with a key held means that key turned a missile this tick.
        // M3 — the Up boost: the owner was visible and held Up while its missile entered.
        for i in 0..2 {
            let c = ControlState::unpack(input[i]);
            let (l, r) = (c.get(ControlState::LEFT), c.get(ControlState::RIGHT));
            if st.worms[i].steerable_count > 0 {
                self.steer_left |= l;
                self.steer_right |= r;
                if l || r {
                    if st.cycles & 1 == 0 {
                        self.steer_step1 = true;
                    } else {
                        self.steer_step2 = true;
                    }
                }
            }
            if pre.visible[i]
                && c.get(ControlState::UP)
                && pre
                    .wobjects
                    .iter()
                    .any(|(_, a)| a.ty == Some(ids.missile) && a.owner_idx == i as i32)
            {
                self.boosted = true;
            }
        }

        // M5 — particle trails: a trail weapon entered a tick on its part_trail_delay.
        for (k, w) in [ids.larpa, ids.bouncy_larpa, ids.crackler]
            .into_iter()
            .enumerate()
        {
            let d = o.weapons[w as usize].part_trail_delay;
            if d > 0 && pre.cycles % d == 0 && pre.wobjects.iter().any(|(_, a)| a.ty == Some(w)) {
                self.part_trail[k] = true;
            }
        }
        // M7 — leave_obj trails: the nobject entered a tick on its delay AND a new sobject
        // of its leave_obj type appeared.
        let trails = [
            ids.napalm_fireballs,
            ids.small_nukes,
            ids.large_nukes,
            ids.hellraider_bullets,
        ];
        for (k, n) in trails.into_iter().enumerate() {
            let t = &o.nobject_types[n as usize];
            if t.leave_obj_delay > 0
                && pre.cycles % t.leave_obj_delay == 0
                && pre.nobjects.iter().any(|a| a.ty == Some(n))
                && !new_sobjects(pre, st, t.leave_obj).is_empty()
            {
                self.leave_obj[k] = true;
            }
        }
        // M6 — Create1 splinters: a MINI NUKE vanished and small nukes appeared.
        let mini_gone = pre
            .wobjects
            .iter()
            .any(|(slot, a)| a.ty == Some(ids.mini_nuke) && vanished(*slot, a, st));
        let small_before = pre
            .nobjects
            .iter()
            .filter(|a| a.ty == Some(ids.small_nukes))
            .count();
        let small_after = st
            .nobjects
            .iter()
            .filter(|a| a.ty == Some(ids.small_nukes))
            .count();
        if mini_gone && small_after > small_before {
            self.scatter_create1 = true;
        }
        // M8 — chain explosion: a BOOBY TRAP with its timer running vanished with no
        // visible worm within 20 px — its only other exits are a worm hit and its timeout.
        for (slot, a) in &pre.wobjects {
            if a.ty != Some(ids.booby) {
                continue;
            }
            self.booby_in_flight = true;
            let (bx, by) = (ftoi(a.pos.x), ftoi(a.pos.y));
            let worm_near = (0..2).any(|i| {
                pre.visible[i] && (pre.ipos[i].0 - bx).abs() < 20 && (pre.ipos[i].1 - by).abs() < 20
            });
            if vanished(*slot, a, st) && a.time_left > 0 && !worm_near {
                self.chain = true;
            }
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "deaths={} respawns={} bonus={} deferred_bonus={} multi_iter={:?} laser_long={} \
             laser_hit={:?} steer(l={} r={} s1={} s2={}) boosted={} part_trail={:?} \
             leave_obj={:?} scatter={} booby={} chain={} in_flight={:?}",
            self.deaths,
            self.respawns,
            self.bonus_dropped,
            self.deferred_bonus,
            self.multi_iter,
            self.laser_long,
            self.laser_hit,
            self.steer_left,
            self.steer_right,
            self.steer_step1,
            self.steer_step2,
            self.boosted,
            self.part_trail,
            self.leave_obj,
            self.scatter_create1,
            self.booby_in_flight,
            self.chain,
            self.in_flight,
        )
    }
}

/// The variant's witnesses (design §5.3). Every variant: both worms spawned, a bonus
/// dropped; `trails` also witnesses the ban lift.
pub fn ok(v: &Variant, l: &Ledger) -> bool {
    let base = l.respawns >= 2 && l.bonus_dropped;
    base && match v.name {
        "laser" => {
            l.multi_iter.iter().all(|&b| b) && l.laser_long && l.laser_hit.iter().any(|&b| b)
        }
        "missile" => l.steer_left && l.steer_right && l.steer_step1 && l.steer_step2 && l.boosted,
        "trails" => {
            l.part_trail.iter().all(|&b| b)
                && l.leave_obj[0]
                && (l.leave_obj[1] || l.leave_obj[2])
                && l.leave_obj[3]
                && l.scatter_create1
                && l.deferred_bonus
        }
        "booby" => l.booby_in_flight && l.chain,
        other => panic!("unknown variant {other}"),
    }
}

/// The steer render golden's witnesses (design §5.4).
#[derive(Default, Debug)]
pub struct SteerLedger {
    /// `(tick, worm)`: visible, `killed_timer <= 0`, `steerable_count > 0` — the viewport's
    /// centroid arm (`viewport.cpp:30-32`) runs for that worm's viewport.
    pub camera_ticks: Vec<(u32, usize)>,
    /// The camera ticks whose centroid is >= 8 px (Chebyshev) from the worm.
    pub off_worm_ticks: Vec<(u32, usize)>,
    pub left: bool,
    pub right: bool,
    pub boosted: bool,
}

impl SteerLedger {
    pub fn observe(&mut self, k: u32, missile: i32, pre: &Pre, st: &SimState, input: [u32; 2]) {
        for i in 0..2 {
            let w = &st.worms[i];
            let c = ControlState::unpack(input[i]);
            if w.steerable_count > 0 {
                self.left |= c.get(ControlState::LEFT);
                self.right |= c.get(ControlState::RIGHT);
                if w.visible && w.killed_timer <= 0 {
                    self.camera_ticks.push((k, i));
                    let cx = w.steerable_sum_x / w.steerable_count;
                    let cy = w.steerable_sum_y / w.steerable_count;
                    if (cx - ftoi(w.pos.x)).abs().max((cy - ftoi(w.pos.y)).abs()) >= 8 {
                        self.off_worm_ticks.push((k, i));
                    }
                }
            }
            if pre.visible[i]
                && c.get(ControlState::UP)
                && pre
                    .wobjects
                    .iter()
                    .any(|(_, a)| a.ty == Some(missile) && a.owner_idx == i as i32)
            {
                self.boosted = true;
            }
        }
    }
}

pub fn steer_ok(l: &SteerLedger) -> bool {
    !l.off_worm_ticks.is_empty() && l.left && l.right && l.boosted
}
