//! Step 4½c-0 T0 — the weapon-branch inventory (design §2), pinned against the shipped TC.
//!
//! Before 4½c-0 thirteen of the forty openliero weapons reached a C++ branch the Rust sim
//! had not ported (a `debug_assert!` tripwire, or — MISSILE — a silent no-op). This test
//! derives, from the REAL weapon/nobject/sobject configs, which of the mechanisms each
//! weapon reaches and pins the table, so a TC edit that moves a weapon into (or out of) a
//! mechanism fails here first. It also pins the TC facts the ports rely on.

use assets::object::Objects;
use assets::tc::TcConfig;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

fn load() -> (TcConfig, Objects) {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    let objects = Objects::load(&tc.types, |sub, id| {
        std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg"))
    })
    .unwrap();
    (tc, objects)
}

/// The C++ mechanisms that were unported before 4½c-0 (design §2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Mech {
    /// `weapon.cpp:144`, `:336-337` — the `ST_LASER` do-loop (<= 8 steps per tick).
    LaserLoop,
    /// `weapon.cpp:337` `w.id == 28` — the loop does not stop after 8 steps.
    LaserUnbounded,
    /// `worm.cpp:1214-1241` ProcessSteerables (+ `viewport.cpp:30-32`, `weapon.cpp:152`).
    Steerable,
    /// `weapon.cpp:201-210` — the particle trail.
    PartTrail,
    /// `weapon.cpp:107-114` — splinters via `Create1`.
    ScatterCreate1,
    /// `nobject.cpp:133-138` — a splinter (transitively) leaves an sobject trail.
    LeaveObj,
    /// `sobject.cpp:148-150` — chain explosion.
    Chain,
    /// `weapon.cpp:137-142` — the RemExp object (the hack is off in this TC).
    RemExp,
}

/// Does nobject type `n` — or a splinter it spawns, transitively — leave an sobject trail?
fn leaves_obj(o: &Objects, n: i32, depth: u32) -> bool {
    if n < 0 || depth > 8 {
        return false;
    }
    let t = &o.nobject_types[n as usize];
    (t.leave_obj >= 0 && t.leave_obj_delay != 0)
        || (t.splinter_amount > 0 && leaves_obj(o, t.splinter_type, depth + 1))
}

fn mechanisms(tc: &TcConfig, o: &Objects, i: usize) -> Vec<Mech> {
    let w = &o.weapons[i];
    let mut m = Vec::new();
    if w.shot_type == 4 {
        m.push(Mech::LaserLoop);
        if w.id == 28 {
            m.push(Mech::LaserUnbounded);
        }
    }
    if w.shot_type == 2 {
        m.push(Mech::Steerable);
    }
    if w.part_trail_obj >= 0 {
        m.push(Mech::PartTrail);
    }
    if w.splinter_amount > 0 && w.splinter_scatter != 0 {
        m.push(Mech::ScatterCreate1);
    }
    if (w.splinter_amount > 0 && leaves_obj(o, w.splinter_type, 0))
        || leaves_obj(o, w.part_trail_obj, 0)
    {
        m.push(Mech::LeaveObj);
    }
    if w.chain_explosion && w.affect_by_explosions {
        m.push(Mech::Chain);
    }
    if w.id == tc.constants.RemExpObject - 1 {
        m.push(Mech::RemExp);
    }
    m
}

#[test]
fn thirteen_weapons_reach_a_formerly_unported_branch() {
    use Mech::*;
    let (tc, o) = load();
    let want: [(&str, &[Mech]); 13] = [
        ("RIFLE", &[LaserLoop]),
        ("WINCHESTER", &[LaserLoop]),
        ("GAUSS GUN", &[LaserLoop]),
        ("LASER", &[LaserLoop, LaserUnbounded]),
        ("MISSILE", &[Steerable]),
        ("LARPA", &[PartTrail]),
        ("BOUNCY LARPA", &[PartTrail]),
        ("CRACKLER", &[PartTrail]),
        ("MINI NUKE", &[ScatterCreate1, LeaveObj]),
        ("BIG NUKE", &[LeaveObj]),
        ("NAPALM", &[LeaveObj]),
        ("HELLRAIDER", &[LeaveObj]),
        ("BOOBY TRAP", &[Chain, RemExp]),
    ];
    let mut want: Vec<(String, Vec<Mech>)> = want
        .iter()
        .map(|(n, m)| (n.to_string(), m.to_vec()))
        .collect();
    let mut got: Vec<(String, Vec<Mech>)> = (0..o.weapons.len())
        .map(|i| (o.weapons[i].name.clone(), mechanisms(&tc, &o, i)))
        .filter(|(_, m)| !m.is_empty())
        .collect();
    want.sort();
    got.sort();
    assert_eq!(
        got, want,
        "design §2.2: the thirteen weapons and their mechanisms"
    );
}

#[test]
fn the_tc_facts_the_ports_rely_on() {
    let (tc, o) = load();
    let idx = |name: &str| {
        o.weapons
            .iter()
            .position(|w| w.name == name)
            .unwrap_or_else(|| panic!("no weapon {name:?}")) as i32
    };
    // weapon.cpp:337 hard-codes `w.id == 28`; common.cpp:495 sets id = index — in this TC
    // weapon 28 IS the LASER, which is also the 1-based `LC(LaserWeapon)` sight/beam slot.
    assert_eq!(idx("LASER"), 28);
    assert_eq!(o.weapons[28].id, 28);
    assert_eq!(tc.constants.LaserWeapon - 1, 28);
    // weapon.cpp:138: the 1-based RemExpObject is the BOOBY TRAP, and the hack is OFF.
    assert_eq!(tc.constants.RemExpObject - 1, idx("BOOBY TRAP"));
    assert!(
        !tc.hacks.RemExp,
        "RemExp off: the ported block is inert in this TC"
    );
    // weapon.cpp:203 / :208 — the particle-trail divisors.
    assert_eq!(
        (
            tc.constants.SplinterLarpaVelDiv,
            tc.constants.SplinterCracklerVelDiv
        ),
        (3, 3)
    );
    for w in &o.weapons {
        // weapon.cpp:336 `used`: the Rust driver cannot observe it, so nothing inside an
        // ST_LASER body may free `this` — no ST_LASER weapon spawns a trail (design §4.1).
        if w.shot_type == 4 {
            assert!(
                w.obj_trail_type < 0 && w.part_trail_obj < 0,
                "{}: an ST_LASER weapon with a trail",
                w.name
            );
        }
        // A chain-exploding weapon whose own obj trail damages could blow itself up
        // mid-Process (C++ frees `this`); no TC weapon combines them (design §4.8).
        if w.chain_explosion && w.affect_by_explosions && w.obj_trail_type >= 0 {
            assert_eq!(
                o.sobject_types[w.obj_trail_type as usize].damage, 0,
                "{}: self-chain through its own trail",
                w.name
            );
        }
        // `cycles % part_trail_delay` never divides by zero (design §4.5).
        if w.part_trail_obj >= 0 {
            assert!(w.part_trail_delay > 0, "{}: zero part_trail_delay", w.name);
        }
    }
}
