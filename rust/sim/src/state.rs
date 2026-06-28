//! Frame-0 simulation state and the builder that constructs it.
//!
//! This is the *initial* state — what the engine holds after a level is loaded
//! and worms are added, but **before** any `ProcessFrame` runs. It mirrors the
//! subset of the C++ `fast_snapshot.hpp` inventory that the tick-0 state hash
//! reads (see the Step 2 Slice 1 design doc, *Datamodel*); later slices widen
//! [`WormState`] and fill the (here empty) object pools.
//!
//! No dynamics live here: no physics, no RNG consumption, no spawned objects.

use assets::level::LevelData;
use sim_core::fixed::Fixed;
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::physics::{worm_process_physics, worm_reactions, PhysicsConsts};
use crate::pool::{BloodPool, Pool};

/// Number of weapon slots per worm. Mirrors C++ `NUM_WEAPONS` (`worm.hpp:13`).
/// `Settings::kSelectableWeapons` is also 5, so `InitWeapons` fills every slot.
pub const NUM_WEAPONS: usize = 5;

// Object-pool capacities, matching the C++ `ExactObjectList` limits in
// `game.hpp:142-145`. Empty this slice, so the values are not load-bearing for
// the tick-0 hash, but we pin them so later slices spawn against the right caps.
const BONUS_CAPACITY: usize = 99;
const WOBJECT_CAPACITY: usize = 600;
const SOBJECT_CAPACITY: usize = 700;
const NOBJECT_CAPACITY: usize = 600;
/// Blood-particle pool cap. C++ sizes this from `settings->blood_particle_max`
/// (`game.cpp:513`), whose default is 700 (`settings.hpp:37`). Empty this slice.
const BLOOD_CAPACITY: usize = 700;

/// A weapon's identity: its `id`, i.e. its array index in
/// `assets::object::Objects::weapons`. The state hash reads `type->id`.
pub type WeaponId = i32;

// ---------------------------------------------------------------------------
// ControlState — 7-bit packed input (mirrors worm.hpp ControlState)
// ---------------------------------------------------------------------------

/// The seven worm control bits, packed into a `u32` (mirrors the
/// `ControlState` struct in `worm.hpp`). Bit layout matches `WormSettings`'
/// `Control` enum: Up=0, Down=1, Left=2, Right=3, Fire=4, Change=5, Jump=6.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ControlState(u32);

impl ControlState {
    pub const UP: u32 = 0;
    pub const DOWN: u32 = 1;
    pub const LEFT: u32 = 2;
    pub const RIGHT: u32 = 3;
    pub const FIRE: u32 = 4;
    pub const CHANGE: u32 = 5;
    pub const JUMP: u32 = 6;

    /// An empty control state (no keys pressed) — the tick-0 value.
    pub fn new() -> Self {
        ControlState(0)
    }

    /// Packs to the raw `istate` word, matching C++ `Pack()` (`worm.hpp:155`),
    /// which returns `istate` unmasked (the 7-bit mask is commented out there).
    /// Because the state only ever enters via [`unpack`](Self::unpack) (masked)
    /// or [`set`](Self::set) (bits 0..7), the value is effectively 7-bit.
    pub fn pack(self) -> u32 {
        self.0
    }

    /// Unpacks an input word, masking to 7 bits, matching C++ `Unpack()`
    /// (`worm.hpp:159`: `istate = state & 0x7f`).
    pub fn unpack(state: u32) -> Self {
        ControlState(state & 0x7f)
    }

    /// Whether control bit `n` is set.
    pub fn get(self, n: u32) -> bool {
        (self.0 >> n) & 1 != 0
    }

    /// Sets or clears control bit `n` (mirrors `ControlState::Set`).
    pub fn set(&mut self, n: u32, v: bool) {
        if v {
            self.0 |= 1 << n;
        } else {
            self.0 &= !(1u32 << n);
        }
    }
}

// ---------------------------------------------------------------------------
// Ninjarope
// ---------------------------------------------------------------------------

/// The worm's ninjarope. Tick-0 subset of C++ `Ninjarope` (`worm.hpp:19`): the
/// hash reads `out` and `pos`. Later slices add `attached`/`length`/`vel`/...
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Ninjarope {
    /// Is the rope deployed? Hashed as its 0/1 int value.
    pub out: bool,
    pub pos: Vec2,
}

// ---------------------------------------------------------------------------
// WormWeapon
// ---------------------------------------------------------------------------

/// One of a worm's weapon slots. Mirrors C++ `WormWeapon` (`worm.hpp:32`); the
/// hash reads `ammo`, `delay_left`, `loading_left`, and `ty.id` (only when set).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WormWeapon {
    /// The resolved weapon's id, or `None` for an empty slot (C++ `type` ptr).
    pub ty: Option<WeaponId>,
    pub ammo: i32,
    pub delay_left: i32,
    pub loading_left: i32,
}

// ---------------------------------------------------------------------------
// WormState + its scenario init
// ---------------------------------------------------------------------------

/// The per-weapon-slot init the scenario supplies, already resolved to a weapon
/// id + starting ammo. The builder copies it into a [`WormWeapon`] with
/// `delay_left = loading_left = 0`, reproducing `Worm::InitWeapons`
/// (`worm.cpp:698`). See [`WormInit::resolve_weapons`] for resolving these from
/// a real `Objects` table the way C++ does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WeaponInit {
    pub ty: Option<WeaponId>,
    pub ammo: i32,
}

/// The scenario description for one worm: the fields `Game::ResetWorms` /
/// `Worm::InitWeapons` set plus identity (`index`, `stats_x`). The builder turns
/// each of these into a tick-0 [`WormState`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WormInit {
    /// 0 or 1 (mirrors `Worm::index`).
    pub index: i32,
    /// Starting health (`w.health = w.settings->health`).
    pub health: i32,
    /// Starting lives (`w.lives = settings->lives`).
    pub lives: i32,
    /// Stats-panel X (`w.stats_x`). A scenario field; **not** read by the hash.
    pub stats_x: i32,
    /// The five selectable weapons, pre-resolved to id + ammo.
    pub weapons: [WeaponInit; NUM_WEAPONS],
    /// Starting position (16.16 fixed-point), copied into [`WormState::pos`].
    /// Slice 1 hard-coded `(0,0)`; the Slice-2 scenario places a worm mid-air.
    pub start_pos: Vec2,
    /// Whether the worm is visible at tick 0 (`Worm::visible`). Slice 1 was
    /// always `false`; the Slice-2 scenario spawns a visible, falling worm.
    pub visible: bool,
}

impl WormInit {
    /// Resolve the selectable weapons exactly as C++ `Worm::InitWeapons`
    /// (`worm.cpp:702-708`): for each slot `j`,
    /// `type = weapons[weap_order[settings_weapons[j] - 1]]`, `ammo = type.ammo`.
    ///
    /// `settings_weapons` are the 1-based menu choices (`settings->weapons[j]`),
    /// `weap_order` is the engine's `weap_order` permutation, and `objects` is
    /// the loaded weapon table. The returned ids are the resolved weapons' `id`
    /// (== their index in `objects.weapons`). Task 7 uses this to match the
    /// oracle; unit tests build [`WeaponInit`]s directly.
    pub fn resolve_weapons(
        objects: &assets::object::Objects,
        weap_order: &[usize],
        settings_weapons: &[u32; NUM_WEAPONS],
    ) -> [WeaponInit; NUM_WEAPONS] {
        let mut out = [WeaponInit::default(); NUM_WEAPONS];
        for (j, slot) in out.iter_mut().enumerate() {
            let order_idx = settings_weapons[j] as usize - 1;
            let weap_idx = weap_order[order_idx];
            let w = &objects.weapons[weap_idx];
            *slot = WeaponInit {
                ty: Some(w.id),
                ammo: w.ammo,
            };
        }
        out
    }
}

/// One worm's frame-0 state. Only the fields the tick-0 hash reads, plus the
/// scenario init fields (`index`, `stats_x`); later slices widen this toward
/// full `WormSimState` parity (design doc, *Datamodel*).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WormState {
    pub pos: Vec2,
    pub vel: Vec2,
    pub aiming_angle: Fixed,
    pub health: i32,
    pub lives: i32,
    pub kills: i32,
    pub timer: i32,
    pub visible: bool,
    pub killed_timer: i32,
    pub control_states: ControlState,
    pub weapons: [WormWeapon; NUM_WEAPONS],
    pub ninjarope: Ninjarope,
    /// Worm index (0 or 1). Scenario identity; not hashed.
    pub index: i32,
    /// Stats-panel X. Scenario field; not hashed.
    pub stats_x: i32,
}

/// `Worm::kKilledTimerInitial` (`worm.hpp:243`): the respawn countdown the worm
/// constructor and `ResetWorms` both set.
pub const KILLED_TIMER_INITIAL: i32 = 150;

impl WormState {
    /// Builds the tick-0 worm from its scenario init, reproducing the post-`AddWorm`
    /// / `ResetWorms` / `InitWeapons` state: `pos = init.start_pos`, zero
    /// velocity/aim, `kills` and `timer` zero, `visible = init.visible`,
    /// `killed_timer = 150`, no input, the rope stowed, and each weapon slot
    /// loaded with `delay_left = loading_left = 0`.
    pub fn from_init(init: &WormInit) -> WormState {
        let weapons = init.weapons.map(|w| WormWeapon {
            ty: w.ty,
            ammo: w.ammo,
            delay_left: 0,
            loading_left: 0,
        });
        WormState {
            pos: init.start_pos,
            vel: Vec2::zero(),
            aiming_angle: 0,
            health: init.health,
            lives: init.lives,
            kills: 0,
            timer: 0,
            visible: init.visible,
            killed_timer: KILLED_TIMER_INITIAL,
            control_states: ControlState::new(),
            weapons,
            ninjarope: Ninjarope::default(),
            index: init.index,
            stats_x: init.stats_x,
        }
    }
}

// ---------------------------------------------------------------------------
// Object-pool element types (empty this slice; fields are what the hash reads)
// ---------------------------------------------------------------------------

/// A weapon bonus crate. Hash reads `x, y, timer, weapon, frame`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bonus {
    pub x: i32,
    pub y: i32,
    pub timer: i32,
    pub weapon: i32,
    pub frame: i32,
}

/// A weapon projectile. Hash reads `pos, vel, cur_frame, time_left, ty.id`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WObject {
    pub pos: Vec2,
    pub vel: Vec2,
    pub cur_frame: i32,
    pub time_left: i32,
    pub ty: Option<WeaponId>,
}

/// A "sound/explosion" object. Hash reads `id, cur_frame`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SObject {
    pub id: i32,
    pub cur_frame: i32,
}

/// A non-weapon object (debris/splinters). Hash reads `pos, vel, cur_frame, ty.id`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NObject {
    pub pos: Vec2,
    pub vel: Vec2,
    pub cur_frame: i32,
    pub ty: Option<i32>,
}

/// A blood particle. Hash reads `pos`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BObject {
    pub pos: Vec2,
}

// ---------------------------------------------------------------------------
// LevelSim
// ---------------------------------------------------------------------------

/// The level material buffer the simulation/hash needs: dimensions plus the
/// per-pixel material id map. `materials`/display are derived/render and omitted
/// (design doc, *Datamodel*).
///
/// `material_flags` is the 256-entry flag table (`TcConfig.materials`); entry
/// `m` is the flag byte for material index `m`, with `Background = 1 << 3`
/// (`material.hpp:11`). It is what the C++ engine precomputes as
/// `materials[idx] = common.materials[material_id[idx]]` — here we keep the flag
/// table once and index it per probe (see [`LevelSim::checked_mat_background`]).
/// Not hashed; the tick-0 level hash reads `material_id` only.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LevelSim {
    pub width: i32,
    pub height: i32,
    pub material_id: Vec<u8>,
    pub material_flags: [u8; 256],
}

/// `Material::kBackground` (`material.hpp:11`): the flag bit a "background"
/// (empty/walkable) material carries.
pub const MAT_BACKGROUND: u8 = 1 << 3;

impl LevelSim {
    /// Port of `Level::CheckedMatWrap(x, y).Background()` (`level.hpp:124-130` +
    /// `material.hpp:18`). Reproduced **bit-for-bit**, including two load-bearing
    /// quirks the physics depends on:
    ///
    /// * The flattened index is `static_cast<unsigned int>(x + y * width)` — a
    ///   two's-complement reinterpret of `x + y*width`. There is **no separate
    ///   `x`-bounds check**, so a negative `x` paired with a `y` that keeps
    ///   `x + y*width` inside `[0, w*h)` reads a *wrapped, wrong-row* pixel.
    /// * The out-of-range fallback returns `zero_material = common.materials[0]`
    ///   (`level.hpp:24`), i.e. flag-table entry **0** — **not**
    ///   `material_flags[material_id[0]]`.
    ///
    /// In range it returns `material_flags[material_id[idx]]` (look up the
    /// pixel's material id, then its flag byte) and tests the background bit.
    pub fn checked_mat_background(&self, x: i32, y: i32) -> bool {
        // Two's-complement unsigned reinterpret of the flattened coordinate,
        // matching `static_cast<unsigned int>(x + y * width)`.
        let idx = x.wrapping_add(y.wrapping_mul(self.width)) as u32 as usize;
        let flags = if idx < self.material_id.len() {
            self.material_flags[self.material_id[idx] as usize]
        } else {
            // OOB -> zero_material == common.materials[0] == flag-table entry 0.
            self.material_flags[0]
        };
        (flags & MAT_BACKGROUND) != 0
    }
}

// ---------------------------------------------------------------------------
// SimState + builder
// ---------------------------------------------------------------------------

/// The whole frame-0 simulation state.
pub struct SimState {
    pub rand: Rand,
    pub cycles: i32,
    pub level: LevelSim,
    pub worms: Vec<WormState>,
    pub bonuses: Pool<Bonus>,
    pub wobjects: Pool<WObject>,
    pub sobjects: Pool<SObject>,
    pub nobjects: Pool<NObject>,
    pub bobjects: BloodPool<BObject>,
    /// The TC physics constants/hacks (`WormGravity`, friction, `MinBounce*`,
    /// …) the worm-physics pass reads. Built once from the TC; not hashed.
    pub physics: PhysicsConsts,
}

impl SimState {
    /// Build the tick-0 state from a loaded level + a worm-init list, seeding the
    /// RNG with `seed`. No RNG is consumed (the level is *loaded*, not generated),
    /// so `rand.last() == 0`; `cycles == 0`; all object pools start empty.
    ///
    /// Weapon resolution is done *before* this call (each [`WormInit`] carries
    /// already-resolved [`WeaponInit`]s); see [`WormInit::resolve_weapons`] for
    /// the `Objects`/`weap_order` path Task 7 uses against the real data.
    ///
    /// `material_flags` is the loaded TC's 256-entry flag table
    /// (`TcConfig.materials`); it feeds [`LevelSim::checked_mat_background`], the
    /// collision-probe port. The caller passes the real table (the differential
    /// dumper/test load it from `tc.cfg`).
    pub fn new(
        level: &LevelData,
        worms_init: &[WormInit],
        seed: u32,
        material_flags: &[u8; 256],
        physics: PhysicsConsts,
    ) -> SimState {
        let mut rand = Rand::new();
        rand.seed(seed);
        let worms = worms_init.iter().map(WormState::from_init).collect();
        SimState {
            rand,
            cycles: 0,
            level: LevelSim {
                width: level.width,
                height: level.height,
                material_id: level.material_id.clone(),
                material_flags: *material_flags,
            },
            worms,
            bonuses: Pool::new(BONUS_CAPACITY),
            wobjects: Pool::new(WOBJECT_CAPACITY),
            sobjects: Pool::new(SOBJECT_CAPACITY),
            nobjects: Pool::new(NOBJECT_CAPACITY),
            bobjects: BloodPool::new(BLOOD_CAPACITY),
            physics,
        }
    }

    /// Advance one worm-physics tick (Slice 2): a *worms-only* pass, **not** the
    /// full `Game::ProcessFrame` (no `cycles++`, no bonus-drop RNG roll, no
    /// object `Process` loops — those land in Slice 6). Named `process_worm_physics`
    /// to keep that distinction honest.
    ///
    /// Applies each worm's scripted input to its `control_states`, then — for
    /// each worm in `worms` order — runs the reaction orchestration
    /// ([`worm_reactions`]) followed by [`worm_process_physics`]. Inputs shorter
    /// than `worms` leave the remaining worms' control state unchanged (Slice 2
    /// drives all-empty input regardless).
    pub fn process_worm_physics(&mut self, inputs: &[ControlState]) {
        for (w, input) in self.worms.iter_mut().zip(inputs.iter()) {
            w.control_states = *input;
        }

        // Disjoint field borrows: the per-worm pass reads `level`/`physics`
        // while mutating each worm in turn.
        let SimState { level, physics, worms, .. } = self;
        for w in worms.iter_mut() {
            let reacts = worm_reactions(level, w, physics);
            worm_process_physics(w, &reacts, physics);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A tiny 4x4 synthetic level with a known material pattern.
    fn synthetic_level() -> LevelData {
        let material_id: Vec<u8> = (0..16).map(|i| (i * 3 + 1) as u8).collect();
        LevelData {
            width: 4,
            height: 4,
            material_id,
            palette: None,
            display: None,
        }
    }

    // Two worms, each with a distinct weapon loadout, mirroring the C++ fixture
    // (worm 0 at stats_x 0, worm 1 at stats_x 218).
    fn two_worms() -> Vec<WormInit> {
        let weapons0 = [
            WeaponInit { ty: Some(0), ammo: 10 },
            WeaponInit { ty: Some(1), ammo: 1 },
            WeaponInit { ty: Some(2), ammo: 50 },
            WeaponInit { ty: Some(3), ammo: 3 },
            WeaponInit { ty: Some(4), ammo: 25 },
        ];
        let weapons1 = [
            WeaponInit { ty: Some(5), ammo: 2 },
            WeaponInit { ty: Some(6), ammo: 8 },
            WeaponInit { ty: Some(7), ammo: 100 },
            WeaponInit { ty: Some(8), ammo: 4 },
            WeaponInit { ty: Some(9), ammo: 1 },
        ];
        vec![
            WormInit {
                index: 0,
                health: 100,
                lives: 5,
                stats_x: 0,
                weapons: weapons0,
                start_pos: Vec2::zero(),
                visible: false,
            },
            WormInit {
                index: 1,
                health: 100,
                lives: 5,
                stats_x: 218,
                weapons: weapons1,
                start_pos: Vec2::zero(),
                visible: false,
            },
        ]
    }

    #[test]
    fn builds_tick0_global_state() {
        let level = synthetic_level();
        let state = SimState::new(&level, &two_worms(), 0x1234, &[0u8; 256], PhysicsConsts::default());
        assert_eq!(state.cycles, 0, "cycles must be 0 at tick 0");
        assert_eq!(state.rand.last(), 0, "no RNG consumed -> last() == 0");
        assert_eq!(state.level.width, 4);
        assert_eq!(state.level.height, 4);
        assert_eq!(state.level.material_id, level.material_id, "material map copied verbatim");
        assert_eq!(state.worms.len(), 2);
    }

    #[test]
    fn pools_start_empty() {
        let state = SimState::new(&synthetic_level(), &two_worms(), 1, &[0u8; 256], PhysicsConsts::default());
        assert!(state.bonuses.is_empty());
        assert!(state.wobjects.is_empty());
        assert!(state.sobjects.is_empty());
        assert!(state.nobjects.is_empty());
        assert!(state.bobjects.is_empty());
        assert_eq!(state.bonuses.iter().count(), 0);
        assert_eq!(state.bobjects.iter().count(), 0);
        // Capacities pinned to the C++ limits.
        assert_eq!(state.bonuses.capacity(), 99);
        assert_eq!(state.wobjects.capacity(), 600);
        assert_eq!(state.sobjects.capacity(), 700);
        assert_eq!(state.nobjects.capacity(), 600);
        assert_eq!(state.bobjects.capacity(), 700);
    }

    #[test]
    fn worm_tick0_scalar_values() {
        let state = SimState::new(&synthetic_level(), &two_worms(), 7, &[0u8; 256], PhysicsConsts::default());
        for w in &state.worms {
            assert_eq!(w.pos, Vec2::zero());
            assert_eq!(w.vel, Vec2::zero());
            assert_eq!(w.aiming_angle, 0);
            assert_eq!(w.kills, 0);
            assert_eq!(w.timer, 0);
            assert!(!w.visible, "worm starts invisible");
            assert_eq!(w.killed_timer, 150, "kKilledTimerInitial");
            assert_eq!(w.control_states.pack(), 0, "no input applied yet");
            assert!(!w.ninjarope.out, "rope stowed");
            assert_eq!(w.ninjarope.pos, Vec2::zero());
            assert_eq!(w.health, 100);
            assert_eq!(w.lives, 5);
        }
        // Scenario identity preserved.
        assert_eq!(state.worms[0].index, 0);
        assert_eq!(state.worms[0].stats_x, 0);
        assert_eq!(state.worms[1].index, 1);
        assert_eq!(state.worms[1].stats_x, 218);
    }

    #[test]
    fn worm_weapons_initialised() {
        let state = SimState::new(&synthetic_level(), &two_worms(), 7, &[0u8; 256], PhysicsConsts::default());
        let w0 = &state.worms[0];
        // Each slot has its type set, ammo from the init, and zero timers.
        for (j, ww) in w0.weapons.iter().enumerate() {
            assert_eq!(ww.ty, Some(j as WeaponId), "slot {j} type set");
            assert_eq!(ww.delay_left, 0);
            assert_eq!(ww.loading_left, 0);
        }
        assert_eq!(w0.weapons[0].ammo, 10);
        assert_eq!(w0.weapons[2].ammo, 50);
        assert_eq!(state.worms[1].weapons[2].ty, Some(7));
        assert_eq!(state.worms[1].weapons[2].ammo, 100);
    }

    #[test]
    fn from_init_honours_start_pos_and_visible() {
        // A non-zero start position and a visible worm flow straight through
        // `from_init` (Slice 2 scenario: a visible worm placed mid-air).
        let init = WormInit {
            index: 0,
            health: 100,
            lives: 10,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::new(6553600, 3276800),
            visible: true,
        };
        let w = WormState::from_init(&init);
        assert_eq!(w.pos, Vec2::new(6553600, 3276800), "pos = init.start_pos");
        assert!(w.visible, "visible = init.visible");
        assert_eq!(w.vel, Vec2::zero(), "vel still starts at zero");
    }

    #[test]
    fn from_init_defaults_match_slice1() {
        // Slice-1 defaults: start_pos = (0,0), visible = false keep tick-0 parity.
        let init = WormInit {
            index: 0,
            health: 100,
            lives: 10,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::zero(),
            visible: false,
        };
        let w = WormState::from_init(&init);
        assert_eq!(w.pos, Vec2::zero());
        assert!(!w.visible);
    }

    #[test]
    fn control_state_pack_unpack_masks_to_7_bits() {
        // Unpack masks to 0x7f; bits above 7 are dropped (C++ `state & 0x7f`).
        assert_eq!(ControlState::unpack(0xffff_ffff).pack(), 0x7f);
        assert_eq!(ControlState::unpack(0).pack(), 0);
        let mut cs = ControlState::new();
        cs.set(ControlState::FIRE, true);
        assert!(cs.get(ControlState::FIRE));
        assert_eq!(cs.pack(), 1 << 4);
        cs.set(ControlState::FIRE, false);
        assert_eq!(cs.pack(), 0);
    }

    #[test]
    fn resolve_weapons_mirrors_init_weapons() {
        // Build a synthetic Objects table whose ammo == 10 * index, then resolve
        // through a non-identity weap_order to prove the indirection is applied.
        use assets::object::{Objects, Weapon};
        let weapons: Vec<Weapon> = (0..5)
            .map(|i| Weapon { id: i, ammo: i * 10, ..Default::default() })
            .collect();
        let objects = Objects { weapons, ..Default::default() };
        // weap_order reverses: order index 0 -> weapon 4, etc.
        let weap_order = [4usize, 3, 2, 1, 0];
        // settings.weapons are 1-based menu choices selecting order slots 1,2,3,4,5.
        let settings_weapons = [1u32, 2, 3, 4, 5];
        let resolved = WormInit::resolve_weapons(&objects, &weap_order, &settings_weapons);
        // slot 0: order[0]=4 -> weapon id 4, ammo 40; slot 4: order[4]=0 -> id 0, ammo 0.
        assert_eq!(resolved[0].ty, Some(4));
        assert_eq!(resolved[0].ammo, 40);
        assert_eq!(resolved[4].ty, Some(0));
        assert_eq!(resolved[4].ammo, 0);
    }

    // ---- checked_mat_background (CheckedMatWrap port) -------------------------

    // A synthetic 4x4 LevelSim crafted to pin every branch of the port:
    //
    //  - flag table entry 0 has NO background bit (the OOB fallback);
    //  - material_id[0] points at material 1, which DOES have it — so the OOB
    //    test can tell `material_flags[0]` (correct) from
    //    `material_flags[material_id[0]]` (the common mistake);
    //  - idx 5 (1,1) is a background pixel, idx 10 (2,2) is a rock pixel;
    //  - idx 3 (row 0, col 3) is a background pixel, used by the wrap test:
    //    the probe (x=-1, y=1) flattens to -1 + 1*4 = 3, a wrapped wrong-row cell.
    //
    // material ids used: 0 (no flags), 1 (background), 2 (rock), 7 (background).
    fn probe_level() -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[0] = 0x00; // zero_material: deliberately NOT background
        material_flags[1] = MAT_BACKGROUND; // 0x08
        material_flags[2] = 1 << 2; // kRock, no background bit
        material_flags[7] = MAT_BACKGROUND; // 0x08

        // 16 cells, row-major (width 4). Defaults to material 0 (no flags).
        let mut material_id = vec![0u8; 16];
        material_id[0] = 1; // (0,0): background-flagged material (OOB decoy)
        material_id[3] = 7; // (3,0): background -> the wrapped cell (-1,1) lands here
        material_id[5] = 1; // (1,1): background pixel
        material_id[10] = 2; // (2,2): rock pixel

        LevelSim {
            width: 4,
            height: 4,
            material_id,
            material_flags,
        }
    }

    #[test]
    fn checked_mat_background_in_bounds_reads_pixel_material() {
        let lvl = probe_level();
        // (1,1) -> idx 5 -> material 1 -> background bit set.
        assert!(lvl.checked_mat_background(1, 1), "background pixel -> true");
        // (2,2) -> idx 10 -> material 2 (rock) -> no background bit.
        assert!(!lvl.checked_mat_background(2, 2), "rock pixel -> false");
    }

    #[test]
    fn checked_mat_background_oob_uses_flag_table_index_0_not_material_id_0() {
        let lvl = probe_level();
        // (100,100) flattens to 100 + 100*4 = 500 >= 16 -> out of range.
        // Correct C++ fallback: zero_material == common.materials[0] ==
        // material_flags[0] == 0x00 -> false.
        // The common mistake material_flags[material_id[0]] would read
        // material_flags[1] == 0x08 -> true. Asserting false proves which is read.
        assert!(
            !lvl.checked_mat_background(100, 100),
            "OOB must read flag-table entry 0 (not material_id[0]'s flags)"
        );
        // A large positive y is OOB the same way.
        assert!(!lvl.checked_mat_background(0, 1000), "large y is OOB -> entry 0");
    }

    #[test]
    fn checked_mat_background_negative_x_wraps_to_wrong_row() {
        let lvl = probe_level();
        // The trap: there is no separate x-range check. (x=-1, y=1) flattens to
        // -1 + 1*4 = 3, which is in range [0,16) -> reads idx 3 (row 0, col 3),
        // a WRAPPED wrong-row cell (material 7, background) rather than failing
        // bounds. The wrapped cell differs from the OOB fallback (entry 0, false),
        // so a true result can only come from reading idx 3.
        assert!(
            lvl.checked_mat_background(-1, 1),
            "negative x wraps to in-range idx 3 and reads that pixel"
        );
        // Sanity: idx 3 is indeed the cell being read — flip it to rock and the
        // same probe must now report not-background.
        let mut lvl2 = probe_level();
        lvl2.material_id[3] = 2; // rock at the wrapped cell
        assert!(
            !lvl2.checked_mat_background(-1, 1),
            "probe reads idx 3 specifically (wrapped wrong-row cell)"
        );
    }

    #[test]
    fn sim_state_new_fills_material_flags_from_table() {
        // SimState::new copies the caller-supplied flag table into the LevelSim,
        // wiring checked_mat_background to the real TC data.
        let mut flags = [0u8; 256];
        flags[7] = MAT_BACKGROUND;
        let level = synthetic_level(); // material_id[i] = i*3+1; idx 2 -> material 7
        let state = SimState::new(&level, &two_worms(), 0, &flags, PhysicsConsts::default());
        assert_eq!(state.level.material_flags, flags, "flag table copied verbatim");
        // synthetic_level idx 2 (x=2,y=0) = material 7 -> background.
        assert!(state.level.checked_mat_background(2, 0));
        // idx 0 = material 1 -> no flag set -> false.
        assert!(!state.level.checked_mat_background(0, 0));
    }
}
