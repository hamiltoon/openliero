# Step 4½, Slice 4½c-0 — the unported Step-2 weapon branches: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** Port, bit-exact against C++, every `WObject::Process`-family branch that a TC weapon can reach and the Rust sim still stubs — the `ST_LASER` do-loop (RIFLE, WINCHESTER, GAUSS GUN, LASER), `ProcessSteerables` + the steerable camera (MISSILE), the particle trail (LARPA, BOUNCY LARPA, CRACKLER), `Create1` splinters (MINI NUKE), the nobject `leave_obj` trail (MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER), chain explosions (BOOBY TRAP) and RemExp — plus the input-application-point fix MISSILE exposes, so 4½c can offer all forty weapons.

**Architecture:** Everything lands in the existing Bevy-free crates: `sim::weapon` (a `WObjectConsts` TC-const struct, `wobject_process` becomes the do-loop around a private `wobject_pass`, the particle trail, `Create1` splinters, `process_steerables`), `sim::nobject` (the trail), `sim::sobject` (chain recursion), `sim::state` (driver order, step 3, inputs at the top of the tick, two `WormState` fields), `render::viewport` (the steerable centroid). The oracle reuses 4½a-1's `settings <file>` path for four mechanism-grouped fuzz goldens with per-branch reach witnesses, and 4d's `render_live` path for one steerable-camera golden; the only C++ edit moves the dumper's reduced-tail input `Unpack` to the top of the tick (proven hash-neutral by regenerating 21 goldens).

**Tech Stack:** Rust 2021 (`sim`, `render`, `scenario`, `oracle-tests`) and 2024 (`game`, untouched); C++ (`src/tools/oracle_dump/sim_physics_dump.cpp`, preset `macos-arm64`, clang-format 22).

**Spec:** `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-slice4.5c0-weapon-branches-design.md` (cited **design §N**). Overview: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md`.

## Global Constraints

- Worktree `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5`, branch `liero-rs-step-4-5` (base `03de202`). Never `cd`; use `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 …` and absolute paths. Every cargo command passes `--manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml`.
- Bash hygiene: one simple command per call; no `&&`, `;`, `$VAR`, heredocs, `>`/`>>` redirection, `find -exec`. Create and edit files with the editor tools (scripts on disk may use shell features). No sub-subagents.
- Determinism: no floats, no wall-clock, no `HashMap` iteration in `sim`; C++ `static_cast` narrowing is Rust `as`; C++ two's-complement fixed math is `wrapping_*`; C++ `int /` is Rust `/` on `i32` (both truncate toward zero); `Vec2::div` is the truncating fixed-vector divide.
- `SimState::new`'s signature does NOT change: `wobject_consts` is a post-`new` field with an inert `Default`; `steerable_sum_x/y` default to 0 in `WormState::from_init`.
- The scenario grammar is frozen: no new directive. Goldens use the existing `settings <file>` (4½a-1) and `weapon` / `render player` / `render_live` / `max_bonuses` directives.
- All prior goldens stay byte-identical: after every task `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` may list only NEW `sim_slice4_5c0_*` / `render_slice4_5c0_steer*` files. The re-diff is `cargo test … --workspace --exclude game` plus `cargo test … -p game`, always in DEBUG (never `--release`: some tests are `#[should_panic]` on `debug_assert!`s).
- `sim-core` stays dependency-free; `sim` gains no dependency; Bevy stays confined to `game`.
- C++ changes are confined to `src/tools/oracle_dump/sim_physics_dump.cpp` (T8 only); the whole file must pass `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/sim_physics_dump.cpp`. If the first C++ configure has to bootstrap vcpkg, prefix the gen-script command with `VCPKG_ROOT=/Users/john/code/openliero/tools/vcpkg/vcpkg`.
- rustfmt only files this plan CREATES (`rustfmt --edition 2021 <abs file>`); never run rustfmt on an existing file or on a `lib.rs`/`main.rs` (it recurses) — hand-format edits to the surrounding style.
- Do not touch the parallel slice's files (`*slice4.5a2*`) or the 4½a-1 ban provenance (`rust/oracle-tests/examples/gen_slice4_5a.rs`, `rust/oracle-tests/golden/sim_slice4_5a_*`, `rust/oracle-tests/tests/sim_slice4_5a_settings_golden.rs`) — design §6.
- Commits use the globally-configured identity (john.alm.martensson@pm.me); do not override it. Every commit message carries two trailers as extra `-m` arguments: `Co-Authored-By: Claude <model> <noreply@anthropic.com>` naming the model that did the work (the commit lines below show `Claude Opus 5`; a Sonnet executor writes its own model name), and `Claude-Session: <session-url>` where `<session-url>` is the executing session's URL from its attribution instructions. Never write "Generated with Claude Code" anywhere.
- Do NOT push and do NOT open a PR — the controller owns push + PR.

## Model tiers

- **[Opus]:** T2 (particle-trail RNG order), T3 (the laser do-loop), T5, T6 (chain recursion + driver order), T7, T8 (C++ + regeneration proof), T10, T11, T12 (goldens + milestone), T13 (broad review) — and every reviewer.
- **[Sonnet]:** T0 (pin test), T1 (const plumbing + a six-line port), T4, T9 (mechanical, fully specified).

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `rust/oracle-tests/tests/weapon_branch_inventory.rs` (create) | T0 | the design §2 table + the facts the ports rely on, derived from the TC configs |
| `rust/sim/src/weapon.rs` (modify) | T1–T4, T7 | `WObjectConsts`; RemExp; particle trail; `wobject_process` loop + `wobject_pass`; `blow_up(vel)` + `Create1` splinters; `process_steerables` |
| `rust/sim/src/nobject.rs` (modify) | T5 | the `leave_obj` sobject trail |
| `rust/sim/src/sobject.rs` (modify) | T6 | chain explosions (index walk + `blow_up` recursion) |
| `rust/sim/src/state.rs` (modify) | T1, T4, T6, T7, T8 | `wobject_consts` field; driver `vel` + free-before-explode; step 3; `steerable_sum_x/y`; inputs at the top of the tick |
| `rust/sim/src/{hash,wide_checksum,physics}.rs` (modify) | T1, T7 | test literals (`wobject_consts`, `steerable_sum_*`) |
| `rust/sim/tests/process_frame_objects.rs` (modify) | T1, T4 | the direct `wobject_process` / `blow_up` calls |
| `rust/render/src/viewport.rs` (modify) | T7, T9 | test literal; the steerable-centroid camera arm |
| `rust/scenario/src/build.rs`, `rust/scenario/src/loader.rs` (modify) | T1 | assign `wobject_consts` (+ `laser_weapon` in `load`) |
| `src/tools/oracle_dump/sim_physics_dump.cpp` (modify) | T8 | reduced tail: `Unpack` every worm at the top of the tick |
| `rust/oracle-tests/tests/sim_slice4_5c0_common/mod.rs` (create) | T10 | variants, lean sidecar writer, input streams, reach witnesses |
| `rust/oracle-tests/examples/gen_slice4_5c0.rs` (create) | T10, T11 | `cfg` / `scan` / `gen` / `steer-scan` / `steer-gen` |
| `rust/oracle-tests/gen_sim_slice4_5c0_golden.sh` (create) | T10 | C++ golden generation + awk gate |
| `rust/oracle-tests/golden/sim_slice4_5c0_{laser,missile,trails,booby}{_setup.cfg,_scenario.txt,.txt}` (create) | T10 | the four settings-driven goldens |
| `rust/oracle-tests/tests/render_slice4d_common/mod.rs` (modify) | T11 | `run_stem` / `modified_stem` / `read_golden_stem` |
| `rust/oracle-tests/gen_render_slice4_5c0_steer.sh` (create) | T11 | C++ `render_live` golden generation |
| `rust/oracle-tests/golden/render_slice4_5c0_steer{_scenario.txt,_sim.txt,.txt}` (create) | T11 | the steerable-camera golden |
| `rust/oracle-tests/tests/sim_slice4_5c0_weapons_golden.rs` (create) | T12 | MILESTONE — four sim goldens |
| `rust/oracle-tests/tests/render_slice4_5c0_steer.rs` (create) | T12 | MILESTONE — the camera golden |
| `docs/superpowers/liero-rs-PROGRESS.md`, the overview (modify) | T13 | status |

## Task dependency map

```
T0 ──────────────────────────────────────────────────────────┐
T1 ─> T2 ─> T3 ─┬─> T4 ─> T6 ─┐                               │
                ├─> T5 ───────┤                               │
                └─> T7 ─> T8 ─┴─> T9 ─> T10 ─> T11 ─> T12 ─> T13
```

---

### Task 0: The weapon-branch inventory, pinned against the TC  [Sonnet]

**Files:**
- Create: `rust/oracle-tests/tests/weapon_branch_inventory.rs`

**Interfaces:**
- Consumes: `assets::tc::TcConfig::load`, `assets::object::Objects::load` (ids = index, `object.rs:426-466`); `Weapon` fields `shot_type, id, part_trail_obj, part_trail_delay, splinter_amount, splinter_scatter, splinter_type, chain_explosion, affect_by_explosions, obj_trail_type, name`; `NObjectType` fields `leave_obj, leave_obj_delay, splinter_amount, splinter_type`; `SObjectType::damage`; `tc.constants.{LaserWeapon, RemExpObject, SplinterLarpaVelDiv, SplinterCracklerVelDiv}`, `tc.hacks.RemExp`.
- Produces: nothing used by later tasks — an executable copy of design §2.2/§2.3.

Why: design §2. The ports below each rely on a TC fact (LASER is index 28, BOOBY TRAP is the RemExp object, no `ST_LASER` weapon has a trail, …). Pin them where a TC edit fails first. This is a pin, not a port: it is GREEN from the start.

- [ ] **Step 1: Write the test** — create `rust/oracle-tests/tests/weapon_branch_inventory.rs`:

```rust
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
    assert_eq!(got, want, "design §2.2: the thirteen weapons and their mechanisms");
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
    assert!(!tc.hacks.RemExp, "RemExp off: the ported block is inert in this TC");
    // weapon.cpp:203 / :208 — the particle-trail divisors.
    assert_eq!(
        (tc.constants.SplinterLarpaVelDiv, tc.constants.SplinterCracklerVelDiv),
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
```

- [ ] **Step 2: Run it**

Run: `rustfmt --edition 2021 /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/weapon_branch_inventory.rs`
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test weapon_branch_inventory`
Expected: PASS, 2 tests. (A pin, not a port. If `thirteen_weapons…` fails, the TC differs from design §2.2 — stop and report the `got` list; do not edit `want` to match.)

- [ ] **Step 3: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/weapon_branch_inventory.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "test(4.5c-0): pin the weapon-branch inventory (13 weapons, 8 mechanisms) against the TC" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 1: `WObjectConsts` plumbing + the RemExp port  [Sonnet]

**Files:**
- Modify: `rust/sim/src/weapon.rs` (imports `:29`; new struct after the `ST_*` consts `:42-46`; `wobject_process` signature `:322-343`, its doc `:316-320`, a new block at the top of its body; the four test helpers `proc_no_worms`/`proc_hit`/`proc_trail`/`proc_impulse`; new tests)
- Modify: `rust/sim/src/state.rs` (import `:34`; field after `laser_weapon` `:1071`; `new` after `laser_weapon: 0,` `:1382`; destructure `:1585`; `:1629`; the `wobject_process` call `:1748-1769`)
- Modify: `rust/sim/src/hash.rs` (`:248`), `rust/sim/src/wide_checksum.rs` (`:185`) — test literals
- Modify: `rust/sim/tests/process_frame_objects.rs` (`:368-389`)
- Modify: `rust/scenario/src/build.rs` (`:171`, test `:394`), `rust/scenario/src/loader.rs` (`:185`, new test)

**Interfaces:**
- Produces (used by T2, T3, T10):
  - `pub struct WObjectConsts { pub h_rem_exp: bool, pub rem_exp_object: i32, pub splinter_larpa_vel_div: i32, pub splinter_crackler_vel_div: i32 }` — `Clone, Copy, Debug, Default, PartialEq, Eq`; `pub fn from_tc(tc: &assets::tc::TcConfig) -> WObjectConsts`.
  - `SimState.wobject_consts: WObjectConsts` (default `WObjectConsts::default()`).
  - `wobject_process(…, game_mode: u32, settings_health: i32, consts: WObjectConsts, rand: &mut Rand) -> WObjectOutcome` (one new parameter before `rand`).

Why: design §4.4, §4.5, §4.9. The RemExp block (`weapon.cpp:137-142`) runs once per `Process`, before the do-loop; it reads the owner's control state. The TC hack is off, so it is unit-tested, not oracle-gated. `scenario::load` also gains the `laser_weapon` it never assigned.

- [ ] **Step 1: Write the failing tests**

(a) Append inside `mod tests` in `rust/sim/src/weapon.rs` (after the last test, before the closing `}` of the module):

```rust
    // ---- 4½c-0 T1: WObjectConsts + the RemExp hack (weapon.cpp:137-142) ----------

    #[test]
    fn wobject_consts_from_tc_reads_the_hack_and_the_three_constants() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc = TcConfig::load(&std::fs::read(format!("{root}/tc.cfg")).unwrap()).unwrap();
        assert_eq!(
            WObjectConsts::from_tc(&tc),
            WObjectConsts {
                h_rem_exp: false,
                rem_exp_object: 35,
                splinter_larpa_vel_div: 3,
                splinter_crackler_vel_div: 3,
            }
        );
    }

    // A BOOBY-TRAP-shaped timed weapon: flight knobs neutral, a long timer.
    fn rem_exp_weapon(id: i32) -> Weapon {
        Weapon {
            id,
            shot_type: ST_NORMAL,
            mult_speed: 100,
            gravity: 0,
            expl_ground: false,
            time_to_explo: 4000,
            obj_trail_type: -1,
            part_trail_obj: -1,
            ..Default::default()
        }
    }

    // HRemExp on; LC(RemExpObject) = 35 -> weapon id 34 (the TC's BOOBY TRAP).
    fn rem_exp_on() -> WObjectConsts {
        WObjectConsts {
            h_rem_exp: true,
            rem_exp_object: 35,
            ..WObjectConsts::default()
        }
    }

    // One Process on an air level with ONE worm — the owner, slot 0 — holding `controls`.
    fn proc_rem_exp(
        weapon: &Weapon,
        controls: u32,
        consts: WObjectConsts,
    ) -> (WObjectOutcome, WObject) {
        let mut level = air_level();
        let mut worms = [hit_worm(900, 900, Vec2::zero())];
        worms[0].control_states = ControlState::unpack(controls);
        let mut obj = WObject {
            pos: Vec2::new(itof(100), itof(100)),
            vel: Vec2::zero(),
            time_left: 100,
            ty: Some(weapon.id),
            owner_idx: 0,
            ..WObject::default()
        };
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut nobjects: Pool<NObject> = Pool::new(1);
        let mut sobjects: Pool<SObject> = Pool::new(1);
        let mut bonuses: Pool<Bonus> = Pool::new(1);
        let cossin = precompute_cossin();
        let mut rand = seeded();
        let out = wobject_process(
            &mut obj,
            &mut level,
            weapon,
            &[],
            0,
            &mut worms,
            &mut wobjects,
            &mut nobjects,
            &[],
            &mut sobjects,
            &[],
            &mut bonuses,
            &SpriteSet::default(),
            &SpriteSet::default(),
            &[],
            &cossin,
            100,
            0,
            100,
            consts,
            &mut rand,
        );
        (out, obj)
    }

    #[test]
    fn rem_exp_zeroes_the_timer_when_the_owner_holds_change_and_fire() {
        // Change (32) + Fire (16): time_left := 0, then the timeout (weapon.cpp:281-285)
        // `--time_left < 0` explodes it THIS tick.
        let (out, obj) = proc_rem_exp(&rem_exp_weapon(34), 32 | 16, rem_exp_on());
        assert_eq!(out, WObjectOutcome::Explode, "Change+Fire detonates the RemExp object");
        assert_eq!(obj.time_left, -1, "zeroed, then decremented past 0");
    }

    #[test]
    fn rem_exp_needs_the_hack_the_slot_and_both_keys() {
        // Fire alone, Change alone, the hack off, or another weapon: a plain countdown.
        for (weapon_id, controls, consts) in [
            (34, 16, rem_exp_on()),
            (34, 32, rem_exp_on()),
            (34, 32 | 16, WObjectConsts::default()),
            (33, 32 | 16, rem_exp_on()),
        ] {
            let (out, obj) = proc_rem_exp(&rem_exp_weapon(weapon_id), controls, consts);
            assert_eq!(out, WObjectOutcome::Keep, "weapon {weapon_id} controls {controls}");
            assert_eq!(obj.time_left, 99, "weapon {weapon_id} controls {controls}: countdown");
        }
    }
```

(b) In `rust/scenario/src/build.rs` test `build_match_maps_every_setting_onto_the_state`, directly after `assert_eq!(st.laser_weapon, tc.constants.LaserWeapon);` insert:

```rust
        assert_eq!(
            st.wobject_consts,
            sim::weapon::WObjectConsts::from_tc(&tc),
            "4½c-0: the WObject::Process TC consts"
        );
```

(c) Append inside `mod tests` in `rust/scenario/src/loader.rs`:

```rust
    #[test]
    fn load_assigns_the_wobject_consts_and_the_laser_weapon() {
        // Step 4½c-0 T1 (design §4.9): the game/`shot` path gets the WObject::Process TC
        // consts (a LARPA picked up live must not divide by zero) and `LC(LaserWeapon)`,
        // so the LASER's sight walk (worm.cpp:1196) arms. Both unhashed.
        let scenario = Scenario::parse(SAMPLE).expect("scenario parses");
        let loaded = load(Path::new(TC_ROOT), &scenario);
        let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
        assert_eq!(
            loaded.state.wobject_consts,
            sim::weapon::WObjectConsts::from_tc(&tc)
        );
        assert_eq!(loaded.state.wobject_consts.splinter_larpa_vel_div, 3);
        assert_eq!(loaded.state.laser_weapon, tc.constants.LaserWeapon);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib rem_exp`
Expected: FAIL to compile — `cannot find type WObjectConsts in this scope` (and `this function takes 20 arguments but 21 arguments were supplied`).

- [ ] **Step 3: Implement in `weapon.rs`**

(1) Replace `use assets::tc::Texture;` with `use assets::tc::{TcConfig, Texture};`.

(2) Directly after `const ST_LASER: i32 = 4;` insert:

```rust

/// The TC constants/hacks `WObject::Process` reads beyond the weapon table (Step 4½c-0,
/// design §4.4-§4.5): the RemExp hack and its object, and the two particle-trail velocity
/// divisors. Not hashed. `Default` is inert — RemExp off, both divisors 0, which are read
/// only when a `part_trail_obj >= 0` weapon flies — so a state that never assigns it (the
/// older oracle harnesses) behaves exactly as before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WObjectConsts {
    /// C++ `common.h[HRemExp]` (`weapon.cpp:138`; `tc.cfg [hacks] RemExp`).
    pub h_rem_exp: bool,
    /// C++ `LC(RemExpObject)` (`weapon.cpp:138`): the **1-based** weapon index of the
    /// remote-explode object (35 = BOOBY TRAP in the openliero TC).
    pub rem_exp_object: i32,
    /// C++ `LC(SplinterLarpaVelDiv)` (`weapon.cpp:203`): the type-1 particle-trail divisor.
    pub splinter_larpa_vel_div: i32,
    /// C++ `LC(SplinterCracklerVelDiv)` (`weapon.cpp:208`): the other particle-trail divisor.
    pub splinter_crackler_vel_div: i32,
}

impl WObjectConsts {
    /// The values the loaded TC carries (`tc.cfg [constants]` / `[hacks]`).
    pub fn from_tc(tc: &TcConfig) -> Self {
        WObjectConsts {
            h_rem_exp: tc.hacks.RemExp,
            rem_exp_object: tc.constants.RemExpObject,
            splinter_larpa_vel_div: tc.constants.SplinterLarpaVelDiv,
            splinter_crackler_vel_div: tc.constants.SplinterCracklerVelDiv,
        }
    }
}
```

(3) In the `wobject_process` definition replace

```rust
    game_mode: u32,
    settings_health: i32,
    rand: &mut Rand,
) -> WObjectOutcome {
```

with

```rust
    game_mode: u32,
    settings_health: i32,
    consts: WObjectConsts,
    rand: &mut Rand,
) -> WObjectOutcome {
    // weapon.cpp:137-142 — the RemExp hack (LIVE since 4½c-0 T1), once per Process call,
    // BEFORE the do-loop: with `h[HRemExp]` set, the `LC(RemExpObject)` weapon (1-based;
    // 35 = BOOBY TRAP) explodes the tick its owner holds Change AND Fire, by zeroing
    // `time_left` so the timeout below fires. Off in the openliero TC. Reads the owner's
    // control state as the object loop sees it (the top-of-tick input, design §4.3).
    if consts.h_rem_exp && weapon.id == consts.rem_exp_object - 1 {
        let owner = worms[obj.owner_idx as usize].control_states;
        if owner.get(ControlState::CHANGE) && owner.get(ControlState::FIRE) {
            obj.time_left = 0;
        }
    }

```

(4) Replace the doc lines

```rust
/// verdict. See the inline block for the exact RNG order. The `RemExp` early-explode block
/// (`weapon.cpp:138-142`, gated on the `HRemExp` hack AND the weapon being the
/// configurable `RemExpObject` LC slot) is likewise omitted: fan is not the
/// `RemExpObject` weapon, so it is inert here (differential-proven over 93 ticks);
/// port it when a slice exercises `RemExpObject`.
```

with

```rust
/// verdict. See the inline block for the exact RNG order. The `RemExp` early-explode block
/// (`weapon.cpp:137-142`) is LIVE since 4½c-0 T1, driven by [`WObjectConsts`]; the hack is
/// off in the openliero TC, so it is unit-tested only (design §4.4).
```

(5) Update the four test helpers: with the editor, replace ALL occurrences (`replace_all`) of

```rust
            100,
            rand,
        )
```

with

```rust
            100,
            WObjectConsts::default(),
            rand,
        )
```

Run: `grep -n "WObjectConsts::default()," /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/weapon.rs` — Expected: exactly 4 lines, inside `proc_no_worms`, `proc_hit`, `proc_trail`, `proc_impulse`.

- [ ] **Step 4: Implement in `state.rs`, the test literals and `process_frame_objects.rs`**

(1) `state.rs`: replace `use crate::weapon::{blow_up, wobject_process, worm_fire, WObjectOutcome};` with `use crate::weapon::{blow_up, wobject_process, worm_fire, WObjectConsts, WObjectOutcome};`.

(2) Replace `    pub laser_weapon: i32,` (the `SimState` field) with:

```rust
    pub laser_weapon: i32,
    /// The TC constants/hacks `WObject::Process` reads (Step 4½c-0: RemExp + the particle-
    /// trail divisors). **Not hashed.** Defaulted inert by `new`; assigned post-`new` by
    /// `scenario::build::build_match` and `scenario::load` (`WObjectConsts::from_tc`).
    pub wobject_consts: WObjectConsts,
```

(3) In `SimState::new` replace

```rust
            laser_weapon: 0,
            // Bonus-drop roll inputs
```

with

```rust
            laser_weapon: 0,
            // 4½c-0: the WObject::Process TC consts — inert Default (RemExp off, trail
            // divisors unread), assigned post-`new` by the builder / loader.
            wobject_consts: WObjectConsts::default(),
            // Bonus-drop roll inputs
```

(4) In the `process_frame` destructure replace `            laser_weapon,\n            settings_max_bonuses,` with `            laser_weapon,\n            wobject_consts,\n            settings_max_bonuses,`, and replace `        let laser_weapon = *laser_weapon;` with `        let laser_weapon = *laser_weapon;\n        let wobject_consts = *wobject_consts;`.

(5) In the wobjects driver's `wobject_process(` call replace

```rust
                blood,
                game_mode,
                settings_health,
                rand,
            ) {
```

with

```rust
                blood,
                game_mode,
                settings_health,
                wobject_consts,
                rand,
            ) {
```

(6) `hash.rs` and `wide_checksum.rs`: in each file replace `            laser_weapon: 0,` with `            laser_weapon: 0,\n            wobject_consts: crate::weapon::WObjectConsts::default(),`.

(7) `rust/sim/tests/process_frame_objects.rs`: replace

```rust
            0,
            100,
            &mut r2,
        ),
        WObjectOutcome::Explode,
```

with

```rust
            0,
            100,
            sim::weapon::WObjectConsts::default(),
            &mut r2,
        ),
        WObjectOutcome::Explode,
```

- [ ] **Step 5: Implement in `scenario`**

(1) `build.rs`: replace `    state.laser_weapon = tc.constants.LaserWeapon;` with

```rust
    state.laser_weapon = tc.constants.LaserWeapon;
    state.wobject_consts = sim::weapon::WObjectConsts::from_tc(&tc); // 4½c-0 (design §4.9)
```

(2) `loader.rs`: replace

```rust
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    state.game_mode = scenario.game_mode as u32;
```

with

```rust
    state.worm_min_spawn_dist_enemy = tc.constants.WormMinSpawnDistEnemy;
    // Step 4½c-0 (design §4.9): the WObject::Process TC consts and `LC(LaserWeapon)` —
    // both unhashed; no golden fires a trail weapon or holds the LASER.
    state.wobject_consts = sim::weapon::WObjectConsts::from_tc(&tc);
    state.laser_weapon = tc.constants.LaserWeapon;
    state.game_mode = scenario.game_mode as u32;
```

- [ ] **Step 6: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib rem_exp` — Expected: PASS (2 tests).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib wobject_consts_from_tc` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario` — Expected: PASS (incl. `load_assigns_the_wobject_consts_and_the_laser_weapon`, `build_match_maps_every_setting_onto_the_state`).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (every golden).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/weapon.rs rust/sim/src/state.rs rust/sim/src/hash.rs rust/sim/src/wide_checksum.rs rust/sim/tests/process_frame_objects.rs rust/scenario/src/build.rs rust/scenario/src/loader.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): WObjectConsts TC plumbing + the RemExp block; scenario::load assigns laser_weapon" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 2: The particle trail (LARPA, BOUNCY LARPA, CRACKLER)  [Opus]

**Files:**
- Modify: `rust/sim/src/weapon.rs` (import `:35`; the guard block at the top of `wobject_process` `:344-358`; the placeholder comment `:496-498`; the "Deferred / inert branches" doc `:295-299`; new tests)

**Interfaces:**
- Consumes: `crate::nobject::{nobject_create1, nobject_create2}` (`nobject.rs:139`, `:175`), `WObjectConsts` (T1).
- Produces: `wobject_process` spawns the particle trail; its signature is unchanged from T1.

Why: design §4.5. `weapon.cpp:201-210`, after the obj trail and before the collide loop: every `part_trail_delay` cycles (the pre-`++cycles` value the obj trail also uses), type 1 → `Create1(vel / SplinterLarpaVelDiv, pos, 0, owner)`; otherwise `rand(128)` then `Create2(angle, vel / SplinterCracklerVelDiv, pos, 0, owner)`. `vel` is the post-steering/bounce/`mult_speed` velocity, `pos` the fixed post-move position.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `weapon.rs`:

```rust
    // ---- 4½c-0 T2: the particle trail (weapon.cpp:201-210) --------------------------

    // A LARPA/CRACKLER-shaped flight: ST_NORMAL, no bounce/gravity/timeout, a particle
    // trail of nobject_types[0] every 4 cycles. part_trail_type 1 = LARPA (Create1),
    // anything else = CRACKLER (rand(128) angle, then Create2).
    fn part_trail_weapon(part_trail_type: i32) -> Weapon {
        Weapon {
            id: 11,
            shot_type: ST_NORMAL,
            mult_speed: 100,
            gravity: 0,
            expl_ground: false,
            time_to_explo: 0,
            obj_trail_type: -1,
            part_trail_obj: 0,
            part_trail_type,
            part_trail_delay: 4,
            ..Default::default()
        }
    }

    // A splinter type whose Create1 and Create2 both draw: distribution 8 (two scatter
    // draws), speed_v 50 (Create2's speed draw); start_frame 0 and time_to_explo_v 0, so
    // Create itself draws nothing.
    fn trail_particle() -> NObjectType {
        NObjectType {
            id: 0,
            speed: 100,
            speed_v: 50,
            distribution: 8,
            ..Default::default()
        }
    }

    fn trail_consts() -> WObjectConsts {
        WObjectConsts {
            splinter_larpa_vel_div: 3,
            splinter_crackler_vel_div: 3,
            ..WObjectConsts::default()
        }
    }

    fn trail_shooter() -> WObject {
        WObject {
            pos: Vec2::new(itof(100), itof(200)),
            vel: Vec2::new(itof(3), itof(-2)),
            owner_idx: 1,
            ty: Some(11),
            ..WObject::default()
        }
    }

    fn proc_part_trail(
        obj: &mut WObject,
        weapon: &Weapon,
        cycles: i32,
        nobjects: &mut Pool<NObject>,
        rand: &mut Rand,
    ) -> WObjectOutcome {
        let mut level = air_level();
        let mut worms: [WormState; 0] = [];
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut sobjects: Pool<SObject> = Pool::new(1);
        let mut bonuses: Pool<Bonus> = Pool::new(1);
        let cossin = precompute_cossin();
        wobject_process(
            obj,
            &mut level,
            weapon,
            &[],
            cycles,
            &mut worms,
            &mut wobjects,
            nobjects,
            &[trail_particle()],
            &mut sobjects,
            &[],
            &mut bonuses,
            &SpriteSet::default(),
            &SpriteSet::default(),
            &[],
            &cossin,
            100,
            0,
            100,
            trail_consts(),
            rand,
        )
    }

    #[test]
    fn larpa_trail_is_create1_of_vel_over_the_larpa_divisor_at_the_moved_pos() {
        // weapon.cpp:202-204: type 1 -> Create1(vel / SplinterLarpaVelDiv, pos, 0, owner)
        // after the tick's `pos += vel` — no angle draw.
        let mut obj = trail_shooter();
        let (pos, vel) = (obj.pos.add(obj.vel), obj.vel);
        let mut refr = seeded();
        let mut want: Pool<NObject> = Pool::new(8);
        nobject_create1(&trail_particle(), vel.div(3), pos, 0, 1, &mut refr, &mut want);

        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        let out = proc_part_trail(&mut obj, &part_trail_weapon(1), 8, &mut nobjects, &mut rand);
        assert_eq!(out, WObjectOutcome::Keep);
        assert_eq!(
            nobjects.iter().copied().collect::<Vec<_>>(),
            want.iter().copied().collect::<Vec<_>>(),
            "one Create1 particle"
        );
        assert_eq!(rand.last(), refr.last(), "Create1's two scatter draws, nothing else");
    }

    #[test]
    fn crackler_trail_draws_an_angle_then_create2_of_vel_over_the_crackler_divisor() {
        // weapon.cpp:205-209: rand(128) FIRST, then Create2(angle, vel /
        // SplinterCracklerVelDiv, pos, 0, owner).
        let cossin = precompute_cossin();
        let mut obj = trail_shooter();
        let (pos, vel) = (obj.pos.add(obj.vel), obj.vel);
        let mut refr = seeded();
        let mut want: Pool<NObject> = Pool::new(8);
        let angle = refr.bound(128) as i32;
        nobject_create2(&trail_particle(), angle, vel.div(3), pos, 0, 1, &cossin, &mut refr, &mut want);

        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        proc_part_trail(&mut obj, &part_trail_weapon(0), 8, &mut nobjects, &mut rand);
        assert_eq!(
            nobjects.iter().copied().collect::<Vec<_>>(),
            want.iter().copied().collect::<Vec<_>>(),
            "one Create2 particle"
        );
        assert_eq!(rand.last(), refr.last(), "rand(128), then Create2's three draws");
    }

    #[test]
    fn particle_trail_waits_for_its_delay() {
        let mut obj = trail_shooter();
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        let before = rand.last();
        proc_part_trail(&mut obj, &part_trail_weapon(0), 9, &mut nobjects, &mut rand);
        assert!(nobjects.is_empty(), "9 % 4 != 0 -> no particle");
        assert_eq!(rand.last(), before, "and no draw");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib trail_`
Expected: FAIL to compile first (`cannot find function nobject_create1`); after adding only the import (Step 3 (1)) the two spawning tests panic with `particle-trail spawn deferred (no fired weapon uses it)`.

- [ ] **Step 3: Implement**

(1) Replace `use crate::nobject::{check_for_spec_worm_hit, nobject_create2};` with `use crate::nobject::{check_for_spec_worm_hit, nobject_create1, nobject_create2};`.

(2) Replace the guard block

```rust
    // Deferred-branch guards. shot_type 0/1/2/3 are all ported now: ST_NORMAL/
    // ST_TYPE1 share the plain flight; ST_STEERABLE (2) and ST_TYPE2 (3, e.g.
    // BAZOOKA) run the steering block below. Only the laser do-loop (shot_type 4)
    // stays deferred. `mult_speed` and the `obj_trail` spawn are LIVE (T7b); the
    // particle-trail spawn stays deferred (no fired weapon in this TC uses it —
    // bazooka's part_trail_obj = -1). A config that would take an un-ported branch
    // fails loudly in debug builds.
    debug_assert!(
        weapon.shot_type != ST_LASER,
        "laser do-loop Process branch deferred"
    );
    debug_assert!(
        weapon.part_trail_obj < 0,
        "particle-trail spawn deferred (no fired weapon uses it)"
    );
```

with

```rust
    // Deferred-branch guard. shot_type 0/1/2/3 are ported (ST_NORMAL/ST_TYPE1 share the
    // plain flight; ST_STEERABLE and ST_TYPE2 run the steering block below), and so are
    // `mult_speed` and both trails (the particle trail since 4½c-0 T2). Only the laser
    // do-loop (shot_type 4) stays deferred, until 4½c-0 T3.
    debug_assert!(
        weapon.shot_type != ST_LASER,
        "laser do-loop Process branch deferred"
    );
```

(3) Replace the placeholder

```rust
    // The particle-trail spawn (weapon.cpp:201-210) goes here in C++; deferred (no
    // fired weapon in this TC has part_trail_obj >= 0 — bazooka = -1; guarded above).
    // The worm-hit loop (weapon.cpp:287-326) is AFTER the timeout, below.
```

with

```rust
    // Particle trail (weapon.cpp:201-210) — LIVE (4½c-0 T2): LARPA / BOUNCY LARPA
    // (`part_trail_type == 1` -> Create1 of `vel / SplinterLarpaVelDiv`) and CRACKLER
    // (else -> a `rand(128)` angle FIRST, then Create2 of `vel / SplinterCracklerVelDiv`).
    // Same pre-`++cycles` gate as the obj trail; `vel` is the post-steering/bounce/
    // mult_speed velocity, `pos` the FIXED post-move position (no Ftoi), colour 0, owner =
    // this wobject's owner. The divisions truncate (`Vec2::div` == C++ `fixedvec / int`).
    // `part_trail_delay > 0` for every trail weapon (`weapon_branch_inventory.rs`).
    // The worm-hit loop (weapon.cpp:287-326) is AFTER the timeout, below.
    if weapon.part_trail_obj >= 0 && cycles % weapon.part_trail_delay == 0 {
        let trail = &nobject_types[weapon.part_trail_obj as usize];
        if weapon.part_trail_type == 1 {
            nobject_create1(
                trail,
                obj.vel.div(consts.splinter_larpa_vel_div),
                obj.pos,
                0,
                obj.owner_idx,
                rand,
                nobjects,
            );
        } else {
            let angle = rand.bound(128) as i32;
            nobject_create2(
                trail,
                angle,
                obj.vel.div(consts.splinter_crackler_vel_div),
                obj.pos,
                0,
                obj.owner_idx,
                cossin,
                rand,
                nobjects,
            );
        }
    }
```

(4) Replace the doc lines

```rust
/// Deferred / inert branches (guarded by `debug_assert!` so a non-fan config
/// trips loudly, or omitted because they need state the driver owns):
/// steering (`shot_type` 2/3) and the laser do-loop, `mult_speed`, and
/// object/particle trails are all `debug_assert`ed to their fan-shaped no-op
/// values.
```

with

```rust
/// Steering (`shot_type` 2/3), `mult_speed` and both trails (the particle trail since
/// 4½c-0 T2) are LIVE; the laser do-loop is still `debug_assert`ed off (4½c-0 T3).
```

- [ ] **Step 4: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib trail_` — Expected: PASS (the 3 new tests + the existing obj-trail tests).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/weapon.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): port the WObject particle trail (LARPA/BOUNCY LARPA Create1, CRACKLER Create2)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 3: The `ST_LASER` do-loop (RIFLE, WINCHESTER, GAUSS GUN, LASER)  [Opus]

**Files:**
- Modify: `rust/sim/src/weapon.rs` (the `wobject_process` definition becomes `wobject_pass`; a new `wobject_process` + two consts above its doc; the RemExp block and the laser guard move/go; doc header `:267-269`; the `do { … }` comment `:362-363`; new tests)

**Interfaces:**
- Produces: `pub fn wobject_process(…)` — the same 21-parameter signature as after T1 — now the do-loop; `fn wobject_pass(…)` (private, same parameters) is one step of it. Callers (the driver, the tests) are unchanged.

Why: design §4.1. C++ re-runs the whole body `while (w.shot_type == kStLaser && used && (iter < 8 || w.id == 28))` (`weapon.cpp:336-337`) and `break`s right after `BlowUpObject`/`Free` (`:328-335`), exactly where the body returns Explode/Remove. RemExp stays once per call, before the loop.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `weapon.rs`:

```rust
    // ---- 4½c-0 T3: the ST_LASER do-loop (weapon.cpp:144, :336-337) ------------------

    fn st_laser_weapon(id: i32, gravity: i32) -> Weapon {
        Weapon {
            id,
            shot_type: ST_LASER,
            bounce: 0,
            mult_speed: 100,
            gravity,
            expl_ground: true,
            time_to_explo: 0,
            num_frames: 0,
            obj_trail_type: -1,
            part_trail_obj: -1,
            ..Default::default()
        }
    }

    fn beam(id: i32, px: i32, py: i32) -> WObject {
        WObject {
            pos: Vec2::new(itof(px), itof(py)),
            vel: Vec2::new(itof(1), 0),
            owner_idx: 1,
            ty: Some(id),
            ..WObject::default()
        }
    }

    #[test]
    fn st_laser_steps_eight_times_per_tick() {
        // The whole body re-runs until iter == 8. Free air + gravity 20: each step moves by
        // the running vel, then adds 20 to vel.y -> after 8 steps x += 8 px, vel.y == 160,
        // y += 20 * (0+1+..+7) = 560 (fixed units).
        let level = air_level();
        let mut rand = seeded();
        let mut obj = beam(2, 100, 500);
        let out = proc_no_worms(&mut obj, &level, &st_laser_weapon(2, 20), 0, &mut rand);
        assert_eq!(out, WObjectOutcome::Keep, "free air: survives the tick");
        assert_eq!(obj.pos.x, itof(108), "8 one-pixel steps");
        assert_eq!(obj.vel, Vec2::new(itof(1), 8 * 20), "8 air steps of gravity");
        assert_eq!(obj.pos.y, itof(500) + 20 * 28, "the sum of the running vel.y");

        // Contrast: the same flight as ST_NORMAL is ONE step.
        let mut one = beam(2, 100, 500);
        let normal = Weapon {
            shot_type: ST_NORMAL,
            ..st_laser_weapon(2, 20)
        };
        proc_no_worms(&mut one, &level, &normal, 0, &mut rand);
        assert_eq!((one.pos.x, one.vel.y), (itof(101), 20));
    }

    #[test]
    fn the_laser_slot_28_steps_until_it_explodes() {
        // `|| w.id == 28`: the LASER does not stop after 8 steps. On the 1000-px air level
        // it walks from x = 100 to the edge in ONE tick: at x = 999 the next cell is outside,
        // the clamp pins x = 999 and `!Inside` explodes it (explGround).
        let level = air_level();
        let mut rand = seeded();
        let mut obj = beam(28, 100, 500);
        let out = proc_no_worms(&mut obj, &level, &st_laser_weapon(28, 0), 0, &mut rand);
        assert_eq!(out, WObjectOutcome::Explode, "explodes at the level edge this tick");
        assert_eq!(obj.pos.x, itof(999), "walked 899 steps, not 8");
    }

    #[test]
    fn st_laser_explodes_on_the_step_that_meets_rock() {
        // floor_level: rock at (10,10) only. Step 1: pos 7, next 8; step 2: pos 8, next 9;
        // step 3: pos 9, next 10 = rock -> explode with pos 9 (the loop breaks there).
        let level = floor_level();
        let mut rand = seeded();
        let mut obj = beam(2, 6, 10);
        let out = proc_no_worms(&mut obj, &level, &st_laser_weapon(2, 0), 0, &mut rand);
        assert_eq!(out, WObjectOutcome::Explode);
        assert_eq!(obj.pos, Vec2::new(itof(9), itof(10)), "stopped on the third step");
    }

    #[test]
    fn st_laser_hits_a_worm_on_a_later_step_and_is_removed() {
        // The worm-hit arm (weapon.cpp:287-326) runs on EVERY step. The fixture's only solid
        // worm pixel is sprite (8,8) -> world (51,53) for a worm at (50,50). From x = 47 the
        // beam reaches x = 51 on step 4: hit -> DoDamage(1) -> worm_collide -> Remove; steps
        // 5..8 never run.
        let cossin = precompute_cossin();
        let (worm_sprites, flags) = worm_hit_sprites();
        let level = hit_level(flags);
        let w = Weapon {
            hit_damage: 1,
            worm_collide: true,
            detect_distance: 0,
            ..st_laser_weapon(2, 0)
        };
        let nobject_types = blood_types();
        let mut worms = [hit_worm(50, 50, Vec2::zero())];
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut rand = seeded();
        let mut obj = beam(2, 47, 53);
        let out = proc_hit(
            &mut obj,
            &level,
            &w,
            &mut worms,
            &mut nobjects,
            &nobject_types,
            &worm_sprites,
            &cossin,
            100,
            &mut rand,
        );
        assert_eq!(out, WObjectOutcome::Remove, "worm_collide without worm_explode");
        assert_eq!(obj.pos.x, itof(51), "removed on step 4");
        assert_eq!(worms[0].health, 99, "one hit, not one per remaining step");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib st_laser`
Expected: FAIL — each panics `laser do-loop Process branch deferred` (debug build).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib the_laser_slot_28` — Expected: FAIL, same panic.

- [ ] **Step 3: Implement**

(1) In the current `wobject_process` definition, replace the RemExp block and the guard (the text from T1 + T2):

```rust
) -> WObjectOutcome {
    // weapon.cpp:137-142 — the RemExp hack (LIVE since 4½c-0 T1), once per Process call,
    // BEFORE the do-loop: with `h[HRemExp]` set, the `LC(RemExpObject)` weapon (1-based;
    // 35 = BOOBY TRAP) explodes the tick its owner holds Change AND Fire, by zeroing
    // `time_left` so the timeout below fires. Off in the openliero TC. Reads the owner's
    // control state as the object loop sees it (the top-of-tick input, design §4.3).
    if consts.h_rem_exp && weapon.id == consts.rem_exp_object - 1 {
        let owner = worms[obj.owner_idx as usize].control_states;
        if owner.get(ControlState::CHANGE) && owner.get(ControlState::FIRE) {
            obj.time_left = 0;
        }
    }

    // Deferred-branch guard. shot_type 0/1/2/3 are ported (ST_NORMAL/ST_TYPE1 share the
    // plain flight; ST_STEERABLE and ST_TYPE2 run the steering block below), and so are
    // `mult_speed` and both trails (the particle trail since 4½c-0 T2). Only the laser
    // do-loop (shot_type 4) stays deferred, until 4½c-0 T3.
    debug_assert!(
        weapon.shot_type != ST_LASER,
        "laser do-loop Process branch deferred"
    );

    let mut do_explode = false;

    // do { ... } while (shot_type == kStLaser && ...): fan is not a laser, so the
    // body runs exactly once.
```

with

```rust
) -> WObjectOutcome {
    let mut do_explode = false;

    // ONE step of the C++ do-loop body (weapon.cpp:145-335); [`wobject_process`] owns the
    // `while (shot_type == kStLaser && used && (iter < 8 || id == 28))` repeat.
```

(2) In that same definition rename `pub fn wobject_process(` to `fn wobject_pass(` (only the definition line; the `#[allow(clippy::too_many_arguments)]` above it stays).

(3) Replace the first three doc lines above it

```rust
/// Port of the single non-laser pass of `WObject::Process` (`weapon.cpp:127-338`)
/// for the **fan** (ST_NORMAL) and **dart** (ST_TYPE1) projectile shapes — the two
/// share an identical per-tick flight (see the `shot_type` guard below).
```

with

```rust
/// One step of `WObject::Process` — the do-loop body (`weapon.cpp:145-335`).
/// [`wobject_process`] runs it once per tick for every shot type but `ST_LASER`, which it
/// repeats (4½c-0 T3). Written first for the **fan** (ST_NORMAL) and **dart** (ST_TYPE1).
```

and replace

```rust
/// Steering (`shot_type` 2/3), `mult_speed` and both trails (the particle trail since
/// 4½c-0 T2) are LIVE; the laser do-loop is still `debug_assert`ed off (4½c-0 T3).
```

with

```rust
/// Steering (`shot_type` 2/3), `mult_speed` and both trails (the particle trail since
/// 4½c-0 T2) are LIVE; the laser do-loop is [`wobject_process`] (4½c-0 T3).
```

(4) Directly above the `/// One step of \`WObject::Process\`` doc line insert:

```rust
/// `weapon.cpp:336-337` `iter < 8`: a non-LASER `ST_LASER` wobject re-runs the Process
/// body at most 8 times per tick.
const LASER_MAX_ITERATIONS: i32 = 8;
/// `weapon.cpp:337` `|| w.id == 28`: the original Liero LASER slot re-runs the body until it
/// explodes or is removed. In the openliero TC weapon 28 IS the LASER (`tc.cfg [types]
/// weapons`, `common.cpp:495` id = index; pinned by `weapon_branch_inventory.rs`).
const LASER_UNBOUNDED_WEAPON_ID: i32 = 28;

/// Port of `WObject::Process` (`weapon.cpp:127-338`): the once-per-call RemExp block
/// (`:137-142`), then `do { ++iter; <body> } while (w.shot_type == kStLaser && used &&
/// (iter < 8 || w.id == 28));` (`:144`, `:336-337`) around [`wobject_pass`], the body.
///
/// Every non-`ST_LASER` weapon runs the body once. An `ST_LASER` wobject (RIFLE,
/// WINCHESTER, GAUSS GUN, LASER) runs the WHOLE body — move, clamp, ground test, gravity,
/// timer, the in-flight worm-hit arm — up to 8 times per tick, the LASER (id 28) without
/// limit, and stops on the step that explodes or removes it: C++ `break`s right after
/// `BlowUpObject`/`Free` (`:328-335`), exactly where the body returns Explode/Remove. C++'s
/// `used` is therefore always true at the loop check: nothing else in an `ST_LASER` body
/// can free `this` (no `ST_LASER` weapon spawns a trail — `weapon_branch_inventory.rs`).
/// No iteration cap, because C++ has none; the LASER (`speed 100`) always progresses
/// toward an edge, where `!Inside` explodes it.
#[allow(clippy::too_many_arguments)]
pub fn wobject_process(
    obj: &mut WObject,
    level: &mut LevelSim,
    weapon: &Weapon,
    weapons: &[Weapon],
    cycles: i32,
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    nobjects: &mut Pool<NObject>,
    nobject_types: &[NObjectType],
    sobjects: &mut Pool<SObject>,
    sobject_types: &[SObjectType],
    bonuses: &mut Pool<Bonus>,
    worm_sprites: &SpriteSet,
    large_sprites: &SpriteSet,
    textures: &[Texture],
    cossin: &[Vec2; 128],
    blood: i32,
    game_mode: u32,
    settings_health: i32,
    consts: WObjectConsts,
    rand: &mut Rand,
) -> WObjectOutcome {
    // weapon.cpp:137-142 — the RemExp hack (4½c-0 T1), once per Process call, BEFORE the
    // do-loop: with `h[HRemExp]` set, the `LC(RemExpObject)` weapon (1-based; 35 = BOOBY
    // TRAP) explodes the tick its owner holds Change AND Fire, by zeroing `time_left` so
    // the body's timeout fires. Off in the openliero TC. Reads the owner's control state
    // as the object loop sees it (the top-of-tick input, design §4.3).
    if consts.h_rem_exp && weapon.id == consts.rem_exp_object - 1 {
        let owner = worms[obj.owner_idx as usize].control_states;
        if owner.get(ControlState::CHANGE) && owner.get(ControlState::FIRE) {
            obj.time_left = 0;
        }
    }

    let mut iter = 0;
    loop {
        iter += 1;
        let out = wobject_pass(
            obj,
            level,
            weapon,
            weapons,
            cycles,
            worms,
            wobjects,
            nobjects,
            nobject_types,
            sobjects,
            sobject_types,
            bonuses,
            worm_sprites,
            large_sprites,
            textures,
            cossin,
            blood,
            game_mode,
            settings_health,
            consts,
            rand,
        );
        // weapon.cpp:328-335: BlowUpObject / Free, then `break`.
        if out != WObjectOutcome::Keep {
            return out;
        }
        // weapon.cpp:336-337.
        if !(weapon.shot_type == ST_LASER
            && (iter < LASER_MAX_ITERATIONS || weapon.id == LASER_UNBOUNDED_WEAPON_ID))
        {
            return WObjectOutcome::Keep;
        }
    }
}

```

- [ ] **Step 4: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib st_laser` — Expected: PASS (3 tests).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib the_laser_slot_28` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib rem_exp` — Expected: PASS (the moved RemExp block).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/weapon.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): port the ST_LASER do-loop (8 steps; unbounded for weapon 28 = LASER)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 4: Splinters via `Create1` (MINI NUKE) — `blow_up` gains `vel`  [Sonnet]

**Files:**
- Modify: `rust/sim/src/weapon.rs` (`blow_up` signature `:774-794` and doc `:769-772`; the O18 guard `:853-863`; the seven test calls `:2220`, `:2296`, `:2485`, `:2579`, `:2667`, `:2732`, and the `#[should_panic]` test `:2758-2797`, replaced)
- Modify: `rust/sim/src/state.rs` (the driver's `blow_up(` call in the Explode arm)
- Modify: `rust/sim/tests/process_frame_objects.rs` (`:393-413`)

**Interfaces:**
- Produces (used by T6): `pub fn blow_up(weapon: &Weapon, level: &mut LevelSim, large_sprites: &SpriteSet, textures: &[Texture], pos: Vec2, vel: Vec2, owner_idx: i32, sobject_types: &[SObjectType], nobject_types: &[NObjectType], cossin: &[Vec2; 128], worms: &mut [WormState], wobjects: &mut Pool<WObject>, weapons: &[Weapon], nobjects: &mut Pool<NObject>, sobjects: &mut Pool<SObject>, bonuses: &mut Pool<Bonus>, blood: i32, game_mode: u32, settings_health: i32, rand: &mut Rand)` — `vel` is new, after `pos`.

Why: design §4.6. `weapon.cpp:107-114`: per splinter `rand(2)` [`kColorSub`] then `Create1(fixedvec(kVelX, kVelY), fixedvec(kX, kY), splinter_colour - kColorSub, cause)` — the exploding wobject's own velocity, no angle.

- [ ] **Step 1: Write the failing test** — in `weapon.rs` replace the whole `#[should_panic(expected = "Create1 branch")]` test `splinter_scatter_nonzero_trips_the_guarded_create1_branch` (from its `#[test]` line through its closing `}`) with:

```rust
    #[test]
    fn splinter_scatter_nonzero_spawns_create1_splinters_carrying_the_wobject_vel() {
        // weapon.cpp:107-114 (MINI NUKE, scatter 1): per splinter rand(2) [kColorSub] THEN
        // nobject_types[splinter_type].Create1(fixedvec(kVelX, kVelY), fixedvec(kX, kY),
        // splinter_colour - kColorSub, cause) — the exploding wobject's OWN vel, no angle.
        let cossin = precompute_cossin();
        let weapon = splinter_weapon(2, 1);
        let nobject_types = vec![dirt_nobject()];
        let pos = Vec2::new(itof(50), itof(60));
        let vel = Vec2::new(itof(2), itof(-1));

        let mut refr = seeded();
        let mut want: Pool<NObject> = Pool::new(8);
        for _ in 0..2 {
            let sub = refr.bound(2) as i32;
            nobject_create1(&nobject_types[0], vel, pos, 80 - sub, 1, &mut refr, &mut want);
        }

        let mut rand = seeded();
        let mut level = air_level();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(8);
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut sobjects: Pool<SObject> = Pool::new(8);
        blow_up(
            &weapon,
            &mut level,
            &SpriteSet::default(),
            &[],
            pos,
            vel,
            1,
            &[],
            &nobject_types,
            &cossin,
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            &mut sobjects,
            &mut Pool::<Bonus>::new(1),
            100,
            0,
            100,
            &mut rand,
        );

        assert_eq!(
            nobjects.iter().copied().collect::<Vec<_>>(),
            want.iter().copied().collect::<Vec<_>>(),
            "two Create1 splinters at the fixed pos with the wobject vel"
        );
        assert_eq!(rand.last(), refr.last(), "rand(2) then Create1's draws, per splinter");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib splinter_scatter_nonzero`
Expected: FAIL to compile — `this function takes 19 arguments but 20 arguments were supplied`.

- [ ] **Step 3: Implement**

(1) In the `blow_up` definition replace

```rust
    textures: &[Texture],
    pos: Vec2,
    owner_idx: i32,
    sobject_types: &[SObjectType],
```

with

```rust
    textures: &[Texture],
    pos: Vec2,
    vel: Vec2,
    owner_idx: i32,
    sobject_types: &[SObjectType],
```

(2) Replace the guard

```rust
        } else {
            // :107-114 scatter != 0 -> the C++ Create1 splinter branch. GUARDED
            // (O18): no weapon in this TC takes it (only mini_nuke has scatter=1,
            // with the special small_nukes type, out of scope), AND blow_up has no
            // access to the wobject velocity that C++ Create1 needs (`fixedvec(
            // kVelX, kVelY)`). A config that would hit it trips loudly here.
            debug_assert!(
                false,
                "splinter_scatter != 0 (Create1 branch) deferred (O18): needs wobject vel"
            );
        }
```

with

```rust
        } else {
            // :107-114 scatter != 0 (MINI NUKE) — LIVE (4½c-0 T4): per splinter
            // rand(2) [kColorSub] THEN nobject_types[splinter_type].Create1 with the
            // exploding wobject's OWN vel (`fixedvec(kVelX, kVelY)`, captured before the
            // Free), the FIXED pos and colour `splinter_colour - kColorSub`. No angle
            // draw; Create1's own scatter draws follow (`nobject.rs` nobject_create1).
            for _ in 0..weapon.splinter_amount {
                let color_sub = rand.bound(2) as i32;
                nobject_create1(
                    &nobject_types[weapon.splinter_type as usize],
                    vel,
                    pos,
                    weapon.splinter_colour - color_sub,
                    owner_idx,
                    rand,
                    nobjects,
                );
            }
        }
```

(3) Replace the doc lines

```rust
/// the dart's own `dirt_effect` — the C++ order is load-bearing because each
/// `Create2` draws its RNG between the two. The `scatter != 0` sub-branch (the
/// C++ `Create1` path) is **guarded** (O18): no weapon in this TC takes it (only
/// `mini_nuke` has `scatter=1`, out of scope) and `blow_up` has no access to the
/// wobject velocity `Create1` needs, so a config that would hit it trips loudly.
```

with

```rust
/// the dart's own `dirt_effect` — the C++ order is load-bearing because each
/// `Create2` draws its RNG between the two. The `scatter != 0` sub-branch (the C++
/// `Create1` path, MINI NUKE) is LIVE since 4½c-0 T4: it spawns `Create1` splinters with
/// the exploding wobject's `vel` (the new parameter; the driver passes `obj.vel`).
```

(4) In each of the six remaining `blow_up(` calls in the test module (at about `:2220`, `:2296`, `:2485`, `:2579`, `:2667`, `:2732`), insert a new argument line `            Vec2::zero(),` directly after the position argument (the fifth argument: `pos,` or `Vec2::new(itof(50), itof(50)),`) and before the owner argument.

(5) `state.rs`, the driver's Explode arm: replace

```rust
                        textures,
                        obj.pos,
                        obj.owner_idx,
```

with

```rust
                        textures,
                        obj.pos,
                        obj.vel,
                        obj.owner_idx,
```

(6) `rust/sim/tests/process_frame_objects.rs`: replace

```rust
        obj.pos,
        0, // fired by worm 0
```

with

```rust
        obj.pos,
        obj.vel,
        0, // fired by worm 0
```

- [ ] **Step 4: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib splinter_` — Expected: PASS (the new test + the scatter-0 and amount-0 tests).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/weapon.rs rust/sim/src/state.rs rust/sim/tests/process_frame_objects.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): port BlowUpObject's Create1 splinter scatter (MINI NUKE); blow_up takes the wobject vel" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 5: The nobject `leave_obj` trail (MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER)  [Opus]

**Files:**
- Modify: `rust/sim/src/nobject.rs` (the guard `:501-509`; the doc bullet `:331-335`; new tests)

**Interfaces:**
- Consumes: `crate::sobject::sobject_create` (already imported, `nobject.rs:64`).
- Produces: `nobject_process` spawns the `leave_obj` sobject; signature unchanged.

Why: design §4.7. `nobject.cpp:133-138`, free-air branch, before gravity: `if (!bounced && leave_obj_delay != 0 && leave_obj >= 0 && cycles % leave_obj_delay == 0) sobject_types[leave_obj].Create(Ftoi(pos.x), Ftoi(pos.y), owner_idx, fired_by)`. The driver's stale copy of this nobject is nudged at its pre-move position and then overwritten, while C++ nudges `this` at delta 0 — a no-op: equal.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `nobject.rs`:

```rust
    // ---- 4½c-0 T5: the leave_obj sobject trail (nobject.cpp:133-138) ----------------

    // A NAPALM-fireball-shaped type: flies (no ground explode, no gravity) and leaves
    // sobject_types[0] every 4 cycles; `bounce` per test.
    fn trail_nobject(bounce: i32) -> NObjectType {
        NObjectType {
            id: 12,
            expl_ground: false,
            bounce,
            gravity: 0,
            leave_obj: 0,
            leave_obj_delay: 4,
            num_frames: 0,
            hit_damage: 0,
            time_to_explo: 0,
            create_on_exp: -1,
            dirt_effect: -1,
            splinter_amount: 0,
            ..Default::default()
        }
    }

    fn run_leave_obj(
        obj: &mut NObject,
        ty: &NObjectType,
        level: &mut LevelSim,
        cycles: i32,
        sobjects: &mut Pool<SObject>,
    ) -> NObjectOutcome {
        let cossin = precompute_cossin();
        let sprites = no_sprites();
        let mut worms: Vec<WormState> = Vec::new();
        let mut wobjects: Pool<WObject> = Pool::new(1);
        let mut nobjects: Pool<NObject> = Pool::new(8);
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        let mut rand = seeded();
        nobject_process(
            obj,
            ty,
            &[],
            &[inert_sobject(0)],
            level,
            &cossin,
            &sprites,
            &sprites,
            &sprites,
            &[],
            &mut worms,
            &mut wobjects,
            &[],
            &mut nobjects,
            sobjects,
            &mut Pool::<Bonus>::new(1),
            &mut bobjects,
            cycles,
            100,
            0,
            0,
            0,
            100,
            &mut rand,
        )
    }

    fn fireball() -> NObject {
        NObject {
            pos: Vec2::new(itof(50), itof(50)),
            vel: Vec2::new(itof(1), 0),
            ty: Some(12),
            owner_idx: 1,
            ..Default::default()
        }
    }

    #[test]
    fn leave_obj_spawns_its_sobject_at_the_moved_pos_on_the_delay_cycle() {
        // In free air, on `cycles % leave_obj_delay == 0`: sobject_types[leave_obj].Create
        // at Ftoi(post-move pos) — the `-8` centre->top-left offset is sobject_create's.
        let mut sobjects: Pool<SObject> = Pool::new(8);
        let mut obj = fireball();
        let out = run_leave_obj(&mut obj, &trail_nobject(0), &mut bg_level(100, 100), 8, &mut sobjects);
        assert_eq!(out, NObjectOutcome::Keep);
        assert_eq!(sobjects.len(), 1, "one trail sobject");
        let s = *sobjects.get(0).expect("trail sobject in slot 0");
        assert_eq!((s.id, s.x, s.y), (0, 51 - 8, 50 - 8), "at Ftoi(post-move pos) - 8");
    }

    #[test]
    fn leave_obj_waits_for_its_delay_and_skips_a_bounce_tick() {
        let mut sobjects: Pool<SObject> = Pool::new(8);
        // Off the delay cycle: nothing.
        let mut obj = fireball();
        run_leave_obj(&mut obj, &trail_nobject(0), &mut bg_level(100, 100), 9, &mut sobjects);
        assert!(sobjects.is_empty(), "9 % 4 != 0 -> no trail");
        // A bounce this tick suppresses it (`!bounced`): rock from x = 52 — after the move
        // to x = 51 the x-probe at 52 reflects vel.x.
        let mut obj = fireball();
        let mut level = level_with_wall(100, 100, 52);
        run_leave_obj(&mut obj, &trail_nobject(50), &mut level, 8, &mut sobjects);
        assert!(obj.vel.x < 0, "it did bounce");
        assert!(sobjects.is_empty(), "bounced -> no trail");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib leave_obj_`
Expected: FAIL — both panic `leave_obj sobject trail deferred (needs SObject Create)`.

- [ ] **Step 3: Implement** — in `nobject_process` replace

```rust
    } else {
        // :133-138 leave_obj sobject trail — deferred (needs SObject Create).
        // C++ gate is `!bounced && leave_obj_delay != 0 && leave_obj >= 0 && ...`;
        // the assert reproduces that gate so `bounced` is load-bearing (the trail
        // is suppressed right after a bounce). Inert for the dirt particle
        // (leave_obj=-1).
        debug_assert!(
            bounced || ty.leave_obj < 0 || ty.leave_obj_delay == 0,
            "leave_obj sobject trail deferred (needs SObject Create)"
        );
        // :140 vel.y += gravity.
```

with

```rust
    } else {
        // :133-138 leave_obj sobject trail — LIVE (4½c-0 T5): napalm fireballs, small /
        // large nukes and hellraider bullets. Gate `!bounced && leave_obj_delay != 0 &&
        // leave_obj >= 0 && cycles % leave_obj_delay == 0` (`bounced` is load-bearing: no
        // trail right after a bounce); `cycles` is the pre-`++cycles` snapshot. Spawns at
        // Ftoi of the post-move (post-clamp) pos. The driver's stale copy of THIS nobject
        // may be nudged by the trail blast's nobject loop and is then overwritten on Keep;
        // C++ nudges `this`, whose delta to the blast is 0 — a no-op. Equal.
        if !bounced
            && ty.leave_obj_delay != 0
            && ty.leave_obj >= 0
            && cycles % ty.leave_obj_delay == 0
        {
            sobject_create(
                &sobject_types[ty.leave_obj as usize],
                ftoi(obj.pos.x),
                ftoi(obj.pos.y),
                obj.owner_idx,
                worms,
                wobjects,
                weapons,
                nobjects,
                nobject_types,
                level,
                cossin,
                large_sprites,
                textures,
                sobjects,
                bonuses,
                sobject_types,
                blood,
                game_mode,
                settings_health,
                rand,
            );
        }
        // :140 vel.y += gravity.
```

and replace the doc lines

```rust
///   `wobject_process`. The `BlitImageOnMap`-on-ground arm (`:119-128`, gated
///   `start_frame > 0 && draw_on_map`) and the `leave_obj` sobject trail
///   (`:133-138`) are `debug_assert!`ed off (need sprite-blit / sobject Create).
```

with

```rust
///   `wobject_process`. The `BlitImageOnMap`-on-ground arm (`:119-128`, gated
///   `start_frame > 0 && draw_on_map`) is LIVE (4d) and so is the `leave_obj` sobject
///   trail (`:133-138`, 4½c-0 T5).
```

- [ ] **Step 4: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib leave_obj_` — Expected: PASS (2 tests).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/nobject.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): port the NObject leave_obj sobject trail (napalm, nukes, hellraider bullets)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 6: Chain explosions (BOOBY TRAP) + free-before-explode in the driver  [Opus]

**Files:**
- Modify: `rust/sim/src/sobject.rs` (the wobjects blow-away loop `:282-320`; module doc `:25-27` and fn doc `:104`; new tests)
- Modify: `rust/sim/src/state.rs` (the wobjects driver's Explode arm; a new test)

**Interfaces:**
- Consumes: `crate::weapon::blow_up` with T4's `vel` parameter.
- Produces: `sobject_create` blows up in-range `chain_explosion` wobjects (recursively); the driver frees an exploding wobject before `blow_up`.

Why: design §4.8. `sobject.cpp:148-150` calls `BlowUpObject(game, owner_idx)` on every in-box `chain_explosion` wobject after nudging it; `BlowUpObject` frees `this` FIRST (`weapon.cpp:87`). The driver must free first as well, or an exploding BOOBY TRAP's own blast finds its stale slot and chains it again.

- [ ] **Step 1: Write the failing tests**

(a) Append inside `mod tests` in `sobject.rs`:

```rust
    // ---- 4½c-0 T6: chain explosions (sobject.cpp:148-150) ----------------------------

    // weapons[0]: a BOOBY-TRAP-shaped mine — affected by explosions, chain-exploding, its
    // own blast sobject_types[1]; weapons[1]: affected but NOT chaining.
    fn chain_weapons() -> Vec<Weapon> {
        vec![
            Weapon {
                id: 0,
                affect_by_explosions: true,
                chain_explosion: true,
                create_on_exp: 1,
                dirt_effect: -1,
                splinter_amount: 0,
                ..Default::default()
            },
            Weapon {
                id: 1,
                affect_by_explosions: true,
                chain_explosion: false,
                create_on_exp: 1,
                dirt_effect: -1,
                splinter_amount: 0,
                ..Default::default()
            },
        ]
    }

    // A silent, no-carve blast with damage > 0 (so ITS wobject loop runs too) and a ±10 box.
    fn chain_blast(id: i32) -> SObjectType {
        SObjectType {
            id,
            start_sound: -1,
            num_sounds: 0,
            anim_delay: 3,
            num_frames: 4,
            detect_range: 10,
            damage: 5,
            blow_away: 0,
            dirt_effect: -1,
            ..Default::default()
        }
    }

    fn mine(ty: i32, px: i32) -> WObject {
        WObject {
            pos: Vec2::new(itof(px), itof(50)),
            ty: Some(ty),
            owner_idx: 0,
            time_left: 100,
            ..WObject::default()
        }
    }

    // One blast of sobject_types[1] at (50,50) owned by worm 1, over `wobjects`.
    fn blast_at_50(wobjects: &mut Pool<WObject>, sobjects: &mut Pool<SObject>) {
        let cossin = precompute_cossin();
        let weapons = chain_weapons();
        let sts = vec![chain_blast(0), chain_blast(1)];
        let nts = nobject_types();
        let mut level = bg_level(100, 100);
        let mut nobjects: Pool<NObject> = Pool::new(600);
        let mut worms: Vec<WormState> = Vec::new();
        let mut rand = seeded();
        sobject_create(
            &sts[1],
            50,
            50,
            1,
            &mut worms,
            wobjects,
            &weapons,
            &mut nobjects,
            &nts,
            &mut level,
            &cossin,
            &SpriteSet::default(),
            &[],
            sobjects,
            &mut Pool::<Bonus>::new(1),
            &sts,
            100,
            0,
            100,
            &mut rand,
        );
    }

    #[test]
    fn a_chain_mine_in_the_blast_is_freed_then_blown_up() {
        // sobject.cpp:148-150 -> BlowUpObject: free FIRST (weapon.cpp:87), then its
        // create_on_exp at Ftoi(pos) — here a second blast at (52,50).
        let mut wobjects: Pool<WObject> = Pool::new(600);
        let mut sobjects: Pool<SObject> = Pool::new(700);
        wobjects.spawn(mine(0, 52));
        blast_at_50(&mut wobjects, &mut sobjects);
        assert!(wobjects.is_empty(), "the chained mine is freed");
        let blasts: Vec<(i32, i32, i32)> = sobjects.iter().map(|s| (s.id, s.x, s.y)).collect();
        assert_eq!(blasts, vec![(1, 42, 42), (1, 44, 42)], "trigger, then the mine's blast");
    }

    #[test]
    fn an_affected_non_chain_wobject_survives_the_blast() {
        let mut wobjects: Pool<WObject> = Pool::new(600);
        let mut sobjects: Pool<SObject> = Pool::new(700);
        wobjects.spawn(mine(1, 52));
        blast_at_50(&mut wobjects, &mut sobjects);
        assert_eq!(wobjects.len(), 1, "no chain_explosion -> only nudged");
        assert_eq!(sobjects.len(), 1, "only the trigger blast");
    }

    #[test]
    fn chains_recurse_depth_first_through_each_mines_own_blast() {
        // Mine A at x 55 is inside the trigger's box (40 < 55 < 60); mine B at x 63 is not
        // (63 >= 60) but is inside A's blast box (45 < 63 < 65). A is freed and blows up
        // first; its blast chains B; the outer walk then finds B's slot already free.
        let mut wobjects: Pool<WObject> = Pool::new(600);
        let mut sobjects: Pool<SObject> = Pool::new(700);
        wobjects.spawn(mine(0, 55));
        wobjects.spawn(mine(0, 63));
        blast_at_50(&mut wobjects, &mut sobjects);
        assert!(wobjects.is_empty(), "both mines went off");
        let xs: Vec<i32> = sobjects.iter().map(|s| s.x + 8).collect();
        assert_eq!(xs, vec![50, 55, 63], "trigger, A, then B (reached only through A)");
    }
```

(b) Append inside `mod tests` in `state.rs`:

```rust
    // -----------------------------------------------------------------------
    // Step 4½c-0 T6: the wobjects driver frees an exploding wobject BEFORE blow_up
    // (weapon.cpp:87), so its own blast's chain loop never sees its stale slot.
    // -----------------------------------------------------------------------

    fn chain_state() -> SimState {
        let w = 200i32;
        let level = LevelData {
            width: w,
            height: w,
            material_id: vec![1u8; (w * w) as usize],
            palette: None,
            display: None,
        };
        let mut flags = [0u8; 256];
        flags[0] = MAT_BACKGROUND;
        flags[1] = MAT_BACKGROUND;
        let weapons = vec![Weapon {
            id: 0,
            shot_type: 0,
            mult_speed: 100,
            affect_by_explosions: true,
            chain_explosion: true,
            time_to_explo: 5,
            create_on_exp: 0,
            dirt_effect: -1,
            splinter_amount: 0,
            obj_trail_type: -1,
            part_trail_obj: -1,
            ammo: 1,
            ..Default::default()
        }];
        let blast = SObjectType {
            id: 0,
            start_sound: -1,
            num_sounds: 0,
            anim_delay: 3,
            num_frames: 4,
            detect_range: 10,
            damage: 5,
            blow_away: 0,
            dirt_effect: -1,
            ..Default::default()
        };
        let mk = |index: i32, pos: Vec2| WormInit {
            index,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit { ty: Some(0), ammo: 1 }; NUM_WEAPONS],
            start_pos: pos,
            visible: true,
        };
        SimState::new(
            &level,
            &[
                mk(0, Vec2::new(itof(20), itof(20))),
                mk(1, Vec2::new(itof(180), itof(20))),
            ],
            1,
            &flags,
            weapons,
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            Vec::new(),
            vec![blast],
            Vec::new(),
            100,
            true,
            100,
        )
    }

    #[test]
    fn an_exploding_chain_wobject_is_freed_before_its_own_blast() {
        // time_left 0 -> the timeout explodes it this tick. Its blast (damage 5, ±10 box)
        // is centred on it: with the free AFTER blow_up, the stale slot would sit inside
        // the box and chain a second explosion.
        let mut state = chain_state();
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(100), itof(100)),
            vel: Vec2::zero(),
            cur_frame: 0,
            time_left: 0,
            ty: Some(0),
            owner_idx: 0,
        });
        state.process_frame(&[ControlState::new(), ControlState::new()]);
        assert!(state.wobjects.is_empty(), "the wobject exploded and is gone");
        assert_eq!(state.sobjects.len(), 1, "exactly one blast — no self-chain");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib chain`
Expected: FAIL — `a_chain_mine…`, `chains_recurse…` and `an_exploding_chain_wobject…` panic `chain_explosion -> BlowUpObject recursion deferred (O9)`; `an_affected_non_chain…` passes.

- [ ] **Step 3: Implement**

(1) `sobject.rs`: replace the whole `// --- 7b. wobjects blow-away loop` block — from the line `        // --- 7b. wobjects blow-away loop (:118-153). Nudges \`vel\` of wobjects` through the closing `        }` of its `for i in wobjects.iter_mut()` loop (the line before `        // --- 7c. nobjects blow-away loop`) — with:

```rust
        // --- 7b. wobjects blow-away loop (:118-153). Nudges `vel` of wobjects with
        // `affect_by_explosions`; draws NO rand. A `chain_explosion` wobject in the box is
        // blown up on the spot (:148-150, LIVE since 4½c-0 T6): `BlowUpObject(game,
        // owner_idx)` frees it FIRST (weapon.cpp:87), then explodes it at its post-nudge
        // pos/vel with THIS blast's owner as the cause — a recursion through `blow_up` ->
        // `sobject_create`. The index walk, restarted at each recursion level, is
        // order-identical to C++'s `wobjects.All()` Range (`Next()` skips freed slots, the
        // walk only advances) — the bonus chain-loop below has the same shape — and every
        // chained wobject is freed before its own blast scans, so the recursion ends.
        let obj_blow_away = ty.blow_away / 3;
        for slot in 0..wobjects.capacity() {
            let chained = {
                let Some(i) = wobjects.get_mut(slot) else {
                    continue;
                };
                let wid = i
                    .ty
                    .expect("live wobject must carry a resolved weapon type");
                let weapon = &weapons[wid as usize];
                if !weapon.affect_by_explosions {
                    continue;
                }
                let ipx = ftoi(i.pos.x);
                let ipy = ftoi(i.pos.y);
                if !(ipx < x + dr && ipx > x - dr && ipy < y + dr && ipy > y - dr) {
                    continue;
                }
                // x nudge: note the `else if delta < 0` — delta == 0 does
                // nothing (distinct from the worm loop's `if/else`).
                let delta = ipx - x;
                let power = dr - delta.abs();
                if power > 0 {
                    if delta > 0 {
                        i.vel.x = i.vel.x.wrapping_add(obj_blow_away.wrapping_mul(power));
                    } else if delta < 0 {
                        i.vel.x = i.vel.x.wrapping_sub(obj_blow_away.wrapping_mul(power));
                    }
                }
                let delta = ipy - y;
                let power = dr - delta.abs();
                if power > 0 {
                    if delta > 0 {
                        i.vel.y = i.vel.y.wrapping_add(obj_blow_away.wrapping_mul(power));
                    } else if delta < 0 {
                        i.vel.y = i.vel.y.wrapping_sub(obj_blow_away.wrapping_mul(power));
                    }
                }
                if weapon.chain_explosion {
                    Some((wid, i.pos, i.vel))
                } else {
                    None
                }
            };
            if let Some((wid, pos, vel)) = chained {
                wobjects.free(slot);
                crate::weapon::blow_up(
                    &weapons[wid as usize],
                    level,
                    large_sprites,
                    textures,
                    pos,
                    vel,
                    owner_idx,
                    sobject_types,
                    nobject_types,
                    cossin,
                    worms,
                    wobjects,
                    weapons,
                    nobjects,
                    sobjects,
                    bonuses,
                    blood,
                    game_mode,
                    settings_health,
                    rand,
                );
            }
        }
```

(2) `sobject.rs` docs: replace

```rust
//!    objects with `affect_by_explosions`; **draw NO rand**. The
//!    `chain_explosion -> BlowUpObject` recursion is **deferred (O9)**.
```

with

```rust
//!    objects with `affect_by_explosions`; **draw NO rand**. The
//!    `chain_explosion -> BlowUpObject` recursion is LIVE since 4½c-0 T6.
```

and replace `/// * **chain_explosion** recursion in the wobjects loop — \`debug_assert!\`ed off (O9);` with `/// * **chain_explosion** recursion in the wobjects loop — LIVE (4½c-0 T6);`.

(3) `state.rs`, the driver's Explode arm: replace

```rust
                WObjectOutcome::Explode => {
                    blow_up(
```

with

```rust
                WObjectOutcome::Explode => {
                    // weapon.cpp:87 — BlowUpObject frees `this` FIRST, then explodes: the
                    // blast's own wobject loop (and a chain it sets off, sobject.cpp:148-150)
                    // must not see this wobject's stale slot. Hash-neutral for every
                    // non-chain blast (the stale slot was only nudged, then freed anyway).
                    wobjects.free(slot);
                    blow_up(
```

and replace

```rust
                    );
                    wobjects.free(slot);
                }
                WObjectOutcome::Remove => {
```

with

```rust
                    );
                }
                WObjectOutcome::Remove => {
```

- [ ] **Step 4: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib chain` — Expected: PASS (the 4 new tests; the existing bonus-chain tests too).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (the free-first reorder is hash-neutral).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/sobject.rs rust/sim/src/state.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): port chain explosions (BOOBY TRAP); the driver frees before BlowUpObject like C++" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 7: `ProcessSteerables` (MISSILE) + `steerable_sum_x/y`  [Opus]

**Files:**
- Modify: `rust/sim/src/weapon.rs` (new `process_steerables` after `worm_fire`; new tests)
- Modify: `rust/sim/src/state.rs` (`WormState` fields after `steerable_count` `:341-344`; `from_init` `:439`; step 3 `:2007`; the `process_frame` doc list item 3 `:1519`; import `:34`; new tests)
- Modify: `rust/sim/src/{hash,wide_checksum,physics}.rs`, `rust/render/src/viewport.rs` (the `WormState` test literals — `steerable_count: 0,` is unique in each file)

**Interfaces:**
- Produces (used by T8, T9, T10):
  - `WormState.steerable_sum_x: i32`, `WormState.steerable_sum_y: i32` — `worm.hpp:267`; not hashed; `from_init` 0.
  - `pub fn process_steerables(worm: &mut WormState, weapons: &[Weapon], wobjects: &mut Pool<WObject>, cycles: i32)`.

Why: design §4.2. `worm.cpp:1214-1241`, called at `:324`: zero the three accumulators; if the CURRENT weapon is `kStSteerable`, every wobject of that weapon type owned by this worm turns (`cur_frame -= / += (cycles & 1) + 1`, `&= 127`), clears `movable`, and feeds the centroid. `cycles` is the worm-loop (post-`++cycles`) value. Today step 3 is a no-op: a MISSILE flies but never turns — a silent divergence.

- [ ] **Step 1: Write the failing tests**

(a) Append inside `mod tests` in `weapon.rs`:

```rust
    // ---- 4½c-0 T7: Worm::ProcessSteerables (worm.cpp:1214-1241) ---------------------

    // weapons[0] steerable (MISSILE-shaped), weapons[1] ST_TYPE2 (BAZOOKA-shaped).
    fn steer_weapons() -> Vec<Weapon> {
        vec![
            Weapon {
                id: 0,
                shot_type: ST_STEERABLE,
                ..Default::default()
            },
            Weapon {
                id: 1,
                shot_type: ST_TYPE2,
                ..Default::default()
            },
        ]
    }

    // Worm index 0 whose current slot (0) holds weapon `current_ty`, holding `controls`.
    fn steer_worm(current_ty: i32, controls: u32) -> WormState {
        let mut w = hit_worm(20, 20, Vec2::zero());
        w.index = 0;
        w.weapons[0].ty = Some(current_ty);
        w.current_weapon = 0;
        w.control_states = ControlState::unpack(controls);
        w
    }

    fn missile(owner: i32, ty: i32, cur_frame: i32, px: i32, py: i32) -> WObject {
        WObject {
            pos: Vec2::new(itof(px), itof(py)),
            ty: Some(ty),
            owner_idx: owner,
            cur_frame,
            ..WObject::default()
        }
    }

    #[test]
    fn steerables_turn_the_owners_current_type_missiles_and_sum_their_pixels() {
        // Left (4), cycles 1 -> step (1 & 1) + 1 = 2; `&= 127` wraps. Only wobjects of the
        // CURRENT weapon's type AND this worm's index are steered; each clears `movable`
        // and feeds the centroid sums.
        let weapons = steer_weapons();
        let mut w = steer_worm(0, 4);
        let mut pool: Pool<WObject> = Pool::new(8);
        pool.spawn(missile(0, 0, 32, 100, 50)); // steered: 32 - 2
        pool.spawn(missile(0, 0, 1, 110, 70)); // steered, wraps: (1 - 2) & 127 = 127
        pool.spawn(missile(1, 0, 40, 10, 10)); // another worm's: untouched
        pool.spawn(missile(0, 1, 50, 10, 10)); // another weapon type: untouched
        process_steerables(&mut w, &weapons, &mut pool, 1);
        let frames: Vec<i32> = pool.iter().map(|o| o.cur_frame).collect();
        assert_eq!(frames, vec![30, 127, 40, 50]);
        assert_eq!(
            (w.steerable_count, w.steerable_sum_x, w.steerable_sum_y),
            (2, 210, 120)
        );
        assert!(!w.movable, "a steered missile freezes the worm (movable = false)");
    }

    #[test]
    fn steerables_step_by_one_on_even_cycles_and_right_adds() {
        let weapons = steer_weapons();
        let mut w = steer_worm(0, 8); // Right
        let mut pool: Pool<WObject> = Pool::new(2);
        pool.spawn(missile(0, 0, 127, 5, 5));
        process_steerables(&mut w, &weapons, &mut pool, 2); // (2 & 1) + 1 = 1
        assert_eq!(pool.iter().next().unwrap().cur_frame, 0, "127 + 1 wraps to 0");
        assert_eq!(w.steerable_count, 1);
    }

    #[test]
    fn steerables_zero_the_sums_and_ignore_a_non_steerable_current_weapon() {
        let weapons = steer_weapons();
        let mut w = steer_worm(1, 4); // the current weapon is ST_TYPE2
        w.steerable_count = 7;
        w.steerable_sum_x = 7;
        w.steerable_sum_y = 7;
        let mut pool: Pool<WObject> = Pool::new(2);
        pool.spawn(missile(0, 1, 32, 5, 5));
        process_steerables(&mut w, &weapons, &mut pool, 1);
        assert_eq!((w.steerable_count, w.steerable_sum_x, w.steerable_sum_y), (0, 0, 0));
        assert_eq!(pool.iter().next().unwrap().cur_frame, 32, "not steerable: untouched");
        assert!(w.movable);
    }
```

(b) Append inside `mod tests` in `state.rs`:

```rust
    // -----------------------------------------------------------------------
    // Step 4½c-0 T7/T8: a MISSILE-shaped steerable in flight.
    // -----------------------------------------------------------------------

    /// Two visible worms on a 200x200 all-background level, every slot holding weapon 0 —
    /// a MISSILE-shaped steerable (shot_type 2, speed 230, add_speed 150) — and one such
    /// missile of worm 0 at rest at (100,100) aiming at cossin[32].
    fn steer_state() -> SimState {
        let w = 200i32;
        let level = LevelData {
            width: w,
            height: w,
            material_id: vec![1u8; (w * w) as usize],
            palette: None,
            display: None,
        };
        let mut flags = [0u8; 256];
        flags[0] = MAT_BACKGROUND;
        flags[1] = MAT_BACKGROUND;
        let weapons = vec![Weapon {
            id: 0,
            shot_type: 2,
            speed: 230,
            add_speed: 150,
            mult_speed: 100,
            obj_trail_type: -1,
            part_trail_obj: -1,
            ammo: 1,
            ..Default::default()
        }];
        let mk = |index: i32, pos: Vec2| WormInit {
            index,
            health: 100,
            lives: 5,
            stats_x: 0,
            weapons: [WeaponInit { ty: Some(0), ammo: 1 }; NUM_WEAPONS],
            start_pos: pos,
            visible: true,
        };
        let mut state = SimState::new(
            &level,
            &[
                mk(0, Vec2::new(itof(20), itof(20))),
                mk(1, Vec2::new(itof(180), itof(20))),
            ],
            1,
            &flags,
            weapons,
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            100,
            true,
            100,
        );
        state.wobjects.spawn(WObject {
            pos: Vec2::new(itof(100), itof(100)),
            vel: Vec2::zero(),
            cur_frame: 32,
            time_left: 0,
            ty: Some(0),
            owner_idx: 0,
        });
        state
    }

    #[test]
    fn the_visible_arm_steers_the_worms_missile_with_the_post_increment_cycles() {
        // worm.cpp:324: ProcessSteerables runs in the worm loop, AFTER `++cycles`
        // (game.cpp:357): cycles 0 -> 1 -> step (1 & 1) + 1 = 2.
        let mut state = steer_state();
        state.process_frame(&[ControlState::unpack(4), ControlState::new()]);
        let m = *state.wobjects.iter().next().expect("the missile flies on");
        assert_eq!(m.cur_frame, 30, "32 - 2");
        let w = &state.worms[0];
        assert_eq!(w.steerable_count, 1);
        assert_eq!(
            (w.steerable_sum_x, w.steerable_sum_y),
            (ftoi(m.pos.x), ftoi(m.pos.y))
        );
        assert!(!w.movable, "Left held + a live missile: movable stays false");
        assert_eq!(state.worms[1].steerable_count, 0, "worm 1 owns no missile");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib steer`
Expected: FAIL to compile — `cannot find function process_steerables`, `no field steerable_sum_x on type WormState`.

- [ ] **Step 3: Implement**

(1) `state.rs` — replace

```rust
    /// **Not hashed.**
    pub steerable_count: i32,
```

with

```rust
    /// **Not hashed.**
    pub steerable_count: i32,
    /// `Worm::steerable_sum_x` (`worm.hpp:267`): the sum of `Ftoi(pos.x)` over the
    /// wobjects `ProcessSteerables` steered this tick — with `steerable_sum_y` and
    /// `steerable_count`, the centroid the viewport follows (`viewport.cpp:30-32`).
    /// Written only by [`crate::weapon::process_steerables`]. **Not hashed** (absent from
    /// `stateHash.hpp` and `WideRollbackChecksum`). Default 0.
    pub steerable_sum_x: i32,
    /// `Worm::steerable_sum_y` (`worm.hpp:267`), see [`steerable_sum_x`](Self::steerable_sum_x).
    pub steerable_sum_y: i32,
```

(2) `state.rs` `from_init`: replace `            steerable_count: 0,` with `            steerable_count: 0,\n            steerable_sum_x: 0,\n            steerable_sum_y: 0,`. Do the same single replacement in `rust/sim/src/hash.rs`, `rust/sim/src/wide_checksum.rs`, `rust/sim/src/physics.rs` and `rust/render/src/viewport.rs` (each contains `steerable_count: 0,` exactly once; keep each file's indentation).

(3) `state.rs`: replace `use crate::weapon::{blow_up, wobject_process, worm_fire, WObjectConsts, WObjectOutcome};` with `use crate::weapon::{\n    blow_up, process_steerables, wobject_process, worm_fire, WObjectConsts, WObjectOutcome,\n};`.

(4) `state.rs` worm loop: replace

```rust
                // 3. process_steerables: no-op this slice (empty wobjects).
```

with

```rust
                // 3. ProcessSteerables (worm.cpp:324, :1214-1241) — LIVE (4½c-0 T7): turn
                //    this worm's wobjects of its CURRENT steerable weapon type by
                //    (cycles & 1) + 1 per held Left/Right — `*cycles` is the post-`++cycles`
                //    worm-loop value — clear `movable`, and accumulate the centroid the
                //    viewport follows. The movable reset below keeps `movable` false while
                //    Left/Right are held, freezing aim and walk.
                process_steerables(w, weapons, wobjects, *cycles);
```

and replace the doc line `    /// 3. \`process_steerables\` — no-op (empty \`wobjects\`).` with `    /// 3. [\`process_steerables\`] — steers this worm's steerable wobjects (4½c-0 T7).`

(5) `weapon.rs` — directly after the closing `}` of `worm_fire` insert:

```rust

/// Port of `Worm::ProcessSteerables` (`worm.cpp:1214-1241`), called from the visible arm
/// of `Worm::Process` right after the bonus pickup (`worm.cpp:324`).
///
/// Zeroes `steerable_count/sum_x/sum_y`, then — only when the worm's CURRENT weapon is
/// `kStSteerable` (MISSILE) — walks the wobjects in slot order and, for each of that SAME
/// weapon type (`i->type == ww.type`) owned by this worm (`owner_idx == index`): Left →
/// `cur_frame -= (cycles & 1) + 1`, Right → `+=` (both when both are held), then
/// `cur_frame &= 127` (the hashed wobject field), `movable = false`, and the centroid sums
/// `+= Ftoi(pos)`, `++count`. `cycles` is the worm-loop value (post-`++cycles`). Draws no
/// rand. An unresolved slot or a missing weapon definition is treated as non-steerable,
/// like `current_weapon_loops` (`state.rs`).
pub fn process_steerables(
    worm: &mut WormState,
    weapons: &[Weapon],
    wobjects: &mut Pool<WObject>,
    cycles: i32,
) {
    worm.steerable_count = 0;
    worm.steerable_sum_x = 0;
    worm.steerable_sum_y = 0;
    let Some(ty) = worm.weapons[worm.current_weapon as usize].ty else {
        return;
    };
    let steerable = weapons
        .get(ty as usize)
        .is_some_and(|w| w.shot_type == ST_STEERABLE);
    if !steerable {
        return;
    }
    let left = worm.control_states.get(ControlState::LEFT);
    let right = worm.control_states.get(ControlState::RIGHT);
    let step = (cycles & 1) + 1;
    for i in wobjects.iter_mut() {
        if i.ty == Some(ty) && i.owner_idx == worm.index {
            if left {
                i.cur_frame -= step;
            }
            if right {
                i.cur_frame += step;
            }
            i.cur_frame &= 127;
            worm.movable = false;
            worm.steerable_sum_x += ftoi(i.pos.x);
            worm.steerable_sum_y += ftoi(i.pos.y);
            worm.steerable_count += 1;
        }
    }
}
```

- [ ] **Step 4: Run the tests, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib steer` — Expected: PASS (the 3 weapon tests + the state test).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (no golden steered a MISSILE — design §5.5 — and the render goldens never reach `steerable_count > 0`, so `viewport.rs:86` stays quiet).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/weapon.rs rust/sim/src/state.rs rust/sim/src/hash.rs rust/sim/src/wide_checksum.rs rust/sim/src/physics.rs rust/render/src/viewport.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4.5c-0): port Worm::ProcessSteerables (MISSILE) + the steerable_sum accumulators" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 8: Inputs at the top of the tick (Rust + the dumper's reduced tail)  [Opus]

**Files:**
- Modify: `rust/sim/src/state.rs` (top of `process_frame` `:1532-1551`; the per-worm apply `:1933-1937`; the doc paragraph `:1506-1510`; a new test)
- Modify: `src/tools/oracle_dump/sim_physics_dump.cpp` (the reduced tail `:1166`, `:1226-1235`; the header comment `:47-48`)

**Interfaces:**
- Produces: `process_frame(inputs)` applies `inputs[i]` to `worms[i].control_states` for every `i < min(inputs.len(), worms.len())` BEFORE any sim work (was: at the top of each worm's pass). The dumper's reduced tail does the same.

Why: design §4.3. C++ controllers set every control state before `Game::ProcessFrame` (`localController.cpp:58-80`, `:175`), and the dumper's `render_live` path does too (`sim_physics_dump.cpp:1151-1160`); the object loops therefore read THIS tick's input. The only object-loop reads are the steerable Up boost (`weapon.cpp:152`) and RemExp (`:139`), which no prior golden reached, and no worm pass reads another worm's control state — so the move is hash-neutral for the corpus, proven below twice.

- [ ] **Step 1: Write the failing test** — append inside `mod tests` in `state.rs` (after T7's `steer_state` tests):

```rust
    #[test]
    fn the_object_loop_reads_this_ticks_input() {
        // Step 4½c-0 T8 (design §4.3): C++ sets control_states BEFORE Game::ProcessFrame, so
        // the MISSILE's Up boost (weapon.cpp:152) in the object loop sees THIS tick's Up:
        // new_vel = dir*speed/100 + dir*add_speed/100; vel = (vel*8 + new_vel)/9.
        let mut state = steer_state();
        state.process_frame(&[ControlState::unpack(1), ControlState::new()]);
        let dir = state.cossin[32];
        let boosted = dir.mul(230).div(100).add(dir.mul(150).div(100));
        let m = *state.wobjects.iter().next().expect("the missile flies on");
        assert_eq!(
            m.vel,
            Vec2::zero().mul(8).add(boosted).div(9),
            "boosted on the very first tick"
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib the_object_loop_reads_this_ticks_input`
Expected: FAIL — `boosted on the very first tick` (left is the unboosted `dir*230/100 / 9`: the object loop still sees the default control state).

- [ ] **Step 3: Implement the Rust side**

(1) In `process_frame` replace

```rust
        // Step 4½a-1: publish this tick's settings->shadow for the CorrectShadow sites.
        crate::shadow::begin_frame(self.shadow);
```

with

```rust
        // Step 4½a-1: publish this tick's settings->shadow for the CorrectShadow sites.
        crate::shadow::begin_frame(self.shadow);

        // Step 4½c-0 (design §4.3): every worm's input is applied HERE, before any sim
        // work — C++ controllers set `control_states` before `Game::ProcessFrame`
        // (`localController.cpp:58-80` / `:175`; the dumper's `render_live` path and, since
        // 4½c-0, its reduced tail too). The object loops therefore read THIS tick's input
        // (the steerable Up boost, `weapon.cpp:152`; RemExp, `:139`). Inputs shorter than
        // `worms` leave the remaining worms' control state unchanged.
        for (w, input) in self.worms.iter_mut().zip(inputs) {
            w.control_states = *input;
        }
```

(2) In the worm loop replace

```rust
        for i in 0..worms.len() {
            // Interleave: apply this worm's input (≈ `Unpack`), then Process it.
            if let Some(input) = inputs.get(i) {
                worms[i].control_states = *input;
            }

```

with

```rust
        for i in 0..worms.len() {
            // (This tick's input was applied at the top of `process_frame` — 4½c-0 T8.)

```

(3) Replace the doc paragraph

```rust
    /// **Input interleave** (matches the C++ dumper `sim_physics_dump.cpp:233-238`):
    /// for each worm in `worms` order, overwrite its `control_states` from the
    /// tick's input (mirroring `ControlState::Unpack`), then run that worm's full
    /// pass before moving to the next worm. Inputs shorter than `worms` leave the
    /// remaining worms' control state unchanged.
```

with

```rust
    /// **Input application** (Step 4½c-0, design §4.3): every worm's `control_states` is
    /// overwritten from the tick's input (mirroring `ControlState::Unpack`) at the TOP of
    /// the tick, before the object loops — as C++ controllers do before
    /// `Game::ProcessFrame`, and as the dumper does on every path since 4½c-0. Inputs
    /// shorter than `worms` leave the remaining worms' control state unchanged.
```

- [ ] **Step 4: Run the test, then the Rust re-diff (before touching C++)**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim --lib the_object_loop_reads_this_ticks_input` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS — the new Rust semantics reproduce every golden the OLD dumper wrote (no golden reaches an object-loop control read).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS (record/replay round-trips feed the same arrays; the passthrough goldens are unaffected).

- [ ] **Step 5: Implement the dumper side** — in `src/tools/oracle_dump/sim_physics_dump.cpp`:

(1) Replace

```cpp
      continue;
    }

    // Bonuses Process loop (game.cpp:287-290), at the TOP of ProcessFrame, BEFORE
```

with

```cpp
      continue;
    }

    // Step 4½c-0: apply every worm's input at the TOP of the tick, before the bonus and
    // object loops — exactly as the real Game::ProcessFrame sees it (controllers set
    // control_states before ProcessFrame; the render_live branch above does the same).
    // The object loops read it (the steerable Up boost, weapon.cpp:152; RemExp, :139);
    // no pre-4½c-0 scenario reaches such a read, so every prior golden is byte-identical.
    {
      std::array<uint32_t, 2> in{0, 0};
      auto const it = scn.inputs.find(t);
      if (it != scn.inputs.end()) {
        in = it->second;
      }
      for (int idx = 0; idx < static_cast<int>(game.worms.size()); ++idx) {
        game.worms[idx]->control_states.Unpack(idx < 2 ? in[idx] : 0);
      }
    }

    // Bonuses Process loop (game.cpp:287-290), at the TOP of ProcessFrame, BEFORE
```

(2) Replace

```cpp
    std::array<uint32_t, 2> in{0, 0};
    auto it = scn.inputs.find(t);
    if (it != scn.inputs.end()) {
      in = it->second;
    }
    for (int idx = 0; idx < static_cast<int>(game.worms.size()); ++idx) {
      auto const& w = game.worms[idx];
      w->control_states.Unpack(idx < 2 ? in[idx] : 0);
      w->Process(game);
    }
```

with

```cpp
    for (auto const& w : game.worms) {
      w->Process(game);
    }
```

(3) Replace the header lines

```cpp
//   input <tick> <worm0_7bit> <worm1_7bit>   (sparse; absent => 0; applied on the
//                                              Process pass advancing <tick>-><tick>+1)
```

with

```cpp
//   input <tick> <worm0_7bit> <worm1_7bit>   (sparse; absent => 0; applied at the TOP
//                                              of the pass advancing <tick>-><tick>+1,
//                                              before the object loops — Step 4½c-0)
```

- [ ] **Step 6: Format check and the regeneration proof**

Run: `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/sim_physics_dump.cpp` — Expected: no output, exit 0.

Regenerate, with the modified dumper, every golden on every dumper path (design §5.5). Run each script (each rebuilds `oracle_dump_sim_physics` incrementally, then prints `wrote …`; the settings script also prints its `gate` lines):

Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_fuzz1.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_fuzz2.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_fuzz3.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_fuzz4.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_fuzz5.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5d_fuzz1_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5d_fuzz2_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5d_fuzz3_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5d_fuzz4_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5prime_fuzz1_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5prime_fuzz2_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5prime_fuzz3_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice5prime_fuzz4_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_scales_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice6_gametag_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice4_5a_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice3b_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice3e_hud.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice3e_death.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice3e_reload.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice4d_live.sh`

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: **no output** — 21 scripts across the reduced tail, the `settings` path, the render injection path and the real-`ProcessFrame` path regenerate byte-identically. Any diff is stop-the-line: restore it with `git -C … checkout -- rust/oracle-tests/golden`, report the file and first differing tick, do not commit.

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/state.rs src/tools/oracle_dump/sim_physics_dump.cpp
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim+oracle(4.5c-0): apply inputs at the top of the tick, as C++ ProcessFrame sees them" -m "The object loops now read this tick's input (the MISSILE Up boost). Hash-neutral: the Rust re-diff passes against the old goldens and 21 goldens regenerate byte-identically with the modified dumper." -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 9: The steerable camera arm in `Viewport::process`  [Sonnet]

**Files:**
- Modify: `rust/render/src/viewport.rs` (`process` `:75-96`; a new test)

**Interfaces:**
- Consumes: `WormState.steerable_count/steerable_sum_x/steerable_sum_y` (T7).
- Produces: `Viewport::process` centres an alive, visible, steering worm's viewport on `(sum_x / count, sum_y / count)`.

Why: design §4.2. `viewport.cpp:30-32`. With T7 the Step-3 guard `debug_assert_eq!(worm.steerable_count, 0)` is reachable — a debug-build crash of `game`/`shot` on the first steered MISSILE.

- [ ] **Step 1: Write the failing test** — append inside `mod tests` in `viewport.rs`:

```rust
    #[test]
    fn process_centres_on_the_steerable_centroid() {
        // viewport.cpp:30-32: an alive, visible worm steering missiles is followed at the
        // missiles' pixel centroid, not at the worm. C++ `int /` truncates like Rust's.
        let mut vp = Viewport::new(Rect::new(0, 0, 158, 158), 0);
        let mut w = worm_at(20, 20);
        w.steerable_count = 2;
        w.steerable_sum_x = 150 + 170; // x 150 and 170 -> 160
        w.steerable_sum_y = 90 + 101; // y 90 and 101 -> 95 (truncating)
        vp.process(&w, 400, 300);
        assert_eq!((vp.x, vp.y), (160 - 79, 95 - 79), "SetCenter(sum / count)");
        // Without steerables the same worm is centred on itself (clamped at 0).
        w.steerable_count = 0;
        vp.process(&w, 400, 300);
        assert_eq!((vp.x, vp.y), (0, 0));
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p render --lib process_centres_on_the_steerable_centroid`
Expected: FAIL — panics `steerable centering deferred to 3b` (the debug guard).

- [ ] **Step 3: Implement** — replace

```rust
    /// `viewport.cpp:22-57`. NB: the `steerable_count > 0` centering
    /// (`viewport.cpp:31-32`) reads `steerable_sum_x/y`, which `WormState` does
    /// not yet carry; 3a scenarios keep `steerable_count == 0`, so that arm is
    /// asserted-unreachable and the pos-centering arm is used. Steerable
    /// centering lands with the sprite pass in 3b.
    pub fn process(&mut self, worm: &WormState, level_w: i32, level_h: i32) {
        self.max_x = level_w - self.rect.width();
        self.max_y = level_h - self.rect.height();

        if worm.killed_timer <= 0 {
            if worm.visible {
                debug_assert_eq!(worm.steerable_count, 0, "steerable centering deferred to 3b");
                self.set_center(ftoi(worm.pos.x), ftoi(worm.pos.y));
            } else {
```

with

```rust
    /// `viewport.cpp:22-57`. An alive, visible worm that steered missiles this tick
    /// (`steerable_count > 0`, set by the sim's `process_steerables`) is followed at the
    /// missiles' pixel centroid (`viewport.cpp:30-32`, 4½c-0 T9); otherwise at its own
    /// position.
    pub fn process(&mut self, worm: &WormState, level_w: i32, level_h: i32) {
        self.max_x = level_w - self.rect.width();
        self.max_y = level_h - self.rect.height();

        if worm.killed_timer <= 0 {
            if worm.visible {
                if worm.steerable_count > 0 {
                    self.set_center(
                        worm.steerable_sum_x / worm.steerable_count,
                        worm.steerable_sum_y / worm.steerable_count,
                    );
                } else {
                    self.set_center(ftoi(worm.pos.x), ftoi(worm.pos.y));
                }
            } else {
```

- [ ] **Step 4: Run the test, then the re-diff**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p render --lib viewport` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (no render golden steers).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.

- [ ] **Step 5: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/render/src/viewport.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "render(4.5c-0): the viewport follows a steering worm's missile centroid (closes the 4d deferral)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 10: The witness module, the generator, and the four settings-driven C++ goldens  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/sim_slice4_5c0_common/mod.rs`
- Create: `rust/oracle-tests/examples/gen_slice4_5c0.rs`
- Create: `rust/oracle-tests/gen_sim_slice4_5c0_golden.sh`
- Create (generated): `rust/oracle-tests/golden/sim_slice4_5c0_{laser,missile,trails,booby}_setup.cfg`, `…_scenario.txt`, `….txt`

**Interfaces:**
- Consumes: `scenario::build::build_match`, `scenario::settings::MatchConfig`, `scenario::settings_toml::settings_from_toml`, `oracle_tests::scenario::Scenario` (`input(tick, worm) -> u32`, `ticks`, `seed`, `level`), `scenario::load`, `sim::state::{SimState, ControlState, WObject, NObject}`, `WormState.steerable_*` (T7).
- Produces (used by T11, T12): module `sim_slice4_5c0_common` (included by tests as `mod sim_slice4_5c0_common;` and by the example via `#[path]`) with `TC_ROOT`, `LEVEL`, `LIVES`, `HEALTH`, `LOADING_TIME`, `BLOOD`, `MAX_BONUSES`, `BLOOD_PARTICLE_MAX`, `FORMERLY_DEFERRED`, `struct Variant { name, input_seed, ticks, p1, p2 }`, `VARIANTS`, `fn variant(&str) -> &'static Variant`, `fn load_objects() -> Objects`, `fn weapon_index(&Objects, &str) -> i32`, `fn menu_index(&Objects, &str) -> u32`, `fn setup_cfg(&Variant, &Objects) -> String`, `fn inputs(u32, u32) -> Vec<[u32; 2]>`, `fn steer_inputs(u32, u32) -> Vec<[u32; 2]>`, `struct Ids` + `Ids::new(&Objects)`, `struct Pre` + `Pre::capture(&SimState)`, `struct Ledger` + `observe(&mut self, &Ids, &Objects, &Pre, &SimState, [u32; 2])` + `summary()`, `fn ok(&Variant, &Ledger) -> bool`, `struct SteerLedger` + `observe(&mut self, u32, i32, &Pre, &SimState, [u32; 2])`, `fn steer_ok(&SteerLedger) -> bool`.

Why: design §5.2-§5.3. One implementation of the witnesses serves the seed scan and the milestone, so they cannot drift. After T1–T9 no weapon branch is a tripwire any more: a panic during a scan is a bug to investigate, never a seed to skip.

- [ ] **Step 1: Create the shared module** `rust/oracle-tests/tests/sim_slice4_5c0_common/mod.rs`:

```rust
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
        p2: ["BOUNCY LARPA", "BIG NUKE", "NAPALM", "MINI NUKE", "CRACKLER"],
    },
    Variant {
        name: "booby",
        input_seed: 1004,
        ticks: 2000,
        p1: ["BOOBY TRAP", "BAZOOKA", "BOOBY TRAP", "GRENADE", "BOOBY TRAP"],
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
    out.push_str(&format!("blood = {BLOOD}\nbloodParticleMax = {BLOOD_PARTICLE_MAX}\n"));
    out.push_str(&format!("gameMode = 0\nlevelFile = '{LEVEL}'\nlives = {LIVES}\n"));
    out.push_str(&format!("loadChange = true\nloadingTime = {LOADING_TIME}\n"));
    out.push_str(&format!("maxBonuses = {MAX_BONUSES}\nrandomLevel = false\n"));
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
        for (k, w) in [ids.rifle, ids.winchester, ids.gauss].into_iter().enumerate() {
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
        for (k, w) in [ids.laser, ids.rifle, ids.winchester].into_iter().enumerate() {
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
        for (k, w) in [ids.larpa, ids.bouncy_larpa, ids.crackler].into_iter().enumerate() {
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
        "missile" => {
            l.steer_left && l.steer_right && l.steer_step1 && l.steer_step2 && l.boosted
        }
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
```

- [ ] **Step 2: Create the generator** `rust/oracle-tests/examples/gen_slice4_5c0.rs`:

```rust
//! Step 4½c-0 T10/T11 — setup-sidecar writer, seed scanner and scenario writer for the
//! weapon-branch goldens (design §5). A dev tool, not a test; not run in CI.
//!
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- cfg  <variant> <out_setup.cfg>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- scan <variant> <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- gen  <variant> <game_seed> <out_scenario.txt>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- steer-scan <game_seed_lo> <game_seed_hi>
//!   cargo run -p oracle-tests --example gen_slice4_5c0 -- steer-gen  <game_seed> <out_scenario.txt>
//!
//! Variants: laser, missile, trails, booby (input seeds fixed in the shared table). Run in
//! a DEBUG build (overflow checks). After 4½c-0 no weapon branch is a tripwire, so a PANIC
//! line from a scan is a real bug — investigate it, never skip it.

#[path = "../tests/sim_slice4_5c0_common/mod.rs"]
mod c0;

use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};
use std::path::Path;

use assets::level::LevelData;
use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::MatchConfig;
use scenario::settings_toml::settings_from_toml;
use sim::state::ControlState;

/// The steer render golden (design §5.4): no POWERLEVEL palette on this level, so the
/// `scenario::load` render path shows the same palette as C++.
const STEER_LEVEL: &str = "Levels/physics_fall_test.lev";
const STEER_TICKS: u32 = 500;
const STEER_INPUT_SEED: u32 = 5151;

fn load_level(rel: &str) -> LevelData {
    assets::level::load(&std::fs::read(format!("{}/{rel}", c0::TC_ROOT)).unwrap()).unwrap()
}

/// Drive a settings variant through the REAL Rust readers + builder; `None` = it panicked.
fn run(v: &c0::Variant, game_seed: u32) -> Option<c0::Ledger> {
    let o = c0::load_objects();
    let ids = c0::Ids::new(&o);
    let level = load_level(c0::LEVEL);
    let settings = settings_from_toml(&c0::setup_cfg(v, &o)).expect("sidecar parses");
    let cfg = MatchConfig {
        settings,
        seed: game_seed,
    };
    let ins = c0::inputs(v.input_seed, v.ticks);
    catch_unwind(AssertUnwindSafe(|| {
        let mut st = build_match(Path::new(c0::TC_ROOT), &cfg, &level)
            .expect("variant config builds")
            .state;
        let mut l = c0::Ledger::default();
        for w in &ins {
            let pre = c0::Pre::capture(&st);
            st.process_frame(&[ControlState::unpack(w[0]), ControlState::unpack(w[1])]);
            l.observe(&ids, &o, &pre, &st, *w);
        }
        l
    }))
    .ok()
}

fn steer_scenario(game_seed: u32, header: &str) -> String {
    let mut out = String::from(header);
    out.push_str(&format!(
        "seed {game_seed}\nlevel {STEER_LEVEL}\nticks {STEER_TICKS}\nmax_bonuses 0\n"
    ));
    out.push_str("# worm <index> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>\n");
    out.push_str("worm 0 0 0 100 10 0   0\nworm 1 0 0 100 10 218 0\n");
    out.push_str("weapon 0 MISSILE\nrender player\nrender_live\n");
    out.push_str("# input <tick> <worm0_7bit> <worm1_7bit>  (Up=1 Down=2 Left=4 Right=8 Fire=16 Change=32 Jump=64)\n");
    for (t, w) in c0::steer_inputs(STEER_INPUT_SEED, STEER_TICKS).iter().enumerate() {
        if w[0] != 0 || w[1] != 0 {
            out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
        }
    }
    out
}

/// Drive the steer scenario through `scenario::load` (the path the render test uses).
fn steer_run(game_seed: u32) -> Option<c0::SteerLedger> {
    let s = Scenario::parse(&steer_scenario(game_seed, "")).expect("steer scenario parses");
    let missile = c0::weapon_index(&c0::load_objects(), "MISSILE");
    catch_unwind(AssertUnwindSafe(|| {
        let mut st = scenario::load(Path::new(c0::TC_ROOT), &s).state;
        let mut l = c0::SteerLedger::default();
        for k in 1..=s.ticks {
            let input = [s.input(k - 1, 0), s.input(k - 1, 1)];
            let pre = c0::Pre::capture(&st);
            st.process_frame(&[ControlState::unpack(input[0]), ControlState::unpack(input[1])]);
            l.observe(k, missile, &pre, &st, input);
        }
        l
    }))
    .ok()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| {
        args.get(i)
            .unwrap_or_else(|| panic!("missing argument {i}"))
            .as_str()
    };
    let num = |i: usize| {
        arg(i)
            .parse::<u32>()
            .unwrap_or_else(|e| panic!("argument {i}: {e}"))
    };
    match arg(0) {
        "cfg" => {
            let v = c0::variant(arg(1));
            std::fs::write(arg(2), c0::setup_cfg(v, &c0::load_objects())).expect("write setup");
            println!("wrote {}", arg(2));
        }
        "scan" => {
            let v = c0::variant(arg(1));
            set_hook(Box::new(|_| {})); // a panic is reported below
            for game_seed in num(2)..=num(3) {
                match run(v, game_seed) {
                    None => println!("{} game_seed={game_seed} PANIC — a real bug", v.name),
                    Some(l) => println!(
                        "{} game_seed={game_seed} ok={} {}",
                        v.name,
                        c0::ok(v, &l),
                        l.summary()
                    ),
                }
            }
        }
        "gen" => {
            let v = c0::variant(arg(1));
            let game_seed = num(2);
            let l = run(v, game_seed).expect("the seed must not panic");
            assert!(c0::ok(v, &l), "seed fails the witnesses: {}", l.summary());
            let mut out = format!(
                "# Step 4½ slice 4½c-0 T10 — weapon-branch match `{name}` (design §5.2). Read by\n\
                 # BOTH the C++ dumper (oracle_dump_sim_physics `settings` path: the real\n\
                 # Settings::FromToml + the LocalController start) and the Rust milestone test\n\
                 # (settings_toml + scenario::build::build_match). Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5c0 -- gen {name} {game_seed} <this file>\n\
                 # LEDGER (Rust, driven state): {summary}\n\
                 # Inputs: per tick Rand({input_seed}).next_u32() & 0x7f, worm 0 then worm 1 (zero pairs omitted).\n\
                 seed {game_seed}\nlevel {level}\nticks {ticks}\nsettings sim_slice4_5c0_{name}_setup.cfg\n",
                name = v.name,
                summary = l.summary(),
                input_seed = v.input_seed,
                level = c0::LEVEL,
                ticks = v.ticks,
            );
            for (t, w) in c0::inputs(v.input_seed, v.ticks).iter().enumerate() {
                if w[0] != 0 || w[1] != 0 {
                    out.push_str(&format!("input {t} {} {}\n", w[0], w[1]));
                }
            }
            std::fs::write(arg(3), out).expect("write scenario");
            println!("wrote {} — {}", arg(3), l.summary());
        }
        "steer-scan" => {
            set_hook(Box::new(|_| {}));
            for game_seed in num(1)..=num(2) {
                match steer_run(game_seed) {
                    None => println!("steer game_seed={game_seed} PANIC — a real bug"),
                    Some(l) => println!(
                        "steer game_seed={game_seed} ok={} camera_ticks={} off_worm={} left={} right={} boosted={}",
                        c0::steer_ok(&l),
                        l.camera_ticks.len(),
                        l.off_worm_ticks.len(),
                        l.left,
                        l.right,
                        l.boosted
                    ),
                }
            }
        }
        "steer-gen" => {
            let game_seed = num(1);
            let l = steer_run(game_seed).expect("the seed must not panic");
            assert!(c0::steer_ok(&l), "seed fails the steer witnesses: {l:?}");
            let header = format!(
                "# Step 4½ slice 4½c-0 T11 — the STEERABLE CAMERA, LIVE (design §5.4). Read by BOTH\n\
                 # the C++ dumper (oracle_dump_sim_physics `render_live`: real Game::ProcessFrame —\n\
                 # inputs first, so the MISSILE Up boost reads this tick's input — and the real\n\
                 # ProcessViewports) and the Rust render test (the 4d live harness). Both worms start\n\
                 # dead at (0,0) and respawn in-sim (killed_timer <= 0: the camera's alive arm), carry\n\
                 # MISSILE in slot 0 and never press Change. Written by\n\
                 #   cargo run -p oracle-tests --example gen_slice4_5c0 -- steer-gen {game_seed} <this file>\n\
                 # LEDGER (Rust): {} camera ticks, {} off the worm (first t{}), left={} right={} boosted={}\n\
                 # Inputs: per tick v = Rand({STEER_INPUT_SEED}).next_u32(); (v & 0x4f) | Fire iff (v >> 8) & 7 == 0.\n",
                l.camera_ticks.len(),
                l.off_worm_ticks.len(),
                l.off_worm_ticks[0].0,
                l.left,
                l.right,
                l.boosted,
            );
            std::fs::write(arg(2), steer_scenario(game_seed, &header)).expect("write scenario");
            println!("wrote {}", arg(2));
        }
        other => panic!("unknown command {other:?} (cfg | scan | gen | steer-scan | steer-gen)"),
    }
}
```

- [ ] **Step 3: Create the gen script** `rust/oracle-tests/gen_sim_slice4_5c0_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/sim_slice4_5c0_<variant>.txt — 12 columns: the 11 oracle_dump_sim_physics
# hash columns + Game::IsGameOver() — for the Step-4½ slice-4½c-0 WEAPON-BRANCH scenarios.
# Each scenario's `settings <file>` sidecar is read by the REAL C++ Settings::FromToml and the
# worms start in the C++ LocalController state (sim_physics_dump.cpp `settings` path).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — NOT run
# in the lightweight rust.yml CI. Override PRESET for other platforms (e.g. linux-x64).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
for v in laser missile trails booby; do
  out="rust/oracle-tests/golden/sim_slice4_5c0_${v}.txt"
  "build/$PRESET/Release/oracle_dump_sim_physics" \
    "rust/oracle-tests/golden/sim_slice4_5c0_${v}_scenario.txt" \
    "$out"
  echo "wrote $out"
  # C++-SIDE GATE: 12 columns on every row, and column 12 (IsGameOver) constant 0 — lives 99,
  # these matches never end (design §5.2). A failure aborts the script (set -e + awk exit 1).
  awk -v v="$v" '
    NF != 12 { printf "FAIL %s: %d columns at tick %s (want 12)\n", v, NF, $1; bad = 1; exit 1 }
    $12 != "0" { printf "FAIL %s: IsGameOver=%s at tick %s (lives 99: want 0)\n", v, $12, $1; bad = 1; exit 1 }
    END { if (bad) { exit 1 } printf "  gate %s: %d rows, 12 columns, IsGameOver constant 0\n", v, NR }
  ' "$out"
done
```

- [ ] **Step 4: Build and scan**

Run: `rustfmt --edition 2021 /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/sim_slice4_5c0_common/mod.rs`
Run: `rustfmt --edition 2021 /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/examples/gen_slice4_5c0.rs`
Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0` — Expected: builds, no warnings.

For each variant, scan game seeds 1..=60 (DEBUG):

Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- scan laser 1 60`
Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- scan missile 1 60`
Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- scan trails 1 60`
Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- scan booby 1 60`

Expected: one line per seed, no `PANIC` line, and at least one `ok=true` per variant. Pick, per variant, the FIRST `ok=true` game seed (call it the chosen seed; record all four in the done-report). If a variant has none in 1..=60, rescan `61 200`; if still none, raise that variant's `ticks` in `VARIANTS` by 500 and rescan — never weaken a witness. A `PANIC` line is stop-the-line: reproduce it with `gen` (which does not catch the panic), fix the port, rerun the re-diff.

- [ ] **Step 5: Write the sidecars and scenarios** — for each variant `<v>` with its chosen seed `<s>`:

Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- cfg <v> /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/sim_slice4_5c0_<v>_setup.cfg`
Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- gen <v> <s> /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/sim_slice4_5c0_<v>_scenario.txt`

Expected: `wrote …` for both, the `gen` line repeating the ledger summary.

- [ ] **Step 6: Generate the C++ goldens**

Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_sim_slice4_5c0_golden.sh`
Expected: four `wrote rust/oracle-tests/golden/sim_slice4_5c0_<v>.txt` lines, each followed by `  gate <v>: <ticks+1> rows, 12 columns, IsGameOver constant 0` (1501 / 1501 / 2001 / 2001 with the table's ticks).
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: exactly the 12 new `sim_slice4_5c0_*` files (`??`), nothing modified.

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/sim_slice4_5c0_common/mod.rs rust/oracle-tests/examples/gen_slice4_5c0.rs rust/oracle-tests/gen_sim_slice4_5c0_golden.sh rust/oracle-tests/golden
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5c-0): weapon-branch witnesses, generator, and the laser/missile/trails/booby C++ goldens" -m "Chosen game seeds: laser <s>, missile <s>, trails <s>, booby <s> (from the T10 scans)." -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

(Write the four chosen seeds into the second `-m` in place of each `<s>`.)

---

### Task 11: The steerable-camera `render_live` golden  [Opus]

**Files:**
- Modify: `rust/oracle-tests/tests/render_slice4d_common/mod.rs` (`read_golden` `:144-150`, `build` `:167-191`, `run` `:248-341`, `modified` `:359-418`)
- Create: `rust/oracle-tests/gen_render_slice4_5c0_steer.sh`
- Create (generated): `rust/oracle-tests/golden/render_slice4_5c0_steer_scenario.txt`, `…_sim.txt`, `….txt`

**Interfaces:**
- Produces (used by T12): `pub fn read_golden_stem(stem: &str, suffix: &str) -> String`, `pub fn run_stem(stem: &str) -> RunResult`, `pub fn modified_stem(stem: &str, target_tick: u32, force_flash: Option<i32>, suppress_shake: bool, suppress_banner: bool) -> (u64, [(i32, i32); 2])`. `read_golden`, `run`, `modified` keep their signatures as `render_slice4d_{name}` wrappers, so `render_slice4d.rs` is untouched.

Why: design §5.4. The C++ `render_live` path runs the real `Game::ProcessFrame` (inputs first — M3 through the real frame) and the real `ProcessViewports` (the centroid arm); the 4d live harness already reproduces that tick shape. Only the alive camera arm needs `killed_timer <= 0`, i.e. an in-sim respawn, so both worms start dead.

- [ ] **Step 1: Generalise the 4d harness to file stems** — in `render_slice4d_common/mod.rs`:

(1) Replace

```rust
pub fn read_golden(name: &str, suffix: &str) -> String {
    let path = format!(
        "{}/golden/render_slice4d_{name}{suffix}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}
```

with

```rust
pub fn read_golden(name: &str, suffix: &str) -> String {
    read_golden_stem(&format!("render_slice4d_{name}"), suffix)
}

/// `golden/<stem><suffix>` — the 4½c-0 steerable-camera golden reuses this harness.
pub fn read_golden_stem(stem: &str, suffix: &str) -> String {
    let path = format!("{}/golden/{stem}{suffix}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}
```

(2) Replace

```rust
fn build(name: &str) -> Built {
    let scenario_text = read_golden(name, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    assert_eq!(scenario.seed, 42, "4d scenario uses seed 42");
```

with

```rust
fn build(stem: &str) -> Built {
    let scenario_text = read_golden_stem(stem, "_scenario.txt");
    let scenario = Scenario::parse(&scenario_text).expect("scenario parses");
    // The seed is the scenario's own: each gen script passes it to the dumper explicitly
    // (4d: 42; the 4½c-0 steer golden: its scanned seed).
```

(3) Replace

```rust
pub fn run(name: &str) -> RunResult {
    let mut b = build(name);
```

with

```rust
pub fn run(name: &str) -> RunResult {
    run_stem(&format!("render_slice4d_{name}"))
}

/// [`run`] for any golden file stem (`golden/<stem>{_scenario,_sim,}.txt`).
pub fn run_stem(name: &str) -> RunResult {
    let mut b = build(name);
```

and inside that body replace `parse_frames(&read_golden(name, ".txt"))` with `parse_frames(&read_golden_stem(name, ".txt"))`, and `let sim_text = read_golden(name, "_sim.txt");` with `let sim_text = read_golden_stem(name, "_sim.txt");`.

(4) Replace

```rust
pub fn modified(
    name: &str,
    target_tick: u32,
    force_flash: Option<i32>,
    suppress_shake: bool,
    suppress_banner: bool,
) -> (u64, [(i32, i32); 2]) {
    let mut b = build(name);
```

with

```rust
pub fn modified(
    name: &str,
    target_tick: u32,
    force_flash: Option<i32>,
    suppress_shake: bool,
    suppress_banner: bool,
) -> (u64, [(i32, i32); 2]) {
    modified_stem(
        &format!("render_slice4d_{name}"),
        target_tick,
        force_flash,
        suppress_shake,
        suppress_banner,
    )
}

/// [`modified`] for any golden file stem.
pub fn modified_stem(
    name: &str,
    target_tick: u32,
    force_flash: Option<i32>,
    suppress_shake: bool,
    suppress_banner: bool,
) -> (u64, [(i32, i32); 2]) {
    let mut b = build(name);
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test render_slice4d` — Expected: PASS (the 4d golden, unchanged, through the wrappers).

- [ ] **Step 2: Scan for the steer seed**

Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- steer-scan 1 100`
Expected: one line per seed, no `PANIC`, at least one `ok=true` (with `off_worm > 0`). Pick the FIRST `ok=true` game seed. If none, rescan `101 300`.

- [ ] **Step 3: Write the scenario**

Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example gen_slice4_5c0 -- steer-gen <seed> /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/render_slice4_5c0_steer_scenario.txt` — Expected: `wrote …`.

- [ ] **Step 4: Create the gen script** `rust/oracle-tests/gen_render_slice4_5c0_steer.sh`:

```bash
#!/usr/bin/env bash
# Regenerates the Step-4½c-0 STEERABLE-CAMERA live render golden (design §5.4). Builds the REAL
# C++ Game + a headless Renderer and drives render_slice4_5c0_steer_scenario.txt through the
# opt-in `render_live` path (real Game::ProcessFrame — inputs applied first — plus the two
# viewports wired into ProcessViewports, whose alive arm centres a steering worm's viewport on
# its missiles' centroid, viewport.cpp:30-32). Writes the 11-column sim record
# (render_slice4_5c0_steer_sim.txt) and the frame sidecar (render_slice4_5c0_steer.txt:
# <tick> <frame_hash16> <state_hash8> + `total`). LOCAL/MANUAL — needs the full C++ build.
# The dumper's 3rd argument is the seed, read from the scenario (the generator scanned it).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_sim_physics
BIN="$ROOT/build/$PRESET/Release/oracle_dump_sim_physics"
GOLD="$ROOT/rust/oracle-tests/golden"
SCN="$GOLD/render_slice4_5c0_steer_scenario.txt"
SEED="$(awk '$1 == "seed" { print $2 }' "$SCN")"
"$BIN" "$SCN" "$GOLD/render_slice4_5c0_steer_sim.txt" "$SEED" "$GOLD/render_slice4_5c0_steer.txt"
echo "wrote render_slice4_5c0_steer{_sim,}.txt (seed $SEED)"
```

- [ ] **Step 5: Generate the C++ golden**

Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_render_slice4_5c0_steer.sh`
Expected: `wrote render_slice4_5c0_steer{_sim,}.txt (seed <seed>)`.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: exactly the 3 new `render_slice4_5c0_steer*` files (`??`) — T10's files are already committed — and nothing modified.

- [ ] **Step 6: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/render_slice4d_common/mod.rs rust/oracle-tests/gen_render_slice4_5c0_steer.sh rust/oracle-tests/golden
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5c-0): the steerable-camera render_live golden; 4d live harness keyed by file stem" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 12: MILESTONE — every new golden bit-exact vs C++  [Opus]

**Files:**
- Create: `rust/oracle-tests/tests/sim_slice4_5c0_weapons_golden.rs`
- Create: `rust/oracle-tests/tests/render_slice4_5c0_steer.rs`

**Interfaces:**
- Consumes: T10's `sim_slice4_5c0_common`, T11's `run_stem`/`modified_stem`/`read_golden_stem`, the committed goldens.

Why: design §1 done-when 2-3, §5.3-§5.4. Bit-exactness (12 columns per tick for the sim goldens; frame + state + isolation triple for the render golden) plus the reach witnesses, re-derived from the committed files through the real readers — nothing regenerated in memory.

- [ ] **Step 1: Create** `rust/oracle-tests/tests/sim_slice4_5c0_weapons_golden.rs`:

```rust
//! Step 4½c-0 T12 — MILESTONE (design §5.2-§5.3): the four weapon-branch matches
//! bit-exact vs C++.
//!
//! Each committed scenario names a lean C++-schema setup (`settings <file>`). The C++
//! dumper read it with the real `Settings::FromToml`; here the SAME files go through
//! `Scenario::parse`, `settings_toml::settings_from_toml` and `build_match`. Every row is
//! asserted on the 11 hash columns (components first, master last) and on column 12
//! (`IsGameOver`, constant 0 — lives 99). The shared ledger then re-derives every reach
//! witness from the driven state, so a golden that passed without reaching its branches
//! fails here.

mod sim_slice4_5c0_common;

use std::path::Path;

use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::MatchConfig;
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::state::{ControlState, SimState};
use sim_slice4_5c0_common as c0;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");

fn read(rel: &str) -> String {
    let path = Path::new(GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

struct Row {
    tick: u32,
    hashes: [u32; 10], // master, rng, level, worm0, worm1, bob, bon, sob, nob, wob
    game_over: u32,
}

fn parse_golden(text: &str) -> Vec<Row> {
    let rows: Vec<Row> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(cols.len(), 12, "settings-path golden lines have 12 columns");
            let mut hashes = [0u32; 10];
            for (i, c) in cols[1..11].iter().enumerate() {
                hashes[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            Row {
                tick: cols[0].parse().expect("tick"),
                hashes,
                game_over: cols[11].parse().expect("game-over column"),
            }
        })
        .collect();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(r.tick, k as u32, "golden row {k} carries tick {}", r.tick);
    }
    rows
}

fn check(state: &SimState, row: &Row) {
    let c = hash_components(state);
    let got = [
        hash_game_state(state),
        c.rng,
        c.level,
        c.worms[0],
        c.worms[1],
        c.bobjects,
        c.bonuses,
        c.sobjects,
        c.nobjects,
        c.wobjects,
    ];
    let names = [
        "master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob",
    ];
    // Components first, master last, so a divergence localises.
    for i in (1..10).chain(0..1) {
        assert_eq!(
            got[i], row.hashes[i],
            "tick {}: {}: got {:08x} want {:08x}",
            row.tick, names[i], got[i], row.hashes[i]
        );
    }
    assert_eq!(is_game_over(state) as u32, row.game_over, "tick {}: IsGameOver", row.tick);
}

struct Case {
    scenario: Scenario,
    cfg: MatchConfig,
}

fn case(name: &str) -> Case {
    let scenario = Scenario::parse(&read(&format!("sim_slice4_5c0_{name}_scenario.txt")))
        .unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"));
    let rel = scenario.settings.clone().expect("a settings-driven scenario");
    assert_eq!(rel, format!("sim_slice4_5c0_{name}_setup.cfg"));
    assert!(scenario.worms.is_empty(), "worms come from the setup");
    let settings = settings_from_toml(&read(&rel))
        .unwrap_or_else(|e| panic!("{name}: setup {rel} parses: {e:?}"));
    Case {
        cfg: MatchConfig {
            settings,
            seed: scenario.seed,
        },
        scenario,
    }
}

/// Intent guard: the committed sidecar carries the variant's settings (design §5.2, §6).
fn assert_sidecar(name: &str, c: &Case, o: &assets::object::Objects) {
    let v = c0::variant(name);
    let s = &c.cfg.settings;
    assert_eq!(s.weap_table, [0u32; 40], "{name}: every weapon may drop (the ban lift)");
    assert_eq!(
        (s.game_mode, s.lives, s.loading_time, s.max_bonuses, s.blood),
        (0, c0::LIVES, c0::LOADING_TIME, c0::MAX_BONUSES, c0::BLOOD),
        "{name}: sim settings"
    );
    assert_eq!(s.blood_particle_max, c0::BLOOD_PARTICLE_MAX);
    assert!(s.shadow && s.load_change, "{name}: shadow + loadChange");
    let ws = &s.worm_settings;
    assert_eq!((ws[0].health, ws[1].health), (c0::HEALTH, c0::HEALTH));
    assert_eq!(ws[0].weapons, v.p1.map(|n| c0::menu_index(o, n)), "{name}: player 1");
    assert_eq!(ws[1].weapons, v.p2.map(|n| c0::menu_index(o, n)), "{name}: player 2");
    assert_eq!(c.scenario.ticks, v.ticks, "{name}: ticks");
    assert_eq!(c.scenario.level, c0::LEVEL, "{name}: level");
}

/// Drive the committed case; with `golden`, assert every row. Returns the ledger and
/// the master series.
fn drive(c: &Case, golden: Option<&[Row]>) -> (c0::Ledger, Vec<u32>) {
    let o = c0::load_objects();
    let ids = c0::Ids::new(&o);
    let bytes = std::fs::read(format!("{}/{}", c0::TC_ROOT, c.scenario.level)).expect("level");
    let level = assets::level::load(&bytes).expect("level loads");
    let mut st = build_match(Path::new(c0::TC_ROOT), &c.cfg, &level)
        .expect("builds")
        .state;
    if let Some(g) = golden {
        assert_eq!(g.len() as u32, c.scenario.ticks + 1, "golden rows 0..=ticks");
        check(&st, &g[0]);
    }
    let mut l = c0::Ledger::default();
    let mut masters = vec![hash_game_state(&st)];
    for k in 1..=c.scenario.ticks {
        let input = [c.scenario.input(k - 1, 0), c.scenario.input(k - 1, 1)];
        let pre = c0::Pre::capture(&st);
        st.process_frame(&[ControlState::unpack(input[0]), ControlState::unpack(input[1])]);
        if let Some(g) = golden {
            check(&st, &g[k as usize]);
        }
        l.observe(&ids, &o, &pre, &st, input);
        masters.push(hash_game_state(&st));
    }
    (l, masters)
}

fn matches_cpp(name: &str) -> c0::Ledger {
    let c = case(name);
    assert_sidecar(name, &c, &c0::load_objects());
    let g = parse_golden(&read(&format!("sim_slice4_5c0_{name}.txt")));
    assert!(g.iter().all(|r| r.game_over == 0), "{name}: C++ never over (lives 99)");
    let (l, _) = drive(&c, Some(&g));
    assert!(
        c0::ok(c0::variant(name), &l),
        "{name}: reach witnesses — {}",
        l.summary()
    );
    l
}

#[test]
fn laser_matches_cpp() {
    // RIFLE / WINCHESTER / GAUSS GUN survive ticks with 8 steps of gravity; a LASER blast
    // lands >= 10 px from every LASER that entered its tick; a beam is removed by a hit.
    matches_cpp("laser");
}

#[test]
fn missile_matches_cpp() {
    // ProcessSteerables turns missiles both ways on both cycle parities; the Up boost is
    // read in the object loop from this tick's input.
    matches_cpp("missile");
}

#[test]
fn trails_matches_cpp() {
    // Three particle trails, the napalm / nuke / hellraider leave_obj trails, MINI NUKE's
    // Create1 splinters, and a formerly-deferred weapon offered by a bonus.
    matches_cpp("trails");
}

#[test]
fn booby_matches_cpp() {
    // A BOOBY TRAP goes off away from every worm with its timer running: a chain.
    matches_cpp("booby");
}

#[test]
fn every_variant_is_internally_deterministic() {
    for name in ["laser", "missile", "trails", "booby"] {
        let c = case(name);
        assert_eq!(drive(&c, None).1, drive(&c, None).1, "{name}");
    }
}

/// Non-vacuity of the comparison itself: one flipped master bit in row 1 must fail.
#[test]
#[should_panic(expected = "tick 1: master")]
fn a_perturbed_golden_row_fails() {
    let c = case("laser");
    let mut g = parse_golden(&read("sim_slice4_5c0_laser.txt"));
    g[1].hashes[0] ^= 1;
    drive(&c, Some(&g));
}
```

- [ ] **Step 2: Create** `rust/oracle-tests/tests/render_slice4_5c0_steer.rs`:

```rust
//! Step 4½c-0 T12 — the STEERABLE CAMERA golden, LIVE (design §5.4).
//!
//! `render_slice4_5c0_steer_scenario.txt` is a generated MISSILE fuzz on
//! physics_fall_test.lev: both worms start dead, respawn in-sim, carry MISSILE in slot 0
//! and never press Change. The C++ `render_live` path drives the REAL Game::ProcessFrame
//! (inputs first, so the Up boost reads this tick's input) and the real ProcessViewports,
//! whose alive arm centres a steering worm's viewport on its missiles' pixel centroid
//! (viewport.cpp:30-32). [`run_stem`] gates frame + state + isolation bit-exact; this file
//! adds the non-vacuity: the witnesses hold on the driven state, and on an off-worm
//! steering tick the centering-only camera sits on the missile centroid.

mod render_slice4d_common;
mod sim_slice4_5c0_common;

use std::path::Path;

use oracle_tests::scenario::Scenario;
use render::viewport::Viewport;
use render_slice4d_common::{modified_stem, read_golden_stem, run_stem};
use sim::state::ControlState;
use sim_core::fixed::ftoi;
use sim_slice4_5c0_common as c0;

const STEM: &str = "render_slice4_5c0_steer";

#[derive(Clone, Copy, Default)]
struct Snap {
    count: i32,
    sum_x: i32,
    sum_y: i32,
    x: i32,
    y: i32,
}

/// Drive the committed scenario sim-only (`scenario::load` + `process_frame`), collecting
/// the steer ledger and per-tick `[worm0, worm1]` snapshots (index == tick) + level size.
fn drive() -> (c0::SteerLedger, Vec<[Snap; 2]>, (i32, i32)) {
    let s = Scenario::parse(&read_golden_stem(STEM, "_scenario.txt")).expect("parses");
    let missile = c0::weapon_index(&c0::load_objects(), "MISSILE");
    let mut st = scenario::load(Path::new(c0::TC_ROOT), &s).state;
    let dims = (st.level.width, st.level.height);
    let mut l = c0::SteerLedger::default();
    let mut snaps = vec![[Snap::default(); 2]];
    for k in 1..=s.ticks {
        let input = [s.input(k - 1, 0), s.input(k - 1, 1)];
        let pre = c0::Pre::capture(&st);
        st.process_frame(&[ControlState::unpack(input[0]), ControlState::unpack(input[1])]);
        l.observe(k, missile, &pre, &st, input);
        snaps.push([0usize, 1].map(|i| {
            let w = &st.worms[i];
            Snap {
                count: w.steerable_count,
                sum_x: w.steerable_sum_x,
                sum_y: w.steerable_sum_y,
                x: ftoi(w.pos.x),
                y: ftoi(w.pos.y),
            }
        }));
    }
    (l, snaps, dims)
}

#[test]
fn steerable_camera_matches_cpp_and_sits_on_the_missile_centroid() {
    // The bit-exact gate: every tick's frame hash == C++, state hash == hash_game_state ==
    // the sim golden master, plus the folded total and the row count.
    let r = run_stem(STEM);
    let (l, snaps, (level_w, level_h)) = drive();
    assert!(c0::steer_ok(&l), "the scenario reaches the steering witnesses: {l:?}");
    assert_eq!(snaps.len() as u32, r.ticks + 1);

    // Non-vacuity: on an off-worm steering tick whose centroid camera differs from the
    // worm-centred one, the centering-only (shake-suppressed) re-render sits on the centroid.
    let layout = Viewport::player_layout();
    let mut proven = false;
    for &(k, i) in &l.off_worm_ticks {
        let s = snaps[k as usize][i];
        let vp = &layout[i];
        let cam = |cx: i32, cy: i32| {
            (
                (cx - vp.center_x).clamp(0, level_w - vp.rect.width()),
                (cy - vp.center_y).clamp(0, level_h - vp.rect.height()),
            )
        };
        let on_missiles = cam(s.sum_x / s.count, s.sum_y / s.count);
        if on_missiles == cam(s.x, s.y) {
            continue;
        }
        let (_, cams) = modified_stem(STEM, k, None, true, false);
        assert_eq!(cams[i], on_missiles, "tick {k} vp{i}: SetCenter(sum / count)");
        proven = true;
        break;
    }
    assert!(proven, "some steering tick moves the camera off the worm onto its missiles");
}
```

- [ ] **Step 3: Run the milestone**

Run: `rustfmt --edition 2021 /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/sim_slice4_5c0_weapons_golden.rs`
Run: `rustfmt --edition 2021 /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/render_slice4_5c0_steer.rs`
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test sim_slice4_5c0_weapons_golden`
Expected: PASS, 7 tests (4 variants, determinism, the perturbation `should_panic`).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test render_slice4_5c0_steer`
Expected: PASS, 1 test.

If a variant diverges, localise before fixing anything: the assertion names the first tick and component. Re-drive with the generator's ledger on the same seed, compare the component (a `wob` divergence on a LASER tick points at M1, `nob` at M5/M6/M7, `sob` at M7/M8, `worm*` at M2/M3), and read the matching C++ lines again. Fix the port, then rerun the whole re-diff — never regenerate a golden to make it pass.

- [ ] **Step 4: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/sim_slice4_5c0_weapons_golden.rs rust/oracle-tests/tests/render_slice4_5c0_steer.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5c-0): MILESTONE — laser/missile/trails/booby + the steerable camera bit-exact vs C++" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

---

### Task 13: Full re-diff, wasm, tripwire sweep, PROGRESS + overview  [Opus review]

**Files:**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md` (header "Last updated" paragraph `:11-34`; the Step 4½ tree's 4½c-0 line `:760-762`; "Open for John" `:785-790`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md` (status line `:3`; the 4½c-0 bullet `:254-258`)

- [ ] **Step 1: The full green board (DEBUG)**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --target wasm32-unknown-unknown` — Expected: builds.
Run: `cargo tree --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim-core --depth 1` — Expected: `sim-core` with no dependencies.

- [ ] **Step 2: The tripwire sweep and the golden audit**

Run: `grep -n "debug_assert!(" /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/weapon.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/nobject.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/sobject.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/render/src/viewport.rs` — Expected: no output (exit 1): design done-when 1.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --name-only --diff-filter=M 03de202 -- rust/oracle-tests/golden` — Expected: no output — no pre-existing golden changed.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --name-only --diff-filter=A 03de202 -- rust/oracle-tests/golden` — Expected: the 15 files `sim_slice4_5c0_{laser,missile,trails,booby}{_setup.cfg,_scenario.txt,.txt}` and `render_slice4_5c0_steer{_scenario.txt,_sim.txt,.txt}` (plus `golden/settings/*` only if the parallel 4½a-2 slice landed meanwhile).
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --name-only 03de202 -- src` — Expected: only `src/tools/oracle_dump/sim_physics_dump.cpp`.

- [ ] **Step 3: Update PROGRESS** — set "Last updated" to the real current date and prepend a paragraph to the header (the previous one becomes "Prior (…)"): **4½c-0 (the unported weapon branches) LANDED — all forty weapons are safe.** Say: the inventory found thirteen weapons, not five (the 4½a design's five + LARPA, BOUNCY LARPA, CRACKLER, MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER, BOOBY TRAP), grouped into eight mechanisms and pinned by `weapon_branch_inventory.rs`; the ports (the `ST_LASER` do-loop incl. the `id == 28` LASER arm, `ProcessSteerables` + `steerable_sum` + the steerable camera, the particle trail, `Create1` splinters, the nobject `leave_obj` trail, chain explosions with free-before-explode, RemExp); the finding that moved input application to the top of the tick (the MISSILE Up boost; the dumper's reduced tail changed too, 21 goldens regenerated byte-identically); the four settings-driven goldens + the `render_live` camera golden bit-exact, with the chosen seeds from T10/T11; no prior golden changed; zero weapon tripwires remain. In the Step 4½ tree replace the 4½c-0 lines with:

```
├─ ✅ 4½c-0 unported Step-2 weapon branches — 13 weapons (not 5): ST_LASER do-loop (RIFLE,
│          WINCHESTER, GAUSS GUN, LASER id 28 unbounded), ProcessSteerables + steerable camera
│          (MISSILE), particle trail (LARPA, BOUNCY LARPA, CRACKLER), Create1 splinters + leave_obj
│          trails (MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER), chain explosions (BOOBY TRAP), RemExp;
│          inputs now apply at the top of the tick (as C++). 🎯 4 settings goldens + 1 render_live
│          golden bit-exact; weap_table all 0 — all 40 weapons safe for 4½c           COMPLETE
```

and under "Open for John" delete the clause about where the laser/steerable weapons get ported (it is done).

- [ ] **Step 4: Update the overview** — status line: add "**4½c-0 LANDED**" after "4½b complete on `liero-rs-step-4-5`". In the 4½c-0 bullet, append: "**Landed** (design `specs/2026-09-10-liero-rs-step4.5-slice4.5c0-weapon-branches-design.md`): the inventory found **thirteen** weapons, not five — LARPA, BOUNCY LARPA, CRACKLER, MINI NUKE, BIG NUKE, NAPALM, HELLRAIDER and BOOBY TRAP also reached unported branches — and MISSILE was a silent divergence, not a panic. All ported bit-exact; the steerable camera deferral from 4d is closed; input application moved to the top of the tick (the C++ order, exposed by the MISSILE Up boost). 4½c may offer all forty weapons."

- [ ] **Step 5: Broad review (Opus)** — re-read the whole 4½c-0 diff against the design: every row of design §2.2 is ported and golden-reached (map each weapon to its witness in §5.3); `wobject_process` transcribes `weapon.cpp:127-338` end to end (RemExp once, the loop, the body); `used` is justified by the inventory pin; chain recursion frees before exploding in both the driver and `sobject_create`; `SimState::new` unchanged; no scenario directive added; the dumper's only change is the input point, with the 21-golden evidence recorded; the 4½a-1 ban provenance untouched; no push, no PR. Bar: 0 Critical / 0 Important.

- [ ] **Step 6: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "docs(4.5c-0): PROGRESS + overview — slice 4.5c-0 landed, all 40 weapons safe" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: <session-url>"
```

## Done-report (each task)

(a) what changed and why, (b) files touched, (c) tests run + risks. Per-task commit on `liero-rs-step-4-5`; no push, no PR. The final report surfaces: the inventory pin (T0), the 21-golden regeneration evidence (T8), the chosen seeds and ledgers (T10, T11), the milestone result (T12), the tripwire sweep (T13), and confirmation that no pre-existing golden changed.
